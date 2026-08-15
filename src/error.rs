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

