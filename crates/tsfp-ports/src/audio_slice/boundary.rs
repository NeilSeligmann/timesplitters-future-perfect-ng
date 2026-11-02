// SPDX-License-Identifier: GPL-3.0-or-later
//! Guest-owned shared audio records: source-inspected addresses, inferred names.
use crate::memory::GuestMemory;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioSettingsWords { pub word_5236b8: u32, pub word_5236bc: u32 }
impl AudioSettingsWords {
    pub fn read<M: GuestMemory>(m: &M) -> Self {
        Self { word_5236b8: m.read32(0x5236b8), word_5236bc: m.read32(0x5236bc) }
    }
}
/// Sound request pointer passed by missions, objects, weapons and UI.
/// Keeps ownership in guest memory; callers retain the original ABI/field offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundRequest { pub address: u32 }
/// Low-level audio attachment remains GuestCall: XDK handles and return register
/// state must be supplied by the retained implementation, never invented here.
pub const SYSTEM: &str = "engine/audio";

/// Source-inspected request fields consumed by the 218d0 wrapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundRequestPrefix { pub id: u32, pub flags: u32, pub scalar18_bits: u32 }
impl SoundRequest {
    pub fn prefix<M: GuestMemory>(self, m: &M) -> SoundRequestPrefix {
        SoundRequestPrefix { id: m.read32(self.address), flags: m.read32(self.address.wrapping_add(0x10)),
            scalar18_bits: m.read32(self.address.wrapping_add(0x18)) }
    }
}
