// SPDX-License-Identifier: GPL-3.0-or-later
//! T3917: the `tsfp_audio_host_hle_*` questions of `src/audio/tsfp_audio_host_lifecycle.h`, answered from the Rust-owned
//! measured surface and registry. The XDK dispatcher and the registration report ask by exact address; no C entry table
//! is read. T3918: the per-call dispatch is here too. Call counts, report-once flags and the log labels live in
//! `hle_surface`; the C entry table is only the `TSFP_RUST_AUDIO_HOST=OFF` fallback.

use std::ffi::c_void;

use crate::hle_surface::{self, contains_address};
use crate::movie_device_hle_ffi::{registration, tsfp_audio_hle_call_rust};

const SECTION_BEGIN: u32 = 0x0040_67C0;
const SECTION_END: u32 = 0x0041_2D14;

#[cfg(not(test))]
unsafe extern "C" {
    /// Writes one finished line through the HLE log hook the host configured.
    fn dsound_hle_log_line(line: *const std::ffi::c_char);
}

#[cfg(not(test))]
fn log_line(line: String) {
    if let Ok(line) = std::ffi::CString::new(line) {
        // SAFETY: the pointer is a live NUL-terminated string for this synchronous call.
        unsafe { dsound_hle_log_line(line.as_ptr()) };
    }
}

#[cfg(test)]
pub(crate) static TEST_LOG: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

#[cfg(test)]
fn log_line(line: String) {
    TEST_LOG.lock().unwrap().push(line);
}

/// One DirectSound call: measured lookup, call count, registration selection, handler, and the once-per-function stub
/// report. Returns the guest result. Unknown targets report every time and answer zero.
///
/// # Safety
/// `context` is the live synchronous call frame the registered handler expects (or null for a stub).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_host_hle_call(address: u32, context: *mut c_void) -> u32 {
    let (mut index, mut calls, mut sites, mut result) = (0, 0, 0, 0);
    // SAFETY: the four outputs are live locals; the caller owns `context` for this call.
    let status = unsafe {
        tsfp_audio_hle_call_rust(
            address,
            context,
            &mut index,
            &mut calls,
            &mut sites,
            &mut result,
        )
    };
    match status {
        5 | 6 => {}
        3 => {
            let name = hle_surface::row_label(index as usize).and_then(|(name, _)| name);
            log_line(format!(
                "dsound: {address:#010x} {} is not implemented ({sites} measured call site{}), returning {result:#x}\n",
                name.unwrap_or("<unnamed in .XTLID>"),
                if sites == 1 { "" } else { "s" },
            ));
        }
        4 => log_line(format!(
            "dsound: Rust handler missing for registered target {address:#010x}\n"
        )),
        hle_surface::LOOKUP_UNKNOWN_INSIDE => log_line(format!(
            "dsound: call to UNKNOWN target {address:#010x} -- inside the DSOUND section but absent from our {}-entry surface table\n",
            hle_surface::tsfp_audio_hle_surface_count()
        )),
        hle_surface::LOOKUP_UNKNOWN_OUTSIDE => log_line(format!(
            "dsound: call to UNKNOWN target {address:#010x} -- OUTSIDE the DSOUND section ({SECTION_BEGIN:#010x}..{SECTION_END:#010x}); this is not a DirectSound address\n"
        )),
        other => {
            log_line(format!(
                "dsound: Rust measured-surface lookup unavailable for target {address:#010x} (status {other})\n"
            ));
            std::process::abort();
        }
    }
    result
}

/// Calls of one measured entry this run (zero for an unknown address).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_hle_call_count(address: u32) -> u64 {
    hle_surface::index_of(address).map_or(0, hle_surface::call_count)
}

/// The address is a measured DSOUND entry.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_hle_known(address: u32) -> bool {
    contains_address(address)
}

/// A handler is registered for the address.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_hle_implemented(address: u32) -> bool {
    registration(address).is_some_and(|(implemented, _)| implemented)
}

/// Implemented exactly when a handler is held. An unregistered measured entry is consistent; an unknown address is not.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_hle_registration_consistent(address: u32) -> bool {
    contains_address(address)
        && registration(address).is_none_or(|(implemented, has_handler)| implemented == has_handler)
}

/// The measured stream, buffer and listener startup surface, where a stub must stop the run instead of answering.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_host_hle_requires_implementation(address: u32) -> bool {
    matches!(
        address,
        0x0040_7B14
            | 0x0040_7B19
            | 0x0040_7B1E
            | 0x0040_7B23
            | 0x0040_7B28
            | 0x0040_85CF
            | 0x0040_85D4
            | 0x0040_85D9
            | 0x0040_85F1
            | 0x0040_8609
            | 0x0040_8632
            | 0x0040_8637
            | 0x0040_8C2D
            | 0x0040_967C
            | 0x0040_7ABC
            | 0x0040_7AA4
            | 0x0040_7AF8
            | 0x0040_8556
            | 0x0040_7A64
            | 0x0040_7AD8
            | 0x0040_8532
            | 0x0040_858B
            | 0x0040_7A80
            | 0x0040_84F2
            | 0x0040_850E
            | 0x0040_85AF
            | 0x0040_8C0D
            | 0x0040_93C8
            | 0x0040_93EC
            | 0x0040_945A
            | 0x0040_9410
    )
}
