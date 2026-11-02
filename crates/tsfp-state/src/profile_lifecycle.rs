// SPDX-License-Identifier: GPL-3.0-or-later
//! Coordinates Rust-owned profile slot identity with default-off save tracking (T3420).
//!
//! The catalogue and save tracker remain independently useful, but their delete
//! boundary has a required ordering: remove the current generation first, then
//! retire its dirty state while preserving pending request identities. This
//! type owns both sides so a storage adapter has one narrow lifecycle seam.

use crate::profile_save_state::{
    ProfileSaveError, ProfileSaveRequest, ProfileSaveState, SaveCompletionOutcome,
};
use crate::profile_state::{
    ActiveProfileState, CategoryId, CategoryOverride, EligibilityDecision, EligibilityPolicy,
    EligibilityRules, MissionId, ProfilePlayerId, ProfileSlotError, ProfileSlotId, ProfileSlotRef,
    ProfileSlotTable,
};

/// Rust-owned profile catalogue plus per-generation save transaction state.
/// Save tracking starts disabled and must be explicitly enabled by an adapter.
#[derive(Debug, Default)]
pub struct ProfileLifecycleState {
    slots: ProfileSlotTable,
    saves: ProfileSaveState,
    eligibility: EligibilityPolicy,
}

impl ProfileLifecycleState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_save_tracking_enabled(&mut self, enabled: bool) {
        self.saves.set_enabled(enabled);
    }

    pub fn insert_profile(
        &mut self,
        slot: ProfileSlotId,
    ) -> Result<ProfileSlotRef, ProfileSlotError> {
        self.slots.insert(slot)
    }

    /// Removes one current file generation and retires its dirty state.
    /// Active player bindings become stale and browser focus is cleared by the
    /// catalogue; pending saves remain identifiable for late completion.
    pub fn remove_profile(&mut self, profile: ProfileSlotRef) -> Result<(), ProfileSlotError> {
        self.slots.remove(profile)?;
        self.saves.retire_profile(profile);
        Ok(())
    }

    pub fn set_active_profile(
        &mut self,
        player: ProfilePlayerId,
        profile: Option<ProfileSlotRef>,
    ) -> Result<(), ProfileSlotError> {
        self.slots.set_active_profile(player, profile)
    }

    pub fn active_profile(&self, player: ProfilePlayerId) -> ActiveProfileState {
        self.slots.active_profile(player)
    }

    pub fn set_browser_focus(
        &mut self,
        profile: Option<ProfileSlotRef>,
    ) -> Result<(), ProfileSlotError> {
        self.slots.set_focus(profile)
    }

    pub fn browser_focus(&self) -> Option<ProfileSlotRef> {
        self.slots.focus()
    }

    pub fn mark_dirty(&mut self, profile: ProfileSlotRef) -> Result<Option<u64>, ProfileSaveError> {
        self.saves.mark_dirty(&self.slots, profile)
    }

    pub fn begin_save(
        &mut self,
        profile: ProfileSlotRef,
    ) -> Result<Option<ProfileSaveRequest>, ProfileSaveError> {
        self.saves.begin_save(&self.slots, profile)
    }

    pub fn complete_save(
        &mut self,
        request: ProfileSaveRequest,
        completed_profile: ProfileSlotRef,
        succeeded: bool,
    ) -> Result<SaveCompletionOutcome, ProfileSaveError> {
        self.saves
            .complete(&self.slots, request, completed_profile, succeeded)
    }

    pub fn is_dirty(&self, profile: ProfileSlotRef) -> bool {
        self.saves.is_dirty(profile)
    }

    pub fn pending_save_count(&self) -> usize {
        self.saves.pending_count()
    }

    /// Enables or disables native profile eligibility. The default is off;
    /// the policy does not become an original-code replacement by itself.
    pub fn set_eligibility_enabled(&mut self, enabled: bool) {
        self.eligibility.set_enabled(enabled);
    }

    pub fn set_global_unlock_override(&mut self, enabled: bool) {
        self.eligibility.set_global_override(enabled);
    }

    pub fn set_category_unlock_override(&mut self, key: CategoryOverride, enabled: bool) {
        self.eligibility.set_category_override(key, enabled);
    }

    /// Evaluates a category through this lifecycle's current typed profile
    /// binding and owned rule provider. A deleted or replaced profile remains
    /// stale and therefore cannot satisfy a requirement.
    pub fn category_eligibility<R: EligibilityRules>(
        &self,
        player: ProfilePlayerId,
        category: CategoryId,
        rules: &mut R,
    ) -> EligibilityDecision {
        self.eligibility
            .category_eligible(&self.slots, player, category, rules)
    }

    /// Evaluates a mission through the same owned lifecycle seam. Original
    /// wrapper ordering, including override early returns, remains in
    /// `EligibilityPolicy` and its provider receives no guest addresses.
    pub fn mission_eligibility<R: EligibilityRules>(
        &self,
        player: ProfilePlayerId,
        category: CategoryId,
        mission: MissionId,
        rules: &mut R,
    ) -> EligibilityDecision {
        self.eligibility
            .mission_eligible(&self.slots, player, category, mission, rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum RuleEvent {
        Category(CategoryId),
        Mission(MissionId),
        CategoryCheck(ProfileSlotRef, CategoryId),
        MissionCheck(ProfileSlotRef, MissionId),
    }

    #[derive(Default)]
    struct FabricatedEligibilityRules {
        events: Vec<RuleEvent>,
        category_met: bool,
        mission_met: bool,
    }

    impl EligibilityRules for FabricatedEligibilityRules {
        type CategoryDescriptor = CategoryId;
        type MissionDescriptor = MissionId;

        fn category_descriptor(&mut self, id: CategoryId) -> Option<CategoryId> {
            self.events.push(RuleEvent::Category(id));
            Some(id)
        }

        fn mission_descriptor(&mut self, id: MissionId) -> Option<MissionId> {
            self.events.push(RuleEvent::Mission(id));
            Some(id)
        }

        fn category_requirement_met(
            &mut self,
            profile: ProfileSlotRef,
            descriptor: CategoryId,
        ) -> bool {
            self.events
                .push(RuleEvent::CategoryCheck(profile, descriptor));
            self.category_met
        }

        fn mission_requirement_met(
            &mut self,
            profile: ProfileSlotRef,
            descriptor: MissionId,
        ) -> bool {
            self.events
                .push(RuleEvent::MissionCheck(profile, descriptor));
            self.mission_met
        }
    }

    fn profile(state: &mut ProfileLifecycleState, slot: usize) -> ProfileSlotRef {
        state
            .insert_profile(ProfileSlotId::from_index(slot).expect("fixture slot in range"))
            .expect("fixture slot empty")
    }

    #[test]
    fn save_tracking_and_requests_start_default_off() {
        let mut state = ProfileLifecycleState::new();
        let profile = profile(&mut state, 2);

        assert_eq!(state.mark_dirty(profile), Ok(None));
        assert_eq!(state.begin_save(profile), Ok(None));
        assert!(!state.is_dirty(profile));
        assert_eq!(state.pending_save_count(), 0);
    }

    #[test]
    fn delete_preserves_stale_binding_and_carries_late_write_to_replacement() {
        let mut state = ProfileLifecycleState::new();
        state.set_save_tracking_enabled(true);
        let player = ProfilePlayerId::new(0);
        let old = profile(&mut state, 4);
        state.set_active_profile(player, Some(old)).unwrap();
        state.set_browser_focus(Some(old)).unwrap();
        state.mark_dirty(old).unwrap();
        let request = state.begin_save(old).unwrap().unwrap();

        state.remove_profile(old).unwrap();

        assert_eq!(state.active_profile(player), ActiveProfileState::Stale(old));
        assert_eq!(state.browser_focus(), None);
        assert!(!state.is_dirty(old));
        assert_eq!(state.pending_save_count(), 1);

        let replacement = profile(&mut state, 4);
        assert_ne!(old.generation(), replacement.generation());
        assert_eq!(
            state.complete_save(request, old, true),
            Ok(SaveCompletionOutcome::StaleProfile)
        );
        assert!(state.is_dirty(replacement));
        assert_eq!(state.pending_save_count(), 0);
    }

    #[test]
    fn rejecting_stale_delete_does_not_retire_current_dirty_state() {
        let mut state = ProfileLifecycleState::new();
        state.set_save_tracking_enabled(true);
        let retired = profile(&mut state, 1);
        state.remove_profile(retired).unwrap();
        let current = profile(&mut state, 1);
        state.mark_dirty(current).unwrap();

        assert_eq!(
            state.remove_profile(retired),
            Err(ProfileSlotError::NotCurrent)
        );
        assert!(state.is_dirty(current));
    }

    #[test]
    fn lifecycle_eligibility_is_default_off_and_uses_its_active_profile() {
        let mut state = ProfileLifecycleState::new();
        let player = ProfilePlayerId::new(0);
        let profile = profile(&mut state, 3);
        state.set_active_profile(player, Some(profile)).unwrap();
        let mut rules = FabricatedEligibilityRules {
            category_met: true,
            mission_met: true,
            ..FabricatedEligibilityRules::default()
        };

        assert_eq!(
            state.mission_eligibility(player, CategoryId::new(6), MissionId::new(30), &mut rules,),
            EligibilityDecision::Disabled
        );
        assert!(rules.events.is_empty());

        state.set_eligibility_enabled(true);
        assert_eq!(
            state.mission_eligibility(player, CategoryId::new(6), MissionId::new(30), &mut rules,),
            EligibilityDecision::Allowed(
                crate::profile_state::EligibilityReason::MissionRequirement
            )
        );
        assert_eq!(
            rules.events,
            [
                RuleEvent::Category(CategoryId::new(6)),
                RuleEvent::CategoryCheck(profile, CategoryId::new(6)),
                RuleEvent::Mission(MissionId::new(30)),
                RuleEvent::MissionCheck(profile, MissionId::new(30)),
            ]
        );
    }

    #[test]
    fn stale_profile_fails_requirement_after_delete_but_override_still_short_circuits() {
        let mut state = ProfileLifecycleState::new();
        state.set_eligibility_enabled(true);
        let player = ProfilePlayerId::new(1);
        let profile = profile(&mut state, 7);
        state.set_active_profile(player, Some(profile)).unwrap();
        state.remove_profile(profile).unwrap();
        let mut rules = FabricatedEligibilityRules::default();

        assert_eq!(
            state.category_eligibility(player, CategoryId::new(7), &mut rules),
            EligibilityDecision::Denied(
                crate::profile_state::EligibilityDenial::MissingActiveProfile
            )
        );
        assert_eq!(rules.events, [RuleEvent::Category(CategoryId::new(7))]);

        rules.events.clear();
        state.set_category_unlock_override(CategoryOverride::Two, true);
        assert_eq!(
            state.category_eligibility(player, CategoryId::new(7), &mut rules),
            EligibilityDecision::Allowed(crate::profile_state::EligibilityReason::CategoryOverride)
        );
        assert_eq!(rules.events, [RuleEvent::Category(CategoryId::new(7))]);
    }
}
