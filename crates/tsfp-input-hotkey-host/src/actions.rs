// SPDX-License-Identifier: GPL-3.0-or-later
//! `src/host/hotkey_actions.c` in Rust: what the host does with a fired hotkey label and with a leaked chord key. The
//! The action state is allocated and owned by Rust on the default input path.
use std::ffi::{CStr, c_char, c_int, c_void};
use std::io::Write;

use crate::grammar::action_of;

type MarkFn = Option<unsafe extern "C" fn(*mut c_void) -> bool>;
type StopFn = Option<unsafe extern "C" fn(*mut c_void, *const c_char)>;
type ShotFn = Option<unsafe extern "C" fn(*mut c_void, u32, *const c_char, u64) -> bool>;

#[repr(C)]
pub struct Hooks {
    mark: MarkFn,
    stop: StopFn,
    user: *mut c_void,
    shot: ShotFn,
    mu: MarkFn,
}

/// Rust-owned `hotkey_actions` state.
#[repr(C)]
pub struct Actions {
    hooks: Hooks,
    source_device: u32,
    marks_queued: u32,
    marks_refused: u32,
    stops_requested: u32,
    shots_taken: u32,
    shots_refused: u32,
    mu_toggles: u32,
    mu_refused: u32,
    leaks_trimmed: u32,
    leaks_ignored: u32,
}

/// `xinput_record_effect`.
#[repr(C)]
struct Effect {
    digital_mask: u16,
    analog: c_int,
    stick: c_int,
}

unsafe extern "C" {
    fn xinput_host_key_effect(
        device: u32,
        name: *const c_char,
        digital: *mut u16,
        analog: *mut c_int,
        stick: *mut c_int,
    ) -> bool;
    fn xinput_record_suppress_begin(effect: *const Effect, from_poll: u64);
    fn xinput_record_suppress_end(effect: *const Effect, at_poll: u64);
}

fn say(message: std::fmt::Arguments) {
    let _ = writeln!(std::io::stderr(), "{message}");
}

/// # Safety
/// `hooks` is null or a `hotkey_action_hooks`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hotkey_actions_create(
    source_device: u32,
    hooks: *const Hooks,
) -> *mut c_void {
    let mut actions = Box::new(Actions {
        hooks: Hooks {
            mark: None,
            stop: None,
            user: std::ptr::null_mut(),
            shot: None,
            mu: None,
        },
        source_device,
        marks_queued: 0,
        marks_refused: 0,
        stops_requested: 0,
        shots_taken: 0,
        shots_refused: 0,
        mu_toggles: 0,
        mu_refused: 0,
        leaks_trimmed: 0,
        leaks_ignored: 0,
    });
    if let Some(hooks) = unsafe { hooks.as_ref() } {
        actions.hooks = Hooks {
            mark: hooks.mark,
            stop: hooks.stop,
            user: hooks.user,
            shot: hooks.shot,
            mu: hooks.mu,
        };
    }
    Box::into_raw(actions).cast()
}

/// # Safety
/// `actions` must be null or a live pointer returned by [`hotkey_actions_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hotkey_actions_destroy(actions: *mut c_void) {
    if !actions.is_null() {
        drop(unsafe { Box::from_raw(actions.cast::<Actions>()) });
    }
}

/// The `xinput_hotkey_fire_fn`: `context` is the `hotkey_actions`.
///
/// # Safety
/// `label` is null or NUL terminated; `context` is null or a live `hotkey_actions`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hotkey_actions_fire(
    number: u32,
    label: *const c_char,
    poll: u64,
    context: *mut c_void,
) {
    let Some(actions) = (unsafe { context.cast::<Actions>().as_mut() }) else {
        return;
    };
    let text = if label.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(label) }
            .to_string_lossy()
            .into_owned()
    };
    let user = actions.hooks.user;
    match if label.is_null() { 0 } else { action_of(&text) } {
        1 => {
            if actions.hooks.mark.is_some_and(|mark| unsafe { mark(user) }) {
                actions.marks_queued += 1;
            } else {
                actions.marks_refused += 1;
                say(format_args!(
                    "hotkey (T1632): 'mark' #{number} at host poll {poll} ignored, no --record-input recording is open"
                ));
            }
        }
        2 => {
            actions.stops_requested += 1;
            say(format_args!(
                "hotkey (T1632): 'stop' #{number} at host poll {poll}, ending the run cleanly"
            ));
            if let Some(stop) = actions.hooks.stop {
                unsafe { stop(user, c"stop hotkey (T1632)".as_ptr()) };
            }
        }
        3 => {
            if actions
                .hooks
                .shot
                .is_some_and(|shot| unsafe { shot(user, number, label, poll) })
            {
                actions.shots_taken += 1;
            } else {
                actions.shots_refused += 1;
            }
        }
        4 => {
            if actions.hooks.mu.is_some_and(|mu| unsafe { mu(user) }) {
                actions.mu_toggles += 1;
            } else {
                actions.mu_refused += 1;
                say(format_args!(
                    "hotkey (T2225): 'mu' #{number} at host poll {poll} refused"
                ));
            }
        }
        _ => {}
    }
}

/// The `xinput_hotkey_leak_fn`: `context` is the `hotkey_actions`.
///
/// # Safety
/// `name` is null or NUL terminated; `context` is null or a live `hotkey_actions`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hotkey_actions_leak(
    device: u32,
    name: *const c_char,
    poll: u64,
    begin: bool,
    context: *mut c_void,
) {
    let Some(actions) = (unsafe { context.cast::<Actions>().as_mut() }) else {
        return;
    };
    let key_device = if device == 2 { 2 } else { 1 };
    let mut effect = Effect {
        digital_mask: 0,
        analog: -1,
        stick: -1,
    };
    let mapped = key_device == actions.source_device
        && unsafe {
            xinput_host_key_effect(
                key_device,
                name,
                &mut effect.digital_mask,
                &mut effect.analog,
                &mut effect.stick,
            )
        };
    if !mapped {
        if begin {
            actions.leaks_ignored += 1;
        }
        return;
    }
    if begin {
        actions.leaks_trimmed += 1;
        unsafe { xinput_record_suppress_begin(&effect, poll) };
    } else {
        unsafe { xinput_record_suppress_end(&effect, poll) };
    }
}
