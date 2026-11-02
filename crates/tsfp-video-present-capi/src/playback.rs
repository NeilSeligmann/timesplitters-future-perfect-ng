//! Serialized C boundary for the movie playback queue.
use std::slice;
use tsfp_video_present::Picture;
use tsfp_video_present::playback::{PlaybackQueue, Submit};

use crate::VideoPicture;

#[derive(Default)]
pub struct VideoPlayback {
    queue: PlaybackQueue,
    displayed: Option<Picture>,
}

/// Snapshot of the queue-owned playback timing counters.
#[repr(C)]
#[derive(Default)]
pub struct VideoPlaybackTiming {
    dropped: u64,
    late_shown: u64,
    queue_max: usize,
    queue_timeouts: u64,
    model_gap_max_ns: u64,
    model_gap_after: u64,
}

/// # Safety
/// The playback object must be live and serialized; out must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_video_playback_timing(
    playback: *const VideoPlayback,
    out: *mut VideoPlaybackTiming,
) -> bool {
    let (Some(playback), Some(out)) = (unsafe { playback.as_ref() }, unsafe { out.as_mut() })
    else {
        return false;
    };
    let timing = playback.queue.timing();
    *out = VideoPlaybackTiming {
        dropped: timing.dropped,
        late_shown: timing.late_shown,
        queue_max: timing.queue_max,
        queue_timeouts: timing.queue_timeouts,
        model_gap_max_ns: timing.model_gap_max_ns,
        model_gap_after: timing.model_gap_after,
    };
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_video_playback_new() -> *mut VideoPlayback {
    Box::into_raw(Box::default())
}

/// # Safety
/// The pointer must be null or an exclusively owned live playback object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_video_playback_free(playback: *mut VideoPlayback) {
    if !playback.is_null() {
        unsafe { drop(Box::from_raw(playback)) };
    }
}

/// # Safety
/// The playback object must be live and exclusively accessible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_video_playback_len(playback: *const VideoPlayback) -> usize {
    unsafe { playback.as_ref() }.map_or(0, |p| p.queue.len())
}

/// Returns 0 on failure, 1 on success, and 2 when full.
///
/// # Safety
/// The playback object must be live and exclusive; pixels must cover len bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_video_playback_submit(
    playback: *mut VideoPlayback,
    number: u64,
    width: u32,
    height: u32,
    pixels: *const u8,
    len: usize,
    vt_ns: u64,
) -> u32 {
    let Some(playback) = (unsafe { playback.as_mut() }) else {
        return 0;
    };
    if pixels.is_null() || len > isize::MAX as usize {
        return 0;
    }
    let pixels = unsafe { slice::from_raw_parts(pixels, len) };
    match playback.queue.submit(number, width, height, pixels, vt_ns) {
        Submit::Failed => 0,
        Submit::Queued => 1,
        Submit::Full => 2,
    }
}

/// # Safety
/// The playback object must be live and exclusively accessible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_video_playback_timeout(playback: *mut VideoPlayback) {
    if let Some(playback) = unsafe { playback.as_mut() } {
        playback.queue.drop_oldest_on_timeout();
    }
}

/// The returned pixels remain valid until the next successful tick or destruction.
///
/// # Safety
/// The playback object must be live and exclusive, and out must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_video_playback_tick(
    playback: *mut VideoPlayback,
    has_clock: bool,
    media_vt: u64,
    everything: bool,
    out: *mut VideoPicture,
) -> bool {
    let (Some(playback), Some(out)) = (unsafe { playback.as_mut() }, unsafe { out.as_mut() })
    else {
        return false;
    };
    let Some(picture) = playback
        .queue
        .tick(has_clock.then_some(media_vt), everything)
    else {
        return false;
    };
    playback.displayed = Some(picture);
    let picture = playback.displayed.as_ref().unwrap();
    *out = VideoPicture {
        number: picture.number,
        width: picture.width,
        height: picture.height,
        pixels: picture.rgb.as_ptr(),
        len: picture.rgb.len(),
    };
    true
}
