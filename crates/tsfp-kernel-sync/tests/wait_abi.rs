// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::BTreeMap;
use tsfp_kernel_sync::{wait::*, *};
#[derive(Default)]
struct Bytes(BTreeMap<u32, u8>);
impl GuestMemory for Bytes {
    fn read<const N: usize>(&mut self, address: u32) -> Option<[u8; N]> {
        if u64::from(address) + N as u64 > 0x100000000 {
            return None;
        }
        let mut result = [0; N];
        for (index, byte) in result.iter_mut().enumerate() {
            *byte = *self.0.get(&(address + index as u32))?;
        }
        Some(result)
    }
    fn write<const N: usize>(&mut self, address: u32, bytes: [u8; N]) -> bool {
        if u64::from(address) + N as u64 > 0x100000000 {
            return false;
        }
        for (index, byte) in bytes.iter().enumerate() {
            self.0.insert(address + index as u32, *byte);
        }
        true
    }
}
fn context() -> Context {
    Context {
        caller: ThreadId(7),
        irql: 0,
        resetting: false,
        active_waits_full: false,
        thread_binding_valid: true,
        current_started_unfinished: true,
    }
}
fn frame(memory: &mut Bytes, handle: u32, timeout: u32) -> Option<CallFrame> {
    for (index, word) in [handle, 1, 0, timeout].iter().enumerate() {
        memory.write(0x1004 + index as u32 * 4, word.to_le_bytes());
    }
    Some(CallFrame {
        stack: 0x1000,
        limit: 0x1014,
    })
}
#[test]
fn pending_actions_require_real_adapter_completion() {
    let mut memory = Bytes::default();
    let mut objects = SyncObjects::default();
    let mut thread = Thread::new(ThreadId(8), false);
    thread.started = true;
    objects.insert(Handle(8), Object::Thread(thread));
    let call = frame(&mut memory, 8, 0);
    assert_eq!(
        objects.wait_guest(234, call, &mut memory, context()),
        Outcome::Wait(WaitOutcome::PendingThread(Handle(8)))
    );
    for (units, nanoseconds) in [(80000i64, 8000000), (160000, 16000000)] {
        memory.write(0x2000, (-units).to_le_bytes());
        let call = frame(&mut memory, 0xfffffffe, 0x2000);
        assert_eq!(
            objects.wait_guest(234, call, &mut memory, context()),
            Outcome::HostPause { nanoseconds }
        );
        let mut saturated = context();
        saturated.active_waits_full = true;
        assert_eq!(
            objects.wait_guest(234, call, &mut memory, saturated),
            Outcome::Wait(WaitOutcome::Refused(Refusal::UnsupportedScope))
        );
    }
    assert_eq!(
        objects.wait_guest(232, None, &mut memory, context()),
        Outcome::Unregistered
    );
}
#[test]
fn top_guest_word_is_valid_but_frame_never_wraps() {
    let mut memory = Bytes::default();
    for (address, value) in [(0xfffffff4, 7u32), (0xfffffff8, 256), (0xfffffffc, 0)] {
        memory.write(address, value.to_le_bytes());
    }
    let call = Some(CallFrame {
        stack: 0xfffffff0,
        limit: 0,
    });
    assert_eq!(
        NtWait::decode(233, call, &mut memory),
        Ok(NtWait {
            handle: Handle(7),
            mode: 1,
            alertable: false,
            timeout: Timeout::Infinite
        })
    );
    assert_eq!(
        NtWait::decode(234, call, &mut memory),
        Err(INVALID_PARAMETER)
    );
}
#[test]
fn stack_limit_grid_requires_all_argument_words_without_wrap() {
    for ordinal in [233, 234] {
        let words = if ordinal == 233 { 3 } else { 4 };
        for stack in [0x1000u32, 0xffff_ffe8, 0xffff_fff0] {
            let end = u64::from(stack) + 4 + words * 4;
            for slack in [0u64, 1, 4] {
                let mut memory = Bytes::default();
                for (index, word) in [7u32, 0, 0, 0].into_iter().take(words as usize).enumerate() {
                    let address = u64::from(stack) + 4 + index as u64 * 4;
                    if address + 4 <= 0x1_0000_0000 {
                        memory.write(address as u32, word.to_le_bytes());
                    }
                }
                let limit = if end <= 0x1_0000_0000 {
                    (end - slack) as u32
                } else {
                    0
                };
                let decoded =
                    NtWait::decode(ordinal, Some(CallFrame { stack, limit }), &mut memory);
                let valid = end <= 0x1_0000_0000 && (limit == 0 || slack == 0);
                assert_eq!(
                    decoded.is_ok(),
                    valid,
                    "ordinal={ordinal} stack={stack:#x} slack={slack}"
                );
            }
        }
    }
}
#[test]
fn timeout_fault_precedes_scope_and_never_consumes_event() {
    let mut memory = Bytes::default();
    let mut objects = SyncObjects::default();
    objects.insert(
        Handle(8),
        Object::Event(Event {
            kind: EventKind::Synchronization,
            signaled: true,
        }),
    );
    // A readable low word is insufficient for the eight-byte timeout read.
    memory.write(0x2000, 0u32.to_le_bytes());
    let call = frame(&mut memory, 8, 0x2000);
    let mut resetting = context();
    resetting.resetting = true;
    assert_eq!(
        objects.wait_guest(234, call, &mut memory, resetting),
        Outcome::Wait(WaitOutcome::Status(ACCESS_VIOLATION))
    );
    assert!(matches!(
        objects.get(Handle(8)),
        Some(Object::Event(Event { signaled: true, .. }))
    ));
}
