// SPDX-License-Identifier: GPL-3.0-or-later
//! Health and armour threshold flags of the player HUD.
//!
//! The HUD shows health (and, for characters, armour) as 20 segments. Segment `i` is lit when the
//! value reaches `table[i] * maximum` (tables 0x4E6D80 / 0x4E6F28, one float per 0x14 bytes). A
//! per-player "flag block" (pointed to by object `+0x268`) keeps the last lit state, a fade timer
//! per segment and a flash timer, so a changed segment can animate.
//!
//! Flag block layout (MEASURED from the loops): health flags `+0x00`, armour flags `+0x50`,
//! health timers `+0xA0`, armour timers `+0xF0` (20 dwords each), flash timer `+0x140` (f32).

use crate::hud::addr::*;
use crate::hud::x86::*;
use crate::memory::GuestMemory;

/// The part of the session the HUD asks about: the network player-index remap (0x356800).
pub trait PlayerRemap {
    fn remap_player(&mut self, player: i32) -> u32;
}

/// Whether `health >= threshold * maximum` for segment `index`, as the original computes it.
fn health_segment_lit<M: GuestMemory>(m: &M, object: u32, index: u32) -> u32 {
    let threshold = read_f32(m, HEALTH_THRESHOLDS + index * THRESHOLD_STRIDE)
        * read_f32(m, object.wrapping_add(OBJ_MAX_HEALTH));
    u32::from(!below(read_f32(m, object.wrapping_add(OBJ_HEALTH)), threshold))
}

/// Armour segment `index` (characters, class 8, only); zero otherwise.
fn armor_segment_lit<M: GuestMemory>(m: &M, object: u32, index: u32) -> u32 {
    if m.read32(object.wrapping_add(OBJ_CLASS)) != CLASS_CHARACTER {
        return 0;
    }
    let character = m.read32(object.wrapping_add(OBJ_CHARACTER));
    let threshold = read_f32(m, ARMOR_THRESHOLDS + index * THRESHOLD_STRIDE)
        * read_f32(m, character.wrapping_add(CHAR_ARMOR_MAX));
    u32::from(!below(read_f32(m, character.wrapping_add(CHAR_ARMOR)), threshold))
}

/// 0x12C830 `game_hud_player_health_fraction_threshold_flags_init_from_object_0x1cc_over_0x1d8_tables`.
/// cdecl(2): (flag_block, object). Sets every flag from the object's values and clears timers.
// portreg: va=0x0012c830 tier=1
// portreg: mirrors=tsfp-ports/c2r/ui_hud
// portreg: mirrors=tsfp-ports/c2r/ui_hud_2
pub fn flags_init<M: GuestMemory>(m: &mut M, block: u32, object: u32) {
    m.write32(block.wrapping_add(FLAGS_FLASH), 0);
    for index in 0..THRESHOLD_COUNT {
        let slot = block.wrapping_add(4 * index);
        let health = health_segment_lit(m, object, index);
        m.write32(slot.wrapping_add(FLAGS_HEALTH), health);
        m.write32(slot.wrapping_add(TIMERS_HEALTH), 0);
        m.write32(slot.wrapping_add(FLAGS_ARMOR), 0);
        m.write32(slot.wrapping_add(TIMERS_ARMOR), 0);
        if m.read32(object.wrapping_add(OBJ_CLASS)) == CLASS_CHARACTER {
            let armor = armor_segment_lit(m, object, index);
            m.write32(slot.wrapping_add(FLAGS_ARMOR), armor);
        }
    }
}

/// 0x13A140 `game_hud_player_health_fraction_threshold_flags_differ_from_stored_rows`. cdecl(2): (flag_block, object). True when any
/// segment's lit state differs from the stored flags.
// portreg: va=0x0013a140 tier=1
pub fn flags_differ<M: GuestMemory>(m: &M, block: u32, object: u32) -> bool {
    for index in 0..THRESHOLD_COUNT {
        let slot = block.wrapping_add(4 * index);
        let health = health_segment_lit(m, object, index);
        let armor = armor_segment_lit(m, object, index);
        if m.read32(slot.wrapping_add(FLAGS_HEALTH)) != health
            || m.read32(slot.wrapping_add(FLAGS_ARMOR)) != armor
        {
            return true;
        }
    }
    false
}

/// 0x13A270 `game_hud_player_slot_set_timer_0x20_and_init_health_fraction_flags`. cdecl(1): (record).
/// `record+4` is the player index, `record+0x14` the character object.
// portreg: va=0x0013a270 tier=1
pub fn slot_init<M: GuestMemory>(m: &mut M, record: u32) {
    let player = read_i32(m, record.wrapping_add(4));
    if player < 0 {
        return;
    }
    let frames = int_to_f32(read_i32(m, FRAME_RATE_INT).wrapping_mul(2));
    let timer = truncate_to_i32(frames * read_f32(m, CONST_LETTERBOX_MIN));
    let block_address = HUD_BLOCK_BASE
        .wrapping_add((player as u32).wrapping_mul(HUD_BLOCK_STRIDE))
        .wrapping_add(BLOCK_TIMER_20);
    m.write32(block_address, timer as u32);
    let object = m.read32(record.wrapping_add(PLAYER_OBJECT));
    let block = m.read32(object.wrapping_add(OBJ_HUD_FLAGS));
    if block == 0 {
        return;
    }
    flags_init(m, block, object);
}

/// 0x13A1D0 `game_hud_player_slot_timer_0x20_advance_then_check_health_fraction_flags_change`. cdecl(2): (player, scale).
/// Advances the player's timer (`+0x20` of the HUD block) by `2 * rate * scale * 0.5` frames,
/// capped at `2 * rate`, then raises the flash when the segments changed (or both session
/// globals 0x6D612C / 0x6D6130 are set).
// portreg: va=0x0013a1d0 tier=1
pub fn slot_update<M: GuestMemory, R: PlayerRemap>(m: &mut M, remap: &mut R, player: i32, scale: f32) {
    if player < 0 {
        return;
    }
    let block_base = HUD_BLOCK_BASE.wrapping_add((player as u32).wrapping_mul(HUD_BLOCK_STRIDE));
    let slot = remap.remap_player(player);
    let record = m
        .read32(CURRENT_PLAYER_PTR)
        .wrapping_add(slot.wrapping_mul(PLAYER_RECORD_SIZE));
    let object = m.read32(record.wrapping_add(PLAYER_OBJECT));
    let block = m.read32(object.wrapping_add(OBJ_HUD_FLAGS));
    let cap = read_i32(m, FRAME_RATE_INT).wrapping_mul(2);
    let step = truncate_to_i32(int_to_f32(cap) * scale * read_f32(m, CONST_HALF));
    let timer = read_i32(m, block_base.wrapping_add(BLOCK_TIMER_20)).wrapping_add(step);
    m.write32(block_base.wrapping_add(BLOCK_TIMER_20), timer as u32);
    if timer > cap {
        m.write32(block_base.wrapping_add(BLOCK_TIMER_20), cap as u32);
    }
    if flags_differ(m, block, object)
        && (m.read32(0x006d_612c) == 0 || m.read32(0x006d_6130) == 0)
    {
        return;
    }
    let flash = m.read32(CONST_FADE);
    m.write32(block.wrapping_add(FLAGS_FLASH), flash);
}

/// 0x13A2C0 `game_hud_player_health_fraction_flag_timers_update_decay_by_frame_step`. cdecl(2): (flag_block, object). Ages the flash and
/// the 40 segment timers by one frame step and restarts a segment's timer when its lit state
/// changed.
// portreg: va=0x0013a2c0 tier=1
pub fn flags_tick<M: GuestMemory>(m: &mut M, block: u32, object: u32) {
    let rate = int_to_f32(read_i32(m, FRAME_RATE_INT));
    let step = read_f32(m, CONST_ONE) / rate * read_f32(m, FRAME_DELTA_F32);
    let flash = read_f32(m, block.wrapping_add(FLAGS_FLASH));
    if flash > 0.0 {
        write_f32(m, block.wrapping_add(FLAGS_FLASH), flash - step);
    }
    let flash_value = m.read32(CONST_FADE);
    let timer_start = m.read32(CONST_FLASH);
    for index in 0..THRESHOLD_COUNT {
        let slot = block.wrapping_add(4 * index);
        let health = health_segment_lit(m, object, index);
        let armor = armor_segment_lit(m, object, index);
        if m.read32(slot.wrapping_add(FLAGS_HEALTH)) != health {
            m.write32(block.wrapping_add(FLAGS_FLASH), flash_value);
            m.write32(slot.wrapping_add(FLAGS_HEALTH), health);
            m.write32(slot.wrapping_add(TIMERS_HEALTH), timer_start);
        }
        if m.read32(slot.wrapping_add(FLAGS_ARMOR)) != armor {
            m.write32(block.wrapping_add(FLAGS_FLASH), flash_value);
            m.write32(slot.wrapping_add(FLAGS_ARMOR), armor);
            m.write32(slot.wrapping_add(TIMERS_ARMOR), timer_start);
        }
    }
    for index in 0..THRESHOLD_COUNT {
        let slot = block.wrapping_add(4 * index);
        for base in [TIMERS_HEALTH, TIMERS_ARMOR] {
            let address = slot.wrapping_add(base);
            let timer = read_f32(m, address);
            if timer > 0.0 {
                let aged = timer - step;
                write_f32(m, address, aged);
                if !below_or_equal(0.0, aged) {
                    write_f32(m, address, 0.0);
                }
            }
        }
    }
}
