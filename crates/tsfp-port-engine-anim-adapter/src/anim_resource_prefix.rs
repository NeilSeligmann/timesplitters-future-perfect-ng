// SPDX-License-Identifier: GPL-3.0-or-later
//! Runtime seam for the readable `0x1C8540` dependency-list prefix.
//!
//! The borrowed list, runtime name table, and archive mount callback remain
//! guest-owned services. Selection, byte-name deduplication, and fallback
//! policy run through the typed animation model.

use crate::dependency_prefix::{
    DependencyRun, DependencyWorld, FALLBACK_NAME_INDEX, run_dependency_prefix,
};
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    memory::GuestMemory,
};
use alloc::vec::Vec;

const DEPENDENCY_LIST_SERVICE: u32 = 0x6bd30;
const PAK_MOUNT_SERVICE: u32 = 0x5a130;
const RESOURCE_NAME_TABLE: u32 = 0x4fa140;
const RESOURCE_NAME_STRIDE: u32 = 0x3c;

fn enabled_from(value: Option<&[u8]>) -> bool {
    value == Some(b"rust")
}

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

/// Default-off A/B switch. Set `TSFP_ANIM_RESOURCE_PREFIX=rust` to use the
/// typed dependency-prefix model at the `0x1C8540` runtime seam.
#[cfg(not(test))]
pub fn enabled() -> bool {
    let value = unsafe { getenv(c"TSFP_ANIM_RESOURCE_PREFIX".as_ptr()) };
    !value.is_null() && enabled_from(Some(unsafe { core::ffi::CStr::from_ptr(value) }.to_bytes()))
}

#[cfg(test)]
pub fn enabled() -> bool {
    false
}

/// Run the native prefix with the same guest services as the retained caller.
/// Registers and ESP are restored around the typed seam; guest memory effects
/// from the list service and mount callbacks remain visible to the caller.
pub fn run_guest_prefix<M: GuestMemory, C: GuestCall>(
    memory: &mut M,
    guest: &mut C,
    registers: &mut ExtRegisters,
    level: i32,
) -> DependencyRun<u32> {
    let entry = *registers;
    let mut world = GuestResourceWorld {
        memory,
        guest,
        registers,
        list: None,
    };
    let result = run_dependency_prefix(level, &mut world);
    *world.registers = entry;
    result
}

struct GuestResourceWorld<'a, M, C> {
    memory: &'a mut M,
    guest: &'a mut C,
    registers: &'a mut ExtRegisters,
    list: Option<u32>,
}

impl<M: GuestMemory, C: GuestCall> GuestResourceWorld<'_, M, C> {
    fn call(&mut self, target: u32, args: &[u32]) -> u32 {
        let before = *self.registers;
        call_cdecl(self.guest, self.memory, self.registers, target, args);
        let result = self.registers.eax;
        *self.registers = before;
        result
    }

    fn resource_name_pointer(index: u32) -> u32 {
        RESOURCE_NAME_TABLE.wrapping_add(index.wrapping_mul(RESOURCE_NAME_STRIDE))
    }

    fn read_name(&self, index: u32) -> Option<Vec<u8>> {
        let pointer = self.memory.read32(Self::resource_name_pointer(index));
        if pointer == 0 {
            return None;
        }
        let mut name = Vec::new();
        let mut at = pointer;
        loop {
            let byte = self.memory.read8(at);
            if byte == 0 {
                return Some(name);
            }
            name.push(byte);
            at = at.wrapping_add(1);
        }
    }
}

impl<M: GuestMemory, C: GuestCall> DependencyWorld for GuestResourceWorld<'_, M, C> {
    type MountResult = u32;

    fn list_present(&mut self) -> bool {
        let list = self.call(DEPENDENCY_LIST_SERVICE, &[]);
        self.list = (list != 0).then_some(list);
        self.list.is_some()
    }

    fn list_count(&mut self) -> i32 {
        self.list
            .map_or(0, |list| self.memory.read32(list.wrapping_add(4)) as i32)
    }

    fn list_index(&mut self, row: i32) -> u32 {
        let list = self.list.unwrap_or(0);
        let offset = 8u32.wrapping_add((row as u32).wrapping_mul(4));
        self.memory.read32(list.wrapping_add(offset))
    }

    fn name_bytes(&mut self, index: u32) -> Option<Vec<u8>> {
        self.read_name(index)
    }

    fn fallback_name(&mut self) -> Vec<u8> {
        self.read_name(FALLBACK_NAME_INDEX).unwrap_or_default()
    }

    fn mount(&mut self, resource_index: u32, _name: &[u8]) -> Self::MountResult {
        // Preserve the game's runtime string pointer instead of rebuilding a
        // host string or canonicalizing the path.
        let pointer = self
            .memory
            .read32(Self::resource_name_pointer(resource_index));
        self.call(PAK_MOUNT_SERVICE, &[pointer])
    }
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};
    use std::collections::BTreeMap;

    use super::{
        DEPENDENCY_LIST_SERVICE, PAK_MOUNT_SERVICE, RESOURCE_NAME_STRIDE, RESOURCE_NAME_TABLE,
        enabled_from, run_guest_prefix,
    };
    use crate::{
        abi::{ExtRegisters, Fpu, GuestCall, Xmm},
        memory::GuestMemory,
    };

    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);

    impl Memory {
        fn write_bytes(&mut self, address: u32, bytes: &[u8]) {
            for (offset, byte) in bytes.iter().copied().enumerate() {
                self.write8(address.wrapping_add(offset as u32), byte);
            }
        }

        fn write_c_string(&mut self, address: u32, bytes: &[u8]) {
            self.write_bytes(address, bytes);
            self.write8(address.wrapping_add(bytes.len() as u32), 0);
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

    #[derive(Default)]
    struct Guest {
        list: u32,
        calls: Vec<(u32, u32)>,
    }

    impl GuestCall for Guest {
        fn call(&mut self, address: u32, registers: &mut ExtRegisters) {
            self.calls.push((address, registers.esp));
            registers.eax = if address == DEPENDENCY_LIST_SERVICE {
                self.list
            } else {
                assert_eq!(address, PAK_MOUNT_SERVICE);
                1
            };
        }
    }

    fn registers() -> ExtRegisters {
        ExtRegisters {
            eax: 0x1111,
            ecx: 0x2222,
            edx: 0x3333,
            esp: 0x8000,
            ebx: 0x4444,
            ebp: 0x5555,
            esi: 0x6666,
            edi: 0x7777,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        }
    }

    fn put_name(memory: &mut Memory, index: u32, pointer: u32, name: &[u8]) {
        memory.write32(
            RESOURCE_NAME_TABLE.wrapping_add(index.wrapping_mul(RESOURCE_NAME_STRIDE)),
            pointer,
        );
        memory.write_c_string(pointer, name);
    }

    #[test]
    fn switch_is_opt_in() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(enabled_from(Some(b"rust")));
    }

    #[test]
    fn guest_prefix_calls_borrowed_list_and_mounts_original_name_pointer() {
        let mut memory = Memory::default();
        memory.write32(0x1004, 1);
        memory.write32(0x1008, 0x38);
        put_name(&mut memory, 0x38, 0x2000, b"anim/dep.pak");
        put_name(&mut memory, 1, 0x2100, b"fallback.pak");
        let mut guest = Guest {
            list: 0x1000,
            calls: Vec::new(),
        };
        let mut registers = registers();
        let entry = registers;

        let run = run_guest_prefix(&mut memory, &mut guest, &mut registers, 0x64);

        assert_eq!(registers, entry);
        assert_eq!(
            guest.calls,
            [
                (DEPENDENCY_LIST_SERVICE, 0x8000),
                (PAK_MOUNT_SERVICE, 0x7ffc)
            ]
        );
        assert_eq!(memory.read32(0x7ffc), 0x2000);
        assert_eq!(run.attempts.len(), 1);
        assert_eq!(run.attempts[0].name, b"anim/dep.pak");
    }

    #[test]
    fn null_list_bypasses_fallback_mount() {
        let mut memory = Memory::default();
        put_name(&mut memory, 1, 0x2100, b"fallback.pak");
        let mut guest = Guest {
            list: 0,
            calls: Vec::new(),
        };
        let mut registers = registers();

        let run = run_guest_prefix(&mut memory, &mut guest, &mut registers, 0x64);

        assert_eq!(guest.calls, [(DEPENDENCY_LIST_SERVICE, 0x8000)]);
        assert!(run.attempts.is_empty());
    }
}
