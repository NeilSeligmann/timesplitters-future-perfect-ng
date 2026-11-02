// SPDX-License-Identifier: GPL-3.0-or-later
use tsfp_kernel_sync::*;
#[test]
fn event_transition_grid_preserves_manual_and_consumes_auto_signals() {
    for kind in [EventKind::Notification, EventKind::Synchronization] {
        for initial in [false, true] {
            for first_wait in [false, true] {
                for repeated_sets in 0..=8 {
                    let mut event = Event {
                        kind,
                        signaled: initial,
                    };
                    if first_wait {
                        assert_eq!(event.try_wait(), initial);
                    }
                    let before_set = initial && (!first_wait || kind == EventKind::Notification);
                    for index in 0..repeated_sets {
                        assert_eq!(event.set(), if index == 0 { before_set } else { true });
                    }
                    let expected = before_set || repeated_sets > 0;
                    assert_eq!(event.try_wait(), expected);
                    assert_eq!(
                        event.try_wait(),
                        expected && kind == EventKind::Notification
                    );
                }
            }
        }
    }
}
#[test]
fn thread_lifecycle_and_launch_acknowledgement() {
    let mut thread = Thread::new(ThreadId(7), true);
    assert_eq!(thread.suspend(), Ok(1));
    assert!(!thread.resume().launch_requested);
    assert!(thread.resume().launch_requested);
    assert!(!thread.started);
    assert!(thread.acknowledge_launch());
    assert!(!thread.acknowledge_launch());
    assert_eq!(thread.suspend(), Err(NOT_IMPLEMENTED));
    assert_eq!(thread.priority(-16), 0);
    assert_eq!(thread.priority(16), -16);
    assert!(!thread.disable_boost(0x100));
    assert!(!thread.disable_boost(0x101));
    assert!(thread.boost_disabled);
    thread.terminate(0x1234);
    assert_eq!(thread.exit_status, Some(0x1234));
    assert_eq!(thread.suspend(), Err(0xc000004b));
    assert!(!thread.resume().launch_requested);
    assert_eq!(entry_frame(100, 200, 300).arguments, [200, 300]);
    assert_eq!(entry_frame(0, 200, 300).arguments, [300, 0]);
}
#[test]
fn suspension_and_mutant_limits_preserve_state() {
    let mut thread = Thread::new(ThreadId(1), false);
    for previous in 0..127 {
        assert_eq!(thread.suspend(), Ok(previous));
    }
    assert_eq!(thread.suspend(), Err(0xc000004a));
    let mut mutant = Mutant::default();
    for _ in 0..0x10000 {
        assert_eq!(mutant.acquire(ThreadId(1)), (SUCCESS, true));
    }
    assert_eq!(mutant.acquire(ThreadId(1)), (MUTANT_LIMIT, false));
    assert_eq!(mutant.release(ThreadId(2)), (MUTANT_NOT_OWNED, None));
    assert_eq!(mutant.recursion, 0x10000);
}
#[test]
fn waits_do_not_fake_completion() {
    let caller = ThreadId(1);
    let handle = Handle(10);
    let mut objects = SyncObjects::default();
    objects.insert(
        handle,
        Object::Event(Event {
            kind: EventKind::Synchronization,
            signaled: true,
        }),
    );
    assert_eq!(
        objects.wait(handle, caller, 0, true, false, Timeout::Absolute(10)),
        WaitOutcome::Status(SUCCESS)
    );
    assert_eq!(
        objects.wait(handle, caller, 0, true, false, Timeout::Poll),
        WaitOutcome::Status(TIMEOUT)
    );
    assert_eq!(
        objects.wait(handle, caller, 0, true, false, Timeout::Infinite),
        WaitOutcome::Refused(Refusal::WouldBlock)
    );
    assert_eq!(
        objects.wait(handle, caller, 2, true, false, Timeout::Poll),
        WaitOutcome::Refused(Refusal::UnsupportedScope)
    );
    let mut thread = Thread::new(ThreadId(2), false);
    assert!(thread.acknowledge_launch());
    objects.insert(handle, Object::Thread(thread.clone()));
    assert_eq!(
        objects.wait(handle, caller, 0, true, false, Timeout::Infinite),
        WaitOutcome::PendingThread(handle)
    );
    thread.returned();
    objects.insert(handle, Object::Thread(thread.clone()));
    assert_eq!(
        objects.wait(handle, caller, 0, true, false, Timeout::Poll),
        WaitOutcome::Refused(Refusal::TerminalHostFailure)
    );
    thread.terminate(0);
    objects.insert(handle, Object::Thread(thread));
    assert_eq!(
        objects.wait(handle, caller, 0, true, false, Timeout::Infinite),
        WaitOutcome::Status(SUCCESS)
    );
    objects.remove(handle);
    assert_eq!(
        objects.wait(handle, caller, 0, true, false, Timeout::Poll),
        WaitOutcome::Status(INVALID_HANDLE)
    );
    assert_eq!(
        objects.wait(handle, caller, 0, true, false, Timeout::Relative(1)),
        WaitOutcome::Refused(Refusal::UnsupportedScope)
    );
    objects.insert(handle, Object::Semaphore);
    assert_eq!(
        objects.wait(handle, caller, 0, true, false, Timeout::Poll),
        WaitOutcome::Refused(Refusal::UnsupportedScope)
    );
}
#[test]
fn critical_section_contention_and_reinit_are_explicit() {
    let mut cs = CriticalSection::default();
    assert!(cs.enter(ThreadId(1)));
    assert!(!cs.enter(ThreadId(2)));
    cs.leave(ThreadId(2));
    assert_eq!(cs.bad_leaves, 1);
    assert_eq!(cs.recursion, 1);
    cs.initialize();
    assert!(cs.stranded);
    assert!(cs.enter(ThreadId(1)));
    cs.leave(ThreadId(1));
    assert!(!cs.enter(ThreadId(2)));
}
#[test]
fn safepoints_require_live_passive_owner() {
    #[derive(Default)]
    struct Delivery(Vec<ThreadId>);
    impl SafepointDelivery for Delivery {
        fn service_pending(&mut self, caller: ThreadId) {
            self.0.push(caller);
        }
    }
    let mut delivery = Delivery::default();
    let mut thread = Thread::new(ThreadId(9), false);
    assert!(!safepoint(&thread, &mut delivery));
    thread.acknowledge_launch();
    thread.irql.raise(2);
    assert!(!safepoint(&thread, &mut delivery));
    thread.irql.lower(0);
    assert!(safepoint(&thread, &mut delivery));
    thread.terminate(0);
    assert!(!safepoint(&thread, &mut delivery));
    assert_eq!(delivery.0, [ThreadId(9)]);
}
#[test]
fn feature_marker_matches_build_selection() {
    assert_eq!(RUST_KERNEL_SYNC_ENABLED, cfg!(feature = "rust-kernel-sync"));
}
