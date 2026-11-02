//! Rust-owned HLE callbacks for measured stream spatial setters.
use std::ffi::c_void;
use tsfp_audio::device_frames::CallFrame;
use tsfp_audio::frames::{I3DL2, MAX_DISTANCE, MINIMUM, ROLLOFF, SET_POSITION, decode_frame};

use crate::frame_guest_memory::FrameGuestMemory;

type DsoundHandler = unsafe extern "C" fn(*mut c_void) -> u32;

const REFUSE_DECODE: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct KernelCallFramePrefix {
    stack_pointer: u32,
    stack_limit: u32,
}

unsafe extern "C" {
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn dsound_stream_spatial_hle_completion_enabled_rust() -> bool;
    fn dsound_stream_spatial_hle_guest_pid_rust() -> i32;
    fn dsound_stream_spatial_hle_apply_rust(entry: u32, args: *const u32) -> u32;
    fn dsound_stream_spatial_hle_refuse_rust(entry: u32, reason: u32) -> !;
}

trait SpatialHleHost {
    fn completion_enabled(&mut self) -> bool;
    fn decode(
        &mut self,
        context: *mut c_void,
        entry: u32,
        completion_enabled: bool,
    ) -> Option<[u32; 5]>;
    fn apply(&mut self, entry: u32, args: &[u32; 5]) -> u32;
    fn refuse(&mut self, entry: u32, reason: u32) -> !;
}

struct HostSpatialHle;

impl SpatialHleHost for HostSpatialHle {
    fn completion_enabled(&mut self) -> bool {
        // SAFETY: C snapshots the adapter flag while holding its stream mutex.
        unsafe { dsound_stream_spatial_hle_completion_enabled_rust() }
    }

    fn decode(
        &mut self,
        context: *mut c_void,
        entry: u32,
        completion_enabled: bool,
    ) -> Option<[u32; 5]> {
        // SAFETY: this is the process whose identity-mapped guest stack is live.
        let host_pid = unsafe { dsound_stream_spatial_hle_guest_pid_rust() };
        // SAFETY: the registered HLE callback receives a live kernel_call_frame.
        unsafe { decode_spatial_frame(context, entry, completion_enabled, host_pid) }
    }

    fn apply(&mut self, entry: u32, args: &[u32; 5]) -> u32 {
        // SAFETY: C validates the exact active object/device lease before entering its
        // existing typed Rust-owned spatial-state transaction.
        unsafe { dsound_stream_spatial_hle_apply_rust(entry, args.as_ptr()) }
    }

    fn refuse(&mut self, entry: u32, reason: u32) -> ! {
        // SAFETY: C reports the typed refusal through the established fatal HLE path.
        unsafe { dsound_stream_spatial_hle_refuse_rust(entry, reason) }
    }
}

unsafe fn decode_spatial_frame(
    context: *mut c_void,
    entry: u32,
    completion_enabled: bool,
    host_pid: i32,
) -> Option<[u32; 5]> {
    if context.is_null() || host_pid <= 0 {
        return None;
    }
    // SAFETY: C asserts that stack_ptr and stack_limit are the first two u32 fields.
    let prefix = unsafe { context.cast::<KernelCallFramePrefix>().read() };
    let memory = FrameGuestMemory::new(host_pid);
    let call = decode_frame(
        &memory,
        entry,
        CallFrame {
            stack_pointer: prefix.stack_pointer,
            stack_limit: prefix.stack_limit,
        },
        completion_enabled,
    )
    .ok()?;
    Some(call.args)
}

fn dispatch<H: SpatialHleHost>(host: &mut H, context: *mut c_void, entry: u32) -> u32 {
    let completion_enabled = host.completion_enabled();
    let Some(args) = host.decode(context, entry, completion_enabled) else {
        host.refuse(entry, REFUSE_DECODE);
    };
    host.apply(entry, &args)
}

unsafe extern "C" fn max_distance_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostSpatialHle, context, MAX_DISTANCE)
}

unsafe extern "C" fn minimum_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostSpatialHle, context, MINIMUM)
}

unsafe extern "C" fn set_position_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostSpatialHle, context, SET_POSITION)
}

unsafe extern "C" fn rolloff_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostSpatialHle, context, ROLLOFF)
}

unsafe extern "C" fn i3dl2_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostSpatialHle, context, I3DL2)
}

fn register(entry: u32, handler: DsoundHandler) -> bool {
    if !crate::movie_device_hle_ffi::tsfp_audio_hle_register_handler_rust(entry, handler) {
        return false;
    }
    // SAFETY: C initializes its measured HLE surface before stream registration.
    unsafe { dsound_hle_register_rust(entry) }
}

/// Register Rust callbacks for stream SetMaxDistance and SetPosition.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_spatial_register_rust() -> usize {
    register(MAX_DISTANCE, max_distance_handler) as usize
        + register(SET_POSITION, set_position_handler) as usize
}

/// Register Rust's SetMinDistance callback on the Rust-host path.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_minimum_register_rust() -> usize {
    register(MINIMUM, minimum_handler) as usize
}

/// Register Rust's SetRolloffCurve callback on the Rust-host path.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_rolloff_register_rust() -> usize {
    register(ROLLOFF, rolloff_handler) as usize
}

/// Register Rust's SetI3DL2Source callback on the Rust-host path.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_i3dl2_register_rust() -> usize {
    register(I3DL2, i3dl2_handler) as usize
}

#[cfg(test)]
fn decode_guest_frame(
    memory: &impl tsfp_audio::adapter::GuestMemory,
    entry: u32,
    stack_pointer: u32,
    stack_limit: u32,
    completion_enabled: bool,
) -> Option<[u32; 5]> {
    let call = decode_frame(
        memory,
        entry,
        CallFrame {
            stack_pointer,
            stack_limit,
        },
        completion_enabled,
    )
    .ok()?;
    Some(call.args)
}

#[cfg(test)]
mod tests {
    use super::{
        I3DL2, MAX_DISTANCE, MINIMUM, REFUSE_DECODE, ROLLOFF, SET_POSITION, SpatialHleHost,
        decode_guest_frame, dispatch,
    };
    use std::cell::RefCell;
    use std::ffi::c_void;
    use tsfp_audio::adapter::GuestMemory;

    struct Memory([u32; 6]);

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
        completion: bool,
        decoded: Option<[u32; 5]>,
        expected_entry: u32,
        expected_args: [u32; 5],
    }

    impl SpatialHleHost for Host {
        fn completion_enabled(&mut self) -> bool {
            self.events.borrow_mut().push("completion");
            self.completion
        }

        fn decode(
            &mut self,
            _context: *mut c_void,
            entry: u32,
            completion_enabled: bool,
        ) -> Option<[u32; 5]> {
            assert_eq!(entry, self.expected_entry);
            assert_eq!(completion_enabled, self.completion);
            self.events.borrow_mut().push("decode");
            self.decoded
        }

        fn apply(&mut self, entry: u32, args: &[u32; 5]) -> u32 {
            assert_eq!(entry, self.expected_entry);
            self.events.borrow_mut().push("apply");
            assert_eq!(args, &self.expected_args);
            0x1234
        }

        fn refuse(&mut self, _entry: u32, reason: u32) -> ! {
            assert_eq!(reason, REFUSE_DECODE);
            panic!("expected fatal refusal")
        }
    }

    #[test]
    fn stream_spatial_frames_preserve_measured_callers_and_words() {
        let memory = Memory([0x29aca, 0x1111, 0x2222, 0x3333, 0x4444, 0x5555]);
        assert_eq!(
            decode_guest_frame(&memory, SET_POSITION, 0x1000, 0x1018, false),
            Some([0x1111, 0x2222, 0x3333, 0x4444, 0x5555])
        );
        let memory = Memory([0x29d86, 0xaaaa, 0xbbbb, 0xcccc, 0, 0]);
        assert_eq!(
            decode_guest_frame(&memory, MAX_DISTANCE, 0x1000, 0x1010, false),
            Some([0xaaaa, 0xbbbb, 0xcccc, 0, 0])
        );
        let memory = Memory([0x29998, 0xdddd, 0xeeee, 0xffff, 0, 0]);
        assert_eq!(
            decode_guest_frame(&memory, MINIMUM, 0x1000, 0x1010, false),
            Some([0xdddd, 0xeeee, 0xffff, 0, 0])
        );
        let memory = Memory([0x2998a, 0xaaaa, 0xbbbb, 1, 0, 0]);
        assert_eq!(
            decode_guest_frame(&memory, I3DL2, 0x1000, 0x1010, false),
            Some([0xaaaa, 0xbbbb, 1, 0, 0])
        );
        for caller in [0x299a8, 0x29e81, 0x29e90] {
            let memory = Memory([caller, 0x1111, 0x2222, 0x3333, 0x4444, 0]);
            assert_eq!(
                decode_guest_frame(&memory, ROLLOFF, 0x1000, 0x1014, false),
                Some([0x1111, 0x2222, 0x3333, 0x4444, 0])
            );
        }
    }

    #[test]
    fn spatial_frames_reject_unmeasured_callers_and_short_stack_limits() {
        let unmeasured = Memory([0x1234, 1, 2, 3, 4, 5]);
        assert_eq!(
            decode_guest_frame(&unmeasured, SET_POSITION, 0x1000, 0x1018, false),
            None
        );
        let short = Memory([0x29aca, 1, 2, 3, 4, 5]);
        assert_eq!(
            decode_guest_frame(&short, SET_POSITION, 0x1000, 0x1014, false),
            None
        );
        let unmeasured_rolloff = Memory([0x1234, 1, 2, 3, 4, 0]);
        assert_eq!(
            decode_guest_frame(&unmeasured_rolloff, ROLLOFF, 0x1000, 0x1014, false),
            None
        );
        let short_rolloff = Memory([0x29e81, 1, 2, 3, 4, 0]);
        assert_eq!(
            decode_guest_frame(&short_rolloff, ROLLOFF, 0x1000, 0x1010, false),
            None
        );
    }

    #[test]
    fn rolloff_handler_keeps_snapshot_decode_apply_order() {
        let mut host = Host {
            events: RefCell::new(Vec::new()),
            completion: true,
            decoded: Some([1, 2, 3, 4, 5]),
            expected_entry: ROLLOFF,
            expected_args: [1, 2, 3, 4, 5],
        };
        assert_eq!(dispatch(&mut host, std::ptr::null_mut(), ROLLOFF), 0x1234);
        assert_eq!(*host.events.borrow(), ["completion", "decode", "apply"]);
    }

    #[test]
    fn minimum_handler_keeps_snapshot_decode_apply_order() {
        let mut host = Host {
            events: RefCell::new(Vec::new()),
            completion: false,
            decoded: Some([7, 8, 9, 0, 0]),
            expected_entry: MINIMUM,
            expected_args: [7, 8, 9, 0, 0],
        };
        assert_eq!(dispatch(&mut host, std::ptr::null_mut(), MINIMUM), 0x1234);
        assert_eq!(*host.events.borrow(), ["completion", "decode", "apply"]);
    }
}
