#![cfg_attr(rustfmt, rustfmt::skip)]
// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-gameplay/synth_leaves
//! T1850 (T1815 slice 3): player inventory (weapon, ammo and item slots).
//! See docs/t-inventory-pose-port.md for the evidence, the labels and the limits.
//!
//! Provenance: every function was transcribed from its lifted original (private, recomp_0011,
//! 0012, 0014, 0024..) by tools/t1850_translate.py and hand cleaned. Behaviour is MEASURED equal to
//! the lifted original on FABRICATED structured memory with stub callees
//! (tests/c/test_rust_invpose.c). Field names are INFERRED (docs/t-player-inventory.md). Not
//! admitted: absent from every selector. Shape and conventions: see pose.rs.
#![allow(clippy::all, unused_parens, unused_assignments, unused_imports, unused_variables)]
use crate::{
    abi::{ExtRegisters, GuestCall, Xmm},
    memory::GuestMemory,
    pose::write16,
};
pub(crate) mod sse {
    pub use tsfp_port_native_candidate_support::inventory::sse::*;
}
// ---- translated bodies begin ----
/// 0x1B6B40. INFERRED: no arguments. Walks the entry table 0x4FA128 (stride 0x3C, to 0x4FB190), writes a running group number into +0x38 (incremented after entries whose flags & 0xC is clear) and the final count to 0x75A024.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn renumber_entry_groups<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b6b40;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1b6b40, previous, pc); previous = pc; }
        match pc {
            0x1b6b40 => {
                ecx = (ecx | 0xffffffffu32);
                m.write32(0x75a024, ecx);
                eax = 0x4fa128;
                edi = edi;
                pc = 0x1b6b50; continue;
            }
            0x1b6b50 => {
                fa = ((((m.read8(eax))) as u32) & 0xffu32);
                fb = ((((0xci32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x1b6b56; continue; }
                pc = 0x1b6b55; continue;
            }
            0x1b6b55 => {
                ecx = ecx.wrapping_add(1);
                pc = 0x1b6b56; continue;
            }
            0x1b6b56 => {
                fa = ((((m.read8(eax.wrapping_add(0x3c)))) as u32) & 0xffu32);
                fb = ((((0xci32)) as u32) & 0xffu32);
                m.write32(eax.wrapping_add(0x38), ecx);
                if ((fa & fb) != 0) { pc = 0x1b6b60; continue; }
                pc = 0x1b6b5f; continue;
            }
            0x1b6b5f => {
                ecx = ecx.wrapping_add(1);
                pc = 0x1b6b60; continue;
            }
            0x1b6b60 => {
                fa = ((((m.read8(eax.wrapping_add(0x78)))) as u32) & 0xffu32);
                fb = ((((0xci32)) as u32) & 0xffu32);
                m.write32(eax.wrapping_add(0x74), ecx);
                if ((fa & fb) != 0) { pc = 0x1b6b6a; continue; }
                pc = 0x1b6b69; continue;
            }
            0x1b6b69 => {
                ecx = ecx.wrapping_add(1);
                pc = 0x1b6b6a; continue;
            }
            0x1b6b6a => {
                fa = ((((m.read8(eax.wrapping_add(0xb4)))) as u32) & 0xffu32);
                fb = ((((0xci32)) as u32) & 0xffu32);
                m.write32(eax.wrapping_add(0xb0), ecx);
                if ((fa & fb) != 0) { pc = 0x1b6b7a; continue; }
                pc = 0x1b6b79; continue;
            }
            0x1b6b79 => {
                ecx = ecx.wrapping_add(1);
                pc = 0x1b6b7a; continue;
            }
            0x1b6b7a => {
                fa = ((((m.read8(eax.wrapping_add(0xf0)))) as u32) & 0xffu32);
                fb = ((((0xci32)) as u32) & 0xffu32);
                m.write32(eax.wrapping_add(0xec), ecx);
                if ((fa & fb) != 0) { pc = 0x1b6b8a; continue; }
                pc = 0x1b6b89; continue;
            }
            0x1b6b89 => {
                ecx = ecx.wrapping_add(1);
                pc = 0x1b6b8a; continue;
            }
            0x1b6b8a => {
                m.write32(eax.wrapping_add(0x128), ecx);
                eax = eax.wrapping_add(0x12c);
                fa = eax;
                fb = ((((0x4fb190i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1b6b50; continue; }
                pc = 0x1b6b9c; continue;
            }
            0x1b6b9c => {
                m.write32(0x75a024, ecx);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}

/// 0x1B68F0. INFERRED: register convention, EAX = entry index, EDX = ext block. Non-zero when an owned entry (byte ext+0x54C+i) of the same group (+0x38) has flag bit 4 (x2), else 0.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn group_has_owned_x2_entry<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b68f0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1b68f0, previous, pc); previous = pc; }
        match pc {
            0x1b68f0 => {
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                push!(ebx);
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x4fa160));
                pc = 0x1b6900; continue;
            }
            0x1b6900 => {
                ecx = 0;
                eax = 0x4fa128;
                pc = 0x1b6910; continue;
            }
            0x1b6910 => {
                ebx = (ebx & 0xffff_ff00) | ((((m.read8(edx.wrapping_add(ecx).wrapping_add(0x54c))) as u32)) & 0xff);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x1b6925; continue; }
                pc = 0x1b691b; continue;
            }
            0x1b691b => {
                fa = m.read32(eax.wrapping_add(0x38));
                fb = esi;
                if (fa != fb) { pc = 0x1b6925; continue; }
                pc = 0x1b6920; continue;
            }
            0x1b6920 => {
                fa = ((((m.read8(eax))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x1b693b; continue; }
                pc = 0x1b6925; continue;
            }
            0x1b6925 => {
                eax = eax.wrapping_add(0x3c);
                ecx = ecx.wrapping_add(1);
                fa = eax;
                fb = ((((0x4fb190i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1b6910; continue; }
                pc = 0x1b6930; continue;
            }
            0x1b6930 => {
                ecx = ecx.wrapping_add(1);
                fa = ecx;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1b6900; continue; }
                pc = 0x1b6936; continue;
            }
            0x1b6936 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1b693b => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}

/// 0x1B66E0. INFERRED: cdecl `(_, obj, type, amount, kind)`. Counts a shot (+0x10C), subtracts a word from the loaded count (+0x38+2*type, kind 0 or 1 uses the reserve +0x61E instead) and clamps negative to 0.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ammo_take_counter<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b66e0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1b66e0, previous, pc); previous = pc; }
        match pc {
            0x1b66e0 => {
                eax = m.read32(esp.wrapping_add(0x8));
                m.write32(eax.wrapping_add(0x10c), m.read32(eax.wrapping_add(0x10c)).wrapping_add(0x1));
                ecx = m.read32(esp.wrapping_add(0x14));
                edx = 0;
                fa = ecx;
                fb = edx;
                push!(esi);
                if (fa == fb) { pc = 0x1b6716; continue; }
                pc = 0x1b66f5; continue;
            }
            0x1b66f5 => {
                fa = ecx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6716; continue; }
                pc = 0x1b66fa; continue;
            }
            0x1b66fa => {
                ecx = m.read32(esp.wrapping_add(0x10));
                esi = (esi & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x14))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x38), (((((m.read16(eax.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x38))) as i32).wrapping_sub(((((esi) as u16)) as i32))) as u32)) as u16);
                fa = ((((m.read16(eax.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x38)))) as u32) & 0xffffu32);
                fb = ((((((edx) as u16))) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas >= fbs) { pc = 0x1b673d; continue; }
                pc = 0x1b670f; continue;
            }
            0x1b670f => {
                write16(m, eax.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x38), (((((edx) as u16)) as u32)) as u16);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1b6716 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(esp.wrapping_add(0x10));
                esi = (esi & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x14))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x61e), (((((m.read16(eax.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x61e))) as i32).wrapping_sub(((((esi) as u16)) as i32))) as u32)) as u16);
                fa = ((((m.read16(eax.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x61e)))) as u32) & 0xffffu32);
                fb = ((((((edx) as u16))) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas >= fbs) { pc = 0x1b673d; continue; }
                pc = 0x1b6735; continue;
            }
            0x1b6735 => {
                write16(m, eax.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x61e), (((((edx) as u16)) as u32)) as u16);
                pc = 0x1b673d; continue;
            }
            0x1b673d => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}

/// 0x1B6C90. INFERRED: cdecl `(owner, state, mode, flag)`. True when reserve (+0x61E) plus loaded (+0x38) of the ammo type of the record entry covers its cost.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ammo_total_covers_cost<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b6c90;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1b6c90, previous, pc); previous = pc; }
        match pc {
            0x1b6c90 => {
                push!(edi);
                edi = m.read32(esp.wrapping_add(0xc));
                eax = m.read32(edi.wrapping_add(0x30));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1b6d03; continue; }
                pc = 0x1b6c9c; continue;
            }
            0x1b6c9c => {
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                push!(esi);
                eax = eax.wrapping_add(m.read32(0x4f9bac));
                esi = m.read32(esp.wrapping_add(0x14));
                ecx = esi;
                ecx = ((ecx as i32).wrapping_mul((0x7c) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(eax).wrapping_add(0x68));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = ecx.wrapping_add(eax).wrapping_add(0x68);
                if (((((fa).wrapping_sub((fb))).wrapping_shr((0x1f & 31))) == 0)) { pc = 0x1b6cc1; continue; }
                pc = 0x1b6cbe; continue;
            }
            0x1b6cbe => {
                ecx = eax.wrapping_add(0x68);
                pc = 0x1b6cc1; continue;
            }
            0x1b6cc1 => {
                edx = m.read32(eax.wrapping_add(0x14));
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x1c));
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa != fb) { pc = 0x1b6cd3; continue; }
                pc = 0x1b6cce; continue;
            }
            0x1b6cce => {
                ecx = m.read32(ecx.wrapping_add(0x24));
                pc = 0x1b6cd6; continue;
            }
            0x1b6cd3 => {
                ecx = m.read32(ecx.wrapping_add(0x28));
                pc = 0x1b6cd6; continue;
            }
            0x1b6cd6 => {
                fa = esi;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa != fb) { pc = 0x1b6ce5; continue; }
                pc = 0x1b6cdc; continue;
            }
            0x1b6cdc => {
                eax = m.read32(eax.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6ce5; continue; }
                pc = 0x1b6ce3; continue;
            }
            0x1b6ce3 => {
                edx = eax;
                pc = 0x1b6ce5; continue;
            }
            0x1b6ce5 => {
                eax = m.read32(esp.wrapping_add(0x8));
                eax = (((((m.read16(eax.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                edx = (((((m.read16(edi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                eax = eax.wrapping_add(edx);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1b6d03; continue; }
                pc = 0x1b6cfc; continue;
            }
            0x1b6cfc => {
                eax = 0x1;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1b6d03 => {
                eax = 0;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}

/// 0x1B9D30. INFERRED: cdecl `(record)`. For each group i below [0x75A024] stores into record+0xE3C+i the first entry (table 0x4FC888) whose group is i, or 0xFF.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn fill_slot_order_bytes<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b9d30;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1b9d30, previous, pc); previous = pc; }
        match pc {
            0x1b9d30 => {
                edx = m.read32(0x75a024);
                ecx = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x1b9d79; continue; }
                pc = 0x1b9d3c; continue;
            }
            0x1b9d3c => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                push!(edi);
                pc = 0x1b9d42; continue;
            }
            0x1b9d42 => {
                eax = 0;
                pc = 0x1b9d50; continue;
            }
            0x1b9d50 => {
                edi = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x4fc888));
                edi = ((edi as i32).wrapping_mul((0x3c) as i32)) as u32;
                fa = m.read32(edi.wrapping_add(0x4fa160));
                fb = ecx;
                if (fa == fb) { pc = 0x1b9d7a; continue; }
                pc = 0x1b9d62; continue;
            }
            0x1b9d62 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1b9d50; continue; }
                pc = 0x1b9d68; continue;
            }
            0x1b9d68 => {
                eax = (eax | 0xffffffffu32);
                pc = 0x1b9d6b; continue;
            }
            0x1b9d6b => {
                m.write8(esi.wrapping_add(ecx).wrapping_add(0xe3c), (((((eax) as u8)) as u32)) as u8);
                ecx = ecx.wrapping_add(1);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1b9d42; continue; }
                pc = 0x1b9d77; continue;
            }
            0x1b9d77 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x1b9d79; continue;
            }
            0x1b9d79 => {
                break 'run;
            }
            0x1b9d7a => {
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x4fc888));
                pc = 0x1b9d6b; continue;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}

/// 0x1BA130. INFERRED: cdecl `(ext, entry)`. Sets ext+0x94 = entry (an x2 entry maps back to its chain head when ext+0x80 is non-zero) and the order byte record+0xE3C+group.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn set_current_weapon_slot<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1ba130;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x1ba130, previous, pc); previous = pc; }
        match pc {
            0x1ba130 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x3c) as i32)) as u32;
                fa = ((((m.read8(ecx.wrapping_add(0x4fa128)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                edx = m.read32(esp.wrapping_add(0x4));
                push!(ebx);
                if ((fa & fb) == 0) { pc = 0x1ba17d; continue; }
                pc = 0x1ba147; continue;
            }
            0x1ba147 => {
                ecx = m.read32(edx.wrapping_add(0x80));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba17d; continue; }
                pc = 0x1ba151; continue;
            }
            0x1ba151 => {
                ecx = m.read32(0x7de458);
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x1ba17d; continue; }
                pc = 0x1ba15c; continue;
            }
            0x1ba15c => {
                eax = eax.wrapping_sub(1);
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x3c) as i32)) as u32;
                ebx = (ebx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x4fa128))) as u32)) & 0xff);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                fas = (((((fa)) as i8)) as i32);
                fbs = (((((fb)) as i8)) as i32);
                ecx = ecx.wrapping_add(0x4fa128);
                if (((((((((((((fas)) as u8)) as i32).wrapping_sub((((((fbs)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) != 0i32)) { pc = 0x1ba17d; continue; }
                pc = 0x1ba172; continue;
            }
            0x1ba172 => {
                ebx = (ebx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(((0x3ci32.wrapping_neg()) as u32)))) as u32)) & 0xff);
                ecx = ecx.wrapping_sub(0x3c);
                eax = eax.wrapping_sub(1);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (((((((((((((fa)) as u8)) as i32).wrapping_sub((((((fb)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) == 0i32)) { pc = 0x1ba172; continue; }
                pc = 0x1ba17d; continue;
            }
            0x1ba17d => {
                ecx = m.read32(edx.wrapping_add(0x8));
                m.write32(edx.wrapping_add(0x94), eax);
                fa = ecx;
                fb = m.read32(0x790950);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1ba1ae; continue; }
                pc = 0x1ba18e; continue;
            }
            0x1ba18e => {
                edx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x1584) as i32)) as u32;
                edx = ((edx as i32).wrapping_mul((0x3c) as i32)) as u32;
                ebx = m.read32(edx.wrapping_add(0x4fa160));
                edx = m.read32(0x7b0c48);
                ecx = ecx.wrapping_add(ebx);
                m.write8(ecx.wrapping_add(edx).wrapping_add(0xe3c), (((((eax) as u8)) as u32)) as u8);
                pc = 0x1ba1ae; continue;
            }
            0x1ba1ae => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}

/// 0x1BA220. INFERRED: cdecl `(ext, type, amount)`. Adds to the reserve word ext+0x61E+2*type, clamped to the type maximum (table 0x4EA3F8 stride 0xE0, scaled by [0x4E8CEC] and [0x478394] for flag bit 0) minus the loaded rounds. Returns the amount added. Calls 0x1B67E0.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ammo_add_reserve_clamped<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1ba220;
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
        { crate::pose::cover(0x1ba220, previous, pc); previous = pc; }
        match pc {
            0x1ba220 => {
                push!(ebx);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x10));
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x10));
                push!(esi);
                push!(edi);
                call!(0x1b67e0);
                pc = 0x1ba232; continue;
            }
            0x1ba232 => {
                edx = (edx & 0xffff_0000) | ((((m.read16(edi.wrapping_add(esi.wrapping_mul(0x2)).wrapping_add(0x61e))) as u32)) & 0xffff);
                ecx = esi;
                ecx = ((ecx as i32).wrapping_mul((0xe0) as i32)) as u32;
                esp = esp.wrapping_add(0x8);
                fa = ((((m.read8(ecx.wrapping_add(0x4ea400)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                ebx = ((((edx) as u16)) as u16 as i16 as i32 as u32);
                if ((fa & fb) == 0) { pc = 0x1ba273; continue; }
                pc = 0x1ba251; continue;
            }
            0x1ba251 => {
                xmm0.lanes[0] = (((m.read32(ecx.wrapping_add(0x4ea3f8))) as i32) as f32).to_bits();
                xmm1.lanes[0] = (((m.read32(0x4e8cec)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x478394))).to_bits();
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                pc = 0x1ba279; continue;
            }
            0x1ba273 => {
                ecx = m.read32(ecx.wrapping_add(0x4ea3f8));
                pc = 0x1ba279; continue;
            }
            0x1ba279 => {
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x1c));
                edx = edx.wrapping_add(ebp);
                write16(m, edi.wrapping_add(esi.wrapping_mul(0x2)).wrapping_add(0x61e), (((((edx) as u16)) as u32)) as u16);
                edx = ((((edx) as u16)) as u16 as i16 as i32 as u32);
                edx = edx.wrapping_add(eax);
                fa = edx;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                if (fas <= fbs) { pc = 0x1ba29c; continue; }
                pc = 0x1ba292; continue;
            }
            0x1ba292 => {
                ecx = ecx.wrapping_sub(eax);
                write16(m, edi.wrapping_add(esi.wrapping_mul(0x2)).wrapping_add(0x61e), (((((ecx) as u16)) as u32)) as u16);
                pc = 0x1ba29c; continue;
            }
            0x1ba29c => {
                eax = (((((m.read16(edi.wrapping_add(esi.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_sub(ebx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x1C1D90. INFERRED: cdecl `(ext, entry)`. Adds the default ammo words (+0x28/+0x2A) of the entry's two records through 0x1BA220.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ammo_add_default_amounts<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1c1d90;
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
        { crate::pose::cover(0x1c1d90, previous, pc); previous = pc; }
        match pc {
            0x1c1d90 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x18));
                edi = ((edi as i32).wrapping_mul((0x3c) as i32)) as u32;
                edi = edi.wrapping_add(0x4fa134);
                ebp = 0x2;
                pc = 0x1c1db0; continue;
            }
            0x1c1db0 => {
                eax = m.read32(edi);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x1c1dfc; continue; }
                pc = 0x1c1db7; continue;
            }
            0x1c1db7 => {
                fa = eax;
                fb = ((((0x42i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1c1dfc; continue; }
                pc = 0x1c1dbc; continue;
            }
            0x1c1dbc => {
                ecx = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                esi = eax.wrapping_add(ecx);
                eax = m.read32(esi.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1c1de1; continue; }
                pc = 0x1c1dd2; continue;
            }
            0x1c1dd2 => {
                edx = (((((m.read16(esi.wrapping_add(0x28)) as i16)) as i32)) as u32);
                push!(edx);
                push!(eax);
                push!(ebx);
                call!(0x1ba220);
                pc = 0x1c1dde; continue;
            }
            0x1c1dde => {
                esp = esp.wrapping_add(0xc);
                pc = 0x1c1de1; continue;
            }
            0x1c1de1 => {
                eax = m.read32(esi.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1c1dfc; continue; }
                pc = 0x1c1de8; continue;
            }
            0x1c1de8 => {
                fa = eax;
                fb = m.read32(esi.wrapping_add(0x14));
                if (fa == fb) { pc = 0x1c1dfc; continue; }
                pc = 0x1c1ded; continue;
            }
            0x1c1ded => {
                ecx = (((((m.read16(esi.wrapping_add(0x2a)) as i16)) as i32)) as u32);
                push!(ecx);
                push!(eax);
                push!(ebx);
                call!(0x1ba220);
                pc = 0x1c1df9; continue;
            }
            0x1c1df9 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x1c1dfc; continue;
            }
            0x1c1dfc => {
                edi = edi.wrapping_add(0x4);
                ebp = ebp.wrapping_sub(1);
                fa = ebp;
                if ((fa != 0)) { pc = 0x1c1db0; continue; }
                pc = 0x1c1e02; continue;
            }
            0x1c1e02 => {
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

/// 0x1BAC50. INFERRED: cdecl `(ext)`. Index of the entry with the smallest switch priority (+0x8) below 100 that passes 0x1BA410, else 0.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn lowest_priority_selectable_entry<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1bac50;
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
        { crate::pose::cover(0x1bac50, previous, pc); previous = pc; }
        match pc {
            0x1bac50 => {
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                ebp = 0;
                ebx = 0x64;
                edi = 0;
                esi = 0x4fa130;
                pc = 0x1bac62; continue;
            }
            0x1bac62 => {
                fa = m.read32(esi);
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1bac7e; continue; }
                pc = 0x1bac66; continue;
            }
            0x1bac66 => {
                eax = m.read32(esp.wrapping_add(0x14));
                push!(0);
                push!(edi);
                push!(eax);
                call!(0x1ba410);
                pc = 0x1bac73; continue;
            }
            0x1bac73 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bac7e; continue; }
                pc = 0x1bac7a; continue;
            }
            0x1bac7a => {
                ebx = m.read32(esi);
                ebp = edi;
                pc = 0x1bac7e; continue;
            }
            0x1bac7e => {
                esi = esi.wrapping_add(0x3c);
                edi = edi.wrapping_add(1);
                fa = esi;
                fb = ((((0x4fb198i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1bac62; continue; }
                pc = 0x1bac8a; continue;
            }
            0x1bac8a => {
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


/// 0x1BFC00. INFERRED: cdecl `(ext, entry)`. Clears the owned byte; when the entry was the current (+0x94) or secondary (+0x9C) one, reselects through 0x1BAC50 and 0x1BA130 and repairs the order byte through 0x1B6BB0.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn weapon_entry_release<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1bfc00;
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
        { crate::pose::cover(0x1bfc00, previous, pc); previous = pc; }
        match pc {
            0x1bfc00 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x10));
                m.write8(esi.wrapping_add(edi).wrapping_add(0x54c), (0) as u8);
                fa = m.read32(esi.wrapping_add(0x94));
                fb = edi;
                if (fa != fb) { pc = 0x1bfc34; continue; }
                pc = 0x1bfc1a; continue;
            }
            0x1bfc1a => {
                push!(esi);
                m.write32(esi.wrapping_add(0x94), 0);
                call!(0x1bac50);
                pc = 0x1bfc2a; continue;
            }
            0x1bfc2a => {
                push!(eax);
                push!(esi);
                call!(0x1ba130);
                pc = 0x1bfc31; continue;
            }
            0x1bfc31 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x1bfc34; continue;
            }
            0x1bfc34 => {
                fa = m.read32(esi.wrapping_add(0x9c));
                fb = edi;
                if (fa != fb) { pc = 0x1bfc59; continue; }
                pc = 0x1bfc3c; continue;
            }
            0x1bfc3c => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bfc59; continue; }
                pc = 0x1bfc40; continue;
            }
            0x1bfc40 => {
                push!(esi);
                m.write32(esi.wrapping_add(0x9c), 0);
                call!(0x1bac50);
                pc = 0x1bfc50; continue;
            }
            0x1bfc50 => {
                esp = esp.wrapping_add(0x4);
                m.write32(esi.wrapping_add(0x9c), eax);
                pc = 0x1bfc59; continue;
            }
            0x1bfc59 => {
                eax = m.read32(esi.wrapping_add(0x8));
                fa = eax;
                fb = m.read32(0x790950);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1bfc97; continue; }
                pc = 0x1bfc64; continue;
            }
            0x1bfc64 => {
                ecx = m.read32(0x7b0c48);
                eax = ((eax as i32).wrapping_mul((0x1584) as i32)) as u32;
                eax = eax.wrapping_add(ecx);
                esi = eax;
                eax = edi;
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x4fa160));
                edx = ((m.read8(ecx.wrapping_add(esi).wrapping_add(0xe3c))) as u8 as u32);
                fa = edx;
                fb = edi;
                if (fa != fb) { pc = 0x1bfc97; continue; }
                pc = 0x1bfc8b; continue;
            }
            0x1bfc8b => {
                call!(0x1b6bb0);
                pc = 0x1bfc90; continue;
            }
            0x1bfc90 => {
                m.write8(ecx.wrapping_add(esi).wrapping_add(0xe3c), (((((eax) as u8)) as u32)) as u8);
                pc = 0x1bfc97; continue;
            }
            0x1bfc97 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0xD7BC0. INFERRED: register/cdecl reset of the player extension block at spawn (owned bytes, ammo words, current weapon, hand structs through 0x1B7A30 and 0xDB2E0).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ext_block_reset<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xd7bc0;
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
        { crate::pose::cover(0xd7bc0, previous, pc); previous = pc; }
        match pc {
            0xd7bc0 => {
                edx = m.read32(0x790950);
                push!(ebx);
                push!(ebp);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(esi.wrapping_add(0x8));
                eax = 0;
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = (eax & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edx = esi.wrapping_add(0x304);
                push!(edi);
                push!(edx);
                ebp = eax;
                call!(0xdb2e0);
                pc = 0xd7be6; continue;
            }
            0xd7be6 => {
                ebx = 0;
                m.write8(esi.wrapping_add(0x54c), (((((ebx) as u8)) as u32)) as u8);
                m.write8(esi.wrapping_add(0x592), (((((ebx) as u8)) as u32)) as u8);
                m.write8(esi.wrapping_add(0x54d), (((((ebx) as u8)) as u32)) as u8);
                m.write8(esi.wrapping_add(0x593), (((((ebx) as u8)) as u32)) as u8);
                eax = m.read32(0x7de458);
                eax = eax.wrapping_shr((((((((0xdi32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax & 0xffff_ff00) | ((((!((eax) as u8)) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                eax = (eax & 0xffff_ff00) | (((((((((eax) as u8)) as i32) & 0x1i32)) as u32)) & 0xff);
                m.write8(esi.wrapping_add(ebp).wrapping_add(0x54c), (((((eax) as u8)) as u32)) as u8);
                eax = esi.wrapping_add(0x594);
                ecx = 0x44;
                pc = 0xd7c21; continue;
            }
            0xd7c21 => {
                m.write8(eax.wrapping_add(((0x46i32.wrapping_neg()) as u32)), (((((ebx) as u8)) as u32)) as u8);
                m.write8(eax, (((((ebx) as u8)) as u32)) as u8);
                eax = eax.wrapping_add(1);
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0xd7c21; continue; }
                pc = 0xd7c2a; continue;
            }
            0xd7c2a => {
                ecx = esi.wrapping_add(0x5d8);
                eax = 0x4fa13c;
                pc = 0xd7c35; continue;
            }
            0xd7c35 => {
                edx = (edx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                eax = eax.wrapping_add(0x3c);
                m.write8(ecx, (((((edx) as u8)) as u32)) as u8);
                ecx = ecx.wrapping_add(1);
                fa = eax;
                fb = ((((0x4fb1a4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0xd7c35; continue; }
                pc = 0xd7c44; continue;
            }
            0xd7c44 => {
                eax = 0;
                ecx = 0x16;
                edi = esi.wrapping_add(0x61e);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                m.write32(esi.wrapping_add(0x80), ebx);
                eax = m.read32(0x7de458);
                eax = (eax & 0x2000);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = !eax;
                ecx = esi.wrapping_add(0xa4);
                eax = (eax & ebp);
                push!(ecx);
                m.write32(esi.wrapping_add(0x94), eax);
                m.write32(esi.wrapping_add(0x98), ebx);
                m.write32(esi.wrapping_add(0xa0), ebx);
                call!(0x1b7a30);
                pc = 0xd7c89; continue;
            }
            0xd7c89 => {
                edx = esi.wrapping_add(0x1d4);
                push!(edx);
                call!(0x1b7a30);
                pc = 0xd7c95; continue;
            }
            0xd7c95 => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                esp = esp.wrapping_add(0x8);
                m.write32(esi.wrapping_add(0x544), xmm0.lanes[0]);
                xmm0 = Xmm::default();
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esi.wrapping_add(0xe88), ebx);
                m.write32(esi.wrapping_add(0x548), xmm0.lanes[0]);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
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
pub const ROOTS: [u32; 11] = [
    0x1b6b40, 0x1b68f0, 0x1b66e0, 0x1b6c90, 0x1b9d30, 0x1ba130, 0x1ba220, 0x1c1d90, 0x1bac50,
    0x1bfc00, 0xd7bc0,
];

/// Differential entry (not selector-registered). True when the Rust body ran.
#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x1b6b40 => renumber_entry_groups(m, regs),
        0x1b68f0 => group_has_owned_x2_entry(m, regs),
        0x1b66e0 => ammo_take_counter(m, regs),
        0x1b6c90 => ammo_total_covers_cost(m, regs),
        0x1b9d30 => fill_slot_order_bytes(m, regs),
        0x1ba130 => set_current_weapon_slot(m, regs),
        0x1ba220 => ammo_add_reserve_clamped(m, guest, regs),
        0x1c1d90 => ammo_add_default_amounts(m, guest, regs),
        0x1bac50 => lowest_priority_selectable_entry(m, guest, regs),
        0x1bfc00 => weapon_entry_release(m, guest, regs),
        0xd7bc0 => ext_block_reset(m, guest, regs),
        _ => return false,
    }
    true
}
// ---- dispatch end ----
