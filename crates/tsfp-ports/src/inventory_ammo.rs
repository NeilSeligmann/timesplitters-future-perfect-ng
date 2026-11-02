// SPDX-License-Identifier: GPL-3.0-or-later
//! T1850 (T1815 slice 3): ammo helpers (counts, reserve, reload, clip transfer).
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
/// 0x1B67E0. INFERRED: read in full: for slots +0x98 (table 4fa134 stride 0x3c) and 4fa138, record +0x14/+0x1c ammo type equals arg then adds word at player+0xa4+0x38+type*2 and player+0x1d4+...
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_ammo_count_for_type_summed_over_two_equ<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b67e0;
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
        { crate::pose::cover(0x1b67e0, previous, pc); previous = pc; }
        match pc {
            0x1b67e0 => {
                push!(ebx);
                push!(ebp);
                ebp = m.read32(0x4f9bac);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x10));
                edx = m.read32(esi.wrapping_add(0x98));
                edx = ((edx as i32).wrapping_mul((0x3c) as i32)) as u32;
                ecx = m.read32(edx.wrapping_add(0x4fa134));
                eax = 0;
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                push!(edi);
                ebx = esi.wrapping_add(0xa4);
                if (fa == fb) { pc = 0x1b6839; continue; }
                pc = 0x1b680a; continue;
            }
            0x1b680a => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6839; continue; }
                pc = 0x1b680e; continue;
            }
            0x1b680e => {
                ecx = ((ecx as i32).wrapping_mul((0x29c) as i32)) as u32;
                edi = m.read32(ecx.wrapping_add(ebp).wrapping_add(0x14));
                ecx = ecx.wrapping_add(ebp);
                fa = edi;
                fb = m.read32(esp.wrapping_add(0x18));
                if (fa != fb) { pc = 0x1b6827; continue; }
                pc = 0x1b6820; continue;
            }
            0x1b6820 => {
                eax = (((((m.read16(ebx.wrapping_add(edi.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                pc = 0x1b6839; continue;
            }
            0x1b6827 => {
                ecx = m.read32(ecx.wrapping_add(0x1c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6839; continue; }
                pc = 0x1b682e; continue;
            }
            0x1b682e => {
                fa = ecx;
                fb = m.read32(esp.wrapping_add(0x18));
                if (fa != fb) { pc = 0x1b6839; continue; }
                pc = 0x1b6834; continue;
            }
            0x1b6834 => {
                eax = (((((m.read16(ebx.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                pc = 0x1b6839; continue;
            }
            0x1b6839 => {
                edx = m.read32(edx.wrapping_add(0x4fa138));
                esi = esi.wrapping_add(0x1d4);
                fa = edx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6880; continue; }
                pc = 0x1b684a; continue;
            }
            0x1b684a => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6880; continue; }
                pc = 0x1b684e; continue;
            }
            0x1b684e => {
                edi = m.read32(esp.wrapping_add(0x18));
                edx = ((edx as i32).wrapping_mul((0x29c) as i32)) as u32;
                ecx = m.read32(edx.wrapping_add(ebp).wrapping_add(0x14));
                edx = edx.wrapping_add(ebp);
                fa = ecx;
                fb = edi;
                if (fa != fb) { pc = 0x1b686e; continue; }
                pc = 0x1b6862; continue;
            }
            0x1b6862 => {
                ecx = (((((m.read16(esi.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = eax.wrapping_add(ecx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1b686e => {
                edx = m.read32(edx.wrapping_add(0x1c));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6880; continue; }
                pc = 0x1b6875; continue;
            }
            0x1b6875 => {
                fa = edx;
                fb = edi;
                if (fa != fb) { pc = 0x1b6880; continue; }
                pc = 0x1b6879; continue;
            }
            0x1b6879 => {
                edx = (((((m.read16(esi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                eax = eax.wrapping_add(edx);
                pc = 0x1b6880; continue;
            }
            0x1b6880 => {
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


/// 0x1B6890. INFERRED: checks owned byte at +0x54c+slot, ammo type from 4f9bac+0x14, 1b67e0 plus word +0x61e, 87 bytes
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ammo_slot_owned_total_via_1b67e0_plus_reserve<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b6890;
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
        { crate::pose::cover(0x1b6890, previous, pc); previous = pc; }
        match pc {
            0x1b6890 => {
                eax = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0xc));
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(eax).wrapping_add(0x54c))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x1b68e2; continue; }
                pc = 0x1b68a5; continue;
            }
            0x1b68a5 => {
                ecx = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(0x4fa138));
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                esi = m.read32(eax.wrapping_add(ecx).wrapping_add(0x14));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b68e2; continue; }
                pc = 0x1b68c2; continue;
            }
            0x1b68c2 => {
                push!(esi);
                push!(edi);
                call!(0x1b67e0);
                pc = 0x1b68c9; continue;
            }
            0x1b68c9 => {
                edx = (((((m.read16(edi.wrapping_add(esi.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                eax = eax.wrapping_add(edx);
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x1b68e2; continue; }
                pc = 0x1b68da; continue;
            }
            0x1b68da => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1b68e2 => {
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


/// 0x1B6950. INFERRED: slot 0x2d uses table_737bd4_count_matching_owner_0x104 else reserve of 4f9bac record, 75 bytes
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn object_slot_ammo_available_special_slot_0x2d_v<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b6950;
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
        { crate::pose::cover(0x1b6950, previous, pc); previous = pc; }
        match pc {
            0x1b6950 => {
                ecx = m.read32(esp.wrapping_add(0x8));
                eax = ecx;
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                fa = ecx;
                fb = ((((0x2di32)) as u32) & 0xffffffffu32);
                eax = m.read32(eax.wrapping_add(0x4fa138));
                if (fa != fb) { pc = 0x1b697b; continue; }
                pc = 0x1b6964; continue;
            }
            0x1b6964 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                push!(ecx);
                call!(0xe02b0);
                pc = 0x1b696e; continue;
            }
            0x1b696e => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6998; continue; }
                pc = 0x1b6975; continue;
            }
            0x1b6975 => {
                eax = 0x1;
                break 'run;
            }
            0x1b697b => {
                edx = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(edx).wrapping_add(0x14));
                eax = eax.wrapping_add(edx);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1b6998; continue; }
                pc = 0x1b6991; continue;
            }
            0x1b6991 => {
                ecx = m.read32(eax.wrapping_add(0x1c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6975; continue; }
                pc = 0x1b6998; continue;
            }
            0x1b6998 => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x1B6D10. INFERRED: returns loaded word >= cost when the clip size (+0x18/+0x20) is not 0 or 1, else tail calls 1b6c90 with the same arguments
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ammo_loaded_covers_fire_cost_else_total_covers<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b6d10;
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
        { crate::pose::cover(0x1b6d10, previous, pc); previous = pc; }
        match pc {
            0x1b6d10 => {
                eax = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(eax.wrapping_add(0x30));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1b6da7; continue; }
                pc = 0x1b6d1f; continue;
            }
            0x1b6d1f => {
                push!(ebp);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                ebp = m.read32(esp.wrapping_add(0x10));
                ecx = ebp;
                ecx = ((ecx as i32).wrapping_mul((0x7c) as i32)) as u32;
                push!(esi);
                eax = eax.wrapping_add(m.read32(0x4f9bac));
                edx = m.read32(ecx.wrapping_add(eax).wrapping_add(0x68));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = ecx.wrapping_add(eax).wrapping_add(0x68);
                if (((((fa).wrapping_sub((fb))).wrapping_shr((0x1f & 31))) == 0)) { pc = 0x1b6d45; continue; }
                pc = 0x1b6d42; continue;
            }
            0x1b6d42 => {
                ecx = eax.wrapping_add(0x68);
                pc = 0x1b6d45; continue;
            }
            0x1b6d45 => {
                esi = m.read32(eax.wrapping_add(0x14));
                edx = m.read32(eax.wrapping_add(0x18));
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x1c));
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(edi);
                if (fa != fb) { pc = 0x1b6d5a; continue; }
                pc = 0x1b6d55; continue;
            }
            0x1b6d55 => {
                edi = m.read32(ecx.wrapping_add(0x24));
                pc = 0x1b6d5d; continue;
            }
            0x1b6d5a => {
                edi = m.read32(ecx.wrapping_add(0x28));
                pc = 0x1b6d5d; continue;
            }
            0x1b6d5d => {
                fa = ebp;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1b6d6e; continue; }
                pc = 0x1b6d62; continue;
            }
            0x1b6d62 => {
                ecx = m.read32(eax.wrapping_add(0x1c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6d6e; continue; }
                pc = 0x1b6d69; continue;
            }
            0x1b6d69 => {
                edx = m.read32(eax.wrapping_add(0x20));
                esi = ecx;
                pc = 0x1b6d6e; continue;
            }
            0x1b6d6e => {
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6d8e; continue; }
                pc = 0x1b6d72; continue;
            }
            0x1b6d72 => {
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6d8e; continue; }
                pc = 0x1b6d77; continue;
            }
            0x1b6d77 => {
                edx = m.read32(esp.wrapping_add(0x18));
                eax = (((((m.read16(edx.wrapping_add(esi.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                ecx = 0;
                fa = eax;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fas >= fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ecx;
                break 'run;
            }
            0x1b6d8e => {
                edx = m.read32(esp.wrapping_add(0x18));
                eax = m.read32(esp.wrapping_add(0x14));
                push!(ebx);
                push!(ebp);
                push!(edx);
                push!(eax);
                call!(0x1b6c90);
                pc = 0x1b6d9f; continue;
            }
            0x1b6d9f => {
                esp = esp.wrapping_add(0x10);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1b6da7 => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x1B6E20. INFERRED: min(reserve word +0x61e, clip size record+0x18 minus loaded word +0x38) for ammo type +0x14 and +0x1c, summed
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ammo_compute_reloadable_amount_for_both_ammo_t<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b6e20;
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
        { crate::pose::cover(0x1b6e20, previous, pc); previous = pc; }
        match pc {
            0x1b6e20 => {
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xc));
                ecx = m.read32(ebp.wrapping_add(0x30));
                ecx = ((ecx as i32).wrapping_mul((0x29c) as i32)) as u32;
                push!(edi);
                edi = m.read32(0x4f9bac);
                ebx = m.read32(ecx.wrapping_add(edi).wrapping_add(0x14));
                ecx = ecx.wrapping_add(edi);
                edi = (edi & 0xffff_0000) | ((((m.read16(esi.wrapping_add(ebx.wrapping_mul(0x2)).wrapping_add(0x61e))) as u32)) & 0xffff);
                eax = 0;
                fa = ((((((edi) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas <= fbs) { pc = 0x1b6e75; continue; }
                pc = 0x1b6e4b; continue;
            }
            0x1b6e4b => {
                edx = m.read32(ecx.wrapping_add(0x18));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6e75; continue; }
                pc = 0x1b6e52; continue;
            }
            0x1b6e52 => {
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6e75; continue; }
                pc = 0x1b6e57; continue;
            }
            0x1b6e57 => {
                eax = ebx;
                ebp = (((((m.read16(ebp.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                eax = (((((m.read16(esi.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                edx = edx.wrapping_sub(ebp);
                fa = edx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x1b6e6f; continue; }
                pc = 0x1b6e6c; continue;
            }
            0x1b6e6c => {
                edx = ((((edi) as u16)) as u16 as i16 as i32 as u32);
                pc = 0x1b6e6f; continue;
            }
            0x1b6e6f => {
                ebp = m.read32(esp.wrapping_add(0x10));
                eax = edx;
                pc = 0x1b6e75; continue;
            }
            0x1b6e75 => {
                edx = m.read32(ecx.wrapping_add(0x1c));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6eb7; continue; }
                pc = 0x1b6e7c; continue;
            }
            0x1b6e7c => {
                fa = edx;
                fb = ebx;
                if (fa == fb) { pc = 0x1b6eb7; continue; }
                pc = 0x1b6e80; continue;
            }
            0x1b6e80 => {
                edi = (edi & 0xffff_0000) | ((((m.read16(esi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x61e))) as u32)) & 0xffff);
                fa = ((((((edi) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas <= fbs) { pc = 0x1b6eb7; continue; }
                pc = 0x1b6e8d; continue;
            }
            0x1b6e8d => {
                edx = m.read32(ecx.wrapping_add(0x20));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6eb7; continue; }
                pc = 0x1b6e94; continue;
            }
            0x1b6e94 => {
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6eb7; continue; }
                pc = 0x1b6e99; continue;
            }
            0x1b6e99 => {
                edx = m.read32(ecx.wrapping_add(0x1c));
                ebx = (((((m.read16(ebp.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                ecx = m.read32(ecx.wrapping_add(0x18));
                edx = (((((m.read16(esi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                ecx = ecx.wrapping_sub(ebx);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x1b6eb5; continue; }
                pc = 0x1b6eb2; continue;
            }
            0x1b6eb2 => {
                ecx = ((((edi) as u16)) as u16 as i16 as i32 as u32);
                pc = 0x1b6eb5; continue;
            }
            0x1b6eb5 => {
                eax = eax.wrapping_add(ecx);
                pc = 0x1b6eb7; continue;
            }
            0x1b6eb7 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x1B6EC0. INFERRED: same min computation then adds to loaded words +0x38 and subtracts from reserve words +0x61e (clamped by +0x90 table), returns total moved
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ammo_reload_transfer_reserve_to_clip_for_both<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b6ec0;
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
        { crate::pose::cover(0x1b6ec0, previous, pc); previous = pc; }
        match pc {
            0x1b6ec0 => {
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                esi = eax;
                ecx = m.read32(esi.wrapping_add(0x30));
                ecx = ((ecx as i32).wrapping_mul((0x29c) as i32)) as u32;
                push!(edi);
                edi = m.read32(0x4f9bac);
                eax = 0;
                ecx = ecx.wrapping_add(edi);
                m.write32(esi.wrapping_add(0xf8), eax);
                edi = m.read32(ecx.wrapping_add(0x14));
                ebx = (ebx & 0xffff_0000) | ((((m.read16(ebp.wrapping_add(edi.wrapping_mul(0x2)).wrapping_add(0x61e))) as u32)) & 0xffff);
                fa = ((((((ebx) as u16))) as u32) & 0xffffu32);
                fb = ((((((eax) as u16))) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas <= fbs) { pc = 0x1b6f64; continue; }
                pc = 0x1b6ef3; continue;
            }
            0x1b6ef3 => {
                edx = m.read32(ecx.wrapping_add(0x18));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6f64; continue; }
                pc = 0x1b6efa; continue;
            }
            0x1b6efa => {
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6f64; continue; }
                pc = 0x1b6eff; continue;
            }
            0x1b6eff => {
                eax = (((((m.read16(esi.wrapping_add(edi.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                edx = edx.wrapping_sub(eax);
                eax = edx;
                edx = m.read32(esp.wrapping_add(0x18));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6f28; continue; }
                pc = 0x1b6f10; continue;
            }
            0x1b6f10 => {
                fa = m.read32(edx.wrapping_add(0x44));
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1b6f28; continue; }
                pc = 0x1b6f16; continue;
            }
            0x1b6f16 => {
                edx = m.read32(ecx.wrapping_add(0xc));
                fa = (((((((edx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x1b6f28; continue; }
                pc = 0x1b6f1e; continue;
            }
            0x1b6f1e => {
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x1b6f28; continue; }
                pc = 0x1b6f23; continue;
            }
            0x1b6f23 => {
                eax = 0x1;
                pc = 0x1b6f28; continue;
            }
            0x1b6f28 => {
                edx = ((((ebx) as u16)) as u16 as i16 as i32 as u32);
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { eax = edx; }
                edx = m.read32(esp.wrapping_add(0x1c));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6f46; continue; }
                pc = 0x1b6f38; continue;
            }
            0x1b6f38 => {
                edx = (((((m.read16(esi.wrapping_add(edi.wrapping_mul(0x2)).wrapping_add(0x90)) as i16)) as i32)) as u32);
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x1b6f46; continue; }
                pc = 0x1b6f44; continue;
            }
            0x1b6f44 => {
                eax = edx;
                pc = 0x1b6f46; continue;
            }
            0x1b6f46 => {
                edx = m.read32(ecx.wrapping_add(0x14));
                write16(m, esi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x38), (((((m.read16(esi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x38))) as i32).wrapping_add(((((eax) as u16)) as i32))) as u32)) as u16);
                edx = esi.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x38);
                edx = m.read32(ecx.wrapping_add(0x14));
                write16(m, ebp.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x61e), (((((m.read16(ebp.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x61e))) as i32).wrapping_sub(((((eax) as u16)) as i32))) as u32)) as u16);
                edx = ebp.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x61e);
                pc = 0x1b6f64; continue;
            }
            0x1b6f64 => {
                edx = m.read32(ecx.wrapping_add(0x1c));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6fe1; continue; }
                pc = 0x1b6f6b; continue;
            }
            0x1b6f6b => {
                fa = edx;
                fb = m.read32(ecx.wrapping_add(0x14));
                if (fa == fb) { pc = 0x1b6fe1; continue; }
                pc = 0x1b6f70; continue;
            }
            0x1b6f70 => {
                edx = (((((m.read16(ebp.wrapping_add(edx.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                fa = ((((((edx) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                m.write32(esp.wrapping_add(0x14), edx);
                if (fas <= fbs) { pc = 0x1b6fe1; continue; }
                pc = 0x1b6f81; continue;
            }
            0x1b6f81 => {
                edx = m.read32(ecx.wrapping_add(0x20));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6fe1; continue; }
                pc = 0x1b6f88; continue;
            }
            0x1b6f88 => {
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6fe1; continue; }
                pc = 0x1b6f8d; continue;
            }
            0x1b6f8d => {
                edi = m.read32(ecx.wrapping_add(0x1c));
                ebx = (((((m.read16(esi.wrapping_add(edi.wrapping_mul(0x2)).wrapping_add(0x38)) as i16)) as i32)) as u32);
                edi = (((((m.read16(ebp.wrapping_add(edi.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                edx = edx.wrapping_sub(ebx);
                fa = edx;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x1b6fa8; continue; }
                pc = 0x1b6fa3; continue;
            }
            0x1b6fa3 => {
                edx = (((((m.read16(esp.wrapping_add(0x14)) as i16)) as i32)) as u32);
                pc = 0x1b6fa8; continue;
            }
            0x1b6fa8 => {
                edi = m.read32(esp.wrapping_add(0x1c));
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1b6fc1; continue; }
                pc = 0x1b6fb0; continue;
            }
            0x1b6fb0 => {
                edi = m.read32(ecx.wrapping_add(0x1c));
                edi = (((((m.read16(esi.wrapping_add(edi.wrapping_mul(0x2)).wrapping_add(0x90)) as i16)) as i32)) as u32);
                fa = edx;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x1b6fc1; continue; }
                pc = 0x1b6fbf; continue;
            }
            0x1b6fbf => {
                edx = edi;
                pc = 0x1b6fc1; continue;
            }
            0x1b6fc1 => {
                edi = m.read32(ecx.wrapping_add(0x1c));
                write16(m, esi.wrapping_add(edi.wrapping_mul(0x2)).wrapping_add(0x38), (((((m.read16(esi.wrapping_add(edi.wrapping_mul(0x2)).wrapping_add(0x38))) as i32).wrapping_add(((((edx) as u16)) as i32))) as u32)) as u16);
                ecx = m.read32(ecx.wrapping_add(0x1c));
                write16(m, ebp.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x61e), (((((m.read16(ebp.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x61e))) as i32).wrapping_sub(((((edx) as u16)) as i32))) as u32)) as u16);
                esi = esi.wrapping_add(edi.wrapping_mul(0x2)).wrapping_add(0x38);
                ecx = ebp.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0x61e);
                eax = eax.wrapping_add(edx);
                pc = 0x1b6fe1; continue;
            }
            0x1b6fe1 => {
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


/// 0x1B9760. INFERRED: adds loaded word +0x38 to reserve word +0x61e and zeroes it for ammo types +0x14 and +0x1c of the record
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ammo_unload_clip_back_into_reserve_both_types<M: GuestMemory, C: GuestCall>(m: &mut M, _guest: &mut C, regs: &mut ExtRegisters) {
    const WEAPON_RECORD_ID: u32 = 0x30;
    const RECORD_TABLE_BASE: u32 = 0x4f9bac;
    const RECORD_STRIDE: u32 = 0x29c;
    const AMMO_TYPES: [u32; 2] = [0x14, 0x1c];
    const CLIP_COUNTS: u32 = 0x38;
    const RESERVE_COUNTS: u32 = 0x61e;

    let entry_esp = regs.esp;
    let saved_esi = regs.esi;
    let saved_ebx = regs.ebx;
    let mut esp = entry_esp.wrapping_sub(4);
    m.write32(esp, saved_esi);
    let slot = m.read32(esp.wrapping_add(0xc));
    let record_id = m.read32(slot.wrapping_add(WEAPON_RECORD_ID));
    regs.eax = record_id;
    if record_id != u32::MAX {
        let table_base = m.read32(RECORD_TABLE_BASE);
        let record = table_base.wrapping_add(record_id.wrapping_mul(RECORD_STRIDE));
        let first_type = m.read32(record.wrapping_add(AMMO_TYPES[0]));
        let player = m.read32(esp.wrapping_add(8));
        esp = esp.wrapping_sub(4);
        m.write32(esp, saved_ebx);
        regs.edx = player;
        regs.ecx = first_type;
        // A zero type is absent. Read the second field after the first transfer,
        // matching the guest's access order even if the memory adapter is live.
        for (index, type_field) in AMMO_TYPES.into_iter().enumerate() {
            if index != 0 {
                regs.ecx = m.read32(record.wrapping_add(type_field));
            }
            let ammo_type = regs.ecx;
            if ammo_type != 0 {
                let offset = ammo_type.wrapping_mul(2);
                let clip_addr = slot.wrapping_add(offset).wrapping_add(CLIP_COUNTS);
                let reserve_addr = player.wrapping_add(offset).wrapping_add(RESERVE_COUNTS);
                let clip = m.read16(clip_addr);
                let reserve = m.read16(reserve_addr);
                write16(m, reserve_addr, reserve.wrapping_add(clip));
                if index == 0 {
                    regs.ecx = m.read32(record.wrapping_add(type_field));
                    let offset = regs.ecx.wrapping_mul(2);
                    write16(m, slot.wrapping_add(offset).wrapping_add(CLIP_COUNTS), 0);
                } else {
                    regs.edx = m.read32(record.wrapping_add(type_field));
                    let offset = regs.edx.wrapping_mul(2);
                    write16(m, slot.wrapping_add(offset).wrapping_add(CLIP_COUNTS), 0);
                }
            }
        }
        regs.eax = record;
        regs.ebx = m.read32(esp);
        esp = esp.wrapping_add(4);
    }
    regs.esi = m.read32(esp);
    regs.esp = esp.wrapping_add(4);
}


/// 0x1BA2B0. INFERRED: slot table 4fa128 +0x10 to record, multiplies amount by clip size +0x18 (min 1) and calls the 1ba220 add with ammo type +0x14
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ammo_add_clips_of_slot_weapon_to_reserve<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1ba2b0;
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
        { crate::pose::cover(0x1ba2b0, previous, pc); previous = pc; }
        match pc {
            0x1ba2b0 => {
                eax = m.read32(esp.wrapping_add(0x8));
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(0x4fa138));
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba2f2; continue; }
                pc = 0x1ba2c2; continue;
            }
            0x1ba2c2 => {
                ecx = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                eax = eax.wrapping_add(ecx);
                ecx = m.read32(eax.wrapping_add(0x14));
                eax = m.read32(eax.wrapping_add(0x18));
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1ba2e0; continue; }
                pc = 0x1ba2db; continue;
            }
            0x1ba2db => {
                eax = 0x1;
                pc = 0x1ba2e0; continue;
            }
            0x1ba2e0 => {
                eax = ((eax as i32).wrapping_mul((m.read32(esp.wrapping_add(0xc))) as i32)) as u32;
                m.write32(esp.wrapping_add(0xc), eax);
                m.write32(esp.wrapping_add(0x8), ecx);
                tail_to!(0x1ba220); break 'run;  // direct tail jump
            }
            0x1ba2f2 => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


const AMMO_ENTRY_TABLE: u32 = 0x4fa138;
const AMMO_ENTRY_STRIDE: u32 = 0x3c;
const AMMO_RECORD_BASE: u32 = 0x4f9bac;
const AMMO_RECORD_STRIDE: u32 = 0x29c;
const PRIMARY_AMMO_TYPE_FIELD: u32 = 0x14;
const AMMO_ENTRY_LIMIT: u32 = 0x46;
const AMMO_RECORD_LIMIT: u32 = 0x42;

/// 0x1BADD0. INFERRED: sum signed ammo halfwords from the reserve and up to
/// two equipped slots for the primary ammo type of a weapon entry.
/// Verification: Tier 0 sparse-memory regression; historical Tier 1 is prior
/// inventory-port evidence on fabricated inputs, not a fresh differential.
pub fn ammo_total_across_three_word_arrays_0x61e_0xdc<M: GuestMemory, C: GuestCall>(
    m: &mut M, _guest: &mut C, regs: &mut ExtRegisters,
) {
    let esp = regs.esp;
    let entry_index = m.read32(esp.wrapping_add(8));
    if entry_index >= AMMO_ENTRY_LIMIT { regs.eax = 0; return; }

    let record_index = m.read32(AMMO_ENTRY_TABLE.wrapping_add(entry_index.wrapping_mul(AMMO_ENTRY_STRIDE)));
    if record_index >= AMMO_RECORD_LIMIT { regs.eax = 0; return; }

    let record_base = m.read32(AMMO_RECORD_BASE);
    regs.ecx = record_base;
    let ammo_type = m.read32(record_base.wrapping_add(record_index.wrapping_mul(AMMO_RECORD_STRIDE)).wrapping_add(PRIMARY_AMMO_TYPE_FIELD));
    if ammo_type == 0 { regs.eax = 0; return; }

    let player = m.read32(esp.wrapping_add(4));
    regs.ecx = player;
    let ammo_offset = ammo_type.wrapping_mul(2);
    let mut total = (m.read16(player.wrapping_add(0x61e).wrapping_add(ammo_offset)) as i16 as i32) as u32;
    regs.edx = total;
    // The translated body saves EDI on the guest stack. Preserve the same
    // stack access even though this readable form does not need EDI as scratch.
    let saved_edi = regs.edi;
    m.write32(esp.wrapping_sub(4), saved_edi);
    if m.read32(player.wrapping_add(0xd4)) != u32::MAX {
        let slot_count = (m.read16(player.wrapping_add(0xdc).wrapping_add(ammo_offset)) as i16 as i32) as u32;
        total = total.wrapping_add(slot_count);
        regs.edx = total;
    }
    let second_slot = m.read32(player.wrapping_add(0x204)) != u32::MAX;
    regs.edi = m.read32(esp.wrapping_sub(4));
    if second_slot {
        let slot_count = (m.read16(player.wrapping_add(0x20c).wrapping_add(ammo_offset)) as i16 as i32) as u32;
        regs.eax = slot_count;
        total = total.wrapping_add(slot_count);
        regs.edx = total;
    }
    regs.eax = total;
}


/// 0x1BAE40. INFERRED: read in full: entry index below 0x46 to record index below 0x42, ammo type at record+0x1c, sums words at player+0x61e, +0xdc (if +0xd4 set) and +0x20c (if +0x204 set)
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_second_ammo_type_count_of_weapon_entry<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1bae40;
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
        { crate::pose::cover(0x1bae40, previous, pc); previous = pc; }
        match pc {
            0x1bae40 => {
                eax = m.read32(esp.wrapping_add(0x8));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1baeaa; continue; }
                pc = 0x1bae48; continue;
            }
            0x1bae48 => {
                fa = eax;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1baeaa; continue; }
                pc = 0x1bae4d; continue;
            }
            0x1bae4d => {
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(0x4fa138));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1baeaa; continue; }
                pc = 0x1bae5a; continue;
            }
            0x1bae5a => {
                fa = eax;
                fb = ((((0x42i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1baeaa; continue; }
                pc = 0x1bae5f; continue;
            }
            0x1bae5f => {
                ecx = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(ecx).wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1baeaa; continue; }
                pc = 0x1bae73; continue;
            }
            0x1bae73 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                edx = (((((m.read16(ecx.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0x61e)) as i16)) as i32)) as u32);
                push!(edi);
                fa = m.read32(ecx.wrapping_add(0xd4));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bae93; continue; }
                pc = 0x1bae89; continue;
            }
            0x1bae89 => {
                edi = (((((m.read16(ecx.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0xdc)) as i16)) as i32)) as u32);
                edx = edx.wrapping_add(edi);
                pc = 0x1bae93; continue;
            }
            0x1bae93 => {
                fa = m.read32(ecx.wrapping_add(0x204));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x1baea7; continue; }
                pc = 0x1bae9d; continue;
            }
            0x1bae9d => {
                eax = (((((m.read16(ecx.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0x20c)) as i16)) as i32)) as u32);
                edx = edx.wrapping_add(eax);
                pc = 0x1baea7; continue;
            }
            0x1baea7 => {
                eax = edx;
                break 'run;
            }
            0x1baeaa => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x1BAEB0. INFERRED: revised from game_ammo_slot_has_any_in_1bae40_or (T1473 rename review T1513): dangling or
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ammo_slot_has_any_from_either_count_helper<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1baeb0;
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
        { crate::pose::cover(0x1baeb0, previous, pc); previous = pc; }
        match pc {
            0x1baeb0 => {
                push!(ebx);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x10));
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x10));
                push!(esi);
                push!(edi);
                call!(0x1bae40);
                pc = 0x1baec2; continue;
            }
            0x1baec2 => {
                push!(esi);
                push!(edi);
                ebx = eax;
                call!(0x1badd0);
                pc = 0x1baecb; continue;
            }
            0x1baecb => {
                ebx = ebx.wrapping_add(eax);
                esp = esp.wrapping_add(0x10);
                cf = (((((ebx) != 0))) as i32);
                ebx = (((((ebx) as i32).wrapping_neg())) as u32);
                ebx = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = (((((ebx) as i32).wrapping_neg())) as u32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebx;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x1BFE60. INFERRED: calls 1ba220 for ammo type +0x14 with word 0 and for +0x1c (when different) with word 1 of the arg array
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ammo_add_pair_of_words_for_both_ammo_types_of<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1bfe60;
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
        { crate::pose::cover(0x1bfe60, previous, pc); previous = pc; }
        match pc {
            0x1bfe60 => {
                edx = m.read32(0x4f9bac);
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x10));
                esi = ((esi as i32).wrapping_mul((0x29c) as i32)) as u32;
                eax = m.read32(esi.wrapping_add(edx).wrapping_add(0x14));
                esi = esi.wrapping_add(edx);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x18));
                if (fa == fb) { pc = 0x1bfe93; continue; }
                pc = 0x1bfe85; continue;
            }
            0x1bfe85 => {
                ecx = (((((m.read16(edi) as i16)) as i32)) as u32);
                push!(ecx);
                push!(eax);
                push!(ebx);
                call!(0x1ba220);
                pc = 0x1bfe90; continue;
            }
            0x1bfe90 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x1bfe93; continue;
            }
            0x1bfe93 => {
                eax = m.read32(esi.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bfeae; continue; }
                pc = 0x1bfe9a; continue;
            }
            0x1bfe9a => {
                fa = eax;
                fb = m.read32(esi.wrapping_add(0x14));
                if (fa == fb) { pc = 0x1bfeae; continue; }
                pc = 0x1bfe9f; continue;
            }
            0x1bfe9f => {
                edx = (((((m.read16(edi.wrapping_add(0x2)) as i16)) as i32)) as u32);
                push!(edx);
                push!(eax);
                push!(ebx);
                call!(0x1ba220);
                pc = 0x1bfeab; continue;
            }
            0x1bfeab => {
                esp = esp.wrapping_add(0xc);
                pc = 0x1bfeae; continue;
            }
            0x1bfeae => {
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


const AMMO_RECORD_ID_STRIDE: u32 = 0x3c;
const AMMO_RECORD_TABLE_ADDRESS: u32 = 0x4fa138;
const AMMO_ADD_PAIR_TARGET: u32 = 0x1bfe60;

/// 0x1c1a90. INFERRED: resolves a record ID and tail-calls the ammo-pair helper.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn ammo_add_pair_for_slot_by_record_id<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut xmm = entry.xmm;
    let mut xmm_hi = entry.xmm_hi;
    let fpu = entry.fpu;

    ecx = m.read32(esp.wrapping_add(0x8));
    ecx = ((ecx as i32).wrapping_mul(AMMO_RECORD_ID_STRIDE as i32)) as u32;
    edx = m.read32(ecx.wrapping_add(AMMO_RECORD_TABLE_ADDRESS));
    m.write32(esp.wrapping_add(0x8), edx);

    let stack_slot = esp;
    let saved_stack_value = m.read32(stack_slot);
    esp = esp.wrapping_add(4);

    let mut call_regs = ExtRegisters {
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
        xmm,
        xmm_hi,
    };
    guest.call(AMMO_ADD_PAIR_TARGET, &mut call_regs);
    xmm_hi = call_regs.xmm_hi;
    (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (
        call_regs.eax,
        call_regs.ecx,
        call_regs.edx,
        call_regs.esp,
        call_regs.ebx,
        call_regs.ebp,
        call_regs.esi,
        call_regs.edi,
    );
    xmm = call_regs.xmm;

    esp = esp.wrapping_sub(4);
    m.write32(stack_slot, saved_stack_value);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        xmm_hi,
        ..entry
    };
}


/// 0x1C0080. INFERRED: read in full: ammo from 1bae40 plus 1badd0 nonzero, entry 0x2d special-cases owned type 0x104 count, records without ammo types count as usable
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_weapon_entry_usable_when_ammo_owned_or<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1c0080;
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
        { crate::pose::cover(0x1c0080, previous, pc); previous = pc; }
        match pc {
            0x1c0080 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x14));
                push!(edi);
                push!(ebx);
                call!(0x1bae40);
                pc = 0x1c0092; continue;
            }
            0x1c0092 => {
                push!(edi);
                push!(ebx);
                esi = eax;
                call!(0x1badd0);
                pc = 0x1c009b; continue;
            }
            0x1c009b => {
                esi = esi.wrapping_add(eax);
                esp = esp.wrapping_add(0x10);
                cf = (((((esi) != 0))) as i32);
                esi = (((((esi) as i32).wrapping_neg())) as u32);
                esi = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = edi;
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = m.read32(eax.wrapping_add(0x4fa138));
                esi = esi.wrapping_add(1);
                fa = edi;
                fb = ((((0x2di32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1c00c9; continue; }
                pc = 0x1c00b5; continue;
            }
            0x1c00b5 => {
                push!(ebx);
                call!(0xe02b0);
                pc = 0x1c00bb; continue;
            }
            0x1c00bb => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1c00ec; continue; }
                pc = 0x1c00c2; continue;
            }
            0x1c00c2 => {
                eax = 0x1;
                pc = 0x1c00ee; continue;
            }
            0x1c00c9 => {
                ecx = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                eax = eax.wrapping_add(ecx);
                ecx = m.read32(eax.wrapping_add(0x14));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1c00ec; continue; }
                pc = 0x1c00de; continue;
            }
            0x1c00de => {
                ecx = m.read32(eax.wrapping_add(0x1c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1c00ec; continue; }
                pc = 0x1c00e5; continue;
            }
            0x1c00e5 => {
                eax = 0x1;
                pc = 0x1c00ee; continue;
            }
            0x1c00ec => {
                eax = 0;
                pc = 0x1c00ee; continue;
            }
            0x1c00ee => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x1c00ff; continue; }
                pc = 0x1c00f5; continue;
            }
            0x1c00f5 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1c00ff; continue; }
                pc = 0x1c00f9; continue;
            }
            0x1c00f9 => {
                eax = 0x1;
                break 'run;
            }
            0x1c00ff => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x1B99B0. INFERRED: resolve a loadout entry's primary ammo type.
/// Verification: this rewrite is Tier 0 on FABRICATED sparse memory.
/// The earlier lifted differential is historical evidence, not a rerun of this body.
pub fn loadout_entry_primary_ammo_type_by_entry_index<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    _guest: &mut C,
    regs: &mut ExtRegisters,
) {
    use crate::views::generated_views::{TsfpWeaponEntryView, TsfpWeaponRecordView};

    const ENTRY_COUNT: i32 = 0x46;
    const RECORD_COUNT: i32 = 0x42;
    const ENTRY_TABLE: u32 = 0x4fa128;
    const RECORD_TABLE_POINTER: u32 = 0x4f9bac;

    let entry_index = m.read32(regs.esp.wrapping_add(4)) as i32;
    if !(0..ENTRY_COUNT).contains(&entry_index) {
        regs.eax = 0;
        return;
    }

    let entry = TsfpWeaponEntryView(
        ENTRY_TABLE.wrapping_add((entry_index as u32).wrapping_mul(TsfpWeaponEntryView::SIZE)),
    );
    // Keep the original dword access width: generated scalar readers use bytes.
    let record_index = m.read32(entry.0.wrapping_add(TsfpWeaponEntryView::OFFSET_RECORD_INDEX)) as i32;
    if !(0..RECORD_COUNT).contains(&record_index) {
        regs.eax = 0;
        return;
    }

    regs.ecx = m.read32(RECORD_TABLE_POINTER);
    let record = TsfpWeaponRecordView(
        regs.ecx.wrapping_add((record_index as u32).wrapping_mul(TsfpWeaponRecordView::SIZE)),
    );
    regs.eax = m.read32(record.0.wrapping_add(TsfpWeaponRecordView::OFFSET_AMMO_TYPE_1));
}


/// 0x1B9CF0. INFERRED: read in full: two entries in table 4fa134 stride 0x3c, each passed with the player arg to the record scaling helper 1b5920
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn weapon_entry_both_records_apply_scaled_value_f<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1b9cf0;
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
        { crate::pose::cover(0x1b9cf0, previous, pc); previous = pc; }
        match pc {
            0x1b9cf0 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x10));
                esi = ((esi as i32).wrapping_mul((0x3c) as i32)) as u32;
                push!(edi);
                esi = esi.wrapping_add(0x4fa134);
                edi = 0x2;
                pc = 0x1b9d10; continue;
            }
            0x1b9d10 => {
                eax = m.read32(esi);
                push!(eax);
                push!(ebx);
                call!(0x1b5920);
                pc = 0x1b9d19; continue;
            }
            0x1b9d19 => {
                esp = esp.wrapping_add(0x8);
                esi = esi.wrapping_add(0x4);
                edi = edi.wrapping_sub(1);
                fa = edi;
                if ((fa != 0)) { pc = 0x1b9d10; continue; }
                pc = 0x1b9d22; continue;
            }
            0x1b9d22 => {
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
pub const ROOTS: [u32; 16] = [0x1b67e0, 0x1b6890, 0x1b6950, 0x1b6d10, 0x1b6e20, 0x1b6ec0, 0x1b9760, 0x1ba2b0, 0x1badd0, 0x1bae40, 0x1baeb0, 0x1bfe60, 0x1c1a90, 0x1c0080, 0x1b99b0, 0x1b9cf0];

/// Differential entry (not selector-registered). True when the Rust body ran.
#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0x1b67e0 => player_ammo_count_for_type_summed_over_two_equ(m, guest, regs),
        0x1b6890 => ammo_slot_owned_total_via_1b67e0_plus_reserve(m, guest, regs),
        0x1b6950 => object_slot_ammo_available_special_slot_0x2d_v(m, guest, regs),
        0x1b6d10 => ammo_loaded_covers_fire_cost_else_total_covers(m, guest, regs),
        0x1b6e20 => ammo_compute_reloadable_amount_for_both_ammo_t(m, guest, regs),
        0x1b6ec0 => ammo_reload_transfer_reserve_to_clip_for_both(m, guest, regs),
        0x1b9760 => ammo_unload_clip_back_into_reserve_both_types(m, guest, regs),
        0x1ba2b0 => ammo_add_clips_of_slot_weapon_to_reserve(m, guest, regs),
        0x1badd0 => ammo_total_across_three_word_arrays_0x61e_0xdc(m, guest, regs),
        0x1bae40 => player_second_ammo_type_count_of_weapon_entry(m, guest, regs),
        0x1baeb0 => ammo_slot_has_any_from_either_count_helper(m, guest, regs),
        0x1bfe60 => ammo_add_pair_of_words_for_both_ammo_types_of(m, guest, regs),
        0x1c1a90 => ammo_add_pair_for_slot_by_record_id(m, guest, regs),
        0x1c0080 => player_weapon_entry_usable_when_ammo_owned_or(m, guest, regs),
        0x1b99b0 => loadout_entry_primary_ammo_type_by_entry_index(m, guest, regs),
        0x1b9cf0 => weapon_entry_both_records_apply_scaled_value_f(m, guest, regs),
        _ => return false,
    }
    true
}
// ---- dispatch end ----
