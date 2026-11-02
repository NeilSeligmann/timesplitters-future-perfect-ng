// SPDX-License-Identifier: GPL-3.0-or-later
//! Guest vertex runs to GPU vertices and NV2A primitives to triangle lists.
//!
//! The device never sees the FVF flags, only `BindVertexStream(base, stride)` and the draw
//! `start`/`count`. The layouts the HUD, menu and effect code queue are recognised by stride
//! (INFERRED, the flags `0xD` and `0x9` of `begin_primitive_batch` in tsfp-engine-render):
//!
//! | stride | flags | layout (little endian) |
//! | ---: | --- | --- |
//! | 24 | `0xD` | `x y z` f32, `diffuse` D3DCOLOR ARGB, `u v` f32 (draw_textured_rect) |
//! | 16 | `0x9` | `x y z` f32, `diffuse` (point mode sprite) |
//!
//! Positions are screen pixels with z as depth, mapped to clip space by the replayer (the
//! original does that in a vertex program, whose constants are not interpreted yet).
use crate::nv2a::argb_to_rgba;

/// A vertex as the GPU pipeline takes it: 84 bytes. `position.w` is 1 for the screen pixel stand-in layouts and the clip w of the
/// vertex program path. `uv_extra` are the texture coordinates of stages 1 to 3 (slice 4), zero outside the program path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: [f32; 4],
    pub colour: [f32; 4],
    pub uv: [f32; 2],
    pub uv_extra: [[f32; 2]; 3],
    /// Fog amount 0 to 1 (slice 6, from `oFog` and the library's fog state, 0 when fog is off).
    pub fog: f32,
    /// NV2A oD1, interpolated independently for combiner register V1.
    pub specular: [f32; 4],
}

impl Vertex {
    pub const SIZE: usize = 84;
    pub fn write(&self, out: &mut Vec<u8>) {
        for value in self
            .position
            .iter()
            .chain(&self.colour)
            .chain(&self.uv)
            .chain(self.uv_extra.iter().flatten())
        {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&self.fog.to_le_bytes());
        for value in self.specular {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
}

/// Strides [`decode_vertex`] understands.
pub fn stride_supported(stride: u32) -> bool {
    matches!(stride, 16 | 24)
}

/// Decode one vertex from `bytes` (at least `stride` long).
pub fn decode_vertex(bytes: &[u8], stride: u32) -> Option<Vertex> {
    let float = |offset: usize| {
        bytes
            .get(offset..offset + 4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let word = |offset: usize| {
        bytes
            .get(offset..offset + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let position = [float(0)?, float(4)?, float(8)?, 1.0];
    let colour = argb_to_rgba(word(12)?);
    let uv = if stride == 24 {
        [float(16)?, float(20)?]
    } else {
        [0.0, 0.0]
    };
    Some(Vertex {
        position,
        colour,
        uv,
        uv_extra: [[0.0; 2]; 3],
        fog: 0.0,
        specular: [0.0; 4],
    })
}

/// Triangle list indices for `count` vertices of NV2A primitive `primitive`
/// (5 triangles, 6 strip, 7 fan, 8 quads, 9 quad strip, 10 polygon). Points and lines give
/// `None`.
pub fn triangulate(primitive: u32, count: u32) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    match primitive {
        5 => out.extend(0..count / 3 * 3),
        6 => {
            for index in 0..count.saturating_sub(2) {
                if index % 2 == 0 {
                    out.extend([index, index + 1, index + 2]);
                } else {
                    out.extend([index + 1, index, index + 2]);
                }
            }
        }
        7 | 10 => {
            for index in 1..count.saturating_sub(1) {
                out.extend([0, index, index + 1]);
            }
        }
        8 => {
            for quad in 0..count / 4 {
                let base = quad * 4;
                out.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
        9 => {
            for pair in 0..count.saturating_sub(2) / 2 {
                let base = pair * 2;
                out.extend([base, base + 1, base + 3, base, base + 3, base + 2]);
            }
        }
        _ => return None,
    }
    Some(out)
}

/// Line segments (pairs of vertex positions) of the NV2A line primitives: 2 lines, 3 line loop, 4 line strip.
pub fn line_list(primitive: u32, count: u32) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    match primitive {
        2 => {
            for pair in 0..count / 2 {
                out.extend([pair * 2, pair * 2 + 1]);
            }
        }
        3 | 4 => {
            for index in 0..count.saturating_sub(1) {
                out.extend([index, index + 1]);
            }
            if primitive == 3 && count > 2 {
                out.extend([count - 1, 0]);
            }
        }
        _ => return None,
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quad_gives_two_triangles() {
        assert_eq!(triangulate(8, 4).unwrap(), vec![0, 1, 2, 0, 2, 3]);
        assert_eq!(triangulate(8, 7).unwrap().len(), 6);
    }

    #[test]
    fn strip_alternates_winding() {
        assert_eq!(triangulate(6, 4).unwrap(), vec![0, 1, 2, 2, 1, 3]);
    }

    #[test]
    fn points_have_no_triangles() {
        assert!(triangulate(1, 4).is_none());
    }

    #[test]
    fn line_primitives_give_segment_pairs() {
        assert_eq!(line_list(2, 5), Some(vec![0, 1, 2, 3]));
        assert_eq!(line_list(4, 4), Some(vec![0, 1, 1, 2, 2, 3]));
        assert_eq!(line_list(3, 3), Some(vec![0, 1, 1, 2, 2, 0]));
        assert_eq!(line_list(5, 3), None);
    }
}
