//! Owned navigation state for the Arcade category and Challenge entry pages.
//!
//! This joins the typed category chooser and per-category mission projection while keeping
//! page layouts, game data loading, and the original chooser transition as separate inputs.

use crate::arcade_challenge::{
    ArcadeCategoryId, ArcadeCategoryMenu, ArcadeCategoryRowId, ArcadeCategorySelectionError,
    ArcadeChallengeBackend, ArcadeChallengeDataProvider, ArcadeChallengeMenu, CategoryBuildError,
    CategoryRowId, ChallengeDescriptor, ChallengeDetailChoices, ChallengeId, ChallengeSetupAttempt,
    ChallengeSetupFailure, ChallengeSetupPlan, ChallengeSetupServices, MenuModeId,
    MenuParticipantCount, OwnedArcadeChallengeData, OwnedChallengeDataProviderError,
    SelectedArcadeCategory, SelectedChallenge, SelectionError,
};
use core::sync::atomic::{AtomicU32, Ordering};

static NEXT_ARCADE_MENU_SESSION_ID: AtomicU32 = AtomicU32::new(1);

/// Identity shared by a session and its projections, including transactional session clones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcadeMenuSessionId(u32);

impl ArcadeMenuSessionId {
    fn next() -> Self {
        Self(NEXT_ARCADE_MENU_SESSION_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Which owned page in the category-to-detail entry flow is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuPage {
    CategoryChooser,
    ChallengeList(ArcadeCategoryId),
    Detail(SelectedChallenge),
}

/// Stable Rust-owned focus identity for the category chooser or mission list.
///
/// Category and mission IDs survive row reordering. The identity says which entry the native
/// UI last focused; it does not claim that this is the original game's focus policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuFocusedEntry {
    Category(ArcadeCategoryId),
    Challenge(ChallengeId),
}

/// Stable caller-assigned identity for one control on an Arcade detail page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcadeMenuDetailControlId(u64);

impl ArcadeMenuDetailControlId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Operation rejected because the session is on a different owned page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuOperation {
    OpenCategory,
    OpenChallenge,
    BackToChallengeList,
    BackToCategoryChooser,
    ConfigureDetail,
}

/// Why an owned catalog snapshot could not replace the active Arcade route.
///
/// Refresh is transactional: a missing active category or mission leaves the old snapshot,
/// route, selections, and projection revision intact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuRefreshError {
    LegacyBackend,
    MissingCategory(ArcadeCategoryId),
    MissingCategoryRow(ArcadeCategoryRowId),
    MissingChallenge(ChallengeId),
    MissingChallengeRow(CategoryRowId),
    ChallengeSelection(SelectionError),
    MissingChallengeMenu,
}

/// Failure while copying a provider and transactionally refreshing an owned menu session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuProviderRefreshError {
    Provider(OwnedChallengeDataProviderError),
    Refresh(ArcadeMenuRefreshError),
}

/// A failed typed route operation. Errors leave the active page and owned selection intact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuSessionError {
    LegacyBackend,
    WrongPage {
        operation: ArcadeMenuOperation,
        current: ArcadeMenuPage,
    },
    UnknownCategory(ArcadeCategoryId),
    UnknownChallenge(ChallengeId),
    CategorySelection(ArcadeCategorySelectionError),
    CategoryBuild(CategoryBuildError),
    ChallengeSelection(SelectionError),
    MissingChallengeMenu,
}

/// Result of accepting the active typed Challenge detail page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuDetailAcceptResult {
    BackendDisabled,
    NotOnDetail,
    ChoicesNotConfigured,
    SetupPending(ChallengeSetupFailure),
    SetupComplete,
}

/// Rust-owned state for Arcade category selection through Challenge detail selection.
///
/// The session consumes owned snapshots. Legacy remains the default backend; callers must
/// explicitly opt into Rust before category or mission rows can transition the session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcadeMenuSession {
    id: ArcadeMenuSessionId,
    data: OwnedArcadeChallengeData,
    detail: ChallengeDescriptor,
    category_menu: ArcadeCategoryMenu,
    challenge_menu: Option<ArcadeChallengeMenu>,
    page: ArcadeMenuPage,
    focused_entry: Option<ArcadeMenuFocusedEntry>,
    focused_challenges: Vec<(ArcadeCategoryId, ChallengeId)>,
    focused_detail_control: Option<(SelectedChallenge, ArcadeMenuDetailControlId)>,
    detail_choices: Option<ChallengeDetailChoices>,
    setup_attempt: Option<ChallengeSetupAttempt>,
    route_revision: u64,
    projection_revision: u64,
}

impl ArcadeMenuSession {
    /// Create a session whose category and mission pages remain on the Legacy backend.
    pub fn new(data: OwnedArcadeChallengeData, detail: ChallengeDescriptor) -> Self {
        Self::with_backend(data, detail, ArcadeChallengeBackend::default())
    }

    /// Copy a provider into an owned session while retaining the Legacy default backend.
    pub fn from_provider(
        provider: &impl ArcadeChallengeDataProvider,
        detail: ChallengeDescriptor,
    ) -> Result<Self, OwnedChallengeDataProviderError> {
        Self::with_backend_from_provider(provider, detail, ArcadeChallengeBackend::default())
    }

    /// Copy a provider into an owned session with an explicit backend choice.
    pub fn with_backend_from_provider(
        provider: &impl ArcadeChallengeDataProvider,
        detail: ChallengeDescriptor,
        backend: ArcadeChallengeBackend,
    ) -> Result<Self, OwnedChallengeDataProviderError> {
        let data = OwnedArcadeChallengeData::copy_from_provider(provider)?;
        Ok(Self::with_backend(data, detail, backend))
    }

    /// Create a session with an explicit backend opt-in.
    pub fn with_backend(
        data: OwnedArcadeChallengeData,
        detail: ChallengeDescriptor,
        backend: ArcadeChallengeBackend,
    ) -> Self {
        let category_menu = ArcadeCategoryMenu::build_with_backend(&data, backend);
        Self {
            id: ArcadeMenuSessionId::next(),
            data,
            detail,
            category_menu,
            challenge_menu: None,
            page: ArcadeMenuPage::CategoryChooser,
            focused_entry: None,
            focused_challenges: Vec::new(),
            focused_detail_control: None,
            detail_choices: None,
            setup_attempt: None,
            route_revision: 0,
            projection_revision: 0,
        }
    }

    /// Identity for rejecting a page projection produced by a different session instance.
    pub const fn id(&self) -> ArcadeMenuSessionId {
        self.id
    }

    pub const fn page(&self) -> ArcadeMenuPage {
        self.page
    }

    pub const fn focused_entry(&self) -> Option<ArcadeMenuFocusedEntry> {
        self.focused_entry
    }

    /// Record focus by category identity while the Rust category chooser is active.
    ///
    /// Disabled or missing rows are rejected by the row map before this state changes.
    pub fn focus_category_row(&mut self, row: ArcadeCategoryRowId) -> bool {
        if self.backend() != ArcadeChallengeBackend::Rust
            || self.page != ArcadeMenuPage::CategoryChooser
        {
            return false;
        }
        let Some(entry) = self
            .category_menu
            .entries()
            .iter()
            .find(|entry| entry.row == row)
        else {
            return false;
        };
        self.focus_entry(ArcadeMenuFocusedEntry::Category(entry.category))
    }

    /// Record focus by mission identity while the Rust mission list is active.
    ///
    /// Visible disabled rows may receive focus so their help action remains reachable.
    pub fn focus_challenge_row(&mut self, row: CategoryRowId) -> bool {
        if self.backend() != ArcadeChallengeBackend::Rust
            || !matches!(self.page, ArcadeMenuPage::ChallengeList(_))
        {
            return false;
        }
        let challenge = match self.page {
            ArcadeMenuPage::ChallengeList(_) => self.challenge_menu.as_ref().and_then(|menu| {
                menu.entries()
                    .iter()
                    .find(|entry| entry.row == row)
                    .map(|entry| entry.challenge)
            }),
            ArcadeMenuPage::CategoryChooser | ArcadeMenuPage::Detail(_) => return false,
        };
        let Some(challenge) = challenge else {
            return false;
        };
        self.focus_entry(ArcadeMenuFocusedEntry::Challenge(challenge))
    }

    /// Record focus using a stable identity supplied by native UI state.
    ///
    /// The identity is resolved against the current owned page, so row reordering does not
    /// change the focused item and identities from another page are rejected.
    pub fn focus_entry(&mut self, entry: ArcadeMenuFocusedEntry) -> bool {
        if self.backend() != ArcadeChallengeBackend::Rust {
            return false;
        }
        match (self.page, entry) {
            (ArcadeMenuPage::CategoryChooser, ArcadeMenuFocusedEntry::Category(category)) => {
                if !self
                    .category_menu
                    .entries()
                    .iter()
                    .any(|entry| entry.category == category)
                {
                    return false;
                }
                self.focused_entry = Some(ArcadeMenuFocusedEntry::Category(category));
                true
            }
            (
                ArcadeMenuPage::ChallengeList(active),
                ArcadeMenuFocusedEntry::Challenge(challenge),
            ) => {
                if self.challenge_menu.as_ref().is_none_or(|menu| {
                    !menu
                        .entries()
                        .iter()
                        .any(|entry| entry.challenge == challenge)
                }) {
                    return false;
                }
                self.focused_entry = Some(ArcadeMenuFocusedEntry::Challenge(challenge));
                self.remember_challenge_focus(active, challenge);
                true
            }
            _ => false,
        }
    }

    /// Record detail-control focus against the selected mission identity.
    ///
    /// The caller resolves its current widget to this stable ID. A layout rebuild can then map
    /// that identity to a replacement widget without storing renderer IDs in the session.
    pub fn focus_detail_control(&mut self, control: ArcadeMenuDetailControlId) -> bool {
        if self.backend() != ArcadeChallengeBackend::Rust {
            return false;
        }
        let ArcadeMenuPage::Detail(selection) = self.page else {
            return false;
        };
        self.focused_detail_control = Some((selection, control));
        true
    }

    /// Detail control focus applies only to the selected mission that recorded it.
    pub fn focused_detail_control(&self) -> Option<ArcadeMenuDetailControlId> {
        match (self.page, self.focused_detail_control) {
            (ArcadeMenuPage::Detail(selection), Some((focused_selection, control)))
                if selection == focused_selection =>
            {
                Some(control)
            }
            _ => None,
        }
    }

    pub fn focused_category_row(&self) -> Option<ArcadeCategoryRowId> {
        let ArcadeMenuFocusedEntry::Category(category) = self.focused_entry? else {
            return None;
        };
        self.category_menu
            .entries()
            .iter()
            .find(|entry| entry.category == category)
            .map(|entry| entry.row)
    }

    /// Return the remembered mission identity for the active category's Rust page.
    ///
    /// This stays separate from `focused_entry`, which tracks the most recent navigation focus
    /// and may point at the category chooser after returning from a mission list.
    pub fn focused_challenge(&self) -> Option<ChallengeId> {
        let ArcadeMenuPage::ChallengeList(category) = self.page else {
            return None;
        };
        self.focused_challenges
            .iter()
            .find_map(|(focused_category, challenge)| {
                (*focused_category == category).then_some(*challenge)
            })
    }

    pub fn focused_challenge_row(&self) -> Option<CategoryRowId> {
        let challenge = self.focused_challenge()?;
        self.challenge_menu
            .as_ref()?
            .entries()
            .iter()
            .find(|entry| entry.challenge == challenge)
            .map(|entry| entry.row)
    }

    fn remember_challenge_focus(&mut self, category: ArcadeCategoryId, challenge: ChallengeId) {
        if let Some((_, remembered)) = self
            .focused_challenges
            .iter_mut()
            .find(|(focused_category, _)| *focused_category == category)
        {
            *remembered = challenge;
        } else {
            self.focused_challenges.push((category, challenge));
        }
    }

    /// Monotonic-within-practical-use revision for invalidating projections from an older page.
    pub const fn route_revision(&self) -> u64 {
        self.route_revision
    }

    /// Revision for invalidating an older page projection after a same-route rebuild.
    pub const fn projection_revision(&self) -> u64 {
        self.projection_revision
    }

    /// Mark a successfully rebuilt presentation as current and invalidate prior projections.
    pub fn begin_projection(&mut self) -> u64 {
        self.projection_revision = self.projection_revision.wrapping_add(1);
        self.projection_revision
    }

    pub const fn backend(&self) -> ArcadeChallengeBackend {
        self.category_menu.backend()
    }

    pub fn category_menu(&self) -> &ArcadeCategoryMenu {
        &self.category_menu
    }

    pub fn challenge_menu(&self) -> Option<&ArcadeChallengeMenu> {
        self.challenge_menu.as_ref()
    }

    pub const fn selected_challenge(&self) -> Option<SelectedChallenge> {
        match self.page {
            ArcadeMenuPage::Detail(selection) => Some(selection),
            ArcadeMenuPage::CategoryChooser | ArcadeMenuPage::ChallengeList(_) => None,
        }
    }

    /// The session owns the acknowledged prefix of detail setup operations.
    pub const fn setup_attempt(&self) -> Option<&ChallengeSetupAttempt> {
        self.setup_attempt.as_ref()
    }

    /// Current user-selected mode and participant count for the active detail entry.
    pub const fn detail_choices(&self) -> Option<ChallengeDetailChoices> {
        self.detail_choices
    }

    /// Commit explicit native detail-page choices to Rust-owned session state.
    ///
    /// No mode/count defaults or allowed-value policy are inferred here. Updating the choices
    /// invalidates any partially applied setup plan so a later Accept starts from the new state.
    pub fn set_detail_choices(
        &mut self,
        choices: ChallengeDetailChoices,
    ) -> Result<(), ArcadeMenuSessionError> {
        if self.backend() == ArcadeChallengeBackend::Legacy {
            return Err(ArcadeMenuSessionError::LegacyBackend);
        }
        if !matches!(self.page, ArcadeMenuPage::Detail(_)) {
            return Err(ArcadeMenuSessionError::WrongPage {
                operation: ArcadeMenuOperation::ConfigureDetail,
                current: self.page,
            });
        }
        if self.detail_choices != Some(choices) {
            self.setup_attempt = None;
            self.detail_choices = Some(choices);
        }
        Ok(())
    }

    /// Apply the measured detail-accept plan and retain each service-confirmed operation.
    ///
    /// The session is the source of truth for native menu setup state. A failed service keeps
    /// its confirmed prefix here, and a retry resumes only when the plan still matches.
    pub fn accept_detail<S: ChallengeSetupServices + ?Sized>(
        &mut self,
        mode: MenuModeId,
        participant_count: MenuParticipantCount,
        services: &mut S,
    ) -> ArcadeMenuDetailAcceptResult {
        if self.backend() != ArcadeChallengeBackend::Rust {
            return ArcadeMenuDetailAcceptResult::BackendDisabled;
        }
        if self.selected_challenge().is_none() {
            return ArcadeMenuDetailAcceptResult::NotOnDetail;
        }
        let choices = ChallengeDetailChoices::new(mode, participant_count);
        if self.detail_choices != Some(choices) {
            self.detail_choices = Some(choices);
            self.setup_attempt = None;
        }
        self.accept_configured_detail(services)
    }

    /// Apply setup using only the choices already committed to this Rust-owned detail session.
    pub fn accept_configured_detail<S: ChallengeSetupServices + ?Sized>(
        &mut self,
        services: &mut S,
    ) -> ArcadeMenuDetailAcceptResult {
        if self.backend() != ArcadeChallengeBackend::Rust {
            return ArcadeMenuDetailAcceptResult::BackendDisabled;
        }
        let Some(selection) = self.selected_challenge() else {
            return ArcadeMenuDetailAcceptResult::NotOnDetail;
        };
        let Some(choices) = self.detail_choices else {
            return ArcadeMenuDetailAcceptResult::ChoicesNotConfigured;
        };

        let plan = ChallengeSetupPlan::from_selected(
            selection,
            choices.mode(),
            choices.participant_count(),
        );
        let attempt = match self.setup_attempt.take() {
            Some(previous) if previous.matches_plan(&plan) => {
                plan.resume_tracked(services, previous)
            }
            Some(_) | None => plan.apply_tracked(services),
        };
        let result = if attempt.is_complete() {
            ArcadeMenuDetailAcceptResult::SetupComplete
        } else {
            ArcadeMenuDetailAcceptResult::SetupPending(
                attempt
                    .failure()
                    .expect("incomplete setup attempts retain their service failure"),
            )
        };
        self.setup_attempt = Some(attempt);
        result
    }

    /// Select a typed chooser row and build its mission projection before committing the route.
    ///
    /// Construction is transactional: a Legacy backend, unknown row, or missing category does
    /// not replace the active page or clear the previous route state.
    pub fn open_category_row(
        &mut self,
        row: ArcadeCategoryRowId,
    ) -> Result<SelectedArcadeCategory, ArcadeMenuSessionError> {
        if self.page != ArcadeMenuPage::CategoryChooser {
            return Err(ArcadeMenuSessionError::WrongPage {
                operation: ArcadeMenuOperation::OpenCategory,
                current: self.page,
            });
        }
        if self.backend() == ArcadeChallengeBackend::Legacy {
            return Err(ArcadeMenuSessionError::CategorySelection(
                ArcadeCategorySelectionError::LegacyBackend,
            ));
        }
        let Some(entry) = self
            .category_menu
            .entries()
            .iter()
            .find(|entry| entry.row == row)
        else {
            return Err(ArcadeMenuSessionError::CategorySelection(
                ArcadeCategorySelectionError::UnknownRow(row),
            ));
        };

        let challenge_menu = ArcadeChallengeMenu::build_from_provider(
            entry.category,
            self.detail,
            &self.data,
            self.backend(),
        )
        .map_err(ArcadeMenuSessionError::CategoryBuild)?;
        let selected = self
            .category_menu
            .select(row)
            .map_err(ArcadeMenuSessionError::CategorySelection)?;

        self.focused_entry = Some(ArcadeMenuFocusedEntry::Category(selected.category));
        self.challenge_menu = Some(challenge_menu);
        self.page = ArcadeMenuPage::ChallengeList(selected.category);
        self.detail_choices = None;
        self.setup_attempt = None;
        self.route_revision = self.route_revision.wrapping_add(1);
        Ok(selected)
    }

    /// Open a category by stable identity, resolving its current row only at the page boundary.
    pub fn open_category(
        &mut self,
        category: ArcadeCategoryId,
    ) -> Result<SelectedArcadeCategory, ArcadeMenuSessionError> {
        if self.page != ArcadeMenuPage::CategoryChooser {
            return Err(ArcadeMenuSessionError::WrongPage {
                operation: ArcadeMenuOperation::OpenCategory,
                current: self.page,
            });
        }
        let row = self
            .category_menu
            .entries()
            .iter()
            .find(|entry| entry.category == category)
            .map(|entry| entry.row)
            .ok_or(ArcadeMenuSessionError::UnknownCategory(category))?;
        self.open_category_row(row)
    }

    /// Select an available mission row and move the typed route to its detail entry.
    pub fn open_challenge_row(
        &mut self,
        row: CategoryRowId,
    ) -> Result<SelectedChallenge, ArcadeMenuSessionError> {
        let ArcadeMenuPage::ChallengeList(category) = self.page else {
            return Err(ArcadeMenuSessionError::WrongPage {
                operation: ArcadeMenuOperation::OpenChallenge,
                current: self.page,
            });
        };
        let Some(challenge_menu) = self.challenge_menu.as_mut() else {
            return Err(ArcadeMenuSessionError::MissingChallengeMenu);
        };
        let selection = challenge_menu
            .select(row)
            .map_err(ArcadeMenuSessionError::ChallengeSelection)?;
        debug_assert_eq!(selection.category, category);
        self.focused_entry = Some(ArcadeMenuFocusedEntry::Challenge(selection.challenge));
        self.remember_challenge_focus(selection.category, selection.challenge);
        self.page = ArcadeMenuPage::Detail(selection);
        self.detail_choices = None;
        self.setup_attempt = None;
        self.route_revision = self.route_revision.wrapping_add(1);
        Ok(selection)
    }

    /// Open a Challenge by stable identity, resolving its current row on the active list.
    pub fn open_challenge(
        &mut self,
        challenge: ChallengeId,
    ) -> Result<SelectedChallenge, ArcadeMenuSessionError> {
        let ArcadeMenuPage::ChallengeList(_) = self.page else {
            return Err(ArcadeMenuSessionError::WrongPage {
                operation: ArcadeMenuOperation::OpenChallenge,
                current: self.page,
            });
        };
        let row = self
            .challenge_menu
            .as_ref()
            .and_then(|menu| {
                menu.entries()
                    .iter()
                    .find(|entry| entry.challenge == challenge)
                    .map(|entry| entry.row)
            })
            .ok_or(ArcadeMenuSessionError::UnknownChallenge(challenge))?;
        self.open_challenge_row(row)
    }

    /// Return from detail while retaining the category mission row for focus restoration.
    pub fn back_to_challenge_list(&mut self) -> Result<(), ArcadeMenuSessionError> {
        let ArcadeMenuPage::Detail(selection) = self.page else {
            return Err(ArcadeMenuSessionError::WrongPage {
                operation: ArcadeMenuOperation::BackToChallengeList,
                current: self.page,
            });
        };
        self.page = ArcadeMenuPage::ChallengeList(selection.category);
        self.detail_choices = None;
        self.setup_attempt = None;
        self.route_revision = self.route_revision.wrapping_add(1);
        Ok(())
    }

    /// Return from a category's mission list and discard its menu projection and selection.
    /// The category chooser retains its selected row so an owned UI can restore focus.
    pub fn back_to_category_chooser(&mut self) -> Result<(), ArcadeMenuSessionError> {
        let ArcadeMenuPage::ChallengeList(category) = self.page else {
            return Err(ArcadeMenuSessionError::WrongPage {
                operation: ArcadeMenuOperation::BackToCategoryChooser,
                current: self.page,
            });
        };
        self.challenge_menu = None;
        self.page = ArcadeMenuPage::CategoryChooser;
        self.detail_choices = None;
        self.focused_entry = Some(ArcadeMenuFocusedEntry::Category(category));
        self.setup_attempt = None;
        self.route_revision = self.route_revision.wrapping_add(1);
        Ok(())
    }

    /// Replace the owned catalog while retaining selected identities across row reordering.
    ///
    /// The operation is available only after explicit Rust-backend opt-in. If the active
    /// category or selected mission is absent or unavailable in the new snapshot, no part of
    /// the session changes. A successful refresh advances the route revision so events from
    /// projections built from the prior snapshot are denied.
    pub fn refresh_catalog(
        &mut self,
        data: OwnedArcadeChallengeData,
    ) -> Result<(), ArcadeMenuRefreshError> {
        if self.backend() != ArcadeChallengeBackend::Rust {
            return Err(ArcadeMenuRefreshError::LegacyBackend);
        }

        let mut category_menu =
            ArcadeCategoryMenu::build_with_backend(&data, ArcadeChallengeBackend::Rust);
        if let Some(old_row) = self.category_menu.selected_row() {
            let selected_category = self
                .category_menu
                .entries()
                .iter()
                .find(|entry| entry.row == old_row)
                .map(|entry| entry.category)
                .ok_or(ArcadeMenuRefreshError::MissingCategoryRow(old_row))?;
            let new_row = category_menu
                .entries()
                .iter()
                .find(|entry| entry.category == selected_category)
                .map(|entry| entry.row)
                .ok_or(ArcadeMenuRefreshError::MissingCategory(selected_category))?;
            category_menu
                .select(new_row)
                .map_err(|_| ArcadeMenuRefreshError::MissingCategory(selected_category))?;
        }

        let mut challenge_menu = None;
        let mut page = self.page;
        if let Some(category) = self.page_category() {
            let mut rebuilt = ArcadeChallengeMenu::build_from_provider(
                category,
                self.detail,
                &data,
                ArcadeChallengeBackend::Rust,
            )
            .map_err(|_| ArcadeMenuRefreshError::MissingCategory(category))?;

            match self.page {
                ArcadeMenuPage::ChallengeList(_) => {
                    let old_menu = self
                        .challenge_menu
                        .as_ref()
                        .ok_or(ArcadeMenuRefreshError::MissingChallengeMenu)?;
                    if let Some(old_row) = old_menu.selected_row() {
                        let challenge = old_menu
                            .entries()
                            .iter()
                            .find(|entry| entry.row == old_row)
                            .map(|entry| entry.challenge)
                            .ok_or(ArcadeMenuRefreshError::MissingChallengeRow(old_row))?;
                        let new_row = rebuilt
                            .entries()
                            .iter()
                            .find(|entry| entry.challenge == challenge)
                            .map(|entry| entry.row)
                            .ok_or(ArcadeMenuRefreshError::MissingChallenge(challenge))?;
                        rebuilt
                            .select(new_row)
                            .map_err(ArcadeMenuRefreshError::ChallengeSelection)?;
                    }
                }
                ArcadeMenuPage::Detail(selection) => {
                    let new_row = rebuilt
                        .entries()
                        .iter()
                        .find(|entry| entry.challenge == selection.challenge)
                        .map(|entry| entry.row)
                        .ok_or(ArcadeMenuRefreshError::MissingChallenge(
                            selection.challenge,
                        ))?;
                    let updated = rebuilt
                        .select(new_row)
                        .map_err(ArcadeMenuRefreshError::ChallengeSelection)?;
                    page = ArcadeMenuPage::Detail(updated);
                }
                ArcadeMenuPage::CategoryChooser => unreachable!("page_category excludes chooser"),
            }
            challenge_menu = Some(rebuilt);
        }

        let previous_selection = self.selected_challenge();
        let focused_entry = match self.focused_entry {
            Some(ArcadeMenuFocusedEntry::Category(category)) => category_menu
                .entries()
                .iter()
                .any(|entry| entry.category == category)
                .then_some(ArcadeMenuFocusedEntry::Category(category)),
            Some(ArcadeMenuFocusedEntry::Challenge(challenge)) => challenge_menu
                .as_ref()
                .is_some_and(|menu| {
                    menu.entries()
                        .iter()
                        .any(|entry| entry.challenge == challenge)
                })
                .then_some(ArcadeMenuFocusedEntry::Challenge(challenge)),
            None => None,
        };
        let focused_challenges = self
            .focused_challenges
            .iter()
            .copied()
            .filter(|(category, challenge)| {
                ArcadeChallengeMenu::build_from_provider(
                    *category,
                    self.detail,
                    &data,
                    ArcadeChallengeBackend::Rust,
                )
                .is_ok_and(|menu| {
                    menu.entries()
                        .iter()
                        .any(|entry| entry.challenge == *challenge)
                })
            })
            .collect();
        self.data = data;
        self.category_menu = category_menu;
        self.challenge_menu = challenge_menu;
        self.page = page;
        self.focused_entry = focused_entry;
        self.focused_challenges = focused_challenges;
        if self.selected_challenge() != previous_selection {
            self.detail_choices = None;
            self.setup_attempt = None;
        }
        self.route_revision = self.route_revision.wrapping_add(1);
        Ok(())
    }

    /// Copy a stable provider view, then transactionally replace the session's owned catalog.
    ///
    /// The provider is consulted only while making the detached snapshot. Both copy failures
    /// and route-reconciliation failures leave the prior session intact.
    pub fn refresh_from_provider(
        &mut self,
        provider: &impl ArcadeChallengeDataProvider,
    ) -> Result<(), ArcadeMenuProviderRefreshError> {
        if self.backend() != ArcadeChallengeBackend::Rust {
            return Err(ArcadeMenuProviderRefreshError::Refresh(
                ArcadeMenuRefreshError::LegacyBackend,
            ));
        }
        let data = OwnedArcadeChallengeData::copy_from_provider(provider)
            .map_err(ArcadeMenuProviderRefreshError::Provider)?;
        self.refresh_catalog(data)
            .map_err(ArcadeMenuProviderRefreshError::Refresh)
    }

    fn page_category(&self) -> Option<ArcadeCategoryId> {
        match self.page {
            ArcadeMenuPage::CategoryChooser => None,
            ArcadeMenuPage::ChallengeList(category) => Some(category),
            ArcadeMenuPage::Detail(selection) => Some(selection.category),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArcadeMenuFocusedEntry, ArcadeMenuOperation, ArcadeMenuPage, ArcadeMenuRefreshError,
        ArcadeMenuSession, ArcadeMenuSessionError,
    };
    use crate::arcade_challenge::{
        ArcadeCategoryId, ArcadeCategoryRowId, ArcadeCategorySnapshot, ArcadeChallengeBackend,
        CategoryRowData, CategoryRowId, ChallengeDescriptor, ChallengeDetailChoices, ChallengeId,
        ChallengeRecordSnapshot, ChallengeSetupFailure, ChallengeSetupOperation,
        ChallengeSetupServices, EntryAvailability, MenuHelpId, MenuModeId, MenuParticipantCount,
        MissionRecordSnapshot, OwnedArcadeChallengeData, OwnedChallengeDataError,
        SelectedArcadeCategory, SelectedChallenge, SelectionError,
    };

    fn fabricated_data() -> OwnedArcadeChallengeData {
        OwnedArcadeChallengeData::new(
            vec![
                ArcadeCategorySnapshot::new(
                    ArcadeCategoryId::new(12),
                    vec![ChallengeId::new(100), ChallengeId::new(101)],
                ),
                ArcadeCategorySnapshot::new(ArcadeCategoryId::new(13), vec![ChallengeId::new(102)]),
            ],
            vec![
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(100),
                    MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: false,
                            availability: EntryAvailability::Available,
                        }),
                    },
                ),
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(101),
                    MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: false,
                            availability: EntryAvailability::Disabled {
                                help: Some(MenuHelpId::new(900)),
                            },
                        }),
                    },
                ),
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(102),
                    MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: false,
                            availability: EntryAvailability::Available,
                        }),
                    },
                ),
            ],
        )
        .expect("fabricated data has unique identities")
    }

    #[test]
    fn fabricated_provider_constructor_copies_data_and_keeps_legacy_default() {
        let provider = fabricated_data();
        let session =
            ArcadeMenuSession::from_provider(&provider, ChallengeDescriptor::new(0x523870))
                .unwrap();
        drop(provider);

        assert_eq!(session.backend(), ArcadeChallengeBackend::Legacy);
        assert_eq!(session.category_menu().entries().len(), 2);
        assert_eq!(
            session.category_menu().entries()[0].category,
            ArcadeCategoryId::new(12)
        );
    }

    fn rust_session() -> ArcadeMenuSession {
        ArcadeMenuSession::with_backend(
            fabricated_data(),
            ChallengeDescriptor::new(0x523870),
            ArcadeChallengeBackend::Rust,
        )
    }

    #[derive(Default)]
    struct RecordingSetupServices(Vec<ChallengeSetupOperation>);

    impl ChallengeSetupServices for RecordingSetupServices {
        fn apply(
            &mut self,
            operation: ChallengeSetupOperation,
        ) -> Result<(), ChallengeSetupFailure> {
            self.0.push(operation);
            Ok(())
        }
    }

    #[test]
    fn fabricated_detail_choices_are_owned_and_required_before_setup_accept() {
        let mut session = rust_session();
        let before = session.clone();
        assert!(
            session
                .set_detail_choices(ChallengeDetailChoices::new(
                    MenuModeId::new(7),
                    MenuParticipantCount::new(2),
                ))
                .is_err()
        );
        assert_eq!(session, before);

        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        session.open_challenge_row(CategoryRowId::new(0)).unwrap();
        let mut services = RecordingSetupServices::default();
        assert_eq!(
            session.accept_configured_detail(&mut services),
            super::ArcadeMenuDetailAcceptResult::ChoicesNotConfigured
        );
        assert!(services.0.is_empty());

        let choices = ChallengeDetailChoices::new(MenuModeId::new(7), MenuParticipantCount::new(2));
        session.set_detail_choices(choices).unwrap();
        assert_eq!(session.detail_choices(), Some(choices));
        assert_eq!(
            session.accept_configured_detail(&mut services),
            super::ArcadeMenuDetailAcceptResult::SetupComplete
        );
        assert!(services.0.iter().any(|operation| matches!(
            operation,
            ChallengeSetupOperation::InitializeParticipants(participants)
                if participants.count() == MenuParticipantCount::new(2)
        )));
        assert!(session.setup_attempt().unwrap().is_complete());

        let changed = ChallengeDetailChoices::new(MenuModeId::new(3), MenuParticipantCount::new(1));
        session.set_detail_choices(changed).unwrap();
        assert_eq!(session.detail_choices(), Some(changed));
        assert!(session.setup_attempt().is_none());
    }

    #[test]
    fn fabricated_session_identity_survives_transaction_clones_only() {
        let original = rust_session();
        let candidate = original.clone();
        let separate = rust_session();

        assert_eq!(original.id(), candidate.id());
        assert_ne!(original.id(), separate.id());
    }

    fn reordered_data(disable_selected: bool, hide_focused: bool) -> OwnedArcadeChallengeData {
        let availability = if disable_selected {
            EntryAvailability::Disabled {
                help: Some(MenuHelpId::new(901)),
            }
        } else {
            EntryAvailability::Available
        };
        OwnedArcadeChallengeData::new(
            vec![
                ArcadeCategorySnapshot::new(ArcadeCategoryId::new(13), vec![ChallengeId::new(102)]),
                ArcadeCategorySnapshot::new(
                    ArcadeCategoryId::new(12),
                    vec![ChallengeId::new(101), ChallengeId::new(100)],
                ),
            ],
            vec![
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(100),
                    MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: false,
                            availability,
                        }),
                    },
                ),
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(101),
                    MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: hide_focused,
                            availability: EntryAvailability::Disabled {
                                help: Some(MenuHelpId::new(900)),
                            },
                        }),
                    },
                ),
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(102),
                    MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: false,
                            availability: EntryAvailability::Available,
                        }),
                    },
                ),
            ],
        )
        .expect("fabricated reordered data has unique identities")
    }

    #[test]
    fn fabricated_legacy_session_rejects_category_open_without_state_change() {
        let mut session =
            ArcadeMenuSession::new(fabricated_data(), ChallengeDescriptor::new(0x523870));
        let before = session.clone();

        assert_eq!(
            session.open_category_row(ArcadeCategoryRowId::new(0)),
            Err(ArcadeMenuSessionError::CategorySelection(
                super::ArcadeCategorySelectionError::LegacyBackend
            ))
        );
        assert_eq!(
            session.refresh_from_provider(&fabricated_data()),
            Err(super::ArcadeMenuProviderRefreshError::Refresh(
                ArcadeMenuRefreshError::LegacyBackend
            ))
        );
        assert_eq!(session, before);
    }

    #[test]
    fn fabricated_category_to_detail_route_keeps_typed_identities() {
        let mut session = rust_session();
        let category = session
            .open_category_row(ArcadeCategoryRowId::new(1))
            .unwrap();
        assert_eq!(
            category,
            SelectedArcadeCategory {
                row: ArcadeCategoryRowId::new(1),
                category: ArcadeCategoryId::new(13),
            }
        );
        assert_eq!(
            session.page(),
            ArcadeMenuPage::ChallengeList(ArcadeCategoryId::new(13))
        );
        let selection = session.open_challenge_row(CategoryRowId::new(0)).unwrap();
        assert_eq!(
            selection,
            SelectedChallenge {
                category: ArcadeCategoryId::new(13),
                row: CategoryRowId::new(0),
                challenge: ChallengeId::new(102),
                detail: ChallengeDescriptor::new(0x523870),
            }
        );
        assert_eq!(session.page(), ArcadeMenuPage::Detail(selection));
        assert_eq!(session.selected_challenge(), Some(selection));
    }

    #[test]
    fn fabricated_stable_identity_routes_resolve_current_rows_after_reorder() {
        let mut session = rust_session();
        session
            .refresh_catalog(reordered_data(false, false))
            .unwrap();

        assert!(session.focus_entry(ArcadeMenuFocusedEntry::Category(ArcadeCategoryId::new(12))));
        assert_eq!(
            session.focused_category_row(),
            Some(ArcadeCategoryRowId::new(1))
        );
        let category = session.open_category(ArcadeCategoryId::new(12)).unwrap();
        assert_eq!(category.row, ArcadeCategoryRowId::new(1));

        assert!(session.focus_entry(ArcadeMenuFocusedEntry::Challenge(ChallengeId::new(100))));
        assert_eq!(session.focused_challenge_row(), Some(CategoryRowId::new(1)));
        let selected = session.open_challenge(ChallengeId::new(100)).unwrap();
        assert_eq!(selected.challenge, ChallengeId::new(100));
        assert_eq!(selected.row, CategoryRowId::new(1));
        assert_eq!(session.page(), ArcadeMenuPage::Detail(selected));
    }

    #[test]
    fn fabricated_unknown_stable_identity_routes_leave_session_unchanged() {
        let mut session = rust_session();
        let before = session.clone();
        assert_eq!(
            session.open_category(ArcadeCategoryId::new(999)),
            Err(ArcadeMenuSessionError::UnknownCategory(
                ArcadeCategoryId::new(999)
            ))
        );
        assert_eq!(session, before);

        session.open_category(ArcadeCategoryId::new(12)).unwrap();
        let before = session.clone();
        assert_eq!(
            session.open_challenge(ChallengeId::new(999)),
            Err(ArcadeMenuSessionError::UnknownChallenge(ChallengeId::new(
                999
            )))
        );
        assert_eq!(session, before);
        assert!(!session.focus_entry(ArcadeMenuFocusedEntry::Challenge(ChallengeId::new(999))));
        assert_eq!(session, before);
    }

    #[test]
    fn fabricated_catalog_refresh_preserves_selected_ids_after_row_reorder() {
        let mut session = rust_session();
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        let selected = session.open_challenge_row(CategoryRowId::new(0)).unwrap();
        let old_revision = session.route_revision();

        let provider = reordered_data(false, false);
        session.refresh_from_provider(&provider).unwrap();
        drop(provider);

        assert_eq!(session.route_revision(), old_revision + 1);
        assert_eq!(
            session.category_menu().selected_row(),
            Some(ArcadeCategoryRowId::new(1))
        );
        assert_eq!(
            session.page(),
            ArcadeMenuPage::Detail(SelectedChallenge {
                category: selected.category,
                row: CategoryRowId::new(1),
                challenge: selected.challenge,
                detail: selected.detail,
            })
        );
        assert_eq!(
            session.challenge_menu().unwrap().selected_row(),
            Some(CategoryRowId::new(1))
        );
    }

    #[test]
    fn fabricated_catalog_refresh_preserves_chooser_and_list_focus_by_identity() {
        let mut chooser = rust_session();
        chooser
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        chooser.back_to_category_chooser().unwrap();
        chooser
            .refresh_catalog(reordered_data(false, false))
            .unwrap();
        assert_eq!(chooser.page(), ArcadeMenuPage::CategoryChooser);
        assert_eq!(
            chooser.focused_entry(),
            Some(ArcadeMenuFocusedEntry::Category(ArcadeCategoryId::new(12)))
        );
        assert_eq!(
            chooser.focused_category_row(),
            Some(ArcadeCategoryRowId::new(1))
        );
        assert_eq!(
            chooser.category_menu().selected_row(),
            Some(ArcadeCategoryRowId::new(1))
        );

        let mut list = rust_session();
        list.open_category_row(ArcadeCategoryRowId::new(0)).unwrap();
        let selected = list.open_challenge_row(CategoryRowId::new(0)).unwrap();
        list.back_to_challenge_list().unwrap();
        list.refresh_catalog(reordered_data(false, false)).unwrap();
        assert_eq!(
            list.focused_entry(),
            Some(ArcadeMenuFocusedEntry::Challenge(selected.challenge))
        );
        assert_eq!(list.focused_challenge_row(), Some(CategoryRowId::new(1)));
        assert_eq!(
            list.page(),
            ArcadeMenuPage::ChallengeList(ArcadeCategoryId::new(12))
        );
        assert_eq!(
            list.challenge_menu().unwrap().selected_row(),
            Some(CategoryRowId::new(1))
        );
    }

    #[test]
    fn fabricated_catalog_refresh_prunes_focus_for_omitted_challenges() {
        let mut session = rust_session();
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        assert!(session.focus_challenge_row(CategoryRowId::new(1)));
        assert_eq!(session.focused_challenge(), Some(ChallengeId::new(101)));

        session.back_to_category_chooser().unwrap();
        session
            .open_category_row(ArcadeCategoryRowId::new(1))
            .unwrap();
        session
            .refresh_catalog(reordered_data(false, true))
            .unwrap();
        assert_eq!(
            session.page(),
            ArcadeMenuPage::ChallengeList(ArcadeCategoryId::new(13))
        );

        session.back_to_category_chooser().unwrap();
        session.open_category(ArcadeCategoryId::new(12)).unwrap();
        assert_eq!(session.focused_challenge(), None);
        assert_eq!(session.focused_challenge_row(), None);
    }

    #[test]
    fn fabricated_mission_focus_survives_category_round_trip_and_reorder() {
        let mut session = rust_session();
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        assert!(session.focus_challenge_row(CategoryRowId::new(1)));
        session.back_to_category_chooser().unwrap();

        session
            .refresh_catalog(reordered_data(false, false))
            .unwrap();
        assert_eq!(
            session.category_menu().selected_row(),
            Some(ArcadeCategoryRowId::new(1))
        );
        session
            .open_category_row(ArcadeCategoryRowId::new(1))
            .unwrap();

        // Mission 101 stays focused by identity although the provider moved it from row 1 to 0.
        assert_eq!(session.focused_challenge_row(), Some(CategoryRowId::new(0)));
        assert_eq!(
            session.focused_entry(),
            Some(ArcadeMenuFocusedEntry::Category(ArcadeCategoryId::new(12)))
        );
    }

    #[test]
    fn fabricated_mission_focus_identity_is_independent_per_category() {
        let mut session = rust_session();
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        assert!(session.focus_challenge_row(CategoryRowId::new(1)));
        assert_eq!(session.focused_challenge(), Some(ChallengeId::new(101)));
        session.back_to_category_chooser().unwrap();

        session
            .open_category_row(ArcadeCategoryRowId::new(1))
            .unwrap();
        assert!(session.focus_challenge_row(CategoryRowId::new(0)));
        assert_eq!(session.focused_challenge(), Some(ChallengeId::new(102)));
        session.back_to_category_chooser().unwrap();

        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        assert_eq!(session.focused_challenge(), Some(ChallengeId::new(101)));
        assert_eq!(session.focused_challenge_row(), Some(CategoryRowId::new(1)));
        session.back_to_category_chooser().unwrap();

        session
            .open_category_row(ArcadeCategoryRowId::new(1))
            .unwrap();
        assert_eq!(session.focused_challenge(), Some(ChallengeId::new(102)));
        assert_eq!(session.focused_challenge_row(), Some(CategoryRowId::new(0)));
    }

    #[test]
    fn fabricated_catalog_refresh_rejects_unavailable_detail_transactionally() {
        let mut session = rust_session();
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        session.open_challenge_row(CategoryRowId::new(0)).unwrap();
        let before = session.clone();

        assert_eq!(
            session.refresh_catalog(reordered_data(true, false)),
            Err(ArcadeMenuRefreshError::ChallengeSelection(
                SelectionError::Disabled {
                    help: Some(MenuHelpId::new(901)),
                }
            ))
        );
        assert_eq!(session, before);
    }

    #[test]
    fn fabricated_rejected_category_or_disabled_mission_keeps_route_state() {
        let mut session = rust_session();
        assert_eq!(
            session.open_category_row(ArcadeCategoryRowId::new(99)),
            Err(ArcadeMenuSessionError::CategorySelection(
                super::ArcadeCategorySelectionError::UnknownRow(ArcadeCategoryRowId::new(99))
            ))
        );
        assert_eq!(session.page(), ArcadeMenuPage::CategoryChooser);
        assert_eq!(session.category_menu().selected_row(), None);

        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        let before = session.clone();
        assert_eq!(
            session.open_challenge_row(CategoryRowId::new(1)),
            Err(ArcadeMenuSessionError::ChallengeSelection(
                super::SelectionError::Disabled {
                    help: Some(MenuHelpId::new(900))
                }
            ))
        );
        assert_eq!(session.page(), before.page());
        assert_eq!(
            session.challenge_menu().unwrap().selected_row(),
            before.challenge_menu().unwrap().selected_row()
        );
        assert_eq!(session.selected_challenge(), None);
    }

    #[test]
    fn fabricated_back_route_clears_stale_mission_state_and_keeps_category_focus() {
        let mut session = rust_session();
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        session.open_challenge_row(CategoryRowId::new(0)).unwrap();
        session.back_to_challenge_list().unwrap();
        assert_eq!(
            session.page(),
            ArcadeMenuPage::ChallengeList(ArcadeCategoryId::new(12))
        );
        assert_eq!(
            session.challenge_menu().unwrap().selected_row(),
            Some(CategoryRowId::new(0))
        );

        session.back_to_category_chooser().unwrap();
        assert_eq!(session.page(), ArcadeMenuPage::CategoryChooser);
        assert_eq!(session.challenge_menu(), None);
        assert_eq!(
            session.category_menu().selected_row(),
            Some(ArcadeCategoryRowId::new(0))
        );
        session
            .open_category_row(ArcadeCategoryRowId::new(1))
            .unwrap();
        assert_eq!(
            session.page(),
            ArcadeMenuPage::ChallengeList(ArcadeCategoryId::new(13))
        );
        assert_eq!(session.selected_challenge(), None);
        assert_eq!(session.challenge_menu().unwrap().selected_row(), None);
    }

    #[test]
    fn fabricated_wrong_page_transition_reports_current_typed_page() {
        let mut session = rust_session();
        assert_eq!(
            session.back_to_category_chooser(),
            Err(ArcadeMenuSessionError::WrongPage {
                operation: ArcadeMenuOperation::BackToCategoryChooser,
                current: ArcadeMenuPage::CategoryChooser,
            })
        );
        assert_eq!(session.page(), ArcadeMenuPage::CategoryChooser);
    }

    #[test]
    fn fabricated_duplicate_catalog_ids_are_rejected_before_session_creation() {
        let error = OwnedArcadeChallengeData::new(
            vec![
                ArcadeCategorySnapshot::new(ArcadeCategoryId::new(12), vec![]),
                ArcadeCategorySnapshot::new(ArcadeCategoryId::new(12), vec![]),
            ],
            vec![],
        )
        .unwrap_err();
        assert_eq!(
            error,
            OwnedChallengeDataError::DuplicateCategory(ArcadeCategoryId::new(12))
        );
    }
}
