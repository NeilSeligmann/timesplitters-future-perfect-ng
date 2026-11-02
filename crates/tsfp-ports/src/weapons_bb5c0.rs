// SPDX-License-Identifier: GPL-3.0-or-later
//! T1831 batch 3b: 0x1BB5C0 (NPC-style shot start: aims from entity B at a target, spreads and spawns the
//! projectile burst). 988 lifted instructions, 4,239 bytes (0x1BB5C0..0x1BC64F), 21 direct callees, no
//! indirect calls. Same method and labels as `weapons_shot.rs` (mechanical block-per-arm transcription of the
//! lifted original, MEASURED by tests/c/test_rust_weapons_bb5c0.c on FABRICATED structured memory with stub
//! callees, names INFERRED). Not admitted. XMM5-7 are scratch locals (ABI carries XMM0-4 only).
#![allow(unexpected_cfgs)]
use super::shot::{fcmp, fl, fpc, parity8, set_st, st, xsqrt};
use super::sse;
use crate::{
    abi::{ExtRegisters, GuestCall, Xmm},
    memory::GuestMemory,
};

/// Edge coverage for the differential (build with `--cfg tsfp_cover`; absent from normal builds).
#[cfg(tsfp_cover)]
#[unsafe(no_mangle)]
pub static mut tsfp_bb5c0_edges: [u32; 4096] = [0; 4096];
#[cfg(tsfp_cover)]
#[unsafe(no_mangle)]
pub static mut tsfp_bb5c0_edge_count: u32 = 0;
#[cfg(tsfp_cover)]
fn cover(from: u32, to: u32) {
    let edge = ((from & 0xffff) << 16) | (to & 0xffff);
    unsafe {
        let count = core::ptr::addr_of_mut!(tsfp_bb5c0_edge_count);
        let list = core::ptr::addr_of_mut!(tsfp_bb5c0_edges) as *mut u32;
        for i in 0..*count {
            if *list.add(i as usize) == edge {
                return;
            }
        }
        if (*count as usize) < 4096 {
            *list.add(*count as usize) = edge;
            *count += 1;
        }
    }
}

/// 0x1BB5C0 `(entity, target, _, mode, flag, ...)`, cdecl, frame 0xE0. Returns 1 when a shot was started.
pub fn bb5c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = [Xmm::default(); 8];
    x[..5].copy_from_slice(&entry.xmm);
    x[5..].copy_from_slice(&entry.xmm_hi);
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, false);
    let (mut fca, mut fcb, mut fpcmp) = (0f64, 0f64, 0i32);
    let mut pc: u32 = 0x1bb5c0;
    // Flag model of the lifted original: the operands of the last CMP/TEST (`cmp!`/`cmp8!`) or the
    // result/source of the last arithmetic op (`result!`, `source!`); conditions read them later.
    macro_rules! cmp {
        ($a:expr, $b:expr) => {{
            fa = $a;
            fb = $b;
            fas = fa as i32;
            fbs = fb as i32;
        }};
    }
    macro_rules! cmp8 {
        ($a:expr, $b:expr) => {{
            fa = $a & 0xff;
            fb = $b & 0xff;
            fas = fa as u8 as i8 as i32;
            fbs = fb as u8 as i8 as i32;
        }};
    }
    macro_rules! result {
        ($v:expr) => {{
            fa = $v;
            fas = fa as i32;
        }};
    }
    macro_rules! source {
        ($v:expr) => {{
            fb = $v;
            fbs = fb as i32;
        }};
    }
    macro_rules! push {
        ($v:expr) => {{
            let value: u32 = $v;
            esp = esp.wrapping_sub(4);
            m.write32(esp, value);
        }};
    }
    macro_rules! call {
        ($va:expr) => {{
            let target: u32 = $va;
            let mut cr = ExtRegisters {
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
                xmm: [x[0], x[1], x[2], x[3], x[4]],
                xmm_hi: [x[5], x[6], x[7]],
            };
            guest.call(target, &mut cr);
            (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (
                cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi,
            );
            fpu = cr.fpu;
            x[..5].copy_from_slice(&cr.xmm);
            x[5..].copy_from_slice(&cr.xmm_hi);
        }};
    }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        {
            cover(previous, pc);
            previous = pc;
        }
        match pc {
            0x1bb5c0 => {
                eax = m.read32(esp.wrapping_add(8));
                source!(0xE0);
                esp = esp.wrapping_sub(0xE0);
                result!(esp);
                cmp!(eax, 0);
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0xE8));
                push!(esi);
                esi = m.read32(ebx.wrapping_add(0x7C));
                push!(edi);
                edi = esi.wrapping_add(0xA4);
                if fa == fb {
                    pc = 0x1bb5e7;
                    continue;
                } // je
                pc = 0x1bb5e1;
                continue;
            }
            0x1bb5e1 => {
                edi = esi.wrapping_add(0x1D4);
                pc = 0x1bb5e7;
                continue;
            }
            0x1bb5e7 => {
                ecx = m.read32(esi.wrapping_add(4));
                cmp!(ecx, 0);
                if fa == fb {
                    pc = 0x1bc643;
                    continue;
                } // je
                pc = 0x1bb5f2;
                continue;
            }
            0x1bb5f2 => {
                ecx = m.read32(ebx.wrapping_add(0x34));
                cmp!(ecx, 0);
                if fa == fb {
                    pc = 0x1bc643;
                    continue;
                } // je
                pc = 0x1bb5fd;
                continue;
            }
            0x1bb5fd => {
                cmp!(m.read32(0x4E7944), 1);
                if fas < fbs {
                    pc = 0x1bc643;
                    continue;
                } // jl
                pc = 0x1bb60a;
                continue;
            }
            0x1bb60a => {
                cmp!(m.read32(edi.wrapping_add(0x30)), 0xFFFFFFFF);
                if fa == fb {
                    pc = 0x1bc643;
                    continue;
                } // je
                pc = 0x1bb614;
                continue;
            }
            0x1bb614 => {
                ecx = m.read32(ebx.wrapping_add(0x150));
                cmp!(ecx, 0);
                if fa != fb {
                    pc = 0x1bb640;
                    continue;
                } // jne
                pc = 0x1bb61e;
                continue;
            }
            0x1bb61e => {
                ecx = m.read32(esp.wrapping_add(0xFC));
                edx = m.read32(esp.wrapping_add(0xF8));
                push!(ecx);
                push!(edx);
                push!(eax);
                push!(ebx);
                call!(0x35de60);
                pc = 0x1bb635;
                continue;
            }
            0x1bb635 => {
                source!(0x10);
                esp = esp.wrapping_add(0x10);
                result!(esp);
                cmp!(eax, 0);
                if fa == fb {
                    pc = 0x1bc643;
                    continue;
                } // je
                pc = 0x1bb640;
                continue;
            }
            0x1bb640 => {
                x[3] = Xmm::scalar(m.read32(esi.wrapping_add(0xB88)));
                m.write32(esp.wrapping_add(0x3C), x[3].lanes[0]);
                x[2] = Xmm::scalar(m.read32(esi.wrapping_add(0xB8C)));
                m.write32(esp.wrapping_add(0x40), x[2].lanes[0]);
                x[1] = Xmm::scalar(m.read32(esi.wrapping_add(0xB90)));
                m.write32(esp.wrapping_add(0x44), x[1].lanes[0]);
                eax = m.read32(esi.wrapping_add(0x2C));
                cmp8!(((eax >> 8) & 0xff), (0x10));
                if (fa & fb) == 0 {
                    pc = 0x1bb8b8;
                    continue;
                } // je
                pc = 0x1bb676;
                continue;
            }
            0x1bb676 => {
                eax = m.read32(esi);
                x[0] = Xmm::scalar(m.read32(eax.wrapping_add(0x6C)));
                x[0].lanes[0] = sse::subss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[1] = Xmm::scalar(m.read32(eax.wrapping_add(0x68)));
                x[1].lanes[0] = sse::subss(fl(x[1].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[2] = Xmm::scalar(m.read32(eax.wrapping_add(0x64)));
                x[2].lanes[0] = sse::subss(fl(x[2].lanes[0]), fl(x[3].lanes[0])).to_bits();
                x[3] = x[2];
                x[3].lanes[0] = sse::mulss(fl(x[3].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[2] = x[1];
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(x[1].lanes[0])).to_bits();
                push!(0x3F800000);
                x[3].lanes[0] = sse::addss(fl(x[3].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[1] = x[0];
                push!(0x3E4CCCCD);
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[3].lanes[0] = sse::addss(fl(x[3].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[3].lanes[0] = sse::mulss(fl(x[3].lanes[0]), fl(m.read32(0x475CE8))).to_bits();
                push!(ecx);
                m.write32(esp, x[3].lanes[0]);
                call!(0x124f0);
                pc = 0x1bb6cd;
                continue;
            }
            0x1bb6cd => {
                m.write32(esp.wrapping_add(0x18), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                source!(0xC);
                esp = esp.wrapping_add(0xC);
                result!(esp);
                call!(0x153b40);
                pc = 0x1bb6d9;
                continue;
            }
            0x1bb6d9 => {
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x4C), eax);
                fpu.push(m.read32(esp.wrapping_add(0x4C)) as i32 as f64);
                if fas >= fbs {
                    pc = 0x1bb6eb;
                    continue;
                } // jge
                pc = 0x1bb6e5;
                continue;
            }
            0x1bb6e5 => {
                {
                    let v = st(&fpu, 0) + fl(m.read32(0x475CCC)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                pc = 0x1bb6eb;
                continue;
            }
            0x1bb6eb => {
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x4785D0)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                x[4] = Xmm::default();
                x[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x3C)));
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x44)));
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x475CD0)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 0) - fl(m.read32(0x475D04)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 0) * fl(m.read32(esp.wrapping_add(0xC))) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 0) + fl(m.read32(esp.wrapping_add(0x40))) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                m.write32(esp.wrapping_add(0x40), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                eax = m.read32(esi);
                x[0].lanes[0] =
                    sse::subss(fl(x[0].lanes[0]), fl(m.read32(eax.wrapping_add(0x6C)))).to_bits();
                x[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                x[2].lanes[0] =
                    sse::subss(fl(x[2].lanes[0]), fl(m.read32(eax.wrapping_add(0x68)))).to_bits();
                x[1].lanes[0] =
                    sse::subss(fl(x[1].lanes[0]), fl(m.read32(eax.wrapping_add(0x64)))).to_bits();
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(x[4].lanes[0])).to_bits();
                x[3] = x[2];
                x[3].lanes[0] = sse::subss(fl(x[3].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[4].lanes[0])).to_bits();
                x[5] = x[1];
                x[1].lanes[0] = sse::subss(fl(x[1].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[5].lanes[0] = sse::mulss(fl(x[5].lanes[0]), fl(x[4].lanes[0])).to_bits();
                x[0].lanes[0] = sse::subss(fl(x[0].lanes[0]), fl(x[5].lanes[0])).to_bits();
                x[4] = x[0];
                x[0] = x[1];
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[2] = x[4];
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(x[4].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[2] = x[3];
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(x[3].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[2].lanes[0])).to_bits();
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(m.read32(0x4765F4)) as f64;
                m.write32(esp.wrapping_add(0x28), x[3].lanes[0]);
                m.write32(esp.wrapping_add(0x2C), x[4].lanes[0]);
                m.write32(esp.wrapping_add(0x30), x[1].lanes[0]);
                if !(fca > fcb) {
                    pc = 0x1bb7d8;
                    continue;
                } // jbe
                pc = 0x1bb788;
                continue;
            }
            0x1bb788 => {
                m.write32(esp.wrapping_add(0x34), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(0x475C78));
                m.write32(esp.wrapping_add(0x38), x[0].lanes[0]);
                x[0].lanes[0] =
                    sse::divss(1.0, xsqrt::ss(fl(m.read32(esp.wrapping_add(0x34))))).to_bits();
                x[0].lanes[0] =
                    sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x38)))).to_bits();
                m.write32(esp.wrapping_add(0x54), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x54)));
                x[2] = x[0];
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(x[3].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x28), x[2].lanes[0]);
                x[2] = x[0];
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(x[4].lanes[0])).to_bits();
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x2C), x[2].lanes[0]);
                m.write32(esp.wrapping_add(0x30), x[0].lanes[0]);
                pc = 0x1bb7d8;
                continue;
            }
            0x1bb7d8 => {
                call!(0x153b40);
                pc = 0x1bb7dd;
                continue;
            }
            0x1bb7dd => {
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x4C), eax);
                fpu.push(m.read32(esp.wrapping_add(0x4C)) as i32 as f64);
                if fas >= fbs {
                    pc = 0x1bb7ef;
                    continue;
                } // jge
                pc = 0x1bb7e9;
                continue;
            }
            0x1bb7e9 => {
                {
                    let v = st(&fpu, 0) + fl(m.read32(0x475CCC)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                pc = 0x1bb7ef;
                continue;
            }
            0x1bb7ef => {
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x4785D0)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 0) + fl(m.read32(0x475CD4)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                call!(0x3c848c);
                pc = 0x1bb800;
                continue;
            }
            0x1bb800 => {
                cmp!(eax, 0);
                if fa != fb {
                    pc = 0x1bb835;
                    continue;
                } // jne
                pc = 0x1bb804;
                continue;
            }
            0x1bb804 => {
                call!(0x153b40);
                pc = 0x1bb809;
                continue;
            }
            0x1bb809 => {
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x4C), eax);
                fpu.push(m.read32(esp.wrapping_add(0x4C)) as i32 as f64);
                if fas >= fbs {
                    pc = 0x1bb81b;
                    continue;
                } // jge
                pc = 0x1bb815;
                continue;
            }
            0x1bb815 => {
                {
                    let v = st(&fpu, 0) + fl(m.read32(0x475CCC)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                pc = 0x1bb81b;
                continue;
            }
            0x1bb81b => {
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x4785D0)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x475CDC)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                fpu.push(fl(m.read32(esp.wrapping_add(0xC))) as f64);
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x475CD4)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 1) + st(&fpu, 0);
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 1, v);
                }
                fpu.pop();
                pc = 0x1bb866;
                continue;
            }
            0x1bb835 => {
                call!(0x153b40);
                pc = 0x1bb83a;
                continue;
            }
            0x1bb83a => {
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x4C), eax);
                fpu.push(m.read32(esp.wrapping_add(0x4C)) as i32 as f64);
                if fas >= fbs {
                    pc = 0x1bb84c;
                    continue;
                } // jge
                pc = 0x1bb846;
                continue;
            }
            0x1bb846 => {
                {
                    let v = st(&fpu, 0) + fl(m.read32(0x475CCC)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                pc = 0x1bb84c;
                continue;
            }
            0x1bb84c => {
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x4785D0)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x475CDC)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                fpu.push(fl(m.read32(esp.wrapping_add(0xC))) as f64);
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x475CD4)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 1) + st(&fpu, 0);
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 1, v);
                }
                fpu.pop();
                {
                    let v = f64::from_bits(st(&fpu, 0).to_bits() ^ (1u64 << 63));
                    set_st(&mut fpu, 0, v);
                } // fchs: flips the sign bit, NaN included
                pc = 0x1bb866;
                continue;
            }
            0x1bb866 => {
                fpu.push(fl(m.read32(esp.wrapping_add(0x28))) as f64);
                {
                    let v = st(&fpu, 0) * st(&fpu, 1);
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                m.write32(esp.wrapping_add(0x28), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                fpu.push(fl(m.read32(esp.wrapping_add(0x2C))) as f64);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                x[0].lanes[0] =
                    sse::addss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x3C)))).to_bits();
                {
                    let v = st(&fpu, 0) * st(&fpu, 1);
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                m.write32(esp.wrapping_add(0x3C), x[0].lanes[0]);
                m.write32(esp.wrapping_add(0x2C), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x2C)));
                {
                    let v = st(&fpu, 0) * fl(m.read32(esp.wrapping_add(0x30))) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                x[0].lanes[0] =
                    sse::addss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x40)))).to_bits();
                m.write32(esp.wrapping_add(0x40), x[0].lanes[0]);
                m.write32(esp.wrapping_add(0x30), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                x[0].lanes[0] =
                    sse::addss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x44)))).to_bits();
                m.write32(esp.wrapping_add(0x44), x[0].lanes[0]);
                pc = 0x1bb8b8;
                continue;
            }
            0x1bb8b8 => {
                ecx = m.read32(esi.wrapping_add(4));
                cmp!(m.read32(ecx.wrapping_add(0x20)), 8);
                push!(ebp);
                if fa != fb {
                    pc = 0x1bba95;
                    continue;
                } // jne
                pc = 0x1bb8c6;
                continue;
            }
            0x1bb8c6 => {
                eax = m.read32(esi);
                cmp!(eax, 0);
                if fa == fb {
                    pc = 0x1bba95;
                    continue;
                } // je
                pc = 0x1bb8d0;
                continue;
            }
            0x1bb8d0 => {
                edx = m.read32(0x790950);
                cmp!(m.read32(esi.wrapping_add(8)), edx);
                if fas < fbs {
                    pc = 0x1bba95;
                    continue;
                } // jl
                pc = 0x1bb8df;
                continue;
            }
            0x1bb8df => {
                ebp = 0x100;
                cmp!(m.read32(eax.wrapping_add(0x28)), ebp);
                if (fa & fb) != 0 {
                    pc = 0x1bb8f6;
                    continue;
                } // jne
                pc = 0x1bb8e9;
                continue;
            }
            0x1bb8e9 => {
                cmp!(m.read32(eax.wrapping_add(0x10)), 0xFFFF);
                if (fa & fb) != 0 {
                    pc = 0x1bba95;
                    continue;
                } // jne
                pc = 0x1bb8f6;
                continue;
            }
            0x1bb8f6 => {
                cmp!(ecx, 0);
                if fa == fb {
                    pc = 0x1bba95;
                    continue;
                } // je
                pc = 0x1bb8fe;
                continue;
            }
            0x1bb8fe => {
                eax = m.read32(ecx.wrapping_add(0x7C));
                cmp!(eax, 0);
                if fa == fb {
                    pc = 0x1bba95;
                    continue;
                } // je
                pc = 0x1bb909;
                continue;
            }
            0x1bb909 => {
                cmp!(m.read32(eax.wrapping_add(8)), edx);
                if fas < fbs {
                    pc = 0x1bba95;
                    continue;
                } // jl
                pc = 0x1bb912;
                continue;
            }
            0x1bb912 => {
                cmp!(m.read32(ecx.wrapping_add(0x28)), ebp);
                if (fa & fb) != 0 {
                    pc = 0x1bb924;
                    continue;
                } // jne
                pc = 0x1bb917;
                continue;
            }
            0x1bb917 => {
                cmp!(m.read32(ecx.wrapping_add(0x10)), 0xFFFF);
                if (fa & fb) != 0 {
                    pc = 0x1bba95;
                    continue;
                } // jne
                pc = 0x1bb924;
                continue;
            }
            0x1bb924 => {
                ecx = m.read32(edi.wrapping_add(0x30));
                eax = m.read32(esp.wrapping_add(0xFC));
                {
                    let product = (ecx as i32 as i64) * (0x29C as u32 as i32 as i64);
                    ecx = product as u32;
                }
                ebp = m.read32(0x4F9BAC);
                push!(eax);
                source!(ebp);
                ecx = ecx.wrapping_add(ebp);
                result!(ecx);
                push!(ecx);
                call!(0x1b5660);
                pc = 0x1bb943;
                continue;
            }
            0x1bb943 => {
                ebp = eax;
                eax = (eax & 0xffff_ff00) | ((m.read8(0x7DE455) as u32) & 0xff);
                source!(8);
                esp = esp.wrapping_add(8);
                result!(esp);
                cmp8!((eax & 0xff), (0xA));
                if fa == fb {
                    pc = 0x1bba77;
                    continue;
                } // je
                pc = 0x1bb955;
                continue;
            }
            0x1bb955 => {
                edx = esp.wrapping_add(0x94);
                push!(edx);
                call!(0xb91f0);
                pc = 0x1bb962;
                continue;
            }
            0x1bb962 => {
                eax = m.read32(esi.wrapping_add(4));
                ecx = m.read32(ebp.wrapping_add(4));
                m.write32(esp.wrapping_add(0x98), eax);
                eax = m.read32(ebp.wrapping_add(0x60));
                source!(4);
                esp = esp.wrapping_add(4);
                result!(esp);
                cmp!(eax, 0xFFFFFFFF);
                m.write32(esp.wrapping_add(0x98), ebx);
                m.write32(esp.wrapping_add(0x9C), ebx);
                m.write32(esp.wrapping_add(0xA8), ecx);
                m.write32(esp.wrapping_add(0xAC), ecx);
                if fa == fb {
                    pc = 0x1bb99b;
                    continue;
                } // je
                pc = 0x1bb996;
                continue;
            }
            0x1bb996 => {
                cmp!(eax, 0x15);
                if fa != fb {
                    pc = 0x1bb9b8;
                    continue;
                } // jne
                pc = 0x1bb99b;
                continue;
            }
            0x1bb99b => {
                push!(ecx);
                call!(0xdc270);
                pc = 0x1bb9a1;
                continue;
            }
            0x1bb9a1 => {
                source!(4);
                esp = esp.wrapping_add(4);
                result!(esp);
                cmp!(eax, 0x802);
                if (fa & fb) == 0 {
                    pc = 0x1bb9b8;
                    continue;
                } // je
                pc = 0x1bb9ab;
                continue;
            }
            0x1bb9ab => {
                m.write32(esp.wrapping_add(0xB8), 0);
                pc = 0x1bb9c2;
                continue;
            }
            0x1bb9b8 => {
                ecx = m.read32(ebp.wrapping_add(0x60));
                m.write32(esp.wrapping_add(0xB8), ecx);
                pc = 0x1bb9c2;
                continue;
            }
            0x1bb9c2 => {
                eax = m.read32(ebp.wrapping_add(8));
                cmp!(eax, 0);
                edx = esp.wrapping_add(0x40);
                m.write32(esp.wrapping_add(0xBC), edx);
                m.write32(esp.wrapping_add(0x10), 0);
                if fas <= fbs {
                    pc = 0x1bba77;
                    continue;
                } // jle
                pc = 0x1bb9e0;
                continue;
            }
            0x1bb9e0 => {
                eax = m.read32(esi.wrapping_add(8));
                push!(0x3F800000);
                push!(eax);
                push!(8);
                call!(0x238510);
                pc = 0x1bb9f0;
                continue;
            }
            0x1bb9f0 => {
                ecx = m.read32(ebp.wrapping_add(4));
                push!(ecx);
                push!(ebx);
                call!(0x2b8ea0);
                pc = 0x1bb9fa;
                continue;
            }
            0x1bb9fa => {
                source!(0x14);
                esp = esp.wrapping_add(0x14);
                result!(esp);
                cmp!(eax, 0);
                if fa == fb {
                    pc = 0x1bba63;
                    continue;
                } // je
                pc = 0x1bba01;
                continue;
            }
            0x1bba01 => {
                edx = esp.wrapping_add(0x94);
                push!(edx);
                call!(0xbd0f0);
                pc = 0x1bba0e;
                continue;
            }
            0x1bba0e => {
                eax = m.read32(ebp.wrapping_add(4));
                push!(eax);
                call!(0xdc250);
                pc = 0x1bba17;
                continue;
            }
            0x1bba17 => {
                source!(8);
                esp = esp.wrapping_add(8);
                result!(esp);
                cmp!(eax, 0);
                if fa == fb {
                    pc = 0x1bba50;
                    continue;
                } // je
                pc = 0x1bba1e;
                continue;
            }
            0x1bba1e => {
                call!(0x153b40);
                pc = 0x1bba23;
                continue;
            }
            0x1bba23 => {
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x50), eax);
                fpu.push(m.read32(esp.wrapping_add(0x50)) as i32 as f64);
                if fas >= fbs {
                    pc = 0x1bba35;
                    continue;
                } // jge
                pc = 0x1bba2f;
                continue;
            }
            0x1bba2f => {
                {
                    let v = st(&fpu, 0) + fl(m.read32(0x475CCC)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                pc = 0x1bba35;
                continue;
            }
            0x1bba35 => {
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x4785D0)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x475D18)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 0) + fl(m.read32(0x475CD4)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                call!(0x3c848c);
                pc = 0x1bba4c;
                continue;
            }
            0x1bba4c => {
                cmp!(eax, 0);
                if fa != fb {
                    pc = 0x1bba63;
                    continue;
                } // jne
                pc = 0x1bba50;
                continue;
            }
            0x1bba50 => {
                ecx = m.read32(esi.wrapping_add(8));
                push!(0x3F800000);
                push!(ecx);
                push!(9);
                call!(0x238510);
                pc = 0x1bba60;
                continue;
            }
            0x1bba60 => {
                source!(0xC);
                esp = esp.wrapping_add(0xC);
                result!(esp);
                pc = 0x1bba63;
                continue;
            }
            0x1bba63 => {
                eax = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(ebp.wrapping_add(8));
                eax = eax.wrapping_add(1);
                result!(eax);
                fb = (fa == 0x8000_0000) as u32;
                cmp!(eax, ecx);
                m.write32(esp.wrapping_add(0x10), eax);
                if fas < fbs {
                    pc = 0x1bb9e0;
                    continue;
                } // jl
                pc = 0x1bba77;
                continue;
            }
            0x1bba77 => {
                eax = 0;
                result!(eax);
                m.write32(edi.wrapping_add(0xF0), eax);
                m.write32(edi.wrapping_add(0xF4), eax);
                pc = 0x1bba85;
                continue;
            }
            0x1bba85 => {
                ebp = m.read32(esp);
                esp = esp.wrapping_add(4);
                edi = m.read32(esp);
                esp = esp.wrapping_add(4);
                esi = m.read32(esp);
                esp = esp.wrapping_add(4);
                eax = 1;
                ebx = m.read32(esp);
                esp = esp.wrapping_add(4);
                source!(0xE0);
                esp = esp.wrapping_add(0xE0);
                result!(esp);
                break 'run;
            }
            0x1bba95 => {
                ebp = m.read32(edi.wrapping_add(0x30));
                eax = m.read32(0x4F9BAC);
                {
                    let product = (ebp as i32 as i64) * (0x29C as u32 as i32 as i64);
                    ebp = product as u32;
                }
                edx = m.read32(esp.wrapping_add(0xFC));
                source!(eax);
                ebp = ebp.wrapping_add(eax);
                result!(ebp);
                push!(edx);
                push!(ebp);
                m.write32(esp.wrapping_add(0x60), ebp);
                call!(0x1b5660);
                pc = 0x1bbab7;
                continue;
            }
            0x1bbab7 => {
                x[4] = Xmm::scalar(m.read32(0x475C78));
                ebx = eax;
                eax = 0;
                result!(eax);
                m.write32(edi.wrapping_add(0xF0), eax);
                m.write32(edi.wrapping_add(0xF4), eax);
                x[0].lanes[0] = (m.read32(0x4E8CEC) as i32 as f32).to_bits();
                x[1] = x[4];
                x[1].lanes[0] = sse::divss(fl(x[1].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(m.read32(0x4E794C))).to_bits();
                x[1].lanes[0] =
                    sse::addss(fl(x[1].lanes[0]), fl(m.read32(edi.wrapping_add(0x104)))).to_bits();
                m.write32(edi.wrapping_add(0x104), x[1].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esi.wrapping_add(0xB7C)));
                x[2] = Xmm::scalar(m.read32(esp.wrapping_add(0x4C)));
                x[3] = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                x[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                m.write32(esp.wrapping_add(0x1C), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esi.wrapping_add(0xB80)));
                m.write32(esp.wrapping_add(0x20), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esi.wrapping_add(0xB84)));
                m.write32(esp.wrapping_add(0x24), x[0].lanes[0]);
                x[2].lanes[0] =
                    sse::subss(fl(x[2].lanes[0]), fl(m.read32(edi.wrapping_add(4)))).to_bits();
                x[3].lanes[0] =
                    sse::subss(fl(x[3].lanes[0]), fl(m.read32(edi.wrapping_add(8)))).to_bits();
                x[1].lanes[0] = sse::subss(fl(x[1].lanes[0]), fl(m.read32(edi))).to_bits();
                x[0] = x[3];
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[3].lanes[0])).to_bits();
                x[5] = x[2];
                x[5].lanes[0] = sse::mulss(fl(x[5].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[5].lanes[0])).to_bits();
                x[5] = x[1];
                x[5].lanes[0] = sse::mulss(fl(x[5].lanes[0]), fl(x[1].lanes[0])).to_bits();
                source!(8);
                esp = esp.wrapping_add(8);
                result!(esp);
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[5].lanes[0])).to_bits();
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(m.read32(0x4765F4)) as f64;
                m.write32(esp.wrapping_add(0x2C), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x30), x[2].lanes[0]);
                m.write32(esp.wrapping_add(0x34), x[3].lanes[0]);
                if !(fca > fcb) {
                    pc = 0x1bbbc3;
                    continue;
                } // jbe
                pc = 0x1bbb7b;
                continue;
            }
            0x1bbb7b => {
                m.write32(esp.wrapping_add(0x38), x[0].lanes[0]);
                m.write32(esp.wrapping_add(0x3C), x[4].lanes[0]);
                x[0].lanes[0] =
                    sse::divss(1.0, xsqrt::ss(fl(m.read32(esp.wrapping_add(0x38))))).to_bits();
                x[0].lanes[0] =
                    sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x3C)))).to_bits();
                m.write32(esp.wrapping_add(0x10), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                x[4] = x[0];
                x[4].lanes[0] = sse::mulss(fl(x[4].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[1] = x[0];
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[3].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x2C), x[4].lanes[0]);
                m.write32(esp.wrapping_add(0x30), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x34), x[0].lanes[0]);
                pc = 0x1bbbc3;
                continue;
            }
            0x1bbbc3 => {
                eax = m.read32(ebx.wrapping_add(4));
                push!(eax);
                call!(0xdc270);
                pc = 0x1bbbcc;
                continue;
            }
            0x1bbbcc => {
                source!(4);
                esp = esp.wrapping_add(4);
                result!(esp);
                cmp8!(((eax >> 8) & 0xff), (2));
                if (fa & fb) == 0 {
                    pc = 0x1bbd59;
                    continue;
                } // je
                pc = 0x1bbbd8;
                continue;
            }
            0x1bbbd8 => {
                ecx = m.read32(esi.wrapping_add(4));
                x[0] = Xmm::scalar(m.read32(ecx.wrapping_add(0x64)));
                ecx = m.read32(esp.wrapping_add(0xF4));
                x[0].lanes[0] =
                    sse::subss(fl(x[0].lanes[0]), fl(m.read32(ecx.wrapping_add(0x64)))).to_bits();
                m.write32(esp.wrapping_add(0x14), x[0].lanes[0]);
                eax = m.read16(esi.wrapping_add(0x48)) as i16 as i32 as u32;
                edx = m.read32(esi.wrapping_add(4));
                x[2] = Xmm::scalar(m.read32(edx.wrapping_add(0x68)));
                eax = eax.wrapping_add(eax.wrapping_mul(2));
                eax = (eax << (((4) & 31) & 31));
                result!(eax);
                x[1] = Xmm::scalar(m.read32(eax.wrapping_add(0x53A3E4)));
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(m.read32(0x475CD0))).to_bits();
                x[1].lanes[0] =
                    sse::addss(fl(x[1].lanes[0]), fl(m.read32(ecx.wrapping_add(0x68)))).to_bits();
                x[2].lanes[0] = sse::subss(fl(x[2].lanes[0]), fl(x[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x18), x[2].lanes[0]);
                edx = m.read32(esi.wrapping_add(4));
                x[3] = Xmm::scalar(m.read32(edx.wrapping_add(0x6C)));
                x[3].lanes[0] =
                    sse::subss(fl(x[3].lanes[0]), fl(m.read32(ecx.wrapping_add(0x6C)))).to_bits();
                x[5] = x[0];
                x[5].lanes[0] = sse::mulss(fl(x[5].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[4] = x[3];
                x[4].lanes[0] = sse::mulss(fl(x[4].lanes[0]), fl(x[3].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x3C), x[5].lanes[0]);
                fpu.push(fl(m.read32(esp.wrapping_add(0x3C))) as f64);
                m.write32(esp.wrapping_add(0x38), x[4].lanes[0]);
                {
                    let v = st(&fpu, 0) + fl(m.read32(esp.wrapping_add(0x38))) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                x[6] = Xmm::scalar(m.read32(0x47E2BC));
                x[7] = x[2];
                {
                    let v = xsqrt::sd(st(&fpu, 0));
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                x[7].lanes[0] = sse::mulss(fl(x[7].lanes[0]), fl(x[6].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x1C), x[3].lanes[0]);
                m.write32(esp.wrapping_add(0x10), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                x[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                x[7].lanes[0] =
                    sse::divss(fl(x[7].lanes[0]), fl(m.read32(esp.wrapping_add(0x10)))).to_bits();
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[6].lanes[0])).to_bits();
                x[6] = Xmm::scalar(m.read32(0x475C78));
                x[1].lanes[0] = sse::addss(fl(x[1].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[6].lanes[0] = sse::subss(fl(x[6].lanes[0]), fl(x[7].lanes[0])).to_bits();
                x[1].lanes[0] = sse::divss(fl(x[1].lanes[0]), fl(x[6].lanes[0])).to_bits();
                x[1].lanes[0] = sse::addss(fl(x[1].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[2] = x[1];
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[2].lanes[0] = sse::addss(fl(x[2].lanes[0]), fl(x[5].lanes[0])).to_bits();
                x[2].lanes[0] = sse::addss(fl(x[2].lanes[0]), fl(x[4].lanes[0])).to_bits();
                fca = fl(x[2].lanes[0]) as f64;
                fcb = fl(m.read32(0x4765F4)) as f64;
                m.write32(esp.wrapping_add(0x18), x[1].lanes[0]);
                if !(fca > fcb) {
                    pc = 0x1bbd09;
                    continue;
                } // jbe
                pc = 0x1bbcb3;
                continue;
            }
            0x1bbcb3 => {
                x[0] = Xmm::scalar(m.read32(0x475C78));
                m.write32(esp.wrapping_add(0x38), x[2].lanes[0]);
                m.write32(esp.wrapping_add(0x3C), x[0].lanes[0]);
                x[0].lanes[0] =
                    sse::divss(1.0, xsqrt::ss(fl(m.read32(esp.wrapping_add(0x38))))).to_bits();
                x[0].lanes[0] =
                    sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x3C)))).to_bits();
                m.write32(esp.wrapping_add(0x10), x[0].lanes[0]);
                x[3] = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                x[0] = x[3];
                x[0].lanes[0] =
                    sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x14)))).to_bits();
                x[1] = x[3];
                x[1].lanes[0] =
                    sse::mulss(fl(x[1].lanes[0]), fl(m.read32(esp.wrapping_add(0x18)))).to_bits();
                x[3].lanes[0] =
                    sse::mulss(fl(x[3].lanes[0]), fl(m.read32(esp.wrapping_add(0x1C)))).to_bits();
                m.write32(esp.wrapping_add(0x14), x[0].lanes[0]);
                m.write32(esp.wrapping_add(0x18), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x1C), x[3].lanes[0]);
                pc = 0x1bbd09;
                continue;
            }
            0x1bbd09 => {
                m.write32(esp.wrapping_add(0x2C), x[0].lanes[0]);
                m.write32(esp.wrapping_add(0x30), x[1].lanes[0]);
                x[1] = Xmm::scalar(m.read32(0x475D24));
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(m.read32(edi))).to_bits();
                m.write32(edi, x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[0].lanes[0] =
                    sse::addss(fl(x[0].lanes[0]), fl(m.read32(edi.wrapping_add(4)))).to_bits();
                m.write32(edi.wrapping_add(4), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x1C)));
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[0].lanes[0] =
                    sse::addss(fl(x[0].lanes[0]), fl(m.read32(edi.wrapping_add(8)))).to_bits();
                m.write32(esp.wrapping_add(0x34), x[3].lanes[0]);
                m.write32(edi.wrapping_add(8), x[0].lanes[0]);
                pc = 0x1bbd60;
                continue;
            }
            0x1bbd59 => {
                ecx = m.read32(esp.wrapping_add(0xF4));
                pc = 0x1bbd60;
                continue;
            }
            0x1bbd60 => {
                eax = m.read32(ebp.wrapping_add(0x14));
                edx = m.read32(ebp.wrapping_add(0x18));
                m.write32(esp.wrapping_add(0x3C), eax);
                cmp!(m.read32(esp.wrapping_add(0x100)), 1);
                m.write32(esp.wrapping_add(0x10), edx);
                if fa != fb {
                    pc = 0x1bbd81;
                    continue;
                } // jne
                pc = 0x1bbd78;
                continue;
            }
            0x1bbd78 => {
                eax = m.read32(ebx.wrapping_add(0x28));
                m.write32(esp.wrapping_add(0x38), eax);
                pc = 0x1bbd88;
                continue;
            }
            0x1bbd81 => {
                edx = m.read32(ebx.wrapping_add(0x24));
                m.write32(esp.wrapping_add(0x38), edx);
                pc = 0x1bbd88;
                continue;
            }
            0x1bbd88 => {
                eax = m.read32(esi.wrapping_add(4));
                x[0] = Xmm::scalar(m.read32(ecx.wrapping_add(0x6C)));
                x[0].lanes[0] =
                    sse::subss(fl(x[0].lanes[0]), fl(m.read32(eax.wrapping_add(0x6C)))).to_bits();
                x[1] = Xmm::scalar(m.read32(ecx.wrapping_add(0x68)));
                x[1].lanes[0] =
                    sse::subss(fl(x[1].lanes[0]), fl(m.read32(eax.wrapping_add(0x68)))).to_bits();
                x[2] = Xmm::scalar(m.read32(ecx.wrapping_add(0x64)));
                x[2].lanes[0] =
                    sse::subss(fl(x[2].lanes[0]), fl(m.read32(eax.wrapping_add(0x64)))).to_bits();
                x[6] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                x[4] = Xmm::scalar(m.read32(esp.wrapping_add(0x2C)));
                x[3] = x[0];
                x[3].lanes[0] = sse::mulss(fl(x[3].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[0] = x[2];
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[2] = Xmm::default();
                x[1].lanes[0] = sse::addss(fl(x[1].lanes[0]), fl(x[3].lanes[0])).to_bits();
                x[3] = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                x[1].lanes[0] = sse::addss(fl(x[1].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x1C)));
                m.write32(esp.wrapping_add(0x74), x[0].lanes[0]);
                x[7] = x[6];
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[7].lanes[0] = sse::mulss(fl(x[7].lanes[0]), fl(x[6].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[7].lanes[0])).to_bits();
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(m.read32(0x4765F4)) as f64;
                x[5] = x[2];
                m.write32(esp.wrapping_add(0x78), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x6C), x[6].lanes[0]);
                m.write32(esp.wrapping_add(0x70), x[2].lanes[0]);
                m.write32(esp.wrapping_add(0x60), x[4].lanes[0]);
                m.write32(esp.wrapping_add(0x64), x[5].lanes[0]);
                m.write32(esp.wrapping_add(0x68), x[3].lanes[0]);
                if !(fca > fcb) {
                    pc = 0x1bbe77;
                    continue;
                } // jbe
                pc = 0x1bbe23;
                continue;
            }
            0x1bbe23 => {
                m.write32(esp.wrapping_add(0x54), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(0x475C78));
                m.write32(esp.wrapping_add(0x5C), x[0].lanes[0]);
                x[0].lanes[0] =
                    sse::divss(1.0, xsqrt::ss(fl(m.read32(esp.wrapping_add(0x54))))).to_bits();
                x[0].lanes[0] =
                    sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x5C)))).to_bits();
                m.write32(esp.wrapping_add(0x4C), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x4C)));
                x[6] = x[0];
                x[6].lanes[0] =
                    sse::mulss(fl(x[6].lanes[0]), fl(m.read32(esp.wrapping_add(0x6C)))).to_bits();
                m.write32(esp.wrapping_add(0x6C), x[6].lanes[0]);
                x[6] = x[0];
                x[0].lanes[0] =
                    sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x74)))).to_bits();
                x[6].lanes[0] = sse::mulss(fl(x[6].lanes[0]), fl(x[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x70), x[6].lanes[0]);
                m.write32(esp.wrapping_add(0x74), x[0].lanes[0]);
                pc = 0x1bbe77;
                continue;
            }
            0x1bbe77 => {
                x[6] = Xmm::scalar(m.read32(esp.wrapping_add(0x2C)));
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                x[7] = x[6];
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[7].lanes[0] = sse::mulss(fl(x[7].lanes[0]), fl(x[6].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[7].lanes[0])).to_bits();
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(m.read32(0x4765F4)) as f64;
                if !(fca > fcb) {
                    pc = 0x1bbef9;
                    continue;
                } // jbe
                pc = 0x1bbe9b;
                continue;
            }
            0x1bbe9b => {
                m.write32(esp.wrapping_add(0x54), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(0x475C78));
                m.write32(esp.wrapping_add(0x4C), x[0].lanes[0]);
                x[0].lanes[0] =
                    sse::divss(1.0, xsqrt::ss(fl(m.read32(esp.wrapping_add(0x54))))).to_bits();
                x[0].lanes[0] =
                    sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x4C)))).to_bits();
                m.write32(esp.wrapping_add(0x5C), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x5C)));
                x[3] = x[0];
                x[3].lanes[0] = sse::mulss(fl(x[3].lanes[0]), fl(x[6].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x60), x[3].lanes[0]);
                x[4] = Xmm::scalar(m.read32(esp.wrapping_add(0x60)));
                x[3] = x[0];
                x[0].lanes[0] =
                    sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x34)))).to_bits();
                x[3].lanes[0] = sse::mulss(fl(x[3].lanes[0]), fl(x[2].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x64), x[3].lanes[0]);
                x[5] = x[3];
                m.write32(esp.wrapping_add(0x68), x[0].lanes[0]);
                x[3] = x[0];
                pc = 0x1bbef9;
                continue;
            }
            0x1bbef9 => {
                ebp = m.read32(esi.wrapping_add(4));
                cmp!(ebp, m.read32(esp.wrapping_add(0xF4)));
                if fa != fb {
                    pc = 0x1bbf3a;
                    continue;
                } // jne
                pc = 0x1bbf05;
                continue;
            }
            0x1bbf05 => {
                push!(0);
                push!(0x80000);
                push!(esi);
                call!(0x17510);
                pc = 0x1bbf12;
                continue;
            }
            0x1bbf12 => {
                x[2] = Xmm::default();
                source!(0xC);
                esp = esp.wrapping_add(0xC);
                result!(esp);
                cmp!(eax, 0);
                if fa != fb {
                    pc = 0x1bbf6d;
                    continue;
                } // jne
                pc = 0x1bbf1c;
                continue;
            }
            0x1bbf1c => {
                x[6] = Xmm::scalar(m.read32(esp.wrapping_add(0x2C)));
                x[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x78)));
                x[4] = Xmm::scalar(m.read32(esp.wrapping_add(0x60)));
                x[5] = Xmm::scalar(m.read32(esp.wrapping_add(0x64)));
                x[3] = Xmm::scalar(m.read32(esp.wrapping_add(0x68)));
                pc = 0x1bbf3a;
                continue;
            }
            0x1bbf3a => {
                eax = m.read32(esi.wrapping_add(0x2C));
                cmp8!(((eax >> 8) & 0xff), (4));
                if (fa & fb) == 0 {
                    pc = 0x1bbf94;
                    continue;
                } // je
                pc = 0x1bbf42;
                continue;
            }
            0x1bbf42 => {
                cmp!(ebp, 0);
                if fa == fb {
                    pc = 0x1bbf6d;
                    continue;
                } // je
                pc = 0x1bbf46;
                continue;
            }
            0x1bbf46 => {
                x[3].lanes[0] =
                    sse::mulss(fl(x[3].lanes[0]), fl(m.read32(esp.wrapping_add(0x74)))).to_bits();
                x[5].lanes[0] =
                    sse::mulss(fl(x[5].lanes[0]), fl(m.read32(esp.wrapping_add(0x70)))).to_bits();
                x[4].lanes[0] =
                    sse::mulss(fl(x[4].lanes[0]), fl(m.read32(esp.wrapping_add(0x6C)))).to_bits();
                x[3].lanes[0] = sse::addss(fl(x[3].lanes[0]), fl(x[5].lanes[0])).to_bits();
                x[3].lanes[0] = sse::addss(fl(x[3].lanes[0]), fl(x[4].lanes[0])).to_bits();
                fca = fl(x[3].lanes[0]) as f64;
                fcb = fl(m.read32(0x47D670)) as f64;
                if fca > fcb {
                    pc = 0x1bc01a;
                    continue;
                } // ja
                pc = 0x1bbf6d;
                continue;
            }
            0x1bbf6d => {
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                m.write32(esi.wrapping_add(0xB7C), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                m.write32(esi.wrapping_add(0xB80), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x1C)));
                pc = 0x1bc036;
                continue;
            }
            0x1bbf94 => {
                x[3].lanes[0] =
                    sse::mulss(fl(x[3].lanes[0]), fl(m.read32(esp.wrapping_add(0x74)))).to_bits();
                x[5].lanes[0] =
                    sse::mulss(fl(x[5].lanes[0]), fl(m.read32(esp.wrapping_add(0x70)))).to_bits();
                x[4].lanes[0] =
                    sse::mulss(fl(x[4].lanes[0]), fl(m.read32(esp.wrapping_add(0x6C)))).to_bits();
                x[3].lanes[0] = sse::addss(fl(x[3].lanes[0]), fl(x[5].lanes[0])).to_bits();
                x[3].lanes[0] = sse::addss(fl(x[3].lanes[0]), fl(x[4].lanes[0])).to_bits();
                fca = fl(x[3].lanes[0]) as f64;
                fcb = fl(m.read32(0x47D670)) as f64;
                if fca > fcb {
                    pc = 0x1bc01a;
                    continue;
                } // ja
                pc = 0x1bbfb7;
                continue;
            }
            0x1bbfb7 => {
                x[0] = Xmm::scalar(m.read32(0x47D55C));
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(x[1].lanes[0]) as f64;
                if fca > fcb {
                    pc = 0x1bc01a;
                    continue;
                } // ja
                pc = 0x1bbfc4;
                continue;
            }
            0x1bbfc4 => {
                eax = m.read32(ebx.wrapping_add(4));
                push!(eax);
                call!(0xdc270);
                pc = 0x1bbfcd;
                continue;
            }
            0x1bbfcd => {
                source!(4);
                esp = esp.wrapping_add(4);
                result!(esp);
                cmp8!(((eax >> 8) & 0xff), (0xE));
                if (fa & fb) == 0 {
                    pc = 0x1bbffc;
                    continue;
                } // je
                pc = 0x1bbfd5;
                continue;
            }
            0x1bbfd5 => {
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                x[2] = Xmm::default();
                m.write32(esi.wrapping_add(0xB7C), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                m.write32(esi.wrapping_add(0xB80), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x1C)));
                pc = 0x1bc036;
                continue;
            }
            0x1bbffc => {
                push!(0);
                push!(1);
                push!(esi);
                call!(0x174e0);
                pc = 0x1bc006;
                continue;
            }
            0x1bc006 => {
                source!(0xC);
                esp = esp.wrapping_add(0xC);
                result!(esp);
                cmp!(eax, 0);
                if fa != fb {
                    pc = 0x1bbfd5;
                    continue;
                } // jne
                pc = 0x1bc00d;
                continue;
            }
            0x1bc00d => {
                ebp = m.read32(esp);
                esp = esp.wrapping_add(4);
                edi = m.read32(esp);
                esp = esp.wrapping_add(4);
                esi = m.read32(esp);
                esp = esp.wrapping_add(4);
                eax = 0;
                result!(eax);
                ebx = m.read32(esp);
                esp = esp.wrapping_add(4);
                source!(0xE0);
                esp = esp.wrapping_add(0xE0);
                result!(esp);
                break 'run;
            }
            0x1bc01a => {
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                m.write32(esi.wrapping_add(0xB80), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                m.write32(esi.wrapping_add(0xB7C), x[6].lanes[0]);
                pc = 0x1bc036;
                continue;
            }
            0x1bc036 => {
                x[5] = Xmm::scalar(m.read32(esi.wrapping_add(0xB7C)));
                cmp!(m.read32(esp.wrapping_add(0xFC)), 1);
                ebp = m.read32(esp.wrapping_add(0x58));
                m.write32(esi.wrapping_add(0xB84), x[0].lanes[0]);
                m.write32(esp.wrapping_add(0x14), x[5].lanes[0]);
                x[7] = Xmm::scalar(m.read32(esi.wrapping_add(0xB80)));
                m.write32(esp.wrapping_add(0x18), x[7].lanes[0]);
                x[4] = Xmm::scalar(m.read32(esi.wrapping_add(0xB84)));
                m.write32(esp.wrapping_add(0x1C), x[4].lanes[0]);
                if fa != fb {
                    pc = 0x1bc08b;
                    continue;
                } // jne
                pc = 0x1bc076;
                continue;
            }
            0x1bc076 => {
                eax = m.read32(ebp.wrapping_add(0x1C));
                cmp!(eax, 0);
                if fa == fb {
                    pc = 0x1bc08b;
                    continue;
                } // je
                pc = 0x1bc07d;
                continue;
            }
            0x1bc07d => {
                cmp!(eax, m.read32(ebp.wrapping_add(0x14)));
                if fa == fb {
                    pc = 0x1bc08b;
                    continue;
                } // je
                pc = 0x1bc082;
                continue;
            }
            0x1bc082 => {
                m.write32(esp.wrapping_add(0x3C), eax);
                eax = m.read32(ebp.wrapping_add(0x20));
                pc = 0x1bc08f;
                continue;
            }
            0x1bc08b => {
                eax = m.read32(esp.wrapping_add(0x10));
                pc = 0x1bc08f;
                continue;
            }
            0x1bc08f => {
                cmp8!((m.read8(0x7DE455) as u32), (0xA));
                if fa != fb {
                    pc = 0x1bc0a1;
                    continue;
                } // jne
                pc = 0x1bc098;
                continue;
            }
            0x1bc098 => {
                cmp!(eax, 0);
                if fa == fb {
                    pc = 0x1bc0cb;
                    continue;
                } // je
                pc = 0x1bc09c;
                continue;
            }
            0x1bc09c => {
                cmp!(eax, 1);
                if fa == fb {
                    pc = 0x1bc0cb;
                    continue;
                } // je
                pc = 0x1bc0a1;
                continue;
            }
            0x1bc0a1 => {
                ecx = m.read32(esp.wrapping_add(0x38));
                edx = m.read32(esp.wrapping_add(0x3C));
                push!(eax);
                push!(ecx);
                push!(edx);
                push!(edi);
                push!(esi);
                call!(0x1b6790);
                pc = 0x1bc0b3;
                continue;
            }
            0x1bc0b3 => {
                x[4] = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                x[7] = Xmm::scalar(m.read32(esp.wrapping_add(0x2C)));
                x[5] = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                x[2] = Xmm::default();
                source!(0x14);
                esp = esp.wrapping_add(0x14);
                result!(esp);
                pc = 0x1bc0cb;
                continue;
            }
            0x1bc0cb => {
                eax = m.read32(ebx.wrapping_add(8));
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x10), 0);
                if fas <= fbs {
                    pc = 0x1bc465;
                    continue;
                } // jle
                pc = 0x1bc0de;
                continue;
            }
            0x1bc0de => {
                eax = m.read32(esp.wrapping_add(0xFC));
                {
                    let product = (eax as i32 as i64) * (0x7C as u32 as i32 as i64);
                    eax = product as u32;
                }
                ecx = eax.wrapping_add(ebp).wrapping_add(0x68);
                ebp = m.read32(esp.wrapping_add(0x10));
                m.write32(esp.wrapping_add(0x50), ecx);
                pc = 0x1bc110;
                continue;
            }
            0x1bc0f6 => {
                x[4] = Xmm::scalar(m.read32(esp.wrapping_add(0x1C)));
                x[7] = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                x[5] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                x[2] = Xmm::default();
                pc = 0x1bc110;
                continue;
            }
            0x1bc110 => {
                cmp!(m.read32(esi.wrapping_add(0x2C)), 0x20000000);
                if (fa & fb) == 0 {
                    pc = 0x1bc130;
                    continue;
                } // je
                pc = 0x1bc119;
                continue;
            }
            0x1bc119 => {
                m.write32(esp.wrapping_add(0x20), x[5].lanes[0]);
                m.write32(esp.wrapping_add(0x24), x[7].lanes[0]);
                m.write32(esp.wrapping_add(0x28), x[4].lanes[0]);
                pc = 0x1bc30d;
                continue;
            }
            0x1bc130 => {
                x[0] = Xmm::scalar(m.read32(esi.wrapping_add(0x9FC)));
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(x[2].lanes[0]) as f64;
                eax = (eax & 0xffff_00ff)
                    | ((((if fca == fcb || fca != fca || fcb != fcb {
                        0x40u32
                    } else {
                        0
                    }) | (if fca != fca || fcb != fcb { 4 } else { 0 })
                        | (if fca < fcb || fca != fca || fcb != fcb {
                            1
                        } else {
                            0
                        })
                        | 2) as u32)
                        << 8);
                cmp8!(((eax >> 8) & 0xff), (0x44));
                if !parity8(fa & fb) {
                    pc = 0x1bc250;
                    continue;
                } // jnp
                pc = 0x1bc145;
                continue;
            }
            0x1bc145 => {
                x[0] = Xmm::scalar(m.read32(esi.wrapping_add(0xAF4)));
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(m.read32(0x47E2B8)) as f64;
                if !(fca > fcb) {
                    pc = 0x1bc530;
                    continue;
                } // jbe
                pc = 0x1bc15a;
                continue;
            }
            0x1bc15a => {
                eax = m.read32(esi.wrapping_add(0x204));
                edx = eax.wrapping_add(eax.wrapping_mul(8));
                cmp!(m.read32(edx.wrapping_mul(4).wrapping_add(0x4ECA78)), 1);
                if fa != fb {
                    pc = 0x1bc530;
                    continue;
                } // jne
                pc = 0x1bc171;
                continue;
            }
            0x1bc171 => {
                x[0] = Xmm::scalar(m.read32(esi.wrapping_add(0x9FC)));
                m.write32(esp.wrapping_add(0x78), x[0].lanes[0]);
                eax = m.read32(esp.wrapping_add(0x78));
                x[0] = Xmm::scalar(m.read32(0x475D1C));
                eax = (eax & 0x7FFFFFFF);
                result!(eax);
                m.write32(esp.wrapping_add(0x4C), eax);
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(m.read32(esp.wrapping_add(0x4C))) as f64;
                if !(fca > fcb) {
                    pc = 0x1bc530;
                    continue;
                } // jbe
                pc = 0x1bc19f;
                continue;
            }
            0x1bc19f => {
                x[0] = Xmm::scalar(m.read32(0x47E2B4));
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(m.read32(esi.wrapping_add(0xAF4))) as f64;
                if !(fca > fcb) {
                    pc = 0x1bc530;
                    continue;
                } // jbe
                pc = 0x1bc1b4;
                continue;
            }
            0x1bc1b4 => {
                cmp8!((m.read8(0x7DE455) as u32), (0xA));
                if fa == fb {
                    pc = 0x1bc530;
                    continue;
                } // je
                pc = 0x1bc1c1;
                continue;
            }
            0x1bc1c1 => {
                eax = (eax & 0xffff_ff00) | ((m.read8(0x7DE457) as u32) & 0xff);
                cmp8!((eax & 0xff), (0));
                if fa == fb {
                    pc = 0x1bc530;
                    continue;
                } // je
                pc = 0x1bc1ce;
                continue;
            }
            0x1bc1ce => {
                x[1] = x[4];
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[0] = x[5];
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[1].lanes[0] = sse::subss(fl(x[1].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[3] = x[7];
                x[3].lanes[0] = sse::mulss(fl(x[3].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[2] = x[1];
                x[2].lanes[0] = sse::mulss(fl(x[2].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[6] = x[3];
                x[6].lanes[0] = sse::subss(fl(x[6].lanes[0]), fl(x[4].lanes[0])).to_bits();
                x[0] = x[5];
                x[0].lanes[0] = sse::subss(fl(x[0].lanes[0]), fl(x[3].lanes[0])).to_bits();
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[1] = x[6];
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[6].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(m.read32(0x4765F4)) as f64;
                if !(fca > fcb) {
                    pc = 0x1bc250;
                    continue;
                } // jbe
                pc = 0x1bc218;
                continue;
            }
            0x1bc218 => {
                m.write32(esp.wrapping_add(0x5C), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(0x475C78));
                m.write32(esp.wrapping_add(0x54), x[0].lanes[0]);
                x[0].lanes[0] =
                    sse::divss(1.0, xsqrt::ss(fl(m.read32(esp.wrapping_add(0x5C))))).to_bits();
                x[0].lanes[0] =
                    sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x54)))).to_bits();
                m.write32(esp.wrapping_add(0x4C), x[0].lanes[0]);
                x[4] = Xmm::scalar(m.read32(esp.wrapping_add(0x1C)));
                x[7] = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                x[5] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                pc = 0x1bc250;
                continue;
            }
            0x1bc250 => {
                m.write32(esp.wrapping_add(0x20), x[5].lanes[0]);
                m.write32(esp.wrapping_add(0x24), x[7].lanes[0]);
                m.write32(esp.wrapping_add(0x28), x[4].lanes[0]);
                pc = 0x1bc262;
                continue;
            }
            0x1bc262 => {
                call!(0x153b40);
                pc = 0x1bc267;
                continue;
            }
            0x1bc267 => {
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x4C), eax);
                fpu.push(m.read32(esp.wrapping_add(0x4C)) as i32 as f64);
                if fas >= fbs {
                    pc = 0x1bc279;
                    continue;
                } // jge
                pc = 0x1bc273;
                continue;
            }
            0x1bc273 => {
                {
                    let v = st(&fpu, 0) + fl(m.read32(0x475CCC)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                pc = 0x1bc279;
                continue;
            }
            0x1bc279 => {
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x4785D0)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 0) * fl(m.read32(ebx.wrapping_add(0x20))) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                fpu.push(fl(m.read32(ebx.wrapping_add(0x20))) as f64);
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x475CD4)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 1) - st(&fpu, 0);
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 1, v);
                }
                fpu.pop();
                {
                    let v = st(&fpu, 0) + fl(m.read32(esp.wrapping_add(0x20))) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                m.write32(esp.wrapping_add(0x20), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                call!(0x153b40);
                pc = 0x1bc29a;
                continue;
            }
            0x1bc29a => {
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x4C), eax);
                fpu.push(m.read32(esp.wrapping_add(0x4C)) as i32 as f64);
                if fas >= fbs {
                    pc = 0x1bc2ac;
                    continue;
                } // jge
                pc = 0x1bc2a6;
                continue;
            }
            0x1bc2a6 => {
                {
                    let v = st(&fpu, 0) + fl(m.read32(0x475CCC)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                pc = 0x1bc2ac;
                continue;
            }
            0x1bc2ac => {
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x4785D0)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 0) * fl(m.read32(ebx.wrapping_add(0x20))) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                fpu.push(fl(m.read32(ebx.wrapping_add(0x20))) as f64);
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x475CD4)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 1) - st(&fpu, 0);
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 1, v);
                }
                fpu.pop();
                {
                    let v = st(&fpu, 0) + fl(m.read32(esp.wrapping_add(0x24))) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                m.write32(esp.wrapping_add(0x24), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                call!(0x153b40);
                pc = 0x1bc2cd;
                continue;
            }
            0x1bc2cd => {
                cmp!(eax, 0);
                m.write32(esp.wrapping_add(0x4C), eax);
                fpu.push(m.read32(esp.wrapping_add(0x4C)) as i32 as f64);
                if fas >= fbs {
                    pc = 0x1bc2df;
                    continue;
                } // jge
                pc = 0x1bc2d9;
                continue;
            }
            0x1bc2d9 => {
                {
                    let v = st(&fpu, 0) + fl(m.read32(0x475CCC)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                pc = 0x1bc2df;
                continue;
            }
            0x1bc2df => {
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x4785D0)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                x[7] = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                x[5] = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                {
                    let v = st(&fpu, 0) * fl(m.read32(ebx.wrapping_add(0x20))) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                fpu.push(fl(m.read32(ebx.wrapping_add(0x20))) as f64);
                {
                    let v = st(&fpu, 0) * fl(m.read32(0x475CD4)) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                {
                    let v = st(&fpu, 1) - st(&fpu, 0);
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 1, v);
                }
                fpu.pop();
                {
                    let v = st(&fpu, 0) + fl(m.read32(esp.wrapping_add(0x28))) as f64;
                    let v = fpc(&fpu, v);
                    set_st(&mut fpu, 0, v);
                }
                m.write32(esp.wrapping_add(0x28), (st(&fpu, 0) as f32).to_bits());
                fpu.pop();
                x[4] = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                pc = 0x1bc30d;
                continue;
            }
            0x1bc30d => {
                x[0] = x[4];
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[4].lanes[0])).to_bits();
                x[1] = x[7];
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[7].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[1] = x[5];
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[5].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[1].lanes[0])).to_bits();
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(m.read32(0x4765F4)) as f64;
                if !(fca > fcb) {
                    pc = 0x1bc39b;
                    continue;
                } // jbe
                pc = 0x1bc333;
                continue;
            }
            0x1bc333 => {
                m.write32(esp.wrapping_add(0x88), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(0x475C78));
                m.write32(esp.wrapping_add(0x90), x[0].lanes[0]);
                x[0].lanes[0] =
                    sse::divss(1.0, xsqrt::ss(fl(m.read32(esp.wrapping_add(0x88))))).to_bits();
                x[0].lanes[0] =
                    sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x90)))).to_bits();
                m.write32(esp.wrapping_add(0x8C), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x8C)));
                x[1] = x[0];
                x[1].lanes[0] =
                    sse::mulss(fl(x[1].lanes[0]), fl(m.read32(esp.wrapping_add(0x20)))).to_bits();
                m.write32(esp.wrapping_add(0x20), x[1].lanes[0]);
                x[1] = x[0];
                x[1].lanes[0] =
                    sse::mulss(fl(x[1].lanes[0]), fl(m.read32(esp.wrapping_add(0x24)))).to_bits();
                x[0].lanes[0] =
                    sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x28)))).to_bits();
                m.write32(esp.wrapping_add(0x24), x[1].lanes[0]);
                m.write32(esp.wrapping_add(0x28), x[0].lanes[0]);
                pc = 0x1bc39b;
                continue;
            }
            0x1bc39b => {
                eax = m.read32(esp.wrapping_add(0x50));
                cmp!(m.read32(eax), 0);
                if (fa.wrapping_sub(fb) >> 31) == 0 {
                    pc = 0x1bc3ab;
                    continue;
                } // jns
                pc = 0x1bc3a4;
                continue;
            }
            0x1bc3a4 => {
                eax = m.read32(esp.wrapping_add(0x58));
                source!(0x68);
                eax = eax.wrapping_add(0x68);
                result!(eax);
                pc = 0x1bc3ab;
                continue;
            }
            0x1bc3ab => {
                ecx = m.read32(eax.wrapping_add(4));
                push!(ecx);
                call!(0xdc250);
                pc = 0x1bc3b4;
                continue;
            }
            0x1bc3b4 => {
                source!(4);
                esp = esp.wrapping_add(4);
                result!(esp);
                cmp!(eax, 0);
                if fa == fb {
                    pc = 0x1bc42b;
                    continue;
                } // je
                pc = 0x1bc3bb;
                continue;
            }
            0x1bc3bb => {
                x[0] = Xmm::scalar(m.read32(0x475DB4));
                x[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                eax = m.read32(esp.wrapping_add(0xF4));
                ecx = m.read32(eax.wrapping_add(0x34));
                push!(0);
                push!(1);
                push!(0x2000058);
                push!(0);
                push!(0);
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[0].lanes[0])).to_bits();
                push!(0);
                m.write32(esp.wrapping_add(0x78), x[1].lanes[0]);
                x[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x3C)));
                push!(0);
                edx = esp.wrapping_add(0x7C);
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[0].lanes[0])).to_bits();
                push!(edx);
                m.write32(esp.wrapping_add(0x84), x[1].lanes[0]);
                x[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                push!(edi);
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[0].lanes[0])).to_bits();
                push!(ecx);
                m.write32(esp.wrapping_add(0x90), x[1].lanes[0]);
                call!(0x88c60);
                pc = 0x1bc420;
                continue;
            }
            0x1bc420 => {
                source!(0x28);
                esp = esp.wrapping_add(0x28);
                result!(esp);
                cmp!(eax, 0);
                if fa != fb {
                    pc = 0x1bc00d;
                    continue;
                } // jne
                pc = 0x1bc42b;
                continue;
            }
            0x1bc42b => {
                push!(0);
                push!(esi);
                push!(2);
                edx = esp.wrapping_add(0x2C);
                push!(edx);
                eax = edx;
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x108));
                ecx = edx;
                edx = m.read32(esp.wrapping_add(0x110));
                push!(ecx);
                push!(edi);
                push!(edi);
                push!(edi);
                push!(edx);
                push!(0);
                push!(edi);
                push!(eax);
                call!(0x1b7680);
                pc = 0x1bc456;
                continue;
            }
            0x1bc456 => {
                eax = m.read32(ebx.wrapping_add(8));
                source!(0x34);
                esp = esp.wrapping_add(0x34);
                result!(esp);
                ebp = ebp.wrapping_add(1);
                result!(ebp);
                fb = (fa == 0x8000_0000) as u32;
                cmp!(ebp, eax);
                if fas < fbs {
                    pc = 0x1bc0f6;
                    continue;
                } // jl
                pc = 0x1bc465;
                continue;
            }
            0x1bc465 => {
                push!(1);
                ecx = esp.wrapping_add(0x80);
                push!(ecx);
                push!(edi);
                call!(0x1b8530);
                pc = 0x1bc475;
                continue;
            }
            0x1bc475 => {
                edx = m.read32(esp.wrapping_add(0x10C));
                ecx = m.read32(esp.wrapping_add(0x108));
                push!(edx);
                eax = esp.wrapping_add(0x8C);
                push!(eax);
                push!(ecx);
                push!(edi);
                call!(0x1b6ff0);
                pc = 0x1bc493;
                continue;
            }
            0x1bc493 => {
                x[0].lanes[0] = (m.read32(ebx.wrapping_add(8)) as i32 as f32).to_bits();
                edx = m.read32(esi.wrapping_add(8));
                source!(0x18);
                esp = esp.wrapping_add(0x18);
                result!(esp);
                m.write32(esp, x[0].lanes[0]);
                push!(edx);
                push!(8);
                call!(0x238510);
                pc = 0x1bc4ab;
                continue;
            }
            0x1bc4ab => {
                source!(0xC68);
                esi = esi.wrapping_add(0xC68);
                result!(esi);
                push!(esi);
                call!(0x11f780);
                pc = 0x1bc4b7;
                continue;
            }
            0x1bc4b7 => {
                fpu.push(fl(m.read32(0x475C78)) as f64);
                source!(0x10);
                esp = esp.wrapping_add(0x10);
                result!(esp);
                fpcmp = fcmp(st(&fpu, 0), st(&fpu, 1));
                fpu.pop();
                fpu.pop();
                eax = (eax & 0xffff_00ff)
                    | ((((if fpcmp == 0 || fpcmp == 2 { 0x40u32 } else { 0 })
                        | (if fpcmp == 2 { 4 } else { 0 })
                        | (if fpcmp < 0 || fpcmp == 2 { 1 } else { 0 })
                        | 2) as u32)
                        << 8);
                cmp8!(((eax >> 8) & 0xff), (0x44));
                if !parity8(fa & fb) {
                    pc = 0x1bc4d4;
                    continue;
                } // jnp
                pc = 0x1bc4ca;
                continue;
            }
            0x1bc4ca => {
                cmp!(m.read32(edi.wrapping_add(0x30)), 0xA);
                if fa != fb {
                    pc = 0x1bba85;
                    continue;
                } // jne
                pc = 0x1bc4d4;
                continue;
            }
            0x1bc4d4 => {
                eax = m.read32(ebx.wrapping_add(0x68));
                cmp!(eax, 0xFFFFFFFF);
                if fa == fb {
                    pc = 0x1bc4f5;
                    continue;
                } // je
                pc = 0x1bc4dc;
                continue;
            }
            0x1bc4dc => {
                edx = m.read32(ebx.wrapping_add(0x6C));
                push!(0xFFFFFFFF);
                ecx = esp.wrapping_add(0x18);
                push!(ecx);
                push!(edi);
                push!(0xFFFFFFFF);
                push!(edx);
                push!(eax);
                push!(0);
                call!(0xdfec0);
                pc = 0x1bc4f2;
                continue;
            }
            0x1bc4f2 => {
                source!(0x1C);
                esp = esp.wrapping_add(0x1C);
                result!(esp);
                pc = 0x1bc4f5;
                continue;
            }
            0x1bc4f5 => {
                eax = m.read32(ebx.wrapping_add(0x74));
                cmp!(eax, 0);
                if fa == fb {
                    pc = 0x1bba85;
                    continue;
                } // je
                pc = 0x1bc500;
                continue;
            }
            0x1bc500 => {
                eax = m.read32(eax);
                cmp!(eax, 0);
                if fas < fbs {
                    pc = 0x1bba85;
                    continue;
                } // jl
                pc = 0x1bc50a;
                continue;
            }
            0x1bc50a => {
                esi = 0;
                result!(esi);
                pc = 0x1bc510;
                continue;
            }
            0x1bc510 => {
                ecx = m.read32(edi.wrapping_add(0xE8));
                cmp!(eax, m.read32(ecx.wrapping_add(0x1E8)));
                if fas >= fbs {
                    pc = 0x1bc613;
                    continue;
                } // jge
                pc = 0x1bc522;
                continue;
            }
            0x1bc522 => {
                ecx = m.read32(ecx.wrapping_add(0x1FC));
                eax = m.read32(ecx.wrapping_add(eax.wrapping_mul(4)));
                pc = 0x1bc618;
                continue;
            }
            0x1bc530 => {
                x[0] = x[7];
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[3] = x[0];
                x[1] = x[4];
                x[1].lanes[0] = sse::mulss(fl(x[1].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[6] = x[5];
                x[6].lanes[0] = sse::mulss(fl(x[6].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[1].lanes[0] = sse::subss(fl(x[1].lanes[0]), fl(x[6].lanes[0])).to_bits();
                x[2] = x[5];
                x[2].lanes[0] = sse::subss(fl(x[2].lanes[0]), fl(x[0].lanes[0])).to_bits();
                x[0] = x[2];
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[6] = x[1];
                x[6].lanes[0] = sse::mulss(fl(x[6].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[6].lanes[0])).to_bits();
                x[3].lanes[0] = sse::subss(fl(x[3].lanes[0]), fl(x[4].lanes[0])).to_bits();
                x[6] = x[3];
                x[6].lanes[0] = sse::mulss(fl(x[6].lanes[0]), fl(x[3].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[6].lanes[0])).to_bits();
                fca = fl(x[0].lanes[0]) as f64;
                fcb = fl(m.read32(0x4765F4)) as f64;
                if !(fca > fcb) {
                    pc = 0x1bc5d6;
                    continue;
                } // jbe
                pc = 0x1bc57d;
                continue;
            }
            0x1bc57d => {
                m.write32(esp.wrapping_add(0x38), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(0x475C78));
                m.write32(esp.wrapping_add(0x3C), x[0].lanes[0]);
                x[0].lanes[0] =
                    sse::divss(1.0, xsqrt::ss(fl(m.read32(esp.wrapping_add(0x38))))).to_bits();
                x[0].lanes[0] =
                    sse::mulss(fl(x[0].lanes[0]), fl(m.read32(esp.wrapping_add(0x3C)))).to_bits();
                m.write32(esp.wrapping_add(0x10), x[0].lanes[0]);
                x[0] = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                x[7] = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                x[5] = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                x[4] = x[0];
                x[4].lanes[0] = sse::mulss(fl(x[4].lanes[0]), fl(x[3].lanes[0])).to_bits();
                x[3] = x[4];
                x[4] = x[0];
                x[4].lanes[0] = sse::mulss(fl(x[4].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[1] = x[4];
                x[4] = Xmm::scalar(m.read32(esp.wrapping_add(0x1C)));
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[2] = x[0];
                pc = 0x1bc5d6;
                continue;
            }
            0x1bc5d6 => {
                x[0] = Xmm::scalar(m.read32(esi.wrapping_add(0x9FC)));
                x[6] = x[0];
                x[6].lanes[0] = sse::mulss(fl(x[6].lanes[0]), fl(x[3].lanes[0])).to_bits();
                x[3] = x[0];
                x[3].lanes[0] = sse::mulss(fl(x[3].lanes[0]), fl(x[1].lanes[0])).to_bits();
                x[0].lanes[0] = sse::mulss(fl(x[0].lanes[0]), fl(x[2].lanes[0])).to_bits();
                x[6].lanes[0] = sse::addss(fl(x[6].lanes[0]), fl(x[5].lanes[0])).to_bits();
                x[3].lanes[0] = sse::addss(fl(x[3].lanes[0]), fl(x[7].lanes[0])).to_bits();
                x[0].lanes[0] = sse::addss(fl(x[0].lanes[0]), fl(x[4].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x20), x[6].lanes[0]);
                m.write32(esp.wrapping_add(0x24), x[3].lanes[0]);
                m.write32(esp.wrapping_add(0x28), x[0].lanes[0]);
                pc = 0x1bc262;
                continue;
            }
            0x1bc613 => {
                eax = m.read32(0x7AD288);
                pc = 0x1bc618;
                continue;
            }
            0x1bc618 => {
                push!(eax);
                call!(0x10e700);
                pc = 0x1bc61e;
                continue;
            }
            0x1bc61e => {
                edx = m.read32(ebx.wrapping_add(0x74));
                eax = m.read32(esi.wrapping_add(edx).wrapping_add(4));
                source!(4);
                esi = esi.wrapping_add(4);
                result!(esi);
                source!(4);
                esp = esp.wrapping_add(4);
                result!(esp);
                cmp!(eax, 0);
                if fas >= fbs {
                    pc = 0x1bc510;
                    continue;
                } // jge
                pc = 0x1bc633;
                continue;
            }
            0x1bc633 => {
                ebp = m.read32(esp);
                esp = esp.wrapping_add(4);
                edi = m.read32(esp);
                esp = esp.wrapping_add(4);
                esi = m.read32(esp);
                esp = esp.wrapping_add(4);
                eax = 1;
                ebx = m.read32(esp);
                esp = esp.wrapping_add(4);
                source!(0xE0);
                esp = esp.wrapping_add(0xE0);
                result!(esp);
                break 'run;
            }
            0x1bc643 => {
                edi = m.read32(esp);
                esp = esp.wrapping_add(4);
                esi = m.read32(esp);
                esp = esp.wrapping_add(4);
                eax = 0;
                result!(eax);
                ebx = m.read32(esp);
                esp = esp.wrapping_add(4);
                source!(0xE0);
                esp = esp.wrapping_add(0xE0);
                result!(esp);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    let _ = (fca, fcb, fpcmp, cf, fa, fb, fas, fbs);
    *regs = ExtRegisters {
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
        xmm: [x[0], x[1], x[2], x[3], x[4]],
        xmm_hi: [x[5], x[6], x[7]],
    };
}
