// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::BTreeMap;
use tsfp_kernel_sync::{ThreadId, critsec::*};
#[path = "support/words.rs"]
mod words;
#[derive(Default)]
struct Memory(BTreeMap<u32, u32>);
impl GuestWords for Memory {
    fn read(&mut self, address: u32) -> Option<u32> {
        words::read_word(&self.0, address)
    }
    fn write(&mut self, address: u32, value: u32) {
        self.0.insert(address, value);
    }
}
fn fixture() -> (CriticalSections, Memory, CallFrame) {
    let mut memory = Memory::default();
    memory.0.insert(104, 200);
    memory.0.insert(200, 0x40001);
    memory.0.insert(216, u32::MAX);
    (
        CriticalSections::default(),
        memory,
        CallFrame {
            stack: 100,
            limit: 108,
        },
    )
}
#[test]
fn pending_contenders_retry_without_early_mutation_or_fifo_claim() {
    let (mut model, mut memory, frame) = fixture();
    model.call(277, Some(frame), ThreadId(1), &mut memory);
    let CallOutcome::Pending(first) = model.call(277, Some(frame), ThreadId(2), &mut memory) else {
        panic!()
    };
    let CallOutcome::Pending(second) = model.call(277, Some(frame), ThreadId(3), &mut memory)
    else {
        panic!()
    };
    assert_eq!(memory.read(216), Some(0));
    assert_eq!(model.diagnostics.contended, 0);
    model.call(294, Some(frame), ThreadId(1), &mut memory);
    // Scheduler chooses third caller first; native mutex has no FIFO contract.
    assert!(matches!(
        model.complete_enter(second, &mut memory),
        CallOutcome::Returned(0)
    ));
    let CallOutcome::Pending(first) = model.complete_enter(first, &mut memory) else {
        panic!()
    };
    model.call(294, Some(frame), ThreadId(3), &mut memory);
    assert!(matches!(
        model.complete_enter(first, &mut memory),
        CallOutcome::Returned(0)
    ));
    assert_eq!(model.diagnostics.contended, 2);
    assert_eq!(
        model.record(GuestAddress(200)).unwrap().owner,
        Some(ThreadId(2))
    );
}
#[test]
fn held_reset_retires_slot_and_never_manufactures_wait_completion() {
    let (mut model, mut memory, frame) = fixture();
    model.call(277, Some(frame), ThreadId(1), &mut memory);
    let CallOutcome::Pending(ticket) = model.call(277, Some(frame), ThreadId(2), &mut memory)
    else {
        panic!()
    };
    model.reset();
    assert_eq!(model.diagnostics.retired, 1);
    assert_eq!(memory.read(216), Some(0));
    assert!(matches!(
        model.complete_enter(ticket, &mut memory),
        CallOutcome::Pending(_)
    ));
    // Same address adopts a fresh native mutex; retired waiter stays stranded.
    assert!(matches!(
        model.call(277, Some(frame), ThreadId(3), &mut memory),
        CallOutcome::Returned(0)
    ));
    assert_eq!(memory.read(216), Some(1));
}
#[test]
fn initialize_retains_native_recursion_and_try_enter_is_unregistered() {
    let (mut model, mut memory, frame) = fixture();
    model.call(277, Some(frame), ThreadId(1), &mut memory);
    model.call(291, Some(frame), ThreadId(2), &mut memory);
    assert!(matches!(
        model.call(277, Some(frame), ThreadId(2), &mut memory),
        CallOutcome::Pending(_)
    ));
    model.call(277, Some(frame), ThreadId(1), &mut memory);
    model.call(294, Some(frame), ThreadId(1), &mut memory);
    assert!(matches!(
        model.call(277, Some(frame), ThreadId(2), &mut memory),
        CallOutcome::Pending(_)
    ));
    assert!(matches!(
        model.call(306, None, ThreadId(1), &mut memory),
        CallOutcome::Unregistered
    ));
    assert_eq!(memory.read(220), Some(0));
    assert_eq!(memory.read(224), Some(0));
}
