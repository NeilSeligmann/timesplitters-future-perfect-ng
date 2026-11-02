// SPDX-License-Identifier: GPL-3.0-or-later
//! The flat colour primitives the HUD draws with (T1898 wave 2): the modulate colour and its
//! presets, a flat quad, a line and the clipped rectangle fill built on them. These are the
//! lower half of the HUD `Draw` trait (`colour_preset`, `fill_rect`, `rect_line`).
//!
//! Evidence class: differential versus the lifted original, see docs/t-render-slice.md.
use alloc::vec::Vec;
use crate::{
    Renderer,
    device::{RenderDevice, RenderHost},
    map::*,
    memory::GuestMemory,
};

/// Original addresses of the functions ported in this module (T1898, see docs/t-render-slice.md).
pub const ROOTS: [u32; 6] = [0x19cc0, 0x1ed60, 0x1a2e0, 0x1afa0, 0x1b090, 0x1efb0];

/// Slot 5 holds which colour parts of the modulate colour are in use.
const MODULATE_MASK_SLOT: u32 = 5;
/// Viewport record: `+0`/`+4` horizontal and `+0x14`/`+0x18` vertical pixel range.
const VIEWPORT_RECORD_POINTER: u32 = 0x007E_71C8;

impl<M: GuestMemory, D: RenderDevice, H: RenderHost> Renderer<'_, M, D, H> {
    /// 0x19CC0: set the modulate colour (0xAARRGGBB). Flushes the pending draw and writes the
    /// slot 5 mask: bit 24 when the alpha byte is clear, bit 0, 8 and 16 pattern `0x10101`
    /// when the colour bytes are clear (the original builds both with a `neg`/`sbb` idiom
    /// and a wrapping add). Returns the previous colour.
    pub fn set_modulate_colour(&mut self, colour: u32) -> u32 {
        let previous = self.mem.read32(MODULATE_COLOUR);
        let alpha_part = if colour & 0xFF00_0000 != 0 {
            0xFF00_0000u32
        } else {
            0
        };
        let colour_part = if colour & 0x00FF_FFFF != 0 {
            0xFFFE_FEFFu32
        } else {
            0
        };
        let high = alpha_part.wrapping_add(0x0100_0000);
        let low = colour_part.wrapping_add(0x0001_0101);
        self.mem.write32(MODULATE_HIGH, high);
        self.mem.write32(MODULATE_LOW, low);
        self.mem.write32(MODULATE_COLOUR, colour);
        self.draw_dispatch();
        self.state_slot_set(MODULATE_MASK_SLOT, low | high);
        previous
    }

    /// 0x1ED60 game_d3d_color_preset_set_return_previous_code: select one of the HUD colour
    /// presets (0 white, 1 black, 2 transparent, 3 white without alpha, other values
    /// transparent). Returns the code of the previous colour: 1, 2 or 3 for black, transparent
    /// and white without alpha, 0 for anything else (including plain white).
    pub fn set_colour_preset(&mut self, preset: u32) -> u32 {
        let colour = match preset {
            0 => 0xFFFF_FFFF,
            1 => 0xFF00_0000,
            3 => 0x00FF_FFFF,
            _ => 0,
        };
        match self.set_modulate_colour(colour) {
            0xFF00_0000 => 1,
            0 => 2,
            0x00FF_FFFF => 3,
            _ => 0,
        }
    }

    /// 0x1A2E0: queue a flat coloured quad as two triangles (6 vertices of x, y, depth,
    /// colour) between the float corners.
    pub fn draw_flat_quad(&mut self, left: f32, top: f32, right: f32, bottom: f32, colour: u32) {
        self.begin_primitive_batch(3, 9, 0x31, 6);
        let depth = self.mem.read32(0x0047_5DBC);
        let vertex = |x: f32, y: f32| [x.to_bits(), y.to_bits(), depth, colour];
        let mut words = Vec::with_capacity(24);
        for (x, y) in [
            (left, top),
            (right, top),
            (left, bottom),
            (right, top),
            (left, bottom),
            (right, bottom),
        ] {
            words.extend(vertex(x, y));
        }
        self.emit_words(&words);
        self.end_batch();
    }

    /// 0x1AFA0: queue a flat coloured line (depth 0) and count it in the draw statistics.
    pub fn draw_flat_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, colour: u32) {
        self.begin_primitive_batch(8, 9, 0x31, 2);
        let words = [
            x0.to_bits(),
            y0.to_bits(),
            0,
            colour,
            x1.to_bits(),
            y1.to_bits(),
            0,
            colour,
        ];
        self.emit_words(&words);
        self.end_batch();
        self.add_draw_statistics(2, 1, 1);
    }

    /// 0x1B090 game_d3d_draw_rect_int_args_count_stats: integer pixel line (the HUD's
    /// `rect_line`), counted a second time in the draw statistics like the original.
    pub fn draw_line_pixels(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, colour: u32) {
        self.draw_flat_line(x0 as f32, y0 as f32, x1 as f32, y1 as f32, colour);
        self.add_draw_statistics(2, 1, 1);
    }

    /// 0x1EFB0 (the HUD's `fill_rect`): fill the pixel rectangle with `colour`, corners in any
    /// order, clipped to the viewport record. Draws nothing when the clipped rectangle is outside.
    pub fn fill_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, colour: u32) {
        let (mut left, mut right) = if x0 > x1 { (x1, x0) } else { (x0, x1) };
        let (mut top, mut bottom) = if y0 > y1 { (y1, y0) } else { (y0, y1) };
        let view = self.mem.read32(VIEWPORT_RECORD_POINTER);
        let min_x = self.mem.read32(view) as i32;
        let limit_x = (self.mem.read32(view.wrapping_add(4)) as i32).wrapping_add(1);
        left = left.max(min_x);
        if left > limit_x || right < min_x {
            return;
        }
        let min_y = self.mem.read32(view.wrapping_add(0x14)) as i32;
        let limit_y = (self.mem.read32(view.wrapping_add(0x18)) as i32).wrapping_add(1);
        right = right.min(limit_x);
        top = top.max(min_y);
        if top > limit_y || bottom < min_y {
            return;
        }
        bottom = bottom.min(limit_y);
        self.draw_flat_quad(left as f32, top as f32, right as f32, bottom as f32, colour);
        self.add_draw_statistics(2, 2, 2);
    }
}
