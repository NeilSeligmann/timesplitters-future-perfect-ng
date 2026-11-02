//! XMV container parsing (T3910, under T1677 / T3879): typed file header and packet-chain walker.
//!
//! Scope: container framing only. Codec, audio mixing, decoder object layout and rendering stay behind the
//! `tsfp-engine-video` contract. Layout facts are MEASURED on the six local title movies (contract tier 0, no
//! comparator against the retained decoder yet): the chain below ends exactly at end of file for all six and the
//! largest packet equals the header's max-packet word.
//!
//! Header (little-endian dwords): `0` next-packet size (for packet 1), `4` first packet size, `8` max packet size,
//! `0xC` magic `"Xbox"`, `0x10` version, `0x14` width, `0x18` height, `0x1C` duration (ms), `0x20` audio stream
//! count, then 12 bytes per stream `{tag | channels << 16, sample rate, bits | flags << 16}`. The retained decoder
//! reads `(audio_count * 3 + 9) * 4` header bytes, which this parser reproduces as [`XmvHeader::header_len`].
//! Every packet starts with a dword giving the size of the *following* packet; zero ends the chain. Per-packet
//! sub-headers live in [`packet`] (T3911).

/// `"Xbox"` read as a little-endian dword.
pub const MAGIC: u32 = 0x5862_6f78;
/// The only version measured on the title's movies.
pub const SUPPORTED_VERSION: u32 = 4;
/// Upper bound on audio streams; the retained decoder's tables are fixed-size.
pub const MAX_AUDIO_STREAMS: u32 = 8;
/// Sector granularity of every measured packet size.
pub const PACKET_ALIGN: u32 = 0x800;
const FIXED_HEADER_WORDS: usize = 9;

/// Why a container was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmvError {
    /// Fewer bytes than the header needs.
    TruncatedHeader {
        have: usize,
        need: usize,
    },
    BadMagic(u32),
    UnsupportedVersion(u32),
    ZeroDimension,
    TooManyAudioStreams(u32),
    /// First packet is smaller than the header, or exceeds the declared maximum (the decoder's `E_FAIL` case).
    BadFirstPacket {
        first: u32,
        max: u32,
        header_len: usize,
    },
    /// A packet size is zero-length, unaligned, or above the declared maximum.
    BadPacketSize {
        index: usize,
        size: u32,
    },
    /// A packet extends past the end of the data.
    TruncatedPacket {
        index: usize,
        offset: usize,
        size: u32,
        len: usize,
    },
    /// The chain ended (next size zero) before the data did.
    TrailingBytes {
        offset: usize,
        len: usize,
    },
    /// Data ended while the last packet still announced a successor.
    MissingPacket {
        index: usize,
        announced: u32,
    },
}

/// One audio stream descriptor from the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioStream {
    pub tag: u16,
    pub channels: u16,
    pub sample_rate: u32,
    pub bits: u16,
    pub flags: u16,
}

/// Parsed file header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmvHeader {
    pub next_packet_size: u32,
    pub first_packet_size: u32,
    pub max_packet_size: u32,
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u32,
    pub audio: Vec<AudioStream>,
}

fn dword(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

impl XmvHeader {
    /// Header bytes consumed by the retained decoder's constructor.
    pub fn header_len(&self) -> usize {
        (self.audio.len() * 3 + FIXED_HEADER_WORDS) * 4
    }

    /// Parses and validates the header from the start of the file.
    pub fn parse(data: &[u8]) -> Result<Self, XmvError> {
        if data.len() < FIXED_HEADER_WORDS * 4 {
            return Err(XmvError::TruncatedHeader {
                have: data.len(),
                need: FIXED_HEADER_WORDS * 4,
            });
        }
        let magic = dword(data, 0xC);
        if magic != MAGIC {
            return Err(XmvError::BadMagic(magic));
        }
        let version = dword(data, 0x10);
        if version != SUPPORTED_VERSION {
            return Err(XmvError::UnsupportedVersion(version));
        }
        let (width, height) = (dword(data, 0x14), dword(data, 0x18));
        if width == 0 || height == 0 {
            return Err(XmvError::ZeroDimension);
        }
        let count = dword(data, 0x20);
        if count > MAX_AUDIO_STREAMS {
            return Err(XmvError::TooManyAudioStreams(count));
        }
        let need = (count as usize * 3 + FIXED_HEADER_WORDS) * 4;
        if data.len() < need {
            return Err(XmvError::TruncatedHeader {
                have: data.len(),
                need,
            });
        }
        let audio = (0..count as usize)
            .map(|i| {
                let at = 0x24 + i * 12;
                let (a, c) = (dword(data, at), dword(data, at + 8));
                AudioStream {
                    tag: a as u16,
                    channels: (a >> 16) as u16,
                    sample_rate: dword(data, at + 4),
                    bits: c as u16,
                    flags: (c >> 16) as u16,
                }
            })
            .collect();
        let header = Self {
            next_packet_size: dword(data, 0),
            first_packet_size: dword(data, 4),
            max_packet_size: dword(data, 8),
            version,
            width,
            height,
            duration_ms: dword(data, 0x1C),
            audio,
        };
        let (first, max) = (header.first_packet_size, header.max_packet_size);
        if (first as usize) < need || first > max {
            return Err(XmvError::BadFirstPacket {
                first,
                max,
                header_len: need,
            });
        }
        Ok(header)
    }
}

/// One packet's location in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketSpan {
    pub offset: usize,
    pub size: u32,
}

/// Walks the packet chain of a whole file, checking every length. Returns the header and the spans, which tile
/// the file exactly.
pub fn walk_packets(data: &[u8]) -> Result<(XmvHeader, Vec<PacketSpan>), XmvError> {
    let header = XmvHeader::parse(data)?;
    let mut spans = Vec::new();
    let (mut offset, mut size) = (0usize, header.first_packet_size);
    loop {
        let index = spans.len();
        if size == 0 || size % PACKET_ALIGN != 0 || size > header.max_packet_size {
            return Err(XmvError::BadPacketSize { index, size });
        }
        let end = offset
            .checked_add(size as usize)
            .filter(|&e| e <= data.len());
        let Some(end) = end else {
            return Err(XmvError::TruncatedPacket {
                index,
                offset,
                size,
                len: data.len(),
            });
        };
        let next = dword(data, offset);
        spans.push(PacketSpan { offset, size });
        offset = end;
        size = next;
        if next == 0 {
            break;
        }
        if offset >= data.len() {
            return Err(XmvError::MissingPacket {
                index: index + 1,
                announced: next,
            });
        }
    }
    if offset != data.len() {
        return Err(XmvError::TrailingBytes {
            offset,
            len: data.len(),
        });
    }
    Ok((header, spans))
}

pub mod digest;
pub mod packet;
pub use packet::{CodecFlagWord, FrameSpan, PacketError, PacketLayout, parse_packet};

#[cfg(test)]
mod digest_tests;
#[cfg(test)]
mod packet_tests;
#[cfg(test)]
mod tests;
