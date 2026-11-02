// SPDX-License-Identifier: GPL-3.0-or-later
//! The owner's player is frozen for a wall-time dependent stretch while the mouse keeps moving; the replay's freeze
//! ends at another tick (docs/t-replay-desync-l3-l4.md, "Control lock"). Recorded `control` decisions make the replay
//! drop the look recorded under the owner's lock and hold the rest until its own release.

use bevy_app::App;
use bevy_input::InputPlugin;
use bevy_input::keyboard::KeyboardInput;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tsfp_input::control_lock::ControlLock;
use tsfp_input::player::{AssistMask, Look, MenuIntent, PlayerInput, SourceKind, Vec2};
use tsfp_input::recording::{Payload, Reader, Record, Writer};
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

const TICKS: u64 = 120;
const OWNER_RELEASE: u64 = 40;
const REPLAY_RELEASE: u64 = 70;

fn row(tick: u64) -> PlayerInput {
    PlayerInput {
        step: tick,
        slot: 0,
        source: SourceKind::MouseKeyboard,
        held: 0,
        pressed: 0,
        released: 0,
        move_axis: Vec2::default(),
        look: Look::Delta {
            yaw_deg: tick as f32,
            pitch_deg: 0.0,
        },
        triggers: [0.0; 2],
        menu: MenuIntent::default(),
        assists: AssistMask::default(),
    }
}

fn yaw(input: &PlayerInput) -> f32 {
    match input.look {
        Look::Delta { yaw_deg, .. } => yaw_deg,
        Look::Stick(_) => unreachable!(),
    }
}

#[test]
fn a_replay_follows_the_owners_lock_and_its_own_release() {
    tsfp_input_intake::set_host(Box::new(TestHost));
    let mut app = App::new();
    app.add_plugins(InputPlugin);
    tsfp_input_intake::install(&mut app);

    *WRITER.lock().unwrap() = Some(Writer::stamped(Vec::new(), false, None).unwrap());
    tsfp_input_intake::start_recording();
    for tick in 0..TICKS {
        POLL.store(tick, Ordering::SeqCst);
        app.update();
        tsfp_input_intake::decide(
            decisions::CONTROL,
            if tick < OWNER_RELEASE { "1" } else { "0" },
        );
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
    assert_eq!(rows.len(), 2, "one row per change of the lock: {rows:?}");

    tsfp_input_intake::start_replay(rows);
    let mut gate = ControlLock::default();
    let mut applied = Vec::new();
    for tick in 0..TICKS {
        POLL.store(tick, Ordering::SeqCst);
        app.update();
        let replay_locked = tick < REPLAY_RELEASE;
        let recorded_locked =
            tsfp_input_intake::recorded_decision(decisions::CONTROL).as_deref() == Some("1");
        applied.push(yaw(&gate.apply(row(tick), recorded_locked, replay_locked)));
    }
    let report = tsfp_input_intake::report().unwrap();
    tsfp_input_intake::stop();

    // Nothing recorded under the owner's lock, and nothing before the replay's release, reaches the player.
    assert!(
        applied[..REPLAY_RELEASE as usize].iter().all(|&v| v == 0.0),
        "{applied:?}"
    );
    // The rows after the owner's release come out from the replay's release on, in order, shifted by the difference.
    let shift = REPLAY_RELEASE - OWNER_RELEASE;
    for tick in REPLAY_RELEASE..TICKS {
        assert_eq!(
            applied[tick as usize],
            (tick - shift) as f32,
            "tick {tick}: the owner's tick {} after the replay's own release",
            tick - shift
        );
    }
    assert!(
        report.contains("overridden=0"),
        "the replay's own lock is not an override: {report}"
    );
}
