//! Fault-safe copies between local buffers and guest virtual addresses.
//!
//! Guest memory is identity-mapped below 4 GiB by the host. Rust checks the guest ABI range and
//! uses `process_vm_readv` / `process_vm_writev` on that address directly, so the Rust-host effects
//! path does not call the C `kernel_guest_at` translator. The process ID comes from the kernel
//! layer so pid-namespace behavior matches the C oracle.

use std::ffi::c_void;

#[repr(C)]
struct IoVec {
    base: *mut c_void,
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
    fn process_vm_writev(
        pid: i32,
        local: *const IoVec,
        local_count: usize,
        remote: *const IoVec,
        remote_count: usize,
        flags: usize,
    ) -> isize;
}

fn local_span_ok(pointer: *const u8, bytes: usize) -> bool {
    !pointer.is_null() && (pointer as usize).checked_add(bytes).is_some()
}

fn guest_span_ok(address: u32, bytes: usize) -> bool {
    const GUEST_LIMIT: u64 = 1u64 << 32;
    address != 0 && bytes != 0 && (bytes as u64) <= GUEST_LIMIT - u64::from(address)
}

/// Copy from a guest virtual address into a local Rust/C buffer.
///
/// # Safety
/// The guest address must refer to a live identity-mapped guest span. Linux validates access to
/// both ranges and returns a refusal for inaccessible pages.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_effects_guest_copy_read_rust(
    pid: i32,
    source: u32,
    destination: *mut u8,
    bytes: usize,
) -> bool {
    if pid <= 0 || !guest_span_ok(source, bytes) || !local_span_ok(destination, bytes) {
        return false;
    }
    let local = IoVec {
        base: destination.cast(),
        len: bytes,
    };
    let remote = IoVec {
        base: (source as usize as *mut c_void),
        len: bytes,
    };
    // SAFETY: validated non-null spans are passed through the fault-safe Linux syscall.
    unsafe { process_vm_readv(pid, &local, 1, &remote, 1, 0) == bytes as isize }
}

/// Copy from a local Rust/C buffer into a guest virtual address.
///
/// # Safety
/// The guest address must refer to a live identity-mapped guest span. Linux validates access to
/// both ranges and returns a refusal for inaccessible pages.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_effects_guest_copy_write_rust(
    pid: i32,
    source: *const u8,
    destination: u32,
    bytes: usize,
) -> bool {
    if pid <= 0 || !local_span_ok(source, bytes) || !guest_span_ok(destination, bytes) {
        return false;
    }
    let local = IoVec {
        base: source.cast_mut().cast(),
        len: bytes,
    };
    let remote = IoVec {
        base: destination as usize as *mut c_void,
        len: bytes,
    };
    // SAFETY: validated non-null spans are passed through the fault-safe Linux syscall.
    let token = crate::direct_witness::prepare();
    let written = unsafe { process_vm_writev(pid, &local, 1, &remote, 1, 0) == bytes as isize };
    if written {
        // SAFETY: `source` spans `bytes` (checked by local_span_ok above).
        crate::direct_witness::report(token, destination, unsafe {
            std::slice::from_raw_parts(source, bytes)
        });
    }
    written
}
