//! Resident animation clip codec (T1873, spec in `docs/t-rust-anim.md`).
//!
//! One PAK entry under `pak/anim/*.pak` or `pak/stream/lv*/anim/*.pak` is one clip. Layout (MEASURED on 3193 real
//! clips, byte-exact length closure on all of them):
//!
//! ```text
//! 0x00  header, 14 dwords (0x38 bytes); [0] and [1] are runtime slot words (0xFFFFFFFF on 3193 clips, 0 on 54, 0xF/0 on one), [2] flags, [3] frame count, [4] key count K,
//!       [5] node count N, [9] = N again, the rest is zero on disc (runtime fills +0x18 duration, +0x1C, +0x20, +0x28)
//! 0x38  K key frame numbers (u32)
//! ....  N node records of 32 bytes (+4 selector, +8 f32 last frame time, +0xC key count; +0x10/+0x14 are
//!       pointers set by the loader, zero on disc)
//! ....  if flags bit 3: K f32 (a monotone table, INFERRED travelled distance)
//! ....  per node, in node order, the node track: size is a function of selector, flags bit 0 and bit 4 and K
//! ....  tail (8 bytes in the single key pose clips, otherwise empty), kept verbatim
//! ```
//!
//! The codec keeps every field it parses and rebuilds the bytes from them (it holds no copy of the input).
use tsfp_format_core::{Result, range, word};

pub mod chrbody;
pub mod chrmodel;
pub mod propset;
pub mod skeleton;

/// Read a little-endian word from an already-sized model or skeleton record.
/// Callers supply a byte offset with four remaining bytes; invalid offsets panic.
fn record_dword(raw: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(raw[at..at + 4].try_into().unwrap())
}

pub const HEADER_LEN: usize = 0x38;
pub const NODE_LEN: usize = 0x20;
/// Flag bit 0: quaternion stored as 16 bit fields.
pub const FLAG_PACKED16: u32 = 1;
/// Flag bit 1: samples live in a streamed key cache (`0x58DF0`); never on the disc, rejected by the parser.
pub const FLAG_STREAMED: u32 = 2;
/// Flag bit 3: a K entry f32 table precedes the node tracks.
pub const FLAG_DISTANCES: u32 = 8;
/// Flag bit 4: only x, y, z of the quaternion are stored, w is derived.
pub const FLAG_DERIVED_W: u32 = 0x10;

/// One 32 byte node record (raw dwords, the typed views are accessors).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub words: [u32; 8],
}

impl Node {
    pub fn selector(&self) -> u32 {
        self.words[1]
    }
    /// `+8`: f32 bits of the last key time (frame count minus one on every real clip seen).
    pub fn last_time(&self) -> f32 {
        f32::from_bits(self.words[2])
    }
    pub fn key_count(&self) -> u32 {
        self.words[3]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub header: [u32; 14],
    pub key_frames: Vec<u32>,
    pub nodes: Vec<Node>,
    /// f32 bits, K entries when `FLAG_DISTANCES` is set, else empty.
    pub distances: Vec<u32>,
    /// One byte track per node.
    pub tracks: Vec<Vec<u8>>,
    pub tail: Vec<u8>,
}

/// Byte length of a node track (the formulas follow the sample decoder `0x59320`, MEASURED by length closure on the
/// disc for selectors 0, 2 (packed and float), 4 (packed), 8, 11, 12, 13; the float forms of 4 and the selector 10
/// stride are INFERRED from the decoder only).
pub fn track_len(selector: u32, flags: u32, keys: usize) -> Result<usize> {
    let derived = usize::from(flags & FLAG_DERIVED_W != 0);
    let quat = if flags & FLAG_PACKED16 != 0 {
        2 * (4 - derived)
    } else {
        4 * (4 - derived)
    };
    let len = match selector {
        0 => Some(12 + quat),
        2 => keys.checked_mul(quat).and_then(|n| n.checked_add(12)),
        4 if keys >= 1 => (keys - 1)
            .checked_mul(12)
            .and_then(|n| n.checked_add(12))
            .and_then(|n| n.checked_add(quat)),
        8 => keys.checked_mul(12 + quat),
        10 => keys.checked_mul(32),
        11 => keys.checked_mul(10),
        12 => keys.checked_mul(4).and_then(|n| n.checked_add(6)),
        13 => keys.checked_mul(6).and_then(|n| n.checked_add(4)),
        _ => return Err("unsupported node selector"),
    };
    len.ok_or("track length overflow")
}

fn validate_resident_flags(flags: u32) -> Result<()> {
    if flags > 0x1F {
        return Err("unknown clip flags");
    }
    if flags & FLAG_STREAMED != 0 {
        return Err("streamed clips are not supported");
    }
    Ok(())
}

impl Clip {
    pub fn flags(&self) -> u32 {
        self.header[2]
    }
    pub fn frame_count(&self) -> u32 {
        self.header[3]
    }
    pub fn key_count(&self) -> u32 {
        self.header[4]
    }
    pub fn node_count(&self) -> u32 {
        self.header[5]
    }

    /// Parses a resident clip, retaining its header words and trailing bytes.
    /// Track lengths are derived from each node's selector; unknown selectors fail.
    ///
    /// ```
    /// use tsfp_format_anim::Clip;
    /// let mut bytes = vec![0u8; 0x38]; // Empty clip: no keys or nodes.
    /// bytes[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
    /// let clip = Clip::parse(&bytes).unwrap();
    /// assert_eq!(clip.key_count(), 0);
    /// assert_eq!(clip.encode().unwrap(), bytes);
    /// ```
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut header = [0u32; 14];
        for (index, slot) in header.iter_mut().enumerate() {
            *slot = word(bytes, 4 * index)?;
        }
        let flags = header[2];
        validate_resident_flags(flags)?;
        if header[5] != header[9] {
            return Err("node count fields differ");
        }
        let keys = header[4] as usize;
        let nodes_total = header[5] as usize;
        let mut cursor = HEADER_LEN;
        let table = keys.checked_mul(4).ok_or("key table overflow")?;
        range(bytes, cursor, table)?;
        let key_frames = (0..keys)
            .map(|k| word(bytes, cursor + 4 * k))
            .collect::<Result<Vec<_>>>()?;
        cursor += table;
        let record_bytes = nodes_total
            .checked_mul(NODE_LEN)
            .ok_or("node table overflow")?;
        range(bytes, cursor, record_bytes)?;
        let mut nodes = Vec::new();
        for index in 0..nodes_total {
            let mut words = [0u32; 8];
            for (k, slot) in words.iter_mut().enumerate() {
                *slot = word(bytes, cursor + NODE_LEN * index + 4 * k)?;
            }
            nodes.push(Node { words });
        }
        cursor += record_bytes;
        let mut distances = Vec::new();
        if flags & FLAG_DISTANCES != 0 {
            range(bytes, cursor, table)?;
            for k in 0..keys {
                distances.push(word(bytes, cursor + 4 * k)?);
            }
            cursor += table;
        }
        let mut tracks = Vec::new();
        for node in &nodes {
            let len = track_len(node.selector(), flags, keys)?;
            tracks.push(range(bytes, cursor, len)?.to_vec());
            cursor += len;
        }
        Ok(Self {
            header,
            key_frames,
            nodes,
            distances,
            tracks,
            tail: bytes[cursor..].to_vec(),
        })
    }

    /// Rebuilds the bytes from the fields; rejects a clip whose counts, tables or tracks no longer agree.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let flags = self.flags();
        validate_resident_flags(flags)?;
        let keys = self.key_count() as usize;
        if self.key_frames.len() != keys
            || self.nodes.len() != self.node_count() as usize
            || self.header[5] != self.header[9]
            || self.tracks.len() != self.nodes.len()
        {
            return Err("clip counts disagree with the tables");
        }
        if self.distances.len() != if flags & FLAG_DISTANCES != 0 { keys } else { 0 } {
            return Err("distance table does not match the flags");
        }
        // Public tracks may have been resized after parsing. Check all of them
        // before copying the header, tables, or any preceding valid tracks.
        for (node, track) in self.nodes.iter().zip(&self.tracks) {
            if track.len() != track_len(node.selector(), flags, keys)? {
                return Err("track length does not match its selector");
            }
        }
        let mut out = Vec::new();
        let mut push = |value: u32| out.extend_from_slice(&value.to_le_bytes());
        self.header.iter().for_each(|w| push(*w));
        self.key_frames.iter().for_each(|w| push(*w));
        self.nodes.iter().flat_map(|n| n.words).for_each(&mut push);
        self.distances.iter().for_each(|w| push(*w));
        for track in &self.tracks {
            out.extend_from_slice(track);
        }
        out.extend_from_slice(&self.tail);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_rejects_flags_that_parse_rejects() {
        let bytes = vec![0u8; HEADER_LEN];
        let mut clip = Clip::parse(&bytes).unwrap();
        assert_eq!(clip.encode().unwrap(), bytes);
        for (flags, error) in [
            (0x20, "unknown clip flags"),
            (FLAG_STREAMED, "streamed clips are not supported"),
        ] {
            clip.header[2] = flags;
            assert_eq!(clip.encode(), Err(error));
            let mut edited = bytes.clone();
            edited[8..12].copy_from_slice(&flags.to_le_bytes());
            assert!(Clip::parse(&edited).is_err());
        }
    }

    #[test]
    fn track_length_rejects_final_addition_overflow() {
        assert_eq!(
            track_len(2, FLAG_DERIVED_W, usize::MAX / 12),
            Err("track length overflow")
        );
        assert_eq!(
            track_len(4, 0, usize::MAX / 12 + 1),
            Err("track length overflow")
        );
        assert_eq!(
            track_len(12, 0, usize::MAX / 4),
            Err("track length overflow")
        );
        assert_eq!(
            track_len(13, 0, usize::MAX / 6),
            Err("track length overflow")
        );
    }

    fn synthetic(flags: u32, selectors: &[u32], keys: u32) -> Vec<u8> {
        let clip = {
            let mut header = [0u32; 14];
            header[..6].copy_from_slice(&[
                u32::MAX,
                u32::MAX,
                flags,
                9,
                keys,
                selectors.len() as u32,
            ]);
            header[9] = selectors.len() as u32;
            let nodes: Vec<Node> = selectors
                .iter()
                .map(|s| Node {
                    words: [0, *s, 0, keys, 0, 0, 0, 0],
                })
                .collect();
            let tracks = selectors
                .iter()
                .map(|s| {
                    (0..track_len(*s, flags, keys as usize).unwrap())
                        .map(|b| b as u8)
                        .collect()
                })
                .collect();
            Clip {
                header,
                key_frames: (0..keys).collect(),
                nodes,
                distances: if flags & FLAG_DISTANCES != 0 {
                    vec![7; keys as usize]
                } else {
                    vec![]
                },
                tracks,
                tail: vec![1, 2, 3],
            }
        };
        clip.encode().unwrap()
    }

    #[test]
    fn round_trip_every_selector_and_flag() {
        for flags in [0, 1, 0x10, 0x11, 0x19] {
            let bytes = synthetic(flags, &[0, 2, 4, 8, 10, 11, 12, 13], 5);
            let clip = Clip::parse(&bytes).unwrap();
            assert_eq!(clip.encode().unwrap(), bytes);
            assert_eq!(clip.nodes.len(), 8);
            assert_eq!(clip.tail, vec![1, 2, 3]);
        }
    }

    #[test]
    fn track_sizes_match_the_disc_measurements() {
        // packed 16 bit and derived w: 12 + 6 for the fixed form, 12 + 6 K for selector 2, 6 + 4 K, 4 + 6 K, 10 K.
        assert_eq!(track_len(0, 0x11, 1).unwrap(), 18);
        assert_eq!(track_len(2, 0x11, 84).unwrap(), 516);
        assert_eq!(track_len(2, 0, 168).unwrap(), 2700);
        assert_eq!(track_len(12, 0x11, 16).unwrap(), 70);
        assert_eq!(track_len(13, 0x11, 2).unwrap(), 16);
        assert_eq!(track_len(11, 0x11, 5).unwrap(), 50);
        assert!(track_len(1, 0, 1).is_err());
        assert!(track_len(4, 0, 0).is_err());
    }

    #[test]
    fn rejects_malformed() {
        let bytes = synthetic(0x11, &[0, 11, 12], 4);
        for cut in [0, 7, 0x37, 0x40, bytes.len() - 5] {
            assert!(Clip::parse(&bytes[..cut]).is_err(), "cut {cut}");
        }
        let mut bad = bytes.clone();
        bad[8] = 0x40;
        assert!(Clip::parse(&bad).is_err());
        let mut streamed = bytes.clone();
        streamed[8] |= 2;
        assert!(Clip::parse(&streamed).is_err());
        let mut counts = bytes.clone();
        counts[0x24] ^= 1;
        assert!(Clip::parse(&counts).is_err());
        let mut clip = Clip::parse(&bytes).unwrap();
        clip.tracks[1].pop();
        assert!(clip.encode().is_err());
    }

    #[test]
    fn encode_checks_edited_tracks_before_serialization() {
        let bytes = synthetic(FLAG_PACKED16, &[0, 11], 4);
        let mut clip = Clip::parse(&bytes).unwrap();
        assert_eq!(clip.encode().unwrap(), bytes);
        clip.tracks[1].push(0x5a);
        assert_eq!(
            clip.encode(),
            Err("track length does not match its selector")
        );
        clip.tracks[1].pop();
        clip.nodes[1].words[1] = 99;
        assert_eq!(clip.encode(), Err("unsupported node selector"));
        clip.nodes[1].words[1] = 11;
        assert_eq!(clip.encode().unwrap(), bytes);
    }

    #[test]
    fn typed_edit_changes_only_its_bytes() {
        let bytes = synthetic(0x11, &[12, 11], 3);
        let mut clip = Clip::parse(&bytes).unwrap();
        clip.key_frames[1] = 77;
        let edited = clip.encode().unwrap();
        let diff: Vec<usize> = (0..bytes.len())
            .filter(|i| bytes[*i] != edited[*i])
            .collect();
        assert_eq!(diff, vec![HEADER_LEN + 4]);
    }

    #[test]
    fn structural_grid_preserves_tracks_tables_and_tail() {
        let selectors = [0, 2, 4, 8, 10, 11, 12, 13];
        let flags = [
            0,
            FLAG_PACKED16,
            FLAG_DERIVED_W,
            FLAG_PACKED16 | FLAG_DERIVED_W,
            FLAG_DISTANCES | FLAG_PACKED16 | FLAG_DERIVED_W,
        ];
        for seed in 0..256usize {
            let flag = flags[seed % flags.len()];
            let keys = 1 + (seed % 9) as u32;
            let chosen = [
                selectors[(seed / 3) % selectors.len()],
                selectors[(seed / 11) % selectors.len()],
            ];
            let bytes = synthetic(flag, &chosen, keys);
            let clip = Clip::parse(&bytes).unwrap();
            assert_eq!(clip.encode().unwrap(), bytes, "seed {seed}");
            assert_eq!(clip.tracks.len(), chosen.len(), "seed {seed}");
            for (node, track) in clip.nodes.iter().zip(&clip.tracks) {
                assert_eq!(
                    track.len(),
                    track_len(node.selector(), flag, keys as usize).unwrap()
                );
            }
            let mut edited = clip.clone();
            edited.key_frames[0] = seed as u32 + 0x100;
            let output = edited.encode().unwrap();
            assert_eq!(
                &output[HEADER_LEN + 4..],
                &bytes[HEADER_LEN + 4..],
                "seed {seed}"
            );
            assert_eq!(
                Clip::parse(&output).unwrap().key_frames[0],
                seed as u32 + 0x100
            );
        }
    }
}
