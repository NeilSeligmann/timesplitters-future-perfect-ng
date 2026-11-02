// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-lane-c2r/lib
//! Extracted T2049 physics roots used by capture Tier 2.
#![allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
use super::rt::*;

/// Original 0x47560 (engine/physics), lifted sub_00047560.
pub fn game_box_overlap_vec3_min_max<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    let (mut fca, mut fcb) = (0f64, 0f64); // operands of the last SSE compare
    r.edx = m.read32(r.esp.wrapping_add(4));
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.edx)).to_bits());
    r.push(m, r.esi);
    r.esi = m.read32(r.esp.wrapping_add(0x14));
    if !((f32::from_bits(r.xmm[0].lanes[0]) as f64) > (f32::from_bits(m.read32(r.esi)) as f64)) {
        let eax_1: u32 = m.read32(r.esp.wrapping_add(0x10));
        r.ecx = m.read32(r.esp.wrapping_add(0xc));
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(eax_1)).to_bits());
        if !((f32::from_bits(r.xmm[0].lanes[0]) as f64) > (f32::from_bits(m.read32(r.ecx)) as f64))
        {
            r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.edx.wrapping_add(4))).to_bits());
            if !((f32::from_bits(r.xmm[0].lanes[0]) as f64)
                > (f32::from_bits(m.read32(r.esi.wrapping_add(4))) as f64))
            {
                r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(eax_1.wrapping_add(4))).to_bits());
                if !((f32::from_bits(r.xmm[0].lanes[0]) as f64)
                    > (f32::from_bits(m.read32(r.ecx.wrapping_add(4))) as f64))
                {
                    r.xmm[0] =
                        Xmm::scalar(f32::from_bits(m.read32(r.edx.wrapping_add(8))).to_bits());
                    if !((f32::from_bits(r.xmm[0].lanes[0]) as f64)
                        > (f32::from_bits(m.read32(r.esi.wrapping_add(8))) as f64))
                    {
                        r.xmm[0] =
                            Xmm::scalar(f32::from_bits(m.read32(eax_1.wrapping_add(8))).to_bits());
                        if !((f32::from_bits(r.xmm[0].lanes[0]) as f64)
                            > (f32::from_bits(m.read32(r.ecx.wrapping_add(8))) as f64))
                        {
                            r.eax = 1;
                            r.esi = r.pop(m);
                            return;
                        }
                    }
                }
            }
        }
    }
    r.eax = 0;
    r.esi = r.pop(m);
}

pub fn game_page_bitmap_allocate_page_by_size_order<M: GuestMemory>(
    m: &mut M,
    r: &mut ExtRegisters,
) {
    let (mut fa, mut fb, mut fas, mut fbs) = (0, 0, 0i32, 0i32); // lifted flag model: operands of the last compare, kept where a later branch reads them
    'at_3d77e: {
        'at_3d734: {
            r.ecx = m.read32(r.esp.wrapping_add(4));
            r.push(m, r.ebx);
            r.ebx = set_lo8(r.ebx, m.read8(0x6b7b00) as u32);
            r.push(m, r.ebp);
            r.push(m, r.esi);
            r.esi = 1u32.wrapping_shl((((r.ecx as u8) as u32) & 0x1f) as u32 & 31);
            r.push(m, r.edi);
            r.ebp = 0;
            r.esi = r.esi.wrapping_sub(1);
            r.edi = 4u32.wrapping_sub(r.ecx);
            r.eax = (r.ebx as u8) as u32;
            r.ecx = r.ebp;
            if (r.edi as i32) >= 0 {
                loop {
                    // 0x3d706
                    fa = r.esi;
                    fb = r.eax;
                    fas = fa as i32;
                    fbs = fb as i32;
                    if (r.esi & r.eax) == 0 {
                        r.eax = r.esi;
                        if !((((r.ecx as u8) as u32) & 0x1f) != 0) {
                            fa = if (fa & fb) == 0 {
                                0u32
                            } else {
                                (if (((fas as u32) & (fbs as u32)) as i32) < 0 {
                                    0x80000000u32
                                } else {
                                    1u32
                                })
                            };
                            fas = fa as i32;
                        }
                        r.eax = r
                            .eax
                            .wrapping_shl((((r.ecx as u8) as u32) & 0x1f) as u32 & 31);
                        if (((r.ecx as u8) as u32) & 0x1f) != 0 {
                            fa = r.eax;
                            fas = fa as i32;
                        }
                        r.ebp = r.ecx.wrapping_shl(12).wrapping_add(0x7e72e0);
                        r.ebx = set_lo8(
                            r.ebx,
                            (((r.ebx as u8) as i32) | ((r.eax as u8) as i32)) as u32,
                        );
                        m.write8(0x6b7b00, ((r.ebx as u8) as u32) as u8);
                        m.write8(r.ecx.wrapping_add(0x6b7af4), ((r.eax as u8) as u32) as u8);
                        if r.ebp != 0 {
                            break 'at_3d77e;
                        }
                        break 'at_3d734;
                    } else {
                        r.eax = r.eax.wrapping_shr(1);
                        r.ecx = r.ecx.wrapping_add(1);
                        if (r.ecx as i32) > (r.edi as i32) {
                            break 'at_3d734;
                        }
                    }
                }
            }
        }
        // 0x3d734
        r.eax = m.read32(0x6b7b04);
        if r.eax != 0 {
            r.eax = r.eax.wrapping_sub(0x7e72e0);
            r.edx = if (r.eax as i32) < 0i32 {
                0xffffffffu32
            } else {
                0
            };
            r.edx = r.edx & 0xfff;
            let eax_1: u32 = ((r.eax.wrapping_add(r.edx) as i32).wrapping_shr(12)) as u32;
            let eax_2: u32 = set_lo8(eax_1, m.read8(eax_1.wrapping_add(0x6b7af4)) as u32);
            let eax_3: u32 = set_lo8(eax_2, (!(eax_2 as u8)) as u32);
            r.ecx = 0;
            r.ebx = set_lo8(
                r.ebx,
                (((r.ebx as u8) as i32) & ((eax_3 as u8) as i32)) as u32,
            );
            m.write8(0x6b7b00, ((r.ebx as u8) as u32) as u8);
            m.write32(0x6b7b04, r.ecx);
            m.write32(0x6b7af8, r.ecx);
            r.eax = (r.ebx as u8) as u32;
            if (r.edi as i32) >= (r.ecx as i32) {
                loop {
                    // 0x3d773
                    fa = r.esi;
                    fb = r.eax;
                    fas = fa as i32;
                    fbs = fb as i32;
                    if (r.esi & r.eax) == 0 {
                        if !((((r.ecx as u8) as u32) & 0x1f) != 0) {
                            fa = if (fa & fb) == 0 {
                                0u32
                            } else {
                                (if (((fas as u32) & (fbs as u32)) as i32) < 0 {
                                    0x80000000u32
                                } else {
                                    1u32
                                })
                            };
                            fas = fa as i32;
                        }
                        r.esi = r
                            .esi
                            .wrapping_shl((((r.ecx as u8) as u32) & 0x1f) as u32 & 31);
                        if (((r.ecx as u8) as u32) & 0x1f) != 0 {
                            fa = r.esi;
                            fas = fa as i32;
                        }
                        r.edi = r.pop(m);
                        let eax_4: u32 = r.esi;
                        r.esi = r.ecx.wrapping_shl(12).wrapping_add(0x7e72e0);
                        r.ebx = set_lo8(
                            r.ebx,
                            (((r.ebx as u8) as i32) | ((eax_4 as u8) as i32)) as u32,
                        );
                        m.write8(r.ecx.wrapping_add(0x6b7af4), ((eax_4 as u8) as u32) as u8);
                        r.eax = r.esi;
                        r.esi = r.pop(m);
                        r.ebp = r.pop(m);
                        m.write8(0x6b7b00, ((r.ebx as u8) as u32) as u8);
                        r.ebx = r.pop(m);
                        return;
                    } else {
                        r.eax = r.eax.wrapping_shr(1);
                        r.ecx = r.ecx.wrapping_add(1);
                        if (r.ecx as i32) > (r.edi as i32) {
                            break 'at_3d77e;
                        }
                    }
                }
            }
        }
    }
    // 0x3d77e
    r.edi = r.pop(m);
    r.esi = r.pop(m);
    r.eax = r.ebp;
    r.ebp = r.pop(m);
    r.ebx = r.pop(m);
}

pub const ROOTS: [u32; 2] = [0x47560, 0x3d6e0];

pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    _guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x3d6e0 => {
            game_page_bitmap_allocate_page_by_size_order(m, regs);
            true
        }
        0x47560 => {
            game_box_overlap_vec3_min_max(m, regs);
            true
        }
        _ => false,
    }
}
