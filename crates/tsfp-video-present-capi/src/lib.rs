//! C boundary for the opt-in video latch. The caller serializes all access.

use std::slice;
use tsfp_video_present::{Latch, Picture};

mod playback;

#[derive(Default)]
pub struct VideoLatch {
    latch: Latch,
    displayed: Option<Picture>,
}

/// A borrowed picture. Its pixels remain valid until the next successful vblank or destruction.
#[repr(C)]
pub struct VideoPicture {
    pub number: u64,
    pub width: u32,
    pub height: u32,
    pub pixels: *const u8,
    pub len: usize,
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_video_latch_new() -> *mut VideoLatch {
    Box::into_raw(Box::default())
}

/// # Safety
/// `latch` must be null or a live pointer returned by `tsfp_video_latch_new`, used once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_video_latch_free(latch: *mut VideoLatch) {
    if !latch.is_null() {
        unsafe { drop(Box::from_raw(latch)) };
    }
}

/// # Safety
/// `latch` must be live and exclusively accessible. If non-null, `pixels` must point to
/// `len` readable bytes for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_video_latch_submit(
    latch: *mut VideoLatch,
    number: u64,
    width: u32,
    height: u32,
    pixels: *const u8,
    len: usize,
) -> bool {
    let Some(latch) = (unsafe { latch.as_mut() }) else {
        return false;
    };
    if len > isize::MAX as usize {
        return false;
    }
    let pixels = if pixels.is_null() {
        None
    } else {
        Some(unsafe { slice::from_raw_parts(pixels, len) })
    };
    let failed = latch.latch.counts().failed;
    latch.latch.submit(number, width, height, pixels);
    latch.latch.counts().failed == failed
}

/// Borrows the pending picture until the next successful submission, vblank, or destruction.
///
/// # Safety
/// `latch` must be live and exclusively accessible; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_video_latch_pending(
    latch: *const VideoLatch,
    out: *mut VideoPicture,
) -> bool {
    let (Some(latch), Some(out)) = (unsafe { latch.as_ref() }, unsafe { out.as_mut() }) else {
        return false;
    };
    let Some(picture) = latch.latch.pending() else {
        return false;
    };
    *out = VideoPicture {
        number: picture.number,
        width: picture.width,
        height: picture.height,
        pixels: picture.rgb.as_ptr(),
        len: picture.rgb.len(),
    };
    true
}

/// Consumes a pending picture and makes it the retained displayed picture.
///
/// # Safety
/// `latch` must be live and exclusively accessible. `out` must be writable when non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_video_latch_vblank(
    latch: *mut VideoLatch,
    out: *mut VideoPicture,
) -> bool {
    let (Some(latch), Some(out)) = (unsafe { latch.as_mut() }, unsafe { out.as_mut() }) else {
        return false;
    };
    let Some(picture) = latch.latch.vblank() else {
        return false;
    };
    latch.displayed = Some(picture);
    let picture = latch.displayed.as_ref().unwrap();
    *out = VideoPicture {
        number: picture.number,
        width: picture.width,
        height: picture.height,
        pixels: picture.rgb.as_ptr(),
        len: picture.rgb.len(),
    };
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picture_survives_empty_vblank_and_failed_submission() {
        unsafe {
            let latch = tsfp_video_latch_new();
            let pixels = [1, 2, 3];
            assert!(tsfp_video_latch_submit(latch, 7, 1, 1, pixels.as_ptr(), 3));
            let mut out = VideoPicture {
                number: 0,
                width: 0,
                height: 0,
                pixels: std::ptr::null(),
                len: 0,
            };
            assert!(tsfp_video_latch_vblank(latch, &mut out));
            assert_eq!((out.number, out.width, out.height, out.len), (7, 1, 1, 3));
            assert!(!tsfp_video_latch_submit(latch, 8, 1, 1, pixels.as_ptr(), 2));
            assert!(!tsfp_video_latch_vblank(latch, &mut out));
            assert_eq!(slice::from_raw_parts(out.pixels, out.len), pixels);
            tsfp_video_latch_free(latch);
        }
    }
}
