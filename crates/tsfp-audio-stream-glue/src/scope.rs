// SPDX-License-Identifier: GPL-3.0-or-later
//! `src/audio/dsound_stream_scope.c`: the measured startup scope snapshot, validated by the Rust scope model.
use crate::ffi::*;
use crate::host_pid;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_stream_scope_validate(
    descriptor: *const u8,
    descriptor_bytes: usize,
    format: *const u8,
    format_bytes: usize,
    output: *mut Scope,
) -> bool {
    // SAFETY: the pointers are the caller's snapshots, validated for length by the Rust model.
    unsafe {
        tsfp_audio_stream_scope_validate_rust(
            descriptor,
            descriptor_bytes,
            format,
            format_bytes,
            output,
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_stream_scope_snapshot(descriptor: u32, output: *mut Scope) -> bool {
    if output.is_null() {
        return false;
    }
    // SAFETY: `output` is caller storage for one scope record.
    unsafe { tsfp_audio_stream_scope_snapshot_rust(host_pid(), descriptor, output) }
}
