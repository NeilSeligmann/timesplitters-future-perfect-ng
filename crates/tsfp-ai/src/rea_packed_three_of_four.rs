//! Raw packed three-of-four component decoder observed at XBE address 0x59210.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackedWordInputError {
    SourceTooShort,
}

/// Read the four source bytes copied by 0x59210, then decode that packed word.
/// Its flag-selected prefix path has identical four-byte output for valid input.
/// Verification: Tier 0 (focused smoke tests only; no differential or replay).
pub fn decode_packed_three_of_four_bytes(
    source: &[u8],
    scale: f32,
    offset: f32,
) -> Result<[f32; 4], PackedWordInputError> {
    let bytes = source
        .get(..4)
        .ok_or(PackedWordInputError::SourceTooShort)?;
    let packed = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    Ok(decode_packed_three_of_four(packed, scale, offset))
}

/// Decode three packed 10-bit fields into the other three of four float slots.
///
/// The low two bits select the omitted output slot. The decoder computes the
/// missing component as `sqrt(1 - sum(x*x, y*y, z*z))`. Scale and offset match
/// globals `0x478658` and `0x478654` at the caller boundary.
/// Verification: Tier 0 (focused smoke tests only; no differential or replay).
pub fn decode_packed_three_of_four(packed: u32, scale: f32, offset: f32) -> [f32; 4] {
    let omitted = (packed & 0x3) as usize;
    let encoded = [
        ((packed >> 2) & 0x3ff) as f32,
        ((packed >> 12) & 0x3ff) as f32,
        ((packed >> 22) & 0x3ff) as f32,
    ];
    let components = encoded.map(|value| value * scale - offset);
    let slots = match omitted {
        0 => [1, 2, 3],
        1 => [0, 2, 3],
        2 => [0, 1, 3],
        _ => [0, 1, 2],
    };

    let mut result = [0.0; 4];
    for (component, slot) in components.iter().copied().zip(slots) {
        result[slot] = component;
    }
    let square_sum = components[0] * components[0]
        + components[1] * components[1]
        + components[2] * components[2];
    result[omitted] = (1.0 - square_sum).sqrt();
    result
}

#[cfg(test)]
mod tests {
    use super::{
        PackedWordInputError, decode_packed_three_of_four, decode_packed_three_of_four_bytes,
    };

    fn pack(omitted: u32, fields: [u32; 3]) -> u32 {
        omitted | (fields[0] << 2) | (fields[1] << 12) | (fields[2] << 22)
    }

    #[test]
    fn routes_three_10_bit_components_around_each_omitted_slot() {
        let scale = f32::from_bits(0x3ab5_f00b);
        let offset = f32::from_bits(0x3f35_c28f);
        let decoded = [
            decode_packed_three_of_four(pack(0, [512, 513, 514]), scale, offset),
            decode_packed_three_of_four(pack(1, [512, 513, 514]), scale, offset),
            decode_packed_three_of_four(pack(2, [512, 513, 514]), scale, offset),
            decode_packed_three_of_four(pack(3, [512, 513, 514]), scale, offset),
        ];
        let values = [512.0_f32, 513.0, 514.0].map(|word| word * scale - offset);

        assert_eq!([decoded[0][1], decoded[0][2], decoded[0][3]], values);
        assert_eq!([decoded[1][0], decoded[1][2], decoded[1][3]], values);
        assert_eq!([decoded[2][0], decoded[2][1], decoded[2][3]], values);
        assert_eq!([decoded[3][0], decoded[3][1], decoded[3][2]], values);
        assert!(
            decoded
                .iter()
                .all(|value| value.iter().all(|x| x.is_finite()))
        );
    }

    #[test]
    fn reads_little_endian_word_and_rejects_short_source() {
        let scale = f32::from_bits(0x3ab5_f00b);
        let offset = f32::from_bits(0x3f35_c28f);
        let packed = pack(2, [512, 513, 514]);
        assert_eq!(
            decode_packed_three_of_four_bytes(&packed.to_le_bytes(), scale, offset),
            Ok(decode_packed_three_of_four(packed, scale, offset))
        );
        assert_eq!(
            decode_packed_three_of_four_bytes(&[1, 2, 3], scale, offset),
            Err(PackedWordInputError::SourceTooShort)
        );
    }
}
