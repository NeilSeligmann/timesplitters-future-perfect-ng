//! C ABI for the buffer creation scope (`dsound_buffer_scope.h`): the Rust model in `tsfp_audio::scope`.

use crate::device_ffi::HostWorld;
use tsfp_audio::adapter::GuestMemory;
use tsfp_audio::scope::{BufferScope, buffer_scope_snapshot, buffer_scope_validate};

/// C layout of `dsound_buffer_scope`.
#[repr(C)]
pub struct CBufferScope {
    descriptor_address: u32,
    format_address: u32,
    flags: u32,
    buffer_bytes: u32,
    sample_rate: u32,
    average_bytes_per_second: u32,
    channels: u16,
    block_align: u16,
    descriptor: [u8; 24],
    format: [u8; 20],
}

fn fill(out: &mut CBufferScope, scope: &BufferScope, descriptor: [u8; 24]) {
    *out = CBufferScope {
        descriptor_address: scope.descriptor_address,
        format_address: scope.format_address,
        flags: scope.flags,
        buffer_bytes: scope.buffer_bytes,
        sample_rate: scope.sample_rate,
        average_bytes_per_second: scope.average_bytes_per_second,
        channels: scope.channels,
        block_align: scope.block_align,
        descriptor,
        format: scope.format,
    };
}

/// `dsound_buffer_scope_validate`.
///
/// # Safety
/// The byte pointers must be valid for their lengths and `output` for one write (all may be null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_buffer_scope_validate(
    descriptor: *const u8,
    descriptor_bytes: usize,
    format: *const u8,
    format_bytes: usize,
    output: *mut CBufferScope,
) -> bool {
    if descriptor.is_null() || format.is_null() || output.is_null() {
        return false;
    }
    // SAFETY: lengths and pointers are the caller's contract.
    let (desc, fmt) = unsafe {
        (
            std::slice::from_raw_parts(descriptor, descriptor_bytes),
            std::slice::from_raw_parts(format, format_bytes),
        )
    };
    let Some(scope) = buffer_scope_validate(desc, fmt) else {
        return false;
    };
    let mut bytes = [0u8; 24];
    bytes.copy_from_slice(desc);
    // SAFETY: non-null checked above.
    fill(unsafe { &mut *output }, &scope, bytes);
    true
}

/// `dsound_buffer_scope_snapshot`.
///
/// # Safety
/// `output` must be null or valid for one write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_buffer_scope_snapshot(
    descriptor: u32,
    output: *mut CBufferScope,
) -> bool {
    if output.is_null() {
        return false;
    }
    let world = HostWorld;
    let Some(scope) = buffer_scope_snapshot(&world, descriptor) else {
        return false;
    };
    let Some(desc) = world.read_bytes(descriptor, 24) else {
        return false;
    };
    let mut bytes = [0u8; 24];
    bytes.copy_from_slice(&desc);
    // SAFETY: non-null checked above.
    fill(unsafe { &mut *output }, &scope, bytes);
    true
}
