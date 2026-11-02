// SPDX-License-Identifier: GPL-3.0-or-later
//! T3933: byte layout of the C snapshot records the Rust completion front-end reads
//! (`dsound_stream_snapshot`, `dsound_buffer_snapshot`). The records are filled by C and read here as
//! raw bytes at fixed offsets; `tests/completion_layout_c.rs` compiles the real headers and compares every constant,
//! so a C field move fails a test instead of corrupting a read.

/// `sizeof(dsound_stream_snapshot)`.
pub const STREAM_SNAPSHOT_BYTES: usize = 440;
/// `sizeof(dsound_buffer_snapshot)`.
pub const BUFFER_SNAPSHOT_BYTES: usize = 272;

pub const STREAM_MAX_PACKETS: usize = 12;
pub const STREAM_AVERAGE_BYTES_PER_SECOND: usize = 20;
pub const STREAM_BLOCK_ALIGN: usize = 26;
pub const STREAM_SCOPE_FORMAT: usize = 52;
pub const STREAM_LEASE_SERIAL: usize = 80;
pub const STREAM_DISCONTINUITY_SEEN: usize = 228;
pub const STREAM_VOLUME_SEEN: usize = 229;
pub const STREAM_FORMAT_SETS: usize = 236;
pub const STREAM_FORMAT: usize = 252;
pub const STREAM_FORMAT_BYTES: usize = 20;

pub const BUFFER_FLAGS: usize = 8;
pub const BUFFER_BLOCK_ALIGN: usize = 26;
pub const BUFFER_LEASE_SERIAL: usize = 80;
pub const BUFFER_CACHE_MASK: usize = 144;
pub const BUFFER_DATA_ADDRESS: usize = 196;
pub const BUFFER_DATA_LENGTH: usize = 200;
pub const BUFFER_DATA_SETS: usize = 204;
pub const BUFFER_LOOP_START: usize = 208;
pub const BUFFER_LOOP_LENGTH: usize = 212;
pub const BUFFER_LOOP_SETS: usize = 216;
pub const BUFFER_VOLUME: usize = 220;
pub const BUFFER_VOLUME_SETS: usize = 224;
pub const BUFFER_PAUSE_SETS: usize = 228;
pub const BUFFER_FREQUENCY: usize = 232;
pub const BUFFER_FREQUENCY_SETS: usize = 236;

/// A raw snapshot record with checked little-endian field reads (C fills it, so any bit pattern is valid).
#[repr(C, align(8))]
pub struct Snapshot<const N: usize>(pub [u8; N]);

impl<const N: usize> Snapshot<N> {
    pub fn zeroed() -> Self {
        Self([0; N])
    }
    pub fn u8_at(&self, offset: usize) -> u8 {
        self.0[offset]
    }
    pub fn u16_at(&self, offset: usize) -> u16 {
        u16::from_le_bytes(self.0[offset..offset + 2].try_into().unwrap())
    }
    pub fn u32_at(&self, offset: usize) -> u32 {
        u32::from_le_bytes(self.0[offset..offset + 4].try_into().unwrap())
    }
    pub fn i32_at(&self, offset: usize) -> i32 {
        self.u32_at(offset) as i32
    }
    pub fn u64_at(&self, offset: usize) -> u64 {
        u64::from_le_bytes(self.0[offset..offset + 8].try_into().unwrap())
    }
    pub fn bytes_at<const L: usize>(&self, offset: usize) -> [u8; L] {
        self.0[offset..offset + L].try_into().unwrap()
    }
    pub fn as_mut_ptr<T>(&mut self) -> *mut T {
        self.0.as_mut_ptr().cast()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_are_little_endian_at_their_offsets() {
        let mut s = Snapshot::<STREAM_SNAPSHOT_BYTES>::zeroed();
        s.0[STREAM_LEASE_SERIAL..STREAM_LEASE_SERIAL + 8]
            .copy_from_slice(&0x1122334455667788u64.to_le_bytes());
        assert_eq!(s.u64_at(STREAM_LEASE_SERIAL), 0x1122334455667788);
        assert_eq!(s.u32_at(STREAM_LEASE_SERIAL), 0x55667788);
        assert_eq!(s.u8_at(STREAM_LEASE_SERIAL), 0x88);
    }
}
