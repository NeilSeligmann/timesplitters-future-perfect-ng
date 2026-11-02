// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI of `xinput_hotkey.h`. The chord table (`HostHotkeys`) owns the engine, the counters and the fire and leak
//! callbacks; C only holds the opaque pointer.
use std::ffi::{CStr, CString, c_char, c_void};

use tsfp_input_devices::hotkey::{ChordSpec, Device, Hotkeys, KEYS_MAX, NAME_MAX, Notice};

use crate::grammar::{self, LABEL_MAX, Spec};

pub type FireFn = Option<unsafe extern "C" fn(u32, *const c_char, u64, *mut c_void)>;
pub type LeakFn = Option<unsafe extern "C" fn(u32, *const c_char, u64, bool, *mut c_void)>;
const MAX_SPECS: u32 = 8;

/// `xinput_hotkey_spec`.
#[repr(C)]
pub struct HotkeySpec {
    pub device: u32,
    pub key_count: u32,
    pub keys: [[c_char; NAME_MAX]; KEYS_MAX],
    pub hold_ms: u32,
    pub label: [c_char; LABEL_MAX],
}

/// `xinput_hotkeys`.
pub struct HostHotkeys {
    engine: Hotkeys,
    fire: FireFn,
    user: *mut c_void,
    leak: LeakFn,
    leak_user: *mut c_void,
}

fn string(chars: &[c_char]) -> String {
    let bytes: Vec<u8> = chars
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| *c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn fill(target: &mut [c_char], text: &str) {
    for (slot, byte) in target
        .iter_mut()
        .zip(text.bytes().chain(std::iter::repeat(0)))
    {
        *slot = byte as c_char;
    }
}

/// `snprintf(buffer, size, "%s", message)`.
fn report(buffer: *mut c_char, size: usize, message: &str) {
    if buffer.is_null() || size == 0 {
        return;
    }
    let kept = message.len().min(size - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(message.as_ptr(), buffer.cast::<u8>(), kept);
        *buffer.add(kept) = 0;
    }
}

fn c_string(text: String) -> CString {
    CString::new(text.replace('\0', "")).unwrap_or_default()
}

/// # Safety
/// `label` is null or NUL terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_hotkey_action_of(label: *const c_char) -> u32 {
    if label.is_null() {
        return 0;
    }
    grammar::action_of(&unsafe { CStr::from_ptr(label) }.to_string_lossy())
}

/// # Safety
/// `text` is null or NUL terminated, `out` is null or a spec, `error` is null or `error_size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_hotkey_parse(
    text: *const c_char,
    out: *mut HotkeySpec,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    if text.is_null() || out.is_null() {
        report(error, error_size, "no hotkey");
        return false;
    }
    let parsed = grammar::parse(&unsafe { CStr::from_ptr(text) }.to_string_lossy());
    match parsed {
        Ok(Spec {
            device,
            keys,
            hold_ms,
            label,
        }) => {
            let out = unsafe { &mut *out };
            out.device = device as u32;
            out.key_count = keys.len() as u32;
            out.keys = [[0; NAME_MAX]; KEYS_MAX];
            for (slot, key) in out.keys.iter_mut().zip(&keys) {
                fill(slot, key);
            }
            out.hold_ms = hold_ms;
            fill(&mut out.label, &label);
            true
        }
        Err(message) => {
            report(error, error_size, &message);
            false
        }
    }
}

fn chord_of(spec: &HotkeySpec) -> Result<ChordSpec, String> {
    let device = Device::from_raw(spec.device).ok_or("hotkeys: a spec needs 2..5 keys")?;
    if spec.key_count < 2 || spec.key_count as usize > KEYS_MAX {
        return Err("hotkeys: a spec needs 2..5 keys".into());
    }
    let mut accept = Vec::new();
    for key in &spec.keys[..spec.key_count as usize] {
        let name = string(key);
        let aliases = grammar::accept_of(device, &name);
        if aliases.is_empty() {
            return Err(format!("hotkeys: unknown key '{name}'"));
        }
        accept.push(aliases);
    }
    Ok(ChordSpec {
        device,
        hold_ms: spec.hold_ms,
        label: string(&spec.label),
        accept,
    })
}

/// # Safety
/// `specs` points to `count` specs, `dir` is null or NUL terminated, `error` is null or `error_size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_hotkeys_create(
    specs: *const HotkeySpec,
    count: u32,
    dir: *const c_char,
    fire: FireFn,
    user: *mut c_void,
    error: *mut c_char,
    error_size: usize,
) -> *mut HostHotkeys {
    if specs.is_null() || count == 0 || count > MAX_SPECS {
        report(error, error_size, "hotkeys: need 1..4 specs");
        return std::ptr::null_mut();
    }
    let specs = unsafe { std::slice::from_raw_parts(specs, count as usize) };
    let chords: Result<Vec<ChordSpec>, String> = specs.iter().map(chord_of).collect();
    let chords = match chords {
        Ok(chords) => chords,
        Err(message) => {
            report(error, error_size, &message);
            return std::ptr::null_mut();
        }
    };
    let dir = (!dir.is_null()).then(|| {
        unsafe { CStr::from_ptr(dir) }
            .to_string_lossy()
            .into_owned()
    });
    match Hotkeys::new(chords, dir.as_deref()) {
        Some(engine) => Box::into_raw(Box::new(HostHotkeys {
            engine,
            fire,
            user,
            leak: None,
            leak_user: std::ptr::null_mut(),
        })),
        None => {
            report(
                error,
                error_size,
                &format!(
                    "hotkeys: cannot create the directory '{}'",
                    dir.unwrap_or_default()
                ),
            );
            std::ptr::null_mut()
        }
    }
}

/// # Safety
/// `hotkeys` is null or from [`xinput_hotkeys_create`], and is not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_hotkeys_free(hotkeys: *mut HostHotkeys) {
    if !hotkeys.is_null() {
        drop(unsafe { Box::from_raw(hotkeys) });
    }
}

/// # Safety
/// `hotkeys` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_hotkeys_set_leak_observer(
    hotkeys: *mut HostHotkeys,
    leak: LeakFn,
    user: *mut c_void,
) {
    if let Some(table) = unsafe { hotkeys.as_mut() } {
        table.leak = leak;
        table.leak_user = user;
    }
}

impl HostHotkeys {
    fn deliver(&self, notices: Vec<Notice>) {
        for notice in notices {
            match notice {
                Notice::Fire {
                    number,
                    label,
                    poll,
                } => {
                    if let Some(fire) = self.fire {
                        let label = c_string(label);
                        unsafe { fire(number, label.as_ptr(), poll, self.user) };
                    }
                }
                Notice::Leak {
                    device,
                    name,
                    poll,
                    begin,
                } => {
                    if let Some(leak) = self.leak {
                        let name = c_string(name);
                        unsafe { leak(device as u32, name.as_ptr(), poll, begin, self.leak_user) };
                    }
                }
            }
        }
    }
}

/// True when the event is to be forwarded; the callbacks run before it returns.
///
/// # Safety
/// `hotkeys` is null or live; `name` is null or NUL terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_hotkeys_event(
    hotkeys: *mut HostHotkeys,
    device: u32,
    name: *const c_char,
    down: bool,
    now_ms: u64,
    poll: u64,
) -> bool {
    let (Some(table), Some(device)) = (unsafe { hotkeys.as_mut() }, Device::from_raw(device))
    else {
        return true;
    };
    if name.is_null() {
        return true;
    }
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    let (forward, notices) = table.engine.event(device, &name, down, now_ms, poll);
    table.deliver(notices);
    forward
}

/// # Safety
/// `hotkeys` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_hotkeys_tick(hotkeys: *mut HostHotkeys, now_ms: u64, poll: u64) {
    if let Some(table) = unsafe { hotkeys.as_mut() } {
        let notices = table.engine.tick(now_ms, poll);
        table.deliver(notices);
    }
}

macro_rules! counter {
    ($name:ident, $method:ident) => {
        /// # Safety
        /// `hotkeys` is null or live.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(hotkeys: *const HostHotkeys) -> u32 {
            unsafe { hotkeys.as_ref() }.map_or(0, |table| table.engine.$method())
        }
    };
}
counter!(xinput_hotkeys_fired, fired);
counter!(xinput_hotkeys_write_failures, write_failures);
counter!(xinput_hotkeys_swallowed_total, swallowed_total);
counter!(xinput_hotkeys_swallow_overflow, swallow_overflow);
counter!(xinput_hotkeys_leak_overflow, leak_overflow);

/// # Safety
/// `hotkeys` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_hotkeys_needs_device(
    hotkeys: *const HostHotkeys,
    device: u32,
) -> bool {
    match (unsafe { hotkeys.as_ref() }, Device::from_raw(device)) {
        (Some(table), Some(device)) => table.engine.needs_device(device),
        _ => false,
    }
}
