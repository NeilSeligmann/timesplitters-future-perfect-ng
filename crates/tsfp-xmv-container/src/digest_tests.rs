use super::*;
use crate::digest::packet_digest_line;

/// Shared with `tests/c/test_xmv_packet_witness.c`: same bytes, same golden line.
fn fixture() -> Vec<u8> {
    let mut b = vec![0u8; 0x800];
    let put = |b: &mut [u8], at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut b, 0, 0x800);
    put(&mut b, 4, 20 | 1 << 23 | 1 << 31);
    put(&mut b, 8, 77);
    put(&mut b, 12, 16);
    put(&mut b, 16, 0x35);
    put(&mut b, 20, 1 | 33 << 17);
    for (i, byte) in b.iter_mut().enumerate().take(48).skip(24) {
        *byte = (i * 13 + 5) as u8;
    }
    b
}

fn header() -> XmvHeader {
    XmvHeader {
        next_packet_size: 0,
        first_packet_size: 0x800,
        max_packet_size: 0x800,
        version: 4,
        width: 16,
        height: 16,
        duration_ms: 1,
        audio: vec![AudioStream {
            tag: 1,
            channels: 2,
            sample_rate: 44100,
            bits: 16,
            flags: 0,
        }],
    }
}

const GOLDEN: &str = "xmv-packet[1]: next=2048 video=20 frames=1 codec=00000035 duration=77 vfnv=6bade71735c53d43 audio=16:e8f1eed7ca7ce9b5";

#[test]
fn digest_line_matches_shared_golden() {
    let span = PacketSpan {
        offset: 0,
        size: 0x800,
    };
    let line = packet_digest_line(&fixture(), span, &header(), 1);
    assert_eq!(line, GOLDEN);
}

#[test]
fn digest_line_rejects_corruption() {
    let mut data = fixture();
    data[4..8].copy_from_slice(&0x7F_FFFFu32.to_le_bytes());
    let span = PacketSpan {
        offset: 0,
        size: 0x800,
    };
    assert!(packet_digest_line(&data, span, &header(), 1).contains("reject="));
}
