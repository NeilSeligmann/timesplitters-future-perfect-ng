// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differential for the measured 0x15FE00 descriptor-admission path.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use tsfp_ai::awareness::{PerceptionCode, PerceptionResult, descriptor_callback_admission};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::ai_behavior::game_ai_target_visibility_check_four_candidates_then_followup_helper,
    memory::GuestMemory,
};

const ACTOR: u32 = 0x1000;
const EXTENSION: u32 = 0x2000;
const DESCRIPTOR: u32 = 0x3000;
const TARGET: u32 = 0x4000;
const STACK: u32 = 0x70000;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<HashMap<u32, u8>>>);

impl Memory {
    fn put8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
    fn put32(&mut self, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.0.borrow_mut().insert(address + offset as u32, byte);
        }
    }
    fn get32(&self, address: u32) -> u32 {
        u32::from_le_bytes(std::array::from_fn(|i| {
            self.0
                .borrow()
                .get(&(address + i as u32))
                .copied()
                .unwrap_or(0)
        }))
    }
}

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
}

struct EligibilityCall;
impl GuestCall for EligibilityCall {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        assert_eq!(address, 0x15e120, "unexpected descriptor callback");
        regs.eax = 1;
    }
}

fn compare_seed(seed: u32) {
    let flags_54 = seed.rotate_left(9) & !0x200;
    let flags_58 = 1 | (seed as u8 & !1);
    let mut memory = Memory::default();
    memory.put32(ACTOR + 0x7c, EXTENSION);
    memory.put32(EXTENSION + 4, TARGET);
    memory.put32(EXTENSION + 0x9bc, seed);
    memory.put32(EXTENSION + 0xbf0, DESCRIPTOR);
    memory.put32(DESCRIPTOR + 0x54, flags_54);
    memory.put8(DESCRIPTOR + 0x58, flags_58);
    memory.put32(STACK + 4, ACTOR);
    memory.put32(STACK + 8, seed & 1);
    let mut regs = ExtRegisters {
        eax: 0,
        ecx: seed,
        edx: seed.rotate_right(7),
        esp: STACK,
        ebx: seed.rotate_left(13),
        ebp: seed.rotate_left(3),
        esi: seed.rotate_left(17),
        edi: seed.rotate_left(23),
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    };
    game_ai_target_visibility_check_four_candidates_then_followup_helper(
        &mut memory,
        &mut EligibilityCall,
        &mut regs,
    );
    let native = descriptor_callback_admission(flags_54, flags_58, 1)
        .expect("fixture selects the measured descriptor callback path");
    assert_eq!(native, PerceptionResult::Known(PerceptionCode::Geometric));
    assert_eq!(regs.eax, 1, "lifted result seed={seed}");
    assert_eq!(memory.get32(EXTENSION + 4), TARGET);
}

#[test]
fn tier1_2048_seeded_descriptor_callback_admissions_match_lifted_original() {
    for seed in 0..2048 {
        compare_seed(seed);
    }
}
