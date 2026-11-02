//! Rust-owned stream Pause and FlushEx HLE callbacks for the Rust audio host.
use std::ffi::c_void;
use tsfp_audio::device_frames::CallFrame;
use tsfp_audio::frames::{FLUSH_EX, FrameCall, PAUSE, SET_FORMAT, decode_frame};

use crate::frame_guest_memory::FrameGuestMemory;

type DsoundHandler = unsafe extern "C" fn(*mut c_void) -> u32;

const REFUSE_DECODE: u32 = 1;
const REFUSE_CALLER_CHANGED: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
struct KernelCallFramePrefix {
    stack_pointer: u32,
    stack_limit: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Prepared {
    completion_enabled: bool,
    caller: Option<u32>,
    extension_result: Option<u32>,
}

trait ControlHleHost {
    fn prepare(&mut self, entry: u32, context: *mut c_void) -> Prepared;
    fn decode(&mut self, context: *mut c_void, entry: u32, completion: bool) -> Option<FrameCall>;
    fn apply(&mut self, entry: u32, call: &FrameCall) -> u32;
    fn refuse(&mut self, entry: u32, reason: u32) -> !;
}

struct HostControlHle;

unsafe extern "C" {
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn dsound_stream_control_hle_prepare_rust(
        entry: u32,
        context: *mut c_void,
        completion_out: *mut bool,
        caller_out: *mut u32,
        result_out: *mut u32,
    ) -> bool;
    fn dsound_stream_control_hle_guest_pid_rust() -> i32;
    fn dsound_stream_control_hle_apply_rust(entry: u32, caller: u32, args: *const u32) -> u32;
    fn dsound_stream_control_hle_refuse_rust(entry: u32, reason: u32) -> !;
}

impl ControlHleHost for HostControlHle {
    fn prepare(&mut self, entry: u32, context: *mut c_void) -> Prepared {
        let mut completion = false;
        let mut caller = 0;
        let mut extension_result = 0;
        // SAFETY: C snapshots the stream policy and movie extension route under its lock.
        let handled = unsafe {
            dsound_stream_control_hle_prepare_rust(
                entry,
                context,
                &mut completion,
                &mut caller,
                &mut extension_result,
            )
        };
        Prepared {
            completion_enabled: completion,
            caller: (caller != 0).then_some(caller),
            extension_result: handled.then_some(extension_result),
        }
    }

    fn decode(&mut self, context: *mut c_void, entry: u32, completion: bool) -> Option<FrameCall> {
        if context.is_null() {
            return None;
        }
        // SAFETY: C passes the live kernel frame to the registered HLE callback.
        let prefix = unsafe { context.cast::<KernelCallFramePrefix>().read() };
        // SAFETY: guest stack words are read from the same host process used by kernel services.
        let host_pid = unsafe { dsound_stream_control_hle_guest_pid_rust() };
        if host_pid <= 0 {
            return None;
        }
        let memory = FrameGuestMemory::new(host_pid);
        decode_frame(
            &memory,
            entry,
            CallFrame {
                stack_pointer: prefix.stack_pointer,
                stack_limit: prefix.stack_limit,
            },
            completion,
        )
        .ok()
    }

    fn apply(&mut self, entry: u32, call: &FrameCall) -> u32 {
        // SAFETY: C keeps exact stream/device validation and applies through the Rust lease model.
        unsafe { dsound_stream_control_hle_apply_rust(entry, call.caller, call.args.as_ptr()) }
    }

    fn refuse(&mut self, entry: u32, reason: u32) -> ! {
        // SAFETY: C reports this refusal through the existing fatal HLE path.
        unsafe { dsound_stream_control_hle_refuse_rust(entry, reason) }
    }
}

fn dispatch(host: &mut impl ControlHleHost, context: *mut c_void, entry: u32) -> u32 {
    let prepared = host.prepare(entry, context);
    if let Some(result) = prepared.extension_result {
        return result;
    }
    let Some(call) = host.decode(context, entry, prepared.completion_enabled) else {
        host.refuse(entry, REFUSE_DECODE);
    };
    if prepared.caller.is_some_and(|caller| caller != call.caller) {
        host.refuse(entry, REFUSE_CALLER_CHANGED);
    }
    host.apply(entry, &call)
}

unsafe extern "C" fn pause_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostControlHle, context, PAUSE)
}

unsafe extern "C" fn flush_ex_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostControlHle, context, FLUSH_EX)
}

unsafe extern "C" fn set_format_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostControlHle, context, SET_FORMAT)
}

fn register(entry: u32, handler: DsoundHandler) -> bool {
    if !crate::movie_device_hle_ffi::tsfp_audio_hle_register_handler_rust(entry, handler) {
        return false;
    }
    // SAFETY: C initializes its measured HLE surface before stream registration.
    unsafe { dsound_hle_register_rust(entry) }
}

/// Register Rust's measured stream Pause and FlushEx callbacks.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_control_register_rust() -> usize {
    register(PAUSE, pause_handler) as usize + register(FLUSH_EX, flush_ex_handler) as usize
}

/// Register Rust's measured stream SetFormat callback when the stream policy is enabled.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_format_register_rust() -> usize {
    register(SET_FORMAT, set_format_handler) as usize
}

#[cfg(test)]
fn decode_test_frame(
    memory: &impl tsfp_audio::adapter::GuestMemory,
    entry: u32,
    stack_pointer: u32,
    stack_limit: u32,
    completion: bool,
) -> Option<FrameCall> {
    decode_frame(
        memory,
        entry,
        CallFrame {
            stack_pointer,
            stack_limit,
        },
        completion,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::{
        ControlHleHost, FLUSH_EX, FrameCall, PAUSE, Prepared, REFUSE_CALLER_CHANGED, REFUSE_DECODE,
        SET_FORMAT, decode_test_frame, dispatch,
    };
    use std::cell::RefCell;
    use std::ffi::c_void;
    use tsfp_audio::adapter::GuestMemory;

    // The standalone Rust test binary does not link the C audio host.
    #[unsafe(no_mangle)]
    extern "C" fn dsound_stream_control_hle_guest_pid_rust() -> i32 {
        0
    }

    #[test]
    fn null_frame_is_refused_before_reading_it() {
        let mut host = super::HostControlHle;
        assert!(host.decode(std::ptr::null_mut(), SET_FORMAT, false).is_none());
    }

    #[test]
    fn unavailable_guest_process_is_refused() {
        let mut host = super::HostControlHle;
        let mut frame = super::KernelCallFramePrefix {
            stack_pointer: 0x1000,
            stack_limit: 0x2000,
        };
        assert!(
            host.decode(
                (&mut frame as *mut super::KernelCallFramePrefix).cast(),
                SET_FORMAT,
                false,
            )
            .is_none()
        );
    }

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
        prepared: Prepared,
        decoded: Option<FrameCall>,
        expected_entry: u32,
    }

    impl ControlHleHost for Host {
        fn prepare(&mut self, entry: u32, _context: *mut c_void) -> Prepared {
            assert_eq!(entry, self.expected_entry);
            self.events.borrow_mut().push("prepare");
            self.prepared
        }

        fn decode(
            &mut self,
            _context: *mut c_void,
            entry: u32,
            completion: bool,
        ) -> Option<FrameCall> {
            assert_eq!(entry, self.expected_entry);
            assert_eq!(completion, self.prepared.completion_enabled);
            self.events.borrow_mut().push("decode");
            self.decoded
        }

        fn apply(&mut self, entry: u32, call: &FrameCall) -> u32 {
            assert_eq!(entry, self.expected_entry);
            self.events.borrow_mut().push("apply");
            assert_eq!(call, self.decoded.as_ref().expect("decoded call"));
            0x1234
        }

        fn refuse(&mut self, _entry: u32, reason: u32) -> ! {
            assert!(reason == REFUSE_DECODE || reason == REFUSE_CALLER_CHANGED);
            panic!("expected refusal {reason}")
        }
    }

    #[test]
    fn pause_decodes_measured_startup_and_completion_callers() {
        for (caller, completion) in [
            (0x29b5a, false),
            (0x29a51, false),
            (0x29f13, true),
            (0x29ed1, true),
        ] {
            let memory = Memory([caller, 0x1111, 0x2222, 0, 0, 0]);
            let call = decode_test_frame(&memory, PAUSE, 0x1000, 0x100c, completion)
                .expect("measured Pause frame");
            assert_eq!(call.caller, caller);
            assert_eq!(&call.args[..2], &[0x1111, 0x2222]);
        }
    }

    #[test]
    fn flush_ex_decodes_four_measured_words_and_rejects_bad_frames() {
        let memory = Memory([0x29b69, 1, 2, 3, 4, 0]);
        let call = decode_test_frame(&memory, FLUSH_EX, 0x1000, 0x1014, false)
            .expect("measured FlushEx frame");
        assert_eq!(call.caller, 0x29b69);
        assert_eq!(&call.args[..4], &[1, 2, 3, 4]);

        let unmeasured = Memory([0x1234, 1, 2, 3, 4, 0]);
        assert!(decode_test_frame(&unmeasured, FLUSH_EX, 0x1000, 0x1014, false).is_none());
        let runtime_without_completion = Memory([0x29ed1, 1, 2, 0, 0, 0]);
        assert!(
            decode_test_frame(&runtime_without_completion, PAUSE, 0x1000, 0x100c, false).is_none()
        );
        let short = Memory([0x29b69, 1, 2, 3, 4, 0]);
        assert!(decode_test_frame(&short, FLUSH_EX, 0x1000, 0x1010, false).is_none());
    }

    #[test]
    fn set_format_decodes_its_measured_caller_and_two_arguments() {
        let memory = Memory([0x299fa, 0x1111, 0x2222, 0, 0, 0]);
        let call = decode_test_frame(&memory, SET_FORMAT, 0x1000, 0x100c, false)
            .expect("measured SetFormat frame");
        assert_eq!(call.caller, 0x299fa);
        assert_eq!(&call.args[..2], &[0x1111, 0x2222]);

        let unmeasured = Memory([0x299fb, 0x1111, 0x2222, 0, 0, 0]);
        assert!(decode_test_frame(&unmeasured, SET_FORMAT, 0x1000, 0x100c, false).is_none());
        assert!(decode_test_frame(&memory, SET_FORMAT, 0x1000, 0x1008, false).is_none());
    }

    #[test]
    fn control_hle_prepares_extension_before_decode_then_applies() {
        let call = FrameCall {
            entry: PAUSE,
            caller: 0x29a51,
            args: [0x1111, 1, 0, 0, 0],
        };
        let mut host = Host {
            events: RefCell::new(Vec::new()),
            prepared: Prepared {
                completion_enabled: false,
                caller: Some(call.caller),
                extension_result: None,
            },
            decoded: Some(call),
            expected_entry: PAUSE,
        };
        assert_eq!(dispatch(&mut host, std::ptr::null_mut(), PAUSE), 0x1234);
        assert_eq!(*host.events.borrow(), ["prepare", "decode", "apply"]);
    }

    #[test]
    fn set_format_movie_extension_short_circuits_decode_and_apply() {
        let mut host = Host {
            events: RefCell::new(Vec::new()),
            prepared: Prepared {
                completion_enabled: false,
                caller: Some(0x299fa),
                extension_result: Some(0x5678),
            },
            decoded: None,
            expected_entry: SET_FORMAT,
        };
        assert_eq!(
            dispatch(&mut host, std::ptr::null_mut(), SET_FORMAT),
            0x5678
        );
        assert_eq!(*host.events.borrow(), ["prepare"]);
    }

    #[test]
    fn movie_extension_result_short_circuits_guest_decode_and_apply() {
        let mut host = Host {
            events: RefCell::new(Vec::new()),
            prepared: Prepared {
                completion_enabled: true,
                caller: Some(0x29f13),
                extension_result: Some(0x5678),
            },
            decoded: None,
            expected_entry: PAUSE,
        };
        assert_eq!(dispatch(&mut host, std::ptr::null_mut(), PAUSE), 0x5678);
        assert_eq!(*host.events.borrow(), ["prepare"]);
    }

    #[test]
    #[should_panic(expected = "expected refusal")]
    fn control_hle_refuses_changed_caller_before_apply() {
        let mut host = Host {
            events: RefCell::new(Vec::new()),
            prepared: Prepared {
                completion_enabled: false,
                caller: Some(0x29b5a),
                extension_result: None,
            },
            decoded: Some(FrameCall {
                entry: PAUSE,
                caller: 0x29a51,
                args: [1, 2, 0, 0, 0],
            }),
            expected_entry: PAUSE,
        };
        let _ = dispatch(&mut host, std::ptr::null_mut(), PAUSE);
        assert_eq!(*host.events.borrow(), ["prepare", "decode"]);
    }
}
