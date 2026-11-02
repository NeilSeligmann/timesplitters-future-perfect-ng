//! Skeleton part records of the character model container (T1873, T1875, spec in `docs/t-rust-anim.md`).
//!
//! A character model holds a table of 0xC8 byte part records (the records the T1872 walks `0x582A0` /
//! `0x58340` / `0x583E0` read). The table is located by the container ([`crate::chrmodel::ChrModel`]): it ends where
//! the model info struct starts. Bytes used by the pose engine (MEASURED offsets): `+0x48` kind (0 bone, 1 helper,
//! 2 follower, 3 alias of another part), `+0x49` signed matrix cell, `+0x4B` first child, `+0x4C` next sibling (negative
//! ends), `+0xB8` blend float. `+0x4A` is the parent part (0xFF for the root); no T1872 routine reads it, its meaning is
//! INFERRED from the data (parent(sibling(i)) == parent(i) on all 516 models, parent(child(i)) == i for every part that
//! is not an alias).
use tsfp_format_core::{Result, range};

pub const PART_LEN: usize = 0xC8;

/// One 0xC8 byte part record, kept verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartRecord {
    pub raw: [u8; PART_LEN],
}

impl PartRecord {
    pub fn kind(&self) -> u8 {
        self.raw[0x48]
    }
    pub fn cell(&self) -> i8 {
        self.raw[0x49] as i8
    }
    /// `+0x4A`, 0xFF for a root (INFERRED parent).
    pub fn parent(&self) -> u8 {
        self.raw[0x4A]
    }
    pub fn child(&self) -> i8 {
        self.raw[0x4B] as i8
    }
    pub fn sibling(&self) -> i8 {
        self.raw[0x4C] as i8
    }
    /// Stream offsets (relative to the model blob, 0 = none) of level of detail slot `lod` (0..3): six dwords at
    /// `0x18 * lod` (MEASURED pointer fields, the fixup `0x8DF60` rebases them; slot 0 holds the mesh in 15259 of
    /// 16477 parts, slots 1 and 2 are rare).
    pub fn lod_streams(&self, lod: usize) -> [u32; 6] {
        let at = 0x18 * lod;
        std::array::from_fn(|i| u32::from_le_bytes(self.raw[at + 4 * i..at + 4 * i + 4].try_into().unwrap()))
    }
    /// Counts of the pointer arrays at `+0x7C`, `+0x80`, `+0x84`, `+0x88` (signed 16 bit words at `+0x50..+0x58`).
    pub fn array_counts(&self) -> [i16; 4] {
        std::array::from_fn(|i| i16::from_le_bytes(self.raw[0x50 + 2 * i..0x52 + 2 * i].try_into().unwrap()))
    }
    /// Every non-zero blob-relative pointer field `(field offset, target offset)` the fixup `0x8DF60` rebases:
    /// the 18 stream dwords, `+0x60/+0x64/+0x68`, `+0x6C`, `+0x78`, `+0x7C`, `+0x80`, `+0x84`, `+0x88`, `+0x8C`,
    /// `+0xA8`, `+0xAC`, `+0xB0`, `+0xB4`, `+0xBC`, `+0xC0`, `+0xC4`.
    pub fn pointer_fields(&self) -> Vec<(usize, u32)> {
        let word = |at: usize| u32::from_le_bytes(self.raw[at..at + 4].try_into().unwrap());
        let mut fields: Vec<usize> = (0..18).map(|i| i * 4).collect();
        fields.extend([0x60, 0x64, 0x68, 0x6C, 0x78, 0x7C, 0x80, 0x84, 0x88, 0x8C, 0xA8, 0xAC, 0xB0, 0xB4, 0xBC, 0xC0, 0xC4]);
        fields.into_iter().map(|at| (at, word(at))).filter(|(_, target)| *target != 0).collect()
    }
    /// Raw little endian dword at a byte offset of the record.
    pub fn raw_dword(&self, at: usize) -> u32 {
        crate::record_dword(&self.raw, at)
    }
    pub fn blend(&self) -> f32 {
        f32::from_le_bytes(self.raw[0xB8..0xBC].try_into().unwrap())
    }
}

/// A run of part records at `offset` of a model file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartTable {
    pub offset: usize,
    pub records: Vec<PartRecord>,
}

impl PartTable {
    /// Reads `count` records at `offset`.
    pub fn parse(bytes: &[u8], offset: usize, count: usize) -> Result<Self> {
        let total = count.checked_mul(PART_LEN).ok_or("part table overflow")?;
        let region = range(bytes, offset, total)?;
        let records = region
            .chunks_exact(PART_LEN)
            .map(|chunk| PartRecord { raw: chunk.try_into().unwrap() })
            .collect();
        Ok(Self { offset, records })
    }

    /// Writes the records back over `bytes` (same extent as parsed).
    pub fn encode_into(&self, bytes: &mut [u8]) -> Result<()> {
        let total = self.records.len().checked_mul(PART_LEN).ok_or("part table overflow")?;
        range(bytes, self.offset, total)?;
        for (index, record) in self.records.iter().enumerate() {
            let at = self.offset + index * PART_LEN;
            bytes[at..at + PART_LEN].copy_from_slice(&record.raw);
        }
        Ok(())
    }

    /// The tree is internally consistent: record 0 is the root, every link names a part of the table, the siblings
    /// share the parent and the first child names its parent. Alias records (kind 3) are exempt from the child rule:
    /// their child link points at a part of the original (MEASURED on all 516 character models).
    pub fn is_consistent(&self) -> bool {
        let n = self.records.len();
        let inside = |link: i8| link < 0 || (link as usize) < n;
        n > 0
            && self.records[0].parent() == 0xFF
            && self.records.iter().enumerate().all(|(i, r)| {
                inside(r.child())
                    && inside(r.sibling())
                    && (i == 0 || (r.parent() as usize) < n)
                    && (r.kind() == 3 || r.child() < 0 || self.records[r.child() as usize].parent() as usize == i)
                    && (r.sibling() < 0 || self.records[r.sibling() as usize].parent() == r.parent())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fork skeleton of 8 bones (two limbs under the root, the second with two leaves).
    pub(crate) fn synthetic_records() -> Vec<u8> {
        let links: [(u8, i8, i8); 8] = [
            (0xFF, 1, -1),
            (0, 2, 4),
            (1, 3, -1),
            (2, -1, -1),
            (0, 5, -1),
            (4, 6, -1),
            (5, -1, 7),
            (5, -1, -1),
        ];
        let mut bytes = Vec::new();
        for (i, (parent, child, sibling)) in links.iter().enumerate() {
            let mut record = [0u8; PART_LEN];
            record[0x49] = i as u8;
            record[0x4A] = *parent;
            record[0x4B] = *child as u8;
            record[0x4C] = *sibling as u8;
            bytes.extend_from_slice(&record);
        }
        bytes
    }

    #[test]
    fn parses_round_trips_and_checks_the_tree() {
        let mut bytes = vec![0xAA; 12];
        bytes.extend(synthetic_records());
        let table = PartTable::parse(&bytes, 12, 8).unwrap();
        assert!(table.is_consistent());
        let mut copy = bytes.clone();
        copy[12..].fill(0);
        table.encode_into(&mut copy).unwrap();
        assert_eq!(copy, bytes);
        assert_eq!((table.records[1].sibling(), table.records[0].parent()), (4, 0xFF));
        assert!(PartTable::parse(&bytes, 12, 9).is_err());
    }

    #[test]
    fn rejects_broken_trees() {
        let bytes = synthetic_records();
        let mut broken = bytes.clone();
        broken[4 * PART_LEN + 0x4A] = 7; // part 4 claims another parent than its sibling relation allows
        assert!(!PartTable::parse(&broken, 0, 8).unwrap().is_consistent());
        let mut no_root = bytes;
        no_root[0x4A] = 0;
        assert!(!PartTable::parse(&no_root, 0, 8).unwrap().is_consistent());
    }

    #[test]
    fn pointer_fields_list_non_zero_offsets() {
        let mut record = PartRecord { raw: [0; PART_LEN] };
        record.raw[0x18..0x1C].copy_from_slice(&77u32.to_le_bytes());
        record.raw[0xAC..0xB0].copy_from_slice(&99u32.to_le_bytes());
        assert_eq!(record.pointer_fields(), vec![(0x18, 77), (0xAC, 99)]);
        assert_eq!(record.lod_streams(1)[0], 77);
    }
}
