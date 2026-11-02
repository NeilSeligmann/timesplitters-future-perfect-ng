// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned profile slot selection and eligibility policy (T3420).
//!
//! The original wrapper ordering is measured in `docs/t1666-rea-profile.md`. This
//! module models its typed decision seam; it does not read guest memory or adapt
//! the existing lifted `save_unlock` implementation. Native eligibility stays
//! disabled until a data source and the Rule 24 replay adoption gate are ready.

use alloc::collections::BTreeMap;

/// The profile loader's indexed table has sixteen entries (slot stride `0x1F54`).
pub const PROFILE_SLOT_COUNT: usize = 16;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProfileSlotId(u8);

impl ProfileSlotId {
    pub fn from_index(index: usize) -> Option<Self> {
        (index < PROFILE_SLOT_COUNT).then_some(Self(index as u8))
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Local player identity is independent of the profile file slot and UI row.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProfilePlayerId(u32);

impl ProfilePlayerId {
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u32 {
        self.0
    }
}

/// Category and mission identifiers are signed because the original wrappers
/// use signed comparisons before applying the category 2/3 override.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CategoryId(i32);

impl CategoryId {
    pub const fn new(raw: i32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> i32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MissionId(i32);

impl MissionId {
    pub const fn new(raw: i32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> i32 {
        self.0
    }
}

/// An indexed slot reference includes a generation so replacing a file in the
/// same table position cannot silently retarget an existing selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileSlotRef {
    slot: ProfileSlotId,
    generation: u32,
}

impl ProfileSlotRef {
    pub const fn slot(self) -> ProfileSlotId {
        self.slot
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileSlotError {
    AlreadyOccupied,
    NotCurrent,
    GenerationExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveProfileState {
    None,
    Present(ProfileSlotRef),
    /// The selected file was removed or replaced. A caller must explicitly
    /// refresh/rebind; this state never chooses a fallback slot on its own.
    Stale(ProfileSlotRef),
}

/// Rust-owned occupancy and selection state for the sixteen indexed profile slots.
/// The UI focus is a stable slot reference, not a visible row number. Removing a
/// selected slot clears browser focus but retains the active binding as stale,
/// matching the measured immediate delete boundary without inventing later
/// catalogue-refresh or fallback behavior.
#[derive(Clone, Debug)]
pub struct ProfileSlotTable {
    occupied: [bool; PROFILE_SLOT_COUNT],
    generations: [u32; PROFILE_SLOT_COUNT],
    active_by_player: BTreeMap<ProfilePlayerId, ProfileSlotRef>,
    browser_focus: Option<ProfileSlotRef>,
}

impl Default for ProfileSlotTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileSlotTable {
    pub fn new() -> Self {
        Self {
            occupied: [false; PROFILE_SLOT_COUNT],
            generations: [0; PROFILE_SLOT_COUNT],
            active_by_player: BTreeMap::new(),
            browser_focus: None,
        }
    }

    pub fn insert(&mut self, slot: ProfileSlotId) -> Result<ProfileSlotRef, ProfileSlotError> {
        let index = slot.index();
        if self.occupied[index] {
            return Err(ProfileSlotError::AlreadyOccupied);
        }
        let generation = self.generations[index]
            .checked_add(1)
            .ok_or(ProfileSlotError::GenerationExhausted)?;
        self.generations[index] = generation;
        self.occupied[index] = true;
        Ok(ProfileSlotRef { slot, generation })
    }

    /// Removes the current occupant. Existing player bindings retain their old
    /// generation and therefore become `Stale` until an explicit caller action.
    pub fn remove(&mut self, reference: ProfileSlotRef) -> Result<(), ProfileSlotError> {
        if !self.is_current(reference) {
            return Err(ProfileSlotError::NotCurrent);
        }
        self.occupied[reference.slot.index()] = false;
        if self.browser_focus == Some(reference) {
            self.browser_focus = None;
        }
        Ok(())
    }

    pub fn contains(&self, reference: ProfileSlotRef) -> bool {
        self.is_current(reference)
    }

    /// Returns the current generation occupying an indexed slot, if any.
    pub fn current_reference(&self, slot: ProfileSlotId) -> Option<ProfileSlotRef> {
        let index = slot.index();
        self.occupied[index].then_some(ProfileSlotRef {
            slot,
            generation: self.generations[index],
        })
    }

    pub fn set_active_profile(
        &mut self,
        player: ProfilePlayerId,
        profile: Option<ProfileSlotRef>,
    ) -> Result<(), ProfileSlotError> {
        if let Some(reference) = profile {
            if !self.is_current(reference) {
                return Err(ProfileSlotError::NotCurrent);
            }
            self.active_by_player.insert(player, reference);
        } else {
            self.active_by_player.remove(&player);
        }
        Ok(())
    }

    pub fn active_profile(&self, player: ProfilePlayerId) -> ActiveProfileState {
        let Some(reference) = self.active_by_player.get(&player).copied() else {
            return ActiveProfileState::None;
        };
        if self.is_current(reference) {
            ActiveProfileState::Present(reference)
        } else {
            ActiveProfileState::Stale(reference)
        }
    }

    pub fn focus(&self) -> Option<ProfileSlotRef> {
        self.browser_focus
    }

    pub fn set_focus(&mut self, focus: Option<ProfileSlotRef>) -> Result<(), ProfileSlotError> {
        if let Some(reference) = focus {
            if !self.is_current(reference) {
                return Err(ProfileSlotError::NotCurrent);
            }
        }
        self.browser_focus = focus;
        Ok(())
    }

    fn is_current(&self, reference: ProfileSlotRef) -> bool {
        let index = reference.slot.index();
        self.occupied[index] && self.generations[index] == reference.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CategoryOverride {
    Two,
    Three,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EligibilityReason {
    GlobalOverride,
    CategoryOverride,
    MissionOverride,
    CategoryRequirement,
    MissionRequirement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EligibilityDenial {
    UnknownCategory,
    UnknownMission,
    MissingActiveProfile,
    RequirementUnmet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EligibilityDecision {
    Disabled,
    Allowed(EligibilityReason),
    Denied(EligibilityDenial),
}

/// Typed source seam for the owned category and mission requirement tables.
/// Implementations receive profile slot identities, never guest addresses.
pub trait EligibilityRules {
    type CategoryDescriptor: Copy;
    type MissionDescriptor: Copy;

    fn category_descriptor(&mut self, id: CategoryId) -> Option<Self::CategoryDescriptor>;
    fn mission_descriptor(&mut self, id: MissionId) -> Option<Self::MissionDescriptor>;
    fn category_requirement_met(
        &mut self,
        profile: ProfileSlotRef,
        descriptor: Self::CategoryDescriptor,
    ) -> bool;
    fn mission_requirement_met(
        &mut self,
        profile: ProfileSlotRef,
        descriptor: Self::MissionDescriptor,
    ) -> bool;
}

/// Default-off native profile eligibility policy. The two override values
/// represent the measured category 2 and category 3 override lookups; they are
/// supplied by an owned data provider and are not synthesized from save bytes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EligibilityPolicy {
    enabled: bool,
    global_override: bool,
    category_2_override: bool,
    category_3_override: bool,
}

impl EligibilityPolicy {
    pub const fn new() -> Self {
        Self {
            enabled: false,
            global_override: false,
            category_2_override: false,
            category_3_override: false,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_global_override(&mut self, enabled: bool) {
        self.global_override = enabled;
    }

    pub fn set_category_override(&mut self, key: CategoryOverride, enabled: bool) {
        match key {
            CategoryOverride::Two => self.category_2_override = enabled,
            CategoryOverride::Three => self.category_3_override = enabled,
        }
    }

    pub fn category_eligible<R: EligibilityRules>(
        &self,
        slots: &ProfileSlotTable,
        player: ProfilePlayerId,
        category: CategoryId,
        rules: &mut R,
    ) -> EligibilityDecision {
        if !self.enabled {
            return EligibilityDecision::Disabled;
        }

        // The original wrapper resolves the descriptor before checking its
        // category 2/3 override, so retain that order at this typed seam.
        let Some(descriptor) = rules.category_descriptor(category) else {
            return EligibilityDecision::Denied(EligibilityDenial::UnknownCategory);
        };
        if self.category_override(category.raw() < 7) {
            return EligibilityDecision::Allowed(EligibilityReason::CategoryOverride);
        }
        let Some(profile) = current_profile(slots, player) else {
            return EligibilityDecision::Denied(EligibilityDenial::MissingActiveProfile);
        };
        if rules.category_requirement_met(profile, descriptor) {
            EligibilityDecision::Allowed(EligibilityReason::CategoryRequirement)
        } else {
            EligibilityDecision::Denied(EligibilityDenial::RequirementUnmet)
        }
    }

    pub fn mission_eligible<R: EligibilityRules>(
        &self,
        slots: &ProfileSlotTable,
        player: ProfilePlayerId,
        category: CategoryId,
        mission: MissionId,
        rules: &mut R,
    ) -> EligibilityDecision {
        if !self.enabled {
            return EligibilityDecision::Disabled;
        }
        if self.global_override {
            return EligibilityDecision::Allowed(EligibilityReason::GlobalOverride);
        }
        if self.category_override(mission.raw() < 21) {
            return EligibilityDecision::Allowed(EligibilityReason::MissionOverride);
        }
        match self.category_eligible(slots, player, category, rules) {
            EligibilityDecision::Allowed(_) => {}
            decision => return decision,
        }

        // Original code obtains the mission descriptor before the shared
        // requirement checker tests active-profile presence.
        let Some(descriptor) = rules.mission_descriptor(mission) else {
            return EligibilityDecision::Denied(EligibilityDenial::UnknownMission);
        };
        let Some(profile) = current_profile(slots, player) else {
            return EligibilityDecision::Denied(EligibilityDenial::MissingActiveProfile);
        };
        if rules.mission_requirement_met(profile, descriptor) {
            EligibilityDecision::Allowed(EligibilityReason::MissionRequirement)
        } else {
            EligibilityDecision::Denied(EligibilityDenial::RequirementUnmet)
        }
    }

    fn category_override(&self, use_three: bool) -> bool {
        if use_three {
            self.category_3_override
        } else {
            self.category_2_override
        }
    }
}

fn current_profile(slots: &ProfileSlotTable, player: ProfilePlayerId) -> Option<ProfileSlotRef> {
    match slots.active_profile(player) {
        ActiveProfileState::Present(profile) => Some(profile),
        ActiveProfileState::None | ActiveProfileState::Stale(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Event {
        Category(CategoryId),
        Mission(MissionId),
        CategoryCheck(ProfileSlotRef, CategoryId),
        MissionCheck(ProfileSlotRef, MissionId),
    }

    #[derive(Default)]
    struct FabricatedRules {
        events: Vec<Event>,
        category_met: bool,
        mission_met: bool,
    }

    impl EligibilityRules for FabricatedRules {
        type CategoryDescriptor = CategoryId;
        type MissionDescriptor = MissionId;

        fn category_descriptor(&mut self, id: CategoryId) -> Option<CategoryId> {
            self.events.push(Event::Category(id));
            (id.raw() >= 0).then_some(id)
        }

        fn mission_descriptor(&mut self, id: MissionId) -> Option<MissionId> {
            self.events.push(Event::Mission(id));
            (id.raw() >= 0).then_some(id)
        }

        fn category_requirement_met(
            &mut self,
            profile: ProfileSlotRef,
            descriptor: CategoryId,
        ) -> bool {
            self.events.push(Event::CategoryCheck(profile, descriptor));
            self.category_met
        }

        fn mission_requirement_met(
            &mut self,
            profile: ProfileSlotRef,
            descriptor: MissionId,
        ) -> bool {
            self.events.push(Event::MissionCheck(profile, descriptor));
            self.mission_met
        }
    }

    fn slot_table() -> (ProfileSlotTable, ProfilePlayerId, ProfileSlotRef) {
        let mut slots = ProfileSlotTable::new();
        let player = ProfilePlayerId::new(2);
        let profile = slots.insert(ProfileSlotId::from_index(5).unwrap()).unwrap();
        slots.set_active_profile(player, Some(profile)).unwrap();
        (slots, player, profile)
    }

    #[test]
    fn default_policy_is_off_and_does_not_query_fabricated_tables() {
        let (slots, player, _) = slot_table();
        let policy = EligibilityPolicy::default();
        let mut rules = FabricatedRules::default();

        assert_eq!(
            policy.mission_eligible(
                &slots,
                player,
                CategoryId::new(1),
                MissionId::new(4),
                &mut rules,
            ),
            EligibilityDecision::Disabled
        );
        assert!(rules.events.is_empty());
    }

    #[test]
    fn global_override_precedes_profile_and_descriptor_lookups() {
        let slots = ProfileSlotTable::new();
        let player = ProfilePlayerId::new(0);
        let mut policy = EligibilityPolicy::new();
        policy.set_enabled(true);
        policy.set_global_override(true);
        let mut rules = FabricatedRules::default();

        assert_eq!(
            policy.mission_eligible(
                &slots,
                player,
                CategoryId::new(-1),
                MissionId::new(-1),
                &mut rules,
            ),
            EligibilityDecision::Allowed(EligibilityReason::GlobalOverride)
        );
        assert!(rules.events.is_empty());
    }

    #[test]
    fn mission_threshold_selects_category_three_below_twenty_one() {
        let slots = ProfileSlotTable::new();
        let player = ProfilePlayerId::new(0);
        let mut policy = EligibilityPolicy::new();
        policy.set_enabled(true);
        policy.set_category_override(CategoryOverride::Three, true);
        let mut rules = FabricatedRules::default();

        assert_eq!(
            policy.mission_eligible(
                &slots,
                player,
                CategoryId::new(99),
                MissionId::new(20),
                &mut rules,
            ),
            EligibilityDecision::Allowed(EligibilityReason::MissionOverride)
        );
        assert!(rules.events.is_empty());

        assert_eq!(
            policy.mission_eligible(
                &slots,
                player,
                CategoryId::new(99),
                MissionId::new(21),
                &mut rules,
            ),
            EligibilityDecision::Denied(EligibilityDenial::MissingActiveProfile)
        );
        assert_eq!(rules.events, [Event::Category(CategoryId::new(99))]);
    }

    #[test]
    fn category_descriptor_is_resolved_before_its_override() {
        let slots = ProfileSlotTable::new();
        let player = ProfilePlayerId::new(0);
        let mut policy = EligibilityPolicy::new();
        policy.set_enabled(true);
        policy.set_category_override(CategoryOverride::Three, true);
        let mut rules = FabricatedRules::default();

        assert_eq!(
            policy.category_eligible(&slots, player, CategoryId::new(6), &mut rules),
            EligibilityDecision::Allowed(EligibilityReason::CategoryOverride)
        );
        assert_eq!(rules.events, [Event::Category(CategoryId::new(6))]);
    }

    #[test]
    fn category_denial_stops_before_mission_descriptor_lookup() {
        let (slots, player, profile) = slot_table();
        let mut policy = EligibilityPolicy::new();
        policy.set_enabled(true);
        let mut rules = FabricatedRules::default();

        assert_eq!(
            policy.mission_eligible(
                &slots,
                player,
                CategoryId::new(6),
                MissionId::new(30),
                &mut rules,
            ),
            EligibilityDecision::Denied(EligibilityDenial::RequirementUnmet)
        );
        assert_eq!(
            rules.events,
            [
                Event::Category(CategoryId::new(6)),
                Event::CategoryCheck(profile, CategoryId::new(6)),
            ]
        );
    }

    #[test]
    fn mission_descriptor_precedes_missing_profile_check_after_category_override() {
        let slots = ProfileSlotTable::new();
        let player = ProfilePlayerId::new(0);
        let mut policy = EligibilityPolicy::new();
        policy.set_enabled(true);
        // Mission 30 selects override two (kept false); category 7 selects
        // override three, allowing the category stage to pass without a profile.
        policy.set_category_override(CategoryOverride::Three, true);
        let mut rules = FabricatedRules::default();

        assert_eq!(
            policy.mission_eligible(
                &slots,
                player,
                CategoryId::new(6),
                MissionId::new(30),
                &mut rules,
            ),
            EligibilityDecision::Denied(EligibilityDenial::MissingActiveProfile)
        );
        assert_eq!(
            rules.events,
            [
                Event::Category(CategoryId::new(6)),
                Event::Mission(MissionId::new(30)),
            ]
        );
    }

    #[test]
    fn eligibility_passes_typed_profile_and_distinct_category_mission_ids() {
        let (slots, player, profile) = slot_table();
        let mut policy = EligibilityPolicy::new();
        policy.set_enabled(true);
        let mut rules = FabricatedRules {
            category_met: true,
            mission_met: true,
            ..FabricatedRules::default()
        };

        assert_eq!(
            policy.mission_eligible(
                &slots,
                player,
                CategoryId::new(8),
                MissionId::new(21),
                &mut rules,
            ),
            EligibilityDecision::Allowed(EligibilityReason::MissionRequirement)
        );
        assert_eq!(
            rules.events,
            [
                Event::Category(CategoryId::new(8)),
                Event::CategoryCheck(profile, CategoryId::new(8)),
                Event::Mission(MissionId::new(21)),
                Event::MissionCheck(profile, MissionId::new(21)),
            ]
        );
    }

    #[test]
    fn deleting_a_slot_clears_focus_but_keeps_an_explicit_stale_binding() {
        let (mut slots, player, profile) = slot_table();
        slots.set_focus(Some(profile)).unwrap();

        slots.remove(profile).unwrap();

        assert_eq!(slots.focus(), None);
        assert_eq!(
            slots.active_profile(player),
            ActiveProfileState::Stale(profile)
        );
        assert_eq!(
            slots.insert(profile.slot()),
            Ok(ProfileSlotRef {
                slot: profile.slot(),
                generation: profile.generation() + 1,
            })
        );
        assert_eq!(
            slots.active_profile(player),
            ActiveProfileState::Stale(profile)
        );
    }
}
