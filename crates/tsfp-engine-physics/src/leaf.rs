// SPDX-License-Identifier: GPL-3.0-or-later
use crate::memory::GuestMemory;
/// 000DC290: table entry word, with the -1 sentinel returning zero.
pub fn entry_word<M: GuestMemory>(m: &M, index: u32) -> u32 {
    if index == u32::MAX { 0 } else { m.read32(index.wrapping_shl(6).wrapping_add(0x4deb40)) }
}
