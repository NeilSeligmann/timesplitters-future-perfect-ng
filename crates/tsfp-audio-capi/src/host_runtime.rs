// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI for the live Rust audio host. The guest DirectSound adapter keeps its measured ABI and
//! forwards accepted host commands here; decoding, mixer state, and PCM production stay in Rust.

use std::ffi::c_char;
use std::ffi::c_void;
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use tsfp_audio::adpcm;
use tsfp_audio::contract::{Command, Routing, SoundHost};
use tsfp_audio::mixer::Mixer;

use crate::effects_guest_copy_ffi::tsfp_effects_guest_copy_read_rust;
use crate::routing_ffi::CRouting;

const MAX_PACKET_BYTES: usize = 4 * 1024 * 1024;
const MAX_MOVIE_PACKET_BYTES: usize = 16 * 1024 * 1024;
const MAX_BUFFER_BYTES: usize = 36 * 32768;

fn guest_audio_bytes(pid: i32, address: u32, length: usize, block_align: usize) -> Option<Vec<u8>> {
    if length == 0 || length > MAX_PACKET_BYTES || block_align == 0 || length % block_align != 0 {
        return None;
    }
    let mut bytes = vec![0; length];
    // SAFETY: the helper checks both spans and uses fault-safe process_vm_readv for guest memory.
    unsafe { tsfp_effects_guest_copy_read_rust(pid, address, bytes.as_mut_ptr(), bytes.len()) }
        .then_some(bytes)
}

fn guest_movie_pcm16_stereo(pid: i32, address: u32, bytes: usize) -> Option<Vec<i16>> {
    if pid <= 0 || address == 0 || bytes == 0 || bytes > MAX_MOVIE_PACKET_BYTES || bytes % 4 != 0 {
        return None;
    }
    let mut encoded = vec![0; bytes];
    // SAFETY: the helper checks both spans and uses fault-safe process_vm_readv for guest memory.
    if !unsafe {
        tsfp_effects_guest_copy_read_rust(pid, address, encoded.as_mut_ptr(), encoded.len())
    } {
        return None;
    }
    Some(
        encoded
            .chunks_exact(2)
            .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
            .collect(),
    )
}

fn guest_buffer_bytes(pid: i32, address: u32, length: usize) -> Option<Vec<u8>> {
    if length > MAX_BUFFER_BYTES {
        return None;
    }
    guest_audio_bytes(pid, address, length, 36)
}

#[cfg(all(not(test), feature = "rust-kernel-clock"))]
unsafe extern "C" {
    #[link_name = "kernel_clock_peek"]
    fn kernel_clock_peek_c_ffi() -> u64;
    #[link_name = "tsfp_kernel_clock_peek"]
    fn kernel_clock_peek_rust_ffi() -> u64;
}

#[cfg(all(not(test), not(feature = "rust-kernel-clock")))]
unsafe extern "C" {
    #[link_name = "kernel_clock_peek"]
    fn kernel_clock_peek_c_ffi() -> u64;
}

#[cfg(all(not(test), feature = "rust-kernel-clock"))]
static RUST_KERNEL_CLOCK_SELECTED: OnceLock<bool> = OnceLock::new();

#[cfg(all(not(test), feature = "rust-kernel-clock"))]
fn rust_kernel_clock_selected() -> bool {
    *RUST_KERNEL_CLOCK_SELECTED
        .get_or_init(|| std::env::var_os("TSFP_RUST_KCLOCK").is_none_or(|value| value != "0"))
}

#[cfg(test)]
static TEST_KERNEL_CLOCK_TICKS: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static TEST_KERNEL_CLOCK_SAMPLES: AtomicU64 = AtomicU64::new(0);

fn sample_kernel_clock_ticks() -> u64 {
    #[cfg(test)]
    {
        TEST_KERNEL_CLOCK_SAMPLES.fetch_add(1, Ordering::Relaxed);
        TEST_KERNEL_CLOCK_TICKS.load(Ordering::Relaxed)
    }
    #[cfg(not(test))]
    {
        #[cfg(feature = "rust-kernel-clock")]
        if rust_kernel_clock_selected() {
            // SAFETY: the Rust kernel clock owns this mutex-protected, non-advancing snapshot.
            return unsafe { kernel_clock_peek_rust_ffi() };
        }

        // SAFETY: this is the compatibility path when the C kernel-clock backend is selected.
        unsafe { kernel_clock_peek_c_ffi() }
    }
}

fn selected_rust_kernel_clock_ticks() -> Option<u64> {
    #[cfg(all(not(test), feature = "rust-kernel-clock"))]
    if rust_kernel_clock_selected() {
        // SAFETY: the Rust kernel clock owns this mutex-protected, non-advancing snapshot.
        return Some(unsafe { kernel_clock_peek_rust_ffi() });
    }
    None
}

fn prefer_selected_rust_clock(rust_ticks: Option<u64>, fallback: impl FnOnce() -> u64) -> u64 {
    rust_ticks.unwrap_or_else(fallback)
}

pub(crate) fn movie_schedule_now_ticks(fallback: impl FnOnce() -> u64) -> u64 {
    prefer_selected_rust_clock(selected_rust_kernel_clock_ticks(), fallback)
}

#[derive(Default)]
struct Runtime {
    mixer: Option<Mixer>,
    capture: Option<crate::audio_capture::LiveCapture>,
    clock_frequency: u64,
    completion_route_enabled: bool,
    completion_clock_frequency: u64,
}

static RUNTIME: Mutex<Runtime> = Mutex::new(Runtime {
    mixer: None,
    capture: None,
    clock_frequency: 0,
    completion_route_enabled: false,
    completion_clock_frequency: 0,
});
static BEVY_AUDIO_RING_SELECTED: OnceLock<bool> = OnceLock::new();

/// Optional native Rust device writer installed by the Bevy host. A zero result delegates to
/// the host's legacy sink callback; positive means handled and negative is a sink failure.
pub type DirectSink = unsafe extern "C" fn(*mut c_void, *const i16, usize, u64, u64) -> i32;
#[derive(Clone, Copy)]
struct DirectSinkBinding {
    callback: DirectSink,
    context: usize,
}
#[derive(Default)]
struct DirectSinkState {
    binding: Option<DirectSinkBinding>,
    active_pumps: usize,
}
static DIRECT_SINK: Mutex<DirectSinkState> = Mutex::new(DirectSinkState {
    binding: None,
    active_pumps: 0,
});

fn direct_sink() -> Option<DirectSinkBinding> {
    DIRECT_SINK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .binding
}

struct DirectSinkPump;

impl DirectSinkPump {
    fn begin() -> Self {
        let mut state = DIRECT_SINK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.active_pumps = state.active_pumps.saturating_add(1);
        Self
    }
}

impl Drop for DirectSinkPump {
    fn drop(&mut self) {
        let mut state = DIRECT_SINK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.active_pumps = state.active_pumps.saturating_sub(1);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_set_direct_sink(
    sink: Option<DirectSink>,
    context: *mut c_void,
) {
    let _ = dsound_audio_runtime_try_set_direct_sink(sink, context);
}

/// Change the optional Bevy writer only while no pump may call its borrowed context.
/// Returns false and preserves the existing binding while a pump is active.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_try_set_direct_sink(
    sink: Option<DirectSink>,
    context: *mut c_void,
) -> bool {
    let mut state = DIRECT_SINK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if state.active_pumps != 0 {
        return false;
    }
    state.binding = sink.map(|callback| DirectSinkBinding {
        callback,
        context: context as usize,
    });
    true
}

fn runtime() -> std::sync::MutexGuard<'static, Runtime> {
    RUNTIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

unsafe fn bytes<'a>(ptr: *const u8, len: usize, limit: usize) -> Option<&'a [u8]> {
    if ptr.is_null() || len == 0 || len > limit {
        return None;
    }
    // SAFETY: callers of this helper must pass a readable allocation of `len` bytes.
    Some(unsafe { std::slice::from_raw_parts(ptr, len) })
}

unsafe fn samples<'a>(ptr: *const i16, frames: usize) -> Option<&'a [i16]> {
    let count = frames.checked_mul(2)?;
    if ptr.is_null() || count == 0 || count > MAX_PACKET_BYTES / 2 {
        return None;
    }
    // SAFETY: callers of this helper must pass `frames * 2` readable samples.
    Some(unsafe { std::slice::from_raw_parts(ptr, count) })
}

fn apply(command: Command) -> bool {
    apply_with_buffer(command, None)
}

fn apply_with_buffer(command: Command, encoded_buffer: Option<&[u8]>) -> bool {
    let mut state = runtime();
    let accepted = state
        .mixer
        .as_mut()
        .is_some_and(|mixer| mixer.apply(&command));
    if let Some(capture) = state.capture.as_mut() {
        capture.command(&command, accepted, encoded_buffer);
    }
    accepted
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_start(output_rate_hz: u32, ticks_per_second: u64) -> bool {
    if output_rate_hz == 0 || ticks_per_second == 0 {
        return false;
    }
    let mut state = runtime();
    if state.mixer.is_some() {
        return false;
    }
    state.mixer = Some(Mixer::new(output_rate_hz, ticks_per_second));
    state.capture =
        crate::audio_capture::LiveCapture::from_environment(output_rate_hz, ticks_per_second);
    state.clock_frequency = ticks_per_second;
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_stop() {
    let mut state = runtime();
    state.mixer = None;
    state.clock_frequency = 0;
    if let Some(capture) = state.capture.take() {
        capture.finish();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_reset_buffers() {
    let mut state = runtime();
    if let Some(mixer) = state.mixer.as_mut() {
        let command = Command::ResetBuffers;
        let accepted = mixer.apply(&command);
        if let Some(capture) = state.capture.as_mut() {
            capture.command(&command, accepted, None);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_active() -> bool {
    runtime().mixer.is_some()
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_clock_frequency() -> u64 {
    let state = runtime();
    if state.mixer.is_some() {
        state.clock_frequency
    } else {
        0
    }
}

/// Keep the passive stream-completion route gate in the Rust host. The C adapter calls this only
/// when its completion policy is configured; the runtime callback below then stays entirely on
/// the Rust side of the audio loop.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_set_completion_route_policy_rust(
    enabled: bool,
    completion_clock_frequency: u64,
) {
    let mut state = runtime();
    state.completion_route_enabled = enabled;
    state.completion_clock_frequency = completion_clock_frequency;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_submit(
    key: u32,
    ticks: u64,
    source_hz: u32,
    encoded: *const u8,
    encoded_bytes: usize,
) -> bool {
    // SAFETY: the C host owns the guest packet buffer for the duration of this synchronous call.
    let Some(data) = (unsafe { bytes(encoded, encoded_bytes, MAX_PACKET_BYTES) }) else {
        return false;
    };
    apply(Command::SubmitAdpcm {
        key,
        ticks,
        source_hz,
        mono: false,
        data: data.to_vec(),
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_submit_mono(
    key: u32,
    ticks: u64,
    source_hz: u32,
    encoded: *const u8,
    encoded_bytes: usize,
) -> bool {
    // SAFETY: the C host owns the guest packet buffer for the duration of this synchronous call.
    let Some(data) = (unsafe { bytes(encoded, encoded_bytes, MAX_PACKET_BYTES) }) else {
        return false;
    };
    apply(Command::SubmitAdpcm {
        key,
        ticks,
        source_hz,
        mono: true,
        data: data.to_vec(),
    })
}

/// Rust-host path: copy an admitted stream packet directly from guest memory before submission.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_submit_guest(
    key: u32,
    ticks: u64,
    source_hz: u32,
    host_pid: i32,
    guest_data: u32,
    encoded_bytes: usize,
    mono: bool,
) -> bool {
    let block_align = if mono { 36 } else { 72 };
    let Some(data) = guest_audio_bytes(host_pid, guest_data, encoded_bytes, block_align) else {
        return false;
    };
    apply(Command::SubmitAdpcm {
        key,
        ticks,
        source_hz,
        mono,
        data,
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_submit_pcm16_stereo(
    key: u32,
    ticks: u64,
    source_hz: u32,
    pcm: *const i16,
    frames: usize,
) -> bool {
    // SAFETY: the caller provides `frames * 2` readable PCM samples for this call.
    let Some(pcm) = (unsafe { samples(pcm, frames) }) else {
        return false;
    };
    apply(Command::SubmitPcm16Stereo {
        key,
        ticks,
        source_hz,
        pcm: pcm.to_vec(),
    })
}

/// Rust-host path: snapshot an admitted XMV stereo PCM16 packet from guest memory and submit it.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_submit_guest_pcm16_stereo(
    key: u32,
    ticks: u64,
    source_hz: u32,
    host_pid: i32,
    guest_data: u32,
    pcm_bytes: usize,
) -> bool {
    let Some(pcm) = guest_movie_pcm16_stereo(host_pid, guest_data, pcm_bytes) else {
        return false;
    };
    apply(Command::SubmitPcm16Stereo {
        key,
        ticks,
        source_hz,
        pcm,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_cut_stream(key: u32, ticks: u64) -> bool {
    apply(Command::CutStream { key, ticks })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_set_stream_running(
    key: u32,
    ticks: u64,
    running: bool,
) -> bool {
    apply(Command::SetStreamRunning {
        key,
        ticks,
        running,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_set_stream_frequency(
    key: u32,
    ticks: u64,
    source_hz: u32,
) -> bool {
    apply(Command::SetStreamFrequency {
        key,
        ticks,
        source_hz,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_frequency_stream(
    key: u32,
    serial: u64,
    ticks: u64,
    base_hz: u32,
    old_hz: u32,
    new_hz: u32,
) -> bool {
    apply(Command::FrequencyStream {
        key,
        serial,
        ticks,
        base_hz,
        old_hz,
        new_hz,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_set_stream_volume(key: u32, millibels: i32) -> bool {
    apply(Command::SetStreamVolume { key, millibels })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_reset_stream_volume(key: u32) -> bool {
    apply(Command::ResetStreamVolume { key })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_route_stream(
    key: u32,
    serial: u64,
    ticks: u64,
    routing: *const CRouting,
) -> bool {
    if routing.is_null() {
        return false;
    }
    // SAFETY: the caller passes a pointer to the public C routing record for this synchronous call.
    let routing = Routing::from(unsafe { *routing });
    apply(Command::RouteStream {
        key,
        serial,
        ticks,
        routing,
    })
}

/// Rust-host route transaction: sample the selected shared kernel clock in Rust before applying
/// the timestamped event. The default C host retains its own clock probe and passes the timestamp.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_route_stream_now(
    key: u32,
    serial: u64,
    routing: *const CRouting,
) -> bool {
    let ticks = sample_kernel_clock_ticks();
    // SAFETY: forward the same synchronous C record contract to the timestamped entry point.
    unsafe { dsound_audio_runtime_route_stream(key, serial, ticks, routing) }
}

/// Rust-owned completion callback used by the Rust audio host. C registers this function only
/// when passive completion and the Rust mixer are active. The configured gate preserves the old
/// completion-enabled and clock-domain checks before sampling the shared kernel clock and
/// applying the route transaction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_route_stream_completion_rust(
    key: u32,
    serial: u64,
    routing: *const CRouting,
) -> bool {
    if routing.is_null() {
        return false;
    }
    {
        let state = runtime();
        if !state.completion_route_enabled
            || state.completion_clock_frequency != state.clock_frequency
            || state.mixer.is_none()
        {
            return false;
        }
    }
    let ticks = sample_kernel_clock_ticks();
    // SAFETY: the stream adapter keeps this routing snapshot alive through the synchronous call.
    let route = Routing::from(unsafe { *routing });
    let mut state = runtime();
    if !state.completion_route_enabled
        || state.completion_clock_frequency != state.clock_frequency
        || state.mixer.is_none()
    {
        return false;
    }
    let command = Command::RouteStream {
        key,
        serial,
        ticks,
        routing: route,
    };
    let accepted = state
        .mixer
        .as_mut()
        .is_some_and(|mixer| mixer.apply(&command));
    if let Some(capture) = state.capture.as_mut() {
        capture.command(&command, accepted, None);
    }
    accepted
}

/// Rust-host admission for the passive completion model's stream-route edge.
/// The C adapter supplies a lock-protected policy snapshot; Rust owns the
/// clock-domain comparison and only samples/applies a route when it matches.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_route_stream_admitted_now(
    key: u32,
    serial: u64,
    completion_enabled: bool,
    completion_clock_frequency: u64,
    routing: *const CRouting,
) -> bool {
    if !completion_enabled || completion_clock_frequency != dsound_audio_runtime_clock_frequency() {
        return false;
    }
    // SAFETY: forward the synchronous routing record after the admission gate passes.
    unsafe { dsound_audio_runtime_route_stream_now(key, serial, routing) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_render(
    now_ticks: u64,
    output: *mut i16,
    frame_capacity: usize,
    frames_written: *mut usize,
) -> bool {
    if frames_written.is_null() || (frame_capacity != 0 && output.is_null()) {
        return false;
    }
    // SAFETY: the caller passes a writable frame count and, when needed, `frame_capacity * 2` samples.
    unsafe { *frames_written = 0 };
    let Some(sample_capacity) = frame_capacity.checked_mul(2) else {
        return false;
    };
    let mut state = runtime();
    let Some(mixer) = state.mixer.as_mut() else {
        return true;
    };
    let pcm = mixer.render_until(now_ticks, frame_capacity);
    if pcm.len() > sample_capacity {
        return false;
    }
    if !pcm.is_empty() {
        // SAFETY: validated output and capacity above; mixer never returns more than requested.
        unsafe { std::ptr::copy_nonoverlapping(pcm.as_ptr(), output, pcm.len()) };
    }
    if let Some(capture) = state.capture.as_mut() {
        capture.render(now_ticks, frame_capacity, &pcm);
    }
    // SAFETY: checked non-null at function entry.
    unsafe { *frames_written = pcm.len() / 2 };
    true
}

/// Samples the host clock per chunk, renders the SoundHost, then synchronously hands PCM to the
/// host's existing sink callback. The selected runtime owns the producer loop; the callback owns
/// only device presentation and must not retain the temporary PCM slice.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_pump(
    clock: Option<unsafe extern "C" fn(*mut c_void) -> u64>,
    sink: Option<unsafe extern "C" fn(*mut c_void, *const i16, usize) -> bool>,
    context: *mut c_void,
    frames_written: *mut u64,
) -> bool {
    let Some(clock) = clock else {
        return false;
    };
    // SAFETY: this closure forwards the caller's synchronous clock callback unchanged.
    unsafe { pump_inner(|| clock(context), sink, context, frames_written, false) }
}

/// Bevy's direct-ring route has no C PCM sink attachment. A missing or rejecting Rust writer is
/// an error; it must not quietly send the block through the hybrid host sink.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_pump_direct(frames_written: *mut u64) -> bool {
    // SAFETY: only the optional writable result pointer crosses this boundary.
    unsafe {
        pump_inner(
            sample_kernel_clock_ticks,
            None,
            std::ptr::null_mut(),
            frames_written,
            true,
        )
    }
}

fn bevy_audio_ring_selected(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some_and(|value| value == "1")
}

fn bevy_audio_ring_is_selected() -> bool {
    *BEVY_AUDIO_RING_SELECTED.get_or_init(|| {
        bevy_audio_ring_selected(std::env::var_os("TSFP_BEVY_AUDIO_RUST_RING").as_deref())
    })
}

/// Run one guest `DirectSoundDoWork` audio pump. Rust owns ring selection and serializes the
/// complete render against the optional refill worker. The C callbacks are used only by the
/// explicit legacy sink mode; the selected Bevy ring path never invokes them.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_guest_work(
    clock: Option<unsafe extern "C" fn(*mut c_void) -> u64>,
    sink: Option<unsafe extern "C" fn(*mut c_void, *const i16, usize) -> bool>,
    context: *mut c_void,
    frames_written: *mut u64,
) -> bool {
    let _work_guard = crate::work_lock::lock_blocking();
    let direct_ring = bevy_audio_ring_is_selected();
    if direct_ring {
        // SAFETY: the Rust Bevy sink was registered before guest work begins; the direct pump
        // samples the selected Rust kernel clock and never uses the C callbacks above.
        unsafe {
            pump_inner(
                sample_kernel_clock_ticks,
                None,
                std::ptr::null_mut(),
                frames_written,
                true,
            )
        }
    } else {
        let Some(clock) = clock else {
            return false;
        };
        // SAFETY: the legacy clock/sink callbacks remain synchronous host boundaries.
        unsafe { pump_inner(|| clock(context), sink, context, frames_written, false) }
    }
}

unsafe fn pump_inner(
    mut clock: impl FnMut() -> u64,
    sink: Option<unsafe extern "C" fn(*mut c_void, *const i16, usize) -> bool>,
    context: *mut c_void,
    frames_written: *mut u64,
    direct_required: bool,
) -> bool {
    if !direct_required && sink.is_none() {
        return false;
    }
    // The registered callback/context pair stays fixed until this guard drops.
    let _direct_sink_pump = DirectSinkPump::begin();
    let ticks_per_second = dsound_audio_runtime_clock_frequency();
    const CHUNK_FRAMES: usize = 2048;
    let mut total = 0u64;
    loop {
        let now_ticks = clock();
        let pcm = {
            let mut state = runtime();
            let Some(mixer) = state.mixer.as_mut() else {
                if !frames_written.is_null() {
                    // SAFETY: checked non-null; caller provides writable result storage.
                    unsafe { *frames_written = total };
                }
                return true;
            };
            let pcm = mixer.render_until(now_ticks, CHUNK_FRAMES);
            if let Some(capture) = state.capture.as_mut() {
                capture.render(now_ticks, CHUNK_FRAMES, &pcm);
            }
            pcm
        };
        let frames = pcm.len() / 2;
        if frames > CHUNK_FRAMES || pcm.len() != frames * 2 {
            if !frames_written.is_null() {
                // SAFETY: checked non-null; caller provides writable result storage.
                unsafe { *frames_written = total };
            }
            return false;
        }
        if frames != 0 {
            // A zero result uses the legacy C sink only on the hybrid pump. The direct-only pump
            // treats an unhandled block as a failure and has no C PCM callback to invoke.
            let direct_result = direct_sink().map_or(0, |binding| {
                // Preserve the C sink contract: timestamp each output block after rendering.
                let end_vt_ticks = clock();
                // SAFETY: the callback/context is installed by the host and PCM stays live here.
                unsafe {
                    (binding.callback)(
                        binding.context as *mut c_void,
                        pcm.as_ptr(),
                        frames,
                        end_vt_ticks,
                        ticks_per_second,
                    )
                }
            });
            let delivered = if direct_result < 0 {
                false
            } else if direct_result > 0 {
                true
            } else if direct_required {
                false
            } else {
                // SAFETY: baseline mode validated the callback and PCM remains live synchronously.
                unsafe { sink.expect("baseline sink validated")(context, pcm.as_ptr(), frames) }
            };
            if !delivered {
                if !frames_written.is_null() {
                    // SAFETY: checked non-null; caller provides writable result storage.
                    unsafe { *frames_written = total };
                }
                return false;
            }
        }
        total = total.saturating_add(frames as u64);
        if frames != CHUNK_FRAMES {
            if !frames_written.is_null() {
                // SAFETY: checked non-null; caller provides writable result storage.
                unsafe { *frames_written = total };
            }
            return true;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_play_buffer(
    key: u32,
    serial: u64,
    ticks: u64,
    frequency: u32,
    encoded: *const u8,
    encoded_bytes: usize,
    paused: bool,
    looping: bool,
    loop_start: usize,
    loop_end: usize,
    volume: i32,
) -> bool {
    if encoded_bytes > MAX_BUFFER_BYTES || encoded_bytes % 36 != 0 {
        return false;
    }
    // SAFETY: the caller owns the encoded buffer for this synchronous call.
    let Some(encoded) = (unsafe { bytes(encoded, encoded_bytes, MAX_BUFFER_BYTES) }) else {
        return false;
    };
    let Some(pcm) = adpcm::decode(encoded, 1) else {
        return false;
    };
    apply_with_buffer(
        Command::PlayBuffer {
            key,
            serial,
            ticks,
            frequency,
            pcm,
            paused,
            looping,
            loop_start,
            loop_end,
            volume,
        },
        Some(encoded),
    )
}

/// Rust-host path: snapshot encoded buffer data from guest memory without a C-owned copy.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_play_buffer_guest(
    key: u32,
    serial: u64,
    ticks: u64,
    frequency: u32,
    host_pid: i32,
    guest_data: u32,
    encoded_bytes: usize,
    paused: bool,
    looping: bool,
    loop_start: usize,
    loop_end: usize,
    volume: i32,
) -> bool {
    let Some(encoded) = guest_buffer_bytes(host_pid, guest_data, encoded_bytes) else {
        return false;
    };
    let Some(pcm) = adpcm::decode(&encoded, 1) else {
        return false;
    };
    apply_with_buffer(
        Command::PlayBuffer {
            key,
            serial,
            ticks,
            frequency,
            pcm,
            paused,
            looping,
            loop_start,
            loop_end,
            volume,
        },
        Some(&encoded),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_pause_buffer(
    key: u32,
    serial: u64,
    ticks: u64,
    paused: bool,
) -> bool {
    apply(Command::PauseBuffer {
        key,
        serial,
        ticks,
        paused,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_stop_buffer(key: u32, serial: u64, ticks: u64) -> bool {
    apply(Command::StopBuffer { key, serial, ticks })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_last_buffer_refusal() -> *const c_char {
    let reason = runtime()
        .mixer
        .as_ref()
        .map_or("unknown", |mixer| mixer.last_buffer_refusal);
    match reason {
        "the render horizon overflows the tick range" => c"the render horizon overflows the tick range".as_ptr(),
        "no live run for this buffer lease (its Play was retired by the end of its data or never reached the mixer)" => {
            c"no live run for this buffer lease (its Play was retired by the end of its data or never reached the mixer)".as_ptr()
        }
        "the command timestamp is before the buffer's latest recorded event" => {
            c"the command timestamp is before the buffer's latest recorded event".as_ptr()
        }
        "the frequency or volume value is invalid" => c"the frequency or volume value is invalid".as_ptr(),
        "the live loop region is empty or past the buffer's data" => {
            c"the live loop region is empty or past the buffer's data".as_ptr()
        }
        "the 64 event history of the run is full (events not yet rendered)" => {
            c"the 64 event history of the run is full (events not yet rendered)".as_ptr()
        }
        _ => c"unknown".as_ptr(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_frequency_buffer(
    key: u32,
    serial: u64,
    ticks: u64,
    frequency: u32,
) -> bool {
    apply(Command::FrequencyBuffer {
        key,
        serial,
        ticks,
        frequency,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_volume_buffer(
    key: u32,
    serial: u64,
    ticks: u64,
    volume: i32,
) -> bool {
    apply(Command::VolumeBuffer {
        key,
        serial,
        ticks,
        volume,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_loop_buffer(
    key: u32,
    serial: u64,
    ticks: u64,
    loop_start: usize,
    loop_end: usize,
) -> bool {
    apply(Command::LoopBuffer {
        key,
        serial,
        ticks,
        loop_start,
        loop_end,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_enable_mixbin_headroom(enabled: bool) -> bool {
    apply(Command::EnableHeadroom { enabled })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_mixbin_headroom(
    identity: u64,
    ticks: u64,
    bin: u32,
    headroom: u32,
) -> bool {
    apply(Command::SetHeadroom {
        identity,
        ticks,
        bin,
        headroom,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_bind_mixbin_headroom(identity: u64) -> bool {
    apply(Command::BindHeadroom { identity })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_format_stream(
    key: u32,
    serial: u64,
    ticks: u64,
    old_hz: u32,
    new_hz: u32,
) -> bool {
    apply(Command::FormatStream {
        key,
        serial,
        ticks,
        old_hz,
        new_hz,
    })
}

/// Command observation remains the C oracle's logger. Runtime replacement has no C runtime logger;
/// the existing C shim calls these only for observation, so define explicit inactive answers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_cmdlog_line(_line: *const c_char) {}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_cmdlog_enabled() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movie_schedule_prefers_selected_rust_clock_and_keeps_host_fallback() {
        let fallback_calls = AtomicU64::new(0);
        let rust_tick = prefer_selected_rust_clock(Some(17), || {
            fallback_calls.fetch_add(1, Ordering::Relaxed);
            23
        });
        assert_eq!(rust_tick, 17);
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 0);

        let host_tick = prefer_selected_rust_clock(None, || {
            fallback_calls.fetch_add(1, Ordering::Relaxed);
            23
        });
        assert_eq!(host_tick, 23);
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn rust_buffer_playback_reads_a_bounded_guest_snapshot() {
        let (page, address) = crate::test_guest_memory::map_guest_page();
        let expected = (0..36).map(|n| n as u8).collect::<Vec<_>>();
        // SAFETY: the test owns a writable page and copies exactly one ADPCM block into it.
        unsafe { std::ptr::copy_nonoverlapping(expected.as_ptr(), page, expected.len()) };
        let pid = std::process::id() as i32;
        assert_eq!(
            guest_buffer_bytes(pid, address, expected.len()),
            Some(expected)
        );
        assert!(guest_buffer_bytes(pid, address + 4090, 36).is_none());
        assert!(guest_buffer_bytes(pid, address, MAX_BUFFER_BYTES + 36).is_none());
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn rust_stream_submission_reads_checked_guest_packets() {
        let (page, address) = crate::test_guest_memory::map_guest_page();
        let expected = (0..72).map(|n| n as u8).collect::<Vec<_>>();
        // SAFETY: the test owns a writable page and copies exactly one stereo ADPCM block.
        unsafe { std::ptr::copy_nonoverlapping(expected.as_ptr(), page, expected.len()) };
        let pid = std::process::id() as i32;
        assert_eq!(guest_audio_bytes(pid, address, 72, 72), Some(expected));
        assert!(guest_audio_bytes(pid, address, 71, 72).is_none());
        assert!(guest_audio_bytes(pid, address + 4090, 72, 72).is_none());
        assert!(guest_audio_bytes(pid, address, MAX_PACKET_BYTES + 72, 72).is_none());
        assert!(guest_audio_bytes(pid, address, 0, 36).is_none());
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn rust_host_stream_routing_samples_rust_clock_and_rejects_backwards_history() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        dsound_audio_runtime_stop();
        assert!(dsound_audio_runtime_start(48_000, 48_000));
        const STREAM: u32 = 0x8C00_0001;
        let pcm = [1_000i16, -1_000, 500, -500];
        // SAFETY: the test supplies two complete interleaved PCM frames for this synchronous call.
        assert!(unsafe {
            dsound_audio_runtime_submit_pcm16_stereo(STREAM, 0, 48_000, pcm.as_ptr(), 2)
        });
        let routing = CRouting::from(tsfp_audio::routing::Routing::default());
        TEST_KERNEL_CLOCK_TICKS.store(2, Ordering::Relaxed);
        TEST_KERNEL_CLOCK_SAMPLES.store(0, Ordering::Relaxed);
        // SAFETY: routing remains live and immutable for the synchronous transaction.
        assert!(unsafe {
            dsound_audio_runtime_route_stream_admitted_now(
                STREAM,
                0x1_0000_0001,
                true,
                48_000,
                &routing,
            )
        });
        TEST_KERNEL_CLOCK_TICKS.store(3, Ordering::Relaxed);
        assert!(!unsafe {
            dsound_audio_runtime_route_stream_admitted_now(
                STREAM,
                0x1_0000_0001,
                false,
                48_000,
                &routing,
            )
        });
        assert!(!unsafe {
            dsound_audio_runtime_route_stream_admitted_now(
                STREAM,
                0x1_0000_0001,
                true,
                44_100,
                &routing,
            )
        });
        TEST_KERNEL_CLOCK_TICKS.store(1, Ordering::Relaxed);
        // The existing timestamp history refuses a backwards event without changing the route.
        assert!(!unsafe {
            dsound_audio_runtime_route_stream_admitted_now(
                STREAM,
                0x1_0000_0001,
                true,
                48_000,
                &routing,
            )
        });
        assert_eq!(TEST_KERNEL_CLOCK_SAMPLES.load(Ordering::Relaxed), 2);
        dsound_audio_runtime_stop();
    }

    #[test]
    fn rust_host_completion_route_uses_rust_policy_gate() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        dsound_audio_runtime_stop();
        dsound_audio_runtime_set_completion_route_policy_rust(false, 48_000);
        assert!(dsound_audio_runtime_start(48_000, 48_000));
        const STREAM: u32 = 0x8C00_0002;
        let pcm = [1_000i16, -1_000, 500, -500];
        // SAFETY: the test supplies two complete interleaved PCM frames for this synchronous call.
        assert!(unsafe {
            dsound_audio_runtime_submit_pcm16_stereo(STREAM, 0, 48_000, pcm.as_ptr(), 2)
        });
        let routing = CRouting::from(tsfp_audio::routing::Routing::default());
        TEST_KERNEL_CLOCK_TICKS.store(2, Ordering::Relaxed);
        TEST_KERNEL_CLOCK_SAMPLES.store(0, Ordering::Relaxed);
        // SAFETY: routing remains live and immutable for this synchronous callback.
        assert!(!unsafe {
            dsound_audio_runtime_route_stream_completion_rust(STREAM, 0x1_0000_0002, &routing)
        });
        assert_eq!(TEST_KERNEL_CLOCK_SAMPLES.load(Ordering::Relaxed), 0);
        dsound_audio_runtime_set_completion_route_policy_rust(true, 44_100);
        assert!(!unsafe {
            dsound_audio_runtime_route_stream_completion_rust(STREAM, 0x1_0000_0002, &routing)
        });
        assert_eq!(TEST_KERNEL_CLOCK_SAMPLES.load(Ordering::Relaxed), 0);
        dsound_audio_runtime_set_completion_route_policy_rust(true, 48_000);
        assert!(unsafe {
            dsound_audio_runtime_route_stream_completion_rust(STREAM, 0x1_0000_0002, &routing)
        });
        assert_eq!(TEST_KERNEL_CLOCK_SAMPLES.load(Ordering::Relaxed), 1);
        dsound_audio_runtime_stop();
    }

    #[test]
    fn rust_movie_submission_reads_bounded_little_endian_guest_pcm() {
        let (page, address) = crate::test_guest_memory::map_guest_page();
        let expected = [0x34, 0x12, 0x00, 0x80, 0xff, 0x7f, 0xcd, 0xab];
        // SAFETY: the test owns a writable page and copies exactly two stereo PCM frames.
        unsafe { std::ptr::copy_nonoverlapping(expected.as_ptr(), page, expected.len()) };
        let pid = std::process::id() as i32;
        assert_eq!(
            guest_movie_pcm16_stereo(pid, address, expected.len()),
            Some(vec![0x1234, i16::MIN, i16::MAX, -21_555])
        );
        assert!(guest_movie_pcm16_stereo(pid, address, 6).is_none());
        assert!(guest_movie_pcm16_stereo(pid, address + 4090, 8).is_none());
        assert!(guest_movie_pcm16_stereo(pid, address, 0).is_none());
        assert!(guest_movie_pcm16_stereo(pid, address, MAX_MOVIE_PACKET_BYTES + 4).is_none());
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn rust_movie_schedule_owns_pcm_submission_and_stats() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        dsound_audio_runtime_stop();
        assert!(dsound_audio_runtime_start(48_000, 176_400));
        const STREAM: u32 = 0x8C00_0000;
        assert!(crate::movie_schedule_ffi::tsfp_audio_movie_schedule_configure_rust(176_400));
        assert!(
            crate::movie_schedule_ffi::tsfp_audio_movie_schedule_attach_rust(
                STREAM,
                0xA000,
                0xB000,
                0x0044_5071,
                0xDEC0
            )
        );
        crate::movie_schedule_ffi::tsfp_audio_movie_pcm_stats_reset_rust();

        let (page, address) = crate::test_guest_memory::map_guest_page();
        let expected = [0x34, 0x12, 0x00, 0x80, 0xff, 0x7f, 0xcd, 0xab];
        // SAFETY: this test owns a writable page and copies two complete PCM16 stereo frames.
        unsafe { std::ptr::copy_nonoverlapping(expected.as_ptr(), page, expected.len()) };
        assert!(
            crate::movie_schedule_ffi::tsfp_audio_movie_schedule_submit_pcm_rust(
                std::process::id() as i32,
                STREAM,
                0,
                address,
                expected.len() as u32,
            )
        );
        assert_eq!(
            crate::movie_schedule_ffi::tsfp_audio_movie_pcm_packets_rust(),
            1
        );
        assert_eq!(
            crate::movie_schedule_ffi::tsfp_audio_movie_pcm_frames_rust(),
            2
        );
        assert_eq!(
            crate::movie_schedule_ffi::tsfp_audio_movie_pcm_refused_rust(),
            0
        );

        assert!(
            !crate::movie_schedule_ffi::tsfp_audio_movie_schedule_submit_pcm_rust(
                std::process::id() as i32,
                STREAM,
                0,
                address + 4090,
                8,
            )
        );
        assert_eq!(
            crate::movie_schedule_ffi::tsfp_audio_movie_pcm_refused_rust(),
            1
        );

        assert!(crate::movie_schedule_ffi::tsfp_audio_movie_schedule_detach_rust(STREAM));
        crate::test_guest_memory::unmap_guest_page(page);
        dsound_audio_runtime_stop();
        crate::movie_schedule_ffi::tsfp_audio_movie_pcm_stats_reset_rust();
    }

    #[test]
    fn bevy_audio_ring_requires_the_exact_opt_in_value() {
        assert!(!bevy_audio_ring_selected(None));
        assert!(!bevy_audio_ring_selected(Some(std::ffi::OsStr::new("0"))));
        assert!(bevy_audio_ring_selected(Some(std::ffi::OsStr::new("1"))));
        assert!(!bevy_audio_ring_selected(Some(std::ffi::OsStr::new(
            "true"
        ))));
    }

    struct PumpCapture {
        ticks: u64,
        pcm: Vec<i16>,
        clock_calls: usize,
        sink_calls: usize,
        direct_ticks: Vec<(u64, u64)>,
        reconfiguration_rejected_while_pumping: bool,
    }

    unsafe extern "C" fn pump_clock(context: *mut c_void) -> u64 {
        // SAFETY: the test passes a live PumpCapture for the duration of the pump.
        let capture = unsafe { &mut *context.cast::<PumpCapture>() };
        capture.clock_calls += 1;
        capture.ticks
    }

    unsafe extern "C" fn pump_sink(context: *mut c_void, pcm: *const i16, frames: usize) -> bool {
        // SAFETY: pump passes the declared interleaved frame count for its live temporary buffer.
        let capture = unsafe { &mut *context.cast::<PumpCapture>() };
        let samples = unsafe { std::slice::from_raw_parts(pcm, frames * 2) };
        capture.pcm.extend_from_slice(samples);
        capture.sink_calls += 1;
        true
    }

    unsafe extern "C" fn pump_direct_sink(
        context: *mut c_void,
        pcm: *const i16,
        frames: usize,
        ticks: u64,
        ticks_per_second: u64,
    ) -> i32 {
        // SAFETY: the test installs this callback with a live PumpCapture until the pump returns.
        let capture = unsafe { &mut *context.cast::<PumpCapture>() };
        // SAFETY: the pump passes a readable interleaved block for the duration of this callback.
        let samples = unsafe { std::slice::from_raw_parts(pcm, frames * 2) };
        capture.pcm.extend_from_slice(samples);
        capture.direct_ticks.push((ticks, ticks_per_second));
        capture.reconfiguration_rejected_while_pumping =
            !dsound_audio_runtime_try_set_direct_sink(None, std::ptr::null_mut());
        1
    }

    static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn runtime_ffi_applies_and_renders_the_contract_pcm() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        dsound_audio_runtime_stop();
        assert!(dsound_audio_runtime_start(48_000, 48_000));
        assert!(dsound_audio_runtime_active());
        assert_eq!(dsound_audio_runtime_clock_frequency(), 48_000);

        let input = [1200, -1200, 900, -900, 600, -600, 300, -300];
        // SAFETY: input has four interleaved stereo frames and lives through the call.
        assert!(unsafe {
            dsound_audio_runtime_submit_pcm16_stereo(0x1234, 0, 48_000, input.as_ptr(), 4)
        });

        let mut reference = Mixer::new(48_000, 48_000);
        assert!(reference.apply(&Command::SubmitPcm16Stereo {
            key: 0x1234,
            ticks: 0,
            source_hz: 48_000,
            pcm: input.to_vec(),
        }));
        let expected = reference.render_until(4, 4);
        let mut actual = [0i16; 8];
        let mut frames = usize::MAX;
        // SAFETY: actual has capacity for four stereo frames and frames is writable.
        assert!(unsafe { dsound_audio_runtime_render(4, actual.as_mut_ptr(), 4, &mut frames) });
        assert_eq!(frames, 4);
        assert_eq!(&actual[..frames * 2], expected.as_slice());

        dsound_audio_runtime_stop();
        assert!(!dsound_audio_runtime_active());
        assert_eq!(dsound_audio_runtime_clock_frequency(), 0);
    }

    #[test]
    fn stopped_runtime_render_reports_zero_frames() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        dsound_audio_runtime_stop();
        let mut frames = usize::MAX;
        // SAFETY: frames is a valid writable pointer; zero capacity permits a null PCM pointer.
        assert!(unsafe { dsound_audio_runtime_render(0, std::ptr::null_mut(), 0, &mut frames) });
        assert_eq!(frames, 0);
    }

    #[test]
    fn runtime_pump_owns_chunk_loop_and_forwards_contract_pcm() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        dsound_audio_runtime_stop();
        assert!(dsound_audio_runtime_start(48_000, 48_000));
        let input: Vec<i16> = (0..8192)
            .map(|sample| (sample as i16).wrapping_mul(7))
            .collect();
        // SAFETY: input holds 4096 interleaved stereo frames until the submit call returns.
        assert!(unsafe {
            dsound_audio_runtime_submit_pcm16_stereo(0x4321, 0, 48_000, input.as_ptr(), 4096)
        });

        let mut reference = Mixer::new(48_000, 48_000);
        assert!(reference.apply(&Command::SubmitPcm16Stereo {
            key: 0x4321,
            ticks: 0,
            source_hz: 48_000,
            pcm: input.clone(),
        }));
        let expected = reference.render_until(4096, 4096);

        let mut capture = PumpCapture {
            ticks: 4096,
            pcm: Vec::new(),
            clock_calls: 0,
            sink_calls: 0,
            direct_ticks: Vec::new(),
            reconfiguration_rejected_while_pumping: false,
        };
        let mut frames = u64::MAX;
        // SAFETY: both callbacks use the live context synchronously and frames is writable.
        assert!(unsafe {
            dsound_audio_runtime_pump(
                Some(pump_clock),
                Some(pump_sink),
                (&mut capture as *mut PumpCapture).cast(),
                &mut frames,
            )
        });
        assert_eq!(frames, 4096);
        assert_eq!(capture.pcm, expected);
        assert_eq!(capture.clock_calls, 3);
        assert_eq!(capture.sink_calls, 2);
        dsound_audio_runtime_stop();
    }

    unsafe extern "C" fn unhandled_direct_sink(
        _context: *mut c_void,
        _pcm: *const i16,
        _frames: usize,
        _ticks: u64,
        _ticks_per_second: u64,
    ) -> i32 {
        0
    }

    #[test]
    fn pump_uses_registered_rust_sink_before_legacy_host_sink() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        dsound_audio_runtime_stop();
        dsound_audio_runtime_set_direct_sink(None, std::ptr::null_mut());
        assert!(dsound_audio_runtime_start(48_000, 48_000));
        let input: Vec<i16> = (0..8192)
            .map(|sample| (sample as i16).wrapping_mul(11))
            .collect();
        // SAFETY: input holds 4096 interleaved stereo frames until the submit call returns.
        assert!(unsafe {
            dsound_audio_runtime_submit_pcm16_stereo(0x4322, 0, 48_000, input.as_ptr(), 4096)
        });
        let mut capture = PumpCapture {
            ticks: 4096,
            pcm: Vec::new(),
            clock_calls: 0,
            sink_calls: 0,
            direct_ticks: Vec::new(),
            reconfiguration_rejected_while_pumping: false,
        };
        dsound_audio_runtime_set_direct_sink(
            Some(pump_direct_sink),
            (&mut capture as *mut PumpCapture).cast(),
        );
        let mut frames = u64::MAX;
        // SAFETY: both callbacks use the live context synchronously and frames is writable.
        assert!(unsafe {
            dsound_audio_runtime_pump(
                Some(pump_clock),
                Some(pump_sink),
                (&mut capture as *mut PumpCapture).cast(),
                &mut frames,
            )
        });
        assert_eq!(frames, 4096);
        assert_eq!(capture.pcm.len(), 8192);
        assert_eq!(capture.direct_ticks, vec![(4096, 48_000), (4096, 48_000)]);
        assert_eq!(capture.clock_calls, 5);
        assert_eq!(capture.sink_calls, 0);
        assert!(capture.reconfiguration_rejected_while_pumping);
        assert!(dsound_audio_runtime_try_set_direct_sink(
            None,
            std::ptr::null_mut()
        ));
        dsound_audio_runtime_stop();
    }

    #[test]
    fn direct_only_pump_fails_without_calling_the_legacy_sink() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        dsound_audio_runtime_stop();
        assert!(dsound_audio_runtime_start(48_000, 48_000));
        TEST_KERNEL_CLOCK_TICKS.store(2, Ordering::Relaxed);
        TEST_KERNEL_CLOCK_SAMPLES.store(0, Ordering::Relaxed);
        let input = [1_000i16, -1_000, 500, -500];
        // SAFETY: input contains two interleaved frames and remains live for submission.
        assert!(unsafe {
            dsound_audio_runtime_submit_pcm16_stereo(0x4323, 0, 48_000, input.as_ptr(), 2)
        });
        let capture = PumpCapture {
            ticks: 2,
            pcm: Vec::new(),
            clock_calls: 0,
            sink_calls: 0,
            direct_ticks: Vec::new(),
            reconfiguration_rejected_while_pumping: false,
        };
        dsound_audio_runtime_set_direct_sink(Some(unhandled_direct_sink), std::ptr::null_mut());
        let mut frames = u64::MAX;
        // SAFETY: frames is writable for the direct pump's synchronous call.
        assert!(!unsafe { dsound_audio_runtime_pump_direct(&mut frames) });
        assert_eq!(frames, 0);
        assert_eq!(TEST_KERNEL_CLOCK_SAMPLES.load(Ordering::Relaxed), 2);
        assert_eq!(capture.sink_calls, 0);
        assert!(capture.pcm.is_empty());
        assert!(dsound_audio_runtime_try_set_direct_sink(
            None,
            std::ptr::null_mut()
        ));
        dsound_audio_runtime_stop();
    }
}
