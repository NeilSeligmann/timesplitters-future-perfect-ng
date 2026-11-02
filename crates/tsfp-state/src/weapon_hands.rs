// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned weapon hand and equip state for the T2376 REA slice.
//!
//! The model deliberately keeps Inventory reserves, hand loaded/saved rounds, selection
//! requests, presentation controllers, and realised equipment as separate typed state. It has
//! no guest-memory offsets or implicit callback ABI. Callers provide the neighbouring record,
//! resource, and presentation contracts explicitly.

use crate::weapon_hands_inventory::{AMMO_TYPES, ENTRY_COUNT, FLAG_X2, Inventory, WeaponEntries};

pub const HAND_COUNT: usize = 2;
pub const CONTROLLER_COUNT: usize = 3;
pub const ENTRY_NONE: u32 = u32::MAX;

/// Typed access to signed reserve words. The legacy view adapts the original 16-bit cells;
/// Rust-owned subsystem state can pass its reserve array directly without a guest address.
pub trait ReserveState {
    fn reserve(&self, ammo_index: usize) -> i16;
    fn set_reserve(&mut self, ammo_index: usize, value: i16);
}

impl ReserveState for Inventory {
    fn reserve(&self, ammo_index: usize) -> i16 {
        self.ammo[ammo_index] as i16
    }

    fn set_reserve(&mut self, ammo_index: usize, value: i16) {
        self.ammo[ammo_index] = value as u16;
    }
}

impl ReserveState for [i16; AMMO_TYPES] {
    fn reserve(&self, ammo_index: usize) -> i16 {
        self[ammo_index]
    }

    fn set_reserve(&mut self, ammo_index: usize, value: i16) {
        self[ammo_index] = value;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AmmoType(u8);

impl AmmoType {
    pub const fn new(value: u8) -> Option<Self> {
        if value != 0 && (value as usize) < AMMO_TYPES {
            Some(Self(value))
        } else {
            None
        }
    }
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponRecordId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadChannel {
    Primary,
    Secondary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReloadChannelTransfer {
    pub channel: ReloadChannel,
    pub ammo_type: Option<AmmoType>,
    pub loaded_before: Option<i16>,
    pub loaded_after: Option<i16>,
    pub reserve_before: Option<i16>,
    pub reserve_after: Option<i16>,
    pub moved: i32,
}

/// Weapon record ammo metadata. Capacities are record dwords; loaded/reserve words are signed
/// 16-bit values, and the original compares the computed 32-bit difference as signed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponAmmoRecord {
    pub id: WeaponRecordId,
    pub primary: Option<AmmoType>,
    pub secondary: Option<AmmoType>,
    pub primary_capacity: i32,
    pub secondary_capacity: i32,
    pub primary_single_round: bool,
}

/// One synchronous reload transfer attached to an owned hand. A transfer commits its ammo
/// channels when applied; cancelling afterwards only prevents a later transfer and never rolls
/// back already committed reserve or loaded-round changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReloadSession {
    pub side: HandSide,
    pub saved_round_limit: bool,
    pub restrict_primary_to_one: bool,
    state: ReloadSessionState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReloadSessionState {
    Pending,
    Committed(i32),
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadSessionOutcome {
    Pending,
    Committed(i32),
    Cancelled,
}

impl ReloadSession {
    pub const fn new(
        side: HandSide,
        saved_round_limit: bool,
        restrict_primary_to_one: bool,
    ) -> Self {
        Self {
            side,
            saved_round_limit,
            restrict_primary_to_one,
            state: ReloadSessionState::Pending,
        }
    }

    pub const fn outcome(self) -> ReloadSessionOutcome {
        match self.state {
            ReloadSessionState::Pending => ReloadSessionOutcome::Pending,
            ReloadSessionState::Committed(moved) => ReloadSessionOutcome::Committed(moved),
            ReloadSessionState::Cancelled => ReloadSessionOutcome::Cancelled,
        }
    }

    /// Cancel only future work. A previously committed transfer remains observable and intact.
    pub fn cancel(&mut self) {
        if matches!(self.state, ReloadSessionState::Pending) {
            self.state = ReloadSessionState::Cancelled;
        }
    }

    pub fn apply(
        &mut self,
        hands: &mut WeaponHands,
        reserve: &mut impl ReserveState,
        active_record: WeaponAmmoRecord,
    ) -> ReloadSessionOutcome {
        if !matches!(self.state, ReloadSessionState::Pending) {
            return self.outcome();
        }
        let moved = hands.transfer_reload(
            reserve,
            self.side,
            active_record,
            self.saved_round_limit,
            self.restrict_primary_to_one,
        );
        self.state = ReloadSessionState::Committed(moved);
        self.outcome()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandAmmoState {
    pub record: Option<WeaponRecordId>,
    pub loaded: [i16; AMMO_TYPES],
    pub saved_rounds: [i16; AMMO_TYPES],
    pub reload_timer: i32,
    /// Hand-relative +0xF0/+0xF4 dwords. REA measured an integer decrement branch,
    /// but their units and lifecycle are unresolved, so this owner keeps them opaque.
    pub hand_counters: [i32; 2],
    pub child: Option<u32>,
}

impl Default for HandAmmoState {
    fn default() -> Self {
        Self::empty()
    }
}

impl HandAmmoState {
    pub const fn empty() -> Self {
        Self {
            record: None,
            loaded: [0; AMMO_TYPES],
            saved_rounds: [0; AMMO_TYPES],
            reload_timer: 1000,
            hand_counters: [1000; 2],
            child: None,
        }
    }
}

/// Modeled Rust-owned state detached by a hand reset. The previous child handle is returned to
/// the caller so the neighboring scene/object owner can decide its lifecycle disposition before
/// the hand forgets it; this receipt does not claim that the original reset body releases it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandResetReceipt {
    pub side: HandSide,
    pub previous: HandAmmoState,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponControllerState {
    pub state: u32,
    pub timer: u32,
    pub flags: u32,
    pub record: Option<WeaponRecordId>,
    pub hand: Option<usize>,
    /// Typed form of the controller's attached/current target handle, cleared by the reload wrapper.
    pub attachment_target: Option<u32>,
}

/// Fully typed Rust-owned hand slice. Inventory keeps reserve, ownership, desired selection and
/// equipped-once bytes; this owner keeps the two distinct hand records and realised selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponHands {
    pub hands: [HandAmmoState; HAND_COUNT],
    pub controllers: [WeaponControllerState; CONTROLLER_COUNT],
    pub realised_entry: Option<usize>,
    pub equipped_once: [bool; ENTRY_COUNT],
}

impl Default for WeaponHands {
    fn default() -> Self {
        Self {
            hands: [HandAmmoState::empty(); HAND_COUNT],
            controllers: [WeaponControllerState::default(); CONTROLLER_COUNT],
            realised_entry: None,
            equipped_once: [false; ENTRY_COUNT],
        }
    }
}

impl WeaponHands {
    pub fn hand(&self, side: HandSide) -> &HandAmmoState {
        &self.hands[side.index()]
    }
    pub fn hand_mut(&mut self, side: HandSide) -> &mut HandAmmoState {
        &mut self.hands[side.index()]
    }

    /// Apply the bounded counter branch from original `0x1BC890` to one owned hand.
    ///
    /// The original calls `0x59D20(owner, -1)` and decrements both hand-relative `+0xF0` and
    /// `+0xF4` dwords only when that helper returns zero. Their units and lifecycle are still
    /// unresolved, so the caller supplies the observed predicate result and this method does
    /// not interpret either counter as a duration or tick value.
    pub fn decrement_hand_counters_if_owner_gate_zero(
        &mut self,
        side: HandSide,
        owner_gate_result: u32,
    ) -> bool {
        if owner_gate_result != 0 {
            return false;
        }
        for counter in &mut self.hand_mut(side).hand_counters {
            *counter = counter.wrapping_sub(1);
        }
        true
    }

    /// Verification Tier 1: 2,048 seeded differential cases against the retained translated reference.
    /// 0x1B6E20. Count what a reload request can move. The measured helper's secondary missing
    /// calculation uses primary capacity, while the actual transfer below uses secondary
    /// capacity. Keep the asymmetry explicit until the original contract changes.
    pub fn reloadable_count(
        &self,
        inventory: &impl ReserveState,
        side: HandSide,
        record: WeaponAmmoRecord,
    ) -> i32 {
        let hand = self.hand(side);
        let mut total = 0i32;
        if let Some(ammo) = record.primary {
            let reserve = inventory.reserve(ammo.index());
            if reserve > 0 && record.primary_capacity != 0 && record.primary_capacity != 1 {
                let missing = record
                    .primary_capacity
                    .wrapping_sub(hand.loaded[ammo.index()] as i32);
                total = total.wrapping_add((reserve as i32).min(missing));
            }
        }
        if let Some(ammo) = record.secondary {
            if Some(ammo) != record.primary
                && record.secondary_capacity != 0
                && record.secondary_capacity != 1
            {
                let reserve = inventory.reserve(ammo.index());
                if reserve > 0 {
                    let missing = record
                        .primary_capacity
                        .wrapping_sub(hand.loaded[ammo.index()] as i32);
                    total = total.wrapping_add((reserve as i32).min(missing));
                }
            }
        }
        total
    }

    /// Verification Tier 1: 2,048 seeded differential cases against the lifted C2R entrypoint.
    /// 0x1B6EC0. Move reserve to one hand using the actual channel capacity. A saved-round limit
    /// is used only by the switch/restore path. Arithmetic wraps as 16-bit words and is signed
    /// only for the original comparisons; malformed over-capacity state can move a negative
    /// amount, matching the measured routine.
    pub fn transfer_reload(
        &mut self,
        inventory: &mut impl ReserveState,
        side: HandSide,
        record: WeaponAmmoRecord,
        saved_round_limit: bool,
        restrict_primary_to_one: bool,
    ) -> i32 {
        self.hand_mut(side).reload_timer = 0;
        let primary = self.transfer_reload_channel(
            inventory,
            side,
            record,
            ReloadChannel::Primary,
            saved_round_limit,
            restrict_primary_to_one,
        );
        let secondary = self.transfer_reload_channel(
            inventory,
            side,
            record,
            ReloadChannel::Secondary,
            saved_round_limit,
            restrict_primary_to_one,
        );
        primary.moved.wrapping_add(secondary.moved)
    }

    /// Commit one source-ordered channel of `0x1B6EC0`. This permits the native reload owner to
    /// retain a completed channel if a later callback cancels before the next channel.
    pub fn transfer_reload_channel(
        &mut self,
        inventory: &mut impl ReserveState,
        side: HandSide,
        record: WeaponAmmoRecord,
        channel: ReloadChannel,
        saved_round_limit: bool,
        restrict_primary_to_one: bool,
    ) -> ReloadChannelTransfer {
        let (ammo, capacity, primary) = match channel {
            ReloadChannel::Primary => (record.primary, record.primary_capacity, true),
            ReloadChannel::Secondary => (record.secondary, record.secondary_capacity, false),
        };
        let Some(ammo) =
            ammo.filter(|ammo| channel == ReloadChannel::Primary || Some(*ammo) != record.primary)
        else {
            return ReloadChannelTransfer {
                channel,
                ammo_type: None,
                loaded_before: None,
                loaded_after: None,
                reserve_before: None,
                reserve_after: None,
                moved: 0,
            };
        };

        let index = ammo.index();
        let hand = self.hand_mut(side);
        let reserve_before = inventory.reserve(index);
        let loaded_before = hand.loaded[index];
        let mut amount = 0i32;
        if reserve_before > 0 && capacity != 0 && capacity != 1 {
            let missing = capacity.wrapping_sub(loaded_before as i32);
            amount = (reserve_before as i32).min(missing);
            if saved_round_limit {
                amount = amount.min(hand.saved_rounds[index] as i32);
            }
            if primary && restrict_primary_to_one && record.primary_single_round && amount > 1 {
                amount = 1;
            }
        }
        let reserve_after = reserve_before.wrapping_sub(amount as i16);
        let loaded_after = loaded_before.wrapping_add(amount as i16);
        // Match the measured 0x1B6EC0 store order: credit loaded rounds before reserve debit.
        hand.loaded[index] = loaded_after;
        inventory.set_reserve(index, reserve_after);

        ReloadChannelTransfer {
            channel,
            ammo_type: Some(ammo),
            loaded_before: Some(loaded_before),
            loaded_after: Some(loaded_after),
            reserve_before: Some(reserve_before),
            reserve_after: Some(reserve_after),
            moved: amount,
        }
    }

    /// Verification Tier 1: 2,048 seeded differential cases against the lifted C2R entrypoint.
    /// 0x1B9760. Return the hand's active record ammo to reserve without a maximum clamp. The
    /// sequential channel order matters when both weapon-record fields name the same ammo type.
    pub fn unload_without_clamp(
        &mut self,
        inventory: &mut impl ReserveState,
        side: HandSide,
        record: WeaponAmmoRecord,
    ) {
        let hand = self.hand_mut(side);
        if hand.record != Some(record.id) {
            return;
        }
        for ammo in [record.primary, record.secondary].into_iter().flatten() {
            let index = ammo.index();
            let loaded = hand.loaded[index];
            let reserve = inventory.reserve(index);
            inventory.set_reserve(index, reserve.wrapping_add(loaded));
            hand.loaded[index] = 0;
        }
    }

    /// Verification Tier 0: compile and typed smoke test for the state-3 holster edge.
    /// 0x1C3100 state-3 holster edge. Save the active record's primary/secondary rounds, then
    /// return all loaded ammo types 1..43 to capped reserve and clear each loaded cell.
    pub fn holster(
        &mut self,
        inventory: &mut impl ReserveState,
        side: HandSide,
        record: WeaponAmmoRecord,
        reserve_maximum: &[i16; AMMO_TYPES],
        desired_entry: usize,
    ) {
        let hand = self.hand_mut(side);
        if hand.record == Some(record.id) {
            for ammo in [record.primary, record.secondary].into_iter().flatten() {
                hand.saved_rounds[ammo.index()] = hand.loaded[ammo.index()];
            }
        }
        for index in 1..AMMO_TYPES {
            let loaded = hand.loaded[index];
            let reserve = inventory.reserve(index);
            let cap = reserve_maximum[index];
            let next = reserve.wrapping_add(loaded);
            inventory.set_reserve(index, if next > cap { cap } else { next });
            hand.loaded[index] = 0;
        }
        self.realised_entry = Some(desired_entry);
    }

    /// Verification Tier 0: compile and typed smoke test for saved-round restoration.
    /// 0x1BDFD0's saved-round restoration core: clear all loaded cells, then transfer with the
    /// saved-round cap. Controller/resource resets are explicit separate operations.
    pub fn restore_saved(
        &mut self,
        inventory: &mut impl ReserveState,
        side: HandSide,
        record: WeaponAmmoRecord,
        restrict_primary_to_one: bool,
    ) -> i32 {
        self.hand_mut(side).loaded = [0; AMMO_TYPES];
        self.transfer_reload(inventory, side, record, true, restrict_primary_to_one)
    }

    /// 0x1BF9E0 handoff setup: raw request change snapshots the old desired value as realised,
    /// then schedules all three controller transitions. Selection normalization is a separate
    /// 0x1BA130 contract and is deliberately performed by the caller before this method.
    pub fn begin_handoff(&mut self, inventory: &Inventory, raw_entry: usize) -> bool {
        if inventory.current as usize == raw_entry {
            return false;
        }
        self.realised_entry = Some(inventory.current as usize);
        for controller in &mut self.controllers {
            controller.state = 3;
            controller.timer = 0;
        }
        true
    }

    /// Commit only after a hand/controller transition has completed; a request is not a receipt.
    pub fn commit_realised(&mut self, entry: usize, inventory: &mut Inventory) {
        self.realised_entry = Some(entry);
        inventory.previous = entry as u32;
        if entry < ENTRY_COUNT {
            self.equipped_once[entry] = true;
            inventory.equipped[entry] = 1;
        }
    }

    /// Commit the equipped-once cell after the adjacent event/callback boundary has completed.
    /// This mirrors the final 0x1BD820 store without repeating its external presentation effects.
    pub fn mark_equipped_once(&mut self, entry: usize, inventory: &mut Inventory) {
        if entry < ENTRY_COUNT {
            self.equipped_once[entry] = true;
            inventory.equipped[entry] = 1;
        }
    }

    /// Verification Tier 0: compile and typed smoke test for the hand initialization contract.
    /// 0x1B7A30 typed default state. Subordinate +0x114 initialization remains an explicit
    /// neighboring effect because that child's contract is owned by another system.
    pub fn initialize_hand(&mut self, side: HandSide) {
        *self.hand_mut(side) = HandAmmoState::empty();
    }

    /// Reset the modeled hand state and preserve the prior state for its neighboring lifecycle
    /// owner. T1678's bounded 0x1B7A30 read clears the 0x130-byte record and writes the child/model
    /// sentinel without a release call in that body. The receipt lets native callers make that
    /// ownership transition explicit; it does not infer whether a caller already released it.
    pub fn reset_hand(&mut self, side: HandSide) -> HandResetReceipt {
        let previous = core::mem::replace(self.hand_mut(side), HandAmmoState::empty());
        HandResetReceipt { side, previous }
    }

    /// Verification Tier 0: compile and typed smoke test for the reload/reset seam.
    /// 0x1B7BC0's typed reload/reset seam. The hand transfer is complete before the neighboring
    /// controller reset, and the controller record stays separate from the hand record.
    pub fn reload_then_reset(
        &mut self,
        inventory: &mut impl ReserveState,
        side: HandSide,
        controller: usize,
        record: WeaponAmmoRecord,
        saved_round_limit: bool,
        restrict_primary_to_one: bool,
    ) -> i32 {
        let moved = self.transfer_reload(
            inventory,
            side,
            record,
            saved_round_limit,
            restrict_primary_to_one,
        );
        self.finish_reload_controller(side, controller);
        moved
    }

    /// Apply the controller cleanup that follows a completed synchronous reload wrapper.
    pub fn finish_reload_controller(&mut self, side: HandSide, controller: usize) {
        if let Some(state) = self.controllers.get_mut(controller) {
            state.flags = 0;
            state.attachment_target = None;
            state.timer = 1000;
        }
        let hand = self.hand_mut(side);
        hand.reload_timer = 0;
        hand.hand_counters = [1000; 2];
    }

    /// 0x1B7A30 delegates child-state initialization after setting hand defaults.
    pub fn initialize_hand_with(
        &mut self,
        side: HandSide,
        initialize_subordinate: impl FnOnce(HandSide),
    ) {
        self.initialize_hand(side);
        initialize_subordinate(side);
    }

    /// Verification Tier 0: compile and typed smoke test for same-group x2 ownership.
    /// 0x1B68F0. Search all owned entries for an x2 member of the candidate's computed group.
    pub fn has_owned_x2(
        &self,
        inventory: &Inventory,
        entries: &WeaponEntries,
        candidate: usize,
    ) -> bool {
        if candidate >= ENTRY_COUNT {
            return false;
        }
        let group = entries.entries[candidate].group();
        (0..ENTRY_COUNT).any(|index| {
            inventory.owned[index] != 0
                && entries.entries[index].group() == group
                && entries.entries[index].flags() & FLAG_X2 != 0
        })
    }

    /// Verification Tier 0: compile and typed smoke test for availability policy.
    /// 0x1B6950 ordinary branch. This checks record ammo-type fields, not reserve counts.
    pub fn record_is_ammo_free(record: WeaponAmmoRecord) -> bool {
        record.primary.is_none() && record.secondary.is_none()
    }

    /// 0x1B6950 entry policy with 0xE02B0's owner-count result supplied by the pool boundary.
    pub fn special_entry_available(
        entry: usize,
        record: WeaponAmmoRecord,
        owned_charge_count: u32,
    ) -> bool {
        if entry == 0x2d {
            owned_charge_count != 0
        } else {
            Self::record_is_ammo_free(record)
        }
    }

    /// Verification Tier 0: compile and typed smoke test for the bounded owner-count predicate.
    /// 0xE02B0 typed pool predicate. The caller provides only records within the valid pool range.
    pub fn count_owned_detonators(owner: u32, objects: &[OwnedObject]) -> u32 {
        objects
            .iter()
            .filter(|object| {
                object.kind == 16
                    && object.state == 16
                    && object.owner != 0
                    && object.owner == owner
            })
            .count() as u32
    }

    /// Verification Tier 0: compile and typed smoke test for reload eligibility.
    /// 0x1BC990's measured ammo gate, before the controller's state transition.
    pub fn reload_request_allowed(
        &self,
        inventory: &impl ReserveState,
        side: HandSide,
        record: WeaponAmmoRecord,
    ) -> bool {
        self.reloadable_count(inventory, side, record) > 0
    }

    /// Verification Tier 0: compile and typed smoke test for post-event equipped-once commit.
    /// 0x1BD820: callbacks/events are owned by the adjacent record/presentation boundary; the
    /// equipped-once byte is committed after those callbacks, even when a visual branch is skipped.
    pub fn enter_realised_state<E: HandEventEffects>(
        &mut self,
        inventory: &mut Inventory,
        entry: usize,
        controller: usize,
        selector: u32,
        phase: u32,
        effects: &mut E,
    ) {
        effects.enter_transition(controller, selector, phase);
        if entry < ENTRY_COUNT {
            self.equipped_once[entry] = true;
            inventory.equipped[entry] = 1;
        }
    }

    /// Verification Tier 0: compile and typed smoke test for mode-resolved refill grants.
    /// 0x1BF000's mode-specific inputs are resolved before entering this typed operation. It
    /// applies signed reserve additions under caller-supplied limits, reloads each realised hand,
    /// and returns the moved total for the adjacent sound/effect policy.
    pub fn refill_and_reload(
        &mut self,
        inventory: &mut Inventory,
        grants: &[ReserveGrant],
        records: [Option<WeaponAmmoRecord>; HAND_COUNT],
        restrict_primary_to_one: bool,
    ) -> RefillOutcome {
        if inventory.current == 0 {
            return RefillOutcome::default();
        }
        let mut reserve_delta = 0i32;
        for grant in grants {
            let index = grant.ammo.index();
            let old = inventory.ammo[index] as i16;
            let added = old.wrapping_add(grant.amount);
            let next = if added > grant.maximum {
                grant.maximum
            } else {
                added
            };
            inventory.ammo[index] = next as u16;
            reserve_delta = reserve_delta.wrapping_add(next.wrapping_sub(old) as i32);
        }
        let mut rounds_loaded = 0i32;
        for (index, record) in records.into_iter().enumerate() {
            if let Some(record) = record {
                rounds_loaded = rounds_loaded.wrapping_add(self.transfer_reload(
                    inventory,
                    if index == 0 {
                        HandSide::Right
                    } else {
                        HandSide::Left
                    },
                    record,
                    false,
                    restrict_primary_to_one,
                ));
            }
        }
        RefillOutcome {
            reserve_delta,
            rounds_loaded,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandSide {
    Right,
    Left,
}

impl HandSide {
    pub const fn index(self) -> usize {
        match self {
            Self::Right => 0,
            Self::Left => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnedObject {
    pub kind: u32,
    pub state: u32,
    pub owner: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReserveGrant {
    pub ammo: AmmoType,
    pub amount: i16,
    pub maximum: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefillOutcome {
    pub reserve_delta: i32,
    pub rounds_loaded: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandRealiseError {
    MissingPreviousRecord(WeaponRecordId),
}

/// Per-hand publication result from one `0x1BE870` realization. `created_child: None` preserves
/// the neighboring resource owner's no-child result after the old child was retired and the new
/// identity was published; callers must not infer rollback from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandRealisePublication {
    Unchanged {
        record: WeaponRecordId,
        current_child: Option<u32>,
    },
    Replaced {
        record: WeaponRecordId,
        retired_child: Option<u32>,
        created_child: Option<u32>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandRealiseOutcome {
    pub publication: HandRealisePublication,
    pub transferred_rounds: i32,
}

/// Resolved per-hand inputs supplied by the weapon-definition owner for one actor update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandRealiseRequest {
    pub previous: Option<WeaponAmmoRecord>,
    pub next: WeaponAmmoRecord,
    pub restrict_primary_to_one: bool,
}

/// The actor update realizes right then left as separate commits. A failure on one side does not
/// suppress the other side's call, matching the measured caller's two independent invocations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandRealisePairOutcome {
    pub right: Result<HandRealiseOutcome, HandRealiseError>,
    pub left: Result<HandRealiseOutcome, HandRealiseError>,
}

pub trait HandEventEffects {
    fn enter_transition(&mut self, controller: usize, selector: u32, phase: u32);
}

/// Native resource/presentation attachment points for the actor/side realization path (0x1BE870).
/// Verification Tier 0: compile and typed smoke test for hand resource/presentation effects.
pub trait HandPresentation {
    fn release_child(&mut self, child: u32);
    /// Request a child after the owner has unloaded and published the replacement record.
    fn create_child(&mut self, record: WeaponRecordId, hand: HandAmmoState) -> Option<u32>;
    fn attach_child(&mut self, side: HandSide, child: Option<u32>);
    /// Observe one fully committed hand update. Actor updates invoke this for the right hand
    /// before beginning the left hand; observers must treat each callback as a per-hand commit,
    /// since REA has not established an atomic two-hand snapshot.
    fn hand_committed(&mut self, _side: HandSide, _outcome: HandRealiseOutcome) {}
}

impl WeaponHands {
    /// Change a realised hand record using Rust-owned state and explicit presentation effects.
    /// The bounded `0x1BE870` static order releases/clears the old child, unloads the old record,
    /// publishes the new record, then requests the new child. `HandRealiseOutcome` keeps each
    /// hand's publication result explicit when a child is not returned; this does not claim
    /// rollback or consumer-visible atomicity. The placement of the possible `0x1B6EC0` transfer
    /// relative to post-create callbacks remains unverified.
    pub fn realise_hand<P: HandPresentation>(
        &mut self,
        inventory: &mut impl ReserveState,
        side: HandSide,
        old_record: Option<WeaponAmmoRecord>,
        next: WeaponAmmoRecord,
        presentation: &mut P,
        restrict_primary_to_one: bool,
    ) -> Result<HandRealiseOutcome, HandRealiseError> {
        let old = self.hand(side).record;
        if old == Some(next.id) {
            return Ok(HandRealiseOutcome {
                publication: HandRealisePublication::Unchanged {
                    record: next.id,
                    current_child: self.hand(side).child,
                },
                transferred_rounds: 0,
            });
        }
        let prior = match old {
            Some(id) => Some(
                old_record
                    .filter(|record| record.id == id)
                    .ok_or(HandRealiseError::MissingPreviousRecord(id))?,
            ),
            None => None,
        };
        let retired_child = self.hand(side).child;
        if let Some(child) = retired_child {
            presentation.release_child(child);
            self.hand_mut(side).child = None;
        }
        if let Some(prior) = prior {
            self.unload_without_clamp(inventory, side, prior);
        }
        // The measured 0x1BE870 replacement path publishes the new hand identity before it
        // requests the replacement child. A failed request keeps that identity and stores an
        // empty child; there is no rollback to the released child or prior record.
        {
            self.hand_mut(side).record = Some(next.id);
        }
        let child = presentation.create_child(next.id, *self.hand(side));
        {
            let hand = self.hand_mut(side);
            hand.child = child;
        }
        presentation.attach_child(side, child);
        let transferred_rounds =
            self.transfer_reload(inventory, side, next, false, restrict_primary_to_one);
        Ok(HandRealiseOutcome {
            publication: HandRealisePublication::Replaced {
                record: next.id,
                retired_child,
                created_child: child,
            },
            transferred_rounds,
        })
    }

    /// Apply the two record replacements issued by actor update `0x2B8F90`.
    ///
    /// T2367 measures `0x1BE870(actor, 1)` before `0x1BE870(actor, 0)`, which selects right
    /// then left. Each side unloads and reloads independently; this method deliberately preserves
    /// that order and does not make the pair atomic. Verification Tier 0: typed state fixture.
    pub fn realise_actor_update<P: HandPresentation>(
        &mut self,
        inventory: &mut impl ReserveState,
        right: HandRealiseRequest,
        left: HandRealiseRequest,
        presentation: &mut P,
    ) -> HandRealisePairOutcome {
        let right = self.realise_hand(
            inventory,
            HandSide::Right,
            right.previous,
            right.next,
            presentation,
            right.restrict_primary_to_one,
        );
        if let Ok(outcome) = right {
            presentation.hand_committed(HandSide::Right, outcome);
        }
        let left = self.realise_hand(
            inventory,
            HandSide::Left,
            left.previous,
            left.next,
            presentation,
            left.restrict_primary_to_one,
        );
        if let Ok(outcome) = left {
            presentation.hand_committed(HandSide::Left, outcome);
        }
        HandRealisePairOutcome { right, left }
    }
}

/// The Rust owner for this slice composes the existing typed Inventory (ownership, reserve and
/// desired selection) with the new two-hand/realisation state. The reference host can continue
/// to own guest bytes until this system is explicitly selected by its A/B integration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponHandSystem {
    pub inventory: Inventory,
    pub hands: WeaponHands,
}

impl WeaponHandSystem {
    pub fn new(inventory: Inventory) -> Self {
        Self {
            inventory,
            hands: WeaponHands::default(),
        }
    }

    pub fn transfer_reload(
        &mut self,
        side: HandSide,
        record: WeaponAmmoRecord,
        saved_round_limit: bool,
        restrict_primary_to_one: bool,
    ) -> i32 {
        self.hands.transfer_reload(
            &mut self.inventory,
            side,
            record,
            saved_round_limit,
            restrict_primary_to_one,
        )
    }

    pub fn commit_realised(&mut self, entry: usize) {
        self.hands.commit_realised(entry, &mut self.inventory);
    }

    pub fn mark_equipped_once(&mut self, entry: usize) {
        self.hands.mark_equipped_once(entry, &mut self.inventory);
    }

    /// Publish a selection only after its original normalization result is available. The raw
    /// request decides whether a transition starts; the normalized entry becomes desired state.
    pub fn request_handoff(&mut self, raw_entry: usize, normalized_entry: usize) -> bool {
        if !self.hands.begin_handoff(&self.inventory, raw_entry) {
            return false;
        }
        self.inventory.current = normalized_entry as u32;
        true
    }
}

/// Typed state-3 controller transition used by the holster path. Desired selection is supplied
/// separately from the controller so UI/request publication cannot masquerade as completion.
pub fn complete_holster_transition(
    hands: &mut WeaponHands,
    inventory: &mut Inventory,
    side: HandSide,
    record: WeaponAmmoRecord,
    reserve_maximum: &[i16; AMMO_TYPES],
    desired_entry: usize,
    controller_index: usize,
) {
    hands.holster(inventory, side, record, reserve_maximum, desired_entry);
    if let Some(controller) = hands.controllers.get_mut(controller_index) {
        if controller.hand == Some(side.index()) && controller.state == 3 {
            controller.state = 0;
            controller.timer = 1000;
        }
    }
}
