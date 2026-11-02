// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-engine-object/closure/calls_w8
//! T1850 (T1815 slice 4): animation key records and player record bitmasks.
//! Same provenance, labels and conventions as pose.rs (MEASURED against the lifted originals on
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
/// 0x606E0. INFERRED: cdecl `(key) -> *record`. Clamps the signed key to 0..count-1 ([0x7BB260]), then either the stream cache (0x60410) when [0x7BB288] is set or the resident index table at [0x6D780C] selecting a 64-byte key record. No writes.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn anim_key_record_pointer_get<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x606e0;
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
        { crate::pose::cover(0x606e0, previous, pc); previous = pc; }
        match pc {
            0x606e0 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x606eb; continue; }
                pc = 0x606e9; continue;
            }
            0x606e9 => {
                esi = 0;
                pc = 0x606eb; continue;
            }
            0x606eb => {
                eax = m.read32(0x7bb260);
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x606f7; continue; }
                pc = 0x606f4; continue;
            }
            0x606f4 => {
                esi = eax.wrapping_add(((0x1i32.wrapping_neg()) as u32));
                pc = 0x606f7; continue;
            }
            0x606f7 => {
                eax = m.read32(0x7bb288);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x6070d; continue; }
                pc = 0x60700; continue;
            }
            0x60700 => {
                push!(0);
                push!(esi);
                call!(0x60410);
                pc = 0x60708; continue;
            }
            0x60708 => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x6070d => {
                ecx = m.read32(0x6d780c);
                eax = m.read32(ecx.wrapping_add(0x8));
                edx = if ((((eax) as i32) < 0i32)) { 0xffffffffu32 } else { 0 };
                edx = (edx & 0x3);
                eax = eax.wrapping_add(edx);
                eax = (((((eax as i32)).wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                eax = ecx.wrapping_add(eax.wrapping_mul(0x4));
                ecx = m.read32(eax);
                ecx = ecx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_add(esi);
                ecx = m.read32(eax.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x4));
                eax = eax.wrapping_add(0x4);
                ecx = ecx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(ecx);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x60740. INFERRED: cdecl `(key, out16, out12, out4)`. Looks the record up (0x606E0) and copies its first 16 bytes, the next 12 bytes and the dword at +0x1C to the optional out pointers.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn anim_key_record_copy_fields<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x60740;
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
        { crate::pose::cover(0x60740, previous, pc); previous = pc; }
        match pc {
            0x60740 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(eax);
                call!(0x606e0);
                pc = 0x6074a; continue;
            }
            0x6074a => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x60795; continue; }
                pc = 0x60751; continue;
            }
            0x60751 => {
                ecx = m.read32(esp.wrapping_add(0x8));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x6076f; continue; }
                pc = 0x60759; continue;
            }
            0x60759 => {
                edx = m.read32(eax);
                m.write32(ecx, edx);
                edx = m.read32(eax.wrapping_add(0x4));
                m.write32(ecx.wrapping_add(0x4), edx);
                edx = m.read32(eax.wrapping_add(0x8));
                m.write32(ecx.wrapping_add(0x8), edx);
                edx = m.read32(eax.wrapping_add(0xc));
                m.write32(ecx.wrapping_add(0xc), edx);
                pc = 0x6076f; continue;
            }
            0x6076f => {
                ecx = m.read32(esp.wrapping_add(0xc));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x60788; continue; }
                pc = 0x60777; continue;
            }
            0x60777 => {
                edx = m.read32(eax.wrapping_add(0x10));
                m.write32(ecx, edx);
                edx = m.read32(eax.wrapping_add(0x14));
                m.write32(ecx.wrapping_add(0x4), edx);
                edx = m.read32(eax.wrapping_add(0x18));
                m.write32(ecx.wrapping_add(0x8), edx);
                pc = 0x60788; continue;
            }
            0x60788 => {
                ecx = m.read32(esp.wrapping_add(0x10));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x60795; continue; }
                pc = 0x60790; continue;
            }
            0x60790 => {
                eax = m.read32(eax.wrapping_add(0x1c));
                m.write32(ecx, eax);
                pc = 0x60795; continue;
            }
            0x60795 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x65340. INFERRED: cdecl `(slot, bit, count)`. Sets `count` consecutive bit indices starting at `bit` in the mask words at record+0xC0C (word bit>>5, stride 0x10), record from 0x69900(slot).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_record_set_bitmask_range<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x65340;
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
        { crate::pose::cover(0x65340, previous, pc); previous = pc; }
        match pc {
            0x65340 => {
                edx = m.read32(esp.wrapping_add(0x8));
                eax = edx;
                ecx = edx;
                eax = (((((eax as i32)).wrapping_shr((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                ecx = (ecx & 0x8000001fu32);
                fa = ecx;
                fas = (fa as i32);
                if ((fas >= 0i32)) { pc = 0x65358; continue; }
                pc = 0x65353; continue;
            }
            0x65353 => {
                ecx = ecx.wrapping_sub(1);
                ecx = (ecx | 0xffffffe0u32);
                ecx = ecx.wrapping_add(1);
                pc = 0x65358; continue;
            }
            0x65358 => {
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x653a0; continue; }
                pc = 0x6535c; continue;
            }
            0x6535c => {
                fa = edx;
                fb = ((((0x80i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x653a0; continue; }
                pc = 0x65364; continue;
            }
            0x65364 => {
                edx = m.read32(esp.wrapping_add(0xc));
                edx = edx.wrapping_sub(1);
                fa = edx;
                fas = (fa as i32);
                if ((fas < 0i32)) { pc = 0x653a0; continue; }
                pc = 0x6536b; continue;
            }
            0x6536b => {
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                push!(edi);
                edi = 0x1;
                if !(((((((((ecx) as u8))) as u32) & 0x1fu32))) != 0) { fa = if (((fa == 0))) { 0x0u32 } else { (if (((fas < 0i32))) { 0x80000000u32 } else { 0x1u32 }) }; fas = ((fa) as i32); }
                edi = edi.wrapping_shl((((((((((ecx) as u8))) as u32) & 0x1fu32))) as u32 & 31));
                if (((((((((ecx) as u8))) as u32) & 0x1fu32))) != 0) { fa = edi; fas = (fa as i32); }
                esi = eax.wrapping_mul(0x4).wrapping_add(0xc0c);
                ebx = edx.wrapping_add(0x1);
                pc = 0x65384; continue;
            }
            0x65384 => {
                push!(ebp);
                call!(0x69900);
                pc = 0x6538a; continue;
            }
            0x6538a => {
                edx = m.read32(eax.wrapping_add(esi));
                eax = eax.wrapping_add(esi);
                edx = (edx | edi);
                esp = esp.wrapping_add(0x4);
                esi = esi.wrapping_add(0x10);
                ebx = ebx.wrapping_sub(1);
                fa = ebx;
                m.write32(eax, edx);
                if ((fa != 0)) { pc = 0x65384; continue; }
                pc = 0x6539c; continue;
            }
            0x6539c => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x653a0; continue;
            }
            0x653a0 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x66280. INFERRED: cdecl `(slot, group, sub) -> count`. Counts the set bits among the low 13 of the mask word at record+((group+0xFF)*3+sub)*4, record from 0x69900(slot).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_record_count_bits_in_mask<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x66280;
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
        { crate::pose::cover(0x66280, previous, pc); previous = pc; }
        match pc {
            0x66280 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(esp.wrapping_add(0xc));
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                push!(ebp);
                push!(esi);
                eax = eax.wrapping_add(0xff);
                esi = ecx.wrapping_add(eax.wrapping_mul(0x2));
                push!(edi);
                esi = esi.wrapping_add(eax);
                ebp = 0;
                edi = 0;
                esi = esi.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                pc = 0x662a1; continue;
            }
            0x662a1 => {
                push!(ebx);
                call!(0x69900);
                pc = 0x662a7; continue;
            }
            0x662a7 => {
                ecx = edi;
                edx = 0x1;
                edx = edx.wrapping_shl((((((((((ecx) as u8))) as u32) & 0x1fu32))) as u32 & 31));
                ecx = m.read32(eax.wrapping_add(esi));
                esp = esp.wrapping_add(0x4);
                fa = edx;
                fb = ecx;
                if ((fa & fb) == 0) { pc = 0x662bb; continue; }
                pc = 0x662ba; continue;
            }
            0x662ba => {
                ebp = ebp.wrapping_add(1);
                pc = 0x662bb; continue;
            }
            0x662bb => {
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = ((((0xdi32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x662a1; continue; }
                pc = 0x662c1; continue;
            }
            0x662c1 => {
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


// ---- translated bodies end ----

// ---- dispatch begin ----
/// Roots ported in this module (differential-only, unadmitted).
pub const ROOTS: [u32; 4] = [0x606e0, 0x60740, 0x65340, 0x66280];

/// Differential entry (not selector-registered). True when the Rust body ran.
#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0x606e0 => anim_key_record_pointer_get(m, guest, regs),
        0x60740 => anim_key_record_copy_fields(m, guest, regs),
        0x65340 => player_record_set_bitmask_range(m, guest, regs),
        0x66280 => player_record_count_bits_in_mask(m, guest, regs),
        _ => return false,
    }
    true
}
// ---- dispatch end ----
