//! Raw unsigned-word to float conversion observed in the 0x59320 record decoder.

/// Convert the three measured component words using the caller-supplied scale
/// and offset (the observed XBE path uses global `0x478650` and `0x475C78`).
/// Verification: Tier 0 (focused smoke tests only; no differential or replay).
pub fn decode_three_u16_components(words: [u16; 3], scale: f32, offset: f32) -> [f32; 3] {
    words.map(|word| (word as f32) * scale - offset)
}

/// Convert four measured unsigned-word components using explicit constants.
/// Verification: Tier 0 (focused smoke tests only; no differential or replay).
pub fn decode_four_u16_components(words: [u16; 4], scale: f32, offset: f32) -> [f32; 4] {
    words.map(|word| (word as f32) * scale - offset)
}

/// Complete the observed optional fourth component from decoded XYZ.
///
/// The XBE computes `1 - (x*x + y*y + z*z)` and only takes the square root
/// when that remainder compares greater than the supplied threshold.
/// Verification: Tier 0 (focused smoke tests only; no differential or replay).
pub fn optional_fourth_component(xyz: [f32; 3], threshold: f32) -> f32 {
    let remainder = 1.0 - (xyz[0] * xyz[0] + xyz[1] * xyz[1] + xyz[2] * xyz[2]);
    if remainder > threshold {
        remainder.sqrt()
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decode_four_u16_components, decode_three_u16_components, optional_fourth_component,
    };

    #[test]
    fn applies_unsigned_conversion_scale_then_subtracts_offset() {
        let scale = f32::from_bits(0x3800_0080);
        let decoded = decode_three_u16_components([0, 32_768, 65_535], scale, 1.0);

        assert_eq!(decoded[0], -1.0);
        assert!(decoded[1] > decoded[0]);
        assert!(decoded[2] > decoded[1]);
        assert_eq!(decoded[1], 32_768.0_f32 * scale - 1.0);
        assert_eq!(decoded[2], 65_535.0_f32 * scale - 1.0);
    }

    #[test]
    fn optional_fourth_component_uses_strict_threshold_before_square_root() {
        let threshold = f32::from_bits(0x3727_c5ac);
        assert_eq!(optional_fourth_component([0.0, 0.0, 0.0], threshold), 1.0);
        assert_eq!(optional_fourth_component([1.0, 0.0, 0.0], threshold), 0.0);
        assert_eq!(
            optional_fourth_component([f32::NAN, 0.0, 0.0], threshold),
            0.0
        );
    }

    #[test]
    fn four_word_variant_converts_each_unsigned_component() {
        let scale = f32::from_bits(0x3800_0080);
        let decoded = decode_four_u16_components([0, 32_768, 65_535, 512], scale, 1.0);
        assert_eq!(decoded[0], -1.0);
        assert_eq!(decoded[1], 32_768.0_f32 * scale - 1.0);
        assert_eq!(decoded[2], 65_535.0_f32 * scale - 1.0);
        assert_eq!(decoded[3], 512.0_f32 * scale - 1.0);
    }
}
