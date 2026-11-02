//! Rust-owned Map Maker UI entry and mode state (T2408).
//!
//! This is a typed model boundary, not a live page adapter. Page identities are
//! semantic Rust values; numeric row and descriptor values stay at their
//! respective typed table boundaries.

use crate::assault_phase_assignment::{
    AssaultPhaseAssignmentCommand, AssaultPhaseAssignmentState, AssaultPhaseAssignmentUpdate,
};
use crate::assault_start_point_picker::{
    StartPointChoiceId, StartPointPickerGeneration, StartPointPickerState,
};
use crate::description::MapDescriptionSession;

mod text_sessions;
use crate::disabled_modes::DisabledModesPage;
use crate::document_close::{
    ClosePromptCommand, ClosePromptPhase, ClosePromptState, ClosePromptToken, ClosePromptUpdate,
};
use crate::history::HistoryAvailability;
use crate::modes::{EditorKind, ModeConfiguration, PreviewRow, PreviewSelection};
use crate::overlays::{
    HintAnchorTransform, MapMakerOverlayRamp, MapMakerOverlayVisibility, shift_hint_anchor,
};
pub use text_sessions::{
    BriefingTextCommit, BriefingTextOutcome, BriefingTextTarget, BriefingTextView,
    MapDescriptionPageView,
};

/// The Beginner/Advanced editor experience is independent of the playable map
/// mode and the preview-table row. T2402 selects Advanced on entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditorExperience {
    Beginner,
    #[default]
    Advanced,
}

/// Logical page identity for the owned entry, mode, and storage routes.
/// These identities are not guest descriptors or widget IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapMakerPage {
    EditorMenu,
    Editor,
    /// Playable mode category selection. This is separate from T2402's removed
    /// Beginner/Advanced experience prompt.
    ModeSelection,
    ModeSettings,
    Preview,
    StorageSource,
    MapList,
    /// Runtime-backed storage progress/status page (builder 0x32AEE0).
    StorageProgress,
}

/// Semantic choices on the runtime-owned Map Maker storage source page.
/// Numeric catalogue positions stay at the storage adapter boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapStorageSource {
    SampleMaps,
    LocalStorage,
}

/// Storage work displayed by the runtime-backed progress page. This records
/// the operation kind only; serialization and device effects stay in the
/// typed storage host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StorageOperation {
    LoadMap,
    SaveMap,
    DeleteMap,
}

/// Identity for one visible storage operation. Late completions from an older
/// page cannot update the current progress state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StorageProgressId(u64);

impl StorageProgressId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageProgressOutcome {
    Pending,
    Succeeded,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageProgress {
    pub id: StorageProgressId,
    pub operation: StorageOperation,
    pub outcome: StorageProgressOutcome,
}

/// Semantic editor pages reached from a loaded-map editor. These are Rust
/// identities; builders, descriptors and widget addresses stay at the runtime
/// page-table boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EditorSurface {
    #[default]
    MainMenu,
    /// Native map preview reached from the loaded editor's Preview Map row.
    /// Distinct from the mode-setup preview page below `MapMakerPage::Preview`.
    MapPreview,
    /// Palette page re-entered from the measured Change to Light row.
    PaletteGrid,
    /// Save transaction route reached from the loaded editor's Save Map row.
    /// Its runtime steps remain supplied by the typed storage host seam.
    SaveMap,
    MapSettings,
    EditorSettings,
    Controls,
    PaletteProperties,
    WeaponSets,
    BotSets,
    /// Rust-owned Map Settings description text editor. Runtime supplies the
    /// current document text and capacity through the typed description API.
    MapDescription,
    /// Runtime-supplied Briefing page with Rust-owned page identity and
    /// navigation. Its content and editing contract are not established here.
    Briefing,
}

/// All typed child pages opened from the loaded editor. `MainMenu` is the
/// editor root and follows the menu-action catalog instead of this surface
/// catalog; keeping the child list here gives every live adapter one contract.
pub const EDITOR_CHILD_SURFACES: &[EditorSurface] = &[
    EditorSurface::MapSettings,
    EditorSurface::EditorSettings,
    EditorSurface::Controls,
    EditorSurface::MapPreview,
    EditorSurface::PaletteGrid,
    EditorSurface::PaletteProperties,
    EditorSurface::SaveMap,
    EditorSurface::WeaponSets,
    EditorSurface::BotSets,
    EditorSurface::MapDescription,
    EditorSurface::Briefing,
];

/// Generation of the runtime-supplied actions for the current editor page.
/// Replacing a page invalidates pointer targets from its previous layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorSurfaceGeneration(u64);

impl EditorSurfaceGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Semantic target resolved by the page registry after hit testing. It carries
/// no widget ID, descriptor, geometry, or guest pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorSurfaceTarget {
    generation: EditorSurfaceGeneration,
    surface: EditorSurface,
}

impl EditorSurfaceTarget {
    pub const fn new(generation: EditorSurfaceGeneration, surface: EditorSurface) -> Self {
        Self {
            generation,
            surface,
        }
    }

    pub const fn generation(self) -> EditorSurfaceGeneration {
        self.generation
    }

    pub const fn surface(self) -> EditorSurface {
        self.surface
    }
}

/// One of the runtime profile-list pages opened from Map Settings. T1714
/// measures their builders and capacities; actual slot labels remain runtime
/// data because the original label words are initialized BSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorSetListKind {
    Weapon,
    Bot,
}

impl EditorSetListKind {
    pub const fn surface(self) -> EditorSurface {
        match self {
            Self::Weapon => EditorSurface::WeaponSets,
            Self::Bot => EditorSurface::BotSets,
        }
    }

    /// T1714's mode-0x66 builders expose 18 weapon slots and 16 bot options.
    pub const fn capacity(self) -> usize {
        match self {
            Self::Weapon => 18,
            Self::Bot => 16,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorSetEntryId(u64);

impl EditorSetEntryId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorSetListGeneration(u64);

impl EditorSetListGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Generation-qualified identity supplied by the runtime profile-list seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorSetEntryTarget {
    generation: EditorSetListGeneration,
    list: EditorSetListKind,
    entry: EditorSetEntryId,
}

impl EditorSetEntryTarget {
    pub const fn new(
        generation: EditorSetListGeneration,
        list: EditorSetListKind,
        entry: EditorSetEntryId,
    ) -> Self {
        Self {
            generation,
            list,
            entry,
        }
    }

    pub const fn generation(self) -> EditorSetListGeneration {
        self.generation
    }

    pub const fn list(self) -> EditorSetListKind {
        self.list
    }

    pub const fn entry(self) -> EditorSetEntryId {
        self.entry
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorSetEntry {
    pub id: EditorSetEntryId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorSetListError {
    NotOnProfileList,
    TooManyEntries,
    DuplicateIdentity,
}

/// Map Settings rows identified by the measured editor bindings in T1714.
/// Their user-facing meanings are semantic labels only; the runtime supplies
/// each row's live choice table and display text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapSettingField {
    Theme,
    WeaponSet,
    BotSet,
    /// Runtime-provided choice for the measured Map Settings "Disable Modes" row.
    /// Choice identities do not imply a decoded mode bit mask.
    DisableModes,
    Music,
    SpawnSounds,
    Sky,
    IndoorLevel,
    Weather,
}

/// Opaque identity from a runtime-owned setting choice table. It is not an
/// original option ID, guest value, or serialized map value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapSettingChoiceId(u64);

impl MapSettingChoiceId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One typed setting row and the current choice identity supplied by the
/// runtime. Runtime labels and concrete setting values remain at the seam.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapSettingRow {
    pub field: MapSettingField,
    pub choices: Vec<MapSettingChoiceId>,
    pub selected: Option<MapSettingChoiceId>,
}

/// Non-choice actions present in the measured Map Settings page (T1714).
/// Their destination identities are inferred from the measured row catalog;
/// opening a destination requires that its current page be registered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapSettingsAction {
    ViewEditWeaponSets,
    ViewEditBotSets,
    EditMapDescription,
    Briefing,
}

impl MapSettingsAction {
    /// INFERRED mapping from T1714's measured rows into their Rust-owned page
    /// identities. Briefing content and editing remain supplied by its caller.
    pub const fn destination_surface(self) -> Option<EditorSurface> {
        match self {
            Self::ViewEditWeaponSets => Some(EditorSurface::WeaponSets),
            Self::ViewEditBotSets => Some(EditorSurface::BotSets),
            Self::EditMapDescription => Some(EditorSurface::MapDescription),
            Self::Briefing => Some(EditorSurface::Briefing),
        }
    }
}

/// Ordered focusable entry in the runtime-installed Map Settings page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapSettingsEntry {
    Field(MapSettingField),
    Action(MapSettingsAction),
}

/// Stable target for any entry in the current Map Settings page generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapSettingsEntryTarget {
    generation: MapSettingsGeneration,
    entry: MapSettingsEntry,
}

impl MapSettingsEntryTarget {
    pub const fn new(generation: MapSettingsGeneration, entry: MapSettingsEntry) -> Self {
        Self { generation, entry }
    }

    pub const fn generation(self) -> MapSettingsGeneration {
        self.generation
    }

    pub const fn entry(self) -> MapSettingsEntry {
        self.entry
    }
}

/// Generation of a runtime-installed Map Settings catalogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapSettingsGeneration(u64);

impl MapSettingsGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable field target returned by the runtime page hit test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapSettingTarget {
    generation: MapSettingsGeneration,
    field: MapSettingField,
}

/// Rust-owned choice rows on the Editor Settings page. Mode changes use a
/// separate accepted/cancelled transaction (T1688), while Xbox Live presence
/// belongs to the dropped network/live subsystem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorSettingField {
    GridIntensity,
    Music,
    Sounds,
}

/// Opaque identity from the runtime's Editor Settings table. It does not
/// encode an original option ID or a persisted value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorSettingChoiceId(u64);

impl EditorSettingChoiceId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorSettingRow {
    pub field: EditorSettingField,
    pub choices: Vec<EditorSettingChoiceId>,
    pub selected: Option<EditorSettingChoiceId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorSettingsGeneration(u64);

impl EditorSettingsGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorSettingTarget {
    generation: EditorSettingsGeneration,
    field: EditorSettingField,
}

impl EditorSettingTarget {
    pub const fn new(generation: EditorSettingsGeneration, field: EditorSettingField) -> Self {
        Self { generation, field }
    }

    pub const fn generation(self) -> EditorSettingsGeneration {
        self.generation
    }

    pub const fn field(self) -> EditorSettingField {
        self.field
    }
}

/// Generation-qualified editor-kind choice hit returned by the live page
/// registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorKindTarget {
    generation: EditorSettingsGeneration,
    kind: EditorKind,
}

impl EditorKindTarget {
    pub const fn new(generation: EditorSettingsGeneration, kind: EditorKind) -> Self {
        Self { generation, kind }
    }

    pub const fn generation(self) -> EditorSettingsGeneration {
        self.generation
    }

    pub const fn kind(self) -> EditorKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorSettingsCatalogError {
    NotOnEditorSettings,
    DuplicateField,
    DuplicateChoice,
    SelectionMissingFromChoices,
}

/// The two named controls established for the kind-A palette properties page.
/// Their concrete callbacks remain with the Rust-owned palette service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PalettePropertyAction {
    LightRow,
    ChangePalette,
}

/// Original runtime text-table identity for a palette-properties action label.
/// The Rust renderer resolves the ID from the loaded data tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PalettePropertyTextId(u16);

impl PalettePropertyTextId {
    pub const CURRENT_LIGHTS: Self = Self(0x0bc4);
    pub const CHANGE_TO_LIGHT: Self = Self(0x0bc6);

    pub const fn get(self) -> u16 {
        self.0
    }
}

impl PalettePropertyAction {
    /// Label IDs measured in the T1714 editor-page catalogue. Text remains runtime data.
    pub const fn text_id(self) -> PalettePropertyTextId {
        match self {
            Self::LightRow => PalettePropertyTextId::CURRENT_LIGHTS,
            Self::ChangePalette => PalettePropertyTextId::CHANGE_TO_LIGHT,
        }
    }
}

/// Generation of one installed palette-properties action catalogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PalettePropertiesGeneration(u64);

impl PalettePropertiesGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable semantic action returned by the active palette page's hit test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PalettePropertyTarget {
    generation: PalettePropertiesGeneration,
    action: PalettePropertyAction,
}

impl PalettePropertyTarget {
    pub const fn new(
        generation: PalettePropertiesGeneration,
        action: PalettePropertyAction,
    ) -> Self {
        Self { generation, action }
    }

    pub const fn generation(self) -> PalettePropertiesGeneration {
        self.generation
    }

    pub const fn action(self) -> PalettePropertyAction {
        self.action
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PalettePropertiesCatalogError {
    NotOnPaletteProperties,
    DuplicateAction,
}

/// Opaque identity for a palette choice supplied by the active runtime page.
/// The UI does not interpret the original palette record or its storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PaletteGridChoiceId(u64);

impl PaletteGridChoiceId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Generation of one installed palette-grid choice catalogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PaletteGridGeneration(u64);

impl PaletteGridGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Semantic target returned by the active palette-grid hit test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PaletteGridChoiceTarget {
    generation: PaletteGridGeneration,
    choice: PaletteGridChoiceId,
}

impl PaletteGridChoiceTarget {
    pub const fn new(generation: PaletteGridGeneration, choice: PaletteGridChoiceId) -> Self {
        Self { generation, choice }
    }

    pub const fn generation(self) -> PaletteGridGeneration {
        self.generation
    }

    pub const fn choice(self) -> PaletteGridChoiceId {
        self.choice
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteGridCatalogError {
    NotOnPaletteGrid,
    DuplicateChoice,
}

/// Opaque identity for one runtime supplied Map Preview action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorPreviewChoiceId(u64);

impl EditorPreviewChoiceId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Generation of the current loaded-editor Map Preview choice list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorPreviewGeneration(u64);

impl EditorPreviewGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable semantic target returned by the active Map Preview page hit test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorPreviewChoiceTarget {
    generation: EditorPreviewGeneration,
    choice: EditorPreviewChoiceId,
}

impl EditorPreviewChoiceTarget {
    pub const fn new(generation: EditorPreviewGeneration, choice: EditorPreviewChoiceId) -> Self {
        Self { generation, choice }
    }

    pub const fn generation(self) -> EditorPreviewGeneration {
        self.generation
    }

    pub const fn choice(self) -> EditorPreviewChoiceId {
        self.choice
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorPreviewCatalogError {
    NotOnMapPreview,
    DuplicateChoice,
}

/// Phase of the separate Editor Settings mode transaction (T1688).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorSettingsEditPhase {
    Editing,
    AwaitingClearConfirmation,
    Finished,
}

/// Typed effects for the Editor Settings mode transaction.
///
/// The page/input adapter performs these in order. `InitializeMap` must run
/// before `PublishRequestedKind` on the confirmed path; no guest pointer or
/// synthetic input crosses this boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorSettingsEditEffect {
    NotifyEditorKindChanged,
    CloseUnchanged,
    RequestClearMapConfirmation,
    DismissClearMapConfirmation,
    InitializeMap,
    PublishRequestedKind(EditorKind),
    MarkChanged,
    CloseSettings,
}

/// Rust-owned current/requested editor-kind transaction for Editor Settings.
///
/// The notify gate is supplied from the runtime's typed snapshot of flag
/// `0x20`. The special direct-commit kind is the measured requested kind `1`;
/// other changed kinds use the clear-map confirmation route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorSettingsEdit {
    current: EditorKind,
    requested: EditorKind,
    notify_on_change: bool,
    notified: bool,
    phase: EditorSettingsEditPhase,
}

impl EditorSettingsEdit {
    pub const fn new(current: EditorKind, notify_on_change: bool) -> Self {
        Self {
            current,
            requested: current,
            notify_on_change,
            notified: false,
            phase: EditorSettingsEditPhase::Editing,
        }
    }

    pub const fn current(&self) -> EditorKind {
        self.current
    }

    pub const fn requested(&self) -> EditorKind {
        self.requested
    }

    pub const fn phase(&self) -> EditorSettingsEditPhase {
        self.phase
    }

    /// Stage a runtime-selected editor kind without publishing it.
    pub fn request(&mut self, kind: EditorKind) -> bool {
        if self.phase != EditorSettingsEditPhase::Editing {
            return false;
        }
        self.requested = kind;
        true
    }

    /// Apply the source-defined Editor Settings Back decision.
    pub fn leave_settings(&mut self) -> Vec<EditorSettingsEditEffect> {
        if self.phase != EditorSettingsEditPhase::Editing {
            return Vec::new();
        }
        if self.requested == self.current {
            self.phase = EditorSettingsEditPhase::Finished;
            return vec![EditorSettingsEditEffect::CloseUnchanged];
        }

        let mut effects = Vec::with_capacity(4);
        if self.notify_on_change && !self.notified {
            self.notified = true;
            effects.push(EditorSettingsEditEffect::NotifyEditorKindChanged);
        }

        if self.requested == EditorKind(1) {
            self.current = self.requested;
            self.phase = EditorSettingsEditPhase::Finished;
            effects.extend([
                EditorSettingsEditEffect::PublishRequestedKind(self.requested),
                EditorSettingsEditEffect::MarkChanged,
                EditorSettingsEditEffect::CloseSettings,
            ]);
        } else {
            self.phase = EditorSettingsEditPhase::AwaitingClearConfirmation;
            effects.push(EditorSettingsEditEffect::RequestClearMapConfirmation);
        }
        effects
    }

    /// Accept after the native clear-map service has completed. The returned
    /// order keeps initialization ahead of publishing the new editor kind.
    pub fn confirm_clear_map(&mut self) -> Vec<EditorSettingsEditEffect> {
        if self.phase != EditorSettingsEditPhase::AwaitingClearConfirmation {
            return Vec::new();
        }
        self.current = self.requested;
        self.phase = EditorSettingsEditPhase::Finished;
        vec![
            EditorSettingsEditEffect::InitializeMap,
            EditorSettingsEditEffect::PublishRequestedKind(self.requested),
            EditorSettingsEditEffect::CloseSettings,
        ]
    }

    /// Dismiss the confirmation and leave the requested value staged.
    ///
    /// Retaining a staged value on cancellation is an INFERRED native policy;
    /// the REA source record establishes the accepted path, not the cancel
    /// branch. A later Back can ask again without duplicating the notification.
    pub fn cancel_clear_map(&mut self) -> Vec<EditorSettingsEditEffect> {
        if self.phase != EditorSettingsEditPhase::AwaitingClearConfirmation {
            return Vec::new();
        }
        self.phase = EditorSettingsEditPhase::Editing;
        vec![EditorSettingsEditEffect::DismissClearMapConfirmation]
    }
}

impl MapSettingTarget {
    pub const fn new(generation: MapSettingsGeneration, field: MapSettingField) -> Self {
        Self { generation, field }
    }

    pub const fn generation(self) -> MapSettingsGeneration {
        self.generation
    }

    pub const fn field(self) -> MapSettingField {
        self.field
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapSettingsCatalogError {
    NotOnMapSettings,
    DuplicateField,
    DuplicateChoice,
    SelectionMissingFromChoices,
    DuplicateEntry,
    EntryFieldMissing,
}

/// Actionable rows on the original loaded-editor menu. The noninteractive
/// `Edit` heading is intentionally absent. Order is measured from T1714's
/// descriptor and row creation sequence; row behavior is supplied separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorMenuAction {
    PreviewMap,
    Undo,
    Redo,
    MapSettings,
    EditorSettings,
    CenterMap,
    ClearMap,
    LoadMap,
    SaveMap,
    Controls,
    QuitMapMaker,
}

/// Typed request to the Rust-owned editor history service. This identifies
/// the requested direction only; record ordering and snapshot application are
/// owned by the history contract described in T1689.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorHistoryOperation {
    Undo,
    Redo,
}

/// Typed viewport operation requested by an editor-menu action. The renderer
/// or map-view service owns the actual camera state and its centering policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorViewportOperation {
    CenterMap,
}

/// Runtime-localized text identity. The UI model does not embed or interpret
/// the original language table's strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PreviewTextId(u32);

impl PreviewTextId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

pub const PREVIEW_CAPACITY_TEXT: PreviewTextId = PreviewTextId(0x16ee);

/// Result of the original preview-start gate sequence. Capacity failure and a
/// broken-logic warning are distinct outcomes; only the latter is overridable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PreviewStartGate {
    CapacityBlocked,
    ContinueAnyway(PreviewTextId),
    Allowed,
}

/// Result of the serialized/editor capacity check that runs before optional
/// preview validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PreviewCapacity {
    Sufficient,
    Insufficient,
}

/// Apply the measured Start ordering: capacity failure blocks unconditionally;
/// only a successful capacity check reaches the optional overridable warning.
/// The warning text ID is supplied by the runtime's selected preview row.
pub fn evaluate_preview_start_gate(
    capacity: PreviewCapacity,
    check_warning: impl FnOnce() -> Option<PreviewTextId>,
) -> PreviewStartGate {
    if capacity == PreviewCapacity::Insufficient {
        return PreviewStartGate::CapacityBlocked;
    }

    match check_warning() {
        Some(text) => PreviewStartGate::ContinueAnyway(text),
        None => PreviewStartGate::Allowed,
    }
}

#[cfg(test)]
mod preview_start_gate_tests {
    use super::{PreviewCapacity, PreviewStartGate, PreviewTextId, evaluate_preview_start_gate};

    #[test]
    fn capacity_failure_skips_warning_and_cannot_be_overridden() {
        assert_eq!(
            evaluate_preview_start_gate(PreviewCapacity::Insufficient, || {
                panic!("warning validation must follow a successful capacity check")
            }),
            PreviewStartGate::CapacityBlocked,
        );
    }

    #[test]
    fn warning_is_a_separate_continue_anyway_outcome() {
        let warning = PreviewTextId::new(0xaeb);
        assert_eq!(
            evaluate_preview_start_gate(PreviewCapacity::Sufficient, || Some(warning)),
            PreviewStartGate::ContinueAnyway(warning),
        );
    }

    #[test]
    fn successful_checks_allow_start() {
        assert_eq!(
            evaluate_preview_start_gate(PreviewCapacity::Sufficient, || None),
            PreviewStartGate::Allowed,
        );
    }
}

/// Generation-qualified preview gate request. A late service response from a
/// previous page visit cannot satisfy a newer Start action for the same row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PreviewGateId(u64);

impl PreviewGateId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PreviewStartRequest {
    pub id: PreviewGateId,
    pub row: PreviewRow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PreviewWarningChoice {
    Cancel,
    Continue,
}

/// Native dialog raised by an editor menu action. The action's original wording
/// and geometry remain runtime supplied; the owned state carries only intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorConfirmation {
    ClearMap,
    QuitMapMaker,
}

/// Semantic buttons on a native confirmation dialog. The page registry owns
/// their displayed text and hit geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorConfirmationChoice {
    Cancel,
    Accept,
}

/// A pointer target qualified by the active editor-menu page generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorConfirmationTarget {
    generation: EditorMenuGeneration,
    choice: EditorConfirmationChoice,
}

impl EditorConfirmationTarget {
    pub const fn new(generation: EditorMenuGeneration, choice: EditorConfirmationChoice) -> Self {
        Self { generation, choice }
    }

    pub const fn choice(self) -> EditorConfirmationChoice {
        self.choice
    }
}

pub const ORIGINAL_EDITOR_MENU_ACTIONS: &[EditorMenuAction] = &[
    EditorMenuAction::PreviewMap,
    EditorMenuAction::Undo,
    EditorMenuAction::Redo,
    EditorMenuAction::MapSettings,
    EditorMenuAction::EditorSettings,
    EditorMenuAction::CenterMap,
    EditorMenuAction::ClearMap,
    EditorMenuAction::LoadMap,
    EditorMenuAction::SaveMap,
    EditorMenuAction::Controls,
    EditorMenuAction::QuitMapMaker,
];

/// Generation of one installed editor-menu action table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorMenuGeneration(u64);

impl EditorMenuGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable semantic target resolved by the editor menu's runtime hit test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorMenuTarget {
    generation: EditorMenuGeneration,
    action: EditorMenuAction,
}

impl EditorMenuTarget {
    pub const fn new(generation: EditorMenuGeneration, action: EditorMenuAction) -> Self {
        Self { generation, action }
    }

    pub const fn generation(self) -> EditorMenuGeneration {
        self.generation
    }

    pub const fn action(self) -> EditorMenuAction {
        self.action
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorMenuCatalogError {
    NotAtEditorMenu,
    DuplicateAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorSurfaceCatalogError {
    NotInLoadedEditor,
    MainMenuIsNotAnAction,
    DuplicateSurface,
}

/// Runtime generation for an installed local-storage map catalogue. Identities
/// from an older generation are rejected even if the host reuses their keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapListGeneration(u64);

impl MapListGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Opaque row identity qualified by the installed catalogue generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapListRowId {
    generation: MapListGeneration,
    key: u64,
}

impl MapListRowId {
    pub const fn new(generation: MapListGeneration, key: u64) -> Self {
        Self { generation, key }
    }

    pub const fn generation(self) -> MapListGeneration {
        self.generation
    }

    pub const fn key(self) -> u64 {
        self.key
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapListRow {
    pub id: MapListRowId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapListError {
    DuplicateIdentity,
    WrongGeneration,
    LoadInFlight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapLoadRequestId(u64);

impl MapLoadRequestId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Typed request passed to the authenticated storage host. Selection alone
/// does not claim that a map was loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapLoadRequest {
    pub id: MapLoadRequestId,
    pub row: MapListRowId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapLoadOutcome {
    Loaded,
    Failed,
}

/// Stable identity supplied by the runtime mode table. It is deliberately
/// separate from editor kind, preview row, launch variant, and game type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModeChoiceId(u64);

impl ModeChoiceId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One runtime-provided row and its complete owned settings snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeChoice {
    pub id: ModeChoiceId,
    pub configuration: ModeConfiguration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeChoiceError {
    DuplicateIdentity,
}

/// Typed events accepted by the owned entry/mode page handlers. Device adapters
/// resolve keyboard, mouse, and pad input to these page-level commands; no
/// guest descriptor, widget ID, or synthetic device event crosses the seam.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapMakerUiCommand {
    /// Enter the loaded editor from the outer Map Maker flow. The caller owns
    /// the page transition and supplies this typed lifecycle event at the seam.
    BeginEditor,
    OpenModeSelection,
    OpenStorage,
    FocusStorageSource(MapStorageSource),
    SelectSampleMaps,
    SelectLocalStorage,
    FocusMode(ModeChoiceId),
    SelectMode(ModeChoiceId),
    FocusMap(MapListRowId),
    SelectMap(MapListRowId),
    CompleteMapLoad {
        request: MapLoadRequestId,
        outcome: MapLoadOutcome,
    },
    OpenStorageProgress(StorageOperation),
    CompleteStorageProgress {
        progress: StorageProgressId,
        outcome: StorageProgressOutcome,
    },
    FocusEditorSurface(EditorSurfaceTarget),
    OpenEditorSurface(EditorSurfaceTarget),
    FocusEditorMenuAction(EditorMenuTarget),
    ActivateEditorMenuAction(EditorMenuTarget),
    FocusEditorConfirmationChoice(EditorConfirmationTarget),
    ResolveEditorConfirmationChoice(EditorConfirmationTarget),
    ObserveQuitMapMakerConfirmation,
    FocusMapSetting(MapSettingTarget),
    FocusMapSettingsEntry(MapSettingsEntryTarget),
    ActivateMapSettingsAction(MapSettingsEntryTarget),
    SelectMapSetting {
        target: MapSettingTarget,
        choice: MapSettingChoiceId,
    },
    FocusEditorSetting(EditorSettingTarget),
    SelectEditorSetting {
        target: EditorSettingTarget,
        choice: EditorSettingChoiceId,
    },
    FocusPaletteProperty(PalettePropertyTarget),
    ActivatePaletteProperty(PalettePropertyTarget),
    FocusPaletteGridChoice(PaletteGridChoiceTarget),
    ActivatePaletteGridChoice(PaletteGridChoiceTarget),
    FocusEditorPreviewChoice(EditorPreviewChoiceTarget),
    ActivateEditorPreviewChoice(EditorPreviewChoiceTarget),
    FocusEditorSetEntry(EditorSetEntryTarget),
    ActivateEditorSetEntry(EditorSetEntryTarget),
    OpenPreview,
    StartPreview,
    CompletePreviewStartGate {
        request: PreviewGateId,
        gate: PreviewStartGate,
    },
    ResolvePreviewWarning(PreviewWarningChoice),
    Back,
}

/// Result of reducing one page-level command against owned UI state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapMakerUiUpdate {
    Ignored,
    PageChanged(MapMakerPage),
    StorageSourceFocused(MapStorageSource),
    ModeFocused(ModeChoiceId),
    PreviewOpened(PreviewRow),
    PreviewStartCheckRequested(PreviewStartRequest),
    PreviewCapacityBlocked {
        row: PreviewRow,
        text: PreviewTextId,
    },
    PreviewCapacityDismissed(PreviewRow),
    PreviewWarningOpened {
        row: PreviewRow,
        text: PreviewTextId,
    },
    PreviewWarningDismissed(PreviewRow),
    PreviewLaunchRequested(PreviewRow),
    MapFocused(MapListRowId),
    LoadRequested(MapLoadRequest),
    LoadCancelled(MapLoadRequestId),
    LoadCompleted(MapListRowId),
    LoadFailed(MapListRowId),
    StorageProgressOpened(StorageProgressId),
    StorageProgressCompleted(StorageProgress),
    StorageProgressDismissed(StorageOperation),
    EditorSurfaceFocused(EditorSurface),
    EditorSurfaceChanged(EditorSurface),
    /// The typed Save Map page is active and may hand off to the authenticated
    /// Rust storage/name-entry session. This is a route event, not save success.
    EditorSaveMapRouteOpened,
    EditorMenuActionFocused(EditorMenuAction),
    EditorMenuActionActivated(EditorMenuAction),
    EditorHistoryRequested(EditorHistoryOperation),
    EditorViewportOperationRequested(EditorViewportOperation),
    EditorConfirmationOpened(EditorConfirmation),
    EditorConfirmationFocused(EditorConfirmationChoice),
    EditorConfirmationDismissed(EditorConfirmation),
    MapSettingFocused(MapSettingField),
    MapSettingsEntryFocused(MapSettingsEntry),
    /// The runtime page handler owns the requested Map Settings action.
    MapSettingsActionRequested(MapSettingsAction),
    /// A Map Settings action with a registered Rust destination opened its
    /// generation-qualified editor surface.
    MapSettingsActionOpened {
        action: MapSettingsAction,
        surface: EditorSurface,
    },
    MapSettingChanged {
        field: MapSettingField,
        choice: MapSettingChoiceId,
    },
    EditorSettingFocused(EditorSettingField),
    EditorSettingChanged {
        field: EditorSettingField,
        choice: EditorSettingChoiceId,
    },
    PalettePropertyFocused(PalettePropertyAction),
    PalettePropertyActivated(PalettePropertyAction),
    PaletteGridChoiceFocused(PaletteGridChoiceId),
    /// Typed selection request; the palette document consumer owns its effect.
    PaletteGridChoiceActivated(PaletteGridChoiceId),
    EditorPreviewChoiceFocused(EditorPreviewChoiceId),
    /// Native preview selection with the original row, variant, and game type
    /// kept as separate typed values.
    EditorPreviewSelectionRequested(PreviewSelection),
    EditorSetEntryFocused(EditorSetEntryId),
    /// The native profile editor owns the effect of selecting this runtime row.
    EditorSetEntryActivated {
        list: EditorSetListKind,
        entry: EditorSetEntryId,
    },
    /// The measured Change to Light row re-enters the palette page. The runtime
    /// applies `PaletteSession::change_row_transition` to its typed document.
    PalettePageReentered,
    EditorSettingsEditEffect(EditorSettingsEditEffect),
    EditorSettingsEditRequested(EditorKind),
}

/// Owned state for Map Maker entry and mode selection.
///
/// `selected_mode` owns the full settings snapshot, including its distinct
/// preview row, launch variant and launch game type. This model deliberately
/// does not infer numeric correspondences between those namespaces.
#[derive(Clone, Debug, PartialEq)]
pub struct MapMakerUiState {
    experience: EditorExperience,
    page: MapMakerPage,
    mode_choices: Vec<ModeChoice>,
    focused_mode: Option<ModeChoiceId>,
    selected_mode_id: Option<ModeChoiceId>,
    selected_mode: Option<ModeConfiguration>,
    preview_row: Option<PreviewRow>,
    pending_preview_gate: Option<PreviewStartRequest>,
    next_preview_gate_id: u64,
    pending_preview_warning: Option<PreviewTextId>,
    preview_capacity_blocked: bool,
    overlay_ramp: Option<MapMakerOverlayRamp>,
    overlay_visibility: Option<MapMakerOverlayVisibility>,
    hint_anchor_transform: Option<HintAnchorTransform>,
    map_rows: Vec<MapListRow>,
    map_list_generation: Option<MapListGeneration>,
    focused_map: Option<MapListRowId>,
    focused_storage_source: Option<MapStorageSource>,
    pending_map_load: Option<MapLoadRequest>,
    next_map_load_id: u64,
    storage_progress: Option<StorageProgress>,
    storage_progress_return_page: Option<MapMakerPage>,
    next_storage_progress_id: u64,
    storage_return_page: MapMakerPage,
    editor_surface: EditorSurface,
    editor_surface_history: Vec<EditorSurface>,
    active_save_map_target: Option<EditorSurfaceTarget>,
    active_map_description_target: Option<EditorSurfaceTarget>,
    active_briefing_target: Option<EditorSurfaceTarget>,
    map_description_session: Option<MapDescriptionSession>,
    briefing_text_session: Option<MapDescriptionSession>,
    focused_editor_surface: Option<EditorSurface>,
    editor_surface_generation: Option<EditorSurfaceGeneration>,
    editor_surfaces: Vec<EditorSurface>,
    editor_menu_generation: Option<EditorMenuGeneration>,
    editor_menu_actions: Vec<EditorMenuAction>,
    focused_editor_menu_action: Option<EditorMenuAction>,
    editor_history_availability: Option<HistoryAvailability>,
    document_close_prompt: Option<ClosePromptState>,
    editor_confirmation: Option<EditorConfirmation>,
    focused_editor_confirmation_choice: Option<EditorConfirmationChoice>,
    map_settings_generation: Option<MapSettingsGeneration>,
    map_settings: Vec<MapSettingRow>,
    map_settings_entries: Vec<MapSettingsEntry>,
    focused_map_settings_entry: Option<MapSettingsEntry>,
    focused_map_setting: Option<MapSettingField>,
    editor_settings_generation: Option<EditorSettingsGeneration>,
    editor_settings: Vec<EditorSettingRow>,
    focused_editor_setting: Option<EditorSettingField>,
    palette_properties_generation: Option<PalettePropertiesGeneration>,
    palette_property_actions: Vec<PalettePropertyAction>,
    focused_palette_property: Option<PalettePropertyAction>,
    palette_grid_generation: Option<PaletteGridGeneration>,
    palette_grid_choices: Vec<PaletteGridChoiceId>,
    focused_palette_grid_choice: Option<PaletteGridChoiceId>,
    editor_preview_generation: Option<EditorPreviewGeneration>,
    editor_preview_choices: Vec<EditorPreviewChoiceId>,
    focused_editor_preview_choice: Option<EditorPreviewChoiceId>,
    editor_preview_selection: Option<PreviewSelection>,
    editor_set_list_generation: Option<EditorSetListGeneration>,
    editor_set_list_kind: Option<EditorSetListKind>,
    editor_set_entries: Vec<EditorSetEntry>,
    focused_editor_set_entry: Option<EditorSetEntryId>,
    editor_settings_edit: Option<EditorSettingsEdit>,
    editor_kind_choices: Vec<EditorKind>,
    disabled_modes: DisabledModesPage,
    assault_phase_assignment: Option<AssaultPhaseAssignmentState>,
    assault_start_point_picker: Option<StartPointPickerState>,
    assault_child_return_pending: bool,
}

/// Read-only projection for a native Map Maker page registry or renderer.
/// Runtime text lookup and layout geometry stay with their owning services;
/// this view exposes only the Rust-owned semantic state.
#[derive(Clone, Debug, PartialEq)]
pub struct MapMakerUiSnapshot<'a> {
    pub experience: EditorExperience,
    pub page: MapMakerPage,
    pub mode_choices: &'a [ModeChoice],
    pub focused_mode: Option<ModeChoiceId>,
    pub selected_mode_id: Option<ModeChoiceId>,
    pub selected_mode: Option<&'a ModeConfiguration>,
    pub preview_row: Option<PreviewRow>,
    pub pending_preview_gate: Option<PreviewStartRequest>,
    pub pending_preview_warning: Option<PreviewTextId>,
    pub preview_capacity_blocked: bool,
    pub overlay_ramp: Option<&'a MapMakerOverlayRamp>,
    pub overlay_visibility: Option<MapMakerOverlayVisibility>,
    pub hint_anchor_transform: Option<HintAnchorTransform>,
    pub map_rows: &'a [MapListRow],
    pub map_list_generation: Option<MapListGeneration>,
    pub focused_map: Option<MapListRowId>,
    pub focused_storage_source: Option<MapStorageSource>,
    pub pending_map_load: Option<MapLoadRequest>,
    pub storage_progress: Option<StorageProgress>,
    pub editor_surface: EditorSurface,
    pub editor_surface_history: &'a [EditorSurface],
    pub focused_editor_surface: Option<EditorSurface>,
    pub editor_surface_generation: Option<EditorSurfaceGeneration>,
    pub editor_surfaces: &'a [EditorSurface],
    pub editor_menu_generation: Option<EditorMenuGeneration>,
    pub editor_menu_actions: &'a [EditorMenuAction],
    pub focused_editor_menu_action: Option<EditorMenuAction>,
    /// Available only after a typed Rust document-history service publishes it.
    /// `None` preserves enabled state supplied by the runtime page.
    pub editor_history_availability: Option<HistoryAvailability>,
    /// Generation-qualified close transaction owned by the editor session.
    /// Row order, labels, and callback bindings remain caller supplied until
    /// the live Map Maker prompt contract is measured.
    pub document_close_prompt: Option<&'a ClosePromptState>,
    pub editor_confirmation: Option<EditorConfirmation>,
    pub focused_editor_confirmation_choice: Option<EditorConfirmationChoice>,
    pub map_settings_generation: Option<MapSettingsGeneration>,
    pub map_settings: &'a [MapSettingRow],
    pub map_settings_entries: &'a [MapSettingsEntry],
    pub focused_map_settings_entry: Option<MapSettingsEntry>,
    pub focused_map_setting: Option<MapSettingField>,
    pub editor_settings_generation: Option<EditorSettingsGeneration>,
    pub editor_settings: &'a [EditorSettingRow],
    pub focused_editor_setting: Option<EditorSettingField>,
    pub palette_properties_generation: Option<PalettePropertiesGeneration>,
    pub palette_property_actions: &'a [PalettePropertyAction],
    pub focused_palette_property: Option<PalettePropertyAction>,
    pub palette_grid_generation: Option<PaletteGridGeneration>,
    pub palette_grid_choices: &'a [PaletteGridChoiceId],
    pub focused_palette_grid_choice: Option<PaletteGridChoiceId>,
    pub editor_preview_generation: Option<EditorPreviewGeneration>,
    pub editor_preview_choices: &'a [EditorPreviewChoiceId],
    pub focused_editor_preview_choice: Option<EditorPreviewChoiceId>,
    pub editor_preview_selection: Option<PreviewSelection>,
    pub editor_set_list_generation: Option<EditorSetListGeneration>,
    pub editor_set_list_kind: Option<EditorSetListKind>,
    pub editor_set_entries: &'a [EditorSetEntry],
    pub focused_editor_set_entry: Option<EditorSetEntryId>,
    pub editor_settings_edit: Option<EditorSettingsEdit>,
    pub editor_kind_choices: &'a [EditorKind],
    /// Rust-owned checkbox values; row identities and serialization mapping
    /// remain with the evidence-backed page adapter.
    pub disabled_modes: &'a DisabledModesPage,
    /// Rust-owned interaction state for the measured Assault phase assignment
    /// page. The dynamic child list and document assignment remain caller data.
    pub assault_phase_assignment: Option<AssaultPhaseAssignmentState>,
    /// Rust-owned ordered choices and focus for the Assault start-point child
    /// page. Choice-to-document mapping and assignment effects remain caller data.
    pub assault_start_point_picker: Option<&'a StartPointPickerState>,
    /// Rust-owned Briefing text buffer. Runtime content, capacity and commit
    /// handling remain caller supplied until their original contract is measured.
    pub briefing_text_session: Option<BriefingTextView>,
}

impl Default for MapMakerUiState {
    fn default() -> Self {
        Self {
            experience: EditorExperience::Advanced,
            page: MapMakerPage::EditorMenu,
            mode_choices: Vec::new(),
            focused_mode: None,
            selected_mode_id: None,
            selected_mode: None,
            preview_row: None,
            pending_preview_gate: None,
            next_preview_gate_id: 0,
            pending_preview_warning: None,
            preview_capacity_blocked: false,
            overlay_ramp: None,
            overlay_visibility: None,
            hint_anchor_transform: None,
            map_rows: Vec::new(),
            map_list_generation: None,
            focused_map: None,
            focused_storage_source: None,
            pending_map_load: None,
            next_map_load_id: 0,
            storage_progress: None,
            storage_progress_return_page: None,
            next_storage_progress_id: 0,
            storage_return_page: MapMakerPage::EditorMenu,
            editor_surface: EditorSurface::MainMenu,
            editor_surface_history: Vec::new(),
            active_save_map_target: None,
            active_map_description_target: None,
            active_briefing_target: None,
            map_description_session: None,
            briefing_text_session: None,
            focused_editor_surface: None,
            editor_surface_generation: None,
            editor_surfaces: Vec::new(),
            editor_menu_generation: None,
            editor_menu_actions: Vec::new(),
            focused_editor_menu_action: None,
            editor_history_availability: None,
            document_close_prompt: None,
            editor_confirmation: None,
            focused_editor_confirmation_choice: None,
            map_settings_generation: None,
            map_settings: Vec::new(),
            map_settings_entries: Vec::new(),
            focused_map_settings_entry: None,
            focused_map_setting: None,
            editor_settings_generation: None,
            editor_settings: Vec::new(),
            focused_editor_setting: None,
            palette_properties_generation: None,
            palette_property_actions: Vec::new(),
            focused_palette_property: None,
            palette_grid_generation: None,
            palette_grid_choices: Vec::new(),
            focused_palette_grid_choice: None,
            editor_preview_generation: None,
            editor_preview_choices: Vec::new(),
            focused_editor_preview_choice: None,
            editor_preview_selection: None,
            editor_set_list_generation: None,
            editor_set_list_kind: None,
            editor_set_entries: Vec::new(),
            focused_editor_set_entry: None,
            editor_settings_edit: None,
            editor_kind_choices: Vec::new(),
            disabled_modes: DisabledModesPage::default(),
            assault_phase_assignment: None,
            assault_start_point_picker: None,
            assault_child_return_pending: false,
        }
    }
}

impl MapMakerUiState {
    /// Attach the generation-qualified close transaction to the owned editor
    /// page. The runtime remains responsible for supplying the document token;
    /// this state never retains a guest pointer or descriptor.
    pub fn open_document_close_prompt(&mut self, token: ClosePromptToken) -> bool {
        if self.page != MapMakerPage::Editor || token.generation() == 0 {
            return false;
        }
        if self.document_close_prompt.as_ref().is_some_and(|prompt| {
            matches!(
                prompt.phase(),
                ClosePromptPhase::AwaitingChoice | ClosePromptPhase::Saving(_)
            )
        }) {
            return false;
        }
        self.document_close_prompt = Some(ClosePromptState::new(token));
        true
    }

    /// Reduce typed modal input only while the editor page owns the UI. The
    /// close model validates its document generation and save request ID.
    pub fn apply_document_close_prompt(
        &mut self,
        command: ClosePromptCommand,
    ) -> ClosePromptUpdate {
        if matches!(command, ClosePromptCommand::Choose { .. }) && self.page != MapMakerPage::Editor
        {
            return ClosePromptUpdate::Ignored;
        }
        self.document_close_prompt
            .as_mut()
            .map_or(ClosePromptUpdate::Ignored, |prompt| {
                prompt.dispatch(command)
            })
    }

    pub const fn document_close_prompt_phase(&self) -> Option<ClosePromptPhase> {
        match &self.document_close_prompt {
            Some(prompt) => Some(prompt.phase()),
            None => None,
        }
    }

    pub const fn document_close_prompt_token(&self) -> Option<ClosePromptToken> {
        match &self.document_close_prompt {
            Some(prompt) => Some(prompt.token()),
            None => None,
        }
    }

    /// Shared Rust-owned per-mode checkbox values. Page-generation targets
    /// expire independently of this editing-session state.
    pub fn disabled_modes(&self) -> &DisabledModesPage {
        &self.disabled_modes
    }

    pub fn disabled_modes_mut(&mut self) -> &mut DisabledModesPage {
        &mut self.disabled_modes
    }

    /// Shared owned interaction state for the Assault phase assignment page.
    /// The live adapter supplies page generations and keeps child-row identity
    /// mapping at its typed caller boundary.
    pub const fn assault_phase_assignment(&self) -> Option<AssaultPhaseAssignmentState> {
        self.assault_phase_assignment
    }

    pub fn set_assault_phase_assignment(&mut self, state: Option<AssaultPhaseAssignmentState>) {
        self.assault_phase_assignment = state;
        if state.is_none() {
            self.assault_child_return_pending = false;
        }
    }

    /// Whether a child-picker return can reuse the current parent page lifetime.
    pub const fn assault_child_return_pending(&self) -> bool {
        self.assault_child_return_pending
    }

    /// Preserve the parent reducer while a start-point child is opening or
    /// closing. A completed/cancelled child result is retained for the next
    /// parent-page observation, then ordinary page transitions clear it.
    pub fn observe_assault_phase_page_change(&mut self, parent_page_active: bool) {
        if parent_page_active && self.assault_child_return_pending {
            self.assault_child_return_pending = false;
            return;
        }
        let child_flow_active = self.assault_start_point_picker.is_some()
            || self
                .assault_phase_assignment
                .is_some_and(|state| state.awaiting_start_point_selection());
        if !parent_page_active && child_flow_active {
            return;
        }
        self.assault_phase_assignment = None;
        self.assault_child_return_pending = false;
        if !parent_page_active {
            self.assault_start_point_picker = None;
        }
    }

    /// Dispatch a native input command through the shared Rust-owned Assault
    /// phase-assignment state. A stale page or absent child is ignored.
    pub fn dispatch_assault_phase_assignment(
        &mut self,
        command: AssaultPhaseAssignmentCommand,
    ) -> AssaultPhaseAssignmentUpdate {
        self.assault_phase_assignment
            .as_mut()
            .map_or(AssaultPhaseAssignmentUpdate::Ignored, |state| {
                state.dispatch(command)
            })
    }

    /// Shared owned interaction state for the Assault start-point child page.
    /// Its choices are opaque and generation-qualified; assignment effects
    /// remain with the document owner until their original callback is known.
    pub fn assault_start_point_picker(&self) -> Option<&StartPointPickerState> {
        self.assault_start_point_picker.as_ref()
    }

    pub fn set_assault_start_point_picker(&mut self, state: Option<StartPointPickerState>) {
        self.assault_start_point_picker = state;
    }

    /// Mutate the active native child-page reducer without copying it into a
    /// renderer or input adapter. Returns `None` when that page is not active.
    pub fn with_assault_start_point_picker_mut<R>(
        &mut self,
        update: impl FnOnce(&mut StartPointPickerState) -> R,
    ) -> Option<R> {
        self.assault_start_point_picker.as_mut().map(update)
    }

    /// Complete the typed parent/child UI handoff for the currently requested
    /// Assault start-point selection. Choice IDs remain opaque: the caller
    /// retains the mapping to its document record and applies that effect.
    pub fn complete_assault_start_point_selection(
        &mut self,
        generation: StartPointPickerGeneration,
        choice: StartPointChoiceId,
    ) -> AssaultPhaseAssignmentUpdate {
        let Some(picker) = self.assault_start_point_picker.as_ref() else {
            return AssaultPhaseAssignmentUpdate::Ignored;
        };
        if picker.generation() != generation || picker.target(choice).is_none() {
            return AssaultPhaseAssignmentUpdate::Ignored;
        }
        let Some(assignment) = self.assault_phase_assignment.as_mut() else {
            return AssaultPhaseAssignmentUpdate::Ignored;
        };
        let update = assignment.assign_start_point(choice);
        if matches!(update, AssaultPhaseAssignmentUpdate::StartPointAssigned(_)) {
            self.assault_start_point_picker = None;
            self.assault_child_return_pending = true;
        }
        update
    }

    /// Cancel the active child choice list while retaining any prior selected
    /// start point. A stale list generation cannot close a newer child page.
    pub fn cancel_assault_start_point_selection(
        &mut self,
        generation: StartPointPickerGeneration,
    ) -> AssaultPhaseAssignmentUpdate {
        if self
            .assault_start_point_picker
            .as_ref()
            .is_none_or(|picker| picker.generation() != generation)
        {
            return AssaultPhaseAssignmentUpdate::Ignored;
        }
        let Some(assignment) = self.assault_phase_assignment.as_mut() else {
            return AssaultPhaseAssignmentUpdate::Ignored;
        };
        let update = assignment.cancel_start_point_selection();
        if update == AssaultPhaseAssignmentUpdate::StartPointSelectionCancelled {
            self.assault_start_point_picker = None;
            self.assault_child_return_pending = true;
        }
        update
    }

    pub fn experience(&self) -> EditorExperience {
        self.experience
    }

    pub fn page(&self) -> MapMakerPage {
        self.page
    }

    pub fn editor_surface(&self) -> EditorSurface {
        self.editor_surface
    }

    pub fn focused_editor_surface(&self) -> Option<EditorSurface> {
        self.focused_editor_surface
    }

    pub fn editor_surfaces(&self) -> &[EditorSurface] {
        &self.editor_surfaces
    }

    pub fn editor_surface_generation(&self) -> Option<EditorSurfaceGeneration> {
        self.editor_surface_generation
    }

    /// Preserve the generation-qualified row that opened the active Save Map
    /// route after its menu catalogue is cleared for the page transition.
    pub fn active_save_map_target(&self) -> Option<EditorSurfaceTarget> {
        (self.page == MapMakerPage::Editor && self.editor_surface == EditorSurface::SaveMap)
            .then_some(self.active_save_map_target)
            .flatten()
    }

    /// Return a current semantic target for keyboard/controller focus or a
    /// renderer hit-test result. The runtime page order is copied into this
    /// state before input is resolved.
    pub fn editor_surface_target(&self, surface: EditorSurface) -> Option<EditorSurfaceTarget> {
        let generation = self.editor_surface_generation?;
        (self.page == MapMakerPage::Editor && self.editor_surfaces.contains(&surface))
            .then_some(EditorSurfaceTarget::new(generation, surface))
    }

    pub fn contains_editor_surface_target(&self, target: EditorSurfaceTarget) -> bool {
        self.editor_surface_generation == Some(target.generation)
            && self.page == MapMakerPage::Editor
            && target.surface != EditorSurface::MainMenu
            && self.editor_surfaces.contains(&target.surface)
    }

    pub fn editor_menu_actions(&self) -> &[EditorMenuAction] {
        &self.editor_menu_actions
    }

    pub fn focused_editor_menu_action(&self) -> Option<EditorMenuAction> {
        self.focused_editor_menu_action
    }

    pub fn editor_menu_generation(&self) -> Option<EditorMenuGeneration> {
        self.editor_menu_generation
    }

    /// Publish availability from the typed Rust document-history service.
    /// Keeping it optional lets the live page retain its current row policy
    /// until that service is attached to the editor document.
    pub fn set_editor_history_availability(&mut self, availability: Option<HistoryAvailability>) {
        self.editor_history_availability = availability;
    }

    pub fn editor_menu_target(&self, action: EditorMenuAction) -> Option<EditorMenuTarget> {
        let generation = self.editor_menu_generation?;
        (self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::MainMenu
            && self.editor_menu_actions.contains(&action))
        .then_some(EditorMenuTarget::new(generation, action))
    }

    pub const fn editor_confirmation(&self) -> Option<EditorConfirmation> {
        self.editor_confirmation
    }

    pub const fn focused_editor_confirmation_choice(&self) -> Option<EditorConfirmationChoice> {
        self.focused_editor_confirmation_choice
    }

    pub fn editor_confirmation_target(
        &self,
        choice: EditorConfirmationChoice,
    ) -> Option<EditorConfirmationTarget> {
        let generation = self.editor_menu_generation?;
        (self.editor_confirmation.is_some()
            && self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::MainMenu)
            .then_some(EditorConfirmationTarget::new(generation, choice))
    }

    pub fn contains_editor_confirmation_target(&self, target: EditorConfirmationTarget) -> bool {
        self.editor_confirmation.is_some()
            && self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::MainMenu
            && self.editor_menu_generation == Some(target.generation)
    }

    pub fn contains_editor_menu_target(&self, target: EditorMenuTarget) -> bool {
        self.editor_menu_generation == Some(target.generation)
            && self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::MainMenu
            && self.editor_confirmation.is_none()
            && self.editor_menu_actions.contains(&target.action)
    }

    pub fn map_settings(&self) -> &[MapSettingRow] {
        &self.map_settings
    }

    pub fn map_settings_generation(&self) -> Option<MapSettingsGeneration> {
        self.map_settings_generation
    }

    pub fn map_settings_entries(&self) -> &[MapSettingsEntry] {
        &self.map_settings_entries
    }

    pub fn focused_map_settings_entry(&self) -> Option<MapSettingsEntry> {
        self.focused_map_settings_entry
    }

    pub fn focused_map_setting(&self) -> Option<MapSettingField> {
        self.focused_map_setting
    }

    pub fn map_setting_target(&self, field: MapSettingField) -> Option<MapSettingTarget> {
        let generation = self.map_settings_generation?;
        (self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::MapSettings
            && self.map_settings.iter().any(|row| row.field == field))
        .then_some(MapSettingTarget::new(generation, field))
    }

    pub fn contains_map_setting_target(&self, target: MapSettingTarget) -> bool {
        self.map_settings_generation == Some(target.generation)
            && self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::MapSettings
            && self
                .map_settings
                .iter()
                .any(|row| row.field == target.field)
    }

    pub fn map_settings_entry_target(
        &self,
        entry: MapSettingsEntry,
    ) -> Option<MapSettingsEntryTarget> {
        let generation = self.map_settings_generation?;
        (self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::MapSettings
            && self.map_settings_entries.contains(&entry))
        .then_some(MapSettingsEntryTarget::new(generation, entry))
    }

    pub fn contains_map_settings_entry_target(&self, target: MapSettingsEntryTarget) -> bool {
        self.map_settings_generation == Some(target.generation)
            && self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::MapSettings
            && self.map_settings_entries.contains(&target.entry)
    }

    /// Install the live Map Settings rows and choice identities atomically.
    /// The original table's exact choice values and labels stay with the
    /// runtime provider; this state keeps only the typed field and identities.
    pub fn install_map_settings(
        &mut self,
        generation: MapSettingsGeneration,
        rows: Vec<MapSettingRow>,
    ) -> Result<(), MapSettingsCatalogError> {
        let entries = rows
            .iter()
            .map(|row| MapSettingsEntry::Field(row.field))
            .collect();
        self.install_map_settings_page(generation, rows, entries)
    }

    /// Install all focusable rows in their runtime order. Actions retain opaque
    /// typed identities; selecting one emits a request without inventing the
    /// underlying map-data or subpage behavior.
    pub fn install_map_settings_page(
        &mut self,
        generation: MapSettingsGeneration,
        rows: Vec<MapSettingRow>,
        entries: Vec<MapSettingsEntry>,
    ) -> Result<(), MapSettingsCatalogError> {
        if self.page != MapMakerPage::Editor || self.editor_surface != EditorSurface::MapSettings {
            return Err(MapSettingsCatalogError::NotOnMapSettings);
        }
        for (index, row) in rows.iter().enumerate() {
            if rows[..index].iter().any(|prior| prior.field == row.field) {
                return Err(MapSettingsCatalogError::DuplicateField);
            }
            for (choice_index, choice) in row.choices.iter().enumerate() {
                if row.choices[..choice_index].contains(choice) {
                    return Err(MapSettingsCatalogError::DuplicateChoice);
                }
            }
            if row
                .selected
                .is_some_and(|selected| !row.choices.contains(&selected))
            {
                return Err(MapSettingsCatalogError::SelectionMissingFromChoices);
            }
        }
        for (index, entry) in entries.iter().enumerate() {
            if entries[..index].contains(entry) {
                return Err(MapSettingsCatalogError::DuplicateEntry);
            }
            if let MapSettingsEntry::Field(field) = entry
                && !rows.iter().any(|row| row.field == *field)
            {
                return Err(MapSettingsCatalogError::EntryFieldMissing);
            }
        }
        if rows
            .iter()
            .any(|row| !entries.contains(&MapSettingsEntry::Field(row.field)))
        {
            return Err(MapSettingsCatalogError::EntryFieldMissing);
        }
        let focus_survives = self
            .focused_map_settings_entry
            .is_some_and(|focused| entries.contains(&focused));
        self.focused_map_settings_entry = if focus_survives {
            self.focused_map_settings_entry
        } else {
            entries.first().copied()
        };
        self.focused_map_setting = match self.focused_map_settings_entry {
            Some(MapSettingsEntry::Field(field)) => Some(field),
            Some(MapSettingsEntry::Action(_)) | None => None,
        };
        self.map_settings_generation = Some(generation);
        self.map_settings = rows;
        self.map_settings_entries = entries;
        Ok(())
    }

    fn invalidate_map_settings_targets(&mut self) {
        self.map_settings_generation = None;
        self.map_settings_entries.clear();
        self.focused_map_settings_entry = None;
        self.focused_map_setting = None;
    }

    pub fn editor_settings(&self) -> &[EditorSettingRow] {
        &self.editor_settings
    }

    pub fn focused_editor_setting(&self) -> Option<EditorSettingField> {
        self.focused_editor_setting
    }

    pub fn palette_property_actions(&self) -> &[PalettePropertyAction] {
        &self.palette_property_actions
    }

    pub fn focused_palette_property(&self) -> Option<PalettePropertyAction> {
        self.focused_palette_property
    }

    pub fn palette_grid_choices(&self) -> &[PaletteGridChoiceId] {
        &self.palette_grid_choices
    }

    pub fn focused_palette_grid_choice(&self) -> Option<PaletteGridChoiceId> {
        self.focused_palette_grid_choice
    }

    pub fn palette_grid_choice_target(
        &self,
        choice: PaletteGridChoiceId,
    ) -> Option<PaletteGridChoiceTarget> {
        let generation = self.palette_grid_generation?;
        (self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::PaletteGrid
            && self.palette_grid_choices.contains(&choice))
        .then_some(PaletteGridChoiceTarget::new(generation, choice))
    }

    pub fn contains_palette_grid_choice_target(&self, target: PaletteGridChoiceTarget) -> bool {
        self.palette_grid_generation == Some(target.generation)
            && self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::PaletteGrid
            && self.palette_grid_choices.contains(&target.choice)
    }

    pub fn editor_preview_choices(&self) -> &[EditorPreviewChoiceId] {
        &self.editor_preview_choices
    }

    pub fn focused_editor_preview_choice(&self) -> Option<EditorPreviewChoiceId> {
        self.focused_editor_preview_choice
    }

    pub fn editor_set_entries(&self) -> &[EditorSetEntry] {
        &self.editor_set_entries
    }

    pub fn editor_set_list_kind(&self) -> Option<EditorSetListKind> {
        self.editor_set_list_kind
    }

    pub fn focused_editor_set_entry(&self) -> Option<EditorSetEntryId> {
        self.focused_editor_set_entry
    }

    pub fn editor_set_entry_target(&self, entry: EditorSetEntryId) -> Option<EditorSetEntryTarget> {
        let generation = self.editor_set_list_generation?;
        let list = self.editor_set_list_kind?;
        (self.page == MapMakerPage::Editor
            && self.editor_surface == list.surface()
            && self.editor_set_entries.iter().any(|row| row.id == entry))
        .then_some(EditorSetEntryTarget::new(generation, list, entry))
    }

    pub fn contains_editor_set_entry_target(&self, target: EditorSetEntryTarget) -> bool {
        self.editor_set_list_generation == Some(target.generation)
            && self.editor_set_list_kind == Some(target.list)
            && self.page == MapMakerPage::Editor
            && self.editor_surface == target.list.surface()
            && self
                .editor_set_entries
                .iter()
                .any(|row| row.id == target.entry)
    }

    pub fn editor_preview_choice_target(
        &self,
        choice: EditorPreviewChoiceId,
    ) -> Option<EditorPreviewChoiceTarget> {
        let generation = self.editor_preview_generation?;
        (self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::MapPreview
            && self.editor_preview_choices.contains(&choice))
        .then_some(EditorPreviewChoiceTarget::new(generation, choice))
    }

    pub fn contains_editor_preview_choice_target(&self, target: EditorPreviewChoiceTarget) -> bool {
        self.editor_preview_generation == Some(target.generation)
            && self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::MapPreview
            && self.editor_preview_choices.contains(&target.choice)
    }

    pub fn palette_property_target(
        &self,
        action: PalettePropertyAction,
    ) -> Option<PalettePropertyTarget> {
        let generation = self.palette_properties_generation?;
        (self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::PaletteProperties
            && self.palette_property_actions.contains(&action))
        .then_some(PalettePropertyTarget::new(generation, action))
    }

    pub fn contains_palette_property_target(&self, target: PalettePropertyTarget) -> bool {
        self.palette_properties_generation == Some(target.generation)
            && self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::PaletteProperties
            && self.palette_property_actions.contains(&target.action)
    }

    pub fn editor_settings_edit(&self) -> Option<EditorSettingsEdit> {
        self.editor_settings_edit
    }

    pub fn editor_kind_choices(&self) -> &[EditorKind] {
        &self.editor_kind_choices
    }

    pub fn editor_kind_target(&self, kind: EditorKind) -> Option<EditorKindTarget> {
        let generation = self.editor_settings_generation?;
        (self
            .editor_settings_edit
            .is_some_and(|edit| edit.phase() == EditorSettingsEditPhase::Editing)
            && self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::EditorSettings
            && self.editor_kind_choices.contains(&kind))
        .then_some(EditorKindTarget::new(generation, kind))
    }

    pub fn contains_editor_kind_target(&self, target: EditorKindTarget) -> bool {
        self.editor_settings_generation == Some(target.generation)
            && self
                .editor_settings_edit
                .is_some_and(|edit| edit.phase() == EditorSettingsEditPhase::Editing)
            && self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::EditorSettings
            && self.editor_kind_choices.contains(&target.kind)
    }

    /// Start the source-defined editor-kind transaction on its owned page.
    pub fn begin_editor_settings_edit(
        &mut self,
        current: EditorKind,
        notify_on_change: bool,
        choices: Vec<EditorKind>,
    ) -> bool {
        let can_restart = self
            .editor_settings_edit
            .is_some_and(|edit| edit.phase() == EditorSettingsEditPhase::Finished);
        if self.page != MapMakerPage::Editor
            || self.editor_surface != EditorSurface::EditorSettings
            || (self.editor_settings_edit.is_some() && !can_restart)
            || !choices.contains(&current)
            || choices
                .iter()
                .enumerate()
                .any(|(index, choice)| choices[..index].contains(choice))
        {
            return false;
        }
        self.editor_kind_choices = choices;
        self.editor_settings_edit = Some(EditorSettingsEdit::new(current, notify_on_change));
        true
    }

    pub fn request_editor_kind(&mut self, kind: EditorKind) -> bool {
        self.editor_kind_choices.contains(&kind)
            && self
                .editor_settings_edit
                .as_mut()
                .is_some_and(|edit| edit.request(kind))
    }

    pub fn leave_editor_settings_edit(&mut self) -> Vec<EditorSettingsEditEffect> {
        if self.page != MapMakerPage::Editor || self.editor_surface != EditorSurface::EditorSettings
        {
            return Vec::new();
        }
        self.editor_settings_edit
            .as_mut()
            .map_or_else(Vec::new, EditorSettingsEdit::leave_settings)
    }

    pub fn confirm_editor_kind_clear(&mut self) -> Vec<EditorSettingsEditEffect> {
        if self.page != MapMakerPage::Editor || self.editor_surface != EditorSurface::EditorSettings
        {
            return Vec::new();
        }
        self.editor_settings_edit
            .as_mut()
            .map_or_else(Vec::new, EditorSettingsEdit::confirm_clear_map)
    }

    pub fn cancel_editor_kind_clear(&mut self) -> Vec<EditorSettingsEditEffect> {
        if self.page != MapMakerPage::Editor || self.editor_surface != EditorSurface::EditorSettings
        {
            return Vec::new();
        }
        self.editor_settings_edit
            .as_mut()
            .map_or_else(Vec::new, EditorSettingsEdit::cancel_clear_map)
    }

    /// Apply the completed transaction's CloseSettings effect to the owned
    /// page stack after its preceding typed effects have been consumed.
    pub fn close_finished_editor_settings_edit(&mut self) -> bool {
        if self.editor_settings_edit.map(|edit| edit.phase())
            != Some(EditorSettingsEditPhase::Finished)
            || self.page != MapMakerPage::Editor
            || self.editor_surface != EditorSurface::EditorSettings
        {
            return false;
        }
        matches!(
            self.dispatch(MapMakerUiCommand::Back),
            MapMakerUiUpdate::EditorSurfaceChanged(_)
        )
    }

    pub fn editor_setting_target(&self, field: EditorSettingField) -> Option<EditorSettingTarget> {
        let generation = self.editor_settings_generation?;
        (self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::EditorSettings
            && self.editor_settings.iter().any(|row| row.field == field))
        .then_some(EditorSettingTarget::new(generation, field))
    }

    pub fn contains_editor_setting_target(&self, target: EditorSettingTarget) -> bool {
        self.editor_settings_generation == Some(target.generation)
            && self.page == MapMakerPage::Editor
            && self.editor_surface == EditorSurface::EditorSettings
            && self
                .editor_settings
                .iter()
                .any(|row| row.field == target.field)
    }

    /// Install ordered runtime choices for the native subset of Editor
    /// Settings. Mode transactions and Xbox Live presence are separate seams.
    pub fn install_editor_settings(
        &mut self,
        generation: EditorSettingsGeneration,
        rows: Vec<EditorSettingRow>,
    ) -> Result<(), EditorSettingsCatalogError> {
        if self.page != MapMakerPage::Editor || self.editor_surface != EditorSurface::EditorSettings
        {
            return Err(EditorSettingsCatalogError::NotOnEditorSettings);
        }
        for (index, row) in rows.iter().enumerate() {
            if rows[..index].iter().any(|prior| prior.field == row.field) {
                return Err(EditorSettingsCatalogError::DuplicateField);
            }
            for (choice_index, choice) in row.choices.iter().enumerate() {
                if row.choices[..choice_index].contains(choice) {
                    return Err(EditorSettingsCatalogError::DuplicateChoice);
                }
            }
            if row
                .selected
                .is_some_and(|selected| !row.choices.contains(&selected))
            {
                return Err(EditorSettingsCatalogError::SelectionMissingFromChoices);
            }
        }
        let focus_survives = self
            .focused_editor_setting
            .is_some_and(|focused| rows.iter().any(|row| row.field == focused));
        self.focused_editor_setting = if focus_survives {
            self.focused_editor_setting
        } else {
            rows.first().map(|row| row.field)
        };
        self.editor_settings_generation = Some(generation);
        self.editor_settings = rows;
        Ok(())
    }

    fn invalidate_editor_settings_targets(&mut self) {
        self.editor_settings_generation = None;
        self.focused_editor_setting = None;
        self.editor_settings_edit = None;
        self.editor_kind_choices.clear();
    }

    pub fn selected_mode(&self) -> Option<&ModeConfiguration> {
        self.selected_mode.as_ref()
    }

    pub fn mode_choices(&self) -> &[ModeChoice] {
        &self.mode_choices
    }

    pub fn focused_mode(&self) -> Option<ModeChoiceId> {
        self.focused_mode
    }

    pub fn selected_mode_id(&self) -> Option<ModeChoiceId> {
        self.selected_mode_id
    }

    pub fn preview_row(&self) -> Option<PreviewRow> {
        self.preview_row
    }

    /// Project owned state into a borrowed renderer-facing view. Consumers
    /// cannot mutate the state or obtain guest page/descriptor identities.
    pub fn snapshot(&self) -> MapMakerUiSnapshot<'_> {
        MapMakerUiSnapshot {
            experience: self.experience,
            page: self.page,
            mode_choices: &self.mode_choices,
            focused_mode: self.focused_mode,
            selected_mode_id: self.selected_mode_id,
            selected_mode: self.selected_mode.as_ref(),
            preview_row: self.preview_row,
            pending_preview_gate: self.pending_preview_gate,
            pending_preview_warning: self.pending_preview_warning,
            preview_capacity_blocked: self.preview_capacity_blocked,
            overlay_ramp: self.overlay_ramp.as_ref(),
            overlay_visibility: self.overlay_visibility,
            hint_anchor_transform: self.hint_anchor_transform,
            map_rows: &self.map_rows,
            map_list_generation: self.map_list_generation,
            focused_map: self.focused_map,
            focused_storage_source: self.focused_storage_source,
            pending_map_load: self.pending_map_load,
            storage_progress: self.storage_progress,
            editor_surface: self.editor_surface,
            editor_surface_history: &self.editor_surface_history,
            focused_editor_surface: self.focused_editor_surface,
            editor_surface_generation: self.editor_surface_generation,
            editor_surfaces: &self.editor_surfaces,
            editor_menu_generation: self.editor_menu_generation,
            editor_menu_actions: &self.editor_menu_actions,
            focused_editor_menu_action: self.focused_editor_menu_action,
            editor_history_availability: self.editor_history_availability,
            document_close_prompt: self.document_close_prompt.as_ref(),
            editor_confirmation: self.editor_confirmation,
            focused_editor_confirmation_choice: self.focused_editor_confirmation_choice,
            map_settings_generation: self.map_settings_generation,
            map_settings: &self.map_settings,
            map_settings_entries: &self.map_settings_entries,
            focused_map_settings_entry: self.focused_map_settings_entry,
            focused_map_setting: self.focused_map_setting,
            editor_settings_generation: self.editor_settings_generation,
            editor_settings: &self.editor_settings,
            focused_editor_setting: self.focused_editor_setting,
            palette_properties_generation: self.palette_properties_generation,
            palette_property_actions: &self.palette_property_actions,
            focused_palette_property: self.focused_palette_property,
            palette_grid_generation: self.palette_grid_generation,
            palette_grid_choices: &self.palette_grid_choices,
            focused_palette_grid_choice: self.focused_palette_grid_choice,
            editor_preview_generation: self.editor_preview_generation,
            editor_preview_choices: &self.editor_preview_choices,
            focused_editor_preview_choice: self.focused_editor_preview_choice,
            editor_preview_selection: self.editor_preview_selection,
            editor_set_list_generation: self.editor_set_list_generation,
            editor_set_list_kind: self.editor_set_list_kind,
            editor_set_entries: &self.editor_set_entries,
            focused_editor_set_entry: self.focused_editor_set_entry,
            editor_settings_edit: self.editor_settings_edit,
            editor_kind_choices: &self.editor_kind_choices,
            disabled_modes: &self.disabled_modes,
            assault_phase_assignment: self.assault_phase_assignment,
            assault_start_point_picker: self.assault_start_point_picker.as_ref(),
            briefing_text_session: self.briefing_text_view(),
        }
    }

    pub fn map_rows(&self) -> &[MapListRow] {
        &self.map_rows
    }

    pub fn focused_map(&self) -> Option<MapListRowId> {
        self.focused_map
    }

    pub fn focused_storage_source(&self) -> Option<MapStorageSource> {
        self.focused_storage_source
    }

    pub fn pending_map_load(&self) -> Option<MapLoadRequest> {
        self.pending_map_load
    }

    pub fn storage_progress(&self) -> Option<StorageProgress> {
        self.storage_progress
    }

    /// Replace the runtime-provided map list atomically. Rows may represent
    /// local saves or provider-owned sample maps; their identity is opaque to
    /// this UI model. Duplicate or wrong-generation IDs leave the prior list
    /// untouched. A list is not refreshed while a host load is outstanding.
    pub fn install_map_rows(
        &mut self,
        generation: MapListGeneration,
        rows: Vec<MapListRow>,
    ) -> Result<(), MapListError> {
        if self.pending_map_load.is_some() {
            return Err(MapListError::LoadInFlight);
        }
        for (index, row) in rows.iter().enumerate() {
            if row.id.generation != generation {
                return Err(MapListError::WrongGeneration);
            }
            if rows[..index].iter().any(|prior| prior.id == row.id) {
                return Err(MapListError::DuplicateIdentity);
            }
        }
        if self.map_list_generation != Some(generation)
            || !rows.iter().any(|row| Some(row.id) == self.focused_map)
        {
            self.focused_map = None;
        }
        self.map_rows = rows;
        self.map_list_generation = Some(generation);
        Ok(())
    }

    /// Compatibility name for the local-save list adapter.
    pub fn install_local_map_rows(
        &mut self,
        generation: MapListGeneration,
        rows: Vec<MapListRow>,
    ) -> Result<(), MapListError> {
        self.install_map_rows(generation, rows)
    }

    /// Install runtime-owned overlay timing without exposing guest globals.
    pub fn install_overlay_ramp(&mut self, ramp: MapMakerOverlayRamp) {
        self.overlay_ramp = Some(ramp);
    }

    /// Install typed widget-tree facts for the current editor frame.
    pub fn install_overlay_visibility(&mut self, visibility: MapMakerOverlayVisibility) {
        self.overlay_visibility = Some(visibility);
    }

    /// Advance the owned overlay from the installed visibility facts.
    ///
    /// Missing facts or a missing ramp leave presentation untouched; runtime
    /// callers must supply the measured inputs for the frame before advancing.
    pub fn step_overlay(&mut self) -> Option<f32> {
        let shown = self.overlay_visibility?.shown();
        let ramp = self.overlay_ramp.as_mut()?;
        ramp.step(shown);
        Some(ramp.alpha())
    }

    /// Install the current pointer-relative hint-anchor transform supplied by
    /// the runtime overlay service, without retaining guest state.
    pub fn install_hint_anchor_transform(&mut self, transform: HintAnchorTransform) {
        self.hint_anchor_transform = Some(transform);
    }

    /// Shift a renderer-provided hint anchor using the T2418 arithmetic
    /// contract. Coordinates and transform operands remain runtime supplied.
    pub fn shift_hint_anchor(&self, x: i32, y_offset: i32) -> Option<(i32, i32)> {
        self.hint_anchor_transform
            .map(|transform| shift_hint_anchor(transform, x, y_offset))
    }

    /// Change the editor experience without changing the semantic map mode or
    /// any mode settings. The live UI binding is owned by a later slice.
    pub fn set_experience(&mut self, experience: EditorExperience) {
        self.experience = experience;
    }

    /// Install the runtime's ordered mode table. Identity is opaque and unique;
    /// no numeric relationship with the mode's other selector domains is used.
    /// A current focus is retained only if its identity survives replacement.
    pub fn install_mode_choices(
        &mut self,
        choices: Vec<ModeChoice>,
    ) -> Result<(), ModeChoiceError> {
        for (index, choice) in choices.iter().enumerate() {
            if choices[..index].iter().any(|prior| prior.id == choice.id) {
                return Err(ModeChoiceError::DuplicateIdentity);
            }
        }
        if !choices
            .iter()
            .any(|choice| Some(choice.id) == self.focused_mode)
        {
            self.focused_mode = None;
        }
        self.mode_choices = choices;
        Ok(())
    }

    /// Atomically install the runtime page registry's ordered semantic editor
    /// actions. A new generation drops focus so an old pointer/focus target
    /// cannot resolve against the replacement page.
    pub fn install_editor_surface_catalog(
        &mut self,
        generation: EditorSurfaceGeneration,
        surfaces: Vec<EditorSurface>,
    ) -> Result<(), EditorSurfaceCatalogError> {
        if self.page != MapMakerPage::Editor {
            return Err(EditorSurfaceCatalogError::NotInLoadedEditor);
        }
        for (index, surface) in surfaces.iter().enumerate() {
            if *surface == EditorSurface::MainMenu {
                return Err(EditorSurfaceCatalogError::MainMenuIsNotAnAction);
            }
            if surfaces[..index].contains(surface) {
                return Err(EditorSurfaceCatalogError::DuplicateSurface);
            }
        }
        if self.editor_surface_generation != Some(generation)
            || self
                .focused_editor_surface
                .is_some_and(|focused| !surfaces.contains(&focused))
        {
            self.focused_editor_surface = None;
            self.invalidate_editor_set_list_targets();
            self.map_description_session = None;
            self.briefing_text_session = None;
        }
        self.editor_surface_generation = Some(generation);
        self.editor_surfaces = surfaces;
        Ok(())
    }

    /// Install a runtime-built Weapon Set or Bot Set page as opaque ordered
    /// entries. T1714 supplies the list limits; labels and profile contents
    /// remain with the runtime data provider.
    pub fn install_editor_set_list(
        &mut self,
        generation: EditorSetListGeneration,
        list: EditorSetListKind,
        entries: Vec<EditorSetEntry>,
    ) -> Result<(), EditorSetListError> {
        if self.page != MapMakerPage::Editor || self.editor_surface != list.surface() {
            return Err(EditorSetListError::NotOnProfileList);
        }
        if entries.len() > list.capacity() {
            return Err(EditorSetListError::TooManyEntries);
        }
        for (index, entry) in entries.iter().enumerate() {
            if entries[..index].iter().any(|prior| prior.id == entry.id) {
                return Err(EditorSetListError::DuplicateIdentity);
            }
        }
        if self.editor_set_list_generation != Some(generation)
            || self.editor_set_list_kind != Some(list)
            || self
                .focused_editor_set_entry
                .is_some_and(|focused| !entries.iter().any(|row| row.id == focused))
        {
            self.focused_editor_set_entry = entries.first().map(|row| row.id);
        }
        self.editor_set_list_generation = Some(generation);
        self.editor_set_list_kind = Some(list);
        self.editor_set_entries = entries;
        Ok(())
    }

    /// Install the known semantic controls for the kind-A palette properties
    /// page. Text and layout remain with the runtime page provider.
    pub fn install_palette_properties_catalog(
        &mut self,
        generation: PalettePropertiesGeneration,
        actions: Vec<PalettePropertyAction>,
    ) -> Result<(), PalettePropertiesCatalogError> {
        if self.page != MapMakerPage::Editor
            || self.editor_surface != EditorSurface::PaletteProperties
        {
            return Err(PalettePropertiesCatalogError::NotOnPaletteProperties);
        }
        for (index, action) in actions.iter().enumerate() {
            if actions[..index].contains(action) {
                return Err(PalettePropertiesCatalogError::DuplicateAction);
            }
        }
        if self.palette_properties_generation != Some(generation)
            || self
                .focused_palette_property
                .is_some_and(|focused| !actions.contains(&focused))
        {
            self.focused_palette_property = actions.first().copied();
        }
        self.palette_properties_generation = Some(generation);
        self.palette_property_actions = actions;
        Ok(())
    }

    /// Install ordered opaque palette choices supplied by the page provider.
    /// Labels, layout, original record addresses, and selection effects remain
    /// with their runtime and typed-document owners.
    pub fn install_palette_grid_catalog(
        &mut self,
        generation: PaletteGridGeneration,
        choices: Vec<PaletteGridChoiceId>,
    ) -> Result<(), PaletteGridCatalogError> {
        if self.page != MapMakerPage::Editor || self.editor_surface != EditorSurface::PaletteGrid {
            return Err(PaletteGridCatalogError::NotOnPaletteGrid);
        }
        for (index, choice) in choices.iter().enumerate() {
            if choices[..index].contains(choice) {
                return Err(PaletteGridCatalogError::DuplicateChoice);
            }
        }
        if self.palette_grid_generation != Some(generation)
            || self
                .focused_palette_grid_choice
                .is_some_and(|focused| !choices.contains(&focused))
        {
            self.focused_palette_grid_choice = choices.first().copied();
        }
        self.palette_grid_generation = Some(generation);
        self.palette_grid_choices = choices;
        Ok(())
    }

    /// Install ordered runtime supplied Map Preview choices. Their labels,
    /// layout, and action effects stay with their respective services.
    pub fn install_editor_preview_catalog(
        &mut self,
        generation: EditorPreviewGeneration,
        choices: Vec<EditorPreviewChoiceId>,
    ) -> Result<(), EditorPreviewCatalogError> {
        if self.page != MapMakerPage::Editor || self.editor_surface != EditorSurface::MapPreview {
            return Err(EditorPreviewCatalogError::NotOnMapPreview);
        }
        for (index, choice) in choices.iter().enumerate() {
            if choices[..index].contains(choice) {
                return Err(EditorPreviewCatalogError::DuplicateChoice);
            }
        }
        if self.editor_preview_generation != Some(generation)
            || self
                .focused_editor_preview_choice
                .is_some_and(|focused| !choices.contains(&focused))
        {
            self.focused_editor_preview_choice = choices.first().copied();
        }
        if self.editor_preview_generation != Some(generation)
            || self.editor_preview_selection.is_some_and(|selection| {
                !choices.contains(&EditorPreviewChoiceId::new(u64::from(selection.row.0)))
            })
        {
            self.editor_preview_selection = None;
        }
        self.editor_preview_generation = Some(generation);
        self.editor_preview_choices = choices;
        Ok(())
    }

    fn invalidate_editor_preview_targets(&mut self) {
        self.editor_preview_generation = None;
        self.editor_preview_choices.clear();
        self.focused_editor_preview_choice = None;
        self.editor_preview_selection = None;
    }

    fn invalidate_palette_properties_targets(&mut self) {
        self.palette_properties_generation = None;
        self.palette_property_actions.clear();
        self.focused_palette_property = None;
    }

    fn invalidate_palette_grid_targets(&mut self) {
        self.palette_grid_generation = None;
        self.palette_grid_choices.clear();
        self.focused_palette_grid_choice = None;
    }

    /// Install the original menu's action order from the measured T1714
    /// descriptor. The caller supplies the page generation; no widget IDs or
    /// guest addresses enter the owned menu state.
    pub fn install_original_editor_menu(
        &mut self,
        generation: EditorMenuGeneration,
    ) -> Result<(), EditorMenuCatalogError> {
        self.install_editor_menu_catalog(generation, ORIGINAL_EDITOR_MENU_ACTIONS.to_vec())
    }

    /// Atomically install ordered semantic menu actions. A new generation
    /// invalidates focus and pointer targets from the old page layout.
    pub fn install_editor_menu_catalog(
        &mut self,
        generation: EditorMenuGeneration,
        actions: Vec<EditorMenuAction>,
    ) -> Result<(), EditorMenuCatalogError> {
        if self.page != MapMakerPage::Editor || self.editor_surface != EditorSurface::MainMenu {
            return Err(EditorMenuCatalogError::NotAtEditorMenu);
        }
        for (index, action) in actions.iter().enumerate() {
            if actions[..index].contains(action) {
                return Err(EditorMenuCatalogError::DuplicateAction);
            }
        }
        if self.editor_menu_generation != Some(generation)
            || self
                .focused_editor_menu_action
                .is_some_and(|focused| !actions.contains(&focused))
        {
            self.focused_editor_menu_action = None;
            if self.editor_confirmation.is_some() {
                self.focused_editor_confirmation_choice = Some(EditorConfirmationChoice::Cancel);
            }
        }
        self.editor_menu_generation = Some(generation);
        self.editor_menu_actions = actions;
        Ok(())
    }

    fn clear_editor_surface_catalog(&mut self) {
        self.editor_surface_generation = None;
        self.editor_surfaces.clear();
        self.focused_editor_surface = None;
        self.invalidate_palette_properties_targets();
        self.invalidate_palette_grid_targets();
        self.invalidate_editor_preview_targets();
        self.invalidate_editor_set_list_targets();
        self.editor_menu_generation = None;
        self.editor_menu_actions.clear();
        self.focused_editor_menu_action = None;
        self.editor_confirmation = None;
        self.focused_editor_confirmation_choice = None;
        self.map_description_session = None;
        self.briefing_text_session = None;
    }

    fn invalidate_editor_set_list_targets(&mut self) {
        self.editor_set_list_generation = None;
        self.editor_set_list_kind = None;
        self.editor_set_entries.clear();
        self.focused_editor_set_entry = None;
    }

    /// Reduce one typed page event. Input adapters provide stable row identity;
    /// configuration values come only from the installed runtime table.
    pub fn dispatch(&mut self, command: MapMakerUiCommand) -> MapMakerUiUpdate {
        match (self.page, command) {
            (page, MapMakerUiCommand::BeginEditor)
                if (page != MapMakerPage::Editor
                    || self.editor_surface != EditorSurface::MainMenu)
                    && self.pending_map_load.is_none()
                    && self.storage_progress.is_none() =>
            {
                self.page = MapMakerPage::Editor;
                self.editor_surface = EditorSurface::MainMenu;
                self.editor_surface_history.clear();
                self.active_save_map_target = None;
                self.active_map_description_target = None;
                self.active_briefing_target = None;
                self.invalidate_map_settings_targets();
                self.invalidate_editor_settings_targets();
                self.clear_editor_surface_catalog();
                self.invalidate_editor_preview_targets();
                self.invalidate_palette_properties_targets();
                self.invalidate_palette_grid_targets();
                self.invalidate_editor_set_list_targets();
                self.editor_menu_generation = None;
                self.editor_menu_actions.clear();
                self.focused_editor_menu_action = None;
                self.editor_confirmation = None;
                self.focused_editor_confirmation_choice = None;
                MapMakerUiUpdate::PageChanged(MapMakerPage::Editor)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::BeginEditor) => MapMakerUiUpdate::Ignored,
            (MapMakerPage::EditorMenu, MapMakerUiCommand::OpenModeSelection) => {
                self.page = MapMakerPage::ModeSelection;
                MapMakerUiUpdate::PageChanged(self.page)
            }
            (MapMakerPage::EditorMenu | MapMakerPage::Editor, MapMakerUiCommand::OpenStorage) => {
                self.invalidate_map_settings_targets();
                self.invalidate_editor_settings_targets();
                self.storage_return_page = self.page;
                self.page = MapMakerPage::StorageSource;
                self.focused_storage_source = None;
                MapMakerUiUpdate::PageChanged(self.page)
            }
            (MapMakerPage::StorageSource, MapMakerUiCommand::FocusStorageSource(source)) => {
                self.focused_storage_source = Some(source);
                MapMakerUiUpdate::StorageSourceFocused(source)
            }
            (MapMakerPage::StorageSource, MapMakerUiCommand::SelectLocalStorage) => {
                self.focused_storage_source = Some(MapStorageSource::LocalStorage);
                self.page = MapMakerPage::MapList;
                MapMakerUiUpdate::PageChanged(self.page)
            }
            (MapMakerPage::StorageSource, MapMakerUiCommand::SelectSampleMaps) => {
                self.focused_storage_source = Some(MapStorageSource::SampleMaps);
                self.page = MapMakerPage::MapList;
                MapMakerUiUpdate::PageChanged(self.page)
            }
            (MapMakerPage::MapList, MapMakerUiCommand::FocusMap(id))
                if self.pending_map_load.is_none()
                    && self.map_rows.iter().any(|row| row.id == id) =>
            {
                self.focused_map = Some(id);
                MapMakerUiUpdate::MapFocused(id)
            }
            (MapMakerPage::MapList, MapMakerUiCommand::SelectMap(id))
                if self.pending_map_load.is_none()
                    && self.map_rows.iter().any(|row| row.id == id) =>
            {
                self.focused_map = Some(id);
                self.next_map_load_id = self.next_map_load_id.wrapping_add(1).max(1);
                let request = MapLoadRequest {
                    id: MapLoadRequestId(self.next_map_load_id),
                    row: id,
                };
                self.pending_map_load = Some(request);
                MapMakerUiUpdate::LoadRequested(request)
            }
            (MapMakerPage::MapList, MapMakerUiCommand::CompleteMapLoad { request, outcome })
                if self.pending_map_load.map(|pending| pending.id) == Some(request) =>
            {
                let completed = self.pending_map_load.take().expect("matching pending load");
                match outcome {
                    MapLoadOutcome::Loaded => {
                        self.page = MapMakerPage::Editor;
                        self.editor_surface = EditorSurface::MainMenu;
                        self.editor_surface_history.clear();
                        self.active_save_map_target = None;
                        self.active_map_description_target = None;
                        self.active_briefing_target = None;
                        self.invalidate_map_settings_targets();
                        self.invalidate_editor_settings_targets();
                        self.clear_editor_surface_catalog();
                        MapMakerUiUpdate::LoadCompleted(completed.row)
                    }
                    MapLoadOutcome::Failed => MapMakerUiUpdate::LoadFailed(completed.row),
                }
            }
            (
                MapMakerPage::EditorMenu
                | MapMakerPage::Editor
                | MapMakerPage::StorageSource
                | MapMakerPage::MapList,
                MapMakerUiCommand::OpenStorageProgress(operation),
            ) if self.pending_map_load.is_none() => {
                self.next_storage_progress_id =
                    self.next_storage_progress_id.wrapping_add(1).max(1);
                let id = StorageProgressId(self.next_storage_progress_id);
                self.storage_progress_return_page = Some(self.page);
                self.storage_progress = Some(StorageProgress {
                    id,
                    operation,
                    outcome: StorageProgressOutcome::Pending,
                });
                self.page = MapMakerPage::StorageProgress;
                MapMakerUiUpdate::StorageProgressOpened(id)
            }
            (
                MapMakerPage::StorageProgress,
                MapMakerUiCommand::CompleteStorageProgress { progress, outcome },
            ) if outcome != StorageProgressOutcome::Pending
                && self.storage_progress.is_some_and(|active| {
                    active.id == progress && active.outcome == StorageProgressOutcome::Pending
                }) =>
            {
                let active = self.storage_progress.as_mut().expect("matching progress");
                active.outcome = outcome;
                MapMakerUiUpdate::StorageProgressCompleted(*active)
            }
            (MapMakerPage::ModeSelection, MapMakerUiCommand::FocusMode(id))
                if self.mode_choices.iter().any(|choice| choice.id == id) =>
            {
                self.focused_mode = Some(id);
                MapMakerUiUpdate::ModeFocused(id)
            }
            (MapMakerPage::ModeSelection, MapMakerUiCommand::SelectMode(id)) => {
                let Some(choice) = self.mode_choices.iter().find(|choice| choice.id == id) else {
                    return MapMakerUiUpdate::Ignored;
                };
                self.focused_mode = Some(id);
                self.selected_mode_id = Some(id);
                let mode = choice.configuration.clone();
                self.selected_mode = Some(mode);
                self.preview_row = None;
                self.pending_preview_gate = None;
                self.pending_preview_warning = None;
                self.preview_capacity_blocked = false;
                self.page = MapMakerPage::ModeSettings;
                MapMakerUiUpdate::PageChanged(self.page)
            }
            (MapMakerPage::ModeSettings, MapMakerUiCommand::OpenPreview) => {
                let Some(row) = self
                    .selected_mode
                    .as_ref()
                    .map(|mode| mode.settings().preview.row)
                else {
                    return MapMakerUiUpdate::Ignored;
                };
                self.preview_row = Some(row);
                self.pending_preview_gate = None;
                self.pending_preview_warning = None;
                self.preview_capacity_blocked = false;
                self.page = MapMakerPage::Preview;
                MapMakerUiUpdate::PreviewOpened(row)
            }
            (MapMakerPage::Preview, MapMakerUiCommand::StartPreview)
                if self.pending_preview_gate.is_none()
                    && self.pending_preview_warning.is_none()
                    && !self.preview_capacity_blocked =>
            {
                let Some(row) = self.preview_row else {
                    return MapMakerUiUpdate::Ignored;
                };
                self.next_preview_gate_id = self.next_preview_gate_id.wrapping_add(1).max(1);
                let request = PreviewStartRequest {
                    id: PreviewGateId(self.next_preview_gate_id),
                    row,
                };
                self.pending_preview_gate = Some(request);
                MapMakerUiUpdate::PreviewStartCheckRequested(request)
            }
            (
                MapMakerPage::Preview,
                MapMakerUiCommand::CompletePreviewStartGate { request, gate },
            ) if self
                .pending_preview_gate
                .is_some_and(|pending| pending.id == request)
                && self.pending_preview_warning.is_none()
                && !self.preview_capacity_blocked =>
            {
                let pending = self.pending_preview_gate.take().expect("validated request");
                let row = pending.row;
                match gate {
                    PreviewStartGate::CapacityBlocked => {
                        self.preview_capacity_blocked = true;
                        MapMakerUiUpdate::PreviewCapacityBlocked {
                            row,
                            text: PREVIEW_CAPACITY_TEXT,
                        }
                    }
                    PreviewStartGate::ContinueAnyway(text) => {
                        self.pending_preview_warning = Some(text);
                        MapMakerUiUpdate::PreviewWarningOpened { row, text }
                    }
                    PreviewStartGate::Allowed => MapMakerUiUpdate::PreviewLaunchRequested(row),
                }
            }
            (MapMakerPage::Preview, MapMakerUiCommand::ResolvePreviewWarning(choice)) => {
                let Some(_text) = self.pending_preview_warning.take() else {
                    return MapMakerUiUpdate::Ignored;
                };
                let Some(row) = self.preview_row else {
                    return MapMakerUiUpdate::Ignored;
                };
                if choice == PreviewWarningChoice::Continue {
                    MapMakerUiUpdate::PreviewLaunchRequested(row)
                } else {
                    MapMakerUiUpdate::PreviewWarningDismissed(row)
                }
            }
            (MapMakerPage::Editor, MapMakerUiCommand::FocusEditorSurface(target))
                if self.contains_editor_surface_target(target) =>
            {
                let surface = target.surface;
                self.focused_editor_surface = Some(surface);
                MapMakerUiUpdate::EditorSurfaceFocused(surface)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::OpenEditorSurface(target))
                if self.contains_editor_surface_target(target)
                    && target.surface != self.editor_surface =>
            {
                let surface = target.surface;
                self.editor_surface_history.push(self.editor_surface);
                self.editor_surface = surface;
                self.active_save_map_target = (surface == EditorSurface::SaveMap).then_some(target);
                self.active_map_description_target =
                    (surface == EditorSurface::MapDescription).then_some(target);
                self.active_briefing_target =
                    (surface == EditorSurface::Briefing).then_some(target);
                self.invalidate_map_settings_targets();
                self.invalidate_editor_settings_targets();
                self.clear_editor_surface_catalog();
                if surface == EditorSurface::SaveMap {
                    MapMakerUiUpdate::EditorSaveMapRouteOpened
                } else {
                    MapMakerUiUpdate::EditorSurfaceChanged(surface)
                }
            }
            (MapMakerPage::Editor, MapMakerUiCommand::FocusEditorMenuAction(target))
                if self.contains_editor_menu_target(target) =>
            {
                self.focused_editor_menu_action = Some(target.action);
                MapMakerUiUpdate::EditorMenuActionFocused(target.action)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::ActivateEditorMenuAction(target))
                if self.contains_editor_menu_target(target) =>
            {
                if self
                    .editor_history_availability
                    .is_some_and(|history| match target.action {
                        EditorMenuAction::Undo => !history.can_undo,
                        EditorMenuAction::Redo => !history.can_redo,
                        _ => false,
                    })
                {
                    return MapMakerUiUpdate::Ignored;
                }
                match target.action {
                    EditorMenuAction::ClearMap => {
                        self.editor_confirmation = Some(EditorConfirmation::ClearMap);
                        // A destructive choice starts on Cancel until the user moves focus.
                        self.focused_editor_confirmation_choice =
                            Some(EditorConfirmationChoice::Cancel);
                        MapMakerUiUpdate::EditorConfirmationOpened(EditorConfirmation::ClearMap)
                    }
                    EditorMenuAction::QuitMapMaker => {
                        MapMakerUiUpdate::EditorMenuActionActivated(EditorMenuAction::QuitMapMaker)
                    }
                    EditorMenuAction::Controls => {
                        let Some(surface) = self.editor_surface_target(EditorSurface::Controls)
                        else {
                            return MapMakerUiUpdate::EditorMenuActionActivated(
                                EditorMenuAction::Controls,
                            );
                        };
                        match self.dispatch(MapMakerUiCommand::OpenEditorSurface(surface)) {
                            MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::Controls) => {
                                MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::Controls)
                            }
                            _ => MapMakerUiUpdate::EditorMenuActionActivated(
                                EditorMenuAction::Controls,
                            ),
                        }
                    }
                    EditorMenuAction::Undo => {
                        MapMakerUiUpdate::EditorHistoryRequested(EditorHistoryOperation::Undo)
                    }
                    EditorMenuAction::Redo => {
                        MapMakerUiUpdate::EditorHistoryRequested(EditorHistoryOperation::Redo)
                    }
                    EditorMenuAction::CenterMap => {
                        MapMakerUiUpdate::EditorViewportOperationRequested(
                            EditorViewportOperation::CenterMap,
                        )
                    }
                    action => MapMakerUiUpdate::EditorMenuActionActivated(action),
                }
            }
            (MapMakerPage::Editor, MapMakerUiCommand::FocusEditorConfirmationChoice(target))
                if self.contains_editor_confirmation_target(target) =>
            {
                self.focused_editor_confirmation_choice = Some(target.choice);
                MapMakerUiUpdate::EditorConfirmationFocused(target.choice)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::ObserveQuitMapMakerConfirmation)
                if self.editor_surface == EditorSurface::MainMenu
                    && self.editor_confirmation.is_none()
                    && self.focused_editor_menu_action == Some(EditorMenuAction::QuitMapMaker) =>
            {
                self.editor_confirmation = Some(EditorConfirmation::QuitMapMaker);
                self.focused_editor_confirmation_choice = Some(EditorConfirmationChoice::Cancel);
                MapMakerUiUpdate::EditorConfirmationOpened(EditorConfirmation::QuitMapMaker)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::ResolveEditorConfirmationChoice(target))
                if self.contains_editor_confirmation_target(target) =>
            {
                let confirmation = self.editor_confirmation.take().expect("validated dialog");
                self.focused_editor_confirmation_choice = None;
                match (confirmation, target.choice) {
                    (EditorConfirmation::ClearMap, EditorConfirmationChoice::Accept) => {
                        MapMakerUiUpdate::EditorMenuActionActivated(EditorMenuAction::ClearMap)
                    }
                    (EditorConfirmation::QuitMapMaker, EditorConfirmationChoice::Accept) => {
                        MapMakerUiUpdate::EditorMenuActionActivated(EditorMenuAction::QuitMapMaker)
                    }
                    (confirmation, EditorConfirmationChoice::Cancel) => {
                        MapMakerUiUpdate::EditorConfirmationDismissed(confirmation)
                    }
                }
            }
            (MapMakerPage::Editor, MapMakerUiCommand::FocusMapSetting(target))
                if self.contains_map_setting_target(target) =>
            {
                self.focused_map_setting = Some(target.field);
                self.focused_map_settings_entry = Some(MapSettingsEntry::Field(target.field));
                MapMakerUiUpdate::MapSettingFocused(target.field)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::FocusMapSettingsEntry(target))
                if self.contains_map_settings_entry_target(target) =>
            {
                self.focused_map_settings_entry = Some(target.entry);
                self.focused_map_setting = match target.entry {
                    MapSettingsEntry::Field(field) => Some(field),
                    MapSettingsEntry::Action(_) => None,
                };
                MapMakerUiUpdate::MapSettingsEntryFocused(target.entry)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::ActivateMapSettingsAction(target))
                if self.contains_map_settings_entry_target(target) =>
            {
                match target.entry {
                    MapSettingsEntry::Action(action) => {
                        self.focused_map_settings_entry = Some(target.entry);
                        self.focused_map_setting = None;
                        let Some(surface) = action.destination_surface() else {
                            return MapMakerUiUpdate::MapSettingsActionRequested(action);
                        };
                        let Some(surface_target) = self.editor_surface_target(surface) else {
                            return MapMakerUiUpdate::MapSettingsActionRequested(action);
                        };
                        if matches!(
                            self.dispatch(MapMakerUiCommand::OpenEditorSurface(surface_target)),
                            MapMakerUiUpdate::EditorSurfaceChanged(opened) if opened == surface
                        ) {
                            MapMakerUiUpdate::MapSettingsActionOpened { action, surface }
                        } else {
                            MapMakerUiUpdate::MapSettingsActionRequested(action)
                        }
                    }
                    MapSettingsEntry::Field(_) => MapMakerUiUpdate::Ignored,
                }
            }
            (MapMakerPage::Editor, MapMakerUiCommand::SelectMapSetting { target, choice })
                if self.contains_map_setting_target(target) =>
            {
                let Some(row) = self
                    .map_settings
                    .iter_mut()
                    .find(|row| row.field == target.field)
                else {
                    return MapMakerUiUpdate::Ignored;
                };
                if !row.choices.contains(&choice) {
                    return MapMakerUiUpdate::Ignored;
                }
                row.selected = Some(choice);
                self.focused_map_setting = Some(target.field);
                self.focused_map_settings_entry = Some(MapSettingsEntry::Field(target.field));
                MapMakerUiUpdate::MapSettingChanged {
                    field: target.field,
                    choice,
                }
            }
            (MapMakerPage::Editor, MapMakerUiCommand::FocusEditorSetting(target))
                if self.contains_editor_setting_target(target) =>
            {
                self.focused_editor_setting = Some(target.field);
                MapMakerUiUpdate::EditorSettingFocused(target.field)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::SelectEditorSetting { target, choice })
                if self.contains_editor_setting_target(target) =>
            {
                let Some(row) = self
                    .editor_settings
                    .iter_mut()
                    .find(|row| row.field == target.field)
                else {
                    return MapMakerUiUpdate::Ignored;
                };
                if !row.choices.contains(&choice) {
                    return MapMakerUiUpdate::Ignored;
                }
                row.selected = Some(choice);
                self.focused_editor_setting = Some(target.field);
                MapMakerUiUpdate::EditorSettingChanged {
                    field: target.field,
                    choice,
                }
            }
            (MapMakerPage::Editor, MapMakerUiCommand::FocusPaletteProperty(target))
                if self.contains_palette_property_target(target) =>
            {
                self.focused_palette_property = Some(target.action);
                MapMakerUiUpdate::PalettePropertyFocused(target.action)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::ActivatePaletteProperty(target))
                if self.contains_palette_property_target(target) =>
            {
                self.focused_palette_property = Some(target.action);
                if target.action == PalettePropertyAction::ChangePalette {
                    self.editor_surface_history.push(self.editor_surface);
                    self.editor_surface = EditorSurface::PaletteGrid;
                    self.active_save_map_target = None;
                    self.active_map_description_target = None;
                    self.active_briefing_target = None;
                    self.invalidate_map_settings_targets();
                    self.invalidate_editor_settings_targets();
                    self.clear_editor_surface_catalog();
                    MapMakerUiUpdate::PalettePageReentered
                } else {
                    MapMakerUiUpdate::PalettePropertyActivated(target.action)
                }
            }
            (MapMakerPage::Editor, MapMakerUiCommand::FocusPaletteGridChoice(target))
                if self.contains_palette_grid_choice_target(target) =>
            {
                self.focused_palette_grid_choice = Some(target.choice);
                MapMakerUiUpdate::PaletteGridChoiceFocused(target.choice)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::ActivatePaletteGridChoice(target))
                if self.contains_palette_grid_choice_target(target) =>
            {
                self.focused_palette_grid_choice = Some(target.choice);
                MapMakerUiUpdate::PaletteGridChoiceActivated(target.choice)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::FocusEditorPreviewChoice(target))
                if self.contains_editor_preview_choice_target(target) =>
            {
                self.focused_editor_preview_choice = Some(target.choice);
                MapMakerUiUpdate::EditorPreviewChoiceFocused(target.choice)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::ActivateEditorPreviewChoice(target))
                if self.contains_editor_preview_choice_target(target) =>
            {
                let Ok(row) = u8::try_from(target.choice.get()) else {
                    return MapMakerUiUpdate::Ignored;
                };
                let Some(selection) = PreviewSelection::from_row(PreviewRow(row)) else {
                    return MapMakerUiUpdate::Ignored;
                };
                self.focused_editor_preview_choice = Some(target.choice);
                self.editor_preview_selection = Some(selection);
                MapMakerUiUpdate::EditorPreviewSelectionRequested(selection)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::FocusEditorSetEntry(target))
                if self.contains_editor_set_entry_target(target) =>
            {
                self.focused_editor_set_entry = Some(target.entry);
                MapMakerUiUpdate::EditorSetEntryFocused(target.entry)
            }
            (MapMakerPage::Editor, MapMakerUiCommand::ActivateEditorSetEntry(target))
                if self.contains_editor_set_entry_target(target) =>
            {
                self.focused_editor_set_entry = Some(target.entry);
                MapMakerUiUpdate::EditorSetEntryActivated {
                    list: target.list,
                    entry: target.entry,
                }
            }
            (MapMakerPage::Editor, MapMakerUiCommand::Back) => {
                if let Some(confirmation) = self.editor_confirmation.take() {
                    self.focused_editor_confirmation_choice = None;
                    MapMakerUiUpdate::EditorConfirmationDismissed(confirmation)
                } else if let Some(surface) = self.editor_surface_history.pop() {
                    self.editor_surface = surface;
                    self.active_save_map_target = None;
                    self.active_map_description_target = None;
                    self.active_briefing_target = None;
                    self.invalidate_map_settings_targets();
                    self.invalidate_editor_settings_targets();
                    self.clear_editor_surface_catalog();
                    MapMakerUiUpdate::EditorSurfaceChanged(surface)
                } else {
                    MapMakerUiUpdate::Ignored
                }
            }
            (MapMakerPage::Preview, MapMakerUiCommand::Back) => {
                self.pending_preview_gate = None;
                if self.preview_capacity_blocked {
                    self.preview_capacity_blocked = false;
                    self.preview_row.map_or(MapMakerUiUpdate::Ignored, |row| {
                        MapMakerUiUpdate::PreviewCapacityDismissed(row)
                    })
                } else if self.pending_preview_warning.is_some() {
                    self.pending_preview_warning = None;
                    self.preview_row.map_or(MapMakerUiUpdate::Ignored, |row| {
                        MapMakerUiUpdate::PreviewWarningDismissed(row)
                    })
                } else {
                    self.pending_preview_warning = None;
                    self.page = MapMakerPage::ModeSettings;
                    MapMakerUiUpdate::PageChanged(self.page)
                }
            }
            (MapMakerPage::ModeSettings, MapMakerUiCommand::Back) => {
                self.page = MapMakerPage::ModeSelection;
                MapMakerUiUpdate::PageChanged(self.page)
            }
            (MapMakerPage::ModeSelection, MapMakerUiCommand::Back) => {
                self.page = MapMakerPage::EditorMenu;
                MapMakerUiUpdate::PageChanged(self.page)
            }
            (MapMakerPage::MapList, MapMakerUiCommand::Back) => {
                if let Some(request) = self.pending_map_load.take() {
                    MapMakerUiUpdate::LoadCancelled(request.id)
                } else {
                    self.page = MapMakerPage::StorageSource;
                    MapMakerUiUpdate::PageChanged(self.page)
                }
            }
            (MapMakerPage::StorageSource, MapMakerUiCommand::Back) => {
                self.page = self.storage_return_page;
                MapMakerUiUpdate::PageChanged(self.page)
            }
            (MapMakerPage::StorageProgress, MapMakerUiCommand::Back)
                if self.storage_progress.is_some_and(|progress| {
                    progress.outcome != StorageProgressOutcome::Pending
                }) =>
            {
                let Some(progress) = self.storage_progress.take() else {
                    return MapMakerUiUpdate::Ignored;
                };
                self.page = self
                    .storage_progress_return_page
                    .take()
                    .unwrap_or(MapMakerPage::EditorMenu);
                MapMakerUiUpdate::StorageProgressDismissed(progress.operation)
            }
            _ => MapMakerUiUpdate::Ignored,
        }
    }

    /// Enter preview for the selected typed table row. The row is not treated
    /// as the mode, launch variant, or launch game type.
    pub fn open_preview(&mut self) -> Option<PreviewRow> {
        if self.page != MapMakerPage::ModeSettings {
            return None;
        }
        let row = self.selected_mode.as_ref()?.settings().preview.row;
        self.page = MapMakerPage::Preview;
        self.preview_row = Some(row);
        Some(row)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EditorExperience, EditorSettingsEdit, EditorSettingsEditEffect, EditorSettingsEditPhase,
        EditorSettingsGeneration, EditorSurface, MapMakerPage, MapMakerUiCommand, MapMakerUiState,
        MapMakerUiUpdate, MapSettingsAction, ModeChoice, ModeChoiceError, ModeChoiceId,
        PalettePropertyAction, PalettePropertyTextId,
    };
    use crate::disabled_modes::{
        DisabledModeId, DisabledModeRow, DisabledModesGeneration, DisabledModesUpdate,
    };
    use crate::modes::{
        AssaultModeConfiguration, EditorKind, GameType, LaunchVariant, MapMode, ModeConfiguration,
        ModeSettings, PreviewRow, PreviewSelection, ThemeId,
    };
    use crate::overlays::{HintAnchorTransform, MapMakerOverlayRamp, MapMakerOverlayVisibility};

    #[test]
    fn disabled_mode_values_belong_to_shared_state_across_editor_page_changes() {
        let mut state = MapMakerUiState::default();
        let id = DisabledModeId::new(7);
        state
            .disabled_modes_mut()
            .install(
                DisabledModesGeneration::new(1),
                vec![DisabledModeRow {
                    id,
                    disabled: false,
                }],
            )
            .unwrap();
        let target = state.disabled_modes().target(id).unwrap();
        assert_eq!(
            state.disabled_modes_mut().toggle(target),
            DisabledModesUpdate::Toggled { id, disabled: true }
        );

        state.dispatch(MapMakerUiCommand::BeginEditor);

        assert!(state.disabled_modes().rows()[0].disabled);
        assert!(state.snapshot().disabled_modes.rows()[0].disabled);
    }

    #[test]
    fn assault_assignment_focus_is_part_of_the_shared_ui_snapshot() {
        use crate::assault_phase_assignment::{
            AssaultPhaseAssignmentAction, AssaultPhaseAssignmentGeneration,
            AssaultPhaseAssignmentState,
        };

        let mut state = MapMakerUiState::default();
        let generation = AssaultPhaseAssignmentGeneration::new(9);
        let mut assignment = AssaultPhaseAssignmentState::new(generation, false);
        let target = assignment.target(AssaultPhaseAssignmentAction::SelectStartPoint);
        assignment.dispatch(
            crate::assault_phase_assignment::AssaultPhaseAssignmentCommand::Focus(target),
        );

        state.set_assault_phase_assignment(Some(assignment));

        assert_eq!(
            state.snapshot().assault_phase_assignment.unwrap().focused(),
            Some(AssaultPhaseAssignmentAction::SelectStartPoint)
        );
        state.set_assault_phase_assignment(None);
        assert!(state.snapshot().assault_phase_assignment.is_none());
    }

    #[test]
    fn assault_start_point_choices_are_part_of_the_shared_ui_snapshot() {
        use crate::assault_start_point_picker::{
            StartPointChoiceId, StartPointPickerCommand, StartPointPickerGeneration,
            StartPointPickerState, StartPointPickerUpdate,
        };

        let first = StartPointChoiceId::new(0xabc);
        let second = StartPointChoiceId::new(0xdef);
        let generation = StartPointPickerGeneration::new(12);
        let mut picker = StartPointPickerState::new(generation, &[first, second], false).unwrap();
        let target = picker.target(second).unwrap();
        assert_eq!(
            picker.dispatch(StartPointPickerCommand::Focus(target)),
            StartPointPickerUpdate::Focused(second)
        );

        let mut state = MapMakerUiState::default();
        state.set_assault_start_point_picker(Some(picker));
        let snapshot = state.snapshot();
        let picker = snapshot.assault_start_point_picker.unwrap();
        assert_eq!(picker.choices(), &[first, second]);
        assert_eq!(picker.focused(), Some(second));

        state.set_assault_start_point_picker(None);
        assert!(state.snapshot().assault_start_point_picker.is_none());
    }

    #[test]
    fn assault_start_point_choice_handoff_is_generation_checked_and_rust_owned() {
        use crate::assault_phase_assignment::{
            AssaultPhaseAssignmentAction, AssaultPhaseAssignmentGeneration,
            AssaultPhaseAssignmentState, AssaultPhaseAssignmentUpdate,
        };
        use crate::assault_start_point_picker::{
            StartPointChoiceId, StartPointPickerGeneration, StartPointPickerState,
        };

        let first = StartPointChoiceId::new(0xabc);
        let second = StartPointChoiceId::new(0xdef);
        let mut state = MapMakerUiState::default();
        let phase_generation = AssaultPhaseAssignmentGeneration::new(9);
        let mut assignment = AssaultPhaseAssignmentState::new(phase_generation, false);
        state.set_assault_phase_assignment(Some(assignment));
        assert_eq!(
            state.dispatch_assault_phase_assignment(
                crate::assault_phase_assignment::AssaultPhaseAssignmentCommand::Activate(
                    assignment.target(AssaultPhaseAssignmentAction::SelectStartPoint)
                )
            ),
            AssaultPhaseAssignmentUpdate::StartPointSelectionRequested
        );

        state.observe_assault_phase_page_change(false);
        assert!(
            state
                .assault_phase_assignment()
                .unwrap()
                .awaiting_start_point_selection()
        );

        let child_generation = StartPointPickerGeneration::new(12);
        state.set_assault_start_point_picker(Some(
            StartPointPickerState::new(child_generation, &[first, second], false).unwrap(),
        ));
        assert_eq!(
            state.complete_assault_start_point_selection(
                StartPointPickerGeneration::new(11),
                second
            ),
            AssaultPhaseAssignmentUpdate::Ignored
        );
        assert_eq!(
            state.complete_assault_start_point_selection(
                child_generation,
                StartPointChoiceId::new(77)
            ),
            AssaultPhaseAssignmentUpdate::Ignored
        );
        assert!(state.assault_start_point_picker().is_some());

        assert_eq!(
            state.complete_assault_start_point_selection(child_generation, second),
            AssaultPhaseAssignmentUpdate::StartPointAssigned(second)
        );
        assert!(state.assault_start_point_picker().is_none());
        assert!(state.assault_child_return_pending());
        state.observe_assault_phase_page_change(true);
        assert!(!state.assault_child_return_pending());
        assert_eq!(
            state
                .assault_phase_assignment()
                .unwrap()
                .selected_start_point(),
            Some(second)
        );

        assert_eq!(
            state.dispatch_assault_phase_assignment(
                crate::assault_phase_assignment::AssaultPhaseAssignmentCommand::Activate(
                    assignment.target(AssaultPhaseAssignmentAction::SelectStartPoint)
                )
            ),
            AssaultPhaseAssignmentUpdate::StartPointSelectionRequested
        );
        let next_generation = StartPointPickerGeneration::new(13);
        state.set_assault_start_point_picker(Some(
            StartPointPickerState::new(next_generation, &[first], false).unwrap(),
        ));
        state.observe_assault_phase_page_change(false);
        assert_eq!(
            state.cancel_assault_start_point_selection(child_generation),
            AssaultPhaseAssignmentUpdate::Ignored
        );
        assert_eq!(
            state.cancel_assault_start_point_selection(next_generation),
            AssaultPhaseAssignmentUpdate::StartPointSelectionCancelled
        );
        assert!(state.assault_start_point_picker().is_none());
        state.observe_assault_phase_page_change(true);
        assert_eq!(
            state
                .assault_phase_assignment()
                .unwrap()
                .selected_start_point(),
            Some(second)
        );
        state.observe_assault_phase_page_change(false);
        assert!(state.assault_phase_assignment().is_none());
    }

    #[test]
    fn briefing_row_resolves_to_a_typed_editor_surface() {
        assert_eq!(
            MapSettingsAction::Briefing.destination_surface(),
            Some(EditorSurface::Briefing)
        );
    }

    #[test]
    fn briefing_text_session_is_rust_owned_and_page_generation_checked() {
        use super::{EditorSurfaceGeneration, MapMakerUiCommand as C, MapMakerUiUpdate as U};
        use crate::description::DescriptionCommand;

        let mut state = MapMakerUiState::default();
        state.dispatch(C::BeginEditor);
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(18),
                vec![EditorSurface::Briefing],
            )
            .unwrap();
        let surface = state
            .editor_surface_target(EditorSurface::Briefing)
            .unwrap();
        assert_eq!(
            state.dispatch(C::OpenEditorSurface(surface)),
            U::EditorSurfaceChanged(EditorSurface::Briefing)
        );

        let field = state
            .install_briefing_text_session(surface, "brief", 12)
            .unwrap();
        let view = state.briefing_text_view().unwrap();
        assert_eq!(view.surface, surface);
        assert_eq!(view.target, field);
        assert_eq!(view.text, "brief");
        assert_eq!(view.byte_limit, 12);
        assert_eq!(
            state.apply_briefing_text_command(surface, field, DescriptionCommand::Insert('!')),
            Ok(super::BriefingTextOutcome::Changed)
        );
        assert_eq!(
            state.snapshot().briefing_text_session.unwrap().text,
            "brief!"
        );
        assert_eq!(
            state.apply_briefing_text_command(surface, field, DescriptionCommand::Commit),
            Ok(super::BriefingTextOutcome::CommitRequested(
                super::BriefingTextCommit {
                    target: field,
                    text: "brief!".into(),
                }
            ))
        );

        let wrong_surface = super::EditorSurfaceTarget::new(
            super::EditorSurfaceGeneration::new(surface.generation().get()),
            EditorSurface::MapDescription,
        );
        assert_eq!(
            state.apply_briefing_text_command(wrong_surface, field, DescriptionCommand::Commit),
            Err(crate::description::DescriptionError::StaleTarget)
        );
    }

    #[test]
    fn controls_menu_action_opens_the_registered_owned_surface() {
        use super::{EditorMenuAction, EditorMenuGeneration, EditorSurfaceGeneration};

        let mut state = MapMakerUiState::default();
        state.dispatch(MapMakerUiCommand::BeginEditor);
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(5),
                vec![EditorSurface::Controls],
            )
            .unwrap();
        state
            .install_original_editor_menu(EditorMenuGeneration::new(5))
            .unwrap();
        let controls = state
            .editor_menu_target(EditorMenuAction::Controls)
            .unwrap();
        state.dispatch(MapMakerUiCommand::FocusEditorMenuAction(controls));

        assert_eq!(
            state.dispatch(MapMakerUiCommand::ActivateEditorMenuAction(controls)),
            MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::Controls)
        );
        assert_eq!(state.editor_surface(), EditorSurface::Controls);
    }

    #[test]
    fn begin_editor_installs_no_guest_identity_and_reentry_invalidates_menu_targets() {
        use super::{EditorMenuAction, EditorMenuGeneration, ORIGINAL_EDITOR_MENU_ACTIONS};

        let mut state = MapMakerUiState::default();
        assert_eq!(
            state.dispatch(MapMakerUiCommand::BeginEditor),
            MapMakerUiUpdate::PageChanged(MapMakerPage::Editor)
        );
        let first = EditorMenuGeneration::new(1);
        state.install_original_editor_menu(first).unwrap();
        let stale = state
            .editor_menu_target(EditorMenuAction::MapSettings)
            .unwrap();
        assert_eq!(state.editor_menu_actions(), ORIGINAL_EDITOR_MENU_ACTIONS);

        assert_eq!(
            state.dispatch(MapMakerUiCommand::BeginEditor),
            MapMakerUiUpdate::Ignored
        );
        assert_eq!(
            state.dispatch(MapMakerUiCommand::OpenStorage),
            MapMakerUiUpdate::PageChanged(MapMakerPage::StorageSource)
        );
        assert_eq!(
            state.dispatch(MapMakerUiCommand::BeginEditor),
            MapMakerUiUpdate::PageChanged(MapMakerPage::Editor)
        );
        let second = EditorMenuGeneration::new(2);
        state.install_original_editor_menu(second).unwrap();
        assert!(!state.contains_editor_menu_target(stale));
    }

    #[test]
    fn quit_map_confirmation_is_opened_only_after_runtime_prompt_observation() {
        use super::{
            EditorConfirmation, EditorConfirmationChoice, EditorMenuAction, EditorMenuGeneration,
        };

        let mut state = MapMakerUiState::default();
        state.dispatch(MapMakerUiCommand::BeginEditor);
        state
            .install_original_editor_menu(EditorMenuGeneration::new(44))
            .unwrap();
        let quit = state
            .editor_menu_target(EditorMenuAction::QuitMapMaker)
            .unwrap();
        assert_eq!(
            state.dispatch(MapMakerUiCommand::FocusEditorMenuAction(quit)),
            MapMakerUiUpdate::EditorMenuActionFocused(EditorMenuAction::QuitMapMaker)
        );
        assert_eq!(
            state.dispatch(MapMakerUiCommand::ActivateEditorMenuAction(quit)),
            MapMakerUiUpdate::EditorMenuActionActivated(EditorMenuAction::QuitMapMaker)
        );
        assert_eq!(state.editor_confirmation(), None);
        assert_eq!(
            state.dispatch(MapMakerUiCommand::ObserveQuitMapMakerConfirmation),
            MapMakerUiUpdate::EditorConfirmationOpened(EditorConfirmation::QuitMapMaker)
        );
        assert_eq!(
            state.focused_editor_confirmation_choice(),
            Some(EditorConfirmationChoice::Cancel)
        );
        let accept = state
            .editor_confirmation_target(EditorConfirmationChoice::Accept)
            .unwrap();
        assert_eq!(
            state.dispatch(MapMakerUiCommand::ResolveEditorConfirmationChoice(accept)),
            MapMakerUiUpdate::EditorMenuActionActivated(EditorMenuAction::QuitMapMaker)
        );
        assert_eq!(state.editor_confirmation(), None);
    }

    #[test]
    fn palette_property_actions_keep_measured_runtime_text_ids() {
        assert_eq!(
            PalettePropertyAction::LightRow.text_id(),
            PalettePropertyTextId::CURRENT_LIGHTS
        );
        assert_eq!(
            PalettePropertyAction::ChangePalette.text_id(),
            PalettePropertyTextId::CHANGE_TO_LIGHT
        );
        assert_eq!(PalettePropertyTextId::CURRENT_LIGHTS.get(), 0x0bc4);
        assert_eq!(PalettePropertyTextId::CHANGE_TO_LIGHT.get(), 0x0bc6);
    }

    #[test]
    fn editor_settings_edit_unchanged_back_closes_without_mutation() {
        let mut edit = EditorSettingsEdit::new(EditorKind(4), true);
        assert_eq!(
            edit.leave_settings(),
            [EditorSettingsEditEffect::CloseUnchanged]
        );
        assert_eq!(edit.current(), EditorKind(4));
        assert_eq!(edit.phase(), EditorSettingsEditPhase::Finished);
    }

    #[test]
    fn editor_settings_edit_kind_one_notifies_then_publishes_and_marks_once() {
        let mut edit = EditorSettingsEdit::new(EditorKind(4), true);
        assert!(edit.request(EditorKind(1)));
        assert_eq!(
            edit.leave_settings(),
            [
                EditorSettingsEditEffect::NotifyEditorKindChanged,
                EditorSettingsEditEffect::PublishRequestedKind(EditorKind(1)),
                EditorSettingsEditEffect::MarkChanged,
                EditorSettingsEditEffect::CloseSettings,
            ]
        );
        assert_eq!(edit.current(), EditorKind(1));
        assert_eq!(edit.phase(), EditorSettingsEditPhase::Finished);
        assert!(edit.leave_settings().is_empty());
    }

    #[test]
    fn editor_settings_edit_confirmation_initializes_before_publish_and_notifies_once() {
        let mut edit = EditorSettingsEdit::new(EditorKind(4), true);
        assert!(edit.request(EditorKind(7)));
        assert_eq!(
            edit.leave_settings(),
            [
                EditorSettingsEditEffect::NotifyEditorKindChanged,
                EditorSettingsEditEffect::RequestClearMapConfirmation,
            ]
        );
        assert_eq!(
            edit.phase(),
            EditorSettingsEditPhase::AwaitingClearConfirmation
        );
        assert_eq!(
            edit.cancel_clear_map(),
            [EditorSettingsEditEffect::DismissClearMapConfirmation]
        );
        assert_eq!(edit.requested(), EditorKind(7));
        assert_eq!(
            edit.leave_settings(),
            [EditorSettingsEditEffect::RequestClearMapConfirmation]
        );
        assert_eq!(
            edit.confirm_clear_map(),
            [
                EditorSettingsEditEffect::InitializeMap,
                EditorSettingsEditEffect::PublishRequestedKind(EditorKind(7)),
                EditorSettingsEditEffect::CloseSettings,
            ]
        );
        assert_eq!(edit.current(), EditorKind(7));
        assert_eq!(edit.phase(), EditorSettingsEditPhase::Finished);
    }

    #[test]
    fn finished_editor_settings_transaction_can_restart_after_reentry() {
        let mut state = MapMakerUiState::default();
        state.page = MapMakerPage::Editor;
        state.editor_surface = EditorSurface::EditorSettings;
        state.editor_settings_generation = Some(EditorSettingsGeneration::new(1));
        let choices = vec![EditorKind(0), EditorKind(1)];
        assert!(state.begin_editor_settings_edit(EditorKind(1), false, choices.clone(),));
        assert!(state.request_editor_kind(EditorKind(0)));
        assert_eq!(
            state.leave_editor_settings_edit(),
            [EditorSettingsEditEffect::RequestClearMapConfirmation]
        );
        assert_eq!(
            state.confirm_editor_kind_clear(),
            [
                EditorSettingsEditEffect::InitializeMap,
                EditorSettingsEditEffect::PublishRequestedKind(EditorKind(0)),
                EditorSettingsEditEffect::CloseSettings,
            ]
        );
        assert!(state.begin_editor_settings_edit(EditorKind(0), false, choices));
        assert_eq!(
            state.editor_settings_edit().unwrap().requested(),
            EditorKind(0)
        );
        assert_eq!(
            state.editor_settings_edit().unwrap().phase(),
            EditorSettingsEditPhase::Editing
        );
    }

    fn assault_mode() -> ModeConfiguration {
        ModeConfiguration::Assault(AssaultModeConfiguration {
            settings: ModeSettings {
                editor_kind: EditorKind(7),
                theme: ThemeId(3),
                preview: PreviewSelection {
                    row: PreviewRow(4),
                    variant: LaunchVariant(0x15),
                    game_type: GameType(9),
                },
                row_options: 0x12,
                duration: 900,
            },
            phases: Default::default(),
        })
    }

    #[test]
    fn entry_defaults_advanced_without_selecting_a_playable_mode() {
        let state = MapMakerUiState::default();
        assert_eq!(state.experience(), EditorExperience::Advanced);
        assert_eq!(state.page(), MapMakerPage::EditorMenu);
        assert!(state.selected_mode().is_none());
    }

    #[test]
    fn mode_settings_and_preview_keep_their_distinct_typed_identities() {
        let mut state = MapMakerUiState::default();
        let mode = assault_mode();
        let choice_id = ModeChoiceId::new(0xfeed);
        state
            .install_mode_choices(vec![ModeChoice {
                id: choice_id,
                configuration: mode.clone(),
            }])
            .unwrap();
        assert_eq!(mode.kind(), MapMode::Assault);
        assert_eq!(
            state.dispatch(MapMakerUiCommand::OpenModeSelection),
            MapMakerUiUpdate::PageChanged(MapMakerPage::ModeSelection)
        );
        assert_eq!(
            state.dispatch(MapMakerUiCommand::FocusMode(choice_id)),
            MapMakerUiUpdate::ModeFocused(choice_id)
        );
        assert_eq!(
            state.dispatch(MapMakerUiCommand::SelectMode(choice_id)),
            MapMakerUiUpdate::PageChanged(MapMakerPage::ModeSettings)
        );
        assert_eq!(state.page(), MapMakerPage::ModeSettings);
        assert_eq!(state.experience(), EditorExperience::Advanced);
        assert_eq!(state.selected_mode_id(), Some(choice_id));
        assert_eq!(state.selected_mode(), Some(&mode));

        assert_eq!(
            state.dispatch(MapMakerUiCommand::OpenPreview),
            MapMakerUiUpdate::PreviewOpened(PreviewRow(4))
        );
        assert_eq!(state.page(), MapMakerPage::Preview);
        assert_eq!(state.preview_row(), Some(PreviewRow(4)));
        assert_eq!(
            state.selected_mode().unwrap().settings().preview.variant,
            LaunchVariant(0x15)
        );
        assert_eq!(
            state.selected_mode().unwrap().settings().preview.game_type,
            GameType(9)
        );
    }

    #[test]
    fn returning_to_editor_retains_mode_and_experience_is_separate() {
        let mut state = MapMakerUiState::default();
        let mode = assault_mode();
        let choice_id = ModeChoiceId::new(7);
        state
            .install_mode_choices(vec![ModeChoice {
                id: choice_id,
                configuration: mode.clone(),
            }])
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenModeSelection);
        state.dispatch(MapMakerUiCommand::SelectMode(choice_id));
        state.dispatch(MapMakerUiCommand::OpenPreview);
        state.set_experience(EditorExperience::Beginner);
        assert_eq!(
            state.dispatch(MapMakerUiCommand::Back),
            MapMakerUiUpdate::PageChanged(MapMakerPage::ModeSettings)
        );
        assert_eq!(
            state.dispatch(MapMakerUiCommand::Back),
            MapMakerUiUpdate::PageChanged(MapMakerPage::ModeSelection)
        );
        assert_eq!(
            state.dispatch(MapMakerUiCommand::Back),
            MapMakerUiUpdate::PageChanged(MapMakerPage::EditorMenu)
        );

        assert_eq!(state.page(), MapMakerPage::EditorMenu);
        assert_eq!(state.experience(), EditorExperience::Beginner);
        assert_eq!(state.selected_mode(), Some(&mode));
    }

    #[test]
    fn mode_focus_uses_runtime_identity_and_rejects_stale_or_duplicate_rows() {
        let mut state = MapMakerUiState::default();
        let choice_id = ModeChoiceId::new(42);
        let choice = ModeChoice {
            id: choice_id,
            configuration: assault_mode(),
        };
        state.install_mode_choices(vec![choice.clone()]).unwrap();
        state.dispatch(MapMakerUiCommand::OpenModeSelection);

        assert_eq!(
            state.dispatch(MapMakerUiCommand::FocusMode(ModeChoiceId::new(99))),
            MapMakerUiUpdate::Ignored
        );
        assert_eq!(state.focused_mode(), None);
        assert_eq!(
            state.dispatch(MapMakerUiCommand::FocusMode(choice_id)),
            MapMakerUiUpdate::ModeFocused(choice_id)
        );
        assert_eq!(state.focused_mode(), Some(choice_id));

        let duplicate = ModeChoice {
            id: choice_id,
            configuration: assault_mode(),
        };
        assert_eq!(
            state.install_mode_choices(vec![choice.clone(), duplicate]),
            Err(ModeChoiceError::DuplicateIdentity)
        );
        assert_eq!(state.mode_choices(), &[choice]);
        assert_eq!(state.focused_mode(), Some(choice_id));

        state.install_mode_choices(Vec::new()).unwrap();
        assert!(state.mode_choices().is_empty());
        assert_eq!(state.focused_mode(), None);
    }

    #[test]
    fn page_commands_are_guarded_and_do_not_change_state_when_ignored() {
        let mut state = MapMakerUiState::default();
        assert_eq!(
            state.dispatch(MapMakerUiCommand::OpenPreview),
            MapMakerUiUpdate::Ignored
        );
        assert_eq!(
            state.dispatch(MapMakerUiCommand::SelectMode(ModeChoiceId::new(1))),
            MapMakerUiUpdate::Ignored
        );
        assert_eq!(state.page(), MapMakerPage::EditorMenu);
        assert!(state.selected_mode().is_none());
        assert!(state.open_preview().is_none());
    }

    #[test]
    fn snapshot_projects_ordered_choices_and_owned_selection_for_rendering() {
        let first = ModeChoiceId::new(0xf001);
        let second = ModeChoiceId::new(0xf002);
        let mut state = MapMakerUiState::default();
        state
            .install_mode_choices(vec![
                ModeChoice {
                    id: first,
                    configuration: assault_mode(),
                },
                ModeChoice {
                    id: second,
                    configuration: assault_mode(),
                },
            ])
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenModeSelection);
        state.dispatch(MapMakerUiCommand::FocusMode(second));

        let view = state.snapshot();
        assert_eq!(view.experience, EditorExperience::Advanced);
        assert_eq!(view.page, MapMakerPage::ModeSelection);
        assert_eq!(
            view.mode_choices
                .iter()
                .map(|choice| choice.id)
                .collect::<Vec<_>>(),
            [first, second]
        );
        assert_eq!(view.focused_mode, Some(second));
        assert_eq!(view.selected_mode_id, None);
        assert!(view.selected_mode.is_none());

        state.dispatch(MapMakerUiCommand::SelectMode(second));
        let view = state.snapshot();
        assert_eq!(view.page, MapMakerPage::ModeSettings);
        assert_eq!(view.selected_mode_id, Some(second));
        assert_eq!(view.selected_mode, Some(&assault_mode()));
        assert_eq!(view.preview_row, None);
    }

    #[test]
    fn snapshot_exposes_and_advances_owned_overlay_presentation() {
        let mut state = MapMakerUiState::default();
        assert!(state.snapshot().overlay_ramp.is_none());
        assert_eq!(state.snapshot().overlay_visibility, None);
        assert!(state.snapshot().hint_anchor_transform.is_none());
        assert_eq!(state.shift_hint_anchor(10, 7), None);
        state.install_overlay_ramp(MapMakerOverlayRamp::new(0.5, 10.0, 1.0, 1.0));
        assert_eq!(state.step_overlay(), None);
        let visibility = MapMakerOverlayVisibility {
            first_leaf_present: true,
            second_leaf_flag_218_bit_0: false,
            third_leaf_is_page: false,
            hud_bit_15: false,
        };
        state.install_overlay_visibility(visibility);
        assert_eq!(state.step_overlay(), Some(0.6));
        assert_eq!(state.snapshot().overlay_ramp.unwrap().alpha(), 0.6);
        assert_eq!(state.snapshot().overlay_visibility, Some(visibility));

        let transform = HintAnchorTransform {
            pointer_x: 0.5,
            pointer_y: 2.0,
            horizontal_scale: 2.0,
            horizontal_bias: 3.0,
            vertical_pointer_scale: 0.5,
            vertical_anchor_scale: 3.0,
        };
        state.install_hint_anchor_transform(transform);
        assert_eq!(state.shift_hint_anchor(10, 7), Some((14, 10)));
        assert_eq!(state.snapshot().hint_anchor_transform, Some(transform));
    }
}

#[cfg(test)]
mod storage_tests {
    use super::{
        EditorHistoryOperation, EditorMenuAction, EditorMenuCatalogError, EditorMenuGeneration,
        EditorSurface, EditorSurfaceCatalogError, EditorSurfaceGeneration, MapListError,
        MapListGeneration, MapListRow, MapListRowId, MapLoadOutcome, MapLoadRequestId,
        MapMakerPage, MapMakerUiCommand as C, MapMakerUiState, MapMakerUiUpdate as U,
        ORIGINAL_EDITOR_MENU_ACTIONS, StorageOperation, StorageProgressOutcome,
    };

    fn row(generation: u64, key: u64) -> MapListRow {
        MapListRow {
            id: MapListRowId::new(MapListGeneration::new(generation), key),
        }
    }

    fn open_map_list(state: &mut MapMakerUiState) {
        assert_eq!(
            state.dispatch(C::OpenStorage),
            U::PageChanged(MapMakerPage::StorageSource)
        );
        assert_eq!(
            state.dispatch(C::SelectLocalStorage),
            U::PageChanged(MapMakerPage::MapList)
        );
    }

    #[test]
    fn document_close_prompt_is_owned_by_the_editor_ui_session() {
        use crate::document_close::{
            ClosePromptAction, ClosePromptCommand, ClosePromptPhase, ClosePromptToken,
            ClosePromptUpdate, MapDocumentId, SaveResult,
        };

        let mut state = MapMakerUiState::default();
        let token = ClosePromptToken::new(MapDocumentId::new(7), 3);
        assert!(
            !state.open_document_close_prompt(ClosePromptToken::new(MapDocumentId::new(7), 0,))
        );
        assert!(!state.open_document_close_prompt(token));
        assert_eq!(state.document_close_prompt_phase(), None);

        assert_eq!(
            state.dispatch(C::BeginEditor),
            U::PageChanged(MapMakerPage::Editor)
        );
        assert!(state.open_document_close_prompt(token));
        assert_eq!(
            state.document_close_prompt_phase(),
            Some(ClosePromptPhase::AwaitingChoice)
        );

        let ClosePromptUpdate::SaveRequested(first_request) =
            state.apply_document_close_prompt(ClosePromptCommand::Choose {
                token,
                action: ClosePromptAction::Save,
            })
        else {
            panic!("save choice should issue a typed request");
        };
        assert_eq!(
            state.document_close_prompt_phase(),
            Some(ClosePromptPhase::Saving(first_request.id))
        );
        assert!(!state.open_document_close_prompt(token));

        assert_eq!(
            state.dispatch(C::OpenStorage),
            U::PageChanged(MapMakerPage::StorageSource)
        );
        assert_eq!(
            state.document_close_prompt_phase(),
            Some(ClosePromptPhase::Saving(first_request.id))
        );
        assert_eq!(
            state.apply_document_close_prompt(ClosePromptCommand::Choose {
                token,
                action: ClosePromptAction::Cancel,
            }),
            ClosePromptUpdate::Ignored
        );

        assert_eq!(
            state.apply_document_close_prompt(ClosePromptCommand::SaveCompleted {
                token: ClosePromptToken::new(MapDocumentId::new(8), 3),
                request: first_request.id,
                result: SaveResult::Committed,
            }),
            ClosePromptUpdate::Ignored
        );
        assert_eq!(
            state.apply_document_close_prompt(ClosePromptCommand::SaveCompleted {
                token,
                request: first_request.id,
                result: SaveResult::Failed,
            }),
            ClosePromptUpdate::SaveFailed(token)
        );
        assert_eq!(
            state.document_close_prompt_phase(),
            Some(ClosePromptPhase::AwaitingChoice)
        );
    }

    #[test]
    fn map_catalogue_install_is_atomic_and_generation_qualified() {
        let mut state = MapMakerUiState::default();
        let first = row(4, 10);
        let second = row(4, 11);
        state
            .install_local_map_rows(MapListGeneration::new(4), vec![first, second])
            .unwrap();
        open_map_list(&mut state);
        state.dispatch(C::FocusMap(second.id));

        assert_eq!(
            state.install_local_map_rows(MapListGeneration::new(4), vec![first, first]),
            Err(MapListError::DuplicateIdentity)
        );
        assert_eq!(
            state.install_local_map_rows(MapListGeneration::new(5), vec![first]),
            Err(MapListError::WrongGeneration)
        );
        assert_eq!(state.map_rows(), &[first, second]);
        assert_eq!(state.focused_map(), Some(second.id));

        let replacement = row(5, 11);
        state
            .install_local_map_rows(MapListGeneration::new(5), vec![replacement])
            .unwrap();
        assert_eq!(state.focused_map(), None);
        assert_ne!(replacement.id, second.id);
    }

    #[test]
    fn loaded_editor_owns_typed_page_history_and_back_navigation() {
        let mut state = MapMakerUiState::default();
        let map = row(7, 12);
        state
            .install_local_map_rows(MapListGeneration::new(7), vec![map])
            .unwrap();
        open_map_list(&mut state);
        let U::LoadRequested(request) = state.dispatch(C::SelectMap(map.id)) else {
            panic!("selected row should request a load");
        };
        assert_eq!(
            state.dispatch(C::CompleteMapLoad {
                request: request.id,
                outcome: MapLoadOutcome::Loaded,
            }),
            U::LoadCompleted(map.id)
        );

        assert_eq!(state.snapshot().editor_surface, EditorSurface::MainMenu);
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(1),
                vec![EditorSurface::MapSettings, EditorSurface::EditorSettings],
            )
            .unwrap();
        let map_settings = state
            .editor_surface_target(EditorSurface::MapSettings)
            .unwrap();
        assert_eq!(
            state.dispatch(C::OpenEditorSurface(map_settings)),
            U::EditorSurfaceChanged(EditorSurface::MapSettings)
        );
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(2),
                vec![EditorSurface::EditorSettings],
            )
            .unwrap();
        let editor_settings = state
            .editor_surface_target(EditorSurface::EditorSettings)
            .unwrap();
        assert_eq!(
            state.dispatch(C::OpenEditorSurface(editor_settings)),
            U::EditorSurfaceChanged(EditorSurface::EditorSettings)
        );
        assert_eq!(
            state.snapshot().editor_surface_history,
            &[EditorSurface::MainMenu, EditorSurface::MapSettings]
        );
        assert_eq!(
            state.dispatch(C::Back),
            U::EditorSurfaceChanged(EditorSurface::MapSettings)
        );
        assert_eq!(
            state.dispatch(C::Back),
            U::EditorSurfaceChanged(EditorSurface::MainMenu)
        );
        assert_eq!(state.dispatch(C::Back), U::Ignored);
        assert_eq!(state.snapshot().editor_surface_history, &[]);
    }

    #[test]
    fn editor_surface_catalog_is_atomic_and_rejects_stale_generation_targets() {
        let mut state = MapMakerUiState::default();
        assert_eq!(
            state.install_editor_surface_catalog(
                EditorSurfaceGeneration::new(9),
                vec![EditorSurface::MapSettings],
            ),
            Err(EditorSurfaceCatalogError::NotInLoadedEditor)
        );
        let map = row(10, 1);
        state
            .install_local_map_rows(MapListGeneration::new(10), vec![map])
            .unwrap();
        open_map_list(&mut state);
        let U::LoadRequested(request) = state.dispatch(C::SelectMap(map.id)) else {
            panic!("selected row should request a load");
        };
        state.dispatch(C::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });

        let first_generation = EditorSurfaceGeneration::new(11);
        state
            .install_editor_surface_catalog(
                first_generation,
                vec![EditorSurface::MapSettings, EditorSurface::EditorSettings],
            )
            .unwrap();
        let old_target = state
            .editor_surface_target(EditorSurface::EditorSettings)
            .unwrap();
        assert_eq!(state.snapshot().editor_surfaces, state.editor_surfaces());
        assert_eq!(
            state.snapshot().editor_surface_generation,
            Some(first_generation)
        );

        assert_eq!(
            state.install_editor_surface_catalog(
                EditorSurfaceGeneration::new(12),
                vec![EditorSurface::MapSettings, EditorSurface::MapSettings],
            ),
            Err(EditorSurfaceCatalogError::DuplicateSurface)
        );
        assert_eq!(
            state.install_editor_surface_catalog(
                EditorSurfaceGeneration::new(12),
                vec![EditorSurface::MainMenu],
            ),
            Err(EditorSurfaceCatalogError::MainMenuIsNotAnAction)
        );
        assert_eq!(state.editor_surface_generation(), Some(first_generation));
        assert_eq!(state.editor_surfaces().len(), 2);

        let next_generation = EditorSurfaceGeneration::new(12);
        state
            .install_editor_surface_catalog(next_generation, vec![EditorSurface::EditorSettings])
            .unwrap();
        assert!(!state.contains_editor_surface_target(old_target));
        assert_eq!(
            state.dispatch(C::FocusEditorSurface(old_target)),
            U::Ignored
        );
        assert_eq!(state.focused_editor_surface(), None);
        let current_target = state
            .editor_surface_target(EditorSurface::EditorSettings)
            .unwrap();
        assert_eq!(
            state.dispatch(C::FocusEditorSurface(current_target)),
            U::EditorSurfaceFocused(EditorSurface::EditorSettings)
        );

        let first_menu_generation = EditorMenuGeneration::new(30);
        state
            .install_original_editor_menu(first_menu_generation)
            .unwrap();
        assert_eq!(state.editor_menu_actions(), ORIGINAL_EDITOR_MENU_ACTIONS);
        assert_eq!(
            state.snapshot().editor_menu_actions,
            ORIGINAL_EDITOR_MENU_ACTIONS
        );
        let stale_menu_target = state.editor_menu_target(EditorMenuAction::Undo).unwrap();
        assert_eq!(
            state.install_editor_menu_catalog(
                EditorMenuGeneration::new(31),
                vec![EditorMenuAction::Undo, EditorMenuAction::Undo],
            ),
            Err(EditorMenuCatalogError::DuplicateAction)
        );
        assert_eq!(state.editor_menu_generation(), Some(first_menu_generation));
        state
            .install_original_editor_menu(EditorMenuGeneration::new(31))
            .unwrap();
        assert!(!state.contains_editor_menu_target(stale_menu_target));
        assert_eq!(
            state.dispatch(C::ActivateEditorMenuAction(stale_menu_target)),
            U::Ignored
        );
        let undo = state.editor_menu_target(EditorMenuAction::Undo).unwrap();
        assert_eq!(
            state.dispatch(C::FocusEditorMenuAction(undo)),
            U::EditorMenuActionFocused(EditorMenuAction::Undo)
        );
        assert_eq!(
            state.dispatch(C::ActivateEditorMenuAction(undo)),
            U::EditorHistoryRequested(EditorHistoryOperation::Undo)
        );
        state.set_editor_history_availability(Some(crate::history::HistoryAvailability {
            can_undo: false,
            can_redo: true,
        }));
        assert_eq!(
            state.snapshot().editor_history_availability,
            Some(crate::history::HistoryAvailability {
                can_undo: false,
                can_redo: true,
            })
        );
        assert_eq!(
            state.dispatch(C::ActivateEditorMenuAction(undo)),
            U::Ignored
        );
        let redo = state.editor_menu_target(EditorMenuAction::Redo).unwrap();
        assert_eq!(
            state.dispatch(C::ActivateEditorMenuAction(redo)),
            U::EditorHistoryRequested(super::EditorHistoryOperation::Redo)
        );
    }

    #[test]
    fn selection_is_a_host_request_and_only_matching_success_returns_to_editor() {
        let mut state = MapMakerUiState::default();
        let selected = row(8, 52);
        state
            .install_local_map_rows(MapListGeneration::new(8), vec![selected])
            .unwrap();
        open_map_list(&mut state);
        let U::LoadRequested(request) = state.dispatch(C::SelectMap(selected.id)) else {
            panic!("valid row should emit a typed host request");
        };
        assert_eq!(request.row, selected.id);
        assert_eq!(state.page(), MapMakerPage::MapList);
        assert_eq!(state.pending_map_load(), Some(request));
        assert_eq!(state.dispatch(C::FocusMap(selected.id)), U::Ignored);
        assert_eq!(
            state.install_local_map_rows(MapListGeneration::new(8), vec![selected]),
            Err(MapListError::LoadInFlight)
        );
        assert_eq!(
            state.dispatch(C::CompleteMapLoad {
                request: MapLoadRequestId(request.id.get() + 1),
                outcome: MapLoadOutcome::Loaded,
            }),
            U::Ignored
        );
        assert_eq!(state.page(), MapMakerPage::MapList);
        assert_eq!(state.pending_map_load(), Some(request));
        assert_eq!(
            state.dispatch(C::CompleteMapLoad {
                request: request.id,
                outcome: MapLoadOutcome::Loaded,
            }),
            U::LoadCompleted(selected.id)
        );
        assert_eq!(state.page(), MapMakerPage::Editor);
        assert_eq!(state.pending_map_load(), None);
    }

    #[test]
    fn failed_load_stays_in_list_and_back_cancels_pending_request() {
        let mut state = MapMakerUiState::default();
        let selected = row(2, 7);
        state
            .install_local_map_rows(MapListGeneration::new(2), vec![selected])
            .unwrap();
        open_map_list(&mut state);
        let U::LoadRequested(request) = state.dispatch(C::SelectMap(selected.id)) else {
            panic!("valid row should emit a typed host request");
        };
        assert_eq!(
            state.dispatch(C::CompleteMapLoad {
                request: request.id,
                outcome: MapLoadOutcome::Failed,
            }),
            U::LoadFailed(selected.id)
        );
        assert_eq!(state.page(), MapMakerPage::MapList);
        assert_eq!(state.pending_map_load(), None);

        let U::LoadRequested(request) = state.dispatch(C::SelectMap(selected.id)) else {
            panic!("valid row should emit a second typed host request");
        };
        assert_eq!(state.dispatch(C::Back), U::LoadCancelled(request.id));
        assert_eq!(state.page(), MapMakerPage::MapList);
        assert_eq!(
            state.dispatch(C::CompleteMapLoad {
                request: request.id,
                outcome: MapLoadOutcome::Loaded,
            }),
            U::Ignored
        );
        assert_eq!(
            state.dispatch(C::Back),
            U::PageChanged(MapMakerPage::StorageSource)
        );
        assert_eq!(
            state.dispatch(C::Back),
            U::PageChanged(MapMakerPage::EditorMenu)
        );
    }

    #[test]
    fn storage_progress_requires_matching_completion_before_back() {
        let mut state = MapMakerUiState::default();
        let U::StorageProgressOpened(first) =
            state.dispatch(C::OpenStorageProgress(StorageOperation::SaveMap))
        else {
            panic!("storage operation should open its progress page");
        };
        assert_eq!(state.page(), MapMakerPage::StorageProgress);
        assert_eq!(
            state.storage_progress().unwrap().outcome,
            StorageProgressOutcome::Pending
        );
        assert_eq!(state.dispatch(C::Back), U::Ignored);
        assert_eq!(
            state.dispatch(C::CompleteStorageProgress {
                progress: first,
                outcome: StorageProgressOutcome::Pending,
            }),
            U::Ignored
        );
        let U::StorageProgressCompleted(completed) = state.dispatch(C::CompleteStorageProgress {
            progress: first,
            outcome: StorageProgressOutcome::Failed,
        }) else {
            panic!("matching host completion should update the status");
        };
        assert_eq!(completed.id, first);
        assert_eq!(completed.operation, StorageOperation::SaveMap);
        assert_eq!(completed.outcome, StorageProgressOutcome::Failed);
        assert_eq!(
            state.dispatch(C::CompleteStorageProgress {
                progress: first,
                outcome: StorageProgressOutcome::Succeeded,
            }),
            U::Ignored
        );
        assert_eq!(
            state.dispatch(C::Back),
            U::StorageProgressDismissed(StorageOperation::SaveMap)
        );
        assert_eq!(state.page(), MapMakerPage::EditorMenu);
        assert_eq!(state.storage_progress(), None);
    }
}
