// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-lane-c2r/lib
//! T3785: capture Tier 2 roots (engine/anim) moved out of the frozen tsfp-ports facade `c2r::engine_anim`
//! (rule 37); `tsfp_ports::c2r::engine_anim` re-exports every function at its old path.
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

/// `game_table_7e1be0_get_by_index_via_map_7e03c0_range_0x25_0xc03`: original 0x56FC0 (engine/anim), lifted `sub_00056FC0`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_table_7e1be0_get_by_index_via_map_7e03c0_range_0x25_0xc03<M: GuestMemory>(
    m: &mut M,
    r: &mut ExtRegisters,
) {
    let eax_1: u32 = m.read32(r.esp.wrapping_add(4));
    if (eax_1 as i32) > 0i32 {
        if (eax_1 as i32) >= 0x25 {
            if (eax_1 as i32) < 0xc03i32 {
                r.eax = m.read32(
                    (((m.read16(eax_1.wrapping_mul(2).wrapping_add(0x7e03c0)) as i16) as i32)
                        as u32)
                        .wrapping_mul(4)
                        .wrapping_add(0x7e1be0),
                );
                return;
            }
        }
    }
    // 0x56fe4
    r.eax = 0;
}

/// `game_anim_set_entries_load_raw_files_and_store_slot_table_pair`: original 0x5B980 (engine/anim), lifted `sub_0005B980`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x3C85D4, 0x57F30.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_anim_set_entries_load_raw_files_and_store_slot_table_pair<
    M: GuestMemory,
    C: GuestCall,
>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    'at_5b9f8: {
        r.esp = r.esp.wrapping_sub(0x80);
        r.push(m, r.edi);
        r.edi = m.read32(r.esp.wrapping_add(0x90));
        if (r.edi as i32) > 0i32 {
            r.push(m, r.ebx);
            r.ebx = m.read32(r.esp.wrapping_add(0x8c));
            r.push(m, r.esi);
            r.esi = m.read32(r.esp.wrapping_add(0x94)).wrapping_add(0x10);
            loop {
                // 0x5b9a5
                r.eax = m.read32(r.esi.wrapping_add((0x10i32.wrapping_neg()) as u32));
                r.push(m, 0x47866c);
                r.push(m, r.eax);
                r.push(m, r.ebx);
                r.ecx = r.esp.wrapping_add(0x18);
                r.push(m, 0x478664);
                r.push(m, r.ecx);
                r.call(guest, 0x3c85d4);
                r.edx = m.read16(r.esi) as u16 as u32;
                r.push(m, 0);
                r.push(m, r.edx);
                r.eax = r.esp.wrapping_add(0x28);
                r.push(m, r.eax);
                r.call(guest, 0x57f30);
                r.edx = ((m.read16(
                    (m.read16(r.esi) as u16 as u32)
                        .wrapping_mul(2)
                        .wrapping_add(0x7e03c0),
                ) as i16) as i32) as u32;
                r.eax = m.read32(r.esi.wrapping_add((8i32.wrapping_neg()) as u32));
                m.write32(r.edx.wrapping_mul(8).wrapping_add(0x7de660), r.eax);
                r.ecx = m.read32(r.esi.wrapping_add((4i32.wrapping_neg()) as u32));
                r.release_stack(0x20);
                r.esi = r.esi.wrapping_add(0x18);
                r.edi = r.edi.wrapping_sub(1);
                m.write32(r.edx.wrapping_mul(8).wrapping_add(0x7de664), r.ecx);
                if r.edi == 0 {
                    r.esi = r.pop(m);
                    r.ebx = r.pop(m);
                    break 'at_5b9f8;
                }
            }
        }
    }
    // 0x5b9f8
    r.edi = r.pop(m);
    r.release_stack(0x80);
}

/// `game_table_7e1be0_entry_nth_slot_with_mask_bits_return_word_or_minus_1`: original 0x613D0 (engine/anim), lifted `sub_000613D0`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x56FC0 `game_table_7e1be0_get_by_index_via_map_7e03c0_range_0x25_0xc03`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_table_7e1be0_entry_nth_slot_with_mask_bits_return_word_or_minus_1<
    M: GuestMemory,
    C: GuestCall,
>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.eax = m.read32(r.esp.wrapping_add(4));
    r.push(m, r.eax);
    r.call(guest, 0x56fc0);
    r.release_stack(4);
    if r.eax != 0 {
        'at_6141b: {
            'at_61417: {
                r.push(m, r.ebx);
                r.ebx = m.read32(r.esp.wrapping_add(0x10));
                r.push(m, r.ebp);
                r.push(m, r.esi);
                let esi_1: u32 = ((m.read16(r.eax.wrapping_add(6)) as i16) as i32) as u32;
                r.ecx = 0;
                r.push(m, r.edi);
                if (esi_1 as i32) > 0i32 {
                    r.edx = r.eax.wrapping_add(0xa);
                    r.edi = m.read32(r.esp.wrapping_add(0x18));
                    loop {
                        // 0x61400
                        let ebp_1: u32 = set_lo16(0u32, m.read16(r.edx) as u32);
                        if ((((ebp_1 & r.edi) as u16) as u32) & 0xffff) != (0 & 0xffff) {
                            r.ebx = r.ebx.wrapping_sub(1);
                            if r.ebx == 0 {
                                break 'at_6141b;
                            }
                        }
                        // 0x6140f
                        r.ecx = r.ecx.wrapping_add(1);
                        r.edx = r.edx.wrapping_add(4);
                        if (r.ecx as i32) >= (esi_1 as i32) {
                            break 'at_61417;
                        }
                    }
                }
            }
            if r.ebx != 0 {
                r.edi = r.pop(m);
                r.esi = r.pop(m);
                r.ebp = r.pop(m);
                r.eax = r.eax | 0xffffffffu32;
                r.ebx = r.pop(m);
                return;
            } else {
            }
        }
        // 0x6141b
        r.eax = m.read16(r.eax.wrapping_add(r.ecx.wrapping_mul(4)).wrapping_add(8)) as u16 as u32;
        r.edi = r.pop(m);
        r.esi = r.pop(m);
        r.ebp = r.pop(m);
        r.ebx = r.pop(m);
        return;
    } else {
        r.eax = r.eax | 0xffffffffu32;
        return;
    }
}

/// `game_anim_set_load_ts3_peekboxlft_set_a_51_entries`: original 0x1C9450 (engine/anim), lifted `sub_001C9450`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x5B980 `game_anim_set_entries_load_raw_files_and_store_slot_table_pair`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_anim_set_load_ts3_peekboxlft_set_a_51_entries<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.push(m, 0x33);
    r.push(m, 0x4805e8);
    r.push(m, 0x49c690);
    r.call(guest, 0x5b980);
    r.release_stack(0xc);
}

/// `game_anim_set_load_ts3_startflyby_27_entries`: original 0x1C96B0 (engine/anim), lifted `sub_001C96B0`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x5B980 `game_anim_set_entries_load_raw_files_and_store_slot_table_pair`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_anim_set_load_ts3_startflyby_27_entries<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.push(m, 0x1b);
    r.push(m, 0x48cd20);
    r.push(m, 0x49c690);
    r.call(guest, 0x5b980);
    r.release_stack(0xc);
}

/// `game_anim_node_key_world_point_via_quat_matrix`: original 0x257DC0 (engine/anim), lifted `sub_00257DC0`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x59320 `game_anim_decode_node_key_transform_scaled`, 0x1506B0 `game_quat_to_matrix4x4`, 0x151E80 `game_matrix4x4_transform_point_vec3_inplace`.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_anim_node_key_world_point_via_quat_matrix<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.esp = r.esp.wrapping_sub(0x6c);
    let edx_1: u32 = m.read32(r.esp.wrapping_add(0x78));
    r.push(m, r.esi);
    r.push(m, 0x3f800000);
    r.push(m, r.esp.wrapping_add(0x18));
    let eax_2: u32 = m.read32(r.esp.wrapping_add(0x7c));
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(eax_2.wrapping_add(8))).to_bits());
    let eax_1: u32 = m.read32(
        (((edx_1 as i32).wrapping_mul(0x2ci32)) as u32)
            .wrapping_add(eax_2)
            .wrapping_add(0x30),
    );
    r.xmm[0].set_f(0, r.xmm[0].f(0) * f32::from_bits(m.read32(0x4d1994)));
    r.edx = m.read32(0x7e3954);
    r.push(m, cvttss2si(r.xmm[0].f(0)));
    r.ecx = ((m.read16(eax_1.wrapping_mul(2).wrapping_add(0x7e03c0)) as i16) as i32) as u32;
    r.eax = m.read32(r.edx.wrapping_add(r.ecx.wrapping_mul(4)));
    r.push(m, 0);
    r.push(m, r.eax);
    r.call(guest, 0x59320);
    r.eax = m.read32(r.esp.wrapping_add(0x98));
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x28))).to_bits());
    r.esi = m.read32(r.esp.wrapping_add(0x9c));
    m.write32(r.eax, r.xmm[0].f(0).to_bits());
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x2c))).to_bits());
    m.write32(r.eax.wrapping_add(4), r.xmm[0].f(0).to_bits());
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x30))).to_bits());
    m.write32(r.eax.wrapping_add(8), r.xmm[0].f(0).to_bits());
    r.xmm[0] = Xmm::default();
    m.write32(r.esi, r.xmm[0].f(0).to_bits());
    m.write32(r.esi.wrapping_add(4), r.xmm[0].f(0).to_bits());
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(0x475c78)).to_bits());
    m.write32(r.esi.wrapping_add(8), r.xmm[0].f(0).to_bits());
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x34))).to_bits());
    m.write32(r.esp.wrapping_add(0x18), r.xmm[0].f(0).to_bits());
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x38))).to_bits());
    m.write32(r.esp.wrapping_add(0x1c), r.xmm[0].f(0).to_bits());
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x3c))).to_bits());
    r.ecx = r.esp.wrapping_add(0x18);
    r.push(m, r.ecx);
    r.edx = r.esp.wrapping_add(0x48);
    m.write32(r.esp.wrapping_add(0x24), r.xmm[0].f(0).to_bits());
    r.xmm[0] = Xmm::scalar(f32::from_bits(m.read32(r.esp.wrapping_add(0x44))).to_bits());
    r.push(m, r.edx);
    m.write32(r.esp.wrapping_add(0x2c), r.xmm[0].f(0).to_bits());
    r.call(guest, 0x1506b0);
    r.eax = r.esp.wrapping_add(0x4c);
    r.push(m, r.esi);
    r.push(m, r.eax);
    r.call(guest, 0x151e80);
    r.release_stack(0x24);
    r.esi = r.pop(m);
    r.release_stack(0x6c);
}

pub const ROOTS: [u32; 4] = [0x613d0, 0x1c9450, 0x1c96b0, 0x257dc0];

#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x613d0 => {
            game_table_7e1be0_entry_nth_slot_with_mask_bits_return_word_or_minus_1(m, guest, regs);
            true
        }
        0x1c9450 => {
            game_anim_set_load_ts3_peekboxlft_set_a_51_entries(m, guest, regs);
            true
        }
        0x1c96b0 => {
            game_anim_set_load_ts3_startflyby_27_entries(m, guest, regs);
            true
        }
        0x257dc0 => {
            game_anim_node_key_world_point_via_quat_matrix(m, guest, regs);
            true
        }
        _ => false,
    }
}
