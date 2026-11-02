//! Lossless XBT texture envelope. Pixel layout and swizzling remain opaque.

use tsfp_format_core::*;

/// The first nine header words, the remaining 92 header bytes, and opaque pixels.
///
/// Words 7 and 8 retain the exact bits of the original floating-point fields.
/// The runtime texture index at offset `0x24` lives in `remainder`; this codec
/// preserves it without assigning a file-format meaning to it.
///
/// ```
/// use tsfp_format_xbt::Texture;
/// let texture = Texture {
///     words: [4, 4, 4, 4, 0, 0, 0, 0x7fc0_0001, 0x8000_0000],
///     remainder: vec![0; 92],
///     pixels: vec![1, 2, 3],
/// };
/// let bytes = texture.encode().unwrap();
/// assert_eq!(Texture::parse(&bytes).unwrap().encode().unwrap(), bytes);
/// ```
#[derive(Debug, Clone)]
pub struct Texture {
    pub words: [u32; 9],
    pub remainder: Vec<u8>,
    pub pixels: Vec<u8>,
}

impl Texture {
    fn validate_words(words: &[u32; 9]) -> Result<()> {
        if words[..4]
            .iter()
            .any(|word| *word == 0 || *word > i32::MAX as u32)
            || words[5] > 7
        {
            return Err("invalid dimensions/format");
        }
        Ok(())
    }

    /// Parse a 128-byte header followed by opaque pixel bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        range(bytes, 0, 128)?;
        let words: [u32; 9] = words(bytes, 0, 9)?.try_into().unwrap();
        Self::validate_words(&words)?;
        Ok(Self {
            words,
            remainder: bytes[36..128].to_vec(),
            pixels: bytes[128..].to_vec(),
        })
    }

    /// Serialize while checking the fixed header shape and parsed field bounds.
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.remainder.len() != 92 {
            return Err("header remainder size");
        }
        Self::validate_words(&self.words)?;
        let mut bytes = Vec::with_capacity(128 + self.pixels.len());
        for word in self.words {
            bytes.extend(word.to_le_bytes());
        }
        bytes.extend(&self.remainder);
        bytes.extend(&self.pixels);
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::Texture;

    fn sample() -> Texture {
        Texture {
            words: [4, 4, 4, 4, 0, 0, 0, 0x7fc1_2345, 0x8000_0000],
            remainder: vec![0; 92],
            pixels: vec![0; 8],
        }
    }

    #[test]
    fn retained_regions_and_float_bits_round_trip() {
        for seed in 0..256u32 {
            let mut texture = sample();
            texture.words[7] = seed.wrapping_mul(0x9e37_79b9);
            texture.words[8] = !texture.words[7];
            for (index, byte) in texture.remainder.iter_mut().enumerate() {
                *byte = (seed as u8).wrapping_add(index as u8);
            }
            texture.pixels = (0..seed % 33).map(|index| index as u8).collect();
            let encoded = texture.encode().unwrap();
            assert_eq!(Texture::parse(&encoded).unwrap().encode().unwrap(), encoded);
            assert_eq!(&encoded[36..128], texture.remainder);
            assert_eq!(&encoded[128..], texture.pixels);
        }
    }

    #[test]
    fn rejects_short_headers_and_invalid_fields() {
        let encoded = sample().encode().unwrap();
        for length in 0..128 {
            assert!(Texture::parse(&encoded[..length]).is_err());
        }
        for field in 0..4 {
            for invalid in [0, 0x8000_0000] {
                let mut texture = sample();
                texture.words[field] = invalid;
                assert!(texture.encode().is_err());
            }
        }
        let mut texture = sample();
        texture.words[5] = 8;
        assert!(texture.encode().is_err());
        texture = sample();
        texture.remainder.pop();
        assert!(texture.encode().is_err());
    }

    #[test]
    fn encode_checks_header_before_copying_payload() {
        let mut texture = sample();
        texture.words[0] = 0;
        texture.pixels = vec![0x5a; 4096];
        assert_eq!(texture.encode(), Err("invalid dimensions/format"));
        texture.remainder.pop();
        assert_eq!(texture.encode(), Err("header remainder size"));
    }
}
