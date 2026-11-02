//! Recovered fixed-width record slices, not an invented mission file envelope.
use tsfp_format_core::*;
#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Setup,
    Rule,
    Point,
}
impl Kind {
    pub fn stride(self) -> usize {
        match self {
            Self::Setup => 0xb4,
            Self::Rule => 0x220,
            Self::Point => 0x58,
        }
    }
}
/// Fixed-width mission records. Parsing rejects a partial final record and encoding
/// preserves every byte, including fields not yet given a typed name.
///
/// ```
/// use tsfp_format_script::{Kind, Records};
/// let mut bytes = vec![0; Kind::Setup.stride()];
/// bytes[12..14].copy_from_slice(&(-7i16).to_le_bytes());
/// let records = Records::parse(&bytes, Kind::Setup).unwrap();
/// assert_eq!(records.signed_index(0).unwrap(), -7);
/// assert_eq!(records.encode().unwrap(), bytes);
/// ```
#[derive(Debug, Clone)]
pub struct Records {
    pub kind: Kind,
    pub records: Vec<Vec<u8>>,
}
impl Records {
    fn record(&self, index: usize) -> Result<&[u8]> {
        let record = self.records.get(index).ok_or("record index")?;
        if record.len() != self.kind.stride() {
            return Err("record width");
        }
        Ok(record)
    }
    pub fn parse(b: &[u8], kind: Kind) -> Result<Self> {
        if b.len() % kind.stride() != 0 {
            return Err("partial script record");
        }
        Ok(Self {
            kind,
            records: b.chunks(kind.stride()).map(|r| r.to_vec()).collect(),
        })
    }
    pub fn field(&self, index: usize, offset: usize) -> Result<u32> {
        // Public records may have been resized after parsing. A field still
        // belongs to the declared record kind, even if extra bytes are present.
        if offset
            .checked_add(4)
            .is_none_or(|end| end > self.kind.stride())
        {
            return Err("field outside record");
        }
        word(self.record(index)?, offset)
    }
    pub fn signed_index(&self, index: usize) -> Result<i16> {
        if !matches!(self.kind, Kind::Setup) {
            return Err("signed setup index only");
        }
        let r = self.record(index)?;
        Ok(i16::from_le_bytes(range(r, 0xc, 2)?.try_into().unwrap()))
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.records.iter().any(|r| r.len() != self.kind.stride()) {
            return Err("record width");
        }
        Ok(self.records.concat())
    }
}
#[test]
fn widths() {
    for k in [Kind::Setup, Kind::Rule, Kind::Point] {
        let b = vec![0x13; k.stride() * 2];
        let r = Records::parse(&b, k).unwrap();
        assert_eq!(r.encode().unwrap(), b);
        assert!(Records::parse(&b[..b.len() - 1], k).is_err());
        assert!(r.field(0, k.stride() - 1).is_err());
    }
}

#[test]
fn record_boundaries_and_signed_setup_indices() {
    for kind in [Kind::Setup, Kind::Rule, Kind::Point] {
        for count in 0..=16 {
            let stride = kind.stride();
            let mut bytes = vec![0u8; count * stride];
            for (index, byte) in bytes.iter_mut().enumerate() {
                *byte = (index as u32).wrapping_mul(73).wrapping_add(count as u32) as u8;
            }
            let records = Records::parse(&bytes, kind).unwrap();
            assert_eq!(records.records.len(), count);
            assert_eq!(records.encode().unwrap(), bytes);
            for index in 0..count {
                assert_eq!(
                    records.field(index, stride - 4).unwrap(),
                    u32::from_le_bytes(
                        bytes[index * stride + stride - 4..(index + 1) * stride]
                            .try_into()
                            .unwrap()
                    )
                );
                assert!(records.field(index, stride - 3).is_err());
                if matches!(kind, Kind::Setup) {
                    assert_eq!(
                        records.signed_index(index).unwrap(),
                        i16::from_le_bytes(
                            bytes[index * stride + 12..index * stride + 14]
                                .try_into()
                                .unwrap()
                        )
                    );
                } else {
                    assert!(records.signed_index(index).is_err());
                }
            }
            assert!(records.field(count, 0).is_err());
            if count > 0 {
                assert!(Records::parse(&bytes[..bytes.len() - 1], kind).is_err());
            }
        }
    }
}

#[test]
fn field_cannot_escape_declared_width_after_public_record_edit() {
    for kind in [Kind::Setup, Kind::Rule, Kind::Point] {
        let mut records = Records::parse(&vec![0; kind.stride()], kind).unwrap();
        records.records[0].extend_from_slice(&0x1234_5678u32.to_le_bytes());
        assert_eq!(records.field(0, kind.stride()), Err("field outside record"));
        assert_eq!(records.field(0, usize::MAX), Err("field outside record"));
        assert_eq!(records.field(0, kind.stride() - 4), Err("record width"));
        assert_eq!(records.encode(), Err("record width"));
    }
}

#[test]
fn accessors_reject_resized_public_records() {
    let mut records = Records::parse(&vec![0; Kind::Setup.stride()], Kind::Setup).unwrap();
    for width in [Kind::Setup.stride() - 1, Kind::Setup.stride() + 1] {
        records.records[0].resize(width, 0);
        assert_eq!(records.field(0, 0), Err("record width"));
        assert_eq!(records.signed_index(0), Err("record width"));
        assert_eq!(records.encode(), Err("record width"));
    }
}

/// Map Maker `.mkr` save container (T1717 envelope): the only real file carrier of
/// 0xB4 setup (tag C7766D, payload 0xC0) and 0x220 rule (tag C7766E) records.
/// Every chunk payload is retained verbatim, so unknown bytes survive. The CRC32 is
/// verified on parse and recomputed on encode. The trailing bytes (zero fill to
/// 0x7FE0 plus the 20-byte XDK signature) are retained and never authenticated.
pub mod mkr {
    use super::*;
    pub const MAGIC: u32 = 0x00C7_7667;
    pub const VERSION: u32 = 0x12D;
    pub const TERMINATOR: u32 = 0x00C7_766A;
    pub const TAG_SETTINGS: u32 = 0x00C7_7668;
    pub const TAG_SETUP: u32 = 0x00C7_766D;
    pub const TAG_RULE: u32 = 0x00C7_766E;
    /// Chunk budget of the original working buffer; the walker never goes past it.
    pub const BUFFER: usize = 0x7FE0;
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Chunk {
        pub tag: u32,
        pub payload: Vec<u8>,
    }
    #[derive(Debug, Clone)]
    pub struct Mkr {
        pub version: u32,
        pub chunks: Vec<Chunk>,
        pub tail: Vec<u8>,
    }
    impl Mkr {
        pub fn parse(b: &[u8]) -> Result<Self> {
            if word(b, 0)? != MAGIC {
                return Err("mkr magic");
            }
            let version = word(b, 4)?;
            if version != VERSION {
                return Err("mkr version");
            }
            let mut at = 8usize;
            let mut chunks = Vec::new();
            loop {
                if at >= BUFFER {
                    return Err("mkr terminator missing");
                }
                let tag = word(b, at)?;
                let size = word(b, at + 4)? as usize;
                if tag == TERMINATOR {
                    if size != at {
                        return Err("mkr terminator offset");
                    }
                    at += 8;
                    break;
                }
                let end = (at + 8).checked_add(size).ok_or("extent overflow")?;
                if end > BUFFER {
                    return Err("mkr chunk past buffer");
                }
                chunks.push(Chunk {
                    tag,
                    payload: range(b, at + 8, size)?.to_vec(),
                });
                at = end;
            }
            let stored = word(b, at)?;
            if crc32(&b[..at]) != stored {
                return Err("mkr crc");
            }
            let m = Self {
                version,
                chunks,
                tail: b[at + 4..].to_vec(),
            };
            Ok(m)
        }
        pub fn encode(&self) -> Result<Vec<u8>> {
            if self.version != VERSION {
                return Err("mkr version");
            }
            // Account for the header and terminator before copying public payloads.
            let mut remaining = BUFFER - 16;
            for chunk in &self.chunks {
                if chunk.tag == TERMINATOR {
                    return Err("terminator tag in chunks");
                }
                u32::try_from(chunk.payload.len()).map_err(|_| "chunk size")?;
                remaining = remaining
                    .checked_sub(8)
                    .and_then(|space| space.checked_sub(chunk.payload.len()))
                    .ok_or("mkr over budget")?;
            }
            let mut b = Vec::new();
            b.extend(MAGIC.to_le_bytes());
            b.extend(self.version.to_le_bytes());
            for c in &self.chunks {
                if c.tag == TERMINATOR {
                    return Err("terminator tag in chunks");
                }
                b.extend(c.tag.to_le_bytes());
                b.extend((u32::try_from(c.payload.len()).map_err(|_| "chunk size")?).to_le_bytes());
                b.extend(&c.payload);
            }
            if b.len() + 8 > BUFFER {
                return Err("mkr over budget");
            }
            b.extend(TERMINATOR.to_le_bytes());
            b.extend((b.len() as u32 - 4).to_le_bytes());
            let crc = crc32(&b);
            b.extend(crc.to_le_bytes());
            b.extend(&self.tail);
            Ok(b)
        }
        /// Payloads of every chunk with `tag`, in file order.
        pub fn payloads(&self, tag: u32) -> Vec<&[u8]> {
            self.chunks
                .iter()
                .filter(|c| c.tag == tag)
                .map(|c| c.payload.as_slice())
                .collect()
        }
        /// 0xB4 setup records (first 0xB4 of each 0xC0 payload; the last 12 bytes are
        /// serialized copy overrun, kept in the chunk and not exposed here).
        pub fn setups(&self) -> Result<Vec<&[u8]>> {
            self.take(TAG_SETUP, Kind::Setup.stride(), 0xC0)
        }
        /// 0x220 rule records (full payload).
        pub fn rules(&self) -> Result<Vec<&[u8]>> {
            self.take(TAG_RULE, Kind::Rule.stride(), 0x220)
        }
        fn take(&self, tag: u32, stride: usize, payload: usize) -> Result<Vec<&[u8]>> {
            self.payloads(tag)
                .into_iter()
                .map(|p| {
                    if p.len() == payload {
                        Ok(&p[..stride])
                    } else {
                        Err("script chunk size")
                    }
                })
                .collect()
        }
        /// Replace the record bytes in the `index`-th chunk of `tag`.
        pub fn set_record(&mut self, tag: u32, index: usize, record: &[u8]) -> Result<()> {
            let c = self
                .chunks
                .iter_mut()
                .filter(|c| c.tag == tag)
                .nth(index)
                .ok_or("chunk index")?;
            let (expected, payload_width) = match tag {
                TAG_SETUP => (Kind::Setup.stride(), 0xc0),
                TAG_RULE => (Kind::Rule.stride(), Kind::Rule.stride()),
                _ => (c.payload.len(), c.payload.len()),
            };
            if record.len() != expected || c.payload.len() != payload_width {
                return Err("record width");
            }
            c.payload[..record.len()].copy_from_slice(record);
            Ok(())
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        pub fn sample(setups: usize, rules: usize) -> Vec<u8> {
            let mut m = Mkr {
                version: VERSION,
                chunks: vec![Chunk {
                    tag: TAG_SETTINGS,
                    payload: vec![7; 0x3dc],
                }],
                tail: vec![0; 0x7fe0 - 0x7c8 - 4 + 20],
            };
            for i in 0..setups {
                m.chunks.push(Chunk {
                    tag: TAG_SETUP,
                    payload: vec![i as u8 + 1; 0xC0],
                });
            }
            for i in 0..rules {
                let mut p = vec![0u8; 0x220];
                p[4] = 1;
                p[0x50] = i as u8;
                m.chunks.push(Chunk {
                    tag: TAG_RULE,
                    payload: p,
                });
            }
            m.encode().unwrap()
        }
        #[test]
        fn roundtrip_with_rules() {
            let b = sample(2, 3);
            let m = Mkr::parse(&b).unwrap();
            assert_eq!(m.setups().unwrap().len(), 2);
            let r = m.rules().unwrap();
            assert_eq!(r.len(), 3);
            assert_eq!(r[2][0x50], 2);
            assert_eq!(m.encode().unwrap(), b);
        }
        #[test]
        fn rejects_damage() {
            let b = sample(1, 1);
            let mut x = b.clone();
            x[20] ^= 1;
            assert!(Mkr::parse(&x).is_err());
            let mut x = b.clone();
            x[0] ^= 1;
            assert!(Mkr::parse(&x).is_err());
            assert!(Mkr::parse(&b[..40]).is_err());
            let mut x = b.clone();
            let i = 8 + 8 + 0x3dc + 4;
            x[i..i + 4].copy_from_slice(&0x7fffu32.to_le_bytes());
            assert!(Mkr::parse(&x).is_err());
        }
        #[test]
        fn rejects_partial_record_without_mutation() {
            let mut m = Mkr::parse(&sample(1, 1)).unwrap();
            for (tag, width) in [
                (TAG_SETUP, Kind::Setup.stride()),
                (TAG_RULE, Kind::Rule.stride()),
            ] {
                let before = m.payloads(tag)[0].to_vec();
                assert_eq!(
                    m.set_record(tag, 0, &vec![0x55; width - 1]),
                    Err("record width")
                );
                assert_eq!(
                    m.set_record(tag, 0, &vec![0x55; width + 1]),
                    Err("record width")
                );
                assert_eq!(m.payloads(tag)[0], before);
            }
        }
        #[test]
        fn rejects_malformed_public_chunk_width_without_mutation() {
            let mut m = Mkr::parse(&sample(1, 1)).unwrap();
            for (tag, width, bad_width) in [
                (TAG_SETUP, Kind::Setup.stride(), Kind::Setup.stride()),
                (TAG_SETUP, Kind::Setup.stride(), 0xc1),
                (TAG_RULE, Kind::Rule.stride(), Kind::Rule.stride() + 1),
            ] {
                let index = m.chunks.iter().position(|chunk| chunk.tag == tag).unwrap();
                m.chunks[index].payload.resize(bad_width, 0x5a);
                let before = m.chunks[index].payload.clone();
                assert_eq!(
                    m.set_record(tag, 0, &vec![0x44; width]),
                    Err("record width")
                );
                assert_eq!(m.chunks[index].payload, before);
                m.chunks[index].payload.resize(
                    if tag == TAG_SETUP {
                        0xc0
                    } else {
                        Kind::Rule.stride()
                    },
                    0,
                );
            }
        }
        #[test]
        fn edit_recomputes_crc() {
            let mut m = Mkr::parse(&sample(0, 1)).unwrap();
            let mut r = m.rules().unwrap()[0].to_vec();
            r[8] = 9;
            m.set_record(TAG_RULE, 0, &r).unwrap();
            let e = m.encode().unwrap();
            assert_eq!(Mkr::parse(&e).unwrap().rules().unwrap()[0][8], 9);
            assert!(m.set_record(TAG_RULE, 1, &r).is_err());
        }
        #[test]
        fn rejects_unsupported_version_before_writing() {
            let mut m = Mkr::parse(&sample(1, 1)).unwrap();
            let original = m.encode().unwrap();
            m.version = VERSION + 1;
            assert_eq!(m.encode(), Err("mkr version"));
            m.version = VERSION;
            assert_eq!(m.encode().unwrap(), original);
        }
        #[test]
        fn encode_checks_chunk_budget_before_copying() {
            let mut m = Mkr {
                version: VERSION,
                chunks: vec![Chunk {
                    tag: TAG_SETUP,
                    payload: vec![0x5a; BUFFER - 24],
                }],
                tail: vec![],
            };
            let encoded = m.encode().unwrap();
            assert_eq!(Mkr::parse(&encoded).unwrap().encode().unwrap(), encoded);
            m.chunks[0].payload.push(0x5a);
            assert_eq!(m.encode(), Err("mkr over budget"));
            m.chunks[0].tag = TERMINATOR;
            assert_eq!(m.encode(), Err("terminator tag in chunks"));
        }
    }
}
