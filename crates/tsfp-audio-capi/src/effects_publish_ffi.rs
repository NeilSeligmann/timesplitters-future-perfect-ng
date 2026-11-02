//! Fault-safe publication of an accepted effects binding into guest virtual addresses.

use std::ffi::c_void;
use std::ptr;

const DESCRIPTOR_BYTES: usize = 296;
const CODE_BYTES: usize = 8080;
const STATE_BYTES: usize = 8088;
const WORKSPACE_BYTES: usize = 393_216;
const Y_BYTES: usize = 4;
const ZERO_CHUNK: [u8; 4096] = [0; 4096];

#[repr(C)]
struct IoVec {
    base: *mut c_void,
    len: usize,
}

unsafe extern "C" {
    fn process_vm_writev(
        pid: i32,
        local: *const IoVec,
        local_count: usize,
        remote: *const IoVec,
        remote_count: usize,
        flags: usize,
    ) -> isize;
    #[cfg(test)]
    fn mmap(
        address: *mut c_void,
        length: usize,
        prot: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut c_void;
    #[cfg(test)]
    fn munmap(address: *mut c_void, length: usize) -> i32;
}

fn local_span_ok(pointer: *const u8, bytes: usize) -> bool {
    !pointer.is_null() && (pointer as usize).checked_add(bytes).is_some()
}

fn guest_span_ok(address: u32, bytes: usize) -> bool {
    const GUEST_LIMIT: u64 = 1u64 << 32;
    address != 0 && bytes != 0 && (bytes as u64) <= GUEST_LIMIT - u64::from(address)
}

fn write_exact(pid: i32, source: *const u8, destination: u32, bytes: usize) -> bool {
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
    // SAFETY: validated spans are passed through the fault-safe Linux syscall.
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

fn zero_exact(pid: i32, mut destination: u32, bytes: usize) -> bool {
    if !guest_span_ok(destination, bytes) {
        return false;
    }
    let mut remaining = bytes;
    while remaining != 0 {
        let count = remaining.min(ZERO_CHUNK.len());
        if !write_exact(pid, ZERO_CHUNK.as_ptr(), destination, count) {
            return false;
        }
        // The final chunk may end exactly at 2^32; the full span was checked above.
        destination = destination.wrapping_add(count as u32);
        remaining -= count;
    }
    true
}

/// Publish the descriptor and its allocated views using the kernel's host PID. The six guest
/// addresses correspond to descriptor, code, state, workspace, Y, and optional GP staging; a
/// zero sixth address means GP staging is disabled. The title output pointer is published last.
///
/// # Safety
/// Sources must be readable for their fixed lengths, and `addresses` must point to six readable
/// `u32`s for the duration of this synchronous call. Guest spans are checked by process_vm_writev.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_effects_binding_publish_rust(
    pid: i32,
    descriptor_source: *const u8,
    code_source: *const u8,
    state_source: *const u8,
    addresses: *const u32,
    output_address: u32,
) -> bool {
    if !local_span_ok(descriptor_source, DESCRIPTOR_BYTES)
        || !local_span_ok(code_source, CODE_BYTES)
        || !local_span_ok(state_source, STATE_BYTES)
        || addresses.is_null()
    {
        return false;
    }
    // SAFETY: caller promises a readable six-element address array.
    let addresses = unsafe { ptr::read(addresses.cast::<[u32; 6]>()) };
    let [descriptor, code, state, workspace, y, staging] = addresses;
    if !guest_span_ok(descriptor, DESCRIPTOR_BYTES)
        || !guest_span_ok(code, CODE_BYTES)
        || !guest_span_ok(state, STATE_BYTES)
        || !guest_span_ok(workspace, WORKSPACE_BYTES)
        || !guest_span_ok(y, Y_BYTES)
        || (staging != 0 && !guest_span_ok(staging, STATE_BYTES))
        || !guest_span_ok(output_address, std::mem::size_of::<u32>())
    {
        return false;
    }

    if !zero_exact(pid, workspace, WORKSPACE_BYTES)
        || !zero_exact(pid, y, Y_BYTES)
        || !write_exact(pid, descriptor_source, descriptor, DESCRIPTOR_BYTES)
        || !write_exact(pid, code_source, code, CODE_BYTES)
        || !write_exact(pid, state_source, state, STATE_BYTES)
        || (staging != 0 && !write_exact(pid, state_source, staging, STATE_BYTES))
    {
        return false;
    }

    // Publish last. The local value remains live for the complete synchronous syscall.
    write_exact(
        pid,
        (&descriptor as *const u32).cast(),
        output_address,
        std::mem::size_of::<u32>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP_PRIVATE: i32 = 0x02;
    const MAP_ANONYMOUS: i32 = 0x20;
    const MAP_32BIT: i32 = 0x40;
    const PROT_READ: i32 = 1;
    const PROT_WRITE: i32 = 2;

    #[test]
    fn publication_writes_guest_views_and_commits_the_output_last() {
        let length = 0x80000usize;
        // SAFETY: this test owns the mapping until the synchronous publication and assertions end.
        let mapping = unsafe {
            mmap(
                ptr::null_mut(),
                length,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS | MAP_32BIT,
                -1,
                0,
            )
        };
        assert_ne!(mapping, (-1isize) as *mut c_void);
        let base = mapping as usize;
        assert!(base != 0 && base + length <= (1usize << 32));
        // SAFETY: the successful mapping is writable for `length` bytes.
        unsafe { ptr::write_bytes(mapping.cast::<u8>(), 0xA5, length) };

        let base = base as u32;
        let addresses = [
            base,
            base + 296,
            base + 296 + 8080,
            base + 296 + 8080 + 8088,
            base + 296 + 8080 + 8088 + WORKSPACE_BYTES as u32,
            base + 296 + 8080 + 8088 + WORKSPACE_BYTES as u32 + Y_BYTES as u32 + 4,
        ];
        let output = addresses[5] + STATE_BYTES as u32;
        let descriptor = [0x31u8; DESCRIPTOR_BYTES];
        let code = [0x52u8; CODE_BYTES];
        let state = [0x73u8; STATE_BYTES];
        let ok = unsafe {
            tsfp_effects_binding_publish_rust(
                std::process::id() as i32,
                descriptor.as_ptr(),
                code.as_ptr(),
                state.as_ptr(),
                addresses.as_ptr(),
                output,
            )
        };
        assert!(ok);

        let descriptor_at = addresses[0] as usize as *const u8;
        let code_at = addresses[1] as usize as *const u8;
        let state_at = addresses[2] as usize as *const u8;
        let workspace_at = addresses[3] as usize as *const u8;
        let y_at = addresses[4] as usize as *const u8;
        let staging_at = addresses[5] as usize as *const u8;
        // SAFETY: all ranges are within the writable low-4-GiB mapping above.
        unsafe {
            assert_eq!(
                std::slice::from_raw_parts(descriptor_at, DESCRIPTOR_BYTES),
                descriptor
            );
            assert_eq!(std::slice::from_raw_parts(code_at, CODE_BYTES), code);
            assert_eq!(std::slice::from_raw_parts(state_at, STATE_BYTES), state);
            assert!(
                std::slice::from_raw_parts(workspace_at, WORKSPACE_BYTES)
                    .iter()
                    .all(|byte| *byte == 0)
            );
            assert!(
                std::slice::from_raw_parts(y_at, Y_BYTES)
                    .iter()
                    .all(|byte| *byte == 0)
            );
            assert_eq!(std::slice::from_raw_parts(staging_at, STATE_BYTES), state);
            assert_eq!(ptr::read(output as usize as *const u32), addresses[0]);
            assert_eq!(munmap(mapping, length), 0);
        }
    }
}
