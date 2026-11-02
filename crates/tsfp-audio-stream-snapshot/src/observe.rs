//! Observer path of the stream adapter: builds the passive snapshot of one owned stream
//! from the Rust-owned lease records, in the order and with the refusal texts of the C
//! `access_owned` observer branch (dsound_stream.c). The caller holds the device ownership.

use crate::{Lease, Routing, Snapshot, tsfp_audio_stream_snapshot_project_rust};
use std::ffi::c_char;
use tsfp_audio::stream_lease::{
    StreamControlState, StreamFormatState, StreamLeaseScope, StreamSpatialState, StreamVolumeState,
};

/// Mirrors `dsound_stream_frequency_state` (12 bytes).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FrequencyState {
    pub accepted_sets: u32,
    pub current_hz: u32,
    pub pitch: i32,
}

const _: () = assert!(std::mem::size_of::<FrequencyState>() == 12);

/// Where the observer reads Rust-owned records; each method is `false` when the record is
/// unavailable for the lease. The real implementation is the `tsfp_audio_stream_*_rust` exports.
pub trait Sources {
    fn identity(&self, stream: u32) -> Option<Lease>;
    fn scope(&self, stream: u32, lease: &Lease) -> Option<StreamLeaseScope>;
    fn format(&self, stream: u32, lease: &Lease) -> Option<StreamFormatState>;
    fn volume(&self, stream: u32, lease: &Lease) -> Option<StreamVolumeState>;
    fn spatial(&self, stream: u32, lease: &Lease) -> Option<StreamSpatialState>;
    fn control(&self, stream: u32, lease: &Lease) -> Option<StreamControlState>;
    fn header(&self, stream: u32, lease: &Lease) -> Option<[u32; 10]>;
    fn publication_address(&self, stream: u32) -> Option<u32>;
    fn frequency(&self, stream: u32, serial: u64) -> Option<FrequencyState>;
    fn routing(&self, stream: u32, serial: u64) -> Option<Routing>;
}

/// Builds the snapshot of `stream`. `Err` carries the C adapter's `access_request.error` text.
///
/// The C Rust-host build keys the node by address only, so `stream_heap` stays 0 here.
pub fn observe<S: Sources>(s: &S, stream: u32) -> Result<Snapshot, &'static str> {
    let lease = s
        .identity(stream)
        .ok_or("Rust stream lease identity is unavailable")?;
    let scope = s
        .scope(stream, &lease)
        .ok_or("Rust stream scope is unavailable for this lease")?;
    let format = s
        .format(stream, &lease)
        .ok_or("Rust stream format sidecar is unavailable for this lease")?;
    let volume = s
        .volume(stream, &lease)
        .ok_or("Rust stream volume sidecar is unavailable for this lease")?;
    let spatial = s
        .spatial(stream, &lease)
        .ok_or("Rust stream spatial sidecar is unavailable for this lease")?;
    let control = s
        .control(stream, &lease)
        .ok_or("Rust stream control sidecar is unavailable for this lease")?;
    let mut format_rate = scope.sample_rate;
    if format.sets != 0 {
        format_rate = u32::from_le_bytes([
            format.bytes[4],
            format.bytes[5],
            format.bytes[6],
            format.bytes[7],
        ]);
    }
    // Zero is a valid bit pattern for every field of this C-compatible record.
    let mut out: Snapshot = unsafe { std::mem::zeroed() };
    out.lease = lease;
    out.stream_address = stream;
    out.header = s
        .header(stream, &lease)
        .ok_or("Rust stream header snapshot is unavailable for this lease")?;
    out.publication_address = s
        .publication_address(stream)
        .ok_or("Rust stream publication address is unavailable")?;
    out.scope = scope;
    unsafe {
        tsfp_audio_stream_snapshot_project_rust(&mut out, &format, &volume, &spatial, &control)
    };
    let frequency = s
        .frequency(stream, lease.serial)
        .ok_or("Rust frequency sidecar is unavailable for this stream generation")?;
    out.frequency_sets = frequency.accepted_sets;
    out.source_rate_hz = if frequency.accepted_sets == 0 || frequency.current_hz == 0 {
        format_rate
    } else {
        frequency.current_hz
    };
    out.frequency_pitch = frequency.pitch;
    out.routing = s
        .routing(stream, lease.serial)
        .ok_or("Rust routing sidecar is unavailable for this lease")?;
    Ok(out)
}

#[cfg(not(test))]
mod exports {
    use super::*;
    unsafe extern "C" {
        fn tsfp_audio_stream_lease_identity_rust(stream: u32, out: *mut Lease) -> bool;
        fn tsfp_audio_stream_lease_scope_rust(
            stream: u32,
            lease: *const Lease,
            out: *mut StreamLeaseScope,
        ) -> bool;
        fn tsfp_audio_stream_lease_format_state_rust(
            stream: u32,
            lease: *const Lease,
            out: *mut StreamFormatState,
        ) -> bool;
        fn tsfp_audio_stream_lease_volume_state_rust(
            stream: u32,
            lease: *const Lease,
            out: *mut StreamVolumeState,
        ) -> bool;
        fn tsfp_audio_stream_lease_spatial_state_rust(
            stream: u32,
            lease: *const Lease,
            out: *mut StreamSpatialState,
        ) -> bool;
        fn tsfp_audio_stream_lease_control_state_rust(
            stream: u32,
            lease: *const Lease,
            out: *mut StreamControlState,
        ) -> bool;
        fn tsfp_audio_stream_lease_header_rust(
            stream: u32,
            lease: *const Lease,
            out: *mut u32,
        ) -> bool;
        fn tsfp_audio_stream_lease_publication_address_rust(stream: u32, out: *mut u32) -> bool;
        fn tsfp_audio_stream_frequency_state_rust(
            stream: u32,
            serial: u64,
            out: *mut FrequencyState,
        ) -> bool;
        fn tsfp_audio_stream_routing_load_rust(stream: u32, serial: u64, out: *mut Routing)
        -> bool;
    }

    /// The exported Rust lease/routing records, linked in the same library.
    struct Exports;

    fn fetch<T>(f: impl FnOnce(*mut T) -> bool) -> Option<T> {
        // Zero is valid for every plain record fetched here; the callee overwrites it on success.
        let mut value: T = unsafe { std::mem::zeroed() };
        f(&mut value).then_some(value)
    }

    impl Sources for Exports {
        fn identity(&self, stream: u32) -> Option<Lease> {
            fetch(|o| unsafe { tsfp_audio_stream_lease_identity_rust(stream, o) })
        }
        fn scope(&self, stream: u32, lease: &Lease) -> Option<StreamLeaseScope> {
            fetch(|o| unsafe { tsfp_audio_stream_lease_scope_rust(stream, lease, o) })
        }
        fn format(&self, stream: u32, lease: &Lease) -> Option<StreamFormatState> {
            fetch(|o| unsafe { tsfp_audio_stream_lease_format_state_rust(stream, lease, o) })
        }
        fn volume(&self, stream: u32, lease: &Lease) -> Option<StreamVolumeState> {
            fetch(|o| unsafe { tsfp_audio_stream_lease_volume_state_rust(stream, lease, o) })
        }
        fn spatial(&self, stream: u32, lease: &Lease) -> Option<StreamSpatialState> {
            fetch(|o| unsafe { tsfp_audio_stream_lease_spatial_state_rust(stream, lease, o) })
        }
        fn control(&self, stream: u32, lease: &Lease) -> Option<StreamControlState> {
            fetch(|o| unsafe { tsfp_audio_stream_lease_control_state_rust(stream, lease, o) })
        }
        fn header(&self, stream: u32, lease: &Lease) -> Option<[u32; 10]> {
            fetch(|o: *mut [u32; 10]| unsafe {
                tsfp_audio_stream_lease_header_rust(stream, lease, o.cast())
            })
        }
        fn publication_address(&self, stream: u32) -> Option<u32> {
            fetch(|o| unsafe { tsfp_audio_stream_lease_publication_address_rust(stream, o) })
        }
        fn frequency(&self, stream: u32, serial: u64) -> Option<FrequencyState> {
            fetch(|o| unsafe { tsfp_audio_stream_frequency_state_rust(stream, serial, o) })
        }
        fn routing(&self, stream: u32, serial: u64) -> Option<Routing> {
            fetch(|o| unsafe { tsfp_audio_stream_routing_load_rust(stream, serial, o) })
        }
    }

    /// Fills `out` with the snapshot of `stream`; on failure returns false and sets `*error` to a
    /// static NUL-terminated refusal text. Replaces the C observer branch of `access_owned`.
    ///
    /// # Safety
    /// `out` and `error` must be valid for writes; the caller holds the stream's device ownership.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn tsfp_audio_stream_observe_rust(
        stream: u32,
        out: *mut Snapshot,
        error: *mut *const c_char,
    ) -> bool {
        match observe(&Exports, stream) {
            Ok(snapshot) => {
                unsafe { out.write(snapshot) };
                true
            }
            Err(text) => {
                unsafe { error.write(error_text(text)) };
                false
            }
        }
    }
}

#[cfg(not(test))]
pub use exports::tsfp_audio_stream_observe_rust;

/// Static C strings for the refusal texts (the C adapter keeps the pointer in `access_request`).
pub(crate) fn error_text(text: &'static str) -> *const c_char {
    const TEXTS: &[&str] = &[
        "Rust frequency sidecar is unavailable for this lease\0",
        "frequency needs a started stereo or fully configured mono PCM stream\0",
        "frequency or guest control word outside measured domain\0",
        "frequency PCM/completion transaction refused\0",
        "Rust stream lease identity is unavailable\0",
        "Rust stream scope is unavailable for this lease\0",
        "Rust stream format sidecar is unavailable for this lease\0",
        "Rust stream volume sidecar is unavailable for this lease\0",
        "Rust stream spatial sidecar is unavailable for this lease\0",
        "Rust stream control sidecar is unavailable for this lease\0",
        "Rust stream header snapshot is unavailable for this lease\0",
        "Rust stream publication address is unavailable\0",
        "Rust frequency sidecar is unavailable for this stream generation\0",
        "Rust routing sidecar is unavailable for this lease\0",
    ];
    TEXTS
        .iter()
        .find(|t| t.trim_end_matches('\0') == text)
        .map_or(c"Rust stream observer refused".as_ptr(), |t| {
            t.as_ptr().cast()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        missing: &'static str,
        frequency: FrequencyState,
        format_sets: u32,
    }

    fn zeroed<T>() -> T {
        unsafe { std::mem::zeroed() }
    }

    impl Fake {
        fn has(&self, name: &str) -> bool {
            self.missing != name
        }
    }

    impl Sources for Fake {
        fn identity(&self, stream: u32) -> Option<Lease> {
            self.has("identity").then(|| Lease {
                device_heap: 1,
                internal_address: stream + 8,
                serial: 77,
            })
        }
        fn scope(&self, _: u32, _: &Lease) -> Option<StreamLeaseScope> {
            self.has("scope").then(|| {
                let mut v: StreamLeaseScope = zeroed();
                v.sample_rate = 48000;
                v
            })
        }
        fn format(&self, _: u32, _: &Lease) -> Option<StreamFormatState> {
            self.has("format").then(|| {
                let mut v: StreamFormatState = zeroed();
                v.sets = self.format_sets;
                v.bytes[4..8].copy_from_slice(&22050u32.to_le_bytes());
                v
            })
        }
        fn volume(&self, _: u32, _: &Lease) -> Option<StreamVolumeState> {
            self.has("volume").then(zeroed)
        }
        fn spatial(&self, _: u32, _: &Lease) -> Option<StreamSpatialState> {
            self.has("spatial").then(zeroed)
        }
        fn control(&self, _: u32, _: &Lease) -> Option<StreamControlState> {
            self.has("control").then(zeroed)
        }
        fn header(&self, _: u32, _: &Lease) -> Option<[u32; 10]> {
            self.has("header").then_some([5; 10])
        }
        fn publication_address(&self, _: u32) -> Option<u32> {
            self.has("publication").then_some(0xabc)
        }
        fn frequency(&self, _: u32, _: u64) -> Option<FrequencyState> {
            self.has("frequency").then_some(self.frequency)
        }
        fn routing(&self, _: u32, _: u64) -> Option<Routing> {
            self.has("routing").then(|| {
                let mut v: Routing = zeroed();
                v.count = 3;
                v
            })
        }
    }

    #[test]
    fn assembles_identity_header_and_zero_heap() {
        let snap = observe(&Fake::default(), 0x4000).unwrap();
        assert_eq!(
            (snap.stream_address, snap.stream_heap, snap.header[0]),
            (0x4000, 0, 5)
        );
        assert_eq!(
            (
                snap.lease.serial,
                snap.publication_address,
                snap.routing.count
            ),
            (77, 0xabc, 3)
        );
    }

    #[test]
    fn source_rate_follows_frequency_then_format_then_scope() {
        let rate = |f: &Fake| observe(f, 1).unwrap().source_rate_hz;
        assert_eq!(rate(&Fake::default()), 48000);
        let formatted = Fake {
            format_sets: 1,
            ..Fake::default()
        };
        assert_eq!(rate(&formatted), 22050);
        let mut set = Fake {
            format_sets: 1,
            ..Fake::default()
        };
        set.frequency = FrequencyState {
            accepted_sets: 2,
            current_hz: 11025,
            pitch: -4,
        };
        let snap = observe(&set, 1).unwrap();
        assert_eq!(
            (
                snap.source_rate_hz,
                snap.frequency_sets,
                snap.frequency_pitch
            ),
            (11025, 2, -4)
        );
        set.frequency.current_hz = 0;
        assert_eq!(rate(&set), 22050);
        set.frequency.accepted_sets = 0;
        set.frequency.current_hz = 9;
        assert_eq!(rate(&set), 22050);
    }

    #[test]
    fn each_missing_record_reports_its_c_refusal_text() {
        let cases = [
            ("identity", "lease identity"),
            ("scope", "stream scope"),
            ("format", "format sidecar"),
            ("volume", "volume sidecar"),
            ("spatial", "spatial sidecar"),
            ("control", "control sidecar"),
            ("header", "header snapshot"),
            ("publication", "publication address"),
            ("frequency", "frequency sidecar"),
            ("routing", "routing sidecar"),
        ];
        for (missing, fragment) in cases {
            let err = observe(
                &Fake {
                    missing,
                    ..Fake::default()
                },
                1,
            )
            .err()
            .unwrap();
            assert!(err.contains(fragment), "{missing}: {err}");
            let c = unsafe { std::ffi::CStr::from_ptr(error_text(err)) };
            assert_eq!(c.to_str().unwrap(), err);
        }
    }
}
