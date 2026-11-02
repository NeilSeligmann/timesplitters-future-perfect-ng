//! T1842 tests: static slot progress tracking (`0x2282B0`, actor registry `0x227170` / `0x2271F0`),
//! the callback bodies of tables `0x4D2B10` and `0x4D2CA0`, and a real-data replay. Synthetic scenarios
//! are FABRICATED inputs checked against rules MEASURED in `build/default.xbe` (see
//! docs/t-story-event-engine.md). Real-data tests skip without TSFP_SETU_PAK_DIR; no game data is
//! committed.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use tsfp_gameplay::story_events::actions::{Action, ActionResult};
use tsfp_gameplay::story_events::callbacks::{
    self, CallbackContext, DISC_IDS, SLOT_DISC_IDS, SlotOp, Step, WorldOp, tag,
};
use tsfp_gameplay::story_events::native_world::NativeStoryWorld;
use tsfp_gameplay::story_events::record::{Condition, EventRecord, Link};
use tsfp_gameplay::story_events::setu::{parse_events, setu_blobs_from_pak};
use tsfp_gameplay::story_events::slots::{
    self, ObjectiveDefinitionKey, STATUS_ACTIVE, STATUS_FINISHED, STATUS_PENDING, SlotCallback,
    SlotEvent, SlotManager, TRACKER_ACTORS, TRACKER_CLASSES, Tracker,
};
use tsfp_gameplay::story_events::world::{
    CallbackTarget, ConditionQuery, IdVariant, PropRequest, SpawnRequest, VoiceRequest, World,
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

fn set_arg(record: &mut EventRecord, offset: usize, value: u32) {
    record.verb_args[(offset - 0x0C) / 4] = value;
}

fn context(record: EventRecord) -> CallbackContext {
    CallbackContext {
        record,
        executed: true,
        words: [0; 2],
        class_254_actor: 0,
    }
}

/// Probe world: scripted health and character ids, records slot events and callback steps.
#[derive(Default)]
struct Probe {
    log: Vec<String>,
    health: HashMap<u32, f32>,
    player_health: f32,
    characters: HashSet<u32>,
    events: Vec<SlotEvent>,
    ops: Vec<WorldOp>,
    slot_ops: Vec<(u32, WorldOp)>,
    models_callbacks: bool,
    saved: bool,
    polled: Vec<(u32, u32)>,
}

impl World for Probe {
    fn play_voice(&mut self, _r: &VoiceRequest) -> Option<u32> {
        Some(1)
    }
    fn voice_finished(&mut self, _h: u32) -> bool {
        true
    }
    fn run_callback(&mut self, callback: u32, target: CallbackTarget) {
        self.log.push(format!("callback {callback} {target:?}"));
    }
    fn apply_callback(&mut self, op: &WorldOp, _target: CallbackTarget) -> bool {
        self.ops.push(op.clone());
        self.models_callbacks
    }
    fn apply_slot_op(&mut self, slot: u32, op: &WorldOp) -> bool {
        self.slot_ops.push((slot, op.clone()));
        true
    }
    fn shtm_saved(&self) -> bool {
        self.saved
    }
    fn set_objective(&mut self, _a: u32, _i: u32) {}
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
        2
    }
    fn player_matches(&self, _p: usize, _v: IdVariant, ids: &[u32]) -> bool {
        ids.iter().any(|id| self.characters.contains(id))
    }
    fn first_player_flag(&self) -> bool {
        false
    }
    fn tag_exists(&self, _k: u32) -> bool {
        true
    }
    fn condition_query(&mut self, _q: &ConditionQuery) -> Option<bool> {
        Some(false)
    }
    fn poll_link(&mut self, _l: &Link) -> bool {
        false
    }
    fn apply_action(&mut self, action: &Action) -> ActionResult {
        match action {
            Action::ObjectAnimStart { .. } => ActionResult::Handle(1),
            _ => ActionResult::Done,
        }
    }
    fn slot_event(&mut self, event: &SlotEvent) {
        self.events.push(*event);
    }
    fn slot_poll(&mut self, slot: u32, callback: u32) {
        self.polled.push((slot, callback));
    }
    fn actor_health(&self, actor: u32) -> Option<f32> {
        Some(self.health.get(&actor).copied().unwrap_or(1.0))
    }
    fn player_health(&self, _player: usize) -> Option<f32> {
        Some(self.player_health)
    }
}

fn started(records: &[EventRecord], context: ModeContext) -> Engine {
    let mut engine = Engine::new(records, BuildOptions { context });
    engine.set_checkpoint_mask(1);
    engine
}

fn probe() -> Probe {
    Probe {
        player_health: 100.0,
        ..Probe::default()
    }
}

// ------------------------------------------------------------------ actor registry (0x227170 / 0x2271F0)

#[test]
fn registry_groups_actors_by_class_and_shifts_later_classes() {
    // FABRICATED inputs, ordering rule MEASURED in 0x227170: insert at the end of the class group.
    let mut tracker = Tracker::default();
    tracker.add_class(0x10);
    tracker.add_class(0x20);
    let mut slots = SlotManager::new();
    let ctx = ModeContext::default();
    assert!(slots.create(&ctx, 0, 0, [0, 0, 0], 0x700));
    slots.add_tracked_class(0x10);
    slots.add_tracked_class(0x20);
    slots.add_tracked_class(0x10); // duplicate ignored
    for (actor, class) in [(1, 0x20), (2, 0x10), (3, 0x20), (4, 0x10)] {
        slots.register_actor(&ctx, actor, class);
    }
    let tracker_state = slots.get(0).unwrap().tracker.clone().unwrap();
    assert_eq!(tracker_state.keys, vec![0x10, 0x20]);
    assert_eq!(&tracker_state.actors[..4], &[2, 4, 1, 3]);
    assert_eq!(&tracker_state.counts[..2], &[2, 2]);
    assert_eq!(slots.tracked_actor(0, 0x10, 1), 4);
    assert_eq!(slots.tracked_actor(0, 0x20, 0), 1);
    assert_eq!(slots.tracked_actor(0, 0x99, 0), 0, "unknown class");
    assert_eq!(slots.tracked_actor(0, 0x20, 5), 0, "past the end");
    assert_eq!(tracker.actor_count(), 0);
}

#[test]
fn registry_rejects_unknown_class_and_full_registry_and_wrong_mode() {
    let ctx = ModeContext::default();
    let mut slots = SlotManager::new();
    slots.create(&ctx, 0, 0, [0, 0, 0], 0x700);
    slots.add_tracked_class(7);
    slots.register_actor(&ctx, 1, 8);
    assert_eq!(
        slots
            .get(0)
            .unwrap()
            .tracker
            .as_ref()
            .unwrap()
            .actor_count(),
        0,
        "class 8 was never added"
    );
    for actor in 1..=12 {
        slots.register_actor(&ctx, actor, 7);
    }
    assert_eq!(
        slots
            .get(0)
            .unwrap()
            .tracker
            .as_ref()
            .unwrap()
            .actor_count(),
        TRACKER_ACTORS
    );
    // Modes other than 3 and 10 do not register at all.
    let other = ModeContext {
        mode_id: 5,
        ..ModeContext::default()
    };
    let mut slots = SlotManager::new();
    slots.create(&ctx, 0, 0, [0, 0, 0], 0x700);
    slots.add_tracked_class(7);
    slots.register_actor(&other, 1, 7);
    assert_eq!(
        slots
            .get(0)
            .unwrap()
            .tracker
            .as_ref()
            .unwrap()
            .actor_count(),
        0
    );
}

#[test]
fn class_list_holds_ten_keys_at_most() {
    let ctx = ModeContext::default();
    let mut slots = SlotManager::new();
    slots.create(&ctx, 0, 0, [0, 0, 0], 0x700);
    for key in 0..20 {
        slots.add_tracked_class(key);
    }
    assert_eq!(
        slots.get(0).unwrap().tracker.as_ref().unwrap().keys.len(),
        TRACKER_CLASSES
    );
}

#[test]
fn unregister_packs_the_registry_and_forces_a_recount() {
    let ctx = ModeContext::default();
    let mut slots = SlotManager::new();
    slots.create(&ctx, 0, 0, [0, 0, 0], 0x700);
    slots.add_tracked_class(1);
    slots.add_tracked_class(2);
    slots.enable_tracking();
    for (actor, class) in [(10, 1), (11, 2), (12, 1)] {
        slots.register_actor(&ctx, actor, class);
    }
    assert_eq!(slots.get(0).unwrap().total, 3);
    slots.unregister_actor(&ctx, 10);
    let slot = slots.get(0).unwrap();
    let tracker = slot.tracker.as_ref().unwrap();
    assert_eq!(&tracker.actors[..3], &[12, 11, 0]);
    assert_eq!(&tracker.counts[..2], &[1, 1]);
    assert_eq!((slot.total, slot.last_count), (2, u32::MAX));
}

#[test]
fn only_slots_below_0x2d_have_a_registry() {
    let ctx = ModeContext::default();
    let mut slots = SlotManager::new();
    slots.create(&ctx, 0x2D, 0, [2, 2, 2], 0x700);
    slots.create(&ctx, 3, 0, [0, 0, 0], 0x700);
    assert!(slots.get(0x2D).unwrap().tracker.is_none());
    assert!(slots.get(3).unwrap().tracker.is_some());
    assert_eq!(slots.current(), Some(3));
}

// ------------------------------------------------------------------ progress pass (0x2282B0)

fn engine_with_tracked_slot(mode_id: u8, actors: &[u32]) -> Engine {
    let context = ModeContext {
        mode_id,
        ..ModeContext::default()
    };
    let mut engine = Engine::new(&[], BuildOptions { context });
    {
        let slots = engine.slots_mut();
        slots.create(
            &context,
            0,
            0x1234,
            [0, 0, 0],
            if mode_id == 3 { 1 } else { 0x700 },
        );
        slots.add_tracked_class(0x77);
        slots.enable_tracking();
        let slot = slots.get_mut(0).unwrap();
        slot.flags = 1;
        slot.progress_one = 0xA1;
        slot.progress_many = 0xA2;
    }
    for &actor in actors {
        engine.register_actor(actor, 0x77);
    }
    engine
}

#[test]
fn killing_every_tracked_actor_finishes_the_slot() {
    let mut engine = engine_with_tracked_slot(10, &[1, 2, 3]);
    let mut world = probe();
    engine.step(0.1, &mut world);
    assert_eq!(engine.slots().status(0), STATUS_ACTIVE, "nobody died yet");
    assert!(world.events.is_empty());
    world.health.insert(1, 0.0);
    engine.step(0.1, &mut world);
    // Two left: the plural label reaches both local players, then the jingle.
    let progress: Vec<_> = world
        .events
        .iter()
        .filter(|e| matches!(e, SlotEvent::Progress { .. }))
        .collect();
    assert_eq!(progress.len(), 2);
    assert!(matches!(
        progress[0],
        SlotEvent::Progress {
            slot: 0,
            player: 0,
            remaining: 2,
            label: 0xA2
        }
    ));
    assert!(world.events.contains(&SlotEvent::ProgressSound { slot: 0 }));
    world.events.clear();
    // Same count again: nothing is posted.
    engine.step(0.1, &mut world);
    assert!(world.events.is_empty());
    world.health.insert(2, -5.0);
    engine.step(0.1, &mut world);
    assert!(
        matches!(
            world.events[0],
            SlotEvent::Progress {
                remaining: 1,
                label: 0xA1,
                ..
            }
        ),
        "singular label"
    );
    world.health.insert(3, 0.0);
    engine.step(0.1, &mut world);
    assert_eq!(engine.slots().status(0), STATUS_FINISHED);
    assert!(world.events.iter().any(|e| matches!(
        e,
        SlotEvent::Changed {
            slot: 0,
            status: 5,
            ..
        }
    )));
    assert!(
        world.events.contains(&SlotEvent::ProgressSound { slot: 0 }),
        "the jingle also plays on completion"
    );
}

#[test]
fn missing_native_actor_health_does_not_advance_slot_progress() {
    let mut engine = engine_with_tracked_slot(10, &[1]);
    let mut world = NativeStoryWorld::default();
    // Model the recount sentinel set by actor unregister; an unknown health query must preserve it.
    engine.slots_mut().get_mut(0).unwrap().last_count = u32::MAX;

    engine.step(0.1, &mut world);
    assert_eq!(engine.slots().status(0), STATUS_ACTIVE);
    assert_eq!(engine.slots().get(0).unwrap().last_count, u32::MAX);

    world.actor_health.insert(1, 0.0);
    engine.step(0.1, &mut world);
    assert_eq!(engine.slots().status(0), STATUS_FINISHED);
}

#[test]
fn nan_health_does_not_count_as_dead() {
    // comiss with an unordered operand takes the "not dead" branch (MEASURED, jb after comiss).
    let mut engine = engine_with_tracked_slot(10, &[1]);
    let mut world = probe();
    engine.slots_mut().get_mut(0).unwrap().last_count = u32::MAX;
    world.health.insert(1, f32::NAN);
    engine.step(0.1, &mut world);
    assert_eq!(engine.slots().status(0), STATUS_ACTIVE);
    assert_eq!(engine.slots().get(0).unwrap().last_count, 0);
}

#[test]
fn mode_3_skips_the_progress_body() {
    // MEASURED: `cmp cl, 3 / je` at the top of 0x2282B0. The only writer of the tracking selector
    // sits in the mode 3 group setup, so the kill count is unreachable in the retail image.
    let mut engine = engine_with_tracked_slot(3, &[1]);
    let mut world = probe();
    world.health.insert(1, 0.0);
    for _ in 0..3 {
        engine.step(0.1, &mut world);
    }
    assert_eq!(engine.slots().status(0), STATUS_ACTIVE);
    assert!(world.events.is_empty());
}

#[test]
fn selector_off_counts_nothing() {
    let context = ModeContext::default();
    let mut engine = Engine::new(&[], BuildOptions { context });
    engine.slots_mut().create(&context, 0, 0, [0, 0, 0], 0x700);
    engine.slots_mut().add_tracked_class(0x77);
    engine.register_actor(1, 0x77);
    let mut world = probe();
    world.health.insert(1, 0.0);
    engine.step(0.1, &mut world);
    assert_eq!(
        engine.slots().get(0).unwrap().total,
        0,
        "registration does not count without a selector"
    );
    assert_eq!(engine.slots().status(0), STATUS_ACTIVE);
}

#[test]
fn unflagged_slot_finishes_without_messages() {
    let mut engine = engine_with_tracked_slot(10, &[1]);
    engine.slots_mut().get_mut(0).unwrap().flags = 0;
    let mut world = probe();
    world.health.insert(1, 0.0);
    engine.step(0.1, &mut world);
    assert_eq!(engine.slots().status(0), STATUS_FINISHED);
    assert!(!world.events.iter().any(|e| matches!(
        e,
        SlotEvent::Progress { .. } | SlotEvent::ProgressSound { .. }
    )));
}

#[test]
fn a_finished_slot_stops_counting() {
    let mut engine = engine_with_tracked_slot(10, &[1, 2]);
    let mut world = probe();
    world.health.insert(1, 0.0);
    world.health.insert(2, 0.0);
    engine.step(0.1, &mut world);
    assert_eq!(engine.slots().status(0), STATUS_FINISHED);
    world.events.clear();
    engine.unregister_actor(1);
    engine.step(0.1, &mut world);
    assert!(
        world.events.is_empty(),
        "status 5 is skipped by the counting branch"
    );
}

#[test]
fn a_living_player_with_a_dependency_character_finishes_the_slot() {
    let context = ModeContext::default();
    let defs = vec![slots::SlotDef {
        id: 2,
        key: ObjectiveDefinitionKey(0),
        mask: 0x700,
        label: 5446,
        kinds: [0, 0, 0],
        callback_id: 0,
        dependencies: [0x555, NONE, NONE, NONE, NONE],
        initially_active: true,
    }];
    let mut engine = Engine::new(&[], BuildOptions { context });
    engine.load_static_slots(&defs);
    let mut world = probe();
    engine.step(0.1, &mut world);
    assert_eq!(
        engine.slots().status(2),
        STATUS_ACTIVE,
        "no player has the character"
    );
    world.characters.insert(0x555);
    world.player_health = 0.0;
    engine.step(0.1, &mut world);
    assert_eq!(
        engine.slots().status(2),
        STATUS_ACTIVE,
        "a dead player does not count"
    );
    world.player_health = 50.0;
    engine.step(0.1, &mut world);
    assert_eq!(engine.slots().status(2), STATUS_FINISHED);
}

#[test]
fn static_slot_callbacks_are_polled_every_frame_with_the_slot() {
    let context = ModeContext::default();
    let defs = vec![slots::SlotDef {
        id: 1,
        key: ObjectiveDefinitionKey(0),
        mask: 0x700,
        label: 0,
        kinds: [0, 0, 0],
        callback_id: 5,
        dependencies: [NONE; 5],
        initially_active: true,
    }];
    let mut engine = Engine::new(&[], BuildOptions { context });
    engine.load_static_slots(&defs);
    assert_eq!(
        engine.slots().get(1).unwrap().callback,
        SlotCallback::World(5)
    );
    let mut world = probe();
    engine.step(0.1, &mut world);
    engine.step(0.1, &mut world);
    assert_eq!(world.polled, vec![(1, 5), (1, 5)]);
    assert_eq!(
        engine.slots().current(),
        None,
        "the poll loop leaves the current slot at -1"
    );
}

// ------------------------------------------------------------------ slot callbacks (table 0x4D2CA0)

#[test]
fn slot_callbacks_5_and_6_act_on_save_and_restore_only() {
    for id in [1, 2, 3, 4, 7, 8, 9] {
        for code in 0..5 {
            assert_eq!(
                callbacks::decode_slot_callback(id, code),
                Ok(vec![]),
                "id {id} is the shared no-op"
            );
        }
    }
    assert_eq!(
        callbacks::decode_slot_callback(5, 2),
        Ok(vec![WorldOp::ShtmSave])
    );
    assert_eq!(
        callbacks::decode_slot_callback(5, 1),
        Ok(vec![WorldOp::ShtmRestore])
    );
    assert_eq!(callbacks::decode_slot_callback(5, 3), Ok(vec![]));
    assert_eq!(
        callbacks::decode_slot_callback(6, 1),
        Ok(vec![WorldOp::ZctpHealth])
    );
    assert_eq!(
        callbacks::decode_slot_callback(6, 2),
        Ok(vec![WorldOp::ZctpHealth])
    );
    assert_eq!(callbacks::decode_slot_callback(6, 4), Ok(vec![]));
    assert!(callbacks::decode_slot_callback(11, 1).is_err());
    for id in SLOT_DISC_IDS {
        assert!(callbacks::decode_slot_callback(id, 1).is_ok());
    }
}

#[test]
fn restore_of_slot_callback_5_fires_etrc_first_when_state_was_saved() {
    let context = ModeContext::default();
    let mut etrc = record(tag(b"ETRC"), 13);
    etrc.link_a.mode = 5; // keyed trigger, never fires by itself
    etrc.link_a.id = 1;
    let defs = vec![slots::SlotDef {
        id: 0,
        key: ObjectiveDefinitionKey(0),
        mask: 0x700,
        label: 0,
        kinds: [0, 0, 0],
        callback_id: 5,
        dependencies: [NONE; 5],
        initially_active: true,
    }];
    let mut engine = Engine::new(&[etrc], BuildOptions { context });
    engine.load_static_slots(&defs);
    let mut world = probe();
    engine.run_slot_callbacks(2, &mut world);
    assert_eq!(world.slot_ops, vec![(0, WorldOp::ShtmSave)]);
    world.slot_ops.clear();
    engine.run_slot_callbacks(1, &mut world);
    assert_eq!(world.slot_ops, vec![(0, WorldOp::ShtmRestore)]);
    assert!(
        engine.take_side_report().fired.is_empty(),
        "nothing was saved: no ETRC"
    );
    world.saved = true;
    engine.run_slot_callbacks(1, &mut world);
    let report = engine.take_side_report();
    assert_eq!(
        report.fired.len(),
        1,
        "ETRC runs through the id triggered entry 0x2378D0"
    );
    assert_eq!(report.fired[0].event_id, tag(b"ETRC"));
}

// ------------------------------------------------------------------ callback bodies

#[test]
fn every_disc_callback_id_decodes_and_unused_ids_stay_explicit_gaps() {
    let mut seen = HashSet::new();
    for id in DISC_IDS {
        assert!(seen.insert(id), "duplicate {id}");
        let ctx = context(record(0x1234, 0));
        assert!(callbacks::decode(id, &ctx).is_ok(), "id {id}");
        assert!(callbacks::is_decoded(id));
    }
    assert_eq!(seen.len(), 83);
    // Ids of the table that the disc never uses are reported with a reason, never guessed.
    for id in [
        8, 11, 14, 15, 17, 30, 34, 37, 43, 44, 46, 48, 71, 74, 89, 94, 100, 150,
    ] {
        let gap = callbacks::decode(id, &context(record(1, 0))).expect_err("not on disc");
        assert_eq!(gap.id, id);
        assert!(gap.reason.contains("no definition"), "{}", gap.reason);
    }
}

#[test]
fn callback_28_32_33_read_the_character_key_and_flags() {
    let mut r = record(1, 0);
    set_arg(&mut r, 0xC, 0x4242);
    let steps = callbacks::decode(28, &context(r)).unwrap();
    assert_eq!(
        steps,
        vec![Step::World(WorldOp::NpcGoal { character: 0x4242 })]
    );
    let steps = callbacks::decode(32, &context(r)).unwrap();
    assert_eq!(
        steps,
        vec![Step::World(WorldOp::ResetInputDriven {
            player: 0,
            character: 0x4242,
            mode: 1
        })]
    );
    r.flags = 0x80_0000;
    let steps = callbacks::decode(33, &context(r)).unwrap();
    assert_eq!(
        steps,
        vec![Step::World(WorldOp::ResetInputDriven {
            player: 1,
            character: 0x4242,
            mode: 5
        })]
    );
}

#[test]
fn callbacks_75_76_10_60_use_the_tag_and_event_key() {
    let mut r = record(0xABCD, 0);
    set_arg(&mut r, 0x14, 0x9999);
    let state = |value| {
        vec![Step::World(WorldOp::SetObjectState {
            tag: 0x9999,
            word: callbacks::StateWord::Script,
            value,
        })]
    };
    assert_eq!(callbacks::decode(75, &context(r)).unwrap(), state(1));
    assert_eq!(callbacks::decode(76, &context(r)).unwrap(), state(0));
    assert_eq!(
        callbacks::decode(10, &context(r)).unwrap(),
        vec![Step::World(WorldOp::DamageObject {
            tag: 0xABCD,
            amount: 1000.0,
            kind: 4,
            direction: [0.0, 1.0, 0.0]
        })]
    );
    assert_eq!(
        callbacks::decode(60, &context(r)).unwrap(),
        vec![Step::World(WorldOp::GroupTrigger { tag: 0xABCD })]
    );
}

#[test]
fn slot_touching_callbacks_return_slot_steps() {
    let r = record(1, 0);
    assert_eq!(
        callbacks::decode(27, &context(r)).unwrap(),
        vec![Step::Slot(SlotOp::Finish(10))]
    );
    let mut idle = context(r);
    idle.executed = false;
    assert!(
        callbacks::decode(27, &idle).unwrap().is_empty(),
        "0x234270 gates id 27"
    );
    assert!(
        callbacks::decode(18, &idle).unwrap().is_empty(),
        "and the ARTR / LPTR family"
    );
    assert!(callbacks::decode(21, &idle).unwrap().is_empty());
    assert!(callbacks::decode(23, &idle).unwrap().iter().all(|_| false));
    assert!(matches!(
        callbacks::decode(54, &context(r)).unwrap()[1],
        Step::Slot(SlotOp::Finish(1))
    ));
    assert!(matches!(
        callbacks::decode(56, &context(r)).unwrap()[1],
        Step::Slot(SlotOp::Finish(3))
    ));
    assert!(matches!(
        callbacks::decode(68, &context(r)).unwrap()[1],
        Step::Slot(SlotOp::Finish(0xB))
    ));
    assert!(matches!(
        callbacks::decode(69, &context(r)).unwrap()[1],
        Step::Slot(SlotOp::Finish(0xA))
    ));
    let mut timed = record(1, 0);
    set_arg(&mut timed, 0xC, 7);
    set_arg(&mut timed, 0x10, 2.5f32.to_bits());
    assert_eq!(
        callbacks::decode(98, &context(timed)).unwrap(),
        vec![Step::Slot(SlotOp::StartTimed {
            slot: 7,
            value: 2.5
        })]
    );
    assert_eq!(
        callbacks::decode(99, &context(timed)).unwrap(),
        vec![Step::Slot(SlotOp::FinishTimed)]
    );
}

#[test]
fn interaction_callbacks_take_their_key_words_from_the_argument() {
    let mut ctx = context(record(1, 0));
    ctx.words = [0xAA, 0xBB];
    assert_eq!(
        callbacks::decode(57, &ctx).unwrap(),
        vec![Step::World(WorldOp::LinkEntry {
            key: 0xAA,
            object_word: 0xBB
        })]
    );
    assert_eq!(
        callbacks::decode(64, &ctx).unwrap(),
        vec![Step::World(WorldOp::LinkEntry {
            key: tag(b"PER1"),
            object_word: 0xBB
        })]
    );
    let mut link = record(1, 0);
    link.link_a.id = 0x55;
    assert_eq!(
        callbacks::decode(63, &context(link)).unwrap(),
        vec![Step::World(WorldOp::EntryLookup {
            key: 0x55,
            link: false,
            object_word: 0
        })],
        "63 reads R+0x44, the other callbacks read the argument"
    );
}

#[test]
fn engine_runs_slot_steps_and_falls_back_to_the_legacy_hook() {
    let context_mode = ModeContext::default();
    let mut callback_event = record(1, 0);
    callback_event.callback = 54;
    let mut engine = started(&[callback_event], context_mode);
    engine
        .slots_mut()
        .create(&context_mode, 1, 0x10, [0, 0, 0], 0x700);
    let mut world = probe();
    engine.step(0.1, &mut world);
    assert_eq!(
        engine.slots().status(1),
        STATUS_FINISHED,
        "callback 54 finishes slot 1"
    );
    assert_eq!(world.ops, vec![WorldOp::EgswStart]);
    assert_eq!(
        world.log,
        vec!["callback 54 Instance(0)"],
        "the world did not model the step: legacy hook runs once"
    );

    // A world that models the step is not called through the legacy hook.
    let mut engine = started(&[callback_event], context_mode);
    engine
        .slots_mut()
        .create(&context_mode, 1, 0x10, [0, 0, 0], 0x700);
    let mut world = probe();
    world.models_callbacks = true;
    engine.step(0.1, &mut world);
    assert!(world.log.is_empty());
    assert_eq!(engine.slots().status(1), STATUS_FINISHED);
}

#[test]
fn undecodable_callback_is_recorded_and_still_reaches_the_legacy_hook() {
    let mut event = record(1, 0);
    event.callback = 8; // in the table, on no definition, body not decoded
    let mut engine = started(&[event], ModeContext::default());
    let mut world = probe();
    let report = engine.step(0.1, &mut world);
    assert!(report.errors.is_empty());
    assert_eq!(world.log, vec!["callback 8 Instance(0)"]);
    assert_eq!(engine.callback_gaps().len(), 1);
    assert_eq!(engine.callback_gaps()[0].id, 8);
}

#[test]
fn timed_slot_callbacks_98_and_99_pair_up() {
    let context_mode = ModeContext::default();
    let mut start = record(1, 0);
    start.callback = 98;
    set_arg(&mut start, 0xC, 4);
    set_arg(&mut start, 0x10, 3f32.to_bits());
    let mut stop = record(2, 0);
    stop.callback = 99;
    stop.link_a = Link {
        mode: 2,
        id: NONE,
        args: [NONE; 4],
    };
    let defs = vec![slots::SlotDef {
        id: 4,
        key: ObjectiveDefinitionKey(0),
        mask: 0x700,
        label: 0,
        kinds: [0, 0, 0],
        callback_id: 0,
        dependencies: [NONE; 5],
        initially_active: false,
    }];
    // Start alone: the slot is started and the callback remembers slot, stamp and value.
    let mut engine = started(&[start], context_mode);
    engine.load_static_slots(&defs);
    assert_eq!(engine.slots().status(4), STATUS_PENDING);
    let mut world = probe();
    engine.step(0.1, &mut world);
    assert_eq!(
        engine.timed_slot().map(|(slot, _, value)| (slot, value)),
        Some((4, 3.0))
    );
    assert_eq!(
        engine.slots().status(4),
        STATUS_ACTIVE,
        "98 starts the pending slot (0x227650)"
    );
    // Both: the stop definition runs in the same pass (link mode 2) and finishes the slot.
    let mut engine = started(&[start, stop], context_mode);
    engine.load_static_slots(&defs);
    engine.step(0.1, &mut world);
    engine.step(0.1, &mut world);
    assert_eq!(
        engine.slots().status(4),
        STATUS_FINISHED,
        "99 finishes the remembered slot"
    );
    assert!(engine.timed_slot().is_none());
}

#[test]
fn callback_5_uses_slot_zero_class_254_actor() {
    let context_mode = ModeContext::default();
    let mut event = record(1, 0);
    event.callback = 5;
    let mut engine = started(&[event], context_mode);
    engine
        .slots_mut()
        .create(&context_mode, 0, 0, [0, 0, 0], 0x700);
    engine.slots_mut().add_tracked_class(0x254);
    engine.register_actor(0xAC7, 0x254);
    let mut world = probe();
    engine.step(0.1, &mut world);
    assert_eq!(world.ops, vec![WorldOp::RoomParticles { actor: 0xAC7 }]);
    // 0x227740(0) makes slot 0 current before the lookup; the slot poll loop of the same frame resets it to -1.
    let mut ctx = context(event);
    ctx.class_254_actor = 0xAC7;
    assert_eq!(
        callbacks::decode(5, &ctx).unwrap(),
        vec![
            Step::Slot(SlotOp::SetCurrent(0)),
            Step::World(WorldOp::RoomParticles { actor: 0xAC7 })
        ]
    );
}

#[test]
fn tick_callbacks_run_on_the_spawned_object() {
    // Verb 4 (object animation start): the tail pass ticks every verb except 3 on the object handle.
    let mut spawn = record(1, 4);
    spawn.tick_callback = 1;
    set_arg(&mut spawn, 0x14, 0x55);
    let mut engine = started(&[spawn], ModeContext::default());
    let mut world = probe();
    for _ in 0..3 {
        engine.step(0.1, &mut world);
    }
    assert!(world.ops.iter().all(|op| *op == WorldOp::AmbientSoundTick));
    assert!(
        !world.ops.is_empty(),
        "the tail pass ticks the object while the verb lives"
    );
}

// ------------------------------------------------------------------ real data (optional)

fn real_blobs() -> Option<Vec<(String, Vec<u8>)>> {
    let dir = PathBuf::from(std::env::var_os("TSFP_SETU_PAK_DIR")?);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).ok()? {
        let path = entry.ok()?.path();
        if path.extension().is_some_and(|ext| ext == "pak") {
            let name = path.file_name()?.to_string_lossy().into_owned();
            for blob in setu_blobs_from_pak(&std::fs::read(&path).ok()?).ok()? {
                out.push((name.clone(), blob));
            }
        }
    }
    out.sort();
    (!out.is_empty()).then_some(out)
}

fn context_for(name: &str, players: u32) -> ModeContext {
    if name.contains("__arcade__") {
        ModeContext {
            mode_id: 3,
            ..ModeContext::default()
        }
    } else if name.contains("__challenge__") {
        ModeContext {
            game_flags: 0x800_0000,
            descriptor_word: 0xE00_0000,
            ..ModeContext::default()
        }
    } else {
        ModeContext {
            players,
            ..ModeContext::default()
        }
    }
}

#[test]
fn real_callback_census_matches_the_decoder() {
    let Some(blobs) = real_blobs() else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let mut uses: BTreeMap<u32, usize> = BTreeMap::new();
    let mut slot_uses: BTreeMap<u32, usize> = BTreeMap::new();
    for (name, blob) in &blobs {
        for event in parse_events(blob).unwrap_or_else(|e| panic!("{name}: {e}")) {
            for id in [event.callback, event.tick_callback] {
                if id != 0 {
                    *uses.entry(id).or_default() += 1;
                }
            }
        }
        for def in slots::parse_slot_defs(blob).unwrap() {
            *slot_uses.entry(def.callback_id).or_default() += 1;
        }
    }
    let disc: HashSet<u32> = DISC_IDS.into_iter().collect();
    let found: HashSet<u32> = uses.keys().copied().collect();
    let undecoded: Vec<_> = found.difference(&disc).collect();
    assert!(
        undecoded.is_empty(),
        "ids on the disc that the decoder does not list: {undecoded:?}"
    );
    let unused: Vec<_> = disc.difference(&found).collect();
    assert!(
        unused.is_empty(),
        "decoder ids that are not on the disc: {unused:?}"
    );
    // MEASURED counts of the six most used ids (T1838 census, recounted).
    for (id, count) in [(28, 104), (32, 40), (33, 39), (75, 20), (10, 14), (60, 10)] {
        assert!(uses[&id] >= count, "id {id}: {} uses", uses[&id]);
    }
    for (&id, _) in &slot_uses {
        if id != 0 {
            assert!(SLOT_DISC_IDS.contains(&id), "slot callback {id}");
        }
    }
    eprintln!(
        "callback census: {} distinct ids ({} uses), slot callbacks {:?}",
        found.len(),
        uses.values().sum::<usize>(),
        slot_uses
    );
}

/// Replay world: models every callback step (so no legacy fallback), answers every query true and
/// records which callbacks and slot steps ran.
#[derive(Default)]
struct Everything {
    handle: u32,
    callbacks: BTreeMap<String, usize>,
}

impl World for Everything {
    fn play_voice(&mut self, _r: &VoiceRequest) -> Option<u32> {
        self.handle += 1;
        Some(self.handle)
    }
    fn voice_finished(&mut self, _h: u32) -> bool {
        true
    }
    fn run_callback(&mut self, callback: u32, _t: CallbackTarget) {
        *self
            .callbacks
            .entry(format!("legacy {callback}"))
            .or_default() += 1;
    }
    fn apply_callback(&mut self, op: &WorldOp, _t: CallbackTarget) -> bool {
        let name = format!("{op:?}");
        let name = name
            .split([' ', '{', '('])
            .next()
            .unwrap_or_default()
            .to_string();
        *self.callbacks.entry(name).or_default() += 1;
        true
    }
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
        false
    }
    fn first_player_flag(&self) -> bool {
        true
    }
    fn tag_exists(&self, _k: u32) -> bool {
        true
    }
    fn condition_query(&mut self, _q: &ConditionQuery) -> Option<bool> {
        Some(true)
    }
    fn poll_link(&mut self, _l: &Link) -> bool {
        true
    }
    fn apply_action(&mut self, action: &Action) -> ActionResult {
        self.handle += 1;
        match action {
            Action::SpawnPlaced { .. }
            | Action::CreateIfAbsent { .. }
            | Action::ObjectAnimStart { .. }
            | Action::Sound { .. }
            | Action::Effect { .. } => ActionResult::Handle(self.handle),
            _ => ActionResult::Done,
        }
    }
    fn room_active(&self, _r: u32) -> bool {
        false
    }
}

#[test]
fn real_levels_run_every_disc_callback_through_the_decoder() {
    let Some(blobs) = real_blobs() else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    let mut gaps: HashSet<u32> = HashSet::new();
    let mut ran_verbs: HashSet<u32> = HashSet::new();
    let mut disc_verbs: HashSet<u32> = HashSet::new();
    for (name, blob) in &blobs {
        let events = parse_events(blob).unwrap();
        disc_verbs.extend(events.iter().map(|e| e.verb));
        for players in [1u32, 2] {
            let mut engine = Engine::new(
                &events,
                BuildOptions {
                    context: context_for(name, players),
                },
            );
            engine.load_static_slots(&slots::parse_slot_defs(blob).unwrap());
            engine.set_checkpoint_mask(u32::MAX);
            let mut world = Everything::default();
            engine.activate_all_slot_links(&mut world);
            engine.run_slot_callbacks(0, &mut world);
            for frame in 0..600 {
                let report = engine.step(0.1, &mut world);
                ran_verbs.extend(report.fired.iter().map(|f| f.verb));
                assert!(report.errors.is_empty(), "{name}: {:?}", report.errors);
                if frame == 300 {
                    engine.run_slot_callbacks(2, &mut world);
                    engine.run_slot_callbacks(1, &mut world);
                }
            }
            gaps.extend(engine.callback_gaps().iter().map(|g| g.id));
            for (key, count) in world.callbacks {
                *totals.entry(key).or_default() += count;
            }
        }
    }
    assert!(
        gaps.is_empty(),
        "callback ids without a decoded body ran: {gaps:?}"
    );
    assert!(
        !totals.keys().any(|k| k.starts_with("legacy")),
        "the legacy hook ran: {totals:?}"
    );
    assert!(
        totals.get("NpcGoal").copied().unwrap_or(0) > 0,
        "callback 28 is the most used one"
    );
    assert!(totals.get("ResetInputDriven").copied().unwrap_or(0) > 0);
    eprintln!("replayed callback steps by kind: {totals:?}");
    eprintln!(
        "disc verbs {}, executed {}",
        disc_verbs.len(),
        ran_verbs.len()
    );
}

/// Objective stimulus (FABRICATED): the arcade group setup `0x233990` creates the group slots at
/// runtime and the objective logic finishes them, neither of which is part of this crate. The
/// replay creates the static slots that link mode 9 waits for (arcade: mask 1) and finishes every
/// static slot after one second, to prove the verbs behind those links run once the objective
/// logic of the game world finishes the slots (54, 79, 80). Verb 77 (level 4) is NOT behind a
/// static slot: its voice chain is gated by an event that must not have executed (gate mode 3),
/// which `activate_all_slot_links` of the full replay breaks by firing every keyed message.
#[test]
fn real_levels_reach_the_verbs_that_wait_for_static_slots() {
    let Some(blobs) = real_blobs() else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let mut reached: BTreeMap<u32, usize> = BTreeMap::new();
    for (name, blob) in &blobs {
        let events = parse_events(blob).unwrap();
        let waiting: Vec<u32> = events
            .iter()
            .filter(|e| e.link_a.mode == 9 || e.link_b.mode == 9)
            .flat_map(|e| [e.link_a.args[0], e.link_b.args[0]])
            .filter(|&id| id < slots::STATIC_SLOT_COUNT)
            .collect();
        for players in [1u32, 2] {
            let ctx = context_for(name, players);
            let mut engine = Engine::new(&events, BuildOptions { context: ctx });
            engine.load_static_slots(&slots::parse_slot_defs(blob).unwrap());
            engine.set_checkpoint_mask(u32::MAX);
            for &id in &waiting {
                if engine.slots().get(id).is_none() {
                    let mask = if ctx.mode_id == 3 { 1 } else { 0x700 };
                    engine.slots_mut().create(&ctx, id, 0, [1, 1, 1], mask);
                }
            }
            let mut world = Everything::default();
            for frame in 0..200 {
                if frame == 10 {
                    for id in 0..slots::STATIC_SLOT_COUNT {
                        if engine.slots().get(id).is_some() {
                            engine.finish_slot(id, &mut world);
                        }
                    }
                }
                for fired in engine.step(0.1, &mut world).fired {
                    if [54, 77, 79, 80].contains(&fired.verb) {
                        *reached.entry(fired.verb).or_default() += 1;
                    }
                }
            }
        }
    }
    eprintln!("verbs behind static slot links, executions: {reached:?}");
    for verb in [54, 77, 79, 80] {
        assert!(
            reached.contains_key(&verb),
            "verb {verb} never ran after the static slots finished"
        );
    }
}
