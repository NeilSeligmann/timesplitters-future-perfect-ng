// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 wave 4 ui ports (breadth-first). Only 0035DE60 is portable: the other requested rows
//! have no C replacement body in src/game (registered=False in docs/data/t1791-ui and
//! t1789-mapmaker classifications) and no spec beyond their names, so they are skipped
//! (docs/t1895-wave4-ui.md is not written here; see the report).

// portreg: va=0x0035de60
/// 0035DE60 game_return_constant_1. cdecl(0): EAX = 1. Tier 0 (spec is the T1884 name; no C body).
pub fn return_constant_1() -> u32 {
    1
}

/// Guest memory view for the wave 4 ui ports (fake memory in tests).
pub trait W4Memory {
    fn read32(&self, address: u32) -> u32;
    fn read_f32(&self, address: u32) -> f32 {
        f32::from_bits(self.read32(address))
    }
}

/// One recorded draw call: callee VA and its stack arguments in call order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawCall {
    pub callee: u32,
    pub args: Vec<u32>,
}

/// Guest screen dimensions and the renderer entry used by the textured HUD rectangle.
mod textured_rect {
    pub const SCREEN_WIDTH: u32 = 0x004e_8cf0;
    pub const SCREEN_HEIGHT: u32 = 0x004e_8cf4;
    pub const DRAW_CALLEE: u32 = 0x0001_b7b0;
    pub const X: u32 = 0x18;
    pub const Y: u32 = 0x1c;
    pub const WIDTH: u32 = 0x20;
    pub const HEIGHT: u32 = 0x24;
    pub const DRAW_FIELDS: [u32; 9] = [0x18, 0x1c, 0x20, 0x24, 0x28, 0x2c, 0x30, 0x34, 0];
}

/// Widget bounds, progress and opacity are offsets from the guest widget pointer.
mod loadbar {
    pub const PROGRESS: u32 = 0xb4;
    pub const LEFT: u32 = 0xe4;
    pub const TOP: u32 = 0xe8;
    pub const RIGHT: u32 = 0xec;
    pub const BOTTOM: u32 = 0xf0;
    pub const ALPHA: u32 = 0xf4;
    pub const INSET: u32 = 0x0047_5d10;
    pub const THRESHOLD: u32 = 0x0049_f648;
    pub const PROGRESS_SCALE: u32 = 0x0047_5cd4;
    pub const PROGRESS_BIAS: u32 = 0x0047_5c78;
    pub const FULL_LEVEL: u32 = 0x0047_5d08;
    pub const DRAW_CALLEE: u32 = 0x002d_7f90;
}

/// SSE cvttss2si: truncate toward zero, out of range or NaN gives 0x80000000.
fn cvttss2si(value: f32) -> u32 {
    if value.is_nan() || value >= 2147483648.0 || value < -2147483648.0 {
        0x8000_0000
    } else {
        (value as i32) as u32
    }
}

/// 00040260 game_hud_draw_textured_rect_when_inside_screen_and_size_positive. cdecl(1: rect).
/// Draws via 0001B7B0 (nine words: rect[+0x18..+0x34] then rect[0]) only when the screen
/// size ints at 4E8CF0/4E8CF4 exceed rect +0x18/+0x1C and rect +0x20/+0x24 are positive.
pub fn hud_draw_textured_rect<M: W4Memory>(m: &M, rect: u32, trace: &mut Vec<DrawCall>) {
    let field = |offset: u32| m.read32(rect.wrapping_add(offset));
    let screen_w = m.read32(textured_rect::SCREEN_WIDTH) as i32 as f32;
    let screen_h = m.read32(textured_rect::SCREEN_HEIGHT) as i32 as f32;
    let float = |offset: u32| m.read_f32(rect.wrapping_add(offset));
    // comiss "jbe" also skips on unordered (NaN) operands.
    let inside = screen_w > float(textured_rect::X) && screen_h > float(textured_rect::Y);
    if !inside || !(float(textured_rect::WIDTH) > 0.0) || !(float(textured_rect::HEIGHT) > 0.0) {
        return;
    }
    // Pushed last-to-first: callee sees +0x18 first, the rect word 0 last.
    let args = textured_rect::DRAW_FIELDS.map(field).to_vec();
    trace.push(DrawCall { callee: textured_rect::DRAW_CALLEE, args });
}

/// 002D8060 game_ui_widget_draw_loadbar_from_widget_bounds_and_alpha. cdecl(2: widget, arg).
/// Converts bounds at +0xE4/+0xE8 and (+0xEC/+0xF0 minus the 475D10 constant) to ints, picks a
/// level from +0xB4 (scaled when above 49F648, else the 475D08 constant), and calls 002D7F90
/// with (x, y, right, bottom, level, raw float alpha from +0xF4)... the sixth word is the
/// second stack argument slot as in the original (arg is overwritten by the alpha float).
pub fn widget_draw_loadbar<M: W4Memory>(m: &M, widget: u32, trace: &mut Vec<DrawCall>) {
    let float = |offset: u32| m.read_f32(widget.wrapping_add(offset));
    let sub = m.read_f32(loadbar::INSET);
    let x = cvttss2si(float(loadbar::LEFT));
    let y = cvttss2si(float(loadbar::TOP));
    let right = cvttss2si(float(loadbar::RIGHT) - sub);
    let bottom = cvttss2si(float(loadbar::BOTTOM) - sub);
    let progress = float(loadbar::PROGRESS);
    let level = if m.read_f32(loadbar::THRESHOLD) > progress {
        progress * m.read_f32(loadbar::PROGRESS_SCALE) - m.read_f32(loadbar::PROGRESS_BIAS)
    } else {
        m.read_f32(loadbar::FULL_LEVEL)
    };
    let alpha_bits = m.read32(widget.wrapping_add(loadbar::ALPHA));
    trace.push(DrawCall {
        callee: loadbar::DRAW_CALLEE,
        args: vec![x, y, right, bottom, cvttss2si(level), alpha_bits],
    });
}
