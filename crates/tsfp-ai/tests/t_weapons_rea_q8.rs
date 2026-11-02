//! T1682 Q8 Tier 1 WEAK adapter differential on fabricated memory.
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
    rea_ai_runtime,
};

const ESP: u32 = 0x70000;
const OWNER: u32 = 0x20000;
const STATE: u32 = 0x30000;
const DEF: u32 = 0x40000;
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
    fn put16(&mut self, a: u32, v: i16) {
        for (i, b) in v.to_le_bytes().into_iter().enumerate() {
            self.write8(a + i as u32, b);
        }
    }
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

/// Independent register-level transliteration of the observed REA instruction
/// contract. This is deliberately a weak reference, not execution of the XBE.
fn reference(
    mode: u32,
    index: i32,
    base_kind: u32,
    alt_kind: u32,
    marker: i32,
    mode_limit: i32,
    def_limit: i32,
    owner: i16,
    state: i16,
) -> bool {
    if index < 0 {
        return false;
    }
    let _kind = if mode == 1 && alt_kind != 0 {
        alt_kind
    } else {
        base_kind
    };
    let limit = if marker >= 0 { mode_limit } else { def_limit };
    i32::from(owner) + i32::from(state) >= limit
}

#[test]
fn fallback_adapter_matches_weak_rea_transliteration_and_mutations() {
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(0x1b6c90, true));
    for seed in 0..3000u32 {
        let mut m = Memory::default();
        let mode = seed % 3;
        let index = if seed % 17 == 0 {
            -1
        } else {
            (seed % 9) as i32
        };
        let base_kind = (seed.rotate_left(5) % 8) + 1;
        let alt_kind = if seed % 4 == 0 {
            0
        } else {
            (seed.rotate_left(11) % 8) + 1
        };
        let marker = if seed % 2 == 0 { -1 } else { 0 };
        let mode_limit = (seed % 21) as i32 - 10;
        let def_limit = (seed % 17) as i32 - 8;
        let owner_rounds = (seed as i16).wrapping_mul(97);
        let state_rounds = (seed.rotate_left(9) as i16).wrapping_mul(53);
        m.put32(ESP + 4, OWNER);
        m.put32(ESP + 8, STATE);
        m.put32(ESP + 12, mode);
        m.put32(STATE + 0x30, index as u32);
        m.put32(0x4f9bac, DEF - ((index.max(0) as u32) * 0x29c));
        m.put32(DEF + 0x14, base_kind);
        m.put32(DEF + 0x1c, alt_kind);
        m.put32(DEF + 0x68, def_limit as u32);
        let row = OWNER + mode * 0x7c + 0x68;
        m.put32(row, marker as u32);
        m.put32(row + if mode == 0 { 0x24 } else { 0x28 }, mode_limit as u32);
        let kind = if mode == 1 && alt_kind != 0 {
            alt_kind
        } else {
            base_kind
        };
        m.put16(OWNER + kind * 2 + 0x61e, owner_rounds);
        m.put16(STATE + kind * 2 + 0x38, state_rounds);
        let expected = reference(
            mode,
            index,
            base_kind,
            alt_kind,
            marker,
            mode_limit,
            def_limit,
            owner_rounds,
            state_rounds,
        );
        let before = m.0.borrow().clone();
        let mut r = regs();
        assert!(rea_ai_runtime::run(0x1b6c90, &mut m, &mut NoCalls, &mut r));
        assert_eq!(r.eax, expected as u32, "seed {seed}");
        assert_eq!(*m.0.borrow(), before, "adapter must not write memory");
    }
    // Mutation-sensitive branch witnesses for equality, fallback, exact mode 1,
    // nonzero alternate id and the negative definition sentinel.
    assert!(reference(0, 0, 2, 7, -1, 99, 4, 5, -1));
    assert!(!reference(0, 0, 2, 7, -1, 99, 5, 5, -1));
    assert!(reference(1, 0, 2, 7, 0, 4, 99, 2, 2));
    assert!(!reference(2, -1, 2, 7, 0, 0, 0, 0, 0));
}

#[test]
fn directed_mutations_distinguish_mode_kind_marker_and_equality() {
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(0x1b6c90, true));
    let run = |mode: u32, marker: i32, cap: i32, rounds: i16| {
        let mut m = Memory::default();
        m.put32(ESP + 4, OWNER);
        m.put32(ESP + 8, STATE);
        m.put32(ESP + 12, mode);
        m.put32(STATE + 0x30, 0);
        m.put32(0x4f9bac, DEF);
        m.put32(DEF + 0x14, 2);
        m.put32(DEF + 0x1c, 7);
        m.put32(DEF + 0x68, cap as u32);
        let row = OWNER + mode * 0x7c + 0x68;
        m.put32(row, marker as u32);
        m.put32(row + if mode == 0 { 0x24 } else { 0x28 }, cap as u32);
        let kind = if mode == 1 { 7 } else { 2 };
        m.put16(OWNER + kind * 2 + 0x61e, rounds);
        m.put16(STATE + kind * 2 + 0x38, 0);
        let mut r = regs();
        assert!(rea_ai_runtime::run(0x1b6c90, &mut m, &mut NoCalls, &mut r));
        r.eax
    };
    assert_eq!(run(1, -1, 5, 5), 1); // alternate kind and definition fallback, equality
    assert_eq!(run(0, -1, 6, 5), 0); // mode zero uses base kind and +0x68 fallback
    assert_eq!(run(2, 0, 5, 5), 1); // nonzero modes select +0x28 row without kind substitution
}
