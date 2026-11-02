//! T1840 tests. Synthetic scenarios are FABRICATED inputs checked against rules MEASURED or
//! INFERRED in docs/t-story-event-verbs.md. Real-data tests skip without TSFP_SETU_PAK_DIR
//! (a directory of .pak files made by `python3 -m tools.setu_dump_paks`); no game data is
//! committed.

use std::collections::HashMap;
use std::path::PathBuf;

use tsfp_gameplay::story_events::record::{Condition, EventRecord, Link};
use tsfp_gameplay::story_events::setu::{parse_events, setu_blobs_from_pak, write_events};
use tsfp_gameplay::story_events::world::{
    CallbackTarget, IdVariant, PropRequest, SpawnRequest, VoiceRequest, World,
};
use tsfp_gameplay::story_events::{BuildOptions, Engine, EngineError, LinkSide, RECORD_SIZE};

const NONE: u32 = u32::MAX;

fn record(event_id: u32, verb: u32) -> EventRecord {
    EventRecord {
        head: [0, 0],
        verb,
        verb_args: [NONE; 11],
        callback: 0,
        tick_callback: 0,
        link_a: Link {
            mode: 3,
            id: 0,
            args: [NONE; 4],
        },
        gate_id: 0,
        gate_mode: 0,
        combine: 0,
        link_b: Link::default(),
        condition_a: Condition {
            kind: 0,
            args: [NONE; 4],
        },
        condition_b: Condition {
            kind: 0,
            args: [NONE; 4],
        },
        delay_bits: 0f32.to_bits(),
        weight_bits: 1f32.to_bits(),
        event_id,
        flags: 0,
        mode_mask: 0x700,
        checkpoint_mask: 1,
        restore_id: 0,
        unknown_c0: event_id,
        unknown_c4: 0,
    }
}

#[derive(Default)]
struct TestWorld {
    log: Vec<String>,
    voice_frames_left: HashMap<u32, u32>,
    next_handle: u32,
    voice_length: u32,
    players: Vec<Vec<u32>>,
    tags: Vec<u32>,
    polled: bool,
    object_ready: bool,
    prop_settled: bool,
    other_verbs_ok: bool,
}

impl World for TestWorld {
    fn play_voice(&mut self, request: &VoiceRequest) -> Option<u32> {
        self.next_handle += 1;
        self.log
            .push(format!("voice {} prio={}", request.label, request.priority));
        self.voice_frames_left
            .insert(self.next_handle, self.voice_length);
        Some(self.next_handle)
    }
    fn voice_finished(&mut self, handle: u32) -> bool {
        let left = self.voice_frames_left.entry(handle).or_insert(0);
        if *left == 0 {
            return true;
        }
        *left -= 1;
        false
    }
    fn run_callback(&mut self, callback: u32, target: CallbackTarget) {
        self.log.push(format!("callback {callback} {target:?}"));
    }
    fn set_objective(&mut self, actor_key: u32, index: u32) {
        self.log.push(format!("objective {actor_key:#x} {index}"));
    }
    fn control_prop(&mut self, request: &PropRequest) {
        self.log.push(format!(
            "prop {:#x} {} {}",
            request.key, request.activate, request.hold
        ));
    }
    fn prop_settled(&mut self, _key: u32) -> bool {
        self.prop_settled
    }
    fn spawn_object(&mut self, request: &SpawnRequest) -> Option<u32> {
        self.next_handle += 1;
        self.log.push(format!(
            "spawn type={} pos={:?}",
            request.object_type, request.position
        ));
        Some(self.next_handle)
    }
    fn object_ready(&mut self, _handle: u32) -> bool {
        self.object_ready
    }
    fn destroy_object(&mut self, handle: u32) {
        self.log.push(format!("destroy {handle}"));
    }
    fn player_count(&self) -> usize {
        self.players.len()
    }
    fn player_matches(&self, player: usize, _variant: IdVariant, ids: &[u32]) -> bool {
        self.players[player].iter().any(|id| ids.contains(id))
    }
    fn first_player_flag(&self) -> bool {
        false
    }
    fn tag_exists(&self, key: u32) -> bool {
        self.tags.contains(&key)
    }
    fn poll_link(&mut self, _link: &Link) -> bool {
        self.polled
    }
    fn other_verb(&mut self, verb: u32, instance: usize) -> bool {
        self.log.push(format!("other verb {verb} #{instance}"));
        self.other_verbs_ok
    }
}

fn run_frames(engine: &mut Engine, world: &mut TestWorld, frames: usize) -> Vec<u32> {
    let mut order = Vec::new();
    for _ in 0..frames {
        order.extend(
            engine
                .step(0.1, world)
                .fired
                .iter()
                .map(|fired| fired.event_id),
        );
    }
    order
}

fn started(records: &[EventRecord]) -> Engine {
    let mut engine = Engine::new(records, BuildOptions::default());
    engine.set_checkpoint_mask(1);
    engine
}

#[test]
fn record_round_trip_is_byte_identical() {
    let mut bytes = [0u8; RECORD_SIZE];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = (i * 37 + 11) as u8;
    }
    let parsed = EventRecord::from_bytes(&bytes).expect("200 bytes");
    assert_eq!(parsed.to_bytes(), bytes);
    assert_eq!(
        parsed.verb,
        u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]])
    );
    assert_eq!(
        parsed.event_id,
        u32::from_le_bytes([bytes[0xAC], bytes[0xAD], bytes[0xAE], bytes[0xAF]])
    );
    assert!(EventRecord::from_bytes(&bytes[..199]).is_none());
}

#[test]
fn setu_blob_round_trip_synthetic() {
    let mut blob = vec![0u8; 0xC8 + 3 * RECORD_SIZE];
    blob[..4].copy_from_slice(b"SETU");
    blob[0x90..0x94].copy_from_slice(&0xC8u32.to_le_bytes());
    blob[0x94..0x98].copy_from_slice(&3u32.to_le_bytes());
    blob[0x98..0x9C].copy_from_slice(&0xC8u32.to_le_bytes());
    for (i, byte) in blob[0xC8..].iter_mut().enumerate() {
        *byte = (i % 251) as u8;
    }
    let records = parse_events(&blob).expect("parse");
    assert_eq!(records.len(), 3);
    let mut copy = vec![0u8; blob.len()];
    copy.copy_from_slice(&blob);
    copy[0xC8..].fill(0);
    write_events(&mut copy, &records).expect("write");
    assert_eq!(copy, blob);
    blob[0x98] = 0x60;
    assert!(parse_events(&blob).is_err());
}

#[test]
fn chain_of_previous_done_runs_in_file_order_in_one_frame() {
    let records = [record(1, 72), record(2, 72), record(3, 72)];
    let mut world = TestWorld::default();
    // First has link mode 3 with no predecessor, so it needs a slot-style kick: make it mode 0.
    let mut first = records;
    first[0].link_a.mode = 0;
    let mut engine = started(&first);
    assert_eq!(run_frames(&mut engine, &mut world, 1), vec![1, 2, 3]);
    assert!(world.log.iter().all(|line| line.starts_with("objective")));
    assert_eq!(run_frames(&mut engine, &mut world, 3), Vec::<u32>::new());
}

#[test]
fn checkpoint_mask_gates_the_active_list() {
    let mut records = [record(1, 72)];
    records[0].link_a.mode = 0;
    records[0].checkpoint_mask = 0b100;
    let mut engine = Engine::new(&records, BuildOptions::default());
    let mut world = TestWorld::default();
    engine.set_checkpoint_mask(0b011);
    assert!(engine.active().is_empty());
    assert!(run_frames(&mut engine, &mut world, 2).is_empty());
    engine.set_checkpoint_mask(0b110);
    assert_eq!(engine.active(), &[0]);
    assert_eq!(run_frames(&mut engine, &mut world, 1), vec![1]);
}

#[test]
fn difficulty_flags_and_mode_mask() {
    let mut records = [record(1, 72), record(2, 72)];
    records[0].link_a.mode = 0;
    records[1].link_a.mode = 0;
    records[0].flags = 2; // difficulty 1 only
    records[1].mode_mask = 0x1; // dropped by the build predicate 0x6B2B0 (T1841: decoded, story sub-mode 0 wants bit 0x100)
    let mut engine = Engine::new(&records, BuildOptions::story(0, 0));
    engine.set_checkpoint_mask(1);
    assert_eq!(engine.dropped(), 1);
    assert!(engine.active().is_empty());
    let mut hard = Engine::new(&records, BuildOptions::story(1, 0));
    hard.set_checkpoint_mask(1);
    assert_eq!(hard.active(), &[0]);
}

#[test]
fn delay_is_counted_down_and_conditions_block() {
    let mut records = [record(1, 72)];
    records[0].link_a.mode = 0;
    records[0].delay_bits = 0.35f32.to_bits();
    let mut engine = started(&records);
    let mut world = TestWorld::default();
    assert!(run_frames(&mut engine, &mut world, 3).is_empty());
    assert_eq!(run_frames(&mut engine, &mut world, 1), vec![1]);

    records[0].condition_a = Condition {
        kind: 35,
        args: [0x55, NONE, NONE, NONE],
    };
    let mut blocked = started(&records);
    let mut world = TestWorld::default();
    assert!(run_frames(&mut blocked, &mut world, 10).is_empty());
    world.tags.push(0x55);
    assert_eq!(run_frames(&mut blocked, &mut world, 4), vec![1]);
}

#[test]
fn unsupported_condition_is_an_error_not_a_guess() {
    let mut records = [record(1, 72)];
    records[0].link_a.mode = 0;
    records[0].condition_b.kind = 5;
    let mut engine = started(&records);
    let mut world = TestWorld::default();
    let report = engine.step(0.1, &mut world);
    assert!(report.fired.is_empty());
    assert_eq!(
        report.errors,
        vec![(
            0,
            EngineError::Unsupported {
                what: "condition kind",
                value: 5
            }
        )]
    );
    // T1841: link mode 7 is not an error, the builder gives it a slot without a handler (MEASURED),
    // so it simply never becomes satisfied.
    records[0].condition_b.kind = 0;
    records[0].link_a.mode = 7;
    let mut engine = started(&records);
    let report = engine.step(0.1, &mut world);
    assert!(report.errors.is_empty() && report.fired.is_empty());
}

#[test]
fn player_conditions() {
    let mut records = [record(1, 72)];
    records[0].link_a.mode = 0;
    records[0].condition_a = Condition {
        kind: 12,
        args: [7, NONE, NONE, NONE],
    }; // all players pass a
    let mut engine = started(&records);
    let mut world = TestWorld {
        players: vec![vec![7], vec![9]],
        ..TestWorld::default()
    };
    assert!(run_frames(&mut engine, &mut world, 2).is_empty());
    world.players[1] = vec![7];
    assert_eq!(run_frames(&mut engine, &mut world, 1), vec![1]);
}

#[test]
fn combine_operators() {
    let build = |combine: u32| {
        let mut records = [record(1, 72)];
        records[0].link_a = Link {
            mode: 1,
            id: 0,
            args: [NONE; 4],
        };
        records[0].link_b = Link {
            mode: 1,
            id: 0,
            args: [NONE; 4],
        };
        records[0].combine = combine;
        started(&records)
    };
    for (combine, a, b, fires) in [
        (0, true, false, true),
        (1, true, false, false),
        (1, true, true, true),
        (2, false, true, true),
        (2, false, false, false),
        (3, true, false, true),
        (3, true, true, false),
    ] {
        let mut engine = build(combine);
        if a {
            engine.activate_link(0, LinkSide::A);
        }
        if b {
            engine.activate_link(0, LinkSide::B);
        }
        let mut world = TestWorld::default();
        assert_eq!(
            !run_frames(&mut engine, &mut world, 1).is_empty(),
            fires,
            "op {combine} a={a} b={b}"
        );
    }
}

#[test]
fn gate_modes() {
    // Event 10 is the gate. Gate id lives in the gated record's +0x58.
    let build = |gate_mode: u32, gate_runs: bool| {
        let mut gate = record(10, 72);
        gate.link_a = Link {
            mode: 1,
            id: 0,
            args: [NONE; 4],
        };
        let mut gated = record(20, 72);
        gated.link_a.mode = 0;
        gated.gate_id = 10;
        gated.gate_mode = gate_mode;
        let mut engine = started(&[gate, gated]);
        if gate_runs {
            engine.activate_link(0, LinkSide::A);
        }
        engine
    };
    let mut world = TestWorld::default();
    // mode 1: runs only after the gate ran.
    assert_eq!(
        run_frames(&mut build(1, false), &mut world, 3),
        Vec::<u32>::new()
    );
    assert_eq!(run_frames(&mut build(1, true), &mut world, 2), vec![10, 20]);
    // mode 3: runs only while the gate has not run (frame order: gated runs first, gate second).
    assert_eq!(run_frames(&mut build(3, false), &mut world, 2), vec![20]);
    let after_gate = run_frames(&mut build(3, true), &mut world, 2);
    assert_eq!(
        after_gate,
        vec![10],
        "gate ran first in pool order so mode 3 blocks"
    );
    // mode 2: fires as soon as the gate ran, ignoring (here unsatisfied) links.
    let mut gate = record(10, 72);
    gate.link_a.mode = 0;
    let mut gated = record(20, 72);
    gated.link_a = Link {
        mode: 1,
        id: 0,
        args: [NONE; 4],
    };
    gated.gate_id = 10;
    gated.gate_mode = 2;
    assert_eq!(
        run_frames(&mut started(&[gate, gated]), &mut world, 2),
        vec![10, 20]
    );
}

#[test]
fn link_mode_4_waits_for_target_event() {
    let mut target = record(77, 72);
    target.link_a = Link {
        mode: 1,
        id: 0,
        args: [NONE; 4],
    };
    let mut waiter = record(1, 72);
    waiter.link_a = Link {
        mode: 4,
        id: 77,
        args: [NONE; 4],
    };
    let mut engine = started(&[waiter, target]);
    let mut world = TestWorld::default();
    assert!(run_frames(&mut engine, &mut world, 3).is_empty());
    engine.activate_link(1, LinkSide::A);
    assert_eq!(run_frames(&mut engine, &mut world, 2), vec![77, 1]);
}

#[test]
fn repeat_flag_rearms() {
    let mut records = [record(1, 72)];
    // Mode 5 (slot, no re-set): the re-armed link waits for a new slot event.
    records[0].link_a = Link {
        mode: 5,
        id: 0,
        args: [NONE; 4],
    };
    records[0].flags = 0x20;
    let mut engine = started(&records);
    let mut world = TestWorld::default();
    engine.activate_link(0, LinkSide::A);
    assert_eq!(run_frames(&mut engine, &mut world, 1), vec![1]);
    assert!(
        run_frames(&mut engine, &mut world, 3).is_empty(),
        "re-armed link waits for a new event"
    );
    engine.activate_link(0, LinkSide::A);
    assert_eq!(run_frames(&mut engine, &mut world, 2), vec![1]);
    assert_eq!(engine.trace(0).executions, 2);

    // Mode 1 is set again immediately on re-arm (spec 4.2), so it fires every other frame.
    records[0].link_a.mode = 1;
    let mut engine = started(&records);
    engine.activate_link(0, LinkSide::A);
    assert!(run_frames(&mut engine, &mut world, 6).len() >= 3);
}

#[test]
fn verbs_0_72_16_effects_and_completion() {
    let mut callback = record(1, 0);
    callback.link_a.mode = 0;
    callback.callback = 26;
    let mut objective = record(2, 72);
    objective.verb_args[(0x10 - 0xC) / 4] = 5;
    objective.verb_args[(0x14 - 0xC) / 4] = 0x4142;
    let mut prop = record(3, 16);
    prop.verb_args[0] = 1;
    prop.verb_args[(0x10 - 0xC) / 4] = 0;
    prop.verb_args[(0x14 - 0xC) / 4] = 0x99;
    let mut after_prop = record(4, 72);
    after_prop.link_a.mode = 3;
    let mut engine = started(&[callback, objective, prop, after_prop]);
    let mut world = TestWorld::default();
    assert_eq!(run_frames(&mut engine, &mut world, 1), vec![1, 2, 3]);
    assert_eq!(
        world.log,
        vec![
            "callback 26 Instance(0)",
            "objective 0x4142 5",
            "prop 0x99 true false"
        ]
    );
    assert!(!engine.is_done(2), "prop verb waits for the tail pass");
    world.prop_settled = true;
    // Tail pass marks done after the state pass, so the successor fires one frame later.
    assert_eq!(run_frames(&mut engine, &mut world, 2), vec![4]);
    assert!(engine.is_done(2));
}

#[test]
fn verb_0_with_tick_callback_is_not_done() {
    let mut node = record(1, 0);
    node.link_a.mode = 0;
    node.tick_callback = 3;
    let mut next = record(2, 72);
    next.link_a.mode = 3;
    let mut engine = started(&[node, next]);
    let mut world = TestWorld::default();
    assert_eq!(run_frames(&mut engine, &mut world, 5), vec![1]);
}

#[test]
fn verb_3_spawn_lifetime() {
    let mut spawn = record(1, 3);
    spawn.link_a.mode = 0;
    spawn.callback = 9;
    spawn.verb_args[(0x10 - 0xC) / 4] = 42;
    spawn.verb_args[(0x20 - 0xC) / 4] = 1.5f32.to_bits();
    let mut follow = record(2, 72);
    follow.link_a.mode = 3;
    let mut engine = started(&[spawn, follow]);
    let mut world = TestWorld::default();
    assert_eq!(run_frames(&mut engine, &mut world, 3), vec![1]);
    assert!(world.log[0].starts_with("spawn type=42 pos=[1.5"));
    assert_eq!(world.log[1], "callback 9 Object(1)");
    world.object_ready = true;
    assert_eq!(run_frames(&mut engine, &mut world, 2), vec![2]);
    assert!(world.log.contains(&"destroy 1".to_string()));

    // Flag 0x10 keeps the object.
    spawn.flags = 0x10;
    let mut keep = started(&[spawn]);
    let mut world = TestWorld {
        object_ready: true,
        ..TestWorld::default()
    };
    run_frames(&mut keep, &mut world, 2);
    assert!(!world.log.iter().any(|line| line.starts_with("destroy")));
}

#[test]
fn verb_1_priority_queue() {
    let voice = |event_id: u32, label: u32, priority: bool| {
        let mut r = record(event_id, 1);
        r.link_a.mode = 0;
        r.verb_args[(0x10 - 0xC) / 4] = label;
        r.verb_args[(0x14 - 0xC) / 4] = NONE;
        r.flags = if priority { 0x40 } else { 0 };
        r
    };
    // A plays (priority). B (priority) queues. C (normal) is dropped. The dropped C completes in
    // its first tail pass and, like every finished verb-1 instance, pops the voice queue
    // (`0x237BC8`, MEASURED, T1841), so B starts in the same frame.
    let mut engine = started(&[
        voice(1, 100, true),
        voice(2, 200, true),
        voice(3, 300, false),
    ]);
    let mut world = TestWorld {
        voice_length: 2,
        ..TestWorld::default()
    };
    let order = run_frames(&mut engine, &mut world, 1);
    assert_eq!(order, vec![1, 2, 3]);
    assert_eq!(
        world.log,
        vec!["voice 100 prio=true", "voice 200 prio=true"]
    );
    assert!(engine.instance(2).done);
    // T1841: a voice lasts at least 0.5 s of subtitle time (tail `0x237AF2`), so 12 frames of 0.1 s.
    run_frames(&mut engine, &mut world, 12);
    assert!(engine.instance(0).done && engine.instance(1).done);

    // Without the dropped line B waits in the queue until A finishes.
    let mut engine = started(&[voice(1, 100, true), voice(2, 200, true)]);
    let mut world = TestWorld {
        voice_length: 2,
        ..TestWorld::default()
    };
    run_frames(&mut engine, &mut world, 1);
    assert_eq!(world.log, vec!["voice 100 prio=true"]);
    assert_eq!(engine.instance(1).running, 2, "queued = suppressed");
    run_frames(&mut engine, &mut world, 12);
    assert_eq!(
        world.log,
        vec!["voice 100 prio=true", "voice 200 prio=true"]
    );

    // Non-priority playing: a normal one plays at once.
    let mut engine = started(&[voice(1, 100, false), voice(2, 200, false)]);
    let mut world = TestWorld {
        voice_length: 3,
        ..TestWorld::default()
    };
    run_frames(&mut engine, &mut world, 1);
    assert_eq!(world.log.len(), 2);
}

#[test]
fn other_verbs_are_unsupported_unless_the_world_models_them() {
    let mut records = [record(1, 13), record(2, 72)];
    records[0].link_a.mode = 0;
    let mut engine = started(&records);
    let mut world = TestWorld::default();
    let report = engine.step(0.1, &mut world);
    assert_eq!(
        report.errors[0].1,
        EngineError::Unsupported {
            what: "verb",
            value: 13
        }
    );
    let mut engine = started(&records);
    let mut world = TestWorld {
        other_verbs_ok: true,
        ..TestWorld::default()
    };
    assert_eq!(run_frames(&mut engine, &mut world, 1), vec![1, 2]);
}

#[test]
fn fire_by_id_runs_immediately() {
    let mut records = [record(5, 72)];
    records[0].link_a.mode = 1;
    let mut engine = started(&records);
    let mut world = TestWorld::default();
    assert_eq!(engine.fire_by_id(5, &mut world), Some(0));
    assert_eq!(world.log.len(), 1);
    assert_eq!(engine.fire_by_id(6, &mut world), None);
}

// ---------------------------------------------------------------- real data (optional)

fn real_paks() -> Option<Vec<(String, Vec<u8>)>> {
    let dir = PathBuf::from(std::env::var_os("TSFP_SETU_PAK_DIR")?);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).ok()? {
        let path = entry.ok()?.path();
        if path.extension().is_some_and(|ext| ext == "pak") {
            out.push((
                path.file_name()?.to_string_lossy().into_owned(),
                std::fs::read(&path).ok()?,
            ));
        }
    }
    out.sort();
    (!out.is_empty()).then_some(out)
}

fn real_levels() -> Option<Vec<(String, Vec<EventRecord>)>> {
    let mut levels = Vec::new();
    for (name, bytes) in real_paks()? {
        for blob in setu_blobs_from_pak(&bytes).ok()? {
            levels.push((name.clone(), parse_events(&blob).ok()?));
        }
    }
    Some(levels)
}

#[test]
fn real_setu_records_round_trip() {
    let Some(paks) = real_paks() else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let mut records = 0usize;
    for (name, bytes) in &paks {
        for blob in setu_blobs_from_pak(bytes).expect("pak") {
            let (start, count) = tsfp_gameplay::story_events::setu::event_span(&blob).expect(name);
            let parsed = parse_events(&blob).expect(name);
            for (index, record) in parsed.iter().enumerate() {
                let at = start + index * RECORD_SIZE;
                assert_eq!(
                    record.to_bytes().as_slice(),
                    &blob[at..at + RECORD_SIZE],
                    "{name} #{index}"
                );
                assert!(record.verb < 93, "{name} #{index} verb {}", record.verb);
            }
            let mut rewritten = blob.clone();
            write_events(&mut rewritten, &parsed).expect("write");
            assert_eq!(rewritten, blob, "{name}");
            records += count;
        }
    }
    // MEASURED by tools.setu_census (T1838): 5864 definitions in 55 SETU entries.
    assert_eq!(records, 5864);
}

/// Optimistic world for replays: all polled links true, every player matches, voices last one
/// frame, props settle, objects ready, other verbs are no-ops. FABRICATED scenario inputs.
#[derive(Default)]
struct ReplayWorld {
    handle: u32,
}

impl World for ReplayWorld {
    fn play_voice(&mut self, _r: &VoiceRequest) -> Option<u32> {
        self.handle += 1;
        Some(self.handle)
    }
    fn voice_finished(&mut self, _h: u32) -> bool {
        true
    }
    fn run_callback(&mut self, _c: u32, _t: CallbackTarget) {}
    fn set_objective(&mut self, _a: u32, _i: u32) {}
    fn control_prop(&mut self, _r: &PropRequest) {}
    fn prop_settled(&mut self, _k: u32) -> bool {
        true
    }
    fn spawn_object(&mut self, _r: &SpawnRequest) -> Option<u32> {
        self.handle += 1;
        Some(self.handle)
    }
    fn object_ready(&mut self, _h: u32) -> bool {
        true
    }
    fn destroy_object(&mut self, _h: u32) {}
    fn player_count(&self) -> usize {
        1
    }
    fn player_matches(&self, _p: usize, _v: IdVariant, _i: &[u32]) -> bool {
        true
    }
    fn first_player_flag(&self) -> bool {
        true
    }
    fn tag_exists(&self, _k: u32) -> bool {
        true
    }
    fn poll_link(&mut self, _l: &Link) -> bool {
        true
    }
    fn other_verb(&mut self, _v: u32, _i: usize) -> bool {
        true
    }
}

fn replay(records: &[EventRecord], frames: usize) -> (Engine, Vec<u32>, usize) {
    let mut engine = Engine::new(records, BuildOptions::default());
    engine.set_checkpoint_mask(u32::MAX);
    let mut world = ReplayWorld::default();
    engine.activate_all_slot_links(&mut world);
    let mut order = Vec::new();
    let mut errors = 0;
    for _ in 0..frames {
        let report = engine.step(0.1, &mut world);
        order.extend(report.fired.iter().map(|fired| fired.instance as u32));
        errors += report.errors.len();
    }
    (engine, order, errors)
}

#[test]
fn real_level_replay_matches_spec_invariants() {
    let Some(levels) = real_levels() else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let (mut total_fired, mut total_errors, mut total_instances, mut total_levels) =
        (0usize, 0usize, 0usize, 0usize);
    for (name, records) in levels.iter().filter(|(_, r)| !r.is_empty()) {
        let (engine, order, errors) = replay(records, 400);
        let (again, order_again, _) = replay(records, 400);
        assert_eq!(order, order_again, "{name}: deterministic firing order");
        assert_eq!(engine.len(), again.len());
        total_fired += order.len();
        total_errors += errors;
        total_instances += engine.len();
        total_levels += 1;
        for index in 0..engine.len() {
            let record = engine.record(index);
            let trace = engine.trace(index);
            // Spec 4.1: the verb never runs before its delay elapsed since it fired (the
            // countdown starts in the firing frame, so one frame of slack, INFERRED).
            if let (Some(fired), Some(executed)) = (trace.fired_at, trace.executed_at) {
                assert!(
                    executed + 0.101 >= fired + record.delay(),
                    "{name} #{index}: delay"
                );
            }
            // Spec 4.1 step 7 and 4.2: link A 2 / 3 / 4 order against the predecessor or target,
            // for single-link, ungated, unconditioned definitions that never repeat.
            let simple = record.combine == 0 && record.gate_mode == 0 && !record.is_repeatable();
            if let (true, Some(executed)) = (simple, trace.executed_at) {
                let predecessor = |wanted_done: bool| {
                    let prev = engine.trace(index.checked_sub(1)?);
                    if wanted_done {
                        prev.done_at
                    } else {
                        prev.executed_at
                    }
                };
                let bound = match record.link_a.mode {
                    2 => predecessor(false),
                    3 => predecessor(true),
                    4 => engine
                        .instance(index)
                        .link_a_target
                        .and_then(|t| engine.trace(t).executed_at),
                    _ => None,
                };
                if let Some(bound) = bound {
                    assert!(executed + 1e-3 >= bound, "{name} #{index}: link order");
                }
                // A first-in-file mode 2/3 definition has no predecessor and must never run.
                if index == 0 && matches!(record.link_a.mode, 2 | 3) {
                    panic!("{name}: first definition with predecessor link ran");
                }
            }
        }
    }
    eprintln!(
        "replay: {total_levels} SETU entries, {total_instances} instances, {total_fired} executions, {total_errors} unsupported-item reports"
    );
    assert!(total_fired > 1000, "replay fired only {total_fired} events");
}
