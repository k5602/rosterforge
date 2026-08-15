use crate::error::SaveFormatError;

pub const T3DB: [u8; 4] = [0x44, 0x42, 0x00, 0x08];
pub const TYPE_SQUADS: &[u8] = b"Type_Squads\0";
pub const GENERATED_HEADER_SIZE: usize = 1178;
pub const BNRY_BLOCK_SIZE: usize = 45_985;
pub const BNRY_MAGIC: &[u8] =
    b"BNRY\0\0\0\x02LTLE\x01\x01\x03\0\0\0cds\x01\0\0\0\0\x01\x03\0\0\0cds";

pub fn find_t3db(data: &[u8]) -> Option<usize> {
    data.get(1000..)?
        .windows(T3DB.len())
        .position(|w| w == T3DB)
        .map(|p| p + 1000)
}

pub fn validate_data(data: &[u8]) -> Result<usize, SaveFormatError> {
    if data.len() < 1000 + T3DB.len() {
        return Err(SaveFormatError::TooShort(data.len()));
    }
    let t3db = find_t3db(data).ok_or(SaveFormatError::MissingT3db)?;
    let sig = data[..t3db]
        .windows(TYPE_SQUADS.len())
        .position(|w| w == TYPE_SQUADS)
        .ok_or(SaveFormatError::MissingTypeSquads)?;
    if t3db < sig + TYPE_SQUADS.len() + 4 {
        return Err(SaveFormatError::IncompleteChecksum);
    }
    validate_bnry(data)?;
    Ok(t3db)
}

pub fn validate_bnry(data: &[u8]) -> Result<(), SaveFormatError> {
    let start = data
        .len()
        .checked_sub(BNRY_BLOCK_SIZE)
        .ok_or(SaveFormatError::InvalidBnry)?;
    if data[start..start + BNRY_MAGIC.len()] != *BNRY_MAGIC {
        return Err(SaveFormatError::InvalidBnry);
    }
    Ok(())
}

pub fn database_from_generated_squads(data: &[u8]) -> Result<&[u8], SaveFormatError> {
    if data.len() < GENERATED_HEADER_SIZE + T3DB.len() + BNRY_BLOCK_SIZE {
        return Err(SaveFormatError::InvalidGeneratedSquads);
    }
    if data[GENERATED_HEADER_SIZE..GENERATED_HEADER_SIZE + 2] != *b"DB" {
        return Err(SaveFormatError::InvalidGeneratedSquads);
    }
    validate_bnry(data)?;
    Ok(&data[GENERATED_HEADER_SIZE..data.len() - BNRY_BLOCK_SIZE])
}

pub fn save_name(data: &[u8]) -> Result<String, SaveFormatError> {
    let bytes = data.get(..4).ok_or(SaveFormatError::TooShort(data.len()))?;
    let len = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if !(1..=128).contains(&len) {
        return Err(SaveFormatError::InvalidSaveNameLength(len));
    }
    let end = 16usize
        .checked_add(len as usize - 1)
        .ok_or(SaveFormatError::InvalidSaveName)?;
    let raw = data
        .get(16..end)
        .ok_or(SaveFormatError::TooShort(data.len()))?;
    String::from_utf8(raw.to_vec()).map_err(|_| SaveFormatError::InvalidSaveName)
}

pub fn safe_folder_name(name: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.') {
            result.push(ch);
            separator = false;
        } else if ch.is_ascii_whitespace() || ch == '_' {
            separator = true;
        }
        if separator && !result.ends_with('_') && !result.is_empty() {
            result.push('_');
        }
        if result.len() >= 26 {
            break;
        }
    }
    result.trim_matches('_').to_owned().if_empty("unknown")
}

trait EmptyFallback {
    fn if_empty(self, fallback: &str) -> String;
}

impl EmptyFallback for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_owned()
        } else {
            self
        }
    }
}
