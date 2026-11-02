// SPDX-License-Identifier: GPL-3.0-or-later
//! Presenter-thread C boundary. SDL initialization remains the caller's duty.

use crate::{
    PORTS, Selector,
    profile::{self, ControllerProfile, XInputPadState},
    provider::{Provider, unmapped_diagnostic},
    sdl::{Sample, SdlBackend},
};
use std::ffi::{CStr, c_char};

pub struct PadProvider {
    provider: Provider<SdlBackend>,
    error: String,
}

impl PadProvider {
    fn record_errors(&mut self, errors: Vec<String>) -> bool {
        let ok = errors.is_empty();
        if let Some(error) = errors.into_iter().last() {
            // controller_sdl.c retains the most recent error across pumps.
            self.error = error;
        }
        ok
    }
}

/// C representation uses bytes rather than Rust booleans for the button array.
#[repr(C)]
pub struct PadSample {
    pub buttons: [u8; 15],
    pub axes: [i16; 6],
}

/// Apply the selected port's validated controller profile in Rust.
///
/// # Safety
/// All pointers must address the corresponding fixed-size C structures and arrays.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_sdl_pad_profile_sample(
    profile: *const ControllerProfile,
    buttons: *const u8,
    axes: *const i16,
    out: *mut XInputPadState,
) -> bool {
    let (Some(profile), Some(buttons), Some(axes), Some(out)) = (
        unsafe { profile.as_ref() },
        unsafe { buttons.cast::<[u8; 15]>().as_ref() },
        unsafe { axes.cast::<[i16; 6]>().as_ref() },
        unsafe { out.as_mut() },
    ) else {
        return false;
    };
    *out = profile::apply(profile, buttons, axes);
    true
}

// Copy like snprintf: return the length before truncation and terminate when
// capacity is nonzero. The caller supplies a valid buffer of that capacity.
unsafe fn copy_text(text: &[u8], buffer: *mut c_char, capacity: usize) -> usize {
    if capacity != 0 && !buffer.is_null() {
        let count = text.len().min(capacity - 1);
        unsafe {
            std::ptr::copy_nonoverlapping(text.as_ptr(), buffer.cast(), count);
            *buffer.add(count) = 0;
        }
    }
    text.len()
}

/// Create and use the provider only on the SDL presenter thread. The caller
/// must initialize SDL's gamepad subsystem before pumping it.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_sdl_pad_create() -> *mut PadProvider {
    Box::into_raw(Box::new(PadProvider {
        provider: Provider::new(
            SdlBackend::default(),
            std::array::from_fn(|_| Selector::default()),
        ),
        error: String::new(),
    }))
}

/// # Safety
/// `provider` must be a live pointer returned by create, on its presenter thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_sdl_pad_destroy(provider: *mut PadProvider) {
    if !provider.is_null() {
        unsafe { drop(Box::from_raw(provider)) };
    }
}

/// # Safety
/// `provider` must be live and `guid` must point to a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_sdl_pad_assign(
    provider: *mut PadProvider,
    port: usize,
    enabled: bool,
    guid: *const c_char,
    occurrence: i32,
) -> bool {
    let Some(provider) = (unsafe { provider.as_mut() }) else {
        return false;
    };
    if guid.is_null() || port >= PORTS {
        return false;
    }
    let guid = unsafe { CStr::from_ptr(guid) }
        .to_string_lossy()
        .into_owned();
    provider.provider.assign(
        port,
        Selector {
            enabled,
            guid,
            occurrence: usize::try_from(occurrence).ok(),
        },
    )
}

/// # Safety
/// `provider` must be live and called on its presenter thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_sdl_pad_pump(provider: *mut PadProvider) -> bool {
    let Some(provider) = (unsafe { provider.as_mut() }) else {
        return false;
    };
    let errors = provider.provider.pump();
    provider.record_errors(errors)
}

/// # Safety
/// `provider` must be live and `out` must be writable for one sample.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_sdl_pad_sample(
    provider: *const PadProvider,
    port: usize,
    out: *mut PadSample,
) -> u32 {
    let (Some(provider), Some(out)) = (unsafe { provider.as_ref() }, unsafe { out.as_mut() })
    else {
        return 0;
    };
    let (Some(id), Some(Sample { buttons, axes })) =
        (provider.provider.id(port), provider.provider.sample(port))
    else {
        return 0;
    };
    out.buttons = buttons.map(u8::from);
    out.axes = *axes;
    id
}

/// # Safety
/// `provider` must be live and called on its presenter thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_sdl_pad_feedback(
    provider: *mut PadProvider,
    port: usize,
    left: u16,
    right: u16,
) -> bool {
    unsafe { provider.as_mut() }.is_some_and(|p| p.provider.feedback(port, left, right))
}

/// # Safety
/// `provider` must be live. Non-null buffers must be writable for their capacities.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_sdl_pad_details(
    provider: *const PadProvider,
    port: usize,
    name: *mut c_char,
    name_capacity: usize,
    guid: *mut c_char,
    guid_capacity: usize,
    rumble: *mut bool,
) -> bool {
    let Some(provider) = (unsafe { provider.as_ref() }) else {
        return false;
    };
    let Some((pad_name, pad_guid, capable)) = provider.provider.details(port) else {
        return false;
    };
    unsafe {
        copy_text(pad_name.as_bytes(), name, name_capacity);
        copy_text(pad_guid.as_bytes(), guid, guid_capacity);
        if let Some(out) = rumble.as_mut() {
            *out = capable;
        }
    }
    true
}

/// Return the diagnostic length, excluding the terminating NUL.
///
/// # Safety
/// `provider` must be live and a non-null buffer must be writable for `capacity` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_sdl_pad_unmapped(
    provider: *const PadProvider,
    buffer: *mut c_char,
    capacity: usize,
) -> usize {
    let Some(provider) = (unsafe { provider.as_ref() }) else {
        return 0;
    };
    let diagnostic = unmapped_diagnostic(provider.provider.unmapped());
    unsafe { copy_text(&diagnostic[..diagnostic.len() - 1], buffer, capacity) }
}

/// Return the most recent pump error length, excluding the terminating NUL.
///
/// # Safety
/// `provider` must be live and a non-null buffer writable for `capacity` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_sdl_pad_error(
    provider: *const PadProvider,
    buffer: *mut c_char,
    capacity: usize,
) -> usize {
    let Some(provider) = (unsafe { provider.as_ref() }) else {
        return 0;
    };
    unsafe { copy_text(provider.error.as_bytes(), buffer, capacity) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_pump_error_survives_success_and_is_bounded_at_boundary() {
        let mut provider = PadProvider {
            provider: Provider::new(
                SdlBackend::default(),
                std::array::from_fn(|_| Selector::default()),
            ),
            error: String::new(),
        };
        assert!(!provider.record_errors(vec!["first".into(), "last error".into()]));
        assert!(provider.record_errors(Vec::new()));
        let mut buffer = [0i8; 5];
        let length = unsafe { tsfp_sdl_pad_error(&provider, buffer.as_mut_ptr(), buffer.len()) };
        assert_eq!(length, 10);
        assert_eq!(buffer, [108, 97, 115, 116, 0]);
    }

    #[test]
    fn null_boundary_arguments_are_rejected() {
        unsafe {
            tsfp_sdl_pad_destroy(std::ptr::null_mut());
            assert!(!tsfp_sdl_pad_pump(std::ptr::null_mut()));
            assert!(!tsfp_sdl_pad_assign(
                std::ptr::null_mut(),
                0,
                true,
                std::ptr::null(),
                -1
            ));
            assert!(!tsfp_sdl_pad_feedback(std::ptr::null_mut(), 0, 1, 2));
            assert_eq!(
                tsfp_sdl_pad_sample(std::ptr::null(), 0, std::ptr::null_mut()),
                0
            );
        }
    }

    #[test]
    fn text_copy_terminates_and_reports_untruncated_length() {
        let mut buffer = [42_i8; 4];
        unsafe {
            assert_eq!(copy_text(b"abcde", buffer.as_mut_ptr(), buffer.len()), 5);
            assert_eq!(buffer, [b'a' as i8, b'b' as i8, b'c' as i8, 0]);
            assert_eq!(copy_text(b"abcde", std::ptr::null_mut(), 0), 5);
        }
    }
}
