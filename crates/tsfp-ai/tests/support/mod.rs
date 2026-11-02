// SPDX-License-Identifier: GPL-3.0-or-later
//! Private sparse byte storage shared by AI unit and integration tests.
//! Each test crate supplies its existing `crate::memory` trait alias.
use crate::memory::GuestMemory;
use std::collections::BTreeMap;

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Memory(pub BTreeMap<u32, u8>);

impl GuestMemory for Memory {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}
