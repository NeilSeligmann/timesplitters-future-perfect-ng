//! Instance pool and trigger engine (spec sections 2, 4 and 5). Per frame: state pass
//! (`0x237E70`) over the active list, tail pass (`0x237920`), then the slot update (`0x228870`),
//! the order the game loop uses (`0x2382A0` before `0x228870`).
//!
//! Instance fields follow the 0x48-byte layout of spec section 2; pointers become indices. In
//! T1841 the state pass, the tail pass and the pool builder were re-ported literally from the
//! instructions, correcting several T1840 readings (see docs/t-story-event-engine.md section 6).

use std::collections::VecDeque;
use std::fmt;

use super::callbacks::{self, CallbackContext, CallbackGap, SlotOp, Step};
use super::condition;
use super::mode::ModeContext;
use super::record::{EventRecord, Link};
use super::slots::{
    SESSION_COMPLETE, STATIC_SLOT_COUNT, SlotCallback, SlotChange, SlotDef, SlotManager,
    event_slot_mask,
};
use super::snapshot::EventSnapshot;
use super::verbs;
use super::world::{CallbackTarget, World};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// A condition kind or verb the world does not model. Never guessed.
    Unsupported { what: &'static str, value: u32 },
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { what, value } => write!(f, "unsupported {what} {value}"),
        }
    }
}

impl std::error::Error for EngineError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSide {
    A,
    B,
}

/// How a link mode is satisfied (spec 4.2, decoded from the builder `0x235630`, the callback
/// `0x235240` and the notifier `0x2351D0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkClass {
    /// Mode 0: no link, counts as satisfied. A slot is still created.
    None,
    /// Mode 2 / 3: previous definition in pool order ran / is done. No slot.
    PreviousRan,
    PreviousDone,
    /// Mode 4: the instance whose event id equals the link id has executed (slot polled).
    Instance,
    /// Mode 9: slot `args[0]` has status 5 (slot polled).
    SlotStatus,
    /// Modes 1, 6, 8, 10, 11, 15, 16, 17, 18: polled through [`World::poll_link`].
    Polled,
    /// Modes 5, 12, 13, 19, 20 and unused modes: slot with no poll handler, satisfied only by an
    /// external slot finish, a key signal (5, 12, 13) or the restore rule (19).
    External,
}

fn link_class(mode: u32) -> LinkClass {
    match mode {
        0 => LinkClass::None,
        2 => LinkClass::PreviousRan,
        3 => LinkClass::PreviousDone,
        4 => LinkClass::Instance,
        9 => LinkClass::SlotStatus,
        1 | 6 | 8 | 10 | 11 | 15..=18 => LinkClass::Polled,
        _ => LinkClass::External,
    }
}

/// Link modes 1, 6, 7 and 15 are set again at once when a repeatable event re-arms (`0x233EC0`).
fn rearm_sets_again(mode: u32) -> bool {
    matches!(mode, 1 | 6 | 7 | 15)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RearmInput {
    link_a_mode: u32,
    link_b_mode: u32,
    has_link_b: bool,
    slot_a: Option<u32>,
    slot_b: Option<u32>,
    delay: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RearmTransition {
    a_set: bool,
    b_set: bool,
    reactivate_a: Option<u32>,
    reactivate_b: Option<u32>,
    running: u8,
    done: bool,
    executed: u8,
    delay: f32,
}

/// Typed Rust-owned transition for repeatable definitions (`0x233E80`).
fn rearm_transition(input: RearmInput) -> RearmTransition {
    RearmTransition {
        a_set: rearm_sets_again(input.link_a_mode),
        b_set: input.has_link_b && rearm_sets_again(input.link_b_mode),
        reactivate_a: input.slot_a,
        reactivate_b: input.has_link_b.then_some(input.slot_b).flatten(),
        running: 0,
        done: false,
        executed: 2,
        delay: input.delay,
    }
}

fn rust_rearm_enabled_from(value: Option<&str>) -> bool {
    value == Some("rust")
}

fn rust_rearm_enabled() -> bool {
    rust_rearm_enabled_from(std::env::var("TSFP_STORY_EVENT_REARM").ok().as_deref())
}

#[derive(Debug, Clone, Copy)]
pub struct BuildOptions {
    /// Game configuration for the build predicate `0x6B2B0` (decoded in T1841, replaces the
    /// FABRICATED `mode_mask` AND of T1840).
    pub context: ModeContext,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            context: ModeContext::default(),
        }
    }
}

impl BuildOptions {
    /// Story mode with the given difficulty (0 to 2) and sub-mode (0 to 2).
    pub fn story(difficulty: u8, submode: u8) -> Self {
        Self {
            context: ModeContext::story(difficulty, submode),
        }
    }
}

/// One pool instance (spec section 2, 0x48 bytes) plus engine bookkeeping.
#[derive(Debug, Clone, Default)]
pub struct Instance {
    /// Index of the definition in the file (before the build-predicate drop).
    pub source_index: usize,
    pub link_a_target: Option<usize>,
    pub link_b_target: Option<usize>,
    /// Slot ids of link A and B (+0x0C, +0x18), `None` = -1.
    pub slot_a: Option<u32>,
    pub slot_b: Option<u32>,
    pub a_set: bool,
    pub b_set: bool,
    pub done: bool,
    pub sound_handle: Option<u32>,
    pub object_handle: Option<u32>,
    pub gate: Option<usize>,
    pub fired: bool,
    /// 1 running, 0 idle, 2 suppressed (queued voice). Stays 1 after completion until re-armed.
    pub running: u8,
    /// 1 executed at least once, 2 re-arm marker.
    pub executed: u8,
    /// +0x38, the delay countdown. Voice lines read it as the subtitle timer.
    pub delay: f32,
    pub checkpoint_mask: u32,
    /// Verb 1 only: waiting in the voice queue (`0x234C00`).
    pub queued: bool,
    /// Cached live done test of the world-wait verbs (10, 11, 24, 25, 26, 62), refreshed by the
    /// tail pass and cleared when the verb runs again.
    pub world_done: bool,
}

/// First-time stamps in seconds of simulated time, for replay assertions (not in the original).
#[derive(Debug, Clone, Copy, Default)]
pub struct Trace {
    pub fired_at: Option<f32>,
    pub executed_at: Option<f32>,
    pub done_at: Option<f32>,
    pub executions: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fired {
    pub instance: usize,
    pub event_id: u32,
    pub verb: u32,
}

#[derive(Debug, Default)]
pub struct StepReport {
    /// Verbs executed this frame in execution order.
    pub fired: Vec<Fired>,
    /// Per-instance unsupported items hit this frame; the instance stays idle.
    pub errors: Vec<(usize, EngineError)>,
    /// Verb 27 instances fired this frame (checkpoint capture requests).
    pub checkpoints: Vec<usize>,
}

pub struct Engine {
    pub(crate) records: Vec<EventRecord>,
    pub(crate) instances: Vec<Instance>,
    traces: Vec<Trace>,
    active: Vec<usize>,
    checkpoint: u32,
    pub(crate) context: ModeContext,
    elapsed: f32,
    pub(crate) last_dt: f32,
    pub(crate) voice_queue: VecDeque<usize>,
    dropped: usize,
    pub(crate) slots: SlotManager,
    /// `[0x7DE45C]`: bit 0x8000 mission complete / match end, 0x10000 failed.
    pub(crate) session_flags: u32,
    /// `[0x78CE20]`: instances whose link A mode is 5, 12 or 13, in pool order.
    key_links: Vec<usize>,
    /// Pending restores (`0x78CDA0`): instance and the two object words to restore.
    pub(crate) pending_restore: Vec<(usize, u32, u32)>,
    /// Reports collected outside `step` (fire_by_id, restore).
    pub(crate) side_report: StepReport,
    /// Callback ids that ran but could not be decoded (see [`callbacks::CallbackGap`]), once each.
    callback_gaps: Vec<CallbackGap>,
    /// Callback 98 state `[0x75C094..0x75C09C]`: slot, frame stamp and value.
    timed_slot: Option<(u32, u32, f32)>,
}

impl Engine {
    pub fn new(definitions: &[EventRecord], options: BuildOptions) -> Self {
        let context = options.context;
        let kept: Vec<(usize, EventRecord)> = definitions
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, record)| {
                super::levels::pool_size(context.level_id, 1) != 0
                    && context.build_predicate(record.mode_mask)
            })
            .collect();
        let records: Vec<EventRecord> = kept.iter().map(|(_, record)| *record).collect();
        let find = |id: u32, own: usize| {
            records
                .iter()
                .enumerate()
                .position(|(i, r)| i != own && r.event_id == id)
        };
        let instances = kept
            .iter()
            .enumerate()
            .map(|(index, (source_index, record))| Instance {
                source_index: *source_index,
                link_a_target: (record.link_a.mode == 4)
                    .then(|| find(record.link_a.id, index))
                    .flatten(),
                link_b_target: (record.link_b.mode == 4)
                    .then(|| find(record.link_b.id, index))
                    .flatten(),
                gate: (record.gate_mode != 0)
                    .then(|| find(record.gate_id, index))
                    .flatten(),
                checkpoint_mask: record.checkpoint_mask,
                ..Instance::default()
            })
            .collect::<Vec<_>>();
        let mut engine = Self {
            traces: vec![Trace::default(); instances.len()],
            dropped: definitions.len() - records.len(),
            key_links: records
                .iter()
                .enumerate()
                .filter(|(_, r)| matches!(r.link_a.mode, 5 | 12 | 13))
                .map(|(i, _)| i)
                .collect(),
            records,
            instances,
            active: Vec::new(),
            checkpoint: 0,
            context,
            elapsed: 0.0,
            last_dt: 0.0,
            voice_queue: VecDeque::new(),
            slots: SlotManager::new(),
            session_flags: 0,
            pending_restore: Vec::new(),
            side_report: StepReport::default(),
            callback_gaps: Vec::new(),
            timed_slot: None,
        };
        engine.build_event_slots();
        engine
    }

    /// Second loop of `0x235630`: create the dependency slots of every instance. Slot ids count up
    /// from 0x2D. Link A always consumes an id (modes 2 and 3 without creating a slot), link B
    /// only when the combine operator is non-zero. Instances disabled on the difficulty get no
    /// slots and consume no id. A slot is active at creation.
    fn build_event_slots(&mut self) {
        let mut next_id = STATIC_SLOT_COUNT;
        for index in 0..self.instances.len() {
            let record = self.records[index];
            if !record.enabled_on_difficulty(self.context.difficulty) {
                continue;
            }
            let sides: &[(LinkSide, u32)] = if record.combine != 0 {
                &[
                    (LinkSide::A, record.link_a.mode),
                    (LinkSide::B, record.link_b.mode),
                ]
            } else {
                &[(LinkSide::A, record.link_a.mode)]
            };
            for &(side, mode) in sides {
                let id = next_id;
                next_id += 1;
                if matches!(mode, 2 | 3) {
                    continue;
                }
                self.slots
                    .create(&self.context, id, 0, [2, 2, 2], event_slot_mask());
                self.slots.set_callback(
                    id,
                    SlotCallback::EventLink {
                        instance: index,
                        side,
                    },
                );
                match side {
                    LinkSide::A => self.instances[index].slot_a = Some(id),
                    LinkSide::B => self.instances[index].slot_b = Some(id),
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Was definition `source_index` of the input list kept by the build predicate?
    pub fn record_kept(&self, source_index: usize) -> bool {
        self.instances
            .iter()
            .any(|instance| instance.source_index == source_index)
    }

    /// Definitions dropped by the build predicate at build time.
    pub fn dropped(&self) -> usize {
        self.dropped
    }

    pub fn instance(&self, index: usize) -> &Instance {
        &self.instances[index]
    }

    pub fn record(&self, index: usize) -> &EventRecord {
        &self.records[index]
    }

    pub fn trace(&self, index: usize) -> &Trace {
        &self.traces[index]
    }

    pub fn active(&self) -> &[usize] {
        &self.active
    }

    pub fn elapsed(&self) -> f32 {
        self.elapsed
    }

    pub fn context(&self) -> &ModeContext {
        &self.context
    }

    pub fn slots(&self) -> &SlotManager {
        &self.slots
    }

    pub fn slots_mut(&mut self) -> &mut SlotManager {
        &mut self.slots
    }

    /// `[0x7DE45C]` session flags (0x8000 complete, 0x10000 failed).
    pub fn session_flags(&self) -> u32 {
        self.session_flags
    }

    pub fn set_session_flags(&mut self, flags: u32) {
        self.session_flags = flags;
    }

    /// Level start (`0x228180` then `0x227E90`): replace the static slots with the level's table.
    /// Event slots (ids from 0x2D) are kept, they were created at build time.
    pub fn load_static_slots(&mut self, defs: &[SlotDef]) {
        let events: Vec<_> = self
            .slots
            .event_link_slots()
            .into_iter()
            .map(|(id, instance, side)| (id, instance, side))
            .collect();
        self.slots.load_level(&self.context, defs);
        for (id, instance, side) in events {
            self.slots
                .create(&self.context, id, 0, [2, 2, 2], event_slot_mask());
            self.slots
                .set_callback(id, SlotCallback::EventLink { instance, side });
        }
    }

    /// `0x2378C0` -> `0x235FC0(mask)`: rebuild the active list (spec 5.1).
    pub fn set_checkpoint_mask(&mut self, mask: u32) {
        self.checkpoint = mask;
        let difficulty = self.context.difficulty;
        self.active = (0..self.instances.len())
            .filter(|&i| {
                let instance = &self.instances[i];
                (instance.checkpoint_mask & mask != 0
                    && self.records[i].enabled_on_difficulty(difficulty))
                    || instance.running != 0
            })
            .collect();
    }

    pub fn checkpoint_mask(&self) -> u32 {
        self.checkpoint
    }

    /// External slot event: satisfy link `side` of instance `index` (callback code 4 of `0x235240`).
    pub fn activate_link(&mut self, index: usize, side: LinkSide) {
        match side {
            LinkSide::A => self.instances[index].a_set = true,
            LinkSide::B => self.instances[index].b_set = true,
        }
    }

    /// Finish every event slot whose link has no poll handler (modes 5, 12, 13, 19, 20 and the
    /// unused modes) through the slot manager, as an external trigger (a key signal, a callback)
    /// would. Polled modes and mode 4 / 9 satisfy themselves in the slot update.
    pub fn activate_all_slot_links(&mut self, world: &mut dyn World) {
        for (id, instance, side) in self.slots.event_link_slots() {
            let mode = match side {
                LinkSide::A => self.records[instance].link_a.mode,
                LinkSide::B => self.records[instance].link_b.mode,
            };
            if link_class(mode) == LinkClass::External {
                self.finish_slot(id, world);
            }
        }
    }

    /// `0x2274C0`: finish a slot and run its callbacks (event link callbacks set the link).
    pub fn finish_slot(&mut self, id: u32, world: &mut dyn World) {
        let change = self.slots.finish(&self.context, id);
        self.apply_slot_change(change, world);
    }

    pub(crate) fn apply_slot_change(&mut self, change: SlotChange, world: &mut dyn World) {
        for event in &change.events {
            world.slot_event(event);
        }
        for (id, callback) in change.callbacks {
            match callback {
                SlotCallback::EventLink { instance, side } => self.activate_link(instance, side),
                SlotCallback::World(code) => world.slot_callback(id, code),
                SlotCallback::None => {}
            }
        }
    }

    /// `0x2344D0` / `0x2345B0` (mode 5) and `0x2345D0` (modes 12, 13): a keyed trigger. With
    /// `reset` (verb 57) matching instances are reset, otherwise the links are satisfied. Returns
    /// the number of instances hit.
    pub fn signal_key(&mut self, key: u32, reset: bool, modes: &[u32]) -> usize {
        let mut hits = 0;
        for index in self.key_links.clone() {
            let record = self.records[index];
            let hit_a = modes.contains(&record.link_a.mode) && record.link_a.id == key;
            let hit_b = record.combine != 0
                && modes.contains(&record.link_b.mode)
                && record.link_b.id == key;
            if !hit_a && !hit_b {
                continue;
            }
            hits += 1;
            if reset {
                self.reset_instance(index, None);
            } else {
                if hit_a {
                    self.instances[index].a_set = true;
                }
                if hit_b {
                    self.instances[index].b_set = true;
                }
            }
        }
        hits
    }

    /// `0x2341D0`: reset the dynamic state of an instance, release its sound and re-activate its slots.
    pub(crate) fn reset_instance(&mut self, index: usize, world: Option<&mut dyn World>) {
        let record = self.records[index];
        let instance = &mut self.instances[index];
        instance.fired = false;
        instance.running = 0;
        instance.executed = 0;
        instance.delay = 0.0;
        let stale_sound = (record.verb == 11)
            .then(|| instance.sound_handle.take())
            .flatten();
        if let (Some(handle), Some(world)) = (stale_sound, world) {
            world.stop_sound(handle);
        }
        let (slot_a, slot_b) = (instance.slot_a, instance.slot_b);
        let instance = &mut self.instances[index];
        instance.done = false;
        instance.a_set = false;
        if let Some(slot) = slot_a {
            self.slots.reactivate(slot);
        }
        if record.combine != 0 {
            self.instances[index].b_set = false;
            if let Some(slot) = slot_b {
                self.slots.reactivate(slot);
            }
        }
    }

    /// `0x234190`: first instance with this event id.
    pub fn find_by_id(&self, event_id: u32) -> Option<usize> {
        self.records.iter().position(|r| r.event_id == event_id)
    }

    /// `0x233ED0`: load the delay, mark fired and, for verb 27, request the checkpoint capture.
    pub(crate) fn fire(&mut self, index: usize, world: &mut dyn World, report: &mut StepReport) {
        let record = self.records[index];
        self.instances[index].delay = record.delay();
        self.instances[index].fired = true;
        self.traces[index].fired_at.get_or_insert(self.elapsed);
        if record.verb == 27 {
            report.checkpoints.push(index);
            world.checkpoint_fired();
        }
    }

    /// `0x2378D0(id)`: find the instance by event id, load its delay, clear running and run now.
    pub fn fire_by_id(&mut self, event_id: u32, world: &mut dyn World) -> Option<usize> {
        let index = self.find_by_id(event_id)?;
        let mut report = StepReport::default();
        self.fire(index, world, &mut report);
        self.instances[index].running = 0;
        self.execute(index, world, &mut report);
        self.side_report.fired.append(&mut report.fired);
        self.side_report.errors.append(&mut report.errors);
        self.side_report.checkpoints.append(&mut report.checkpoints);
        Some(index)
    }

    /// Reports produced outside `step` (`fire_by_id`, `restore`), drained by the caller.
    pub fn take_side_report(&mut self) -> StepReport {
        std::mem::take(&mut self.side_report)
    }

    /// `0x2279D0`: register a spawned actor of `class` (`actor+0x94`) with the static slots.
    pub fn register_actor(&mut self, actor: u32, class: u32) {
        self.slots.register_actor(&self.context, actor, class);
    }

    /// `0x227920`: an actor was destroyed.
    pub fn unregister_actor(&mut self, actor: u32) {
        self.slots.unregister_actor(&self.context, actor);
    }

    /// Callback ids that were run without a decoded body so far, each with its reason.
    pub fn callback_gaps(&self) -> &[CallbackGap] {
        &self.callback_gaps
    }

    /// Callback 98 state: `(slot, frame stamp, value)`.
    pub fn timed_slot(&self) -> Option<(u32, u32, f32)> {
        self.timed_slot
    }

    /// Run callback `id` (`R+0x38` or `R+0x3C` of instance `index`). The body is decoded by
    /// [`callbacks::decode`]: slot steps run here, world steps go to [`World::apply_callback`]
    /// and any step the world does not model also calls the legacy [`World::run_callback`] once.
    /// An id without a decoded body is recorded in [`Engine::callback_gaps`] and given to the
    /// legacy hook.
    pub(crate) fn dispatch_callback(
        &mut self,
        id: u32,
        target: CallbackTarget,
        index: usize,
        world: &mut dyn World,
    ) {
        if id == 0 {
            return;
        }
        let words = world.callback_words(target);
        let context = CallbackContext {
            record: self.records[index],
            executed: self.instances[index].executed != 0,
            words,
            class_254_actor: self.slots.tracked_actor(0, 0x254, 0),
        };
        let steps = match callbacks::decode(id, &context) {
            Ok(steps) => steps,
            Err(gap) => {
                if !self.callback_gaps.contains(&gap) {
                    self.callback_gaps.push(gap);
                }
                world.run_callback(id, target);
                return;
            }
        };
        let mut unmodelled = false;
        for step in steps {
            match step {
                Step::Slot(op) => self.apply_slot_op(op, world),
                Step::World(op) => unmodelled |= !world.apply_callback(&op, target),
            }
        }
        if unmodelled {
            world.run_callback(id, target);
        }
    }

    /// Run the callback of every slot that has one with `code` in creation order, like the loops
    /// `0x227B50` (code 0, level load), `0x227DE0` (code 1, restore) and `0x227C90` (code 2, save;
    /// it also clears the aux byte `+0xA`). Event link callbacks ignore these codes.
    pub fn run_slot_callbacks(&mut self, code: u32, world: &mut dyn World) {
        for id in self.slots.ids().to_vec() {
            let Some(SlotCallback::World(callback)) = self.slots.get(id).map(|slot| slot.callback)
            else {
                continue;
            };
            self.slots.set_current(Some(id));
            match callbacks::decode_slot_callback(callback, code) {
                Ok(ops) => {
                    for op in ops {
                        if matches!(op, callbacks::WorldOp::ShtmRestore) && world.shtm_saved() {
                            self.fire_by_id(callbacks::tag(b"ETRC"), world);
                        }
                        world.apply_slot_op(id, &op);
                    }
                }
                Err(gap) => {
                    if !self.callback_gaps.contains(&gap) {
                        self.callback_gaps.push(gap);
                    }
                }
            }
        }
        self.slots.set_current(None);
    }

    fn apply_slot_op(&mut self, op: SlotOp, world: &mut dyn World) {
        match op {
            SlotOp::Finish(slot) => self.finish_slot(slot, world),
            SlotOp::Start(slot) => {
                let change = self.slots.start_pending(slot);
                self.apply_slot_change(change, world);
            }
            SlotOp::Reactivate(slot) => {
                let change = self.slots.reactivate(slot);
                self.apply_slot_change(change, world);
            }
            SlotOp::StartTimed { slot, value } => {
                self.timed_slot = Some((slot, self.slots.frame, value));
                let change = self.slots.start_pending(slot);
                self.apply_slot_change(change, world);
            }
            SlotOp::FinishTimed => {
                if let Some((slot, _, _)) = self.timed_slot.take() {
                    self.finish_slot(slot, world);
                }
            }
            SlotOp::SetCurrent(slot) => self.slots.set_current(Some(slot)),
        }
    }

    /// Advance one frame of `dt` seconds.
    pub fn step(&mut self, dt: f32, world: &mut dyn World) -> StepReport {
        let mut report = StepReport::default();
        self.elapsed += dt;
        self.last_dt = dt;
        let active = self.active.clone();
        for &index in &active {
            self.state_pass(index, dt, world, &mut report);
        }
        for &index in &active {
            verbs::tail(self, index, world, &mut report);
        }
        self.update_slots(world);
        report
    }

    /// Slot update `0x228870`: every event slot polls its link (callback code 3 of `0x235240`)
    /// and finishes when satisfied, then the mission evaluation runs.
    fn update_slots(&mut self, world: &mut dyn World) {
        self.slots.frame += 1;
        let link_slots = self.slots.event_link_slots();
        for id in self.slots.ids().to_vec() {
            self.slot_frame(id, world);
            if let Some(&(_, instance, side)) = link_slots.iter().find(|(slot, _, _)| *slot == id) {
                self.poll_event_slot(id, instance, side, world);
            }
        }
        let down = world.players_all_down();
        let (event, flags) = self
            .slots
            .evaluate_mission(&self.context, self.session_flags, down);
        self.session_flags = flags;
        if let Some(event) = event {
            world.mission_event(event);
            if !matches!(event, super::slots::MissionEvent::ObjectiveFailedPending) {
                world.end_match();
            }
        }
    }

    /// Callback code 3 of `0x235240`: the event slot finishes when the link mode's test passes.
    fn poll_event_slot(&mut self, id: u32, instance: usize, side: LinkSide, world: &mut dyn World) {
        let record = self.records[instance];
        let link = match side {
            LinkSide::A => record.link_a,
            LinkSide::B => record.link_b,
        };
        let target = match side {
            LinkSide::A => self.instances[instance].link_a_target,
            LinkSide::B => self.instances[instance].link_b_target,
        };
        let satisfied = match link_class(link.mode) {
            LinkClass::Instance => target.is_some_and(|t| self.instances[t].executed != 0),
            LinkClass::SlotStatus => self.slots.is_finished(link.args[0]),
            LinkClass::Polled => world.poll_link(&link),
            _ => false,
        };
        if satisfied {
            self.finish_slot(id, world);
        }
    }

    /// Is the link satisfied for the state pass: the word set, or mode 0 (`R+0x40 == 0`).
    fn link_set(&self, index: usize, side: LinkSide) -> bool {
        let record = &self.records[index];
        let instance = &self.instances[index];
        match side {
            LinkSide::A => instance.a_set || record.link_a.mode == 0,
            LinkSide::B => instance.b_set || record.link_b.mode == 0,
        }
    }

    /// Spec 4.1 step 3 (jump table `0x238220`, combine operator `R+0x60`), literal from the
    /// instructions. Link B counts as satisfied when absent (`R+0x64 == 0`) except for operator 3.
    fn combine(&self, index: usize) -> bool {
        let record = &self.records[index];
        let a = self.link_set(index, LinkSide::A);
        let b = self.link_set(index, LinkSide::B);
        let b_exists = record.link_b.mode != 0;
        match record.combine {
            0 => a,
            1 => a && b,
            2 => a || b,
            3 => a && b_exists && !self.instances[index].b_set,
            _ => false,
        }
    }

    fn state_pass(
        &mut self,
        index: usize,
        dt: f32,
        world: &mut dyn World,
        report: &mut StepReport,
    ) {
        let record = self.records[index];
        if self.instances[index].executed == 2 {
            self.instances[index].executed = 0;
        }
        if self.instances[index].running == 2 {
            return;
        }
        let ran = self.instances[index].executed != 0;
        let skip_links = ran && self.instances[index].running != 0;
        if !skip_links && !self.instances[index].fired {
            // Step 2: gate mode 2 fires as soon as the gate ran, ignoring the links.
            let gate_ran = self.instances[index]
                .gate
                .is_some_and(|g| self.instances[g].executed != 0);
            if !self.instances[index].done && record.gate_mode == 2 && gate_ran {
                self.fire(index, world, report);
            }
            if !self.instances[index].fired && !self.instances[index].done && self.combine(index) {
                self.fire(index, world, report);
            }
        }
        if !self.instances[index].fired {
            return;
        }
        // Step 4: both conditions must hold every frame until the first execution.
        if self.instances[index].executed == 0 {
            let slots = &self.slots;
            let verdict = condition::evaluate(&record.condition_a, world, slots).and_then(|a| {
                if a {
                    condition::evaluate(&record.condition_b, world, slots)
                } else {
                    Ok(false)
                }
            });
            match verdict {
                Ok(true) => {}
                Ok(false) => return,
                Err(error) => {
                    report.errors.push((index, error));
                    return;
                }
            }
        }
        // Step 5: the delay counts down every frame (also while a pre-wait blocks), then the pre-wait.
        self.instances[index].delay -= dt;
        if let Some(wait) = super::actions::prewait(&record)
            && !world.prewait_check(record.verb, &wait)
        {
            return;
        }
        if self.instances[index].delay > 0.0 {
            return;
        }
        // Step 6: gate, only for an instance that is not running. A missing gate instance skips
        // the execution for gate modes 1 and 3 (MEASURED, corrects the T1838 wording).
        if self.instances[index].running == 0 {
            let gate = self.instances[index].gate;
            let gate_ran = gate.is_some_and(|g| self.instances[g].executed != 0);
            let run = match (record.gate_mode, gate) {
                (1, None) | (3, None) => false,
                (1, Some(_)) => {
                    if !gate_ran {
                        return;
                    }
                    true
                }
                (3, Some(_)) => {
                    if gate_ran {
                        return;
                    }
                    true
                }
                _ => true,
            };
            if run {
                self.execute(index, world, report);
            }
        }
        // Step 7: done test and notification of the next definition, then step 8.
        let done = self.is_done(index);
        self.refresh_done(index);
        self.notify_next(index, done);
        if !done {
            return;
        }
        self.instances[index].fired = false;
        if record.is_repeatable() {
            self.rearm(index, &record);
        }
        if record.verb == 10 && record.arg(0xC) != u32::MAX {
            verbs::effect_burst(self, index, world);
        }
    }

    /// `0x233E80`: clear the link word, re-activate the slot, set again for modes 1, 6, 7, 15.
    fn rearm_link(&mut self, index: usize, side: LinkSide, link: &Link) {
        let slot = match side {
            LinkSide::A => self.instances[index].slot_a,
            LinkSide::B => self.instances[index].slot_b,
        };
        let set = rearm_sets_again(link.mode);
        match side {
            LinkSide::A => self.instances[index].a_set = set,
            LinkSide::B => self.instances[index].b_set = set,
        }
        if let Some(slot) = slot {
            self.slots.reactivate(slot);
        }
    }

    /// Spec 4.1 step 8: flag 0x20 re-arms links, clears running/done, sets marker 2.
    fn rearm(&mut self, index: usize, record: &EventRecord) {
        if rust_rearm_enabled() {
            let instance = &self.instances[index];
            let transition = rearm_transition(RearmInput {
                link_a_mode: record.link_a.mode,
                link_b_mode: record.link_b.mode,
                has_link_b: record.combine != 0,
                slot_a: instance.slot_a,
                slot_b: instance.slot_b,
                delay: record.delay(),
            });
            let instance = &mut self.instances[index];
            instance.a_set = transition.a_set;
            instance.b_set = transition.b_set;
            instance.running = transition.running;
            instance.done = transition.done;
            instance.executed = transition.executed;
            instance.delay = transition.delay;
            for slot in [transition.reactivate_a, transition.reactivate_b]
                .into_iter()
                .flatten()
            {
                self.slots.reactivate(slot);
            }
            return;
        }
        self.rearm_link(index, LinkSide::A, &record.link_a);
        if record.combine != 0 {
            self.rearm_link(index, LinkSide::B, &record.link_b);
        }
        let instance = &mut self.instances[index];
        instance.running = 0;
        instance.delay = record.delay();
        instance.done = false;
        instance.executed = 2;
    }

    pub(crate) fn execute(&mut self, index: usize, world: &mut dyn World, report: &mut StepReport) {
        let record = self.records[index];
        self.instances[index].running = 1;
        self.instances[index].executed = 1;
        self.instances[index].world_done = false;
        self.traces[index].executed_at.get_or_insert(self.elapsed);
        self.traces[index].executions += 1;
        match verbs::run(self, index, world) {
            Ok(verbs::Outcome::Ran) => {
                report.fired.push(Fired {
                    instance: index,
                    event_id: record.event_id,
                    verb: record.verb,
                });
            }
            Ok(verbs::Outcome::Aborted) => self.undo_execution(index),
            Err(error) => {
                self.undo_execution(index);
                report.errors.push((index, error));
            }
        }
        self.refresh_done(index);
    }

    fn undo_execution(&mut self, index: usize) {
        self.instances[index].running = 0;
        self.instances[index].executed = 0;
        self.traces[index].executed_at = None;
        self.traces[index].executions -= 1;
    }

    /// Spec 5.2 `0x2350A0`. World waits set the `done` flag in the tail pass, so this stays pure.
    pub fn is_done(&self, index: usize) -> bool {
        use super::actions::{Completion, completion};
        let instance = &self.instances[index];
        if instance.executed == 0 || instance.running == 2 {
            return false;
        }
        let record = &self.records[index];
        match completion(record.verb) {
            Completion::Immediate => true,
            Completion::Flag => instance.done,
            Completion::World => instance.done || instance.world_done,
            Completion::Sound => instance.world_done,
            Completion::NoTickCallback => record.tick_callback == 0,
            Completion::SoundOrNone => record.arg(0xC) == u32::MAX || instance.world_done,
        }
    }

    pub(crate) fn refresh_done(&mut self, index: usize) {
        if self.is_done(index) {
            self.traces[index].done_at.get_or_insert(self.elapsed);
        }
    }

    /// Spec 4.1 step 7 (`0x2351D0`): tell the next definition in pool order.
    fn notify_next(&mut self, index: usize, done: bool) {
        let next = index + 1;
        if next >= self.instances.len() {
            return;
        }
        let record = self.records[next];
        let notification = next_link_notification(
            record.link_a.mode,
            record.link_b.mode,
            record.combine != 0,
            done,
        );
        if notification.link_a {
            self.instances[next].a_set = true;
        }
        if notification.link_b {
            self.instances[next].b_set = true;
        }
    }

    // ---- checkpoint snapshot / restore (spec 5.3) ------------------------------------------

    /// `0x234320`: 16 bytes per instance.
    pub fn capture(&mut self, world: &mut dyn World) -> EventSnapshot {
        self.pending_restore.clear();
        let mut entries = Vec::with_capacity(self.instances.len());
        for index in 0..self.instances.len() {
            let instance = &self.instances[index];
            let record = &self.records[index];
            let mut words = [0u32; 4];
            words[0] = (u32::from(instance.fired) << 24)
                | (u32::from(instance.executed) << 16)
                | (u32::from(instance.a_set) << 8)
                | u32::from(instance.b_set);
            if record.restore_id != 0 {
                words[1] = world.restore_save(record.restore_id);
            }
            let alive = instance.object_handle.filter(|&h| world.object_exists(h));
            if record.verb == 3
                && let Some((first, second)) = alive.and_then(|h| world.object_words(h))
            {
                words[2] = first;
                words[3] = second;
            }
            if record.verb == 43 && instance.fired && alive.is_some() {
                words[0] = 0;
            }
            entries.push(words);
        }
        EventSnapshot { entries }
    }

    /// `0x237500`: write the snapshot back and re-execute / mark done as the original does.
    pub fn restore(&mut self, snapshot: &EventSnapshot, world: &mut dyn World) {
        let mut report = StepReport::default();
        self.pending_restore.clear();
        for index in 0..self.instances.len().min(snapshot.entries.len()) {
            let [d0, d1, d2, d3] = snapshot.entries[index];
            let record = self.records[index];
            {
                let instance = &mut self.instances[index];
                instance.fired = (d0 >> 24) & 0xFF != 0;
                instance.executed = ((d0 >> 16) & 0xFF) as u8;
                instance.a_set = (d0 >> 8) & 0xFF != 0;
                instance.b_set = d0 & 0xFF != 0;
                if instance.fired && instance.executed == 0 {
                    instance.delay = record.delay();
                }
            }
            let executed = self.instances[index].executed != 0;
            let rerun = record.verb == 27 || (record.verb != 3 && record.flags & 0x1000 != 0);
            if rerun {
                if executed {
                    self.execute(index, world, &mut report);
                }
            } else if executed && !record.is_repeatable() {
                self.instances[index].done = true;
            }
            if record.verb == 43 && !self.instances[index].fired {
                self.reset_instance(index, Some(world));
            }
            if record.restore_id != 0 {
                world.restore_apply(record.restore_id, d1);
            }
            let stage = world.mission_stage();
            if record.link_a.mode == 19 && stage == record.link_a.args[0] {
                self.instances[index].a_set = true;
            }
            if record.combine != 0 && record.link_b.mode == 19 && stage == record.link_b.args[0] {
                self.instances[index].b_set = true;
            }
            if record.verb == 3 && record.flags & 0x10 != 0 && executed {
                self.restore_persistent_object(index, d2, d3, world, &mut report);
            }
        }
        self.side_report.fired.append(&mut report.fired);
        self.side_report.errors.append(&mut report.errors);
    }

    fn restore_persistent_object(
        &mut self,
        index: usize,
        d2: u32,
        d3: u32,
        world: &mut dyn World,
        report: &mut StepReport,
    ) {
        let record = self.records[index];
        // `0x23765B..0x2376C1`: the per-type gate (T1844) sends a flagged type to the deferral path.
        let gated = super::levels::restore_gate_applies(&self.context)
            && world.object_class_flags(record.arg(0x10)) & super::levels::CLASS_FLAG_NO_RESTORE
                != 0;
        if !gated && world.persistent_spawn_ready(record.arg(0x10)) {
            self.execute(index, world, report);
            if let Some(handle) = self.instances[index].object_handle {
                world.set_object_words(handle, d2, d3);
            }
        } else {
            // `0x234490`: defer until the asset is available.
            self.instances[index].running = 0;
            self.instances[index].done = false;
            if !self.pending_restore.iter().any(|&(i, _, _)| i == index) {
                self.pending_restore.push((index, d2, d3));
            }
        }
    }

    /// `0x237770`: run deferred persistent spawns whose asset is now available.
    pub fn apply_pending_restore(&mut self, world: &mut dyn World) -> usize {
        // `0x237770` returns at once for levels above 0x64 (T1844).
        if !super::levels::pending_restore_enabled(self.context.level_id) {
            return 0;
        }
        let pending = std::mem::take(&mut self.pending_restore);
        let mut report = StepReport::default();
        let mut applied = 0;
        for (index, d2, d3) in pending {
            let record = self.records[index];
            let idle = self.instances[index].running == 0 && !self.instances[index].done;
            if idle && world.persistent_spawn_ready(record.arg(0x10)) {
                self.execute(index, world, &mut report);
                if let Some(handle) = self.instances[index].object_handle {
                    world.set_object_words(handle, d2, d3);
                }
                applied += 1;
            } else if idle {
                self.pending_restore.push((index, d2, d3));
            }
        }
        self.side_report.fired.append(&mut report.fired);
        self.side_report.errors.append(&mut report.errors);
        applied
    }

    /// `0x228870`: run the per-level hook `[level+0x110]` of the context when the dispatch plan reaches it.
    /// `steps` is `[0x4E7944]`, `descriptor_flags` the descriptor dword `+0x18` (only read with game flag 0x40).
    /// Returns the plan so the caller can run the slot poll first. Level 0x64's hook is null (T1845, MEASURED).
    pub fn frame_level_hook(
        &mut self,
        steps: i32,
        descriptor_flags: u32,
        world: &mut dyn World,
    ) -> super::levels::HookDispatch {
        let plan =
            super::levels::frame_hook_dispatch(steps, self.context.game_flags, descriptor_flags);
        if plan == super::levels::HookDispatch::AfterObjective && self.context.mode_id == 5 {
            world.run_mode5_beeper(steps as f32);
        }
        if plan != super::levels::HookDispatch::Skip {
            if let super::levels::HookSource::Static(address) =
                super::levels::level_hook(&self.context)
            {
                world.run_level_hook(address);
            }
        }
        plan
    }

    /// Mode 3 objective runtime of `0x228870` (`0x232270`, T1845): slot status 5 is answered by this engine's slot
    /// manager, the other queries by `arcade_world`. Every `SlotFinished` event finishes the slot like `0x2274C0`.
    /// The events are returned so the caller can run the hook and the world side effects.
    pub fn run_arcade_objective(
        &mut self,
        table: &mut super::arcade::ObjectiveTable,
        arcade_world: &mut dyn super::arcade::ArcadeWorld,
        world: &mut dyn World,
    ) -> Vec<super::arcade::ObjectiveEvent> {
        struct Bridge<'a> {
            slots: &'a SlotManager,
            inner: &'a mut dyn super::arcade::ArcadeWorld,
        }
        impl super::arcade::ArcadeWorld for Bridge<'_> {
            fn slot_finished(&self, slot: u32) -> bool {
                self.slots.is_finished(slot)
            }
            fn random_u32(&mut self) -> u32 {
                self.inner.random_u32()
            }
            fn actor(&self, slot: u32, index: usize) -> Option<super::arcade::ActorState> {
                self.inner.actor(slot, index)
            }
            fn member_complete(&mut self, slot: u32, member: usize) -> bool {
                self.inner.member_complete(slot, member)
            }
            fn member_bits(
                &mut self,
                slot: u32,
                member: usize,
                kind: u32,
            ) -> super::arcade::MemberBits {
                self.inner.member_bits(slot, member, kind)
            }
            fn sub_entry_count(&self, slot: u32, member: usize) -> usize {
                self.inner.sub_entry_count(slot, member)
            }
            fn sub_entry_triggered(&self, slot: u32, member: usize, index: usize) -> bool {
                self.inner.sub_entry_triggered(slot, member, index)
            }
            fn set_sub_entry_triggered(&mut self, slot: u32, member: usize, index: usize) {
                self.inner.set_sub_entry_triggered(slot, member, index)
            }
            fn type7_candidates(
                &mut self,
                slot: u32,
                member: usize,
            ) -> (u32, Vec<super::arcade::TargetObject>) {
                self.inner.type7_candidates(slot, member)
            }
            fn type89_targets(
                &mut self,
                slot: u32,
                member: usize,
            ) -> Vec<super::arcade::TargetObject> {
                self.inner.type89_targets(slot, member)
            }
        }
        let events = table.frame(&mut Bridge {
            slots: &self.slots,
            inner: arcade_world,
        });
        for event in &events {
            if let super::arcade::ObjectiveEvent::SlotFinished { slot } = event {
                self.finish_slot(*slot, world);
            }
        }
        events
    }

    pub fn mission_complete(&self) -> bool {
        self.session_flags & SESSION_COMPLETE != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NextLinkNotification {
    link_a: bool,
    link_b: bool,
}

/// Typed decision made by `0x2351D0` for the next definition in pool order.
fn next_link_notification(
    link_a_mode: u32,
    link_b_mode: u32,
    has_link_b: bool,
    current_done: bool,
) -> NextLinkNotification {
    let notify = |mode| mode == 2 || (mode == 3 && current_done);
    NextLinkNotification {
        link_a: notify(link_a_mode),
        link_b: has_link_b && notify(link_b_mode),
    }
}

#[cfg(test)]
mod successor_notification_tests {
    use super::*;

    // Branch-shaped translation of the retained `0x2351D0` predicates: mode 2 always marks;
    // mode 3 marks only when the current event's completion predicate is true.
    fn lifted_notification(
        link_a_mode: u32,
        link_b_mode: u32,
        has_link_b: bool,
        current_done: bool,
    ) -> NextLinkNotification {
        let mut link_a = false;
        let mut link_b = false;
        if link_a_mode == 3 {
            if current_done {
                link_a = true;
            }
        } else if link_a_mode == 2 {
            link_a = true;
        }
        if has_link_b {
            if link_b_mode == 3 {
                if current_done {
                    link_b = true;
                }
            } else if link_b_mode == 2 {
                link_b = true;
            }
        }
        NextLinkNotification { link_a, link_b }
    }

    #[test]
    fn tier1_weak_successor_differential_matches_4096_fabricated_cases() {
        let mut seed = 0x2351_d170u32;
        for _ in 0..4096 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let link_a_mode = seed % 24;
            let link_b_mode = seed.rotate_left(11) % 24;
            let has_link_b = seed & 0x100 != 0;
            let current_done = seed & 0x8000 != 0;
            assert_eq!(
                next_link_notification(link_a_mode, link_b_mode, has_link_b, current_done),
                lifted_notification(link_a_mode, link_b_mode, has_link_b, current_done)
            );
        }
    }

    #[test]
    fn changing_done_state_mutates_mode_three_but_not_mode_two_notification() {
        let mode_three_not_done = next_link_notification(3, 3, true, false);
        let mode_three_done = next_link_notification(3, 3, true, true);
        assert_eq!(
            mode_three_not_done,
            NextLinkNotification {
                link_a: false,
                link_b: false
            }
        );
        assert_eq!(
            mode_three_done,
            NextLinkNotification {
                link_a: true,
                link_b: true
            }
        );
        assert_eq!(
            next_link_notification(2, 2, true, false),
            NextLinkNotification {
                link_a: true,
                link_b: true
            }
        );
    }
}

#[cfg(test)]
mod rearm_transition_tests {
    use super::*;

    // Branch-shaped model of the retained rearm sequence: 0x233E80 clears each selected link,
    // only modes 1/6/7/15 set it again, and slot ids are reactivated independently of that bit.
    fn lifted_rearm(input: RearmInput) -> RearmTransition {
        let mut a_set = false;
        let mut b_set = false;
        if input.link_a_mode == 1
            || input.link_a_mode == 6
            || input.link_a_mode == 7
            || input.link_a_mode == 15
        {
            a_set = true;
        }
        let (reactivate_b, b_set) = if input.has_link_b {
            let mut set = false;
            if input.link_b_mode == 1
                || input.link_b_mode == 6
                || input.link_b_mode == 7
                || input.link_b_mode == 15
            {
                set = true;
            }
            (input.slot_b, set)
        } else {
            (None, false)
        };
        RearmTransition {
            a_set,
            b_set,
            reactivate_a: input.slot_a,
            reactivate_b,
            running: 0,
            done: false,
            executed: 2,
            delay: input.delay,
        }
    }

    #[test]
    fn tier1_weak_rearm_differential_matches_4096_fabricated_cases() {
        let mut seed = 0x233e_80u32;
        for _ in 0..4096 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let input = RearmInput {
                link_a_mode: seed % 24,
                link_b_mode: seed.rotate_left(9) % 24,
                has_link_b: seed & 0x100 != 0,
                slot_a: (seed & 0x200 != 0).then_some(seed % 32),
                slot_b: (seed & 0x400 != 0).then_some(seed.rotate_left(15) % 32),
                delay: f32::from_bits(seed.rotate_left(5)),
            };
            let rust = rearm_transition(input);
            let lifted = lifted_rearm(input);
            // NaN is a valid fabricated delay bit pattern; compare its payload rather than
            // IEEE equality, where even identical NaNs compare unequal.
            assert_eq!(rust.a_set, lifted.a_set);
            assert_eq!(rust.b_set, lifted.b_set);
            assert_eq!(rust.reactivate_a, lifted.reactivate_a);
            assert_eq!(rust.reactivate_b, lifted.reactivate_b);
            assert_eq!(rust.running, lifted.running);
            assert_eq!(rust.done, lifted.done);
            assert_eq!(rust.executed, lifted.executed);
            assert_eq!(rust.delay.to_bits(), lifted.delay.to_bits());
        }
    }

    #[test]
    fn mutations_cover_rearm_modes_slot_selection_and_state_reset() {
        let base = RearmInput {
            link_a_mode: 1,
            link_b_mode: 7,
            has_link_b: true,
            slot_a: Some(3),
            slot_b: Some(5),
            delay: 2.5,
        };
        let transition = rearm_transition(base);
        assert!(transition.a_set && transition.b_set);
        assert_eq!(
            (transition.reactivate_a, transition.reactivate_b),
            (Some(3), Some(5))
        );
        assert_eq!(
            (transition.running, transition.done, transition.executed),
            (0, false, 2)
        );
        assert_eq!(transition.delay, base.delay);

        let changed_mode = rearm_transition(RearmInput {
            link_a_mode: 2,
            ..base
        });
        assert!(!changed_mode.a_set);
        let no_b = rearm_transition(RearmInput {
            has_link_b: false,
            ..base
        });
        assert!(!no_b.b_set);
        assert_eq!(no_b.reactivate_b, None);
    }

    #[test]
    fn rust_ab_switch_is_opt_in() {
        assert!(!rust_rearm_enabled_from(None));
        assert!(!rust_rearm_enabled_from(Some("0")));
        assert!(rust_rearm_enabled_from(Some("rust")));
    }
}
