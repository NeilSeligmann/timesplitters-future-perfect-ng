// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-lane-c2r/lib
//! Extracted capture Tier 2 roots from the T2049 c2r batches.
#![allow(unused_assignments, unused_mut, unused_variables, unused_parens, clippy::all)]
use super::rt::*;
use crate::state_wire as gs;

pub fn game_state_table_code8_set_return_old_apply<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.push(m, r.ebx);
    r.push(m, 8);
    r.call(guest, 0x21440);
    r.ebx = set_lo8(r.ebx, (r.eax as u8) as u32);
    r.call(guest, 0x22ff0);
    r.eax = m.read8(r.esp.wrapping_add(0xc)) as u8 as u32;
    r.push(m, r.eax);
    r.push(m, 8);
    r.call(guest, 0x21450);
    r.release_stack(0xc);
    r.call(guest, 0x214c0);
    r.eax = set_lo8(r.eax, (r.ebx as u8) as u32);
    r.ebx = r.pop(m);
}

pub const ROOTS: [u32; 1] = [0x23270];

#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0x23270 => { game_state_table_code8_set_return_old_apply(m, guest, regs); true },
        _ => false,
    }
}
