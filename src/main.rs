mod cli;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use clap::Parser;
use sha2::{Digest, Sha256};

use cli::{Cli, Command, UpdateArgs};
use rf::backup;
use rf::download;
use rf::patch::patch_data;
use rf::{save_format, source_detect};

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Verify { path } => verify(&path),
        Command::Inspect { path } => inspect(&path),
        Command::Download { output_dir } => {
            let path = download::download_latest(&output_dir)?;
            println!("Downloaded: {}", path.display());
            Ok(())
        }
        Command::Update(args) => update(args),
    }
}

fn verify(path: &Path) -> Result<()> {
    let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    let t3db = save_format::validate_data(&bytes)?;
    println!("DATA is valid");
    println!("T3DB offset: {t3db}");
    println!("Size: {} bytes", bytes.len());
    if let Ok(name) = save_format::save_name(&bytes) {
        println!("Save name: {name}");
    }
    println!(
        "Database SHA-256: {}",
        hash(&bytes[t3db..bytes.len() - save_format::BNRY_BLOCK_SIZE])
    );
    Ok(())
}

fn inspect(path: &Path) -> Result<()> {
    if path.is_file() {
        let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
        if let Ok(t3db) = save_format::validate_data(&bytes) {
            println!(
                "Kind: PS4 DATA\nT3DB offset: {t3db}\nSize: {} bytes",
                bytes.len()
            );
            return Ok(());
        }
    }
    let source = source_detect::detect(path)?;
    let database = source_detect::database(&source)?;
    println!(
        "Kind: EA squad source\nDatabase size: {} bytes\nDatabase SHA-256: {}",
        database.len(),
        hash(&database)
    );
    Ok(())
}

fn update(args: UpdateArgs) -> Result<()> {
    let (data_path, save_folder) = match args.data {
        Some(path) => {
            let folder = path
                .parent()
                .filter(|parent| {
                    parent
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().contains("_Squads"))
                })
                .map(Path::to_owned);
            (path, folder)
        }
        None => {
            let folder = rf::platform::find_apollo_save()?;
            (folder.join("DATA"), Some(folder))
        }
    };
    let user_data =
        fs::read(&data_path).with_context(|| format!("cannot read {}", data_path.display()))?;
    let t3db = save_format::validate_data(&user_data)?;
    let backup_path = if !args.dry_run {
        let backup_path =
            backup::ensure_backup(&data_path, &user_data, &args.backup_dir, save_folder.as_deref())?;
        println!("Backup: {}", backup_path.display());
        Some(backup_path)
    } else {
        None
    };
    let source = match args.squad_file.as_deref() {
        Some(path) => source_detect::detect(path)?,
        None => match source_detect::detect(&args.download_dir) {
            Ok(source) => source,
            Err(_) => {
                let path = download::download_latest(&args.download_dir)?;
                source_detect::detect(&path)?
            }
        },
    };
    let database = source_detect::database(&source)?;
    let current = &user_data[t3db..user_data.len() - save_format::BNRY_BLOCK_SIZE];
    println!("Current database SHA-256: {}", hash(current));
    println!("New database SHA-256:     {}", hash(&database));
    if current == database.as_slice() {
        println!("No updates available.");
        return Ok(());
    }
    let patched = patch_data(&user_data, &database)?;
    save_format::validate_data(&patched)?;
    if args.dry_run {
        println!("Dry run: no files were changed.");
        println!(
            "Would write {} bytes to {}",
            patched.len(),
            args.output_dir.display()
        );
        return Ok(());
    }
    fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("cannot create {}", args.output_dir.display()))?;
    let output_container = args.output_dir.join(backup::container_name(&user_data));
    if output_container.exists() {
        fs::remove_dir_all(&output_container)
            .with_context(|| format!("cannot replace {}", output_container.display()))?;
    }
    if let (Some(path), Some(folder)) = (&backup_path, &save_folder) {
        let backup_save = path.join(folder.file_name().context("save folder has no name")?);
        backup::mirror_save_folder(
            &backup_save,
            &output_container.join(folder.file_name().context("save folder has no name")?),
            &patched,
        )?;
    }
    let destination = if let Some(folder) = &save_folder {
        output_container
            .join(folder.file_name().context("save folder has no name")?)
            .join("DATA")
    } else {
        output_container.join("DATA")
    };
    fs::create_dir_all(destination.parent().context("invalid output path")?)?;
    let temporary = destination.with_extension("part");
    fs::write(&temporary, &patched)
        .with_context(|| format!("cannot write {}", temporary.display()))?;
    fs::rename(&temporary, &destination)
        .with_context(|| format!("cannot commit {}", destination.display()))?;
    if let Some(path) = backup_path {
        backup::save_database_hash(&path, &database)?;
    }
    println!("Patched DATA written to {}", destination.display());
    Ok(())
}

fn hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
