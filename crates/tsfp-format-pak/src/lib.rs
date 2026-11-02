//! Lossless P5CK envelope for the Xbox PAK variant.
//!
//! The table is ordered: duplicate path keys are valid and must survive encoding.
//! Payload bytes, gaps, and compression bitstreams are retained verbatim.

use tsfp_format_core::*;

/// One ordered table row. A zero `stored_size` means an uncompressed payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: u32,
    pub offset: u32,
    pub decoded_size: u32,
    pub stored_size: u32,
}

impl Entry {
    /// Bytes addressed by this row; zero stored size selects the decoded size.
    fn payload_len(&self) -> usize {
        (if self.stored_size == 0 {
            self.decoded_size
        } else {
            self.stored_size
        }) as usize
    }
}

/// Parsed envelope retaining the original bytes for lossless re-encoding.
///
/// Only table fields can be edited. The number of entries and payload bytes stay fixed.
///
/// ```
/// use tsfp_format_pak::Pak;
/// let mut bytes = b"P5CK".to_vec();
/// bytes.extend_from_slice(&12u32.to_le_bytes());
/// bytes.extend_from_slice(&0u32.to_le_bytes());
/// let pak = Pak::parse(&bytes).unwrap();
/// assert!(pak.entries.is_empty());
/// assert_eq!(pak.encode().unwrap(), bytes);
/// ```
#[derive(Debug, Clone)]
pub struct Pak {
    pub entries: Vec<Entry>,
    original: Vec<u8>,
    toc: usize,
}

fn payload_extent_valid(entry: &Entry, toc: usize) -> bool {
    let size = entry.payload_len();
    size == 0
        || ((entry.offset as usize) >= 12
            && (entry.offset as usize)
                .checked_add(size)
                .is_some_and(|end| end <= toc))
}

impl Pak {
    /// Parse a P5CK envelope whose nonempty table ends at EOF.
    pub fn parse(b: &[u8]) -> Result<Self> {
        if range(b, 0, 4)? != b"P5CK" {
            return Err("unsupported PAK magic");
        }
        let toc = word(b, 4)? as usize;
        let size = word(b, 8)? as usize;
        if size % 16 != 0 {
            return Err("partial TOC");
        }
        if size != 0 && (toc < 12 || toc.checked_add(size) != Some(b.len())) {
            return Err("TOC not at EOF");
        }
        range(b, 0, 12)?;
        if size != 0 {
            range(b, toc, size)?;
        }
        let mut entries = Vec::new();
        for i in 0..size / 16 {
            let w = words(b, toc + i * 16, 4)?;
            let entry = Entry {
                key: w[0],
                offset: w[1],
                decoded_size: w[2],
                stored_size: w[3],
            };
            if !payload_extent_valid(&entry, toc) {
                return Err("payload overlaps header/TOC");
            }
            entries.push(entry);
        }
        Ok(Self {
            entries,
            original: b.to_vec(),
            toc,
        })
    }

    /// Return the exact stored bytes, including a compressed bitstream if present.
    pub fn stored(&self, index: usize) -> Result<&[u8]> {
        let e = self.entries.get(index).ok_or("entry index")?;
        if !payload_extent_valid(e, self.toc) {
            return Err("payload overlaps header/TOC");
        }
        let size = e.payload_len();
        // Empty rows address no payload bytes, even if their retained offset
        // does not name a position in the envelope.
        if size == 0 {
            return Ok(&self.original[..0]);
        }
        range(&self.original, e.offset as usize, size)
    }

    /// Return an uncompressed payload; compressed rows require an external decoder.
    pub fn payload(&self, index: usize) -> Result<&[u8]> {
        if self.entries.get(index).ok_or("entry index")?.stored_size != 0 {
            return Err("gzip decoding unsupported; use stored() for exact bitstream");
        }
        self.stored(index)
    }

    /// Rewrite table fields while retaining all other bytes; reject invalid edits.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let declared = word(&self.original, 8)? as usize / 16;
        if self.entries.len() != declared {
            return Err("entry count cannot change in lossless envelope");
        }
        if self
            .entries
            .iter()
            .any(|entry| !payload_extent_valid(entry, self.toc))
        {
            return Err("payload overlaps header/TOC");
        }
        let mut b = self.original.clone();
        for (i, e) in self.entries.iter().enumerate() {
            for (j, w) in [e.key, e.offset, e.decoded_size, e.stored_size]
                .iter()
                .enumerate()
            {
                put(&mut b, self.toc + i * 16 + j * 4, *w);
            }
        }
        Self::parse(&b)?;
        Ok(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(rows: &[(u32, u32, u32, u32)]) -> Vec<u8> {
        let toc = 64u32;
        let mut b = vec![0xa5; toc as usize + rows.len() * 16];
        b[..4].copy_from_slice(b"P5CK");
        put(&mut b, 4, toc);
        put(&mut b, 8, (rows.len() * 16) as u32);
        for (i, row) in rows.iter().enumerate() {
            for (j, value) in [row.0, row.1, row.2, row.3].iter().enumerate() {
                put(&mut b, toc as usize + i * 16 + j * 4, *value);
            }
        }
        b
    }

    #[test]
    fn ordered_duplicate_round_trips_and_preserves_gap_bytes() {
        for seed in 0..256u32 {
            let key = seed.wrapping_mul(0x9e37_79b9);
            let b = archive(&[(key, 12, 4, 0), (key, 20, 99, 7)]);
            let mut p = Pak::parse(&b).unwrap();
            assert_eq!(p.entries[0].key, p.entries[1].key);
            assert_eq!(p.payload(0).unwrap(), &b[12..16]);
            assert_eq!(p.stored(1).unwrap(), &b[20..27]);
            assert!(p.payload(1).is_err());
            assert_eq!(p.encode().unwrap(), b);
            p.entries[1].key = !key;
            let edited = p.encode().unwrap();
            assert_eq!(&edited[..64], &b[..64]);
            assert_eq!(Pak::parse(&edited).unwrap().entries[1].key, !key);
        }
    }

    #[test]
    fn invalid_extents_and_entry_count_are_rejected() {
        for offset in [0, 1, 11, 61, 63, 64, u32::MAX] {
            let b = archive(&[(7, offset, 4, 0)]);
            assert!(Pak::parse(&b).is_err(), "offset {offset}");
        }
        let b = archive(&[(7, 12, 4, 0)]);
        let mut p = Pak::parse(&b).unwrap();
        p.entries.push(p.entries[0].clone());
        assert!(p.encode().is_err());
        let mut p = Pak::parse(&b).unwrap();
        p.entries[0].offset = 63;
        assert!(p.encode().is_err());
    }

    #[test]
    fn empty_and_partial_table() {
        let mut b = b"P5CK".to_vec();
        b.extend(16u32.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        assert_eq!(Pak::parse(&b).unwrap().encode().unwrap(), b);
        put(&mut b, 8, 1);
        assert!(Pak::parse(&b).is_err());
        assert!(Pak::parse(&b[..11]).is_err());
    }

    #[test]
    fn edited_payload_extents_keep_parse_contract() {
        let original = archive(&[(7, 12, 4, 0)]);
        for (offset, decoded_size, stored_size) in
            [(11, 4, 0), (63, 2, 0), (63, 4, 4), (u32::MAX, 4, 0)]
        {
            let mut pak = Pak::parse(&original).unwrap();
            pak.entries[0].offset = offset;
            pak.entries[0].decoded_size = decoded_size;
            pak.entries[0].stored_size = stored_size;
            assert_eq!(pak.encode().unwrap_err(), "payload overlaps header/TOC");
        }
        let mut pak = Pak::parse(&original).unwrap();
        pak.entries[0].offset = u32::MAX;
        pak.entries[0].decoded_size = 0;
        assert!(pak.encode().is_ok()); // Zero-sized rows do not address payload bytes.
        assert_eq!(pak.stored(0), Ok(&[][..]));
        assert_eq!(pak.payload(0), Ok(&[][..]));
    }

    #[test]
    fn stored_rejects_edited_extent_into_toc() {
        let original = archive(&[(7, 12, 4, 0)]);
        let mut pak = Pak::parse(&original).unwrap();
        assert_eq!(pak.stored(0).unwrap(), &original[12..16]);
        pak.entries[0].offset = 64;
        assert_eq!(pak.stored(0), Err("payload overlaps header/TOC"));
        assert_eq!(pak.payload(0), Err("payload overlaps header/TOC"));
        assert_eq!(pak.encode(), Err("payload overlaps header/TOC"));
    }
}
