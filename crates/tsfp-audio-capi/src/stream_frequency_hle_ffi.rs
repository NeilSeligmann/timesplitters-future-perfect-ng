//! Rust-owned measured stream SetFrequency callback for the Rust audio host.
use std::ffi::c_void;
use tsfp_audio::device_frames::CallFrame;
use tsfp_audio::frames::{FrameCall, SET_FREQUENCY, decode_frame};

use crate::frame_guest_memory::FrameGuestMemory;
use crate::stream_routing_hle_ffi::read_global_audio_state;

const REFUSE_DECODE: u32 = 1;
const REFUSE_CONTROL_WORD: u32 = 3;
const REFUSE_POLICY: u32 = 4;
const REFUSE_GUEST_STATE: u32 = 5;
const E_FAIL: u32 = 0x8000_4005;

#[repr(C)]
#[derive(Clone, Copy)]
struct KernelCallFramePrefix {
    stack_pointer: u32,
    stack_limit: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FrequencyPolicyInputs {
    enabled: u32,
    completion: u32,
    handler_available: u32,
    irql_known: u32,
    irql: u32,
}

impl FrequencyPolicyInputs {
    fn admits(self) -> bool {
        self.enabled != 0
            && self.completion != 0
            && self.handler_available != 0
            && self.irql_known != 0
            && self.irql == 0
    }
}

trait FrequencyHleHost {
    fn policy_inputs(&mut self) -> Option<FrequencyPolicyInputs>;
    fn guest_state(&mut self) -> Option<u32>;
    fn decode(&mut self, context: *mut c_void) -> Option<FrameCall>;
    fn control_word(&mut self) -> Option<u16>;
    fn apply(&mut self, call: &FrameCall, control_word: u16) -> u32;
    fn refuse(&mut self, reason: u32) -> !;
}

struct HostFrequencyHle;

unsafe extern "C" {
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn dsound_stream_frequency_hle_policy_inputs_rust(output: *mut FrequencyPolicyInputs) -> bool;
    fn dsound_stream_frequency_hle_guest_pid_rust() -> i32;
    fn dsound_stream_frequency_hle_control_word_rust(output: *mut u16) -> bool;
    fn dsound_stream_frequency_hle_apply_rust(stream: u32, hertz: u32, control_word: u16) -> u32;
    fn dsound_stream_control_hle_refuse_rust(entry: u32, reason: u32) -> !;
}

impl FrequencyHleHost for HostFrequencyHle {
    fn policy_inputs(&mut self) -> Option<FrequencyPolicyInputs> {
        let mut inputs = FrequencyPolicyInputs::default();
        // SAFETY: C snapshots synchronized policy fields and current IRQL into this C-layout record.
        unsafe { dsound_stream_frequency_hle_policy_inputs_rust(&mut inputs).then_some(inputs) }
    }

    fn guest_state(&mut self) -> Option<u32> {
        // SAFETY: this is the DMTCP-aware identity-mapped process selected for the live guest.
        let host_pid = unsafe { dsound_stream_frequency_hle_guest_pid_rust() };
        if host_pid <= 0 {
            return None;
        }
        read_global_audio_state(&FrameGuestMemory::new(host_pid))
    }

    fn decode(&mut self, context: *mut c_void) -> Option<FrameCall> {
        if context.is_null() {
            return None;
        }
        // SAFETY: the C HLE dispatcher passes a live kernel_call_frame with this stable prefix.
        let prefix = unsafe { context.cast::<KernelCallFramePrefix>().read() };
        // SAFETY: this identifies the process whose identity-mapped guest stack is active.
        let host_pid = unsafe { dsound_stream_frequency_hle_guest_pid_rust() };
        if host_pid <= 0 {
            return None;
        }
        let memory = FrameGuestMemory::new(host_pid);
        decode_frame(
            &memory,
            SET_FREQUENCY,
            CallFrame {
                stack_pointer: prefix.stack_pointer,
                stack_limit: prefix.stack_limit,
            },
            false,
        )
        .ok()
    }

    fn control_word(&mut self) -> Option<u16> {
        let mut value = 0;
        // SAFETY: C reads the guest control word under its existing stream synchronization.
        unsafe { dsound_stream_frequency_hle_control_word_rust(&mut value).then_some(value) }
    }

    fn apply(&mut self, call: &FrameCall, control_word: u16) -> u32 {
        // SAFETY: C performs the same exact-object/lease access transaction as the prior handler.
        unsafe { dsound_stream_frequency_hle_apply_rust(call.args[0], call.args[1], control_word) }
    }

    fn refuse(&mut self, reason: u32) -> ! {
        // SAFETY: C routes refusals through the established fatal HLE path.
        unsafe { dsound_stream_control_hle_refuse_rust(SET_FREQUENCY, reason) }
    }
}

fn dispatch(host: &mut impl FrequencyHleHost, context: *mut c_void) -> u32 {
    let Some(inputs) = host.policy_inputs() else {
        host.refuse(REFUSE_POLICY);
    };
    if !inputs.admits() {
        host.refuse(REFUSE_POLICY);
    }
    let Some(global_state) = host.guest_state() else {
        host.refuse(REFUSE_GUEST_STATE);
    };
    if global_state != 0 {
        return E_FAIL;
    }
    let Some(call) = host.decode(context) else {
        host.refuse(REFUSE_DECODE);
    };
    let Some(control_word) = host.control_word() else {
        host.refuse(REFUSE_CONTROL_WORD);
    };
    host.apply(&call, control_word)
}

unsafe extern "C" fn set_frequency_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostFrequencyHle, context)
}

/// Register Rust's measured SetFrequency callback after C enables its PCM/completion policy.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_frequency_register_rust() -> usize {
    let selected = crate::movie_device_hle_ffi::tsfp_audio_hle_register_handler_rust(
        SET_FREQUENCY,
        set_frequency_handler,
    );
    if !selected {
        return 0;
    }
    // SAFETY: C accepts only addresses in its fixed measured DirectSound HLE surface.
    unsafe { dsound_hle_register_rust(SET_FREQUENCY) as usize }
}

#[cfg(test)]
mod tests {
    use super::{
        E_FAIL, FrequencyHleHost, FrequencyPolicyInputs, dispatch, read_global_audio_state,
    };
    use crate::stream_routing_hle_ffi::GLOBAL_STATE_ADDRESS;
    use std::cell::RefCell;
    use std::ffi::c_void;
    use tsfp_audio::adapter::GuestMemory;
    use tsfp_audio::device_frames::CallFrame;
    use tsfp_audio::frames::{FrameCall, SET_FREQUENCY, decode_frame};

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

    #[test]
    fn measured_frequency_frame_decodes_caller_and_two_words() {
        let memory = Memory([0x29B35, 0x1234, 44_100]);
        let call = decode_frame(
            &memory,
            SET_FREQUENCY,
            CallFrame {
                stack_pointer: 0x1000,
                stack_limit: 0x100c,
            },
            false,
        )
        .expect("measured SetFrequency frame");
        assert_eq!(call.caller, 0x29B35);
        assert_eq!(call.args[..2], [0x1234, 44_100]);
        assert!(
            decode_frame(
                &Memory([0x29B36, 0x1234, 44_100]),
                SET_FREQUENCY,
                CallFrame {
                    stack_pointer: 0x1000,
                    stack_limit: 0x100c,
                },
                false,
            )
            .is_err()
        );
        assert!(
            decode_frame(
                &memory,
                SET_FREQUENCY,
                CallFrame {
                    stack_pointer: 0x1000,
                    stack_limit: 0x1008,
                },
                false,
            )
            .is_err()
        );
    }

    struct FakeHost {
        order: RefCell<Vec<&'static str>>,
        policy: Option<FrequencyPolicyInputs>,
        global_state: Option<u32>,
        decoded: Option<FrameCall>,
        control: Option<u16>,
        applied: RefCell<Option<(u32, u32, u16)>>,
    }

    impl FrequencyHleHost for FakeHost {
        fn policy_inputs(&mut self) -> Option<FrequencyPolicyInputs> {
            self.order.borrow_mut().push("policy");
            self.policy
        }

        fn guest_state(&mut self) -> Option<u32> {
            self.order.borrow_mut().push("guest-state");
            self.global_state
        }

        fn decode(&mut self, _context: *mut c_void) -> Option<FrameCall> {
            self.order.borrow_mut().push("decode");
            self.decoded
        }

        fn control_word(&mut self) -> Option<u16> {
            self.order.borrow_mut().push("control-word");
            self.control
        }

        fn apply(&mut self, call: &FrameCall, control_word: u16) -> u32 {
            self.order.borrow_mut().push("apply");
            *self.applied.borrow_mut() = Some((call.args[0], call.args[1], control_word));
            0x8000_0001
        }

        fn refuse(&mut self, reason: u32) -> ! {
            panic!("frequency refused with reason {reason}")
        }
    }

    fn fake(
        decoded: Option<FrameCall>,
        control: Option<u16>,
        global_state: Option<u32>,
    ) -> FakeHost {
        FakeHost {
            order: RefCell::new(Vec::new()),
            policy: Some(FrequencyPolicyInputs {
                enabled: 1,
                completion: 1,
                handler_available: 1,
                irql_known: 1,
                irql: 0,
            }),
            global_state,
            decoded,
            control,
            applied: RefCell::new(None),
        }
    }

    fn measured_call() -> FrameCall {
        FrameCall {
            entry: SET_FREQUENCY,
            caller: 0x29B35,
            args: [0x1234, 44_100, 0, 0, 0],
        }
    }

    #[test]
    fn dispatch_preserves_policy_global_decode_control_word_and_apply_order() {
        let mut host = fake(Some(measured_call()), Some(0x027f), Some(0));
        assert_eq!(dispatch(&mut host, std::ptr::null_mut()), 0x8000_0001);
        assert_eq!(
            *host.order.borrow(),
            ["policy", "guest-state", "decode", "control-word", "apply"]
        );
        assert_eq!(*host.applied.borrow(), Some((0x1234, 44_100, 0x027f)));
    }

    #[test]
    fn policy_refusal_short_circuits_guest_state_and_frame_reads() {
        for policy in [
            FrequencyPolicyInputs {
                enabled: 0,
                completion: 1,
                handler_available: 1,
                irql_known: 1,
                irql: 0,
            },
            FrequencyPolicyInputs {
                enabled: 1,
                completion: 0,
                handler_available: 1,
                irql_known: 1,
                irql: 0,
            },
            FrequencyPolicyInputs {
                enabled: 1,
                completion: 1,
                handler_available: 0,
                irql_known: 1,
                irql: 0,
            },
            FrequencyPolicyInputs {
                enabled: 1,
                completion: 1,
                handler_available: 1,
                irql_known: 0,
                irql: 0,
            },
            FrequencyPolicyInputs {
                enabled: 1,
                completion: 1,
                handler_available: 1,
                irql_known: 1,
                irql: 2,
            },
        ] {
            let mut host = fake(None, None, Some(0));
            host.policy = Some(policy);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                dispatch(&mut host, std::ptr::null_mut())
            }));
            assert!(result.is_err());
            assert_eq!(*host.order.borrow(), ["policy"]);
        }
    }

    #[test]
    fn frequency_global_state_uses_the_shared_guarded_guest_word() {
        struct StateMemory {
            read_address: std::cell::Cell<u32>,
            value: Option<u32>,
        }
        impl GuestMemory for StateMemory {
            fn read_u32(&self, address: u32) -> Option<u32> {
                self.read_address.set(address);
                self.value
            }
            fn write_u32(&mut self, _: u32, _: u32) -> bool {
                false
            }
            fn read_bytes(&self, _: u32, _: usize) -> Option<Vec<u8>> {
                None
            }
            fn range_readable(&self, _: u32, _: u32) -> bool {
                false
            }
        }

        let memory = StateMemory {
            read_address: std::cell::Cell::new(0),
            value: Some(0),
        };
        assert_eq!(read_global_audio_state(&memory), Some(0));
        assert_eq!(memory.read_address.get(), GLOBAL_STATE_ADDRESS);
    }

    #[test]
    fn nonzero_global_state_returns_hresult_before_frame_reads() {
        let mut host = fake(None, None, Some(1));
        assert_eq!(dispatch(&mut host, std::ptr::null_mut()), E_FAIL);
        assert_eq!(*host.order.borrow(), ["policy", "guest-state"]);
        assert_eq!(*host.applied.borrow(), None);
    }

    #[test]
    fn inaccessible_global_state_refuses_before_frame_reads() {
        let mut host = fake(None, None, None);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatch(&mut host, std::ptr::null_mut())
        }));
        assert!(result.is_err());
        assert_eq!(*host.order.borrow(), ["policy", "guest-state"]);
        assert_eq!(*host.applied.borrow(), None);
    }

    #[test]
    fn missing_control_word_refuses_before_apply() {
        let mut host = fake(Some(measured_call()), None, Some(0));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatch(&mut host, std::ptr::null_mut())
        }));
        assert!(result.is_err());
        assert_eq!(
            *host.order.borrow(),
            ["policy", "guest-state", "decode", "control-word"]
        );
        assert_eq!(*host.applied.borrow(), None);
    }
}
