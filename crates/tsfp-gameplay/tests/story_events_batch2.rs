//! T1841 tests: slot manager, build predicate, condition kinds, verb actions, tail waits,
//! checkpoint snapshot/restore. Synthetic scenarios are FABRICATED inputs checked against rules
//! MEASURED in `build/default.xbe` (see docs/t-story-event-engine.md). Real-data tests skip
//! without TSFP_SETU_PAK_DIR; no game data is committed.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use tsfp_gameplay::story_events::actions::{
    self, Action, ActionResult, Completion, FlagOp, PreWait, Wait,
};
use tsfp_gameplay::story_events::record::{Condition, EventRecord, Link};
use tsfp_gameplay::story_events::setu::{parse_events, setu_blobs_from_pak};
use tsfp_gameplay::story_events::slots::{
    self, CampaignOrdinal, MissionEvent, ObjectiveDefinitionKey, ObjectiveSlotId, SESSION_COMPLETE,
    SESSION_FAILED, STATUS_ACTIVE, STATUS_FAILED, STATUS_FINISHED, STATUS_MARKED, STATUS_PENDING,
    SlotCallback, SlotEvent, SlotManager,
};
use tsfp_gameplay::story_events::snapshot::EventSnapshot;
use tsfp_gameplay::story_events::world::{
    CallbackTarget, ConditionQuery, IdVariant, PropRequest, SpawnRequest, VoiceRequest, World,
};
use tsfp_gameplay::story_events::{BuildOptions, Engine, EngineError, LinkSide, ModeContext};

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

/// Recording world. Everything is scripted by plain fields so each test states its inputs.
#[derive(Default)]
struct W {
    log: Vec<String>,
    actions: Vec<Action>,
    action_result: Option<ActionResult>,
    waits_done: HashSet<String>,
    all_waits_done: bool,
    waits_seen: Vec<Wait>,
    prewait_blocked: bool,
    prewaits_seen: Vec<PreWait>,
    polled: HashMap<u32, bool>,
    queries: HashMap<String, bool>,
    players: Vec<Vec<u32>>,
    first_player: Option<bool>,
    tags: Vec<u32>,
    slot_events: Vec<SlotEvent>,
    slot_callbacks: Vec<(u32, u32)>,
    missions: Vec<MissionEvent>,
    matches_ended: u32,
    checkpoints: u32,
    down: bool,
    stopped: Vec<u32>,
    cleared: Vec<u32>,
    next_handle: u32,
    object_ready: bool,
    object_missing: HashSet<u32>,
    random: f32,
    room_active: bool,
    restore_words: HashMap<u32, u32>,
    object_words: HashMap<u32, (u32, u32)>,
    set_words: Vec<(u32, u32, u32)>,
    persistent_ready: bool,
    stage: u32,
    voice_len: u32,
    voice_left: HashMap<u32, u32>,
    legacy_ok: bool,
}

impl World for W {
    fn play_voice(&mut self, r: &VoiceRequest) -> Option<u32> {
        self.next_handle += 1;
        self.log.push(format!("voice {}", r.label));
        self.voice_left.insert(self.next_handle, self.voice_len);
        Some(self.next_handle)
    }
    fn voice_finished(&mut self, h: u32) -> bool {
        let left = self.voice_left.entry(h).or_insert(0);
        if *left == 0 {
            return true;
        }
        *left -= 1;
        false
    }
    fn run_callback(&mut self, c: u32, t: CallbackTarget) {
        self.log.push(format!("callback {c} {t:?}"));
    }
    fn set_objective(&mut self, a: u32, i: u32) {
        self.log.push(format!("objective {a:#x} {i}"));
    }
    fn control_prop(&mut self, r: &PropRequest) {
        self.log
            .push(format!("prop {:#x} {} {}", r.key, r.activate, r.hold));
    }
    fn prop_settled(&mut self, _key: u32) -> bool {
        self.all_waits_done
    }
    fn spawn_object(&mut self, r: &SpawnRequest) -> Option<u32> {
        self.next_handle += 1;
        self.log.push(format!("spawn {}", r.object_type));
        Some(self.next_handle)
    }
    fn object_ready(&mut self, _h: u32) -> bool {
        self.object_ready
    }
    fn destroy_object(&mut self, h: u32) {
        self.log.push(format!("destroy {h}"));
    }
    fn object_exists(&self, h: u32) -> bool {
        !self.object_missing.contains(&h)
    }
    fn player_count(&self) -> usize {
        self.players.len()
    }
    fn player_matches(&self, p: usize, _v: IdVariant, ids: &[u32]) -> bool {
        self.players[p].iter().any(|id| ids.contains(id))
    }
    fn first_player_available(&self) -> bool {
        self.first_player.unwrap_or(!self.players.is_empty())
    }
    fn first_player_flag(&self) -> bool {
        true
    }
    fn tag_exists(&self, k: u32) -> bool {
        self.tags.contains(&k)
    }
    fn condition_query(&mut self, q: &ConditionQuery) -> Option<bool> {
        self.queries.get(&format!("{q:?}")).copied()
    }
    fn poll_link(&mut self, l: &Link) -> bool {
        self.polled.get(&l.mode).copied().unwrap_or(false)
    }
    fn prewait_check(&mut self, _verb: u32, check: &PreWait) -> bool {
        self.prewaits_seen.push(*check);
        !self.prewait_blocked
    }
    fn other_verb(&mut self, verb: u32, i: usize) -> bool {
        self.log.push(format!("legacy {verb} #{i}"));
        self.legacy_ok
    }
    fn apply_action(&mut self, a: &Action) -> ActionResult {
        self.actions.push(a.clone());
        self.action_result.unwrap_or(ActionResult::Unsupported)
    }
    fn wait_done(&mut self, w: &Wait) -> bool {
        self.waits_seen.push(*w);
        self.all_waits_done || self.waits_done.contains(&format!("{w:?}"))
    }
    fn slot_event(&mut self, e: &SlotEvent) {
        self.slot_events.push(*e);
    }
    fn slot_callback(&mut self, s: u32, c: u32) {
        self.slot_callbacks.push((s, c));
    }
    fn mission_event(&mut self, e: MissionEvent) {
        self.missions.push(e);
    }
    fn players_all_down(&self) -> bool {
        self.down
    }
    fn end_match(&mut self) {
        self.matches_ended += 1;
    }
    fn checkpoint_fired(&mut self) {
        self.checkpoints += 1;
    }
    fn clear_target(&mut self, k: u32) {
        self.cleared.push(k);
    }
    fn stop_sound(&mut self, h: u32) {
        self.stopped.push(h);
    }
    fn random_unit(&mut self) -> f32 {
        self.random
    }
    fn room_active(&self, _r: u32) -> bool {
        self.room_active
    }
    fn restore_save(&mut self, id: u32) -> u32 {
        self.restore_words.get(&id).copied().unwrap_or(0)
    }
    fn restore_apply(&mut self, id: u32, word: u32) {
        self.log.push(format!("restore_apply {id} {word:#x}"));
    }
    fn object_words(&self, h: u32) -> Option<(u32, u32)> {
        self.object_words.get(&h).copied()
    }
    fn set_object_words(&mut self, h: u32, a: u32, b: u32) {
        self.set_words.push((h, a, b));
    }
    fn persistent_spawn_ready(&mut self, _t: u32) -> bool {
        self.persistent_ready
    }
    fn mission_stage(&self) -> u32 {
        self.stage
    }
}

fn started(records: &[EventRecord]) -> Engine {
    let mut engine = Engine::new(records, BuildOptions::default());
    engine.set_checkpoint_mask(1);
    engine
}

fn frames(engine: &mut Engine, world: &mut W, count: usize) -> Vec<u32> {
    let mut order = Vec::new();
    for _ in 0..count {
        order.extend(engine.step(0.1, world).fired.iter().map(|f| f.event_id));
    }
    order
}

// ---------------------------------------------------------------- build predicate 0x6B2B0

#[test]
fn build_predicate_story_submodes_and_difficulty() {
    let ctx = |difficulty, submode| ModeContext::story(difficulty, submode);
    // The 0x700 family: one bit per sub-mode.
    assert!(ctx(0, 0).build_predicate(0x700));
    assert!(ctx(0, 2).build_predicate(0x700));
    assert!(ctx(0, 0).build_predicate(0x100) && !ctx(0, 1).build_predicate(0x100));
    assert!(ctx(0, 1).build_predicate(0x200) && !ctx(0, 2).build_predicate(0x200));
    assert!(ctx(0, 2).build_predicate(0x400) && !ctx(0, 0).build_predicate(0x400));
    assert!(!ctx(0, 0).build_predicate(0x1), "no sub-mode bit");
    // Difficulty bits 0x800, 0x1000, 0x2000 select difficulty 0, 1, 2 (zero = all).
    assert!(ctx(0, 0).build_predicate(0x900) && !ctx(1, 0).build_predicate(0x900));
    assert!(ctx(1, 0).build_predicate(0x1100) && ctx(2, 0).build_predicate(0x2100));
    assert!(ctx(2, 0).build_predicate(0x3900) && !ctx(0, 0).build_predicate(0x2100));
}

#[test]
fn build_predicate_other_modes_flags_and_players() {
    let base = ModeContext::default();
    let mode = |id| ModeContext {
        mode_id: id,
        ..base
    };
    // Jump table 0x6B284: mode id -> mask bit.
    for (id, bit) in [
        (1, 0x02),
        (2, 0x04),
        (3, 0x01),
        (4, 0x08),
        (5, 0x40),
        (6, 0x20),
        (7, 0x10),
        (9, 0x08),
    ] {
        assert!(mode(id).build_predicate(bit), "mode {id}");
        assert!(
            !mode(id).build_predicate(!bit & 0x7F),
            "mode {id} other bits"
        );
    }
    assert!(!mode(8).build_predicate(0xFFFF) && !mode(11).build_predicate(0xFFFF));
    // Bit 18 path when game flag 0x20000000 is set.
    let flagged = ModeContext {
        game_flags: 0x2000_0000,
        ..base
    };
    assert!(flagged.build_predicate(0x4_0000) && !flagged.build_predicate(0x700));
    // Descriptor path for flags 0x40 / 0x8000000.
    let descriptor = ModeContext {
        game_flags: 0x40,
        descriptor_word: 0x30,
        ..base
    };
    assert!(descriptor.build_predicate(0x10) && !descriptor.build_predicate(0x700));
    let descriptor = ModeContext {
        game_flags: 0x800_0000,
        descriptor_word: 0x30,
        ..base
    };
    assert!(descriptor.base_predicate(0x20));
    // Player count bits 0x200000 (one) and 0x400000 (two); network bit 31.
    let masks = 0x700 | 0x20_0000;
    assert!(ModeContext { players: 1, ..base }.build_predicate(masks));
    assert!(!ModeContext { players: 2, ..base }.build_predicate(masks));
    assert!(ModeContext { players: 2, ..base }.build_predicate(0x700 | 0x40_0000));
    assert!(ModeContext { players: 2, ..base }.build_predicate(0x700 | 0x60_0000));
    assert!(
        !ModeContext {
            network: true,
            ..base
        }
        .build_predicate(0x8000_0700)
    );
    assert!(
        ModeContext {
            network: false,
            ..base
        }
        .build_predicate(0x8000_0700)
    );
    // The event-slot mask 0xE00077F passes for one or two players in story mode.
    assert!(ModeContext { players: 2, ..base }.base_predicate(slots::event_slot_mask()));
}

#[test]
fn engine_drops_definitions_by_the_decoded_predicate() {
    let mut a = record(1, 72);
    a.mode_mask = 0x200; // sub-mode 1 only
    let mut b = record(2, 72);
    b.mode_mask = 0x100;
    let engine = Engine::new(&[a, b], BuildOptions::story(0, 0));
    assert_eq!((engine.len(), engine.dropped()), (1, 1));
    assert_eq!(engine.record(0).event_id, 2);
    assert_eq!(
        Engine::new(&[a, b], BuildOptions::story(0, 1))
            .record(0)
            .event_id,
        1
    );
}

// ---------------------------------------------------------------- slot manager

fn ctx() -> ModeContext {
    ModeContext::default()
}

#[test]
fn slot_creation_status_and_counters() {
    let mut slots = SlotManager::new();
    assert!(slots.create(&ctx(), 0, 0x13F7, [0, 0, 0], 0x700));
    assert_eq!(slots.status(0), STATUS_ACTIVE);
    assert_eq!(slots.active_count(0), 1);
    // Kind -1 for the active sub-mode: nothing created, call still succeeds.
    assert!(slots.create(&ctx(), 1, 0, [NONE, NONE, NONE], 0x700));
    assert_eq!(slots.status(1), 0);
    // A mask the predicate rejects: not created, call fails.
    assert!(!slots.create(&ctx(), 2, 0, [0, 0, 0], 0x1));
    assert_eq!(slots.len(), 1);
    // Hidden kind 2 slot.
    slots.create(&ctx(), 3, 0, [2, 2, 2], 0x700);
    assert_eq!((slots.active_count(2), slots.count_of_kind(2)), (1, 1));
}

#[test]
fn slot_verbs_follow_the_status_machine() {
    let mut slots = SlotManager::new();
    slots.create(&ctx(), 4, 9, [0, 0, 0], 0x700);
    slots.deactivate(4);
    assert_eq!(
        (slots.status(4), slots.active_count(0)),
        (STATUS_PENDING, 0)
    );
    // Verb 34: pending -> active, counter up, HUD notice.
    let change = slots.start_pending(4);
    assert_eq!((slots.status(4), slots.active_count(0)), (STATUS_ACTIVE, 1));
    assert_eq!(
        change.events,
        vec![SlotEvent::Changed {
            slot: 4,
            status: STATUS_ACTIVE,
            label: 9
        }]
    );
    // Verb 36: active -> marked.
    slots.activate(4);
    assert_eq!(slots.status(4), STATUS_MARKED);
    // Verb 37: -> failed. Verb 33 -> active again, counter unchanged.
    slots.mark_failed(4);
    assert!(slots.is_failed(4));
    slots.reactivate(4);
    assert_eq!((slots.status(4), slots.active_count(0)), (STATUS_ACTIVE, 1));
    // Verb 32 quiet complete: no events, no callback.
    slots.complete_quiet(4);
    assert!(slots.is_finished(4));
    // Free slots ignore every verb.
    slots.mark_failed(40);
    slots.reactivate(40);
    assert_eq!(slots.status(40), 0);
}

#[test]
fn slot_finish_activates_pending_and_returns_callbacks() {
    let mut slots = SlotManager::new();
    slots.create(&ctx(), 7, 3, [0, 0, 0], 0x700);
    slots.set_callback(7, SlotCallback::World(5));
    slots.deactivate(7);
    let change = slots.finish(&ctx(), 7);
    assert!(slots.is_finished(7));
    assert_eq!(slots.active_count(0), 1, "pending slot was started first");
    assert_eq!(change.callbacks, vec![(7, SlotCallback::World(5))]);
    assert!(
        matches!(change.events[0], SlotEvent::Finished { .. })
            || matches!(change.events[0], SlotEvent::Changed { .. })
    );
    // The measured objective terminal latch suppresses the repeated completion side effects.
    let repeated = slots.finish(&ctx(), 7);
    assert!(repeated.callbacks.is_empty());
    assert!(repeated.events.is_empty());
    assert_eq!(slots.active_count(0), 1);
    // A failed slot cannot finish.
    slots.create(&ctx(), 8, 0, [0, 0, 0], 0x700);
    slots.mark_failed(8);
    assert!(slots.finish(&ctx(), 8).callbacks.is_empty());
}

#[test]
fn slot_finish_mode3_path_stamps_the_frame() {
    let mode3 = ModeContext {
        mode_id: 3,
        ..ModeContext::default()
    };
    let mut slots = SlotManager::new();
    slots.frame = 1000;
    slots.create(&mode3, 1, 11, [0, 0, 0], 0x1);
    slots.create(&mode3, 2, 12, [2, 2, 2], 0x1);
    let change = slots.finish(&mode3, 1);
    assert_eq!(slots.get(1).unwrap().finished_stamp, 1000 - 180);
    assert!(matches!(
        change.events[0],
        SlotEvent::Finished {
            mode3: true,
            slot: 1,
            label: 11,
            ..
        }
    ));
    // Kind 2 slots use the generic path (no HUD event).
    assert!(slots.finish(&mode3, 2).events.is_empty());
    assert!(slots.is_finished(2));
}

#[test]
fn mission_evaluation_matches_0x228870() {
    let build = |statuses: &[u8]| {
        let mut slots = SlotManager::new();
        for (i, status) in statuses.iter().enumerate() {
            slots.create(&ctx(), i as u32, 0, [0, 0, 0], 0x700);
            match status {
                5 => slots.complete_quiet(i as u32),
                4 => {
                    slots.mark_failed(i as u32);
                }
                _ => {}
            }
        }
        slots
    };
    // All finished: complete, flag 0x8000.
    let mut slots = build(&[5, 5]);
    let (event, flags) = slots.evaluate_mission(&ctx(), 0, false);
    assert_eq!(
        (event, flags),
        (Some(MissionEvent::Complete), SESSION_COMPLETE)
    );
    // Open objective: nothing.
    let mut slots = build(&[5, 2]);
    assert_eq!(slots.evaluate_mission(&ctx(), 0, false), (None, 0));
    // One failed, all resolved: failed.
    let mut slots = build(&[5, 4]);
    assert_eq!(
        slots.evaluate_mission(&ctx(), 0, false),
        (Some(MissionEvent::ObjectivesFailed), SESSION_FAILED)
    );
    // One failed, one open: message only every 10th frame.
    let mut slots = build(&[4, 2]);
    slots.frame = 7;
    assert_eq!(slots.evaluate_mission(&ctx(), 0, false), (None, 0));
    slots.frame = 20;
    assert_eq!(
        slots.evaluate_mission(&ctx(), 0, false),
        (Some(MissionEvent::ObjectiveFailedPending), SESSION_FAILED)
    );
    // All players down in story mode.
    let mut slots = build(&[2]);
    assert_eq!(
        slots.evaluate_mission(&ctx(), 0, true),
        (Some(MissionEvent::AllPlayersDown), SESSION_FAILED)
    );
    // Already complete: nothing.
    let mut slots = build(&[5]);
    assert_eq!(
        slots.evaluate_mission(&ctx(), SESSION_COMPLETE, false),
        (None, SESSION_COMPLETE)
    );
    // Flag bit 1 counts as done and is auto-finished on completion (0x227E40).
    let mut slots = build(&[5, 2]);
    slots.get_mut(1).unwrap().flags = 2;
    let (event, _) = slots.evaluate_mission(&ctx(), 0, false);
    assert_eq!(event, Some(MissionEvent::Complete));
    assert!(slots.is_finished(1));
}

#[test]
fn mission_outcome_is_a_read_only_typed_snapshot_of_admitted_static_slots() {
    let mut slots = SlotManager::new();
    assert!(slots.create(&ctx(), 0, 11, [0, 0, 0], 0x700));
    assert!(slots.create(&ctx(), 5, 22, [2, 2, 2], 0x700));
    assert!(slots.create(&ctx(), 0x2d, 33, [2, 2, 2], 0x700));
    slots.mark_failed(0);
    let ids_before = slots.ids().to_vec();
    let outcome = slots.mission_outcome(
        10,
        SESSION_COMPLETE | SESSION_FAILED,
        8,
        Some(MissionEvent::ObjectivesFailed),
        CampaignOrdinal::new(0),
    );
    assert_eq!(outcome.mode_id, 10);
    assert_eq!(outcome.result_flags, SESSION_COMPLETE | SESSION_FAILED);
    assert_eq!(outcome.phase, 8);
    assert!(outcome.has_success_result());
    assert!(outcome.has_failure_result());
    assert_eq!(outcome.campaign_ordinal, CampaignOrdinal::new(0));
    assert_eq!(outcome.objectives.len(), 2);
    assert_eq!(outcome.objectives[0].id, ObjectiveSlotId(0));
    assert_eq!(outcome.objectives[0].status, STATUS_FAILED);
    assert_eq!(outcome.objectives[1].id, ObjectiveSlotId(5));
    assert_eq!(outcome.objectives[1].kind, 2);
    assert_eq!(slots.ids(), ids_before);
    assert_eq!(slots.status(0), STATUS_FAILED);
    assert_eq!(slots.status(0x2d), STATUS_ACTIVE);
}

#[test]
fn mission_outcome_preserves_runtime_objective_definition_keys() {
    let definition = slots::SlotDef {
        id: 2,
        key: ObjectiveDefinitionKey(0x1234_5678),
        mask: 0x700,
        label: 9,
        kinds: [0, 0, 0],
        callback_id: 0,
        dependencies: [NONE; 5],
        initially_active: true,
    };
    let mut slots = SlotManager::new();
    slots.load_level(&ctx(), &[definition]);

    let outcome = slots.mission_outcome(10, 0, 2, None, CampaignOrdinal::new(0));
    assert_eq!(outcome.objectives.len(), 1);
    assert_eq!(outcome.objectives[0].id, ObjectiveSlotId(2));
    assert_eq!(
        outcome.objectives[0].definition_key,
        Some(ObjectiveDefinitionKey(0x1234_5678))
    );
}

fn slot_blob(defs: &[[u32; 14]]) -> Vec<u8> {
    let mut blob = vec![0u8; 0xC0 + defs.len() * 0x38];
    blob[..4].copy_from_slice(b"SETU");
    blob[0x84..0x88].copy_from_slice(&0xC0u32.to_le_bytes());
    blob[0x88..0x8C].copy_from_slice(&(defs.len() as u32).to_le_bytes());
    blob[0x8C..0x90].copy_from_slice(&0x38u32.to_le_bytes());
    for (i, def) in defs.iter().enumerate() {
        for (k, word) in def.iter().enumerate() {
            let at = 0xC0 + i * 0x38 + k * 4;
            blob[at..at + 4].copy_from_slice(&word.to_le_bytes());
        }
    }
    blob
}

#[test]
fn level_start_activation_from_the_slot_table() {
    // Shape of a Story level table: three pending objectives, then hidden active kind-2 slots.
    let blob = slot_blob(&[
        [
            0, 0x4F424A31, 0x700, 0x13F7, 0, 0, 0, 5, NONE, NONE, NONE, NONE, NONE, 0,
        ],
        [
            1, 0x4F424A32, 0x700, 0x13F8, 0, 0, 0, 0, 7, NONE, NONE, NONE, NONE, 0,
        ],
        [
            3, 0x4F424A34, 0x700, 0, 2, 2, 2, 0, NONE, NONE, NONE, NONE, NONE, 1,
        ],
        [
            4, 0x4F424A35, 0x1, 0, 2, 2, 2, 0, NONE, NONE, NONE, NONE, NONE, 1,
        ],
    ]);
    let defs = slots::parse_slot_defs(&blob).expect("parse");
    assert_eq!(defs.len(), 4);
    assert_eq!(defs[0].callback_id, 5);
    assert!(!defs[0].initially_active && defs[2].initially_active);
    let mut engine = Engine::new(&[], BuildOptions::default());
    engine.load_static_slots(&defs);
    let slots = engine.slots();
    assert_eq!(
        (slots.status(0), slots.status(1)),
        (STATUS_PENDING, STATUS_PENDING)
    );
    assert_eq!(
        slots.status(3),
        STATUS_ACTIVE,
        "entry flag +0x34 set: active at start"
    );
    assert_eq!(
        slots.status(4),
        0,
        "mask 0x1 fails the story predicate: not created"
    );
    assert_eq!(slots.active_count(0), 0, "pending objectives do not count");
    assert_eq!(slots.active_count(2), 1);
    assert_eq!(slots.get(0).unwrap().callback, SlotCallback::World(5));
    assert_eq!(slots.get(1).unwrap().dependencies, vec![7]);
    // Verb 36 / 34 starts a pending objective.
    let mut start = record(1, 36);
    set_arg(&mut start, 0xC, 0);
    let mut engine2 = Engine::new(&[start], BuildOptions::default());
    engine2.load_static_slots(&defs);
    engine2.set_checkpoint_mask(1);
    let mut world = W::default();
    frames(&mut engine2, &mut world, 1);
    assert_eq!(engine2.slots().status(0), STATUS_ACTIVE);
    assert_eq!(
        world.slot_events,
        vec![SlotEvent::Changed {
            slot: 0,
            status: STATUS_ACTIVE,
            label: 0x13F7
        }]
    );
    assert!(slots::parse_slot_defs(&blob[..0xC4]).is_err());
}

#[test]
fn slot_verbs_31_to_37_run_through_the_engine() {
    let verbs = [31u32, 32, 33, 34, 35, 36, 37];
    for verb in verbs {
        let mut r = record(1, verb);
        set_arg(&mut r, 0xC, 2);
        let mut engine = started(&[r]);
        let mut world = W::default();
        engine
            .slots_mut()
            .create(&ModeContext::default(), 2, 7, [0, 0, 0], 0x700);
        assert_eq!(frames(&mut engine, &mut world, 1), vec![1], "verb {verb}");
        let status = engine.slots().status(2);
        let expected = match verb {
            31 | 32 => STATUS_FINISHED,
            33 | 34 => STATUS_ACTIVE,
            35 => STATUS_PENDING,
            36 => STATUS_MARKED,
            _ => STATUS_FAILED,
        };
        assert_eq!(status, expected, "verb {verb}");
        assert!(engine.is_done(0), "slot verbs complete at once");
    }
}

// ---------------------------------------------------------------- event slots (builder 0x235630)

#[test]
fn event_slot_ids_count_up_from_0x2d_and_follow_the_builder() {
    let mut first = record(1, 72);
    first.link_a.mode = 1; // slot 0x2D
    let mut second = record(2, 72);
    second.link_a.mode = 3; // consumes 0x2E, no slot
    let mut third = record(3, 72);
    third.link_a.mode = 4;
    third.combine = 1;
    third.link_b.mode = 6; // 0x2F and 0x30
    let mut fourth = record(4, 72);
    fourth.link_a.mode = 5; // no combine: only A -> 0x31
    fourth.link_b.mode = 6; // B ignored
    let mut disabled = record(5, 72);
    disabled.link_a.mode = 1;
    disabled.flags = 2; // difficulty 1 only: no slots, no id consumed
    let mut last = record(6, 72);
    last.link_a.mode = 1; // 0x32
    let engine = Engine::new(
        &[first, second, third, fourth, disabled, last],
        BuildOptions::story(0, 0),
    );
    let ids: Vec<_> = (0..engine.len())
        .map(|i| (engine.instance(i).slot_a, engine.instance(i).slot_b))
        .collect();
    assert_eq!(
        ids,
        vec![
            (Some(0x2D), None),
            (None, None),
            (Some(0x2F), Some(0x30)),
            (Some(0x31), None),
            (None, None),
            (Some(0x32), None),
        ]
    );
    // Event slots are created active and hidden (kind 2).
    let slot = engine.slots().get(0x2D).expect("slot");
    assert_eq!((slot.status, slot.kind), (STATUS_ACTIVE, 2));
    assert_eq!(
        slot.callback,
        SlotCallback::EventLink {
            instance: 0,
            side: LinkSide::A
        }
    );
    assert_eq!(engine.slots().active_count(2), 5);
}

#[test]
fn polled_link_is_satisfied_by_the_slot_update_one_frame_later() {
    let mut r = record(1, 72);
    r.link_a.mode = 6;
    let mut engine = started(&[r]);
    let mut world = W::default();
    assert!(frames(&mut engine, &mut world, 3).is_empty());
    world.polled.insert(6, true);
    // Frame 1: state pass sees nothing, slot update polls and finishes the slot. Frame 2 fires.
    assert!(frames(&mut engine, &mut world, 1).is_empty());
    assert!(engine.slots().is_finished(0x2D));
    assert_eq!(frames(&mut engine, &mut world, 1), vec![1]);
}

#[test]
fn link_mode_9_waits_for_a_slot_and_mode_4_for_an_instance() {
    let mut watcher = record(1, 72);
    watcher.link_a = Link {
        mode: 9,
        id: 0,
        args: [3, NONE, NONE, NONE],
    };
    let mut engine = started(&[watcher]);
    engine.slots_mut().create(&ctx(), 3, 0, [0, 0, 0], 0x700);
    let mut world = W::default();
    assert!(frames(&mut engine, &mut world, 3).is_empty());
    engine.slots_mut().complete_quiet(3);
    frames(&mut engine, &mut world, 1);
    assert_eq!(frames(&mut engine, &mut world, 1), vec![1]);
}

#[test]
fn external_links_need_a_finish_a_key_signal_or_the_restore_rule() {
    for mode in [5u32, 12, 13, 19, 20] {
        let mut r = record(1, 72);
        r.link_a = Link {
            mode,
            id: 0xAB,
            args: [4, NONE, NONE, NONE],
        };
        let mut engine = started(&[r]);
        let mut world = W::default();
        assert!(
            frames(&mut engine, &mut world, 4).is_empty(),
            "mode {mode} is never polled"
        );
        engine.finish_slot(0x2D, &mut world);
        assert_eq!(
            frames(&mut engine, &mut world, 1),
            vec![1],
            "mode {mode} via slot finish"
        );
    }
    // Key signals: mode 5 through 0x2345B0, modes 12 and 13 through 0x2345D0.
    let mut five = record(1, 72);
    five.link_a = Link {
        mode: 5,
        id: 0xAB,
        args: [NONE; 4],
    };
    let mut twelve = record(2, 72);
    twelve.link_a = Link {
        mode: 12,
        id: 0xCD,
        args: [NONE; 4],
    };
    let mut engine = started(&[five, twelve]);
    let mut world = W::default();
    assert_eq!(engine.signal_key(0xCD, false, &[5]), 0, "wrong key class");
    assert_eq!(engine.signal_key(0xAB, false, &[5]), 1);
    assert_eq!(frames(&mut engine, &mut world, 1), vec![1]);
    assert_eq!(engine.signal_key(0xCD, false, &[12, 13]), 1);
    assert_eq!(frames(&mut engine, &mut world, 1), vec![2]);
}

#[test]
fn verb_57_resets_keyed_instances_and_asks_the_world_to_toggle_the_object() {
    let mut keyed = record(1, 72);
    keyed.link_a = Link {
        mode: 5,
        id: 0xAB,
        args: [NONE; 4],
    };
    let mut toggler = record(2, 57);
    set_arg(&mut toggler, 0xC, 0xAB);
    let mut engine = started(&[keyed, toggler]);
    let mut world = W {
        action_result: Some(ActionResult::Done),
        ..W::default()
    };
    engine.signal_key(0xAB, false, &[5]);
    assert_eq!(frames(&mut engine, &mut world, 1), vec![1, 2]);
    // Verb 57 reset instance 0: link cleared, slot re-armed, may run again after a new signal.
    assert!(!engine.instance(0).a_set && engine.instance(0).executed == 0);
    assert_eq!(world.actions, vec![Action::ActivateByKey { key: 0xAB }]);
    engine.signal_key(0xAB, false, &[5]);
    assert_eq!(frames(&mut engine, &mut world, 1), vec![1]);
}

// ---------------------------------------------------------------- state pass details

#[test]
fn missing_gate_instance_skips_the_run_for_gate_modes_1_and_3() {
    for (mode, runs) in [(0u32, true), (1, false), (2, true), (3, false)] {
        let mut r = record(1, 72);
        r.gate_id = 999; // not in the pool
        r.gate_mode = mode;
        let mut engine = started(&[r]);
        let mut world = W::default();
        assert_eq!(
            !frames(&mut engine, &mut world, 2).is_empty(),
            runs,
            "gate mode {mode}"
        );
    }
}

#[test]
fn delay_counts_down_while_a_prewait_blocks() {
    let mut r = record(1, 43);
    r.delay_bits = 0.25f32.to_bits();
    set_arg(&mut r, 0x14, 0x77);
    let mut engine = started(&[r]);
    let mut world = W {
        prewait_blocked: true,
        action_result: Some(ActionResult::Handle(5)),
        ..W::default()
    };
    assert!(frames(&mut engine, &mut world, 6).is_empty());
    assert_eq!(world.prewaits_seen[0], PreWait::KeyStage { key: 0x77 });
    world.prewait_blocked = false;
    // The delay already elapsed during the block (T1840 counted it only after the pre-wait).
    assert_eq!(frames(&mut engine, &mut world, 1), vec![1]);
    assert_eq!(engine.instance(0).object_handle, Some(5));
}

#[test]
fn prewait_descriptors_per_verb() {
    let mut r = record(1, 3);
    set_arg(&mut r, 0x10, 9);
    set_arg(&mut r, 0x18, 0x0001_0044);
    assert_eq!(
        actions::prewait(&r),
        Some(PreWait::ObjectType {
            type_word: 0x44,
            table_index: 9
        })
    );
    r.verb = 7;
    assert!(
        actions::prewait(&r).is_some(),
        "verb 7 has the object-type pre-wait (T1841 correction)"
    );
    r.verb = 24;
    set_arg(&mut r, 0x18, 3);
    assert_eq!(actions::prewait(&r), Some(PreWait::Movie { index: 3 }));
    for verb in [43, 46, 48, 49, 51] {
        r.verb = verb;
        set_arg(&mut r, 0x14, 0x55);
        assert_eq!(
            actions::prewait(&r),
            Some(PreWait::KeyStage { key: 0x55 }),
            "verb {verb}"
        );
    }
    r.verb = 74;
    set_arg(&mut r, 0x10, 0x66);
    assert_eq!(actions::prewait(&r), Some(PreWait::KeyStage { key: 0x66 }));
    r.verb = 13;
    assert_eq!(actions::prewait(&r), None);
}

#[test]
fn mode_2_link_is_notified_whenever_the_predecessor_reaches_the_done_test() {
    let mut a = record(1, 72);
    a.link_a.mode = 0;
    a.delay_bits = 0.3f32.to_bits();
    let mut b = record(2, 72);
    b.link_a.mode = 2;
    let mut engine = started(&[a, b]);
    let mut world = W::default();
    // B does not wait for A's execution: it is set the first frame A is past its delay.
    assert_eq!(frames(&mut engine, &mut world, 5), vec![1, 2]);
}

// ---------------------------------------------------------------- conditions

fn conditional(condition: Condition) -> (Engine, W) {
    let mut r = record(1, 72);
    r.condition_a = condition;
    (started(&[r]), W::default())
}

fn cond(kind: u32, arg: u32) -> Condition {
    Condition {
        kind,
        args: [arg, NONE, NONE, NONE],
    }
}

#[test]
fn condition_kinds_2_28_32_read_the_slot_manager() {
    for (kind, slot, arg) in [(2u32, 5u32, NONE), (28, 2, NONE), (32, 9, 9)] {
        let (mut engine, mut world) = conditional(cond(kind, arg));
        engine.slots_mut().create(&ctx(), slot, 0, [0, 0, 0], 0x700);
        assert!(
            frames(&mut engine, &mut world, 2).is_empty(),
            "kind {kind} open"
        );
        engine.slots_mut().complete_quiet(slot);
        assert_eq!(
            frames(&mut engine, &mut world, 1),
            vec![1],
            "kind {kind} finished"
        );
    }
}

#[test]
fn condition_kinds_without_a_world_answer_stay_unsupported() {
    for kind in [1u32, 3, 4, 5, 6, 7, 8, 9, 29, 30, 31, 33, 36, 37] {
        let (mut engine, mut world) = conditional(cond(kind, 0x10));
        let report = engine.step(0.1, &mut world);
        assert_eq!(
            report.errors,
            vec![(
                0,
                EngineError::Unsupported {
                    what: "condition kind",
                    value: kind
                }
            )],
            "kind {kind}"
        );
        assert!(report.fired.is_empty());
    }
}

#[test]
fn condition_kinds_with_a_world_answer() {
    let queries = [
        (
            1u32,
            "Scene { kind: 1, args: [16, 4294967295, 4294967295, 4294967295] }",
        ),
        (33, "TableField { key: 16 }"),
        (36, "ChildState { key: 16 }"),
        (37, "FewerThanTwoOfClass"),
        (
            9,
            "Scene { kind: 9, args: [16, 4294967295, 4294967295, 4294967295] }",
        ),
    ];
    for (kind, key) in queries {
        let (mut engine, mut world) = conditional(cond(kind, 0x10));
        world.queries.insert(key.to_string(), false);
        assert!(
            frames(&mut engine, &mut world, 2).is_empty(),
            "kind {kind} false"
        );
        world.queries.insert(key.to_string(), true);
        assert_eq!(
            frames(&mut engine, &mut world, 1),
            vec![1],
            "kind {kind} true"
        );
    }
}

#[test]
fn zero_players_and_out_of_range_kinds() {
    // "All players" over no players is true (MEASURED: esi = 1 before the loop), "any" is false.
    let (mut engine, mut world) = conditional(cond(12, 7));
    assert_eq!(frames(&mut engine, &mut world, 1), vec![1]);
    let (mut engine, mut world) = conditional(cond(11, 7));
    assert!(frames(&mut engine, &mut world, 2).is_empty());
    // Kinds above 37 are false in the original (`ja 0x235f14`), kind 10 always false.
    for kind in [10u32, 38, 99] {
        let (mut engine, mut world) = conditional(cond(kind, 0));
        let report = engine.step(0.1, &mut world);
        assert!(
            report.errors.is_empty() && report.fired.is_empty(),
            "kind {kind}"
        );
    }
}

#[test]
fn first_player_kinds_with_a_null_first_player() {
    // `0x277FE0` can return null: kinds 23, 24, 26, 27 are false, kind 25 (fails b) is true.
    for (kind, expected) in [
        (23u32, false),
        (24, false),
        (26, false),
        (27, false),
        (25, true),
    ] {
        let (mut engine, mut world) = conditional(Condition {
            kind,
            args: [7, NONE, NONE, NONE],
        });
        world.players = vec![vec![7]];
        world.first_player = Some(false);
        assert_eq!(
            !frames(&mut engine, &mut world, 1).is_empty(),
            expected,
            "kind {kind}"
        );
    }
}

// ---------------------------------------------------------------- verbs

fn arg_pairs(verb: u32, args: &[(usize, u32)], flags: u32) -> EventRecord {
    let mut r = record(1, verb);
    r.flags = flags;
    for &(offset, value) in args {
        set_arg(&mut r, offset, value);
    }
    r
}

#[test]
fn decode_reads_the_fields_the_original_reads() {
    let one = 1.0f32.to_bits();
    let d = |r: &EventRecord| actions::decode(r).expect("decoded");
    assert_eq!(
        d(&arg_pairs(13, &[(0x14, 0x41), (0xC, 0x8)], 0)),
        Action::PropFlags {
            key: 0x41,
            mask: 8,
            op: FlagOp::Set
        }
    );
    assert_eq!(
        d(&arg_pairs(14, &[(0x14, 0x41), (0xC, 0x8)], 0)),
        Action::PropFlags {
            key: 0x41,
            mask: 8,
            op: FlagOp::Clear
        }
    );
    assert_eq!(
        d(&arg_pairs(15, &[(0x14, 0x41), (0xC, 0x8)], 0)),
        Action::PropFlags {
            key: 0x41,
            mask: 8,
            op: FlagOp::Toggle
        }
    );
    assert_eq!(
        d(&arg_pairs(44, &[(0x14, 0x9)], 0)),
        Action::DestroyObject { key: 9 }
    );
    assert_eq!(
        d(&arg_pairs(66, &[(0xC, 8192), (0x10, 3)], 0)),
        Action::AlertGroup {
            flags: 8192,
            group: 3
        }
    );
    assert_eq!(
        d(&arg_pairs(
            25,
            &[(0x18, 0x9B5)],
            0x10_0000 | 0x20_0000 | 0x100_0000
        )),
        Action::CameraMode {
            word: 0x9B5,
            flags: 0x10_0000 | 0x80_0000 | 0x100_0000
        }
    );
    assert_eq!(
        d(&arg_pairs(60, &[(0xC, 5), (0x10, 6)], 0x10_0000)),
        Action::CameraCue {
            camera: 5,
            argument: 6,
            freeze_flags: 4
        }
    );
    assert_eq!(
        d(&arg_pairs(60, &[(0xC, 5), (0x10, 6)], 0x2000_0000)),
        Action::CameraCue {
            camera: 5,
            argument: 6,
            freeze_flags: 8
        }
    );
    assert_eq!(
        d(&arg_pairs(
            51,
            &[
                (0x14, 4),
                (0x20, one),
                (0x24, one),
                (0x28, one),
                (0x2C, 2.0f32.to_bits())
            ],
            0x4000
        )),
        Action::PoseRestore {
            key: 4,
            position: Some([1.0, 1.0, 1.0]),
            orientation: None
        }
    );
    assert_eq!(
        d(&arg_pairs(
            53,
            &[
                (0x14, 4),
                (0x10, 3.0f32.to_bits()),
                (0x2C, one),
                (0x30, 2.0f32.to_bits()),
                (0x34, 0)
            ],
            0
        )),
        Action::Impulse {
            key: 4,
            impulse: [3.0, 6.0, 0.0],
            point: None
        }
    );
    assert_eq!(
        d(&arg_pairs(86, &[(0x10, 99), (0x14, 0x3F3F_3F3F)], 0)),
        Action::SampleByText {
            label: 99,
            tag: None
        }
    );
    assert_eq!(
        d(&arg_pairs(86, &[(0x10, 99), (0x14, 7)], 0)),
        Action::SampleByText {
            label: 99,
            tag: Some(7)
        }
    );
    assert_eq!(
        d(&arg_pairs(67, &[(0xC, (-1i32) as u32)], 0)),
        Action::PlayerFlag { player: -1 }
    );
    assert_eq!(
        d(&arg_pairs(70, &[], 0)),
        Action::PlayersFlag8000 { set: true }
    );
    assert_eq!(
        d(&arg_pairs(71, &[], 0)),
        Action::PlayersFlag8000 { set: false }
    );
    // Verbs the engine runs itself have no world action.
    for verb in [
        0, 1, 3, 16, 26, 27, 29, 30, 31, 32, 33, 34, 35, 36, 37, 49, 50, 52, 56, 72, 84, 88, 91,
    ] {
        assert!(actions::decode(&record(1, verb)).is_none(), "verb {verb}");
    }
}

#[test]
fn every_verb_below_93_has_a_defined_executor() {
    // With a world that answers every action every verb runs; without one only the engine's own
    // verbs run and the rest are Unsupported (never silently skipped).
    let native = [
        0u32, 1, 3, 16, 26, 27, 29, 30, 31, 32, 33, 34, 35, 36, 37, 49, 50, 52, 56, 57, 72, 84, 88,
        91,
    ];
    for verb in 0..93u32 {
        let mut r = record(1, verb);
        for offset in (0xC..=0x34).step_by(4) {
            set_arg(&mut r, offset, 1);
        }
        let mut modelled = started(&[r]);
        let mut world = W {
            action_result: Some(ActionResult::Done),
            ..W::default()
        };
        let report = modelled.step(0.1, &mut world);
        assert!(report.errors.is_empty(), "verb {verb}: {:?}", report.errors);
        assert_eq!(report.fired.len(), 1, "verb {verb}");

        let mut bare = started(&[r]);
        let report = bare.step(0.1, &mut W::default());
        if native.contains(&verb) {
            assert!(report.errors.is_empty(), "native verb {verb}");
        } else {
            assert_eq!(
                report.errors[0].1,
                EngineError::Unsupported {
                    what: "verb",
                    value: verb
                },
                "verb {verb}"
            );
        }
    }
}

#[test]
fn action_results_store_handles_and_failed_lookups_abort() {
    let mut sound = arg_pairs(11, &[(0xC, 7)], 0);
    sound.event_id = 1;
    let mut engine = started(&[sound]);
    let mut world = W {
        action_result: Some(ActionResult::Handle(41)),
        ..W::default()
    };
    frames(&mut engine, &mut world, 1);
    assert_eq!(engine.instance(0).sound_handle, Some(41));
    assert_eq!(engine.instance(0).object_handle, None);
    // Failed lookup: verbs 43 and 46 undo running/executed so they can retry.
    for verb in [43u32, 46] {
        let mut engine = started(&[arg_pairs(verb, &[(0x14, 3)], 0)]);
        let mut world = W {
            action_result: Some(ActionResult::Failed),
            ..W::default()
        };
        assert!(frames(&mut engine, &mut world, 1).is_empty());
        assert_eq!(
            (engine.instance(0).running, engine.instance(0).executed),
            (0, 0)
        );
        assert!(!frames(&mut engine, &mut world, 1).is_empty() == false);
    }
    // Other verbs keep running/executed set on a failed lookup (MEASURED early return).
    let mut engine = started(&[arg_pairs(44, &[(0x14, 3)], 0)]);
    let mut world = W {
        action_result: Some(ActionResult::Failed),
        ..W::default()
    };
    assert_eq!(frames(&mut engine, &mut world, 1), vec![1]);
    assert_eq!(engine.instance(0).executed, 1);
}

#[test]
fn legacy_other_verb_hook_still_works_after_an_unsupported_action() {
    let mut engine = started(&[arg_pairs(13, &[(0x14, 1), (0xC, 1)], 0)]);
    let mut world = W {
        legacy_ok: true,
        ..W::default()
    };
    assert_eq!(frames(&mut engine, &mut world, 1), vec![1]);
    assert_eq!(world.log, vec!["legacy 13 #0"]);
}

#[test]
fn completion_classes_match_the_done_table() {
    let flag = [1u32, 3, 4, 5, 6, 7, 16, 18, 28, 49, 50, 60, 61];
    let no_tick = [0u32, 12, 17, 19, 29, 30, 38, 39, 42];
    for verb in 0..93u32 {
        let class = actions::completion(verb);
        let expected = if flag.contains(&verb) {
            Completion::Flag
        } else if no_tick.contains(&verb) {
            Completion::NoTickCallback
        } else {
            match verb {
                10 => Completion::SoundOrNone,
                11 => Completion::Sound,
                24 | 25 | 26 | 62 => Completion::World,
                _ => Completion::Immediate,
            }
        };
        assert_eq!(class, expected, "verb {verb}");
    }
    // Tail pass: the verbs that set the done flag at once (class 0 of the table at 0x237E0C).
    let at_once: Vec<u32> = (0..93)
        .filter(|&v| actions::tail_sets_flag_at_once(v))
        .collect();
    assert_eq!(
        at_once,
        vec![0, 5, 6, 53, 54, 55, 56, 58, 66, 67, 72, 73, 74, 88, 89, 90]
    );
}

#[test]
fn verb_5_and_6_complete_one_pass_after_running_and_gate_the_chain() {
    for verb in [5u32, 6] {
        let mut anim = arg_pairs(verb, &[(0x14, 0x31)], 0);
        anim.link_a.mode = 0;
        let mut next = record(2, 72);
        next.link_a.mode = 3;
        let mut engine = started(&[anim, next]);
        let mut world = W {
            action_result: Some(ActionResult::Handle(8)),
            ..W::default()
        };
        // Class 0 of the tail table: done flag set in the tail pass of the same frame.
        assert_eq!(
            frames(&mut engine, &mut world, 2),
            vec![1, 2],
            "verb {verb}"
        );
    }
}

#[test]
fn object_anim_verbs_wait_for_the_world_and_a_missing_object_is_done() {
    let mut anim = arg_pairs(4, &[(0x14, 0x31), (0x18, 0x0003_0002)], 0);
    anim.callback = 12;
    let mut engine = started(&[anim]);
    let mut world = W {
        action_result: Some(ActionResult::Handle(8)),
        ..W::default()
    };
    frames(&mut engine, &mut world, 2);
    assert_eq!(world.log, vec!["callback 12 Object(8)"]);
    assert_eq!(
        world.waits_seen.last(),
        Some(&Wait::ObjectAnim {
            handle: 8,
            state: 2
        })
    );
    assert!(!engine.is_done(0));
    world.all_waits_done = true;
    frames(&mut engine, &mut world, 1);
    assert!(engine.is_done(0));
    // Missing object: done without asking the wait.
    let mut engine = started(&[arg_pairs(7, &[(0x14, 0x31)], 0)]);
    let mut world = W {
        action_result: Some(ActionResult::Handle(9)),
        ..W::default()
    };
    world.object_missing.insert(9);
    frames(&mut engine, &mut world, 1);
    assert!(engine.is_done(0) && world.waits_seen.is_empty());
}

#[test]
fn tick_callback_runs_each_tail_pass_for_verbs_except_3() {
    let mut anim = arg_pairs(4, &[(0x14, 1)], 0);
    anim.tick_callback = 3;
    let mut engine = started(&[anim]);
    let mut world = W {
        action_result: Some(ActionResult::Handle(8)),
        ..W::default()
    };
    frames(&mut engine, &mut world, 3);
    let ticks = world
        .log
        .iter()
        .filter(|l| l.starts_with("callback 3"))
        .count();
    assert_eq!(ticks, 3);
    // A vanished object clears the handle.
    world.object_missing.insert(8);
    frames(&mut engine, &mut world, 1);
    assert_eq!(engine.instance(0).object_handle, None);
}

#[test]
fn verb_3_lifetime_missing_object_and_kept_object() {
    let mut spawn = arg_pairs(3, &[(0x10, 42)], 0);
    spawn.link_a.mode = 0;
    let mut engine = started(&[spawn]);
    let mut world = W::default();
    frames(&mut engine, &mut world, 1);
    world.object_missing.insert(1);
    frames(&mut engine, &mut world, 1);
    assert!(
        engine.is_done(0),
        "object vanished: done (MEASURED 0x2379C3)"
    );
    assert!(!world.log.iter().any(|l| l.starts_with("destroy")));
    // Kept object stays referenced after it finished.
    spawn.flags = 0x10;
    let mut engine = started(&[spawn]);
    let mut world = W {
        object_ready: true,
        ..W::default()
    };
    frames(&mut engine, &mut world, 2);
    assert!(engine.is_done(0) && engine.instance(0).object_handle.is_some());
}

#[test]
fn sound_verbs_complete_when_the_sound_stops() {
    let mut sound = arg_pairs(11, &[(0xC, 5)], 0);
    sound.link_a.mode = 0;
    let mut engine = started(&[sound]);
    let mut world = W {
        action_result: Some(ActionResult::Handle(3)),
        ..W::default()
    };
    frames(&mut engine, &mut world, 2);
    assert!(!engine.is_done(0));
    world
        .waits_done
        .insert(format!("{:?}", Wait::SoundFinished { handle: 3 }));
    frames(&mut engine, &mut world, 1);
    assert!(engine.is_done(0));
    // Verb 10 with sound id -1 has nothing to wait for.
    let mut effect = arg_pairs(10, &[(0xC, NONE), (0x14, 2)], 0);
    effect.link_a.mode = 0;
    let mut engine = started(&[effect]);
    let mut world = W {
        action_result: Some(ActionResult::Done),
        ..W::default()
    };
    frames(&mut engine, &mut world, 1);
    assert!(engine.is_done(0));
}

#[test]
fn verb_10_is_a_periodic_emitter_while_the_room_is_active() {
    let mut effect = arg_pairs(10, &[(0xC, 5), (0x14, 2)], 0);
    effect.link_a.mode = 0;
    effect.weight_bits = 30.0f32.to_bits(); // 30 per second, p = 3 per 0.1 s frame -> always
    let mut engine = started(&[effect]);
    let mut world = W {
        action_result: Some(ActionResult::Handle(3)),
        all_waits_done: true,
        room_active: true,
        random: 0.5,
        ..W::default()
    };
    frames(&mut engine, &mut world, 4);
    assert!(
        engine.trace(0).executions >= 3,
        "re-triggered: {}",
        engine.trace(0).executions
    );
    // Delay reloads with 10 s and the link word is cleared on every tail pass.
    assert!((engine.instance(0).delay - 10.0).abs() < 1e-3);
    // Room inactive or probability not met: no more runs.
    let runs = engine.trace(0).executions;
    world.room_active = false;
    frames(&mut engine, &mut world, 3);
    assert_eq!(engine.trace(0).executions, runs);
    world.room_active = true;
    world.random = 0.0;
    effect.weight_bits = 0.5f32.to_bits(); // threshold 0.95, u = 0 < threshold
    let mut quiet = started(&[effect]);
    frames(&mut quiet, &mut world, 4);
    assert_eq!(quiet.trace(0).executions, 1);
}

#[test]
fn verb_10_never_reports_done_to_the_state_pass_while_it_ticks() {
    // The tail reloads the delay with 10 s on every pass (`0x237A6C`), so the state pass returns at
    // the delay test before it reaches the done test: the mode-3 successor and the completion
    // burst helper `0x233BD0` are unreachable for a ticking verb 10 (MEASURED reading, so the
    // successor below never runs even though the sound finished).
    let mut effect = arg_pairs(10, &[(0xC, 5), (0x10, 9), (0x14, 2), (0x18, 3)], 0);
    effect.callback = 11;
    effect.weight_bits = 0f32.to_bits();
    let mut next = record(2, 72);
    next.link_a.mode = 3;
    let mut engine = started(&[effect, next]);
    let mut world = W {
        action_result: Some(ActionResult::Handle(3)),
        room_active: true,
        all_waits_done: true,
        ..W::default()
    };
    assert_eq!(frames(&mut engine, &mut world, 6), vec![1]);
    assert!(engine.instance(0).world_done, "the sound itself finished");
    assert!(engine.is_done(0), "the live done test is true");
    assert!(
        world
            .actions
            .iter()
            .all(|a| !matches!(a, Action::EffectBurst { .. }))
    );
    assert!((engine.instance(0).delay - 10.0).abs() < 1e-3 + 0.1);
}

#[test]
fn world_wait_verbs_set_done_from_the_world() {
    let cases: [(u32, &[(usize, u32)], &str); 9] = [
        (24, &[(0x18, 6)], "MovieFinished { index: 6 }"),
        (25, &[(0x18, 0x9B5)], "CameraFlag"),
        (26, &[(0x10, 4)], "MissionStage { stage: 4 }"),
        (
            49,
            &[(0x14, 1), (0xC, 2), (0x10, 3)],
            "ItemGive { object: 1, value: 2, player: 3 }",
        ),
        (
            50,
            &[(0x14, 1), (0x10, 3)],
            "ItemTake { object: 1, player: 3 }",
        ),
        (60, &[(0xC, 8)], "CameraReached { camera: 8 }"),
        (61, &[(0xC, 8)], "CameraTargetReached { camera: 8 }"),
        (62, &[(0x14, 12)], "PropCheck { key: 12 }"),
        (
            16,
            &[(0x14, 12), (0xC, 1)],
            "PropState { key: 12, activate: true }",
        ),
    ];
    for (verb, args, wait) in cases {
        let r = arg_pairs(verb, args, 0);
        let mut engine = started(&[r]);
        let mut world = W {
            action_result: Some(ActionResult::Done),
            ..W::default()
        };
        frames(&mut engine, &mut world, 2);
        assert!(!engine.is_done(0), "verb {verb} waits");
        world.waits_done.insert(wait.to_string());
        world.all_waits_done = verb == 16; // prop_settled is the legacy path for 16
        frames(&mut engine, &mut world, 1);
        assert!(engine.is_done(0), "verb {verb} done after {wait}");
    }
}

#[test]
fn verb_28_timed_view_waits_with_its_parameters() {
    let r = arg_pairs(
        28,
        &[(0xC, 1), (0x10, 2.5f32.to_bits()), (0x14, 0.5f32.to_bits())],
        0,
    );
    let mut engine = started(&[r]);
    let mut world = W {
        action_result: Some(ActionResult::Done),
        ..W::default()
    };
    frames(&mut engine, &mut world, 1);
    assert_eq!(
        world.waits_seen[0],
        Wait::TimedView {
            mode: 1,
            seconds: 2.5,
            fade: 0.5
        }
    );
}

#[test]
fn verb_27_requests_the_checkpoint_when_it_fires() {
    let mut save = record(1, 27);
    save.delay_bits = 0.15f32.to_bits();
    let mut engine = started(&[save]);
    let mut world = W::default();
    let mut checkpoints = Vec::new();
    for _ in 0..4 {
        checkpoints.extend(engine.step(0.1, &mut world).checkpoints);
    }
    assert_eq!(
        checkpoints,
        vec![0],
        "requested at fire time, not at execution"
    );
    assert_eq!(world.checkpoints, 1);
    let mut engine = started(&[record(9, 27)]);
    engine.fire_by_id(9, &mut world);
    assert_eq!(engine.take_side_report().checkpoints, vec![0]);
}

#[test]
fn verb_56_resets_listed_instances() {
    let mut node = record(10, 72);
    node.link_a = Link {
        mode: 5,
        id: 1,
        args: [NONE; 4],
    };
    node.flags = 0;
    let mut reset = record(20, 56);
    for (i, offset) in [0xC, 0x10, 0x14, 0x18, 0x1C].into_iter().enumerate() {
        set_arg(&mut reset, offset, if i == 0 { 10 } else { 0 });
    }
    let mut engine = started(&[node, reset]);
    let mut world = W::default();
    engine.finish_slot(0x2D, &mut world);
    assert_eq!(frames(&mut engine, &mut world, 1), vec![10, 20]);
    let instance = engine.instance(0);
    assert_eq!(
        (
            instance.executed,
            instance.running,
            instance.fired,
            instance.a_set
        ),
        (0, 0, false, false)
    );
    assert_eq!(
        engine.slots().status(0x2D),
        STATUS_ACTIVE,
        "slot re-activated (0x227710)"
    );
}

#[test]
fn verb_84_releases_voices_and_targets() {
    let mut voice = record(1, 1);
    set_arg(&mut voice, 0x10, 7);
    set_arg(&mut voice, 0x14, NONE);
    let mut release = record(2, 84);
    for (i, offset) in [0xC, 0x10, 0x14, 0x18].into_iter().enumerate() {
        set_arg(&mut release, offset, [0x3F3F_3F3F, 0x77, 0, 0][i]);
    }
    let mut engine = started(&[voice, release]);
    let mut world = W {
        voice_len: 50,
        ..W::default()
    };
    frames(&mut engine, &mut world, 2);
    assert_eq!(world.stopped, vec![1]);
    assert_eq!(world.cleared, vec![0x77]);
    assert_eq!(engine.instance(0).sound_handle, None);
}

#[test]
fn verb_88_ends_the_match() {
    let mut engine = started(&[record(1, 88)]);
    let mut world = W::default();
    frames(&mut engine, &mut world, 1);
    assert_eq!(engine.session_flags() & SESSION_COMPLETE, SESSION_COMPLETE);
    assert!(engine.mission_complete());
    assert_eq!(world.matches_ended, 1);
}

#[test]
fn mission_events_reach_the_world_through_the_slot_update() {
    let mut engine = started(&[]);
    engine.slots_mut().create(&ctx(), 0, 0, [0, 0, 0], 0x700);
    let mut world = W::default();
    engine.step(0.1, &mut world);
    assert!(world.missions.is_empty());
    engine.slots_mut().complete_quiet(0);
    engine.step(0.1, &mut world);
    assert_eq!(world.missions, vec![MissionEvent::Complete]);
    assert_eq!(world.matches_ended, 1);
    assert_eq!(engine.session_flags(), SESSION_COMPLETE);
    engine.step(0.1, &mut world);
    assert_eq!(world.missions.len(), 1, "reported once");
    let mut engine = started(&[]);
    let mut world = W {
        down: true,
        ..W::default()
    };
    engine.step(0.1, &mut world);
    assert_eq!(world.missions, vec![MissionEvent::AllPlayersDown]);
}

// ---------------------------------------------------------------- snapshot / restore

#[test]
fn snapshot_layout_and_bytes() {
    let mut a = record(1, 72);
    a.restore_id = 3;
    let mut b = record(2, 3);
    b.link_a.mode = 0;
    let mut engine = started(&[a, b]);
    let mut world = W::default();
    world.restore_words.insert(3, 0xDEAD);
    frames(&mut engine, &mut world, 1);
    world.object_words.insert(1, (0x11, 0x22));
    let snapshot = engine.capture(&mut world);
    assert_eq!(snapshot.entries.len(), 2);
    // Event 1 ran and completed: fired cleared (byte 3 = 0), executed 1 (byte 2), link words 0.
    assert_eq!(snapshot.entries[0][0], 0x0001_0000);
    assert_eq!(snapshot.entries[0][1], 0xDEAD);
    // Event 2 (verb 3) holds the object extension words.
    assert_eq!(&snapshot.entries[1][2..], &[0x11, 0x22]);
    let bytes = snapshot.to_bytes();
    assert_eq!(bytes.len(), 32);
    assert_eq!(EventSnapshot::from_bytes(&bytes).unwrap(), snapshot);
    assert!(EventSnapshot::from_bytes(&bytes[..31]).is_err());
}

#[test]
fn restore_marks_done_reexecutes_and_applies_rules() {
    // 0: plain executed verb -> done. 1: repeatable executed verb -> not marked. 2: flag 0x1000 -> re-executed.
    // 3: verb 27 -> re-executed. 4: verb 43 that had not fired -> reset. 5: restore function.
    let mut plain = record(1, 72);
    plain.link_a.mode = 5;
    let mut repeat = record(2, 72);
    repeat.link_a.mode = 5;
    repeat.flags = 0x20;
    let mut again = record(3, 72);
    again.link_a.mode = 5;
    again.flags = 0x1000;
    let mut save = record(4, 27);
    save.link_a.mode = 5;
    let mut placed = record(5, 43);
    placed.link_a.mode = 5;
    let mut restorer = record(6, 0);
    restorer.link_a.mode = 5;
    restorer.restore_id = 3;
    let mut engine = started(&[plain, repeat, again, save, placed, restorer]);
    let mut world = W::default();
    let pack =
        |fired: u32, executed: u32, a: u32, b: u32| (fired << 24) | (executed << 16) | (a << 8) | b;
    let snapshot = EventSnapshot {
        entries: vec![
            [pack(1, 1, 1, 0), 0, 0, 0],
            [pack(1, 1, 1, 0), 0, 0, 0],
            [pack(1, 1, 1, 0), 0, 0, 0],
            [pack(1, 1, 1, 0), 0, 0, 0],
            [pack(0, 0, 1, 0), 0, 0, 0],
            [pack(0, 0, 0, 0), 0x55, 0, 0],
        ],
    };
    engine.restore(&snapshot, &mut world);
    assert!(engine.instance(0).done && !engine.instance(1).done);
    assert_eq!(engine.instance(0).executed, 1);
    assert!(engine.instance(0).a_set && !engine.instance(0).b_set && engine.instance(0).fired);
    let side = engine.take_side_report();
    let rerun: Vec<u32> = side.fired.iter().map(|f| f.event_id).collect();
    assert_eq!(rerun, vec![3, 4], "flag 0x1000 and verb 27 re-execute");
    assert_eq!(engine.instance(2).running, 1);
    assert!(world.log.contains(&"restore_apply 3 0x55".to_string()));
    // Verb 43 that had not fired is reset: link words cleared.
    assert!(!engine.instance(4).a_set);
}

#[test]
fn restore_reloads_the_delay_of_a_fired_but_not_executed_instance() {
    let mut r = record(1, 72);
    r.link_a.mode = 5;
    r.delay_bits = 2.5f32.to_bits();
    let mut engine = started(&[r]);
    let snapshot = EventSnapshot {
        entries: vec![[1 << 24, 0, 0, 0]],
    };
    engine.restore(&snapshot, &mut W::default());
    assert_eq!(engine.instance(0).delay, 2.5);
    assert!(engine.instance(0).fired && engine.instance(0).executed == 0);
}

#[test]
fn restore_mode_19_compares_the_stage() {
    let mut r = record(1, 72);
    r.link_a = Link {
        mode: 19,
        id: 0,
        args: [4, NONE, NONE, NONE],
    };
    r.combine = 1;
    r.link_b = Link {
        mode: 19,
        id: 0,
        args: [5, NONE, NONE, NONE],
    };
    let snapshot = EventSnapshot {
        entries: vec![[0; 4]],
    };
    for (stage, a, b) in [(4, true, false), (5, false, true), (9, false, false)] {
        let mut engine = started(&[r]);
        let mut world = W {
            stage,
            ..W::default()
        };
        engine.restore(&snapshot, &mut world);
        assert_eq!(
            (engine.instance(0).a_set, engine.instance(0).b_set),
            (a, b),
            "stage {stage}"
        );
    }
}

#[test]
fn restore_recreates_persistent_objects_or_defers_them() {
    let mut keep = arg_pairs(3, &[(0x10, 42), (0x18, 7)], 0x10);
    keep.link_a.mode = 5;
    let snapshot = EventSnapshot {
        entries: vec![[(1 << 24) | (1 << 16), 0, 0x11, 0x22]],
    };
    let mut engine = started(&[keep]);
    let mut world = W {
        persistent_ready: true,
        ..W::default()
    };
    engine.restore(&snapshot, &mut world);
    assert_eq!(world.log, vec!["spawn 42"]);
    assert_eq!(world.set_words, vec![(1, 0x11, 0x22)]);
    // Asset not ready: nothing spawns, the instance waits in the pending list.
    let mut engine = started(&[keep]);
    let mut world = W::default();
    engine.restore(&snapshot, &mut world);
    assert!(world.log.is_empty() && engine.instance(0).running == 0);
    assert_eq!(engine.apply_pending_restore(&mut world), 0);
    world.persistent_ready = true;
    assert_eq!(engine.apply_pending_restore(&mut world), 1);
    assert_eq!(world.set_words, vec![(1, 0x11, 0x22)]);
    assert_eq!(
        engine.apply_pending_restore(&mut world),
        0,
        "list was drained"
    );
}

#[test]
fn capture_then_restore_reproduces_the_state() {
    let mut chain = [record(1, 72), record(2, 72), record(3, 72)];
    chain[0].link_a.mode = 0;
    chain[1].link_a.mode = 3;
    chain[2].link_a.mode = 3;
    chain[2].delay_bits = 1.0f32.to_bits();
    let mut engine = started(&chain);
    let mut world = W::default();
    frames(&mut engine, &mut world, 3);
    let snapshot = engine.capture(&mut world);
    // Fresh engine, restore: executed ones are done, the waiting one keeps its fired state.
    let mut fresh = started(&chain);
    fresh.restore(&snapshot, &mut world);
    for i in 0..3 {
        assert_eq!(
            fresh.instance(i).executed,
            engine.instance(i).executed,
            "#{i}"
        );
        assert_eq!(fresh.instance(i).fired, engine.instance(i).fired, "#{i}");
        assert_eq!(fresh.instance(i).a_set, engine.instance(i).a_set, "#{i}");
    }
    assert_eq!(fresh.capture(&mut world), snapshot);
}

// ---------------------------------------------------------------- real data (optional)

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

#[test]
fn real_slot_tables_parse_and_start_as_decoded() {
    let Some(blobs) = real_blobs() else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let (mut entries, mut pending, mut active, mut tables) = (0usize, 0usize, 0usize, 0usize);
    for (name, blob) in &blobs {
        let defs = slots::parse_slot_defs(blob).unwrap_or_else(|e| panic!("{name}: {e}"));
        let events = parse_events(blob).unwrap();
        let mut engine = Engine::new(&events, BuildOptions::default());
        engine.load_static_slots(&defs);
        tables += 1;
        for def in &defs {
            entries += 1;
            assert!(
                def.id < slots::STATIC_SLOT_COUNT,
                "{name}: slot id {} is not a static slot",
                def.id
            );
            match engine.slots().status(def.id) {
                STATUS_PENDING => pending += 1,
                STATUS_ACTIVE => active += 1,
                _ => {}
            }
        }
        // Event slot ids never collide with static ones.
        for (id, _, _) in engine.slots().event_link_slots() {
            assert!(id >= slots::STATIC_SLOT_COUNT, "{name}");
        }
    }
    eprintln!(
        "slot tables: {tables} entries parsed, {entries} slot definitions, {pending} start pending, {active} start active"
    );
    assert!(entries > 0);
}

/// Game configuration of a pak by its category (the disc paths are `story`, `arcade`, `challenge`).
/// Arcade masks (0x80000001) select mode id 3, challenge masks (0xE000000) only pass the descriptor
/// path of `0x6B100` (game flag 0x8000000), story masks (0x700 family) mode id 10. MEASURED by
/// counting which definitions the predicate keeps, see `real_modes_keep_the_right_definitions`.
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
fn real_modes_keep_the_right_definitions() {
    let Some(blobs) = real_blobs() else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let (mut arcade, mut challenge, mut story_one, mut story_two, mut arcade_in_story) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    for (name, blob) in &blobs {
        let events = parse_events(blob).unwrap();
        let build = |players| {
            Engine::new(
                &events,
                BuildOptions {
                    context: context_for(name, players),
                },
            )
        };
        if name.contains("__arcade__") || name.contains("__challenge__") {
            let engine = build(1);
            assert_eq!(
                engine.dropped(),
                0,
                "{name}: every non-story definition is kept in its own mode"
            );
            if name.contains("__arcade__") {
                arcade += engine.len();
            } else {
                challenge += engine.len();
            }
            // The Story context drops the arcade-only masks (0x80000001, 0x1, 0x7f).
            if name.contains("__arcade__") {
                arcade_in_story += Engine::new(&events, BuildOptions::default()).len();
            }
            continue;
        }
        let (one, two) = (build(1), build(2));
        story_one += one.len();
        story_two += two.len();
        for (index, event) in events.iter().enumerate() {
            let in_one = one.record_kept(index);
            let in_two = two.record_kept(index);
            match event.mode_mask {
                0x200700 => assert!(in_one && !in_two, "{name} #{index}: one-player mask"),
                0x400700 => assert!(!in_one && in_two, "{name} #{index}: two-player mask"),
                0x700 => assert!(in_one && in_two, "{name} #{index}: all-player mask"),
                _ => {}
            }
        }
    }
    eprintln!(
        "kept: arcade {arcade}, challenge {challenge}, story 1p {story_one}, story 2p {story_two}"
    );
    assert_eq!(arcade, 91, "MEASURED: 91 arcade definitions (T1838)");
    assert!(
        arcade_in_story < 10,
        "story context kept {arcade_in_story} arcade definitions"
    );
    assert!(
        story_one < story_two + 200 && story_one > 4500,
        "player-count masks split the story set"
    );
    assert_eq!(challenge, 60, "MEASURED: 60 challenge definitions (T1838)");
}

/// Replay world that models every action and answers every query (FABRICATED scenario).
#[derive(Default)]
struct Everything {
    handle: u32,
    conditions: HashSet<u32>,
}

impl World for Everything {
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
    fn condition_query(&mut self, query: &ConditionQuery) -> Option<bool> {
        if let ConditionQuery::Scene { kind, .. } = query {
            self.conditions.insert(*kind);
        }
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
fn real_levels_run_every_disc_verb_with_a_complete_world() {
    let Some(blobs) = real_blobs() else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let mut disc_verbs: HashSet<u32> = HashSet::new();
    let mut ran: HashSet<u32> = HashSet::new();
    let (mut executions, mut errors, mut unsupported_verbs) = (0usize, 0usize, HashSet::new());
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
            for _ in 0..600 {
                let report = engine.step(0.1, &mut world);
                for fired in &report.fired {
                    ran.insert(fired.verb);
                }
                executions += report.fired.len();
                for (_, error) in &report.errors {
                    errors += 1;
                    if let EngineError::Unsupported {
                        what: "verb",
                        value,
                    } = error
                    {
                        unsupported_verbs.insert(*value);
                    }
                }
            }
            // Snapshot round trip on whatever state the replay reached.
            let snapshot = engine.capture(&mut world);
            assert_eq!(snapshot.entries.len(), engine.len(), "{name}");
            assert_eq!(
                EventSnapshot::from_bytes(&snapshot.to_bytes()).unwrap(),
                snapshot,
                "{name}"
            );
            let mut again = Engine::new(
                &events,
                BuildOptions {
                    context: context_for(name, players),
                },
            );
            again.set_checkpoint_mask(u32::MAX);
            again.restore(&snapshot, &mut world);
            assert_eq!(
                again.capture(&mut world).entries.len(),
                snapshot.entries.len(),
                "{name}"
            );
        }
    }
    let mut missing: Vec<_> = disc_verbs.difference(&ran).copied().collect();
    missing.sort_unstable();
    eprintln!(
        "disc verbs {}, executed {}, executions {executions}, errors {errors}, never executed: {missing:?}",
        disc_verbs.len(),
        ran.len()
    );
    assert!(
        unsupported_verbs.is_empty(),
        "verbs reported Unsupported with a complete world: {unsupported_verbs:?}"
    );
    assert!(
        ran.len() >= 60,
        "only {} distinct verbs executed",
        ran.len()
    );
}
