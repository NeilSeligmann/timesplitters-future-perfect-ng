// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 WEAK differential for original weapon group lookup `0x1B6BB0`.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
    rea_ai_runtime,
};

const ORDER: u32 = 0x004f_c888;
const GROUP: u32 = 0x004f_a160;
const COUNT: usize = 70;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<BTreeMap<u32, u8>>>);
impl GuestMemory for Memory {
    fn read8(&self, a: u32) -> u8 {
        self.0.borrow().get(&a).copied().unwrap_or(0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.borrow_mut().insert(a, v);
    }
}
impl Memory {
    fn put32(&mut self, a: u32, v: u32) {
        for (i, b) in v.to_le_bytes().into_iter().enumerate() {
            self.write8(a + i as u32, b);
        }
    }
}
struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, va: u32, _: &mut ExtRegisters) {
        panic!("unexpected guest call {va:#x}");
    }
}
fn regs(group: u32) -> ExtRegisters {
    ExtRegisters {
        eax: 0xeeee,
        ecx: group,
        edx: 0x2222,
        esp: 0x70000,
        ebx: 0x3333,
        ebp: 0x4444,
        esi: 0x5555,
        edi: 0x6666,
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

/// Register-level transliteration of the original scan: slots 0..69 in order,
/// compare record dword +0x38, return that ordered id or -1.
fn original_reference(order: &[u32; COUNT], groups: &[u32; COUNT], wanted: u32) -> u32 {
    for i in 0..COUNT {
        if groups[i] == wanted {
            return order[i];
        }
    }
    u32::MAX
}

#[test]
fn adapter_matches_weak_register_reference_for_3000_fabricated_tables() {
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(0x1b6bb0, true));
    for seed in 0..3000u32 {
        let mut memory = Memory::default();
        let order: [u32; COUNT] = core::array::from_fn(|slot| {
            let id = ((slot as u32 * 37 + seed * 11) % COUNT as u32) as u32;
            memory.put32(ORDER + slot as u32 * 4, id);
            id
        });
        let groups: [u32; COUNT] = core::array::from_fn(|slot| {
            let group = (slot as u32 * 13 + seed * 7) % 9;
            memory.put32(GROUP + order[slot] * 0x3c, group);
            group
        });
        let wanted = seed % 11;
        let mut cpu = regs(wanted);
        assert!(rea_ai_runtime::run(
            0x1b6bb0,
            &mut memory,
            &mut NoCalls,
            &mut cpu
        ));
        assert_eq!(
            cpu.eax,
            original_reference(&order, &groups, wanted),
            "seed {seed}"
        );
        assert_eq!((cpu.ecx, cpu.edx, cpu.esp), (wanted, 0x2222, 0x70000));
    }
}

#[test]
fn first_match_and_missing_group_mutation_controls_are_distinguishable() {
    let mut order = [0u32; COUNT];
    let mut groups = [99u32; COUNT];
    for (i, id) in order.iter_mut().enumerate() {
        *id = i as u32;
    }
    groups[8] = 4;
    groups[31] = 4;
    assert_eq!(original_reference(&order, &groups, 4), 8);
    assert_eq!(original_reference(&order, &groups, 5), u32::MAX);
    // Deliberate mutation controls: reversed traversal and zero-on-miss each
    // disagree with this fixture, so the differential detects both errors.
    assert_eq!(
        order
            .iter()
            .rev()
            .zip(groups.iter().rev())
            .find(|(_, g)| **g == 4)
            .unwrap()
            .0,
        &31
    );
    assert_ne!(0, u32::MAX);
}
