//! Rust-owned HLE callbacks for the measured DirectSound stream-routing entries.
use std::ffi::c_void;
use tsfp_audio::adapter::GuestMemory;
use tsfp_audio::device_frames::CallFrame;
use tsfp_audio::frames::decode_frame;

use crate::frame_guest_memory::FrameGuestMemory;

type DsoundHandler = unsafe extern "C" fn(*mut c_void) -> u32;

const HEADROOM: u32 = 0x0040_7B19;
const MIX_VOLUMES: u32 = 0x0040_7B1E;
const MIX_BINS: u32 = 0x0040_85D4;
const E_FAIL: u32 = 0x8000_4005;
pub(crate) const GLOBAL_STATE_ADDRESS: u32 = 0x0041_24A8;
const REFUSE_POLICY_SERVICE: u32 = 1;
const REFUSE_POLICY: u32 = 2;
const REFUSE_GLOBAL_STATE: u32 = 3;
const REFUSE_DECODE: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RoutingPolicySnapshot {
    admitted: bool,
    completion_enabled: bool,
    irql_known: bool,
    irql: u8,
}

/// Prefix of `kernel_call_frame` from `src/xbox/kernel_call.h`.
/// C asserts both offsets at the routing HLE adapter: the remaining register/result
/// fields are irrelevant to these stdcall entries.
#[repr(C)]
#[derive(Clone, Copy)]
struct KernelCallFramePrefix {
    stack_pointer: u32,
    stack_limit: u32,
}

unsafe extern "C" {
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn dsound_stream_routing_hle_policy_snapshot_rust(
        admitted_out: *mut u32,
        completion_enabled_out: *mut u32,
        irql_known_out: *mut u32,
        irql_out: *mut u32,
    ) -> bool;
    fn dsound_stream_routing_hle_guest_pid_rust() -> i32;
    fn dsound_stream_routing_hle_apply_rust(entry: u32, stream: u32, argument: u32) -> u32;
    fn dsound_stream_routing_hle_refuse_rust(entry: u32, reason: u32) -> !;
}

trait RoutingHleHost {
    fn policy_snapshot(&mut self, entry: u32) -> Option<RoutingPolicySnapshot>;
    fn global_state(&mut self) -> Option<u32>;
    fn decode(
        &mut self,
        context: *mut c_void,
        entry: u32,
        completion_enabled: bool,
    ) -> Option<(u32, u32)>;
    fn apply(&mut self, entry: u32, stream: u32, argument: u32) -> u32;
    fn refuse(&mut self, entry: u32, reason: u32) -> !;
}

struct HostRoutingHle;

impl RoutingHleHost for HostRoutingHle {
    fn policy_snapshot(&mut self, _entry: u32) -> Option<RoutingPolicySnapshot> {
        let mut admitted = 0;
        let mut completion_enabled = 0;
        let mut irql_known = 0;
        let mut irql = 0;
        // SAFETY: C snapshots synchronized host policy and IRQL providers without reading guest arguments.
        let available = unsafe {
            dsound_stream_routing_hle_policy_snapshot_rust(
                &mut admitted,
                &mut completion_enabled,
                &mut irql_known,
                &mut irql,
            )
        };
        available.then_some(RoutingPolicySnapshot {
            admitted: admitted != 0,
            completion_enabled: completion_enabled != 0,
            irql_known: irql_known != 0,
            irql: irql.min(u32::from(u8::MAX)) as u8,
        })
    }

    fn global_state(&mut self) -> Option<u32> {
        // SAFETY: the C service supplies the same host PID used by the live frame decoder.
        let host_pid = unsafe { dsound_stream_routing_hle_guest_pid_rust() };
        (host_pid > 0)
            .then(|| read_global_audio_state(&FrameGuestMemory::new(host_pid)))
            .flatten()
    }

    fn decode(
        &mut self,
        context: *mut c_void,
        entry: u32,
        completion_enabled: bool,
    ) -> Option<(u32, u32)> {
        // SAFETY: the host PID is the process whose identity-mapped guest stack is live.
        let host_pid = unsafe { dsound_stream_routing_hle_guest_pid_rust() };
        // SAFETY: the registered callback context is the live kernel_call_frame.
        unsafe { decode_routing_frame(context, entry, completion_enabled, host_pid) }
    }

    fn apply(&mut self, entry: u32, stream: u32, argument: u32) -> u32 {
        // SAFETY: the C service validates object identity and enters the typed route transaction,
        // whose exact-generation state and candidate are Rust-owned.
        unsafe { dsound_stream_routing_hle_apply_rust(entry, stream, argument) }
    }

    fn refuse(&mut self, entry: u32, reason: u32) -> ! {
        // SAFETY: C maps this typed refusal reason to the existing fatal HLE reporting path.
        unsafe { dsound_stream_routing_hle_refuse_rust(entry, reason) }
    }
}

pub(crate) fn read_global_audio_state(memory: &impl GuestMemory) -> Option<u32> {
    memory.read_u32(GLOBAL_STATE_ADDRESS)
}

unsafe fn decode_routing_frame(
    context: *mut c_void,
    entry: u32,
    completion_enabled: bool,
    host_pid: i32,
) -> Option<(u32, u32)> {
    if context.is_null() || host_pid <= 0 {
        return None;
    }
    // SAFETY: the registered HLE callback supplies a live kernel_call_frame. C's
    // static assertions bind the first two u32 fields to this prefix type.
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
    Some((call.args[0], call.args[1]))
}

fn dispatch<H: RoutingHleHost>(host: &mut H, context: *mut c_void, entry: u32) -> u32 {
    let Some(policy) = host.policy_snapshot(entry) else {
        host.refuse(entry, REFUSE_POLICY_SERVICE);
    };
    if !policy.admitted || !policy.irql_known || policy.irql != 0 {
        host.refuse(entry, REFUSE_POLICY);
    }
    let Some(global_state) = host.global_state() else {
        host.refuse(entry, REFUSE_GLOBAL_STATE);
    };
    if global_state != 0 {
        return E_FAIL;
    }
    let Some((stream, argument)) = host.decode(context, entry, policy.completion_enabled) else {
        host.refuse(entry, REFUSE_DECODE);
    };
    host.apply(entry, stream, argument)
}

unsafe extern "C" fn headroom_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostRoutingHle, context, HEADROOM)
}

unsafe extern "C" fn mix_volumes_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostRoutingHle, context, MIX_VOLUMES)
}

unsafe extern "C" fn mix_bins_handler(context: *mut c_void) -> u32 {
    dispatch(&mut HostRoutingHle, context, MIX_BINS)
}

fn register(entry: u32, handler: DsoundHandler) -> bool {
    if !crate::movie_device_hle_ffi::tsfp_audio_hle_register_handler_rust(entry, handler) {
        return false;
    }
    // SAFETY: the C measured surface is initialized before stream registration.
    unsafe { dsound_hle_register_rust(entry) }
}

/// Register Rust callbacks for the three measured two-argument routing HLE entries.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_routing_register_rust() -> usize {
    register(HEADROOM, headroom_handler) as usize
        + register(MIX_BINS, mix_bins_handler) as usize
        + register(MIX_VOLUMES, mix_volumes_handler) as usize
}

#[cfg(test)]
mod tests {
    use super::{
        E_FAIL, GLOBAL_STATE_ADDRESS, HEADROOM, KernelCallFramePrefix, MIX_BINS, MIX_VOLUMES,
        REFUSE_DECODE, REFUSE_GLOBAL_STATE, REFUSE_POLICY, REFUSE_POLICY_SERVICE, RoutingHleHost,
        RoutingPolicySnapshot, decode_routing_frame, dispatch, read_global_audio_state,
    };
    use std::cell::Cell;
    use std::ffi::c_void;
    use tsfp_audio::adapter::GuestMemory;

    struct GlobalStateMemory {
        read_address: Cell<u32>,
        value: Option<u32>,
    }

    impl GuestMemory for GlobalStateMemory {
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

    #[test]
    fn reads_only_the_fixed_original_audio_state_word() {
        let memory = GlobalStateMemory {
            read_address: Cell::new(0),
            value: Some(7),
        };
        assert_eq!(read_global_audio_state(&memory), Some(7));
        assert_eq!(memory.read_address.get(), GLOBAL_STATE_ADDRESS);
    }

    #[test]
    fn unreadable_original_audio_state_is_a_refusal() {
        let memory = GlobalStateMemory {
            read_address: Cell::new(0),
            value: None,
        };
        assert_eq!(read_global_audio_state(&memory), None);
        assert_eq!(memory.read_address.get(), GLOBAL_STATE_ADDRESS);
    }

    #[test]
    fn routing_frame_is_decoded_in_rust_with_the_supplied_stack_bound() {
        let (page, stack) = crate::test_guest_memory::map_guest_page();
        // SAFETY: this test owns the low-address writable page for the decoder call.
        unsafe {
            page.cast::<u32>().write(0x1234_5678);
            page.cast::<u32>().add(1).write(0xAABB_CCDD);
            page.cast::<u32>().add(2).write(0x0102_0304);
        }
        let mut frame = KernelCallFramePrefix {
            stack_pointer: stack,
            stack_limit: stack + 12,
        };
        let context = std::ptr::from_ref(&frame).cast_mut().cast::<c_void>();
        // SAFETY: frame prefix and guest page are live for this call.
        let decoded =
            unsafe { decode_routing_frame(context, HEADROOM, false, std::process::id() as i32) };
        assert_eq!(decoded, Some((0xAABB_CCDD, 0x0102_0304)));

        frame.stack_limit = stack + 8;
        // SAFETY: same live frame, but its exclusive bound omits argument 1.
        let bounded = unsafe {
            decode_routing_frame(
                std::ptr::from_ref(&frame).cast_mut().cast::<c_void>(),
                HEADROOM,
                false,
                std::process::id() as i32,
            )
        };
        assert_eq!(bounded, None);
        crate::test_guest_memory::unmap_guest_page(page);
    }

    struct FakeHost {
        policy: Option<RoutingPolicySnapshot>,
        global_state: Option<u32>,
        decoded: Option<(u32, u32)>,
        decode_completion: Vec<bool>,
        applied: Vec<(u32, u32, u32)>,
        refused: Option<u32>,
        events: Vec<&'static str>,
    }

    impl RoutingHleHost for FakeHost {
        fn policy_snapshot(&mut self, _entry: u32) -> Option<RoutingPolicySnapshot> {
            self.events.push("policy");
            self.policy
        }

        fn global_state(&mut self) -> Option<u32> {
            self.events.push("global");
            self.global_state
        }

        fn decode(
            &mut self,
            _context: *mut c_void,
            _entry: u32,
            completion_enabled: bool,
        ) -> Option<(u32, u32)> {
            self.events.push("decode");
            self.decode_completion.push(completion_enabled);
            self.decoded
        }

        fn apply(&mut self, entry: u32, stream: u32, argument: u32) -> u32 {
            self.events.push("apply");
            self.applied.push((entry, stream, argument));
            0x1234
        }

        fn refuse(&mut self, _entry: u32, reason: u32) -> ! {
            self.refused = Some(reason);
            panic!("stream routing refused with reason {reason}")
        }
    }

    fn admitted_host(decoded: Option<(u32, u32)>) -> FakeHost {
        FakeHost {
            policy: Some(RoutingPolicySnapshot {
                admitted: true,
                completion_enabled: true,
                irql_known: true,
                irql: 0,
            }),
            global_state: Some(0),
            decoded,
            decode_completion: Vec::new(),
            applied: Vec::new(),
            refused: None,
            events: Vec::new(),
        }
    }

    #[test]
    fn routing_callbacks_preflight_then_decode_then_apply_exact_arguments() {
        for entry in [HEADROOM, MIX_BINS, MIX_VOLUMES] {
            let mut host = admitted_host(Some((0x1234, 0x5678)));
            assert_eq!(dispatch(&mut host, std::ptr::null_mut(), entry), 0x1234);
            assert_eq!(host.events, ["policy", "global", "decode", "apply"]);
            assert_eq!(host.applied, [(entry, 0x1234, 0x5678)]);
            assert_eq!(host.decode_completion, [true]);
        }
    }

    #[test]
    fn nonzero_global_state_returns_hresult_before_frame_decode_or_apply() {
        let mut host = admitted_host(Some((0x1234, 0x5678)));
        host.global_state = Some(1);
        assert_eq!(dispatch(&mut host, std::ptr::null_mut(), HEADROOM), E_FAIL);
        assert_eq!(host.events, ["policy", "global"]);
        assert!(host.applied.is_empty());
    }

    #[test]
    fn policy_denial_refuses_before_guest_state_or_frame_reads() {
        for policy in [
            Some(RoutingPolicySnapshot {
                admitted: false,
                completion_enabled: false,
                irql_known: true,
                irql: 0,
            }),
            Some(RoutingPolicySnapshot {
                admitted: true,
                completion_enabled: true,
                irql_known: false,
                irql: 0,
            }),
            Some(RoutingPolicySnapshot {
                admitted: true,
                completion_enabled: true,
                irql_known: true,
                irql: 2,
            }),
        ] {
            let mut host = admitted_host(Some((0x1234, 0x5678)));
            host.policy = policy;
            let refusal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                dispatch(&mut host, std::ptr::null_mut(), MIX_BINS)
            }));
            assert!(refusal.is_err());
            assert_eq!(host.events, ["policy"]);
            assert_eq!(host.refused, Some(REFUSE_POLICY));
            assert!(host.applied.is_empty());
        }
        let mut host = admitted_host(Some((0x1234, 0x5678)));
        host.policy = None;
        let refusal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatch(&mut host, std::ptr::null_mut(), MIX_BINS)
        }));
        assert!(refusal.is_err());
        assert_eq!(host.refused, Some(REFUSE_POLICY_SERVICE));
    }

    #[test]
    fn unreadable_global_state_uses_fatal_refusal_before_frame_decode() {
        let mut host = admitted_host(Some((0x1234, 0x5678)));
        host.global_state = None;
        let refusal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatch(&mut host, std::ptr::null_mut(), MIX_VOLUMES)
        }));
        assert!(refusal.is_err());
        assert_eq!(host.events, ["policy", "global"]);
        assert_eq!(host.refused, Some(REFUSE_GLOBAL_STATE));
        assert!(host.applied.is_empty());
    }

    #[test]
    fn a_failed_frame_decode_uses_the_fatal_refusal_path() {
        let mut host = admitted_host(None);
        let refusal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatch(&mut host, std::ptr::null_mut(), MIX_BINS)
        }));
        assert!(refusal.is_err());
        assert_eq!(host.events, ["policy", "global", "decode"]);
        assert_eq!(host.refused, Some(REFUSE_DECODE));
        assert!(host.applied.is_empty());
    }
}
