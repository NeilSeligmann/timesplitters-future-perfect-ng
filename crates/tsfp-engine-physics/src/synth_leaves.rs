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

// portreg: va=0x00167d20
/// 00167d20 cdecl(2). collision_shape_records_have_active_or_match_table_51c088_index.
pub fn collision_shape_records_have_active_or_match_table_51c088_index<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    let mut st: u32 = 0;
    'machine: loop {
        match st {
            0 => {
                c.ecx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(1u32)));
                c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
                m.write32(c.esp.wrapping_sub(4u32), c.esi);
                if (c.ecx == 0xFFFFFFFFu32) {
                    c.ecx = m.read32(c.eax);
                    c.edx = 0u32;
                    if ((c.ecx as i32) > 0) {
                        c.esi = c.ecx.wrapping_mul(0xC8u32);
                        c.eax = c.eax.wrapping_sub(c.esi).wrapping_add(0x8Cu32);
                        loop {
                            if (((m.read16(c.eax.wrapping_sub(0x34u32)) as u32) != 0u32)
                                && (m.read32(c.eax) != 0u32))
                            {
                                c.eax = 1u32;
                                {
                                    st = 1;
                                    continue 'machine;
                                }
                            }
                            c.edx = c.edx.wrapping_add(1);
                            c.eax = c.eax.wrapping_add(0xC8u32);
                            if !((c.edx as i32) < (c.ecx as i32)) {
                                break;
                            }
                        }
                    }
                } else {
                    c.edx = m.read32(c.eax);
                    c.esi = m.read32(c.ecx.wrapping_mul(8u32).wrapping_add(0x51C08Cu32));
                    c.edx = c.edx.wrapping_mul(0xC8u32);
                    c.eax = c.eax.wrapping_sub(c.edx);
                    c.edx = m.read32(c.eax.wrapping_add(0x8Cu32));
                    if (c.edx == c.esi) {
                        c.eax = ((((m.read16(c.eax.wrapping_add(0x58u32)) as u32) as i16) as i32)
                            as u32);
                        if (c.eax == m.read32(c.ecx.wrapping_mul(8u32).wrapping_add(0x51C088u32))) {
                            c.eax = 1u32;
                            {
                                st = 1;
                                continue 'machine;
                            }
                        }
                    }
                }
                c.eax = 0u32;
                st = 1;
            }
            1 => {
                c.esi = m.read32(c.esp.wrapping_sub(4u32));
                break 'machine;
            }
            _ => break 'machine,
        }
    }
}
