// SPDX-License-Identifier: GPL-3.0-or-later
//! T3959: the host-facing half of the SILENT DirectSound device. Owns what `crates/tsfp-audio-conformance/oracle/dsound_device.c` still held in the
//! Rust-host build: the fatal and create-note hooks, the DirectSoundCreate HLE handler and its frame decoding, the
//! checked reset, the movie Release frame snapshot and the refusal paths. Device state, leases and references stay in
//! `tsfp_audio::lifecycle::Device` behind [`crate::device_ffi`].

use std::ffi::{CString, c_char, c_void};
use std::sync::Mutex;

use crate::device_ffi::{
    CCreateOutcome, CReason, tsfp_audio_device_create, tsfp_audio_device_movie_calls_enabled,
    tsfp_audio_device_release_movie, tsfp_audio_device_reset_checked,
};
use crate::movie_device_hle_ffi::tsfp_audio_hle_register_handler_rust;

const CREATE_ENTRY: u32 = 0x0040_9635;
const CREATE_CALLER: u32 = 0x0002_79D5;
const MOVIE_CREATE_CALLER: u32 = 0x0044_570B;
const RELEASE_ENTRY: u32 = 0x0040_6A8A;

type FatalFn = unsafe extern "C" fn(u32, *const c_char);
type NoteFn = unsafe extern "C" fn(u32, u32) -> bool;

unsafe extern "C" {
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn dsound_hle_log_line(line: *const c_char);
    fn dsound_hle_codec_ready() -> bool;
    fn kernel_guest_read_u32(address: u32, out: *mut u32) -> bool;
    fn kernel_frame_arg(frame: *const c_void, index: u32, out: *mut u32) -> bool;
}

static FATAL: Mutex<Option<FatalFn>> = Mutex::new(None);
static NOTE: Mutex<Option<NoteFn>> = Mutex::new(None);

fn log_line(line: &str) {
    if let Ok(line) = CString::new(line) {
        // SAFETY: a live NUL-terminated string for this synchronous call.
        unsafe { dsound_hle_log_line(line.as_ptr()) };
    }
}

/// `dsound_device_set_fatal`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_device_set_fatal(handler: Option<FatalFn>) {
    *FATAL.lock().unwrap() = handler;
}

/// `dsound_device_set_create_note`: the PCM epoch observer of a successful Create.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_device_set_create_note(note: Option<NoteFn>) {
    *NOTE.lock().unwrap() = note;
}

fn refuse_at(entry: u32, message: &str) -> ! {
    log_line(&format!(
        "dsound SILENT facade: {entry:#x} refused: {message}\n"
    ));
    let fatal = *FATAL.lock().unwrap();
    if let (Some(fatal), Ok(message)) = (fatal, CString::new(message)) {
        // SAFETY: the host fatal hook takes the entry and a NUL-terminated reason.
        unsafe { fatal(entry, message.as_ptr()) };
    }
    std::process::abort()
}

fn refuse_reason_at(entry: u32, reason: &CReason) -> ! {
    refuse_at(entry, reason.as_str())
}

/// `dsound_device_reset_checked`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_device_reset_checked() -> bool {
    tsfp_audio_device_reset_checked()
}

/// `dsound_device_reset`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_device_reset() {
    let _ = tsfp_audio_device_reset_checked();
}

/// `dsound_device_set_movie_calls`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_device_set_movie_calls(enabled: bool) {
    crate::device_ffi::tsfp_audio_device_set_movie_calls(enabled);
}

/// `dsound_device_movie_calls_enabled`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_device_movie_calls_enabled() -> bool {
    tsfp_audio_device_movie_calls_enabled()
}

/// `dsound_device_movie_references`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_device_movie_references() -> u32 {
    crate::device_ffi::tsfp_audio_device_movie_references()
}

fn create_device(guid: u32, output: u32, outer: u32, movie: bool) -> u32 {
    let mut outcome = std::mem::MaybeUninit::<CCreateOutcome>::uninit();
    // SAFETY: the codec query is a plain C call; the outcome slot is written by the callee.
    let codec_ready = unsafe { dsound_hle_codec_ready() };
    let note = *NOTE.lock().unwrap();
    // SAFETY: `outcome` is a live writable slot.
    let accepted = unsafe {
        tsfp_audio_device_create(
            guid,
            output,
            outer,
            movie,
            codec_ready,
            note,
            outcome.as_mut_ptr(),
        )
    };
    // SAFETY: initialised by the call above on both paths.
    let outcome = unsafe { outcome.assume_init() };
    if !accepted {
        refuse_reason_at(CREATE_ENTRY, &outcome.reason);
    }
    if outcome.announce {
        log_line(
            "dsound: explicit SILENT public-device facade; omitted B4/540/APU, IRQ/DPC/timer, silent-buffer and guest \
             critical-section internals; no audio output or DSP acknowledgement; nested fields remain unsupported\n",
        );
    }
    outcome.status
}

unsafe extern "C" fn create_handler(context: *mut c_void) -> u32 {
    // SAFETY: the dispatcher passes the live `kernel_call_frame`, whose first field is the guest stack pointer.
    let stack = if context.is_null() {
        0
    } else {
        unsafe { *context.cast::<u32>() }
    };
    let mut caller = 0;
    // SAFETY: `caller` is a live out slot.
    if context.is_null() || !unsafe { kernel_guest_read_u32(stack, &mut caller) } {
        refuse_at(CREATE_ENTRY, "unreadable Create frame");
    }
    let startup = caller == CREATE_CALLER;
    if !startup && !(caller == MOVIE_CREATE_CALLER && tsfp_audio_device_movie_calls_enabled()) {
        refuse_at(
            CREATE_ENTRY,
            "only measured startup caller return 0x000279D5 is recovered",
        );
    }
    let mut arguments = [0u32; 3];
    for (index, slot) in arguments.iter_mut().enumerate() {
        // SAFETY: the frame is live and `slot` is a live out slot.
        if !unsafe { kernel_frame_arg(context, index as u32, slot) } {
            refuse_at(CREATE_ENTRY, "unreadable Create argument");
        }
    }
    create_device(arguments[0], arguments[1], arguments[2], !startup)
}

/// `dsound_device_register`: the measured DirectSoundCreate entry.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_device_register() -> usize {
    if !tsfp_audio_hle_register_handler_rust(CREATE_ENTRY, create_handler) {
        return 0;
    }
    // SAFETY: the compatibility telemetry row exists once the HLE table is initialised.
    usize::from(unsafe { dsound_hle_register_rust(CREATE_ENTRY) })
}

/// `dsound_device_release_movie`: the movie-owned device Release; a refusal is the fatal stop.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_device_release_movie(interface: u32) -> u32 {
    let mut result = 0;
    let mut reason = CReason::none();
    // SAFETY: both out slots are live locals.
    if !unsafe { tsfp_audio_device_release_movie(interface, &mut result, &mut reason) } {
        refuse_reason_at(RELEASE_ENTRY, &reason);
    }
    result
}

/// The movie Release call frame: host pid and the guest stack bounds. False without a frame.
///
/// # Safety
/// `frame` is null or the live `kernel_call_frame` of the running call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_movie_release_frame_snapshot(
    frame: *mut c_void,
    host_pid_out: *mut i32,
    stack_address_out: *mut u32,
    stack_limit_out: *mut u32,
) -> bool {
    if frame.is_null()
        || host_pid_out.is_null()
        || stack_address_out.is_null()
        || stack_limit_out.is_null()
    {
        return false;
    }
    let words = frame.cast::<u32>();
    // SAFETY: the frame starts with stack_ptr and stack_limit; the outputs are non-null and writable.
    unsafe {
        *host_pid_out = i32::try_from(std::process::id()).unwrap_or(0);
        *stack_address_out = words.read();
        *stack_limit_out = words.add(1).read();
    }
    true
}

/// The movie Release refusal: 0 is a wrong caller, anything else an unreadable argument.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_release_refuse(reason: u32) -> ! {
    if reason == 0 {
        refuse_at(
            RELEASE_ENTRY,
            "only the measured movie caller return 0x00445726 of device Release is recovered",
        )
    }
    refuse_at(RELEASE_ENTRY, "unreadable Release argument")
}

/// `dsound_device_register_movie`: the movie-owned device Release entry.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_device_register_movie() -> usize {
    crate::movie_device_hle_ffi::tsfp_audio_movie_device_register_release_rust()
}

type OwnedInterfaceFn = unsafe extern "C" fn(u32, *mut c_void, *mut u32);

struct InterfaceTrampoline {
    callback: OwnedInterfaceFn,
    userdata: *mut c_void,
}

unsafe extern "C" fn interface_trampoline(
    words: *const u32,
    context: *mut c_void,
    result: *mut u32,
) {
    // SAFETY: `context` is the live trampoline of the call below; `words` holds the owned internal address.
    unsafe {
        let trampoline = &*context.cast::<InterfaceTrampoline>();
        (trampoline.callback)(*words, trampoline.userdata, result);
    }
}

/// `dsound_device_with_owned_interface`: the callback gets the owned internal address under the device lock.
///
/// # Safety
/// `userdata` and `result` are valid for the synchronous callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_device_with_owned_interface(
    interface: u32,
    callback: Option<OwnedInterfaceFn>,
    userdata: *mut c_void,
    result: *mut u32,
) -> bool {
    let Some(callback) = callback else {
        return false;
    };
    let mut trampoline = InterfaceTrampoline { callback, userdata };
    // SAFETY: the trampoline outlives the synchronous call.
    unsafe {
        crate::device_ffi::tsfp_audio_device_with_owned_interface(
            interface,
            Some(interface_trampoline),
            (&raw mut trampoline).cast(),
            result,
        )
    }
}

/// `dsound_device_create`: the public DirectSoundCreate body (non-movie caller).
#[unsafe(no_mangle)]
pub extern "C" fn dsound_device_create(guid: u32, output: u32, outer: u32) -> u32 {
    create_device(guid, output, outer, false)
}

/// `dsound_device_with_owned_identity`: the callback gets `{device_heap, internal_address}` under the device lock.
///
/// # Safety
/// `userdata` and `result` are valid for the synchronous callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_device_with_owned_identity(
    interface: u32,
    callback: Option<crate::device_ffi::OwnedCallback>,
    userdata: *mut c_void,
    result: *mut u32,
) -> bool {
    // SAFETY: forwarded contract.
    unsafe {
        crate::device_ffi::tsfp_audio_device_with_owned_identity(
            interface, callback, userdata, result,
        )
    }
}
