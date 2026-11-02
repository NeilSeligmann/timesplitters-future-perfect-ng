// SPDX-License-Identifier: GPL-3.0-or-later
//! Vertex ring, primitive batches, vertex declarations and the hand built quad and sprite
//! batches (T1898).
//!
//! `begin_primitive_batch` (0x18EA0) reserves vertices in the two ring buffers and points the
//! vertex cursor [`VERTEX_CURSOR`] at them. The batch functions write vertices at the cursor
//! and `draw_dispatch` later draws the run.
//!
//! Evidence class: differential versus the lifted original, see docs/t-render-slice.md.
use crate::{
    Renderer,
    device::{HostFunction, RenderDevice, RenderHost},
    gprs::Gprs,
    map::*,
    mem::GuestAccess,
    memory::GuestMemory,
    vertex_format::vertex_format_size,
};
use alloc::vec::Vec;

/// Original addresses of the functions ported in this module (T1898, see docs/t-render-slice.md).
pub const ROOTS: [u32; 13] = [
    0x1e770, 0x1e7d0, 0x1e7f0, 0x1e970, 0x1eb00, 0x18de0, 0x18ea0, 0x1b7b0, 0x1d5f0, 0x1ed50,
    0x1c720, 0x389b0, 0x459d0,
];

/// Vertex array format words are `(stride_byte << 8) | type`. Types as stored by the
/// declaration builder (INFERRED from the NV2A vertex attribute formats).
pub mod attribute {
    pub const UNUSED: u32 = 2;
    pub const BYTE4_COLOUR: u32 = 0x25;
    pub const FLOAT3: u32 = 0x32;
    pub const FLOAT2: u32 = 0x22;
    pub const SHORT2: u32 = 0x16;
    pub const FLOAT1_PACKED: u32 = 0x40;
}

/// 0x1E770 game_u32_map_1_4_6_5_2_8_custom_eax: map a game primitive kind to the device
/// primitive type, `u32::MAX` when out of range. Register convention (EAX in and out).
pub fn primitive_type_for_kind(kind: u32) -> u32 {
    match kind {
        0 => 1,
        1 => 4,
        2 => 6,
        3 => 5,
        4 => 2,
        5..=7 => 8,
        _ => u32::MAX,
    }
}

/// Device primitive type chosen by `begin_primitive_batch` and whether it forces a flush.
fn batch_primitive(kind: u32, extra: u32) -> (u32, bool) {
    match kind {
        1 if extra == 2 => (2, false),
        1 => (4, true),
        2 if extra == 3 => (5, false),
        2 => (6, true),
        3 => (5, false),
        5 => (1, false),
        6 => (8, false),
        7 => (9, false),
        8 => (2, false),
        _ => (kind, false),
    }
}

impl<M: GuestMemory, D: RenderDevice, H: RenderHost> Renderer<'_, M, D, H> {
    /// 0x1E7D0 game_d3d_draw_indexed_vertices_call: draw `count` indices at `indices` with
    /// the device primitive for game primitive `kind`. Returns 1.
    pub fn draw_indexed_kind(&mut self, kind: u32, indices: u32, count: u32) -> u32 {
        let primitive = primitive_type_for_kind(kind);
        self.device.draw_indexed_vertices(primitive, count, indices);
        1
    }

    /// 0x1E7F0: set vertex stream binding `index` (0..8) to `address` with attribute byte
    /// `stride_byte`. Index 5 and values above 8 do nothing.
    pub fn set_stream_binding(&mut self, index: u32, address: u32, stride_byte: u8) {
        let slot = match index {
            0 => 0x55F9BC,
            1 => 0x55F9C4,
            2 => 0x55F9CC,
            3 => 0x55F9D4,
            4 => 0x55F9DC,
            6 => 0x55F9E4,
            7 => 0x55F9EC,
            8 => 0x55F9F4,
            _ => return,
        };
        self.mem.write32(slot, address);
        self.mem.write8(slot + 4, stride_byte);
    }

    /// 0x1E970 game_d3d_vertex_declaration_build_from_flags: fill the 16 entry (16 bytes
    /// each) declaration block at `block` for the vertex format `flags`. Each entry holds a
    /// byte offset at +4 and an attribute type at +8, unused entries keep type 2.
    pub fn build_vertex_declaration(&mut self, block: u32, flags: u32) {
        for entry in 0..16u32 {
            let base = block.wrapping_add(entry * 0x10);
            self.mem.write32(base, 0);
            self.mem.write32(base.wrapping_add(8), attribute::UNUSED);
        }
        let mut offset = 0u32;
        let put = |memory: &mut M, entry: u32, kind: u32, size: u32, offset: &mut u32| {
            let base = block.wrapping_add(entry * 0x10);
            memory.write32(base.wrapping_add(4), *offset);
            memory.write32(base.wrapping_add(8), kind);
            *offset = offset.wrapping_add(size);
        };
        if flags & 0x10 != 0 {
            put(self.mem, 0, attribute::BYTE4_COLOUR, 4, &mut offset);
        }
        if flags & 0x1 != 0 {
            put(self.mem, 1, attribute::FLOAT3, 12, &mut offset);
        }
        if flags & 0x40 != 0 {
            put(self.mem, 5, attribute::FLOAT3, 12, &mut offset);
        } else if flags & 0x8000 != 0 {
            put(self.mem, 5, attribute::SHORT2, 4, &mut offset);
        }
        if flags & 0x8 != 0 {
            put(self.mem, 2, attribute::FLOAT1_PACKED, 4, &mut offset);
        }
        if flags & 0x4 != 0 {
            put(self.mem, 3, attribute::FLOAT2, 8, &mut offset);
        } else if flags & 0x80 != 0 {
            put(self.mem, 3, attribute::FLOAT3, 12, &mut offset);
        }
        if flags & 0x200 != 0 {
            put(self.mem, 8, attribute::FLOAT2, 8, &mut offset);
        } else if flags & 0x1000 != 0 {
            put(self.mem, 8, attribute::FLOAT3, 12, &mut offset);
        }
        if flags & 0x400 != 0 {
            put(self.mem, 9, attribute::FLOAT2, 8, &mut offset);
        } else if flags & 0x2000 != 0 {
            put(self.mem, 9, attribute::FLOAT3, 12, &mut offset);
        }
        if flags & 0x800 != 0 {
            put(self.mem, 10, attribute::FLOAT2, 8, &mut offset);
        } else if flags & 0x4000 != 0 {
            put(self.mem, 10, attribute::FLOAT3, 12, &mut offset);
        }
        if flags & 0x20 != 0 {
            put(self.mem, 4, attribute::FLOAT2, 8, &mut offset);
        }
        if flags & 0x100 != 0 {
            put(self.mem, 6, attribute::FLOAT3, 12, &mut offset);
            put(self.mem, 7, attribute::FLOAT3, 12, &mut offset);
        }
        if flags & 0x1_0000 != 0 {
            put(self.mem, 6, attribute::SHORT2, 4, &mut offset);
            put(self.mem, 7, attribute::SHORT2, 4, &mut offset);
        }
        if flags & 0x2_0000 != 0 {
            put(self.mem, 12, attribute::FLOAT1_PACKED, 4, &mut offset);
        }
    }

    /// 0x1EB00 game_d3d_push_buffer_emit: write the 34 dword vertex binding block into the
    /// push buffer: 16 array formats from the stream strides and the declaration `flags`,
    /// then 16 array offsets from the stream addresses (high bit stripped). The device seam
    /// receives these as typed arrays so the native renderer need not build a raw packet.
    pub fn emit_vertex_bindings(&mut self, flags: u32) {
        let stride_of =
            |renderer: &Self, byte_address: u32| u32::from(renderer.mem.read8(byte_address)) << 8;
        let unused = attribute::UNUSED;
        let mut formats = [unused; 16];
        if flags & 0x10 != 0 {
            formats[0] = stride_of(self, 0x55F9E8) | attribute::BYTE4_COLOUR;
        }
        if flags & 0x1 != 0 {
            formats[1] = stride_of(self, 0x55F9C0) | attribute::FLOAT3;
        }
        if flags & 0x8 != 0 {
            formats[2] = stride_of(self, 0x55F9D0) | attribute::FLOAT1_PACKED;
        }
        if flags & 0x4 != 0 {
            formats[3] = stride_of(self, 0x55F9C8) | attribute::FLOAT2;
        } else if flags & 0x80 != 0 {
            formats[3] = stride_of(self, 0x55F9C8) | attribute::FLOAT3;
        }
        if flags & 0x40 != 0 {
            formats[5] = stride_of(self, 0x55F9E0) | attribute::FLOAT3;
        } else if flags & 0x8000 != 0 {
            formats[5] = stride_of(self, 0x55F9E0) | attribute::SHORT2;
        }
        if flags & 0x100 != 0 {
            formats[6] = stride_of(self, 0x55F9F0) | attribute::FLOAT3;
            formats[7] = stride_of(self, 0x55F9F8) | attribute::FLOAT3;
        }
        if flags & 0x2_0000 != 0 {
            formats[12] = stride_of(self, 0x55F9D8) | attribute::FLOAT1_PACKED;
        }
        let strip = |renderer: &Self, address: u32| renderer.mem.read32(address) & 0x7FFF_FFFF;
        let mut offsets = [0u32; 16];
        offsets[0] = strip(self, 0x55F9E4);
        offsets[1] = strip(self, 0x55F9BC);
        offsets[2] = strip(self, 0x55F9CC);
        offsets[3] = strip(self, 0x55F9C4);
        offsets[5] = strip(self, 0x55F9DC);
        offsets[6] = strip(self, 0x55F9EC);
        offsets[7] = strip(self, 0x55F9F4);
        offsets[12] = strip(self, 0x55F9D4);
        self.device.set_vertex_bindings(formats, offsets);
    }

    /// 0x18DE0 game_d3d_vertex_buffer_lock_fenced: reserve `count` vertices of `stride` bytes
    /// in the current ring buffer. When the ring is full it fences, switches to the other
    /// ring (waiting for its fence) and starts at 0. Returns the data pointer and the index
    /// of the first reserved vertex (the original stores the index through a pointer).
    pub fn lock_vertex_ring(&mut self, stride: u32, count: u32) -> (u32, u32) {
        let ring = self.mem.read32(VERTEX_RING_INDEX);
        let mut record = ring
            .wrapping_mul(VERTEX_RING_STRIDE)
            .wrapping_add(VERTEX_RING_BASE);
        let mut used = self.mem.read32(record.wrapping_add(4));
        assert!(
            stride != 0,
            "vertex stride 0 divides by zero in the original"
        );
        let remainder = (used as i32).wrapping_rem(stride as i32);
        if remainder != 0 {
            used = used.wrapping_add(stride.wrapping_sub(remainder as u32));
        }
        let reserve = stride.wrapping_mul(count);
        let end = reserve.wrapping_add(used);
        let (first_index, new_end) = if (end as i32) >= 0x8_0000 {
            self.draw_dispatch();
            let fence = self.device.insert_fence();
            let pending = self.mem.read32(record.wrapping_add(0xC));
            self.mem
                .write32(record.wrapping_add(0xC), pending.wrapping_add(1));
            self.mem.write32(record.wrapping_add(8), fence);
            let next = (ring as i32).wrapping_add(1).wrapping_rem(2) as u32;
            record = next
                .wrapping_mul(VERTEX_RING_STRIDE)
                .wrapping_add(VERTEX_RING_BASE);
            self.mem.write32(VERTEX_RING_INDEX, next);
            if (self.mem.read32(record.wrapping_add(0xC)) as i32) > 0 {
                let waiting = self.mem.read32(record.wrapping_add(8));
                self.device.block_until_fence(waiting);
            }
            used = 0;
            self.mem.write32(record.wrapping_add(4), 0);
            (0, reserve)
        } else {
            ((used as i32).wrapping_div(stride as i32) as u32, end)
        };
        let base = self.mem.read32(record.wrapping_add(0x10));
        self.mem.write32(VERTEX_BUFFER_BASE_SLOT, base);
        let buffer = self.mem.read32(record);
        let data = self.device.lock_vertex_buffer(buffer, 0x30);
        self.mem.write32(record.wrapping_add(4), new_end);
        (data.wrapping_add(used), first_index)
    }

    /// 0x18EA0 game_d3d_primitive_batch_begin: start (or extend) a vertex run of `count`
    /// vertices for primitive `kind` (1..8, see [`batch_primitive`]), vertex format `format`
    /// and `extra`. Flushes when the primitive, format or pending state changed, locks the
    /// ring and points the vertex cursor at the reservation. Returns the run end index.
    pub fn begin_primitive_batch(&mut self, kind: u32, format: u32, extra: u32, count: u32) -> u32 {
        let (primitive, forced) = batch_primitive(kind, count);
        let pending = forced
            || self.mem.read32(INDEX_FIRST) != self.mem.read32(INDEX_LAST)
            || self.mem.read8(MATRIX_DIRTY_ANY) != 0
            || self.mem.read8(CONSTANT_BLOCK_DIRTY) != 0
            || self.dirty_list_has_entries();
        if primitive != self.mem.read32(BATCH_PRIMITIVE) || pending {
            self.draw_dispatch();
            self.mem.write32(BATCH_PRIMITIVE, primitive);
        }
        if self.mem.read32(BATCH_FORMAT) != format || self.mem.read32(BATCH_EXTRA) != extra {
            self.draw_dispatch();
            self.mem.write32(BATCH_FORMAT, format);
            self.mem.write32(BATCH_EXTRA, extra);
            let stride = vertex_format_size(format, Gprs::default()).eax;
            self.mem.write32(BATCH_STRIDE, stride);
        }
        let stride = self.mem.read32(BATCH_STRIDE);
        let (cursor, first_index) = self.lock_vertex_ring(stride, count);
        self.mem.write32(VERTEX_CURSOR, cursor);
        if self.mem.read32(VERTEX_RUN_START) == u32::MAX {
            self.mem.write32(VERTEX_RUN_START, first_index);
            let end = first_index.wrapping_add(count);
            self.mem.write32(VERTEX_RUN_END, end);
            end
        } else {
            let end = self.mem.read32(VERTEX_RUN_END).wrapping_add(count);
            self.mem.write32(VERTEX_RUN_END, end);
            end
        }
    }

    /// Append `words` at the vertex cursor and advance it.
    pub(crate) fn emit_words(&mut self, words: &[u32]) {
        let mut cursor = self.mem.read32(VERTEX_CURSOR);
        for &word in words {
            self.mem.write32(cursor, word);
            cursor = cursor.wrapping_add(4);
        }
        self.mem.write32(VERTEX_CURSOR, cursor);
    }

    /// 0x459D0, a tail jump to 0x389B0: the end of a hand built batch. Both are a bare `ret`
    /// in the retail image (MEASURED in the lifted code), so nothing happens. Kept as a named
    /// seam because the debug build of the title unlocked the vertex buffer here.
    pub(crate) fn end_batch(&mut self) {}

    pub(crate) fn add_draw_statistics(&mut self, vertices: u32, triangles: u32, batches: u32) {
        let add = |memory: &mut M, address: u32, amount: u32| {
            let value = memory.read32(address);
            memory.write32(address, value.wrapping_add(amount));
        };
        add(self.mem, STAT_VERTICES, vertices);
        add(self.mem, STAT_TRIANGLES, triangles);
        add(self.mem, STAT_BATCHES, batches);
    }

    /// 0x1B7B0 game_d3d_draw_textured_rect_scaled_by_stage0_texture_size: queue one screen
    /// quad. `left..bottom` are the float corners, `u0..v1` texel coordinates scaled by the
    /// reciprocal size of the current texture record, `colour` the diffuse word.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_textured_rect(
        &mut self,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        u0: i32,
        v0: i32,
        u1: i32,
        v1: i32,
        colour: u32,
    ) {
        let record = self.mem.read32(CURRENT_TEXTURE_RECORD);
        let (scale_u, scale_v) = if record != 0 {
            let width = f32::from(self.mem.read16(record.wrapping_add(0x26)));
            let height = f32::from(self.mem.read16(record.wrapping_add(0x2A)));
            let texel = self.mem.read_f32(0x0047_5DC0);
            let one = self.mem.read_f32(CONST_ONE);
            (one / (width * texel), one / (height * texel))
        } else {
            let fallback = self.mem.read_f32(0x0047_5DC4);
            (fallback, fallback)
        };
        let u0 = u0 as f32 * scale_u;
        let v0 = v0 as f32 * scale_v;
        let u1 = u1 as f32 * scale_u;
        let v1 = v1 as f32 * scale_v;
        self.begin_primitive_batch(6, 0xD, 0x35, 4);
        let depth = self.mem.read32(0x0047_5DBC);
        let vertex = |x: f32, y: f32, u: f32, v: f32| {
            [
                x.to_bits(),
                y.to_bits(),
                depth,
                colour,
                u.to_bits(),
                v.to_bits(),
            ]
        };
        let mut words = Vec::with_capacity(24);
        words.extend(vertex(left, top, u0, v0));
        words.extend(vertex(right, top, u1, v0));
        words.extend(vertex(right, bottom, u1, v1));
        words.extend(vertex(left, bottom, u0, v1));
        self.emit_words(&words);
        self.end_batch();
        self.add_draw_statistics(2, 2, 2);
    }

    /// 0x1D5F0: queue one point sprite at (`x`, `y`, `z`) with `colour`. Two paths, a single
    /// point vertex when [`BATCH_POINT_MODE`] is set, else a four vertex quad whose corners
    /// carry the ids 0xFFFFFFBC..BF, the colour and the two size words.
    pub fn draw_point_sprite(
        &mut self,
        x: f32,
        y: f32,
        z: f32,
        colour: u32,
        size_x: f32,
        size_y: f32,
    ) {
        if self.mem.read32(BATCH_POINT_MODE) != 0 {
            if self.mem.read32(BATCH_OPEN) == 0 {
                self.begin_primitive_batch(5, 9, 0x12, 1);
            }
            self.emit_words(&[x.to_bits(), y.to_bits(), z.to_bits(), colour]);
        } else {
            if self.mem.read32(BATCH_OPEN) == 0 {
                self.begin_primitive_batch(6, 0x1D, 0x56, 4);
            }
            let adjusted = size_y - self.mem.read_f32(0x0047_5CE4);
            let mut words = Vec::with_capacity(28);
            for corner in 0xFFFF_FFBCu32..=0xFFFF_FFBF {
                words.extend([
                    corner,
                    x.to_bits(),
                    y.to_bits(),
                    z.to_bits(),
                    colour,
                    adjusted.to_bits(),
                    size_x.to_bits(),
                ]);
            }
            self.emit_words(&words);
            self.add_draw_statistics(4, 2, 2);
        }
        if self.mem.read32(BATCH_OPEN) == 0 {
            self.end_batch();
        }
    }

    /// 0x1C720 (reached through 0x1ED50 game_d3d_current_record_select_wrapper_hud_draw):
    /// select texture table entry `index` as the current record. Indices outside
    /// `(table[+8], table[+0xC])` go to the fallback selector.
    pub fn select_current_texture_record(&mut self, index: u32) {
        let table = self.mem.read32(TEXTURE_TABLE_POINTER);
        let signed = index as i32;
        let valid = signed >= 0
            && signed < self.mem.read32(table.wrapping_add(0xC)) as i32
            && signed > self.mem.read32(table.wrapping_add(8)) as i32;
        if !valid {
            self.host.call(
                HostFunction::TextureSelectFallback,
                &[index, u32::MAX, u32::MAX, 0, 0],
            );
            return;
        }
        if index == self.mem.read32(CURRENT_TEXTURE_INDEX) {
            return;
        }
        self.draw_dispatch();
        let table = self.mem.read32(TEXTURE_TABLE_POINTER);
        let record = index
            .wrapping_mul(0x34)
            .wrapping_add(self.mem.read32(table));
        self.mem.write32(CURRENT_TEXTURE_INDEX, index);
        self.mem.write32(CURRENT_TEXTURE_RECORD, record);
    }
}
