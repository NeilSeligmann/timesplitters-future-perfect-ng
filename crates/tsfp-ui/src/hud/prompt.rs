// SPDX-License-Identifier: GPL-3.0-or-later
//! Small HUD gating predicates.

use crate::hud::addr::*;
use crate::memory::GuestMemory;

/// 0x42F90 `game_hud_prompt_allowed_by_flags_7de458_and_player_field_0x114`. cdecl(0). Prompts are
/// blocked when any of mode flags 0xE0000200 is set, or in a mode at or below 0x64 while the
/// current player's `+0x114` is 0x100.
// portreg: va=0x00042f90 tier=1
// portreg: mirrors=tsfp-ports/hud_lane
// portreg: mirrors=tsfp-ports/c2r/ui_hud
pub fn prompt_allowed<M: GuestMemory>(m: &M) -> u32 {
    if m.read32(MODE_FLAGS) & 0xe000_0200 != 0 {
        return 0;
    }
    if (m.read32(MODE_WORD) as i32) > 0x64 {
        return 1;
    }
    let player = m.read32(CURRENT_PLAYER_PTR);
    if player == 0 {
        return 1;
    }
    u32::from(m.read32(player.wrapping_add(0x114)) != 0x100)
}

/// 0x139160 `game_hud_local_player_slot_state_not_0xc_or_0x11_and_flag_0x40_clear`. cdecl(0).
/// The local viewer record is `[0x7B0C7C]`: `+4` player index, `+0xC` flag byte (bit 0x40 hides
/// the HUD). The player's HUD block state `+0` must not be 0xC or 0x11.
// portreg: va=0x00139160 tier=1
pub fn local_slot_visible<M: GuestMemory>(m: &M) -> u32 {
    let viewer = m.read32(0x007b_0c7c);
    let player = m.read32(viewer.wrapping_add(4)) as i32;
    if player < 0 {
        return 0;
    }
    let flags = m.read8(viewer.wrapping_add(0xc));
    if flags & 0x40 != 0 {
        return 0;
    }
    let state = m.read32(
        HUD_BLOCK_BASE.wrapping_add((player as u32).wrapping_mul(HUD_BLOCK_STRIDE)),
    );
    u32::from(state != 0x0c && state != 0x11)
}
