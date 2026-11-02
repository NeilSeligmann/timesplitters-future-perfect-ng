// SPDX-License-Identifier: GPL-3.0-or-later
use tsfp_t2_offline::clock::ClockReplay;
use tsfp_t2_offline::spool::{ClockEvent, Run};

fn event(after_callees: u32, value: u64, byte: u8) -> ClockEvent {
    ClockEvent { after_callees, value, writes: vec![Run { addr: 0x270, data: vec![byte] }] }
}

#[test]
fn applies_each_read_and_its_writes_in_order() {
    let events = [event(0, 42, 1), event(1, 99, 2), event(1, 100, 3)];
    let mut replay = ClockReplay::new(&events);
    let mut writes = Vec::new();
    let mut write = |run: &Run| { writes.push((run.addr, run.data.clone())); Ok(()) };
    assert_eq!(replay.read(0, &mut write), Ok(42));
    assert_eq!(replay.read(1, &mut write), Ok(99));
    assert_eq!(replay.read(1, &mut write), Ok(100));
    assert!(replay.finish().is_ok());
    assert_eq!(writes, vec![(0x270, vec![1]), (0x270, vec![2]), (0x270, vec![3])]);
}

#[test]
fn rejects_missing_extra_and_out_of_order_reads() {
    let events = [event(1, 42, 1)];
    let mut replay = ClockReplay::new(&events);
    assert!(replay.finish().unwrap_err().contains("missing"));
    assert!(replay.read(0, |_| Ok(())).unwrap_err().contains("expected 1"));
    assert!(replay.read(1, |_| Ok(())).is_err());
    assert!(replay.finish().is_err());
    let mut replay = ClockReplay::new(&[]);
    assert!(replay.read(0, |_| Ok(())).unwrap_err().contains("unexpected"));
    assert!(replay.finish().is_err());
}

#[test]
fn failed_write_cannot_be_accepted() {
    let events = [event(0, 42, 1)];
    let mut replay = ClockReplay::new(&events);
    assert_eq!(replay.read(0, |_| Err("unmapped".into())), Err("unmapped".into()));
    assert_eq!(replay.finish(), Err("unmapped".into()));
}
