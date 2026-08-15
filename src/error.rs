use std::path::PathBuf;

use thiserror::Error;

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

