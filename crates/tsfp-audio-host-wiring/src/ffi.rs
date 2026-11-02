// SPDX-License-Identifier: GPL-3.0-or-later
//! The C side of the wiring: DirectSound HLE modules and thunk routes. Callback parameters are plain function pointers; the
//! C typedefs differ per module but share one pointer ABI, so a single `Cb` carries them all.
use std::ffi::{c_char, c_int, c_void};

pub type Cb = Option<unsafe extern "C" fn()>;

#[repr(C)]
pub struct CompletionStats {
    pub stream_packets: u32,
    pub stream_completed: u32,
    pub buffer_plays: u32,
    pub buffer_finished: u32,
    pub observations: u32,
    pub buffer_stops: u32,
    pub stream_aborted: u32,
}

pub const CENSUS_MAX: usize = 12;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CensusEntry {
    pub tag: u32,
    pub channels: u32,
    pub rate: u32,
    pub block: u32,
    pub bits: u32,
    pub packets: u32,
    pub mixed: u32,
    pub bytes: u32,
    pub first_ms: u64,
    pub last_ms: u64,
}

/// `DSOUND_CODEC_READY` of `dsound_codec_state`.
pub const DSOUND_CODEC_READY: u32 = 1;

/// `xdk_cc` value of the stdcall convention.
pub const XDK_CC_STDCALL: u32 = 0;

unsafe extern "C" {
    pub static stderr: *mut c_void;
    pub fn printf(format: *const c_char, ...) -> c_int;
    pub fn fprintf(stream: *mut c_void, format: *const c_char, ...) -> c_int;

    pub fn dsound_hle_init();
    pub fn dsound_hle_set_log(printer: Cb);
    pub fn dsound_hle_set_codec_state(state: u32);
    pub fn dsound_hle_implemented_count() -> usize;

    pub fn dsound_device_reset();
    pub fn dsound_device_register() -> usize;
    pub fn dsound_device_register_movie() -> usize;
    pub fn dsound_device_set_fatal(handler: Cb);
    pub fn dsound_device_set_movie_calls(enabled: bool);
    pub fn dsound_device_movie_calls_enabled() -> bool;
    pub fn dsound_device_movie_references() -> u32;

    pub fn dsound_hrtf_register() -> u32;
    pub fn dsound_hrtf_set_fatal(fatal: Cb);
    pub fn dsound_hrtf_set_irql_provider(provider: Cb);

    pub fn dsound_effects_binding_reset();
    pub fn dsound_effects_binding_register() -> usize;
    pub fn dsound_effects_binding_set_fatal(handler: Cb);
    pub fn dsound_effects_binding_set_enabled(enabled: bool);
    pub fn dsound_effects_binding_set_gp_enabled(enabled: bool) -> bool;

    pub fn dsound_stream_reset();
    pub fn dsound_stream_reset_checked() -> bool;
    pub fn dsound_stream_register() -> usize;
    pub fn dsound_stream_set_fatal(fatal: Cb);
    pub fn dsound_stream_set_irql_provider(provider: Cb);
    pub fn dsound_stream_set_enabled(enabled: bool);
    pub fn dsound_stream_set_routing_note(note: Cb);
    pub fn dsound_stream_set_frequency_note(note: Cb, control_word: Cb);
    pub fn dsound_stream_set_format_note(note: Cb);
    pub fn dsound_stream_set_completion(enabled: bool, note: Cb);
    pub fn dsound_stream_set_extension(route: Cb, reset: Cb);
    pub fn dsound_stream_cache_discontinuity();
    pub fn dsound_stream_get_startup_status();
    pub fn dsound_stream_frequency_set_count() -> u64;

    pub fn dsound_buffer_stops_ready() -> bool;
    pub fn dsound_buffer_reset();
    pub fn dsound_buffer_reset_checked() -> bool;
    pub fn dsound_buffer_register() -> usize;
    pub fn dsound_buffer_set_fatal(fatal: Cb);
    pub fn dsound_buffer_set_irql_provider(provider: Cb);
    pub fn dsound_buffer_set_enabled(enabled: bool);
    pub fn dsound_buffer_set_completion(enabled: bool, started: Cb);
    pub fn dsound_buffer_set_completion_pause(pause: Cb);
    pub fn dsound_buffer_set_completion_frequency(frequency: Cb);
    pub fn dsound_buffer_set_completion_voice_running(running: Cb);
    pub fn dsound_buffer_set_completion_loop(loop_fn: Cb);

    pub fn dsound_listener_reset();
    pub fn dsound_listener_register() -> usize;
    pub fn dsound_listener_set_fatal(fatal: Cb);
    pub fn dsound_listener_set_irql_provider(provider: Cb);
    pub fn dsound_listener_set_enabled(enabled: bool);
    pub fn dsound_listener_set_work_route(route: Cb);

    pub fn dsound_mixbin_headroom_configure(note: Cb, bind: Cb, irql: Cb, fatal: Cb);
    pub fn dsound_mixbin_headroom_register() -> usize;

    pub fn dsound_movie_stream_set_fatal(fatal: Cb);
    pub fn dsound_movie_stream_set_trace(enabled: bool);
    pub fn dsound_movie_stream_set_irql_provider(provider: Cb);
    pub fn dsound_movie_stream_set_callback_runner(runner: Cb);
    pub fn dsound_movie_stream_set_enabled(enabled: bool);
    pub fn dsound_movie_stream_route_public();
    pub fn dsound_movie_stream_reset_checked();
    pub fn dsound_movie_stream_method_owned();
    pub fn dsound_movie_stream_route_method();
    pub fn dsound_movie_stream_route_synch();
    pub fn dsound_movie_stream_count() -> usize;
    pub fn dsound_movie_stream_operation_count() -> u64;
    pub fn dsound_movie_stream_completion_count() -> u64;
    pub fn dsound_movie_stream_pcm_packets() -> u64;
    pub fn dsound_movie_stream_pcm_frames() -> u64;
    pub fn dsound_movie_stream_pcm_refused() -> u64;

    pub fn dsound_completion_set_fatal(fatal: Cb);
    pub fn dsound_completion_set_enabled(enabled: bool);
    pub fn dsound_completion_set_dowork_delivery(enabled: bool);
    pub fn dsound_completion_register() -> usize;
    pub fn dsound_completion_get_stats() -> CompletionStats;
    pub fn dsound_completion_stream_census(out: *mut CensusEntry, capacity: usize) -> usize;
    pub fn dsound_completion_note_frequency();
    pub fn dsound_completion_note_format();
    pub fn dsound_completion_note_pause();
    pub fn dsound_completion_mixbin_headroom();
    pub fn dsound_completion_bind_mixbin_headroom();
    pub fn dsound_completion_buffer_started();
    pub fn dsound_completion_buffer_pause();
    pub fn dsound_completion_buffer_frequency();
    pub fn dsound_completion_buffer_voice_running();
    pub fn dsound_completion_buffer_loop();
    pub fn dsound_completion_method_owned();
    pub fn dsound_completion_route_method();

    pub fn dsound_audio_runtime_active() -> bool;
    pub fn dsound_audio_runtime_enable_mixbin_headroom(enabled: bool) -> bool;
    pub fn dsound_audio_runtime_route_stream_completion_rust();

    pub fn xdk_thunk_declare_abi(
        address: u32,
        cc: u32,
        stack_args: u32,
        register_args: u32,
    ) -> bool;
    pub fn xdk_thunk_set_completion_method_handler(owns: Cb, handler: Cb) -> bool;
    pub fn xdk_thunk_set_movie_method_handler(owns: Cb, handler: Cb) -> bool;
    pub fn xdk_thunk_set_synch_handler(handler: Cb) -> bool;
    pub fn xdk_thunk_set_stream_virtual_handlers(discontinuity: Cb, status: Cb) -> bool;
    pub fn xdk_thunk_synch_stop_ready() -> bool;
    pub fn xdk_thunk_synch_route_enabled() -> bool;
    pub fn xdk_thunk_completion_method_call_count() -> u64;
    pub fn xdk_thunk_movie_method_call_count() -> u64;
    pub fn xdk_thunk_synch_call_count() -> u64;
}
