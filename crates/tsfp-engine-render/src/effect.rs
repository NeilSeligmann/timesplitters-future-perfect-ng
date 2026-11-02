// SPDX-License-Identifier: GPL-3.0-or-later
//! Effect row selection and the draw prepare step (T1898).
//!
//! An effect row (16 bytes at [`EFFECT_ROW_TABLE`], selected by index) holds function pointers
//! that the effects system installs: `+0` begins the effect pass, `+8` ends it, `+0xC` holds the
//! veto flags used by `refresh_blend_methods`, `+4` is the middle handler. The draw prepare step (0x22BE0) runs before every
//! flush, reconciles the cached vertex stream, declaration and batch flags with the pending
//! batch and uploads the pixel shader constants that depend on the blend mode and alpha.
//!
//! Evidence class: differential versus the lifted original, see docs/t-render-slice.md.
use crate::{
    Renderer,
    device::{HostFunction, RenderDevice, RenderHost},
    map::*,
    mem::GuestAccess,
    memory::GuestMemory,
    state::slot,
};

/// Original addresses of the functions ported in this module (T1898, see docs/t-render-slice.md).
pub const ROOTS: [u32; 7] = [
    0x22be0, 0x2a2d0, 0x2a310, 0x2a320, 0x2a340, 0x2a360, 0x1d380,
];

impl<M: GuestMemory, D: RenderDevice, H: RenderHost> Renderer<'_, M, D, H> {
    /// 0x2A2D0 game_flush_then_set_4b917c_selector_and_665470_record_pointer: flush the pending
    /// draw, then select effect row `index` (negative selects none).
    pub fn select_effect_row(&mut self, index: u32) {
        self.draw_dispatch();
        self.mem.write32(EFFECT_PASS, u32::MAX);
        self.mem.write32(EFFECT_ROW_INDEX, index);
        let row = if (index as i32) < 0 {
            0
        } else {
            index.wrapping_shl(4).wrapping_add(EFFECT_ROW_TABLE)
        };
        self.mem.write32(EFFECT_ROW_POINTER, row);
    }

    /// 0x2A310 game_global_4b9180_set: set the effect pass counter.
    pub fn set_effect_pass(&mut self, pass: u32) {
        self.mem.write32(EFFECT_PASS, pass);
    }

    /// 0x2A360 game_call_665470_handler_slot0: run the begin handler of the active effect row.
    /// Like the original it does not test the row for null.
    /// The original tail jumps to the handler, so EAX is the handler's result: the number of passes (the callers loop
    /// over it, T1976: the Tier 1 differential did not compare EAX of this root and the hybrid run looped for ever).
    pub fn begin_effect_pass(&mut self) -> u32 {
        let row = self.mem.read32(EFFECT_ROW_POINTER);
        let handler = self.mem.read32(row);
        self.host.call_address(handler, &[])
    }

    /// 0x2A340 game_call_665470_handler_slot1_with_4b9180: run the middle handler (`+4`) of the
    /// active effect row with the pass counter as its one argument.
    /// Returns the handler's EAX (the original leaves it in EAX).
    pub fn effect_pass_middle(&mut self) -> u32 {
        let pass = self.mem.read32(EFFECT_PASS);
        let row = self.mem.read32(EFFECT_ROW_POINTER);
        let handler = self.mem.read32(row.wrapping_add(4));
        self.host.call_address(handler, &[pass])
    }

    /// 0x2A320 game_call_665470_handler_slot2_after_dispatch: flush the pending draw, then run
    /// the end handler of the active effect row when it has one.
    /// Returns the handler's EAX, 0 when the row has no end handler (the original tail jumps to the handler).
    pub fn end_effect_pass(&mut self) -> u32 {
        self.draw_dispatch();
        let row = self.mem.read32(EFFECT_ROW_POINTER);
        let handler = self.mem.read32(row.wrapping_add(8));
        if handler != 0 {
            self.host.call_address(handler, &[])
        } else {
            0
        }
    }

    /// 0x1D380 game_globals_55fb64_55fb68_clear_with_55fb6c_gated_cleanup: close the batch
    /// counters. With a pass still open (`55FB64` or `55FB68` set) and no batch open
    /// (`55FB6C` clear) the effect pass ends and the row is deselected.
    pub fn batch_cleanup(&mut self) {
        let open = self.mem.read32(BATCH_OPEN) != 0;
        let pass_pending =
            self.mem.read32(BATCH_CLEANUP_A) != 0 || self.mem.read32(BATCH_CLEANUP_B) != 0;
        if !pass_pending && !open {
            self.host.call_address(0x0004_59D0, &[]);
        } else if pass_pending && !open {
            self.end_effect_pass();
            self.select_effect_row(u32::MAX);
        }
        self.mem.write32(BATCH_CLEANUP_A, 0);
        self.mem.write32(BATCH_CLEANUP_B, 0);
    }

    /// 0x22BE0 game_global_4b85e4_reconcile_cached_state_and_submit_pixel_constants: the step
    /// before every draw. Rebinds the vertex stream when the ring or stride changed. With an
    /// effect row selected it restarts the effect when needed and rebuilds the declaration;
    /// without a row it binds the four texture slots, derives the batch flag word `4B85E4`
    /// from them and the blend mode and uploads the blend dependent pixel constants
    /// (0 = alpha broadcast, 1 = packed alpha or fixed, 6 = effect colour). Ends by refreshing
    /// the blend methods.
    pub fn prepare_draw(&mut self) {
        let mode = self.blend_mode();
        let stride = self.mem.read32(BATCH_STRIDE);
        let base = self.mem.read32(VERTEX_BUFFER_BASE_SLOT);
        let declaration_valid = self.mem.read32(DECLARATION_VALID) != 0;
        let stream_cached = declaration_valid
            && self.mem.read32(PREPARE_STREAM_BASE) == base
            && self.mem.read32(PREPARE_STREAM_STRIDE) == stride;
        if !stream_cached {
            self.host
                .call(HostFunction::BindVertexStream, &[base, stride]);
            let base = self.mem.read32(VERTEX_BUFFER_BASE_SLOT);
            let stride = self.mem.read32(BATCH_STRIDE);
            self.mem.write32(PREPARE_STREAM_BASE, base);
            self.mem.write32(PREPARE_STREAM_STRIDE, stride);
        }
        if self.mem.read32(EFFECT_ROW_POINTER) != 0 {
            self.prepare_effect_row();
            return;
        }
        let mut any_texture = false;
        for stage in 0..4u32 {
            let slot_address = BOUND_TEXTURES.wrapping_add(stage * 4);
            let texture = self.mem.read32(slot_address);
            self.bind_texture_with_constants(texture, stage);
            if self.mem.read32(slot_address) != 0 {
                any_texture = true;
            }
        }
        if !any_texture {
            let flags = self.mem.read32(BATCH_EXTRA) & 0xFFFE_FFFB;
            self.mem.write32(BATCH_EXTRA, flags);
        }
        if self.mem.read32(BATCH_EXTRA) & 0x1_0004 != 0 {
            let first = self.mem.read32(BOUND_TEXTURES);
            if first == TEXTURE_PAIR_RECORDS || first == TEXTURE_PAIR_TAIL {
                self.mem.or32(BATCH_EXTRA, 0x4_0000);
            }
        }
        let signed = mode as i32;
        if (0xF..=0x17).contains(&signed) {
            self.mem.or32(BATCH_EXTRA, 0x2000);
        }
        if mode == 0xA || mode == 0x13 {
            self.mem.or32(BATCH_EXTRA, 0x4000);
        }
        let combiner = self.state_slot_pending(slot::COMBINER_MODE) != 0;
        let flags = self.mem.read32(BATCH_EXTRA);
        self.mem.write32(
            BATCH_EXTRA,
            if combiner {
                flags | 0x400_0000
            } else {
                flags & 0xFBFF_FFFF
            },
        );
        let format = self.mem.read32(BATCH_FORMAT);
        let declaration_cached = self.mem.read32(DECLARATION_VALID) != 0
            && self.mem.read32(PREPARE_EXTRA_FLAGS) == self.mem.read32(BATCH_EXTRA)
            && self.mem.read32(PREPARE_DECLARATION_FORMAT) == format;
        if !declaration_cached {
            self.build_vertex_declaration(DECLARATION_BLOCK, format);
            self.mem.write32(DECLARATION_VALID, 1);
            let format = self.mem.read32(BATCH_FORMAT);
            self.mem.write32(PREPARE_DECLARATION_FORMAT, format);
            let flags = self.mem.read32(BATCH_EXTRA);
            self.host.call(HostFunction::EffectStateSubmit, &[flags]);
            let flags = self.mem.read32(BATCH_EXTRA);
            self.mem.write32(PREPARE_EXTRA_FLAGS, flags);
        }
        self.upload_blend_constants(mode);
        self.refresh_blend_methods();
    }

    /// The effect row arm of [`Self::prepare_draw`].
    fn prepare_effect_row(&mut self) {
        if self.mem.read32(DECLARATION_VALID) == 0 || self.mem.read32(BATCH_EXTRA) != 0 {
            self.host.call(HostFunction::EffectBegin, &[]);
            self.host.call(HostFunction::EffectSetup, &[]);
        }
        let format = self.mem.read32(BATCH_FORMAT);
        if self.mem.read32(DECLARATION_VALID) == 0
            || self.mem.read32(PREPARE_DECLARATION_FORMAT) != format
        {
            self.build_vertex_declaration(DECLARATION_BLOCK, format);
            self.mem.write32(DECLARATION_VALID, 1);
            let format = self.mem.read32(BATCH_FORMAT);
            self.mem.write32(PREPARE_DECLARATION_FORMAT, format);
        }
        self.effect_pass_middle();
        self.mem.write32(PREPARE_EXTRA_FLAGS, 0);
    }

    /// Pixel shader constants of the draw prepare step. A byte becomes `byte * (1 / 255)` in
    /// x87 double precision, stored as a float.
    fn upload_blend_constants(&mut self, mode: u32) {
        let scale = f64::from(self.mem.read_f32(PIXEL_BYTE_SCALE));
        let unit = |byte: u32| (f64::from(byte) * scale) as f32;
        let signed = mode as i32;
        if (0xF..=0x17).contains(&signed) {
            let alpha = unit(self.blend_alpha() & 0xFF);
            self.device.set_pixel_constants(0, &[[alpha; 4]]);
        }
        if self.mem.read32(BATCH_EXTRA) & 0x4000_4000 != 0 {
            let colour = self.blend_alpha();
            let vector = [
                colour >> 24,
                (colour >> 16) & 0xFF,
                (colour >> 8) & 0xFF,
                colour & 0xFF,
            ]
            .map(unit);
            self.device.set_pixel_constants(1, &[vector]);
        }
        if self.mem.read32(BATCH_EXTRA) & 0x800_0000 != 0 {
            let colour = self.host.call(HostFunction::ConstantColour, &[]);
            let vector = [
                colour & 0xFF,
                (colour >> 8) & 0xFF,
                (colour >> 16) & 0xFF,
                colour >> 24,
            ]
            .map(unit);
            self.device.set_pixel_constants(6, &[vector]);
        }
        if mode == 0xA || mode == 0x13 || mode == 0xB {
            let half = self.mem.read_f32(0x0047_5CD4);
            self.device.set_pixel_constants(1, &[[half; 4]]);
        }
        if (self.mem.read32(BATCH_EXTRA) as i32) < 0 {
            let one = self.mem.read_f32(CONST_ONE);
            self.device.set_pixel_constants(1, &[[0.0, 0.0, one, 0.0]]);
        }
    }
}
