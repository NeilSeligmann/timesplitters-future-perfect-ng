// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-lane-c2r/lib
//! T3785: capture Tier 2 roots (engine/math) moved out of the frozen tsfp-ports facade `c2r::t1937_math`
//! (rule 37); `tsfp_ports::c2r::t1937_math` re-exports every function at its old path.
#![allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
use super::rt::*;
#[allow(unused_imports)]
use crate::state_wire as gs;

/// `game_matrix_55fa00_set_from_ptr`: original 0x1C6F0 (engine/math), lifted `sub_0001C6F0`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x22520 `game_matrix_slot_store`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_matrix_55fa00_set_from_ptr<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.eax = m.read32(r.esp.wrapping_add(4));
    r.push(m, r.esi);
    r.push(m, r.edi);
    let esi_1: u32 = r.eax;
    let ecx_1: u32 = 0x10u32;
    let edi_1: u32 = 0x55fa00u32;
    r.push(m, 1);
    {
        let step: u32 = 4;
        for i in 0..ecx_1 {
            m.write32(
                edi_1.wrapping_add(i.wrapping_mul(step)),
                m.read32(esi_1.wrapping_add(i.wrapping_mul(step))),
            );
        }
        r.esi = esi_1.wrapping_add(ecx_1.wrapping_mul(step));
        r.edi = edi_1.wrapping_add(ecx_1.wrapping_mul(step));
    }
    r.ecx = 0;
    r.push(m, r.eax);
    m.write32(0x55fad0, 1);
    r.call(guest, 0x22520);
    r.release_stack(8);
    r.edi = r.pop(m);
    r.esi = r.pop(m);
}

/// `game_matrix_55fa40_set_from_ptr`: original 0x1C5D0 (engine/math), lifted `sub_0001C5D0`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x22520 `game_matrix_slot_store`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_matrix_55fa40_set_from_ptr<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.eax = m.read32(r.esp.wrapping_add(4));
    r.push(m, r.esi);
    r.push(m, r.edi);
    let esi_1: u32 = r.eax;
    let ecx_1: u32 = 0x10u32;
    let edi_1: u32 = 0x55fa40u32;
    r.push(m, 0);
    {
        let step: u32 = 4;
        for i in 0..ecx_1 {
            m.write32(
                edi_1.wrapping_add(i.wrapping_mul(step)),
                m.read32(esi_1.wrapping_add(i.wrapping_mul(step))),
            );
        }
        r.esi = esi_1.wrapping_add(ecx_1.wrapping_mul(step));
        r.edi = edi_1.wrapping_add(ecx_1.wrapping_mul(step));
    }
    r.ecx = 0;
    r.push(m, r.eax);
    m.write32(0x55fad0, 1);
    r.call(guest, 0x22520);
    r.release_stack(8);
    r.edi = r.pop(m);
    r.esi = r.pop(m);
}

/// `game_math_sin_f32`: original 0x12010 (engine/math), lifted `sub_00012010`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_math_sin_f32<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    r.fpu
        .push(f32::from_bits(m.read32(r.esp.wrapping_add(4))) as f64);
    r.fpu.set_st(0, sin(r.fpu.st(0)));
}

/// `game_math_cos_f32`: original 0x12000 (engine/math), lifted `sub_00012000`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_math_cos_f32<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    r.fpu
        .push(f32::from_bits(m.read32(r.esp.wrapping_add(4))) as f64);
    r.fpu.set_st(0, cos(r.fpu.st(0)));
}

/// `game_math_wrap_to_range_f32`: original 0x12540 (engine/math), lifted `sub_00012540`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x3C8501.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_math_wrap_to_range_f32<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.fpu
        .push(f32::from_bits(m.read32(r.esp.wrapping_add(0xc))) as f64);
    r.esp = r.esp.wrapping_sub(8);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) - (f32::from_bits(m.read32(r.esp.wrapping_add(0x10))) as f64),
        ),
    );
    m.write32(r.esp.wrapping_add(0x14), (r.fpu.st(0) as f32).to_bits());
    r.fpu.pop();
    r.fpu
        .push(f32::from_bits(m.read32(r.esp.wrapping_add(0xc))) as f64);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) - (f32::from_bits(m.read32(r.esp.wrapping_add(0x10))) as f64),
        ),
    );
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) / (f32::from_bits(m.read32(r.esp.wrapping_add(0x14))) as f64),
        ),
    );
    write64(m, r.esp, r.fpu.st(0).to_bits());
    r.fpu.pop();
    r.call(guest, 0x3c8501);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) * (f32::from_bits(m.read32(r.esp.wrapping_add(0x14))) as f64),
        ),
    );
    r.release_stack(8);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            (f32::from_bits(m.read32(r.esp.wrapping_add(4))) as f64) - r.fpu.st(0),
        ),
    );
}

/// `game_math_select_period_shifted_difference_f32`: original 0x12570 (engine/math), lifted `sub_00012570`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_math_select_period_shifted_difference_f32<M: GuestMemory>(
    m: &mut M,
    r: &mut ExtRegisters,
) {
    let (mut fca, mut fcb) = (0f64, 0f64); // operands of the last SSE compare
    'at_12659: {
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x10))).to_bits());
        r.xmm[1] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(8))).to_bits());
        r.xmm[0].set_f(
            0,
            r.xmm[0].f(0) - f32::from_bits(m.read32(r.esp.wrapping_add(0xc))),
        );
        r.xmm[2] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(4))).to_bits());
        r.xmm[3] = r.xmm[1];
        r.xmm[3].set_f(0, r.xmm[3].f(0) - r.xmm[2].f(0));
        r.xmm_hi[0] = r.xmm[0];
        m.write32(r.esp.wrapping_add(0x10), r.xmm[3].f(0).to_bits());
        let eax_1: u32 = m.read32(r.esp.wrapping_add(0x10));
        r.xmm_hi[0].set_f(0, r.xmm_hi[0].f(0) + r.xmm[2].f(0));
        r.xmm[4] = r.xmm[1];
        r.xmm[4].set_f(0, r.xmm[4].f(0) - r.xmm_hi[0].f(0));
        m.write32(r.esp.wrapping_add(0x10), r.xmm[4].f(0).to_bits());
        r.ecx = m.read32(r.esp.wrapping_add(0x10)) & 0x7fffffff;
        r.eax = eax_1 & 0x7fffffff;
        r.xmm[2].set_f(0, r.xmm[2].f(0) - r.xmm[0].f(0));
        m.write32(r.esp.wrapping_add(0x10), r.ecx);
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x10))).to_bits());
        m.write32(r.esp.wrapping_add(0xc), r.eax);
        fca = f32::from_bits(r.xmm[0].lanes[0]) as f64;
        fcb = f32::from_bits(m.read32(r.esp.wrapping_add(0xc))) as f64;
        r.xmm[1].set_f(0, r.xmm[1].f(0) - r.xmm[2].f(0));
        if (fca <= fcb) || ((fca != fca) || (fcb != fcb)) {
            m.write32(r.esp.wrapping_add(0x10), r.xmm[4].f(0).to_bits());
            let ecx_1: u32 = m.read32(r.esp.wrapping_add(0x10));
            m.write32(r.esp.wrapping_add(0x10), r.xmm[1].f(0).to_bits());
            r.edx = m.read32(r.esp.wrapping_add(0x10)) & 0x7fffffff;
            r.ecx = ecx_1 & 0x7fffffff;
            m.write32(r.esp.wrapping_add(0x10), r.edx);
            r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x10))).to_bits());
            m.write32(r.esp.wrapping_add(0xc), r.ecx);
            fcb = f32::from_bits(m.read32(r.esp.wrapping_add(0xc))) as f64;
            m.write32(r.esp.wrapping_add(0x10), r.xmm[4].f(0).to_bits());
            if (f32::from_bits(r.xmm[0].lanes[0]) as f64) > fcb {
                break 'at_12659;
            }
        } else {
            m.write32(r.esp.wrapping_add(0x10), r.xmm[3].f(0).to_bits());
            let edx_1: u32 = m.read32(r.esp.wrapping_add(0x10));
            m.write32(r.esp.wrapping_add(0x10), r.xmm[1].f(0).to_bits());
            r.eax = m.read32(r.esp.wrapping_add(0x10)) & 0x7fffffff;
            r.edx = edx_1 & 0x7fffffff;
            m.write32(r.esp.wrapping_add(0x10), r.eax);
            r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x10))).to_bits());
            m.write32(r.esp.wrapping_add(0xc), r.edx);
            fca = f32::from_bits(r.xmm[0].lanes[0]) as f64;
            fcb = f32::from_bits(m.read32(r.esp.wrapping_add(0xc))) as f64;
            if !((fca <= fcb) || ((fca != fca) || (fcb != fcb))) {
                m.write32(r.esp.wrapping_add(0x10), r.xmm[3].f(0).to_bits());
                r.fpu
                    .push(f32::from_bits(m.read32(r.esp.wrapping_add(0x10))) as f64);
                return;
            }
        }
        // 0x12653
        m.write32(r.esp.wrapping_add(0x10), r.xmm[1].f(0).to_bits());
    }
    // 0x12659
    r.fpu
        .push(f32::from_bits(m.read32(r.esp.wrapping_add(0x10))) as f64);
}

/// `game_math_sign_directed_period_shifted_step_and_floor_wrap_f32`: original 0x12720 (engine/math), lifted `sub_00012720`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x12570 `game_math_select_period_shifted_difference_f32`, 0x3C8501.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_math_sign_directed_period_shifted_step_and_floor_wrap_f32<
    M: GuestMemory,
    C: GuestCall,
>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.push(m, r.ecx);
    r.ecx = m.read32(r.esp.wrapping_add(0x14));
    r.edx = m.read32(r.esp.wrapping_add(0xc));
    r.push(m, m.read32(r.esp.wrapping_add(0x18)));
    r.eax = m.read32(r.esp.wrapping_add(0xc));
    r.push(m, r.ecx);
    r.push(m, r.edx);
    r.push(m, r.eax);
    r.call(guest, 0x12570);
    m.write32(r.esp.wrapping_add(0x10), (r.fpu.st(0) as f32).to_bits());
    r.fpu.pop();
    r.ecx = (m.read32(r.esp.wrapping_add(0x10)) & 0xbf800000u32) | 0x3f800000;
    m.write32(r.esp.wrapping_add(0x10), r.ecx);
    r.fpu
        .push(f32::from_bits(m.read32(r.esp.wrapping_add(0x10))) as f64);
    r.release_stack(8);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) * (f32::from_bits(m.read32(r.esp.wrapping_add(0x18))) as f64),
        ),
    );
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) + (f32::from_bits(m.read32(r.esp.wrapping_add(0x10))) as f64),
        ),
    );
    m.write32(r.esp.wrapping_add(0x18), (r.fpu.st(0) as f32).to_bits());
    r.fpu.pop();
    r.fpu
        .push(f32::from_bits(m.read32(r.esp.wrapping_add(0x20))) as f64);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) - (f32::from_bits(m.read32(r.esp.wrapping_add(0x1c))) as f64),
        ),
    );
    m.write32(r.esp.wrapping_add(0x10), (r.fpu.st(0) as f32).to_bits());
    r.fpu.pop();
    r.fpu
        .push(f32::from_bits(m.read32(r.esp.wrapping_add(0x18))) as f64);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) - (f32::from_bits(m.read32(r.esp.wrapping_add(0x1c))) as f64),
        ),
    );
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) / (f32::from_bits(m.read32(r.esp.wrapping_add(0x10))) as f64),
        ),
    );
    write64(m, r.esp, r.fpu.st(0).to_bits());
    r.fpu.pop();
    r.call(guest, 0x3c8501);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) * (f32::from_bits(m.read32(r.esp.wrapping_add(0x10))) as f64),
        ),
    );
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            (f32::from_bits(m.read32(r.esp.wrapping_add(0x18))) as f64) - r.fpu.st(0),
        ),
    );
    r.release_stack(0xc);
}

/// `game_matrix_55fa40_55fa00_set_from_camera_7b0cc0_fields_0x204_0x208`: original 0x56630 (engine/math), lifted `sub_00056630`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x1C5D0 `game_matrix_55fa40_set_from_ptr`, 0x1C6F0 `game_matrix_55fa00_set_from_ptr`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_matrix_55fa40_55fa00_set_from_camera_7b0cc0_fields_0x204_0x208<
    M: GuestMemory,
    C: GuestCall,
>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.eax = m.read32(0x7b0cc0);
    r.ecx = m.read32(r.eax.wrapping_add(0x204));
    r.push(m, r.ecx);
    r.call(guest, 0x1c5d0);
    r.edx = m.read32(0x7b0cc0);
    r.eax = m.read32(r.edx.wrapping_add(0x208));
    r.push(m, r.eax);
    r.call(guest, 0x1c6f0);
    r.release_stack(8);
}

/// `game_matrix4x4_make_from_three_axis_vec3`: original 0x152990 (engine/math), lifted `sub_00152990`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_matrix4x4_make_from_three_axis_vec3<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    r.eax = m.read32(r.esp.wrapping_add(4));
    let ecx_3: u32 = m.read32(r.esp.wrapping_add(8));
    let edx_3: u32 = m.read32(ecx_3);
    r.xmm[0] = Xmm::default();
    m.write32(r.eax, edx_3);
    let edx_2: u32 = m.read32(ecx_3.wrapping_add(4));
    m.write32(r.eax.wrapping_add(4), edx_2);
    m.write32(r.eax.wrapping_add(8), m.read32(ecx_3.wrapping_add(8)));
    let ecx_2: u32 = m.read32(r.esp.wrapping_add(0xc));
    m.write32(r.eax.wrapping_add(0xc), r.xmm[0].f(0).to_bits());
    m.write32(r.eax.wrapping_add(0x10), m.read32(ecx_2));
    let edx_1: u32 = m.read32(ecx_2.wrapping_add(4));
    m.write32(r.eax.wrapping_add(0x14), edx_1);
    m.write32(r.eax.wrapping_add(0x18), m.read32(ecx_2.wrapping_add(8)));
    let ecx_1: u32 = m.read32(r.esp.wrapping_add(0x10));
    m.write32(r.eax.wrapping_add(0x1c), r.xmm[0].f(0).to_bits());
    m.write32(r.eax.wrapping_add(0x20), m.read32(ecx_1));
    r.edx = m.read32(ecx_1.wrapping_add(4));
    m.write32(r.eax.wrapping_add(0x24), r.edx);
    r.ecx = m.read32(ecx_1.wrapping_add(8));
    m.write32(r.eax.wrapping_add(0x2c), r.xmm[0].f(0).to_bits());
    m.write32(r.eax.wrapping_add(0x30), r.xmm[0].f(0).to_bits());
    m.write32(r.eax.wrapping_add(0x34), r.xmm[0].f(0).to_bits());
    m.write32(r.eax.wrapping_add(0x38), r.xmm[0].f(0).to_bits());
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x475c78)).to_bits());
    m.write32(r.eax.wrapping_add(0x28), r.ecx);
    m.write32(r.eax.wrapping_add(0x3c), r.xmm[0].f(0).to_bits());
}

/// `game_math_two_angles_degrees_to_direction_vec3`: original 0x153C60 (engine/math), lifted `sub_00153C60`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x12010 `game_math_sin_f32`, 0x12000 `game_math_cos_f32`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_math_two_angles_degrees_to_direction_vec3<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.esp = r.esp.wrapping_sub(8);
    r.fpu
        .push(f32::from_bits(m.read32(r.esp.wrapping_add(0x14))) as f64);
    r.push(m, r.esi);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) * (f32::from_bits(m.read32(0x475d0c)) as f64),
        ),
    );
    m.write32(r.esp.wrapping_add(0x18), (r.fpu.st(0) as f32).to_bits());
    r.fpu.pop();
    r.esi = m.read32(r.esp.wrapping_add(0x18));
    r.push(m, r.esi);
    r.call(guest, 0x12010);
    m.write32(r.esp.wrapping_add(8), (r.fpu.st(0) as f32).to_bits());
    r.fpu.pop();
    r.push(m, r.esi);
    r.call(guest, 0x12000);
    m.write32(r.esp.wrapping_add(0x10), (r.fpu.st(0) as f32).to_bits());
    r.fpu.pop();
    r.fpu
        .push(f32::from_bits(m.read32(r.esp.wrapping_add(0x1c))) as f64);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) * (f32::from_bits(m.read32(0x475d0c)) as f64),
        ),
    );
    m.write32(r.esp.wrapping_add(0x20), (r.fpu.st(0) as f32).to_bits());
    r.fpu.pop();
    r.esi = m.read32(r.esp.wrapping_add(0x20));
    r.push(m, r.esi);
    r.call(guest, 0x12010);
    m.write32(r.esp.wrapping_add(0x24), (r.fpu.st(0) as f32).to_bits());
    r.fpu.pop();
    r.push(m, r.esi);
    r.call(guest, 0x12000);
    r.fpu
        .push(f32::from_bits(m.read32(r.esp.wrapping_add(0x14))) as f64);
    r.eax = m.read32(r.esp.wrapping_add(0x20));
    r.fpu.set_st(0, fp_pc(&r.fpu, r.fpu.st(0) * r.fpu.st(1)));
    r.ecx = m.read32(r.esp.wrapping_add(0x28));
    r.release_stack(0x10);
    m.write32(r.eax, (r.fpu.st(0) as f32).to_bits());
    r.fpu.pop();
    m.write32(r.eax.wrapping_add(4), r.ecx);
    r.esi = r.pop(m);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) * (f32::from_bits(m.read32(r.esp.wrapping_add(4))) as f64),
        ),
    );
    m.write32(r.eax.wrapping_add(8), (r.fpu.st(0) as f32).to_bits());
    r.fpu.pop();
    r.release_stack(8);
}

/// `game_float_step_toward_target_by_tick_scaled_rate_returns_arrived`: original 0x225DF0 (engine/math), lifted `sub_00225DF0`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_float_step_toward_target_by_tick_scaled_rate_returns_arrived<M: GuestMemory>(
    m: &mut M,
    r: &mut ExtRegisters,
) {
    let (mut fca, mut fcb) = (0f64, 0f64); // operands of the last SSE compare
    r.xmm[1].set_f(0, (m.read32(0x4e8cec) as i32) as f32);
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x475c78)).to_bits());
    let eax_1: u32 = m.read32(r.esp.wrapping_add(4));
    r.xmm[2] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(8))).to_bits());
    r.xmm[0].set_f(0, r.xmm[0].f(0) / r.xmm[1].f(0));
    r.xmm[0].set_f(0, r.xmm[0].f(0) * f32::from_bits(m.read32(0x4e794c)));
    r.xmm[1] = Xmm::scalar(f32::from_bits(m.read32(eax_1)).to_bits());
    r.xmm[0].set_f(
        0,
        r.xmm[0].f(0) * f32::from_bits(m.read32(r.esp.wrapping_add(0xc))),
    );
    r.xmm[1].set_f(0, r.xmm[1].f(0) - r.xmm[2].f(0));
    fca = f32::from_bits(r.xmm[1].lanes[0]) as f64;
    fcb = f32::from_bits(r.xmm[0].lanes[0]) as f64;
    if (fca <= fcb) || ((fca != fca) || (fcb != fcb)) {
        r.xmm[3] = Xmm::default();
        r.xmm[3].set_f(0, r.xmm[3].f(0) - r.xmm[0].f(0));
        fca = f32::from_bits(r.xmm[3].lanes[0]) as f64;
        fcb = f32::from_bits(r.xmm[1].lanes[0]) as f64;
        if (fca <= fcb) || ((fca != fca) || (fcb != fcb)) {
            m.write32(eax_1, r.xmm[2].f(0).to_bits());
            r.eax = 1;
            return;
        }
        r.xmm[1] = Xmm::scalar(f32::from_bits(m.read32(eax_1)).to_bits());
        r.xmm[1].set_f(0, r.xmm[1].f(0) + r.xmm[0].f(0));
        m.write32(eax_1, r.xmm[1].f(0).to_bits());
        r.eax = 0;
        return;
    } else {
        r.xmm[1] = Xmm::scalar(f32::from_bits(m.read32(eax_1)).to_bits());
        r.xmm[1].set_f(0, r.xmm[1].f(0) - r.xmm[0].f(0));
        m.write32(eax_1, r.xmm[1].f(0).to_bits());
        r.eax = 0;
        return;
    }
}

/// `game_float_scale_by_mode_byte_base_times_type_table_scale_when_flag_bit19`: original 0x350CF0 (engine/math), lifted `sub_00350CF0`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_float_scale_by_mode_byte_base_times_type_table_scale_when_flag_bit19<M: GuestMemory>(
    m: &mut M,
    r: &mut ExtRegisters,
) {
    r.push(m, r.ecx);
    let eax_2: u32 = set_lo8(r.eax, m.read8(0x7de457) as u32);
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x475c78)).to_bits());
    m.write32(r.esp, r.xmm[0].f(0).to_bits());
    if (((eax_2 as u8) as u32) & 0xff) == (0 & 0xff) {
        r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x4785e0)).to_bits());
        m.write32(r.esp, r.xmm[0].f(0).to_bits());
    }
    // 0x350d14
    r.eax = gs::get_flags(m);
    r.fpu.push(f32::from_bits(m.read32(r.esp)) as f64);
    if (r.eax & 0x80000) == 0 {
        r.fpu.set_st(
            0,
            fp_pc(
                &r.fpu,
                r.fpu.st(0) * (f32::from_bits(m.read32(0x47d1e4)) as f64),
            ),
        );
        r.ecx = r.pop(m);
        return;
    }
    let eax_1: u32 = m.read32(r.esp.wrapping_add(8));
    r.eax = eax_1.wrapping_add(eax_1.wrapping_mul(4));
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0)
                * (f32::from_bits(m.read32(r.eax.wrapping_mul(4).wrapping_add(0x53c4f8))) as f64),
        ),
    );
    r.ecx = r.pop(m);
}

pub const ROOTS: [u32; 8] = [
    0x12000, 0x12540, 0x12720, 0x56630, 0x152990, 0x153c60, 0x225df0, 0x350cf0,
];

#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x12000 => {
            game_math_cos_f32(m, regs);
            true
        }
        0x12540 => {
            game_math_wrap_to_range_f32(m, guest, regs);
            true
        }
        0x12720 => {
            game_math_sign_directed_period_shifted_step_and_floor_wrap_f32(m, guest, regs);
            true
        }
        0x56630 => {
            game_matrix_55fa40_55fa00_set_from_camera_7b0cc0_fields_0x204_0x208(m, guest, regs);
            true
        }
        0x152990 => {
            game_matrix4x4_make_from_three_axis_vec3(m, regs);
            true
        }
        0x153c60 => {
            game_math_two_angles_degrees_to_direction_vec3(m, guest, regs);
            true
        }
        0x225df0 => {
            game_float_step_toward_target_by_tick_scaled_rate_returns_arrived(m, regs);
            true
        }
        0x350cf0 => {
            game_float_scale_by_mode_byte_base_times_type_table_scale_when_flag_bit19(m, regs);
            true
        }
        _ => false,
    }
}
