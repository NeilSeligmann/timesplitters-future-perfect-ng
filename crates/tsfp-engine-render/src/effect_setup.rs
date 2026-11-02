// SPDX-License-Identifier: GPL-3.0-or-later
//! Effect setup functions of the d3d layer (T1898 wave 3). Each binds the texture of the
//! current effect, uploads the vertex shader constants the effect prepared in the caches at
//! `0x665268..0x665274` and `0x665308`, selects the pixel program `0x4B9330` and loads a vertex
//! program. docs/t-d3d-effect-setup.md lists the family of 56.
//!
//! Evidence class: differential versus the lifted original, see docs/t-render-slice.md.
use alloc::vec;
use crate::{
    Renderer,
    device::{HostArg, HostFunction, RenderDevice, RenderHost},
    mem::GuestAccess,
    memory::GuestMemory,
};

/// Original addresses of the functions ported in this module (T1898, see docs/t-render-slice.md).
pub const ROOTS: [u32; 3] = [0x2a900, 0x2abe0, 0x2ad60];

const EFFECT_TEXTURE_INDEX: u32 = 0x0066_52E0;
const CONSTANT_SOURCES: u32 = 0x0066_5268;
const CONSTANT_BLOCK: u32 = 0x0066_5308;
const PIXEL_PROGRAM: u32 = 0x004B_9330;

impl<M: GuestMemory, D: RenderDevice, H: RenderHost> Renderer<'_, M, D, H> {
    /// Four vector constants from the pointer cached at `0x665268 + 4 * slot` when it is set.
    fn upload_cached_matrix(&mut self, slot: u32, first_register: u32) {
        let source = self.mem.read32(CONSTANT_SOURCES + 4 * slot);
        if source != 0 {
            let words = self.mem.read_words(source, 0x10);
            self.device
                .set_vertex_constants_uncached(first_register, &words);
        }
    }

    /// Like [`Self::upload_cached_matrix`] but uploads the transpose (0x1530E0) of the matrix.
    fn upload_cached_matrix_transposed(&mut self, slot: u32, first_register: u32) {
        let source = self.mem.read32(CONSTANT_SOURCES + 4 * slot);
        if source != 0 {
            let mut buffers = [vec![0u32; 16]];
            self.host.call_buffers(
                HostFunction::MatrixTranspose,
                &[HostArg::Buffer(0), HostArg::Word(source)],
                &mut buffers,
            );
            self.device
                .set_vertex_constants_uncached(first_register, &buffers[0]);
        }
    }

    fn finish_effect_setup(&mut self, vertex_program: u32) {
        self.set_pixel_program(PIXEL_PROGRAM);
        self.load_vertex_shader(vertex_program, 0, 1);
    }

    /// 0x2A900 game_d3d_effect_setup_texture_slot0_constant_ps_program_vs_select: texture to
    /// stage 0, constants `0x44` from the cached pointer `0x665270`, vertex program `0x4B9190`.
    pub fn effect_setup_texture_and_constant_44(&mut self) {
        let texture = self.mem.read32(EFFECT_TEXTURE_INDEX);
        self.bind_texture_by_index(texture, 0);
        self.upload_cached_matrix(2, 0x44);
        self.finish_effect_setup(0x004B_9190);
    }

    /// 0x2ABE0 game_d3d_effect_setup_variant_4b9330_b: as above plus constants `0x40` from
    /// `0x66526C` and the ten vector constant block `0x665308` at register 0, vertex program
    /// `0x4B9510`.
    pub fn effect_setup_variant_b(&mut self) {
        let texture = self.mem.read32(EFFECT_TEXTURE_INDEX);
        self.bind_texture_by_index(texture, 0);
        self.upload_cached_matrix(1, 0x40);
        self.upload_cached_matrix(2, 0x44);
        let block = self.mem.read_words(CONSTANT_BLOCK, 0x28);
        self.device.set_vertex_constants_uncached(0, &block);
        self.finish_effect_setup(0x004B_9510);
    }

    /// 0x2AD60 game_d3d_effect_setup_variant_4b9330_transposed_c: constants `0x3C` from
    /// `0x665268`, the transposes of the matrices at `0x66526C`, `0x665270` and `0x665274` to
    /// registers `0x40`, `0x44` and `0x48`, the constant block, vertex program `0x4B9990`.
    pub fn effect_setup_variant_transposed(&mut self) {
        let texture = self.mem.read32(EFFECT_TEXTURE_INDEX);
        self.bind_texture_by_index(texture, 0);
        self.upload_cached_matrix(0, 0x3C);
        self.upload_cached_matrix_transposed(1, 0x40);
        self.upload_cached_matrix_transposed(2, 0x44);
        self.upload_cached_matrix_transposed(3, 0x48);
        let block = self.mem.read_words(CONSTANT_BLOCK, 0x28);
        self.device.set_vertex_constants_uncached(0, &block);
        self.finish_effect_setup(0x004B_9990);
    }
}
