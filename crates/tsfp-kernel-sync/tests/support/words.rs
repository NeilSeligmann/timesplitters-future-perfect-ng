// SPDX-License-Identifier: GPL-3.0-or-later
//! Private word reader shared by critical-section tests and the oracle driver.
use std::collections::BTreeMap;

pub fn read_word(words: &BTreeMap<u32, u32>, address: u32) -> Option<u32> {
    words.get(&address).copied()
}
