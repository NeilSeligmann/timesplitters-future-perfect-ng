// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared drawing primitives of the HUD (`game_hud_draw_*` helpers used by every element).

use crate::hud::addr::*;
use crate::hud::draw::{Draw, TexturedRect};
use crate::hud::x86::*;
use crate::memory::GuestMemory;

/// 0x40260 `game_hud_draw_textured_rect_when_inside_screen_and_size_positive`. cdecl(1): (descriptor).
/// The hottest HUD function (about 115 calls per frame). Draws the descriptor through 0x1B7B0 when
/// its origin is left of and above the screen's far edges and both sizes are positive.
/// Descriptor (INFERRED names): `+0` texture, `+0x18/+0x1C` x/y, `+0x20/+0x24` width/height,
/// `+0x28..+0x34` four more words passed through.
// portreg: va=0x00040260 tier=1
// portreg: mirrors=tsfp-ports/hud_lane
// portreg: mirrors=tsfp-ports/c2r/ui_hud
pub fn draw_textured_rect<M: GuestMemory, D: Draw>(m: &M, d: &mut D, descriptor: u32) {
    let at = |offset: u32| descriptor.wrapping_add(offset);
    let screen_width = int_to_f32(read_i32(m, SCREEN_WIDTH_INT));
    if below_or_equal(screen_width, read_f32(m, at(0x18))) {
        return;
    }
    let screen_height = int_to_f32(read_i32(m, SCREEN_HEIGHT_INT));
    if below_or_equal(screen_height, read_f32(m, at(0x1c))) {
        return;
    }
    if below_or_equal(read_f32(m, at(0x20)), 0.0) || below_or_equal(read_f32(m, at(0x24)), 0.0) {
        return;
    }
    d.textured_rect(TexturedRect {
        x: read_f32(m, at(0x18)),
        y: read_f32(m, at(0x1c)),
        width: read_f32(m, at(0x20)),
        height: read_f32(m, at(0x24)),
        raw_28: m.read32(at(0x28)),
        raw_2c: m.read32(at(0x2c)),
        raw_30: m.read32(at(0x30)),
        raw_34: m.read32(at(0x34)),
        texture: m.read32(descriptor),
    });
}
