// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-port-gameplay-modes/c2r/modes_deferred
//! Readable rewrites (T1947, `python3 -m tools.llm_readable_c2r`) of the mechanical `modes_deferred` bodies.
//! Each function was accepted only after the T1928 differential against the retained lifted original
//! reported 0 divergences (lifted O0 x Rust O0 strict, O3 x O3 NaN tolerant; hand-built fixtures of
//! tests/c/c2r_hints.inc). The mechanical module `modes_deferred` stays as the reference and fallback.
//! Dispatch: `c2r::run` consults this module first.
#![allow(
    unused_parens,
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_macros,
    clippy::all
)]
use super::rt::*;
use crate::state_wire as gs;

macro_rules! call_story_mission_tick_or_lifted {
    ($runtime:ident, $guest:ident) => {
        crate::story_mission_tick::dispatch_registered(|| $runtime.call($guest, 0x228870));
    };
}

/// 0x674c0. INFERRED: Updates per-frame mode timers, player limits, and leader messages.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_match_per_frame_rules_timers_limits_and_leader_messages_when_byte_7de456_modes<
    M: GuestMemory,
    C: GuestCall,
>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    let (mut fa,) = (0,);
    let (mut fca, mut fcb) = (0f64, 0f64);
    'at_67627: {
        r.esp = r.esp.wrapping_sub(0x44);
        r.eax = set_lo8(r.eax, m.read8(0x7de456) as u32);
        r.push(m, r.ebx);
        r.push(m, r.ebp);
        r.push(m, r.esi);
        r.ebx = 0;
        r.push(m, r.edi);
        m.write32(r.esp.wrapping_add(0x10), 0x80000000u32);
        r.esi = 1;
        if (((r.eax as u8) as u32) & 0xff) == 0x13 {
            'at_6761c: {
                r.xmm[1].set_f(0, (m.read32(0x4e8cec) as i32) as f32);
                r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x475c78)).to_bits());
                fa = m.read32(0x6f11b0);
                r.xmm[0].set_f(0, r.xmm[0].f(0) / r.xmm[1].f(0));
                r.xmm[0].set_f(0, r.xmm[0].f(0) * f32::from_bits(m.read32(0x4e794c)));
                r.xmm[0].set_f(0, r.xmm[0].f(0) + f32::from_bits(m.read32(0x6f11ac)));
                m.write32(0x6f11ac, r.xmm[0].f(0).to_bits());
                if fa == r.ebx {
                    fca = f32::from_bits(r.xmm[0].lanes[0]) as f64;
                    fcb = f32::from_bits(m.read32(0x4788fc)) as f64;
                    if (fca <= fcb) || (fca.is_nan() || fcb.is_nan()) {
                        break 'at_67627;
                    }
                    'at_6775b: {
                        'at_67749: {
                            r.edi = gs::get_players(m);
                            r.eax = gs::get_extra_players(m);
                            r.eax = r.eax.wrapping_add(r.edi);
                            m.write32(0x6f11b0, r.esi);
                            if (r.edi as i32) < (r.eax as i32) {
                                r.ebp = r.edi;
                                r.ebp = ((r.ebp as i32).wrapping_mul(0xea0i32)) as u32;
                                loop {
                                    r.ecx = m.read32(0x7356d8);
                                    r.edx = ((m.read16(r.ecx.wrapping_add(r.ebp).wrapping_add(0x48))
                                        as i16) as i32)
                                        as u32;
                                    r.esi = r.ecx.wrapping_add(r.ebp);
                                    r.push(m, r.edx);
                                    r.call(guest, 0xd6c90);
                                    r.esp = r.esp.wrapping_add(4);
                                    if r.eax != 0 {
                                        if (gs::get_mode(m) as u32 & 0xff) != 0xa {
                                            r.edx = m.read32(r.esi.wrapping_add(0x1c));
                                            r.eax = m.read32(r.esi.wrapping_add(0x18));
                                            m.write32(r.esp.wrapping_add(0x18), r.edx);
                                        } else {
                                            r.ecx = m.read32(r.esi.wrapping_add(0x14));
                                            r.eax = m.read32(r.esi.wrapping_add(0x10));
                                            m.write32(r.esp.wrapping_add(0x18), r.ecx);
                                        }
                                        r.eax &= 0x10000;
                                        r.ecx = 0;
                                        r.eax |= r.ecx;
                                        if r.eax == 0 {
                                            break 'at_6775b;
                                        }
                                    }
                                    r.eax = gs::get_players(m);
                                    r.ecx = gs::get_extra_players(m);
                                    r.edi = r.edi.wrapping_add(1);
                                    r.ecx = r.ecx.wrapping_add(r.eax);
                                    r.ebp = r.ebp.wrapping_add(0xea0);
                                    if (r.edi as i32) >= (r.ecx as i32) {
                                        if m.read32(0x6f11b0) == r.ebx {
                                            break 'at_67627;
                                        }
                                        break 'at_67749;
                                    }
                                }
                            }
                        }
                        r.push(m, r.ebx);
                        r.call(guest, 0x64350);
                        r.esp = r.esp.wrapping_add(4);
                        m.write32(0x7de308, r.eax);
                        if r.eax != r.ebx {
                            r.edx = m.read32(r.eax.wrapping_add(0x7c));
                            r.push(m, r.edx);
                            r.call(guest, 0x64240);
                            r.esp = r.esp.wrapping_add(4);
                            m.write32(0x7de324, r.eax);
                            if r.eax != r.ebx {
                                break 'at_6761c;
                            }
                            m.write32(0x6f11b0, r.ebx);
                            break 'at_67627;
                        }
                    }
                    m.write32(0x6f11b0, r.ebx);
                    break 'at_67627;
                }
                r.eax = m.read32(0x7de324);
                r.eax = m.read32(r.eax.wrapping_add(0x7c));
                if (m.read8(r.eax.wrapping_add(0x24)) as u32 & 0xff & 8) == 0 {
                    r.push(m, r.ebx);
                    r.push(m, r.esi);
                    r.push(m, r.eax);
                    r.call(guest, 0x174e0);
                    r.esp = r.esp.wrapping_add(0xc);
                    if r.eax == 0 {
                        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x6f11ac)).to_bits());
                        fca = f32::from_bits(r.xmm[0].lanes[0]) as f64;
                        fcb = f32::from_bits(m.read32(0x478900)) as f64;
                        if (fca <= fcb) || (fca.is_nan() || fcb.is_nan()) {
                            break 'at_67627;
                        }
                    }
                    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x6f11ac)).to_bits());
                }
                r.xmm[1] = Xmm::scalar(f32::from_bits(m.read32(0x4788fc)).to_bits());
                fca = f32::from_bits(r.xmm[1].lanes[0]) as f64;
                fcb = f32::from_bits(r.xmm[0].lanes[0]) as f64;
                m.write32(0x6f11b0, r.ebx);
                if !((fca <= fcb) || (fca.is_nan() || fcb.is_nan())) {
                    r.push(m, r.ebx);
                    r.call(guest, 0x64240);
                    r.esp = r.esp.wrapping_add(4);
                    m.write32(0x7de324, r.eax);
                    if r.eax != r.ebx && r.eax != m.read32(0x7de308) {
                        m.write32(0x6f11b0, r.esi);
                        break 'at_67627;
                    }
                    if m.read32(0x6f11b0) != r.ebx {
                        break 'at_67627;
                    }
                }
                r.edi = gs::get_players(m);
                r.ecx = gs::get_extra_players(m);
                r.ecx = r.ecx.wrapping_add(r.edi);
                if (r.edi as i32) < (r.ecx as i32) {
                    r.ebp = r.edi;
                    r.ebp = ((r.ebp as i32).wrapping_mul(0xea0i32)) as u32;
                    loop {
                        r.edx = m.read32(0x7356d8);
                        r.eax = ((m.read16(r.edx.wrapping_add(r.ebp).wrapping_add(0x48)) as i16)
                            as i32) as u32;
                        r.esi = r.edx.wrapping_add(r.ebp);
                        r.push(m, r.eax);
                        r.call(guest, 0xd6c90);
                        r.esp = r.esp.wrapping_add(4);
                        if r.eax != 0 && (m.read8(r.esi.wrapping_add(0x24)) as u32 & 0xff & 8) == 0
                        {
                            r.push(m, r.ebx);
                            r.push(m, 0x10003);
                            r.push(m, r.esi);
                            r.call(guest, 0x174e0);
                            r.esp = r.esp.wrapping_add(0xc);
                            if r.eax == 0 {
                                r.push(m, r.ebx);
                                r.push(m, 0x10000);
                                r.push(m, r.esi);
                                r.call(guest, 0x17570);
                                r.esp = r.esp.wrapping_add(0xc);
                            }
                        }
                        r.ecx = gs::get_players(m);
                        r.edx = gs::get_extra_players(m);
                        r.edi = r.edi.wrapping_add(1);
                        r.edx = r.edx.wrapping_add(r.ecx);
                        r.ebp = r.ebp.wrapping_add(0xea0);
                        if (r.edi as i32) >= (r.edx as i32) {
                            break 'at_6761c;
                        }
                    }
                }
            }
            r.xmm[0] = Xmm::default();
            m.write32(0x6f11ac, r.xmm[0].f(0).to_bits());
        }
    }
    'at_6781f: {
        r.eax = m.read32(0x7de45c);
        r.ebp = 0x4000;
        if r.eax & r.ebp == 0 && m.read32(0x7de474) == r.ebx && gs::get_flags(m) & 0x800000 == 0 {
            r.eax = gs::get_extra_players(m);
            r.ecx = gs::get_players(m);
            r.eax = r.eax.wrapping_add(r.ecx);
            r.esi = 0;
            if (r.eax as i32) > 0 {
                r.edi = 0;
                loop {
                    r.push(m, r.esi);
                    r.call(guest, 0x3568c0);
                    r.esp = r.esp.wrapping_add(4);
                    if r.eax != 0 {
                        r.edx = m.read32(0x7356d8).wrapping_add(r.edi);
                        r.push(m, r.edx);
                        r.call(guest, 0x640c0);
                        r.esp = r.esp.wrapping_add(4);
                        if (r.eax as i32) > 0 {
                            'at_677b0: {
                                r.ecx = m.read32(0x4c0454);
                                if r.ecx != 4 {
                                    fa = r.ecx;
                                    r.eax = 0x4766d8;
                                    r.ecx = 0x47602c;
                                    if fa == 1 {
                                        break 'at_677b0;
                                    }
                                } else {
                                    r.eax = 0x4788f8;
                                }
                                r.ecx = 0x4766d8;
                            }
                            r.edx = m.read32(0x7356d8);
                            r.esi = ((r.esi as i32).wrapping_mul(0xea0i32)) as u32;
                            r.push(m, r.ecx);
                            r.ecx = m.read32(r.esi.wrapping_add(r.edx).wrapping_add(0x70));
                            r.push(m, r.ecx);
                            r.push(m, r.eax);
                            r.push(m, 0xe96);
                            r.call(guest, 0x3d7b0);
                            r.esp = r.esp.wrapping_add(4);
                            r.push(m, r.eax);
                            r.push(m, 0x4788ec);
                            r.call(guest, 0x151030);
                            r.esp = r.esp.wrapping_add(0x14);
                            r.edi = r.eax;
                            r.esi = 0;
                            r.call(guest, 0x1cdae0);
                            'at_67807: {
                                if (r.eax as i32) > 0 {
                                    loop {
                                        r.push(m, r.ebx);
                                        r.push(m, r.edi);
                                        r.push(m, 3);
                                        r.push(m, r.esi);
                                        r.call(guest, 0x12d350);
                                        r.esp = r.esp.wrapping_add(0x10);
                                        r.esi = r.esi.wrapping_add(1);
                                        r.call(guest, 0x1cdae0);
                                        if (r.esi as i32) >= (r.eax as i32) {
                                            break 'at_67807;
                                        }
                                    }
                                }
                            }
                            r.edx = m.read32(0x7de45c) | r.ebp;
                            r.push(m, 3);
                            m.write32(0x7de45c, r.edx);
                            r.call(guest, 0x164a30);
                            r.esp = r.esp.wrapping_add(4);
                            break 'at_6781f;
                        }
                    }
                    r.eax = gs::get_players(m);
                    r.ecx = gs::get_extra_players(m);
                    r.esi = r.esi.wrapping_add(1);
                    r.ecx = r.ecx.wrapping_add(r.eax);
                    r.edi = r.edi.wrapping_add(0xea0);
                    if (r.esi as i32) >= (r.ecx as i32) {
                        break 'at_6781f;
                    }
                }
            }
        }
    }
    'at_67963: {
        if (m.read32(0x7de464) as i32) > (r.ebx as i32) {
            r.eax = gs::get_extra_players(m);
            r.ecx = gs::get_players(m);
            r.edx = 0;
            m.write32(r.esp.wrapping_add(0x14), r.edx);
            m.write32(r.esp.wrapping_add(0x18), r.edx);
            r.eax = r.eax.wrapping_add(r.ecx);
            m.write32(r.esp.wrapping_add(0x1c), r.edx);
            r.edi = 0;
            m.write32(r.esp.wrapping_add(0x20), r.edx);
            if (r.eax as i32) > 0 {
                r.ebp = 0;
                r.ebx = 0x77728c;
                loop {
                    r.esi = r.esi | 0xffffffffu32;
                    r.call(guest, 0x3533f0);
                    'at_678d5: {
                        if r.eax != 0 && (m.read8(r.ebx) as u32 & 0xff & 2) == 0 {
                            break 'at_678d5;
                        }
                        if gs::get_flags_low8(m) as u32 & 0xff & 1 == 0 {
                            r.call(guest, 0x355ec0);
                            if r.eax == 0 {
                                r.eax = m.read32(0x7356d8);
                                r.esi = m.read32(r.eax.wrapping_add(r.ebp).wrapping_add(0xa6c));
                            } else {
                                r.push(m, r.edi);
                                r.call(guest, 0x641c0);
                                r.esp = r.esp.wrapping_add(4);
                                r.esi = r.eax;
                            }
                        } else {
                            r.edx = m.read32(0x7356d8);
                            r.eax = m.read32(r.edx.wrapping_add(r.ebp).wrapping_add(0x84));
                            r.ecx = set_lo8(
                                r.ecx,
                                m.read8(r.esp.wrapping_add(r.eax).wrapping_add(0x14)) as u32,
                            );
                            if r.ecx as u8 == 0 {
                                r.push(m, r.eax);
                                m.write8(r.esp.wrapping_add(r.eax).wrapping_add(0x18), 1);
                                r.call(guest, 0x64160);
                                r.esp = r.esp.wrapping_add(4);
                                r.esi = r.eax;
                            }
                        }
                        if (r.esi as i32) > (m.read32(r.esp.wrapping_add(0x10)) as i32) {
                            m.write32(r.esp.wrapping_add(0x10), r.esi);
                        }
                        if (r.esi as i32) >= (m.read32(0x7de464) as i32) {
                            r.call(guest, 0x355f10);
                            if r.eax != 0 {
                                r.call(guest, 0x357690);
                                if r.eax == 0 {
                                    break 'at_67963;
                                }
                            }
                            r.eax = set_lo8(r.eax, gs::get_mode(m) as u32);
                            if r.eax as u8 != 3 && r.eax as u8 != 7 {
                                m.write32(0x6f11a4, 1);
                                r.call(guest, 0x66e90);
                            }
                            r.eax = gs::get_flags(m);
                            if r.eax as u8 & 0x40 == 0 && r.eax & 0x8000000 == 0 {
                                r.call(guest, 0x66fc0);
                                break 'at_67963;
                            }
                            r.ecx = gs::get_match_state(m);
                            r.eax = 8;
                            if r.ecx != r.eax {
                                r.xmm[0] = Xmm::default();
                                gs::set_match_state(m, r.eax);
                                m.write32(0x7de300, r.xmm[0].f(0).to_bits());
                            }
                            r.call(guest, 0x231a20);
                            break 'at_67963;
                        }
                    }
                    r.ecx = gs::get_extra_players(m);
                    r.edx = gs::get_players(m);
                    r.edi = r.edi.wrapping_add(1);
                    r.ecx = r.ecx.wrapping_add(r.edx);
                    r.ebx = r.ebx.wrapping_add(0x124);
                    r.ebp = r.ebp.wrapping_add(0xea0);
                    if (r.edi as i32) >= (r.ecx as i32) {
                        break 'at_67963;
                    }
                }
            }
        }
    }
    'at_67a38: {
        r.eax = m.read32(0x7de474);
        if r.eax != 0 {
            'at_679cf: {
                r.eax = gs::get_extra_players(m);
                r.ecx = gs::get_players(m);
                r.eax = r.eax.wrapping_add(r.ecx);
                r.ebp = 0;
                r.edi = 0;
                if (r.eax as i32) > 0 {
                    r.esi = 0;
                    loop {
                        r.push(m, r.edi);
                        r.call(guest, 0x3568c0);
                        r.esp = r.esp.wrapping_add(4);
                        if r.eax != 0 {
                            r.eax = m.read32(0x7356d8);
                            if (((m.read16(r.esi.wrapping_add(r.eax).wrapping_add(0xa60)) as u32
                                & 0xffff) as i16) as i32)
                                > 0
                            {
                                r.edx = m.read32(0x7de318);
                                m.write32(r.esi.wrapping_add(r.eax).wrapping_add(0xa64), r.edx);
                                r.ebp = r.ebp.wrapping_add(1);
                            }
                        }
                        r.eax = gs::get_players(m);
                        r.ecx = gs::get_extra_players(m);
                        r.edi = r.edi.wrapping_add(1);
                        r.ecx = r.ecx.wrapping_add(r.eax);
                        r.esi = r.esi.wrapping_add(0xea0);
                        if (r.edi as i32) >= (r.ecx as i32) {
                            if (r.ebp as i32) > 1 {
                                break 'at_67a38;
                            }
                            break 'at_679cf;
                        }
                    }
                }
            }
            r.call(guest, 0x355f10);
            if r.eax != 0 {
                r.call(guest, 0x357690);
                if r.eax == 0 {
                    break 'at_67a38;
                }
            }
            r.eax = set_lo8(r.eax, gs::get_mode(m) as u32);
            if r.eax as u8 != 3 && r.eax as u8 != 7 {
                m.write32(0x6f11a4, 1);
                r.call(guest, 0x66e90);
            }
            r.eax = gs::get_flags(m);
            if r.eax as u8 & 0x40 == 0 && r.eax & 0x8000000 == 0 {
                r.call(guest, 0x66fc0);
                break 'at_67a38;
            }
            r.ecx = gs::get_match_state(m);
            r.eax = 8;
            if r.ecx != r.eax {
                r.xmm[0] = Xmm::default();
                gs::set_match_state(m, r.eax);
                m.write32(0x7de300, r.xmm[0].f(0).to_bits());
            }
            r.call(guest, 0x231a20);
        }
    }
    'at_67c58: {
        'at_67b8b: {
            'at_67ac8: {
                if m.read8(0x7de456) == 0xd {
                    r.eax = m.read32(0x7de318);
                    r.edx = if (r.eax as i32) < 0 { 0xffffffff } else { 0 };
                    let dividend = (((r.edx as i32 as i64) << 32) | r.eax as i64) as i64;
                    r.eax = (dividend / (m.read32(0x4e8cec) as i32 as i64)) as i32 as u32;
                    r.edx = (dividend % (m.read32(0x4e8cec) as i32 as i64)) as i32 as u32;
                    if r.edx != 0 {
                        break 'at_67b8b;
                    }
                    r.eax = m.read32(0x74c37c);
                    if (r.eax as i32) <= 0 {
                        break 'at_67b8b;
                    }
                    r.xmm[1] = Xmm::scalar(f32::from_bits(m.read32(0x475ce8)).to_bits());
                    r.edi = r.edi;
                    loop {
                        r.eax = m.read32(r.edx.wrapping_mul(4).wrapping_add(0x7a3580));
                        if r.eax != 0 {
                            r.ecx = m.read32(r.eax);
                            r.xmm[0] = Xmm::scalar(
                                f32::from_bits(m.read32(r.ecx.wrapping_add(0x1cc))).to_bits(),
                            );
                            r.xmm[0].set_f(0, r.xmm[0].f(0) + r.xmm[1].f(0));
                            m.write32(r.ecx.wrapping_add(0x1cc), r.xmm[0].f(0).to_bits());
                            r.eax = m.read32(r.eax);
                            r.xmm[0] = Xmm::scalar(
                                f32::from_bits(m.read32(r.eax.wrapping_add(0x1cc))).to_bits(),
                            );
                            fca = f32::from_bits(r.xmm[0].lanes[0]) as f64;
                            fcb = f32::from_bits(m.read32(r.eax.wrapping_add(0x1d8))) as f64;
                            if (fca <= fcb) || (fca.is_nan() || fcb.is_nan()) {
                                r.xmm[0] = Xmm::scalar(
                                    f32::from_bits(m.read32(r.eax.wrapping_add(0x1cc))).to_bits(),
                                );
                            } else {
                                r.xmm[0] = Xmm::scalar(
                                    f32::from_bits(m.read32(r.eax.wrapping_add(0x1d8))).to_bits(),
                                );
                            }
                            m.write32(r.eax.wrapping_add(0x1cc), r.xmm[0].f(0).to_bits());
                        }
                        r.eax = m.read32(0x74c37c);
                        r.edx = r.edx.wrapping_add(1);
                        if (r.edx as i32) >= (r.eax as i32) {
                            break 'at_67ac8;
                        }
                    }
                }
            }
            'at_67b7e: {
                if m.read8(0x7de456) == 9 {
                    if gs::get_match_state(m) != 7 {
                        break 'at_67b8b;
                    }
                    r.eax = m.read32(0x74c37c);
                    r.edi = 0;
                    if (r.eax as i32) <= 0 {
                        break 'at_67b8b;
                    }
                    loop {
                        r.esi = m.read32(r.edi.wrapping_mul(4).wrapping_add(0x7a3580));
                        if r.esi != 0 {
                            r.eax = m.read32(r.esi);
                            r.xmm[0] = Xmm::scalar(
                                f32::from_bits(m.read32(r.eax.wrapping_add(0x1cc))).to_bits(),
                            );
                            r.xmm[1] = Xmm::default();
                            fca = f32::from_bits(r.xmm[0].lanes[0]) as f64;
                            fcb = f32::from_bits(r.xmm[1].lanes[0]) as f64;
                            if !((fca <= fcb) || (fca.is_nan() || fcb.is_nan())) {
                                r.xmm[0].set_f(0, (m.read32(0x4e8cec) as i32) as f32);
                                r.xmm[0]
                                    .set_f(0, r.xmm[0].f(0) * f32::from_bits(m.read32(0x4782c4)));
                                r.xmm[2] =
                                    Xmm::scalar(f32::from_bits(m.read32(0x475c78)).to_bits());
                                r.xmm[2]
                                    .set_f(0, r.xmm[2].f(0) / f32::from_bits(m.read32(0x7de48c)));
                                r.xmm[2].set_f(0, r.xmm[2].f(0) / r.xmm[0].f(0));
                                r.xmm[0].set_f(0, (m.read32(0x4e7944) as i32) as f32);
                                r.xmm[2].set_f(0, r.xmm[2].f(0) * r.xmm[0].f(0));
                                r.xmm[0] = Xmm::scalar(
                                    f32::from_bits(m.read32(r.esi.wrapping_add(0xa88))).to_bits(),
                                );
                                r.xmm[0].set_f(0, r.xmm[0].f(0) - r.xmm[2].f(0));
                                fca = f32::from_bits(r.xmm[1].lanes[0]) as f64;
                                fcb = f32::from_bits(r.xmm[0].lanes[0]) as f64;
                                m.write32(r.esi.wrapping_add(0xa88), r.xmm[0].f(0).to_bits());
                                if !((fca < fcb) || (fca.is_nan() || fcb.is_nan())) {
                                    r.push(m, r.eax);
                                    r.call(guest, 0xbd090);
                                    r.xmm[0] =
                                        Xmm::scalar(f32::from_bits(m.read32(0x475c78)).to_bits());
                                    r.esp = r.esp.wrapping_add(4);
                                    m.write32(r.esi.wrapping_add(0xa88), r.xmm[0].f(0).to_bits());
                                }
                            }
                        }
                        r.eax = m.read32(0x74c37c);
                        r.edi = r.edi.wrapping_add(1);
                        if (r.edi as i32) >= (r.eax as i32) {
                            break 'at_67b7e;
                        }
                    }
                }
            }
            if m.read8(0x7de456) == 0xa {
                break 'at_67c58;
            }
        }
        r.eax = m.read32(r.esp.wrapping_add(0x10));
        if r.eax != 0x80000000 {
            r.ecx = m.read32(0x7de464);
            if (r.eax as i32) < (r.ecx as i32) {
                r.ecx = r.ecx.wrapping_sub(r.eax);
                r.eax = r.ecx;
                r.ecx = m.read32(0x7de45c);
                if (r.eax as i32) > 3 {
                    r.ecx &= 0xfffff7ff;
                    m.write32(0x7de45c, r.ecx);
                }
                if (r.eax as i32) > 2 {
                    r.ecx &= 0xffffefff;
                    m.write32(0x7de45c, r.ecx);
                }
                if (r.eax as i32) > 1 {
                    r.ecx &= 0xffffdfff;
                    m.write32(0x7de45c, r.ecx);
                }
                if r.eax == 1 {
                    if ((r.ecx >> 8) as u8) & 0x20 != 0 {
                        break 'at_67c58;
                    }
                    r.ecx |= 0x2000;
                    r.push(m, 0);
                    m.write32(0x7de45c, r.ecx);
                    r.call(guest, 0x164a30);
                    r.esp = r.esp.wrapping_add(4);
                    r.edi = m.read32(r.esp);
                    r.esp = r.esp.wrapping_add(4);
                    r.esi = m.read32(r.esp);
                    r.esp = r.esp.wrapping_add(4);
                    r.ebp = m.read32(r.esp);
                    r.esp = r.esp.wrapping_add(4);
                    r.ebx = m.read32(r.esp);
                    r.esp = r.esp.wrapping_add(4);
                    r.esp = r.esp.wrapping_add(0x44);
                    return;
                } else if r.eax == 2 {
                    if ((r.ecx >> 8) as u8) & 0x10 != 0 {
                        break 'at_67c58;
                    }
                    r.ecx = r.esp.wrapping_add(0x24);
                    r.push(m, r.ecx);
                    r.call(guest, 0x45390);
                    r.edx = r.esp.wrapping_add(0x28);
                    r.push(m, r.edx);
                    m.write32(r.esp.wrapping_add(0x2c), 0xd49);
                    r.call(guest, 0x45820);
                    r.push(m, 1);
                    r.call(guest, 0x164a30);
                    m.write32(0x7de45c, m.read32(0x7de45c) | 0x1000);
                    r.push(m, 0x16f3);
                } else if r.eax == 3 {
                    if ((r.ecx >> 8) as u8) & 8 != 0 {
                        break 'at_67c58;
                    }
                    r.edx = r.esp.wrapping_add(0x24);
                    r.push(m, r.edx);
                    r.call(guest, 0x45390);
                    r.eax = r.esp.wrapping_add(0x28);
                    r.push(m, r.eax);
                    m.write32(r.esp.wrapping_add(0x2c), 0xd4a);
                    r.call(guest, 0x45820);
                    r.push(m, 2);
                    r.call(guest, 0x164a30);
                    m.write32(0x7de45c, m.read32(0x7de45c) | 0x800);
                    r.push(m, 0x16f2);
                } else {
                    break 'at_67c58;
                }
                r.call(guest, 0x3d7b0);
                r.edi = r.eax;
                r.esp = r.esp.wrapping_add(0x10);
                if r.edi != 0 {
                    r.esi = 0;
                    r.call(guest, 0x1cdae0);
                    if (r.eax as i32) > 0 {
                        loop {
                            r.push(m, 0);
                            r.push(m, r.edi);
                            r.push(m, 3);
                            r.push(m, r.esi);
                            r.call(guest, 0x12d350);
                            r.esp = r.esp.wrapping_add(0x10);
                            r.esi = r.esi.wrapping_add(1);
                            r.call(guest, 0x1cdae0);
                            if (r.esi as i32) >= (r.eax as i32) {
                                break 'at_67c58;
                            }
                        }
                    }
                }
            }
        }
    }
    r.edi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.esi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ebp = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ebx = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.esp = r.esp.wrapping_add(0x44);
}

/// 0x681c0. INFERRED: Updates match timers, countdown warning sounds, and end-of-match state.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_match_time_limit_countdown_warnings_sounds_0xd49_0xd4a_0xd48_and_end_of_match<
    M: GuestMemory,
    C: GuestCall,
>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    let mut jt;
    'at_685da: {
        'at_6843c: {
            r.eax = set_lo8(r.eax, gs::get_mode(m) as u32);
            r.esp = r.esp.wrapping_sub(0x30);
            r.push(m, r.ebx);
            r.push(m, r.esi);
            r.push(m, r.edi);
            if (r.eax as u8 as u32) & 0xff == 0xa {
                if (gs::get_flags_low8(m) as u32 & 0xff) & 0x40 == 0 {
                    break 'at_6843c;
                }
            }

            r.esi = 0;
            r.call(guest, 0x3533f0);
            if r.eax != 0 {
                r.call(guest, 0x355f10);
                if r.eax != 0 {
                    r.esi = 1;
                }
            }
            if (gs::get_mode(m) as u32 & 0xff) != 0xa {
                r.push(m, r.esi);
                r.call(guest, 0x64810);
                r.esp = r.esp.wrapping_add(4);
            }
            if gs::get_level(m) == 0x34 {
                r.call(guest, 0x225c70);
            }

            'at_68286: {
                'at_68280: {
                    r.eax = gs::get_flags(m);
                    if (r.eax as u8 as u32 & 0xff) & 0x40 == 0 {
                        if r.eax & 0x8000000 == 0 {
                            break 'at_68280;
                        }
                    }
                    r.eax = gs::get_players(m);
                    r.edi = 0;
                    if (r.eax as i32) > 0 {
                        r.ebx = m.read32(0x7de45c);
                        r.esi = 0;
                        loop {
                            r.eax = m.read32(0x7b0c48);
                            r.eax = r.eax.wrapping_add(r.esi);
                            m.write32(0x7b0c7c, r.eax);
                            if (m.read32(r.eax.wrapping_add(4)) as i32) > -1 {
                                r.call(guest, 0x231760);
                                r.call(guest, 0x231a20);
                                r.ebx = m.read32(0x7de45c);
                                if r.ebx & 0x18000 != 0 {
                                    break 'at_685da;
                                }
                            }
                            r.eax = gs::get_players(m);
                            r.edi = r.edi.wrapping_add(1);
                            r.esi = r.esi.wrapping_add(0x1584);
                            if (r.edi as i32) >= (r.eax as i32) {
                                break 'at_68286;
                            }
                        }
                    }
                }
                r.ebx = m.read32(0x7de45c);
            }

            r.eax = m.read32(0x7de460);
            if (r.eax as i32) > 0 {
                'at_6836c: {
                    'at_68345: {
                        r.edx = if (r.eax as i32) < 0 { u32::MAX } else { 0 };
                        let dividend = (((r.edx as i32 as i64) << 32) | r.eax as i64) as i64;
                        let divisor = m.read32(0x4e8cec) as i32 as i64;
                        r.eax = (dividend / divisor) as i32 as u32;
                        r.edx = (dividend % divisor) as i32 as u32;
                        if (r.eax as i32) < 0x3c && (r.ebx as u8 as u32 & 0xff) & 1 == 0 {
                            r.ecx = r.esp.wrapping_add(0xc);
                            r.push(m, r.ecx);
                            r.call(guest, 0x45390);
                            r.edx = r.esp.wrapping_add(0x10);
                            r.push(m, r.edx);
                            m.write32(r.esp.wrapping_add(0x14), 0xd49);
                            r.call(guest, 0x45820);
                            r.push(m, 0xe94);
                            r.call(guest, 0x3d7b0);
                            r.edi = r.eax;
                            r.eax = m.read32(0x7de45c);
                            r.esp = r.esp.wrapping_add(0xc);
                            r.eax |= 1;
                            m.write32(0x7de45c, r.eax);
                            break 'at_68345;
                        }

                        r.esi = 9;
                        loop {
                            r.ecx = r.esi.wrapping_add(1);
                            if (r.eax as i32) < (r.ecx as i32) {
                                r.edx = 2;
                                r.ecx = r.esi;
                                r.edx = r.edx.wrapping_shl((r.ecx as u8 as u32 & 0x1f) & 31);
                                if r.ebx & r.edx == 0 {
                                    r.eax = r.esp.wrapping_add(0xc);
                                    r.push(m, r.eax);
                                    r.call(guest, 0x45390);
                                    r.ecx = r.esp.wrapping_add(0x10);
                                    r.push(m, r.ecx);
                                    m.write32(r.esp.wrapping_add(0x14), 0xd4a);
                                    r.call(guest, 0x45820);
                                    r.edx = r.esi.wrapping_add(1);
                                    r.esp = r.esp.wrapping_add(8);
                                    if r.edx != 0xa {
                                        r.edi = 0;
                                    } else {
                                        r.push(m, 0xe95);
                                        r.call(guest, 0x3d7b0);
                                        r.esp = r.esp.wrapping_add(4);
                                        r.edi = r.eax;
                                    }
                                    r.ecx = r.esi;
                                    r.eax = 2;
                                    r.eax = r.eax.wrapping_shl((r.ecx as u8 as u32 & 0x1f) & 31);
                                    m.write32(0x7de45c, m.read32(0x7de45c) | r.eax);
                                    break 'at_68345;
                                }
                            }
                            r.esi = r.esi.wrapping_sub(1);
                            if (r.esi as i32) < 0 {
                                break 'at_6836c;
                            }
                        }
                    }

                    if r.edi != 0 {
                        r.esi = 0;
                        r.call(guest, 0x1cdae0);
                        if (r.eax as i32) > 0 {
                            loop {
                                r.push(m, 0);
                                r.push(m, r.edi);
                                r.push(m, 3);
                                r.push(m, r.esi);
                                r.call(guest, 0x12d350);
                                r.esp = r.esp.wrapping_add(0x10);
                                r.esi = r.esi.wrapping_add(1);
                                r.call(guest, 0x1cdae0);
                                if (r.esi as i32) >= (r.eax as i32) {
                                    break 'at_6836c;
                                }
                            }
                        }
                    }
                }

                if gs::get_match_state(m) != 5 {
                    r.eax = m.read32(0x7de460);
                    r.ecx = m.read32(0x4e7944);
                    r.eax = r.eax.wrapping_sub(r.ecx);
                    m.write32(0x7de460, r.eax);
                }
                r.eax = m.read32(0x7de460);
                if (r.eax as i32) <= 0 {
                    r.edx = r.esp.wrapping_add(0xc);
                    r.push(m, r.edx);
                    r.call(guest, 0x45390);
                    r.eax = r.esp.wrapping_add(0x10);
                    r.push(m, r.eax);
                    m.write32(r.esp.wrapping_add(0x14), 0xd48);
                    r.call(guest, 0x45820);
                    r.esp = r.esp.wrapping_add(8);
                    r.esi = 0;
                    r.call(guest, 0x1cdae0);
                    'at_683e5: {
                        if (r.eax as i32) > 0 {
                            loop {
                                r.push(m, 0);
                                r.push(m, 0x16f7);
                                r.call(guest, 0x227c30);
                                r.esp = r.esp.wrapping_add(4);
                                r.push(m, r.eax);
                                r.push(m, 4);
                                r.push(m, r.esi);
                                r.call(guest, 0x12d350);
                                r.esp = r.esp.wrapping_add(0x10);
                                r.esi = r.esi.wrapping_add(1);
                                r.call(guest, 0x1cdae0);
                                if (r.esi as i32) >= (r.eax as i32) {
                                    break 'at_683e5;
                                }
                            }
                        }
                    }
                    r.call(guest, 0x355f10);
                    'at_68432: {
                        if r.eax != 0 {
                            r.call(guest, 0x357690);
                            if r.eax == 0 {
                                break 'at_68432;
                            }
                        }
                        r.eax = gs::get_flags(m);
                        if (r.eax as u8 as u32 & 0xff) & 0x40 == 0 && r.eax & 0x8000000 == 0 {
                            r.call(guest, 0x66fc0);
                            break 'at_68432;
                        }
                        r.ecx = gs::get_match_state(m);
                        r.eax = 8;
                        if r.ecx != r.eax {
                            r.xmm[0] = Xmm::default();
                            gs::set_match_state(m, r.eax);
                            m.write32(0x7de300, r.xmm[0].f(0).to_bits());
                        }
                        r.call(guest, 0x231a20);
                    }
                    r.push(m, 8);
                    r.call(guest, 0x164a30);
                    r.esp = r.esp.wrapping_add(4);
                }
            }
        }

        'at_6852f: {
            r.eax = gs::get_flags(m);
            if r.eax & 0x10000000 != 0 && r.eax & 0x60000000 == 0 {
                r.eax = m.read32(0x7de3a0);
                if (r.eax as i32) > 0 {
                    r.eax = m.read32(0x6d60e8);
                    if r.eax == 0 {
                        r.eax = m.read32(0x7de460);
                        r.eax = r.eax.wrapping_sub(m.read32(0x4e7944));
                        m.write32(0x7de460, r.eax);
                        if (r.eax as i32) <= 0 {
                            r.ecx = r.esp.wrapping_add(0xc);
                            r.push(m, r.ecx);
                            r.call(guest, 0x45390);
                            r.edx = r.esp.wrapping_add(0x10);
                            r.push(m, r.edx);
                            m.write32(r.esp.wrapping_add(0x14), 0xd48);
                            r.call(guest, 0x45820);
                            r.esp = r.esp.wrapping_add(8);
                            r.esi = 0;
                            r.call(guest, 0x1cdae0);
                            'at_684d8: {
                                if (r.eax as i32) > 0 {
                                    loop {
                                        r.push(m, 0);
                                        r.push(m, 0x16f8);
                                        r.call(guest, 0x3d7b0);
                                        r.esp = r.esp.wrapping_add(4);
                                        r.push(m, r.eax);
                                        r.push(m, 4);
                                        r.push(m, r.esi);
                                        r.call(guest, 0x12d350);
                                        r.esp = r.esp.wrapping_add(0x10);
                                        r.esi = r.esi.wrapping_add(1);
                                        r.call(guest, 0x1cdae0);
                                        if (r.esi as i32) >= (r.eax as i32) {
                                            break 'at_684d8;
                                        }
                                    }
                                }
                            }
                            m.write32(0x7de45c, m.read32(0x7de45c) | 0x10000);
                            r.call(guest, 0x355f10);
                            if r.eax != 0 {
                                r.call(guest, 0x357690);
                                if r.eax == 0 {
                                    break 'at_6852f;
                                }
                            }
                            r.eax = gs::get_flags(m);
                            if (r.eax as u8 as u32 & 0xff) & 0x40 == 0 && r.eax & 0x8000000 == 0 {
                                r.call(guest, 0x66fc0);
                                break 'at_6852f;
                            }
                            r.ecx = gs::get_match_state(m);
                            r.eax = 8;
                            if r.ecx != r.eax {
                                r.xmm[0] = Xmm::default();
                                gs::set_match_state(m, r.eax);
                                m.write32(0x7de300, r.xmm[0].f(0).to_bits());
                            }
                            r.call(guest, 0x231a20);
                        }
                    }
                }
            }
        }

        r.eax = gs::get_mode(m) as u8 as u32;
        r.eax = r.eax.wrapping_sub(1);
        if r.eax <= 9 {
            jt = m.read32(r.eax.wrapping_mul(4).wrapping_add(0x685e4));
            if jt == 0x68547 {
                r.call(guest, 0x61f30);
                r.call(guest, 0x2382a0);
                call_story_mission_tick_or_lifted!(r, guest);
                r.edi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.ebx = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esp = r.esp.wrapping_add(0x30);
                return;
            } else if jt == 0x6855d {
                r.call(guest, 0x674c0);
                r.call(guest, 0x2382a0);
                call_story_mission_tick_or_lifted!(r, guest);
                r.edi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.ebx = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esp = r.esp.wrapping_add(0x30);
                return;
            } else if jt == 0x68573 {
                r.call(guest, 0x67cd0);
                r.call(guest, 0x2382a0);
                call_story_mission_tick_or_lifted!(r, guest);
                r.edi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.ebx = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esp = r.esp.wrapping_add(0x30);
                return;
            } else if jt == 0x68589 {
                r.call(guest, 0x67e60);
                r.call(guest, 0x2382a0);
                call_story_mission_tick_or_lifted!(r, guest);
                r.edi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.ebx = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esp = r.esp.wrapping_add(0x30);
                return;
            } else if jt == 0x6859f {
                r.call(guest, 0x654e0);
                r.call(guest, 0x2382a0);
                call_story_mission_tick_or_lifted!(r, guest);
                r.edi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.ebx = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esp = r.esp.wrapping_add(0x30);
                return;
            } else if jt == 0x685b5 {
                r.call(guest, 0x67f00);
                r.call(guest, 0x2382a0);
                call_story_mission_tick_or_lifted!(r, guest);
                r.edi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.ebx = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esp = r.esp.wrapping_add(0x30);
                return;
            } else if jt == 0x685cb {
                r.call(guest, 0x65590);
            } else if jt != 0x685d0 {
                r.tail_call(guest, jt);
                return;
            }
        }
        r.call(guest, 0x2382a0);
        call_story_mission_tick_or_lifted!(r, guest);
    }

    r.edi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.esi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ebx = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.esp = r.esp.wrapping_add(0x30);
}

/// 0x68830. INFERRED: Dispatches match-state updates through a jump table.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_match_state_machine_tick_dispatch_by_7de30c_jump_table_12_states<
    M: GuestMemory,
    C: GuestCall,
>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    let (mut fa,) = (0,);
    let (mut fpcmp,) = (0i32,);
    let (mut fca, mut fcb) = (0f64, 0f64);
    let (mut jt,) = (0,);
    'at_6957f: {
        r.push(m, r.ebp);
        r.ebp = r.esp;
        r.esp = r.esp & 0xfffffff8u32;
        r.esp = r.esp.wrapping_sub(0xb0);
        r.eax = gs::get_match_state(m);
        r.xmm[1].set_f(0, (m.read32(0x4e8cec) as i32) as f32);
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x475c78)).to_bits());
        r.push(m, r.ebx);
        r.push(m, r.ebp);
        r.xmm[0].set_f(0, r.xmm[0].f(0) / r.xmm[1].f(0));
        r.xmm[0].set_f(0, r.xmm[0].f(0) * f32::from_bits(m.read32(0x4e7948)));
        r.xmm[0].set_f(0, r.xmm[0].f(0) + f32::from_bits(m.read32(0x7de300)));
        r.eax = r.eax.wrapping_sub(1);
        r.push(m, r.esi);
        r.push(m, r.edi);
        m.write32(0x7de300, r.xmm[0].f(0).to_bits());
        if r.eax <= 0xb {
            'at_688b0: {
                jt = m.read32(r.eax.wrapping_mul(4).wrapping_add(0x69588));
                if jt == 0x68882 {
                    r.eax = gs::get_level(m);
                    if r.eax != 0x68 {
                        if r.eax != 0x69 {
                            if (gs::get_flags(m) & 0x20000000) == 0 {
                                break 'at_688b0;
                            }
                            r.call(guest, 0x65fe0);
                            if r.eax == 0 {
                                break 'at_688b0;
                            }
                            r.push(m, 0);
                            r.call(guest, 0x98db0);
                            r.esp = r.esp.wrapping_add(4);
                            break 'at_688b0;
                        }
                    }
                    r.call(guest, 0x65470);
                    r.edi = m.read32(r.esp);
                    r.esp = r.esp.wrapping_add(4);
                    r.esi = m.read32(r.esp);
                    r.esp = r.esp.wrapping_add(4);
                    r.ebp = m.read32(r.esp);
                    r.esp = r.esp.wrapping_add(4);
                    r.ebx = m.read32(r.esp);
                    r.esp = r.esp.wrapping_add(4);
                    r.esp = r.ebp;
                    r.ebp = m.read32(r.esp);
                    r.esp = r.esp.wrapping_add(4);
                    return;
                } else {
                    if jt != 0x688da {
                        if jt == 0x688e1 {
                            r.push(m, 0);
                            r.call(guest, 0x2e68c0);
                            r.esp = r.esp.wrapping_add(4);
                            if r.eax == 0 {
                                r.call(guest, 0x3533f0);
                                if r.eax != 0 {
                                    'at_68976: {
                                        r.eax = gs::get_players(m);
                                        r.edi = 0;
                                        if (r.eax as i32) > 0i32 {
                                            r.ecx = m.read32(0x7b0c48);
                                            r.esi = 0;
                                            r.ebx = 0xfffffffeu32;
                                            loop {
                                                'at_68966: {
                                                    'at_68960: {
                                                        r.eax = m.read32(
                                                            r.esi
                                                                .wrapping_add(r.ecx)
                                                                .wrapping_add(4),
                                                        );
                                                        if r.eax == 0 {
                                                            r.eax = m.read32(
                                                                r.esi
                                                                    .wrapping_add(r.ecx)
                                                                    .wrapping_add(8),
                                                            );
                                                            if r.eax != 0 {
                                                                r.ecx = m.read32(0x4e8cf4);
                                                                r.edx = m.read32(0x4e8cf0);
                                                                r.ecx = r.ecx.wrapping_sub(1);
                                                                r.push(m, r.ecx);
                                                                r.edx = r.edx.wrapping_sub(1);
                                                                r.push(m, r.edx);
                                                                r.push(m, 0);
                                                                r.eax = r.eax.wrapping_add(0x224);
                                                                r.push(m, 0);
                                                                r.push(m, r.eax);
                                                                r.call(guest, 0x55d90);
                                                                r.esp = r.esp.wrapping_add(0x14);
                                                                break 'at_68960;
                                                            }
                                                        }
                                                        r.edx = m.read32(
                                                            r.esi
                                                                .wrapping_add(r.ecx)
                                                                .wrapping_add(8),
                                                        );
                                                        r.eax = r
                                                            .esi
                                                            .wrapping_add(r.ecx)
                                                            .wrapping_add(8);
                                                        if r.edx == 0 {
                                                            break 'at_68966;
                                                        }
                                                        r.eax = r.edx;
                                                        m.write32(
                                                            r.eax.wrapping_add(0x21c),
                                                            m.read32(r.eax.wrapping_add(0x21c))
                                                                & r.ebx,
                                                        );
                                                    }
                                                    r.ecx = m.read32(0x7b0c48);
                                                }
                                                r.eax = gs::get_players(m);
                                                r.edi = r.edi.wrapping_add(1);
                                                r.esi = r.esi.wrapping_add(0x1584);
                                                if (r.edi as i32) >= (r.eax as i32) {
                                                    break 'at_68976;
                                                }
                                            }
                                        }
                                    }
                                    r.push(m, 0);
                                    r.call(guest, 0x2e68e0);
                                    r.esp = r.esp.wrapping_add(4);
                                    r.call(guest, 0x2ec250);
                                }
                            }
                            r.eax = set_lo8(r.eax, gs::get_mode(m) as u32);
                            if (((r.eax as u8) as u32) & 0xff) != (0xa & 0xff) {
                                if (((r.eax as u8) as u32) & 0xff) != (4 & 0xff) {
                                    'at_689d2: {
                                        r.eax = m.read32(0x74c37c);
                                        r.edx = 0;
                                        if r.eax as i32 > 0i32 {
                                            r.ecx = 0;
                                            r.esi = 0xffffff9eu32;
                                            loop {
                                                r.eax = m.read32(0x7356d8);
                                                r.eax = r.eax.wrapping_add(r.ecx);
                                                if r.eax != 0 {
                                                    r.eax = m.read32(r.eax);
                                                    if r.eax != 0 {
                                                        m.write32(
                                                            r.eax.wrapping_add(0x28),
                                                            m.read32(r.eax.wrapping_add(0x28))
                                                                & r.esi,
                                                        );
                                                    }
                                                }
                                                r.eax = m.read32(0x74c37c);
                                                r.edx = r.edx.wrapping_add(1);
                                                r.ecx = r.ecx.wrapping_add(0xea0);
                                                if (r.edx as i32) >= (r.eax as i32) {
                                                    break 'at_689d2;
                                                }
                                            }
                                        }
                                    }
                                    r.call(guest, 0x357bd0);
                                    r.esi = r.eax;
                                    r.call(guest, 0x3533f0);
                                    'at_68cc9: {
                                        'at_68a1e: {
                                            r.ebx = 1;
                                            if r.eax != 0 {
                                                if r.esi != 0 {
                                                    r.xmm[0] = Xmm::scalar(
                                                        f32::from_bits(m.read32(0x7de300))
                                                            .to_bits(),
                                                    );
                                                    fca = f32::from_bits(r.xmm[0].lanes[0]) as f64;
                                                    fcb = f32::from_bits(m.read32(0x4760cc)) as f64;
                                                    if fca > fcb {
                                                        break 'at_68a1e;
                                                    }
                                                }
                                                r.call(guest, 0x358e40);
                                                if r.eax == 0 {
                                                    break 'at_68cc9;
                                                }
                                                r.xmm[0] = Xmm::scalar(
                                                    f32::from_bits(m.read32(0x7de300)).to_bits(),
                                                );
                                                fca = f32::from_bits(r.xmm[0].lanes[0]) as f64;
                                                fcb = f32::from_bits(m.read32(0x475d10)) as f64;
                                                if (fca <= fcb) || ((fca != fca) || (fcb != fcb)) {
                                                    break 'at_68cc9;
                                                }
                                            }
                                        }
                                        r.call(guest, 0x3533f0);
                                        if r.eax != 0 {
                                            r.call(guest, 0x352bb0);
                                            'at_68b55: {
                                                if r.eax != 0 {
                                                    r.ecx = 0;
                                                    r.edx = 0x777368;
                                                    r.ebx = set_lo8(r.ebx, 0x10);
                                                    r.edi = 0x61;
                                                    r.esi = r.esi | 0xffffffffu32;
                                                    loop {
                                                        r.eax = m.read32(r.edx.wrapping_add(
                                                            (0x124i32.wrapping_neg()) as u32,
                                                        ));
                                                        if r.eax != 0 {
                                                            r.eax = m.read32(r.eax);
                                                            r.eax = ((r.eax as i32)
                                                                .wrapping_mul(0x30ci32))
                                                                as u32;
                                                            if (((m
                                                                .read8(r.eax.wrapping_add(0x7741b4))
                                                                as u32)
                                                                & 0xff)
                                                                & (((r.ebx as u8) as u32) & 0xff))
                                                                != 0
                                                            {
                                                                r.eax = m.read32(0x7356d8);
                                                                r.eax = r.eax.wrapping_add(r.ecx);
                                                                if r.eax != 0 {
                                                                    r.eax = m.read32(r.eax);
                                                                    if r.eax != 0 {
                                                                        m.write32(
                                                                            r.eax
                                                                                .wrapping_add(0x28),
                                                                            m.read32(
                                                                                r.eax.wrapping_add(
                                                                                    0x28,
                                                                                ),
                                                                            ) | r.edi,
                                                                        );
                                                                        m.write32(
                                                                            r.eax.wrapping_add(
                                                                                0x258,
                                                                            ),
                                                                            r.esi,
                                                                        );
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        r.eax = m.read32(r.edx);
                                                        if r.eax != 0 {
                                                            r.eax = m.read32(r.eax);
                                                            r.eax = ((r.eax as i32)
                                                                .wrapping_mul(0x30ci32))
                                                                as u32;
                                                            if (((m
                                                                .read8(r.eax.wrapping_add(0x7741b4))
                                                                as u32)
                                                                & 0xff)
                                                                & (((r.ebx as u8) as u32) & 0xff))
                                                                != 0
                                                            {
                                                                r.eax = m.read32(0x7356d8);
                                                                r.eax = r
                                                                    .ecx
                                                                    .wrapping_add(r.eax)
                                                                    .wrapping_add(0xea0);
                                                                if r.eax != 0 {
                                                                    r.eax = m.read32(r.eax);
                                                                    if r.eax != 0 {
                                                                        m.write32(
                                                                            r.eax
                                                                                .wrapping_add(0x28),
                                                                            m.read32(
                                                                                r.eax.wrapping_add(
                                                                                    0x28,
                                                                                ),
                                                                            ) | r.edi,
                                                                        );
                                                                        m.write32(
                                                                            r.eax.wrapping_add(
                                                                                0x258,
                                                                            ),
                                                                            r.esi,
                                                                        );
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        r.eax = m.read32(r.edx.wrapping_add(0x124));
                                                        if r.eax != 0 {
                                                            r.eax = m.read32(r.eax);
                                                            r.eax = ((r.eax as i32)
                                                                .wrapping_mul(0x30ci32))
                                                                as u32;
                                                            if (((m
                                                                .read8(r.eax.wrapping_add(0x7741b4))
                                                                as u32)
                                                                & 0xff)
                                                                & (((r.ebx as u8) as u32) & 0xff))
                                                                != 0
                                                            {
                                                                r.eax = m.read32(0x7356d8);
                                                                r.eax = r
                                                                    .ecx
                                                                    .wrapping_add(r.eax)
                                                                    .wrapping_add(0x1d40);
                                                                if r.eax != 0 {
                                                                    r.eax = m.read32(r.eax);
                                                                    if r.eax != 0 {
                                                                        m.write32(
                                                                            r.eax
                                                                                .wrapping_add(0x28),
                                                                            m.read32(
                                                                                r.eax.wrapping_add(
                                                                                    0x28,
                                                                                ),
                                                                            ) | r.edi,
                                                                        );
                                                                        m.write32(
                                                                            r.eax.wrapping_add(
                                                                                0x258,
                                                                            ),
                                                                            r.esi,
                                                                        );
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        r.eax = m.read32(r.edx.wrapping_add(0x248));
                                                        if r.eax != 0 {
                                                            r.eax = m.read32(r.eax);
                                                            r.eax = ((r.eax as i32)
                                                                .wrapping_mul(0x30ci32))
                                                                as u32;
                                                            if (((m
                                                                .read8(r.eax.wrapping_add(0x7741b4))
                                                                as u32)
                                                                & 0xff)
                                                                & (((r.ebx as u8) as u32) & 0xff))
                                                                != 0
                                                            {
                                                                r.eax = m.read32(0x7356d8);
                                                                r.eax = r
                                                                    .ecx
                                                                    .wrapping_add(r.eax)
                                                                    .wrapping_add(0x2be0);
                                                                if r.eax != 0 {
                                                                    r.eax = m.read32(r.eax);
                                                                    if r.eax != 0 {
                                                                        m.write32(
                                                                            r.eax
                                                                                .wrapping_add(0x28),
                                                                            m.read32(
                                                                                r.eax.wrapping_add(
                                                                                    0x28,
                                                                                ),
                                                                            ) | r.edi,
                                                                        );
                                                                        m.write32(
                                                                            r.eax.wrapping_add(
                                                                                0x258,
                                                                            ),
                                                                            r.esi,
                                                                        );
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        r.edx = r.edx.wrapping_add(0x490);
                                                        r.ecx = r.ecx.wrapping_add(0x3a80);
                                                        if (r.edx as i32) >= 0x7785a8 {
                                                            r.ecx = m.read32(0x774028);
                                                            r.push(m, r.ecx);
                                                            r.call(guest, 0x359aa0);
                                                            r.esp = r.esp.wrapping_add(4);
                                                            r.call(guest, 0x3526d0);
                                                            break 'at_68b55;
                                                        }
                                                    }
                                                }
                                            }
                                            r.ecx = gs::get_match_state(m);
                                            r.eax = 6;
                                            if r.ecx == r.eax {
                                                break 'at_6957f;
                                            }
                                            r.xmm[0] = Xmm::default();
                                            gs::set_match_state(m, r.eax);
                                            m.write32(0x7de300, r.xmm[0].f(0).to_bits());
                                            r.edi = m.read32(r.esp);
                                            r.esp = r.esp.wrapping_add(4);
                                            r.esi = m.read32(r.esp);
                                            r.esp = r.esp.wrapping_add(4);
                                            r.ebp = m.read32(r.esp);
                                            r.esp = r.esp.wrapping_add(4);
                                            r.ebx = m.read32(r.esp);
                                            r.esp = r.esp.wrapping_add(4);
                                            r.esp = r.ebp;
                                            r.ebp = m.read32(r.esp);
                                            r.esp = r.esp.wrapping_add(4);
                                            return;
                                        }
                                        'at_68bb4: {
                                            r.eax = m.read32(0x74c37c);
                                            r.edx = 0;
                                            if r.eax as i32 > 0i32 {
                                                r.ecx = 0;
                                                r.esi = 0x61;
                                                loop {
                                                    r.eax = m.read32(0x7356d8);
                                                    r.eax = r.eax.wrapping_add(r.ecx);
                                                    if r.eax != 0 {
                                                        r.eax = m.read32(r.eax);
                                                        if r.eax != 0 {
                                                            m.write32(
                                                                r.eax.wrapping_add(0x28),
                                                                m.read32(r.eax.wrapping_add(0x28))
                                                                    | r.esi,
                                                            );
                                                        }
                                                    }
                                                    r.eax = m.read32(0x74c37c);
                                                    r.edx = r.edx.wrapping_add(1);
                                                    r.ecx = r.ecx.wrapping_add(0xea0);
                                                    if (r.edx as i32) >= (r.eax as i32) {
                                                        break 'at_68bb4;
                                                    }
                                                }
                                            }
                                        }
                                        r.push(m, 0);
                                        r.call(guest, 0xc6390);
                                        'at_68c82: {
                                            r.eax = gs::get_players(m);
                                            r.esp = r.esp.wrapping_add(4);
                                            r.ebp = 0;
                                            if r.eax as i32 > 0i32 {
                                                r.eax = m.read32(0x7b0c48);
                                                r.esi = 0;
                                                loop {
                                                    r.edi = m.read32(
                                                        r.esi.wrapping_add(r.eax).wrapping_add(4),
                                                    );
                                                    if r.edi as i32 >= 0 {
                                                        r.ecx = m.read32(
                                                            r.esi
                                                                .wrapping_add(r.eax)
                                                                .wrapping_add(8),
                                                        );
                                                        if r.ecx != 0 {
                                                            r.ecx = m.read32(
                                                                r.esi
                                                                    .wrapping_add(r.eax)
                                                                    .wrapping_add(0x14),
                                                            );
                                                            r.edx =
                                                                m.read32(r.ecx.wrapping_add(0x7c));
                                                            r.edx = r.edx.wrapping_add(0xc68);
                                                            r.push(m, 0);
                                                            r.push(m, r.edx);
                                                            r.call(guest, 0x125f50);
                                                            r.eax = m.read32(0x7b0c48);
                                                            r.ecx = m.read32(
                                                                r.esi
                                                                    .wrapping_add(r.eax)
                                                                    .wrapping_add(8),
                                                            );
                                                            r.push(m, 0);
                                                            r.push(m, r.ecx);
                                                            r.call(guest, 0xc63f0);
                                                            r.eax = m.read32(0x7b0c48);
                                                            r.edx = m.read32(
                                                                r.esi
                                                                    .wrapping_add(r.eax)
                                                                    .wrapping_add(0x14),
                                                            );
                                                            r.eax = m.read32(
                                                                r.esi
                                                                    .wrapping_add(r.eax)
                                                                    .wrapping_add(8),
                                                            );
                                                            r.push(m, r.edx);
                                                            r.push(m, 0);
                                                            r.push(m, r.eax);
                                                            r.call(guest, 0xcbcf0);
                                                            r.eax = m.read32(0x7b0c48);
                                                            r.ecx = m.read32(
                                                                r.esi
                                                                    .wrapping_add(r.eax)
                                                                    .wrapping_add(8),
                                                            );
                                                            r.edx = m.read32(
                                                                r.esi
                                                                    .wrapping_add(r.eax)
                                                                    .wrapping_add(0x14),
                                                            );
                                                            m.write32(
                                                                r.ecx.wrapping_add(0x2a8),
                                                                r.edx,
                                                            );
                                                            r.eax = m.read32(0x7b0c48);
                                                            r.ecx = m.read32(
                                                                r.esi
                                                                    .wrapping_add(r.eax)
                                                                    .wrapping_add(8),
                                                            );
                                                            r.ecx = r.ecx.wrapping_add(0x224);
                                                            r.push(m, r.ecx);
                                                            r.push(m, r.edi);
                                                            r.call(guest, 0x25a570);
                                                            r.esp = r.esp.wrapping_add(0x24);
                                                            if r.edi != 0 {
                                                                r.edx = m.read32(0x7b0c48);
                                                                r.eax = m.read32(
                                                                    r.esi
                                                                        .wrapping_add(r.edx)
                                                                        .wrapping_add(8),
                                                                );
                                                                m.write32(
                                                                    r.eax.wrapping_add(0x21c),
                                                                    m.read32(
                                                                        r.eax.wrapping_add(0x21c),
                                                                    ) | r.ebx,
                                                                );
                                                            }
                                                            r.eax = m.read32(0x7b0c48);
                                                        }
                                                    }
                                                    r.ecx = gs::get_players(m);
                                                    r.ebp = r.ebp.wrapping_add(1);
                                                    r.esi = r.esi.wrapping_add(0x1584);
                                                    if (r.ebp as i32) >= (r.ecx as i32) {
                                                        break 'at_68c82;
                                                    }
                                                }
                                            }
                                        }
                                        'at_68cbe: {
                                            if (((gs::get_flags_low8(m) as u32) & 0xff)
                                                & (0x40 & 0xff))
                                                != 0
                                            {
                                                r.call(guest, 0x22dec0);
                                                if r.eax == 0 {
                                                    r.ecx = gs::get_match_state(m);
                                                    r.eax = 7;
                                                    if r.ecx == r.eax {
                                                        break 'at_68cc9;
                                                    }
                                                    gs::set_match_state(m, r.eax);
                                                    break 'at_68cbe;
                                                }
                                            }
                                            r.eax = gs::get_match_state(m);
                                            r.edi = 5;
                                            if r.eax == r.edi {
                                                break 'at_68cc9;
                                            }
                                            gs::set_match_state(m, r.edi);
                                        }
                                        r.xmm[0] = Xmm::default();
                                        m.write32(0x7de300, r.xmm[0].f(0).to_bits());
                                    }
                                    if (((gs::get_flags_low8(m) as u32) & 0xff) & (0x40 & 0xff))
                                        != 0
                                    {
                                        r.call(guest, 0x22dec0);
                                        if r.eax == 0 {
                                            break 'at_6957f;
                                        }
                                    }
                                    m.write32(0x6f11bc, r.ebx);
                                    r.edi = m.read32(r.esp);
                                    r.esp = r.esp.wrapping_add(4);
                                    r.esi = m.read32(r.esp);
                                    r.esp = r.esp.wrapping_add(4);
                                    r.ebp = m.read32(r.esp);
                                    r.esp = r.esp.wrapping_add(4);
                                    r.ebx = m.read32(r.esp);
                                    r.esp = r.esp.wrapping_add(4);
                                    r.esp = r.ebp;
                                    r.ebp = m.read32(r.esp);
                                    r.esp = r.esp.wrapping_add(4);
                                    return;
                                }
                            }
                            r.call(guest, 0x667b0);
                            if (((gs::get_flags_low8(m) as u32) & 0xff) & (0x40 & 0xff)) != 0 {
                                r.call(guest, 0x22dec0);
                                if r.eax != 0 {
                                    r.push(m, 5);
                                    r.call(guest, 0x65230);
                                    r.esp = r.esp.wrapping_add(4);
                                    m.write32(0x6f11bc, 1);
                                    r.edi = m.read32(r.esp);
                                    r.esp = r.esp.wrapping_add(4);
                                    r.esi = m.read32(r.esp);
                                    r.esp = r.esp.wrapping_add(4);
                                    r.ebp = m.read32(r.esp);
                                    r.esp = r.esp.wrapping_add(4);
                                    r.ebx = m.read32(r.esp);
                                    r.esp = r.esp.wrapping_add(4);
                                    r.esp = r.ebp;
                                    r.ebp = m.read32(r.esp);
                                    r.esp = r.esp.wrapping_add(4);
                                    return;
                                }
                            }
                            r.push(m, 7);
                            r.call(guest, 0x65230);
                            r.esp = r.esp.wrapping_add(4);
                            r.edi = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            r.esi = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            r.ebp = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            r.ebx = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            r.esp = r.ebp;
                            r.ebp = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            return;
                        } else {
                            'at_68f83: {
                                if jt == 0x68d32 {
                                    'at_68d6f: {
                                        r.eax = set_lo8(r.eax, gs::get_flags_low8(m) as u32);
                                        r.ebp = 0;
                                        r.ebx = set_lo8(r.ebx, 0x10);
                                        if ((((r.eax as u8) as u32) & 0xff) & (1 & 0xff)) == 0 {
                                            r.eax = m.read32(0x774028);
                                            r.ebp = r.ebp | 0xffffffffu32;
                                        } else {
                                            r.call(guest, 0x2edd90);
                                            fa = r.eax;
                                            r.eax = m.read32(0x774028);
                                            if fa != 0 {
                                                r.ecx = m.read32(r.eax.wrapping_add(0xc));
                                                r.ebp = m.read32(r.ecx.wrapping_add(0x48));
                                                if r.ebp != 0 {
                                                    break 'at_68d6f;
                                                }
                                            }
                                            r.edx = m.read32(r.eax);
                                            r.edx = ((r.edx as i32).wrapping_mul(0x30ci32)) as u32;
                                            if (((m.read8(r.edx.wrapping_add(0x7741b4)) as u32)
                                                & 0xff)
                                                & (((r.ebx as u8) as u32) & 0xff))
                                                == 0
                                            {
                                                break 'at_68f83;
                                            }
                                        }
                                    }
                                    'at_68de5: {
                                        m.write8(
                                            r.eax.wrapping_add(0x34),
                                            (((m.read8(r.eax.wrapping_add(0x34)) as i32)
                                                | ((r.ebx as u8) as i32))
                                                as u32)
                                                as u8,
                                        );
                                        r.eax = m.read32(0x74c37c);
                                        r.esi = 0;
                                        m.write32(r.esp.wrapping_add(0x10), 1);
                                        if r.eax as i32 > 0i32 {
                                            r.edi = 0;
                                            r.ebx = 0x61;
                                            loop {
                                                r.eax = m.read32(0x7356d8);
                                                r.eax = r.eax.wrapping_add(r.edi);
                                                if r.eax != 0 {
                                                    r.ecx = m.read32(r.eax);
                                                    if (r.ebp as i32) > 0i32 {
                                                        if r.ecx != 0 {
                                                            r.edx =
                                                                m.read32(r.ecx.wrapping_add(0x150));
                                                            if r.edx != 0 {
                                                                if (m.read32(r.edx.wrapping_add(4))
                                                                    as i32)
                                                                    >= 0
                                                                {
                                                                    m.write32(
                                                                        r.ecx.wrapping_add(0x28),
                                                                        m.read32(
                                                                            r.ecx
                                                                                .wrapping_add(0x28),
                                                                        ) | r.ebx,
                                                                    );
                                                                    r.ecx = m.read32(
                                                                        r.eax.wrapping_add(8),
                                                                    );
                                                                    r.edx = r.ecx;
                                                                    r.edx = ((r.edx as i32)
                                                                        .wrapping_mul(0x124i32))
                                                                        as u32;
                                                                    r.edx = m.read32(
                                                                        r.edx
                                                                            .wrapping_add(0x777288),
                                                                    );
                                                                    m.write32(
                                                                        r.eax.wrapping_add(0x84),
                                                                        r.edx,
                                                                    );
                                                                    r.eax = set_lo8(
                                                                        r.eax,
                                                                        (r.edx as u8) as u32,
                                                                    );
                                                                    m.write8(
                                                                        r.ecx
                                                                            .wrapping_add(0x7de424),
                                                                        ((r.eax as u8) as u32)
                                                                            as u8,
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                r.eax = m.read32(0x74c37c);
                                                r.esi = r.esi.wrapping_add(1);
                                                r.edi = r.edi.wrapping_add(0xea0);
                                                if (r.esi as i32) >= (r.eax as i32) {
                                                    break 'at_68de5;
                                                }
                                            }
                                        }
                                    }
                                    r.push(m, 0);
                                    r.call(guest, 0xc6390);
                                    'at_68f32: {
                                        r.eax = gs::get_players(m);
                                        r.esp = r.esp.wrapping_add(4);
                                        r.ebx = 0;
                                        if r.eax as i32 > 0i32 {
                                            r.esi = 0;
                                            r.ebp = 0x77728c;
                                            loop {
                                                r.ecx = m.read32(0x7b0c48);
                                                r.edi = m.read32(
                                                    r.esi.wrapping_add(r.ecx).wrapping_add(4),
                                                );
                                                r.call(guest, 0x355ec0);
                                                if r.eax != 0 {
                                                    if (((m.read8(r.ebp) as u32) & 0xff)
                                                        & (2 & 0xff))
                                                        != 0
                                                    {
                                                        r.edx = m.read32(0x7b0c48);
                                                        r.push(m, 0xcaf);
                                                        r.eax = r.esi.wrapping_add(r.edx);
                                                        r.push(m, 0x47889c);
                                                        r.push(m, r.eax);
                                                        r.call(guest, 0xd0ce0);
                                                        r.esp = r.esp.wrapping_add(0xc);
                                                    }
                                                }
                                                if (r.edi as i32) >= 0 {
                                                    r.eax = m.read32(0x7b0c48);
                                                    r.ecx = m.read32(
                                                        r.esi.wrapping_add(r.eax).wrapping_add(8),
                                                    );
                                                    if r.ecx != 0 {
                                                        r.ecx = m.read32(
                                                            r.esi
                                                                .wrapping_add(r.eax)
                                                                .wrapping_add(0x14),
                                                        );
                                                        r.edx = m.read32(r.ecx.wrapping_add(0x7c));
                                                        r.edx = r.edx.wrapping_add(0xc68);
                                                        r.push(m, 0);
                                                        r.push(m, r.edx);
                                                        r.call(guest, 0x125f50);
                                                        r.eax = m.read32(0x7b0c48);
                                                        r.ecx = m.read32(
                                                            r.esi
                                                                .wrapping_add(r.eax)
                                                                .wrapping_add(8),
                                                        );
                                                        r.push(m, 0);
                                                        r.push(m, r.ecx);
                                                        r.call(guest, 0xc63f0);
                                                        r.eax = m.read32(0x7b0c48);
                                                        r.edx = m.read32(
                                                            r.esi
                                                                .wrapping_add(r.eax)
                                                                .wrapping_add(0x14),
                                                        );
                                                        r.eax = m.read32(
                                                            r.esi
                                                                .wrapping_add(r.eax)
                                                                .wrapping_add(8),
                                                        );
                                                        r.push(m, r.edx);
                                                        r.push(m, 0);
                                                        r.push(m, r.eax);
                                                        r.call(guest, 0xcbcf0);
                                                        r.eax = m.read32(0x7b0c48);
                                                        r.ecx = m.read32(
                                                            r.esi
                                                                .wrapping_add(r.eax)
                                                                .wrapping_add(8),
                                                        );
                                                        r.edx = m.read32(
                                                            r.esi
                                                                .wrapping_add(r.eax)
                                                                .wrapping_add(0x14),
                                                        );
                                                        m.write32(r.ecx.wrapping_add(0x2a8), r.edx);
                                                        r.eax = m.read32(0x7b0c48);
                                                        r.ecx = m.read32(
                                                            r.esi
                                                                .wrapping_add(r.eax)
                                                                .wrapping_add(8),
                                                        );
                                                        r.ecx = r.ecx.wrapping_add(0x224);
                                                        r.push(m, r.ecx);
                                                        r.push(m, r.edi);
                                                        r.call(guest, 0x25a570);
                                                        r.edx = m.read32(0x7b0c48);
                                                        r.eax = m.read32(
                                                            r.esi
                                                                .wrapping_add(r.edx)
                                                                .wrapping_add(8),
                                                        );
                                                        m.write32(
                                                            r.eax.wrapping_add(0x21c),
                                                            m.read32(r.eax.wrapping_add(0x21c)) | 1,
                                                        );
                                                        r.eax = m.read32(0x7b0c48);
                                                        r.ecx = m.read32(
                                                            r.eax
                                                                .wrapping_add(r.esi)
                                                                .wrapping_add(0x1504),
                                                        );
                                                        r.eax = r.eax.wrapping_add(r.esi);
                                                        r.esp = r.esp.wrapping_add(0x24);
                                                        if r.ecx == 0x7fffffff {
                                                            r.edi = 0;
                                                            r.push(m, r.eax);
                                                            m.write32(0x76b1a4, 0xf);
                                                            m.write32(0x76b1a8, r.edi);
                                                            r.call(guest, 0x35db10);
                                                            r.esp = r.esp.wrapping_add(4);
                                                            m.write32(
                                                                r.esp.wrapping_add(0x10),
                                                                r.edi,
                                                            );
                                                        }
                                                        m.write32(r.ebp, m.read32(r.ebp) | 2);
                                                    }
                                                }
                                                r.eax = gs::get_players(m);
                                                r.ebx = r.ebx.wrapping_add(1);
                                                r.esi = r.esi.wrapping_add(0x1584);
                                                r.ebp = r.ebp.wrapping_add(0x124);
                                                if (r.ebx as i32) >= (r.eax as i32) {
                                                    r.ecx = m.read32(r.esp.wrapping_add(0x10));
                                                    if r.ecx == 0 {
                                                        break 'at_68f83;
                                                    }
                                                    break 'at_68f32;
                                                }
                                            }
                                        }
                                    }
                                    'at_68f64: {
                                        r.esi = 0;
                                        if r.eax as i32 > 0i32 {
                                            r.edi = 0;
                                            loop {
                                                r.ecx = m.read32(0x7b0c48);
                                                r.edx = r.edi.wrapping_add(r.ecx);
                                                r.push(m, 1);
                                                r.push(m, r.edx);
                                                r.call(guest, 0xd50e0);
                                                r.eax = gs::get_players(m);
                                                r.esp = r.esp.wrapping_add(8);
                                                r.esi = r.esi.wrapping_add(1);
                                                r.edi = r.edi.wrapping_add(0x1584);
                                                if (r.esi as i32) >= (r.eax as i32) {
                                                    break 'at_68f64;
                                                }
                                            }
                                        }
                                    }
                                    r.eax = gs::get_match_state(m);
                                    r.edi = 5;
                                    if r.eax != r.edi {
                                        r.xmm[0] = Xmm::default();
                                        gs::set_match_state(m, r.edi);
                                        m.write32(0x7de300, r.xmm[0].f(0).to_bits());
                                    }
                                } else {
                                    if jt != 0x68fb2 {
                                        if jt == 0x691c7 {
                                            r.eax = gs::get_flags(m);
                                            if (r.eax & 0x20000000) == 0 {
                                                if (r.eax & 0x40000000) == 0 {
                                                    r.call(guest, 0x153af0);
                                                    if r.eax == 0 {
                                                        r.eax = m.read32(0x7de318);
                                                        r.eax =
                                                            r.eax.wrapping_add(m.read32(0x4e7944));
                                                        m.write32(0x7de318, r.eax);
                                                        m.write32(0x7de31c, r.eax);
                                                        r.call(guest, 0x667f0);
                                                        r.call(guest, 0x681c0);
                                                        r.eax = m.read32(0x6f1194);
                                                        if r.eax != 0 {
                                                            r.call(guest, 0x164b60);
                                                            if r.eax == 0 {
                                                                r.call(guest, 0x3533f0);
                                                                'at_692d9: {
                                                                    if r.eax == 0 {
                                                                        if ((gs::get_mode(m)
                                                                            as u32)
                                                                            & 0xff)
                                                                            == (3 & 0xff)
                                                                        {
                                                                            r.eax =
                                                                                gs::get_players(m);
                                                                            r.edi = 0;
                                                                            if r.eax as i32 > 0i32 {
                                                                                r.esi = 0;
                                                                                loop {
                                                                                    r.ecx = m
                                                                                        .read32(
                                                                                        0x7b0c48,
                                                                                    );
                                                                                    r.eax = r.esi.wrapping_add(r.ecx);
                                                                                    r.ecx = m.read32(r.eax.wrapping_add(4));
                                                                                    if r.ecx as i32
                                                                                        >= 0
                                                                                    {
                                                                                        r.push(
                                                                                            m,
                                                                                            r.eax,
                                                                                        );
                                                                                        r.call(guest, 0x2321c0);
                                                                                        r.esp = r.esp.wrapping_add(4);
                                                                                    }
                                                                                    r.eax = gs::get_players(m);
                                                                                    r.edi = r.edi.wrapping_add(1);
                                                                                    r.esi = r.esi.wrapping_add(0x1584);
                                                                                    if (r.edi
                                                                                        as i32)
                                                                                        >= (r.eax
                                                                                            as i32)
                                                                                    {
                                                                                        break 'at_692d9;
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                m.write32(0x6f1194, 0);
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    r.call(guest, 0x65fe0);
                                                    r.push(m, 0);
                                                    r.call(guest, 0x42870);
                                                    r.push(m, 1);
                                                    r.call(guest, 0x42870);
                                                    r.eax = m.read32(0x7de318);
                                                    r.eax = r.eax.wrapping_add(m.read32(0x4e7944));
                                                    r.esp = r.esp.wrapping_add(8);
                                                    m.write32(0x7de318, r.eax);
                                                    m.write32(0x7de31c, r.eax);
                                                    r.call(guest, 0x681c0);
                                                }
                                            } else {
                                                r.call(guest, 0x65fe0);
                                                if r.eax != 0 {
                                                    r.push(m, 0);
                                                    r.call(guest, 0x98db0);
                                                    r.esp = r.esp.wrapping_add(4);
                                                }
                                                r.eax = m.read32(0x7de318);
                                                r.eax = r.eax.wrapping_add(m.read32(0x4e7944));
                                                r.push(m, 0);
                                                m.write32(0x7de318, r.eax);
                                                m.write32(0x7de31c, r.eax);
                                                r.call(guest, 0x42870);
                                                r.push(m, 1);
                                                r.call(guest, 0x42870);
                                                r.esp = r.esp.wrapping_add(8);
                                            }
                                            r.call(guest, 0x355ec0);
                                            if r.eax == 0 {
                                                break 'at_6957f;
                                            }
                                            r.call(guest, 0x3533f0);
                                            if r.eax == 0 {
                                                break 'at_6957f;
                                            }
                                            r.call(guest, 0x35e960);
                                            if r.eax == 0 {
                                                break 'at_6957f;
                                            }
                                            r.call(guest, 0x344e0);
                                            r.edi = m.read32(r.esp);
                                            r.esp = r.esp.wrapping_add(4);
                                            r.esi = m.read32(r.esp);
                                            r.esp = r.esp.wrapping_add(4);
                                            r.ebp = m.read32(r.esp);
                                            r.esp = r.esp.wrapping_add(4);
                                            r.ebx = m.read32(r.esp);
                                            r.esp = r.esp.wrapping_add(4);
                                            r.esp = r.ebp;
                                            r.ebp = m.read32(r.esp);
                                            r.esp = r.esp.wrapping_add(4);
                                            return;
                                        } else {
                                            if jt == 0x69317 {
                                                r.esi = m.read32(0x7de318);
                                                r.edx = m.read32(0x4e7944);
                                                r.esi = r.esi.wrapping_add(r.edx);
                                                m.write32(0x7de318, r.esi);
                                                r.call(guest, 0x3533f0);
                                                if r.eax != 0 {
                                                    r.call(guest, 0x355f10);
                                                }
                                                r.call(guest, 0x65270);
                                                'at_69420: {
                                                    r.fpu
                                                        .push(f32::from_bits(m.read32(0x7de300))
                                                            as f64);
                                                    fpcmp = fcmp(r.fpu.st(0), r.fpu.st(1));
                                                    r.fpu.pop();
                                                    r.fpu.pop();
                                                    if fpcmp == 1 {
                                                        r.call(guest, 0x3533f0);
                                                        'at_6937e: {
                                                            if r.eax != 0 {
                                                                r.call(guest, 0x3533f0);
                                                                if r.eax != 0 {
                                                                    r.call(guest, 0x357690);
                                                                    if r.eax != 0 {
                                                                        break 'at_6937e;
                                                                    }
                                                                }
                                                                r.xmm[0] = Xmm::scalar(
                                                                    f32::from_bits(
                                                                        m.read32(0x7de300),
                                                                    )
                                                                    .to_bits(),
                                                                );
                                                                fca = f32::from_bits(
                                                                    r.xmm[0].lanes[0],
                                                                )
                                                                    as f64;
                                                                fcb = f32::from_bits(
                                                                    m.read32(0x475d18),
                                                                )
                                                                    as f64;
                                                                if (fca <= fcb)
                                                                    || ((fca != fca)
                                                                        || (fcb != fcb))
                                                                {
                                                                    break 'at_69420;
                                                                }
                                                            }
                                                        }
                                                        r.call(guest, 0x65250);
                                                        'at_693bf: {
                                                            if r.eax == 0 {
                                                                if ((gs::get_mode(m) as u32) & 0xff)
                                                                    != (0xa & 0xff)
                                                                {
                                                                    break 'at_693bf;
                                                                }
                                                                if gs::get_players(m) as i32 <= 1 {
                                                                    break 'at_693bf;
                                                                }
                                                            }
                                                            r.push(m, 0);
                                                            r.call(guest, 0x356800);
                                                            r.eax = ((r.eax as i32)
                                                                .wrapping_mul(0x1584i32))
                                                                as u32;
                                                            r.ecx = m.read32(0x7b0c48);
                                                            r.edx = m.read32(
                                                                r.eax
                                                                    .wrapping_add(r.ecx)
                                                                    .wrapping_add(8),
                                                            );
                                                            r.edx = r.edx.wrapping_add(0x224);
                                                            r.push(m, r.edx);
                                                            r.call(guest, 0x55d80);
                                                            r.esp = r.esp.wrapping_add(8);
                                                        }
                                                        r.eax = m.read32(0x4e8cf4);
                                                        r.ecx = m.read32(0x4e8cf0);
                                                        r.eax = r.eax.wrapping_sub(1);
                                                        r.push(m, r.eax);
                                                        r.ecx = r.ecx.wrapping_sub(1);
                                                        r.push(m, r.ecx);
                                                        r.push(m, 0);
                                                        r.push(m, 0);
                                                        r.call(guest, 0x56050);
                                                        r.esp = r.esp.wrapping_add(0x10);
                                                        r.call(guest, 0x64d10);
                                                        if r.eax != 0 {
                                                            r.eax = m.read32(0x7de45c);
                                                            if ((((((((((r.eax >> 8) as u8) as u32)
                                                                & 0xff)
                                                                as u8)
                                                                as i32)
                                                                .wrapping_sub(
                                                                    ((0 & 0xff) as u8) as i32,
                                                                ))
                                                                as u8)
                                                                as i32)
                                                                .wrapping_shr(7u32 & 31))
                                                                != 0
                                                            {
                                                                r.call(guest, 0x2eb030);
                                                                break 'at_69420;
                                                            }
                                                        }
                                                        r.esi = 0;
                                                        r.call(guest, 0x1cdae0);
                                                        if r.eax as i32 > 0i32 {
                                                            r.edi = r.edi;
                                                            loop {
                                                                r.push(m, r.esi);
                                                                r.call(guest, 0x2e4c10);
                                                                r.esp = r.esp.wrapping_add(4);
                                                                if r.eax != 0 {
                                                                    r.push(m, r.esi);
                                                                    r.call(guest, 0x2f43d0);
                                                                    r.esp = r.esp.wrapping_add(4);
                                                                }
                                                                r.esi = r.esi.wrapping_add(1);
                                                                r.call(guest, 0x1cdae0);
                                                                if (r.esi as i32) >= (r.eax as i32)
                                                                {
                                                                    break 'at_69420;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                if gs::get_level(m) == 0x11 {
                                                    r.call(guest, 0x212490);
                                                }
                                                r.eax = m.read32(0x4d2880);
                                                if r.eax as i32 >= 0 {
                                                    r.eax = r.eax.wrapping_sub(1);
                                                    m.write32(0x4d2880, r.eax);
                                                    if r.eax == 0 {
                                                        m.write32(0x4d2880, 0xffffffffu32);
                                                        r.call(guest, 0x355ec0);
                                                        if r.eax != 0 {
                                                            r.push(m, 1);
                                                            r.call(guest, 0x359e70);
                                                            r.esp = r.esp.wrapping_add(4);
                                                            r.call(guest, 0x2eb1f0);
                                                        }
                                                    }
                                                }
                                                r.call(guest, 0x355ec0);
                                                if r.eax == 0 {
                                                    break 'at_6957f;
                                                }
                                                r.call(guest, 0x3533f0);
                                                if r.eax == 0 {
                                                    break 'at_6957f;
                                                }
                                                r.call(guest, 0x35e960);
                                                if r.eax == 0 {
                                                    break 'at_6957f;
                                                }
                                                r.call(guest, 0x344e0);
                                                r.edi = m.read32(r.esp);
                                                r.esp = r.esp.wrapping_add(4);
                                                r.esi = m.read32(r.esp);
                                                r.esp = r.esp.wrapping_add(4);
                                                r.ebp = m.read32(r.esp);
                                                r.esp = r.esp.wrapping_add(4);
                                                r.ebx = m.read32(r.esp);
                                                r.esp = r.esp.wrapping_add(4);
                                                r.esp = r.ebp;
                                                r.ebp = m.read32(r.esp);
                                                r.esp = r.esp.wrapping_add(4);
                                                return;
                                            } else {
                                                if jt == 0x69495 {
                                                    r.call(guest, 0x2e44a0);
                                                    r.fpu
                                                        .push(f32::from_bits(m.read32(0x7de300))
                                                            as f64);
                                                    fpcmp = fcmp(r.fpu.st(0), r.fpu.st(1));
                                                    r.fpu.pop();
                                                    r.fpu.pop();
                                                    if fpcmp != 1 {
                                                        break 'at_6957f;
                                                    }
                                                    r.push(m, 0xb);
                                                    r.call(guest, 0x65230);
                                                    r.push(m, 0xf);
                                                    r.push(m, 0x188);
                                                    r.push(m, 0x195);
                                                    r.push(m, 0x3c);
                                                    r.call(guest, 0x1553e0);
                                                    r.esp = r.esp.wrapping_add(0x14);
                                                    r.edi = m.read32(r.esp);
                                                    r.esp = r.esp.wrapping_add(4);
                                                    r.esi = m.read32(r.esp);
                                                    r.esp = r.esp.wrapping_add(4);
                                                    r.ebp = m.read32(r.esp);
                                                    r.esp = r.esp.wrapping_add(4);
                                                    r.ebx = m.read32(r.esp);
                                                    r.esp = r.esp.wrapping_add(4);
                                                    r.esp = r.ebp;
                                                    r.ebp = m.read32(r.esp);
                                                    r.esp = r.esp.wrapping_add(4);
                                                    return;
                                                } else {
                                                    if jt == 0x694cf {
                                                        fca = f32::from_bits(r.xmm[0].lanes[0])
                                                            as f64;
                                                        fcb = f32::from_bits(m.read32(0x475cd4))
                                                            as f64;
                                                        if (fca <= fcb)
                                                            || ((fca != fca) || (fcb != fcb))
                                                        {
                                                            break 'at_6957f;
                                                        }
                                                        r.push(m, 0xb);
                                                        r.call(guest, 0x65230);
                                                        r.esp = r.esp.wrapping_add(4);
                                                        r.edi = m.read32(r.esp);
                                                        r.esp = r.esp.wrapping_add(4);
                                                        r.esi = m.read32(r.esp);
                                                        r.esp = r.esp.wrapping_add(4);
                                                        r.ebp = m.read32(r.esp);
                                                        r.esp = r.esp.wrapping_add(4);
                                                        r.ebx = m.read32(r.esp);
                                                        r.esp = r.esp.wrapping_add(4);
                                                        r.esp = r.ebp;
                                                        r.ebp = m.read32(r.esp);
                                                        r.esp = r.esp.wrapping_add(4);
                                                        return;
                                                    } else {
                                                        if jt == 0x694ee {
                                                            r.eax = m.read32(0x6b7aa8);
                                                            if r.eax == 0 {
                                                                r.edx = r.esp.wrapping_add(0x20);
                                                                r.eax = 0;
                                                                r.ecx = 0x28;
                                                                r.edi = r.esp.wrapping_add(0x20);
                                                                r.push(m, r.edx);
                                                                {
                                                                    let step: u32 = 4;
                                                                    for i in 0..r.ecx {
                                                                        m.write32(
                                                                            r.edi.wrapping_add(
                                                                                i.wrapping_mul(
                                                                                    step,
                                                                                ),
                                                                            ),
                                                                            r.eax,
                                                                        );
                                                                    }
                                                                    r.edi = r.edi.wrapping_add(
                                                                        r.ecx.wrapping_mul(step),
                                                                    );
                                                                }
                                                                r.ecx = 0;
                                                                m.write32(0x78ad50, 2);
                                                                r.call(guest, 0x65940);
                                                                r.eax = r.esp.wrapping_add(0x24);
                                                                r.push(m, r.eax);
                                                                r.call(guest, 0x658d0);
                                                                r.esp = r.esp.wrapping_add(8);
                                                            }
                                                            r.call(guest, 0x459e0);
                                                            r.push(m, 0xc);
                                                            m.write32(0x6f1198, 5);
                                                            r.call(guest, 0x65230);
                                                            r.esp = r.esp.wrapping_add(4);
                                                            r.edi = m.read32(r.esp);
                                                            r.esp = r.esp.wrapping_add(4);
                                                            r.esi = m.read32(r.esp);
                                                            r.esp = r.esp.wrapping_add(4);
                                                            r.ebp = m.read32(r.esp);
                                                            r.esp = r.esp.wrapping_add(4);
                                                            r.ebx = m.read32(r.esp);
                                                            r.esp = r.esp.wrapping_add(4);
                                                            r.esp = r.ebp;
                                                            r.ebp = m.read32(r.esp);
                                                            r.esp = r.esp.wrapping_add(4);
                                                            return;
                                                        } else {
                                                            if jt == 0x69546 {
                                                                'at_69564: {
                                                                    r.eax = gs::get_players(m);
                                                                    r.esi = 0;
                                                                    if r.eax as i32 > 0i32 {
                                                                        loop {
                                                                            r.push(m, r.esi);
                                                                            r.call(guest, 0x42870);
                                                                            r.eax =
                                                                                gs::get_players(m);
                                                                            r.esp = r
                                                                                .esp
                                                                                .wrapping_add(4);
                                                                            r.esi = r
                                                                                .esi
                                                                                .wrapping_add(1);
                                                                            if (r.esi as i32)
                                                                                >= (r.eax as i32)
                                                                            {
                                                                                break 'at_69564;
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                r.eax = m.read32(0x6f1198);
                                                                if r.eax as i32 <= 0 {
                                                                    break 'at_6957f;
                                                                }
                                                                r.eax = r.eax.wrapping_sub(1);
                                                                m.write32(0x6f1198, r.eax);
                                                                if r.eax != 0 {
                                                                    break 'at_6957f;
                                                                }
                                                                m.write32(0x4c0a2c, 0x29a);
                                                                break 'at_6957f;
                                                            } else {
                                                                if jt == 0x6957f {
                                                                    break 'at_6957f;
                                                                }
                                                                r.tail_call(guest, jt);
                                                                return;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    r.ebx = 0;
                                    r.esi = 0;
                                    r.call(guest, 0x1cdae0);
                                    'at_68fe2: {
                                        if r.eax as i32 > 0i32 {
                                            loop {
                                                r.push(m, r.esi);
                                                r.call(guest, 0x2e68c0);
                                                r.esp = r.esp.wrapping_add(4);
                                                if r.eax != 0 {
                                                    r.call(guest, 0x3533f0);
                                                    if r.eax == 0 {
                                                        break 'at_68f83;
                                                    }
                                                    break 'at_68fe2;
                                                } else {
                                                    r.esi = r.esi.wrapping_add(1);
                                                    r.call(guest, 0x1cdae0);
                                                    if (r.esi as i32) >= (r.eax as i32) {
                                                        break 'at_68fe2;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if m.read32(0x4d287c) == 0xffffffffu32 {
                                        r.eax = m.read32(0x4e8cec);
                                        m.write32(0x4d287c, r.eax);
                                    }
                                    r.call(guest, 0x358e40);
                                    if r.eax != 0 {
                                        if (m.read32(0x4d2878) as i32) >= (r.ebx as i32) {
                                            m.write32(0x4d2878, r.ebx);
                                        }
                                    }
                                    r.push(m, 0xfffffdffu32);
                                    r.push(m, r.ebx);
                                    r.call(guest, 0x42830);
                                    r.push(m, 0xfffffdffu32);
                                    r.push(m, 1);
                                    r.call(guest, 0x42830);
                                    r.push(m, 0xfffffdffu32);
                                    r.push(m, 2);
                                    r.call(guest, 0x42830);
                                    r.push(m, 0xfffffdffu32);
                                    r.push(m, 3);
                                    r.call(guest, 0x42830);
                                    r.xmm[0].set_f(0, (m.read32(0x4d287c) as i32) as f32);
                                    r.xmm[0].set_f(
                                        0,
                                        r.xmm[0].f(0) + f32::from_bits(m.read32(0x4e7948)),
                                    );
                                    r.ecx = m.read32(0x4e8cec);
                                    r.eax = cvttss2si(r.xmm[0].f(0));
                                    r.esp = r.esp.wrapping_add(0x20);
                                    m.write32(0x4d287c, r.eax);
                                    if (r.eax as i32) > (r.ecx as i32) {
                                        r.eax = m.read32(0x4d2878);
                                        if r.eax != r.ebx {
                                            r.push(m, r.eax);
                                            r.push(m, 0x4760fc);
                                            r.edx = r.esp.wrapping_add(0x1c);
                                            r.push(m, r.edx);
                                        } else {
                                            r.push(m, 0x3b9);
                                            r.call(guest, 0x3d7b0);
                                            r.push(m, r.eax);
                                            r.ecx = r.esp.wrapping_add(0x1c);
                                            r.push(m, r.ecx);
                                        }
                                        r.call(guest, 0x3c85d4);
                                        r.esp = r.esp.wrapping_add(0xc);
                                        r.esi = 0;
                                        r.call(guest, 0x1cdae0);
                                        'at_690e5: {
                                            if r.eax as i32 > 0i32 {
                                                r.edi = 5;
                                                r.ebp = 0x13;
                                                loop {
                                                    'at_690d2: {
                                                        if (((gs::get_flags_low8(m) as u32) & 0xff)
                                                            & (0x40 & 0xff))
                                                            != 0
                                                        {
                                                            if m.read32(0x7de470) == r.ebp {
                                                                r.push(m, r.ebx);
                                                                r.eax = r.esp.wrapping_add(0x18);
                                                                r.push(m, r.eax);
                                                                r.push(m, 0xd);
                                                                break 'at_690d2;
                                                            }
                                                        }
                                                        r.push(m, r.ebx);
                                                        r.ecx = r.esp.wrapping_add(0x18);
                                                        r.push(m, r.ecx);
                                                        r.push(m, r.edi);
                                                    }
                                                    r.push(m, r.esi);
                                                    r.call(guest, 0x12d350);
                                                    r.esp = r.esp.wrapping_add(0x10);
                                                    r.esi = r.esi.wrapping_add(1);
                                                    r.call(guest, 0x1cdae0);
                                                    if (r.esi as i32) >= (r.eax as i32) {
                                                        break 'at_690e5;
                                                    }
                                                }
                                            }
                                        }
                                        r.edi = m.read32(0x4d2878);
                                        r.edx = 7;
                                        r.edx = r.edx.wrapping_sub(r.edi);
                                        r.push(m, r.edx);
                                        r.call(guest, 0x164a30);
                                        r.eax = m.read32(0x4d2878);
                                        r.esp = r.esp.wrapping_add(4);
                                        r.eax = r.eax.wrapping_sub(1);
                                        m.write32(0x4d2878, r.eax);
                                        m.write32(0x4d287c, r.ebx);
                                    }
                                    if m.read32(0x4d2878) == 0xffffffffu32 {
                                        r.eax = m.read32(0x4e8cec);
                                        m.write32(0x4d287c, r.eax);
                                        fa = (gs::get_mode(m) as u32) & 0xff;
                                        m.write32(0x6f11bc, r.ebx);
                                        m.write32(0x4d2878, 3);
                                        if fa == (0xa & 0xff) {
                                            if gs::get_level(m) != 0x64 {
                                                r.call(guest, 0x9da10);
                                                r.push(m, r.ebx);
                                                r.call(guest, 0x61ee0);
                                                r.esp = r.esp.wrapping_add(4);
                                                if r.eax != 0xffffffffu32 {
                                                    r.push(m, 1);
                                                    r.push(m, r.eax);
                                                    r.call(guest, 0x61e90);
                                                    r.esp = r.esp.wrapping_add(8);
                                                }
                                            }
                                        }
                                        r.ecx = gs::get_match_state(m);
                                        r.eax = 7;
                                        if r.ecx != r.eax {
                                            r.xmm[0] = Xmm::default();
                                            gs::set_match_state(m, r.eax);
                                            m.write32(0x7de300, r.xmm[0].f(0).to_bits());
                                        }
                                        r.call(guest, 0x2f2d30);
                                        'at_691b8: {
                                            r.ecx = gs::get_players(m);
                                            if (r.ecx as i32) < (m.read32(0x74c37c) as i32) {
                                                r.edx = 0xfffffffbu32;
                                                loop {
                                                    r.eax = m.read32(
                                                        r.ecx
                                                            .wrapping_mul(4)
                                                            .wrapping_add(0x7a3580),
                                                    );
                                                    if r.eax != r.ebx {
                                                        m.write32(
                                                            r.eax.wrapping_add(0x2c),
                                                            m.read32(r.eax.wrapping_add(0x2c))
                                                                & r.edx,
                                                        );
                                                    }
                                                    r.eax = m.read32(0x74c37c);
                                                    r.ecx = r.ecx.wrapping_add(1);
                                                    if (r.ecx as i32) >= (r.eax as i32) {
                                                        break 'at_691b8;
                                                    }
                                                }
                                            }
                                        }
                                        m.write32(0x6f1194, 1);
                                    }
                                }
                            }
                            r.eax = m.read32(0x7de318);
                            r.eax = r.eax.wrapping_add(m.read32(0x4e7944));
                            m.write32(0x7de318, r.eax);
                            m.write32(0x7de31c, r.eax);
                            r.call(guest, 0x681c0);
                            r.edi = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            r.esi = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            r.ebp = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            r.ebx = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            r.esp = r.ebp;
                            r.ebp = m.read32(r.esp);
                            r.esp = r.esp.wrapping_add(4);
                            return;
                        }
                    }
                    r.call(guest, 0x687a0);
                }
            }
            r.eax = m.read32(0x7de318);
            r.eax = r.eax.wrapping_add(m.read32(0x4e7944));
            m.write32(0x7de318, r.eax);
            m.write32(0x7de31c, r.eax);
            r.edi = m.read32(r.esp);
            r.esp = r.esp.wrapping_add(4);
            r.esi = m.read32(r.esp);
            r.esp = r.esp.wrapping_add(4);
            r.ebp = m.read32(r.esp);
            r.esp = r.esp.wrapping_add(4);
            r.ebx = m.read32(r.esp);
            r.esp = r.esp.wrapping_add(4);
            r.esp = r.ebp;
            r.ebp = m.read32(r.esp);
            r.esp = r.esp.wrapping_add(4);
            return;
        }
    }
    r.edi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.esi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ebp = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ebx = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.esp = r.ebp;
    r.ebp = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
}

/// 0x231A20. INFERRED: Checks player-state completion conditions and posts the appropriate round-ending messages.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_state_descriptor_check_completion_and_end_round<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.eax = m.read32(0x7de470);
    r.esp = r.esp.wrapping_sub(0x34);
    r.push(m, r.ebx);
    r.push(m, r.ebp);
    r.push(m, r.esi);
    r.push(m, r.edi);
    if (r.eax as i32) >= 0x15 {
        r.eax = m.read32(r.eax.wrapping_mul(4).wrapping_add(0x513d04));
        m.write32(r.esp.wrapping_add(0x14), r.eax);
        r.esi = r.eax;
    } else {
        r.esi = m.read32(r.eax.wrapping_mul(4).wrapping_add(0x510788));
        m.write32(r.esp.wrapping_add(0x14), r.esi);
    }
    r.ebp = 0;
    r.edi = 1;
    r.ebx = 0;
    m.write32(r.esp.wrapping_add(0x24), r.edi);
    m.write32(r.esp.wrapping_add(0x28), r.edi);
    m.write32(r.esp.wrapping_add(0x2c), r.edi);
    m.write32(r.esp.wrapping_add(0x18), r.ebx);
    m.write32(r.esp.wrapping_add(0x34), r.ebp);
    m.write32(r.esp.wrapping_add(0x1c), r.ebp);
    m.write32(r.esp.wrapping_add(0x3c), r.ebp);
    m.write32(r.esp.wrapping_add(0x38), r.ebp);
    r.call(guest, 0x22d170);
    'at_231a95: {
        r.eax = m.read32(r.esi.wrapping_add(0x20));
        if (r.eax as i32) > (r.ebp as i32) {
            r.ecx = m.read32(r.esi.wrapping_add(0x1c));
            r.edx = r.eax;
            loop {
                if (m.read8(r.ecx) & 0x20) != 0 {
                    r.ebx = r.edi;
                }
                r.ecx = r.ecx.wrapping_add(0x14);
                r.edx = r.edx.wrapping_sub(1);
                if r.edx == 0 {
                    m.write32(r.esp.wrapping_add(0x18), r.ebx);
                    break 'at_231a95;
                }
            }
        }
    }
    'at_231dbc: {
        m.write32(r.esp.wrapping_add(0x30), r.ebp);
        if (r.eax as i32) > (r.ebp as i32) {
            m.write32(r.esp.wrapping_add(0x20), r.ebp);
            'loop_231aa5: loop {
                'at_231ca8: {
                    r.esi = m.read32(r.esi.wrapping_add(0x1c));
                    r.ecx = m.read32(r.esp.wrapping_add(0x20));
                    r.edi = m.read32(r.esi.wrapping_add(r.ecx).wrapping_add(4));
                    r.esi = r.esi.wrapping_add(r.ecx);
                    m.write32(r.esp.wrapping_add(0x10), r.ebp);
                    if r.edi != 0xa {
                        if r.edi != 9 {
                            if m.read32(r.esi.wrapping_add(8)) == 0xfffffffdu32 {
                                r.ecx = m.read32(0x7356d8);
                                r.edx = m.read32(r.ecx.wrapping_add(0x84));
                                r.push(m, r.ebp);
                                r.push(m, r.edi);
                                m.write32(r.esp.wrapping_add(0x48), r.edx);
                                r.call(guest, 0x22d570);
                                r.ecx = gs::get_extra_players(m);
                                r.ebx = r.eax;
                                r.eax = gs::get_players(m);
                                r.ecx = r.ecx.wrapping_add(r.eax);
                                r.esp = r.esp.wrapping_add(8);
                                r.edi = 0;
                                if (r.ecx as i32) <= 0 {
                                    break 'at_231ca8;
                                }
                                r.ebp = 0;
                                loop {
                                    r.eax = m.read32(0x7356d8);
                                    r.edx = m.read32(r.esp.wrapping_add(0x40));
                                    if m.read32(r.eax.wrapping_add(r.ebp).wrapping_add(0x84))
                                        != r.edx
                                    {
                                        r.ecx = m.read32(r.esi.wrapping_add(4));
                                        r.push(m, r.edi);
                                        r.push(m, r.ecx);
                                        r.call(guest, 0x22d570);
                                        'at_231b4a: {
                                            r.ecx = m.read32(r.esi.wrapping_add(4));
                                            r.esp = r.esp.wrapping_add(8);
                                            if r.ecx != 0 {
                                                if r.ecx != 1 {
                                                    r.ecx = m.read32(r.esi.wrapping_add(0x10));
                                                    break 'at_231b4a;
                                                }
                                            }
                                            r.ecx = m.read32(r.esi.wrapping_add(0x10));
                                            r.ecx = ((r.ecx as i32)
                                                .wrapping_mul(m.read32(0x4e8cec) as i32))
                                                as u32;
                                        }
                                        r.edx = m.read32(r.esi.wrapping_add(0xc));
                                        r.push(m, r.edx);
                                        r.ecx = r.ecx.wrapping_add(r.eax);
                                        r.push(m, r.ecx);
                                        r.push(m, r.ebx);
                                        r.call(guest, 0x22d4e0);
                                        r.esp = r.esp.wrapping_add(0xc);
                                        if r.eax != 0 {
                                            m.write32(r.esp.wrapping_add(0x10), 1);
                                        }
                                    }
                                    r.eax = gs::get_extra_players(m);
                                    r.ecx = gs::get_players(m);
                                    r.edi = r.edi.wrapping_add(1);
                                    r.eax = r.eax.wrapping_add(r.ecx);
                                    r.ebp = r.ebp.wrapping_add(0xea0);
                                    if (r.edi as i32) >= (r.eax as i32) {
                                        break 'at_231ca8;
                                    }
                                }
                            }
                        }
                        if r.edi != 0xa {
                            if r.edi != 9 {
                                if m.read32(r.esi.wrapping_add(8)) == 0xffffffffu32 {
                                    r.edx = gs::get_extra_players(m);
                                    r.eax = gs::get_players(m);
                                    r.edx = r.edx.wrapping_add(r.eax);
                                    r.ebp = 0;
                                    if (r.edx as i32) <= 0 {
                                        break 'at_231ca8;
                                    }
                                    r.ebx = 1;
                                    loop {
                                        r.push(m, r.ebp);
                                        r.push(m, r.edi);
                                        r.call(guest, 0x22d570);
                                        'at_231bd7: {
                                            r.edi = m.read32(r.esi.wrapping_add(4));
                                            r.esp = r.esp.wrapping_add(8);
                                            if r.edi != 0 {
                                                if r.edi != r.ebx {
                                                    r.ecx = m.read32(r.esi.wrapping_add(0x10));
                                                    break 'at_231bd7;
                                                }
                                            }
                                            r.ecx = m.read32(r.esi.wrapping_add(0x10));
                                            r.ecx = ((r.ecx as i32)
                                                .wrapping_mul(m.read32(0x4e8cec) as i32))
                                                as u32;
                                        }
                                        r.edx = m.read32(r.esi.wrapping_add(0xc));
                                        r.push(m, r.edx);
                                        r.push(m, r.ecx);
                                        r.push(m, r.eax);
                                        r.call(guest, 0x22d4e0);
                                        r.esp = r.esp.wrapping_add(0xc);
                                        if r.eax != 0 {
                                            m.write32(r.esp.wrapping_add(0x10), r.ebx);
                                        }
                                        r.eax = gs::get_players(m);
                                        r.ecx = gs::get_extra_players(m);
                                        r.ebp = r.ebp.wrapping_add(1);
                                        r.ecx = r.ecx.wrapping_add(r.eax);
                                        if (r.ebp as i32) >= (r.ecx as i32) {
                                            break 'at_231ca8;
                                        }
                                    }
                                }
                            }
                            if r.edi != 0xa {
                                if r.edi != 9 {
                                    if m.read32(r.esi.wrapping_add(8)) == 0xfffffffeu32 {
                                        r.eax = gs::get_players(m);
                                        r.ebp = 0;
                                        if (r.eax as i32) <= 0 {
                                            break 'at_231ca8;
                                        }
                                        r.ebx = 1;
                                        loop {
                                            r.push(m, r.ebp);
                                            r.push(m, r.edi);
                                            r.call(guest, 0x22d570);
                                            'at_231c4c: {
                                                r.edi = m.read32(r.esi.wrapping_add(4));
                                                r.esp = r.esp.wrapping_add(8);
                                                if r.edi != 0 {
                                                    if r.edi != r.ebx {
                                                        r.ecx = m.read32(r.esi.wrapping_add(0x10));
                                                        break 'at_231c4c;
                                                    }
                                                }
                                                r.ecx = m.read32(r.esi.wrapping_add(0x10));
                                                r.ecx = ((r.ecx as i32)
                                                    .wrapping_mul(m.read32(0x4e8cec) as i32))
                                                    as u32;
                                            }
                                            r.edx = m.read32(r.esi.wrapping_add(0xc));
                                            r.push(m, r.edx);
                                            r.push(m, r.ecx);
                                            r.push(m, r.eax);
                                            r.call(guest, 0x22d4e0);
                                            r.esp = r.esp.wrapping_add(0xc);
                                            if r.eax != 0 {
                                                m.write32(r.esp.wrapping_add(0x10), r.ebx);
                                            }
                                            r.eax = gs::get_players(m);
                                            r.ebp = r.ebp.wrapping_add(1);
                                            if (r.ebp as i32) >= (r.eax as i32) {
                                                break 'at_231ca8;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    r.eax = m.read32(r.esi.wrapping_add(8));
                    r.push(m, r.eax);
                    r.push(m, r.edi);
                    r.call(guest, 0x22d570);
                    'at_231c96: {
                        r.ecx = m.read32(r.esi.wrapping_add(4));
                        r.esp = r.esp.wrapping_add(8);
                        if r.ecx != r.ebp {
                            if r.ecx != 1 {
                                r.ecx = m.read32(r.esi.wrapping_add(0x10));
                                break 'at_231c96;
                            }
                        }
                        r.ecx = m.read32(r.esi.wrapping_add(0x10));
                        r.ecx = ((r.ecx as i32).wrapping_mul(m.read32(0x4e8cec) as i32)) as u32;
                    }
                    r.edx = m.read32(r.esi.wrapping_add(0xc));
                    r.push(m, r.edx);
                    r.push(m, r.ecx);
                    r.push(m, r.eax);
                    r.call(guest, 0x22d4e0);
                    r.esp = r.esp.wrapping_add(0xc);
                    m.write32(r.esp.wrapping_add(0x10), r.eax);
                }
                r.ecx = set_lo16(r.ecx, m.read16(r.esi) as u32);
                r.edi = m.read32(r.esp.wrapping_add(0x10));
                r.ebp = 0;
                if (r.ecx as u8) & 2 != 0 {
                    if r.edi == r.ebp {
                        m.write32(r.esp.wrapping_add(0x24), r.ebp);
                        m.write32(r.esp.wrapping_add(0x28), r.ebp);
                        m.write32(r.esp.wrapping_add(0x2c), r.ebp);
                        m.write32(r.esp.wrapping_add(0x18), r.ebp);
                    }
                }
                if (r.ecx as u8) & 4 != 0 {
                    if r.edi == r.ebp {
                        m.write32(r.esp.wrapping_add(0x24), r.ebp);
                    }
                }
                if (r.ecx as u8) & 8 != 0 {
                    if r.edi == r.ebp {
                        m.write32(r.esp.wrapping_add(0x28), r.ebp);
                    }
                }
                if (r.ecx as u8) & 0x10 != 0 {
                    if r.edi == r.ebp {
                        m.write32(r.esp.wrapping_add(0x2c), r.ebp);
                    }
                }
                'at_231d06: {
                    if (r.ecx as u8) & 0x20 != 0 {
                        if r.edi == r.ebp {
                            r.ebx = 0;
                            m.write32(r.esp.wrapping_add(0x18), r.ebx);
                            break 'at_231d06;
                        }
                    }
                    r.ebx = m.read32(r.esp.wrapping_add(0x18));
                }
                'at_231d5f: {
                    if (r.ecx as u8 as i8 as i32 as u8 as i32).wrapping_shr(7u32 & 31) == 0 {
                        if ((r.ecx >> 8) as u8) & 1 == 0 {
                            break 'at_231d5f;
                        }
                    }
                    'at_231d30: {
                        r.ecx = m.read32(r.esi.wrapping_add(4));
                        r.eax = m.read32(r.esi.wrapping_add(8));
                        if r.ecx != 0xa {
                            if r.ecx != 9 {
                                if r.eax != 0xffffffffu32 {
                                    if r.eax != 0xfffffffeu32 {
                                        if r.eax != 0xfffffffdu32 {
                                            break 'at_231d30;
                                        }
                                    }
                                }
                                r.eax = 0;
                            }
                        }
                    }
                    r.push(m, r.eax);
                    r.push(m, r.ecx);
                    r.call(guest, 0x22d570);
                    r.ecx = set_lo16(r.ecx, m.read16(r.esi) as u32);
                    r.esp = r.esp.wrapping_add(8);
                    if (r.ecx as u8 as i32).wrapping_shr(7u32 & 31) != 0 {
                        let condition_kind = m.read16(r.esi.wrapping_add(2)) as u32;
                        m.write32(0x75c984, r.eax);
                        if condition_kind == 2 {
                            m.write32(r.esp.wrapping_add(0x38), 1);
                        }
                    }
                    if ((r.ecx >> 8) as u8) & 1 != 0 {
                        m.write32(0x75c974, r.eax);
                    }
                }
                'at_231ee2: {
                    if (r.ecx as u8) & 1 != 0 {
                        if r.edi != r.ebp {
                            let condition_kind = m.read32(r.esi.wrapping_add(4));
                            r.esi = m.read32(r.esp.wrapping_add(0x14));
                            if condition_kind != r.ebp {
                                break 'at_231ee2;
                            }
                            m.write32(r.esp.wrapping_add(0x3c), 1);
                            break 'at_231ee2;
                        } else {
                        }
                    }
                    if (r.ecx as u8) & 0x40 != 0 {
                        let previous_count = m.read32(r.esp.wrapping_add(0x1c));
                        m.write32(r.esp.wrapping_add(0x34), 1);
                        if previous_count <= r.edi {
                            m.write32(r.esp.wrapping_add(0x1c), r.edi);
                        }
                    }
                    r.eax = m.read32(r.esp.wrapping_add(0x30));
                    r.edx = m.read32(r.esp.wrapping_add(0x20));
                    r.esi = m.read32(r.esp.wrapping_add(0x14));
                    r.ecx = m.read32(r.esi.wrapping_add(0x20));
                    r.eax = r.eax.wrapping_add(1);
                    r.edx = r.edx.wrapping_add(0x14);
                    m.write32(r.esp.wrapping_add(0x30), r.eax);
                    m.write32(r.esp.wrapping_add(0x20), r.edx);
                    if (r.eax as i32) < (r.ecx as i32) {
                        continue 'loop_231aa5;
                    }
                    if m.read32(r.esp.wrapping_add(0x24)) == r.ebp {
                        if m.read32(r.esp.wrapping_add(0x1c)) != r.ebp {
                            break 'at_231ee2;
                        }
                    } else {
                        if m.read32(r.esp.wrapping_add(0x34)) == r.ebp {
                            break 'at_231dbc;
                        }
                        if m.read32(r.esp.wrapping_add(0x1c)) != r.ebp {
                            break 'at_231dbc;
                        }
                    }
                    r.edi = m.read32(r.esp);
                    r.esp = r.esp.wrapping_add(4);
                    r.esi = m.read32(r.esp);
                    r.esp = r.esp.wrapping_add(4);
                    r.ebp = m.read32(r.esp);
                    r.esp = r.esp.wrapping_add(4);
                    r.ebx = m.read32(r.esp);
                    r.esp = r.esp.wrapping_add(4);
                    r.esp = r.esp.wrapping_add(0x34);
                    r.tail_call(guest, 0x230850);
                    return;
                }
                r.call(guest, 0x231650);
                r.eax = set_lo8(r.eax, m.read8(r.esi.wrapping_add(0x18)) as u32);
                if m.read32(r.esp.wrapping_add(0x3c)) == r.ebp {
                    if (r.eax as u8) & 4 == 0 {
                        'at_231f54: {
                            r.eax = m.read32(0x7de470);
                            if r.eax != 0x13 {
                                r.esi = 4;
                                if r.eax != 0x11 {
                                    break 'at_231f54;
                                }
                            }
                            r.esi = 0xb;
                        }
                        r.push(m, r.ebp);
                        r.push(m, 0x1818);
                        r.call(guest, 0x3d7b0);
                        r.esp = r.esp.wrapping_add(4);
                        r.push(m, r.eax);
                        r.push(m, r.esi);
                        r.push(m, r.ebp);
                        r.call(guest, 0x12d350);
                        r.esp = r.esp.wrapping_add(0x10);
                    } else {
                        r.call(guest, 0x66e90);
                    }
                } else {
                    if (r.eax as u8) & 4 == 0 {
                        r.push(m, r.ebp);
                        r.push(m, 0x181a);
                        r.call(guest, 0x3d7b0);
                        r.esp = r.esp.wrapping_add(4);
                        r.push(m, r.eax);
                        r.push(m, 4);
                        r.push(m, r.ebp);
                        r.call(guest, 0x12d350);
                        r.eax = m.read32(0x7de45c);
                        r.esp = r.esp.wrapping_add(0x10);
                        r.eax = r.eax | 0x20000;
                        m.write32(0x7de45c, r.eax);
                    } else {
                        r.call(guest, 0x66e90);
                        m.write32(0x7de45c, m.read32(0x7de45c) | 0x20000);
                    }
                }
                r.call(guest, 0x69e60);
                if r.eax != 0 {
                    r.call(guest, 0x69e60);
                    m.write32(r.eax.wrapping_add(0xc), r.ebp);
                    r.call(guest, 0x69e60);
                    m.write32(r.eax.wrapping_add(0x130), r.ebp);
                }
                r.eax = m.read32(0x7de45c);
                r.edi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esi = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.eax = r.eax | 0x10000;
                r.ebp = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                m.write32(0x7de45c, r.eax);
                r.ebx = m.read32(r.esp);
                r.esp = r.esp.wrapping_add(4);
                r.esp = r.esp.wrapping_add(0x34);
                r.tail_call(guest, 0x66fc0);
                return;
            }
        }
    }
    if (m.read8(r.esi.wrapping_add(0x18)) & 4) == 0 {
        'at_231dfc: {
            r.eax = m.read32(0x7de470);
            if r.eax != 0x13 {
                r.esi = 4;
                if r.eax != 0x11 {
                    break 'at_231dfc;
                }
            }
            r.esi = 0xb;
        }
        r.push(m, r.ebp);
        r.push(m, 0x6e0);
        r.call(guest, 0x3d7b0);
        r.esp = r.esp.wrapping_add(4);
        r.push(m, r.eax);
        r.push(m, r.esi);
        r.push(m, r.ebp);
        r.call(guest, 0x12d350);
        r.esp = r.esp.wrapping_add(0x10);
    } else {
        r.call(guest, 0x66e90);
    }
    r.ecx = m.read32(0x7de45c);
    r.eax = m.read32(r.esp.wrapping_add(0x28));
    r.ecx = r.ecx | 0x108000;
    m.write32(0x7de45c, r.ecx);
    if r.eax != r.ebp {
        r.eax = r.ecx;
        r.eax = r.eax | 0x200000;
        m.write32(0x7de45c, r.eax);
    }
    if m.read32(r.esp.wrapping_add(0x2c)) != r.ebp {
        m.write32(0x7de45c, m.read32(0x7de45c) | 0x400000);
    }
    if r.ebx != r.ebp {
        m.write32(0x7de45c, m.read32(0x7de45c) | 0x800000);
    }
    r.call(guest, 0x231560);
    r.call(guest, 0x66fc0);
    r.call(guest, 0x425e0);
    if m.read32(r.esp.wrapping_add(0x38)) == r.ebp {
        r.call(guest, 0x69e60);
        r.ecx = m.read32(0x75c984);
        r.edi = m.read32(r.esp);
        r.esp = r.esp.wrapping_add(4);
        r.esi = m.read32(r.esp);
        r.esp = r.esp.wrapping_add(4);
        r.ebp = m.read32(r.esp);
        r.esp = r.esp.wrapping_add(4);
        m.write32(r.eax.wrapping_add(0x130), r.ecx);
        r.ebx = m.read32(r.esp);
        r.esp = r.esp.wrapping_add(4);
        r.esp = r.esp.wrapping_add(0x34);
        return;
    }
    r.xmm[0].set_f(0, (m.read32(0x75c984) as i32) as f32);
    if m.read32(0x4e8cec) == 0x3c {
        r.xmm[0].set_f(0, r.xmm[0].f(0) * f32::from_bits(m.read32(0x475d54)));
    } else {
        r.xmm[0].set_f(0, r.xmm[0].f(0) * f32::from_bits(m.read32(0x475d58)));
    }
    r.ecx = cvttss2si(r.xmm[0].f(0));
    r.eax = 0x51eb851f;
    {
        let wide = ((r.eax as i32 as i64).wrapping_mul(r.ecx as i32 as i64)) as u64;
        r.eax = wide as u32;
        r.edx = (wide >> 32) as u32;
    }
    r.edx = ((r.edx as i32).wrapping_shr(8)) as u32;
    r.esi = r.edx;
    r.esi = r.esi.wrapping_shr(31);
    m.write32(0x75c984, r.ecx);
    r.esi = r.esi.wrapping_add(r.edx);
    r.call(guest, 0x69e60);
    r.edi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    m.write32(r.eax.wrapping_add(0x130), r.esi);
    r.esi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ebp = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ebx = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.esp = r.esp.wrapping_add(0x34);
}

/// 0x23F390. INFERRED: Preloads resources described by a mode-table entry.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_mode_resources_preload_from_mode_table_entry<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    let mut jt = 0;
    r.push(m, r.ebp);
    r.ebp = r.esp;
    r.esp &= 0xfffffff8u32;
    r.eax = 0x13a0;
    r.call(guest, 0x3c9d90);
    'at_23f888: {
        r.eax = m.read32(0x784014);
        r.push(m, r.ebx);
        r.push(m, r.ebp);
        r.push(m, r.esi);
        r.ebp = r.eax.wrapping_add(8);
        r.push(m, r.edi);
        r.edi = 0;
        r.eax = r.eax.wrapping_add(0x7fe0);
        m.write32(r.esp.wrapping_add(0x10), r.edi);
        m.write32(r.esp.wrapping_add(0x18), r.edi);
        m.write32(r.esp.wrapping_add(0x1c), r.edi);
        if r.ebp < r.eax {
            loop {
                r.ebx = r.ebp;
                r.eax = m.read32(r.ebx);
                r.ebp = r.ebp.wrapping_add(8);
                m.write32(r.esp.wrapping_add(0x14), r.ebx);
                if r.eax != 0xc7766a {
                    'at_23f75b: {
                        if r.eax != 0xc77668 {
                            if r.eax != 0xc77672 {
                                if r.eax != 0xc7766b {
                                    if r.eax != 0xc7766c {
                                        if r.eax != 0xc77669 {
                                            if r.eax == 0xc77673 {
                                                r.eax = m.read32(r.ebx.wrapping_add(4));
                                                r.esi = r.ebp;
                                                m.write32(r.esp.wrapping_add(0x14), r.eax);
                                                r.call(guest, 0x3210c0);
                                                r.edi = r.eax;
                                                r.eax = m.read32(r.esp.wrapping_add(0x14));
                                                r.ecx = r.eax;
                                                r.edx = r.ecx;
                                                r.ecx >>= 2;
                                                {
                                                    let step: u32 = 4;
                                                    for i in 0..r.ecx {
                                                        m.write32(
                                                            r.edi
                                                                .wrapping_add(i.wrapping_mul(step)),
                                                            m.read32(r.esi.wrapping_add(
                                                                i.wrapping_mul(step),
                                                            )),
                                                        );
                                                    }
                                                    r.esi = r
                                                        .esi
                                                        .wrapping_add(r.ecx.wrapping_mul(step));
                                                    r.edi = r
                                                        .edi
                                                        .wrapping_add(r.ecx.wrapping_mul(step));
                                                }
                                                r.ecx = 0;
                                                r.ecx = r.edx;
                                                r.ecx &= 3;
                                                for i in 0..r.ecx {
                                                    m.write8(
                                                        r.edi.wrapping_add(i),
                                                        m.read8(r.esi.wrapping_add(i)),
                                                    );
                                                }
                                                r.esi = r.esi.wrapping_add(r.ecx);
                                                r.edi = r.edi.wrapping_add(r.ecx);
                                                r.ecx = 0;
                                                break 'at_23f75b;
                                            }
                                            if r.eax != 0xc77674 {
                                                if r.eax != 0xc77675 {
                                                    break 'at_23f75b;
                                                }
                                                r.eax = 0;
                                                r.ecx = 0x2ee;
                                                r.edi = r.esp.wrapping_add(0x7f8);
                                                {
                                                    let step: u32 = 4;
                                                    for i in 0..r.ecx {
                                                        m.write32(
                                                            r.edi
                                                                .wrapping_add(i.wrapping_mul(step)),
                                                            r.eax,
                                                        );
                                                    }
                                                    r.edi = r
                                                        .edi
                                                        .wrapping_add(r.ecx.wrapping_mul(step));
                                                }
                                                r.ecx = 0;
                                                r.eax = m.read32(r.ebx.wrapping_add(4));
                                                r.ecx = r.eax;
                                                r.edx = r.ecx;
                                                r.ecx >>= 2;
                                                r.esi = r.ebp;
                                                r.edi = r.esp.wrapping_add(0x7f8);
                                                {
                                                    let step: u32 = 4;
                                                    for i in 0..r.ecx {
                                                        m.write32(
                                                            r.edi
                                                                .wrapping_add(i.wrapping_mul(step)),
                                                            m.read32(r.esi.wrapping_add(
                                                                i.wrapping_mul(step),
                                                            )),
                                                        );
                                                    }
                                                    r.esi = r
                                                        .esi
                                                        .wrapping_add(r.ecx.wrapping_mul(step));
                                                    r.edi = r
                                                        .edi
                                                        .wrapping_add(r.ecx.wrapping_mul(step));
                                                }
                                                r.ecx = 0;
                                                r.push(m, r.eax);
                                                r.ecx = r.edx;
                                                r.eax = r.esp.wrapping_add(0x7fc);
                                                r.ecx &= 3;
                                                r.push(m, r.eax);
                                                for i in 0..r.ecx {
                                                    m.write8(
                                                        r.edi.wrapping_add(i),
                                                        m.read8(r.esi.wrapping_add(i)),
                                                    );
                                                }
                                                r.esi = r.esi.wrapping_add(r.ecx);
                                                r.edi = r.edi.wrapping_add(r.ecx);
                                                r.ecx = 0;
                                                r.call(guest, 0x320fc0);
                                            } else {
                                                r.edx = m.read32(r.ebx.wrapping_add(4));
                                                r.eax = 0;
                                                r.ecx = 0x1f4;
                                                r.edi = r.esp.wrapping_add(0x28);
                                                {
                                                    let step: u32 = 4;
                                                    for i in 0..r.ecx {
                                                        m.write32(
                                                            r.edi
                                                                .wrapping_add(i.wrapping_mul(step)),
                                                            r.eax,
                                                        );
                                                    }
                                                    r.edi = r
                                                        .edi
                                                        .wrapping_add(r.ecx.wrapping_mul(step));
                                                }
                                                r.ecx = 0;
                                                r.ecx = r.edx;
                                                r.eax = r.ecx;
                                                r.ecx >>= 2;
                                                r.esi = r.ebp;
                                                r.edi = r.esp.wrapping_add(0x28);
                                                {
                                                    let step: u32 = 4;
                                                    for i in 0..r.ecx {
                                                        m.write32(
                                                            r.edi
                                                                .wrapping_add(i.wrapping_mul(step)),
                                                            m.read32(r.esi.wrapping_add(
                                                                i.wrapping_mul(step),
                                                            )),
                                                        );
                                                    }
                                                    r.esi = r
                                                        .esi
                                                        .wrapping_add(r.ecx.wrapping_mul(step));
                                                    r.edi = r
                                                        .edi
                                                        .wrapping_add(r.ecx.wrapping_mul(step));
                                                }
                                                r.ecx = 0;
                                                r.ecx = r.eax;
                                                r.eax = 0xcccccccdu32;
                                                {
                                                    let wide =
                                                        (r.eax as u64).wrapping_mul(r.edx as u64);
                                                    r.eax = wide as u32;
                                                    r.edx = (wide >> 32) as u32;
                                                }
                                                r.ecx &= 3;
                                                r.edx >>= 4;
                                                for i in 0..r.ecx {
                                                    m.write8(
                                                        r.edi.wrapping_add(i),
                                                        m.read8(r.esi.wrapping_add(i)),
                                                    );
                                                }
                                                r.esi = r.esi.wrapping_add(r.ecx);
                                                r.edi = r.edi.wrapping_add(r.ecx);
                                                r.ecx = 0;
                                                r.push(m, r.edx);
                                                r.ecx = r.esp.wrapping_add(0x2c);
                                                r.push(m, r.ecx);
                                                r.call(guest, 0x320ea0);
                                            }
                                        } else {
                                            r.ecx = m.read32(r.esp.wrapping_add(0x10));
                                            let limit = m.read32(r.ecx.wrapping_add(4));
                                            r.esi = r.ebp;
                                            m.write32(r.esp.wrapping_add(0x24), r.edi);
                                            if (limit as i32) <= (r.edi as i32) {
                                                break 'at_23f75b;
                                            }
                                            loop {
                                                r.edx = m.read32(r.esi);
                                                r.edi = m.read16(r.esi.wrapping_add(0x1e)) as u16
                                                    as u32;
                                                r.push(m, r.edx);
                                                r.call(guest, 0x333700);
                                                r.ebx = r.eax;
                                                r.eax = m.read32(r.esi);
                                                r.push(m, r.eax);
                                                r.call(guest, 0x2fdf50);
                                                'at_23f686: {
                                                    r.ebx = r.ebx.wrapping_add(r.eax);
                                                    r.esp = r.esp.wrapping_add(8);
                                                    r.esi = r
                                                        .esi
                                                        .wrapping_add(r.ebx.wrapping_mul(8))
                                                        .wrapping_add(0x28);
                                                    if (r.edi as i32) <= 0 {
                                                        r.ebx = m.read32(r.esp.wrapping_add(0x10));
                                                    } else {
                                                        m.write32(r.esp.wrapping_add(0x20), r.edi);
                                                        loop {
                                                            r.edi = ((m.read16(
                                                                r.esi.wrapping_add(0xe) as u32,
                                                            )
                                                                as i16)
                                                                as i32)
                                                                as u32;
                                                            r.ebx =
                                                                m.read32(r.esp.wrapping_add(0x10));
                                                            r.edi = ((r.edi as i32)
                                                                .wrapping_mul(0xc4i32))
                                                                as u32;
                                                            r.ecx = m
                                                                .read16(r.ebx.wrapping_add(0xa))
                                                                as u16
                                                                as u32;
                                                            r.push(m, r.ecx);
                                                            r.edi = r.edi.wrapping_add(0x524f58);
                                                            r.push(m, r.edi);
                                                            r.call(guest, 0x6bf70);
                                                            r.esp = r.esp.wrapping_add(8);
                                                            if r.eax == 0 {
                                                                r.eax = ((m
                                                                    .read16(r.esi.wrapping_add(0xe))
                                                                    as i16)
                                                                    as i32)
                                                                    as u32;
                                                                r.edx = m
                                                                    .read16(r.ebx.wrapping_add(0xa))
                                                                    as u16
                                                                    as u32;
                                                                r.eax = ((r.eax as i32)
                                                                    .wrapping_mul(0xc4i32))
                                                                    as u32;
                                                                r.push(m, r.edx);
                                                                r.eax =
                                                                    r.eax.wrapping_add(0x524f58);
                                                                r.push(m, r.eax);
                                                                r.call(guest, 0x6c090);
                                                                r.push(m, r.eax);
                                                                r.call(guest, 0xa5e40);
                                                                r.esp = r.esp.wrapping_add(0xc);
                                                            } else {
                                                                r.push(m, r.eax);
                                                                r.call(guest, 0x6b2d0);
                                                                r.esp = r.esp.wrapping_add(4);
                                                            }
                                                            'at_23f612: {
                                                                r.eax = set_lo16(
                                                                    r.eax,
                                                                    m.read16(r.edi.wrapping_add(2))
                                                                        as u32,
                                                                );
                                                                if [8, 9, 0x13, 0x14].contains(
                                                                    &((r.eax as u16) as u32),
                                                                ) && m.read32(r.esi) != 0
                                                                {
                                                                    r.push(m, 0x122);
                                                                    r.call(guest, 0xa5e40);
                                                                    r.esp = r.esp.wrapping_add(4);
                                                                }
                                                            }
                                                            'at_23f666: {
                                                                r.eax = ((m
                                                                    .read16(r.esi.wrapping_add(0xe))
                                                                    as i16)
                                                                    as i32)
                                                                    as u32;
                                                                r.ecx = r.eax.wrapping_add(
                                                                    (0x2bi32.wrapping_neg()) as u32,
                                                                );
                                                                if r.ecx <= 0xc {
                                                                    r.ecx = m.read8(
                                                                        r.ecx
                                                                            .wrapping_add(0x23f924),
                                                                    )
                                                                        as u8
                                                                        as u32;
                                                                    jt = m.read32(
                                                                        r.ecx
                                                                            .wrapping_mul(4)
                                                                            .wrapping_add(0x23f914),
                                                                    );
                                                                    if jt == 0x23f62c {
                                                                        r.edx = m.read16(
                                                                            r.ebx.wrapping_add(0xa),
                                                                        )
                                                                            as u16
                                                                            as u32;
                                                                        r.push(m, r.edx);
                                                                        r.push(m, r.eax);
                                                                        r.call(guest, 0x33dda0);
                                                                        r.push(m, r.eax);
                                                                        r.call(guest, 0x180700);
                                                                    } else if jt == 0x23f63f {
                                                                        r.eax = m.read32(
                                                                            r.esi.wrapping_add(4),
                                                                        );
                                                                        r.push(m, r.eax);
                                                                    } else if jt == 0x23f645 {
                                                                        r.ecx = ((m.read16(
                                                                            r.esi.wrapping_add(8),
                                                                        )
                                                                            as i16)
                                                                            as i32)
                                                                            as u32;
                                                                        r.push(m, r.ecx);
                                                                    } else if jt != 0x23f666 {
                                                                        r.tail_call(guest, jt);
                                                                        return;
                                                                    }
                                                                    if jt != 0x23f62c {
                                                                        if jt != 0x23f666 {
                                                                            r.call(guest, 0x1b6010);
                                                                            r.push(m, 0xd62);
                                                                            r.call(guest, 0x45590);
                                                                            r.push(m, 0xd63);
                                                                            r.call(guest, 0x45590);
                                                                        }
                                                                    }
                                                                    if jt != 0x23f666 {
                                                                        r.esp =
                                                                            r.esp.wrapping_add(0xc);
                                                                    }
                                                                }
                                                            }
                                                            r.edx = m
                                                                .read8(r.esi.wrapping_add(0x14))
                                                                as u8
                                                                as u32;
                                                            r.eax = r
                                                                .edx
                                                                .wrapping_mul(4)
                                                                .wrapping_add(0x1b)
                                                                & 0xfffffffcu32;
                                                            r.esi = r.esi.wrapping_add(r.eax);
                                                            m.write32(
                                                                r.esp.wrapping_add(0x20),
                                                                m.read32(r.esp.wrapping_add(0x20))
                                                                    .wrapping_sub(1),
                                                            );
                                                            if m.read32(r.esp.wrapping_add(0x20))
                                                                == 0
                                                            {
                                                                break 'at_23f686;
                                                            }
                                                        }
                                                    }
                                                }
                                                r.eax = m.read32(r.esp.wrapping_add(0x24));
                                                r.ecx = m.read32(r.ebx.wrapping_add(4));
                                                r.eax = r.eax.wrapping_add(1);
                                                m.write32(r.esp.wrapping_add(0x24), r.eax);
                                                if (r.eax as i32) >= (r.ecx as i32) {
                                                    r.ebx = m.read32(r.esp.wrapping_add(0x14));
                                                    break 'at_23f75b;
                                                }
                                            }
                                        }
                                    } else {
                                        r.eax = set_lo16(
                                            r.eax,
                                            m.read16(r.ebp.wrapping_add(0x2a)) as u32,
                                        );
                                        if (((r.eax as u16) as i16) as i32) <= -1 {
                                            break 'at_23f75b;
                                        }
                                        r.edx = (r.eax as u16) as i16 as i32 as u32;
                                        r.edx = ((r.edx as i32).wrapping_mul(0xc4i32)) as u32;
                                        r.eax = m.read32(r.edx.wrapping_add(0x524f60));
                                        r.push(m, r.eax);
                                        r.call(guest, 0xbe850);
                                        r.push(m, r.eax);
                                        r.call(guest, 0x6b2d0);
                                    }
                                    r.esp = r.esp.wrapping_add(8);
                                } else {
                                    r.eax = m.read8(r.ebp.wrapping_add(4)) as u8 as u32;
                                    r.eax = r.eax.wrapping_sub(r.edi);
                                    if r.eax == 0 {
                                        m.write32(r.esp.wrapping_add(0x18), 1);
                                    } else {
                                        r.eax = r.eax.wrapping_sub(1);
                                        if r.eax == 0 {
                                            m.write32(r.esp.wrapping_add(0x1c), 1);
                                        }
                                    }
                                }
                            } else {
                                r.ecx = 0x40;
                                r.esi = r.ebp;
                                r.edi = 0x783a80;
                                {
                                    let step: u32 = 4;
                                    for i in 0..r.ecx {
                                        m.write32(
                                            r.edi.wrapping_add(i.wrapping_mul(step)),
                                            m.read32(r.esi.wrapping_add(i.wrapping_mul(step))),
                                        );
                                    }
                                    r.esi = r.esi.wrapping_add(r.ecx.wrapping_mul(step));
                                    r.edi = r.edi.wrapping_add(r.ecx.wrapping_mul(step));
                                }
                                r.ecx = 0;
                            }
                        } else {
                            r.call(guest, 0x6a540);
                            r.esi = r.eax;
                            r.eax = m.read16(r.ebp.wrapping_add(0xa)) as u16 as u32;
                            r.push(m, r.eax);
                            m.write32(r.esp.wrapping_add(0x14), r.ebp);
                            r.call(guest, 0x311880);
                            m.write32(r.esi.wrapping_add(0x1c), r.eax);
                            r.ecx = m.read16(r.ebp.wrapping_add(0xa)) as u16 as u32;
                            r.push(m, r.ecx);
                            r.call(guest, 0x311870);
                            m.write32(r.esi.wrapping_add(0x14), r.eax);
                            r.edx = m.read16(r.ebp.wrapping_add(0xa)) as u16 as u32;
                            r.push(m, r.edx);
                            r.eax = r.esi.wrapping_add(0x2c);
                            r.push(m, r.eax);
                            r.call(guest, 0x3118b0);
                            r.ecx = m.read8(r.ebp.wrapping_add(0x12)) as u8 as u32;
                            r.eax = m.read32(r.ecx.wrapping_mul(8).wrapping_add(0x5196e0));
                            r.esp = r.esp.wrapping_add(0x10);
                            if r.eax != r.edi {
                                r.call(guest, r.eax);
                            }
                            if m.read32(r.ebp.wrapping_add(0x244)) & 0x20000 != 0 {
                                r.push(m, 0xffffffffu32);
                            } else {
                                r.edx = m.read8(r.ebp.wrapping_add(0x12)) as u8 as u32;
                                r.eax = m.read32(r.edx.wrapping_mul(8).wrapping_add(0x5196dc));
                                r.push(m, r.eax);
                            }
                            r.call(guest, 0x338150);
                            'at_23f482: {
                                r.ecx = set_lo8(r.ecx, gs::get_mode(m) as u32);
                                r.eax = ((m.read16(r.ebp.wrapping_add(0x14)) as i16) as i32) as u32;
                                r.esp = r.esp.wrapping_add(4);
                                if (r.ecx as u8) == 0xa {
                                    if (r.eax as i32) >= 0xc {
                                        r.ecx = r.eax.wrapping_add((0xci32.wrapping_neg()) as u32);
                                        if r.ecx == 5 {
                                            r.ebx = r.ebp.wrapping_add(0x16);
                                            r.call(guest, 0x23f320);
                                            r.ebx = m.read32(r.esp.wrapping_add(0x14));
                                            break 'at_23f482;
                                        }
                                    }
                                    r.call(guest, 0x23f350);
                                }
                            }
                            r.eax = set_lo16(r.eax, m.read16(r.ebp.wrapping_add(0x10)) as u32);
                            if (((r.eax as u16) as i16) as i32) <= -1 {
                                r.eax = m.read16(r.ebp.wrapping_add(0xa)) as u16 as u32;
                                r.push(m, r.eax);
                                r.call(guest, 0x311860);
                                r.esp = r.esp.wrapping_add(4);
                                m.write32(r.esi.wrapping_add(0x10), r.eax);
                            } else {
                                r.edx = (r.eax as u16) as i16 as i32 as u32;
                                m.write32(r.esi.wrapping_add(0x10), r.edx);
                            }
                            r.ecx = m.read32(r.esi.wrapping_add(0x10));
                            r.ecx = ((r.ecx as i32).wrapping_mul(0x34i32)) as u32;
                            r.eax = m.read32(r.ecx.wrapping_add(0x5149ac));
                            if r.eax != 0xffffffffu32 {
                                r.push(m, r.eax);
                                r.call(guest, 0xa5e40);
                                r.esp = r.esp.wrapping_add(4);
                            }
                        }
                    }
                    r.ebp = r.ebp.wrapping_add(m.read32(r.ebx.wrapping_add(4)));
                    r.ecx = m.read32(0x784014);
                    r.ecx = r.ecx.wrapping_add(0x7fe0);
                    r.edi = 0;
                    if r.ebp < r.ecx {
                        continue;
                    }
                }
                if m.read32(r.esp.wrapping_add(0x18)) != r.edi {
                    r.edx = m.read32(r.esp.wrapping_add(0x10));
                    r.eax = m.read16(r.edx.wrapping_add(0xa)) as u16 as u32;
                    if r.eax <= 5 {
                        jt = m.read32(r.eax.wrapping_mul(4).wrapping_add(0x23f934));
                        if jt == 0x23f792 {
                            r.push(m, 0x87a);
                            r.call(guest, 0xa5e40);
                            r.push(m, 0x87b);
                        } else if jt == 0x23f7a3 {
                            r.push(m, 0x87c);
                            r.call(guest, 0xa5e40);
                            r.push(m, 0x87d);
                        } else if jt == 0x23f7b4 {
                            r.push(m, 0x87e);
                            r.call(guest, 0xa5e40);
                            r.push(m, 0x87f);
                        } else if jt == 0x23f7c5 {
                            r.push(m, 0x880);
                            r.call(guest, 0xa5e40);
                            r.push(m, 0x881);
                        } else if jt == 0x23f7d6 {
                            r.push(m, 0x882);
                            r.call(guest, 0xa5e40);
                            r.push(m, 0x883);
                        } else if jt == 0x23f7e7 {
                            r.push(m, 0x884);
                            r.call(guest, 0xa5e40);
                            r.push(m, 0x885);
                        } else {
                            r.tail_call(guest, jt);
                            return;
                        }
                        r.call(guest, 0xa5e40);
                        r.esp = r.esp.wrapping_add(8);
                    }
                }
                if m.read32(r.esp.wrapping_add(0x1c)) == r.edi {
                    break 'at_23f888;
                }
                r.eax = m.read32(r.esp.wrapping_add(0x10));
                r.eax = m.read16(r.eax.wrapping_add(0xa)) as u16 as u32;
                if r.eax > 5 {
                    break 'at_23f888;
                }
                jt = m.read32(r.eax.wrapping_mul(4).wrapping_add(0x23f94c));
                if jt == 0x23f81c {
                    r.push(m, 0x886);
                    r.call(guest, 0xa5e40);
                    r.push(m, 0x887);
                } else if jt == 0x23f82d {
                    r.push(m, 0x888);
                    r.call(guest, 0xa5e40);
                    r.push(m, 0x889);
                } else if jt == 0x23f83e {
                    r.push(m, 0x88c);
                    r.call(guest, 0xa5e40);
                    r.push(m, 0x88d);
                } else if jt == 0x23f84f {
                    r.push(m, 0x88a);
                    r.call(guest, 0xa5e40);
                    r.push(m, 0x88b);
                } else if jt == 0x23f860 {
                    r.push(m, 0x88e);
                    r.call(guest, 0xa5e40);
                    r.push(m, 0x88f);
                } else if jt == 0x23f871 {
                    r.push(m, 0x890);
                    r.call(guest, 0xa5e40);
                    r.push(m, 0x891);
                } else {
                    r.tail_call(guest, jt);
                    return;
                }
                r.call(guest, 0xa5e40);
                r.esp = r.esp.wrapping_add(8);
                break 'at_23f888;
            }
        }
    }
    'at_23f8aa: {
        if gs::get_mode(m) as u32 == 2 {
            r.esi = 0;
            loop {
                r.push(m, r.esi);
                r.push(m, 1);
                r.call(guest, 0xbe860);
                r.push(m, r.eax);
                r.call(guest, 0x6b2d0);
                r.esp = r.esp.wrapping_add(0xc);
                r.esi = r.esi.wrapping_add(1);
                if (r.esi as i32) >= 2 {
                    break 'at_23f8aa;
                }
            }
        }
    }
    if gs::get_mode(m) as u32 == 0xa {
        r.push(m, 0x24);
        r.call(guest, 0x1b6030);
        r.esp = r.esp.wrapping_add(4);
    }
    r.call(guest, 0x65250);
    if r.eax != 0 {
        r.push(m, 0xe5);
        r.call(guest, 0xa5e40);
        r.esp = r.esp.wrapping_add(4);
    }
    r.eax = 0;
    r.ecx = 0x55;
    r.edi = 0x78cc00;
    r.push(m, 0x51);
    {
        let step: u32 = 4;
        for i in 0..r.ecx {
            m.write32(r.edi.wrapping_add(i.wrapping_mul(step)), r.eax);
        }
        r.edi = r.edi.wrapping_add(r.ecx.wrapping_mul(step));
    }
    r.ecx = 0;
    r.call(guest, 0x45590);
    r.push(m, 0x1a6);
    r.call(guest, 0x45590);
    r.push(m, 0x1a7);
    r.call(guest, 0x45590);
    r.push(m, 0x631);
    r.call(guest, 0x45590);
    r.esp = r.esp.wrapping_add(0x10);
    r.edi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.esi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ebp = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ebx = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.esp = r.ebp;
    r.ebp = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
}

/// 0x2540B0. INFERRED: Updates a deferred object using rotation and translation matrices.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn sub_002540b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, r: &mut ExtRegisters) {
    let (mut fca, mut fcb) = (0f64, 0f64);
    r.edx = m.read32(0x78b560);
    r.xmm[0] = Xmm::default();
    r.esp = r.esp.wrapping_sub(0xcc);
    r.push(m, r.esi);
    r.esi = m.read32(r.esp.wrapping_add(0xd4));
    r.esi = ((r.esi as i32).wrapping_mul(0x4ci32)) as u32;
    r.eax = m.read32(r.esi.wrapping_add(r.edx).wrapping_add(0x20));
    r.esi = r.esi.wrapping_add(r.edx);
    r.eax = r.eax.wrapping_sub(0);
    r.xmm[1] = r.xmm[0];
    if r.eax == 0 {
        r.xmm[1].set_f(0, (m.read32(0x4e8cec) as i32) as f32);
        r.xmm[1].set_f(0, r.xmm[1].f(0) * f32::from_bits(m.read32(0x475ce8)));
    } else {
        r.eax = r.eax.wrapping_sub(1);
        if r.eax == 0 {
            r.xmm[1].set_f(0, (m.read32(0x4e8cec) as i32) as f32);
            r.xmm[1].set_f(0, r.xmm[1].f(0) * f32::from_bits(m.read32(0x475d10)));
        }
    }
    r.xmm[2].set_f(0, (m.read32(r.esi.wrapping_add(0x1c)) as i32) as f32);
    fca = f32::from_bits(r.xmm[2].lanes[0]) as f64;
    fcb = f32::from_bits(r.xmm[1].lanes[0]) as f64;
    if (fca <= fcb) || ((fca != fca) || (fcb != fcb)) {
        r.eax = 0;
        r.esi = m.read32(r.esp);
        r.esp = r.esp.wrapping_add(4);
        r.esp = r.esp.wrapping_add(0xcc);
        return;
    }
    r.xmm[1] = Xmm::scalar(f32::from_bits(m.read32(0x4760d0)).to_bits());
    r.ecx = m.read32(r.esi.wrapping_add(0x34));
    r.eax = m.read32(r.edi.wrapping_add(4));
    r.edx = r.ecx;
    r.edx = ((r.edx as i32).wrapping_shr(16)) as u32;
    m.write32(r.esp.wrapping_add(0x10), r.xmm[1].f(0).to_bits());
    m.write32(r.esp.wrapping_add(8), r.xmm[0].f(0).to_bits());
    m.write32(r.esp.wrapping_add(0xc), r.xmm[0].f(0).to_bits());
    m.write32(r.esp.wrapping_add(0x20), r.xmm[0].f(0).to_bits());
    m.write32(r.esp.wrapping_add(0x24), r.xmm[0].f(0).to_bits());
    m.write32(r.esp.wrapping_add(0x28), r.xmm[0].f(0).to_bits());
    r.eax = m.read32(r.eax.wrapping_add(4));
    r.push(m, r.ebx);
    r.push(m, r.ebp);
    r.ebp = m.read32(r.eax);
    r.edx = r.edx.wrapping_sub(r.ebp);
    r.edx = ((r.edx as i32).wrapping_mul(0xc8i32)) as u32;
    r.edx = m.read32(r.edx.wrapping_add(r.eax).wrapping_add(0x78));
    r.ecx = r.ecx & 0xffff;
    r.ecx = r.ecx.wrapping_add(r.ecx.wrapping_mul(2));
    r.ecx = r.edx.wrapping_add(r.ecx.wrapping_mul(8));
    m.write32(r.esp.wrapping_add(0x54), r.ecx);
    r.ecx = m.read32(r.esi.wrapping_add(0x30));
    r.edx = r.ecx;
    r.edx = ((r.edx as i32).wrapping_shr(16)) as u32;
    r.ebx = r.ebp;
    r.edx = r.edx.wrapping_sub(r.ebx);
    r.edx = ((r.edx as i32).wrapping_mul(0xc8i32)) as u32;
    r.edx = m.read32(r.edx.wrapping_add(r.eax).wrapping_add(0x78));
    r.ecx = r.ecx & 0xffff;
    r.ecx = r.ecx.wrapping_add(r.ecx.wrapping_mul(2));
    r.ecx = r.edx.wrapping_add(r.ecx.wrapping_mul(8));
    m.write32(r.esp.wrapping_add(0x4c), r.ecx);
    r.ecx = m.read32(r.esi.wrapping_add(0x2c));
    r.edx = r.ecx;
    r.xmm[1] = Xmm::scalar(f32::from_bits(m.read32(r.esi)).to_bits());
    r.xmm[1].set_f(
        0,
        r.xmm[1].f(0) + f32::from_bits(m.read32(r.edi.wrapping_add(0x70))),
    );
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x475d0c)).to_bits());
    r.edx = ((r.edx as i32).wrapping_shr(16)) as u32;
    r.edx = r.edx.wrapping_sub(r.ebp);
    r.edx = ((r.edx as i32).wrapping_mul(0xc8i32)) as u32;
    r.edx = m.read32(r.edx.wrapping_add(r.eax).wrapping_add(0x78));
    r.ecx = r.ecx & 0xffff;
    r.ecx = r.ecx.wrapping_add(r.ecx.wrapping_mul(2));
    r.ecx = r.edx.wrapping_add(r.ecx.wrapping_mul(8));
    r.edx = r.ebp;
    r.edx = ((r.edx as i32).wrapping_mul(0xc8i32)) as u32;
    r.push(m, 0);
    r.xmm[1].set_f(0, r.xmm[1].f(0) * r.xmm[0].f(0));
    r.esp = r.esp.wrapping_sub(8);
    m.write32(r.esp.wrapping_add(4), r.xmm[1].f(0).to_bits());
    r.xmm[1] = Xmm::scalar(f32::from_bits(m.read32(r.esi.wrapping_add(0xc))).to_bits());
    m.write32(r.esp.wrapping_add(0x5c), r.ecx);
    r.eax = r.eax.wrapping_sub(r.edx);
    r.eax = m.read32(r.eax.wrapping_add(0x2d0));
    r.xmm[1].set_f(0, r.xmm[1].f(0) * r.xmm[0].f(0));
    r.ecx = r.esp.wrapping_add(0x64);
    m.write32(r.esp, r.xmm[1].f(0).to_bits());
    r.push(m, r.ecx);
    m.write32(r.esp.wrapping_add(0x58), r.eax);
    r.call(guest, 0x151650);
    r.edx = r.esp.wrapping_add(0x20);
    r.push(m, r.edx);
    r.eax = r.esp.wrapping_add(0x6c);
    r.push(m, r.eax);
    r.call(guest, 0x151e80);
    r.eax = m.read32(r.esi.wrapping_add(0x2c));
    r.edx = m.read32(r.edi.wrapping_add(4));
    r.ebx = m.read32(r.edx.wrapping_add(4));
    r.ecx = m.read32(r.ebx);
    r.ebp = r.eax;
    r.ebp = ((r.ebp as i32).wrapping_shr(16)) as u32;
    r.ebp = r.ebp.wrapping_sub(r.ecx);
    r.ebp = ((r.ebp as i32).wrapping_mul(0xc8i32)) as u32;
    r.ebx = m.read32(r.ebx.wrapping_add(r.ebp).wrapping_add(0x78));
    r.ecx = r.eax;
    r.ecx = r.ecx & 0xffff;
    r.ecx = r.ecx.wrapping_add(r.ecx.wrapping_mul(2));
    r.xmm[0] =
        Xmm::scalar(f32::from_bits(m.read32(r.ebx.wrapping_add(r.ecx.wrapping_mul(8)))).to_bits());
    m.write32(r.esp.wrapping_add(0x34), r.xmm[0].f(0).to_bits());
    r.ebx = m.read32(r.edx.wrapping_add(4));
    r.ecx = m.read32(r.ebx);
    r.ebp = r.eax;
    r.ebp = ((r.ebp as i32).wrapping_shr(16)) as u32;
    r.ebp = r.ebp.wrapping_sub(r.ecx);
    r.ebp = ((r.ebp as i32).wrapping_mul(0xc8i32)) as u32;
    r.ebx = m.read32(r.ebx.wrapping_add(r.ebp).wrapping_add(0x78));
    r.ecx = r.eax;
    r.ecx = r.ecx & 0xffff;
    r.ecx = r.ecx.wrapping_add(r.ecx.wrapping_mul(2));
    r.xmm[0] = Xmm::scalar(
        f32::from_bits(m.read32(r.ebx.wrapping_add(r.ecx.wrapping_mul(8)).wrapping_add(4)))
            .to_bits(),
    );
    m.write32(r.esp.wrapping_add(0x38), r.xmm[0].f(0).to_bits());
    r.edx = m.read32(r.edx.wrapping_add(4));
    r.ebx = m.read32(r.edx);
    r.ecx = r.eax;
    r.ecx = ((r.ecx as i32).wrapping_shr(16)) as u32;
    r.ecx = r.ecx.wrapping_sub(r.ebx);
    r.ecx = ((r.ecx as i32).wrapping_mul(0xc8i32)) as u32;
    r.ecx = m.read32(r.ecx.wrapping_add(r.edx).wrapping_add(0x78));
    r.edx = m.read32(r.edi.wrapping_add(0x6c));
    r.eax = r.eax & 0xffff;
    r.eax = r.eax.wrapping_add(r.eax.wrapping_mul(2));
    r.xmm[0] = Xmm::scalar(
        f32::from_bits(m.read32(r.ecx.wrapping_add(r.eax.wrapping_mul(8)).wrapping_add(8)))
            .to_bits(),
    );
    r.eax = m.read32(r.edi.wrapping_add(0x68));
    r.ecx = m.read32(r.edi.wrapping_add(0x64));
    r.push(m, r.edx);
    r.push(m, r.eax);
    r.push(m, r.ecx);
    r.edx = r.esp.wrapping_add(0x7c);
    r.push(m, r.edx);
    m.write32(r.esp.wrapping_add(0x4c), r.xmm[0].f(0).to_bits());
    r.call(guest, 0x153550);
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.edi.wrapping_add(0x70))).to_bits());
    r.xmm[0].set_f(0, r.xmm[0].f(0) * f32::from_bits(m.read32(0x475d0c)));
    r.esp = r.esp.wrapping_add(0x24);
    r.eax = r.esp.wrapping_add(0x9c);
    m.write32(r.esp, r.xmm[0].f(0).to_bits());
    r.push(m, r.eax);
    r.call(guest, 0x153440);
    r.ecx = r.esp.wrapping_add(0xa0);
    r.push(m, r.ecx);
    r.edx = r.esp.wrapping_add(0x64);
    r.push(m, r.edx);
    r.call(guest, 0x153650);
    r.eax = r.esp.wrapping_add(0x2c);
    r.push(m, r.eax);
    r.ecx = r.esp.wrapping_add(0x6c);
    r.push(m, r.ecx);
    r.call(guest, 0x151e80);
    r.xmm[3] = Xmm::scalar(f32::from_bits(m.read32(r.esi.wrapping_add(0x14))).to_bits());
    r.xmm[3].set_f(
        0,
        r.xmm[3].f(0) - f32::from_bits(m.read32(r.esp.wrapping_add(0x38))),
    );
    r.xmm[2] = Xmm::scalar(f32::from_bits(m.read32(r.esi.wrapping_add(0x18))).to_bits());
    r.xmm[2].set_f(
        0,
        r.xmm[2].f(0) - f32::from_bits(m.read32(r.esp.wrapping_add(0x3c))),
    );
    r.xmm[1] = Xmm::scalar(f32::from_bits(m.read32(r.esi.wrapping_add(0x10))).to_bits());
    r.xmm[1].set_f(
        0,
        r.xmm[1].f(0) - f32::from_bits(m.read32(r.esp.wrapping_add(0x34))),
    );
    r.xmm[4] = r.xmm[3];
    r.xmm[4].set_f(0, r.xmm[4].f(0) * r.xmm[3].f(0));
    r.xmm[0] = r.xmm[2];
    r.xmm[0].set_f(0, r.xmm[0].f(0) * r.xmm[2].f(0));
    r.xmm[0].set_f(0, r.xmm[0].f(0) + r.xmm[4].f(0));
    r.xmm[4] = Xmm::scalar(f32::from_bits(m.read32(0x475c78)).to_bits());
    r.xmm[3] = r.xmm[1];
    r.xmm[3].set_f(0, r.xmm[3].f(0) * r.xmm[1].f(0));
    r.xmm[0].set_f(0, r.xmm[0].f(0) + r.xmm[3].f(0));
    r.xmm[3] = Xmm::scalar(f32::from_bits(m.read32(0x4765f4)).to_bits());
    r.esp = r.esp.wrapping_add(0x18);
    fca = f32::from_bits(r.xmm[0].lanes[0]) as f64;
    fcb = f32::from_bits(r.xmm[3].lanes[0]) as f64;
    m.write32(r.esp.wrapping_add(0x34), r.xmm[1].f(0).to_bits());
    m.write32(r.esp.wrapping_add(0x3c), r.xmm[2].f(0).to_bits());
    if !((fca <= fcb) || ((fca != fca) || (fcb != fcb))) {
        m.write32(r.esp.wrapping_add(0x40), r.xmm[0].f(0).to_bits());
        m.write32(r.esp.wrapping_add(0xc), r.xmm[4].f(0).to_bits());
        r.xmm[0].set_f(
            0,
            1.0f32 / sqrtf(f32::from_bits(m.read32(r.esp.wrapping_add(0x40)))),
        );
        r.xmm[0].set_f(
            0,
            r.xmm[0].f(0) * f32::from_bits(m.read32(r.esp.wrapping_add(0xc))),
        );
        m.write32(r.esp.wrapping_add(0x44), r.xmm[0].f(0).to_bits());
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x44))).to_bits());
        r.xmm_hi[0] = r.xmm[0];
        r.xmm_hi[0].set_f(0, r.xmm_hi[0].f(0) * r.xmm[1].f(0));
        r.xmm[0].set_f(0, r.xmm[0].f(0) * r.xmm[2].f(0));
        m.write32(r.esp.wrapping_add(0x34), r.xmm_hi[0].f(0).to_bits());
        m.write32(r.esp.wrapping_add(0x3c), r.xmm[0].f(0).to_bits());
    }
    r.xmm[1] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x14))).to_bits());
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x18))).to_bits());
    r.xmm[2] = r.xmm[1];
    r.xmm[2].set_f(0, r.xmm[2].f(0) * r.xmm[1].f(0));
    r.xmm[1] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x10))).to_bits());
    r.xmm[0].set_f(0, r.xmm[0].f(0) * r.xmm[0].f(0));
    r.xmm[0].set_f(0, r.xmm[0].f(0) + r.xmm[2].f(0));
    r.xmm[2] = r.xmm[1];
    r.xmm[2].set_f(0, r.xmm[2].f(0) * r.xmm[1].f(0));
    r.xmm[0].set_f(0, r.xmm[0].f(0) + r.xmm[2].f(0));
    fca = f32::from_bits(r.xmm[0].lanes[0]) as f64;
    fcb = f32::from_bits(r.xmm[3].lanes[0]) as f64;
    if !((fca <= fcb) || ((fca != fca) || (fcb != fcb))) {
        m.write32(r.esp.wrapping_add(0x40), r.xmm[0].f(0).to_bits());
        m.write32(r.esp.wrapping_add(0x44), r.xmm[4].f(0).to_bits());
        r.xmm[0].set_f(
            0,
            1.0f32 / sqrtf(f32::from_bits(m.read32(r.esp.wrapping_add(0x40)))),
        );
        r.xmm[0].set_f(
            0,
            r.xmm[0].f(0) * f32::from_bits(m.read32(r.esp.wrapping_add(0x44))),
        );
        m.write32(r.esp.wrapping_add(0xc), r.xmm[0].f(0).to_bits());
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0xc))).to_bits());
        r.xmm[1] = r.xmm[0];
        r.xmm[1].set_f(
            0,
            r.xmm[1].f(0) * f32::from_bits(m.read32(r.esp.wrapping_add(0x10))),
        );
        m.write32(r.esp.wrapping_add(0x10), r.xmm[1].f(0).to_bits());
        r.xmm[1] = r.xmm[0];
        r.xmm[1].set_f(
            0,
            r.xmm[1].f(0) * f32::from_bits(m.read32(r.esp.wrapping_add(0x14))),
        );
        r.xmm[0].set_f(
            0,
            r.xmm[0].f(0) * f32::from_bits(m.read32(r.esp.wrapping_add(0x18))),
        );
        m.write32(r.esp.wrapping_add(0x14), r.xmm[1].f(0).to_bits());
        m.write32(r.esp.wrapping_add(0x18), r.xmm[0].f(0).to_bits());
    }
    r.fpu
        .push(f32::from_bits(m.read32(r.esp.wrapping_add(0x34))) as f64);
    r.esp = r.esp.wrapping_sub(8);
    r.fpu
        .push(f32::from_bits(m.read32(r.esp.wrapping_add(0x44))) as f64);
    r.fpu.set_st(1, atan2(r.fpu.st(1), r.fpu.st(0)));
    r.fpu.pop();
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) * (f32::from_bits(m.read32(0x478638)) as f64),
        ),
    );
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) - (f32::from_bits(m.read32(r.edi.wrapping_add(0x70))) as f64),
        ),
    );
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) + (f32::from_bits(m.read32(0x475c84)) as f64),
        ),
    );
    m.write32(r.esp.wrapping_add(0x14), (r.fpu.st(0) as f32).to_bits());
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) - (f32::from_bits(m.read32(0x475c8c)) as f64),
        ),
    );
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) * (f32::from_bits(m.read32(0x475cb4)) as f64),
        ),
    );
    write64(m, r.esp, r.fpu.st(0).to_bits());
    r.fpu.pop();
    r.call(guest, 0x3c8501);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) * (f32::from_bits(m.read32(0x475c88)) as f64),
        ),
    );
    r.push(m, r.esi);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            (f32::from_bits(m.read32(r.esp.wrapping_add(0x18))) as f64) - r.fpu.st(0),
        ),
    );
    m.write32(r.esp.wrapping_add(0x18), (r.fpu.st(0) as f32).to_bits());
    r.fpu.pop();
    r.edx = m.read32(r.esp.wrapping_add(0x18));
    r.push(m, r.edx);
    r.call(guest, m.read32(r.esi.wrapping_add(0x44)));
    r.esp = r.esp.wrapping_add(0x10);
    if r.eax != 0 {
        r.eax = m.read32(r.esp.wrapping_add(0x4c));
        r.ecx = m.read32(r.esp.wrapping_add(0x54));
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.ecx)).to_bits());
        r.xmm[0].set_f(0, r.xmm[0].f(0) - f32::from_bits(m.read32(r.eax)));
        r.ebx = m.read32(r.esp.wrapping_add(0x48));
        m.write32(r.esp.wrapping_add(0x28), r.xmm[0].f(0).to_bits());
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.ecx.wrapping_add(4))).to_bits());
        r.xmm[0].set_f(
            0,
            r.xmm[0].f(0) - f32::from_bits(m.read32(r.eax.wrapping_add(4))),
        );
        m.write32(r.esp.wrapping_add(0x2c), r.xmm[0].f(0).to_bits());
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.ecx.wrapping_add(8))).to_bits());
        r.xmm[0].set_f(
            0,
            r.xmm[0].f(0) - f32::from_bits(m.read32(r.eax.wrapping_add(8))),
        );
        r.ecx = m.read32(r.esp.wrapping_add(0x50));
        m.write32(r.esp.wrapping_add(0x30), r.xmm[0].f(0).to_bits());
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.ecx)).to_bits());
        r.xmm[0].set_f(0, r.xmm[0].f(0) - f32::from_bits(m.read32(r.ebx)));
        m.write32(r.esp.wrapping_add(0x1c), r.xmm[0].f(0).to_bits());
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.ecx.wrapping_add(4))).to_bits());
        r.xmm[0].set_f(
            0,
            r.xmm[0].f(0) - f32::from_bits(m.read32(r.ebx.wrapping_add(4))),
        );
        m.write32(r.esp.wrapping_add(0x20), r.xmm[0].f(0).to_bits());
        r.fpu
            .push(f32::from_bits(m.read32(r.ecx.wrapping_add(8))) as f64);
        r.fpu.set_st(
            0,
            fp_pc(
                &r.fpu,
                r.fpu.st(0) - (f32::from_bits(m.read32(r.ebx.wrapping_add(8))) as f64),
            ),
        );
        m.write32(r.esp.wrapping_add(0x24), (r.fpu.st(0) as f32).to_bits());
        r.fpu.pop();
        r.fpu.push(f32::from_bits(m.read32(r.eax)) as f64);
        r.fpu.set_st(
            0,
            fp_pc(
                &r.fpu,
                r.fpu.st(0) - (f32::from_bits(m.read32(r.ecx)) as f64),
            ),
        );
        m.write32(r.esp.wrapping_add(0x34), (r.fpu.st(0) as f32).to_bits());
        r.fpu.pop();
        r.fpu
            .push(f32::from_bits(m.read32(r.eax.wrapping_add(4))) as f64);
        r.edx = m.read32(r.esp.wrapping_add(0x34));
        r.fpu.set_st(
            0,
            fp_pc(
                &r.fpu,
                r.fpu.st(0) - (f32::from_bits(m.read32(r.ecx.wrapping_add(4))) as f64),
            ),
        );
        m.write32(r.esp.wrapping_add(0x38), (r.fpu.st(0) as f32).to_bits());
        r.fpu.pop();
        r.fpu
            .push(f32::from_bits(m.read32(r.eax.wrapping_add(8))) as f64);
        r.fpu.set_st(
            0,
            fp_pc(
                &r.fpu,
                r.fpu.st(0) - (f32::from_bits(m.read32(r.ecx.wrapping_add(8))) as f64),
            ),
        );
        r.ecx = m.read32(r.esp.wrapping_add(0x38));
        m.write32(r.esp.wrapping_add(0x3c), (r.fpu.st(0) as f32).to_bits());
        r.fpu.pop();
        r.eax = m.read32(r.esp.wrapping_add(0x3c));
        r.push(m, r.eax);
        r.push(m, r.ecx);
        r.push(m, r.edx);
        r.eax = r.esp.wrapping_add(0x64);
        r.push(m, r.eax);
        r.call(guest, 0x153550);
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esi.wrapping_add(0xc))).to_bits());
        r.xmm[0].set_f(0, r.xmm[0].f(0) * f32::from_bits(m.read32(0x475d0c)));
        r.esp = r.esp.wrapping_add(0xc);
        r.ecx = r.esp.wrapping_add(0x9c);
        m.write32(r.esp, r.xmm[0].f(0).to_bits());
        r.push(m, r.ecx);
        r.call(guest, 0x1533e0);
        r.edx = r.esp.wrapping_add(0xa0);
        r.push(m, r.edx);
        r.eax = r.esp.wrapping_add(0x64);
        r.push(m, r.eax);
        r.call(guest, 0x153650);
        r.ecx = r.esp.wrapping_add(0x38);
        r.push(m, r.ecx);
        r.edx = r.esp.wrapping_add(0x6c);
        r.push(m, r.edx);
        r.call(guest, 0x151e80);
        r.eax = m.read32(r.esp.wrapping_add(0x3c));
        r.ecx = m.read32(r.esp.wrapping_add(0x38));
        r.edx = m.read32(r.esp.wrapping_add(0x34));
        r.push(m, r.eax);
        r.push(m, r.ecx);
        r.push(m, r.edx);
        r.eax = r.esp.wrapping_add(0x7c);
        r.push(m, r.eax);
        r.call(guest, 0x153550);
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esi)).to_bits());
        r.xmm[0].set_f(0, r.xmm[0].f(0) * f32::from_bits(m.read32(0x475d0c)));
        r.esp = r.esp.wrapping_add(0x24);
        r.ecx = r.esp.wrapping_add(0x9c);
        m.write32(r.esp, r.xmm[0].f(0).to_bits());
        r.push(m, r.ecx);
        r.call(guest, 0x153440);
        r.edx = r.esp.wrapping_add(0xa0);
        r.push(m, r.edx);
        r.eax = r.esp.wrapping_add(0x64);
        r.push(m, r.eax);
        r.call(guest, 0x153650);
        r.ecx = r.esp.wrapping_add(0x38);
        r.push(m, r.ecx);
        r.edx = r.esp.wrapping_add(0x6c);
        r.push(m, r.edx);
        r.call(guest, 0x151e80);
        r.eax = m.read32(r.ebx.wrapping_add(8));
        r.ecx = m.read32(r.ebx.wrapping_add(4));
        r.edx = m.read32(r.ebx);
        r.push(m, r.eax);
        r.push(m, r.ecx);
        r.push(m, r.edx);
        r.eax = r.esp.wrapping_add(0x7c);
        r.push(m, r.eax);
        r.call(guest, 0x153550);
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x78b578)).to_bits());
        r.xmm[0].set_f(0, r.xmm[0].f(0) * f32::from_bits(m.read32(0x475d0c)));
        r.esp = r.esp.wrapping_add(0x24);
        r.ecx = r.esp.wrapping_add(0x9c);
        m.write32(r.esp, r.xmm[0].f(0).to_bits());
        r.push(m, r.ecx);
        r.call(guest, 0x1533e0);
        r.edx = r.esp.wrapping_add(0xa0);
        r.push(m, r.edx);
        r.eax = r.esp.wrapping_add(0x64);
        r.push(m, r.eax);
        r.call(guest, 0x153650);
        r.ecx = r.esp.wrapping_add(0x38);
        r.push(m, r.ecx);
        r.edx = r.esp.wrapping_add(0x6c);
        r.push(m, r.edx);
        r.call(guest, 0x151e80);
        r.eax = m.read32(r.edi.wrapping_add(0x6c));
        r.ecx = m.read32(r.edi.wrapping_add(0x68));
        r.edx = m.read32(r.edi.wrapping_add(0x64));
        r.push(m, r.eax);
        r.push(m, r.ecx);
        r.push(m, r.edx);
        r.eax = r.esp.wrapping_add(0x7c);
        r.push(m, r.eax);
        r.call(guest, 0x153550);
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.edi.wrapping_add(0x70))).to_bits());
        r.xmm[0].set_f(0, r.xmm[0].f(0) * f32::from_bits(m.read32(0x475d0c)));
        r.esp = r.esp.wrapping_add(0x24);
        r.ecx = r.esp.wrapping_add(0x9c);
        m.write32(r.esp, r.xmm[0].f(0).to_bits());
        r.push(m, r.ecx);
        r.call(guest, 0x153440);
        r.edx = r.esp.wrapping_add(0xa0);
        r.push(m, r.edx);
        r.eax = r.esp.wrapping_add(0x64);
        r.push(m, r.eax);
        r.call(guest, 0x153650);
        r.ecx = r.esp.wrapping_add(0x38);
        r.push(m, r.ecx);
        r.edx = r.esp.wrapping_add(0x6c);
        r.push(m, r.edx);
        r.call(guest, 0x151e80);
        r.eax = m.read32(r.esp.wrapping_add(0xf4));
        r.push(m, r.eax);
        r.ecx = r.esp.wrapping_add(0x2c);
        r.push(m, r.ecx);
        r.edx = r.esp.wrapping_add(0x48);
        r.push(m, r.edx);
        r.push(m, r.edi);
        r.call(guest, m.read32(r.esi.wrapping_add(0x3c)));
        r.esp = r.esp.wrapping_add(0x28);
        r.ebp = m.read32(r.esp);
        r.esp = r.esp.wrapping_add(4);
        r.ebx = m.read32(r.esp);
        r.esp = r.esp.wrapping_add(4);
        m.write32(r.esi.wrapping_add(0x1c), 0);
        r.eax = 1;
        r.esi = m.read32(r.esp);
        r.esp = r.esp.wrapping_add(4);
        r.esp = r.esp.wrapping_add(0xcc);
    } else {
        r.ebp = m.read32(r.esp);
        r.esp = r.esp.wrapping_add(4);
        r.ebx = m.read32(r.esp);
        r.esp = r.esp.wrapping_add(4);
        r.esi = m.read32(r.esp);
        r.esp = r.esp.wrapping_add(4);
        r.esp = r.esp.wrapping_add(0xcc);
    }
}

pub const ROOTS: &[u32] = &[0x674c0, 0x681c0, 0x68830, 0x231a20, 0x23f390, 0x2540b0];

/// Runs the readable version of `va`; false when `va` has none.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x674c0 => {
            game_match_per_frame_rules_timers_limits_and_leader_messages_when_byte_7de456_modes(
                m, guest, regs,
            );
            true
        }
        0x681c0 => {
            game_match_time_limit_countdown_warnings_sounds_0xd49_0xd4a_0xd48_and_end_of_match(
                m, guest, regs,
            );
            true
        }
        0x68830 => {
            game_match_state_machine_tick_dispatch_by_7de30c_jump_table_12_states(m, guest, regs);
            true
        }
        0x231a20 => {
            game_state_descriptor_check_completion_and_end_round(m, guest, regs);
            true
        }
        0x23f390 => {
            game_mode_resources_preload_from_mode_table_entry(m, guest, regs);
            true
        }
        0x2540b0 => {
            sub_002540b0(m, guest, regs);
            true
        }
        _ => false,
    }
}
