// SPDX-License-Identifier: GPL-3.0-or-later
//! T1850 (T1815 slice 3): weapon selection predicates (selectable, can switch to).
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
/// 0x1BA410. INFERRED: cdecl `(ext, entry, ignore_ammo) -> bool`. Selectable predicate: not selectable when flag 0x10 or 0x2 (never), not owned, flag 0x100 with an owned x2 of the group (0x1B68F0), or no ammo (reserve + loaded of both hands, skipped when ignore_ammo is 1). Calls 0x1B68F0, 0x1B6950 and 0xE02B0.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn weapon_entry_selectable<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1ba410;
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
        { crate::pose::cover(0x1ba410, previous, pc); previous = pc; }
        match pc {
            0x1ba410 => {
                esp = esp.wrapping_sub(0x2c);
                push!(ebp);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x3c));
                ebp = 0;
                fa = esi;
                fb = ebp;
                if (fa != fb) { pc = 0x1ba439; continue; }
                pc = 0x1ba41f; continue;
            }
            0x1ba41f => {
                ecx = m.read32(esp.wrapping_add(0x38));
                edx = m.read32(ecx.wrapping_add(0x8));
                ecx = m.read32(0x790950);
                eax = 0;
                fa = edx;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = (eax & 0xffff_ff00) | ((((if ((fas >= fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x2c);
                break 'run;
            }
            0x1ba439 => {
                push!(edi);
                edi = 0x1;
                fa = esi;
                fb = edi;
                if (fa != fb) { pc = 0x1ba47c; continue; }
                pc = 0x1ba443; continue;
            }
            0x1ba443 => {
                eax = m.read32(esp.wrapping_add(0x3c));
                ecx = m.read32(eax.wrapping_add(0x8));
                fa = ecx;
                fb = m.read32(0x790950);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1ba473; continue; }
                pc = 0x1ba452; continue;
            }
            0x1ba452 => {
                ecx = m.read32(0x7de458);
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x1ba473; continue; }
                pc = 0x1ba45d; continue;
            }
            0x1ba45d => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x584))) as u32)) & 0xff);
                edx = 0;
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                edx = (edx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = edx;
                esp = esp.wrapping_add(0x2c);
                break 'run;
            }
            0x1ba473 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x2c);
                break 'run;
            }
            0x1ba47c => {
                edx = m.read32(0x790950);
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x40));
                ecx = m.read32(ebx.wrapping_add(0x8));
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1ba4a0; continue; }
                pc = 0x1ba48e; continue;
            }
            0x1ba48e => {
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                fa = ((((m.read8(eax.wrapping_add(0x4fa128)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x1ba7a0; continue; }
                pc = 0x1ba4a0; continue;
            }
            0x1ba4a0 => {
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                m.write32(esp.wrapping_add(0x10), eax);
                eax = m.read32(eax.wrapping_add(0x4fa128));
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x1ba7a0; continue; }
                pc = 0x1ba4b7; continue;
            }
            0x1ba4b7 => {
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1ba4d1; continue; }
                pc = 0x1ba4bb; continue;
            }
            0x1ba4bb => {
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x1ba4d1; continue; }
                pc = 0x1ba4c0; continue;
            }
            0x1ba4c0 => {
                eax = esi;
                edx = ebx;
                call!(0x1b68f0);
                pc = 0x1ba4c9; continue;
            }
            0x1ba4c9 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1ba7a0; continue; }
                pc = 0x1ba4d1; continue;
            }
            0x1ba4d1 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(ebx.wrapping_add(esi).wrapping_add(0x54c))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x1ba7a0; continue; }
                pc = 0x1ba4e0; continue;
            }
            0x1ba4e0 => {
                ecx = m.read32(ebx.wrapping_add(0x98));
                ecx = ((ecx as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = m.read32(ecx.wrapping_add(0x4fa134));
                edx = 0;
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x2c), ebp);
                m.write32(esp.wrapping_add(0x30), ebp);
                m.write32(esp.wrapping_add(0x38), ebp);
                m.write32(esp.wrapping_add(0x24), ebp);
                m.write32(esp.wrapping_add(0x28), ebp);
                m.write32(esp.wrapping_add(0x1c), ebp);
                m.write32(esp.wrapping_add(0x20), ebp);
                m.write32(esp.wrapping_add(0x14), edi);
                m.write32(esp.wrapping_add(0x18), edi);
                if (fa == fb) { pc = 0x1ba55d; continue; }
                pc = 0x1ba51a; continue;
            }
            0x1ba51a => {
                edi = ebx.wrapping_add(0xa4);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba55d; continue; }
                pc = 0x1ba524; continue;
            }
            0x1ba524 => {
                edx = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                esi = m.read32(eax.wrapping_add(edx).wrapping_add(0x1c));
                eax = eax.wrapping_add(edx);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edx = m.read32(eax.wrapping_add(0x14));
                ebp = (((((m.read16(edi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                m.write32(esp.wrapping_add(0x2c), ebp);
                if (fa == fb) { pc = 0x1ba559; continue; }
                pc = 0x1ba546; continue;
            }
            0x1ba546 => {
                fa = esi;
                fb = edx;
                if (fa == fb) { pc = 0x1ba559; continue; }
                pc = 0x1ba54a; continue;
            }
            0x1ba54a => {
                eax = esi;
                esi = (((((m.read16(edi.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                m.write32(esp.wrapping_add(0x24), esi);
                m.write32(esp.wrapping_add(0x1c), eax);
                pc = 0x1ba559; continue;
            }
            0x1ba559 => {
                esi = m.read32(esp.wrapping_add(0x44));
                pc = 0x1ba55d; continue;
            }
            0x1ba55d => {
                ecx = m.read32(ecx.wrapping_add(0x4fa138));
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba5ae; continue; }
                pc = 0x1ba568; continue;
            }
            0x1ba568 => {
                esi = ebx.wrapping_add(0x1d4);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba5aa; continue; }
                pc = 0x1ba572; continue;
            }
            0x1ba572 => {
                eax = m.read32(0x4f9bac);
                ecx = ((ecx as i32).wrapping_mul((0x29c) as i32)) as u32;
                ecx = ecx.wrapping_add(eax);
                eax = m.read32(ecx.wrapping_add(0x14));
                edi = (((((m.read16(esi.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                m.write32(esp.wrapping_add(0x38), eax);
                eax = m.read32(ecx.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x30), edi);
                if (fa == fb) { pc = 0x1ba5aa; continue; }
                pc = 0x1ba596; continue;
            }
            0x1ba596 => {
                fa = eax;
                fb = m.read32(ecx.wrapping_add(0x14));
                if (fa == fb) { pc = 0x1ba5aa; continue; }
                pc = 0x1ba59b; continue;
            }
            0x1ba59b => {
                ecx = eax;
                eax = (((((m.read16(esi.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                m.write32(esp.wrapping_add(0x28), eax);
                m.write32(esp.wrapping_add(0x20), ecx);
                pc = 0x1ba5aa; continue;
            }
            0x1ba5aa => {
                esi = m.read32(esp.wrapping_add(0x44));
                pc = 0x1ba5ae; continue;
            }
            0x1ba5ae => {
                esi = ((esi as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = m.read32(esi.wrapping_add(0x4fa134));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba630; continue; }
                pc = 0x1ba5bc; continue;
            }
            0x1ba5bc => {
                ecx = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                eax = eax.wrapping_add(ecx);
                ecx = m.read32(eax.wrapping_add(0x14));
                edi = (((((m.read16(ebx.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                eax = m.read32(eax.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x14), edi);
                if (fa == fb) { pc = 0x1ba5f6; continue; }
                pc = 0x1ba5e0; continue;
            }
            0x1ba5e0 => {
                fa = eax;
                fb = ecx;
                if (fa == fb) { pc = 0x1ba5f6; continue; }
                pc = 0x1ba5e4; continue;
            }
            0x1ba5e4 => {
                ebp = (((((m.read16(ebx.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                edi = edi.wrapping_add(ebp);
                ebp = m.read32(esp.wrapping_add(0x2c));
                m.write32(esp.wrapping_add(0x14), edi);
                pc = 0x1ba5f6; continue;
            }
            0x1ba5f6 => {
                fa = ecx;
                fb = edx;
                if (fa != fb) { pc = 0x1ba600; continue; }
                pc = 0x1ba5fa; continue;
            }
            0x1ba5fa => {
                edi = edi.wrapping_add(ebp);
                m.write32(esp.wrapping_add(0x14), edi);
                pc = 0x1ba600; continue;
            }
            0x1ba600 => {
                fa = ecx;
                fb = m.read32(esp.wrapping_add(0x38));
                if (fa != fb) { pc = 0x1ba60e; continue; }
                pc = 0x1ba606; continue;
            }
            0x1ba606 => {
                edi = edi.wrapping_add(m.read32(esp.wrapping_add(0x30)));
                m.write32(esp.wrapping_add(0x14), edi);
                pc = 0x1ba60e; continue;
            }
            0x1ba60e => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba634; continue; }
                pc = 0x1ba612; continue;
            }
            0x1ba612 => {
                fa = eax;
                fb = m.read32(esp.wrapping_add(0x1c));
                if (fa != fb) { pc = 0x1ba620; continue; }
                pc = 0x1ba618; continue;
            }
            0x1ba618 => {
                edi = edi.wrapping_add(m.read32(esp.wrapping_add(0x24)));
                m.write32(esp.wrapping_add(0x14), edi);
                pc = 0x1ba620; continue;
            }
            0x1ba620 => {
                fa = eax;
                fb = m.read32(esp.wrapping_add(0x20));
                if (fa != fb) { pc = 0x1ba634; continue; }
                pc = 0x1ba626; continue;
            }
            0x1ba626 => {
                edi = edi.wrapping_add(m.read32(esp.wrapping_add(0x28)));
                m.write32(esp.wrapping_add(0x14), edi);
                pc = 0x1ba634; continue;
            }
            0x1ba630 => {
                edi = m.read32(esp.wrapping_add(0x14));
                pc = 0x1ba634; continue;
            }
            0x1ba634 => {
                eax = m.read32(esi.wrapping_add(0x4fa138));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba6c3; continue; }
                pc = 0x1ba643; continue;
            }
            0x1ba643 => {
                ecx = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                eax = eax.wrapping_add(ecx);
                ecx = m.read32(eax.wrapping_add(0x14));
                esi = (((((m.read16(ebx.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                eax = m.read32(eax.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x18), esi);
                if (fa == fb) { pc = 0x1ba67b; continue; }
                pc = 0x1ba667; continue;
            }
            0x1ba667 => {
                fa = eax;
                fb = ecx;
                if (fa == fb) { pc = 0x1ba67b; continue; }
                pc = 0x1ba66b; continue;
            }
            0x1ba66b => {
                esi = (((((m.read16(ebx.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                m.write32(esp.wrapping_add(0x18), m.read32(esp.wrapping_add(0x18)).wrapping_add(esi));
                edi = m.read32(esp.wrapping_add(0x14));
                pc = 0x1ba67b; continue;
            }
            0x1ba67b => {
                fa = ecx;
                fb = edx;
                if (fa != fb) { pc = 0x1ba683; continue; }
                pc = 0x1ba67f; continue;
            }
            0x1ba67f => {
                m.write32(esp.wrapping_add(0x18), m.read32(esp.wrapping_add(0x18)).wrapping_add(ebp));
                pc = 0x1ba683; continue;
            }
            0x1ba683 => {
                fa = ecx;
                fb = m.read32(esp.wrapping_add(0x38));
                if (fa != fb) { pc = 0x1ba697; continue; }
                pc = 0x1ba689; continue;
            }
            0x1ba689 => {
                edx = m.read32(esp.wrapping_add(0x18));
                ecx = m.read32(esp.wrapping_add(0x30));
                edx = edx.wrapping_add(ecx);
                m.write32(esp.wrapping_add(0x18), edx);
                pc = 0x1ba697; continue;
            }
            0x1ba697 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba6c3; continue; }
                pc = 0x1ba69b; continue;
            }
            0x1ba69b => {
                fa = eax;
                fb = m.read32(esp.wrapping_add(0x1c));
                if (fa != fb) { pc = 0x1ba6af; continue; }
                pc = 0x1ba6a1; continue;
            }
            0x1ba6a1 => {
                ecx = m.read32(esp.wrapping_add(0x18));
                edx = m.read32(esp.wrapping_add(0x24));
                ecx = ecx.wrapping_add(edx);
                m.write32(esp.wrapping_add(0x18), ecx);
                pc = 0x1ba6af; continue;
            }
            0x1ba6af => {
                fa = eax;
                fb = m.read32(esp.wrapping_add(0x20));
                if (fa != fb) { pc = 0x1ba6c3; continue; }
                pc = 0x1ba6b5; continue;
            }
            0x1ba6b5 => {
                ecx = m.read32(esp.wrapping_add(0x18));
                eax = m.read32(esp.wrapping_add(0x28));
                ecx = ecx.wrapping_add(eax);
                m.write32(esp.wrapping_add(0x18), ecx);
                pc = 0x1ba6c3; continue;
            }
            0x1ba6c3 => {
                eax = m.read32(esp.wrapping_add(0x48));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba704; continue; }
                pc = 0x1ba6cb; continue;
            }
            0x1ba6cb => {
                esi = m.read32(esp.wrapping_add(0x44));
                fa = esi;
                fb = ((((0x2fi32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba700; continue; }
                pc = 0x1ba6d4; continue;
            }
            0x1ba6d4 => {
                fa = esi;
                fb = ((((0x2ei32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba700; continue; }
                pc = 0x1ba6d9; continue;
            }
            0x1ba6d9 => {
                fa = esi;
                fb = ((((0x2ci32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba700; continue; }
                pc = 0x1ba6de; continue;
            }
            0x1ba6de => {
                fa = esi;
                fb = ((((0x2bi32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba700; continue; }
                pc = 0x1ba6e3; continue;
            }
            0x1ba6e3 => {
                fa = esi;
                fb = ((((0x2di32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1ba6f9; continue; }
                pc = 0x1ba6e8; continue;
            }
            0x1ba6e8 => {
                push!(ebx);
                call!(0xe02b0);
                pc = 0x1ba6ee; continue;
            }
            0x1ba6ee => {
                esp = esp.wrapping_add(0x4);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                pc = 0x1ba716; continue;
            }
            0x1ba6f9 => {
                eax = 0x1;
                pc = 0x1ba716; continue;
            }
            0x1ba700 => {
                eax = 0;
                pc = 0x1ba716; continue;
            }
            0x1ba704 => {
                ecx = m.read32(esp.wrapping_add(0x44));
                push!(ecx);
                push!(ebx);
                call!(0x1b6950);
                pc = 0x1ba70f; continue;
            }
            0x1ba70f => {
                esi = m.read32(esp.wrapping_add(0x4c));
                esp = esp.wrapping_add(0x8);
                pc = 0x1ba716; continue;
            }
            0x1ba716 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba722; continue; }
                pc = 0x1ba71a; continue;
            }
            0x1ba71a => {
                ecx = m.read32(esp.wrapping_add(0x18));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1ba726; continue; }
                pc = 0x1ba722; continue;
            }
            0x1ba722 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba7a0; continue; }
                pc = 0x1ba726; continue;
            }
            0x1ba726 => {
                eax = m.read32(ebx.wrapping_add(0x80));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba759; continue; }
                pc = 0x1ba730; continue;
            }
            0x1ba730 => {
                eax = m.read32(ebx.wrapping_add(0x94));
                ecx = m.read32(esp.wrapping_add(0x10));
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                edx = m.read32(eax.wrapping_add(0x4fa138));
                fa = edx;
                fb = m.read32(ecx.wrapping_add(0x4fa138));
                if (fa != fb) { pc = 0x1ba759; continue; }
                pc = 0x1ba74b; continue;
            }
            0x1ba74b => {
                eax = m.read32(eax.wrapping_add(0x4fa134));
                fa = eax;
                fb = m.read32(ecx.wrapping_add(0x4fa134));
                if (fa != fb) { pc = 0x1ba7a0; continue; }
                pc = 0x1ba759; continue;
            }
            0x1ba759 => {
                ecx = m.read32(0x790950);
                fa = m.read32(ebx.wrapping_add(0x8));
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1ba793; continue; }
                pc = 0x1ba764; continue;
            }
            0x1ba764 => {
                fa = esi;
                fb = ((((0x31i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1ba793; continue; }
                pc = 0x1ba769; continue;
            }
            0x1ba769 => {
                eax = m.read32(0x4facb4);
                edx = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(edx).wrapping_add(0xc));
                eax = eax.wrapping_add(edx);
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x1ba793; continue; }
                pc = 0x1ba785; continue;
            }
            0x1ba785 => {
                edx = m.read32(eax.wrapping_add(0x1c));
                fa = ((((m.read16(ebx.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x61e)))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x1ba7a0; continue; }
                pc = 0x1ba793; continue;
            }
            0x1ba793 => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x2c);
                break 'run;
            }
            0x1ba7a0 => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x2c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x1BA7B0. INFERRED: cdecl `(ext, entry) -> bool`. Pickup auto-switch policy by mode 0..4 (jump table at 0x1BA8DC, mode from the actor chain [[ext]+0x150]+0x1500+0x68, default 2): compares the switch priorities (+0x8) of the new and the current entry and asks 0x1BA410.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn can_switch_to_entry<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1ba7b0;
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
        { crate::pose::cover(0x1ba7b0, previous, pc); previous = pc; }
        match pc {
            0x1ba7b0 => {
                edx = m.read32(esp.wrapping_add(0x4));
                eax = m.read32(edx);
                eax = m.read32(eax.wrapping_add(0x150));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x4), 0x2);
                if (fa == fb) { pc = 0x1ba7d9; continue; }
                pc = 0x1ba7c8; continue;
            }
            0x1ba7c8 => {
                eax = m.read32(eax.wrapping_add(0x1500));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba7d9; continue; }
                pc = 0x1ba7d2; continue;
            }
            0x1ba7d2 => {
                ecx = m.read32(eax.wrapping_add(0x68));
                m.write32(esp.wrapping_add(0x4), ecx);
                pc = 0x1ba7d9; continue;
            }
            0x1ba7d9 => {
                push!(ebx);
                ebx = m.read32(0x790950);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x10));
                push!(edi);
                edi = m.read32(edx.wrapping_add(0x8));
                fa = edi;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1ba80a; continue; }
                pc = 0x1ba7ed; continue;
            }
            0x1ba7ed => {
                fa = esi;
                fb = ((((0x38i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba7f7; continue; }
                pc = 0x1ba7f2; continue;
            }
            0x1ba7f2 => {
                fa = esi;
                fb = ((((0x39i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1ba80a; continue; }
                pc = 0x1ba7f7; continue;
            }
            0x1ba7f7 => {
                ecx = m.read32(edx.wrapping_add(0x94));
                eax = 0x1;
                fa = ecx;
                fb = eax;
                if (fa == fb) { pc = 0x1ba8d5; continue; }
                pc = 0x1ba80a; continue;
            }
            0x1ba80a => {
                ecx = m.read32(edx.wrapping_add(0x94));
                eax = esi;
                ecx = ((ecx as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                push!(ebp);
                ebp = m.read32(eax.wrapping_add(0x4fa138));
                fa = ebp;
                fb = m.read32(ecx.wrapping_add(0x4fa138));
                if (fa != fb) { pc = 0x1ba834; continue; }
                pc = 0x1ba827; continue;
            }
            0x1ba827 => {
                fa = m.read32(eax.wrapping_add(0x4fa134));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba8d2; continue; }
                pc = 0x1ba834; continue;
            }
            0x1ba834 => {
                ebp = m.read32(esp.wrapping_add(0x14));
                fa = ebp;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x1ba8d2; continue; }
                pc = 0x1ba841; continue;
            }
            0x1ba841 => {
                jt = m.read32(ebp.wrapping_mul(0x4).wrapping_add(0x1ba8dc));
                if (jt == 0x1ba848u32) { pc = 0x1ba848; continue; }
                if (jt == 0x1ba859u32) { pc = 0x1ba859; continue; }
                if (jt == 0x1ba872u32) { pc = 0x1ba872; continue; }
                if (jt == 0x1ba8bcu32) { pc = 0x1ba8bc; continue; }
                if (jt == 0x1ba8d2u32) { pc = 0x1ba8d2; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x1ba848 => {
                push!(0);
                push!(esi);
                push!(edx);
                call!(0x1ba410);
                pc = 0x1ba851; continue;
            }
            0x1ba851 => {
                esp = esp.wrapping_add(0xc);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1ba859 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1ba8d2; continue; }
                pc = 0x1ba861; continue;
            }
            0x1ba861 => {
                push!(0);
                push!(esi);
                push!(edx);
                call!(0x1ba410);
                pc = 0x1ba86a; continue;
            }
            0x1ba86a => {
                esp = esp.wrapping_add(0xc);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1ba872 => {
                ebp = m.read32(esp.wrapping_add(0x1c));
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1ba8d2; continue; }
                pc = 0x1ba87a; continue;
            }
            0x1ba87a => {
                fa = edi;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1ba89d; continue; }
                pc = 0x1ba87e; continue;
            }
            0x1ba87e => {
                eax = m.read32(eax.wrapping_add(0x4fa130));
                fa = eax;
                fb = m.read32(ecx.wrapping_add(0x4fa130));
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x1ba8d2; continue; }
                pc = 0x1ba88c; continue;
            }
            0x1ba88c => {
                push!(0);
                push!(esi);
                push!(edx);
                call!(0x1ba410);
                pc = 0x1ba895; continue;
            }
            0x1ba895 => {
                esp = esp.wrapping_add(0xc);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1ba89d => {
                eax = m.read32(eax.wrapping_add(0x4fa12c));
                fa = eax;
                fb = m.read32(ecx.wrapping_add(0x4fa12c));
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x1ba8d2; continue; }
                pc = 0x1ba8ab; continue;
            }
            0x1ba8ab => {
                push!(0);
                push!(esi);
                push!(edx);
                call!(0x1ba410);
                pc = 0x1ba8b4; continue;
            }
            0x1ba8b4 => {
                esp = esp.wrapping_add(0xc);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1ba8bc => {
                fa = edi;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1ba87e; continue; }
                pc = 0x1ba8c0; continue;
            }
            0x1ba8c0 => {
                eax = m.read32(eax.wrapping_add(0x4fa12c));
                fa = eax;
                fb = m.read32(ecx.wrapping_add(0x4fa12c));
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x1ba848; continue; }
                pc = 0x1ba8d2; continue;
            }
            0x1ba8d2 => {
                eax = 0;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x1ba8d5; continue;
            }
            0x1ba8d5 => {
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


// ---- translated bodies end ----

// ---- dispatch begin ----
/// Roots ported in this module (differential-only, unadmitted).
pub const ROOTS: [u32; 2] = [0x1ba410, 0x1ba7b0];

/// Differential entry (not selector-registered). True when the Rust body ran.
#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0x1ba410 => weapon_entry_selectable(m, guest, regs),
        0x1ba7b0 => can_switch_to_entry(m, guest, regs),
        _ => return false,
    }
    true
}
// ---- dispatch end ----
