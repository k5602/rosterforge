use std::path::PathBuf;

use walkdir::WalkDir;

use crate::error::PlatformError;

/// Filesystem roots that can hold removable media with the Apollo layout.
pub fn mount_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(user) = std::env::var("USER") {
        roots.push(PathBuf::from("/media").join(&user));
        roots.push(PathBuf::from("/run/media").join(user));
    }
    roots.push(PathBuf::from("/mnt"));
    roots.retain(|path| path.is_dir());
    roots.sort();
    roots.dedup();
    roots
}

pub fn find_apollo_data() -> Result<PathBuf, PlatformError> {
    let mut candidates = Vec::new();
    let mut roots = vec![PathBuf::from("input")];
    roots.extend(mount_roots());
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        for entry in WalkDir::new(&root).follow_links(false).max_depth(6) {
            let entry = entry.map_err(|source| {
                let message = source.to_string();
                PlatformError::Scan {
                    root: root.clone(),
                    source: source
                        .into_io_error()
                        .unwrap_or_else(|| std::io::Error::other(message)),
                }
            })?;
            if !entry.file_type().is_file() || entry.file_name() != "DATA" {
                continue;
            }
            if entry.path().parent().is_some_and(|parent| {
                parent
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().contains("_Squads"))
            }) || entry.path().parent() == Some(root.as_path())
            {
                candidates.push(entry.into_path());
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    match candidates.len() {
        0 => Err(PlatformError::DataNotFound),
        1 => Ok(candidates.remove(0)),
        count => Err(PlatformError::AmbiguousData { count }),
    }
}

pub fn find_apollo_save() -> Result<PathBuf, PlatformError> {
    let mut candidates = Vec::new();
    let mut roots = mount_roots();
    roots.push(PathBuf::from("input"));
    for root in roots {
        let apollo = root.join("PS4").join("APOLLO");
        if !apollo.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&apollo).map_err(|source| PlatformError::Scan {
            root: apollo.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| PlatformError::Scan {
                root: apollo.clone(),
                source,
            })?;
            let path = entry.path();
            if path.is_dir()
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().contains("_Squads"))
                && path.join("DATA").is_file()
            {
                candidates.push(path);
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    match candidates.len() {
        0 => Err(PlatformError::SaveNotFound),
        1 => Ok(candidates.remove(0)),
        count => Err(PlatformError::AmbiguousSave { count }),
    }
}
