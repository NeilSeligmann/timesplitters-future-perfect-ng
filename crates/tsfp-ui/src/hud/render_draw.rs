// SPDX-License-Identifier: GPL-3.0-or-later
//! T1987 (T1976 proposal d): the HUD [`Draw`] trait backed by the real render slice
//! (`tsfp_engine_render::Renderer`), so HUD code can draw through the ported D3D8 emitters
//! instead of the recorded-call stand-in. The mapping is docs/t-render-slice.md section "HUD
//! `Draw` trait": state group 0x19650, blend mode 0x19D70, colour preset 0x1ED60, flat rectangle
//! 0x1EFB0 (the `alpha` argument IS the packed colour word), textured rectangle 0x1B7B0 (its nine
//! arguments are the four float CORNERS then four integer texel coordinates then the packed
//! diffuse colour, so `TexturedRect` fields `x y width height` carry left, top, right, bottom and
//! `texture` carries the colour, the doc note of the render slice) and the pixel line.
//! `text`, `corner_frame` and `loadbar` are UI widgets, not render entry points: they go to the
//! `ui` delegate (for example the hand-ported text path or [`RecordingDraw`]).
use crate::hud::draw::{Draw, TexturedRect};
use tsfp_engine_render::{
    Renderer,
    device::{RenderDevice, RenderHost},
    memory::GuestMemory,
};

/// A [`Draw`] that forwards the render calls to a [`Renderer`] and the widget calls to `ui`.
pub struct RendererDraw<'a, M: GuestMemory, D: RenderDevice, H: RenderHost, U: Draw> {
    pub renderer: Renderer<'a, M, D, H>,
    pub ui: U,
}

impl<'a, M: GuestMemory, D: RenderDevice, H: RenderHost, U: Draw> RendererDraw<'a, M, D, H, U> {
    pub fn new(renderer: Renderer<'a, M, D, H>, ui: U) -> Self {
        Self { renderer, ui }
    }
}

impl<M: GuestMemory, D: RenderDevice, H: RenderHost, U: Draw> Draw for RendererDraw<'_, M, D, H, U> {
    fn state_group(&mut self, group: u32) {
        self.renderer.set_state_group(group);
    }
    fn blend_mode(&mut self, mode: u32) {
        self.renderer.set_blend_mode(mode);
    }
    fn colour_preset(&mut self, preset: u32) {
        self.renderer.set_colour_preset(preset);
    }
    fn fill_rect(&mut self, left: i32, top: i32, right: i32, bottom: i32, alpha: u32) {
        self.renderer.fill_rect(left, top, right, bottom, alpha);
    }
    fn textured_rect(&mut self, rect: TexturedRect) {
        self.renderer.draw_textured_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            rect.raw_28 as i32,
            rect.raw_2c as i32,
            rect.raw_30 as i32,
            rect.raw_34 as i32,
            rect.texture,
        );
    }
    fn text(&mut self, x: i32, y: i32, text: &str, colour: u32) {
        self.ui.text(x, y, text, colour);
    }
    fn rect_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, colour: u32) {
        self.renderer.draw_line_pixels(x0, y0, x1, y1, colour);
    }
    fn corner_frame(&mut self, a: i32, b: i32, c: i32, d: i32, thickness: i32, alpha_bits: u32) {
        self.ui.corner_frame(a, b, c, d, thickness, alpha_bits);
    }
    fn loadbar(&mut self, a: i32, b: i32, c: i32, d: i32, fill: i32, alpha_bits: u32) {
        self.ui.loadbar(a, b, c, d, fill, alpha_bits);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_engine_render::memory::GuestMemory;
    use crate::hud::draw::RecordingDraw;
    use tsfp_engine_render::{
        device::{RecordingDevice, RecordingHost},
        map::{DIRTY_NEWEST, DIRTY_OLDEST, DIRTY_SENTINEL},
        mem::SparseMemory,
        state::StateSlot,
    };

    #[test]
    fn render_calls_reach_the_device_and_widgets_reach_the_delegate() {
        let mut mem = SparseMemory::new();
        // the renderer expects the booted guest state: an empty dirty-render-state list and the 35 state slots
        mem.write32(DIRTY_NEWEST, DIRTY_SENTINEL);
        mem.write32(DIRTY_OLDEST, DIRTY_SENTINEL);
        mem.write32(DIRTY_SENTINEL + 0x10, DIRTY_SENTINEL);
        mem.write32(DIRTY_SENTINEL + 0x14, DIRTY_SENTINEL);
        for index in 0..35u32 {
            mem.write32(StateSlot::address(index), index);
        }
        let mut device = RecordingDevice::default();
        let mut host = RecordingHost::default();
        let mut draw = RendererDraw::new(Renderer::new(&mut mem, &mut device, &mut host), RecordingDraw::default());
        draw.state_group(2);
        draw.blend_mode(1);
        draw.colour_preset(1);
        draw.fill_rect(10, 10, 50, 40, 0x8000_0000);
        draw.textured_rect(TexturedRect {
            x: 1.0,
            y: 2.0,
            width: 33.0,
            height: 34.0,
            raw_28: 0,
            raw_2c: 0,
            raw_30: 8,
            raw_34: 8,
            texture: 0xFFFF_FFFF,
        });
        draw.rect_line(0, 0, 9, 9, 0xFF00_00FF);
        draw.text(5, 6, "HUD", 0xFFFF_FFFF);
        draw.corner_frame(1, 2, 3, 4, 1, 0x3F80_0000);
        draw.loadbar(1, 2, 3, 4, 5, 0x3F80_0000);
        let ui_calls = draw.ui.calls.len();
        drop(draw);
        assert_eq!(ui_calls, 3, "text, corner_frame and loadbar stay in the widget delegate");
        assert_eq!(mem.read32(tsfp_engine_render::map::STATE_GROUP), 2, "state group written by the renderer");
        assert_eq!(mem.read32(tsfp_engine_render::map::BLEND_MODE), 1, "blend mode written by the renderer");
        assert!(!device.calls.is_empty(), "draws reach the render device");
    }
}
