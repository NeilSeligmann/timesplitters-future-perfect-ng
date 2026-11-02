// SPDX-License-Identifier: GPL-3.0-or-later
//! T1848 wave 4 render leaves, ports of call-free C replacements of `src/game`. See
//! docs/t1848-rust-wave4.md.
use crate::{gprs::Gprs, memory::GuestMemory};

const WEATHER_FLAG: u32 = 0x0076_5e58;
const RELOCATE_FLAG: u32 = 0x1000_0000;
const RELOCATE_SKIP: u32 = 0x4000_0000;
const TERMINATOR: u32 = u32::MAX;

/// 003443F0 game_render_is_weather_pass_flag_set. cdecl(0): ECX = [765E58], EAX = ECX != 0.
// portreg: va=0x003443f0 tier=1
pub fn is_weather_pass_flag_set<M: GuestMemory>(m: &M, entry: Gprs) -> Gprs {
    let ecx = m.read32(WEATHER_FLAG);
    Gprs { eax: (ecx != 0) as u32, ecx, edx: entry.edx }
}

// The body lives in vertex_format.rs, which the lane build also includes at its crate root (T1976).
#[path = "vertex_format.rs"]
mod vertex_format_impl;
pub use vertex_format_impl::vertex_format_size;

/// 0008E3E0 game_render_relocate_flagged_records. cdecl(2): walks 16-byte records from `table`
/// until a record whose first word is 0xFFFFFFFF. A non-skip, nonzero first word of a record
/// whose word at +12 has bit 0x10000000 gets `delta` added. The terminator is re-read after
/// each record, in C order, so a store into a later record is seen. No iteration cap.
// portreg: va=0x0008e3e0 tier=1
pub fn relocate_flagged_records<M: GuestMemory>(m: &mut M, table: u32, delta: u32, entry: Gprs) -> Gprs {
    if m.read32(table) == TERMINATOR {
        return Gprs { eax: entry.eax, ecx: table, edx: entry.edx };
    }
    let mut record = table;
    loop {
        let mut word = m.read32(record);
        if word != RELOCATE_SKIP && m.read32(record.wrapping_add(12)) & RELOCATE_FLAG != 0 {
            if word != 0 {
                word = word.wrapping_add(delta);
            }
            m.write32(record, word);
        }
        let next = m.read32(record.wrapping_add(16));
        record = record.wrapping_add(16);
        if next == TERMINATOR {
            return Gprs { eax: next, ecx: record, edx: record };
        }
    }
}
