// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned player interaction and lock-on attachment state (T3398 / T1680).
//!
//! Lock-on slot results are not object-pool identities or guest pointers. The original
//! `0xD01C0` contract returns a slot index and writes two independent vec3 outputs; those
//! outputs remain observable even when the returned index is `-1`. This module owns that
//! result against the observed current PlayerSet generation. Selection and guest-memory adapters
//! stay at the caller seam.

/// A slot index returned by the original lock-on scan. This is not an `ObjectId`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LockOnSlotId(pub u32);

/// A D01C0 result interpreted against the candidate-list bound observed for that scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LockOnCandidateIndex(u32);

impl LockOnCandidateIndex {
    pub const fn new(index: u32, candidate_count: u32) -> Option<Self> {
        if index < candidate_count {
            Some(Self(index))
        } else {
            None
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A checked view of the D01C0 candidate-record array.
///
/// The ABI returns an index bounded by the current candidate count; records use the measured
/// `0xEA0` stride from the table pointer at `[0x7356D8]`. Keeping the base and bound together
/// prevents consumers from rebuilding an unchecked guest address from an index alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LockOnCandidateTableView {
    base: u32,
    candidate_count: u32,
}

impl LockOnCandidateTableView {
    pub const RECORD_STRIDE: u32 = 0xea0;

    pub const fn new(base: u32, candidate_count: u32) -> Option<Self> {
        if candidate_count == 0 {
            return Some(Self {
                base,
                candidate_count,
            });
        }
        if base < 0x1_0000 {
            return None;
        }
        let Some(last_offset) = (candidate_count - 1).checked_mul(Self::RECORD_STRIDE) else {
            return None;
        };
        let Some(last_record) = base.checked_add(last_offset) else {
            return None;
        };
        if last_record.checked_add(Self::RECORD_STRIDE - 1).is_none() {
            return None;
        }
        Some(Self {
            base,
            candidate_count,
        })
    }

    pub const fn candidate_count(self) -> u32 {
        self.candidate_count
    }

    pub const fn base(self) -> u32 {
        self.base
    }

    pub const fn record_address(
        self,
        index: LockOnCandidateIndex,
    ) -> Option<LockOnCandidateRecordAddress> {
        if index.get() >= self.candidate_count {
            return None;
        }
        let Some(offset) = index.get().checked_mul(Self::RECORD_STRIDE) else {
            return None;
        };
        let Some(address) = self.base.checked_add(offset) else {
            return None;
        };
        Some(LockOnCandidateRecordAddress(address))
    }
}

/// Guest record address derived only through a valid `LockOnCandidateTableView`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LockOnCandidateRecordAddress(u32);

impl LockOnCandidateRecordAddress {
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Typed interpretation of D01C0's signed candidate-list result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockOnCandidateSelection {
    NoCandidate,
    Candidate(LockOnCandidateIndex),
    InvalidResult(i32),
    CandidateCountUnavailable,
}

/// Generation of the current player record owned by `PlayerSet`/`PlayerRetire`.
///
/// This is deliberately independent of `WeaponPlayerId`: the D01C0 contract reads the current
/// record and does not establish a mapping from that record's profile entry to an inventory slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerSetGeneration(u32);

impl PlayerSetGeneration {
    /// Construct a generation token from the owner's nonzero rebase count.
    pub const fn new(generation: u32) -> Option<Self> {
        if generation == 0 {
            None
        } else {
            Some(Self(generation))
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Index of a record in the inline `[0x7B0C48]` player-record array.
///
/// This identity is separate from both a D01C0 lock-on candidate slot and a weapon inventory
/// slot. The original record array uses a fixed `0x1584` byte stride.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerRecordIndex(u32);

impl PlayerRecordIndex {
    pub const RECORD_STRIDE: u32 = 0x1584;

    pub const fn new(index: u32, record_count: u32) -> Option<Self> {
        if index < record_count {
            Some(Self(index))
        } else {
            None
        }
    }

    /// Resolve a current-record pointer only when it names an aligned slot in the measured
    /// inline array. This keeps the guest address at the adapter edge and publishes a typed ID.
    pub const fn from_array(
        array_base: u32,
        record_count: u32,
        current_record: u32,
    ) -> Option<Self> {
        if array_base < 0x1_0000
            || array_base & 3 != 0
            || current_record < array_base
            || current_record & 3 != 0
        {
            return None;
        }
        let offset = current_record - array_base;
        if offset % Self::RECORD_STRIDE != 0 {
            return None;
        }
        Self::new(offset / Self::RECORD_STRIDE, record_count)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One of the two vec3 outputs written by the lock-on scan.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LockOnVector(pub [f32; 3]);

/// The return value and both outputs of one `0xD01C0` call.
///
/// The two vectors are intentionally retained when `returned_index` is `-1`: the observed
/// routine may write output state before returning no selected slot.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LockOnScanOutput {
    returned_index: i32,
    vectors: [LockOnVector; 2],
}

impl LockOnScanOutput {
    pub const fn new(returned_index: i32, first: LockOnVector, second: LockOnVector) -> Self {
        Self {
            returned_index,
            vectors: [first, second],
        }
    }

    pub const fn returned_index(self) -> i32 {
        self.returned_index
    }

    /// The measured ABI uses `-1` for no slot and nonnegative values for slot indices.
    pub const fn selected_slot(self) -> Option<LockOnSlotId> {
        if self.returned_index < 0 {
            None
        } else {
            Some(LockOnSlotId(self.returned_index as u32))
        }
    }

    /// Interpret the result using the candidate-list count captured for this scan.
    pub const fn candidate_selection(self, candidate_count: u32) -> LockOnCandidateSelection {
        if self.returned_index == -1 {
            return LockOnCandidateSelection::NoCandidate;
        }
        if self.returned_index < 0 {
            return LockOnCandidateSelection::InvalidResult(self.returned_index);
        }
        match LockOnCandidateIndex::new(self.returned_index as u32, candidate_count) {
            Some(index) => LockOnCandidateSelection::Candidate(index),
            None => LockOnCandidateSelection::InvalidResult(self.returned_index),
        }
    }

    pub const fn vectors(self) -> [LockOnVector; 2] {
        self.vectors
    }
}

/// A lock-on scan attached to the current inline player record that owned the call.
///
/// The record index is distinct from both the D01C0 candidate slot and weapon inventory slot.
/// The generation is a lifecycle token and is not an identity by itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerLockOnSnapshot {
    generation: PlayerSetGeneration,
    player_record: PlayerRecordIndex,
    candidate_count: Option<u32>,
    candidate_table: Option<LockOnCandidateTableView>,
    scan: LockOnScanOutput,
}

impl PlayerLockOnSnapshot {
    pub const fn new(
        generation: PlayerSetGeneration,
        player_record: PlayerRecordIndex,
        candidate_count: Option<u32>,
        scan: LockOnScanOutput,
    ) -> Self {
        Self {
            generation,
            player_record,
            candidate_count,
            candidate_table: None,
            scan,
        }
    }

    pub const fn with_candidate_table(
        generation: PlayerSetGeneration,
        player_record: PlayerRecordIndex,
        candidate_table: LockOnCandidateTableView,
        scan: LockOnScanOutput,
    ) -> Self {
        Self {
            generation,
            player_record,
            candidate_count: Some(candidate_table.candidate_count()),
            candidate_table: Some(candidate_table),
            scan,
        }
    }

    pub const fn generation(self) -> PlayerSetGeneration {
        self.generation
    }

    pub const fn scan(self) -> LockOnScanOutput {
        self.scan
    }

    pub const fn player_record(self) -> PlayerRecordIndex {
        self.player_record
    }

    pub const fn candidate_count(self) -> Option<u32> {
        self.candidate_count
    }

    pub const fn candidate_table(self) -> Option<LockOnCandidateTableView> {
        self.candidate_table
    }

    /// Consume the scan result without treating a candidate slot as a player or object identity.
    pub const fn selection(self) -> LockOnCandidateSelection {
        if self.scan.returned_index() == -1 {
            return LockOnCandidateSelection::NoCandidate;
        }
        if self.scan.returned_index() < 0 {
            return LockOnCandidateSelection::InvalidResult(self.scan.returned_index());
        }
        let Some(candidate_count) = self.candidate_count else {
            return LockOnCandidateSelection::CandidateCountUnavailable;
        };
        self.scan.candidate_selection(candidate_count)
    }

    /// Present one selected candidate as a typed native observation.
    ///
    /// This keeps the candidate-list index and the two ABI outputs paired with the player
    /// record and lifecycle that produced them. It deliberately does not resolve the index to
    /// an object or inventory identity; that bridge is not part of the measured contract.
    pub const fn selected_candidate(self) -> Option<PlayerLockOnCandidate> {
        match self.selection() {
            LockOnCandidateSelection::Candidate(index) => {
                let record_address = match self.candidate_table {
                    Some(table) => table.record_address(index),
                    None => None,
                };
                Some(PlayerLockOnCandidate {
                    generation: self.generation,
                    player_record: self.player_record,
                    index,
                    record_address,
                    scan: self.scan,
                })
            }
            LockOnCandidateSelection::NoCandidate
            | LockOnCandidateSelection::InvalidResult(_)
            | LockOnCandidateSelection::CandidateCountUnavailable => None,
        }
    }
}

/// A checked candidate observation produced by one player-owned lock-on scan.
///
/// `index` addresses the D01C0 candidate list only. The scan retains both output vectors and
/// the original signed result; no target object identity is inferred here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerLockOnCandidate {
    generation: PlayerSetGeneration,
    player_record: PlayerRecordIndex,
    index: LockOnCandidateIndex,
    record_address: Option<LockOnCandidateRecordAddress>,
    scan: LockOnScanOutput,
}

impl PlayerLockOnCandidate {
    pub const fn generation(self) -> PlayerSetGeneration {
        self.generation
    }

    pub const fn player_record(self) -> PlayerRecordIndex {
        self.player_record
    }

    pub const fn index(self) -> LockOnCandidateIndex {
        self.index
    }

    pub const fn record_address(self) -> Option<LockOnCandidateRecordAddress> {
        self.record_address
    }

    pub const fn scan(self) -> LockOnScanOutput {
        self.scan
    }
}

/// Player-owned lock-on outputs. A0D30's interaction-completed flag remains in `WeaponInventory`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerInteractionAttachment {
    lock_on: Option<LockOnScanOutput>,
    candidate_count: Option<u32>,
    candidate_table: Option<LockOnCandidateTableView>,
}

impl PlayerInteractionAttachment {
    pub const fn lock_on(&self) -> Option<LockOnScanOutput> {
        self.lock_on
    }
}

/// Current-player interaction attachment keyed by lifecycle generation and typed record index.
/// Lock-on candidate slots and weapon inventory identities are deliberately separate domains.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerInteractionStore {
    enabled: bool,
    latest_player_set_generation: Option<PlayerSetGeneration>,
    latest_player_record: Option<PlayerRecordIndex>,
    player_set_attachment: Option<PlayerInteractionAttachment>,
}

impl PlayerInteractionStore {
    /// Whether the native lock-on attachment is enabled for this owner.
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable this opt-in attachment. Disabling it clears published results so callers cannot
    /// observe stale native output after returning to the lifted path.
    pub fn configure(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            if let Some(state) = &mut self.player_set_attachment {
                state.lock_on = None;
                state.candidate_count = None;
                state.candidate_table = None;
            }
        }
    }

    /// Begin observing a PlayerSet-backed generation without requiring a weapon inventory.
    /// A replacement generation always starts with empty interaction state, even if no valid
    /// lock-on output is produced for its first scan.
    pub fn observe_player_set_generation(&mut self, generation: PlayerSetGeneration) -> bool {
        if !self.enabled {
            return false;
        }
        match self.latest_player_set_generation {
            Some(latest) if generation < latest => return false,
            Some(latest) if generation == latest => {}
            _ => {
                self.latest_player_set_generation = Some(generation);
                self.latest_player_record = None;
                self.player_set_attachment = Some(PlayerInteractionAttachment::default());
            }
        }
        true
    }

    /// Observe the measured current-record slot for a generation. Changing the slot invalidates
    /// the previous scan even if a caller reused the same lifecycle token.
    pub fn observe_player_record(
        &mut self,
        generation: PlayerSetGeneration,
        player_record: PlayerRecordIndex,
    ) -> bool {
        if !self.observe_player_set_generation(generation) {
            return false;
        }
        if self.latest_player_record != Some(player_record) {
            self.latest_player_record = Some(player_record);
            self.player_set_attachment = Some(PlayerInteractionAttachment::default());
        }
        true
    }

    /// Clear the previous current-player scan before beginning another D01C0 call.
    ///
    /// This sidecar policy prevents stale vectors from remaining published when a later scan
    /// has unreadable output pointers or its player owner changes during the call.
    pub fn clear_current_player_set_lock_on_output(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        let Some(attachment) = self.player_set_attachment.as_mut() else {
            return false;
        };
        attachment.lock_on = None;
        attachment.candidate_count = None;
        attachment.candidate_table = None;
        true
    }

    /// Attach output to an observed PlayerSet-backed generation without requiring a weapon
    /// inventory. The generation is independent of inventory slots and profile-array entries.
    pub fn attach_lock_on_output_for_generation(
        &mut self,
        generation: PlayerSetGeneration,
        output: LockOnScanOutput,
    ) -> bool {
        if !self.observe_player_set_generation(generation) {
            return false;
        }
        let Some(attachment) = self.player_set_attachment.as_mut() else {
            return false;
        };
        attachment.lock_on = Some(output);
        attachment.candidate_count = None;
        attachment.candidate_table = None;
        true
    }

    /// Attach the scan to a typed current player-record slot and its lifecycle generation.
    pub fn attach_lock_on_output_for_player(
        &mut self,
        generation: PlayerSetGeneration,
        player_record: PlayerRecordIndex,
        output: LockOnScanOutput,
    ) -> bool {
        if !self.observe_player_record(generation, player_record) {
            return false;
        }
        let Some(attachment) = self.player_set_attachment.as_mut() else {
            return false;
        };
        attachment.lock_on = Some(output);
        attachment.candidate_count = None;
        true
    }

    /// Attach a scan with its stable candidate-list bound for typed native consumption.
    pub fn attach_lock_on_output_for_player_with_candidate_count(
        &mut self,
        generation: PlayerSetGeneration,
        player_record: PlayerRecordIndex,
        output: LockOnScanOutput,
        candidate_count: u32,
    ) -> bool {
        self.attach_lock_on_output_for_player_with_candidate_info(
            generation,
            player_record,
            output,
            candidate_count,
            None,
        )
    }

    /// Attach output with a stable, checked view of its candidate-record table.
    pub fn attach_lock_on_output_for_player_with_candidate_table(
        &mut self,
        generation: PlayerSetGeneration,
        player_record: PlayerRecordIndex,
        output: LockOnScanOutput,
        candidate_table: LockOnCandidateTableView,
    ) -> bool {
        self.attach_lock_on_output_for_player_with_candidate_info(
            generation,
            player_record,
            output,
            candidate_table.candidate_count(),
            Some(candidate_table),
        )
    }

    fn attach_lock_on_output_for_player_with_candidate_info(
        &mut self,
        generation: PlayerSetGeneration,
        player_record: PlayerRecordIndex,
        output: LockOnScanOutput,
        candidate_count: u32,
        candidate_table: Option<LockOnCandidateTableView>,
    ) -> bool {
        if !matches!(
            output.candidate_selection(candidate_count),
            LockOnCandidateSelection::NoCandidate | LockOnCandidateSelection::Candidate(_)
        ) {
            return false;
        }
        if !self.observe_player_record(generation, player_record) {
            return false;
        }
        let Some(attachment) = self.player_set_attachment.as_mut() else {
            return false;
        };
        attachment.lock_on = Some(output);
        attachment.candidate_count = Some(candidate_count);
        attachment.candidate_table = candidate_table;
        true
    }

    pub fn get_for_generation(
        &self,
        generation: PlayerSetGeneration,
    ) -> Option<&PlayerInteractionAttachment> {
        if self.latest_player_set_generation == Some(generation) {
            self.player_set_attachment.as_ref()
        } else {
            None
        }
    }

    /// Read the attached scan together with its lifecycle token and player-record index.
    ///
    /// Native consumers can use this snapshot without reconstructing an identity from the
    /// lock-on slot or reaching into guest memory. The generation remains a lifecycle token,
    /// not a weapon inventory identity.
    pub fn current_lock_on_snapshot(&self) -> Option<PlayerLockOnSnapshot> {
        let generation = self.latest_player_set_generation?;
        let player_record = self.latest_player_record?;
        let attachment = self.get_for_generation(generation)?;
        let output = attachment.lock_on()?;
        Some(match attachment.candidate_table {
            Some(candidate_table) => PlayerLockOnSnapshot::with_candidate_table(
                generation,
                player_record,
                candidate_table,
                output,
            ),
            None => PlayerLockOnSnapshot::new(
                generation,
                player_record,
                attachment.candidate_count,
                output,
            ),
        })
    }

    /// Read only a candidate whose result is valid for the captured list bound.
    pub fn current_lock_on_candidate(&self) -> Option<PlayerLockOnCandidate> {
        self.current_lock_on_snapshot()?.selected_candidate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_observation_pairs_checked_index_outputs_and_player_owner() {
        let generation = PlayerSetGeneration::new(9).unwrap();
        let player_record = PlayerRecordIndex::new(2, 4).unwrap();
        let candidate_table = LockOnCandidateTableView::new(0x20_000, 3).unwrap();
        let scan = LockOnScanOutput::new(
            1,
            LockOnVector([1.0, 2.0, 3.0]),
            LockOnVector([4.0, 5.0, 6.0]),
        );
        let snapshot = PlayerLockOnSnapshot::with_candidate_table(
            generation,
            player_record,
            candidate_table,
            scan,
        );

        let candidate = snapshot.selected_candidate().unwrap();
        assert_eq!(candidate.generation(), generation);
        assert_eq!(candidate.player_record(), player_record);
        assert_eq!(candidate.index(), LockOnCandidateIndex::new(1, 3).unwrap());
        assert_eq!(
            candidate.record_address().unwrap().get(),
            0x20_000 + LockOnCandidateTableView::RECORD_STRIDE
        );
        assert_eq!(snapshot.candidate_table(), Some(candidate_table));
        assert_eq!(candidate.scan(), scan);
    }

    #[test]
    fn candidate_table_view_checks_stride_bounds_and_guest_address_overflow() {
        let table = LockOnCandidateTableView::new(0x20_000, 3).unwrap();
        let last = LockOnCandidateIndex::new(2, 3).unwrap();
        assert_eq!(
            table.record_address(last).unwrap().get(),
            0x20_000 + 2 * LockOnCandidateTableView::RECORD_STRIDE
        );
        assert!(
            table
                .record_address(LockOnCandidateIndex::new(3, 4).unwrap())
                .is_none()
        );
        assert!(LockOnCandidateTableView::new(0x1_0000, 0).is_some());
        assert!(LockOnCandidateTableView::new(0, 1).is_none());
        assert!(LockOnCandidateTableView::new(0x20_001, 1).is_some());
        assert!(LockOnCandidateTableView::new(0xffff, 1).is_none());
        assert!(LockOnCandidateTableView::new(u32::MAX - 3, 2).is_none());
    }

    #[test]
    fn candidate_observation_rejects_absent_unbounded_and_invalid_results() {
        let generation = PlayerSetGeneration::new(9).unwrap();
        let player_record = PlayerRecordIndex::new(2, 4).unwrap();
        let vectors = [LockOnVector::default(); 2];

        let no_candidate = PlayerLockOnSnapshot::new(
            generation,
            player_record,
            Some(3),
            LockOnScanOutput::new(-1, vectors[0], vectors[1]),
        );
        let no_bound = PlayerLockOnSnapshot::new(
            generation,
            player_record,
            None,
            LockOnScanOutput::new(0, vectors[0], vectors[1]),
        );
        let out_of_range = PlayerLockOnSnapshot::new(
            generation,
            player_record,
            Some(3),
            LockOnScanOutput::new(3, vectors[0], vectors[1]),
        );
        let invalid_negative = PlayerLockOnSnapshot::new(
            generation,
            player_record,
            Some(3),
            LockOnScanOutput::new(-2, vectors[0], vectors[1]),
        );

        assert_eq!(no_candidate.selected_candidate(), None);
        assert_eq!(no_bound.selected_candidate(), None);
        assert_eq!(out_of_range.selected_candidate(), None);
        assert_eq!(invalid_negative.selected_candidate(), None);
    }

    #[test]
    fn no_slot_keeps_both_measured_output_vectors() {
        let mut state = PlayerInteractionStore::default();
        assert!(!state.is_enabled());
        let generation = PlayerSetGeneration::new(1).unwrap();
        assert!(
            !state.attach_lock_on_output_for_generation(generation, LockOnScanOutput::default())
        );
        state.configure(true);

        let output = LockOnScanOutput::new(
            -1,
            LockOnVector([1.25, -2.5, 3.75]),
            LockOnVector([-4.0, 5.5, 6.25]),
        );
        assert!(state.attach_lock_on_output_for_generation(generation, output));

        let observed = state
            .get_for_generation(generation)
            .and_then(PlayerInteractionAttachment::lock_on)
            .unwrap();
        assert_eq!(observed.selected_slot(), None);
        assert_eq!(observed.returned_index(), -1);
        assert_eq!(observed.vectors(), output.vectors());
    }

    #[test]
    fn candidate_result_is_typed_and_checked_against_the_scan_bound() {
        let no_candidate =
            LockOnScanOutput::new(-1, LockOnVector::default(), LockOnVector::default());
        let first_candidate =
            LockOnScanOutput::new(0, LockOnVector::default(), LockOnVector::default());
        let out_of_range =
            LockOnScanOutput::new(2, LockOnVector::default(), LockOnVector::default());
        let invalid_negative =
            LockOnScanOutput::new(-2, LockOnVector::default(), LockOnVector::default());

        assert_eq!(
            no_candidate.candidate_selection(0),
            LockOnCandidateSelection::NoCandidate
        );
        assert_eq!(
            first_candidate.candidate_selection(1),
            LockOnCandidateSelection::Candidate(LockOnCandidateIndex(0))
        );
        assert_eq!(
            out_of_range.candidate_selection(2),
            LockOnCandidateSelection::InvalidResult(2)
        );
        assert_eq!(
            invalid_negative.candidate_selection(4),
            LockOnCandidateSelection::InvalidResult(-2)
        );
    }

    #[test]
    fn disabling_clears_published_output_and_rejects_new_results() {
        let mut state = PlayerInteractionStore::default();
        let generation = PlayerSetGeneration::new(2).unwrap();
        let output = LockOnScanOutput::new(
            2,
            LockOnVector([1.0, 2.0, 3.0]),
            LockOnVector([4.0, 5.0, 6.0]),
        );

        state.configure(true);
        assert!(state.attach_lock_on_output_for_generation(generation, output));
        assert_eq!(
            state
                .get_for_generation(generation)
                .and_then(PlayerInteractionAttachment::lock_on),
            Some(output)
        );

        state.configure(false);
        assert_eq!(
            state
                .get_for_generation(generation)
                .and_then(PlayerInteractionAttachment::lock_on),
            None
        );
        assert!(!state.attach_lock_on_output_for_generation(generation, output));
    }

    #[test]
    fn replacing_player_set_generation_clears_old_output_and_rejects_stale_writes() {
        let mut state = PlayerInteractionStore::default();
        state.configure(true);
        let first = PlayerSetGeneration::new(3).unwrap();
        let output = LockOnScanOutput::new(7, LockOnVector::default(), LockOnVector::default());
        assert_eq!(output.selected_slot(), Some(LockOnSlotId(7)));
        assert!(state.attach_lock_on_output_for_generation(first, output));

        let second = PlayerSetGeneration::new(4).unwrap();
        assert!(state.observe_player_set_generation(second));
        assert_eq!(
            state
                .get_for_generation(second)
                .and_then(PlayerInteractionAttachment::lock_on),
            None
        );
        assert!(!state.attach_lock_on_output_for_generation(first, output));
    }

    #[test]
    fn player_set_generation_owns_interaction_without_inventory_identity() {
        let mut state = PlayerInteractionStore::default();
        let output = LockOnScanOutput::new(
            -1,
            LockOnVector([1.0, 2.0, 3.0]),
            LockOnVector([4.0, 5.0, 6.0]),
        );
        state.configure(true);

        assert!(PlayerSetGeneration::new(0).is_none());
        let fourth = PlayerSetGeneration::new(4).unwrap();
        assert!(state.attach_lock_on_output_for_generation(fourth, output));
        assert_eq!(
            state
                .get_for_generation(fourth)
                .and_then(PlayerInteractionAttachment::lock_on),
            Some(output)
        );
        assert!(
            !state
                .attach_lock_on_output_for_generation(PlayerSetGeneration::new(3).unwrap(), output)
        );
        let fifth = PlayerSetGeneration::new(5).unwrap();
        assert!(state.observe_player_set_generation(fifth));
        assert!(state.get_for_generation(fourth).is_none());
        assert_eq!(
            state
                .get_for_generation(fifth)
                .and_then(PlayerInteractionAttachment::lock_on),
            None
        );
        assert!(state.attach_lock_on_output_for_generation(fifth, output));
        assert_eq!(
            state
                .get_for_generation(fifth)
                .and_then(PlayerInteractionAttachment::lock_on),
            Some(output)
        );
    }

    #[test]
    fn beginning_a_current_player_scan_clears_the_previous_attached_result() {
        let mut state = PlayerInteractionStore::default();
        state.configure(true);
        let generation = PlayerSetGeneration::new(6).unwrap();
        let previous = LockOnScanOutput::new(
            3,
            LockOnVector([1.0, 2.0, 3.0]),
            LockOnVector([4.0, 5.0, 6.0]),
        );
        assert!(state.attach_lock_on_output_for_generation(generation, previous));
        assert_eq!(
            state
                .get_for_generation(generation)
                .and_then(PlayerInteractionAttachment::lock_on),
            Some(previous)
        );

        assert!(state.clear_current_player_set_lock_on_output());
        assert_eq!(
            state
                .get_for_generation(generation)
                .and_then(PlayerInteractionAttachment::lock_on),
            None
        );
    }

    #[test]
    fn current_output_snapshot_carries_its_generation_and_expires_with_it() {
        let mut state = PlayerInteractionStore::default();
        state.configure(true);
        let first_generation = PlayerSetGeneration::new(7).unwrap();
        let first_output = LockOnScanOutput::new(
            2,
            LockOnVector([1.0, 2.0, 3.0]),
            LockOnVector([4.0, 5.0, 6.0]),
        );
        let first_player = PlayerRecordIndex::new(1, 4).unwrap();
        assert!(state.attach_lock_on_output_for_player_with_candidate_count(
            first_generation,
            first_player,
            first_output,
            3
        ));
        let first_snapshot = state.current_lock_on_snapshot().unwrap();
        assert_eq!(first_snapshot.generation(), first_generation);
        assert_eq!(first_snapshot.player_record(), first_player);
        assert_eq!(first_snapshot.candidate_count(), Some(3));
        assert_eq!(
            first_snapshot.selection(),
            LockOnCandidateSelection::Candidate(LockOnCandidateIndex(2))
        );
        assert_eq!(first_snapshot.scan(), first_output);

        let second_generation = PlayerSetGeneration::new(8).unwrap();
        assert!(state.observe_player_set_generation(second_generation));
        assert_eq!(state.current_lock_on_snapshot(), None);

        let second_output = LockOnScanOutput::new(
            -1,
            LockOnVector([-1.0, -2.0, -3.0]),
            LockOnVector([-4.0, -5.0, -6.0]),
        );
        let second_player = PlayerRecordIndex::new(2, 4).unwrap();
        assert!(state.attach_lock_on_output_for_player_with_candidate_count(
            second_generation,
            second_player,
            second_output,
            0
        ));
        let second_snapshot = state.current_lock_on_snapshot().unwrap();
        assert_eq!(second_snapshot.generation(), second_generation);
        assert_eq!(second_snapshot.player_record(), second_player);
        assert_eq!(
            second_snapshot.selection(),
            LockOnCandidateSelection::NoCandidate
        );
        assert_eq!(second_snapshot.scan(), second_output);
    }

    #[test]
    fn candidate_count_is_required_before_a_positive_result_can_be_consumed() {
        let mut state = PlayerInteractionStore::default();
        state.configure(true);
        let generation = PlayerSetGeneration::new(10).unwrap();
        let player = PlayerRecordIndex::new(0, 1).unwrap();
        let output = LockOnScanOutput::new(0, LockOnVector::default(), LockOnVector::default());
        assert!(state.attach_lock_on_output_for_player(generation, player, output));
        assert_eq!(
            state.current_lock_on_snapshot().unwrap().selection(),
            LockOnCandidateSelection::CandidateCountUnavailable
        );
    }

    #[test]
    fn candidate_table_stays_paired_with_the_player_owned_scan() {
        let mut state = PlayerInteractionStore::default();
        state.configure(true);
        let generation = PlayerSetGeneration::new(91).unwrap();
        let player_record = PlayerRecordIndex::new(1, 3).unwrap();
        let table = LockOnCandidateTableView::new(0x30_000, 4).unwrap();
        let output = LockOnScanOutput::new(
            3,
            LockOnVector([1.0, 2.0, 3.0]),
            LockOnVector([4.0, 5.0, 6.0]),
        );
        assert!(state.observe_player_set_generation(generation));
        assert!(state.attach_lock_on_output_for_player_with_candidate_table(
            generation,
            player_record,
            output,
            table,
        ));
        let candidate = state.current_lock_on_candidate().unwrap();
        assert_eq!(candidate.generation(), generation);
        assert_eq!(candidate.player_record(), player_record);
        assert_eq!(
            candidate.record_address(),
            table.record_address(LockOnCandidateIndex::new(3, 4).unwrap())
        );

        state.configure(false);
        assert_eq!(state.current_lock_on_candidate(), None);
    }

    #[test]
    fn player_record_index_resolves_only_aligned_in_range_inline_records() {
        let base = 0x10000;
        assert_eq!(
            PlayerRecordIndex::from_array(base, 3, base + PlayerRecordIndex::RECORD_STRIDE),
            PlayerRecordIndex::new(1, 3)
        );
        assert_eq!(PlayerRecordIndex::from_array(base, 3, base + 1), None);
        assert_eq!(
            PlayerRecordIndex::from_array(base, 3, base + 3 * PlayerRecordIndex::RECORD_STRIDE),
            None
        );
        assert_eq!(PlayerRecordIndex::from_array(0, 3, base), None);
        assert_eq!(PlayerRecordIndex::new(3, 3), None);
    }

    #[test]
    fn changing_player_record_slot_expires_snapshot_within_generation() {
        let mut state = PlayerInteractionStore::default();
        state.configure(true);
        let generation = PlayerSetGeneration::new(9).unwrap();
        let first_player = PlayerRecordIndex::new(0, 2).unwrap();
        let output = LockOnScanOutput::new(1, LockOnVector::default(), LockOnVector::default());
        assert!(state.attach_lock_on_output_for_player(generation, first_player, output));

        let second_player = PlayerRecordIndex::new(1, 2).unwrap();
        assert!(state.observe_player_record(generation, second_player));
        assert_eq!(state.current_lock_on_snapshot(), None);
    }
}
