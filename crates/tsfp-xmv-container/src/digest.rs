//! Packet-span digest lines (T3912): the offline twin of the host's `xmv-packet[n]:` trace witness
//! (`src/host/xmv_packet_witness.c`). Lengths and FNV-1a 64 digests only, never bytes, so the original's trace and
//! this printer can be diffed line by line once a real original run exists.

use crate::{PacketSpan, XmvHeader, parse_packet};
use std::fmt::Write;

/// FNV-1a 64 over `data`, the digest both sides use.
pub fn fnv1a(data: &[u8]) -> u64 {
    data.iter().fold(0xCBF2_9CE4_8422_2325u64, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3)
    })
}

/// One witness line (no newline) for packet `index` at `span`.
pub fn packet_digest_line(
    data: &[u8],
    span: PacketSpan,
    header: &XmvHeader,
    index: usize,
) -> String {
    let first = index == 0;
    let layout = match parse_packet(data, span, header, first) {
        Ok(layout) => layout,
        Err(error) => return format!("xmv-packet[{index}]: reject={error:?}"),
    };
    let video = layout.video_region_size as usize;
    let mut line = format!(
        "xmv-packet[{index}]: next={} video={video} frames={} codec=",
        layout.next_packet_size,
        layout.frames.len()
    );
    match layout.codec_flags {
        Some(flags) => write!(line, "{:08X}", flags.raw).ok(),
        None => write!(line, "-").ok(),
    };
    let region = &data[layout.audio_offset - video..layout.audio_offset];
    write!(
        line,
        " duration={} vfnv={:016x} audio=",
        layout.duration_word,
        fnv1a(region)
    )
    .ok();
    let mut offset = layout.audio_offset;
    for (i, &length) in layout.audio_sizes.iter().enumerate() {
        let end = offset + length as usize;
        let sep = if i == 0 { "" } else { "," };
        write!(line, "{sep}{length}:{:016x}", fnv1a(&data[offset..end])).ok();
        offset = end;
    }
    if layout.audio_sizes.is_empty() {
        line.push('-');
    }
    line
}
