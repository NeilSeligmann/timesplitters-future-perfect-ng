//! Checked little-endian format primitives. No guest pointers are dereferenced.
//!
//! ```
//! use tsfp_format_core::{crc32, put, word, words};
//! let mut bytes = [0; 8];
//! put(&mut bytes, 0, 0x1234_5678);
//! put(&mut bytes, 4, 9);
//! assert_eq!(word(&bytes, 0), Ok(0x1234_5678));
//! assert_eq!(words(&bytes, 0, 2), Ok(vec![0x1234_5678, 9]));
//! assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
//! ```
pub type Result<T> = std::result::Result<T, &'static str>;
pub fn range(b: &[u8], o: usize, n: usize) -> Result<&[u8]> {
    b.get(o..o.checked_add(n).ok_or("extent overflow")?)
        .ok_or("truncated extent")
}
pub fn word(b: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(range(b, o, 4)?.try_into().unwrap()))
}
pub fn words(b: &[u8], o: usize, n: usize) -> Result<Vec<u32>> {
    range(b, o, n.checked_mul(4).ok_or("count overflow")?)?;
    (0..n).map(|i| word(b, o + i * 4)).collect()
}
/// Write one little-endian word, returning the same extent errors as [`range`].
pub fn try_put(b: &mut [u8], o: usize, w: u32) -> Result<()> {
    let end = o.checked_add(4).ok_or("extent overflow")?;
    b.get_mut(o..end)
        .ok_or("truncated extent")?
        .copy_from_slice(&w.to_le_bytes());
    Ok(())
}
/// Write one word for callers that have already checked the output extent.
pub fn put(b: &mut [u8], o: usize, w: u32) {
    try_put(b, o, w).expect("word output extent");
}
pub fn crc32(b: &[u8]) -> u32 {
    let mut c = !0u32;
    for x in b {
        c ^= *x as u32;
        for _ in 0..8 {
            c = (c >> 1) ^ (0xedb88320u32 & 0u32.wrapping_sub(c & 1));
        }
    }
    !c
}
#[test]
fn bounds() {
    assert!(range(&[], usize::MAX, 2).is_err());
    assert!(words(&[], 0, usize::MAX).is_err());
    assert_eq!(crc32(b"123456789"), 0xcbf43926);
}

#[cfg(test)]
mod properties {
    use super::*;

    #[test]
    fn checked_write_preserves_buffer_on_invalid_extents() {
        let mut bytes = [0xa5; 8];
        assert_eq!(try_put(&mut bytes, usize::MAX, 7), Err("extent overflow"));
        assert_eq!(try_put(&mut bytes, 5, 7), Err("truncated extent"));
        assert_eq!(bytes, [0xa5; 8]);
        assert_eq!(try_put(&mut bytes, 2, 0x1234_5678), Ok(()));
        assert_eq!(&bytes[2..6], &[0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn aligned_words_round_trip_and_reject_truncation() {
        for count in 0..=64usize {
            let mut bytes = vec![0u8; count * 4];
            let expected: Vec<u32> = (0..count)
                .map(|index| (index as u32).wrapping_mul(0x9e37_79b9) ^ count as u32)
                .collect();
            for (index, value) in expected.iter().enumerate() {
                put(&mut bytes, index * 4, *value);
            }
            assert_eq!(words(&bytes, 0, count), Ok(expected));
            if count > 0 {
                assert!(words(&bytes[..bytes.len() - 1], 0, count).is_err());
            }
        }
        assert!(range(&[0; 4], usize::MAX, 1).is_err());
        assert!(words(&[0; 4], 0, usize::MAX).is_err());
    }

    #[test]
    fn crc32_stream_concatenation_identity() {
        // CRC(a || b) can be recomputed by feeding b into the finalized CRC of a.
        for len in 0..=256usize {
            let bytes: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(37)).collect();
            for split in [0, len / 2, len] {
                let mut state = !crc32(&bytes[..split]);
                for byte in &bytes[split..] {
                    state ^= *byte as u32;
                    for _ in 0..8 {
                        state = (state >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(state & 1));
                    }
                }
                assert_eq!(crc32(&bytes), !state);
            }
        }
    }
}
