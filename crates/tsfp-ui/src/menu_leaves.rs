// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 3 ui (menu and hud) leaves. Each body ports a call-free C replacement of
//! `src/game`, see docs/t1836-rust-wave3.md. Reads and writes keep the C order.
use crate::{gprs::Gprs, memory::GuestMemory};

const HUD_FLAGS: u32 = 0x007d_e458;
const PLAYER_COUNT_WORD: u32 = 0x0079_094c;
const CURRENT_PLAYER: u32 = 0x007b_0c48;
const MENU_NIBBLE_WORD: u32 = 0x0075_6e58;
const MODE_BYTE: u32 = 0x007d_e455;
const ENTRY_TABLE: u32 = 0x004d_1f88;
const PLAYER_PROMPT_STATE: u32 = 0x114;
const PLAYER_PROMPT_HIDDEN: u32 = 0x100;
const PROMPT_BLOCKING_FLAGS: u32 = 0xe000_0200;
const MENU_SELECTED_FIELD: u32 = 0x184;
const MENU_LOW_NIBBLE_CLEAR: u32 = !0xf;
const ENTRY_STRIDE: u32 = 0x20;
const ENTRY_MISSING_FLAG: u32 = 0x0200_0000;
const MODE_AVAILABLE_FLAG: u32 = 0x0002_0000;
const MODE_DISABLED: u8 = 0x0a;

/// 00042F90 game_hud_prompt_allowed_by_flags_7de458_and_player_field_0x114. cdecl(0), EAX only.
pub fn hud_prompt_allowed<M: GuestMemory>(m: &M) -> u32 {
    if m.read32(HUD_FLAGS) & PROMPT_BLOCKING_FLAGS != 0 {
        return 0;
    }
    if (m.read32(PLAYER_COUNT_WORD) as i32) > 0x64 {
        return 1;
    }
    let player = m.read32(CURRENT_PLAYER);
    if player == 0 {
        return 1;
    }
    u32::from(m.read32(player.wrapping_add(PLAYER_PROMPT_STATE)) != PLAYER_PROMPT_HIDDEN)
}

/// 001957D0 game_menu_store_object_field_0x184_into_low_nibble_of_global_756e58. cdecl(1).
/// The field is OR-ed unmasked, as in the original.
pub fn menu_store_field_into_low_nibble<M: GuestMemory>(m: &mut M, object: u32) -> Gprs {
    let word = m.read32(MENU_NIBBLE_WORD);
    let field = m.read32(object.wrapping_add(MENU_SELECTED_FIELD));
    let eax = (word & MENU_LOW_NIBBLE_CLEAR) | field;
    m.write32(MENU_NIBBLE_WORD, eax);
    Gprs { eax, ecx: object, edx: field }
}

/// 002D2BE0 game_menu_flag_mask_mode_not_0xa_and_entry_4d1f88_stride_20_zero. cdecl(2): only
/// the second stack argument `entry` is used.
pub fn menu_flag_mask<M: GuestMemory>(m: &M, entry: u32) -> Gprs {
    let mut eax = if m.read8(MODE_BYTE) == MODE_DISABLED { 0 } else { MODE_AVAILABLE_FLAG };
    let ecx = entry.wrapping_mul(ENTRY_STRIDE);
    let edx = m.read32(ecx.wrapping_add(ENTRY_TABLE));
    if edx == 0 {
        eax |= ENTRY_MISSING_FLAG;
    }
    Gprs { eax, ecx, edx }
}
