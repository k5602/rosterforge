use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::error::BackupError;
use crate::save_format::{safe_folder_name, save_name};

pub fn container_name(data: &[u8]) -> String {
    let name =
        save_name(data).map_or_else(|_| "unknown".to_owned(), |value| safe_folder_name(&value));
    format!("manual_{name}")
}

pub fn ensure_backup(
    data_path: &Path,
    data: &[u8],
    root: &Path,
    save_folder: Option<&Path>,
) -> Result<PathBuf, BackupError> {
    let container = root.join(container_name(data));
    let destination = if let Some(folder) = save_folder {
        container
            .join(folder.file_name().ok_or(BackupError::NoFolderName)?)
            .join("DATA")
    } else {
        container.join("DATA")
    };
    if destination.exists() {
        let existing = fs::read(&destination).map_err(|source| BackupError::ReadExisting {
            path: destination.clone(),
            source,
        })?;
        if existing != data {
            return Err(BackupError::Mismatch {
                data_path: data_path.to_owned(),
            });
        }
        return Ok(container);
    }
    fs::create_dir_all(destination.parent().ok_or(BackupError::NoFolderName)?).map_err(
        |source| BackupError::CreateDir {
            path: container.clone(),
            source,
        },
    )?;
    if let Some(source_folder) = save_folder {
        copy_directory(
            source_folder,
            destination.parent().ok_or(BackupError::NoFolderName)?,
        )?;
    } else {
        let temporary = container.join("DATA.part");
        fs::write(&temporary, data).map_err(|source| BackupError::Write {
            path: temporary.clone(),
            source,
        })?;
        fs::rename(&temporary, &destination).map_err(|source| BackupError::Commit {
            destination: destination.clone(),
            source,
        })?;
    }
    fs::write(container.join(".source_sha256"), hash(data)).map_err(|source| {
        BackupError::Write {
            path: container.join(".source_sha256"),
            source,
        }
    })?;
    Ok(container)
}

/// Locate the DATA file inside a backup container.
///
/// Containers hold either `DATA` at the root or one save folder that
/// contains it.
pub fn backup_data_path(container: &Path) -> Result<PathBuf, BackupError> {
    if container.join("DATA").is_file() {
        return Ok(container.join("DATA"));
    }
    for entry in fs::read_dir(container).map_err(|source| BackupError::ScanContainer {
        path: container.to_owned(),
        source,
    })? {
        let path = entry
            .map_err(|source| BackupError::ScanContainer {
                path: container.to_owned(),
                source,
            })?
            .path();
        if path.is_dir() && path.join("DATA").is_file() {
            return Ok(path.join("DATA"));
        }
    }
    Err(BackupError::MissingData(container.to_owned()))
}

pub fn mirror_save_folder(
    source: &Path,
    destination: &Path,
    patched_data: &[u8],
) -> Result<(), BackupError> {
    copy_directory(source, destination)?;
    let data_path = destination.join("DATA");
    let temporary = destination.join("DATA.part");
    fs::write(&temporary, patched_data).map_err(|source| BackupError::Write {
        path: temporary.clone(),
        source,
    })?;
    fs::rename(&temporary, &data_path).map_err(|source| BackupError::Commit {
        destination: data_path.clone(),
        source,
    })?;
    Ok(())
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), BackupError> {
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(BackupError::InvalidPath)?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|_| BackupError::InvalidSavePath(entry.path().to_owned()))?;
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).map_err(|io_error| BackupError::Copy {
                source_path: target.clone(),
                source: io_error,
            })?;
        } else {
            fs::copy(entry.path(), &target).map_err(|io_error| BackupError::Copy {
                source_path: target.clone(),
                source: io_error,
            })?;
        }
    }
    Ok(())
}

pub fn save_database_hash(container: &Path, database: &[u8]) -> Result<(), BackupError> {
    fs::write(container.join(".last_db_hash"), hash(database)).map_err(|source| {
        BackupError::Write {
            path: container.join(".last_db_hash"),
            source,
        }
    })
}

fn hash(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
