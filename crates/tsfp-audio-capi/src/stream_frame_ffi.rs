//! Live C ABI for the Rust typed stream-call decoder.
use crate::frame_guest_memory::FrameGuestMemory;
use tsfp_audio::device_frames::CallFrame;
use tsfp_audio::frames::decode_frame;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CStreamCall {
    pub entry: u32,
    pub caller: u32,
    pub args: [u32; 5],
}

/// Decode one live DirectSound stream call using Rust's typed frame contract.
/// Returns false without touching `out` when decoding or admission refuses the call.
///
/// # Safety
/// `out` must point to writable storage when non-null. The guest span must be
/// identity-mapped in the process identified by `host_pid`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_frame_decode(
    entry: u32,
    stack_pointer: u32,
    stack_limit: u32,
    completion_enabled: bool,
    host_pid: i32,
    out: *mut CStreamCall,
) -> bool {
    if out.is_null() || host_pid <= 0 {
        return false;
    }
    let memory = FrameGuestMemory::new(host_pid);
    let frame = CallFrame {
        stack_pointer,
        stack_limit,
    };
    let Ok(call) = decode_frame(&memory, entry, frame, completion_enabled) else {
        return false;
    };
    // SAFETY: checked non-null and guaranteed writable by the caller.
    unsafe {
        out.write(CStreamCall {
            entry: call.entry,
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
    fn ffi_decodes_a_checked_stream_call() {
        let (page, stack) = crate::test_guest_memory::map_guest_page();
        // SAFETY: the test owns a writable low-address page and initializes three words.
        unsafe {
            page.cast::<u32>().write(0x29943);
            page.cast::<u32>().add(1).write(0x55);
            page.cast::<u32>().add(2).write(0x66);
        }
        let mut out = CStreamCall {
            entry: 0,
            caller: 0,
            args: [0; 5],
        };
        // SAFETY: mapped guest page and output storage remain live for this call.
        let accepted = unsafe {
            tsfp_audio_stream_frame_decode(
                tsfp_audio::frames::CREATE,
                stack,
                stack + 12,
                false,
                std::process::id() as i32,
                &mut out,
            )
        };
        assert!(accepted);
        assert_eq!(out.entry, tsfp_audio::frames::CREATE);
        assert_eq!(out.caller, 0x29943);
        assert_eq!(out.args, [0x55, 0x66, 0, 0, 0]);
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn ffi_refusal_preserves_output() {
        let (page, stack) = crate::test_guest_memory::map_guest_page();
        // SAFETY: the test owns a writable low-address page and initializes three words.
        unsafe {
            page.cast::<u32>().write(0x12345);
            page.cast::<u32>().add(1).write(0x55);
            page.cast::<u32>().add(2).write(0x66);
        }
        let sentinel = CStreamCall {
            entry: 1,
            caller: 2,
            args: [3, 4, 5, 6, 7],
        };
        let mut out = sentinel;
        // SAFETY: mapped guest page and output storage remain live for this call.
        let accepted = unsafe {
            tsfp_audio_stream_frame_decode(
                tsfp_audio::frames::CREATE,
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

    #[test]
    fn ffi_stack_limit_refuses_partial_frame_without_publishing() {
        let (page, stack) = crate::test_guest_memory::map_guest_page();
        // SAFETY: the test owns a writable low-address page and initializes three words.
        unsafe {
            page.cast::<u32>().write(0x29943);
            page.cast::<u32>().add(1).write(0x55);
            page.cast::<u32>().add(2).write(0x66);
        }
        let sentinel = CStreamCall {
            entry: 1,
            caller: 2,
            args: [3, 4, 5, 6, 7],
        };
        let mut out = sentinel;
        // SAFETY: mapped page and output storage remain live; stack limit excludes arg 1.
        let accepted = unsafe {
            tsfp_audio_stream_frame_decode(
                tsfp_audio::frames::CREATE,
                stack,
                stack + 8,
                false,
                std::process::id() as i32,
                &mut out,
            )
        };
        assert!(!accepted);
        assert_eq!(out, sentinel);
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn ffi_refuses_an_unmapped_guest_frame_without_faulting_or_publishing() {
        let sentinel = CStreamCall {
            entry: 1,
            caller: 2,
            args: [3, 4, 5, 6, 7],
        };
        let mut out = sentinel;
        // SAFETY: output storage remains valid; the invalid guest address is read with process_vm_readv.
        let accepted = unsafe {
            tsfp_audio_stream_frame_decode(
                tsfp_audio::frames::CREATE,
                u32::MAX - 3,
                0,
                false,
                std::process::id() as i32,
                &mut out,
            )
        };
        assert!(!accepted);
        assert_eq!(out, sentinel);
    }
}
