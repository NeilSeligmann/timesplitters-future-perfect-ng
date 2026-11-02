// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-lane-c2r/lib
//! Extracted capture Tier 2 roots from the T2049 c2r batches.
#![allow(unused_assignments, unused_mut, unused_variables, unused_parens, clippy::all)]
use super::rt::*;
use crate::state_wire as gs;

pub fn game_table_515ff8_find_entry_by_global_79094c<M: GuestMemory>(
    m: &mut M,
    r: &mut ExtRegisters,
) {
    'at_6a567: {
        r.ecx = 0;
        if ((m.read16(0x515ffc) as u32) & 0xffff) != (((r.ecx as u16) as u32) & 0xffff) {
            r.edx = gs::get_level(m);
            r.eax = 0x515ffc;
            loop {
                if m.read32(r.eax.wrapping_add(4)) == r.edx {
                    r.ecx = r.ecx.wrapping_shl(7).wrapping_add(0x515ff8);
                    r.eax = r.ecx;
                    return;
                } else {
                    r.eax = r.eax.wrapping_add(0x80);
                    r.ecx = r.ecx.wrapping_add(1);
                    if ((m.read16(r.eax) as u32) & 0xffff) == (0 & 0xffff) {
                        break 'at_6a567;
                    }
                }
            }
        }
    }
    // 0x6a567
    r.eax = 0;
}

pub const ROOTS: [u32; 1] = [0x6a540];

#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0x6a540 => { game_table_515ff8_find_entry_by_global_79094c(m, regs); true },
        _ => false,
    }
}
