//! Read-only validation for the measured effects image used by the host DSP adapter.
//!
//! The pinned FNV-1a and CRC32 values detect accidental image changes; they are not
//! cryptographic authentication. Guest-memory reads and mapping protection remain at
//! the host boundary. This module only validates an already captured immutable span.

use crate::effects_metadata::{self, Metadata};

pub const IMAGE_BYTES: usize = 18_608;
const EXPECTED_FNV1A64: u64 = 0x9BBE_1429_7F9A_F82B;
const EXPECTED_CRC32: u32 = 0xCAD2_2F6F;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fingerprints {
    pub fnv1a64: u64,
    pub crc32: u32,
}

/// Hash a captured image using the checksums used by the existing host guard.
pub fn fingerprints(image: &[u8]) -> Fingerprints {
    let mut fnv = 0xCBF2_9CE4_8422_2325u64;
    let mut crc = u32::MAX;
    for &byte in image {
        fnv = (fnv ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3);
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ ((0u32.wrapping_sub(crc & 1)) & 0xEDB8_8320);
        }
    }
    Fingerprints {
        fnv1a64: fnv,
        crc32: !crc,
    }
}

/// Validate the pinned image identity, measured header, and exact layout contract.
pub fn validate_known_image(image: &[u8]) -> Option<Metadata> {
    validate_image_with_fingerprints(
        image,
        Fingerprints {
            fnv1a64: EXPECTED_FNV1A64,
            crc32: EXPECTED_CRC32,
        },
    )
}

/// Validate an image against supplied checksums. This supports synthetic differential tests;
/// production callers should use [`validate_known_image`] and its pinned values.
#[doc(hidden)]
pub fn validate_image_with_fingerprints(image: &[u8], expected: Fingerprints) -> Option<Metadata> {
    if image.len() != IMAGE_BYTES || fingerprints(image) != expected {
        return None;
    }

    for (offset, expected_word) in [
        (0x800, 0),
        (0x804, 2020),
        (0x808, 10152),
        (0x80C, 2022),
        (0x810, 3),
        (0x814, 0),
    ] {
        if read_word(image, offset)? != expected_word {
            return None;
        }
    }

    let metadata = effects_metadata::validate_layout(image)?;
    (metadata.map_count == 9
        && metadata.workspace_bytes == 393_216
        && metadata.descriptor_offset == 0x4740
        && metadata.descriptor_bytes == 296
        && metadata.iv_offset == 0x4868
        && metadata.iv_bytes == 72)
        .then_some(metadata)
}

fn read_word(image: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = image.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_known_layout() -> Vec<u8> {
        let mut image = vec![0; IMAGE_BYTES];
        for (offset, value) in [
            (0x800usize, 0u32),
            (0x804, 2020),
            (0x808, 10152),
            (0x80C, 2022),
            (0x810, 3),
            (0x814, 0),
            (0x4740, 9),
            (0x4744, 393_216),
        ] {
            image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for index in 0..9 {
            let offset = 0x4748 + index * 32;
            for (word, value) in [0x818u32, 0, 0x27A8, 0, 0, 0, 0xC000, 0]
                .into_iter()
                .enumerate()
            {
                image[offset + word * 4..offset + word * 4 + 4]
                    .copy_from_slice(&value.to_le_bytes());
            }
        }
        image
    }

    #[test]
    fn synthetic_identity_requires_full_span_checksum_and_measured_layout() {
        let image = synthetic_known_layout();
        let expected = fingerprints(&image);
        assert_eq!(expected, fingerprints(&image));
        let metadata = validate_image_with_fingerprints(&image, expected).unwrap();
        assert_eq!(metadata.map_count, 9);
        assert_eq!(metadata.workspace_bytes, 393_216);

        let mut changed = image.clone();
        changed[0x800] = 1;
        assert!(validate_image_with_fingerprints(&changed, expected).is_none());
        let mut changed_header = image;
        changed_header[0x800] = 1;
        let changed_digest = fingerprints(&changed_header);
        assert!(validate_image_with_fingerprints(&changed_header, changed_digest).is_none());
    }
}
