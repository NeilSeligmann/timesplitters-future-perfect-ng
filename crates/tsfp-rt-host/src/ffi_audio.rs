// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI of the audio thread (slice 2, no header file yet, like the render ABI). A separate handle from the render runtime: it needs no
//! GPU adapter. Every entry catches panics (unwinding across the host's C frames is undefined behaviour).
#![allow(unsafe_code)]

use std::ffi::{CStr, c_char, c_int};
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::audio::{AudioRuntime, AudioStats, parse_audio_args};

/// Mirror of `tsfp_rt_audio_stats` in the C header (u64 counters, then u32 flags).
#[repr(C)]
#[derive(Default)]
pub struct CAudioStats {
    pub batches_sent: u64,
    pub batches_dropped: u64,
    pub stalls: u64,
    pub stall_ns_total: u64,
    pub stall_ns_max: u64,
    pub lines: u64,
    pub commands: u64,
    pub verdict_mismatch: u64,
    pub renders: u64,
    pub render_equal: u64,
    pub render_diff: u64,
    pub frames: u64,
    pub pcm_hash: u64,
    pub ring_overruns: u64,
    pub ring_underruns: u64,
    pub gave_up: u32,
    pub sink_error: u32,
}

fn to_c(s: AudioStats) -> CAudioStats {
    CAudioStats {
        batches_sent: s.batches_sent,
        batches_dropped: s.batches_dropped,
        stalls: s.stalls,
        stall_ns_total: s.stall_ns_total,
        stall_ns_max: s.stall_ns_max,
        lines: s.lines,
        commands: s.commands,
        verdict_mismatch: s.verdict_mismatch,
        renders: s.renders,
        render_equal: s.render_equal,
        render_diff: s.render_diff,
        frames: s.frames,
        pcm_hash: s.pcm_hash,
        ring_overruns: s.ring_overruns,
        ring_underruns: s.ring_underruns,
        gave_up: s.gave_up as u32,
        sink_error: s.sink_error as u32,
    }
}

/// Start the audio thread. `args`: whitespace separated `--sink null|wav=PATH|ring=SAMPLES --queue N --batch-bytes N --stall-ms N
/// --stall-total-ms N`; may be null. Returns null on bad arguments (message on stderr).
///
/// # Safety
/// `args` is null or a NUL terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_audio_create(args: *const c_char) -> *mut AudioRuntime {
    let text = if args.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(args) }
            .to_string_lossy()
            .into_owned()
    };
    let words: Vec<String> = text.split_whitespace().map(str::to_string).collect();
    match catch_unwind(AssertUnwindSafe(|| {
        parse_audio_args(&words).and_then(AudioRuntime::start)
    })) {
        Ok(Ok(runtime)) => Box::into_raw(Box::new(runtime)),
        Ok(Err(error)) => {
            eprintln!("rt-host: audio: {error}");
            std::ptr::null_mut()
        }
        Err(_) => {
            eprintln!("rt-host: panic in audio create");
            std::ptr::null_mut()
        }
    }
}

/// Append command log bytes (any chunking, lines are re-assembled). Never blocks longer than the stall limit. Returns `len`.
///
/// # Safety
/// `runtime` came from `tsfp_rt_audio_create` and is not destroyed; `bytes` points at `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_audio_write(
    runtime: *mut AudioRuntime,
    bytes: *const u8,
    len: usize,
) -> isize {
    if runtime.is_null() || bytes.is_null() {
        return -1;
    }
    let (runtime, slice) = unsafe { (&*runtime, std::slice::from_raw_parts(bytes, len)) };
    match catch_unwind(AssertUnwindSafe(|| runtime.write(slice))) {
        Ok(()) => len as isize,
        Err(_) => -1,
    }
}

/// Append one command line (newline added).
///
/// # Safety
/// As `tsfp_rt_audio_write`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_audio_send_line(
    runtime: *mut AudioRuntime,
    text: *const u8,
    len: usize,
) -> c_int {
    if runtime.is_null() || text.is_null() {
        return -1;
    }
    let (runtime, slice) = unsafe { (&*runtime, std::slice::from_raw_parts(text, len)) };
    let line = String::from_utf8_lossy(slice).into_owned();
    catch_unwind(AssertUnwindSafe(|| runtime.send_line(&line))).map_or(-1, |()| 0)
}

/// Hand every complete queued line to the audio thread.
///
/// # Safety
/// `runtime` came from `tsfp_rt_audio_create` and is not destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_audio_flush(runtime: *mut AudioRuntime) -> c_int {
    if runtime.is_null() {
        return -1;
    }
    let runtime = unsafe { &*runtime };
    catch_unwind(AssertUnwindSafe(|| runtime.flush())).map_or(-1, |()| 0)
}

/// Device callback side of a `ring=` sink: fill `out[0..samples]` (interleaved stereo i16), silence on underrun. Returns the
/// real samples copied.
///
/// # Safety
/// `runtime` as above, `out` points at `samples` writable i16.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_audio_pull(
    runtime: *mut AudioRuntime,
    out: *mut i16,
    samples: usize,
) -> isize {
    if runtime.is_null() || out.is_null() {
        return -1;
    }
    let (runtime, slice) = unsafe { (&*runtime, std::slice::from_raw_parts_mut(out, samples)) };
    catch_unwind(AssertUnwindSafe(|| runtime.pull(slice))).map_or(-1, |n| n as isize)
}

/// Copy the counters out.
///
/// # Safety
/// `runtime` as above, `out` points at one `CAudioStats`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_audio_stats(
    runtime: *mut AudioRuntime,
    out: *mut CAudioStats,
) -> c_int {
    if runtime.is_null() || out.is_null() {
        return -1;
    }
    let runtime = unsafe { &*runtime };
    match catch_unwind(AssertUnwindSafe(|| runtime.stats())) {
        Ok(stats) => {
            unsafe { out.write(to_c(stats)) };
            0
        }
        Err(_) => -1,
    }
}

/// Flush, wait for the audio thread (WAV header patched), print one summary line to stderr, free the runtime.
///
/// # Safety
/// `runtime` came from `tsfp_rt_audio_create`; it must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_audio_destroy(runtime: *mut AudioRuntime) {
    if runtime.is_null() {
        return;
    }
    let runtime = unsafe { Box::from_raw(runtime) };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let s = runtime.finish();
        eprintln!(
            "rt-host: audio done: {} lines, {} commands ({} verdict mismatches), {} renders ({} equal, {} differ), {} frames, hash {:016x}, {} batches sent, {} dropped, {} stalls (max {} us, total {} us){}",
            s.lines,
            s.commands,
            s.verdict_mismatch,
            s.renders,
            s.render_equal,
            s.render_diff,
            s.frames,
            s.pcm_hash,
            s.batches_sent,
            s.batches_dropped,
            s.stalls,
            s.stall_ns_max / 1000,
            s.stall_ns_total / 1000,
            if s.gave_up { ", GAVE UP" } else { "" }
        );
    }));
}
