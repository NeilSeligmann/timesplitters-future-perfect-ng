// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-gameplay/slot_leaves
//! T1902: source-inspected, mechanically translated Tier 0 candidates.
//! Names/system placement INFERRED; no runtime admission. See docs/t-system-vehicles.md.
#![allow(clippy::all, unused_parens, unused_assignments, unused_imports, unused_variables)]
use crate::{abi::{ExtRegisters, GuestCall, Xmm}, memory::GuestMemory, pose::write16};
use crate::inventory::sse;
use crate::system_slice::{round_pc, read_double, sqrt_double, reciprocal_sqrt, compare_fp};

#[path = "system_slice/vehicles_boundary.rs"] mod boundary;
pub use boundary::*;

/// 0x00021860: Tier 0, lifted-source inspection; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f21860<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x21860;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x21860, previous, pc); previous = pc; }
        match pc {
            0x21860 => {
                esp = esp.wrapping_sub(0x30);
                eax = esp;
                push!(eax);
                call!(0x45390);
                pc = 0x2186c; continue;
            }
            0x2186c => {
                ecx = m.read32(esp.wrapping_add(0x38));
                edx = m.read32(esp.wrapping_add(0x3c));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                eax = esp.wrapping_add(0x4);
                push!(eax);
                m.write32(esp.wrapping_add(0x8), ecx);
                m.write32(esp.wrapping_add(0xc), edx);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                call!(0x45820);
                pc = 0x21892; continue;
            }
            0x21892 => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x168160: write one physics-entry field through the borrowed table view.
/// Tier 0, FABRICATED host-free access-contract test; index meaning is INFERRED.
pub fn f168160<M: GuestMemory, C: GuestCall>(m: &mut M, _guest: &mut C, regs: &mut ExtRegisters) {
    let index = m.read32(regs.esp.wrapping_add(4));
    let value = m.read32(regs.esp.wrapping_add(8));
    let table = PhysicsEntryTable::read(m);
    table.set_field(m, index, value);
    // Preserve the lifted callback's observable register results.
    regs.eax = (index & 0xffff).wrapping_shl(9);
    regs.ecx = value;
    regs.edx = table.0;
}

/// 0x001E1DC0: Tier 0, lifted-source inspection; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1e1dc0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1e1dc0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1e1dc0, previous, pc); previous = pc; }
        match pc {
            0x1e1dc0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(0x504c4e45);
                push!(eax);
                call!(0x27e960);
                pc = 0x1e1dcf; continue;
            }
            0x1e1dcf => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x001E1DE0: Tier 0, lifted-source inspection; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1e1de0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1e1de0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1e1de0, previous, pc); previous = pc; }
        match pc {
            0x1e1de0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(0x504c4e53);
                push!(eax);
                call!(0x27e940);
                pc = 0x1e1def; continue;
            }
            0x1e1def => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x001E1E00: Tier 0, lifted-source inspection; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1e1e00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1e1e00;
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
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1e1e00, previous, pc); previous = pc; }
        match pc {
            0x1e1e00 => {
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0xc));
                esi = m.read32(edi.wrapping_add(0x7c));
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(0x20))) as f64));
                push!(0x44524956);
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(0x1c))) as f64));
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(0x18))) as f64));
                { let value = fpu.st(0); fpu.push(value); }
                { let value = round_pc(fpu.st(0) * fpu.st(1), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = fpu.st(2); fpu.push(value); }
                { let value = round_pc(fpu.st(0) * fpu.st(3), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(1) + fpu.st(0), fpu.control); fpu.stack[((fpu.top + 1) & 7) as usize] = value; } fpu.pop();
                { let value = fpu.st(3); fpu.push(value); }
                { let value = round_pc(fpu.st(0) * fpu.st(4), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(1) + fpu.st(0), fpu.control); fpu.stack[((fpu.top + 1) & 7) as usize] = value; } fpu.pop();
                { let value = round_pc(sqrt_double(fpu.st(0)), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.stack[((fpu.top + 3) & 7) as usize] = fpu.st(0); fpu.pop();
                fpu.pop();
                fpu.pop();
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475c78)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x23ff20);
                pc = 0x1e1e3e; continue;
            }
            0x1e1e3e => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e1e69; continue; }
                pc = 0x1e1e45; continue;
            }
            0x1e1e45 => {
                ecx = m.read32(edi.wrapping_add(0x4));
                edx = m.read32(eax.wrapping_add(0x4));
                ecx = m.read32(ecx.wrapping_add(0xdc));
                m.write32(edx.wrapping_add(0xdc), ecx);
                edx = m.read32(edi.wrapping_add(0x4));
                eax = m.read32(eax.wrapping_add(0x4));
                ecx = m.read32(edx.wrapping_add(0x108));
                m.write32(eax.wrapping_add(0x108), ecx);
                pc = 0x1e1e69; continue;
            }
            0x1e1e69 => {
                push!(0x50415353);
                call!(0x23ff20);
                pc = 0x1e1e73; continue;
            }
            0x1e1e73 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e1e9e; continue; }
                pc = 0x1e1e7a; continue;
            }
            0x1e1e7a => {
                edx = m.read32(edi.wrapping_add(0x4));
                ecx = m.read32(eax.wrapping_add(0x4));
                edx = m.read32(edx.wrapping_add(0xdc));
                m.write32(ecx.wrapping_add(0xdc), edx);
                ecx = m.read32(edi.wrapping_add(0x4));
                edx = m.read32(eax.wrapping_add(0x4));
                eax = m.read32(ecx.wrapping_add(0x108));
                m.write32(edx.wrapping_add(0x108), eax);
                pc = 0x1e1e9e; continue;
            }
            0x1e1e9e => {
                push!(0);
                push!(0x3f800000);
                push!(0x42960000);
                push!(0x40000000);
                push!(0x1a0);
                edi = edi.wrapping_add(0x22c);
                push!(esi);
                push!(edi);
                call!(0x468a0);
                pc = 0x1e1ec1; continue;
            }
            0x1e1ec1 => {
                edi = m.read32(edi);
                esp = esp.wrapping_add(0x1c);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1e1ed8; continue; }
                pc = 0x1e1eca; continue;
            }
            0x1e1eca => {
                ecx = m.read32(esp.wrapping_add(0xc));
                push!(ecx);
                push!(edi);
                call!(0x454b0);
                pc = 0x1e1ed5; continue;
            }
            0x1e1ed5 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x1e1ed8; continue;
            }
            0x1e1ed8 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x001E2890: Tier 0, lifted-source inspection; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1e2890<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1e2890;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1e2890, previous, pc); previous = pc; }
        match pc {
            0x1e2890 => {
                esp = esp.wrapping_sub(0xa4);
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0xac));
                push!(esi);
                esi = m.read32(ebx.wrapping_add(0x7c));
                push!(edi);
                push!(0);
                push!(0x1);
                eax = esp.wrapping_add(0x78);
                push!(ebx);
                push!(eax);
                m.write32(esp.wrapping_add(0x2c), 0xffffffffu32);
                call!(0xa14b0);
                pc = 0x1e28ba; continue;
            }
            0x1e28ba => {
                eax = m.read32(ebx.wrapping_add(0x4));
                ecx = m.read32(eax.wrapping_add(0x8c));
                edx = m.read32(ecx.wrapping_add(0x18));
                ecx = m.read32(eax.wrapping_add(0x10));
                edx = edx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(ecx).wrapping_add(0x30)));
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                edx = m.read32(eax.wrapping_add(0x8c));
                ecx = m.read32(edx.wrapping_add(0x18));
                edx = m.read32(eax.wrapping_add(0x10));
                ecx = ecx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(edx).wrapping_add(0x34)));
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                ecx = m.read32(eax.wrapping_add(0x8c));
                edx = m.read32(ecx.wrapping_add(0x18));
                eax = m.read32(eax.wrapping_add(0x10));
                edx = edx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(eax).wrapping_add(0x38)));
                ecx = esp.wrapping_add(0x20);
                push!(ecx);
                edx = esp.wrapping_add(0x84);
                push!(edx);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                call!(0x151e80);
                pc = 0x1e2920; continue;
            }
            0x1e2920 => {
                push!(0xf);
                eax = esp.wrapping_add(0x50);
                push!(eax);
                push!(0x504c4e45);
                call!(0x240070);
                pc = 0x1e2931; continue;
            }
            0x1e2931 => {
                ecx = eax;
                eax = m.read32(esi.wrapping_add(0x4));
                esp = esp.wrapping_add(0x24);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x2c), ecx);
                if (fa == fb) { pc = 0x1e29a7; continue; }
                pc = 0x1e2941; continue;
            }
            0x1e2941 => {
                edx = eax;
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(0x64)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                eax = m.read32(esi.wrapping_add(0x4));
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x68)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm1.lanes[0]);
                edx = m.read32(esi.wrapping_add(0x4));
                xmm3 = Xmm::scalar(m.read32(edx.wrapping_add(0x6c)));
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm3.lanes[0]);
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xbc)))).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm3.lanes[0]);
                pc = 0x1e29b8; continue;
            }
            0x1e29a7 => {
                xmm0 = Xmm::scalar(m.read32(0x475d5c));
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                xmm3 = xmm0;
                pc = 0x1e29b8; continue;
            }
            0x1e29b8 => {
                edi = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x1e2abb; continue; }
                pc = 0x1e29c2; continue;
            }
            0x1e29c2 => {
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xbc));
                pc = 0x1e29d0; continue;
            }
            0x1e29d0 => {
                eax = m.read32(esp.wrapping_add(edi.wrapping_mul(0x4)).wrapping_add(0x38));
                fa = ((((m.read16(eax.wrapping_add(0x94)))) as u32) & 0xffffu32);
                fb = ((((0x270i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x1e2a9b; continue; }
                pc = 0x1e29e3; continue;
            }
            0x1e29e3 => {
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x64)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm1.lanes[0]);
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x68)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm2.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x6c)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                xmm4 = xmm2;
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2 = xmm1;
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm3.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x1e2a9b; continue; }
                pc = 0x1e2a3b; continue;
            }
            0x1e2a3b => {
                ecx = m.read32(eax.wrapping_add(0x28));
                esi = ecx;
                esi = (esi & 0x60);
                edx = esi;
                edx = !edx;
                edx = (edx & ecx);
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(eax.wrapping_add(0x28), edx);
                if (fa != fb) { pc = 0x1e2a7b; continue; }
                pc = 0x1e2a50; continue;
            }
            0x1e2a50 => {
                edx = m.read32(ebx.wrapping_add(0x34));
                push!(ebp);
                push!(ebp);
                push!(0x2410278);
                push!(ebp);
                push!(ebp);
                push!(ebp);
                push!(ebp);
                eax = esp.wrapping_add(0x40);
                push!(eax);
                ecx = esp.wrapping_add(0x34);
                push!(ecx);
                push!(edx);
                call!(0x88c60);
                pc = 0x1e2a6e; continue;
            }
            0x1e2a6e => {
                esp = esp.wrapping_add(0x28);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1e2a85; continue; }
                pc = 0x1e2a75; continue;
            }
            0x1e2a75 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                pc = 0x1e2a7b; continue;
            }
            0x1e2a7b => {
                m.write32(esp.wrapping_add(0x20), edi);
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                pc = 0x1e2a85; continue;
            }
            0x1e2a85 => {
                eax = m.read32(esp.wrapping_add(edi.wrapping_mul(0x4)).wrapping_add(0x38));
                ecx = m.read32(eax.wrapping_add(0x28));
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                ecx = (ecx | esi);
                m.write32(eax.wrapping_add(0x28), ecx);
                ecx = m.read32(esp.wrapping_add(0x30));
                pc = 0x1e2a9b; continue;
            }
            0x1e2a9b => {
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1e29d0; continue; }
                pc = 0x1e2aa4; continue;
            }
            0x1e2aa4 => {
                eax = m.read32(esp.wrapping_add(0x20));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                if (fas < fbs) { pc = 0x1e2abb; continue; }
                pc = 0x1e2aad; continue;
            }
            0x1e2aad => {
                eax = m.read32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x34));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xa4);
                break 'run;
            }
            0x1e2abb => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xa4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x001E2AD0: Tier 0, lifted-source inspection; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1e2ad0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1e2ad0;
    let mut jt = 0u32;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1e2ad0, previous, pc); previous = pc; }
        match pc {
            0x1e2ad0 => {
                eax = m.read32(0x7b0c7c);
                eax = m.read32(eax.wrapping_add(0xf54));
                eax = eax.wrapping_sub(1);
                fa = eax;
                fb = ((((0x5bi32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x1e2b07; continue; }
                pc = 0x1e2ae1; continue;
            }
            0x1e2ae1 => {
                ecx = ((m.read8(eax.wrapping_add(0x1e2b20))) as u8 as u32);
                jt = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x1e2b0c));
                if (jt == 0x1e2aefu32) { pc = 0x1e2aef; continue; }
                if (jt == 0x1e2af5u32) { pc = 0x1e2af5; continue; }
                if (jt == 0x1e2afbu32) { pc = 0x1e2afb; continue; }
                if (jt == 0x1e2b01u32) { pc = 0x1e2b01; continue; }
                if (jt == 0x1e2b07u32) { pc = 0x1e2b07; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x1e2aef => {
                eax = 0x1;
                break 'run;
            }
            0x1e2af5 => {
                eax = 0x2;
                break 'run;
            }
            0x1e2afb => {
                eax = 0x3;
                break 'run;
            }
            0x1e2b01 => {
                eax = 0x4;
                break 'run;
            }
            0x1e2b07 => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x001E2B80: Tier 0, lifted-source inspection; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1e2b80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
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
    let mut pc: u32 = 0x1e2b80;
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
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1e2b80, previous, pc); previous = pc; }
        match pc {
            0x1e2b80 => {
                esp = esp.wrapping_sub(0x74);
                push!(ebx);
                push!(esi);
                push!(edi);
                call!(0x1e2ad0);
                pc = 0x1e2b8b; continue;
            }
            0x1e2b8b => {
                ecx = m.read32(0x75bc04);
                m.write32(ecx.wrapping_add(0x674), eax);
                xmm0.lanes[0] = (((m.read32(eax.wrapping_mul(0x4).wrapping_add(0x4fea1c))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4785d4))).to_bits();
                push!(0x3ca3d70a);
                push!(ecx);
                ecx = ecx.wrapping_add(0x680);
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                call!(0x1515e0);
                pc = 0x1e2bbf; continue;
            }
            0x1e2bbf => {
                eax = m.read32(0x4fea30);
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = m.read32(0x75bc04);
                if (fa == fb) { pc = 0x1e3158; continue; }
                pc = 0x1e2bd4; continue;
            }
            0x1e2bd4 => {
                ecx = m.read32(eax.wrapping_add(0x674));
                fa = ecx;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e2be8; continue; }
                pc = 0x1e2bdf; continue;
            }
            0x1e2bdf => {
                fa = ecx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1e3158; continue; }
                pc = 0x1e2be8; continue;
            }
            0x1e2be8 => {
                eax = m.read32(0x75bc0c);
                edi = m.read32(0x4e7944);
                esi = m.read32(0x4e8cec);
                eax = eax.wrapping_add(edi);
                ecx = 0;
                fa = esi;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fas > fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                m.write32(0x75bc0c, eax);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x1e2e41; continue; }
                pc = 0x1e2c10; continue;
            }
            0x1e2c10 => {
                call!(0x153b40);
                pc = 0x1e2c15; continue;
            }
            0x1e2c15 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                fpu.push((m.read32(esp.wrapping_add(0x1c)) as i32 as f64));
                if (fas >= fbs) { pc = 0x1e2c27; continue; }
                pc = 0x1e2c21; continue;
            }
            0x1e2c21 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x1e2c27; continue;
            }
            0x1e2c27 => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.push((f32::from_bits(m.read32(0x47d6dc)) as f64));
                { let a = fpu.top as usize; let b = ((fpu.top + 1) & 7) as usize; fpu.stack.swap(a, b); }
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if fp_cmp != 1 { pc = 0x001e2e41; continue; }
                pc = 0x1e2c3f; continue;
            }
            0x1e2c3f => {
                eax = m.read32(0x75bc10);
                edi = eax.wrapping_add(eax.wrapping_mul(0x2));
                push!(0x504c4e45);
                edi = edi.wrapping_mul(0x4).wrapping_add(0x4fea34);
                call!(0x2401f0);
                pc = 0x1e2c58; continue;
            }
            0x1e2c58 => {
                esi = eax;
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e2e41; continue; }
                pc = 0x1e2c65; continue;
            }
            0x1e2c65 => {
                eax = m.read32(esi.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e2e41; continue; }
                pc = 0x1e2c70; continue;
            }
            0x1e2c70 => {
                ecx = m.read32(eax.wrapping_add(0x10));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e2e41; continue; }
                pc = 0x1e2c7b; continue;
            }
            0x1e2c7b => {
                xmm0 = Xmm::default();
                edx = esp.wrapping_add(0x2c);
                push!(edx);
                eax = ecx;
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                push!(eax);
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                m.write32(0x75bc0c, 0);
                call!(0x152090);
                pc = 0x1e2caf; continue;
            }
            0x1e2caf => {
                ecx = m.read32(0x75bc04);
                edx = m.read32(ecx.wrapping_add(0x760));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                edx = ((edx as i32).wrapping_mul((0x2c) as i32)) as u32;
                m.write32(edx.wrapping_add(ecx).wrapping_add(0x69c), xmm0.lanes[0]);
                eax = m.read32(ecx.wrapping_add(0x760));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                m.write32(eax.wrapping_add(ecx).wrapping_add(0x6a0), xmm0.lanes[0]);
                edx = m.read32(ecx.wrapping_add(0x760));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                edx = ((edx as i32).wrapping_mul((0x2c) as i32)) as u32;
                m.write32(edx.wrapping_add(ecx).wrapping_add(0x6a4), xmm0.lanes[0]);
                eax = m.read32(ecx.wrapping_add(0x760));
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                fpu.push((f32::from_bits(m.read32(eax.wrapping_add(ecx).wrapping_add(0x69c))) as f64));
                { let value = round_pc(fpu.st(0) * read_double(m, 0x49e120), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(eax.wrapping_add(ecx).wrapping_add(0x69c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                edx = m.read32(ecx.wrapping_add(0x760));
                eax = eax.wrapping_add(ecx).wrapping_add(0x69c);
                edx = ((edx as i32).wrapping_mul((0x2c) as i32)) as u32;
                fpu.push((f32::from_bits(m.read32(edx.wrapping_add(ecx).wrapping_add(0x6a0))) as f64));
                { let value = round_pc(fpu.st(0) * read_double(m, 0x49e120), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(edx.wrapping_add(ecx).wrapping_add(0x6a0), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = edx.wrapping_add(ecx).wrapping_add(0x6a0);
                eax = m.read32(ecx.wrapping_add(0x760));
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                fpu.push((f32::from_bits(m.read32(eax.wrapping_add(ecx).wrapping_add(0x6a4))) as f64));
                { let value = round_pc(fpu.st(0) * read_double(m, 0x49e120), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(eax.wrapping_add(ecx).wrapping_add(0x6a4), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x34))) as f64));
                { let value = round_pc(fpu.st(0) * read_double(m, 0x49e118), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x38))) as f64));
                edx = m.read32(ecx.wrapping_add(0x760));
                { let value = round_pc(fpu.st(0) * read_double(m, 0x49e118), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                eax = eax.wrapping_add(ecx).wrapping_add(0x6a4);
                edx = ((edx as i32).wrapping_mul((0x2c) as i32)) as u32;
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x3c))) as f64));
                { let value = round_pc(fpu.st(0) * read_double(m, 0x49e118), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x54), (fpu.st(0) as f32).to_bits()); fpu.pop();
                { let a = fpu.top as usize; let b = ((fpu.top + 1) & 7) as usize; fpu.stack.swap(a, b); }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esi.wrapping_add(0x64))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                esp = esp.wrapping_add(0x8);
                m.write32(edx.wrapping_add(ecx).wrapping_add(0x684), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = m.read32(ecx.wrapping_add(0x760));
                eax = eax.wrapping_add(0x26);
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esi.wrapping_add(0x68))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                m.write32(eax.wrapping_add(ecx), (fpu.st(0) as f32).to_bits()); fpu.pop();
                edx = m.read32(ecx.wrapping_add(0x760));
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x4c))) as f64));
                edx = ((edx as i32).wrapping_mul((0x2c) as i32)) as u32;
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esi.wrapping_add(0x6c))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(edx.wrapping_add(ecx).wrapping_add(0x68c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = m.read32(ecx.wrapping_add(0x760));
                edx = m.read32(edi);
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                m.write32(eax.wrapping_add(ecx).wrapping_add(0x690), edx);
                eax = m.read32(ecx.wrapping_add(0x760));
                edx = m.read32(edi.wrapping_add(0x4));
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                m.write32(eax.wrapping_add(ecx).wrapping_add(0x694), edx);
                eax = m.read32(ecx.wrapping_add(0x760));
                edx = m.read32(edi.wrapping_add(0x8));
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                m.write32(eax.wrapping_add(ecx).wrapping_add(0x698), edx);
                eax = m.read32(ecx.wrapping_add(0x760));
                eax = ((eax as i32).wrapping_mul((0x2c) as i32)) as u32;
                m.write32(eax.wrapping_add(ecx).wrapping_add(0x6a8), 0x5);
                edx = m.read32(ecx.wrapping_add(0x760));
                edx = ((edx as i32).wrapping_mul((0x2c) as i32)) as u32;
                m.write32(edx.wrapping_add(ecx).wrapping_add(0x6ac), 0);
                eax = m.read32(ecx.wrapping_add(0x760));
                eax = eax.wrapping_add(1);
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                esi = 0x5;
                { let dividend = ((edx as i32 as i64) << 32) | eax as i64;
                eax = (dividend / (esi as i32 as i64)) as i32 as u32;
                edx = (dividend % (esi as i32 as i64)) as i32 as u32; }
                m.write32(ecx.wrapping_add(0x760), edx);
                pc = 0x1e2e41; continue;
            }
            0x1e2e41 => {
                call!(0x153b40);
                pc = 0x1e2e46; continue;
            }
            0x1e2e46 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                fpu.push((m.read32(esp.wrapping_add(0x1c)) as i32 as f64));
                if (fas >= fbs) { pc = 0x1e2e58; continue; }
                pc = 0x1e2e52; continue;
            }
            0x1e2e52 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x1e2e58; continue;
            }
            0x1e2e58 => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.push((f32::from_bits(m.read32(0x475cd4)) as f64));
                { let a = fpu.top as usize; let b = ((fpu.top + 1) & 7) as usize; fpu.stack.swap(a, b); }
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if fp_cmp != 1 { pc = 0x001e2e73; continue; }
                pc = 0x1e2e6c; continue;
            }
            0x1e2e6c => {
                edi = 0x1;
                pc = 0x1e2e75; continue;
            }
            0x1e2e73 => {
                edi = 0;
                pc = 0x1e2e75; continue;
            }
            0x1e2e75 => {
                eax = m.read32(0x75bc04);
                esi = 0;
                ebx = 0xc6f;
                pc = 0x1e2e81; continue;
            }
            0x1e2e81 => {
                ecx = m.read32(esi.wrapping_add(eax).wrapping_add(0x6a8));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e3149; continue; }
                pc = 0x1e2e90; continue;
            }
            0x1e2e90 => {
                edx = m.read32(esi.wrapping_add(eax).wrapping_add(0x6ac));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e2ea8; continue; }
                pc = 0x1e2e9b; continue;
            }
            0x1e2e9b => {
                edx = edx.wrapping_sub(1);
                m.write32(esi.wrapping_add(eax).wrapping_add(0x6ac), edx);
                pc = 0x1e3149; continue;
            }
            0x1e2ea8 => {
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(eax).wrapping_add(0x684))) as f64));
                m.write32(esi.wrapping_add(eax).wrapping_add(0x6ac), 0xa);
                ecx = ecx.wrapping_sub(1);
                m.write32(esi.wrapping_add(eax).wrapping_add(0x6a8), ecx);
                m.write32(esp.wrapping_add(0x20), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(eax).wrapping_add(0x688))) as f64));
                m.write32(esp.wrapping_add(0x24), (fpu.st(0) as f32).to_bits()); fpu.pop();
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(eax).wrapping_add(0x68c))) as f64));
                m.write32(esp.wrapping_add(0x28), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x153b40);
                pc = 0x1e2ee1; continue;
            }
            0x1e2ee1 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                fpu.push((m.read32(esp.wrapping_add(0x1c)) as i32 as f64));
                if (fas >= fbs) { pc = 0x1e2ef3; continue; }
                pc = 0x1e2eed; continue;
            }
            0x1e2eed => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x1e2ef3; continue;
            }
            0x1e2ef3 => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475cd4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x47862c)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x20))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x20), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x153b40);
                pc = 0x1e2f12; continue;
            }
            0x1e2f12 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                fpu.push((m.read32(esp.wrapping_add(0x1c)) as i32 as f64));
                if (fas >= fbs) { pc = 0x1e2f24; continue; }
                pc = 0x1e2f1e; continue;
            }
            0x1e2f1e => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x1e2f24; continue;
            }
            0x1e2f24 => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475cd4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x47862c)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x24), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x153b40);
                pc = 0x1e2f43; continue;
            }
            0x1e2f43 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                fpu.push((m.read32(esp.wrapping_add(0x1c)) as i32 as f64));
                if (fas >= fbs) { pc = 0x1e2f55; continue; }
                pc = 0x1e2f4f; continue;
            }
            0x1e2f4f => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x1e2f55; continue;
            }
            0x1e2f55 => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                eax = m.read32(0x75bc04);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475cd4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x47862c)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x28))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x28), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(eax).wrapping_add(0x690)))).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                m.write32(esp.wrapping_add(0x2c), xmm1.lanes[0]);
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(eax).wrapping_add(0x694)))).to_bits();
                m.write32(esp.wrapping_add(0x30), xmm2.lanes[0]);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(eax).wrapping_add(0x698)))).to_bits();
                xmm3 = xmm2;
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2 = xmm1;
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x4765f4)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x1e302c; continue; }
                pc = 0x1e2fd6; continue;
            }
            0x1e2fd6 => {
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0.lanes[0] = reciprocal_sqrt(f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                m.write32(esp.wrapping_add(0x2c), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x34)))).to_bits();
                m.write32(esp.wrapping_add(0x30), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                pc = 0x1e302c; continue;
            }
            0x1e302c => {
                push!(0x1);
                push!(0);
                push!(0);
                push!(0);
                eax = esp.wrapping_add(0x3c);
                push!(eax);
                ecx = esp.wrapping_add(0x34);
                push!(ecx);
                push!(0xffffffffu32);
                push!(0xf);
                call!(0xefbd0);
                pc = 0x1e3047; continue;
            }
            0x1e3047 => {
                eax = m.read32(0x75bc04);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(eax).wrapping_add(0x69c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(eax).wrapping_add(0x684)))).to_bits();
                m.write32(esi.wrapping_add(eax).wrapping_add(0x684), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(eax).wrapping_add(0x6a0)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(eax).wrapping_add(0x688)))).to_bits();
                m.write32(esi.wrapping_add(eax).wrapping_add(0x688), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(eax).wrapping_add(0x6a4)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(eax).wrapping_add(0x68c)))).to_bits();
                esp = esp.wrapping_add(0x20);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esi.wrapping_add(eax).wrapping_add(0x68c), xmm0.lanes[0]);
                if (fa != fb) { pc = 0x1e3149; continue; }
                pc = 0x1e30a8; continue;
            }
            0x1e30a8 => {
                fpu.push((f32::from_bits(m.read32(eax.wrapping_add(0x680))) as f64));
                edi = 0x1;
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x47d5bc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0xc), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x153b40);
                pc = 0x1e30c2; continue;
            }
            0x1e30c2 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                fpu.push((m.read32(esp.wrapping_add(0x1c)) as i32 as f64));
                if (fas >= fbs) { pc = 0x1e30d4; continue; }
                pc = 0x1e30ce; continue;
            }
            0x1e30ce => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x1e30d4; continue;
            }
            0x1e30d4 => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(esp.wrapping_add(0xc))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0xc))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x1c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x153b40);
                pc = 0x1e30eb; continue;
            }
            0x1e30eb => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0xc), eax);
                fpu.push((m.read32(esp.wrapping_add(0xc)) as i32 as f64));
                if (fas >= fbs) { pc = 0x1e30fd; continue; }
                pc = 0x1e30f7; continue;
            }
            0x1e30f7 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x1e30fd; continue;
            }
            0x1e30fd => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                edx = esp.wrapping_add(0x50);
                push!(edx);
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475cd4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475d24)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475c78)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x45390);
                pc = 0x1e3123; continue;
            }
            0x1e3123 => {
                eax = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(esp.wrapping_add(0x20));
                edx = esp.wrapping_add(0x54);
                push!(edx);
                m.write32(esp.wrapping_add(0x58), ebx);
                m.write32(esp.wrapping_add(0x6c), eax);
                m.write32(esp.wrapping_add(0x70), ecx);
                call!(0x45820);
                pc = 0x1e3141; continue;
            }
            0x1e3141 => {
                eax = m.read32(0x75bc04);
                esp = esp.wrapping_add(0x8);
                pc = 0x1e3149; continue;
            }
            0x1e3149 => {
                esi = esi.wrapping_add(0x2c);
                fa = esi;
                fb = ((((0xdci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1e2e81; continue; }
                pc = 0x1e3158; continue;
            }
            0x1e3158 => {
                fa = m.read32(eax.wrapping_add(0x674));
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1e32c6; continue; }
                pc = 0x1e3165; continue;
            }
            0x1e3165 => {
                call!(0x153b40);
                pc = 0x1e316a; continue;
            }
            0x1e316a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                fpu.push((m.read32(esp.wrapping_add(0x1c)) as i32 as f64));
                if (fas >= fbs) { pc = 0x1e317c; continue; }
                pc = 0x1e3176; continue;
            }
            0x1e3176 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x1e317c; continue;
            }
            0x1e317c => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.push((f32::from_bits(m.read32(0x475dc8)) as f64));
                { let a = fpu.top as usize; let b = ((fpu.top + 1) & 7) as usize; fpu.stack.swap(a, b); }
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if fp_cmp != 1 { pc = 0x001e32c1; continue; }
                pc = 0x1e3194; continue;
            }
            0x1e3194 => {
                eax = m.read32(0x7b0c7c);
                fa = m.read32(eax.wrapping_add(0x114));
                fb = ((((0x100i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e32c1; continue; }
                pc = 0x1e31a9; continue;
            }
            0x1e31a9 => {
                eax = m.read32(eax.wrapping_add(0x8));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e32c1; continue; }
                pc = 0x1e31b4; continue;
            }
            0x1e31b4 => {
                xmm0 = Xmm::default();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(eax.wrapping_add(0x398))) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x1e32c1; continue; }
                pc = 0x1e31c4; continue;
            }
            0x1e31c4 => {
                call!(0x153b40);
                pc = 0x1e31c9; continue;
            }
            0x1e31c9 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                fpu.push((m.read32(esp.wrapping_add(0x1c)) as i32 as f64));
                if (fas >= fbs) { pc = 0x1e31db; continue; }
                pc = 0x1e31d5; continue;
            }
            0x1e31d5 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x1e31db; continue;
            }
            0x1e31db => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ecx);
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785e0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475cdc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits());
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475cd4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp, (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(0x1a6);
                call!(0x218a0);
                pc = 0x1e3205; continue;
            }
            0x1e3205 => {
                esp = esp.wrapping_add(0x8);
                call!(0x153b40);
                pc = 0x1e320d; continue;
            }
            0x1e320d => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                fpu.push((m.read32(esp.wrapping_add(0x1c)) as i32 as f64));
                if (fas >= fbs) { pc = 0x1e321f; continue; }
                pc = 0x1e3219; continue;
            }
            0x1e3219 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475ccc)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                pc = 0x1e321f; continue;
            }
            0x1e321f => {
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x4785d0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fpu.push((f32::from_bits(m.read32(0x475d40)) as f64));
                { let a = fpu.top as usize; let b = ((fpu.top + 1) & 7) as usize; fpu.stack.swap(a, b); }
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if fp_cmp != 1 { pc = 0x001e3252; continue; }
                pc = 0x1e3233; continue;
            }
            0x1e3233 => {
                xmm0.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                push!(0x3f000000);
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                push!(0x1a7);
                call!(0x21860);
                pc = 0x1e324f; continue;
            }
            0x1e324f => {
                esp = esp.wrapping_add(0xc);
                pc = 0x1e3252; continue;
            }
            0x1e3252 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                ecx = m.read32(0x7b0c7c);
                esp = esp.wrapping_sub(0x8);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475cf8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                m.write32(esp, xmm0.lanes[0]);
                push!(0x3f19999a);
                push!(ecx);
                call!(0xd1ec0);
                pc = 0x1e328a; continue;
            }
            0x1e328a => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                edx = m.read32(0x7b0c7c);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x476040))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cc8))).to_bits();
                m.write32(edx.wrapping_add(0xfbc), xmm1.lanes[0]);
                eax = m.read32(0x7b0c7c);
                esp = esp.wrapping_add(0x10);
                m.write32(eax.wrapping_add(0xfc0), xmm0.lanes[0]);
                pc = 0x1e32c1; continue;
            }
            0x1e32c1 => {
                eax = m.read32(0x75bc04);
                pc = 0x1e32c6; continue;
            }
            0x1e32c6 => {
                edi = m.read32(0x7a20c0);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e3336; continue; }
                pc = 0x1e32d0; continue;
            }
            0x1e32d0 => {
                ecx = m.read32(edi.wrapping_add(0x48));
                push!(ecx);
                call!(0x1679d0);
                pc = 0x1e32d9; continue;
            }
            0x1e32d9 => {
                xmm0 = Xmm::scalar(m.read32(0x47da88));
                esp = esp.wrapping_add(0x4);
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(eax.wrapping_add(0x4))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x1e332b; continue; }
                pc = 0x1e32ea; continue;
            }
            0x1e32ea => {
                esi = edi.wrapping_add(0x70);
                ebx = 0x4;
                pc = 0x1e32f2; continue;
            }
            0x1e32f2 => {
                eax = m.read32(esi);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e3325; continue; }
                pc = 0x1e32f8; continue;
            }
            0x1e32f8 => {
                ecx = eax;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e331a; continue; }
                pc = 0x1e32fe; continue;
            }
            0x1e32fe => {
                edx = m.read32(0x72f5e8);
                eax = eax.wrapping_shr((((((((0x16i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = ((eax as i32).wrapping_mul((0x2e8) as i32)) as u32;
                eax = eax.wrapping_add(edx);
                fa = m.read32(eax.wrapping_add(0x2c));
                fb = ecx;
                if (fa == fb) { pc = 0x1e331c; continue; }
                pc = 0x1e3314; continue;
            }
            0x1e3314 => {
                m.write32(esi, 0);
                pc = 0x1e331a; continue;
            }
            0x1e331a => {
                eax = 0;
                pc = 0x1e331c; continue;
            }
            0x1e331c => {
                push!(eax);
                call!(0xbd090);
                pc = 0x1e3322; continue;
            }
            0x1e3322 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x1e3325; continue;
            }
            0x1e3325 => {
                esi = esi.wrapping_add(0x34);
                ebx = ebx.wrapping_sub(1);
                fa = ebx;
                if ((fa != 0)) { pc = 0x1e32f2; continue; }
                pc = 0x1e332b; continue;
            }
            0x1e332b => {
                edi = m.read32(edi);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1e32d0; continue; }
                pc = 0x1e3331; continue;
            }
            0x1e3331 => {
                eax = m.read32(0x75bc04);
                pc = 0x1e3336; continue;
            }
            0x1e3336 => {
                edx = m.read32(eax.wrapping_add(0x680));
                push!(0);
                push!(0x3f800000);
                push!(0x42480000);
                push!(edx);
                push!(0x3);
                eax = eax.wrapping_add(0x7b4);
                push!(0);
                push!(eax);
                call!(0x468a0);
                pc = 0x1e3358; continue;
            }
            0x1e3358 => {
                eax = m.read32(0x75bc04);
                ecx = m.read32(eax.wrapping_add(0x7c0));
                push!(0);
                push!(0x3f800000);
                push!(0x42480000);
                push!(ecx);
                push!(0x21);
                eax = eax.wrapping_add(0x7bc);
                push!(0);
                push!(eax);
                call!(0x468a0);
                pc = 0x1e337f; continue;
            }
            0x1e337f => {
                eax = m.read32(0x75bc04);
                ecx = m.read32(eax.wrapping_add(0x674));
                esp = esp.wrapping_add(0x38);
                fa = ecx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa != fb) { pc = 0x1e36ef; continue; }
                pc = 0x1e3399; continue;
            }
            0x1e3399 => {
                edx = m.read32(eax.wrapping_add(0x4f8));
                xmm0 = Xmm::default();
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                push!(edx);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x34), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                call!(0x10e6e0);
                pc = 0x1e33c2; continue;
            }
            0x1e33c2 => {
                eax = m.read32(0x75bc04);
                ecx = m.read32(eax.wrapping_add(0x4f8));
                push!(0x3f800000);
                push!(ecx);
                call!(0x10eb50);
                pc = 0x1e33d8; continue;
            }
            0x1e33d8 => {
                eax = m.read32(0x7b0c7c);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0xb4)));
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0xb8)));
                m.write32(esp.wrapping_add(0x24), xmm1.lanes[0]);
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0xbc)));
                m.write32(esp.wrapping_add(0x28), xmm3.lanes[0]);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xb4)))).to_bits();
                xmm2 = xmm0;
                m.write32(esp.wrapping_add(0x20), xmm2.lanes[0]);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xb8)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm1.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0xbc)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::default();
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x158)))).to_bits();
                m.write32(esp.wrapping_add(0x20), xmm2.lanes[0]);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x15c)))).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm1.lanes[0]);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x160)))).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0xc0)));
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0xc4)));
                m.write32(esp.wrapping_add(0x30), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0xc8)));
                xmm2 = xmm1;
                xmm4 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                esp = esp.wrapping_add(0xc);
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x4765f4)) as f64);
                m.write32(esp.wrapping_add(0x28), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x24), xmm3.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x1e34fc; continue; }
                pc = 0x1e34b3; continue;
            }
            0x1e34b3 => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0xc), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0.lanes[0] = reciprocal_sqrt(f32::from_bits(m.read32(esp.wrapping_add(0xc)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x8)));
                eax = m.read32(0x7b0c7c);
                xmm0 = xmm1;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x20)))).to_bits();
                xmm3 = xmm1;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x28)))).to_bits();
                pc = 0x1e34fc; continue;
            }
            0x1e34fc => {
                xmm2 = Xmm::scalar(m.read32(0x475db4));
                push!(0);
                push!(0);
                push!(0xfffffffdu32);
                push!(0);
                edx = esp.wrapping_add(0x48);
                push!(edx);
                ecx = esp.wrapping_add(0x34);
                push!(ecx);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x2c)))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x3c), xmm3.lanes[0]);
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x40), xmm1.lanes[0]);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x34)))).to_bits();
                push!(0x1);
                edx = esp.wrapping_add(0x48);
                push!(edx);
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x38), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x3c), xmm1.lanes[0]);
                edx = m.read32(eax.wrapping_add(0x14));
                ecx = eax.wrapping_add(0xb4);
                eax = m.read32(edx.wrapping_add(0x34));
                push!(ecx);
                push!(eax);
                call!(0x88c60);
                pc = 0x1e3572; continue;
            }
            0x1e3572 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4c)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                edx = m.read32(0x75bc04);
                ecx = esp.wrapping_add(0x3c);
                m.write32(esp.wrapping_add(0x40), xmm0.lanes[0]);
                eax = m.read32(edx.wrapping_add(0x4f8));
                push!(ecx);
                push!(eax);
                call!(0x10e730);
                pc = 0x1e359d; continue;
            }
            0x1e359d => {
                ecx = m.read32(0x7b0c7c);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x54)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0xb8)))).to_bits();
                esp = esp.wrapping_add(0x30);
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x47862c)) as f64);
                if ((fca > fcb)) { pc = 0x1e35e1; continue; }
                pc = 0x1e35bd; continue;
            }
            0x1e35bd => {
                xmm1 = Xmm::scalar(m.read32(0x475d24));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x1e35e1; continue; }
                pc = 0x1e35ca; continue;
            }
            0x1e35ca => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475ce8)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x1e361f; continue; }
                pc = 0x1e35e1; continue;
            }
            0x1e35e1 => {
                edx = m.read32(0x75bc04);
                eax = m.read32(edx.wrapping_add(0x4f8));
                push!(eax);
                call!(0x10ebb0);
                pc = 0x1e35f3; continue;
            }
            0x1e35f3 => {
                fpu.push((f32::from_bits(m.read32(0x475cd0)) as f64));
                { let a = fpu.top as usize; let b = ((fpu.top + 1) & 7) as usize; fpu.stack.swap(a, b); }
                esp = esp.wrapping_add(0x4);
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if fp_cmp != 1 { pc = 0x001e361f; continue; }
                pc = 0x1e3604; continue;
            }
            0x1e3604 => {
                ecx = m.read32(0x75bc04);
                edx = m.read32(ecx.wrapping_add(0x4f8));
                push!(0);
                push!(edx);
                call!(0x10eb50);
                pc = 0x1e3618; continue;
            }
            0x1e3618 => {
                esp = esp.wrapping_add(0x8);
                esp = esp.wrapping_add(0x74);
                break 'run;
            }
            0x1e361f => {
                eax = m.read32(0x75bc04);
                ecx = m.read32(eax.wrapping_add(0x4f8));
                push!(ecx);
                call!(0x10ebb0);
                pc = 0x1e3630; continue;
            }
            0x1e3630 => {
                fpu.push((m.read32(0x4e8cec) as i32 as f64));
                esp = esp.wrapping_add(0x4);
                { let value = round_pc((f32::from_bits(m.read32(0x475c78)) as f64) / fpu.st(0), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) * (m.read32(0x4e7944) as i32 as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if fp_cmp != 1 { pc = 0x001e3696; continue; }
                pc = 0x1e364b; continue;
            }
            0x1e364b => {
                edx = m.read32(0x75bc04);
                eax = m.read32(edx.wrapping_add(0x4f8));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x70)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (!(((((fa) & (fb))) as u8).count_ones() % 2 == 0)) { pc = 0x1e3696; continue; }
                pc = 0x1e3669; continue;
            }
            0x1e3669 => {
                ecx = esp.wrapping_add(0x44);
                push!(ecx);
                call!(0x45390);
                pc = 0x1e3673; continue;
            }
            0x1e3673 => {
                xmm0 = Xmm::scalar(m.read32(0x475d04));
                edx = esp.wrapping_add(0x48);
                push!(edx);
                m.write32(esp.wrapping_add(0x4c), 0x1a6);
                m.write32(esp.wrapping_add(0x64), xmm0.lanes[0]);
                call!(0x45820);
                pc = 0x1e3693; continue;
            }
            0x1e3693 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x1e3696; continue;
            }
            0x1e3696 => {
                eax = m.read32(0x75bc04);
                ecx = m.read32(eax.wrapping_add(0x4f8));
                push!(ecx);
                call!(0x10ebb0);
                pc = 0x1e36a7; continue;
            }
            0x1e36a7 => {
                fpu.push((f32::from_bits(m.read32(0x475cdc)) as f64));
                esp = esp.wrapping_add(0x4);
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if fp_cmp != 1 { pc = 0x001e36fe; continue; }
                pc = 0x1e36b6; continue;
            }
            0x1e36b6 => {
                edx = m.read32(0x75bc04);
                eax = m.read32(edx.wrapping_add(0x4f8));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x70)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (!(((((fa) & (fb))) as u8).count_ones() % 2 == 0)) { pc = 0x1e36fe; continue; }
                pc = 0x1e36d4; continue;
            }
            0x1e36d4 => {
                push!(0x3dcccccd);
                push!(0x3dcccccd);
                push!(0x3dcccccd);
                call!(0xd1f10);
                pc = 0x1e36e8; continue;
            }
            0x1e36e8 => {
                esp = esp.wrapping_add(0xc);
                esp = esp.wrapping_add(0x74);
                break 'run;
            }
            0x1e36ef => {
                ecx = m.read32(eax.wrapping_add(0x4f8));
                push!(ecx);
                call!(0x10e6f0);
                pc = 0x1e36fb; continue;
            }
            0x1e36fb => {
                esp = esp.wrapping_add(0x4);
                pc = 0x1e36fe; continue;
            }
            0x1e36fe => {
                esp = esp.wrapping_add(0x74);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x001E4030: Tier 0, lifted-source inspection; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1e4030<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1e4030;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1e4030, previous, pc); previous = pc; }
        match pc {
            0x1e4030 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(esi.wrapping_add(0x158));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1e40cd; continue; }
                pc = 0x1e4043; continue;
            }
            0x1e4043 => {
                push!(edi);
                push!(0x54414e4b);
                edi = eax;
                call!(0x23ff20);
                pc = 0x1e4050; continue;
            }
            0x1e4050 => {
                esp = esp.wrapping_add(0x4);
                fa = edi;
                fb = eax;
                if (fa != fb) { pc = 0x1e4064; continue; }
                pc = 0x1e4057; continue;
            }
            0x1e4057 => {
                push!(esi);
                push!(edi);
                call!(0x256040);
                pc = 0x1e405e; continue;
            }
            0x1e405e => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1e4064 => {
                edi = m.read32(esi.wrapping_add(0x158));
                push!(0x434d4452);
                call!(0x23ff20);
                pc = 0x1e4074; continue;
            }
            0x1e4074 => {
                esp = esp.wrapping_add(0x4);
                fa = edi;
                fb = eax;
                if (fa != fb) { pc = 0x1e40cc; continue; }
                pc = 0x1e407b; continue;
            }
            0x1e407b => {
                eax = m.read32(0x75b638);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1e40cc; continue; }
                pc = 0x1e4084; continue;
            }
            0x1e4084 => {
                eax = m.read32(edi.wrapping_add(0x4));
                push!(0x1);
                push!(0);
                push!(0);
                push!(0);
                ecx = esi.wrapping_add(0x180);
                push!(ecx);
                edx = esi.wrapping_add(0x1c);
                push!(edx);
                push!(0);
                push!(0x11);
                m.write32(eax.wrapping_add(0x30), 0x2);
                call!(0xefbd0);
                pc = 0x1e40aa; continue;
            }
            0x1e40aa => {
                eax = m.read32(esi.wrapping_add(0x158));
                push!(eax);
                call!(0x1129b0);
                pc = 0x1e40b6; continue;
            }
            0x1e40b6 => {
                push!(0);
                push!(0x8);
                m.write32(0x75b638, 0x1);
                call!(0x2274c0);
                pc = 0x1e40c9; continue;
            }
            0x1e40c9 => {
                esp = esp.wrapping_add(0x2c);
                pc = 0x1e40cc; continue;
            }
            0x1e40cc => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x1e40cd; continue;
            }
            0x1e40cd => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x001F9330: Tier 0, lifted-source inspection; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1f9330<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1f9330;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1f9330, previous, pc); previous = pc; }
        match pc {
            0x1f9330 => {
                esp = esp.wrapping_sub(0x2c);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x34));
                eax = m.read32(esi.wrapping_add(0x24));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(edi);
                edi = m.read32(esi);
                if (fa != fb) { pc = 0x1f9536; continue; }
                pc = 0x1f9346; continue;
            }
            0x1f9346 => {
                eax = m.read32(esi.wrapping_add(0x28));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1f9369; continue; }
                pc = 0x1f934d; continue;
            }
            0x1f934d => {
                xmm0 = Xmm::scalar(m.read32(eax));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x20), xmm3.lanes[0]);
                xmm4 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                pc = 0x1f9384; continue;
            }
            0x1f9369 => {
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x64)));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm3 = Xmm::scalar(m.read32(edi.wrapping_add(0x68)));
                m.write32(esp.wrapping_add(0x20), xmm3.lanes[0]);
                xmm4 = Xmm::scalar(m.read32(edi.wrapping_add(0x6c)));
                pc = 0x1f9384; continue;
            }
            0x1f9384 => {
                eax = m.read32(esi.wrapping_add(0x2c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x24), xmm4.lanes[0]);
                if (fa == fb) { pc = 0x1f93c2; continue; }
                pc = 0x1f9391; continue;
            }
            0x1f9391 => {
                xmm0 = Xmm::default();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                m.write32(esp.wrapping_add(0x18), xmm1.lanes[0]);
                pc = 0x1f948e; continue;
            }
            0x1f93c2 => {
                eax = m.read32(esi.wrapping_add(0x30));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1f93e8; continue; }
                pc = 0x1f93c9; continue;
            }
            0x1f93c9 => {
                xmm0 = Xmm::scalar(m.read32(eax));
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                pc = 0x1f9485; continue;
            }
            0x1f93e8 => {
                xmm2 = Xmm::scalar(m.read32(edi.wrapping_add(0x68)));
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(0x64)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(edi.wrapping_add(0x6c)));
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0 = xmm3;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm4 = xmm2;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = xmm1;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x4765f4)) as f64);
                m.write32(esp.wrapping_add(0x10), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x14), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x18), xmm3.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x1f948b; continue; }
                pc = 0x1f943b; continue;
            }
            0x1f943b => {
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                xmm0.lanes[0] = reciprocal_sqrt(f32::from_bits(m.read32(esp.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x38)))).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xmm4 = xmm0;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm4.lanes[0]);
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                pc = 0x1f9485; continue;
            }
            0x1f9485 => {
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                pc = 0x1f948b; continue;
            }
            0x1f948b => {
                xmm0 = Xmm::default();
                pc = 0x1f948e; continue;
            }
            0x1f948e => {
                xmm1 = Xmm::scalar(m.read32(edi.wrapping_add(0x1cc)));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x1f952d; continue; }
                pc = 0x1f949f; continue;
            }
            0x1f949f => {
                eax = m.read32(esi.wrapping_add(0x24));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1f952d; continue; }
                pc = 0x1f94aa; continue;
            }
            0x1f94aa => {
                eax = m.read32(esi.wrapping_add(0x8));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1f952d; continue; }
                pc = 0x1f94b1; continue;
            }
            0x1f94b1 => {
                fa = m.read32(eax.wrapping_add(0x20));
                fb = ((((0x400000i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1f952d; continue; }
                pc = 0x1f94ba; continue;
            }
            0x1f94ba => {
                eax = m.read32(eax.wrapping_add(0x80));
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                edx = esp.wrapping_add(0x20);
                push!(edx);
                push!(eax);
                call!(0x166060);
                pc = 0x1f94d0; continue;
            }
            0x1f94d0 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x20)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x1c)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x44), xmm0.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x44));
                ecx = (ecx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x44), ecx);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
                esp = esp.wrapping_add(0xc);
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x47d5dc)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x1f952d; continue; }
                pc = 0x1f9522; continue;
            }
            0x1f9522 => {
                xmm0 = Xmm::default();
                m.write32(edi.wrapping_add(0x1cc), xmm0.lanes[0]);
                pc = 0x1f952d; continue;
            }
            0x1f952d => {
                push!(esi);
                call!(0x172170);
                pc = 0x1f9533; continue;
            }
            0x1f9533 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x1f9536; continue;
            }
            0x1f9536 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x2c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x001F9800: Tier 0, lifted-source inspection; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f1f9800<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1f9800;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1f9800, previous, pc); previous = pc; }
        match pc {
            0x1f9800 => {
                esp = esp.wrapping_sub(0x70);
                push!(ebx);
                push!(edi);
                push!(0x1f9540);
                push!(0x4);
                push!(esi);
                call!(0xa5ce0);
                pc = 0x1f9812; continue;
            }
            0x1f9812 => {
                push!(0x1f96a0);
                push!(0);
                push!(esi);
                call!(0xa5ce0);
                pc = 0x1f981f; continue;
            }
            0x1f981f => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0xc0)));
                edi = m.read32(esi.wrapping_add(0x28));
                eax = esp.wrapping_add(0x38);
                push!(eax);
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x70)));
                ebx = esi.wrapping_add(0x64);
                m.write32(esp.wrapping_add(0x40), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0xc8)));
                push!(ebx);
                edi = (edi | 0x4);
                push!(0x463);
                m.write32(esi.wrapping_add(0x28), edi);
                m.write32(esp.wrapping_add(0x4c), xmm0.lanes[0]);
                call!(0xa6880);
                pc = 0x1f9862; continue;
            }
            0x1f9862 => {
                edi = eax;
                push!(0x1f9330);
                push!(edi);
                call!(0x9e720);
                pc = 0x1f986f; continue;
            }
            0x1f986f => {
                edx = m.read32(edi.wrapping_add(0x28));
                xmm0 = Xmm::scalar(m.read32(0x47862c));
                ecx = m.read32(edi.wrapping_add(0x4));
                edx = (edx | 0x70);
                push!(0x1);
                m.write32(edi.wrapping_add(0x1cc), xmm0.lanes[0]);
                m.write32(edi.wrapping_add(0x1d8), xmm0.lanes[0]);
                m.write32(edi.wrapping_add(0x28), edx);
                push!(edi);
                m.write32(ecx.wrapping_add(0x30), 0x1);
                call!(0x16c170);
                pc = 0x1f98a2; continue;
            }
            0x1f98a2 => {
                push!(0x600f40);
                push!(0xffffffffu32);
                push!(0x9);
                push!(edi);
                push!(eax);
                m.write32(edi.wrapping_add(0x80), eax);
                m.write32(edi.wrapping_add(0x1a0), 0);
                call!(0x172760);
                pc = 0x1f98c2; continue;
            }
            0x1f98c2 => {
                edx = m.read32(edi.wrapping_add(0x80));
                esp = esp.wrapping_add(0x48);
                push!(0xe);
                push!(edx);
                call!(0x168160);
                pc = 0x1f98d3; continue;
            }
            0x1f98d3 => {
                push!(0);
                push!(0x1);
                eax = esp.wrapping_add(0x48);
                push!(esi);
                push!(eax);
                call!(0xa14b0);
                pc = 0x1f98e2; continue;
            }
            0x1f98e2 => {
                ecx = m.read32(esi.wrapping_add(0x4));
                eax = m.read32(ecx.wrapping_add(0x4));
                edx = m.read32(eax);
                edx = ((edx as i32).wrapping_mul((0xc8) as i32)) as u32;
                eax = eax.wrapping_sub(edx);
                eax = m.read32(eax.wrapping_add(0x78));
                xmm0 = Xmm::scalar(m.read32(eax));
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                eax = m.read32(ecx.wrapping_add(0x4));
                edx = m.read32(eax);
                edx = ((edx as i32).wrapping_mul((0xc8) as i32)) as u32;
                eax = eax.wrapping_sub(edx);
                eax = m.read32(eax.wrapping_add(0x78));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                eax = m.read32(ecx.wrapping_add(0x4));
                ecx = m.read32(eax);
                ecx = ((ecx as i32).wrapping_mul((0xc8) as i32)) as u32;
                eax = eax.wrapping_sub(ecx);
                edx = m.read32(eax.wrapping_add(0x78));
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(0x8)));
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                eax = esp.wrapping_add(0x2c);
                push!(eax);
                ecx = esp.wrapping_add(0x54);
                push!(ecx);
                call!(0x151e80);
                pc = 0x1f9944; continue;
            }
            0x1f9944 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x58)));
                m.write32(esp.wrapping_add(0x4c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x5c)));
                edx = esp.wrapping_add(0x4c);
                push!(edx);
                m.write32(esp.wrapping_add(0x54), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x64)));
                eax = esp.wrapping_add(0x38);
                push!(eax);
                m.write32(esp.wrapping_add(0x5c), xmm0.lanes[0]);
                ecx = m.read32(edi.wrapping_add(0x80));
                push!(0);
                push!(ecx);
                call!(0x16b380);
                pc = 0x1f9980; continue;
            }
            0x1f9980 => {
                push!(0);
                edi = eax;
                push!(0xbf860a92u32);
                push!(edi);
                call!(0x16b870);
                pc = 0x1f998f; continue;
            }
            0x1f998f => {
                push!(0x3e800000);
                push!(edi);
                call!(0x16b7e0);
                pc = 0x1f999a; continue;
            }
            0x1f999a => {
                esp = esp.wrapping_add(0x44);
                push!(0x3f19999a);
                push!(edi);
                call!(0x16b830);
                pc = 0x1f99a8; continue;
            }
            0x1f99a8 => {
                xmm0 = Xmm::scalar(m.read32(ebx));
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x10));
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x68)));
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                eax = m.read32(esp.wrapping_add(0x10));
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x6c)));
                eax = (eax & 0x7fffffff);
                edx = (edx & 0x7fffffff);
                m.write32(esp.wrapping_add(0x18), eax);
                m.write32(esp.wrapping_add(0x14), edx);
                edx = (((((f32::from_bits(m.read32(esp.wrapping_add(0x14))) as f64)) as i32)) as u32);
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x10));
                ecx = (ecx & 0x7fffffff);
                eax = ((((edx) as u8)) as u8 as i8 as i32 as u32);
                m.write32(esp.wrapping_add(0x10), ecx);
                ecx = (((((f32::from_bits(m.read32(esp.wrapping_add(0x18))) as f64)) as i32)) as u32);
                eax = (eax | 0x4200);
                edx = ((((ecx) as u8)) as u8 as i8 as i32 as u32);
                ecx = (((((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64)) as i32)) as u32);
                eax = eax.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax | edx);
                edx = ((((ecx) as u8)) as u8 as i8 as i32 as u32);
                eax = eax.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax | edx);
                push!(eax);
                push!(edi);
                m.write32(esi.wrapping_add(0x1a0), eax);
                call!(0x16b850);
                pc = 0x1f9a28; continue;
            }
            0x1f9a28 => {
                esp = esp.wrapping_add(0x10);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x70);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0027E940: Tier 0, lifted-source inspection; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27e940<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27e940;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x27e940, previous, pc); previous = pc; }
        match pc {
            0x27e940 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(esp.wrapping_add(0x4));
                push!(0);
                push!(eax);
                push!(ecx);
                call!(0x27e6c0);
                pc = 0x27e951; continue;
            }
            0x27e951 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0027E960: Tier 0, lifted-source inspection; opt-in only.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f27e960<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x27e960;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x27e960, previous, pc); previous = pc; }
        match pc {
            0x27e960 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(esp.wrapping_add(0x4));
                push!(0x1);
                push!(eax);
                push!(ecx);
                call!(0x27e6c0);
                pc = 0x27e971; continue;
            }
            0x27e971 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// Explicit per-function A/B boundary: disabled or unknown roots return false without mutation.
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, enabled: bool, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    if !enabled { return false; }
    match va {
        0x21860 => f21860(m, guest, regs),
        0x168160 => f168160(m, guest, regs),
        0x1e1dc0 => f1e1dc0(m, guest, regs),
        0x1e1de0 => f1e1de0(m, guest, regs),
        0x1e1e00 => f1e1e00(m, guest, regs),
        0x1e2890 => f1e2890(m, guest, regs),
        0x1e2ad0 => f1e2ad0(m, guest, regs),
        0x1e2b80 => f1e2b80(m, guest, regs),
        0x1e4030 => f1e4030(m, guest, regs),
        0x1f9330 => f1f9330(m, guest, regs),
        0x1f9800 => f1f9800(m, guest, regs),
        0x27e940 => f27e940(m, guest, regs),
        0x27e960 => f27e960(m, guest, regs),
        _ => return false,
    }
    true
}
/// INFERRED readable name from function_names.csv.
pub use f21860 as call_45820_with_initialized_id_word4_scalar18_packet;
/// INFERRED readable name from function_names.csv.
pub use f168160 as entry_74c458_set_field_0x1c4;
/// INFERRED readable name from function_names.csv.
pub use f1e1dc0 as scene_plne_group_call_27e960_with_flag_1;
/// INFERRED readable name from function_names.csv.
pub use f1e1de0 as scene_plns_group_call_27e940_with_flag_0;
/// INFERRED readable name from function_names.csv.
pub use f1e1e00 as scene_copy_fields_0xdc_0x108_to_driv_and_pass_objects_then_set_sound_frequency;
/// INFERRED readable name from function_names.csv.
pub use f1e2890 as scene_find_nearest_plne_object_state_0x270_to_matrix_point_with_ray_clear_check;
/// INFERRED readable name from function_names.csv.
pub use f1e2ad0 as jump_table_by_current_player_7b0c7c_field_0xf54_minus1_0_to_0x5b;
/// INFERRED readable name from function_names.csv.
pub use f1e2b80 as scene_plne_object_per_frame_update_with_player_state_jump_table_0_to_0x5b_forced_kill_and_effects;
/// INFERRED readable name from function_names.csv.
pub use f1e4030 as scene_owner_hit_dispatch_tank_health_or_cmdr_death_effect_once;
/// INFERRED readable name from function_names.csv.
pub use f1f9330 as scene_attached_object_update_direction_and_zero_health_when_point_speed_exceeds_3_5;
/// INFERRED readable name from function_names.csv.
pub use f1f9800 as scene_object_spawn_model_0x463_with_rigid_entry_and_74c464_slot_at_matrix_point;
/// INFERRED readable name from function_names.csv.
pub use f27e940 as call_27e6c0_with_flag_0_27e940;
/// INFERRED readable name from function_names.csv.
pub use f27e960 as call_27e6c0_with_flag_1_27e960;
