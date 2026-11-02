// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic host-write witness replay: ordering, bypass-shaped scalar writes, negatives and mutations.
use tsfp_host_memory_capture::{
    Writer,
    host_write::{Error, replay},
    producer::Producer,
    read,
};

const ALLOC: u32 = 1;
const WRITE: u32 = 30;
const OPS: [(u32, usize, usize); 2] = [(ALLOC, 0, 0), (WRITE, 2, 0)];
type Trace = Producer<Vec<u8>>;

fn trace() -> Trace {
    Producer::new(
        Writer::new(Vec::new(), 8192).unwrap(),
        b"synthetic-host-write",
        true,
        &OPS,
    )
    .unwrap()
}
fn place(t: &mut Trace, base: u32, len: u64) {
    let id = t.begin(1, ALLOC, &[]).unwrap();
    t.placement(1, 0, len, base, base).unwrap();
    t.end(1, id, base, &[]).unwrap();
}
fn host_write(t: &mut Trace, addr: u32, bytes: &[u8]) {
    let id = t.begin(1, WRITE, &[addr, bytes.len() as u32]).unwrap();
    t.memory(1, true, addr, bytes).unwrap();
    t.end(1, id, 1, &[]).unwrap();
}
fn decode(t: Trace) -> Vec<(u32, Vec<u8>)> {
    read(t.seal().unwrap().as_slice(), 8192).unwrap()
}

#[test]
fn ordered_u8_u32_and_bytes_writes_replay_last_writer_wins() {
    let mut t = trace();
    place(&mut t, 0x10000, 0x20);
    host_write(&mut t, 0x10000, &0xdead_beefu32.to_le_bytes());
    host_write(&mut t, 0x10001, &[0x55]);
    host_write(&mut t, 0x10008, b"abcdefgh");
    let regions = replay(&decode(t), WRITE).unwrap();
    let bytes = &regions[&0x10000];
    assert_eq!(&bytes[..4], &[0xef, 0x55, 0xad, 0xde]);
    assert_eq!(&bytes[8..16], b"abcdefgh");
    assert!(bytes[4..8].iter().chain(&bytes[16..]).all(|b| *b == 0));
}

#[test]
fn write_outside_placed_region_is_rejected_not_fabricated() {
    let mut t = trace();
    place(&mut t, 0x10000, 0x10);
    host_write(&mut t, 0x1000e, &[1, 2, 3, 4]);
    assert!(matches!(replay(&decode(t), WRITE), Err(Error::Access(_))));
}

#[test]
fn wrapper_without_exactly_one_matching_write_is_rejected() {
    for variant in 0..3 {
        let mut t = trace();
        place(&mut t, 0x10000, 0x10);
        let id = t.begin(1, WRITE, &[0x10000, 4]).unwrap();
        match variant {
            0 => {}                                            // no write
            1 => t.memory(1, true, 0x10004, &[0; 4]).unwrap(), // wrong address
            _ => {
                t.memory(1, true, 0x10000, &[0; 4]).unwrap();
                t.memory(1, true, 0x10000, &[0; 4]).unwrap(); // duplicate
            }
        }
        t.end(1, id, 1, &[]).unwrap();
        assert!(
            matches!(replay(&decode(t), WRITE), Err(Error::Wrapper(_))),
            "variant {variant}"
        );
    }
}

#[test]
fn failed_result_and_stale_read_are_rejected() {
    let mut t = trace();
    place(&mut t, 0x10000, 0x10);
    let id = t.begin(1, WRITE, &[0x10000, 1]).unwrap();
    t.memory(1, true, 0x10000, &[9]).unwrap();
    t.end(1, id, 0, &[]).unwrap();
    assert!(matches!(replay(&decode(t), WRITE), Err(Error::Wrapper(_))));
    let mut t = trace();
    place(&mut t, 0x10000, 0x10);
    host_write(&mut t, 0x10000, &[9]);
    let id = t.begin(1, ALLOC, &[]).unwrap();
    t.memory(1, false, 0x10000, &[8]).unwrap(); // reads a value that was never written
    t.end(1, id, 0, &[]).unwrap();
    assert!(matches!(replay(&decode(t), WRITE), Err(Error::Access(_))));
}

#[test]
fn mutated_or_dropped_write_changes_replayed_bytes_or_fails() {
    let mut t = trace();
    place(&mut t, 0x10000, 0x10);
    host_write(&mut t, 0x10000, &[1]);
    host_write(&mut t, 0x10000, &[2]);
    let events = decode(t);
    let honest = replay(&events, WRITE).unwrap();
    assert_eq!(honest[&0x10000][0], 2);
    let mut reordered = events.clone();
    let writes: Vec<usize> = (0..reordered.len())
        .filter(|i| reordered[*i].0 == 2)
        .collect();
    let (a, b) = (writes[writes.len() - 2], writes[writes.len() - 1]);
    reordered.swap(a, b); // swap the two wrapper begins only: ids no longer match their ends
    assert!(replay(&reordered, WRITE).is_err());
    let mut dropped = events.clone();
    let memory = dropped.iter().rposition(|e| e.0 == 4).unwrap();
    dropped.remove(memory);
    assert!(matches!(replay(&dropped, WRITE), Err(Error::Wrapper(_))));
    let mut flipped = events;
    let memory = flipped.iter().rposition(|e| e.0 == 4).unwrap();
    *flipped[memory].1.last_mut().unwrap() ^= 0xff;
    assert_ne!(
        replay(&flipped, WRITE).unwrap()[&0x10000][0],
        honest[&0x10000][0]
    );
}

#[test]
fn unwrapped_events_and_unbalanced_stack_are_layout_errors() {
    let mut t = trace();
    place(&mut t, 0x10000, 0x10);
    host_write(&mut t, 0x10000, &[1]);
    let mut events = decode(t);
    events.pop(); // drop the final CALL_END
    assert_eq!(replay(&events, WRITE), Err(Error::Layout));
}

// Fence: a write is recorded after it happened, so it is only provably adjacent to its record when no other
// event entered the stream since the pre-write epoch. Anything else poisons the capture.
fn fenced_write(t: &mut Trace, thread: u64, epoch: u64, addr: u32, bytes: &[u8]) -> Option<()> {
    let id = t
        .begin_after(thread, epoch, WRITE, &[addr, bytes.len() as u32])
        .ok()?;
    t.memory(thread, true, addr, bytes).ok()?;
    t.end(thread, id, 1, &[]).ok()
}

#[test]
fn fenced_write_with_no_intervening_event_replays() {
    let mut t = trace();
    place(&mut t, 0x10000, 0x10);
    let epoch = t.epoch();
    fenced_write(&mut t, 1, epoch, 0x10000, &[9]).unwrap();
    let regions = replay(&decode(t), WRITE).unwrap();
    assert_eq!(regions[&0x10000][0], 9);
}

#[test]
fn write_overtaken_by_another_threads_complete_write_poisons_the_stream() {
    // Thread 1 mutates at epoch e, thread 2 records a whole write before thread 1 records: the order of the
    // two mutations is unknown, so replay would be last-record-wins, not last-mutation-wins.
    let mut t = trace();
    place(&mut t, 0x10000, 0x10);
    let epoch = t.epoch();
    fenced_write(&mut t, 2, epoch, 0x10000, &[2]).unwrap();
    assert!(fenced_write(&mut t, 1, epoch, 0x10000, &[1]).is_none());
    assert!(t.seal().is_err());
}

#[test]
fn write_overtaken_by_a_registry_call_or_open_call_poisons_the_stream() {
    let mut t = trace();
    place(&mut t, 0x10000, 0x10);
    let epoch = t.epoch();
    place(&mut t, 0x20000, 0x10); // another thread's registry call completed meanwhile
    assert!(fenced_write(&mut t, 1, epoch, 0x10000, &[1]).is_none());
    assert!(t.seal().is_err());

    let mut t = trace();
    let open = t.begin(2, ALLOC, &[]).unwrap(); // registry call still open on thread 2
    let epoch = t.epoch();
    assert!(fenced_write(&mut t, 1, epoch, 0x10000, &[1]).is_none());
    let _ = open;
    assert!(t.seal().is_err());
}

#[test]
fn stale_or_absent_epoch_is_never_accepted() {
    let mut t = trace();
    place(&mut t, 0x10000, 0x10);
    assert!(fenced_write(&mut t, 1, u64::MAX, 0x10000, &[1]).is_none());
    assert!(t.seal().is_err());
}
