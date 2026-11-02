//! Slot manager (T1841). A **slot** is a numbered objective/dependency cell with a status byte.
//! Static slots (ids below 0x2D) come from the level's slot table, **event slots** (ids from 0x2D)
//! are created by the pool builder `0x235630` for the link A/B of each event instance and are
//! finished by the link callback `0x235240`. Everything here is decoded from `build/default.xbe`
//! (`0x227100..0x228B34`, record 0x4C bytes at `[0x78CECC]`, list `[0x75C14C]`, counters
//! `[0x75C134]`) and labelled MEASURED unless a comment says otherwise.

use super::engine::LinkSide;
use super::mode::ModeContext;
pub use tsfp_state::story_flow::CampaignOrdinal;

/// Static slots are ids below this; event slots start here (`cmp edi, 0x2d` in `0x227750`).
pub const STATIC_SLOT_COUNT: u32 = 0x2D;
/// Header triple of the static slot table in a SETU entry: offset +0x84, count +0x88, stride +0x8C.
/// The field layout of the entries is MEASURED (`0x227E90`), the header location is INFERRED from
/// the match of the entry shape with that loader.
pub const SLOT_TABLE_TRIPLE: usize = 0x84;
pub const SLOT_DEF_STRIDE: usize = 0x38;

pub const STATUS_FREE: u8 = 0;
/// Created but not started (static slots with `initially_active == 0`).
pub const STATUS_PENDING: u8 = 1;
pub const STATUS_ACTIVE: u8 = 2;
/// Reached by a second activation of an active slot (verb 36); meaning INFERRED.
pub const STATUS_MARKED: u8 = 3;
/// Failed objective (verb 37).
pub const STATUS_FAILED: u8 = 4;
pub const STATUS_FINISHED: u8 = 5;

/// Four-byte identity copied from a static objective definition. This is distinct from the
/// runtime objective slot number and from the Story campaign ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectiveDefinitionKey(pub u32);

impl ObjectiveDefinitionKey {
    pub const fn get(self) -> u32 {
        self.0
    }
}

const NO_KIND: u32 = u32::MAX;
const MASK_EVENT_SLOT: u32 = 0x0E00_077F;
const MASK_STATIC_SLOT: u32 = 0x700;
const FRAMES_PER_SECOND: u32 = 60;

/// One entry of the static slot table (0x38 bytes, `0x227E90`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotDef {
    /// +0x00 slot id.
    pub id: u32,
    /// +0x04 four-character key (for example the objective tag).
    pub key: ObjectiveDefinitionKey,
    /// +0x08 mode mask tested with `0x6B100`.
    pub mask: u32,
    /// +0x0C label (text id for the HUD, 0 = hidden).
    pub label: u32,
    /// +0x10, +0x14, +0x18: slot kind for sub-mode 0, 1, 2 (-1 = not present in that sub-mode).
    pub kinds: [u32; 3],
    /// +0x1C callback id (rewritten to a code address at load; 0 = none).
    pub callback_id: u32,
    /// +0x20..+0x30 up to five dependency ids (-1 = end).
    pub dependencies: [u32; 5],
    /// +0x34: non-zero = the slot is active at level start, zero = it starts pending (status 1).
    pub initially_active: bool,
}

/// What a slot calls when it finishes (`[slot+0x3C]`, invoked with the slot and code 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlotCallback {
    #[default]
    None,
    /// `0x235240`: sets link A or B of an event instance.
    EventLink { instance: usize, side: LinkSide },
    /// Static slot callback id, executed by the world (`World::slot_callback`).
    World(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub id: u32,
    /// Four-character objective key from the loaded static definition, when this slot came from
    /// a decoded level table. Event slots and ad-hoc test slots have no definition key.
    pub definition_key: Option<ObjectiveDefinitionKey>,
    /// +2: HUD label. +4 and +6: alternative labels (+4 is used by mode 3 with two players).
    pub label: u32,
    pub alt_label: u32,
    pub label_extra: u32,
    /// +8: slot kind, index of the counters. 0 mission objective, 2 hidden (event slots).
    pub kind: u8,
    /// +9: bit 1 = counts as complete when the mission finishes (`0x227E40`), bit 0 enables progress tracking.
    pub flags: u8,
    /// +0xB.
    pub status: u8,
    /// +0x34: frame stamp written by a finish in mode 3 (`now - 3 s`).
    pub finished_stamp: u32,
    pub callback: SlotCallback,
    /// +0x10..+0x20 dependency ids registered by `0x2278E0`.
    pub dependencies: Vec<u32>,
    /// +0x48 progress tracking selector (0xFF = off).
    pub tracking: u8,
    /// +0x2C: dead count seen by the last progress pass (`u32::MAX` after an actor left the tracker, forces a recount).
    pub last_count: u32,
    /// +0x30: number of actors registered while tracking was on.
    pub total: u32,
    /// +0x24 and +0x26: HUD labels of the progress message for one and for several remaining. Writers not found (0 = no message).
    pub progress_one: u32,
    pub progress_many: u32,
    /// +0xC for static slots: the actor registry ([`Tracker`]).
    pub tracker: Option<Tracker>,
}

/// Number of class keys and of actors one slot tracker holds (`0x227100` clears 10 + 10 + 10 dwords).
pub const TRACKER_CLASSES: usize = 10;
pub const TRACKER_ACTORS: usize = 10;

/// Actor registry of a static slot (struct of 0x7C bytes in the pool `[0x78CED4]`, initialised by
/// `0x227100` with the sentinel -1). Layout: +0 sentinel, +4 ten class keys, +0x2C ten per class
/// counts, +0x54 ten actor handles. Actors are kept grouped by class in key order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tracker {
    /// Class keys in insertion order (`0x227880`), at most [`TRACKER_CLASSES`], no duplicates.
    pub keys: Vec<u32>,
    /// Actors registered per class (parallel to `keys`).
    pub counts: [u32; TRACKER_CLASSES],
    /// Actor handles grouped by class, 0 = empty.
    pub actors: [u32; TRACKER_ACTORS],
}

impl Tracker {
    /// `0x227880`: add a class key unless present or the list is full.
    pub fn add_class(&mut self, key: u32) {
        if !self.keys.contains(&key) && self.keys.len() < TRACKER_CLASSES {
            self.keys.push(key);
        }
    }

    /// Actors currently registered.
    pub fn actor_count(&self) -> usize {
        self.actors.iter().filter(|&&actor| actor != 0).count()
    }

    /// `0x227170` minus the slot counter: insert `actor` at the end of its class group. Returns
    /// whether it was stored (false = unknown class, registry full or no class registered).
    fn register(&mut self, actor: u32, class: u32) -> bool {
        let Some(class_index) = self.keys.iter().position(|&key| key == class) else {
            return false;
        };
        if self.actors[TRACKER_ACTORS - 1] != 0 {
            return false;
        }
        let position: u32 = self.counts[..=class_index].iter().sum();
        let position = position as usize;
        if position < TRACKER_ACTORS - 1 {
            for index in (position + 1..TRACKER_ACTORS).rev() {
                self.actors[index] = self.actors[index - 1];
            }
        }
        self.actors[position.min(TRACKER_ACTORS - 1)] = actor;
        self.counts[class_index] += 1;
        true
    }

    /// `0x2271F0` minus the slot fields: remove every occurrence of `actor`, keep the actors
    /// packed and decrement the owning class. Returns how many were removed.
    fn unregister(&mut self, actor: u32) -> u32 {
        if self.actors[0] == 0 {
            return 0;
        }
        let mut removed = 0;
        let mut index = 0;
        while index < TRACKER_ACTORS && self.actors[index] != 0 {
            if self.actors[index] != actor {
                index += 1;
                continue;
            }
            for shift in index..TRACKER_ACTORS - 1 {
                self.actors[shift] = self.actors[shift + 1];
            }
            self.actors[TRACKER_ACTORS - 1] = 0;
            let mut cumulative = 0;
            for class_index in 0..self.keys.len() {
                cumulative += self.counts[class_index] as usize;
                if index < cumulative {
                    self.counts[class_index] -= 1;
                    removed += 1;
                    break;
                }
            }
        }
        removed
    }

    /// `0x227980`: the `index`-th actor of class `class`, 0 when the class is unknown.
    pub fn actor_of(&self, class: u32, index: u32) -> u32 {
        let mut before = 0usize;
        for (class_index, &key) in self.keys.iter().enumerate() {
            if key == class {
                return self
                    .actors
                    .get(before + index as usize)
                    .copied()
                    .unwrap_or(0);
            }
            before += self.counts[class_index] as usize;
        }
        0
    }
}

impl Slot {
    fn empty(id: u32) -> Self {
        Self {
            id,
            definition_key: None,
            label: 0,
            alt_label: 0,
            label_extra: 0,
            kind: 0,
            flags: 0,
            status: STATUS_FREE,
            finished_stamp: 0,
            callback: SlotCallback::None,
            dependencies: Vec::new(),
            tracking: 0xFF,
            last_count: 0,
            total: 0,
            progress_one: 0,
            progress_many: 0,
            tracker: None,
        }
    }
}

/// HUD-facing notification for the world (`0x227430` posts per player messages).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotEvent {
    /// Status changed through verbs 33, 34, 36, 37.
    Changed { slot: u32, status: u8, label: u32 },
    /// A finish (`0x2274C0`). `mode3` selects the arcade-style path with per player messages.
    Finished {
        slot: u32,
        label: u32,
        alt_label: u32,
        mode3: bool,
    },
    /// Static slot progress (`0x2282B0`): `remaining` tracked actors are left. `label` is the
    /// HUD text id (`+0x24` for one remaining, `+0x26` for several), posted to each local player
    /// as message kind 2 with the count in front. Only emitted when the slot has flag bit 0.
    Progress {
        slot: u32,
        player: u32,
        remaining: u32,
        label: u32,
    },
    /// Progress jingle: effect 0xD6C started by `0x2282B0` after a progress change of a flagged slot.
    ProgressSound { slot: u32 },
}

/// Result of a state change that may require follow-up work by the engine.
#[derive(Debug, Default)]
pub struct SlotChange {
    pub events: Vec<SlotEvent>,
    /// Callbacks to invoke with code 4 (finish).
    pub callbacks: Vec<(u32, SlotCallback)>,
}

/// Outcome of the per frame mission evaluation after the slot update (`0x228870`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionEvent {
    /// All local players are in state 0x10: sets session flag 0x10000 and ends the match.
    AllPlayersDown,
    /// Every kind-0 slot is resolved and at least one failed: message 0x15F7, flag 0x10000, match end.
    ObjectivesFailed,
    /// A failed objective is pending while others are open: message 0x15F8 every 10th frame, flag 0x10000.
    ObjectiveFailedPending,
    /// Every kind-0 slot is finished: auto-finish flagged slots, message 0x15F9, flag 0x8000, match end.
    Complete,
}

pub const SESSION_COMPLETE: u32 = 0x8000;
pub const SESSION_FAILED: u32 = 0x10000;

/// Identity for an admitted static objective slot. Event dependency slots are not objective ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectiveSlotId(pub u32);

/// Rust-owned snapshot of the documented mission/profile attachment boundary.
///
/// `result_flags` retains completion and failure bits independently; phase 8 is carried as raw
/// state and does not imply success. `campaign_ordinal` is already translated from the gameplay
/// level id by the caller. The objective list contains admitted static slots in creation order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionOutcome {
    pub mode_id: u8,
    pub result_flags: u32,
    pub phase: u32,
    pub event: Option<MissionEvent>,
    pub campaign_ordinal: Option<CampaignOrdinal>,
    pub objectives: Vec<ObjectiveProgress>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectiveProgress {
    pub id: ObjectiveSlotId,
    /// Source-table key when admitted from a decoded static objective definition.
    pub definition_key: Option<ObjectiveDefinitionKey>,
    pub kind: u8,
    pub status: u8,
    pub flags: u8,
}

impl MissionOutcome {
    /// Success is the original result bit; phase and objective presence are separate evidence.
    pub const fn has_success_result(&self) -> bool {
        self.result_flags & SESSION_COMPLETE != 0
    }

    pub const fn has_failure_result(&self) -> bool {
        self.result_flags & SESSION_FAILED != 0
    }
}

#[derive(Debug, Default)]
pub struct SlotManager {
    slots: Vec<Slot>,
    /// `[0x75C14C]`: slot ids in creation order.
    order: Vec<u32>,
    /// `[0x75C134..]`: number of active slots per kind.
    counters: [i32; 3],
    /// `[0x7DE318]`, advanced by the caller each frame (used for the mode 3 stamp).
    pub frame: u32,
    /// `[0x78CED0]`: the slot created last or being called back (target of `0x227880`, `0x227A30`, `0x227A50`).
    current: Option<u32>,
}

impl SlotManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// `0x228180` without the level loader: all slots free, lists and counters cleared.
    pub fn reset(&mut self) {
        self.slots.clear();
        self.order.clear();
        self.counters = [0; 3];
        self.current = None;
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn ids(&self) -> &[u32] {
        &self.order
    }

    /// Capture the typed boundary consumed by Story progress. This is observational: it does not
    /// evaluate the mission, change slot status, or request profile persistence.
    pub fn mission_outcome(
        &self,
        mode_id: u8,
        result_flags: u32,
        phase: u32,
        event: Option<MissionEvent>,
        campaign_ordinal: Option<CampaignOrdinal>,
    ) -> MissionOutcome {
        let objectives = self
            .order
            .iter()
            .filter_map(|&id| {
                if id >= STATIC_SLOT_COUNT {
                    return None;
                }
                let slot = &self.slots[id as usize];
                Some(ObjectiveProgress {
                    id: ObjectiveSlotId(id),
                    definition_key: slot.definition_key,
                    kind: slot.kind,
                    status: slot.status,
                    flags: slot.flags,
                })
            })
            .collect();
        MissionOutcome {
            mode_id,
            result_flags,
            phase,
            event,
            campaign_ordinal,
            objectives,
        }
    }

    pub fn get(&self, id: u32) -> Option<&Slot> {
        self.order.contains(&id).then(|| &self.slots[id as usize])
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Slot> {
        self.slots.get_mut(id as usize)
    }

    fn slot_mut(&mut self, id: u32) -> Option<&mut Slot> {
        self.slots.get_mut(id as usize)
    }

    /// Status of a slot, 0 for an id that was never created.
    pub fn status(&self, id: u32) -> u8 {
        self.slots
            .get(id as usize)
            .map_or(STATUS_FREE, |slot| slot.status)
    }

    /// `0x2273C0`.
    pub fn is_finished(&self, id: u32) -> bool {
        self.status(id) == STATUS_FINISHED
    }

    /// `0x2273E0`.
    pub fn is_failed(&self, id: u32) -> bool {
        self.status(id) == STATUS_FAILED
    }

    /// `0x227400`: neither free nor pending.
    pub fn is_started(&self, id: u32) -> bool {
        !matches!(self.status(id), STATUS_FREE | STATUS_PENDING)
    }

    /// `0x227A70` with a zero kind argument returns `[0x75C134 + kind*4]`.
    pub fn active_count(&self, kind: usize) -> i32 {
        self.counters.get(kind).copied().unwrap_or(0)
    }

    /// `0x227A70` with a kind: number of listed slots of that kind regardless of status.
    pub fn count_of_kind(&self, kind: u8) -> usize {
        self.order
            .iter()
            .filter(|&&id| self.slots[id as usize].kind == kind)
            .count()
    }

    fn ensure(&mut self, id: u32) {
        let needed = id as usize + 1;
        while self.slots.len() < needed {
            let next = self.slots.len() as u32;
            self.slots.push(Slot::empty(next));
        }
    }

    /// `0x227750`: create a slot. `kinds` is chosen by the sub-mode (-1 = absent in that
    /// sub-mode, then nothing is created but the call still returns true). Returns false when the
    /// mode predicate rejects `mask`. The created slot is **active** (status 2).
    pub fn create(
        &mut self,
        ctx: &ModeContext,
        id: u32,
        label: u32,
        kinds: [u32; 3],
        mask: u32,
    ) -> bool {
        self.current = None;
        let Some(&kind) = kinds.get(ctx.submode as usize) else {
            return true;
        };
        if kind == NO_KIND {
            return true;
        }
        if !ctx.base_predicate(mask) {
            return false;
        }
        self.ensure(id);
        let kind = (kind as u8).min(2);
        self.counters[kind as usize] += 1;
        self.current = Some(id);
        let mut slot = Slot::empty(id);
        if id < STATIC_SLOT_COUNT {
            slot.tracker = Some(Tracker::default());
        }
        slot.kind = kind;
        slot.label = label;
        slot.status = STATUS_ACTIVE;
        self.slots[id as usize] = slot;
        self.order.push(id);
        true
    }

    /// `[0x78CED0]`.
    pub fn current(&self) -> Option<u32> {
        self.current
    }

    /// Callback dispatch sets the current slot like the loops `0x227B60` and `0x2282B0` do.
    pub fn set_current(&mut self, id: Option<u32>) {
        self.current = id;
    }

    /// `0x227880`: add a class key to the registry of the current slot.
    pub fn add_tracked_class(&mut self, key: u32) {
        if let Some(tracker) = self
            .current
            .and_then(|id| self.slot_mut(id))
            .and_then(|slot| slot.tracker.as_mut())
        {
            tracker.add_class(key);
        }
    }

    /// `0x227A30`: switch progress tracking on for the current slot (selector 0). The only
    /// caller in the image is the mode 3 group setup `0x2339FF`.
    pub fn enable_tracking(&mut self) {
        if let Some(slot) = self.current.and_then(|id| self.slot_mut(id)) {
            slot.tracking = 0;
        }
    }

    /// `0x2279D0`: register `actor` of class `class` (`actor+0x94`) with every static slot. Only
    /// in modes 10 and 3.
    pub fn register_actor(&mut self, ctx: &ModeContext, actor: u32, class: u32) {
        if !matches!(ctx.mode_id, 10 | 3) {
            return;
        }
        for id in self.order.clone() {
            if id >= STATIC_SLOT_COUNT {
                continue;
            }
            let Some(slot) = self.slot_mut(id) else {
                continue;
            };
            let tracking_on = slot.tracking != 0xFF;
            if let Some(tracker) = slot.tracker.as_mut()
                && tracker.register(actor, class)
                && tracking_on
            {
                slot.total += 1;
            }
        }
    }

    /// `0x227920`: remove `actor` from every static slot (modes 10 and 3), forcing a recount.
    pub fn unregister_actor(&mut self, ctx: &ModeContext, actor: u32) {
        if !matches!(ctx.mode_id, 10 | 3) {
            return;
        }
        for id in self.order.clone() {
            if id >= STATIC_SLOT_COUNT {
                continue;
            }
            let Some(slot) = self.slot_mut(id) else {
                continue;
            };
            let Some(tracker) = slot.tracker.as_mut() else {
                continue;
            };
            let removed = tracker.unregister(actor);
            if removed > 0 {
                slot.total = slot.total.wrapping_sub(removed);
                slot.last_count = u32::MAX;
            }
        }
    }

    /// `0x227980(slot, class, index)`: a registered actor, 0 when none.
    pub fn tracked_actor(&self, id: u32, class: u32, index: u32) -> u32 {
        self.get(id)
            .and_then(|slot| slot.tracker.as_ref())
            .map_or(0, |tracker| tracker.actor_of(class, index))
    }

    /// `0x227A50`: set the callback of a slot (the "current" slot of the original).
    pub fn set_callback(&mut self, id: u32, callback: SlotCallback) {
        if let Some(slot) = self.slot_mut(id) {
            slot.callback = callback;
        }
    }

    /// `0x2278E0`: register a dependency id (deduplicated, at most 5).
    pub fn add_dependency(&mut self, id: u32, dependency: u32) {
        if let Some(slot) = self.slot_mut(id)
            && slot.dependencies.len() < 5
            && !slot.dependencies.contains(&dependency)
        {
            slot.dependencies.push(dependency);
        }
    }

    /// Level start: `0x228180` clears, `0x227E90` creates every table entry whose mask passes
    /// `0x6B100` as active, then entries with `initially_active == false` drop to pending (1) and
    /// leave the active counter (`dec [0x75C134+kind*4]`). This is the level-start slot activation
    /// that T1838 left UNRESOLVED.
    pub fn load_level(&mut self, ctx: &ModeContext, defs: &[SlotDef]) {
        self.reset();
        for def in defs {
            if !ctx.base_predicate(def.mask) {
                continue;
            }
            if !self.create(ctx, def.id, def.label, def.kinds, MASK_STATIC_SLOT) {
                continue;
            }
            if let Some(slot) = self.slot_mut(def.id) {
                slot.definition_key = Some(def.key);
            }
            if def.callback_id != 0 {
                self.set_callback(def.id, SlotCallback::World(def.callback_id));
            }
            if !def.initially_active {
                self.demote_to_pending(def.id);
            }
            for dependency in def
                .dependencies
                .iter()
                .copied()
                .take_while(|&d| d != u32::MAX)
            {
                self.add_dependency(def.id, dependency);
            }
        }
    }

    fn demote_to_pending(&mut self, id: u32) {
        let Some(slot) = self.slots.get_mut(id as usize) else {
            return;
        };
        if matches!(slot.status, STATUS_ACTIVE | STATUS_MARKED) {
            slot.status = STATUS_PENDING;
            self.counters[slot.kind as usize] -= 1;
        }
    }

    fn changed(&self, id: u32) -> SlotEvent {
        let slot = &self.slots[id as usize];
        SlotEvent::Changed {
            slot: id,
            status: slot.status,
            label: slot.label,
        }
    }

    /// `0x227430` only notifies for slots that are not kind 2.
    fn notify(&self, id: u32, change: &mut SlotChange) {
        if self.slots[id as usize].kind != 2 {
            change.events.push(self.changed(id));
        }
    }

    /// `0x227620` (verb 35): status 2 or 3 back to pending, leaves the counter.
    pub fn deactivate(&mut self, id: u32) {
        self.demote_to_pending(id);
    }

    /// `0x227650` (verb 34): pending to active, counter up, notify.
    pub fn start_pending(&mut self, id: u32) -> SlotChange {
        let mut change = SlotChange::default();
        if self.status(id) == STATUS_PENDING {
            self.promote(id);
            self.notify(id, &mut change);
        }
        change
    }

    fn promote(&mut self, id: u32) {
        let slot = &mut self.slots[id as usize];
        slot.status = STATUS_ACTIVE;
        self.counters[slot.kind as usize] += 1;
    }

    /// `0x227690` (verb 36): pending to active (counter up), active or marked to marked. Notifies.
    pub fn activate(&mut self, id: u32) -> SlotChange {
        let mut change = SlotChange::default();
        match self.status(id) {
            STATUS_PENDING => {
                self.promote(id);
                self.notify(id, &mut change);
            }
            STATUS_ACTIVE | STATUS_MARKED => {
                self.slots[id as usize].status = STATUS_MARKED;
                self.notify(id, &mut change);
            }
            _ => {}
        }
        change
    }

    /// `0x2276E0` (verb 37): any started status except 4 becomes 4 (failed). Notifies.
    pub fn mark_failed(&mut self, id: u32) -> SlotChange {
        let mut change = SlotChange::default();
        let status = self.status(id);
        if status != STATUS_FAILED && status != STATUS_FREE {
            self.slots[id as usize].status = STATUS_FAILED;
            self.notify(id, &mut change);
        }
        change
    }

    /// `0x227710` (verb 33, link re-arm): any status except 0 and 2 becomes 2 (no counter change).
    pub fn reactivate(&mut self, id: u32) -> SlotChange {
        let mut change = SlotChange::default();
        let status = self.status(id);
        if status != STATUS_ACTIVE && status != STATUS_FREE {
            self.slots[id as usize].status = STATUS_ACTIVE;
            self.notify(id, &mut change);
        }
        change
    }

    /// `0x2275F0` (verb 32): quiet completion of an active or marked slot.
    pub fn complete_quiet(&mut self, id: u32) {
        if matches!(self.status(id), STATUS_ACTIVE | STATUS_MARKED) {
            self.slots[id as usize].status = STATUS_FINISHED;
        }
    }

    /// `0x2274C0` (verb 31 and the link callbacks): finish a slot. A pending slot is started first
    /// (non mode 3 path). Only an active or marked slot finishes. On success the finish callbacks
    /// are returned for the engine to run (the `[+0x38]` hook is not modelled, its setter was not
    /// found).
    pub fn finish(&mut self, ctx: &ModeContext, id: u32) -> SlotChange {
        let mut change = SlotChange::default();
        let Some(slot) = self.slots.get(id as usize) else {
            return change;
        };
        let mode3 = ctx.mode_id == 3;
        if mode3 && slot.kind != 2 {
            if !matches!(slot.status, STATUS_ACTIVE | STATUS_MARKED) {
                return change;
            }
            let stamp = self.frame.saturating_sub(3 * FRAMES_PER_SECOND);
            let slot = &mut self.slots[id as usize];
            slot.status = STATUS_FINISHED;
            slot.finished_stamp = stamp;
            let (label, alt_label) = (slot.label, slot.alt_label);
            change.events.push(SlotEvent::Finished {
                slot: id,
                label,
                alt_label,
                mode3: true,
            });
        } else {
            if slot.status == STATUS_PENDING {
                self.promote(id);
            }
            if !matches!(
                self.slots[id as usize].status,
                STATUS_ACTIVE | STATUS_MARKED
            ) {
                return change;
            }
            self.slots[id as usize].status = STATUS_FINISHED;
            self.notify(id, &mut change);
        }
        let callback = self.slots[id as usize].callback;
        if callback != SlotCallback::None {
            change.callbacks.push((id, callback));
        }
        change
    }

    /// `0x227E40`: flagged slots (bit 1 of +9) that are active or marked become finished.
    pub fn auto_finish_flagged(&mut self) {
        for &id in &self.order {
            let slot = &mut self.slots[id as usize];
            if slot.flags & 2 != 0 && matches!(slot.status, STATUS_ACTIVE | STATUS_MARKED) {
                slot.status = STATUS_FINISHED;
            }
        }
    }

    /// Per frame mission evaluation `0x228870` (Story mode only), after the slot callbacks ran.
    /// `session_flags` is `[0x7DE45C]`. Returns the event and the new session flags.
    pub fn evaluate_mission(
        &mut self,
        ctx: &ModeContext,
        session_flags: u32,
        all_players_down: bool,
    ) -> (Option<MissionEvent>, u32) {
        let evaluation = tsfp_state::story_flow::evaluate_mission(
            ctx.mode_id,
            ctx.players as i32,
            all_players_down,
            session_flags,
            self.counters[0],
            self.frame,
            self.order.iter().map(|id| {
                let slot = &self.slots[*id as usize];
                tsfp_state::story_flow::ObjectiveState {
                    included: slot.kind == 0,
                    status: slot.status,
                    flags: slot.flags,
                }
            }),
        );
        if evaluation.auto_finish_flagged {
            self.auto_finish_flagged();
        }
        let event = evaluation.event.map(|event| match event {
            tsfp_state::story_flow::MissionResultEvent::AllPlayersDown => {
                MissionEvent::AllPlayersDown
            }
            tsfp_state::story_flow::MissionResultEvent::ObjectivesFailed => {
                MissionEvent::ObjectivesFailed
            }
            tsfp_state::story_flow::MissionResultEvent::ObjectiveFailedPending => {
                MissionEvent::ObjectiveFailedPending
            }
            tsfp_state::story_flow::MissionResultEvent::Complete => MissionEvent::Complete,
        });
        (event, evaluation.result_flags)
    }

    /// Slot ids of event-link slots with a poll callback, in list order.
    pub fn event_link_slots(&self) -> Vec<(u32, usize, LinkSide)> {
        self.order
            .iter()
            .filter_map(|&id| match self.slots[id as usize].callback {
                SlotCallback::EventLink { instance, side } => Some((id, instance, side)),
                _ => None,
            })
            .collect()
    }
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    let raw = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

/// Parse the static slot table of a SETU entry (header triple at +0x84).
pub fn parse_slot_defs(blob: &[u8]) -> Result<Vec<SlotDef>, String> {
    let start = word(blob, SLOT_TABLE_TRIPLE).ok_or("short header")? as usize;
    let count = word(blob, SLOT_TABLE_TRIPLE + 4).ok_or("short header")? as usize;
    let stride = word(blob, SLOT_TABLE_TRIPLE + 8).ok_or("short header")? as usize;
    if stride != SLOT_DEF_STRIDE {
        return Err(format!("slot stride {stride:#x}, expected 0x38"));
    }
    let end = count
        .checked_mul(stride)
        .and_then(|size| start.checked_add(size))
        .ok_or("slot span overflow")?;
    if end > blob.len() {
        return Err("slot table past end of entry".into());
    }
    let field = |base: usize, offset: usize| {
        word(blob, base + offset).ok_or_else(|| "short slot".to_string())
    };
    (0..count)
        .map(|index| {
            let base = start + index * stride;
            Ok(SlotDef {
                id: field(base, 0)?,
                key: ObjectiveDefinitionKey(field(base, 4)?),
                mask: field(base, 8)?,
                label: field(base, 0xC)?,
                kinds: [field(base, 0x10)?, field(base, 0x14)?, field(base, 0x18)?],
                callback_id: field(base, 0x1C)?,
                dependencies: [
                    field(base, 0x20)?,
                    field(base, 0x24)?,
                    field(base, 0x28)?,
                    field(base, 0x2C)?,
                    field(base, 0x30)?,
                ],
                initially_active: field(base, 0x34)? != 0,
            })
        })
        .collect()
}

/// Mask used when the pool builder creates event slots (`push 0xE00077F` at `0x235821`).
pub fn event_slot_mask() -> u32 {
    MASK_EVENT_SLOT
}
