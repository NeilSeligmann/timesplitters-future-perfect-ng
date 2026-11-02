// SPDX-License-Identifier: GPL-3.0-or-later
//! Attachment point 2: the renderer calls the HUD makes.
//!
//! Each method is one original renderer entry point (address in the doc comment). The HUD never
//! touches the renderer any other way, so a software rasteriser, the recorded-call implementation
//! below or a Vulkan backend can all stand behind the trait.

use crate::hud::addr;

/// Arguments of the textured rectangle entry 0x1B7B0, in the original's argument order. Fields
/// whose meaning is not established keep their raw bits (INFERRED names: the callee is
/// `..._scaled_by_stage0_texture_size`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexturedRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub raw_28: u32,
    pub raw_2c: u32,
    pub raw_30: u32,
    pub raw_34: u32,
    /// First dword of the descriptor (texture handle or record).
    pub texture: u32,
}

/// One renderer call, as the original made it.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawCall {
    /// 0x19650(group)
    StateGroup(u32),
    /// 0x19d70(mode)
    BlendMode(u32),
    /// 0x1ed60(preset)
    ColourPreset(u32),
    /// 0x1efb0(left, top, right, bottom, alpha), clipped to the viewport
    FillRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        alpha: u32,
    },
    /// 0x1b7b0(...)
    TexturedRect(TexturedRect),
    /// 0x42490(position, text, colour): `colour` is 0xAARRGGBB-style as the HUD builds it
    Text {
        x: i32,
        y: i32,
        text: String,
        colour: u32,
    },
    /// 0x1b090(x0, y0, x1, y1, colour)
    RectLine {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        colour: u32,
    },
    /// 0x2d76b0(a, b, c, d, thickness, alpha_bits)
    CornerFrame([u32; 6]),
    /// 0x2d7f90(a, b, c, d, fill, alpha_bits)
    Loadbar([u32; 6]),
}

/// The renderer as the HUD sees it.
pub trait Draw {
    /// 0x19650: select the render state group.
    fn state_group(&mut self, group: u32);
    /// 0x19d70: select the blend mode.
    fn blend_mode(&mut self, mode: u32);
    /// 0x1ed60: select the colour preset (1 is the HUD's flat black).
    fn colour_preset(&mut self, preset: u32);
    /// 0x1efb0: flat rectangle clipped to the current viewport.
    fn fill_rect(&mut self, left: i32, top: i32, right: i32, bottom: i32, alpha: u32);
    /// 0x1b7b0: textured rectangle.
    fn textured_rect(&mut self, rect: TexturedRect);
    /// 0x42490: text at an integer position.
    fn text(&mut self, x: i32, y: i32, text: &str, colour: u32);
    /// 0x1B090: one pixel wide rectangle outline or line in a packed colour (RGBA, alpha in the
    /// low byte, INFERRED from the colour scaling helper 0x154FE0).
    fn rect_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, colour: u32);
    /// 0x2D76B0: framed corner brackets of a widget (INFERRED name `corner_frame_variant_one`).
    /// Arguments in the original's order; `alpha_bits` is the widget alpha float.
    fn corner_frame(&mut self, a: i32, b: i32, c: i32, d: i32, thickness: i32, alpha_bits: u32);
    /// 0x2D7F90: framed progress bar of a widget (INFERRED name `loadbar_framed_progress_bar`).
    fn loadbar(&mut self, a: i32, b: i32, c: i32, d: i32, fill: i32, alpha_bits: u32);
}

/// Records every call. Used by the tests, the differential oracle and the picture tool.
#[derive(Default, Debug)]
pub struct RecordingDraw {
    pub calls: Vec<DrawCall>,
}

impl Draw for RecordingDraw {
    fn state_group(&mut self, group: u32) {
        self.calls.push(DrawCall::StateGroup(group));
    }
    fn blend_mode(&mut self, mode: u32) {
        self.calls.push(DrawCall::BlendMode(mode));
    }
    fn colour_preset(&mut self, preset: u32) {
        self.calls.push(DrawCall::ColourPreset(preset));
    }
    fn fill_rect(&mut self, left: i32, top: i32, right: i32, bottom: i32, alpha: u32) {
        self.calls.push(DrawCall::FillRect {
            left,
            top,
            right,
            bottom,
            alpha,
        });
    }
    fn textured_rect(&mut self, rect: TexturedRect) {
        self.calls.push(DrawCall::TexturedRect(rect));
    }
    fn text(&mut self, x: i32, y: i32, text: &str, colour: u32) {
        self.calls.push(DrawCall::Text {
            x,
            y,
            text: text.to_owned(),
            colour,
        });
    }
    fn rect_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, colour: u32) {
        self.calls.push(DrawCall::RectLine { x0, y0, x1, y1, colour });
    }
    fn corner_frame(&mut self, a: i32, b: i32, c: i32, d: i32, thickness: i32, alpha_bits: u32) {
        self.calls.push(DrawCall::CornerFrame([
            a as u32, b as u32, c as u32, d as u32, thickness as u32, alpha_bits,
        ]));
    }
    fn loadbar(&mut self, a: i32, b: i32, c: i32, d: i32, fill: i32, alpha_bits: u32) {
        self.calls.push(DrawCall::Loadbar([
            a as u32, b as u32, c as u32, d as u32, fill as u32, alpha_bits,
        ]));
    }
}

impl DrawCall {
    /// The original callee address of this call.
    pub fn callee(&self) -> u32 {
        match self {
            DrawCall::StateGroup(_) => addr::FN_STATE_GROUP,
            DrawCall::BlendMode(_) => addr::FN_BLEND_MODE,
            DrawCall::ColourPreset(_) => addr::FN_COLOUR_PRESET,
            DrawCall::FillRect { .. } => addr::FN_FILL_RECT,
            DrawCall::TexturedRect(_) => addr::FN_TEXTURED_RECT,
            DrawCall::Text { .. } => addr::FN_TEXT,
            DrawCall::RectLine { .. } => addr::FN_RECT_LINE,
            DrawCall::CornerFrame(_) => addr::FN_CORNER_FRAME,
            DrawCall::Loadbar(_) => addr::FN_LOADBAR,
        }
    }

    /// The call's arguments as the original pushed them (first argument first), for the
    /// differential comparison against the emulated original. Text has no word form.
    pub fn words(&self) -> Vec<u32> {
        match self {
            DrawCall::StateGroup(v) | DrawCall::BlendMode(v) | DrawCall::ColourPreset(v) => {
                vec![*v]
            }
            DrawCall::FillRect {
                left,
                top,
                right,
                bottom,
                alpha,
            } => vec![*left as u32, *top as u32, *right as u32, *bottom as u32, *alpha],
            DrawCall::TexturedRect(r) => vec![
                r.x.to_bits(),
                r.y.to_bits(),
                r.width.to_bits(),
                r.height.to_bits(),
                r.raw_28,
                r.raw_2c,
                r.raw_30,
                r.raw_34,
                r.texture,
            ],
            DrawCall::Text { x, y, colour, .. } => vec![*x as u32, *y as u32, *colour],
            DrawCall::RectLine {
                x0,
                y0,
                x1,
                y1,
                colour,
            } => vec![*x0 as u32, *y0 as u32, *x1 as u32, *y1 as u32, *colour],
            DrawCall::CornerFrame(words) | DrawCall::Loadbar(words) => words.to_vec(),
        }
    }

    /// One JSON object per call for `tools/hud_render.py` (no serde dependency).
    pub fn to_json(&self) -> String {
        match self {
            DrawCall::StateGroup(v) => format!("{{\"op\":\"state_group\",\"v\":{v}}}"),
            DrawCall::BlendMode(v) => format!("{{\"op\":\"blend\",\"v\":{v}}}"),
            DrawCall::ColourPreset(v) => format!("{{\"op\":\"preset\",\"v\":{v}}}"),
            DrawCall::FillRect {
                left,
                top,
                right,
                bottom,
                alpha,
            } => format!(
                "{{\"op\":\"fill\",\"l\":{left},\"t\":{top},\"r\":{right},\"b\":{bottom},\"a\":{alpha}}}"
            ),
            DrawCall::TexturedRect(r) => format!(
                "{{\"op\":\"tex\",\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"tex\":{}}}",
                json_f32(r.x),
                json_f32(r.y),
                json_f32(r.width),
                json_f32(r.height),
                r.texture
            ),
            DrawCall::RectLine {
                x0,
                y0,
                x1,
                y1,
                colour,
            } => format!(
                "{{\"op\":\"line\",\"l\":{x0},\"t\":{y0},\"r\":{x1},\"b\":{y1},\"c\":{colour}}}"
            ),
            DrawCall::CornerFrame(w) => format!("{{\"op\":\"corner\",\"w\":{w:?}}}"),
            DrawCall::Loadbar(w) => format!("{{\"op\":\"loadbar\",\"w\":{w:?}}}"),
            DrawCall::Text { x, y, text, colour } => format!(
                "{{\"op\":\"text\",\"x\":{x},\"y\":{y},\"c\":{colour},\"s\":\"{}\"}}",
                json_escape(text)
            ),
        }
    }
}

fn json_f32(value: f32) -> String {
    if value.is_finite() {
        format!("{value}")
    } else {
        "0".to_owned()
    }
}

fn json_escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
