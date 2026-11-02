//! Live C ABI for the typed buffer-call decoder in `tsfp-audio`.
use crate::frame_guest_memory::FrameGuestMemory;
use tsfp_audio::buffer_frames::{BufferEntry, decode};
use tsfp_audio::device_frames::CallFrame;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CBufferCall {
    pub entry: u32,
    pub caller: u32,
    pub args: [u32; 5],
}

/// Decode one live DirectSound buffer call using the Rust typed frame contract.
/// Returns false without touching `out` when the address, memory, or call is refused.
///
/// # Safety
/// `out` must point to writable storage when non-null. The guest span must be
/// identity-mapped in the process identified by `host_pid`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_buffer_frame_decode(
    entry_address: u32,
    stack_pointer: u32,
    stack_limit: u32,
    completion_enabled: bool,
    host_pid: i32,
    out: *mut CBufferCall,
) -> bool {
    let Some(entry) = BufferEntry::from_address(entry_address) else {
        return false;
    };
    if out.is_null() || host_pid <= 0 {
        return false;
    }
    let memory = FrameGuestMemory::new(host_pid);
    let frame = CallFrame {
        stack_pointer,
        stack_limit,
    };
    let Ok(call) = decode(&memory, entry, Some(frame), completion_enabled) else {
        return false;
    };
    // SAFETY: checked non-null and guaranteed writable by the caller.
    unsafe {
        out.write(CBufferCall {
            entry: entry_address,
            caller: call.caller,
            args: call.args,
        });
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_decodes_a_checked_buffer_call() {
        let (page, stack) = crate::test_guest_memory::map_guest_page();
        // SAFETY: the test owns a writable low-address page and initializes three words.
        unsafe {
            page.cast::<u32>().write(0x28348);
            page.cast::<u32>().add(1).write(0x55);
            page.cast::<u32>().add(2).write(0xffff_d8f0);
        }
        let mut out = CBufferCall {
            entry: 0,
            caller: 0,
            args: [0; 5],
        };
        // SAFETY: mapped guest page and output storage remain live for this call.
        let accepted = unsafe {
            tsfp_audio_buffer_frame_decode(
                0x407a64,
                stack,
                stack + 12,
                false,
                std::process::id() as i32,
                &mut out,
            )
        };
        assert!(accepted);
        assert_eq!(
            out,
            CBufferCall {
                entry: 0x407a64,
                caller: 0x28348,
                args: [0x55, 0xffff_d8f0, 0, 0, 0],
            }
        );
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn ffi_refusal_preserves_output() {
        let (page, stack) = crate::test_guest_memory::map_guest_page();
        // SAFETY: the test owns a writable low-address page and initializes three words.
        unsafe {
            page.cast::<u32>().write(0x12345);
            page.cast::<u32>().add(1).write(0x55);
            page.cast::<u32>().add(2).write(1);
        }
        let sentinel = CBufferCall {
            entry: 1,
            caller: 2,
            args: [3, 4, 5, 6, 7],
        };
        let mut out = sentinel;
        // SAFETY: mapped guest page and output storage remain live for this call.
        let accepted = unsafe {
            tsfp_audio_buffer_frame_decode(
                0x407a64,
                stack,
                stack + 12,
                false,
                std::process::id() as i32,
                &mut out,
            )
        };
        assert!(!accepted);
        assert_eq!(out, sentinel);
        crate::test_guest_memory::unmap_guest_page(page);
    }
}
