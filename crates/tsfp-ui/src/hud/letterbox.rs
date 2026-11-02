// SPDX-License-Identifier: GPL-3.0-or-later
//! Cinematic letterbox bars: start, per-frame update and draw.
//!
//! State: amount `0x7490EC` (0 hidden, 1 full), mode `0x7490F0` (1 fading in, 0 holding, -1 fading
//! out), hold counter `0x7490F4`, draw gate `0x7490E8` (nonzero suppresses the draw).
//! Tier: see docs/t-hud-slice.md.

use crate::hud::addr::*;
use crate::hud::draw::Draw;
use crate::hud::x86::*;
use crate::memory::GuestMemory;

/// The start functions ignore the request in split-screen (more than one local player, mode byte
/// 0x0A, flag 0x200 clear) unless `force` is nonzero.
fn start_is_ignored<M: GuestMemory>(m: &M, force: u32) -> bool {
    m.read8(MODE_BYTE) == 0x0a
        && read_i32(m, LOCAL_PLAYER_COUNT) > 1
        && m.read32(MODE_FLAGS) & 0x200 == 0
        && force == 0
}

/// 0x139520 `game_hud_letterbox_start_hold_state_0_full_amount`. cdecl(2): (hold_frames, force).
/// Bars appear instantly at full amount and hold for `hold_frames`.
// portreg: va=0x00139520 tier=1
pub fn start_hold<M: GuestMemory>(m: &mut M, hold_frames: u32, force: u32) {
    if start_is_ignored(m, force) {
        return;
    }
    let full = m.read32(CONST_ONE);
    m.write32(LETTERBOX_NO_DRAW, force);
    m.write32(LETTERBOX_AMOUNT, full);
    m.write32(LETTERBOX_HOLD, hold_frames);
    m.write32(LETTERBOX_STATE, 0);
}

/// 0x139570 `game_hud_letterbox_start_fade_in_state_1`. cdecl(2): (hold_frames, force).
// portreg: va=0x00139570 tier=1
pub fn start_fade_in<M: GuestMemory>(m: &mut M, hold_frames: u32, force: u32) {
    if start_is_ignored(m, force) {
        return;
    }
    m.write32(LETTERBOX_NO_DRAW, force);
    m.write32(LETTERBOX_STATE, 1);
    m.write32(LETTERBOX_HOLD, hold_frames);
}

/// 0x1395C0 `game_hud_letterbox_state_7490f0_update_fade_in_hold_fade_out`. cdecl(0).
// portreg: va=0x001395c0 tier=1
pub fn update<M: GuestMemory>(m: &mut M) {
    match m.read32(LETTERBOX_STATE) as i32 {
        1 => {
            let one = read_f32(m, CONST_ONE);
            let amount = read_f32(m, LETTERBOX_AMOUNT);
            if below_or_equal(one, amount) {
                return;
            }
            let rate = int_to_f32(read_i32(m, FRAME_RATE_INT));
            let grown = read_f32(m, CONST_LETTERBOX_RATE) / rate * read_f32(m, FRAME_STEP_F32) + amount;
            write_f32(m, LETTERBOX_AMOUNT, grown);
            if below_or_equal(grown, one) {
                return;
            }
            m.write32(LETTERBOX_STATE, 0);
            write_f32(m, LETTERBOX_AMOUNT, one);
        }
        0 => {
            let hold = read_i32(m, LETTERBOX_HOLD);
            if hold < 0 {
                return;
            }
            let remaining = hold.wrapping_sub(read_i32(m, FRAME_DELTA_INT));
            m.write32(LETTERBOX_HOLD, remaining as u32);
            if remaining > 0 {
                return;
            }
            m.write32(LETTERBOX_HOLD, 0);
            m.write32(LETTERBOX_STATE, u32::MAX);
        }
        -1 => {
            let amount = read_f32(m, LETTERBOX_AMOUNT);
            if below_or_equal(amount, 0.0) {
                return;
            }
            let rate = int_to_f32(read_i32(m, FRAME_RATE_INT));
            let shrunk = amount - read_f32(m, CONST_LETTERBOX_RATE) / rate * read_f32(m, FRAME_STEP_F32);
            write_f32(m, LETTERBOX_AMOUNT, shrunk);
            if below_or_equal(0.0, shrunk) {
                return;
            }
            m.write32(LETTERBOX_STATE, 0);
            write_f32(m, LETTERBOX_AMOUNT, 0.0);
        }
        _ => {}
    }
}

/// 0x1396B0 `game_hud_draw_letterbox_bars_by_amount`. cdecl(0). Two flat bars of
/// `amount * scale` pixels at the bottom and top of the viewport (`[0x7E71C8]`: x0, x1 at +0/+4,
/// y0, y1 at +0x14/+0x18; INFERRED names).
// portreg: va=0x001396b0 tier=1
// portreg: mirrors=tsfp-ports/c2r/ui_hud
// portreg: mirrors=tsfp-ports/c2r/ui_hud_2
pub fn draw<M: GuestMemory, D: Draw>(m: &M, d: &mut D) {
    if m.read32(LETTERBOX_NO_DRAW) != 0 {
        return;
    }
    let amount = read_f32(m, LETTERBOX_AMOUNT);
    if below_or_equal(amount, read_f32(m, CONST_LETTERBOX_MIN)) {
        return;
    }
    let bar = truncate_to_i32(amount * read_f32(m, CONST_BAR_SCALE));
    d.state_group(0);
    d.blend_mode(1);
    d.colour_preset(1);
    let viewport = m.read32(VIEWPORT_PTR);
    let x0 = m.read32(viewport) as i32;
    let x1 = m.read32(viewport.wrapping_add(4)) as i32;
    let y1 = m.read32(viewport.wrapping_add(0x18)) as i32;
    d.fill_rect(x0, y1.wrapping_sub(bar), x1.wrapping_add(1), y1.wrapping_add(1), 0x80);
    let viewport = m.read32(VIEWPORT_PTR);
    let x0 = m.read32(viewport) as i32;
    let x1 = m.read32(viewport.wrapping_add(4)) as i32;
    let y0 = m.read32(viewport.wrapping_add(0x14)) as i32;
    d.fill_rect(
        x0,
        y0,
        x1.wrapping_add(1),
        y0.wrapping_add(bar).wrapping_add(1),
        0x80,
    );
}
