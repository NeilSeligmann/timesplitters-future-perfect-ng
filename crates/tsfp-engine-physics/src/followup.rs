// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-engine-physics/leaf
// portreg: mirrors=tsfp-engine-physics/fp_scalar_d
//! T1934 mechanically translated opt-in candidates. See docs/t1934-physics-followup.md.
#![allow(clippy::all, unused_parens, unused_assignments, unused_imports, unused_variables, unexpected_cfgs)] #![cfg_attr(rustfmt, rustfmt::skip)]
use super::boundary::{ExtRegisters, GuestCall, Xmm, write16, sse};
use super::arithmetic::*; // round_pc, read_double, sqrt_double, reciprocal_sqrt, compare_fp (no_std: Atan2)
use crate::memory::GuestMemory;

// portreg: va=0x000471a0
/// 0x000471a0: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f471a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut fp_cmp = 0i32;
    let mut pc: u32 = 0x471a0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x471a0 => {
                esp = esp.wrapping_sub(0x14);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64));
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x28))) as f64));
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x28))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc((fpu.st(1) + fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 1) & 7) as usize] = value; } fpu.pop();
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp, (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(0x475cac)) as f64));
                fpu.push((f32::from_bits(m.read32(esp)) as f64));
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fp_cmp == 0i32) || (fp_cmp == 0x2i32)))) { 0x40u32 } else { 0x0u32 }) | (if (((fp_cmp == 0x2i32))) { 0x4u32 } else { 0x0u32 })) | (if ((((fp_cmp < 0i32) || (fp_cmp == 0x2i32)))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (((((fa) & (fb))) as u8).count_ones() % 2 == 0) { pc = 0x471d7; continue; }
                pc = 0x471cd; continue;
            }
            0x471cd => {
                fpu.push((f32::from_bits(m.read32(0x475c78)) as f64));
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x471d7 => {
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp))).to_bits();
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1 = xh[0];
                xmm2 = xmm4;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::default();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x10), xmm4.lanes[0]);
                m.write32(esp.wrapping_add(0xc), xh[0].lanes[0]);
                m.write32(esp.wrapping_add(0x8), xmm1.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x473cb; continue; }
                pc = 0x4723a; continue;
            }
            0x4723a => {
                xmm0 = xh[0];
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                xmm1 = xmm4;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x28)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if ((fca >= fcb)) { pc = 0x471cd; continue; }
                pc = 0x47259; continue;
            }
            0x47259 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xmm0 = xmm3;
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = xmm2;
                xh[2] = xmm0;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = Xmm::default();
                fca = f32::from_bits(xh[2].lanes[0]) as f64; fcb = f32::from_bits(xh[1].lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x471cd; continue; }
                pc = 0x47291; continue;
            }
            0x47291 => {
                xh[1] = xmm0;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = xmm2;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                fca = f32::from_bits(xh[2].lanes[0]) as f64; fcb = f32::from_bits(xh[1].lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x472bb; continue; }
                pc = 0x472a8; continue;
            }
            0x472a8 => {
                ecx = m.read32(esp.wrapping_add(0x30));
                edx = m.read32(esp.wrapping_add(0x2c));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                pc = 0x473d3; continue;
            }
            0x472bb => {
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x8))) as f64);
                eax = m.read32(esp.wrapping_add(0x3c));
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x472d9; continue; }
                pc = 0x472c6; continue;
            }
            0x472c6 => {
                ecx = m.read32(esp.wrapping_add(0x38));
                edx = m.read32(esp.wrapping_add(0x34));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                pc = 0x473d7; continue;
            }
            0x472d9 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x8))) as f64));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(((f32::from_bits(m.read32(0x475c78)) as f64) / fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0xc))) as f64));
                { let value = round_pc((fpu.st(0) * fpu.st(1)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0xc), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.stack[fpu.top as usize] = -fpu.st(0);
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                if (fa == fb) { pc = 0x47312; continue; }
                pc = 0x47309; continue;
            }
            0x47309 => {
                m.write32(eax, xmm4.lanes[0]);
                m.write32(eax.wrapping_add(0x4), xh[0].lanes[0]);
                pc = 0x47312; continue;
            }
            0x47312 => {
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x28)))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (!(((((fa) & (fb))) as u8).count_ones() % 2 == 0)) { pc = 0x471cd; continue; }
                pc = 0x4734f; continue;
            }
            0x4734f => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x47398; continue; }
                pc = 0x47354; continue;
            }
            0x47354 => {
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if ((fca >= fcb)) { pc = 0x471cd; continue; }
                pc = 0x47363; continue;
            }
            0x47363 => {
                xmm2 = xh[2];
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if ((fca >= fcb)) { pc = 0x471cd; continue; }
                pc = 0x47373; continue;
            }
            0x47373 => {
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x4738e; continue; }
                pc = 0x47378; continue;
            }
            0x47378 => {
                xmm1 = xh[2];
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm1.lanes[0]);
                pc = 0x4741b; continue;
            }
            0x4738e => {
                fpu.push((f32::from_bits(m.read32(0x475cac)) as f64));
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x47398 => {
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if ((fca >= fcb)) { pc = 0x471cd; continue; }
                pc = 0x473a7; continue;
            }
            0x473a7 => {
                xmm1 = xh[2];
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca >= fcb)) { pc = 0x471cd; continue; }
                pc = 0x473b7; continue;
            }
            0x473b7 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x4738e; continue; }
                pc = 0x473bc; continue;
            }
            0x473bc => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1 = xmm0;
                m.write32(esp.wrapping_add(0x28), xmm1.lanes[0]);
                pc = 0x4741b; continue;
            }
            0x473cb => {
                ecx = m.read32(esp.wrapping_add(0x38));
                edx = m.read32(esp.wrapping_add(0x34));
                pc = 0x473d3; continue;
            }
            0x473d3 => {
                eax = m.read32(esp.wrapping_add(0x3c));
                pc = 0x473d7; continue;
            }
            0x473d7 => {
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x24));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x24));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x24));
                esp = esp.wrapping_sub(0x8);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x38)))).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x47030);
                pc = 0x4740b; continue;
            }
            0x4740b => {
                m.write32(esp.wrapping_add(0x48), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                xh[2] = Xmm::default();
                esp = esp.wrapping_add(0x20);
                pc = 0x4741b; continue;
            }
            0x4741b => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp)) as f64);
                if ((fca >= fcb)) { pc = 0x471cd; continue; }
                pc = 0x47425; continue;
            }
            0x47425 => {
                fca = f32::from_bits(xh[2].lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if ((fca >= fcb)) { pc = 0x4738e; continue; }
                pc = 0x4742e; continue;
            }
            0x4742e => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x28))) as f64));
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x4))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

// portreg: va=0x00047440
/// 0x00047440: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f47440<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut fp_cmp = 0i32;
    let mut pc: u32 = 0x47440;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x47440 => {
                esp = esp.wrapping_sub(0x8);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x18))) as f64));
                eax = m.read32(esp.wrapping_add(0x2c));
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x18))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                ecx = m.read32(esp.wrapping_add(0x24));
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x1c))) as f64));
                edx = m.read32(esp.wrapping_add(0x20));
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x1c))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x14));
                { let value = round_pc((fpu.st(1) + fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 1) & 7) as usize] = value; } fpu.pop();
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x14));
                push!(edx);
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                esp = esp.wrapping_sub(0xc);
                m.write32(esp.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x34)))).to_bits();
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                m.write32(esp.wrapping_add(0x8), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                push!(ecx);
                call!(0x47030);
                pc = 0x474bc; continue;
            }
            0x474bc => {
                m.write32(esp.wrapping_add(0x3c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64));
                esp = esp.wrapping_add(0x20);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x1c))) as f64));
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if (((fp_cmp < 0i32) || (fp_cmp == 0x2i32))) { pc = 0x474db; continue; }
                pc = 0x474d1; continue;
            }
            0x474d1 => {
                fpu.push((f32::from_bits(m.read32(0x475c78)) as f64));
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x474db => {
                xmm0 = Xmm::default();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x1c))) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x474ef; continue; }
                pc = 0x474e5; continue;
            }
            0x474e5 => {
                fpu.push((f32::from_bits(m.read32(0x475cac)) as f64));
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x474ef => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x1c))) as f64));
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x4))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000494e0: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f494e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut fp_cmp = 0i32;
    let mut pc: u32 = 0x494e0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x494e0 => {
                esp = esp.wrapping_sub(0x28);
                eax = m.read32(esp.wrapping_add(0x2c));
                ecx = m.read32(esp.wrapping_add(0x34));
                edx = m.read32(esp.wrapping_add(0x30));
                xmm0 = Xmm::scalar(m.read32(eax));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                xh[0] = Xmm::scalar(m.read32(edx.wrapping_add(0x4)));
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_add(0x8)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(edx));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                edx = m.read32(esp.wrapping_add(0x38));
                xmm3 = Xmm::scalar(m.read32(edx.wrapping_add(0x4)));
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                xh[1] = Xmm::scalar(m.read32(edx));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x4), xh[1].lanes[0]);
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm4 = xmm1;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xh[2] = xmm2;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                xh[2] = xmm0;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0 = xmm3;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::default();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x8), xmm3.lanes[0]);
                xh[1] = xmm0;
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                m.write32(esp.wrapping_add(0x2c), xh[1].lanes[0]);
                xh[1] = xmm3;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x20), xh[0].lanes[0]);
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = xmm3;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                m.write32(esp.wrapping_add(0x24), xmm1.lanes[0]);
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = xmm2;
                m.write32(esp.wrapping_add(0x1c), xmm2.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = xmm3;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm2;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                xmm2 = xh[2];
                xh[0] = xmm3;
                m.write32(esp, xh[1].lanes[0]);
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fpu.push(read_double(m, 0x478608));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x34), xmm2.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x34));
                edx = (edx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x30), edx);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x30))) as f64));
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if (((fp_cmp < 0i32) || (fp_cmp == 0x2i32))) { pc = 0x49a0b; continue; }
                pc = 0x49672; continue;
            }
            0x49672 => {
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x8)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm2;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                xh[0] = xmm1;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x34), xmm3.lanes[0]);
                xmm3 = xmm1;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                xh[0] = xmm2;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x38), xmm3.lanes[0]);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x498ff; continue; }
                pc = 0x496e8; continue;
            }
            0x496e8 => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x34))) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x497d8; continue; }
                pc = 0x496f9; continue;
            }
            0x496f9 => {
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x497aa; continue; }
                pc = 0x49702; continue;
            }
            0x49702 => {
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x49778; continue; }
                pc = 0x49707; continue;
            }
            0x49707 => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = xmm0;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x34)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = Xmm::scalar(m.read32(0x475d10));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xmm3;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = xh[0];
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp))).to_bits();
                pc = 0x49bc2; continue;
            }
            0x49778 => {
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                xmm3 = Xmm::scalar(m.read32(0x475c78));
                xh[0] = xmm0;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x49940; continue; }
                pc = 0x49792; continue;
            }
            0x49792 => {
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0 = xmm2;
                pc = 0x49bc2; continue;
            }
            0x497aa => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                xmm3 = xmm0;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x499ab; continue; }
                pc = 0x497b6; continue;
            }
            0x497b6 => {
                xh[0] = xmm0;
                xmm0 = xh[1];
                pc = 0x49bc2; continue;
            }
            0x497c1 => {
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::divss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xh[0] = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                pc = 0x49bbe; continue;
            }
            0x497d8 => {
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x498ae; continue; }
                pc = 0x497e1; continue;
            }
            0x497e1 => {
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                xmm3 = xmm0;
                if ((fca >= fcb)) { pc = 0x49852; continue; }
                pc = 0x497e9; continue;
            }
            0x497e9 => {
                xh[0] = xmm1;
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x34), xh[0].lanes[0]);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x4984f; continue; }
                pc = 0x49801; continue;
            }
            0x49801 => {
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                xmm3 = Xmm::scalar(m.read32(0x475c78));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x49829; continue; }
                pc = 0x49816; continue;
            }
            0x49816 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[0] = xmm0;
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0 = xmm2;
                pc = 0x49bc2; continue;
            }
            0x49829 => {
                xh[0].lanes[0] = sse::divss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = xmm0;
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                pc = 0x49bc2; continue;
            }
            0x4984f => {
                xmm3 = xmm0;
                pc = 0x49852; continue;
            }
            0x49852 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                xh[0] = Xmm::scalar(m.read32(0x475c78));
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x4987d; continue; }
                pc = 0x49865; continue;
            }
            0x49865 => {
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0 = xmm1;
                pc = 0x49bc2; continue;
            }
            0x4987d => {
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xh[2].lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x498d1; continue; }
                pc = 0x49886; continue;
            }
            0x49886 => {
                xmm3 = Xmm::scalar(m.read32(0x475c78));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0 = xmm2;
                pc = 0x49bc2; continue;
            }
            0x498ae => {
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x4984f; continue; }
                pc = 0x498ba; continue;
            }
            0x498ba => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                xmm3 = xmm0;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x497c1; continue; }
                pc = 0x498c6; continue;
            }
            0x498c6 => {
                xh[0] = xmm0;
                xmm0 = xh[1];
                pc = 0x49bc2; continue;
            }
            0x498d1 => {
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                xmm3 = xmm2;
                xmm3.lanes[0] = sse::divss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0 = xmm2;
                pc = 0x49bc2; continue;
            }
            0x498ff => {
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x4999f; continue; }
                pc = 0x49908; continue;
            }
            0x49908 => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x4992a; continue; }
                pc = 0x49913; continue;
            }
            0x49913 => {
                xh[0] = xmm0;
                pc = 0x49916; continue;
            }
            0x49916 => {
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x499c8; continue; }
                pc = 0x4991f; continue;
            }
            0x4991f => {
                xmm3 = xmm0;
                xmm0 = xh[1];
                pc = 0x49bc2; continue;
            }
            0x4992a => {
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                xh[0] = xmm0;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x49916; continue; }
                pc = 0x49938; continue;
            }
            0x49938 => {
                xmm3 = Xmm::scalar(m.read32(0x475c78));
                pc = 0x49940; continue;
            }
            0x49940 => {
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm4.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x49971; continue; }
                pc = 0x49949; continue;
            }
            0x49949 => {
                xh[0] = Xmm::scalar(m.read32(0x475c78));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0 = xmm1;
                pc = 0x49bc2; continue;
            }
            0x49971 => {
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                xh[0] = xmm1;
                xh[0].lanes[0] = sse::divss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0 = xmm1;
                pc = 0x49bc2; continue;
            }
            0x4999f => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x49913; continue; }
                pc = 0x499a8; continue;
            }
            0x499a8 => {
                xmm3 = xmm0;
                pc = 0x499ab; continue;
            }
            0x499ab => {
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm4.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x497c1; continue; }
                pc = 0x499bb; continue;
            }
            0x499bb => {
                xh[0] = Xmm::scalar(m.read32(0x475c78));
                pc = 0x49865; continue;
            }
            0x499c8 => {
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xh[2].lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x499f4; continue; }
                pc = 0x499d4; continue;
            }
            0x499d4 => {
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                xmm3 = Xmm::scalar(m.read32(0x475c78));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0 = xmm2;
                pc = 0x49bc2; continue;
            }
            0x499f4 => {
                xmm1 = xmm2;
                xmm1.lanes[0] = sse::divss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                pc = 0x49bbe; continue;
            }
            0x49a0b => {
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x49b10; continue; }
                pc = 0x49a14; continue;
            }
            0x49a14 => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x49a27; continue; }
                pc = 0x49a19; continue;
            }
            0x49a19 => {
                xh[0] = xmm0;
                xmm3 = xmm0;
                xmm0 = xh[1];
                pc = 0x49bc2; continue;
            }
            0x49a27 => {
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x49a3b; continue; }
                pc = 0x49a33; continue;
            }
            0x49a33 => {
                xh[0] = xmm0;
                pc = 0x49b34; continue;
            }
            0x49a3b => {
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x8)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                xh[0] = Xmm::scalar(m.read32(0x475c78));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x30), xmm2.lanes[0]);
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x34), xmm2.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x49ac5; continue; }
                pc = 0x49a9b; continue;
            }
            0x49a9b => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm3 = xh[0];
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                pc = 0x49bc2; continue;
            }
            0x49ac5 => {
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                xmm3.lanes[0] = sse::divss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                xmm2 = xmm3;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                pc = 0x49bc2; continue;
            }
            0x49b10 => {
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm4.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x49b2c; continue; }
                pc = 0x49b1c; continue;
            }
            0x49b1c => {
                xh[0] = Xmm::scalar(m.read32(0x475c78));
                xmm3 = xmm0;
                pc = 0x49865; continue;
            }
            0x49b2c => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                xh[0] = xmm0;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x49b4b; continue; }
                pc = 0x49b34; continue;
            }
            0x49b34 => {
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::divss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm3 = xmm0;
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0 = xh[0];
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                pc = 0x49bbe; continue;
            }
            0x49b4b => {
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x8)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                xmm4 = xmm0;
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm4.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x49ba7; continue; }
                pc = 0x49b92; continue;
            }
            0x49b92 => {
                xmm3 = Xmm::scalar(m.read32(0x475c78));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0 = xmm2;
                pc = 0x49bc2; continue;
            }
            0x49ba7 => {
                xmm1.lanes[0] = sse::divss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                pc = 0x49bbe; continue;
            }
            0x49bbe => {
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                pc = 0x49bc2; continue;
            }
            0x49bc2 => {
                edx = m.read32(esp.wrapping_add(0x3c));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x49bfe; continue; }
                pc = 0x49bca; continue;
            }
            0x49bca => {
                xmm1 = xh[0];
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                m.write32(edx, xmm1.lanes[0]);
                xmm1 = xh[0];
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x20)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                m.write32(edx.wrapping_add(0x4), xmm1.lanes[0]);
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                m.write32(edx.wrapping_add(0x8), xh[0].lanes[0]);
                pc = 0x49bfe; continue;
            }
            0x49bfe => {
                eax = m.read32(esp.wrapping_add(0x40));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x49c3a; continue; }
                pc = 0x49c06; continue;
            }
            0x49c06 => {
                xmm1 = xmm3;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                m.write32(eax, xmm1.lanes[0]);
                xmm1 = xmm3;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x8)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xc)))).to_bits();
                m.write32(eax.wrapping_add(0x4), xmm1.lanes[0]);
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                m.write32(eax.wrapping_add(0x8), xmm3.lanes[0]);
                pc = 0x49c3a; continue;
            }
            0x49c3a => {
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                eax = m.read32(esp.wrapping_add(0x2c));
                eax = (eax & 0x7fffffff);
                m.write32(esp.wrapping_add(0x2c), eax);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x2c))) as f64));
                esp = esp.wrapping_add(0x28);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

// portreg: va=0x0004bbd0
/// 0x0004bbd0: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f4bbd0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x4bbd0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x4bbd0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(esp.wrapping_add(0x8));
                edx = m.read32(ecx);
                m.write32(eax, edx);
                edx = m.read32(ecx.wrapping_add(0x4));
                m.write32(eax.wrapping_add(0x4), edx);
                edx = m.read32(ecx.wrapping_add(0x8));
                m.write32(eax.wrapping_add(0x8), edx);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x10));
                edx = m.read32(esi);
                m.write32(eax.wrapping_add(0xc), edx);
                edx = m.read32(esi.wrapping_add(0x4));
                m.write32(eax.wrapping_add(0x10), edx);
                edx = m.read32(esi.wrapping_add(0x8));
                m.write32(eax.wrapping_add(0x14), edx);
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(0x8))) as f64));
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(0x4))) as f64));
                edx = m.read32(esp.wrapping_add(0x14));
                fpu.push((f32::from_bits(m.read32(esi)) as f64));
                { let value = fpu.st(2); fpu.push(value); }
                { let value = round_pc((fpu.st(3) * fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 3) & 7) as usize] = value; } fpu.pop();
                { let value = fpu.st(1); fpu.push(value); }
                { let value = round_pc((fpu.st(0) * fpu.st(2)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc((fpu.st(3) + fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 3) & 7) as usize] = value; } fpu.pop();
                { let value = fpu.st(0); fpu.push(value); }
                { let value = round_pc((fpu.st(0) * fpu.st(1)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc((fpu.st(3) + fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 3) & 7) as usize] = value; } fpu.pop();
                fpu.stack.swap(fpu.top as usize, ((fpu.top + 2) & 7) as usize);
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.stack[((fpu.top + 2) & 7) as usize] = fpu.st(0); fpu.pop();
                fpu.pop();
                m.write32(eax.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm0 = Xmm::scalar(m.read32(esi));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx))).to_bits();
                m.write32(eax.wrapping_add(0xf4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x4)))).to_bits();
                m.write32(eax.wrapping_add(0xf8), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                m.write32(eax.wrapping_add(0xfc), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(0x10)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                m.write32(eax.wrapping_add(0x100), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x14)))).to_bits();
                m.write32(eax.wrapping_add(0x104), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(0x18)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                m.write32(eax.wrapping_add(0x108), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x20)))).to_bits();
                m.write32(eax.wrapping_add(0x10c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(0x24)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                m.write32(eax.wrapping_add(0x110), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x28)))).to_bits();
                m.write32(eax.wrapping_add(0x114), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf4)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x10c)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x100)))).to_bits();
                m.write32(eax.wrapping_add(0x7c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf8)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x110)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x104)))).to_bits();
                m.write32(eax.wrapping_add(0x80), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xfc)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x114)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x108)))).to_bits();
                m.write32(eax.wrapping_add(0x84), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x10c)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x100)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf4)))).to_bits();
                m.write32(eax.wrapping_add(0x88), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x110)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x104)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf8)))).to_bits();
                m.write32(eax.wrapping_add(0x8c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x114)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x108)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xfc)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                m.write32(eax.wrapping_add(0x90), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf4)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x100)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x10c)))).to_bits();
                m.write32(eax.wrapping_add(0x94), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf8)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x104)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x110)))).to_bits();
                m.write32(eax.wrapping_add(0x98), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0xfc)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x108)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x114)))).to_bits();
                m.write32(eax.wrapping_add(0x9c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf4)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x100)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x10c)))).to_bits();
                m.write32(eax.wrapping_add(0xa0), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf8)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x104)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x110)))).to_bits();
                m.write32(eax.wrapping_add(0xa4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xfc)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x108)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x114)))).to_bits();
                m.write32(eax.wrapping_add(0xa8), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf4)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x100)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x10c)))).to_bits();
                m.write32(eax.wrapping_add(0xac), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf8)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x104)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x110)))).to_bits();
                m.write32(eax.wrapping_add(0xb0), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xfc)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x108)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x114)))).to_bits();
                m.write32(eax.wrapping_add(0xb4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x100)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf4)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x10c)))).to_bits();
                m.write32(eax.wrapping_add(0xb8), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x104)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf8)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x110)))).to_bits();
                m.write32(eax.wrapping_add(0xbc), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x108)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xfc)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x114)))).to_bits();
                m.write32(eax.wrapping_add(0xc0), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf4)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x100)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x10c)))).to_bits();
                m.write32(eax.wrapping_add(0xc4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf8)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x104)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x110)))).to_bits();
                m.write32(eax.wrapping_add(0xc8), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0xfc)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x108)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x114)))).to_bits();
                m.write32(eax.wrapping_add(0xcc), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf4)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x100)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x10c)))).to_bits();
                m.write32(eax.wrapping_add(0xd0), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xf8)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x104)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x110)))).to_bits();
                m.write32(eax.wrapping_add(0xd4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xfc)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x108)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x114)))).to_bits();
                m.write32(eax.wrapping_add(0xd8), xmm0.lanes[0]);
                xmm0 = Xmm::default();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edx))).to_bits();
                m.write32(eax.wrapping_add(0x1c), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x4)))).to_bits();
                m.write32(eax.wrapping_add(0x20), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x8)))).to_bits();
                m.write32(eax.wrapping_add(0x24), xmm1.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x90)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x8c)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x20)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x88)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x1c)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(eax.wrapping_add(0x28), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x10)))).to_bits();
                m.write32(eax.wrapping_add(0x2c), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x14)))).to_bits();
                m.write32(eax.wrapping_add(0x30), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x18)))).to_bits();
                m.write32(eax.wrapping_add(0x34), xmm1.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x90)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x8c)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x30)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x88)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x2c)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(eax.wrapping_add(0x38), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x20)))).to_bits();
                m.write32(eax.wrapping_add(0x3c), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x24)))).to_bits();
                m.write32(eax.wrapping_add(0x40), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x28)))).to_bits();
                m.write32(eax.wrapping_add(0x44), xmm1.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x90)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x8c)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x40)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x88)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x3c)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(eax.wrapping_add(0x48), xmm1.lanes[0]);
                esi = m.read32(edx);
                m.write32(eax.wrapping_add(0x4c), esi);
                esi = m.read32(edx.wrapping_add(0x4));
                m.write32(eax.wrapping_add(0x50), esi);
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_add(0x8)));
                m.write32(eax.wrapping_add(0x54), xmm1.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0xd8)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0xd4)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x50)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x4c)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xd0)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(eax.wrapping_add(0x58), xmm1.lanes[0]);
                esi = m.read32(edx.wrapping_add(0x10));
                m.write32(eax.wrapping_add(0x5c), esi);
                esi = m.read32(edx.wrapping_add(0x14));
                m.write32(eax.wrapping_add(0x60), esi);
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_add(0x18)));
                m.write32(eax.wrapping_add(0x64), xmm1.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0xd8)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0xd4)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x60)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x5c)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xd0)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(eax.wrapping_add(0x68), xmm1.lanes[0]);
                esi = m.read32(edx.wrapping_add(0x20));
                m.write32(eax.wrapping_add(0x6c), esi);
                esi = m.read32(edx.wrapping_add(0x24));
                m.write32(eax.wrapping_add(0x70), esi);
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_add(0x28)));
                m.write32(eax.wrapping_add(0x74), xmm1.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0xd8)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0xd4)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x70)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x6c)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xd0)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(eax.wrapping_add(0x78), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0xf4)));
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0xc));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x100)));
                edx = (edx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x10), edx);
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0xc));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x10c)));
                edx = (edx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x8), edx);
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0xc));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0xf8)));
                edx = (edx & 0x7fffffff);
                m.write32(esp.wrapping_add(0xc), edx);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x8)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0xc));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x104)));
                edx = (edx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x10), edx);
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0xc));
                edx = (edx & 0x7fffffff);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x110)));
                xmm3 = Xmm::scalar(m.read32(ecx));
                m.write32(esp.wrapping_add(0x8), edx);
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0xc));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0xfc)));
                edx = (edx & 0x7fffffff);
                m.write32(esp.wrapping_add(0xc), edx);
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x8)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0xc));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x108)));
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x114)));
                edx = (edx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x10), edx);
                edx = m.read32(esp.wrapping_add(0xc));
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                edx = (edx & 0x7fffffff);
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x8), edx);
                edx = m.read32(esp.wrapping_add(0xc));
                m.write32(eax.wrapping_add(0xdc), xmm3.lanes[0]);
                xmm3 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                edx = (edx & 0x7fffffff);
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(eax.wrapping_add(0xe0), xmm3.lanes[0]);
                xmm3 = Xmm::scalar(m.read32(ecx.wrapping_add(0x8)));
                m.write32(esp.wrapping_add(0xc), edx);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(eax.wrapping_add(0xe4), xmm3.lanes[0]);
                xmm3 = Xmm::scalar(m.read32(ecx));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(eax.wrapping_add(0xe8), xmm3.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(eax.wrapping_add(0xec), xmm1.lanes[0]);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                m.write32(eax.wrapping_add(0xf0), xmm0.lanes[0]);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00050170: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f50170<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x50170;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x50170 => {
                ecx = m.read32(esp.wrapping_add(0x8));
                esp = esp.wrapping_sub(0x44);
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x4c));
                fa = ebx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                push!(esi);
                push!(edi);
                if (fa != fb) { pc = 0x5019d; continue; }
                pc = 0x50183; continue;
            }
            0x50183 => {
                xmm0 = Xmm::scalar(m.read32(ecx));
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x54), xmm0.lanes[0]);
                pc = 0x503dd; continue;
            }
            0x5019d => {
                fa = ebx;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x501d6; continue; }
                pc = 0x501a2; continue;
            }
            0x501a2 => {
                xmm1 = Xmm::scalar(m.read32(ecx.wrapping_add(0x8)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                xmm0 = Xmm::scalar(m.read32(0x475cd4));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x54), xmm1.lanes[0]);
                pc = 0x503dd; continue;
            }
            0x501d6 => {
                xmm1 = Xmm::default();
                esi = ebx.wrapping_add(((0x1i32.wrapping_neg()) as u32));
                edi = 0;
                fa = esi;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                xmm2 = xmm1;
                xmm4 = xmm1;
                if (fas < fbs) { pc = 0x50315; continue; }
                pc = 0x501ed; continue;
            }
            0x501ed => {
                edx = esi.wrapping_add(((0x4i32.wrapping_neg()) as u32));
                edx = edx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_add(1);
                eax = ecx.wrapping_add(0x8);
                edi = edx.wrapping_mul(0x4);
                edi = edi;
                pc = 0x50200; continue;
            }
            0x50200 => {
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(((0x8i32.wrapping_neg()) as u32))));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                xmm3 = Xmm::scalar(m.read32(eax));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(((0x4i32.wrapping_neg()) as u32))))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(((0x8i32.wrapping_neg()) as u32))))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(((0x4i32.wrapping_neg()) as u32))))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(eax));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xc)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = xmm0;
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0xc)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x14)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0xc)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x10)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x10)))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0xc)));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x14)))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x18)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x14)))).to_bits();
                xh[0] = xmm0;
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x10)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x1c)))).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(eax.wrapping_add(0x18)));
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x10)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xmm4;
                xmm4 = Xmm::scalar(m.read32(eax.wrapping_add(0x1c)));
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x14)))).to_bits();
                eax = eax.wrapping_add(0x20);
                edx = edx.wrapping_sub(1);
                fa = edx;
                xmm1 = xmm0;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                if ((fa != 0)) { pc = 0x50200; continue; }
                pc = 0x50315; continue;
            }
            0x50315 => {
                fa = edi;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x50370; continue; }
                pc = 0x50319; continue;
            }
            0x50319 => {
                eax = ecx.wrapping_add(edi.wrapping_mul(0x8)).wrapping_add(0xc);
                esi = esi.wrapping_sub(edi);
                pc = 0x50320; continue;
            }
            0x50320 => {
                xmm0 = Xmm::scalar(m.read32(eax));
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(((0x4i32.wrapping_neg()) as u32))));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(((0x8i32.wrapping_neg()) as u32))))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(((0xci32.wrapping_neg()) as u32))))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm3;
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(((0x4i32.wrapping_neg()) as u32))));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(((0xci32.wrapping_neg()) as u32))))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xmm3;
                xmm3 = Xmm::scalar(m.read32(eax));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(((0x8i32.wrapping_neg()) as u32))))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                eax = eax.wrapping_add(0x8);
                esi = esi.wrapping_sub(1);
                fa = esi;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = xmm3;
                if ((fa != 0)) { pc = 0x50320; continue; }
                pc = 0x50370; continue;
            }
            0x50370 => {
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm3 = Xmm::scalar(m.read32(ecx.wrapping_add(ebx.wrapping_mul(0x8)).wrapping_add(((0x4i32.wrapping_neg()) as u32))));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(ebx.wrapping_mul(0x8)).wrapping_add(((0x8i32.wrapping_neg()) as u32))))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x475d18))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                xmm1.lanes[0] = sse::divss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(ecx));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(ebx.wrapping_mul(0x8)).wrapping_add(((0x8i32.wrapping_neg()) as u32))))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(ecx.wrapping_add(ebx.wrapping_mul(0x8)).wrapping_add(((0x4i32.wrapping_neg()) as u32))));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x54), xmm2.lanes[0]);
                pc = 0x503dd; continue;
            }
            0x503dd => {
                eax = 0;
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x5040f; continue; }
                pc = 0x503e3; continue;
            }
            0x503e3 => {
                fpu.push((f32::from_bits(m.read32(ecx.wrapping_add(eax.wrapping_mul(0x8)).wrapping_add(0x4))) as f64));
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                { let value = round_pc((fpu.st(0) - (f32::from_bits(m.read32(esp.wrapping_add(0x54))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.push((f32::from_bits(m.read32(ecx.wrapping_add(eax.wrapping_mul(0x8)).wrapping_add(((0x8i32.wrapping_neg()) as u32)))) as f64));
                { let value = round_pc((fpu.st(0) - (f32::from_bits(m.read32(esp.wrapping_add(0xc))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = fpu.st(1).atan2(fpu.st(0)); fpu.stack[((fpu.top + 1) & 7) as usize] = value; fpu.pop(); }
                m.write32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x2c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                if (fas < fbs) { pc = 0x503e3; continue; }
                pc = 0x503fe; continue;
            }
            0x503fe => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x5040f; continue; }
                pc = 0x50402; continue;
            }
            0x50402 => {
                ecx = ebx;
                eax = 0x1;
                edi = esp.wrapping_add(0x10);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                pc = 0x5040f; continue;
            }
            0x5040f => {
                eax = m.read32(esp.wrapping_add(0x60));
                ecx = m.read32(esp.wrapping_add(0x64));
                edi = m.read32(esp.wrapping_add(0x5c));
                m.write32(ecx, eax);
                esi = 0x1;
                ecx = ecx.wrapping_add(0x4);
                fa = edi;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x10), 0);
                if (fas <= fbs) { pc = 0x50621; continue; }
                pc = 0x50435; continue;
            }
            0x50435 => {
                xmm4 = Xmm::scalar(m.read32(0x475c94));
                xh[1] = Xmm::scalar(m.read32(0x47861c));
                xmm3 = Xmm::scalar(m.read32(0x475c90));
                eax = esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x30);
                xmm0.lanes[0] = (((edi) as i32) as f32).to_bits();
                xh[0] = xmm4;
                xh[0].lanes[0] = sse::divss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x64), eax);
                push!(ebp);
                pc = 0x50461; continue;
            }
            0x50461 => {
                edx = m.read32(esp.wrapping_add(0x68));
                xmm0.lanes[0] = (((esi) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx))).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                xmm2 = xh[1];
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x5047d; continue; }
                pc = 0x50479; continue;
            }
            0x50479 => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                pc = 0x5047d; continue;
            }
            0x5047d => {
                eax = 0;
                fa = ebx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x505b8; continue; }
                pc = 0x50488; continue;
            }
            0x50488 => {
                edx = ebx.wrapping_add(((0x3i32.wrapping_neg()) as u32));
                pc = 0x50490; continue;
            }
            0x50490 => {
                ebp = m.read32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x14));
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x504d5; continue; }
                pc = 0x50498; continue;
            }
            0x50498 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x34)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x58), xmm1.lanes[0]);
                ebp = m.read32(esp.wrapping_add(0x58));
                ebp = (ebp & 0x7fffffff);
                m.write32(esp.wrapping_add(0x64), ebp);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x64)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x504cb; continue; }
                pc = 0x504c1; continue;
            }
            0x504c1 => {
                xh[2] = xmm4;
                xh[2].lanes[0] = sse::subss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xh[2];
                pc = 0x504cb; continue;
            }
            0x504cb => {
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x504d5; continue; }
                pc = 0x504d0; continue;
            }
            0x504d0 => {
                xmm2 = xmm1;
                m.write32(ecx, eax);
                pc = 0x504d5; continue;
            }
            0x504d5 => {
                ebp = m.read32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x18));
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x5051d; continue; }
                pc = 0x504dd; continue;
            }
            0x504dd => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x38)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x58), xmm1.lanes[0]);
                ebp = m.read32(esp.wrapping_add(0x58));
                ebp = (ebp & 0x7fffffff);
                m.write32(esp.wrapping_add(0x64), ebp);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x64)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50510; continue; }
                pc = 0x50506; continue;
            }
            0x50506 => {
                xh[2] = xmm4;
                xh[2].lanes[0] = sse::subss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xh[2];
                pc = 0x50510; continue;
            }
            0x50510 => {
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x5051d; continue; }
                pc = 0x50515; continue;
            }
            0x50515 => {
                ebp = eax.wrapping_add(0x1);
                xmm2 = xmm1;
                m.write32(ecx, ebp);
                pc = 0x5051d; continue;
            }
            0x5051d => {
                ebp = m.read32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x1c));
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x50565; continue; }
                pc = 0x50525; continue;
            }
            0x50525 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x3c)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x58), xmm1.lanes[0]);
                ebp = m.read32(esp.wrapping_add(0x58));
                ebp = (ebp & 0x7fffffff);
                m.write32(esp.wrapping_add(0x64), ebp);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x64)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50558; continue; }
                pc = 0x5054e; continue;
            }
            0x5054e => {
                xh[2] = xmm4;
                xh[2].lanes[0] = sse::subss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xh[2];
                pc = 0x50558; continue;
            }
            0x50558 => {
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50565; continue; }
                pc = 0x5055d; continue;
            }
            0x5055d => {
                ebp = eax.wrapping_add(0x2);
                xmm2 = xmm1;
                m.write32(ecx, ebp);
                pc = 0x50565; continue;
            }
            0x50565 => {
                ebp = m.read32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x20));
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x505ad; continue; }
                pc = 0x5056d; continue;
            }
            0x5056d => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x40)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x58), xmm1.lanes[0]);
                ebp = m.read32(esp.wrapping_add(0x58));
                ebp = (ebp & 0x7fffffff);
                m.write32(esp.wrapping_add(0x64), ebp);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x64)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x505a0; continue; }
                pc = 0x50596; continue;
            }
            0x50596 => {
                xh[2] = xmm4;
                xh[2].lanes[0] = sse::subss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xh[2];
                pc = 0x505a0; continue;
            }
            0x505a0 => {
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x505ad; continue; }
                pc = 0x505a5; continue;
            }
            0x505a5 => {
                ebp = eax.wrapping_add(0x3);
                xmm2 = xmm1;
                m.write32(ecx, ebp);
                pc = 0x505ad; continue;
            }
            0x505ad => {
                eax = eax.wrapping_add(0x4);
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x50490; continue; }
                pc = 0x505b8; continue;
            }
            0x505b8 => {
                fa = eax;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x5060a; continue; }
                pc = 0x505bc; continue;
            }
            0x505bc => {
                pc = 0x505c0; continue;
            }
            0x505c0 => {
                edx = m.read32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x14));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x50605; continue; }
                pc = 0x505c8; continue;
            }
            0x505c8 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x34)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x58), xmm1.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x58));
                edx = (edx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x64), edx);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x64)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x505fb; continue; }
                pc = 0x505f1; continue;
            }
            0x505f1 => {
                xh[2] = xmm4;
                xh[2].lanes[0] = sse::subss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xh[2];
                pc = 0x505fb; continue;
            }
            0x505fb => {
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50605; continue; }
                pc = 0x50600; continue;
            }
            0x50600 => {
                xmm2 = xmm1;
                m.write32(ecx, eax);
                pc = 0x50605; continue;
            }
            0x50605 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x505c0; continue; }
                pc = 0x5060a; continue;
            }
            0x5060a => {
                eax = m.read32(ecx);
                ecx = ecx.wrapping_add(0x4);
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x14), 0);
                if (fas < fbs) { pc = 0x50461; continue; }
                pc = 0x50620; continue;
            }
            0x50620 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x50621; continue;
            }
            0x50621 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x44);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00050630: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f50630<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x50630;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x50630 => {
                esp = esp.wrapping_sub(0x184);
                ecx = m.read32(esp.wrapping_add(0x188));
                push!(ebx);
                eax = 0;
                push!(ebp);
                m.write32(esp.wrapping_add(0x8), eax);
                m.write32(esp.wrapping_add(0x9c), eax);
                eax = m.read32(esp.wrapping_add(0x19c));
                xmm0 = Xmm::scalar(m.read32(eax));
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x19c));
                push!(edi);
                eax = esp.wrapping_add(0x70);
                push!(eax);
                push!(esi);
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x3c), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x40), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x78), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x7c), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x80), xmm2.lanes[0]);
                call!(0x1521d0);
                pc = 0x506aa; continue;
            }
            0x506aa => {
                eax = m.read32(esp.wrapping_add(0x1a4));
                xmm0 = Xmm::scalar(m.read32(eax));
                edi = m.read32(esp.wrapping_add(0x1b4));
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x4)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x4)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0x4)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x14)))).to_bits();
                m.write32(esp.wrapping_add(0x44), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                xh[0] = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x28)))).to_bits();
                m.write32(esp.wrapping_add(0x48), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                eax = m.read32(esp.wrapping_add(0x1b0));
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x10)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(edi))).to_bits();
                m.write32(esp.wrapping_add(0x4c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax));
                m.write32(esp.wrapping_add(0x54), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x58), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                m.write32(esp.wrapping_add(0x5c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(edi));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x18)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(edi.wrapping_add(0x10)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(edi.wrapping_add(0x20)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                edx = edi.wrapping_add(0x10);
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0x4)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x24)))).to_bits();
                ebp = edi.wrapping_add(0x20);
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(edi.wrapping_add(0x8)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x18)))).to_bits();
                eax = esi.wrapping_add(0x10);
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x14)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                esp = esp.wrapping_add(0x8);
                m.write32(esp.wrapping_add(0x58), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x5c), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x48), xh[0].lanes[0]);
                m.write32(esp.wrapping_add(0x7c), xmm2.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0x14)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x14)))).to_bits();
                xmm3 = Xmm::scalar(m.read32(eax));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(edx))).to_bits();
                xh[1] = Xmm::scalar(m.read32(edi.wrapping_add(0x28)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x18)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x18)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x18)))).to_bits();
                xmm4 = Xmm::scalar(m.read32(esi.wrapping_add(0x20)));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(edx))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                ebx = m.read32(esp.wrapping_add(0x24));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x7c)));
                ebx = (ebx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x88), ebx);
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x20)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(edi))).to_bits();
                xh[2] = Xmm::scalar(m.read32(edi.wrapping_add(0x24)));
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x24)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm1.lanes[0]);
                ebx = m.read32(esp.wrapping_add(0x24));
                ebx = (ebx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x90), ebx);
                m.write32(esp.wrapping_add(0x64), xmm2.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(edi.wrapping_add(0x24)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x14)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xh[0].lanes[0]);
                ebx = m.read32(esp.wrapping_add(0x24));
                ebx = (ebx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x60), ebx);
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(eax));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(ebp))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                ebx = m.read32(esp.wrapping_add(0x24));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x64)));
                ebx = (ebx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x84), ebx);
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(edi.wrapping_add(0x8)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x28)))).to_bits();
                ecx = esi.wrapping_add(0x20);
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x24)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                ebx = m.read32(esp.wrapping_add(0x24));
                ebx = (ebx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x8c), ebx);
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x24)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x14)))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(esi.wrapping_add(0x28)));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x18)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xh[1].lanes[0]);
                ebx = m.read32(esp.wrapping_add(0x24));
                ebx = (ebx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x94), ebx);
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(edi.wrapping_add(0x28)));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x28)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm2.lanes[0]);
                ebx = m.read32(esp.wrapping_add(0x24));
                ebx = (ebx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x24), ebx);
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = Xmm::scalar(m.read32(ecx));
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(ebp))).to_bits();
                m.write32(esp.wrapping_add(0x1c), xmm3.lanes[0]);
                ebx = m.read32(esp.wrapping_add(0x1c));
                ebx = (ebx & 0x7fffffff);
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x80), ebx);
                m.write32(esp.wrapping_add(0x1c), xmm4.lanes[0]);
                ebx = m.read32(esp.wrapping_add(0x1c));
                ebx = (ebx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x6c), xh[1].lanes[0]);
                m.write32(esp.wrapping_add(0x68), ebx);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x70)));
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x54)));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x90)));
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                xh[1] = Xmm::scalar(m.read32(0x478618));
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                ebx = m.read32(esp.wrapping_add(0x2c));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x60)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x88)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4c)))).to_bits();
                ebx = (ebx & 0x7fffffff);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                m.write32(esp.wrapping_add(0x2c), ebx);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::default();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x20), xh[1].lanes[0]);
                m.write32(esp.wrapping_add(0x1c), 0);
                m.write32(esp.wrapping_add(0x28), 0);
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x509dd; continue;
            }
            0x509dd => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xh[1].lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50a0b; continue; }
                pc = 0x509e2; continue;
            }
            0x509e2 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x70))) as f64);
                m.write32(esp.wrapping_add(0x20), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x10), esi);
                m.write32(esp.wrapping_add(0x1c), 0x1);
                if ((fca > fcb)) { pc = 0x50a03; continue; }
                pc = 0x509fb; continue;
            }
            0x509fb => {
                m.write32(esp.wrapping_add(0x1c), 0);
                pc = 0x50a03; continue;
            }
            0x50a03 => {
                m.write32(esp.wrapping_add(0x28), 0x1);
                pc = 0x50a0b; continue;
            }
            0x50a0b => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x74)));
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x8c)));
                m.write32(esp.wrapping_add(0x2c), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x94)));
                ebx = m.read32(esp.wrapping_add(0x2c));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x84)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x4c)));
                ebx = (ebx & 0x7fffffff);
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                m.write32(esp.wrapping_add(0x2c), ebx);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x50a7f; continue;
            }
            0x50a7f => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50ab2; continue; }
                pc = 0x50a86; continue;
            }
            0x50a86 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x74))) as f64);
                m.write32(esp.wrapping_add(0x20), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x10), eax);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50aa6; continue; }
                pc = 0x50a97; continue;
            }
            0x50a97 => {
                ebx = 0x1;
                m.write32(esp.wrapping_add(0x28), 0x2);
                pc = 0x50ab6; continue;
            }
            0x50aa6 => {
                ebx = 0;
                m.write32(esp.wrapping_add(0x28), 0x2);
                pc = 0x50ab6; continue;
            }
            0x50ab2 => {
                ebx = m.read32(esp.wrapping_add(0x1c));
                pc = 0x50ab6; continue;
            }
            0x50ab6 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x78)));
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x68)));
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                eax = m.read32(esp.wrapping_add(0x14));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x80)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x50)))).to_bits();
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                eax = (eax & 0x7fffffff);
                xmm1 = xh[0];
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), eax);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x50b1a; continue;
            }
            0x50b1a => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50b4f; continue; }
                pc = 0x50b21; continue;
            }
            0x50b21 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x78))) as f64);
                m.write32(esp.wrapping_add(0x20), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x10), ecx);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50b42; continue; }
                pc = 0x50b32; continue;
            }
            0x50b32 => {
                eax = 0x3;
                ebx = 0x1;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x50b53; continue;
            }
            0x50b42 => {
                eax = 0x3;
                ebx = 0;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x50b53; continue;
            }
            0x50b4f => {
                eax = m.read32(esp.wrapping_add(0x28));
                pc = 0x50b53; continue;
            }
            0x50b53 => {
                xmm1 = Xmm::scalar(m.read32(edi));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                xh[2] = Xmm::scalar(m.read32(edi.wrapping_add(0x8)));
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x38)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x34)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = xh[0];
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x84)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x14));
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x88)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                ecx = (ecx & 0x7fffffff);
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4c)))).to_bits();
                m.write32(esp.wrapping_add(0x14), ecx);
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x50bd5; continue;
            }
            0x50bd5 => {
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50bfd; continue; }
                pc = 0x50bdc; continue;
            }
            0x50bdc => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x20), xh[0].lanes[0]);
                m.write32(esp.wrapping_add(0x10), edi);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50bf2; continue; }
                pc = 0x50beb; continue;
            }
            0x50beb => {
                ebx = 0x1;
                pc = 0x50bf4; continue;
            }
            0x50bf2 => {
                ebx = 0;
                pc = 0x50bf4; continue;
            }
            0x50bf4 => {
                eax = 0x4;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x50bfd; continue;
            }
            0x50bfd => {
                xmm1 = Xmm::scalar(m.read32(edx));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                xh[0] = Xmm::scalar(m.read32(edi.wrapping_add(0x14)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x34)))).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x80)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x44)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x18)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x8c)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x90)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x14));
                ecx = (ecx & 0x7fffffff);
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x50)))).to_bits();
                m.write32(esp.wrapping_add(0x14), ecx);
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x50c87; continue;
            }
            0x50c87 => {
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50cb6; continue; }
                pc = 0x50c8e; continue;
            }
            0x50c8e => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x20), xh[0].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50ca9; continue; }
                pc = 0x50c99; continue;
            }
            0x50c99 => {
                eax = 0x5;
                ebx = 0x1;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x50cba; continue;
            }
            0x50ca9 => {
                eax = 0x5;
                ebx = 0;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x50cba; continue;
            }
            0x50cb6 => {
                edx = m.read32(esp.wrapping_add(0x10));
                pc = 0x50cba; continue;
            }
            0x50cba => {
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(0x24)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x34)))).to_bits();
                xh[0] = Xmm::scalar(m.read32(edi.wrapping_add(0x28)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x38)))).to_bits();
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x68)));
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x44)))).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x54)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(ebp));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x94)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x60)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x14));
                ecx = (ecx & 0x7fffffff);
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), ecx);
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x50d43; continue;
            }
            0x50d43 => {
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50d69; continue; }
                pc = 0x50d4a; continue;
            }
            0x50d4a => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x20), xh[0].lanes[0]);
                edx = ebp;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50d5e; continue; }
                pc = 0x50d57; continue;
            }
            0x50d57 => {
                ebx = 0x1;
                pc = 0x50d60; continue;
            }
            0x50d5e => {
                ebx = 0;
                pc = 0x50d60; continue;
            }
            0x50d60 => {
                eax = 0x6;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x50d69; continue;
            }
            0x50d69 => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x7c)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x78)))).to_bits();
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x90)));
                xmm1 = xmm2;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x74)))).to_bits();
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x1c), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x60)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x50)))).to_bits();
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x84)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x44)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xh[0].lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x14));
                ecx = (ecx & 0x7fffffff);
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), ecx);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x50df6; continue;
            }
            0x50df6 => {
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xh[1].lanes[0]);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x7c)));
                xh[2] = xh[1];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x10), xh[1].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50ea5; continue; }
                pc = 0x50e28; continue;
            }
            0x50e28 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                xh[1] = Xmm::scalar(m.read32(0x475c78));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xh[1].lanes[0] = sse::divss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xh[2] = xh[1];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xh[2];
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x478620))).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50ea5; continue; }
                pc = 0x50e59; continue;
            }
            0x50e59 => {
                xmm1 = xh[1];
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x30), xmm1.lanes[0]);
                xmm1 = xh[1];
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x7c)))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                edx = 0;
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xh[0].lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x20), xh[2].lanes[0]);
                m.write32(esp.wrapping_add(0x38), xh[1].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50e98; continue; }
                pc = 0x50e88; continue;
            }
            0x50e88 => {
                eax = 0x7;
                ebx = 0x1;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x50eab; continue;
            }
            0x50e98 => {
                eax = 0x7;
                ebx = 0;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x50eab; continue;
            }
            0x50ea5 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                pc = 0x50eab; continue;
            }
            0x50eab => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x64)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x78)))).to_bits();
                xh[1] = xmm3;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x74)))).to_bits();
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xh[0].lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x14));
                m.write32(esp.wrapping_add(0x2c), xh[0].lanes[0]);
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x88)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x54)))).to_bits();
                xh[1] = xmm0;
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x1c), xh[1].lanes[0]);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x60)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4c)))).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x80)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x8c)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x44)))).to_bits();
                ecx = (ecx & 0x7fffffff);
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), ecx);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x50f43; continue;
            }
            0x50f43 => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xh[0].lanes[0]);
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x64)));
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x10), xh[0].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50fe9; continue; }
                pc = 0x50f75; continue;
            }
            0x50f75 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                xh[0] = Xmm::scalar(m.read32(0x475c78));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xh[0].lanes[0] = sse::divss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = xh[2];
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(0x478620))).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50fe9; continue; }
                pc = 0x50fa6; continue;
            }
            0x50fa6 => {
                xmm1 = xh[0];
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x30), xmm1.lanes[0]);
                xmm1 = xh[0];
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x64)))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                edx = 0;
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x2c))) as f64);
                m.write32(esp.wrapping_add(0x20), xh[2].lanes[0]);
                m.write32(esp.wrapping_add(0x38), xh[0].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x50fde; continue; }
                pc = 0x50fd7; continue;
            }
            0x50fd7 => {
                ebx = 0x1;
                pc = 0x50fe0; continue;
            }
            0x50fde => {
                ebx = 0;
                pc = 0x50fe0; continue;
            }
            0x50fe0 => {
                eax = 0x8;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x50fe9; continue;
            }
            0x50fe9 => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x6c)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x78)))).to_bits();
                xh[1] = xmm4;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x74)))).to_bits();
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xh[0].lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x14));
                m.write32(esp.wrapping_add(0x2c), xh[0].lanes[0]);
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x68)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                xh[1] = xmm0;
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x1c), xh[1].lanes[0]);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x88)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x50)))).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x94)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x44)))).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x90)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4c)))).to_bits();
                ecx = (ecx & 0x7fffffff);
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), ecx);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x51081; continue;
            }
            0x51081 => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xh[0].lanes[0]);
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x6c)));
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x10), xh[0].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51127; continue; }
                pc = 0x510b3; continue;
            }
            0x510b3 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                xh[0] = Xmm::scalar(m.read32(0x475c78));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xh[0].lanes[0] = sse::divss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = xh[2];
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(0x478620))).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51127; continue; }
                pc = 0x510e4; continue;
            }
            0x510e4 => {
                xmm1 = xh[0];
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x30), xmm1.lanes[0]);
                xmm1 = xh[0];
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x6c)))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                edx = 0;
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x2c))) as f64);
                m.write32(esp.wrapping_add(0x20), xh[2].lanes[0]);
                m.write32(esp.wrapping_add(0x38), xh[0].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x5111c; continue; }
                pc = 0x51115; continue;
            }
            0x51115 => {
                ebx = 0x1;
                pc = 0x5111e; continue;
            }
            0x5111c => {
                ebx = 0;
                pc = 0x5111e; continue;
            }
            0x5111e => {
                eax = 0x9;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x51127; continue;
            }
            0x51127 => {
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x58)));
                xh[0] = xmm2;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x70)))).to_bits();
                xh[1] = xh[2];
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x78)))).to_bits();
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xh[0].lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x14));
                m.write32(esp.wrapping_add(0x2c), xh[0].lanes[0]);
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x8c)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x54)))).to_bits();
                xh[1] = xmm0;
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x1c), xh[1].lanes[0]);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x94)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x50)))).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x88)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x44)))).to_bits();
                ecx = (ecx & 0x7fffffff);
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), ecx);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x511c2; continue;
            }
            0x511c2 => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xh[2] = xmm2;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x10), xh[0].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51254; continue; }
                pc = 0x511e2; continue;
            }
            0x511e2 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                xh[0] = Xmm::scalar(m.read32(0x475c78));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xh[0].lanes[0] = sse::divss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = xh[2];
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(0x478620))).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51254; continue; }
                pc = 0x51213; continue;
            }
            0x51213 => {
                xmm1 = xh[0];
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x30), xmm1.lanes[0]);
                xmm1 = xh[0];
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                edx = 0;
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x2c))) as f64);
                m.write32(esp.wrapping_add(0x20), xh[2].lanes[0]);
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x38), xh[0].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51249; continue; }
                pc = 0x51242; continue;
            }
            0x51242 => {
                ebx = 0x1;
                pc = 0x5124b; continue;
            }
            0x51249 => {
                ebx = 0;
                pc = 0x5124b; continue;
            }
            0x5124b => {
                eax = 0xa;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x51254; continue;
            }
            0x51254 => {
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x5c)));
                xh[0] = xh[1];
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x78)))).to_bits();
                xmm2 = xmm3;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x70)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = xmm0;
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x94)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4c)))).to_bits();
                m.write32(esp.wrapping_add(0x1c), xh[0].lanes[0]);
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x84)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x54)))).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x80)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x90)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x44)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm2.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x14));
                ecx = (ecx & 0x7fffffff);
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), ecx);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x512ec; continue;
            }
            0x512ec => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xh[2] = xmm3;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x10), xh[0].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x5137c; continue; }
                pc = 0x5130c; continue;
            }
            0x5130c => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                xh[0] = Xmm::scalar(m.read32(0x475c78));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xh[0].lanes[0] = sse::divss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = xh[2];
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(0x478620))).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x5137c; continue; }
                pc = 0x5133d; continue;
            }
            0x5133d => {
                xmm1 = xh[0];
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x30), xmm1.lanes[0]);
                xmm1 = xh[0];
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                edx = 0;
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x20), xh[2].lanes[0]);
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x38), xh[0].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51371; continue; }
                pc = 0x5136a; continue;
            }
            0x5136a => {
                ebx = 0x1;
                pc = 0x51373; continue;
            }
            0x51371 => {
                ebx = 0;
                pc = 0x51373; continue;
            }
            0x51373 => {
                eax = 0xb;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x5137c; continue;
            }
            0x5137c => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x84)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x50)))).to_bits();
                xmm3 = xh[0];
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x78)))).to_bits();
                xmm2 = xmm4;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x70)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x8c)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4c)))).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x68)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x60)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x44)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm2.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x14));
                ecx = (ecx & 0x7fffffff);
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), ecx);
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x51408; continue;
            }
            0x51408 => {
                xh[1] = xmm4;
                xh[2] = xmm3;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x10), xh[1].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x5149c; continue; }
                pc = 0x51425; continue;
            }
            0x51425 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                xh[1] = Xmm::scalar(m.read32(0x475c78));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xh[1].lanes[0] = sse::divss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xh[2] = xh[1];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x1c), xh[2].lanes[0]);
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(0x478620))).to_bits();
                fca = f32::from_bits(xh[2].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x5149c; continue; }
                pc = 0x51459; continue;
            }
            0x51459 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                m.write32(esp.wrapping_add(0x20), xmm1.lanes[0]);
                xmm1 = xh[1];
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x30), xmm1.lanes[0]);
                edx = 0;
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                xmm1 = xh[1];
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x38), xmm3.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51491; continue; }
                pc = 0x5148a; continue;
            }
            0x5148a => {
                ebx = 0x1;
                pc = 0x51493; continue;
            }
            0x51491 => {
                ebx = 0;
                pc = 0x51493; continue;
            }
            0x51493 => {
                eax = 0xc;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x5149c; continue;
            }
            0x5149c => {
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x7c)));
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x58)));
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x68)));
                xmm3 = xmm4;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x70)))).to_bits();
                xmm2 = xh[2];
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x74)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x80)));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x54)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm2.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x14));
                xh[0] = xh[1];
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x50)))).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm4.lanes[0]);
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x84)));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x88)));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                ecx = (ecx & 0x7fffffff);
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xh[0].lanes[0]);
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                m.write32(esp.wrapping_add(0x2c), ecx);
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x5154c; continue;
            }
            0x5154c => {
                xmm4 = xmm3;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm4.lanes[0]);
                xmm4 = xh[2];
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x2c), xmm4.lanes[0]);
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x10), xmm4.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x515f7; continue; }
                pc = 0x51581; continue;
            }
            0x51581 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm4.lanes[0] = sse::divss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xh[2] = xmm4;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x1c), xh[2].lanes[0]);
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(0x478620))).to_bits();
                fca = f32::from_bits(xh[2].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x515f7; continue; }
                pc = 0x515b5; continue;
            }
            0x515b5 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                m.write32(esp.wrapping_add(0x20), xmm1.lanes[0]);
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1 = xmm4;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x58)))).to_bits();
                edx = 0;
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x30), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x38), xmm4.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x515ec; continue; }
                pc = 0x515e5; continue;
            }
            0x515e5 => {
                ebx = 0x1;
                pc = 0x515ee; continue;
            }
            0x515ec => {
                ebx = 0;
                pc = 0x515ee; continue;
            }
            0x515ee => {
                eax = 0xd;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x515f7; continue;
            }
            0x515f7 => {
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x64)));
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x5c)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x74)))).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4c)))).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x8c)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                xmm4 = xmm2;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x70)))).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = xmm0;
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                m.write32(esp.wrapping_add(0x14), xmm3.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x14));
                xh[2] = xmm2;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x54)))).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x90)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                ecx = (ecx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x14), ecx);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x51683; continue;
            }
            0x51683 => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x5c)));
                xh[2] = xmm4;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x10), xh[0].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51719; continue; }
                pc = 0x516a3; continue;
            }
            0x516a3 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                xh[0] = Xmm::scalar(m.read32(0x475c78));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xh[0].lanes[0] = sse::divss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x1c), xh[2].lanes[0]);
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(0x478620))).to_bits();
                fca = f32::from_bits(xh[2].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51719; continue; }
                pc = 0x516d7; continue;
            }
            0x516d7 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                m.write32(esp.wrapping_add(0x20), xmm1.lanes[0]);
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1 = xh[0];
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x5c)))).to_bits();
                edx = 0;
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x30), xmm4.lanes[0]);
                m.write32(esp.wrapping_add(0x38), xh[0].lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x5170e; continue; }
                pc = 0x51707; continue;
            }
            0x51707 => {
                ebx = 0x1;
                pc = 0x51710; continue;
            }
            0x5170e => {
                ebx = 0;
                pc = 0x51710; continue;
            }
            0x51710 => {
                eax = 0xe;
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x51719; continue;
            }
            0x51719 => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x6c)));
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x50)))).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x94)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                xmm4 = xh[0];
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x70)))).to_bits();
                xmm3 = xh[2];
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x74)))).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = xmm0;
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x80)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4c)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm3.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x14));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x60)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                ecx = (ecx & 0x7fffffff);
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), ecx);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x522b8; continue; }
                pc = 0x517a5; continue;
            }
            0x517a5 => {
                xmm2 = xh[2];
                xh[0] = xmm4;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x10), xmm2.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51b3f; continue; }
                pc = 0x517c6; continue;
            }
            0x517c6 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                xmm2 = Xmm::scalar(m.read32(0x475c78));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm2.lanes[0] = sse::divss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xh[0] = xmm2;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = xh[0];
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(0x478620))).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51b3f; continue; }
                pc = 0x517fb; continue;
            }
            0x517fb => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1 = xmm2;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x20), xh[0].lanes[0]);
                m.write32(esp.wrapping_add(0x30), xmm4.lanes[0]);
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x38), xmm2.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51828; continue; }
                pc = 0x51821; continue;
            }
            0x51821 => {
                ebx = 0x1;
                pc = 0x5182a; continue;
            }
            0x51828 => {
                ebx = 0;
                pc = 0x5182a; continue;
            }
            0x5182a => {
                m.write32(esp.wrapping_add(0x28), 0xf);
                pc = 0x51832; continue;
            }
            0x51832 => {
                ebp = m.read32(esp.wrapping_add(0x1b4));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                m.write32(ebp, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                push!(ebp);
                push!(esi);
                m.write32(ebp.wrapping_add(0x4), xmm1.lanes[0]);
                m.write32(ebp.wrapping_add(0x8), xmm0.lanes[0]);
                call!(0x152090);
                pc = 0x5185b; continue;
            }
            0x5185b => {
                xmm0 = Xmm::default();
                eax = m.read32(esp.wrapping_add(0x30));
                esp = esp.wrapping_add(0x8);
                pc = 0x51865; continue;
            }
            0x51865 => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x51890; continue; }
                pc = 0x51869; continue;
            }
            0x51869 => {
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ebp))).to_bits();
                m.write32(ebp, xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x4)))).to_bits();
                m.write32(ebp.wrapping_add(0x4), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x8)))).to_bits();
                m.write32(ebp.wrapping_add(0x8), xmm1.lanes[0]);
                pc = 0x51890; continue;
            }
            0x51890 => {
                fa = eax;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = m.read32(esp.wrapping_add(0x1b8));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                m.write32(ecx, xmm1.lanes[0]);
                if (fas <= fbs) { pc = 0x51c38; continue; }
                pc = 0x518aa; continue;
            }
            0x518aa => {
                eax = m.read32(esp.wrapping_add(0x198));
                xmm2 = Xmm::scalar(m.read32(eax));
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                xh[0] = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                xmm4 = Xmm::scalar(m.read32(0x4760d0));
                ecx = 0;
                edx = esi.wrapping_add(0x4);
                eax = esi.wrapping_add(0x8);
                pc = 0x518d0; continue;
            }
            0x518d0 => {
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_add(((0x4i32.wrapping_neg()) as u32))));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ebp))).to_bits();
                xh[1] = Xmm::scalar(m.read32(edx.wrapping_add(0x4)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x8)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(edx));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x4)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                if ((fca > fcb)) { pc = 0x51905; continue; }
                pc = 0x51902; continue;
            }
            0x51902 => {
                xmm1 = xmm4;
                pc = 0x51905; continue;
            }
            0x51905 => {
                xh[1] = Xmm::scalar(m.read32(eax.wrapping_add(((0x8i32.wrapping_neg()) as u32))));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x3c)))).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xh[1];
                xh[1] = Xmm::scalar(m.read32(eax.wrapping_add(((0x4i32.wrapping_neg()) as u32))));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x3c)))).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xh[1];
                xh[1] = Xmm::scalar(m.read32(eax));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x3c)))).to_bits();
                ecx = ecx.wrapping_add(1);
                edx = edx.wrapping_add(0x10);
                eax = eax.wrapping_add(0x10);
                fa = ecx;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = xh[1];
                if (fas < fbs) { pc = 0x518d0; continue; }
                pc = 0x51956; continue;
            }
            0x51956 => {
                eax = m.read32(esp.wrapping_add(0x1a4));
                xh[1] = Xmm::scalar(m.read32(eax));
                xh[2] = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                m.write32(esp.wrapping_add(0x44), xh[0].lanes[0]);
                xh[0] = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x40), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x3c), xmm2.lanes[0]);
                ecx = 0;
                edx = edi.wrapping_add(0x4);
                eax = edi.wrapping_add(0x8);
                pc = 0x51990; continue;
                edi = edi;
                pc = 0x51990; continue;
            }
            0x51990 => {
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_add(((0x4i32.wrapping_neg()) as u32))));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ebp))).to_bits();
                xmm2 = Xmm::scalar(m.read32(edx.wrapping_add(0x4)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x8)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(edx));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x4)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                xmm1 = xmm4;
                if ((fca > fcb)) { pc = 0x519c5; continue; }
                pc = 0x519bd; continue;
            }
            0x519bd => {
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                pc = 0x519c5; continue;
            }
            0x519c5 => {
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(((0x8i32.wrapping_neg()) as u32))));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x4c)))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = xmm2;
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(((0x4i32.wrapping_neg()) as u32))));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x4c)))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = xmm2;
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x4c)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                ecx = ecx.wrapping_add(1);
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                edx = edx.wrapping_add(0x10);
                eax = eax.wrapping_add(0x10);
                fa = ecx;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = xmm2;
                if (fas < fbs) { pc = 0x51990; continue; }
                pc = 0x51a16; continue;
            }
            0x51a16 => {
                eax = m.read32(esp.wrapping_add(0x28));
                eax = eax.wrapping_add(0xfffffff9u32);
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                ecx = 0x3;
                { let dividend = ((edx as i32 as i64) << 32) | eax as i64;
                eax = (dividend / (ecx as i32 as i64)) as u32;
                edx = (dividend % (ecx as i32 as i64)) as u32; }
                m.write32(esp.wrapping_add(0xb0), xh[2].lanes[0]);
                xh[2].lanes[0] = sse::subss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x44)))).to_bits();
                m.write32(esp.wrapping_add(0xa0), xh[2].lanes[0]);
                m.write32(esp.wrapping_add(0xac), xh[0].lanes[0]);
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                m.write32(esp.wrapping_add(0xa8), xh[1].lanes[0]);
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                m.write32(esp.wrapping_add(0x98), xh[1].lanes[0]);
                eax = eax.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x8)));
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(eax.wrapping_mul(0x4))));
                xmm4 = Xmm::scalar(m.read32(esi.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x4)));
                eax = edx.wrapping_mul(0x4);
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(eax.wrapping_mul(0x4))));
                m.write32(esp.wrapping_add(0x30), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x34), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x8)));
                m.write32(esp.wrapping_add(0x54), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x38), xmm1.lanes[0]);
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x54)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = xmm4;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = xmm3;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xa0)))).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x98)))).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(0x475c78));
                xh[1] = xmm0;
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = xmm1;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xh[0].lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51b6c; continue; }
                pc = 0x51b37; continue;
            }
            0x51b37 => {
                xh[0] = xmm0;
                xmm2 = xmm0;
                pc = 0x51b93; continue;
            }
            0x51b3f => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x522b8; continue; }
                pc = 0x51b47; continue;
            }
            0x51b47 => {
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x51832; continue; }
                pc = 0x51b4f; continue;
            }
            0x51b4f => {
                ecx = m.read32(edx);
                ebp = m.read32(esp.wrapping_add(0x1b4));
                m.write32(ebp, ecx);
                ecx = m.read32(edx.wrapping_add(0x4));
                m.write32(ebp.wrapping_add(0x4), ecx);
                edx = m.read32(edx.wrapping_add(0x8));
                m.write32(ebp.wrapping_add(0x8), edx);
                pc = 0x51865; continue;
            }
            0x51b6c => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = xh[1];
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                pc = 0x51b93; continue;
            }
            0x51b93 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x54)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                eax = m.read32(esp.wrapping_add(0x1b0));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x44)))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                xh[1] = xmm1;
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xa8)))).to_bits();
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xac)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xb0)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(0x475cd4));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax, xmm1.lanes[0]);
                m.write32(eax.wrapping_add(0x4), xh[0].lanes[0]);
                m.write32(eax.wrapping_add(0x8), xmm0.lanes[0]);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x184);
                break 'run;
            }
            0x51c38 => {
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x51c7e; continue; }
                pc = 0x51c3d; continue;
            }
            0x51c3d => {
                xmm0 = Xmm::scalar(m.read32(ebp));
                edx = m.read32(esp.wrapping_add(0x1a4));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebp.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x58), esi);
                eax = esp.wrapping_add(0x3c);
                esi = edi;
                edi = m.read32(esp.wrapping_add(0x198));
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebp.wrapping_add(0x8)));
                m.write32(esp.wrapping_add(0x48), edx);
                m.write32(esp.wrapping_add(0x64), eax);
                ebx = esp.wrapping_add(0x4c);
                pc = 0x51cc1; continue;
            }
            0x51c7e => {
                ecx = m.read32(esp.wrapping_add(0x198));
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ebp))).to_bits();
                m.write32(esp.wrapping_add(0x30), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x4)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x8)))).to_bits();
                edx = esp.wrapping_add(0x4c);
                m.write32(esp.wrapping_add(0x58), edi);
                edi = m.read32(esp.wrapping_add(0x1a4));
                m.write32(esp.wrapping_add(0x48), ecx);
                m.write32(esp.wrapping_add(0x64), edx);
                ebx = esp.wrapping_add(0x3c);
                m.write32(esp.wrapping_add(0x34), xmm1.lanes[0]);
                pc = 0x51cc1; continue;
            }
            0x51cc1 => {
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                m.write32(esp.wrapping_add(0xa8), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                eax = esp.wrapping_add(0xa8);
                push!(eax);
                m.write32(esp.wrapping_add(0xb0), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                push!(esi);
                m.write32(esp.wrapping_add(0x70), edi);
                m.write32(esp.wrapping_add(0xb8), xmm0.lanes[0]);
                call!(0x1521d0);
                pc = 0x51d06; continue;
            }
            0x51d06 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xb0)));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x1c));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xb4)));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x1c));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xb8)));
                ecx = (ecx & 0x7fffffff);
                edx = (edx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                eax = m.read32(esp.wrapping_add(0x1c));
                m.write32(esp.wrapping_add(0xa0), ecx);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0xa0)));
                m.write32(esp.wrapping_add(0xa4), edx);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xa4)));
                eax = (eax & 0x7fffffff);
                esp = esp.wrapping_add(0x8);
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0xa0), eax);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51d9a; continue; }
                pc = 0x51d7f; continue;
            }
            0x51d7f => {
                ebp = 0;
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0xa0))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51db7; continue; }
                pc = 0x51d8b; continue;
            }
            0x51d8b => {
                eax = 0x1;
                m.write32(esp.wrapping_add(0x1c), 0x2);
                pc = 0x51dc4; continue;
            }
            0x51d9a => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0xa0))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51db5; continue; }
                pc = 0x51da4; continue;
            }
            0x51da4 => {
                eax = 0;
                ebp = 0x1;
                m.write32(esp.wrapping_add(0x1c), 0x2);
                pc = 0x51dc4; continue;
            }
            0x51db5 => {
                ebp = 0;
                pc = 0x51db7; continue;
            }
            0x51db7 => {
                m.write32(esp.wrapping_add(0x1c), 0x1);
                eax = 0x2;
                pc = 0x51dc4; continue;
            }
            0x51dc4 => {
                xmm0 = Xmm::default();
                ecx = m.read32(esp.wrapping_add(0x48));
                eax = eax.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(eax).wrapping_add(0xa8))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x51e22; continue; }
                pc = 0x51dd8; continue;
            }
            0x51dd8 => {
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(ebx)));
                xmm1 = Xmm::scalar(m.read32(ecx));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edi))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(eax.wrapping_mul(0x4))))).to_bits();
                xmm2 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x4)))).to_bits();
                xmm3 = Xmm::scalar(m.read32(ecx.wrapping_add(0x8)));
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x4)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(ebx)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x8)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(ebx)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                pc = 0x51e6a; continue;
            }
            0x51e22 => {
                xmm0 = Xmm::scalar(m.read32(ecx));
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(ebx)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(eax.wrapping_mul(0x4))))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi))).to_bits();
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x4)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(ebx)))).to_bits();
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x8)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(ebx)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x4)))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(ecx.wrapping_add(0x8)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                pc = 0x51e6a; continue;
            }
            0x51e6a => {
                eax = m.read32(esp.wrapping_add(0x28));
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x98), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x9c), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0xa0), xmm2.lanes[0]);
                if (fas > fbs) { pc = 0x51e91; continue; }
                pc = 0x51e8e; continue;
            }
            0x51e8e => {
                eax = eax.wrapping_sub(1);
                pc = 0x51e94; continue;
            }
            0x51e91 => {
                eax = eax.wrapping_add(0xfffffffcu32);
                pc = 0x51e94; continue;
            }
            0x51e94 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x7c), eax);
                m.write32(esp.wrapping_add(0x5c), 0x2);
                if (fa != fb) { pc = 0x51eae; continue; }
                pc = 0x51ea4; continue;
            }
            0x51ea4 => {
                m.write32(esp.wrapping_add(0x6c), 0x1);
                pc = 0x51ec3; continue;
            }
            0x51eae => {
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x6c), 0);
                if (fa == fb) { pc = 0x51ec3; continue; }
                pc = 0x51ebb; continue;
            }
            0x51ebb => {
                m.write32(esp.wrapping_add(0x5c), 0x1);
                pc = 0x51ec3; continue;
            }
            0x51ec3 => {
                edx = m.read32(esp.wrapping_add(0x6c));
                ecx = m.read32(esp.wrapping_add(0x58));
                edi = m.read32(esp.wrapping_add(0x1c));
                xmm4 = xmm0;
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(ecx)))).to_bits();
                xh[0] = Xmm::scalar(m.read32(edx.wrapping_add(ecx).wrapping_add(0x4)));
                eax = edx.wrapping_add(ecx);
                edx = m.read32(esp.wrapping_add(0x5c));
                xmm3 = xmm1;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(edx).wrapping_add(0x4)))).to_bits();
                ecx = ecx.wrapping_add(edx);
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = xmm2;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                edx = ebp;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(ecx));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(esi).wrapping_add(0x4)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(esi)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(esi).wrapping_add(0x8)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                edx = edx.wrapping_add(esi);
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                edi = edi.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                xh[1] = Xmm::scalar(m.read32(edi.wrapping_add(esi).wrapping_add(0x4)));
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(esi)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(edi.wrapping_add(esi).wrapping_add(0x8)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(esi).wrapping_add(0x8)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                edi = edi.wrapping_add(esi);
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(ecx));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx))).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_add(0x8)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x4)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(ecx));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                edi = ebp.wrapping_mul(0x4);
                ebp = m.read32(esp.wrapping_add(0x1c));
                m.write32(esp.wrapping_add(0x24), xmm2.lanes[0]);
                ebp = ebp.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(ebx.wrapping_add(ebp)))).to_bits();
                m.write32(esp.wrapping_add(0x1c), xmm2.lanes[0]);
                xmm0 = xh[0];
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(ebx)))).to_bits();
                xmm2 = xmm3;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm2.lanes[0]);
                xh[2] = xh[1];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(ebx.wrapping_add(ebp)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xb4), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x58), xmm1.lanes[0]);
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(ebx)))).to_bits();
                xmm2 = xmm4;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x60), ebp);
                m.write32(esp.wrapping_add(0x48), xh[2].lanes[0]);
                m.write32(esp.wrapping_add(0x2c), xmm2.lanes[0]);
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                eax = m.read32(esp.wrapping_add(0x64));
                ecx = m.read32(esp.wrapping_add(0x6c));
                edx = m.read32(esp.wrapping_add(0x5c));
                m.write32(esp.wrapping_add(0xb8), xmm2.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                m.write32(esp.wrapping_add(0xbc), xmm2.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xc0), xh[2].lanes[0]);
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x48)))).to_bits();
                m.write32(esp.wrapping_add(0xcc), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(ecx.wrapping_mul(0x4))));
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xc4), xh[2].lanes[0]);
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(edx.wrapping_mul(0x4))));
                eax = esp.wrapping_add(0xf4);
                xh[2] = xmm2;
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                push!(eax);
                ecx = esp.wrapping_add(0x18);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                push!(ecx);
                edx = esp.wrapping_add(0xbc);
                m.write32(esp.wrapping_add(0xd0), xh[2].lanes[0]);
                m.write32(esp.wrapping_add(0xd8), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                call!(0x4ba10);
                pc = 0x520ed; continue;
            }
            0x520ed => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x14), eax);
                if (fas < fbs) { pc = 0x522b8; continue; }
                pc = 0x520fd; continue;
            }
            0x520fd => {
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x58)));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0 = xmm2;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xh[2] = xmm0;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = xmm0;
                xmm1 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                ebx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x58), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x48), ebx);
                if (fas <= fbs) { pc = 0x522b8; continue; }
                pc = 0x52159; continue;
            }
            0x52159 => {
                edx = esp.wrapping_add(0x138);
                ecx = edx;
                pc = 0x52170; continue;
                pc = 0x52170; continue;
            }
            0x52170 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(ebx.wrapping_mul(0x8)).wrapping_add(0xf8)));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xh[1] = xmm0;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                eax = esp.wrapping_add(0xf4);
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(ebx.wrapping_mul(0x8))));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x58)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                eax = m.read32(esp.wrapping_add(0x64));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(ebp.wrapping_mul(0x4))))).to_bits();
                xh[1] = xmm1;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(edi.wrapping_mul(0x4))))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x98)))).to_bits();
                m.write32(ecx.wrapping_add(((0x4i32.wrapping_neg()) as u32)), xmm2.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(ebp.wrapping_mul(0x4)).wrapping_add(0x4)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(ebp.wrapping_mul(0x4)).wrapping_add(0x8)))).to_bits();
                ebp = m.read32(esp.wrapping_add(0x7c));
                xh[1] = xmm1;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(edi.wrapping_mul(0x4)).wrapping_add(0x4)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x9c)))).to_bits();
                m.write32(ecx, xmm2.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(edi.wrapping_mul(0x4)).wrapping_add(0x8)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xa0)))).to_bits();
                m.write32(ecx.wrapping_add(0x4), xmm2.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_add(0x4)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x38)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(((0x4i32.wrapping_neg()) as u32))));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(ebp.wrapping_mul(0x4))));
                eax = m.read32(esp.wrapping_add(0xa4));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::default();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                m.write32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0xb4), xmm0.lanes[0]);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x5229c; continue; }
                pc = 0x5226d; continue;
            }
            0x5226d => {
                ebp = esp.wrapping_add(0xf4);
                ebx = m.read32(ebp.wrapping_add(ebx.wrapping_mul(0x8)));
                m.write32(ebp.wrapping_add(eax.wrapping_mul(0x8)), ebx);
                ebx = m.read32(esp.wrapping_add(0x48));
                ebp = m.read32(esp.wrapping_add(ebx.wrapping_mul(0x8)).wrapping_add(0xf8));
                m.write32(esp.wrapping_add(eax.wrapping_mul(0x8)).wrapping_add(0xf8), ebp);
                eax = eax.wrapping_add(1);
                ecx = ecx.wrapping_add(0xc);
                m.write32(esp.wrapping_add(0xa4), eax);
                edx = edx.wrapping_add(0xc);
                pc = 0x5229c; continue;
            }
            0x5229c => {
                ebp = m.read32(esp.wrapping_add(0x60));
                ebx = ebx.wrapping_add(1);
                fa = ebx;
                fb = m.read32(esp.wrapping_add(0x14));
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x48), ebx);
                if (fas < fbs) { pc = 0x52170; continue; }
                pc = 0x522af; continue;
            }
            0x522af => {
                ecx = 0x1;
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x522c5; continue; }
                pc = 0x522b8; continue;
            }
            0x522b8 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x184);
                break 'run;
            }
            0x522c5 => {
                esi = m.read32(esp.wrapping_add(0x1bc));
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x522d9; continue; }
                pc = 0x522d0; continue;
            }
            0x522d0 => {
                m.write32(esp.wrapping_add(0x1bc), eax);
                esi = eax;
                pc = 0x522d9; continue;
            }
            0x522d9 => {
                fa = esi;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x522e6; continue; }
                pc = 0x522dd; continue;
            }
            0x522dd => {
                m.write32(esp.wrapping_add(0x1bc), ecx);
                esi = ecx;
                pc = 0x522e6; continue;
            }
            0x522e6 => {
                fa = eax;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x5238f; continue; }
                pc = 0x522ee; continue;
            }
            0x522ee => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x524e0; continue; }
                pc = 0x522f6; continue;
            }
            0x522f6 => {
                ecx = m.read32(esp.wrapping_add(0x1c0));
                esi = m.read32(esp.wrapping_add(0x1b8));
                edi = m.read32(esp.wrapping_add(0x68));
                edx = ecx.wrapping_mul(0x4);
                ebx = esp.wrapping_add(0xb4);
                m.write32(esp.wrapping_add(0x14), edx);
                edx = m.read32(esp.wrapping_add(0x1b0));
                ecx = esp.wrapping_add(0x138);
                ebx = ebx.wrapping_sub(esi);
                m.write32(esp.wrapping_add(0xa4), eax);
                pc = 0x52331; continue;
            }
            0x52331 => {
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(((0x4i32.wrapping_neg()) as u32))));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi))).to_bits();
                ebp = m.read32(esp.wrapping_add(0x14));
                m.write32(edx, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                m.write32(edx.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                m.write32(edx.wrapping_add(0x8), xmm0.lanes[0]);
                xmm0 = xmm1;
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ebx.wrapping_add(esi)))).to_bits();
                edx = edx.wrapping_add(ebp);
                ebp = m.read32(esp.wrapping_add(0xa4));
                m.write32(esi, xmm0.lanes[0]);
                ecx = ecx.wrapping_add(0xc);
                esi = esi.wrapping_add(0x4);
                ebp = ebp.wrapping_sub(1);
                fa = ebp;
                m.write32(esp.wrapping_add(0xa4), ebp);
                if ((fa != 0)) { pc = 0x52331; continue; }
                pc = 0x52384; continue;
            }
            0x52384 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x184);
                break 'run;
            }
            0x5238f => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xb4)));
                edi = eax.wrapping_add(((0x1i32.wrapping_neg()) as u32));
                edx = 0;
                fa = edi;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x52406; continue; }
                pc = 0x523a2; continue;
            }
            0x523a2 => {
                edi = eax.wrapping_add(((0x3i32.wrapping_neg()) as u32));
                pc = 0x523b0; continue;
                edi = edi;
                pc = 0x523b0; continue;
            }
            0x523b0 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0xb4)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x523c3; continue; }
                pc = 0x523be; continue;
            }
            0x523be => {
                xmm0 = xmm1;
                edx = ecx;
                pc = 0x523c3; continue;
            }
            0x523c3 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0xb8)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x523d7; continue; }
                pc = 0x523d1; continue;
            }
            0x523d1 => {
                xmm0 = xmm1;
                edx = ecx.wrapping_add(0x1);
                pc = 0x523d7; continue;
            }
            0x523d7 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0xbc)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x523eb; continue; }
                pc = 0x523e5; continue;
            }
            0x523e5 => {
                xmm0 = xmm1;
                edx = ecx.wrapping_add(0x2);
                pc = 0x523eb; continue;
            }
            0x523eb => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0xc0)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x523ff; continue; }
                pc = 0x523f9; continue;
            }
            0x523f9 => {
                xmm0 = xmm1;
                edx = ecx.wrapping_add(0x3);
                pc = 0x523ff; continue;
            }
            0x523ff => {
                ecx = ecx.wrapping_add(0x4);
                fa = ecx;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x523b0; continue; }
                pc = 0x52406; continue;
            }
            0x52406 => {
                fa = ecx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x52428; continue; }
                pc = 0x5240a; continue;
            }
            0x5240a => {
                pc = 0x52410; continue;
            }
            0x52410 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0xb4)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x52423; continue; }
                pc = 0x5241e; continue;
            }
            0x5241e => {
                xmm0 = xmm1;
                edx = ecx;
                pc = 0x52423; continue;
            }
            0x52423 => {
                ecx = ecx.wrapping_add(1);
                fa = ecx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x52410; continue; }
                pc = 0x52428; continue;
            }
            0x52428 => {
                ecx = esp.wrapping_add(0xd4);
                push!(ecx);
                push!(edx);
                push!(esi);
                edx = esp.wrapping_add(0x100);
                push!(edx);
                push!(eax);
                call!(0x50170);
                pc = 0x52440; continue;
            }
            0x52440 => {
                esp = esp.wrapping_add(0x14);
                ecx = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x524de; continue; }
                pc = 0x5244d; continue;
            }
            0x5244d => {
                ebp = m.read32(esp.wrapping_add(0x1c0));
                ebx = m.read32(esp.wrapping_add(0x1b8));
                esi = m.read32(esp.wrapping_add(0x1b0));
                edi = m.read32(esp.wrapping_add(0x68));
                eax = esp.wrapping_add(0xd4);
                ebp = ebp.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                ebx = ebx.wrapping_sub(eax);
                pc = 0x52472; continue;
            }
            0x52472 => {
                edx = m.read32(esp.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0xd4));
                eax = edx.wrapping_add(edx.wrapping_mul(0x2));
                eax = eax.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(eax).wrapping_add(0x134)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi))).to_bits();
                m.write32(esi, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(eax).wrapping_add(0x138)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x4)))).to_bits();
                m.write32(esi.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(eax).wrapping_add(0x13c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                eax = m.read32(esp.wrapping_add(0x1bc));
                m.write32(esi.wrapping_add(0x8), xmm0.lanes[0]);
                xmm0 = Xmm::default();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0xb4)))).to_bits();
                edx = ebx.wrapping_add(ecx.wrapping_mul(0x4));
                esi = esi.wrapping_add(ebp);
                ecx = ecx.wrapping_add(1);
                fa = ecx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(edx).wrapping_add(0xd4), xmm0.lanes[0]);
                if (fas < fbs) { pc = 0x52472; continue; }
                pc = 0x524dc; continue;
            }
            0x524dc => {
                esi = eax;
                pc = 0x524de; continue;
            }
            0x524de => {
                eax = esi;
                pc = 0x524e0; continue;
            }
            0x524e0 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x184);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000529d0: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f529d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x529d0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x529d0 => {
                esp = esp.wrapping_sub(0x28);
                eax = m.read32(esp.wrapping_add(0x3c));
                edx = m.read32(esp.wrapping_add(0x38));
                ecx = m.read32(esp.wrapping_add(0x34));
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx))).to_bits();
                xmm1 = Xmm::scalar(m.read32(edx));
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0x10)));
                xmm4 = Xmm::scalar(m.read32(eax.wrapping_add(0x14)));
                xh[2] = Xmm::scalar(m.read32(eax.wrapping_add(0x18)));
                xh[0] = Xmm::default();
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x18)));
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x10)));
                m.write32(esp, xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x14)));
                eax = m.read32(esp.wrapping_add(0x2c));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                ecx = m.read32(esp.wrapping_add(0x30));
                xh[1] = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                xh[0] = Xmm::scalar(m.read32(ecx));
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm2.lanes[0]);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                m.write32(esp.wrapping_add(0x18), xh[2].lanes[0]);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = xh[2];
                xh[2] = xmm2;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x8), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xc)))).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xh[1];
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x8)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x38), xh[2].lanes[0]);
                xmm2 = xh[0];
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xh[2] = xh[1];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x34), xmm2.lanes[0]);
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xc)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm4.lanes[0]);
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm3.lanes[0]);
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                m.write32(esp.wrapping_add(0x3c), xmm2.lanes[0]);
                xmm2 = xmm4;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x8)))).to_bits();
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xmm3;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4)))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x3c), xh[2].lanes[0]);
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm4;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x38)))).to_bits();
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::default();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x52ba6; continue; }
                pc = 0x52ba1; continue;
            }
            0x52ba1 => {
                xmm4 = xmm0;
                pc = 0x52bcf; continue;
            }
            0x52ba6 => {
                xmm2 = xmm0;
                xmm0 = xh[2];
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x38)))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = xh[2];
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                pc = 0x52bcf; continue;
            }
            0x52bcf => {
                xmm0 = Xmm::default();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x30), xmm3.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x52bf4; continue; }
                pc = 0x52bdd; continue;
            }
            0x52bdd => {
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                xmm2 = xmm0;
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                pc = 0x52c1a; continue;
            }
            0x52bf4 => {
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x52c14; continue; }
                pc = 0x52bf9; continue;
            }
            0x52bf9 => {
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm2 = xmm3;
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                pc = 0x52c1a; continue;
            }
            0x52c14 => {
                m.write32(esp.wrapping_add(0x3c), xmm3.lanes[0]);
                pc = 0x52c1a; continue;
            }
            0x52c1a => {
                xmm0 = Xmm::default();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm4.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x52c46; continue; }
                pc = 0x52c22; continue;
            }
            0x52c22 => {
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                xmm4 = xmm0;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x52c36; continue; }
                pc = 0x52c31; continue;
            }
            0x52c31 => {
                xmm2 = xmm0;
                pc = 0x52c79; continue;
            }
            0x52c36 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x52c73; continue; }
                pc = 0x52c41; continue;
            }
            0x52c41 => {
                xmm2 = xmm3;
                pc = 0x52c79; continue;
            }
            0x52c46 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x52c79; continue; }
                pc = 0x52c51; continue;
            }
            0x52c51 => {
                xmm2 = Xmm::default();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                xmm4 = xmm0;
                if ((fca > fcb)) { pc = 0x52c79; continue; }
                pc = 0x52c60; continue;
            }
            0x52c60 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x52c70; continue; }
                pc = 0x52c6b; continue;
            }
            0x52c6b => {
                xmm2 = xmm3;
                pc = 0x52c79; continue;
            }
            0x52c70 => {
                xmm2 = xmm1;
                pc = 0x52c73; continue;
            }
            0x52c73 => {
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                pc = 0x52c79; continue;
            }
            0x52c79 => {
                xmm2.lanes[0] = sse::divss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                xmm4.lanes[0] = sse::divss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                xmm1 = xmm2;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xmm3 = xmm1;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xc)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x8)))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                m.write32(esp.wrapping_add(0x18), xmm1.lanes[0]);
                xmm0 = xmm4;
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                xh[2] = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm4 = xmm2;
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xh[0] = xmm3;
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0 = xh[1];
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm1 = xh[0];
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm4;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x52dd3; continue; }
                pc = 0x52d23; continue;
            }
            0x52d23 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x4765f4)) as f64);
                eax = m.read32(esp.wrapping_add(0x44));
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x52d7c; continue; }
                pc = 0x52d30; continue;
            }
            0x52d30 => {
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                xmm0.lanes[0] = reciprocal_sqrt(f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x38)))).to_bits();
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(eax, xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(eax.wrapping_add(0x4), xmm1.lanes[0]);
                m.write32(eax.wrapping_add(0x8), xmm0.lanes[0]);
                pc = 0x52d7c; continue;
            }
            0x52d7c => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x30))) as f64));
                xmm0 = Xmm::scalar(m.read32(edx));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                ecx = m.read32(esp.wrapping_add(0x40));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(ecx, xmm2.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx))).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                m.write32(ecx.wrapping_add(0x4), xmm3.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edx))).to_bits();
                eax = m.read32(esp.wrapping_add(0x48));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(ecx.wrapping_add(0x8), xmm0.lanes[0]);
                { let value = round_pc(((f32::from_bits(m.read32(edx)) as f64) - fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.stack[fpu.top as usize] = -fpu.st(0);
                m.write32(eax, (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = 0x1;
                esp = esp.wrapping_add(0x28);
                break 'run;
            }
            0x52dd3 => {
                eax = 0;
                esp = esp.wrapping_add(0x28);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000554c0: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f554c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut fp_cmp = 0i32;
    let mut pc: u32 = 0x554c0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x554c0 => {
                esp = esp.wrapping_sub(0x5c);
                edx = m.read32(esp.wrapping_add(0x74));
                eax = ((m.read16(edx.wrapping_add(0x2a))) as u16 as u32);
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x1c) as i32)) as u32;
                xmm1 = Xmm::scalar(m.read32(ecx.wrapping_add(edx).wrapping_add(0x2c)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x38)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(edx).wrapping_add(0x28)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x34)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(edx).wrapping_add(0x24)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x30)))).to_bits();
                push!(edi);
                edi = ecx.wrapping_add(edx).wrapping_add(0x24);
                ecx = m.read32(esp.wrapping_add(0x7c));
                xmm2 = Xmm::scalar(m.read32(ecx.wrapping_add(0x8)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x38)))).to_bits();
                xmm3 = Xmm::scalar(m.read32(ecx.wrapping_add(0x18)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x38)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x34)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(ecx));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x30)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(ecx.wrapping_add(0x10)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x30)))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(edx.wrapping_add(0x34)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x14)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(ecx.wrapping_add(0xc))) as f64);
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xc), eax);
                m.write32(esp.wrapping_add(0x18), edi);
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x34), xmm3.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55575; continue; }
                pc = 0x5556f; continue;
            }
            0x5556f => {
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(ecx.wrapping_add(0xc))) as f64);
                if ((fca > fcb)) { pc = 0x55582; continue; }
                pc = 0x55575; continue;
            }
            0x55575 => {
                xmm1 = Xmm::default();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55589; continue; }
                pc = 0x5557d; continue;
            }
            0x5557d => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55589; continue; }
                pc = 0x55582; continue;
            }
            0x55582 => {
                eax = 0;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x5c);
                break 'run;
            }
            0x55589 => {
                xmm2 = Xmm::scalar(m.read32(0x478618));
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x74));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x74));
                m.write32(esi, xmm2.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                ebx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x84), ebx);
                m.write32(esp.wrapping_add(0x2c), xmm2.lanes[0]);
                if (fa <= fb) { pc = 0x559a1; continue; }
                pc = 0x555c0; continue;
            }
            0x555c0 => {
                esi = m.read32(esp.wrapping_add(0x70));
                esi = esi.wrapping_add(0x8);
                ebx = edx.wrapping_add(0x40);
                m.write32(esp.wrapping_add(0x1c), esi);
                m.write32(esp.wrapping_add(0x20), eax);
                pc = 0x555d2; continue;
            }
            0x555d2 => {
                xmm0 = Xmm::scalar(m.read32(ebx));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi))).to_bits();
                xh[0] = Xmm::scalar(m.read32(ecx.wrapping_add(0x14)));
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                xmm4 = Xmm::scalar(m.read32(ecx.wrapping_add(0x18)));
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xh[1] = Xmm::scalar(m.read32(ecx.wrapping_add(0x10)));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                m.write32(esp.wrapping_add(0x48), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x4)))).to_bits();
                xmm3 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x4c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                xmm1 = Xmm::scalar(m.read32(ecx));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edi))).to_bits();
                xmm2 = xmm4;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x50), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                m.write32(esp.wrapping_add(0x5c), xmm4.lanes[0]);
                xh[2] = xh[1];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x4c)));
                m.write32(esp.wrapping_add(0x88), xmm2.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = xh[2];
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                m.write32(esp.wrapping_add(0x58), xh[0].lanes[0]);
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                xh[0] = xh[2];
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = xmm0;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x5c)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x50)))).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = xmm3;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x58)))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4c)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x28), xh[0].lanes[0]);
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = xmm1;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x48)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm0;
                xmm3 = xmm4;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x88)))).to_bits();
                xmm0 = xmm2;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::default();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                if ((fca >= fcb)) { pc = 0x55749; continue; }
                pc = 0x55723; continue;
            }
            0x55723 => {
                xmm0 = xmm1;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x88)))).to_bits();
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                pc = 0x55749; continue;
            }
            0x55749 => {
                xh[2] = Xmm::default();
                fca = f32::from_bits(xh[2].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x10), xmm3.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55762; continue; }
                pc = 0x55757; continue;
            }
            0x55757 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0 = xh[2];
                pc = 0x55779; continue;
            }
            0x55762 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55776; continue; }
                pc = 0x55767; continue;
            }
            0x55767 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0 = xmm3;
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                pc = 0x55779; continue;
            }
            0x55776 => {
                xmm4 = xmm3;
                pc = 0x55779; continue;
            }
            0x55779 => {
                fca = f32::from_bits(xh[2].lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x557ad; continue; }
                pc = 0x5577e; continue;
            }
            0x5577e => {
                xmm0 = xh[2];
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                fca = f32::from_bits(xh[2].lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                xmm1 = xh[2];
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55792; continue; }
                pc = 0x5578d; continue;
            }
            0x5578d => {
                xmm0 = xh[2];
                pc = 0x557df; continue;
            }
            0x55792 => {
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x88)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x557a5; continue; }
                pc = 0x557a0; continue;
            }
            0x557a0 => {
                xmm0 = xmm3;
                pc = 0x557df; continue;
            }
            0x557a5 => {
                m.write32(esp.wrapping_add(0x10), xmm2.lanes[0]);
                pc = 0x557df; continue;
            }
            0x557ad => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm4.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x557df; continue; }
                pc = 0x557b2; continue;
            }
            0x557b2 => {
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                fca = f32::from_bits(xh[2].lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                xmm1 = xmm4;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x557c3; continue; }
                pc = 0x557be; continue;
            }
            0x557be => {
                xmm0 = xh[2];
                pc = 0x557df; continue;
            }
            0x557c3 => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x88)));
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xh[0].lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x557d6; continue; }
                pc = 0x557d1; continue;
            }
            0x557d1 => {
                xmm0 = xmm3;
                pc = 0x557df; continue;
            }
            0x557d6 => {
                xmm0 = xmm2;
                m.write32(esp.wrapping_add(0x10), xh[0].lanes[0]);
                pc = 0x557df; continue;
            }
            0x557df => {
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                xh[0] = xmm0;
                xmm1.lanes[0] = sse::divss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm3 = xmm1;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x48)))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(edi))).to_bits();
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                xh[1] = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x5c)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x58)))).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                xmm4 = xmm0;
                xmm0 = xmm1;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x50)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4c)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x4)))).to_bits();
                xmm2 = xh[0];
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x68), xmm4.lanes[0]);
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3 = xh[1];
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = xmm4;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1 = xmm3;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm2;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55977; continue; }
                pc = 0x55880; continue;
            }
            0x55880 => {
                esi = m.read32(esp.wrapping_add(0x84));
                fa = esi;
                fb = m.read32(esp.wrapping_add(0x7c));
                if (fa >= fb) { pc = 0x55977; continue; }
                pc = 0x55891; continue;
            }
            0x55891 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x4765f4)) as f64);
                esi = m.read32(edx.wrapping_add(0x30));
                m.write32(ebp, esi);
                esi = m.read32(edx.wrapping_add(0x34));
                m.write32(ebp.wrapping_add(0x4), esi);
                esi = m.read32(edx.wrapping_add(0x38));
                m.write32(ebp.wrapping_add(0x8), esi);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x558f3; continue; }
                pc = 0x558ac; continue;
            }
            0x558ac => {
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0.lanes[0] = reciprocal_sqrt(f32::from_bits(m.read32(esp.wrapping_add(0x34)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xmm1;
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm3 = xmm1;
                xmm4 = xmm0;
                pc = 0x558f3; continue;
            }
            0x558f3 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x3c))) as f64));
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                esi = m.read32(esp.wrapping_add(0x1c));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esi.wrapping_add(((0x8i32.wrapping_neg()) as u32)), xh[0].lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x68)));
                m.write32(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)), xh[1].lanes[0]);
                xmm1 = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esi, xmm0.lanes[0]);
                { let value = round_pc(((f32::from_bits(m.read32(ecx.wrapping_add(0xc))) as f64) - fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.stack[fpu.top as usize] = -fpu.st(0);
                m.write32(esp.wrapping_add(0x1c), (fpu.st(0) as f32).to_bits());
                fpu.push((f32::from_bits(m.read32(0x475cac)) as f64));
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if fp_cmp != 1 { pc = 0x00055960; continue; }
                pc = 0x5594d; continue;
            }
            0x5594d => {
                edi = m.read32(esp.wrapping_add(0x74));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(edi)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55960; continue; }
                pc = 0x5595c; continue;
            }
            0x5595c => {
                m.write32(edi, xmm0.lanes[0]);
                pc = 0x55960; continue;
            }
            0x55960 => {
                m.write32(esp.wrapping_add(0x84), m.read32(esp.wrapping_add(0x84)).wrapping_add(0x1));
                edi = m.read32(esp.wrapping_add(0x80));
                edi = edi.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = esi.wrapping_add(edi);
                m.write32(esp.wrapping_add(0x1c), esi);
                pc = 0x55977; continue;
            }
            0x55977 => {
                esi = m.read32(esp.wrapping_add(0x20));
                edi = ebx;
                ebx = ebx.wrapping_add(0x1c);
                esi = esi.wrapping_sub(1);
                fa = esi;
                m.write32(esp.wrapping_add(0x20), esi);
                if ((fa != 0)) { pc = 0x555d2; continue; }
                pc = 0x5598b; continue;
            }
            0x5598b => {
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
                xmm1 = Xmm::default();
                ebx = m.read32(esp.wrapping_add(0x84));
                pc = 0x559a1; continue;
            }
            0x559a1 => {
                edi = (((((m.read8(edx.wrapping_add(0x3d)) as i8)) as i32)) as u32);
                esi = (((((m.read8(edx.wrapping_add(0x3c)) as i8)) as i32)) as u32);
                m.write32(esp.wrapping_add(0x20), edi);
                fa = ebx;
                fb = m.read32(esp.wrapping_add(0x7c));
                m.write32(esp.wrapping_add(0x40), esi);
                if (fa >= fb) { pc = 0x55d10; continue; }
                pc = 0x559bb; continue;
            }
            0x559bb => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55b71; continue; }
                pc = 0x559c4; continue;
            }
            0x559c4 => {
                xmm2 = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55b71; continue; }
                pc = 0x559d2; continue;
            }
            0x559d2 => {
                xmm2 = xmm3;
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d1c))).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55b71; continue; }
                pc = 0x559e6; continue;
            }
            0x559e6 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm4 = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x30)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(ecx));
                xh[0] = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x34)))).to_bits();
                xh[1] = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x38)))).to_bits();
                edi = m.read32(esp.wrapping_add(0x24));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = Xmm::scalar(m.read32(ecx.wrapping_add(0x8)));
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x54), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x58), xmm4.lanes[0]);
                m.write32(esp.wrapping_add(0x5c), xh[0].lanes[0]);
                m.write32(esp.wrapping_add(0x88), 0);
                if (fa <= fb) { pc = 0x55b05; continue; }
                pc = 0x55a47; continue;
            }
            0x55a47 => {
                ebx = m.read32(esp.wrapping_add(0x20));
                ebp = esi.wrapping_mul(0x4);
                ebx = ebx.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = edx.wrapping_add(0x40);
                esi = ebp;
                esi = esi.wrapping_sub(ebx);
                m.write32(esp.wrapping_add(0x10), eax);
                eax = ebx.wrapping_add(edx).wrapping_add(0x40);
                m.write32(esp.wrapping_add(0x14), esi);
                pc = 0x55a80; continue;
            }
            0x55a6a => {
                eax = m.read32(esp.wrapping_add(0x44));
                esi = m.read32(esp.wrapping_add(0x14));
                pc = 0x55a80; continue;
                pc = 0x55a80; continue;
            }
            0x55a80 => {
                xh[0] = Xmm::scalar(m.read32(edi.wrapping_add(ebp)));
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(eax)))).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(ebx).wrapping_add(0x54)));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(ebx.wrapping_add(edi)))).to_bits();
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(ebp).wrapping_add(0x54)));
                xh[2].lanes[0] = sse::subss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(ebp)))).to_bits();
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(eax));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(ebx.wrapping_add(edi)))).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x55b62; continue; }
                pc = 0x55abe; continue;
            }
            0x55abe => {
                esi = m.read32(esp.wrapping_add(0x88));
                edi = m.read32(esp.wrapping_add(0x10));
                esi = esi.wrapping_add(1);
                m.write32(esp.wrapping_add(0x88), esi);
                esi = edi;
                esi = esi.wrapping_add(0x1c);
                eax = eax.wrapping_add(0x1c);
                m.write32(esp.wrapping_add(0x10), esi);
                esi = m.read32(esp.wrapping_add(0x88));
                m.write32(esp.wrapping_add(0x44), eax);
                eax = m.read32(esp.wrapping_add(0x18));
                fa = esi;
                fb = eax;
                if (fa < fb) { pc = 0x55a6a; continue; }
                pc = 0x55af4; continue;
            }
            0x55af4 => {
                xh[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x5c)));
                ebp = m.read32(esp.wrapping_add(0x78));
                ebx = m.read32(esp.wrapping_add(0x84));
                pc = 0x55b05; continue;
            }
            0x55b05 => {
                esi = m.read32(edx.wrapping_add(0x30));
                edi = m.read32(esp.wrapping_add(0x70));
                m.write32(ebp, esi);
                esi = m.read32(edx.wrapping_add(0x34));
                m.write32(ebp.wrapping_add(0x4), esi);
                esi = m.read32(edx.wrapping_add(0x38));
                m.write32(ebp.wrapping_add(0x8), esi);
                esi = ebx;
                esi = ((esi as i32).wrapping_mul((m.read32(esp.wrapping_add(0x80))) as i32)) as u32;
                edi = edi.wrapping_add(esi.wrapping_mul(0x4));
                m.write32(edi, xmm2.lanes[0]);
                m.write32(edi.wrapping_add(0x4), xmm4.lanes[0]);
                m.write32(edi.wrapping_add(0x8), xh[0].lanes[0]);
                xmm4 = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55b58; continue; }
                pc = 0x55b4b; continue;
            }
            0x55b4b => {
                esi = m.read32(esp.wrapping_add(0x74));
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55b58; continue; }
                pc = 0x55b54; continue;
            }
            0x55b54 => {
                m.write32(esi, xmm2.lanes[0]);
                pc = 0x55b58; continue;
            }
            0x55b58 => {
                ebx = ebx.wrapping_add(1);
                m.write32(esp.wrapping_add(0x84), ebx);
                pc = 0x55b71; continue;
            }
            0x55b62 => {
                ebp = m.read32(esp.wrapping_add(0x78));
                eax = m.read32(esp.wrapping_add(0x18));
                ebx = m.read32(esp.wrapping_add(0x84));
                pc = 0x55b71; continue;
            }
            0x55b71 => {
                fa = ebx;
                fb = m.read32(esp.wrapping_add(0x7c));
                if (fa >= fb) { pc = 0x55d10; continue; }
                pc = 0x55b7b; continue;
            }
            0x55b7b => {
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55d10; continue; }
                pc = 0x55b84; continue;
            }
            0x55b84 => {
                xmm2 = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55d10; continue; }
                pc = 0x55b92; continue;
            }
            0x55b92 => {
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d1c))).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55d10; continue; }
                pc = 0x55ba3; continue;
            }
            0x55ba3 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x30)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x10)));
                xmm4 = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x34)))).to_bits();
                xh[0] = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x38)))).to_bits();
                esi = m.read32(esp.wrapping_add(0x24));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(ecx.wrapping_add(0x14)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(ecx.wrapping_add(0x18)));
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x54), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x58), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x5c), xmm4.lanes[0]);
                m.write32(esp.wrapping_add(0x88), 0);
                if (fa <= fb) { pc = 0x55cab; continue; }
                pc = 0x55c00; continue;
            }
            0x55c00 => {
                ebx = m.read32(esp.wrapping_add(0x40));
                edi = m.read32(esp.wrapping_add(0x20));
                ebx = ebx.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                edi = edi.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = edx.wrapping_add(0x40);
                ebp = ebx;
                ebp = ebp.wrapping_sub(edi);
                m.write32(esp.wrapping_add(0x10), eax);
                eax = edi.wrapping_add(edx).wrapping_add(0x40);
                m.write32(esp.wrapping_add(0x14), ebp);
                pc = 0x55c30; continue;
            }
            0x55c23 => {
                eax = m.read32(esp.wrapping_add(0x44));
                ebp = m.read32(esp.wrapping_add(0x14));
                pc = 0x55c30; continue;
            }
            0x55c30 => {
                xh[0] = Xmm::scalar(m.read32(ebx.wrapping_add(esi)));
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(ebp)))).to_bits();
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(edi).wrapping_add(0x54)));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(esi)))).to_bits();
                xh[2] = Xmm::scalar(m.read32(esp.wrapping_add(ebx).wrapping_add(0x54)));
                xh[2].lanes[0] = sse::subss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(ebx.wrapping_add(esi)))).to_bits();
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xh[1] = Xmm::scalar(m.read32(eax));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(esi)))).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                fca = f32::from_bits(xh[0].lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x55d09; continue; }
                pc = 0x55c6e; continue;
            }
            0x55c6e => {
                ebp = m.read32(esp.wrapping_add(0x88));
                esi = m.read32(esp.wrapping_add(0x10));
                ebp = ebp.wrapping_add(1);
                m.write32(esp.wrapping_add(0x88), ebp);
                ebp = esi;
                ebp = ebp.wrapping_add(0x1c);
                eax = eax.wrapping_add(0x1c);
                m.write32(esp.wrapping_add(0x10), ebp);
                ebp = m.read32(esp.wrapping_add(0x18));
                m.write32(esp.wrapping_add(0x44), eax);
                eax = m.read32(esp.wrapping_add(0x88));
                fa = eax;
                fb = ebp;
                if (fa < fb) { pc = 0x55c23; continue; }
                pc = 0x55ca0; continue;
            }
            0x55ca0 => {
                ebp = m.read32(esp.wrapping_add(0x78));
                ebx = m.read32(esp.wrapping_add(0x84));
                pc = 0x55cab; continue;
            }
            0x55cab => {
                eax = m.read32(edx.wrapping_add(0x30));
                m.write32(ebp, eax);
                eax = m.read32(edx.wrapping_add(0x34));
                m.write32(ebp.wrapping_add(0x4), eax);
                edx = m.read32(edx.wrapping_add(0x38));
                eax = ebx;
                eax = ((eax as i32).wrapping_mul((m.read32(esp.wrapping_add(0x80))) as i32)) as u32;
                m.write32(ebp.wrapping_add(0x8), edx);
                edx = m.read32(esp.wrapping_add(0x70));
                edx = edx.wrapping_add(eax.wrapping_mul(0x4));
                m.write32(edx, xmm0.lanes[0]);
                m.write32(edx.wrapping_add(0x4), xmm2.lanes[0]);
                m.write32(edx.wrapping_add(0x8), xmm4.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0 = xmm1;
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55cfe; continue; }
                pc = 0x55cf1; continue;
            }
            0x55cf1 => {
                eax = m.read32(esp.wrapping_add(0x74));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(eax)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x55cfe; continue; }
                pc = 0x55cfa; continue;
            }
            0x55cfa => {
                m.write32(eax, xmm0.lanes[0]);
                pc = 0x55cfe; continue;
            }
            0x55cfe => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = ebx.wrapping_add(1);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebx;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x5c);
                break 'run;
            }
            0x55d09 => {
                ebx = m.read32(esp.wrapping_add(0x84));
                pc = 0x55d10; continue;
            }
            0x55d10 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebx;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x5c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00092a10: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f92a10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut join_zero = false;
    let mut pc: u32 = 0x92a10;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x92a10 => {
                esp = esp.wrapping_sub(0xd4);
                eax = m.read32(esp.wrapping_add(0xe4));
                fa = ((((m.read16(eax.wrapping_add(0x6)))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                ecx = m.read32(esp.wrapping_add(0xd8));
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                m.write32(esp.wrapping_add(0x10), 0);
                if (fa == fb) { pc = 0x92ca7; continue; }
                pc = 0x92a3b; continue;
            }
            0x92a3b => {
                edi = m.read32(ecx.wrapping_add(0x4));
                ebp = m.read32(esp.wrapping_add(0xec));
                ebx = m.read32(edi);
                ecx = m.read32(ecx);
                ebp = ebp.wrapping_sub(ebx);
                ebp = ((ebp as i32).wrapping_mul((0xc8) as i32)) as u32;
                m.write32(esp.wrapping_add(0x14), ecx);
                ecx = 0;
                ebp = ebp.wrapping_add(edi);
                fa = ((((m.read16(eax.wrapping_add(0xa)))) as u32) & 0xffffu32);
                fb = ((((((ecx) as u16))) as u32) & 0xffffu32);
                m.write32(esp.wrapping_add(0x18), edi);
                m.write32(esp.wrapping_add(0x40), ebp);
                m.write32(esp.wrapping_add(0x34), ecx);
                if (fa <= fb) { pc = 0x92f1f; continue; }
                pc = 0x92a6f; continue;
            }
            0x92a6f => {
                pc = 0x92a75; continue;
            }
            0x92a71 => {
                edi = m.read32(esp.wrapping_add(0x18));
                pc = 0x92a75; continue;
            }
            0x92a75 => {
                eax = ((m.read16(eax.wrapping_add(0x8))) as u16 as u32);
                edi = m.read32(edi.wrapping_add(0xc));
                eax = eax.wrapping_add(ecx);
                edx = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = m.read32(esp.wrapping_add(0xf0));
                ecx = m.read32(eax.wrapping_add(0x1c));
                eax = ((m.read8(ecx.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x4))) as u8 as u32);
                ecx = ecx.wrapping_add(edx.wrapping_mul(0x4));
                edx = ((m.read16(ecx.wrapping_add(0x6))) as u16 as u32);
                eax = eax.wrapping_shr((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                edi = (edi & 0x10);
                fa = edi;
                esi = m.read32(ebp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x60));
                edx = edx.wrapping_add(edx.wrapping_mul(0x4));
                esi = esi.wrapping_add(edx.wrapping_mul(0x2));
                edx = ((m.read16(esi.wrapping_add(0x6))) as u16 as u32);
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                m.write32(esp.wrapping_add(0x3c), esi);
                eax = ebp.wrapping_add(eax.wrapping_mul(0x8));
                if ((fa == 0)) { pc = 0x92ac0; continue; }
                pc = 0x92ab8; continue;
            }
            0x92ab8 => {
                eax = m.read32(eax.wrapping_add(0x4));
                edx = eax.wrapping_add(edx.wrapping_mul(0x8));
                pc = 0x92ac8; continue;
            }
            0x92ac0 => {
                ebx = m.read32(eax.wrapping_add(0x4));
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_add(ebx);
                pc = 0x92ac8; continue;
            }
            0x92ac8 => {
                ebx = (ebx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x4))) as u32)) & 0xff);
                esi = (esi & 0xffff_0000) | ((((m.read16(esi)) as u32)) & 0xffff);
                eax = 0;
                ebx = (ebx & 0xffff_ff00) | ((((((((ebx) as u8)) as i32).wrapping_shr((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31))) as u32)) & 0xff);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = ((((((esi) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                m.write32(esp.wrapping_add(0x38), eax);
                if (fas < fbs) { pc = 0x92b14; continue; }
                pc = 0x92ae1; continue;
            }
            0x92ae1 => {
                eax = ((((esi) as u16)) as u16 as i16 as i32 as u32);
                esi = m.read32(esp.wrapping_add(0x18));
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = eax.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(m.read32(esi.wrapping_add(0x14)));
                eax = m.read32(eax.wrapping_add(0x1c));
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x92afc; continue; }
                pc = 0x92af8; continue;
            }
            0x92af8 => {
                join_zero = eax & 1 == 0;
                pc = 0x92b0e; continue;
            }
            0x92afc => {
                esi = m.read32(esp.wrapping_add(0x110));
                fa = esi;
                fb = eax;
                if ((fa & fb) != 0) { pc = 0x92c7c; continue; }
                pc = 0x92b0b; continue;
            }
            0x92b0b => {
                join_zero = ebx & 0xff == 1;
                pc = 0x92b0e; continue;
            }
            0x92b0e => {
                if join_zero { pc = 0x00092c7c; continue; }
                pc = 0x92b14; continue;
            }
            0x92b14 => {
                esi = m.read32(esp.wrapping_add(0x18));
                eax = m.read32(esp.wrapping_add(0xf0));
                push!(ecx);
                push!(edi);
                ecx = esi.wrapping_add(0x38);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x44));
                push!(eax);
                push!(edx);
                push!(0x4);
                push!(0x8);
                push!(ecx);
                edx = esp.wrapping_add(0x3c);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x120));
                eax = esp.wrapping_add(0x68);
                push!(eax);
                ecx = esp.wrapping_add(0x8c);
                push!(ecx);
                push!(edx);
                call_to!(m.read32(0x4d1858));
                pc = 0x92b50; continue;
            }
            0x92b50 => {
                edi = eax;
                esp = esp.wrapping_add(0x30);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x92c7c; continue; }
                pc = 0x92b5d; continue;
            }
            0x92b5d => {
                eax = m.read32(esi.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x92bc4; continue; }
                pc = 0x92b64; continue;
            }
            0x92b64 => {
                eax = m.read32(esp.wrapping_add(0x14));
                eax = m.read32(eax.wrapping_add(0x28));
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x92bc4; continue; }
                pc = 0x92b6f; continue;
            }
            0x92b6f => {
                fa = eax;
                fb = ((((0x200000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x92bc4; continue; }
                pc = 0x92b76; continue;
            }
            0x92b76 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x92ba3; continue; }
                pc = 0x92b7a; continue;
            }
            0x92b7a => {
                esi = esp.wrapping_add(0x64);
                ebx = edi;
                pc = 0x92b80; continue;
            }
            0x92b80 => {
                ecx = (((((m.read8(ebp.wrapping_add(0x49)) as i8)) as i32)) as u32);
                edx = m.read32(esp.wrapping_add(0xe8));
                eax = m.read32(edx.wrapping_add(0x10));
                ecx = ecx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_add(eax);
                push!(esi);
                push!(ecx);
                call!(0x151e80);
                pc = 0x92b9a; continue;
            }
            0x92b9a => {
                esp = esp.wrapping_add(0x8);
                esi = esi.wrapping_add(0x10);
                ebx = ebx.wrapping_sub(1);
                fa = ebx;
                if ((fa != 0)) { pc = 0x92b80; continue; }
                pc = 0x92ba3; continue;
            }
            0x92ba3 => {
                ecx = (((((m.read8(ebp.wrapping_add(0x49)) as i8)) as i32)) as u32);
                edx = m.read32(esp.wrapping_add(0xe8));
                esi = m.read32(edx.wrapping_add(0x10));
                eax = esp.wrapping_add(0x1c);
                ecx = ecx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(eax);
                ecx = ecx.wrapping_add(esi);
                push!(ecx);
                call!(0x152090);
                pc = 0x92bc1; continue;
            }
            0x92bc1 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x92bc4; continue;
            }
            0x92bc4 => {
                eax = m.read32(esp.wrapping_add(0x14));
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x32))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x92c0c; continue; }
                pc = 0x92bcf; continue;
            }
            0x92bcf => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x92bf7; continue; }
                pc = 0x92bd3; continue;
            }
            0x92bd3 => {
                esi = esp.wrapping_add(0x64);
                ebx = edi;
                pc = 0x92be0; continue;
            }
            0x92be0 => {
                ecx = m.read32(esp.wrapping_add(0x10c));
                push!(esi);
                push!(ecx);
                call!(0x151e80);
                pc = 0x92bee; continue;
            }
            0x92bee => {
                esp = esp.wrapping_add(0x8);
                esi = esi.wrapping_add(0x10);
                ebx = ebx.wrapping_sub(1);
                fa = ebx;
                if ((fa != 0)) { pc = 0x92be0; continue; }
                pc = 0x92bf7; continue;
            }
            0x92bf7 => {
                eax = m.read32(esp.wrapping_add(0x10c));
                edx = esp.wrapping_add(0x1c);
                push!(edx);
                push!(eax);
                call!(0x152090);
                pc = 0x92c09; continue;
            }
            0x92c09 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x92c0c; continue;
            }
            0x92c0c => {
                esi = 0;
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x92c7c; continue; }
                pc = 0x92c12; continue;
            }
            0x92c12 => {
                ebp = esp.wrapping_add(0x64);
                pc = 0x92c16; continue;
            }
            0x92c16 => {
                call!(0x1665a0);
                pc = 0x92c1b; continue;
            }
            0x92c1b => {
                ebx = eax;
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x92f1f; continue; }
                pc = 0x92c25; continue;
            }
            0x92c25 => {
                ecx = m.read32(esp.wrapping_add(0x3c));
                edx = (((((m.read16(ecx) as i16)) as i32)) as u32);
                eax = m.read32(esp.wrapping_add(0x104));
                ecx = m.read32(esp.wrapping_add(0x14));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x104));
                push!(eax);
                eax = m.read32(esp.wrapping_add(esi.wrapping_mul(0x4)).wrapping_add(0x4c));
                push!(ecx);
                push!(edx);
                push!(eax);
                push!(0);
                ecx = esp.wrapping_add(0x34);
                push!(ecx);
                push!(ebp);
                push!(ebx);
                call!(0x1665f0);
                pc = 0x92c55; continue;
            }
            0x92c55 => {
                edx = m.read32(esp.wrapping_add(0x12c));
                push!(ebx);
                push!(edx);
                call!(0x1665e0);
                pc = 0x92c63; continue;
            }
            0x92c63 => {
                ebx = m.read32(esp.wrapping_add(0x3c));
                ebx = (ebx | edi);
                esp = esp.wrapping_add(0x2c);
                esi = esi.wrapping_add(1);
                ebp = ebp.wrapping_add(0x10);
                fa = esi;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), ebx);
                if (fas < fbs) { pc = 0x92c16; continue; }
                pc = 0x92c78; continue;
            }
            0x92c78 => {
                ebp = m.read32(esp.wrapping_add(0x40));
                pc = 0x92c7c; continue;
            }
            0x92c7c => {
                eax = m.read32(esp.wrapping_add(0xf4));
                ecx = m.read32(esp.wrapping_add(0x34));
                edx = ((m.read16(eax.wrapping_add(0xa))) as u16 as u32);
                ecx = ecx.wrapping_add(1);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x34), ecx);
                if (fas < fbs) { pc = 0x92a71; continue; }
                pc = 0x92c98; continue;
            }
            0x92c98 => {
                eax = m.read32(esp.wrapping_add(0x10));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xd4);
                break 'run;
            }
            0x92ca7 => {
                edx = m.read32(ecx.wrapping_add(0x4));
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(edx.wrapping_add(0xc))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (((((((((((((fa)) as u8)) as i32).wrapping_sub((((((fb)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) == 0i32)) { pc = 0x92f1f; continue; }
                pc = 0x92cb5; continue;
            }
            0x92cb5 => {
                esi = m.read32(esp.wrapping_add(0xf8));
                eax = ((m.read16(eax.wrapping_add(0x8))) as u16 as u32);
                ebp = m.read32(esp.wrapping_add(0xf0));
                ecx = m.read32(ebp.wrapping_add(0x24));
                xmm3 = Xmm::scalar(m.read32(0x475dcc));
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x10)));
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0x14)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                edx = ((m.read8(ecx.wrapping_add(eax.wrapping_mul(0x4)))) as u8 as u32);
                edi = ecx.wrapping_add(eax.wrapping_mul(0x4));
                xmm3.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                m.write32(esp.wrapping_add(0x1c), xmm3.lanes[0]);
                eax = ((m.read8(edi.wrapping_add(0x1))) as u8 as u32);
                xmm3.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x20), xmm3.lanes[0]);
                ecx = ((m.read8(edi.wrapping_add(0x2))) as u8 as u32);
                ebx = m.read32(esp.wrapping_add(0xfc));
                xmm3.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm3.lanes[0]);
                edx = ((m.read8(edi.wrapping_add(0x3))) as u8 as u32);
                xmm3.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm3.lanes[0]);
                eax = ((m.read8(edi.wrapping_add(0x4))) as u8 as u32);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                ecx = ((m.read8(edi.wrapping_add(0x5))) as u8 as u32);
                edx = esp.wrapping_add(0x1c);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                push!(edx);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                push!(ebx);
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                call!(0x4c3e0);
                pc = 0x92d9b; continue;
            }
            0x92d9b => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x92dee; continue; }
                pc = 0x92da2; continue;
            }
            0x92da2 => {
                eax = m.read32(esp.wrapping_add(0x110));
                ecx = m.read32(esp.wrapping_add(0x10c));
                edx = m.read32(esp.wrapping_add(0x108));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x108));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x108));
                push!(edx);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0xfc));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0xfc));
                push!(ebx);
                edx = esp.wrapping_add(0x34);
                push!(edx);
                push!(edi);
                push!(ebp);
                push!(eax);
                push!(ecx);
                call!(0x92a10);
                pc = 0x92de7; continue;
            }
            0x92de7 => {
                esp = esp.wrapping_add(0x2c);
                m.write32(esp.wrapping_add(0x10), eax);
                pc = 0x92dee; continue;
            }
            0x92dee => {
                edx = m.read32(esp.wrapping_add(0xf4));
                eax = ((m.read16(edx.wrapping_add(0xa))) as u16 as u32);
                ecx = m.read32(ebp.wrapping_add(0x24));
                xmm3 = Xmm::scalar(m.read32(0x475dcc));
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x10)));
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0x14)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                edx = ((m.read8(ecx.wrapping_add(eax.wrapping_mul(0x4)))) as u8 as u32);
                edi = ecx.wrapping_add(eax.wrapping_mul(0x4));
                xmm3.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                m.write32(esp.wrapping_add(0x1c), xmm3.lanes[0]);
                eax = ((m.read8(edi.wrapping_add(0x1))) as u8 as u32);
                xmm3.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x20), xmm3.lanes[0]);
                ecx = ((m.read8(edi.wrapping_add(0x2))) as u8 as u32);
                xmm3.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm3.lanes[0]);
                edx = ((m.read8(edi.wrapping_add(0x3))) as u8 as u32);
                xmm3.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm3.lanes[0]);
                eax = ((m.read8(edi.wrapping_add(0x4))) as u8 as u32);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                ecx = ((m.read8(edi.wrapping_add(0x5))) as u8 as u32);
                edx = esp.wrapping_add(0x1c);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                push!(edx);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                push!(ebx);
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                call!(0x4c3e0);
                pc = 0x92ec6; continue;
            }
            0x92ec6 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x92f1f; continue; }
                pc = 0x92ecd; continue;
            }
            0x92ecd => {
                eax = m.read32(esp.wrapping_add(0x110));
                ecx = m.read32(esp.wrapping_add(0x10c));
                edx = m.read32(esp.wrapping_add(0x108));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x108));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x108));
                push!(edx);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0xfc));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0xfc));
                push!(ebx);
                edx = esp.wrapping_add(0x34);
                push!(edx);
                push!(edi);
                push!(ebp);
                push!(eax);
                push!(ecx);
                call!(0x92a10);
                pc = 0x92f12; continue;
            }
            0x92f12 => {
                ecx = m.read32(esp.wrapping_add(0x3c));
                esp = esp.wrapping_add(0x2c);
                ecx = (ecx | eax);
                m.write32(esp.wrapping_add(0x10), ecx);
                pc = 0x92f1f; continue;
            }
            0x92f1f => {
                eax = m.read32(esp.wrapping_add(0x10));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xd4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00093690: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f93690<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut join_zero = false;
    let mut pc: u32 = 0x93690;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x93690 => {
                esp = esp.wrapping_sub(0xd4);
                eax = m.read32(esp.wrapping_add(0xe4));
                fa = ((((m.read16(eax.wrapping_add(0x6)))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                ecx = m.read32(esp.wrapping_add(0xd8));
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                m.write32(esp.wrapping_add(0x10), 0);
                if (fa == fb) { pc = 0x93927; continue; }
                pc = 0x936bb; continue;
            }
            0x936bb => {
                edi = m.read32(ecx.wrapping_add(0x4));
                ebp = m.read32(esp.wrapping_add(0xec));
                ebx = m.read32(edi);
                ecx = m.read32(ecx);
                ebp = ebp.wrapping_sub(ebx);
                ebp = ((ebp as i32).wrapping_mul((0xc8) as i32)) as u32;
                m.write32(esp.wrapping_add(0x14), ecx);
                ecx = 0;
                ebp = ebp.wrapping_add(edi);
                fa = ((((m.read16(eax.wrapping_add(0xa)))) as u32) & 0xffffu32);
                fb = ((((((ecx) as u16))) as u32) & 0xffffu32);
                m.write32(esp.wrapping_add(0x18), edi);
                m.write32(esp.wrapping_add(0x40), ebp);
                m.write32(esp.wrapping_add(0x34), ecx);
                if (fa <= fb) { pc = 0x93b9f; continue; }
                pc = 0x936ef; continue;
            }
            0x936ef => {
                pc = 0x936f5; continue;
            }
            0x936f1 => {
                edi = m.read32(esp.wrapping_add(0x18));
                pc = 0x936f5; continue;
            }
            0x936f5 => {
                eax = ((m.read16(eax.wrapping_add(0x8))) as u16 as u32);
                edi = m.read32(edi.wrapping_add(0xc));
                eax = eax.wrapping_add(ecx);
                edx = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = m.read32(esp.wrapping_add(0xf0));
                ecx = m.read32(eax.wrapping_add(0x1c));
                eax = ((m.read8(ecx.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x4))) as u8 as u32);
                ecx = ecx.wrapping_add(edx.wrapping_mul(0x4));
                edx = ((m.read16(ecx.wrapping_add(0x6))) as u16 as u32);
                eax = eax.wrapping_shr((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                edi = (edi & 0x10);
                fa = edi;
                esi = m.read32(ebp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x60));
                edx = edx.wrapping_add(edx.wrapping_mul(0x4));
                esi = esi.wrapping_add(edx.wrapping_mul(0x2));
                edx = ((m.read16(esi.wrapping_add(0x6))) as u16 as u32);
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                m.write32(esp.wrapping_add(0x3c), esi);
                eax = ebp.wrapping_add(eax.wrapping_mul(0x8));
                if ((fa == 0)) { pc = 0x93740; continue; }
                pc = 0x93738; continue;
            }
            0x93738 => {
                eax = m.read32(eax.wrapping_add(0x4));
                edx = eax.wrapping_add(edx.wrapping_mul(0x8));
                pc = 0x93748; continue;
            }
            0x93740 => {
                ebx = m.read32(eax.wrapping_add(0x4));
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_add(ebx);
                pc = 0x93748; continue;
            }
            0x93748 => {
                ebx = (ebx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x4))) as u32)) & 0xff);
                esi = (esi & 0xffff_0000) | ((((m.read16(esi)) as u32)) & 0xffff);
                eax = 0;
                ebx = (ebx & 0xffff_ff00) | ((((((((ebx) as u8)) as i32).wrapping_shr((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31))) as u32)) & 0xff);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = ((((((esi) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                m.write32(esp.wrapping_add(0x38), eax);
                if (fas < fbs) { pc = 0x93794; continue; }
                pc = 0x93761; continue;
            }
            0x93761 => {
                eax = ((((esi) as u16)) as u16 as i16 as i32 as u32);
                esi = m.read32(esp.wrapping_add(0x18));
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = eax.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(m.read32(esi.wrapping_add(0x14)));
                eax = m.read32(eax.wrapping_add(0x1c));
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x9377c; continue; }
                pc = 0x93778; continue;
            }
            0x93778 => {
                join_zero = eax & 1 == 0;
                pc = 0x9378e; continue;
            }
            0x9377c => {
                esi = m.read32(esp.wrapping_add(0x110));
                fa = esi;
                fb = eax;
                if ((fa & fb) != 0) { pc = 0x938fc; continue; }
                pc = 0x9378b; continue;
            }
            0x9378b => {
                join_zero = ebx & 0xff == 1;
                pc = 0x9378e; continue;
            }
            0x9378e => {
                if join_zero { pc = 0x000938fc; continue; }
                pc = 0x93794; continue;
            }
            0x93794 => {
                esi = m.read32(esp.wrapping_add(0x18));
                eax = m.read32(esp.wrapping_add(0xf0));
                push!(ecx);
                push!(edi);
                ecx = esi.wrapping_add(0x38);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x44));
                push!(eax);
                push!(edx);
                push!(0x4);
                push!(0x8);
                push!(ecx);
                edx = esp.wrapping_add(0x3c);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x120));
                eax = esp.wrapping_add(0x68);
                push!(eax);
                ecx = esp.wrapping_add(0x8c);
                push!(ecx);
                push!(edx);
                call!(0x54730);
                pc = 0x937cf; continue;
            }
            0x937cf => {
                edi = eax;
                esp = esp.wrapping_add(0x30);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x938fc; continue; }
                pc = 0x937dc; continue;
            }
            0x937dc => {
                eax = m.read32(esi.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x93844; continue; }
                pc = 0x937e3; continue;
            }
            0x937e3 => {
                eax = m.read32(esp.wrapping_add(0x14));
                eax = m.read32(eax.wrapping_add(0x28));
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x93844; continue; }
                pc = 0x937ee; continue;
            }
            0x937ee => {
                fa = eax;
                fb = ((((0x200000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x93844; continue; }
                pc = 0x937f5; continue;
            }
            0x937f5 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x93823; continue; }
                pc = 0x937f9; continue;
            }
            0x937f9 => {
                esi = esp.wrapping_add(0x64);
                ebx = edi;
                pc = 0x93800; continue;
            }
            0x93800 => {
                ecx = (((((m.read8(ebp.wrapping_add(0x49)) as i8)) as i32)) as u32);
                edx = m.read32(esp.wrapping_add(0xe8));
                eax = m.read32(edx.wrapping_add(0x10));
                ecx = ecx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_add(eax);
                push!(esi);
                push!(ecx);
                call!(0x151e80);
                pc = 0x9381a; continue;
            }
            0x9381a => {
                esp = esp.wrapping_add(0x8);
                esi = esi.wrapping_add(0x10);
                ebx = ebx.wrapping_sub(1);
                fa = ebx;
                if ((fa != 0)) { pc = 0x93800; continue; }
                pc = 0x93823; continue;
            }
            0x93823 => {
                ecx = (((((m.read8(ebp.wrapping_add(0x49)) as i8)) as i32)) as u32);
                edx = m.read32(esp.wrapping_add(0xe8));
                esi = m.read32(edx.wrapping_add(0x10));
                eax = esp.wrapping_add(0x1c);
                ecx = ecx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(eax);
                ecx = ecx.wrapping_add(esi);
                push!(ecx);
                call!(0x152090);
                pc = 0x93841; continue;
            }
            0x93841 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x93844; continue;
            }
            0x93844 => {
                eax = m.read32(esp.wrapping_add(0x14));
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x32))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x9388c; continue; }
                pc = 0x9384f; continue;
            }
            0x9384f => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x93877; continue; }
                pc = 0x93853; continue;
            }
            0x93853 => {
                esi = esp.wrapping_add(0x64);
                ebx = edi;
                pc = 0x93860; continue;
            }
            0x93860 => {
                ecx = m.read32(esp.wrapping_add(0x10c));
                push!(esi);
                push!(ecx);
                call!(0x151e80);
                pc = 0x9386e; continue;
            }
            0x9386e => {
                esp = esp.wrapping_add(0x8);
                esi = esi.wrapping_add(0x10);
                ebx = ebx.wrapping_sub(1);
                fa = ebx;
                if ((fa != 0)) { pc = 0x93860; continue; }
                pc = 0x93877; continue;
            }
            0x93877 => {
                eax = m.read32(esp.wrapping_add(0x10c));
                edx = esp.wrapping_add(0x1c);
                push!(edx);
                push!(eax);
                call!(0x152090);
                pc = 0x93889; continue;
            }
            0x93889 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x9388c; continue;
            }
            0x9388c => {
                esi = 0;
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x938fc; continue; }
                pc = 0x93892; continue;
            }
            0x93892 => {
                ebp = esp.wrapping_add(0x64);
                pc = 0x93896; continue;
            }
            0x93896 => {
                call!(0x1665a0);
                pc = 0x9389b; continue;
            }
            0x9389b => {
                ebx = eax;
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x93b9f; continue; }
                pc = 0x938a5; continue;
            }
            0x938a5 => {
                ecx = m.read32(esp.wrapping_add(0x3c));
                edx = (((((m.read16(ecx) as i16)) as i32)) as u32);
                eax = m.read32(esp.wrapping_add(0x104));
                ecx = m.read32(esp.wrapping_add(0x14));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x104));
                push!(eax);
                eax = m.read32(esp.wrapping_add(esi.wrapping_mul(0x4)).wrapping_add(0x4c));
                push!(ecx);
                push!(edx);
                push!(eax);
                push!(0);
                ecx = esp.wrapping_add(0x34);
                push!(ecx);
                push!(ebp);
                push!(ebx);
                call!(0x1665f0);
                pc = 0x938d5; continue;
            }
            0x938d5 => {
                edx = m.read32(esp.wrapping_add(0x12c));
                push!(ebx);
                push!(edx);
                call!(0x1665e0);
                pc = 0x938e3; continue;
            }
            0x938e3 => {
                ebx = m.read32(esp.wrapping_add(0x3c));
                ebx = (ebx | edi);
                esp = esp.wrapping_add(0x2c);
                esi = esi.wrapping_add(1);
                ebp = ebp.wrapping_add(0x10);
                fa = esi;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), ebx);
                if (fas < fbs) { pc = 0x93896; continue; }
                pc = 0x938f8; continue;
            }
            0x938f8 => {
                ebp = m.read32(esp.wrapping_add(0x40));
                pc = 0x938fc; continue;
            }
            0x938fc => {
                eax = m.read32(esp.wrapping_add(0xf4));
                ecx = m.read32(esp.wrapping_add(0x34));
                edx = ((m.read16(eax.wrapping_add(0xa))) as u16 as u32);
                ecx = ecx.wrapping_add(1);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x34), ecx);
                if (fas < fbs) { pc = 0x936f1; continue; }
                pc = 0x93918; continue;
            }
            0x93918 => {
                eax = m.read32(esp.wrapping_add(0x10));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xd4);
                break 'run;
            }
            0x93927 => {
                edx = m.read32(ecx.wrapping_add(0x4));
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(edx.wrapping_add(0xc))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (((((((((((((fa)) as u8)) as i32).wrapping_sub((((((fb)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) == 0i32)) { pc = 0x93b9f; continue; }
                pc = 0x93935; continue;
            }
            0x93935 => {
                esi = m.read32(esp.wrapping_add(0xf8));
                eax = ((m.read16(eax.wrapping_add(0x8))) as u16 as u32);
                ebp = m.read32(esp.wrapping_add(0xf0));
                ecx = m.read32(ebp.wrapping_add(0x24));
                xmm3 = Xmm::scalar(m.read32(0x475dcc));
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x10)));
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0x14)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                edx = ((m.read8(ecx.wrapping_add(eax.wrapping_mul(0x4)))) as u8 as u32);
                edi = ecx.wrapping_add(eax.wrapping_mul(0x4));
                xmm3.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                m.write32(esp.wrapping_add(0x1c), xmm3.lanes[0]);
                eax = ((m.read8(edi.wrapping_add(0x1))) as u8 as u32);
                xmm3.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x20), xmm3.lanes[0]);
                ecx = ((m.read8(edi.wrapping_add(0x2))) as u8 as u32);
                ebx = m.read32(esp.wrapping_add(0xfc));
                xmm3.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm3.lanes[0]);
                edx = ((m.read8(edi.wrapping_add(0x3))) as u8 as u32);
                xmm3.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm3.lanes[0]);
                eax = ((m.read8(edi.wrapping_add(0x4))) as u8 as u32);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                ecx = ((m.read8(edi.wrapping_add(0x5))) as u8 as u32);
                edx = esp.wrapping_add(0x1c);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                push!(edx);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                push!(ebx);
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                call!(0x524f0);
                pc = 0x93a1b; continue;
            }
            0x93a1b => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x93a6e; continue; }
                pc = 0x93a22; continue;
            }
            0x93a22 => {
                eax = m.read32(esp.wrapping_add(0x110));
                ecx = m.read32(esp.wrapping_add(0x10c));
                edx = m.read32(esp.wrapping_add(0x108));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x108));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x108));
                push!(edx);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0xfc));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0xfc));
                push!(ebx);
                edx = esp.wrapping_add(0x34);
                push!(edx);
                push!(edi);
                push!(ebp);
                push!(eax);
                push!(ecx);
                call!(0x93690);
                pc = 0x93a67; continue;
            }
            0x93a67 => {
                esp = esp.wrapping_add(0x2c);
                m.write32(esp.wrapping_add(0x10), eax);
                pc = 0x93a6e; continue;
            }
            0x93a6e => {
                edx = m.read32(esp.wrapping_add(0xf4));
                eax = ((m.read16(edx.wrapping_add(0xa))) as u16 as u32);
                ecx = m.read32(ebp.wrapping_add(0x24));
                xmm3 = Xmm::scalar(m.read32(0x475dcc));
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x10)));
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0x14)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                edx = ((m.read8(ecx.wrapping_add(eax.wrapping_mul(0x4)))) as u8 as u32);
                edi = ecx.wrapping_add(eax.wrapping_mul(0x4));
                xmm3.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                m.write32(esp.wrapping_add(0x1c), xmm3.lanes[0]);
                eax = ((m.read8(edi.wrapping_add(0x1))) as u8 as u32);
                xmm3.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x20), xmm3.lanes[0]);
                ecx = ((m.read8(edi.wrapping_add(0x2))) as u8 as u32);
                xmm3.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm3.lanes[0]);
                edx = ((m.read8(edi.wrapping_add(0x3))) as u8 as u32);
                xmm3.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm3.lanes[0]);
                eax = ((m.read8(edi.wrapping_add(0x4))) as u8 as u32);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                ecx = ((m.read8(edi.wrapping_add(0x5))) as u8 as u32);
                edx = esp.wrapping_add(0x1c);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                push!(edx);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                push!(ebx);
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                call!(0x524f0);
                pc = 0x93b46; continue;
            }
            0x93b46 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x93b9f; continue; }
                pc = 0x93b4d; continue;
            }
            0x93b4d => {
                eax = m.read32(esp.wrapping_add(0x110));
                ecx = m.read32(esp.wrapping_add(0x10c));
                edx = m.read32(esp.wrapping_add(0x108));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x108));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x108));
                push!(edx);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0xfc));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0xfc));
                push!(ebx);
                edx = esp.wrapping_add(0x34);
                push!(edx);
                push!(edi);
                push!(ebp);
                push!(eax);
                push!(ecx);
                call!(0x93690);
                pc = 0x93b92; continue;
            }
            0x93b92 => {
                ecx = m.read32(esp.wrapping_add(0x3c));
                esp = esp.wrapping_add(0x2c);
                ecx = (ecx | eax);
                m.write32(esp.wrapping_add(0x10), ecx);
                pc = 0x93b9f; continue;
            }
            0x93b9f => {
                eax = m.read32(esp.wrapping_add(0x10));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xd4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

// portreg: va=0x000a3ae0
/// 0x000a3ae0: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn fa3ae0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut fp_cmp = 0i32;
    let mut pc: u32 = 0xa3ae0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0xa3ae0 => {
                push!(ebp);
                ebp = esp;
                esp = (esp & 0xfffffff8u32);
                esp = esp.wrapping_sub(0x4c);
                ecx = m.read32(ebp.wrapping_add(0x8));
                edx = m.read32(ecx);
                push!(ebx);
                push!(esi);
                esi = m.read32(0x7356d8);
                eax = 0;
                fa = edx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                push!(edi);
                m.write32(esp.wrapping_add(0x1c), eax);
                if (fa == fb) { pc = 0xa3b13; continue; }
                pc = 0xa3b02; continue;
            }
            0xa3b02 => {
                eax = edx;
                eax = ((eax as i32).wrapping_mul((0xea0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(esi));
                m.write32(esp.wrapping_add(0xc), ecx);
                pc = 0xa3b17; continue;
            }
            0xa3b13 => {
                m.write32(esp.wrapping_add(0xc), eax);
                pc = 0xa3b17; continue;
            }
            0xa3b17 => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                eax = 0;
                ebx = (ebx | 0xffffffffu32);
                fa = edx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                ecx = 0xb;
                edi = esp.wrapping_add(0x2c);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                m.write32(esp.wrapping_add(0x18), ebx);
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                if (fa == fb) { pc = 0xa3b67; continue; }
                pc = 0xa3b3e; continue;
            }
            0xa3b3e => {
                edx = ((edx as i32).wrapping_mul((0xea0) as i32)) as u32;
                ecx = m.read32(edx.wrapping_add(esi).wrapping_add(0xc14));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = edx.wrapping_add(esi).wrapping_add(0xc14);
                if (fa == fb) { pc = 0xa3b67; continue; }
                pc = 0xa3b56; continue;
            }
            0xa3b56 => {
                eax = ecx;
                edx = m.read32(eax.wrapping_add(0x28));
                ecx = edx;
                ecx = (ecx & 0xffffffbfu32);
                m.write32(esp.wrapping_add(0x1c), edx);
                m.write32(eax.wrapping_add(0x28), ecx);
                pc = 0xa3b67; continue;
            }
            0xa3b67 => {
                ecx = m.read32(0x715fa4);
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x14), eax);
                if (fas <= fbs) { pc = 0xa3d9f; continue; }
                pc = 0xa3b7b; continue;
            }
            0xa3b7b => {
                pc = 0xa3b85; continue;
            }
            0xa3b7d => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                pc = 0xa3b85; continue;
            }
            0xa3b85 => {
                ebx = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x72f2a0));
                fa = ((((m.read8(ebx.wrapping_add(0x28)))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0xa3d84; continue; }
                pc = 0xa3b96; continue;
            }
            0xa3b96 => {
                fa = m.read32(ebx.wrapping_add(0x20));
                fb = ((((0x8i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xa3cc2; continue; }
                pc = 0xa3ba0; continue;
            }
            0xa3ba0 => {
                eax = m.read32(ebp.wrapping_add(0x8));
                eax = m.read32(eax);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = m.read32(ebx.wrapping_add(0x7c));
                if (fas <= fbs) { pc = 0xa3bc1; continue; }
                pc = 0xa3bad; continue;
            }
            0xa3bad => {
                edx = m.read32(0x7356d8);
                eax = ((eax as i32).wrapping_mul((0xea0) as i32)) as u32;
                fa = ((((m.read16(eax.wrapping_add(edx).wrapping_add(0x48)))) as u32) & 0xffffu32);
                fb = ((((0x4di32)) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0xa3bc8; continue; }
                pc = 0xa3bc1; continue;
            }
            0xa3bc1 => {
                fa = ((((m.read16(ecx.wrapping_add(0x48)))) as u32) & 0xffffu32);
                fb = ((((0x4di32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0xa3bd2; continue; }
                pc = 0xa3bc8; continue;
            }
            0xa3bc8 => {
                xmm0 = Xmm::scalar(m.read32(0x475cd0));
                pc = 0xa3bf6; continue;
            }
            0xa3bd2 => {
                eax = m.read32(esp.wrapping_add(0xc));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xa3bf6; continue; }
                pc = 0xa3bda; continue;
            }
            0xa3bda => {
                edx = m.read32(eax.wrapping_add(0x150));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xa3bf6; continue; }
                pc = 0xa3be4; continue;
            }
            0xa3be4 => {
                eax = m.read32(ebx.wrapping_add(0x150));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xa3bf6; continue; }
                pc = 0xa3bee; continue;
            }
            0xa3bee => {
                xmm0 = Xmm::scalar(m.read32(0x475cd4));
                pc = 0xa3bf6; continue;
            }
            0xa3bf6 => {
                eax = (eax & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x48))) as u32)) & 0xffff);
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0x46i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0xa3c4b; continue; }
                pc = 0xa3c00; continue;
            }
            0xa3c00 => {
                xmm1 = Xmm::scalar(m.read32(0x53b10c));
                eax = esp.wrapping_add(0x24);
                push!(eax);
                eax = m.read32(ebp.wrapping_add(0x1c));
                ecx = m.read32(eax.wrapping_add(0x4));
                edx = m.read32(eax);
                esp = esp.wrapping_sub(0xc);
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x8), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(ebx.wrapping_add(0x1b0)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ebx.wrapping_add(0x6c)))).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(ebx.wrapping_add(0x1a8)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ebx.wrapping_add(0x64)))).to_bits();
                m.write32(esp, xmm1.lanes[0]);
                push!(ecx);
                push!(edx);
                pc = 0xa3c7d; continue;
            }
            0xa3c4b => {
                eax = ((((eax) as u16)) as u16 as i16 as i32 as u32);
                edx = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = m.read32(ebx.wrapping_add(0x6c));
                ecx = esp.wrapping_add(0x24);
                push!(ecx);
                push!(ecx);
                ecx = m.read32(ebx.wrapping_add(0x64));
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_add(0x53a3ec)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp, xmm1.lanes[0]);
                push!(eax);
                eax = m.read32(ebp.wrapping_add(0x1c));
                edx = m.read32(eax.wrapping_add(0x4));
                eax = m.read32(eax);
                push!(ecx);
                push!(edx);
                push!(eax);
                pc = 0xa3c7d; continue;
            }
            0xa3c7d => {
                eax = m.read32(ebp.wrapping_add(0x10));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x14)))).to_bits();
                edx = m.read32(eax);
                push!(ecx);
                ecx = m.read32(eax.wrapping_add(0x8));
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                push!(edx);
                call!(0x47440);
                pc = 0xa3c97; continue;
            }
            0xa3c97 => {
                m.write32(esp.wrapping_add(0x34), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x5c)));
                esp = esp.wrapping_add(0x24);
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0xa3d84; continue; }
                pc = 0xa3cb3; continue;
            }
            0xa3cb3 => {
                eax = m.read32(ebx.wrapping_add(0x7c));
                ecx = m.read32(eax.wrapping_add(0x8));
                m.write32(esp.wrapping_add(0x18), ecx);
                pc = 0xa3d5a; continue;
            }
            0xa3cc2 => {
                eax = m.read32(ebx.wrapping_add(0x80));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xa3d84; continue; }
                pc = 0xa3cd0; continue;
            }
            0xa3cd0 => {
                push!(eax);
                call!(0x1680b0);
                pc = 0xa3cd6; continue;
            }
            0xa3cd6 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0x200000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0xa3d84; continue; }
                pc = 0xa3ce4; continue;
            }
            0xa3ce4 => {
                edx = m.read32(ebx.wrapping_add(0x80));
                push!(edx);
                call!(0x167a90);
                pc = 0xa3cf0; continue;
            }
            0xa3cf0 => {
                esp = esp.wrapping_add(0x4);
                edi = eax;
                esi = 0x20;
                eax = 0;
                pc = 0xa3d02; continue;
            }
            0xa3cfe => {
                eax = m.read32(esp.wrapping_add(0x20));
                pc = 0xa3d02; continue;
            }
            0xa3d02 => {
                edx = m.read32(esi.wrapping_add(edi).wrapping_add(((0x4i32.wrapping_neg()) as u32)));
                esi = esi.wrapping_sub(0x8);
                eax = eax.wrapping_add(edi);
                ecx = esp.wrapping_add(0x24);
                push!(ecx);
                ecx = m.read32(esi.wrapping_add(edi));
                push!(edx);
                edx = m.read32(eax.wrapping_add(0x4));
                eax = m.read32(eax);
                push!(ecx);
                push!(edx);
                push!(eax);
                eax = m.read32(ebp.wrapping_add(0x1c));
                ecx = m.read32(eax.wrapping_add(0x4));
                edx = m.read32(eax);
                eax = m.read32(ebp.wrapping_add(0x14));
                push!(ecx);
                push!(edx);
                push!(eax);
                eax = m.read32(ebp.wrapping_add(0x10));
                ecx = m.read32(eax.wrapping_add(0x8));
                edx = m.read32(eax);
                push!(ecx);
                push!(edx);
                m.write32(esp.wrapping_add(0x48), esi);
                call!(0x471a0);
                pc = 0xa3d3d; continue;
            }
            0xa3d3d => {
                m.write32(esp.wrapping_add(0x38), (fpu.st(0) as f32).to_bits());
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x60))) as f64));
                esp = esp.wrapping_add(0x28);
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if ((fp_cmp == 0x1i32)) { pc = 0xa3d54; continue; }
                pc = 0xa3d4e; continue;
            }
            0xa3d4e => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xa3cfe; continue; }
                pc = 0xa3d52; continue;
            }
            0xa3d52 => {
                pc = 0xa3d84; continue;
            }
            0xa3d54 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                pc = 0xa3d5a; continue;
            }
            0xa3d5a => {
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                m.write32(esp.wrapping_add(0x2c), 0);
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x3c), ebx);
                pc = 0xa3d84; continue;
            }
            0xa3d84 => {
                eax = m.read32(esp.wrapping_add(0x14));
                ecx = m.read32(0x715fa4);
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x14), eax);
                if (fas < fbs) { pc = 0xa3b7d; continue; }
                pc = 0xa3d9b; continue;
            }
            0xa3d9b => {
                ebx = m.read32(esp.wrapping_add(0x18));
                pc = 0xa3d9f; continue;
            }
            0xa3d9f => {
                eax = m.read32(ebp.wrapping_add(0x8));
                eax = m.read32(eax);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xa3dc7; continue; }
                pc = 0xa3da9; continue;
            }
            0xa3da9 => {
                ecx = m.read32(0x7356d8);
                eax = ((eax as i32).wrapping_mul((0xea0) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(ecx).wrapping_add(0xc14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xa3dc7; continue; }
                pc = 0xa3dc0; continue;
            }
            0xa3dc0 => {
                edx = m.read32(esp.wrapping_add(0x1c));
                m.write32(eax.wrapping_add(0x28), edx);
                pc = 0xa3dc7; continue;
            }
            0xa3dc7 => {
                eax = m.read32(ebp.wrapping_add(0x20));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xa3dfc; continue; }
                pc = 0xa3dce; continue;
            }
            0xa3dce => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                ecx = m.read32(esp.wrapping_add(0x2c));
                edx = m.read32(esp.wrapping_add(0x3c));
                m.write32(eax.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                m.write32(eax.wrapping_add(0x8), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                m.write32(eax, ecx);
                m.write32(eax.wrapping_add(0xc), xmm0.lanes[0]);
                m.write32(eax.wrapping_add(0x10), edx);
                pc = 0xa3dfc; continue;
            }
            0xa3dfc => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebx;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000dc290: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn fdc290<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xdc290;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    'run: loop {
            match pc {
            0xdc290 => {
                eax = m.read32(esp.wrapping_add(0x4));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xdc2a3; continue; }
                pc = 0xdc299; continue;
            }
            0xdc299 => {
                eax = eax.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = m.read32(eax.wrapping_add(0x4deb40));
                break 'run;
            }
            0xdc2a3 => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000dcfb0: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn fdcfb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xdcfb0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    'run: loop {
            match pc {
            0xdcfb0 => {
                esp = esp.wrapping_sub(0x34);
                push!(ebx);
                eax = esp.wrapping_add(0x18);
                push!(ebp);
                push!(eax);
                call!(0x913d0);
                pc = 0xdcfbf; continue;
            }
            0xdcfbf => {
                edx = m.read32(esp.wrapping_add(0x20));
                ebx = 0;
                esp = esp.wrapping_add(0x4);
                fa = edx;
                fb = ebx;
                if (fa == fb) { pc = 0xdd121; continue; }
                pc = 0xdcfd0; continue;
            }
            0xdcfd0 => {
                eax = m.read32(esp.wrapping_add(0x20));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xdd121; continue; }
                pc = 0xdcfdd; continue;
            }
            0xdcfdd => {
                ecx = m.read32(edx.wrapping_add(0x4));
                eax = eax.wrapping_sub(m.read32(ecx));
                eax = ((eax as i32).wrapping_mul((0xc8) as i32)) as u32;
                eax = eax.wrapping_add(ecx);
                ecx = m.read32(esp.wrapping_add(0x24));
                ebp = m.read32(eax.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x60));
                fa = ebp;
                fb = ebx;
                if (fa == fb) { pc = 0xdd121; continue; }
                pc = 0xdcffa; continue;
            }
            0xdcffa => {
                push!(esi);
                esi = m.read32(edx.wrapping_add(0x4));
                push!(edi);
                fa = m.read32(esi.wrapping_add(0x8));
                fb = ebx;
                ecx = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                ecx = eax.wrapping_add(ecx.wrapping_mul(0x8));
                if (fa == fb) { pc = 0xdd029; continue; }
                pc = 0xdd00a; continue;
            }
            0xdd00a => {
                edx = m.read32(edx);
                edx = m.read32(edx.wrapping_add(0x34));
                esi = m.read32(0x708788);
                edx = ((edx as i32).wrapping_mul((0xb4) as i32)) as u32;
                fa = ((((m.read8(edx.wrapping_add(esi).wrapping_add(0xb2)))) as u32) & 0xffu32);
                fb = ((((0x80i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0xdd114; continue; }
                pc = 0xdd029; continue;
            }
            0xdd029 => {
                ecx = m.read32(ecx.wrapping_add(0x10));
                fa = ecx;
                fb = ebx;
                m.write32(esp.wrapping_add(0x1c), ecx);
                if (fa == fb) { pc = 0xdd114; continue; }
                pc = 0xdd038; continue;
            }
            0xdd038 => {
                ecx = m.read32(eax.wrapping_add(0xa8));
                edx = m.read32(esp.wrapping_add(0x34));
                eax = edx.wrapping_add(edx.wrapping_mul(0x2));
                edx = m.read32(ecx.wrapping_add(0x1c));
                edi = ((m.read16(edx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x8))) as u16 as u32);
                esi = edx.wrapping_add(eax.wrapping_mul(0x4));
                edx = m.read32(ecx.wrapping_add(0x2c));
                ecx = m.read32(esp.wrapping_add(0x30));
                esi = (((((m.read8(esi) as i8)) as i32)) as u32);
                eax = ecx.wrapping_add(ecx.wrapping_mul(0x4));
                ecx = ((m.read16(ebp.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0x6))) as u16 as u32);
                edi = edi.wrapping_add(edx);
                edx = m.read32(esp.wrapping_add(0x1c));
                eax = edx.wrapping_add(ecx.wrapping_mul(0x4));
                edx = 0;
                fa = esi;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x18), ebx);
                m.write32(esp.wrapping_add(0x14), ebx);
                m.write32(esp.wrapping_add(0x10), ebx);
                m.write32(esp.wrapping_add(0x1c), eax);
                if (fas <= fbs) { pc = 0xdd0ca; continue; }
                pc = 0xdd081; continue;
            }
            0xdd081 => {
                ecx = (((((m.read8(edx.wrapping_add(edi)) as i8)) as i32)) as u32);
                eax = m.read32(esp.wrapping_add(0x1c));
                ecx = m.read32(eax.wrapping_add(ecx.wrapping_mul(0x4)));
                ebp = m.read32(esp.wrapping_add(0x18));
                eax = ecx;
                eax = (eax & 0xff);
                ebx = ebx.wrapping_add(eax);
                eax = (((((ecx) >> 8) as u8)) as u8 as u32);
                ebp = ebp.wrapping_add(eax);
                eax = ecx;
                eax = eax.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write32(esp.wrapping_add(0x18), ebp);
                ebp = m.read32(esp.wrapping_add(0x14));
                eax = ((((eax) as u8)) as u8 as u32);
                ebp = ebp.wrapping_add(eax);
                m.write32(esp.wrapping_add(0x14), ebp);
                ebp = m.read32(esp.wrapping_add(0x10));
                m.write32(esp.wrapping_add(0x20), ecx);
                ecx = ecx.wrapping_shr((((((((0x18i32)) as u32) & 0x1fu32))) as u32 & 31));
                ebp = ebp.wrapping_add(ecx);
                edx = edx.wrapping_add(1);
                fa = edx;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), ebp);
                if (fas < fbs) { pc = 0xdd081; continue; }
                pc = 0xdd0ca; continue;
            }
            0xdd0ca => {
                eax = m.read32(esp.wrapping_add(0x10));
                edx = 0;
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (esi as u64)) as u32;
                edx = (dividend % (esi as u64)) as u32; }
                edx = 0;
                ecx = eax;
                eax = m.read32(esp.wrapping_add(0x14));
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (esi as u64)) as u32;
                edx = (dividend % (esi as u64)) as u32; }
                ecx = ecx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = 0;
                ecx = (ecx | eax);
                eax = m.read32(esp.wrapping_add(0x18));
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (esi as u64)) as u32;
                edx = (dividend % (esi as u64)) as u32; }
                ecx = ecx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = 0;
                ecx = (ecx | eax);
                eax = ebx;
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (esi as u64)) as u32;
                edx = (dividend % (esi as u64)) as u32; }
                edx = m.read32(esp.wrapping_add(0x24));
                ecx = ecx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | eax);
                eax = m.read32(edx);
                edx = m.read32(eax.wrapping_add(0x34));
                push!(edx);
                push!(ecx);
                call!(0x1164c0);
                pc = 0xdd109; continue;
            }
            0xdd109 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x34);
                break 'run;
            }
            0xdd114 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x5f5f5f5f;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x34);
                break 'run;
            }
            0xdd121 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x34);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000dd130: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn fdd130<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xdd130;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    'run: loop {
            match pc {
            0xdd130 => {
                esp = esp.wrapping_sub(0x20);
                push!(esi);
                eax = esp.wrapping_add(0x4);
                push!(eax);
                esi = 0;
                call!(0x913d0);
                pc = 0xdd140; continue;
            }
            0xdd140 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0xdd162; continue; }
                pc = 0xdd14b; continue;
            }
            0xdd14b => {
                ecx = m.read32(0x4b85c8);
                fa = eax;
                fb = m.read32(ecx.wrapping_add(0xc));
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xdd162; continue; }
                pc = 0xdd156; continue;
            }
            0xdd156 => {
                ecx = m.read32(ecx.wrapping_add(0x4));
                eax = ((m.read8(ecx.wrapping_add(eax))) as u8 as u32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x20);
                break 'run;
            }
            0xdd162 => {
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x20);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000dd170: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn fdd170<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xdd170;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    'run: loop {
            match pc {
            0xdd170 => {
                eax = esp.wrapping_add(((0x20i32.wrapping_neg()) as u32));
                esp = esp.wrapping_sub(0x20);
                push!(eax);
                call!(0x913d0);
                pc = 0xdd17d; continue;
            }
            0xdd17d => {
                eax = m.read32(esp.wrapping_add(0x1c));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0xdd19b; continue; }
                pc = 0xdd188; continue;
            }
            0xdd188 => {
                fa = m.read32(esp.wrapping_add(0x1c));
                fb = ((((0x1000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0xdd19b; continue; }
                pc = 0xdd192; continue;
            }
            0xdd192 => {
                eax = 0x1;
                esp = esp.wrapping_add(0x20);
                break 'run;
            }
            0xdd19b => {
                eax = 0;
                esp = esp.wrapping_add(0x20);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000dfd00: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn fdfd00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xdfd00;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0xdfd00 => {
                esp = esp.wrapping_sub(0x2c);
                push!(ebp);
                push!(esi);
                esi = eax;
                eax = esp.wrapping_add(0x14);
                push!(eax);
                call!(0x913d0);
                pc = 0xdfd11; continue;
            }
            0xdfd11 => {
                ebp = m.read32(esp.wrapping_add(0x2c));
                esp = esp.wrapping_add(0x4);
                fa = ebp;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xdfe47; continue; }
                pc = 0xdfd21; continue;
            }
            0xdfd21 => {
                push!(edi);
                edi = esi;
                edi = edi.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                fa = m.read32(edi.wrapping_add(0x4deb54));
                fb = ((((0x40000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0xdfe46; continue; }
                pc = 0xdfd37; continue;
            }
            0xdfd37 => {
                fa = esi;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xdfd7f; continue; }
                pc = 0xdfd3c; continue;
            }
            0xdfd3c => {
                fa = esi;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xdfd7f; continue; }
                pc = 0xdfd41; continue;
            }
            0xdfd41 => {
                eax = m.read32(esp.wrapping_add(0x40));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xdfe46; continue; }
                pc = 0xdfd4d; continue;
            }
            0xdfd4d => {
                ecx = m.read32(eax.wrapping_add(0x150));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xdfe46; continue; }
                pc = 0xdfd5b; continue;
            }
            0xdfd5b => {
                ecx = m.read32(0x7de318);
                eax = m.read32(0x73986c);
                ecx = ecx.wrapping_add(0xffffffecu32);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xdfe46; continue; }
                pc = 0xdfd71; continue;
            }
            0xdfd71 => {
                call!(0x1cdae0);
                pc = 0xdfd76; continue;
            }
            0xdfd76 => {
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xdfe46; continue; }
                pc = 0xdfd7f; continue;
            }
            0xdfd7f => {
                call!(0xdcfb0);
                pc = 0xdfd84; continue;
            }
            0xdfd84 => {
                xmm0 = Xmm::scalar(m.read32(0x475db4));
                ecx = eax;
                esi = eax;
                edx = eax;
                eax = eax.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx & 0xff00);
                ecx = (ecx | eax);
                eax = m.read32(esp.wrapping_add(0x3c));
                xmm1 = Xmm::scalar(m.read32(eax));
                esi = (esi | 0x7f000000);
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                esi = esi.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write32(esp.wrapping_add(0xc), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                edx = (edx & 0xff0000);
                esi = (esi | edx);
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                eax = m.read32(edi.wrapping_add(0x4deb40));
                esi = esi.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = (esi | ecx);
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                if (fa != fb) { pc = 0xdfe0e; continue; }
                pc = 0xdfdf1; continue;
            }
            0xdfdf1 => {
                edx = m.read32(edi.wrapping_add(0x4deb44));
                push!(esi);
                push!(edx);
                eax = esp.wrapping_add(0x14);
                push!(eax);
                push!(ebx);
                push!(0x1);
                push!(ebp);
                push!(0x4);
                push!(0);
                call!(0x108250);
                pc = 0xdfe0b; continue;
            }
            0xdfe0b => {
                esp = esp.wrapping_add(0x20);
                pc = 0xdfe0e; continue;
            }
            0xdfe0e => {
                push!(esi);
                push!(esi);
                ecx = esp.wrapping_add(0x14);
                push!(ecx);
                push!(ebx);
                push!(0x1);
                push!(ebp);
                push!(0x2);
                push!(0);
                call!(0x108250);
                pc = 0xdfe22; continue;
            }
            0xdfe22 => {
                push!(0x7f);
                push!(0x7f);
                edx = esp.wrapping_add(0x34);
                push!(edx);
                push!(ebx);
                push!(0x1);
                push!(0xffffffffu32);
                push!(0x3);
                push!(0);
                call!(0x108250);
                pc = 0xdfe39; continue;
            }
            0xdfe39 => {
                eax = m.read32(0x7de318);
                esp = esp.wrapping_add(0x40);
                m.write32(0x73986c, eax);
                pc = 0xdfe46; continue;
            }
            0xdfe46 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0xdfe47; continue;
            }
            0xdfe47 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x2c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000dfe50: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn fdfe50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xdfe50;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    'run: loop {
            match pc {
            0xdfe50 => {
                esp = esp.wrapping_sub(0x20);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xdfeb7; continue; }
                pc = 0xdfe57; continue;
            }
            0xdfe57 => {
                eax = esp;
                push!(eax);
                call!(0x913d0);
                pc = 0xdfe60; continue;
            }
            0xdfe60 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0xdfeb7; continue; }
                pc = 0xdfe6b; continue;
            }
            0xdfe6b => {
                fa = m.read32(esp.wrapping_add(0x1c));
                fb = ((((0x1000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0xdfeb7; continue; }
                pc = 0xdfe75; continue;
            }
            0xdfe75 => {
                call!(0x1cdae0);
                pc = 0xdfe7a; continue;
            }
            0xdfe7a => {
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = m.read32(0x7de318);
                if (fas <= fbs) { pc = 0xdfe91; continue; }
                pc = 0xdfe84; continue;
            }
            0xdfe84 => {
                edx = m.read32(0x7390e4);
                ecx = eax.wrapping_add(((0x6i32.wrapping_neg()) as u32));
                fa = edx;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xdfeb7; continue; }
                pc = 0xdfe91; continue;
            }
            0xdfe91 => {
                edx = m.read32(esp.wrapping_add(0x30));
                ecx = m.read32(esi.wrapping_add(0x34));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x2c));
                m.write32(0x7390e4, eax);
                eax = m.read32(esp.wrapping_add(0x30));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x2c));
                push!(esi);
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x114af0);
                pc = 0xdfeb4; continue;
            }
            0xdfeb4 => {
                esp = esp.wrapping_add(0x18);
                pc = 0xdfeb7; continue;
            }
            0xdfeb7 => {
                esp = esp.wrapping_add(0x20);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0012a170: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f12a170<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x12a170;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x12a170 => {
                esp = esp.wrapping_sub(0x54);
                push!(ebp);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x60));
                eax = m.read32(edi);
                ebp = 0;
                fa = eax;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x14), ebp);
                if (fas <= fbs) { pc = 0x12a580; continue; }
                pc = 0x12a189; continue;
            }
            0x12a189 => {
                push!(ebx);
                push!(esi);
                edi = edi.wrapping_add(0x8);
                pc = 0x12a192; continue;
            }
            0x12a190 => {
                ebp = 0;
                pc = 0x12a192; continue;
            }
            0x12a192 => {
                esi = m.read32(edi.wrapping_add(0x30));
                eax = m.read32(esi.wrapping_add(0x2c));
                fa = eax;
                fb = ebp;
                if (fa == fb) { pc = 0x12a1aa; continue; }
                pc = 0x12a19c; continue;
            }
            0x12a19c => {
                ecx = m.read32(edi);
                edx = m.read32(ecx.wrapping_add(0x4));
                fa = m.read32(edx.wrapping_add(0x30));
                fb = eax;
                if ((fa & fb) == 0) { pc = 0x12a564; continue; }
                pc = 0x12a1aa; continue;
            }
            0x12a1aa => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x28))) as u32)) & 0xff);
                ebx = (ebx | 0xffffffffu32);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                m.write32(esp.wrapping_add(0x10), ebx);
                if ((fa & fb) == 0) { pc = 0x12a212; continue; }
                pc = 0x12a1b8; continue;
            }
            0x12a1b8 => {
                eax = (((((m.read16(esi.wrapping_add(0xc)) as i16)) as i32)) as u32);
                push!(eax);
                call!(0x88e90);
                pc = 0x12a1c2; continue;
            }
            0x12a1c2 => {
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x14)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xc)))).to_bits();
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(0x10)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x10)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x18)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::default();
                esp = esp.wrapping_add(0x4);
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x12a1fc; continue; }
                pc = 0x12a1f3; continue;
            }
            0x12a1f3 => {
                ecx = m.read32(edi);
                edx = m.read32(ecx.wrapping_add(0x34));
                fa = edx;
                fb = m.read32(eax);
                if (fa == fb) { pc = 0x12a212; continue; }
                pc = 0x12a1fc; continue;
            }
            0x12a1fc => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x12a564; continue; }
                pc = 0x12a205; continue;
            }
            0x12a205 => {
                ecx = m.read32(edi);
                edx = m.read32(ecx.wrapping_add(0x34));
                fa = edx;
                fb = m.read32(eax);
                if (fa == fb) { pc = 0x12a564; continue; }
                pc = 0x12a212; continue;
            }
            0x12a212 => {
                eax = 0;
                ecx = esi.wrapping_add(0x10);
                pc = 0x12a217; continue;
            }
            0x12a217 => {
                fa = m.read32(ecx);
                fb = ebp;
                if (fa == fb) { pc = 0x12a226; continue; }
                pc = 0x12a21b; continue;
            }
            0x12a21b => {
                eax = eax.wrapping_add(1);
                ecx = ecx.wrapping_add(0x4);
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x12a217; continue; }
                pc = 0x12a224; continue;
            }
            0x12a224 => {
                pc = 0x12a22a; continue;
            }
            0x12a226 => {
                m.write32(esp.wrapping_add(0x10), eax);
                pc = 0x12a22a; continue;
            }
            0x12a22a => {
                call!(0x153b40);
                pc = 0x12a22f; continue;
            }
            0x12a22f => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x18), eax);
                fpu.push((m.read32(esp.wrapping_add(0x18)) as i32 as f64));
                if (fas >= fbs) { pc = 0x12a241; continue; }
                pc = 0x12a23b; continue;
            }
            0x12a23b => {
                { let value = round_pc((fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x12a241; continue;
            }
            0x12a241 => {
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x153b40);
                pc = 0x12a250; continue;
            }
            0x12a250 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x14), eax);
                fpu.push((m.read32(esp.wrapping_add(0x14)) as i32 as f64));
                if (fas >= fbs) { pc = 0x12a262; continue; }
                pc = 0x12a25c; continue;
            }
            0x12a25c => {
                { let value = round_pc((fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x12a262; continue;
            }
            0x12a262 => {
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x18))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(0x47d5dc)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc((fpu.st(0) + (f32::from_bits(m.read32(esi)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esi, (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = m.read32(0x7abf80);
                fa = eax;
                fb = ebp;
                if (fa != fb) { pc = 0x12a2a0; continue; }
                pc = 0x12a27f; continue;
            }
            0x12a27f => {
                call!(0x153b40);
                pc = 0x12a284; continue;
            }
            0x12a284 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x18), eax);
                fpu.push((m.read32(esp.wrapping_add(0x18)) as i32 as f64));
                if (fas >= fbs) { pc = 0x12a296; continue; }
                pc = 0x12a290; continue;
            }
            0x12a290 => {
                { let value = round_pc((fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x12a296; continue;
            }
            0x12a296 => {
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc((fpu.st(0) + (f32::from_bits(m.read32(esi)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esi, (fpu.st(0) as f32).to_bits()); fpu.pop();
                pc = 0x12a2a0; continue;
            }
            0x12a2a0 => {
                xmm1 = Xmm::scalar(m.read32(esi));
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca >= fcb)) { pc = 0x12a2c6; continue; }
                pc = 0x12a2b1; continue;
            }
            0x12a2b1 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x70)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca >= fcb)) { pc = 0x12a2c6; continue; }
                pc = 0x12a2bc; continue;
            }
            0x12a2bc => {
                fa = m.read32(esp.wrapping_add(0x10));
                fb = ebx;
                m.write32(esp.wrapping_add(0x14), ebp);
                if (fa != fb) { pc = 0x12a2ce; continue; }
                pc = 0x12a2c6; continue;
            }
            0x12a2c6 => {
                m.write32(esp.wrapping_add(0x14), 0x1);
                pc = 0x12a2ce; continue;
            }
            0x12a2ce => {
                xmm0 = Xmm::scalar(m.read32(esi));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x12a2eb; continue; }
                pc = 0x12a2db; continue;
            }
            0x12a2db => {
                fa = ((((m.read16(esi.wrapping_add(0x28)))) as u32) & 0xffffu32);
                fb = ((((0x40ci32)) as u32) & 0xffffu32);
                m.write32(esp.wrapping_add(0x18), 0x1);
                if ((fa & fb) == 0) { pc = 0x12a2ef; continue; }
                pc = 0x12a2eb; continue;
            }
            0x12a2eb => {
                m.write32(esp.wrapping_add(0x18), ebp);
                pc = 0x12a2ef; continue;
            }
            0x12a2ef => {
                ebx = m.read32(esp.wrapping_add(0x6c));
                ecx = 0;
                ecx = (ecx & 0xffff_0000) | ((((m.read16(esi.wrapping_add(0x28))) as u32)) & 0xffff);
                fa = ecx;
                fb = ((((0x40ci32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x12a322; continue; }
                pc = 0x12a301; continue;
            }
            0x12a301 => {
                fa = ebx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x12a322; continue; }
                pc = 0x12a306; continue;
            }
            0x12a306 => {
                eax = ebx;
                eax = eax.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = m.read32(eax.wrapping_add(0x4deb54));
                fa = (((((((edx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x6i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x12a31b; continue; }
                pc = 0x12a316; continue;
            }
            0x12a316 => {
                fa = ebx;
                fb = ((((0x15i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x12a322; continue; }
                pc = 0x12a31b; continue;
            }
            0x12a31b => {
                eax = 0x1;
                pc = 0x12a324; continue;
            }
            0x12a322 => {
                eax = 0;
                pc = 0x12a324; continue;
            }
            0x12a324 => {
                fa = m.read32(esp.wrapping_add(0x14));
                fb = ebp;
                if (fa == fb) { pc = 0x12a330; continue; }
                pc = 0x12a32a; continue;
            }
            0x12a32a => {
                fa = m.read32(esp.wrapping_add(0x18));
                fb = ebp;
                if (fa != fb) { pc = 0x12a33e; continue; }
                pc = 0x12a330; continue;
            }
            0x12a330 => {
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x12a33e; continue; }
                pc = 0x12a335; continue;
            }
            0x12a335 => {
                fa = eax;
                fb = ebp;
                if (fa != fb) { pc = 0x12a33e; continue; }
                pc = 0x12a339; continue;
            }
            0x12a339 => {
                ebp = 0x1;
                pc = 0x12a33e; continue;
            }
            0x12a33e => {
                edx = m.read32(0x7abf80);
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x12a4cc; continue; }
                pc = 0x12a34c; continue;
            }
            0x12a34c => {
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x12a4cc; continue; }
                pc = 0x12a355; continue;
            }
            0x12a355 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x12a4cc; continue; }
                pc = 0x12a35d; continue;
            }
            0x12a35d => {
                fa = ebx;
                fb = ((((0x14i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x94)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x40)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(edi.wrapping_add(0x8)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x44)))).to_bits();
                xmm4 = Xmm::scalar(m.read32(esi.wrapping_add(0x60)));
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x44)))).to_bits();
                xh[0] = Xmm::scalar(m.read32(esi.wrapping_add(0x64)));
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x48)))).to_bits();
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x40)))).to_bits();
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x5c)));
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x40)))).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x98)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x44)))).to_bits();
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x9c)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x48)))).to_bits();
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x48)))).to_bits();
                xh[1] = xmm0;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[2] = xmm2;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = xmm1;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[0] = xmm4;
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = xmm3;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm4;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = xmm3;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm2;
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::divss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x20), xh[1].lanes[0]);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                if (fa == fb) { pc = 0x12a4bb; continue; }
                pc = 0x12a468; continue;
            }
            0x12a468 => {
                ecx = esp.wrapping_add(0x34);
                push!(ecx);
                call!(0x45390);
                pc = 0x12a472; continue;
            }
            0x12a472 => {
                xmm0 = Xmm::scalar(m.read32(0x475d28));
                m.write32(esp.wrapping_add(0x64), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x54), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x8)));
                edx = esp.wrapping_add(0x38);
                m.write32(esp.wrapping_add(0x58), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0xc)));
                push!(edx);
                m.write8(esp.wrapping_add(0x64), (0x1) as u8);
                m.write32(esp.wrapping_add(0x3c), 0xd4f);
                m.write32(esp.wrapping_add(0x60), xmm0.lanes[0]);
                call!(0x45820);
                pc = 0x12a4b8; continue;
            }
            0x12a4b8 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x12a4bb; continue;
            }
            0x12a4bb => {
                push!(esi);
                ebx = esp.wrapping_add(0x24);
                call!(0x1296a0);
                pc = 0x12a4c5; continue;
            }
            0x12a4c5 => {
                ebx = m.read32(esp.wrapping_add(0x70));
                esp = esp.wrapping_add(0x4);
                pc = 0x12a4cc; continue;
            }
            0x12a4cc => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x12a518; continue; }
                pc = 0x12a4d0; continue;
            }
            0x12a4d0 => {
                push!(ebx);
                call!(0xdc290);
                pc = 0x12a4d6; continue;
            }
            0x12a4d6 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x12a564; continue; }
                pc = 0x12a4e1; continue;
            }
            0x12a4e1 => {
                fa = ((((m.read8(esi.wrapping_add(0x29)))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x12a564; continue; }
                pc = 0x12a4eb; continue;
            }
            0x12a4eb => {
                ebp = m.read32(esp.wrapping_add(0x10));
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x12a564; continue; }
                pc = 0x12a4f3; continue;
            }
            0x12a4f3 => {
                eax = ((m.read16(esi.wrapping_add(0x2a))) as u16 as u32);
                push!(eax);
                ecx = esi.wrapping_add(0x40);
                push!(ecx);
                ecx = m.read32(edi);
                edx = edi.wrapping_add(0x1c);
                push!(edx);
                eax = edi.wrapping_add(0x4);
                push!(eax);
                push!(0);
                push!(ecx);
                push!(ebx);
                call!(0x115070);
                pc = 0x12a50f; continue;
            }
            0x12a50f => {
                esp = esp.wrapping_add(0x1c);
                m.write32(esi.wrapping_add(ebp.wrapping_mul(0x4)).wrapping_add(0x10), eax);
                pc = 0x12a564; continue;
            }
            0x12a518 => {
                eax = m.read32(esp.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x12a564; continue; }
                pc = 0x12a520; continue;
            }
            0x12a520 => {
                eax = m.read32(esp.wrapping_add(0x18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x12a564; continue; }
                pc = 0x12a528; continue;
            }
            0x12a528 => {
                edx = m.read32(esp.wrapping_add(0x70));
                push!(edx);
                ebx = edi.wrapping_add(0x10);
                push!(ebx);
                ebp = edi.wrapping_add(0x1c);
                push!(ebp);
                eax = edi.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(edi);
                push!(esi);
                push!(eax);
                call!(0x129f50);
                pc = 0x12a542; continue;
            }
            0x12a542 => {
                ecx = m.read32(esp.wrapping_add(0x8c));
                edx = m.read32(esp.wrapping_add(0x88));
                push!(ecx);
                push!(edx);
                push!(ebx);
                push!(ebp);
                eax = edi.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(edi);
                push!(esi);
                push!(eax);
                call!(0x389b0);
                pc = 0x12a561; continue;
            }
            0x12a561 => {
                esp = esp.wrapping_add(0x34);
                pc = 0x12a564; continue;
            }
            0x12a564 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                ecx = m.read32(esp.wrapping_add(0x68));
                edx = m.read32(ecx);
                eax = eax.wrapping_add(1);
                edi = edi.wrapping_add(0x34);
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                if (fas < fbs) { pc = 0x12a190; continue; }
                pc = 0x12a57e; continue;
            }
            0x12a57e => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x12a580; continue;
            }
            0x12a580 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x54);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0012a590: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f12a590<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x12a590;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x12a590 => {
                esp = esp.wrapping_sub(0x78);
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x80));
                ecx = m.read32(ebx.wrapping_add(0x1f8));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x8), ecx);
                if (fa == fb) { pc = 0x12aa8a; continue; }
                pc = 0x12a5ad; continue;
            }
            0x12a5ad => {
                edx = m.read32(ebx.wrapping_add(0x1c));
                eax = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x4), eax);
                if (fas <= fbs) { pc = 0x12aa8a; continue; }
                pc = 0x12a5be; continue;
            }
            0x12a5be => {
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x90));
                push!(esi);
                push!(edi);
                pc = 0x12a5d0; continue;
            }
            0x12a5ca => {
                ecx = m.read32(esp.wrapping_add(0x14));
                edi = edi;
                pc = 0x12a5d0; continue;
            }
            0x12a5d0 => {
                edi = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)));
                xmm0 = Xmm::scalar(m.read32(edi));
                xh[1] = Xmm::default();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xh[1].lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x12aa73; continue; }
                pc = 0x12a5e3; continue;
            }
            0x12a5e3 => {
                ecx = ((m.read16(edi.wrapping_add(0x2a))) as u16 as u32);
                xh[2] = Xmm::scalar(m.read32(0x475c78));
                esi = 0;
                fa = ecx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm4 = xh[2];
                xmm1 = xh[1];
                xmm2 = xh[1];
                xmm3 = xh[1];
                xmm4.lanes[0] = sse::divss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                if (fas < fbs) { pc = 0x12a691; continue; }
                pc = 0x12a60e; continue;
            }
            0x12a60e => {
                edx = ecx.wrapping_add(((0x4i32.wrapping_neg()) as u32));
                edx = edx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_add(1);
                eax = edi.wrapping_add(0x44);
                esi = edx.wrapping_mul(0x4);
                pc = 0x12a620; continue;
            }
            0x12a620 => {
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(((0x4i32.wrapping_neg()) as u32))));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0x18)));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x1c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x20)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x34)));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0x38)));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x3c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x50)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x54)));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0x58)));
                eax = eax.wrapping_add(0x70);
                edx = edx.wrapping_sub(1);
                fa = edx;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                if ((fa != 0)) { pc = 0x12a620; continue; }
                pc = 0x12a691; continue;
            }
            0x12a691 => {
                fa = esi;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x12a6c9; continue; }
                pc = 0x12a695; continue;
            }
            0x12a695 => {
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x1c) as i32)) as u32;
                eax = eax.wrapping_add(edi).wrapping_add(0x44);
                ecx = ecx.wrapping_sub(esi);
                pc = 0x12a6a0; continue;
            }
            0x12a6a0 => {
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(((0x4i32.wrapping_neg()) as u32))));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm0;
                xmm0 = Xmm::scalar(m.read32(eax));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xmm0;
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                eax = eax.wrapping_add(0x1c);
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm0;
                if ((fa != 0)) { pc = 0x12a6a0; continue; }
                pc = 0x12a6c9; continue;
            }
            0x12a6c9 => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = xmm4;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm4;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm2 = xmm4;
                if (fa == fb) { pc = 0x12a72a; continue; }
                pc = 0x12a6e2; continue;
            }
            0x12a6e2 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(ebp)) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x12aa73; continue; }
                pc = 0x12a6ec; continue;
            }
            0x12a6ec => {
                xmm3 = Xmm::scalar(m.read32(ebp.wrapping_add(0xc)));
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x12aa73; continue; }
                pc = 0x12a6fa; continue;
            }
            0x12a6fa => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(ebp.wrapping_add(0x4))) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x12aa73; continue; }
                pc = 0x12a704; continue;
            }
            0x12a704 => {
                xmm3 = Xmm::scalar(m.read32(ebp.wrapping_add(0x10)));
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x12aa73; continue; }
                pc = 0x12a712; continue;
            }
            0x12a712 => {
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(ebp.wrapping_add(0x8))) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x12aa73; continue; }
                pc = 0x12a71c; continue;
            }
            0x12a71c => {
                xmm3 = Xmm::scalar(m.read32(ebp.wrapping_add(0x14)));
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x12aa73; continue; }
                pc = 0x12a72a; continue;
            }
            0x12a72a => {
                ecx = m.read32(esp.wrapping_add(0x90));
                eax = m.read32(ecx.wrapping_add(0x28));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                xh[0] = xmm2;
                m.write32(esp.wrapping_add(0x3c), xh[0].lanes[0]);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x3c))) as f64));
                m.write32(esp.wrapping_add(0x38), xmm1.lanes[0]);
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x3c))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm4 = xmm0;
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x38))) as f64));
                m.write32(esp.wrapping_add(0x34), xmm4.lanes[0]);
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x38))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm0 = xh[2];
                { let value = round_pc((fpu.st(1) + fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 1) & 7) as usize] = value; } fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x34))) as f64));
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x34))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc((fpu.st(1) + fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 1) & 7) as usize] = value; } fpu.pop();
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::divss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x54)))).to_bits();
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x34), xmm3.lanes[0]);
                xmm3 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                xmm0 = xh[2];
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x94)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4785e4))).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xh[1].lanes[0]) as f64;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x38), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x12aa73; continue; }
                pc = 0x12a7d9; continue;
            }
            0x12a7d9 => {
                call!(0x153b40);
                pc = 0x12a7de; continue;
            }
            0x12a7de => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x18), eax);
                fpu.push((m.read32(esp.wrapping_add(0x18)) as i32 as f64));
                if (fas >= fbs) { pc = 0x12a7f0; continue; }
                pc = 0x12a7ea; continue;
            }
            0x12a7ea => {
                { let value = round_pc((fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x12a7f0; continue;
            }
            0x12a7f0 => {
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x2c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x153b40);
                pc = 0x12a7ff; continue;
            }
            0x12a7ff => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x18), eax);
                fpu.push((m.read32(esp.wrapping_add(0x18)) as i32 as f64));
                if (fas >= fbs) { pc = 0x12a811; continue; }
                pc = 0x12a80b; continue;
            }
            0x12a80b => {
                { let value = round_pc((fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x12a811; continue;
            }
            0x12a811 => {
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(0x78)));
                xmm3 = Xmm::scalar(m.read32(edi.wrapping_add(0x5c)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x40)))).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x40)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x7c)));
                xmm4 = Xmm::scalar(m.read32(edi.wrapping_add(0x60)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x44)))).to_bits();
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x44)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(edi.wrapping_add(0x80)));
                xh[0] = Xmm::scalar(m.read32(edi.wrapping_add(0x64)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x48)))).to_bits();
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x48)))).to_bits();
                eax = edi.wrapping_add(0x40);
                m.write32(esp.wrapping_add(0x30), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xh[1] = xmm1;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                m.write32(esp.wrapping_add(0x18), xh[1].lanes[0]);
                xh[2] = xmm3;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                m.write32(esp.wrapping_add(0x4c), xh[1].lanes[0]);
                xh[1] = xmm0;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                m.write32(esp.wrapping_add(0x18), xh[1].lanes[0]);
                ecx = 0;
                ecx = (ecx & 0xffff_0000) | ((((m.read16(edi.wrapping_add(0x28))) as u32)) & 0xffff);
                xh[2] = xmm4;
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x44)))).to_bits();
                m.write32(esp.wrapping_add(0x50), xh[1].lanes[0]);
                xh[1] = xmm2;
                fa = ecx;
                fb = ((((0x40ci32)) as u32) & 0xffffffffu32);
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                m.write32(esp.wrapping_add(0x18), xh[1].lanes[0]);
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x48)))).to_bits();
                m.write32(esp.wrapping_add(0x54), xh[1].lanes[0]);
                if ((fa & fb) != 0) { pc = 0x12a9f5; continue; }
                pc = 0x12a8e3; continue;
            }
            0x12a8e3 => {
                xh[2] = xmm0;
                xh[1] = xmm2;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x40), xh[1].lanes[0]);
                xh[1] = xmm1;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                push!(eax);
                m.write32(esp.wrapping_add(0x48), xh[1].lanes[0]);
                m.write32(esp.wrapping_add(0x4c), xmm0.lanes[0]);
                call!(0x129c20);
                pc = 0x12a928; continue;
            }
            0x12a928 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x20));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                ecx = (ecx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x1c), ecx);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(0x475cd4));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x44), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                m.write32(esp.wrapping_add(0x24), xmm1.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x24));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                edx = (edx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x1c), edx);
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                edx = m.read32(esp.wrapping_add(0x2c));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x4c)));
                m.write32(esp.wrapping_add(0x28), xmm1.lanes[0]);
                eax = m.read32(esp.wrapping_add(0x28));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                eax = (eax & 0x7fffffff);
                ecx = esp.wrapping_add(0x50);
                push!(ecx);
                m.write32(esp.wrapping_add(0x4c), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x20), eax);
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                push!(edx);
                eax = esp.wrapping_add(0x4c);
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                push!(eax);
                ecx = esp.wrapping_add(0x3c);
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                push!(ecx);
                m.write32(esp.wrapping_add(0x5c), xmm2.lanes[0]);
                call!(0x129ca0);
                pc = 0x12a9f0; continue;
            }
            0x12a9f0 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x12aa6c; continue;
            }
            0x12a9f5 => {
                eax = m.read32(0x7abf80);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x12aa73; continue; }
                pc = 0x12a9fe; continue;
            }
            0x12a9fe => {
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x12aa73; continue; }
                pc = 0x12aa03; continue;
            }
            0x12aa03 => {
                edx = esp.wrapping_add(0x58);
                push!(edx);
                call!(0x45390);
                pc = 0x12aa0d; continue;
            }
            0x12aa0d => {
                xmm0 = Xmm::scalar(m.read32(0x475d28));
                m.write32(esp.wrapping_add(0x88), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                m.write32(esp.wrapping_add(0x78), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x54)));
                eax = esp.wrapping_add(0x5c);
                m.write32(esp.wrapping_add(0x7c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x58)));
                push!(eax);
                m.write8(esp.wrapping_add(0x88), (0x1) as u8);
                m.write32(esp.wrapping_add(0x60), 0xd4f);
                m.write32(esp.wrapping_add(0x84), xmm0.lanes[0]);
                call!(0x45820);
                pc = 0x12aa5f; continue;
            }
            0x12aa5f => {
                push!(edi);
                ebx = esp.wrapping_add(0x38);
                call!(0x1296a0);
                pc = 0x12aa69; continue;
            }
            0x12aa69 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x12aa6c; continue;
            }
            0x12aa6c => {
                ebx = m.read32(esp.wrapping_add(0x8c));
                pc = 0x12aa73; continue;
            }
            0x12aa73 => {
                eax = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(ebx.wrapping_add(0x1c));
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), eax);
                if (fas < fbs) { pc = 0x12a5ca; continue; }
                pc = 0x12aa87; continue;
            }
            0x12aa87 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x12aa8a; continue;
            }
            0x12aa8a => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x78);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00166060: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f166060<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x166060;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x166060 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(0x74c458);
                eax = (eax & 0xffff);
                eax = eax.wrapping_shl((((((((0x9i32)) as u32) & 0x1fu32))) as u32 & 31));
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(ecx).wrapping_add(0x138)));
                eax = eax.wrapping_add(ecx);
                ecx = m.read32(esp.wrapping_add(0x8));
                xmm1 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x144)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(ecx.wrapping_add(0x8)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x148)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(ecx));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x140)))).to_bits();
                ecx = m.read32(esp.wrapping_add(0xc));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm4 = xmm1;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x13c)))).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(ecx, xmm3.lanes[0]);
                xmm4 = Xmm::scalar(m.read32(eax.wrapping_add(0x134)));
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x13c)))).to_bits();
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(ecx.wrapping_add(0x4), xmm3.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x134)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x138)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(ecx.wrapping_add(0x8), xmm2.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x14c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                m.write32(ecx, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x150)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(ecx.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x154)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(ecx.wrapping_add(0x8), xmm0.lanes[0]);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00166140: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f166140<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x166140;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x166140 => {
                eax = m.read32(esp.wrapping_add(0x4));
                edx = m.read32(esp.wrapping_add(0xc));
                ecx = m.read32(0x74c458);
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(0x8)));
                eax = (eax & 0xffff);
                eax = eax.wrapping_shl((((((((0x9i32)) as u32) & 0x1fu32))) as u32 & 31));
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(ecx).wrapping_add(0x148)));
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(ecx).wrapping_add(0x140)));
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(ecx).wrapping_add(0x144)));
                xmm4 = Xmm::scalar(m.read32(eax.wrapping_add(ecx).wrapping_add(0x154)));
                eax = eax.wrapping_add(ecx);
                ecx = m.read32(esp.wrapping_add(0x8));
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x4)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(edx));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(edx));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(edx.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edx.wrapping_add(0x4)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x140)));
                ecx = m.read32(esp.wrapping_add(0x10));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x144)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x148)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0x150)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(ecx, xmm3.lanes[0]);
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0x154)));
                xmm4 = Xmm::scalar(m.read32(eax.wrapping_add(0x14c)));
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(ecx.wrapping_add(0x4), xmm3.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x14c)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x150)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(ecx.wrapping_add(0x8), xmm1.lanes[0]);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0016a2f0: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f16a2f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x16a2f0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x16a2f0 => {
                esp = esp.wrapping_sub(0x34);
                push!(esi);
                esi = ecx;
                eax = eax.wrapping_add(0x100);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(eax);
                if (fa == fb) { pc = 0x16a344; continue; }
                pc = 0x16a300; continue;
            }
            0x16a300 => {
                eax = esp.wrapping_add(0x1c);
                push!(eax);
                call!(0x150920);
                pc = 0x16a30a; continue;
            }
            0x16a30a => {
                esi = esi.wrapping_add(0x100);
                push!(esi);
                ecx = esp.wrapping_add(0x24);
                push!(ecx);
                edx = esp.wrapping_add(0x38);
                push!(edx);
                call!(0x150830);
                pc = 0x16a320; continue;
            }
            0x16a320 => {
                eax = esp.wrapping_add(0x2c);
                push!(edi);
                push!(eax);
                call!(0x150920);
                pc = 0x16a32b; continue;
            }
            0x16a32b => {
                ecx = esp.wrapping_add(0x34);
                push!(ecx);
                edx = esp.wrapping_add(0x48);
                push!(edx);
                eax = esp.wrapping_add(0x2c);
                push!(eax);
                call!(0x150830);
                pc = 0x16a33f; continue;
            }
            0x16a33f => {
                esp = esp.wrapping_add(0x28);
                pc = 0x16a370; continue;
            }
            0x16a344 => {
                ecx = esp.wrapping_add(0x1c);
                push!(ecx);
                call!(0x150920);
                pc = 0x16a34e; continue;
            }
            0x16a34e => {
                edx = esp.wrapping_add(0x30);
                push!(edi);
                push!(edx);
                call!(0x150920);
                pc = 0x16a359; continue;
            }
            0x16a359 => {
                eax = esp.wrapping_add(0x38);
                push!(eax);
                ecx = esp.wrapping_add(0x2c);
                push!(ecx);
                edx = esp.wrapping_add(0x20);
                push!(edx);
                call!(0x150830);
                pc = 0x16a36d; continue;
            }
            0x16a36d => {
                esp = esp.wrapping_add(0x1c);
                pc = 0x16a370; continue;
            }
            0x16a370 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                xmm1 = Xmm::scalar(m.read32(ebx.wrapping_add(0x8)));
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0xc))) as f64));
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0x4)));
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0xc))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xc)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(ebx));
                { let value = round_pc((fpu.st(1) + fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 1) & 7) as usize] = value; } fpu.pop();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x8)))).to_bits();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x8))) as f64));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0x8))) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm0 = Xmm::default();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                { let value = round_pc((fpu.st(1) + fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 1) & 7) as usize] = value; } fpu.pop();
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x16a3c5; continue; }
                pc = 0x16a3bf; continue;
            }
            0x16a3bf => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                pc = 0x16a3cb; continue;
            }
            0x16a3c5 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64));
                fpu.stack[fpu.top as usize] = -fpu.st(0);
                pc = 0x16a3cb; continue;
            }
            0x16a3cb => {
                { let value = fpu.st(1).atan2(fpu.st(0)); fpu.stack[((fpu.top + 1) & 7) as usize] = value; fpu.pop(); }
                xmm2 = Xmm::scalar(m.read32(0x475c94));
                m.write32(esp, (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm1 = Xmm::scalar(m.read32(esp));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475c90)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x16a3f2; continue; }
                pc = 0x16a3ee; continue;
            }
            0x16a3ee => {
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                pc = 0x16a3f2; continue;
            }
            0x16a3f2 => {
                xmm3 = Xmm::scalar(m.read32(0x475c98));
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x16a403; continue; }
                pc = 0x16a3ff; continue;
            }
            0x16a3ff => {
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                pc = 0x16a403; continue;
            }
            0x16a403 => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                esp = esp.wrapping_add(0x34);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0016b380: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f16b380<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x16b380;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x16b380 => {
                esp = esp.wrapping_sub(0xc);
                call!(0x16b130);
                pc = 0x16b388; continue;
            }
            0x16b388 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x8), eax);
                if (fa == fb) { pc = 0x16b54a; continue; }
                pc = 0x16b395; continue;
            }
            0x16b395 => {
                ecx = m.read32(esp.wrapping_add(0x10));
                edx = m.read32(0x74c458);
                push!(ebx);
                push!(ebp);
                push!(esi);
                ebp = ecx;
                ebp = (ebp & 0xffff);
                esi = eax;
                eax = m.read32(esp.wrapping_add(0x20));
                push!(edi);
                edi = m.read32(0x74c464);
                esi = esi.wrapping_shl((((((((0x7i32)) as u32) & 0x1fu32))) as u32 & 31));
                ebp = ebp.wrapping_shl((((((((0x9i32)) as u32) & 0x1fu32))) as u32 & 31));
                ebx = 0;
                esi = esi.wrapping_add(edi);
                ebp = ebp.wrapping_add(edx);
                fa = eax;
                fb = ebx;
                if (fa == fb) { pc = 0x16b3da; continue; }
                pc = 0x16b3c7; continue;
            }
            0x16b3c7 => {
                edi = eax;
                edi = (edi & 0xffff);
                edi = edi.wrapping_shl((((((((0x9i32)) as u32) & 0x1fu32))) as u32 & 31));
                edi = edi.wrapping_add(edx);
                m.write32(esp.wrapping_add(0x10), edi);
                pc = 0x16b3e0; continue;
            }
            0x16b3da => {
                m.write32(esp.wrapping_add(0x10), ebx);
                edi = ebx;
                pc = 0x16b3e0; continue;
            }
            0x16b3e0 => {
                xmm0 = Xmm::scalar(m.read32(0x478618));
                m.write32(esi.wrapping_add(0x40), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475d5c));
                m.write32(esi.wrapping_add(0x44), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x4785e4));
                m.write32(esi.wrapping_add(0x5c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475d60));
                m.write32(esi.wrapping_add(0x60), xmm0.lanes[0]);
                xmm0 = Xmm::default();
                m.write32(esi.wrapping_add(0x7c), ebx);
                ebx = m.read32(esp.wrapping_add(0x28));
                m.write32(esi.wrapping_add(0x58), 0x3);
                m.write32(esi.wrapping_add(0x70), ecx);
                m.write32(esi.wrapping_add(0x74), eax);
                m.write32(esi.wrapping_add(0x64), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x6c), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x68), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebx));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x140)))).to_bits();
                m.write32(esi, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x144)))).to_bits();
                m.write32(esi.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ebp.wrapping_add(0x148)))).to_bits();
                push!(esi);
                push!(ebp);
                m.write32(esi.wrapping_add(0x8), xmm0.lanes[0]);
                call!(0x1521d0);
                pc = 0x16b475; continue;
            }
            0x16b475 => {
                ecx = m.read32(esp.wrapping_add(0x34));
                edx = m.read32(ecx);
                eax = esi.wrapping_add(0x20);
                m.write32(eax, edx);
                edx = m.read32(ecx.wrapping_add(0x4));
                m.write32(esi.wrapping_add(0x24), edx);
                ecx = m.read32(ecx.wrapping_add(0x8));
                push!(eax);
                push!(ebp);
                m.write32(esp.wrapping_add(0x24), eax);
                m.write32(esi.wrapping_add(0x28), ecx);
                call!(0x1521d0);
                pc = 0x16b497; continue;
            }
            0x16b497 => {
                esp = esp.wrapping_add(0x10);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16b4ff; continue; }
                pc = 0x16b49e; continue;
            }
            0x16b49e => {
                xmm0 = Xmm::scalar(m.read32(ebx));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x140)))).to_bits();
                eax = esi.wrapping_add(0x10);
                m.write32(eax, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x144)))).to_bits();
                m.write32(esi.wrapping_add(0x14), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x148)))).to_bits();
                push!(eax);
                push!(edi);
                m.write32(esi.wrapping_add(0x18), xmm0.lanes[0]);
                call!(0x1521d0);
                pc = 0x16b4dc; continue;
            }
            0x16b4dc => {
                ecx = m.read32(esp.wrapping_add(0x34));
                edx = m.read32(ecx);
                eax = esi.wrapping_add(0x30);
                m.write32(eax, edx);
                edx = m.read32(ecx.wrapping_add(0x4));
                m.write32(esi.wrapping_add(0x34), edx);
                ecx = m.read32(ecx.wrapping_add(0x8));
                push!(eax);
                push!(edi);
                m.write32(esi.wrapping_add(0x38), ecx);
                call!(0x1521d0);
                pc = 0x16b4fa; continue;
            }
            0x16b4fa => {
                esp = esp.wrapping_add(0x10);
                pc = 0x16b525; continue;
            }
            0x16b4ff => {
                edx = m.read32(ebx);
                m.write32(esi.wrapping_add(0x10), edx);
                eax = m.read32(ebx.wrapping_add(0x4));
                m.write32(esi.wrapping_add(0x14), eax);
                ecx = m.read32(ebx.wrapping_add(0x8));
                eax = m.read32(esp.wrapping_add(0x2c));
                m.write32(esi.wrapping_add(0x18), ecx);
                edx = m.read32(eax);
                m.write32(esi.wrapping_add(0x30), edx);
                ecx = m.read32(eax.wrapping_add(0x4));
                m.write32(esi.wrapping_add(0x34), ecx);
                edx = m.read32(eax.wrapping_add(0x8));
                m.write32(esi.wrapping_add(0x38), edx);
                pc = 0x16b525; continue;
            }
            0x16b525 => {
                ecx = edi;
                eax = ebp;
                edx = esi;
                call!(0x16b330);
                pc = 0x16b530; continue;
            }
            0x16b530 => {
                ebx = m.read32(esp.wrapping_add(0x14));
                ecx = m.read32(esp.wrapping_add(0x10));
                edi = esi.wrapping_add(0x48);
                eax = ebp;
                call!(0x16a2f0);
                pc = 0x16b542; continue;
            }
            0x16b542 => {
                eax = m.read32(esp.wrapping_add(0x18));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x16b54a; continue;
            }
            0x16b54a => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

// portreg: va=0x0016c300
/// 0x0016c300: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f16c300<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x16c300;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    'run: loop {
            match pc {
            0x16c300 => {
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x14));
                esi = edi;
                push!(edi);
                esi = (esi & 0xffff);
                call!(0x1677e0);
                pc = 0x16c316; continue;
            }
            0x16c316 => {
                ecx = m.read32(0x74c458);
                ebp = (ebp & 0xffff_0000) | ((((m.read16(0x74c470)) as u32)) & 0xffff);
                eax = esi;
                eax = eax.wrapping_shl((((((((0x9i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(ecx);
                ecx = m.read32(0x74c464);
                esp = esp.wrapping_add(0x4);
                ecx = ecx.wrapping_add(0x58);
                edx = 0x5d;
                ebx = 0;
                pc = 0x16c340; continue;
            }
            0x16c340 => {
                fa = m.read32(ecx.wrapping_add(0x18));
                fb = edi;
                if (fa == fb) { pc = 0x16c34a; continue; }
                pc = 0x16c345; continue;
            }
            0x16c345 => {
                fa = m.read32(ecx.wrapping_add(0x1c));
                fb = edi;
                if (fa != fb) { pc = 0x16c358; continue; }
                pc = 0x16c34a; continue;
            }
            0x16c34a => {
                fa = m.read32(ecx);
                fb = ebx;
                if (fa == fb) { pc = 0x16c358; continue; }
                pc = 0x16c34e; continue;
            }
            0x16c34e => {
                ebp = (ebp & 0xffff_0000) | ((((((((ebp) as u16)) as i32).wrapping_sub(0x1i32)) as u32)) & 0xffff);
                m.write32(ecx, ebx);
                m.write32(ecx.wrapping_add(0x20), ebx);
                m.write32(ecx.wrapping_add(0x24), ebx);
                pc = 0x16c358; continue;
            }
            0x16c358 => {
                ecx = ecx.wrapping_add(0x80);
                edx = edx.wrapping_sub(1);
                fa = edx;
                if ((fa != 0)) { pc = 0x16c340; continue; }
                pc = 0x16c361; continue;
            }
            0x16c361 => {
                ecx = m.read32(eax.wrapping_add(0x1f0));
                fa = ecx;
                fb = ebx;
                write16(m, 0x74c470, (((((ebp) as u16)) as u32)) as u16);
                if (fa != fb) { pc = 0x16c380; continue; }
                pc = 0x16c372; continue;
            }
            0x16c372 => {
                ecx = m.read32(eax.wrapping_add(0x1f4));
                m.write32(0x7a20cc, ecx);
                pc = 0x16c38c; continue;
            }
            0x16c380 => {
                edx = m.read32(eax.wrapping_add(0x1f4));
                m.write32(ecx.wrapping_add(0x1f4), edx);
                pc = 0x16c38c; continue;
            }
            0x16c38c => {
                ecx = m.read32(eax.wrapping_add(0x1f4));
                fa = ecx;
                fb = ebx;
                if (fa == fb) { pc = 0x16c3a2; continue; }
                pc = 0x16c396; continue;
            }
            0x16c396 => {
                edx = m.read32(eax.wrapping_add(0x1f0));
                m.write32(ecx.wrapping_add(0x1f0), edx);
                pc = 0x16c3a2; continue;
            }
            0x16c3a2 => {
                ecx = m.read32(0x74c46c);
                m.write32(eax.wrapping_add(0x1f4), ebx);
                m.write32(eax.wrapping_add(0x1f0), ebx);
                m.write32(ecx.wrapping_add(esi.wrapping_mul(0x4)), ebx);
                ecx = m.read32(0x74c480);
                m.write32(ecx.wrapping_add(esi.wrapping_mul(0x4)), (m.read32(ecx.wrapping_add(esi.wrapping_mul(0x4))) | 0x10000));
                ecx = ((m.read8(eax.wrapping_add(0x1cd))) as u8 as u32);
                ecx = ecx.wrapping_sub(ebx);
                fa = ecx;
                edx = (((((m.read16(0x74c45c) as i16)) as i32)) as u32);
                if ((fa == 0)) { pc = 0x16c3e3; continue; }
                pc = 0x16c3d6; continue;
            }
            0x16c3d6 => {
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa == 0)) { pc = 0x16c401; continue; }
                pc = 0x16c3d9; continue;
            }
            0x16c3d9 => {
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x16c3fc; continue; }
                pc = 0x16c3dc; continue;
            }
            0x16c3dc => {
                edx = (((((m.read16(0x74c460) as i16)) as i32)) as u32);
                pc = 0x16c3e3; continue;
            }
            0x16c3e3 => {
                ecx = m.read32(0x74c488);
                write16(m, ecx.wrapping_add(edx.wrapping_mul(0x2)), (((((esi) as u16)) as u32)) as u16);
                eax = ((m.read8(eax.wrapping_add(0x1cd))) as u8 as u32);
                write16(m, eax.wrapping_mul(0x2).wrapping_add(0x74c45c), (((((m.read16(eax.wrapping_mul(0x2).wrapping_add(0x74c45c))) as i32).wrapping_add(0x1i32)) as u32)) as u16);
                pc = 0x16c3fc; continue;
            }
            0x16c3fc => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x16c401 => {
                write16(m, 0x74c45e, (((((m.read16(0x74c45e)) as i32).wrapping_sub(0x1i32)) as u32)) as u16);
                edx = ((m.read8(eax.wrapping_add(0x1cd))) as u8 as u32);
                eax = (((((m.read16(edx.wrapping_mul(0x2).wrapping_add(0x74c45c)) as i16)) as i32)) as u32);
                ecx = m.read32(0x74c488);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                write16(m, ecx.wrapping_add(eax.wrapping_mul(0x2)), (((((esi) as u16)) as u32)) as u16);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0016f4b0: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f16f4b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x16f4b0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x16f4b0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(0x74c46c);
                esp = esp.wrapping_sub(0x38);
                push!(ebx);
                ebx = m.read32(0x74c458);
                push!(edi);
                eax = (eax & 0xffff);
                edi = eax;
                edi = edi.wrapping_shl((((((((0x9i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = esp.wrapping_add(0xc);
                push!(edx);
                edi = edi.wrapping_add(ebx);
                ebx = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)));
                edx = ((m.read16(ebx.wrapping_add(0x94))) as u16 as u32);
                eax = esp.wrapping_add(0x48);
                push!(eax);
                ecx = esp.wrapping_add(0x10);
                push!(ecx);
                push!(edx);
                call!(0x257040);
                pc = 0x16f4f0; continue;
            }
            0x16f4f0 => {
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16f582; continue; }
                pc = 0x16f4fb; continue;
            }
            0x16f4fb => {
                ecx = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x48));
                esi = esi.wrapping_sub(ecx);
                esi = esi.wrapping_add(1);
                call!(0x153b40);
                pc = 0x16f50c; continue;
            }
            0x16f50c => {
                edx = 0;
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (esi as u64)) as u32;
                edx = (dividend % (esi as u64)) as u32; }
                eax = esp.wrapping_add(0x14);
                push!(eax);
                esi = edx;
                esi = esi.wrapping_add(m.read32(esp.wrapping_add(0x4c)));
                call!(0x45390);
                pc = 0x16f520; continue;
            }
            0x16f520 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x58)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4782c4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47862c))).to_bits();
                m.write32(esp.wrapping_add(0x44), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0x64)));
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0x68)));
                ecx = esp.wrapping_add(0x18);
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0x6c)));
                push!(ecx);
                m.write8(esp.wrapping_add(0x44), (0x1) as u8);
                m.write32(esp.wrapping_add(0x1c), esi);
                m.write32(esp.wrapping_add(0x40), xmm0.lanes[0]);
                call!(0x45820);
                pc = 0x16f57e; continue;
            }
            0x16f57e => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x16f582; continue;
            }
            0x16f582 => {
                edx = m.read32(esp.wrapping_add(0x50));
                eax = m.read32(esp.wrapping_add(0x4c));
                ecx = m.read32(esp.wrapping_add(0x48));
                push!(0);
                push!(0x4);
                push!(0x1);
                push!(edx);
                edx = m.read32(edi.wrapping_add(0x1c0));
                push!(eax);
                eax = 0;
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x1cc))) as u32)) & 0xff);
                push!(ecx);
                ecx = m.read32(edi.wrapping_add(0x1c4));
                edx = (edx & 0xff5dd0bfu32);
                push!(edx);
                push!(eax);
                push!(ecx);
                push!(ebx);
                call!(0x16eab0);
                pc = 0x16f5ba; continue;
            }
            0x16f5ba => {
                esp = esp.wrapping_add(0x28);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00170e20: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
// portreg: va=0x00170e20
pub fn f170e20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x170e20;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    'run: loop {
            match pc {
            0x170e20 => {
                fa = ((((m.read16(0x74c454))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x170ecf; continue; }
                pc = 0x170e2e; continue;
            }
            0x170e2e => {
                push!(esi);
                push!(edi);
                push!(0x1);
                call!(0x3d6e0);
                pc = 0x170e37; continue;
            }
            0x170e37 => {
                edi = eax;
                eax = (eax & 0xffff_0000) | ((((m.read16(0x74c454)) as u32)) & 0xffff);
                esi = 0x1;
                esp = esp.wrapping_add(0x4);
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((((esi) as u16))) as u32) & 0xffffu32);
                write16(m, edi, (0) as u16);
                if (fa <= fb) { pc = 0x170e74; continue; }
                pc = 0x170e51; continue;
            }
            0x170e51 => {
                call!(0x153b40);
                pc = 0x170e56; continue;
            }
            0x170e56 => {
                edx = 0;
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (esi as u64)) as u32;
                edx = (dividend % (esi as u64)) as u32; }
                eax = (eax & 0xffff_0000) | ((((m.read16(edi.wrapping_add(edx.wrapping_mul(0x2)))) as u32)) & 0xffff);
                write16(m, edi.wrapping_add(esi.wrapping_mul(0x2)), (((((eax) as u16)) as u32)) as u16);
                eax = (eax & 0xffff_0000) | ((((m.read16(0x74c454)) as u32)) & 0xffff);
                ecx = ((((eax) as u16)) as u16 as u32);
                write16(m, edi.wrapping_add(edx.wrapping_mul(0x2)), (((((esi) as u16)) as u32)) as u16);
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x170e51; continue; }
                pc = 0x170e74; continue;
            }
            0x170e74 => {
                esi = 0;
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                if (fa <= fb) { pc = 0x170ec4; continue; }
                pc = 0x170e7b; continue;
            }
            0x170e7b => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x10));
                pc = 0x170e80; continue;
            }
            0x170e80 => {
                edx = ((m.read16(edi.wrapping_add(esi.wrapping_mul(0x2)))) as u16 as u32);
                eax = m.read32(0x74c498);
                eax = ((m.read16(eax.wrapping_add(edx.wrapping_mul(0x2)))) as u16 as u32);
                edx = m.read32(0x74c464);
                ecx = eax;
                ecx = ecx.wrapping_shl((((((((0x7i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = m.read32(ecx.wrapping_add(edx).wrapping_add(0x58));
                ecx = ecx.wrapping_sub(0x2);
                fa = ecx;
                if ((fa == 0)) { pc = 0x170ead; continue; }
                pc = 0x170ea1; continue;
            }
            0x170ea1 => {
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x170eb7; continue; }
                pc = 0x170ea4; continue;
            }
            0x170ea4 => {
                push!(ebx);
                push!(eax);
                call!(0x16a410);
                pc = 0x170eab; continue;
            }
            0x170eab => {
                pc = 0x170eb4; continue;
            }
            0x170ead => {
                push!(ebx);
                push!(eax);
                call!(0x16f5d0);
                pc = 0x170eb4; continue;
            }
            0x170eb4 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x170eb7; continue;
            }
            0x170eb7 => {
                eax = ((m.read16(0x74c454)) as u16 as u32);
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x170e80; continue; }
                pc = 0x170ec3; continue;
            }
            0x170ec3 => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x170ec4; continue;
            }
            0x170ec4 => {
                push!(edi);
                call!(0x3d6b0);
                pc = 0x170eca; continue;
            }
            0x170eca => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x170ecf; continue;
            }
            0x170ecf => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00171560: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
// portreg: va=0x00171560
pub fn f171560<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x171560;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x171560 => {
                esp = esp.wrapping_sub(0xfc);
                xmm0 = Xmm::default();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x108)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x104));
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x1719d8; continue; }
                pc = 0x171589; continue;
            }
            0x171589 => {
                push!(ebx);
                push!(esi);
                push!(edi);
                pc = 0x171590; continue;
            }
            0x171590 => {
                ebx = m.read32(esp.wrapping_add(0x114));
                esi = 0x1;
                fa = ebx;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x1715be; continue; }
                pc = 0x1715a0; continue;
            }
            0x1715a0 => {
                call!(0x153b40);
                pc = 0x1715a5; continue;
            }
            0x1715a5 => {
                edx = 0;
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (esi as u64)) as u32;
                edx = (dividend % (esi as u64)) as u32; }
                eax = m.read32(ebp.wrapping_add(esi.wrapping_mul(0x4)));
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = m.read32(ebp.wrapping_add(edx.wrapping_mul(0x4)));
                m.write32(ebp.wrapping_add(esi.wrapping_mul(0x4)).wrapping_add(((0x4i32.wrapping_neg()) as u32)), ecx);
                m.write32(ebp.wrapping_add(edx.wrapping_mul(0x4)), eax);
                if (fas < fbs) { pc = 0x1715a0; continue; }
                pc = 0x1715be; continue;
            }
            0x1715be => {
                edx = m.read32(esp.wrapping_add(0x11c));
                push!(edx);
                call!(0x170e20);
                pc = 0x1715cb; continue;
            }
            0x1715cb => {
                esp = esp.wrapping_add(0x4);
                edi = 0;
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x171641; continue; }
                pc = 0x1715d4; continue;
            }
            0x1715d4 => {
                edx = m.read32(ebp.wrapping_add(edi.wrapping_mul(0x4)));
                eax = (eax & 0xffff_0000) | ((((m.read16(edx.wrapping_add(0x2e))) as u32)) & 0xffff);
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0x5i32)) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x17163c; continue; }
                pc = 0x1715e2; continue;
            }
            0x1715e2 => {
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0x3i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x17160c; continue; }
                pc = 0x1715e8; continue;
            }
            0x1715e8 => {
                eax = m.read32(edx.wrapping_add(0x24));
                ecx = m.read32(eax.wrapping_add(0x80));
                eax = m.read32(0x74c458);
                ecx = (ecx & 0xffff);
                ecx = ecx.wrapping_shl((((((((0x9i32)) as u32) & 0x1fu32))) as u32 & 31));
                fa = m.read32(ecx.wrapping_add(eax).wrapping_add(0x1c0));
                fb = ((((0x2000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x17163c; continue; }
                pc = 0x17160c; continue;
            }
            0x17160c => {
                esi = esp.wrapping_add(0x40);
                call!(0x1667c0);
                pc = 0x171615; continue;
            }
            0x171615 => {
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                eax = m.read32(esp.wrapping_add(0x2c));
                ecx = esi;
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x120));
                push!(eax);
                push!(ecx);
                ebx = edx;
                call!(0x16fc00);
                pc = 0x171632; continue;
            }
            0x171632 => {
                ebx = m.read32(esp.wrapping_add(0x120));
                esp = esp.wrapping_add(0xc);
                pc = 0x17163c; continue;
            }
            0x17163c => {
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1715d4; continue; }
                pc = 0x171641; continue;
            }
            0x171641 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                esi = m.read32(0x7a20c8);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x11c)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                if (fa == fb) { pc = 0x1719c1; continue; }
                pc = 0x171664; continue;
            }
            0x171664 => {
                edx = m.read32(esp.wrapping_add(0x11c));
                push!(edx);
                push!(esi);
                call!(0x16ca00);
                pc = 0x171672; continue;
            }
            0x171672 => {
                eax = 0;
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x1cc))) as u32)) & 0xff);
                push!(eax);
                call!(0x2561b0);
                pc = 0x171680; continue;
            }
            0x171680 => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x134)));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x128)));
                m.write32(esp.wrapping_add(0x48), xmm0.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x48));
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x134)));
                ecx = (ecx & 0xbf800000u32);
                ecx = (ecx | 0x3f800000);
                m.write32(esp.wrapping_add(0x1c), ecx);
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esi.wrapping_add(0x134), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x138)));
                m.write32(esp.wrapping_add(0x40), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x40));
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x138)));
                edx = (edx & 0xbf800000u32);
                edx = (edx | 0x3f800000);
                m.write32(esp.wrapping_add(0x1c), edx);
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esi.wrapping_add(0x138), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x13c)));
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x3c));
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                ecx = (ecx & 0xbf800000u32);
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x13c)));
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x1c0))) as u32)) & 0xff);
                ecx = (ecx | 0x3f800000);
                m.write32(esp.wrapping_add(0x1c), ecx);
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esi.wrapping_add(0x13c), xmm0.lanes[0]);
                if ((fa & fb) != 0) { pc = 0x17183d; continue; }
                pc = 0x1717a2; continue;
            }
            0x1717a2 => {
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x138)));
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0x134)));
                xmm3 = xmm1;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1 = xmm2;
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x47d5d4)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x17183d; continue; }
                pc = 0x1717d5; continue;
            }
            0x1717d5 => {
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x47cff4));
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0.lanes[0] = reciprocal_sqrt(f32::from_bits(m.read32(esp.wrapping_add(0x38)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x134)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esi.wrapping_add(0x134), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x138)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esi.wrapping_add(0x138), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x13c)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esi.wrapping_add(0x13c), xmm1.lanes[0]);
                pc = 0x17183d; continue;
            }
            0x17183d => {
                ebx = m.read32(esi.wrapping_add(0x1e4));
                eax = m.read32(0x74c458);
                edx = ebx;
                edx = (edx & 0xffff);
                edx = edx.wrapping_shl((((((((0x9i32)) as u32) & 0x1fu32))) as u32 & 31));
                edi = edx.wrapping_add(eax);
                ecx = esp.wrapping_add(0x4c);
                push!(edi);
                push!(ecx);
                call!(0x1530e0);
                pc = 0x171861; continue;
            }
            0x171861 => {
                push!(edi);
                edx = edi.wrapping_add(0xc0);
                push!(edx);
                eax = esp.wrapping_add(0x9c);
                push!(eax);
                call!(0x153270);
                pc = 0x171876; continue;
            }
            0x171876 => {
                ecx = esp.wrapping_add(0xa0);
                push!(ecx);
                edx = esp.wrapping_add(0x64);
                push!(edx);
                eax = esp.wrapping_add(0xe8);
                push!(eax);
                call!(0x153270);
                pc = 0x171890; continue;
            }
            0x171890 => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x134)));
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x138)));
                ecx = esp.wrapping_add(0x38);
                push!(ecx);
                edx = esp.wrapping_add(0xf0);
                m.write32(esp.wrapping_add(0x40), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x13c)));
                push!(edx);
                m.write32(esp.wrapping_add(0x48), xmm0.lanes[0]);
                call!(0x151e80);
                pc = 0x1718cc; continue;
            }
            0x1718cc => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                m.write32(edi.wrapping_add(0x128), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
                m.write32(edi.wrapping_add(0x12c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                push!(ebx);
                m.write32(edi.wrapping_add(0x130), xmm0.lanes[0]);
                call!(0x167880);
                pc = 0x1718fc; continue;
            }
            0x1718fc => {
                eax = m.read32(esi.wrapping_add(0x1b8));
                esp = esp.wrapping_add(0x2c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x171940; continue; }
                pc = 0x171909; continue;
            }
            0x171909 => {
                ecx = m.read32(eax);
                m.write32(esi.wrapping_add(0x110), ecx);
                edx = m.read32(eax.wrapping_add(0x4));
                m.write32(esi.wrapping_add(0x114), edx);
                ecx = m.read32(eax.wrapping_add(0x8));
                m.write32(esi.wrapping_add(0x118), ecx);
                edx = m.read32(eax.wrapping_add(0xc));
                m.write32(esi.wrapping_add(0x11c), edx);
                ecx = m.read32(eax.wrapping_add(0x10));
                m.write32(esi.wrapping_add(0x120), ecx);
                edx = m.read32(eax.wrapping_add(0x14));
                m.write32(esi.wrapping_add(0x124), edx);
                pc = 0x171973; continue;
            }
            0x171940 => {
                xmm0 = Xmm::default();
                m.write32(esi.wrapping_add(0x110), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x114), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x118), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x11c), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x120), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x124), xmm0.lanes[0]);
                pc = 0x171973; continue;
            }
            0x171973 => {
                fa = m.read32(esi.wrapping_add(0x1c0));
                fb = ((((0x40000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x1719b3; continue; }
                pc = 0x17197f; continue;
            }
            0x17197f => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x118)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x14))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x1719b3; continue; }
                pc = 0x17198f; continue;
            }
            0x17198f => {
                xmm0 = Xmm::scalar(m.read32(0x47ddbc));
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x114)));
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x1a8)))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esi.wrapping_add(0x114), xmm1.lanes[0]);
                pc = 0x1719b3; continue;
            }
            0x1719b3 => {
                esi = m.read32(esi.wrapping_add(0x1fc));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x171664; continue; }
                pc = 0x1719c1; continue;
            }
            0x1719c1 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x118)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x14))) as f64);
                if ((fca > fcb)) { pc = 0x171590; continue; }
                pc = 0x1719d5; continue;
            }
            0x1719d5 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x1719d8; continue;
            }
            0x1719d8 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xfc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x001719e0: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1719e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1719e0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x1719e0 => {
                esp = esp.wrapping_sub(0x34);
                push!(esi);
                esi = m.read32(0x7a20c8);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x171fa8; continue; }
                pc = 0x1719f2; continue;
            }
            0x1719f2 => {
                push!(ebp);
                push!(edi);
                pc = 0x171a00; continue;
            }
            0x171a00 => {
                xmm0 = Xmm::default();
                eax = m.read32(esi.wrapping_add(0x1fc));
                ecx = m.read32(esi.wrapping_add(0x1e4));
                edx = m.read32(0x74c46c);
                ecx = (ecx & 0xffff);
                m.write32(esp.wrapping_add(0xc), eax);
                m.write32(esi.wrapping_add(0x110), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x114), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x118), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x11c), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x120), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x124), xmm0.lanes[0]);
                fa = m.read32(edx.wrapping_add(ecx.wrapping_mul(0x4)));
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x171f9a; continue; }
                pc = 0x171a59; continue;
            }
            0x171a59 => {
                push!(esi);
                call!(0x26a500);
                pc = 0x171a5f; continue;
            }
            0x171a5f => {
                eax = m.read32(esi.wrapping_add(0x1e4));
                esp = esp.wrapping_add(0x4);
                call!(0x168180);
                pc = 0x171a6d; continue;
            }
            0x171a6d => {
                eax = m.read32(esi.wrapping_add(0x1e4));
                ecx = m.read32(0x74c46c);
                eax = (eax & 0xffff);
                edi = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)));
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x171f9a; continue; }
                pc = 0x171a89; continue;
            }
            0x171a89 => {
                edx = m.read32(edi.wrapping_add(0x18));
                ecx = m.read32(edi.wrapping_add(0xc));
                edx = (edx | 0x40);
                m.write32(edi.wrapping_add(0x18), edx);
                edx = m.read32(edi.wrapping_add(0x10));
                edx = (edx | ecx);
                fa = edx;
                fb = ((((0xffffi32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x171aad; continue; }
                pc = 0x171aa2; continue;
            }
            0x171aa2 => {
                eax = m.read32(0x7de318);
                m.write32(esi.wrapping_add(0x1bc), eax);
                pc = 0x171aad; continue;
            }
            0x171aad => {
                push!(0x1);
                push!(edi);
                call!(0x2572a0);
                pc = 0x171ab5; continue;
            }
            0x171ab5 => {
                eax = m.read32(esi.wrapping_add(0x1c0));
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0x10000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x171d7f; continue; }
                pc = 0x171ac9; continue;
            }
            0x171ac9 => {
                ecx = m.read32(esi.wrapping_add(0x14c));
                m.write32(edi.wrapping_add(0xb4), ecx);
                edx = m.read32(esi.wrapping_add(0x150));
                m.write32(edi.wrapping_add(0xb8), edx);
                eax = m.read32(esi.wrapping_add(0x154));
                m.write32(edi.wrapping_add(0xbc), eax);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x140)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x158)))).to_bits();
                eax = m.read32(edi.wrapping_add(0x34));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebp = esi.wrapping_add(0x158);
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x144)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x15c)))).to_bits();
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x148)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x160)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                if (fa == fb) { pc = 0x171b43; continue; }
                pc = 0x171b3c; continue;
            }
            0x171b3c => {
                fa = eax;
                fb = ((((0xc8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x171b53; continue; }
                pc = 0x171b43; continue;
            }
            0x171b43 => {
                push!(edi);
                m.write32(edi.wrapping_add(0x34), 0);
                call!(0x9fbe0);
                pc = 0x171b50; continue;
            }
            0x171b50 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x171b53; continue;
            }
            0x171b53 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x30))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x171b68; continue; }
                pc = 0x171b5a; continue;
            }
            0x171b5a => {
                eax = 0xd5;
                m.write32(edi.wrapping_add(0x38), eax);
                m.write8(edi.wrapping_add(0x30), (0x1) as u8);
                pc = 0x171b90; continue;
            }
            0x171b68 => {
                fa = m.read32(edi.wrapping_add(0x28));
                fb = ((((0x40000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x171b93; continue; }
                pc = 0x171b71; continue;
            }
            0x171b71 => {
                eax = m.read32(edi.wrapping_add(0x34));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x171b93; continue; }
                pc = 0x171b78; continue;
            }
            0x171b78 => {
                fa = eax;
                fb = ((((0xc8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x171b93; continue; }
                pc = 0x171b7f; continue;
            }
            0x171b7f => {
                push!(0);
                ecx = esp.wrapping_add(0x20);
                push!(ecx);
                push!(eax);
                push!(ebp);
                call!(0x888a0);
                pc = 0x171b8d; continue;
            }
            0x171b8d => {
                esp = esp.wrapping_add(0x10);
                pc = 0x171b90; continue;
            }
            0x171b90 => {
                m.write32(edi.wrapping_add(0x34), eax);
                pc = 0x171b93; continue;
            }
            0x171b93 => {
                eax = m.read32(esi.wrapping_add(0x144));
                edx = m.read32(esi.wrapping_add(0x140));
                ecx = m.read32(esi.wrapping_add(0x148));
                xmm0 = Xmm::default();
                m.write32(ebp, edx);
                m.write32(esi.wrapping_add(0x15c), eax);
                m.write32(esi.wrapping_add(0x160), ecx);
                fa = ((((m.read8(edi.wrapping_add(0x8)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                m.write32(edi.wrapping_add(0xc0), xmm0.lanes[0]);
                m.write32(edi.wrapping_add(0x70), xmm0.lanes[0]);
                m.write32(edi.wrapping_add(0xc8), xmm0.lanes[0]);
                if ((fa & fb) == 0) { pc = 0x171bec; continue; }
                pc = 0x171bd2; continue;
            }
            0x171bd2 => {
                edx = m.read32(esi.wrapping_add(0x1e4));
                push!(edx);
                call!(0x170f90);
                pc = 0x171bde; continue;
            }
            0x171bde => {
                eax = m.read32(esi.wrapping_add(0x1e4));
                esp = esp.wrapping_add(0x4);
                call!(0x165880);
                pc = 0x171bec; continue;
            }
            0x171bec => {
                eax = m.read32(esi.wrapping_add(0x1e4));
                push!(eax);
                call!(0x165a40);
                pc = 0x171bf8; continue;
            }
            0x171bf8 => {
                eax = m.read32(esi.wrapping_add(0x1c0));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0x200000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x171c17; continue; }
                pc = 0x171c08; continue;
            }
            0x171c08 => {
                ecx = m.read32(esi.wrapping_add(0x1e4));
                push!(ecx);
                call!(0x165b90);
                pc = 0x171c14; continue;
            }
            0x171c14 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x171c17; continue;
            }
            0x171c17 => {
                fa = m.read32(edi.wrapping_add(0x28));
                fb = ((((0x40000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x171c3c; continue; }
                pc = 0x171c20; continue;
            }
            0x171c20 => {
                eax = m.read32(edi.wrapping_add(0x34));
                fa = eax;
                fb = ((((0xd5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x171c3c; continue; }
                pc = 0x171c2a; continue;
            }
            0x171c2a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x171c35; continue; }
                pc = 0x171c2e; continue;
            }
            0x171c2e => {
                fa = eax;
                fb = ((((0xc8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x171c3c; continue; }
                pc = 0x171c35; continue;
            }
            0x171c35 => {
                ebp = 0x1;
                pc = 0x171c8f; continue;
            }
            0x171c3c => {
                eax = m.read32(esi.wrapping_add(0x1e4));
                edx = m.read32(0x74c458);
                eax = (eax & 0xffff);
                eax = eax.wrapping_shl((((((((0x9i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(edx);
                eax = m.read32(eax.wrapping_add(0x1d0));
                ebp = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x171c8f; continue; }
                pc = 0x171c5e; continue;
            }
            0x171c5e => {
                edx = m.read32(0x7de318);
                xmm1.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                xmm2 = Xmm::scalar(m.read32(0x475c78));
                edx = edx.wrapping_sub(eax);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::divss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x47862c)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x171d7f; continue; }
                pc = 0x171c8f; continue;
            }
            0x171c8f => {
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x1a8)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x64)))).to_bits();
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(0x1ac)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x68)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(edi.wrapping_add(0x1b0)));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x6c)))).to_bits();
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x1c0)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x1c0)))).to_bits();
                m.write32(esp.wrapping_add(0x34), xmm3.lanes[0]);
                xmm3 = xmm1;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x1c4)))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x1c4)))).to_bits();
                m.write32(esp.wrapping_add(0x38), xmm3.lanes[0]);
                xmm3 = xmm2;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x1c8)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x1c8)))).to_bits();
                eax = esp.wrapping_add(0x28);
                push!(eax);
                m.write32(esp.wrapping_add(0x40), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x30), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x34), xmm2.lanes[0]);
                call!(0x872f0);
                pc = 0x171d1d; continue;
            }
            0x171d1d => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x171d59; continue; }
                pc = 0x171d24; continue;
            }
            0x171d24 => {
                eax = (eax & 0xffff_0000) | ((((m.read16(edi.wrapping_add(0x94))) as u32)) & 0xffff);
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0x385i32)) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x171d59; continue; }
                pc = 0x171d31; continue;
            }
            0x171d31 => {
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0x898i32)) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x171d59; continue; }
                pc = 0x171d37; continue;
            }
            0x171d37 => {
                fa = m.read32(edi.wrapping_add(0x20));
                fb = ((((0x20000000i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x171d4e; continue; }
                pc = 0x171d40; continue;
            }
            0x171d40 => {
                edi = edi.wrapping_add(0x7c);
                push!(edi);
                call!(0x351f60);
                pc = 0x171d49; continue;
            }
            0x171d49 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x171d7f; continue;
            }
            0x171d4e => {
                push!(edi);
                call!(0xa9070);
                pc = 0x171d54; continue;
            }
            0x171d54 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x171d7f; continue;
            }
            0x171d59 => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x171d7f; continue; }
                pc = 0x171d5d; continue;
            }
            0x171d5d => {
                push!(0xa);
                ebp = edi.wrapping_add(0x38);
                push!(ebp);
                ecx = esp.wrapping_add(0x30);
                push!(ecx);
                push!(0);
                call!(0x873c0);
                pc = 0x171d6f; continue;
            }
            0x171d6f => {
                esp = esp.wrapping_add(0x10);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                m.write8(edi.wrapping_add(0x30), (((((eax) as u8)) as u32)) as u8);
                if (fa == fb) { pc = 0x171d7f; continue; }
                pc = 0x171d79; continue;
            }
            0x171d79 => {
                edx = m.read32(ebp);
                m.write32(edi.wrapping_add(0x34), edx);
                pc = 0x171d7f; continue;
            }
            0x171d7f => {
                eax = m.read32(esi.wrapping_add(0x1c0));
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x171f9a; continue; }
                pc = 0x171d8d; continue;
            }
            0x171d8d => {
                edx = m.read32(esi.wrapping_add(0x1bc));
                ecx = m.read32(0x7de318);
                ecx = ecx.wrapping_sub(edx);
                edx = m.read32(0x4e8cec);
                edx = edx.wrapping_add(edx.wrapping_mul(0x4));
                edx = edx.wrapping_shl((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31));
                fa = ecx;
                fb = edx;
                if (fa <= fb) { pc = 0x171db5; continue; }
                pc = 0x171daa; continue;
            }
            0x171daa => {
                fa = eax;
                fb = ((((0x10000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x171f83; continue; }
                pc = 0x171db5; continue;
            }
            0x171db5 => {
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x171f69; continue; }
                pc = 0x171dbe; continue;
            }
            0x171dbe => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x184)));
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x180)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x13c)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x138)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0x184)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x134)))).to_bits();
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x17c)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x138)))).to_bits();
                xmm4 = Xmm::scalar(m.read32(esi.wrapping_add(0x150)));
                xh[0] = Xmm::scalar(m.read32(esi.wrapping_add(0x14c)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x17c)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x13c)))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0x180)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x134)))).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x154)));
                xh[1] = xmm3;
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm4;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xh[0];
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xh[1].lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475d1c)) as f64);
                if ((fca > fcb)) { pc = 0x171f69; continue; }
                pc = 0x171e6c; continue;
            }
            0x171e6c => {
                xmm3 = xmm2;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm0;
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x4785d4)) as f64);
                if ((fca > fcb)) { pc = 0x171f69; continue; }
                pc = 0x171e96; continue;
            }
            0x171e96 => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x17c)));
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x180)));
                eax = esp.wrapping_add(0x10);
                push!(eax);
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x184)));
                push!(esi);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                call!(0x152090);
                pc = 0x171ecb; continue;
            }
            0x171ecb => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x140)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x144)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0x148)));
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x20)))).to_bits();
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x1ac)))).to_bits();
                xmm4 = xmm1;
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x1b0)))).to_bits();
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x1c), xmm1.lanes[0]);
                m.write32(esi.wrapping_add(0x1ac), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x1b0), xmm1.lanes[0]);
                xh[0] = xmm2;
                xh[0].lanes[0] = sse::subss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x1b4)))).to_bits();
                xmm0 = xh[0];
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1 = xmm4;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm3;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                esp = esp.wrapping_add(0x8);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x476240)) as f64);
                m.write32(esp.wrapping_add(0x18), xmm2.lanes[0]);
                m.write32(esi.wrapping_add(0x1b4), xmm2.lanes[0]);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x171f83; continue; }
                pc = 0x171f69; continue;
            }
            0x171f69 => {
                eax = 0x2aaaaaab;
                { let product = (eax as i32 as i64) * (m.read32(0x4e8cec) as i32 as i64); eax = product as u32; edx = ((product as u64) >> 32) as u32; }
                ecx = edx;
                ecx = ecx.wrapping_shr((((((((0x1fi32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_add(edx);
                m.write8(esi.wrapping_add(0x1c8), (((((ecx) as u8)) as u32)) as u8);
                pc = 0x171f9a; continue;
            }
            0x171f83 => {
                m.write8(esi.wrapping_add(0x1c8), (((((m.read8(esi.wrapping_add(0x1c8))) as i32).wrapping_sub(0x1i32)) as u32)) as u8);
                fa = ((((m.read8(esi.wrapping_add(0x1c8)))) as u32) & 0xffu32);
                if ((fa != 0)) { pc = 0x171f9a; continue; }
                pc = 0x171f8b; continue;
            }
            0x171f8b => {
                edx = m.read32(esi.wrapping_add(0x1e4));
                push!(edx);
                call!(0x1677e0);
                pc = 0x171f97; continue;
            }
            0x171f97 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x171f9a; continue;
            }
            0x171f9a => {
                esi = m.read32(esp.wrapping_add(0xc));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x171a00; continue; }
                pc = 0x171fa6; continue;
            }
            0x171fa6 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x171fa8; continue;
            }
            0x171fa8 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x34);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00171fb0: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
// portreg: va=0x00171fb0
pub fn f171fb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x171fb0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        fpu = cr.fpu;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x171fb0 => {
                push!(ebp);
                ebp = esp;
                esp = (esp & 0xfffffff8u32);
                esp = esp.wrapping_sub(0x10);
                eax = m.read32(0x4e7944);
                push!(ebx);
                push!(ebp);
                ebp = 0;
                fa = eax;
                fb = ebp;
                push!(esi);
                push!(edi);
                if (fa == fb) { pc = 0x172159; continue; }
                pc = 0x171fcc; continue;
            }
            0x171fcc => {
                push!(ebp);
                push!(0x100);
                call!(0x3dfa0);
                pc = 0x171fd7; continue;
            }
            0x171fd7 => {
                edi = eax;
                eax = 0;
                push!(ebp);
                m.write32(0x74c474, edi);
                ecx = 0x40;
                push!(0x40);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                call!(0x3dfa0);
                pc = 0x171ff0; continue;
            }
            0x171ff0 => {
                edi = eax;
                eax = 0;
                m.write32(0x74c484, edi);
                ecx = 0x10;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                push!(0x3);
                m.write32(0x74c47c, 0x10);
                m.write32(0x74c490, ebp);
                m.write32(0x7a20d0, 0x1);
                call!(0x3d6e0);
                pc = 0x172022; continue;
            }
            0x172022 => {
                xmm0 = Xmm::scalar(m.read32(0x4e794c));
                esi = eax;
                eax = m.read32(0x74c450);
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x74c414), xmm0.lanes[0]);
                esp = esp.wrapping_add(0x14);
                eax = eax.wrapping_add(1);
                edx = 0;
                ecx = 0xf;
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (ecx as u64)) as u32;
                edx = (dividend % (ecx as u64)) as u32; }
                xmm1 = Xmm::default();
                eax = 0x74c450;
                m.write32(0x74c450, edx);
                pc = 0x172060; continue;
                edi = edi;
                pc = 0x172060; continue;
            }
            0x172060 => {
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(((0x4i32.wrapping_neg()) as u32))));
                eax = eax.wrapping_sub(0x4);
                fa = eax;
                fb = ((((0x74c414i32)) as u32) & 0xffffffffu32);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm0;
                if (fa != fb) { pc = 0x172060; continue; }
                pc = 0x172076; continue;
            }
            0x172076 => {
                xmm2.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x47621c))).to_bits();
                eax = m.read32(0x4e7944);
                fa = eax;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d50))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                if (fas <= fbs) { pc = 0x172126; continue; }
                pc = 0x1720bb; continue;
            }
            0x1720bb => {
                ebx = m.read32(esp.wrapping_add(0x10));
                pc = 0x1720c0; continue;
            }
            0x1720c0 => {
                push!(ebx);
                edi = esp.wrapping_add(0x1c);
                call!(0x170500);
                pc = 0x1720ca; continue;
            }
            0x1720ca => {
                edx = edi;
                push!(edx);
                call!(0x16ccd0);
                pc = 0x1720d2; continue;
            }
            0x1720d2 => {
                push!(0xdb);
                push!(esi);
                eax = edi;
                call!(0x170690);
                pc = 0x1720df; continue;
            }
            0x1720df => {
                esp = esp.wrapping_add(0x10);
                edi = eax;
                call!(0x16b6b0);
                pc = 0x1720e9; continue;
            }
            0x1720e9 => {
                call!(0x16b050);
                pc = 0x1720ee; continue;
            }
            0x1720ee => {
                eax = m.read32(esp.wrapping_add(0x18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x17210c; continue; }
                pc = 0x1720f6; continue;
            }
            0x1720f6 => {
                fa = ((((m.read16(0x74c454))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x17210c; continue; }
                pc = 0x172100; continue;
            }
            0x172100 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                pc = 0x17210c; continue;
            }
            0x17210c => {
                eax = m.read32(esp.wrapping_add(0x14));
                push!(eax);
                push!(ebx);
                push!(edi);
                push!(esi);
                call!(0x171560);
                pc = 0x172119; continue;
            }
            0x172119 => {
                eax = m.read32(0x4e7944);
                esp = esp.wrapping_add(0x10);
                ebp = ebp.wrapping_add(1);
                fa = ebp;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1720c0; continue; }
                pc = 0x172126; continue;
            }
            0x172126 => {
                push!(esi);
                call!(0x3d6b0);
                pc = 0x17212c; continue;
            }
            0x17212c => {
                ecx = m.read32(0x74c478);
                esp = esp.wrapping_add(0x4);
                m.write32(0x4e826c, 0x1f4);
                m.write32(0x74c468, ecx);
                m.write32(0x7a20d0, 0);
                call!(0x16c0d0);
                pc = 0x172154; continue;
            }
            0x172154 => {
                call!(0x1719e0);
                pc = 0x172159; continue;
            }
            0x172159 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

// portreg: va=0x002572a0
/// 0x002572a0: Tier 0 zero-fixture smoke; opt-in, differential verification debt.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2572a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut fp_cmp = 0i32;
    let mut pc: u32 = 0x2572a0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    'run: loop {
            match pc {
            0x2572a0 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                eax = ((m.read16(esi.wrapping_add(0x94))) as u16 as u32);
                push!(edi);
                call!(0x2570e0);
                pc = 0x2572b2; continue;
            }
            0x2572b2 => {
                edi = eax;
                fa = edi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2573d5; continue; }
                pc = 0x2572bd; continue;
            }
            0x2572bd => {
                eax = m.read32(esp.wrapping_add(0x10));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2573a5; continue; }
                pc = 0x2572c9; continue;
            }
            0x2572c9 => {
                eax = m.read32(esi.wrapping_add(0x80));
                push!(eax);
                call!(0x167a10);
                pc = 0x2572d5; continue;
            }
            0x2572d5 => {
                fpu.push((f32::from_bits(m.read32(eax.wrapping_add(0x8))) as f64));
                fpu.push((f32::from_bits(m.read32(eax.wrapping_add(0x4))) as f64));
                ecx = m.read32(esi.wrapping_add(0x80));
                fpu.push((f32::from_bits(m.read32(eax)) as f64));
                push!(ecx);
                { let value = fpu.st(0); fpu.push(value); }
                { let value = round_pc((fpu.st(0) * fpu.st(1)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = fpu.st(2); fpu.push(value); }
                { let value = round_pc((fpu.st(0) * fpu.st(3)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc((fpu.st(1) + fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 1) & 7) as usize] = value; } fpu.pop();
                { let value = fpu.st(3); fpu.push(value); }
                { let value = round_pc((fpu.st(0) * fpu.st(4)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc((fpu.st(1) + fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 1) & 7) as usize] = value; } fpu.pop();
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.stack[((fpu.top + 3) & 7) as usize] = fpu.st(0); fpu.pop();
                fpu.pop();
                fpu.pop();
                { let value = round_pc((fpu.st(0) * (f32::from_bits(m.read32(0x4765e0)) as f64)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x14), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x165690);
                pc = 0x25730b; continue;
            }
            0x25730b => {
                fpu.push((f32::from_bits(m.read32(0x475d10)) as f64));
                xmm1 = Xmm::scalar(m.read32(0x475d10));
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x14))) as f64));
                esp = esp.wrapping_add(0x8);
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if ((fp_cmp == 0x1i32)) { pc = 0x25732c; continue; }
                pc = 0x257326; continue;
            }
            0x257326 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                pc = 0x25732c; continue;
            }
            0x25732c => {
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x1e0)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd0))).to_bits();
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x4785e4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(0x475d20));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                m.write32(eax.wrapping_add(0x1e0), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0xc), xmm2.lanes[0]);
                if ((fca > fcb)) { pc = 0x25736c; continue; }
                pc = 0x257366; continue;
            }
            0x257366 => {
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                pc = 0x25736c; continue;
            }
            0x25736c => {
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4765e0))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x476044))).to_bits();
                edx = m.read32(esp.wrapping_add(0xc));
                push!(0x1);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                push!(0x41c80000);
                push!(edx);
                eax = esi.wrapping_add(0x64);
                push!(edi);
                push!(eax);
                esi = esi.wrapping_add(0x22c);
                push!(esi);
                call!(0x468a0);
                pc = 0x25739f; continue;
            }
            0x25739f => {
                esp = esp.wrapping_add(0x1c);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2573a5 => {
                ecx = m.read32(esi.wrapping_add(0x80));
                push!(ecx);
                call!(0x165690);
                pc = 0x2573b1; continue;
            }
            0x2573b1 => {
                xmm0 = Xmm::default();
                m.write32(eax.wrapping_add(0x1e0), xmm0.lanes[0]);
                edx = m.read32(esi.wrapping_add(0x22c));
                push!(edx);
                call!(0x45460);
                pc = 0x2573c8; continue;
            }
            0x2573c8 => {
                esp = esp.wrapping_add(0x8);
                m.write32(esi.wrapping_add(0x22c), 0xffffffffu32);
                pc = 0x2573d5; continue;
            }
            0x2573d5 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// Disabled/unknown roots leave memory and registers untouched.
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, enabled: bool, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
if !enabled { return false; }
match va {

0x471a0 => f471a0(m, guest, regs),

0x47440 => f47440(m, guest, regs),

0x494e0 => f494e0(m, guest, regs),

0x4bbd0 => f4bbd0(m, guest, regs),

0x50170 => f50170(m, guest, regs),

0x50630 => f50630(m, guest, regs),

0x529d0 => f529d0(m, guest, regs),

0x554c0 => f554c0(m, guest, regs),

0x92a10 => f92a10(m, guest, regs),

0x93690 => f93690(m, guest, regs),

0xa3ae0 => fa3ae0(m, guest, regs),

0xdc290 => fdc290(m, guest, regs),

0xdcfb0 => fdcfb0(m, guest, regs),

0xdd130 => fdd130(m, guest, regs),

0xdd170 => fdd170(m, guest, regs),

0xdfd00 => fdfd00(m, guest, regs),

0xdfe50 => fdfe50(m, guest, regs),

0x12a170 => f12a170(m, guest, regs),

0x12a590 => f12a590(m, guest, regs),

0x166060 => f166060(m, guest, regs),

0x166140 => f166140(m, guest, regs),

0x16a2f0 => f16a2f0(m, guest, regs),

0x16b380 => f16b380(m, guest, regs),

0x16c300 => f16c300(m, guest, regs),

0x16f4b0 => f16f4b0(m, guest, regs),

0x170e20 => f170e20(m, guest, regs),

0x171560 => f171560(m, guest, regs),

0x1719e0 => f1719e0(m, guest, regs),

0x171fb0 => f171fb0(m, guest, regs),

0x2572a0 => f2572a0(m, guest, regs),

_ => return false,
}
true
}

pub const ROOTS: &[u32] = &[0x471a0,0x47440,0x494e0,0x4bbd0,0x50170,0x50630,0x529d0,0x554c0,0x92a10,0x93690,0xa3ae0,0xdc290,0xdcfb0,0xdd130,0xdd170,0xdfd00,0xdfe50,0x12a170,0x12a590,0x166060,0x166140,0x16a2f0,0x16b380,0x16c300,0x16f4b0,0x170e20,0x171560,0x1719e0,0x171fb0,0x2572a0];
