// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 0 contracts for staged actions; Tier 1 ABI cases live in the Python harness.
use std::collections::BTreeMap;
use tsfp_kernel_object::{GuestAddress as Address, GuestMemory, Handle, ObjectTable};
use tsfp_kernel_sync::thread::*;

#[derive(Default)]
struct Memory {
    bytes: BTreeMap<u32, u8>,
    reject: Option<u32>,
}
impl Memory {
    fn map(&mut self, base: u32, size: u32, fill: u8) {
        for at in base..base + size {
            self.bytes.insert(at, fill);
        }
    }
    fn frame(&mut self, args: &[u32]) -> Frame {
        self.map(0x1000, 48, 0);
        for (i, v) in args.iter().enumerate() {
            assert!(self.write_word(Address(0x1004 + i as u32 * 4), *v));
        }
        Frame {
            stack: Address(0x1000),
            limit: Address(0x1030),
        }
    }
}
impl GuestMemory for Memory {
    fn read_word(&self, at: Address) -> Option<u32> {
        let mut bytes = [0; 4];
        for (i, v) in bytes.iter_mut().enumerate() {
            *v = *self.bytes.get(&at.0.checked_add(i as u32)?)?;
        }
        Some(u32::from_le_bytes(bytes))
    }
    fn write_word(&mut self, at: Address, v: u32) -> bool {
        for (i, b) in v.to_le_bytes().into_iter().enumerate() {
            let Some(at) = at.0.checked_add(i as u32) else {
                return false;
            };
            if !self.write_byte(Address(at), b) {
                return false;
            }
        }
        true
    }
    fn readable(&self, at: Address, len: u32) -> bool {
        (0..len).all(|i| {
            at.0.checked_add(i)
                .is_some_and(|a| self.bytes.contains_key(&a))
        })
    }
}
impl ThreadMemory for Memory {
    fn write_byte(&mut self, at: Address, v: u8) -> bool {
        if self.reject == Some(at.0) {
            return false;
        }
        let Some(byte) = self.bytes.get_mut(&at.0) else {
            return false;
        };
        *byte = v;
        true
    }
}
struct Provider {
    failure: Option<u32>,
    releases: u32,
    storage: Storage,
}
impl ThreadStorage for Provider {
    fn provision(&mut self, _: CreateRequest, _: &mut impl GuestMemory) -> Result<Storage, u32> {
        self.failure.map_or(Ok(self.storage), Err)
    }
    fn release(&mut self, _: Storage) {
        self.releases += 1;
    }
}
fn fixture() -> (Memory, Provider) {
    let mut memory = Memory::default();
    memory.map(0x2000, 8, 0xa5);
    memory.map(0x3000, 0x1000, 0);
    memory.map(0x4000, 0x1000, 0);
    memory.map(0x5000, 0x1000, 0);
    (
        memory,
        Provider {
            failure: None,
            releases: 0,
            storage: Storage {
                stack_low: Address(0x3000),
                stack_high: Address(0x4000),
                control: Address(0x4000),
                tls: Address(0x6000),
                monitor: Address(0x5000),
            },
        },
    )
}
fn create(
    memory: &mut Memory,
    threads: &mut Threads,
    objects: &mut ObjectTable,
    provider: &mut Provider,
    suspended: u32,
) -> Outcome {
    let frame = memory.frame(&[0x2000, 0, 0, 65, 0x2004, 123, 456, suspended, 0, 789]);
    threads.create(Some(frame), memory, objects, provider)
}
#[test]
fn launch_snapshot_requires_ready_unsuspended_unstarted_record() {
    let (mut memory, mut provider) = fixture();
    let mut threads = Threads::default();
    let mut objects = ObjectTable::default();
    assert_eq!(
        create(&mut memory, &mut threads, &mut objects, &mut provider, 1),
        Outcome::Returned(0)
    );
    let handle = threads.records[0].as_ref().unwrap().handle;
    assert_eq!(threads.launch_snapshot(handle), None);
    assert!(!threads.acknowledge_launch(handle));
    assert_eq!(threads.launch_snapshot(Handle(handle.0 ^ 0x10000)), None);
    let frame = memory.frame(&[handle.0, 0]);
    assert_eq!(
        threads.call(224, Some(frame), &mut memory, &objects, 0, true),
        Outcome::Pending(Action::Launch(handle))
    );
    let launch = threads.launch_snapshot(handle).unwrap();
    assert_eq!(launch.entry, 789);
    assert_eq!(launch.esp, Address(0x3ff4));
    assert_eq!(launch.fs_base, Address(0x4000));
    assert_eq!(launch.start, 123);
    assert_eq!(launch.context, 456);
    assert_eq!(launch.stack_low, Address(0x3000));
    assert_eq!(launch.stack_high, Address(0x4000));
    assert_eq!(threads.launch_snapshot(handle), Some(launch));
    assert!(threads.acknowledge_launch(handle));
    assert_eq!(threads.launch_snapshot(handle), None);
    assert!(!threads.acknowledge_launch(handle));
}
#[test]
fn publication_resume_and_termination_are_distinct_actions() {
    let (mut memory, mut provider) = fixture();
    let mut threads = Threads::default();
    let mut objects = ObjectTable::default();
    assert_eq!(
        create(&mut memory, &mut threads, &mut objects, &mut provider, 256),
        Outcome::Returned(0)
    );
    let handle = threads.records[0].as_ref().unwrap().handle;
    assert_eq!(memory.read_word(Address(0x2000)), Some(handle.0));
    assert_eq!(memory.read_word(Address(0x3ff4)), Some(0xdeadf00d));
    assert_eq!(memory.read_word(Address(0x3ff8)), Some(123));
    assert_eq!(memory.read_word(Address(0x3ffc)), Some(456));
    let frame = memory.frame(&[handle.0, 0x1004]);
    assert_eq!(
        threads.call(224, Some(frame), &mut memory, &objects, 0, true),
        Outcome::Pending(Action::Launch(handle))
    );
    assert_eq!(memory.read_word(Address(0x1004)), Some(1));
    assert!(!threads.records[0].as_ref().unwrap().state.started);
    assert!(
        threads.records[0]
            .as_mut()
            .unwrap()
            .state
            .acknowledge_launch()
    );
    threads.current = Some(CurrentThread { slot: 0, handle });
    let frame = memory.frame(&[0xbad]);
    assert_eq!(
        threads.call(258, Some(frame), &mut memory, &objects, 0, true),
        Outcome::Pending(Action::Terminate {
            handle,
            status: 0xbad
        })
    );
    let r = threads.records[0].as_ref().unwrap();
    assert!(!r.state.finished && !r.state.terminated);
    assert!(r.termination_requested);
    assert_eq!(
        threads.termination_hook_returned(threads.current.unwrap()),
        Outcome::Returned(tsfp_kernel_sync::NOT_IMPLEMENTED)
    );
    let r = threads.records[0].as_ref().unwrap();
    assert!(!r.termination_requested);
    assert_eq!(r.exit_eax, 0);
}
#[test]
fn failed_publication_rolls_back_and_retry_gets_a_new_object_handle() {
    let (mut memory, mut provider) = fixture();
    let mut threads = Threads::default();
    let mut objects = ObjectTable::default();
    memory.reject = Some(0x2000);
    assert_eq!(
        create(&mut memory, &mut threads, &mut objects, &mut provider, 0),
        Outcome::Returned(INVALID_PARAMETER)
    );
    assert!(threads.records.is_empty());
    assert_eq!(objects.live_count(), 0);
    assert_eq!(provider.releases, 1);
    memory.reject = None;
    assert_eq!(
        create(&mut memory, &mut threads, &mut objects, &mut provider, 0),
        Outcome::Returned(0)
    );
    assert_ne!(
        threads.records[0].as_ref().unwrap().handle,
        Handle::encode(0, 0)
    );
    assert!(!threads.records[0].as_ref().unwrap().state.started);
}
#[test]
fn provisioning_failure_retains_rollback_identity_including_monitor_failure() {
    let (mut memory, mut provider) = fixture();
    let mut threads = Threads::default();
    let mut objects = ObjectTable::default();
    // Deliberate safe model deviation from C's monitor-allocation abort, documented.
    provider.failure = Some(NO_MEMORY);
    assert_eq!(
        create(&mut memory, &mut threads, &mut objects, &mut provider, 0),
        Outcome::Returned(NO_MEMORY)
    );
    assert_eq!(objects.live_count(), 0);
    assert!(threads.records.is_empty());
    assert_eq!(provider.releases, 0);
}
#[test]
fn incomplete_frame_and_optional_id_do_not_undo_publication() {
    let (mut memory, mut provider) = fixture();
    let mut threads = Threads::default();
    let mut objects = ObjectTable::default();
    memory.reject = Some(0x3ff4);
    assert_eq!(
        create(&mut memory, &mut threads, &mut objects, &mut provider, 1),
        Outcome::Returned(0)
    );
    let r = threads.records[0].as_ref().unwrap();
    assert!(!r.frame_ready);
    assert_eq!(r.esp, Address(0));
    assert_eq!(objects.live_count(), 1);
    let frame = memory.frame(&[r.handle.0, 0]);
    assert_eq!(
        threads.call(224, Some(frame), &mut memory, &objects, 0, true),
        Outcome::Returned(0)
    );
    assert!(threads.diagnostics.contains(&Diagnostic::FrameMissing));
}
#[test]
fn control_writes_are_ordered_partial_and_tls_end_is_requested_size() {
    let (mut memory, provider) = fixture();
    let storage = provider.storage;
    memory.reject = Some(0x4020);
    assert!(!initialize_control(&mut memory, storage, 0x28, 123));
    assert_eq!(memory.read_word(Address(0x4000)), Some(u32::MAX));
    assert_eq!(memory.read_word(Address(0x5014)), Some(123));
    assert_eq!(memory.read_word(Address(0x4278)), Some(0));
    memory.reject = None;
    assert!(initialize_control(&mut memory, storage, 0x28, 123));
    assert!(publish_tls_end(
        &mut memory,
        storage.control,
        storage.tls,
        65
    ));
    assert_eq!(memory.read_word(Address(0x4004)), Some(0x6041));
    assert_eq!(tls_allocation_size(65), Some(4096));
    assert_eq!(tls_allocation_size(u32::MAX), None);
    assert!(!publish_tls_end(
        &mut memory,
        storage.control,
        Address(u32::MAX - 3),
        4
    ));
}
#[test]
fn saturation_and_current_identity_do_not_fabricate_execution() {
    let (mut memory, mut provider) = fixture();
    let mut threads = Threads::default();
    let mut objects = ObjectTable::default();
    threads.records.resize_with(64, || None);
    assert_eq!(
        create(&mut memory, &mut threads, &mut objects, &mut provider, 0),
        Outcome::Returned(NO_MEMORY)
    );
    assert_eq!(objects.live_count(), 0);
    threads.current = Some(CurrentThread {
        slot: 99,
        handle: Handle(123),
    });
    assert_eq!(threads.current_identity(), tsfp_kernel_sync::ThreadId(123));
    let frame = memory.frame(&[42]);
    assert_eq!(
        threads.call(258, Some(frame), &mut memory, &objects, 0, true),
        Outcome::Returned(tsfp_kernel_sync::NOT_IMPLEMENTED)
    );
    assert!(!tsfp_kernel_sync::RUST_KERNEL_SYNC_ENABLED || cfg!(feature = "rust-kernel-sync"));
}

#[test]
fn optional_id_write_failure_retains_published_thread() {
    let (mut memory, mut provider) = fixture();
    let mut threads = Threads::default();
    let mut objects = ObjectTable::default();
    memory.reject = Some(0x2004);
    assert_eq!(
        create(&mut memory, &mut threads, &mut objects, &mut provider, 1),
        Outcome::Returned(0)
    );
    assert_eq!(objects.live_count(), 1);
    assert!(threads.diagnostics.contains(&Diagnostic::OptionalIdFailed));
    assert_eq!(provider.releases, 0);
    assert_eq!(memory.read_word(Address(0x2004)), Some(0xa5a5a5a5));
}

#[test]
fn completion_requires_confirmation_and_publishes_status_before_signal() {
    for confirmed in [false, true] {
        let (mut memory, mut provider) = fixture();
        let mut threads = Threads::default();
        let mut objects = ObjectTable::default();
        assert_eq!(
            create(&mut memory, &mut threads, &mut objects, &mut provider, 0),
            Outcome::Returned(0)
        );
        let r = threads.records[0].as_mut().unwrap();
        assert!(r.state.acknowledge_launch());
        let current = CurrentThread {
            slot: 0,
            handle: r.handle,
        };
        threads.current = Some(current);
        let frame = memory.frame(&[0xbad]);
        assert!(matches!(
            threads.call(258, Some(frame), &mut memory, &objects, 0, true),
            Outcome::Pending(_)
        ));
        assert!(threads.complete_entry(current, confirmed, &mut memory));
        let r = threads.records[0].as_ref().unwrap();
        assert!(r.state.finished);
        assert_eq!(r.state.terminated, confirmed);
        assert_eq!(
            memory.read_word(Address(0x4920)),
            Some(if confirmed { 0xbad } else { 0 })
        );
        assert_eq!(
            memory.read_word(Address(0x4804)),
            Some(u32::from(confirmed))
        );
        assert_eq!(threads.current_identity(), tsfp_kernel_sync::ThreadId(0));
    }
}

#[test]
fn failed_exit_signal_never_becomes_successful_termination() {
    let (mut memory, mut provider) = fixture();
    let mut threads = Threads::default();
    let mut objects = ObjectTable::default();
    assert_eq!(
        create(&mut memory, &mut threads, &mut objects, &mut provider, 0),
        Outcome::Returned(0)
    );
    let r = threads.records[0].as_mut().unwrap();
    assert!(r.state.acknowledge_launch());
    let current = CurrentThread {
        slot: 0,
        handle: r.handle,
    };
    threads.current = Some(current);
    let frame = memory.frame(&[42]);
    assert!(matches!(
        threads.call(258, Some(frame), &mut memory, &objects, 0, true),
        Outcome::Pending(_)
    ));
    memory.reject = Some(0x4804);
    assert!(threads.complete_entry(current, true, &mut memory));
    assert_eq!(memory.read_word(Address(0x4920)), Some(42));
    assert_eq!(memory.read_word(Address(0x4804)), Some(0));
    let r = threads.records[0].as_ref().unwrap();
    assert!(r.state.finished);
    assert!(!r.state.terminated);
    assert!(
        threads
            .diagnostics
            .contains(&Diagnostic::ExitPublicationFailed)
    );
}
