// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-lane-c2r/lib
//! Extracted capture Tier 2 roots from the T2049 c2r batches.
#![allow(unused_assignments, unused_mut, unused_variables, unused_parens, clippy::all)]
use super::rt::*;
use crate::state_wire as gs;

pub fn game_list_0x230_record_decay_timer_0x23c_by_frame_dt_clamped_zero<M: GuestMemory>(
    m: &mut M,
    r: &mut ExtRegisters,
) {
    let (mut fca, mut fcb) = (0f64, 0f64); // operands of the last SSE compare
    r.eax = m.read32(r.esp.wrapping_add(4));
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.eax.wrapping_add(0x23c))).to_bits());
    r.xmm[1] = Xmm::default();
    fca = f32::from_bits(r.xmm[0].lanes[0]) as f64;
    fcb = f32::from_bits(r.xmm[1].lanes[0]) as f64;
    if (fca <= fcb) || ((fca != fca) || (fcb != fcb)) {
        return;
    }
    r.xmm[0].set_f(0, (m.read32(0x4e8cec) as i32) as f32);
    r.xmm[2] = Xmm::scalar(f32::from_bits(m.read32(0x475c78)).to_bits());
    r.xmm[2].set_f(0, r.xmm[2].f(0) / r.xmm[0].f(0));
    r.xmm[2].set_f(0, r.xmm[2].f(0) * f32::from_bits(m.read32(0x4e794c)));
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.eax.wrapping_add(0x23c))).to_bits());
    r.xmm[0].set_f(0, r.xmm[0].f(0) - r.xmm[2].f(0));
    fca = f32::from_bits(r.xmm[1].lanes[0]) as f64;
    fcb = f32::from_bits(r.xmm[0].lanes[0]) as f64;
    m.write32(r.eax.wrapping_add(0x23c), r.xmm[0].f(0).to_bits());
    if (fca <= fcb) || ((fca != fca) || (fcb != fcb)) {
        return;
    }
    m.write32(r.eax.wrapping_add(0x23c), r.xmm[1].f(0).to_bits());
}

pub fn game_composite_instance_validate_parts_release_if_invalid<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.push(m, r.edi);
    r.edi = m.read32(r.esp.wrapping_add(8));
    if r.edi != 0 {
        r.ecx = m.read32(r.edi);
        if r.ecx != 0 {
            'at_352103: {
                let eax_1: u32 = (((m.read32(r.ecx.wrapping_add(4)) as i32).wrapping_mul(0x4dci32))
                    as u32)
                    .wrapping_add(0x53fa58);
                r.eax = m.read32(eax_1);
                r.push(m, r.ebx);
                r.edx = 0;
                r.push(m, r.esi);
                if (r.eax as i32) > 0i32 {
                    r.esi = r.ecx.wrapping_add(8);
                    loop {
                        // 0x3520e0
                        r.ecx = m.read32(r.esi);
                        r.ebx = m.read32(r.ecx.wrapping_add(0x80));
                        if r.ebx != 0 {
                            r.ecx = m.read32(r.ecx.wrapping_add(0x34));
                            if (r.ecx as i32) > 0i32 {
                                if (r.ecx as i32) <= 0xd5 {
                                    r.edx = r.edx.wrapping_add(1);
                                    r.esi = r.esi.wrapping_add(4);
                                    if (r.edx as i32) < (r.eax as i32) {
                                        continue;
                                    }
                                    break 'at_352103;
                                }
                            }
                        }
                        // 0x35210c
                        r.push(m, r.edi);
                        r.call(guest, 0x351f60);
                        r.release_stack(4);
                        r.esi = r.pop(m);
                        r.ebx = r.pop(m);
                        r.eax = 0;
                        r.edi = r.pop(m);
                        return;
                    }
                }
            }
            // 0x352103
            r.esi = r.pop(m);
            r.ebx = r.pop(m);
            r.eax = 1;
            r.edi = r.pop(m);
            return;
        }
    }
    // 0x35211b
    r.eax = 0;
    r.edi = r.pop(m);
}

pub const ROOTS: [u32; 2] = [0xdb4c0, 0x3520b0];

#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0xdb4c0 => { game_list_0x230_record_decay_timer_0x23c_by_frame_dt_clamped_zero(m, regs); true },
        0x3520b0 => { game_composite_instance_validate_parts_release_if_invalid(m, guest, regs); true },
        _ => false,
    }
}
