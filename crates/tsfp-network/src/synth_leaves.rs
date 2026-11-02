// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 11: synthesized-domain roots (T1781/T1782/T1783), translated one statement at a
//! time from the proven C replacements of src/game by tools/t1836_c2rs.py (regenerate, do not
//! edit by hand). Equality with the C is MEASURED by crates/tsfp-c-diff wave11 (120,000 cases at
//! O0/O2/O3), equality with the original XBE is INFERRED. Evidence class of the roots:
//! synthesized-domain (docs/t1836-rust-wave11.md). Placement in this crate is INFERRED.
#![allow(
    unused_mut,
    unused_assignments,
    unused_variables,
    dead_code,
    unused_parens,
    unreachable_code
)]
use crate::memory::GuestMemory as Guest;
include!("../../tsfp-engine-memory/src/cpu.rs");

/// 00038f30 cdecl(1). messenger_slot_clear.
pub fn messenger_slot_clear<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.edx = m.read32(0x68C6FCu32);
    {
        let value = c.edi;
        push(m, c, value);
    }
    c.edi = m
        .read32(c.esp.wrapping_add(8u32))
        .wrapping_mul(0x1A0u32)
        .wrapping_add(c.edx);
    c.ecx = 0x68u32;
    c.eax = 0u32;
    rep_stosd(m, c);
    c.edi = pop(m, c);
}

/// 00038f50 cdecl(2). messenger_slot_copy_out.
pub fn messenger_slot_copy_out<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(0x68C6FCu32);
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = m.read32(c.esp.wrapping_add(12u32)).wrapping_mul(0x1A0u32);
    {
        let value = c.edi;
        push(m, c, value);
    }
    c.edi = m.read32(c.esp.wrapping_add(12u32));
    c.esi = c.esi.wrapping_add(c.ecx);
    c.ecx = 0x68u32;
    rep_movsd(m, c);
    c.edi = pop(m, c);
    c.esi = pop(m, c);
}
