// SPDX-License-Identifier: GPL-3.0-or-later
//! Audio stream records retain guest ownership. Meanings are INFERRED.
#[path = "../audio_slice/boundary.rs"] mod request_boundary;
pub use request_boundary::*;
use crate::memory::GuestMemory;

/// Thiscall object consumed by stream methods; no host mirror or backend handle ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioStreamRef { pub address: u32 }

/// Source-inspected field offsets, with bits retained rather than assumed host units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioStreamPrefix {
    pub backend_handle: u32,
    pub field54: u32,
    pub scalar58_bits: u32,
    pub scalar5c_bits: u32,
    pub field64: u32,
}
impl AudioStreamRef {
    pub fn prefix<M: GuestMemory>(self, memory: &M) -> AudioStreamPrefix {
        let read = |offset| memory.read32(self.address.wrapping_add(offset));
        AudioStreamPrefix { backend_handle: read(4), field54: read(0x54), scalar58_bits: read(0x58), scalar5c_bits: read(0x5c), field64: read(0x64) }
    }
}
