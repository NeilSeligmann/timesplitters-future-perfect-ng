// SPDX-License-Identifier: GPL-3.0-or-later
//! Generation-safe profile save state (T3420).
//!
//! This typed transaction seam is default-off and does not perform file I/O.
//! The request captures a profile slot reference and content revision so a
//! later completion can only clean the same generation and revision.

use alloc::collections::BTreeMap;

use crate::profile_state::{PROFILE_SLOT_COUNT, ProfileSlotRef, ProfileSlotTable};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirtyState {
    profile: ProfileSlotRef,
    revision: u64,
    dirty: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSave {
    profile: ProfileSlotRef,
    revision: u64,
}

/// Opaque identity for one save attempt.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SaveRequestId(u64);

/// Immutable request snapshot passed to the storage adapter and returned with
/// its completion. Its profile reference is generation-tagged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileSaveRequest {
    id: SaveRequestId,
    profile: ProfileSlotRef,
    revision: u64,
}

impl ProfileSaveRequest {
    pub const fn id(self) -> SaveRequestId {
        self.id
    }

    pub const fn profile(self) -> ProfileSlotRef {
        self.profile
    }

    pub const fn revision(self) -> u64 {
        self.revision
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveCompletionOutcome {
    /// The saved revision is still current, so this profile is clean.
    SavedClean,
    /// The write succeeded, but the profile changed after the request began.
    SavedButDirty,
    /// The storage adapter reported failure; dirty state is retained.
    Failed,
    /// The profile slot no longer contains the request's generation.
    StaleProfile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileSaveError {
    StaleProfile,
    RevisionExhausted,
    RequestIdExhausted,
    UnknownRequest,
    RequestMismatch,
    ProfileMismatch,
}

/// Rust-owned profile dirty state and pending save snapshots.
#[derive(Clone, Debug)]
pub struct ProfileSaveState {
    enabled: bool,
    slots: [Option<DirtyState>; PROFILE_SLOT_COUNT],
    pending: BTreeMap<SaveRequestId, PendingSave>,
    next_request_id: u64,
}

impl Default for ProfileSaveState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileSaveState {
    pub const fn new() -> Self {
        Self {
            enabled: false,
            slots: [None; PROFILE_SLOT_COUNT],
            pending: BTreeMap::new(),
            next_request_id: 1,
        }
    }

    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Disabling prevents new dirty writes and requests. Already-pending
    /// requests may still complete so they cannot strand a storage operation.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Marks the current generation dirty and advances its content revision.
    /// Returns `Ok(None)` while the default-off policy is disabled.
    pub fn mark_dirty(
        &mut self,
        slots: &ProfileSlotTable,
        profile: ProfileSlotRef,
    ) -> Result<Option<u64>, ProfileSaveError> {
        if !self.enabled {
            return Ok(None);
        }
        if !slots.contains(profile) {
            return Err(ProfileSaveError::StaleProfile);
        }

        let index = profile.slot().index();
        let current = self.slots[index];
        let mut state = match current {
            Some(state) if state.profile == profile => state,
            _ => {
                // Reuse of the same indexed slot retires old dirty state, but
                // keeps in-flight request identity to detect a late write.
                DirtyState {
                    profile,
                    revision: 0,
                    dirty: false,
                }
            }
        };
        state.revision = state
            .revision
            .checked_add(1)
            .ok_or(ProfileSaveError::RevisionExhausted)?;
        state.dirty = true;
        self.slots[index] = Some(state);
        Ok(Some(state.revision))
    }

    /// Captures the current dirty revision for a storage write.
    pub fn begin_save(
        &mut self,
        slots: &ProfileSlotTable,
        profile: ProfileSlotRef,
    ) -> Result<Option<ProfileSaveRequest>, ProfileSaveError> {
        if !self.enabled {
            return Ok(None);
        }
        if !slots.contains(profile) {
            return Err(ProfileSaveError::StaleProfile);
        }
        let Some(state) = self.slots[profile.slot().index()] else {
            return Ok(None);
        };
        if state.profile != profile || !state.dirty {
            return Ok(None);
        }

        let id = SaveRequestId(self.next_request_id);
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or(ProfileSaveError::RequestIdExhausted)?;
        let request = ProfileSaveRequest {
            id,
            profile,
            revision: state.revision,
        };
        self.pending.insert(
            id,
            PendingSave {
                profile,
                revision: state.revision,
            },
        );
        Ok(Some(request))
    }

    /// Applies a storage result only to the request's still-current profile
    /// generation. A successful old revision leaves newer edits dirty.
    pub fn complete(
        &mut self,
        slots: &ProfileSlotTable,
        request: ProfileSaveRequest,
        completed_profile: ProfileSlotRef,
        succeeded: bool,
    ) -> Result<SaveCompletionOutcome, ProfileSaveError> {
        let Some(pending) = self.pending.get(&request.id).copied() else {
            return Err(ProfileSaveError::UnknownRequest);
        };
        if pending.profile != request.profile || pending.revision != request.revision {
            return Err(ProfileSaveError::RequestMismatch);
        }
        if completed_profile != pending.profile {
            return Err(ProfileSaveError::ProfileMismatch);
        }
        self.pending.remove(&request.id);

        if !slots.contains(pending.profile) {
            if succeeded {
                self.record_stale_write(slots, pending.profile);
            }
            return Ok(SaveCompletionOutcome::StaleProfile);
        }
        if !succeeded {
            return Ok(SaveCompletionOutcome::Failed);
        }

        let Some(mut state) = self.slots[pending.profile.slot().index()] else {
            return Ok(SaveCompletionOutcome::StaleProfile);
        };
        if state.profile != pending.profile {
            return Ok(SaveCompletionOutcome::StaleProfile);
        }
        if state.revision != pending.revision {
            // An older write may replace newer bytes on disk, even if a newer
            // request had already marked this revision clean.
            state.dirty = true;
            self.slots[pending.profile.slot().index()] = Some(state);
            return Ok(SaveCompletionOutcome::SavedButDirty);
        }
        state.dirty = false;
        self.slots[pending.profile.slot().index()] = Some(state);
        Ok(SaveCompletionOutcome::SavedClean)
    }

    /// Retires dirty state after the corresponding profile slot is removed.
    /// Pending request identity remains until completion so a late successful
    /// write can dirty a replacement in the same indexed slot.
    pub fn retire_profile(&mut self, profile: ProfileSlotRef) {
        let index = profile.slot().index();
        if self.slots[index].is_some_and(|state| state.profile == profile) {
            self.slots[index] = None;
        }
    }

    pub fn is_dirty(&self, profile: ProfileSlotRef) -> bool {
        self.slots[profile.slot().index()]
            .is_some_and(|state| state.profile == profile && state.dirty)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    fn record_stale_write(&mut self, slots: &ProfileSlotTable, retired: ProfileSlotRef) {
        let Some(current) = slots.current_reference(retired.slot()) else {
            return;
        };
        if current == retired {
            return;
        }
        let index = current.slot().index();
        let state = match self.slots[index] {
            Some(mut state) if state.profile == current => {
                state.dirty = true;
                state
            }
            _ => DirtyState {
                profile: current,
                revision: 1,
                dirty: true,
            },
        };
        self.slots[index] = Some(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_state::ProfileSlotId;

    fn profile(slots: &mut ProfileSlotTable, index: usize) -> ProfileSlotRef {
        slots
            .insert(ProfileSlotId::from_index(index).expect("fixture slot in range"))
            .expect("fixture slot is empty")
    }

    fn enabled_state() -> ProfileSaveState {
        let mut state = ProfileSaveState::default();
        state.set_enabled(true);
        state
    }

    #[test]
    fn default_off_policy_does_not_create_dirty_state_or_requests() {
        let mut slots = ProfileSlotTable::new();
        let profile = profile(&mut slots, 0);
        let mut state = ProfileSaveState::default();

        assert!(!state.is_enabled());
        assert_eq!(state.mark_dirty(&slots, profile), Ok(None));
        assert!(!state.is_dirty(profile));
        assert_eq!(state.begin_save(&slots, profile), Ok(None));
    }

    #[test]
    fn matching_success_clears_only_the_saved_profile() {
        let mut slots = ProfileSlotTable::new();
        let first = profile(&mut slots, 0);
        let second = profile(&mut slots, 1);
        let mut state = enabled_state();
        state.mark_dirty(&slots, first).unwrap();
        state.mark_dirty(&slots, second).unwrap();
        let request = state.begin_save(&slots, first).unwrap().unwrap();

        assert_eq!(
            state.complete(&slots, request, first, true),
            Ok(SaveCompletionOutcome::SavedClean)
        );
        assert!(!state.is_dirty(first));
        assert!(state.is_dirty(second));
    }

    #[test]
    fn successful_completion_for_replaced_generation_cannot_clean_new_profile() {
        let mut slots = ProfileSlotTable::new();
        let slot = ProfileSlotId::from_index(2).unwrap();
        let old_profile = slots.insert(slot).unwrap();
        let mut state = enabled_state();
        state.mark_dirty(&slots, old_profile).unwrap();
        let old_request = state.begin_save(&slots, old_profile).unwrap().unwrap();

        slots.remove(old_profile).unwrap();
        let new_profile = slots.insert(slot).unwrap();
        state.mark_dirty(&slots, new_profile).unwrap();
        assert_eq!(state.pending_count(), 1);
        assert_eq!(
            state.complete(&slots, old_request, old_profile, true),
            Ok(SaveCompletionOutcome::StaleProfile)
        );
        assert!(state.is_dirty(new_profile));
    }

    #[test]
    fn successful_write_of_an_older_revision_keeps_new_edits_dirty() {
        let mut slots = ProfileSlotTable::new();
        let profile = profile(&mut slots, 3);
        let mut state = enabled_state();
        assert_eq!(state.mark_dirty(&slots, profile), Ok(Some(1)));
        let request = state.begin_save(&slots, profile).unwrap().unwrap();
        assert_eq!(state.mark_dirty(&slots, profile), Ok(Some(2)));

        assert_eq!(
            state.complete(&slots, request, profile, true),
            Ok(SaveCompletionOutcome::SavedButDirty)
        );
        assert!(state.is_dirty(profile));
    }

    #[test]
    fn late_old_write_redirties_revision_already_saved_by_newer_request() {
        let mut slots = ProfileSlotTable::new();
        let profile = profile(&mut slots, 9);
        let mut state = enabled_state();
        state.mark_dirty(&slots, profile).unwrap();
        let old_request = state.begin_save(&slots, profile).unwrap().unwrap();
        state.mark_dirty(&slots, profile).unwrap();
        let new_request = state.begin_save(&slots, profile).unwrap().unwrap();

        assert_eq!(
            state.complete(&slots, new_request, profile, true),
            Ok(SaveCompletionOutcome::SavedClean)
        );
        assert!(!state.is_dirty(profile));
        assert_eq!(
            state.complete(&slots, old_request, profile, true),
            Ok(SaveCompletionOutcome::SavedButDirty)
        );
        assert!(state.is_dirty(profile));
    }

    #[test]
    fn failed_write_keeps_dirty_state_and_consumes_its_request_once() {
        let mut slots = ProfileSlotTable::new();
        let profile = profile(&mut slots, 4);
        let mut state = enabled_state();
        state.mark_dirty(&slots, profile).unwrap();
        let request = state.begin_save(&slots, profile).unwrap().unwrap();

        assert_eq!(
            state.complete(&slots, request, profile, false),
            Ok(SaveCompletionOutcome::Failed)
        );
        assert!(state.is_dirty(profile));
        assert_eq!(
            state.complete(&slots, request, profile, true),
            Err(ProfileSaveError::UnknownRequest)
        );
    }

    #[test]
    fn mismatched_completion_profile_does_not_consume_request() {
        let mut slots = ProfileSlotTable::new();
        let first = profile(&mut slots, 5);
        let second = profile(&mut slots, 6);
        let mut state = enabled_state();
        state.mark_dirty(&slots, first).unwrap();
        let request = state.begin_save(&slots, first).unwrap().unwrap();

        assert_eq!(
            state.complete(&slots, request, second, true),
            Err(ProfileSaveError::ProfileMismatch)
        );
        assert_eq!(state.pending_count(), 1);
        assert_eq!(
            state.complete(&slots, request, first, true),
            Ok(SaveCompletionOutcome::SavedClean)
        );
    }

    #[test]
    fn late_removed_profile_write_marks_replacement_dirty() {
        let mut slots = ProfileSlotTable::new();
        let slot = ProfileSlotId::from_index(7).unwrap();
        let old_profile = slots.insert(slot).unwrap();
        let mut state = enabled_state();
        state.mark_dirty(&slots, old_profile).unwrap();
        let request = state.begin_save(&slots, old_profile).unwrap().unwrap();
        slots.remove(old_profile).unwrap();
        let replacement = slots.insert(slot).unwrap();

        assert_eq!(
            state.complete(&slots, request, old_profile, true),
            Ok(SaveCompletionOutcome::StaleProfile)
        );
        assert!(state.is_dirty(replacement));
    }

    #[test]
    fn retiring_a_profile_keeps_pending_identity_for_stale_completion() {
        let mut slots = ProfileSlotTable::new();
        let profile = profile(&mut slots, 8);
        let mut state = enabled_state();
        state.mark_dirty(&slots, profile).unwrap();
        let request = state.begin_save(&slots, profile).unwrap().unwrap();
        slots.remove(profile).unwrap();
        state.retire_profile(profile);

        assert_eq!(state.pending_count(), 1);
        assert!(!state.is_dirty(profile));
        assert_eq!(
            state.complete(&slots, request, profile, true),
            Ok(SaveCompletionOutcome::StaleProfile)
        );
        assert_eq!(state.pending_count(), 0);
    }
}
