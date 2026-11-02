// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differential for the full-DWORD awareness-cache fallback in `0x15E120`.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use tsfp_ai::awareness::AwarenessCache;
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::ai_behavior::game_ai_target_candidate_check_pair_table_74c368_xz_view_cone_dot_and_mode_a_first_object as lifted_eligibility,
    memory::GuestMemory,
};

const EXTENSION: u32 = 0x2000;
const TARGET: u32 = 0x3000;
const STACK: u32 = 0x70000;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<HashMap<u32, u8>>>);

impl Memory {
    fn put32(&mut self, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.0.borrow_mut().insert(address + offset as u32, byte);
        }
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

struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, address: u32, _: &mut ExtRegisters) {
        panic!("fallback fixture unexpectedly called {address:#x}")
    }
}

fn compare_raw_cache(seed: u32, raw: u32) {
    let mut memory = Memory::default();
    memory.put32(0x7de458, 0); // global flag does not take the early rejection branch
    memory.put32(STACK + 4, EXTENSION);
    memory.put32(STACK + 8, seed & 1);
    memory.put32(EXTENSION, 0x5000);
    memory.put32(EXTENSION + 4, TARGET);
    memory.put32(EXTENSION + 0xadc, raw);
    memory.put32(TARGET + 0x20, 0); // target kind != the special kind 8 path

    let mut regs = ExtRegisters {
        eax: 0,
        ecx: seed.rotate_left(5),
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
    lifted_eligibility(&mut memory, &mut NoCalls, &mut regs);

    let cache = AwarenessCache::from_raw(raw);
    assert_eq!(cache.raw_result(), regs.eax, "seed={seed} raw={raw:#010x}");
    assert_eq!(cache.full_dword_nonzero(), regs.eax != 0, "seed={seed}");
}

#[test]
fn tier1_4096_full_dword_cache_reads_match_lifted_eligibility_fallback() {
    for seed in 0..4096u32 {
        let raw = match seed % 8 {
            0 => 0,
            1 => 1,
            2 => 0x100,
            3 => 0x8000_0000,
            4 => 0xffff_ff00,
            _ => seed.wrapping_mul(0x9e37_79b9).rotate_left(11),
        };
        compare_raw_cache(seed, raw);
    }
}
