use crate::error::PatchError;
use crate::save_format::{
    BNRY_BLOCK_SIZE, BNRY_MAGIC, T3DB, TYPE_SQUADS, find_t3db, validate_data,
};

pub fn patch_data(user_data: &[u8], database: &[u8]) -> Result<Vec<u8>, PatchError> {
    if database.get(..T3DB.len()) != Some(&T3DB) {
        return Err(PatchError::MissingDatabaseMarker);
    }
    let t3db = validate_data(user_data)?;
    let mut header = user_data[..t3db].to_vec();
    let signature = header
        .windows(TYPE_SQUADS.len())
        .position(|w| w == TYPE_SQUADS)
        .ok_or(crate::error::SaveFormatError::MissingTypeSquads)?;
    let checksum = signature + TYPE_SQUADS.len();
    if checksum + 4 > header.len() {
        return Err(crate::error::SaveFormatError::IncompleteChecksum.into());
    }
    header[checksum..checksum + 4].fill(0);
    let mut patched = Vec::with_capacity(header.len() + database.len() + BNRY_BLOCK_SIZE);
    patched.extend_from_slice(&header);
    patched.extend_from_slice(database);
    patched.extend_from_slice(BNRY_MAGIC);
    patched.resize(patched.len() + BNRY_BLOCK_SIZE - BNRY_MAGIC.len(), 0);
    if find_t3db(&patched) != Some(t3db) {
        return Err(PatchError::MissingDatabaseMarker);
    }
    Ok(patched)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> Vec<u8> {
        let mut data = vec![0; 1100];
        data[0..4].copy_from_slice(&5u32.to_le_bytes());
        data[16..20].copy_from_slice(b"Test");
        data[20] = 0;
        data.extend_from_slice(TYPE_SQUADS);
        data.extend_from_slice(&[1, 2, 3, 4]);
        data.extend_from_slice(&T3DB);
        data.extend_from_slice(b"old");
        data.extend_from_slice(BNRY_MAGIC);
        data.resize(data.len() + BNRY_BLOCK_SIZE - BNRY_MAGIC.len(), 0);
        data
    }

    #[test]
    fn patch_preserves_header_and_clears_checksum() {
        let original = sample_data();
        let database = [T3DB.as_slice(), b"new"].concat();
        let result = patch_data(&original, &database).expect("valid sample");
        assert_eq!(&result[..1100], &original[..1100]);
        assert_eq!(
            &result[1100 + TYPE_SQUADS.len()..1100 + TYPE_SQUADS.len() + 4],
            &[0; 4]
        );
        assert_eq!(
            &result[1100 + TYPE_SQUADS.len() + 4..1100 + TYPE_SQUADS.len() + 4 + database.len()],
            &database
        );
        assert_eq!(
            &result
                [result.len() - BNRY_BLOCK_SIZE..result.len() - BNRY_BLOCK_SIZE + BNRY_MAGIC.len()],
            BNRY_MAGIC
        );
    }
}
