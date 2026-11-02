// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-lane-c2r/lib
//! T3785: capture Tier 2 roots (engine/object) moved out of the frozen tsfp-ports facade `c2r::t2049_exec_003`
//! (rule 37); `tsfp_ports::c2r::t2049_exec_003` re-exports every function at its old path.
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

/// `game_frame_update_prelude_rand_state_hud_match_tick_then_visibility_and_object_lists`: original 0x1CE2E0 (engine/object), lifted `sub_001CE2E0`.
/// Calls through the `GuestCall` trampoline to the retained original: 0x153C00 `game_rand_state_save_or_restore`, 0xFE000 `game_hud_overlay_players_update_per_frame_palette_blend_and_level`, 0x3F050 `game_texture_stream_block_load`, 0xC7950 `game_player_slots_reset_field_0x3b8_to_one_and_clear`, 0x68830 `game_match_state_machine_tick_dispatch_by_7de30c_jump_table_12_states`, 0xA0130 `game_object_pool_build_update_list_and_mark_bitmap` and 26 more.
/// Register convention: `ExtRegisters` in and out, cdecl stack arguments read from guest memory.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_parens,
    clippy::all
)]
pub fn game_frame_update_prelude_rand_state_hud_match_tick_then_visibility_and_object_lists<
    M: GuestMemory,
    C: GuestCall,
>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    let (mut jt,) = (0,); // switch target
    r.eax = m.read32(0x4e7944);
    r.push(m, r.eax);
    r.call(guest, 0x153c00);
    r.release_stack(4);
    r.call(guest, 0xfe000);
    r.call(guest, 0x3f050);
    r.call(guest, 0xc7950);
    r.call(guest, 0x68830);
    r.eax = gs::get_level(m).wrapping_add(0xffffff9bu32);
    if r.eax > 6 {
        r.call(guest, 0x116410);
        if (gs::get_flags(m) & 0x20000000) == 0 {
            r.call(guest, 0x347cd0);
            r.call(guest, 0x9e4b0);
            r.call(guest, 0x11cb30);
            r.call(guest, 0x897d0);
            r.call(guest, 0x1d1960);
            r.call(guest, 0xc9c20);
            r.call(guest, 0x187580);
            r.call(guest, 0x46350);
            r.call(guest, 0x46610);
            r.call(guest, 0x164e00);
            r.call(guest, 0x1651c0);
            r.call(guest, 0xa0130);
            r.call(guest, 0x158c80);
            r.call(guest, 0xa7670);
            r.call(guest, 0x15e830);
            r.call(guest, 0xdc7b0);
            r.call(guest, 0x1154e0);
            r.call(guest, 0xf6d50);
            r.call(guest, 0x3493d0);
            r.call(guest, 0xf7b10);
            r.call(guest, 0x259930);
            r.call(guest, 0x389b0);
            r.eax = gs::get_flags(m);
            if (((((r.eax >> 8) as u8) as u32) & 0xff) & (2 & 0xff)) == 0 {
                return;
            }
            r.tail_call(guest, 0x99ee0); // direct tail jump
            return;
        } else {
            r.call(guest, 0x347cd0);
            r.call(guest, 0x9e4b0);
            r.call(guest, 0x897d0);
            r.call(guest, 0xa0130);
            r.call(guest, 0xa7670);
            r.call(guest, 0x99990);
            r.tail_call(guest, 0xdc7b0); // direct tail jump
            return;
        }
    } else {
        jt = m.read32(r.eax.wrapping_mul(4).wrapping_add(0x1ce404));
        if jt == 0x1ce316 {
            r.call(guest, 0xa0130);
            r.call(guest, 0xa7670);
            r.call(guest, 0x2d2930);
            r.call(guest, 0x46350);
            r.tail_call(guest, 0x46610); // direct tail jump
            return;
        } else {
            if jt == 0x1ce32f {
                r.call(guest, 0xa0130);
                r.call(guest, 0xa7670);
                r.call(guest, 0x2fb4c0);
                r.tail_call(guest, 0x46350); // direct tail jump
                return;
            } else {
                if jt == 0x1ce343 {
                    r.call(guest, 0xa0130);
                    r.tail_call(guest, 0xa7670); // direct tail jump
                    return;
                }
                if jt == 0x1ce34d {
                    r.tail_call(guest, 0x2d2930); // direct tail jump
                    return;
                }
                if jt == 0x1ce403 {
                    return;
                }
                r.tail_call(guest, jt); // indirect tail jump, target outside the switch
                return;
            }
        }
    }
}

pub const ROOTS: [u32; 1] = [0x1ce2e0];

#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x1ce2e0 => {
            game_frame_update_prelude_rand_state_hud_match_tick_then_visibility_and_object_lists(
                m, guest, regs,
            );
            true
        }
        _ => false,
    }
}
