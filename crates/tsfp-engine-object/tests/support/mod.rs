// SPDX-License-Identifier: GPL-3.0-or-later
//! Sparse memory and stack setup shared by the wave smoke suites.
use std::collections::HashMap;
use tsfp_engine_object::closure::{callee::Cpu, memory::GuestMemory};

#[derive(Default)]
pub struct Mem(HashMap<u32, u8>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}

pub fn setup(args: &[u32]) -> (Mem, Cpu) {
    let mut m = Mem::default();
    let cpu = Cpu {
        esp: 0x9000,
        ..Cpu::default()
    };
    for (i, a) in args.iter().enumerate() {
        m.write32(0x9004 + 4 * i as u32, *a);
    }
    (m, cpu)
}
