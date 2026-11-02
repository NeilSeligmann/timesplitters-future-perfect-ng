// SPDX-License-Identifier: GPL-3.0-or-later
//! Capture-only ABI adapter for the entry-flag rule at 0x00162450.
#![no_std]

use tsfp_gameplay::rea_object_flags::{RULE_162450, SLOT_COUNT, SlotWord};
use tsfp_port_runtime::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

const OWNER_POINTER: u32 = 0x0074_c36c;
const OWNER_BASE_OFFSET: u32 = 0x74;
const RECORD_STRIDE: u32 = 0x98;
const ENTRY_ARRAY_POINTER: u32 = 0x007a_2820;
const ENTRY_STRIDE: u32 = 24;
const ENTRY_FLAG_OFFSET: u32 = 0x10;
const SLOT_TABLE: u32 = 0x007a_2d84;
const SLOT_STRIDE: u32 = 0x60;

/// Adapt the original cdecl ABI into the shared typed entry-flag rule.
///
/// The capture-only path reads the captured guest image and never registers this
/// root for live dispatch. Captured boundary cases not in the corpus remain debt.
pub fn set_entry_flag<M: GuestMemory>(memory: &mut M, regs: &mut ExtRegisters) {
    let index = memory.read32(regs.esp.wrapping_add(4));
    let owner_record = memory.read32(OWNER_POINTER);
    let record = memory
        .read32(owner_record.wrapping_add(OWNER_BASE_OFFSET))
        .wrapping_add(index.wrapping_mul(RECORD_STRIDE));
    let entry = memory
        .read32(ENTRY_ARRAY_POINTER)
        .wrapping_add(index.wrapping_mul(ENTRY_STRIDE));
    let mask = memory.read32(record.wrapping_add(RULE_162450.mask_offset));
    let word_offset = match RULE_162450.slot_word {
        SlotWord::First => 0,
        SlotWord::Second => 4,
    };
    let flag = RULE_162450.flag(mask, |slot| {
        memory.read32(
            SLOT_TABLE
                .wrapping_add(slot.wrapping_mul(SLOT_STRIDE))
                .wrapping_add(word_offset),
        )
    });
    memory.write32(entry.wrapping_add(ENTRY_FLAG_OFFSET), flag);

    regs.eax = record;
    regs.ecx = SLOT_COUNT;
    regs.edx = SLOT_COUNT + 2;
}

/// Run this leaf through the generated offline lane interface.
pub fn run<M: GuestMemory, C: GuestCall>(memory: &mut M, _guest: &mut C, regs: &mut ExtRegisters) {
    set_entry_flag(memory, regs);
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);

    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    impl Memory {
        fn put32(&mut self, address: u32, value: u32) {
            for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
                self.write8(address.wrapping_add(offset as u32), byte);
            }
        }
    }

    fn registers(esp: u32) -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: tsfp_port_runtime::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [tsfp_port_runtime::abi::Xmm::default(); 5],
            xmm_hi: [tsfp_port_runtime::abi::Xmm::default(); 3],
        }
    }

    #[test]
    fn entry_flag_uses_first_slot_table_and_preserves_cdecl_outputs() {
        let index = 3;
        let owner_record = 0x2000;
        let records = 0x3000;
        let record = records + index * RECORD_STRIDE;
        let entry_array = 0x4000;
        let entry = entry_array + index * ENTRY_STRIDE;
        let mut memory = Memory::default();
        let mut regs = registers(0x1000);
        memory.put32(regs.esp + 4, index);
        memory.put32(OWNER_POINTER, owner_record);
        memory.put32(owner_record + OWNER_BASE_OFFSET, records);
        memory.put32(ENTRY_ARRAY_POINTER, entry_array);
        memory.put32(record + RULE_162450.mask_offset, (1 << 2) | (1 << 7));
        memory.put32(SLOT_TABLE + 2 * SLOT_STRIDE, 0);
        memory.put32(SLOT_TABLE + 7 * SLOT_STRIDE, 0x1234);

        set_entry_flag(&mut memory, &mut regs);

        assert_eq!(memory.read32(entry + ENTRY_FLAG_OFFSET), 1);
        assert_eq!(regs.eax, record);
        assert_eq!(regs.ecx, SLOT_COUNT);
        assert_eq!(regs.edx, SLOT_COUNT + 2);
    }

    #[test]
    fn entry_flag_clears_without_a_matching_nonzero_slot() {
        let index = 0;
        let owner_record = 0x2000;
        let records = 0x3000;
        let record = records;
        let entry_array = 0x4000;
        let entry = entry_array;
        let mut memory = Memory::default();
        let mut regs = registers(0x1000);
        memory.put32(regs.esp + 4, index);
        memory.put32(OWNER_POINTER, owner_record);
        memory.put32(owner_record + OWNER_BASE_OFFSET, records);
        memory.put32(ENTRY_ARRAY_POINTER, entry_array);
        memory.put32(record + RULE_162450.mask_offset, 1);
        memory.put32(SLOT_TABLE, 0);
        memory.put32(entry + ENTRY_FLAG_OFFSET, 0xffff_ffff);

        set_entry_flag(&mut memory, &mut regs);

        assert_eq!(memory.read32(entry + ENTRY_FLAG_OFFSET), 0);
    }
}
