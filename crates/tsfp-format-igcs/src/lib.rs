//! ANRS resident animation envelope. Compressed chunks and inline records remain opaque.
//!
//! `Animation::parse` checks descriptor walks and known table extents. `encode`
//! writes edited fixed header words into a retained copy of the input bytes.
//!
//! ```
//! use tsfp_format_igcs::Animation;
//! let mut bytes = vec![0u8; 256];
//! bytes[..4].copy_from_slice(b"ANRS");
//! for (offset, value) in [(4, 200), (12, 1), (16, 1), (20, 1),
//!                         (24, 1), (28, 200), (32, 208), (36, 212)] {
//!     bytes[offset..offset + 4].copy_from_slice(&(value as u32).to_le_bytes());
//! }
//! let mut animation = Animation::parse(&bytes)?;
//! animation.nodes[0].words[0] = 7;
//! assert_eq!(Animation::parse(&animation.encode()?)?.nodes[0].words[0], 7);
//! # Ok::<(), &'static str>(())
//! ```
use tsfp_format_core::*;
#[derive(Debug, Clone)]
/// Fixed words at a byte offset in the retained ANRS envelope.
pub struct Descriptor {
    pub offset: usize,
    pub words: Vec<u32>,
}
#[derive(Debug, Clone)]
/// Parsed fixed headers plus the original bytes for lossless opaque preservation.
pub struct Animation {
    pub header: Vec<u32>,
    pub nodes: Vec<Descriptor>,
    pub animations: Vec<Descriptor>,
    original: Vec<u8>,
    node_offsets: Vec<usize>,
    animation_offsets: Vec<usize>,
}
impl Animation {
    /// Validate the known header, table extents, and descriptor walk.
    pub fn parse(b: &[u8]) -> Result<Self> {
        let h = words(b, 0, 10)?;
        if range(b, 0, 4)? != b"ANRS" || h[3..7].contains(&0) {
            return Err("invalid ANRS header");
        }
        for o in [h[1], h[7], h[8], h[9]] {
            range(b, o as usize, 1)?;
        }
        range(
            b,
            h[7] as usize,
            (h[6] as usize).checked_mul(8).ok_or("block overflow")?,
        )?;
        range(
            b,
            h[8] as usize,
            (h[4] as usize).checked_mul(4).ok_or("frame overflow")?,
        )?;
        let mut c = 40;
        let mut nodes = Vec::new();
        for _ in 0..h[3] {
            let w = words(b, c, 18)?;
            let n = (w[5] as usize).checked_mul(8).ok_or("link overflow")?;
            range(b, c + 72, n)?;
            nodes.push(Descriptor {
                offset: c,
                words: w,
            });
            c += 72 + n;
        }
        let mut animations = Vec::new();
        for i in 0..h[3] {
            c = c.checked_add(7).ok_or("align overflow")? & !7;
            let w = words(b, c, 14)?;
            if w[8] != 0 {
                range(b, w[8] as usize, 1)?;
            }
            let n = if i == 0 {
                0
            } else {
                (w[9] as usize)
                    .checked_mul(32)
                    .and_then(|n| n.checked_add((w[4] as usize).checked_mul(4)?))
                    .ok_or("inline overflow")?
            };
            range(b, c + 56, n)?;
            animations.push(Descriptor {
                offset: c,
                words: w,
            });
            c += 56 + n;
        }
        let node_offsets = nodes.iter().map(|descriptor| descriptor.offset).collect();
        let animation_offsets = animations
            .iter()
            .map(|descriptor| descriptor.offset)
            .collect();
        Ok(Self {
            header: h,
            nodes,
            animations,
            original: b.to_vec(),
            node_offsets,
            animation_offsets,
        })
    }
    /// Repack edited fixed words while preserving all other original bytes.
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.header.len() != 10 {
            return Err("header width");
        }
        // Check public descriptors in write order before copying the envelope.
        for (descriptors, width) in [(&self.nodes, 18), (&self.animations, 14)] {
            for descriptor in descriptors {
                if descriptor.words.len() != width {
                    return Err("descriptor width");
                }
                range(&self.original, descriptor.offset, width * 4)?;
            }
        }
        // These header fields are public, so reject invalid edits before copying
        // the retained opaque envelope. The full descriptor walk still runs below.
        if self.header[0] != u32::from_le_bytes(*b"ANRS") || self.header[3..7].contains(&0) {
            return Err("invalid ANRS header");
        }
        for offset in [
            self.header[1],
            self.header[7],
            self.header[8],
            self.header[9],
        ] {
            range(&self.original, offset as usize, 1)?;
        }
        range(
            &self.original,
            self.header[7] as usize,
            (self.header[6] as usize)
                .checked_mul(8)
                .ok_or("block overflow")?,
        )?;
        range(
            &self.original,
            self.header[8] as usize,
            (self.header[4] as usize)
                .checked_mul(4)
                .ok_or("frame overflow")?,
        )?;
        // The header drives the parser's descriptor walk. Keep public edits to
        // the lists in sync so an omitted or added descriptor cannot be lost.
        if self.nodes.len() != self.header[3] as usize
            || self.animations.len() != self.header[3] as usize
        {
            return Err("descriptor count");
        }
        // Descriptor positions belong to the retained envelope. A public offset
        // edit can otherwise overwrite unrelated opaque bytes while still leaving
        // the original descriptor walk parseable.
        if self
            .nodes
            .iter()
            .map(|descriptor| descriptor.offset)
            .ne(self.node_offsets.iter().copied())
            || self
                .animations
                .iter()
                .map(|descriptor| descriptor.offset)
                .ne(self.animation_offsets.iter().copied())
        {
            return Err("descriptor offset");
        }
        let mut b = self.original.clone();
        for (i, w) in self.header.iter().enumerate() {
            put(&mut b, i * 4, *w);
        }
        for descriptors in [&self.nodes, &self.animations] {
            for d in descriptors {
                for (i, w) in d.words.iter().enumerate() {
                    put(&mut b, d.offset + i * 4, *w);
                }
            }
        }
        Self::parse(&b)?;
        Ok(b)
    }
}
#[test]
fn envelope() {
    let mut b = vec![0; 256];
    b[..4].copy_from_slice(b"ANRS");
    for (o, w) in [
        (4, 200),
        (12, 1),
        (16, 1),
        (20, 1),
        (24, 1),
        (28, 200),
        (32, 208),
        (36, 212),
    ] {
        put(&mut b, o, w);
    }
    let a = Animation::parse(&b).unwrap();
    assert_eq!(a.nodes.len(), 1);
    assert_eq!(a.encode().unwrap(), b);
    assert!(Animation::parse(&b[..150]).is_err());
}

#[cfg(test)]
mod quality {
    use super::*;

    fn fixture(node_count: usize, seed: u32) -> Vec<u8> {
        let mut bytes = vec![0x5au8; 4096];
        bytes[..4].copy_from_slice(b"ANRS");
        for (offset, value) in [
            (4, 4000),
            (12, node_count as u32),
            (16, 1),
            (20, 32),
            (24, 1),
            (28, 4000),
            (32, 4016),
            (36, 4032),
        ] {
            put(&mut bytes, offset, value);
        }
        let mut cursor = 40;
        for node in 0..node_count {
            let links = ((seed as usize + node) % 4) as u32;
            put(&mut bytes, cursor, seed ^ node as u32);
            put(&mut bytes, cursor + 20, links);
            cursor += 72 + links as usize * 8;
        }
        for node in 0..node_count {
            cursor = (cursor + 7) & !7;
            let records = if node == 0 {
                0
            } else {
                (seed as usize + node) % 3
            };
            let keys = if node == 0 {
                0
            } else {
                (seed as usize + node * 3) % 5
            };
            put(&mut bytes, cursor, seed.wrapping_add(node as u32));
            put(&mut bytes, cursor + 16, keys as u32);
            put(&mut bytes, cursor + 32, 0);
            put(&mut bytes, cursor + 36, records as u32);
            cursor += 56 + records * 32 + keys * 4;
        }
        bytes
    }

    #[test]
    fn descriptor_walk_round_trips_opaque_bytes_and_header_edits() {
        for count in 1..=8 {
            for seed in 0..32 {
                let bytes = fixture(count, seed);
                let mut parsed = Animation::parse(&bytes).unwrap();
                assert_eq!(parsed.nodes.len(), count);
                assert_eq!(parsed.animations.len(), count);
                assert_eq!(parsed.encode().unwrap(), bytes);
                parsed.nodes[count - 1].words[0] ^= 0x1234_5678;
                let encoded = parsed.encode().unwrap();
                assert_eq!(
                    Animation::parse(&encoded).unwrap().nodes[count - 1].words[0],
                    parsed.nodes[count - 1].words[0]
                );
                assert_eq!(encoded[4095], 0x5a);
            }
        }
    }

    #[test]
    fn rejects_truncated_descriptors_and_bad_extent_offsets() {
        let bytes = fixture(2, 1);
        let parsed = Animation::parse(&bytes).unwrap();
        for descriptor in parsed.nodes.iter().chain(parsed.animations.iter()) {
            assert!(Animation::parse(&bytes[..descriptor.offset + 3]).is_err());
        }
        for offset in [4, 28, 32, 36] {
            let mut invalid = bytes.clone();
            put(&mut invalid, offset, u32::MAX);
            assert!(Animation::parse(&invalid).is_err(), "offset {offset}");
        }
    }

    #[test]
    fn encode_rejects_invalid_public_descriptors_in_write_order() {
        let mut parsed = Animation::parse(&fixture(2, 1)).unwrap();
        parsed.nodes[0].words.pop();
        parsed.animations[0].words.pop();
        assert_eq!(parsed.encode(), Err("descriptor width"));

        parsed.nodes[0].words.push(0);
        parsed.nodes[0].offset = usize::MAX;
        assert_eq!(parsed.encode(), Err("extent overflow"));
    }

    #[test]
    fn encode_preflights_public_header_without_changing_valid_bytes() {
        let bytes = fixture(2, 1);
        let mut parsed = Animation::parse(&bytes).unwrap();
        assert_eq!(parsed.encode().unwrap(), bytes);
        parsed.header[0] = 0;
        assert_eq!(parsed.encode(), Err("invalid ANRS header"));
        parsed.header[0] = u32::from_le_bytes(*b"ANRS");
        parsed.header[3] = 0;
        assert_eq!(parsed.encode(), Err("invalid ANRS header"));
        parsed.header[3] = 2;
        parsed.header[7] = u32::MAX;
        assert!(parsed.encode().is_err());
    }

    #[test]
    fn encode_rejects_descriptor_count_edits() {
        let bytes = fixture(2, 1);
        let mut parsed = Animation::parse(&bytes).unwrap();
        parsed.nodes.pop();
        assert_eq!(parsed.encode(), Err("descriptor count"));

        let mut parsed = Animation::parse(&bytes).unwrap();
        parsed.animations.push(parsed.animations[0].clone());
        assert_eq!(parsed.encode(), Err("descriptor count"));

        let mut parsed = Animation::parse(&bytes).unwrap();
        parsed.header[3] = 1;
        assert_eq!(parsed.encode(), Err("descriptor count"));
        assert_eq!(Animation::parse(&bytes).unwrap().encode().unwrap(), bytes);
    }

    #[test]
    fn encode_rejects_in_bounds_descriptor_relocation() {
        let bytes = fixture(2, 1);
        let mut parsed = Animation::parse(&bytes).unwrap();
        parsed.nodes[0].offset += 1;
        assert_eq!(parsed.encode(), Err("descriptor offset"));

        let mut parsed = Animation::parse(&bytes).unwrap();
        parsed.animations[1].offset += 1;
        assert_eq!(parsed.encode(), Err("descriptor offset"));
        assert_eq!(Animation::parse(&bytes).unwrap().encode().unwrap(), bytes);
    }
}
