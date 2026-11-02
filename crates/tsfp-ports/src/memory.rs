// SPDX-License-Identifier: GPL-3.0-or-later
// Host backing for the shared low-level GuestMemory trait in tsfp-port-runtime.
#[path = "../../tsfp-direct-witness/src/lib.rs"]
mod direct_witness;
pub use tsfp_port_runtime::memory::GuestMemory;

/// Hybrid backing: supplied by the existing low-window mapping, never owns it.
pub struct MappedGuest {
    offset: usize,
}
unsafe extern "C" {
    fn tsfp_render_trace_readable(address: u32, length: u32) -> u32;
}
impl MappedGuest {
    /// # Safety
    /// The caller owns mapped readable/writable guest ranges for the duration of use.
    /// Invalid accesses intentionally fault for the differential driver's fault channel.
    pub unsafe fn new(offset: usize) -> Self {
        Self { offset }
    }
    /// The mapping offset (T2161: lets a nested `GuestCall` build its own view of the same window).
    pub fn offset(&self) -> usize {
        self.offset
    }

    fn frame_state_span_is_owned(address: u32, len: usize) -> bool {
        crate::frame_state_ffi::native_frame_owned_span(address, len)
    }
}
impl GuestMemory for MappedGuest {
    fn is_readable(&self, address: u32, len: u32) -> bool {
        // SAFETY: this host helper checks the guest range against the image and
        // the live guest allocation registry without dereferencing the range.
        unsafe { tsfp_render_trace_readable(address, len) != 0 }
    }
    fn read8(&self, address: u32) -> u8 {
        if let Some(value) = crate::frame_state_ffi::native_frame_owned_word_read(address, 1) {
            return value as u8;
        }
        unsafe {
            core::ptr::read_volatile((address as usize).wrapping_add(self.offset) as *const u8)
        }
    }
    fn as_slice(&self, address: u32, len: u32) -> Option<&[u8]> {
        if Self::frame_state_span_is_owned(address, len as usize) {
            return None;
        }
        // SAFETY: as for `read8`, the caller (the render trace) checked the span is mapped (`tsfp_render_trace_readable`).
        Some(unsafe {
            core::slice::from_raw_parts(
                (address as usize).wrapping_add(self.offset) as *const u8,
                len as usize,
            )
        })
    }
    fn read_bytes(&self, address: u32, out: &mut [u8]) {
        if Self::frame_state_span_is_owned(address, out.len()) {
            for (index, value) in out.iter_mut().enumerate() {
                let at = address.wrapping_add(index as u32);
                *value = if let Some(owned) =
                    crate::frame_state_ffi::native_frame_owned_word_read(at, 1)
                {
                    owned as u8
                } else {
                    unsafe {
                        core::ptr::read_volatile(
                            (at as usize).wrapping_add(self.offset) as *const u8
                        )
                    }
                };
            }
            return;
        }
        unsafe {
            core::ptr::copy_nonoverlapping(
                (address as usize).wrapping_add(self.offset) as *const u8,
                out.as_mut_ptr(),
                out.len(),
            )
        }
    }
    fn write8(&mut self, address: u32, value: u8) {
        #[cfg(rust_lane_mutation)]
        crate::lane_inv::note_write(address);
        if crate::frame_state_ffi::native_frame_owned_word_write(address, 1, value as u32) {
            return;
        }
        let pointer = (address as usize).wrapping_add(self.offset);
        unsafe { direct_witness::store8(pointer, address, value) }
    }
    // T2143: aligned 16 and 32 bit accesses are one volatile load or store. The trait default is four (two) volatile byte
    // accesses, and a Rust port reads and writes guest dwords all the time (every store of an object-family root first
    // decodes the address with up to 32 table reads), which made a trivial Rust leaf cost about 600 ns against 30 ns for its
    // lifted original (MEASURED, docs/t-perf-playtest.md). One dword access is also closer to the guest's own `mov`. The
    // mutation build keeps the byte writes, its write log (`note_write`) is per byte. Unaligned accesses stay bytewise.
    fn read16(&self, address: u32) -> u16 {
        if let Some(value) = crate::frame_state_ffi::native_frame_owned_word_read(address, 2) {
            return value as u16;
        }
        let pointer = (address as usize).wrapping_add(self.offset);
        if pointer & 1 == 0 {
            u16::from_le(unsafe { core::ptr::read_volatile(pointer as *const u16) })
        } else {
            u16::from_le_bytes([self.read8(address), self.read8(address.wrapping_add(1))])
        }
    }
    fn read32(&self, address: u32) -> u32 {
        if let Some(value) = crate::frame_state_ffi::native_frame_owned_word_read(address, 4) {
            return value;
        }
        let pointer = (address as usize).wrapping_add(self.offset);
        if pointer & 3 == 0 {
            u32::from_le(unsafe { core::ptr::read_volatile(pointer as *const u32) })
        } else {
            u32::from_le_bytes(core::array::from_fn(|i| {
                self.read8(address.wrapping_add(i as u32))
            }))
        }
    }
    #[cfg(not(rust_lane_mutation))]
    fn write32(&mut self, address: u32, value: u32) {
        if crate::frame_state_ffi::native_frame_owned_word_write(address, 4, value) {
            return;
        }
        let pointer = (address as usize).wrapping_add(self.offset);
        if pointer & 3 == 0 {
            unsafe { direct_witness::store32(pointer, address, value) }
        } else {
            for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
                self.write8(address.wrapping_add(i as u32), byte);
            }
        }
    }
}
