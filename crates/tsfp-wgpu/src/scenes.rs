// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic scenes recorded by running the real T1898 render code (`Renderer`) over a
//! [`Recorder`]. Everything here is FABRICATED input to MEASURED code: the guest tables the
//! original reads from the retail image (state slot to method header table, texture table,
//! vertex ring records, float constants) are written by [`Rig::new`] with the values the
//! slice's own tests use, and the textures are tiny generated images. No retail bytes.
//!
//! The recorded stream is what the wgpu replayer is developed against until streams recorded
//! from a running game exist (the host lane trace of docs/t-render-slice.md slice 2).
use tsfp_engine_render::Renderer;
use tsfp_engine_render::map::*;
use tsfp_engine_render::mem::GuestAccess;
use tsfp_engine_render::memory::GuestMemory;
use tsfp_engine_render::state::{StateSlot, slot};

use crate::stream::{Recorder, RecorderDevice, RecorderHost, SharedMemory, Stream};

/// Guest address of the fabricated vertex ring data (ring 0).
pub const RING_BASE: u32 = 0x0200_0000;
const TEXTURE_TABLE: u32 = 0x0300_0000;
const TEXTURE_RECORDS: u32 = 0x0300_1000;
const RECORD_SIZE: u32 = 0x34;

/// NV097 method headers (count 1) for the state slots, INFERRED from the slot names
/// (docs/t-render-slice.md). Slots not listed send `NO_OPERATION` (0x0100).
fn slot_method(slot_number: u32) -> u32 {
    match slot_number {
        slot::BLEND_ENABLE => 0x0304,
        slot::ALPHA_FUNC => 0x033C,
        slot::ALPHA_REF => 0x0340,
        slot::ALPHA_TEST_ENABLE => 0x0300,
        slot::BLEND_EQUATION => 0x0350,
        slot::BLEND_DESTINATION => 0x0348,
        slot::BLEND_SOURCE => 0x0344,
        slot::STENCIL_ENABLE => 0x032C,
        slot::STENCIL_FUNC => 0x0364,
        slot::STENCIL_FAIL_OP => 0x0370,
        slot::STENCIL_REF => 0x0368,
        slot::BLEND_COLOUR => 0x034C,
        slot::DEPTH_FUNC => 0x0354,
        slot::DEPTH_WRITE => 0x035C,
        _ => 0x0100,
    }
}

/// A rig that runs the render code and records a stream.
pub struct Rig {
    pub recorder: Recorder,
    pub memory: SharedMemory,
    pub device: RecorderDevice,
    pub host: RecorderHost,
    textures: u32,
}

impl Rig {
    pub fn new() -> Self {
        Self::on(Recorder::new())
    }

    /// The same rig over an existing recorder (a [`crate::live::LiveDevice`]'s, so the scene renders as it is built).
    pub fn on(recorder: Recorder) -> Self {
        let mut memory = recorder.memory();
        memory.write32(DIRTY_NEWEST, DIRTY_SENTINEL);
        memory.write32(DIRTY_OLDEST, DIRTY_SENTINEL);
        memory.write32(DIRTY_SENTINEL + 0x10, DIRTY_SENTINEL);
        memory.write32(DIRTY_SENTINEL + 0x14, DIRTY_SENTINEL);
        for index in 0..35u32 {
            memory.write32(StateSlot::address(index), index);
            memory.write32(
                METHOD_HEADER_TABLE + index * 4,
                0x4_0000 | slot_method(index),
            );
        }
        // Float constants of the original's data segment.
        memory.write_f32(CONST_ONE, 1.0);
        memory.write_f32(0x0047_5DC0, 1.0);
        memory.write_f32(0x0047_5DC4, 1.0);
        memory.write_f32(0x0047_5DBC, 0.5);
        memory.write_f32(PIXEL_BYTE_SCALE, 1.0 / 255.0);
        memory.write_f32(0x0047_5CD4, 0.5);
        // Vertex ring 0: buffer handle, used bytes, fence, pending, data base.
        recorder.set_lock_address(RING_BASE);
        memory.write32(VERTEX_RING_BASE, 0x1111);
        memory.write32(VERTEX_RING_BASE + 0x10, RING_BASE);
        memory.write32(VERTEX_RUN_START, u32::MAX);
        // Render target size for the scissor clamp.
        memory.write32(SCISSOR_TARGET_WIDTH, 640);
        memory.write32(SCISSOR_TARGET_HEIGHT, 480);
        // Texture table: records pointer, first valid index -1, count.
        memory.write32(TEXTURE_TABLE_POINTER, TEXTURE_TABLE);
        memory.write32(TEXTURE_TABLE, TEXTURE_RECORDS);
        memory.write32(TEXTURE_TABLE + 8, u32::MAX);
        memory.write32(CURRENT_TEXTURE_INDEX, u32::MAX);
        let device = recorder.device();
        let host = recorder.host();
        let mut rig = Self {
            recorder,
            memory,
            device,
            host,
            textures: 0,
        };
        // D3D8 device initialisation the game does outside the slot table: depth test on.
        use tsfp_engine_render::device::RenderDevice;
        rig.device.push_method(0x4_030C, 1);
        rig
    }

    pub fn renderer(&mut self) -> Renderer<'_, SharedMemory, RecorderDevice, RecorderHost> {
        Renderer::new(&mut self.memory, &mut self.device, &mut self.host)
    }

    /// Add texture table entry `index = previous count` with generated pixels and return its
    /// index. The record address is the texture id the stream carries.
    pub fn add_texture(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> u32 {
        let index = self.textures;
        let record = TEXTURE_RECORDS + index * RECORD_SIZE;
        self.memory.write32(record + 0x18, 0x0800_0000);
        self.memory.write8(record + 0x20, 1);
        self.memory.write8(record + 0x21, 1);
        for (offset, value) in [(0x24, width), (0x26, width), (0x28, height), (0x2A, height)] {
            self.memory.write8(record + offset, value as u8);
            self.memory.write8(record + offset + 1, (value >> 8) as u8);
        }
        self.memory.write_f32(record + 0x2C, 1.0);
        self.memory.write_f32(record + 0x30, 1.0);
        self.textures += 1;
        self.memory.write32(TEXTURE_TABLE + 0xC, self.textures);
        self.recorder.push_texture(record, width, height, rgba);
        index
    }

    /// Flush the pending draw and close the frame.
    pub fn present(&mut self) {
        self.renderer().draw_dispatch();
        self.recorder.present();
    }

    pub fn finish(&self) -> Stream {
        self.recorder.finish()
    }
}

impl Default for Rig {
    fn default() -> Self {
        Self::new()
    }
}

/// 32x32 gradient: red = x * 8, green = y * 8, blue 128, opaque.
pub fn gradient_texture() -> Vec<u8> {
    (0..32u32)
        .flat_map(|y| (0..32u32).flat_map(move |x| [(x * 8) as u8, (y * 8) as u8, 128, 255]))
        .collect()
}

/// 32x32 white disc with alpha fading to 0 at the rim.
pub fn disc_texture() -> Vec<u8> {
    (0..32u32)
        .flat_map(|y| {
            (0..32u32).flat_map(move |x| {
                let (dx, dy) = (x as f32 + 0.5 - 16.0, y as f32 + 0.5 - 16.0);
                let alpha = (1.0 - (dx * dx + dy * dy).sqrt() / 16.0).clamp(0.0, 1.0);
                [255, 255, 255, (alpha * 255.0) as u8]
            })
        })
        .collect()
}

/// 4x4 opaque white, so a draw shows the vertex colour exactly.
pub fn white_texture() -> Vec<u8> {
    vec![255; 4 * 4 * 4]
}

/// Texture indices of [`title_scene`].
pub struct TitleTextures {
    pub gradient: u32,
    pub disc: u32,
    pub white: u32,
}

/// A title/menu style frame: textured background, an alpha blended logo disc, a row of menu
/// panels (alpha blended, one additive highlight), and a scissored banner. Exercises the state
/// group and blend mode scripts, the dirty slot list, textured rect vertex rings and the
/// scissor. Screen is 640x480.
pub fn title_scene() -> Stream {
    let mut rig = Rig::new();
    build_title_scene(&mut rig);
    rig.finish()
}

/// Run the title scene calls on `rig` (offline into a stream or live into a renderer).
pub fn build_title_scene(rig: &mut Rig) {
    let textures = TitleTextures {
        gradient: rig.add_texture(32, 32, gradient_texture()),
        disc: rig.add_texture(32, 32, disc_texture()),
        white: rig.add_texture(4, 4, white_texture()),
    };
    {
        let mut renderer = rig.renderer();
        renderer.set_state_group(1);
        // Opaque background: mode 14 is source one, destination zero.
        renderer.set_blend_mode(14);
        renderer.select_current_texture_record(textures.gradient);
        renderer.draw_textured_rect(0.0, 0.0, 640.0, 480.0, 0, 0, 32, 32, 0xFFFF_FFFF);
        // Opaque grey slab, the known destination for the blend probes.
        renderer.select_current_texture_record(textures.white);
        renderer.draw_textured_rect(40.0, 300.0, 600.0, 460.0, 0, 0, 4, 4, 0xFF64_6464);
        // Logo: source alpha, inverse source alpha.
        renderer.set_blend_mode(1);
        renderer.select_current_texture_record(textures.disc);
        renderer.draw_textured_rect(200.0, 40.0, 440.0, 160.0, 0, 0, 32, 32, 0xFFFF_FFFF);
        // Menu panels: half transparent orange over the slab, then an additive highlight.
        renderer.select_current_texture_record(textures.white);
        renderer.draw_textured_rect(60.0, 320.0, 200.0, 380.0, 0, 0, 4, 4, 0x80FF_8000);
        renderer.set_blend_mode(2);
        renderer.draw_textured_rect(220.0, 320.0, 360.0, 380.0, 0, 0, 4, 4, 0x8020_4060);
        // Subtractive (reverse) mode 3 over the slab.
        renderer.set_blend_mode(3);
        renderer.draw_textured_rect(380.0, 320.0, 520.0, 380.0, 0, 0, 4, 4, 0x8020_4060);
        // Scissored banner: the rect is wider than the clip.
        renderer.set_blend_mode(14);
        renderer.set_scissor_clamped(60, 400, 300, 440);
        renderer.draw_textured_rect(0.0, 390.0, 640.0, 450.0, 0, 0, 4, 4, 0xFFC8_2828);
        renderer.set_scissor_clamped(0, 0, 640, 480);
    }
    rig.present();
}
