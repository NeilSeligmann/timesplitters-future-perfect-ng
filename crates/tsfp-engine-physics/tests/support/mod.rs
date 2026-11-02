// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded zero-fixture memory, call recording and register state shared by physics suites.
use std::{cell::Cell, collections::BTreeMap};
use tsfp_engine_physics::{
    boundary::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
};
#[derive(Default)]
pub struct Mem {
    pub words: BTreeMap<u32, u8>,
    pub reads: Cell<usize>,
}
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        let n = self.reads.get() + 1;
        self.reads.set(n);
        assert!(n < 20000, "fixture budget exceeded");
        *self.words.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.words.insert(a, v);
        assert!(self.words.len() < 20000, "write budget exceeded");
    }
}
#[derive(Default)]
pub struct Calls(pub Vec<u32>);
impl GuestCall for Calls {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        self.0.push(va);
        assert!(self.0.len() < 1000);
        r.eax = 0;
    }
}
pub fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: 0x100000,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.; 8],
            top: 0,
            control: 0x37f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
