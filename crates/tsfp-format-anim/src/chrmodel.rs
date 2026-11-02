//! Character model container of `pak/chr.pak` (T1875, spec in `docs/t-rust-anim.md` section 8).
//!
//! 516 of the 911 entries of `pak/chr.pak` are models (the others are 4224/32896 byte textures and small blobs). Layout
//! (MEASURED on all 516, loader `0x96410` `game_model_xbr_load_or_fail` and the pointer fixup `0x8DF60`):
//!
//! ```text
//! 0x00  u32 descriptor offset (always 12: the runtime stores base + 12 at info+0x10; the block holds the shared
//!       header: 4 identifier pairs, vertex format words, quantised vertex data ... opaque here)
//! 0x04  u32 info offset A         (runtime: the model pointer, [model+4] of the table row)
//! 0x08  u32 slot table offset B   (always A + 76; runtime: info+0x14)
//! 0x0C  body: mesh streams, vertex data, per part sub structures (opaque, referenced by part pointer fields)
//! A-0xC8*F  F part records (F = info dword 0), the skeleton tree, 0xC8 bytes each (see `skeleton`)
//! A     the 76 byte model info (`ModelInfo`)
//! B     slot records of 48 bytes to the end of the entry (index, three 0xFFFFFFFF runtime slots, zero fill; the last
//!       record is an all 0xFFFFFFFF terminator in the data seen)
//! ```
//! All offsets stored in the blob are relative to the entry start; the loader adds the base address (`0x8DF60`).
use crate::skeleton::{PART_LEN, PartTable};
use tsfp_format_core::{Result, range, word};

pub const HEADER_LEN: usize = 12;
pub const INFO_LEN: usize = 76;
pub const SLOT_LEN: usize = 48;

/// The 76 byte model info kept verbatim (a runtime model state follows it in memory: `0x966E0` writes the animation
/// state at `+0xD4`/`+0x12C`, so the 48 byte slot records after the info are overlaid with it at runtime).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    pub raw: [u8; INFO_LEN],
}

impl ModelInfo {
    /// `+0x00` number of part records F (the table ends at the info).
    pub fn part_count(&self) -> u32 {
        crate::record_dword(&self.raw, 0)
    }
    /// `+0x04` number of animation nodes N: the parts of kind 0 or 2, whose matrix cells are 0..N-1 (MEASURED 516/516).
    /// A clip drives the model when its node count equals N.
    pub fn anim_nodes(&self) -> u32 {
        crate::record_dword(&self.raw, 4)
    }
    /// `+0x08` is 1 on every model (INFERRED version).
    pub fn version(&self) -> u32 {
        crate::record_dword(&self.raw, 8)
    }
    /// `+0x0C` flags: bit 4 quantised 16 bit vertex positions, bit 6 packed 11/11/10 normals (`0x12860`, `0x12B20`),
    /// bit 23 set by the loader once the fixup ran. Seen on disc: 0x42040, 0x40, 0x40040, 0x2040, 0x41040.
    pub fn flags(&self) -> u32 {
        crate::record_dword(&self.raw, 0x0C)
    }
    /// `+0x30` f32 size scale, 1.0 when zero (`0x8E5F0`).
    pub fn scale(&self) -> f32 {
        f32::from_bits(crate::record_dword(&self.raw, 0x30))
    }
    /// `+0x38..+0x44` quantisation origin (`0x12860`, `0x8E630`).
    pub fn origin(&self) -> [f32; 3] {
        std::array::from_fn(|i| f32::from_bits(crate::record_dword(&self.raw, 0x38 + 4 * i)))
    }
    /// `+0x44` blob relative pointer rebased by the fixup (zero on most models).
    pub fn pointer_44(&self) -> u32 {
        crate::record_dword(&self.raw, 0x44)
    }
    /// `+0x48` (16 on 495 of 516 models, 13 on four; meaning unknown).
    pub fn word_48(&self) -> u32 {
        crate::record_dword(&self.raw, 0x48)
    }
}

/// A parsed character model. `encode` returns the exact input bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChrModel {
    pub descriptor_offset: u32,
    /// Bytes from the end of the 12 byte header to the first part record.
    pub body: Vec<u8>,
    pub parts: PartTable,
    pub info: ModelInfo,
    /// Bytes between the info and the slot table (empty on every model seen).
    pub gap: Vec<u8>,
    pub slots: Vec<[u8; SLOT_LEN]>,
}

impl ChrModel {
    /// Parses a model entry. Errors on anything that is not a model (textures, truncated or inconsistent entries).
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let descriptor_offset = word(bytes, 0)?;
        let (info_offset, slots_offset) = (word(bytes, 4)? as usize, word(bytes, 8)? as usize);
        if descriptor_offset as usize != HEADER_LEN {
            return Err("descriptor offset is not 12");
        }
        let info: [u8; INFO_LEN] = range(bytes, info_offset, INFO_LEN)?.try_into().unwrap();
        let info = ModelInfo { raw: info };
        let (count, nodes) = (info.part_count() as usize, info.anim_nodes() as usize);
        if count == 0 || nodes > count {
            return Err("part counts out of range");
        }
        let parts_offset = info_offset.checked_sub(count.checked_mul(PART_LEN).ok_or("part table overflow")?).ok_or("part table before the entry")?;
        if parts_offset < HEADER_LEN {
            return Err("part table overlaps the header");
        }
        let gap_start = info_offset + INFO_LEN;
        if slots_offset < gap_start || slots_offset > bytes.len() || (bytes.len() - slots_offset) % SLOT_LEN != 0 {
            return Err("slot table extent");
        }
        let slots = bytes[slots_offset..].chunks_exact(SLOT_LEN).map(|c| c.try_into().unwrap()).collect();
        Ok(Self {
            descriptor_offset,
            body: bytes[HEADER_LEN..parts_offset].to_vec(),
            parts: PartTable::parse(bytes, parts_offset, count)?,
            info,
            gap: bytes[gap_start..slots_offset].to_vec(),
            slots,
        })
    }

    pub fn info_offset(&self) -> usize {
        self.parts.offset + self.parts.records.len() * PART_LEN
    }

    /// Rebuilds the entry bytes.
    pub fn encode(&self) -> Vec<u8> {
        let info_offset = self.info_offset();
        let slots_offset = info_offset + INFO_LEN + self.gap.len();
        let mut out = Vec::with_capacity(slots_offset + self.slots.len() * SLOT_LEN);
        out.extend_from_slice(&self.descriptor_offset.to_le_bytes());
        out.extend_from_slice(&(info_offset as u32).to_le_bytes());
        out.extend_from_slice(&(slots_offset as u32).to_le_bytes());
        out.extend_from_slice(&self.body);
        for record in &self.parts.records {
            out.extend_from_slice(&record.raw);
        }
        out.extend_from_slice(&self.info.raw);
        out.extend_from_slice(&self.gap);
        for slot in &self.slots {
            out.extend_from_slice(slot);
        }
        out
    }

    /// Total length of the entry in bytes.
    pub fn len(&self) -> usize {
        self.info_offset() + INFO_LEN + self.gap.len() + self.slots.len() * SLOT_LEN
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// Matrix cells of the animation parts (kind 0 or 2) in table order.
    pub fn anim_cells(&self) -> Vec<i8> {
        self.parts.records.iter().filter(|r| matches!(r.kind(), 0 | 2)).map(|r| r.cell()).collect()
    }

    /// The structural invariants MEASURED on all 516 models: tree consistent, N animation parts whose cells are a
    /// permutation of 0..N, every blob pointer inside the entry.
    pub fn validate(&self) -> Result<()> {
        if !self.parts.is_consistent() {
            return Err("part tree inconsistent");
        }
        let mut cells = self.anim_cells();
        cells.sort_unstable();
        if cells.len() != self.info.anim_nodes() as usize || cells.iter().enumerate().any(|(i, c)| *c as i32 != i as i32) {
            return Err("animation cells are not a permutation of the node count");
        }
        let len = self.len() as u32;
        if self.parts.records.iter().any(|r| r.pointer_fields().iter().any(|(_, target)| *target >= len)) {
            return Err("part pointer outside the entry");
        }
        Ok(())
    }

    /// A clip drives this model when it has one node per animation cell (the necessary condition of the pose engine:
    /// node `n` fills matrix cell `n`).
    pub fn accepts_clip(&self, clip: &crate::Clip) -> bool {
        clip.nodes.len() == self.info.anim_nodes() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic() -> Vec<u8> {
        let records = {
            let mut bytes = Vec::new();
            // root (cell 0) with one child follower (cell 1) and an alias (kind 3, cell 0)
            for (kind, cell, parent, child, sibling) in [(0u8, 0u8, 0xFFu8, 1i8, -1i8), (2, 1, 0, -1, 2), (3, 0, 0, 0, -1)] {
                let mut r = [0u8; PART_LEN];
                (r[0x48], r[0x49], r[0x4A], r[0x4B], r[0x4C]) = (kind, cell, parent, child as u8, sibling as u8);
                bytes.extend_from_slice(&r);
            }
            bytes
        };
        let mut info = [0u8; INFO_LEN];
        info[0..4].copy_from_slice(&3u32.to_le_bytes());
        info[4..8].copy_from_slice(&2u32.to_le_bytes());
        info[8..12].copy_from_slice(&1u32.to_le_bytes());
        let body = vec![0x5A; 20];
        let info_offset = HEADER_LEN + body.len() + records.len();
        let mut bytes = Vec::new();
        for w in [12u32, info_offset as u32, (info_offset + INFO_LEN) as u32] {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
        bytes.extend(&body);
        bytes.extend(&records);
        bytes.extend(info);
        let mut slot = [0u8; SLOT_LEN];
        slot[4..16].fill(0xFF);
        bytes.extend(slot);
        bytes
    }

    #[test]
    fn parse_encode_round_trip_and_validate() {
        let bytes = synthetic();
        let model = ChrModel::parse(&bytes).unwrap();
        assert_eq!((model.parts.records.len(), model.info.anim_nodes(), model.slots.len(), model.body.len()), (3, 2, 1, 20));
        assert_eq!(model.encode(), bytes);
        assert_eq!(model.len(), bytes.len());
        model.validate().unwrap();
        assert_eq!(model.anim_cells(), vec![0, 1]);
    }

    #[test]
    fn rejects_non_models_and_broken_extents() {
        let bytes = synthetic();
        assert!(ChrModel::parse(&bytes[..40]).is_err());
        let mut bad = bytes.clone();
        bad[0] = 13;
        assert!(ChrModel::parse(&bad).is_err());
        let mut bad = bytes.clone();
        bad[8] += 1; // slot table no longer 48 byte aligned to the end
        assert!(ChrModel::parse(&bad).is_err());
        let mut bad = bytes.clone();
        let at = bytes.len() - SLOT_LEN - INFO_LEN;
        bad[at..at + 4].copy_from_slice(&9999u32.to_le_bytes()); // F too large for the entry
        assert!(ChrModel::parse(&bad).is_err());
        assert!(ChrModel::parse(&[0u8; 4224]).is_err());
    }

    #[test]
    fn validate_catches_cell_and_pointer_defects() {
        let mut bytes = synthetic();
        let mut model = ChrModel::parse(&bytes).unwrap();
        model.parts.records[1].raw[0x49] = 5;
        assert!(model.validate().is_err());
        bytes = synthetic();
        let mut model = ChrModel::parse(&bytes).unwrap();
        model.parts.records[0].raw[0..4].copy_from_slice(&0xFFFFu32.to_le_bytes());
        assert!(model.validate().is_err());
    }
}
