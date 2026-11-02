//! Level of detail 0 mesh streams of the character model body (T1668, spec `docs/t1668-chr-body.md`).
//!
//! A part record's first four stream offsets (`PartRecord::lod_streams(0)[0..4]`, entry relative) address four
//! contiguous arrays in the model body (MEASURED on all 15259 parts with a mesh of the 516 models of `pak/chr.pak`):
//!
//! ```text
//! stream 0  header, 16 * k bytes: groups of 16 bytes made of one 4 byte value repeated four times (vertex shader
//!           constant splats: 0x802A.. / 0x302E4000 / 0x412 ...), meaning of the values INFERRED
//! stream 1  V * 16: position f32 x 3, then a u32 skin word (see below)
//! stream 2  V * 12: texture coordinate f32 x 3 (u, v, 1.0)
//! stream 3  V *  8: u32 packed normal (11/11/10 bit signed, x low bits), u32 animation node (matrix cell) of the vertex
//! ```
//! Draw groups: see [`DrawGroup`] and [`ChrModel::part_groups`].
//! Skin word (flags 0x42040 models, 495 of 516): `0xFFFFFFE0 + 4 * slot`, slot 0..2 is the vertex's entry in the part's
//! matrix palette of at most three nodes; slot and node are one to one inside a part. A rigid part has one node, equal to
//! the part's own cell (8573 of 15259 parts); 3530 parts blend two nodes and 3156 three. In 123 parts the word is not a function of the node (small
//! integers 0..5 on some vertices, INFERRED another encoding), so use [`Vertex::palette_slot`] as an option.
use crate::chrmodel::{ChrModel, HEADER_LEN};
use tsfp_format_core::Result;

/// One vertex across the three vertex streams.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub skin_word: u32,
    pub uv: [f32; 3],
    pub packed_normal: u32,
    pub node: u32,
}

impl Vertex {
    /// Palette slot of the skin word of the common encoding, `None` for another encoding.
    pub fn palette_slot(&self) -> Option<u32> {
        let slot = self.skin_word.wrapping_add(32);
        (slot % 4 == 0 && slot / 4 < 3).then_some(slot / 4)
    }

    pub fn normal(&self) -> [f32; 3] {
        unpack_normal(self.packed_normal)
    }
}

/// 11/11/10 bit signed normal (`x` in bits 0..11, `y` 11..22, `z` 22..32), each divided by its largest positive value.
pub fn unpack_normal(packed: u32) -> [f32; 3] {
    let field = |shift: u32, bits: u32| {
        let raw = (packed >> shift) & ((1 << bits) - 1);
        let signed = if raw >= 1 << (bits - 1) { raw as i32 - (1 << bits) } else { raw as i32 };
        signed as f32 / ((1 << (bits - 1)) - 1) as f32
    };
    [field(0, 11), field(11, 11), field(22, 10)]
}

/// The decoded level of detail 0 mesh of one part.
#[derive(Debug, Clone, PartialEq)]
pub struct PartMesh {
    pub header: Vec<u8>,
    pub vertices: Vec<Vertex>,
}

impl PartMesh {
    /// Re-encodes the four streams concatenated (header, positions, uvs, normals), which is the body byte range.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.header.clone();
        for v in &self.vertices {
            v.position.iter().for_each(|f| out.extend(f.to_le_bytes()));
            out.extend(v.skin_word.to_le_bytes());
        }
        for v in &self.vertices {
            v.uv.iter().for_each(|f| out.extend(f.to_le_bytes()));
        }
        for v in &self.vertices {
            out.extend(v.packed_normal.to_le_bytes());
            out.extend(v.node.to_le_bytes());
        }
        out
    }
}

impl ChrModel {
    fn body_slice(&self, offset: usize, len: usize) -> Result<&[u8]> {
        let start = offset.checked_sub(HEADER_LEN).ok_or("stream before the body")?;
        self.body.get(start..start.checked_add(len).ok_or("stream overflow")?).ok_or("stream outside the body")
    }

    /// Decodes the level of detail 0 mesh of a part (`None` for parts without one: roots and helpers).
    pub fn part_mesh(&self, part: usize) -> Result<Option<PartMesh>> {
        self.part_mesh_lod(part, 0)
    }

    /// Same for level of detail slot `lod` (0..3; slots 1 and 2 have the same layout, MEASURED on every record that has one).
    pub fn part_mesh_lod(&self, part: usize, lod: usize) -> Result<Option<PartMesh>> {
        if lod > 2 {
            return Err("lod slot out of range");
        }
        let [s0, s1, s2, s3, ..] = self.parts.records.get(part).ok_or("part out of range")?.lod_streams(lod);
        if s0 == 0 {
            return Ok(None);
        }
        let (s0, s1, s2, s3) = (s0 as usize, s1 as usize, s2 as usize, s3 as usize);
        if s1 < s0 || s2 < s1 || s3 < s2 || (s2 - s1) % 16 != 0 {
            return Err("streams are not contiguous");
        }
        let count = (s2 - s1) / 16;
        if s3 - s2 != count * 12 {
            return Err("uv stream length");
        }
        let (header, pos, uv, nrm) = (
            self.body_slice(s0, s1 - s0)?,
            self.body_slice(s1, count * 16)?,
            self.body_slice(s2, count * 12)?,
            self.body_slice(s3, count * 8)?,
        );
        let f = |b: &[u8], at: usize| f32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let u = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let vertices = (0..count)
            .map(|i| Vertex {
                position: [f(pos, 16 * i), f(pos, 16 * i + 4), f(pos, 16 * i + 8)],
                skin_word: u(pos, 16 * i + 12),
                uv: [f(uv, 12 * i), f(uv, 12 * i + 4), f(uv, 12 * i + 8)],
                packed_normal: u(nrm, 8 * i),
                node: u(nrm, 8 * i + 4),
            })
            .collect();
        Ok(Some(PartMesh { header: header.to_vec(), vertices }))
    }

    /// Bytes of the four streams as stored (header to the end of the normal stream), for round trip checks.
    pub fn part_stream_bytes(&self, part: usize) -> Result<Option<&[u8]>> {
        self.part_stream_bytes_lod(part, 0)
    }

    pub fn part_stream_bytes_lod(&self, part: usize, lod: usize) -> Result<Option<&[u8]>> {
        let Some(mesh) = self.part_mesh_lod(part, lod)? else { return Ok(None) };
        let start = self.parts.records[part].lod_streams(lod)[0] as usize;
        self.body_slice(start, mesh.encode().len()).map(Some)
    }
}

/// One draw group of a part (level of detail 0): a contiguous vertex range with its triangle strip.
///
/// Records (10 bytes) at the part's `+0x60` pointer, terminated by a record whose byte 3 is `>= 0x80`:
/// `u16 flag, u16 tag = (mode << 8) | vertex count, u16 group number, u16 first vertex, u16 0xFFFF`. The groups partition
/// the part's vertices (first vertex of group 0 is 0, each group starts where the last ended, the last ends at V) on all
/// 15259 meshes. The 8 byte entry `j` of the `+0xBC` table is `(pointer, count)` of group `j`'s strip, `count` u16
/// vertex indices (stitched with repeated indices, every index below V). Mode is 3 or 4 (NV2A primitive mode numbers of the
/// begin command, INFERRED); the low byte of the tag equals the vertex range length except for 375 groups of one or two vertices
/// (372 and 3) whose tag count is 0. `flag` (0..10) is UNRESOLVED (not the node count, not the palette slot).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawGroup {
    pub flag: u16,
    pub tag: u16,
    pub first_vertex: u16,
    /// Vertex count of the group (derived from the next group's start, the last group ends at V).
    pub vertex_count: u16,
    pub strip: Vec<u16>,
}

impl DrawGroup {
    pub fn mode(&self) -> u16 {
        self.tag >> 8
    }
}

impl ChrModel {
    /// The draw groups of a part's level of detail 0 mesh (`None` without a mesh).
    pub fn part_groups(&self, part: usize) -> Result<Option<Vec<DrawGroup>>> {
        self.part_groups_lod(part, 0)
    }

    /// Draw groups of level of detail slot `lod`: group records at `+0x60 + 4 lod`, strip table at `+0xBC + 4 lod`.
    pub fn part_groups_lod(&self, part: usize, lod: usize) -> Result<Option<Vec<DrawGroup>>> {
        let Some(mesh) = self.part_mesh_lod(part, lod)? else { return Ok(None) };
        let record = &self.parts.records[part];
        let (groups_at, table_at) = (record.raw_dword(0x60 + 4 * lod) as usize, record.raw_dword(0xBC + 4 * lod) as usize);
        let mut raw = Vec::new();
        loop {
            let at = groups_at + 10 * raw.len();
            let bytes = self.body_slice(at, 10)?;
            if bytes[3] >= 0x80 {
                break;
            }
            let h = |i: usize| u16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]);
            raw.push((h(0), h(1), h(2), h(3), h(4)));
            if raw.len() > 4096 {
                return Err("group list does not end");
            }
        }
        let vertex_total = mesh.vertices.len();
        let mut groups = Vec::with_capacity(raw.len());
        for (index, &(flag, tag, number, first, marker)) in raw.iter().enumerate() {
            let end = raw.get(index + 1).map_or(vertex_total, |next| next.3 as usize);
            if number as usize != index || marker != 0xFFFF || (first as usize) > end || end > vertex_total {
                return Err("group record inconsistent");
            }
            let entry = self.body_slice(table_at + 8 * index, 8)?;
            let pointer = u32::from_le_bytes(entry[0..4].try_into().unwrap()) as usize;
            let count = u32::from_le_bytes(entry[4..8].try_into().unwrap()) as usize;
            let strip = if count == 0 {
                Vec::new()
            } else {
                let data = self.body_slice(pointer, count * 2)?;
                let strip: Vec<u16> = data.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                if strip.iter().any(|&i| i as usize >= vertex_total) {
                    return Err("strip index outside the part");
                }
                strip
            };
            groups.push(DrawGroup { flag, tag, first_vertex: first, vertex_count: (end - first as usize) as u16, strip });
        }
        Ok(Some(groups))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_normal_axes_and_palette_slots() {
        assert_eq!(unpack_normal(0x3FF), [1.0, 0.0, 0.0]);
        assert_eq!(unpack_normal(0x1FF << 22), [0.0, 0.0, 1.0]);
        assert_eq!(unpack_normal(0x400 << 11)[1], -1024.0 / 1023.0);
        let v = |skin_word| Vertex { position: [0.0; 3], skin_word, uv: [0.0; 3], packed_normal: 0, node: 0 };
        assert_eq!(v(0xFFFF_FFE0).palette_slot(), Some(0));
        assert_eq!(v(0xFFFF_FFE8).palette_slot(), Some(2));
        assert_eq!(v(1).palette_slot(), None);
    }
}
