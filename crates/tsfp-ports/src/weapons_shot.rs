// SPDX-License-Identifier: GPL-3.0-or-later
//! T1831 batch 3: 0x1C0200 "shot start" (fire one pull of the weapon record in EBX for the shooter
//! on the stack). 843 lifted instructions, 167 basic blocks, 25 callees (23 direct, 2 indirect).
//!
//! Provenance and labels: the block structure and every operation were transcribed from the lifted
//! original `sub_001C0200` (private, recomp_0023.c) by a scratch translator and are MEASURED equal to it
//! on FABRICATED structured memory with stub callees (tests/c/test_rust_weapons3.c). The field and
//! purpose names in the comments are INFERRED from the disassembly shape and docs/t-player-inventory.md.
//! Not admitted: absent from every selector.
//!
//! Shape: one arm per lifted basic block, keyed by its original address (`pc`); the flag model of the
//! original (`cmp!`, `result!`, `source!`) is kept because many conditional jumps read flags set
//! several blocks earlier. Registers are plain locals, locals of the original frame live in guest
//! memory at the same `esp`-relative offsets, so callee-visible stack words and register inputs of
//! every call equal the original's. Calls go through `GuestCall` (retained lifted originals).
//! XMM5-7 are scratch locals: `ExtRegisters` carries XMM0-4 only (an ABI gap, see
//! docs/t-weapons-port.md). The indirect handlers at definition+0x294 are called through
//! `GuestCall` as well, without the lifted `RECOMP_ICALL_IS_CODE` range check.
#![allow(unexpected_cfgs)]
use super::sse;
use crate::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
};

pub(super) fn fl(bits: u32) -> f32 { f32::from_bits(bits) }
pub(super) fn st(f: &Fpu, i: u32) -> f64 { f.st(i) }
pub(super) fn set_st(f: &mut Fpu, i: u32, v: f64) { f.stack[((f.top + i) & 7) as usize] = v; }


/// x87 `RECOMP_FP_PC`: narrow the significand to 24 bits unless precision control selects more,
/// keeping the exponent range of the register (recomp_fp_round24 in the lifted model).
pub(super) fn fpc(f: &Fpu, v: f64) -> f64 { if f.control & 0x300 != 0 { v } else { round24(v) } }
fn abs64(v: f64) -> f64 { f64::from_bits(v.to_bits() & 0x7fff_ffff_ffff_ffff) }
fn pow2(exponent: i32) -> f64 { f64::from_bits(((exponent + 1023) as u64) << 52) }
fn round24(x: f64) -> f64 {
    let ax = abs64(x);
    if (ax <= 3.4028234663852886e38 && ax >= 1.1754943508222875e-38) || ax == 0.0 || x != x || ax == f64::INFINITY {
        return x as f32 as f64;
    }
    // frexp: scale subnormals into the normal range first (exact), then pull out the exponent
    let (mut scaled, mut e) = (x, 0i32);
    if ax < 2.2250738585072014e-308 { scaled *= pow2(54); e -= 54; }
    let bits = scaled.to_bits();
    e += ((bits >> 52) & 0x7ff) as i32 - 1022;
    let mut mantissa = f64::from_bits((bits & 0x800f_ffff_ffff_ffff) | (1022u64 << 52)) as f32 as f64;
    while e > 1000 { mantissa *= pow2(1000); e -= 1000; }
    while e < -1000 { mantissa *= pow2(-1000); e += 1000; }
    mantissa * pow2(e)
}
pub(super) fn fcmp(a: f64, b: f64) -> i32 { if a != a || b != b { 2 } else if a < b { -1 } else if a > b { 1 } else { 0 } }
pub(super) fn parity8(v: u32) -> bool { (v & 0xff).count_ones() % 2 == 0 }
pub(super) mod xsqrt {
    #[cfg(target_arch = "x86_64")]
    pub fn ss(a: f32) -> f32 {
        let mut r = a;
        unsafe { core::arch::asm!("sqrtss {r}, {r}", r = inout(xmm_reg) r, options(pure, nomem, nostack)) };
        r
    }
    #[cfg(target_arch = "x86_64")]
    pub fn sd(a: f64) -> f64 {
        let mut r = a;
        unsafe { core::arch::asm!("sqrtsd {r}, {r}", r = inout(xmm_reg) r, options(pure, nomem, nostack)) };
        r
    }
    #[cfg(not(target_arch = "x86_64"))]
    pub fn ss(a: f32) -> f32 { sd(a as f64) as f32 }
    #[cfg(not(target_arch = "x86_64"))]
    pub fn sd(a: f64) -> f64 {
        if !(a > 0.0) || a == f64::INFINITY { return if a == 0.0 { a } else { f64::from_bits(0xfff8_0000_0000_0000) }; }
        let mut g = a; for _ in 0..80 { g = 0.5 * (g + a / g); } g
    }
}

/// Edge coverage for the differential (build with `--cfg tsfp_cover`; absent from normal builds).
#[cfg(tsfp_cover)]
#[unsafe(no_mangle)]
pub static mut tsfp_shot_edges: [u32; 4096] = [0; 4096];
#[cfg(tsfp_cover)]
#[unsafe(no_mangle)]
pub static mut tsfp_shot_edge_count: u32 = 0;
#[cfg(tsfp_cover)]
fn cover(from: u32, to: u32) {
    let edge = ((from & 0xffff) << 16) | (to & 0xffff);
    unsafe {
        let count = core::ptr::addr_of_mut!(tsfp_shot_edge_count);
        let list = core::ptr::addr_of_mut!(tsfp_shot_edges) as *mut u32;
        for i in 0..*count { if *list.add(i as usize) == edge { return; } }
        if (*count as usize) < 4096 { *list.add(*count as usize) = edge; *count += 1; }
    }
}

pub fn shot_start<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = [Xmm::default(); 8];
    x[..5].copy_from_slice(&entry.xmm);
    x[5..].copy_from_slice(&entry.xmm_hi);
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, false);
    let (mut fca, mut fcb, mut fpcmp) = (0f64, 0f64, 0i32);
    let mut pc: u32 = 0x1c0200;
    // Flag model of the lifted original: the operands of the last CMP/TEST (`cmp!`/`cmp8!`) or the
    // result/source of the last arithmetic op (`result!`, `source!`); conditions read them later.
    macro_rules! cmp { ($a:expr, $b:expr) => {{ fa = $a; fb = $b; fas = fa as i32; fbs = fb as i32; }}; }
    macro_rules! cmp8 { ($a:expr, $b:expr) => {{ fa = $a & 0xff; fb = $b & 0xff; fas = fa as u8 as i8 as i32; fbs = fb as u8 as i8 as i32; }}; }
    macro_rules! result { ($v:expr) => {{ fa = $v; fas = fa as i32; }}; }
    macro_rules! source { ($v:expr) => {{ fb = $v; fbs = fb as i32; }}; }
    // `mov eax, 1; shl eax, cl`: a zero count leaves the flags of the preceding CMP (ZF/SF) intact.
    macro_rules! one_shl_cl { () => {{
        eax = 1;
        let count = ecx & 31;
        if count == 0 { fa = if fa == fb { 0 } else if (fa.wrapping_sub(fb) >> 31) != 0 { 0x8000_0000 } else { 1 }; fas = fa as i32; }
        eax <<= count;
        if count != 0 { result!(eax); }
    }}; }
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [x[0], x[1], x[2], x[3], x[4]], xmm_hi: [x[5], x[6], x[7]] };
        guest.call(target, &mut cr);
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        x[..5].copy_from_slice(&cr.xmm);
        x[5..].copy_from_slice(&cr.xmm_hi);
    }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        {
            cover(previous, pc);
            previous = pc;
        }
        match pc {
            0x1c0200 => {
                // PROLOGUE. Frame 0xC8, EBX = weapon record R, stack arg = shooter O. Entry E = definition(O.id)+0x68+R.mode*0x7C (base entry when its flags are negative), P = player, Y = P.+0x14 -> +0x7C. Locals: [+0xAC] definition, [+0x84] E.+4, [+0x58] shots = E.+8.
                source!(0xC8);
                esp = esp.wrapping_sub(0xC8);
                result!(esp);
                eax = m.read32(ebx.wrapping_add(0x5C));
                push!(ebp);
                { let product = (eax as i32 as i64) * (0x7C as u32 as i32 as i64);
                eax = product as u32;
                }
                push!(esi);
                esi = m.read32(0x4F9BAC);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0xD8));
                ecx = m.read32(edi.wrapping_add(0x30));
                edx = ecx;
                { let product = (edx as i32 as i64) * (0x29C as u32 as i32 as i64);
                edx = product as u32;
                }
                source!(esi);
                edx = edx.wrapping_add(esi);
                result!(edx);
                ebp = eax.wrapping_add(edx).wrapping_add(0x68);
                cmp!(m.read32(ebp), 0);
                m.write32(esp.wrapping_add(0xAC), edx);
                if (fa.wrapping_sub(fb) >> 31) == 0 { pc = 0x1c023d; continue; } // jns
                pc = 0x1c023a; continue;
            }
            0x1c023a => {
                ebp = edx.wrapping_add(0x68);
                pc = 0x1c023d; continue;
            }
            0x1c023d => {
                eax = m.read32(0x7B0C7C);
                edx = m.read32(eax.wrapping_add(0x14));
                edx = m.read32(edx.wrapping_add(0x7C));
                m.write32(esp.wrapping_add(0x44), edx);
                edx = m.read32(ebp.wrapping_add(4));
                m.write32(esp.wrapping_add(0x84), edx);
                edx = m.read32(ebp.wrapping_add(8));
                esi = 0;
                result!(esi);
                cmp!(ecx, 6);
                m.write32(esp.wrapping_add(0x58), edx);
                if fa != fb { pc = 0x1c027a; continue; } // jne
                pc = 0x1c0264; continue;
            }
            0x1c0264 => {
                // shooter id 6: shots = R.+0xC0 + 1 and the burst counter R.+0xC0 is cleared
                eax = m.read32(ebx.wrapping_add(0xC0));
                eax = eax.wrapping_add(1);
                result!(eax);
                fb = (fa == 0x8000_0000) as u32;
                m.write32(esp.wrapping_add(0x58), eax);
                m.write32(ebx.wrapping_add(0xC0), esi);
                eax = m.read32(0x7B0C7C);
                pc = 0x1c027a; continue;
            }
            0x1c027a => {
                // local [+0x24] = aim-assist style gate (P.+4 >= 0, global G set, G.byte[0x21C] & 4)
                ecx = m.read32(eax.wrapping_add(4));
                cmp!(ecx, esi);
                if fas < fbs { pc = 0x1c029c; continue; } // jl
                pc = 0x1c0281; continue;
            }
            0x1c0281 => {
                edx = m.read32(0x7B0CC0);
                cmp!(edx, esi);
                if fa == fb { pc = 0x1c029c; continue; } // je
                pc = 0x1c028b; continue;
            }
            0x1c028b => {
                cmp8!((m.read8(edx.wrapping_add(0x21C)) as u32), (4));
                m.write32(esp.wrapping_add(0x24), 1);
                if (fa & fb) != 0 { pc = 0x1c02a0; continue; } // jne
                pc = 0x1c029c; continue;
            }
            0x1c029c => {
                m.write32(esp.wrapping_add(0x24), esi);
                pc = 0x1c02a0; continue;
            }
            0x1c02a0 => {
                // record ready check: when R.id is valid and P allows it, 0x1B9A30(R) == 0 aborts the whole shot (epilogue)
                esi = m.read32(ebx.wrapping_add(0x30));
                cmp!(esi, 0xFFFFFFFF);
                if fa == fb { pc = 0x1c02d9; continue; } // je
                pc = 0x1c02a8; continue;
            }
            0x1c02a8 => {
                cmp!(ecx, 0);
                if fas < fbs { pc = 0x1c02d9; continue; } // jl
                pc = 0x1c02ac; continue;
            }
            0x1c02ac => {
                ecx = m.read32(eax.wrapping_add(0xE84));
                cmp!(ecx, 0);
                if fa == fb { pc = 0x1c02d9; continue; } // je
                pc = 0x1c02b6; continue;
            }
            0x1c02b6 => {
                ecx = m.read32(0x4F9BAC);
                eax = esi;
                { let product = (eax as i32 as i64) * (0x29C as u32 as i32 as i64);
                eax = product as u32;
                }
                cmp!(m.read32(eax.wrapping_add(ecx)), 0xFFFFFFFF);
                if fa == fb { pc = 0x1c02d9; continue; } // je
                pc = 0x1c02ca; continue;
            }
            0x1c02ca => {
                edx = ebx;
                call!(0x1b9a30);
                pc = 0x1c02d1; continue;
            }
            0x1c02d1 => {
                cmp!(eax, 0);
                if fa == fb { pc = 0x1c0f8f; continue; } // je
                pc = 0x1c02d9; continue;
            }
            0x1c02d9 => {
                // RESET record state: R.+0x68 = 1, R.+0x6C/+0x74/+0x78/+0x7C = 0.0, R.+0x80 = 0.0 (or jittered by rand() for shooter id 0xA)
                cmp!(esi, 0xFFFFFFFF);
                x[0] = Xmm::default();
                m.write32(ebx.wrapping_add(0x68), 1);
                m.write32(ebx.wrapping_add(0x74), x[0].lanes[0]);
                m.write32(ebx.wrapping_add(0x78), x[0].lanes[0]);
                m.write32(ebx.wrapping_add(0x7C), x[0].lanes[0]);
                m.write32(ebx.wrapping_add(0x6C), x[0].lanes[0]);
                if fa == fb { pc = 0x1c032e; continue; } // je
                pc = 0x1c02fc; continue;
            }
            0x1c02fc => {
                eax = m.read32(0x7B0C7C);
                ecx = m.read32(eax.wrapping_add(4));
                cmp!(ecx, 0);
                if fas < fbs { pc = 0x1c032e; continue; } // jl
                pc = 0x1c0308; continue;
            }
            0x1c0308 => {
                ecx = m.read32(eax.wrapping_add(0xE84));
                cmp!(ecx, 0);
                if fa == fb { pc = 0x1c032e; continue; } // je
                pc = 0x1c0312; continue;
            }
            0x1c0312 => {
                edx = m.read32(0x4F9BAC);
                { let product = (esi as i32 as i64) * (0x29C as u32 as i32 as i64);
                esi = product as u32;
                }
                cmp!(m.read32(esi.wrapping_add(edx)), 0xFFFFFFFF);
                if fa == fb { pc = 0x1c032e; continue; } // je
                pc = 0x1c0324; continue;
            }
            0x1c0324 => {
                m.write32(ebx.wrapping_add(0x80), x[0].lanes[0]);
                pc = 0x1c0371; continue;
            }
            0x1c032e => {
                m.write32(ebx.wrapping_add(0x80), x[0].lanes[0]);
                cmp!(m.read32(edi.wrapping_add(0x30)), 0xA);
                if fa != fb { pc = 0x1c0371; continue; } // jne
                pc = 0x1c033c; continue;
            }
            0x1c033c => {
                call!(0x153b40);
                pc = 0x1c0341; continue;
            }
            0x1c0341 => {
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x5C), eax);
                fpu.push(m.read32(esp.wrapping_add(0x5C)) as i32 as f64);
                if fas >= fbs { pc = 0x1c0353; continue; } // jge
                pc = 0x1c034d; continue;
            }
            0x1c034d => {
                { let v = st(&fpu, 0) + fl(m.read32(0x475CCC)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                pc = 0x1c0353; continue;
            }
            0x1c0353 => {
                { let v = st(&fpu, 0) * fl(m.read32(0x4785D0)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 0) * fl(m.read32(0x4785E4)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 0) + fl(m.read32(0x475CD0)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 0) * fl(m.read32(ebx.wrapping_add(0x80))) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                m.write32(ebx.wrapping_add(0x80), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                pc = 0x1c0371; continue;
            }
            0x1c0371 => {
                // R.+0xA0 = 1.0, then a random roll against R.+0x9C: on success the accuracy block R.+0x84..+0x98 is cleared and R.+0x9C = 1.0
                x[0] = Xmm::scalar(m.read32(0x475C78));
                m.write32(ebx.wrapping_add(0xA0), x[0].lanes[0]);
                call!(0x153b40);
                pc = 0x1c0386; continue;
            }
            0x1c0386 => {
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x5C), eax);
                fpu.push(m.read32(esp.wrapping_add(0x5C)) as i32 as f64);
                if fas >= fbs { pc = 0x1c0398; continue; } // jge
                pc = 0x1c0392; continue;
            }
            0x1c0392 => {
                { let v = st(&fpu, 0) + fl(m.read32(0x475CCC)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                pc = 0x1c0398; continue;
            }
            0x1c0398 => {
                { let v = st(&fpu, 0) * fl(m.read32(0x4785D0)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 0) * fl(m.read32(0x4765E0)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                fpu.push(fl(m.read32(ebx.wrapping_add(0x9C))) as f64);
                { let t = st(&fpu, 0); let u = st(&fpu, 1); set_st(&mut fpu, 0, u); set_st(&mut fpu, 1, t); } // fxch
                fpcmp = fcmp(st(&fpu, 0), st(&fpu, 1));
                fpu.pop();
                fpu.pop();
                if fpcmp != 1 { pc = 0x1c03f5; continue; } // fcompi */) goto loc_001C03F5; /* jbe
                pc = 0x1c03b2; continue;
            }
            0x1c03b2 => {
                x[0] = Xmm::default();
                m.write32(ebx.wrapping_add(0x84), x[0].lanes[0]);
                m.write32(ebx.wrapping_add(0x88), x[0].lanes[0]);
                m.write32(ebx.wrapping_add(0x8C), x[0].lanes[0]);
                m.write32(ebx.wrapping_add(0x90), x[0].lanes[0]);
                m.write32(ebx.wrapping_add(0x94), x[0].lanes[0]);
                m.write32(ebx.wrapping_add(0x98), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(0x475C78));
                m.write32(ebx.wrapping_add(0x9C), x[0].lanes[0]);
                pc = 0x1c03f5; continue;
            }
            0x1c03f5 => {
                // RECOIL TABLE: R.+0xC4 = table[E.+0x64] at 0x4ED3C0 (8-byte stride), then 0x1B7260(O, R.mode) and 0x1BB2F0(P, R, mode) when its local result is non-zero
                eax = m.read32(ebp.wrapping_add(0x64));
                edx = m.read32(ebx.wrapping_add(0x5C));
                ecx = m.read32(eax.wrapping_mul(8).wrapping_add(0x4ED3C0));
                push!(edx);
                push!(edi);
                m.write32(ebx.wrapping_add(0xC4), ecx);
                call!(0x1b7260);
                pc = 0x1c040f; continue;
            }
            0x1c040f => {
                eax = m.read32(esp.wrapping_add(0x2C));
                source!(8);
                esp = esp.wrapping_add(8);
                result!(esp);
                cmp!(eax, 0);
                if fa == fb { pc = 0x1c042e; continue; } // je
                pc = 0x1c041a; continue;
            }
            0x1c041a => {
                eax = m.read32(ebx.wrapping_add(0x5C));
                ecx = m.read32(0x7B0C7C);
                push!(eax);
                push!(ebx);
                push!(ecx);
                call!(0x1bb2f0);
                pc = 0x1c042b; continue;
            }
            0x1c042b => {
                source!(0xC);
                esp = esp.wrapping_add(0xC);
                result!(esp);
                pc = 0x1c042e; continue;
            }
            0x1c042e => {
                // weapon id 0x1B: 0x1BE7D0(Y), R.+0xAC = 1 and return early
                cmp!(m.read32(ebx.wrapping_add(0x30)), 0x1B);
                if fa != fb { pc = 0x1c0455; continue; } // jne
                pc = 0x1c0434; continue;
            }
            0x1c0434 => {
                edx = m.read32(esp.wrapping_add(0x44));
                push!(edx);
                call!(0x1be7d0);
                pc = 0x1c043e; continue;
            }
            0x1c043e => {
                source!(4);
                esp = esp.wrapping_add(4);
                result!(esp);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(ebx.wrapping_add(0xAC), 1);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                source!(0xC8);
                esp = esp.wrapping_add(0xC8);
                result!(esp);
                break 'run;
            }
            0x1c0455 => {
                // start effects: when E.+4 != -1 and 0xDC270(E.+4) has bit 0x80000 clear, trigger 3 audio/effect calls (0x2384D0, 0x238510, 0x2385B0)
                eax = m.read32(esp.wrapping_add(0x84));
                cmp!(eax, 0xFFFFFFFF);
                if fa == fb { pc = 0x1c04bf; continue; } // je
                pc = 0x1c0461; continue;
            }
            0x1c0461 => {
                push!(eax);
                call!(0xdc270);
                pc = 0x1c0467; continue;
            }
            0x1c0467 => {
                source!(4);
                esp = esp.wrapping_add(4);
                result!(esp);
                cmp!(eax, 0x80000);
                if (fa & fb) != 0 { pc = 0x1c04bf; continue; } // jne
                pc = 0x1c0471; continue;
            }
            0x1c0471 => {
                eax = m.read32(0x7B0C7C);
                ecx = m.read32(eax);
                push!(0x3F800000);
                push!(ecx);
                push!(3);
                call!(0x2384d0);
                pc = 0x1c0485; continue;
            }
            0x1c0485 => {
                edx = m.read32(0x7B0C7C);
                eax = m.read32(edx);
                push!(0x3F800000);
                push!(eax);
                push!(8);
                call!(0x238510);
                pc = 0x1c049a; continue;
            }
            0x1c049a => {
                esi = m.read32(esp.wrapping_add(0x5C));
                ecx = m.read32(esi.wrapping_add(0x98));
                edx = m.read32(0x7B0C7C);
                eax = m.read32(edx);
                push!(0x3F800000);
                push!(ecx);
                push!(eax);
                push!(0);
                call!(0x2385b0);
                pc = 0x1c04ba; continue;
            }
            0x1c04ba => {
                source!(0x28);
                esp = esp.wrapping_add(0x28);
                result!(esp);
                pc = 0x1c04c3; continue;
            }
            0x1c04bf => {
                esi = m.read32(esp.wrapping_add(0x44));
                pc = 0x1c04c3; continue;
            }
            0x1c04c3 => {
                // SHOT LOOP head: [+0x48] = shot index 0, loop while shots > 0 (the loop runs at 0x1C04E0, counter at 0x1C0E62)
                eax = m.read32(esp.wrapping_add(0x58));
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x48), 0);
                if fas <= fbs { pc = 0x1c0e77; continue; } // jle
                pc = 0x1c04d7; continue;
            }
            0x1c04d7 => {
                eax = m.read32(0x7B0C7C);
                pc = 0x1c04e0; continue;
            }
            0x1c04e0 => {
                // per shot: muzzle position/direction vectors into locals [+0x28..+0x40] from O (or R / P by E.flags 0x20)
                x[0] = Xmm::scalar(m.read32(edi));
                ecx = m.read32(esp.wrapping_add(0x24));
                cmp!(ecx, 0);
                m.write32(esp.wrapping_add(0x60), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(edi.wrapping_add(4)));
                m.write32(esp.wrapping_add(0x64), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(edi.wrapping_add(8)));
                m.write32(esp.wrapping_add(0x68), x[0].lanes[0]);
                if fa == fb { pc = 0x1c0570; continue; } // je
                pc = 0x1c0508; continue;
            }
            0x1c0508 => {
                x[0] = Xmm::scalar(m.read32(ebx));
                cmp8!((m.read8(ebp) as u32), (0x20));
                m.write32(esp.wrapping_add(0x28), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(ebx.wrapping_add(4)));
                m.write32(esp.wrapping_add(0x2C), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(ebx.wrapping_add(8)));
                m.write32(esp.wrapping_add(0x30), x[0].lanes[0]);
                if (fa & fb) == 0 { pc = 0x1c0554; continue; } // je
                pc = 0x1c052e; continue;
            }
            0x1c052e => {
                x[0] = Xmm::scalar(m.read32(eax.wrapping_add(0xB4)));
                m.write32(esp.wrapping_add(0x38), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(eax.wrapping_add(0xB8)));
                m.write32(esp.wrapping_add(0x3C), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(eax.wrapping_add(0xBC)));
                pc = 0x1c05aa; continue;
            }
            0x1c0554 => {
                x[0] = Xmm::scalar(m.read32(ebx));
                m.write32(esp.wrapping_add(0x38), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(ebx.wrapping_add(4)));
                m.write32(esp.wrapping_add(0x3C), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(ebx.wrapping_add(8)));
                pc = 0x1c05aa; continue;
            }
            0x1c0570 => {
                x[0] = Xmm::scalar(m.read32(edi));
                m.write32(esp.wrapping_add(0x28), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(edi.wrapping_add(4)));
                m.write32(esp.wrapping_add(0x2C), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(edi.wrapping_add(8)));
                m.write32(esp.wrapping_add(0x30), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(edi));
                m.write32(esp.wrapping_add(0x38), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(edi.wrapping_add(4)));
                m.write32(esp.wrapping_add(0x3C), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(edi.wrapping_add(8)));
                pc = 0x1c05aa; continue;
            }
            0x1c05aa => {
                // E.flags & 0x1000: three rand() jitters (rand * 2^-32 - 0.5) * scale added to the origin
                eax = m.read32(ebp);
                cmp8!(((eax >> 8) & 0xff), (0x10));
                m.write32(esp.wrapping_add(0x40), x[0].lanes[0]);
                if (fa & fb) == 0 { pc = 0x1c064f; continue; } // je
                pc = 0x1c05bc; continue;
            }
            0x1c05bc => {
                call!(0x153b40);
                pc = 0x1c05c1; continue;
            }
            0x1c05c1 => {
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x34), eax);
                fpu.push(m.read32(esp.wrapping_add(0x34)) as i32 as f64);
                if fas >= fbs { pc = 0x1c05d3; continue; } // jge
                pc = 0x1c05cd; continue;
            }
            0x1c05cd => {
                { let v = st(&fpu, 0) + fl(m.read32(0x475CCC)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                pc = 0x1c05d3; continue;
            }
            0x1c05d3 => {
                { let v = st(&fpu, 0) * fl(m.read32(0x4785D0)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 0) - fl(m.read32(0x475CD4)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 0) * fl(m.read32(0x475D24)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 0) + fl(m.read32(esp.wrapping_add(0x28))) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                m.write32(esp.wrapping_add(0x28), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                call!(0x153b40);
                pc = 0x1c05f2; continue;
            }
            0x1c05f2 => {
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x34), eax);
                fpu.push(m.read32(esp.wrapping_add(0x34)) as i32 as f64);
                if fas >= fbs { pc = 0x1c0604; continue; } // jge
                pc = 0x1c05fe; continue;
            }
            0x1c05fe => {
                { let v = st(&fpu, 0) + fl(m.read32(0x475CCC)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                pc = 0x1c0604; continue;
            }
            0x1c0604 => {
                { let v = st(&fpu, 0) * fl(m.read32(0x4785D0)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 0) - fl(m.read32(0x475CD4)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 0) * fl(m.read32(0x475D24)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 0) + fl(m.read32(esp.wrapping_add(0x2C))) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                m.write32(esp.wrapping_add(0x2C), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                call!(0x153b40);
                pc = 0x1c0623; continue;
            }
            0x1c0623 => {
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x34), eax);
                fpu.push(m.read32(esp.wrapping_add(0x34)) as i32 as f64);
                if fas >= fbs { pc = 0x1c0635; continue; } // jge
                pc = 0x1c062f; continue;
            }
            0x1c062f => {
                { let v = st(&fpu, 0) + fl(m.read32(0x475CCC)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                pc = 0x1c0635; continue;
            }
            0x1c0635 => {
                { let v = st(&fpu, 0) * fl(m.read32(0x4785D0)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 0) - fl(m.read32(0x475CD4)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 0) * fl(m.read32(0x475D24)) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 0) + fl(m.read32(esp.wrapping_add(0x30))) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                m.write32(esp.wrapping_add(0x30), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                pc = 0x1c064f; continue;
            }
            0x1c064f => {
                // spread: 0xE12D0(O.+0x114, 0.7f, 0.3f, out[+0x88]) then scale by E.+0x20 and the flag modifiers (P.flags 0x20, E.flags 2/0x800, weapon id 0x22, type 2 with the 0x40 definition flag and O.+0x108)
                ecx = esp.wrapping_add(0x88);
                push!(ecx);
                push!(0x3E99999A);
                eax = edi.wrapping_add(0x114);
                push!(0x3F333333);
                push!(eax);
                call!(0xe12d0);
                pc = 0x1c066d; continue;
            }
            0x1c066d => {
                eax = m.read32(0x7B0C7C);
                ecx = m.read32(eax.wrapping_add(0xC));
                x[0] = Xmm::scalar(m.read32(ebp.wrapping_add(0x20)));
                x[2] = Xmm::scalar(m.read32(ebp.wrapping_add(0x20)));
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x98)))).to_bits();
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(m.read32(esp.wrapping_add(0x9C)))).to_bits();
                source!(0x10);
                esp = esp.wrapping_add(0x10);
                result!(esp);
                ecx = (ecx & 0x20);
                result!(ecx);
                if fa != 0 { pc = 0x1c06a5; continue; } // jne
                pc = 0x1c0699; continue;
            }
            0x1c0699 => {
                edx = m.read32(eax.wrapping_add(0x1500));
                cmp!(m.read32(edx.wrapping_add(0x74)), 2);
                if fa != fb { pc = 0x1c06b5; continue; } // jne
                pc = 0x1c06a5; continue;
            }
            0x1c06a5 => {
                cmp8!((m.read8(ebp) as u32), (2));
                if (fa & fb) != 0 { pc = 0x1c06b5; continue; } // jne
                pc = 0x1c06ab; continue;
            }
            0x1c06ab => {
                x[1] = Xmm::scalar(m.read32(0x475D24));
                pc = 0x1c06c1; continue;
            }
            0x1c06b5 => {
                cmp!(ecx, 0);
                if fa == fb { pc = 0x1c06c9; continue; } // je
                pc = 0x1c06b9; continue;
            }
            0x1c06b9 => {
                x[1] = Xmm::scalar(m.read32(0x475CD4));
                pc = 0x1c06c1; continue;
            }
            0x1c06c1 => {
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(x[1].lanes[0])).to_bits();
                pc = 0x1c06c9; continue;
            }
            0x1c06c9 => {
                ecx = m.read32(ebp);
                cmp8!(((ecx >> 8) & 0xff), (8));
                if (fa & fb) == 0 { pc = 0x1c06df; continue; } // je
                pc = 0x1c06d1; continue;
            }
            0x1c06d1 => {
                ecx = m.read32(esp.wrapping_add(0x48));
                cmp!(ecx, 0);
                if fa != fb { pc = 0x1c06df; continue; } // jne
                pc = 0x1c06d9; continue;
            }
            0x1c06d9 => {
                x[0] = Xmm::default();
                x[2] = x[0];
                pc = 0x1c06df; continue;
            }
            0x1c06df => {
                ecx = m.read32(ebx.wrapping_add(0x30));
                cmp!(ecx, 0x22);
                if fa != fb { pc = 0x1c06ef; continue; } // jne
                pc = 0x1c06e7; continue;
            }
            0x1c06e7 => {
                x[2].lanes[0] = sse::addss(fl(x[2].lanes[0]), fl(m.read32(0x475CF8))).to_bits();
                pc = 0x1c06ef; continue;
            }
            0x1c06ef => {
                cmp!(m.read32(ebx.wrapping_add(0x3C)), 2);
                if fa != fb { pc = 0x1c0724; continue; } // jne
                pc = 0x1c06f5; continue;
            }
            0x1c06f5 => {
                cmp!(ecx, 0xFFFFFFFF);
                if fa == fb { pc = 0x1c0724; continue; } // je
                pc = 0x1c06fa; continue;
            }
            0x1c06fa => {
                edx = m.read32(0x4F9BAC);
                { let product = (ecx as i32 as i64) * (0x29C as u32 as i32 as i64);
                ecx = product as u32;
                }
                cmp8!((m.read8(ecx.wrapping_add(edx).wrapping_add(0xD)) as u32), (0x40));
                if (fa & fb) == 0 { pc = 0x1c0724; continue; } // je
                pc = 0x1c070d; continue;
            }
            0x1c070d => {
                x[1] = Xmm::scalar(m.read32(edi.wrapping_add(0x108)));
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(m.read32(0x4765E0))).to_bits();
                x[1].lanes[0] = sse::addss(fl(x[1].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[2] = x[1];
                pc = 0x1c0724; continue;
            }
            0x1c0724 => {
                // aim ray: direction from the player eye (P.+0xDC..+0xF0, +0xF88..+0xF90) minus the origin, normalised with the lifted rsqrt model (1 / sqrt, threshold 0x4765F4)
                x[5] = Xmm::scalar(m.read32(eax.wrapping_add(0xE8)));
                x[6] = Xmm::scalar(m.read32(eax.wrapping_add(0xEC)));
                x[7] = Xmm::scalar(m.read32(eax.wrapping_add(0xF0)));
                ecx = m.read32(eax.wrapping_add(0xF84));
                cmp!(ecx, 0);
                x[1] = Xmm::scalar(m.read32(eax.wrapping_add(0xDC)));
                x[4] = Xmm::scalar(m.read32(eax.wrapping_add(0xE0)));
                x[5].lanes[0] = sse::mulss(fl(x[5].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[6].lanes[0] = sse::mulss(fl(x[6].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[7].lanes[0] = sse::mulss(fl(x[7].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[0] = Xmm::scalar(m.read32(eax.wrapping_add(0xE4)));
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[4].lanes[0] = sse::mulss(fl(x[4].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x78), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x7C), x[4].lanes[0]);
                m.write32(esp.wrapping_add(0x80), x[0].lanes[0]);
                if fa == fb { pc = 0x1c0a43; continue; } // je
                pc = 0x1c078f; continue;
            }
            0x1c078f => {
                ecx = m.read32(esp.wrapping_add(0x24));
                cmp!(ecx, 0);
                if fa == fb { pc = 0x1c0a43; continue; } // je
                pc = 0x1c079b; continue;
            }
            0x1c079b => {
                x[1] = Xmm::scalar(m.read32(eax.wrapping_add(0xF88)));
                x[1].lanes[0] = sse::subss(fl(x[1].lanes[0]), fl(m.read32(esp.wrapping_add(0x28)))).to_bits();
                m.write32(esp.wrapping_add(0x18), x[1].lanes[0]);
                x[3] = Xmm::scalar(m.read32(eax.wrapping_add(0xF8C)));
                x[3].lanes[0] = sse::subss(fl(x[3].lanes[0]), fl(m.read32(esp.wrapping_add(0x2C)))).to_bits();
                m.write32(esp.wrapping_add(0x1C), x[3].lanes[0]);
                x[4] = Xmm::scalar(m.read32(eax.wrapping_add(0xF90)));
                x[4].lanes[0] = sse::subss(fl(x[4].lanes[0]), fl(m.read32(esp.wrapping_add(0x30)))).to_bits();
                m.write32(esp.wrapping_add(0x20), x[4].lanes[0]);
                fpu.push(fl(m.read32(esp.wrapping_add(0x20))) as f64);
                { let v = st(&fpu, 0) * fl(m.read32(esp.wrapping_add(0x20))) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                fpu.push(fl(m.read32(esp.wrapping_add(0x1C))) as f64);
                { let v = st(&fpu, 0) * fl(m.read32(esp.wrapping_add(0x1C))) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 1) + st(&fpu, 0); let v = fpc(&fpu, v); set_st(&mut fpu, 1, v); }
                fpu.pop();
                fpu.push(fl(m.read32(esp.wrapping_add(0x18))) as f64);
                { let v = st(&fpu, 0) * fl(m.read32(esp.wrapping_add(0x18))) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 1) + st(&fpu, 0); let v = fpc(&fpu, v); set_st(&mut fpu, 1, v); }
                fpu.pop();
                { let v = xsqrt::sd(st(&fpu, 0)); let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = fl(m.read32(0x475C78)) as f64 / st(&fpu, 0); let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                m.write32(esp.wrapping_add(0x34), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                x[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                x[0] = x[2];
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[1] = x[2];
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(x[4].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[5].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x78)))).to_bits();
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[3].lanes[0])).to_bits();
                x[2].lanes[0] = sse::addss(fl(x[2].lanes[0]), fl(x[7].lanes[0])).to_bits();
                x[3] = x[2];
                x[3].lanes[0] = sse::addss(fl(x[3].lanes[0]), fl(m.read32(esp.wrapping_add(0x80)))).to_bits();
                x[2] = x[0];
                x[1].lanes[0] = sse::addss(fl(x[1].lanes[0]), fl(x[6].lanes[0])).to_bits();
                x[1].lanes[0] = sse::addss(fl(x[1].lanes[0]), fl(m.read32(esp.wrapping_add(0x7C)))).to_bits();
                x[0] = x[3];
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[3].lanes[0])).to_bits();
                x[4] = x[1];
                x[4].lanes[0] = sse::mulss(fl(x[4].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[4].lanes[0])).to_bits();
                x[4] = x[2];
                x[4].lanes[0] = sse::mulss(fl(x[4].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[4].lanes[0])).to_bits();
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(m.read32(0x4765F4)) as f64;
                m.write32(esp.wrapping_add(0x18), x[2].lanes[0]);
                m.write32(esp.wrapping_add(0x1C), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x20), x[3].lanes[0]);
                if !(fca > fcb) { pc = 0x1c08e3; continue; } // jbe
                pc = 0x1c0876; continue;
            }
            0x1c0876 => {
                m.write32(esp.wrapping_add(0xA4), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(0x475C78));
                m.write32(esp.wrapping_add(0xC0), x[0].lanes[0]);
                x[0].lanes[0] = sse::divss(1.0, xsqrt::ss(fl(m.read32(esp.wrapping_add(0xA4))))).to_bits();
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0xC0)))).to_bits();
                m.write32(esp.wrapping_add(0xB4), x[0].lanes[0]);
                x[3] = Xmm::scalar(m.read32(esp.wrapping_add(0xB4)));
                eax = m.read32(0x7B0C7C);
                x[2] = x[3];
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(m.read32(esp.wrapping_add(0x18)))).to_bits();
                x[1] = x[3];
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(m.read32(esp.wrapping_add(0x1C)))).to_bits();
                x[3].lanes[0] = sse::mulss(fl(x[3].lanes[0]), fl(m.read32(esp.wrapping_add(0x20)))).to_bits();
                m.write32(esp.wrapping_add(0x18), x[2].lanes[0]);
                m.write32(esp.wrapping_add(0x1C), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x20), x[3].lanes[0]);
                pc = 0x1c08e3; continue;
            }
            0x1c08e3 => {
                m.write32(esp.wrapping_add(0x50), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x4C), x[2].lanes[0]);
                m.write32(esp.wrapping_add(0x54), x[3].lanes[0]);
                x[1] = Xmm::scalar(m.read32(eax.wrapping_add(0xF88)));
                x[1].lanes[0] = sse::subss(fl(x[1].lanes[0]), fl(m.read32(esp.wrapping_add(0x38)))).to_bits();
                m.write32(esp.wrapping_add(0xC), x[1].lanes[0]);
                x[3] = Xmm::scalar(m.read32(eax.wrapping_add(0xF8C)));
                x[3].lanes[0] = sse::subss(fl(x[3].lanes[0]), fl(m.read32(esp.wrapping_add(0x3C)))).to_bits();
                m.write32(esp.wrapping_add(0x10), x[3].lanes[0]);
                x[4] = Xmm::scalar(m.read32(eax.wrapping_add(0xF90)));
                x[4].lanes[0] = sse::subss(fl(x[4].lanes[0]), fl(m.read32(esp.wrapping_add(0x40)))).to_bits();
                m.write32(esp.wrapping_add(0x14), x[4].lanes[0]);
                fpu.push(fl(m.read32(esp.wrapping_add(0x14))) as f64);
                { let v = st(&fpu, 0) * fl(m.read32(esp.wrapping_add(0x14))) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                fpu.push(fl(m.read32(esp.wrapping_add(0x10))) as f64);
                { let v = st(&fpu, 0) * fl(m.read32(esp.wrapping_add(0x10))) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 1) + st(&fpu, 0); let v = fpc(&fpu, v); set_st(&mut fpu, 1, v); }
                fpu.pop();
                fpu.push(fl(m.read32(esp.wrapping_add(0xC))) as f64);
                { let v = st(&fpu, 0) * fl(m.read32(esp.wrapping_add(0xC))) as f64; let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = st(&fpu, 1) + st(&fpu, 0); let v = fpc(&fpu, v); set_st(&mut fpu, 1, v); }
                fpu.pop();
                { let v = xsqrt::sd(st(&fpu, 0)); let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                { let v = fl(m.read32(0x475C78)) as f64 / st(&fpu, 0); let v = fpc(&fpu, v); set_st(&mut fpu, 0, v); }
                m.write32(esp.wrapping_add(0x34), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                x[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                x[0] = x[2];
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[1] = x[2];
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[5].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x78)))).to_bits();
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[3].lanes[0])).to_bits();
                x[3] = x[0];
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(x[4].lanes[0])).to_bits();
                x[2].lanes[0] = sse::addss(fl(x[2].lanes[0]), fl(x[7].lanes[0])).to_bits();
                x[2].lanes[0] = sse::addss(fl(x[2].lanes[0]), fl(m.read32(esp.wrapping_add(0x80)))).to_bits();
                x[1].lanes[0] = sse::addss(fl(x[1].lanes[0]), fl(x[6].lanes[0])).to_bits();
                x[1].lanes[0] = sse::addss(fl(x[1].lanes[0]), fl(m.read32(esp.wrapping_add(0x7C)))).to_bits();
                x[0] = x[2];
                x[2] = x[1];
                m.write32(esp.wrapping_add(0x14), x[0].lanes[0]);
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(x[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x10), x[1].lanes[0]);
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[1] = x[3];
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[3].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(m.read32(0x4765F4)) as f64;
                m.write32(esp.wrapping_add(0xC), x[3].lanes[0]);
                if !(fca > fcb) { pc = 0x1c0b3e; continue; } // jbe
                pc = 0x1c09d1; continue;
            }
            0x1c09d1 => {
                m.write32(esp.wrapping_add(0xA8), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(0x475C78));
                m.write32(esp.wrapping_add(0xBC), x[0].lanes[0]);
                x[0].lanes[0] = sse::divss(1.0, xsqrt::ss(fl(m.read32(esp.wrapping_add(0xA8))))).to_bits();
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0xBC)))).to_bits();
                m.write32(esp.wrapping_add(0xB8), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0xB8)));
                eax = m.read32(0x7B0C7C);
                x[1] = x[0];
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(m.read32(esp.wrapping_add(0xC)))).to_bits();
                m.write32(esp.wrapping_add(0xC), x[1].lanes[0]);
                x[1] = x[0];
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(m.read32(esp.wrapping_add(0x10)))).to_bits();
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x14)))).to_bits();
                m.write32(esp.wrapping_add(0x10), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x14), x[0].lanes[0]);
                pc = 0x1c0b3e; continue;
            }
            0x1c0a43 => {
                // no player aim: direction from Y.+0xB7C..+0xB84 plus the spread terms, same normalisation
                x[0] = Xmm::scalar(m.read32(esi.wrapping_add(0xB7C)));
                x[2] = Xmm::scalar(m.read32(esi.wrapping_add(0xB80)));
                x[3] = Xmm::scalar(m.read32(esi.wrapping_add(0xB84)));
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[5].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[1] = x[0];
                x[2].lanes[0] = sse::addss(fl(x[2].lanes[0]), fl(x[6].lanes[0])).to_bits();
                x[2].lanes[0] = sse::addss(fl(x[2].lanes[0]), fl(x[4].lanes[0])).to_bits();
                x[3].lanes[0] = sse::addss(fl(x[3].lanes[0]), fl(x[7].lanes[0])).to_bits();
                x[3].lanes[0] = sse::addss(fl(x[3].lanes[0]), fl(m.read32(esp.wrapping_add(0x80)))).to_bits();
                x[0] = x[3];
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[3].lanes[0])).to_bits();
                x[4] = x[2];
                x[4].lanes[0] = sse::mulss(fl(x[4].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[4].lanes[0])).to_bits();
                x[4] = x[1];
                x[4].lanes[0] = sse::mulss(fl(x[4].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[4].lanes[0])).to_bits();
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(m.read32(0x4765F4)) as f64;
                m.write32(esp.wrapping_add(0xC), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x10), x[2].lanes[0]);
                m.write32(esp.wrapping_add(0x14), x[3].lanes[0]);
                if !(fca > fcb) { pc = 0x1c0b1a; continue; } // jbe
                pc = 0x1c0ab3; continue;
            }
            0x1c0ab3 => {
                m.write32(esp.wrapping_add(0xA0), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(0x475C78));
                m.write32(esp.wrapping_add(0xB0), x[0].lanes[0]);
                x[0].lanes[0] = sse::divss(1.0, xsqrt::ss(fl(m.read32(esp.wrapping_add(0xA0))))).to_bits();
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0xB0)))).to_bits();
                m.write32(esp.wrapping_add(0x5C), x[0].lanes[0]);
                x[3] = Xmm::scalar(m.read32(esp.wrapping_add(0x5C)));
                eax = m.read32(0x7B0C7C);
                x[1] = x[3];
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(m.read32(esp.wrapping_add(0xC)))).to_bits();
                x[2] = x[3];
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(m.read32(esp.wrapping_add(0x10)))).to_bits();
                x[3].lanes[0] = sse::mulss(fl(x[3].lanes[0]), fl(m.read32(esp.wrapping_add(0x14)))).to_bits();
                m.write32(esp.wrapping_add(0xC), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x10), x[2].lanes[0]);
                m.write32(esp.wrapping_add(0x14), x[3].lanes[0]);
                pc = 0x1c0b1a; continue;
            }
            0x1c0b1a => {
                m.write32(esp.wrapping_add(0x18), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x1C), x[2].lanes[0]);
                m.write32(esp.wrapping_add(0x20), x[3].lanes[0]);
                m.write32(esp.wrapping_add(0x4C), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x50), x[2].lanes[0]);
                m.write32(esp.wrapping_add(0x54), x[3].lanes[0]);
                pc = 0x1c0b3e; continue;
            }
            0x1c0b3e => {
                // PROJECTILE SPAWN: P.+0xF60 = Q.+0x34 then 0x1B7680 with 13 stack arguments (definition, origin, direction, shot index, ...); 0x11F780(Y.+0xC68) returns a float compared with 1.0
                cmp!(m.read32(esp.wrapping_add(0x84)), 0xFFFFFFFF);
                if fa == fb { pc = 0x1c0e62; continue; } // je
                pc = 0x1c0b4c; continue;
            }
            0x1c0b4c => {
                ecx = m.read32(eax.wrapping_add(0x14));
                edx = m.read32(ecx.wrapping_add(0x34));
                m.write32(eax.wrapping_add(0xF60), edx);
                eax = m.read32(esp.wrapping_add(0x48));
                push!(eax);
                push!(esi);
                push!(1);
                ecx = esp.wrapping_add(0x58);
                push!(ecx);
                edx = esp.wrapping_add(0x28);
                push!(edx);
                eax = esp.wrapping_add(0x20);
                push!(eax);
                ecx = esp.wrapping_add(0x78);
                push!(ecx);
                ecx = m.read32(ebx.wrapping_add(0x5C));
                edx = esp.wrapping_add(0x44);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x44));
                cf = edx != 0;
                edx = (edx.wrapping_neg());
                result!(edx);
                eax = esp.wrapping_add(0x58);
                push!(eax);
                eax = m.read32(0x7B0C7C);
                push!(ecx);
                ecx = m.read32(eax.wrapping_add(0x14));
                edx = if cf { 0xffff_ffff } else { 0 };
                result!(edx);
                edx = (edx & ebx);
                result!(edx);
                push!(edx);
                push!(edi);
                push!(ecx);
                call!(0x1b7680);
                pc = 0x1c0b9c; continue;
            }
            0x1c0b9c => {
                edx = esi.wrapping_add(0xC68);
                push!(edx);
                call!(0x11f780);
                pc = 0x1c0ba8; continue;
            }
            0x1c0ba8 => {
                fpu.push(fl(m.read32(0x475C78)) as f64);
                source!(0x38);
                esp = esp.wrapping_add(0x38);
                result!(esp);
                fpcmp = fcmp(st(&fpu, 0), st(&fpu, 1));
                fpu.pop();
                fpu.pop();
                eax = (eax & 0xffff_00ff) | ((((if fpcmp == 0 || fpcmp == 2 { 0x40u32 } else { 0 }) | (if fpcmp == 2 { 4 } else { 0 }) | (if fpcmp < 0 || fpcmp == 2 { 1 } else { 0 }) | 2) as u32) << 8);
                cmp8!(((eax >> 8) & 0xff), (0x44));
                if !parity8(fa & fb) { pc = 0x1c0bc5; continue; } // jnp
                pc = 0x1c0bbb; continue;
            }
            0x1c0bbb => {
                cmp!(m.read32(edi.wrapping_add(0x30)), 0xA);
                if fa != fb { pc = 0x1c0e5d; continue; } // jne
                pc = 0x1c0bc5; continue;
            }
            0x1c0bc5 => {
                // effect handles: 0x151FC0 / 0x152090 (aim point), 0xDFEC0 effect starts from the E.+0x68/+0x6C ids and the E.+0x70 id list, 0x1CDAE0 count check
                eax = m.read32(esp.wrapping_add(0x24));
                cmp!(eax, 0);
                if fa == fb { pc = 0x1c0db6; continue; } // je
                pc = 0x1c0bd1; continue;
            }
            0x1c0bd1 => {
                eax = m.read32(ebx.wrapping_add(0x54));
                cmp!(eax, 0);
                if fa == fb { pc = 0x1c0db6; continue; } // je
                pc = 0x1c0bdc; continue;
            }
            0x1c0bdc => {
                x[0] = Xmm::default();
                edx = m.read32(0x7B0CC0);
                m.write32(esp.wrapping_add(0x6C), x[0].lanes[0]);
                m.write32(esp.wrapping_add(0x70), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(0x475C78));
                eax = esp.wrapping_add(0xC4);
                push!(eax);
                ecx = esp.wrapping_add(0x2C);
                m.write32(esp.wrapping_add(0x78), x[0].lanes[0]);
                eax = m.read32(edx.wrapping_add(0x208));
                push!(ecx);
                push!(eax);
                call!(0x151fc0);
                pc = 0x1c0c18; continue;
            }
            0x1c0c18 => {
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                edx = m.read32(0x7B0CC0);
                m.write32(esp.wrapping_add(0x78), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                m.write32(esp.wrapping_add(0x7C), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x2C)));
                ecx = esp.wrapping_add(0x78);
                m.write32(esp.wrapping_add(0x80), x[0].lanes[0]);
                eax = m.read32(edx.wrapping_add(0x208));
                push!(ecx);
                push!(eax);
                call!(0x152090);
                pc = 0x1c0c56; continue;
            }
            0x1c0c56 => {
                edx = m.read32(ebp.wrapping_add(0x68));
                source!(0x14);
                esp = esp.wrapping_add(0x14);
                result!(esp);
                cmp!(edx, 0xFFFFFFFF);
                if fa == fb { pc = 0x1c0cff; continue; } // je
                pc = 0x1c0c65; continue;
            }
            0x1c0c65 => {
                eax = m.read32(ebp);
                cmp8!(((eax >> 8) & 0xff), (0));
                if ((fas as u8).wrapping_sub(fbs as u8) >> 7) != 0 { pc = 0x1c0cff; continue; } // js
                pc = 0x1c0c70; continue;
            }
            0x1c0c70 => {
                ecx = m.read32(0x7B0CC0);
                cmp!(ecx, 0);
                if fa == fb { pc = 0x1c0c89; continue; } // je
                pc = 0x1c0c7a; continue;
            }
            0x1c0c7a => {
                ecx = m.read32(ecx.wrapping_add(0x214));
                one_shl_cl!();
                pc = 0x1c0c8b; continue;
            }
            0x1c0c89 => {
                eax = 0;
                result!(eax);
                pc = 0x1c0c8b; continue;
            }
            0x1c0c8b => {
                ecx = m.read32(0x7B0C7C);
                ecx = m.read32(ecx.wrapping_add(4));
                source!(0xCA);
                ecx = ecx.wrapping_add(0xCA);
                result!(ecx);
                push!(ecx);
                ecx = esp.wrapping_add(0x70);
                push!(ecx);
                ecx = esp.wrapping_add(0xCC);
                push!(ecx);
                push!(eax);
                eax = m.read32(ebp.wrapping_add(0x6C));
                push!(eax);
                push!(edx);
                push!(1);
                call!(0xdfec0);
                pc = 0x1c0cb5; continue;
            }
            0x1c0cb5 => {
                source!(0x1C);
                esp = esp.wrapping_add(0x1C);
                result!(esp);
                call!(0x1cdae0);
                pc = 0x1c0cbd; continue;
            }
            0x1c0cbd => {
                cmp!(eax, 1);
                if fas <= fbs { pc = 0x1c0cff; continue; } // jle
                pc = 0x1c0cc2; continue;
            }
            0x1c0cc2 => {
                ecx = m.read32(0x7B0CC0);
                cmp!(ecx, 0);
                if fa == fb { pc = 0x1c0cdd; continue; } // je
                pc = 0x1c0ccc; continue;
            }
            0x1c0ccc => {
                ecx = m.read32(ecx.wrapping_add(0x214));
                one_shl_cl!();
                eax = (!eax);
                pc = 0x1c0ce0; continue;
            }
            0x1c0cdd => {
                eax = (eax | 0xFFFFFFFF);
                result!(eax);
                pc = 0x1c0ce0; continue;
            }
            0x1c0ce0 => {
                push!(0xFFFFFFFF);
                ecx = esp.wrapping_add(0x50);
                push!(ecx);
                ecx = m.read32(ebp.wrapping_add(0x68));
                edx = esp.wrapping_add(0x68);
                push!(edx);
                push!(eax);
                eax = m.read32(ebp.wrapping_add(0x6C));
                push!(eax);
                push!(ecx);
                push!(0);
                call!(0xdfec0);
                pc = 0x1c0cfc; continue;
            }
            0x1c0cfc => {
                source!(0x1C);
                esp = esp.wrapping_add(0x1C);
                result!(esp);
                pc = 0x1c0cff; continue;
            }
            0x1c0cff => {
                eax = m.read32(ebp.wrapping_add(0x70));
                cmp!(eax, 0);
                if fa == fb { pc = 0x1c0dda; continue; } // je
                pc = 0x1c0d0a; continue;
            }
            0x1c0d0a => {
                cmp!(m.read32(eax), 0);
                if fas < fbs { pc = 0x1c0dda; continue; } // jl
                pc = 0x1c0d13; continue;
            }
            0x1c0d13 => {
                edi = 0;
                result!(edi);
                pc = 0x1c0d20; continue;
            }
            0x1c0d20 => {
                // E.+0x70 list: per id pick an effect object (H.+0x1FC array or the default 0x7AD288) and run 0x10E800/0x10E220/0x10E6C0/0x10E700
                ecx = m.read32(ebx.wrapping_add(0x54));
                eax = m.read32(edi.wrapping_add(eax));
                cmp!(eax, m.read32(ecx.wrapping_add(0x1E8)));
                if fas >= fbs { pc = 0x1c0d39; continue; } // jge
                pc = 0x1c0d2e; continue;
            }
            0x1c0d2e => {
                edx = m.read32(ecx.wrapping_add(0x1FC));
                esi = m.read32(edx.wrapping_add(eax.wrapping_mul(4)));
                pc = 0x1c0d3f; continue;
            }
            0x1c0d39 => {
                esi = m.read32(0x7AD288);
                pc = 0x1c0d3f; continue;
            }
            0x1c0d3f => {
                eax = m.read32(ebp);
                cmp8!(((eax >> 8) & 0xff), (2));
                if (fa & fb) != 0 { pc = 0x1c0d6c; continue; } // jne
                pc = 0x1c0d47; continue;
            }
            0x1c0d47 => {
                eax = m.read32(0x7B0C7C);
                ecx = m.read32(eax.wrapping_add(4));
                source!(0xCA);
                ecx = ecx.wrapping_add(0xCA);
                result!(ecx);
                push!(ecx);
                push!(esi);
                call!(0x10e800);
                pc = 0x1c0d5c; continue;
            }
            0x1c0d5c => {
                push!(0);
                push!(0x10000);
                push!(esi);
                call!(0x10e220);
                pc = 0x1c0d69; continue;
            }
            0x1c0d69 => {
                source!(0x14);
                esp = esp.wrapping_add(0x14);
                result!(esp);
                pc = 0x1c0d6c; continue;
            }
            0x1c0d6c => {
                ecx = m.read32(0x7B0CC0);
                cmp!(ecx, 0);
                if fa == fb { pc = 0x1c0d85; continue; } // je
                pc = 0x1c0d76; continue;
            }
            0x1c0d76 => {
                ecx = m.read32(ecx.wrapping_add(0x214));
                one_shl_cl!();
                pc = 0x1c0d87; continue;
            }
            0x1c0d85 => {
                eax = 0;
                result!(eax);
                pc = 0x1c0d87; continue;
            }
            0x1c0d87 => {
                push!(eax);
                push!(esi);
                call!(0x10e6c0);
                pc = 0x1c0d8e; continue;
            }
            0x1c0d8e => {
                push!(esi);
                call!(0x10e700);
                pc = 0x1c0d94; continue;
            }
            0x1c0d94 => {
                eax = m.read32(ebp.wrapping_add(0x70));
                ecx = m.read32(edi.wrapping_add(eax).wrapping_add(4));
                source!(4);
                edi = edi.wrapping_add(4);
                result!(edi);
                source!(0xC);
                esp = esp.wrapping_add(0xC);
                result!(esp);
                cmp!(ecx, 0);
                if fas >= fbs { pc = 0x1c0d20; continue; } // jge
                pc = 0x1c0da9; continue;
            }
            0x1c0da9 => {
                edi = m.read32(esp.wrapping_add(0xD8));
                esi = m.read32(esp.wrapping_add(0x44));
                pc = 0x1c0dda; continue;
            }
            0x1c0db6 => {
                eax = m.read32(ebp.wrapping_add(0x68));
                cmp!(eax, 0xFFFFFFFF);
                if fa == fb { pc = 0x1c0dda; continue; } // je
                pc = 0x1c0dbe; continue;
            }
            0x1c0dbe => {
                ecx = m.read32(ebp.wrapping_add(0x6C));
                push!(0xFFFFFFFF);
                edx = esp.wrapping_add(0x98);
                push!(edx);
                push!(edi);
                push!(0xFFFFFFFF);
                push!(ecx);
                push!(eax);
                push!(0);
                call!(0xdfec0);
                pc = 0x1c0dd7; continue;
            }
            0x1c0dd7 => {
                source!(0x1C);
                esp = esp.wrapping_add(0x1C);
                result!(esp);
                pc = 0x1c0dda; continue;
            }
            0x1c0dda => {
                // E.+0x74 list: same walk for the second list with the owner object J (O.+0xE8) and the negated mask
                eax = m.read32(ebp.wrapping_add(0x74));
                cmp!(eax, 0);
                if fa == fb { pc = 0x1c0e5d; continue; } // je
                pc = 0x1c0de1; continue;
            }
            0x1c0de1 => {
                eax = m.read32(eax);
                cmp!(eax, 0);
                if fas < fbs { pc = 0x1c0e5d; continue; } // jl
                pc = 0x1c0de7; continue;
            }
            0x1c0de7 => {
                edi = 0;
                result!(edi);
                pc = 0x1c0df0; continue;
            }
            0x1c0df0 => {
                edx = m.read32(esp.wrapping_add(0xD8));
                ecx = m.read32(edx.wrapping_add(0xE8));
                cmp!(eax, m.read32(ecx.wrapping_add(0x1E8)));
                if fas >= fbs { pc = 0x1c0e10; continue; } // jge
                pc = 0x1c0e05; continue;
            }
            0x1c0e05 => {
                ecx = m.read32(ecx.wrapping_add(0x1FC));
                esi = m.read32(ecx.wrapping_add(eax.wrapping_mul(4)));
                pc = 0x1c0e16; continue;
            }
            0x1c0e10 => {
                esi = m.read32(0x7AD288);
                pc = 0x1c0e16; continue;
            }
            0x1c0e16 => {
                ecx = m.read32(0x7B0CC0);
                cmp!(ecx, 0);
                if fa == fb { pc = 0x1c0e31; continue; } // je
                pc = 0x1c0e20; continue;
            }
            0x1c0e20 => {
                ecx = m.read32(ecx.wrapping_add(0x214));
                one_shl_cl!();
                eax = (!eax);
                pc = 0x1c0e34; continue;
            }
            0x1c0e31 => {
                eax = (eax | 0xFFFFFFFF);
                result!(eax);
                pc = 0x1c0e34; continue;
            }
            0x1c0e34 => {
                push!(eax);
                push!(esi);
                call!(0x10e6c0);
                pc = 0x1c0e3b; continue;
            }
            0x1c0e3b => {
                push!(esi);
                call!(0x10e700);
                pc = 0x1c0e41; continue;
            }
            0x1c0e41 => {
                edx = m.read32(ebp.wrapping_add(0x74));
                eax = m.read32(edi.wrapping_add(edx).wrapping_add(4));
                source!(4);
                edi = edi.wrapping_add(4);
                result!(edi);
                source!(0xC);
                esp = esp.wrapping_add(0xC);
                result!(esp);
                cmp!(eax, 0);
                if fas >= fbs { pc = 0x1c0df0; continue; } // jge
                pc = 0x1c0e52; continue;
            }
            0x1c0e52 => {
                edi = m.read32(esp.wrapping_add(0xD8));
                esi = m.read32(esp.wrapping_add(0x44));
                pc = 0x1c0e5d; continue;
            }
            0x1c0e5d => {
                eax = m.read32(0x7B0C7C);
                pc = 0x1c0e62; continue;
            }
            0x1c0e62 => {
                // shot index++ and loop back while index < shots
                ecx = m.read32(esp.wrapping_add(0x48));
                edx = m.read32(esp.wrapping_add(0x58));
                ecx = ecx.wrapping_add(1);
                result!(ecx);
                fb = (fa == 0x8000_0000) as u32;
                cmp!(ecx, edx);
                m.write32(esp.wrapping_add(0x48), ecx);
                if fas < fbs { pc = 0x1c04e0; continue; } // jl
                pc = 0x1c0e77; continue;
            }
            0x1c0e77 => {
                // REFIRE FRAMES: O.+0x110 = trunc(frame rate * definition.+0x298); when <= 0 and E.+0x54 != -1, 0x1C0110(R, O, frames, E) runs and the field is cleared
                x[0].lanes[0] = (m.read32(0x4E8CEC) as i32 as f32).to_bits();
                ecx = m.read32(esp.wrapping_add(0xAC));
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(m.read32(ecx.wrapping_add(0x298)))).to_bits();
                eax = sse::cvttss2si(fl(x[0].lanes[0]));
                cmp!(eax, 0);
                m.write32(edi.wrapping_add(0x110), eax);
                if fas > fbs { pc = 0x1c0eb8; continue; } // jg
                pc = 0x1c0e9c; continue;
            }
            0x1c0e9c => {
                cmp!(m.read32(ebp.wrapping_add(0x54)), 0xFFFFFFFF);
                if fa == fb { pc = 0x1c0eb8; continue; } // je
                pc = 0x1c0ea2; continue;
            }
            0x1c0ea2 => {
                push!(ebp);
                push!(ecx);
                push!(edi);
                push!(ebx);
                call!(0x1c0110);
                pc = 0x1c0eab; continue;
            }
            0x1c0eab => {
                source!(0x10);
                esp = esp.wrapping_add(0x10);
                result!(esp);
                m.write32(edi.wrapping_add(0x110), 0);
                pc = 0x1c0eb8; continue;
            }
            0x1c0eb8 => {
                // AMMO TAKE: 0x1B8530(O, buf, 1) then 0x1B6FF0(O, R.id, buf, R.+0x60); R.+0xAC = 1 (fired flag)
                push!(1);
                eax = esp.wrapping_add(0x70);
                push!(eax);
                push!(edi);
                call!(0x1b8530);
                pc = 0x1c0ec5; continue;
            }
            0x1c0ec5 => {
                ecx = m.read32(ebx.wrapping_add(0x60));
                eax = m.read32(ebx.wrapping_add(0x5C));
                push!(ecx);
                edx = esp.wrapping_add(0x7C);
                push!(edx);
                push!(eax);
                push!(edi);
                call!(0x1b6ff0);
                pc = 0x1c0ed8; continue;
            }
            0x1c0ed8 => {
                eax = m.read32(ebx.wrapping_add(0x30));
                source!(0x1C);
                esp = esp.wrapping_add(0x1C);
                result!(esp);
                cmp!(eax, 0xFFFFFFFF);
                m.write32(ebx.wrapping_add(0xAC), 1);
                ecx = m.read32(0x7B0C7C);
                if fa == fb { pc = 0x1c0f18; continue; } // je
                pc = 0x1c0ef3; continue;
            }
            0x1c0ef3 => {
                edx = m.read32(ecx.wrapping_add(4));
                cmp!(edx, 0);
                if fas < fbs { pc = 0x1c0f18; continue; } // jl
                pc = 0x1c0efa; continue;
            }
            0x1c0efa => {
                edx = m.read32(ecx.wrapping_add(0xE84));
                cmp!(edx, 0);
                if fa == fb { pc = 0x1c0f18; continue; } // je
                pc = 0x1c0f04; continue;
            }
            0x1c0f04 => {
                esi = m.read32(0x4F9BAC);
                edx = eax;
                { let product = (edx as i32 as i64) * (0x29C as u32 as i32 as i64);
                edx = product as u32;
                }
                cmp!(m.read32(edx.wrapping_add(esi)), 0xFFFFFFFF);
                if fa != fb { pc = 0x1c0f37; continue; } // jne
                pc = 0x1c0f18; continue;
            }
            0x1c0f18 => {
                // weapon id 0x22: R.+0x44 = 5 and R.+0x48 = [0x475D04]
                cmp!(eax, 0x22);
                if fa != fb { pc = 0x1c0f37; continue; } // jne
                pc = 0x1c0f1d; continue;
            }
            0x1c0f1d => {
                x[0] = Xmm::scalar(m.read32(0x475D04));
                m.write32(ebx.wrapping_add(0x44), 5);
                m.write32(ebx.wrapping_add(0x48), x[0].lanes[0]);
                ecx = m.read32(0x7B0C7C);
                pc = 0x1c0f37; continue;
            }
            0x1c0f37 => {
                // script hooks: when P.+4 >= 0 and the definition has a callback at +0x294, call it (9, R, O, 0), then the same for event 7
                edx = m.read32(ecx.wrapping_add(4));
                cmp!(edx, 0);
                if fas < fbs { pc = 0x1c0f65; continue; } // jl
                pc = 0x1c0f3e; continue;
            }
            0x1c0f3e => {
                cmp!(eax, 0);
                if fas < fbs { pc = 0x1c0f65; continue; } // jl
                pc = 0x1c0f42; continue;
            }
            0x1c0f42 => {
                edx = m.read32(0x4F9BAC);
                { let product = (eax as i32 as i64) * (0x29C as u32 as i32 as i64);
                eax = product as u32;
                }
                source!(edx);
                eax = eax.wrapping_add(edx);
                result!(eax);
                eax = m.read32(eax.wrapping_add(0x294));
                cmp!(eax, 0);
                if fa == fb { pc = 0x1c0f65; continue; } // je
                pc = 0x1c0f5a; continue;
            }
            0x1c0f5a => {
                push!(0);
                push!(edi);
                push!(ebx);
                push!(9);
                call!(eax); // indirect call, no code-range check (see module docs)
                pc = 0x1c0f62; continue;
            }
            0x1c0f62 => {
                source!(0x10);
                esp = esp.wrapping_add(0x10);
                result!(esp);
                pc = 0x1c0f65; continue;
            }
            0x1c0f65 => {
                eax = m.read32(ebx.wrapping_add(0x30));
                cmp!(eax, 0);
                if fas < fbs { pc = 0x1c0f8f; continue; } // jl
                pc = 0x1c0f6c; continue;
            }
            0x1c0f6c => {
                edx = m.read32(0x4F9BAC);
                { let product = (eax as i32 as i64) * (0x29C as u32 as i32 as i64);
                eax = product as u32;
                }
                source!(edx);
                eax = eax.wrapping_add(edx);
                result!(eax);
                eax = m.read32(eax.wrapping_add(0x294));
                cmp!(eax, 0);
                if fa == fb { pc = 0x1c0f8f; continue; } // je
                pc = 0x1c0f84; continue;
            }
            0x1c0f84 => {
                push!(0);
                push!(edi);
                push!(ebx);
                push!(7);
                call!(eax); // indirect call, no code-range check (see module docs)
                pc = 0x1c0f8c; continue;
            }
            0x1c0f8c => {
                source!(0x10);
                esp = esp.wrapping_add(0x10);
                result!(esp);
                pc = 0x1c0f8f; continue;
            }
            0x1c0f8f => {
                // EPILOGUE: pop EDI/ESI/EBP, release the frame
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                source!(0xC8);
                esp = esp.wrapping_add(0xC8);
                result!(esp);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    let _ = (fca, fcb, fpcmp, cf, fa, fb, fas, fbs);
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [x[0], x[1], x[2], x[3], x[4]], xmm_hi: [x[5], x[6], x[7]] };
}
