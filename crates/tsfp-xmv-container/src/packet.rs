//! Per-packet layout (T3911): the sub-header the retained `GetNextFrame` (`0x44525D`) reads from each packet.
//!
//! Read from the original code and cross-checked on the six local title movies (every packet of all six: frames fit
//! the video region, the region tail is zero, the audio sizes end exactly at the packet end). Layout, with `p` the
//! packet start (file offset 0 plus the header length for the first packet, otherwise the span offset):
//!
//! - `p+0` size of the following packet (already used by the chain walker);
//! - `p+4` word: bits 0..22 video region size `V` (counted from `p+12`), bits 23..30 frame count, bit 31 codec flag word present;
//! - `p+8` duration word (the original adds it to a running total; its unit is INFERRED);
//! - `p+12` one dword per audio stream: that stream's payload size in bytes;
//! - optional codec flag word (bit 31): bits 0..5 single flags, bits 6..8 a 3-bit value, bits 9.. unnamed;
//! - per frame: header dword (low 17 bits `N`, high 15 bits delta, INFERRED milliseconds), then `4*N + 4` payload
//!   bytes handed to the codec (untouched here);
//! - the video region spans `p+12 .. p+12+V` (it is the audio size array and frames); audio payloads follow back
//!   to back, one per stream, unpadded.
//!
//! The codec flag word has been observed only in each movie's first packet; later packets never set bit 31.

use crate::{PacketSpan, XmvHeader, dword};

const VIDEO_SIZE_MASK: u32 = 0x7F_FFFF;
const FRAME_SIZE_MASK: u32 = 0x1_FFFF;

/// Why a packet's sub-header was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacketError {
    /// Packet too short for the fixed sub-header and audio size array.
    TruncatedSubHeader { need: usize, have: usize },
    /// The video region extends past the packet end.
    VideoRegionOverrun {
        region_end: usize,
        packet_end: usize,
    },
    /// Frame `index` (header or payload) leaves the video region.
    FrameOverrun { index: usize },
    /// Audio payload sizes extend past the packet end.
    AudioOverrun { needed: u64, available: usize },
}

/// One frame inside a packet's video region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSpan {
    /// File offset of the frame header dword.
    pub offset: usize,
    /// Payload bytes after the header dword (`4 * N + 4`), the codec input.
    pub payload_len: usize,
    /// High 15 bits of the frame header; INFERRED milliseconds since the previous frame.
    pub delta: u32,
}

/// Raw codec flag word with the original's field split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecFlagWord {
    pub raw: u32,
}

impl CodecFlagWord {
    /// Bits 0..5, each read by the original into its own byte.
    pub fn flags(self) -> [bool; 6] {
        std::array::from_fn(|i| self.raw >> i & 1 != 0)
    }
    /// Bits 6..8.
    pub fn value(self) -> u32 {
        self.raw >> 6 & 7
    }
    /// Bits 9..31: the original never reads them; unsupported, kept raw.
    pub fn unsupported_bits(self) -> u32 {
        self.raw >> 9
    }
}

/// Parsed packet sub-header and the location of every region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketLayout {
    pub next_packet_size: u32,
    /// Bits 0..22 of the first sub-header word.
    pub video_region_size: u32,
    pub duration_word: u32,
    pub audio_sizes: Vec<u32>,
    pub codec_flags: Option<CodecFlagWord>,
    pub frames: Vec<FrameSpan>,
    /// File offset one past the video region (`p + 12 + V`), the first audio payload.
    pub audio_offset: usize,
    /// Packet bytes after the last audio payload (zero in every measured packet).
    pub trailing: usize,
    /// Zero bytes between the last frame and the region end were all zero.
    pub region_tail_zero: bool,
}

/// Parses the sub-header of `span`. `first` selects the header-offset start used by the first packet.
pub fn parse_packet(
    data: &[u8],
    span: PacketSpan,
    header: &XmvHeader,
    first: bool,
) -> Result<PacketLayout, PacketError> {
    let end = span.offset + span.size as usize;
    let p = span.offset + if first { header.header_len() } else { 0 };
    let streams = header.audio.len();
    let fixed = 12 + 4 * streams;
    if p + fixed > end || end > data.len() {
        return Err(PacketError::TruncatedSubHeader {
            need: fixed,
            have: end.saturating_sub(p),
        });
    }
    let word = dword(data, p + 4);
    let video = (word & VIDEO_SIZE_MASK) as usize;
    let count = (word >> 23 & 0xFF) as usize;
    let region_end = p + 12 + video;
    if region_end > end || region_end < p + fixed {
        return Err(PacketError::VideoRegionOverrun {
            region_end,
            packet_end: end,
        });
    }
    let audio_sizes: Vec<u32> = (0..streams).map(|i| dword(data, p + 12 + 4 * i)).collect();
    let mut cur = p + fixed;
    let mut codec_flags = None;
    if word >> 31 != 0 {
        if cur + 4 > region_end {
            return Err(PacketError::FrameOverrun { index: 0 });
        }
        codec_flags = Some(CodecFlagWord {
            raw: dword(data, cur),
        });
        cur += 4;
    }
    let mut frames = Vec::with_capacity(count);
    for index in 0..count {
        if cur + 4 > region_end {
            return Err(PacketError::FrameOverrun { index });
        }
        let h = dword(data, cur);
        let payload_len = (h & FRAME_SIZE_MASK) as usize * 4 + 4;
        if cur + 4 + payload_len > region_end {
            return Err(PacketError::FrameOverrun { index });
        }
        frames.push(FrameSpan {
            offset: cur,
            payload_len,
            delta: h >> 17,
        });
        cur += 4 + payload_len;
    }
    let audio_total: u64 = audio_sizes.iter().map(|&s| u64::from(s)).sum();
    let available = end - region_end;
    if audio_total > available as u64 {
        return Err(PacketError::AudioOverrun {
            needed: audio_total,
            available,
        });
    }
    Ok(PacketLayout {
        next_packet_size: dword(data, p),
        video_region_size: video as u32,
        duration_word: dword(data, p + 8),
        audio_sizes,
        codec_flags,
        frames,
        audio_offset: region_end,
        trailing: available - audio_total as usize,
        region_tail_zero: data[cur..region_end].iter().all(|&b| b == 0),
    })
}
