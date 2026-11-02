// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI over the Rust scripted and live pad sources (T3926 slice 9, `src/input/xinput_source.c`).
//! C keeps the file reads; the parsed script and the live last-state/refused counter live here.

use std::ffi::c_char;

use tsfp_input_devices::script::{Entry, Live, PadValues, Script, parse_live_line};

/// Mirrors `xinput_pad_state` in `src/input/xinput_hle.h`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TsfpPadState {
    pub packet_number: u32,
    pub digital_buttons: u16,
    pub analog: [u8; 8],
    pub thumbs: [i16; 4],
}

/// Mirrors `xinput_script_entry`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TsfpScriptEntry {
    pub frames: u32,
    pub state: TsfpPadState,
}

impl From<PadValues> for TsfpPadState {
    fn from(v: PadValues) -> Self {
        Self {
            packet_number: 0,
            digital_buttons: v.digital,
            analog: v.analog,
            thumbs: v.thumbs,
        }
    }
}

impl From<TsfpPadState> for PadValues {
    fn from(s: TsfpPadState) -> Self {
        Self {
            digital: s.digital_buttons,
            analog: s.analog,
            thumbs: s.thumbs,
        }
    }
}

/// Copy `message` into `error` (NUL terminated, truncated like `snprintf`).
unsafe fn write_error(error: *mut c_char, size: usize, message: &str) {
    if error.is_null() || size == 0 {
        return;
    }
    let n = message.len().min(size - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(message.as_ptr().cast::<c_char>(), error, n);
        *error.add(n) = 0;
    }
}

/// Parse script text. Null on a refusal, with the message in `error`.
///
/// # Safety
/// `text` has `len` readable bytes; `error` is null or has `error_size` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_script_parse(
    text: *const u8,
    len: usize,
    error: *mut c_char,
    error_size: usize,
) -> *mut Script {
    unsafe { write_error(error, error_size, "") };
    let text = if text.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(text, len) }
    };
    match Script::parse(text) {
        Ok(script) => Box::into_raw(Box::new(script)),
        Err(message) => {
            unsafe { write_error(error, error_size, &message) };
            std::ptr::null_mut()
        }
    }
}

/// # Safety
/// `entries` has `count` readable entries.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_script_from_entries(
    entries: *const TsfpScriptEntry,
    count: usize,
) -> *mut Script {
    let entries = if entries.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(entries, count) }
    };
    let entries: Vec<Entry> = entries
        .iter()
        .map(|e| Entry {
            frames: e.frames,
            values: e.state.into(),
        })
        .collect();
    Box::into_raw(Box::new(Script::from_entries(&entries)))
}

/// # Safety
/// `script` is null or from this module and not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_script_free(script: *mut Script) {
    if !script.is_null() {
        drop(unsafe { Box::from_raw(script) });
    }
}

/// # Safety
/// `script` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_script_entry_count(script: *const Script) -> usize {
    unsafe { script.as_ref() }.map_or(0, Script::entry_count)
}

/// # Safety
/// `script` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_script_total_frames(script: *const Script) -> u64 {
    unsafe { script.as_ref() }.map_or(0, Script::total_frames)
}

/// # Safety
/// `script` is null or live; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_script_state_at(
    script: *const Script,
    poll_index: u64,
    out: *mut TsfpPadState,
) {
    let values =
        unsafe { script.as_ref() }.map_or_else(PadValues::default, |s| s.state_at(poll_index));
    unsafe { *out = values.into() };
}

/// One live line: false when a token is refused (`out` untouched).
///
/// # Safety
/// `text` has `len` readable bytes; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_live_parse_line(
    text: *const u8,
    len: usize,
    out: *mut TsfpPadState,
) -> bool {
    let text = if text.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(text, len) }
    };
    parse_live_line(text).is_some_and(|values| {
        unsafe { *out = values.into() };
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_live_create() -> *mut Live {
    Box::into_raw(Box::new(Live::default()))
}

/// # Safety
/// `live` is null or from [`tsfp_input_live_create`] and not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_live_free(live: *mut Live) {
    if !live.is_null() {
        drop(unsafe { Box::from_raw(live) });
    }
}

/// Feed one read of the live file (`got < 0`: the read failed) and fill the values to install.
///
/// # Safety
/// `live` is live; `buffer` has `got` readable bytes when `got > 0`; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_live_poll(
    live: *mut Live,
    buffer: *const u8,
    got: isize,
    out: *mut TsfpPadState,
) {
    let Some(live) = (unsafe { live.as_mut() }) else {
        return;
    };
    let read = (got >= 0).then(|| {
        if got == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(buffer, got as usize) }
        }
    });
    unsafe { *out = live.poll(read).into() };
}

/// # Safety
/// `live` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_live_refused(live: *const Live) -> u64 {
    unsafe { live.as_ref() }.map_or(0, Live::refused)
}
