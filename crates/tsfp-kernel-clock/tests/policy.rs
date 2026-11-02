use tsfp_kernel_clock::timers::{ObjectId, Timers};
use tsfp_kernel_clock::{Backend, Calendar, Clock, FREQUENCY, Operation, Presenter, Selection};
#[test]
fn frame_fraction_and_invalid_rate_property_grid() {
    for rate in 1..=1000u32 {
        let mut clock = Clock::default();
        for frame in 1..=rate {
            let floor = clock.frame(rate).unwrap();
            assert_eq!(floor, FREQUENCY * u64::from(frame) / u64::from(rate));
            assert_eq!(clock.peek(), floor);
        }
        assert_eq!(clock.peek(), FREQUENCY);
        let before = clock.stats();
        assert_eq!(clock.frame(0), None);
        assert_eq!(clock.frame(1001), None);
        assert_eq!(clock.stats(), before);
    }
}
#[test]
fn calendar_pacing_and_default_off() {
    let mut selected = Selection::new(Clock::default());
    assert_eq!(selected.backend, Backend::C);
    selected.apply(Operation::Read);
    assert_eq!(selected.rust.peek(), 0);
    selected.backend = Backend::Rust;
    assert_eq!(selected.apply(Operation::Read).result, 1000);
    let mut c = Calendar::default();
    assert_eq!(c.query(Some(100)), 100);
    assert_eq!(c.query(Some(90)), 101);
    assert_eq!(c.query(None), 102);
    assert!(!c.set(10, None));
    assert!(c.set(10, Some(100)));
    assert_eq!(c.query(Some(100)), 10);
    let mut presenter = Presenter::default();
    for _ in 0..59_999 {
        presenter.next_deadline_ns();
    }
    assert_eq!(presenter.next_deadline_ns(), 1_001_000_000_000);
}
#[test]
fn timer_coalescing_fifo_and_callback_generation() {
    let mut s = Timers::default();
    let a = ObjectId(1);
    let b = ObjectId(2);
    let t = ObjectId(3);
    s.initialize_dpc(a, 10, 20);
    s.initialize_dpc(b, 11, 21);
    s.initialize_timer(t, 1);
    assert!(!s.set(t, -10, 1, Some(a), 0, 0));
    assert!(!s.begin_service(30_000, 0, false));
    assert!(s.timers[0].armed);
    assert!(!s.begin_service(30_000, 3, true));
    assert!(s.enqueue(b, [8, 9]));
    assert!(!s.enqueue(b, [0, 0]));
    assert!(s.begin_service(30_000, 0, true));
    assert_eq!(s.timers[0].deadline, 30_010);
    let first = s.next_delivery().unwrap();
    assert_eq!(first.id, b);
    assert_eq!(first.arguments, [8, 9]);
    assert!(s.enqueue(b, [1, 2]));
    s.finish_delivery(true);
    assert_eq!(s.next_delivery().unwrap().id, a);
    s.finish_delivery(false);
    assert!(s.dpcs[0].queued); // refusal retains pending; callback requeue is still pending too
    assert!(s.begin_service(30_000, 0, true));
    assert_eq!(s.next_delivery().unwrap().id, a);
    s.remove(a);
    s.finish_delivery(false);
    assert!(!s.dpcs[0].queued);
    assert!(s.cancel(t));
    assert!(!s.cancel(t));
}
#[test]
fn relative_min_absolute_and_saturation() {
    let mut s = Timers::default();
    let t = ObjectId(1);
    s.set(t, i64::MIN, 0, None, u64::MAX - 5, 0);
    assert_eq!(s.timers[0].deadline, u64::MAX);
    assert!(s.set(t, 90, -1, None, 7, 100));
    assert_eq!(s.timers[0].deadline, 7);
    assert!(s.begin_service(7, 0, true));
    assert!(!s.timers[0].armed);
    assert!(s.next_delivery().is_none());
    s.end_service();
    s.set(t, -10, 1, None, 0, 0);
    s.initialize_timer(t, 0);
    assert!(!s.timers[0].armed);
}

#[test]
fn rust_kernel_clock_feature_defaults_off() {
    assert_eq!(
        tsfp_kernel_clock::RUST_KERNEL_CLOCK_ENABLED,
        cfg!(feature = "rust-kernel-clock")
    );
}

#[test]
fn one_shot_cancel_high_word_and_late_queue_boundary() {
    let (t, d, late) = (ObjectId(1), ObjectId(2), ObjectId(3));
    let mut s = Timers::default();
    s.initialize_timer(t, 0);
    s.initialize_dpc(d, 0x10, 0x20);
    s.initialize_dpc(late, 0x30, 0x40);
    // Zero period is one-shot: expires once, disarms.
    s.set(t, -5, 0, Some(d), 0, 0);
    let elapsed = (7u64 << 32) | 9;
    assert!(s.begin_service(elapsed, 2, true));
    let first = s.next_delivery().unwrap();
    assert_eq!(first.arguments, [9, 7]);
    // Work queued by a callback waits for the next pass.
    assert!(s.enqueue(late, [1, 2]));
    s.finish_delivery(true);
    assert_eq!(s.next_delivery(), None);
    s.end_service();
    assert!(!s.timers[0].armed);
    assert!(s.begin_service(elapsed, 2, true));
    assert_eq!(s.next_delivery().unwrap().id, late);
    s.finish_delivery(true);
    s.end_service();
    // Cancel clears DPC and period.
    s.set(t, -5, 10, Some(d), 0, 0);
    assert!(s.cancel(t));
    assert_eq!((s.timers[0].dpc, s.timers[0].period_ms), (None, 0));
}
