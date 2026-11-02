//! Rust-owned wrapper for the measured DirectSound stream SetVolume HLE entry.
use std::ffi::c_void;
use tsfp_audio::device_frames::CallFrame;
use tsfp_audio::frames::{VOLUME, VOLUME_AFTER_FORMAT_CALLER, decode_frame};

use crate::frame_guest_memory::FrameGuestMemory;

const REFUSE_DECODE: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct KernelCallFramePrefix {
    stack_pointer: u32,
    stack_limit: u32,
}

unsafe extern "C" {
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn dsound_stream_volume_hle_extension_rust(context: *mut c_void, result_out: *mut u32) -> bool;
    fn dsound_stream_volume_hle_completion_enabled_rust() -> bool;
    fn dsound_stream_volume_hle_guest_pid_rust() -> i32;
    fn dsound_stream_volume_hle_apply_rust(stream: u32, millibels: u32, after_format: bool) -> u32;
    fn dsound_stream_volume_hle_refuse_rust(reason: u32) -> !;
}

trait VolumeHleHost {
    fn extension(&mut self, context: *mut c_void) -> Option<u32>;
    fn completion_enabled(&mut self) -> bool;
    fn decode(
        &mut self,
        context: *mut c_void,
        completion_enabled: bool,
    ) -> Option<(u32, u32, bool)>;
    fn apply(&mut self, stream: u32, millibels: u32, after_format: bool) -> u32;
    fn refuse(&mut self, reason: u32) -> !;
}

struct HostVolumeHle;

impl VolumeHleHost for HostVolumeHle {
    fn extension(&mut self, context: *mut c_void) -> Option<u32> {
        let mut result = 0;
        // SAFETY: this preserves the C movie-stream extension's first chance before ordinary decoding.
        unsafe { dsound_stream_volume_hle_extension_rust(context, &mut result).then_some(result) }
    }

    fn completion_enabled(&mut self) -> bool {
        // SAFETY: C snapshots the adapter flag while holding its stream mutex.
        unsafe { dsound_stream_volume_hle_completion_enabled_rust() }
    }

    fn decode(
        &mut self,
        context: *mut c_void,
        completion_enabled: bool,
    ) -> Option<(u32, u32, bool)> {
        // SAFETY: the host PID identifies the process whose identity-mapped guest stack is live.
        let host_pid = unsafe { dsound_stream_volume_hle_guest_pid_rust() };
        // SAFETY: registered HLE callback receives a live kernel_call_frame; C asserts this prefix layout.
        unsafe { decode_volume_frame(context, completion_enabled, host_pid) }
    }

    fn apply(&mut self, stream: u32, millibels: u32, after_format: bool) -> u32 {
        // SAFETY: C serializes and validates the exact stream lease, then applies the Rust-owned transaction.
        unsafe { dsound_stream_volume_hle_apply_rust(stream, millibels, after_format) }
    }

    fn refuse(&mut self, reason: u32) -> ! {
        // SAFETY: C reports through the established fatal HLE path.
        unsafe { dsound_stream_volume_hle_refuse_rust(reason) }
    }
}

unsafe fn decode_volume_frame(
    context: *mut c_void,
    completion_enabled: bool,
    host_pid: i32,
) -> Option<(u32, u32, bool)> {
    if context.is_null() || host_pid <= 0 {
        return None;
    }
    // SAFETY: C asserts that stack_ptr and stack_limit are the first two u32 fields.
    let prefix = unsafe { context.cast::<KernelCallFramePrefix>().read() };
    let memory = FrameGuestMemory::new(host_pid);
    let call = decode_frame(
        &memory,
        VOLUME,
        CallFrame {
            stack_pointer: prefix.stack_pointer,
            stack_limit: prefix.stack_limit,
        },
        completion_enabled,
    )
    .ok()?;
    Some((
        call.args[0],
        call.args[1],
        call.caller == VOLUME_AFTER_FORMAT_CALLER,
    ))
}

fn dispatch<H: VolumeHleHost>(host: &mut H, context: *mut c_void) -> u32 {
    if let Some(result) = host.extension(context) {
        return result;
    }
    let completion_enabled = host.completion_enabled();
    let Some((stream, millibels, after_format)) = host.decode(context, completion_enabled) else {
        host.refuse(REFUSE_DECODE);
    };
    host.apply(stream, millibels, after_format)
}

unsafe extern "C" fn volume_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostVolumeHle, context)
}

/// Register the Rust callback for the measured stream SetVolume entry.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_volume_register_rust() -> usize {
    // SAFETY: the C measured surface is initialized before DirectSound handler registration.
    let selected =
        crate::movie_device_hle_ffi::tsfp_audio_hle_register_handler_rust(VOLUME, volume_handler);
    if !selected {
        return 0;
    }
    // SAFETY: C accepts only addresses in its fixed measured DirectSound surface.
    unsafe { dsound_hle_register_rust(VOLUME) as usize }
}

#[cfg(test)]
fn decode_guest_frame(
    memory: &impl tsfp_audio::adapter::GuestMemory,
    stack_pointer: u32,
    stack_limit: u32,
    completion_enabled: bool,
) -> Option<(u32, u32, bool)> {
    let call = decode_frame(
        memory,
        VOLUME,
        CallFrame {
            stack_pointer,
            stack_limit,
        },
        completion_enabled,
    )
    .ok()?;
    Some((
        call.args[0],
        call.args[1],
        call.caller == VOLUME_AFTER_FORMAT_CALLER,
    ))
}

#[cfg(test)]
mod tests {
    use super::{REFUSE_DECODE, VolumeHleHost, decode_guest_frame, dispatch};
    use std::cell::RefCell;
    use std::ffi::c_void;
    use tsfp_audio::adapter::GuestMemory;

    struct Memory([u32; 3]);

    impl GuestMemory for Memory {
        fn read_u32(&self, address: u32) -> Option<u32> {
            let index = address.checked_sub(0x1000)? as usize / 4;
            self.0.get(index).copied()
        }

        fn write_u32(&mut self, _address: u32, _value: u32) -> bool {
            false
        }

        fn read_bytes(&self, _address: u32, _len: usize) -> Option<Vec<u8>> {
            None
        }

        fn range_readable(&self, _address: u32, _len: u32) -> bool {
            false
        }
    }

    struct Host {
        events: RefCell<Vec<&'static str>>,
        extension_result: Option<u32>,
        decoded: Option<(u32, u32, bool)>,
    }

    impl VolumeHleHost for Host {
        fn extension(&mut self, _context: *mut c_void) -> Option<u32> {
            self.events.borrow_mut().push("extension");
            self.extension_result
        }
        fn completion_enabled(&mut self) -> bool {
            self.events.borrow_mut().push("completion");
            true
        }
        fn decode(&mut self, _context: *mut c_void, completion: bool) -> Option<(u32, u32, bool)> {
            assert!(completion);
            self.events.borrow_mut().push("decode");
            self.decoded
        }
        fn apply(&mut self, _stream: u32, _millibels: u32, _after_format: bool) -> u32 {
            self.events.borrow_mut().push("apply");
            0x1234
        }
        fn refuse(&mut self, reason: u32) -> ! {
            assert_eq!(reason, REFUSE_DECODE);
            panic!("expected fatal refusal")
        }
    }

    #[test]
    fn extension_precedes_completion_snapshot_decode_and_apply() {
        let mut host = Host {
            events: RefCell::new(Vec::new()),
            extension_result: Some(7),
            decoded: Some((1, 2, false)),
        };
        assert_eq!(dispatch(&mut host, std::ptr::null_mut()), 7);
        assert_eq!(*host.events.borrow(), ["extension"]);
    }

    #[test]
    fn ordinary_volume_keeps_extension_then_snapshot_decode_apply_order() {
        let mut host = Host {
            events: RefCell::new(Vec::new()),
            extension_result: None,
            decoded: Some((0x123, 0xffff_d8f0, true)),
        };
        assert_eq!(dispatch(&mut host, std::ptr::null_mut()), 0x1234);
        assert_eq!(
            *host.events.borrow(),
            ["extension", "completion", "decode", "apply"]
        );
    }

    #[test]
    fn volume_decoder_preserves_measured_caller_and_arguments() {
        let memory = Memory([0x29a67, 0x2222, 0xffff_d8f0]);
        assert_eq!(
            decode_guest_frame(&memory, 0x1000, 0x100c, false),
            Some((0x2222, 0xffff_d8f0, true))
        );
    }

    #[test]
    fn volume_decoder_refuses_truncated_or_unmeasured_frames() {
        let truncated = Memory([0x29f68, 0x2222, 0xffff_d8f0]);
        assert_eq!(decode_guest_frame(&truncated, 0x1000, 0x1008, false), None);
        let unmeasured = Memory([0x1234, 0x2222, 0xffff_d8f0]);
        assert_eq!(decode_guest_frame(&unmeasured, 0x1000, 0x100c, false), None);
    }
}
