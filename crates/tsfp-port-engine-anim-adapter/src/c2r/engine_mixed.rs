// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-lane-c2r/lib
//! Extracted capture Tier 2 roots from the T2049 c2r batches.
#![allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
use super::rt::*;
use crate::state_wire as gs;

/// `game_mode_79094c_is_0x65_or_0x66`: original 0x1CEBA0 (engine/render/scene), lifted `sub_001CEBA0`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_mode_79094c_is_0x65_or_0x66<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    let level: u32 = gs::get_level(m);
    if level != 0x65 {
        if level != 0x66 {
            r.eax = 0;
            return;
        }
    }
    // 0x1cebb2
    r.eax = 1;
}

/// `game_lipsync_apply_jaw_rotation_z_from_current_keyframe_to_node_matrix`: original 0x1CB350 (engine/physics), lifted `sub_001CB350`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x153490 `game_matrix4x4_make_rotation_z`, 0x153650 `game_matrix4x4_pre_multiply_in_place`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_lipsync_apply_jaw_rotation_z_from_current_keyframe_to_node_matrix<
    M: GuestMemory,
    C: GuestCall,
>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.esp = r.esp.wrapping_sub(0x40);
    r.push(m, r.esi);
    r.esi = m.read32(r.esp.wrapping_add(0x48));
    r.eax = m.read32(r.esi.wrapping_add(0xe90));
    if r.eax != 0 {
        r.eax = m.read32(r.esi);
        r.ecx = m.read32(r.eax.wrapping_add(4));
        r.edx = m.read32(r.ecx.wrapping_add(0x88));
        if m.read32(r.edx.wrapping_add(4)) == 0x7c {
            let eax_1: u32 = m.read32(r.esi.wrapping_add(0xe90));
            let ecx_1: u32 = m.read32(eax_1.wrapping_add(4));
            r.edx =
                m.read8(eax_1.wrapping_add(ecx_1.wrapping_mul(8)).wrapping_add(0x14)) as u8 as u32;
            r.xmm[1] = Xmm::scalar(
                f32::from_bits(
                    m.read32(eax_1.wrapping_add(ecx_1.wrapping_mul(8)).wrapping_add(0x18)),
                )
                .to_bits(),
            );
            r.xmm[1].set_f(
                0,
                r.xmm[1].f(0)
                    - f32::from_bits(
                        m.read32(eax_1.wrapping_add(ecx_1.wrapping_mul(8)).wrapping_add(0x10)),
                    ),
            );
            r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(eax_1.wrapping_add(8))).to_bits());
            r.xmm[0].set_f(
                0,
                r.xmm[0].f(0)
                    - f32::from_bits(
                        m.read32(eax_1.wrapping_add(ecx_1.wrapping_mul(8)).wrapping_add(0x10)),
                    ),
            );
            r.eax =
                m.read8(eax_1.wrapping_add(ecx_1.wrapping_mul(8)).wrapping_add(0x1c)) as u8 as u32;
            r.xmm[3] = Xmm::scalar(f32::from_bits(m.read32(0x475c78)).to_bits());
            r.xmm[0].set_f(0, r.xmm[0].f(0) / r.xmm[1].f(0));
            r.xmm[1] = Xmm::scalar(f32::from_bits(m.read32(0x475dcc)).to_bits());
            r.xmm[3].set_f(0, r.xmm[3].f(0) - r.xmm[0].f(0));
            r.xmm[2].set_f(0, (r.edx as i32) as f32);
            r.xmm[2].set_f(0, r.xmm[2].f(0) * r.xmm[1].f(0));
            r.xmm[2].set_f(0, r.xmm[2].f(0) * r.xmm[3].f(0));
            r.xmm[3].set_f(0, (r.eax as i32) as f32);
            r.xmm[3].set_f(0, r.xmm[3].f(0) * r.xmm[1].f(0));
            r.push(m, ecx_1);
            r.xmm[3].set_f(0, r.xmm[3].f(0) * r.xmm[0].f(0));
            r.xmm[2].set_f(0, r.xmm[2].f(0) + r.xmm[3].f(0));
            r.xmm[2].set_f(0, r.xmm[2].f(0) * f32::from_bits(m.read32(0x475cd4)));
            r.ecx = r.esp.wrapping_add(8);
            m.write32(r.esp, r.xmm[2].f(0).to_bits());
            r.push(m, r.ecx);
            r.call(guest, 0x153490);
            r.eax = m.read32(m.read32(r.esi).wrapping_add(4));
            r.edx = m.read32(r.eax.wrapping_add(0x8c));
            r.esi = m.read32(r.eax.wrapping_add(0x10));
            r.push(m, r.esp.wrapping_add(0xc));
            r.ecx = (m.read32(r.edx.wrapping_add(0x58)).wrapping_shl(6)).wrapping_add(r.esi);
            r.push(m, r.ecx);
            r.call(guest, 0x153650);
            r.release_stack(0x10);
        }
    }
    // 0x1cb415
    r.esi = r.pop(m);
    r.release_stack(0x40);
}

pub const ROOTS: [u32; 2] = [0x1ceba0, 0x1cb350];

#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x1ceba0 => {
            game_mode_79094c_is_0x65_or_0x66(m, regs);
            true
        }
        0x1cb350 => {
            game_lipsync_apply_jaw_rotation_z_from_current_keyframe_to_node_matrix(m, guest, regs);
            true
        }
        _ => false,
    }
}
