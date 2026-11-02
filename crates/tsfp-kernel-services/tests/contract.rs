// SPDX-License-Identifier: GPL-3.0-or-later
use tsfp_kernel_services::{crypto, identity::DiskIdentity, *};
#[path = "support/memory.rs"]
mod memory;
use memory::Memory;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
#[test]
fn standard_vectors_and_stream_boundaries() {
    // FIPS SHA-1 and RFC 2202 HMAC vectors, independent of either local implementation.
    assert_eq!(
        hex(&crypto::sha1(b"")),
        "da39a3ee5e6b4b0d3255bfef95601890afd80709"
    );
    assert_eq!(
        hex(&crypto::sha1(b"abc")),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
    assert_eq!(
        hex(&crypto::hmac(&[0x0b; 20], b"Hi There", b"", true)),
        "b617318655057264e28bc0b6fb378c8ef146be00"
    );
    assert_eq!(
        hex(&crypto::hmac(
            &[0xaa; 80],
            b"Test Using Larger Than Block-Size Key - Hash Key First",
            b"",
            true
        )),
        "aa4ae5e15272d00e95705637ce8a3b55ed402112"
    );
    let message = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    for split in 0..=message.len() {
        let mut state = crypto::Sha1::default();
        state.update(&message[..split]);
        state.update(&message[split..]);
        assert_eq!(
            hex(&state.finish()),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }
    for byte in 0..=255 {
        let p = crypto::parity(byte);
        assert_eq!(p & 0xfe, byte & 0xfe);
        assert_eq!(p.count_ones() % 2, 1);
    }
}
#[test]
fn typed_threads_reset_and_fail_closed_boundaries() {
    let mut services = RustKernelServices::default();
    let mut memory = Memory(vec![0; 1024]);
    assert!(services.call(99, &[], 1, &mut memory).is_none());
    services.call(142, &[64], 1, &mut memory);
    services.call(142, &[64], 2, &mut memory);
    services.call(142, &[96], 1, &mut memory);
    services.call(139, &[64], 1, &mut memory); // Non-LIFO is honored, counted.
    assert_eq!(services.fp.depth(1), 1);
    assert_eq!(services.fp.depth(2), 1);
    assert_eq!(services.fp.anomalies, 1);
    services.call(139, &[64], 3, &mut memory);
    assert_eq!(services.fp.total(), 2);
    assert_eq!(services.fp.anomalies, 2);
    services.call(139, &[64], 2, &mut memory);
    assert_eq!(services.fp.depth(2), 0);
    assert!(memory.0.iter().all(|&b| b == 0)); // Brackets never manufacture an x87 state image.
    services.call(3, &[1, 99, 2, 3, 4, 5], 1, &mut memory);
    let display = services.av.display.as_ref().unwrap();
    assert_eq!(
        (
            display.register_base,
            display.mode,
            display.format,
            display.pitch,
            display.frame_buffer,
            display.calls
        ),
        (1, 2, 3, 4, 5, 1)
    );
    services.av.widescreen = true;
    services.reset();
    assert!(services.av.widescreen);
    assert_eq!(services.fp.total(), 0);
    assert_eq!(add(u32::MAX, 1), 0);
    assert_eq!(add(0, 1), 0);
    let disk = DiskIdentity {
        model: vec![b'M'; 40],
        serial: vec![b'S'; 20],
        device: (1, 2),
    };
    let before = memory.0.clone();
    assert!(!disk.publish(&mut memory, 64, 68, 128));
    assert_eq!(memory.0, before);
    assert!(!disk.publish(&mut memory, 64, 80, u32::MAX - 32));
    assert_eq!(memory.0, before);
    assert!(disk.publish(&mut memory, 64, 80, 128));
    let read = DiskIdentity::read(&memory, 64, 80).unwrap();
    assert_eq!(read.model, disk.model);
    assert_eq!(read.serial, disk.serial);
    let selection = Selection::default();
    assert!(ORDINALS.iter().all(|&(o, _)| !selection.rust(o)));
}
#[test]
fn guest_faults_and_sha_context_guard() {
    let mut services = RustKernelServices::default();
    let mut memory = Memory(vec![0; 1024]);
    assert_eq!(
        services
            .call(336, &[64, 512, 1], 1, &mut memory)
            .unwrap()
            .eax,
        INVALID_PARAMETER
    );
    assert_eq!(services.crypto.stale, 1);
    services.call(335, &[64], 1, &mut memory);
    memory.put_word(64 + 32, 64);
    assert_eq!(
        services.call(337, &[64, 512], 1, &mut memory).unwrap().eax,
        INVALID_PARAMETER
    );
    assert_eq!(services.crypto.stale, 2);
    assert_eq!(
        services
            .call(338, &[u32::MAX, 0, 128], 1, &mut memory)
            .unwrap()
            .eax,
        INVALID_PARAMETER
    );
    assert_eq!(
        services
            .call(339, &[64, u32::MAX, 128], 1, &mut memory)
            .unwrap()
            .eax,
        INVALID_PARAMETER
    );
    assert_eq!(
        services
            .call(340, &[128, u32::MAX, 0, 0, 0, 0, 512], 1, &mut memory)
            .unwrap()
            .eax,
        INVALID_PARAMETER
    );
    assert_eq!(
        services.call(346, &[0, 0], 1, &mut memory).unwrap().eax,
        SUCCESS
    );
}
