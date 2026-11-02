// SPDX-License-Identifier: GPL-3.0-or-later
//! Measured actor allocation result through initial extension-slot binding.
#![no_std]

#[cfg(test)]
extern crate std;

use tsfp_port_runtime::memory::GuestMemory;

pub const ACTOR_EXTENSION_TABLE: u32 = 0x0073_56d8;
pub const ACTOR_EXTENSION_STRIDE: u32 = 0xea0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnBinding {
    AllocationFailed,
    Bound {
        object: u32,
        extension: u32,
        slot: u32,
    },
}

pub const fn resolve_spawn_binding(object: u32, extension_table: u32, slot: u32) -> SpawnBinding {
    if object == 0 {
        SpawnBinding::AllocationFailed
    } else {
        SpawnBinding::Bound {
            object,
            extension: extension_table.wrapping_add(slot.wrapping_mul(ACTOR_EXTENSION_STRIDE)),
            slot,
        }
    }
}

/// Checks the constructor's published object/extension/slot tuple at append.
pub fn published_binding_matches<M: GuestMemory>(memory: &M, object: u32) -> bool {
    object != 0
        && published_slot_for_extension(memory, memory.read32(object.wrapping_add(0x7c))).is_some()
}

/// Returns the measured pool slot only when the extension and object back-links
/// agree with the constructor's fixed slot table.
pub fn published_slot_for_extension<M: GuestMemory>(memory: &M, extension: u32) -> Option<u16> {
    let offset = extension.checked_sub(ACTOR_EXTENSION_TABLE)?;
    if offset % ACTOR_EXTENSION_STRIDE != 0 {
        return None;
    }
    let slot = offset / ACTOR_EXTENSION_STRIDE;
    let object = memory.read32(extension);
    if object == 0
        || memory.read32(object.wrapping_add(0x7c)) != extension
        || memory.read32(extension.wrapping_add(8)) != slot
    {
        return None;
    }
    u16::try_from(slot).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        ACTOR_EXTENSION_STRIDE, ACTOR_EXTENSION_TABLE, SpawnBinding, published_binding_matches,
        published_slot_for_extension, resolve_spawn_binding,
    };
    use std::collections::BTreeMap;
    use tsfp_port_runtime::memory::GuestMemory;

    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);

    impl Memory {
        fn word(&mut self, address: u32, value: u32) {
            for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
                self.0.insert(address.wrapping_add(offset as u32), byte);
            }
        }
    }

    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[test]
    fn failed_allocation_returns_before_extension_binding() {
        assert_eq!(
            resolve_spawn_binding(0, ACTOR_EXTENSION_TABLE, 7),
            SpawnBinding::AllocationFailed
        );
    }

    #[test]
    fn successful_allocation_binds_actor_and_slot_extension() {
        assert_eq!(
            resolve_spawn_binding(0x1234, ACTOR_EXTENSION_TABLE, 7),
            SpawnBinding::Bound {
                object: 0x1234,
                extension: ACTOR_EXTENSION_TABLE + 7 * ACTOR_EXTENSION_STRIDE,
                slot: 7,
            }
        );
    }

    #[test]
    fn guest_address_math_wraps_like_the_32_bit_constructor() {
        assert_eq!(
            resolve_spawn_binding(0x1234, u32::MAX - 4, 1),
            SpawnBinding::Bound {
                object: 0x1234,
                extension: (u32::MAX - 4).wrapping_add(ACTOR_EXTENSION_STRIDE),
                slot: 1,
            }
        );
    }

    #[test]
    fn append_observer_accepts_only_the_published_constructor_slot() {
        let object = 0x2000;
        let slot = 3;
        let extension = ACTOR_EXTENSION_TABLE + slot * ACTOR_EXTENSION_STRIDE;
        let mut memory = Memory::default();
        memory.word(object + 0x7c, extension);
        memory.word(extension, object);
        memory.word(extension + 8, slot);

        assert!(published_binding_matches(&memory, object));
        assert_eq!(published_slot_for_extension(&memory, extension), Some(3));
        memory.word(object + 0x7c, extension + 4);
        assert!(!published_binding_matches(&memory, object));
        assert_eq!(published_slot_for_extension(&memory, extension), None);
        assert!(!published_binding_matches(&memory, 0));
    }
}
