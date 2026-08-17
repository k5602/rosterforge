use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::error::SourceError;
use crate::refpack::decompress;
use crate::save_format::database_from_generated_squads;

#[derive(Debug, Clone)]
pub enum SquadSource {
    Compressed(PathBuf),
    Generated(PathBuf),
}

pub fn detect(path: &Path) -> Result<SquadSource, SourceError> {
    if path.is_file() {
        return classify(path);
    }
    if !path.is_dir() {
        return Err(SourceError::Missing(path.to_owned()));
    }
    let mut candidates = Vec::new();
    for entry in WalkDir::new(path).follow_links(false).max_depth(6) {
        let entry = entry.map_err(|source| SourceError::Scan {
            path: path.to_owned(),
            source,
        })?;
        if entry.file_type().is_file()
            && !entry.file_name().to_string_lossy().starts_with("FutSquads")
            && let Ok(source) = classify(entry.path())
        {
            candidates.push(source);
        }
    }
    candidates.sort_by_key(|source| match source {
        SquadSource::Generated(path) => (0, path.clone()),
        SquadSource::Compressed(path) => (1, path.clone()),
    });
    candidates.dedup_by(|a, b| source_path(a) == source_path(b));
    match candidates.len() {
        0 => Err(SourceError::NotFound(path.to_owned())),
        1 => Ok(candidates.remove(0)),
        _ => Err(SourceError::Ambiguous {
            path: path.to_owned(),
        }),
    }
}

pub fn database(source: &SquadSource) -> Result<Vec<u8>, SourceError> {
    let path = source_path(source);
    let bytes = std::fs::read(path).map_err(|source| SourceError::Read {
        path: path.to_owned(),
        source,
    })?;
    match source {
        SquadSource::Generated(_) => database_from_generated_squads(&bytes)
            .map(|data| data.to_vec())
            .map_err(SourceError::from),
        SquadSource::Compressed(_) => decompress(&bytes).map_err(SourceError::from),
    }
}

fn classify(path: &Path) -> Result<SquadSource, SourceError> {
    let bytes = std::fs::read(path).map_err(|source| SourceError::Read {
        path: path.to_owned(),
        source,
    })?;
    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with("Squads"))
    {
        database_from_generated_squads(&bytes)?;
        return Ok(SquadSource::Generated(path.to_owned()));
    }
    decompress(&bytes)?;
    Ok(SquadSource::Compressed(path.to_owned()))
}

fn source_path(source: &SquadSource) -> &Path {
    match source {
        SquadSource::Compressed(path) | SquadSource::Generated(path) => path,
    }
}
