//! Rust-owned Arcade/Challenge category rows and selection state.
//!
//! This is the data-side attachment described by `docs/t1701-rea-arcade.md`. Callers
//! provide owned snapshots from the category and mission data services; the model keeps
//! category row identity separate from mission identity and never retains guest pointers.
//! Runtime page construction and mission/rule publication remain separate attachments.

/// Category identity (`0x78ad38`), distinct from both a category row and a mission ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArcadeCategoryId(u32);

impl ArcadeCategoryId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Position in the original category array. Hidden or invalid records still consume a row ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CategoryRowId(u32);

impl CategoryRowId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Position in the Arcade category chooser. Distinct from a mission row within a category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ArcadeCategoryRowId(u32);

impl ArcadeCategoryRowId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Mission identity from the category table, not a category row or runtime rule ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChallengeId(u32);

impl ChallengeId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Owned identity for the detail page descriptor (the original descriptor is `0x523870`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChallengeDescriptor(u32);

impl ChallengeDescriptor {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Runtime rule identity, deliberately distinct from the front-end `ChallengeId`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuleConfigurationId(u32);

impl RuleConfigurationId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Help token attached by the category availability service when a visible row is disabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuHelpId(u32);

impl MenuHelpId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Whether an otherwise visible category entry is available to open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryAvailability {
    Available,
    Disabled { help: Option<MenuHelpId> },
}

/// The source's row helper succeeded and supplied the fields used by the menu builder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CategoryRowData {
    /// The original record byte at `+0x18` has bit `0x20` set.
    pub hidden: bool,
    /// Availability is independent of row omission: unavailable rows remain visible.
    pub availability: EntryAvailability,
}

/// Owned result of looking up one mission in the category's mission table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissionRecordSnapshot {
    /// Whether the mission selector (`0x22e050`) returned a record.
    pub descriptor_present: bool,
    /// `None` means the row-data helper (`0x22df10`) failed.
    pub row_data: Option<CategoryRowData>,
}

/// Provides copied, typed data to build the menu. Implementations own the data lookup policy.
pub trait ArcadeChallengeCatalog {
    /// `None` represents a missing mission record; returned values must not borrow guest memory.
    fn mission_record(&self, challenge: ChallengeId) -> Option<MissionRecordSnapshot>;
}

/// Owned copy of one category's ordered mission table. Row IDs are derived from this order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcadeCategorySnapshot {
    category: ArcadeCategoryId,
    challenges: Vec<ChallengeId>,
}

impl ArcadeCategorySnapshot {
    pub fn new(category: ArcadeCategoryId, challenges: Vec<ChallengeId>) -> Self {
        Self {
            category,
            challenges,
        }
    }

    pub const fn category(&self) -> ArcadeCategoryId {
        self.category
    }

    pub fn challenges(&self) -> &[ChallengeId] {
        &self.challenges
    }
}

/// Owned record data keyed by the mission identity used in a category table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChallengeRecordSnapshot {
    challenge: ChallengeId,
    record: MissionRecordSnapshot,
}

impl ChallengeRecordSnapshot {
    pub const fn new(challenge: ChallengeId, record: MissionRecordSnapshot) -> Self {
        Self { challenge, record }
    }

    pub const fn challenge(self) -> ChallengeId {
        self.challenge
    }

    pub const fn record(self) -> MissionRecordSnapshot {
        self.record
    }
}

/// Rejected duplicate IDs at the owned data boundary instead of choosing one silently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnedChallengeDataError {
    DuplicateCategory(ArcadeCategoryId),
    DuplicateMission(ChallengeId),
}

/// Owned category and mission snapshots suitable for the Rust menu model.
///
/// Guest table adapters must copy their records into this type before passing them to UI code.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OwnedArcadeChallengeData {
    categories: Vec<ArcadeCategorySnapshot>,
    missions: Vec<ChallengeRecordSnapshot>,
}

impl OwnedArcadeChallengeData {
    pub fn new(
        categories: Vec<ArcadeCategorySnapshot>,
        missions: Vec<ChallengeRecordSnapshot>,
    ) -> Result<Self, OwnedChallengeDataError> {
        for (index, category) in categories.iter().enumerate() {
            if categories[..index]
                .iter()
                .any(|prior| prior.category == category.category)
            {
                return Err(OwnedChallengeDataError::DuplicateCategory(
                    category.category,
                ));
            }
        }
        for (index, mission) in missions.iter().enumerate() {
            if missions[..index]
                .iter()
                .any(|prior| prior.challenge == mission.challenge)
            {
                return Err(OwnedChallengeDataError::DuplicateMission(mission.challenge));
            }
        }
        Ok(Self {
            categories,
            missions,
        })
    }

    /// Copies a provider's ordered category rows and referenced mission records into owned data.
    ///
    /// Each referenced mission is read at most once, in first-seen category order. Missing
    /// mission records remain absent so the page projection can apply its existing omission
    /// rule. The provider must present a stable view for the duration of this call.
    pub fn copy_from_provider(
        provider: &impl ArcadeChallengeDataProvider,
    ) -> Result<Self, OwnedChallengeDataProviderError> {
        let mut categories = Vec::new();
        let mut visited = Vec::new();
        let mut missions = Vec::new();

        for category in provider.category_snapshots() {
            let category_id = category.category();
            let challenges = provider.category_challenges(category_id).ok_or(
                OwnedChallengeDataProviderError::MissingCategory(category_id),
            )?;
            categories.push(ArcadeCategorySnapshot::new(
                category_id,
                challenges.to_vec(),
            ));

            for &challenge in challenges {
                if visited.contains(&challenge) {
                    continue;
                }
                visited.push(challenge);
                if let Some(record) = provider.mission_record(challenge) {
                    missions.push(ChallengeRecordSnapshot::new(challenge, record));
                }
            }
        }

        Self::new(categories, missions).map_err(OwnedChallengeDataProviderError::InvalidData)
    }

    pub fn category(&self, category: ArcadeCategoryId) -> Option<&ArcadeCategorySnapshot> {
        self.categories
            .iter()
            .find(|snapshot| snapshot.category == category)
    }
}

/// Supplies owned category order and copied mission records to the menu projection.
pub trait ArcadeChallengeDataProvider: ArcadeChallengeCatalog {
    /// Owned category snapshots in source-table order, with no guest-memory references.
    fn category_snapshots(&self) -> &[ArcadeCategorySnapshot];

    /// The slice must be owned by the provider and contain no guest-memory references.
    fn category_challenges(&self, category: ArcadeCategoryId) -> Option<&[ChallengeId]>;
}

impl ArcadeChallengeCatalog for OwnedArcadeChallengeData {
    fn mission_record(&self, challenge: ChallengeId) -> Option<MissionRecordSnapshot> {
        self.missions
            .iter()
            .find(|snapshot| snapshot.challenge == challenge)
            .map(|snapshot| snapshot.record)
    }
}

impl ArcadeChallengeDataProvider for OwnedArcadeChallengeData {
    fn category_snapshots(&self) -> &[ArcadeCategorySnapshot] {
        &self.categories
    }

    fn category_challenges(&self, category: ArcadeCategoryId) -> Option<&[ChallengeId]> {
        self.category(category)
            .map(ArcadeCategorySnapshot::challenges)
    }
}

/// Why copying a provider into a self-contained Arcade catalog was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnedChallengeDataProviderError {
    MissingCategory(ArcadeCategoryId),
    InvalidData(OwnedChallengeDataError),
}

/// Failure to resolve the requested category from the owned data provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CategoryBuildError {
    MissingCategory(ArcadeCategoryId),
}

/// Backend switch for this slice. The original page remains selected unless a caller opts in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArcadeChallengeBackend {
    #[default]
    Legacy,
    Rust,
}

/// One visible entry. Its row ID preserves the position in the source category table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcadeChallengeEntry {
    pub row: CategoryRowId,
    pub challenge: ChallengeId,
    pub detail: ChallengeDescriptor,
    pub availability: EntryAvailability,
}

/// Result returned to the detail-page attachment after selecting an available row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedChallenge {
    pub category: ArcadeCategoryId,
    pub row: CategoryRowId,
    pub challenge: ChallengeId,
    pub detail: ChallengeDescriptor,
}

/// One category chooser entry. Its row retains the owned source-table position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcadeCategoryEntry {
    pub row: ArcadeCategoryRowId,
    pub category: ArcadeCategoryId,
}

/// Category chosen from the Arcade chooser, before any mission row is selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedArcadeCategory {
    pub row: ArcadeCategoryRowId,
    pub category: ArcadeCategoryId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeCategorySelectionError {
    LegacyBackend,
    UnknownRow(ArcadeCategoryRowId),
}

/// Rust-owned category chooser data and selection state. Legacy remains the default backend.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArcadeCategoryMenu {
    backend: ArcadeChallengeBackend,
    entries: Vec<ArcadeCategoryEntry>,
    selected_row: Option<ArcadeCategoryRowId>,
}

impl ArcadeCategoryMenu {
    pub fn build(provider: &impl ArcadeChallengeDataProvider) -> Self {
        Self::build_with_backend(provider, ArcadeChallengeBackend::default())
    }

    pub fn build_with_backend(
        provider: &impl ArcadeChallengeDataProvider,
        backend: ArcadeChallengeBackend,
    ) -> Self {
        let entries = provider
            .category_snapshots()
            .iter()
            .enumerate()
            .map(|(index, category)| {
                let row = u32::try_from(index)
                    .expect("Arcade category row index exceeds its typed identity range");
                ArcadeCategoryEntry {
                    row: ArcadeCategoryRowId::new(row),
                    category: category.category(),
                }
            })
            .collect();
        Self {
            backend,
            entries,
            selected_row: None,
        }
    }

    pub const fn backend(&self) -> ArcadeChallengeBackend {
        self.backend
    }

    pub fn entries(&self) -> &[ArcadeCategoryEntry] {
        &self.entries
    }

    pub const fn selected_row(&self) -> Option<ArcadeCategoryRowId> {
        self.selected_row
    }

    pub fn select(
        &mut self,
        row: ArcadeCategoryRowId,
    ) -> Result<SelectedArcadeCategory, ArcadeCategorySelectionError> {
        if self.backend == ArcadeChallengeBackend::Legacy {
            return Err(ArcadeCategorySelectionError::LegacyBackend);
        }
        let Some(entry) = self.entries.iter().find(|entry| entry.row == row) else {
            return Err(ArcadeCategorySelectionError::UnknownRow(row));
        };
        let selected = SelectedArcadeCategory {
            row: entry.row,
            category: entry.category,
        };
        self.selected_row = Some(row);
        Ok(selected)
    }
}

/// Why a row could not be opened by the opt-in Rust model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionError {
    LegacyBackend,
    UnknownRow(CategoryRowId),
    Disabled { help: Option<MenuHelpId> },
}

/// Frontend mode value used to decide whether the original preview replacement path applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuModeId(i32);

impl MenuModeId {
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i32 {
        self.0
    }
}

/// Menu participant count, kept distinct from the runtime participant count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuParticipantCount(u32);

impl MenuParticipantCount {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// User-selected setup values owned by the Rust Challenge detail page.
///
/// The original menu's allowed values and defaults are not established by the REA checkpoint,
/// so construction requires an explicit selection from the page/input adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChallengeDetailChoices {
    mode: MenuModeId,
    participant_count: MenuParticipantCount,
}

impl ChallengeDetailChoices {
    pub const fn new(mode: MenuModeId, participant_count: MenuParticipantCount) -> Self {
        Self {
            mode,
            participant_count,
        }
    }

    pub const fn mode(self) -> MenuModeId {
        self.mode
    }

    pub const fn participant_count(self) -> MenuParticipantCount {
        self.participant_count
    }
}

/// Descriptor opened by the Challenge setup flow (original descriptor `0x5237f0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChallengeSetupPage(u32);

impl ChallengeSetupPage {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One of the four hidden setup slots. The service receives this as the original EAX value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HiddenSetupSlot(u8);

impl HiddenSetupSlot {
    const fn new(value: u8) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

pub const PREVIEW_SLOT_COUNT: u8 = 4;
pub const PREVIEW_SLOT_RECORD_STRIDE_BYTES: u32 = 0x54;
pub const PARTICIPANT_RECORD_STRIDE_BYTES: u32 = 0x16c;

/// Original preview model used by the special mode-7 Challenge path (`0x385`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PreviewModelId(u32);

impl PreviewModelId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A participant's typed slot placement derived from the measured menu count rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParticipantPlacement {
    index: u32,
    slot_width: i64,
    slot_offset: i64,
}

impl ParticipantPlacement {
    pub const fn index(self) -> u32 {
        self.index
    }

    pub const fn slot_width(self) -> i64 {
        self.slot_width
    }

    pub const fn slot_offset(self) -> i64 {
        self.slot_offset
    }
}

/// Owned participant layout. It models the known placement fields, not the unmeasured full record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParticipantSetup {
    count: MenuParticipantCount,
    slot_width: i64,
}

impl ParticipantSetup {
    fn from_menu_count(count: MenuParticipantCount) -> Option<Self> {
        if count.get() == 0 {
            return None;
        }

        // The original special-cases three participants to divide by four.
        let divisor = if count.get() == 3 {
            4
        } else {
            i64::from(count.get())
        };
        Some(Self {
            count,
            slot_width: 4 / divisor,
        })
    }

    pub const fn count(self) -> MenuParticipantCount {
        self.count
    }

    pub const fn slot_width(self) -> i64 {
        self.slot_width
    }

    pub const fn record_stride_bytes(self) -> u32 {
        PARTICIPANT_RECORD_STRIDE_BYTES
    }

    /// Lazily yields placement values without imposing a count limit or allocating a second list.
    pub fn placements(self) -> impl Iterator<Item = ParticipantPlacement> {
        (0..self.count.get()).map(move |index| ParticipantPlacement {
            index,
            slot_width: self.slot_width,
            slot_offset: self.slot_width * i64::from(index),
        })
    }
}

/// Special preview replacement can fail at native resource boundaries; callers must not treat
/// failure as a successful model replacement. These cases are inferred service errors, not REA
/// measurements of original failure behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewReplacementFailure {
    ExistingObjectReleaseFailed,
    ModelUnavailable(PreviewModelId),
    ExtensionUnavailable,
}

/// A semantic operation in the original detail-accept/setup sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengeSetupOperation {
    PrepareDetailAcceptance,
    SetSetupInitializationActive(bool),
    ReleaseExistingSetup,
    InitializeHiddenSlotInAccumulator(HiddenSetupSlot),
    RebuildPreviewSlots,
    ReplaceSpecialPreview {
        model: PreviewModelId,
        extension_flags: u32,
    },
    RestoreMenuActivation,
    OpenSetupPage(ChallengeSetupPage),
    InitializeParticipants(ParticipantSetup),
    ClearPostSetupSelection,
}

/// Errors reported by a Rust-owned setup service. The open descriptor comes from the provider;
/// model and participant failures are explicit so adapters cannot silently report success.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengeSetupFailure {
    SetupPageUnavailable(ChallengeSetupPage),
    PreviewReplacement(PreviewReplacementFailure),
    ParticipantStorageUnavailable,
    /// The saved confirmed prefix came from a different setup plan and cannot be resumed safely.
    AttemptDoesNotMatchPlan,
    ServiceRejected,
}

/// Native services that apply an already ordered setup plan to Rust-owned state.
/// `Ok(())` confirms that this operation committed; `Err` confirms that the failing operation
/// made no change. Earlier successful operations remain committed and are reported by
/// `ChallengeSetupAttempt`; this interface does not promise whole-plan rollback.
pub trait ChallengeSetupServices {
    fn apply(&mut self, operation: ChallengeSetupOperation) -> Result<(), ChallengeSetupFailure>;
}

/// Ordered, owned setup transition prepared from a selected menu entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengeSetupPlan {
    selection: SelectedChallenge,
    participant_setup: Option<ParticipantSetup>,
    operations: Vec<ChallengeSetupOperation>,
}

impl ChallengeSetupPlan {
    fn from_selection(
        selection: SelectedChallenge,
        mode: MenuModeId,
        participant_count: MenuParticipantCount,
    ) -> Self {
        let participant_setup = ParticipantSetup::from_menu_count(participant_count);
        let mut operations = vec![
            ChallengeSetupOperation::PrepareDetailAcceptance,
            ChallengeSetupOperation::SetSetupInitializationActive(false),
            ChallengeSetupOperation::ReleaseExistingSetup,
        ];
        operations.extend((0..PREVIEW_SLOT_COUNT).map(|slot| {
            ChallengeSetupOperation::InitializeHiddenSlotInAccumulator(HiddenSetupSlot::new(slot))
        }));
        operations.extend([
            ChallengeSetupOperation::SetSetupInitializationActive(true),
            ChallengeSetupOperation::RebuildPreviewSlots,
        ]);
        if mode.get() == 7 && matches!(selection.challenge.get(), 6..=8) {
            operations.push(ChallengeSetupOperation::ReplaceSpecialPreview {
                model: PreviewModelId::new(0x385),
                extension_flags: 1 << 2,
            });
        }
        operations.extend([
            ChallengeSetupOperation::RestoreMenuActivation,
            ChallengeSetupOperation::OpenSetupPage(ChallengeSetupPage::new(0x5237f0)),
        ]);
        if let Some(participants) = participant_setup {
            operations.push(ChallengeSetupOperation::InitializeParticipants(
                participants,
            ));
        }
        operations.extend([
            ChallengeSetupOperation::ClearPostSetupSelection,
            ChallengeSetupOperation::OpenSetupPage(ChallengeSetupPage::new(0x5237f0)),
        ]);

        Self {
            selection,
            participant_setup,
            operations,
        }
    }

    /// Prepares setup from the already-owned detail selection. This does not resolve a row again,
    /// so a category refresh cannot reinterpret the selected row ordinal as another mission.
    pub fn from_selected(
        selection: SelectedChallenge,
        mode: MenuModeId,
        participant_count: MenuParticipantCount,
    ) -> Self {
        Self::from_selection(selection, mode, participant_count)
    }

    pub const fn selection(&self) -> SelectedChallenge {
        self.selection
    }

    pub const fn participant_setup(&self) -> Option<ParticipantSetup> {
        self.participant_setup
    }

    pub fn operations(&self) -> &[ChallengeSetupOperation] {
        &self.operations
    }

    /// Builds the owned Rust projection represented by this plan without calling host services.
    /// Resource requests remain requests; this does not claim that a model was installed.
    pub fn materialize_state(&self) -> ChallengeSetupState {
        let mut state = ChallengeSetupState::new(self.selection);
        for &operation in &self.operations {
            state.apply(operation);
        }
        state
    }

    /// Applies the operations in recorded order and returns the first explicit service failure.
    pub fn apply(
        &self,
        services: &mut impl ChallengeSetupServices,
    ) -> Result<(), ChallengeSetupFailure> {
        for &operation in &self.operations {
            services.apply(operation)?;
        }
        Ok(())
    }

    /// Applies operations in order and retains the confirmed Rust-owned state if a later
    /// operation fails. A service error means that operation was not committed; earlier
    /// acknowledged operations remain visible in the returned partial attempt.
    pub fn apply_tracked<S: ChallengeSetupServices + ?Sized>(
        &self,
        services: &mut S,
    ) -> ChallengeSetupAttempt {
        self.resume_tracked(
            services,
            ChallengeSetupAttempt {
                plan: self.clone(),
                state: ChallengeSetupState::new(self.selection),
                applied_operations: 0,
                failure: None,
            },
        )
    }

    /// Continues after the confirmed prefix without reapplying operations already committed by
    /// the native service. The saved attempt is accepted only for this exact plan.
    pub fn resume_tracked<S: ChallengeSetupServices + ?Sized>(
        &self,
        services: &mut S,
        mut attempt: ChallengeSetupAttempt,
    ) -> ChallengeSetupAttempt {
        if attempt.plan != *self || attempt.applied_operations > self.operations.len() {
            attempt.failure = Some(ChallengeSetupFailure::AttemptDoesNotMatchPlan);
            return attempt;
        }

        attempt.failure = None;
        for &operation in &self.operations[attempt.applied_operations..] {
            if let Err(failure) = services.apply(operation) {
                attempt.failure = Some(failure);
                return attempt;
            }
            attempt.state.apply(operation);
            attempt.applied_operations += 1;
        }
        attempt
    }
}

/// Confirmed progress from applying a detail-accept plan to native Rust services.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengeSetupAttempt {
    plan: ChallengeSetupPlan,
    state: ChallengeSetupState,
    applied_operations: usize,
    failure: Option<ChallengeSetupFailure>,
}

impl ChallengeSetupAttempt {
    pub const fn state(&self) -> &ChallengeSetupState {
        &self.state
    }

    /// Returns whether this confirmed prefix can resume the supplied plan without replaying
    /// already committed service operations.
    pub fn matches_plan(&self, plan: &ChallengeSetupPlan) -> bool {
        self.plan == *plan && self.applied_operations <= plan.operations.len()
    }

    pub const fn applied_operations(&self) -> usize {
        self.applied_operations
    }

    pub const fn failure(&self) -> Option<ChallengeSetupFailure> {
        self.failure
    }

    pub fn is_complete(&self) -> bool {
        self.failure.is_none() && self.applied_operations == self.plan.operations.len()
    }
}

/// A typed request made by the special Challenge preview branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpecialPreviewRequest {
    model: PreviewModelId,
    extension_flags: u32,
}

impl SpecialPreviewRequest {
    pub const fn model(self) -> PreviewModelId {
        self.model
    }

    pub const fn extension_flags(self) -> u32 {
        self.extension_flags
    }
}

/// Rust-owned view of the Challenge setup fields established by the measured operation plan.
/// Preview and participant records intentionally expose only the fields known by the REA source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengeSetupState {
    selection: SelectedChallenge,
    detail_acceptance_prepared: bool,
    setup_initialization_active: bool,
    existing_setup_released: bool,
    initialized_hidden_slots: [bool; PREVIEW_SLOT_COUNT as usize],
    preview_slots_rebuilt: bool,
    special_preview_request: Option<SpecialPreviewRequest>,
    menu_activation_restored: bool,
    opened_setup_pages: Vec<ChallengeSetupPage>,
    participant_placements: Vec<ParticipantPlacement>,
    post_setup_selection_cleared: bool,
}

impl ChallengeSetupState {
    fn new(selection: SelectedChallenge) -> Self {
        Self {
            selection,
            detail_acceptance_prepared: false,
            setup_initialization_active: false,
            existing_setup_released: false,
            initialized_hidden_slots: [false; PREVIEW_SLOT_COUNT as usize],
            preview_slots_rebuilt: false,
            special_preview_request: None,
            menu_activation_restored: false,
            opened_setup_pages: Vec::new(),
            participant_placements: Vec::new(),
            post_setup_selection_cleared: false,
        }
    }

    fn apply(&mut self, operation: ChallengeSetupOperation) {
        match operation {
            ChallengeSetupOperation::PrepareDetailAcceptance => {
                self.detail_acceptance_prepared = true;
            }
            ChallengeSetupOperation::SetSetupInitializationActive(active) => {
                self.setup_initialization_active = active;
            }
            ChallengeSetupOperation::ReleaseExistingSetup => {
                self.existing_setup_released = true;
                self.initialized_hidden_slots = [false; PREVIEW_SLOT_COUNT as usize];
                self.preview_slots_rebuilt = false;
                self.special_preview_request = None;
                self.opened_setup_pages.clear();
                self.participant_placements.clear();
            }
            ChallengeSetupOperation::InitializeHiddenSlotInAccumulator(slot) => {
                if let Some(initialized) = self
                    .initialized_hidden_slots
                    .get_mut(usize::from(slot.get()))
                {
                    *initialized = true;
                }
            }
            ChallengeSetupOperation::RebuildPreviewSlots => {
                self.preview_slots_rebuilt = true;
            }
            ChallengeSetupOperation::ReplaceSpecialPreview {
                model,
                extension_flags,
            } => {
                self.special_preview_request = Some(SpecialPreviewRequest {
                    model,
                    extension_flags,
                });
            }
            ChallengeSetupOperation::RestoreMenuActivation => {
                self.menu_activation_restored = true;
            }
            ChallengeSetupOperation::OpenSetupPage(page) => self.opened_setup_pages.push(page),
            ChallengeSetupOperation::InitializeParticipants(participants) => {
                self.participant_placements = participants.placements().collect();
            }
            ChallengeSetupOperation::ClearPostSetupSelection => {
                self.post_setup_selection_cleared = true;
            }
        }
    }

    pub const fn selection(&self) -> SelectedChallenge {
        self.selection
    }

    pub const fn detail_acceptance_prepared(&self) -> bool {
        self.detail_acceptance_prepared
    }

    pub const fn setup_initialization_active(&self) -> bool {
        self.setup_initialization_active
    }

    pub const fn existing_setup_released(&self) -> bool {
        self.existing_setup_released
    }

    pub const fn initialized_hidden_slots(&self) -> &[bool; PREVIEW_SLOT_COUNT as usize] {
        &self.initialized_hidden_slots
    }

    pub const fn preview_slots_rebuilt(&self) -> bool {
        self.preview_slots_rebuilt
    }

    pub const fn special_preview_request(&self) -> Option<SpecialPreviewRequest> {
        self.special_preview_request
    }

    pub const fn menu_activation_restored(&self) -> bool {
        self.menu_activation_restored
    }

    pub fn opened_setup_pages(&self) -> &[ChallengeSetupPage] {
        &self.opened_setup_pages
    }

    pub fn participant_placements(&self) -> &[ParticipantPlacement] {
        &self.participant_placements
    }

    pub const fn post_setup_selection_cleared(&self) -> bool {
        self.post_setup_selection_cleared
    }
}

/// Rust-owned category projection and selection state for Arcade/Challenge entry pages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcadeChallengeMenu {
    category: ArcadeCategoryId,
    detail: ChallengeDescriptor,
    backend: ArcadeChallengeBackend,
    entries: Vec<ArcadeChallengeEntry>,
    selected_row: Option<CategoryRowId>,
}

impl ArcadeChallengeMenu {
    /// Builds the original visible-row projection. The switch defaults to the legacy backend.
    pub fn build(
        category: ArcadeCategoryId,
        detail: ChallengeDescriptor,
        challenges: &[ChallengeId],
        catalog: &impl ArcadeChallengeCatalog,
    ) -> Self {
        Self::build_with_backend(
            category,
            detail,
            challenges,
            catalog,
            ArcadeChallengeBackend::default(),
        )
    }

    /// Builds the same projection with an explicit backend opt-in.
    pub fn build_with_backend(
        category: ArcadeCategoryId,
        detail: ChallengeDescriptor,
        challenges: &[ChallengeId],
        catalog: &impl ArcadeChallengeCatalog,
        backend: ArcadeChallengeBackend,
    ) -> Self {
        let mut entries = Vec::new();
        for (index, &challenge) in challenges.iter().enumerate() {
            let Some(row_data) = catalog
                .mission_record(challenge)
                .filter(|record| record.descriptor_present)
                .and_then(|record| record.row_data)
                .filter(|row| !row.hidden)
            else {
                continue;
            };

            let row_id = u32::try_from(index).expect("category row index exceeds original u32 ID");
            entries.push(ArcadeChallengeEntry {
                row: CategoryRowId::new(row_id),
                challenge,
                detail,
                availability: row_data.availability,
            });
        }

        Self {
            category,
            detail,
            backend,
            entries,
            selected_row: None,
        }
    }

    /// Builds from an owned data provider, reporting a missing category before projecting rows.
    pub fn build_from_provider(
        category: ArcadeCategoryId,
        detail: ChallengeDescriptor,
        provider: &impl ArcadeChallengeDataProvider,
        backend: ArcadeChallengeBackend,
    ) -> Result<Self, CategoryBuildError> {
        let challenges = provider
            .category_challenges(category)
            .ok_or(CategoryBuildError::MissingCategory(category))?;
        Ok(Self::build_with_backend(
            category, detail, challenges, provider, backend,
        ))
    }

    pub const fn category(&self) -> ArcadeCategoryId {
        self.category
    }

    pub const fn detail_descriptor(&self) -> ChallengeDescriptor {
        self.detail
    }

    pub const fn backend(&self) -> ArcadeChallengeBackend {
        self.backend
    }

    pub fn entries(&self) -> &[ArcadeChallengeEntry] {
        &self.entries
    }

    pub const fn selected_row(&self) -> Option<CategoryRowId> {
        self.selected_row
    }

    /// Resolves an available category row to owned mission and page identities.
    pub fn select(&mut self, row: CategoryRowId) -> Result<SelectedChallenge, SelectionError> {
        if self.backend != ArcadeChallengeBackend::Rust {
            return Err(SelectionError::LegacyBackend);
        }
        let Some(entry) = self.entries.iter().find(|entry| entry.row == row) else {
            return Err(SelectionError::UnknownRow(row));
        };
        if let EntryAvailability::Disabled { help } = entry.availability {
            return Err(SelectionError::Disabled { help });
        }

        let selection = SelectedChallenge {
            category: self.category,
            row: entry.row,
            challenge: entry.challenge,
            detail: entry.detail,
        };
        self.selected_row = Some(row);
        Ok(selection)
    }

    /// Prepares the detail-accept/setup transition for an available row. The backend remains
    /// guarded by the same explicit Rust opt-in as category selection.
    pub fn accept_detail(
        &mut self,
        row: CategoryRowId,
        mode: MenuModeId,
        participant_count: MenuParticipantCount,
    ) -> Result<ChallengeSetupPlan, SelectionError> {
        let selection = self.select(row)?;
        Ok(ChallengeSetupPlan::from_selection(
            selection,
            mode,
            participant_count,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct FabricatedCatalog(BTreeMap<u32, Option<MissionRecordSnapshot>>);

    impl ArcadeChallengeCatalog for FabricatedCatalog {
        fn mission_record(&self, challenge: ChallengeId) -> Option<MissionRecordSnapshot> {
            self.0.get(&challenge.get()).copied().flatten()
        }
    }

    struct FabricatedDataProvider {
        categories: Vec<ArcadeCategorySnapshot>,
        records: Vec<ChallengeRecordSnapshot>,
        unavailable_category: Option<ArcadeCategoryId>,
    }

    impl ArcadeChallengeCatalog for FabricatedDataProvider {
        fn mission_record(&self, challenge: ChallengeId) -> Option<MissionRecordSnapshot> {
            self.records
                .iter()
                .find(|record| record.challenge() == challenge)
                .map(|record| record.record())
        }
    }

    impl ArcadeChallengeDataProvider for FabricatedDataProvider {
        fn category_snapshots(&self) -> &[ArcadeCategorySnapshot] {
            &self.categories
        }

        fn category_challenges(&self, category: ArcadeCategoryId) -> Option<&[ChallengeId]> {
            if self.unavailable_category == Some(category) {
                return None;
            }
            self.categories
                .iter()
                .find(|snapshot| snapshot.category() == category)
                .map(ArcadeCategorySnapshot::challenges)
        }
    }

    fn visible(availability: EntryAvailability) -> Option<MissionRecordSnapshot> {
        Some(MissionRecordSnapshot {
            descriptor_present: true,
            row_data: Some(CategoryRowData {
                hidden: false,
                availability,
            }),
        })
    }

    #[test]
    fn fabricated_category_menu_preserves_source_order_and_separates_row_identity() {
        let first = ArcadeCategoryId::new(7);
        let second = ArcadeCategoryId::new(2);
        let data = OwnedArcadeChallengeData::new(
            vec![
                ArcadeCategorySnapshot::new(first, vec![ChallengeId::new(50)]),
                ArcadeCategorySnapshot::new(second, vec![ChallengeId::new(60)]),
            ],
            vec![],
        )
        .unwrap();

        let mut legacy = ArcadeCategoryMenu::build(&data);
        assert_eq!(legacy.backend(), ArcadeChallengeBackend::Legacy);
        assert_eq!(
            legacy.select(ArcadeCategoryRowId::new(0)),
            Err(ArcadeCategorySelectionError::LegacyBackend)
        );

        let mut native =
            ArcadeCategoryMenu::build_with_backend(&data, ArcadeChallengeBackend::Rust);
        assert_eq!(
            native
                .entries()
                .iter()
                .map(|entry| (entry.row.get(), entry.category.get()))
                .collect::<Vec<_>>(),
            [(0, 7), (1, 2)]
        );
        assert_eq!(
            native.select(ArcadeCategoryRowId::new(1)),
            Ok(SelectedArcadeCategory {
                row: ArcadeCategoryRowId::new(1),
                category: second,
            })
        );
        assert_eq!(native.selected_row(), Some(ArcadeCategoryRowId::new(1)));
        assert_eq!(
            native.select(ArcadeCategoryRowId::new(2)),
            Err(ArcadeCategorySelectionError::UnknownRow(
                ArcadeCategoryRowId::new(2)
            ))
        );
        assert_eq!(native.selected_row(), Some(ArcadeCategoryRowId::new(1)));
    }

    #[test]
    fn fabricated_owned_provider_projects_category_order_and_missing_records() {
        let category = ArcadeCategoryId::new(4);
        let data = OwnedArcadeChallengeData::new(
            vec![ArcadeCategorySnapshot::new(
                category,
                vec![
                    ChallengeId::new(40),
                    ChallengeId::new(41),
                    ChallengeId::new(42),
                    ChallengeId::new(43),
                ],
            )],
            vec![
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(40),
                    visible(EntryAvailability::Available).unwrap(),
                ),
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(42),
                    MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: true,
                            availability: EntryAvailability::Available,
                        }),
                    },
                ),
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(43),
                    visible(EntryAvailability::Disabled {
                        help: Some(MenuHelpId::new(17)),
                    })
                    .unwrap(),
                ),
            ],
        )
        .unwrap();

        let legacy = ArcadeChallengeMenu::build_from_provider(
            category,
            ChallengeDescriptor::new(0x523870),
            &data,
            ArcadeChallengeBackend::default(),
        )
        .unwrap();
        assert_eq!(legacy.backend(), ArcadeChallengeBackend::Legacy);
        assert_eq!(
            legacy
                .entries()
                .iter()
                .map(|entry| (entry.row.get(), entry.challenge.get()))
                .collect::<Vec<_>>(),
            [(0, 40), (3, 43)]
        );

        let mut rust = ArcadeChallengeMenu::build_from_provider(
            category,
            ChallengeDescriptor::new(0x523870),
            &data,
            ArcadeChallengeBackend::Rust,
        )
        .unwrap();
        assert_eq!(
            rust.select(CategoryRowId::new(3)),
            Err(SelectionError::Disabled {
                help: Some(MenuHelpId::new(17))
            })
        );
        assert_eq!(
            rust.select(CategoryRowId::new(0)).unwrap().challenge,
            ChallengeId::new(40)
        );
    }

    #[test]
    fn fabricated_owned_snapshot_copies_ordered_rows_and_only_referenced_records() {
        let first = ArcadeCategoryId::new(7);
        let second = ArcadeCategoryId::new(2);
        let shared = ChallengeId::new(51);
        let provider = FabricatedDataProvider {
            categories: vec![
                ArcadeCategorySnapshot::new(first, vec![ChallengeId::new(50), shared]),
                ArcadeCategorySnapshot::new(second, vec![shared, ChallengeId::new(60)]),
            ],
            records: vec![
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(50),
                    visible(EntryAvailability::Available).unwrap(),
                ),
                ChallengeRecordSnapshot::new(
                    shared,
                    visible(EntryAvailability::Disabled {
                        help: Some(MenuHelpId::new(8)),
                    })
                    .unwrap(),
                ),
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(99),
                    visible(EntryAvailability::Available).unwrap(),
                ),
            ],
            unavailable_category: None,
        };

        let snapshot = OwnedArcadeChallengeData::copy_from_provider(&provider).unwrap();
        drop(provider);

        assert_eq!(
            snapshot
                .category_snapshots()
                .iter()
                .map(|category| (
                    category.category().get(),
                    category
                        .challenges()
                        .iter()
                        .map(|challenge| challenge.get())
                        .collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>(),
            [(7, vec![50, 51]), (2, vec![51, 60])]
        );
        assert!(snapshot.mission_record(ChallengeId::new(50)).is_some());
        assert!(snapshot.mission_record(shared).is_some());
        assert!(snapshot.mission_record(ChallengeId::new(60)).is_none());
        assert!(snapshot.mission_record(ChallengeId::new(99)).is_none());
    }

    #[test]
    fn fabricated_owned_snapshot_rejects_missing_categories_and_duplicate_category_ids() {
        let category = ArcadeCategoryId::new(3);
        let provider = FabricatedDataProvider {
            categories: vec![ArcadeCategorySnapshot::new(category, vec![])],
            records: vec![],
            unavailable_category: Some(category),
        };
        assert_eq!(
            OwnedArcadeChallengeData::copy_from_provider(&provider),
            Err(OwnedChallengeDataProviderError::MissingCategory(category))
        );

        let duplicate_provider = FabricatedDataProvider {
            categories: vec![
                ArcadeCategorySnapshot::new(category, vec![]),
                ArcadeCategorySnapshot::new(category, vec![]),
            ],
            records: vec![],
            unavailable_category: None,
        };
        assert_eq!(
            OwnedArcadeChallengeData::copy_from_provider(&duplicate_provider),
            Err(OwnedChallengeDataProviderError::InvalidData(
                OwnedChallengeDataError::DuplicateCategory(category)
            ))
        );
    }

    #[test]
    fn fabricated_owned_provider_reports_missing_category_and_duplicate_ids() {
        let category = ArcadeCategoryId::new(1);
        let empty = OwnedArcadeChallengeData::default();
        assert_eq!(
            ArcadeChallengeMenu::build_from_provider(
                category,
                ChallengeDescriptor::new(0x523870),
                &empty,
                ArcadeChallengeBackend::Rust,
            ),
            Err(CategoryBuildError::MissingCategory(category))
        );

        let duplicate_categories = OwnedArcadeChallengeData::new(
            vec![
                ArcadeCategorySnapshot::new(category, vec![]),
                ArcadeCategorySnapshot::new(category, vec![]),
            ],
            vec![],
        );
        assert_eq!(
            duplicate_categories,
            Err(OwnedChallengeDataError::DuplicateCategory(category))
        );

        let challenge = ChallengeId::new(9);
        let record = visible(EntryAvailability::Available).unwrap();
        let duplicate_missions = OwnedArcadeChallengeData::new(
            vec![],
            vec![
                ChallengeRecordSnapshot::new(challenge, record),
                ChallengeRecordSnapshot::new(challenge, record),
            ],
        );
        assert_eq!(
            duplicate_missions,
            Err(OwnedChallengeDataError::DuplicateMission(challenge))
        );
    }

    #[test]
    fn fabricated_rows_keep_original_positions_and_separate_availability() {
        let catalog = FabricatedCatalog(BTreeMap::from([
            (10, visible(EntryAvailability::Available)),
            (11, None), // no mission record
            (
                12,
                Some(MissionRecordSnapshot {
                    descriptor_present: false,
                    row_data: Some(CategoryRowData {
                        hidden: false,
                        availability: EntryAvailability::Available,
                    }),
                }),
            ),
            (
                13,
                Some(MissionRecordSnapshot {
                    descriptor_present: true,
                    row_data: None, // row helper rejected the record
                }),
            ),
            (
                14,
                Some(MissionRecordSnapshot {
                    descriptor_present: true,
                    row_data: Some(CategoryRowData {
                        hidden: true,
                        availability: EntryAvailability::Available,
                    }),
                }),
            ),
            (
                15,
                visible(EntryAvailability::Disabled {
                    help: Some(MenuHelpId::new(71)),
                }),
            ),
        ]));
        let challenges = (10..=15).map(ChallengeId::new).collect::<Vec<_>>();
        let menu = ArcadeChallengeMenu::build(
            ArcadeCategoryId::new(4),
            ChallengeDescriptor::new(0x523870),
            &challenges,
            &catalog,
        );

        assert_eq!(menu.backend(), ArcadeChallengeBackend::Legacy);
        assert_eq!(
            menu.entries(),
            &[
                ArcadeChallengeEntry {
                    row: CategoryRowId::new(0),
                    challenge: ChallengeId::new(10),
                    detail: ChallengeDescriptor::new(0x523870),
                    availability: EntryAvailability::Available,
                },
                ArcadeChallengeEntry {
                    row: CategoryRowId::new(5),
                    challenge: ChallengeId::new(15),
                    detail: ChallengeDescriptor::new(0x523870),
                    availability: EntryAvailability::Disabled {
                        help: Some(MenuHelpId::new(71)),
                    },
                },
            ]
        );
    }

    #[test]
    fn fabricated_sparse_row_projection_preserves_ordinals_for_status_combinations() {
        const ROW_COUNT: usize = 5;
        const STATUS_COUNT: usize = 6;
        let cases = STATUS_COUNT.pow(ROW_COUNT as u32);

        // Exhaust the small combinations of absent records, missing descriptors, rejected row
        // data, hidden rows, disabled rows, and available rows. This checks only the Rust-owned
        // projection contract; the status combinations are authored test data.
        for case in 0..cases {
            let mut digits = case;
            let mut records = BTreeMap::new();
            let mut expected = Vec::new();

            for index in 0..ROW_COUNT {
                let challenge = ChallengeId::new(100 + index as u32);
                let status = digits % STATUS_COUNT;
                digits /= STATUS_COUNT;

                let record = match status {
                    0 => continue,
                    1 => MissionRecordSnapshot {
                        descriptor_present: false,
                        row_data: Some(CategoryRowData {
                            hidden: false,
                            availability: EntryAvailability::Available,
                        }),
                    },
                    2 => MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: None,
                    },
                    3 => MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: true,
                            availability: EntryAvailability::Available,
                        }),
                    },
                    4 => MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: false,
                            availability: EntryAvailability::Disabled {
                                help: Some(MenuHelpId::new(40 + index as u32)),
                            },
                        }),
                    },
                    5 => MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: false,
                            availability: EntryAvailability::Available,
                        }),
                    },
                    _ => unreachable!("base-six status digit is in range"),
                };

                if status == 4 || status == 5 {
                    expected.push(ArcadeChallengeEntry {
                        row: CategoryRowId::new(index as u32),
                        challenge,
                        detail: ChallengeDescriptor::new(0x523870),
                        availability: if status == 4 {
                            EntryAvailability::Disabled {
                                help: Some(MenuHelpId::new(40 + index as u32)),
                            }
                        } else {
                            EntryAvailability::Available
                        },
                    });
                }
                records.insert(challenge.get(), Some(record));
            }

            let challenges = (0..ROW_COUNT)
                .map(|index| ChallengeId::new(100 + index as u32))
                .collect::<Vec<_>>();
            let menu = ArcadeChallengeMenu::build(
                ArcadeCategoryId::new(7),
                ChallengeDescriptor::new(0x523870),
                &challenges,
                &FabricatedCatalog(records),
            );

            assert_eq!(menu.entries(), expected, "status combination {case}");
        }
    }

    #[test]
    fn fabricated_selection_requires_rust_opt_in_and_preserves_typed_ids() {
        let catalog = FabricatedCatalog(BTreeMap::from([(
            22,
            visible(EntryAvailability::Available),
        )]));
        let challenges = [ChallengeId::new(22)];
        let mut legacy = ArcadeChallengeMenu::build(
            ArcadeCategoryId::new(3),
            ChallengeDescriptor::new(0x523870),
            &challenges,
            &catalog,
        );
        assert_eq!(
            legacy.select(CategoryRowId::new(0)),
            Err(SelectionError::LegacyBackend)
        );
        assert_eq!(legacy.selected_row(), None);

        let mut rust = ArcadeChallengeMenu::build_with_backend(
            ArcadeCategoryId::new(3),
            ChallengeDescriptor::new(0x523870),
            &challenges,
            &catalog,
            ArcadeChallengeBackend::Rust,
        );
        assert_eq!(
            rust.select(CategoryRowId::new(0)),
            Ok(SelectedChallenge {
                category: ArcadeCategoryId::new(3),
                row: CategoryRowId::new(0),
                challenge: ChallengeId::new(22),
                detail: ChallengeDescriptor::new(0x523870),
            })
        );
        assert_eq!(rust.selected_row(), Some(CategoryRowId::new(0)));
        assert_eq!(
            rust.select(CategoryRowId::new(1)),
            Err(SelectionError::UnknownRow(CategoryRowId::new(1)))
        );
    }

    #[test]
    fn fabricated_disabled_row_keeps_its_help_attachment_without_opening() {
        let catalog = FabricatedCatalog(BTreeMap::from([(
            31,
            visible(EntryAvailability::Disabled {
                help: Some(MenuHelpId::new(88)),
            }),
        )]));
        let mut menu = ArcadeChallengeMenu::build_with_backend(
            ArcadeCategoryId::new(2),
            ChallengeDescriptor::new(0x523870),
            &[ChallengeId::new(31)],
            &catalog,
            ArcadeChallengeBackend::Rust,
        );
        assert_eq!(
            menu.select(CategoryRowId::new(0)),
            Err(SelectionError::Disabled {
                help: Some(MenuHelpId::new(88)),
            })
        );
        assert_eq!(menu.selected_row(), None);
    }

    #[derive(Default)]
    struct RecordingSetupServices {
        operations: Vec<ChallengeSetupOperation>,
        fail_on: Option<ChallengeSetupOperation>,
        failure: Option<ChallengeSetupFailure>,
    }

    impl ChallengeSetupServices for RecordingSetupServices {
        fn apply(
            &mut self,
            operation: ChallengeSetupOperation,
        ) -> Result<(), ChallengeSetupFailure> {
            self.operations.push(operation);
            if self.fail_on == Some(operation) {
                return Err(self
                    .failure
                    .unwrap_or(ChallengeSetupFailure::ServiceRejected));
            }
            Ok(())
        }
    }

    fn fabricated_rust_menu(challenge: u32) -> ArcadeChallengeMenu {
        let catalog = FabricatedCatalog(BTreeMap::from([(
            challenge,
            visible(EntryAvailability::Available),
        )]));
        ArcadeChallengeMenu::build_with_backend(
            ArcadeCategoryId::new(2),
            ChallengeDescriptor::new(0x523870),
            &[ChallengeId::new(challenge)],
            &catalog,
            ArcadeChallengeBackend::Rust,
        )
    }

    #[test]
    fn fabricated_setup_plan_keeps_reset_and_double_open_order() {
        let mut menu = fabricated_rust_menu(22);
        let plan = menu
            .accept_detail(
                CategoryRowId::new(0),
                MenuModeId::new(4),
                MenuParticipantCount::new(3),
            )
            .unwrap();

        let layout = plan.participant_setup().unwrap();
        assert_eq!(layout.count(), MenuParticipantCount::new(3));
        assert_eq!(layout.slot_width(), 1); // three uses divisor four
        assert_eq!(
            layout
                .placements()
                .map(|p| (p.index(), p.slot_width(), p.slot_offset()))
                .collect::<Vec<_>>(),
            [(0, 1, 0), (1, 1, 1), (2, 1, 2)]
        );
        let opens = ChallengeSetupOperation::OpenSetupPage(ChallengeSetupPage::new(0x5237f0));
        assert_eq!(
            plan.operations(),
            &[
                ChallengeSetupOperation::PrepareDetailAcceptance,
                ChallengeSetupOperation::SetSetupInitializationActive(false),
                ChallengeSetupOperation::ReleaseExistingSetup,
                ChallengeSetupOperation::InitializeHiddenSlotInAccumulator(
                    HiddenSetupSlot::new(0,)
                ),
                ChallengeSetupOperation::InitializeHiddenSlotInAccumulator(
                    HiddenSetupSlot::new(1,)
                ),
                ChallengeSetupOperation::InitializeHiddenSlotInAccumulator(
                    HiddenSetupSlot::new(2,)
                ),
                ChallengeSetupOperation::InitializeHiddenSlotInAccumulator(
                    HiddenSetupSlot::new(3,)
                ),
                ChallengeSetupOperation::SetSetupInitializationActive(true),
                ChallengeSetupOperation::RebuildPreviewSlots,
                ChallengeSetupOperation::RestoreMenuActivation,
                opens,
                ChallengeSetupOperation::InitializeParticipants(layout),
                ChallengeSetupOperation::ClearPostSetupSelection,
                opens,
            ]
        );

        let state = plan.materialize_state();
        assert_eq!(state.selection(), plan.selection());
        assert!(state.detail_acceptance_prepared());
        assert!(state.existing_setup_released());
        assert!(state.setup_initialization_active());
        assert_eq!(state.initialized_hidden_slots(), &[true; 4]);
        assert!(state.preview_slots_rebuilt());
        assert!(state.menu_activation_restored());
        assert_eq!(
            state.opened_setup_pages(),
            &[ChallengeSetupPage::new(0x5237f0); 2]
        );
        assert_eq!(
            state
                .participant_placements()
                .iter()
                .map(|p| (p.index(), p.slot_width(), p.slot_offset()))
                .collect::<Vec<_>>(),
            [(0, 1, 0), (1, 1, 1), (2, 1, 2)]
        );
        assert!(state.post_setup_selection_cleared());

        let mut services = RecordingSetupServices::default();
        plan.apply(&mut services).unwrap();
        assert_eq!(services.operations, plan.operations());
    }

    #[test]
    fn fabricated_special_preview_path_is_conditional_and_failure_is_explicit() {
        let mut menu = fabricated_rust_menu(7);
        let plan = menu
            .accept_detail(
                CategoryRowId::new(0),
                MenuModeId::new(7),
                MenuParticipantCount::new(0),
            )
            .unwrap();
        let special = ChallengeSetupOperation::ReplaceSpecialPreview {
            model: PreviewModelId::new(0x385),
            extension_flags: 1 << 2,
        };
        let opens = ChallengeSetupOperation::OpenSetupPage(ChallengeSetupPage::new(0x5237f0));
        assert_eq!(plan.participant_setup(), None);
        assert_eq!(
            plan.operations()
                .iter()
                .filter(|operation| **operation == opens)
                .count(),
            2
        );
        assert!(plan.operations().contains(&special));
        assert!(
            plan.operations()
                .contains(&ChallengeSetupOperation::ClearPostSetupSelection)
        );
        let state = plan.materialize_state();
        assert_eq!(
            state.special_preview_request(),
            Some(SpecialPreviewRequest {
                model: PreviewModelId::new(0x385),
                extension_flags: 1 << 2,
            })
        );

        let failure = ChallengeSetupFailure::PreviewReplacement(
            PreviewReplacementFailure::ModelUnavailable(PreviewModelId::new(0x385)),
        );
        let mut services = RecordingSetupServices {
            fail_on: Some(special),
            failure: Some(failure),
            ..RecordingSetupServices::default()
        };
        assert_eq!(plan.apply(&mut services), Err(failure));
        assert_eq!(services.operations.last(), Some(&special));
        assert!(
            !services
                .operations
                .contains(&ChallengeSetupOperation::RestoreMenuActivation)
        );
    }

    #[test]
    fn fabricated_setup_retry_resumes_after_confirmed_prefix() {
        let mut menu = fabricated_rust_menu(22);
        let plan = menu
            .accept_detail(
                CategoryRowId::new(0),
                MenuModeId::new(4),
                MenuParticipantCount::new(2),
            )
            .unwrap();
        let mut services = RecordingSetupServices {
            fail_on: Some(plan.operations()[5]),
            failure: Some(ChallengeSetupFailure::ServiceRejected),
            ..RecordingSetupServices::default()
        };

        let partial = plan.apply_tracked(&mut services);
        let confirmed_prefix = partial.applied_operations();
        assert_eq!(confirmed_prefix, 5);
        assert!(!partial.is_complete());
        assert_eq!(
            partial.failure(),
            Some(ChallengeSetupFailure::ServiceRejected)
        );
        // The fixture records calls, including the rejected operation; only the
        // acknowledged count is committed to the Rust-owned projection.
        assert_eq!(
            services.operations,
            plan.operations()[..confirmed_prefix + 1]
        );

        services.fail_on = None;
        let resumed = plan.resume_tracked(&mut services, partial);
        assert!(resumed.is_complete());
        assert_eq!(resumed.applied_operations(), plan.operations().len());
        let mut expected_calls = plan.operations()[..confirmed_prefix + 1].to_vec();
        expected_calls.extend_from_slice(&plan.operations()[confirmed_prefix..]);
        assert_eq!(services.operations, expected_calls);
    }

    #[test]
    fn fabricated_setup_retry_rejects_a_different_plan_without_service_calls() {
        let mut menu = fabricated_rust_menu(6);
        let plan = menu
            .accept_detail(
                CategoryRowId::new(0),
                MenuModeId::new(4),
                MenuParticipantCount::new(2),
            )
            .unwrap();
        let mut services = RecordingSetupServices {
            fail_on: Some(plan.operations()[5]),
            ..RecordingSetupServices::default()
        };
        let partial = plan.apply_tracked(&mut services);
        let calls_before_mismatch = services.operations.len();
        let different_plan = ChallengeSetupPlan::from_selected(
            plan.selection(),
            MenuModeId::new(7),
            MenuParticipantCount::new(2),
        );

        let rejected = different_plan.resume_tracked(&mut services, partial);
        assert!(!rejected.is_complete());
        assert_eq!(
            rejected.failure(),
            Some(ChallengeSetupFailure::AttemptDoesNotMatchPlan)
        );
        assert_eq!(services.operations.len(), calls_before_mismatch);
    }

    #[test]
    fn fabricated_legacy_backend_cannot_prepare_detail_setup() {
        let catalog = FabricatedCatalog(BTreeMap::from([(
            22,
            visible(EntryAvailability::Available),
        )]));
        let mut menu = ArcadeChallengeMenu::build(
            ArcadeCategoryId::new(2),
            ChallengeDescriptor::new(0x523870),
            &[ChallengeId::new(22)],
            &catalog,
        );
        assert_eq!(
            menu.accept_detail(
                CategoryRowId::new(0),
                MenuModeId::new(4),
                MenuParticipantCount::new(1),
            ),
            Err(SelectionError::LegacyBackend)
        );
    }
}
