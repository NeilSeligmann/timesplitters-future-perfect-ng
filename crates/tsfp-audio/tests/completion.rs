//! FABRICATED scenarios for the completion model (no retail data); equality with the C model is checked
//! by replaying a captured log (tests/slice1.rs, TSFP_AUDIO_CMD_LOG_FIXTURE).
use tsfp_audio::completion::*;

const HZ: u64 = 1000;

#[test]
fn packet_completes_after_running_time_only() {
    let mut c = Completion::new(HZ);
    c.pause_note(0x10, 0, 0).unwrap(); // run
    let (v, _) = c.stream_status(0x10, 1, 0, 2).unwrap();
    assert_eq!(v, 1); // room, nothing queued
    // 500 bytes at 1000 bytes/s = 500 ticks of running time
    let (r, _) = c.stream_process(0x10, 1, 0, 500, 0x100, 0x104, 1000, 22050, 2).unwrap();
    assert_eq!(r, Process::Queued);
    let (v, ev) = c.stream_status(0x10, 1, 100, 2).unwrap();
    assert_eq!((v, ev.len()), (1 | 0x10000, 0));
    c.pause_note(0x10, 1, 200).unwrap(); // hold: time does not count
    let (v, _) = c.stream_status(0x10, 1, 900, 2).unwrap();
    assert_eq!(v, 1 | 0x20000);
    c.pause_note(0x10, 0, 900).unwrap();
    let (v, ev) = c.stream_status(0x10, 1, 1199, 2).unwrap();
    assert_eq!((v & 0x10000 != 0, ev.len()), (true, 0));
    let (v, ev) = c.stream_status(0x10, 1, 1200, 2).unwrap();
    assert_eq!(v, 1);
    assert_eq!(ev, vec![Event { stream: 0x10, completed: 0x100, size: 500, status_addr: 0x104, status: 0 }]);
}

#[test]
fn list_full_and_dowork_delivery() {
    let mut c = Completion::new(HZ);
    c.dowork_delivery = true;
    c.pause_note(1, 0, 0).unwrap();
    assert_eq!(c.stream_process(1, 7, 0, 100, 0, 0, 1000, 8000, 1).unwrap().0, Process::Queued);
    assert_eq!(c.stream_process(1, 7, 0, 100, 0, 0, 1000, 8000, 1).unwrap().0, Process::ListFull);
    let (_, ev) = c.stream_status(1, 7, 150, 1).unwrap();
    assert!(ev.is_empty()); // finished but waits for DoWork
    assert_eq!(c.work(150).unwrap().len(), 1);
}

#[test]
fn buffer_finishes_loops_pauses() {
    let mut c = Completion::new(HZ);
    assert_eq!(c.buffer_status(5, 1, 0), 0);
    assert!(c.buffer_play(5, 1, 0, false, 1000, 1000, 0, 0)); // 1000 ticks long
    assert_eq!(c.buffer_status(5, 1, 10), 1);
    assert!(c.buffer_pause(5, 1, 20, 1, &mut |_| true));
    assert_eq!(c.buffer_status(5, 1, 500), 2);
    assert!(c.buffer_pause(5, 1, 500, 0, &mut |_| true));
    assert_eq!(c.buffer_status(5, 1, 1479), 1);
    assert_eq!(c.buffer_status(5, 1, 1480), 0); // 20 + (500-500) ... finished at 1480
    assert!(!c.buffer_pause(5, 1, 1500, 1, &mut |_| true)); // finished: Pause(1) refused
    assert!(c.buffer_play(5, 1, 2000, true, 1000, 1000, 0, 1000));
    assert_eq!(c.buffer_status(5, 1, 9000), 1 | 4);
    assert!(c.buffer_stop(5, 1, 9500, &mut || true)); // drains the rest of the data
    assert_eq!(c.buffer_status(5, 1, 9500), 1);
    assert_eq!(c.buffer_status(5, 1, 10000), 0);
    assert_eq!(c.stats.buffer_stops, 1);
}

#[test]
fn format_event_aborts_queued_packets() {
    let mut c = Completion::new(HZ);
    c.pause_note(2, 0, 0).unwrap();
    c.stream_process(2, 3, 0, 5000, 0x40, 0x44, 1000, 22050, 4).unwrap();
    let (ok, ev) = c.stream_format(2, 3, 10, 22050, 11025, true, &mut || true).unwrap();
    assert!(ok);
    assert_eq!(ev, vec![Event { stream: 2, completed: 0x40, size: 5000, status_addr: 0x44, status: ABORTED_STATUS }]);
    let (v, _) = c.stream_status(2, 3, 20, 4).unwrap();
    assert_eq!(v, 1); // queue empty
}
