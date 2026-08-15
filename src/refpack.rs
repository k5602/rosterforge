use crate::error::RefpackError;
use crate::save_format::T3DB;

const MAX_OUTPUT_SIZE: usize = 64 * 1024 * 1024;

pub fn decompress(data: &[u8]) -> Result<Vec<u8>, RefpackError> {
    if data.len() < 10 {
        return Err(RefpackError::InputTooShort { actual: data.len() });
    }
    let size = (usize::from(data[2]) << 16) | (usize::from(data[3]) << 8) | usize::from(data[4]);
    if !(T3DB.len()..=MAX_OUTPUT_SIZE).contains(&size) {
        return Err(RefpackError::InvalidOutputSize(size));
    }
    let mut output = vec![0; size];
    output[..T3DB.len()].copy_from_slice(&T3DB);
    let mut input = 10;
    let mut output_pos = T3DB.len();
    let mut last_control = 0;
    while input < data.len() && output_pos < size {
        let control = data[input];
        input += 1;
        last_control = control;
        if control & 0x80 == 0 {
            let b1 = *data
                .get(input)
                .ok_or(RefpackError::Truncated { offset: input })?;
            input += 1;
            let literals = usize::from(control & 3);
            copy_literals(data, &mut input, &mut output, &mut output_pos, literals)?;
            let length = usize::from((control >> 2) & 7) + 3;
            let offset = usize::from(b1) + (usize::from(control & 0x60) << 3) + 1;
            copy_backref(&mut output, &mut output_pos, length, offset)?;
        } else if control & 0x40 == 0 {
            let b2 = *data
                .get(input)
                .ok_or(RefpackError::Truncated { offset: input })?;
            let b3 = *data
                .get(input + 1)
                .ok_or(RefpackError::Truncated { offset: input + 1 })?;
            input += 2;
            let literals = usize::from(b2 >> 6);
            copy_literals(data, &mut input, &mut output, &mut output_pos, literals)?;
            copy_backref(
                &mut output,
                &mut output_pos,
                usize::from(control & 0x3f) + 4,
                (((usize::from(b2 & 0x3f)) << 8) | usize::from(b3)) + 1,
            )?;
        } else if control & 0x20 == 0 {
            let b2 = *data
                .get(input)
                .ok_or(RefpackError::Truncated { offset: input })?;
            let b3 = *data
                .get(input + 1)
                .ok_or(RefpackError::Truncated { offset: input + 1 })?;
            let b4 = *data
                .get(input + 2)
                .ok_or(RefpackError::Truncated { offset: input + 2 })?;
            input += 3;
            let literals = usize::from(control & 3);
            copy_literals(data, &mut input, &mut output, &mut output_pos, literals)?;
            copy_backref(
                &mut output,
                &mut output_pos,
                usize::from(b4) + (usize::from(control & 0x0c) << 6) + 5,
                (((usize::from(control & 0x10)) << 12) | (usize::from(b2) << 8) | usize::from(b3))
                    + 1,
            )?;
        } else {
            let literals = usize::from(control & 0x1f) * 4 + 4;
            if literals > 0x70 {
                break;
            }
            copy_literals(data, &mut input, &mut output, &mut output_pos, literals)?;
        }
    }
    let trailing = usize::from(last_control & 3);
    if trailing > 0 && output_pos < size {
        copy_literals(data, &mut input, &mut output, &mut output_pos, trailing)?;
    }
    if output_pos != size {
        return Err(RefpackError::IncompleteOutput {
            expected: size,
            actual: output_pos,
        });
    }
    Ok(output)
}

fn copy_literals(
    data: &[u8],
    input: &mut usize,
    output: &mut [u8],
    output_pos: &mut usize,
    count: usize,
) -> Result<(), RefpackError> {
    let end_input = input
        .checked_add(count)
        .ok_or(RefpackError::Truncated { offset: *input })?;
    let end_output = output_pos
        .checked_add(count)
        .ok_or(RefpackError::LiteralOutputOverflow {
            offset: *output_pos,
        })?;
    if end_input > data.len() {
        return Err(RefpackError::Truncated { offset: *input });
    }
    if end_output > output.len() {
        return Err(RefpackError::LiteralOutputOverflow {
            offset: *output_pos,
        });
    }
    output[*output_pos..end_output].copy_from_slice(&data[*input..end_input]);
    *input = end_input;
    *output_pos = end_output;
    Ok(())
}

fn copy_backref(
    output: &mut [u8],
    output_pos: &mut usize,
    count: usize,
    offset: usize,
) -> Result<(), RefpackError> {
    if offset == 0 || offset > *output_pos {
        return Err(RefpackError::InvalidBackReference {
            offset: *output_pos,
        });
    }
    let end = output_pos
        .checked_add(count)
        .ok_or(RefpackError::CopyOutputOverflow {
            offset: *output_pos,
        })?;
    if end > output.len() {
        return Err(RefpackError::CopyOutputOverflow {
            offset: *output_pos,
        });
    }
    let source_start = *output_pos - offset;
    for (source, position) in (source_start..).zip(*output_pos..end) {
        output[position] = output[source];
    }
    *output_pos = end;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decompresses_literal_run() {
        let mut compressed = vec![0; 10];
        compressed[2..5].copy_from_slice(&8u32.to_be_bytes()[1..]);
        compressed.extend_from_slice(&[0xe0, b'a', b'b', b'c', b'd']);
        assert_eq!(
            decompress(&compressed).expect("valid RefPack"),
            b"DB\0\x08abcd"
        );
    }

    #[test]
    fn rejects_truncated_literal_run() {
        let mut compressed = vec![0; 10];
        compressed[2..5].copy_from_slice(&8u32.to_be_bytes()[1..]);
        compressed.extend_from_slice(&[0xe0, b'a']);
        assert!(matches!(
            decompress(&compressed),
            Err(RefpackError::Truncated { .. })
        ));
    }

    #[test]
    fn rejects_invalid_back_reference() {
        // Stream declares 8 output bytes. Only the 4-byte marker exists
        // when the first pointer runs, so offset 5 points past its start.
        let mut compressed = vec![0; 10];
        compressed[2..5].copy_from_slice(&8u32.to_be_bytes()[1..]);
        compressed.extend_from_slice(&[0x04, 0x04]);
        assert!(matches!(
            decompress(&compressed),
            Err(RefpackError::InvalidBackReference { .. })
        ));
    }

    #[test]
    fn rejects_incomplete_output() {
        let mut compressed = vec![0; 10];
        compressed[2..5].copy_from_slice(&64u32.to_be_bytes()[1..]);
        compressed.push(0xfc);
        assert!(matches!(
            decompress(&compressed),
            Err(RefpackError::IncompleteOutput { expected: 64, actual: 4 })
        ));
    }

    #[test]
    fn rejects_undersized_and_unfillable_declarations() {
        assert!(matches!(
            decompress(&[0; 10]),
            Err(RefpackError::InvalidOutputSize(0))
        ));
        // A 24-bit size never exceeds the 64 MiB cap, so the largest
        // declaration fails as incomplete instead of invalid.
        let mut compressed = vec![0; 10];
        compressed[2..5].copy_from_slice(&[0xff, 0xff, 0xff]);
        assert!(matches!(
            decompress(&compressed),
            Err(RefpackError::IncompleteOutput {
                expected: 0xff_ffff,
                actual: 4
            })
        ));
    }

    #[test]
    fn round_trips_varied_payloads_through_test_encoder() {
        for (name, plain) in test_payloads() {
            let encoded = encode(&plain);
            let decoded =
                decompress(&encoded).expect("encoder output must be valid");
            let mut expected = T3DB.to_vec();
            expected.extend_from_slice(&plain);
            assert_eq!(decoded, expected, "payload: {name}");
        }
    }

    #[test]
    fn round_trips_overlapping_runs() {
        // A single repeated byte forces offset-1 overlapping copies across
        // every pointer size.
        let plain = vec![0xaau8; 3000];
        let encoded = encode(&plain);
        let mut expected = T3DB.to_vec();
        expected.extend_from_slice(&plain);
        assert_eq!(decompress(&encoded).expect("valid RefPack"), expected);
    }

    /// Deterministic payloads that hit every command path: literal runs,
    /// small, medium, and large pointers, and their boundary lengths.
    fn test_payloads() -> Vec<(String, Vec<u8>)> {
        let mut state = 0x1234_5678_9abc_def0u64;
        let mut random = move || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) as u8
        };
        let mut payloads: Vec<(String, Vec<u8>)> = Vec::new();
        for size in [0usize, 1, 3, 4, 5, 63, 64, 65, 112, 113, 127, 1000, 4096] {
            let data: Vec<u8> = (0..size).map(|_| random()).collect();
            payloads.push((format!("random-{size}"), data));
        }
        // Periodic patterns force long-distance matches.
        for period in [3usize, 17, 256] {
            let data: Vec<u8> = (0..4096).map(|index| (index % period) as u8).collect();
            payloads.push((format!("periodic-{period}"), data));
        }
        payloads
    }

    /// Minimal greedy RefPack encoder used only by tests.
    ///
    /// It emits every command form the decoder understands: literal runs,
    /// small, medium, and large pointers with folded literal prefixes, and
    /// the stop code with trailing literals.
    const WINDOW: usize = 16 * 1024;
    const MEDIUM_MAX: usize = 67;
    const LARGE_MAX: usize = 1028;
    const SMALL_MAX_OFFSET: usize = 1024;
    const SMALL_MAX: usize = 10;

    fn encode(plain: &[u8]) -> Vec<u8> {
        let size = T3DB.len() + plain.len();
        let mut out = vec![0; 10];
        out[2..5].copy_from_slice(
            &u32::try_from(size)
                .expect("test payload fits 24 bits")
                .to_be_bytes()[1..],
        );

        let mut pending: Vec<u8> = Vec::new();
        let mut heads: std::collections::HashMap<[u8; 3], Vec<usize>> =
            std::collections::HashMap::new();

        let mut position = 0;
        while position < plain.len() {
            let mut best_len = 0;
            let mut best_offset = 0;
            if position + 3 <= plain.len() {
                let key = [plain[position], plain[position + 1], plain[position + 2]];
                if let Some(positions) = heads.get(&key) {
                    for &start in positions.iter().rev().take(48) {
                        let offset = position - start;
                        if offset == 0 || offset > WINDOW {
                            continue;
                        }
                        let mut length = 0;
                        while position + length < plain.len()
                            && plain[start + length] == plain[position + length]
                            && length < LARGE_MAX
                        {
                            length += 1;
                        }
                        if length > best_len {
                            best_len = length;
                            best_offset = offset;
                            if length == LARGE_MAX {
                                break;
                            }
                        }
                    }
                }
                heads.entry(key).or_default().push(position);
            }
            match select_command(best_len, best_offset) {
                Some((kind, length)) => {
                    flush_literal_runs(&mut out, &mut pending);
                    let literals = pending.len();
                    emit_pointer(&mut out, kind, length, best_offset, literals);
                    out.extend_from_slice(&pending);
                    pending.clear();
                    // Advance past the matched span; intermediate positions
                    // still join the match index.
                    for step in 1..length {
                        let index = position + step;
                        if index + 3 <= plain.len() {
                            let key = [
                                plain[index],
                                plain[index + 1],
                                plain[index + 2],
                            ];
                            heads.entry(key).or_default().push(index);
                        }
                    }
                    position += length;
                }
                None => {
                    pending.push(plain[position]);
                    position += 1;
                }
            }
        }
        flush_literal_runs(&mut out, &mut pending);
        out.push(0xfc | pending.len() as u8);
        out.extend_from_slice(&pending);
        out
    }

    #[derive(Clone, Copy)]
    enum PointerKind {
        Small,
        Medium,
        Large,
    }

    fn select_command(length: usize, offset: usize) -> Option<(PointerKind, usize)> {
        match () {
            // Long matches use the large pointer regardless of distance.
            _ if length > MEDIUM_MAX && offset <= WINDOW => {
                Some((PointerKind::Large, length.min(LARGE_MAX)))
            }
            _ if length >= 4 && offset <= WINDOW => {
                Some((PointerKind::Medium, length.min(MEDIUM_MAX)))
            }
            _ if length >= 3 && offset <= SMALL_MAX_OFFSET => {
                Some((PointerKind::Small, length.min(SMALL_MAX)))
            }
            // Far matches of five or more bytes still beat literals.
            _ if length >= 5 && offset <= WINDOW => {
                Some((PointerKind::Large, length.min(LARGE_MAX)))
            }
            _ => None,
        }
    }

    /// Drain pending literals in chunks of at most 112 bytes, keeping the
    /// final fewer-than-four bytes for pointer folding or the stop code.
    fn flush_literal_runs(out: &mut Vec<u8>, pending: &mut Vec<u8>) {
        while pending.len() > 3 {
            let chunk = 112.min(pending.len() - pending.len() % 4);
            if chunk == 0 {
                return;
            }
            out.push(0xe0 | (chunk / 4 - 1) as u8);
            out.extend_from_slice(&pending[..chunk]);
            pending.drain(..chunk);
        }
    }

    fn emit_pointer(
        out: &mut Vec<u8>,
        kind: PointerKind,
        length: usize,
        offset: usize,
        literals: usize,
    ) {
        let raw_offset = offset - 1;
        match kind {
            PointerKind::Small => {
                let control =
                    (((raw_offset >> 8) as u8) << 5) | (((length - 3) as u8) << 2) | literals as u8;
                out.push(control);
                out.push(raw_offset as u8);
            }
            PointerKind::Medium => {
                out.push(0x80 | (length - 4) as u8);
                out.push(((literals as u8) << 6) | ((raw_offset >> 8) as u8));
                out.push(raw_offset as u8);
            }
            PointerKind::Large => {
                let raw_length = length - 5;
                let control = 0xc0
                    | (((raw_length >> 8) as u8) << 2)
                    | ((((raw_offset >> 16) as u8) & 1) << 4)
                    | literals as u8;
                out.push(control);
                out.push((raw_offset >> 8) as u8);
                out.push(raw_offset as u8);
                out.push(raw_length as u8);
            }
        }
    }
}
