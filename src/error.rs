use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RefpackError {
    #[error("RefPack input is too short: {actual} bytes")]
    InputTooShort { actual: usize },
    #[error("RefPack output size is invalid: {0} bytes")]
    InvalidOutputSize(usize),
    #[error("RefPack stream is truncated at input offset {offset}")]
    Truncated { offset: usize },
    #[error("RefPack literal run exceeds output at output offset {offset}")]
    LiteralOutputOverflow { offset: usize },
    #[error("RefPack back-reference is invalid at output offset {offset}")]
    InvalidBackReference { offset: usize },
    #[error("RefPack copy exceeds output at output offset {offset}")]
    CopyOutputOverflow { offset: usize },
    #[error("RefPack stream ended before producing {expected} bytes; produced {actual}")]
    IncompleteOutput { expected: usize, actual: usize },
    #[error("decompressed data does not start with the T3DB marker")]
    MissingT3db,
}

#[derive(Debug, Error)]
pub enum SaveFormatError {
    #[error("DATA file is too short: {0} bytes")]
    TooShort(usize),
    #[error("T3DB marker was not found at or after offset 1000")]
    MissingT3db,
    #[error("Type_Squads signature was not found before T3DB")]
    MissingTypeSquads,
    #[error("checksum field after Type_Squads is incomplete")]
    IncompleteChecksum,
    #[error("BNRY block is missing or invalid")]
    InvalidBnry,
    #[error("save name length is invalid: {0}")]
    InvalidSaveNameLength(u32),
    #[error("save name is not valid UTF-8")]
    InvalidSaveName,
    #[error("generated squad file is shorter than its 1178-byte header")]
    InvalidGeneratedSquads,
}

#[derive(Debug, Error)]
pub enum PatchError {
    #[error("EA database does not start with the T3DB marker")]
    MissingDatabaseMarker,
    #[error("{0}")]
    Save(#[from] SaveFormatError),
}

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("Apollo save folder has no name")]
    NoFolderName,
    #[error("cannot read existing backup {path}: {source}")]
    ReadExisting {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("backup exists but differs from {data_path}; use a new backup directory")]
    Mismatch { data_path: PathBuf },
    #[error("cannot create backup directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot commit {destination}: {source}")]
    Commit {
        destination: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot copy {source_path}: {source}")]
    Copy {
        source_path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid save path: {0}")]
    InvalidPath(walkdir::Error),
    #[error("entry {0} is outside the save folder")]
    InvalidSavePath(PathBuf),
    #[error("DATA is missing from backup {0}")]
    MissingData(PathBuf),
    #[error("cannot scan backup container {path}: {source}")]
    ScanContainer {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("backup metadata {path} is invalid: {reason}")]
    Metadata { path: PathBuf, reason: &'static str },
}

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("cannot scan {root}: {source}")]
    Scan {
        root: PathBuf,
        source: std::io::Error,
    },
    #[error("no DATA file found in input/ or mounted game roots")]
    DataNotFound,
    #[error("multiple DATA files found; use --data: {count} candidates")]
    AmbiguousData { count: usize },
    #[error("no Apollo Squads save found under PS4/APOLLO")]
    SaveNotFound,
    #[error("multiple Apollo saves found; use --data: {count} candidates")]
    AmbiguousSave { count: usize },
}
#[derive(Debug, Error)]
pub enum SourceError {
    #[error("squad source does not exist: {0}")]
    Missing(PathBuf),
    #[error("cannot scan {path}: {source}")]
    Scan {
        path: PathBuf,
        source: walkdir::Error,
    },
    #[error("no valid PS4 squad source found under {0}")]
    NotFound(PathBuf),
    #[error("multiple squad sources found under {path}; use --squad-file")]
    Ambiguous { path: PathBuf },
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{0}")]
    Save(#[from] SaveFormatError),
    #[error("{0}")]
    Refpack(#[from] RefpackError),
}

#[derive(Debug, Error)]
pub enum RosterError {
    #[error("roster XML parse failed: {0}")]
    Xml(String),
    #[error("roster manifest has no entry for platform {platform}")]
    MissingPlatform { platform: &'static str },
    #[error("{platform} roster entry is missing {field}")]
    MissingField {
        platform: &'static str,
        field: &'static str,
    },
}

