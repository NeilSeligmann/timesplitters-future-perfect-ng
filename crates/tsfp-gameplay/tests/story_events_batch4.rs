//! T1844 tests (batch 4): level hook table and special level ids (`levels`), tick function models and
//! catalogue (`ticks`), the persistent restore gate in the engine and a real-data census. Synthetic scenarios are
//! FABRICATED inputs against rules MEASURED in `build/default.xbe` (docs/t-story-event-engine.md section 8). The
//! real-data test skips without TSFP_SETU_PAK_DIR; no game data is committed.

use std::collections::BTreeSet;
use std::path::PathBuf;

use tsfp_gameplay::story_events::levels::{
    self, HookDispatch, HookSource, LEVEL_HOOKS, frame_hook_dispatch, hook_source,
    pending_restore_enabled, pool_size, restore_gate_applies,
};
use tsfp_gameplay::story_events::record::{Condition, EventRecord, Link};
use tsfp_gameplay::story_events::setu::{parse_events, setu_blobs_from_pak};
use tsfp_gameplay::story_events::ticks::{self, ShutterEffect, frame_dt, ramp_step, shutter_step};
use tsfp_gameplay::story_events::world::{
    IdVariant, PropRequest, SpawnRequest, VoiceRequest, World,
};
use tsfp_gameplay::story_events::{BuildOptions, Engine, ModeContext};

const NONE: u32 = u32::MAX;

fn record(event_id: u32, verb: u32) -> EventRecord {
    EventRecord {
        head: [0, 0],
        verb,
        verb_args: [NONE; 11],
        callback: 0,
        tick_callback: 0,
        link_a: Link {
            mode: 0,
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
struct Probe {
    hooks: Vec<u32>,
    class_flags: u32,
    spawn_ready: bool,
    voices: u32,
}

impl World for Probe {
    fn play_voice(&mut self, _r: &VoiceRequest) -> Option<u32> {
        self.voices += 1;
        Some(self.voices)
    }
    fn voice_finished(&mut self, _h: u32) -> bool {
        true
    }
    fn run_callback(
        &mut self,
        _callback: u32,
        _t: tsfp_gameplay::story_events::world::CallbackTarget,
    ) {
    }
    fn object_class_flags(&self, _t: u32) -> u32 {
        self.class_flags
    }
    fn persistent_spawn_ready(&mut self, _t: u32) -> bool {
        self.spawn_ready
    }
    fn run_level_hook(&mut self, address: u32) {
        self.hooks.push(address);
    }
    fn set_objective(&mut self, _k: u32, _i: u32) {}
    fn control_prop(&mut self, _r: &PropRequest) {}
    fn prop_settled(&mut self, _k: u32) -> bool {
        true
    }
    fn spawn_object(&mut self, _r: &SpawnRequest) -> Option<u32> {
        Some(1)
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
        false
    }
    fn tag_exists(&self, _k: u32) -> bool {
        true
    }
    fn poll_link(&mut self, _l: &Link) -> bool {
        false
    }
}

#[test]
fn level_table_has_the_measured_shape() {
    assert_eq!(LEVEL_HOOKS.len(), 87);
    let ids: BTreeSet<u32> = LEVEL_HOOKS.iter().map(|row| row.0).collect();
    assert_eq!(ids.len(), 87, "level ids are unique");
    assert_eq!(hook_source(1, true), HookSource::Static(0x1DDC90));
    assert_eq!(hook_source(1, false), HookSource::None);
    assert_eq!(hook_source(0x38, true), HookSource::Static(0x389B0));
    assert_eq!(hook_source(0x64, true), HookSource::None);
    assert_eq!(hook_source(0x65, false), HookSource::None);
    assert_eq!(hook_source(0x1B, false), HookSource::Static(0x389B0));
    assert_eq!(hook_source(0xFFFF, true), HookSource::None);
    // The 25 distinct non-null hooks stay countable (bodies are a follow-up).
    let distinct: BTreeSet<u32> = LEVEL_HOOKS
        .iter()
        .flat_map(|row| [row.1, row.2])
        .filter(|&a| a != 0)
        .collect();
    assert!(distinct.len() >= 20, "{} distinct hooks", distinct.len());
}

#[test]
fn level_hook_follows_the_story_struct_rule() {
    let story = ModeContext {
        level_id: 5,
        ..ModeContext::default()
    };
    assert_eq!(levels::level_hook(&story), HookSource::Static(0x220600));
    let arcade = ModeContext {
        mode_id: 3,
        level_id: 5,
        ..ModeContext::default()
    };
    assert_eq!(levels::level_hook(&arcade), HookSource::None);
    assert!(levels::uses_story_struct(10, 200, false));
    assert!(levels::uses_story_struct(3, 5, true));
    assert!(!levels::uses_story_struct(3, 0x65, true));
}

#[test]
fn frame_hook_dispatch_paths() {
    assert_eq!(frame_hook_dispatch(0, 0, 0), HookDispatch::Skip);
    assert_eq!(frame_hook_dispatch(1, 0, 0), HookDispatch::AfterObjective);
    assert_eq!(
        frame_hook_dispatch(1, 0x40, 2),
        HookDispatch::AfterObjective
    );
    assert_eq!(frame_hook_dispatch(1, 0x40, 0x40), HookDispatch::HookOnly);
    assert_eq!(frame_hook_dispatch(1, 0x40, 0), HookDispatch::Skip);
}

#[test]
fn engine_runs_the_level_hook_once_per_plan() {
    let context = ModeContext {
        level_id: 5,
        ..ModeContext::default()
    };
    let mut engine = Engine::new(&[], BuildOptions { context });
    let mut world = Probe::default();
    assert_eq!(
        engine.frame_level_hook(1, 0, &mut world),
        HookDispatch::AfterObjective
    );
    assert_eq!(world.hooks, [0x220600]);
    assert_eq!(
        engine.frame_level_hook(0, 0, &mut world),
        HookDispatch::Skip
    );
    assert_eq!(world.hooks.len(), 1);
    // Level 0x64 is runtime filled: the plan is returned, nothing runs.
    let context = ModeContext {
        level_id: 0x64,
        ..ModeContext::default()
    };
    let mut engine = Engine::new(&[], BuildOptions { context });
    let mut world = Probe::default();
    assert_eq!(
        engine.frame_level_hook(1, 0, &mut world),
        HookDispatch::AfterObjective
    );
    assert!(world.hooks.is_empty());
}

#[test]
fn special_level_rules() {
    assert_eq!(pool_size(0x65, 12), 0);
    assert_eq!(pool_size(0x66, 12), 12);
    assert!(pending_restore_enabled(0x64));
    assert!(!pending_restore_enabled(0x65));
    assert!(!pending_restore_enabled(0x66));
    let story = |level_id| ModeContext {
        level_id,
        ..ModeContext::default()
    };
    assert!(restore_gate_applies(&story(5)));
    assert!(!restore_gate_applies(&story(0x38)));
    assert!(!restore_gate_applies(&story(0x64)));
    let arcade = |level_id| ModeContext {
        mode_id: 3,
        level_id,
        ..ModeContext::default()
    };
    assert!(!restore_gate_applies(&arcade(5)));
    assert!(restore_gate_applies(&arcade(0x66)));
    let flagged = ModeContext {
        game_flags: 0x40,
        level_id: 5,
        ..ModeContext::default()
    };
    assert!(!restore_gate_applies(&flagged));
}

#[test]
fn level_0x65_builds_an_empty_pool() {
    let defs = [record(1, 0), record(2, 0)];
    let context = ModeContext {
        level_id: 0x65,
        ..ModeContext::default()
    };
    assert!(Engine::new(&defs, BuildOptions { context }).is_empty());
    let context = ModeContext {
        level_id: 5,
        ..ModeContext::default()
    };
    assert_eq!(Engine::new(&defs, BuildOptions { context }).len(), 2);
}

fn persistent_spawn() -> EventRecord {
    let mut spawn = record(7, 3);
    spawn.flags = 0x10;
    spawn.verb_args[(0x10 - 0x0C) / 4] = 0x123;
    spawn
}

#[test]
fn restore_gate_defers_a_flagged_type_and_ignores_it_when_exempt() {
    let defs = [persistent_spawn()];
    for (level, deferred) in [(5, true), (0x38, false), (0x64, false)] {
        let context = ModeContext {
            level_id: level,
            ..ModeContext::default()
        };
        let mut engine = Engine::new(&defs, BuildOptions { context });
        let mut world = Probe {
            class_flags: 0x200_0000,
            spawn_ready: true,
            ..Probe::default()
        };
        let snapshot = tsfp_gameplay::story_events::EventSnapshot {
            entries: vec![[0x0001_0000, 0, 1, 2]],
        };
        engine.restore(&snapshot, &mut world);
        // The gate only defers (0x237770 does not re-test it): a gated type runs when the queue is applied.
        let applied = engine.apply_pending_restore(&mut world);
        assert_eq!(applied, usize::from(deferred), "level {level}");
        assert!(
            engine.instance(0).running != 0 || engine.instance(0).done || !deferred,
            "level {level}"
        );
    }
}

#[test]
fn pending_restore_is_off_above_level_0x64() {
    let defs = [persistent_spawn()];
    let context = ModeContext {
        level_id: 0x66,
        game_flags: 0,
        mode_id: 10,
        ..ModeContext::default()
    };
    let mut engine = Engine::new(&defs, BuildOptions { context });
    let mut world = Probe {
        spawn_ready: false,
        ..Probe::default()
    };
    let snapshot = tsfp_gameplay::story_events::EventSnapshot {
        entries: vec![[0x0001_0000, 0, 1, 2]],
    };
    engine.restore(&snapshot, &mut world);
    world.spawn_ready = true;
    assert_eq!(engine.apply_pending_restore(&mut world), 0);
    let context = ModeContext {
        level_id: 5,
        ..ModeContext::default()
    };
    let mut engine = Engine::new(&defs, BuildOptions { context });
    let mut world = Probe {
        spawn_ready: false,
        ..Probe::default()
    };
    engine.restore(&snapshot, &mut world);
    world.spawn_ready = true;
    assert_eq!(engine.apply_pending_restore(&mut world), 1);
}

#[test]
fn shutter_opens_clamps_and_starts_slot_0x27() {
    let dt = frame_dt(1);
    let first = shutter_step(2, 0.0, dt);
    assert_eq!(first.state, 2);
    assert!((first.progress + dt * 2.94).abs() < 1e-6);
    assert!(
        first
            .effects
            .contains(&ShutterEffect::ShutterDriving { value: 3 })
    );
    let done = shutter_step(2, -0.82, 0.01);
    assert_eq!(done.state, 1);
    assert!((done.progress + 0.823).abs() < 1e-6);
    assert!(
        done.effects
            .contains(&ShutterEffect::OpenedStartSlot { slot: 0x27 })
    );
    let mut state = 2;
    let mut progress = 0.0;
    let mut frames = 0;
    while state == 2 {
        let step = shutter_step(state, progress, dt);
        state = step.state;
        progress = step.progress;
        frames += 1;
        assert!(frames < 100);
    }
    assert!((16..=18).contains(&frames), "{frames} frames at 60 Hz");
}

#[test]
fn shutter_closes_back_to_zero_and_finishes_slots() {
    let closing = shutter_step(3, -0.5, frame_dt(1));
    assert_eq!(closing.state, 3);
    assert!(closing.progress > -0.5);
    assert!(
        closing
            .effects
            .contains(&ShutterEffect::ShutterDriving { value: 2 })
    );
    let done = shutter_step(3, -0.01, 0.1);
    assert_eq!((done.state, done.progress), (0, 0.0));
    assert!(
        done.effects
            .contains(&ShutterEffect::ClosedFinishSlots { slots: [2, 0x27] })
    );
    let idle = shutter_step(0, -0.3, 0.1);
    assert_eq!((idle.state, idle.progress), (0, -0.3));
    assert!(idle.effects.is_empty());
}

#[test]
fn ramp_caps_and_leaves_a_unit_scale_alone() {
    let (blend, scale) = ramp_step(0.0, 1.0, frame_dt(1));
    assert!((blend - 1.570_796_4 / 60.0).abs() < 1e-6);
    assert_eq!(scale, 1.0);
    let (blend, scale) = ramp_step(1.57, 0.0, 1.0);
    assert!((blend - 1.570_796_4).abs() < 1e-7);
    assert!((scale - 0.01).abs() < 1e-7, "capped at 0.01, got {scale}");
    assert!(
        (ramp_step(0.0, -0.5, 1.0).1 + 0.48).abs() < 1e-6,
        "below the cap it just rises"
    );
    let (_, scale) = ramp_step(0.0, f32::NAN, 1.0);
    assert!(scale.is_nan() || scale <= 0.01);
}

#[test]
fn catalogue_is_unique_and_labelled() {
    let addresses: BTreeSet<u32> = ticks::CATALOGUE.iter().map(|f| f.address).collect();
    assert_eq!(addresses.len(), ticks::CATALOGUE.len());
    assert!(ticks::CATALOGUE.iter().any(|f| f.address == 0x1E0A70));
    assert!(ticks::CATALOGUE.iter().all(|f| !f.role.is_empty()));
}

/// Census over the real SETU data: the callbacks of the batch 4 items are all verb 0 definitions (so their
/// argument is the 0x48 byte instance), one use each except 63 / 66 / 80.
#[test]
fn real_open_item_callbacks_are_verb_0_uses() {
    let Some(dir) = std::env::var_os("TSFP_SETU_PAK_DIR").map(PathBuf::from) else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let items = [
        24u32, 38, 41, 51, 52, 53, 54, 55, 56, 57, 63, 64, 65, 66, 67, 68, 69, 70, 72, 77, 80,
    ];
    let mut seen: std::collections::BTreeMap<u32, (usize, BTreeSet<u32>)> = Default::default();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "pak") {
            continue;
        }
        for blob in setu_blobs_from_pak(&std::fs::read(&path).unwrap()).unwrap_or_default() {
            for event in parse_events(&blob).unwrap() {
                if items.contains(&event.callback) {
                    let slot = seen.entry(event.callback).or_default();
                    slot.0 += 1;
                    slot.1.insert(event.verb);
                }
            }
        }
    }
    assert_eq!(
        seen.len(),
        items.len(),
        "every open-item callback is on the disc: {seen:?}"
    );
    for (id, (count, verbs)) in &seen {
        assert_eq!(
            verbs.iter().copied().collect::<Vec<_>>(),
            [0],
            "callback {id} runs only from verb 0"
        );
        assert!(*count >= 1);
    }
    assert_eq!(seen[&63].0, 4);
    assert_eq!(seen[&66].0, 5);
    assert_eq!(seen[&80].0, 5);
}

/// T1845: `Engine::run_arcade_objective` answers `0x2273C0` from the engine's slot manager and finishes a slot on
/// the objective table's bit 15 event (FABRICATED scenario, rule MEASURED in docs/t-story-event-arcade-runtime.md).
#[test]
fn arcade_objective_progresses_engine_slots() {
    use tsfp_gameplay::story_events::arcade::{
        ActorState, ArcadeWorld, EntryDescriptor, MemberBits, ObjectiveEvent, ObjectiveTable,
    };
    struct Arcade;
    impl ArcadeWorld for Arcade {
        fn slot_finished(&self, _slot: u32) -> bool {
            panic!("the engine bridge answers this")
        }
        fn random_u32(&mut self) -> u32 {
            0
        }
        fn actor(&self, _slot: u32, _index: usize) -> Option<ActorState> {
            None
        }
        fn member_complete(&mut self, _slot: u32, _member: usize) -> bool {
            false
        }
        fn member_bits(&mut self, _slot: u32, _member: usize, _kind: u32) -> MemberBits {
            MemberBits { bits: 0, width: 0 }
        }
    }
    let context = ModeContext {
        mode_id: 3,
        level_id: 1,
        ..ModeContext::default()
    };
    let mut engine = Engine::new(&[], BuildOptions { context });
    assert!(
        engine
            .slots_mut()
            .create(&context, 2, 0, [0, 0, 0], u32::MAX)
    );
    let mut table = ObjectiveTable::build(
        1,
        &[EntryDescriptor {
            slot: 2,
            kind: 1,
            members: vec![],
        }],
    );
    let mut world = Probe::default();
    let quiet = engine.run_arcade_objective(&mut table, &mut Arcade, &mut world);
    assert!(quiet.is_empty() && !engine.slots().is_finished(2));
    engine.finish_slot(2, &mut world);
    assert!(engine.slots().is_finished(2));
    let events = engine.run_arcade_objective(&mut table, &mut Arcade, &mut world);
    assert!(
        events.contains(&ObjectiveEvent::SlotFinished { slot: 2 }),
        "{events:?}"
    );
}
