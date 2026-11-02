// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-engine-object/synth_leaves
//! Draw dispatch, texture and shader binding, scissor (T1898).
//!
//! The game batches indexed triangles and vertex runs into the pending draw state of
//! [`crate::map`] and flushes it with [`Renderer::draw_dispatch`] (0x22FF0) before anything
//! that changes device state. The flush draws the batch, uploads dirty matrix constants and
//! applies the dirty render state slots.
//!
//! Evidence class: differential versus the lifted original, see docs/t-render-slice.md.
use crate::{
    Renderer,
    device::{RenderDevice, RenderHost},
    map::*,
    mem::GuestAccess,
    memory::GuestMemory,
};

/// Original addresses of the functions ported in this module (T1898, see docs/t-render-slice.md).
pub const ROOTS: [u32; 8] = [
    0x22790, 0x2a630, 0x22950, 0x226e0, 0x22720, 0x204b0, 0x22ff0, 0x1a010,
];

/// A texture record (0x34 bytes, table at [`TEXTURE_TABLE_POINTER`]). INFERRED roles.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextureRecord {
    /// +0x18 flag word, bit 0x0800_0000 marks a texture with explicit pixel dimensions.
    pub flags: u32,
    /// +0x20 stage control selector 0 (zero or non zero).
    pub control_a: u8,
    /// +0x21 stage control selector 1.
    pub control_b: u8,
    /// +0x24 width in texels.
    pub width: u16,
    /// +0x28 height in texels.
    pub height: u16,
    /// +0x2C shader constant scale s.
    pub scale_s: f32,
    /// +0x30 shader constant scale t.
    pub scale_t: f32,
}

impl TextureRecord {
    pub fn read<M: GuestMemory>(memory: &M, address: u32) -> Self {
        Self {
            flags: memory.read32(address.wrapping_add(0x18)),
            control_a: memory.read8(address.wrapping_add(0x20)),
            control_b: memory.read8(address.wrapping_add(0x21)),
            width: memory.read16(address.wrapping_add(0x24)),
            height: memory.read16(address.wrapping_add(0x28)),
            scale_s: memory.read_f32(address.wrapping_add(0x2C)),
            scale_t: memory.read_f32(address.wrapping_add(0x30)),
        }
    }
}

impl<M: GuestMemory, D: RenderDevice, H: RenderHost> Renderer<'_, M, D, H> {
    /// 0x22FF0 d3d_draw_dispatch: flush the pending draw. Draws the index batch, or else the
    /// vertex run, uploads the dirty matrix constants and the constant block, then applies
    /// the dirty render state slots.
    pub fn draw_dispatch(&mut self) {
        if self.mem.read32(INDEX_FIRST) != self.mem.read32(INDEX_LAST) {
            self.prepare_draw();
            let first = self.mem.read32(INDEX_FIRST);
            let count = self.mem.read32(INDEX_LAST).wrapping_sub(first);
            if (count as i32) > 0 {
                let primitive = self.mem.read32(BATCH_PRIMITIVE);
                let indices = first.wrapping_mul(2).wrapping_add(INDEX_ARRAY);
                self.device.draw_indexed_vertices(primitive, count, indices);
            }
            let last = self.mem.read32(INDEX_LAST);
            self.mem.write32(INDEX_FLAG, 0);
            self.mem.write32(INDEX_FIRST, last);
        } else if self.mem.read32(VERTEX_RUN_START) != u32::MAX {
            self.prepare_draw();
            let start = self.mem.read32(VERTEX_RUN_START);
            let count = self.mem.read32(VERTEX_RUN_END).wrapping_sub(start);
            if (count as i32) > 0 {
                let primitive = self.mem.read32(BATCH_PRIMITIVE);
                self.device.draw_vertices(primitive, start, count);
            }
            let ring = self.mem.read32(VERTEX_RING_INDEX);
            let used_address = ring
                .wrapping_mul(VERTEX_RING_STRIDE)
                .wrapping_add(VERTEX_RING_BASE + 4);
            let used = self.mem.read32(used_address);
            self.mem.write32(VERTEX_RUN_START, u32::MAX);
            self.mem
                .write32(used_address, used.wrapping_add(0x1F) & !0x1F);
        }
        self.upload_dirty_matrices();
        if self.mem.read8(CONSTANT_BLOCK_DIRTY) != 0 {
            let words = self.mem.read_words(CONSTANT_BLOCK, 0x28);
            self.device.set_vertex_constants(0, &words);
            self.mem.write8(CONSTANT_BLOCK_DIRTY, 0);
        }
        self.state_apply();
    }

    /// The matrix part of the flush: runs of dirty 4x4 matrices (64 bytes each at
    /// [`MATRIX_STORE`]) go to vertex shader registers `0x3C + 4 * index`.
    fn upload_dirty_matrices(&mut self) {
        if self.mem.read8(MATRIX_DIRTY_ANY) == 0 {
            return;
        }
        let count = self.mem.read32(MATRIX_DIRTY_COUNT) as i32;
        if count >= 0 {
            let mut index: i32 = 0;
            while index <= count {
                if self
                    .mem
                    .read8(MATRIX_DIRTY_FLAGS.wrapping_add(index as u32))
                    != 0
                {
                    let run_start = index;
                    self.mem
                        .write8(MATRIX_DIRTY_FLAGS.wrapping_add(index as u32), 0);
                    index += 1;
                    while index <= count
                        && self
                            .mem
                            .read8(MATRIX_DIRTY_FLAGS.wrapping_add(index as u32))
                            != 0
                    {
                        self.mem
                            .write8(MATRIX_DIRTY_FLAGS.wrapping_add(index as u32), 0);
                        index += 1;
                    }
                    self.upload_matrix_run(run_start as u32, (index - run_start) as u32);
                }
                index += 1;
            }
        }
        self.mem.write8(MATRIX_DIRTY_ANY, 0);
        self.mem.write32(MATRIX_DIRTY_COUNT, 0);
    }

    fn upload_matrix_run(&mut self, first: u32, matrices: u32) {
        let source = MATRIX_STORE.wrapping_add(first.wrapping_shl(6));
        let register = first.wrapping_mul(4).wrapping_add(0x3C);
        let vectors = matrices.wrapping_shl(2);
        let words = self.mem.read_words(source, vectors.wrapping_shl(2));
        match vectors {
            1 => self.device.set_vertex_constant_one_vector(register, &words),
            4 => self
                .device
                .set_vertex_constant_four_vectors(register, &words),
            _ => self.device.set_vertex_constants(register, &words),
        }
    }

    /// 0x22790 d3d_texture_vs_constant_set: bind `texture` (a [`TextureRecord`] address or 0)
    /// to `stage`. Stage 0 also uploads the texture scale constants to vertex register 0xB.
    /// The per stage cache avoids redundant binds.
    pub fn bind_texture_with_constants(&mut self, texture: u32, stage: u32) {
        let texture = if stage == 1 && self.mem.read32(TEXTURE_CACHE) == 0 {
            0
        } else {
            texture
        };
        let cache_address = stage.wrapping_mul(4).wrapping_add(TEXTURE_CACHE);
        if self.mem.read32(cache_address) == texture {
            return;
        }
        if texture == 0 {
            self.device.set_texture(stage, 0);
            self.mem.write32(cache_address, 0);
            return;
        }
        if stage == 0 {
            let record = TextureRecord::read(&*self.mem, texture);
            let one = self.mem.read_f32(CONST_ONE);
            let (width, height) = if record.flags & 0x0800_0000 != 0 {
                (f32::from(record.width), f32::from(record.height))
            } else {
                (one, one)
            };
            let constants = [
                width.to_bits(),
                height.to_bits(),
                one.to_bits(),
                one.to_bits(),
                record.scale_s.to_bits(),
                record.scale_t.to_bits(),
                one.to_bits(),
                one.to_bits(),
            ];
            self.device.set_vertex_constants(0xB, &constants);
        }
        self.device.set_texture(stage, texture);
        let control_a = u32::from(self.mem.read8(texture.wrapping_add(0x20)));
        let filter_a = stage.wrapping_mul(4).wrapping_add(TEXTURE_FILTER_CACHE);
        if self.mem.read32(filter_a) != control_a {
            let mode = if control_a == 0 { 1 } else { 3 };
            self.device.defer_texture_stage_word(stage, 0, mode);
            self.mem.write32(filter_a, control_a);
        }
        let control_b = u32::from(self.mem.read8(texture.wrapping_add(0x21)));
        let filter_b = stage.wrapping_mul(4).wrapping_add(TEXTURE_FILTER_CACHE_B);
        if self.mem.read32(filter_b) != control_b {
            let mode = if control_b == 0 { 1 } else { 3 };
            self.device.defer_texture_stage_word(stage, 1, mode);
            self.mem.write32(filter_b, control_b);
        }
        self.mem.write32(cache_address, texture);
    }

    /// 0x22950: 1 when the two texture pair slots name the same object, else 0 (INFERRED).
    pub fn texture_pair_matches(&self) -> u32 {
        let first = self.mem.read32(TEXTURE_PAIR_A);
        u32::from(self.mem.read32(TEXTURE_PAIR_B) == self.mem.read32(first.wrapping_add(4)))
    }

    /// 0x2A630 d3d_texture_slot_vs_constant_set_by_index: bind the texture table entry
    /// `index` (EAX) to `stage` (EDX). -1 and out of range indices unbind. Indices -2, -5 and
    /// below -5 select the special records (override, pair tail, pair head), -3, -4, -8 do
    /// nothing.
    pub fn bind_texture_by_index(&mut self, index: u32, stage: u32) {
        let signed = index as i32;
        let table = self.mem.read32(TEXTURE_TABLE_POINTER);
        if index == u32::MAX || signed >= self.mem.read32(table.wrapping_add(0xC)) as i32 {
            self.bind_texture_with_constants(0, stage);
            return;
        }
        if signed >= 0 {
            let record = index
                .wrapping_mul(0x34)
                .wrapping_add(self.mem.read32(table));
            self.bind_texture_with_constants(record, stage);
            return;
        }
        if signed == -8 {
            return;
        }
        match index.wrapping_add(5) {
            0 => {
                let pair = self.texture_pair_matches();
                let record = TEXTURE_PAIR_TAIL.wrapping_sub(pair.wrapping_mul(0x34));
                self.bind_texture_with_constants(record, stage);
            }
            1 | 2 => {}
            3 => {
                let override_index = self.mem.read32(TEXTURE_OVERRIDE_INDEX);
                let record = override_index
                    .wrapping_mul(0x34)
                    .wrapping_add(self.mem.read32(table));
                self.bind_texture_with_constants(record, stage);
            }
            _ => {
                let pair = self.texture_pair_matches();
                let record = pair.wrapping_mul(0x34).wrapping_add(TEXTURE_PAIR_RECORDS);
                self.bind_texture_with_constants(record, stage);
            }
        }
    }

    /// 0x226E0 d3d_vertex_shader_load_select: load vertex program `program` at `address`,
    /// remember it, and when `select` is non zero and a declaration is valid select the
    /// declaration block at [`DECLARATION_BLOCK`] for `address`.
    pub fn load_vertex_shader(&mut self, program: u32, address: u32, select: u32) {
        self.device.load_vertex_program(program, address);
        self.mem.write32(VERTEX_PROGRAM_CACHE, program);
        if select != 0 && self.mem.read32(DECLARATION_VALID) != 0 {
            self.device
                .select_vertex_declaration(DECLARATION_BLOCK, address);
        }
    }

    /// 0x22720 d3d_pixel_shader_program_set: bind pixel program `program` when it changed,
    /// then refresh the blend dependent methods.
    pub fn set_pixel_program(&mut self, program: u32) {
        if self.mem.read32(PIXEL_PROGRAM_CACHE) != program {
            self.device.set_pixel_shader(program);
            self.mem.write32(PIXEL_PROGRAM_CACHE, program);
        }
        self.refresh_blend_methods();
    }

    /// 0x204B0 d3d_select_vertex_shader_direct_wrapper: build the declaration block for the
    /// vertex `flags`, select it and emit the vertex binding block.
    pub fn select_vertex_format(&mut self, flags: u32) {
        self.build_vertex_declaration(DECLARATION_BLOCK, flags);
        self.device.select_vertex_declaration(DECLARATION_BLOCK, 0);
        self.mem.write32(DECLARATION_VALID, 0);
        self.emit_vertex_bindings(flags);
    }

    /// 0x1A010 d3d_set_scissor_clamped: set the scissor rectangle (left, top, right, bottom)
    /// clamped to the render target. Swapped corners fall back to the full extent, empty
    /// rectangles are widened by one and degenerate results are ignored.
    pub fn set_scissor_clamped(&mut self, left: i32, top: i32, right: i32, bottom: i32) {
        let width = self.mem.read32(SCISSOR_TARGET_WIDTH);
        let height = self.mem.read32(SCISSOR_TARGET_HEIGHT);
        self.draw_dispatch();
        let (mut left, mut right) = if left > right {
            (0, width as i32)
        } else {
            (left, right)
        };
        let (mut top, mut bottom) = if top > bottom {
            (0, height as i32)
        } else {
            (top, bottom)
        };
        if left == right {
            right = right.wrapping_add(1);
        }
        if top == bottom {
            bottom = bottom.wrapping_add(1);
        }
        left = left.max(0);
        right = (right as u32).min(width) as i32;
        top = top.max(0);
        bottom = (bottom as u32).min(height) as i32;
        if left == right || top == bottom {
            return;
        }
        let cached = self.mem.read32(SCISSOR_CACHE_LEFT) == left as u32
            && self.mem.read32(SCISSOR_CACHE_RIGHT) == right as u32
            && self.mem.read32(SCISSOR_CACHE_TOP) == top as u32
            && self.mem.read32(SCISSOR_CACHE_BOTTOM) == bottom as u32;
        if cached {
            return;
        }
        self.mem.write32(SCISSOR_CACHE_BOTTOM, bottom as u32);
        self.mem.write32(SCISSOR_CACHE_LEFT, left as u32);
        self.mem.write32(SCISSOR_CACHE_RIGHT, right as u32);
        self.mem.write32(SCISSOR_CACHE_TOP, top as u32);
        self.device.set_scissor(1, 0, &[[left, top, right, bottom]]);
    }
}
