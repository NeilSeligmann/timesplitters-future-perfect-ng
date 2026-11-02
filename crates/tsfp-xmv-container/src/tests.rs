use super::*;

/// Synthetic container: one stereo PCM stream, packets of `sizes` (first is `sizes[0]`).
fn build(sizes: &[u32], max: u32) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, &s) in sizes.iter().enumerate() {
        let mut p = vec![0u8; s as usize];
        let next = sizes.get(i + 1).copied().unwrap_or(0);
        p[..4].copy_from_slice(&next.to_le_bytes());
        if i == 0 {
            let w = |p: &mut Vec<u8>, at: usize, v: u32| {
                p[at..at + 4].copy_from_slice(&v.to_le_bytes())
            };
            w(&mut p, 4, s);
            w(&mut p, 8, max);
            w(&mut p, 0xC, MAGIC);
            w(&mut p, 0x10, 4);
            w(&mut p, 0x14, 640);
            w(&mut p, 0x18, 480);
            w(&mut p, 0x1C, 1000);
            w(&mut p, 0x20, 1);
            w(&mut p, 0x24, 1 | 2 << 16);
            w(&mut p, 0x28, 44100);
            w(&mut p, 0x2C, 16);
        }
        out.extend(p);
    }
    out
}

#[test]
fn parses_header_and_tiles_file() {
    let d = build(&[0x1000, 0x800, 0x1000], 0x1000);
    let (h, spans) = walk_packets(&d).unwrap();
    assert_eq!((h.width, h.height, h.header_len()), (640, 480, 0x30));
    assert_eq!(
        h.audio[0],
        AudioStream {
            tag: 1,
            channels: 2,
            sample_rate: 44100,
            bits: 16,
            flags: 0
        }
    );
    let total: usize = spans.iter().map(|s| s.size as usize).sum();
    assert_eq!((spans.len(), total), (3, d.len()));
    assert_eq!(spans[2].offset, 0x1800);
}

#[test]
fn rejects_header_corruption() {
    let good = build(&[0x1000, 0x800], 0x1000);
    assert!(matches!(
        XmvHeader::parse(&good[..0x20]),
        Err(XmvError::TruncatedHeader { .. })
    ));
    let mut d = good.clone();
    d[0xC] ^= 1;
    assert!(matches!(XmvHeader::parse(&d), Err(XmvError::BadMagic(_))));
    let mut d = good.clone();
    d[0x10] = 3;
    assert_eq!(XmvHeader::parse(&d), Err(XmvError::UnsupportedVersion(3)));
    let mut d = good.clone();
    d[0x14] = 0;
    d[0x15] = 0;
    assert_eq!(XmvHeader::parse(&d), Err(XmvError::ZeroDimension));
    let mut d = good.clone();
    d[0x20] = 200;
    assert_eq!(
        XmvHeader::parse(&d),
        Err(XmvError::TooManyAudioStreams(200))
    );
    // Stream count that needs more header than the buffer holds.
    assert!(matches!(XmvHeader::parse(&good[..0x30]), Ok(_)));
    let mut d = good[..0x30].to_vec();
    d[0x20] = 4;
    assert!(matches!(
        XmvHeader::parse(&d),
        Err(XmvError::TruncatedHeader { need: 0x54, .. })
    ));
    // max smaller than first packet (decoder E_FAIL case)
    let d = build(&[0x1000, 0x800], 0x800);
    assert!(matches!(
        XmvHeader::parse(&d),
        Err(XmvError::BadFirstPacket { .. })
    ));
}

#[test]
fn rejects_chain_corruption() {
    let good = build(&[0x1000, 0x800, 0x800], 0x1000);
    // truncated last packet
    assert!(matches!(
        walk_packets(&good[..good.len() - 4]),
        Err(XmvError::TruncatedPacket { index: 2, .. })
    ));
    // data ends exactly where a successor is announced
    assert!(matches!(
        walk_packets(&good[..0x1800]),
        Err(XmvError::MissingPacket { index: 2, .. })
    ));
    // trailing bytes after the zero terminator
    let mut d = good.clone();
    d.extend([0u8; 16]);
    assert!(matches!(
        walk_packets(&d),
        Err(XmvError::TrailingBytes { .. })
    ));
    // zero, unaligned, oversize next-size
    for bad in [0u32, 0x801, 0x2000] {
        let mut d = good.clone();
        d[0x1000..0x1004].copy_from_slice(&bad.to_le_bytes());
        let r = walk_packets(&d);
        if bad == 0 {
            assert!(matches!(r, Err(XmvError::TrailingBytes { .. })));
        } else {
            assert!(matches!(r, Err(XmvError::BadPacketSize { index: 2, .. })));
        }
    }
    // a size that would overflow offset arithmetic
    let mut d = good;
    d[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        walk_packets(&d),
        Err(XmvError::BadPacketSize { index: 1, .. })
    ));
}

/// Optional measurement over owner movies: `TSFP_XMV_DIR` names a directory; nothing is printed or copied.
#[test]
fn local_movies_tile_exactly_when_available() {
    let Some(dir) = std::env::var_os("TSFP_XMV_DIR") else {
        return;
    };
    let mut seen = 0;
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        if e.path().extension().is_some_and(|x| x == "xmv") {
            let d = std::fs::read(e.path()).unwrap();
            let (h, spans) = walk_packets(&d).unwrap();
            let max = spans.iter().map(|s| s.size).max().unwrap();
            assert_eq!(max, h.max_packet_size);
            seen += 1;
        }
    }
    assert!(seen > 0, "TSFP_XMV_DIR holds no .xmv files");
}
