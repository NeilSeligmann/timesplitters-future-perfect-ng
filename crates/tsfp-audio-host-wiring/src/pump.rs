// SPDX-License-Identifier: GPL-3.0-or-later
//! T3917: the audio runtime lifecycle and PCM pump entry points of `src/host/tsfp_audio_host_pump.h`, in Rust. The host
//! passes its clock and PCM writer in a [`PumpSink`]; the render loop, the 5 ms refill worker and the work lock are the Rust
//! audio runtime's (`dsound_audio_runtime_*` exports of `tsfp-audio-capi`). What this module owns is the state the C host
//! kept in `main.c` statics: the guest-thread rendered-frame count and whether the refill worker was started.
use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

type ClockFn = Option<unsafe extern "C" fn(*mut c_void) -> u64>;
type WriteFn = Option<unsafe extern "C" fn(*mut c_void, *const i16, usize) -> bool>;

/// `tsfp_audio_host_pump_sink` of `src/host/tsfp_audio_host_pump.h`.
#[repr(C)]
pub struct PumpSink {
    clock: ClockFn,
    write: WriteFn,
    context: *mut c_void,
    register_direct: Option<unsafe extern "C" fn()>,
    direct_ring: bool,
}

unsafe extern "C" {
    static stdout: *mut c_void;
    fn fflush(stream: *mut c_void) -> i32;
    fn fprintf(stream: *mut c_void, format: *const c_char, ...) -> i32;
    fn dsound_audio_runtime_start(output_rate_hz: u32, ticks_per_second: u64) -> bool;
    fn dsound_audio_runtime_stop();
    fn dsound_audio_runtime_active() -> bool;
    fn dsound_audio_runtime_guest_work(
        clock: ClockFn,
        sink: WriteFn,
        context: *mut c_void,
        frames_written: *mut u64,
    ) -> bool;
    fn dsound_audio_runtime_start_pump(clock: ClockFn, sink: WriteFn, context: *mut c_void)
    -> bool;
    fn dsound_audio_runtime_start_pump_direct() -> bool;
    fn dsound_audio_runtime_stop_pump();
    fn dsound_audio_runtime_pump_stats(frames: *mut u64, renders: *mut u64, failures: *mut u64);
    fn dsound_audio_runtime_work_lock();
    fn dsound_audio_runtime_work_unlock();
    fn dsound_audio_runtime_set_direct_sink(
        sink: Option<unsafe extern "C" fn()>,
        context: *mut c_void,
    );
    fn dsound_completion_work();
    fn dsound_completion_capture_finish(quiescent: bool);
    fn dsound_movie_stream_route_work(return_address: u32) -> bool;
}

/// Frames the guest-thread route rendered into the sink (`g_audio_rendered_frames` of the C host).
static RENDERED_FRAMES: AtomicU64 = AtomicU64::new(0);
static PUMP_STARTED: AtomicBool = AtomicBool::new(false);

/// T3783 shutdown triage: explicit, flushed markers in both captured streams.
fn shutdown_trace(stage: &std::ffi::CStr) {
    unsafe {
        crate::ffi::printf(c"audio shutdown trace: %s\n".as_ptr(), stage.as_ptr());
        fflush(stdout);
        fprintf(
            crate::ffi::stderr,
            c"audio shutdown trace: %s\n".as_ptr(),
            stage.as_ptr(),
        );
        fflush(crate::ffi::stderr);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_runtime_start(
    output_rate_hz: u32,
    ticks_per_second: u64,
) -> bool {
    unsafe { dsound_audio_runtime_start(output_rate_hz, ticks_per_second) }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_runtime_stop() {
    unsafe { dsound_audio_runtime_stop() }
}

/// # Safety
/// `sink` must point to a valid `tsfp_audio_host_pump_sink`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_host_work_route(
    return_address: u32,
    sink: *const PumpSink,
) -> bool {
    let sink = unsafe { &*sink };
    unsafe {
        dsound_completion_work();
        if dsound_audio_runtime_active() {
            if let Some(register) = sink.register_direct {
                register();
            }
            let mut written = 0u64;
            if !dsound_audio_runtime_guest_work(sink.clock, sink.write, sink.context, &mut written)
            {
                fprintf(
                    crate::ffi::stderr,
                    c"audio mixer failed while pumping virtual-clock PCM\n".as_ptr(),
                );
                return false;
            }
            RENDERED_FRAMES.fetch_add(written, Ordering::Relaxed);
        }
        dsound_movie_stream_route_work(return_address)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_rendered_frames() -> u64 {
    RENDERED_FRAMES.load(Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_work_lock() {
    unsafe { dsound_audio_runtime_work_lock() }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_work_unlock() {
    unsafe { dsound_audio_runtime_work_unlock() }
}

/// # Safety
/// `sink` must point to a valid `tsfp_audio_host_pump_sink` whose callbacks and context outlive the worker.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_host_pump_start(sink: *const PumpSink) -> bool {
    let sink = unsafe { &*sink };
    let started = unsafe {
        if let Some(register) = sink.register_direct {
            register();
        }
        if sink.direct_ring {
            dsound_audio_runtime_start_pump_direct()
        } else {
            dsound_audio_runtime_start_pump(sink.clock, sink.write, sink.context)
        }
    };
    PUMP_STARTED.store(started, Ordering::Relaxed);
    started
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_pump_stop() {
    if !PUMP_STARTED.swap(false, Ordering::Relaxed) {
        return;
    }
    shutdown_trace(c"before refill worker join");
    unsafe { dsound_audio_runtime_stop_pump() };
    shutdown_trace(c"after refill worker join");
    let mut failures = 0u64;
    unsafe {
        dsound_audio_runtime_pump_stats(std::ptr::null_mut(), std::ptr::null_mut(), &mut failures)
    };
    if failures != 0 {
        unsafe {
            fprintf(
                crate::ffi::stderr,
                c"audio mixer failed while pumping virtual-clock PCM (%llu failures)\n".as_ptr(),
                failures,
            );
        }
    }
}

/// # Safety
/// Each pointer is either null or valid for a `u64` write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_host_pump_stats(
    frames: *mut u64,
    renders: *mut u64,
    failures: *mut u64,
) {
    unsafe { dsound_audio_runtime_pump_stats(frames, renders, failures) }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_capture_finish(quiescent: bool) {
    unsafe { dsound_completion_capture_finish(quiescent) }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_set_direct_sink(
    sink: Option<unsafe extern "C" fn()>,
    context: *mut c_void,
) {
    unsafe { dsound_audio_runtime_set_direct_sink(sink, context) }
}
