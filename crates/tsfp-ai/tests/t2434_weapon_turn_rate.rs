// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 WEAK differential: fabricated pointer tables against a register-level transcription
//! of REA's 0x59D50 instructions. This is not an original runtime or independent oracle.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
    rea_ai_runtime,
};

const ACTOR: u32 = 0x10000;
const RECORD: u32 = 0x20000;
const INDEX_TABLE: u32 = 0x30000;
const ROWS: u32 = 0x40000;
const STACK: u32 = 0x70000;
const TABLE_POINTER: u32 = 0x7e3954;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<BTreeMap<u32, u8>>>);

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
}

impl Memory {
    fn write32(&mut self, address: u32, value: u32) {
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.write8(address.wrapping_add(i as u32), byte);
        }
    }
    fn snapshot(&self) -> BTreeMap<u32, u8> {
        self.0.borrow().clone()
    }
}

fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 0x1111,
        ecx: 0x2222,
        edx: 0x3333,
        esp: STACK,
        ebx: 0x4444,
        ebp: 0x5555,
        esi: 0x6666,
        edi: 0x7777,
        eflags: 0x202,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 3,
            control: 0x27f,
        },
        xmm: [Xmm::scalar(0x8765_4321); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, address: u32, _: &mut ExtRegisters) {
        panic!("unexpected guest call {address:#x}");
    }
}

fn seed(seed: u32) -> (Memory, u32) {
    let index = seed % 8;
    let row = ROWS + index * 0x40;
    let bits = ((seed.wrapping_mul(0x9e37_79b9) as i32) as f32 / 1024.0).to_bits();
    let mut memory = Memory::default();
    memory.write32(STACK + 4, ACTOR);
    memory.write32(ACTOR + 4, RECORD);
    memory.write32(RECORD + 0xd8, index);
    memory.write32(TABLE_POINTER, INDEX_TABLE);
    memory.write32(INDEX_TABLE + index * 4, row);
    memory.write32(row + 0x18, bits);
    (memory, bits)
}

/// The byte-level interpretation of the measured MOV/MOV/MOV/MOV/FLD sequence.
fn rea_reference(memory: &Memory) -> f32 {
    let actor = memory.read32(STACK + 4);
    let record = memory.read32(actor + 4);
    let index = memory.read32(record + 0xd8);
    let rows = memory.read32(TABLE_POINTER);
    let row = memory.read32(rows.wrapping_add(index.wrapping_mul(4)));
    f32::from_bits(memory.read32(row + 0x18))
}

fn run_adapter(memory: &mut Memory, regs: &mut ExtRegisters) -> f32 {
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(0x59d50, true));
    assert!(rea_ai_runtime::run(0x59d50, memory, &mut NoCalls, regs));
    regs.fpu.st(0) as f32
}

#[test]
fn tier1_weak_register_transcription_differential_3000_fabricated_tables() {
    for seed_value in 0..3000 {
        let (mut memory, expected) = seed(seed_value);
        let before_memory = memory.snapshot();
        let mut regs = registers();
        let before = regs;
        let actual = run_adapter(&mut memory, &mut regs);
        assert_eq!(actual, f32::from_bits(expected), "seed {seed_value}");
        assert_eq!(
            regs.fpu.top,
            (before.fpu.top + 7) & 7,
            "x87 push, seed {seed_value}"
        );
        assert_eq!(
            regs.fpu.st(1),
            before.fpu.st(0),
            "preserve prior ST0, seed {seed_value}"
        );
        assert_eq!(regs.eax, before.eax, "no EAX result, seed {seed_value}");
        assert_eq!(
            memory.snapshot(),
            before_memory,
            "read-only leaf, seed {seed_value}"
        );
        assert_eq!(
            regs.fpu.st(0) as f32,
            rea_reference(&memory),
            "reference, seed {seed_value}"
        );
    }
}

#[test]
fn mutation_sentinels_follow_each_pointer_and_field_edge() {
    let (mut memory, expected) = seed(5);
    assert_eq!(rea_reference(&memory).to_bits(), expected);

    let mut regs = registers();
    assert_eq!(
        run_adapter(&mut memory, &mut regs),
        f32::from_bits(expected)
    );

    memory.write32(RECORD + 0xd8, 6);
    let alternate = ROWS + 6 * 0x40;
    memory.write32(INDEX_TABLE + 6 * 4, alternate);
    memory.write32(alternate + 0x18, 18.25f32.to_bits());
    let mut regs = registers();
    assert_eq!(run_adapter(&mut memory, &mut regs), 18.25);

    memory.write32(TABLE_POINTER, INDEX_TABLE + 0x100);
    memory.write32(INDEX_TABLE + 0x100 + 6 * 4, alternate + 0x1000);
    memory.write32(alternate + 0x1000 + 0x18, (-9.5f32).to_bits());
    let mut regs = registers();
    assert_eq!(run_adapter(&mut memory, &mut regs), -9.5);

    memory.write32(alternate + 0x1000 + 0x18, 0x1234_5678);
    let mut regs = registers();
    assert_eq!(run_adapter(&mut memory, &mut regs).to_bits(), 0x1234_5678);
}
