// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 WEAK differential against the bounded lifted callback-result branch.

#[path = "../src/port_scaffolds/verb_callback_dispatch_boundary_at_0x236099_as_a_typed_rust_borrowed_view_over_the_current_level_arena_event_instance.rs"]
mod port;
use port::{
    CallbackId, CallbackInvocation, DefinitionId, DispatchGate, EventInstanceId,
    StoryEventInstance, VerbCallbackInvoker, dispatch,
};

struct CallbackResult(u32);
impl VerbCallbackInvoker for CallbackResult {
    fn invoke(&mut self, _: CallbackInvocation<'_>) -> Option<u32> {
        Some(self.0)
    }
}

fn lifted_body(raw_result: u32) -> DispatchGate {
    // Bounded transcription of `test eax,eax; jnz shared_noop` at the callback edge.
    if raw_result == 0 {
        DispatchGate::ContinueVerb
    } else {
        DispatchGate::SharedNoOp
    }
}

#[test]
fn tier1_lifted_differential_3000_seeds_weak() {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for index in 0..3000u32 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let raw = if seed & 3 == 0 {
            0
        } else {
            (seed >> 32) as u32 | 1
        };
        let instance = StoryEventInstance {
            identity: EventInstanceId(index),
            definition: DefinitionId((seed >> 11) as u32),
        };
        let mut callback = CallbackResult(raw);
        let actual = dispatch(true, CallbackId(index ^ 0x236099), &instance, &mut callback);
        assert_eq!(actual, Some(lifted_body(raw)), "seed={seed:#018x}");
    }
}
