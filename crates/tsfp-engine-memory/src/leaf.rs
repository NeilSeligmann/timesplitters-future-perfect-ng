// SPDX-License-Identifier: GPL-3.0-or-later
/// Addresses remain guest u32 values; implementations choose the backing store.
pub trait GuestMemory {
    /// Whether the complete guest byte range is currently mapped and readable.
    /// Generic test memories may omit mapping information; live host mappings
    /// override this before adapters dereference guest supplied pointers.
    fn is_readable(&self, _address: u32, _len: u32) -> bool {
        true
    }
    fn read8(&self, address: u32) -> u8;
    fn write8(&mut self, address: u32, value: u8);
    fn read16(&self, address: u32) -> u16 {
        u16::from_le_bytes([self.read8(address), self.read8(address.wrapping_add(1))])
    }
    fn read32(&self, address: u32) -> u32 {
        u32::from_le_bytes(core::array::from_fn(|i| {
            self.read8(address.wrapping_add(i as u32))
        }))
    }
    /// Copy `out.len()` bytes starting at `address` (T2136d: the render trace snapshots whole runs; a backing store with a flat
    /// mapping overrides this with one `memcpy`).
    fn read_bytes(&self, address: u32, out: &mut [u8]) {
        for (offset, byte) in out.iter_mut().enumerate() {
            *byte = self.read8(address.wrapping_add(offset as u32));
        }
    }
    /// `len` bytes at `address` as a slice when the backing store is flat memory (T2136d: the render trace hashes and copies
    /// them without a per byte loop). `None` when it cannot hand out a slice.
    fn as_slice(&self, _address: u32, _len: u32) -> Option<&[u8]> {
        None
    }
    fn write32(&mut self, address: u32, value: u32) {
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.write8(address.wrapping_add(i as u32), byte);
        }
    }
}
