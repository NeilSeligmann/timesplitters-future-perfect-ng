#![cfg_attr(rustfmt, rustfmt::skip)] // SPDX-License-Identifier: GPL-3.0-or-later
//! T1924 opt-in audio candidates; names INFERRED. See docs/t-system-audio.md.
#![allow(clippy::all, unused_parens, unused_assignments, unused_imports, unused_variables)]
use crate::{abi::{ExtRegisters, GuestCall, Xmm}, memory::GuestMemory, pose::write16};
use crate::inventory::sse;
use crate::system_slice::{round_pc, read_double, sqrt_double, reciprocal_sqrt, compare_fp};
#[path = "audio_remainder_slice/boundary.rs"] mod boundary;
pub use boundary::*;
/// 0x00021780: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21780<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21780;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21780, previous, pc); previous = pc; }
        match pc {
            0x21780 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x2178c);
                call!(0x45390);
                pc = 0x2178c; continue;
            }
            0x2178c => {
                ecx = m.read32(esp.wrapping_add(0x38));
                edx = esp.wrapping_add(0x4);
                push!(edx);
                m.write32(esp.wrapping_add(0x8), ecx);
                crate::audio_lane::set_return(0x2179e);
                call!(0x45820);
                pc = 0x2179e; continue;
            }
            0x2179e => {
                esp = esp.wrapping_add(0x38);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00021910: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21910<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21910;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21910, previous, pc); previous = pc; }
        match pc {
            0x21910 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x2191c);
                call!(0x45390);
                pc = 0x2191c; continue;
            }
            0x2191c => {
                eax = m.read32(esp.wrapping_add(0x40));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                ecx = m.read32(esp.wrapping_add(0x38));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax));
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                edx = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                push!(edx);
                m.write8(esp.wrapping_add(0x30), (0x1) as u8);
                m.write32(esp.wrapping_add(0x8), ecx);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x21963);
                call!(0x45820);
                pc = 0x21963; continue;
            }
            0x21963 => {
                esp = esp.wrapping_add(0x38);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000219E0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f219e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x219e0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x219e0, previous, pc); previous = pc; }
        match pc {
            0x219e0 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x219ec);
                call!(0x45390);
                pc = 0x219ec; continue;
            }
            0x219ec => {
                eax = m.read32(esp.wrapping_add(0x40));
                xmm0 = Xmm::scalar(m.read32(eax));
                ecx = m.read32(esp.wrapping_add(0x38));
                edx = m.read32(esp.wrapping_add(0x3c));
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                eax = esp.wrapping_add(0x4);
                push!(eax);
                m.write8(esp.wrapping_add(0x30), (0x1) as u8);
                m.write32(esp.wrapping_add(0x8), ecx);
                m.write32(esp.wrapping_add(0xc), edx);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x21a2f);
                call!(0x45820);
                pc = 0x21a2f; continue;
            }
            0x21a2f => {
                esp = esp.wrapping_add(0x38);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021A40: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21a40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21a40;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21a40, previous, pc); previous = pc; }
        match pc {
            0x21a40 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x21a4c);
                call!(0x45390);
                pc = 0x21a4c; continue;
            }
            0x21a4c => {
                eax = m.read32(esp.wrapping_add(0x44));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                ecx = m.read32(esp.wrapping_add(0x38));
                edx = m.read32(esp.wrapping_add(0x3c));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax));
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                eax = esp.wrapping_add(0x4);
                push!(eax);
                m.write8(esp.wrapping_add(0x30), (0x1) as u8);
                m.write32(esp.wrapping_add(0x8), ecx);
                m.write32(esp.wrapping_add(0xc), edx);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x21a9b);
                call!(0x45820);
                pc = 0x21a9b; continue;
            }
            0x21a9b => {
                esp = esp.wrapping_add(0x38);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021BB0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21bb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21bb0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21bb0, previous, pc); previous = pc; }
        match pc {
            0x21bb0 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x21bbc);
                call!(0x45390);
                pc = 0x21bbc; continue;
            }
            0x21bbc => {
                eax = m.read32(esp.wrapping_add(0x40));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                ecx = m.read32(esp.wrapping_add(0x38));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax));
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                edx = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                push!(edx);
                m.write8(esp.wrapping_add(0x30), (0x1) as u8);
                m.write32(esp.wrapping_add(0x8), ecx);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x21c03);
                call!(0x45820);
                pc = 0x21c03; continue;
            }
            0x21c03 => {
                esp = esp.wrapping_add(0x38);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021C80: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21c80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21c80;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21c80, previous, pc); previous = pc; }
        match pc {
            0x21c80 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x21c8c);
                call!(0x45390);
                pc = 0x21c8c; continue;
            }
            0x21c8c => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                eax = m.read32(esp.wrapping_add(0x44));
                ecx = m.read32(esp.wrapping_add(0x14));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax));
                ecx = (ecx | 0xa0);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x14), ecx);
                ecx = m.read32(esp.wrapping_add(0x38));
                edx = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                push!(edx);
                m.write8(esp.wrapping_add(0x30), (0x1) as u8);
                m.write32(esp.wrapping_add(0x8), ecx);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x21ced);
                call!(0x45820);
                pc = 0x21ced; continue;
            }
            0x21ced => {
                esp = esp.wrapping_add(0x38);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00026F00: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f26f00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x26f00;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x26f00, previous, pc); previous = pc; }
        match pc {
            0x26f00 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x26f5d; continue; }
                pc = 0x26f04; continue;
            }
            0x26f04 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x26f24; continue; }
                pc = 0x26f17; continue;
            }
            0x26f17 => {
                xmm2 = Xmm::default();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x26f5d; continue; }
                pc = 0x26f1f; continue;
            }
            0x26f1f => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x26f3f; continue; }
                pc = 0x26f24; continue;
            }
            0x26f24 => {
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x4))) as f64));
                fpu.push(0.30102999566398119521f64);
                { let a = fpu.top as usize; let b = ((fpu.top + 1) & 7) as usize; fpu.stack.swap(a, b); }
                { let result = fpu.st(1) * crate::c2r::rt::log2(fpu.st(0)); fpu.stack[((fpu.top + 1) & 7) as usize] = result; fpu.pop(); }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4761d4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                eax = (fpu.st(0) as i64) as u32; fpu.pop(); break 'run;
            }
            0x26f3f => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475d60)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x26f5d; continue; }
                pc = 0x26f48; continue;
            }
            0x26f48 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x4))) as f64));
                fpu.push(0.30102999566398119521f64);
                { let a = fpu.top as usize; let b = ((fpu.top + 1) & 7) as usize; fpu.stack.swap(a, b); }
                { let result = fpu.st(1) * crate::c2r::rt::log2(fpu.st(0)); fpu.stack[((fpu.top + 1) & 7) as usize] = result; fpu.pop(); }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4761d4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                eax = (fpu.st(0) as i64) as u32; fpu.pop(); break 'run;
            }
            0x26f5d => {
                eax = 0xffffd8f0u32;
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000273E0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f273e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x273e0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x273e0, previous, pc); previous = pc; }
        match pc {
            0x273e0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                push!(esi);
                crate::audio_lane::set_return(0x273ea);
                call!(0x26bb0);
                pc = 0x273ea; continue;
            }
            0x273ea => {
                esi = eax;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2746a; continue; }
                pc = 0x273f0; continue;
            }
            0x273f0 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x15))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x27460; continue; }
                pc = 0x273f7; continue;
            }
            0x273f7 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x3i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x27460; continue; }
                pc = 0x273fb; continue;
            }
            0x273fb => {
                ecx = m.read32(esi);
                crate::audio_lane::set_return(0x27402);
                call!(0x26b70);
                pc = 0x27402; continue;
            }
            0x27402 => {
                eax = m.read32(esi.wrapping_add(0x34));
                ecx = m.read32(esi.wrapping_add(0x30));
                m.write32(eax.wrapping_add(0x30), ecx);
                edx = m.read32(esi.wrapping_add(0x30));
                eax = m.read32(esi.wrapping_add(0x34));
                m.write32(edx.wrapping_add(0x34), eax);
                m.write32(esi.wrapping_add(0x34), 0);
                m.write32(esi.wrapping_add(0x30), 0);
                esi = esi.wrapping_sub(0x5819b8);
                eax = 0x92492493u32;
                { let wide = (((eax) as i32 as i64).wrapping_mul((esi) as i32 as i64)) as u64; eax = wide as u32; edx = (wide >> 32) as u32; }
                edx = edx.wrapping_add(esi);
                edx = (((((edx as i32)).wrapping_shr((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                eax = edx;
                eax = eax.wrapping_shr((((((((0x1fi32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(edx);
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x38) as i32)) as u32;
                m.write32(ecx.wrapping_add(0x5819b8), 0);
                ecx = m.read32(0x565b40);
                m.write32(ecx.wrapping_mul(0x4).wrapping_add(0x5657c0), eax);
                ecx = ecx.wrapping_add(1);
                m.write32(0x565b40, ecx);
                pc = 0x2745e; continue;
            }
            0x2745e => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x27460 => {
                edx = m.read32(esi);
                push!(edx);
                crate::audio_lane::set_return(0x27468);
                call_to!(0x00407AA4);
                pc = 0x27468; continue;
            }
            0x27468 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x2746a => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2745e; continue; }
                pc = 0x2746e; continue;
            }
            0x2746e => {
                eax = 0;
                pc = 0x27470; continue;
            }
            0x27470 => {
                fa = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x581990));
                fb = ecx;
                if (fa == fb) { pc = 0x27481; continue; }
                pc = 0x27479; continue;
            }
            0x27479 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27470; continue; }
                pc = 0x2747f; continue;
            }
            0x2747f => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x27481 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2745e; continue; }
                pc = 0x27486; continue;
            }
            0x27486 => {
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x14094) as i32)) as u32;
                ecx = ecx.wrapping_add(0x5836d0);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x29b40); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000274A0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f274a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x274a0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x274a0, previous, pc); previous = pc; }
        match pc {
            0x274a0 => {
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                push!(edi);
                crate::audio_lane::set_return(0x274ac);
                call!(0x26bb0);
                pc = 0x274ac; continue;
            }
            0x274ac => {
                esi = eax;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x275de; continue; }
                pc = 0x274b6; continue;
            }
            0x274b6 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (!(((((fa) & (fb))) as u8).count_ones() % 2 == 0)) { pc = 0x275c5; continue; }
                pc = 0x274cd; continue;
            }
            0x274cd => {
                eax = m.read32(esi.wrapping_add(0x18));
                ecx = m.read32(0x4ca9b4);
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                edx = ((m.read8(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x4))) as u8 as u32);
                eax = ((m.read16(edx.wrapping_mul(0x2).wrapping_add(0x4ca9b8))) as u16 as u32);
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                ecx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                ecx = ((ecx as i32).wrapping_mul((0xbb80) as i32)) as u32;
                m.write32(esp.wrapping_add(0x8), ecx);
                push!(ebx);
                fpu.push((m.read32(esp.wrapping_add(0xc)) as i32 as f64));
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4761d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                crate::audio_lane::set_return(0x2750c);
                call!(0x3c848c);
                pc = 0x2750c; continue;
            }
            0x2750c => {
                ebx = eax;
                fa = ((((m.read8(esi.wrapping_add(0x17)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x27522; continue; }
                pc = 0x27514; continue;
            }
            0x27514 => {
                edx = m.read32(esi);
                push!(0);
                push!(edx);
                crate::audio_lane::set_return(0x2751e);
                call_to!(0x00407ABC);
                pc = 0x2751e; continue;
            }
            0x2751e => {
                m.write8(esi.wrapping_add(0x17), (0) as u8);
                pc = 0x27522; continue;
            }
            0x27522 => {
                fa = ebx;
                fb = ((((0xbci32)) as u32) & 0xffffffffu32);
                if (fa >= fb) { pc = 0x27531; continue; }
                pc = 0x2752a; continue;
            }
            0x2752a => {
                ebx = 0xbc;
                pc = 0x2753e; continue;
            }
            0x27531 => {
                fa = ebx;
                fb = ((((0x2edefi32)) as u32) & 0xffffffffu32);
                if (fa <= fb) { pc = 0x2753e; continue; }
                pc = 0x27539; continue;
            }
            0x27539 => {
                ebx = 0x2edef;
                pc = 0x2753e; continue;
            }
            0x2753e => {
                eax = m.read32(esi);
                push!(ebx);
                push!(eax);
                crate::audio_lane::set_return(0x27547);
                call_to!(0x004084F2);
                pc = 0x27547; continue;
            }
            0x27547 => {
                eax = m.read32(esi.wrapping_add(0x18));
                ecx = m.read32(0x4ca9b4);
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                eax = eax.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                fa = ((((m.read8(eax.wrapping_add(ecx).wrapping_add(0x8)))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x275af; continue; }
                pc = 0x2755d; continue;
            }
            0x2755d => {
                edi = m.read32(eax.wrapping_add(0x565b58));
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x275b4; continue; }
                pc = 0x27567; continue;
            }
            0x27567 => {
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                xmm0.lanes[0] = (((ebx) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(0x8)));
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                edx = (edx & 0x3f);
                eax = eax.wrapping_add(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                ecx = eax.wrapping_add(eax.wrapping_mul(0x8));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                edx = (edx & 0x3f);
                eax = eax.wrapping_add(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                edx = eax.wrapping_add(eax.wrapping_mul(0x8));
                eax = m.read32(esi);
                ecx = ecx.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_sub(ecx);
                push!(edx);
                push!(ecx);
                push!(eax);
                crate::audio_lane::set_return(0x275af);
                call_to!(0x00407AD8);
                pc = 0x275af; continue;
            }
            0x275af => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x275b0; continue;
            }
            0x275b0 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x275b4 => {
                ecx = m.read32(esi);
                push!(0);
                push!(0);
                push!(ecx);
                crate::audio_lane::set_return(0x275c0);
                call_to!(0x00407AD8);
                pc = 0x275c0; continue;
            }
            0x275c0 => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x275c5 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x17))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x275b0; continue; }
                pc = 0x275cc; continue;
            }
            0x275cc => {
                edx = m.read32(esi);
                push!(0x1);
                push!(edx);
                crate::audio_lane::set_return(0x275d6);
                call_to!(0x00407ABC);
                pc = 0x275d6; continue;
            }
            0x275d6 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write8(esi.wrapping_add(0x17), (0x1) as u8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x275de => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x275b0; continue; }
                pc = 0x275e2; continue;
            }
            0x275e2 => {
                esi = 0;
                pc = 0x275e4; continue;
            }
            0x275e4 => {
                fa = m.read32(esi.wrapping_mul(0x4).wrapping_add(0x581990));
                fb = ecx;
                if (fa == fb) { pc = 0x275f7; continue; }
                pc = 0x275ed; continue;
            }
            0x275ed => {
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x275e4; continue; }
                pc = 0x275f3; continue;
            }
            0x275f3 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x275f7 => {
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x275b0; continue; }
                pc = 0x275fc; continue;
            }
            0x275fc => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (!(((((fa) & (fb))) as u8).count_ones() % 2 == 0)) { pc = 0x27675; continue; }
                pc = 0x2760f; continue;
            }
            0x2760f => {
                esi = ((esi as i32).wrapping_mul((0x14094) as i32)) as u32;
                eax = m.read32(esi.wrapping_add(0x58371c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x275b0; continue; }
                pc = 0x2761f; continue;
            }
            0x2761f => {
                ecx = m.read32(0x4ca9b4);
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                edx = ((m.read8(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x4))) as u8 as u32);
                eax = ((m.read16(edx.wrapping_mul(0x2).wrapping_add(0x4ca9b8))) as u16 as u32);
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                ecx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                ecx = ((ecx as i32).wrapping_mul((0xbb80) as i32)) as u32;
                m.write32(esp.wrapping_add(0x8), ecx);
                fpu.push((m.read32(esp.wrapping_add(0x8)) as i32 as f64));
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4761d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                crate::audio_lane::set_return(0x2765a);
                call!(0x3c848c);
                pc = 0x2765a; continue;
            }
            0x2765a => {
                esi = esi.wrapping_add(0x5836d0);
                ecx = esi;
                edi = eax;
                crate::audio_lane::set_return(0x27669);
                call!(0x29ee0);
                pc = 0x27669; continue;
            }
            0x27669 => {
                push!(edi);
                ecx = esi;
                crate::audio_lane::set_return(0x27671);
                call!(0x29b20);
                pc = 0x27671; continue;
            }
            0x27671 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x27675 => {
                ecx = esi;
                ecx = ((ecx as i32).wrapping_mul((0x14094) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = ecx.wrapping_add(0x5836d0);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x4);
                tail_to!(0x29ea0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00027690: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27690<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27690;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x27690, previous, pc); previous = pc; }
        match pc {
            0x27690 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                crate::audio_lane::set_return(0x27699);
                call!(0x26bb0);
                pc = 0x27699; continue;
            }
            0x27699 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x276bc; continue; }
                pc = 0x2769d; continue;
            }
            0x2769d => {
                ecx = m.read32(esp.wrapping_add(0x8));
                edx = m.read32(esp.wrapping_add(0xc));
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { ecx = edx; }
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d1c))).to_bits();
                m.write32(eax.wrapping_add(0x20), xmm0.lanes[0]);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x276bc => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x276f4; continue; }
                pc = 0x276c0; continue;
            }
            0x276c0 => {
                eax = 0;
                pc = 0x276c2; continue;
            }
            0x276c2 => {
                fa = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x581990));
                fb = ecx;
                if (fa == fb) { pc = 0x276d2; continue; }
                pc = 0x276cb; continue;
            }
            0x276cb => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x276c2; continue; }
                pc = 0x276d1; continue;
            }
            0x276d1 => {
                esp = esp.wrapping_add(0); break 'run;
            }
            0x276d2 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x276f4; continue; }
                pc = 0x276d7; continue;
            }
            0x276d7 => {
                ecx = m.read32(esp.wrapping_add(0xc));
                edx = m.read32(esp.wrapping_add(0x8));
                push!(ecx);
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x14094) as i32)) as u32;
                push!(edx);
                ecx = ecx.wrapping_add(0x5836d0);
                crate::audio_lane::set_return(0x276f4);
                call!(0x29af0);
                pc = 0x276f4; continue;
            }
            0x276f4 => {
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00027700: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27700<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27700;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x27700, previous, pc); previous = pc; }
        match pc {
            0x27700 => {
                push!(ebp);
                ebp = esp;
                esp = (esp & 0xfffffff8u32);
                esp = esp.wrapping_sub(0xe0);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                crate::audio_lane::set_return(0x27715);
                call!(0x389b0);
                pc = 0x27715; continue;
            }
            0x27715 => {
                eax = m.read32(0x79094c);
                fa = eax;
                fb = ((((0x67i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27969; continue; }
                pc = 0x27723; continue;
            }
            0x27723 => {
                fa = eax;
                fb = ((((0x68i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27969; continue; }
                pc = 0x2772c; continue;
            }
            0x2772c => {
                fa = eax;
                fb = ((((0x6ai32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27969; continue; }
                pc = 0x27735; continue;
            }
            0x27735 => {
                ebp = 0;
                push!(ebp);
                crate::audio_lane::set_return(0x2773d);
                call!(0x1553b0);
                pc = 0x2773d; continue;
            }
            0x2773d => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                push!(ebp);
                push!(0x4761f8);
                crate::audio_lane::set_return(0x2774d);
                call!(0x62f00);
                pc = 0x2774d; continue;
            }
            0x2774d => {
                ecx = m.read32(esp.wrapping_add(0x34));
                esp = esp.wrapping_add(0x10);
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x20), eax);
                if (fa == fb) { pc = 0x27969; continue; }
                pc = 0x27761; continue;
            }
            0x27761 => {
                ecx = (ecx & 0xffff_0000) | ((((m.read8(eax)) as u8 as u32)) & 0xffff);
                ebx = eax.wrapping_add(0x1);
                edx = 0;
                edx = (edx & 0xffff00ff) | ((((m.read8(ebx)) as u32) & 0xff) << 8);
                push!(ebp);
                ecx = (ecx | edx);
                esi = ((((ecx) as u16)) as u16 as u32);
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0xd4) as i32)) as u32;
                eax = eax.wrapping_add(0xf);
                eax = (eax & 0xfffffff0u32);
                push!(eax);
                ebx = ebx.wrapping_add(1);
                crate::audio_lane::set_return(0x27787);
                call!(0x3dfa0);
                pc = 0x27787; continue;
            }
            0x27787 => {
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                if (fas <= fbs) { pc = 0x27816; continue; }
                pc = 0x27796; continue;
            }
            0x27796 => {
                m.write32(esp.wrapping_add(0x18), eax);
                m.write32(esp.wrapping_add(0x14), esi);
                edi = edi;
                pc = 0x277a0; continue;
            }
            0x277a0 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(ebx)) as u32)) & 0xff);
                ebp = ((((eax) as u8)) as u8 as u32);
                ecx = ebp;
                edx = ecx;
                ecx = ecx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                ebx = ebx.wrapping_add(1);
                esi = ebx;
                edi = esp.wrapping_add(0x28);
                { let step = if entry.eflags & 0x400 == 0 { 4u32 } else { 4u32.wrapping_neg() }; for i in 0..ecx { let offset = i.wrapping_mul(step); let value = m.read32(esi.wrapping_add(offset)); m.write32(edi.wrapping_add(offset), value); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); ecx = 0; }
                ecx = edx;
                ecx = (ecx & 0x3);
                { let step = if entry.eflags & 0x400 == 0 { 1u32 } else { 1u32.wrapping_neg() }; for i in 0..ecx { let offset = i.wrapping_mul(step); let value = m.read8(esi.wrapping_add(offset)); m.write8(edi.wrapping_add(offset), value); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); ecx = 0; }
                fa = ((((m.read8(esp.wrapping_add(ebp).wrapping_add(0x25)))) as u32) & 0xffu32);
                fb = ((((0x76i32)) as u32) & 0xffu32);
                m.write8(esp.wrapping_add(ebp).wrapping_add(0x28), (0) as u8);
                m.write8(esp.wrapping_add(ebp).wrapping_add(0x26), (0x62) as u8);
                m.write8(esp.wrapping_add(ebp).wrapping_add(0x25), (0x78) as u8);
                if (fa != fb) { pc = 0x277d9; continue; }
                pc = 0x277d2; continue;
            }
            0x277d2 => {
                m.write8(esp.wrapping_add(ebp).wrapping_add(0x27), (0x73) as u8);
                pc = 0x277de; continue;
            }
            0x277d9 => {
                m.write8(esp.wrapping_add(ebp).wrapping_add(0x27), (0x6d) as u8);
                pc = 0x277de; continue;
            }
            0x277de => {
                eax = m.read32(esp.wrapping_add(0x18));
                push!(eax);
                ecx = esp.wrapping_add(0x2c);
                push!(ecx);
                crate::audio_lane::set_return(0x277ed);
                call!(0x350c70);
                pc = 0x277ed; continue;
            }
            0x277ed => {
                eax = m.read32(esp.wrapping_add(0x20));
                m.write32(eax.wrapping_add(0xc8), 0);
                eax = eax.wrapping_add(0xd4);
                m.write32(esp.wrapping_add(0x20), eax);
                eax = m.read32(esp.wrapping_add(0x1c));
                esp = esp.wrapping_add(0x8);
                ebx = ebx.wrapping_add(ebp);
                eax = eax.wrapping_sub(1);
                fa = eax;
                m.write32(esp.wrapping_add(0x14), eax);
                if ((fa != 0)) { pc = 0x277a0; continue; }
                pc = 0x27814; continue;
            }
            0x27814 => {
                ebp = 0;
                pc = 0x27816; continue;
            }
            0x27816 => {
                edi = 0;
                m.write32(esp.wrapping_add(0x14), ebx);
                esi = 0;
                edi = edi;
                pc = 0x27820; continue;
            }
            0x27820 => {
                eax = m.read32(0x4ca9b4);
                edx = ((m.read8(ebx)) as u8 as u32);
                fa = m.read32(esi.wrapping_add(eax).wrapping_add(0xc));
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                ebx = ebx.wrapping_add(edx).wrapping_add(0x3);
                if (fas <= fbs) { pc = 0x2783b; continue; }
                pc = 0x27832; continue;
            }
            0x27832 => {
                m.write32(esi.wrapping_add(eax).wrapping_add(0xc), ebp);
                eax = m.read32(0x4ca9b4);
                pc = 0x2783b; continue;
            }
            0x2783b => {
                m.write32(esi.wrapping_add(eax).wrapping_add(0x10), ebp);
                eax = m.read32(0x4ca9b4);
                m.write32(esi.wrapping_add(eax), ebp);
                ecx = (ecx & 0xffff_0000) | ((((m.read16(ebx)) as u32)) & 0xffff);
                edx = m.read32(0x4ca9b4);
                write16(m, esi.wrapping_add(edx).wrapping_add(0x4), (((((ecx) as u16)) as u32)) as u16);
                eax = m.read32(ebx.wrapping_add(0x2));
                ecx = m.read32(0x4ca9b4);
                ebx = ebx.wrapping_add(0x2);
                m.write32(esi.wrapping_add(ecx).wrapping_add(0x8), eax);
                edx = (edx & 0xffff_0000) | ((((m.read16(ebx.wrapping_add(0x4))) as u32)) & 0xffff);
                eax = m.read32(0x4ca9b4);
                ebx = ebx.wrapping_add(0x4);
                push!(edi);
                write16(m, esi.wrapping_add(eax).wrapping_add(0x6), (((((edx) as u16)) as u32)) as u16);
                ebx = ebx.wrapping_add(0x2);
                crate::audio_lane::set_return(0x2787f);
                call!(0x45670);
                pc = 0x2787f; continue;
            }
            0x2787f => {
                esp = esp.wrapping_add(0x4);
                esi = esi.wrapping_add(0x14);
                edi = edi.wrapping_add(1);
                fa = esi;
                fb = ((((0x1bd8ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27820; continue; }
                pc = 0x2788e; continue;
            }
            0x2788e => {
                esi = m.read32(esp.wrapping_add(0x14));
                edi = 0;
                pc = 0x27894; continue;
            }
            0x27894 => {
                ecx = ((m.read8(esi)) as u8 as u32);
                ebx = ((m.read8(esi.wrapping_add(ecx).wrapping_add(0x1))) as u8 as u32);
                esi = esi.wrapping_add(ecx).wrapping_add(0x1);
                edx = 0;
                edx = (edx & 0xffff00ff) | ((((m.read8(esi.wrapping_add(0x1))) as u32) & 0xff) << 8);
                esi = esi.wrapping_add(1);
                push!(ebp);
                esi = esi.wrapping_add(0x9);
                ebx = (ebx | edx);
                crate::audio_lane::set_return(0x278b1);
                call!(0x45670);
                pc = 0x278b1; continue;
            }
            0x278b1 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27942; continue; }
                pc = 0x278bc; continue;
            }
            0x278bc => {
                eax = m.read32(esp.wrapping_add(0x1c));
                ebx = ((ebx as i32).wrapping_mul((0xd4) as i32)) as u32;
                ebx = ebx.wrapping_add(eax);
                eax = m.read32(ebx.wrapping_add(0xc8));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x278eb; continue; }
                pc = 0x278d2; continue;
            }
            0x278d2 => {
                eax = ebp;
                m.write32(ebx.wrapping_add(0xc8), 0x1);
                m.write32(ebx.wrapping_add(0xd0), ebp);
                crate::audio_lane::set_return(0x278e9);
                call!(0x27290);
                pc = 0x278e9; continue;
            }
            0x278e9 => {
                pc = 0x27942; continue;
            }
            0x278eb => {
                eax = m.read32(ebx.wrapping_add(0xd0));
                ecx = eax.wrapping_add(eax.wrapping_mul(0x4));
                edx = ecx.wrapping_mul(0x4).wrapping_add(0x565b48);
                ecx = m.read32(edx);
                eax = edi.wrapping_add(0x565b48);
                m.write32(eax, ecx);
                ecx = m.read32(edx.wrapping_add(0x4));
                m.write32(eax.wrapping_add(0x4), ecx);
                ecx = m.read32(edx.wrapping_add(0x8));
                m.write32(eax.wrapping_add(0x8), ecx);
                ecx = m.read32(edx.wrapping_add(0xc));
                m.write32(eax.wrapping_add(0xc), ecx);
                edx = m.read32(edx.wrapping_add(0x10));
                m.write32(eax.wrapping_add(0x10), edx);
                eax = m.read32(0x4ca9b4);
                m.write32(edi.wrapping_add(eax).wrapping_add(0xc), 0x1);
                ebx = m.read32(ebx.wrapping_add(0xd0));
                eax = m.read32(0x4ca9b4);
                ecx = ebx.wrapping_add(ebx.wrapping_mul(0x4));
                edx = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x565b54));
                m.write32(edi.wrapping_add(eax), edx);
                pc = 0x27942; continue;
            }
            0x27942 => {
                edi = edi.wrapping_add(0x14);
                ebp = ebp.wrapping_add(1);
                fa = edi;
                fb = ((((0x1bd8ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27894; continue; }
                pc = 0x27952; continue;
            }
            0x27952 => {
                ecx = m.read32(esp.wrapping_add(0x1c));
                push!(ecx);
                crate::audio_lane::set_return(0x2795c);
                call!(0x3e1e0);
                pc = 0x2795c; continue;
            }
            0x2795c => {
                edx = m.read32(esp.wrapping_add(0x24));
                push!(edx);
                crate::audio_lane::set_return(0x27966);
                call!(0x3e1e0);
                pc = 0x27966; continue;
            }
            0x27966 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x27969; continue;
            }
            0x27969 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00027CC0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27cc0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27cc0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x27cc0, previous, pc); previous = pc; }
        match pc {
            0x27cc0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                esp = esp.wrapping_sub(0xc);
                push!(esi);
                crate::audio_lane::set_return(0x27ccd);
                call!(0x26bb0);
                pc = 0x27ccd; continue;
            }
            0x27ccd => {
                esi = eax;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27d39; continue; }
                pc = 0x27cd3; continue;
            }
            0x27cd3 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x14))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x27d29; continue; }
                pc = 0x27cda; continue;
            }
            0x27cda => {
                eax = m.read32(esp.wrapping_add(0x18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27cf3; continue; }
                pc = 0x27ce2; continue;
            }
            0x27ce2 => {
                ecx = m.read32(eax);
                m.write32(esi.wrapping_add(0x8), ecx);
                edx = m.read32(eax.wrapping_add(0x4));
                m.write32(esi.wrapping_add(0xc), edx);
                eax = m.read32(eax.wrapping_add(0x8));
                m.write32(esi.wrapping_add(0x10), eax);
                pc = 0x27cf3; continue;
            }
            0x27cf3 => {
                push!(edi);
                edi = m.read32(esi);
                ecx = esi.wrapping_add(0x8);
                push!(ecx);
                edx = esp.wrapping_add(0xc);
                push!(edx);
                crate::audio_lane::set_return(0x27d04);
                call!(0x26d20);
                pc = 0x27d04; continue;
            }
            0x27d04 => {
                xmm0 = Xmm::default();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                eax = m.read32(esp.wrapping_add(0x14));
                esp = esp.wrapping_add(0x8);
                push!(0x1);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x10));
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                push!(ecx);
                push!(edi);
                crate::audio_lane::set_return(0x27d28);
                call_to!(0x00408556);
                pc = 0x27d28; continue;
            }
            0x27d28 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x27d29; continue;
            }
            0x27d29 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                m.write32(esi.wrapping_add(0x28), xmm0.lanes[0]);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x27d39 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27d97; continue; }
                pc = 0x27d3d; continue;
            }
            0x27d3d => {
                esi = 0;
                pc = 0x27d40; continue;
            }
            0x27d40 => {
                fa = m.read32(esi.wrapping_mul(0x4).wrapping_add(0x581990));
                fb = ecx;
                if (fa == fb) { pc = 0x27d54; continue; }
                pc = 0x27d49; continue;
            }
            0x27d49 => {
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27d40; continue; }
                pc = 0x27d4f; continue;
            }
            0x27d4f => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x27d54 => {
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27d97; continue; }
                pc = 0x27d59; continue;
            }
            0x27d59 => {
                ecx = m.read32(esp.wrapping_add(0x18));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27d7f; continue; }
                pc = 0x27d61; continue;
            }
            0x27d61 => {
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x14094) as i32)) as u32;
                edx = m.read32(eax.wrapping_add(0x583720));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27d7f; continue; }
                pc = 0x27d73; continue;
            }
            0x27d73 => {
                push!(ecx);
                ecx = eax.wrapping_add(0x5836d0);
                crate::audio_lane::set_return(0x27d7f);
                call!(0x29dd0);
                pc = 0x27d7f; continue;
            }
            0x27d7f => {
                edx = m.read32(esp.wrapping_add(0x1c));
                ecx = esi;
                ecx = ((ecx as i32).wrapping_mul((0x14094) as i32)) as u32;
                push!(edx);
                ecx = ecx.wrapping_add(0x5836d0);
                crate::audio_lane::set_return(0x27d97);
                call!(0x29e50);
                pc = 0x27d97; continue;
            }
            0x27d97 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00028110: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28110<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28110;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28110, previous, pc); previous = pc; }
        match pc {
            0x28110 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                crate::audio_lane::set_return(0x28119);
                call!(0x26bb0);
                pc = 0x28119; continue;
            }
            0x28119 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28131; continue; }
                pc = 0x2811d; continue;
            }
            0x2811d => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47621c))).to_bits();
                m.write32(eax.wrapping_add(0x24), xmm0.lanes[0]);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x28131 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x28173; continue; }
                pc = 0x28135; continue;
            }
            0x28135 => {
                eax = 0;
                pc = 0x28137; continue;
            }
            0x28137 => {
                fa = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x581990));
                fb = ecx;
                if (fa == fb) { pc = 0x28147; continue; }
                pc = 0x28140; continue;
            }
            0x28140 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x28137; continue; }
                pc = 0x28146; continue;
            }
            0x28146 => {
                esp = esp.wrapping_add(0); break 'run;
            }
            0x28147 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28173; continue; }
                pc = 0x2814c; continue;
            }
            0x2814c => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47621c))).to_bits();
                push!(ecx);
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x14094) as i32)) as u32;
                ecx = ecx.wrapping_add(0x5836d0);
                m.write32(esp, xmm0.lanes[0]);
                crate::audio_lane::set_return(0x28173);
                call!(0x29d90);
                pc = 0x28173; continue;
            }
            0x28173 => {
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00028180: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28180<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28180;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28180, previous, pc); previous = pc; }
        match pc {
            0x28180 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x8)));
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                eax = m.read32(esp.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                xmm3 = xmm1;
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                if (fas < fbs) { pc = 0x281e7; continue; }
                pc = 0x281b7; continue;
            }
            0x281b7 => {
                eax = m.read32(esp.wrapping_add(0x14));
                eax = (eax & 0x200);
                push!(eax);
                crate::audio_lane::set_return(0x281c6);
                call!(0x457b0);
                pc = 0x281c6; continue;
            }
            0x281c6 => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0xc))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0xc), (fpu.st(0) as f32).to_bits()); fpu.pop();
                crate::audio_lane::set_return(0x281d3);
                call!(0x69640);
                pc = 0x281d3; continue;
            }
            0x281d3 => {
                ecx = m.read32(esp.wrapping_add(0xc));
                eax = m.read32(eax.wrapping_add(0x11c));
                push!(ecx);
                crate::audio_lane::set_return(0x281e3);
                call!(0x26f00);
                pc = 0x281e3; continue;
            }
            0x281e3 => {
                esp = esp.wrapping_add(0x8);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x281e7 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                if ((fca > fcb)) { pc = 0x281ff; continue; }
                pc = 0x281f2; continue;
            }
            0x281f2 => {
                xmm2 = Xmm::default();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x28238; continue; }
                pc = 0x281fa; continue;
            }
            0x281fa => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x2821a; continue; }
                pc = 0x281ff; continue;
            }
            0x281ff => {
                m.write32(esp.wrapping_add(0x8), xmm1.lanes[0]);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x8))) as f64));
                fpu.push(0.30102999566398119521f64);
                { let a = fpu.top as usize; let b = ((fpu.top + 1) & 7) as usize; fpu.stack.swap(a, b); }
                { let result = fpu.st(1) * crate::c2r::rt::log2(fpu.st(0)); fpu.stack[((fpu.top + 1) & 7) as usize] = result; fpu.pop(); }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4761d4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                eax = (fpu.st(0) as i64) as u32; fpu.pop(); break 'run;
            }
            0x2821a => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475d60)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x28238; continue; }
                pc = 0x28223; continue;
            }
            0x28223 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x8))) as f64));
                fpu.push(0.30102999566398119521f64);
                { let a = fpu.top as usize; let b = ((fpu.top + 1) & 7) as usize; fpu.stack.swap(a, b); }
                { let result = fpu.st(1) * crate::c2r::rt::log2(fpu.st(0)); fpu.stack[((fpu.top + 1) & 7) as usize] = result; fpu.pop(); }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4761d4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                eax = (fpu.st(0) as i64) as u32; fpu.pop(); break 'run;
            }
            0x28238 => {
                eax = 0xffffd8f0u32;
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00028760: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28760<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28760;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28760, previous, pc); previous = pc; }
        match pc {
            0x28760 => {
                push!(ecx);
                xmm0 = Xmm::default();
                push!(ebx);
                push!(esi);
                esi = m.read32(0x5659f4);
                fa = esi;
                fb = ((((0x5659c4i32)) as u32) & 0xffffffffu32);
                push!(edi);
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                ebx = 0x200;
                if (fa == fb) { pc = 0x287c5; continue; }
                pc = 0x28780; continue;
            }
            0x28780 => {
                fa = m.read32(esi);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = m.read32(esi.wrapping_add(0x30));
                if (fa == fb) { pc = 0x287bb; continue; }
                pc = 0x28788; continue;
            }
            0x28788 => {
                push!(0);
                eax = esi;
                crate::audio_lane::set_return(0x28791);
                call!(0x28610);
                pc = 0x28791; continue;
            }
            0x28791 => {
                eax = m.read32(esi.wrapping_add(0x1c));
                esp = esp.wrapping_add(0x4);
                fa = ebx;
                fb = eax;
                if ((fa & fb) == 0) { pc = 0x287bb; continue; }
                pc = 0x2879b; continue;
            }
            0x2879b => {
                eax = m.read32(esi.wrapping_add(0x2c));
                eax = eax.wrapping_add(0x2710);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x476240))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xc)))).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                pc = 0x287bb; continue;
            }
            0x287bb => {
                fa = edi;
                fb = ((((0x5659c4i32)) as u32) & 0xffffffffu32);
                esi = edi;
                if (fa != fb) { pc = 0x28780; continue; }
                pc = 0x287c5; continue;
            }
            0x287c5 => {
                esi = m.read32(0x5836c0);
                fa = esi;
                fb = ((((0x583690i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28819; continue; }
                pc = 0x287d3; continue;
            }
            0x287d3 => {
                fa = m.read32(esi);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = m.read32(esi.wrapping_add(0x30));
                if (fa == fb) { pc = 0x2880f; continue; }
                pc = 0x287db; continue;
            }
            0x287db => {
                push!(0x1);
                eax = esi;
                crate::audio_lane::set_return(0x287e4);
                call!(0x28610);
                pc = 0x287e4; continue;
            }
            0x287e4 => {
                eax = m.read32(esi.wrapping_add(0x1c));
                esp = esp.wrapping_add(0x4);
                fa = ebx;
                fb = eax;
                if ((fa & fb) == 0) { pc = 0x2880f; continue; }
                pc = 0x287ee; continue;
            }
            0x287ee => {
                ecx = m.read32(esi.wrapping_add(0x2c));
                ecx = ecx.wrapping_add(0x2710);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x476240))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xc)))).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                pc = 0x2880f; continue;
            }
            0x2880f => {
                fa = edi;
                fb = ((((0x583690i32)) as u32) & 0xffffffffu32);
                esi = edi;
                if (fa != fb) { pc = 0x287d3; continue; }
                pc = 0x28819; continue;
            }
            0x28819 => {
                esi = 0x5836d0;
                edi = edi;
                pc = 0x28820; continue;
            }
            0x28820 => {
                ecx = esi;
                crate::audio_lane::set_return(0x28827);
                call!(0x29f30);
                pc = 0x28827; continue;
            }
            0x28827 => {
                ecx = esi;
                crate::audio_lane::set_return(0x2882e);
                call!(0x29cc0);
                pc = 0x2882e; continue;
            }
            0x2882e => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x28858; continue; }
                pc = 0x28832; continue;
            }
            0x28832 => {
                fa = m.read32(esi.wrapping_add(0x68));
                fb = ebx;
                if ((fa & fb) == 0) { pc = 0x28858; continue; }
                pc = 0x28837; continue;
            }
            0x28837 => {
                edx = m.read32(esi.wrapping_add(0x60));
                edx = edx.wrapping_add(0x2710);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x476240))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xc)))).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                pc = 0x28858; continue;
            }
            0x28858 => {
                esi = esi.wrapping_add(0x14094);
                fa = esi;
                fb = ((((0x64bc98i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x28820; continue; }
                pc = 0x28866; continue;
            }
            0x28866 => {
                eax = m.read32(esp.wrapping_add(0xc));
                push!(eax);
                crate::audio_lane::set_return(0x28870);
                call!(0x462f0);
                pc = 0x28870; continue;
            }
            0x28870 => {
                ecx = m.read32(0x581988);
                esp = esp.wrapping_add(0x4);
                push!(ecx);
                crate::audio_lane::set_return(0x2887f);
                call_to!(0x00409109);
                pc = 0x2887f; continue;
            }
            0x2887f => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x4);
                tail_to!(0x00407B40); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000298D0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f298d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x298d0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x298d0, previous, pc); previous = pc; }
        match pc {
            0x298d0 => {
                esp = esp.wrapping_sub(0x50);
                push!(ebx);
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x60));
                eax = 0;
                ebx = 0;
                fa = edi;
                fb = ebx;
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = ecx;
                ecx = esp.wrapping_add(0xc);
                eax = eax.wrapping_add(1);
                push!(eax);
                push!(0x4);
                push!(0xac44);
                push!(ecx);
                crate::audio_lane::set_return(0x298f8);
                call!(0x26c90);
                pc = 0x298f8; continue;
            }
            0x298f8 => {
                edx = 0;
                m.write32(esp.wrapping_add(0x30), edx);
                m.write32(esp.wrapping_add(0x34), edx);
                m.write32(esp.wrapping_add(0x38), edx);
                m.write32(esp.wrapping_add(0x3c), edx);
                esp = esp.wrapping_add(0x10);
                fa = edi;
                fb = ebx;
                m.write32(esp.wrapping_add(0x30), edx);
                m.write32(esi.wrapping_add(0x50), edi);
                m.write32(esp.wrapping_add(0x34), edx);
                if (fa == fb) { pc = 0x29925; continue; }
                pc = 0x2991c; continue;
            }
            0x2991c => {
                eax = edx;
                eax = (eax | 0x10);
                m.write32(esp.wrapping_add(0x20), eax);
                pc = 0x29925; continue;
            }
            0x29925 => {
                esi = esi.wrapping_add(0x4);
                push!(esi);
                ecx = esp.wrapping_add(0x24);
                eax = esp.wrapping_add(0x10);
                push!(ecx);
                m.write32(esp.wrapping_add(0x2c), 0x3);
                m.write32(esp.wrapping_add(0x30), eax);
                crate::audio_lane::set_return(0x29943);
                call_to!(0x0040967C);
                pc = 0x29943; continue;
            }
            0x29943 => {
                fa = eax;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x299aa; continue; }
                pc = 0x29947; continue;
            }
            0x29947 => {
                fa = edi;
                fb = ebx;
                if (fa == fb) { pc = 0x299a8; continue; }
                pc = 0x2994b; continue;
            }
            0x2994b => {
                xmm0 = Xmm::default();
                eax = m.read32(esi);
                push!(ebx);
                edx = esp.wrapping_add(0x3c);
                push!(edx);
                push!(eax);
                m.write32(esp.wrapping_add(0x44), ebx);
                m.write32(esp.wrapping_add(0x48), ebx);
                m.write32(esp.wrapping_add(0x4c), 0xfffff448u32);
                m.write32(esp.wrapping_add(0x50), ebx);
                m.write32(esp.wrapping_add(0x54), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x58), ebx);
                m.write32(esp.wrapping_add(0x5c), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x60), ebx);
                m.write32(esp.wrapping_add(0x64), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x2998a);
                call_to!(0x00408637);
                pc = 0x2998a; continue;
            }
            0x2998a => {
                ecx = m.read32(esi);
                push!(ebx);
                push!(0x3f800000);
                push!(ecx);
                crate::audio_lane::set_return(0x29998);
                call_to!(0x004085F1);
                pc = 0x29998; continue;
            }
            0x29998 => {
                edx = m.read32(esi);
                push!(ebx);
                push!(0x4);
                push!(0x4b914c);
                push!(edx);
                crate::audio_lane::set_return(0x299a8);
                call_to!(0x00408632);
                pc = 0x299a8; continue;
            }
            0x299a8 => {
                eax = 0;
                pc = 0x299aa; continue;
            }
            0x299aa => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x50);
                esp = esp.wrapping_add(4); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000299C0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f299c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x299c0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x299c0, previous, pc); previous = pc; }
        match pc {
            0x299c0 => {
                esp = esp.wrapping_sub(0x14);
                push!(ebx);
                push!(esi);
                esi = ecx;
                ecx = m.read32(esp.wrapping_add(0x3c));
                ebx = 0;
                eax = 0;
                fa = ecx;
                fb = ebx;
                ecx = m.read32(esp.wrapping_add(0x38));
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edx = esp.wrapping_add(0x8);
                m.write8(esi, (((((ebx) as u8)) as u32)) as u8);
                eax = eax.wrapping_add(1);
                push!(eax);
                push!(0x4);
                push!(ecx);
                push!(edx);
                crate::audio_lane::set_return(0x299e9);
                call!(0x26c90);
                pc = 0x299e9; continue;
            }
            0x299e9 => {
                ecx = m.read32(esi.wrapping_add(0x4));
                esp = esp.wrapping_add(0x10);
                eax = esp.wrapping_add(0x8);
                push!(eax);
                push!(ecx);
                crate::audio_lane::set_return(0x299fa);
                call_to!(0x00408C2D);
                pc = 0x299fa; continue;
            }
            0x299fa => {
                edx = m.read32(esp.wrapping_add(0x30));
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                ecx = m.read32(esp.wrapping_add(0x2c));
                eax = m.read32(esp.wrapping_add(0x20));
                push!(ebx);
                push!(0x3f800000);
                push!(edx);
                m.write32(esi.wrapping_add(0x58), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                push!(ecx);
                m.write32(esi.wrapping_add(0x54), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                push!(eax);
                m.write32(esi.wrapping_add(0x4c), eax);
                m.write32(esi.wrapping_add(0x5c), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x29a3a);
                call!(0x28180);
                pc = 0x29a3a; continue;
            }
            0x29a3a => {
                edx = m.read32(esi.wrapping_add(0x4));
                esp = esp.wrapping_add(0x14);
                push!(0x1);
                push!(edx);
                m.write32(esi.wrapping_add(0x60), eax);
                m.write32(esi.wrapping_add(0x24), ebx);
                m.write32(esi.wrapping_add(0x64), ebx);
                crate::audio_lane::set_return(0x29a51);
                call_to!(0x00407B23);
                pc = 0x29a51; continue;
            }
            0x29a51 => {
                eax = m.read32(esi.wrapping_add(0x4));
                ecx = m.read32(eax);
                push!(eax);
                crate::audio_lane::set_return(0x29a5a);
                call_to!(m.read32(ecx.wrapping_add(0x14)));
                pc = 0x29a5a; continue;
            }
            0x29a5a => {
                edx = m.read32(esi.wrapping_add(0x60));
                eax = m.read32(esi.wrapping_add(0x4));
                push!(edx);
                push!(eax);
                crate::audio_lane::set_return(0x29a67);
                call_to!(0x00407B14);
                pc = 0x29a67; continue;
            }
            0x29a67 => {
                eax = m.read32(esp.wrapping_add(0x24));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x29a7d; continue; }
                pc = 0x29a70; continue;
            }
            0x29a70 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                esp = esp.wrapping_add(36); break 'run;
            }
            0x29a7d => {
                ecx = m.read32(esp.wrapping_add(0x34));
                edx = m.read32(esp.wrapping_add(0x28));
                m.write32(esi.wrapping_add(0x38), eax);
                eax = m.read32(esp.wrapping_add(0x40));
                fa = ecx;
                fb = ebx;
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                m.write32(esi.wrapping_add(0x48), eax);
                m.write8(esi.wrapping_add(0x2), (((((ebx) as u8)) as u32)) as u8);
                m.write8(esi.wrapping_add(0x1), (((((ecx) as u8)) as u32)) as u8);
                m.write32(esi.wrapping_add(0x3c), edx);
                m.write32(esi.wrapping_add(0x40), ebx);
                m.write32(esi.wrapping_add(0x2c), 0x1);
                m.write32(esi.wrapping_add(0xc), ebx);
                m.write32(esi.wrapping_add(0x10), ebx);
                m.write32(esi.wrapping_add(0x14), ebx);
                fa = m.read32(esi.wrapping_add(0x50));
                fb = ebx;
                m.write32(esi.wrapping_add(0x28), ebx);
                if (fa == fb) { pc = 0x29ada; continue; }
                pc = 0x29ab8; continue;
            }
            0x29ab8 => {
                ecx = m.read32(esi.wrapping_add(0x4));
                push!(0x1);
                push!(0x3d4ccccd);
                push!(ebx);
                push!(ebx);
                push!(ecx);
                crate::audio_lane::set_return(0x29aca);
                call_to!(0x00408609);
                pc = 0x29aca; continue;
            }
            0x29aca => {
                edx = m.read32(esi.wrapping_add(0x4));
                push!(0x1);
                push!(0x42c80000);
                push!(edx);
                crate::audio_lane::set_return(0x29ada);
                call_to!(0x004085D9);
                pc = 0x29ada; continue;
            }
            0x29ada => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                esp = esp.wrapping_add(36); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00029AF0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29af0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29af0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29af0, previous, pc); previous = pc; }
        match pc {
            0x29af0 => {
                eax = m.read32(ecx.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29b15; continue; }
                pc = 0x29af7; continue;
            }
            0x29af7 => {
                eax = m.read32(esp.wrapping_add(0x4));
                edx = m.read32(esp.wrapping_add(0x8));
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { eax = edx; }
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d1c))).to_bits();
                m.write32(ecx.wrapping_add(0x54), xmm0.lanes[0]);
                pc = 0x29b15; continue;
            }
            0x29b15 => {
                esp = esp.wrapping_add(8); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00029B20: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29b20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29b20;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29b20, previous, pc); previous = pc; }
        match pc {
            0x29b20 => {
                push!(esi);
                esi = ecx;
                eax = m.read32(esi.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29b35; continue; }
                pc = 0x29b2a; continue;
            }
            0x29b2a => {
                ecx = m.read32(esp.wrapping_add(0x8));
                push!(ecx);
                push!(eax);
                crate::audio_lane::set_return(0x29b35);
                call_to!(0x004085CF);
                pc = 0x29b35; continue;
            }
            0x29b35 => {
                m.write32(esi.wrapping_add(0x24), 0);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(4); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00029BB0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29bb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29bb0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29bb0, previous, pc); previous = pc; }
        match pc {
            0x29bb0 => {
                eax = 0;
                ecx = ecx.wrapping_add(0xc);
                pc = 0x29bb5; continue;
            }
            0x29bb5 => {
                fa = m.read32(ecx);
                fb = ((0x8000000au32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x29bcb; continue; }
                pc = 0x29bbd; continue;
            }
            0x29bbd => {
                eax = eax.wrapping_add(1);
                ecx = ecx.wrapping_add(0x4);
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa < fb) { pc = 0x29bb5; continue; }
                pc = 0x29bc6; continue;
            }
            0x29bc6 => {
                eax = 0;
                esp = esp.wrapping_add(4); break 'run;
            }
            0x29bcb => {
                ecx = m.read32(esp.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29bd5; continue; }
                pc = 0x29bd3; continue;
            }
            0x29bd3 => {
                m.write32(ecx, eax);
                pc = 0x29bd5; continue;
            }
            0x29bd5 => {
                eax = 0x1;
                esp = esp.wrapping_add(4); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00029BE0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29be0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29be0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29be0, previous, pc); previous = pc; }
        match pc {
            0x29be0 => {
                eax = m.read32(esp.wrapping_add(0x8));
                push!(ebx);
                push!(ebp);
                push!(esi);
                esi = ecx;
                ecx = m.read32(eax);
                edx = m.read32(esi.wrapping_add(0x40));
                ebx = ecx.wrapping_add(edx);
                ecx = m.read32(esi.wrapping_add(0x3c));
                push!(edi);
                edi = edx;
                eax = ebx.wrapping_add(0x7ff);
                ebp = 0;
                edi = (edi & 0xfffff800u32);
                eax = (eax & 0xfffff800u32);
                fa = ebx;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x29c1d; continue; }
                pc = 0x29c0e; continue;
            }
            0x29c0e => {
                eax = ecx.wrapping_add(0x7ff);
                eax = (eax & 0xfffff800u32);
                ecx = ecx.wrapping_sub(edx);
                ebp = ecx;
                pc = 0x29c1d; continue;
            }
            0x29c1d => {
                ebx = m.read32(esp.wrapping_add(0x1c));
                edx = m.read32(ebx);
                ecx = m.read32(esi.wrapping_add(0x38));
                eax = eax.wrapping_sub(edi);
                push!(eax);
                push!(edi);
                eax = edx.wrapping_add(esi).wrapping_add(0x94);
                push!(eax);
                push!(ecx);
                crate::audio_lane::set_return(0x29c38);
                call!(0x29850);
                pc = 0x29c38; continue;
            }
            0x29c38 => {
                esp = esp.wrapping_add(0x10);
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29c6e; continue; }
                pc = 0x29c3f; continue;
            }
            0x29c3f => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x1))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x29c6e; continue; }
                pc = 0x29c46; continue;
            }
            0x29c46 => {
                eax = m.read32(esp.wrapping_add(0x18));
                m.write32(esi.wrapping_add(0x40), 0);
                m.write32(eax, m.read32(eax).wrapping_sub(ebp));
                edx = m.read32(esi.wrapping_add(0x3c));
                eax = m.read32(ebx);
                edx = edx.wrapping_sub(edi);
                eax = eax.wrapping_add(edx);
                m.write32(ebx, eax);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esi.wrapping_add(0x2c), 0x2);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(12); break 'run;
            }
            0x29c6e => {
                eax = m.read32(esp.wrapping_add(0x18));
                ecx = m.read32(eax);
                m.write32(esi.wrapping_add(0x40), m.read32(esi.wrapping_add(0x40)).wrapping_add(ecx));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax, 0);
                m.write32(esi.wrapping_add(0x2c), 0x2);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(12); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00029D50: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29d50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29d50;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29d50, previous, pc); previous = pc; }
        match pc {
            0x29d50 => {
                eax = m.read32(ecx.wrapping_add(0x50));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29d86; continue; }
                pc = 0x29d57; continue;
            }
            0x29d57 => {
                xmm0 = Xmm::scalar(m.read32(0x475d20));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                if ((fca > fcb)) { pc = 0x29d76; continue; }
                pc = 0x29d70; continue;
            }
            0x29d70 => {
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                pc = 0x29d76; continue;
            }
            0x29d76 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(ecx.wrapping_add(0x4));
                push!(0x1);
                push!(eax);
                push!(ecx);
                crate::audio_lane::set_return(0x29d86);
                call_to!(0x004085D9);
                pc = 0x29d86; continue;
            }
            0x29d86 => {
                esp = esp.wrapping_add(4); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00029D90: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29d90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29d90;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29d90, previous, pc); previous = pc; }
        match pc {
            0x29d90 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x29dbb; continue; }
                pc = 0x29da3; continue;
            }
            0x29da3 => {
                xmm2 = Xmm::default();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x29db6; continue; }
                pc = 0x29dab; continue;
            }
            0x29dab => {
                xmm0 = xmm2;
                pc = 0x29dae; continue;
            }
            0x29dae => {
                m.write32(ecx.wrapping_add(0x5c), xmm0.lanes[0]);
                esp = esp.wrapping_add(4); break 'run;
            }
            0x29db6 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x29dae; continue; }
                pc = 0x29dbb; continue;
            }
            0x29dbb => {
                m.write32(ecx.wrapping_add(0x5c), xmm1.lanes[0]);
                esp = esp.wrapping_add(4); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00029DD0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29dd0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29dd0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29dd0, previous, pc); previous = pc; }
        match pc {
            0x29dd0 => {
                esp = esp.wrapping_sub(0xc);
                push!(esi);
                esi = ecx;
                eax = m.read32(esi.wrapping_add(0x50));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29e2b; continue; }
                pc = 0x29ddd; continue;
            }
            0x29ddd => {
                eax = m.read32(esi.wrapping_add(0x64));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29e2b; continue; }
                pc = 0x29de4; continue;
            }
            0x29de4 => {
                eax = m.read32(esp.wrapping_add(0x14));
                ecx = m.read32(eax);
                m.write32(esi.wrapping_add(0x6c), ecx);
                edx = m.read32(eax.wrapping_add(0x4));
                m.write32(esi.wrapping_add(0x70), edx);
                ecx = m.read32(eax.wrapping_add(0x8));
                push!(eax);
                edx = esp.wrapping_add(0x8);
                push!(edx);
                m.write32(esi.wrapping_add(0x74), ecx);
                crate::audio_lane::set_return(0x29e04);
                call!(0x26d20);
                pc = 0x29e04; continue;
            }
            0x29e04 => {
                xmm0 = Xmm::default();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                eax = m.read32(esp.wrapping_add(0x10));
                edx = m.read32(esi.wrapping_add(0x4));
                esp = esp.wrapping_add(0x8);
                push!(0x1);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0xc));
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                push!(ecx);
                push!(edx);
                crate::audio_lane::set_return(0x29e2b);
                call_to!(0x00408609);
                pc = 0x29e2b; continue;
            }
            0x29e2b => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                esp = esp.wrapping_add(4); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00029E40: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29e40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29e40;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29e40, previous, pc); previous = pc; }
        match pc {
            0x29e40 => {
                eax = m.read32(esp.wrapping_add(0x4));
                m.write32(ecx.wrapping_add(0x64), eax);
                esp = esp.wrapping_add(4); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00029E50: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29e50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29e50;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29e50, previous, pc); previous = pc; }
        match pc {
            0x29e50 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                m.write32(ecx.wrapping_add(0x58), xmm0.lanes[0]);
                esp = esp.wrapping_add(4); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00029F30: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29f30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29f30;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29f30, previous, pc); previous = pc; }
        match pc {
            0x29f30 => {
                esp = esp.wrapping_sub(0x14);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                ebp = ecx;
                eax = m.read32(ebp.wrapping_add(0x58));
                ecx = m.read32(ebp.wrapping_add(0x5c));
                edx = m.read32(ebp.wrapping_add(0x54));
                edi = 0;
                push!(edi);
                push!(eax);
                eax = m.read32(ebp.wrapping_add(0x4c));
                push!(ecx);
                push!(edx);
                push!(eax);
                crate::audio_lane::set_return(0x29f51);
                call!(0x28180);
                pc = 0x29f51; continue;
            }
            0x29f51 => {
                ecx = m.read32(ebp.wrapping_add(0x60));
                esp = esp.wrapping_add(0x14);
                fa = eax;
                fb = ecx;
                if (fa == fb) { pc = 0x29f68; continue; }
                pc = 0x29f5b; continue;
            }
            0x29f5b => {
                ecx = m.read32(ebp.wrapping_add(0x4));
                push!(eax);
                push!(ecx);
                m.write32(ebp.wrapping_add(0x60), eax);
                crate::audio_lane::set_return(0x29f68);
                call_to!(0x00407B14);
                pc = 0x29f68; continue;
            }
            0x29f68 => {
                fa = m.read32(ebp.wrapping_add(0x64));
                fb = edi;
                if (fa == fb) { pc = 0x29fad; continue; }
                pc = 0x29f6d; continue;
            }
            0x29f6d => {
                fa = m.read32(ebp.wrapping_add(0x50));
                fb = edi;
                if (fa == fb) { pc = 0x29fad; continue; }
                pc = 0x29f72; continue;
            }
            0x29f72 => {
                fa = m.read32(ebp.wrapping_add(0x28));
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x29fad; continue; }
                pc = 0x29f78; continue;
            }
            0x29f78 => {
                edx = ebp.wrapping_add(0x6c);
                push!(edx);
                eax = esp.wrapping_add(0x18);
                push!(eax);
                crate::audio_lane::set_return(0x29f86);
                call!(0x26d20);
                pc = 0x29f86; continue;
            }
            0x29f86 => {
                xmm0 = Xmm::default();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                edx = m.read32(esp.wrapping_add(0x1c));
                eax = m.read32(ebp.wrapping_add(0x4));
                esp = esp.wrapping_add(0x8);
                push!(0x1);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x20));
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                push!(edx);
                push!(eax);
                crate::audio_lane::set_return(0x29fad);
                call_to!(0x00408609);
                pc = 0x29fad; continue;
            }
            0x29fad => {
                eax = m.read32(ebp.wrapping_add(0x2c));
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x2a08a; continue; }
                pc = 0x29fb7; continue;
            }
            0x29fb7 => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0x2a1f3; continue; }
                pc = 0x29fbe; continue;
            }
            0x29fbe => {
                ecx = m.read32(ebp.wrapping_add(0x38));
                push!(ecx);
                crate::audio_lane::set_return(0x29fc7);
                call!(0x294b0);
                pc = 0x29fc7; continue;
            }
            0x29fc7 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2a1f3; continue; }
                pc = 0x29fd2; continue;
            }
            0x29fd2 => {
                fa = m.read32(ebp.wrapping_add(0x20));
                fb = edi;
                if (fa == fb) { pc = 0x2a016; continue; }
                pc = 0x29fd7; continue;
            }
            0x29fd7 => {
                fa = m.read32(ebp.wrapping_add(0x30));
                fb = edi;
                edx = m.read32(ebp.wrapping_add(0x34));
                edx = edx.wrapping_add(ebp).wrapping_add(0x94);
                if (fa == fb) { pc = 0x2a016; continue; }
                pc = 0x29fe6; continue;
            }
            0x29fe6 => {
                eax = m.read32(ebp.wrapping_add(0x30));
                ecx = m.read32(ebp.wrapping_add(0x3c));
                esi = m.read32(ebp.wrapping_add(0x20));
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { eax = ecx; }
                ecx = eax;
                ebx = ecx;
                ecx = ecx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                edi = edx;
                { let step = if entry.eflags & 0x400 == 0 { 4u32 } else { 4u32.wrapping_neg() }; for i in 0..ecx { let offset = i.wrapping_mul(step); let value = m.read32(esi.wrapping_add(offset)); m.write32(edi.wrapping_add(offset), value); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); ecx = 0; }
                ecx = ebx;
                ecx = (ecx & 0x3);
                { let step = if entry.eflags & 0x400 == 0 { 1u32 } else { 1u32.wrapping_neg() }; for i in 0..ecx { let offset = i.wrapping_mul(step); let value = m.read8(esi.wrapping_add(offset)); m.write8(edi.wrapping_add(offset), value); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); ecx = 0; }
                esi = m.read32(ebp.wrapping_add(0x30));
                esi = esi.wrapping_sub(eax);
                edx = edx.wrapping_add(eax);
                eax = esi;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(ebp.wrapping_add(0x30), esi);
                if (fa != fb) { pc = 0x29fe6; continue; }
                pc = 0x2a016; continue;
            }
            0x2a016 => {
                ecx = m.read32(ebp.wrapping_add(0x30));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = ebp.wrapping_add(0x30);
                if (fa == fb) { pc = 0x2a03a; continue; }
                pc = 0x2a020; continue;
            }
            0x2a020 => {
                edx = m.read32(ebp.wrapping_add(0x44));
                ecx = ebp.wrapping_add(0x34);
                push!(ecx);
                push!(eax);
                push!(edx);
                ecx = ebp;
                crate::audio_lane::set_return(0x2a030);
                call!(0x29be0);
                pc = 0x2a030; continue;
            }
            0x2a030 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x2a03a => {
                eax = m.read32(ebp.wrapping_add(0x90));
                edi = m.read32(ebp.wrapping_add(0x78));
                esi = eax.wrapping_add(ebp).wrapping_add(0x94);
                ecx = 0x4800;
                { let step = if entry.eflags & 0x400 == 0 { 4u32 } else { 4u32.wrapping_neg() }; for i in 0..ecx { let offset = i.wrapping_mul(step); let value = m.read32(esi.wrapping_add(offset)); m.write32(edi.wrapping_add(offset), value); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); ecx = 0; }
                ecx = ebp;
                crate::audio_lane::set_return(0x2a058);
                call!(0x29c90);
                pc = 0x2a058; continue;
            }
            0x2a058 => {
                eax = m.read32(ebp.wrapping_add(0x28));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2a070; continue; }
                pc = 0x2a05f; continue;
            }
            0x2a05f => {
                ecx = ebp;
                crate::audio_lane::set_return(0x2a066);
                call!(0x29ee0);
                pc = 0x2a066; continue;
            }
            0x2a066 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x2a070 => {
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2a1f3; continue; }
                pc = 0x2a079; continue;
            }
            0x2a079 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(ebp.wrapping_add(0x28), 0x2);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x2a08a => {
                edx = m.read32(ebp.wrapping_add(0x40));
                esi = m.read32(ebp.wrapping_add(0x3c));
                fa = edx;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x2a18b; continue; }
                pc = 0x2a098; continue;
            }
            0x2a098 => {
                ecx = esp.wrapping_add(0x10);
                push!(ecx);
                ecx = ebp;
                crate::audio_lane::set_return(0x2a0a4);
                call!(0x29bb0);
                pc = 0x2a0a4; continue;
            }
            0x2a0a4 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2a187; continue; }
                pc = 0x2a0ac; continue;
            }
            0x2a0ac => {
                eax = m.read32(ebp.wrapping_add(0x4));
                edx = (edx & 0xfffff800u32);
                ecx = esp.wrapping_add(0x14);
                push!(ecx);
                ebx = edx;
                edx = m.read32(eax);
                push!(eax);
                crate::audio_lane::set_return(0x2a0c2);
                call_to!(m.read32(edx.wrapping_add(0x8)));
                pc = 0x2a0c2; continue;
            }
            0x2a0c2 => {
                esi = m.read32(esp.wrapping_add(0x10));
                edx = 0;
                eax = ebp.wrapping_add(0x78);
                ecx = eax;
                m.write32(ecx, edx);
                m.write32(ecx.wrapping_add(0x4), edx);
                m.write32(ecx.wrapping_add(0x8), edx);
                m.write32(ecx.wrapping_add(0xc), edx);
                m.write32(ecx.wrapping_add(0x10), edx);
                m.write32(ecx.wrapping_add(0x14), edx);
                ecx = m.read32(ebp.wrapping_add(0x8));
                edx = esi.wrapping_add(esi.wrapping_mul(0x8));
                edx = edx.wrapping_shl((((((((0xdi32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_add(ecx);
                m.write32(eax, edx);
                eax = (eax & 0xffff_ff00) | ((((m.read8(ebp.wrapping_add(0x1))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x2a0fb; continue; }
                pc = 0x2a0f2; continue;
            }
            0x2a0f2 => {
                m.write32(ebp.wrapping_add(0x7c), 0x12000);
                pc = 0x2a12d; continue;
            }
            0x2a0fb => {
                edx = m.read32(ebp.wrapping_add(0x40));
                ecx = m.read32(ebp.wrapping_add(0x3c));
                edi = m.read32(esp.wrapping_add(0x18));
                ecx = ecx.wrapping_sub(edx);
                edx = 0;
                eax = ecx;
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (edi as u64)) as u32;
                edx = (dividend % (edi as u64)) as u32; }
                m.write32(ebp.wrapping_add(0x7c), ecx);
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2a11b; continue; }
                pc = 0x2a114; continue;
            }
            0x2a114 => {
                edi = edi.wrapping_sub(edx);
                edi = edi.wrapping_add(ecx);
                m.write32(ebp.wrapping_add(0x7c), edi);
                pc = 0x2a11b; continue;
            }
            0x2a11b => {
                eax = m.read32(ebp.wrapping_add(0x7c));
                fa = eax;
                fb = ((((0x12000i32)) as u32) & 0xffffffffu32);
                if (fa <= fb) { pc = 0x2a12a; continue; }
                pc = 0x2a125; continue;
            }
            0x2a125 => {
                eax = 0x12000;
                pc = 0x2a12a; continue;
            }
            0x2a12a => {
                m.write32(ebp.wrapping_add(0x7c), eax);
                pc = 0x2a12d; continue;
            }
            0x2a12d => {
                eax = ebp.wrapping_add(esi.wrapping_mul(0x4)).wrapping_add(0xc);
                m.write32(ebp.wrapping_add(0x84), eax);
                m.write32(eax, 0x8000000au32);
                eax = 0;
                ecx = 0x5000;
                edi = ebp.wrapping_add(0x94);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                eax = m.read32(ebp.wrapping_add(0x40));
                eax = eax.wrapping_sub(ebx);
                ecx = ebp.wrapping_add(0x34);
                push!(ecx);
                m.write32(ebp.wrapping_add(0x90), eax);
                eax = ebp.wrapping_add(0x30);
                push!(eax);
                m.write32(ecx, 0);
                push!(esi);
                ecx = ebp;
                m.write32(eax, 0x12000);
                m.write32(ebp.wrapping_add(0x44), esi);
                m.write32(ebp.wrapping_add(0x20), 0);
                crate::audio_lane::set_return(0x2a17d);
                call!(0x29be0);
                pc = 0x2a17d; continue;
            }
            0x2a17d => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x2a187 => {
                fa = edx;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2a1f3; continue; }
                pc = 0x2a18b; continue;
            }
            0x2a18b => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(ebp.wrapping_add(0x1))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x2a1ec; continue; }
                pc = 0x2a192; continue;
            }
            0x2a192 => {
                eax = m.read32(ebp.wrapping_add(0x38));
                esi = (esi | 0xffffffffu32);
                fa = eax;
                fb = esi;
                if (fa == fb) { pc = 0x2a1b9; continue; }
                pc = 0x2a19c; continue;
            }
            0x2a19c => {
                eax = m.read32(ebp.wrapping_add(0x4));
                ecx = m.read32(eax);
                push!(eax);
                crate::audio_lane::set_return(0x2a1a5);
                call_to!(m.read32(ecx.wrapping_add(0x14)));
                pc = 0x2a1a5; continue;
            }
            0x2a1a5 => {
                fa = m.read32(ebp.wrapping_add(0x48));
                fb = edi;
                if (fa == fb) { pc = 0x2a1b6; continue; }
                pc = 0x2a1aa; continue;
            }
            0x2a1aa => {
                edx = m.read32(ebp.wrapping_add(0x38));
                push!(edx);
                crate::audio_lane::set_return(0x2a1b3);
                call!(0x29400);
                pc = 0x2a1b3; continue;
            }
            0x2a1b3 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x2a1b6; continue;
            }
            0x2a1b6 => {
                m.write32(ebp.wrapping_add(0x38), esi);
                pc = 0x2a1b9; continue;
            }
            0x2a1b9 => {
                eax = m.read32(ebp.wrapping_add(0x4));
                ecx = m.read32(eax);
                edx = esp.wrapping_add(0x10);
                push!(edx);
                push!(eax);
                crate::audio_lane::set_return(0x2a1c7);
                call_to!(m.read32(ecx.wrapping_add(0xc)));
                pc = 0x2a1c7; continue;
            }
            0x2a1c7 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2a1f3; continue; }
                pc = 0x2a1cb; continue;
            }
            0x2a1cb => {
                fa = m.read32(esp.wrapping_add(0x10));
                fb = ((((0x30000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x2a1f3; continue; }
                pc = 0x2a1d5; continue;
            }
            0x2a1d5 => {
                m.write32(ebp.wrapping_add(0x2c), edi);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(ebp.wrapping_add(0x4c), esi);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(ebp.wrapping_add(0x28), 0x3);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x2a1ec => {
                fa = edx;
                fb = esi;
                if (fa != fb) { pc = 0x2a1f3; continue; }
                pc = 0x2a1f0; continue;
            }
            0x2a1f0 => {
                m.write32(ebp.wrapping_add(0x40), edi);
                pc = 0x2a1f3; continue;
            }
            0x2a1f3 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00044850: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f44850<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x44850;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x44850, previous, pc); previous = pc; }
        match pc {
            0x44850 => {
                edx = m.read32(0x6b990c);
                push!(esi);
                eax = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(edi);
                if (fas <= fbs) { pc = 0x4487c; continue; }
                pc = 0x4485e; continue;
            }
            0x4485e => {
                edi = m.read32(0x6b9908);
                esi = m.read32(esp.wrapping_add(0xc));
                ecx = edi;
                pc = 0x44870; continue;
            }
            0x44870 => {
                fa = m.read32(ecx);
                fb = esi;
                if (fa == fb) { pc = 0x44881; continue; }
                pc = 0x44874; continue;
            }
            0x44874 => {
                eax = eax.wrapping_add(1);
                ecx = ecx.wrapping_add(0x64);
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x44870; continue; }
                pc = 0x4487c; continue;
            }
            0x4487c => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                esp = esp.wrapping_add(0); break 'run;
            }
            0x44881 => {
                eax = ((eax as i32).wrapping_mul((0x64) as i32)) as u32;
                eax = eax.wrapping_add(edi);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00044AC0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f44ac0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm5, mut xmm6, mut xmm7) = (xh[0], xh[1], xh[2]);
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut fp_cmp = 0i32;
    let mut pc: u32 = 0x44ac0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: [xmm5, xmm6, xmm7] };
        guest.call(target, &mut cr);
        [xmm5, xmm6, xmm7] = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x44ac0, previous, pc); previous = pc; }
        match pc {
            0x44ac0 => {
                esp = esp.wrapping_sub(0x6c);
                push!(edi);
                edi = m.read32(0x6b9914);
                crate::audio_lane::set_return(0x44acf);
                call!(0x153af0);
                pc = 0x44acf; continue;
            }
            0x44acf => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45084; continue; }
                pc = 0x44ad7; continue;
            }
            0x44ad7 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(ebx);
                push!(ebp);
                push!(esi);
                if (fas <= fbs) { pc = 0x44c7a; continue; }
                pc = 0x44ae2; continue;
            }
            0x44ae2 => {
                eax = edi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                pc = 0x44af0; continue;
            }
            0x44af0 => {
                ecx = m.read32(0x6b98b8);
                eax = eax.wrapping_sub(0x2c);
                esi = eax.wrapping_add(ecx);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x28))) as u32)) & 0xff);
                edi = edi.wrapping_sub(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x18i32)) as u32) & 0xffu32);
                m.write32(esp.wrapping_add(0x10), edi);
                m.write32(esp.wrapping_add(0x20), eax);
                if ((fa & fb) != 0) { pc = 0x44c72; continue; }
                pc = 0x44b11; continue;
            }
            0x44b11 => {
                ebp = m.read32(0x790950);
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x44c72; continue; }
                pc = 0x44b1f; continue;
            }
            0x44b1f => {
                ebx = ebp;
                ebx = ((ebx as i32).wrapping_mul((0x1584) as i32)) as u32;
                pc = 0x44b30; continue;
            }
            0x44b30 => {
                edx = m.read32(0x7b0c48);
                ecx = (((((m.read16(esi.wrapping_add(0x4)) as i16)) as i32)) as u32);
                ebx = ebx.wrapping_sub(0x1584);
                eax = m.read32(ebx.wrapping_add(edx).wrapping_add(0x14));
                eax = m.read32(eax.wrapping_add(0x34));
                edi = ebx.wrapping_add(edx);
                ebp = ebp.wrapping_sub(1);
                fa = eax;
                fb = ecx;
                if (fa == fb) { pc = 0x44b77; continue; }
                pc = 0x44b4f; continue;
            }
            0x44b4f => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x44c62; continue; }
                pc = 0x44b57; continue;
            }
            0x44b57 => {
                fa = eax;
                fb = m.read32(0x7b975c);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x44c62; continue; }
                pc = 0x44b63; continue;
            }
            0x44b63 => {
                push!(0xa);
                edx = esp.wrapping_add(0x30);
                push!(edx);
                push!(eax);
                crate::audio_lane::set_return(0x44b70);
                call!(0x87720);
                pc = 0x44b70; continue;
            }
            0x44b70 => {
                esp = esp.wrapping_add(0xc);
                m.write32(esp.wrapping_add(0x24), eax);
                pc = 0x44b77; continue;
            }
            0x44b77 => {
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x10)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0xbc)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0xb4)))).to_bits();
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x1c)));
                xmm2 = Xmm::scalar(m.read32(edi.wrapping_add(0xb8)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0xc)))).to_bits();
                xmm4 = xmm1;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = xmm3;
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm4.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44c62; continue; }
                pc = 0x44bc5; continue;
            }
            0x44bc5 => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x18)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44c62; continue; }
                pc = 0x44bd3; continue;
            }
            0x44bd3 => {
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi.wrapping_add(0x14))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44c62; continue; }
                pc = 0x44bdd; continue;
            }
            0x44bdd => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x20)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d0c))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475ce0))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(0xb4)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0xbc)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x10)))).to_bits();
                ecx = eax.wrapping_add(0x200);
                ecx = (ecx & 0x7ff);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x7a38a0)))).to_bits();
                eax = (eax & 0x7ff);
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_mul(0x4).wrapping_add(0x7a38a0)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                push!(ebp);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44c55; continue; }
                pc = 0x44c49; continue;
            }
            0x44c49 => {
                edx = m.read32(esp.wrapping_add(0x14));
                push!(edx);
                crate::audio_lane::set_return(0x44c53);
                call!(0x44980);
                pc = 0x44c53; continue;
            }
            0x44c53 => {
                pc = 0x44c5f; continue;
            }
            0x44c55 => {
                eax = m.read32(esp.wrapping_add(0x14));
                push!(eax);
                crate::audio_lane::set_return(0x44c5f);
                call!(0x44a20);
                pc = 0x44c5f; continue;
            }
            0x44c5f => {
                esp = esp.wrapping_add(0x8);
                pc = 0x44c62; continue;
            }
            0x44c62 => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x44b30; continue; }
                pc = 0x44c6a; continue;
            }
            0x44c6a => {
                edi = m.read32(esp.wrapping_add(0x10));
                eax = m.read32(esp.wrapping_add(0x20));
                pc = 0x44c72; continue;
            }
            0x44c72 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x44af0; continue; }
                pc = 0x44c7a; continue;
            }
            0x44c7a => {
                eax = m.read32(0x6b990c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                edi = eax;
                if (fas <= fbs) { pc = 0x45081; continue; }
                pc = 0x44c89; continue;
            }
            0x44c89 => {
                eax = ((eax as i32).wrapping_mul((0x64) as i32)) as u32;
                pc = 0x44c92; continue;
            }
            0x44c8e => {
                eax = m.read32(esp.wrapping_add(0x24));
                pc = 0x44c92; continue;
            }
            0x44c92 => {
                ecx = m.read32(0x6b9908);
                xmm1 = Xmm::scalar(m.read32(0x475d5c));
                xmm6 = Xmm::default();
                eax = eax.wrapping_sub(0x64);
                esi = eax.wrapping_add(ecx);
                m.write32(esp.wrapping_add(0x24), eax);
                eax = m.read32(esi.wrapping_add(0x60));
                edi = edi.wrapping_sub(1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x18i32)) as u32) & 0xffu32);
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x20), xmm6.lanes[0]);
                m.write32(esp.wrapping_add(0x10), edi);
                if ((fa & fb) != 0) { pc = 0x45075; continue; }
                pc = 0x44cc9; continue;
            }
            0x44cc9 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x45075; continue; }
                pc = 0x44cd1; continue;
            }
            0x44cd1 => {
                ebp = eax;
                eax = (eax & 0xfffffffeu32);
                ebp = (ebp & 0x1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                m.write32(esi.wrapping_add(0x60), eax);
                if ((fa & fb) == 0) { pc = 0x44d2f; continue; }
                pc = 0x44ce0; continue;
            }
            0x44ce0 => {
                eax = m.read32(0x6b991c);
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x4505a; continue; }
                pc = 0x44cef; continue;
            }
            0x44cef => {
                edx = m.read32(0x6b98bc);
                pc = 0x44d00; continue;
            }
            0x44cf7 => {
                edi = m.read32(esp.wrapping_add(0x10));
                pc = 0x44d00; continue;
            }
            0x44d00 => {
                ebx = (((((m.read16(edx.wrapping_add(eax.wrapping_mul(0x8)).wrapping_add(((0x6i32.wrapping_neg()) as u32))) as i16)) as i32)) as u32);
                eax = eax.wrapping_sub(1);
                fa = ebx;
                fb = edi;
                if (fa != fb) { pc = 0x44d23; continue; }
                pc = 0x44d0a; continue;
            }
            0x44d0a => {
                edi = (((((m.read16(edx.wrapping_add(eax.wrapping_mul(0x8))) as i16)) as i32)) as u32);
                ebx = m.read32(0x6b98b8);
                edi = ((edi as i32).wrapping_mul((0x2c) as i32)) as u32;
                fa = m.read32(edi.wrapping_add(ebx).wrapping_add(0x24));
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa <= fb) { pc = 0x44d23; continue; }
                pc = 0x44d1e; continue;
            }
            0x44d1e => {
                ecx = 0x1;
                pc = 0x44d23; continue;
            }
            0x44d23 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x44cf7; continue; }
                pc = 0x44d27; continue;
            }
            0x44d27 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x4505a; continue; }
                pc = 0x44d2f; continue;
            }
            0x44d2f => {
                eax = m.read32(0x790950);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x44ffd; continue; }
                pc = 0x44d3c; continue;
            }
            0x44d3c => {
                ebx = eax;
                ebx = ((ebx as i32).wrapping_mul((0x1584) as i32)) as u32;
                m.write32(esp.wrapping_add(0x1c), eax);
                pc = 0x44d50; continue;
            }
            0x44d50 => {
                edx = m.read32(0x7b0c48);
                ecx = (((((m.read16(esi.wrapping_add(0x8)) as i16)) as i32)) as u32);
                ebx = ebx.wrapping_sub(0x1584);
                eax = m.read32(ebx.wrapping_add(edx).wrapping_add(0x14));
                eax = m.read32(eax.wrapping_add(0x34));
                fa = eax;
                fb = ecx;
                edi = ebx.wrapping_add(edx);
                if (fa == fb) { pc = 0x44d99; continue; }
                pc = 0x44d6e; continue;
            }
            0x44d6e => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x44fed; continue; }
                pc = 0x44d76; continue;
            }
            0x44d76 => {
                fa = eax;
                fb = m.read32(0x7b975c);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x44fed; continue; }
                pc = 0x44d82; continue;
            }
            0x44d82 => {
                push!(0xa);
                edx = esp.wrapping_add(0x58);
                push!(edx);
                push!(eax);
                crate::audio_lane::set_return(0x44d8f);
                call!(0x87720);
                pc = 0x44d8f; continue;
            }
            0x44d8f => {
                xmm6 = Xmm::default();
                esp = esp.wrapping_add(0xc);
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x44d99; continue;
            }
            0x44d99 => {
                ecx = m.read32(esi.wrapping_add(0x60));
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x44f65; continue; }
                pc = 0x44da5; continue;
            }
            0x44da5 => {
                xmm0 = Xmm::scalar(m.read32(0x475c88));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x44)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d0c))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475ce0))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(0xb4)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x1c)))).to_bits();
                xmm3 = Xmm::scalar(m.read32(edi.wrapping_add(0xb8)));
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x20)))).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0xbc)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x24)))).to_bits();
                edx = eax.wrapping_add(0x200);
                edx = (edx & 0x7ff);
                xmm4 = Xmm::scalar(m.read32(edx.wrapping_mul(0x4).wrapping_add(0x7a38a0)));
                eax = (eax & 0x7ff);
                xmm5 = Xmm::scalar(m.read32(eax.wrapping_mul(0x4).wrapping_add(0x7a38a0)));
                xmm2 = xmm0;
                xmm7 = xmm1;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm7.lanes[0] = sse::mulss(f32::from_bits(xmm7.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(esi.wrapping_add(0x28)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm5.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x48)));
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm5.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4c)))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm7.lanes[0])).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm4.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44fed; continue; }
                pc = 0x44e52; continue;
            }
            0x44e52 => {
                xmm4 = Xmm::scalar(m.read32(esi.wrapping_add(0x50)));
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x2c)))).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44fed; continue; }
                pc = 0x44e69; continue;
            }
            0x44e69 => {
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi.wrapping_add(0x38))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44fed; continue; }
                pc = 0x44e73; continue;
            }
            0x44e73 => {
                xmm4 = Xmm::scalar(m.read32(esi.wrapping_add(0x3c)));
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44fed; continue; }
                pc = 0x44e81; continue;
            }
            0x44e81 => {
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x30)));
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x54)))).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44fed; continue; }
                pc = 0x44e98; continue;
            }
            0x44e98 => {
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x58)));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x34)))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44fed; continue; }
                pc = 0x44eaf; continue;
            }
            0x44eaf => {
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x58)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x50)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x48)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x54)))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4c)))).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475d1c)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44fe7; continue; }
                pc = 0x44ed5; continue;
            }
            0x44ed5 => {
                xmm3 = Xmm::scalar(m.read32(esi.wrapping_add(0x28)));
                eax = 0;
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                xmm1 = xmm6;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44ef2; continue; }
                pc = 0x44ee4; continue;
            }
            0x44ee4 => {
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm6.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44f0a; continue; }
                pc = 0x44eed; continue;
            }
            0x44eed => {
                xmm1 = xmm3;
                pc = 0x44f0a; continue;
            }
            0x44ef2 => {
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi.wrapping_add(0x2c))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44f0a; continue; }
                pc = 0x44ef8; continue;
            }
            0x44ef8 => {
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x2c)))).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm6.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44f0a; continue; }
                pc = 0x44f02; continue;
            }
            0x44f02 => {
                xmm1 = xmm2;
                eax = 0x1;
                pc = 0x44f0a; continue;
            }
            0x44f0a => {
                xmm2 = Xmm::scalar(m.read32(esi.wrapping_add(0x30)));
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44f27; continue; }
                pc = 0x44f14; continue;
            }
            0x44f14 => {
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44f3f; continue; }
                pc = 0x44f1d; continue;
            }
            0x44f1d => {
                xmm1 = xmm2;
                eax = 0x2;
                pc = 0x44f3f; continue;
            }
            0x44f27 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi.wrapping_add(0x34))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44f3f; continue; }
                pc = 0x44f2d; continue;
            }
            0x44f2d => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x34)))).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44f3f; continue; }
                pc = 0x44f37; continue;
            }
            0x44f37 => {
                xmm1 = xmm0;
                eax = 0x3;
                pc = 0x44f3f; continue;
            }
            0x44f3f => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44fe7; continue; }
                pc = 0x44f4e; continue;
            }
            0x44f4e => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x4c)));
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                pc = 0x44fe7; continue;
            }
            0x44f65 => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x1c)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0xb4)))).to_bits();
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x24)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0xbc)))).to_bits();
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x48)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x40)))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(edi.wrapping_add(0xb8)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x20)))).to_bits();
                xmm3 = xmm0;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x18), xmm1.lanes[0]);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44fed; continue; }
                pc = 0x44fb7; continue;
            }
            0x44fb7 => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x3c)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44fed; continue; }
                pc = 0x44fc1; continue;
            }
            0x44fc1 => {
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi.wrapping_add(0x38))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x44fed; continue; }
                pc = 0x44fc7; continue;
            }
            0x44fc7 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x18))) as f64));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x18), (fpu.st(0) as f32).to_bits());
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x14))) as f64));
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if fp_cmp != 1 { pc = 0x00044fe7; continue; }
                pc = 0x44fdb; continue;
            }
            0x44fdb => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                pc = 0x44fe7; continue;
            }
            0x44fe7 => {
                ecx = (ecx | 0x1);
                m.write32(esi.wrapping_add(0x60), ecx);
                pc = 0x44fed; continue;
            }
            0x44fed => {
                m.write32(esp.wrapping_add(0x1c), m.read32(esp.wrapping_add(0x1c)).wrapping_sub(0x1));
                fa = m.read32(esp.wrapping_add(0x1c));
                if ((fa != 0)) { pc = 0x44d50; continue; }
                pc = 0x44ff7; continue;
            }
            0x44ff7 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                pc = 0x44ffd; continue;
            }
            0x44ffd => {
                eax = m.read32(esi.wrapping_add(0x60));
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x4505a; continue; }
                pc = 0x45004; continue;
            }
            0x45004 => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x48)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x20)))).to_bits();
                xmm2 = Xmm::scalar(m.read32(0x475d1c));
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x4502a; continue; }
                pc = 0x4501c; continue;
            }
            0x4501c => {
                push!(0x3f800000);
                eax = esi;
                crate::audio_lane::set_return(0x45028);
                call!(0x44890);
                pc = 0x45028; continue;
            }
            0x45028 => {
                pc = 0x45072; continue;
            }
            0x4502a => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x4503b; continue; }
                pc = 0x4502e; continue;
            }
            0x4502e => {
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x40)))).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm6.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x4503b; continue; }
                pc = 0x45038; continue;
            }
            0x45038 => {
                xmm1 = xmm6;
                pc = 0x4503b; continue;
            }
            0x4503b => {
                xmm1.lanes[0] = sse::divss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                push!(ecx);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                eax = esi;
                m.write32(esp, xmm0.lanes[0]);
                crate::audio_lane::set_return(0x45058);
                call!(0x44890);
                pc = 0x45058; continue;
            }
            0x45058 => {
                pc = 0x45072; continue;
            }
            0x4505a => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x45075; continue; }
                pc = 0x4505e; continue;
            }
            0x4505e => {
                eax = m.read32(esi.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x45075; continue; }
                pc = 0x45065; continue;
            }
            0x45065 => {
                push!(eax);
                crate::audio_lane::set_return(0x4506b);
                call!(0x45460);
                pc = 0x4506b; continue;
            }
            0x4506b => {
                m.write32(esi.wrapping_add(0x4), 0xffffffffu32);
                pc = 0x45072; continue;
            }
            0x45072 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x45075; continue;
            }
            0x45075 => {
                edi = m.read32(esp.wrapping_add(0x10));
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x44c8e; continue; }
                pc = 0x45081; continue;
            }
            0x45081 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x45084; continue;
            }
            0x45084 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x6c);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: [xmm5, xmm6, xmm7], ..entry };
}

/// 0x000454C0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f454c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x454c0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x454c0, previous, pc); previous = pc; }
        match pc {
            0x454c0 => {
                tail_to!(0x28110); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045540: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45540<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45540;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45540, previous, pc); previous = pc; }
        match pc {
            0x45540 => {
                edx = m.read32(esp.wrapping_add(0x4));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x4554b; continue; }
                pc = 0x45548; continue;
            }
            0x45548 => {
                eax = 0;
                esp = esp.wrapping_add(0); break 'run;
            }
            0x4554b => {
                fa = edx;
                fb = ((((0x20000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x45573; continue; }
                pc = 0x45553; continue;
            }
            0x45553 => {
                eax = 0x6d58a4;
                pc = 0x45558; continue;
            }
            0x45558 => {
                fa = ((m.read32(eax.wrapping_add(((0xci32.wrapping_neg()) as u32)))) & 0xffffffffu32);
                fb = edx;
                if (fa != fb) { pc = 0x45569; continue; }
                pc = 0x4555d; continue;
            }
            0x4555d => {
                ecx = m.read32(eax);
                fa = ecx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x4557c; continue; }
                pc = 0x45564; continue;
            }
            0x45564 => {
                fa = ecx;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x4557c; continue; }
                pc = 0x45569; continue;
            }
            0x45569 => {
                eax = eax.wrapping_add(0x40);
                fa = eax;
                fb = ((((0x6d5ee4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x45558; continue; }
                pc = 0x45573; continue;
            }
            0x45573 => {
                m.write32(esp.wrapping_add(0x4), edx);
                tail_to!(0x27bb0); break 'run;  // direct tail jump
            }
            0x4557c => {
                eax = 0x1;
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000455B0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f455b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x455b0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x455b0, previous, pc); previous = pc; }
        match pc {
            0x455b0 => {
                tail_to!(0x270d0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000455C0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f455c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x455c0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x455c0, previous, pc); previous = pc; }
        match pc {
            0x455c0 => {
                tail_to!(0x271a0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045600: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45600<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45600;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45600, previous, pc); previous = pc; }
        match pc {
            0x45600 => {
                tail_to!(0x27cc0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045610: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45610<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45610;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45610, previous, pc); previous = pc; }
        match pc {
            0x45610 => {
                tail_to!(0x27c20); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045A00: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45a00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45a00;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45a00, previous, pc); previous = pc; }
        match pc {
            0x45a00 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x45a30; continue; }
                pc = 0x45a0a; continue;
            }
            0x45a0a => {
                push!(esi);
                crate::audio_lane::set_return(0x45a10);
                call!(0x45d60);
                pc = 0x45a10; continue;
            }
            0x45a10 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45a30; continue; }
                pc = 0x45a17; continue;
            }
            0x45a17 => {
                eax = m.read32(esp.wrapping_add(0xc));
                push!(eax);
                push!(esi);
                crate::audio_lane::set_return(0x45a22);
                call!(0x289c0);
                pc = 0x45a22; continue;
            }
            0x45a22 => {
                esp = esp.wrapping_add(0x8);
                push!(esi);
                crate::audio_lane::set_return(0x45a2b);
                call!(0x46280);
                pc = 0x45a2b; continue;
            }
            0x45a2b => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x45a30 => {
                crate::audio_lane::set_return(0x45a35);
                call!(0x28a30);
                pc = 0x45a35; continue;
            }
            0x45a35 => {
                push!(esi);
                crate::audio_lane::set_return(0x45a3b);
                call!(0x46280);
                pc = 0x45a3b; continue;
            }
            0x45a3b => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045C60: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45c60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45c60;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45c60, previous, pc); previous = pc; }
        match pc {
            0x45c60 => {
                ecx = m.read32(0x79094c);
                eax = m.read32(0x5236bc);
                push!(ecx);
                m.write32(0x6d5eb4, eax);
                crate::audio_lane::set_return(0x45c76);
                call!(0x1ceb00);
                pc = 0x45c76; continue;
            }
            0x45c76 => {
                ecx = m.read32(0x5236b8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45c89; continue; }
                pc = 0x45c83; continue;
            }
            0x45c83 => {
                ecx = m.read32(0x5236b4);
                pc = 0x45c89; continue;
            }
            0x45c89 => {
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45ca1; continue; }
                pc = 0x45c94; continue;
            }
            0x45c94 => {
                push!(0x42c80000);
                crate::audio_lane::set_return(0x45c9e);
                call!(0x28a80);
                pc = 0x45c9e; continue;
            }
            0x45c9e => {
                esp = esp.wrapping_add(0x4);
                pc = 0x45ca1; continue;
            }
            0x45ca1 => {
                edx = m.read32(0x79094c);
                push!(edx);
                crate::audio_lane::set_return(0x45cad);
                call!(0x1ceb00);
                pc = 0x45cad; continue;
            }
            0x45cad => {
                ecx = m.read32(0x5236b8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45cc0; continue; }
                pc = 0x45cba; continue;
            }
            0x45cba => {
                ecx = m.read32(0x5236b4);
                pc = 0x45cc0; continue;
            }
            0x45cc0 => {
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x45cdf; continue; }
                pc = 0x45ccb; continue;
            }
            0x45ccb => {
                push!(0);
                crate::audio_lane::set_return(0x45cd2);
                call!(0x28a80);
                pc = 0x45cd2; continue;
            }
            0x45cd2 => {
                esp = esp.wrapping_add(0x4);
                m.write32(0x6d5eb4, 0);
                pc = 0x45cdf; continue;
            }
            0x45cdf => {
                fa = m.read32(0x79094c);
                fb = ((((0x65i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45cee; continue; }
                pc = 0x45ce8; continue;
            }
            0x45ce8 => {
                m.write32(0x6d5eb4, (((((((m.read32(0x6d5eb4))) as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32));
                pc = 0x45cee; continue;
            }
            0x45cee => {
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045FD0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45fd0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45fd0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45fd0, previous, pc); previous = pc; }
        match pc {
            0x45fd0 => {
                fa = m.read32(0x7de458);
                fb = ((((0x20000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x46027; continue; }
                pc = 0x45fdc; continue;
            }
            0x45fdc => {
                eax = m.read32(0x6b7aac);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x45ff6; continue; }
                pc = 0x45fe5; continue;
            }
            0x45fe5 => {
                push!(0x1);
                crate::audio_lane::set_return(0x45fec);
                call!(0x462d0);
                pc = 0x45fec; continue;
            }
            0x45fec => {
                push!(eax);
                crate::audio_lane::set_return(0x45ff2);
                call!(0x45a00);
                pc = 0x45ff2; continue;
            }
            0x45ff2 => {
                esp = esp.wrapping_add(0x8);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x45ff6 => {
                eax = m.read32(0x79094c);
                push!(eax);
                crate::audio_lane::set_return(0x46001);
                call!(0x6a850);
                pc = 0x46001; continue;
            }
            0x46001 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x4601a; continue; }
                pc = 0x46008; continue;
            }
            0x46008 => {
                eax = m.read32(eax.wrapping_add(0x120));
                push!(0x1);
                push!(eax);
                crate::audio_lane::set_return(0x46016);
                call!(0x45a00);
                pc = 0x46016; continue;
            }
            0x46016 => {
                esp = esp.wrapping_add(0x8);
                esp = esp.wrapping_add(0); break 'run;
            }
            0x4601a => {
                eax = 0;
                push!(0x1);
                push!(eax);
                crate::audio_lane::set_return(0x46024);
                call!(0x45a00);
                pc = 0x46024; continue;
            }
            0x46024 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x46027; continue;
            }
            0x46027 => {
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000462B0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f462b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x462b0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x462b0, previous, pc); previous = pc; }
        match pc {
            0x462b0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                m.write32(0x4d1850, eax);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000462F0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f462f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
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
    let mut pc: u32 = 0x462f0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x462f0, previous, pc); previous = pc; }
        match pc {
            0x462f0 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x4))) as f64));
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x4), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(0x475c78)) as f64));
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x4))) as f64));
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if fp_cmp == 1 { pc = 0x0004632a; continue; }
                pc = 0x46312; continue;
            }
            0x46312 => {
                xmm2 = Xmm::default();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x46325; continue; }
                pc = 0x46320; continue;
            }
            0x46320 => {
                xmm0 = xmm2;
                pc = 0x4632d; continue;
            }
            0x46325 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x4632d; continue; }
                pc = 0x4632a; continue;
            }
            0x4632a => {
                xmm0 = xmm1;
                pc = 0x4632d; continue;
            }
            0x4632d => {
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d04))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(0x4d1840, xmm1.lanes[0]);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00046AB0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f46ab0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x46ab0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x46ab0, previous, pc); previous = pc; }
        match pc {
            0x46ab0 => {
                eax = m.read32(esp.wrapping_add(0x8));
                esp = esp.wrapping_sub(0x34);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x48));
                push!(eax);
                push!(edi);
                crate::audio_lane::set_return(0x46ac3);
                call!(0x45310);
                pc = 0x46ac3; continue;
            }
            0x46ac3 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x46b7a; continue; }
                pc = 0x46ace; continue;
            }
            0x46ace => {
                push!(esi);
                crate::audio_lane::set_return(0x46ad4);
                call!(0x467a0);
                pc = 0x46ad4; continue;
            }
            0x46ad4 => {
                esi = eax;
                crate::audio_lane::set_return(0x46adb);
                call!(0x153b40);
                pc = 0x46adb; continue;
            }
            0x46adb => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x8), eax);
                fpu.push((m.read32(esp.wrapping_add(0x8)) as i32 as f64));
                if (fas >= fbs) { pc = 0x46aed; continue; }
                pc = 0x46ae7; continue;
            }
            0x46ae7 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x46aed; continue;
            }
            0x46aed => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                ecx = esp.wrapping_add(0xc);
                push!(ecx);
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785e4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x476040)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0xc), (fpu.st(0) as f32).to_bits()); fpu.pop();
                crate::audio_lane::set_return(0x46b0d);
                call!(0x45390);
                pc = 0x46b0d; continue;
            }
            0x46b0d => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                edx = m.read32(esp.wrapping_add(0x44));
                eax = m.read32(esp.wrapping_add(0xc));
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi));
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x8)));
                ecx = esp.wrapping_add(0x10);
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4c)));
                push!(ecx);
                m.write8(esp.wrapping_add(0x3c), (0x1) as u8);
                m.write32(esp.wrapping_add(0x14), edx);
                m.write32(esp.wrapping_add(0x28), eax);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x46b64);
                call!(0x45820);
                pc = 0x46b64; continue;
            }
            0x46b64 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                esp = esp.wrapping_add(0x8);
                m.write32(esi, eax);
                m.write32(esi.wrapping_add(0x8), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x4), xmm0.lanes[0]);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x46b7a; continue;
            }
            0x46b7a => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x34);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00046CB0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f46cb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x46cb0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x46cb0, previous, pc); previous = pc; }
        match pc {
            0x46cb0 => {
                esp = esp.wrapping_sub(0xc);
                ecx = 0;
                eax = 0x6d6028;
                push!(edi);
                pc = 0x46cc0; continue;
            }
            0x46cc0 => {
                fa = m.read32(eax);
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x46d3b; continue; }
                pc = 0x46cc5; continue;
            }
            0x46cc5 => {
                eax = eax.wrapping_add(0xc);
                ecx = ecx.wrapping_add(1);
                fa = eax;
                fb = ((((0x6d6058i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x46cc0; continue; }
                pc = 0x46cd0; continue;
            }
            0x46cd0 => {
                edi = 0;
                pc = 0x46cd2; continue;
            }
            0x46cd2 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = 0x20;
                if (fa == fb) { pc = 0x46d74; continue; }
                pc = 0x46cdf; continue;
            }
            0x46cdf => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x18));
                fa = ebx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x46d73; continue; }
                pc = 0x46ced; continue;
            }
            0x46ced => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x24));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x46cfb; continue; }
                pc = 0x46cf6; continue;
            }
            0x46cf6 => {
                eax = 0x2a0;
                pc = 0x46cfb; continue;
            }
            0x46cfb => {
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x24));
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(eax);
                if (fa == fb) { pc = 0x46d47; continue; }
                pc = 0x46d05; continue;
            }
            0x46d05 => {
                xmm0 = Xmm::default();
                eax = esp.wrapping_add(0x14);
                push!(eax);
                push!(0);
                push!(0x42b40000);
                push!(0x3f800000);
                push!(0x44834000);
                push!(ebx);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x46d36);
                call!(0x21ed0);
                pc = 0x46d36; continue;
            }
            0x46d36 => {
                esp = esp.wrapping_add(0x1c);
                pc = 0x46d54; continue;
            }
            0x46d3b => {
                edi = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                edi = edi.wrapping_mul(0x4).wrapping_add(0x6d6028);
                pc = 0x46cd2; continue;
            }
            0x46d47 => {
                push!(0);
                push!(0);
                push!(ebx);
                crate::audio_lane::set_return(0x46d51);
                call!(0x21f60);
                pc = 0x46d51; continue;
            }
            0x46d51 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x46d54; continue;
            }
            0x46d54 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x46d71; continue; }
                pc = 0x46d58; continue;
            }
            0x46d58 => {
                ecx = 0;
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                cf = (((((esi) != 0))) as i32);
                esi = (((((esi) as i32).wrapping_neg())) as u32);
                esi = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                esi = (esi & 0x2);
                m.write32(edi, ebx);
                m.write32(edi.wrapping_add(0x4), eax);
                ecx = (ecx | esi);
                write16(m, edi.wrapping_add(0x8), (((((ecx) as u16)) as u32)) as u16);
                pc = 0x46d71; continue;
            }
            0x46d71 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x46d73; continue;
            }
            0x46d73 => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x46d74; continue;
            }
            0x46d74 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00063C00: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f63c00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x63c00;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x63c00, previous, pc); previous = pc; }
        match pc {
            0x63c00 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(ebx);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x10));
                eax = eax.wrapping_add(0x8);
                pc = 0x63c10; continue;
            }
            0x63c10 => {
                edx = (edx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                ebx = (ebx & 0xffff_ff00) | ((((m.read8(esi)) as u32)) & 0xff);
                ecx = (ecx & 0xffff_ff00) | ((((((edx) as u8)) as u32)) & 0xff);
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((((ebx) as u8))) as u32) & 0xffu32);
                cf = ((((fa < fb))) as i32);
                if (fa != fb) { pc = 0x63c42; continue; }
                pc = 0x63c1a; continue;
            }
            0x63c1a => {
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x63c34; continue; }
                pc = 0x63c1e; continue;
            }
            0x63c1e => {
                edx = (edx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x1))) as u32)) & 0xff);
                ebx = (ebx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x1))) as u32)) & 0xff);
                ecx = (ecx & 0xffff_ff00) | ((((((edx) as u8)) as u32)) & 0xff);
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((((ebx) as u8))) as u32) & 0xffu32);
                cf = ((((fa < fb))) as i32);
                if (fa != fb) { pc = 0x63c42; continue; }
                pc = 0x63c2a; continue;
            }
            0x63c2a => {
                eax = eax.wrapping_add(0x2);
                esi = esi.wrapping_add(0x2);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x63c10; continue; }
                pc = 0x63c34; continue;
            }
            0x63c34 => {
                eax = 0;
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ecx;
                esp = esp.wrapping_add(0); break 'run;
            }
            0x63c42 => {
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                { let wide = (eax as u64).wrapping_sub(0xffff_ffff).wrapping_sub(cf as u64); cf = ((wide >> 32) & 1) as i32; eax = wide as u32; }
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ecx;
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0007D960: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f7d960<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x7d960;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x7d960, previous, pc); previous = pc; }
        match pc {
            0x7d960 => {
                eax = m.read32(0x79094c);
                esp = esp.wrapping_sub(0x34);
                fa = eax;
                fb = ((((0x66i32)) as u32) & 0xffffffffu32);
                push!(esi);
                if (fa != fb) { pc = 0x7d9b3; continue; }
                pc = 0x7d96e; continue;
            }
            0x7d96e => {
                crate::audio_lane::set_return(0x7d973);
                call!(0x153b40);
                pc = 0x7d973; continue;
            }
            0x7d973 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x4), eax);
                fpu.push((m.read32(esp.wrapping_add(0x4)) as i32 as f64));
                if (fas >= fbs) { pc = 0x7d985; continue; }
                pc = 0x7d97f; continue;
            }
            0x7d97f => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x7d985; continue;
            }
            0x7d985 => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785e4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x476040)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x4), (fpu.st(0) as f32).to_bits()); fpu.pop();
                crate::audio_lane::set_return(0x7d9a0);
                call!(0x153b40);
                pc = 0x7d9a0; continue;
            }
            0x7d9a0 => {
                edx = 0;
                ecx = 0x3;
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (ecx as u64)) as u32;
                edx = (dividend % (ecx as u64)) as u32; }
                esi = edx;
                esi = esi.wrapping_add(0x1641);
                pc = 0x7d9f6; continue;
            }
            0x7d9b3 => {
                crate::audio_lane::set_return(0x7d9b8);
                call!(0x153b40);
                pc = 0x7d9b8; continue;
            }
            0x7d9b8 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x4), eax);
                fpu.push((m.read32(esp.wrapping_add(0x4)) as i32 as f64));
                if (fas >= fbs) { pc = 0x7d9ca; continue; }
                pc = 0x7d9c4; continue;
            }
            0x7d9c4 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x7d9ca; continue;
            }
            0x7d9ca => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785e4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x476040)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x4), (fpu.st(0) as f32).to_bits()); fpu.pop();
                crate::audio_lane::set_return(0x7d9e5);
                call!(0x153b40);
                pc = 0x7d9e5; continue;
            }
            0x7d9e5 => {
                edx = 0;
                ecx = 0x3;
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (ecx as u64)) as u32;
                edx = (dividend % (ecx as u64)) as u32; }
                esi = edx;
                esi = esi.wrapping_add(0x163e);
                pc = 0x7d9f6; continue;
            }
            0x7d9f6 => {
                edx = esp.wrapping_add(0x8);
                push!(edx);
                crate::audio_lane::set_return(0x7da00);
                call!(0x45390);
                pc = 0x7da00; continue;
            }
            0x7da00 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = esp.wrapping_add(0xc);
                push!(ecx);
                m.write32(esp.wrapping_add(0x24), eax);
                m.write32(esp.wrapping_add(0x10), esi);
                crate::audio_lane::set_return(0x7da16);
                call!(0x45820);
                pc = 0x7da16; continue;
            }
            0x7da16 => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x34);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000BA700: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn fba700<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xba700;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0xba700, previous, pc); previous = pc; }
        match pc {
            0xba700 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(esp.wrapping_add(0x4));
                esp = esp.wrapping_sub(0x34);
                push!(ebx);
                push!(edi);
                push!(eax);
                push!(ecx);
                crate::audio_lane::set_return(0xba714);
                call!(0xd76a0);
                pc = 0xba714; continue;
            }
            0xba714 => {
                edi = eax;
                ebx = m.read32(edi.wrapping_add(0x38));
                esp = esp.wrapping_add(0x8);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xba7c3; continue; }
                pc = 0xba724; continue;
            }
            0xba724 => {
                xmm0 = Xmm::scalar(m.read32(0x7e71f0));
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d5d0))).to_bits();
                push!(esi);
                esi = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                crate::audio_lane::set_return(0xba74c);
                call!(0x153b40);
                pc = 0xba74c; continue;
            }
            0xba74c => {
                edx = 0;
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (ebx as u64)) as u32;
                edx = (dividend % (ebx as u64)) as u32; }
                edi = (((((m.read16(edi.wrapping_add(edx.wrapping_mul(0x2))) as i16)) as i32)) as u32);
                edx = esp.wrapping_add(0x10);
                push!(edx);
                crate::audio_lane::set_return(0xba75e);
                call!(0x45390);
                pc = 0xba75e; continue;
            }
            0xba75e => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                eax = m.read32(esp.wrapping_add(0x54));
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x47d208));
                m.write32(esp.wrapping_add(0x40), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                eax = esp.wrapping_add(0x14);
                push!(eax);
                m.write8(esp.wrapping_add(0x40), (0x1) as u8);
                m.write32(esp.wrapping_add(0x18), edi);
                m.write32(esp.wrapping_add(0x1c), esi);
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                crate::audio_lane::set_return(0xba7bf);
                call!(0x45820);
                pc = 0xba7bf; continue;
            }
            0xba7bf => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0xba7c3; continue;
            }
            0xba7c3 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x34);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000BA7D0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn fba7d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xba7d0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0xba7d0, previous, pc); previous = pc; }
        match pc {
            0xba7d0 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(esp.wrapping_add(0x4));
                esp = esp.wrapping_sub(0x34);
                push!(esi);
                push!(edi);
                push!(eax);
                push!(ecx);
                crate::audio_lane::set_return(0xba7e4);
                call!(0xd76a0);
                pc = 0xba7e4; continue;
            }
            0xba7e4 => {
                esi = eax;
                edi = m.read32(esi.wrapping_add(0x3c));
                esp = esp.wrapping_add(0x8);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xba89a; continue; }
                pc = 0xba7f4; continue;
            }
            0xba7f4 => {
                crate::audio_lane::set_return(0xba7f9);
                call!(0x153b40);
                pc = 0xba7f9; continue;
            }
            0xba7f9 => {
                edx = 0;
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (edi as u64)) as u32;
                edx = (dividend % (edi as u64)) as u32; }
                xmm0 = Xmm::scalar(m.read32(0x7e71f0));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d20))).to_bits();
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d5d0))).to_bits();
                edi = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                esi = (((((m.read16(esi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x24)) as i16)) as i32)) as u32);
                edx = esp.wrapping_add(0xc);
                push!(edx);
                crate::audio_lane::set_return(0xba836);
                call!(0x45390);
                pc = 0xba836; continue;
            }
            0xba836 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                eax = m.read32(esp.wrapping_add(0x50));
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x47cff4));
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4c)));
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax));
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                eax = esp.wrapping_add(0x10);
                push!(eax);
                m.write8(esp.wrapping_add(0x3c), (0x1) as u8);
                m.write32(esp.wrapping_add(0x14), esi);
                m.write32(esp.wrapping_add(0x18), edi);
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                crate::audio_lane::set_return(0xba897);
                call!(0x45820);
                pc = 0xba897; continue;
            }
            0xba897 => {
                esp = esp.wrapping_add(0x8);
                pc = 0xba89a; continue;
            }
            0xba89a => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x34);
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00164770: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f164770<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x164770;
    let mut jt = 0u32;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x164770, previous, pc); previous = pc; }
        match pc {
            0x164770 => {
                eax = m.read32(0x79094c);
                eax = eax.wrapping_sub(1);
                fa = eax;
                fb = ((((0x50i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x164834; continue; }
                pc = 0x16477f; continue;
            }
            0x16477f => {
                eax = ((m.read8(eax.wrapping_add(0x164870))) as u8 as u32);
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x164838));
                if (jt == 0x16478du32) { pc = 0x16478d; continue; }
                if (jt == 0x164794u32) { pc = 0x164794; continue; }
                if (jt == 0x16479eu32) { pc = 0x16479e; continue; }
                if (jt == 0x1647a8u32) { pc = 0x1647a8; continue; }
                if (jt == 0x1647bcu32) { pc = 0x1647bc; continue; }
                if (jt == 0x1647c6u32) { pc = 0x1647c6; continue; }
                if (jt == 0x1647dau32) { pc = 0x1647da; continue; }
                if (jt == 0x1647eeu32) { pc = 0x1647ee; continue; }
                if (jt == 0x1647f8u32) { pc = 0x1647f8; continue; }
                if (jt == 0x164802u32) { pc = 0x164802; continue; }
                if (jt == 0x164816u32) { pc = 0x164816; continue; }
                if (jt == 0x164820u32) { pc = 0x164820; continue; }
                if (jt == 0x16482au32) { pc = 0x16482a; continue; }
                if (jt == 0x164834u32) { pc = 0x164834; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x16478d => {
                eax = 0;
                tail_to!(0x164730); break 'run;  // direct tail jump
            }
            0x164794 => {
                eax = 0x1;
                tail_to!(0x164730); break 'run;  // direct tail jump
            }
            0x16479e => {
                eax = 0x2;
                tail_to!(0x164730); break 'run;  // direct tail jump
            }
            0x1647a8 => {
                eax = 0x3;
                crate::audio_lane::set_return(0x1647b2);
                call!(0x164730);
                pc = 0x1647b2; continue;
            }
            0x1647b2 => {
                eax = 0x4;
                tail_to!(0x164730); break 'run;  // direct tail jump
            }
            0x1647bc => {
                eax = 0x5;
                tail_to!(0x164730); break 'run;  // direct tail jump
            }
            0x1647c6 => {
                eax = 0x7;
                crate::audio_lane::set_return(0x1647d0);
                call!(0x164730);
                pc = 0x1647d0; continue;
            }
            0x1647d0 => {
                eax = 0x6;
                tail_to!(0x164730); break 'run;  // direct tail jump
            }
            0x1647da => {
                eax = 0x8;
                crate::audio_lane::set_return(0x1647e4);
                call!(0x164730);
                pc = 0x1647e4; continue;
            }
            0x1647e4 => {
                eax = 0x9;
                tail_to!(0x164730); break 'run;  // direct tail jump
            }
            0x1647ee => {
                eax = 0xa;
                tail_to!(0x164730); break 'run;  // direct tail jump
            }
            0x1647f8 => {
                eax = 0xb;
                tail_to!(0x164730); break 'run;  // direct tail jump
            }
            0x164802 => {
                eax = 0xc;
                crate::audio_lane::set_return(0x16480c);
                call!(0x164730);
                pc = 0x16480c; continue;
            }
            0x16480c => {
                eax = 0xd;
                tail_to!(0x164730); break 'run;  // direct tail jump
            }
            0x164816 => {
                eax = 0xe;
                tail_to!(0x164730); break 'run;  // direct tail jump
            }
            0x164820 => {
                eax = 0xf;
                tail_to!(0x164730); break 'run;  // direct tail jump
            }
            0x16482a => {
                eax = 0x10;
                tail_to!(0x164730); break 'run;  // direct tail jump
            }
            0x164834 => {
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00164E00: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f164e00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x164e00;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        fpu = cr.fpu;
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x164e00, previous, pc); previous = pc; }
        match pc {
            0x164e00 => {
                eax = m.read32(0x4e7944);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x164f5d; continue; }
                pc = 0x164e0d; continue;
            }
            0x164e0d => {
                ecx = m.read32(0x74c3f0);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                push!(esi);
                if (fa == fb) { pc = 0x164ed0; continue; }
                pc = 0x164e24; continue;
            }
            0x164e24 => {
                xmm0.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::divss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x4e794c))).to_bits();
                xmm0 = Xmm::scalar(m.read32(0x74c3d0));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::default();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(0x74c3d0, xmm0.lanes[0]);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x164ed0; continue; }
                pc = 0x164e57; continue;
            }
            0x164e57 => {
                esi = m.read32(0x72f5e8);
                eax = ecx;
                eax = eax.wrapping_shr((((((((0x16i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = ((eax as i32).wrapping_mul((0x2e8) as i32)) as u32;
                edx = m.read32(eax.wrapping_add(esi).wrapping_add(0x2c));
                eax = eax.wrapping_add(esi);
                fa = edx;
                fb = ecx;
                if (fa != fb) { pc = 0x164ec6; continue; }
                pc = 0x164e72; continue;
            }
            0x164e72 => {
                push!(edi);
                edi = m.read32(eax.wrapping_add(0x7c));
                crate::audio_lane::set_return(0x164e7b);
                call!(0x164470);
                pc = 0x164e7b; continue;
            }
            0x164e7b => {
                esi = eax;
                crate::audio_lane::set_return(0x164e82);
                call!(0x153b40);
                pc = 0x164e82; continue;
            }
            0x164e82 => {
                edx = 0;
                { let dividend = ((edx as u64) << 32) | eax as u64;
                eax = (dividend / (m.read32(esi) as u64)) as u32;
                edx = (dividend % (m.read32(esi) as u64)) as u32; }
                edx = m.read32(esi.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x4));
                fa = edx;
                fb = ((((0x1008i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x164ead; continue; }
                pc = 0x164e92; continue;
            }
            0x164e92 => {
                fa = edx;
                fb = ((((0x1b25i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x164ead; continue; }
                pc = 0x164e9a; continue;
            }
            0x164e9a => {
                push!(0);
                push!(0x900);
                push!(edx);
                push!(edi);
                crate::audio_lane::set_return(0x164ea8);
                call!(0x164c40);
                pc = 0x164ea8; continue;
            }
            0x164ea8 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x164ebd; continue;
            }
            0x164ead => {
                push!(0);
                push!(0x100);
                push!(edx);
                crate::audio_lane::set_return(0x164eba);
                call!(0x164310);
                pc = 0x164eba; continue;
            }
            0x164eba => {
                esp = esp.wrapping_add(0xc);
                pc = 0x164ebd; continue;
            }
            0x164ebd => {
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x164ec6; continue;
            }
            0x164ec6 => {
                m.write32(0x74c3f0, 0);
                pc = 0x164ed0; continue;
            }
            0x164ed0 => {
                xmm0 = Xmm::scalar(m.read32(0x74c3c8));
                xmm2 = Xmm::default();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x164f00; continue; }
                pc = 0x164ee0; continue;
            }
            0x164ee0 => {
                xmm3.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::divss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x4e794c))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(0x74c3c8, xmm0.lanes[0]);
                pc = 0x164f00; continue;
            }
            0x164f00 => {
                eax = m.read32(0x74c3ec);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x164f5c; continue; }
                pc = 0x164f09; continue;
            }
            0x164f09 => {
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x164f5c; continue; }
                pc = 0x164f0e; continue;
            }
            0x164f0e => {
                esi = m.read32(0x72f5e8);
                ecx = eax;
                ecx = ecx.wrapping_shr((((((((0x16i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ((ecx as i32).wrapping_mul((0x2e8) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(esi).wrapping_add(0x2c));
                ecx = ecx.wrapping_add(esi);
                fa = edx;
                fb = eax;
                if (fa != fb) { pc = 0x164f52; continue; }
                pc = 0x164f29; continue;
            }
            0x164f29 => {
                push!(0);
                push!(0x100);
                crate::audio_lane::set_return(0x164f35);
                call!(0x164900);
                pc = 0x164f35; continue;
            }
            0x164f35 => {
                push!(eax);
                eax = m.read32(ecx.wrapping_add(0x7c));
                push!(eax);
                crate::audio_lane::set_return(0x164f3f);
                call!(0x164c40);
                pc = 0x164f3f; continue;
            }
            0x164f3f => {
                xmm0 = Xmm::scalar(m.read32(0x4760cc));
                esp = esp.wrapping_add(0x10);
                m.write32(0x74c3c8, xmm0.lanes[0]);
                pc = 0x164f52; continue;
            }
            0x164f52 => {
                m.write32(0x74c3ec, 0);
                pc = 0x164f5c; continue;
            }
            0x164f5c => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x164f5d; continue;
            }
            0x164f5d => {
                esp = esp.wrapping_add(0); break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

pub const ROOTS: &[u32] = &[0x21780,0x21910,0x219e0,0x21a40,0x21bb0,0x21c80,0x26f00,0x273e0,0x274a0,0x27690,0x27700,0x27cc0,0x28110,0x28180,0x28760,0x298d0,0x299c0,0x29af0,0x29b20,0x29bb0,0x29be0,0x29d50,0x29d90,0x29dd0,0x29e40,0x29e50,0x29f30,0x44850,0x44ac0,0x454c0,0x45540,0x455b0,0x455c0,0x45600,0x45610,0x45a00,0x45c60,0x45fd0,0x462b0,0x462f0,0x46ab0,0x46cb0,0x63c00,0x7d960,0xba700,0xba7d0,0x164770,0x164e00];
/// Per-root A/B switch. Disabled/unknown roots leave state untouched.
pub fn run<M: GuestMemory,C: GuestCall>(va:u32,enabled:bool,m:&mut M,guest:&mut C,regs:&mut ExtRegisters)->bool { if !enabled {return false;} match va {
0x21780 => f21780(m,guest,regs),
0x21910 => f21910(m,guest,regs),
0x219e0 => f219e0(m,guest,regs),
0x21a40 => f21a40(m,guest,regs),
0x21bb0 => f21bb0(m,guest,regs),
0x21c80 => f21c80(m,guest,regs),
0x26f00 => f26f00(m,guest,regs),
0x273e0 => f273e0(m,guest,regs),
0x274a0 => f274a0(m,guest,regs),
0x27690 => f27690(m,guest,regs),
0x27700 => f27700(m,guest,regs),
0x27cc0 => f27cc0(m,guest,regs),
0x28110 => f28110(m,guest,regs),
0x28180 => f28180(m,guest,regs),
0x28760 => f28760(m,guest,regs),
0x298d0 => f298d0(m,guest,regs),
0x299c0 => f299c0(m,guest,regs),
0x29af0 => f29af0(m,guest,regs),
0x29b20 => f29b20(m,guest,regs),
0x29bb0 => f29bb0(m,guest,regs),
0x29be0 => f29be0(m,guest,regs),
0x29d50 => f29d50(m,guest,regs),
0x29d90 => f29d90(m,guest,regs),
0x29dd0 => f29dd0(m,guest,regs),
0x29e40 => f29e40(m,guest,regs),
0x29e50 => f29e50(m,guest,regs),
0x29f30 => f29f30(m,guest,regs),
0x44850 => f44850(m,guest,regs),
0x44ac0 => f44ac0(m,guest,regs),
0x454c0 => f454c0(m,guest,regs),
0x45540 => f45540(m,guest,regs),
0x455b0 => f455b0(m,guest,regs),
0x455c0 => f455c0(m,guest,regs),
0x45600 => f45600(m,guest,regs),
0x45610 => f45610(m,guest,regs),
0x45a00 => f45a00(m,guest,regs),
0x45c60 => f45c60(m,guest,regs),
0x45fd0 => f45fd0(m,guest,regs),
0x462b0 => f462b0(m,guest,regs),
0x462f0 => f462f0(m,guest,regs),
0x46ab0 => f46ab0(m,guest,regs),
0x46cb0 => f46cb0(m,guest,regs),
0x63c00 => f63c00(m,guest,regs),
0x7d960 => f7d960(m,guest,regs),
0xba700 => fba700(m,guest,regs),
0xba7d0 => fba7d0(m,guest,regs),
0x164770 => f164770(m,guest,regs),
0x164e00 => f164e00(m,guest,regs),
_ => return false, } true }
pub use f21780 as sound_request_submit_wrapper_hud;
pub use f21910 as call_45820_with_initialized_id_scalar18_xyz_packet;
pub use f219e0 as call_45820_with_initialized_id_word4_xyz_packet;
pub use f21a40 as call_45820_with_initialized_id_word4_scalar2c_xyz_packet;
pub use f21bb0 as call_45820_with_initialized_id_scalar2c_xyz_packet;
pub use f21c80 as sound_play_at_position_flag_0xa0_c;
pub use f26f00 as sound_linear_volume_to_millibels;
pub use f273e0 as sound_stop_by_handle;
pub use f274a0 as sound_handle_set_frequency_scale;
pub use f27690 as sound_handle_set_field_0x20_from_max_scaled;
pub use f27700 as sound_data_load;
pub use f27cc0 as sound_handle_set_position_optional_and_field_0x28;
pub use f28110 as sound_stream_set_field_0x5c_from_list_key_clamped;
pub use f28180 as sound_volume_to_millibels_blended;
pub use f28760 as sound_update_buffers_streams_and_do_work;
pub use f298d0 as sound_stream_create;
pub use f299c0 as sound_stream_configure;
pub use f29af0 as sound_stream_set_field_0x54_from_max_scaled;
pub use f29b20 as sound_stream_set_frequency;
pub use f29bb0 as sound_stream_find_first_nonpending_status_index;
pub use f29be0 as sound_stream_enqueue_async_read;
pub use f29d50 as sound_stream_set_max_distance_with_floor;
pub use f29d90 as sound_stream_set_field_0x5c_clamped;
pub use f29dd0 as sound_stream_set_position;
pub use f29e40 as sound_stream_set_field_0x64;
pub use f29e50 as sound_stream_set_field_0x58_f32;
pub use f29f30 as sound_stream_update_3d;
pub use f44850 as table_6b9908_find_by_key;
pub use f44ac0 as sound_pad_level_entries_update_masks_for_active_entries;
pub use f454c0 as thunk_sound_stream_set_field_0x5c_from_list_key_clamped;
pub use f45540 as sound_handle_is_playing_27bb0_including_deferred_slots;
pub use f455b0 as thunk_sound_pause_all_not_flagged_0x800;
pub use f455c0 as thunk_sound_resume_all_paused;
pub use f45600 as thunk_sound_handle_set_position_optional_and_field_0x28;
pub use f45610 as thunk_sound_handle_set_position_3d;
pub use f45a00 as sound_music_start_slot_unless_voice_masked_then_flush_stream;
pub use f45c60 as music_apply_menu_volume_settings_5236bc_then_set_scaled_volume;
pub use f45fd0 as music_start_for_mode_or_player_field_0x120_unless_flag_0x20000000;
pub use f462b0 as global_4d1850_set;
pub use f462f0 as sqrt_clamped_to_unit_float;
pub use f46ab0 as sound_play_random_pitch_if_near_player;
pub use f46cb0 as table_6d6028_play_sound_into_free_slot;
pub use f63c00 as struct_string_at_0x8_equals;
pub use f7d960 as sound_play_random_ui_tick_by_mode_0x1641_or_0x163e_plus_rng_mod_3;
pub use fba700 as sound_play_random_volume_scaled_by_global_4e8cec_a;
pub use fba7d0 as sound_play_random_volume_scaled_by_global_4e8cec_b;
pub use f164770 as table_4e7ce8_entries_reset_text_id_state_by_game_mode_jump_table;
pub use f164e00 as voice_line_scheduler_per_frame_pick_table_4e7ce8_entry_random_roll_and_start_line_by_map_ids_0x1b26_0x10e8;
