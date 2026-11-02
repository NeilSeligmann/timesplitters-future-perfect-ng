// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned weapon inventory state and the measured ammo-pickup reserve transaction.
//!
//! This slice deliberately contains no guest address or guest-memory adapter. Callers resolve
//! immutable weapon/ammo rows and the realised hand records at the narrow seam. See
//! `docs/t2411-weapons-inventory.md` for evidence and remaining subsystem work.

use crate::object_pool::{ObjectId, REC_CAP};
use crate::scene_membership::{MembershipError, SceneMembership};
use crate::weapon_cycle::{WEAPON_CYCLE_SLOTS, WeaponCycleEntry, next_selectable};
use crate::weapon_hands::{
    AmmoType, HandRealiseOutcome, HandRealisePairOutcome, HandRealiseRequest, HandSide,
    ReloadSession as HandReloadSession, ReloadSessionOutcome as HandReloadSessionOutcome,
    WeaponAmmoRecord, WeaponHands,
};
use crate::weapon_payload::{
    C1950PayloadInputs, ShotAdmission, ShotAmmoSource, ShotSelection, WeaponPayloadLifecycle,
    WeaponPayloadLifecycleError, WeaponPayloadPose,
};

pub const ENTRY_COUNT: usize = 70;
pub const AMMO_TYPE_COUNT: usize = 44;
const ENTRY_FLAG_X2: u32 = 0x04;
const ENTRY_FLAG_LEFT: u32 = 0x08;
const ENTRY_FLAG_ACQUISITION_MASK: u32 = 0x210;

/// The supported reserve-cap fields from one ammo row. The rest of its 0xe0-byte record remains
/// an opaque neighboring definition/resource contract until its consumers are established.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmmoReservePolicy {
    base_maximum: i32,
    frame_scaled: bool,
}

impl AmmoReservePolicy {
    pub const fn from_row_fields(base_maximum: i32, row_flags: u8) -> Self {
        Self {
            base_maximum,
            frame_scaled: row_flags & 1 != 0,
        }
    }

    pub const fn is_frame_scaled(self) -> bool {
        self.frame_scaled
    }

    /// Apply the row's measured optional frame-scale cap in the original multiplication order.
    /// Callers supply values from the neighboring runtime clock/scale owner.
    pub fn maximum(self, frame_count: i32, frame_scale: f32) -> i32 {
        if self.frame_scaled {
            ((self.base_maximum as f32) * (frame_count as f32) * frame_scale) as i32
        } else {
            self.base_maximum
        }
    }
}

/// One pair from the pickup extension (`type`, `amount`). Values retain their source dword form
/// until the original positive/range guards and the short-argument narrowing have been applied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PickupAmmoPair {
    pub ammo_type: u32,
    pub amount: u32,
}

/// One `0x1BA220` call resolved from a world-weapon pickup. The caller supplies table-derived
/// facts and the already narrowed signed argument; the owner supplies loaded rounds and reserve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponPickupAmmo {
    pub ammo_type: AmmoType,
    pub amount: i16,
    pub reserve_maximum: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponPickupAmmoMutation {
    pub ammo_type: AmmoType,
    pub reserve_before: i16,
    pub reserve_after: i16,
    pub loaded_at_pickup: i32,
    pub reserve_maximum: i32,
    pub reserve_delta: i32,
}

/// Inventory-side result of the measured `0x0C0310` partial-commit decision. `inventory_accepted`
/// describes the callback's accept branch; world-object retirement remains a separate owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponPickupOutcome {
    pub entry_valid: bool,
    pub previously_owned: bool,
    pub ammo: [Option<WeaponPickupAmmoMutation>; 2],
    /// Signed sum returned by the two reserve helper calls, in source order.
    pub reserve_delta_total: i32,
    /// Diagnostic: at least one individual helper call returned a positive delta.
    pub reserve_increased: bool,
    /// The callback's zero-total duplicate gate was reached after both reserve calls.
    pub duplicate_predicate_consulted: bool,
    /// Native neighboring policy result used only when the duplicate total is zero.
    pub duplicate_predicate_passed: bool,
    pub inventory_accepted: bool,
    pub grant_called: bool,
    /// The accept branch calls `0x1BFCA0`, but its definition row was unavailable to this owner.
    pub grant_unresolved: bool,
    pub grant: Option<CollectEntryOutcome>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadSessionState {
    Pending,
    Complete,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadSessionAdvance {
    TransferProcessed {
        state: ReloadSessionState,
        total_moved: i32,
    },
    Terminal {
        state: ReloadSessionState,
        total_moved: i32,
    },
}

/// One generation-scoped reload sequence. The active record is supplied when the synchronous
/// transfer runs, so a request does not pin a stale definition across its animation delay.
#[derive(Debug, PartialEq, Eq)]
pub struct ReloadSession {
    owner: WeaponPlayerHandle,
    side: HandSide,
    controller: usize,
    saved_round_limit: bool,
    restrict_primary_to_one: bool,
    state: ReloadSessionState,
    total_moved: i32,
}

impl ReloadSession {
    pub const fn owner(&self) -> WeaponPlayerHandle {
        self.owner
    }

    pub const fn side(&self) -> HandSide {
        self.side
    }

    pub const fn state(&self) -> ReloadSessionState {
        self.state
    }

    pub const fn total_moved(&self) -> i32 {
        self.total_moved
    }
}

/// Resolved, immutable parts of one weapon entry needed by the measured ammo-acquisition walk.
/// `record_present` is supplied by the weapon-table owner after validating the main record index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventoryEntryDefinition {
    pub group: u32,
    pub flags: u32,
    pub record_present: bool,
    pub primary_ammo: Option<AmmoType>,
    pub secondary_ammo: Option<AmmoType>,
    /// First-acquisition saved-round quantities from the entry's two record ammo pairs.
    /// The table owner supplies the already narrowed signed words; `None` keeps the
    /// measured side effect explicitly unresolved for this definition.
    pub grant_saved_rounds: Option<GrantSavedRounds>,
}

/// Saved-round values associated with the primary and secondary ammo pairs in one entry record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GrantSavedRounds {
    pub primary: i16,
    pub secondary: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcquisitionResult {
    /// True when at least one previously unowned matching entry was granted.
    pub acquired: bool,
    /// Entries whose owned bytes changed, including companions granted with a group.
    pub newly_owned: [bool; ENTRY_COUNT],
    /// A new entry was granted but its record-derived saved-round values were not supplied.
    pub saved_rounds_unresolved: bool,
}

impl Default for AcquisitionResult {
    fn default() -> Self {
        Self {
            acquired: false,
            newly_owned: [false; ENTRY_COUNT],
            saved_rounds_unresolved: false,
        }
    }
}

/// The owned portion of one `0x1BFCA0` collect call plus its already-resolved selection edge.
/// The neighboring policy and entry-chain resolver decide whether to select and which normalized
/// entry `0x1BA130` would publish; hand realisation remains a later controller transition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollectEntryOutcome {
    pub previously_owned: bool,
    pub acquisition: AcquisitionResult,
    pub selection_requested: bool,
    pub selected_entry: Option<u8>,
    /// Selection was requested but its normalized entry was absent or out of range.
    pub selection_unresolved: bool,
}

/// Typed policy inputs resolved by the adjacent player-record, hand and weapon-table owners for
/// the ammo-restoration selection loop. Priority values already select the local `+8` or actor
/// `+4` table field; normalized entries are the result of the separate 1BA130 contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmmoAutoSelectionContext {
    pub profile_available: bool,
    pub player_index: i32,
    pub weapon_change_mode: u8,
    pub priorities: [Option<i32>; ENTRY_COUNT],
    pub matching_loaded_rounds_zero: [bool; ENTRY_COUNT],
    pub normalized_entries: [Option<u8>; ENTRY_COUNT],
}

impl Default for AmmoAutoSelectionContext {
    fn default() -> Self {
        Self {
            profile_available: false,
            player_index: -1,
            weapon_change_mode: 2,
            priorities: [None; ENTRY_COUNT],
            matching_loaded_rounds_zero: [false; ENTRY_COUNT],
            normalized_entries: [None; ENTRY_COUNT],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmmoAutoSelectionResult {
    /// Every entry whose original 1BA130 selection call was requested, in table order.
    pub selected_entries: [bool; ENTRY_COUNT],
    pub resulting_desired_entry: Option<u8>,
    /// First entry for which the resolved table facts were insufficient to continue faithfully.
    pub unresolved_at_entry: Option<usize>,
}

impl Default for AmmoAutoSelectionResult {
    fn default() -> Self {
        Self {
            selected_entries: [false; ENTRY_COUNT],
            resulting_desired_entry: None,
            unresolved_at_entry: None,
        }
    }
}

/// The two player fields committed by the fire-mode selector before its immediate fire handler.
/// Their selector policy and the input edge's clear/latch timing remain caller-owned contracts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FireModeSelection {
    pub mode_index: u32,
    pub selector: u32,
}

/// Result of advancing the desired weapon through the immutable cycle table.
/// The selector returns the original dword even if it cannot fit the typed entry owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CycleSelectionOutcome {
    pub selected_id: u32,
    pub desired_entry_committed: bool,
}

/// Definition-table facts resolved by the neighboring weapon-record owner for release repair.
/// `selectable` is the result of the original entry predicate after the released ownership bit
/// has been cleared; this owner does not reconstruct definition-table eligibility.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectionRepairCandidate {
    pub entry: usize,
    pub priority: i32,
    pub selectable: bool,
}

/// Result of releasing one inventory entry and, when it was selected, repairing desired state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryReleaseOutcome {
    pub entry: u8,
    pub was_owned: bool,
    pub selection_repaired: bool,
    pub resulting_desired_entry: Option<u8>,
}

impl FireModeSelection {
    pub const fn new(mode_index: u32, selector: u32) -> Self {
        Self {
            mode_index,
            selector,
        }
    }
}

/// Player values owned by the Rust weapons/inventory subsystem. Realised entry, loaded/saved
/// rounds, presentation controllers and equipped-once state stay in the adjacent `WeaponHands`
/// owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponInventory {
    pub desired_entry: Option<u8>,
    pub default_throwable: Option<u8>,
    /// The player interaction flag written by the measured A0D30 continuation.
    pub interaction_flag: bool,
    /// The resolved player `+0x5c/+0x60` mode selection; `None` means no owner supplied it yet.
    pub fire_mode_selection: Option<FireModeSelection>,
    /// The `ext+0x28` collect latch bit (mask `0x8`) owned separately from unknown runtime bits.
    pub collected_latch: bool,
    pub owned: [bool; ENTRY_COUNT],
    /// Signed interpretation of the original 16-bit reserve words.
    pub reserve: [i16; AMMO_TYPE_COUNT],
    pub hands: WeaponHands,
}

impl Default for WeaponInventory {
    fn default() -> Self {
        Self {
            desired_entry: None,
            default_throwable: None,
            interaction_flag: false,
            fire_mode_selection: None,
            collected_latch: false,
            owned: [false; ENTRY_COUNT],
            reserve: [0; AMMO_TYPE_COUNT],
            hands: WeaponHands::default(),
        }
    }
}

/// Profile-owned quantities copied by the bounded `0x25EC70` restore path.
///
/// The caller resolves the source entry and fallback before constructing this value. Weapon
/// record identities, saved rounds, capacities, metadata, and controller/presentation state are
/// deliberately absent: the retained static evidence does not establish these as copied profile
/// fields, and definition-derived facts stay with their adjacent owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileWeaponInventory {
    pub desired_entry: Option<u8>,
    pub owned: [bool; ENTRY_COUNT],
    pub reserve: [i16; AMMO_TYPE_COUNT],
    /// Indexed by `HandSide::index()` (right, then left), then by ammo type.
    pub loaded_rounds: [[i16; AMMO_TYPE_COUNT]; 2],
}

/// Stable player-slot identity for the host-owned weapon subsystem. The guest extension address
/// is an adapter input and must not be used as the persistent key for inventory state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WeaponPlayerId(u16);

impl WeaponPlayerId {
    pub const fn new(slot: u16) -> Self {
        Self(slot)
    }

    pub const fn slot(self) -> usize {
        self.0 as usize
    }
}

/// Generation-checked identity for one Rust-owned player inventory lifetime.
///
/// A slot number alone can name a later player after retirement. Native callers should retain
/// this handle and use the handle-based store operations so delayed work cannot mutate a reused
/// slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WeaponPlayerHandle {
    player: WeaponPlayerId,
    generation: u32,
}

impl WeaponPlayerHandle {
    /// Construct a handle only inside the state crate when a typed owner provides its parts.
    /// External callers must obtain handles from a lifetime-owning store.
    pub(crate) const fn from_parts(player: WeaponPlayerId, generation: u32) -> Self {
        Self { player, generation }
    }

    pub const fn player(self) -> WeaponPlayerId {
        self.player
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

/// Host-owned inventory values indexed by stable player slot, independent of guest allocation
/// addresses. The host chooses capacity from its player-slot contract; removing a slot explicitly
/// ends that player's inventory lifetime before the slot is reused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponInventoryStore<const PLAYER_SLOTS: usize> {
    players: [Option<WeaponInventory>; PLAYER_SLOTS],
    generations: [u32; PLAYER_SLOTS],
}

impl<const PLAYER_SLOTS: usize> Default for WeaponInventoryStore<PLAYER_SLOTS> {
    fn default() -> Self {
        Self {
            players: core::array::from_fn(|_| None),
            generations: [1; PLAYER_SLOTS],
        }
    }
}

impl<const PLAYER_SLOTS: usize> WeaponInventoryStore<PLAYER_SLOTS> {
    /// Get the state for a player slot, creating its Rust-owned inventory on first use.
    pub fn get_or_default(&mut self, player: WeaponPlayerId) -> Option<&mut WeaponInventory> {
        if self.generations.get(player.slot()).copied() == Some(0) {
            return None;
        }
        self.players
            .get_mut(player.slot())
            .map(|state| state.get_or_insert_with(WeaponInventory::default))
    }

    pub fn get(&self, player: WeaponPlayerId) -> Option<&WeaponInventory> {
        self.players.get(player.slot()).and_then(Option::as_ref)
    }

    /// Mutate an already-owned player inventory without implicitly starting a new lifetime.
    pub fn get_mut(&mut self, player: WeaponPlayerId) -> Option<&mut WeaponInventory> {
        self.players.get_mut(player.slot()).and_then(Option::as_mut)
    }

    /// Begin a distinct player lifetime and return its generation-checked identity.
    pub fn begin_lifetime(
        &mut self,
        player: WeaponPlayerId,
        inventory: WeaponInventory,
    ) -> Option<WeaponPlayerHandle> {
        let generation = *self.generations.get(player.slot())?;
        if generation == 0 {
            return None;
        }
        let owner = self.players.get_mut(player.slot())?;
        if owner.is_some() {
            return None;
        }
        *owner = Some(inventory);
        Some(WeaponPlayerHandle { player, generation })
    }

    /// Begin a fresh spawn lifetime with the resolved hands entry selected by the spawn owner.
    pub fn begin_spawn_lifetime(
        &mut self,
        player: WeaponPlayerId,
        hands_entry: usize,
    ) -> Option<WeaponPlayerHandle> {
        self.begin_lifetime(player, WeaponInventory::for_spawn(hands_entry)?)
    }

    /// Return a checked handle for the current lifetime in a slot.
    pub fn handle_for(&self, player: WeaponPlayerId) -> Option<WeaponPlayerHandle> {
        self.get(player)?;
        let generation = *self.generations.get(player.slot())?;
        (generation != 0).then_some(WeaponPlayerHandle { player, generation })
    }

    pub fn get_handle(&self, handle: WeaponPlayerHandle) -> Option<&WeaponInventory> {
        if self.generations.get(handle.player.slot()).copied() != Some(handle.generation) {
            return None;
        }
        self.get(handle.player)
    }

    pub fn get_handle_mut(&mut self, handle: WeaponPlayerHandle) -> Option<&mut WeaponInventory> {
        if self.generations.get(handle.player.slot()).copied() != Some(handle.generation) {
            return None;
        }
        self.get_mut(handle.player)
    }

    /// Restore copied profile quantities into the exact live Rust-owned player lifetime.
    /// A stale handle cannot apply a profile after its player slot has been reused.
    pub fn restore_profile_for_lifetime(
        &mut self,
        handle: WeaponPlayerHandle,
        profile: ProfileWeaponInventory,
    ) -> bool {
        let Some(inventory) = self.get_handle_mut(handle) else {
            return false;
        };
        inventory.restore_profile(profile);
        true
    }

    /// Realise both hand records for one live player generation in measured right-then-left
    /// order. Record resolution and presentation resource work remain typed neighboring inputs.
    pub fn realise_actor_update_for_lifetime<P: crate::weapon_hands::HandPresentation>(
        &mut self,
        handle: WeaponPlayerHandle,
        right: HandRealiseRequest,
        left: HandRealiseRequest,
        presentation: &mut P,
    ) -> Option<HandRealisePairOutcome> {
        let inventory = self.get_handle_mut(handle)?;
        Some(inventory.realise_actor_update(right, left, presentation))
    }

    /// Reset one hand only while its Rust-owned player generation is live.
    ///
    /// The receipt preserves former hand and child values for the neighboring resource owner.
    /// The reset itself does not release a child; current T1678 evidence does not establish
    /// whether callers release it before the original reset overwrites the handle.
    pub fn reset_hand_for_lifetime(
        &mut self,
        handle: WeaponPlayerHandle,
        side: HandSide,
    ) -> Option<crate::weapon_hands::HandResetReceipt> {
        Some(self.get_handle_mut(handle)?.reset_hand(side))
    }

    /// Select and commit the next entry against the exact Rust-owned player lifetime.
    /// Candidate order and table flags are resolved by the neighboring weapon-table owner;
    /// owned bits always come from this inventory, never from the adapter-shaped rows.
    pub fn select_cycle_entry_for_lifetime(
        &mut self,
        handle: WeaponPlayerHandle,
        entries: &[WeaponCycleEntry; WEAPON_CYCLE_SLOTS],
        previous_entry: u32,
    ) -> Option<CycleSelectionOutcome> {
        let owner = self.get_handle_mut(handle)?;
        let selected_id = owner.next_cycle_entry(entries, previous_entry);
        let desired_entry_committed = if selected_id < ENTRY_COUNT as u32 {
            owner.desired_entry = Some(selected_id as u8);
            true
        } else {
            false
        };
        Some(CycleSelectionOutcome {
            selected_id,
            desired_entry_committed,
        })
    }

    /// Release and repair one entry only for the exact live Rust-owned player generation.
    pub fn release_entry_for_lifetime(
        &mut self,
        handle: WeaponPlayerHandle,
        entry: usize,
        candidates: &[SelectionRepairCandidate; ENTRY_COUNT],
    ) -> Option<EntryReleaseOutcome> {
        self.get_handle_mut(handle)?
            .release_entry_and_repair_selection(entry, candidates)
    }

    /// Verification Tier 0: commit the resolved mode pair before the immediate Rust fire action.
    /// Selector eligibility and original input routing are not part of this fixture.
    /// Commit the resolved mode pair to a live Rust owner before the immediate fire action.
    /// A stale player generation declines without updating state or invoking the action.
    pub fn select_fire_mode_and_fire(
        &mut self,
        handle: WeaponPlayerHandle,
        selection: FireModeSelection,
        fire: impl FnOnce(&WeaponInventory),
    ) -> bool {
        let Some(owner) = self.get_handle_mut(handle) else {
            return false;
        };
        owner.fire_mode_selection = Some(selection);
        fire(owner);
        true
    }

    /// Start a typed reload sequence only for an existing player lifetime.
    pub fn begin_reload_session(
        &mut self,
        handle: WeaponPlayerHandle,
        side: HandSide,
        controller: usize,
        saved_round_limit: bool,
        restrict_primary_to_one: bool,
    ) -> Option<ReloadSession> {
        Some(self.get_handle_mut(handle)?.begin_reload_session(
            handle,
            side,
            controller,
            saved_round_limit,
            restrict_primary_to_one,
        ))
    }

    /// Commit both source-ordered ammo channels synchronously using the definition active at
    /// transfer time. A reload request does not snapshot a definition before its animation delay.
    pub fn advance_reload_session(
        &mut self,
        handle: WeaponPlayerHandle,
        session: &mut ReloadSession,
        active_record: WeaponAmmoRecord,
    ) -> Option<ReloadSessionAdvance> {
        if session.owner != handle {
            return None;
        }
        Some(
            self.get_handle_mut(handle)?
                .advance_reload_session(session, active_record),
        )
    }

    /// Cancel a pending reload before transfer. A completed synchronous transfer is not rolled back.
    pub fn cancel_reload_session(
        &mut self,
        handle: WeaponPlayerHandle,
        session: &mut ReloadSession,
    ) -> Option<ReloadSessionAdvance> {
        if session.owner != handle {
            return None;
        }
        self.get_handle_mut(handle)?;
        if matches!(session.state, ReloadSessionState::Pending) {
            session.state = ReloadSessionState::Cancelled;
        }
        Some(ReloadSessionAdvance::Terminal {
            state: session.state,
            total_moved: session.total_moved,
        })
    }

    /// Retire only the lifetime identified by this handle; stale handles leave a newer owner intact.
    pub fn retire_handle(&mut self, handle: WeaponPlayerHandle) -> Option<WeaponInventory> {
        if self.generations.get(handle.player.slot()).copied() != Some(handle.generation) {
            return None;
        }
        self.retire(handle.player)
    }

    /// End a slot's current player lifetime. A later `get_or_default` starts from clean state.
    pub fn retire(&mut self, player: WeaponPlayerId) -> Option<WeaponInventory> {
        let retired = self.players.get_mut(player.slot())?.take()?;
        let generation = self.generations.get_mut(player.slot())?;
        *generation = generation.checked_add(1).unwrap_or(0);
        Some(retired)
    }
}

#[cfg(test)]
mod entry_release_selection_tests {
    use super::{
        ENTRY_COUNT, EntryReleaseOutcome, SelectionRepairCandidate, WeaponInventory,
        WeaponInventoryStore, WeaponPlayerId,
    };

    fn candidates() -> [SelectionRepairCandidate; ENTRY_COUNT] {
        core::array::from_fn(|entry| SelectionRepairCandidate {
            entry,
            ..SelectionRepairCandidate::default()
        })
    }

    #[test]
    fn selected_entry_repair_uses_strict_lowest_priority_and_keeps_first_tie() {
        let mut inventory = WeaponInventory::default();
        inventory.owned[7] = true;
        inventory.desired_entry = Some(7);
        let mut candidates = candidates();
        candidates[7] = SelectionRepairCandidate {
            entry: 7,
            priority: 0,
            selectable: true,
        };
        candidates[2] = SelectionRepairCandidate {
            entry: 2,
            priority: 20,
            selectable: true,
        };
        candidates[3] = SelectionRepairCandidate {
            entry: 3,
            priority: 20,
            selectable: true,
        };
        candidates[4] = SelectionRepairCandidate {
            entry: 4,
            priority: 5,
            selectable: false,
        };
        candidates[5] = SelectionRepairCandidate {
            entry: 5,
            priority: 100,
            selectable: true,
        };

        assert_eq!(
            inventory.release_entry_and_repair_selection(7, &candidates),
            Some(EntryReleaseOutcome {
                entry: 7,
                was_owned: true,
                selection_repaired: true,
                resulting_desired_entry: Some(2),
            })
        );
        assert!(!inventory.owned[7]);
        assert_eq!(inventory.desired_entry, Some(2));
    }

    #[test]
    fn selected_entry_repair_falls_back_to_zero_when_no_row_is_eligible_below_100() {
        let mut inventory = WeaponInventory::default();
        inventory.desired_entry = Some(7);
        let mut candidates = candidates();
        candidates[2] = SelectionRepairCandidate {
            entry: 2,
            priority: 100,
            selectable: true,
        };
        candidates[3] = SelectionRepairCandidate {
            entry: 3,
            priority: 8,
            selectable: false,
        };

        let outcome = inventory
            .release_entry_and_repair_selection(7, &candidates)
            .expect("valid entry");
        assert!(outcome.selection_repaired);
        assert_eq!(outcome.resulting_desired_entry, Some(0));
        assert_eq!(inventory.desired_entry, Some(0));
    }

    #[test]
    fn nonselected_release_preserves_desired_and_stale_lifetime_is_rejected() {
        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        let first = store
            .begin_lifetime(player, WeaponInventory::default())
            .expect("initial lifetime");
        {
            let inventory = store.get_handle_mut(first).expect("live owner");
            inventory.owned[4] = true;
            inventory.owned[7] = true;
            inventory.desired_entry = Some(7);
        }
        let candidates = candidates();

        let released = store
            .release_entry_for_lifetime(first, 4, &candidates)
            .expect("live player");
        assert!(!released.selection_repaired);
        assert_eq!(released.resulting_desired_entry, Some(7));
        assert!(!store.get_handle(first).unwrap().owned[4]);

        assert!(store.retire_handle(first).is_some());
        let second = store
            .begin_lifetime(player, WeaponInventory::default())
            .expect("reused player slot");
        {
            let inventory = store.get_handle_mut(second).expect("new live owner");
            inventory.owned[7] = true;
            inventory.desired_entry = Some(7);
        }
        assert!(
            store
                .release_entry_for_lifetime(first, 7, &candidates)
                .is_none()
        );
        assert!(store.get_handle(second).unwrap().owned[7]);
        assert_eq!(store.get_handle(second).unwrap().desired_entry, Some(7));
    }
}

#[cfg(test)]
mod fire_mode_selection_tests {
    use super::{FireModeSelection, WeaponInventory, WeaponInventoryStore, WeaponPlayerId};

    #[test]
    fn fire_action_observes_committed_mode_and_stale_lifetime_is_rejected() {
        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        let first = store
            .begin_lifetime(player, WeaponInventory::default())
            .expect("first player lifetime");
        let selected = FireModeSelection::new(2, 7);
        let mut fired = false;

        assert!(store.select_fire_mode_and_fire(first, selected, |owner| {
            assert_eq!(owner.fire_mode_selection, Some(selected));
            fired = true;
        }));
        assert!(fired);

        assert!(store.retire_handle(first).is_some());
        let second = store
            .begin_lifetime(player, WeaponInventory::default())
            .expect("reused slot starts a new generation");
        let mut stale_fire_called = false;
        assert!(!store.select_fire_mode_and_fire(first, selected, |_| {
            stale_fire_called = true;
        }));
        assert!(!stale_fire_called);
        assert_eq!(store.get_handle(second).unwrap().fire_mode_selection, None);
    }
}

#[cfg(test)]
mod actor_hand_refresh_owner_tests {
    use super::{ENTRY_COUNT, WeaponInventory, WeaponInventoryStore, WeaponPlayerId};
    use crate::weapon_hands::{
        HandAmmoState, HandPresentation, HandRealiseRequest, HandSide, WeaponAmmoRecord,
        WeaponRecordId,
    };
    use alloc::vec::Vec;

    #[derive(Default)]
    struct Presentation(Vec<(HandSide, u16)>);

    impl HandPresentation for Presentation {
        fn release_child(&mut self, _child: u32) {}

        fn create_child(&mut self, record: WeaponRecordId, hand: HandAmmoState) -> Option<u32> {
            assert_eq!(hand.record, Some(record));
            assert_eq!(hand.child, None);
            Some(record.0 as u32)
        }

        fn attach_child(&mut self, side: HandSide, child: Option<u32>) {
            if let Some(record) = child {
                self.0.push((side, record as u16));
            }
        }
    }

    fn request(record: u16) -> HandRealiseRequest {
        HandRealiseRequest {
            previous: None,
            next: WeaponAmmoRecord {
                id: WeaponRecordId(record),
                ..WeaponAmmoRecord::default()
            },
            restrict_primary_to_one: false,
        }
    }

    #[test]
    fn actor_refresh_runs_right_then_left_only_for_the_current_generation() {
        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        let first = store
            .begin_lifetime(player, WeaponInventory::default())
            .expect("initial player lifetime");
        let mut presentation = Presentation::default();

        let outcome = store
            .realise_actor_update_for_lifetime(first, request(7), request(9), &mut presentation)
            .expect("live generation");
        assert_eq!(
            outcome.right.map(|outcome| outcome.transferred_rounds),
            Ok(0)
        );
        assert_eq!(
            outcome.left.map(|outcome| outcome.transferred_rounds),
            Ok(0)
        );
        assert_eq!(presentation.0, [(HandSide::Right, 7), (HandSide::Left, 9)]);
        let owner = store.get_handle(first).expect("same generation");
        assert_eq!(
            owner.hands.hand(HandSide::Right).record,
            Some(WeaponRecordId(7))
        );
        assert_eq!(
            owner.hands.hand(HandSide::Left).record,
            Some(WeaponRecordId(9))
        );

        assert!(store.retire_handle(first).is_some());
        let second = store
            .begin_lifetime(player, WeaponInventory::default())
            .expect("reused player slot");
        presentation.0.clear();
        assert!(
            store
                .realise_actor_update_for_lifetime(
                    first,
                    request(11),
                    request(13),
                    &mut presentation,
                )
                .is_none()
        );
        assert!(presentation.0.is_empty());
        assert!(store.get_handle(second).is_some());
    }

    #[test]
    fn spawn_inventory_selects_only_the_resolved_hands_entry() {
        let owner = WeaponInventory::for_spawn(3).expect("valid hands entry");
        assert_eq!(owner.desired_entry, Some(3));
        assert_eq!(owner.hands.realised_entry, None);
        assert_eq!(
            owner.hands.hand(HandSide::Right),
            &crate::weapon_hands::HandAmmoState::empty()
        );
        assert_eq!(
            owner.hands.hand(HandSide::Left),
            &crate::weapon_hands::HandAmmoState::empty()
        );
        assert!(owner.owned.iter().all(|owned| !owned));
        assert!(owner.reserve.iter().all(|amount| *amount == 0));
        assert!(WeaponInventory::for_spawn(ENTRY_COUNT).is_none());
    }

    #[test]
    fn spawn_lifetime_is_generation_checked_and_rejects_duplicate_start() {
        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        let first = store
            .begin_spawn_lifetime(player, 2)
            .expect("valid spawn owner");
        assert_eq!(store.get_handle(first).unwrap().desired_entry, Some(2));
        assert!(store.begin_spawn_lifetime(player, 3).is_none());
        assert!(store.begin_spawn_lifetime(player, ENTRY_COUNT).is_none());
        assert!(store.retire_handle(first).is_some());
        let second = store
            .begin_spawn_lifetime(player, 4)
            .expect("reused player slot");
        assert_ne!(first.generation(), second.generation());
        assert_eq!(store.get_handle(second).unwrap().desired_entry, Some(4));
    }

    #[test]
    fn hand_reset_returns_resource_receipt_only_for_the_current_generation() {
        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        let first = store
            .begin_lifetime(player, WeaponInventory::default())
            .expect("initial player lifetime");
        let ammo = crate::weapon_hands::AmmoType::new(3).expect("valid ammo type");
        {
            let owner = store.get_handle_mut(first).expect("live owner");
            let hand = owner.hands.hand_mut(HandSide::Right);
            hand.record = Some(WeaponRecordId(17));
            hand.child = Some(91);
            hand.loaded[ammo.index()] = 6;
        }

        let receipt = store
            .reset_hand_for_lifetime(first, HandSide::Right)
            .expect("live generation");
        assert_eq!(receipt.side, HandSide::Right);
        assert_eq!(receipt.previous.record, Some(WeaponRecordId(17)));
        assert_eq!(receipt.previous.child, Some(91));
        assert_eq!(receipt.previous.loaded[ammo.index()], 6);
        let owner = store.get_handle(first).expect("same generation");
        assert_eq!(owner.hands.hand(HandSide::Right).record, None);
        assert_eq!(owner.hands.hand(HandSide::Right).child, None);
        assert_eq!(owner.hands.hand(HandSide::Right).loaded[ammo.index()], 0);

        assert!(store.retire_handle(first).is_some());
        let second = store
            .begin_lifetime(player, WeaponInventory::default())
            .expect("reused player slot");
        assert!(
            store
                .reset_hand_for_lifetime(first, HandSide::Right)
                .is_none()
        );
        assert_eq!(
            store
                .get_handle(second)
                .unwrap()
                .hands
                .hand(HandSide::Right),
            &crate::weapon_hands::HandAmmoState::empty()
        );
    }
}

/// Shared Rust owner for the weapons and inventory subsystem's player state and payload poses.
/// Player lifetimes and object lifetimes retain their own generation-checked identities because
/// they are created and retired by neighbouring owners at different points in the game flow.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponSubsystemState<const PLAYER_SLOTS: usize> {
    /// Per-player hands, inventory, reserve, and equip state.
    pub players: WeaponInventoryStore<PLAYER_SLOTS>,
    /// Creation-time payload pose attached to native object-pool identities.
    pub payloads: WeaponPayloadLifecycle,
    /// Projectile descriptor owner, separate from attached payload poses and allocator policy.
    pub projectiles: crate::weapon_projectiles::WeaponProjectileStore,
    /// Interaction callbacks and mission markers attached to those same scene generations.
    /// The host still supplies callback execution and player effects at the typed boundary.
    pub interaction_targets: crate::weapon_interaction::InteractionTargetStore,
}

/// Failure while retiring a scene object that owns weapon payload state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponObjectRetireError {
    Scene(MembershipError),
    Payload(WeaponPayloadLifecycleError),
}

/// Failure while admitting a newly allocated scene identity as a weapon payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponPayloadSpawnError {
    Scene(MembershipError),
    Payload(WeaponPayloadLifecycleError),
    /// The pose could not attach and the new scene identity also failed to roll back.
    Rollback {
        payload: WeaponPayloadLifecycleError,
        scene: MembershipError,
    },
}

impl<const PLAYER_SLOTS: usize> WeaponSubsystemState<PLAYER_SLOTS> {
    /// Create one subsystem owner using the pool's measured maximum record capacity.
    pub fn new() -> Self {
        Self::with_object_capacity(REC_CAP)
    }

    /// Create one subsystem owner with the neighbouring object owner's configured capacity.
    pub fn with_object_capacity(object_slots: usize) -> Self {
        Self {
            players: WeaponInventoryStore::default(),
            payloads: WeaponPayloadLifecycle::with_capacity(object_slots),
            projectiles: crate::weapon_projectiles::WeaponProjectileStore::default(),
            interaction_targets: crate::weapon_interaction::InteractionTargetStore::default(),
        }
    }

    /// Begin weapon inventory state for one typed player generation.
    pub fn begin_player_lifetime(
        &mut self,
        player: WeaponPlayerId,
        inventory: WeaponInventory,
    ) -> Option<WeaponPlayerHandle> {
        self.players.begin_lifetime(player, inventory)
    }

    /// Begin a fresh spawn lifetime using its already-resolved hands entry.
    pub fn begin_player_lifetime_for_spawn(
        &mut self,
        player: WeaponPlayerId,
        hands_entry: usize,
    ) -> Option<WeaponPlayerHandle> {
        self.players.begin_spawn_lifetime(player, hands_entry)
    }

    /// Begin a player generation from the profile-owned quantities copied by the measured
    /// `0x25EC70` path. The caller supplies typed profile data; this native boundary does not
    /// read guest memory or resolve definition-derived fields.
    pub fn begin_player_lifetime_from_profile(
        &mut self,
        player: WeaponPlayerId,
        profile: ProfileWeaponInventory,
    ) -> Option<WeaponPlayerHandle> {
        let mut inventory = WeaponInventory::default();
        inventory.restore_profile(profile);
        self.begin_player_lifetime(player, inventory)
    }

    /// Replace the current weapon-inventory generation with profile-owned values.
    ///
    /// The hybrid restore adapter uses this when the retained profile restore begins a fresh
    /// gameplay lifetime for an already-used slot. PlayerSet interaction state has a separate
    /// generation owner and is not inferred from this weapon-player handle.
    pub fn replace_player_lifetime_from_profile(
        &mut self,
        player: WeaponPlayerId,
        profile: ProfileWeaponInventory,
    ) -> Option<WeaponPlayerHandle> {
        if let Some(current) = self.players.handle_for(player) {
            self.retire_player_lifetime(current)?;
        }
        self.begin_player_lifetime_from_profile(player, profile)
    }

    /// End one weapon-inventory generation.
    pub fn retire_player_lifetime(
        &mut self,
        handle: WeaponPlayerHandle,
    ) -> Option<WeaponInventory> {
        self.players.retire_handle(handle)
    }

    /// Continue one interaction through Rust-owned player and target state.
    ///
    /// `captured` is the generation-checked player/actor identity captured before this call, as
    /// A0D30 does before sending. The sender and callback receive typed owners so they can
    /// re-enter safely; the target callback and mission key are reacquired around those calls.
    pub fn submit_native_interaction<H: crate::weapon_interaction::NativeInteractionHandler>(
        &mut self,
        scene: &mut SceneMembership,
        captured: crate::weapon_interaction::CapturedInteractionPlayer,
        target: ObjectId,
        handler: &mut H,
    ) -> Result<
        crate::weapon_interaction::InteractionSubmissionOutcome,
        crate::weapon_interaction::NativeInteractionError<H::Error>,
    > {
        crate::weapon_interaction::submit_owned_interaction(
            &mut self.players,
            &mut self.interaction_targets,
            scene,
            captured,
            target,
            handler,
        )
    }

    /// Apply the inventory-owned part of one weapon pickup to a live Rust player generation.
    ///
    /// Ammo definitions and the two realised hand records are resolved by their typed owners.
    /// A stale player handle has no effects. The result covers reserve/acquisition state only;
    /// pickup world-object retirement remains with the neighbouring scene owner.
    pub fn collect_weapon_pickup_for_lifetime(
        &mut self,
        player: WeaponPlayerHandle,
        entry: usize,
        ammo_pairs: [Option<WeaponPickupAmmo>; 2],
        realised_hand_records: [Option<WeaponAmmoRecord>; 2],
        definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
        zero_delta_duplicate_predicate_passed: bool,
        select: bool,
        normalized_entry: Option<usize>,
    ) -> Option<WeaponPickupOutcome> {
        Some(
            self.players
                .get_handle_mut(player)?
                .collect_weapon_pickup_with_duplicate_predicate(
                    entry,
                    ammo_pairs,
                    realised_hand_records,
                    definitions,
                    zero_delta_duplicate_predicate_passed,
                    select,
                    normalized_entry,
                ),
        )
    }

    /// Apply a weapon pickup only while both its scene identity and player lifetime are live.
    ///
    /// The scene owner supplies an identity after confirming the object is a world weapon
    /// pickup. This boundary rejects callbacks captured before either object was retired or a
    /// player slot was reused. It returns the measured inventory decision; the neighboring scene
    /// owner still decides whether an accepted object is retired because the original callback's
    /// successful-return lifecycle is not established by REA.
    pub fn collect_weapon_pickup_for_object(
        &mut self,
        scene: &SceneMembership,
        object: ObjectId,
        player: WeaponPlayerHandle,
        entry: usize,
        ammo_pairs: [Option<WeaponPickupAmmo>; 2],
        realised_hand_records: [Option<WeaponAmmoRecord>; 2],
        definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
        zero_delta_duplicate_predicate_passed: bool,
        select: bool,
        normalized_entry: Option<usize>,
    ) -> Option<WeaponPickupOutcome> {
        if !scene.is_active(object) {
            return None;
        }
        self.collect_weapon_pickup_for_lifetime(
            player,
            entry,
            ammo_pairs,
            realised_hand_records,
            definitions,
            zero_delta_duplicate_predicate_passed,
            select,
            normalized_entry,
        )
    }

    pub fn attach_payload(
        &mut self,
        object: ObjectId,
        pose: WeaponPayloadPose,
    ) -> Result<(), WeaponPayloadLifecycleError> {
        self.payloads.attach(object, pose)
    }

    /// Build and attach C1950's creation pose under the new object's generation-checked identity.
    ///
    /// The table resolver supplies typed values; this method preserves constructor then template
    /// ordering and only publishes the completed pose if the object slot accepts that identity.
    /// Resource resolution, scene allocation and the live C1950 call path remain neighboring work.
    pub fn create_c1950_payload(
        &mut self,
        object: ObjectId,
        inputs: C1950PayloadInputs,
    ) -> Result<WeaponPayloadPose, WeaponPayloadLifecycleError> {
        let pose = WeaponPayloadPose::from_c1950_inputs(inputs);
        self.attach_payload(object, pose)?;
        Ok(pose)
    }

    /// Admit one scene identity and attach C1950's completed creation pose as one transaction.
    ///
    /// The object owner still resolves model/resource data and publishes render membership after
    /// this method succeeds. If the pose owner rejects the generation, the just-created scene
    /// identity is retired before returning so callers never observe a half-created payload.
    pub fn spawn_c1950_payload(
        &mut self,
        scene: &mut SceneMembership,
        object_slot: u32,
        inputs: C1950PayloadInputs,
    ) -> Result<(ObjectId, WeaponPayloadPose), WeaponPayloadSpawnError> {
        let object = scene
            .spawn(object_slot)
            .map_err(WeaponPayloadSpawnError::Scene)?;
        match self.create_c1950_payload(object, inputs) {
            Ok(pose) => Ok((object, pose)),
            Err(payload) => match scene.retire(object) {
                Ok(()) => Err(WeaponPayloadSpawnError::Payload(payload)),
                Err(scene) => Err(WeaponPayloadSpawnError::Rollback { payload, scene }),
            },
        }
    }

    /// Commit the equipped-once fact for a live Rust-owned player lifetime.
    ///
    /// The checked handle makes this the native mutation seam: it does not resolve a guest
    /// extension, import an inventory snapshot, or mirror a compatibility byte. The hybrid
    /// adapter may mirror the resulting bit separately for retained lifted readers.
    pub fn mark_equipped_once_for_lifetime(
        &mut self,
        player: WeaponPlayerHandle,
        entry: usize,
    ) -> bool {
        let Some(owner) = self.players.get_handle_mut(player) else {
            return false;
        };
        owner.mark_equipped_once(entry)
    }

    pub fn payload_pose(
        &self,
        object: ObjectId,
    ) -> Result<WeaponPayloadPose, WeaponPayloadLifecycleError> {
        self.payloads.pose(object)
    }

    /// Retire payload state before the neighbouring object pool reuses this identity.
    pub fn retire_payload(
        &mut self,
        object: ObjectId,
    ) -> Result<WeaponPayloadPose, WeaponPayloadLifecycleError> {
        self.payloads.retire(object)
    }

    /// Retire the payload and scene membership for the exact live object generation.
    ///
    /// Both owners are validated before mutation. The returned pose is a retirement receipt for
    /// adjacent resource owners; it does not claim that the original game releases resources at
    /// this boundary. Objects without an attached weapon payload use `SceneMembership::retire`.
    pub fn retire_payload_object(
        &mut self,
        scene: &mut SceneMembership,
        object: ObjectId,
    ) -> Result<WeaponPayloadPose, WeaponObjectRetireError> {
        if !scene.is_active(object) {
            return Err(WeaponObjectRetireError::Scene(MembershipError::NotActive));
        }
        let pose = self
            .payload_pose(object)
            .map_err(WeaponObjectRetireError::Payload)?;

        // The preflight checks above and exclusive borrows make both mutations infallible unless
        // one of the owners violates its own checked identity contract.
        let retired_pose = self
            .retire_payload(object)
            .map_err(WeaponObjectRetireError::Payload)?;
        scene
            .retire(object)
            .map_err(WeaponObjectRetireError::Scene)?;
        debug_assert_eq!(retired_pose, pose);
        Ok(retired_pose)
    }

    /// Read the active payload pose through the owning scene identity.
    ///
    /// This keeps render-prep consumers on the same generation-checked boundary as creation and
    /// retirement. It does not synthesize transforms or claim that the live renderer consumes it.
    pub fn active_payload_pose(
        &self,
        scene: &SceneMembership,
        object: ObjectId,
    ) -> Result<WeaponPayloadPose, WeaponPayloadLifecycleError> {
        if !scene.is_active(object) {
            return Err(WeaponPayloadLifecycleError::StaleObjectId);
        }
        self.payload_pose(object)
    }
}

impl<const PLAYER_SLOTS: usize> Default for WeaponSubsystemState<PLAYER_SLOTS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const PLAYER_SLOTS: usize> core::ops::Deref for WeaponSubsystemState<PLAYER_SLOTS> {
    type Target = WeaponInventoryStore<PLAYER_SLOTS>;

    fn deref(&self) -> &Self::Target {
        &self.players
    }
}

impl<const PLAYER_SLOTS: usize> core::ops::DerefMut for WeaponSubsystemState<PLAYER_SLOTS> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.players
    }
}

#[cfg(test)]
mod weapon_payload_scene_retirement_tests {
    use super::{WeaponObjectRetireError, WeaponPayloadSpawnError, WeaponSubsystemState};
    use crate::object_pool::ObjectId;
    use crate::scene_membership::{MembershipError, SceneMembership};
    use crate::weapon_payload::{
        C1950PayloadInputs, C1950PayloadTemplate, PayloadRotationMode, WeaponPayloadLifecycleError,
        WeaponPayloadPose,
    };

    fn pose(yaw: f32) -> WeaponPayloadPose {
        WeaponPayloadPose {
            pitch_yaw_roll_degrees: [0.0, yaw, 0.0],
            staged_yaw_degrees: yaw + 5.0,
            rotation_mode: PayloadRotationMode::Yaw,
        }
    }

    #[test]
    fn payload_and_scene_retirement_are_generation_scoped() {
        let mut scene = SceneMembership::default();
        let mut weapons = WeaponSubsystemState::<1>::with_object_capacity(16);
        let first = scene.spawn(7).unwrap();
        let first_pose = pose(30.0);
        weapons.attach_payload(first, first_pose).unwrap();

        assert_eq!(
            weapons.retire_payload_object(&mut scene, first),
            Ok(first_pose)
        );
        assert!(!scene.is_active(first));
        assert_eq!(
            weapons.payload_pose(first),
            Err(WeaponPayloadLifecycleError::NotAttached)
        );

        let second = scene.spawn(7).unwrap();
        assert_eq!(second.generation, first.generation + 1);
        let second_pose = pose(70.0);
        weapons.attach_payload(second, second_pose).unwrap();
        assert_eq!(
            weapons.retire_payload_object(&mut scene, first),
            Err(WeaponObjectRetireError::Scene(MembershipError::NotActive))
        );
        assert!(scene.is_active(second));
        assert_eq!(weapons.payload_pose(second), Ok(second_pose));

        let active_without_payload = scene.spawn(8).unwrap();
        assert_eq!(
            weapons.retire_payload_object(&mut scene, active_without_payload),
            Err(WeaponObjectRetireError::Payload(
                WeaponPayloadLifecycleError::NotAttached
            ))
        );
        assert!(scene.is_active(active_without_payload));
    }

    #[test]
    fn stale_generation_cannot_retire_reused_payload_slot() {
        let mut scene = SceneMembership::default();
        let mut weapons = WeaponSubsystemState::<1>::with_object_capacity(16);
        let first = scene.spawn(9).unwrap();
        weapons.attach_payload(first, pose(10.0)).unwrap();
        weapons.retire_payload_object(&mut scene, first).unwrap();

        let reused = scene.spawn(9).unwrap();
        let reused_pose = pose(20.0);
        weapons.attach_payload(reused, reused_pose).unwrap();
        assert_ne!(first, reused);
        assert_eq!(
            weapons.retire_payload_object(
                &mut scene,
                ObjectId {
                    slot: reused.slot,
                    generation: first.generation,
                }
            ),
            Err(WeaponObjectRetireError::Scene(MembershipError::NotActive))
        );
        assert!(scene.is_active(reused));
        assert_eq!(weapons.payload_pose(reused), Ok(reused_pose));
    }

    #[test]
    fn active_payload_pose_requires_the_current_scene_generation() {
        let mut scene = SceneMembership::default();
        let mut weapons = WeaponSubsystemState::<1>::with_object_capacity(16);
        let first = scene.spawn(9).unwrap();
        let first_pose = pose(10.0);
        weapons.attach_payload(first, first_pose).unwrap();
        assert_eq!(weapons.active_payload_pose(&scene, first), Ok(first_pose));

        weapons.retire_payload_object(&mut scene, first).unwrap();
        let reused = scene.spawn(9).unwrap();
        let reused_pose = pose(20.0);
        weapons.attach_payload(reused, reused_pose).unwrap();
        assert_eq!(
            weapons.active_payload_pose(&scene, first),
            Err(WeaponPayloadLifecycleError::StaleObjectId)
        );
        assert_eq!(weapons.active_payload_pose(&scene, reused), Ok(reused_pose));
        assert_eq!(
            weapons.payload_pose(first),
            Err(WeaponPayloadLifecycleError::StaleObjectId)
        );
    }

    #[test]
    fn c1950_scene_spawn_attaches_pose_before_admission_returns() {
        let mut scene = SceneMembership::default();
        let mut weapons = WeaponSubsystemState::<1>::with_object_capacity(16);
        let inputs = C1950PayloadInputs {
            definition_angles_degrees: [0.0, 25.0, 0.0],
            staged_yaw_degrees: 7.0,
            template: Some(C1950PayloadTemplate {
                flags: 0x2,
                angles_degrees: [10.0, 20.0, 30.0],
            }),
        };

        let (object, attached_pose) = weapons
            .spawn_c1950_payload(&mut scene, 3, inputs)
            .expect("resolved payload should enter the native scene owner");

        assert!(scene.is_active(object));
        assert_eq!(weapons.payload_pose(object), Ok(attached_pose));
        assert_eq!(attached_pose.pitch_yaw_roll_degrees, [10.0, 20.0, 30.0]);
        assert_eq!(attached_pose.staged_yaw_degrees, 7.0);
        assert_eq!(attached_pose.rotation_mode, PayloadRotationMode::Euler);
    }

    #[test]
    fn c1950_scene_spawn_rolls_back_when_payload_owner_rejects_slot() {
        let mut scene = SceneMembership::default();
        let mut weapons = WeaponSubsystemState::<1>::with_object_capacity(0);
        let inputs = C1950PayloadInputs {
            definition_angles_degrees: [0.0, 25.0, 0.0],
            staged_yaw_degrees: 7.0,
            template: None,
        };

        assert_eq!(
            weapons.spawn_c1950_payload(&mut scene, 3, inputs),
            Err(WeaponPayloadSpawnError::Payload(
                WeaponPayloadLifecycleError::SlotOutOfRange
            ))
        );
        assert!(!scene.is_active(ObjectId {
            slot: 3,
            generation: 1,
        }));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickupAmmoMutation {
    pub ammo_type: AmmoType,
    pub reserve_before: i16,
    pub reserve_after: i16,
    pub loaded_at_pickup: i32,
    pub reserve_maximum: i16,
    /// Signed reserve-after minus reserve-before, as returned by the measured helper.
    pub reserve_delta: i32,
    /// The original pickup path rechecks acquisition when reserve changed, or when its caller's
    /// pickup flag permits the no-change path.
    pub acquisition_check_requested: bool,
}

/// The owned core result for a pickup: each reserve mutation is paired with the acquisition
/// grants it requests, in the same source order as the original two ammo pairs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PickupAcquisitionBatch {
    pub pickups: [Option<PickupAmmoMutation>; 2],
    pub acquisitions: [AcquisitionResult; 2],
    pub auto_selections: [AmmoAutoSelectionResult; 2],
}

/// The known pre-consumption branch of the ordinary ammo-pickup callback.
///
/// This reports only whether the callback can return before its later acquisition and
/// consumed-output decisions. Continuing does not mean the world pickup was consumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmmoPickupCallbackProgress {
    /// Both reserve calls returned zero, so the ordinary path returns zero without writing the
    /// consumed-output pointer. The world pickup remains available for another attempt.
    ReturnZeroWithoutConsumption,
    /// A reserve call changed state; the original continues through acquisition/eligibility.
    ContinueOrdinaryAcquisition,
    /// The pickup flag contributes to the callback accumulator, so acquisition can run even with
    /// no reserve delta. The flag's wider pickup-class semantics remain unresolved.
    ContinueFlaggedAcquisition,
}

impl PickupAcquisitionBatch {
    /// Classify the bounded `0x0BFB40` callback prefix after its two reserve pairs are applied.
    ///
    /// T2367 establishes the ordinary zero-delta early return and the flag-bit exception. The
    /// later acquisition/eligibility result still controls the callback's final success and any
    /// consumed-output write.
    pub fn callback_progress(
        &self,
        pickup_flag_allows_unchanged_acquisition: bool,
    ) -> AmmoPickupCallbackProgress {
        if pickup_flag_allows_unchanged_acquisition {
            return AmmoPickupCallbackProgress::ContinueFlaggedAcquisition;
        }
        if self
            .pickups
            .iter()
            .flatten()
            .any(|pickup| pickup.reserve_delta != 0)
        {
            AmmoPickupCallbackProgress::ContinueOrdinaryAcquisition
        } else {
            AmmoPickupCallbackProgress::ReturnZeroWithoutConsumption
        }
    }
}

impl WeaponInventory {
    /// Construct the bounded Rust-owned inventory state initialized by the player spawn path.
    /// The caller resolves the hands entry; transition history and presentation state are not
    /// inferred from this reset constructor.
    pub fn for_spawn(hands_entry: usize) -> Option<Self> {
        if hands_entry >= ENTRY_COUNT {
            return None;
        }
        Some(Self {
            desired_entry: Some(hands_entry as u8),
            ..Self::default()
        })
    }

    /// Apply the measured cycle selector to this inventory's owned-entry bits.
    /// Caller intent and the immutable order/eligibility table remain neighboring inputs.
    pub fn next_cycle_entry(
        &self,
        entries: &[WeaponCycleEntry; WEAPON_CYCLE_SLOTS],
        previous_entry: u32,
    ) -> u32 {
        let owned_entries = core::array::from_fn(|slot| {
            let mut entry = entries[slot];
            entry.owned = self.owned.get(entry.id as usize).copied().unwrap_or(false);
            entry
        });
        next_selectable(&owned_entries, previous_entry)
    }

    /// Apply the profile-owned values copied directly by the measured static restore path.
    ///
    /// This does not simulate a reload: loaded rounds are copied into both hands as stored.
    /// Record identities, saved rounds, controller timers, and definition-derived values are
    /// preserved for their neighboring owners to resolve.
    pub fn restore_profile(&mut self, profile: ProfileWeaponInventory) {
        self.desired_entry = profile.desired_entry;
        self.owned = profile.owned;
        self.reserve = profile.reserve;
        self.hands.hand_mut(HandSide::Right).loaded =
            profile.loaded_rounds[HandSide::Right.index()];
        self.hands.hand_mut(HandSide::Left).loaded = profile.loaded_rounds[HandSide::Left.index()];
    }

    fn begin_reload_session(
        &mut self,
        owner: WeaponPlayerHandle,
        side: HandSide,
        controller: usize,
        saved_round_limit: bool,
        restrict_primary_to_one: bool,
    ) -> ReloadSession {
        self.hands.hand_mut(side).reload_timer = 0;
        ReloadSession {
            owner,
            side,
            controller,
            saved_round_limit,
            restrict_primary_to_one,
            state: ReloadSessionState::Pending,
            total_moved: 0,
        }
    }

    fn advance_reload_session(
        &mut self,
        session: &mut ReloadSession,
        active_record: WeaponAmmoRecord,
    ) -> ReloadSessionAdvance {
        if session.state != ReloadSessionState::Pending {
            return ReloadSessionAdvance::Terminal {
                state: session.state,
                total_moved: session.total_moved,
            };
        }
        let moved = self.hands.transfer_reload(
            &mut self.reserve,
            session.side,
            active_record,
            session.saved_round_limit,
            session.restrict_primary_to_one,
        );
        session.total_moved = session.total_moved.wrapping_add(moved);
        session.state = ReloadSessionState::Complete;
        self.hands
            .finish_reload_controller(session.side, session.controller);
        ReloadSessionAdvance::TransferProcessed {
            state: ReloadSessionState::Complete,
            total_moved: session.total_moved,
        }
    }

    /// Admit the first eligible owned weapon and consume its reserve before the original actor
    /// emission gates. The table/resource owner supplies the resolved source facts; no guest
    /// memory or projectile dispatch crosses this owned-state boundary.
    pub fn admit_shot(
        &mut self,
        sources: &[ShotAmmoSource; ENTRY_COUNT],
        game_type: u8,
        actor_flags: u32,
        actor_low16: u16,
    ) -> ShotAdmission {
        let selected = if game_type == 10 {
            ShotSelection {
                weapon: crate::weapon_hands::WeaponRecordId(22),
                ammo_type: None,
                reserve_consumed: false,
                emission_allowed: false,
            }
        } else {
            let mut found = None;
            for (entry, source) in sources.iter().enumerate() {
                if !self.owned[entry] || source.resource_flags & 0x400 == 0 {
                    continue;
                }
                let (Some(weapon), Some(ammo)) = (source.weapon, source.ammo_type) else {
                    continue;
                };
                let reserve = self.reserve[ammo.index()];
                if reserve <= 0 {
                    continue;
                }
                let after = reserve.wrapping_sub(1);
                self.reserve[ammo.index()] = if after < 0 { 0 } else { after };
                found = Some(ShotSelection {
                    weapon,
                    ammo_type: Some(ammo),
                    reserve_consumed: true,
                    emission_allowed: false,
                });
                break;
            }
            let Some(found) = found else {
                return ShotAdmission::NoEligibleAmmo;
            };
            found
        };
        ShotAdmission::Selected(ShotSelection {
            emission_allowed: actor_flags & 0x100 == 0 && actor_low16 == 0,
            ..selected
        })
    }

    /// Begin the measured 0x1BF9E0 request handoff using Rust-owned desired, realised and
    /// controller state. The raw request decides whether a transition starts; the adjacent
    /// 0x1BA130 owner supplies its already-normalized desired entry. A request is not a receipt:
    /// the prior desired entry is captured as currently realised equipment during transition
    /// setup; completion later commits the new desired entry.
    pub fn request_handoff(&mut self, raw_entry: usize, normalized_entry: usize) -> bool {
        if raw_entry >= ENTRY_COUNT || normalized_entry >= ENTRY_COUNT {
            return false;
        }
        if self.desired_entry == Some(raw_entry as u8) {
            return false;
        }

        self.hands.realised_entry = self.desired_entry.map(usize::from);
        self.desired_entry = Some(normalized_entry as u8);
        for controller in &mut self.hands.controllers {
            controller.state = 3;
            controller.timer = 0;
        }
        true
    }

    /// Commit the entry after the neighbouring hand/controller transition has completed.
    /// This is separate from request publication and from the later equipped-once callback edge.
    pub fn complete_handoff(&mut self, entry: usize) -> bool {
        if entry >= ENTRY_COUNT {
            return false;
        }
        self.hands.realised_entry = Some(entry);
        true
    }

    /// Release one owned entry and repair `desired_entry` if it named that entry.
    ///
    /// The caller supplies all table priorities and the post-release result of the original
    /// selectable predicate. The current T2367 static reading models the repair as the first
    /// table row with the lowest priority strictly below 100, falling back to entry 0. This is
    /// INFERRED from bounded MEASURED-static evidence; it does not model the separate inferred
    /// `+0x9c` secondary-entry reader or callback/HUD timing.
    pub fn release_entry_and_repair_selection(
        &mut self,
        entry: usize,
        candidates: &[SelectionRepairCandidate; ENTRY_COUNT],
    ) -> Option<EntryReleaseOutcome> {
        if entry >= ENTRY_COUNT {
            return None;
        }

        let was_owned = self.owned[entry];
        self.owned[entry] = false;
        let selection_repaired = self.desired_entry == Some(entry as u8);
        if selection_repaired {
            self.desired_entry = None;
            let mut best_priority = 100i32;
            let mut replacement = 0usize;
            for candidate in candidates {
                if candidate.selectable
                    && candidate.entry < ENTRY_COUNT
                    && candidate.entry != entry
                    && candidate.priority < best_priority
                {
                    best_priority = candidate.priority;
                    replacement = candidate.entry;
                }
            }
            self.desired_entry = Some(replacement as u8);
        }

        Some(EntryReleaseOutcome {
            entry: entry as u8,
            was_owned,
            selection_repaired,
            resulting_desired_entry: self.desired_entry,
        })
    }

    /// Commit the equipped-once fact after its event/callback boundary has completed.
    pub fn mark_equipped_once(&mut self, entry: usize) -> bool {
        let Some(equipped_once) = self.hands.equipped_once.get_mut(entry) else {
            return false;
        };
        *equipped_once = true;
        true
    }

    /// Apply the measured ownership, first-acquisition saved rounds and collect-latch portions of
    /// `0x1BFCA0`. Selection policy, notification, audio and statistics remain explicit
    /// neighboring effects. The record owner must supply the saved-round pair values.
    pub fn collect_entry(
        &mut self,
        definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
        entry: usize,
    ) -> AcquisitionResult {
        if !matches!(definitions.get(entry), Some(Some(_))) {
            return AcquisitionResult::default();
        }
        self.collected_latch = true;
        self.grant_entry_with_saved_rounds(definitions, entry)
    }

    /// Apply `0x1BFaa0`'s ownership-group grant and first-acquisition saved-round writes.
    /// Collection-latch and selection effects belong to the adjacent `0x1BFca0` operation.
    pub fn grant_entry_with_saved_rounds(
        &mut self,
        definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
        entry: usize,
    ) -> AcquisitionResult {
        let mut result = self.grant_entry(definitions, entry);
        if result.acquired {
            let definition = definitions[entry].expect("validated entry");
            if let Some(saved) = definition.grant_saved_rounds {
                for side in [HandSide::Left, HandSide::Right] {
                    let hand = self.hands.hand_mut(side);
                    if let Some(ammo) = definition.primary_ammo {
                        hand.saved_rounds[ammo.index()] = saved.primary;
                    }
                    if let Some(ammo) = definition.secondary_ammo {
                        hand.saved_rounds[ammo.index()] = saved.secondary;
                    }
                }
            } else {
                result.saved_rounds_unresolved = true;
            }
        }
        result
    }

    /// Compose the measured grant/latch operation with an adjacent `0x1BFCA0` selection result.
    /// `select` is supplied by the separate `0x1BA7B0` policy owner, while `normalized_entry`
    /// is supplied by `0x1BA130`'s entry-chain resolver. This writes desired state only; it does
    /// not complete or begin the distinct realised-hand transition.
    pub fn collect_entry_and_select(
        &mut self,
        definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
        entry: usize,
        select: bool,
        normalized_entry: Option<usize>,
    ) -> CollectEntryOutcome {
        let Some(Some(_)) = definitions.get(entry) else {
            return CollectEntryOutcome::default();
        };

        let previously_owned = self.owned[entry];
        let acquisition = self.collect_entry(definitions, entry);
        let mut outcome = CollectEntryOutcome {
            previously_owned,
            acquisition,
            selection_requested: select,
            ..CollectEntryOutcome::default()
        };
        if !select {
            return outcome;
        }

        let Some(normalized) = normalized_entry.filter(|candidate| *candidate < ENTRY_COUNT) else {
            outcome.selection_unresolved = true;
            return outcome;
        };
        self.desired_entry = Some(normalized as u8);
        outcome.selected_entry = Some(normalized as u8);
        outcome
    }

    /// Apply the bounded inventory prefix of world-weapon pickup callback `0x0C0310` with the
    /// zero-total duplicate predicate defaulted to false. Native callers with a resolved policy
    /// should use [`Self::collect_weapon_pickup_with_duplicate_predicate`]. Acceptance does not
    /// decide whether the neighboring world-object owner retires the pickup.
    pub fn collect_weapon_pickup(
        &mut self,
        entry: usize,
        ammo_pairs: [Option<WeaponPickupAmmo>; 2],
        realised_hand_records: [Option<WeaponAmmoRecord>; 2],
        definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
        select: bool,
        normalized_entry: Option<usize>,
    ) -> WeaponPickupOutcome {
        self.collect_weapon_pickup_with_duplicate_predicate(
            entry,
            ammo_pairs,
            realised_hand_records,
            definitions,
            false,
            select,
            normalized_entry,
        )
    }

    /// Apply the measured `0x0C0310` inventory prefix with its native duplicate gate result.
    ///
    /// REA shows both signed `0x1BA220` results accumulated before the already-owned check. A
    /// zero total consults the separate `0x3533F0` predicate; this method takes that result from
    /// the neighboring native policy owner rather than replaying guest code. The predicate's
    /// broader session semantics and world-pickup retirement are outside this contract.
    pub fn collect_weapon_pickup_with_duplicate_predicate(
        &mut self,
        entry: usize,
        ammo_pairs: [Option<WeaponPickupAmmo>; 2],
        realised_hand_records: [Option<WeaponAmmoRecord>; 2],
        definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
        zero_total_duplicate_predicate_passed: bool,
        select: bool,
        normalized_entry: Option<usize>,
    ) -> WeaponPickupOutcome {
        let mut outcome = self.prepare_weapon_pickup_with_duplicate_predicate(
            entry,
            ammo_pairs,
            realised_hand_records,
            zero_total_duplicate_predicate_passed,
        );
        self.finish_weapon_pickup_grant(&mut outcome, definitions, entry, select, normalized_entry);
        outcome
    }

    fn prepare_weapon_pickup_with_duplicate_predicate(
        &mut self,
        entry: usize,
        ammo_pairs: [Option<WeaponPickupAmmo>; 2],
        realised_hand_records: [Option<WeaponAmmoRecord>; 2],
        zero_total_duplicate_predicate_passed: bool,
    ) -> WeaponPickupOutcome {
        let Some(previously_owned) = self.owned.get(entry).copied() else {
            return WeaponPickupOutcome::default();
        };
        let mut outcome = WeaponPickupOutcome {
            entry_valid: true,
            previously_owned,
            ..WeaponPickupOutcome::default()
        };

        for (slot, pair) in ammo_pairs.into_iter().enumerate() {
            let Some(pair) = pair else { continue };
            let loaded = self.loaded_for_realised_hands(pair.ammo_type, realised_hand_records);
            let before = self.reserve[pair.ammo_type.index()];
            self.add_ammo_reserve(pair.ammo_type, pair.amount, loaded, pair.reserve_maximum);
            let after = self.reserve[pair.ammo_type.index()];
            let delta = after as i32 - before as i32;
            outcome.reserve_increased |= delta > 0;
            outcome.ammo[slot] = Some(WeaponPickupAmmoMutation {
                ammo_type: pair.ammo_type,
                reserve_before: before,
                reserve_after: after,
                loaded_at_pickup: loaded,
                reserve_maximum: pair.reserve_maximum,
                reserve_delta: delta,
            });
            outcome.reserve_delta_total = outcome.reserve_delta_total.wrapping_add(delta);
        }

        outcome.duplicate_predicate_consulted =
            previously_owned && outcome.reserve_delta_total == 0;
        outcome.duplicate_predicate_passed =
            outcome.duplicate_predicate_consulted && zero_total_duplicate_predicate_passed;
        outcome.inventory_accepted = !previously_owned
            || outcome.reserve_delta_total != 0
            || outcome.duplicate_predicate_passed;
        outcome
    }

    fn finish_weapon_pickup_grant(
        &mut self,
        outcome: &mut WeaponPickupOutcome,
        definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
        entry: usize,
        select: bool,
        normalized_entry: Option<usize>,
    ) {
        if !outcome.inventory_accepted {
            return;
        }
        outcome.grant_called = true;
        if definitions[entry].is_none() {
            outcome.grant_unresolved = true;
            return;
        }
        outcome.grant =
            Some(self.collect_entry_and_select(definitions, entry, select, normalized_entry));
    }

    /// Grant one entry and its measured ownership group. An X1 entry grants itself and
    /// non-X2 left-hand companions; an X2 entry grants every member of the same group.
    pub fn grant_entry(
        &mut self,
        definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
        entry: usize,
    ) -> AcquisitionResult {
        let Some(Some(selected)) = definitions.get(entry) else {
            return AcquisitionResult::default();
        };
        let dual = selected.flags & ENTRY_FLAG_X2 != 0;
        let mut result = AcquisitionResult::default();
        for (index, candidate) in definitions.iter().enumerate() {
            let Some(candidate) = candidate else { continue };
            let member = index == entry
                || (candidate.group == selected.group
                    && (dual
                        || (candidate.flags & ENTRY_FLAG_LEFT != 0
                            && candidate.flags & ENTRY_FLAG_X2 == 0)));
            if member && !self.owned[index] {
                self.owned[index] = true;
                result.newly_owned[index] = true;
            }
        }
        result.acquired = result.newly_owned[entry];
        result
    }

    /// Apply the measured `0x1BFEC0` acquisition scan for one ammo type. The caller resolves
    /// definition rows and applies the adjacent `0x1BFCA0` selectability/presentation policy.
    /// The returned bool matches the original helper's acquisition result, not selection.
    pub fn acquire_matching_ammo(
        &mut self,
        definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
        ammo_type: AmmoType,
    ) -> AcquisitionResult {
        let mut result = AcquisitionResult::default();
        for entry in 1..ENTRY_COUNT {
            let Some(definition) = definitions[entry] else {
                continue;
            };
            let matches_ammo = definition.primary_ammo == Some(ammo_type)
                || definition.secondary_ammo == Some(ammo_type);
            if self.owned[entry]
                || !definition.record_present
                || definition.flags & ENTRY_FLAG_ACQUISITION_MASK == 0
                || !matches_ammo
            {
                continue;
            }
            // 0x1BFEC0 delegates each eligible unowned candidate through 0x1BFCA0.
            let grant = self.collect_entry(definitions, entry);
            result.acquired |= grant.acquired;
            result.saved_rounds_unresolved |= grant.saved_rounds_unresolved;
            for (changed, was_granted) in result.newly_owned.iter_mut().zip(grant.newly_owned) {
                *changed |= was_granted;
            }
        }
        result
    }

    /// Count rounds the selected hand can pull from this Rust-owned reserve state.
    pub fn reloadable_count(&self, side: HandSide, record: WeaponAmmoRecord) -> i32 {
        self.hands.reloadable_count(&self.reserve, side, record)
    }

    /// Transfer reserve into one owned hand using the REA reload arithmetic and capacities.
    pub fn transfer_reload(
        &mut self,
        side: HandSide,
        record: WeaponAmmoRecord,
        saved_round_limit: bool,
        restrict_primary_to_one: bool,
    ) -> i32 {
        self.hands.transfer_reload(
            &mut self.reserve,
            side,
            record,
            saved_round_limit,
            restrict_primary_to_one,
        )
    }

    /// Run the owned synchronous reload transfer with the active table record. A later cancel
    /// cannot undo the committed primary and secondary channel writes.
    pub fn apply_reload_session(
        &mut self,
        session: &mut HandReloadSession,
        active_record: WeaponAmmoRecord,
    ) -> HandReloadSessionOutcome {
        session.apply(&mut self.hands, &mut self.reserve, active_record)
    }

    /// Gate a reload request using the same signed reserve and hand-capacity calculation.
    pub fn reload_request_allowed(&self, side: HandSide, record: WeaponAmmoRecord) -> bool {
        self.hands
            .reload_request_allowed(&self.reserve, side, record)
    }

    /// Reset one owned hand and return its prior modeled state for the neighboring object owner.
    pub fn reset_hand(&mut self, side: HandSide) -> crate::weapon_hands::HandResetReceipt {
        self.hands.reset_hand(side)
    }

    /// Apply the transfer before resetting the adjacent controller's typed state.
    pub fn reload_then_reset(
        &mut self,
        side: HandSide,
        controller: usize,
        record: WeaponAmmoRecord,
        saved_round_limit: bool,
        restrict_primary_to_one: bool,
    ) -> i32 {
        self.hands.reload_then_reset(
            &mut self.reserve,
            side,
            controller,
            record,
            saved_round_limit,
            restrict_primary_to_one,
        )
    }

    /// Realise one hand record through this Rust-owned inventory, including its reserve transfer
    /// and neighboring child/model effects. The old record is explicit because the per-hand state
    /// stores its identity while the immutable definition belongs to the definition resolver.
    pub fn realise_hand<P: crate::weapon_hands::HandPresentation>(
        &mut self,
        side: HandSide,
        old_record: Option<WeaponAmmoRecord>,
        next_record: WeaponAmmoRecord,
        presentation: &mut P,
        restrict_primary_to_one: bool,
    ) -> Result<HandRealiseOutcome, crate::weapon_hands::HandRealiseError> {
        self.hands.realise_hand(
            &mut self.reserve,
            side,
            old_record,
            next_record,
            presentation,
            restrict_primary_to_one,
        )
    }

    /// Apply the measured two-side `0x2B8F90` refresh through the Rust-owned inventory.
    /// Each side commits independently; a missing old record on one side does not suppress the
    /// other side's realization.
    pub fn realise_actor_update<P: crate::weapon_hands::HandPresentation>(
        &mut self,
        right: HandRealiseRequest,
        left: HandRealiseRequest,
        presentation: &mut P,
    ) -> HandRealisePairOutcome {
        self.hands
            .realise_actor_update(&mut self.reserve, right, left, presentation)
    }

    /// Return the active record's loaded words to the owned reserve without a maximum clamp.
    pub fn unload_without_clamp(&mut self, side: HandSide, record: WeaponAmmoRecord) {
        self.hands
            .unload_without_clamp(&mut self.reserve, side, record);
    }

    /// Complete the state-3 ammo handoff: save active rounds, cap returned reserves, clear the
    /// hand's loaded words, and publish the realised entry only after the transition boundary.
    pub fn holster(
        &mut self,
        side: HandSide,
        record: WeaponAmmoRecord,
        reserve_maximum: &[i16; AMMO_TYPE_COUNT],
        desired_entry: usize,
    ) {
        self.hands.holster(
            &mut self.reserve,
            side,
            record,
            reserve_maximum,
            desired_entry,
        );
    }

    /// Apply the Rust-owned ammo and selection portion of the measured `0x1C3100` state-3
    /// handoff. Controller and presentation completion remain with their adjacent owners.
    pub fn complete_state3_holster(
        &mut self,
        side: HandSide,
        controller: usize,
        record: WeaponAmmoRecord,
        reserve_maximum: &[i16; AMMO_TYPE_COUNT],
    ) -> bool {
        if !matches!(
            self.hands.controllers.get(controller),
            Some(state) if state.state == 3 && state.hand == Some(side.index())
        ) {
            return false;
        }
        let Some(desired_entry) = self
            .desired_entry
            .map(usize::from)
            .filter(|entry| *entry < ENTRY_COUNT)
        else {
            return false;
        };

        self.holster(side, record, reserve_maximum, desired_entry);
        true
    }

    /// Restore saved rounds into the selected hand, clearing its current loaded words first.
    pub fn restore_saved(
        &mut self,
        side: HandSide,
        record: WeaponAmmoRecord,
        restrict_primary_to_one: bool,
    ) -> i32 {
        self.hands
            .restore_saved(&mut self.reserve, side, record, restrict_primary_to_one)
    }

    /// Apply the two pickup ammo pairs in source order. `realised_hand_records` and `maximums`
    /// are borrowed typed data resolved by neighbouring owners; no guest pointer crosses this
    /// operation. A repeated ammo type observes the prior pair's reserve write.
    pub fn collect_ammo_pickup(
        &mut self,
        pairs: [PickupAmmoPair; 2],
        realised_hand_records: [Option<WeaponAmmoRecord>; 2],
        maximums: &[i16; AMMO_TYPE_COUNT],
        pickup_flag_allows_unchanged_acquisition: bool,
    ) -> [Option<PickupAmmoMutation>; 2] {
        let mut outcomes = [None; 2];
        for (slot, pair) in pairs.into_iter().enumerate() {
            // 0xBFB40 only enters reserve mutation for positive type < 44 and positive amount.
            let Some(ammo_type) = (pair.ammo_type > 0 && pair.ammo_type < AMMO_TYPE_COUNT as u32)
                .then(|| AmmoType::new(pair.ammo_type as u8))
                .flatten()
            else {
                continue;
            };
            // The pickup caller compares the source dword as signed and only forwards positive
            // amounts. Preserve that guard before narrowing the argument to a signed word.
            if pair.amount as i32 <= 0 {
                continue;
            }

            let index = ammo_type.index();
            let before = self.reserve[index];
            // The original forwards the amount through a 16-bit argument and stores the sum as
            // a 16-bit word before its signed maximum comparison.
            let amount = pair.amount as u16 as i16;
            let added = before.wrapping_add(amount);
            let loaded = self.loaded_for_realised_hands(ammo_type, realised_hand_records);
            let combined = added as i32 + loaded;
            let maximum = maximums[index];
            let after = if combined > maximum as i32 {
                (maximum as i32 - loaded) as i16
            } else {
                added
            };
            self.reserve[index] = after;
            let delta = after as i32 - before as i32;
            outcomes[slot] = Some(PickupAmmoMutation {
                ammo_type,
                reserve_before: before,
                reserve_after: after,
                loaded_at_pickup: loaded,
                reserve_maximum: maximum,
                reserve_delta: delta,
                acquisition_check_requested: delta != 0 || pickup_flag_allows_unchanged_acquisition,
            });
        }
        outcomes
    }

    /// Apply the measured `0x1BA220` reserve addition to owned state. `amount` is already
    /// narrowed to the original signed 16-bit argument; `loaded` and `maximum` are resolved by
    /// the hand and ammo-table owners. Arithmetic preserves the original wrapping word write
    /// before its signed maximum comparison.
    pub fn add_ammo_reserve(
        &mut self,
        ammo_type: AmmoType,
        amount: i16,
        loaded: i32,
        maximum: i32,
    ) -> i32 {
        let index = ammo_type.index();
        let before = self.reserve[index];
        let added = before.wrapping_add(amount);
        let after = if (added as i32).wrapping_add(loaded) > maximum {
            maximum.wrapping_sub(loaded) as i16
        } else {
            added
        };
        self.reserve[index] = after;
        after as i32 - before as i32
    }

    /// Apply a pickup and its measured ownership acquisition walk as one Rust-owned operation.
    /// Selection policy and its presentation side effects remain a separate typed attachment;
    /// this operation neither writes guest memory nor treats an acquisition result as selection.
    pub fn collect_ammo_pickup_and_acquire(
        &mut self,
        pairs: [PickupAmmoPair; 2],
        realised_hand_records: [Option<WeaponAmmoRecord>; 2],
        maximums: &[i16; AMMO_TYPE_COUNT],
        pickup_flag_allows_unchanged_acquisition: bool,
        definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
    ) -> PickupAcquisitionBatch {
        self.collect_ammo_pickup_acquire_and_select(
            pairs,
            realised_hand_records,
            maximums,
            pickup_flag_allows_unchanged_acquisition,
            definitions,
            [None; 2],
        )
    }

    /// Apply each pickup pair, acquisition scan and optional auto-selection before moving to
    /// the next pair. This preserves source order when both pairs share an ammo type or the first
    /// scan changes ownership/desired selection used by the second.
    pub fn collect_ammo_pickup_acquire_and_select(
        &mut self,
        pairs: [PickupAmmoPair; 2],
        realised_hand_records: [Option<WeaponAmmoRecord>; 2],
        maximums: &[i16; AMMO_TYPE_COUNT],
        pickup_flag_allows_unchanged_acquisition: bool,
        definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
        auto_selection: [Option<AmmoAutoSelectionContext>; 2],
    ) -> PickupAcquisitionBatch {
        let mut batch = PickupAcquisitionBatch::default();
        for (slot, pair) in pairs.into_iter().enumerate() {
            batch.pickups[slot] = self.collect_ammo_pickup(
                [pair, PickupAmmoPair::default()],
                realised_hand_records,
                maximums,
                pickup_flag_allows_unchanged_acquisition,
            )[0];
            let Some(pickup) = batch.pickups[slot] else {
                continue;
            };
            if pickup.acquisition_check_requested {
                batch.acquisitions[slot] =
                    self.acquire_matching_ammo(definitions, pickup.ammo_type);
                if let Some(context) = auto_selection[slot] {
                    batch.auto_selections[slot] = self.auto_select_matching_ammo(
                        definitions,
                        pickup.ammo_type,
                        pickup.reserve_before,
                        context,
                    );
                }
            }
        }
        batch
    }

    /// Apply T2367's second 1BFEC0 loop. It can select already-owned matching entries even
    /// when the acquisition loop grants nothing. Each selection updates desired state before
    /// the next candidate's priority is compared; realised hands remain owned by `WeaponHands`.
    pub fn auto_select_matching_ammo(
        &mut self,
        definitions: &[Option<InventoryEntryDefinition>; ENTRY_COUNT],
        ammo_type: AmmoType,
        previous_reserve: i16,
        context: AmmoAutoSelectionContext,
    ) -> AmmoAutoSelectionResult {
        let mut result = AmmoAutoSelectionResult {
            resulting_desired_entry: self.desired_entry,
            ..AmmoAutoSelectionResult::default()
        };
        if previous_reserve != 0
            || !context.profile_available
            || context.player_index < 0
            || !matches!(context.weapon_change_mode, 0 | 2)
        {
            return result;
        }
        let Some(current_entry) = self
            .desired_entry
            .map(usize::from)
            .filter(|entry| *entry < ENTRY_COUNT)
        else {
            result.unresolved_at_entry = self.desired_entry.map(usize::from);
            return result;
        };
        let Some(mut current_rank) = context.priorities[current_entry] else {
            result.unresolved_at_entry = Some(current_entry);
            return result;
        };

        for entry in 1..ENTRY_COUNT {
            let Some(definition) = definitions[entry] else {
                continue;
            };
            if !self.owned[entry]
                || !(definition.primary_ammo == Some(ammo_type)
                    || definition.secondary_ammo == Some(ammo_type))
                || !context.matching_loaded_rounds_zero[entry]
            {
                continue;
            }
            let Some(candidate_rank) = context.priorities[entry] else {
                result.unresolved_at_entry.get_or_insert(entry);
                continue;
            };
            if candidate_rank > current_rank {
                continue;
            }
            let Some(normalized) = context.normalized_entries[entry]
                .filter(|normalized| (*normalized as usize) < ENTRY_COUNT)
            else {
                result.unresolved_at_entry.get_or_insert(entry);
                continue;
            };

            self.desired_entry = Some(normalized);
            result.selected_entries[entry] = true;
            if let Some(next_rank) = context.priorities[normalized as usize] {
                current_rank = next_rank;
            } else {
                // Keep the selection but stop rather than fabricate an unresolved priority.
                result
                    .unresolved_at_entry
                    .get_or_insert(normalized as usize);
                break;
            }
        }
        result.resulting_desired_entry = self.desired_entry;
        result
    }

    /// Sum each matching realised hand once. A record with the same type in both ammo fields
    /// contributes that hand's loaded word only once, matching the T2367 `0x1B67E0` contract.
    fn loaded_for_realised_hands(
        &self,
        ammo_type: AmmoType,
        realised_hand_records: [Option<WeaponAmmoRecord>; 2],
    ) -> i32 {
        let mut loaded = 0i32;
        for (index, record) in realised_hand_records.into_iter().enumerate() {
            let Some(record) = record else { continue };
            let hand = &self.hands.hands[index];
            if hand.record != Some(record.id) {
                continue;
            }
            if record.primary == Some(ammo_type) || record.secondary == Some(ammo_type) {
                loaded += hand.loaded[ammo_type.index()] as i32;
            }
        }
        loaded
    }
}

#[cfg(test)]
mod weapon_cycle_owner_tests {
    use super::{WeaponInventory, WeaponInventoryStore, WeaponPlayerId};
    use crate::weapon_cycle::{WEAPON_CYCLE_SLOTS, WeaponCycleEntry};

    fn rows() -> [WeaponCycleEntry; WEAPON_CYCLE_SLOTS] {
        core::array::from_fn(|slot| WeaponCycleEntry {
            id: slot as u32,
            selection_flag_0x10: true,
            // Rust-owned selection must overwrite this adapter-shaped value.
            owned: slot == 2,
        })
    }

    #[test]
    fn cycle_selection_uses_owned_state_and_commits_desired_entry_atomically() {
        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        let mut initial = WeaponInventory::default();
        initial.desired_entry = Some(1);
        initial.owned[3] = true;
        initial.hands.realised_entry = Some(0);
        initial.reserve[4] = 12;
        let handle = store
            .begin_lifetime(player, initial)
            .expect("player lifetime");

        let result = store
            .select_cycle_entry_for_lifetime(handle, &rows(), 1)
            .expect("current player lifetime");
        assert_eq!(result.selected_id, 3);
        assert!(result.desired_entry_committed);
        let owner = store.get_handle(handle).expect("same player lifetime");
        assert_eq!(owner.desired_entry, Some(3));
        assert_eq!(owner.hands.realised_entry, Some(0));
        assert_eq!(owner.reserve[4], 12);
    }

    #[test]
    fn cycle_selection_preserves_the_original_no_match_fallback() {
        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        let handle = store
            .begin_lifetime(player, WeaponInventory::default())
            .expect("player lifetime");

        let result = store
            .select_cycle_entry_for_lifetime(handle, &rows(), 4)
            .expect("current player lifetime");
        assert_eq!(result.selected_id, 4);
        assert!(result.desired_entry_committed);
        assert_eq!(store.get_handle(handle).unwrap().desired_entry, Some(4));
    }

    #[test]
    fn invalid_cycle_fallback_is_returned_but_not_committed_as_a_valid_entry() {
        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        let mut initial = WeaponInventory::default();
        initial.desired_entry = Some(4);
        let handle = store
            .begin_lifetime(player, initial)
            .expect("player lifetime");
        let mut entries = rows();
        for (slot, entry) in entries.iter_mut().enumerate() {
            entry.id = 70 + slot as u32;
            entry.selection_flag_0x10 = false;
        }

        let result = store
            .select_cycle_entry_for_lifetime(handle, &entries, 70)
            .expect("current player lifetime");
        assert_eq!(result.selected_id, 70);
        assert!(!result.desired_entry_committed);
        assert_eq!(store.get_handle(handle).unwrap().desired_entry, Some(4));
    }

    #[test]
    fn stale_cycle_selection_cannot_mutate_a_reused_player_slot() {
        let player = WeaponPlayerId::new(0);
        let mut store = WeaponInventoryStore::<1>::default();
        let stale = store
            .begin_lifetime(player, WeaponInventory::default())
            .expect("first player lifetime");
        assert!(store.retire_handle(stale).is_some());
        let current = store
            .begin_lifetime(player, WeaponInventory::default())
            .expect("reused player lifetime");

        assert!(
            store
                .select_cycle_entry_for_lifetime(stale, &rows(), 4)
                .is_none()
        );
        assert_eq!(store.get_handle(current).unwrap().desired_entry, None);
    }
}
