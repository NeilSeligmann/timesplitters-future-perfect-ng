//! C ABI over the Rust GP effects DSP (`tsfp-audio-gpdsp`), mirroring `dsound_effects_dsp.h`.
//!
//! The C host selects these entry points with `TSFP_GPDSP_RUST`; the C core stays as the fallback.
//! Errors are sticky like the C core: a failed run poisons the context until it is destroyed.

use std::ffi::c_char;
use tsfp_audio_gpdsp::gp::{FRAME_WORDS, GpDsp};
use tsfp_audio_gpdsp::prepare;

pub struct Context {
    gp: GpDsp,
}

const ABSENT: &[u8] = b"DSP context absent\0";

fn slice<'a>(pointer: *const u8, len: usize) -> Option<&'a [u8]> {
    if len == 0 {
        return Some(&[]);
    }
    // SAFETY: the caller passes a readable span of `len` bytes, as in the C header.
    (!pointer.is_null()).then(|| unsafe { std::slice::from_raw_parts(pointer, len) })
}

fn create(
    firmware: *const u8,
    firmware_bytes: usize,
    image: *const u8,
    image_bytes: usize,
    finish_frame: bool,
) -> *mut Context {
    let (Some(firmware), Some(image)) = (
        slice(firmware, firmware_bytes).filter(|s| !s.is_empty()),
        slice(image, image_bytes).filter(|s| !s.is_empty()),
    ) else {
        return std::ptr::null_mut();
    };
    let Some(prepared) = prepare(firmware, image) else {
        return std::ptr::null_mut();
    };
    match GpDsp::new(prepared, finish_frame) {
        Ok(gp) => Box::into_raw(Box::new(Context { gp })),
        Err(_) => std::ptr::null_mut(),
    }
}

/// # Safety
/// Spans must be readable for their lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_gpdsp_create(
    firmware: *const u8,
    firmware_bytes: usize,
    image: *const u8,
    image_bytes: usize,
) -> *mut Context {
    create(firmware, firmware_bytes, image, image_bytes, true)
}

/// # Safety
/// Spans must be readable for their lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_gpdsp_create_at_download(
    firmware: *const u8,
    firmware_bytes: usize,
    image: *const u8,
    image_bytes: usize,
) -> *mut Context {
    create(firmware, firmware_bytes, image, image_bytes, false)
}

/// # Safety
/// `context` is null or came from a create call and is not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_gpdsp_destroy(context: *mut Context) {
    if !context.is_null() {
        // SAFETY: ownership returns from the C caller.
        drop(unsafe { Box::from_raw(context) });
    }
}

fn live<'a>(context: *mut Context) -> Option<&'a mut Context> {
    // SAFETY: the C caller serialises access and passes a live context or null.
    unsafe { context.as_mut() }.filter(|c| c.gp.error().is_none())
}

/// # Safety
/// `source` is readable for `bytes` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_gpdsp_stage(
    context: *mut Context,
    map: u32,
    offset: usize,
    source: *const u8,
    bytes: usize,
) -> bool {
    let (Some(c), Some(data)) = (live(context), slice(source, bytes)) else {
        return false;
    };
    c.gp.stage(map as usize, offset, data).is_ok()
}

/// # Safety
/// `source` is readable for `bytes` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_gpdsp_live_aligned(
    context: *mut Context,
    map: u32,
    offset: usize,
    source: *const u8,
    bytes: usize,
) -> bool {
    let (Some(c), Some(data)) = (live(context), slice(source, bytes)) else {
        return false;
    };
    c.gp.live_aligned(map as usize, offset, data).is_ok()
}

/// # Safety
/// `source` is readable for `bytes` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_gpdsp_write(
    context: *mut Context,
    map: u32,
    offset: usize,
    source: *const u8,
    bytes: usize,
    deferred: bool,
) -> bool {
    let (Some(c), Some(data)) = (live(context), slice(source, bytes)) else {
        return false;
    };
    c.gp.write(map as usize, offset, data, deferred).is_ok()
}

/// # Safety
/// `context` is live or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_gpdsp_defer(
    context: *mut Context,
    map: u32,
    offset: usize,
    bytes: usize,
) -> bool {
    live(context).is_some_and(|c| c.gp.defer(map as usize, offset, bytes).is_ok())
}

/// # Safety
/// `start` and `bytes` are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_gpdsp_pending(
    context: *const Context,
    start: *mut u32,
    bytes: *mut u32,
) -> bool {
    // SAFETY: live context or null.
    let Some(c) = (unsafe { context.as_ref() }).filter(|c| c.gp.error().is_none()) else {
        return false;
    };
    if start.is_null() || bytes.is_null() {
        return false;
    }
    let (s, b) = c.gp.pending();
    // SAFETY: both outputs are non-null and writable.
    unsafe {
        *start = s;
        *bytes = b;
    }
    true
}

/// # Safety
/// `context` is live or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_gpdsp_commit(context: *mut Context) -> bool {
    live(context).is_some_and(|c| c.gp.commit().is_ok())
}

/// # Safety
/// `input` and `output` are 1024 words.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_gpdsp_frame(
    context: *mut Context,
    input: *const u32,
    output: *mut u32,
) -> bool {
    let Some(c) = live(context) else { return false };
    if input.is_null() || output.is_null() {
        return false;
    }
    // SAFETY: the C contract is two 1024-word buffers.
    let input = unsafe { std::slice::from_raw_parts(input, FRAME_WORDS) };
    let Ok(input) = <&[u32; FRAME_WORDS]>::try_from(input) else {
        return false;
    };
    match c.gp.frame(input) {
        Ok(words) => {
            // SAFETY: as above.
            unsafe { std::slice::from_raw_parts_mut(output, FRAME_WORDS) }.copy_from_slice(&words);
            true
        }
        Err(_) => false,
    }
}

/// # Safety
/// `output` is writable for `bytes` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_gpdsp_read(
    context: *mut Context,
    map: u32,
    offset: usize,
    output: *mut u8,
    bytes: usize,
) -> bool {
    let Some(c) = live(context) else { return false };
    if bytes == 0 {
        return c.gp.read(map as usize, offset, &mut []).is_ok();
    }
    if output.is_null() {
        return false;
    }
    // SAFETY: writable span per the C contract.
    let out = unsafe { std::slice::from_raw_parts_mut(output, bytes) };
    c.gp.read(map as usize, offset, out).is_ok()
}

/// # Safety
/// `context` is live or null; the returned string is static.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_gpdsp_error(context: *const Context) -> *const c_char {
    // SAFETY: live context or null.
    match unsafe { context.as_ref() } {
        None => ABSENT.as_ptr().cast(),
        Some(c) => c.gp.error().map_or(std::ptr::null(), |m| m.as_ptr().cast()),
    }
}
