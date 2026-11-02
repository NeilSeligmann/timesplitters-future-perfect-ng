// SPDX-License-Identifier: GPL-3.0-or-later
//! T1850 (T1815 slice 3): weapon grant, collect and world pickup.
//! Same provenance, labels and conventions as inventory.rs (MEASURED against the lifted originals on
//! FABRICATED memory with stub callees, names INFERRED, not admitted).
#![allow(clippy::all, unused_parens, unused_assignments, unused_imports, unused_variables)]
use crate::{
    abi::{ExtRegisters, GuestCall, Xmm},
    memory::GuestMemory,
    pose::write16,
};
#[allow(unused_imports)]
use crate::inventory::sse;

// ---- translated bodies begin ----
/// 0x1BFAA0. INFERRED: cdecl `(ext, entry, default_throwable)`. Marks owned bytes of the entry's group (x2 entries only when acquired as x2), replaces the order byte when the old representative fails 0x1BA410, sets the default throwable (+0xA0) and on first acquisition writes the clip-size caps ext+0x134 / +0x264 from the entry records.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn weapon_entry_acquire<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1bfaa0;
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
        { crate::pose::cover(0x1bfaa0, previous, pc); previous = pc; }
        match pc {
            0x1bfaa0 => {
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x14));
                eax = ((m.read8(esi.wrapping_add(ebp).wrapping_add(0x54c))) as u8 as u32);
                esi = ((esi as i32).wrapping_mul((0x3c) as i32)) as u32;
                ebx = m.read32(esi.wrapping_add(0x4fa160));
                m.write32(esp.wrapping_add(0x10), eax);
                push!(edi);
                ecx = 0;
                eax = 0x4fa128;
                edx = (edx & 0xffff_ff00) | ((0x4) & 0xff);
                pc = 0x1bfad0; continue;
            }
            0x1bfad0 => {
                fa = ebx;
                fb = m.read32(eax.wrapping_add(0x38));
                if (fa != fb) { pc = 0x1bfae9; continue; }
                pc = 0x1bfad5; continue;
            }
            0x1bfad5 => {
                fa = ((((m.read8(eax))) as u32) & 0xffu32);
                fb = ((((((edx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x1bfae1; continue; }
                pc = 0x1bfad9; continue;
            }
            0x1bfad9 => {
                fa = ((((m.read8(esi.wrapping_add(0x4fa128)))) as u32) & 0xffu32);
                fb = ((((((edx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x1bfae9; continue; }
                pc = 0x1bfae1; continue;
            }
            0x1bfae1 => {
                m.write8(ecx.wrapping_add(ebp).wrapping_add(0x54c), (0x1) as u8);
                pc = 0x1bfae9; continue;
            }
            0x1bfae9 => {
                eax = eax.wrapping_add(0x3c);
                ecx = ecx.wrapping_add(1);
                fa = eax;
                fb = ((((0x4fb190i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1bfad0; continue; }
                pc = 0x1bfaf4; continue;
            }
            0x1bfaf4 => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bfb58; continue; }
                pc = 0x1bfaf8; continue;
            }
            0x1bfaf8 => {
                eax = m.read32(ebp.wrapping_add(0x8));
                fa = eax;
                fb = m.read32(0x790950);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1bfb58; continue; }
                pc = 0x1bfb03; continue;
            }
            0x1bfb03 => {
                edx = m.read32(0x7b0c48);
                eax = ((eax as i32).wrapping_mul((0x1584) as i32)) as u32;
                ecx = m.read32(esp.wrapping_add(0x18));
                eax = eax.wrapping_add(edx);
                edi = eax;
                eax = ((m.read8(edi.wrapping_add(ebx).wrapping_add(0xe3c))) as u8 as u32);
                fa = eax;
                fb = ecx;
                if (fa == fb) { pc = 0x1bfb3e; continue; }
                pc = 0x1bfb23; continue;
            }
            0x1bfb23 => {
                push!(0x1);
                push!(eax);
                push!(ebp);
                call!(0x1ba410);
                pc = 0x1bfb2c; continue;
            }
            0x1bfb2c => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bfb3e; continue; }
                pc = 0x1bfb33; continue;
            }
            0x1bfb33 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(esp.wrapping_add(0x18))) as u32)) & 0xff);
                m.write8(edi.wrapping_add(ebx).wrapping_add(0xe3c), (((((ecx) as u8)) as u32)) as u8);
                pc = 0x1bfb3e; continue;
            }
            0x1bfb3e => {
                eax = m.read32(esp.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bfb58; continue; }
                pc = 0x1bfb46; continue;
            }
            0x1bfb46 => {
                fa = ((((m.read8(esi.wrapping_add(0x4fa128)))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x1bfb61; continue; }
                pc = 0x1bfb4f; continue;
            }
            0x1bfb4f => {
                push!(edi);
                call!(0x1427d0);
                pc = 0x1bfb55; continue;
            }
            0x1bfb55 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x1bfb58; continue;
            }
            0x1bfb58 => {
                fa = ((((m.read8(esi.wrapping_add(0x4fa128)))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x1bfb75; continue; }
                pc = 0x1bfb61; continue;
            }
            0x1bfb61 => {
                eax = m.read32(ebp.wrapping_add(0xa0));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bfb75; continue; }
                pc = 0x1bfb6b; continue;
            }
            0x1bfb6b => {
                edx = m.read32(esp.wrapping_add(0x18));
                m.write32(ebp.wrapping_add(0xa0), edx);
                pc = 0x1bfb75; continue;
            }
            0x1bfb75 => {
                eax = m.read32(esp.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bfbfb; continue; }
                pc = 0x1bfb7d; continue;
            }
            0x1bfb7d => {
                eax = m.read32(esi.wrapping_add(0x4fa134));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1bfbbc; continue; }
                pc = 0x1bfb87; continue;
            }
            0x1bfb87 => {
                edi = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(edi).wrapping_add(0x14));
                eax = eax.wrapping_add(edi);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bfba9; continue; }
                pc = 0x1bfb9d; continue;
            }
            0x1bfb9d => {
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x18))) as u32)) & 0xffff);
                write16(m, ebp.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x134), (((((edx) as u16)) as u32)) as u16);
                pc = 0x1bfba9; continue;
            }
            0x1bfba9 => {
                ecx = m.read32(eax.wrapping_add(0x1c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bfbbc; continue; }
                pc = 0x1bfbb0; continue;
            }
            0x1bfbb0 => {
                eax = (eax & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x20))) as u32)) & 0xffff);
                write16(m, ebp.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x134), (((((eax) as u16)) as u32)) as u16);
                pc = 0x1bfbbc; continue;
            }
            0x1bfbbc => {
                eax = m.read32(esi.wrapping_add(0x4fa138));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1bfbfb; continue; }
                pc = 0x1bfbc6; continue;
            }
            0x1bfbc6 => {
                esi = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(esi).wrapping_add(0x14));
                eax = eax.wrapping_add(esi);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bfbe8; continue; }
                pc = 0x1bfbdc; continue;
            }
            0x1bfbdc => {
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x18))) as u32)) & 0xffff);
                write16(m, ebp.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x264), (((((edx) as u16)) as u32)) as u16);
                pc = 0x1bfbe8; continue;
            }
            0x1bfbe8 => {
                ecx = m.read32(eax.wrapping_add(0x1c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bfbfb; continue; }
                pc = 0x1bfbef; continue;
            }
            0x1bfbef => {
                eax = (eax & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x20))) as u32)) & 0xffff);
                write16(m, ebp.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x264), (((((eax) as u16)) as u32)) as u16);
                pc = 0x1bfbfb; continue;
            }
            0x1bfbfb => {
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


/// 0x1BFCA0. INFERRED: cdecl `(ext, entry)`. Grant (0x1BFAA0), collect latch ext+0x28 |= 8, auto-switch decision (0x1BA7B0) with 0x1BA130, HUD message, pickup sound and stat series. The per-entry unlock callback (table 0x4FB1E0) is called through the guest call interface.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn weapon_entry_collect<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1bfca0;
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
        { crate::pose::cover(0x1bfca0, previous, pc); previous = pc; }
        match pc {
            0x1bfca0 => {
                esp = esp.wrapping_sub(0xc);
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x18));
                push!(ebp);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x1c));
                ebp = ((m.read8(edi.wrapping_add(ebx).wrapping_add(0x54c))) as u8 as u32);
                push!(ebx);
                push!(edi);
                m.write32(esp.wrapping_add(0x28), ebp);
                call!(0x1bfaa0);
                pc = 0x1bfcc1; continue;
            }
            0x1bfcc1 => {
                edx = m.read32(edi.wrapping_add(0x28));
                push!(ebp);
                edx = (edx | 0x8);
                push!(ebx);
                push!(edi);
                m.write32(edi.wrapping_add(0x28), edx);
                call!(0x1ba7b0);
                pc = 0x1bfcd2; continue;
            }
            0x1bfcd2 => {
                esp = esp.wrapping_add(0x14);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bfce7; continue; }
                pc = 0x1bfcd9; continue;
            }
            0x1bfcd9 => {
                fa = ebx;
                fb = ((((0x22i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bfd03; continue; }
                pc = 0x1bfcde; continue;
            }
            0x1bfcde => {
                call!(0x64d10);
                pc = 0x1bfce3; continue;
            }
            0x1bfce3 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bfd03; continue; }
                pc = 0x1bfce7; continue;
            }
            0x1bfce7 => {
                eax = m.read32(edi);
                ecx = m.read32(eax.wrapping_add(0x4));
                push!(0x3e99999a);
                push!(0x2);
                push!(ecx);
                call!(0x5c1c0);
                pc = 0x1bfcf9; continue;
            }
            0x1bfcf9 => {
                push!(ebx);
                push!(edi);
                call!(0x1ba130);
                pc = 0x1bfd00; continue;
            }
            0x1bfd00 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x1bfd03; continue;
            }
            0x1bfd03 => {
                eax = m.read32(edi.wrapping_add(0x8));
                fa = eax;
                fb = m.read32(0x790950);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1bfe37; continue; }
                pc = 0x1bfd12; continue;
            }
            0x1bfd12 => {
                ecx = m.read32(0x7b0c48);
                eax = ((eax as i32).wrapping_mul((0x1584) as i32)) as u32;
                eax = eax.wrapping_add(ecx);
                push!(esi);
                esi = eax;
                eax = m.read32(esi.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x20), esi);
                if (fas < fbs) { pc = 0x1bfdef; continue; }
                pc = 0x1bfd32; continue;
            }
            0x1bfd32 => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bfd3f; continue; }
                pc = 0x1bfd36; continue;
            }
            0x1bfd36 => {
                fa = ebx;
                fb = ((((0x2fi32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bfdef; continue; }
                pc = 0x1bfd3f; continue;
            }
            0x1bfd3f => {
                fa = m.read32(0x7de318);
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1bfdef; continue; }
                pc = 0x1bfd4c; continue;
            }
            0x1bfd4c => {
                call!(0x1cdae0);
                pc = 0x1bfd51; continue;
            }
            0x1bfd51 => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bfd91; continue; }
                pc = 0x1bfd56; continue;
            }
            0x1bfd56 => {
                call!(0x1cdae0);
                pc = 0x1bfd5b; continue;
            }
            0x1bfd5b => {
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bfd67; continue; }
                pc = 0x1bfd60; continue;
            }
            0x1bfd60 => {
                eax = m.read32(esi.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x1bfd91; continue; }
                pc = 0x1bfd67; continue;
            }
            0x1bfd67 => {
                esi = ebx;
                esi = ((esi as i32).wrapping_mul((0x54) as i32)) as u32;
                edx = m.read32(esi.wrapping_add(0x4fb190));
                push!(edx);
                call!(0x3d7b0);
                pc = 0x1bfd78; continue;
            }
            0x1bfd78 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x18fc);
                call!(0x3d7b0);
                pc = 0x1bfd86; continue;
            }
            0x1bfd86 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x478904);
                pc = 0x1bfda8; continue;
            }
            0x1bfd91 => {
                esi = ebx;
                esi = ((esi as i32).wrapping_mul((0x54) as i32)) as u32;
                eax = m.read32(esi.wrapping_add(0x4fb190));
                push!(eax);
                call!(0x3d7b0);
                pc = 0x1bfda2; continue;
            }
            0x1bfda2 => {
                push!(eax);
                push!(0x475d88);
                pc = 0x1bfda8; continue;
            }
            0x1bfda8 => {
                call!(0x151030);
                pc = 0x1bfdad; continue;
            }
            0x1bfdad => {
                ebp = eax;
                eax = (eax & 0xffff_ff00) | ((((m.read8(0x7de455)) as u32)) & 0xff);
                esp = esp.wrapping_add(0xc);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x1bfdda; continue; }
                pc = 0x1bfdbb; continue;
            }
            0x1bfdbb => {
                eax = m.read32(esp.wrapping_add(0x24));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bfdda; continue; }
                pc = 0x1bfdc3; continue;
            }
            0x1bfdc3 => {
                esi = m.read32(esi.wrapping_add(0x4fb1e0));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bfdda; continue; }
                pc = 0x1bfdcd; continue;
            }
            0x1bfdcd => {
                ecx = m.read32(esp.wrapping_add(0x20));
                edx = m.read32(ecx.wrapping_add(0x4));
                push!(edx);
                call_to!(esi);
                pc = 0x1bfdd7; continue;
            }
            0x1bfdd7 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x1bfdda; continue;
            }
            0x1bfdda => {
                eax = m.read32(esp.wrapping_add(0x20));
                ecx = m.read32(eax.wrapping_add(0x4));
                push!(0);
                push!(ebp);
                push!(0x2);
                push!(ecx);
                call!(0x12d350);
                pc = 0x1bfdec; continue;
            }
            0x1bfdec => {
                esp = esp.wrapping_add(0x10);
                pc = 0x1bfdef; continue;
            }
            0x1bfdef => {
                fa = m.read32(0x7de458);
                fb = ((((0x60000000i32)) as u32) & 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                if ((fa & fb) != 0) { pc = 0x1bfe37; continue; }
                pc = 0x1bfdfc; continue;
            }
            0x1bfdfc => {
                edx = esp.wrapping_add(0xc);
                push!(edx);
                push!(edi);
                call!(0x45cf0);
                pc = 0x1bfe07; continue;
            }
            0x1bfe07 => {
                ecx = m.read32(0x7e71f0);
                edx = ebx;
                edx = ((edx as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = esp.wrapping_add(0x14);
                push!(eax);
                eax = m.read32(edx.wrapping_add(0x4fa138));
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                push!(ecx);
                ecx = m.read32(0x4f9bac);
                edx = m.read32(eax.wrapping_add(ecx).wrapping_add(0x30));
                push!(edx);
                call!(0x21910);
                pc = 0x1bfe34; continue;
            }
            0x1bfe34 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x1bfe37; continue;
            }
            0x1bfe37 => {
                eax = m.read32(edi.wrapping_add(0x8));
                push!(0x3f800000);
                push!(ebx);
                push!(eax);
                push!(0x4);
                call!(0x2385b0);
                pc = 0x1bfe48; continue;
            }
            0x1bfe48 => {
                esp = esp.wrapping_add(0x10);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0xC0310. INFERRED: cdecl `(pickup, collector, out_a, out_b) -> bool`. World pickup of a weapon: adds the pickup's two ammo pairs through 0x1BA220, refuses when already owned and nothing was added, then collects through 0x1BFCA0 (and selects through 0x1BA130).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pickup_weapon_collect<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xc0310;
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
        { crate::pose::cover(0xc0310, previous, pc); previous = pc; }
        match pc {
            0xc0310 => {
                push!(ecx);
                eax = m.read32(esp.wrapping_add(0xc));
                ecx = m.read32(esp.wrapping_add(0x8));
                push!(ebx);
                ebx = m.read32(eax.wrapping_add(0x7c));
                push!(esi);
                esi = m.read32(ecx.wrapping_add(0x7c));
                eax = m.read32(esi.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xc0336; continue; }
                pc = 0xc0328; continue;
            }
            0xc0328 => {
                fa = eax;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xc0336; continue; }
                pc = 0xc032d; continue;
            }
            0xc032d => {
                push!(eax);
                call!(0x6bec0);
                pc = 0xc0333; continue;
            }
            0xc0333 => {
                esp = esp.wrapping_add(0x4);
                pc = 0xc0336; continue;
            }
            0xc0336 => {
                fa = ((((m.read8(0x7de456))) as u32) & 0xffu32);
                fb = ((((0x11i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0xc0351; continue; }
                pc = 0xc033f; continue;
            }
            0xc033f => {
                edx = ((m.read8(0x7de498)) as u8 as u32);
                fa = m.read32(ebx.wrapping_add(0x8));
                fb = edx;
                if (fa != fb) { pc = 0xc0351; continue; }
                pc = 0xc034b; continue;
            }
            0xc034b => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0xc0351 => {
                eax = m.read32(ebx.wrapping_add(0x44));
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                push!(ebp);
                if (fa != fb) { pc = 0xc0370; continue; }
                pc = 0xc035a; continue;
            }
            0xc035a => {
                ecx = m.read32(esi.wrapping_add(0x14));
                edx = m.read32(0x4f9bac);
                ecx = ((ecx as i32).wrapping_mul((0x29c) as i32)) as u32;
                fa = ((((m.read8(ecx.wrapping_add(edx).wrapping_add(0xc)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0xc0378; continue; }
                pc = 0xc0370; continue;
            }
            0xc0370 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xc0447; continue; }
                pc = 0xc0378; continue;
            }
            0xc0378 => {
                edx = m.read32(0x790950);
                ecx = m.read32(ebx.wrapping_add(0x8));
                eax = 0;
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                edx = m.read32(esi.wrapping_add(0x44));
                eax = (eax & 0xffff_ff00) | ((((if ((fas >= fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(edi);
                edi = (((((m.read8(esi.wrapping_add(0x10)) as i8)) as i32)) as u32);
                push!(edx);
                edi = (edi & 0x10);
                m.write32(esp.wrapping_add(0x14), eax);
                eax = m.read32(esi.wrapping_add(0x40));
                push!(eax);
                push!(ebx);
                edi = edi.wrapping_shr((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                call!(0x1ba220);
                pc = 0xc03a5; continue;
            }
            0xc03a5 => {
                ecx = m.read32(esp.wrapping_add(0x1c));
                edx = m.read32(esi.wrapping_add(0x48));
                edi = (edi | ecx);
                ecx = m.read32(esi.wrapping_add(0x4c));
                push!(ecx);
                push!(edx);
                ebp = eax;
                push!(ebx);
                ebp = ebp.wrapping_add(edi);
                call!(0x1ba220);
                pc = 0xc03bd; continue;
            }
            0xc03bd => {
                ebp = ebp.wrapping_add(eax);
                eax = m.read32(esi.wrapping_add(0x14));
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(ebx).wrapping_add(0x54c))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x18);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0xc03f3; continue; }
                pc = 0xc03d1; continue;
            }
            0xc03d1 => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xc03f3; continue; }
                pc = 0xc03d5; continue;
            }
            0xc03d5 => {
                call!(0x3533f0);
                pc = 0xc03da; continue;
            }
            0xc03da => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xc0447; continue; }
                pc = 0xc03de; continue;
            }
            0xc03de => {
                ecx = m.read32(esp.wrapping_add(0x18));
                eax = m.read32(ecx.wrapping_add(0x150));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xc0447; continue; }
                pc = 0xc03ec; continue;
            }
            0xc03ec => {
                ecx = m.read32(eax.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xc0447; continue; }
                pc = 0xc03f3; continue;
            }
            0xc03f3 => {
                edx = m.read32(esp.wrapping_add(0x14));
                push!(edx);
                call!(0xbf360);
                pc = 0xc03fd; continue;
            }
            0xc03fd => {
                eax = m.read32(esi.wrapping_add(0x14));
                push!(eax);
                push!(ebx);
                call!(0x1bfca0);
                pc = 0xc0407; continue;
            }
            0xc0407 => {
                eax = m.read32(esi.wrapping_add(0x18));
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xc0423; continue; }
                pc = 0xc0411; continue;
            }
            0xc0411 => {
                fa = ((((m.read8(eax))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0xc0423; continue; }
                pc = 0xc0416; continue;
            }
            0xc0416 => {
                ecx = m.read32(esi.wrapping_add(0x14));
                push!(ecx);
                push!(ebx);
                call!(0x1ba130);
                pc = 0xc0420; continue;
            }
            0xc0420 => {
                esp = esp.wrapping_add(0x8);
                pc = 0xc0423; continue;
            }
            0xc0423 => {
                fa = ((((m.read8(esi.wrapping_add(0x10)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0xc0433; continue; }
                pc = 0xc0429; continue;
            }
            0xc0429 => {
                edx = m.read32(esp.wrapping_add(0x20));
                m.write32(edx, 0x1);
                pc = 0xc0433; continue;
            }
            0xc0433 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax, 0x1);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0xc0447 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
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


// ---- translated bodies end ----

// ---- dispatch begin ----
/// Roots ported in this module (differential-only, unadmitted).
pub const ROOTS: [u32; 3] = [0x1bfaa0, 0x1bfca0, 0xc0310];

/// Differential entry (not selector-registered). True when the Rust body ran.
#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0x1bfaa0 => weapon_entry_acquire(m, guest, regs),
        0x1bfca0 => weapon_entry_collect(m, guest, regs),
        0xc0310 => pickup_weapon_collect(m, guest, regs),
        _ => return false,
    }
    true
}
// ---- dispatch end ----
