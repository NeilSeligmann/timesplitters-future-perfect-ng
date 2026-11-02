//! Rust-host publication of the initial DirectSound stream packet status words.
use crate::effects_guest_copy_ffi::{
    tsfp_effects_guest_copy_read_rust, tsfp_effects_guest_copy_write_rust,
};

const STREAM_HEADER_BYTES: u64 = 40;
const PENDING_STATUS: u32 = 0x8000_000A;

fn word_ok(pid: i32, address: u32, stream: u32) -> bool {
    if address == 0 {
        return true;
    }
    let address_end = u64::from(address) + 4;
    let stream_end = u64::from(stream) + STREAM_HEADER_BYTES;
    if address_end > 1u64 << 32
        || (u64::from(address) < stream_end && u64::from(stream) < address_end)
    {
        return false;
    }
    let mut bytes = [0; 4];
    // SAFETY: both copies validate the guest address and use fault-safe process_vm operations.
    unsafe {
        tsfp_effects_guest_copy_read_rust(pid, address, bytes.as_mut_ptr(), bytes.len())
            && tsfp_effects_guest_copy_write_rust(pid, bytes.as_ptr(), address, bytes.len())
    }
}

/// Validate both optional packet completion words, then publish the initial size/status pair.
/// The writes preserve the C adapter order: completed size first, pending status second.
///
/// # Safety
/// The guest spans must belong to the process identified by `host_pid`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_pending_publish_rust(
    host_pid: i32,
    stream: u32,
    completed: u32,
    status: u32,
) -> bool {
    if host_pid <= 0 || (completed != 0 && completed == status) {
        return false;
    }
    if !word_ok(host_pid, completed, stream) || !word_ok(host_pid, status, stream) {
        return false;
    }
    if completed != 0 {
        let zero = 0u32.to_le_bytes();
        // SAFETY: the word was probed above; process_vm safely refuses if it changed meanwhile.
        if !unsafe {
            tsfp_effects_guest_copy_write_rust(host_pid, zero.as_ptr(), completed, zero.len())
        } {
            return false;
        }
    }
    if status != 0 {
        let pending = PENDING_STATUS.to_le_bytes();
        // SAFETY: the word was probed above; process_vm safely refuses if it changed meanwhile.
        if !unsafe {
            tsfp_effects_guest_copy_write_rust(host_pid, pending.as_ptr(), status, pending.len())
        } {
            return false;
        }
    }
    true
}

/// Publish the already-due packet's size and success status in the observed order.
/// Each optional write is attempted independently, matching the C adapter even
/// when the first destination has become unavailable.
///
/// # Safety
/// The guest spans must belong to the process identified by `host_pid`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_completion_publish_rust(
    host_pid: i32,
    completed: u32,
    size: u32,
    status: u32,
) -> bool {
    if host_pid <= 0 {
        return false;
    }
    let mut accepted = true;
    if completed != 0 {
        let bytes = size.to_le_bytes();
        // SAFETY: the process_vm boundary validates the guest write without faulting.
        accepted = unsafe {
            tsfp_effects_guest_copy_write_rust(host_pid, bytes.as_ptr(), completed, bytes.len())
        };
    }
    if status != 0 {
        let bytes = 0u32.to_le_bytes();
        // Always attempt the second write: the C adapter did not short circuit
        // after a failed completed-size write.
        let status_accepted = unsafe {
            tsfp_effects_guest_copy_write_rust(host_pid, bytes.as_ptr(), status, bytes.len())
        };
        accepted &= status_accepted;
    }
    accepted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_completed_zero_before_pending_status_values() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let stream = base + 0x100;
        let completed = base + 0x200;
        let status = completed + 4;
        // SAFETY: the test owns a writable page containing both guest words.
        unsafe {
            (completed as usize as *mut u32).write(0x1111_2222);
            (status as usize as *mut u32).write(0x3333_4444);
        }
        // SAFETY: the test's low-address page remains mapped for the call.
        assert!(unsafe {
            tsfp_audio_stream_pending_publish_rust(
                std::process::id() as i32,
                stream,
                completed,
                status,
            )
        });
        // SAFETY: both words remain mapped and writable.
        unsafe {
            assert_eq!((completed as usize as *const u32).read(), 0);
            assert_eq!((status as usize as *const u32).read(), PENDING_STATUS);
        }
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn refuses_aliases_before_publishing_either_word() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let stream = base + 0x100;
        let completed = base + 0x200;
        // SAFETY: the test owns a writable page containing the completion word.
        unsafe { (completed as usize as *mut u32).write(0xAABB_CCDD) };
        // SAFETY: the test's low-address page remains mapped for the call.
        assert!(!unsafe {
            tsfp_audio_stream_pending_publish_rust(
                std::process::id() as i32,
                stream,
                completed,
                stream + 8,
            )
        });
        // SAFETY: the completion word remains mapped and unchanged on refusal.
        assert_eq!(
            unsafe { (completed as usize as *const u32).read() },
            0xAABB_CCDD
        );
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn delivers_size_then_status_even_for_overlapping_guest_words() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let completed = base + 0x300;
        let status = completed + 1;
        // SAFETY: the test owns the mapped low-address output bytes.
        unsafe {
            (completed as usize as *mut u8).write_bytes(0xCC, 5);
        }
        // SAFETY: the mapped test page remains available for both writes.
        assert!(unsafe {
            tsfp_audio_stream_completion_publish_rust(
                std::process::id() as i32,
                completed,
                0x1122_3344,
                status,
            )
        });
        // The second write overwrites the overlapping high bytes of the first.
        // SAFETY: both addresses remain in the mapped page.
        unsafe {
            assert_eq!((completed as usize as *const u8).read(), 0x44);
            assert_eq!((status as usize as *const u32).read_unaligned(), 0);
        }
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn attempts_status_write_when_size_destination_is_unmapped() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let status = base + 0x300;
        // SAFETY: status is writable in the mapped page.
        unsafe { (status as usize as *mut u32).write(0xAABB_CCDD) };
        // SAFETY: process_vm safely refuses the protected guest write.
        assert!(!unsafe {
            tsfp_audio_stream_completion_publish_rust(
                std::process::id() as i32,
                0xFFFF_FFFC,
                0x1234,
                status,
            )
        });
        // SAFETY: the writable status word remains mapped.
        assert_eq!(unsafe { (status as usize as *const u32).read() }, 0);
        crate::test_guest_memory::unmap_guest_page(page);
    }
}
