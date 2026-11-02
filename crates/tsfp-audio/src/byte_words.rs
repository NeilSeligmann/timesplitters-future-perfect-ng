//! Shared byte decoding for validated audio records.

/// Read a little-endian word at a byte offset. Callers validate record extents;
/// an undersized slice retains the existing indexing panic rather than fabricating data.
pub(crate) fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}
