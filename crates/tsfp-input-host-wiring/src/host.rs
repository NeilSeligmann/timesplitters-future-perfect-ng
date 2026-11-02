// SPDX-License-Identifier: GPL-3.0-or-later
//! The `tsfp_input_host_*` C entry points (T3975). Same calls, order and messages as the C unit.
use std::ffi::{c_char, c_void};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::{HotkeyPoll, PadConfig, PadStep, ffi, pad_plan};

/// Wiring state owned here: how often the host was reset and what the last pad configuration did.
struct WiringState {
    resets: AtomicU32,
    pad_steps: AtomicU32,
    handlers: AtomicU64,
}

static STATE: WiringState = WiringState {
    resets: AtomicU32::new(0),
    pad_steps: AtomicU32::new(0),
    handlers: AtomicU64::new(0),
};

pub type PollObserver = unsafe extern "C" fn(u32, u64, *mut c_void);
pub type PreInstallHook = unsafe extern "C" fn(u64, *const c_void, *mut c_void);

/// Parse the stable host source tags shared with `xinput_host_source` (script=0, keyboard=1, gamepad=2).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_pad_source_kind_parse(
    text: *const c_char,
    kind: *mut u32,
) -> bool {
    if text.is_null() || kind.is_null() {
        return false;
    }
    let bytes = unsafe { std::ffi::CStr::from_ptr(text) }.to_bytes();
    let parsed = match bytes {
        b"script" => 0,
        b"keyboard" => 1,
        b"gamepad" => 2,
        _ => return false,
    };
    unsafe { *kind = parsed };
    true
}

#[repr(C)]
pub struct HostEvent {
    poll: u64,
    device: u32,
    action: u32,
    code: u32,
    value: i32,
}

/// Convert one SDL input event into the XInput host-source event consumed by the source engine.
/// # Safety
/// `name` is null or a valid NUL-terminated string and `out` points to writable event storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_translate_sdl_event(
    kind: u32,
    name: *const c_char,
    raw: i32,
    value: i32,
    poll: u64,
    out: *mut HostEvent,
) {
    const KEY_DOWN: u32 = 1;
    const KEY_UP: u32 = 2;
    const PAD_DOWN: u32 = 3;
    const PAD_UP: u32 = 4;
    const KEYBOARD: u32 = 1;
    const GAMEPAD: u32 = 2;
    const DOWN: u32 = 1;
    const UP: u32 = 2;
    const AXIS: u32 = 3;
    let keyboard = kind == KEY_DOWN || kind == KEY_UP;
    let action = match kind {
        KEY_DOWN | PAD_DOWN => DOWN,
        KEY_UP | PAD_UP => UP,
        _ => AXIS,
    };
    let code = if name.is_null() {
        u32::MAX
    } else if action == AXIS {
        unsafe { ffi::xinput_host_axis_of(name) }
    } else {
        unsafe { ffi::xinput_host_code_of(if keyboard { KEYBOARD } else { GAMEPAD }, name) }
    };
    unsafe {
        out.write(HostEvent {
            poll,
            device: if keyboard { KEYBOARD } else { GAMEPAD },
            action,
            code: if code == u32::MAX {
                0x1000_0000 + (raw as u32 & 0x00ff_ffff)
            } else {
                code
            },
            value,
        });
    }
}

#[derive(Clone, Copy)]
struct PollObserverState(Option<(PollObserver, usize)>);

static POLL_OBSERVER: Mutex<PollObserverState> = Mutex::new(PollObserverState(None));
static PRE_INSTALL_HOOK: Mutex<Option<(PreInstallHook, usize)>> = Mutex::new(None);

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_set_pre_install_hook(
    hook: Option<PreInstallHook>,
    user: *mut c_void,
) {
    unsafe { ffi::xinput_source_set_pre_install_hook(hook, user) };
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_store_pre_install_hook(
    hook: Option<PreInstallHook>,
    user: *mut c_void,
) {
    *PRE_INSTALL_HOOK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
        hook.map(|callback| (callback, user as usize));
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_pre_install(poll: u64, state: *const c_void) {
    let hook = PRE_INSTALL_HOOK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((callback, user)) = *hook {
        unsafe { callback(poll, state, user as *mut c_void) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_set_poll_observer(
    observer: Option<PollObserver>,
    user: *mut c_void,
) {
    *POLL_OBSERVER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
        PollObserverState(observer.map(|callback| (callback, user as usize)));
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_notify_poll_observer(port: u32) {
    let observer = POLL_OBSERVER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .0;
    if let Some((callback, user)) = observer {
        unsafe { callback(port, ffi::tsfp_input_poll_port(port), user as *mut c_void) };
    }
}

#[repr(C)]
pub struct PadConfigC {
    synthetic_pad: bool,
    controllers: bool,
    pad_ports: bool,
    pad_ports_router_present: bool,
    synthetic_pad_remove_after_polls: u32,
    pad_script: *const c_char,
    pad_script_live: bool,
}

#[repr(C)]
pub struct RegisterConfigC {
    synthetic_pad: bool,
    controllers: bool,
    pad_ports: bool,
    device_fatal: Option<unsafe extern "C" fn(u32, *const c_char)>,
    ports_sample: *const c_void,
    ports_bind: Option<
        unsafe extern "C" fn(
            Option<unsafe extern "C" fn(u32) -> bool>,
            Option<unsafe extern "C" fn(u32) -> bool>,
        ) -> bool,
    >,
}

macro_rules! out {
    ($fmt:literal $(, $arg:expr)*) => { unsafe { ffi::printf($fmt.as_ptr() $(, $arg)*) } };
}
macro_rules! err {
    ($fmt:literal $(, $arg:expr)*) => { unsafe { ffi::fprintf(ffi::stderr, $fmt.as_ptr() $(, $arg)*) } };
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_reset_all() {
    STATE.resets.fetch_add(1, Ordering::Relaxed);
    unsafe {
        ffi::xinput_hle_init();
        ffi::xinput_devices_reset();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_set_log(printer: ffi::LogFn) {
    unsafe { ffi::xinput_hle_set_log(printer) }
}

/// # Safety
/// `config` points to a valid `tsfp_input_host_pad_config`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_configure_pads(config: *const PadConfigC) -> bool {
    let c = unsafe { &*config };
    let plan = PadConfig {
        synthetic_pad: c.synthetic_pad,
        controllers: c.controllers,
        pad_ports: c.pad_ports,
        pad_ports_router_present: c.pad_ports_router_present,
        remove_after_polls: c.synthetic_pad_remove_after_polls,
        has_script: !c.pad_script.is_null(),
        script_live: c.pad_script_live,
    };
    let steps = match pad_plan(&plan) {
        Ok(steps) => steps,
        Err(_) => {
            err!(c"--pad-ports: needs the Bevy shell's input router (crates/tsfp-input-slots, T3771)\n");
            return false;
        }
    };
    let path = c.pad_script;
    let mut error = [0 as c_char; 160];
    for (index, step) in steps.iter().enumerate() {
        STATE.pad_steps.store(index as u32 + 1, Ordering::Relaxed);
        match *step {
            PadStep::SyntheticPad(on) => {
                unsafe { ffi::xinput_devices_enable_synthetic_pad(on) };
            }
            PadStep::AttachSyntheticPad => {
                unsafe { ffi::xinput_hle_attach_synthetic_pad(0) };
                out!(c"xinput policy  --synthetic-pad FABRICATED: port 0 reports one gamepad inserted, the title's open path is answered, buttons and sticks at rest (T717)\n");
            }
            PadStep::Multiport => unsafe { ffi::xinput_devices_enable_multiport(true) },
            PadStep::PadPortsMultiport => {
                unsafe { ffi::xinput_devices_enable_multiport(true) };
                out!(c"xinput policy  --pad-ports: four-port adapter, port 0 the --synthetic-pad queue pad, ports 1-3 follow the input router slots 1-3 (Rust-owned presence and state, T3771)\n");
            }
            PadStep::RemoveAfterPolls(polls) => {
                unsafe { ffi::xinput_pad_remove_after_polls(polls) };
                if polls != 0 {
                    out!(c"xinput policy  --synthetic-pad-remove-after-polls FABRICATED: the pad is unplugged after XInputGetState call %u (T731)\n", polls);
                }
            }
            PadStep::SourceReset => unsafe { ffi::xinput_source_reset() },
            PadStep::LoadLiveScript => {
                let live = unsafe { ffi::xinput_live_open(path, error.as_mut_ptr(), error.len()) };
                if live.is_null() {
                    err!(c"--pad-script-live %s: %s\n", path, error.as_ptr());
                    return false;
                }
                unsafe { ffi::xinput_live_install(live) };
                out!(c"xinput policy  --pad-script-live FABRICATED: one line of pad tokens re-read at every XInputGetState from %s, rest while empty (T1222)\n", path);
            }
            PadStep::LoadScript => {
                let script =
                    unsafe { ffi::xinput_script_load(path, error.as_mut_ptr(), error.len()) };
                if script.is_null() {
                    err!(c"--pad-script %s: %s\n", path, error.as_ptr());
                    return false;
                }
                unsafe { ffi::xinput_script_install(script) };
                out!(c"xinput policy  --pad-script FABRICATED: %zu entries, %llu frames of scripted pad state from %s, one per XInputGetState, rest afterwards (T707)\n",
                    unsafe { ffi::xinput_script_entry_count(script) },
                    unsafe { ffi::xinput_script_total_frames(script) },
                    path);
            }
        }
    }
    true
}

/// # Safety
/// `config` points to a valid `tsfp_input_host_register_config`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_register_all(config: *const RegisterConfigC) {
    let c = unsafe { &*config };
    unsafe {
        ffi::xinput_devices_set_fatal(c.device_fatal);
        ffi::xinput_devices_register();
        ffi::xvoice_media_register();
        ffi::mu_register();
        if c.pad_ports {
            ffi::xinput_source_install_ports(c.ports_sample, std::ptr::null_mut());
            if let Some(bind) = c.ports_bind {
                bind(
                    Some(ffi::xinput_pad_connect),
                    Some(ffi::xinput_pad_disconnect),
                );
            }
        }
        if c.synthetic_pad || c.controllers {
            ffi::xinput_devices_register_pad();
        }
        let count = ffi::xinput_hle_implemented_count();
        STATE.handlers.store(count as u64, Ordering::Relaxed);
        out!(c"xinput handlers %zu registered\n", count);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_register_report_surface() {
    unsafe {
        ffi::xinput_devices_register();
        ffi::xvoice_media_register();
        ffi::mu_register();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_poll_count(port: u32) -> u64 {
    unsafe { ffi::tsfp_input_poll_port(port) }
}

/// Return the baseline reader's recorded poll count for the Rust-owned host report.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_replay_total_polls() -> u64 {
    unsafe { ffi::xinput_replay_total_polls() }
}

/// Return replay budget metadata to the host through the Rust input contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_replay_budgets(out: *mut crate::ReplayBudgets) -> bool {
    if out.is_null() {
        return false;
    }
    unsafe { ffi::xinput_replay_budgets(out) }
}

/// Reserve guest port zero for the explicitly selected script/keyboard source.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_reserve_port_zero() {
    unsafe { ffi::xinput_pad_connect(0) };
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_timing_enable(enabled: bool) {
    unsafe { ffi::xinput_source_t3615_timing_enable(enabled) }
}

/// # Safety
/// `out` is a valid C `FILE *`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_timing_print(out: *mut c_void) {
    unsafe { ffi::xinput_source_t3615_timing_print(out) }
}

/// # Safety
/// `hotkeys` is a valid opaque XInput hotkey handle, or null for no hotkeys.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_hotkey_feed_create(hotkeys: *mut c_void) -> *mut c_void {
    if hotkeys.is_null() {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(HotkeyFeed {
        hotkeys,
        poll: HotkeyPoll::default(),
    }))
    .cast()
}

struct HotkeyFeed {
    hotkeys: *mut c_void,
    poll: HotkeyPoll,
}

/// # Safety
/// `feed` is a handle returned by `tsfp_input_host_hotkey_feed_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_hotkey_feed_free(feed: *mut c_void) {
    if !feed.is_null() {
        drop(unsafe { Box::from_raw(feed.cast::<HotkeyFeed>()) });
    }
}

/// # Safety
/// `feed` is a handle returned by `tsfp_input_host_hotkey_feed_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_hotkey_feed_tick(
    feed: *mut c_void,
    now_ms: u64,
    poll: u64,
) {
    if let Some(feed) = unsafe { feed.cast::<HotkeyFeed>().as_mut() }
        && feed.poll.begin(poll)
    {
        unsafe { ffi::xinput_hotkeys_tick(feed.hotkeys, now_ms, poll) };
    }
}

/// `kind` is PRESENT_INPUT_KEY_DOWN/UP/PAD_DOWN/UP (1 through 4).
/// # Safety
/// `feed` is a handle returned by `tsfp_input_host_hotkey_feed_create`; `name` is null or a NUL-terminated name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_hotkey_feed_event(
    feed: *mut c_void,
    kind: u32,
    name: *const c_char,
    now_ms: u64,
    poll: u64,
) -> bool {
    let Some(feed) = (unsafe { feed.cast::<HotkeyFeed>().as_mut() }) else {
        return true;
    };
    let (device, down) = match kind {
        1 => (1, true),
        2 => (1, false),
        3 => (2, true),
        4 => (2, false),
        _ => return true,
    };
    unsafe { ffi::xinput_hotkeys_event(feed.hotkeys, device, name, down, now_ms, poll) }
}

/// # Safety
/// `hotkeys` is null or a valid opaque XInput hotkey handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_hotkey_needs_device(
    hotkeys: *const c_void,
    device: u32,
) -> bool {
    !hotkeys.is_null() && unsafe { ffi::xinput_hotkeys_needs_device(hotkeys, device) }
}
