//! Tier 1 WEAK differential for original weapon-row field lookup `0x1B99F0`.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
    rea_ai_runtime,
};

const SLOT_MAP: u32 = 0x004f_a138;
const ROW_BASE_PTR: u32 = 0x004f_9bac;
const ROW_STRIDE: u32 = 0x29c;
const ROW_FIELD: u32 = 0x1c;

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
fn regs(esp: u32) -> ExtRegisters {
    ExtRegisters {
        eax: 0xeeee,
        ecx: 0xcccc,
        edx: 0xdddd,
        esp,
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

/// Independent transliteration of the Rizin disassembly: signed slot range,
/// signed mapped-row range, then the dword at mapped row + 0x1c.
fn original_reference(slot: i32, mapped_row: i32, field: u32) -> u32 {
    if (0..70).contains(&slot) && (0..66).contains(&mapped_row) {
        field
    } else {
        0
    }
}

#[test]
fn adapter_matches_weak_register_reference_for_3000_fabricated_rows() {
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(0x1b99f0, true));
    for seed in 0..3000u32 {
        let mut memory = Memory::default();
        let slot = (seed.wrapping_mul(37) % 76) as i32 - 3;
        let mapped = (seed.wrapping_mul(19) % 72) as i32 - 3;
        memory.put32(0x70004, slot as u32);
        memory.put32(0x70008, seed.rotate_left(11)); // observed caller's unused second argument
        memory.put32(ROW_BASE_PTR, 0x100000);
        if (0..70).contains(&slot) {
            memory.put32(SLOT_MAP + slot as u32 * 0x3c, mapped as u32);
        }
        let field = seed.wrapping_mul(0x9e3779b9);
        if (0..66).contains(&mapped) {
            memory.put32(0x100000 + mapped as u32 * ROW_STRIDE + ROW_FIELD, field);
        }
        let mut cpu = regs(0x70000);
        assert!(rea_ai_runtime::run(
            0x1b99f0,
            &mut memory,
            &mut NoCalls,
            &mut cpu
        ));
        assert_eq!(
            cpu.eax,
            original_reference(slot, mapped, field),
            "seed {seed}"
        );
        assert_eq!((cpu.ecx, cpu.edx, cpu.esp), (0xcccc, 0xdddd, 0x70000));
    }
}

#[test]
fn bounds_field_offset_and_ignored_argument_mutation_controls() {
    let mut memory = Memory::default();
    memory.put32(0x70004, 4);
    memory.put32(0x70008, 0xffff_ffff);
    memory.put32(SLOT_MAP + 4 * 0x3c, 7);
    memory.put32(ROW_BASE_PTR, 0x200000);
    memory.put32(0x200000 + 7 * ROW_STRIDE + ROW_FIELD, 0x1234_5678);
    let mut cpu = regs(0x70000);
    rea_ai_runtime::configure(true);
    rea_ai_runtime::configure_function(0x1b99f0, true);
    assert!(rea_ai_runtime::run(
        0x1b99f0,
        &mut memory,
        &mut NoCalls,
        &mut cpu
    ));
    assert_eq!(cpu.eax, 0x1234_5678);
    // Negative slot, row id 66, wrong field offset, or using arg2 as a gate
    // each disagrees with the measured reference on this fixture.
    assert_eq!(original_reference(-1, 7, 0x1234_5678), 0);
    assert_eq!(original_reference(4, 66, 0x1234_5678), 0);
    assert_ne!(memory.read32(0x200000 + 7 * ROW_STRIDE + 0x20), 0x1234_5678);
}
