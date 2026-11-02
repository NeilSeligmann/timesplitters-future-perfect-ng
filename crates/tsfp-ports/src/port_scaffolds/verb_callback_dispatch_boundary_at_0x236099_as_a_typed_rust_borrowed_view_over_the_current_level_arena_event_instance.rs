// SPDX-License-Identifier: GPL-3.0-or-later
//! T2493: bounded Story verb callback context and raw return dispatch gate.
//!
//! This module models only the borrowed invocation boundary at `0x236099`. The
//! definition and event instance are Rust-owned typed values supplied by the
//! adapter; it never interprets a guest address or retains callback context.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DefinitionId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallbackId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventInstanceId(pub u32);

/// Minimal typed event state exposed to one callback invocation.
///
/// Payload fields stay opaque until their individual contracts are measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryEventInstance {
    pub identity: EventInstanceId,
    pub definition: DefinitionId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallbackInvocation<'a> {
    pub callback: CallbackId,
    pub instance: &'a StoryEventInstance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchGate {
    ContinueVerb,
    SharedNoOp,
}

/// Preserve only the observed raw EAX zero/nonzero branch.
pub const fn dispatch_gate(raw_result: u32) -> DispatchGate {
    if raw_result == 0 {
        DispatchGate::ContinueVerb
    } else {
        DispatchGate::SharedNoOp
    }
}

/// Implemented by the callback owner. `None` means unsupported and asks the
/// caller to keep the lifted path authoritative.
pub trait VerbCallbackInvoker {
    fn invoke(&mut self, invocation: CallbackInvocation<'_>) -> Option<u32>;
}

/// An explicit A/B boundary: return `None` while disabled or when the callback
/// form is unsupported. The borrowed context cannot outlive this call.
pub fn dispatch<I: VerbCallbackInvoker>(
    enabled: bool,
    callback: CallbackId,
    instance: &StoryEventInstance,
    invoker: &mut I,
) -> Option<DispatchGate> {
    if !enabled {
        return None;
    }
    let raw_result = invoker.invoke(CallbackInvocation { callback, instance })?;
    Some(dispatch_gate(raw_result))
}

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

fn enabled_from(value: Option<&[u8]>) -> bool {
    value == Some(b"rust")
}

/// Default-off opt-in; the lifted callback dispatcher stays authoritative.
#[cfg(not(test))]
pub fn enabled() -> bool {
    unsafe {
        let value = getenv(c"TSFP_STORY_CALLBACK_DISPATCH".as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}
#[cfg(test)]
pub fn enabled() -> bool {
    true
}

/// Apply the environment-controlled A/B selection for native callback owners.
pub fn dispatch_selected<I: VerbCallbackInvoker>(
    callback: CallbackId,
    instance: &StoryEventInstance,
    invoker: &mut I,
) -> Option<DispatchGate> {
    dispatch(enabled(), callback, instance, invoker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct FixedResult(Option<u32>);
    impl VerbCallbackInvoker for FixedResult {
        fn invoke(&mut self, _: CallbackInvocation<'_>) -> Option<u32> {
            self.0
        }
    }

    #[test]
    fn raw_result_is_only_a_zero_nonzero_gate() {
        assert_eq!(dispatch_gate(0), DispatchGate::ContinueVerb);
        for raw in [1, 2, u32::MAX] {
            assert_eq!(dispatch_gate(raw), DispatchGate::SharedNoOp);
        }
    }

    #[test]
    fn disabled_or_unsupported_dispatch_declines_to_lifted_body() {
        let instance = StoryEventInstance {
            identity: EventInstanceId(4),
            definition: DefinitionId(9),
        };
        assert_eq!(
            dispatch(false, CallbackId(2), &instance, &mut FixedResult(Some(0))),
            None
        );
        assert_eq!(
            dispatch(true, CallbackId(2), &instance, &mut FixedResult(None)),
            None
        );
    }

    #[test]
    fn invocation_borrows_the_current_typed_instance() {
        struct CheckIdentity;
        impl VerbCallbackInvoker for CheckIdentity {
            fn invoke(&mut self, call: CallbackInvocation<'_>) -> Option<u32> {
                (call.callback == CallbackId(2)
                    && call.instance.identity == EventInstanceId(4)
                    && call.instance.definition == DefinitionId(9))
                .then_some(0)
            }
        }
        let instance = StoryEventInstance {
            identity: EventInstanceId(4),
            definition: DefinitionId(9),
        };
        assert_eq!(
            dispatch(true, CallbackId(2), &instance, &mut CheckIdentity),
            Some(DispatchGate::ContinueVerb)
        );
    }

    #[test]
    fn switch_is_opt_in() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(enabled_from(Some(b"rust")));
    }
}
