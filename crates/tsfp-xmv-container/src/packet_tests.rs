use super::*;

fn put(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn header(audio: usize) -> XmvHeader {
    XmvHeader {
        next_packet_size: 0,
        first_packet_size: 0x800,
        max_packet_size: 0x800,
        version: 4,
        width: 16,
        height: 16,
        duration_ms: 1,
        audio: vec![
            AudioStream {
                tag: 1,
                channels: 2,
                sample_rate: 44100,
                bits: 16,
                flags: 0,
            };
            audio
        ],
    }
}

/// One audio stream, flag word optional, frames of `(n_words, delta)`, `audio` payload bytes.
fn packet(flag: Option<u32>, frames: &[(u32, u32)], audio: u32) -> Vec<u8> {
    let mut b = vec![0u8; 0x800];
    let frame_bytes: usize = frames.iter().map(|f| 8 + 4 * f.0 as usize).sum();
    let video = 4 + 4 * flag.is_some() as usize + frame_bytes;
    let w = video as u32 | (frames.len() as u32) << 23 | (flag.is_some() as u32) << 31;
    put(&mut b, 4, w);
    put(&mut b, 8, 77);
    put(&mut b, 12, audio);
    let mut cur = 16;
    if let Some(f) = flag {
        put(&mut b, cur, f);
        cur += 4;
    }
    for &(n, d) in frames {
        put(&mut b, cur, n | d << 17);
        cur += 8 + 4 * n as usize;
    }
    b
}

fn span() -> PacketSpan {
    PacketSpan {
        offset: 0,
        size: 0x800,
    }
}

#[test]
fn parses_frames_flags_and_audio() {
    let b = packet(Some(53 | 1 << 6), &[(2, 33), (1, 34)], 100);
    let l = parse_packet(&b, span(), &header(1), false).unwrap();
    assert_eq!(l.frames.len(), 2);
    assert_eq!((l.frames[0].payload_len, l.frames[0].delta), (12, 33));
    assert_eq!(
        (l.frames[1].offset, l.frames[1].delta),
        (16 + 4 + 12 + 4, 34)
    );
    assert_eq!((l.duration_word, l.audio_sizes.clone()), (77, vec![100]));
    let f = l.codec_flags.unwrap();
    assert_eq!(f.flags(), [true, false, true, false, true, true]);
    assert_eq!((f.value(), f.unsupported_bits()), (1, 0));
    assert_eq!(l.audio_offset, 12 + video_len(&b));
    assert_eq!(l.trailing, 0x800 - l.audio_offset - 100);
    assert!(l.region_tail_zero);
}

fn video_len(b: &[u8]) -> usize {
    (dword(b, 4) & 0x7F_FFFF) as usize
}

#[test]
fn first_packet_starts_after_header() {
    let h = header(1);
    let hl = h.header_len();
    let mut b = vec![0u8; 0x800];
    let inner = packet(None, &[(1, 33)], 0);
    b[hl..].copy_from_slice(&inner[..0x800 - hl]);
    let l = parse_packet(&b, span(), &h, true).unwrap();
    assert_eq!(l.frames.len(), 1);
    assert!(l.codec_flags.is_none());
}

#[test]
fn rejects_corrupt_packets() {
    let h = header(1);
    let good = packet(None, &[(2, 33)], 10);
    // video region past the packet end
    let mut b = good.clone();
    put(&mut b, 4, 0x7F_FFFF | 1 << 23);
    assert!(matches!(
        parse_packet(&b, span(), &h, false),
        Err(PacketError::VideoRegionOverrun { .. })
    ));
    // frame payload leaves the region
    let mut b = good.clone();
    put(&mut b, 16, 0x1_FFFF);
    assert_eq!(
        parse_packet(&b, span(), &h, false),
        Err(PacketError::FrameOverrun { index: 0 })
    );
    // claims more frames than the region holds
    let mut b = good.clone();
    let w = dword(&b, 4) | 5 << 23;
    put(&mut b, 4, w);
    assert_eq!(
        parse_packet(&b, span(), &h, false),
        Err(PacketError::FrameOverrun { index: 1 })
    );
    // audio larger than what is left, and u32::MAX must not wrap
    let mut b = good.clone();
    put(&mut b, 12, u32::MAX);
    assert!(matches!(
        parse_packet(&b, span(), &h, false),
        Err(PacketError::AudioOverrun { .. })
    ));
    // video region shorter than its own audio size array
    let mut b = good;
    put(&mut b, 4, 0);
    assert!(matches!(
        parse_packet(&b, span(), &h, false),
        Err(PacketError::VideoRegionOverrun { .. })
    ));
    // packet shorter than the fixed sub-header
    let tiny = PacketSpan { offset: 0, size: 8 };
    assert!(matches!(
        parse_packet(&[0; 8], tiny, &h, false),
        Err(PacketError::TruncatedSubHeader { .. })
    ));
}

/// Structural cross-check on owner movies (`TSFP_XMV_DIR`); nothing printed or copied. Not a decoder comparison.
#[test]
fn local_movies_layout_consistent_when_available() {
    let Some(dir) = std::env::var_os("TSFP_XMV_DIR") else {
        return;
    };
    let mut seen = 0;
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        if e.path().extension().is_some_and(|x| x == "xmv") {
            let d = std::fs::read(e.path()).unwrap();
            let (h, spans) = crate::walk_packets(&d).unwrap();
            for (i, s) in spans.iter().enumerate() {
                let l = parse_packet(&d, *s, &h, i == 0).unwrap();
                assert!(
                    l.region_tail_zero && l.trailing == 0,
                    "packet {i} {} {}",
                    l.region_tail_zero,
                    l.trailing
                );
                assert_eq!(l.codec_flags.is_some(), i == 0);
            }
            seen += 1;
        }
    }
    assert!(seen > 0, "TSFP_XMV_DIR holds no .xmv files");
}
