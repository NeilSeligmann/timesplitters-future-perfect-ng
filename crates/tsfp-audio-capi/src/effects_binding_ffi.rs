//! C ABI for Rust-owned DirectSound effects descriptor construction.

use std::ptr;
use tsfp_audio::effects_binding::{self, DESCRIPTOR_BYTES};
use tsfp_audio::effects_metadata::Metadata;

/// Translate validated image metadata into the title-visible guest descriptor.
///
/// The caller supplies the four already-allocated guest views in code/state/workspace/Y order.
/// The output is published only on success and must be exactly 296 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_effects_binding_build_descriptor_rust(
    metadata: *const Metadata,
    code_base: u32,
    state_base: u32,
    workspace_base: u32,
    y_base: u32,
    output: *mut u8,
    output_bytes: usize,
) -> bool {
    if metadata.is_null() || output.is_null() || output_bytes != DESCRIPTOR_BYTES {
        return false;
    }
    // SAFETY: the C ABI requires one readable Metadata value for this call.
    let metadata = unsafe { ptr::read(metadata) };
    let Some(descriptor) =
        effects_binding::build_descriptor(&metadata, code_base, state_base, workspace_base, y_base)
    else {
        return false;
    };
    // SAFETY: the C ABI requires a writable output span of exactly DESCRIPTOR_BYTES bytes.
    unsafe { ptr::copy_nonoverlapping(descriptor.as_ptr(), output, DESCRIPTOR_BYTES) };
    true
}
