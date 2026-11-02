// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 0 guest-boundary check for the memory-unit reset at VA 00190A80.
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu},
    memory::GuestMemory,
};
use tsfp_ui::call_leaves::reset_memory_unit_globals;

#[path = "support/sparse_memory.rs"]
mod sparse_memory;
use sparse_memory::SparseMemory as Memory;

#[derive(Default)]
struct Calls(Vec<(u32, u32, u32)>);

impl Callee<Memory> for Calls {
    fn call(&mut self, memory: &mut Memory, target: u32, cpu: &mut Cpu) {
        self.0
            .push((target, memory.read32(cpu.esp), memory.read32(cpu.esp + 4)));
    }
}

#[test]
fn resets_selection_then_clears_pending_action() {
    let mut memory = Memory::default();
    let mut calls = Calls::default();
    let mut cpu = Cpu {
        esp: 0x9000,
        ..Cpu::default()
    };
    memory.write32(0x007b_978c, 0xdead_beef);
    memory.write32(0x007b_9794, 7);
    memory.write32(0x007b_9798, 3);

    reset_memory_unit_globals(&mut calls, &mut memory, &mut cpu);

    assert_eq!(calls.0, [(0x0008_27b0, 0x0019_0a87, 0)]);
    assert_eq!(memory.read32(0x007b_978c), 0);
    assert_eq!(memory.read32(0x007b_9794), 7);
    assert_eq!(memory.read32(0x007b_9798), 3);
    assert_eq!(cpu.esp, 0x9000);
}
