// SPDX-License-Identifier: GPL-3.0-or-later
//! T3916: the `tsfp_audio_host_*` entry points of the DirectSound HLE host wiring, in Rust. They replace the C units
//! `src/audio/tsfp_audio_host_lifecycle.c` and `src/host/tsfp_audio_host_wiring.c` when the host is built with
//! `TSFP_RUST_AUDIO_HOST=ON` (linked through `tsfp-audio-capi`, feature `rust-audio-host`); `=OFF` keeps the C units.
//!
//! The calls and their order are those of the C units (T3913), pinned by
//! `tests/test_rust_audio_host_build_review.py`. The wiring decisions the C units kept in file statics (`g_stream_routing_pcm`,
//! `g_movie_device_calls`) are Rust-owned state here. The module implementations behind the calls are still C
//! (`src/audio/dsound_*.c`); moving them is the rest of T3879. The compare hook (`dsound_audio_cmp_start`) is not called:
//! compare mode requires the C-host oracle and is refused together with this host (`cmake/rust-audio-cmp.cmake`).
#![allow(unused_unsafe)] // the printing macro wraps its own call
mod ffi;
pub mod gain;
/// T3879: the stream handler glue of the former `dsound_stream.c`; re-exported so the cdylib exports its C symbols.
pub use tsfp_audio_stream_glue as stream_glue;
pub mod pump;

use ffi::{Cb, CensusEntry};
use std::ffi::c_char;
use std::sync::atomic::{AtomicBool, Ordering};

/// `tsfp_audio_host_config` of `src/host/tsfp_audio_host_wiring.h`.
#[repr(C)]
pub struct HostConfig {
    headless_effects: bool,
    gp_effects: bool,
    headless_streams: bool,
    headless_buffers: bool,
    headless_listener: bool,
    headless_movie_audio: bool,
    passive_audio_completion: bool,
    passive_audio_completion_at_dowork: bool,
    fatal: Cb,
    irql: Cb,
    control_word: Cb,
    movie_callback: Cb,
    work_route: Cb,
}

/// Decided in the early phase, read by the late phase and the reports.
struct WiringState {
    stream_routing_pcm: AtomicBool,
    movie_device_calls: AtomicBool,
}

static STATE: WiringState = WiringState {
    stream_routing_pcm: AtomicBool::new(false),
    movie_device_calls: AtomicBool::new(false),
};

macro_rules! fail {
    ($msg:literal) => {{
        unsafe { ffi::fprintf(ffi::stderr, $msg.as_ptr()) };
        return false;
    }};
}

// ---- lifecycle ---------------------------------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_reset_all() {
    unsafe {
        ffi::dsound_hle_init();
        ffi::dsound_listener_reset();
        ffi::dsound_buffer_reset();
        ffi::dsound_stream_reset();
        ffi::dsound_effects_binding_reset();
        ffi::dsound_device_reset();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_register_report_surface() {
    unsafe {
        ffi::dsound_device_register();
        ffi::dsound_device_register_movie();
        ffi::dsound_hrtf_register();
        ffi::dsound_effects_binding_register();
        ffi::dsound_stream_register();
        ffi::dsound_buffer_register();
        ffi::dsound_listener_register();
    }
}

/// # Safety
/// Both pointers must be valid for a `bool` write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_host_teardown(
    buffers_cleaned: *mut bool,
    streams_cleaned: *mut bool,
) {
    unsafe {
        ffi::dsound_listener_reset();
        *buffers_cleaned = ffi::dsound_buffer_reset_checked();
        if !*buffers_cleaned {
            ffi::fprintf(
                ffi::stderr,
                c"buffer cleanup refused changed ownership; allocations preserved\n".as_ptr(),
            );
        }
        *streams_cleaned = ffi::dsound_stream_reset_checked();
        if !*streams_cleaned {
            ffi::fprintf(
                ffi::stderr,
                c"stream cleanup refused changed ownership; allocations preserved\n".as_ptr(),
            );
        }
        ffi::dsound_effects_binding_reset();
        ffi::dsound_device_reset();
    }
}

// ---- configuration -----------------------------------------------------------------------------------------------

/// # Safety
/// `config` must point to a valid `tsfp_audio_host_config`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_host_configure_early(config: *const HostConfig) -> bool {
    let c = unsafe { &*config };
    unsafe {
        ffi::dsound_effects_binding_set_fatal(c.fatal);
        ffi::dsound_effects_binding_set_enabled(c.headless_effects);
        if !ffi::dsound_effects_binding_set_gp_enabled(c.gp_effects) {
            fail!(c"GP effects policy must be configured before the effects binding is created\n");
        }
        if c.gp_effects && !ffi::xdk_thunk_declare_abi(0x00407A02, ffi::XDK_CC_STDCALL, 6, 0) {
            fail!(c"--gp-effects original SetEffectData ABI declaration failed\n");
        }
        ffi::dsound_stream_set_fatal(c.fatal);
        ffi::dsound_stream_set_irql_provider(c.irql);
        ffi::dsound_stream_set_enabled(c.headless_streams);
        let routing_pcm = c.passive_audio_completion && ffi::dsound_audio_runtime_active();
        STATE
            .stream_routing_pcm
            .store(routing_pcm, Ordering::Relaxed);
        ffi::dsound_stream_set_routing_note(if routing_pcm {
            Some(ffi::dsound_audio_runtime_route_stream_completion_rust as unsafe extern "C" fn())
        } else {
            None
        });
        let passive = c.passive_audio_completion;
        ffi::dsound_stream_set_frequency_note(
            passive.then_some(ffi::dsound_completion_note_frequency as unsafe extern "C" fn()),
            if passive { c.control_word } else { None },
        );
        ffi::dsound_stream_set_format_note(
            routing_pcm.then_some(ffi::dsound_completion_note_format as unsafe extern "C" fn()),
        );
        if c.headless_streams
            && passive
            && !ffi::xdk_thunk_declare_abi(0x004085CF, ffi::XDK_CC_STDCALL, 2, 0)
        {
            fail!(c"stream frequency original ABI declaration refused\n");
        }
        if c.headless_streams && routing_pcm {
            // T1129: exact original forwarding bodies end RET8; explicit ABI does not weaken measured quorum for any
            // other XDK method.
            for entry in [0x00407B19u32, 0x00407B1E, 0x004085D4] {
                if !ffi::xdk_thunk_declare_abi(entry, ffi::XDK_CC_STDCALL, 2, 0) {
                    ffi::fprintf(
                        ffi::stderr,
                        c"stream routing ABI declaration refused %#x\n".as_ptr(),
                        entry,
                    );
                    return false;
                }
            }
            ffi::printf(
                c"stream routing stereo speaker projection/timeline INFERRED; DSP/HRTF routes refused\n".as_ptr(),
            );
        }
        ffi::dsound_buffer_set_fatal(c.fatal);
        ffi::dsound_buffer_set_irql_provider(c.irql);
        ffi::dsound_buffer_set_enabled(c.headless_buffers);
        ffi::dsound_listener_set_fatal(c.fatal);
        ffi::dsound_listener_set_irql_provider(c.irql);
        ffi::dsound_listener_set_enabled(c.headless_listener);
        let mixbin_pcm =
            routing_pcm && (c.headless_streams || c.headless_buffers || c.headless_movie_audio);
        if mixbin_pcm && !ffi::dsound_audio_runtime_enable_mixbin_headroom(true) {
            fail!(c"global mix-bin headroom PCM initialization failed\n");
        }
        ffi::dsound_mixbin_headroom_configure(
            mixbin_pcm.then_some(ffi::dsound_completion_mixbin_headroom as unsafe extern "C" fn()),
            mixbin_pcm
                .then_some(ffi::dsound_completion_bind_mixbin_headroom as unsafe extern "C" fn()),
            c.irql,
            c.fatal,
        );
        if mixbin_pcm && !ffi::xdk_thunk_declare_abi(0x00407A2C, ffi::XDK_CC_STDCALL, 3, 0) {
            fail!(c"global mix-bin headroom original ABI declaration failed\n");
        }
        if mixbin_pcm {
            ffi::printf(
                c"global submix headroom enabled; stereo projection/timeline INFERRED, DSP/HRTF unsupported\n".as_ptr(),
            );
        }
        ffi::dsound_device_set_fatal(c.fatal);
        ffi::dsound_hrtf_set_fatal(c.fatal);
        ffi::dsound_hrtf_set_irql_provider(c.irql);
        ffi::dsound_device_register();
        ffi::dsound_mixbin_headroom_register();
        ffi::dsound_hrtf_register();
        ffi::dsound_effects_binding_register();
        ffi::dsound_stream_register();
        ffi::dsound_movie_stream_set_fatal(c.fatal);
        ffi::dsound_movie_stream_set_irql_provider(c.irql);
        ffi::dsound_movie_stream_set_callback_runner(c.movie_callback);
        ffi::dsound_movie_stream_set_enabled(c.headless_movie_audio);
        ffi::dsound_stream_set_extension(
            Some(ffi::dsound_movie_stream_route_public as unsafe extern "C" fn()),
            Some(ffi::dsound_movie_stream_reset_checked as unsafe extern "C" fn()),
        );
    }
    true
}

/// # Safety
/// `config` must point to a valid `tsfp_audio_host_config`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_host_configure_late(config: *const HostConfig) -> bool {
    let c = unsafe { &*config };
    let passive = c.passive_audio_completion;
    let movies = c.headless_movie_audio;
    unsafe {
        ffi::dsound_completion_set_fatal(c.fatal);
        ffi::dsound_completion_set_enabled(passive);
        ffi::dsound_completion_set_dowork_delivery(passive && c.passive_audio_completion_at_dowork);
        ffi::dsound_stream_set_completion(
            passive,
            Some(ffi::dsound_completion_note_pause as unsafe extern "C" fn()),
        );
        ffi::dsound_buffer_set_completion(
            passive,
            Some(ffi::dsound_completion_buffer_started as unsafe extern "C" fn()),
        );
        ffi::dsound_buffer_set_completion_pause(Some(
            ffi::dsound_completion_buffer_pause as unsafe extern "C" fn(),
        ));
        ffi::dsound_buffer_set_completion_frequency(Some(
            ffi::dsound_completion_buffer_frequency as unsafe extern "C" fn(),
        ));
        ffi::dsound_buffer_set_completion_voice_running(Some(
            ffi::dsound_completion_buffer_voice_running as unsafe extern "C" fn(),
        ));
        ffi::dsound_buffer_set_completion_loop(Some(
            ffi::dsound_completion_buffer_loop as unsafe extern "C" fn(),
        ));
        ffi::dsound_completion_register();
        ffi::dsound_listener_set_work_route(c.work_route);
        if !ffi::xdk_thunk_set_completion_method_handler(
            passive.then_some(ffi::dsound_completion_method_owned as unsafe extern "C" fn()),
            passive.then_some(ffi::dsound_completion_route_method as unsafe extern "C" fn()),
        ) {
            fail!(c"passive audio completion route requires all compiled stream stop boundaries\n");
        }
        if !ffi::xdk_thunk_set_movie_method_handler(
            movies.then_some(ffi::dsound_movie_stream_method_owned as unsafe extern "C" fn()),
            movies.then_some(ffi::dsound_movie_stream_route_method as unsafe extern "C" fn()),
        ) {
            fail!(c"movie stream method route requires all compiled stream stop boundaries\n");
        }
        // T421: the XMV GetNextFrame device calls (second DirectSoundCreate, SynchPlayback, device Release) need the
        // compiled SynchPlayback stop boundary of the re-lift. A lift without it keeps the movie stream model and the
        // XMV DirectSoundCreate stays refused, as before.
        let device_calls = movies && ffi::xdk_thunk_synch_stop_ready();
        STATE
            .movie_device_calls
            .store(device_calls, Ordering::Relaxed);
        ffi::dsound_device_set_movie_calls(device_calls);
        if device_calls {
            ffi::dsound_device_register_movie();
        }
        if !ffi::xdk_thunk_set_synch_handler(
            device_calls.then_some(ffi::dsound_movie_stream_route_synch as unsafe extern "C" fn()),
        ) {
            fail!(c"SynchPlayback route requires the compiled SynchPlayback stop boundary\n");
        }
        if !ffi::xdk_thunk_set_stream_virtual_handlers(
            c.headless_streams
                .then_some(ffi::dsound_stream_cache_discontinuity as unsafe extern "C" fn()),
            c.headless_streams
                .then_some(ffi::dsound_stream_get_startup_status as unsafe extern "C" fn()),
        ) {
            fail!(c"stream virtual request policy requires the complete compiled DSOUND guards\n");
        }
        // dsound_audio_cmp_start is the compare-mode hook and a no-op in every build that can link this crate.
        ffi::dsound_buffer_register();
        ffi::dsound_listener_register();
    }
    true
}

// ---- reports -----------------------------------------------------------------------------------------------------

macro_rules! out {
    ($fmt:literal $(, $arg:expr)*) => {
        unsafe { ffi::printf($fmt.as_ptr() $(, $arg)*) }
    };
}

/// # Safety
/// `config` must point to a valid `tsfp_audio_host_config`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_host_print_policy(config: *const HostConfig) {
    let c = unsafe { &*config };
    if c.headless_movie_audio && unsafe { ffi::dsound_audio_runtime_active() } {
        out!(c"dsound policy  XMV movie stream bounded CPU model; its 44100 Hz stereo S16 PCM is mixed into the audio sink, packets complete in DirectSoundDoWork on the virtual clock at the format byte rate\n");
    } else if c.headless_movie_audio {
        out!(c"dsound policy  XMV movie stream bounded CPU model; no audio is produced, packets complete in DirectSoundDoWork on the virtual clock at the format byte rate\n");
    }
    if STATE.movie_device_calls.load(Ordering::Relaxed) {
        out!(c"dsound policy  XMV device calls: a second DirectSoundCreate and device Release adjust the device reference, SynchPlayback starts the Pause(2) movie streams; no APU voice exists\n");
    } else if c.headless_movie_audio {
        out!(c"dsound policy  XMV device calls NOT enabled: the lift lacks the SynchPlayback stop boundary (tools/config/movie_synch_boundaries.json), the XMV DirectSoundCreate stays refused\n");
    }
    if c.gp_effects {
        out!(c"dsound policy  Rust GP effects binding enabled by default; real image attestation required; GP frame-to-PCM scheduling not yet evidenced\n");
    } else if c.headless_effects {
        out!(c"dsound policy  passive CPU effects views; initial state copied, workspace zeroed; no DSP execution or acknowledgement\n");
    }
    out!(c"dsound handlers %zu registered\n", unsafe {
        ffi::dsound_hle_implemented_count()
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_print_completion_report() {
    let s = unsafe { ffi::dsound_completion_get_stats() };
    out!(
        c"passive audio  thunk-routed stream methods %llu, packets %u (completed %u, aborted %u), buffer plays %u (finished %u), observations %u, buffer stops %u (T681, T733)\n",
        unsafe { ffi::xdk_thunk_completion_method_call_count() },
        s.stream_packets,
        s.stream_completed,
        s.stream_aborted,
        s.buffer_plays,
        s.buffer_finished,
        s.observations,
        s.buffer_stops
    );
    out!(
        c"stream pitch   %llu accepted owned frequency transactions (T1245); PCM timing/projection INFERRED, no APU pitch writes\n",
        unsafe { ffi::dsound_stream_frequency_set_count() }
    );
    let mut census = [CensusEntry::default(); ffi::CENSUS_MAX];
    let count =
        unsafe { ffi::dsound_completion_stream_census(census.as_mut_ptr(), ffi::CENSUS_MAX) };
    for e in census.iter().take(count.min(ffi::CENSUS_MAX)) {
        let verdict: *const c_char = if e.mixed != 0 {
            c"MIXED into the sink".as_ptr()
        } else {
            c"NOT MIXED (silent)".as_ptr()
        };
        out!(
            c"passive audio  stream format tag %#x channels %u rate %u block %u bits %u: packets %u, bytes %u, first at guest %.1f s, last %.1f s, %s (T1238)\n",
            e.tag,
            e.channels,
            e.rate,
            e.block,
            e.bits,
            e.packets,
            e.bytes,
            e.first_ms as f64 / 1000.0,
            e.last_ms as f64 / 1000.0,
            verdict
        );
    }
}

/// # Safety
/// `config` must point to a valid `tsfp_audio_host_config`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_host_print_movie_report(config: *const HostConfig) {
    let c = unsafe { &*config };
    if !c.headless_movie_audio {
        return;
    }
    let on = |flag: bool| -> *const c_char {
        if flag {
            c"on".as_ptr()
        } else {
            c"off".as_ptr()
        }
    };
    unsafe {
        out!(
            c"movie stream   streams %zu, operations %llu, completions %llu, thunk-routed methods %llu\n",
            ffi::dsound_movie_stream_count(),
            ffi::dsound_movie_stream_operation_count(),
            ffi::dsound_movie_stream_completion_count(),
            ffi::xdk_thunk_movie_method_call_count()
        );
        if ffi::dsound_audio_runtime_active() {
            let frames = ffi::dsound_movie_stream_pcm_frames();
            out!(
                c"movie audio    pcm packets %llu, frames %llu (%.3f s at 44100 Hz), refused %llu\n",
                ffi::dsound_movie_stream_pcm_packets(),
                frames,
                frames as f64 / 44100.0,
                ffi::dsound_movie_stream_pcm_refused()
            );
        }
        out!(
            c"movie device   calls %s, SynchPlayback route %s, outstanding references %u, SynchPlayback calls %llu\n",
            on(ffi::dsound_device_movie_calls_enabled()),
            on(ffi::xdk_thunk_synch_route_enabled()),
            ffi::dsound_device_movie_references(),
            ffi::xdk_thunk_synch_call_count()
        );
    }
}

// ---- single option calls (T3917) ---------------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_buffer_stops_ready() -> bool {
    unsafe { ffi::dsound_buffer_stops_ready() }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_set_movie_trace(enabled: bool) {
    unsafe { ffi::dsound_movie_stream_set_trace(enabled) }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_set_hle_log(printer: Cb) {
    unsafe { ffi::dsound_hle_set_log(printer) }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_set_codec_ready() {
    unsafe { ffi::dsound_hle_set_codec_state(ffi::DSOUND_CODEC_READY) }
}
