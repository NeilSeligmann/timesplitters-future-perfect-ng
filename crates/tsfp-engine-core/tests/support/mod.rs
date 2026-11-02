// SPDX-License-Identifier: GPL-3.0-or-later
//! Private sparse memory for engine-core smoke tests; missing bytes read as zero.
use std::collections::HashMap;
use tsfp_engine_object::closure::memory::GuestMemory;

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
