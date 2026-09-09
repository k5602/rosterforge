use crate::error::SaveFormatError;

pub const T3DB: [u8; 4] = [0x44, 0x42, 0x00, 0x08];
pub const TYPE_SQUADS: &[u8] = b"Type_Squads\0";
pub const GENERATED_HEADER_SIZE: usize = 1178;
pub const BNRY_BLOCK_SIZE: usize = 45_985;
pub const BNRY_MAGIC: &[u8] =
    b"BNRY\0\0\0\x02LTLE\x01\x01\x03\0\0\0cds\x01\0\0\0\0\x01\x03\0\0\0cds";
pub const MAX_GENERATED_SQUADS_SIZE: usize = 256 * 1024 * 1024;

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
    if data.len() > MAX_GENERATED_SQUADS_SIZE {
        return Err(SaveFormatError::InvalidGeneratedSquads);
    }
    if data[GENERATED_HEADER_SIZE..GENERATED_HEADER_SIZE + T3DB.len()] != T3DB {
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
        .checked_add(len as usize)
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
    let trimmed = result.trim_matches('_');
    if trimmed.is_empty() {
        "unknown".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_name_reads_exact_length() {
        let mut data = vec![0u8; 16 + 10];
        data[0..4].copy_from_slice(&5u32.to_le_bytes());
        data[16..21].copy_from_slice(b"hello");
        assert_eq!(save_name(&data).unwrap(), "hello");
    }

    #[test]
    fn save_name_single_char() {
        let mut data = vec![0u8; 16 + 1];
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[16] = b'X';
        assert_eq!(save_name(&data).unwrap(), "X");
    }

    #[test]
    fn save_name_max_length() {
        let name = "a".repeat(128);
        let mut data = vec![0u8; 16 + 128];
        data[0..4].copy_from_slice(&128u32.to_le_bytes());
        data[16..].copy_from_slice(name.as_bytes());
        assert_eq!(save_name(&data).unwrap(), name);
    }

    #[test]
    fn save_name_rejects_zero_length() {
        let mut data = vec![0u8; 100];
        data[0..4].copy_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            save_name(&data),
            Err(SaveFormatError::InvalidSaveNameLength(0))
        ));
    }

    #[test]
    fn save_name_rejects_too_long() {
        let mut data = vec![0u8; 16 + 200];
        data[0..4].copy_from_slice(&129u32.to_le_bytes());
        assert!(matches!(
            save_name(&data),
            Err(SaveFormatError::InvalidSaveNameLength(129))
        ));
    }

    #[test]
    fn save_name_rejects_truncated() {
        let mut data = vec![0u8; 16 + 3];
        data[0..4].copy_from_slice(&10u32.to_le_bytes());
        assert!(matches!(
            save_name(&data),
            Err(SaveFormatError::TooShort(_))
        ));
    }

    #[test]
    fn save_name_rejects_non_utf8() {
        let mut data = vec![0u8; 16 + 4];
        data[0..4].copy_from_slice(&4u32.to_le_bytes());
        data[16] = 0xff;
        data[17] = 0xfe;
        data[18] = 0xfd;
        data[19] = 0xfc;
        assert!(matches!(
            save_name(&data),
            Err(SaveFormatError::InvalidSaveName)
        ));
    }

    #[test]
    fn safe_folder_name_replaces_separators() {
        assert_eq!(safe_folder_name("hello world"), "hello_world");
        assert_eq!(safe_folder_name("foo__bar"), "foo_bar");
    }

    #[test]
    fn safe_folder_name_truncates_at_26() {
        assert_eq!(
            safe_folder_name("abcdefghijklmnopqrstuvwxyz0123456789"),
            "abcdefghijklmnopqrstuvwxyz"
        );
    }

    #[test]
    fn safe_folder_name_falls_back_to_unknown() {
        assert_eq!(safe_folder_name("!@#$%"), "unknown");
        assert_eq!(safe_folder_name(""), "unknown");
        assert_eq!(safe_folder_name("   "), "unknown");
    }

    #[test]
    fn safe_folder_name_keeps_safe_chars() {
        assert_eq!(safe_folder_name("save-file.2"), "save-file.2");
    }

    #[test]
    fn validate_data_rejects_short_data() {
        let data = vec![0u8; 500];
        assert!(matches!(
            validate_data(&data),
            Err(SaveFormatError::TooShort(_))
        ));
    }

    #[test]
    fn validate_data_rejects_missing_t3db() {
        let mut data = vec![0u8; 2000];
        data[..TYPE_SQUADS.len()].copy_from_slice(TYPE_SQUADS);
        assert!(matches!(
            validate_data(&data),
            Err(SaveFormatError::MissingT3db)
        ));
    }

    #[test]
    fn validate_data_rejects_t3db_before_type_squads() {
        let mut data = vec![0u8; 2000];
        data[1004..1008].copy_from_slice(&T3DB);
        data[1100..1100 + TYPE_SQUADS.len()].copy_from_slice(TYPE_SQUADS);
        assert!(matches!(
            validate_data(&data),
            Err(SaveFormatError::MissingTypeSquads)
        ));
    }

    #[test]
    fn validate_data_rejects_missing_bnry() {
        let mut data = vec![0u8; 2000 + BNRY_BLOCK_SIZE];
        data[100..100 + TYPE_SQUADS.len()].copy_from_slice(TYPE_SQUADS);
        data[1000..1004].copy_from_slice(&T3DB);
        assert!(matches!(
            validate_data(&data),
            Err(SaveFormatError::InvalidBnry)
        ));
    }

    #[test]
    fn find_t3db_ignores_marker_before_1000() {
        let mut data = vec![0u8; 2000];
        data[0..4].copy_from_slice(&T3DB);
        assert_eq!(find_t3db(&data), None);
        data[1004..1008].copy_from_slice(&T3DB);
        assert_eq!(find_t3db(&data), Some(1004));
    }

    #[test]
    fn generated_squads_requires_the_full_t3db_marker() {
        let mut data = vec![0u8; GENERATED_HEADER_SIZE + T3DB.len() + BNRY_BLOCK_SIZE];
        data[GENERATED_HEADER_SIZE..GENERATED_HEADER_SIZE + T3DB.len()].copy_from_slice(&T3DB);
        let bnry = data.len() - BNRY_BLOCK_SIZE;
        data[bnry..bnry + BNRY_MAGIC.len()].copy_from_slice(BNRY_MAGIC);
        assert!(database_from_generated_squads(&data).is_ok());
        data[GENERATED_HEADER_SIZE + T3DB.len() - 1] = 0x09;
        assert!(matches!(
            database_from_generated_squads(&data),
            Err(SaveFormatError::InvalidGeneratedSquads)
        ));
    }
}
