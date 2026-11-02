// SPDX-License-Identifier: GPL-3.0-or-later
use crate::memory::GuestMemory;
/// 00059D70: value at the pointed animation record's +0xd4.
pub fn record_key<M: GuestMemory>(m: &M, object: u32) -> u32 {
    m.read32(m.read32(object.wrapping_add(4)).wrapping_add(0xd4))
}
