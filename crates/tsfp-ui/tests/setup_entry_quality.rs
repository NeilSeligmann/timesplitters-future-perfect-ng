// SPDX-License-Identifier: GPL-3.0-or-later
//! T1960 Tier 0 synthetic check for the selected setup entry's guest boundary.
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu},
    memory::GuestMemory,
};
use tsfp_ui::call_leaves::setup_entry_available;

#[path = "support/sparse_memory.rs"]
mod sparse_memory;
use sparse_memory::SparseMemory as Mem;

#[derive(Default)]
struct Calls {
    targets: Vec<(u32, u32)>,
    session: u32,
}

impl Callee<Mem> for Calls {
    fn call(&mut self, memory: &mut Mem, target: u32, cpu: &mut Cpu) {
        self.targets.push((target, memory.read32(cpu.esp)));
        cpu.eax = if target == 0x0006_9640 {
            0x1000
        } else {
            self.session
        };
    }
}

#[test]
fn selected_setup_requires_unlock_then_local_mode_or_session() {
    let mut memory = Mem::default();
    let mut cpu = Cpu {
        esp: 0x9000,
        ..Cpu::default()
    };
    let mut calls = Calls::default();
    memory.write8(0x0078_adad, 2);
    memory.write8(0x0078_adae, 3);
    let kind = 0x004d_1f84 + 3 * 0x20;
    let unlock = 0x1000 + 2 * 20 + 0x24;

    memory.write32(kind, 2);
    setup_entry_available(&mut calls, &mut memory, &mut cpu);
    assert_eq!(cpu.eax, 0);
    assert_eq!(calls.targets, [(0x0006_9640, 0x002b_e9b5)]);
    assert_eq!(cpu.esp, 0x9000);

    memory.write32(unlock, 1);
    calls.targets.clear();
    calls.session = 7;
    setup_entry_available(&mut calls, &mut memory, &mut cpu);
    assert_eq!(cpu.eax, 1);
    assert_eq!(
        calls.targets,
        [(0x0006_9640, 0x002b_e9b5), (0x0035_33f0, 0x002b_e9f0)]
    );

    memory.write32(0x0079_094c, 0x65);
    calls.targets.clear();
    setup_entry_available(&mut calls, &mut memory, &mut cpu);
    assert_eq!(cpu.eax, 1);
    assert_eq!(calls.targets, [(0x0006_9640, 0x002b_e9b5)]);

    memory.write32(unlock, 0);
    memory.write32(kind, 1);
    calls.targets.clear();
    setup_entry_available(&mut calls, &mut memory, &mut cpu);
    assert_eq!(cpu.eax, 1);
    assert_eq!(calls.targets, [(0x0006_9640, 0x002b_e9b5)]);
}
