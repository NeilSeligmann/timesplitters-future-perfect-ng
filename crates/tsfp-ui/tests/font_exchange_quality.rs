// SPDX-License-Identifier: GPL-3.0-or-later
//! Host-free Tier 0 check of the guest font selection contract.
use std::collections::BTreeMap;

use tsfp_ui::hud_text::{RecordedCalls, font_exchange};
use tsfp_ui::memory::GuestMemory;

#[derive(Default)]
struct SparseMemory(BTreeMap<u32, u8>);

impl GuestMemory for SparseMemory {
    fn read8(&self, address: u32) -> u8 {
        *self.0.get(&address).unwrap_or(&0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

#[test]
fn selects_normal_and_alternate_font_without_changing_call_order() {
    let mut memory = SparseMemory::default();
    let mut calls = RecordedCalls::default();
    memory.write32(0x006b_93b4, 7);
    memory.write32(0x004c_8de8 + 3 * 20, 2);

    assert_eq!(font_exchange(&mut memory, &mut calls, 3), 7);
    assert_eq!(memory.read32(0x006b_93b4), 3);
    assert_eq!(memory.read32(0x006b_93bc), 0x004c_8de8 + 3 * 20);
    assert_eq!(memory.read32(0x006b_93b8), 0x004c_8c28 + 2 * 0x38);

    memory.write32(0x006b_93a8, 1);
    assert_eq!(font_exchange(&mut memory, &mut calls, 0), 3);
    assert_eq!(memory.read32(0x006b_93bc), 0x004c_8de8);
    assert_eq!(memory.read32(0x006b_93b8), 0x004c_8c98);
    assert_eq!(calls.0, vec![(0x0003_f8d0, vec![]), (0x0003_f8d0, vec![])]);
}
