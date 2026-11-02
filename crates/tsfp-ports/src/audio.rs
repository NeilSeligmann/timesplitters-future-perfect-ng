// SPDX-License-Identifier: GPL-3.0-or-later
//! T1913 opt-in audio candidates; names INFERRED. See docs/t-system-audio.md.
#![allow(clippy::all, unused_parens, unused_assignments, unused_imports, unused_variables)]
use crate::{abi::{ExtRegisters, GuestCall, Xmm}, memory::GuestMemory, pose::write16};
use crate::state_wire as gs;
use crate::inventory::sse;
use crate::system_slice::{round_pc, read_double, sqrt_double, reciprocal_sqrt, compare_fp};
#[path = "audio_slice/boundary.rs"] mod boundary;
pub use boundary::*;
/// 0x000218D0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f218d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x218d0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x218d0, previous, pc); previous = pc; }
        match pc {
            0x218d0 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x218dc);
                call!(0x45390);
                pc = 0x218dc; continue;
            }
            0x218dc => {
                ecx = m.read32(esp.wrapping_add(0x14));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                ecx = (ecx | 0x200);
                m.write32(esp.wrapping_add(0x14), ecx);
                ecx = m.read32(esp.wrapping_add(0x38));
                edx = esp.wrapping_add(0x4);
                push!(edx);
                m.write32(esp.wrapping_add(0x8), ecx);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x21908);
                call!(0x45820);
                pc = 0x21908; continue;
            }
            0x21908 => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021970: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21970<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21970;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21970, previous, pc); previous = pc; }
        match pc {
            0x21970 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x2197c);
                call!(0x45390);
                pc = 0x2197c; continue;
            }
            0x2197c => {
                eax = m.read32(esp.wrapping_add(0x14));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                ecx = m.read32(esp.wrapping_add(0x3c));
                edx = m.read32(esp.wrapping_add(0x38));
                eax = (eax | 0xa0);
                m.write32(esp.wrapping_add(0x14), eax);
                eax = m.read32(esp.wrapping_add(0x44));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax));
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                eax = esp.wrapping_add(0x4);
                push!(eax);
                m.write8(esp.wrapping_add(0x30), (0x1) as u8);
                m.write32(esp.wrapping_add(0xc), ecx);
                m.write32(esp.wrapping_add(0x8), edx);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x219d8);
                call!(0x45820);
                pc = 0x219d8; continue;
            }
            0x219d8 => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021AA0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21aa0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21aa0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21aa0, previous, pc); previous = pc; }
        match pc {
            0x21aa0 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x21aac);
                call!(0x45390);
                pc = 0x21aac; continue;
            }
            0x21aac => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                eax = m.read32(esp.wrapping_add(0x4c));
                ecx = m.read32(esp.wrapping_add(0x38));
                edx = m.read32(esp.wrapping_add(0x3c));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
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
                crate::audio_lane::set_return(0x21b13);
                call!(0x45820);
                pc = 0x21b13; continue;
            }
            0x21b13 => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021B20: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21b20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21b20;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21b20, previous, pc); previous = pc; }
        match pc {
            0x21b20 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x21b2c);
                call!(0x45390);
                pc = 0x21b2c; continue;
            }
            0x21b2c => {
                eax = m.read32(esp.wrapping_add(0x14));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                ecx = m.read32(esp.wrapping_add(0x38));
                edx = m.read32(esp.wrapping_add(0x3c));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                eax = (eax | 0xa0);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
                m.write32(esp.wrapping_add(0x14), eax);
                eax = m.read32(esp.wrapping_add(0x4c));
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
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
                crate::audio_lane::set_return(0x21ba0);
                call!(0x45820);
                pc = 0x21ba0; continue;
            }
            0x21ba0 => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021C10: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21c10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21c10;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21c10, previous, pc); previous = pc; }
        match pc {
            0x21c10 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x21c1c);
                call!(0x45390);
                pc = 0x21c1c; continue;
            }
            0x21c1c => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                eax = m.read32(esp.wrapping_add(0x44));
                ecx = m.read32(esp.wrapping_add(0x38));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
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
                crate::audio_lane::set_return(0x21c6f);
                call!(0x45820);
                pc = 0x21c6f; continue;
            }
            0x21c6f => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021D00: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21d00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21d00;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21d00, previous, pc); previous = pc; }
        match pc {
            0x21d00 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x21d0c);
                call!(0x45390);
                pc = 0x21d0c; continue;
            }
            0x21d0c => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                eax = m.read32(esp.wrapping_add(0x48));
                ecx = m.read32(esp.wrapping_add(0x14));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
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
                crate::audio_lane::set_return(0x21d79);
                call!(0x45820);
                pc = 0x21d79; continue;
            }
            0x21d79 => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021D80: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21d80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21d80;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21d80, previous, pc); previous = pc; }
        match pc {
            0x21d80 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x21d8c);
                call!(0x45390);
                pc = 0x21d8c; continue;
            }
            0x21d8c => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                eax = m.read32(esp.wrapping_add(0x48));
                ecx = m.read32(esp.wrapping_add(0x14));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax));
                ecx = (ecx | 0x2a0);
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
                crate::audio_lane::set_return(0x21df9);
                call!(0x45820);
                pc = 0x21df9; continue;
            }
            0x21df9 => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021E00: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21e00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21e00;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21e00, previous, pc); previous = pc; }
        match pc {
            0x21e00 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x21e0c);
                call!(0x45390);
                pc = 0x21e0c; continue;
            }
            0x21e0c => {
                eax = m.read32(esp.wrapping_add(0x40));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                ecx = m.read32(esp.wrapping_add(0x38));
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
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
                crate::audio_lane::set_return(0x21e53);
                call!(0x45820);
                pc = 0x21e53; continue;
            }
            0x21e53 => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021E60: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21e60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21e60;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21e60, previous, pc); previous = pc; }
        match pc {
            0x21e60 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x21e6c);
                call!(0x45390);
                pc = 0x21e6c; continue;
            }
            0x21e6c => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                eax = m.read32(esp.wrapping_add(0x44));
                ecx = m.read32(esp.wrapping_add(0x38));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
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
                crate::audio_lane::set_return(0x21ebf);
                call!(0x45820);
                pc = 0x21ebf; continue;
            }
            0x21ebf => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021ED0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21ed0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21ed0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21ed0, previous, pc); previous = pc; }
        match pc {
            0x21ed0 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x21edc);
                call!(0x45390);
                pc = 0x21edc; continue;
            }
            0x21edc => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                eax = m.read32(esp.wrapping_add(0x4c));
                edx = m.read32(esp.wrapping_add(0x50));
                ecx = m.read32(esp.wrapping_add(0x38));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax));
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                m.write32(esp.wrapping_add(0x14), (m.read32(esp.wrapping_add(0x14)) | edx));
                eax = esp.wrapping_add(0x4);
                push!(eax);
                m.write8(esp.wrapping_add(0x30), (0x1) as u8);
                m.write32(esp.wrapping_add(0x8), ecx);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x21f4f);
                call!(0x45820);
                pc = 0x21f4f; continue;
            }
            0x21f4f => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021F60: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21f60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21f60;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21f60, previous, pc); previous = pc; }
        match pc {
            0x21f60 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x21f6c);
                call!(0x45390);
                pc = 0x21f6c; continue;
            }
            0x21f6c => {
                eax = m.read32(esp.wrapping_add(0x14));
                edx = m.read32(esp.wrapping_add(0x44));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                ecx = m.read32(esp.wrapping_add(0x38));
                eax = (eax | edx);
                m.write32(esp.wrapping_add(0x14), eax);
                eax = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                push!(eax);
                m.write32(esp.wrapping_add(0x8), ecx);
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x21fa4);
                call!(0x45820);
                pc = 0x21fa4; continue;
            }
            0x21fa4 => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00021FB0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21fb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21fb0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21fb0, previous, pc); previous = pc; }
        match pc {
            0x21fb0 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x21fbc);
                call!(0x45390);
                pc = 0x21fbc; continue;
            }
            0x21fbc => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                eax = m.read32(esp.wrapping_add(0x48));
                ecx = m.read32(esp.wrapping_add(0x38));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax));
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                edx = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
                push!(edx);
                m.write8(esp.wrapping_add(0x30), (0x1) as u8);
                m.write32(esp.wrapping_add(0x8), ecx);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x2201b);
                call!(0x45820);
                pc = 0x2201b; continue;
            }
            0x2201b => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00026BB0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f26bb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x26bb0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x26bb0, previous, pc); previous = pc; }
        match pc {
            0x26bb0 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x26bef; continue; }
                pc = 0x26bb4; continue;
            }
            0x26bb4 => {
                eax = m.read32(0x5836c0);
                fa = eax;
                fb = ((((0x583690i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x26bcf; continue; }
                pc = 0x26bc0; continue;
            }
            0x26bc0 => {
                fa = m.read32(eax.wrapping_add(0x4));
                fb = ecx;
                if (fa == fb) { pc = 0x26bf1; continue; }
                pc = 0x26bc5; continue;
            }
            0x26bc5 => {
                eax = m.read32(eax.wrapping_add(0x30));
                fa = eax;
                fb = ((((0x583690i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x26bc0; continue; }
                pc = 0x26bcf; continue;
            }
            0x26bcf => {
                eax = m.read32(0x5659f4);
                fa = eax;
                fb = ((((0x5659c4i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x26bef; continue; }
                pc = 0x26bdb; continue;
            }
            0x26bdb => {
                pc = 0x26be0; continue;
            }
            0x26be0 => {
                fa = m.read32(eax.wrapping_add(0x4));
                fb = ecx;
                if (fa == fb) { pc = 0x26bf1; continue; }
                pc = 0x26be5; continue;
            }
            0x26be5 => {
                eax = m.read32(eax.wrapping_add(0x30));
                fa = eax;
                fb = ((((0x5659c4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x26be0; continue; }
                pc = 0x26bef; continue;
            }
            0x26bef => {
                eax = 0;
                pc = 0x26bf1; continue;
            }
            0x26bf1 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00026C00: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f26c00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x26c00;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x26c00, previous, pc); previous = pc; }
        match pc {
            0x26c00 => {
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x38) as i32)) as u32;
                m.write32(ecx.wrapping_add(0x5819b8), 0);
                ecx = m.read32(0x565b40);
                m.write32(ecx.wrapping_mul(0x4).wrapping_add(0x5657c0), eax);
                ecx = ecx.wrapping_add(1);
                m.write32(0x565b40, ecx);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00026C30: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f26c30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x26c30;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x26c30, previous, pc); previous = pc; }
        match pc {
            0x26c30 => {
                eax = 0;
                edx = 0x5819f0;
                pc = 0x26c40; continue;
            }
            0x26c40 => {
                ecx = eax.wrapping_add(0x1);
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x5657c0), ecx);
                m.write32(edx, 0);
                eax = ecx;
                edx = edx.wrapping_add(0x38);
                fa = eax;
                fb = ((((0x80i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x26c40; continue; }
                pc = 0x26c5c; continue;
            }
            0x26c5c => {
                eax = 0x5659c4;
                m.write32(0x5659f4, eax);
                m.write32(0x5659f8, eax);
                eax = 0x583690;
                m.write32(0x565b40, 0x80);
                m.write32(0x5836c0, eax);
                m.write32(0x5836c4, eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00026C90: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f26c90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x26c90;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x26c90, previous, pc); previous = pc; }
        match pc {
            0x26c90 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                eax = 0;
                edx = ecx;
                m.write32(edx, eax);
                m.write32(edx.wrapping_add(0x4), eax);
                m.write32(edx.wrapping_add(0x8), eax);
                m.write32(edx.wrapping_add(0xc), eax);
                m.write32(edx.wrapping_add(0x10), eax);
                eax = m.read32(esp.wrapping_add(0xc));
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0x4i32)) as u32) & 0xffffu32);
                edx = m.read32(esp.wrapping_add(0x10));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                write16(m, ecx.wrapping_add(0x2), (((((edx) as u16)) as u32)) as u16);
                m.write32(ecx.wrapping_add(0x4), esi);
                write16(m, ecx.wrapping_add(0xe), (((((eax) as u16)) as u32)) as u16);
                if (fa != fb) { pc = 0x26ced; continue; }
                pc = 0x26cc4; continue;
            }
            0x26cc4 => {
                eax = edx.wrapping_add(edx.wrapping_mul(0x8));
                eax = eax.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                write16(m, ecx.wrapping_add(0xc), (((((eax) as u16)) as u32)) as u16);
                eax = ((((eax) as u16)) as u16 as u32);
                eax = ((eax as i32).wrapping_mul((esi) as i32)) as u32;
                eax = eax.wrapping_shr((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                write16(m, ecx, (0x69) as u16);
                write16(m, ecx.wrapping_add(0x10), (0x2) as u16);
                write16(m, ecx.wrapping_add(0x12), (0x40) as u16);
                m.write32(ecx.wrapping_add(0x8), eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x26ced => {
                eax = ((((eax) as u16)) as u16 as u32);
                edx = ((((edx) as u16)) as u16 as u32);
                eax = ((eax as i32).wrapping_mul((edx) as i32)) as u32;
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                edx = (edx & 0x7);
                eax = eax.wrapping_add(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x3i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                write16(m, ecx.wrapping_add(0xc), (((((eax) as u16)) as u32)) as u16);
                eax = ((((eax) as u16)) as u16 as u32);
                eax = ((eax as i32).wrapping_mul((esi) as i32)) as u32;
                write16(m, ecx, (0x1) as u16);
                write16(m, ecx.wrapping_add(0x10), (0) as u16);
                write16(m, ecx.wrapping_add(0x12), (0) as u16);
                m.write32(ecx.wrapping_add(0x8), eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00026D20: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f26d20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x26d20;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x26d20, previous, pc); previous = pc; }
        match pc {
            0x26d20 => {
                esp = esp.wrapping_sub(0x10);
                xmm0 = Xmm::scalar(m.read32(0x475d5c));
                push!(ebx);
                push!(ebp);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x24));
                eax = esp.wrapping_add(0x10);
                push!(eax);
                push!(edi);
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                ebp = 0;
                crate::audio_lane::set_return(0x26d45);
                call!(0x45200);
                pc = 0x26d45; continue;
            }
            0x26d45 => {
                esp = esp.wrapping_add(0x8);
                ebx = 0;
                crate::audio_lane::set_return(0x26d4f);
                call!(0x1cdae0);
                pc = 0x26d4f; continue;
            }
            0x26d4f => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x26e73; continue; }
                pc = 0x26d57; continue;
            }
            0x26d57 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x24));
                pc = 0x26d60; continue;
            }
            0x26d60 => {
                push!(ebx);
                crate::audio_lane::set_return(0x26d66);
                call!(0x356800);
                pc = 0x26d66; continue;
            }
            0x26d66 => {
                eax = ((eax as i32).wrapping_mul((0x1584) as i32)) as u32;
                ecx = m.read32(0x7b0c48);
                eax = m.read32(eax.wrapping_add(ecx).wrapping_add(0x8));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x26de5; continue; }
                pc = 0x26d7d; continue;
            }
            0x26d7d => {
                xmm0 = Xmm::scalar(m.read32(edi));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x27c)))).to_bits();
                m.write32(esi, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x280)))).to_bits();
                xmm2 = xmm0;
                m.write32(esi.wrapping_add(0x4), xmm0.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(0x8)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x284)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(esi));
                xmm3 = xmm2;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esi.wrapping_add(0x8), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x26de5; continue; }
                pc = 0x26ddd; continue;
            }
            0x26ddd => {
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                ebp = eax;
                pc = 0x26de5; continue;
            }
            0x26de5 => {
                ebx = ebx.wrapping_add(1);
                crate::audio_lane::set_return(0x26deb);
                call!(0x1cdae0);
                pc = 0x26deb; continue;
            }
            0x26deb => {
                fa = ebx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x26d60; continue; }
                pc = 0x26df3; continue;
            }
            0x26df3 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475d5c)) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (!(((((fa) & (fb))) as u8).count_ones() % 2 == 0)) { pc = 0x26e72; continue; }
                pc = 0x26e06; continue;
            }
            0x26e06 => {
                xmm1 = Xmm::scalar(m.read32(0x475d24));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x26e61; continue; }
                pc = 0x26e13; continue;
            }
            0x26e13 => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(edi));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                m.write32(esi, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                m.write32(esi.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                m.write32(esi.wrapping_add(0x8), xmm0.lanes[0]);
                if (fa == fb) { pc = 0x26e72; continue; }
                pc = 0x26e45; continue;
            }
            0x26e45 => {
                eax = m.read32(ebp.wrapping_add(0x208));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x26e72; continue; }
                pc = 0x26e4f; continue;
            }
            0x26e4f => {
                push!(esi);
                push!(eax);
                crate::audio_lane::set_return(0x26e56);
                call!(0x152090);
                pc = 0x26e56; continue;
            }
            0x26e56 => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            0x26e61 => {
                xmm0 = Xmm::default();
                m.write32(esi, xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x4), xmm0.lanes[0]);
                m.write32(esi.wrapping_add(0x8), xmm1.lanes[0]);
                pc = 0x26e72; continue;
            }
            0x26e72 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x26e73; continue;
            }
            0x26e73 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00026E80: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f26e80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x26e80;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x26e80, previous, pc); previous = pc; }
        match pc {
            0x26e80 => {
                eax = m.read32(0x4b9110);
                push!(ebx);
                push!(esi);
                push!(edi);
                edi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x26eb4; continue; }
                pc = 0x26e8e; continue;
            }
            0x26e8e => {
                ebx = m.read32(esp.wrapping_add(0x10));
                esi = 0x4b90b0;
                pc = 0x26e97; continue;
            }
            0x26e97 => {
                eax = m.read32(esi);
                push!(eax);
                push!(ebx);
                crate::audio_lane::set_return(0x26ea0);
                call!(0x3c8690);
                pc = 0x26ea0; continue;
            }
            0x26ea0 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x26eba; continue; }
                pc = 0x26ea7; continue;
            }
            0x26ea7 => {
                eax = m.read32(0x4b9110);
                edi = edi.wrapping_add(1);
                esi = esi.wrapping_add(0xc);
                fa = edi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x26e97; continue; }
                pc = 0x26eb4; continue;
            }
            0x26eb4 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x26eba => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00026EC0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f26ec0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x26ec0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x26ec0, previous, pc); previous = pc; }
        match pc {
            0x26ec0 => {
                esp = esp.wrapping_sub(0xc);
                push!(eax);
                ecx = esp.wrapping_add(0x4);
                push!(ecx);
                crate::audio_lane::set_return(0x26ece);
                call!(0x26d20);
                pc = 0x26ece; continue;
            }
            0x26ece => {
                xmm0 = Xmm::default();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                edx = m.read32(esp.wrapping_add(0xc));
                eax = m.read32(esp.wrapping_add(0x8));
                esp = esp.wrapping_add(0x8);
                push!(0x1);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x18));
                m.write32(esp, xmm0.lanes[0]);
                push!(edx);
                push!(eax);
                push!(ecx);
                crate::audio_lane::set_return(0x26ef6);
                call_to!(0x00408556);
                pc = 0x26ef6; continue;
            }
            0x26ef6 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00026F70: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f26f70<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x26f70;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x26f70, previous, pc); previous = pc; }
        match pc {
            0x26f70 => {
                eax = m.read32(esp.wrapping_add(0x4));
                edx = esp.wrapping_add(((0x100i32.wrapping_neg()) as u32));
                esp = esp.wrapping_sub(0x104);
                edx = edx.wrapping_sub(eax);
                pc = 0x26f83; continue;
            }
            0x26f83 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                m.write8(edx.wrapping_add(eax), (((((ecx) as u8)) as u32)) as u8);
                eax = eax.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x26f83; continue; }
                pc = 0x26f8d; continue;
            }
            0x26f8d => {
                eax = esp.wrapping_add(0x4);
                push!(0x4761e0);
                push!(eax);
                crate::audio_lane::set_return(0x26f9c);
                call!(0x3c8690);
                pc = 0x26f9c; continue;
            }
            0x26f9c => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x26fb4; continue; }
                pc = 0x26fa3; continue;
            }
            0x26fa3 => {
                ecx = m.read32(0x4761d8);
                m.write32(eax, ecx);
                edx = (edx & 0xffff_ff00) | ((((m.read8(0x4761dc)) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x4), (((((edx) as u8)) as u32)) as u8);
                pc = 0x26fb4; continue;
            }
            0x26fb4 => {
                push!(esi);
                eax = esp.wrapping_add(0x8);
                push!(0);
                push!(eax);
                crate::audio_lane::set_return(0x26fc1);
                call!(0x62c30);
                pc = 0x26fc1; continue;
            }
            0x26fc1 => {
                esi = eax;
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x26fdd; continue; }
                pc = 0x26fca; continue;
            }
            0x26fca => {
                ecx = esp.wrapping_add(0x8);
                push!(ecx);
                crate::audio_lane::set_return(0x26fd4);
                call!(0x62ca0);
                pc = 0x26fd4; continue;
            }
            0x26fd4 => {
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x4), eax);
                pc = 0x26ff3; continue;
            }
            0x26fdd => {
                edx = esp.wrapping_add(0x4);
                push!(edx);
                eax = esp.wrapping_add(0xc);
                push!(0x1);
                push!(eax);
                crate::audio_lane::set_return(0x26fee);
                call!(0x62f00);
                pc = 0x26fee; continue;
            }
            0x26fee => {
                esp = esp.wrapping_add(0xc);
                esi = eax;
                pc = 0x26ff3; continue;
            }
            0x26ff3 => {
                ecx = esp.wrapping_add(0x8);
                push!(0x1);
                push!(ecx);
                crate::audio_lane::set_return(0x26fff);
                call!(0x389b0);
                pc = 0x26fff; continue;
            }
            0x26fff => {
                eax = m.read32(esp.wrapping_add(0xc));
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x2701d; continue; }
                pc = 0x2700a; continue;
            }
            0x2700a => {
                edx = m.read32(esp.wrapping_add(0x110));
                m.write32(edx, eax);
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x104);
                break 'run;
            }
            0x2701d => {
                eax = 0;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x104);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00027030: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27030<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27030;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x27030, previous, pc); previous = pc; }
        match pc {
            0x27030 => {
                eax = m.read32(0x5836c8);
                ecx = m.read32(0x58198c);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & ecx);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00027050: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27050<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27050;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x27050, previous, pc); previous = pc; }
        match pc {
            0x27050 => {
                esp = esp.wrapping_sub(0x8);
                push!(esi);
                push!(eax);
                crate::audio_lane::set_return(0x2705a);
                call!(0x37c914);
                pc = 0x2705a; continue;
            }
            0x2705a => {
                esi = eax;
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x27070; continue; }
                pc = 0x27061; continue;
            }
            0x27061 => {
                push!(0x1);
                crate::audio_lane::set_return(0x27068);
                call!(0x389b0);
                pc = 0x27068; continue;
            }
            0x27068 => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x27070 => {
                push!(edi);
                push!(esi);
                crate::audio_lane::set_return(0x27077);
                call!(0x37c97b);
                pc = 0x27077; continue;
            }
            0x27077 => {
                push!(esi);
                edi = eax;
                crate::audio_lane::set_return(0x2707f);
                call!(0x37c9ba);
                pc = 0x2707f; continue;
            }
            0x2707f => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x270bf; continue; }
                pc = 0x27083; continue;
            }
            0x27083 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x270b9; continue; }
                pc = 0x27087; continue;
            }
            0x27087 => {
                edx = m.read32(0x581988);
                push!(0x58198c);
                ecx = esp.wrapping_add(0xc);
                push!(ecx);
                push!(eax);
                push!(edi);
                push!(edx);
                m.write32(esp.wrapping_add(0x1c), 0x3);
                m.write32(esp.wrapping_add(0x20), 0x4);
                crate::audio_lane::set_return(0x270af);
                call_to!(0x004079DB);
                pc = 0x270af; continue;
            }
            0x270af => {
                m.write32(0x5836c8, 0x1);
                pc = 0x270b9; continue;
            }
            0x270b9 => {
                push!(esi);
                crate::audio_lane::set_return(0x270bf);
                call!(0x37c99c);
                pc = 0x270bf; continue;
            }
            0x270bf => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000270D0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f270d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x270d0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x270d0, previous, pc); previous = pc; }
        match pc {
            0x270d0 => {
                push!(ebx);
                push!(ebp);
                push!(esi);
                esi = m.read32(0x5659f4);
                fa = esi;
                fb = ((((0x5659c4i32)) as u32) & 0xffffffffu32);
                push!(edi);
                ebp = 0x800;
                ebx = (ebx & 0xffff_ff00) | ((0x2) & 0xff);
                if (fa == fb) { pc = 0x27120; continue; }
                pc = 0x270e9; continue;
            }
            0x270e9 => {
                pc = 0x270f0; continue;
            }
            0x270f0 => {
                ecx = m.read32(esi);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = m.read32(esi.wrapping_add(0x30));
                if (fa == fb) { pc = 0x27116; continue; }
                pc = 0x270f9; continue;
            }
            0x270f9 => {
                eax = m.read32(esi.wrapping_add(0x18));
                edx = m.read32(0x4ca9b4);
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                fa = m.read32(edx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x8));
                fb = ebp;
                if ((fa & fb) != 0) { pc = 0x27116; continue; }
                pc = 0x2710b; continue;
            }
            0x2710b => {
                push!(0x1);
                push!(ecx);
                crate::audio_lane::set_return(0x27113);
                call_to!(0x00407ABC);
                pc = 0x27113; continue;
            }
            0x27113 => {
                m.write8(esi.wrapping_add(0x17), (((((ebx) as u8)) as u32)) as u8);
                pc = 0x27116; continue;
            }
            0x27116 => {
                fa = edi;
                fb = ((((0x5659c4i32)) as u32) & 0xffffffffu32);
                esi = edi;
                if (fa != fb) { pc = 0x270f0; continue; }
                pc = 0x27120; continue;
            }
            0x27120 => {
                esi = m.read32(0x5836c0);
                fa = esi;
                fb = ((((0x583690i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27160; continue; }
                pc = 0x2712e; continue;
            }
            0x2712e => {
                edi = edi;
                pc = 0x27130; continue;
            }
            0x27130 => {
                ecx = m.read32(esi);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = m.read32(esi.wrapping_add(0x30));
                if (fa == fb) { pc = 0x27156; continue; }
                pc = 0x27139; continue;
            }
            0x27139 => {
                eax = m.read32(esi.wrapping_add(0x18));
                edx = m.read32(0x4ca9b4);
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                fa = m.read32(edx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x8));
                fb = ebp;
                if ((fa & fb) != 0) { pc = 0x27156; continue; }
                pc = 0x2714b; continue;
            }
            0x2714b => {
                push!(0x1);
                push!(ecx);
                crate::audio_lane::set_return(0x27153);
                call_to!(0x00407ABC);
                pc = 0x27153; continue;
            }
            0x27153 => {
                m.write8(esi.wrapping_add(0x17), (((((ebx) as u8)) as u32)) as u8);
                pc = 0x27156; continue;
            }
            0x27156 => {
                fa = edi;
                fb = ((((0x583690i32)) as u32) & 0xffffffffu32);
                esi = edi;
                if (fa != fb) { pc = 0x27130; continue; }
                pc = 0x27160; continue;
            }
            0x27160 => {
                esi = 0x5836d0;
                pc = 0x27165; continue;
            }
            0x27165 => {
                eax = m.read32(esi.wrapping_add(0x4c));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27183; continue; }
                pc = 0x2716d; continue;
            }
            0x2716d => {
                ecx = m.read32(0x4ca9b4);
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                fa = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x8));
                fb = ebp;
                if ((fa & fb) != 0) { pc = 0x27183; continue; }
                pc = 0x2717c; continue;
            }
            0x2717c => {
                ecx = esi;
                crate::audio_lane::set_return(0x27183);
                call!(0x29ea0);
                pc = 0x27183; continue;
            }
            0x27183 => {
                esi = esi.wrapping_add(0x14094);
                fa = esi;
                fb = ((((0x64bc98i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27165; continue; }
                pc = 0x27191; continue;
            }
            0x27191 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
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

/// 0x000271A0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f271a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x271a0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x271a0, previous, pc); previous = pc; }
        match pc {
            0x271a0 => {
                push!(ebx);
                push!(esi);
                esi = m.read32(0x5659f4);
                ebx = 0;
                fa = esi;
                fb = ((((0x5659c4i32)) as u32) & 0xffffffffu32);
                push!(edi);
                if (fa == fb) { pc = 0x271d5; continue; }
                pc = 0x271b3; continue;
            }
            0x271b3 => {
                eax = m.read32(esi);
                fa = eax;
                fb = ebx;
                edi = m.read32(esi.wrapping_add(0x30));
                if (fa == fb) { pc = 0x271cb; continue; }
                pc = 0x271bc; continue;
            }
            0x271bc => {
                fa = ((((m.read8(esi.wrapping_add(0x17)))) as u32) & 0xffu32);
                fb = ((((((ebx) as u8))) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x271cb; continue; }
                pc = 0x271c1; continue;
            }
            0x271c1 => {
                push!(ebx);
                push!(eax);
                crate::audio_lane::set_return(0x271c8);
                call_to!(0x00407ABC);
                pc = 0x271c8; continue;
            }
            0x271c8 => {
                m.write8(esi.wrapping_add(0x17), (((((ebx) as u8)) as u32)) as u8);
                pc = 0x271cb; continue;
            }
            0x271cb => {
                fa = edi;
                fb = ((((0x5659c4i32)) as u32) & 0xffffffffu32);
                esi = edi;
                if (fa != fb) { pc = 0x271b3; continue; }
                pc = 0x271d5; continue;
            }
            0x271d5 => {
                esi = m.read32(0x5836c0);
                fa = esi;
                fb = ((((0x583690i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27205; continue; }
                pc = 0x271e3; continue;
            }
            0x271e3 => {
                eax = m.read32(esi);
                fa = eax;
                fb = ebx;
                edi = m.read32(esi.wrapping_add(0x30));
                if (fa == fb) { pc = 0x271fb; continue; }
                pc = 0x271ec; continue;
            }
            0x271ec => {
                fa = ((((m.read8(esi.wrapping_add(0x17)))) as u32) & 0xffu32);
                fb = ((((((ebx) as u8))) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x271fb; continue; }
                pc = 0x271f1; continue;
            }
            0x271f1 => {
                push!(ebx);
                push!(eax);
                crate::audio_lane::set_return(0x271f8);
                call_to!(0x00407ABC);
                pc = 0x271f8; continue;
            }
            0x271f8 => {
                m.write8(esi.wrapping_add(0x17), (((((ebx) as u8)) as u32)) as u8);
                pc = 0x271fb; continue;
            }
            0x271fb => {
                fa = edi;
                fb = ((((0x583690i32)) as u32) & 0xffffffffu32);
                esi = edi;
                if (fa != fb) { pc = 0x271e3; continue; }
                pc = 0x27205; continue;
            }
            0x27205 => {
                esi = 0x5836f4;
                edi = 0x800;
                pc = 0x27210; continue;
            }
            0x27210 => {
                eax = m.read32(esi.wrapping_add(0x28));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27233; continue; }
                pc = 0x27218; continue;
            }
            0x27218 => {
                ecx = m.read32(0x4ca9b4);
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                fa = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x8));
                fb = edi;
                if ((fa & fb) != 0) { pc = 0x27233; continue; }
                pc = 0x27227; continue;
            }
            0x27227 => {
                fa = m.read32(esi);
                fb = ebx;
                if (fa != fb) { pc = 0x27233; continue; }
                pc = 0x2722b; continue;
            }
            0x2722b => {
                ecx = esi.wrapping_add(((0x24i32.wrapping_neg()) as u32));
                crate::audio_lane::set_return(0x27233);
                call!(0x29ee0);
                pc = 0x27233; continue;
            }
            0x27233 => {
                esi = esi.wrapping_add(0x14094);
                fa = esi;
                fb = ((((0x64bcbci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27210; continue; }
                pc = 0x27241; continue;
            }
            0x27241 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00027290: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27290<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27290;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x27290, previous, pc); previous = pc; }
        match pc {
            0x27290 => {
                esp = esp.wrapping_sub(0x100);
                push!(esi);
                esi = eax.wrapping_add(eax.wrapping_mul(0x4));
                eax = m.read32(0x4ca9b4);
                esi = esi.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = m.read32(esi.wrapping_add(eax).wrapping_add(0x8));
                fa = ecx;
                fb = ((((0x240i32)) as u32) & 0xffffffffu32);
                push!(edi);
                if ((fa & fb) == 0) { pc = 0x2736b; continue; }
                pc = 0x272b3; continue;
            }
            0x272b3 => {
                edi = ecx;
                edi = (edi & 0x240);
                edi = edi.wrapping_sub(0x40);
                cf = (((((edi) != 0))) as i32);
                edi = (((((edi) as i32).wrapping_neg())) as u32);
                edi = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                edx = esp.wrapping_add(0x8);
                edi = edi.wrapping_add(1);
                eax = ebx;
                edx = edx.wrapping_sub(ebx);
                pc = 0x272d0; continue;
            }
            0x272d0 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                m.write8(edx.wrapping_add(eax), (((((ecx) as u8)) as u32)) as u8);
                eax = eax.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x272d0; continue; }
                pc = 0x272da; continue;
            }
            0x272da => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = 0x4761e0;
                if (fa != fb) { pc = 0x272e8; continue; }
                pc = 0x272e3; continue;
            }
            0x272e3 => {
                eax = 0x4761f0;
                pc = 0x272e8; continue;
            }
            0x272e8 => {
                push!(eax);
                eax = esp.wrapping_add(0xc);
                push!(eax);
                crate::audio_lane::set_return(0x272f3);
                call!(0x3c8690);
                pc = 0x272f3; continue;
            }
            0x272f3 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2731a; continue; }
                pc = 0x272fa; continue;
            }
            0x272fa => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edx = 0x4761d8;
                if (fa != fb) { pc = 0x27308; continue; }
                pc = 0x27303; continue;
            }
            0x27303 => {
                edx = 0x4761e8;
                pc = 0x27308; continue;
            }
            0x27308 => {
                ecx = eax;
                pc = 0x27310; continue;
            }
            0x27310 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edx)) as u32)) & 0xff);
                edx = edx.wrapping_add(1);
                m.write8(ecx, (((((eax) as u8)) as u32)) as u8);
                ecx = ecx.wrapping_add(1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x27310; continue; }
                pc = 0x2731a; continue;
            }
            0x2731a => {
                ecx = esp.wrapping_add(0x8);
                push!(ecx);
                m.write32(esi.wrapping_add(0x565b48), 0);
                crate::audio_lane::set_return(0x2732e);
                call!(0x27250);
                pc = 0x2732e; continue;
            }
            0x2732e => {
                edx = esp.wrapping_add(0xc);
                push!(edx);
                m.write32(esi.wrapping_add(0x565b4c), eax);
                m.write32(esi.wrapping_add(0x565b50), edi);
                crate::audio_lane::set_return(0x27344);
                call!(0x62ca0);
                pc = 0x27344; continue;
            }
            0x27344 => {
                m.write32(esi.wrapping_add(0x565b54), eax);
                eax = m.read32(0x4ca9b4);
                m.write32(esi.wrapping_add(eax).wrapping_add(0xc), 0x1);
                ecx = m.read32(esi.wrapping_add(0x565b54));
                edx = m.read32(0x4ca9b4);
                esp = esp.wrapping_add(0x8);
                m.write32(esi.wrapping_add(edx), ecx);
                pc = 0x273bd; continue;
            }
            0x2736b => {
                eax = m.read32(0x7de510);
                push!(eax);
                crate::audio_lane::set_return(0x27376);
                call!(0x624a0);
                pc = 0x27376; continue;
            }
            0x27376 => {
                edi = esi.wrapping_add(0x565b54);
                push!(edi);
                push!(ebx);
                crate::audio_lane::set_return(0x27383);
                call!(0x26f70);
                pc = 0x27383; continue;
            }
            0x27383 => {
                ecx = m.read32(0x4ca9b4);
                m.write32(esi.wrapping_add(0x565b48), eax);
                m.write32(esi.wrapping_add(0x565b4c), 0);
                m.write32(esi.wrapping_add(0x565b50), 0xffffffffu32);
                m.write32(esi.wrapping_add(ecx).wrapping_add(0xc), 0x1);
                edx = m.read32(edi);
                eax = m.read32(0x4ca9b4);
                esp = esp.wrapping_add(0xc);
                m.write32(esi.wrapping_add(eax), edx);
                crate::audio_lane::set_return(0x273bd);
                call!(0x624b0);
                pc = 0x273bd; continue;
            }
            0x273bd => {
                push!(ebx);
                crate::audio_lane::set_return(0x273c3);
                call!(0x26e80);
                pc = 0x273c3; continue;
            }
            0x273c3 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esi.wrapping_add(0x565b58), eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x100);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00027980: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27980<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27980;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x27980, previous, pc); previous = pc; }
        match pc {
            0x27980 => {
                push!(esi);
                esi = 0x5836d0;
                pc = 0x27986; continue;
            }
            0x27986 => {
                ecx = esi;
                crate::audio_lane::set_return(0x2798d);
                call!(0x298b0);
                pc = 0x2798d; continue;
            }
            0x2798d => {
                esi = esi.wrapping_add(0x14094);
                fa = esi;
                fb = ((((0x64bc98i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27986; continue; }
                pc = 0x2799b; continue;
            }
            0x2799b => {
                ecx = 0;
                eax = 0;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x279a0; continue;
            }
            0x279a0 => {
                edx = m.read32(0x4ca9b4);
                m.write32(eax.wrapping_add(edx).wrapping_add(0xc), ecx);
                edx = m.read32(0x4ca9b4);
                m.write32(eax.wrapping_add(edx).wrapping_add(0x10), ecx);
                eax = eax.wrapping_add(0x14);
                fa = eax;
                fb = ((((0x1bd8ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x279a0; continue; }
                pc = 0x279be; continue;
            }
            0x279be => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000279C0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f279c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x279c0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x279c0, previous, pc); previous = pc; }
        match pc {
            0x279c0 => {
                esp = esp.wrapping_sub(0x50);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                esi = 0;
                push!(esi);
                push!(0x581988);
                push!(esi);
                crate::audio_lane::set_return(0x279d5);
                call_to!(0x00409635);
                pc = 0x279d5; continue;
            }
            0x279d5 => {
                crate::audio_lane::set_return(0x279da);
                call_to!(0x00406AB6);
                pc = 0x279da; continue;
            }
            0x279da => {
                eax = 0x476210;
                crate::audio_lane::set_return(0x279e4);
                call!(0x27050);
                pc = 0x279e4; continue;
            }
            0x279e4 => {
                ebx = 0;
                edi = 0x5836d0;
                pc = 0x279f0; continue;
            }
            0x279f0 => {
                eax = 0;
                fa = ebx;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = (eax & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ecx = edi;
                push!(eax);
                crate::audio_lane::set_return(0x27a00);
                call!(0x298d0);
                pc = 0x27a00; continue;
            }
            0x27a00 => {
                edi = edi.wrapping_add(0x14094);
                ebx = ebx.wrapping_add(1);
                fa = edi;
                fb = ((((0x64bc98i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x279f0; continue; }
                pc = 0x27a0f; continue;
            }
            0x27a0f => {
                ecx = 0;
                m.write32(esp.wrapping_add(0x10), ecx);
                m.write32(esp.wrapping_add(0x14), ecx);
                m.write32(esp.wrapping_add(0x18), ecx);
                m.write32(esp.wrapping_add(0x1c), ecx);
                m.write32(esp.wrapping_add(0x20), ecx);
                write16(m, esp.wrapping_add(0x12), (0x1) as u16);
                m.write32(esp.wrapping_add(0x14), 0xabe0);
                write16(m, esp.wrapping_add(0x1e), (0x4) as u16);
                write16(m, esp.wrapping_add(0x10), (0x69) as u16);
                write16(m, esp.wrapping_add(0x20), (0x2) as u16);
                write16(m, esp.wrapping_add(0x22), (0x40) as u16);
                write16(m, esp.wrapping_add(0x1c), (0x24) as u16);
                m.write32(esp.wrapping_add(0x18), 0x60ae);
                ebx = 0;
                ebp = 0x18;
                pc = 0x27a70; continue;
            }
            0x27a70 => {
                ecx = m.read32(0x581988);
                push!(esi);
                edi = ebx.wrapping_add(0x5835f0);
                push!(edi);
                eax = esp.wrapping_add(0x2c);
                push!(eax);
                edx = esp.wrapping_add(0x1c);
                push!(ecx);
                m.write32(esp.wrapping_add(0x34), ebp);
                m.write32(esp.wrapping_add(0x38), 0x10);
                m.write32(esp.wrapping_add(0x3c), esi);
                m.write32(esp.wrapping_add(0x40), edx);
                m.write32(esp.wrapping_add(0x44), esi);
                m.write32(esp.wrapping_add(0x48), esi);
                crate::audio_lane::set_return(0x27aa9);
                call_to!(0x004093C8);
                pc = 0x27aa9; continue;
            }
            0x27aa9 => {
                xmm0 = Xmm::default();
                eax = m.read32(edi);
                push!(esi);
                edx = esp.wrapping_add(0x40);
                push!(edx);
                push!(eax);
                m.write32(esp.wrapping_add(0x48), esi);
                m.write32(esp.wrapping_add(0x4c), esi);
                m.write32(esp.wrapping_add(0x50), 0xfffff448u32);
                m.write32(esp.wrapping_add(0x54), esi);
                m.write32(esp.wrapping_add(0x58), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x5c), esi);
                m.write32(esp.wrapping_add(0x60), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x64), esi);
                m.write32(esp.wrapping_add(0x68), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x27ae8);
                call_to!(0x004085AF);
                pc = 0x27ae8; continue;
            }
            0x27ae8 => {
                ecx = m.read32(edi);
                push!(esi);
                push!(0x3f800000);
                push!(ecx);
                crate::audio_lane::set_return(0x27af6);
                call_to!(0x00408532);
                pc = 0x27af6; continue;
            }
            0x27af6 => {
                edx = m.read32(edi);
                push!(esi);
                push!(0x1);
                push!(0x64bc98);
                push!(edx);
                crate::audio_lane::set_return(0x27b06);
                call_to!(0x0040858B);
                pc = 0x27b06; continue;
            }
            0x27b06 => {
                m.write32(ebx.wrapping_add(0x565a00), esi);
                ebx = ebx.wrapping_add(0x4);
                fa = ebx;
                fb = ((((0xa0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27a70; continue; }
                pc = 0x27b1b; continue;
            }
            0x27b1b => {
                edi = 0;
                pc = 0x27b20; continue;
            }
            0x27b20 => {
                push!(esi);
                ecx = edi.wrapping_add(0x5818e8);
                eax = esp.wrapping_add(0x14);
                push!(ecx);
                edx = esp.wrapping_add(0x2c);
                m.write32(esp.wrapping_add(0x38), eax);
                eax = m.read32(0x581988);
                push!(edx);
                push!(eax);
                m.write32(esp.wrapping_add(0x34), ebp);
                m.write32(esp.wrapping_add(0x38), esi);
                m.write32(esp.wrapping_add(0x3c), esi);
                m.write32(esp.wrapping_add(0x44), esi);
                m.write32(esp.wrapping_add(0x48), esi);
                crate::audio_lane::set_return(0x27b54);
                call_to!(0x004093C8);
                pc = 0x27b54; continue;
            }
            0x27b54 => {
                m.write32(edi.wrapping_add(0x565aa0), esi);
                edi = edi.wrapping_add(0x4);
                fa = edi;
                fb = ((((0xa0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27b20; continue; }
                pc = 0x27b65; continue;
            }
            0x27b65 => {
                crate::audio_lane::set_return(0x27b6a);
                call!(0x26c30);
                pc = 0x27b6a; continue;
            }
            0x27b6a => {
                ecx = m.read32(0x581988);
                push!(esi);
                push!(esi);
                push!(ecx);
                crate::audio_lane::set_return(0x27b78);
                call_to!(0x004093EC);
                pc = 0x27b78; continue;
            }
            0x27b78 => {
                edx = m.read32(0x581988);
                push!(esi);
                push!(esi);
                push!(esi);
                push!(esi);
                push!(edx);
                crate::audio_lane::set_return(0x27b88);
                call_to!(0x0040945A);
                pc = 0x27b88; continue;
            }
            0x27b88 => {
                eax = m.read32(0x581988);
                push!(esi);
                push!(esi);
                push!(0x3f800000);
                push!(esi);
                push!(0x3f800000);
                push!(esi);
                push!(esi);
                push!(eax);
                crate::audio_lane::set_return(0x27ba2);
                call_to!(0x00409410);
                pc = 0x27ba2; continue;
            }
            0x27ba2 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x50);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00027BB0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27bb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27bb0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x27bb0, previous, pc); previous = pc; }
        match pc {
            0x27bb0 => {
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x8));
                crate::audio_lane::set_return(0x27bba);
                call!(0x26bb0);
                pc = 0x27bba; continue;
            }
            0x27bba => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27bde; continue; }
                pc = 0x27bbe; continue;
            }
            0x27bbe => {
                eax = m.read32(eax);
                ecx = esp;
                push!(ecx);
                push!(eax);
                crate::audio_lane::set_return(0x27bca);
                call_to!(0x00407AF8);
                pc = 0x27bca; continue;
            }
            0x27bca => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x27bda; continue; }
                pc = 0x27bce; continue;
            }
            0x27bce => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(esp)) as u32)) & 0xff);
                eax = 0x1;
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x27bdc; continue; }
                pc = 0x27bda; continue;
            }
            0x27bda => {
                eax = 0;
                pc = 0x27bdc; continue;
            }
            0x27bdc => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x27bde => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27bda; continue; }
                pc = 0x27be2; continue;
            }
            0x27be2 => {
                eax = 0;
                pc = 0x27be4; continue;
            }
            0x27be4 => {
                fa = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x581990));
                fb = ecx;
                if (fa == fb) { pc = 0x27bf7; continue; }
                pc = 0x27bed; continue;
            }
            0x27bed => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27be4; continue; }
                pc = 0x27bf3; continue;
            }
            0x27bf3 => {
                eax = 0;
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x27bf7 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27bda; continue; }
                pc = 0x27bfc; continue;
            }
            0x27bfc => {
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x14094) as i32)) as u32;
                ecx = ecx.wrapping_add(0x5836d0);
                crate::audio_lane::set_return(0x27c0f);
                call!(0x29cc0);
                pc = 0x27c0f; continue;
            }
            0x27c0f => {
                eax = ((((eax) as u8)) as u8 as u32);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00027C20: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27c20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27c20;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x27c20, previous, pc); previous = pc; }
        match pc {
            0x27c20 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                esp = esp.wrapping_sub(0xc);
                crate::audio_lane::set_return(0x27c2c);
                call!(0x26bb0);
                pc = 0x27c2c; continue;
            }
            0x27c2c => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27c7e; continue; }
                pc = 0x27c30; continue;
            }
            0x27c30 => {
                edx = m.read32(esp.wrapping_add(0x14));
                push!(esi);
                esi = m.read32(edx);
                ecx = eax.wrapping_add(0x8);
                m.write32(ecx, esi);
                esi = m.read32(edx.wrapping_add(0x4));
                m.write32(eax.wrapping_add(0xc), esi);
                edx = m.read32(edx.wrapping_add(0x8));
                esi = m.read32(eax);
                m.write32(eax.wrapping_add(0x10), edx);
                push!(ecx);
                eax = esp.wrapping_add(0x8);
                push!(eax);
                crate::audio_lane::set_return(0x27c55);
                call!(0x26d20);
                pc = 0x27c55; continue;
            }
            0x27c55 => {
                xmm0 = Xmm::default();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                edx = m.read32(esp.wrapping_add(0xc));
                esp = esp.wrapping_add(0x8);
                push!(0x1);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x10));
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                push!(edx);
                push!(esi);
                crate::audio_lane::set_return(0x27c79);
                call_to!(0x00408556);
                pc = 0x27c79; continue;
            }
            0x27c79 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            0x27c7e => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27cb4; continue; }
                pc = 0x27c82; continue;
            }
            0x27c82 => {
                eax = 0;
                pc = 0x27c84; continue;
            }
            0x27c84 => {
                fa = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x581990));
                fb = ecx;
                if (fa == fb) { pc = 0x27c97; continue; }
                pc = 0x27c8d; continue;
            }
            0x27c8d => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27c84; continue; }
                pc = 0x27c93; continue;
            }
            0x27c93 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            0x27c97 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27cb4; continue; }
                pc = 0x27c9c; continue;
            }
            0x27c9c => {
                ecx = m.read32(esp.wrapping_add(0x14));
                push!(ecx);
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x14094) as i32)) as u32;
                ecx = ecx.wrapping_add(0x5836d0);
                crate::audio_lane::set_return(0x27cb4);
                call!(0x29dd0);
                pc = 0x27cb4; continue;
            }
            0x27cb4 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00027DA0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27da0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27da0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x27da0, previous, pc); previous = pc; }
        match pc {
            0x27da0 => {
                push!(esi);
                push!(edi);
                esi = 0x5836d0;
                pc = 0x27da7; continue;
            }
            0x27da7 => {
                ecx = esi;
                crate::audio_lane::set_return(0x27dae);
                call!(0x29b40);
                pc = 0x27dae; continue;
            }
            0x27dae => {
                esi = esi.wrapping_add(0x14094);
                fa = esi;
                fb = ((((0x64bc98i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27da7; continue; }
                pc = 0x27dbc; continue;
            }
            0x27dbc => {
                esi = m.read32(0x5659f4);
                edi = 0;
                fa = esi;
                fb = ((((0x5659c4i32)) as u32) & 0xffffffffu32);
                push!(ebx);
                if (fa == fb) { pc = 0x27e80; continue; }
                pc = 0x27dd1; continue;
            }
            0x27dd1 => {
                eax = m.read32(esi);
                fa = eax;
                fb = edi;
                ebx = m.read32(esi.wrapping_add(0x30));
                if (fa == fb) { pc = 0x27e72; continue; }
                pc = 0x27dde; continue;
            }
            0x27dde => {
                push!(eax);
                crate::audio_lane::set_return(0x27de4);
                call_to!(0x00407AA4);
                pc = 0x27de4; continue;
            }
            0x27de4 => {
                ecx = m.read32(esi);
                eax = 0;
                pc = 0x27df0; continue;
            }
            0x27df0 => {
                fa = ecx;
                fb = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x5835f0));
                if (fa == fb) { pc = 0x27e12; continue; }
                pc = 0x27df9; continue;
            }
            0x27df9 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x28i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27df0; continue; }
                pc = 0x27dff; continue;
            }
            0x27dff => {
                eax = 0;
                pc = 0x27e01; continue;
            }
            0x27e01 => {
                fa = ecx;
                fb = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x5818e8));
                if (fa == fb) { pc = 0x27e1b; continue; }
                pc = 0x27e0a; continue;
            }
            0x27e0a => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x28i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27e01; continue; }
                pc = 0x27e10; continue;
            }
            0x27e10 => {
                pc = 0x27e22; continue;
            }
            0x27e12 => {
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x565a00), edi);
                pc = 0x27e22; continue;
            }
            0x27e1b => {
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x565aa0), edi);
                pc = 0x27e22; continue;
            }
            0x27e22 => {
                eax = m.read32(esi.wrapping_add(0x34));
                ecx = m.read32(esi.wrapping_add(0x30));
                m.write32(eax.wrapping_add(0x30), ecx);
                edx = m.read32(esi.wrapping_add(0x30));
                eax = m.read32(esi.wrapping_add(0x34));
                m.write32(edx.wrapping_add(0x34), eax);
                m.write32(esi.wrapping_add(0x34), edi);
                m.write32(esi.wrapping_add(0x30), edi);
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
                m.write32(ecx.wrapping_add(0x5819b8), edi);
                ecx = m.read32(0x565b40);
                m.write32(ecx.wrapping_mul(0x4).wrapping_add(0x5657c0), eax);
                ecx = ecx.wrapping_add(1);
                m.write32(0x565b40, ecx);
                pc = 0x27e72; continue;
            }
            0x27e72 => {
                fa = ebx;
                fb = ((((0x5659c4i32)) as u32) & 0xffffffffu32);
                esi = ebx;
                if (fa != fb) { pc = 0x27dd1; continue; }
                pc = 0x27e80; continue;
            }
            0x27e80 => {
                esi = m.read32(0x5836c0);
                fa = esi;
                fb = ((((0x583690i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x27f40; continue; }
                pc = 0x27e92; continue;
            }
            0x27e92 => {
                eax = m.read32(esi);
                fa = eax;
                fb = edi;
                ebx = m.read32(esi.wrapping_add(0x30));
                if (fa == fb) { pc = 0x27f32; continue; }
                pc = 0x27e9f; continue;
            }
            0x27e9f => {
                push!(eax);
                crate::audio_lane::set_return(0x27ea5);
                call_to!(0x00407AA4);
                pc = 0x27ea5; continue;
            }
            0x27ea5 => {
                ecx = m.read32(esi);
                eax = 0;
                pc = 0x27eb0; continue;
            }
            0x27eb0 => {
                fa = ecx;
                fb = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x5835f0));
                if (fa == fb) { pc = 0x27ed2; continue; }
                pc = 0x27eb9; continue;
            }
            0x27eb9 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x28i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27eb0; continue; }
                pc = 0x27ebf; continue;
            }
            0x27ebf => {
                eax = 0;
                pc = 0x27ec1; continue;
            }
            0x27ec1 => {
                fa = ecx;
                fb = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x5818e8));
                if (fa == fb) { pc = 0x27edb; continue; }
                pc = 0x27eca; continue;
            }
            0x27eca => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x28i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27ec1; continue; }
                pc = 0x27ed0; continue;
            }
            0x27ed0 => {
                pc = 0x27ee2; continue;
            }
            0x27ed2 => {
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x565a00), edi);
                pc = 0x27ee2; continue;
            }
            0x27edb => {
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x565aa0), edi);
                pc = 0x27ee2; continue;
            }
            0x27ee2 => {
                edx = m.read32(esi.wrapping_add(0x34));
                eax = m.read32(esi.wrapping_add(0x30));
                m.write32(edx.wrapping_add(0x30), eax);
                ecx = m.read32(esi.wrapping_add(0x30));
                edx = m.read32(esi.wrapping_add(0x34));
                m.write32(ecx.wrapping_add(0x34), edx);
                m.write32(esi.wrapping_add(0x34), edi);
                m.write32(esi.wrapping_add(0x30), edi);
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
                m.write32(ecx.wrapping_add(0x5819b8), edi);
                ecx = m.read32(0x565b40);
                m.write32(ecx.wrapping_mul(0x4).wrapping_add(0x5657c0), eax);
                ecx = ecx.wrapping_add(1);
                m.write32(0x565b40, ecx);
                pc = 0x27f32; continue;
            }
            0x27f32 => {
                fa = ebx;
                fb = ((((0x583690i32)) as u32) & 0xffffffffu32);
                esi = ebx;
                if (fa != fb) { pc = 0x27e92; continue; }
                pc = 0x27f40; continue;
            }
            0x27f40 => {
                esi = 0x5836d0;
                edi = (edi | 0xffffffffu32);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x27f50; continue;
            }
            0x27f50 => {
                fa = m.read32(esi.wrapping_add(0x4c));
                fb = edi;
                if (fa == fb) { pc = 0x27f5c; continue; }
                pc = 0x27f55; continue;
            }
            0x27f55 => {
                ecx = esi;
                crate::audio_lane::set_return(0x27f5c);
                call!(0x29b40);
                pc = 0x27f5c; continue;
            }
            0x27f5c => {
                esi = esi.wrapping_add(0x14094);
                fa = esi;
                fb = ((((0x64bc98i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x27f50; continue; }
                pc = 0x27f6a; continue;
            }
            0x27f6a => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00027F70: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27f70<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27f70;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x27f70, previous, pc); previous = pc; }
        match pc {
            0x27f70 => {
                tail_to!(0x27da0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00027FA0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27fa0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27fa0;
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
        { crate::pose::cover(0x27fa0, previous, pc); previous = pc; }
        match pc {
            0x27fa0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(0x4ca9b4);
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                edx = ((m.read8(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x4))) as u8 as u32);
                fpu.push((m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4))) as i32 as f64));
                eax = ecx.wrapping_add(eax.wrapping_mul(0x4));
                ecx = ((m.read16(edx.wrapping_mul(0x2).wrapping_add(0x4ca9b8))) as u16 as u32);
                eax = m.read32(eax.wrapping_add(0x8));
                ecx = ((ecx as i32).wrapping_mul((0xbb80) as i32)) as u32;
                eax = (eax & 0x200);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & 0x20);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4761d0))).to_bits();
                eax = eax.wrapping_add(0x24);
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                edx = ((edx as i32).wrapping_mul((eax) as i32)) as u32;
                m.write32(esp.wrapping_add(0x4), edx);
                fpu.push((m.read32(esp.wrapping_add(0x4)) as i32 as f64));
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x476218)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(1) / fpu.st(0), fpu.control); fpu.stack[((fpu.top + 1) & 7) as usize] = value; } fpu.pop();
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00028000: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28000<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28000;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28000, previous, pc); previous = pc; }
        match pc {
            0x28000 => {
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x8));
                crate::audio_lane::set_return(0x2800a);
                call!(0x26bb0);
                pc = 0x2800a; continue;
            }
            0x2800a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2802e; continue; }
                pc = 0x2800e; continue;
            }
            0x2800e => {
                eax = m.read32(eax);
                ecx = esp;
                push!(ecx);
                push!(eax);
                crate::audio_lane::set_return(0x2801a);
                call_to!(0x00407AF8);
                pc = 0x2801a; continue;
            }
            0x2801a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2802a; continue; }
                pc = 0x2801e; continue;
            }
            0x2801e => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(esp)) as u32)) & 0xff);
                eax = 0x1;
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x2802c; continue; }
                pc = 0x2802a; continue;
            }
            0x2802a => {
                eax = 0;
                pc = 0x2802c; continue;
            }
            0x2802c => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2802e => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2802a; continue; }
                pc = 0x28032; continue;
            }
            0x28032 => {
                eax = 0;
                pc = 0x28034; continue;
            }
            0x28034 => {
                fa = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x581990));
                fb = ecx;
                if (fa == fb) { pc = 0x28047; continue; }
                pc = 0x2803d; continue;
            }
            0x2803d => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x28034; continue; }
                pc = 0x28043; continue;
            }
            0x28043 => {
                eax = 0;
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x28047 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2802a; continue; }
                pc = 0x2804c; continue;
            }
            0x2804c => {
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x14094) as i32)) as u32;
                ecx = ecx.wrapping_add(0x5836d0);
                crate::audio_lane::set_return(0x2805f);
                call!(0x29d10);
                pc = 0x2805f; continue;
            }
            0x2805f => {
                eax = ((((eax) as u8)) as u8 as u32);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00028070: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28070<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28070;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28070, previous, pc); previous = pc; }
        match pc {
            0x28070 => {
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x8));
                crate::audio_lane::set_return(0x2807a);
                call!(0x26bb0);
                pc = 0x2807a; continue;
            }
            0x2807a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x280b7; continue; }
                pc = 0x2807e; continue;
            }
            0x2807e => {
                ecx = m.read32(eax.wrapping_add(0x1c));
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                xmm1 = Xmm::scalar(m.read32(0x475d20));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                if ((fa & fb) == 0) { pc = 0x28098; continue; }
                pc = 0x28094; continue;
            }
            0x28094 => {
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                pc = 0x28098; continue;
            }
            0x28098 => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp, xmm1.lanes[0]);
                if ((fca > fcb)) { pc = 0x280a7; continue; }
                pc = 0x280a2; continue;
            }
            0x280a2 => {
                m.write32(esp, xmm0.lanes[0]);
                pc = 0x280a7; continue;
            }
            0x280a7 => {
                ecx = m.read32(esp);
                edx = m.read32(eax);
                push!(0x1);
                push!(ecx);
                push!(edx);
                crate::audio_lane::set_return(0x280b5);
                call_to!(0x0040850E);
                pc = 0x280b5; continue;
            }
            0x280b5 => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x280b7 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2810b; continue; }
                pc = 0x280bb; continue;
            }
            0x280bb => {
                eax = 0;
                pc = 0x280c0; continue;
            }
            0x280c0 => {
                fa = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x581990));
                fb = ecx;
                if (fa == fb) { pc = 0x280d1; continue; }
                pc = 0x280c9; continue;
            }
            0x280c9 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x280c0; continue; }
                pc = 0x280cf; continue;
            }
            0x280cf => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x280d1 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2810b; continue; }
                pc = 0x280d6; continue;
            }
            0x280d6 => {
                eax = ((eax as i32).wrapping_mul((0x14094) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x583738));
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x280fb; continue; }
                pc = 0x280e7; continue;
            }
            0x280e7 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d20))).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                pc = 0x280fb; continue;
            }
            0x280fb => {
                ecx = m.read32(esp.wrapping_add(0xc));
                push!(ecx);
                ecx = eax.wrapping_add(0x5836d0);
                crate::audio_lane::set_return(0x2810b);
                call!(0x29d50);
                pc = 0x2810b; continue;
            }
            0x2810b => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00028610: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28610<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28610;
    let mut jt = 0u32;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28610, previous, pc); previous = pc; }
        match pc {
            0x28610 => {
                push!(ecx);
                push!(esi);
                esi = eax;
                eax = m.read32(esi.wrapping_add(0x1c));
                ecx = m.read32(esi.wrapping_add(0x28));
                edx = m.read32(esi.wrapping_add(0x24));
                push!(eax);
                eax = m.read32(esi.wrapping_add(0x20));
                push!(ecx);
                ecx = m.read32(esi.wrapping_add(0x18));
                push!(edx);
                push!(eax);
                push!(ecx);
                crate::audio_lane::set_return(0x2862d);
                call!(0x28180);
                pc = 0x2862d; continue;
            }
            0x2862d => {
                ecx = m.read32(esi.wrapping_add(0x2c));
                esp = esp.wrapping_add(0x14);
                fa = eax;
                fb = ecx;
                if (fa == fb) { pc = 0x28643; continue; }
                pc = 0x28637; continue;
            }
            0x28637 => {
                edx = m.read32(esi);
                push!(eax);
                push!(edx);
                m.write32(esi.wrapping_add(0x2c), eax);
                crate::audio_lane::set_return(0x28643);
                call_to!(0x00407A64);
                pc = 0x28643; continue;
            }
            0x28643 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x17))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x28743; continue; }
                pc = 0x2864e; continue;
            }
            0x2864e => {
                eax = ((m.read8(esi.wrapping_add(0x15))) as u8 as u32);
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x28743; continue; }
                pc = 0x2865b; continue;
            }
            0x2865b => {
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x28748));
                if (jt == 0x28662u32) { pc = 0x28662; continue; }
                if (jt == 0x28672u32) { pc = 0x28672; continue; }
                if (jt == 0x286a3u32) { pc = 0x286a3; continue; }
                if (jt == 0x286b3u32) { pc = 0x286b3; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x28662 => {
                m.write8(esi.wrapping_add(0x16), (((((m.read8(esi.wrapping_add(0x16))) as i32).wrapping_sub(0x1i32)) as u32)) as u8);
                fa = ((((m.read8(esi.wrapping_add(0x16)))) as u32) & 0xffu32);
                if ((fa != 0)) { pc = 0x28743; continue; }
                pc = 0x2866b; continue;
            }
            0x2866b => {
                m.write8(esi.wrapping_add(0x15), (0x1) as u8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x28672 => {
                eax = m.read32(esi.wrapping_add(0x18));
                ecx = m.read32(0x4ca9b4);
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                edx = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x8));
                eax = m.read32(esi);
                edx = (edx & 0x10);
                edx = (edx | 0x20);
                edx = edx.wrapping_shr((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(edx);
                push!(0);
                push!(0);
                push!(eax);
                crate::audio_lane::set_return(0x28698);
                call_to!(0x00407A80);
                pc = 0x28698; continue;
            }
            0x28698 => {
                m.write8(esi.wrapping_add(0x15), (0x3) as u8);
                m.write8(esi.wrapping_add(0x16), (0x2) as u8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x286a3 => {
                m.write8(esi.wrapping_add(0x16), (((((m.read8(esi.wrapping_add(0x16))) as i32).wrapping_sub(0x1i32)) as u32)) as u8);
                fa = ((((m.read8(esi.wrapping_add(0x16)))) as u32) & 0xffu32);
                if ((fa != 0)) { pc = 0x28743; continue; }
                pc = 0x286ac; continue;
            }
            0x286ac => {
                m.write8(esi.wrapping_add(0x15), (0x2) as u8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x286b3 => {
                eax = m.read32(esi);
                ecx = esp.wrapping_add(0x4);
                push!(ecx);
                push!(eax);
                crate::audio_lane::set_return(0x286c0);
                call_to!(0x00407AF8);
                pc = 0x286c0; continue;
            }
            0x286c0 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x28743; continue; }
                pc = 0x286c4; continue;
            }
            0x286c4 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esp.wrapping_add(0x4))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x3i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x28714; continue; }
                pc = 0x286cc; continue;
            }
            0x286cc => {
                ecx = m.read32(esi);
                crate::audio_lane::set_return(0x286d3);
                call!(0x26b70);
                pc = 0x286d3; continue;
            }
            0x286d3 => {
                edx = m.read32(esi.wrapping_add(0x34));
                eax = m.read32(esi.wrapping_add(0x30));
                m.write32(edx.wrapping_add(0x30), eax);
                edx = m.read32(esi.wrapping_add(0x34));
                ecx = m.read32(esi.wrapping_add(0x30));
                m.write32(ecx.wrapping_add(0x34), edx);
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
                crate::audio_lane::set_return(0x28711);
                call!(0x26c00);
                pc = 0x28711; continue;
            }
            0x28711 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x28714 => {
                ecx = m.read32(esp.wrapping_add(0xc));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28743; continue; }
                pc = 0x2871c; continue;
            }
            0x2871c => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x28735; continue; }
                pc = 0x28720; continue;
            }
            0x28720 => {
                eax = m.read32(esi.wrapping_add(0x18));
                ecx = m.read32(0x4ca9b4);
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                edx = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x8));
                fa = (((((((edx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x28743; continue; }
                pc = 0x28735; continue;
            }
            0x28735 => {
                edx = m.read32(esi);
                push!(edx);
                eax = esi.wrapping_add(0x8);
                crate::audio_lane::set_return(0x28740);
                call!(0x26ec0);
                pc = 0x28740; continue;
            }
            0x28740 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x28743; continue;
            }
            0x28743 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00028890: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28890<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28890;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28890, previous, pc); previous = pc; }
        match pc {
            0x28890 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0xc));
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x288dd; continue; }
                pc = 0x28899; continue;
            }
            0x28899 => {
                push!(esi);
                esi = m.read32(ebx.wrapping_mul(0x4).wrapping_add(0x535530));
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x10));
                push!(0x25);
                push!(0x476244);
                push!(0);
                push!(esi);
                crate::audio_lane::set_return(0x288b5);
                call!(0x62e20);
                pc = 0x288b5; continue;
            }
            0x288b5 => {
                push!(esi);
                m.write32(edi.wrapping_mul(0x4).wrapping_add(0x64bd40), eax);
                crate::audio_lane::set_return(0x288c2);
                call!(0x62ca0);
                pc = 0x288c2; continue;
            }
            0x288c2 => {
                push!(ebx);
                m.write32(edi.wrapping_mul(0x4).wrapping_add(0x64bca0), eax);
                crate::audio_lane::set_return(0x288cf);
                call!(0x350c40);
                pc = 0x288cf; continue;
            }
            0x288cf => {
                esp = esp.wrapping_add(0x18);
                m.write32(edi.wrapping_mul(0x4).wrapping_add(0x64bcf0), eax);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x288dd => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = (ecx | 0xffffffffu32);
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x64bd40), ecx);
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x64bca0), ecx);
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x64bcf0), 0xac44);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00028900: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28900<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28900;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28900, previous, pc); previous = pc; }
        match pc {
            0x28900 => {
                eax = (eax | 0xffffffffu32);
                m.write32(0x64bd40, eax);
                m.write32(0x64bd44, eax);
                m.write32(0x64bd48, eax);
                m.write32(0x64bd4c, eax);
                m.write32(0x64bd50, eax);
                m.write32(0x64bd54, eax);
                m.write32(0x64bd58, eax);
                m.write32(0x64bd5c, eax);
                m.write32(0x64bd60, eax);
                m.write32(0x64bd64, eax);
                m.write32(0x64bd68, eax);
                m.write32(0x64bd6c, eax);
                m.write32(0x64bd70, eax);
                m.write32(0x64bd74, eax);
                m.write32(0x64bd78, eax);
                m.write32(0x64bd7c, eax);
                m.write32(0x64bd80, eax);
                m.write32(0x64bd84, eax);
                m.write32(0x64bd88, eax);
                push!(0);
                ecx = 0x64bd98;
                m.write32(0x64bd8c, eax);
                crate::audio_lane::set_return(0x28973);
                call!(0x298d0);
                pc = 0x28973; continue;
            }
            0x28973 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00028980: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28980<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28980;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28980, previous, pc); previous = pc; }
        match pc {
            0x28980 => {
                eax = m.read32(0x4b9134);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x289ae; continue; }
                pc = 0x28989; continue;
            }
            0x28989 => {
                push!(eax);
                crate::audio_lane::set_return(0x2898f);
                call!(0x45b60);
                pc = 0x2898f; continue;
            }
            0x2898f => {
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x64bd90))).to_bits();
                esp = esp.wrapping_add(0x4);
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                push!(eax);
                ecx = 0x64bd98;
                crate::audio_lane::set_return(0x289ae);
                call!(0x29af0);
                pc = 0x289ae; continue;
            }
            0x289ae => {
                ecx = 0x64bd98;
                tail_to!(0x29f30); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000289C0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f289c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x289c0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x289c0, previous, pc); previous = pc; }
        match pc {
            0x289c0 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                crate::audio_lane::set_return(0x289cb);
                call!(0x45b60);
                pc = 0x289cb; continue;
            }
            0x289cb => {
                ecx = m.read32(esi.wrapping_mul(0x4).wrapping_add(0x64bcf0));
                edx = m.read32(esp.wrapping_add(0x10));
                esp = esp.wrapping_add(0x4);
                push!(0);
                push!(0);
                push!(ecx);
                push!(edx);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d1c))).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                push!(0);
                push!(ecx);
                ecx = m.read32(esi.wrapping_mul(0x4).wrapping_add(0x64bd40));
                m.write32(0x64bd90, xmm0.lanes[0]);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                eax = m.read32(esi.wrapping_mul(0x4).wrapping_add(0x64bca0));
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                push!(ecx);
                push!(0xffffffffu32);
                ecx = 0x64bd98;
                m.write32(0x4b9134, esi);
                crate::audio_lane::set_return(0x28a2d);
                call!(0x299c0);
                pc = 0x28a2d; continue;
            }
            0x28a2d => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00028A30: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28a30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28a30;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28a30, previous, pc); previous = pc; }
        match pc {
            0x28a30 => {
                ecx = 0x64bd98;
                tail_to!(0x29b40); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00028A40: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28a40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28a40;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28a40, previous, pc); previous = pc; }
        match pc {
            0x28a40 => {
                ecx = 0x64bd98;
                tail_to!(0x29ea0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00028A50: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28a50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28a50;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28a50, previous, pc); previous = pc; }
        match pc {
            0x28a50 => {
                ecx = 0x64bd98;
                tail_to!(0x29ee0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00028A60: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28a60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28a60;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28a60, previous, pc); previous = pc; }
        match pc {
            0x28a60 => {
                ecx = 0x64bd98;
                crate::audio_lane::set_return(0x28a6a);
                call!(0x29d10);
                pc = 0x28a6a; continue;
            }
            0x28a6a => {
                cf = (((((((((eax) as u8))) as i32) != 0i32))) as i32);
                eax = (eax & 0xffff_ff00) | (((((((((eax) as u8)) as i32).wrapping_neg())) as u32)) & 0xff);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00028A80: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28a80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28a80;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28a80, previous, pc); previous = pc; }
        match pc {
            0x28a80 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d1c))).to_bits();
                m.write32(0x64bd90, xmm0.lanes[0]);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00028AA0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28aa0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28aa0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28aa0, previous, pc); previous = pc; }
        match pc {
            0x28aa0 => {
                eax = m.read32(0x6b7aa8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x28ad2; continue; }
                pc = 0x28aa9; continue;
            }
            0x28aa9 => {
                push!(esi);
                esi = 0x64bd40;
                pc = 0x28ab0; continue;
            }
            0x28ab0 => {
                eax = m.read32(esi);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28ac6; continue; }
                pc = 0x28ab7; continue;
            }
            0x28ab7 => {
                push!(eax);
                crate::audio_lane::set_return(0x28abd);
                call!(0x29400);
                pc = 0x28abd; continue;
            }
            0x28abd => {
                esp = esp.wrapping_add(0x4);
                m.write32(esi, 0xffffffffu32);
                pc = 0x28ac6; continue;
            }
            0x28ac6 => {
                esi = esi.wrapping_add(0x4);
                fa = esi;
                fb = ((((0x64bd90i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x28ab0; continue; }
                pc = 0x28ad1; continue;
            }
            0x28ad1 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x28ad2; continue;
            }
            0x28ad2 => {
                ecx = 0x64bd98;
                tail_to!(0x29b40); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00028AE0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f28ae0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28ae0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x28ae0, previous, pc); previous = pc; }
        match pc {
            0x28ae0 => {
                ecx = 0x64bd98;
                tail_to!(0x298b0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000298B0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f298b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x298b0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x298b0, previous, pc); previous = pc; }
        match pc {
            0x298b0 => {
                push!(esi);
                push!(0x1);
                push!(0x36000);
                push!(0x10);
                esi = ecx;
                crate::audio_lane::set_return(0x298c1);
                call!(0x3e0b0);
                pc = 0x298c1; continue;
            }
            0x298c1 => {
                esp = esp.wrapping_add(0xc);
                m.write32(esi.wrapping_add(0x8), eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00029B40: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29b40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29b40;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29b40, previous, pc); previous = pc; }
        match pc {
            0x29b40 => {
                push!(esi);
                esi = ecx;
                eax = m.read32(esi.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29ba6; continue; }
                pc = 0x29b4a; continue;
            }
            0x29b4a => {
                xmm0 = Xmm::default();
                push!(0x1);
                push!(eax);
                m.write32(esi.wrapping_add(0x54), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x29b5a);
                call_to!(0x00407B23);
                pc = 0x29b5a; continue;
            }
            0x29b5a => {
                eax = m.read32(esi.wrapping_add(0x4));
                push!(0x1);
                push!(0);
                push!(0);
                push!(eax);
                crate::audio_lane::set_return(0x29b69);
                call_to!(0x00407B28);
                pc = 0x29b69; continue;
            }
            0x29b69 => {
                eax = m.read32(esi.wrapping_add(0x4));
                ecx = m.read32(eax);
                push!(eax);
                crate::audio_lane::set_return(0x29b72);
                call_to!(m.read32(ecx.wrapping_add(0x14)));
                pc = 0x29b72; continue;
            }
            0x29b72 => {
                eax = m.read32(esi.wrapping_add(0x38));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                m.write32(esi.wrapping_add(0x2c), 0);
                m.write32(esi.wrapping_add(0x28), 0x3);
                m.write32(esi.wrapping_add(0x4c), 0xffffffffu32);
                if (fa == fb) { pc = 0x29ba6; continue; }
                pc = 0x29b8f; continue;
            }
            0x29b8f => {
                ecx = m.read32(esi.wrapping_add(0x48));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29b9f; continue; }
                pc = 0x29b96; continue;
            }
            0x29b96 => {
                push!(eax);
                crate::audio_lane::set_return(0x29b9c);
                call!(0x29400);
                pc = 0x29b9c; continue;
            }
            0x29b9c => {
                esp = esp.wrapping_add(0x4);
                pc = 0x29b9f; continue;
            }
            0x29b9f => {
                m.write32(esi.wrapping_add(0x38), 0xffffffffu32);
                pc = 0x29ba6; continue;
            }
            0x29ba6 => {
                eax = 0;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00029C90: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29c90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29c90;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29c90, previous, pc); previous = pc; }
        match pc {
            0x29c90 => {
                eax = m.read32(ecx.wrapping_add(0x4));
                m.write32(ecx.wrapping_add(0x2c), 0x1);
                edx = m.read32(eax);
                push!(0);
                ecx = ecx.wrapping_add(0x78);
                push!(ecx);
                push!(eax);
                crate::audio_lane::set_return(0x29ca6);
                call_to!(m.read32(edx.wrapping_add(0x10)));
                pc = 0x29ca6; continue;
            }
            0x29ca6 => {
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fas >= fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ecx = ecx.wrapping_sub(1);
                eax = (eax & ecx);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00029CC0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29cc0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29cc0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29cc0, previous, pc); previous = pc; }
        match pc {
            0x29cc0 => {
                push!(ecx);
                eax = m.read32(ecx.wrapping_add(0x4));
                push!(ebx);
                ebx = (ebx & 0xffff_ff00) | ((0) & 0xff);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29d07; continue; }
                pc = 0x29ccb; continue;
            }
            0x29ccb => {
                eax = m.read32(ecx.wrapping_add(0x28));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29d02; continue; }
                pc = 0x29cd2; continue;
            }
            0x29cd2 => {
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29d02; continue; }
                pc = 0x29cd7; continue;
            }
            0x29cd7 => {
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29d02; continue; }
                pc = 0x29cdc; continue;
            }
            0x29cdc => {
                ecx = m.read32(ecx.wrapping_add(0x4));
                eax = m.read32(ecx);
                edx = esp.wrapping_add(0x4);
                push!(edx);
                push!(ecx);
                crate::audio_lane::set_return(0x29cea);
                call_to!(m.read32(eax.wrapping_add(0xc)));
                pc = 0x29cea; continue;
            }
            0x29cea => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x29d07; continue; }
                pc = 0x29cee; continue;
            }
            0x29cee => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = (eax & 0x30000);
                fa = eax;
                fb = ((((0x10000i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x29d02 => {
                eax = (eax & 0xffff_ff00) | ((0x1) & 0xff);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x29d07 => {
                eax = (eax & 0xffff_ff00) | ((((((ebx) as u8)) as u32)) & 0xff);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00029D10: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29d10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29d10;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29d10, previous, pc); previous = pc; }
        match pc {
            0x29d10 => {
                push!(ecx);
                eax = m.read32(ecx.wrapping_add(0x4));
                push!(ebx);
                ebx = (ebx & 0xffff_ff00) | ((0) & 0xff);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29d40; continue; }
                pc = 0x29d1b; continue;
            }
            0x29d1b => {
                edx = esp.wrapping_add(0x4);
                ecx = eax;
                eax = m.read32(ecx);
                push!(edx);
                push!(ecx);
                crate::audio_lane::set_return(0x29d28);
                call_to!(m.read32(eax.wrapping_add(0xc)));
                pc = 0x29d28; continue;
            }
            0x29d28 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x29d40; continue; }
                pc = 0x29d2c; continue;
            }
            0x29d2c => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = (eax & 0x30000);
                fa = eax;
                fb = ((((0x10000i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x29d40 => {
                eax = (eax & 0xffff_ff00) | ((((((ebx) as u8)) as u32)) & 0xff);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00029E60: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29e60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29e60;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29e60, previous, pc); previous = pc; }
        match pc {
            0x29e60 => {
                eax = m.read32(ecx.wrapping_add(0x50));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29e90; continue; }
                pc = 0x29e67; continue;
            }
            0x29e67 => {
                eax = m.read32(ecx.wrapping_add(0x68));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                push!(0);
                push!(0x4);
                if ((fa & fb) == 0) { pc = 0x29e82; continue; }
                pc = 0x29e73; continue;
            }
            0x29e73 => {
                eax = m.read32(ecx.wrapping_add(0x4));
                push!(0x4b916c);
                push!(eax);
                crate::audio_lane::set_return(0x29e81);
                call_to!(0x00408632);
                pc = 0x29e81; continue;
            }
            0x29e81 => {
                break 'run;
            }
            0x29e82 => {
                ecx = m.read32(ecx.wrapping_add(0x4));
                push!(0x4b915c);
                push!(ecx);
                crate::audio_lane::set_return(0x29e90);
                call_to!(0x00408632);
                pc = 0x29e90; continue;
            }
            0x29e90 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00029EA0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29ea0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29ea0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29ea0, previous, pc); previous = pc; }
        match pc {
            0x29ea0 => {
                push!(esi);
                esi = ecx;
                eax = m.read32(esi.wrapping_add(0x28));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x29eb1; continue; }
                pc = 0x29eaa; continue;
            }
            0x29eaa => {
                m.write32(esi.wrapping_add(0x28), 0x1);
                pc = 0x29eb1; continue;
            }
            0x29eb1 => {
                fa = m.read32(esi.wrapping_add(0x28));
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29ec2; continue; }
                pc = 0x29eb7; continue;
            }
            0x29eb7 => {
                ecx = esi;
                crate::audio_lane::set_return(0x29ebe);
                call!(0x29cc0);
                pc = 0x29ebe; continue;
            }
            0x29ebe => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x29ed1; continue; }
                pc = 0x29ec2; continue;
            }
            0x29ec2 => {
                esi = m.read32(esi.wrapping_add(0x4));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29ed1; continue; }
                pc = 0x29ec9; continue;
            }
            0x29ec9 => {
                push!(0x1);
                push!(esi);
                crate::audio_lane::set_return(0x29ed1);
                call_to!(0x00407B23);
                pc = 0x29ed1; continue;
            }
            0x29ed1 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00029EE0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f29ee0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29ee0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x29ee0, previous, pc); previous = pc; }
        match pc {
            0x29ee0 => {
                push!(esi);
                esi = ecx;
                fa = m.read32(esi.wrapping_add(0x28));
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x29ef9; continue; }
                pc = 0x29ee9; continue;
            }
            0x29ee9 => {
                m.write32(esi.wrapping_add(0x28), 0);
                m.write32(esi.wrapping_add(0x24), 0);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x29ef9 => {
                ecx = esi;
                crate::audio_lane::set_return(0x29f00);
                call!(0x29cc0);
                pc = 0x29f00; continue;
            }
            0x29f00 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x29f1a; continue; }
                pc = 0x29f04; continue;
            }
            0x29f04 => {
                eax = m.read32(esi.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29f1a; continue; }
                pc = 0x29f0b; continue;
            }
            0x29f0b => {
                push!(0);
                push!(eax);
                crate::audio_lane::set_return(0x29f13);
                call_to!(0x00407B23);
                pc = 0x29f13; continue;
            }
            0x29f13 => {
                m.write32(esi.wrapping_add(0x28), 0x2);
                pc = 0x29f1a; continue;
            }
            0x29f1a => {
                m.write32(esi.wrapping_add(0x24), 0);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00043B70: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f43b70<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x43b70;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x43b70, previous, pc); previous = pc; }
        match pc {
            0x43b70 => {
                ecx = m.read32(0x6b990c);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(esi);
                esi = m.read32(0x6b9908);
                if (fas <= fbs) { pc = 0x43b8d; continue; }
                pc = 0x43b81; continue;
            }
            0x43b81 => {
                eax = esi.wrapping_add(0x60);
                pc = 0x43b84; continue;
            }
            0x43b84 => {
                m.write32(eax, (m.read32(eax) & 0xfffffffdu32));
                eax = eax.wrapping_add(0x64);
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x43b84; continue; }
                pc = 0x43b8d; continue;
            }
            0x43b8d => {
                eax = m.read32(0x6b991c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x43bc2; continue; }
                pc = 0x43b96; continue;
            }
            0x43b96 => {
                ecx = m.read32(0x6b98bc);
                ecx = ecx.wrapping_add(0x2);
                edx = eax;
                push!(edi);
                pc = 0x43ba2; continue;
            }
            0x43ba2 => {
                fa = ((((m.read8(ecx.wrapping_add(0x2)))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x43bbb; continue; }
                pc = 0x43ba8; continue;
            }
            0x43ba8 => {
                eax = (((((m.read16(ecx) as i16)) as i32)) as u32);
                eax = ((eax as i32).wrapping_mul((0x64) as i32)) as u32;
                edi = m.read32(eax.wrapping_add(esi).wrapping_add(0x60));
                eax = eax.wrapping_add(esi).wrapping_add(0x60);
                edi = (edi | 0x2);
                m.write32(eax, edi);
                pc = 0x43bbb; continue;
            }
            0x43bbb => {
                ecx = ecx.wrapping_add(0x8);
                edx = edx.wrapping_sub(1);
                fa = edx;
                if ((fa != 0)) { pc = 0x43ba2; continue; }
                pc = 0x43bc1; continue;
            }
            0x43bc1 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x43bc2; continue;
            }
            0x43bc2 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00043BD0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f43bd0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x43bd0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x43bd0, previous, pc); previous = pc; }
        match pc {
            0x43bd0 => {
                eax = m.read32(0x6b9914);
                push!(ebx);
                push!(esi);
                esi = eax;
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                edx = 0;
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x43c0a; continue; }
                pc = 0x43be2; continue;
            }
            0x43be2 => {
                ebx = m.read32(0x6b98b8);
                pc = 0x43be8; continue;
            }
            0x43be8 => {
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x2c) as i32)) as u32;
                ecx = m.read32(ecx.wrapping_add(ebx));
                fa = edi;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x43bf8; continue; }
                pc = 0x43bf4; continue;
            }
            0x43bf4 => {
                esi = eax;
                pc = 0x43bfc; continue;
            }
            0x43bf8 => {
                if (fas <= fbs) { pc = 0x43c0d; continue; }
                pc = 0x43bfa; continue;
            }
            0x43bfa => {
                edx = eax;
                pc = 0x43bfc; continue;
            }
            0x43bfc => {
                ecx = eax;
                eax = esi;
                eax = eax.wrapping_sub(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                eax = eax.wrapping_add(edx);
                fa = eax;
                fb = ecx;
                if (fa != fb) { pc = 0x43be8; continue; }
                pc = 0x43c0a; continue;
            }
            0x43c0a => {
                eax = (eax | 0xffffffffu32);
                pc = 0x43c0d; continue;
            }
            0x43c0d => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00043C10: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f43c10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x43c10;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x43c10, previous, pc); previous = pc; }
        match pc {
            0x43c10 => {
                eax = m.read32(0x6b990c);
                push!(ebx);
                push!(esi);
                esi = eax;
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                edx = 0;
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x43c4a; continue; }
                pc = 0x43c22; continue;
            }
            0x43c22 => {
                ebx = m.read32(0x6b9908);
                pc = 0x43c28; continue;
            }
            0x43c28 => {
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x64) as i32)) as u32;
                ecx = m.read32(ecx.wrapping_add(ebx));
                fa = edi;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x43c38; continue; }
                pc = 0x43c34; continue;
            }
            0x43c34 => {
                esi = eax;
                pc = 0x43c3c; continue;
            }
            0x43c38 => {
                if (fas <= fbs) { pc = 0x43c4d; continue; }
                pc = 0x43c3a; continue;
            }
            0x43c3a => {
                edx = eax;
                pc = 0x43c3c; continue;
            }
            0x43c3c => {
                ecx = eax;
                eax = esi;
                eax = eax.wrapping_sub(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                eax = eax.wrapping_add(edx);
                fa = eax;
                fb = ecx;
                if (fa != fb) { pc = 0x43c28; continue; }
                pc = 0x43c4a; continue;
            }
            0x43c4a => {
                eax = (eax | 0xffffffffu32);
                pc = 0x43c4d; continue;
            }
            0x43c4d => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00043C50: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f43c50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x43c50;
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
        { crate::pose::cover(0x43c50, previous, pc); previous = pc; }
        match pc {
            0x43c50 => {
                esp = esp.wrapping_sub(0x124);
                eax = gs::get_level(m);
                push!(ebp);
                push!(esi);
                push!(eax);
                ebp = 0;
                push!(0x4783e0);
                push!(0x6b98c8);
                m.write32(esp.wrapping_add(0x28), ebp);
                m.write32(esp.wrapping_add(0x18), ebp);
                crate::audio_lane::set_return(0x43c77);
                call!(0x3c85d4);
                pc = 0x43c77; continue;
            }
            0x43c77 => {
                push!(0x6b98c8);
                crate::audio_lane::set_return(0x43c81);
                call!(0x62ca0);
                pc = 0x43c81; continue;
            }
            0x43c81 => {
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x8), eax);
                if (fas <= fbs) { pc = 0x43d33; continue; }
                pc = 0x43c90; continue;
            }
            0x43c90 => {
                ecx = gs::get_level(m);
                push!(ecx);
                push!(0x4783e0);
                push!(0x6b98c8);
                crate::audio_lane::set_return(0x43ca6);
                call!(0x3c85d4);
                pc = 0x43ca6; continue;
            }
            0x43ca6 => {
                edx = esp.wrapping_add(0x14);
                push!(edx);
                push!(0x6b98c8);
                crate::audio_lane::set_return(0x43cb5);
                call!(0x62c30);
                pc = 0x43cb5; continue;
            }
            0x43cb5 => {
                esp = esp.wrapping_add(0x14);
                fa = eax;
                fb = ebp;
                if (fa != fb) { pc = 0x43cec; continue; }
                pc = 0x43cbc; continue;
            }
            0x43cbc => {
                eax = gs::get_level(m);
                push!(eax);
                push!(0x4783e0);
                push!(0x6b98c8);
                crate::audio_lane::set_return(0x43cd1);
                call!(0x3c85d4);
                pc = 0x43cd1; continue;
            }
            0x43cd1 => {
                ecx = esp.wrapping_add(0x14);
                push!(ecx);
                push!(ebp);
                push!(0x6b98c8);
                crate::audio_lane::set_return(0x43ce1);
                call!(0x62f00);
                pc = 0x43ce1; continue;
            }
            0x43ce1 => {
                esp = esp.wrapping_add(0x18);
                m.write32(esp.wrapping_add(0x1c), 0x1);
                pc = 0x43cec; continue;
            }
            0x43cec => {
                esi = eax;
                fa = m.read32(esp.wrapping_add(0x8));
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0xc), esi);
                if (fas <= fbs) { pc = 0x43d1c; continue; }
                pc = 0x43cf8; continue;
            }
            0x43cf8 => {
                edx = gs::get_level(m);
                push!(edx);
                push!(0x4783e0);
                push!(0x6b98c8);
                crate::audio_lane::set_return(0x43d0e);
                call!(0x3c85d4);
                pc = 0x43d0e; continue;
            }
            0x43d0e => {
                push!(ebp);
                push!(0x6b98c8);
                crate::audio_lane::set_return(0x43d19);
                call!(0x389b0);
                pc = 0x43d19; continue;
            }
            0x43d19 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x43d1c; continue;
            }
            0x43d1c => {
                eax = m.read32(esp.wrapping_add(0x8));
                push!(eax);
                ecx = esp.wrapping_add(0x24);
                push!(esi);
                push!(ecx);
                crate::audio_lane::set_return(0x43d2c);
                call!(0x63c60);
                pc = 0x43d2c; continue;
            }
            0x43d2c => {
                eax = m.read32(esp.wrapping_add(0x14));
                esp = esp.wrapping_add(0xc);
                pc = 0x43d33; continue;
            }
            0x43d33 => {
                fa = eax;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x18), ebp);
                m.write32(esp.wrapping_add(0x10), ebp);
                m.write32(esp.wrapping_add(0x14), ebp);
                if (fas <= fbs) { pc = 0x4483e; continue; }
                pc = 0x43d47; continue;
            }
            0x43d47 => {
                edx = esp.wrapping_add(0x20);
                push!(edx);
                crate::audio_lane::set_return(0x43d51);
                call!(0x63bf0);
                pc = 0x43d51; continue;
            }
            0x43d51 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x44829; continue; }
                pc = 0x43d5c; continue;
            }
            0x43d5c => {
                push!(ebx);
                push!(edi);
                ebx = 0x10;
                pc = 0x43d63; continue;
            }
            0x43d63 => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x43d6d);
                call!(0x63e70);
                pc = 0x43d6d; continue;
            }
            0x43d6d => {
                ecx = esp.wrapping_add(0x2c);
                push!(0x4785a4);
                push!(ecx);
                crate::audio_lane::set_return(0x43d7c);
                call!(0x63c00);
                pc = 0x43d7c; continue;
            }
            0x43d7c => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x43f95; continue; }
                pc = 0x43d87; continue;
            }
            0x43d87 => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                crate::audio_lane::set_return(0x43d91);
                call!(0x63bf0);
                pc = 0x43d91; continue;
            }
            0x43d91 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x43ea5; continue; }
                pc = 0x43d9c; continue;
            }
            0x43d9c => {
                pc = 0x43da0; continue;
            }
            0x43da0 => {
                eax = esp.wrapping_add(0x28);
                push!(0x478598);
                push!(eax);
                crate::audio_lane::set_return(0x43daf);
                call!(0x63c00);
                pc = 0x43daf; continue;
            }
            0x43daf => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x43ea5; continue; }
                pc = 0x43dba; continue;
            }
            0x43dba => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x43dc4);
                call!(0x63e70);
                pc = 0x43dc4; continue;
            }
            0x43dc4 => {
                edx = esp.wrapping_add(0x2c);
                push!(0x478588);
                push!(edx);
                crate::audio_lane::set_return(0x43dd3);
                call!(0x63c00);
                pc = 0x43dd3; continue;
            }
            0x43dd3 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x43df9; continue; }
                pc = 0x43dda; continue;
            }
            0x43dda => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x43de4);
                call!(0x63e70);
                pc = 0x43de4; continue;
            }
            0x43de4 => {
                push!(eax);
                crate::audio_lane::set_return(0x43dea);
                call!(0x3c8fd0);
                pc = 0x43dea; continue;
            }
            0x43dea => {
                m.write32(0x6b990c, eax);
                m.write32(0x6b9910, eax);
                pc = 0x43e8d; continue;
            }
            0x43df9 => {
                ecx = esp.wrapping_add(0x28);
                push!(0x478578);
                push!(ecx);
                crate::audio_lane::set_return(0x43e08);
                call!(0x63c00);
                pc = 0x43e08; continue;
            }
            0x43e08 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x43e2b; continue; }
                pc = 0x43e0f; continue;
            }
            0x43e0f => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                crate::audio_lane::set_return(0x43e19);
                call!(0x63e70);
                pc = 0x43e19; continue;
            }
            0x43e19 => {
                push!(eax);
                crate::audio_lane::set_return(0x43e1f);
                call!(0x3c8fd0);
                pc = 0x43e1f; continue;
            }
            0x43e1f => {
                m.write32(0x6b9914, eax);
                m.write32(0x6b9918, eax);
                pc = 0x43e8d; continue;
            }
            0x43e2b => {
                eax = esp.wrapping_add(0x28);
                push!(0x478560);
                push!(eax);
                crate::audio_lane::set_return(0x43e3a);
                call!(0x63c00);
                pc = 0x43e3a; continue;
            }
            0x43e3a => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x43e5d; continue; }
                pc = 0x43e41; continue;
            }
            0x43e41 => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x43e4b);
                call!(0x63e70);
                pc = 0x43e4b; continue;
            }
            0x43e4b => {
                push!(eax);
                crate::audio_lane::set_return(0x43e51);
                call!(0x3c8fd0);
                pc = 0x43e51; continue;
            }
            0x43e51 => {
                m.write32(0x6b991c, eax);
                m.write32(0x6b9920, eax);
                pc = 0x43e8d; continue;
            }
            0x43e5d => {
                edx = esp.wrapping_add(0x28);
                push!(0x478548);
                push!(edx);
                crate::audio_lane::set_return(0x43e6c);
                call!(0x63c00);
                pc = 0x43e6c; continue;
            }
            0x43e6c => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x43e90; continue; }
                pc = 0x43e73; continue;
            }
            0x43e73 => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x43e7d);
                call!(0x63e70);
                pc = 0x43e7d; continue;
            }
            0x43e7d => {
                push!(eax);
                crate::audio_lane::set_return(0x43e83);
                call!(0x3c8fd0);
                pc = 0x43e83; continue;
            }
            0x43e83 => {
                m.write32(0x6b9924, eax);
                m.write32(0x6b9928, eax);
                pc = 0x43e8d; continue;
            }
            0x43e8d => {
                esp = esp.wrapping_add(0x8);
                pc = 0x43e90; continue;
            }
            0x43e90 => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x43e9a);
                call!(0x63bf0);
                pc = 0x43e9a; continue;
            }
            0x43e9a => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x43da0; continue; }
                pc = 0x43ea5; continue;
            }
            0x43ea5 => {
                eax = m.read32(0x6b990c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x43ee0; continue; }
                pc = 0x43eae; continue;
            }
            0x43eae => {
                eax = ((eax as i32).wrapping_mul((0x64) as i32)) as u32;
                eax = eax.wrapping_add(0xf);
                eax = (eax & 0xfffffff0u32);
                push!(0x1);
                push!(eax);
                crate::audio_lane::set_return(0x43ebf);
                call!(0x3dfa0);
                pc = 0x43ebf; continue;
            }
            0x43ebf => {
                ecx = m.read32(0x6b990c);
                ecx = ((ecx as i32).wrapping_mul((0x64) as i32)) as u32;
                ecx = ecx.wrapping_add(0xf);
                edi = eax;
                ecx = (ecx & 0xfffffff0u32);
                eax = 0;
                esp = esp.wrapping_add(0x8);
                ecx = ecx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write32(0x6b9908, edi);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                pc = 0x43ee0; continue;
            }
            0x43ee0 => {
                eax = m.read32(0x6b9914);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x43f1b; continue; }
                pc = 0x43ee9; continue;
            }
            0x43ee9 => {
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                eax = eax.wrapping_add(0xf);
                eax = (eax & 0xfffffff0u32);
                push!(0x1);
                push!(eax);
                crate::audio_lane::set_return(0x43efa);
                call!(0x3dfa0);
                pc = 0x43efa; continue;
            }
            0x43efa => {
                ecx = m.read32(0x6b9914);
                ecx = ((ecx as i32).wrapping_mul((0x2c) as i32)) as u32;
                ecx = ecx.wrapping_add(0xf);
                edi = eax;
                ecx = (ecx & 0xfffffff0u32);
                eax = 0;
                esp = esp.wrapping_add(0x8);
                ecx = ecx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write32(0x6b98b8, edi);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                pc = 0x43f1b; continue;
            }
            0x43f1b => {
                eax = m.read32(0x6b991c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x43f58; continue; }
                pc = 0x43f24; continue;
            }
            0x43f24 => {
                edx = eax.wrapping_mul(0x8).wrapping_add(0xf);
                edx = (edx & 0xfffffff0u32);
                push!(0x1);
                push!(edx);
                crate::audio_lane::set_return(0x43f36);
                call!(0x3dfa0);
                pc = 0x43f36; continue;
            }
            0x43f36 => {
                ecx = m.read32(0x6b991c);
                ecx = ecx.wrapping_mul(0x8).wrapping_add(0xf);
                edi = eax;
                ecx = (ecx & 0xfffffff0u32);
                eax = 0;
                esp = esp.wrapping_add(0x8);
                ecx = ecx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write32(0x6b98bc, edi);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                pc = 0x43f58; continue;
            }
            0x43f58 => {
                eax = m.read32(0x6b9924);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x43f95; continue; }
                pc = 0x43f61; continue;
            }
            0x43f61 => {
                eax = eax.wrapping_mul(0x8).wrapping_add(0xf);
                eax = (eax & 0xfffffff0u32);
                push!(0x1);
                push!(eax);
                crate::audio_lane::set_return(0x43f73);
                call!(0x3dfa0);
                pc = 0x43f73; continue;
            }
            0x43f73 => {
                ecx = m.read32(0x6b9924);
                ecx = ecx.wrapping_mul(0x8).wrapping_add(0xf);
                edi = eax;
                ecx = (ecx & 0xfffffff0u32);
                eax = 0;
                esp = esp.wrapping_add(0x8);
                ecx = ecx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write32(0x6b98c0, edi);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                pc = 0x43f95; continue;
            }
            0x43f95 => {
                ecx = esp.wrapping_add(0x28);
                push!(0x478538);
                push!(ecx);
                crate::audio_lane::set_return(0x43fa4);
                call!(0x63c00);
                pc = 0x43fa4; continue;
            }
            0x43fa4 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44431; continue; }
                pc = 0x43faf; continue;
            }
            0x43faf => {
                edx = m.read32(esp.wrapping_add(0x1c));
                fa = edx;
                fb = m.read32(0x6b990c);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x44431; continue; }
                pc = 0x43fbf; continue;
            }
            0x43fbf => {
                eax = edx;
                edx = m.read32(0x6b9908);
                esi = eax;
                esi = ((esi as i32).wrapping_mul((0x64) as i32)) as u32;
                esi = esi.wrapping_add(edx);
                eax = eax.wrapping_add(1);
                m.write32(esp.wrapping_add(0x1c), eax);
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x43fdd);
                call!(0x63bf0);
                pc = 0x43fdd; continue;
            }
            0x43fdd => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x44410; continue; }
                pc = 0x43fe8; continue;
            }
            0x43fe8 => {
                pc = 0x43ff0; continue;
            }
            0x43ff0 => {
                ecx = esp.wrapping_add(0x28);
                push!(0x47852c);
                push!(ecx);
                crate::audio_lane::set_return(0x43fff);
                call!(0x63c00);
                pc = 0x43fff; continue;
            }
            0x43fff => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x44410; continue; }
                pc = 0x4400a; continue;
            }
            0x4400a => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                crate::audio_lane::set_return(0x44014);
                call!(0x63e70);
                pc = 0x44014; continue;
            }
            0x44014 => {
                eax = esp.wrapping_add(0x2c);
                push!(0x478524);
                push!(eax);
                crate::audio_lane::set_return(0x44023);
                call!(0x63c00);
                pc = 0x44023; continue;
            }
            0x44023 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44041; continue; }
                pc = 0x4402a; continue;
            }
            0x4402a => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x44034);
                call!(0x63e70);
                pc = 0x44034; continue;
            }
            0x44034 => {
                push!(eax);
                crate::audio_lane::set_return(0x4403a);
                call!(0x3c8fd0);
                pc = 0x4403a; continue;
            }
            0x4403a => {
                m.write32(esi, eax);
                pc = 0x443f8; continue;
            }
            0x44041 => {
                edx = esp.wrapping_add(0x28);
                push!(0x47851c);
                push!(edx);
                crate::audio_lane::set_return(0x44050);
                call!(0x63c00);
                pc = 0x44050; continue;
            }
            0x44050 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44070; continue; }
                pc = 0x44057; continue;
            }
            0x44057 => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x44061);
                call!(0x63e70);
                pc = 0x44061; continue;
            }
            0x44061 => {
                push!(eax);
                crate::audio_lane::set_return(0x44067);
                call!(0x3c8fd0);
                pc = 0x44067; continue;
            }
            0x44067 => {
                write16(m, esi.wrapping_add(0x8), (((((eax) as u16)) as u32)) as u16);
                pc = 0x443f8; continue;
            }
            0x44070 => {
                ecx = esp.wrapping_add(0x28);
                push!(0x478514);
                push!(ecx);
                crate::audio_lane::set_return(0x4407f);
                call!(0x63c00);
                pc = 0x4407f; continue;
            }
            0x4407f => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x4409f; continue; }
                pc = 0x44086; continue;
            }
            0x44086 => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                crate::audio_lane::set_return(0x44090);
                call!(0x63e70);
                pc = 0x44090; continue;
            }
            0x44090 => {
                push!(eax);
                crate::audio_lane::set_return(0x44096);
                call!(0x3c8fd0);
                pc = 0x44096; continue;
            }
            0x44096 => {
                write16(m, esi.wrapping_add(0xa), (((((eax) as u16)) as u32)) as u16);
                pc = 0x443f8; continue;
            }
            0x4409f => {
                eax = esp.wrapping_add(0x28);
                push!(0x478510);
                push!(eax);
                crate::audio_lane::set_return(0x440ae);
                call!(0x63c00);
                pc = 0x440ae; continue;
            }
            0x440ae => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x4413e; continue; }
                pc = 0x440b9; continue;
            }
            0x440b9 => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x440c3);
                call!(0x63e70);
                pc = 0x440c3; continue;
            }
            0x440c3 => {
                push!(eax);
                crate::audio_lane::set_return(0x440c9);
                call!(0x3c9c6f);
                pc = 0x440c9; continue;
            }
            0x440c9 => {
                crate::audio_lane::set_return(0x440ce);
                call!(0x3c848c);
                pc = 0x440ce; continue;
            }
            0x440ce => {
                edx = esp.wrapping_add(0x30);
                push!(edx);
                write16(m, esi.wrapping_add(0xc), (((((eax) as u16)) as u32)) as u16);
                crate::audio_lane::set_return(0x440dc);
                call!(0x63e70);
                pc = 0x440dc; continue;
            }
            0x440dc => {
                push!(eax);
                crate::audio_lane::set_return(0x440e2);
                call!(0x3c9c6f);
                pc = 0x440e2; continue;
            }
            0x440e2 => {
                crate::audio_lane::set_return(0x440e7);
                call!(0x3c848c);
                pc = 0x440e7; continue;
            }
            0x440e7 => {
                write16(m, esi.wrapping_add(0xe), (((((eax) as u16)) as u32)) as u16);
                eax = esp.wrapping_add(0x38);
                push!(eax);
                crate::audio_lane::set_return(0x440f5);
                call!(0x63e70);
                pc = 0x440f5; continue;
            }
            0x440f5 => {
                push!(eax);
                crate::audio_lane::set_return(0x440fb);
                call!(0x3c9c6f);
                pc = 0x440fb; continue;
            }
            0x440fb => {
                crate::audio_lane::set_return(0x44100);
                call!(0x3c848c);
                pc = 0x44100; continue;
            }
            0x44100 => {
                ecx = esp.wrapping_add(0x40);
                push!(ecx);
                write16(m, esi.wrapping_add(0x10), (((((eax) as u16)) as u32)) as u16);
                crate::audio_lane::set_return(0x4410e);
                call!(0x63e70);
                pc = 0x4410e; continue;
            }
            0x4410e => {
                push!(eax);
                crate::audio_lane::set_return(0x44114);
                call!(0x3c9c6f);
                pc = 0x44114; continue;
            }
            0x44114 => {
                crate::audio_lane::set_return(0x44119);
                call!(0x3c848c);
                pc = 0x44119; continue;
            }
            0x44119 => {
                edx = esp.wrapping_add(0x48);
                push!(edx);
                write16(m, esi.wrapping_add(0x12), (((((eax) as u16)) as u32)) as u16);
                crate::audio_lane::set_return(0x44127);
                call!(0x63e70);
                pc = 0x44127; continue;
            }
            0x44127 => {
                push!(eax);
                crate::audio_lane::set_return(0x4412d);
                call!(0x3c9c6f);
                pc = 0x4412d; continue;
            }
            0x4412d => {
                esp = esp.wrapping_add(0x28);
                crate::audio_lane::set_return(0x44135);
                call!(0x3c848c);
                pc = 0x44135; continue;
            }
            0x44135 => {
                write16(m, esi.wrapping_add(0x14), (((((eax) as u16)) as u32)) as u16);
                pc = 0x443fb; continue;
            }
            0x4413e => {
                eax = esp.wrapping_add(0x28);
                push!(0x478508);
                push!(eax);
                crate::audio_lane::set_return(0x4414d);
                call!(0x63c00);
                pc = 0x4414d; continue;
            }
            0x4414d => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x4416c; continue; }
                pc = 0x44154; continue;
            }
            0x44154 => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x4415e);
                call!(0x63e70);
                pc = 0x4415e; continue;
            }
            0x4415e => {
                push!(eax);
                crate::audio_lane::set_return(0x44164);
                call!(0x3c9c6f);
                pc = 0x44164; continue;
            }
            0x44164 => {
                m.write32(esi.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                pc = 0x443f8; continue;
            }
            0x4416c => {
                edx = esp.wrapping_add(0x28);
                push!(0x478504);
                push!(edx);
                crate::audio_lane::set_return(0x4417b);
                call!(0x63c00);
                pc = 0x4417b; continue;
            }
            0x4417b => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = esp.wrapping_add(0x28);
                if (fa == fb) { pc = 0x441c3; continue; }
                pc = 0x44186; continue;
            }
            0x44186 => {
                push!(eax);
                crate::audio_lane::set_return(0x4418c);
                call!(0x63e70);
                pc = 0x4418c; continue;
            }
            0x4418c => {
                push!(eax);
                crate::audio_lane::set_return(0x44192);
                call!(0x3c9c6f);
                pc = 0x44192; continue;
            }
            0x44192 => {
                m.write32(esi.wrapping_add(0x1c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ecx = esp.wrapping_add(0x30);
                push!(ecx);
                crate::audio_lane::set_return(0x4419f);
                call!(0x63e70);
                pc = 0x4419f; continue;
            }
            0x4419f => {
                push!(eax);
                crate::audio_lane::set_return(0x441a5);
                call!(0x3c9c6f);
                pc = 0x441a5; continue;
            }
            0x441a5 => {
                m.write32(esi.wrapping_add(0x20), (fpu.st(0) as f32).to_bits()); fpu.pop();
                edx = esp.wrapping_add(0x38);
                push!(edx);
                crate::audio_lane::set_return(0x441b2);
                call!(0x63e70);
                pc = 0x441b2; continue;
            }
            0x441b2 => {
                push!(eax);
                crate::audio_lane::set_return(0x441b8);
                call!(0x3c9c6f);
                pc = 0x441b8; continue;
            }
            0x441b8 => {
                m.write32(esi.wrapping_add(0x24), (fpu.st(0) as f32).to_bits()); fpu.pop();
                esp = esp.wrapping_add(0x18);
                pc = 0x443fb; continue;
            }
            0x441c3 => {
                push!(0x4784fc);
                push!(eax);
                crate::audio_lane::set_return(0x441ce);
                call!(0x63c00);
                pc = 0x441ce; continue;
            }
            0x441ce => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44229; continue; }
                pc = 0x441d5; continue;
            }
            0x441d5 => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x441df);
                call!(0x63e70);
                pc = 0x441df; continue;
            }
            0x441df => {
                push!(eax);
                crate::audio_lane::set_return(0x441e5);
                call!(0x3c9c6f);
                pc = 0x441e5; continue;
            }
            0x441e5 => {
                m.write32(esi.wrapping_add(0x28), (fpu.st(0) as f32).to_bits()); fpu.pop();
                edx = esp.wrapping_add(0x30);
                push!(edx);
                crate::audio_lane::set_return(0x441f2);
                call!(0x63e70);
                pc = 0x441f2; continue;
            }
            0x441f2 => {
                push!(eax);
                crate::audio_lane::set_return(0x441f8);
                call!(0x3c9c6f);
                pc = 0x441f8; continue;
            }
            0x441f8 => {
                m.write32(esi.wrapping_add(0x2c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = esp.wrapping_add(0x38);
                push!(eax);
                crate::audio_lane::set_return(0x44205);
                call!(0x63e70);
                pc = 0x44205; continue;
            }
            0x44205 => {
                push!(eax);
                crate::audio_lane::set_return(0x4420b);
                call!(0x3c9c6f);
                pc = 0x4420b; continue;
            }
            0x4420b => {
                m.write32(esi.wrapping_add(0x30), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ecx = esp.wrapping_add(0x40);
                push!(ecx);
                crate::audio_lane::set_return(0x44218);
                call!(0x63e70);
                pc = 0x44218; continue;
            }
            0x44218 => {
                push!(eax);
                crate::audio_lane::set_return(0x4421e);
                call!(0x3c9c6f);
                pc = 0x4421e; continue;
            }
            0x4421e => {
                m.write32(esi.wrapping_add(0x34), (fpu.st(0) as f32).to_bits()); fpu.pop();
                esp = esp.wrapping_add(0x20);
                pc = 0x443fb; continue;
            }
            0x44229 => {
                edx = esp.wrapping_add(0x28);
                push!(0x4784f4);
                push!(edx);
                crate::audio_lane::set_return(0x44238);
                call!(0x63c00);
                pc = 0x44238; continue;
            }
            0x44238 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44257; continue; }
                pc = 0x4423f; continue;
            }
            0x4423f => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x44249);
                call!(0x63e70);
                pc = 0x44249; continue;
            }
            0x44249 => {
                push!(eax);
                crate::audio_lane::set_return(0x4424f);
                call!(0x3c9c6f);
                pc = 0x4424f; continue;
            }
            0x4424f => {
                m.write32(esi.wrapping_add(0x38), (fpu.st(0) as f32).to_bits()); fpu.pop();
                pc = 0x443f8; continue;
            }
            0x44257 => {
                ecx = esp.wrapping_add(0x28);
                push!(0x4784ec);
                push!(ecx);
                crate::audio_lane::set_return(0x44266);
                call!(0x63c00);
                pc = 0x44266; continue;
            }
            0x44266 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44285; continue; }
                pc = 0x4426d; continue;
            }
            0x4426d => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                crate::audio_lane::set_return(0x44277);
                call!(0x63e70);
                pc = 0x44277; continue;
            }
            0x44277 => {
                push!(eax);
                crate::audio_lane::set_return(0x4427d);
                call!(0x3c9c6f);
                pc = 0x4427d; continue;
            }
            0x4427d => {
                m.write32(esi.wrapping_add(0x3c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                pc = 0x443f8; continue;
            }
            0x44285 => {
                eax = esp.wrapping_add(0x28);
                push!(0x4784e4);
                push!(eax);
                crate::audio_lane::set_return(0x44294);
                call!(0x63c00);
                pc = 0x44294; continue;
            }
            0x44294 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x442b3; continue; }
                pc = 0x4429b; continue;
            }
            0x4429b => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x442a5);
                call!(0x63e70);
                pc = 0x442a5; continue;
            }
            0x442a5 => {
                push!(eax);
                crate::audio_lane::set_return(0x442ab);
                call!(0x3c9c6f);
                pc = 0x442ab; continue;
            }
            0x442ab => {
                m.write32(esi.wrapping_add(0x40), (fpu.st(0) as f32).to_bits()); fpu.pop();
                pc = 0x443f8; continue;
            }
            0x442b3 => {
                edx = esp.wrapping_add(0x28);
                push!(0x4784dc);
                push!(edx);
                crate::audio_lane::set_return(0x442c2);
                call!(0x63c00);
                pc = 0x442c2; continue;
            }
            0x442c2 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x442e1; continue; }
                pc = 0x442c9; continue;
            }
            0x442c9 => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x442d3);
                call!(0x63e70);
                pc = 0x442d3; continue;
            }
            0x442d3 => {
                push!(eax);
                crate::audio_lane::set_return(0x442d9);
                call!(0x3c9c6f);
                pc = 0x442d9; continue;
            }
            0x442d9 => {
                m.write32(esi.wrapping_add(0x44), (fpu.st(0) as f32).to_bits()); fpu.pop();
                pc = 0x443f8; continue;
            }
            0x442e1 => {
                ecx = esp.wrapping_add(0x28);
                push!(0x4784d4);
                push!(ecx);
                crate::audio_lane::set_return(0x442f0);
                call!(0x63c00);
                pc = 0x442f0; continue;
            }
            0x442f0 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x4430f; continue; }
                pc = 0x442f7; continue;
            }
            0x442f7 => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                crate::audio_lane::set_return(0x44301);
                call!(0x63e70);
                pc = 0x44301; continue;
            }
            0x44301 => {
                push!(eax);
                crate::audio_lane::set_return(0x44307);
                call!(0x3c9c6f);
                pc = 0x44307; continue;
            }
            0x44307 => {
                m.write32(esi.wrapping_add(0x48), (fpu.st(0) as f32).to_bits()); fpu.pop();
                pc = 0x443f8; continue;
            }
            0x4430f => {
                eax = esp.wrapping_add(0x28);
                push!(0x4784c8);
                push!(eax);
                crate::audio_lane::set_return(0x4431e);
                call!(0x63c00);
                pc = 0x4431e; continue;
            }
            0x4431e => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44379; continue; }
                pc = 0x44325; continue;
            }
            0x44325 => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x4432f);
                call!(0x63e70);
                pc = 0x4432f; continue;
            }
            0x4432f => {
                push!(eax);
                crate::audio_lane::set_return(0x44335);
                call!(0x3c9c6f);
                pc = 0x44335; continue;
            }
            0x44335 => {
                m.write32(esi.wrapping_add(0x4c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                edx = esp.wrapping_add(0x30);
                push!(edx);
                crate::audio_lane::set_return(0x44342);
                call!(0x63e70);
                pc = 0x44342; continue;
            }
            0x44342 => {
                push!(eax);
                crate::audio_lane::set_return(0x44348);
                call!(0x3c9c6f);
                pc = 0x44348; continue;
            }
            0x44348 => {
                m.write32(esi.wrapping_add(0x50), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = esp.wrapping_add(0x38);
                push!(eax);
                crate::audio_lane::set_return(0x44355);
                call!(0x63e70);
                pc = 0x44355; continue;
            }
            0x44355 => {
                push!(eax);
                crate::audio_lane::set_return(0x4435b);
                call!(0x3c9c6f);
                pc = 0x4435b; continue;
            }
            0x4435b => {
                m.write32(esi.wrapping_add(0x54), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ecx = esp.wrapping_add(0x40);
                push!(ecx);
                crate::audio_lane::set_return(0x44368);
                call!(0x63e70);
                pc = 0x44368; continue;
            }
            0x44368 => {
                push!(eax);
                crate::audio_lane::set_return(0x4436e);
                call!(0x3c9c6f);
                pc = 0x4436e; continue;
            }
            0x4436e => {
                m.write32(esi.wrapping_add(0x58), (fpu.st(0) as f32).to_bits()); fpu.pop();
                esp = esp.wrapping_add(0x20);
                pc = 0x443fb; continue;
            }
            0x44379 => {
                edx = esp.wrapping_add(0x28);
                push!(0x4784c0);
                push!(edx);
                crate::audio_lane::set_return(0x44388);
                call!(0x63c00);
                pc = 0x44388; continue;
            }
            0x44388 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x443a4; continue; }
                pc = 0x4438f; continue;
            }
            0x4438f => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x44399);
                call!(0x63e70);
                pc = 0x44399; continue;
            }
            0x44399 => {
                push!(eax);
                crate::audio_lane::set_return(0x4439f);
                call!(0x3c8fd0);
                pc = 0x4439f; continue;
            }
            0x4439f => {
                m.write8(esi.wrapping_add(0x5c), (((((eax) as u8)) as u32)) as u8);
                pc = 0x443f8; continue;
            }
            0x443a4 => {
                ecx = esp.wrapping_add(0x28);
                push!(0x4784b8);
                push!(ecx);
                crate::audio_lane::set_return(0x443b3);
                call!(0x63c00);
                pc = 0x443b3; continue;
            }
            0x443b3 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x443cf; continue; }
                pc = 0x443ba; continue;
            }
            0x443ba => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                crate::audio_lane::set_return(0x443c4);
                call!(0x63e70);
                pc = 0x443c4; continue;
            }
            0x443c4 => {
                push!(eax);
                crate::audio_lane::set_return(0x443ca);
                call!(0x3c8fd0);
                pc = 0x443ca; continue;
            }
            0x443ca => {
                m.write8(esi.wrapping_add(0x5d), (((((eax) as u8)) as u32)) as u8);
                pc = 0x443f8; continue;
            }
            0x443cf => {
                eax = esp.wrapping_add(0x28);
                push!(0x4784b0);
                push!(eax);
                crate::audio_lane::set_return(0x443de);
                call!(0x63c00);
                pc = 0x443de; continue;
            }
            0x443de => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x443fb; continue; }
                pc = 0x443e5; continue;
            }
            0x443e5 => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x443ef);
                call!(0x63e70);
                pc = 0x443ef; continue;
            }
            0x443ef => {
                push!(eax);
                crate::audio_lane::set_return(0x443f5);
                call!(0x3c8fd0);
                pc = 0x443f5; continue;
            }
            0x443f5 => {
                m.write32(esi.wrapping_add(0x60), eax);
                pc = 0x443f8; continue;
            }
            0x443f8 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x443fb; continue;
            }
            0x443fb => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                crate::audio_lane::set_return(0x44405);
                call!(0x63bf0);
                pc = 0x44405; continue;
            }
            0x44405 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x43ff0; continue; }
                pc = 0x44410; continue;
            }
            0x44410 => {
                push!(0);
                eax = esi.wrapping_add(0x1c);
                push!(0x3fa00000);
                push!(eax);
                crate::audio_lane::set_return(0x44420);
                call!(0xc3c50);
                pc = 0x44420; continue;
            }
            0x44420 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x4442a; continue; }
                pc = 0x44427; continue;
            }
            0x44427 => {
                m.write32(esi.wrapping_add(0x60), (m.read32(esi.wrapping_add(0x60)) | ebx));
                pc = 0x4442a; continue;
            }
            0x4442a => {
                m.write32(esi.wrapping_add(0x4), 0xffffffffu32);
                pc = 0x44431; continue;
            }
            0x44431 => {
                ecx = esp.wrapping_add(0x28);
                push!(0x4784a0);
                push!(ecx);
                crate::audio_lane::set_return(0x44440);
                call!(0x63c00);
                pc = 0x44440; continue;
            }
            0x44440 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44636; continue; }
                pc = 0x4444b; continue;
            }
            0x4444b => {
                edx = m.read32(esp.wrapping_add(0x18));
                fa = edx;
                fb = m.read32(0x6b9914);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x44636; continue; }
                pc = 0x4445b; continue;
            }
            0x4445b => {
                eax = edx;
                edx = m.read32(0x6b98b8);
                esi = eax;
                esi = ((esi as i32).wrapping_mul((0x2c) as i32)) as u32;
                esi = esi.wrapping_add(edx);
                eax = eax.wrapping_add(1);
                m.write32(esp.wrapping_add(0x18), eax);
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x44479);
                call!(0x63bf0);
                pc = 0x44479; continue;
            }
            0x44479 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x4461c; continue; }
                pc = 0x44484; continue;
            }
            0x44484 => {
                ecx = esp.wrapping_add(0x28);
                push!(0x478490);
                push!(ecx);
                crate::audio_lane::set_return(0x44493);
                call!(0x63c00);
                pc = 0x44493; continue;
            }
            0x44493 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x4461c; continue; }
                pc = 0x4449e; continue;
            }
            0x4449e => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                crate::audio_lane::set_return(0x444a8);
                call!(0x63e70);
                pc = 0x444a8; continue;
            }
            0x444a8 => {
                eax = esp.wrapping_add(0x2c);
                push!(0x478524);
                push!(eax);
                crate::audio_lane::set_return(0x444b7);
                call!(0x63c00);
                pc = 0x444b7; continue;
            }
            0x444b7 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x444d5; continue; }
                pc = 0x444be; continue;
            }
            0x444be => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x444c8);
                call!(0x63e70);
                pc = 0x444c8; continue;
            }
            0x444c8 => {
                push!(eax);
                crate::audio_lane::set_return(0x444ce);
                call!(0x3c8fd0);
                pc = 0x444ce; continue;
            }
            0x444ce => {
                m.write32(esi, eax);
                pc = 0x44604; continue;
            }
            0x444d5 => {
                edx = esp.wrapping_add(0x28);
                push!(0x47851c);
                push!(edx);
                crate::audio_lane::set_return(0x444e4);
                call!(0x63c00);
                pc = 0x444e4; continue;
            }
            0x444e4 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44504; continue; }
                pc = 0x444eb; continue;
            }
            0x444eb => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x444f5);
                call!(0x63e70);
                pc = 0x444f5; continue;
            }
            0x444f5 => {
                push!(eax);
                crate::audio_lane::set_return(0x444fb);
                call!(0x3c8fd0);
                pc = 0x444fb; continue;
            }
            0x444fb => {
                write16(m, esi.wrapping_add(0x4), (((((eax) as u16)) as u32)) as u16);
                pc = 0x44604; continue;
            }
            0x44504 => {
                ecx = esp.wrapping_add(0x28);
                push!(0x478504);
                push!(ecx);
                crate::audio_lane::set_return(0x44513);
                call!(0x63c00);
                pc = 0x44513; continue;
            }
            0x44513 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edx = esp.wrapping_add(0x28);
                if (fa == fb) { pc = 0x4455b; continue; }
                pc = 0x4451e; continue;
            }
            0x4451e => {
                push!(edx);
                crate::audio_lane::set_return(0x44524);
                call!(0x63e70);
                pc = 0x44524; continue;
            }
            0x44524 => {
                push!(eax);
                crate::audio_lane::set_return(0x4452a);
                call!(0x3c9c6f);
                pc = 0x4452a; continue;
            }
            0x4452a => {
                m.write32(esi.wrapping_add(0x8), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = esp.wrapping_add(0x30);
                push!(eax);
                crate::audio_lane::set_return(0x44537);
                call!(0x63e70);
                pc = 0x44537; continue;
            }
            0x44537 => {
                push!(eax);
                crate::audio_lane::set_return(0x4453d);
                call!(0x3c9c6f);
                pc = 0x4453d; continue;
            }
            0x4453d => {
                m.write32(esi.wrapping_add(0xc), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ecx = esp.wrapping_add(0x38);
                push!(ecx);
                crate::audio_lane::set_return(0x4454a);
                call!(0x63e70);
                pc = 0x4454a; continue;
            }
            0x4454a => {
                push!(eax);
                crate::audio_lane::set_return(0x44550);
                call!(0x3c9c6f);
                pc = 0x44550; continue;
            }
            0x44550 => {
                m.write32(esi.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                esp = esp.wrapping_add(0x18);
                pc = 0x44607; continue;
            }
            0x4455b => {
                push!(0x4784f4);
                push!(edx);
                crate::audio_lane::set_return(0x44566);
                call!(0x63c00);
                pc = 0x44566; continue;
            }
            0x44566 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44585; continue; }
                pc = 0x4456d; continue;
            }
            0x4456d => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x44577);
                call!(0x63e70);
                pc = 0x44577; continue;
            }
            0x44577 => {
                push!(eax);
                crate::audio_lane::set_return(0x4457d);
                call!(0x3c9c6f);
                pc = 0x4457d; continue;
            }
            0x4457d => {
                m.write32(esi.wrapping_add(0x14), (fpu.st(0) as f32).to_bits()); fpu.pop();
                pc = 0x44604; continue;
            }
            0x44585 => {
                ecx = esp.wrapping_add(0x28);
                push!(0x4784ec);
                push!(ecx);
                crate::audio_lane::set_return(0x44594);
                call!(0x63c00);
                pc = 0x44594; continue;
            }
            0x44594 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x445b0; continue; }
                pc = 0x4459b; continue;
            }
            0x4459b => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                crate::audio_lane::set_return(0x445a5);
                call!(0x63e70);
                pc = 0x445a5; continue;
            }
            0x445a5 => {
                push!(eax);
                crate::audio_lane::set_return(0x445ab);
                call!(0x3c9c6f);
                pc = 0x445ab; continue;
            }
            0x445ab => {
                m.write32(esi.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                pc = 0x44604; continue;
            }
            0x445b0 => {
                eax = esp.wrapping_add(0x28);
                push!(0x4784e4);
                push!(eax);
                crate::audio_lane::set_return(0x445bf);
                call!(0x63c00);
                pc = 0x445bf; continue;
            }
            0x445bf => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x445db; continue; }
                pc = 0x445c6; continue;
            }
            0x445c6 => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x445d0);
                call!(0x63e70);
                pc = 0x445d0; continue;
            }
            0x445d0 => {
                push!(eax);
                crate::audio_lane::set_return(0x445d6);
                call!(0x3c9c6f);
                pc = 0x445d6; continue;
            }
            0x445d6 => {
                m.write32(esi.wrapping_add(0x1c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                pc = 0x44604; continue;
            }
            0x445db => {
                edx = esp.wrapping_add(0x28);
                push!(0x4784dc);
                push!(edx);
                crate::audio_lane::set_return(0x445ea);
                call!(0x63c00);
                pc = 0x445ea; continue;
            }
            0x445ea => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44607; continue; }
                pc = 0x445f1; continue;
            }
            0x445f1 => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x445fb);
                call!(0x63e70);
                pc = 0x445fb; continue;
            }
            0x445fb => {
                push!(eax);
                crate::audio_lane::set_return(0x44601);
                call!(0x3c9c6f);
                pc = 0x44601; continue;
            }
            0x44601 => {
                m.write32(esi.wrapping_add(0x20), (fpu.st(0) as f32).to_bits()); fpu.pop();
                pc = 0x44604; continue;
            }
            0x44604 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x44607; continue;
            }
            0x44607 => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x44611);
                call!(0x63bf0);
                pc = 0x44611; continue;
            }
            0x44611 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44484; continue; }
                pc = 0x4461c; continue;
            }
            0x4461c => {
                push!(0);
                edx = esi.wrapping_add(0x8);
                push!(0x3fa00000);
                push!(edx);
                crate::audio_lane::set_return(0x4462c);
                call!(0xc3c50);
                pc = 0x4462c; continue;
            }
            0x4462c => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x44636; continue; }
                pc = 0x44633; continue;
            }
            0x44633 => {
                m.write32(esi.wrapping_add(0x28), (m.read32(esi.wrapping_add(0x28)) | ebx));
                pc = 0x44636; continue;
            }
            0x44636 => {
                eax = esp.wrapping_add(0x28);
                push!(0x478478);
                push!(eax);
                crate::audio_lane::set_return(0x44645);
                call!(0x63c00);
                pc = 0x44645; continue;
            }
            0x44645 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44725; continue; }
                pc = 0x44650; continue;
            }
            0x44650 => {
                eax = m.read32(esp.wrapping_add(0x20));
                fa = eax;
                fb = m.read32(0x6b991c);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x44725; continue; }
                pc = 0x44660; continue;
            }
            0x44660 => {
                ecx = m.read32(0x6b98bc);
                esi = ecx.wrapping_add(eax.wrapping_mul(0x8));
                edx = esp.wrapping_add(0x28);
                eax = eax.wrapping_add(1);
                push!(edx);
                m.write32(esp.wrapping_add(0x24), eax);
                crate::audio_lane::set_return(0x44678);
                call!(0x63bf0);
                pc = 0x44678; continue;
            }
            0x44678 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x44725; continue; }
                pc = 0x44683; continue;
            }
            0x44683 => {
                eax = esp.wrapping_add(0x28);
                push!(0x478464);
                push!(eax);
                crate::audio_lane::set_return(0x44692);
                call!(0x63c00);
                pc = 0x44692; continue;
            }
            0x44692 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x44725; continue; }
                pc = 0x4469d; continue;
            }
            0x4469d => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x446a7);
                call!(0x63e70);
                pc = 0x446a7; continue;
            }
            0x446a7 => {
                edx = esp.wrapping_add(0x2c);
                push!(0x478458);
                push!(edx);
                crate::audio_lane::set_return(0x446b6);
                call!(0x63c00);
                pc = 0x446b6; continue;
            }
            0x446b6 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x446dc; continue; }
                pc = 0x446bd; continue;
            }
            0x446bd => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x446c7);
                call!(0x63e70);
                pc = 0x446c7; continue;
            }
            0x446c7 => {
                push!(eax);
                crate::audio_lane::set_return(0x446cd);
                call!(0x3c8fd0);
                pc = 0x446cd; continue;
            }
            0x446cd => {
                esp = esp.wrapping_add(0x8);
                edi = eax;
                crate::audio_lane::set_return(0x446d7);
                call!(0x43bd0);
                pc = 0x446d7; continue;
            }
            0x446d7 => {
                write16(m, esi, (((((eax) as u16)) as u32)) as u16);
                pc = 0x44710; continue;
            }
            0x446dc => {
                ecx = esp.wrapping_add(0x28);
                push!(0x47844c);
                push!(ecx);
                crate::audio_lane::set_return(0x446eb);
                call!(0x63c00);
                pc = 0x446eb; continue;
            }
            0x446eb => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44710; continue; }
                pc = 0x446f2; continue;
            }
            0x446f2 => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                crate::audio_lane::set_return(0x446fc);
                call!(0x63e70);
                pc = 0x446fc; continue;
            }
            0x446fc => {
                push!(eax);
                crate::audio_lane::set_return(0x44702);
                call!(0x3c8fd0);
                pc = 0x44702; continue;
            }
            0x44702 => {
                esp = esp.wrapping_add(0x8);
                edi = eax;
                crate::audio_lane::set_return(0x4470c);
                call!(0x43c10);
                pc = 0x4470c; continue;
            }
            0x4470c => {
                write16(m, esi.wrapping_add(0x2), (((((eax) as u16)) as u32)) as u16);
                pc = 0x44710; continue;
            }
            0x44710 => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x4471a);
                call!(0x63bf0);
                pc = 0x4471a; continue;
            }
            0x4471a => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44683; continue; }
                pc = 0x44725; continue;
            }
            0x44725 => {
                ecx = esp.wrapping_add(0x28);
                push!(0x478434);
                push!(ecx);
                crate::audio_lane::set_return(0x44734);
                call!(0x63c00);
                pc = 0x44734; continue;
            }
            0x44734 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44812; continue; }
                pc = 0x4473f; continue;
            }
            0x4473f => {
                fa = ebp;
                fb = m.read32(0x6b9924);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x44812; continue; }
                pc = 0x4474b; continue;
            }
            0x4474b => {
                edx = m.read32(0x6b98c0);
                eax = esp.wrapping_add(0x28);
                esi = edx.wrapping_add(ebp.wrapping_mul(0x8));
                push!(eax);
                ebp = ebp.wrapping_add(1);
                crate::audio_lane::set_return(0x4475f);
                call!(0x63bf0);
                pc = 0x4475f; continue;
            }
            0x4475f => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x44812; continue; }
                pc = 0x4476a; continue;
            }
            0x4476a => {
                pc = 0x44770; continue;
            }
            0x44770 => {
                ecx = esp.wrapping_add(0x28);
                push!(0x47841c);
                push!(ecx);
                crate::audio_lane::set_return(0x4477f);
                call!(0x63c00);
                pc = 0x4477f; continue;
            }
            0x4477f => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x44812; continue; }
                pc = 0x4478a; continue;
            }
            0x4478a => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                crate::audio_lane::set_return(0x44794);
                call!(0x63e70);
                pc = 0x44794; continue;
            }
            0x44794 => {
                eax = esp.wrapping_add(0x2c);
                push!(0x478410);
                push!(eax);
                crate::audio_lane::set_return(0x447a3);
                call!(0x63c00);
                pc = 0x447a3; continue;
            }
            0x447a3 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x447c9; continue; }
                pc = 0x447aa; continue;
            }
            0x447aa => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x447b4);
                call!(0x63e70);
                pc = 0x447b4; continue;
            }
            0x447b4 => {
                push!(eax);
                crate::audio_lane::set_return(0x447ba);
                call!(0x3c8fd0);
                pc = 0x447ba; continue;
            }
            0x447ba => {
                esp = esp.wrapping_add(0x8);
                edi = eax;
                crate::audio_lane::set_return(0x447c4);
                call!(0x43bd0);
                pc = 0x447c4; continue;
            }
            0x447c4 => {
                write16(m, esi, (((((eax) as u16)) as u32)) as u16);
                pc = 0x447fd; continue;
            }
            0x447c9 => {
                edx = esp.wrapping_add(0x28);
                push!(0x478404);
                push!(edx);
                crate::audio_lane::set_return(0x447d8);
                call!(0x63c00);
                pc = 0x447d8; continue;
            }
            0x447d8 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x447fd; continue; }
                pc = 0x447df; continue;
            }
            0x447df => {
                eax = esp.wrapping_add(0x28);
                push!(eax);
                crate::audio_lane::set_return(0x447e9);
                call!(0x63e70);
                pc = 0x447e9; continue;
            }
            0x447e9 => {
                push!(eax);
                crate::audio_lane::set_return(0x447ef);
                call!(0x3c8fd0);
                pc = 0x447ef; continue;
            }
            0x447ef => {
                esp = esp.wrapping_add(0x8);
                edi = eax;
                crate::audio_lane::set_return(0x447f9);
                call!(0x43bd0);
                pc = 0x447f9; continue;
            }
            0x447f9 => {
                write16(m, esi.wrapping_add(0x2), (((((eax) as u16)) as u32)) as u16);
                pc = 0x447fd; continue;
            }
            0x447fd => {
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                crate::audio_lane::set_return(0x44807);
                call!(0x63bf0);
                pc = 0x44807; continue;
            }
            0x44807 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x44770; continue; }
                pc = 0x44812; continue;
            }
            0x44812 => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                crate::audio_lane::set_return(0x4481c);
                call!(0x63bf0);
                pc = 0x4481c; continue;
            }
            0x4481c => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x43d63; continue; }
                pc = 0x44827; continue;
            }
            0x44827 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x44829; continue;
            }
            0x44829 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x4483e; continue; }
                pc = 0x44831; continue;
            }
            0x44831 => {
                eax = m.read32(esp.wrapping_add(0xc));
                push!(eax);
                crate::audio_lane::set_return(0x4483b);
                call!(0x3e1e0);
                pc = 0x4483b; continue;
            }
            0x4483b => {
                esp = esp.wrapping_add(0x4);
                pc = 0x4483e; continue;
            }
            0x4483e => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x124);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00044890: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f44890<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x44890;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x44890, previous, pc); previous = pc; }
        match pc {
            0x44890 => {
                esp = esp.wrapping_sub(0x34);
                xmm2 = Xmm::scalar(m.read32(0x475d1c));
                push!(esi);
                esi = eax;
                eax = ((m.read8(esi.wrapping_add(0x5c))) as u8 as u32);
                ecx = ((m.read8(esi.wrapping_add(0x5d))) as u8 as u32);
                ecx = ecx.wrapping_sub(eax);
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                eax = m.read32(esi.wrapping_add(0x4));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x3c)))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                if (fa != fb) { pc = 0x44963; continue; }
                pc = 0x448da; continue;
            }
            0x448da => {
                edx = esp.wrapping_add(0x8);
                push!(edx);
                crate::audio_lane::set_return(0x448e4);
                call!(0x45390);
                pc = 0x448e4; continue;
            }
            0x448e4 => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x1c)));
                eax = (((((m.read16(esi.wrapping_add(0xa)) as i16)) as i32)) as u32);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0xc))) as u32)) & 0xff);
                edx = (edx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x14))) as u32)) & 0xff);
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x20)));
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x24)));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x18)));
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x8)));
                m.write32(esp.wrapping_add(0xc), eax);
                eax = esp.wrapping_add(0xc);
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                push!(eax);
                m.write8(esp.wrapping_add(0x1c), (((((ecx) as u8)) as u32)) as u8);
                m.write8(esp.wrapping_add(0x1d), (((((edx) as u8)) as u32)) as u8);
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                m.write8(esp.wrapping_add(0x38), (0x1) as u8);
                crate::audio_lane::set_return(0x4494d);
                call!(0x45820);
                pc = 0x4494d; continue;
            }
            0x4494d => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esi.wrapping_add(0x4), eax);
                if (fas >= fbs) { pc = 0x44975; continue; }
                pc = 0x44957; continue;
            }
            0x44957 => {
                m.write32(esi.wrapping_add(0x4), 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x34);
                break 'run;
            }
            0x44963 => {
                ecx = m.read32(esp.wrapping_add(0x3c));
                push!(ecx);
                esi = esi.wrapping_add(0x1c);
                push!(esi);
                push!(eax);
                crate::audio_lane::set_return(0x44972);
                call!(0x45600);
                pc = 0x44972; continue;
            }
            0x44972 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x44975; continue;
            }
            0x44975 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x34);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00044980: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f44980<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x44980;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x44980, previous, pc); previous = pc; }
        match pc {
            0x44980 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(esp.wrapping_add(0x8));
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                push!(ebx);
                ebx = m.read32(0x6b98b8);
                push!(esi);
                esi = m.read32(0x6b9924);
                push!(edi);
                edi = 0x1;
                edi = edi.wrapping_shl((((((((((ecx) as u8))) as u32) & 0x1fu32))) as u32 & 31));
                ecx = m.read32(eax.wrapping_add(ebx).wrapping_add(0x24));
                eax = eax.wrapping_add(ebx).wrapping_add(0x24);
                ecx = (ecx | edi);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(eax, ecx);
                if (fas <= fbs) { pc = 0x44a15; continue; }
                pc = 0x449b1; continue;
            }
            0x449b1 => {
                edx = m.read32(0x6b98c0);
                push!(ebp);
                pc = 0x449c0; continue;
            }
            0x449ba => {
                ebx = m.read32(0x6b98b8);
                pc = 0x449c0; continue;
            }
            0x449c0 => {
                ecx = (((((m.read16(edx.wrapping_add(esi.wrapping_mul(0x8)).wrapping_add(((0x8i32.wrapping_neg()) as u32))) as i16)) as i32)) as u32);
                eax = m.read32(esp.wrapping_add(0x14));
                esi = esi.wrapping_sub(1);
                fa = ecx;
                fb = eax;
                if (fa != fb) { pc = 0x449e6; continue; }
                pc = 0x449ce; continue;
            }
            0x449ce => {
                eax = (((((m.read16(edx.wrapping_add(esi.wrapping_mul(0x8)).wrapping_add(0x2)) as i16)) as i32)) as u32);
                ebp = eax;
                ebp = ((ebp as i32).wrapping_mul((0x2c) as i32)) as u32;
                fa = m.read32(ebx.wrapping_add(ebp).wrapping_add(0x24));
                fb = edi;
                if ((fa & fb) != 0) { pc = 0x449e6; continue; }
                pc = 0x449de; continue;
            }
            0x449de => {
                ecx = m.read32(esp.wrapping_add(0x18));
                push!(ecx);
                push!(eax);
                pc = 0x44a02; continue;
            }
            0x449e6 => {
                eax = (((((m.read16(edx.wrapping_add(esi.wrapping_mul(0x8)).wrapping_add(0x2)) as i16)) as i32)) as u32);
                fa = eax;
                fb = m.read32(esp.wrapping_add(0x14));
                if (fa != fb) { pc = 0x44a10; continue; }
                pc = 0x449f1; continue;
            }
            0x449f1 => {
                eax = ecx;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                fa = m.read32(eax.wrapping_add(ebx).wrapping_add(0x24));
                fb = edi;
                if ((fa & fb) != 0) { pc = 0x44a10; continue; }
                pc = 0x449fc; continue;
            }
            0x449fc => {
                edx = m.read32(esp.wrapping_add(0x18));
                push!(edx);
                push!(ecx);
                pc = 0x44a02; continue;
            }
            0x44a02 => {
                crate::audio_lane::set_return(0x44a07);
                call!(0x44980);
                pc = 0x44a07; continue;
            }
            0x44a07 => {
                edx = m.read32(0x6b98c0);
                esp = esp.wrapping_add(0x8);
                pc = 0x44a10; continue;
            }
            0x44a10 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x449ba; continue; }
                pc = 0x44a14; continue;
            }
            0x44a14 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x44a15; continue;
            }
            0x44a15 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00044A20: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f44a20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x44a20;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x44a20, previous, pc); previous = pc; }
        match pc {
            0x44a20 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(esp.wrapping_add(0x8));
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                push!(ebx);
                ebx = m.read32(0x6b98b8);
                edx = m.read32(eax.wrapping_add(ebx).wrapping_add(0x24));
                push!(esi);
                esi = m.read32(0x6b9924);
                push!(edi);
                edi = 0x1;
                edi = edi.wrapping_shl((((((((((ecx) as u8))) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(ebx).wrapping_add(0x24);
                ecx = edi;
                ecx = !ecx;
                edx = (edx & ecx);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(eax, edx);
                if (fas <= fbs) { pc = 0x44ab9; continue; }
                pc = 0x44a55; continue;
            }
            0x44a55 => {
                edx = m.read32(0x6b98c0);
                push!(ebp);
                pc = 0x44a64; continue;
            }
            0x44a5e => {
                ebx = m.read32(0x6b98b8);
                pc = 0x44a64; continue;
            }
            0x44a64 => {
                ecx = (((((m.read16(edx.wrapping_add(esi.wrapping_mul(0x8)).wrapping_add(((0x8i32.wrapping_neg()) as u32))) as i16)) as i32)) as u32);
                eax = m.read32(esp.wrapping_add(0x14));
                esi = esi.wrapping_sub(1);
                fa = ecx;
                fb = eax;
                if (fa != fb) { pc = 0x44a8a; continue; }
                pc = 0x44a72; continue;
            }
            0x44a72 => {
                eax = (((((m.read16(edx.wrapping_add(esi.wrapping_mul(0x8)).wrapping_add(0x2)) as i16)) as i32)) as u32);
                ebp = eax;
                ebp = ((ebp as i32).wrapping_mul((0x2c) as i32)) as u32;
                fa = m.read32(ebx.wrapping_add(ebp).wrapping_add(0x24));
                fb = edi;
                if ((fa & fb) == 0) { pc = 0x44a8a; continue; }
                pc = 0x44a82; continue;
            }
            0x44a82 => {
                edx = m.read32(esp.wrapping_add(0x18));
                push!(edx);
                push!(eax);
                pc = 0x44aa6; continue;
            }
            0x44a8a => {
                eax = (((((m.read16(edx.wrapping_add(esi.wrapping_mul(0x8)).wrapping_add(0x2)) as i16)) as i32)) as u32);
                fa = eax;
                fb = m.read32(esp.wrapping_add(0x14));
                if (fa != fb) { pc = 0x44ab4; continue; }
                pc = 0x44a95; continue;
            }
            0x44a95 => {
                eax = ecx;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                fa = m.read32(eax.wrapping_add(ebx).wrapping_add(0x24));
                fb = edi;
                if ((fa & fb) == 0) { pc = 0x44ab4; continue; }
                pc = 0x44aa0; continue;
            }
            0x44aa0 => {
                edx = m.read32(esp.wrapping_add(0x18));
                push!(edx);
                push!(ecx);
                pc = 0x44aa6; continue;
            }
            0x44aa6 => {
                crate::audio_lane::set_return(0x44aab);
                call!(0x44a20);
                pc = 0x44aab; continue;
            }
            0x44aab => {
                edx = m.read32(0x6b98c0);
                esp = esp.wrapping_add(0x8);
                pc = 0x44ab4; continue;
            }
            0x44ab4 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x44a5e; continue; }
                pc = 0x44ab8; continue;
            }
            0x44ab8 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x44ab9; continue;
            }
            0x44ab9 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045090: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45090<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45090;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45090, previous, pc); previous = pc; }
        match pc {
            0x45090 => {
                eax = m.read32(0x6b990c);
                push!(ebx);
                ebx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x450d2; continue; }
                pc = 0x4509c; continue;
            }
            0x4509c => {
                push!(esi);
                push!(edi);
                edi = 0;
                pc = 0x450a0; continue;
            }
            0x450a0 => {
                eax = m.read32(0x6b9908);
                esi = edi.wrapping_add(eax);
                eax = m.read32(esi.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x450c3; continue; }
                pc = 0x450af; continue;
            }
            0x450af => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x450c3; continue; }
                pc = 0x450b3; continue;
            }
            0x450b3 => {
                push!(eax);
                crate::audio_lane::set_return(0x450b9);
                call!(0x45460);
                pc = 0x450b9; continue;
            }
            0x450b9 => {
                esp = esp.wrapping_add(0x4);
                m.write32(esi.wrapping_add(0x4), 0xffffffffu32);
                pc = 0x450c3; continue;
            }
            0x450c3 => {
                eax = m.read32(0x6b990c);
                ebx = ebx.wrapping_add(1);
                edi = edi.wrapping_add(0x64);
                fa = ebx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x450a0; continue; }
                pc = 0x450d0; continue;
            }
            0x450d0 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x450d2; continue;
            }
            0x450d2 => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000450E0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f450e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x450e0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x450e0, previous, pc); previous = pc; }
        match pc {
            0x450e0 => {
                eax = gs::get_level(m);
                push!(esi);
                esi = 0;
                fa = eax;
                fb = ((((0x64i32)) as u32) & 0xffffffffu32);
                m.write32(0x6b9928, esi);
                m.write32(0x6b9924, esi);
                m.write32(0x6b9920, esi);
                m.write32(0x6b991c, esi);
                m.write32(0x6b9918, esi);
                m.write32(0x6b9914, esi);
                m.write32(0x6b9910, esi);
                m.write32(0x6b990c, esi);
                if (fa == fb) { pc = 0x4515a; continue; }
                pc = 0x4511d; continue;
            }
            0x4511d => {
                crate::audio_lane::set_return(0x45122);
                call!(0x43c50);
                pc = 0x45122; continue;
            }
            0x45122 => {
                eax = m.read32(0x6b990c);
                fa = eax;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x45154; continue; }
                pc = 0x4512b; continue;
            }
            0x4512b => {
                fa = eax;
                fb = ((((0x3e8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x4514a; continue; }
                pc = 0x45132; continue;
            }
            0x45132 => {
                fa = m.read32(0x6b9914);
                fb = ((((0x3e8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x4514a; continue; }
                pc = 0x4513e; continue;
            }
            0x4513e => {
                fa = m.read32(0x6b991c);
                fb = ((((0x1f4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x45154; continue; }
                pc = 0x4514a; continue;
            }
            0x4514a => {
                push!(0xffffffffu32);
                crate::audio_lane::set_return(0x45151);
                call!(0x389b0);
                pc = 0x45151; continue;
            }
            0x45151 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x45154; continue;
            }
            0x45154 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x43b70); break 'run;  // direct tail jump
            }
            0x4515a => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045160: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45160<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45160;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45160, previous, pc); previous = pc; }
        match pc {
            0x45160 => {
                crate::audio_lane::set_return(0x45165);
                call!(0x45090);
                pc = 0x45165; continue;
            }
            0x45165 => {
                ecx = m.read32(0x6b7aa8);
                eax = 0;
                fa = ecx;
                fb = eax;
                if (fa != fb) { pc = 0x4518a; continue; }
                pc = 0x45171; continue;
            }
            0x45171 => {
                m.write32(0x6b9908, eax);
                m.write32(0x6b98b8, eax);
                m.write32(0x6b98bc, eax);
                m.write32(0x6b98c0, eax);
                m.write32(0x6b990c, eax);
                pc = 0x4518a; continue;
            }
            0x4518a => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000451F0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f451f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x451f0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x451f0, previous, pc); previous = pc; }
        match pc {
            0x451f0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                m.write32(0x6d5eb0, eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000454D0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f454d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x454d0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x454d0, previous, pc); previous = pc; }
        match pc {
            0x454d0 => {
                tail_to!(0x28070); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045500: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45500<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45500;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45500, previous, pc); previous = pc; }
        match pc {
            0x45500 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x45532; continue; }
                pc = 0x45508; continue;
            }
            0x45508 => {
                fa = ecx;
                fb = ((((0x20000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x45529; continue; }
                pc = 0x45510; continue;
            }
            0x45510 => {
                eax = 0x6d58a4;
                pc = 0x45515; continue;
            }
            0x45515 => {
                fa = ((m.read32(eax.wrapping_add(((0xci32.wrapping_neg()) as u32)))) & 0xffffffffu32);
                fb = ecx;
                if (fa != fb) { pc = 0x4551f; continue; }
                pc = 0x4551a; continue;
            }
            0x4551a => {
                fa = m.read32(eax);
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x45532; continue; }
                pc = 0x4551f; continue;
            }
            0x4551f => {
                eax = eax.wrapping_add(0x40);
                fa = eax;
                fb = ((((0x6d5ee4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x45515; continue; }
                pc = 0x45529; continue;
            }
            0x45529 => {
                m.write32(esp.wrapping_add(0x4), ecx);
                tail_to!(0x28000); break 'run;  // direct tail jump
            }
            0x45532 => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000455D0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f455d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x455d0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x455d0, previous, pc); previous = pc; }
        match pc {
            0x455d0 => {
                eax = 0x6d58a4;
                pc = 0x455e0; continue;
                edi = edi;
                pc = 0x455e0; continue;
            }
            0x455e0 => {
                m.write32(eax, 0);
                eax = eax.wrapping_add(0x40);
                fa = eax;
                fb = ((((0x6d5ee4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x455e0; continue; }
                pc = 0x455f0; continue;
            }
            0x455f0 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045620: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45620<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45620;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45620, previous, pc); previous = pc; }
        match pc {
            0x45620 => {
                tail_to!(0x27fa0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045630: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45630<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45630;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45630, previous, pc); previous = pc; }
        match pc {
            0x45630 => {
                tail_to!(0x279c0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045640: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45640<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45640;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45640, previous, pc); previous = pc; }
        match pc {
            0x45640 => {
                eax = 0x6d58a4;
                pc = 0x45650; continue;
                edi = edi;
                pc = 0x45650; continue;
            }
            0x45650 => {
                m.write32(eax, 0);
                eax = eax.wrapping_add(0x40);
                fa = eax;
                fb = ((((0x6d5ee4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x45650; continue; }
                pc = 0x45660; continue;
            }
            0x45660 => {
                tail_to!(0x27f70); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045670: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45670<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45670;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45670, previous, pc); previous = pc; }
        match pc {
            0x45670 => {
                push!(ecx);
                edx = gs::get_level(m);
                fa = edx;
                fb = ((((0x66i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x456d1; continue; }
                pc = 0x4567c; continue;
            }
            0x4567c => {
                fa = edx;
                fb = ((((0x65i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x456d1; continue; }
                pc = 0x45681; continue;
            }
            0x45681 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(0x4ca9b4);
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                ecx = ecx.wrapping_add(eax.wrapping_mul(0x4));
                eax = m.read32(ecx.wrapping_add(0x8));
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x456b1; continue; }
                pc = 0x45698; continue;
            }
            0x45698 => {
                fa = ((((gs::get_mode(m))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x456aa; continue; }
                pc = 0x456a1; continue;
            }
            0x456a1 => {
                fa = ((((gs::get_flags_low8(m))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x45702; continue; }
                pc = 0x456aa; continue;
            }
            0x456aa => {
                eax = 0x20;
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x456b1 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x456c0; continue; }
                pc = 0x456b5; continue;
            }
            0x456b5 => {
                push!(esi);
                esi = eax;
                esi = (((((esi as i32)).wrapping_shr((((((((0x18i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                fa = esi;
                fb = edx;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa != fb) { pc = 0x45702; continue; }
                pc = 0x456c0; continue;
            }
            0x456c0 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x3i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x456ca; continue; }
                pc = 0x456c4; continue;
            }
            0x456c4 => {
                fa = m.read32(ecx.wrapping_add(0xc));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45702; continue; }
                pc = 0x456ca; continue;
            }
            0x456ca => {
                eax = 0x4;
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x456d1 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(0x4ca9b4);
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                eax = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x8));
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                m.write32(esp, eax);
                if ((fa & fb) == 0) { pc = 0x45702; continue; }
                pc = 0x456e9; continue;
            }
            0x456e9 => {
                ecx = 0;
                eax = eax.wrapping_shr((((((((0x18i32)) as u32) & 0x1fu32))) as u32 & 31));
                fa = edx;
                fb = ((((0x66i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ecx = ecx.wrapping_add(0x7f);
                fa = eax;
                fb = ecx;
                if (fa != fb) { pc = 0x45702; continue; }
                pc = 0x456fb; continue;
            }
            0x456fb => {
                eax = 0x80000004u32;
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x45702 => {
                eax = 0;
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045780: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45780<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45780;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45780, previous, pc); previous = pc; }
        match pc {
            0x45780 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                m.write32(0x4caa04, xmm0.lanes[0]);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00045790: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45790<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45790;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45790, previous, pc); previous = pc; }
        match pc {
            0x45790 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(0x3ccccccd);
                push!(eax);
                push!(0x6b9930);
                crate::audio_lane::set_return(0x457a4);
                call!(0x1515e0);
                pc = 0x457a4; continue;
            }
            0x457a4 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000457B0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f457b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x457b0;
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
        { crate::pose::cover(0x457b0, previous, pc); previous = pc; }
        match pc {
            0x457b0 => {
                eax = m.read32(0x5236b8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x457cb; continue; }
                pc = 0x457b9; continue;
            }
            0x457b9 => {
                xmm1.lanes[0] = (((m.read32(0x5236c0)) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                pc = 0x457ce; continue;
            }
            0x457cb => {
                xmm1 = Xmm::default();
                pc = 0x457ce; continue;
            }
            0x457ce => {
                eax = m.read32(esp.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x4caa04));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                if (fa != fb) { pc = 0x457f9; continue; }
                pc = 0x457e2; continue;
            }
            0x457e2 => {
                xmm1 = Xmm::scalar(m.read32(0x6b9930));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x4))) as f64));
                break 'run;
            }
            0x457f9 => {
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x6b9930))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm2.lanes[0]);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x4))) as f64));
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000458B0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f458b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x458b0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x458b0, previous, pc); previous = pc; }
        match pc {
            0x458b0 => {
                push!(esi);
                esi = 0x6d58a4;
                pc = 0x458b6; continue;
            }
            0x458b6 => {
                eax = m.read32(esi);
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x45910; continue; }
                pc = 0x458bb; continue;
            }
            0x458bb => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x458ed; continue; }
                pc = 0x458be; continue;
            }
            0x458be => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0x45916; continue; }
                pc = 0x458c1; continue;
            }
            0x458c1 => {
                eax = esi.wrapping_add(((0x3ci32.wrapping_neg()) as u32));
                push!(eax);
                crate::audio_lane::set_return(0x458ca);
                call!(0x45820);
                pc = 0x458ca; continue;
            }
            0x458ca => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esi.wrapping_add(((0x8i32.wrapping_neg()) as u32)), eax);
                if (fas < fbs) { pc = 0x458e5; continue; }
                pc = 0x458d4; continue;
            }
            0x458d4 => {
                eax = (eax & 0x40000000);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & 0x3);
                eax = eax.wrapping_add(1);
                m.write32(esi, eax);
                pc = 0x45916; continue;
            }
            0x458e5 => {
                m.write32(esi, 0x1);
                pc = 0x45916; continue;
            }
            0x458ed => {
                edx = m.read32(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)));
                ecx = m.read32(0x4e7944);
                edx = edx.wrapping_sub(ecx);
                eax = edx;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)), edx);
                if (fas > fbs) { pc = 0x45916; continue; }
                pc = 0x45901; continue;
            }
            0x45901 => {
                m.write32(esi, 0x3);
                m.write32(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)), 0);
                pc = 0x45916; continue;
            }
            0x45910 => {
                m.write32(esi, 0);
                pc = 0x45916; continue;
            }
            0x45916 => {
                esi = esi.wrapping_add(0x40);
                fa = esi;
                fb = ((((0x6d5ee4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x458b6; continue; }
                pc = 0x45921; continue;
            }
            0x45921 => {
                crate::audio_lane::set_return(0x45926);
                call!(0x389b0);
                pc = 0x45926; continue;
            }
            0x45926 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x28760); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045930: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45930<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45930;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45930, previous, pc); previous = pc; }
        match pc {
            0x45930 => {
                eax = 0;
                pc = 0x45932; continue;
            }
            0x45932 => {
                ecx = m.read32(0x4ca9b4);
                m.write32(eax.wrapping_add(ecx).wrapping_add(0xc), 0);
                eax = eax.wrapping_add(0x14);
                fa = eax;
                fb = ((((0x1bda0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x45932; continue; }
                pc = 0x4594a; continue;
            }
            0x4594a => {
                crate::audio_lane::set_return(0x4594f);
                call!(0x27980);
                pc = 0x4594f; continue;
            }
            0x4594f => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(0x6d5eb0, 0);
                m.write32(0x4caa04, xmm0.lanes[0]);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00045970: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45970<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45970;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45970, previous, pc); previous = pc; }
        match pc {
            0x45970 => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(0x4caa04, xmm0.lanes[0]);
                crate::audio_lane::set_return(0x45985);
                call!(0x389b0);
                pc = 0x45985; continue;
            }
            0x45985 => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(0x6b9930, xmm0.lanes[0]);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000459A0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f459a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x459a0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x459a0, previous, pc); previous = pc; }
        match pc {
            0x459a0 => {
                tail_to!(0x28900); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000459B0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f459b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x459b0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x459b0, previous, pc); previous = pc; }
        match pc {
            0x459b0 => {
                push!(0x1);
                push!(0);
                push!(0x4785b8);
                crate::audio_lane::set_return(0x459be);
                call!(0x622d0);
                pc = 0x459be; continue;
            }
            0x459be => {
                esp = esp.wrapping_add(0xc);
                crate::audio_lane::set_return(0x459c6);
                call!(0x28ae0);
                pc = 0x459c6; continue;
            }
            0x459c6 => {
                tail_to!(0x45eb0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000459F0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f459f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x459f0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x459f0, previous, pc); previous = pc; }
        match pc {
            0x459f0 => {
                tail_to!(0x28980); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045A50: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45a50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45a50;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45a50, previous, pc); previous = pc; }
        match pc {
            0x45a50 => {
                eax = m.read32(0x6d5eb8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045A70: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45a70<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45a70;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45a70, previous, pc); previous = pc; }
        match pc {
            0x45a70 => {
                eax = gs::get_level(m);
                push!(eax);
                crate::audio_lane::set_return(0x45a7b);
                call!(0x1ceb00);
                pc = 0x45a7b; continue;
            }
            0x45a7b => {
                ecx = m.read32(0x5236b8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45a8e; continue; }
                pc = 0x45a88; continue;
            }
            0x45a88 => {
                ecx = m.read32(0x5236b4);
                pc = 0x45a8e; continue;
            }
            0x45a8e => {
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045AB0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45ab0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45ab0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45ab0, previous, pc); previous = pc; }
        match pc {
            0x45ab0 => {
                eax = gs::get_level(m);
                push!(eax);
                m.write32(0x6d5eb8, 0x1);
                crate::audio_lane::set_return(0x45ac5);
                call!(0x1ceb00);
                pc = 0x45ac5; continue;
            }
            0x45ac5 => {
                ecx = m.read32(0x5236b8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45ad8; continue; }
                pc = 0x45ad2; continue;
            }
            0x45ad2 => {
                ecx = m.read32(0x5236b4);
                pc = 0x45ad8; continue;
            }
            0x45ad8 => {
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45ae8; continue; }
                pc = 0x45ae3; continue;
            }
            0x45ae3 => {
                tail_to!(0x28a40); break 'run;  // direct tail jump
            }
            0x45ae8 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045AF0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45af0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45af0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45af0, previous, pc); previous = pc; }
        match pc {
            0x45af0 => {
                eax = gs::get_level(m);
                push!(eax);
                m.write32(0x6d5eb8, 0);
                crate::audio_lane::set_return(0x45b05);
                call!(0x1ceb00);
                pc = 0x45b05; continue;
            }
            0x45b05 => {
                ecx = m.read32(0x5236b8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45b18; continue; }
                pc = 0x45b12; continue;
            }
            0x45b12 => {
                ecx = m.read32(0x5236b4);
                pc = 0x45b18; continue;
            }
            0x45b18 => {
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45b28; continue; }
                pc = 0x45b23; continue;
            }
            0x45b23 => {
                tail_to!(0x28a50); break 'run;  // direct tail jump
            }
            0x45b28 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045B30: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45b30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45b30;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45b30, previous, pc); previous = pc; }
        match pc {
            0x45b30 => {
                eax = gs::get_level(m);
                push!(eax);
                crate::audio_lane::set_return(0x45b3b);
                call!(0x1ceb00);
                pc = 0x45b3b; continue;
            }
            0x45b3b => {
                ecx = m.read32(0x5236b8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45b4e; continue; }
                pc = 0x45b48; continue;
            }
            0x45b48 => {
                ecx = m.read32(0x5236b4);
                pc = 0x45b4e; continue;
            }
            0x45b4e => {
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45b5e; continue; }
                pc = 0x45b59; continue;
            }
            0x45b59 => {
                tail_to!(0x28a80); break 'run;  // direct tail jump
            }
            0x45b5e => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045B60: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45b60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45b60;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45b60, previous, pc); previous = pc; }
        match pc {
            0x45b60 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                fa = esi;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x45b74; continue; }
                pc = 0x45b6a; continue;
            }
            0x45b6a => {
                fa = esi;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x45b74; continue; }
                pc = 0x45b6f; continue;
            }
            0x45b6f => {
                fa = esi;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45b9f; continue; }
                pc = 0x45b74; continue;
            }
            0x45b74 => {
                eax = m.read32(0x5236b8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45b81; continue; }
                pc = 0x45b7d; continue;
            }
            0x45b7d => {
                eax = 0;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x45b81 => {
                xmm0.lanes[0] = (((m.read32(0x5236c0)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4785c8))).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x45b9f => {
                fa = gs::get_level(m);
                fb = ((((0x53i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x45baf; continue; }
                pc = 0x45ba8; continue;
            }
            0x45ba8 => {
                eax = m.read32(0x5358b8);
                pc = 0x45be8; continue;
            }
            0x45baf => {
                fa = ((((gs::get_mode(m))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x45bdb; continue; }
                pc = 0x45bb8; continue;
            }
            0x45bb8 => {
                fa = ((((gs::get_flags_low8(m))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x45bdb; continue; }
                pc = 0x45bc1; continue;
            }
            0x45bc1 => {
                crate::audio_lane::set_return(0x45bc6);
                call!(0x6a590);
                pc = 0x45bc6; continue;
            }
            0x45bc6 => {
                eax = ((eax as i32).wrapping_mul((0xe) as i32)) as u32;
                eax = eax.wrapping_add(esi);
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x535ea8));
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x535858));
                pc = 0x45be8; continue;
            }
            0x45bdb => {
                ecx = m.read32(0x4cb548);
                eax = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x535858));
                pc = 0x45be8; continue;
            }
            0x45be8 => {
                xmm0.lanes[0] = (((m.read32(0x5236bc)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                edx = gs::get_level(m);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4785c8))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d1c))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                push!(edx);
                esi = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                crate::audio_lane::set_return(0x45c20);
                call!(0x1ceb00);
                pc = 0x45c20; continue;
            }
            0x45c20 => {
                ecx = m.read32(0x5236b8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45c33; continue; }
                pc = 0x45c2d; continue;
            }
            0x45c2d => {
                ecx = m.read32(0x5236b4);
                pc = 0x45c33; continue;
            }
            0x45c33 => {
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x45c40; continue; }
                pc = 0x45c3e; continue;
            }
            0x45c3e => {
                esi = 0;
                pc = 0x45c40; continue;
            }
            0x45c40 => {
                fa = gs::get_level(m);
                fb = ((((0x65i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45c52; continue; }
                pc = 0x45c49; continue;
            }
            0x45c49 => {
                eax = esi;
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                esi = eax;
                pc = 0x45c52; continue;
            }
            0x45c52 => {
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00045D80: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45d80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45d80;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45d80, previous, pc); previous = pc; }
        match pc {
            0x45d80 => {
                push!(esi);
                esi = eax;
                fa = esi;
                fb = ((((0x4bi32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x45d97; continue; }
                pc = 0x45d88; continue;
            }
            0x45d88 => {
                fa = esi;
                fb = ((((0xaei32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x45d97; continue; }
                pc = 0x45d90; continue;
            }
            0x45d90 => {
                crate::audio_lane::set_return(0x45d95);
                call!(0x350c60);
                pc = 0x45d95; continue;
            }
            0x45d95 => {
                esi = esi.wrapping_add(eax);
                pc = 0x45d97; continue;
            }
            0x45d97 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = edi;
                if (fa != fb) { pc = 0x45dbe; continue; }
                pc = 0x45d9d; continue;
            }
            0x45d9d => {
                eax = 0x1;
                if !(((((((((ecx) as u8))) as u32) & 0x1fu32))) != 0) { fa = if ((fa == fb)) { 0x0u32 } else { (if ((((((((fas)) as u32).wrapping_sub((((fbs)) as u32))).wrapping_shr((0x1f & 31))) != 0))) { 0x80000000u32 } else { 0x1u32 }) }; fas = ((fa) as i32); }
                eax = eax.wrapping_shl((((((((((ecx) as u8))) as u32) & 0x1fu32))) as u32 & 31));
                if (((((((((ecx) as u8))) as u32) & 0x1fu32))) != 0) { fa = eax; fas = (fa as i32); }
                ecx = m.read32(0x4cb54c);
                push!(esi);
                push!(edi);
                ecx = (ecx | eax);
                m.write32(0x4cb54c, ecx);
                crate::audio_lane::set_return(0x45db9);
                call!(0x28890);
                pc = 0x45db9; continue;
            }
            0x45db9 => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x45dbe => {
                eax = m.read32(0x4cb54c);
                edx = 0x1;
                if !(((((((((ecx) as u8))) as u32) & 0x1fu32))) != 0) { fa = if ((fa == fb)) { 0x0u32 } else { (if ((((((((fas)) as u32).wrapping_sub((((fbs)) as u32))).wrapping_shr((0x1f & 31))) != 0))) { 0x80000000u32 } else { 0x1u32 }) }; fas = ((fa) as i32); }
                edx = edx.wrapping_shl((((((((((ecx) as u8))) as u32) & 0x1fu32))) as u32 & 31));
                if (((((((((ecx) as u8))) as u32) & 0x1fu32))) != 0) { fa = edx; fas = (fa as i32); }
                push!(esi);
                push!(edi);
                edx = !edx;
                eax = (eax & edx);
                m.write32(0x4cb54c, eax);
                crate::audio_lane::set_return(0x45dda);
                call!(0x28890);
                pc = 0x45dda; continue;
            }
            0x45dda => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045DE0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45de0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45de0;
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
        { crate::pose::cover(0x45de0, previous, pc); previous = pc; }
        match pc {
            0x45de0 => {
                push!(ecx);
                fa = eax;
                fb = ((0xfffffffeu32) & 0xffffffffu32);
                m.write32(0x4cb548, eax);
                if (fa == fb) { pc = 0x45df6; continue; }
                pc = 0x45deb; continue;
            }
            0x45deb => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45e2f; continue; }
                pc = 0x45df0; continue;
            }
            0x45df0 => {
                eax = m.read32(esp.wrapping_add(0x8));
                pc = 0x45e2a; continue;
            }
            0x45df6 => {
                crate::audio_lane::set_return(0x45dfb);
                call!(0x153b40);
                pc = 0x45dfb; continue;
            }
            0x45dfb => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp, eax);
                fpu.push((m.read32(esp) as i32 as f64));
                if (fas >= fbs) { pc = 0x45e0b; continue; }
                pc = 0x45e05; continue;
            }
            0x45e05 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x45e0b; continue;
            }
            0x45e0b => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785cc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475cd4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                crate::audio_lane::set_return(0x45e22);
                call!(0x3c848c);
                pc = 0x45e22; continue;
            }
            0x45e22 => {
                eax = eax.wrapping_add(1);
                eax = m.read32(eax.wrapping_mul(0x8).wrapping_add(0x537210));
                pc = 0x45e2a; continue;
            }
            0x45e2a => {
                m.write32(0x4cb548, eax);
                pc = 0x45e2f; continue;
            }
            0x45e2f => {
                push!(edi);
                edi = 0;
                crate::audio_lane::set_return(0x45e37);
                call!(0x45d80);
                pc = 0x45e37; continue;
            }
            0x45e37 => {
                eax = m.read32(esp.wrapping_add(0x10));
                edi = 0x2;
                crate::audio_lane::set_return(0x45e45);
                call!(0x45d80);
                pc = 0x45e45; continue;
            }
            0x45e45 => {
                eax = m.read32(esp.wrapping_add(0x14));
                edi = 0x1;
                crate::audio_lane::set_return(0x45e53);
                call!(0x45d80);
                pc = 0x45e53; continue;
            }
            0x45e53 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045E60: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45e60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45e60;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45e60, previous, pc); previous = pc; }
        match pc {
            0x45e60 => {
                eax = m.read32(0x784014);
                push!(esi);
                push!(0xc77668);
                push!(eax);
                crate::audio_lane::set_return(0x45e71);
                call!(0x32a5b0);
                pc = 0x45e71; continue;
            }
            0x45e71 => {
                esi = eax;
                eax = m.read32(0x761404);
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x45e9d; continue; }
                pc = 0x45e7f; continue;
            }
            0x45e7f => {
                fa = ((((gs::get_mode(m))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x45e9d; continue; }
                pc = 0x45e88; continue;
            }
            0x45e88 => {
                crate::audio_lane::set_return(0x45e8d);
                call!(0x69640);
                pc = 0x45e8d; continue;
            }
            0x45e8d => {
                eax = m.read32(eax.wrapping_add(0x114));
                fa = m.read32(eax.wrapping_mul(0x8).wrapping_add(0x537210));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x45ea1; continue; }
                pc = 0x45e9d; continue;
            }
            0x45e9d => {
                eax = (((((m.read16(esi.wrapping_add(0xe)) as i16)) as i32)) as u32);
                pc = 0x45ea1; continue;
            }
            0x45ea1 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00045EB0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f45eb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x45eb0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x45eb0, previous, pc); previous = pc; }
        match pc {
            0x45eb0 => {
                push!(esi);
                push!(0x1);
                push!(0);
                esi = (esi | 0xffffffffu32);
                push!(0x4785b8);
                m.write32(0x4cb54c, esi);
                crate::audio_lane::set_return(0x45ec8);
                call!(0x622d0);
                pc = 0x45ec8; continue;
            }
            0x45ec8 => {
                esp = esp.wrapping_add(0xc);
                crate::audio_lane::set_return(0x45ed0);
                call!(0x6a590);
                pc = 0x45ed0; continue;
            }
            0x45ed0 => {
                ecx = gs::get_level(m);
                fa = ecx;
                fb = ((((0x64i32)) as u32) & 0xffffffffu32);
                m.write32(0x6d5ebc, eax);
                if (fa == fb) { pc = 0x45f7a; continue; }
                pc = 0x45ee4; continue;
            }
            0x45ee4 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x45fbf; continue; }
                pc = 0x45eec; continue;
            }
            0x45eec => {
                fa = ecx;
                fb = ((((0x64i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x45f50; continue; }
                pc = 0x45ef1; continue;
            }
            0x45ef1 => {
                fa = ((((gs::get_mode(m))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                ecx = (ecx & 0xffff_ff00) | ((((gs::get_flags_low8(m)) as u32)) & 0xff);
                if (fa != fb) { pc = 0x45f05; continue; }
                pc = 0x45f00; continue;
            }
            0x45f00 => {
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x45f50; continue; }
                pc = 0x45f05; continue;
            }
            0x45f05 => {
                eax = ((eax as i32).wrapping_mul((0x38) as i32)) as u32;
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                push!(0x2b);
                if ((fa & fb) == 0) { pc = 0x45f31; continue; }
                pc = 0x45f0f; continue;
            }
            0x45f0f => {
                eax = m.read32(eax.wrapping_add(0x535ea8));
                push!(0xc1);
                push!(eax);
                crate::audio_lane::set_return(0x45f20);
                call!(0x22d780);
                pc = 0x45f20; continue;
            }
            0x45f20 => {
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x5371b0));
                crate::audio_lane::set_return(0x45f2c);
                call!(0x45de0);
                pc = 0x45f2c; continue;
            }
            0x45f2c => {
                esp = esp.wrapping_add(0xc);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x45f31 => {
                ecx = m.read32(eax.wrapping_add(0x535ea8));
                push!(0x33);
                push!(ecx);
                crate::audio_lane::set_return(0x45f3f);
                call!(0x45a60);
                pc = 0x45f3f; continue;
            }
            0x45f3f => {
                eax = m.read32(eax.wrapping_mul(0x8).wrapping_add(0x537210));
                crate::audio_lane::set_return(0x45f4b);
                call!(0x45de0);
                pc = 0x45f4b; continue;
            }
            0x45f4b => {
                esp = esp.wrapping_add(0xc);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x45f50 => {
                push!(edi);
                edi = 0;
                pc = 0x45f60; continue;
            }
            0x45f55 => {
                eax = m.read32(0x6d5ebc);
                pc = 0x45f60; continue;
            }
            0x45f60 => {
                eax = ((eax as i32).wrapping_mul((0xe) as i32)) as u32;
                eax = eax.wrapping_add(edi);
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x535ea8));
                crate::audio_lane::set_return(0x45f71);
                call!(0x45d80);
                pc = 0x45f71; continue;
            }
            0x45f71 => {
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = ((((0xei32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x45f55; continue; }
                pc = 0x45f77; continue;
            }
            0x45f77 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x45f7a => {
                crate::audio_lane::set_return(0x45f7f);
                call!(0x45e60);
                pc = 0x45f7f; continue;
            }
            0x45f7f => {
                fa = eax;
                fb = esi;
                if (fa == fb) { pc = 0x45f92; continue; }
                pc = 0x45f83; continue;
            }
            0x45f83 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x45f90; continue; }
                pc = 0x45f87; continue;
            }
            0x45f87 => {
                esi = m.read32(eax.wrapping_mul(0x8).wrapping_add(0x537210));
                pc = 0x45f92; continue;
            }
            0x45f90 => {
                esi = 0;
                pc = 0x45f92; continue;
            }
            0x45f92 => {
                edx = m.read32(0x784014);
                push!(0x2b);
                push!(0x33);
                push!(0xc77668);
                push!(edx);
                crate::audio_lane::set_return(0x45fa7);
                call!(0x32a5b0);
                pc = 0x45fa7; continue;
            }
            0x45fa7 => {
                eax = ((m.read16(eax.wrapping_add(0xa))) as u16 as u32);
                push!(eax);
                crate::audio_lane::set_return(0x45fb1);
                call!(0x311850);
                pc = 0x45fb1; continue;
            }
            0x45fb1 => {
                esp = esp.wrapping_add(0xc);
                push!(eax);
                eax = esi;
                crate::audio_lane::set_return(0x45fbc);
                call!(0x45de0);
                pc = 0x45fbc; continue;
            }
            0x45fbc => {
                esp = esp.wrapping_add(0xc);
                pc = 0x45fbf; continue;
            }
            0x45fbf => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00046270: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f46270<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x46270;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x46270, previous, pc); previous = pc; }
        match pc {
            0x46270 => {
                eax = m.read32(esp.wrapping_add(0x4));
                m.write32(0x6d5ec0, eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000462C0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f462c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x462c0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x462c0, previous, pc); previous = pc; }
        match pc {
            0x462c0 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                m.write32(0x4d1848, xmm0.lanes[0]);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000462D0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f462d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x462d0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x462d0, previous, pc); previous = pc; }
        match pc {
            0x462d0 => {
                eax = m.read32(0x4d1854);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00046350: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f46350<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x46350;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x46350, previous, pc); previous = pc; }
        match pc {
            0x46350 => {
                push!(ecx);
                xmm0 = Xmm::scalar(m.read32(0x4d1840));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d40))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d4c))).to_bits();
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                crate::audio_lane::set_return(0x46374);
                call!(0x45790);
                pc = 0x46374; continue;
            }
            0x46374 => {
                esp = esp.wrapping_add(0x4);
                crate::audio_lane::set_return(0x4637c);
                call!(0x45a50);
                pc = 0x4637c; continue;
            }
            0x4637c => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x46395; continue; }
                pc = 0x46380; continue;
            }
            0x46380 => {
                eax = m.read32(0x6d5ec0);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x4649b; continue; }
                pc = 0x4638d; continue;
            }
            0x4638d => {
                esp = esp.wrapping_add(0x4);
                tail_to!(0x45af0); break 'run;  // direct tail jump
            }
            0x46395 => {
                push!(esi);
                crate::audio_lane::set_return(0x4639b);
                call!(0x45aa0);
                pc = 0x4639b; continue;
            }
            0x4639b => {
                ecx = m.read32(0x4d184c);
                esi = eax;
                eax = m.read32(0x4d1850);
                fa = eax;
                fb = ecx;
                if (fa != fb) { pc = 0x463c8; continue; }
                pc = 0x463ac; continue;
            }
            0x463ac => {
                ecx = m.read32(0x6d5ec0);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x463c8; continue; }
                pc = 0x463b6; continue;
            }
            0x463b6 => {
                xmm0 = Xmm::scalar(m.read32(0x4d1848));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4d1840))).to_bits();
                pc = 0x463cb; continue;
            }
            0x463c8 => {
                xmm0 = Xmm::default();
                pc = 0x463cb; continue;
            }
            0x463cb => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                if (fa == fb) { pc = 0x463fc; continue; }
                pc = 0x463d5; continue;
            }
            0x463d5 => {
                eax = m.read32(0x4d1838);
                ecx = m.read32(esp.wrapping_add(0x4));
                push!(eax);
                push!(ecx);
                push!(0x4d183c);
                crate::audio_lane::set_return(0x463ea);
                call!(0x1515e0);
                pc = 0x463ea; continue;
            }
            0x463ea => {
                eax = m.read32(0x4d1850);
                xmm0 = Xmm::scalar(m.read32(0x4d183c));
                esp = esp.wrapping_add(0xc);
                pc = 0x46404; continue;
            }
            0x463fc => {
                m.write32(0x4d183c, xmm0.lanes[0]);
                pc = 0x46404; continue;
            }
            0x46404 => {
                xmm1 = Xmm::scalar(m.read32(0x475d1c));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x46442; continue; }
                pc = 0x46411; continue;
            }
            0x46411 => {
                fa = eax;
                fb = m.read32(0x4d184c);
                if (fa == fb) { pc = 0x4642c; continue; }
                pc = 0x46419; continue;
            }
            0x46419 => {
                push!(0x1);
                push!(eax);
                crate::audio_lane::set_return(0x46421);
                call!(0x45a00);
                pc = 0x46421; continue;
            }
            0x46421 => {
                xmm0 = Xmm::scalar(m.read32(0x4d183c));
                esp = esp.wrapping_add(0x8);
                pc = 0x4642c; continue;
            }
            0x4642c => {
                eax = m.read32(0x6d5ec0);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x46442; continue; }
                pc = 0x46435; continue;
            }
            0x46435 => {
                crate::audio_lane::set_return(0x4643a);
                call!(0x45ab0);
                pc = 0x4643a; continue;
            }
            0x4643a => {
                xmm0 = Xmm::scalar(m.read32(0x4d183c));
                pc = 0x46442; continue;
            }
            0x46442 => {
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x4d1844))).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x4));
                xmm1 = Xmm::scalar(m.read32(0x4785d8));
                edx = (edx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x4), edx);
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esp.wrapping_add(0x4))) as f64);
                if ((fca > fcb)) { pc = 0x4649a; continue; }
                pc = 0x46470; continue;
            }
            0x46470 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x4649a; continue; }
                pc = 0x46474; continue;
            }
            0x46474 => {
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4785c8))).to_bits();
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                crate::audio_lane::set_return(0x46487);
                call!(0x45b30);
                pc = 0x46487; continue;
            }
            0x46487 => {
                xmm0 = Xmm::scalar(m.read32(0x4d183c));
                esp = esp.wrapping_add(0x4);
                m.write32(0x4d1844, xmm0.lanes[0]);
                pc = 0x4649a; continue;
            }
            0x4649a => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x4649b; continue;
            }
            0x4649b => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000464A0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f464a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x464a0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x464a0, previous, pc); previous = pc; }
        match pc {
            0x464a0 => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                eax = (eax | 0xffffffffu32);
                m.write32(0x7e71f0, xmm0.lanes[0]);
                xmm0 = Xmm::default();
                m.write32(0x6d5ec8, eax);
                m.write32(0x6d5ee4, eax);
                m.write32(0x6d5f00, eax);
                m.write32(0x6d5f1c, eax);
                m.write32(0x6d5f38, eax);
                m.write32(0x6d5f54, eax);
                m.write32(0x6d5f70, eax);
                m.write32(0x6d5f8c, eax);
                m.write32(0x6d5fa8, eax);
                m.write32(0x6d5fc4, eax);
                m.write32(0x6d5fe0, eax);
                m.write32(0x6d5fe4, xmm0.lanes[0]);
                m.write32(0x6d5fe8, xmm0.lanes[0]);
                m.write32(0x6d5fec, eax);
                m.write32(0x6d5ff0, xmm0.lanes[0]);
                m.write32(0x6d5ff4, xmm0.lanes[0]);
                m.write32(0x6d5ff8, eax);
                m.write32(0x6d5ffc, xmm0.lanes[0]);
                m.write32(0x6d6000, xmm0.lanes[0]);
                m.write32(0x6d6004, eax);
                m.write32(0x6d6008, xmm0.lanes[0]);
                m.write32(0x6d600c, xmm0.lanes[0]);
                m.write32(0x6d6010, eax);
                m.write32(0x6d6014, xmm0.lanes[0]);
                m.write32(0x6d6018, xmm0.lanes[0]);
                m.write32(0x6d601c, eax);
                m.write32(0x6d6020, xmm0.lanes[0]);
                m.write32(0x6d6024, xmm0.lanes[0]);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00046610: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f46610<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x46610;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x46610, previous, pc); previous = pc; }
        match pc {
            0x46610 => {
                esp = esp.wrapping_sub(0xc);
                eax = gs::get_players(m);
                xmm0 = Xmm::default();
                push!(esi);
                push!(edi);
                edi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                if (fas <= fbs) { pc = 0x46712; continue; }
                pc = 0x46633; continue;
            }
            0x46633 => {
                esi = 0;
                pc = 0x46640; continue;
                edi = edi;
                pc = 0x46640; continue;
            }
            0x46640 => {
                eax = m.read32(0x7b0c48);
                fa = m.read32(esi.wrapping_add(eax).wrapping_add(0x4));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x466b6; continue; }
                pc = 0x4664c; continue;
            }
            0x4664c => {
                fa = m.read32(esi.wrapping_add(eax).wrapping_add(0x114));
                fb = ((((0x2000i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                if (fa != fb) { pc = 0x46696; continue; }
                pc = 0x46667; continue;
            }
            0x46667 => {
                eax = m.read32(0x7b0c7c);
                push!(eax);
                crate::audio_lane::set_return(0x46672);
                call!(0x36cf00);
                pc = 0x46672; continue;
            }
            0x46672 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x46690; continue; }
                pc = 0x46679; continue;
            }
            0x46679 => {
                ecx = m.read32(eax.wrapping_add(0x7c));
                fa = ((((m.read8(ecx.wrapping_add(0x1a)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x46690; continue; }
                pc = 0x46682; continue;
            }
            0x46682 => {
                xmm0 = Xmm::scalar(m.read32(0x475d04));
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                pc = 0x46690; continue;
            }
            0x46690 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x8)));
                pc = 0x46696; continue;
            }
            0x46696 => {
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                pc = 0x466b6; continue;
            }
            0x466b6 => {
                eax = gs::get_players(m);
                edi = edi.wrapping_add(1);
                esi = esi.wrapping_add(0x1584);
                fa = edi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x46640; continue; }
                pc = 0x466ca; continue;
            }
            0x466ca => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (!(((((fa) & (fb))) as u8).count_ones() % 2 == 0)) { pc = 0x46712; continue; }
                pc = 0x466dd; continue;
            }
            0x466dd => {
                xmm0 = Xmm::scalar(m.read32(0x4e794c));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475ce8))).to_bits();
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(0x7e71f0);
                crate::audio_lane::set_return(0x4670f);
                call!(0x1515e0);
                pc = 0x4670f; continue;
            }
            0x4670f => {
                esp = esp.wrapping_add(0xc);
                pc = 0x46712; continue;
            }
            0x46712 => {
                esi = 0x6d5ecc;
                pc = 0x46717; continue;
            }
            0x46717 => {
                eax = m.read32(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x4678b; continue; }
                pc = 0x4671e; continue;
            }
            0x4671e => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x4)));
                xmm1 = Xmm::scalar(m.read32(esi));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4e7948))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                push!(eax);
                m.write32(esi, xmm1.lanes[0]);
                crate::audio_lane::set_return(0x4673d);
                call!(0x45540);
                pc = 0x4673d; continue;
            }
            0x4673d => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x4674d; continue; }
                pc = 0x46744; continue;
            }
            0x46744 => {
                m.write32(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)), 0xffffffffu32);
                pc = 0x4678b; continue;
            }
            0x4674d => {
                xmm0 = Xmm::default();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi)) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x4676a; continue; }
                pc = 0x46755; continue;
            }
            0x46755 => {
                edx = m.read32(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)));
                push!(edx);
                crate::audio_lane::set_return(0x4675e);
                call!(0x45460);
                pc = 0x4675e; continue;
            }
            0x4675e => {
                esp = esp.wrapping_add(0x4);
                m.write32(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)), 0xffffffffu32);
                pc = 0x4678b; continue;
            }
            0x4676a => {
                fa = ((((m.read8(esi.wrapping_add(0x8)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                eax = m.read32(esi);
                push!(eax);
                if ((fa & fb) == 0) { pc = 0x4677d; continue; }
                pc = 0x46773; continue;
            }
            0x46773 => {
                edx = m.read32(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)));
                ecx = esi.wrapping_add(0xc);
                push!(ecx);
                push!(edx);
                pc = 0x46783; continue;
            }
            0x4677d => {
                ecx = m.read32(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)));
                push!(0);
                push!(ecx);
                pc = 0x46783; continue;
            }
            0x46783 => {
                crate::audio_lane::set_return(0x46788);
                call!(0x45600);
                pc = 0x46788; continue;
            }
            0x46788 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x4678b; continue;
            }
            0x4678b => {
                esi = esi.wrapping_add(0x1c);
                fa = esi;
                fb = ((((0x6d5fe4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x46717; continue; }
                pc = 0x46796; continue;
            }
            0x46796 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x000467A0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f467a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x467a0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x467a0, previous, pc); previous = pc; }
        match pc {
            0x467a0 => {
                push!(ecx);
                xmm0 = Xmm::scalar(m.read32(0x475d5c));
                push!(esi);
                push!(edi);
                edi = 0x6d5fe0;
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                esi = edi;
                pc = 0x467b8; continue;
            }
            0x467b8 => {
                eax = m.read32(esi);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x467d1; continue; }
                pc = 0x467bf; continue;
            }
            0x467bf => {
                xmm0 = Xmm::scalar(m.read32(0x4760d0));
                edi = esi;
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                pc = 0x46821; continue;
            }
            0x467d1 => {
                push!(eax);
                crate::audio_lane::set_return(0x467d7);
                call!(0x45540);
                pc = 0x467d7; continue;
            }
            0x467d7 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x467f6; continue; }
                pc = 0x467de; continue;
            }
            0x467de => {
                xmm0 = Xmm::scalar(m.read32(0x4760d0));
                edi = esi;
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                m.write32(esi, 0xffffffffu32);
                pc = 0x46821; continue;
            }
            0x467f6 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x8)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(esi.wrapping_add(0x8))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x4680f; continue; }
                pc = 0x46802; continue;
            }
            0x46802 => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                edi = esi;
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                pc = 0x4680f; continue;
            }
            0x4680f => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4785e0))).to_bits();
                m.write32(esi.wrapping_add(0x8), xmm0.lanes[0]);
                pc = 0x46821; continue;
            }
            0x46821 => {
                esi = esi.wrapping_add(0xc);
                fa = esi;
                fb = ((((0x6d6028i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x467b8; continue; }
                pc = 0x4682c; continue;
            }
            0x4682c => {
                edx = m.read32(edi);
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                if (fas < fbs) { pc = 0x46860; continue; }
                pc = 0x46837; continue;
            }
            0x46837 => {
                eax = 0;
                ecx = 0x6d5ec8;
                edi = edi;
                pc = 0x46840; continue;
            }
            0x46840 => {
                fa = m.read32(ecx);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x46866; continue; }
                pc = 0x46845; continue;
            }
            0x46845 => {
                ecx = ecx.wrapping_add(0x1c);
                eax = eax.wrapping_add(1);
                fa = ecx;
                fb = ((((0x6d5fe0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x46840; continue; }
                pc = 0x46851; continue;
            }
            0x46851 => {
                push!(edx);
                crate::audio_lane::set_return(0x46857);
                call!(0x45460);
                pc = 0x46857; continue;
            }
            0x46857 => {
                esp = esp.wrapping_add(0x4);
                m.write32(edi, 0xffffffffu32);
                pc = 0x46860; continue;
            }
            0x46860 => {
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x46866 => {
                eax = ((eax as i32).wrapping_mul((0x1c) as i32)) as u32;
                m.write32(eax.wrapping_add(0x6d5ecc), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x4785dc));
                m.write32(eax.wrapping_add(0x6d5ec8), edx);
                m.write32(eax.wrapping_add(0x6d5ed0), xmm0.lanes[0]);
                write16(m, eax.wrapping_add(0x6d5ed4), (0) as u16);
                m.write32(edi, 0xffffffffu32);
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00046B80: Tier 0 candidate; opt-in only.
// portreg: va=0x00046b80 tier=0
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f46b80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x46b80;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x46b80, previous, pc); previous = pc; }
        match pc {
            0x46b80 => {
                push!(edi);
                eax = 0;
                ecx = 0xc;
                edi = 0x6d6028;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                eax = (eax | 0xffffffffu32);
                m.write32(0x6d6028, eax);
                m.write32(0x6d602c, eax);
                m.write32(0x6d6034, eax);
                m.write32(0x6d6038, eax);
                m.write32(0x6d6040, eax);
                m.write32(0x6d6044, eax);
                m.write32(0x6d604c, eax);
                m.write32(0x6d6050, eax);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00046C40: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f46c40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x46c40;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x46c40, previous, pc); previous = pc; }
        match pc {
            0x46c40 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                fa = ebx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x46c7c; continue; }
                pc = 0x46c4a; continue;
            }
            0x46c4a => {
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x14));
                esi = 0x6d6028;
                pc = 0x46c55; continue;
            }
            0x46c55 => {
                fa = m.read32(esi);
                fb = ebx;
                if (fa != fb) { pc = 0x46c6f; continue; }
                pc = 0x46c59; continue;
            }
            0x46c59 => {
                eax = m.read32(esi.wrapping_add(0x4));
                push!(eax);
                crate::audio_lane::set_return(0x46c62);
                call!(0x45460);
                pc = 0x46c62; continue;
            }
            0x46c62 => {
                esp = esp.wrapping_add(0x4);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esi, 0xffffffffu32);
                if (fa == fb) { pc = 0x46c7a; continue; }
                pc = 0x46c6f; continue;
            }
            0x46c6f => {
                esi = esi.wrapping_add(0xc);
                fa = esi;
                fb = ((((0x6d6058i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x46c55; continue; }
                pc = 0x46c7a; continue;
            }
            0x46c7a => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x46c7c; continue;
            }
            0x46c7c => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00046C80: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f46c80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x46c80;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x46c80, previous, pc); previous = pc; }
        match pc {
            0x46c80 => {
                push!(esi);
                esi = 0x6d6028;
                pc = 0x46c86; continue;
            }
            0x46c86 => {
                eax = m.read32(esi.wrapping_add(0x4));
                push!(eax);
                crate::audio_lane::set_return(0x46c8f);
                call!(0x45460);
                pc = 0x46c8f; continue;
            }
            0x46c8f => {
                m.write32(esi, 0xffffffffu32);
                esi = esi.wrapping_add(0xc);
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = ((((0x6d6058i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x46c86; continue; }
                pc = 0x46ca3; continue;
            }
            0x46ca3 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000624A0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f624a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x624a0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x624a0, previous, pc); previous = pc; }
        match pc {
            0x624a0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                m.write32(0x4d19ec, eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000624B0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f624b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x624b0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x624b0, previous, pc); previous = pc; }
        match pc {
            0x624b0 => {
                eax = m.read32(0x4d19ec);
                m.write32(0x4d19ec, 0xffffffffu32);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00063C60: Tier 0 candidate; opt-in only.
// portreg: va=0x00063c60 tier=0
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f63c60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x63c60;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x63c60, previous, pc); previous = pc; }
        match pc {
            0x63c60 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(esp.wrapping_add(0x4));
                edx = m.read32(esp.wrapping_add(0xc));
                m.write32(ecx, eax);
                eax = eax.wrapping_add(edx);
                m.write32(ecx.wrapping_add(0x4), eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00074F00: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f74f00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x74f00;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x74f00, previous, pc); previous = pc; }
        match pc {
            0x74f00 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(esi);
                push!(eax);
                crate::audio_lane::set_return(0x74f0d);
                call!(0x45390);
                pc = 0x74f0d; continue;
            }
            0x74f0d => {
                ecx = m.read32(esp.wrapping_add(0x3c));
                edx = esp.wrapping_add(0x8);
                push!(edx);
                m.write32(esp.wrapping_add(0xc), ecx);
                crate::audio_lane::set_return(0x74f1f);
                call!(0x45820);
                pc = 0x74f1f; continue;
            }
            0x74f1f => {
                esi = eax;
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x74f63; continue; }
                pc = 0x74f28; continue;
            }
            0x74f28 => {
                eax = m.read32(0x7018b0);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x74f53; continue; }
                pc = 0x74f31; continue;
            }
            0x74f31 => {
                eax = m.read32(0x4d8d74);
                push!(0);
                push!(0x3d4ccccd);
                ecx = esp.wrapping_add(0x40);
                push!(0x3f800000);
                push!(ecx);
                m.write32(esp.wrapping_add(0x48), eax);
                crate::audio_lane::set_return(0x74f50);
                call!(0x46570);
                pc = 0x74f50; continue;
            }
            0x74f50 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x74f53; continue;
            }
            0x74f53 => {
                m.write32(0x4d8d74, esi);
                m.write32(0x7018b0, 0);
                pc = 0x74f63; continue;
            }
            0x74f63 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000B7BA0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn fb7ba0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xb7ba0;
    let mut jt = 0u32;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0xb7ba0, previous, pc); previous = pc; }
        match pc {
            0xb7ba0 => {
                fa = eax;
                fb = ((((0x465i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0xb7c17; continue; }
                pc = 0xb7ba7; continue;
            }
            0xb7ba7 => {
                if (fa == fb) { pc = 0xb7c11; continue; }
                pc = 0xb7ba9; continue;
            }
            0xb7ba9 => {
                fa = eax;
                fb = ((((0xc2i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0xb7be5; continue; }
                pc = 0xb7bb0; continue;
            }
            0xb7bb0 => {
                fa = eax;
                fb = ((((0xc1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xb7bdf; continue; }
                pc = 0xb7bb7; continue;
            }
            0xb7bb7 => {
                eax = eax.wrapping_add(0xffffff71u32);
                fa = eax;
                fb = ((((0x30i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0xb7c53; continue; }
                pc = 0xb7bc5; continue;
            }
            0xb7bc5 => {
                eax = ((m.read8(eax.wrapping_add(0xb7c74))) as u8 as u32);
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0xb7c64));
                if (jt == 0xb7bd3u32) { pc = 0xb7bd3; continue; }
                if (jt == 0xb7bd9u32) { pc = 0xb7bd9; continue; }
                if (jt == 0xb7bdfu32) { pc = 0xb7bdf; continue; }
                if (jt == 0xb7c53u32) { pc = 0xb7c53; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0xb7bd3 => {
                eax = 0x623;
                break 'run;
            }
            0xb7bd9 => {
                eax = 0x652;
                break 'run;
            }
            0xb7bdf => {
                eax = 0x686;
                break 'run;
            }
            0xb7be5 => {
                eax = eax.wrapping_add(0xfffffd3au32);
                fa = eax;
                fb = ((((0xe1i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0xb7c53; continue; }
                pc = 0xb7bf1; continue;
            }
            0xb7bf1 => {
                ecx = ((m.read8(eax.wrapping_add(0xb7cb8))) as u8 as u32);
                jt = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0xb7ca8));
                if (jt == 0xb7bffu32) { pc = 0xb7bff; continue; }
                if (jt == 0xb7c05u32) { pc = 0xb7c05; continue; }
                if (jt == 0xb7c0bu32) { pc = 0xb7c0b; continue; }
                if (jt == 0xb7c53u32) { pc = 0xb7c53; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0xb7bff => {
                eax = 0x62c;
                break 'run;
            }
            0xb7c05 => {
                eax = 0x36e;
                break 'run;
            }
            0xb7c0b => {
                eax = 0x3b5;
                break 'run;
            }
            0xb7c11 => {
                eax = 0x607;
                break 'run;
            }
            0xb7c17 => {
                fa = eax;
                fb = ((((0x7aei32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0xb7c49; continue; }
                pc = 0xb7c1e; continue;
            }
            0xb7c1e => {
                if (fa == fb) { pc = 0xb7c43; continue; }
                pc = 0xb7c20; continue;
            }
            0xb7c20 => {
                eax = eax.wrapping_sub(0x4d5);
                fa = eax;
                if ((fa == 0)) { pc = 0xb7c3d; continue; }
                pc = 0xb7c27; continue;
            }
            0xb7c27 => {
                eax = eax.wrapping_sub(0x2d7);
                fa = eax;
                if ((fa == 0)) { pc = 0xb7c37; continue; }
                pc = 0xb7c2e; continue;
            }
            0xb7c2e => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0xb7c53; continue; }
                pc = 0xb7c31; continue;
            }
            0xb7c31 => {
                eax = 0xdc4;
                break 'run;
            }
            0xb7c37 => {
                eax = 0xdc3;
                break 'run;
            }
            0xb7c3d => {
                eax = 0x1574;
                break 'run;
            }
            0xb7c43 => {
                eax = 0xdc5;
                break 'run;
            }
            0xb7c49 => {
                eax = eax.wrapping_sub(0x7af);
                fa = eax;
                if ((fa == 0)) { pc = 0xb7c5d; continue; }
                pc = 0xb7c50; continue;
            }
            0xb7c50 => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0xb7c57; continue; }
                pc = 0xb7c53; continue;
            }
            0xb7c53 => {
                eax = (eax | 0xffffffffu32);
                break 'run;
            }
            0xb7c57 => {
                eax = 0xdc7;
                break 'run;
            }
            0xb7c5d => {
                eax = 0xdc6;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x000B9C00: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn fb9c00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xb9c00;
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
        { crate::pose::cover(0xb9c00, previous, pc); previous = pc; }
        match pc {
            0xb9c00 => {
                esp = esp.wrapping_sub(0x30);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x38));
                eax = ((m.read16(esi.wrapping_add(0x94))) as u16 as u32);
                eax = eax.wrapping_sub(0x30);
                fa = eax;
                if ((fa == 0)) { pc = 0xb9ca8; continue; }
                pc = 0xb9c18; continue;
            }
            0xb9c18 => {
                eax = eax.wrapping_sub(0x2);
                fa = eax;
                if ((fa == 0)) { pc = 0xb9ca8; continue; }
                pc = 0xb9c21; continue;
            }
            0xb9c21 => {
                eax = eax.wrapping_sub(0x6e);
                fa = eax;
                if ((fa != 0)) { pc = 0xb9ced; continue; }
                pc = 0xb9c2a; continue;
            }
            0xb9c2a => {
                push!(edi);
                crate::audio_lane::set_return(0xb9c30);
                call!(0x153b40);
                pc = 0xb9c30; continue;
            }
            0xb9c30 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x3c), eax);
                fpu.push((m.read32(esp.wrapping_add(0x3c)) as i32 as f64));
                if (fas >= fbs) { pc = 0xb9c42; continue; }
                pc = 0xb9c3c; continue;
            }
            0xb9c3c => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0xb9c42; continue;
            }
            0xb9c42 => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475d18)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475cd4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                crate::audio_lane::set_return(0xb9c59);
                call!(0x3c848c);
                pc = 0xb9c59; continue;
            }
            0xb9c59 => {
                edi = eax;
                eax = esp.wrapping_add(0x8);
                push!(eax);
                edi = edi.wrapping_add(0xd83);
                crate::audio_lane::set_return(0xb9c6b);
                call!(0x45390);
                pc = 0xb9c6b; continue;
            }
            0xb9c6b => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x64)));
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x68)));
                ecx = esp.wrapping_add(0xc);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x6c)));
                push!(ecx);
                m.write8(esp.wrapping_add(0x38), (0x1) as u8);
                m.write32(esp.wrapping_add(0x10), edi);
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                crate::audio_lane::set_return(0xb9c9f);
                call!(0x45820);
                pc = 0xb9c9f; continue;
            }
            0xb9c9f => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            0xb9ca8 => {
                edx = esp.wrapping_add(0x4);
                push!(edx);
                crate::audio_lane::set_return(0xb9cb2);
                call!(0x45390);
                pc = 0xb9cb2; continue;
            }
            0xb9cb2 => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x64)));
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x68)));
                eax = esp.wrapping_add(0x8);
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x6c)));
                push!(eax);
                m.write8(esp.wrapping_add(0x34), (0x1) as u8);
                m.write32(esp.wrapping_add(0xc), 0x683);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                crate::audio_lane::set_return(0xb9cea);
                call!(0x45820);
                pc = 0xb9cea; continue;
            }
            0xb9cea => {
                esp = esp.wrapping_add(0x8);
                pc = 0xb9ced; continue;
            }
            0xb9ced => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00164470: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f164470<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x164470;
    let mut jt = 0u32;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x164470, previous, pc); previous = pc; }
        match pc {
            0x164470 => {
                push!(esi);
                push!(edi);
                edi = m.read32(eax.wrapping_add(0x7c));
                eax = gs::get_level(m);
                esi = (esi | 0xffffffffu32);
                eax = eax.wrapping_sub(1);
                fa = eax;
                fb = ((((0x50i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x16469b; continue; }
                pc = 0x164487; continue;
            }
            0x164487 => {
                ecx = ((m.read8(eax.wrapping_add(0x1646d8))) as u8 as u32);
                jt = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x1646a0));
                if (jt == 0x164495u32) { pc = 0x164495; continue; }
                if (jt == 0x1644a4u32) { pc = 0x1644a4; continue; }
                if (jt == 0x1644c7u32) { pc = 0x1644c7; continue; }
                if (jt == 0x1644eau32) { pc = 0x1644ea; continue; }
                if (jt == 0x164509u32) { pc = 0x164509; continue; }
                if (jt == 0x16452cu32) { pc = 0x16452c; continue; }
                if (jt == 0x16456du32) { pc = 0x16456d; continue; }
                if (jt == 0x164593u32) { pc = 0x164593; continue; }
                if (jt == 0x1645c8u32) { pc = 0x1645c8; continue; }
                if (jt == 0x1645ebu32) { pc = 0x1645eb; continue; }
                if (jt == 0x16462du32) { pc = 0x16462d; continue; }
                if (jt == 0x16464cu32) { pc = 0x16464c; continue; }
                if (jt == 0x16466bu32) { pc = 0x16466b; continue; }
                if (jt == 0x16469bu32) { pc = 0x16469b; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x164495 => {
                esi = 0;
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1644a4 => {
                push!(edi);
                crate::audio_lane::set_return(0x1644aa);
                call!(0x278040);
                pc = 0x1644aa; continue;
            }
            0x1644aa => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16469b; continue; }
                pc = 0x1644b5; continue;
            }
            0x1644b5 => {
                esi = 0x1;
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1644c7 => {
                push!(edi);
                crate::audio_lane::set_return(0x1644cd);
                call!(0x278040);
                pc = 0x1644cd; continue;
            }
            0x1644cd => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16469b; continue; }
                pc = 0x1644d8; continue;
            }
            0x1644d8 => {
                esi = 0x2;
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1644ea => {
                push!(edi);
                crate::audio_lane::set_return(0x1644f0);
                call!(0x278040);
                pc = 0x1644f0; continue;
            }
            0x1644f0 => {
                esi = eax;
                esp = esp.wrapping_add(0x4);
                cf = (((((esi) != 0))) as i32);
                esi = (((((esi) as i32).wrapping_neg())) as u32);
                esi = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                esi = esi.wrapping_add(0x4);
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x164509 => {
                push!(edi);
                crate::audio_lane::set_return(0x16450f);
                call!(0x278040);
                pc = 0x16450f; continue;
            }
            0x16450f => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16469b; continue; }
                pc = 0x16451a; continue;
            }
            0x16451a => {
                esi = 0x5;
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x16452c => {
                push!(0x1);
                push!(edi);
                crate::audio_lane::set_return(0x164534);
                call!(0x350e70);
                pc = 0x164534; continue;
            }
            0x164534 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x164540; continue; }
                pc = 0x16453b; continue;
            }
            0x16453b => {
                esi = 0x7;
                pc = 0x164540; continue;
            }
            0x164540 => {
                edx = m.read32(edi.wrapping_add(0xbf0));
                eax = m.read32(edx.wrapping_add(0x8));
                fa = eax;
                fb = ((((0x52534247i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16455b; continue; }
                pc = 0x164550; continue;
            }
            0x164550 => {
                fa = eax;
                fb = ((((0x52533032i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x16468a; continue; }
                pc = 0x16455b; continue;
            }
            0x16455b => {
                esi = 0x6;
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x16456d => {
                eax = m.read32(edi.wrapping_add(0xbf0));
                edx = m.read32(eax.wrapping_add(0x8));
                ecx = 0;
                fa = edx;
                fb = ((((0x5054435ai32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = ecx.wrapping_add(0x8);
                esi = ecx;
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x164593 => {
                push!(edi);
                crate::audio_lane::set_return(0x164599);
                call!(0x278040);
                pc = 0x164599; continue;
            }
            0x164599 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16469b; continue; }
                pc = 0x1645a4; continue;
            }
            0x1645a4 => {
                push!(0xb);
                crate::audio_lane::set_return(0x1645ab);
                call!(0x2273c0);
                pc = 0x1645ab; continue;
            }
            0x1645ab => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16469b; continue; }
                pc = 0x1645b6; continue;
            }
            0x1645b6 => {
                esi = 0xa;
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1645c8 => {
                push!(edi);
                crate::audio_lane::set_return(0x1645ce);
                call!(0x278040);
                pc = 0x1645ce; continue;
            }
            0x1645ce => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16469b; continue; }
                pc = 0x1645d9; continue;
            }
            0x1645d9 => {
                esi = 0xb;
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1645eb => {
                push!(edi);
                crate::audio_lane::set_return(0x1645f1);
                call!(0x278040);
                pc = 0x1645f1; continue;
            }
            0x1645f1 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x164607; continue; }
                pc = 0x1645f8; continue;
            }
            0x1645f8 => {
                edx = m.read32(edi.wrapping_add(0xbf0));
                fa = m.read32(edx.wrapping_add(0x8));
                fb = ((((0x52313142i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x16460c; continue; }
                pc = 0x164607; continue;
            }
            0x164607 => {
                esi = 0xc;
                pc = 0x16460c; continue;
            }
            0x16460c => {
                eax = m.read32(edi.wrapping_add(0xbf0));
                fa = m.read32(eax.wrapping_add(0x8));
                fb = ((((0x4352545ai32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x16468a; continue; }
                pc = 0x16461b; continue;
            }
            0x16461b => {
                esi = 0xd;
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x16462d => {
                push!(edi);
                crate::audio_lane::set_return(0x164633);
                call!(0x278040);
                pc = 0x164633; continue;
            }
            0x164633 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16469b; continue; }
                pc = 0x16463a; continue;
            }
            0x16463a => {
                esi = 0xe;
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x16464c => {
                push!(edi);
                crate::audio_lane::set_return(0x164652);
                call!(0x278040);
                pc = 0x164652; continue;
            }
            0x164652 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16469b; continue; }
                pc = 0x164659; continue;
            }
            0x164659 => {
                esi = 0xf;
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x16466b => {
                push!(edi);
                crate::audio_lane::set_return(0x164671);
                call!(0x278040);
                pc = 0x164671; continue;
            }
            0x164671 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16469b; continue; }
                pc = 0x164678; continue;
            }
            0x164678 => {
                esi = 0x10;
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x16468a => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x16469b; continue; }
                pc = 0x16468e; continue;
            }
            0x16468e => {
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(0x4e7ce8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x16469b => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00164730: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f164730<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x164730;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x164730, previous, pc); previous = pc; }
        match pc {
            0x164730 => {
                push!(ebx);
                ebx = eax;
                ebx = ((ebx as i32).wrapping_mul((0x2c) as i32)) as u32;
                ebx = ebx.wrapping_add(0x4e7ce8);
                eax = m.read32(ebx);
                push!(esi);
                esi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x16476c; continue; }
                pc = 0x164745; continue;
            }
            0x164745 => {
                push!(edi);
                edi = ebx.wrapping_add(0x4);
                pc = 0x164750; continue;
            }
            0x164750 => {
                eax = m.read32(edi);
                push!(eax);
                crate::audio_lane::set_return(0x164758);
                call!(0x461c0);
                pc = 0x164758; continue;
            }
            0x164758 => {
                push!(eax);
                crate::audio_lane::set_return(0x16475e);
                call!(0x45590);
                pc = 0x16475e; continue;
            }
            0x16475e => {
                eax = m.read32(ebx);
                esp = esp.wrapping_add(0x8);
                esi = esi.wrapping_add(1);
                edi = edi.wrapping_add(0x4);
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x164750; continue; }
                pc = 0x16476b; continue;
            }
            0x16476b => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x16476c; continue;
            }
            0x16476c => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x001648D0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1648d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1648d0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1648d0, previous, pc); previous = pc; }
        match pc {
            0x1648d0 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                eax = esi;
                crate::audio_lane::set_return(0x1648dc);
                call!(0x164470);
                pc = 0x1648dc; continue;
            }
            0x1648dc => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1648f8; continue; }
                pc = 0x1648e0; continue;
            }
            0x1648e0 => {
                eax = m.read32(esi.wrapping_add(0x2c));
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(0x74c3f0, eax);
                m.write32(0x74c3d0, xmm0.lanes[0]);
                pc = 0x1648f8; continue;
            }
            0x1648f8 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00164900: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f164900<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x164900;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x164900, previous, pc); previous = pc; }
        match pc {
            0x164900 => {
                eax = gs::get_level(m);
                eax = eax.wrapping_sub(0x4);
                fa = eax;
                if ((fa == 0)) { pc = 0x164918; continue; }
                pc = 0x16490a; continue;
            }
            0x16490a => {
                eax = eax.wrapping_sub(0x2);
                fa = eax;
                if ((fa == 0)) { pc = 0x164912; continue; }
                pc = 0x16490f; continue;
            }
            0x16490f => {
                eax = 0;
                break 'run;
            }
            0x164912 => {
                eax = 0x10e8;
                break 'run;
            }
            0x164918 => {
                eax = 0x1b26;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00164920: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f164920<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x164920;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x164920, previous, pc); previous = pc; }
        match pc {
            0x164920 => {
                eax = gs::get_level(m);
                eax = eax.wrapping_sub(0x4);
                fa = eax;
                if ((fa == 0)) { pc = 0x16492f; continue; }
                pc = 0x16492a; continue;
            }
            0x16492a => {
                eax = eax.wrapping_sub(0x2);
                fa = eax;
                if ((fa != 0)) { pc = 0x16495d; continue; }
                pc = 0x16492f; continue;
            }
            0x16492f => {
                xmm0 = Xmm::scalar(m.read32(0x74c3c8));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                if ((fca > fcb)) { pc = 0x16495d; continue; }
                pc = 0x164940; continue;
            }
            0x164940 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(eax.wrapping_add(0x2c));
                xmm0 = Xmm::scalar(m.read32(0x4785e4));
                m.write32(0x74c3ec, ecx);
                m.write32(0x74c3c8, xmm0.lanes[0]);
                pc = 0x16495d; continue;
            }
            0x16495d => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x001649E0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1649e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1649e0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1649e0, previous, pc); previous = pc; }
        match pc {
            0x1649e0 => {
                edx = m.read32(0x74c3fc);
                eax = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                if (fas <= fbs) { pc = 0x164a10; continue; }
                pc = 0x1649f1; continue;
            }
            0x1649f1 => {
                ecx = m.read32(eax.wrapping_mul(0x8).wrapping_add(0x7a2804));
                fa = ecx;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x164a08; continue; }
                pc = 0x1649fc; continue;
            }
            0x1649fc => {
                fa = ecx;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x164a2e; continue; }
                pc = 0x164a01; continue;
            }
            0x164a01 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1649f1; continue; }
                pc = 0x164a06; continue;
            }
            0x164a06 => {
                pc = 0x164a10; continue;
            }
            0x164a08 => {
                edx = eax;
                m.write32(0x74c3fc, edx);
                pc = 0x164a10; continue;
            }
            0x164a10 => {
                fa = edx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x164a2e; continue; }
                pc = 0x164a15; continue;
            }
            0x164a15 => {
                eax = m.read32(esp.wrapping_add(0x8));
                m.write32(edx.wrapping_mul(0x8).wrapping_add(0x7a2804), esi);
                m.write32(edx.wrapping_mul(0x8).wrapping_add(0x7a2800), eax);
                edx = edx.wrapping_add(1);
                m.write32(0x74c3fc, edx);
                pc = 0x164a2e; continue;
            }
            0x164a2e => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00164B60: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f164b60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x164b60;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x164b60, previous, pc); previous = pc; }
        match pc {
            0x164b60 => {
                eax = m.read32(0x74c3d8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x164b75; continue; }
                pc = 0x164b69; continue;
            }
            0x164b69 => {
                eax = m.read32(0x74c3fc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x164b75; continue; }
                pc = 0x164b72; continue;
            }
            0x164b72 => {
                eax = 0;
                break 'run;
            }
            0x164b75 => {
                eax = 0x1;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00164B80: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f164b80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x164b80;
    let mut jt = 0u32;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x164b80, previous, pc); previous = pc; }
        match pc {
            0x164b80 => {
                push!(edi);
                edi = eax;
                crate::audio_lane::set_return(0x164b88);
                call!(0x350c60);
                pc = 0x164b88; continue;
            }
            0x164b88 => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x164bcc; continue; }
                pc = 0x164b8e; continue;
            }
            0x164b8e => {
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x164bdc));
                if (jt == 0x164b95u32) { pc = 0x164b95; continue; }
                if (jt == 0x164ba7u32) { pc = 0x164ba7; continue; }
                if (jt == 0x164bb9u32) { pc = 0x164bb9; continue; }
                if (jt == 0x164bcbu32) { pc = 0x164bcb; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x164b95 => {
                edi = edi.wrapping_add(0xfffffffcu32);
                eax = edi;
                push!(eax);
                m.write32(esi, edi);
                crate::audio_lane::set_return(0x164ba2);
                call!(0x45590);
                pc = 0x164ba2; continue;
            }
            0x164ba2 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x164ba7 => {
                edi = edi.wrapping_add(0xfffffffdu32);
                eax = edi;
                push!(eax);
                m.write32(esi, edi);
                crate::audio_lane::set_return(0x164bb4);
                call!(0x45590);
                pc = 0x164bb4; continue;
            }
            0x164bb4 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x164bb9 => {
                edi = edi.wrapping_add(0xfffffffeu32);
                eax = edi;
                push!(eax);
                m.write32(esi, edi);
                crate::audio_lane::set_return(0x164bc6);
                call!(0x45590);
                pc = 0x164bc6; continue;
            }
            0x164bc6 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x164bcb => {
                edi = edi.wrapping_sub(1);
                pc = 0x164bcc; continue;
            }
            0x164bcc => {
                eax = edi;
                push!(eax);
                m.write32(esi, edi);
                crate::audio_lane::set_return(0x164bd6);
                call!(0x45590);
                pc = 0x164bd6; continue;
            }
            0x164bd6 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00164BF0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f164bf0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x164bf0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x164bf0, previous, pc); previous = pc; }
        match pc {
            0x164bf0 => {
                eax = (eax | 0xffffffffu32);
                m.write32(0x74c3d8, eax);
                m.write32(0x74c3dc, eax);
                m.write32(0x74c3d4, eax);
                eax = 0;
                m.write32(0x74c3fc, eax);
                m.write32(0x74c3f4, eax);
                eax = 0x4e7fe0;
                pc = 0x164c20; continue;
            }
            0x164c20 => {
                m.write32(eax, 0xfffffc18u32);
                eax = eax.wrapping_add(0xc);
                fa = eax;
                fb = ((((0x4e8274i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x164c20; continue; }
                pc = 0x164c30; continue;
            }
            0x164c30 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00164C40: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f164c40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x164c40;
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
        { crate::pose::cover(0x164c40, previous, pc); previous = pc; }
        match pc {
            0x164c40 => {
                eax = m.read32(esp.wrapping_add(0x10));
                esp = esp.wrapping_sub(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x164c53; continue; }
                pc = 0x164c4b; continue;
            }
            0x164c4b => {
                eax = m.read32(eax);
                m.write32(esp.wrapping_add(0xc), eax);
                pc = 0x164c5b; continue;
            }
            0x164c53 => {
                m.write32(esp.wrapping_add(0xc), 0);
                pc = 0x164c5b; continue;
            }
            0x164c5b => {
                ecx = m.read32(esp.wrapping_add(0x18));
                push!(ebx);
                push!(edi);
                push!(ecx);
                crate::audio_lane::set_return(0x164c67);
                call!(0x461c0);
                pc = 0x164c67; continue;
            }
            0x164c67 => {
                xmm0 = Xmm::scalar(m.read32(0x475d18));
                edi = m.read32(esp.wrapping_add(0x20));
                ebx = eax;
                eax = (eax & 0xffff_ff00) | ((((gs::get_mode(m)) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                if (fa != fb) { pc = 0x164c93; continue; }
                pc = 0x164c87; continue;
            }
            0x164c87 => {
                edx = m.read32(edi.wrapping_add(0x14));
                eax = m.read32(edi.wrapping_add(0x10));
                m.write32(esp.wrapping_add(0x10), edx);
                pc = 0x164c9d; continue;
            }
            0x164c93 => {
                ecx = m.read32(edi.wrapping_add(0x1c));
                eax = m.read32(edi.wrapping_add(0x18));
                m.write32(esp.wrapping_add(0x10), ecx);
                pc = 0x164c9d; continue;
            }
            0x164c9d => {
                eax = (eax & 0x1);
                ecx = 0;
                eax = (eax | ecx);
                fa = eax;
                if ((fa != 0)) { pc = 0x164df7; continue; }
                pc = 0x164caa; continue;
            }
            0x164caa => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(ebp);
                push!(esi);
                if (fas < fbs) { pc = 0x164db9; continue; }
                pc = 0x164cb4; continue;
            }
            0x164cb4 => {
                esi = m.read32(esp.wrapping_add(0x2c));
                fa = esi;
                fb = ((((0x800i32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x74c408));
                if ((fa & fb) != 0) { pc = 0x164cd0; continue; }
                pc = 0x164cc8; continue;
            }
            0x164cc8 => {
                xmm0 = Xmm::scalar(m.read32(0x74c3cc));
                pc = 0x164cd0; continue;
            }
            0x164cd0 => {
                edx = m.read32(edi);
                push!(edx);
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x164cde);
                call!(0x1642c0);
                pc = 0x164cde; continue;
            }
            0x164cde => {
                push!(ebx);
                crate::audio_lane::set_return(0x164ce4);
                call!(0x45620);
                pc = 0x164ce4; continue;
            }
            0x164ce4 => {
                m.write32(esp.wrapping_add(0x18), (fpu.st(0) as f32).to_bits()); fpu.pop();
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0x4000i32)) as u32) & 0xffffffffu32);
                push!(0x1);
                if ((fa & fb) == 0) { pc = 0x164d05; continue; }
                pc = 0x164cf5; continue;
            }
            0x164cf5 => {
                push!(0);
                push!(0x3fa00000);
                push!(0x3f800000);
                push!(0);
                pc = 0x164d1a; continue;
            }
            0x164d05 => {
                eax = m.read32(edi);
                ecx = m.read32(esp.wrapping_add(0x18));
                eax = eax.wrapping_add(0x64);
                push!(eax);
                push!(0x3fe00000);
                push!(0x3f800000);
                push!(ecx);
                pc = 0x164d1a; continue;
            }
            0x164d1a => {
                push!(ebx);
                crate::audio_lane::set_return(0x164d20);
                call!(0x46d80);
                pc = 0x164d20; continue;
            }
            0x164d20 => {
                ebx = m.read32(esp.wrapping_add(0x48));
                esp = esp.wrapping_add(0x18);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebp = eax;
                if (fa == fb) { pc = 0x164d35; continue; }
                pc = 0x164d2d; continue;
            }
            0x164d2d => {
                edx = m.read32(edi);
                eax = m.read32(edx.wrapping_add(0x2c));
                m.write32(ebx.wrapping_add(0x24), eax);
                pc = 0x164d35; continue;
            }
            0x164d35 => {
                fa = esi;
                fb = ((((0x10000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x164d48; continue; }
                pc = 0x164d3d; continue;
            }
            0x164d3d => {
                push!(0);
                push!(ebp);
                crate::audio_lane::set_return(0x164d45);
                call!(0x454b0);
                pc = 0x164d45; continue;
            }
            0x164d45 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x164d48; continue;
            }
            0x164d48 => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x164d4f; continue; }
                pc = 0x164d4c; continue;
            }
            0x164d4c => {
                m.write32(ebx.wrapping_add(0x20), ebp);
                pc = 0x164d4f; continue;
            }
            0x164d4f => {
                m.write32(edi.wrapping_add(0xe8c), ebp);
                crate::audio_lane::set_return(0x164d5a);
                call!(0x64cd0);
                pc = 0x164d5a; continue;
            }
            0x164d5a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x164db9; continue; }
                pc = 0x164d5e; continue;
            }
            0x164d5e => {
                eax = m.read32(esp.wrapping_add(0x28));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x164db9; continue; }
                pc = 0x164d66; continue;
            }
            0x164d66 => {
                eax = m.read32(esp.wrapping_add(0x30));
                esi = 0;
                fa = eax;
                fb = esi;
                if (fa == fb) { pc = 0x164d7b; continue; }
                pc = 0x164d70; continue;
            }
            0x164d70 => {
                ecx = m.read32(eax);
                edx = m.read32(ecx.wrapping_add(0x1c));
                m.write32(esp.wrapping_add(0x14), edx);
                pc = 0x164d7f; continue;
            }
            0x164d7b => {
                m.write32(esp.wrapping_add(0x14), esi);
                pc = 0x164d7f; continue;
            }
            0x164d7f => {
                eax = gs::get_players(m);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x164db9; continue; }
                pc = 0x164d88; continue;
            }
            0x164d88 => {
                ebx = m.read32(esp.wrapping_add(0x10));
                pc = 0x164d90; continue;
            }
            0x164d90 => {
                eax = m.read32(esp.wrapping_add(0x14));
                ecx = m.read32(esp.wrapping_add(0x28));
                edx = m.read32(edi.wrapping_add(0x8));
                push!(ebp);
                push!(ebx);
                push!(0);
                push!(eax);
                push!(ecx);
                push!(0x9);
                push!(edx);
                push!(esi);
                crate::audio_lane::set_return(0x164daa);
                call!(0x148a90);
                pc = 0x164daa; continue;
            }
            0x164daa => {
                eax = gs::get_players(m);
                esp = esp.wrapping_add(0x20);
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x164d90; continue; }
                pc = 0x164db7; continue;
            }
            0x164db7 => {
                pc = 0x164dbd; continue;
            }
            0x164db9 => {
                ebx = m.read32(esp.wrapping_add(0x10));
                pc = 0x164dbd; continue;
            }
            0x164dbd => {
                eax = m.read32(edi.wrapping_add(0xe8c));
                ecx = m.read32(esp.wrapping_add(0x28));
                push!(ebx);
                push!(eax);
                push!(ecx);
                crate::audio_lane::set_return(0x164dcf);
                call!(0x3d7b0);
                pc = 0x164dcf; continue;
            }
            0x164dcf => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(edi);
                crate::audio_lane::set_return(0x164dd9);
                call!(0x1cb8a0);
                pc = 0x164dd9; continue;
            }
            0x164dd9 => {
                eax = m.read32(esp.wrapping_add(0x2c));
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x164df7; continue; }
                pc = 0x164de6; continue;
            }
            0x164de6 => {
                eax = m.read32(eax.wrapping_add(0x38));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x164df7; continue; }
                pc = 0x164ded; continue;
            }
            0x164ded => {
                edx = m.read32(esp.wrapping_add(0x28));
                push!(edx);
                crate::audio_lane::set_return(0x164df4);
                call_to!(eax);
                pc = 0x164df4; continue;
            }
            0x164df4 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x164df7; continue;
            }
            0x164df7 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x001651C0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1651c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1651c0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1651c0, previous, pc); previous = pc; }
        match pc {
            0x1651c0 => {
                fa = ((((gs::get_mode(m))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x1651d6; continue; }
                pc = 0x1651c9; continue;
            }
            0x1651c9 => {
                fa = ((((gs::get_flags_low8(m))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x165384; continue; }
                pc = 0x1651d6; continue;
            }
            0x1651d6 => {
                eax = m.read32(0x74c3fc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(esi);
                if (fas <= fbs) { pc = 0x1652b9; continue; }
                pc = 0x1651e4; continue;
            }
            0x1651e4 => {
                eax = m.read32(0x74c3dc);
                esi = 0x36;
                fa = eax;
                fb = esi;
                if (fa != fb) { pc = 0x1651ff; continue; }
                pc = 0x1651f2; continue;
            }
            0x1651f2 => {
                eax = m.read32(0x74c3f4);
                fa = eax;
                fb = m.read32(0x7a2804);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x16523a; continue; }
                pc = 0x1651ff; continue;
            }
            0x1651ff => {
                xmm0.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                push!(0);
                push!(ecx);
                xmm1.lanes[0] = sse::divss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp, xmm1.lanes[0]);
                push!(0x3f800000);
                push!(0x74c3d8);
                crate::audio_lane::set_return(0x16522a);
                call!(0x46570);
                pc = 0x16522a; continue;
            }
            0x16522a => {
                esp = esp.wrapping_add(0x10);
                eax = (eax | 0xffffffffu32);
                m.write32(0x74c3dc, eax);
                m.write32(0x74c3d8, eax);
                pc = 0x16523a; continue;
            }
            0x16523a => {
                ecx = m.read32(0x74c3d8);
                push!(ecx);
                crate::audio_lane::set_return(0x165246);
                call!(0x45540);
                pc = 0x165246; continue;
            }
            0x165246 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x165383; continue; }
                pc = 0x165251; continue;
            }
            0x165251 => {
                edx = m.read32(0x74c3e0);
                eax = m.read32(0x7a2800);
                push!(edx);
                push!(eax);
                crate::audio_lane::set_return(0x165263);
                call!(0x218d0);
                pc = 0x165263; continue;
            }
            0x165263 => {
                ecx = m.read32(0x7a2804);
                m.write32(0x74c3f4, ecx);
                ecx = m.read32(0x74c3fc);
                esp = esp.wrapping_add(0x8);
                m.write32(0x74c3d8, eax);
                eax = 0;
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(0x74c3dc, esi);
                if (fas <= fbs) { pc = 0x1652b1; continue; }
                pc = 0x16528a; continue;
            }
            0x16528a => {
                pc = 0x165290; continue;
            }
            0x165290 => {
                edx = m.read32(eax.wrapping_mul(0x8).wrapping_add(0x7a2808));
                m.write32(eax.wrapping_mul(0x8).wrapping_add(0x7a2800), edx);
                edx = m.read32(eax.wrapping_mul(0x8).wrapping_add(0x7a280c));
                m.write32(eax.wrapping_mul(0x8).wrapping_add(0x7a2804), edx);
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x165290; continue; }
                pc = 0x1652b1; continue;
            }
            0x1652b1 => {
                m.write32(0x74c3fc, ecx);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1652b9 => {
                eax = m.read32(0x74c3d4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x165363; continue; }
                pc = 0x1652c6; continue;
            }
            0x1652c6 => {
                fa = eax;
                fb = ((((0x37i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x165363; continue; }
                pc = 0x1652cf; continue;
            }
            0x1652cf => {
                esi = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = m.read32(0x74c3d8);
                push!(eax);
                esi = esi.wrapping_mul(0x4).wrapping_add(0x4e7fd8);
                crate::audio_lane::set_return(0x1652e4);
                call!(0x45540);
                pc = 0x1652e4; continue;
            }
            0x1652e4 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16532b; continue; }
                pc = 0x1652eb; continue;
            }
            0x1652eb => {
                fa = m.read32(esi.wrapping_add(0x4));
                fb = ((((0x32i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1652fd; continue; }
                pc = 0x1652f1; continue;
            }
            0x1652f1 => {
                m.write32(0x74c3d4, 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1652fd => {
                xmm0.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                push!(0);
                push!(ecx);
                xmm1.lanes[0] = sse::divss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp, xmm1.lanes[0]);
                push!(0x3f800000);
                push!(0x74c3d8);
                crate::audio_lane::set_return(0x165328);
                call!(0x46570);
                pc = 0x165328; continue;
            }
            0x165328 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x16532b; continue;
            }
            0x16532b => {
                edx = m.read32(0x74c3e0);
                eax = m.read32(esi);
                ecx = m.read32(0x7de318);
                push!(edx);
                push!(eax);
                m.write32(esi.wrapping_add(0x8), ecx);
                crate::audio_lane::set_return(0x165343);
                call!(0x218d0);
                pc = 0x165343; continue;
            }
            0x165343 => {
                ecx = m.read32(0x74c3d4);
                esp = esp.wrapping_add(0x8);
                m.write32(0x74c3dc, ecx);
                m.write32(0x74c3d4, 0xffffffffu32);
                m.write32(0x74c3d8, eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x165363 => {
                edx = m.read32(0x74c3d8);
                push!(edx);
                crate::audio_lane::set_return(0x16536f);
                call!(0x45540);
                pc = 0x16536f; continue;
            }
            0x16536f => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x165383; continue; }
                pc = 0x165376; continue;
            }
            0x165376 => {
                eax = (eax | 0xffffffffu32);
                m.write32(0x74c3dc, eax);
                m.write32(0x74c3d8, eax);
                pc = 0x165383; continue;
            }
            0x165383 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x165384; continue;
            }
            0x165384 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00165390: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f165390<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x165390;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x165390, previous, pc); previous = pc; }
        match pc {
            0x165390 => {
                fa = gs::get_level(m);
                fb = ((((0x64i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x16542d; continue; }
                pc = 0x16539d; continue;
            }
            0x16539d => {
                fa = ((((gs::get_mode(m))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x1653b8; continue; }
                pc = 0x1653a6; continue;
            }
            0x1653a6 => {
                eax = gs::get_flags(m);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x1653ff; continue; }
                pc = 0x1653af; continue;
            }
            0x1653af => {
                crate::audio_lane::set_return(0x1653b4);
                call!(0x22dec0);
                pc = 0x1653b4; continue;
            }
            0x1653b4 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1653f6; continue; }
                pc = 0x1653b8; continue;
            }
            0x1653b8 => {
                push!(esi);
                eax = 0x162e;
                esi = 0x4e8008;
                crate::audio_lane::set_return(0x1653c8);
                call!(0x164b80);
                pc = 0x1653c8; continue;
            }
            0x1653c8 => {
                eax = 0x1633;
                esi = 0x4e8014;
                crate::audio_lane::set_return(0x1653d7);
                call!(0x164b80);
                pc = 0x1653d7; continue;
            }
            0x1653d7 => {
                eax = 0x1638;
                esi = 0x4e8020;
                crate::audio_lane::set_return(0x1653e6);
                call!(0x164b80);
                pc = 0x1653e6; continue;
            }
            0x1653e6 => {
                eax = 0x163d;
                esi = 0x4e802c;
                crate::audio_lane::set_return(0x1653f5);
                call!(0x164b80);
                pc = 0x1653f5; continue;
            }
            0x1653f5 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x1653f6; continue;
            }
            0x1653f6 => {
                eax = gs::get_flags(m);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x16542d; continue; }
                pc = 0x1653ff; continue;
            }
            0x1653ff => {
                fa = eax;
                fb = ((((0x8000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x165412; continue; }
                pc = 0x165406; continue;
            }
            0x165406 => {
                m.write32(0x74c40c, 0);
                pc = 0x165437; continue;
            }
            0x165412 => {
                fa = eax;
                fb = ((((0x4000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x16542d; continue; }
                pc = 0x165419; continue;
            }
            0x165419 => {
                ecx = m.read32(0x5236b0);
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                m.write32(0x74c40c, eax);
                pc = 0x165437; continue;
            }
            0x16542d => {
                m.write32(0x74c40c, 0x1);
                pc = 0x165437; continue;
            }
            0x165437 => {
                crate::audio_lane::set_return(0x16543c);
                call!(0x350c60);
                pc = 0x16543c; continue;
            }
            0x16543c => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16546f; continue; }
                pc = 0x165440; continue;
            }
            0x165440 => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x16545a; continue; }
                pc = 0x165445; continue;
            }
            0x165445 => {
                xmm0 = Xmm::scalar(m.read32(0x47ddb0));
                m.write32(0x74c3e0, xmm0.lanes[0]);
                tail_to!(0x164bf0); break 'run;  // direct tail jump
            }
            0x16545a => {
                xmm0 = Xmm::scalar(m.read32(0x47ddac));
                m.write32(0x74c3e0, xmm0.lanes[0]);
                tail_to!(0x164bf0); break 'run;  // direct tail jump
            }
            0x16546f => {
                xmm0 = Xmm::scalar(m.read32(0x47890c));
                m.write32(0x74c3e0, xmm0.lanes[0]);
                tail_to!(0x164bf0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x001B2430: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1b2430<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b2430;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1b2430, previous, pc); previous = pc; }
        match pc {
            0x1b2430 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x1b243c);
                call!(0x45390);
                pc = 0x1b243c; continue;
            }
            0x1b243c => {
                ecx = esp.wrapping_add(0x4);
                push!(ecx);
                m.write32(esp.wrapping_add(0x8), 0xffffffffu32);
                crate::audio_lane::set_return(0x1b244e);
                call!(0x45820);
                pc = 0x1b244e; continue;
            }
            0x1b244e => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x001D96C0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1d96c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1d96c0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1d96c0, previous, pc); previous = pc; }
        match pc {
            0x1d96c0 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                push!(0);
                push!(0x3f800000);
                push!(0x40a00000);
                push!(0x3fa66666);
                push!(0x180);
                push!(esi);
                crate::audio_lane::set_return(0x1d96e1);
                call!(0xa5a60);
                pc = 0x1d96e1; continue;
            }
            0x1d96e1 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = esi.wrapping_add(0x22c);
                push!(eax);
                crate::audio_lane::set_return(0x1d96f1);
                call!(0x468a0);
                pc = 0x1d96f1; continue;
            }
            0x1d96f1 => {
                esp = esp.wrapping_add(0x1c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1d96fc; continue; }
                pc = 0x1d96f8; continue;
            }
            0x1d96f8 => {
                m.write32(esi.wrapping_add(0x18), (m.read32(esi.wrapping_add(0x18)) | 0x40));
                pc = 0x1d96fc; continue;
            }
            0x1d96fc => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00207250: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f207250<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x207250;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x207250, previous, pc); previous = pc; }
        match pc {
            0x207250 => {
                push!(0x1);
                push!(0x10);
                crate::audio_lane::set_return(0x207259);
                call!(0x3dfa0);
                pc = 0x207259; continue;
            }
            0x207259 => {
                push!(0x1e);
                m.write32(0x75c044, eax);
                crate::audio_lane::set_return(0x207265);
                call!(0x45590);
                pc = 0x207265; continue;
            }
            0x207265 => {
                push!(0x1f);
                crate::audio_lane::set_return(0x20726c);
                call!(0x45590);
                pc = 0x20726c; continue;
            }
            0x20726c => {
                esp = esp.wrapping_add(0x10);
                break 'run;
                push!(0x3f2);
                crate::audio_lane::set_return(0x20729f);
                call!(0x9cc40);
                pc = 0x20729f; continue;
            }
            0x20729f => {
                ecx = m.read32(0x715f2c);
                eax = eax.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(ecx).wrapping_add(0x4);
                ecx = m.read32(eax);
                ecx = (ecx | 0x20000);
                push!(0x47f);
                m.write32(eax, ecx);
                crate::audio_lane::set_return(0x2072c0);
                call!(0x9cc40);
                pc = 0x2072c0; continue;
            }
            0x2072c0 => {
                edx = m.read32(0x715f2c);
                eax = eax.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = m.read32(eax.wrapping_add(edx).wrapping_add(0x4));
                eax = eax.wrapping_add(edx).wrapping_add(0x4);
                ecx = (ecx | 0x20000);
                push!(0x486);
                m.write32(eax, ecx);
                crate::audio_lane::set_return(0x2072e3);
                call!(0x9cc40);
                pc = 0x2072e3; continue;
            }
            0x2072e3 => {
                ecx = m.read32(0x715f2c);
                eax = eax.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(ecx).wrapping_add(0x4);
                ecx = m.read32(eax);
                ecx = (ecx | 0x20000);
                push!(0x3fd);
                m.write32(eax, ecx);
                crate::audio_lane::set_return(0x207304);
                call!(0x9cc40);
                pc = 0x207304; continue;
            }
            0x207304 => {
                edx = m.read32(0x715f2c);
                eax = eax.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = m.read32(eax.wrapping_add(edx).wrapping_add(0x4));
                eax = eax.wrapping_add(edx).wrapping_add(0x4);
                esp = esp.wrapping_add(0x10);
                ecx = (ecx | 0x20000);
                m.write32(eax, ecx);
                pc = 0x207320; continue;
            }
            0x207320 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00207270: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f207270<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x207270;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x207270, previous, pc); previous = pc; }
        match pc {
            0x207270 => {
                eax = m.read32(0x75c044);
                m.write32(eax, 0);
                m.write32(eax.wrapping_add(0x4), 0xffffffffu32);
                m.write32(eax, 0xffffffffu32);
                fa = ((((gs::get_flags_low8(m))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x207320; continue; }
                pc = 0x207295; continue;
            }
            0x207295 => {
                push!(0x3f2);
                crate::audio_lane::set_return(0x20729f);
                call!(0x9cc40);
                pc = 0x20729f; continue;
            }
            0x20729f => {
                ecx = m.read32(0x715f2c);
                eax = eax.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(ecx).wrapping_add(0x4);
                ecx = m.read32(eax);
                ecx = (ecx | 0x20000);
                push!(0x47f);
                m.write32(eax, ecx);
                crate::audio_lane::set_return(0x2072c0);
                call!(0x9cc40);
                pc = 0x2072c0; continue;
            }
            0x2072c0 => {
                edx = m.read32(0x715f2c);
                eax = eax.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = m.read32(eax.wrapping_add(edx).wrapping_add(0x4));
                eax = eax.wrapping_add(edx).wrapping_add(0x4);
                ecx = (ecx | 0x20000);
                push!(0x486);
                m.write32(eax, ecx);
                crate::audio_lane::set_return(0x2072e3);
                call!(0x9cc40);
                pc = 0x2072e3; continue;
            }
            0x2072e3 => {
                ecx = m.read32(0x715f2c);
                eax = eax.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(ecx).wrapping_add(0x4);
                ecx = m.read32(eax);
                ecx = (ecx | 0x20000);
                push!(0x3fd);
                m.write32(eax, ecx);
                crate::audio_lane::set_return(0x207304);
                call!(0x9cc40);
                pc = 0x207304; continue;
            }
            0x207304 => {
                edx = m.read32(0x715f2c);
                eax = eax.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = m.read32(eax.wrapping_add(edx).wrapping_add(0x4));
                eax = eax.wrapping_add(edx).wrapping_add(0x4);
                esp = esp.wrapping_add(0x10);
                ecx = (ecx | 0x20000);
                m.write32(eax, ecx);
                pc = 0x207320; continue;
            }
            0x207320 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002211A0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2211a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2211a0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x2211a0, previous, pc); previous = pc; }
        match pc {
            0x2211a0 => {
                edx = m.read32(0x75c10c);
                eax = 0;
                push!(edi);
                ecx = 0xe;
                edi = edx;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                eax = (eax | 0xffffffffu32);
                m.write32(edx, eax);
                m.write32(edx.wrapping_add(0x4), eax);
                m.write32(edx.wrapping_add(0x8), eax);
                m.write32(edx.wrapping_add(0xc), eax);
                m.write32(edx.wrapping_add(0x10), eax);
                m.write32(edx.wrapping_add(0x14), eax);
                m.write32(edx.wrapping_add(0x18), eax);
                m.write32(edx.wrapping_add(0x1c), eax);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002212C0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2212c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2212c0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x2212c0, previous, pc); previous = pc; }
        match pc {
            0x2212c0 => {
                esp = esp.wrapping_sub(0x30);
                eax = m.read32(0x75c10c);
                push!(0);
                push!(0x3e4ccccd);
                ecx = eax.wrapping_add(edi.wrapping_mul(0x4));
                push!(0x3fc00000);
                push!(ecx);
                crate::audio_lane::set_return(0x2212dd);
                call!(0x46570);
                pc = 0x2212dd; continue;
            }
            0x2212dd => {
                edx = esp.wrapping_add(0x10);
                push!(edx);
                crate::audio_lane::set_return(0x2212e7);
                call!(0x45390);
                pc = 0x2212e7; continue;
            }
            0x2212e7 => {
                xmm0 = Xmm::scalar(m.read32(0x4788b8));
                eax = m.read32(esp.wrapping_add(0x24));
                m.write32(esp.wrapping_add(0x40), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475d20));
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi));
                eax = (eax | 0xa0);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x24), eax);
                ecx = esp.wrapping_add(0x14);
                eax = edi.wrapping_add(0x66e);
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                push!(ecx);
                m.write8(esp.wrapping_add(0x40), (0x1) as u8);
                m.write32(esp.wrapping_add(0x18), eax);
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x221349);
                call!(0x45820);
                pc = 0x221349; continue;
            }
            0x221349 => {
                edx = m.read32(0x75c10c);
                m.write32(edx.wrapping_add(edi.wrapping_mul(0x4)), eax);
                esp = esp.wrapping_add(0x48);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00221410: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f221410<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x221410;
    let mut jt = 0u32;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x221410, previous, pc); previous = pc; }
        match pc {
            0x221410 => {
                push!(ebx);
                push!(ebp);
                push!(esi);
                eax = eax.wrapping_sub(1);
                fa = eax;
                fb = ((((0x9i32)) as u32) & 0xffffffffu32);
                push!(edi);
                esi = ecx;
                ebx = edx;
                if (fa > fb) { pc = 0x2214a5; continue; }
                pc = 0x221422; continue;
            }
            0x221422 => {
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x2214ac));
                if (jt == 0x221429u32) { pc = 0x221429; continue; }
                if (jt == 0x221430u32) { pc = 0x221430; continue; }
                if (jt == 0x221434u32) { pc = 0x221434; continue; }
                if (jt == 0x22143bu32) { pc = 0x22143b; continue; }
                if (jt == 0x221442u32) { pc = 0x221442; continue; }
                if (jt == 0x221449u32) { pc = 0x221449; continue; }
                if (jt == 0x22145eu32) { pc = 0x22145e; continue; }
                if (jt == 0x221473u32) { pc = 0x221473; continue; }
                if (jt == 0x2214a5u32) { pc = 0x2214a5; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x221429 => {
                ebp = 0x1;
                pc = 0x22148b; continue;
            }
            0x221430 => {
                ebp = 0;
                pc = 0x22148b; continue;
            }
            0x221434 => {
                ebp = 0x6;
                pc = 0x22148b; continue;
            }
            0x22143b => {
                ebp = 0x5;
                pc = 0x22148b; continue;
            }
            0x221442 => {
                ebp = 0x4;
                pc = 0x22148b; continue;
            }
            0x221449 => {
                edi = m.read32(0x75c10c);
                ebp = 0x2;
                edi = edi.wrapping_add(0x20);
                eax = 0x70000;
                pc = 0x221486; continue;
            }
            0x22145e => {
                edi = m.read32(0x75c10c);
                ebp = 0x3;
                edi = edi.wrapping_add(0x28);
                eax = 0x90000;
                pc = 0x221486; continue;
            }
            0x221473 => {
                edi = m.read32(0x75c10c);
                ebp = 0x7;
                edi = edi.wrapping_add(0x30);
                eax = 0xa0000;
                pc = 0x221486; continue;
            }
            0x221486 => {
                crate::audio_lane::set_return(0x22148b);
                call!(0x221360);
                pc = 0x22148b; continue;
            }
            0x22148b => {
                eax = m.read32(ebx.wrapping_add(0x18));
                eax = (eax | 0x40);
                edi = ebp;
                m.write32(ebx.wrapping_add(0x18), eax);
                crate::audio_lane::set_return(0x22149b);
                call!(0x2212c0);
                pc = 0x22149b; continue;
            }
            0x22149b => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2214a5 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0022DEC0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f22dec0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x22dec0;
    let mut jt = 0u32;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x22dec0, previous, pc); previous = pc; }
        match pc {
            0x22dec0 => {
                eax = m.read32(0x7de470);
                fa = eax;
                fb = ((((0x14i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x22dede; continue; }
                pc = 0x22deca; continue;
            }
            0x22deca => {
                eax = ((m.read8(eax.wrapping_add(0x22deec))) as u8 as u32);
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x22dee4));
                if (jt == 0x22ded8u32) { pc = 0x22ded8; continue; }
                if (jt == 0x22dedeu32) { pc = 0x22dede; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x22ded8 => {
                eax = 0x1;
                break 'run;
            }
            0x22dede => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00247510: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f247510<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x247510;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x247510, previous, pc); previous = pc; }
        match pc {
            0x247510 => {
                eax = m.read32(0x51ae34);
                esp = esp.wrapping_sub(0x30);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x24753f; continue; }
                pc = 0x24751c; continue;
            }
            0x24751c => {
                eax = esp;
                push!(eax);
                crate::audio_lane::set_return(0x247525);
                call!(0x45390);
                pc = 0x247525; continue;
            }
            0x247525 => {
                ecx = esp.wrapping_add(0x4);
                push!(ecx);
                m.write32(esp.wrapping_add(0x8), 0x39d);
                crate::audio_lane::set_return(0x247537);
                call!(0x45820);
                pc = 0x247537; continue;
            }
            0x247537 => {
                esp = esp.wrapping_add(0x8);
                m.write32(0x51ae34, eax);
                pc = 0x24753f; continue;
            }
            0x24753f => {
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00257620: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f257620<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x257620;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x257620, previous, pc); previous = pc; }
        match pc {
            0x257620 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(esi.wrapping_add(0x22c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x25764c; continue; }
                pc = 0x25762f; continue;
            }
            0x25762f => {
                push!(eax);
                crate::audio_lane::set_return(0x257635);
                call!(0x45540);
                pc = 0x257635; continue;
            }
            0x257635 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x257648; continue; }
                pc = 0x25763c; continue;
            }
            0x25763c => {
                m.write32(esi.wrapping_add(0x22c), 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x257648 => {
                m.write32(esi.wrapping_add(0x18), (m.read32(esi.wrapping_add(0x18)) | 0x40));
                pc = 0x25764c; continue;
            }
            0x25764c => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002C3390: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2c3390<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2c3390;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x2c3390, previous, pc); previous = pc; }
        match pc {
            0x2c3390 => {
                crate::audio_lane::set_return(0x2c3395);
                call!(0x69640);
                pc = 0x2c3395; continue;
            }
            0x2c3395 => {
                eax = m.read32(eax.wrapping_add(0x120));
                fa = eax;
                fb = m.read32(0x5236bc);
                if (fa != fb) { pc = 0x2c3405; continue; }
                pc = 0x2c33a3; continue;
            }
            0x2c33a3 => {
                crate::audio_lane::set_return(0x2c33a8);
                call!(0x69640);
                pc = 0x2c33a8; continue;
            }
            0x2c33a8 => {
                ecx = m.read32(eax.wrapping_add(0x118));
                fa = ecx;
                fb = m.read32(0x5236b4);
                if (fa != fb) { pc = 0x2c3405; continue; }
                pc = 0x2c33b6; continue;
            }
            0x2c33b6 => {
                crate::audio_lane::set_return(0x2c33bb);
                call!(0x69640);
                pc = 0x2c33bb; continue;
            }
            0x2c33bb => {
                edx = m.read32(eax.wrapping_add(0x11c));
                fa = edx;
                fb = m.read32(0x5236b8);
                if (fa != fb) { pc = 0x2c3405; continue; }
                pc = 0x2c33c9; continue;
            }
            0x2c33c9 => {
                crate::audio_lane::set_return(0x2c33ce);
                call!(0x69640);
                pc = 0x2c33ce; continue;
            }
            0x2c33ce => {
                ecx = m.read32(0x7613d8);
                fa = m.read32(eax.wrapping_add(0x12c));
                fb = ecx;
                if (fa != fb) { pc = 0x2c3405; continue; }
                pc = 0x2c33dc; continue;
            }
            0x2c33dc => {
                crate::audio_lane::set_return(0x2c33e1);
                call!(0x69640);
                pc = 0x2c33e1; continue;
            }
            0x2c33e1 => {
                edx = m.read32(eax.wrapping_add(0x124));
                fa = edx;
                fb = m.read32(0x5236c0);
                if (fa != fb) { pc = 0x2c3405; continue; }
                pc = 0x2c33ef; continue;
            }
            0x2c33ef => {
                crate::audio_lane::set_return(0x2c33f4);
                call!(0x69640);
                pc = 0x2c33f4; continue;
            }
            0x2c33f4 => {
                eax = m.read32(eax.wrapping_add(0x130));
                fa = eax;
                fb = m.read32(0x7613dc);
                if (fa != fb) { pc = 0x2c3405; continue; }
                pc = 0x2c3402; continue;
            }
            0x2c3402 => {
                eax = 0;
                break 'run;
            }
            0x2c3405 => {
                eax = 0x1;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002D48D0: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2d48d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2d48d0;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x2d48d0, previous, pc); previous = pc; }
        match pc {
            0x2d48d0 => {
                esp = esp.wrapping_sub(0xc);
                push!(ebp);
                push!(esi);
                esi = ebx;
                esi = ((esi as i32).wrapping_mul((0x16c) as i32)) as u32;
                eax = m.read32(esi.wrapping_add(0x78a144));
                push!(edi);
                edi = m.read32(esi.wrapping_add(0x78a140));
                push!(eax);
                push!(edi);
                crate::audio_lane::set_return(0x2d48f1);
                call!(0x2be590);
                pc = 0x2d48f1; continue;
            }
            0x2d48f1 => {
                ecx = edi.wrapping_add(edi.wrapping_mul(0x2));
                ecx = ecx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = m.read32(ecx.wrapping_add(0x53a3d0));
                ebp = eax;
                eax = ebx;
                crate::audio_lane::set_return(0x2d4906);
                call!(0x2c43b0);
                pc = 0x2d4906; continue;
            }
            0x2d4906 => {
                edx = m.read32(esi.wrapping_add(0x78a144));
                eax = m.read32(esi.wrapping_add(0x78a140));
                push!(edx);
                push!(0x1);
                push!(0xffffffffu32);
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                push!(ebx);
                push!(eax);
                push!(eax);
                crate::audio_lane::set_return(0x2d4927);
                call!(0xdac30);
                pc = 0x2d4927; continue;
            }
            0x2d4927 => {
                m.write32(esi.wrapping_add(0x78a294), eax);
                eax = m.read32(esi.wrapping_add(0x78a144));
                m.write32(esi.wrapping_add(0x78a298), eax);
                edi = m.read32(ebp.wrapping_add(0x28));
                esp = esp.wrapping_add(0x30);
                crate::audio_lane::set_return(0x2d4944);
                call!(0x350c60);
                pc = 0x2d4944; continue;
            }
            0x2d4944 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2d495d; continue; }
                pc = 0x2d4948; continue;
            }
            0x2d4948 => {
                edx = m.read32(0x4ca9b4);
                ecx = edi.wrapping_add(edi.wrapping_mul(0x4));
                eax = m.read32(edx.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x8));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x2d495d; continue; }
                pc = 0x2d495a; continue;
            }
            0x2d495a => {
                edi = (edi | 0xffffffffu32);
                pc = 0x2d495d; continue;
            }
            0x2d495d => {
                eax = m.read32(esi.wrapping_add(0x78a2a8));
                fa = eax;
                fb = edi;
                if (fa == fb) { pc = 0x2d4982; continue; }
                pc = 0x2d4967; continue;
            }
            0x2d4967 => {
                push!(0);
                push!(eax);
                crate::audio_lane::set_return(0x2d496f);
                call!(0x46c40);
                pc = 0x2d496f; continue;
            }
            0x2d496f => {
                push!(0x1);
                push!(0);
                push!(edi);
                m.write32(esi.wrapping_add(0x78a2a8), edi);
                crate::audio_lane::set_return(0x2d497f);
                call!(0x46cb0);
                pc = 0x2d497f; continue;
            }
            0x2d497f => {
                esp = esp.wrapping_add(0x14);
                pc = 0x2d4982; continue;
            }
            0x2d4982 => {
                edi = m.read32(esi.wrapping_add(0x78a294));
                m.write32(esi.wrapping_add(0x78a27c), 0x1);
                eax = m.read32(ebp.wrapping_add(0x2c));
                edx = m.read32(edi.wrapping_add(0x4));
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                ecx = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x5210f0));
                push!(0x3f800000);
                push!(ecx);
                push!(edx);
                crate::audio_lane::set_return(0x2d49ae);
                call!(0x966e0);
                pc = 0x2d49ae; continue;
            }
            0x2d49ae => {
                ebp = m.read32(ebp.wrapping_add(0x2c));
                eax = ebp.wrapping_add(ebp.wrapping_mul(0x4));
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x5210fc));
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2d4a37; continue; }
                pc = 0x2d49c4; continue;
            }
            0x2d49c4 => {
                xmm0 = Xmm::default();
                push!(0);
                eax = eax.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(0x521888);
                push!(0);
                ecx = esp.wrapping_add(0x14);
                push!(ecx);
                ebp = eax;
                eax = m.read32(ebp.wrapping_add(0xc));
                edx = ecx;
                push!(edx);
                push!(eax);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x2d49f8);
                call!(0xa6f90);
                pc = 0x2d49f8; continue;
            }
            0x2d49f8 => {
                ecx = ebx;
                ecx = ((ecx as i32).wrapping_mul((0x54) as i32)) as u32;
                m.write32(esi.wrapping_add(0x78a280), eax);
                ecx = ecx.wrapping_add(0x7b67e0);
                m.write32(eax.wrapping_add(0x254), ecx);
                eax = m.read32(esi.wrapping_add(0x78a280));
                m.write32(eax.wrapping_add(0x28), (m.read32(eax.wrapping_add(0x28)) | 0x40000000));
                edx = m.read32(ebp);
                eax = m.read32(esi.wrapping_add(0x78a280));
                ecx = m.read32(eax.wrapping_add(0x4));
                push!(0x3f800000);
                push!(edx);
                push!(ecx);
                crate::audio_lane::set_return(0x2d4a34);
                call!(0x966e0);
                pc = 0x2d4a34; continue;
            }
            0x2d4a34 => {
                esp = esp.wrapping_add(0x20);
                pc = 0x2d4a37; continue;
            }
            0x2d4a37 => {
                push!(edi);
                crate::audio_lane::set_return(0x2d4a3d);
                call!(0x15bca0);
                pc = 0x2d4a3d; continue;
            }
            0x2d4a3d => {
                edx = ebx;
                edx = ((edx as i32).wrapping_mul((0x54) as i32)) as u32;
                edx = edx.wrapping_add(0x7b67e0);
                m.write32(edi.wrapping_add(0x254), edx);
                m.write32(edi.wrapping_add(0x28), (m.read32(edi.wrapping_add(0x28)) | 0x40000000));
                eax = (eax & 0xffff_ff00) | ((((m.read8(ebx.wrapping_add(0x75fcf4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x2d4a6e; continue; }
                pc = 0x2d4a62; continue;
            }
            0x2d4a62 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                if (fa != fb) { pc = 0x2d4a71; continue; }
                pc = 0x2d4a6e; continue;
            }
            0x2d4a6e => {
                xmm0 = Xmm::default();
                pc = 0x2d4a71; continue;
            }
            0x2d4a71 => {
                esi = m.read32(esi.wrapping_add(0x78a294));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(ebx.wrapping_mul(0x4).wrapping_add(0x78ae30), xmm0.lanes[0]);
                if (fa == fb) { pc = 0x2d4aa5; continue; }
                pc = 0x2d4a84; continue;
            }
            0x2d4a84 => {
                esi = m.read32(esi.wrapping_add(0x7c));
                fa = ((((m.read16(esi.wrapping_add(0x48)))) as u32) & 0xffffu32);
                fb = ((((0xa0i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x2d4aa5; continue; }
                pc = 0x2d4a8f; continue;
            }
            0x2d4a8f => {
                push!(0);
                push!(0x3f800000);
                esi = esi.wrapping_add(0xc68);
                push!(esi);
                crate::audio_lane::set_return(0x2d4aa2);
                call!(0x1252c0);
                pc = 0x2d4aa2; continue;
            }
            0x2d4aa2 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x2d4aa5; continue;
            }
            0x2d4aa5 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00311850: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f311850<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x311850;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x311850, previous, pc); previous = pc; }
        match pc {
            0x311850 => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = ((eax as i32).wrapping_mul((0x58) as i32)) as u32;
                eax = ((m.read16(eax.wrapping_add(0x52cf40))) as u16 as u32);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00350C40: Tier 0 candidate; opt-in only.
// portreg: va=0x00350c40 tier=0
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f350c40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x350c40;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x350c40, previous, pc); previous = pc; }
        match pc {
            0x350c40 => {
                eax = m.read32(esp.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x350c57; continue; }
                pc = 0x350c48; continue;
            }
            0x350c48 => {
                fa = eax;
                fb = ((((0xcai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x350c57; continue; }
                pc = 0x350c4f; continue;
            }
            0x350c4f => {
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x535b80));
                break 'run;
            }
            0x350c57 => {
                eax = 0xac44;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00350C60: Tier 0 candidate; opt-in only.
// portreg: va=0x00350c60 tier=0
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f350c60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x350c60;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x350c60, previous, pc); previous = pc; }
        match pc {
            0x350c60 => {
                eax = m.read32(0x4c0454);
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ecx = ecx.wrapping_sub(1);
                eax = (eax & ecx);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00377770: Tier 0 candidate; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f377770<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x377770;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); crate::audio_lane::set_return(saved); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x377770, previous, pc); previous = pc; }
        match pc {
            0x377770 => {
                esp = esp.wrapping_sub(0x30);
                push!(esi);
                esi = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = m.read32(esi.wrapping_mul(0x8).wrapping_add(0x548ec4));
                fa = eax;
                fb = ((((0x5e9i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x377793; continue; }
                pc = 0x377785; continue;
            }
            0x377785 => {
                crate::audio_lane::set_return(0x37778a);
                call!(0x350c60);
                pc = 0x37778a; continue;
            }
            0x37778a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x377798; continue; }
                pc = 0x37778e; continue;
            }
            0x37778e => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            0x377793 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3777df; continue; }
                pc = 0x377798; continue;
            }
            0x377798 => {
                esi = m.read32(esi.wrapping_mul(0x8).wrapping_add(0x548ec4));
                eax = esp.wrapping_add(0x4);
                push!(eax);
                crate::audio_lane::set_return(0x3777a9);
                call!(0x45390);
                pc = 0x3777a9; continue;
            }
            0x3777a9 => {
                xmm0 = Xmm::scalar(m.read32(edi));
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x4)));
                ecx = esp.wrapping_add(0x8);
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x8)));
                push!(ecx);
                m.write8(esp.wrapping_add(0x34), (0x1) as u8);
                m.write32(esp.wrapping_add(0xc), esi);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                crate::audio_lane::set_return(0x3777dc);
                call!(0x45820);
                pc = 0x3777dc; continue;
            }
            0x3777dc => {
                esp = esp.wrapping_add(0x8);
                pc = 0x3777df; continue;
            }
            0x3777df => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

pub const ROOTS: &[u32] = &[0x218d0,0x21970,0x21aa0,0x21b20,0x21c10,0x21d00,0x21d80,0x21e00,0x21e60,0x21ed0,0x21f60,0x21fb0,0x26bb0,0x26c00,0x26c30,0x26c90,0x26d20,0x26e80,0x26ec0,0x26f70,0x27030,0x27050,0x270d0,0x271a0,0x27290,0x27980,0x279c0,0x27bb0,0x27c20,0x27da0,0x27f70,0x27fa0,0x28000,0x28070,0x28610,0x28890,0x28900,0x28980,0x289c0,0x28a30,0x28a40,0x28a50,0x28a60,0x28a80,0x28aa0,0x28ae0,0x298b0,0x29b40,0x29c90,0x29cc0,0x29d10,0x29e60,0x29ea0,0x29ee0,0x43b70,0x43bd0,0x43c10,0x43c50,0x44890,0x44980,0x44a20,0x45090,0x450e0,0x45160,0x451f0,0x454d0,0x45500,0x455d0,0x45620,0x45630,0x45640,0x45670,0x45780,0x45790,0x457b0,0x458b0,0x45930,0x45970,0x459a0,0x459b0,0x459f0,0x45a50,0x45a70,0x45ab0,0x45af0,0x45b30,0x45b60,0x45d80,0x45de0,0x45e60,0x45eb0,0x46270,0x462c0,0x462d0,0x46350,0x464a0,0x46610,0x467a0,0x46b80,0x46c40,0x46c80,0x624a0,0x624b0,0x63c60,0x74f00,0xb7ba0,0xb9c00,0x164470,0x164730,0x1648d0,0x164900,0x164920,0x1649e0,0x164b60,0x164b80,0x164bf0,0x164c40,0x1651c0,0x165390,0x1b2430,0x1d96c0,0x207250,0x207270,0x2211a0,0x2212c0,0x221410,0x22dec0,0x247510,0x257620,0x2c3390,0x2d48d0,0x311850,0x350c40,0x350c60,0x377770];
/// Per-root A/B switch. Disabled/unknown roots leave state untouched.
pub fn run<M: GuestMemory,C: GuestCall>(va:u32,enabled:bool,m:&mut M,guest:&mut C,regs:&mut ExtRegisters)->bool { if !enabled {return false;} match va {
0x218d0 => f218d0(m,guest,regs),
0x21970 => f21970(m,guest,regs),
0x21aa0 => f21aa0(m,guest,regs),
0x21b20 => f21b20(m,guest,regs),
0x21c10 => f21c10(m,guest,regs),
0x21d00 => f21d00(m,guest,regs),
0x21d80 => f21d80(m,guest,regs),
0x21e00 => f21e00(m,guest,regs),
0x21e60 => f21e60(m,guest,regs),
0x21ed0 => f21ed0(m,guest,regs),
0x21f60 => f21f60(m,guest,regs),
0x21fb0 => f21fb0(m,guest,regs),
0x26bb0 => f26bb0(m,guest,regs),
0x26c00 => f26c00(m,guest,regs),
0x26c30 => f26c30(m,guest,regs),
0x26c90 => f26c90(m,guest,regs),
0x26d20 => f26d20(m,guest,regs),
0x26e80 => f26e80(m,guest,regs),
0x26ec0 => f26ec0(m,guest,regs),
0x26f70 => f26f70(m,guest,regs),
0x27030 => f27030(m,guest,regs),
0x27050 => f27050(m,guest,regs),
0x270d0 => f270d0(m,guest,regs),
0x271a0 => f271a0(m,guest,regs),
0x27290 => f27290(m,guest,regs),
0x27980 => f27980(m,guest,regs),
0x279c0 => f279c0(m,guest,regs),
0x27bb0 => f27bb0(m,guest,regs),
0x27c20 => f27c20(m,guest,regs),
0x27da0 => f27da0(m,guest,regs),
0x27f70 => f27f70(m,guest,regs),
0x27fa0 => f27fa0(m,guest,regs),
0x28000 => f28000(m,guest,regs),
0x28070 => f28070(m,guest,regs),
0x28610 => f28610(m,guest,regs),
0x28890 => f28890(m,guest,regs),
0x28900 => f28900(m,guest,regs),
0x28980 => f28980(m,guest,regs),
0x289c0 => f289c0(m,guest,regs),
0x28a30 => f28a30(m,guest,regs),
0x28a40 => f28a40(m,guest,regs),
0x28a50 => f28a50(m,guest,regs),
0x28a60 => f28a60(m,guest,regs),
0x28a80 => f28a80(m,guest,regs),
0x28aa0 => f28aa0(m,guest,regs),
0x28ae0 => f28ae0(m,guest,regs),
0x298b0 => f298b0(m,guest,regs),
0x29b40 => f29b40(m,guest,regs),
0x29c90 => f29c90(m,guest,regs),
0x29cc0 => f29cc0(m,guest,regs),
0x29d10 => f29d10(m,guest,regs),
0x29e60 => f29e60(m,guest,regs),
0x29ea0 => f29ea0(m,guest,regs),
0x29ee0 => f29ee0(m,guest,regs),
0x43b70 => f43b70(m,guest,regs),
0x43bd0 => f43bd0(m,guest,regs),
0x43c10 => f43c10(m,guest,regs),
0x43c50 => f43c50(m,guest,regs),
0x44890 => f44890(m,guest,regs),
0x44980 => f44980(m,guest,regs),
0x44a20 => f44a20(m,guest,regs),
0x45090 => f45090(m,guest,regs),
0x450e0 => f450e0(m,guest,regs),
0x45160 => f45160(m,guest,regs),
0x451f0 => f451f0(m,guest,regs),
0x454d0 => f454d0(m,guest,regs),
0x45500 => f45500(m,guest,regs),
0x455d0 => f455d0(m,guest,regs),
0x45620 => f45620(m,guest,regs),
0x45630 => f45630(m,guest,regs),
0x45640 => f45640(m,guest,regs),
0x45670 => f45670(m,guest,regs),
0x45780 => f45780(m,guest,regs),
0x45790 => f45790(m,guest,regs),
0x457b0 => f457b0(m,guest,regs),
0x458b0 => f458b0(m,guest,regs),
0x45930 => f45930(m,guest,regs),
0x45970 => f45970(m,guest,regs),
0x459a0 => f459a0(m,guest,regs),
0x459b0 => f459b0(m,guest,regs),
0x459f0 => f459f0(m,guest,regs),
0x45a50 => f45a50(m,guest,regs),
0x45a70 => f45a70(m,guest,regs),
0x45ab0 => f45ab0(m,guest,regs),
0x45af0 => f45af0(m,guest,regs),
0x45b30 => f45b30(m,guest,regs),
0x45b60 => f45b60(m,guest,regs),
0x45d80 => f45d80(m,guest,regs),
0x45de0 => f45de0(m,guest,regs),
0x45e60 => f45e60(m,guest,regs),
0x45eb0 => f45eb0(m,guest,regs),
0x46270 => f46270(m,guest,regs),
0x462c0 => f462c0(m,guest,regs),
0x462d0 => f462d0(m,guest,regs),
0x46350 => f46350(m,guest,regs),
0x464a0 => f464a0(m,guest,regs),
0x46610 => f46610(m,guest,regs),
0x467a0 => f467a0(m,guest,regs),
0x46b80 => f46b80(m,guest,regs),
0x46c40 => f46c40(m,guest,regs),
0x46c80 => f46c80(m,guest,regs),
0x624a0 => f624a0(m,guest,regs),
0x624b0 => f624b0(m,guest,regs),
0x63c60 => f63c60(m,guest,regs),
0x74f00 => f74f00(m,guest,regs),
0xb7ba0 => fb7ba0(m,guest,regs),
0xb9c00 => fb9c00(m,guest,regs),
0x164470 => f164470(m,guest,regs),
0x164730 => f164730(m,guest,regs),
0x1648d0 => f1648d0(m,guest,regs),
0x164900 => f164900(m,guest,regs),
0x164920 => f164920(m,guest,regs),
0x1649e0 => f1649e0(m,guest,regs),
0x164b60 => f164b60(m,guest,regs),
0x164b80 => f164b80(m,guest,regs),
0x164bf0 => f164bf0(m,guest,regs),
0x164c40 => f164c40(m,guest,regs),
0x1651c0 => f1651c0(m,guest,regs),
0x165390 => f165390(m,guest,regs),
0x1b2430 => f1b2430(m,guest,regs),
0x1d96c0 => f1d96c0(m,guest,regs),
0x207250 => f207250(m,guest,regs),
0x207270 => f207270(m,guest,regs),
0x2211a0 => f2211a0(m,guest,regs),
0x2212c0 => f2212c0(m,guest,regs),
0x221410 => f221410(m,guest,regs),
0x22dec0 => f22dec0(m,guest,regs),
0x247510 => f247510(m,guest,regs),
0x257620 => f257620(m,guest,regs),
0x2c3390 => f2c3390(m,guest,regs),
0x2d48d0 => f2d48d0(m,guest,regs),
0x311850 => f311850(m,guest,regs),
0x350c40 => f350c40(m,guest,regs),
0x350c60 => f350c60(m,guest,regs),
0x377770 => f377770(m,guest,regs),
_ => return false, } true }
pub use f218d0 as call_45820_with_initialized_id_scalar18_packet_flags_200;
pub use f21970 as sound_play_at_position_flag_0xa0_a;
pub use f21aa0 as sound_play_at_position_no_flags_a;
pub use f21b20 as sound_play_at_position_flag_0xa0_b;
pub use f21c10 as call_45820_with_initialized_id_scalar2c_scalar18_xyz_packet;
pub use f21d00 as sound_play_at_position_flag_0xa0_d;
pub use f21d80 as sound_play_at_position_flag_0x2a0;
pub use f21e00 as sound_request_submit_positional_with_id_and_scalar;
pub use f21e60 as call_45820_with_initialized_id_scalar18_scalar14_xyz_packet;
pub use f21ed0 as sound_play_at_position_caller_flags;
pub use f21f60 as call_45820_with_initialized_id_scalar18_scalar14_flags_or_packet;
pub use f21fb0 as sound_play_at_position_no_flags_b;
pub use f26bb0 as two_sentinel_lists_find_first_nonnegative_key_custom_ecx;
pub use f26c00 as sound_slot_free_list_push;
pub use f26c30 as sound_slot_free_list_init;
pub use f26c90 as sound_wave_format_fill;
pub use f26d20 as sound_position_relative_to_nearest_player;
pub use f26e80 as sound_name_table_find_substring;
pub use f26ec0 as buffer_set_aggregated_position_with_negated_third_custom_eax;
pub use f26f70 as sound_vag_extension_check;
pub use f27030 as global_58198c_get_if_5836c8_set;
pub use f27050 as sound_effects_image_download;
pub use f270d0 as sound_pause_all_not_flagged_0x800;
pub use f271a0 as sound_resume_all_paused;
pub use f27290 as sound_file_type_from_extension;
pub use f27980 as sound_stream_pool_init;
pub use f279c0 as sound_system_init;
pub use f27bb0 as sound_handle_is_playing_27bb0;
pub use f27c20 as sound_handle_set_position_3d;
pub use f27da0 as sound_stop_all_and_release_slots;
pub use f27f70 as sound_stop_all_wrapper;
pub use f27fa0 as indexed_record_ratio_to_scaled_word_table_value;
pub use f28000 as sound_handle_is_playing_28000;
pub use f28070 as sound_handle_set_max_distance;
pub use f28610 as sound_buffer_update_volume_and_playback_state;
pub use f28890 as sound_stream_slot_info_set_from_pak;
pub use f28900 as sound_music_stream_init_clear_slots_and_create;
pub use f28980 as sound_music_stream_set_volume_scaled;
pub use f289c0 as sound_music_stream_start_by_slot;
pub use f28a30 as sound_stream_64bd98_flush;
pub use f28a40 as sound_stream_64bd98_pause;
pub use f28a50 as sound_stream_64bd98_resume;
pub use f28a60 as sound_music_stream_is_loading_status_check;
pub use f28a80 as global_64bd90_set_scaled_by_475d1c;
pub use f28aa0 as sound_music_stream_release_and_flush;
pub use f28ae0 as sound_stream_64bd98_alloc_buffer;
pub use f298b0 as sound_stream_alloc_buffer_0x36000;
pub use f29b40 as sound_stream_flush;
pub use f29c90 as sound_stream_process_packet78_normalize_success;
pub use f29cc0 as sound_stream_state012_or_status_10000_without_20000;
pub use f29d10 as sound_stream_has_status_10000_without_20000;
pub use f29e60 as sound_stream_set_rolloff;
pub use f29ea0 as sound_stream_pause;
pub use f29ee0 as sound_stream_resume;
pub use f43b70 as sound_pad_level_entries_flag_2_clear_then_set_for_active_ids;
pub use f43bd0 as sound_pad_level_table_6b98b8_binary_search_by_key;
pub use f43c10 as sound_pad_level_table_binary_search_by_key_returns_index;
pub use f43c50 as sound_pad_level_parse;
pub use f44890 as sound_level_entry_start_or_update_handle_with_volume_lerp;
pub use f44980 as sound_pad_level_entry_mask_bit_set_propagate;
pub use f44a20 as sound_pad_level_entry_mask_bit_clear_propagate;
pub use f45090 as sound_pad_level_entries_release_handles_and_mark_unused;
pub use f450e0 as sound_pad_level_reset_and_parse_validated;
pub use f45160 as sound_pad_level_tables_release_then_clear_globals_unless;
pub use f451f0 as global_6d5eb0_set;
pub use f454d0 as thunk_sound_handle_set_max_distance;
pub use f45500 as sound_handle_is_playing_28000_including_deferred_slots;
pub use f455d0 as sound_deferred_slots_6d58a4_clear_all_states;
pub use f45620 as thunk_indexed_record_ratio_to_scaled_word_table_value;
pub use f45630 as wrapper_game_sound_system_init;
pub use f45640 as sound_deferred_slots_clear_states_then_jmp_27f70;
pub use f45670 as sound_record_load_flags_for_current_mode;
pub use f45780 as global_4caa04_set_float;
pub use f45790 as sound_volume_6b9930_approach_target_step_0p025;
pub use f457b0 as sound_volume_scale_blended_by_ramp;
pub use f458b0 as sound_deferred_requests_update;
pub use f45930 as sound_state_reset_and_stream_pool_init;
pub use f45970 as sound_volume_scales_4caa04_6b9930_reset_to_default;
pub use f459a0 as thunk_sound_music_stream_init_clear_slots_and_create;
pub use f459b0 as music_pak_path_get;
pub use f459f0 as thunk_sound_music_stream_set_volume_scaled;
pub use f45a50 as global_6d5eb8_get;
pub use f45a70 as music_volume_setting_is_zero_by_mode_cache_flag;
pub use f45ab0 as music_stream_pause_unless_volume_zero_sets;
pub use f45af0 as music_stream_resume_unless_volume_zero;
pub use f45b30 as music_set_volume_scaled_unless_volume_zero;
pub use f45b60 as music_stream_volume_from_slot_track_and_audio_settings;
pub use f45d80 as music_voice_mask_4cb54c_set_bit_then_load_stream_slot_info_from_pak;
pub use f45de0 as music_track_select_random_or_given;
pub use f45e60 as music_chunk_c77668_word_0xe_get_for_local_player_or_default;
pub use f45eb0 as music_pak_open;
pub use f46270 as global_6d5ec0_set;
pub use f462c0 as global_4d1848_set_float;
pub use f462d0 as global_4d1854_get;
pub use f46350 as music_volume_fade_tick_with_scale_4d1840_and_stream_update;
pub use f464a0 as table_6d5ec8_entries_reset_to_minus1_and_clear_floats;
pub use f46610 as sound_update_weighted_player_level_and_fading_handles;
pub use f467a0 as sound_deferred_entries_decay_scale_then_promote_smallest_to_free_active_slot_or_stop;
pub use f46b80 as table_6d6028_init;
pub use f46c40 as table_6d6028_release_by_id;
pub use f46c80 as table_6d6028_release_all;
pub use f624a0 as global_4d19ec_set;
pub use f624b0 as global_4d19ec_take_and_clear;
pub use f63c60 as store_span;
pub use f74f00 as sound_play_ui_effect_by_id_and_replace_tracked_handle;
pub use fb7ba0 as sound_lookup_index_to_id_remap_minus1_when_unmapped;
pub use fb9c00 as sound_play_random_pitch_variant;
pub use f164470 as table_4e7ce8_entry_ptr_by_game_mode_jump_table_with_object_state_and_tag_checks_or_null;
pub use f164730 as table_4e7ce8_entry_by_index_reset_table_4ca9b4_field_0xc_for_each_text_id;
pub use f1648d0 as voice_priority_globals_74c3f0_74c3d0_set_from_table_4e7ce8_entry_field_0x2c_for_object;
pub use f164900 as global_79094c_map_4_0x1b26_6_0x10e8;
pub use f164920 as voice_priority_globals_74c3ec_74c3c8_set_from_entry_field_0x2c_in_modes_4_and_6_when_idle;
pub use f1649e0 as priority_list_7a2800_insert_pair_by_key_truncating_lower_keys_max_4_count;
pub use f164b60 as flags_allow;
pub use f164b80 as table_4ca9b4_entry_reset_by_jump_table_on_global_4c0454_cases_1_to_4;
pub use f164bf0 as globals_74c3d4_74c3d8_74c3dc_set_minus1_counts_zeroed_table_4e7fe0_fill_minus_1000;
pub use f164c40 as object_start_voice_line_by_text_id_with_sound_frequency_scale_clear_target_handle_and_lipsync_slot;
pub use f1651c0 as voice_announcer_sound_handle_assign_in_table_6d5ec8_with_params_74c3d8_and_playing_check_by_id_0x37;
pub use f165390 as voice_table_4ca9b4_reset_by_match_state_7de470_jump_table_and_global_4c0454_case;
pub use f1b2430 as sound_submit_default_request_with_id_minus_one;
pub use f1d96c0 as scene_submit_sound_0x180_at_object_world_position_via_468a0;
pub use f207250 as crossfade_state_alloc_and_reset_sound_entries_0x1e_0x1f;
pub use f207270 as crossfade_state_init_then_flag_four_table_entries_0x20000_when_state_bit_0x40;
pub use f2211a0 as sound_handle_array_state_block_init_all_unset;
pub use f2212c0 as sound_handle_array_slot_start_sound_at_position;
pub use f221410 as sound_handle_array_start_slot_for_kind_with_aim_angles;
pub use f22dec0 as jump_table_by_state_7de470_0_to_0x14;
pub use f247510 as sound_play_once_store_handle_in_global;
pub use f257620 as sound_handle_is_playing_wrapper;
pub use f2c3390 as audio_settings_menu_globals_match_profile_values;
pub use f2d48d0 as setup_screen_spawn_slot_preview_object;
pub use f311850 as record_58_word_8_by_index;
pub use f350c40 as table_535b80_get_by_index_below_0xca_default_0xac44;
pub use f350c60 as get_global_4c0454_clamped_to_nonnegative;
pub use f377770 as sound_play_indexed_table_entry_at_position_skipping_id_0x5e9_when_gated;
