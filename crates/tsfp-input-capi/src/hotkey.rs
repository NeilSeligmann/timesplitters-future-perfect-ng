// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI over the Rust-owned host hotkey engine (T3926 slice 7, `src/input/xinput_hotkey.c`).
//! C keeps the `--hotkey` grammar and passes each chord with its accepted key aliases; the event
//! and tick calls deliver the engine's notices to the C fire and leak callbacks afterwards.

use std::ffi::{CStr, c_char, c_void};

use tsfp_input_devices::hotkey::{ChordSpec, Device, Hotkeys, KEYS_MAX, NAME_MAX, Notice};

const ALIASES: usize = 2;
const LABEL_MAX: usize = 32;

#[repr(C)]
pub struct TsfpHotkeySpec {
    pub device: u32,
    pub hold_ms: u32,
    pub label: [c_char; LABEL_MAX],
    pub key_count: u32,
    pub accept_count: [u32; KEYS_MAX],
    pub accept: [[[c_char; NAME_MAX]; ALIASES]; KEYS_MAX],
}

type FireFn = Option<unsafe extern "C" fn(u32, *const c_char, u64, *mut c_void)>;
type LeakFn = Option<unsafe extern "C" fn(u32, *const c_char, u64, bool, *mut c_void)>;

fn string(chars: &[c_char]) -> String {
    let bytes: Vec<u8> = chars
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| *c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A chord table. Null when the specs are refused or the directory cannot be made.
///
/// # Safety
/// `specs` points to `count` specs; `dir` is null or NUL terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_hotkeys_create(
    specs: *const TsfpHotkeySpec,
    count: u32,
    dir: *const c_char,
) -> *mut Hotkeys {
    if specs.is_null() {
        return std::ptr::null_mut();
    }
    let specs = unsafe { std::slice::from_raw_parts(specs, count as usize) };
    let mut chords = Vec::with_capacity(specs.len());
    for spec in specs {
        let Some(device) = Device::from_raw(spec.device) else {
            return std::ptr::null_mut();
        };
        let keys = (spec.key_count as usize).min(KEYS_MAX);
        let accept = (0..keys)
            .map(|k| {
                let n = (spec.accept_count[k] as usize).min(ALIASES);
                spec.accept[k][..n]
                    .iter()
                    .map(|name| string(name))
                    .collect()
            })
            .collect();
        chords.push(ChordSpec {
            device,
            hold_ms: spec.hold_ms,
            label: string(&spec.label),
            accept,
        });
    }
    let dir = (!dir.is_null()).then(|| {
        unsafe { CStr::from_ptr(dir) }
            .to_string_lossy()
            .into_owned()
    });
    match Hotkeys::new(chords, dir.as_deref()) {
        Some(hotkeys) => Box::into_raw(Box::new(hotkeys)),
        None => std::ptr::null_mut(),
    }
}

/// # Safety
/// `hotkeys` is null or from [`tsfp_input_hotkeys_create`], and is not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_hotkeys_free(hotkeys: *mut Hotkeys) {
    if !hotkeys.is_null() {
        drop(unsafe { Box::from_raw(hotkeys) });
    }
}

fn deliver(notices: Vec<Notice>, fire: FireFn, leak: LeakFn, user: *mut c_void) {
    for notice in notices {
        match notice {
            Notice::Fire {
                number,
                label,
                poll,
            } => {
                if let Some(fire) = fire {
                    let label = std::ffi::CString::new(label.replace('\0', "")).unwrap_or_default();
                    unsafe { fire(number, label.as_ptr(), poll, user) };
                }
            }
            Notice::Leak {
                device,
                name,
                poll,
                begin,
            } => {
                if let Some(leak) = leak {
                    let name = std::ffi::CString::new(name.replace('\0', "")).unwrap_or_default();
                    unsafe { leak(device as u32, name.as_ptr(), poll, begin, user) };
                }
            }
        }
    }
}

/// Returns true when the event is to be forwarded; the callbacks run before it returns and share `user`.
///
/// # Safety
/// `hotkeys` is live; `name` is null or NUL terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_hotkeys_event(
    hotkeys: *mut Hotkeys,
    device: u32,
    name: *const c_char,
    down: bool,
    now_ms: u64,
    poll: u64,
    fire: FireFn,
    leak: LeakFn,
    user: *mut c_void,
) -> bool {
    let (Some(hotkeys), Some(device)) = (unsafe { hotkeys.as_mut() }, Device::from_raw(device))
    else {
        return true;
    };
    if name.is_null() {
        return true;
    }
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    let (forward, notices) = hotkeys.event(device, &name, down, now_ms, poll);
    deliver(notices, fire, leak, user);
    forward
}

/// # Safety
/// `hotkeys` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_hotkeys_tick(
    hotkeys: *mut Hotkeys,
    now_ms: u64,
    poll: u64,
    fire: FireFn,
    leak: LeakFn,
    user: *mut c_void,
) {
    if let Some(hotkeys) = unsafe { hotkeys.as_mut() } {
        deliver(hotkeys.tick(now_ms, poll), fire, leak, user);
    }
}

/// Counter `which`: 0 fired, 1 write failures, 2 swallowed total, 3 swallow overflow, 4 leak overflow.
///
/// # Safety
/// `hotkeys` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_hotkeys_counter(hotkeys: *const Hotkeys, which: u32) -> u32 {
    let Some(h) = (unsafe { hotkeys.as_ref() }) else {
        return 0;
    };
    match which {
        0 => h.fired(),
        1 => h.write_failures(),
        2 => h.swallowed_total(),
        3 => h.swallow_overflow(),
        4 => h.leak_overflow(),
        _ => 0,
    }
}

/// # Safety
/// `hotkeys` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_hotkeys_needs_device(
    hotkeys: *const Hotkeys,
    device: u32,
) -> bool {
    match (unsafe { hotkeys.as_ref() }, Device::from_raw(device)) {
        (Some(h), Some(d)) => h.needs_device(d),
        _ => false,
    }
}
