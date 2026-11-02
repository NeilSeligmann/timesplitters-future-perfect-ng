// SPDX-License-Identifier: GPL-3.0-or-later
//! The C-callable window/input module (`src/host/window_input.h`, contract in docs/t-host-window-input-contract.md) for the
//! Bevy shell, plus the picture entry `tsfp_bevy_picture` (`src/host/bevy_sink.h`). Same queue semantics as the SDL3
//! implementation (`crates/tsfp-game/src/sdl_window.rs`): both feed `window_input::InputState`, so key names, repeat drop,
//! focus-loss releases, Y negation, the 256 slot queue with counted drops and the close causes are one piece of code. The
//! difference is the event source: Bevy events (`events.rs`) instead of `SDL_PollEvent`.
use crate::shared::{SHARED, lock};
use crate::window_input::{InputEvent, RawEvent};
use std::ffi::{CStr, c_char, c_void};
use std::sync::atomic::Ordering;

#[cfg(not(tsfp_no_host))]
unsafe extern "C" {
    /// T2348: 1-based total fixed simulation tick published by render_rate.c.
    fn tsfp_host_simulation_tick() -> u64;
    /// Number of guest XInput polls completed for one controller port.
    fn xinput_source_port_poll_count(port: u32) -> u64;
    fn xinput_source_guest_boundary() -> u64;
}

// Guest pad polls, the ordered guest boundary (T2046) and the fixed 60 Hz tick: distinct clocks.
// C counters are unavailable in standalone Bevy tests; return zero there.
macro_rules! host_counter {
    ($name:ident, $call:expr) => {
        #[cfg(not(tsfp_no_host))]
        pub fn $name() -> u64 {
            // SAFETY: argument-free C reads linked with the host; each takes its own lock or atomic.
            unsafe { $call }
        }
        #[cfg(tsfp_no_host)]
        pub fn $name() -> u64 {
            0
        }
    };
}
host_counter!(guest_pad_poll_count, xinput_source_port_poll_count(0));
host_counter!(guest_pad_boundary, xinput_source_guest_boundary());
host_counter!(simulation_tick, tsfp_host_simulation_tick());

pub fn host_poll_index() -> u64 {
    SHARED.counters.host_poll_index()
}

fn copy_error(text: &str, out: *mut c_char, size: usize) {
    if out.is_null() || size == 0 {
        return;
    }
    let bytes = text.as_bytes();
    let count = bytes.len().min(size - 1);
    // SAFETY: the caller passes a writable buffer of `size` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), out, count);
        *out.add(count) = 0;
    }
}

/// The "window" is a token: Bevy owns the real one. The title is forwarded to it.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_window_create(
    title: *const c_char,
    vulkan: bool,
    error: *mut c_char,
    error_size: usize,
) -> *mut c_void {
    if vulkan {
        // The C sink creates the hidden SDL surface of the live renderer itself and never asks for a Vulkan token.
        copy_error(
            "the Bevy shell has no SDL Vulkan window to hand out (T2023)",
            error,
            error_size,
        );
        return std::ptr::null_mut();
    }
    if !title.is_null() {
        // SAFETY: `title` is NUL-terminated.
        let text = unsafe { CStr::from_ptr(title) }
            .to_string_lossy()
            .into_owned();
        let mut wi = lock(&SHARED.wi);
        wi.title = Some(text);
        wi.title_changed = true;
    }
    std::ptr::NonNull::<u8>::dangling().as_ptr().cast()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_window_destroy(_window: *mut c_void) {}

/// The input state is process global (one Bevy window); the handle is a non-NULL token.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_new() -> *mut c_void {
    std::ptr::NonNull::<u8>::dangling().as_ptr().cast()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_free(_handle: *mut c_void) {}

/// Close request and redraw flag collected from the Bevy events since the last call. Bit 0 close (`*cause` the first
/// cause, static text), bit 1 redraw.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_pump(_handle: *mut c_void, cause: *mut *const c_char) -> u32 {
    let poll = SHARED.counters.next_host_poll();
    crate::shared::PROGRESS.bump(); // T3394: wake a headless Bevy waiting for guest progress
    crate::input_recording::lockstep_wait(poll); // T2487: fast-forward replay input lockstep
    let result = std::mem::take(&mut lock(&SHARED.wi).pending);
    if !cause.is_null() {
        // SAFETY: the caller passes a writable pointer slot.
        unsafe { *cause = result.close_cause.map_or(std::ptr::null(), CStr::as_ptr) };
    }
    u32::from(result.close_cause.is_some()) | u32::from(result.redraw) << 1
}

/// T3437: called by `xinput_source_poll_port` (weak reference, `src/input/xinput_source.c`) on the
/// guest thread before a controller poll drains the input queue; a replay waits there until the
/// current tick's recorded rows are queued. No lock is held by the caller.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_replay_guest_poll_sync() {
    crate::input_recording::guest_poll_sync();
}

/// T3771: `xinput_pad_connect` / `xinput_pad_disconnect` handed over by `--pad-ports` (src/host/main.c).
type PadPortFn = unsafe extern "C" fn(u32) -> bool;
static PAD_PORT_CALLS: std::sync::OnceLock<(PadPortFn, PadPortFn)> = std::sync::OnceLock::new();

fn pad_port_change(change: tsfp_input_slots::ports::PortChange) -> bool {
    use tsfp_input_slots::ports::PortChange;
    let Some(&(connect, disconnect)) = PAD_PORT_CALLS.get() else {
        return false;
    };
    let (call, port) = match change {
        PortChange::Connect(port) => (connect, port),
        PortChange::Disconnect(port) => (disconnect, port),
    };
    // SAFETY: C functions of the linked host; they take the device lock themselves.
    unsafe { call(u32::from(port)) }
}

/// T3771: weak reference of `--pad-ports` (src/host/main.c): the router gets the guest port presence calls.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_pad_ports_bind(connect: PadPortFn, disconnect: PadPortFn) -> bool {
    let _ = PAD_PORT_CALLS.set((connect, disconnect));
    tsfp_input_slots::ports::bind(pad_port_change);
    true
}

/// T3771: the state of guest port 1 to 3 (guest thread, device lock held); false leaves the port unchanged.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_pad_ports_state(
    port: u32,
    out: *mut tsfp_input_slots::ports::CPadState,
) -> bool {
    match tsfp_input_slots::ports::state(port) {
        // SAFETY: the C provider passes a writable `xinput_pad_state`.
        Some(state) if !out.is_null() => unsafe { out.write(state) },
        _ => return false,
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_enable(
    _handle: *mut c_void,
    keyboard: bool,
    gamepad: bool,
    _error: *mut c_char,
    _error_size: usize,
) -> bool {
    let mut wi = lock(&SHARED.wi);
    wi.input.keyboard |= keyboard;
    wi.input.gamepad |= gamepad;
    true
}

/// `present_input_event` of present_sink.h: kind, static name or NULL, raw code, axis value.
#[repr(C)]
pub struct CInputEvent {
    kind: i32,
    name: *const c_char,
    raw: i32,
    value: i32,
}

impl From<InputEvent> for CInputEvent {
    fn from(event: InputEvent) -> Self {
        Self {
            kind: event.kind as i32,
            name: event.name.map_or(std::ptr::null(), CStr::as_ptr),
            raw: event.raw,
            value: event.value,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_next(_handle: *mut c_void, out: *mut CInputEvent) -> bool {
    match lock(&SHARED.wi).input.next() {
        Some(event) if !out.is_null() => {
            // SAFETY: `out` is a writable present_input_event.
            unsafe { out.write(event.into()) };
            true
        }
        _ => false,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_dropped(_handle: *mut c_void) -> u64 {
    lock(&SHARED.wi).input.dropped()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_pads_open(_handle: *mut c_void) -> usize {
    let wi = lock(&SHARED.wi);
    if wi.input.gamepad { wi.pads.len() } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_shutdown(_handle: *mut c_void) {}

/// Presenter thread, sink lock held: RGB picture into the latest-wins mailbox.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_bevy_picture(number: u64, width: u32, height: u32, rgb: *const u8) {
    if rgb.is_null() || width == 0 || height == 0 {
        return;
    }
    let bytes = width as usize * height as usize * 3;
    // SAFETY: the C sink passes `width*height*3` readable bytes.
    let rgb = unsafe { std::slice::from_raw_parts(rgb, bytes) };
    SHARED.publish(number, width, height, rgb);
}

/// Presenter thread: an RGBA8 frame (the live renderer's composed front) into the mailbox.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_bevy_picture_rgba(number: u64, width: u32, height: u32, rgba: *const u8) {
    if rgba.is_null() || width == 0 || height == 0 {
        return;
    }
    let bytes = width as usize * height as usize * 4;
    // SAFETY: the C sink passes `width*height*4` readable bytes.
    let rgba = unsafe { std::slice::from_raw_parts(rgba, bytes) };
    SHARED.publish_rgba(number, width, height, rgba);
}

/// Close the guest when Bevy itself goes away (app exit, no window left): the pump reports the cause.
pub fn request_quit(event: RawEvent) {
    lock(&SHARED.wi).feed(event);
}

/// Pairs the unused-function guard: rustc codegens only what a bin's `main` reaches, so the C-called exports are
/// referenced from `keep_exports` (T2003 finding).
pub fn keep_exports() {
    let table: [*const (); 14] = [
        tsfp_wi_window_create as *const (),
        tsfp_wi_window_destroy as *const (),
        tsfp_wi_new as *const (),
        tsfp_wi_free as *const (),
        tsfp_wi_pump as *const (),
        tsfp_wi_enable as *const (),
        tsfp_wi_next as *const (),
        tsfp_wi_dropped as *const (),
        tsfp_wi_pads_open as *const (),
        tsfp_wi_shutdown as *const (),
        tsfp_bevy_picture as *const (),
        tsfp_bevy_picture_rgba as *const (),
        tsfp_movie_overlay::tsfp_bevy_movie_overlay as *const (),
        crate::audio::tsfp_bevy_audio_attach as *const (),
    ];
    std::hint::black_box(table);
    std::hint::black_box(crate::audio::tsfp_bevy_audio_detach as *const ());
    // T2246: the C host calls the render runtime by name; the rlib members are only linked when something references them.
    use tsfp_rt_host::{ffi, ffi_audio};
    let runtime: [*const (); 25] = [
        ffi::tsfp_rt_host_current as *const (),
        ffi::tsfp_rt_host_set_output as *const (),
        ffi::tsfp_rt_host_set_window as *const (),
        ffi::tsfp_rt_host_create as *const (),
        ffi::tsfp_rt_host_write as *const (),
        ffi::tsfp_rt_host_send_line as *const (),
        ffi::tsfp_rt_host_send_binary as *const (),
        ffi::tsfp_rt_host_present as *const (),
        ffi::tsfp_rt_host_flush as *const (),
        ffi::tsfp_rt_host_frame as *const (),
        ffi::tsfp_rt_host_alive as *const (),
        ffi::tsfp_rt_host_pressure as *const (),
        ffi::tsfp_rt_host_stats as *const (),
        ffi::tsfp_rt_host_destroy as *const (),
        ffi::tsfp_rt_host_share_layout as *const (),
        ffi::tsfp_rt_host_share_gpu as *const (),
        ffi::tsfp_rt_host_shared_acquire as *const (),
        ffi::tsfp_rt_host_shared_release as *const (),
        ffi_audio::tsfp_rt_audio_create as *const (),
        ffi_audio::tsfp_rt_audio_write as *const (),
        ffi_audio::tsfp_rt_audio_send_line as *const (),
        ffi_audio::tsfp_rt_audio_flush as *const (),
        ffi_audio::tsfp_rt_audio_pull as *const (),
        ffi_audio::tsfp_rt_audio_stats as *const (),
        ffi_audio::tsfp_rt_audio_destroy as *const (),
    ];
    std::hint::black_box(runtime);
    let _ = Ordering::Relaxed;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_the_c_present_input_event() {
        assert_eq!(std::mem::size_of::<CInputEvent>(), 24);
    }

    #[test]
    fn pump_reports_first_close_and_clears() {
        let mut cause: *const c_char = std::ptr::null();
        lock(&SHARED.wi).feed(RawEvent::CloseRequested);
        lock(&SHARED.wi).feed(RawEvent::Quit);
        let flags = tsfp_wi_pump(std::ptr::null_mut(), &mut cause);
        assert_eq!(flags & 1, 1);
        // SAFETY: the cause is a static C string.
        assert_eq!(unsafe { CStr::from_ptr(cause) }, c"window close request");
        assert_eq!(tsfp_wi_pump(std::ptr::null_mut(), &mut cause), 0);
    }
}
