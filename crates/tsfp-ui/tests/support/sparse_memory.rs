// SPDX-License-Identifier: GPL-3.0-or-later
//! Private fixture for UI tests using the object closure memory contract.
use std::collections::HashMap;

use tsfp_engine_object::closure::memory::GuestMemory;

#[derive(Default)]
pub struct SparseMemory(HashMap<u32, u8>);

impl GuestMemory for SparseMemory {
    fn read8(&self, address: u32) -> u8 {
        *self.0.get(&address).unwrap_or(&0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}
