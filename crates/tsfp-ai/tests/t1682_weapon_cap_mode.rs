// SPDX-License-Identifier: GPL-3.0-or-later
//! T1682 Q7 Tier 1 WEAK adapter differential: fabricated memory against an
//! independent transliteration of the observed REA instructions.
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
    rea_ai_runtime,
};

const ESP: u32 = 0x70000;
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
    fn word(&mut self, a: u32, v: u32) {
        for n in 0..4 {
            self.write8(a + n, (v >> (n * 8)) as u8);
        }
    }
}
#[derive(Default)]
struct Guest {
    calls: Vec<u32>,
    result: u32,
}
impl GuestCall for Guest {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        self.calls.push(va);
        regs.eax = self.result;
    }
}
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: ESP,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
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
fn invoke(m: &mut Memory, g: &mut Guest, flag: u32) -> u32 {
    m.word(ESP + 4, 0x2000);
    m.word(ESP + 8, 0x3000);
    m.word(ESP + 12, flag);
    let mut r = regs();
    assert!(rea_ai_runtime::run(0x1b6db0, m, g, &mut r));
    r.eax
}
// Independent register-contract reference: signed index gate; selected cap;
// sum sign-extended words; otherwise use the helper result.
fn reference(
    index: i32,
    flag: u32,
    cap8c: i32,
    cap90: i32,
    owner: i16,
    held: i16,
    fallback: u32,
) -> u32 {
    if index >= 0 && i32::from(owner) + i32::from(held) >= if flag == 0 { cap8c } else { cap90 } {
        1
    } else {
        fallback
    }
}
#[test]
fn rea_weapon_cap_mode_matches_instruction_transliteration_and_mutations() {
    rea_ai_runtime::configure(true);
    rea_ai_runtime::configure_function(0x1b6db0, true);
    let mut m = Memory::default();
    let mut g = Guest {
        result: 1,
        ..Guest::default()
    };
    let owner = 0x2000;
    let state = 0x3000;
    let def = 0x4000;
    let table = 0x5000;
    m.word(0x4f9bac, table);
    m.word(table, def);
    m.word(state + 0x30, 0);
    m.word(def + 0x14, 0);
    m.word(owner + 0x61e, 4);
    m.word(state + 0x38, 5);
    m.word(def + 0x8c, 9);
    m.word(def + 0x90, 20);
    let mut seen = 0;
    for flag in [0, 1, u32::MAX] {
        for a in -2i16..=2 {
            for b in -2i16..=2 {
                m.word(owner + 0x61e, a as i32 as u32);
                m.word(state + 0x38, b as i32 as u32);
                let actual = invoke(&mut m, &mut g, flag);
                let expected = reference(0, flag, 9, 20, a, b, 1);
                assert_eq!(actual, expected);
                seen += 1;
            }
        }
    }
    assert_eq!(seen, 75);
    // Independent mutations: alternate cap offset, zero/nonzero mode test,
    // signed-word loads and >= threshold each have a killing boundary case.
    m.word(owner + 0x61e, 4);
    m.word(state + 0x38, 5);
    m.word(def + 0x8c, 20);
    m.word(def + 0x90, 9);
    m.word(def + 0x8c, 10);
    assert_eq!(invoke(&mut m, &mut g, 0), 1);
    m.word(def + 0x8c, 20);
    assert_eq!(invoke(&mut m, &mut g, 1), 1);
    m.word(def + 0x8c, 9);
    m.word(def + 0x90, 20);
    m.word(owner + 0x61e, 0xffff);
    m.word(state + 0x38, 2);
    m.word(def + 0x8c, 1);
    assert_eq!(invoke(&mut m, &mut g, 0), 1); // -1 + 2 reaches cap exactly.
    rea_ai_runtime::configure(false);
}
