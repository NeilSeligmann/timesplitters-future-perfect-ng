// SPDX-License-Identifier: GPL-3.0-or-later
//! T1906 mechanical data/assets bodies; regenerate with assets/generate.py.
//! Names INFERRED; opt in per function; see docs/t-system-assets.md for tiers and debt.
#![allow(clippy::all, unused_parens, unused_assignments, unused_imports, unused_variables)]
use crate::{abi::{ExtRegisters, GuestCall, Xmm}, memory::GuestMemory, pose::write16};
use crate::inventory::sse;
use super::AssetGuest;
/// 0x175D0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn load_mode_resources_slot_fill<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x175d0;
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
        { crate::pose::cover(0x175d0, previous, pc); previous = pc; }
        match pc {
            0x175d0 => {
                eax = (eax | 0xffffffffu32);
                m.write32(0x54e568, eax);
                m.write32(0x54e578, eax);
                m.write32(0x54e588, eax);
                m.write32(0x54e598, eax);
                m.write32(0x54e5a8, eax);
                m.write32(0x54e5b8, eax);
                m.write32(0x54e5c8, eax);
                m.write32(0x54e5d8, eax);
                tail_to!(0x12c370); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x18200. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn blank_texture_load<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x18200;
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
        { crate::pose::cover(0x18200, previous, pc); previous = pc; }
        match pc {
            0x18200 => {
                push!(0);
                push!(0x475d68);
                call!(0x63b40);
                pc = 0x1820c; continue;
            }
            0x1820c => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x25020. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_read_checked<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x25020;
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
        { crate::pose::cover(0x25020, previous, pc); previous = pc; }
        match pc {
            0x25020 => {
                push!(ecx);
                eax = m.read32(esi.wrapping_add(0xc));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2502f; continue; }
                pc = 0x25028; continue;
            }
            0x25028 => {
                eax = 0x1;
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2502f => {
                edx = m.read32(esi.wrapping_add(0x4));
                push!(0);
                ecx = esp.wrapping_add(0x4);
                push!(ecx);
                push!(edx);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x18));
                push!(eax);
                call!(0x37cc08);
                pc = 0x25045; continue;
            }
            0x25045 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2505a; continue; }
                pc = 0x25049; continue;
            }
            0x25049 => {
                push!(0x8);
                call!(0x37e9a7);
                pc = 0x25050; continue;
            }
            0x25050 => {
                call!(0x24900);
                pc = 0x25055; continue;
            }
            0x25055 => {
                esp = esp.wrapping_add(0x4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2505a => {
                ecx = m.read32(esp);
                edx = m.read32(esi.wrapping_add(0x4));
                eax = 0;
                fa = ecx;
                fb = edx;
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                eax = eax.wrapping_sub(1);
                eax = (eax & 0x6);
                eax = eax.wrapping_add(1);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x25070. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_write_checked<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x25070;
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
        { crate::pose::cover(0x25070, previous, pc); previous = pc; }
        match pc {
            0x25070 => {
                push!(ecx);
                eax = m.read32(esi.wrapping_add(0xc));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2507f; continue; }
                pc = 0x25078; continue;
            }
            0x25078 => {
                eax = 0x1;
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2507f => {
                edx = m.read32(esi.wrapping_add(0x4));
                push!(0);
                ecx = esp.wrapping_add(0x4);
                push!(ecx);
                push!(edx);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x18));
                push!(eax);
                call!(0x37ccf5);
                pc = 0x25095; continue;
            }
            0x25095 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x250aa; continue; }
                pc = 0x25099; continue;
            }
            0x25099 => {
                push!(0x3);
                call!(0x37e9a7);
                pc = 0x250a0; continue;
            }
            0x250a0 => {
                call!(0x24900);
                pc = 0x250a5; continue;
            }
            0x250a5 => {
                esp = esp.wrapping_add(0x4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x250aa => {
                ecx = m.read32(esp);
                edx = m.read32(esi.wrapping_add(0x4));
                eax = 0;
                fa = ecx;
                fb = edx;
                eax = (eax & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                eax = eax.wrapping_add(eax).wrapping_add(0x1);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x28AF0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_handle_slot_find_free<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28af0;
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
        { crate::pose::cover(0x28af0, previous, pc); previous = pc; }
        match pc {
            0x28af0 => {
                edx = m.read32(0x4b9138);
                eax = 0;
                ecx = (ecx | 0xffffffffu32);
                fa = edx;
                fb = eax;
                if (fa == fb) { pc = 0x28c00; continue; }
                pc = 0x28b03; continue;
            }
            0x28b03 => {
                edx = ecx;
                m.write32(0x6612d0, edx);
                m.write32(0x6612d4, edx);
                m.write32(0x6612d8, edx);
                m.write32(0x6612dc, edx);
                m.write32(0x6612e0, edx);
                m.write32(0x6612e4, edx);
                m.write32(0x6612e8, edx);
                m.write32(0x6612ec, edx);
                m.write32(0x6612f0, edx);
                m.write32(0x6612f4, edx);
                m.write32(0x6612f8, edx);
                m.write32(0x6612fc, edx);
                m.write32(0x661300, edx);
                m.write32(0x661304, edx);
                m.write32(0x661308, edx);
                m.write32(0x66130c, edx);
                m.write32(0x661310, edx);
                m.write32(0x661314, edx);
                m.write32(0x661318, edx);
                m.write32(0x66131c, edx);
                m.write32(0x661320, edx);
                m.write32(0x661324, edx);
                m.write32(0x661328, edx);
                m.write32(0x66132c, edx);
                m.write32(0x661330, edx);
                m.write32(0x661334, edx);
                m.write32(0x661338, edx);
                m.write32(0x66133c, edx);
                m.write32(0x661340, edx);
                m.write32(0x661344, edx);
                m.write32(0x661348, edx);
                m.write32(0x66134c, edx);
                m.write32(0x661350, edx);
                m.write32(0x661354, edx);
                m.write32(0x661358, edx);
                m.write32(0x66135c, edx);
                m.write32(0x661360, edx);
                m.write32(0x661364, edx);
                m.write32(0x661368, edx);
                m.write32(0x66136c, edx);
                m.write32(0x4b9138, eax);
                pc = 0x28c00; continue;
            }
            0x28c00 => {
                fa = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x6612d0));
                fb = ecx;
                if (fa == fb) { pc = 0x28c11; continue; }
                pc = 0x28c09; continue;
            }
            0x28c09 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x28i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x28c00; continue; }
                pc = 0x28c0f; continue;
            }
            0x28c0f => {
                eax = ecx;
                pc = 0x28c11; continue;
            }
            0x28c11 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x28C20. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_path_to_dvd_backslash<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28c20;
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
        { crate::pose::cover(0x28c20, previous, pc); previous = pc; }
        match pc {
            0x28c20 => {
                push!(esi);
                push!(edi);
                esi = eax;
                push!(0x476264);
                push!(esi);
                call!(0x3c8690);
                pc = 0x28c2f; continue;
            }
            0x28c2f => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28c39; continue; }
                pc = 0x28c36; continue;
            }
            0x28c36 => {
                eax = eax.wrapping_add(1);
                pc = 0x28c3b; continue;
            }
            0x28c39 => {
                eax = esi;
                pc = 0x28c3b; continue;
            }
            0x28c3b => {
                m.write32(0x6648e0, 0x5c3a64);
                edx = eax;
                pc = 0x28c47; continue;
            }
            0x28c47 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                eax = eax.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x28c47; continue; }
                pc = 0x28c4e; continue;
            }
            0x28c4e => {
                edi = 0x6648e0;
                eax = eax.wrapping_sub(edx);
                edi = edi.wrapping_sub(1);
                pc = 0x28c56; continue;
            }
            0x28c56 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x1))) as u32)) & 0xff);
                edi = edi.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x28c56; continue; }
                pc = 0x28c5e; continue;
            }
            0x28c5e => {
                ecx = eax;
                ecx = ecx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = edx;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                ecx = eax;
                ecx = (ecx & 0x3);
                { let step = if entry.eflags & 0x400 != 0 { u32::MAX } else { 1 }; for i in 0..ecx { m.write8(edi.wrapping_add(i.wrapping_mul(step)), m.read8(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }
                ecx = 0;
                eax = 0x6648e0;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                edx = eax.wrapping_add(0x1);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x28c78; continue;
            }
            0x28c78 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                eax = eax.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x28c78; continue; }
                pc = 0x28c7f; continue;
            }
            0x28c7f => {
                eax = eax.wrapping_sub(edx);
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x28ca4; continue; }
                pc = 0x28c87; continue;
            }
            0x28c87 => {
                edx = (edx & 0xffff_ff00) | ((0x5c) & 0xff);
                pc = 0x28c90; continue;
            }
            0x28c90 => {
                fa = ((((m.read8(ecx.wrapping_add(0x6648e0)))) as u32) & 0xffu32);
                fb = ((((0x2fi32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x28c9f; continue; }
                pc = 0x28c99; continue;
            }
            0x28c99 => {
                m.write8(ecx.wrapping_add(0x6648e0), (((((edx) as u8)) as u32)) as u8);
                pc = 0x28c9f; continue;
            }
            0x28c9f => {
                ecx = ecx.wrapping_add(1);
                fa = ecx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x28c90; continue; }
                pc = 0x28ca4; continue;
            }
            0x28ca4 => {
                eax = 0x6648e0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x28CB0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_path_skip_to_substring_prefix_d_drive_and_normalize_slashes<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28cb0;
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
        { crate::pose::cover(0x28cb0, previous, pc); previous = pc; }
        match pc {
            0x28cb0 => {
                push!(esi);
                push!(edi);
                esi = eax;
                push!(0x476264);
                push!(esi);
                call!(0x3c8690);
                pc = 0x28cbf; continue;
            }
            0x28cbf => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28cc9; continue; }
                pc = 0x28cc6; continue;
            }
            0x28cc6 => {
                eax = eax.wrapping_add(1);
                pc = 0x28ccb; continue;
            }
            0x28cc9 => {
                eax = esi;
                pc = 0x28ccb; continue;
            }
            0x28ccb => {
                m.write32(0x664960, 0x5c3a64);
                edx = eax;
                pc = 0x28cd7; continue;
            }
            0x28cd7 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                eax = eax.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x28cd7; continue; }
                pc = 0x28cde; continue;
            }
            0x28cde => {
                edi = 0x664960;
                eax = eax.wrapping_sub(edx);
                edi = edi.wrapping_sub(1);
                pc = 0x28ce6; continue;
            }
            0x28ce6 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x1))) as u32)) & 0xff);
                edi = edi.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x28ce6; continue; }
                pc = 0x28cee; continue;
            }
            0x28cee => {
                ecx = eax;
                ecx = ecx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = edx;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                ecx = eax;
                ecx = (ecx & 0x3);
                { let step = if entry.eflags & 0x400 != 0 { u32::MAX } else { 1 }; for i in 0..ecx { m.write8(edi.wrapping_add(i.wrapping_mul(step)), m.read8(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }
                ecx = 0;
                eax = 0x664960;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                edx = eax.wrapping_add(0x1);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x28d08; continue;
            }
            0x28d08 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                eax = eax.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x28d08; continue; }
                pc = 0x28d0f; continue;
            }
            0x28d0f => {
                eax = eax.wrapping_sub(edx);
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x28d34; continue; }
                pc = 0x28d17; continue;
            }
            0x28d17 => {
                edx = (edx & 0xffff_ff00) | ((0x5c) & 0xff);
                pc = 0x28d20; continue;
            }
            0x28d20 => {
                fa = ((((m.read8(ecx.wrapping_add(0x664960)))) as u32) & 0xffu32);
                fb = ((((0x2fi32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x28d2f; continue; }
                pc = 0x28d29; continue;
            }
            0x28d29 => {
                m.write8(ecx.wrapping_add(0x664960), (((((edx) as u8)) as u32)) as u8);
                pc = 0x28d2f; continue;
            }
            0x28d2f => {
                ecx = ecx.wrapping_add(1);
                fa = ecx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x28d20; continue; }
                pc = 0x28d34; continue;
            }
            0x28d34 => {
                eax = 0x664960;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x28D50. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_async_read_service<M: GuestMemory, C: AssetGuest>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x28d50;
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
        { crate::pose::cover(0x28d50, previous, pc); previous = pc; }
        match pc {
            0x28d50 => {
                esp = esp.wrapping_sub(0x24);
                push!(ebp);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x30));
                ebp = 0;
                fa = edi;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2901e; continue; }
                pc = 0x28d63; continue;
            }
            0x28d63 => {
                fa = edi;
                fb = ((((0x28i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x2901e; continue; }
                pc = 0x28d6c; continue;
            }
            0x28d6c => {
                fa = m.read32(edi.wrapping_mul(0x4).wrapping_add(0x6612d0));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2901e; continue; }
                pc = 0x28d7a; continue;
            }
            0x28d7a => {
                eax = m.read32(0x4b913c);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x3c));
                push!(esi);
                m.write32(esp.wrapping_add(0x14), ebp);
                if (fa == fb) { pc = 0x28d94; continue; }
                pc = 0x28d8e; continue;
            }
            0x28d8e => {
                push!(eax);
                call!(0x37fcbc);
                pc = 0x28d94; continue;
            }
            0x28d94 => {
                eax = edi.wrapping_mul(0x4).wrapping_add(0x65fe30);
                m.write32(esp.wrapping_add(0x1c), eax);
                eax = m.read32(eax);
                ecx = eax;
                ecx = (ecx & 0xfffff800u32);
                m.write32(esp.wrapping_add(0x28), ecx);
                m.write32(esp.wrapping_add(0x2c), ebp);
                m.write32(esp.wrapping_add(0x30), ebp);
                guest.writeback_invalidate(0x00028DB5);
                eax = (eax & 0x7ff);
                fa = eax;
                esi = eax;
                if ((fa == 0)) { pc = 0x28ea8; continue; }
                pc = 0x28dc4; continue;
            }
            0x28dc4 => {
                fa = m.read32(0x4b9140);
                fb = edi;
                if (fa != fb) { pc = 0x28de3; continue; }
                pc = 0x28dcc; continue;
            }
            0x28dcc => {
                edx = m.read32(esp.wrapping_add(0x28));
                fa = m.read32(0x4b9144);
                fb = edx;
                if (fa != fb) { pc = 0x28de3; continue; }
                pc = 0x28dd8; continue;
            }
            0x28dd8 => {
                edx = 0x800;
                m.write32(esp.wrapping_add(0x10), edx);
                pc = 0x28e5a; continue;
            }
            0x28de3 => {
                edx = m.read32(edi.wrapping_mul(0x4).wrapping_add(0x6612d0));
                eax = esp.wrapping_add(0x20);
                push!(eax);
                ecx = esp.wrapping_add(0x1c);
                push!(ecx);
                push!(0x800);
                push!(0x664a68);
                push!(edx);
                call!(0x37cc08);
                pc = 0x28e04; continue;
            }
            0x28e04 => {
                fa = eax;
                fb = ebp;
                if (fa != fb) { pc = 0x28e19; continue; }
                pc = 0x28e08; continue;
            }
            0x28e08 => {
                call!(0x37e9a7);
                pc = 0x28e0d; continue;
            }
            0x28e0d => {
                fa = eax;
                fb = ((((0x3e5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28e19; continue; }
                pc = 0x28e14; continue;
            }
            0x28e14 => {
                call!(0x235e0);
                pc = 0x28e19; continue;
            }
            0x28e19 => {
                edx = m.read32(edi.wrapping_mul(0x4).wrapping_add(0x6612d0));
                push!(0x1);
                eax = esp.wrapping_add(0x14);
                push!(eax);
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                push!(edx);
                call!(0x37ea14);
                pc = 0x28e32; continue;
            }
            0x28e32 => {
                fa = eax;
                fb = ebp;
                if (fa != fb) { pc = 0x28e47; continue; }
                pc = 0x28e36; continue;
            }
            0x28e36 => {
                call!(0x37e9a7);
                pc = 0x28e3b; continue;
            }
            0x28e3b => {
                fa = eax;
                fb = ((((0x3e5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28e47; continue; }
                pc = 0x28e42; continue;
            }
            0x28e42 => {
                call!(0x235e0);
                pc = 0x28e47; continue;
            }
            0x28e47 => {
                eax = m.read32(esp.wrapping_add(0x28));
                edx = m.read32(esp.wrapping_add(0x10));
                m.write32(0x4b9140, edi);
                m.write32(0x4b9144, eax);
                pc = 0x28e5a; continue;
            }
            0x28e5a => {
                ebp = m.read32(esp.wrapping_add(0x3c));
                eax = 0x800;
                eax = eax.wrapping_sub(esi);
                fa = eax;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { eax = ebx; }
                ecx = eax;
                ebx = ecx;
                ecx = ecx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = esi.wrapping_add(0x664a68);
                edi = ebp;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                ecx = ebx;
                ebx = m.read32(esp.wrapping_add(0x40));
                ecx = (ecx & 0x3);
                ebp = ebp.wrapping_add(eax);
                { let step = if entry.eflags & 0x400 != 0 { u32::MAX } else { 1 }; for i in 0..ecx { m.write8(edi.wrapping_add(i.wrapping_mul(step)), m.read8(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }
                ecx = 0;
                edi = m.read32(esp.wrapping_add(0x38));
                fa = edx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = eax;
                if (fas <= fbs) { ecx = edx; }
                ebx = ebx.wrapping_sub(eax);
                eax = m.read32(esp.wrapping_add(0x28));
                eax = eax.wrapping_add(0x800);
                m.write32(esp.wrapping_add(0x14), ecx);
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x28eac; continue;
            }
            0x28ea8 => {
                ebp = m.read32(esp.wrapping_add(0x3c));
                pc = 0x28eac; continue;
            }
            0x28eac => {
                esi = ebx;
                esi = (esi & 0xfffff800u32);
                fa = esi;
                if ((fa == 0)) { pc = 0x28f2e; continue; }
                pc = 0x28eb6; continue;
            }
            0x28eb6 => {
                eax = m.read32(edi.wrapping_mul(0x4).wrapping_add(0x6612d0));
                ecx = esp.wrapping_add(0x20);
                push!(ecx);
                edx = esp.wrapping_add(0x1c);
                push!(edx);
                push!(esi);
                push!(ebp);
                push!(eax);
                call!(0x37cc08);
                pc = 0x28ecf; continue;
            }
            0x28ecf => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x28ee4; continue; }
                pc = 0x28ed3; continue;
            }
            0x28ed3 => {
                call!(0x37e9a7);
                pc = 0x28ed8; continue;
            }
            0x28ed8 => {
                fa = eax;
                fb = ((((0x3e5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28ee4; continue; }
                pc = 0x28edf; continue;
            }
            0x28edf => {
                call!(0x235e0);
                pc = 0x28ee4; continue;
            }
            0x28ee4 => {
                eax = m.read32(edi.wrapping_mul(0x4).wrapping_add(0x6612d0));
                push!(0x1);
                ecx = esp.wrapping_add(0x14);
                push!(ecx);
                edx = esp.wrapping_add(0x28);
                push!(edx);
                push!(eax);
                call!(0x37ea14);
                pc = 0x28efd; continue;
            }
            0x28efd => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x28f12; continue; }
                pc = 0x28f01; continue;
            }
            0x28f01 => {
                call!(0x37e9a7);
                pc = 0x28f06; continue;
            }
            0x28f06 => {
                fa = eax;
                fb = ((((0x3e5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28f12; continue; }
                pc = 0x28f0d; continue;
            }
            0x28f0d => {
                call!(0x235e0);
                pc = 0x28f12; continue;
            }
            0x28f12 => {
                edx = m.read32(esp.wrapping_add(0x14));
                ecx = m.read32(esp.wrapping_add(0x10));
                eax = m.read32(esp.wrapping_add(0x28));
                edx = edx.wrapping_add(ecx);
                ebp = ebp.wrapping_add(esi);
                ebx = ebx.wrapping_sub(esi);
                eax = eax.wrapping_add(esi);
                m.write32(esp.wrapping_add(0x14), edx);
                m.write32(esp.wrapping_add(0x28), eax);
                pc = 0x28f2e; continue;
            }
            0x28f2e => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28fed; continue; }
                pc = 0x28f36; continue;
            }
            0x28f36 => {
                fa = m.read32(0x4b9140);
                fb = edi;
                if (fa != fb) { pc = 0x28f50; continue; }
                pc = 0x28f3e; continue;
            }
            0x28f3e => {
                edx = m.read32(esp.wrapping_add(0x28));
                fa = m.read32(0x4b9144);
                fb = edx;
                if (fa != fb) { pc = 0x28f50; continue; }
                pc = 0x28f4a; continue;
            }
            0x28f4a => {
                m.write32(esp.wrapping_add(0x10), ebx);
                pc = 0x28fc3; continue;
            }
            0x28f50 => {
                edx = m.read32(edi.wrapping_mul(0x4).wrapping_add(0x6612d0));
                eax = esp.wrapping_add(0x20);
                push!(eax);
                ecx = esp.wrapping_add(0x1c);
                push!(ecx);
                push!(0x800);
                push!(0x664a68);
                push!(edx);
                call!(0x37cc08);
                pc = 0x28f71; continue;
            }
            0x28f71 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x28f86; continue; }
                pc = 0x28f75; continue;
            }
            0x28f75 => {
                call!(0x37e9a7);
                pc = 0x28f7a; continue;
            }
            0x28f7a => {
                fa = eax;
                fb = ((((0x3e5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28f86; continue; }
                pc = 0x28f81; continue;
            }
            0x28f81 => {
                call!(0x235e0);
                pc = 0x28f86; continue;
            }
            0x28f86 => {
                edx = m.read32(edi.wrapping_mul(0x4).wrapping_add(0x6612d0));
                push!(0x1);
                eax = esp.wrapping_add(0x14);
                push!(eax);
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                push!(edx);
                call!(0x37ea14);
                pc = 0x28f9f; continue;
            }
            0x28f9f => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x28fb4; continue; }
                pc = 0x28fa3; continue;
            }
            0x28fa3 => {
                call!(0x37e9a7);
                pc = 0x28fa8; continue;
            }
            0x28fa8 => {
                fa = eax;
                fb = ((((0x3e5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x28fb4; continue; }
                pc = 0x28faf; continue;
            }
            0x28faf => {
                call!(0x235e0);
                pc = 0x28fb4; continue;
            }
            0x28fb4 => {
                eax = m.read32(esp.wrapping_add(0x28));
                m.write32(0x4b9140, edi);
                m.write32(0x4b9144, eax);
                pc = 0x28fc3; continue;
            }
            0x28fc3 => {
                ecx = ebx;
                edx = ecx;
                ecx = ecx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = 0x664a68;
                edi = ebp;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                ecx = edx;
                ecx = (ecx & 0x3);
                { let step = if entry.eflags & 0x400 != 0 { u32::MAX } else { 1 }; for i in 0..ecx { m.write8(edi.wrapping_add(i.wrapping_mul(step)), m.read8(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }
                ecx = 0;
                eax = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(esp.wrapping_add(0x14));
                fa = eax;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { eax = ebx; }
                ecx = ecx.wrapping_add(eax);
                m.write32(esp.wrapping_add(0x14), ecx);
                pc = 0x28fed; continue;
            }
            0x28fed => {
                esi = m.read32(esp.wrapping_add(0x40));
                fa = m.read32(esp.wrapping_add(0x14));
                fb = esi;
                if (fa == fb) { pc = 0x28ffc; continue; }
                pc = 0x28ff7; continue;
            }
            0x28ff7 => {
                call!(0x235e0);
                pc = 0x28ffc; continue;
            }
            0x28ffc => {
                eax = m.read32(esp.wrapping_add(0x1c));
                m.write32(eax, m.read32(eax).wrapping_add(esi));
                eax = m.read32(0x4b913c);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x29014; continue; }
                pc = 0x2900e; continue;
            }
            0x2900e => {
                push!(eax);
                call!(0x37fce2);
                pc = 0x29014; continue;
            }
            0x29014 => {
                eax = m.read32(esp.wrapping_add(0xc));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x24);
                break 'run;
            }
            0x2901e => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = (eax | 0xffffffffu32);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x24);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x29030. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_seek_slot<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29030;
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
        { crate::pose::cover(0x29030, previous, pc); previous = pc; }
        match pc {
            0x29030 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2907f; continue; }
                pc = 0x29039; continue;
            }
            0x29039 => {
                fa = esi;
                fb = ((((0x28i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x2907f; continue; }
                pc = 0x2903e; continue;
            }
            0x2903e => {
                eax = m.read32(esi.wrapping_mul(0x4).wrapping_add(0x6612d0));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2907f; continue; }
                pc = 0x2904a; continue;
            }
            0x2904a => {
                ecx = m.read32(esp.wrapping_add(0x10));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x29066; continue; }
                pc = 0x29052; continue;
            }
            0x29052 => {
                push!(ecx);
                push!(eax);
                call!(0x37d1f6);
                pc = 0x29059; continue;
            }
            0x29059 => {
                eax = eax.wrapping_add(m.read32(esp.wrapping_add(0xc)));
                m.write32(esi.wrapping_mul(0x4).wrapping_add(0x65fe30), eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x29066 => {
                fa = ecx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x29076; continue; }
                pc = 0x2906b; continue;
            }
            0x2906b => {
                eax = m.read32(esp.wrapping_add(0xc));
                m.write32(esi.wrapping_mul(0x4).wrapping_add(0x65fe30), eax);
                pc = 0x29076; continue;
            }
            0x29076 => {
                eax = m.read32(esi.wrapping_mul(0x4).wrapping_add(0x65fe30));
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2907f => {
                eax = (eax | 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x29090. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_close_slot<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29090;
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
        { crate::pose::cover(0x29090, previous, pc); previous = pc; }
        match pc {
            0x29090 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x290e0; continue; }
                pc = 0x29099; continue;
            }
            0x29099 => {
                fa = esi;
                fb = ((((0x28i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x290e0; continue; }
                pc = 0x2909e; continue;
            }
            0x2909e => {
                eax = m.read32(esi.wrapping_mul(0x4).wrapping_add(0x6612d0));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x290e0; continue; }
                pc = 0x290aa; continue;
            }
            0x290aa => {
                push!(eax);
                call!(0x37cbc4);
                pc = 0x290b0; continue;
            }
            0x290b0 => {
                eax = esi;
                eax = eax.wrapping_shl((((((((0x7i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write8(eax.wrapping_add(0x65fed0), (0) as u8);
                fa = m.read32(0x4b9140);
                fb = esi;
                m.write32(esi.wrapping_mul(0x4).wrapping_add(0x6612d0), 0xffffffffu32);
                eax = 0x1;
                if (fa != fb) { pc = 0x290e5; continue; }
                pc = 0x290d4; continue;
            }
            0x290d4 => {
                m.write32(0x4b9140, 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x290e0 => {
                eax = 0x1;
                pc = 0x290e5; continue;
            }
            0x290e5 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x290F0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_physical_sort_key_get<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x290f0;
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
        { crate::pose::cover(0x290f0, previous, pc); previous = pc; }
        match pc {
            0x290f0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x29129; continue; }
                pc = 0x290f8; continue;
            }
            0x290f8 => {
                fa = eax;
                fb = ((((0x28i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x29129; continue; }
                pc = 0x290fd; continue;
            }
            0x290fd => {
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x6612d0));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29129; continue; }
                pc = 0x29109; continue;
            }
            0x29109 => {
                push!(eax);
                call!(0x37d14f);
                pc = 0x2910f; continue;
            }
            0x2910f => {
                ecx = eax;
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29129; continue; }
                pc = 0x29116; continue;
            }
            0x29116 => {
                eax = m.read32(esp.wrapping_add(0x8));
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                edx = (edx & 0x7ff);
                eax = eax.wrapping_add(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0xbi32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                eax = eax.wrapping_add(ecx);
                break 'run;
            }
            0x29129 => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x29130. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_request_queue_select_first_signed_min_field30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29130;
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
        { crate::pose::cover(0x29130, previous, pc); previous = pc; }
        match pc {
            0x29130 => {
                eax = m.read32(0x6648d0);
                ecx = 0;
                fa = eax;
                fb = ((((0x664890i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29158; continue; }
                pc = 0x2913e; continue;
            }
            0x2913e => {
                edi = edi;
                pc = 0x29140; continue;
            }
            0x29140 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2914c; continue; }
                pc = 0x29144; continue;
            }
            0x29144 => {
                edx = m.read32(eax.wrapping_add(0x30));
                fa = edx;
                fb = m.read32(ecx.wrapping_add(0x30));
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x2914e; continue; }
                pc = 0x2914c; continue;
            }
            0x2914c => {
                ecx = eax;
                pc = 0x2914e; continue;
            }
            0x2914e => {
                eax = m.read32(eax.wrapping_add(0x40));
                fa = eax;
                fb = ((((0x664890i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x29140; continue; }
                pc = 0x29158; continue;
            }
            0x29158 => {
                m.write32(0x6648d4, ecx);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x29160. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_request_unlink_and_clear_current_if_selected<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29160;
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
        { crate::pose::cover(0x29160, previous, pc); previous = pc; }
        match pc {
            0x29160 => {
                ecx = m.read32(eax.wrapping_add(0x3c));
                edx = 0;
                fa = ecx;
                fb = edx;
                push!(esi);
                if (fa == fb) { pc = 0x29170; continue; }
                pc = 0x2916a; continue;
            }
            0x2916a => {
                esi = m.read32(eax.wrapping_add(0x40));
                m.write32(ecx.wrapping_add(0x40), esi);
                pc = 0x29170; continue;
            }
            0x29170 => {
                ecx = m.read32(eax.wrapping_add(0x40));
                fa = ecx;
                fb = edx;
                if (fa == fb) { pc = 0x2917d; continue; }
                pc = 0x29177; continue;
            }
            0x29177 => {
                esi = m.read32(eax.wrapping_add(0x3c));
                m.write32(ecx.wrapping_add(0x3c), esi);
                pc = 0x2917d; continue;
            }
            0x2917d => {
                fa = m.read32(0x6648d4);
                fb = eax;
                m.write32(eax.wrapping_add(0x40), edx);
                m.write32(eax.wrapping_add(0x3c), edx);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa != fb) { pc = 0x29192; continue; }
                pc = 0x2918c; continue;
            }
            0x2918c => {
                m.write32(0x6648d4, edx);
                pc = 0x29192; continue;
            }
            0x29192 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x291A0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_request_table_reset_links_and_free_markers<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x291a0;
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
        { crate::pose::cover(0x291a0, previous, pc); previous = pc; }
        match pc {
            0x291a0 => {
                ecx = 0;
                eax = 0x6613ac;
                edx = (edx | 0xffffffffu32);
                pc = 0x291b0; continue;
            }
            0x291b0 => {
                m.write32(eax.wrapping_add(0x4), ecx);
                m.write32(eax, ecx);
                m.write32(eax.wrapping_add(((0x28i32.wrapping_neg()) as u32)), edx);
                eax = eax.wrapping_add(0x44);
                fa = eax;
                fb = ((((0x6648cci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x291b0; continue; }
                pc = 0x291c2; continue;
            }
            0x291c2 => {
                eax = 0x664890;
                m.write32(0x6648d4, ecx);
                m.write32(0x6648d0, eax);
                m.write32(0x6648cc, eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x291E0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_request_slot_allocate_creating_event_on_first_use<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x291e0;
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
        { crate::pose::cover(0x291e0, previous, pc); previous = pc; }
        match pc {
            0x291e0 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(0x4b9148)) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x29207; continue; }
                pc = 0x291e9; continue;
            }
            0x291e9 => {
                call!(0x291a0);
                pc = 0x291ee; continue;
            }
            0x291ee => {
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                m.write8(0x4b9148, (0) as u8);
                call!(0x37ff30);
                pc = 0x29202; continue;
            }
            0x29202 => {
                m.write32(0x6648dc, eax);
                pc = 0x29207; continue;
            }
            0x29207 => {
                ecx = 0;
                eax = 0x661384;
                edi = edi;
                pc = 0x29210; continue;
            }
            0x29210 => {
                fa = m.read32(eax);
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29224; continue; }
                pc = 0x29215; continue;
            }
            0x29215 => {
                eax = eax.wrapping_add(0x44);
                ecx = ecx.wrapping_add(1);
                fa = eax;
                fb = ((((0x6648a4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x29210; continue; }
                pc = 0x29220; continue;
            }
            0x29220 => {
                eax = (eax | 0xffffffffu32);
                break 'run;
            }
            0x29224 => {
                eax = ecx.wrapping_add(0x10000);
                ecx = ((ecx as i32).wrapping_mul((0x44) as i32)) as u32;
                m.write32(ecx.wrapping_add(0x661394), eax);
                m.write32(0x6648d8, m.read32(0x6648d8).wrapping_add(0x1));
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x29240. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_async_request_create_with_size<M: GuestMemory, C: AssetGuest>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29240;
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
        { crate::pose::cover(0x29240, previous, pc); previous = pc; }
        match pc {
            0x29240 => {
                esp = esp.wrapping_sub(0x34);
                push!(edi);
                call!(0x291e0);
                pc = 0x29249; continue;
            }
            0x29249 => {
                edi = eax;
                fa = edi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x292b3; continue; }
                pc = 0x29250; continue;
            }
            0x29250 => {
                push!(ebx);
                push!(esi);
                esi = edi;
                esi = (esi & 0xffff);
                fa = esi;
                fas = (fa as i32);
                ebx = 0;
                if ((fas < 0i32)) { pc = 0x29274; continue; }
                pc = 0x29261; continue;
            }
            0x29261 => {
                fa = esi;
                fb = ((((0xc8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x29274; continue; }
                pc = 0x29269; continue;
            }
            0x29269 => {
                esi = ((esi as i32).wrapping_mul((0x44) as i32)) as u32;
                esi = esi.wrapping_add(0x661370);
                pc = 0x29276; continue;
            }
            0x29274 => {
                esi = 0;
                pc = 0x29276; continue;
            }
            0x29276 => {
                guest.writeback_invalidate(0x00029276);
                eax = m.read32(esp.wrapping_add(0x44));
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x6612d0));
                ecx = esp.wrapping_add(0xc);
                push!(ecx);
                push!(eax);
                m.write32(esi.wrapping_add(0x14), eax);
                call!(0x37d011);
                pc = 0x29291; continue;
            }
            0x29291 => {
                ecx = m.read32(esp.wrapping_add(0x30));
                edx = m.read32(esp.wrapping_add(0x48));
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & ecx);
                m.write8(esi.wrapping_add(0x20), (((((ebx) as u8)) as u32)) as u8);
                m.write32(esi.wrapping_add(0x2c), ebx);
                m.write32(esi.wrapping_add(0x40), ebx);
                m.write32(esi.wrapping_add(0x3c), ebx);
                m.write32(esi.wrapping_add(0x1c), eax);
                m.write32(esi.wrapping_add(0x18), edx);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x292b3; continue;
            }
            0x292b3 => {
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x34);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x292C0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_info_by_path<M: GuestMemory, C: AssetGuest>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x292c0;
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
        { crate::pose::cover(0x292c0, previous, pc); previous = pc; }
        match pc {
            0x292c0 => {
                esp = esp.wrapping_sub(0x34);
                push!(esi);
                push!(edi);
                call!(0x291e0);
                pc = 0x292ca; continue;
            }
            0x292ca => {
                edi = eax;
                fa = edi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2935f; continue; }
                pc = 0x292d5; continue;
            }
            0x292d5 => {
                esi = edi;
                esi = (esi & 0xffff);
                fa = esi;
                fas = (fa as i32);
                if ((fas < 0i32)) { pc = 0x292f2; continue; }
                pc = 0x292df; continue;
            }
            0x292df => {
                fa = esi;
                fb = ((((0xc8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x292f2; continue; }
                pc = 0x292e7; continue;
            }
            0x292e7 => {
                esi = ((esi as i32).wrapping_mul((0x44) as i32)) as u32;
                esi = esi.wrapping_add(0x661370);
                pc = 0x292f4; continue;
            }
            0x292f2 => {
                esi = 0;
                pc = 0x292f4; continue;
            }
            0x292f4 => {
                guest.writeback_invalidate(0x000292F4);
                eax = m.read32(esp.wrapping_add(0x40));
                push!(0);
                push!(0x60000000);
                push!(0x3);
                push!(0);
                push!(0x1);
                push!(0x80000000u32);
                call!(0x28cb0);
                pc = 0x29311; continue;
            }
            0x29311 => {
                push!(eax);
                call!(0x37d231);
                pc = 0x29317; continue;
            }
            0x29317 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                m.write32(esi.wrapping_add(0x14), eax);
                if (fa == fb) { pc = 0x2935f; continue; }
                pc = 0x2931f; continue;
            }
            0x2931f => {
                ecx = esp.wrapping_add(0x8);
                push!(ecx);
                push!(eax);
                call!(0x37d011);
                pc = 0x2932a; continue;
            }
            0x2932a => {
                ecx = m.read32(esp.wrapping_add(0x2c));
                edx = m.read32(esp.wrapping_add(0x44));
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & ecx);
                m.write32(esi.wrapping_add(0x1c), eax);
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esi.wrapping_add(0x18), edx);
                m.write8(esi.wrapping_add(0x20), (0x1) as u8);
                m.write32(esi.wrapping_add(0x2c), 0);
                m.write32(esi.wrapping_add(0x40), 0);
                m.write32(esi.wrapping_add(0x3c), 0);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x34);
                break 'run;
            }
            0x2935f => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = (eax | 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x34);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x29370. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_async_request_enqueue_read<M: GuestMemory, C: AssetGuest>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29370;
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
        { crate::pose::cover(0x29370, previous, pc); previous = pc; }
        match pc {
            0x29370 => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = (eax & 0xffff);
                fa = eax;
                fas = (fa as i32);
                if ((fas < 0i32)) { pc = 0x29390; continue; }
                pc = 0x2937b; continue;
            }
            0x2937b => {
                fa = eax;
                fb = ((((0xc8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x29390; continue; }
                pc = 0x29382; continue;
            }
            0x29382 => {
                eax = ((eax as i32).wrapping_mul((0x44) as i32)) as u32;
                eax = eax.wrapping_add(0x661370);
                ecx = 0;
                fa = eax;
                fb = ecx;
                if (fa != fb) { pc = 0x29393; continue; }
                pc = 0x29390; continue;
            }
            0x29390 => {
                eax = 0;
                break 'run;
            }
            0x29393 => {
                push!(esi);
                guest.writeback_invalidate(0x00029394);
                esi = m.read32(esp.wrapping_add(0x10));
                edx = m.read32(eax.wrapping_add(0x18));
                edx = edx.wrapping_add(esi);
                esi = m.read32(esp.wrapping_add(0x18));
                m.write32(eax.wrapping_add(0x8), edx);
                edx = m.read32(esp.wrapping_add(0x14));
                edx = edx.wrapping_add(0x7ff);
                edx = (edx & 0xfffff800u32);
                m.write32(eax.wrapping_add(0x2c), edx);
                edx = m.read32(0x7497a8);
                edx = edx.wrapping_sub(esi);
                m.write32(eax.wrapping_add(0x30), edx);
                edx = m.read32(esp.wrapping_add(0xc));
                m.write32(eax.wrapping_add(0x38), edx);
                fa = m.read32(eax.wrapping_add(0x40));
                fb = ecx;
                m.write32(eax.wrapping_add(0xc), ecx);
                m.write32(eax.wrapping_add(0x10), ecx);
                m.write32(eax.wrapping_add(0x34), 0x1);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa != fb) { pc = 0x293f6; continue; }
                pc = 0x293de; continue;
            }
            0x293de => {
                ecx = m.read32(0x6648d0);
                m.write32(eax.wrapping_add(0x40), ecx);
                m.write32(eax.wrapping_add(0x3c), 0x664890);
                m.write32(ecx.wrapping_add(0x3c), eax);
                m.write32(0x6648d0, eax);
                pc = 0x293f6; continue;
            }
            0x293f6 => {
                eax = 0x1;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x29400. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_async_request_release<M: GuestMemory, C: AssetGuest>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29400;
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
        { crate::pose::cover(0x29400, previous, pc); previous = pc; }
        match pc {
            0x29400 => {
                push!(ebx);
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x10));
                edi = (edi & 0xffff);
                fa = edi;
                fas = (fa as i32);
                if ((fas < 0i32)) { pc = 0x29428; continue; }
                pc = 0x2940f; continue;
            }
            0x2940f => {
                fa = edi;
                fb = ((((0xc8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x29428; continue; }
                pc = 0x29417; continue;
            }
            0x29417 => {
                esi = edi;
                esi = ((esi as i32).wrapping_mul((0x44) as i32)) as u32;
                esi = esi.wrapping_add(0x661370);
                ebx = 0;
                fa = esi;
                fb = ebx;
                if (fa != fb) { pc = 0x2942e; continue; }
                pc = 0x29428; continue;
            }
            0x29428 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2942e => {
                guest.writeback_invalidate(0x0002942E);
                fa = ((((m.read8(esi.wrapping_add(0x20)))) as u32) & 0xffu32);
                fb = ((((((ebx) as u8))) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x2943e; continue; }
                pc = 0x29435; continue;
            }
            0x29435 => {
                eax = m.read32(esi.wrapping_add(0x14));
                push!(eax);
                call!(0x37cbc4);
                pc = 0x2943e; continue;
            }
            0x2943e => {
                eax = m.read32(esi.wrapping_add(0x3c));
                fa = eax;
                fb = ebx;
                if (fa == fb) { pc = 0x2944b; continue; }
                pc = 0x29445; continue;
            }
            0x29445 => {
                ecx = m.read32(esi.wrapping_add(0x40));
                m.write32(eax.wrapping_add(0x40), ecx);
                pc = 0x2944b; continue;
            }
            0x2944b => {
                eax = m.read32(esi.wrapping_add(0x40));
                fa = eax;
                fb = ebx;
                if (fa == fb) { pc = 0x29458; continue; }
                pc = 0x29452; continue;
            }
            0x29452 => {
                edx = m.read32(esi.wrapping_add(0x3c));
                m.write32(eax.wrapping_add(0x3c), edx);
                pc = 0x29458; continue;
            }
            0x29458 => {
                fa = m.read32(0x6648d4);
                fb = esi;
                m.write32(esi.wrapping_add(0x40), ebx);
                m.write32(esi.wrapping_add(0x3c), ebx);
                if (fa != fb) { pc = 0x2946c; continue; }
                pc = 0x29466; continue;
            }
            0x29466 => {
                m.write32(0x6648d4, ebx);
                pc = 0x2946c; continue;
            }
            0x2946c => {
                fa = edi;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2949b; continue; }
                pc = 0x29470; continue;
            }
            0x29470 => {
                fa = edi;
                fb = ((((0xc8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x2949b; continue; }
                pc = 0x29478; continue;
            }
            0x29478 => {
                edi = ((edi as i32).wrapping_mul((0x44) as i32)) as u32;
                edi = edi.wrapping_add(0x661370);
                fa = edi;
                fb = ebx;
                if (fa == fb) { pc = 0x2949b; continue; }
                pc = 0x29485; continue;
            }
            0x29485 => {
                eax = m.read32(0x6648d8);
                eax = eax.wrapping_sub(1);
                write16(m, edi.wrapping_add(0x24), (((((ebx) as u16)) as u32)) as u16);
                m.write32(edi.wrapping_add(0x14), 0xffffffffu32);
                m.write32(0x6648d8, eax);
                pc = 0x2949b; continue;
            }
            0x2949b => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x294F0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_async_read_poll_pending_request<M: GuestMemory, C: AssetGuest>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x294f0;
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
        { crate::pose::cover(0x294f0, previous, pc); previous = pc; }
        match pc {
            0x294f0 => {
                push!(ecx);
                eax = (eax & 0xffff_ff00) | ((((m.read8(0x4b9148)) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x29506; continue; }
                pc = 0x294fa; continue;
            }
            0x294fa => {
                call!(0x291a0);
                pc = 0x294ff; continue;
            }
            0x294ff => {
                m.write8(0x4b9148, (0) as u8);
                pc = 0x29506; continue;
            }
            0x29506 => {
                eax = m.read32(0x6648d4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2963c; continue; }
                pc = 0x29513; continue;
            }
            0x29513 => {
                ecx = m.read32(eax.wrapping_add(0x34));
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                push!(esi);
                if ((fa == 0)) { pc = 0x295a2; continue; }
                pc = 0x2951e; continue;
            }
            0x2951e => {
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x2954e; continue; }
                pc = 0x29521; continue;
            }
            0x29521 => {
                edx = m.read32(eax.wrapping_add(0x14));
                push!(0);
                ecx = esp.wrapping_add(0x8);
                push!(ecx);
                push!(eax);
                push!(edx);
                call!(0x37ea14);
                pc = 0x29532; continue;
            }
            0x29532 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x29551; continue; }
                pc = 0x29536; continue;
            }
            0x29536 => {
                call!(0x37e9a7);
                pc = 0x2953b; continue;
            }
            0x2953b => {
                fa = eax;
                fb = ((((0x3e5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2954e; continue; }
                pc = 0x29542; continue;
            }
            0x29542 => {
                fa = eax;
                fb = ((((0x3e4i32)) as u32) & 0xffffffffu32);
                pc = 0x29547; continue;
            }
            0x29547 => {
                if (fa == fb) { pc = 0x2954e; continue; }
                pc = 0x29549; continue;
            }
            0x29549 => {
                call!(0x235e0);
                pc = 0x2954e; continue;
            }
            0x2954e => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x29551 => {
                eax = m.read32(0x6648d4);
                ecx = m.read32(eax.wrapping_add(0x28));
                edx = m.read32(esp.wrapping_add(0x4));
                fa = edx;
                fb = ecx;
                if (fa == fb) { pc = 0x2956b; continue; }
                pc = 0x29561; continue;
            }
            0x29561 => {
                esi = m.read32(eax.wrapping_add(0x8));
                esi = esi.wrapping_add(edx);
                fa = esi;
                fb = m.read32(eax.wrapping_add(0x1c));
                if (fa < fb) { pc = 0x29587; continue; }
                pc = 0x2956b; continue;
            }
            0x2956b => {
                edx = m.read32(0x7497a8);
                esi = m.read32(eax.wrapping_add(0x8));
                m.write32(eax.wrapping_add(0x30), edx);
                m.write32(eax.wrapping_add(0x2c), m.read32(eax.wrapping_add(0x2c)).wrapping_sub(ecx));
                edx = m.read32(eax.wrapping_add(0x38));
                esi = esi.wrapping_add(ecx);
                edx = edx.wrapping_add(ecx);
                m.write32(eax.wrapping_add(0x8), esi);
                m.write32(eax.wrapping_add(0x38), edx);
                pc = 0x29587; continue;
            }
            0x29587 => {
                ecx = m.read32(eax.wrapping_add(0x2c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(eax.wrapping_add(0x34), 0x1);
                if (fa != fb) { pc = 0x2959a; continue; }
                pc = 0x29595; continue;
            }
            0x29595 => {
                call!(0x29160);
                pc = 0x2959a; continue;
            }
            0x2959a => {
                call!(0x29130);
                pc = 0x2959f; continue;
            }
            0x2959f => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x295a2 => {
                guest.writeback_invalidate(0x000295A2);
                esi = m.read32(eax.wrapping_add(0x2c));
                fa = esi;
                fb = ((((0x40000i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x295b4; continue; }
                pc = 0x295af; continue;
            }
            0x295af => {
                esi = 0x40000;
                pc = 0x295b4; continue;
            }
            0x295b4 => {
                ecx = m.read32(0x6648dc);
                push!(eax);
                edx = esp.wrapping_add(0x8);
                push!(edx);
                edx = m.read32(eax.wrapping_add(0x14));
                m.write32(eax.wrapping_add(0x10), ecx);
                ecx = m.read32(eax.wrapping_add(0x38));
                push!(esi);
                push!(ecx);
                push!(edx);
                call!(0x37cc08);
                pc = 0x295d1; continue;
            }
            0x295d1 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = m.read32(0x7497a8);
                edx = m.read32(0x6648d4);
                m.write32(edx.wrapping_add(0x30), ecx);
                if (fa != fb) { pc = 0x2959a; continue; }
                pc = 0x295e4; continue;
            }
            0x295e4 => {
                call!(0x37e9a7);
                pc = 0x295e9; continue;
            }
            0x295e9 => {
                fa = eax;
                fb = ((((0x3e5i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x29632; continue; }
                pc = 0x295f0; continue;
            }
            0x295f0 => {
                if (fa == fb) { pc = 0x29620; continue; }
                pc = 0x295f2; continue;
            }
            0x295f2 => {
                fa = eax;
                fb = ((((0x8i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2954e; continue; }
                pc = 0x295fb; continue;
            }
            0x295fb => {
                fa = eax;
                fb = ((((0x26i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x29549; continue; }
                pc = 0x29604; continue;
            }
            0x29604 => {
                fa = m.read32(esp.wrapping_add(0x4));
                fb = esi;
                if (fa < fb) { pc = 0x29549; continue; }
                pc = 0x2960e; continue;
            }
            0x2960e => {
                eax = m.read32(0x6648d4);
                call!(0x29160);
                pc = 0x29618; continue;
            }
            0x29618 => {
                call!(0x29130);
                pc = 0x2961d; continue;
            }
            0x2961d => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x29620 => {
                eax = m.read32(0x6648d4);
                m.write32(eax.wrapping_add(0x28), esi);
                m.write32(eax.wrapping_add(0x34), 0x2);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x29632 => {
                fa = eax;
                fb = ((((0x6f8i32)) as u32) & 0xffffffffu32);
                pc = 0x29547; continue;
            }
            0x2963c => {
                call!(0x29130);
                pc = 0x29641; continue;
            }
            0x29641 => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x29650. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn directory_create<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29650;
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
        { crate::pose::cover(0x29650, previous, pc); previous = pc; }
        match pc {
            0x29650 => {
                esp = esp.wrapping_sub(0x100);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x108));
                push!(esi);
                push!(edi);
                edi = ebp;
                esi = 0;
                ecx = edi.wrapping_add(0x1);
                pc = 0x29667; continue;
            }
            0x29667 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi)) as u32)) & 0xff);
                edi = edi.wrapping_add(1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x29667; continue; }
                pc = 0x2966e; continue;
            }
            0x2966e => {
                edi = edi.wrapping_sub(ecx);
                pc = 0x29670; continue;
            }
            0x29670 => {
                edi = edi.wrapping_sub(1);
                fa = edi;
                if ((fa == 0)) { pc = 0x2967e; continue; }
                pc = 0x29673; continue;
            }
            0x29673 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(ebp))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x5ci32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x2967e; continue; }
                pc = 0x2967a; continue;
            }
            0x2967a => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2fi32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x29670; continue; }
                pc = 0x2967e; continue;
            }
            0x2967e => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x296da; continue; }
                pc = 0x29682; continue;
            }
            0x29682 => {
                fa = esi;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x296c5; continue; }
                pc = 0x29686; continue;
            }
            0x29686 => {
                edx = esp.wrapping_add(0xc);
                eax = esi.wrapping_add(ebp);
                edx = edx.wrapping_sub(ebp);
                pc = 0x29690; continue;
            }
            0x29690 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x5ci32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x2969c; continue; }
                pc = 0x29697; continue;
            }
            0x29697 => {
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x2fi32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x296a7; continue; }
                pc = 0x2969c; continue;
            }
            0x2969c => {
                esi = esi.wrapping_add(1);
                m.write8(edx.wrapping_add(eax), (((((ecx) as u8)) as u32)) as u8);
                eax = eax.wrapping_add(1);
                fa = esi;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x29690; continue; }
                pc = 0x296a5; continue;
            }
            0x296a5 => {
                pc = 0x296c5; continue;
            }
            0x296a7 => {
                fa = esi;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x296c5; continue; }
                pc = 0x296ab; continue;
            }
            0x296ab => {
                eax = esi.wrapping_add(ebp);
                edi = edi;
                pc = 0x296b0; continue;
            }
            0x296b0 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x5ci32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x296c5; continue; }
                pc = 0x296b7; continue;
            }
            0x296b7 => {
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x2fi32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x296c5; continue; }
                pc = 0x296bc; continue;
            }
            0x296bc => {
                esi = esi.wrapping_add(1);
                m.write8(eax.wrapping_add(edx), (((((ecx) as u8)) as u32)) as u8);
                eax = eax.wrapping_add(1);
                fa = esi;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x296b0; continue; }
                pc = 0x296c5; continue;
            }
            0x296c5 => {
                push!(0);
                eax = esp.wrapping_add(0x10);
                push!(eax);
                m.write8(esp.wrapping_add(esi).wrapping_add(0x14), (0) as u8);
                call!(0x3800df);
                pc = 0x296d6; continue;
            }
            0x296d6 => {
                fa = esi;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x29686; continue; }
                pc = 0x296da; continue;
            }
            0x296da => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x100);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x296F0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_open_path<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x296f0;
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
        { crate::pose::cover(0x296f0, previous, pc); previous = pc; }
        match pc {
            0x296f0 => {
                esp = esp.wrapping_sub(0x10);
                call!(0x28af0);
                pc = 0x296f8; continue;
            }
            0x296f8 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x4), eax);
                if (fa != fb) { pc = 0x29707; continue; }
                pc = 0x29701; continue;
            }
            0x29701 => {
                eax = (eax | eax);
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            0x29707 => {
                eax = m.read32(esp.wrapping_add(0x18));
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                ebx = 0;
                edi = 0;
                ebp = 0;
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                m.write32(esp.wrapping_add(0x10), ebx);
                if ((fa & fb) == 0) { pc = 0x29734; continue; }
                pc = 0x2971d; continue;
            }
            0x2971d => {
                edi = 0x80000000u32;
                ebp = 0x3;
                ebx = 0x1;
                m.write32(esp.wrapping_add(0x10), 0x60000000);
                pc = 0x29734; continue;
            }
            0x29734 => {
                edx = eax;
                edx = (edx & 0x8);
                fa = edx;
                m.write32(esp.wrapping_add(0x18), edx);
                if ((fa == 0)) { pc = 0x29748; continue; }
                pc = 0x2973f; continue;
            }
            0x2973f => {
                edi = (edi | 0x40000000);
                ebx = (ebx | 0x2);
                pc = 0x29748; continue;
            }
            0x29748 => {
                ecx = eax;
                ecx = (ecx & 0x4);
                fa = ecx;
                m.write32(esp.wrapping_add(0x1c), ecx);
                if ((fa == 0)) { pc = 0x29759; continue; }
                pc = 0x29753; continue;
            }
            0x29753 => {
                ebp = (ebp & 0xfffffffdu32);
                ebp = (ebp | 0x5);
                pc = 0x29759; continue;
            }
            0x29759 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                esi = m.read32(esp.wrapping_add(0x24));
                if ((fa & fb) == 0) { pc = 0x2977b; continue; }
                pc = 0x29761; continue;
            }
            0x29761 => {
                ebp = (ebp & 0xfffffffau32);
                eax = esi;
                ebp = (ebp | 0x2);
                call!(0x28c20);
                pc = 0x2976e; continue;
            }
            0x2976e => {
                push!(eax);
                call!(0x29650);
                pc = 0x29774; continue;
            }
            0x29774 => {
                edx = m.read32(esp.wrapping_add(0x1c));
                esp = esp.wrapping_add(0x4);
                pc = 0x2977b; continue;
            }
            0x2977b => {
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(0);
                if (fa == fb) { pc = 0x297a5; continue; }
                pc = 0x29781; continue;
            }
            0x29781 => {
                eax = m.read32(esp.wrapping_add(0x14));
                push!(eax);
                push!(ebp);
                push!(0);
                push!(ebx);
                push!(edi);
                eax = esi;
                call!(0x28c20);
                pc = 0x29792; continue;
            }
            0x29792 => {
                push!(eax);
                call!(0x37d231);
                pc = 0x29798; continue;
            }
            0x29798 => {
                ebp = m.read32(esp.wrapping_add(0x14));
                m.write32(ebp.wrapping_mul(0x4).wrapping_add(0x6612d0), eax);
                pc = 0x297c9; continue;
            }
            0x297a5 => {
                ecx = m.read32(esp.wrapping_add(0x14));
                push!(ecx);
                push!(ebp);
                push!(0);
                push!(ebx);
                push!(edi);
                eax = esi;
                call!(0x28cb0);
                pc = 0x297b6; continue;
            }
            0x297b6 => {
                push!(eax);
                call!(0x37d231);
                pc = 0x297bc; continue;
            }
            0x297bc => {
                edx = m.read32(esp.wrapping_add(0x14));
                m.write32(edx.wrapping_mul(0x4).wrapping_add(0x6612d0), eax);
                ebp = edx;
                pc = 0x297c9; continue;
            }
            0x297c9 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x29808; continue; }
                pc = 0x297d1; continue;
            }
            0x297d1 => {
                fa = m.read32(ebp.wrapping_mul(0x4).wrapping_add(0x6612d0));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x29808; continue; }
                pc = 0x297db; continue;
            }
            0x297db => {
                eax = m.read32(esp.wrapping_add(0x18));
                push!(0);
                push!(0);
                push!(0x1);
                push!(0);
                push!(ebx);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(edi);
                eax = esi;
                if (fa == fb) { pc = 0x297f6; continue; }
                pc = 0x297ef; continue;
            }
            0x297ef => {
                call!(0x28c20);
                pc = 0x297f4; continue;
            }
            0x297f4 => {
                pc = 0x297fb; continue;
            }
            0x297f6 => {
                call!(0x28cb0);
                pc = 0x297fb; continue;
            }
            0x297fb => {
                push!(eax);
                call!(0x37d231);
                pc = 0x29801; continue;
            }
            0x29801 => {
                m.write32(ebp.wrapping_mul(0x4).wrapping_add(0x6612d0), eax);
                pc = 0x29808; continue;
            }
            0x29808 => {
                ecx = m.read32(ebp.wrapping_mul(0x4).wrapping_add(0x6612d0));
                eax = (eax | 0xffffffffu32);
                fa = ecx;
                fb = eax;
                m.write32(ebp.wrapping_mul(0x4).wrapping_add(0x65fe30), 0);
                if (fa == fb) { pc = 0x2983c; continue; }
                pc = 0x29821; continue;
            }
            0x29821 => {
                ecx = ebp;
                ecx = ecx.wrapping_shl((((((((0x7i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = ecx.wrapping_add(0x65fed0);
                eax = esi;
                edx = edx.wrapping_sub(esi);
                pc = 0x29830; continue;
            }
            0x29830 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                m.write8(edx.wrapping_add(eax), (((((ecx) as u8)) as u32)) as u8);
                eax = eax.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x29830; continue; }
                pc = 0x2983a; continue;
            }
            0x2983a => {
                eax = ebp;
                pc = 0x2983c; continue;
            }
            0x2983c => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x29850. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_async_request_enqueue_read_with_zero_arg5<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x29850;
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
        { crate::pose::cover(0x29850, previous, pc); previous = pc; }
        match pc {
            0x29850 => {
                eax = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(esp.wrapping_add(0xc));
                edx = m.read32(esp.wrapping_add(0x8));
                push!(0);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0xc));
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x29370);
                pc = 0x2986b; continue;
            }
            0x2986b => {
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x3EF10. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn wrapper_3ee70_first_arg_zero<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3ef10;
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
        { crate::pose::cover(0x3ef10, previous, pc); previous = pc; }
        match pc {
            0x3ef10 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(esp.wrapping_add(0x4));
                push!(eax);
                push!(ecx);
                push!(0);
                call!(0x3ee70);
                pc = 0x3ef21; continue;
            }
            0x3ef21 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x3EF30. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_or_disk_request_slot_register_in_48_byte_table_6b83e4_returning_slot_index<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3ef30;
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
        { crate::pose::cover(0x3ef30, previous, pc); previous = pc; }
        match pc {
            0x3ef30 => {
                eax = m.read32(0x6b83d8);
                push!(ebx);
                push!(ebp);
                push!(esi);
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(edi);
                ebp = eax;
                if (fas <= fbs) { pc = 0x3ef56; continue; }
                pc = 0x3ef41; continue;
            }
            0x3ef41 => {
                esi = m.read32(0x6b83e4);
                pc = 0x3ef47; continue;
            }
            0x3ef47 => {
                edx = m.read32(esi.wrapping_add(0x8));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3ef9c; continue; }
                pc = 0x3ef4e; continue;
            }
            0x3ef4e => {
                ecx = ecx.wrapping_add(1);
                esi = esi.wrapping_add(0x30);
                fa = ecx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3ef47; continue; }
                pc = 0x3ef56; continue;
            }
            0x3ef56 => {
                edi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3ef8a; continue; }
                pc = 0x3ef5c; continue;
            }
            0x3ef5c => {
                ebx = 0;
                edi = edi;
                pc = 0x3ef60; continue;
            }
            0x3ef60 => {
                eax = m.read32(0x6b83e4);
                esi = ebx.wrapping_add(eax);
                eax = m.read32(esi.wrapping_add(0xc));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3ef82; continue; }
                pc = 0x3ef6f; continue;
            }
            0x3ef6f => {
                ecx = m.read32(esi.wrapping_add(0x4));
                edx = m.read32(esi);
                push!(0);
                push!(ecx);
                push!(edi);
                push!(edx);
                call_to!(eax);
                pc = 0x3ef7b; continue;
            }
            0x3ef7b => {
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3ef9c; continue; }
                pc = 0x3ef82; continue;
            }
            0x3ef82 => {
                edi = edi.wrapping_add(1);
                ebx = ebx.wrapping_add(0x30);
                fa = edi;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3ef60; continue; }
                pc = 0x3ef8a; continue;
            }
            0x3ef8a => {
                ecx = m.read32(esp.wrapping_add(0x14));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(ecx, 0);
                eax = (eax | 0xffffffffu32);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x3ef9c => {
                eax = m.read32(esp.wrapping_add(0x18));
                edx = m.read32(esi);
                ecx = m.read32(esp.wrapping_add(0x1c));
                m.write32(esi.wrapping_add(0xc), eax);
                eax = m.read32(esp.wrapping_add(0x14));
                m.write32(esi.wrapping_add(0x8), 0x1);
                m.write32(esi.wrapping_add(0x4), ecx);
                m.write32(eax, edx);
                esi = esi.wrapping_sub(m.read32(0x6b83e4));
                eax = 0x2aaaaaab;
                { let wide = (((eax) as i32 as i64).wrapping_mul((esi) as i32 as i64)) as u64; eax = wide as u32; edx = (wide >> 32) as u32; }
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                edx = (((((edx as i32)).wrapping_shr((((((((0x3i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = edx;
                eax = eax.wrapping_shr((((((((0x1fi32)) as u32) & 0x1fu32))) as u32 & 31));
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(edx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x57B80. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_text_find_prefix_at_line_start_in_bounded_buffer<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x57b80;
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
        { crate::pose::cover(0x57b80, previous, pc); previous = pc; }
        match pc {
            0x57b80 => {
                push!(ecx);
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0xc));
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(edi);
                if (fa == fb) { pc = 0x57c30; continue; }
                pc = 0x57b8f; continue;
            }
            0x57b8f => {
                edi = m.read32(esp.wrapping_add(0x14));
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x57c30; continue; }
                pc = 0x57b9b; continue;
            }
            0x57b9b => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi)) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                m.write8(esp.wrapping_add(0xb), (((((eax) as u8)) as u32)) as u8);
                if (fa == fb) { pc = 0x57c30; continue; }
                pc = 0x57ba9; continue;
            }
            0x57ba9 => {
                eax = edi;
                edx = eax.wrapping_add(0x1);
                edi = edi;
                pc = 0x57bb0; continue;
            }
            0x57bb0 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                eax = eax.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x57bb0; continue; }
                pc = 0x57bb7; continue;
            }
            0x57bb7 => {
                eax = eax.wrapping_sub(edx);
                push!(ebp);
                ebp = eax;
                eax = m.read32(esp.wrapping_add(0x1c));
                push!(esi);
                esi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x57c1f; continue; }
                pc = 0x57bc7; continue;
            }
            0x57bc7 => {
                edx = edi;
                edx = (((((edx) as i32).wrapping_neg())) as u32);
                pc = 0x57bd0; continue;
            }
            0x57bd0 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x57bdb; continue; }
                pc = 0x57bd4; continue;
            }
            0x57bd4 => {
                fa = ((((m.read8(esi.wrapping_add(ebx).wrapping_add(((0x1i32.wrapping_neg()) as u32))))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x57c15; continue; }
                pc = 0x57bdb; continue;
            }
            0x57bdb => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esp.wrapping_add(0x13))) as u32)) & 0xff);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(ebx))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((((eax) as u8))) as u32) & 0xffu32);
                edi = esi.wrapping_add(ebx);
                if (fa != fb) { pc = 0x57c15; continue; }
                pc = 0x57be9; continue;
            }
            0x57be9 => {
                ecx = 0x1;
                fa = ebp;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x57c11; continue; }
                pc = 0x57bf2; continue;
            }
            0x57bf2 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                eax = eax.wrapping_add(1);
                pc = 0x57bf7; continue;
            }
            0x57bf7 => {
                ebx = edx.wrapping_add(eax);
                fa = ebx;
                fb = m.read32(esp.wrapping_add(0x20));
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x57c0d; continue; }
                pc = 0x57c00; continue;
            }
            0x57c00 => {
                ebx = (ebx & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(ecx))) as u32)) & 0xff);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((m.read8(eax))) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x57c0d; continue; }
                pc = 0x57c07; continue;
            }
            0x57c07 => {
                ecx = ecx.wrapping_add(1);
                eax = eax.wrapping_add(1);
                fa = ecx;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x57bf7; continue; }
                pc = 0x57c0d; continue;
            }
            0x57c0d => {
                ebx = m.read32(esp.wrapping_add(0x18));
                pc = 0x57c11; continue;
            }
            0x57c11 => {
                fa = ecx;
                fb = ebp;
                if (fa == fb) { pc = 0x57c27; continue; }
                pc = 0x57c15; continue;
            }
            0x57c15 => {
                eax = m.read32(esp.wrapping_add(0x20));
                esi = esi.wrapping_add(1);
                edx = edx.wrapping_add(1);
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x57bd0; continue; }
                pc = 0x57c1f; continue;
            }
            0x57c1f => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x57c27 => {
                eax = esi.wrapping_add(ebx);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x57c30 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x57DB0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_text_records_parse_with_atol_into_arena_table<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x57db0;
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
        { crate::pose::cover(0x57db0, previous, pc); previous = pc; }
        match pc {
            0x57db0 => {
                push!(ecx);
                push!(ebx);
                push!(ebp);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x14));
                push!(edi);
                ebp = 0;
                push!(esi);
                m.write32(esp.wrapping_add(0x14), ebp);
                call!(0x613c0);
                pc = 0x57dc5; continue;
            }
            0x57dc5 => {
                ebx = eax;
                eax = esi;
                esp = esp.wrapping_add(0x4);
                edx = eax.wrapping_add(0x1);
                pc = 0x57dd0; continue;
            }
            0x57dd0 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                eax = eax.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x57dd0; continue; }
                pc = 0x57dd7; continue;
            }
            0x57dd7 => {
                eax = eax.wrapping_sub(edx);
                esi = esi.wrapping_add(eax);
                fa = ((((m.read8(esi))) as u32) & 0xffu32);
                fb = ((((0x2fi32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x57de8; continue; }
                pc = 0x57de0; continue;
            }
            0x57de0 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(((0x1i32.wrapping_neg()) as u32)))) as u32)) & 0xff);
                esi = esi.wrapping_sub(1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2fi32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x57de0; continue; }
                pc = 0x57de8; continue;
            }
            0x57de8 => {
                push!(ebp);
                push!(ebx);
                call!(0x62c30);
                pc = 0x57def; continue;
            }
            0x57def => {
                push!(ebx);
                edi = eax;
                call!(0x62ca0);
                pc = 0x57df7; continue;
            }
            0x57df7 => {
                ecx = m.read32(esp.wrapping_add(0x28));
                ecx = (((((m.read16(ecx.wrapping_mul(0x2).wrapping_add(0x7e03c0)) as i16)) as i32)) as u32);
                edx = m.read32(0x7e3954);
                ecx = ecx.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                esp = esp.wrapping_add(0xc);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(ecx.wrapping_add(0x7e1be0), ebp);
                ebp = m.read32(ecx.wrapping_add(edx));
                if (fa == fb) { pc = 0x57f2a; continue; }
                pc = 0x57e20; continue;
            }
            0x57e20 => {
                push!(eax);
                esi = esi.wrapping_add(1);
                push!(esi);
                push!(edi);
                call!(0x57b80);
                pc = 0x57e29; continue;
            }
            0x57e29 => {
                esi = eax;
                esp = esp.wrapping_add(0xc);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x57f2a; continue; }
                pc = 0x57e36; continue;
            }
            0x57e36 => {
                fa = ((((m.read8(esi))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x57e48; continue; }
                pc = 0x57e3b; continue;
            }
            0x57e3b => {
                pc = 0x57e40; continue;
            }
            0x57e40 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x1))) as u32)) & 0xff);
                esi = esi.wrapping_add(1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x57e40; continue; }
                pc = 0x57e48; continue;
            }
            0x57e48 => {
                esi = esi.wrapping_add(1);
                push!(esi);
                call!(0x3c8fd0);
                pc = 0x57e4f; continue;
            }
            0x57e4f => {
                ebx = eax;
                esp = esp.wrapping_add(0x4);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x57ea9; continue; }
                pc = 0x57e58; continue;
            }
            0x57e58 => {
                eax = m.read32(esp.wrapping_add(0x20));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x57e7b; continue; }
                pc = 0x57e60; continue;
            }
            0x57e60 => {
                eax = ebx.wrapping_mul(0x4).wrapping_add(0x17);
                eax = (eax & 0xfffffff0u32);
                push!(0);
                push!(eax);
                call!(0x3dfa0);
                pc = 0x57e72; continue;
            }
            0x57e72 => {
                esp = esp.wrapping_add(0x8);
                m.write32(esp.wrapping_add(0x10), eax);
                pc = 0x57ea9; continue;
            }
            0x57e7b => {
                ecx = m.read32(0x6d6e14);
                eax = ebx.wrapping_mul(0x4).wrapping_add(0xb);
                eax = (eax & 0xfffffffcu32);
                eax = eax.wrapping_add(ecx);
                fa = eax;
                fb = ((((0x1400i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x57e98; continue; }
                pc = 0x57e94; continue;
            }
            0x57e94 => {
                ebx = 0;
                pc = 0x57ea9; continue;
            }
            0x57e98 => {
                edx = m.read32(0x6d7800);
                edx = edx.wrapping_add(ecx);
                m.write32(esp.wrapping_add(0x10), edx);
                m.write32(0x6d6e14, eax);
                pc = 0x57ea9; continue;
            }
            0x57ea9 => {
                eax = m.read32(esp.wrapping_add(0x10));
                write16(m, eax.wrapping_add(0x6), (((((ebx) as u16)) as u32)) as u16);
                ecx = (ecx & 0xffff_0000) | ((((m.read16(ebp.wrapping_add(0x10))) as u32)) & 0xffff);
                edi = 0;
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                write16(m, eax.wrapping_add(0x4), (((((ecx) as u16)) as u32)) as u16);
                if (fas <= fbs) { pc = 0x57f13; continue; }
                pc = 0x57ebf; continue;
            }
            0x57ebf => {
                pc = 0x57ec0; continue;
            }
            0x57ec0 => {
                fa = ((((m.read8(esi))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x57ecd; continue; }
                pc = 0x57ec5; continue;
            }
            0x57ec5 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x1))) as u32)) & 0xff);
                esi = esi.wrapping_add(1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x57ec5; continue; }
                pc = 0x57ecd; continue;
            }
            0x57ecd => {
                esi = esi.wrapping_add(1);
                push!(esi);
                call!(0x3c8fd0);
                pc = 0x57ed4; continue;
            }
            0x57ed4 => {
                ebp = m.read32(esp.wrapping_add(0x14));
                write16(m, ebp.wrapping_add(edi.wrapping_mul(0x4)).wrapping_add(0x8), (((((eax) as u16)) as u32)) as u16);
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi)) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x57ef3; continue; }
                pc = 0x57ee6; continue;
            }
            0x57ee6 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x9i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x57ef3; continue; }
                pc = 0x57eea; continue;
            }
            0x57eea => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x1))) as u32)) & 0xff);
                esi = esi.wrapping_add(1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x57ee6; continue; }
                pc = 0x57ef2; continue;
            }
            0x57ef2 => {
                esi = esi.wrapping_add(1);
                pc = 0x57ef3; continue;
            }
            0x57ef3 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi)) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x57efd; continue; }
                pc = 0x57ef9; continue;
            }
            0x57ef9 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x9i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x57f00; continue; }
                pc = 0x57efd; continue;
            }
            0x57efd => {
                esi = esi.wrapping_add(1);
                pc = 0x57ef3; continue;
            }
            0x57f00 => {
                push!(esi);
                call!(0x3c8fd0);
                pc = 0x57f06; continue;
            }
            0x57f06 => {
                esp = esp.wrapping_add(0x4);
                write16(m, ebp.wrapping_add(edi.wrapping_mul(0x4)).wrapping_add(0xa), (((((eax) as u16)) as u32)) as u16);
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x57ec0; continue; }
                pc = 0x57f13; continue;
            }
            0x57f13 => {
                edx = m.read32(esp.wrapping_add(0x1c));
                eax = (((((m.read16(edx.wrapping_mul(0x2).wrapping_add(0x7e03c0)) as i16)) as i32)) as u32);
                ecx = m.read32(esp.wrapping_add(0x10));
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7e1be0), ecx);
                pc = 0x57f2a; continue;
            }
            0x57f2a => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x5A130. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_load_from_path_returns_success_bool<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x5a130;
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
        { crate::pose::cover(0x5a130, previous, pc); previous = pc; }
        match pc {
            0x5a130 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(0x1);
                push!(0x1);
                push!(eax);
                call!(0x622d0);
                pc = 0x5a13e; continue;
            }
            0x5a13e => {
                ecx = 0;
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fas >= fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                eax = ecx;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x613C0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn global_4d19dc_get<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x613c0;
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
        { crate::pose::cover(0x613c0, previous, pc); previous = pc; }
        match pc {
            0x613c0 => {
                eax = m.read32(0x4d19dc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x616D0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_entries_fill_size_field_0x14_stride_0x18_skip_marker_0x454d5059<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x616d0;
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
        { crate::pose::cover(0x616d0, previous, pc); previous = pc; }
        match pc {
            0x616d0 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x10));
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x10));
                if (fas <= fbs) { pc = 0x61704; continue; }
                pc = 0x616de; continue;
            }
            0x616de => {
                push!(esi);
                push!(edi);
                esi = ebp.wrapping_add(0x14);
                edi = ebx;
                pc = 0x616e5; continue;
            }
            0x616e5 => {
                fa = ((m.read32(esi.wrapping_add(((0x14i32.wrapping_neg()) as u32)))) & 0xffffffffu32);
                fb = ((((0x454d5059i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x616fc; continue; }
                pc = 0x616ee; continue;
            }
            0x616ee => {
                eax = m.read32(esi.wrapping_add(((0x10i32.wrapping_neg()) as u32)));
                push!(eax);
                call!(0x62510);
                pc = 0x616f7; continue;
            }
            0x616f7 => {
                esp = esp.wrapping_add(0x4);
                m.write32(esi, eax);
                pc = 0x616fc; continue;
            }
            0x616fc => {
                esi = esi.wrapping_add(0x18);
                edi = edi.wrapping_sub(1);
                fa = edi;
                if ((fa != 0)) { pc = 0x616e5; continue; }
                pc = 0x61702; continue;
            }
            0x61702 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x61704; continue;
            }
            0x61704 => {
                eax = m.read32(esp.wrapping_add(0xc));
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x6d7844), ebp);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x6d784c), ebx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x61FD0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_slot_alloc_and_store_name<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x61fd0;
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
        { crate::pose::cover(0x61fd0, previous, pc); previous = pc; }
        match pc {
            0x61fd0 => {
                ecx = m.read32(0x6d7880);
                push!(edi);
                edi = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x61fef; continue; }
                pc = 0x61fdd; continue;
            }
            0x61fdd => {
                eax = 0x6d7918;
                pc = 0x61fe2; continue;
            }
            0x61fe2 => {
                fa = m.read32(eax);
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x61ff8; continue; }
                pc = 0x61fe7; continue;
            }
            0x61fe7 => {
                edi = edi.wrapping_add(1);
                eax = eax.wrapping_add(0x1c);
                fa = edi;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x61fe2; continue; }
                pc = 0x61fef; continue;
            }
            0x61fef => {
                edi = ecx;
                ecx = ecx.wrapping_add(1);
                m.write32(0x6d7880, ecx);
                pc = 0x61ff8; continue;
            }
            0x61ff8 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                esi = edi;
                esi = ((esi as i32).wrapping_mul((0x1c) as i32)) as u32;
                eax = ebx;
                esi = esi.wrapping_add(0x6d7900);
                edx = eax.wrapping_add(0x1);
                edi = edi;
                pc = 0x62010; continue;
            }
            0x62010 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                eax = eax.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x62010; continue; }
                pc = 0x62017; continue;
            }
            0x62017 => {
                ecx = m.read32(esp.wrapping_add(0x14));
                eax = eax.wrapping_sub(edx);
                push!(ecx);
                eax = eax.wrapping_add(1);
                push!(eax);
                call!(0x3dfa0);
                pc = 0x62025; continue;
            }
            0x62025 => {
                esp = esp.wrapping_add(0x8);
                m.write32(esi.wrapping_add(0x14), eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = ebx;
                edx = eax;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x62031; continue;
            }
            0x62031 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(ecx)) as u32)) & 0xff);
                ecx = ecx.wrapping_add(1);
                m.write8(edx, (((((eax) as u8)) as u32)) as u8);
                edx = edx.wrapping_add(1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x62031; continue; }
                pc = 0x6203b; continue;
            }
            0x6203b => {
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62040. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn asset_62040<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62040;
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
        { crate::pose::cover(0x62040, previous, pc); previous = pc; }
        match pc {
            0x62040 => {
                ecx = m.read32(0x6d7880);
                esp = esp.wrapping_sub(0xc);
                fa = ecx;
                fb = ((((0x1ei32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62055; continue; }
                pc = 0x6204e; continue;
            }
            0x6204e => {
                eax = (eax | 0xffffffffu32);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            0x62055 => {
                push!(ebx);
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x1c));
                eax = edi;
                edx = m.read32(eax);
                m.write32(esp.wrapping_add(0xc), edx);
                edx = m.read32(eax.wrapping_add(0x4));
                m.write32(esp.wrapping_add(0x10), edx);
                edx = m.read32(eax.wrapping_add(0x8));
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x6208d; continue; }
                pc = 0x62074; continue;
            }
            0x62074 => {
                esi = 0x6d7918;
                pc = 0x62080; continue;
            }
            0x62080 => {
                fa = m.read32(esi);
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62096; continue; }
                pc = 0x62085; continue;
            }
            0x62085 => {
                eax = eax.wrapping_add(1);
                esi = esi.wrapping_add(0x1c);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62080; continue; }
                pc = 0x6208d; continue;
            }
            0x6208d => {
                eax = ecx;
                ecx = ecx.wrapping_add(1);
                m.write32(0x6d7880, ecx);
                pc = 0x62096; continue;
            }
            0x62096 => {
                esi = m.read32(esp.wrapping_add(0x24));
                ebx = m.read32(esp.wrapping_add(0x20));
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x1c) as i32)) as u32;
                ecx = ecx.wrapping_add(0x6d7900);
                m.write32(ecx.wrapping_add(0x18), esi);
                esi = edi;
                esi = esi.wrapping_sub(edx);
                esi = esi.wrapping_add(ebx);
                m.write32(ecx.wrapping_add(0xc), edi);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(ecx.wrapping_add(0x10), esi);
                edx = edx.wrapping_shr((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(ecx.wrapping_add(0x14), 0);
                m.write32(ecx, 0xffffffffu32);
                m.write32(ecx.wrapping_add(0x8), edx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62055. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_slot_register_buffer<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62055;
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
        { crate::pose::cover(0x62055, previous, pc); previous = pc; }
        match pc {
            0x62040 => {
                ecx = m.read32(0x6d7880);
                esp = esp.wrapping_sub(0xc);
                fa = ecx;
                fb = ((((0x1ei32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62055; continue; }
                pc = 0x6204e; continue;
            }
            0x6204e => {
                eax = (eax | 0xffffffffu32);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            0x62055 => {
                push!(ebx);
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x1c));
                eax = edi;
                edx = m.read32(eax);
                m.write32(esp.wrapping_add(0xc), edx);
                edx = m.read32(eax.wrapping_add(0x4));
                m.write32(esp.wrapping_add(0x10), edx);
                edx = m.read32(eax.wrapping_add(0x8));
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x6208d; continue; }
                pc = 0x62074; continue;
            }
            0x62074 => {
                esi = 0x6d7918;
                pc = 0x62080; continue;
            }
            0x62080 => {
                fa = m.read32(esi);
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62096; continue; }
                pc = 0x62085; continue;
            }
            0x62085 => {
                eax = eax.wrapping_add(1);
                esi = esi.wrapping_add(0x1c);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62080; continue; }
                pc = 0x6208d; continue;
            }
            0x6208d => {
                eax = ecx;
                ecx = ecx.wrapping_add(1);
                m.write32(0x6d7880, ecx);
                pc = 0x62096; continue;
            }
            0x62096 => {
                esi = m.read32(esp.wrapping_add(0x24));
                ebx = m.read32(esp.wrapping_add(0x20));
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x1c) as i32)) as u32;
                ecx = ecx.wrapping_add(0x6d7900);
                m.write32(ecx.wrapping_add(0x18), esi);
                esi = edi;
                esi = esi.wrapping_sub(edx);
                esi = esi.wrapping_add(ebx);
                m.write32(ecx.wrapping_add(0xc), edi);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(ecx.wrapping_add(0x10), esi);
                edx = edx.wrapping_shr((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(ecx.wrapping_add(0x14), 0);
                m.write32(ecx, 0xffffffffu32);
                m.write32(ecx.wrapping_add(0x8), edx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x620E0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_header_read_and_validate<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x620e0;
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
        { crate::pose::cover(0x620e0, previous, pc); previous = pc; }
        match pc {
            0x620e0 => {
                eax = m.read32(0x6d7880);
                esp = esp.wrapping_sub(0xc);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x14));
                esi = ((esi as i32).wrapping_mul((0x1c) as i32)) as u32;
                push!(edi);
                edi = 0;
                esi = esi.wrapping_add(0x6d7900);
                fa = eax;
                fb = ((((0x1ei32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62107; continue; }
                pc = 0x620fe; continue;
            }
            0x620fe => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = (eax | 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            0x62107 => {
                eax = m.read32(esp.wrapping_add(0x20));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x20));
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x30));
                if (fa == fb) { pc = 0x62152; continue; }
                pc = 0x62119; continue;
            }
            0x62119 => {
                edi = m.read32(esp.wrapping_add(0x34));
                push!(0x1);
                push!(0);
                push!(ebx);
                call!(0x29030);
                pc = 0x62127; continue;
            }
            0x62127 => {
                push!(edi);
                push!(ebp);
                push!(ebx);
                call!(0x28d50);
                pc = 0x6212f; continue;
            }
            0x6212f => {
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = edi;
                if (fa != fb) { pc = 0x62248; continue; }
                pc = 0x6213a; continue;
            }
            0x6213a => {
                ecx = ebp;
                eax = m.read32(ecx);
                edx = m.read32(ecx.wrapping_add(0x4));
                ecx = m.read32(ecx.wrapping_add(0x8));
                m.write32(esp.wrapping_add(0x10), eax);
                m.write32(esp.wrapping_add(0x14), edx);
                m.write32(esp.wrapping_add(0x18), ecx);
                pc = 0x62173; continue;
            }
            0x62152 => {
                push!(0xc);
                eax = esp.wrapping_add(0x14);
                push!(eax);
                push!(ebx);
                call!(0x28d50);
                pc = 0x6215f; continue;
            }
            0x6215f => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0xci32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x62248; continue; }
                pc = 0x6216b; continue;
            }
            0x6216b => {
                eax = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(esp.wrapping_add(0x18));
                pc = 0x62173; continue;
            }
            0x62173 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x50i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x62248; continue; }
                pc = 0x6217b; continue;
            }
            0x6217b => {
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x35i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x62248; continue; }
                pc = 0x62184; continue;
            }
            0x62184 => {
                fa = ((((m.read8(esp.wrapping_add(0x12)))) as u32) & 0xffu32);
                fb = ((((0x43i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x62248; continue; }
                pc = 0x6218f; continue;
            }
            0x6218f => {
                fa = ((((m.read8(esp.wrapping_add(0x13)))) as u32) & 0xffu32);
                fb = ((((0x4bi32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x62248; continue; }
                pc = 0x6219a; continue;
            }
            0x6219a => {
                fa = ecx;
                fb = ((((0x80000i32)) as u32) & 0xffffffffu32);
                if (fa <= fb) { pc = 0x621af; continue; }
                pc = 0x621a2; continue;
            }
            0x621a2 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0xffffffffu32;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            0x621af => {
                edx = m.read32(esp.wrapping_add(0x28));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = m.read32(esp.wrapping_add(0x2c));
                m.write32(esi.wrapping_add(0x18), eax);
                if (fa == fb) { pc = 0x621d0; continue; }
                pc = 0x621be; continue;
            }
            0x621be => {
                edi = edi.wrapping_sub(ecx);
                edi = edi.wrapping_add(ebp);
                m.write32(esi.wrapping_add(0x10), edi);
                m.write32(esi, 0xffffffffu32);
                m.write32(esi.wrapping_add(0xc), ebp);
                pc = 0x62223; continue;
            }
            0x621d0 => {
                edx = m.read32(esi.wrapping_add(0x10));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x621ea; continue; }
                pc = 0x621d7; continue;
            }
            0x621d7 => {
                push!(eax);
                push!(ecx);
                push!(0x40);
                call!(0x3e0b0);
                pc = 0x621e0; continue;
            }
            0x621e0 => {
                ecx = m.read32(esp.wrapping_add(0x24));
                esp = esp.wrapping_add(0xc);
                m.write32(esi.wrapping_add(0x10), eax);
                pc = 0x621ea; continue;
            }
            0x621ea => {
                push!(0);
                ecx = (((((ecx) as i32).wrapping_neg())) as u32);
                push!(ecx);
                push!(ebx);
                call!(0x29030);
                pc = 0x621f5; continue;
            }
            0x621f5 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x62248; continue; }
                pc = 0x621fc; continue;
            }
            0x621fc => {
                ecx = m.read32(esp.wrapping_add(0x18));
                edx = m.read32(esi.wrapping_add(0x10));
                push!(ecx);
                push!(edx);
                push!(ebx);
                call!(0x28d50);
                pc = 0x6220b; continue;
            }
            0x6220b => {
                ecx = m.read32(esp.wrapping_add(0x24));
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ecx;
                if (fa != fb) { pc = 0x62248; continue; }
                pc = 0x62216; continue;
            }
            0x62216 => {
                eax = m.read32(esp.wrapping_add(0x2c));
                m.write32(esi, ebx);
                m.write32(esi.wrapping_add(0xc), 0);
                pc = 0x62223; continue;
            }
            0x62223 => {
                ecx = ecx.wrapping_shr((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write32(esi.wrapping_add(0x8), ecx);
                ecx = m.read32(esp.wrapping_add(0x28));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x6223c; continue; }
                pc = 0x62231; continue;
            }
            0x62231 => {
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x6223c; continue; }
                pc = 0x62236; continue;
            }
            0x62236 => {
                m.write32(0x6d7c48, m.read32(0x6d7c48).wrapping_add(0x1));
                pc = 0x6223c; continue;
            }
            0x6223c => {
                eax = m.read32(esp.wrapping_add(0x20));
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            0x62248 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = (eax | 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62260. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_slot_find_by_name<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62260;
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
        { crate::pose::cover(0x62260, previous, pc); previous = pc; }
        match pc {
            0x62260 => {
                push!(ebx);
                ebx = m.read32(0x6d7880);
                push!(edi);
                eax = (eax | 0xffffffffu32);
                edi = 0;
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x622c4; continue; }
                pc = 0x62271; continue;
            }
            0x62271 => {
                push!(ebp);
                ebp = 0x6d7914;
                push!(esi);
                pc = 0x62278; continue;
            }
            0x62278 => {
                esi = m.read32(ebp);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x622b0; continue; }
                pc = 0x6227f; continue;
            }
            0x6227f => {
                ecx = m.read32(esp.wrapping_add(0x14));
                pc = 0x62283; continue;
            }
            0x62283 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(ecx)) as u32)) & 0xff);
                edx = (edx & 0xffff_ff00) | ((((((eax) as u8)) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((m.read8(esi))) as u32) & 0xffu32);
                cf = ((((fa < fb))) as i32);
                if (fa != fb) { pc = 0x622a7; continue; }
                pc = 0x6228b; continue;
            }
            0x6228b => {
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x622a3; continue; }
                pc = 0x6228f; continue;
            }
            0x6228f => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x1))) as u32)) & 0xff);
                edx = (edx & 0xffff_ff00) | ((((((eax) as u8)) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((m.read8(esi.wrapping_add(0x1)))) as u32) & 0xffu32);
                cf = ((((fa < fb))) as i32);
                if (fa != fb) { pc = 0x622a7; continue; }
                pc = 0x62299; continue;
            }
            0x62299 => {
                ecx = ecx.wrapping_add(0x2);
                esi = esi.wrapping_add(0x2);
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x62283; continue; }
                pc = 0x622a3; continue;
            }
            0x622a3 => {
                ecx = 0;
                pc = 0x622ac; continue;
            }
            0x622a7 => {
                ecx = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                { let wide = (ecx as u64).wrapping_sub(0xFFFFFFFFu64).wrapping_sub(cf as u64); cf = ((wide >> 32) & 1) as i32; ecx = wide as u32; }
                pc = 0x622ac; continue;
            }
            0x622ac => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x622c0; continue; }
                pc = 0x622b0; continue;
            }
            0x622b0 => {
                edi = edi.wrapping_add(1);
                ebp = ebp.wrapping_add(0x1c);
                fa = edi;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62278; continue; }
                pc = 0x622b8; continue;
            }
            0x622b8 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = (eax | 0xffffffffu32);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x622c0 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = edi;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x622c4; continue;
            }
            0x622c4 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x622D0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_load_from_path<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x622d0;
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
        { crate::pose::cover(0x622d0, previous, pc); previous = pc; }
        match pc {
            0x622d0 => {
                push!(ecx);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                call!(0x62260);
                pc = 0x622dc; continue;
            }
            0x622dc => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x62370; continue; }
                pc = 0x622e8; continue;
            }
            0x622e8 => {
                push!(ebx);
                push!(0x1);
                push!(esi);
                call!(0x296f0);
                pc = 0x622f1; continue;
            }
            0x622f1 => {
                ebx = 0;
                esi = eax;
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x8), ebx);
                if (fas >= fbs) { pc = 0x62312; continue; }
                pc = 0x62300; continue;
            }
            0x62300 => {
                push!(ebx);
                esi = (esi | 0xffffffffu32);
                call!(0x389b0);
                pc = 0x62309; continue;
            }
            0x62309 => {
                esp = esp.wrapping_add(0x4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62312 => {
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x18));
                fa = ebp;
                fb = ebx;
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x20));
                if (fa == fb) { pc = 0x6233f; continue; }
                pc = 0x62320; continue;
            }
            0x62320 => {
                push!(0);
                push!(0);
                push!(esi);
                call!(0x29030);
                pc = 0x6232a; continue;
            }
            0x6232a => {
                ebx = eax;
                push!(edi);
                push!(ebx);
                push!(0x80);
                call!(0x3e0b0);
                pc = 0x62338; continue;
            }
            0x62338 => {
                esp = esp.wrapping_add(0x18);
                m.write32(esp.wrapping_add(0x10), eax);
                pc = 0x6233f; continue;
            }
            0x6233f => {
                eax = m.read32(esp.wrapping_add(0x18));
                push!(edi);
                push!(eax);
                call!(0x61fd0);
                pc = 0x6234a; continue;
            }
            0x6234a => {
                ecx = m.read32(esp.wrapping_add(0x18));
                push!(ebx);
                push!(ecx);
                push!(edi);
                push!(ebp);
                push!(esi);
                push!(eax);
                call!(0x620e0);
                pc = 0x62359; continue;
            }
            0x62359 => {
                esp = esp.wrapping_add(0x20);
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = eax;
                if (fa == fb) { pc = 0x6236b; continue; }
                pc = 0x62362; continue;
            }
            0x62362 => {
                push!(esi);
                call!(0x29090);
                pc = 0x62368; continue;
            }
            0x62368 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x6236b; continue;
            }
            0x6236b => {
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x62370; continue;
            }
            0x62370 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62380. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_slot_find_entry_by_key_stride_0x10_in_table<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62380;
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
        { crate::pose::cover(0x62380, previous, pc); previous = pc; }
        match pc {
            0x62380 => {
                ecx = m.read32(0x4d19ec);
                edx = edi;
                edx = ((edx as i32).wrapping_mul((0x1c) as i32)) as u32;
                fa = edi;
                fb = ecx;
                eax = m.read32(edx.wrapping_add(0x6d7910));
                if (fa != fb) { pc = 0x62398; continue; }
                pc = 0x62395; continue;
            }
            0x62395 => {
                eax = 0;
                break 'run;
            }
            0x62398 => {
                push!(esi);
                esi = m.read32(edx.wrapping_add(0x6d7908));
                ecx = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x623b1; continue; }
                pc = 0x623a5; continue;
            }
            0x623a5 => {
                fa = m.read32(eax);
                fb = ebx;
                if (fa == fb) { pc = 0x623b5; continue; }
                pc = 0x623a9; continue;
            }
            0x623a9 => {
                eax = eax.wrapping_add(0x10);
                ecx = ecx.wrapping_add(1);
                fa = ecx;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x623a5; continue; }
                pc = 0x623b1; continue;
            }
            0x623b1 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                break 'run;
            }
            0x623b5 => {
                ecx = m.read32(0x4d19f4);
                edx = edx.wrapping_add(0x6d7900);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(0x6d7c50, eax);
                m.write32(0x4d19f8, ecx);
                m.write32(0x4d19f4, edi);
                m.write32(0x6d78ec, edx);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x623E0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_slot_release<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x623e0;
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
        { crate::pose::cover(0x623e0, previous, pc); previous = pc; }
        match pc {
            0x623e0 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                esi = ((esi as i32).wrapping_mul((0x1c) as i32)) as u32;
                eax = m.read32(esi.wrapping_add(0x6d7900));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                push!(edi);
                if (fa == fb) { pc = 0x623fd; continue; }
                pc = 0x623f4; continue;
            }
            0x623f4 => {
                push!(eax);
                call!(0x29090);
                pc = 0x623fa; continue;
            }
            0x623fa => {
                esp = esp.wrapping_add(0x4);
                pc = 0x623fd; continue;
            }
            0x623fd => {
                eax = m.read32(esi.wrapping_add(0x6d7918));
                edi = 0;
                fa = eax;
                fb = edi;
                if (fa != fb) { pc = 0x6244a; continue; }
                pc = 0x62409; continue;
            }
            0x62409 => {
                fa = m.read32(esi.wrapping_add(0x6d790c));
                fb = edi;
                if (fa != fb) { pc = 0x62424; continue; }
                pc = 0x62411; continue;
            }
            0x62411 => {
                eax = m.read32(esi.wrapping_add(0x6d7910));
                fa = eax;
                fb = edi;
                if (fa == fb) { pc = 0x62424; continue; }
                pc = 0x6241b; continue;
            }
            0x6241b => {
                push!(eax);
                call!(0x3e1e0);
                pc = 0x62421; continue;
            }
            0x62421 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x62424; continue;
            }
            0x62424 => {
                eax = m.read32(esi.wrapping_add(0x6d7914));
                fa = eax;
                fb = edi;
                if (fa == fb) { pc = 0x62437; continue; }
                pc = 0x6242e; continue;
            }
            0x6242e => {
                push!(eax);
                call!(0x3e1e0);
                pc = 0x62434; continue;
            }
            0x62434 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x62437; continue;
            }
            0x62437 => {
                eax = m.read32(esi.wrapping_add(0x6d790c));
                fa = eax;
                fb = edi;
                if (fa == fb) { pc = 0x6244a; continue; }
                pc = 0x62441; continue;
            }
            0x62441 => {
                push!(eax);
                call!(0x3e1e0);
                pc = 0x62447; continue;
            }
            0x62447 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x6244a; continue;
            }
            0x6244a => {
                m.write32(esi.wrapping_add(0x6d7914), edi);
                m.write32(esi.wrapping_add(0x6d790c), edi);
                m.write32(esi.wrapping_add(0x6d7908), edi);
                m.write32(esi.wrapping_add(0x6d7910), edi);
                m.write32(0x6d7c50, edi);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esi.wrapping_add(0x6d7918), 0xffffffffu32);
                m.write32(0x4d19f4, 0xffffffffu32);
                m.write32(0x4d19f8, 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x624C0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_entry_data_pointer_by_name_crc32_when_offset_16_aligned_and_field_0xc_zero<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x624c0;
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
        { crate::pose::cover(0x624c0, previous, pc); previous = pc; }
        match pc {
            0x624c0 => {
                eax = m.read32(0x6d7880);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62507; continue; }
                pc = 0x624c9; continue;
            }
            0x624c9 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(ebx);
                push!(edi);
                push!(eax);
                call!(0x150530);
                pc = 0x624d5; continue;
            }
            0x624d5 => {
                edi = m.read32(esp.wrapping_add(0x14));
                esp = esp.wrapping_add(0x4);
                ebx = eax;
                call!(0x62380);
                pc = 0x624e3; continue;
            }
            0x624e3 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x62507; continue; }
                pc = 0x624e9; continue;
            }
            0x624e9 => {
                ecx = m.read32(0x6d78ec);
                ecx = m.read32(ecx.wrapping_add(0xc));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62507; continue; }
                pc = 0x624f6; continue;
            }
            0x624f6 => {
                edx = m.read32(eax.wrapping_add(0xc));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x62507; continue; }
                pc = 0x624fd; continue;
            }
            0x624fd => {
                eax = m.read32(eax.wrapping_add(0x4));
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0xfi32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x62507; continue; }
                pc = 0x62504; continue;
            }
            0x62504 => {
                eax = eax.wrapping_add(ecx);
                break 'run;
            }
            0x62507 => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62510. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_size_via_open_seek_end_close_slot<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62510;
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
        { crate::pose::cover(0x62510, previous, pc); previous = pc; }
        match pc {
            0x62510 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(esi);
                push!(0x1);
                push!(eax);
                call!(0x296f0);
                pc = 0x6251d; continue;
            }
            0x6251d => {
                esi = eax;
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62541; continue; }
                pc = 0x62526; continue;
            }
            0x62526 => {
                push!(edi);
                push!(0);
                push!(0);
                push!(esi);
                call!(0x29030);
                pc = 0x62531; continue;
            }
            0x62531 => {
                push!(esi);
                edi = eax;
                call!(0x29090);
                pc = 0x62539; continue;
            }
            0x62539 => {
                esp = esp.wrapping_add(0x10);
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62541 => {
                eax = (eax | 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62550. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn level_pak_path_prefix_build<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62550;
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
        { crate::pose::cover(0x62550, previous, pc); previous = pc; }
        match pc {
            0x62550 => {
                eax = m.read32(esp.wrapping_add(0x4));
                fa = eax;
                fb = ((((0x64i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(edi);
                m.write8(0x6d78f0, (0) as u8);
                if (fas >= fbs) { pc = 0x625e7; continue; }
                pc = 0x62565; continue;
            }
            0x62565 => {
                push!(eax);
                call!(0x1ceb00);
                pc = 0x6256b; continue;
            }
            0x6256b => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62587; continue; }
                pc = 0x62572; continue;
            }
            0x62572 => {
                edi = 0x6d78f0;
                edi = edi.wrapping_sub(1);
                pc = 0x62578; continue;
            }
            0x62578 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x1))) as u32)) & 0xff);
                edi = edi.wrapping_add(1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x62578; continue; }
                pc = 0x62580; continue;
            }
            0x62580 => {
                eax = m.read32(0x478748);
                pc = 0x625e5; continue;
            }
            0x62587 => {
                fa = ((((m.read8(0x7de458))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x625a8; continue; }
                pc = 0x62590; continue;
            }
            0x62590 => {
                edi = 0x6d78f0;
                edi = edi.wrapping_sub(1);
                pc = 0x62596; continue;
            }
            0x62596 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x1))) as u32)) & 0xff);
                edi = edi.wrapping_add(1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x62596; continue; }
                pc = 0x6259e; continue;
            }
            0x6259e => {
                ecx = m.read32(0x478744);
                m.write32(edi, ecx);
                pc = 0x625e7; continue;
            }
            0x625a8 => {
                fa = ((((m.read8(0x7de455))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x625d2; continue; }
                pc = 0x625b1; continue;
            }
            0x625b1 => {
                fa = m.read32(0x79094c);
                fb = ((((0x64i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x625d2; continue; }
                pc = 0x625ba; continue;
            }
            0x625ba => {
                edi = 0x6d78f0;
                edi = edi.wrapping_sub(1);
                pc = 0x625c0; continue;
            }
            0x625c0 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x1))) as u32)) & 0xff);
                edi = edi.wrapping_add(1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x625c0; continue; }
                pc = 0x625c8; continue;
            }
            0x625c8 => {
                edx = m.read32(0x478740);
                m.write32(edi, edx);
                pc = 0x625e7; continue;
            }
            0x625d2 => {
                edi = 0x6d78f0;
                edi = edi.wrapping_sub(1);
                pc = 0x625d8; continue;
            }
            0x625d8 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x1))) as u32)) & 0xff);
                edi = edi.wrapping_add(1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x625d8; continue; }
                pc = 0x625e0; continue;
            }
            0x625e0 => {
                eax = m.read32(0x47873c);
                pc = 0x625e5; continue;
            }
            0x625e5 => {
                m.write32(edi, eax);
                pc = 0x625e7; continue;
            }
            0x625e7 => {
                fa = m.read32(esp.wrapping_add(0xc));
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x62606; continue; }
                pc = 0x625ee; continue;
            }
            0x625ee => {
                edi = 0x6d78f0;
                edi = edi.wrapping_sub(1);
                pc = 0x625f4; continue;
            }
            0x625f4 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x1))) as u32)) & 0xff);
                edi = edi.wrapping_add(1);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x625f4; continue; }
                pc = 0x625fc; continue;
            }
            0x625fc => {
                ecx = (ecx & 0xffff_0000) | ((((m.read16(0x478738)) as u32)) & 0xffff);
                write16(m, edi, (((((ecx) as u16)) as u32)) as u16);
                pc = 0x62606; continue;
            }
            0x62606 => {
                eax = 0x6d78f0;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62610. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn level_pak_path_format<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62610;
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
        { crate::pose::cover(0x62610, previous, pc); previous = pc; }
        match pc {
            0x62610 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                push!(edi);
                push!(ebx);
                call!(0x1ceb00);
                pc = 0x6261d; continue;
            }
            0x6261d => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x6262b; continue; }
                pc = 0x62624; continue;
            }
            0x62624 => {
                edi = 0x47872c;
                pc = 0x62657; continue;
            }
            0x6262b => {
                fa = ((((m.read8(0x7de458))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x6263b; continue; }
                pc = 0x62634; continue;
            }
            0x62634 => {
                edi = 0x477ff0;
                pc = 0x62657; continue;
            }
            0x6263b => {
                fa = ((((m.read8(0x7de455))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x62652; continue; }
                pc = 0x62644; continue;
            }
            0x62644 => {
                fa = m.read32(0x79094c);
                fb = ((((0x64i32)) as u32) & 0xffffffffu32);
                edi = 0x478090;
                if (fa != fb) { pc = 0x62657; continue; }
                pc = 0x62652; continue;
            }
            0x62652 => {
                edi = 0x478724;
                pc = 0x62657; continue;
            }
            0x62657 => {
                eax = m.read32(esp.wrapping_add(0x14));
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                esi = 0x4766d8;
                if (fa != fb) { pc = 0x6266a; continue; }
                pc = 0x62665; continue;
            }
            0x62665 => {
                esi = 0x478760;
                pc = 0x6266a; continue;
            }
            0x6266a => {
                push!(eax);
                push!(ebx);
                call!(0x62550);
                pc = 0x62671; continue;
            }
            0x62671 => {
                push!(eax);
                push!(ebx);
                push!(esi);
                push!(edi);
                push!(0x47874c);
                push!(0x6d7888);
                call!(0x3c85d4);
                pc = 0x62684; continue;
            }
            0x62684 => {
                esp = esp.wrapping_add(0x20);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x6d7888;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62690. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_open_async_read_and_close_by_path<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62690;
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
        { crate::pose::cover(0x62690, previous, pc); previous = pc; }
        match pc {
            0x62690 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(esi);
                push!(edi);
                push!(0x1);
                push!(eax);
                call!(0x296f0);
                pc = 0x6269e; continue;
            }
            0x6269e => {
                esi = eax;
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x626b1; continue; }
                pc = 0x626a7; continue;
            }
            0x626a7 => {
                push!(0);
                call!(0x389b0);
                pc = 0x626ae; continue;
            }
            0x626ae => {
                esp = esp.wrapping_add(0x4);
                pc = 0x626b1; continue;
            }
            0x626b1 => {
                ecx = m.read32(esp.wrapping_add(0x14));
                edx = m.read32(esp.wrapping_add(0x10));
                push!(ecx);
                push!(edx);
                push!(esi);
                call!(0x28d50);
                pc = 0x626c1; continue;
            }
            0x626c1 => {
                push!(esi);
                edi = eax;
                call!(0x29090);
                pc = 0x626c9; continue;
            }
            0x626c9 => {
                esp = esp.wrapping_add(0x10);
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x626E0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_slot_count_6d7880_reset_and_4d19ec_clear<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x626e0;
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
        { crate::pose::cover(0x626e0, previous, pc); previous = pc; }
        match pc {
            0x626e0 => {
                m.write32(0x6d7880, 0);
                m.write32(0x4d19ec, 0xffffffffu32);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62700. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_slot_state_6d7c50_4d19f4_4d19f8_reset<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62700;
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
        { crate::pose::cover(0x62700, previous, pc); previous = pc; }
        match pc {
            0x62700 => {
                eax = (eax | 0xffffffffu32);
                m.write32(0x6d7c50, 0);
                m.write32(0x4d19f4, eax);
                m.write32(0x4d19f8, eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62720. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_close_all_and_reset<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62720;
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
        { crate::pose::cover(0x62720, previous, pc); previous = pc; }
        match pc {
            0x62720 => {
                eax = m.read32(0x6d7880);
                push!(esi);
                esi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(edi);
                if (fas <= fbs) { pc = 0x6274e; continue; }
                pc = 0x6272d; continue;
            }
            0x6272d => {
                edi = 0x6d7900;
                pc = 0x62732; continue;
            }
            0x62732 => {
                eax = m.read32(edi);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62741; continue; }
                pc = 0x62738; continue;
            }
            0x62738 => {
                push!(eax);
                call!(0x29090);
                pc = 0x6273e; continue;
            }
            0x6273e => {
                esp = esp.wrapping_add(0x4);
                pc = 0x62741; continue;
            }
            0x62741 => {
                eax = m.read32(0x6d7880);
                esi = esi.wrapping_add(1);
                edi = edi.wrapping_add(0x1c);
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62732; continue; }
                pc = 0x6274e; continue;
            }
            0x6274e => {
                eax = (eax | 0xffffffffu32);
                m.write32(0x4d19f4, eax);
                m.write32(0x4d19f8, eax);
                ecx = 0xd2;
                eax = 0;
                edi = 0x6d7900;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(0x6d7c50, 0);
                m.write32(0x6d7c48, 0);
                m.write32(0x6d7880, 0);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62A90. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_slots_close_open_files_and_reset_cache_entry<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62a90;
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
        { crate::pose::cover(0x62a90, previous, pc); previous = pc; }
        match pc {
            0x62a90 => {
                eax = m.read32(0x6b7aa8);
                push!(edi);
                edi = 0;
                fa = eax;
                fb = edi;
                m.write32(0x6d7c50, edi);
                m.write32(0x4d19f4, 0xffffffffu32);
                m.write32(0x4d19f8, 0xffffffffu32);
                if (fa != fb) { pc = 0x62aed; continue; }
                pc = 0x62ab6; continue;
            }
            0x62ab6 => {
                fa = m.read32(0x6d7880);
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x62aed; continue; }
                pc = 0x62abe; continue;
            }
            0x62abe => {
                push!(esi);
                esi = 0x6d7900;
                pc = 0x62ac4; continue;
            }
            0x62ac4 => {
                fa = m.read32(esi.wrapping_add(0x18));
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62adf; continue; }
                pc = 0x62aca; continue;
            }
            0x62aca => {
                eax = m.read32(esi);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62adf; continue; }
                pc = 0x62ad0; continue;
            }
            0x62ad0 => {
                push!(eax);
                call!(0x29090);
                pc = 0x62ad6; continue;
            }
            0x62ad6 => {
                esp = esp.wrapping_add(0x4);
                m.write32(esi, 0xffffffffu32);
                pc = 0x62adf; continue;
            }
            0x62adf => {
                eax = m.read32(0x6d7880);
                edi = edi.wrapping_add(1);
                esi = esi.wrapping_add(0x1c);
                fa = edi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62ac4; continue; }
                pc = 0x62aec; continue;
            }
            0x62aec => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x62aed; continue;
            }
            0x62aed => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62AF0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_file_info_by_path_with_offset<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62af0;
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
        { crate::pose::cover(0x62af0, previous, pc); previous = pc; }
        match pc {
            0x62af0 => {
                eax = m.read32(0x6d7880);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62b49; continue; }
                pc = 0x62af9; continue;
            }
            0x62af9 => {
                push!(ebx);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                push!(edi);
                push!(esi);
                call!(0x150530);
                pc = 0x62b06; continue;
            }
            0x62b06 => {
                edi = m.read32(esp.wrapping_add(0x1c));
                esp = esp.wrapping_add(0x4);
                ebx = eax;
                call!(0x62380);
                pc = 0x62b14; continue;
            }
            0x62b14 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62b37; continue; }
                pc = 0x62b18; continue;
            }
            0x62b18 => {
                eax = m.read32(eax.wrapping_add(0x4));
                ebx = m.read32(esp.wrapping_add(0x14));
                ecx = m.read32(0x6d78ec);
                edx = m.read32(ecx);
                eax = eax.wrapping_add(ebx);
                push!(eax);
                push!(edx);
                call!(0x29240);
                pc = 0x62b30; continue;
            }
            0x62b30 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62b37 => {
                eax = m.read32(esp.wrapping_add(0x14));
                push!(eax);
                push!(esi);
                call!(0x292c0);
                pc = 0x62b42; continue;
            }
            0x62b42 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62b49 => {
                ecx = m.read32(esp.wrapping_add(0x8));
                edx = m.read32(esp.wrapping_add(0x4));
                push!(ecx);
                push!(edx);
                call!(0x292c0);
                pc = 0x62b58; continue;
            }
            0x62b58 => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62B60. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn thunk_file_async_read_poll_pending_request<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62b60;
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
        { crate::pose::cover(0x62b60, previous, pc); previous = pc; }
        match pc {
            0x62b60 => {
                tail_to!(0x294f0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62B70. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_find_entry_by_crc_key_across_slots<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62b70;
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
        { crate::pose::cover(0x62b70, previous, pc); previous = pc; }
        match pc {
            0x62b70 => {
                push!(ebx);
                push!(edi);
                edi = m.read32(0x4d19f0);
                ebx = eax;
                eax = 0;
                fa = edi;
                fb = ((0xfffffffeu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x62b84; continue; }
                pc = 0x62b81; continue;
            }
            0x62b81 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62b84 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62b90; continue; }
                pc = 0x62b88; continue;
            }
            0x62b88 => {
                call!(0x62380);
                pc = 0x62b8d; continue;
            }
            0x62b8d => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62b90 => {
                ecx = m.read32(0x6d7c50);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62ba3; continue; }
                pc = 0x62b9a; continue;
            }
            0x62b9a => {
                fa = ebx;
                fb = m.read32(ecx);
                if (fa != fb) { pc = 0x62ba3; continue; }
                pc = 0x62b9e; continue;
            }
            0x62b9e => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ecx;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62ba3 => {
                edi = m.read32(0x4d19f4);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(ebp);
                ebp = m.read32(0x6d7880);
                if (fas < fbs) { pc = 0x62bc1; continue; }
                pc = 0x62bb4; continue;
            }
            0x62bb4 => {
                fa = edi;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x62bc1; continue; }
                pc = 0x62bb8; continue;
            }
            0x62bb8 => {
                call!(0x62380);
                pc = 0x62bbd; continue;
            }
            0x62bbd => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x62c28; continue; }
                pc = 0x62bc1; continue;
            }
            0x62bc1 => {
                edi = m.read32(0x4d19f8);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62bde; continue; }
                pc = 0x62bcb; continue;
            }
            0x62bcb => {
                fa = edi;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x62bde; continue; }
                pc = 0x62bcf; continue;
            }
            0x62bcf => {
                call!(0x62380);
                pc = 0x62bd4; continue;
            }
            0x62bd4 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x62c28; continue; }
                pc = 0x62bd8; continue;
            }
            0x62bd8 => {
                edi = m.read32(0x4d19f8);
                pc = 0x62bde; continue;
            }
            0x62bde => {
                push!(esi);
                esi = 0;
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x62c07; continue; }
                pc = 0x62be5; continue;
            }
            0x62be5 => {
                fa = esi;
                fb = m.read32(0x4d19f4);
                if (fa == fb) { pc = 0x62c02; continue; }
                pc = 0x62bed; continue;
            }
            0x62bed => {
                fa = esi;
                fb = edi;
                if (fa == fb) { pc = 0x62c02; continue; }
                pc = 0x62bf1; continue;
            }
            0x62bf1 => {
                edi = esi;
                call!(0x62380);
                pc = 0x62bf8; continue;
            }
            0x62bf8 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x62c27; continue; }
                pc = 0x62bfc; continue;
            }
            0x62bfc => {
                edi = m.read32(0x4d19f8);
                pc = 0x62c02; continue;
            }
            0x62c02 => {
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62be5; continue; }
                pc = 0x62c07; continue;
            }
            0x62c07 => {
                edi = m.read32(0x4d19ec);
                fa = edi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                m.write32(0x4d19ec, 0xffffffffu32);
                if (fa == fb) { pc = 0x62c21; continue; }
                pc = 0x62c1c; continue;
            }
            0x62c1c => {
                call!(0x62380);
                pc = 0x62c21; continue;
            }
            0x62c21 => {
                m.write32(0x4d19ec, edi);
                pc = 0x62c27; continue;
            }
            0x62c27 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x62c28; continue;
            }
            0x62c28 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62C30. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_entry_memory_ptr_by_path<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62c30;
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
        { crate::pose::cover(0x62c30, previous, pc); previous = pc; }
        match pc {
            0x62c30 => {
                eax = m.read32(0x6d7880);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62c9a; continue; }
                pc = 0x62c39; continue;
            }
            0x62c39 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                push!(0x47879c);
                push!(esi);
                call!(0x3c8690);
                pc = 0x62c49; continue;
            }
            0x62c49 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62c53; continue; }
                pc = 0x62c50; continue;
            }
            0x62c50 => {
                esi = eax.wrapping_add(0x6);
                pc = 0x62c53; continue;
            }
            0x62c53 => {
                push!(esi);
                call!(0x150530);
                pc = 0x62c59; continue;
            }
            0x62c59 => {
                esp = esp.wrapping_add(0x4);
                call!(0x62b70);
                pc = 0x62c61; continue;
            }
            0x62c61 => {
                ecx = eax;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62c74; continue; }
                pc = 0x62c67; continue;
            }
            0x62c67 => {
                edx = m.read32(0x6d78ec);
                eax = m.read32(edx.wrapping_add(0xc));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x62c78; continue; }
                pc = 0x62c74; continue;
            }
            0x62c74 => {
                eax = 0;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62c78 => {
                eax = m.read32(ecx.wrapping_add(0xc));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x62c74; continue; }
                pc = 0x62c7f; continue;
            }
            0x62c7f => {
                fa = ((((m.read8(ecx.wrapping_add(0x4)))) as u32) & 0xffu32);
                fb = ((((0xfi32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x62c74; continue; }
                pc = 0x62c85; continue;
            }
            0x62c85 => {
                eax = m.read32(esp.wrapping_add(0xc));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62c92; continue; }
                pc = 0x62c8d; continue;
            }
            0x62c8d => {
                esi = m.read32(ecx.wrapping_add(0x8));
                m.write32(eax, esi);
                pc = 0x62c92; continue;
            }
            0x62c92 => {
                eax = m.read32(edx.wrapping_add(0xc));
                eax = eax.wrapping_add(m.read32(ecx.wrapping_add(0x4)));
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62c9a => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62CA0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn file_size_by_path_pak_or_disk<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62ca0;
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
        { crate::pose::cover(0x62ca0, previous, pc); previous = pc; }
        match pc {
            0x62ca0 => {
                eax = m.read32(0x6d7880);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0xc));
                if (fa == fb) { pc = 0x62cee; continue; }
                pc = 0x62caf; continue;
            }
            0x62caf => {
                push!(0x47879c);
                push!(edi);
                esi = edi;
                call!(0x3c8690);
                pc = 0x62cbc; continue;
            }
            0x62cbc => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62cc6; continue; }
                pc = 0x62cc3; continue;
            }
            0x62cc3 => {
                esi = eax.wrapping_add(0x6);
                pc = 0x62cc6; continue;
            }
            0x62cc6 => {
                push!(esi);
                call!(0x150530);
                pc = 0x62ccc; continue;
            }
            0x62ccc => {
                esp = esp.wrapping_add(0x4);
                call!(0x62b70);
                pc = 0x62cd4; continue;
            }
            0x62cd4 => {
                ecx = eax;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62cee; continue; }
                pc = 0x62cda; continue;
            }
            0x62cda => {
                eax = m.read32(ecx.wrapping_add(0xc));
                edx = m.read32(ecx.wrapping_add(0x8));
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & 0x100);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(edx);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62cee => {
                push!(0x1);
                push!(edi);
                call!(0x296f0);
                pc = 0x62cf6; continue;
            }
            0x62cf6 => {
                esi = eax;
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x62d19; continue; }
                pc = 0x62cff; continue;
            }
            0x62cff => {
                push!(0);
                push!(0);
                push!(esi);
                call!(0x29030);
                pc = 0x62d09; continue;
            }
            0x62d09 => {
                push!(esi);
                edi = eax;
                call!(0x29090);
                pc = 0x62d11; continue;
            }
            0x62d11 => {
                esp = esp.wrapping_add(0x10);
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62d19 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = (eax | 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62D20. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_file_read_by_path_or_disk<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62d20;
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
        { crate::pose::cover(0x62d20, previous, pc); previous = pc; }
        match pc {
            0x62d20 => {
                eax = m.read32(0x6d7880);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                push!(edi);
                if (fa == fb) { pc = 0x62dc7; continue; }
                pc = 0x62d34; continue;
            }
            0x62d34 => {
                push!(0x47879c);
                push!(ebx);
                esi = ebx;
                call!(0x3c8690);
                pc = 0x62d41; continue;
            }
            0x62d41 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62d4b; continue; }
                pc = 0x62d48; continue;
            }
            0x62d48 => {
                esi = eax.wrapping_add(0x6);
                pc = 0x62d4b; continue;
            }
            0x62d4b => {
                push!(esi);
                call!(0x150530);
                pc = 0x62d51; continue;
            }
            0x62d51 => {
                esp = esp.wrapping_add(0x4);
                call!(0x62b70);
                pc = 0x62d59; continue;
            }
            0x62d59 => {
                edi = eax;
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62dc7; continue; }
                pc = 0x62d5f; continue;
            }
            0x62d5f => {
                eax = m.read32(edi.wrapping_add(0x4));
                ecx = m.read32(0x6d78ec);
                edx = m.read32(ecx);
                push!(0x1);
                push!(eax);
                push!(edx);
                call!(0x29030);
                pc = 0x62d73; continue;
            }
            0x62d73 => {
                eax = m.read32(edi.wrapping_add(0xc));
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x62d91; continue; }
                pc = 0x62d7d; continue;
            }
            0x62d7d => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                edx = m.read32(0x6d78ec);
                eax = m.read32(edx);
                m.write32(esp.wrapping_add(0x4), eax);
                tail_to!(0x28d50); break 'run;  // direct tail jump
            }
            0x62d91 => {
                eax = m.read32(edi.wrapping_add(0xc));
                ebx = m.read32(esp.wrapping_add(0x14));
                edx = m.read32(esp.wrapping_add(0x18));
                ecx = m.read32(0x6d78ec);
                esi = ebx;
                esi = esi.wrapping_sub(eax);
                esi = esi.wrapping_add(edx);
                edx = m.read32(ecx);
                push!(eax);
                esi = (esi & 0xffffffe0u32);
                push!(esi);
                push!(edx);
                call!(0x28d50);
                pc = 0x62db5; continue;
            }
            0x62db5 => {
                eax = m.read32(edi.wrapping_add(0xc));
                push!(ebx);
                push!(eax);
                push!(esi);
                call!(0x35de70);
                pc = 0x62dc0; continue;
            }
            0x62dc0 => {
                esp = esp.wrapping_add(0x18);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62dc7 => {
                fa = m.read32(0x7de398);
                fb = ((((0x100000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x62de3; continue; }
                pc = 0x62dd3; continue;
            }
            0x62dd3 => {
                push!(0);
                call!(0x389b0);
                pc = 0x62dda; continue;
            }
            0x62dda => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62de3 => {
                push!(0x1);
                push!(ebx);
                call!(0x296f0);
                pc = 0x62deb; continue;
            }
            0x62deb => {
                esi = eax;
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x62dfe; continue; }
                pc = 0x62df4; continue;
            }
            0x62df4 => {
                push!(0);
                call!(0x389b0);
                pc = 0x62dfb; continue;
            }
            0x62dfb => {
                esp = esp.wrapping_add(0x4);
                pc = 0x62dfe; continue;
            }
            0x62dfe => {
                ecx = m.read32(esp.wrapping_add(0x18));
                edx = m.read32(esp.wrapping_add(0x14));
                push!(ecx);
                push!(edx);
                push!(esi);
                call!(0x28d50);
                pc = 0x62e0e; continue;
            }
            0x62e0e => {
                push!(esi);
                edi = eax;
                call!(0x29090);
                pc = 0x62e16; continue;
            }
            0x62e16 => {
                esp = esp.wrapping_add(0x10);
                eax = edi;
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

/// 0x62E20. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_file_info_at_offset_by_path<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62e20;
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
        { crate::pose::cover(0x62e20, previous, pc); previous = pc; }
        match pc {
            0x62e20 => {
                eax = m.read32(0x6d7880);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62e85; continue; }
                pc = 0x62e29; continue;
            }
            0x62e29 => {
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0xc));
                push!(0x47879c);
                push!(edi);
                esi = edi;
                call!(0x3c8690);
                pc = 0x62e3c; continue;
            }
            0x62e3c => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62e46; continue; }
                pc = 0x62e43; continue;
            }
            0x62e43 => {
                esi = eax.wrapping_add(0x6);
                pc = 0x62e46; continue;
            }
            0x62e46 => {
                push!(esi);
                call!(0x150530);
                pc = 0x62e4c; continue;
            }
            0x62e4c => {
                esp = esp.wrapping_add(0x4);
                call!(0x62b70);
                pc = 0x62e54; continue;
            }
            0x62e54 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62e74; continue; }
                pc = 0x62e58; continue;
            }
            0x62e58 => {
                eax = m.read32(eax.wrapping_add(0x4));
                eax = eax.wrapping_add(m.read32(esp.wrapping_add(0x10)));
                ecx = m.read32(0x6d78ec);
                edx = m.read32(ecx);
                push!(eax);
                push!(edx);
                call!(0x29240);
                pc = 0x62e6e; continue;
            }
            0x62e6e => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62e74 => {
                eax = m.read32(esp.wrapping_add(0x10));
                push!(eax);
                push!(edi);
                call!(0x292c0);
                pc = 0x62e7f; continue;
            }
            0x62e7f => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x62e85 => {
                ecx = m.read32(esp.wrapping_add(0x8));
                edx = m.read32(esp.wrapping_add(0x4));
                push!(ecx);
                push!(edx);
                call!(0x292c0);
                pc = 0x62e94; continue;
            }
            0x62e94 => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62EA0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_entry_physical_sort_key_by_path_crc_after_marker<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62ea0;
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
        { crate::pose::cover(0x62ea0, previous, pc); previous = pc; }
        match pc {
            0x62ea0 => {
                eax = m.read32(0x6d7880);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62ef1; continue; }
                pc = 0x62ea9; continue;
            }
            0x62ea9 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                push!(0x47879c);
                push!(esi);
                call!(0x3c8690);
                pc = 0x62eb9; continue;
            }
            0x62eb9 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62ec3; continue; }
                pc = 0x62ec0; continue;
            }
            0x62ec0 => {
                esi = eax.wrapping_add(0x6);
                pc = 0x62ec3; continue;
            }
            0x62ec3 => {
                push!(esi);
                call!(0x150530);
                pc = 0x62ec9; continue;
            }
            0x62ec9 => {
                esp = esp.wrapping_add(0x4);
                call!(0x62b70);
                pc = 0x62ed1; continue;
            }
            0x62ed1 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x62ef1; continue; }
                pc = 0x62ed6; continue;
            }
            0x62ed6 => {
                ecx = m.read32(0x6d78ec);
                ecx = m.read32(ecx);
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62ef1; continue; }
                pc = 0x62ee3; continue;
            }
            0x62ee3 => {
                edx = m.read32(eax.wrapping_add(0x4));
                push!(edx);
                push!(ecx);
                call!(0x290f0);
                pc = 0x62eed; continue;
            }
            0x62eed => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x62ef1 => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x62F00. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_file_read_into_arena_alloc_by_path_returns_ptr_and_size<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62f00;
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
        { crate::pose::cover(0x62f00, previous, pc); previous = pc; }
        match pc {
            0x62f00 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                push!(edi);
                push!(ebx);
                edi = 0;
                call!(0x62ca0);
                pc = 0x62f0f; continue;
            }
            0x62f0f => {
                esi = eax;
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x62f37; continue; }
                pc = 0x62f18; continue;
            }
            0x62f18 => {
                eax = m.read32(esp.wrapping_add(0x14));
                push!(eax);
                push!(esi);
                push!(0x80);
                call!(0x3e0b0);
                pc = 0x62f28; continue;
            }
            0x62f28 => {
                edi = eax;
                push!(esi);
                push!(edi);
                push!(ebx);
                call!(0x62d20);
                pc = 0x62f32; continue;
            }
            0x62f32 => {
                esp = esp.wrapping_add(0x18);
                esi = eax;
                pc = 0x62f37; continue;
            }
            0x62f37 => {
                eax = m.read32(esp.wrapping_add(0x18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62f41; continue; }
                pc = 0x62f3f; continue;
            }
            0x62f3f => {
                m.write32(eax, esi);
                pc = 0x62f41; continue;
            }
            0x62f41 => {
                eax = edi;
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

/// 0x62F50. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_entry_memory_ptr_or_read_into_arena_alloc_by_path<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x62f50;
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
        { crate::pose::cover(0x62f50, previous, pc); previous = pc; }
        match pc {
            0x62f50 => {
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x10));
                push!(edi);
                edi = m.read32(esp.wrapping_add(0xc));
                push!(ebp);
                push!(edi);
                call!(0x62c30);
                pc = 0x62f61; continue;
            }
            0x62f61 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x62fa5; continue; }
                pc = 0x62f68; continue;
            }
            0x62f68 => {
                push!(ebx);
                push!(esi);
                push!(edi);
                ebx = 0;
                call!(0x62ca0);
                pc = 0x62f72; continue;
            }
            0x62f72 => {
                esi = eax;
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x62f9a; continue; }
                pc = 0x62f7b; continue;
            }
            0x62f7b => {
                eax = m.read32(esp.wrapping_add(0x18));
                push!(eax);
                push!(esi);
                push!(0x80);
                call!(0x3e0b0);
                pc = 0x62f8b; continue;
            }
            0x62f8b => {
                ebx = eax;
                push!(esi);
                push!(ebx);
                push!(edi);
                call!(0x62d20);
                pc = 0x62f95; continue;
            }
            0x62f95 => {
                esp = esp.wrapping_add(0x18);
                esi = eax;
                pc = 0x62f9a; continue;
            }
            0x62f9a => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x62fa1; continue; }
                pc = 0x62f9e; continue;
            }
            0x62f9e => {
                m.write32(ebp, esi);
                pc = 0x62fa1; continue;
            }
            0x62fa1 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebx;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x62fa5; continue;
            }
            0x62fa5 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x64E50. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn preload_all_object_type_loadout_texture_and_fixed_resources<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x64e50;
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
        { crate::pose::cover(0x64e50, previous, pc); previous = pc; }
        match pc {
            0x64e50 => {
                call!(0xbdab0);
                pc = 0x64e55; continue;
            }
            0x64e55 => {
                call!(0x1b6130);
                pc = 0x64e5a; continue;
            }
            0x64e5a => {
                call!(0x22ce60);
                pc = 0x64e5f; continue;
            }
            0x64e5f => {
                call!(0x12c730);
                pc = 0x64e64; continue;
            }
            0x64e64 => {
                tail_to!(0x2fe270); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x663F0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn preload_object_resources_and_raw_texture_paths<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x663f0;
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
        { crate::pose::cover(0x663f0, previous, pc); previous = pc; }
        match pc {
            0x663f0 => {
                push!(esi);
                esi = 0x4d28a8;
                pc = 0x663f6; continue;
            }
            0x663f6 => {
                eax = m.read32(esi);
                push!(eax);
                call!(0xa5e40);
                pc = 0x663fe; continue;
            }
            0x663fe => {
                esi = esi.wrapping_add(0x4);
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = ((((0x4d28b8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x663f6; continue; }
                pc = 0x6640c; continue;
            }
            0x6640c => {
                esi = 0x4d1ff6;
                pc = 0x66411; continue;
            }
            0x66411 => {
                ecx = (((((m.read16(esi) as i16)) as i32)) as u32);
                push!(ecx);
                call!(0x810c0);
                pc = 0x6641a; continue;
            }
            0x6641a => {
                esi = esi.wrapping_add(0x20);
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = ((((0x4d2216i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x66411; continue; }
                pc = 0x66428; continue;
            }
            0x66428 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x6A660. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn table_5149ac_get_field_0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x6a660;
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
        { crate::pose::cover(0x6a660, previous, pc); previous = pc; }
        match pc {
            0x6a660 => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = ((eax as i32).wrapping_mul((0x34) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(0x5149ac));
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x6B2F0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn preload_mode_specific_resources_for_byte_7de455_2_and_7de456_0xa<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x6b2f0;
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
        { crate::pose::cover(0x6b2f0, previous, pc); previous = pc; }
        match pc {
            0x6b2f0 => {
                fa = ((((m.read8(0x7de455))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x6b31e; continue; }
                pc = 0x6b2f9; continue;
            }
            0x6b2f9 => {
                push!(esi);
                esi = 0;
                pc = 0x6b300; continue;
            }
            0x6b300 => {
                push!(esi);
                push!(0x1);
                call!(0xbe860);
                pc = 0x6b308; continue;
            }
            0x6b308 => {
                push!(eax);
                call!(0xdb2c0);
                pc = 0x6b30e; continue;
            }
            0x6b30e => {
                push!(eax);
                call!(0xa5e40);
                pc = 0x6b314; continue;
            }
            0x6b314 => {
                esp = esp.wrapping_add(0x10);
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x6b300; continue; }
                pc = 0x6b31d; continue;
            }
            0x6b31d => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x6b31e; continue;
            }
            0x6b31e => {
                fa = ((((m.read8(0x7de456))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x6b342; continue; }
                pc = 0x6b327; continue;
            }
            0x6b327 => {
                push!(0);
                push!(0x400);
                call!(0xbe860);
                pc = 0x6b333; continue;
            }
            0x6b333 => {
                push!(eax);
                call!(0xdb2c0);
                pc = 0x6b339; continue;
            }
            0x6b339 => {
                push!(eax);
                call!(0xa5e40);
                pc = 0x6b33f; continue;
            }
            0x6b33f => {
                esp = esp.wrapping_add(0x10);
                pc = 0x6b342; continue;
            }
            0x6b342 => {
                fa = ((((m.read8(0x7de455))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x6b350; continue; }
                pc = 0x6b34b; continue;
            }
            0x6b34b => {
                tail_to!(0x164770); break 'run;  // direct tail jump
            }
            0x6b350 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x83620. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn level_xbr_chunk_unload_free_textures_and_reset_listed_rooms<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x83620;
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
        { crate::pose::cover(0x83620, previous, pc); previous = pc; }
        match pc {
            0x83620 => {
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xc));
                eax = m.read32(ebp);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebx = m.read32(ebp.wrapping_add(0x4));
                push!(esi);
                push!(edi);
                if (fa == fb) { pc = 0x83637; continue; }
                pc = 0x83632; continue;
            }
            0x83632 => {
                esi = eax.wrapping_add(ebp);
                pc = 0x83639; continue;
            }
            0x83637 => {
                esi = 0;
                pc = 0x83639; continue;
            }
            0x83639 => {
                eax = ebp.wrapping_add(0x8);
                push!(eax);
                call!(0x192a0);
                pc = 0x83642; continue;
            }
            0x83642 => {
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x83688; continue; }
                pc = 0x83649; continue;
            }
            0x83649 => {
                fa = m.read32(esi);
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x83683; continue; }
                pc = 0x8364e; continue;
            }
            0x8364e => {
                eax = esi;
                edi = 0;
                pc = 0x83652; continue;
            }
            0x83652 => {
                eax = m.read32(eax);
                fa = eax;
                fb = ((((0x40000000i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x83673; continue; }
                pc = 0x8365b; continue;
            }
            0x8365b => {
                ecx = m.read32(0x708780);
                fa = m.read32(edi.wrapping_add(ecx));
                fb = ((((0x40000000i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x83673; continue; }
                pc = 0x8366a; continue;
            }
            0x8366a => {
                push!(eax);
                call!(0x18ce0);
                pc = 0x83670; continue;
            }
            0x83670 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x83673; continue;
            }
            0x83673 => {
                ecx = m.read32(esi.wrapping_add(0x10));
                esi = esi.wrapping_add(0x10);
                edi = edi.wrapping_add(0x10);
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                eax = esi;
                if (fa != fb) { pc = 0x83652; continue; }
                pc = 0x83683; continue;
            }
            0x83683 => {
                call!(0x389b0);
                pc = 0x83688; continue;
            }
            0x83688 => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x836b4; continue; }
                pc = 0x8368c; continue;
            }
            0x8368c => {
                edi = ebp.wrapping_add(0x18);
                ebp = 0xfffffffeu32;
                pc = 0x83694; continue;
            }
            0x83694 => {
                esi = m.read32(edi);
                eax = esi;
                call!(0x83530);
                pc = 0x8369d; continue;
            }
            0x8369d => {
                edx = m.read32(0x7b9784);
                esi = m.read32(edx.wrapping_add(esi.wrapping_mul(0x4)));
                edx = m.read32(esi.wrapping_add(0x28));
                edx = (edx & ebp);
                edi = edi.wrapping_add(0x8);
                ebx = ebx.wrapping_sub(1);
                fa = ebx;
                m.write32(esi.wrapping_add(0x28), edx);
                if ((fa != 0)) { pc = 0x83694; continue; }
                pc = 0x836b4; continue;
            }
            0x836b4 => {
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

/// 0x83710. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_or_disk_file_size_fits_limit<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x83710;
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
        { crate::pose::cover(0x83710, previous, pc); previous = pc; }
        match pc {
            0x83710 => {
                push!(ecx);
                push!(esi);
                push!(ebx);
                call!(0x62ca0);
                pc = 0x83718; continue;
            }
            0x83718 => {
                esi = eax;
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x8377e; continue; }
                pc = 0x83721; continue;
            }
            0x83721 => {
                call!(0x3f030);
                pc = 0x83726; continue;
            }
            0x83726 => {
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x8377e; continue; }
                pc = 0x8372a; continue;
            }
            0x8372a => {
                push!(edi);
                eax = esp.wrapping_add(0x8);
                push!(0x836c0);
                push!(eax);
                call!(0x3ef30);
                pc = 0x8373a; continue;
            }
            0x8373a => {
                edx = m.read32(esp.wrapping_add(0x10));
                esp = esp.wrapping_add(0xc);
                eax = eax.wrapping_add(1);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & edx);
                fa = eax;
                m.write32(0x7087b4, eax);
                if ((fa == 0)) { pc = 0x8377e; continue; }
                pc = 0x8374f; continue;
            }
            0x8374f => {
                ecx = m.read32(0x7087bc);
                push!(ebp);
                m.write32(ecx.wrapping_add(edi.wrapping_mul(0x8)), eax);
                edx = m.read32(0x7497b0);
                ebp = m.read32(0x4e7944);
                push!(esi);
                push!(eax);
                edx = edx.wrapping_add(ebp.wrapping_mul(0x2));
                push!(ebx);
                m.write32(0x7087b0, esi);
                m.write32(ecx.wrapping_add(edi.wrapping_mul(0x8)).wrapping_add(0x4), edx);
                call!(0x83080);
                pc = 0x8377a; continue;
            }
            0x8377a => {
                esp = esp.wrapping_add(0xc);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x8377e; continue;
            }
            0x8377e => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x83860. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn level_xbr_release_async_request_and_clear_room_slots<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x83860;
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
        { crate::pose::cover(0x83860, previous, pc); previous = pc; }
        match pc {
            0x83860 => {
                eax = m.read32(0x7b9778);
                push!(ebx);
                ebx = 0;
                fa = eax;
                fb = ebx;
                if (fa == fb) { pc = 0x83908; continue; }
                pc = 0x83870; continue;
            }
            0x83870 => {
                eax = m.read32(0x4d96fc);
                push!(ebp);
                push!(eax);
                call!(0x29400);
                pc = 0x8387c; continue;
            }
            0x8387c => {
                eax = m.read32(0x7b975c);
                esp = esp.wrapping_add(0x4);
                ebp = 0;
                fa = eax;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(0x7087a8, ebx);
                if (fas <= fbs) { pc = 0x838e7; continue; }
                pc = 0x83890; continue;
            }
            0x83890 => {
                push!(esi);
                push!(edi);
                edi = 0xd8c0;
                pc = 0x838a0; continue;
            }
            0x838a0 => {
                ecx = m.read32(0x7087a4);
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(ecx))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                esi = edi.wrapping_add(ecx);
                if ((fa & fb) == 0) { pc = 0x838ca; continue; }
                pc = 0x838b0; continue;
            }
            0x838b0 => {
                edx = m.read32(0x708788);
                eax = m.read32(ebx.wrapping_add(edx).wrapping_add(0xb4));
                push!(0);
                push!(eax);
                eax = esi;
                call!(0x832a0);
                pc = 0x838c7; continue;
            }
            0x838c7 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x838ca; continue;
            }
            0x838ca => {
                m.write32(esi, 0);
                eax = m.read32(0x7b975c);
                ebp = ebp.wrapping_add(1);
                edi = edi.wrapping_add(0x18);
                ebx = ebx.wrapping_add(0xb4);
                fa = ebp;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x838a0; continue; }
                pc = 0x838e3; continue;
            }
            0x838e3 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = 0;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x838e7; continue;
            }
            0x838e7 => {
                eax = m.read32(0x703834);
                m.write32(eax, 0x1);
                m.write32(eax.wrapping_add(0x4), ebx);
                m.write32(eax.wrapping_add(0x8), ebx);
                m.write32(eax.wrapping_add(0xc), 0x500000);
                m.write32(0x703838, eax);
                m.write32(eax.wrapping_add(0x10), ebx);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x83908; continue;
            }
            0x83908 => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x85B80. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn level_xbr_load_85b80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x85b80;
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
        { crate::pose::cover(0x85b80, previous, pc); previous = pc; }
        match pc {
            0x85b80 => {
                eax = m.read32(0x79094c);
                esp = esp.wrapping_sub(0x40);
                fa = eax;
                fb = ((((0x64i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x85cb5; continue; }
                pc = 0x85b91; continue;
            }
            0x85b91 => {
                fa = eax;
                fb = ((((0x65i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x85cb5; continue; }
                pc = 0x85b9a; continue;
            }
            0x85b9a => {
                fa = eax;
                fb = ((((0x66i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x85cb5; continue; }
                pc = 0x85ba3; continue;
            }
            0x85ba3 => {
                fa = eax;
                fb = ((((0x67i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x85cb5; continue; }
                pc = 0x85bac; continue;
            }
            0x85bac => {
                fa = eax;
                fb = ((((0x68i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x85cb5; continue; }
                pc = 0x85bb5; continue;
            }
            0x85bb5 => {
                fa = eax;
                fb = ((((0x69i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x85cb5; continue; }
                pc = 0x85bbe; continue;
            }
            0x85bbe => {
                fa = eax;
                fb = ((((0x6bi32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x85cb5; continue; }
                pc = 0x85bc7; continue;
            }
            0x85bc7 => {
                fa = eax;
                fb = ((((0x6ai32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x85cb5; continue; }
                pc = 0x85bd0; continue;
            }
            0x85bd0 => {
                push!(edi);
                push!(eax);
                push!(eax);
                push!(eax);
                eax = esp.wrapping_add(0x10);
                push!(0x47d328);
                push!(eax);
                call!(0x3c85d4);
                pc = 0x85be3; continue;
            }
            0x85be3 => {
                ecx = esp.wrapping_add(0x18);
                push!(ecx);
                call!(0x630c0);
                pc = 0x85bed; continue;
            }
            0x85bed => {
                edi = m.read32(eax.wrapping_add(0x10));
                eax = m.read32(edi.wrapping_add(0x1c));
                esp = esp.wrapping_add(0x18);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x85c04; continue; }
                pc = 0x85bfa; continue;
            }
            0x85bfa => {
                m.write32(0x7b9778, 0x1);
                pc = 0x85c04; continue;
            }
            0x85c04 => {
                eax = m.read32(edi);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x85c0f; continue; }
                pc = 0x85c0a; continue;
            }
            0x85c0a => {
                ecx = eax.wrapping_add(edi);
                pc = 0x85c11; continue;
            }
            0x85c0f => {
                ecx = 0;
                pc = 0x85c11; continue;
            }
            0x85c11 => {
                eax = m.read32(edi.wrapping_add(0x28));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(0x708780, ecx);
                if (fa == fb) { pc = 0x85c22; continue; }
                pc = 0x85c1e; continue;
            }
            0x85c1e => {
                eax = eax.wrapping_add(edi);
                pc = 0x85c24; continue;
            }
            0x85c22 => {
                eax = 0;
                pc = 0x85c24; continue;
            }
            0x85c24 => {
                push!(ecx);
                push!(edi);
                push!(eax);
                m.write32(0x708784, eax);
                call!(0x8dd10);
                pc = 0x85c31; continue;
            }
            0x85c31 => {
                eax = m.read32(0x708780);
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x85ca4; continue; }
                pc = 0x85c3d; continue;
            }
            0x85c3d => {
                push!(edi);
                push!(eax);
                call!(0x8e3e0);
                pc = 0x85c44; continue;
            }
            0x85c44 => {
                eax = m.read32(0x708780);
                ecx = m.read32(eax);
                esp = esp.wrapping_add(0x8);
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x85ca4; continue; }
                pc = 0x85c53; continue;
            }
            0x85c53 => {
                push!(esi);
                esi = 0;
                pc = 0x85c60; continue;
            }
            0x85c60 => {
                ecx = m.read32(eax.wrapping_add(esi));
                eax = eax.wrapping_add(esi);
                fa = ecx;
                fb = ((((0x40000000i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x85c92; continue; }
                pc = 0x85c6d; continue;
            }
            0x85c6d => {
                edx = m.read32(eax.wrapping_add(0xc));
                fa = edx;
                fb = ((((0x200000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x85c92; continue; }
                pc = 0x85c78; continue;
            }
            0x85c78 => {
                fa = edx;
                fb = ((((0x10000000i32)) as u32) & 0xffffffffu32);
                push!(eax);
                push!(eax);
                push!(ecx);
                if ((fa & fb) == 0) { pc = 0x85c8a; continue; }
                pc = 0x85c83; continue;
            }
            0x85c83 => {
                call!(0x18a30);
                pc = 0x85c88; continue;
            }
            0x85c88 => {
                pc = 0x85c8f; continue;
            }
            0x85c8a => {
                call!(0x637e0);
                pc = 0x85c8f; continue;
            }
            0x85c8f => {
                esp = esp.wrapping_add(0xc);
                pc = 0x85c92; continue;
            }
            0x85c92 => {
                eax = m.read32(0x708780);
                ecx = m.read32(esi.wrapping_add(eax).wrapping_add(0x10));
                esi = esi.wrapping_add(0x10);
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x85c60; continue; }
                pc = 0x85ca3; continue;
            }
            0x85ca3 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x85ca4; continue;
            }
            0x85ca4 => {
                edx = m.read32(edi.wrapping_add(0x18));
                eax = m.read32(edi.wrapping_add(0x14));
                push!(edx);
                push!(eax);
                call!(0x17600);
                pc = 0x85cb1; continue;
            }
            0x85cb1 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x85cb5; continue;
            }
            0x85cb5 => {
                esp = esp.wrapping_add(0x40);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x8BC10. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn level_xbr_load_8bc10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x8bc10;
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
        { crate::pose::cover(0x8bc10, previous, pc); previous = pc; }
        match pc {
            0x8bc10 => {
                esp = esp.wrapping_sub(0x40);
                push!(esi);
                esi = 0;
                m.write32(0x70877c, esi);
                m.write32(0x708788, esi);
                m.write32(0x70878c, esi);
                m.write32(0x708790, esi);
                m.write32(0x7ac87c, esi);
                m.write32(0x70879c, esi);
                m.write32(0x7b9778, esi);
                call!(0x1cf630);
                pc = 0x8bc45; continue;
            }
            0x8bc45 => {
                xmm0 = Xmm::scalar(m.read32(0x475d5c));
                eax = m.read32(0x79094c);
                fa = eax;
                fb = ((((0x64i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(0x7b9760, xmm0.lanes[0]);
                m.write32(0x7b9764, xmm0.lanes[0]);
                m.write32(0x7b9768, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x478618));
                m.write32(0x7b976c, xmm0.lanes[0]);
                m.write32(0x7b9770, xmm0.lanes[0]);
                m.write32(0x7b9774, xmm0.lanes[0]);
                if (fa != fb) { pc = 0x8bcb0; continue; }
                pc = 0x8bc8f; continue;
            }
            0x8bc8f => {
                push!(0x4766d8);
                call!(0x3c230);
                pc = 0x8bc99; continue;
            }
            0x8bc99 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x8bca9; continue; }
                pc = 0x8bca0; continue;
            }
            0x8bca0 => {
                push!(esi);
                call!(0x389b0);
                pc = 0x8bca6; continue;
            }
            0x8bca6 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x8bca9; continue;
            }
            0x8bca9 => {
                call!(0x8a780);
                pc = 0x8bcae; continue;
            }
            0x8bcae => {
                pc = 0x8bcd1; continue;
            }
            0x8bcb0 => {
                if (fas > fbs) { pc = 0x8bcd1; continue; }
                pc = 0x8bcb2; continue;
            }
            0x8bcb2 => {
                push!(eax);
                push!(eax);
                push!(eax);
                eax = esp.wrapping_add(0x10);
                push!(0x47d328);
                push!(eax);
                call!(0x3c85d4);
                pc = 0x8bcc4; continue;
            }
            0x8bcc4 => {
                ecx = esp.wrapping_add(0x18);
                push!(ecx);
                call!(0x8a330);
                pc = 0x8bcce; continue;
            }
            0x8bcce => {
                esp = esp.wrapping_add(0x18);
                pc = 0x8bcd1; continue;
            }
            0x8bcd1 => {
                fa = m.read32(0x7087c8);
                fb = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x8bcea; continue; }
                pc = 0x8bcda; continue;
            }
            0x8bcda => {
                call!(0x834d0);
                pc = 0x8bcdf; continue;
            }
            0x8bcdf => {
                push!(eax);
                push!(0x6);
                call!(0x3ef10);
                pc = 0x8bce7; continue;
            }
            0x8bce7 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x8bcea; continue;
            }
            0x8bcea => {
                call!(0x83910);
                pc = 0x8bcef; continue;
            }
            0x8bcef => {
                call!(0x86130);
                pc = 0x8bcf4; continue;
            }
            0x8bcf4 => {
                call!(0x86fc0);
                pc = 0x8bcf9; continue;
            }
            0x8bcf9 => {
                call!(0x89700);
                pc = 0x8bcfe; continue;
            }
            0x8bcfe => {
                esp = esp.wrapping_add(0x40);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x97FB0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn tail_call_257b30_when_global_7de304_is_zero<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x97fb0;
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
        { crate::pose::cover(0x97fb0, previous, pc); previous = pc; }
        match pc {
            0x97fb0 => {
                eax = m.read32(0x7de304);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x97fbe; continue; }
                pc = 0x97fb9; continue;
            }
            0x97fb9 => {
                tail_to!(0x257b30); break 'run;  // direct tail jump
            }
            0x97fbe => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x9DEB0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn load_level_point_link_data_and_rebuild_group_tables<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x9deb0;
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
        { crate::pose::cover(0x9deb0, previous, pc); previous = pc; }
        match pc {
            0x9deb0 => {
                esp = esp.wrapping_sub(0x8);
                eax = m.read32(0x79094c);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                push!(eax);
                call!(0x6a850);
                pc = 0x9dec2; continue;
            }
            0x9dec2 => {
                m.write32(esp.wrapping_add(0x18), eax);
                eax = m.read32(0x79094c);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0x64i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x9df6b; continue; }
                pc = 0x9ded7; continue;
            }
            0x9ded7 => {
                edi = 0;
                m.write32(esp.wrapping_add(0x10), edi);
                call!(0x9dc70);
                pc = 0x9dee2; continue;
            }
            0x9dee2 => {
                fa = m.read32(0x7b666c);
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                esi = m.read32(0x715f2c);
                if (fas <= fbs) { pc = 0x9df42; continue; }
                pc = 0x9def0; continue;
            }
            0x9def0 => {
                ebp = m.read32(0x7b6660);
                pc = 0x9def6; continue;
            }
            0x9def6 => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x9df38; continue; }
                pc = 0x9defa; continue;
            }
            0x9defa => {
                eax = m.read32(0x715f30);
                eax = eax.wrapping_add(0x8);
                ebx = ebp;
                pc = 0x9df04; continue;
            }
            0x9df04 => {
                fa = ((m.read32(eax.wrapping_add(((0x8i32.wrapping_neg()) as u32)))) & 0xffffffffu32);
                fb = ((((0x10000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x9df32; continue; }
                pc = 0x9df0d; continue;
            }
            0x9df0d => {
                ecx = m.read32(eax.wrapping_add(((0x4i32.wrapping_neg()) as u32)));
                edx = m.read32(eax);
                ecx = ecx.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (((((m.read16(ecx.wrapping_add(esi).wrapping_add(0x8)) as i16)) as i32)) as u32);
                edx = edx.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (((((m.read16(edx.wrapping_add(esi).wrapping_add(0x8)) as i16)) as i32)) as u32);
                fa = ecx;
                fb = edx;
                if (fa == fb) { pc = 0x9df32; continue; }
                pc = 0x9df26; continue;
            }
            0x9df26 => {
                fa = ecx;
                fb = edi;
                if (fa == fb) { pc = 0x9df2e; continue; }
                pc = 0x9df2a; continue;
            }
            0x9df2a => {
                fa = edx;
                fb = edi;
                if (fa != fb) { pc = 0x9df32; continue; }
                pc = 0x9df2e; continue;
            }
            0x9df2e => {
                m.write32(esp.wrapping_add(0x10), m.read32(esp.wrapping_add(0x10)).wrapping_add(0x1));
                pc = 0x9df32; continue;
            }
            0x9df32 => {
                eax = eax.wrapping_add(0x14);
                ebx = ebx.wrapping_sub(1);
                fa = ebx;
                if ((fa != 0)) { pc = 0x9df04; continue; }
                pc = 0x9df38; continue;
            }
            0x9df38 => {
                eax = m.read32(0x7b666c);
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x9def6; continue; }
                pc = 0x9df42; continue;
            }
            0x9df42 => {
                eax = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(0x7b65bc);
                edi = m.read32(0x7b6778);
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_sub(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                eax = eax.wrapping_sub(ecx);
                edi = edi.wrapping_sub(eax);
                ecx = ecx.wrapping_add(eax);
                m.write32(0x7b6778, edi);
                m.write32(0x7b65bc, ecx);
                pc = 0x9df8e; continue;
            }
            0x9df6b => {
                push!(eax);
                push!(0x47d3f0);
                push!(0x712000);
                call!(0x3c85d4);
                pc = 0x9df7b; continue;
            }
            0x9df7b => {
                esp = esp.wrapping_add(0xc);
                eax = 0x712000;
                call!(0x9ce00);
                pc = 0x9df88; continue;
            }
            0x9df88 => {
                esi = m.read32(0x715f2c);
                pc = 0x9df8e; continue;
            }
            0x9df8e => {
                eax = m.read32(0x7b6664);
                edi = 0;
                fa = eax;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x9e24b; continue; }
                pc = 0x9df9d; continue;
            }
            0x9df9d => {
                ecx = 0;
                fa = eax;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(0x715f28, edi);
                m.write32(0x7b666c, ecx);
                if (fas <= fbs) { pc = 0x9dfca; continue; }
                pc = 0x9dfaf; continue;
            }
            0x9dfaf => {
                edx = esi.wrapping_add(0x8);
                esi = eax;
                pc = 0x9dfb4; continue;
            }
            0x9dfb4 => {
                eax = (((((m.read16(edx) as i16)) as i32)) as u32);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x9dfc4; continue; }
                pc = 0x9dfbb; continue;
            }
            0x9dfbb => {
                ecx = eax.wrapping_add(0x1);
                m.write32(0x7b666c, ecx);
                pc = 0x9dfc4; continue;
            }
            0x9dfc4 => {
                edx = edx.wrapping_add(0x20);
                esi = esi.wrapping_sub(1);
                fa = esi;
                if ((fa != 0)) { pc = 0x9dfb4; continue; }
                pc = 0x9dfca; continue;
            }
            0x9dfca => {
                eax = m.read32(0x7b65bc);
                ebp = m.read32(0x7b6778);
                edx = eax.wrapping_add(ecx.wrapping_mul(0x2));
                eax = m.read32(0x7b6674);
                edx = edx.wrapping_add(ecx);
                eax = eax.wrapping_add(edx.wrapping_mul(0x2));
                eax = eax.wrapping_add(ebp);
                eax = eax.wrapping_add(ecx);
                eax = eax.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(0xf);
                eax = (eax & 0xfffffff0u32);
                push!(0x1);
                push!(eax);
                call!(0x3dfa0);
                pc = 0x9dff7; continue;
            }
            0x9dff7 => {
                edx = m.read32(0x7b666c);
                esp = esp.wrapping_add(0x8);
                fa = edx;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(0x7b6770, eax);
                ecx = eax.wrapping_add(edx.wrapping_mul(0x4));
                m.write32(esp.wrapping_add(0x10), edi);
                if (fas <= fbs) { pc = 0x9e200; continue; }
                pc = 0x9e014; continue;
            }
            0x9e014 => {
                xmm1 = Xmm::default();
                xmm2 = Xmm::scalar(m.read32(0x475c78));
                pc = 0x9e030; continue;
            }
            0x9e021 => {
                eax = m.read32(0x7b6770);
                edi = m.read32(esp.wrapping_add(0x10));
                pc = 0x9e030; continue;
            }
            0x9e030 => {
                m.write32(eax.wrapping_add(edi.wrapping_mul(0x4)), ecx);
                eax = 0;
                m.write32(ecx, eax);
                m.write32(ecx.wrapping_add(0x4), eax);
                m.write32(ecx.wrapping_add(0x8), eax);
                m.write32(ecx.wrapping_add(0xc), xmm1.lanes[0]);
                m.write32(ecx.wrapping_add(0x10), xmm1.lanes[0]);
                m.write32(ecx.wrapping_add(0x14), xmm1.lanes[0]);
                esi = m.read32(0x7b6664);
                edx = 0;
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x9e0ce; continue; }
                pc = 0x9e058; continue;
            }
            0x9e058 => {
                esi = m.read32(0x715f2c);
                edi = edi;
                pc = 0x9e060; continue;
            }
            0x9e060 => {
                fa = ((((m.read8(eax.wrapping_add(esi).wrapping_add(0x4)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x9e0c0; continue; }
                pc = 0x9e067; continue;
            }
            0x9e067 => {
                ebx = (((((m.read16(eax.wrapping_add(esi).wrapping_add(0x8)) as i16)) as i32)) as u32);
                fa = ebx;
                fb = edi;
                if (fa != fb) { pc = 0x9e0c0; continue; }
                pc = 0x9e070; continue;
            }
            0x9e070 => {
                esi = m.read32(ecx);
                m.write32(ecx.wrapping_add(esi.wrapping_mul(0x4)).wrapping_add(0x18), edx);
                m.write32(ecx, m.read32(ecx).wrapping_add(0x1));
                esi = m.read32(0x715f2c);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(esi).wrapping_add(0xc)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0xc)))).to_bits();
                m.write32(ecx.wrapping_add(0xc), xmm0.lanes[0]);
                esi = m.read32(0x715f2c);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(esi).wrapping_add(0x10)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x10)))).to_bits();
                m.write32(ecx.wrapping_add(0x10), xmm0.lanes[0]);
                esi = m.read32(0x715f2c);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(esi).wrapping_add(0x14)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x14)))).to_bits();
                m.write32(ecx.wrapping_add(0x14), xmm0.lanes[0]);
                esi = m.read32(0x715f2c);
                pc = 0x9e0c0; continue;
            }
            0x9e0c0 => {
                ebx = m.read32(0x7b6664);
                edx = edx.wrapping_add(1);
                eax = eax.wrapping_add(0x20);
                fa = edx;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x9e060; continue; }
                pc = 0x9e0ce; continue;
            }
            0x9e0ce => {
                xmm3.lanes[0] = (((m.read32(ecx)) as i32) as f32).to_bits();
                xmm0 = xmm2;
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(ecx.wrapping_add(0xc)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(ecx.wrapping_add(0xc), xmm3.lanes[0]);
                xmm3 = Xmm::scalar(m.read32(ecx.wrapping_add(0x10)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(ecx.wrapping_add(0x10), xmm3.lanes[0]);
                xmm3 = Xmm::scalar(m.read32(ecx.wrapping_add(0x14)));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(ecx.wrapping_add(0x14), xmm3.lanes[0]);
                esi = m.read32(0x7b6660);
                ebp = m.read32(0x715f2c);
                eax = m.read32(0x715f30);
                ebx = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x9e178; continue; }
                pc = 0x9e11a; continue;
            }
            0x9e11a => {
                edx = 0;
                pc = 0x9e120; continue;
            }
            0x9e120 => {
                fa = m.read32(edx.wrapping_add(eax));
                fb = ((((0x10000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x9e16a; continue; }
                pc = 0x9e129; continue;
            }
            0x9e129 => {
                esi = m.read32(edx.wrapping_add(eax).wrapping_add(0x4));
                edi = m.read32(edx.wrapping_add(eax).wrapping_add(0x8));
                esi = esi.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = (((((m.read16(esi.wrapping_add(ebp).wrapping_add(0x8)) as i16)) as i32)) as u32);
                edi = edi.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                edi = (((((m.read16(edi.wrapping_add(ebp).wrapping_add(0x8)) as i16)) as i32)) as u32);
                fa = esi;
                fb = edi;
                if (fa == fb) { pc = 0x9e16a; continue; }
                pc = 0x9e145; continue;
            }
            0x9e145 => {
                fa = esi;
                fb = m.read32(esp.wrapping_add(0x10));
                if (fa == fb) { pc = 0x9e151; continue; }
                pc = 0x9e14b; continue;
            }
            0x9e14b => {
                fa = edi;
                fb = m.read32(esp.wrapping_add(0x10));
                if (fa != fb) { pc = 0x9e16a; continue; }
                pc = 0x9e151; continue;
            }
            0x9e151 => {
                esi = m.read32(ecx.wrapping_add(0x4));
                eax = m.read32(ecx);
                eax = eax.wrapping_add(esi);
                m.write32(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x18), ebx);
                m.write32(ecx.wrapping_add(0x4), m.read32(ecx.wrapping_add(0x4)).wrapping_add(0x1));
                eax = m.read32(0x715f30);
                ebp = m.read32(0x715f2c);
                pc = 0x9e16a; continue;
            }
            0x9e16a => {
                esi = m.read32(0x7b6660);
                ebx = ebx.wrapping_add(1);
                edx = edx.wrapping_add(0x14);
                fa = ebx;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x9e120; continue; }
                pc = 0x9e178; continue;
            }
            0x9e178 => {
                edi = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x9e1d9; continue; }
                pc = 0x9e17e; continue;
            }
            0x9e17e => {
                edx = 0;
                pc = 0x9e180; continue;
            }
            0x9e180 => {
                fa = m.read32(edx.wrapping_add(eax));
                fb = ((((0x10000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x9e1cb; continue; }
                pc = 0x9e189; continue;
            }
            0x9e189 => {
                esi = m.read32(edx.wrapping_add(eax).wrapping_add(0x4));
                ebx = m.read32(esp.wrapping_add(0x10));
                esi = esi.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = (((((m.read16(esi.wrapping_add(ebp).wrapping_add(0x8)) as i16)) as i32)) as u32);
                fa = esi;
                fb = ebx;
                if (fa != fb) { pc = 0x9e1cb; continue; }
                pc = 0x9e19d; continue;
            }
            0x9e19d => {
                ebx = m.read32(edx.wrapping_add(eax).wrapping_add(0x8));
                ebx = ebx.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                ebx = (((((m.read16(ebx.wrapping_add(ebp).wrapping_add(0x8)) as i16)) as i32)) as u32);
                fa = esi;
                fb = ebx;
                if (fa != fb) { pc = 0x9e1cb; continue; }
                pc = 0x9e1ad; continue;
            }
            0x9e1ad => {
                eax = m.read32(ecx);
                ebx = m.read32(ecx.wrapping_add(0x4));
                esi = m.read32(ecx.wrapping_add(0x8));
                eax = eax.wrapping_add(ebx);
                eax = eax.wrapping_add(esi);
                m.write32(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x18), edi);
                m.write32(ecx.wrapping_add(0x8), m.read32(ecx.wrapping_add(0x8)).wrapping_add(0x1));
                eax = m.read32(0x715f30);
                ebp = m.read32(0x715f2c);
                pc = 0x9e1cb; continue;
            }
            0x9e1cb => {
                esi = m.read32(0x7b6660);
                edi = edi.wrapping_add(1);
                edx = edx.wrapping_add(0x14);
                fa = edi;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x9e180; continue; }
                pc = 0x9e1d9; continue;
            }
            0x9e1d9 => {
                edx = m.read32(ecx);
                ebx = m.read32(ecx.wrapping_add(0x8));
                edi = m.read32(ecx.wrapping_add(0x4));
                eax = m.read32(esp.wrapping_add(0x10));
                edx = edx.wrapping_add(ebx);
                edx = edx.wrapping_add(edi);
                ecx = ecx.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x18);
                edx = m.read32(0x7b666c);
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), eax);
                if (fas < fbs) { pc = 0x9e021; continue; }
                pc = 0x9e200; continue;
            }
            0x9e200 => {
                call!(0x9af90);
                pc = 0x9e205; continue;
            }
            0x9e205 => {
                eax = m.read32(esp.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(0x715f4c, 0);
                if (fa == fb) { pc = 0x9e23a; continue; }
                pc = 0x9e217; continue;
            }
            0x9e217 => {
                eax = m.read32(eax.wrapping_add(0x68));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x9e23a; continue; }
                pc = 0x9e21e; continue;
            }
            0x9e21e => {
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = eax.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(0xf);
                eax = (eax & 0xfffffff0u32);
                push!(0x1);
                push!(eax);
                call!(0x3dfa0);
                pc = 0x9e232; continue;
            }
            0x9e232 => {
                esp = esp.wrapping_add(0x8);
                m.write32(0x715f4c, eax);
                pc = 0x9e23a; continue;
            }
            0x9e23a => {
                call!(0x9dd60);
                pc = 0x9e23f; continue;
            }
            0x9e23f => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                tail_to!(0x389b0); break 'run;  // direct tail jump
            }
            0x9e24b => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x11D3F0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn load_mode_resource_preload_request_id_0x35<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x11d3f0;
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
        { crate::pose::cover(0x11d3f0, previous, pc); previous = pc; }
        match pc {
            0x11d3f0 => {
                push!(0x35);
                call!(0xa5e40);
                pc = 0x11d3f7; continue;
            }
            0x11d3f7 => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x1271D0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn load_mode_resource_when_any_field_0x70_to_0x7c_set_then_call_812c0_flag_0x2000000<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1271d0;
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
        { crate::pose::cover(0x1271d0, previous, pc); previous = pc; }
        match pc {
            0x1271d0 => {
                call!(0x6a540);
                pc = 0x1271d5; continue;
            }
            0x1271d5 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x127207; continue; }
                pc = 0x1271d9; continue;
            }
            0x1271d9 => {
                ecx = m.read32(eax.wrapping_add(0x70));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1271f5; continue; }
                pc = 0x1271e0; continue;
            }
            0x1271e0 => {
                ecx = m.read32(eax.wrapping_add(0x74));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1271f5; continue; }
                pc = 0x1271e7; continue;
            }
            0x1271e7 => {
                ecx = m.read32(eax.wrapping_add(0x78));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1271f5; continue; }
                pc = 0x1271ee; continue;
            }
            0x1271ee => {
                ecx = m.read32(eax.wrapping_add(0x7c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x127207; continue; }
                pc = 0x1271f5; continue;
            }
            0x1271f5 => {
                push!(0x2000000);
                call!(0x812c0);
                pc = 0x1271ff; continue;
            }
            0x1271ff => {
                esp = esp.wrapping_add(0x4);
                tail_to!(0x194a60); break 'run;  // direct tail jump
            }
            0x127207 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x150530. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn crc32_cstring<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x150530;
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
        { crate::pose::cover(0x150530, previous, pc); previous = pc; }
        match pc {
            0x150530 => {
                edx = m.read32(esp.wrapping_add(0x4));
                eax = edx;
                push!(esi);
                esi = eax.wrapping_add(0x1);
                pc = 0x150540; continue;
            }
            0x150540 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                eax = eax.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x150540; continue; }
                pc = 0x150547; continue;
            }
            0x150547 => {
                eax = eax.wrapping_sub(esi);
                push!(eax);
                push!(edx);
                call!(0x1504f0);
                pc = 0x150550; continue;
            }
            0x150550 => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x1B6130. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn preload_all_loadout_and_effect_resource_ids_0x743_to_0x746<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b6130;
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
        { crate::pose::cover(0x1b6130, previous, pc); previous = pc; }
        match pc {
            0x1b6130 => {
                push!(ebx);
                push!(esi);
                push!(edi);
                esi = 0x4ea56c;
                pc = 0x1b6140; continue;
            }
            0x1b6140 => {
                eax = m.read32(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)));
                push!(eax);
                call!(0xa5e40);
                pc = 0x1b6149; continue;
            }
            0x1b6149 => {
                eax = m.read32(esi);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b615c; continue; }
                pc = 0x1b6153; continue;
            }
            0x1b6153 => {
                push!(eax);
                call!(0xa5e40);
                pc = 0x1b6159; continue;
            }
            0x1b6159 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x1b615c; continue;
            }
            0x1b615c => {
                esi = esi.wrapping_add(0xe0);
                fa = esi;
                fb = ((((0x4ecb0ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1b6140; continue; }
                pc = 0x1b616a; continue;
            }
            0x1b616a => {
                edi = 0;
                ebx = 0x4fb198;
                esi = 0x4fa128;
                pc = 0x1b6176; continue;
            }
            0x1b6176 => {
                fa = ((((m.read8(esi))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x1b619e; continue; }
                pc = 0x1b617b; continue;
            }
            0x1b617b => {
                push!(0);
                eax = edi;
                call!(0x1b5f40);
                pc = 0x1b6184; continue;
            }
            0x1b6184 => {
                eax = m.read32(ebx);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b619e; continue; }
                pc = 0x1b618e; continue;
            }
            0x1b618e => {
                fa = eax;
                fb = ((((0x9d0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1b619e; continue; }
                pc = 0x1b6195; continue;
            }
            0x1b6195 => {
                push!(eax);
                call!(0xa5e40);
                pc = 0x1b619b; continue;
            }
            0x1b619b => {
                esp = esp.wrapping_add(0x4);
                pc = 0x1b619e; continue;
            }
            0x1b619e => {
                esi = esi.wrapping_add(0x3c);
                edi = edi.wrapping_add(1);
                ebx = ebx.wrapping_add(0x54);
                fa = esi;
                fb = ((((0x4fb154i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1b6176; continue; }
                pc = 0x1b61ad; continue;
            }
            0x1b61ad => {
                push!(0x743);
                call!(0xa5e40);
                pc = 0x1b61b7; continue;
            }
            0x1b61b7 => {
                push!(0x745);
                call!(0xa5e40);
                pc = 0x1b61c1; continue;
            }
            0x1b61c1 => {
                push!(0x744);
                call!(0xa5e40);
                pc = 0x1b61cb; continue;
            }
            0x1b61cb => {
                push!(0x746);
                call!(0xa5e40);
                pc = 0x1b61d5; continue;
            }
            0x1b61d5 => {
                esp = esp.wrapping_add(0x10);
                call!(0xf6b70);
                pc = 0x1b61dd; continue;
            }
            0x1b61dd => {
                ebx = 0;
                call!(0x1b5bb0);
                pc = 0x1b61e4; continue;
            }
            0x1b61e4 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x663f0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x1CDC50. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn load_mode_resources_by_global<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1cdc50;
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
        { crate::pose::cover(0x1cdc50, previous, pc); previous = pc; }
        match pc {
            0x1cdc50 => {
                call!(0x389b0);
                pc = 0x1cdc55; continue;
            }
            0x1cdc55 => {
                call!(0x81220);
                pc = 0x1cdc5a; continue;
            }
            0x1cdc5a => {
                call!(0x18200);
                pc = 0x1cdc5f; continue;
            }
            0x1cdc5f => {
                call!(0x389b0);
                pc = 0x1cdc64; continue;
            }
            0x1cdc64 => {
                eax = m.read32(0x79094c);
                eax = eax.wrapping_add(0xffffff9bu32);
                fa = eax;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x1cdcd2; continue; }
                pc = 0x1cdc71; continue;
            }
            0x1cdc71 => {
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x1cdd40));
                if (jt == 0x1cdc78u32) { pc = 0x1cdc78; continue; }
                if (jt == 0x1cdc91u32) { pc = 0x1cdc91; continue; }
                if (jt == 0x1cdca5u32) { pc = 0x1cdca5; continue; }
                if (jt == 0x1cdcb4u32) { pc = 0x1cdcb4; continue; }
                if (jt == 0x1cdcc3u32) { pc = 0x1cdcc3; continue; }
                if (jt == 0x1cdd36u32) { pc = 0x1cdd36; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x1cdc78 => {
                call!(0x389b0);
                pc = 0x1cdc7d; continue;
            }
            0x1cdc7d => {
                call!(0x175d0);
                pc = 0x1cdc82; continue;
            }
            0x1cdc82 => {
                call!(0x2d2710);
                pc = 0x1cdc87; continue;
            }
            0x1cdc87 => {
                call!(0x97fb0);
                pc = 0x1cdc8c; continue;
            }
            0x1cdc8c => {
                tail_to!(0x6c920); break 'run;  // direct tail jump
            }
            0x1cdc91 => {
                call!(0x2fe230);
                pc = 0x1cdc96; continue;
            }
            0x1cdc96 => {
                call!(0x389b0);
                pc = 0x1cdc9b; continue;
            }
            0x1cdc9b => {
                call!(0x97fb0);
                pc = 0x1cdca0; continue;
            }
            0x1cdca0 => {
                tail_to!(0x6c920); break 'run;  // direct tail jump
            }
            0x1cdca5 => {
                call!(0x70400);
                pc = 0x1cdcaa; continue;
            }
            0x1cdcaa => {
                call!(0x97fb0);
                pc = 0x1cdcaf; continue;
            }
            0x1cdcaf => {
                tail_to!(0x6c920); break 'run;  // direct tail jump
            }
            0x1cdcb4 => {
                call!(0x389b0);
                pc = 0x1cdcb9; continue;
            }
            0x1cdcb9 => {
                call!(0x97fb0);
                pc = 0x1cdcbe; continue;
            }
            0x1cdcbe => {
                tail_to!(0x6c920); break 'run;  // direct tail jump
            }
            0x1cdcc3 => {
                call!(0x81220);
                pc = 0x1cdcc8; continue;
            }
            0x1cdcc8 => {
                call!(0x97fb0);
                pc = 0x1cdccd; continue;
            }
            0x1cdccd => {
                tail_to!(0x6c920); break 'run;  // direct tail jump
            }
            0x1cdcd2 => {
                call!(0x389b0);
                pc = 0x1cdcd7; continue;
            }
            0x1cdcd7 => {
                call!(0x1271d0);
                pc = 0x1cdcdc; continue;
            }
            0x1cdcdc => {
                call!(0x11d3f0);
                pc = 0x1cdce1; continue;
            }
            0x1cdce1 => {
                call!(0x389b0);
                pc = 0x1cdce6; continue;
            }
            0x1cdce6 => {
                call!(0x11c9b0);
                pc = 0x1cdceb; continue;
            }
            0x1cdceb => {
                call!(0x119960);
                pc = 0x1cdcf0; continue;
            }
            0x1cdcf0 => {
                call!(0x389b0);
                pc = 0x1cdcf5; continue;
            }
            0x1cdcf5 => {
                call!(0xac8a0);
                pc = 0x1cdcfa; continue;
            }
            0x1cdcfa => {
                call!(0x175d0);
                pc = 0x1cdcff; continue;
            }
            0x1cdcff => {
                call!(0xcf230);
                pc = 0x1cdd04; continue;
            }
            0x1cdd04 => {
                call!(0x85920);
                pc = 0x1cdd09; continue;
            }
            0x1cdd09 => {
                call!(0x9a140);
                pc = 0x1cdd0e; continue;
            }
            0x1cdd0e => {
                call!(0x1b60f0);
                pc = 0x1cdd13; continue;
            }
            0x1cdd13 => {
                call!(0x389b0);
                pc = 0x1cdd18; continue;
            }
            0x1cdd18 => {
                call!(0x158bc0);
                pc = 0x1cdd1d; continue;
            }
            0x1cdd1d => {
                call!(0x351210);
                pc = 0x1cdd22; continue;
            }
            0x1cdd22 => {
                call!(0x2e6560);
                pc = 0x1cdd27; continue;
            }
            0x1cdd27 => {
                call!(0x389b0);
                pc = 0x1cdd2c; continue;
            }
            0x1cdd2c => {
                call!(0x22cd30);
                pc = 0x1cdd31; continue;
            }
            0x1cdd31 => {
                call!(0x148310);
                pc = 0x1cdd36; continue;
            }
            0x1cdd36 => {
                call!(0x97fb0);
                pc = 0x1cdd3b; continue;
            }
            0x1cdd3b => {
                tail_to!(0x6c920); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x1CF630. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn clear_globals_75af1c_75af20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1cf630;
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
        { crate::pose::cover(0x1cf630, previous, pc); previous = pc; }
        match pc {
            0x1cf630 => {
                eax = 0;
                m.write32(0x75af1c, eax);
                m.write32(0x75af20, eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x1F1C00. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn scene_preload_object_resources_and_load_slides_pak<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1f1c00;
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
        { crate::pose::cover(0x1f1c00, previous, pc); previous = pc; }
        match pc {
            0x1f1c00 => {
                push!(0x323);
                call!(0xa5e40);
                pc = 0x1f1c0a; continue;
            }
            0x1f1c0a => {
                push!(0x326);
                call!(0xa5e40);
                pc = 0x1f1c14; continue;
            }
            0x1f1c14 => {
                push!(0x327);
                call!(0xa5e40);
                pc = 0x1f1c1e; continue;
            }
            0x1f1c1e => {
                push!(0x321);
                call!(0xa5e40);
                pc = 0x1f1c28; continue;
            }
            0x1f1c28 => {
                push!(0x328);
                call!(0xa5e40);
                pc = 0x1f1c32; continue;
            }
            0x1f1c32 => {
                push!(0x36c);
                call!(0xa5e40);
                pc = 0x1f1c3c; continue;
            }
            0x1f1c3c => {
                push!(0x743);
                call!(0xa5e40);
                pc = 0x1f1c46; continue;
            }
            0x1f1c46 => {
                push!(0x745);
                call!(0xa5e40);
                pc = 0x1f1c50; continue;
            }
            0x1f1c50 => {
                push!(0x376);
                call!(0xa5e40);
                pc = 0x1f1c5a; continue;
            }
            0x1f1c5a => {
                push!(0x377);
                call!(0xa5e40);
                pc = 0x1f1c64; continue;
            }
            0x1f1c64 => {
                push!(0x378);
                call!(0xa5e40);
                pc = 0x1f1c6e; continue;
            }
            0x1f1c6e => {
                push!(0x379);
                call!(0xa5e40);
                pc = 0x1f1c78; continue;
            }
            0x1f1c78 => {
                push!(0x38b);
                call!(0xa5e40);
                pc = 0x1f1c82; continue;
            }
            0x1f1c82 => {
                push!(0x38c);
                call!(0xa5e40);
                pc = 0x1f1c8c; continue;
            }
            0x1f1c8c => {
                push!(0x38d);
                call!(0xa5e40);
                pc = 0x1f1c96; continue;
            }
            0x1f1c96 => {
                push!(0x147);
                call!(0xa5e40);
                pc = 0x1f1ca0; continue;
            }
            0x1f1ca0 => {
                esp = esp.wrapping_add(0x40);
                push!(0x144);
                call!(0xa5e40);
                pc = 0x1f1cad; continue;
            }
            0x1f1cad => {
                push!(0x31d);
                call!(0xa5e40);
                pc = 0x1f1cb7; continue;
            }
            0x1f1cb7 => {
                push!(0x389);
                call!(0xa5e40);
                pc = 0x1f1cc1; continue;
            }
            0x1f1cc1 => {
                push!(0x38a);
                call!(0xa5e40);
                pc = 0x1f1ccb; continue;
            }
            0x1f1ccb => {
                push!(0x337);
                call!(0xa5e40);
                pc = 0x1f1cd5; continue;
            }
            0x1f1cd5 => {
                push!(0x338);
                call!(0xa5e40);
                pc = 0x1f1cdf; continue;
            }
            0x1f1cdf => {
                push!(0x339);
                call!(0xa5e40);
                pc = 0x1f1ce9; continue;
            }
            0x1f1ce9 => {
                push!(0x336);
                call!(0xa5e40);
                pc = 0x1f1cf3; continue;
            }
            0x1f1cf3 => {
                push!(0x1);
                push!(0);
                push!(0x49e360);
                call!(0x622d0);
                pc = 0x1f1d01; continue;
            }
            0x1f1d01 => {
                push!(0x1e);
                m.write32(0x78e3c4, eax);
                call!(0xf4830);
                pc = 0x1f1d0d; continue;
            }
            0x1f1d0d => {
                push!(0x3b4);
                call!(0x45590);
                pc = 0x1f1d17; continue;
            }
            0x1f1d17 => {
                push!(0xc6b);
                call!(0x45590);
                pc = 0x1f1d21; continue;
            }
            0x1f1d21 => {
                esp = esp.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x1F7870. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn scene_preload_object_resources_0x454_0x47f_0x480_and_tail_call_d8c30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1f7870;
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
        { crate::pose::cover(0x1f7870, previous, pc); previous = pc; }
        match pc {
            0x1f7870 => {
                push!(0x454);
                call!(0xa5e40);
                pc = 0x1f787a; continue;
            }
            0x1f787a => {
                push!(0x47f);
                call!(0xa5e40);
                pc = 0x1f7884; continue;
            }
            0x1f7884 => {
                push!(0x480);
                call!(0xa5e40);
                pc = 0x1f788e; continue;
            }
            0x1f788e => {
                push!(0x7);
                call!(0x45590);
                pc = 0x1f7895; continue;
            }
            0x1f7895 => {
                push!(0x8);
                call!(0x45590);
                pc = 0x1f789c; continue;
            }
            0x1f789c => {
                esp = esp.wrapping_add(0x14);
                tail_to!(0xd8c30); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x1FC870. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn scene_clear_resource_entry_0x4ca_field_0xc<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1fc870;
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
        { crate::pose::cover(0x1fc870, previous, pc); previous = pc; }
        match pc {
            0x1fc870 => {
                push!(0x4ca);
                call!(0x45590);
                pc = 0x1fc87a; continue;
            }
            0x1fc87a => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x2270F0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn scene_preload_object_resource_0x13_and_tail_call_d8c30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2270f0;
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
        { crate::pose::cover(0x2270f0, previous, pc); previous = pc; }
        match pc {
            0x2270f0 => {
                push!(0x13);
                call!(0xa5e40);
                pc = 0x2270f7; continue;
            }
            0x2270f7 => {
                esp = esp.wrapping_add(0x4);
                tail_to!(0xd8c30); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x22CE60. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn preload_texture_resources_for_all_state_indices<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x22ce60;
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
        { crate::pose::cover(0x22ce60, previous, pc); previous = pc; }
        match pc {
            0x22ce60 => {
                eax = m.read32(0x7de458);
                push!(ebx);
                ebx = m.read32(0x7de470);
                push!(esi);
                push!(edi);
                edi = eax;
                eax = (eax | 0x40);
                m.write32(0x7de458, eax);
                esi = 0;
                pc = 0x22ce80; continue;
            }
            0x22ce80 => {
                m.write32(0x7de470, esi);
                call!(0x22cd30);
                pc = 0x22ce8b; continue;
            }
            0x22ce8b => {
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ((((0x15i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x22ce80; continue; }
                pc = 0x22ce91; continue;
            }
            0x22ce91 => {
                push!(0x400000);
                m.write32(0x7de458, edi);
                m.write32(0x7de470, ebx);
                call!(0x812c0);
                pc = 0x22cea7; continue;
            }
            0x22cea7 => {
                esp = esp.wrapping_add(0x4);
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

/// 0x242700. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pak_entry_load_into_slot_table_and_count_records<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x242700;
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
        { crate::pose::cover(0x242700, previous, pc); previous = pc; }
        match pc {
            0x242700 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x18));
                esi = edi.wrapping_mul(0x4).wrapping_add(0x78b740);
                push!(esi);
                push!(0x1);
                push!(ebx);
                ebp = 0;
                call!(0x62f50);
                pc = 0x24271e; continue;
            }
            0x24271e => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(edi.wrapping_mul(0x4).wrapping_add(0x78b7a0), eax);
                if (fa == fb) { pc = 0x242772; continue; }
                pc = 0x24272c; continue;
            }
            0x24272c => {
                fa = m.read32(esi);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x242772; continue; }
                pc = 0x242731; continue;
            }
            0x242731 => {
                push!(ebp);
                push!(ebx);
                call!(0x389b0);
                pc = 0x242738; continue;
            }
            0x242738 => {
                eax = m.read32(0x78b850);
                ecx = m.read32(edi.wrapping_mul(0x4).wrapping_add(0x78b7a0));
                edx = m.read32(esi);
                esp = esp.wrapping_add(0x8);
                eax = eax.wrapping_add(1);
                m.write32(0x78b850, eax);
                ebx = m.read32(ecx.wrapping_add(0x4));
                ecx = ecx.wrapping_add(0x4);
                edx = edx.wrapping_sub(ebx);
                edx = edx.wrapping_add(0x8);
                eax = 0xcccccccdu32;
                { let wide = (eax as u64) * (edx as u64); eax = wide as u32; edx = (wide >> 32) as u32; }
                edx = edx.wrapping_shr((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write32(edi.wrapping_mul(0x4).wrapping_add(0x78b800), edx);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ecx;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x242772 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x257B20. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn load_float_75f5b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x257b20;
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
        { crate::pose::cover(0x257b20, previous, pc); previous = pc; }
        match pc {
            0x257b20 => {
                fpu.top = (fpu.top + 7) & 7; fpu.stack[fpu.top as usize] = f32::from_bits(m.read32(0x75f5b0)) as f64;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x257B30. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn call_a5e40_with_0x24d_if_mode_79094c_is_0x1e<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x257b30;
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
        { crate::pose::cover(0x257b30, previous, pc); previous = pc; }
        match pc {
            0x257b30 => {
                fa = m.read32(0x79094c);
                fb = ((((0x1ei32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x257b44; continue; }
                pc = 0x257b39; continue;
            }
            0x257b39 => {
                push!(0x24d);
                call!(0xa5e40);
                pc = 0x257b43; continue;
            }
            0x257b43 => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x257b44; continue;
            }
            0x257b44 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x275F40. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn load_float_from_2d_table_78b2d8_bounds_checked<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x275f40;
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
        { crate::pose::cover(0x275f40, previous, pc); previous = pc; }
        match pc {
            0x275f40 => {
                edx = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(0x7b666c);
                fa = edx;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x275f67; continue; }
                pc = 0x275f4e; continue;
            }
            0x275f4e => {
                eax = m.read32(esp.wrapping_add(0x8));
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x275f67; continue; }
                pc = 0x275f56; continue;
            }
            0x275f56 => {
                ecx = m.read32(0x78b2d8);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x275f67; continue; }
                pc = 0x275f60; continue;
            }
            0x275f60 => {
                ecx = m.read32(ecx.wrapping_add(edx.wrapping_mul(0x4)));
                fpu.top = (fpu.top + 7) & 7; fpu.stack[fpu.top as usize] = f32::from_bits(m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)))) as f64;
                break 'run;
            }
            0x275f67 => {
                fpu.top = (fpu.top + 7) & 7; fpu.stack[fpu.top as usize] = f32::from_bits(m.read32(0x4760d0)) as f64;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x2C3300. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn load_audio_settings_from_profile_into_menu_globals<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2c3300;
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
        { crate::pose::cover(0x2c3300, previous, pc); previous = pc; }
        match pc {
            0x2c3300 => {
                call!(0x69640);
                pc = 0x2c3305; continue;
            }
            0x2c3305 => {
                eax = m.read32(eax.wrapping_add(0x114));
                m.write32(0x7613d0, eax);
                m.write32(0x7613d4, 0);
                call!(0x69640);
                pc = 0x2c331f; continue;
            }
            0x2c331f => {
                ecx = m.read32(eax.wrapping_add(0x118));
                m.write32(0x5236b4, ecx);
                call!(0x69640);
                pc = 0x2c3330; continue;
            }
            0x2c3330 => {
                edx = m.read32(eax.wrapping_add(0x11c));
                m.write32(0x5236b8, edx);
                call!(0x69640);
                pc = 0x2c3341; continue;
            }
            0x2c3341 => {
                eax = m.read32(eax.wrapping_add(0x120));
                m.write32(0x5236bc, eax);
                call!(0x69640);
                pc = 0x2c3351; continue;
            }
            0x2c3351 => {
                ecx = m.read32(eax.wrapping_add(0x124));
                m.write32(0x5236c0, ecx);
                call!(0x69640);
                pc = 0x2c3362; continue;
            }
            0x2c3362 => {
                edx = m.read32(eax.wrapping_add(0x12c));
                m.write32(0x7613d8, edx);
                call!(0x69640);
                pc = 0x2c3373; continue;
            }
            0x2c3373 => {
                eax = m.read32(eax.wrapping_add(0x130));
                m.write32(0x7613dc, eax);
                tail_to!(0x45c60); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x2D95B0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn load_loadbar_texture<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2d95b0;
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
        { crate::pose::cover(0x2d95b0, previous, pc); previous = pc; }
        match pc {
            0x2d95b0 => {
                esp = esp.wrapping_sub(0x80);
                push!(0x1);
                push!(0);
                push!(0x47dcf4);
                call!(0x622d0);
                pc = 0x2d95c4; continue;
            }
            0x2d95c4 => {
                eax = m.read32(0x79094c);
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0x6bi32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2d95d8; continue; }
                pc = 0x2d95d1; continue;
            }
            0x2d95d1 => {
                eax = 0x18e;
                pc = 0x2d95e6; continue;
            }
            0x2d95d8 => {
                eax = m.read32(0x78ad48);
                eax = ((eax as i32).wrapping_mul((0x44) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(0x4d1ad8));
                pc = 0x2d95e6; continue;
            }
            0x2d95e6 => {
                push!(eax);
                call!(0x80f30);
                pc = 0x2d95ec; continue;
            }
            0x2d95ec => {
                push!(eax);
                ecx = esp.wrapping_add(0x8);
                push!(0x47dcd4);
                push!(ecx);
                call!(0x3c85d4);
                pc = 0x2d95fc; continue;
            }
            0x2d95fc => {
                push!(0);
                edx = esp.wrapping_add(0x14);
                push!(0);
                push!(edx);
                call!(0x62f00);
                pc = 0x2d960a; continue;
            }
            0x2d960a => {
                esp = esp.wrapping_add(0x1c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2d9628; continue; }
                pc = 0x2d9611; continue;
            }
            0x2d9611 => {
                push!(0x5246f8);
                push!(0);
                push!(eax);
                call!(0x18a30);
                pc = 0x2d961e; continue;
            }
            0x2d961e => {
                esp = esp.wrapping_add(0xc);
                esp = esp.wrapping_add(0x80);
                break 'run;
            }
            0x2d9628 => {
                push!(0x336);
                call!(0x80f20);
                pc = 0x2d9632; continue;
            }
            0x2d9632 => {
                esp = esp.wrapping_add(0x4);
                m.write32(0x5246f8, eax);
                esp = esp.wrapping_add(0x80);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x2FE270. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn preload_object_resources_for_table_52d150_entries<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe270;
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
        { crate::pose::cover(0x2fe270, previous, pc); previous = pc; }
        match pc {
            0x2fe270 => {
                push!(esi);
                esi = 0;
                call!(0x3118d0);
                pc = 0x2fe278; continue;
            }
            0x2fe278 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x2fe2ae; continue; }
                pc = 0x2fe27c; continue;
            }
            0x2fe27c => {
                pc = 0x2fe280; continue;
            }
            0x2fe280 => {
                push!(esi);
                call!(0x3118e0);
                pc = 0x2fe286; continue;
            }
            0x2fe286 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x2fe2a4; continue; }
                pc = 0x2fe28d; continue;
            }
            0x2fe28d => {
                push!(eax);
                call!(0x6a660);
                pc = 0x2fe293; continue;
            }
            0x2fe293 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe2a4; continue; }
                pc = 0x2fe29b; continue;
            }
            0x2fe29b => {
                push!(eax);
                call!(0xa5e40);
                pc = 0x2fe2a1; continue;
            }
            0x2fe2a1 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x2fe2a4; continue;
            }
            0x2fe2a4 => {
                esi = esi.wrapping_add(1);
                call!(0x3118d0);
                pc = 0x2fe2aa; continue;
            }
            0x2fe2aa => {
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2fe280; continue; }
                pc = 0x2fe2ae; continue;
            }
            0x2fe2ae => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x311840. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn record_58_first_resource_id_field_4_by_index<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x311840;
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
        { crate::pose::cover(0x311840, previous, pc); previous = pc; }
        match pc {
            0x311840 => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = ((eax as i32).wrapping_mul((0x58) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(0x52cf3c));
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x3118D0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn return_constant_0xb<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3118d0;
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
        { crate::pose::cover(0x3118d0, previous, pc); previous = pc; }
        match pc {
            0x3118d0 => {
                eax = 0xb;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x3118E0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn get_preload_object_resource_table_entry_by_index<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3118e0;
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
        { crate::pose::cover(0x3118e0, previous, pc); previous = pc; }
        match pc {
            0x3118e0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = m.read32(eax.wrapping_mul(0x8).wrapping_add(0x52d150));
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x311BA0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn load_hud_panel_rect_float_by_index_stride8<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x311ba0;
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
        { crate::pose::cover(0x311ba0, previous, pc); previous = pc; }
        match pc {
            0x311ba0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                fpu.top = (fpu.top + 7) & 7; fpu.stack[fpu.top as usize] = f32::from_bits(m.read32(eax.wrapping_mul(0x8).wrapping_add(0x52d43c))) as f64;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x32CAB0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn load_pad_tile_raw_files<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x32cab0;
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
        { crate::pose::cover(0x32cab0, previous, pc); previous = pc; }
        match pc {
            0x32cab0 => {
                push!(ecx);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                push!(0x1);
                push!(0x1e5b0);
                ebx = 0;
                ebp = 0;
                call!(0x3dfa0);
                pc = 0x32cac5; continue;
            }
            0x32cac5 => {
                m.write32(0x778d34, eax);
                m.write32(eax.wrapping_add(0x5a0), ebx);
                edi = m.read32(0x778d34);
                eax = 0;
                ecx = 0x140;
                edi = edi.wrapping_add(0xa0);
                esp = esp.wrapping_add(0x8);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                call!(0x61fc0);
                pc = 0x32caed; continue;
            }
            0x32caed => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x32cb3a; continue; }
                pc = 0x32caf1; continue;
            }
            0x32caf1 => {
                esi = 0x7d9;
                edi = 0x1d;
                pc = 0x32cb00; continue;
            }
            0x32cb00 => {
                eax = esi.wrapping_add(((0x1di32.wrapping_neg()) as u32));
                push!(eax);
                call!(0xa5e40);
                pc = 0x32cb09; continue;
            }
            0x32cb09 => {
                push!(esi);
                call!(0xa5e40);
                pc = 0x32cb0f; continue;
            }
            0x32cb0f => {
                ecx = esi.wrapping_add(0x1d);
                push!(ecx);
                call!(0xa5e40);
                pc = 0x32cb18; continue;
            }
            0x32cb18 => {
                edx = esi.wrapping_add(0x3a);
                push!(edx);
                call!(0xa5e40);
                pc = 0x32cb21; continue;
            }
            0x32cb21 => {
                eax = esi.wrapping_add(0x57);
                push!(eax);
                call!(0xa5e40);
                pc = 0x32cb2a; continue;
            }
            0x32cb2a => {
                ecx = esi.wrapping_add(0x74);
                push!(ecx);
                call!(0xa5e40);
                pc = 0x32cb33; continue;
            }
            0x32cb33 => {
                esp = esp.wrapping_add(0x18);
                esi = esi.wrapping_add(1);
                edi = edi.wrapping_sub(1);
                fa = edi;
                if ((fa != 0)) { pc = 0x32cb00; continue; }
                pc = 0x32cb3a; continue;
            }
            0x32cb3a => {
                eax = 0x529661;
                pc = 0x32cb40; continue;
            }
            0x32cb40 => {
                ecx = ((m.read8(eax.wrapping_add(((0x140i32.wrapping_neg()) as u32)))) as u8 as u32);
                fa = ecx;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { ebp = ecx; }
                ecx = ((m.read8(eax)) as u8 as u32);
                fa = ecx;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { ebp = ecx; }
                ecx = ((m.read8(eax.wrapping_add(0x140))) as u8 as u32);
                fa = ecx;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { ebp = ecx; }
                ecx = ((m.read8(eax.wrapping_add(0x280))) as u8 as u32);
                fa = ecx;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { ebp = ecx; }
                ecx = ((m.read8(eax.wrapping_add(0x3c0))) as u8 as u32);
                fa = ecx;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { ebp = ecx; }
                eax = eax.wrapping_add(0x640);
                fa = eax;
                fb = ((((0x52cea1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x32cb40; continue; }
                pc = 0x32cb84; continue;
            }
            0x32cb84 => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x32cbce; continue; }
                pc = 0x32cb88; continue;
            }
            0x32cb88 => {
                m.write32(esp.wrapping_add(0x10), 0x7850c0);
                pc = 0x32cb90; continue;
            }
            0x32cb90 => {
                esi = m.read32(esp.wrapping_add(0x10));
                eax = 0;
                ebx = ebx.wrapping_add(1);
                pc = 0x32cb97; continue;
            }
            0x32cb97 => {
                push!(ebx);
                edi = eax.wrapping_add(0x1);
                push!(edi);
                push!(0x47d364);
                push!(esi);
                call!(0x3c85d4);
                pc = 0x32cba7; continue;
            }
            0x32cba7 => {
                push!(0);
                push!(esi);
                call!(0x639e0);
                pc = 0x32cbaf; continue;
            }
            0x32cbaf => {
                eax = edi;
                esp = esp.wrapping_add(0x18);
                esi = esi.wrapping_add(0xb40);
                fa = eax;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x32cb97; continue; }
                pc = 0x32cbbf; continue;
            }
            0x32cbbf => {
                ecx = m.read32(esp.wrapping_add(0x10));
                ecx = ecx.wrapping_add(0x40);
                fa = ebx;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), ecx);
                if (fas < fbs) { pc = 0x32cb90; continue; }
                pc = 0x32cbce; continue;
            }
            0x32cbce => {
                call!(0x338110);
                pc = 0x32cbd3; continue;
            }
            0x32cbd3 => {
                push!(0x895);
                call!(0xa5e40);
                pc = 0x32cbdd; continue;
            }
            0x32cbdd => {
                push!(0x896);
                call!(0xa5e40);
                pc = 0x32cbe7; continue;
            }
            0x32cbe7 => {
                push!(0x897);
                call!(0xa5e40);
                pc = 0x32cbf1; continue;
            }
            0x32cbf1 => {
                esp = esp.wrapping_add(0xc);
                ebp = 0;
                edi = 0x525010;
                pc = 0x32cc00; continue;
            }
            0x32cc00 => {
                ecx = m.read32(edi);
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x32cc65; continue; }
                pc = 0x32cc07; continue;
            }
            0x32cc07 => {
                eax = (eax & 0xffff_0000) | ((((m.read16(edi.wrapping_add(((0xb6i32.wrapping_neg()) as u32)))) as u32)) & 0xffff);
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0x8i32)) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x32cc65; continue; }
                pc = 0x32cc14; continue;
            }
            0x32cc14 => {
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0x9i32)) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x32cc65; continue; }
                pc = 0x32cc1a; continue;
            }
            0x32cc1a => {
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x32cc48; continue; }
                pc = 0x32cc1f; continue;
            }
            0x32cc1f => {
                esi = edi.wrapping_add(((0xaci32.wrapping_neg()) as u32));
                ebx = 0x6;
                pc = 0x32cc30; continue;
            }
            0x32cc30 => {
                eax = m.read32(esi);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x32cc40; continue; }
                pc = 0x32cc37; continue;
            }
            0x32cc37 => {
                push!(eax);
                call!(0xa5e40);
                pc = 0x32cc3d; continue;
            }
            0x32cc3d => {
                esp = esp.wrapping_add(0x4);
                pc = 0x32cc40; continue;
            }
            0x32cc40 => {
                esi = esi.wrapping_add(0x1c);
                ebx = ebx.wrapping_sub(1);
                fa = ebx;
                if ((fa != 0)) { pc = 0x32cc30; continue; }
                pc = 0x32cc46; continue;
            }
            0x32cc46 => {
                pc = 0x32cc65; continue;
            }
            0x32cc48 => {
                push!(ebp);
                call!(0x2fe840);
                pc = 0x32cc4e; continue;
            }
            0x32cc4e => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x32cc65; continue; }
                pc = 0x32cc56; continue;
            }
            0x32cc56 => {
                push!(ebp);
                call!(0x2fe840);
                pc = 0x32cc5c; continue;
            }
            0x32cc5c => {
                push!(eax);
                call!(0xa5e40);
                pc = 0x32cc62; continue;
            }
            0x32cc62 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x32cc65; continue;
            }
            0x32cc65 => {
                edi = edi.wrapping_add(0xc4);
                ebp = ebp.wrapping_add(1);
                fa = edi;
                fb = ((((0x528d50i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x32cc00; continue; }
                pc = 0x32cc74; continue;
            }
            0x32cc74 => {
                esi = 0x4ea564;
                pc = 0x32cc80; continue;
            }
            0x32cc80 => {
                fa = m.read32(esi.wrapping_add(0x4));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x32cc95; continue; }
                pc = 0x32cc86; continue;
            }
            0x32cc86 => {
                push!(esi);
                call!(0xdb2c0);
                pc = 0x32cc8c; continue;
            }
            0x32cc8c => {
                push!(eax);
                call!(0xa5e40);
                pc = 0x32cc92; continue;
            }
            0x32cc92 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x32cc95; continue;
            }
            0x32cc95 => {
                esi = esi.wrapping_add(0xe0);
                fa = esi;
                fb = ((((0x4ecb04i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x32cc80; continue; }
                pc = 0x32cca3; continue;
            }
            0x32cca3 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x330DF0. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn load_tileset_resource_entry_copy_into_arena_buffers_by_index<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x330df0;
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
        { crate::pose::cover(0x330df0, previous, pc); previous = pc; }
        match pc {
            0x330df0 => {
                esp = esp.wrapping_sub(0x48);
                ecx = m.read32(0x784020);
                push!(ebx);
                ecx = ((ecx as i32).wrapping_mul((0x2d) as i32)) as u32;
                ebx = m.read32(esp.wrapping_add(0x50));
                push!(ebp);
                ebp = ebx.wrapping_add(ebx.wrapping_mul(0x4));
                ebp = ebp.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                ebp = ebp.wrapping_add(0x529510);
                eax = ((m.read8(ebp.wrapping_add(0x11))) as u8 as u32);
                eax = eax.wrapping_add(ecx);
                push!(esi);
                eax = eax.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = eax.wrapping_add(0x785080);
                push!(edi);
                push!(edx);
                call!(0x630c0);
                pc = 0x330e25; continue;
            }
            0x330e25 => {
                esi = eax;
                eax = m.read32(esi.wrapping_add(0x10));
                edi = m.read32(eax.wrapping_add(0x4));
                eax = m.read32(esp.wrapping_add(0x64));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x5c), esi);
                m.write32(esp.wrapping_add(0x10), edi);
                if (fa == fb) { pc = 0x330e5a; continue; }
                pc = 0x330e40; continue;
            }
            0x330e40 => {
                ecx = edi;
                ecx = ecx.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_add(0x2c);
                push!(0x1);
                push!(ecx);
                call!(0x3dfa0);
                pc = 0x330e50; continue;
            }
            0x330e50 => {
                esp = esp.wrapping_add(0x8);
                m.write32(ebx.wrapping_mul(0x4).wrapping_add(0x785000), eax);
                pc = 0x330e5a; continue;
            }
            0x330e5a => {
                esi = m.read32(esi.wrapping_add(0x10));
                eax = edi;
                edi = m.read32(ebx.wrapping_mul(0x4).wrapping_add(0x785000));
                eax = eax.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = ebx.wrapping_mul(0x4).wrapping_add(0x785000);
                ecx = eax.wrapping_add(0x2c);
                m.write32(esp.wrapping_add(0x14), edx);
                edx = ecx;
                ecx = ecx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                ecx = edx;
                ecx = (ecx & 0x3);
                { let step = if entry.eflags & 0x400 != 0 { u32::MAX } else { 1 }; for i in 0..ecx { m.write8(edi.wrapping_add(i.wrapping_mul(step)), m.read8(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }
                ecx = 0;
                ecx = m.read32(esp.wrapping_add(0x5c));
                ecx = m.read32(ecx.wrapping_add(0x10));
                eax = ecx.wrapping_add(eax).wrapping_add(0xc);
                ecx = m.read32(ecx.wrapping_add(0x8));
                edx = ecx.wrapping_add(ecx.wrapping_mul(0x4));
                edi = m.read32(eax.wrapping_add(edx.wrapping_mul(0x4)));
                esi = eax.wrapping_add(edx.wrapping_mul(0x4));
                eax = m.read32(esp.wrapping_add(0x60));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x5c), edi);
                if (fa == fb) { pc = 0x330ec2; continue; }
                pc = 0x330ea8; continue;
            }
            0x330ea8 => {
                eax = edi;
                eax = eax.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(0x2c);
                push!(0x1);
                push!(eax);
                call!(0x3dfa0);
                pc = 0x330eb8; continue;
            }
            0x330eb8 => {
                esp = esp.wrapping_add(0x8);
                m.write32(ebx.wrapping_mul(0x4).wrapping_add(0x789440), eax);
                pc = 0x330ec2; continue;
            }
            0x330ec2 => {
                ecx = m.read32(ebx.wrapping_mul(0x4).wrapping_add(0x789440));
                m.write32(ecx.wrapping_add(0x4), edi);
                edx = m.read32(ebx.wrapping_mul(0x4).wrapping_add(0x789440));
                ebx = ebx.wrapping_mul(0x4).wrapping_add(0x789440);
                ecx = edi;
                ecx = ecx.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = ecx;
                m.write32(edx.wrapping_add(0x8), 0);
                edi = m.read32(ebx);
                ecx = ecx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = esi.wrapping_add(0x4);
                edi = edi.wrapping_add(0xc);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                ecx = eax;
                ecx = (ecx & 0x3);
                { let step = if entry.eflags & 0x400 != 0 { u32::MAX } else { 1 }; for i in 0..ecx { m.write8(edi.wrapping_add(i.wrapping_mul(step)), m.read8(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }
                ecx = 0;
                fa = ((((m.read8(ebp.wrapping_add(0x12)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x330f8a; continue; }
                pc = 0x330f06; continue;
            }
            0x330f06 => {
                ecx = esp.wrapping_add(0x18);
                push!(0x528da8);
                push!(ecx);
                call!(0x1531d0);
                pc = 0x330f15; continue;
            }
            0x330f15 => {
                xmm0.lanes[0] = (((m.read32(ebp.wrapping_add(0x14))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                esp = esp.wrapping_add(0x8);
                push!(0);
                push!(0);
                push!(ecx);
                edx = esp.wrapping_add(0x24);
                m.write32(esp, xmm0.lanes[0]);
                push!(edx);
                call!(0x1518f0);
                pc = 0x330f39; continue;
            }
            0x330f39 => {
                edi = m.read32(esp.wrapping_add(0x20));
                esp = esp.wrapping_add(0x10);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x330f64; continue; }
                pc = 0x330f44; continue;
            }
            0x330f44 => {
                esi = 0;
                pc = 0x330f46; continue;
            }
            0x330f46 => {
                eax = m.read32(esp.wrapping_add(0x14));
                ecx = m.read32(eax);
                edx = esi.wrapping_add(ecx).wrapping_add(0x18);
                push!(edx);
                eax = esp.wrapping_add(0x1c);
                push!(eax);
                call!(0x151e80);
                pc = 0x330f5b; continue;
            }
            0x330f5b => {
                esp = esp.wrapping_add(0x8);
                esi = esi.wrapping_add(0x20);
                edi = edi.wrapping_sub(1);
                fa = edi;
                if ((fa != 0)) { pc = 0x330f46; continue; }
                pc = 0x330f64; continue;
            }
            0x330f64 => {
                edi = m.read32(esp.wrapping_add(0x5c));
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x330f8a; continue; }
                pc = 0x330f6c; continue;
            }
            0x330f6c => {
                esi = 0;
                edi = edi;
                pc = 0x330f70; continue;
            }
            0x330f70 => {
                ecx = m.read32(ebx);
                edx = esi.wrapping_add(ecx).wrapping_add(0x18);
                push!(edx);
                eax = esp.wrapping_add(0x1c);
                push!(eax);
                call!(0x151e80);
                pc = 0x330f81; continue;
            }
            0x330f81 => {
                esp = esp.wrapping_add(0x8);
                esi = esi.wrapping_add(0x20);
                edi = edi.wrapping_sub(1);
                fa = edi;
                if ((fa != 0)) { pc = 0x330f70; continue; }
                pc = 0x330f8a; continue;
            }
            0x330f8a => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x48);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x334710. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn load_all_0x2d_tileset_resource_entries_with_zero_arg<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x334710;
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
        { crate::pose::cover(0x334710, previous, pc); previous = pc; }
        match pc {
            0x334710 => {
                push!(esi);
                esi = 0;
                pc = 0x334713; continue;
            }
            0x334713 => {
                push!(0);
                push!(esi);
                call!(0x330df0);
                pc = 0x33471b; continue;
            }
            0x33471b => {
                esp = esp.wrapping_add(0x8);
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ((((0x2di32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x334713; continue; }
                pc = 0x334724; continue;
            }
            0x334724 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x338110. INFERRED name; verification recorded per address in the assets registry.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn preload_object_resources_of_record_58_index_0_count_0x1d<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x338110;
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
        { crate::pose::cover(0x338110, previous, pc); previous = pc; }
        match pc {
            0x338110 => {
                push!(ebx);
                push!(esi);
                push!(edi);
                ebx = 0;
                pc = 0x338115; continue;
            }
            0x338115 => {
                push!(ebx);
                call!(0x311840);
                pc = 0x33811b; continue;
            }
            0x33811b => {
                esp = esp.wrapping_add(0x4);
                fa = ebx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                edi = eax;
                if (fas >= fbs) { pc = 0x338139; continue; }
                pc = 0x338125; continue;
            }
            0x338125 => {
                esi = 0;
                pc = 0x338127; continue;
            }
            0x338127 => {
                eax = esi.wrapping_add(edi);
                push!(eax);
                call!(0xa5e40);
                pc = 0x338130; continue;
            }
            0x338130 => {
                esp = esp.wrapping_add(0x4);
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ((((0x1di32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x338127; continue; }
                pc = 0x338139; continue;
            }
            0x338139 => {
                ebx = ebx.wrapping_add(1);
                fa = ebx;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x338115; continue; }
                pc = 0x33813f; continue;
            }
            0x33813f => {
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
