//! Projection of accepted stream sidecars into the passive host snapshot.

mod frequency;
mod observe;
#[cfg(not(test))]
pub use frequency::tsfp_audio_stream_set_frequency_rust;
#[cfg(not(test))]
pub use observe::tsfp_audio_stream_observe_rust;
pub use observe::{FrequencyState, Sources, observe};

use tsfp_audio::stream_lease::{
    StreamControlState, StreamFormatState, StreamLeaseScope, StreamSpatialState, StreamVolumeState,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Lease {
    device_heap: u32,
    internal_address: u32,
    serial: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Routing {
    volume: u32,
    headroom: u32,
    aggregate: u32,
    count: u32,
    bins: [u8; 8],
    bin_volume: [u32; 32],
}

// Matches dsound_stream_snapshot, including C bool fields and their padding.
#[repr(C)]
pub struct Snapshot {
    scope: StreamLeaseScope,
    lease: Lease,
    stream_address: u32,
    stream_heap: u32,
    header: [u32; 10],
    publication_address: u32,
    i3dl2_address: u32,
    cache_mask: u32,
    i3dl2: [u32; 9],
    min_distance_bits: u32,
    max_distance_seen: bool,
    max_distance_bits: u32,
    curve_address: u32,
    curve_count: u32,
    pause_seen: bool,
    pause_mode: u32,
    flush_seen: bool,
    flush_time_low: u32,
    flush_time_high: u32,
    flush_flags: u32,
    discontinuity_seen: bool,
    volume_seen: bool,
    volume: i32,
    format_sets: u32,
    source_rate_hz: u32,
    frequency_sets: u32,
    frequency_pitch: i32,
    format: [u8; 20],
    routing: Routing,
    position_bits: [u32; 3],
    position_sets: u32,
}

// Paired with the C adapter assertions; fail compilation if the boundary drifts.
const _: () = {
    assert!(std::mem::size_of::<Snapshot>() == 440);
    assert!(std::mem::offset_of!(Snapshot, cache_mask) == 144);
    assert!(std::mem::offset_of!(Snapshot, pause_seen) == 204);
    assert!(std::mem::offset_of!(Snapshot, format) == 252);
    assert!(std::mem::offset_of!(Snapshot, routing) == 272);
    assert!(std::mem::offset_of!(Snapshot, position_bits) == 424);
};

/// Projects already validated sidecars. The caller retains the stream lock.
///
/// # Safety
/// All pointers must be valid, aligned, and non-overlapping for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_snapshot_project_rust(
    out: *mut Snapshot,
    format: *const StreamFormatState,
    volume: *const StreamVolumeState,
    spatial: *const StreamSpatialState,
    control: *const StreamControlState,
) {
    let (out, format, volume, spatial, control) =
        unsafe { (&mut *out, &*format, &*volume, &*spatial, &*control) };
    out.format_sets = format.sets;
    out.format = format.bytes;
    out.volume_seen = volume.seen != 0;
    out.volume = volume.value;
    out.cache_mask = spatial.startup_cache_mask;
    out.i3dl2_address = spatial.i3dl2_address;
    out.i3dl2 = spatial.i3dl2_words;
    out.min_distance_bits = spatial.min_distance_bits;
    out.position_bits = spatial.position_bits;
    out.position_sets = spatial.position_sets;
    out.max_distance_seen = spatial.max_distance_seen != 0;
    out.max_distance_bits = spatial.max_distance_bits;
    out.curve_address = spatial.curve_address;
    out.curve_count = spatial.curve_count;
    out.pause_seen = control.pause_seen != 0;
    out.pause_mode = control.pause_mode;
    out.flush_seen = control.flush_seen != 0;
    out.flush_time_low = control.flush_time_low;
    out.flush_time_high = control.flush_time_high;
    out.flush_flags = control.flush_flags;
    out.discontinuity_seen = control.discontinuity_seen != 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_sidecars_and_preserves_other_fields() {
        // Zero is a valid bit pattern for every field of this C-compatible record.
        let mut out: Snapshot = unsafe { std::mem::zeroed() };
        out.stream_address = 0x1234;
        out.source_rate_hz = 44100;
        let format = StreamFormatState {
            bytes: [0xa5; 20],
            sets: 3,
        };
        let volume = StreamVolumeState {
            seen: 2,
            value: -10000,
        };
        let spatial = StreamSpatialState {
            startup_cache_mask: 7,
            i3dl2_address: 9,
            i3dl2_words: [11; 9],
            min_distance_bits: 12,
            position_bits: [13, 14, 15],
            position_sets: 16,
            max_distance_seen: 2,
            max_distance_bits: 17,
            curve_address: 18,
            curve_count: 19,
            revision: 20,
        };
        let control = StreamControlState {
            pause_seen: 2,
            pause_mode: 21,
            flush_seen: 3,
            flush_time_low: 22,
            flush_time_high: 23,
            flush_flags: 24,
            discontinuity_seen: 4,
            revision: 25,
        };
        unsafe {
            tsfp_audio_stream_snapshot_project_rust(&mut out, &format, &volume, &spatial, &control)
        };
        assert_eq!(
            (out.format, out.format_sets, out.volume_seen, out.volume),
            ([0xa5; 20], 3, true, -10000)
        );
        assert_eq!(
            (
                out.cache_mask,
                out.i3dl2_address,
                out.i3dl2,
                out.min_distance_bits
            ),
            (7, 9, [11; 9], 12)
        );
        assert_eq!(
            (
                out.position_bits,
                out.position_sets,
                out.max_distance_seen,
                out.max_distance_bits,
                out.curve_address,
                out.curve_count
            ),
            ([13, 14, 15], 16, true, 17, 18, 19)
        );
        assert_eq!(
            (
                out.pause_seen,
                out.pause_mode,
                out.flush_seen,
                out.flush_time_low,
                out.flush_time_high,
                out.flush_flags,
                out.discontinuity_seen
            ),
            (true, 21, true, 22, 23, 24, true)
        );
        assert_eq!((out.stream_address, out.source_rate_hz), (0x1234, 44100));
    }
}
