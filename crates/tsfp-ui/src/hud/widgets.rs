// SPDX-License-Identifier: GPL-3.0-or-later
//! `ui/framework` widget drawers the HUD and menus share, plus the colour helper they use.
//!
//! A widget record (INFERRED, read by every drawer below): `+0x20` style pointer (colours at
//! `+0x3C` / `+0x40`), `+0xB4` size parameter, `+0xE4/+0xE8` x/y, `+0xEC/+0xF0` right/bottom
//! (floats), `+0xF4` alpha (float). docs/t-ui-framework.md describes the widget system.

use crate::hud::addr::*;
use crate::hud::draw::Draw;
use crate::hud::x86::*;
use crate::memory::GuestMemory;

/// 0x154FE0 `game_packed_color_scale_low_byte_by_clamped_float`. cdecl(2): (colour, alpha).
/// Returns `colour` with its low byte (the alpha channel of the HUD's packed colours) multiplied by
/// `alpha` clamped to 0..1 and truncated. The original multiplies on the x87 stack: the product of
/// an 8 bit integer and a float is exact in `f64`, which this port uses (it assumes the x87
/// precision control is at least 53 bits, INFERRED). A NaN alpha stays NaN and the game's `_ftol`
/// turns it into 0 (see [`ftol_low32`]).
// portreg: va=0x00154fe0 tier=1
// portreg: mirrors=tsfp-ui/text_entry
pub fn scale_colour_low_byte(colour: u32, alpha: f32) -> u32 {
    let one = 1.0f32;
    let clamped = if alpha > one {
        one
    } else if !below_or_equal(0.0, alpha) {
        0.0
    } else {
        alpha
    };
    let scaled = ftol_low32(f64::from(colour & 0xff) * f64::from(clamped));
    (colour & 0xffff_ff00) | scaled
}

/// 0x73110 `game_ui_widget_draw_two_horizontal_divider_lines_in_rect_arg_with_colors_0x3c_0x40`.
/// cdecl(3): (widget, _, rect). `rect` is four floats `x0, y0, x1, y1`. Draws two one pixel lines
/// at the vertical middle (and one pixel below) in the style's two colours scaled by the widget
/// alpha. The second argument is not read.
// portreg: va=0x00073110 tier=1
pub fn draw_divider_lines<M: GuestMemory, D: Draw>(m: &M, d: &mut D, widget: u32, rect: u32) {
    let alpha = read_f32(m, widget.wrapping_add(WIDGET_ALPHA));
    let style = m.read32(widget.wrapping_add(WIDGET_STYLE));
    let colour_a = scale_colour_low_byte(m.read32(style.wrapping_add(STYLE_COLOUR_A)), alpha);
    let alpha = read_f32(m, widget.wrapping_add(WIDGET_ALPHA));
    let colour_b = scale_colour_low_byte(m.read32(style.wrapping_add(STYLE_COLOUR_B)), alpha);
    let x0 = read_f32(m, rect);
    let y0 = read_f32(m, rect.wrapping_add(4));
    let x1 = read_f32(m, rect.wrapping_add(8));
    let y1 = read_f32(m, rect.wrapping_add(12));
    let middle = (y1 - y0) * read_f32(m, CONST_HALF) + y0;
    let (x0, x1) = (truncate_to_i32(x0), truncate_to_i32(x1));
    let row = truncate_to_i32(middle);
    d.rect_line(x0, row, x1, row, colour_a);
    let row_below = truncate_to_i32(middle + read_f32(m, CONST_ONE));
    d.rect_line(x0, row_below, x1, row_below, colour_b);
}

/// Widget bounds as the frame drawers use them: `(x, y, right - 1.0, bottom - 1.0)` truncated.
fn frame_bounds<M: GuestMemory>(m: &M, widget: u32) -> [i32; 4] {
    let inset = read_f32(m, CONST_FADE);
    [
        truncate_to_i32(read_f32(m, widget.wrapping_add(WIDGET_X))),
        truncate_to_i32(read_f32(m, widget.wrapping_add(WIDGET_Y))),
        truncate_to_i32(read_f32(m, widget.wrapping_add(WIDGET_RIGHT)) - inset),
        truncate_to_i32(read_f32(m, widget.wrapping_add(WIDGET_BOTTOM)) - inset),
    ]
}

/// 0x2D7AE0 `game_ui_widget_draw_corner_frame_from_widget_bounds`. cdecl(1): (widget).
// portreg: va=0x002d7ae0 tier=1
pub fn draw_corner_frame<M: GuestMemory, D: Draw>(m: &M, d: &mut D, widget: u32) {
    let [a, b, c, e] = frame_bounds(m, widget);
    let size = read_f32(m, widget.wrapping_add(WIDGET_SIZE_B4));
    let thickness = if read_f32(m, CONST_CORNER_LIMIT) > size {
        size * read_f32(m, CONST_HALF)
    } else {
        read_f32(m, CONST_DEFAULT_THICKNESS)
    };
    d.corner_frame(
        a,
        b,
        c,
        e,
        truncate_to_i32(thickness),
        m.read32(widget.wrapping_add(WIDGET_ALPHA)),
    );
}

/// 0x2D8060 `game_ui_widget_draw_loadbar_from_widget_bounds_and_alpha`. cdecl(1): (widget).
// portreg: va=0x002d8060 tier=1
// portreg: mirrors=tsfp-ports/c2r/ui_framework
// portreg: mirrors=tsfp-ports/c2r/t1937_math
pub fn draw_loadbar<M: GuestMemory, D: Draw>(m: &M, d: &mut D, widget: u32) {
    let [a, b, c, e] = frame_bounds(m, widget);
    let size = read_f32(m, widget.wrapping_add(WIDGET_SIZE_B4));
    let fill = if read_f32(m, CONST_LOADBAR_LIMIT) > size {
        size * read_f32(m, CONST_HALF) - read_f32(m, CONST_ONE)
    } else {
        read_f32(m, CONST_DEFAULT_THICKNESS)
    };
    d.loadbar(
        a,
        b,
        c,
        e,
        truncate_to_i32(fill),
        m.read32(widget.wrapping_add(WIDGET_ALPHA)),
    );
}
