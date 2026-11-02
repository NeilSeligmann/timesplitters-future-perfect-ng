// SPDX-License-Identifier: GPL-3.0-or-later
//! T3820: a session recorded with raw look active replays with raw look active, although the replay's own guard
//! (fed replay-time hook counters that stall, the L3/L4 desync of `docs/t-replay-desync-l3-l4.md`) falls back.

use bevy_app::App;
use bevy_input::InputPlugin;
use bevy_input::keyboard::KeyboardInput;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tsfp_input::guard::{GuardEvent, HostProbe, RawLookGuard};
use tsfp_input::recording::{Payload, Reader, Record, Writer};
use tsfp_input::router::InputSource;
use tsfp_input_intake::decisions;

static POLL: AtomicU64 = AtomicU64::new(0);
static WRITER: Mutex<Option<Writer<Vec<u8>>>> = Mutex::new(None);

struct TestHost;

impl tsfp_input_intake::Host for TestHost {
    fn clock(&self) -> (u64, u64) {
        let poll = POLL.load(Ordering::SeqCst);
        (poll, poll)
    }
    fn write(&self, sim_tick: u64, host_poll: u64, payload: String) {
        let row = Payload {
            sim_tick,
            host_poll,
            wall_ns: host_poll,
            payload,
        };
        WRITER
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .device(&row)
            .unwrap();
    }
    fn replay_poll(&self) -> u64 {
        POLL.load(Ordering::SeqCst)
    }
    fn live_key(&self, _key: &KeyboardInput) {}
    fn progress(&self) {}
}

const TICKS: u64 = 200;

#[test]
fn a_raw_look_session_replays_with_raw_look_active() {
    tsfp_input_intake::set_host(Box::new(TestHost));
    let mut app = App::new();
    app.add_plugins(InputPlugin);
    tsfp_input_intake::install(&mut app);

    // Live: the look hook runs every tick, raw look stays active; the keyboard player owns slot 0, a pad slot 1.
    *WRITER.lock().unwrap() = Some(Writer::stamped(Vec::new(), false, None).unwrap());
    tsfp_input_intake::start_recording();
    let mut guard = RawLookGuard::new();
    let live_sources = [
        Some(InputSource::MouseKeyboard),
        Some(InputSource::Pad(7)),
        None,
        None,
    ];
    for tick in 0..TICKS {
        POLL.store(tick, Ordering::SeqCst);
        app.update();
        let probe = HostProbe {
            reachable: true,
            hits: tick as u32 + 1,
        };
        guard.update(tick as f32 / 60.0, true, probe);
        assert!(decisions::raw_look(guard.raw()));
        assert_eq!(decisions::slot_sources(live_sources), live_sources);
    }
    tsfp_input_intake::stop();
    let bytes = WRITER.lock().unwrap().take().unwrap().finish().unwrap();
    let mut reader = Reader::new(bytes.as_slice()).unwrap();
    let mut rows = Vec::new();
    while let Some(record) = reader.next().unwrap() {
        if let Record::Device(row) = record {
            rows.push((row.host_poll, row.payload));
        }
    }
    assert_eq!(
        rows.len(),
        2,
        "one row per decision, written when it changes: {rows:?}"
    );

    // Replay: the hook is reached once, then never at the replay's ticks; the replay's guard falls back.
    tsfp_input_intake::start_replay(rows);
    let mut guard = RawLookGuard::new();
    let mut fell_back = false;
    for tick in 0..TICKS {
        POLL.store(tick, Ordering::SeqCst);
        app.update();
        let probe = HostProbe {
            reachable: true,
            hits: 1,
        };
        fell_back |= matches!(
            guard.update(tick as f32 / 60.0, true, probe),
            Some(GuardEvent::NotActive(_))
        );
        assert!(
            decisions::raw_look(guard.raw()),
            "tick {tick}: the recorded raw look holds"
        );
        assert_eq!(
            decisions::slot_sources([None; 4]),
            live_sources,
            "tick {tick}"
        );
    }
    assert!(
        fell_back && !guard.raw(),
        "the replay-time guard did fall back"
    );
    let report = tsfp_input_intake::report().unwrap();
    tsfp_input_intake::stop();
    assert!(report.contains("decisions applied=2"), "{report}");
}
