use tsfp_kernel_object::*;

#[derive(Default)]
struct Memory {
    word: u32,
    readable: bool,
    writable: bool,
}
impl GuestMemory for Memory {
    fn read_word(&self, _: GuestAddress) -> Option<u32> {
        self.readable.then_some(self.word)
    }
    fn write_word(&mut self, _: GuestAddress, value: u32) -> bool {
        if self.writable {
            self.word = value;
            true
        } else {
            false
        }
    }
    fn readable(&self, _: GuestAddress, _: u32) -> bool {
        self.readable
    }
}

#[test]
fn marker_default_and_unregistered_leave_everything_untouched() {
    assert_eq!(RUST_SELECTED, cfg!(feature = "rust-kernel-object"));
    let mut table = ObjectTable::default();
    let mut memory = Memory {
        word: 0x1234,
        ..Default::default()
    };
    assert_eq!(
        table.dispatch(247, None, &mut memory, Owner(0), &mut NoFileBodies),
        CallOutcome::Unregistered
    );
    assert_eq!(memory.word, 0x1234);
    assert_eq!(table.fresh_count(), 0);
    assert_eq!(table.diagnostics, Diagnostics::default());
}

#[test]
fn every_encoded_generation_remains_distinct_and_avoids_pseudo_handles() {
    let mut previous = Handle(0);
    for generation in 0..16384 {
        for slot in [0, 63, 255] {
            let handle = Handle::encode(slot, generation);
            assert_eq!(handle.0 & 0x00ff0003, 0x00e10000);
            assert!(![0, 0xfffffffc, 0xfffffffe, 0xffffffff].contains(&handle.0));
            assert_ne!(handle, previous);
            previous = handle;
        }
    }
}

#[test]
fn body_lifetime_is_separate_from_handle_and_unmapped_binds_are_refused() {
    let mut table = ObjectTable::default();
    let handle = table.create(Kind::Thread, Owner(255));
    let mut memory = Memory::default();
    assert!(!table.bind_thread_body(handle, Owner(255), GuestAddress(0x1000), &memory));
    memory.readable = true;
    assert!(!table.bind_thread_body(handle, Owner(254), GuestAddress(0x1000), &memory));
    assert!(!table.bind_thread_body(handle, Owner(255), GuestAddress(0xfffffff0), &memory));
    assert!(table.bind_thread_body(handle, Owner(255), GuestAddress(0x1000), &memory));
    assert!(table.release(handle, &mut NoFileBodies));
    assert!(table.find(handle).is_none());
    assert_eq!(table.live_count(), 0);
    assert!(table.entries().iter().any(Option::is_some));
    assert!(table.detach_thread_bodies());
    assert!(table.entries().iter().all(Option::is_none));
}

#[test]
fn failed_publication_removes_binding_without_releasing_the_handle() {
    let mut table = ObjectTable::default();
    let handle = table.create(Kind::Thread, Owner(255));
    let mut memory = Memory {
        readable: true,
        ..Default::default()
    };
    assert!(!table.publish_thread_body(
        handle,
        Owner(255),
        GuestAddress(0x1000),
        GuestAddress(0x2000),
        &mut memory
    ));
    assert_eq!(
        table.find(handle).unwrap().state,
        ObjectState::Thread {
            open: true,
            body: GuestAddress(0),
            body_references: 0
        }
    );
    memory.writable = true;
    assert!(table.publish_thread_body(
        handle,
        Owner(255),
        GuestAddress(0x1000),
        GuestAddress(0x2000),
        &mut memory
    ));
    assert_eq!(memory.word, handle.0);
}

#[test]
fn full_table_reuses_freed_slots_in_fifo_order_without_reviving_stale_handles() {
    let mut table = ObjectTable::default();
    let mut handles: Vec<_> = (0..256)
        .map(|_| table.create(Kind::Event, Owner(7)))
        .collect();
    assert_eq!(table.create(Kind::Event, Owner(7)), Handle(0));
    assert_eq!(table.fresh_count(), 256);
    assert_eq!(table.live_count(), 256);

    for round in 0..128 {
        let slots = [7, 91, 255];
        let stale: Vec<_> = slots.iter().map(|&slot| handles[slot]).collect();
        for &old in &stale {
            assert!(table.release(old, &mut NoFileBodies));
            assert!(table.find(old).is_none());
        }
        assert_eq!(table.free_slots().collect::<Vec<_>>(), slots);
        for (index, &slot) in slots.iter().enumerate() {
            let next = table.create(Kind::Event, Owner(7));
            assert_eq!(next, Handle::encode(slot, round + 1));
            assert_ne!(next, stale[index]);
            handles[slot] = next;
        }
        assert_eq!(table.live_count(), 256);
        assert!(table.free_slots().next().is_none());
    }
    assert_eq!(table.retired_count(), 0);
}

#[test]
fn file_body_references_update_authoritative_table_and_reject_stale_or_wrong_kind() {
    let mut table = ObjectTable::default();
    let file = table.create(Kind::File, Owner(1));
    let event = table.create(Kind::Event, Owner(2));
    assert!(!table.file_body_reference(event));
    assert_eq!(table.file_body_dereference(event), None);
    assert!(table.file_body_reference(file));
    assert_eq!(table.find(file).unwrap().references, 2);
    assert!(table.release(file, &mut NoFileBodies));
    assert_eq!(table.find(file).unwrap().references, 1);
    assert_eq!(table.file_body_dereference(file), Some(true));
    assert!(table.find(file).is_none());
    assert!(!table.file_body_reference(file));
    assert_eq!(table.file_body_dereference(file), None);
}
