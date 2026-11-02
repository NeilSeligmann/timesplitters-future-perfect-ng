//! C boundary for the Rust effects-image layout validator.

use std::ptr;
use tsfp_audio::effects_identity;
use tsfp_audio::effects_metadata::{self, Metadata};

#[repr(C)]
struct IoVec {
    base: *mut std::ffi::c_void,
    len: usize,
}

unsafe extern "C" {
    fn process_vm_readv(
        pid: i32,
        local: *const IoVec,
        local_count: usize,
        remote: *const IoVec,
        remote_count: usize,
        flags: usize,
    ) -> isize;
}

/// Copy a host span without dereferencing it in this process. A partial read is a refusal.
fn fault_safe_snapshot(source: *const u8, bytes: usize) -> Option<Vec<u8>> {
    if source.is_null() || bytes == 0 || bytes > isize::MAX as usize {
        return None;
    }
    let mut snapshot = vec![0u8; bytes];
    let local = IoVec {
        base: snapshot.as_mut_ptr().cast(),
        len: snapshot.len(),
    };
    let remote = IoVec {
        base: source.cast_mut().cast(),
        len: bytes,
    };
    // SAFETY: local is a live writable Vec span; the kernel validates the remote mapping.
    let copied = unsafe { process_vm_readv(std::process::id() as i32, &local, 1, &remote, 1, 0) };
    (copied == bytes as isize).then_some(snapshot)
}

/// Validate a readable image span and publish the parsed C-layout result on success.
///
/// The caller owns the span and must keep it readable for the duration of this call. On refusal,
/// the output remains untouched, matching `dsound_effects_validate_layout`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_effects_validate_layout_rust(
    image: *const u8,
    bytes: usize,
    output: *mut Metadata,
) -> bool {
    if image.is_null()
        || output.is_null()
        || bytes > u32::MAX as usize
        || bytes > isize::MAX as usize
    {
        return false;
    }
    // SAFETY: the C contract requires a readable `bytes`-length input span for this call.
    let image = unsafe { std::slice::from_raw_parts(image, bytes) };
    let Some(metadata) = effects_metadata::validate_layout(image) else {
        return false;
    };
    // SAFETY: the C contract requires one writable Metadata result on success.
    unsafe { ptr::write(output, metadata) };
    true
}

/// Validate the captured, fixed effects image on the Rust-host audio path.
///
/// The caller performs the fault-safe guest-memory snapshot. This function owns the
/// immutable-image fingerprints, measured header guards, and exact layout checks; output
/// is published only after every guard succeeds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_effects_validate_known_image_rust(
    image: *const u8,
    bytes: usize,
    output: *mut Metadata,
) -> bool {
    if image.is_null()
        || output.is_null()
        || bytes != effects_identity::IMAGE_BYTES
        || bytes > isize::MAX as usize
    {
        return false;
    }
    // SAFETY: the C snapshot adapter owns a readable immutable image span for this call.
    let image = unsafe { std::slice::from_raw_parts(image, bytes) };
    let Some(metadata) = effects_identity::validate_known_image(image) else {
        return false;
    };
    // SAFETY: the C contract provides one writable Metadata result on success.
    unsafe { ptr::write(output, metadata) };
    true
}

/// Fault-safely snapshot and validate the exact known guest image. The caller supplies a source
/// pointer resolved by the guest-memory layer and writable output spans for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_effects_snapshot_known_image_rust(
    source: *const u8,
    bytes: usize,
    image_output: *mut u8,
    metadata_output: *mut Metadata,
) -> bool {
    if bytes != effects_identity::IMAGE_BYTES || image_output.is_null() || metadata_output.is_null()
    {
        return false;
    }
    let Some(snapshot) = fault_safe_snapshot(source, bytes) else {
        return false;
    };
    let Some(metadata) = effects_identity::validate_known_image(&snapshot) else {
        return false;
    };
    // SAFETY: the ABI contract provides writable spans of the fixed image/result sizes.
    unsafe {
        ptr::copy_nonoverlapping(snapshot.as_ptr(), image_output, bytes);
        ptr::write(metadata_output, metadata);
    }
    true
}

/// Snapshot the known effects image directly from its identity-mapped guest address.
/// The Rust-audio-host path uses the shared fault-safe guest-memory contract and does not
/// ask the C kernel layer to resolve or probe the guest span first.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_effects_snapshot_known_guest_rust(
    host_pid: i32,
    address: u32,
    bytes: usize,
    image_output: *mut u8,
    metadata_output: *mut Metadata,
) -> bool {
    if bytes != effects_identity::IMAGE_BYTES || image_output.is_null() || metadata_output.is_null()
    {
        return false;
    }
    let Some(snapshot) = <crate::frame_guest_memory::FrameGuestMemory as
        tsfp_audio::adapter::GuestMemory>::read_bytes(
        &crate::frame_guest_memory::FrameGuestMemory::new(host_pid),
        address,
        bytes,
    )
    else {
        return false;
    };
    let Some(metadata) = effects_identity::validate_known_image(&snapshot) else {
        return false;
    };
    // SAFETY: the ABI contract provides writable spans of the fixed image/result sizes.
    unsafe {
        ptr::copy_nonoverlapping(snapshot.as_ptr(), image_output, bytes);
        ptr::write(metadata_output, metadata);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_guest_snapshot_refuses_unmapped_address_without_publishing_outputs() {
        let mut image = vec![0xA5; effects_identity::IMAGE_BYTES];
        // SAFETY: Metadata is a plain C-layout value and all-zero is a valid sentinel here.
        let mut metadata: Metadata = unsafe { std::mem::zeroed() };
        let before = metadata;
        // SAFETY: output buffers have the exact fixed sizes; the address is deliberately unmapped.
        let accepted = unsafe {
            dsound_effects_snapshot_known_guest_rust(
                std::process::id() as i32,
                0xFFFF_FFFF,
                effects_identity::IMAGE_BYTES,
                image.as_mut_ptr(),
                &mut metadata,
            )
        };
        assert!(!accepted);
        assert!(image.iter().all(|byte| *byte == 0xA5));
        assert_eq!(metadata, before);
    }
}
