// SPDX-License-Identifier: GPL-3.0-or-later
//! Guest memory helpers for the render slice (T1898): typed accessors on top of
//! [`GuestMemory`] and a sparse in-memory image for tests and tools.
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::collections::HashMap;
#[cfg(feature = "std")]
use std::hash::{BuildHasherDefault, Hasher};

use crate::memory::GuestMemory;

/// Fixed-width accessors the render code needs beyond the byte-level [`GuestMemory`].
pub trait GuestAccess: GuestMemory {
    fn write16(&mut self, address: u32, value: u16) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.write8(address.wrapping_add(offset as u32), byte);
        }
    }
    fn read_f32(&self, address: u32) -> f32 {
        f32::from_bits(self.read32(address))
    }
    fn write_f32(&mut self, address: u32, value: f32) {
        self.write32(address, value.to_bits());
    }
    /// `*address |= bits`.
    fn or32(&mut self, address: u32, bits: u32) {
        let value = self.read32(address);
        self.write32(address, value | bits);
    }
    /// Copy `count` dwords starting at `address`.
    fn read_words(&self, address: u32, count: u32) -> Vec<u32> {
        (0..count)
            .map(|index| self.read32(address.wrapping_add(index.wrapping_mul(4))))
            .collect()
    }
}

impl<T: GuestMemory + ?Sized> GuestAccess for T {}

#[cfg(feature = "std")]
const PAGE_BITS: u32 = 12;
#[cfg(feature = "std")]
const PAGE_SIZE: usize = 1 << PAGE_BITS;

/// Multiplicative hash for the page numbers (T2136f): the default SipHash cost most of a replay's time, every guest byte read and
/// written went through one lookup.
#[cfg(feature = "std")]
#[derive(Default)]
pub struct PageHasher(u64);

#[cfg(feature = "std")]
impl Hasher for PageHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0.rotate_left(5) ^ u64::from(byte)).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }
    fn write_u32(&mut self, value: u32) {
        self.0 = u64::from(value).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
    fn finish(&self) -> u64 {
        self.0 ^ (self.0 >> 32)
    }
}

/// Sparse guest image, zero filled, used by the unit tests and the recorded-call tools.
#[cfg(feature = "std")]
#[derive(Clone, Debug, Default)]
pub struct SparseMemory {
    pages: HashMap<u32, Box<[u8; PAGE_SIZE]>, BuildHasherDefault<PageHasher>>,
}

#[cfg(feature = "std")]
impl SparseMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Like [`write_bytes`](Self::write_bytes), but `changed` is called with the page number of every page whose content really
    /// changed, and the number of bytes that differed is returned (T2136f: the guest re-sends texel memory that did not change).
    pub fn write_bytes_diff(
        &mut self,
        address: u32,
        bytes: &[u8],
        mut changed: impl FnMut(u32),
    ) -> usize {
        let mut address = address;
        let mut rest = bytes;
        let mut different = 0;
        while !rest.is_empty() {
            let offset = (address as usize) & (PAGE_SIZE - 1);
            let take = rest.len().min(PAGE_SIZE - offset);
            let page_number = address >> PAGE_BITS;
            let page = self
                .pages
                .entry(page_number)
                .or_insert_with(|| Box::new([0; PAGE_SIZE]));
            let target = &mut page[offset..offset + take];
            if target != &rest[..take] {
                different += target
                    .iter()
                    .zip(&rest[..take])
                    .filter(|(old, new)| old != new)
                    .count();
                target.copy_from_slice(&rest[..take]);
                changed(page_number);
            }
            address = address.wrapping_add(take as u32);
            rest = &rest[take..];
        }
        different
    }

    /// Write a run of bytes, one page lookup per page instead of per byte (T2136f). Addresses wrap at 4 GiB like `write8`.
    pub fn write_bytes(&mut self, address: u32, bytes: &[u8]) {
        let mut address = address;
        let mut rest = bytes;
        while !rest.is_empty() {
            let offset = (address as usize) & (PAGE_SIZE - 1);
            let take = rest.len().min(PAGE_SIZE - offset);
            let page = self
                .pages
                .entry(address >> PAGE_BITS)
                .or_insert_with(|| Box::new([0; PAGE_SIZE]));
            page[offset..offset + take].copy_from_slice(&rest[..take]);
            address = address.wrapping_add(take as u32);
            rest = &rest[take..];
        }
    }
}

#[cfg(feature = "std")]
impl GuestMemory for SparseMemory {
    fn read8(&self, address: u32) -> u8 {
        self.pages
            .get(&(address >> PAGE_BITS))
            .map_or(0, |page| page[(address as usize) & (PAGE_SIZE - 1)])
    }
    fn write8(&mut self, address: u32, value: u8) {
        let page = self
            .pages
            .entry(address >> PAGE_BITS)
            .or_insert_with(|| Box::new([0; PAGE_SIZE]));
        page[(address as usize) & (PAGE_SIZE - 1)] = value;
    }
    fn read16(&self, address: u32) -> u16 {
        let offset = (address as usize) & (PAGE_SIZE - 1);
        if offset + 2 <= PAGE_SIZE {
            return self.pages.get(&(address >> PAGE_BITS)).map_or(0, |page| {
                u16::from_le_bytes([page[offset], page[offset + 1]])
            });
        }
        u16::from_le_bytes([self.read8(address), self.read8(address.wrapping_add(1))])
    }
    fn read32(&self, address: u32) -> u32 {
        let offset = (address as usize) & (PAGE_SIZE - 1);
        if offset + 4 <= PAGE_SIZE {
            return self.pages.get(&(address >> PAGE_BITS)).map_or(0, |page| {
                u32::from_le_bytes([
                    page[offset],
                    page[offset + 1],
                    page[offset + 2],
                    page[offset + 3],
                ])
            });
        }
        u32::from_le_bytes(core::array::from_fn(|i| {
            self.read8(address.wrapping_add(i as u32))
        }))
    }
    fn read_bytes(&self, address: u32, out: &mut [u8]) {
        let mut address = address;
        let mut done = 0;
        while done < out.len() {
            let offset = (address as usize) & (PAGE_SIZE - 1);
            let take = (out.len() - done).min(PAGE_SIZE - offset);
            match self.pages.get(&(address >> PAGE_BITS)) {
                Some(page) => out[done..done + take].copy_from_slice(&page[offset..offset + take]),
                None => out[done..done + take].fill(0),
            }
            address = address.wrapping_add(take as u32);
            done += take;
        }
    }
}

/// `cvttss2si`: truncate toward zero. NaN and values outside the i32 range give
/// `i32::MIN`, the x86 "integer indefinite" value (Rust's `as` saturates instead).
pub fn truncate_to_i32(value: f32) -> i32 {
    if (-2_147_483_648.0..2_147_483_648.0).contains(&value) {
        value as i32
    } else {
        i32::MIN
    }
}
