// SPDX-License-Identifier: GPL-3.0-or-later
use crate::memory::GuestMemory;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-core/src/leaf.rs"]
pub mod integer;

pub fn count_until_negative<M: GuestMemory>(m: &M, object: u32) -> u32 {
    let mut record = m.read32(object.wrapping_add(0x10));
    let mut count = 0u32;
    while (m.read32(record) as i32) >= 0 {
        record = record.wrapping_add(0x10);
        count = count.wrapping_add(1);
    }
    count
}
pub fn first_node_data<M: GuestMemory>(m: &M, list: u32) -> u32 {
    if m.read32(list) == 0 {
        0
    } else {
        m.read32(list.wrapping_add(4)).wrapping_add(4)
    }
}
