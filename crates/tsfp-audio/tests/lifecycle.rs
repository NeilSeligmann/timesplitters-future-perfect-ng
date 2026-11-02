//! Lifecycle conformance (T2046 9b): the Rust device facade and stream/buffer headers over the Rust
//! `GuestRegions` heap, checked against the values the C host (`dsound_device.c` etc.) produces.

mod common;
use common::World;
use tsfp_audio::adapter::GuestMemory;
use tsfp_audio::lifecycle::*;

const OUT: u32 = 0x0010_0100;
const DESC: u32 = 0x0010_0200;
const FMT: u32 = 0x0010_0300;
const SCOPE: Scope = Scope {
    flags: 0,
    descriptor_address: DESC,
    format_address: FMT,
};

fn words(w: &World, a: u32, n: usize) -> Vec<u32> {
    w.read_bytes(a, n * 4)
        .unwrap()
        .chunks(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn started() -> (World, Device, u32) {
    let mut w = World::new();
    let mut d = Device::new();
    assert_eq!(d.create(&mut w, 0, OUT, 0, false, true).ok(), Some(0));
    let iface = w.read_u32(OUT).unwrap();
    (w, d, iface)
}

#[test]
fn startup_device_matches_c_facade() {
    let (w, d, iface) = started();
    let at = d.owned_address();
    assert_eq!(iface, at + 8);
    assert_eq!(w.read_u32(SINGLETON), Some(at));
    assert_eq!(d.reference_count(), 6);
    assert_eq!(
        words(&w, at, 11),
        vec![DEVICE_VTABLE, 6, 0, 0, at + 16, at + 16, 0, 0, 0, 0, 0]
    );
    assert!(d.validate_owned(&w, iface));
}

#[test]
fn codec_not_ready_reports_nodriver_with_one_reference() {
    let mut w = World::new();
    let mut d = Device::new();
    assert_eq!(
        d.create(&mut w, 0, OUT, 0, false, false).ok(),
        Some(DSERR_NODRIVER)
    );
    assert_eq!(d.reference_count(), 1);
    assert_eq!(
        w.read_u32(OUT),
        Some(0),
        "no interface is published on failure"
    );
}

#[test]
fn cached_create_adds_one_reference_and_movie_release_takes_it_back() {
    let (mut w, mut d, iface) = started();
    d.movie_calls = true;
    assert!(d.create(&mut w, 0, OUT, 0, false, true).is_ok());
    assert_eq!(d.reference_count(), 7);
    assert_eq!(d.movie_references(), 0);
    assert_eq!(d.create(&mut w, 0, OUT, 0, true, true).ok(), Some(0));
    assert_eq!((d.reference_count(), d.movie_references()), (8, 1));
    assert_eq!(
        w.read_u32(d.owned_address() + 4),
        Some(8),
        "guest word tracks the shadow count"
    );
    assert_eq!(d.release_movie(&mut w, iface).ok(), Some(7));
    assert!(
        d.release_movie(&mut w, iface).is_err(),
        "no movie reference is outstanding"
    );
    assert_eq!(d.reference_count(), 7);
}

#[test]
fn refusals_mirror_the_oracle() {
    let mut w = World::new();
    let mut d = Device::new();
    assert!(d.create(&mut w, 1, OUT, 0, false, true).is_err(), "GUID");
    assert!(d.create(&mut w, 0, OUT, 7, false, true).is_err(), "outer");
    assert!(
        d.create(&mut w, 0, SINGLETON, 0, false, true).is_err(),
        "output aliases singleton"
    );
    assert!(
        d.create(&mut w, 0, 0x7000_0000, 0, false, true).is_err(),
        "output unmapped"
    );
    assert!(
        d.create(&mut w, 0, OUT, 0, true, true).is_err(),
        "movie create needs the startup device"
    );
    w.write_u32(SINGLETON, 0x1234);
    assert!(
        d.create(&mut w, 0, OUT, 0, false, true).is_err(),
        "foreign singleton"
    );
    w.write_u32(SINGLETON, 0);
    w.fail_heaps = true;
    assert_eq!(
        d.create(&mut w, 0, OUT, 0, false, true).ok(),
        Some(E_OUTOFMEMORY)
    );
    assert_eq!(d.owned_address(), 0);
}

#[test]
fn tampered_header_is_refused() {
    let (mut w, mut d, _) = started();
    let at = d.owned_address();
    w.write_u32(at + 8, 99);
    assert!(d.create(&mut w, 0, OUT, 0, false, true).is_err());
    assert!(
        !d.reset_checked(&mut w),
        "a changed header is not destroyed silently"
    );
    assert_eq!(
        w.read_u32(SINGLETON),
        Some(at),
        "refused reset leaves guest state intact"
    );
}

#[test]
fn stream_and_buffer_headers_and_references() {
    let (mut w, mut d, _) = started();
    let at = d.owned_address();
    let mut objs = Objects::new();
    assert_eq!(
        objs.create(&mut d, &mut w, Kind::Stream, SCOPE, OUT + 8)
            .ok(),
        Some(0)
    );
    assert_eq!(
        objs.create(&mut d, &mut w, Kind::Buffer, SCOPE, OUT + 16)
            .ok(),
        Some(0)
    );
    assert_eq!(d.reference_count(), 8);
    assert_eq!(w.read_u32(at + 4), Some(8));
    assert_eq!(d.lease_count(), 2);
    let stream = w.read_u32(OUT + 8).unwrap();
    let buffer = w.read_u32(OUT + 16).unwrap();
    let s = words(&w, stream, 10);
    assert_eq!(&s[..4], &[STREAM_VTABLE, STREAM_SECONDARY, 1, at]);
    assert!(s[4..].iter().all(|&x| x == 0));
    let header = buffer - BUFFER_PUBLIC_OFFSET;
    let b = words(&w, header, 9);
    assert_eq!(&b[..3], &[BUFFER_VTABLE, 1, at]);
    assert_eq!(objs.find(buffer).map(|o| o.header_address), Some(header));
    assert_eq!((objs.count(Kind::Stream), objs.count(Kind::Buffer)), (1, 1));
    // serials are issued in order and never reused
    assert_eq!(objs.live[1].lease.serial, 1);
    assert_eq!(objs.live[0].lease.serial, 2);
}

#[test]
fn checked_reset_releases_leases_then_device() {
    let (mut w, mut d, _) = started();
    let mut objs = Objects::new();
    for i in 0..3 {
        let kind = if i % 2 == 0 {
            Kind::Stream
        } else {
            Kind::Buffer
        };
        assert_eq!(
            objs.create(&mut d, &mut w, kind, SCOPE, OUT + 8 * (i + 1))
                .ok(),
            Some(0)
        );
    }
    assert_eq!(d.reference_count(), 9);
    assert!(
        !d.reset_checked(&mut w),
        "device reset refuses while leases are live"
    );
    assert!(objs.reset_checked(&mut d, &mut w));
    assert_eq!((d.reference_count(), d.lease_count()), (6, 0));
    assert!(d.reset_checked(&mut w));
    assert_eq!(w.read_u32(SINGLETON), Some(0));
    assert_eq!(d.owned_address(), 0);
    assert_eq!(w.regions.heap_count(), 0, "every private heap is destroyed");
    assert_eq!(d.create(&mut w, 0, OUT, 0, false, true).ok(), Some(0));
    assert_ne!(d.owned_address(), 0);
}

#[test]
fn child_refusals_and_out_of_memory() {
    let (mut w, mut d, _) = started();
    let heaps = w.regions.heap_count();
    let mut objs = Objects::new();
    assert!(
        objs.create(&mut d, &mut w, Kind::Stream, SCOPE, SINGLETON)
            .is_err(),
        "output aliases singleton"
    );
    assert!(
        objs.create(&mut d, &mut w, Kind::Buffer, SCOPE, FMT + 4)
            .is_err(),
        "output aliases format input"
    );
    assert!(
        objs.create(&mut d, &mut w, Kind::Stream, SCOPE, 0x7000_0000)
            .is_err(),
        "unmapped output"
    );
    assert_eq!(
        (d.reference_count(), d.lease_count()),
        (6, 0),
        "refused creates leave the count alone"
    );
    assert_eq!(
        w.regions.heap_count(),
        heaps,
        "refused creates free their private heap"
    );
    w.fail_heaps = true;
    assert_eq!(
        objs.create(&mut d, &mut w, Kind::Stream, SCOPE, OUT + 8)
            .ok(),
        Some(E_OUTOFMEMORY)
    );
    assert_eq!(d.reference_count(), 6);
    let mut w2 = World::new();
    let mut d2 = Device::new();
    assert!(
        Objects::new()
            .create(&mut d2, &mut w2, Kind::Stream, SCOPE, OUT)
            .is_err(),
        "no device"
    );
}

#[test]
fn create_note_rejection_rolls_back_the_unpublished_device() {
    let mut w = World::new();
    let mut d = Device::new();
    let refusal = d
        .create_with_note(&mut w, 0, OUT, 0, false, true, |_, _| false)
        .unwrap_err();
    assert_eq!(
        refusal.0,
        "PCM epoch rejected retained audio or inactive renderer"
    );
    assert_eq!(d.owned_address(), 0, "nothing published");
    assert_eq!(w.read_u32(SINGLETON), Some(0));
    assert_eq!(d.create(&mut w, 0, OUT, 0, false, true).ok(), Some(0));
}

#[test]
fn release_with_reports_whether_prepare_ran() {
    let (mut w, mut d, iface) = started();
    let heap = w.heap_create();
    let block = w.heap_alloc(heap, 40);
    let child = Child {
        heap,
        address: block,
        bytes: 40,
    };
    let lease = d
        .acquire_lease(&mut w, iface, |_, _| Some(child))
        .expect("lease");
    let wrong = Lease {
        serial: lease.serial + 9,
        ..lease
    };
    assert_eq!(
        d.release_lease_with(&mut w, &wrong, |_| panic!("prepare must not run")),
        Err(ReleaseError::Refused(LeaseError::Invalid))
    );
    assert_eq!(
        d.release_lease_with(&mut w, &lease, |_| None),
        Err(ReleaseError::AfterPrepare(LeaseError::PrepareFailed))
    );
    let other = Child { bytes: 41, ..child };
    assert_eq!(
        d.release_lease_with(&mut w, &lease, |_| Some(other)),
        Err(ReleaseError::AfterPrepare(LeaseError::Invalid))
    );
    assert_eq!(d.lease_count(), 1, "refusals keep the lease");
    assert_eq!(
        d.release_lease_with(&mut w, &lease, |_| Some(child)),
        Ok(())
    );
    assert_eq!(d.lease_count(), 0);
}

#[test]
fn stream_create_refuses_an_unprotected_secondary_vtable_target() {
    let (mut w, mut d, _) = started();
    assert!(w.write_u32(STREAM_SECONDARY + 8, 0x0123_4567));
    let mut objs = Objects::new();
    let refusal = objs
        .create(&mut d, &mut w, Kind::Stream, SCOPE, OUT + 8)
        .unwrap_err();
    assert_eq!(refusal.0, "original vtable contains an unprotected target");
    assert_eq!(d.lease_count(), 0, "the lease transaction aborted");
}
