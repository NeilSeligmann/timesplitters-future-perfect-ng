// SPDX-License-Identifier: GPL-3.0-or-later
//! T1902: source-inspected, mechanically translated Tier 0 candidates.
//! Names/system placement INFERRED; no runtime admission. See docs/t-system-video.md.
#![allow(
    clippy::all,
    unused_parens,
    unused_assignments,
    unused_imports,
    unused_variables
)]
use crate::inventory::sse;
use crate::system_slice::{compare_fp, read_double, reciprocal_sqrt, round_pc, sqrt_double};
use crate::{
    abi::{ExtRegisters, GuestCall, Xmm},
    memory::GuestMemory,
    pose::write16,
};

#[path = "system_slice/video_boundary.rs"]
mod boundary;
pub use boundary::*;

/// 0x00028D40: Tier 0, lifted-source inspection; opt-in only.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_macros,
    clippy::all
)]
pub fn f28d40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28d40;
    macro_rules! push {
        ($v:expr) => {{
            let value: u32 = $v;
            esp = esp.wrapping_sub(4);
            m.write32(esp, value);
        }};
    }
    macro_rules! call_to {
        ($va:expr) => {{
            let target: u32 = $va;
            let mut cr = ExtRegisters {
                eax,
                ecx,
                edx,
                esp,
                ebx,
                ebp,
                esi,
                edi,
                eflags: entry.eflags,
                fpu,
                xmm: x,
                xmm_hi: xh,
            };
            guest.call(target, &mut cr);
            xh = cr.xmm_hi;
            (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (
                cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi,
            );
            x = cr.xmm;
        }};
    }
    macro_rules! call {
        ($va:expr) => {{ call_to!($va) }};
    }
    macro_rules! tail_to {
        ($va:expr) => {{
            let slot = esp;
            let saved = m.read32(slot);
            esp = esp.wrapping_add(4);
            call_to!($va);
            esp = esp.wrapping_sub(4);
            m.write32(slot, saved);
        }};
    }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        {
            crate::pose::cover(0x28d40, previous, pc);
            previous = pc;
        }
        match pc {
            0x28d40 => {
                eax = m.read32(esp.wrapping_add(0x4));
                tail_to!(0x28cb0);
                break 'run; // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm: x,
        fpu,
        xmm_hi: xh,
        ..entry
    };
}

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-gameplay/src/cutscene_movie_setup.rs"]
mod movie_setup;

/// Readable original movie setup adapter. Guest memory is confined to the ABI boundary.
/// Shared arithmetic is also used by the Rust-owned presentation model.
pub fn f30210<M: GuestMemory, C: GuestCall>(m: &mut M, _guest: &mut C, regs: &mut ExtRegisters) {
    let width = m.read32(regs.esp.wrapping_add(4)) as i32;
    let height = m.read32(regs.esp.wrapping_add(8)) as i32;
    let mode = m.read32(regs.esp.wrapping_add(12)) as i32;
    let result = movie_setup::configure(
        width,
        height,
        mode,
        f32::from_bits(m.read32(0x476688)),
        f32::from_bits(m.read32(0x475d40)),
    );
    if let Some((width, height)) = result.crop {
        m.write32(0x687e44, width as u32);
        m.write32(0x687e48, height as u32);
    }
    if let Some(mode) = result.mode {
        m.write32(0x4bfc6c, mode as u32);
    }
    regs.eax = mode as u32;
    regs.ecx = result.height_scratch as u32;
    if let Some(scalar) = result.converted_scalar {
        regs.xmm[0].lanes[0] = scalar.to_bits();
    }
}

/// 0x00030210: Tier 0, lifted-source inspection; opt-in only.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_macros,
    clippy::all
)]
pub fn f30210_reference<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30210;
    macro_rules! push {
        ($v:expr) => {{
            let value: u32 = $v;
            esp = esp.wrapping_sub(4);
            m.write32(esp, value);
        }};
    }
    macro_rules! call_to {
        ($va:expr) => {{
            let target: u32 = $va;
            let mut cr = ExtRegisters {
                eax,
                ecx,
                edx,
                esp,
                ebx,
                ebp,
                esi,
                edi,
                eflags: entry.eflags,
                fpu,
                xmm: [xmm0, xmm1, xmm2, xmm3, xmm4],
                xmm_hi: xh,
            };
            guest.call(target, &mut cr);
            xh = cr.xmm_hi;
            (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (
                cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi,
            );
            [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
        }};
    }
    macro_rules! call {
        ($va:expr) => {{ call_to!($va) }};
    }
    macro_rules! tail_to {
        ($va:expr) => {{
            let slot = esp;
            let saved = m.read32(slot);
            esp = esp.wrapping_add(4);
            call_to!($va);
            esp = esp.wrapping_sub(4);
            m.write32(slot, saved);
        }};
    }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        {
            crate::pose::cover(0x30210, previous, pc);
            previous = pc;
        }
        match pc {
            0x30210 => {
                eax = m.read32(esp.wrapping_add(0x4));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                ecx = m.read32(esp.wrapping_add(0x8));
                if (fa != fb) {
                    pc = 0x30233;
                    continue;
                }
                pc = 0x3021d;
                continue;
            }
            0x3021d => {
                fa = ecx;
                fb = (((0i32) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) {
                    pc = 0x30233;
                    continue;
                }
                pc = 0x30221;
                continue;
            }
            0x30221 => {
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(
                    f32::from_bits(xmm0.lanes[0]),
                    f32::from_bits(m.read32(0x476688)),
                )
                .to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                pc = 0x30256;
                continue;
            }
            0x30233 => {
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) {
                    pc = 0x3024e;
                    continue;
                }
                pc = 0x30238;
                continue;
            }
            0x30238 => {
                fa = eax;
                fb = (((0i32) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) {
                    pc = 0x30261;
                    continue;
                }
                pc = 0x3023c;
                continue;
            }
            0x3023c => {
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(
                    f32::from_bits(xmm0.lanes[0]),
                    f32::from_bits(m.read32(0x475d40)),
                )
                .to_bits();
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                pc = 0x30256;
                continue;
            }
            0x3024e => {
                fa = eax;
                fb = (((0i32) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) {
                    pc = 0x30261;
                    continue;
                }
                pc = 0x30252;
                continue;
            }
            0x30252 => {
                fa = ecx;
                fb = (((0i32) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) {
                    pc = 0x30261;
                    continue;
                }
                pc = 0x30256;
                continue;
            }
            0x30256 => {
                m.write32(0x687e44, eax);
                m.write32(0x687e48, ecx);
                pc = 0x30261;
                continue;
            }
            0x30261 => {
                eax = m.read32(esp.wrapping_add(0xc));
                fa = eax;
                fb = (((0i32) as u32) & 0xffffffffu32);
                if (fa != fb) {
                    pc = 0x3026f;
                    continue;
                }
                pc = 0x30269;
                continue;
            }
            0x30269 => {
                m.write32(0x4bfc6c, eax);
                break 'run;
            }
            0x3026f => {
                fa = eax;
                fb = (((0x1i32) as u32) & 0xffffffffu32);
                if (fa != fb) {
                    pc = 0x30279;
                    continue;
                }
                pc = 0x30274;
                continue;
            }
            0x30274 => {
                m.write32(0x4bfc6c, eax);
                pc = 0x30279;
                continue;
            }
            0x30279 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm: [xmm0, xmm1, xmm2, xmm3, xmm4],
        fpu,
        xmm_hi: xh,
        ..entry
    };
}

/// 0x00030280: Tier 0, lifted-source inspection; opt-in only.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_macros,
    clippy::all
)]
pub fn f30280<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30280;
    macro_rules! push {
        ($v:expr) => {{
            let value: u32 = $v;
            esp = esp.wrapping_sub(4);
            m.write32(esp, value);
        }};
    }
    macro_rules! call_to {
        ($va:expr) => {{
            let target: u32 = $va;
            let mut cr = ExtRegisters {
                eax,
                ecx,
                edx,
                esp,
                ebx,
                ebp,
                esi,
                edi,
                eflags: entry.eflags,
                fpu,
                xmm: [xmm0, xmm1, xmm2, xmm3, xmm4],
                xmm_hi: xh,
            };
            guest.call(target, &mut cr);
            xh = cr.xmm_hi;
            (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (
                cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi,
            );
            fpu = cr.fpu;
            [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
        }};
    }
    macro_rules! call {
        ($va:expr) => {{ call_to!($va) }};
    }
    macro_rules! tail_to {
        ($va:expr) => {{
            let slot = esp;
            let saved = m.read32(slot);
            esp = esp.wrapping_add(4);
            call_to!($va);
            esp = esp.wrapping_sub(4);
            m.write32(slot, saved);
        }};
    }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        {
            crate::pose::cover(0x30280, previous, pc);
            previous = pc;
        }
        match pc {
            0x30280 => {
                esp = esp.wrapping_sub(0x274);
                push!(ebp);
                push!(esi);
                push!(edi);
                m.write32(esp.wrapping_add(0xc), 0);
                esi = 0x5;
                call!(0x214c0);
                pc = 0x3029b;
                continue;
            }
            0x3029b => {
                eax = m.read32(esp.wrapping_add(0x284));
                push!(eax);
                push!(0x476698);
                ecx = esp.wrapping_add(0x80);
                push!(0x47668c);
                push!(ecx);
                call!(0x3c85d4);
                pc = 0x302ba;
                continue;
            }
            0x302ba => {
                edx = esp.wrapping_add(0x88);
                push!(edx);
                call!(0x28d40);
                pc = 0x302c7;
                continue;
            }
            0x302c7 => {
                esp = esp.wrapping_add(0x14);
                edx = esp.wrapping_add(0x17c);
                pc = 0x302d1;
                continue;
            }
            0x302d1 => {
                ecx = (ecx & 0xffff_ff00) | (((m.read8(eax)) as u32) & 0xff);
                eax = eax.wrapping_add(1);
                m.write8(edx, (((ecx) as u8) as u32) as u8);
                edx = edx.wrapping_add(1);
                fa = ((((ecx) as u8) as u32) & 0xffu32);
                fb = (((0i32) as u32) & 0xffu32);
                if (fa != fb) {
                    pc = 0x302d1;
                    continue;
                }
                pc = 0x302db;
                continue;
            }
            0x302db => {
                pc = 0x302e0;
                continue;
            }
            0x302e0 => {
                eax = esp.wrapping_add(0xc);
                push!(eax);
                ecx = esp.wrapping_add(0x180);
                push!(ecx);
                push!(0);
                call_to!(0x00444A2D);
                pc = 0x302f4;
                continue;
            }
            0x302f4 => {
                fa = eax;
                fb = (((0i32) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) {
                    pc = 0x30307;
                    continue;
                }
                pc = 0x302f8;
                continue;
            }
            0x302f8 => {
                push!(0x32);
                call!(0x3800d1);
                pc = 0x302ff;
                continue;
            }
            0x302ff => {
                esi = esi.wrapping_sub(1);
                fa = esi;
                if (fa != 0) {
                    pc = 0x302e0;
                    continue;
                }
                pc = 0x30302;
                continue;
            }
            0x30302 => {
                call!(0x235e0);
                pc = 0x30307;
                continue;
            }
            0x30307 => {
                eax = m.read32(esp.wrapping_add(0xc));
                edx = esp.wrapping_add(0x14);
                push!(edx);
                push!(0);
                push!(0);
                push!(0);
                push!(eax);
                call_to!(0x004450C2);
                pc = 0x3031c;
                continue;
            }
            0x3031c => {
                ecx = m.read32(esp.wrapping_add(0xc));
                push!(0);
                push!(ecx);
                call_to!(0x00445241);
                pc = 0x30328;
                continue;
            }
            0x30328 => {
                eax = m.read32(esp.wrapping_add(0x14));
                fa = eax;
                fb = (((0i32) as u32) & 0xffffffffu32);
                if (fa == fb) {
                    pc = 0x30364;
                    continue;
                }
                pc = 0x30330;
                continue;
            }
            0x30330 => {
                xmm0.lanes[0] = (((m.read32(0x5236bc)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(
                    f32::from_bits(xmm0.lanes[0]),
                    f32::from_bits(m.read32(0x475dc4)),
                )
                .to_bits();
                push!(0);
                push!(0x3f800000);
                push!(0);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                push!(0xffffffffu32);
                call!(0x28180);
                pc = 0x30356;
                continue;
            }
            0x30356 => {
                edx = m.read32(esp.wrapping_add(0x28));
                esp = esp.wrapping_add(0x14);
                push!(eax);
                push!(edx);
                call_to!(0x00407B14);
                pc = 0x30364;
                continue;
            }
            0x30364 => {
                ecx = m.read32(esp.wrapping_add(0xc));
                eax = esp.wrapping_add(0x20);
                push!(eax);
                push!(ecx);
                call_to!(0x00445055);
                pc = 0x30373;
                continue;
            }
            0x30373 => {
                esi = m.read32(esp.wrapping_add(0x24));
                esi = ((esi as i32).wrapping_mul((m.read32(esp.wrapping_add(0x20))) as i32)) as u32;
                esi = esi.wrapping_mul(0x4).wrapping_add(0x7f);
                esi = (esi & 0xffffff80u32);
                push!(0);
                esi = esi.wrapping_shl(((((0x1i32) as u32) & 0x1fu32) as u32 & 31));
                push!(esi);
                push!(0x80);
                call!(0x3e0b0);
                pc = 0x30395;
                continue;
            }
            0x30395 => {
                ebp = eax;
                ecx = esi;
                edx = ecx;
                ecx = ecx.wrapping_shr(((((0x2i32) as u32) & 0x1fu32) as u32 & 31));
                eax = 0;
                edi = ebp;
                {
                    let step: u32 = if entry.eflags & 0x400 != 0 {
                        4u32.wrapping_neg()
                    } else {
                        4
                    };
                    for i in 0..ecx {
                        m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax);
                    }
                    edi = edi.wrapping_add(ecx.wrapping_mul(step));
                } // rep stosd, DF from entry.eflags
                ecx = 0;
                esp = esp.wrapping_add(0xc);
                ecx = edx;
                ecx = (ecx & 0x3);
                {
                    let step = if entry.eflags & 0x400 == 0 {
                        1u32
                    } else {
                        u32::MAX
                    };
                    for i in 0..ecx {
                        m.write8(edi.wrapping_add(i.wrapping_mul(step)), eax as u8);
                    }
                    edi = edi.wrapping_add(ecx.wrapping_mul(step));
                }
                ecx = 0;
                eax = m.read32(esp.wrapping_add(0x20));
                ecx = eax.wrapping_mul(0x4);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x28));
                push!(0);
                edx = esp.wrapping_add(0x6c);
                push!(edx);
                push!(0);
                push!(0x24);
                push!(0);
                push!(0x1);
                push!(ecx);
                push!(eax);
                call_to!(0x003E6684);
                pc = 0x303d4;
                continue;
            }
            0x303d4 => {
                push!(ebp);
                edx = esp.wrapping_add(0x68);
                push!(edx);
                call_to!(0x003D4D70);
                pc = 0x303df;
                continue;
            }
            0x303df => {
                ecx = m.read32(esp.wrapping_add(0x24));
                eax = m.read32(esp.wrapping_add(0x20));
                edx = ecx;
                edx = ((edx as i32).wrapping_mul((eax) as i32)) as u32;
                esi = edx.wrapping_mul(0x4).wrapping_add(0x7f);
                edx = eax.wrapping_mul(0x4);
                push!(edx);
                push!(0);
                edx = esp.wrapping_add(0x58);
                push!(edx);
                push!(0);
                push!(0x24);
                push!(0);
                push!(0x1);
                push!(ecx);
                esi = (esi & 0xffffff80u32);
                push!(eax);
                esi = esi.wrapping_add(ebp);
                call_to!(0x003E6684);
                pc = 0x30416;
                continue;
            }
            0x30416 => {
                push!(esi);
                eax = esp.wrapping_add(0x54);
                push!(eax);
                call_to!(0x003D4D70);
                pc = 0x30421;
                continue;
            }
            0x30421 => {
                edi = 0;
                push!(edi);
                ecx = esp.wrapping_add(0x68);
                push!(ecx);
                call_to!(0x003D4E10);
                pc = 0x3042e;
                continue;
            }
            0x3042e => {
                push!(edi);
                edx = esp.wrapping_add(0x54);
                push!(edx);
                m.write32(esp.wrapping_add(0x24), eax);
                call_to!(0x003D4E10);
                pc = 0x3043d;
                continue;
            }
            0x3043d => {
                push!(edi);
                push!(0x3f800000);
                push!(edi);
                esi = eax;
                push!(0xf0);
                push!(edi);
                push!(edi);
                call_to!(0x003D5EB0);
                pc = 0x30452;
                continue;
            }
            0x30452 => {
                push!(edi);
                call_to!(0x003D8E50);
                pc = 0x30458;
                continue;
            }
            0x30458 => {
                push!(edi);
                push!(0x3f800000);
                push!(edi);
                push!(0xf0);
                push!(edi);
                push!(edi);
                call_to!(0x003D5EB0);
                pc = 0x3046b;
                continue;
            }
            0x3046b => {
                push!(0x1);
                call_to!(0x003D9990);
                pc = 0x30472;
                continue;
            }
            0x30472 => {
                push!(edi);
                call_to!(0x003D8E50);
                pc = 0x30478;
                continue;
            }
            0x30478 => {
                ecx = m.read32(0x687e44);
                edx = m.read32(esp.wrapping_add(0x20));
                eax = m.read32(0x687e48);
                edx = edx.wrapping_sub(ecx);
                m.write32(esp.wrapping_add(0x40), ecx);
                ecx = m.read32(esp.wrapping_add(0x24));
                ecx = ecx.wrapping_sub(eax);
                m.write32(esp.wrapping_add(0x44), eax);
                eax = m.read32(0x4bfc6c);
                fa = eax;
                fb = (((0i32) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x48), edx);
                edx = m.read32(0x4e8cf4);
                m.write32(esp.wrapping_add(0x4c), ecx);
                m.write32(esp.wrapping_add(0x3c), edx);
                m.write32(esp.wrapping_add(0x34), edi);
                if (fa == fb) {
                    pc = 0x30512;
                    continue;
                }
                pc = 0x304b6;
                continue;
            }
            0x304b6 => {
                eax = m.read32(0x4c0458);
                fpu.push((m.read32(0x4c0458) as i32 as f64));
                fa = eax;
                fb = (((0i32) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) {
                    pc = 0x304cb;
                    continue;
                }
                pc = 0x304c5;
                continue;
            }
            0x304c5 => {
                {
                    let value = round_pc(
                        fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64),
                        fpu.control,
                    );
                    fpu.stack[((fpu.top + 0) & 7) as usize] = value;
                }
                pc = 0x304cb;
                continue;
            }
            0x304cb => {
                ecx = m.read32(0x4c045c);
                fpu.push((m.read32(0x4c045c) as i32 as f64));
                fa = ecx;
                fb = (((0i32) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) {
                    pc = 0x304e1;
                    continue;
                }
                pc = 0x304db;
                continue;
            }
            0x304db => {
                {
                    let value = round_pc(
                        fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64),
                        fpu.control,
                    );
                    fpu.stack[((fpu.top + 0) & 7) as usize] = value;
                }
                pc = 0x304e1;
                continue;
            }
            0x304e1 => {
                {
                    let value = round_pc(fpu.st(1) / fpu.st(0), fpu.control);
                    fpu.stack[((fpu.top + 1) & 7) as usize] = value;
                }
                fpu.pop();
                {
                    let value = round_pc(
                        (f32::from_bits(m.read32(0x476688)) as f64) / fpu.st(0),
                        fpu.control,
                    );
                    fpu.stack[((fpu.top + 0) & 7) as usize] = value;
                }
                {
                    let value =
                        round_pc(fpu.st(0) * (m.read32(0x4e8cf0) as i32 as f64), fpu.control);
                    fpu.stack[((fpu.top + 0) & 7) as usize] = value;
                }
                {
                    let value = round_pc(
                        fpu.st(0) * (f32::from_bits(m.read32(0x475cd4)) as f64),
                        fpu.control,
                    );
                    fpu.stack[((fpu.top + 0) & 7) as usize] = value;
                }
                call!(0x3c848c);
                pc = 0x304fa;
                continue;
            }
            0x304fa => {
                ecx = m.read32(0x4e8cf0);
                ecx = (((ecx as i32).wrapping_shr(((((0x1i32) as u32) & 0x1fu32) as u32 & 31)))
                    as u32);
                edx = ecx;
                edx = edx.wrapping_sub(eax);
                ecx = ecx.wrapping_add(eax);
                m.write32(esp.wrapping_add(0x30), edx);
                m.write32(esp.wrapping_add(0x38), ecx);
                pc = 0x30523;
                continue;
            }
            0x30512 => {
                eax = m.read32(0x4e8cf0);
                m.write32(esp.wrapping_add(0x30), 0);
                m.write32(esp.wrapping_add(0x38), eax);
                pc = 0x30523;
                continue;
            }
            0x30523 => {
                call!(0x438b0);
                pc = 0x30528;
                continue;
            }
            0x30528 => {
                call!(0x42fd0);
                pc = 0x3052d;
                continue;
            }
            0x3052d => {
                call!(0x428d0);
                pc = 0x30532;
                continue;
            }
            0x30532 => {
                call!(0x45a60);
                pc = 0x30537;
                continue;
            }
            0x30537 => {
                m.write32(esp.wrapping_add(0x18), eax);
                eax = 0;
                m.write32(esp.wrapping_add(0x10), eax);
                push!(ebx);
                pc = 0x30542;
                continue;
            }
            0x30542 => {
                fa = eax;
                fb = (((0x3i32) as u32) & 0xffffffffu32);
                if (fa == fb) {
                    pc = 0x30604;
                    continue;
                }
                pc = 0x3054b;
                continue;
            }
            0x3054b => {
                eax = m.read32(esp.wrapping_add(0x28c));
                fa = eax;
                fb = (((0i32) as u32) & 0xffffffffu32);
                if (fa == fb) {
                    pc = 0x30599;
                    continue;
                }
                pc = 0x30556;
                continue;
            }
            0x30556 => {
                push!(0x3);
                call!(0x42c30);
                pc = 0x3055d;
                continue;
            }
            0x3055d => {
                push!(0x2);
                ebx = eax;
                call!(0x42c30);
                pc = 0x30566;
                continue;
            }
            0x30566 => {
                push!(0x1);
                ebx = (ebx | eax);
                call!(0x42c30);
                pc = 0x3056f;
                continue;
            }
            0x3056f => {
                push!(0);
                ebx = (ebx | eax);
                call!(0x42c30);
                pc = 0x30578;
                continue;
            }
            0x30578 => {
                ebx = (ebx | eax);
                esp = esp.wrapping_add(0x10);
                fa = ebx;
                fb = (((0x280i32) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) {
                    pc = 0x30599;
                    continue;
                }
                pc = 0x30585;
                continue;
            }
            0x30585 => {
                call!(0x428d0);
                pc = 0x3058a;
                continue;
            }
            0x3058a => {
                call!(0x73c80);
                pc = 0x3058f;
                continue;
            }
            0x3058f => {
                ecx = m.read32(esp.wrapping_add(0x10));
                push!(ecx);
                call_to!(0x00445252);
                pc = 0x30599;
                continue;
            }
            0x30599 => {
                call!(0x438b0);
                pc = 0x3059e;
                continue;
            }
            0x3059e => {
                call_to!(0x00407B40);
                pc = 0x305a3;
                continue;
            }
            0x305a3 => {
                fa = edi;
                fb = (((0i32) as u32) & 0xffffffffu32);
                if (fa == fb) {
                    pc = 0x305ba;
                    continue;
                }
                pc = 0x305a7;
                continue;
            }
            0x305a7 => {
                call_to!(0x003D97F0);
                pc = 0x305ac;
                continue;
            }
            0x305ac => {
                fa = eax;
                fb = (((0i32) as u32) & 0xffffffffu32);
                if (fa == fb) {
                    pc = 0x305ba;
                    continue;
                }
                pc = 0x305b0;
                continue;
            }
            0x305b0 => {
                esi = m.read32(esp.wrapping_add(0x20));
                m.write32(esp.wrapping_add(0x20), edi);
                edi = 0;
                pc = 0x305ba;
                continue;
            }
            0x305ba => {
                eax = m.read32(esp.wrapping_add(0x10));
                push!(0);
                edx = esp.wrapping_add(0x18);
                push!(edx);
                push!(esi);
                push!(eax);
                call_to!(0x0044525D);
                pc = 0x305cc;
                continue;
            }
            0x305cc => {
                fa = m.read32(esp.wrapping_add(0x14));
                fb = (((0x1i32) as u32) & 0xffffffffu32);
                if (fa != fb) {
                    pc = 0x305eb;
                    continue;
                }
                pc = 0x305d3;
                continue;
            }
            0x305d3 => {
                push!(0);
                push!(0);
                ecx = esp.wrapping_add(0x3c);
                push!(ecx);
                edx = esp.wrapping_add(0x50);
                push!(edx);
                push!(esi);
                call_to!(0x003D9810);
                pc = 0x305e7;
                continue;
            }
            0x305e7 => {
                edi = esi;
                esi = 0;
                pc = 0x305eb;
                continue;
            }
            0x305eb => {
                call!(0x42fd0);
                pc = 0x305f0;
                continue;
            }
            0x305f0 => {
                push!(0);
                call_to!(0x003D8E50);
                pc = 0x305f7;
                continue;
            }
            0x305f7 => {
                eax = m.read32(esp.wrapping_add(0x14));
                fa = eax;
                fb = (((0x2i32) as u32) & 0xffffffffu32);
                if (fa != fb) {
                    pc = 0x30542;
                    continue;
                }
                pc = 0x30604;
                continue;
            }
            0x30604 => {
                push!(0);
                call_to!(0x003D9990);
                pc = 0x3060b;
                continue;
            }
            0x3060b => {
                push!(0);
                call_to!(0x003D8E50);
                pc = 0x30612;
                continue;
            }
            0x30612 => {
                eax = m.read32(esp.wrapping_add(0x18));
                fa = eax;
                fb = (((0i32) as u32) & 0xffffffffu32);
                ebx = m.read32(esp);
                esp = esp.wrapping_add(4);
                if (fa == fb) {
                    pc = 0x3062b;
                    continue;
                }
                pc = 0x3061b;
                continue;
            }
            0x3061b => {
                ecx = m.read32(eax);
                push!(eax);
                call_to!(m.read32(ecx.wrapping_add(0x18)));
                pc = 0x30621;
                continue;
            }
            0x30621 => {
                eax = m.read32(esp.wrapping_add(0x14));
                edx = m.read32(eax);
                push!(eax);
                call_to!(m.read32(edx.wrapping_add(0x4)));
                pc = 0x3062b;
                continue;
            }
            0x3062b => {
                eax = m.read32(esp.wrapping_add(0xc));
                push!(eax);
                call_to!(0x00444F71);
                pc = 0x30635;
                continue;
            }
            0x30635 => {
                ecx = esp.wrapping_add(0x50);
                push!(ecx);
                call_to!(0x003D4C90);
                pc = 0x3063f;
                continue;
            }
            0x3063f => {
                edx = esp.wrapping_add(0x64);
                push!(edx);
                call_to!(0x003D4C90);
                pc = 0x30649;
                continue;
            }
            0x30649 => {
                push!(ebp);
                call!(0x3e1e0);
                pc = 0x3064f;
                continue;
            }
            0x3064f => {
                eax = m.read32(esp.wrapping_add(0x14));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = (((0x3i32) as u32) & 0xffffffffu32);
                edi = m.read32(esp);
                esp = esp.wrapping_add(4);
                esi = m.read32(esp);
                esp = esp.wrapping_add(4);
                ebp = m.read32(esp);
                esp = esp.wrapping_add(4);
                if (fa != fb) {
                    pc = 0x30663;
                    continue;
                }
                pc = 0x3065e;
                continue;
            }
            0x3065e => {
                call!(0x235e0);
                pc = 0x30663;
                continue;
            }
            0x30663 => {
                eax = m.read32(esp.wrapping_add(0xc));
                push!(0x1);
                push!(eax);
                call!(0x45a00);
                pc = 0x3066f;
                continue;
            }
            0x3066f => {
                esp = esp.wrapping_add(0x27c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm: [xmm0, xmm1, xmm2, xmm3, xmm4],
        fpu,
        xmm_hi: xh,
        ..entry
    };
}

/// Explicit per-function A/B boundary: disabled or unknown roots return false without mutation.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    enabled: bool,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !enabled {
        return false;
    }
    match va {
        0x28d40 => f28d40(m, guest, regs),
        0x30210 => f30210(m, guest, regs),
        0x30280 => f30280(m, guest, regs),
        _ => return false,
    }
    true
}
/// INFERRED readable name from function_names.csv.
pub use f28d40 as xmv_movie_play_tail_helper;
/// INFERRED readable name from function_names.csv.
pub use f30210 as xmv_movie_play_state_setup;
/// INFERRED readable name from function_names.csv.
pub use f30280 as xmv_movie_play;
