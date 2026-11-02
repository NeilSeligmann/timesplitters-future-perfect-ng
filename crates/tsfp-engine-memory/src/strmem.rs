// SPDX-License-Identifier: GPL-3.0-or-later
//! T1892 shared string and record helpers over the byte-level `GuestMemory` of `leaf.rs`.
//! Path-included (`crate::memory` is `leaf.rs`) by the small-system crates so the readable
//! ports can say `strlen`, `strncpy`, `strchr` instead of open-coding the byte loops.
use crate::memory::GuestMemory;
use std::collections::HashMap;

/// C-runtime style helpers on guest memory. All addresses wrap at 32 bits.
pub trait GuestStrings: GuestMemory {
    fn write16(&mut self, address: u32, value: u16) {
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.write8(address.wrapping_add(i as u32), byte);
        }
    }
    /// Signed 32-bit read, for fields the game compares as `int`.
    fn read_i32(&self, address: u32) -> i32 {
        self.read32(address) as i32
    }
    /// `strlen`.
    fn strlen(&self, text: u32) -> u32 {
        let mut length = 0u32;
        while self.read8(text.wrapping_add(length)) != 0 {
            length = length.wrapping_add(1);
        }
        length
    }
    /// `strchr`: address of the first `ch` (or of the terminator when `ch` is 0), else 0.
    fn strchr(&self, text: u32, ch: u8) -> u32 {
        let mut at = text;
        loop {
            let byte = self.read8(at);
            if byte == ch {
                return at;
            }
            if byte == 0 {
                return 0;
            }
            at = at.wrapping_add(1);
        }
    }
    /// `strstr`: address of the first occurrence of `needle` in `haystack`, else 0.
    fn strstr(&self, haystack: u32, needle: u32) -> u32 {
        let needle_len = self.strlen(needle);
        let mut at = haystack;
        loop {
            if (0..needle_len).all(|i| self.read8(at.wrapping_add(i)) == self.read8(needle.wrapping_add(i))) {
                return at;
            }
            if self.read8(at) == 0 {
                return 0;
            }
            at = at.wrapping_add(1);
        }
    }
    /// Copies the string with its terminator (`strcpy`).
    fn strcpy(&mut self, dst: u32, src: u32) {
        let mut i = 0u32;
        loop {
            let byte = self.read8(src.wrapping_add(i));
            self.write8(dst.wrapping_add(i), byte);
            if byte == 0 {
                return;
            }
            i = i.wrapping_add(1);
        }
    }
    /// `strncpy`: copies up to `count` bytes and zero-pads the rest.
    fn strncpy(&mut self, dst: u32, src: u32, count: u32) {
        let mut ended = false;
        for i in 0..count {
            let byte = if ended { 0 } else { self.read8(src.wrapping_add(i)) };
            ended |= byte == 0;
            self.write8(dst.wrapping_add(i), byte);
        }
    }
    /// `memcpy` forward byte copy.
    fn copy_bytes(&mut self, dst: u32, src: u32, count: u32) {
        for i in 0..count {
            let byte = self.read8(src.wrapping_add(i));
            self.write8(dst.wrapping_add(i), byte);
        }
    }
    /// Reads a NUL-terminated string (Latin-1 widened to chars) for tests and logs.
    fn read_cstring(&self, text: u32) -> String {
        let length = self.strlen(text);
        (0..length).map(|i| self.read8(text.wrapping_add(i)) as char).collect()
    }
    /// Writes `text` plus a NUL terminator.
    fn write_cstring(&mut self, dst: u32, text: &str) {
        for (i, byte) in text.bytes().enumerate() {
            self.write8(dst.wrapping_add(i as u32), byte);
        }
        self.write8(dst.wrapping_add(text.len() as u32), 0);
    }
}
impl<T: GuestMemory + ?Sized> GuestStrings for T {}

/// Sparse little-endian guest memory for tests and fixtures. Unwritten bytes read as zero.
#[derive(Clone, Debug, Default)]
pub struct SparseMemory {
    bytes: HashMap<u32, u8>,
}
impl SparseMemory {
    pub fn new() -> Self {
        Self::default()
    }
    /// Number of explicitly written bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}
impl GuestMemory for SparseMemory {
    fn read8(&self, address: u32) -> u8 {
        self.bytes.get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.bytes.insert(address, value);
    }
}
