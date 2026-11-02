// SPDX-License-Identifier: GPL-3.0-or-later
//! T1850 (T1815 slice 3): world pickups, loadout grant and inventory save/restore.
//! Same provenance, labels and conventions as inventory.rs / pose.rs (MEASURED against the lifted originals on
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
/// 0xBE090. INFERRED: calls game_ammo_add_to_reserve_clamped_to_type_max_returns_added, game_net_is_session_active_76b108, 0x1bfca0 and reads 0x4f9bac and 0x790950 slot globals
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pickup_weapon_ammo_collect_if_not_full<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xbe090;
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
        { crate::pose::cover(0xbe090, previous, pc); previous = pc; }
        match pc {
            0xbe090 => {
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x7c));
                eax = m.read32(esi.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(edi);
                edi = m.read32(ebp.wrapping_add(0x7c));
                if (fas <= fbs) { pc = 0xbe0b3; continue; }
                pc = 0xbe0a5; continue;
            }
            0xbe0a5 => {
                fa = eax;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xbe0b3; continue; }
                pc = 0xbe0aa; continue;
            }
            0xbe0aa => {
                push!(eax);
                call!(0x6bec0);
                pc = 0xbe0b0; continue;
            }
            0xbe0b0 => {
                esp = esp.wrapping_add(0x4);
                pc = 0xbe0b3; continue;
            }
            0xbe0b3 => {
                eax = m.read32(edi.wrapping_add(0x44));
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xbe0d1; continue; }
                pc = 0xbe0bb; continue;
            }
            0xbe0bb => {
                ecx = m.read32(esi.wrapping_add(0x14));
                edx = m.read32(0x4f9bac);
                ecx = ((ecx as i32).wrapping_mul((0x29c) as i32)) as u32;
                fa = ((((m.read8(ecx.wrapping_add(edx).wrapping_add(0xc)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0xbe0d9; continue; }
                pc = 0xbe0d1; continue;
            }
            0xbe0d1 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xbe175; continue; }
                pc = 0xbe0d9; continue;
            }
            0xbe0d9 => {
                eax = m.read32(edi.wrapping_add(0x8));
                fa = ((((m.read16(esi.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0x50)))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0xbe175; continue; }
                pc = 0xbe0e8; continue;
            }
            0xbe0e8 => {
                edx = m.read32(0x790950);
                ecx = m.read32(esi.wrapping_add(0x40));
                ebx = 0;
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = m.read32(esi.wrapping_add(0x44));
                push!(eax);
                push!(ecx);
                push!(edi);
                ebx = (ebx & 0xffff_ff00) | ((((if ((fas >= fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                call!(0x1ba220);
                pc = 0xbe103; continue;
            }
            0xbe103 => {
                edx = m.read32(esi.wrapping_add(0x4c));
                ebx = ebx.wrapping_add(eax);
                eax = m.read32(esi.wrapping_add(0x48));
                push!(edx);
                push!(eax);
                push!(edi);
                call!(0x1ba220);
                pc = 0xbe113; continue;
            }
            0xbe113 => {
                ecx = m.read32(esi.wrapping_add(0x14));
                ebx = ebx.wrapping_add(eax);
                eax = (eax & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(edi).wrapping_add(0x54c))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x18);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0xbe144; continue; }
                pc = 0xbe126; continue;
            }
            0xbe126 => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xbe144; continue; }
                pc = 0xbe12a; continue;
            }
            0xbe12a => {
                call!(0x3533f0);
                pc = 0xbe12f; continue;
            }
            0xbe12f => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xbe175; continue; }
                pc = 0xbe133; continue;
            }
            0xbe133 => {
                eax = m.read32(ebp.wrapping_add(0x150));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xbe175; continue; }
                pc = 0xbe13d; continue;
            }
            0xbe13d => {
                ecx = m.read32(eax.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xbe175; continue; }
                pc = 0xbe144; continue;
            }
            0xbe144 => {
                edx = m.read32(esi.wrapping_add(0x14));
                push!(edx);
                push!(edi);
                call!(0x1bfca0);
                pc = 0xbe14e; continue;
            }
            0xbe14e => {
                eax = m.read32(esp.wrapping_add(0x20));
                m.write32(eax, 0x1);
                eax = m.read32(0x4e8cec);
                edx = m.read32(edi.wrapping_add(0x8));
                esp = esp.wrapping_add(0x8);
                ecx = eax.wrapping_add(eax.wrapping_mul(0x4));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                write16(m, esi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x50), (((((ecx) as u16)) as u32)) as u16);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0xbe175 => {
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


/// 0xBFB40. INFERRED: adds ammo via 1ba220 1bfec0 for slots [ext+0x48] [ext+0x40], text 0x18fc 0x18fd with format '%s %s' via 151030, stat 0x24, calls bf360, returns 1 if taken
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pickup_collect_weapon_ammo_add_with_message_an<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xbfb40;
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
        { crate::pose::cover(0xbfb40, previous, pc); previous = pc; }
        match pc {
            0xbfb40 => {
                esp = esp.wrapping_sub(0x10);
                eax = m.read32(esp.wrapping_add(0x14));
                ecx = m.read32(esp.wrapping_add(0x18));
                push!(ebx);
                push!(ebp);
                ebp = m.read32(ecx.wrapping_add(0x7c));
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x7c));
                eax = m.read32(esi.wrapping_add(0x48));
                push!(edi);
                edi = (((((m.read8(esi.wrapping_add(0x10)) as i8)) as i32)) as u32);
                edi = (edi & 0x10);
                edi = edi.wrapping_shr((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), 0);
                if (fas <= fbs) { pc = 0xbfbab; continue; }
                pc = 0xbfb6e; continue;
            }
            0xbfb6e => {
                fa = eax;
                fb = ((((0x2ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xbfbab; continue; }
                pc = 0xbfb73; continue;
            }
            0xbfb73 => {
                ecx = m.read32(esi.wrapping_add(0x4c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xbfbab; continue; }
                pc = 0xbfb7a; continue;
            }
            0xbfb7a => {
                ebx = (((((m.read16(ebp.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                push!(ecx);
                push!(eax);
                push!(ebp);
                call!(0x1ba220);
                pc = 0xbfb8a; continue;
            }
            0xbfb8a => {
                esp = esp.wrapping_add(0xc);
                edi = edi.wrapping_add(eax);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xbfb99; continue; }
                pc = 0xbfb93; continue;
            }
            0xbfb93 => {
                fa = ((((m.read8(esi.wrapping_add(0x10)))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0xbfbab; continue; }
                pc = 0xbfb99; continue;
            }
            0xbfb99 => {
                edx = m.read32(esi.wrapping_add(0x48));
                push!(ebx);
                push!(edx);
                push!(ebp);
                call!(0x1bfec0);
                pc = 0xbfba4; continue;
            }
            0xbfba4 => {
                esp = esp.wrapping_add(0xc);
                m.write32(esp.wrapping_add(0x10), eax);
                pc = 0xbfbab; continue;
            }
            0xbfbab => {
                eax = m.read32(esi.wrapping_add(0x40));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xbfbf5; continue; }
                pc = 0xbfbb2; continue;
            }
            0xbfbb2 => {
                fa = eax;
                fb = ((((0x2ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xbfbf5; continue; }
                pc = 0xbfbb7; continue;
            }
            0xbfbb7 => {
                ecx = m.read32(esi.wrapping_add(0x44));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xbfbf5; continue; }
                pc = 0xbfbbe; continue;
            }
            0xbfbbe => {
                ebx = (((((m.read16(ebp.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                push!(ecx);
                push!(eax);
                push!(ebp);
                call!(0x1ba220);
                pc = 0xbfbce; continue;
            }
            0xbfbce => {
                esp = esp.wrapping_add(0xc);
                edi = edi.wrapping_add(eax);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xbfbdd; continue; }
                pc = 0xbfbd7; continue;
            }
            0xbfbd7 => {
                fa = ((((m.read8(esi.wrapping_add(0x10)))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0xbfbf5; continue; }
                pc = 0xbfbdd; continue;
            }
            0xbfbdd => {
                eax = m.read32(esi.wrapping_add(0x40));
                push!(ebx);
                push!(eax);
                push!(ebp);
                call!(0x1bfec0);
                pc = 0xbfbe8; continue;
            }
            0xbfbe8 => {
                ecx = m.read32(esp.wrapping_add(0x1c));
                esp = esp.wrapping_add(0xc);
                ecx = ecx.wrapping_add(eax);
                m.write32(esp.wrapping_add(0x10), ecx);
                pc = 0xbfbf5; continue;
            }
            0xbfbf5 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xbfd8b; continue; }
                pc = 0xbfbfd; continue;
            }
            0xbfbfd => {
                eax = m.read32(esi.wrapping_add(0x40));
                fa = eax;
                fb = ((((0x21i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xbfc0a; continue; }
                pc = 0xbfc05; continue;
            }
            0xbfc05 => {
                fa = eax;
                fb = ((((0x20i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xbfc1f; continue; }
                pc = 0xbfc0a; continue;
            }
            0xbfc0a => {
                push!(0x1);
                push!(esi);
                call!(0xbdc40);
                pc = 0xbfc12; continue;
            }
            0xbfc12 => {
                ecx = m.read32(esp.wrapping_add(0x38));
                esp = esp.wrapping_add(0x8);
                m.write32(ecx, 0x1);
                pc = 0xbfc1f; continue;
            }
            0xbfc1f => {
                edx = m.read32(esp.wrapping_add(0x24));
                push!(edx);
                call!(0xbf360);
                pc = 0xbfc29; continue;
            }
            0xbfc29 => {
                edi = m.read32(esp.wrapping_add(0x2c));
                eax = m.read32(edi.wrapping_add(0x150));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xbfd61; continue; }
                pc = 0xbfc3e; continue;
            }
            0xbfc3e => {
                eax = m.read32(esi.wrapping_add(0x40));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xbfc4a; continue; }
                pc = 0xbfc45; continue;
            }
            0xbfc45 => {
                fa = eax;
                fb = ((((0x2ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0xbfc5e; continue; }
                pc = 0xbfc4a; continue;
            }
            0xbfc4a => {
                eax = m.read32(esi.wrapping_add(0x48));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xbfd61; continue; }
                pc = 0xbfc55; continue;
            }
            0xbfc55 => {
                fa = eax;
                fb = ((((0x2ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xbfd61; continue; }
                pc = 0xbfc5e; continue;
            }
            0xbfc5e => {
                eax = m.read32(esp.wrapping_add(0x10));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xbfd61; continue; }
                pc = 0xbfc6a; continue;
            }
            0xbfc6a => {
                eax = esp.wrapping_add(0x14);
                push!(eax);
                push!(ebp);
                call!(0x45cf0);
                pc = 0xbfc75; continue;
            }
            0xbfc75 => {
                ecx = esp.wrapping_add(0x1c);
                push!(ecx);
                push!(0xd38);
                call!(0x217b0);
                pc = 0xbfc84; continue;
            }
            0xbfc84 => {
                eax = m.read32(esi.wrapping_add(0x40));
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xbfd09; continue; }
                pc = 0xbfc8e; continue;
            }
            0xbfc8e => {
                fa = eax;
                fb = ((((0x2ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xbfd09; continue; }
                pc = 0xbfc93; continue;
            }
            0xbfc93 => {
                ecx = m.read32(esi.wrapping_add(0x44));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xbfd09; continue; }
                pc = 0xbfc9a; continue;
            }
            0xbfc9a => {
                fa = eax;
                fb = ((((0x21i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xbfcc1; continue; }
                pc = 0xbfc9f; continue;
            }
            0xbfc9f => {
                fa = eax;
                fb = ((((0x20i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xbfcc1; continue; }
                pc = 0xbfca4; continue;
            }
            0xbfca4 => {
                eax = ((eax as i32).wrapping_mul((0xe0) as i32)) as u32;
                edx = m.read32(eax.wrapping_add(0x4ea484));
                push!(edx);
                call!(0x3d7b0);
                pc = 0xbfcb6; continue;
            }
            0xbfcb6 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x18fc);
                pc = 0xbfcdc; continue;
            }
            0xbfcc1 => {
                eax = ((eax as i32).wrapping_mul((0xe0) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(0x4ea484));
                push!(eax);
                call!(0x3d7b0);
                pc = 0xbfcd3; continue;
            }
            0xbfcd3 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x18fd);
                pc = 0xbfcdc; continue;
            }
            0xbfcdc => {
                call!(0x3d7b0);
                pc = 0xbfce1; continue;
            }
            0xbfce1 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x478904);
                call!(0x151030);
                pc = 0xbfcef; continue;
            }
            0xbfcef => {
                ecx = m.read32(edi.wrapping_add(0x150));
                edx = m.read32(ecx.wrapping_add(0x4));
                esp = esp.wrapping_add(0xc);
                push!(0);
                push!(eax);
                push!(0x2);
                push!(edx);
                call!(0x12d350);
                pc = 0xbfd06; continue;
            }
            0xbfd06 => {
                esp = esp.wrapping_add(0x10);
                pc = 0xbfd09; continue;
            }
            0xbfd09 => {
                eax = m.read32(esi.wrapping_add(0x48));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xbfd61; continue; }
                pc = 0xbfd10; continue;
            }
            0xbfd10 => {
                fa = eax;
                fb = ((((0x2ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xbfd61; continue; }
                pc = 0xbfd15; continue;
            }
            0xbfd15 => {
                ecx = m.read32(esi.wrapping_add(0x4c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xbfd61; continue; }
                pc = 0xbfd1c; continue;
            }
            0xbfd1c => {
                eax = ((eax as i32).wrapping_mul((0xe0) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(0x4ea484));
                push!(eax);
                call!(0x3d7b0);
                pc = 0xbfd2e; continue;
            }
            0xbfd2e => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x18fc);
                call!(0x3d7b0);
                pc = 0xbfd3c; continue;
            }
            0xbfd3c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x478904);
                call!(0x151030);
                pc = 0xbfd4a; continue;
            }
            0xbfd4a => {
                ecx = m.read32(edi.wrapping_add(0x150));
                edx = m.read32(ecx.wrapping_add(0x4));
                push!(0);
                push!(eax);
                push!(0x2);
                push!(edx);
                call!(0x12d350);
                pc = 0xbfd5e; continue;
            }
            0xbfd5e => {
                esp = esp.wrapping_add(0x1c);
                pc = 0xbfd61; continue;
            }
            0xbfd61 => {
                eax = m.read32(ebp.wrapping_add(0x8));
                push!(0x3f800000);
                push!(eax);
                push!(0x24);
                call!(0x238510);
                pc = 0xbfd71; continue;
            }
            0xbfd71 => {
                ecx = m.read32(esp.wrapping_add(0x38));
                esp = esp.wrapping_add(0xc);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(ecx, 0x1);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            0xbfd8b => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0xBFF70. INFERRED: 101 byte body calling 0xbf360, game_object_realign_current_weapon_slot_0x94_to_chain_head and 0xa2c00
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn pickup_realign_weapon_slot_after_take<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xbff70;
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
        { crate::pose::cover(0xbff70, previous, pc); previous = pc; }
        match pc {
            0xbff70 => {
                push!(ebx);
                push!(ebp);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x14));
                ebx = m.read32(esi.wrapping_add(0x7c));
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x14));
                ebp = m.read32(edi.wrapping_add(0x7c));
                push!(edi);
                call!(0xbf360);
                pc = 0xbff88; continue;
            }
            0xbff88 => {
                push!(ebx);
                m.write32(ebx.wrapping_add(0x80), edi);
                call!(0x1ba1b0);
                pc = 0xbff94; continue;
            }
            0xbff94 => {
                eax = m.read32(esi.wrapping_add(0x74));
                ecx = m.read32(eax.wrapping_add(0x8c));
                edx = m.read32(ecx.wrapping_add(0x2c));
                eax = m.read32(esi.wrapping_add(0x4));
                push!(0x2);
                push!(0xffffffffu32);
                push!(edx);
                push!(eax);
                push!(edi);
                call!(0xa2c00);
                pc = 0xbffaf; continue;
            }
            0xbffaf => {
                ecx = m.read32(esi.wrapping_add(0x64));
                m.write32(edi.wrapping_add(0x248), ecx);
                edx = m.read32(esi.wrapping_add(0x68));
                m.write32(edi.wrapping_add(0x24c), edx);
                eax = m.read32(esi.wrapping_add(0x6c));
                esp = esp.wrapping_add(0x1c);
                m.write32(edi.wrapping_add(0x250), eax);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(ebp.wrapping_add(0x4), esi);
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


/// 0xCEF30. INFERRED: Reads the descriptor from game_get_descriptor_for_current_state_7de470_from_tables_510788_513d04 (field +0x74 by slot), falls back to the weapon table at 0x4fa128 stride 0x3c, then calls game_ammo_add_default_amounts_words_0x28_0x2a_of_both
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_grant_default_weapon_and_ammo_from_stat<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xcef30;
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
        { crate::pose::cover(0xcef30, previous, pc); previous = pc; }
        match pc {
            0xcef30 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(ebx.wrapping_add(0x14));
                push!(ebp);
                ebp = m.read32(eax.wrapping_add(0x7c));
                eax = m.read32(0x7de458);
                push!(esi);
                esi = 0;
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0xcef4f; continue; }
                pc = 0xcef48; continue;
            }
            0xcef48 => {
                fa = eax;
                fb = ((((0x8000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0xcef67; continue; }
                pc = 0xcef4f; continue;
            }
            0xcef4f => {
                call!(0x230510);
                pc = 0xcef54; continue;
            }
            0xcef54 => {
                ecx = m.read32(ebx.wrapping_add(0x4));
                ecx = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                esi = m.read32(eax.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x74));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xcefd5; continue; }
                pc = 0xcef62; continue;
            }
            0xcef62 => {
                eax = m.read32(0x7de458);
                pc = 0xcef67; continue;
            }
            0xcef67 => {
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0xcefc9; continue; }
                pc = 0xcef6c; continue;
            }
            0xcef6c => {
                push!(edi);
                call!(0x6bd30);
                pc = 0xcef72; continue;
            }
            0xcef72 => {
                edi = eax;
                ecx = m.read32(edi.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xcefbf; continue; }
                pc = 0xcef7b; continue;
            }
            0xcef7b => {
                eax = m.read32(edi.wrapping_add(0x8));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xcef92; continue; }
                pc = 0xcef82; continue;
            }
            0xcef82 => {
                edx = eax;
                edx = ((edx as i32).wrapping_mul((0x3c) as i32)) as u32;
                fa = ((((m.read8(edx.wrapping_add(0x4fa128)))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0xcef92; continue; }
                pc = 0xcef90; continue;
            }
            0xcef90 => {
                esi = eax;
                pc = 0xcef92; continue;
            }
            0xcef92 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xcefbf; continue; }
                pc = 0xcef96; continue;
            }
            0xcef96 => {
                eax = m.read32(edi.wrapping_add(0x8));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xcefbf; continue; }
                pc = 0xcef9d; continue;
            }
            0xcef9d => {
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x3c) as i32)) as u32;
                fa = ((((m.read8(ecx.wrapping_add(0x4fa128)))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0xcefbf; continue; }
                pc = 0xcefab; continue;
            }
            0xcefab => {
                push!(eax);
                push!(ebp);
                call!(0x1c1d90);
                pc = 0xcefb2; continue;
            }
            0xcefb2 => {
                edx = m.read32(edi.wrapping_add(0x8));
                push!(edx);
                push!(ebp);
                call!(0x1bfaa0);
                pc = 0xcefbc; continue;
            }
            0xcefbc => {
                esp = esp.wrapping_add(0x10);
                pc = 0xcefbf; continue;
            }
            0xcefbf => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa != fb) { pc = 0xcefd5; continue; }
                pc = 0xcefc4; continue;
            }
            0xcefc4 => {
                eax = m.read32(0x7de458);
                pc = 0xcefc9; continue;
            }
            0xcefc9 => {
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0xcefda; continue; }
                pc = 0xcefce; continue;
            }
            0xcefce => {
                esi = 0x1;
                pc = 0xcefda; continue;
            }
            0xcefd5 => {
                eax = m.read32(0x7de458);
                pc = 0xcefda; continue;
            }
            0xcefda => {
                fa = m.read32(0x79094c);
                fb = ((((0x64i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xceff8; continue; }
                pc = 0xcefe3; continue;
            }
            0xcefe3 => {
                fa = ((((m.read8(0x7de455))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0xceff8; continue; }
                pc = 0xcefec; continue;
            }
            0xcefec => {
                fa = m.read32(ebx);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0xceff8; continue; }
                pc = 0xceff1; continue;
            }
            0xceff1 => {
                esi = 0x38;
                pc = 0xcf001; continue;
            }
            0xceff8 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0xcf05a; continue; }
                pc = 0xceffc; continue;
            }
            0xceffc => {
                fa = esi;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0xcf05a; continue; }
                pc = 0xcf001; continue;
            }
            0xcf001 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0xcf00c; continue; }
                pc = 0xcf005; continue;
            }
            0xcf005 => {
                fa = eax;
                fb = ((((0x8000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0xcf03f; continue; }
                pc = 0xcf00c; continue;
            }
            0xcf00c => {
                call!(0x230510);
                pc = 0xcf011; continue;
            }
            0xcf011 => {
                ecx = eax;
                eax = m.read32(ebx.wrapping_add(0x4));
                edx = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = m.read32(ecx.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x74));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xcf03f; continue; }
                pc = 0xcf021; continue;
            }
            0xcf021 => {
                call!(0x230510);
                pc = 0xcf026; continue;
            }
            0xcf026 => {
                ecx = m.read32(ebx.wrapping_add(0x4));
                ecx = ecx.wrapping_add(0xa);
                ecx = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                edx = eax.wrapping_add(ecx.wrapping_mul(0x4));
                push!(edx);
                push!(esi);
                push!(ebp);
                call!(0x1c1a90);
                pc = 0xcf03a; continue;
            }
            0xcf03a => {
                esp = esp.wrapping_add(0xc);
                pc = 0xcf049; continue;
            }
            0xcf03f => {
                push!(esi);
                push!(ebp);
                call!(0x1c1d90);
                pc = 0xcf046; continue;
            }
            0xcf046 => {
                esp = esp.wrapping_add(0x8);
                pc = 0xcf049; continue;
            }
            0xcf049 => {
                push!(esi);
                push!(ebp);
                call!(0x1bfaa0);
                pc = 0xcf050; continue;
            }
            0xcf050 => {
                push!(esi);
                push!(ebp);
                call!(0x1ba130);
                pc = 0xcf057; continue;
            }
            0xcf057 => {
                esp = esp.wrapping_add(0x10);
                pc = 0xcf05a; continue;
            }
            0xcf05a => {
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


/// 0x357160. INFERRED: serialises player fields from g7b0c48 players array into packet bytes
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn net_write_player_record_for_code_0x21<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x357160;
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
        { crate::pose::cover(0x357160, previous, pc); previous = pc; }
        match pc {
            0x357160 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(eax);
                push!(ebx);
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x14));
                edx = (edx & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(0x3c))) as u32)) & 0xff);
                m.write8(ecx, (((((edx) as u8)) as u32)) as u8);
                esi = m.read32(eax);
                esi = esi.wrapping_add(1);
                m.write32(eax, esi);
                ecx = m.read32(edi.wrapping_add(0x3c));
                edx = m.read32(0x7356d8);
                ecx = ((ecx as i32).wrapping_mul((0xea0) as i32)) as u32;
                ecx = ecx.wrapping_add(edx);
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x94))) as u32)) & 0xff);
                m.write8(esi, (((((edx) as u8)) as u32)) as u8);
                ebx = m.read32(eax);
                ebx = ebx.wrapping_add(1);
                edx = ebx;
                m.write32(eax, ebx);
                esi = m.read32(0x4f9bac);
                m.write32(esp.wrapping_add(0x10), edx);
                edx = m.read32(ecx.wrapping_add(0x204));
                fa = edx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3571b0; continue; }
                pc = 0x3571ac; continue;
            }
            0x3571ac => {
                ebx = 0;
                pc = 0x3571bc; continue;
            }
            0x3571b0 => {
                ebx = edx;
                ebx = ((ebx as i32).wrapping_mul((0x29c) as i32)) as u32;
                ebx = m.read32(ebx.wrapping_add(esi).wrapping_add(0x14));
                pc = 0x3571bc; continue;
            }
            0x3571bc => {
                fa = edx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x14), ebx);
                if (fa != fb) { pc = 0x3571c9; continue; }
                pc = 0x3571c5; continue;
            }
            0x3571c5 => {
                edx = 0;
                pc = 0x3571d3; continue;
            }
            0x3571c9 => {
                edx = ((edx as i32).wrapping_mul((0x29c) as i32)) as u32;
                edx = m.read32(edx.wrapping_add(esi).wrapping_add(0x1c));
                pc = 0x3571d3; continue;
            }
            0x3571d3 => {
                esi = m.read32(esp.wrapping_add(0x10));
                m.write8(esi, (((((ebx) as u8)) as u32)) as u8);
                esi = m.read32(eax);
                esi = esi.wrapping_add(1);
                m.write32(eax, esi);
                ebx = (ebx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(ebx.wrapping_mul(0x2)).wrapping_add(0x61e))) as u32)) & 0xffff);
                write16(m, esi, (((((ebx) as u16)) as u32)) as u16);
                ebx = m.read32(eax);
                ebx = ebx.wrapping_add(0x2);
                m.write32(eax, ebx);
                esi = ebx;
                ebx = m.read32(esp.wrapping_add(0x14));
                ebx = (ebx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(ebx.wrapping_mul(0x2)).wrapping_add(0x20c))) as u32)) & 0xffff);
                write16(m, esi, (((((ebx) as u16)) as u32)) as u16);
                esi = m.read32(eax);
                esi = esi.wrapping_add(0x2);
                m.write32(eax, esi);
                m.write8(esi, (((((edx) as u8)) as u32)) as u8);
                ebx = m.read32(eax);
                ebx = ebx.wrapping_add(1);
                m.write32(eax, ebx);
                esi = ebx;
                ebx = (ebx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x61e))) as u32)) & 0xffff);
                write16(m, esi, (((((ebx) as u16)) as u32)) as u16);
                esi = m.read32(eax);
                esi = esi.wrapping_add(0x2);
                m.write32(eax, esi);
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x20c))) as u32)) & 0xffff);
                write16(m, esi, (((((edx) as u16)) as u32)) as u16);
                ebx = m.read32(eax);
                ebx = ebx.wrapping_add(0x2);
                m.write32(eax, ebx);
                edx = m.read32(edi.wrapping_add(0x3c));
                edx = ((edx as i32).wrapping_mul((0x1584) as i32)) as u32;
                esi = ebx;
                ebx = m.read32(0x7b0c48);
                edx = (edx & 0xffff_ff00) | ((((m.read8(edx.wrapping_add(ebx).wrapping_add(0x594))) as u32)) & 0xff);
                m.write8(esi, (((((edx) as u8)) as u32)) as u8);
                esi = m.read32(eax);
                esi = esi.wrapping_add(1);
                edx = esi;
                m.write32(eax, esi);
                esi = m.read32(ecx.wrapping_add(0xd4));
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x10), edx);
                edx = m.read32(0x4f9bac);
                if (fa != fb) { pc = 0x35726f; continue; }
                pc = 0x35726b; continue;
            }
            0x35726b => {
                ebx = 0;
                pc = 0x35727b; continue;
            }
            0x35726f => {
                ebx = esi;
                ebx = ((ebx as i32).wrapping_mul((0x29c) as i32)) as u32;
                ebx = m.read32(ebx.wrapping_add(edx).wrapping_add(0x14));
                pc = 0x35727b; continue;
            }
            0x35727b => {
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x14), ebx);
                if (fa != fb) { pc = 0x357288; continue; }
                pc = 0x357284; continue;
            }
            0x357284 => {
                edx = 0;
                pc = 0x357292; continue;
            }
            0x357288 => {
                esi = ((esi as i32).wrapping_mul((0x29c) as i32)) as u32;
                edx = m.read32(esi.wrapping_add(edx).wrapping_add(0x1c));
                pc = 0x357292; continue;
            }
            0x357292 => {
                esi = m.read32(esp.wrapping_add(0x10));
                m.write8(esi, (((((ebx) as u8)) as u32)) as u8);
                esi = m.read32(eax);
                esi = esi.wrapping_add(1);
                m.write32(eax, esi);
                ebx = (ebx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(ebx.wrapping_mul(0x2)).wrapping_add(0x61e))) as u32)) & 0xffff);
                write16(m, esi, (((((ebx) as u16)) as u32)) as u16);
                ebx = m.read32(eax);
                ebx = ebx.wrapping_add(0x2);
                m.write32(eax, ebx);
                esi = ebx;
                ebx = m.read32(esp.wrapping_add(0x14));
                ebx = (ebx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(ebx.wrapping_mul(0x2)).wrapping_add(0xdc))) as u32)) & 0xffff);
                write16(m, esi, (((((ebx) as u16)) as u32)) as u16);
                esi = m.read32(eax);
                esi = esi.wrapping_add(0x2);
                m.write32(eax, esi);
                m.write8(esi, (((((edx) as u8)) as u32)) as u8);
                ebx = m.read32(eax);
                ebx = ebx.wrapping_add(1);
                m.write32(eax, ebx);
                esi = ebx;
                ebx = (ebx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x61e))) as u32)) & 0xffff);
                write16(m, esi, (((((ebx) as u16)) as u32)) as u16);
                esi = m.read32(eax);
                esi = esi.wrapping_add(0x2);
                m.write32(eax, esi);
                ecx = (ecx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0xdc))) as u32)) & 0xffff);
                write16(m, esi, (((((ecx) as u16)) as u32)) as u16);
                edx = m.read32(eax);
                edx = edx.wrapping_add(0x2);
                m.write32(eax, edx);
                esi = m.read32(0x7b0c48);
                ecx = edx;
                edx = m.read32(edi.wrapping_add(0x3c));
                edx = ((edx as i32).wrapping_mul((0x1584) as i32)) as u32;
                edx = (edx & 0xffff_ff00) | ((((m.read8(edx.wrapping_add(esi).wrapping_add(0x1bc))) as u32)) & 0xff);
                m.write8(ecx, (((((edx) as u8)) as u32)) as u8);
                ecx = m.read32(eax);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = ecx.wrapping_add(1);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax, ecx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x357320. INFERRED: parses packed fields into g7b0c48 player record, object_stash_slot_0x94_into_0x98, attached_node_toggle_flag
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn net_read_player_record_for_code_0x21_apply_to<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x357320;
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
        { crate::pose::cover(0x357320, previous, pc); previous = pc; }
        match pc {
            0x357320 => {
                push!(ebx);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                eax = m.read32(esi);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                ebx = ((((ecx) as u8)) as u8 as u32);
                push!(edi);
                edi = ebx;
                edi = ((edi as i32).wrapping_mul((0xea0) as i32)) as u32;
                eax = eax.wrapping_add(1);
                m.write32(esi, eax);
                edx = m.read32(0x7356d8);
                ecx = m.read32(edi.wrapping_add(edx));
                edi = edi.wrapping_add(edx);
                edx = m.read32(ecx.wrapping_add(0x150));
                ecx = m.read32(edx.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x35735a; continue; }
                pc = 0x357351; continue;
            }
            0x357351 => {
                eax = eax.wrapping_add(0x17);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esi, eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x35735a => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                push!(ebp);
                ebp = ((((ecx) as u8)) as u8 as u32);
                eax = eax.wrapping_add(1);
                m.write32(esi, eax);
                eax = m.read32(edi.wrapping_add(0x94));
                push!(ebp);
                push!(edi);
                m.write8(eax.wrapping_add(edi).wrapping_add(0x54c), (0x1) as u8);
                call!(0x1bf9e0);
                pc = 0x357378; continue;
            }
            0x357378 => {
                eax = m.read32(esi);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                ecx = ((((ecx) as u8)) as u8 as u32);
                esp = esp.wrapping_add(0x8);
                eax = eax.wrapping_add(1);
                m.write32(esi, eax);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax)) as u32)) & 0xffff);
                eax = eax.wrapping_add(0x2);
                m.write32(esi, eax);
                write16(m, edi.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x61e), (((((edx) as u16)) as u32)) as u16);
                eax = m.read32(esi);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax)) as u32)) & 0xffff);
                eax = eax.wrapping_add(0x2);
                m.write32(esi, eax);
                write16(m, edi.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x20c), (((((edx) as u16)) as u32)) as u16);
                eax = m.read32(esi);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                ecx = ((((ecx) as u8)) as u8 as u32);
                eax = eax.wrapping_add(1);
                m.write32(esi, eax);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax)) as u32)) & 0xffff);
                eax = eax.wrapping_add(0x2);
                m.write32(esi, eax);
                write16(m, edi.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x61e), (((((edx) as u16)) as u32)) as u16);
                eax = m.read32(esi);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax)) as u32)) & 0xffff);
                eax = eax.wrapping_add(0x2);
                m.write32(esi, eax);
                write16(m, edi.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x20c), (((((edx) as u16)) as u32)) as u16);
                eax = m.read32(esi);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                eax = eax.wrapping_add(1);
                m.write32(esi, eax);
                edx = m.read32(0x7b0c48);
                eax = ebx;
                ecx = ((((ecx) as u8)) as u8 as u32);
                eax = ((eax as i32).wrapping_mul((0x1584) as i32)) as u32;
                m.write32(eax.wrapping_add(edx).wrapping_add(0x594), ecx);
                ecx = m.read32(0x7b0c48);
                edx = m.read32(eax.wrapping_add(ecx).wrapping_add(0x594));
                m.write32(edi.wrapping_add(0x208), edx);
                ecx = m.read32(esi);
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx)) as u32)) & 0xff);
                ecx = ecx.wrapping_add(1);
                m.write32(esi, ecx);
                ebx = (ebx & 0xffff_0000) | ((((m.read16(ecx)) as u32)) & 0xffff);
                ecx = ecx.wrapping_add(0x2);
                m.write32(esi, ecx);
                edx = ((((edx) as u8)) as u8 as u32);
                write16(m, edi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x61e), (((((ebx) as u16)) as u32)) as u16);
                ecx = m.read32(esi);
                ebx = (ebx & 0xffff_0000) | ((((m.read16(ecx)) as u32)) & 0xffff);
                ecx = ecx.wrapping_add(0x2);
                m.write32(esi, ecx);
                write16(m, edi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0xdc), (((((ebx) as u16)) as u32)) as u16);
                ecx = m.read32(esi);
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx)) as u32)) & 0xff);
                ecx = ecx.wrapping_add(1);
                m.write32(esi, ecx);
                ebx = (ebx & 0xffff_0000) | ((((m.read16(ecx)) as u32)) & 0xffff);
                edx = ((((edx) as u8)) as u8 as u32);
                ecx = ecx.wrapping_add(0x2);
                m.write32(esi, ecx);
                write16(m, edi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x61e), (((((ebx) as u16)) as u32)) as u16);
                ecx = m.read32(esi);
                ebx = (ebx & 0xffff_0000) | ((((m.read16(ecx)) as u32)) & 0xffff);
                ecx = ecx.wrapping_add(0x2);
                m.write32(esi, ecx);
                write16(m, edi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0xdc), (((((ebx) as u16)) as u32)) as u16);
                ecx = m.read32(esi);
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx)) as u32)) & 0xff);
                ecx = ecx.wrapping_add(1);
                fa = ebp;
                fb = ((((0x20i32)) as u32) & 0xffffffffu32);
                m.write32(esi, ecx);
                ecx = ((((edx) as u8)) as u8 as u32);
                edx = m.read32(0x7b0c48);
                m.write32(eax.wrapping_add(edx).wrapping_add(0x1bc), ecx);
                ecx = m.read32(0x7b0c48);
                edx = m.read32(eax.wrapping_add(ecx).wrapping_add(0x1bc));
                m.write32(edi.wrapping_add(0xd8), edx);
                if (fa != fb) { pc = 0x357490; continue; }
                pc = 0x35748c; continue;
            }
            0x35748c => {
                push!(0);
                pc = 0x357497; continue;
            }
            0x357490 => {
                fa = ebp;
                fb = ((((0x21i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3574b4; continue; }
                pc = 0x357495; continue;
            }
            0x357495 => {
                push!(0x1);
                pc = 0x357497; continue;
            }
            0x357497 => {
                ecx = m.read32(0x7b0c48);
                edi = edi.wrapping_add(0x1d4);
                edx = eax.wrapping_add(ecx).wrapping_add(0x55c);
                push!(edi);
                push!(edx);
                call!(0x1b57c0);
                pc = 0x3574b1; continue;
            }
            0x3574b1 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x3574b4; continue;
            }
            0x3574b4 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
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


/// 0x25EAD0. INFERRED: text_string_get_by_id then hud_player_message_post_by_kind, tests flags 0x100 and 0x200 of mode 7de458
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn hud_message_post_by_mode_flags_with_text_id<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x25ead0;
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
        { crate::pose::cover(0x25ead0, previous, pc); previous = pc; }
        match pc {
            0x25ead0 => {
                push!(ebp);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                eax = m.read32(esi.wrapping_add(0x14));
                ebp = m.read32(eax.wrapping_add(0x7c));
                m.write32(ebx, ecx);
                ecx = m.read32(ebx.wrapping_add(0x4));
                push!(edi);
                ecx = (ecx | edx);
                push!(0);
                push!(0x18f9);
                m.write32(ebx.wrapping_add(0x4), ecx);
                call!(0x3d7b0);
                pc = 0x25eaf3; continue;
            }
            0x25eaf3 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x2);
                push!(0);
                call!(0x12d350);
                pc = 0x25eb00; continue;
            }
            0x25eb00 => {
                eax = m.read32(ebp.wrapping_add(0x78));
                m.write32(ebx.wrapping_add(0x8), eax);
                ecx = m.read32(ebp.wrapping_add(0x7c));
                m.write32(ebx.wrapping_add(0xc), ecx);
                edx = m.read32(ebp.wrapping_add(0x94));
                m.write32(ebx.wrapping_add(0x10), edx);
                eax = m.read32(ebp.wrapping_add(0xa0));
                m.write32(ebx.wrapping_add(0x1c), eax);
                eax = m.read32(esi.wrapping_add(0x114));
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0x100i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x25eb55; continue; }
                pc = 0x25eb2e; continue;
            }
            0x25eb2e => {
                fa = eax;
                fb = ((((0x200i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x25eb55; continue; }
                pc = 0x25eb35; continue;
            }
            0x25eb35 => {
                eax = m.read32(0x7de458);
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x25eb55; continue; }
                pc = 0x25eb3f; continue;
            }
            0x25eb3f => {
                eax = m.read32(ebp.wrapping_add(0x204));
                ecx = 0;
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                m.write32(ebx.wrapping_add(0x230), ecx);
                pc = 0x25eb68; continue;
            }
            0x25eb55 => {
                edx = m.read32(ebp.wrapping_add(0x9c));
                m.write32(ebx.wrapping_add(0x10), edx);
                m.write32(ebx.wrapping_add(0x230), 0);
                pc = 0x25eb68; continue;
            }
            0x25eb68 => {
                eax = m.read32(ebp.wrapping_add(0x98));
                m.write32(ebx.wrapping_add(0x18), eax);
                ecx = m.read32(ebp.wrapping_add(0x9c));
                m.write32(ebx.wrapping_add(0x14), ecx);
                edx = m.read32(esi.wrapping_add(0x14));
                eax = m.read32(edx.wrapping_add(0x1cc));
                m.write32(ebx.wrapping_add(0xc0), eax);
                ecx = m.read32(esi.wrapping_add(0x14));
                edx = m.read32(ecx.wrapping_add(0x1d8));
                m.write32(ebx.wrapping_add(0xc4), edx);
                eax = m.read32(esi.wrapping_add(0x14));
                ecx = m.read32(eax.wrapping_add(0x1dc));
                m.write32(ebx.wrapping_add(0xc8), ecx);
                edx = m.read32(ebp.wrapping_add(0x544));
                m.write32(ebx.wrapping_add(0x234), edx);
                esi = ebp.wrapping_add(0x20c);
                edi = ebx.wrapping_add(0x124);
                ecx = 0x16;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                esi = ebp.wrapping_add(0xdc);
                edi = ebx.wrapping_add(0xcc);
                ecx = 0x16;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                esi = ebp.wrapping_add(0x61e);
                edi = ebx.wrapping_add(0x20);
                ecx = 0x16;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                esi = ebp.wrapping_add(0x264);
                edi = ebx.wrapping_add(0x1d4);
                ecx = 0x16;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                esi = ebp.wrapping_add(0x134);
                edi = ebx.wrapping_add(0x17c);
                ecx = 0x16;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                esi = ebp.wrapping_add(0x54c);
                edi = ebx.wrapping_add(0x78);
                ecx = 0x11;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                write16(m, edi, (((m.read16(esi)) as u32)) as u16);
                esi = esi.wrapping_add(0x2);
                edi = edi.wrapping_add(0x2);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x25EC70. INFERRED: T1628 read to 0x25ee44: record 238 (ebx) into the player ext block of record eax: +0x8/+0xc to ext +0x78/+0x7c, +0x10 current entry via 1ba130, +0x14/+0x18/+0x1c to ext +0x9c/+0x98/+0xa0, record +0x20 (0x16 dwords) to ammo words +0x61e, +0x
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_restore_ext_inventory_ammo_health_and_c<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x25ec70;
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
        { crate::pose::cover(0x25ec70, previous, pc); previous = pc; }
        match pc {
            0x25ec70 => {
                ecx = m.read32(ebx.wrapping_add(0x8));
                esp = esp.wrapping_sub(0x8);
                push!(ebp);
                push!(esi);
                esi = eax;
                eax = m.read32(esi.wrapping_add(0x14));
                ebp = m.read32(eax.wrapping_add(0x7c));
                m.write32(ebp.wrapping_add(0x78), ecx);
                edx = m.read32(ebx.wrapping_add(0xc));
                m.write32(ebp.wrapping_add(0x7c), edx);
                eax = m.read32(ebx.wrapping_add(0x18));
                m.write32(ebp.wrapping_add(0x98), eax);
                ecx = m.read32(ebx.wrapping_add(0x14));
                m.write32(ebp.wrapping_add(0x9c), ecx);
                edx = m.read32(ebx.wrapping_add(0x1c));
                m.write32(ebp.wrapping_add(0xa0), edx);
                eax = m.read32(ebx.wrapping_add(0x10));
                push!(eax);
                push!(ebp);
                call!(0x1ba130);
                pc = 0x25ecae; continue;
            }
            0x25ecae => {
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0xc4)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cdc))).to_bits();
                esp = esp.wrapping_add(0x8);
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(ebx.wrapping_add(0xc0))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x25ecd7; continue; }
                pc = 0x25ecca; continue;
            }
            0x25ecca => {
                ecx = m.read32(esi.wrapping_add(0x14));
                m.write32(ecx.wrapping_add(0x1cc), xmm0.lanes[0]);
                pc = 0x25ece6; continue;
            }
            0x25ecd7 => {
                edx = m.read32(esi.wrapping_add(0x14));
                eax = m.read32(ebx.wrapping_add(0xc0));
                m.write32(edx.wrapping_add(0x1cc), eax);
                pc = 0x25ece6; continue;
            }
            0x25ece6 => {
                ecx = m.read32(esi.wrapping_add(0x14));
                edx = m.read32(ebx.wrapping_add(0xc4));
                m.write32(ecx.wrapping_add(0x1d8), edx);
                eax = m.read32(esi.wrapping_add(0x14));
                ecx = m.read32(ebx.wrapping_add(0xc8));
                m.write32(eax.wrapping_add(0x1dc), ecx);
                edx = m.read32(ebx.wrapping_add(0x234));
                push!(edi);
                m.write32(ebp.wrapping_add(0x544), edx);
                edi = ebp.wrapping_add(0x61e);
                esi = ebx.wrapping_add(0x20);
                ecx = 0x16;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                esi = ebx.wrapping_add(0x1d4);
                edi = ebp.wrapping_add(0x264);
                ecx = 0x16;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                esi = ebx.wrapping_add(0x17c);
                edi = ebp.wrapping_add(0x134);
                ecx = 0x16;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                esi = ebx.wrapping_add(0x78);
                edi = ebp.wrapping_add(0x54c);
                ecx = 0x11;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                write16(m, edi, (((m.read16(esi)) as u32)) as u16);
                esi = esi.wrapping_add(0x2);
                edi = edi.wrapping_add(0x2);
                esi = ebx.wrapping_add(0x124);
                m.write32(esp.wrapping_add(0xc), esi);
                edi = ebp.wrapping_add(0x20c);
                ecx = 0x16;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                esi = ebx.wrapping_add(0xcc);
                edi = ebp.wrapping_add(0xdc);
                ecx = 0x16;
                m.write32(esp.wrapping_add(0x10), esi);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                fa = m.read32(ebx.wrapping_add(0x230));
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x25ee1e; continue; }
                pc = 0x25ed94; continue;
            }
            0x25ed94 => {
                call!(0x1c4400);
                pc = 0x25ed99; continue;
            }
            0x25ed99 => {
                esi = m.read32(esp.wrapping_add(0xc));
                ecx = 0x16;
                edi = ebp.wrapping_add(0x20c);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                esi = m.read32(esp.wrapping_add(0x10));
                ecx = 0x16;
                edi = ebp.wrapping_add(0xdc);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), m.read32(esi.wrapping_add(i.wrapping_mul(step)))); } esi = esi.wrapping_add(ecx.wrapping_mul(step)); edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep movsd, DF from entry.eflags
                ecx = 0;
                eax = ebp.wrapping_add(0x61e);
                ecx = 0xb;
                pc = 0x25edd0; continue;
            }
            0x25edd0 => {
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(((0x412i32.wrapping_neg()) as u32)))) as u32)) & 0xffff);
                edx = (edx & 0xffff_0000) | ((((((((edx) as u16)) as i32).wrapping_add(((m.read16(eax.wrapping_add(((0x542i32.wrapping_neg()) as u32)))) as i32))) as u32)) & 0xffff);
                eax = eax.wrapping_add(0x8);
                write16(m, eax.wrapping_add(((0x8i32.wrapping_neg()) as u32)), (((((m.read16(eax.wrapping_add(((0x8i32.wrapping_neg()) as u32)))) as i32).wrapping_add(((((edx) as u16)) as i32))) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(((0x548i32.wrapping_neg()) as u32)))) as u32)) & 0xffff);
                edx = (edx & 0xffff_0000) | ((((((((edx) as u16)) as i32).wrapping_add(((m.read16(eax.wrapping_add(((0x418i32.wrapping_neg()) as u32)))) as i32))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(((0x6i32.wrapping_neg()) as u32)), (((((m.read16(eax.wrapping_add(((0x6i32.wrapping_neg()) as u32)))) as i32).wrapping_add(((((edx) as u16)) as i32))) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(((0x546i32.wrapping_neg()) as u32)))) as u32)) & 0xffff);
                edx = (edx & 0xffff_0000) | ((((((((edx) as u16)) as i32).wrapping_add(((m.read16(eax.wrapping_add(((0x416i32.wrapping_neg()) as u32)))) as i32))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(((0x4i32.wrapping_neg()) as u32)), (((((m.read16(eax.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) as i32).wrapping_add(((((edx) as u16)) as i32))) as u32)) as u16);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(((0x544i32.wrapping_neg()) as u32)))) as u32)) & 0xffff);
                edx = (edx & 0xffff_0000) | ((((((((edx) as u16)) as i32).wrapping_add(((m.read16(eax.wrapping_add(((0x414i32.wrapping_neg()) as u32)))) as i32))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(((0x2i32.wrapping_neg()) as u32)), (((((m.read16(eax.wrapping_add(((0x2i32.wrapping_neg()) as u32)))) as i32).wrapping_add(((((edx) as u16)) as i32))) as u32)) as u16);
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x25edd0; continue; }
                pc = 0x25ee1e; continue;
            }
            0x25ee1e => {
                eax = m.read32(ebp.wrapping_add(0x94));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa != fb) { pc = 0x25ee32; continue; }
                pc = 0x25ee29; continue;
            }
            0x25ee29 => {
                push!(ebp);
                call!(0x1b6650);
                pc = 0x25ee2f; continue;
            }
            0x25ee2f => {
                esp = esp.wrapping_add(0x4);
                pc = 0x25ee32; continue;
            }
            0x25ee32 => {
                eax = m.read32(ebp.wrapping_add(0x94));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x25ee56; continue; }
                pc = 0x25ee3c; continue;
            }
            0x25ee3c => {
                eax = m.read32(ebp.wrapping_add(0x9c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(ebp.wrapping_add(0x94), eax);
                if (fa != fb) { pc = 0x25ee56; continue; }
                pc = 0x25ee4c; continue;
            }
            0x25ee4c => {
                m.write32(ebp.wrapping_add(0x94), 0x1);
                pc = 0x25ee56; continue;
            }
            0x25ee56 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}


// ---- translated bodies end ----

// ---- dispatch begin ----
/// Roots ported in this module (differential-only, unadmitted).
pub const ROOTS: [u32; 8] = [0xbe090, 0xbfb40, 0xbff70, 0xcef30, 0x357160, 0x357320, 0x25ead0, 0x25ec70];

/// Differential entry (not selector-registered). True when the Rust body ran.
#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0xbe090 => pickup_weapon_ammo_collect_if_not_full(m, guest, regs),
        0xbfb40 => pickup_collect_weapon_ammo_add_with_message_an(m, guest, regs),
        0xbff70 => pickup_realign_weapon_slot_after_take(m, guest, regs),
        0xcef30 => player_grant_default_weapon_and_ammo_from_stat(m, guest, regs),
        0x357160 => net_write_player_record_for_code_0x21(m, guest, regs),
        0x357320 => net_read_player_record_for_code_0x21_apply_to(m, guest, regs),
        0x25ead0 => hud_message_post_by_mode_flags_with_text_id(m, guest, regs),
        0x25ec70 => player_restore_ext_inventory_ammo_health_and_c(m, guest, regs),
        _ => return false,
    }
    true
}
// ---- dispatch end ----
