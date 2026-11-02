//! Safe byte-buffer form of the XBE copy leaf at address 0x591D0.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackedCopyError {
    MissingSourceFlagByte,
    PrefixRequiresTwoBytes,
    SourceTooShort,
    DestinationTooShort,
}

/// Copy exactly `byte_count` bytes using the observed optional two-byte prefix.
///
/// When source byte zero has bit `0x02`, the original copies its first word
/// separately, then copies the remaining bytes. For valid lengths the final
/// destination bytes are the same as a contiguous copy. Invalid buffer extents
/// are reported rather than allowing the original pointer routine to overrun.
/// Verification: Tier 0 (focused smoke tests only; no differential or replay).
pub fn copy_packed_bytes_591d0(
    source: &[u8],
    destination: &mut [u8],
    byte_count: usize,
) -> Result<usize, PackedCopyError> {
    let flags = *source
        .first()
        .ok_or(PackedCopyError::MissingSourceFlagByte)?;
    if flags & 0x02 != 0 && byte_count < 2 {
        return Err(PackedCopyError::PrefixRequiresTwoBytes);
    }
    if source.len() < byte_count {
        return Err(PackedCopyError::SourceTooShort);
    }
    if destination.len() < byte_count {
        return Err(PackedCopyError::DestinationTooShort);
    }

    if flags & 0x02 != 0 {
        destination[..2].copy_from_slice(&source[..2]);
        destination[2..byte_count].copy_from_slice(&source[2..byte_count]);
    } else {
        destination[..byte_count].copy_from_slice(&source[..byte_count]);
    }
    Ok(byte_count)
}

#[cfg(test)]
mod tests {
    use super::{PackedCopyError, copy_packed_bytes_591d0};

    #[test]
    fn copies_counted_payload_with_or_without_prefix_flag() {
        let source = [0x02, 0xA1, 0xB2, 3, 4, 5, 6, 7];
        let mut prefixed = [0; 8];
        assert_eq!(copy_packed_bytes_591d0(&source, &mut prefixed, 8), Ok(8));
        assert_eq!(prefixed, source);

        let source_without_prefix = [0x00, 0xA1, 0xB2, 3, 4, 5, 6, 7];
        let mut plain = [0; 8];
        assert_eq!(
            copy_packed_bytes_591d0(&source_without_prefix, &mut plain, 8),
            Ok(8)
        );
        assert_eq!(plain, source_without_prefix);
    }

    #[test]
    fn rejects_prefix_underflow_and_short_buffers() {
        let source = [0x02, 1];
        let mut destination = [0; 2];
        assert_eq!(
            copy_packed_bytes_591d0(&source, &mut destination, 1),
            Err(PackedCopyError::PrefixRequiresTwoBytes)
        );
        assert_eq!(
            copy_packed_bytes_591d0(&source, &mut destination, 3),
            Err(PackedCopyError::SourceTooShort)
        );

        let source = [0x00, 1, 2];
        assert_eq!(
            copy_packed_bytes_591d0(&source, &mut destination, 3),
            Err(PackedCopyError::DestinationTooShort)
        );
    }
}
