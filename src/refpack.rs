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
            Err(RefpackError::IncompleteOutput {
                expected: 64,
                actual: 4
            })
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
    fn decodes_stream_using_every_command_form() {
        // Hand-built stream exercising: literal run, large pointer
        // (length 96 via offset-1 overlap), small pointer with two folded
        // literals, medium pointer, and a stop code with trailing literals.
        let mut compressed = vec![0; 10];
        compressed[2..5].copy_from_slice(&115u32.to_be_bytes()[1..]);
        compressed.extend_from_slice(&[
            0xe0, b'Q', b'Q', b'Q', b'Q', // 4 literals
            0xc0, 0x00, 0x00, 0x5b, // large pointer: 96 bytes, offset 1
            0x02, 0x04, b'X', b'Y', // small pointer: 3 bytes, offset 5, 2 literals
            0x80, 0x00, 0x03, // medium pointer: 4 bytes, offset 4
            0xfe, b'O', b'K', // stop code with 2 trailing literals
        ]);
        let mut expected = T3DB.to_vec();
        expected.extend_from_slice(b"QQQQ");
        expected.extend_from_slice(&[b'Q'; 96]);
        expected.extend_from_slice(b"XYQQQYQQQOK");
        assert_eq!(decompress(&compressed).expect("valid RefPack"), expected);
    }
}
