//! Tier 0 FABRICATED spec tests of the guest adapter, expected values read from
//! src/xbox/kernel_event.c (T2049/T2207).
use std::collections::HashMap;
use tsfp_kernel_clock::guest::*;

#[derive(Default)]
struct Memory {
    words: HashMap<u32, u32>,
    halves: HashMap<u32, u16>,
    mapped_below: u32,
}
impl GuestMemory for Memory {
    fn read_u32(&self, a: u32) -> Option<u32> {
        (a < self.mapped_below).then(|| self.words.get(&a).copied().unwrap_or(0))
    }
    fn write_u32(&mut self, a: u32, v: u32) -> bool {
        a < self.mapped_below && self.words.insert(a, v).is_none() | true
    }
    fn write_u16(&mut self, a: u32, v: u16) -> bool {
        a < self.mapped_below && self.halves.insert(a, v).is_none() | true
    }
}
const T: HostTime = HostTime {
    elapsed: 1000,
    wall: Some(0x1_0000_0005),
};
fn mem() -> Memory {
    Memory {
        mapped_below: 0x10000,
        ..Default::default()
    }
}
fn run(a: &mut Adapter, m: &mut Memory, ordinal: u32, slots: &[u32]) -> u32 {
    a.call(Call::decode(ordinal, slots).expect("decodes"), T, m)
}

#[test]
fn decode_slot_counts_match_thunk_table() {
    for (ordinal, slots) in [
        (97, 1),
        (107, 3),
        (113, 2),
        (119, 3),
        (128, 1),
        (137, 1),
        (149, 4),
        (150, 5),
        (228, 2),
    ] {
        assert!(
            Call::decode(ordinal, &vec![0; slots]).is_some(),
            "{ordinal}"
        );
        assert!(
            Call::decode(ordinal, &vec![0; slots - 1]).is_none(),
            "{ordinal}"
        );
    }
    assert_eq!(Call::decode(126, &[0; 4]), None);
    assert_eq!(
        Call::decode(150, &[1, 2, 3, 0xFFFF_FFFF, 5]),
        Some(Call::SetTimer {
            timer: 1,
            due_low: 2,
            due_high: 3,
            period_ms: Some(-1),
            dpc: 5
        })
    );
}

#[test]
fn timer_set_cancel_returns_previous_state_and_relative_deadline() {
    let (mut a, mut m) = (Adapter::default(), mem());
    run(&mut a, &mut m, 113, &[0x100, 1]);
    // -500 relative: low 0xFFFFFE0C, high 0xFFFFFFFF.
    assert_eq!(
        run(
            &mut a,
            &mut m,
            150,
            &[0x100, 0xFFFF_FE0C, 0xFFFF_FFFF, 20, 0x200]
        ),
        0
    );
    let t = &a.timers.timers[0];
    assert_eq!(
        (t.due, t.deadline, t.period_ms, t.dpc.map(|d| d.0)),
        (-500, 1500, 20, Some(0x200))
    );
    assert_eq!(
        run(&mut a, &mut m, 149, &[0x100, 0xFFFF_FE0C, 0xFFFF_FFFF, 0]),
        1
    );
    assert_eq!(a.timers.timers[0].dpc, None);
    assert_eq!(a.timers.timers[0].period_ms, 0);
    assert_eq!(run(&mut a, &mut m, 97, &[0x100]), 1);
    assert_eq!(run(&mut a, &mut m, 97, &[0x100]), 0);
    assert_eq!(run(&mut a, &mut m, 97, &[0x999]), 0);
}

#[test]
fn absolute_due_measured_against_calendar() {
    let (mut a, mut m) = (Adapter::default(), mem());
    run(&mut a, &mut m, 149, &[0x100, 0x0000_0105, 0x1, 0]);
    assert_eq!(a.timers.timers[0].deadline, 1000 + 0x100);
    // Due 0 is absolute, so it samples the calendar.
    let mut fresh = Adapter::default();
    run(&mut fresh, &mut m, 149, &[0x108, 0, 0, 0]);
    assert_eq!(fresh.calendar.last, 0x1_0000_0005);
    // A due time in the past fires at once (delay 0).
    run(&mut a, &mut m, 149, &[0x104, 1, 0, 0]);
    assert_eq!(a.timers.timers[1].deadline, 1000);
}

#[test]
fn dpc_stamp_queue_refusal_and_remove() {
    let (mut a, mut m) = (Adapter::default(), mem());
    run(&mut a, &mut m, 107, &[0x300, 0xAAAA, 0xBBBB]);
    assert_eq!(m.halves.get(&0x300), Some(&DPC_TYPE_INITIALISED));
    assert_eq!(run(&mut a, &mut m, 119, &[0x300, 7, 8]), 1);
    assert_eq!(run(&mut a, &mut m, 119, &[0x300, 9, 9]), 0);
    assert_eq!(a.timers.dpcs[0].delivery.arguments, [7, 8]);
    assert_eq!(run(&mut a, &mut m, 137, &[0x300]), 1);
    assert_eq!(run(&mut a, &mut m, 137, &[0x300]), 0);
    // Unmapped DPC: model still initialised, no stamp.
    run(&mut a, &mut m, 107, &[0x20000, 1, 2]);
    assert!(!m.halves.contains_key(&0x20000));
}

#[test]
fn query_system_time_writes_low_then_high_and_null_is_skipped() {
    let (mut a, mut m) = (Adapter::default(), mem());
    assert_eq!(run(&mut a, &mut m, 128, &[0x40]), 0);
    assert_eq!((m.words[&0x40], m.words[&0x44]), (5, 1));
    run(&mut a, &mut m, 128, &[0]);
    assert_eq!(a.calendar.last, 0x1_0000_0005);
    // Monotonic: same host sample reads one unit later.
    run(&mut a, &mut m, 128, &[0x40]);
    assert_eq!(m.words[&0x40], 6);
}

#[test]
fn set_system_time_status_codes_and_rebase() {
    let (mut a, mut m) = (Adapter::default(), mem());
    assert_eq!(
        run(&mut a, &mut m, 228, &[0x50, 0x60]),
        STATUS_NOT_IMPLEMENTED
    );
    assert_eq!(run(&mut a, &mut m, 228, &[0, 0]), STATUS_ACCESS_VIOLATION);
    assert_eq!(
        run(&mut a, &mut m, 228, &[0xFFFC, 0]),
        STATUS_ACCESS_VIOLATION
    );
    assert_eq!(
        run(&mut a, &mut m, 228, &[0x50, 0]),
        STATUS_INVALID_PARAMETER
    );
    m.words.insert(0x54, 0x8000_0000);
    assert_eq!(
        run(&mut a, &mut m, 228, &[0x50, 0]),
        STATUS_INVALID_PARAMETER
    );
    m.words.insert(0x54, 0x4000_0000);
    assert_eq!(run(&mut a, &mut m, 228, &[0x50, 0]), STATUS_SUCCESS);
    m.words.insert(0x54, 2);
    assert_eq!(run(&mut a, &mut m, 228, &[0x50, 0]), STATUS_SUCCESS);
    assert_eq!(a.calendar.sets, 2);
    run(&mut a, &mut m, 128, &[0x40]);
    assert_eq!((m.words[&0x40], m.words[&0x44]), (0, 2));
    let no_wall = HostTime { wall: None, ..T };
    assert_eq!(
        a.call(
            Call::SetSystemTime {
                new_time: 0x50,
                old_time: 0
            },
            no_wall,
            &mut m
        ),
        STATUS_UNSUCCESSFUL
    );
}
