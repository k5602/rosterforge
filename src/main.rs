mod cli;

use std::fs;
use std::io;
use std::path::Path;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use sha2::{Digest, Sha256};

use cli::{Cli, Command, RestoreArgs, UpdateArgs};
use rf::backup;
use rf::download::{self, SquadKind};
use rf::error::SourceError;
use rf::patch::patch_data;
use rf::{save_format, source_detect};

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Verify { path } => verify(&path),
        Command::Inspect { path } => inspect(&path),
        Command::Download {
            platform,
            fut,
            content_url,
            output_dir,
        } => {
            let kind = if fut {
                SquadKind::Fut
            } else {
                SquadKind::Major
            };
            let path = download::download_latest(&output_dir, &content_url, platform.into(), kind)?;
            println!("Downloaded: {}", path.display());
            Ok(())
        }
        Command::Restore(args) => restore(&args),
        Command::Update(args) => update(args),
        Command::Completions { shell } => write_stdout_ignoring_broken_pipe(|out| {
            generate(shell, &mut Cli::command(), "rf", out);
            Ok(())
        }),
        Command::Manpage => print_manpage(),
    }
}

/// Write generated text to stdout while tolerating closed pipes such as
/// `rf manpage | head`.
fn write_stdout_ignoring_broken_pipe(
    write: impl FnOnce(&mut dyn io::Write) -> io::Result<()>,
) -> Result<()> {
    let mut stdout = io::stdout().lock();
    if let Err(error) = write(&mut stdout)
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(error.into());
    }
    Ok(())
}

fn print_manpage() -> Result<()> {
    write_stdout_ignoring_broken_pipe(|out| clap_mangen::Man::new(Cli::command()).render(out))
}

/// Copy a verified backup DATA file back over a squad save.
///
/// The destination must be a valid squad DATA file or live inside a
/// `*_Squads` Apollo folder. This prevents accidental overwrites of
/// unrelated files.
fn restore(args: &RestoreArgs) -> Result<()> {
    let backup_data = backup::backup_data_path(&args.backup)?;
    let expected = backup::stored_source_hash(&args.backup)?;
    let bytes =
        fs::read(&backup_data).with_context(|| format!("cannot read {}", backup_data.display()))?;
    let actual = hash(&bytes);
    anyhow::ensure!(
        actual == expected,
        "backup integrity failure in {}: marker records {expected} but DATA hashes to {actual}",
        args.backup.display()
    );
    let destination_bytes = fs::read(&args.destination)
        .with_context(|| format!("cannot read {}", args.destination.display()))?;
    let looks_like_save = args.destination.parent().is_some_and(|parent| {
        parent
            .file_name()
            .is_some_and(|name| name.to_string_lossy().contains("_Squads"))
    }) || save_format::validate_data(&destination_bytes).is_ok();
    anyhow::ensure!(
        looks_like_save,
        "refusing to overwrite {}: it is not inside a *_Squads folder and does not contain a valid squad DATA file",
        args.destination.display()
    );
    println!("Backup DATA SHA-256: {}", actual);
    println!("Target SHA-256:      {}", hash(&destination_bytes));
    println!("Restore size:        {} bytes", bytes.len());
    if args.dry_run {
        println!("Dry run: no files were changed.");
        return Ok(());
    }
    let temporary = args.destination.with_extension("part");
    fs::write(&temporary, &bytes)
        .with_context(|| format!("cannot write {}", temporary.display()))?;
    fs::rename(&temporary, &args.destination)
        .with_context(|| format!("cannot commit {}", args.destination.display()))?;
    println!(
        "Restored {} to {}",
        backup_data.display(),
        args.destination.display()
    );
    Ok(())
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
        let backup_path = backup::ensure_backup(
            &data_path,
            &user_data,
            &args.backup_dir,
            save_folder.as_deref(),
        )?;
        println!("Backup: {}", backup_path.display());
        Some(backup_path)
    } else {
        None
    };
    let source = match args.squad_file.as_deref() {
        Some(path) => source_detect::detect(path)?,
        None => match source_detect::detect(&args.download_dir) {
            Ok(source) => source,
            // No local source yet: fetch one. Every other failure, such as
            // an ambiguous directory, must reach the user unchanged.
            Err(SourceError::NotFound(_) | SourceError::Missing(_)) => {
                let path = download::download_latest(
                    &args.download_dir,
                    &args.content_url,
                    rf::roster::Platform::Ps4,
                    SquadKind::Major,
                )?;
                source_detect::detect(&path)?
            }
            Err(error) => return Err(error.into()),
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
    if let Ok(metadata) = fs::symlink_metadata(&output_container) {
        if metadata.is_symlink() {
            anyhow::bail!(
                "refusing to remove {}: it is a symlink, which could point outside the output directory",
                output_container.display()
            );
        }
        if metadata.is_dir() {
            fs::remove_dir_all(&output_container)
                .with_context(|| format!("cannot replace {}", output_container.display()))?;
        } else {
            fs::remove_file(&output_container)
                .with_context(|| format!("cannot replace {}", output_container.display()))?;
        }
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
