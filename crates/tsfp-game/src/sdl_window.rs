// SPDX-License-Identifier: GPL-3.0-or-later
//! T2003: the C-callable window and input module (src/host/window_input.h) backed by SDL3 through a minimal
//! hand-written FFI. Compiled only with `--cfg tsfp_rust_window` (build.rs, set when the host build dir was
//! configured with -DTSFP_RUST_WINDOW=ON), because it needs the host link line to provide libSDL3.
//!
//! FFI pin: SDL 3.2.10 (`pkg-config --modversion sdl3`), the same shared library the C host links, so the
//! `SDL_Window *` created here is the one the C Vulkan/SDL_Renderer code draws to. No sdl3 crate: cargo is
//! offline and has no registry. Layouts and constants were printed by compiling against /usr/include/SDL3
//! (docs/t-host-window-input-contract.md, "FFI pin"); `layout_matches_the_pin` re-checks sizes at run time.
use crate::window_input::{Effect, InputEvent, InputState, MAX_PADS, PumpResult, RawEvent};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::Mutex;

const SDL_INIT_VIDEO: u32 = 0x20;
const SDL_INIT_GAMEPAD: u32 = 0x2000;
const SDL_WINDOW_RESIZABLE: u64 = 0x20;
const SDL_WINDOW_VULKAN: u64 = 0x1000_0000;

const EVENT_QUIT: u32 = 0x100;
const EVENT_WINDOW_SHOWN: u32 = 0x202;
const EVENT_WINDOW_EXPOSED: u32 = 0x204;
const EVENT_WINDOW_RESIZED: u32 = 0x206;
const EVENT_WINDOW_PIXEL_SIZE_CHANGED: u32 = 0x207;
const EVENT_WINDOW_RESTORED: u32 = 0x20B;
const EVENT_WINDOW_FOCUS_LOST: u32 = 0x20F;
const EVENT_WINDOW_CLOSE_REQUESTED: u32 = 0x210;
const EVENT_KEY_DOWN: u32 = 0x300;
const EVENT_KEY_UP: u32 = 0x301;
const EVENT_GAMEPAD_AXIS_MOTION: u32 = 0x650;
const EVENT_GAMEPAD_BUTTON_DOWN: u32 = 0x651;
const EVENT_GAMEPAD_BUTTON_UP: u32 = 0x652;
const EVENT_GAMEPAD_ADDED: u32 = 0x653;
const EVENT_GAMEPAD_REMOVED: u32 = 0x654;

/// `SDL_Event` is a 128 byte union (checked against the headers); fields are read at the offsets printed from them.
#[repr(C, align(8))]
struct SdlEvent {
    bytes: [u8; 128],
}

impl SdlEvent {
    fn u32_at(&self, at: usize) -> u32 {
        u32::from_ne_bytes(self.bytes[at..at + 4].try_into().unwrap())
    }

    /// Decode into the SDL-free event (offsets: key 28, repeat 37, which 16, button/axis 20, axis value 24).
    fn decode(&self) -> RawEvent {
        match self.u32_at(0) {
            EVENT_QUIT => RawEvent::Quit,
            EVENT_WINDOW_CLOSE_REQUESTED => RawEvent::CloseRequested,
            EVENT_WINDOW_EXPOSED
            | EVENT_WINDOW_RESIZED
            | EVENT_WINDOW_PIXEL_SIZE_CHANGED
            | EVENT_WINDOW_SHOWN
            | EVENT_WINDOW_RESTORED => RawEvent::Redraw,
            EVENT_WINDOW_FOCUS_LOST => RawEvent::FocusLost,
            EVENT_KEY_DOWN => RawEvent::KeyDown {
                key: self.u32_at(28),
                repeat: self.bytes[37] != 0,
            },
            EVENT_KEY_UP => RawEvent::KeyUp {
                key: self.u32_at(28),
                repeat: self.bytes[37] != 0,
            },
            EVENT_GAMEPAD_ADDED => RawEvent::GamepadAdded(self.u32_at(16)),
            EVENT_GAMEPAD_REMOVED => RawEvent::GamepadRemoved(self.u32_at(16)),
            EVENT_GAMEPAD_BUTTON_DOWN => {
                RawEvent::button_down(Some(self.u32_at(16)), self.bytes[20])
            }
            EVENT_GAMEPAD_BUTTON_UP => RawEvent::button_up(Some(self.u32_at(16)), self.bytes[20]),
            EVENT_GAMEPAD_AXIS_MOTION => RawEvent::axis(
                Some(self.u32_at(16)),
                self.bytes[20],
                i16::from_ne_bytes([self.bytes[24], self.bytes[25]]),
            ),
            _ => RawEvent::Other,
        }
    }
}

unsafe extern "C" {
    fn SDL_Init(flags: u32) -> bool;
    fn SDL_InitSubSystem(flags: u32) -> bool;
    fn SDL_QuitSubSystem(flags: u32);
    fn SDL_GetError() -> *const c_char;
    fn SDL_CreateWindow(title: *const c_char, w: c_int, h: c_int, flags: u64) -> *mut c_void;
    fn SDL_DestroyWindow(window: *mut c_void);
    fn SDL_PollEvent(event: *mut SdlEvent) -> bool;
    fn SDL_GetGamepads(count: *mut c_int) -> *mut u32;
    fn SDL_OpenGamepad(id: u32) -> *mut c_void;
    fn SDL_CloseGamepad(pad: *mut c_void);
    fn SDL_GetGamepadID(pad: *mut c_void) -> u32;
    fn SDL_free(memory: *mut c_void);
}

fn sdl_error() -> String {
    // SAFETY: SDL_GetError returns a valid NUL-terminated string owned by SDL (thread local).
    unsafe { CStr::from_ptr(SDL_GetError()) }
        .to_string_lossy()
        .into_owned()
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

/// Everything behind the opaque `tsfp_wi *`: the event state plus the open SDL pads.
pub struct WindowInput {
    state: InputState,
    pads: [*mut c_void; MAX_PADS],
    pads_init: bool,
}

// SAFETY: the C side serialises access (sink mutex) and the pads are only touched on the presenter thread.
unsafe impl Send for WindowInput {}

pub struct Handle(Mutex<WindowInput>);

impl WindowInput {
    fn open_pad(&mut self, id: u32) {
        // SAFETY: pads hold pointers returned by SDL_OpenGamepad and not yet closed.
        let known = self
            .pads
            .iter()
            .any(|&pad| !pad.is_null() && unsafe { SDL_GetGamepadID(pad) } == id);
        if known {
            return; // ADDED may repeat
        }
        if let Some(slot) = self.pads.iter_mut().find(|pad| pad.is_null()) {
            // SAFETY: SDL gamepad subsystem was initialised by tsfp_wi_enable before pad events are enabled.
            *slot = unsafe { SDL_OpenGamepad(id) };
        }
    }

    fn close_pad(&mut self, id: u32) {
        for pad in &mut self.pads {
            // SAFETY: as in open_pad.
            if !pad.is_null() && unsafe { SDL_GetGamepadID(*pad) } == id {
                unsafe { SDL_CloseGamepad(*pad) };
                *pad = std::ptr::null_mut();
            }
        }
    }

    fn apply(&mut self, effect: Effect) {
        if let Some(id) = effect.open_pad {
            self.open_pad(id);
        }
        if let Some(id) = effect.close_pad {
            self.close_pad(id);
        }
    }
}

/// Create the window (SDL video init included). NULL with `error` filled on failure.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_window_create(
    title: *const c_char,
    vulkan: bool,
    error: *mut c_char,
    error_size: usize,
) -> *mut c_void {
    // SAFETY: SDL FFI; `title` is NUL-terminated or NULL.
    unsafe {
        if !SDL_Init(SDL_INIT_VIDEO) {
            copy_error(
                &format!("--present window: SDL_Init failed: {}", sdl_error()),
                error,
                error_size,
            );
            return std::ptr::null_mut();
        }
        let title = if title.is_null() {
            c"tsfp".as_ptr()
        } else {
            title
        };
        let flags = SDL_WINDOW_RESIZABLE | if vulkan { SDL_WINDOW_VULKAN } else { 0 };
        let window = SDL_CreateWindow(title, 640, 480, flags);
        if window.is_null() {
            let what = if vulkan {
                "--gpu-live: could not create the Vulkan window"
            } else {
                "--present window: could not create the window"
            };
            copy_error(&format!("{what}: {}", sdl_error()), error, error_size);
            SDL_QuitSubSystem(SDL_INIT_VIDEO);
        }
        window
    }
}

/// Destroy the window and release the video subsystem reference (the teardown pair of create).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_window_destroy(window: *mut c_void) {
    // SAFETY: `window` came from tsfp_wi_window_create; the presenter thread owns it.
    unsafe {
        if !window.is_null() {
            SDL_DestroyWindow(window);
        }
        SDL_QuitSubSystem(SDL_INIT_VIDEO);
    }
}

/// Allocate the input state behind the opaque `tsfp_wi *` (no SDL call).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_new() -> *mut Handle {
    Box::into_raw(Box::new(Handle(Mutex::new(WindowInput {
        state: InputState::new(),
        pads: [std::ptr::null_mut(); MAX_PADS],
        pads_init: false,
    }))))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_free(handle: *mut Handle) {
    if !handle.is_null() {
        // SAFETY: allocated by tsfp_wi_new, freed once by the sink owner after tsfp_wi_shutdown.
        drop(unsafe { Box::from_raw(handle) });
    }
}

fn lock(handle: *mut Handle) -> Option<std::sync::MutexGuard<'static, WindowInput>> {
    // SAFETY: the handle outlives every call (the sink frees it last).
    unsafe { handle.as_ref() }.map(|handle| {
        handle
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    })
}

/// Drain the SDL event queue (presenter thread only). Bit 0 of the result: a close was requested, `*cause` is the
/// first cause (static text); bit 1: redraw requested.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_pump(handle: *mut Handle, cause: *mut *const c_char) -> u32 {
    let Some(mut input) = lock(handle) else {
        return 0;
    };
    let mut result = PumpResult::default();
    let mut event = SdlEvent { bytes: [0; 128] };
    // SAFETY: SDL fills the 128 byte event.
    while unsafe { SDL_PollEvent(&mut event) } {
        let effect = input.state.handle(event.decode(), &mut result);
        input.apply(effect);
    }
    if !cause.is_null() {
        // SAFETY: the caller passes a writable pointer slot.
        unsafe { *cause = result.close_cause.map_or(std::ptr::null(), CStr::as_ptr) };
    }
    u32::from(result.close_cause.is_some()) | u32::from(result.redraw) << 1
}

/// Turn the keyboard and or gamepad events on; a gamepad also initialises the SDL gamepad subsystem and opens
/// the attached pads (presenter thread). False with `error` on an SDL failure.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_enable(
    handle: *mut Handle,
    keyboard: bool,
    gamepad: bool,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    let Some(mut input) = lock(handle) else {
        return false;
    };
    if gamepad && !input.state.gamepad {
        // SAFETY: SDL FFI on the presenter thread.
        unsafe {
            if !SDL_InitSubSystem(SDL_INIT_GAMEPAD) {
                copy_error(
                    &format!("window input: SDL gamepad init failed: {}", sdl_error()),
                    error,
                    error_size,
                );
                return false;
            }
            input.pads_init = true;
            let mut count: c_int = 0;
            let ids = SDL_GetGamepads(&mut count);
            for index in 0..if ids.is_null() { 0 } else { count as usize } {
                input.open_pad(*ids.add(index));
            }
            SDL_free(ids.cast());
        }
    }
    input.state.keyboard |= keyboard;
    input.state.gamepad |= gamepad;
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
pub extern "C" fn tsfp_wi_next(handle: *mut Handle, out: *mut CInputEvent) -> bool {
    let Some(mut input) = lock(handle) else {
        return false;
    };
    match input.state.next() {
        Some(event) if !out.is_null() => {
            // SAFETY: `out` is a writable present_input_event.
            unsafe { out.write(event.into()) };
            true
        }
        _ => false,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_dropped(handle: *mut Handle) -> u64 {
    lock(handle).map_or(0, |input| input.state.dropped())
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_pads_open(handle: *mut Handle) -> usize {
    lock(handle).map_or(0, |input| {
        input.pads.iter().filter(|pad| !pad.is_null()).count()
    })
}

/// Close every pad and release the gamepad subsystem (presenter thread, before the window is destroyed).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_wi_shutdown(handle: *mut Handle) {
    let Some(mut input) = lock(handle) else {
        return;
    };
    for pad in &mut input.pads {
        if !pad.is_null() {
            // SAFETY: opened by open_pad.
            unsafe { SDL_CloseGamepad(*pad) };
            *pad = std::ptr::null_mut();
        }
    }
    if input.pads_init {
        // SAFETY: balanced with the InitSubSystem in tsfp_wi_enable.
        unsafe { SDL_QuitSubSystem(SDL_INIT_GAMEPAD) };
        input.pads_init = false;
    }
}

/// rustc codegens only what a bin crate's `main` reaches, so the C-called exports are referenced from here
/// (called once from main) to keep them in the object file; the C host objects then resolve against them.
pub fn keep_exports() {
    let table: [*const (); 11] = [
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
        std::ptr::null(),
    ];
    std::hint::black_box(table);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_the_pin() {
        assert_eq!(std::mem::size_of::<SdlEvent>(), 128);
        assert_eq!(std::mem::size_of::<CInputEvent>(), 24);
    }

    #[test]
    fn decode_reads_the_pinned_offsets() {
        let mut event = SdlEvent { bytes: [0; 128] };
        event.bytes[0..4].copy_from_slice(&EVENT_KEY_DOWN.to_ne_bytes());
        event.bytes[28..32].copy_from_slice(&97u32.to_ne_bytes());
        event.bytes[37] = 1;
        assert_eq!(
            event.decode(),
            RawEvent::KeyDown {
                key: 97,
                repeat: true
            }
        );
        event.bytes[0..4].copy_from_slice(&EVENT_GAMEPAD_AXIS_MOTION.to_ne_bytes());
        event.bytes[20] = 1;
        event.bytes[24..26].copy_from_slice(&(-300i16).to_ne_bytes());
        assert_eq!(
            event.decode(),
            RawEvent::Axis {
                pad: Some(0),
                axis: 1,
                value: -300
            }
        );
    }

    #[test]
    fn decode_keeps_gamepad_runtime_id_for_each_input_event() {
        let mut event = SdlEvent { bytes: [0; 128] };
        event.bytes[0..4].copy_from_slice(&EVENT_GAMEPAD_BUTTON_DOWN.to_ne_bytes());
        event.bytes[16..20].copy_from_slice(&42u32.to_ne_bytes());
        event.bytes[20] = 3;
        assert_eq!(
            event.decode(),
            RawEvent::ButtonDown {
                pad: Some(42),
                button: 3
            }
        );

        event.bytes[0..4].copy_from_slice(&EVENT_GAMEPAD_AXIS_MOTION.to_ne_bytes());
        event.bytes[20] = 2;
        event.bytes[24..26].copy_from_slice(&(-300i16).to_ne_bytes());
        assert_eq!(
            event.decode(),
            RawEvent::Axis {
                pad: Some(42),
                axis: 2,
                value: -300
            }
        );
    }
}
