//! Rust-owned Map Maker appearance palette state (T1690).
//!
//! The original boundary is a lossless five-DWORD descriptor. The REA notes establish
//! capture, reset/dialog initialization, eager synchronization to the current selection,
//! and navigation dispatch. They do not establish field meanings or cancel rollback, so
//! this module keeps the descriptor opaque and makes exit a navigation effect only.

use crate::focus::{NavLayout, Wrap};
use crate::model::{Page, PageId, Widget, WidgetId};
use tsfp_mapmaker::ui::{
    EditorSurface, MapMakerPage, MapMakerUiCommand, MapMakerUiSnapshot, MapMakerUiState,
    MapMakerUiUpdate, PaletteGridChoiceId, PaletteGridChoiceTarget,
};
use tsfp_ui_renderer::AnchoredRect;

pub const PALETTE_PAGE_DESCRIPTOR: u32 = 0x52e000;
pub const PALETTE_PROPERTIES_DESCRIPTOR: u32 = 0x52ea48;

/// Runtime supplied widget and opaque choice identity for one palette-grid row.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteGridChoiceLayout {
    pub choice: PaletteGridChoiceId,
    pub widget: Widget,
}

/// Original page structure comes from the current runtime page; this projection
/// accepts its widget family, navigation and widget values without interpreting
/// palette records or assigning selection effects.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteGridLayout {
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub nav: NavLayout,
    pub wrap: Wrap,
    pub choices: Vec<PaletteGridChoiceLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteGridBuildError {
    WrongPage,
    MissingGeneration,
    ChoiceCountMismatch,
    ChoiceOrderMismatch,
    DuplicateWidget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PaletteGridWidgetBinding {
    widget: WidgetId,
    target: PaletteGridChoiceTarget,
}

/// Renderer-facing PaletteGrid page with typed routes to Rust-owned choice state.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteGridPage {
    pub page: Page,
    bindings: Vec<PaletteGridWidgetBinding>,
}

impl PaletteGridPage {
    pub fn target_for_widget(&self, widget: WidgetId) -> Option<PaletteGridChoiceTarget> {
        self.bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.target)
    }

    pub fn focus_widget(&self, state: &mut MapMakerUiState, widget: WidgetId) -> MapMakerUiUpdate {
        self.target_for_widget(widget)
            .map_or(MapMakerUiUpdate::Ignored, |target| {
                state.dispatch(MapMakerUiCommand::FocusPaletteGridChoice(target))
            })
    }

    pub fn activate_widget(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
    ) -> MapMakerUiUpdate {
        self.target_for_widget(widget)
            .map_or(MapMakerUiUpdate::Ignored, |target| {
                state.dispatch(MapMakerUiCommand::ActivatePaletteGridChoice(target))
            })
    }
}

/// Bind the ordered Rust-owned choice identities to widgets supplied by the
/// live page provider. Text, geometry, and navigation remain runtime data.
pub fn build_palette_grid_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    page_id: PageId,
    layout: PaletteGridLayout,
) -> Result<PaletteGridPage, PaletteGridBuildError> {
    if snapshot.page != MapMakerPage::Editor
        || snapshot.editor_surface != EditorSurface::PaletteGrid
    {
        return Err(PaletteGridBuildError::WrongPage);
    }
    let generation = snapshot
        .palette_grid_generation
        .ok_or(PaletteGridBuildError::MissingGeneration)?;
    if layout.choices.len() != snapshot.palette_grid_choices.len() {
        return Err(PaletteGridBuildError::ChoiceCountMismatch);
    }

    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = layout.nav;
    page.wrap = layout.wrap;
    let mut bindings = Vec::with_capacity(layout.choices.len());
    for (index, row) in layout.choices.into_iter().enumerate() {
        if snapshot.palette_grid_choices.get(index) != Some(&row.choice) {
            return Err(PaletteGridBuildError::ChoiceOrderMismatch);
        }
        if page.widgets.iter().any(|widget| widget.id == row.widget.id) {
            return Err(PaletteGridBuildError::DuplicateWidget);
        }
        page.widgets.push(row.widget.clone());
        bindings.push(PaletteGridWidgetBinding {
            widget: row.widget.id,
            target: PaletteGridChoiceTarget::new(generation, row.choice),
        });
    }
    page.initial_focus = snapshot.focused_palette_grid_choice.and_then(|focused| {
        bindings
            .iter()
            .find(|binding| binding.target.choice() == focused)
            .map(|binding| binding.widget)
    });
    Ok(PaletteGridPage { page, bindings })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteViewFlagEffect {
    /// Clear editor `+25C` bit `10000000` before opening `52EA48`.
    Clear,
    /// Set editor `+25C` bit `10000000` before re-entering `52E000`.
    Set,
}

/// Result of entering either side of the palette/properties route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteDialogTransition {
    pub descriptor: u32,
    pub view_flag: PaletteViewFlagEffect,
    pub initialize_properties_marker: bool,
    pub initialize_current_descriptor_if_zero: bool,
}

/// Services used by the properties-page header. The host resolves navigation data and owns
/// the document/history stores; calls are made in the observed source order.
pub trait PalettePropertiesServices {
    fn rerun_eligible_selected_entry_handlers(&mut self);
    fn page_rows_changed(&mut self, page: PageId) -> bool;
    fn selected_entry_snapshot_changed(&mut self) -> bool;
    fn history_suppressed(&self) -> bool;
    fn submit_history(&mut self, kind: u32);
    fn mark_history_suppressed(&mut self);
    fn clear_view_suppression_flag_and_selected_entry_snapshots(&mut self);
    fn navigation_parent(&self, page: PageId) -> u32;
    fn navigate(&mut self, parent: u32, descriptor: u32);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PalettePropertiesExit {
    pub history_submitted: bool,
    pub descriptor: u32,
}

/// Header behavior for the shared properties dialog (`328BF0`).
pub fn exit_properties_dialog(
    page: PageId,
    destination_descriptor: u32,
    services: &mut impl PalettePropertiesServices,
) -> PalettePropertiesExit {
    services.rerun_eligible_selected_entry_handlers();
    let rows_changed = services.page_rows_changed(page);
    let entry_changed = !rows_changed && services.selected_entry_snapshot_changed();
    let history_submitted = (rows_changed || entry_changed) && !services.history_suppressed();
    if history_submitted {
        services.submit_history(16);
        // The history service may change document/editor state, so this is a fresh service call.
        services.mark_history_suppressed();
    }
    services.clear_view_suppression_flag_and_selected_entry_snapshots();
    let parent = services.navigation_parent(page);
    services.navigate(parent, destination_descriptor);
    PalettePropertiesExit {
        history_submitted,
        descriptor: destination_descriptor,
    }
}

/// The five DWORDs copied by the Map Maker appearance path. Their individual meanings are
/// intentionally opaque until the original data tables establish them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaletteDescriptor(pub [u32; 5]);

impl PaletteDescriptor {
    pub const WORD_COUNT: usize = 5;

    pub const fn words(self) -> [u32; Self::WORD_COUNT] {
        self.0
    }
}

/// Stable identity supplied by the Rust-owned document. It is generation-qualified so a
/// stale selection cannot accidentally name a newly allocated record at the same slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PaletteRecordId {
    pub owner: u64,
    pub slot: u32,
    pub generation: u64,
}

/// The capture source and current synchronization destination are separate identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteCaptureSource {
    LinkedAppearance(PaletteRecordId),
    SelectedObjectNode(PaletteRecordId),
}

/// Values returned by the original reset helpers and selected-record view. Keeping these
/// explicit prevents the UI model from inventing a meaning for global `0x475D4C` or for the
/// helper that supplies the first scratch byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteResetInput {
    pub returned_first_byte: u8,
    pub reset_word: u32,
    pub selected_record: PaletteRecordId,
    pub selected_record_initialized: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteNavigation {
    PopCurrentPage,
    ResetToolThenPop,
    IgnoreUnknownPage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteExit {
    Properties,
    PaletteGrid,
    AppearanceRow,
    AppearancePrompt,
    ToolPalette,
    Unknown(u32),
}

impl PaletteExit {
    /// Source-established `31A500` routing. The delegated navigation operation owns page
    /// teardown; this function does not claim that an exit is a user cancel or rollback.
    pub const fn navigation(self) -> PaletteNavigation {
        match self {
            Self::Properties | Self::PaletteGrid | Self::AppearanceRow | Self::AppearancePrompt => {
                PaletteNavigation::PopCurrentPage
            }
            Self::ToolPalette => PaletteNavigation::ResetToolThenPop,
            Self::Unknown(_) => PaletteNavigation::IgnoreUnknownPage,
        }
    }
}

/// The current Rust document resolves its selected destination every time the original page
/// gate would run. No borrowed guest pointer or previously captured destination is retained.
pub trait PaletteDocument {
    fn current_palette_record(&self) -> Option<PaletteRecordId>;
    fn write_palette_descriptor(&mut self, id: PaletteRecordId, value: PaletteDescriptor) -> bool;
    fn initialize_properties_marker_if_zero(&mut self, id: PaletteRecordId) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteSync {
    NoCurrentRecord,
    CurrentRecordUnavailable(PaletteRecordId),
    Written(PaletteRecordId),
}

/// Typed, Rust-owned scratch state shared by the palette and its properties dialog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaletteSession {
    working: PaletteDescriptor,
    capture_source: Option<PaletteCaptureSource>,
}

impl PaletteSession {
    pub const fn working(&self) -> PaletteDescriptor {
        self.working
    }

    pub const fn capture_source(&self) -> Option<PaletteCaptureSource> {
        self.capture_source
    }

    /// Entry chosen by the source-inspected reset window: initialize the scratch words and
    /// request the properties page. The caller supplies the exact helper results and 8C state.
    pub fn reset_dialog_transition(
        &mut self,
        document: &mut impl PaletteDocument,
        input: PaletteResetInput,
    ) -> PaletteDialogTransition {
        let initialize_properties_marker = self.reset_into_properties(document, input);
        PaletteDialogTransition {
            descriptor: PALETTE_PROPERTIES_DESCRIPTOR,
            view_flag: PaletteViewFlagEffect::Clear,
            initialize_properties_marker,
            initialize_current_descriptor_if_zero: false,
        }
    }

    /// The registered change row sets the view flag and re-enters the palette page. Its
    /// descriptor initializer runs only when the freshly resolved current record is unmarked.
    pub const fn change_row_transition(
        current_descriptor_initialized: bool,
    ) -> PaletteDialogTransition {
        PaletteDialogTransition {
            descriptor: PALETTE_PAGE_DESCRIPTOR,
            view_flag: PaletteViewFlagEffect::Set,
            initialize_properties_marker: false,
            initialize_current_descriptor_if_zero: !current_descriptor_initialized,
        }
    }

    /// Mirrors `31A710`: when the active descriptor's first DWORD equals the value stored
    /// at `52E030`, capture is skipped and the existing working copy is left untouched.
    pub fn capture_linked_appearance(
        &mut self,
        active_descriptor_first_word: u32,
        comparison_value: u32,
        record: PaletteRecordId,
        descriptor: PaletteDescriptor,
    ) -> bool {
        if active_descriptor_first_word == comparison_value {
            return false;
        }
        self.working = descriptor;
        self.capture_source = Some(PaletteCaptureSource::LinkedAppearance(record));
        true
    }

    /// Mirrors the observed selected-object capture condition: a present node reports true
    /// even when the view flag is clear, while scratch changes only when that flag is set.
    pub fn capture_selected_object_node(
        &mut self,
        node: Option<(PaletteRecordId, PaletteDescriptor)>,
        copy_enabled: bool,
    ) -> bool {
        let Some((id, descriptor)) = node else {
            return false;
        };
        if copy_enabled {
            self.working = descriptor;
            self.capture_source = Some(PaletteCaptureSource::SelectedObjectNode(id));
        }
        true
    }

    /// Applies the established reset writes while preserving bytes 2 and 3 of word zero.
    /// The record marker is initialized only when its prior value was zero.
    pub fn reset_into_properties(
        &mut self,
        document: &mut impl PaletteDocument,
        input: PaletteResetInput,
    ) -> bool {
        let mut words = self.working.0;
        words[0] = (words[0] & 0xffff_0000) | u32::from(input.returned_first_byte) | (7 << 8);
        words[1] = 0;
        words[2] = 0;
        words[3] = input.reset_word;
        words[4] = 0;
        self.working = PaletteDescriptor(words);
        self.capture_source = Some(PaletteCaptureSource::LinkedAppearance(
            input.selected_record,
        ));
        !input.selected_record_initialized
            && document.initialize_properties_marker_if_zero(input.selected_record)
    }

    /// Mirrors the `52E000` gate's unconditional five-word write before input dispatch.
    /// The destination is freshly resolved for each call and is not retained in the session.
    pub fn synchronize_before_input(&self, document: &mut impl PaletteDocument) -> PaletteSync {
        let Some(id) = document.current_palette_record() else {
            return PaletteSync::NoCurrentRecord;
        };
        if !document.write_palette_descriptor(id, self.working) {
            return PaletteSync::CurrentRecordUnavailable(id);
        }
        PaletteSync::Written(id)
    }

    /// Native input handlers replace the opaque working descriptor through typed state.
    pub fn replace_working_copy(&mut self, value: PaletteDescriptor) {
        self.working = value;
    }

    /// Exit only returns the documented navigation request; it does not restore or commit
    /// palette data. Delegated callback effects and natural cancel semantics are unknown.
    pub const fn exit(&self, page: PaletteExit) -> PaletteNavigation {
        page.navigation()
    }
}

/// Immutable appearance data for the Rust UI renderer. Rendering cannot mutate document state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteAppearanceView {
    pub descriptor: PaletteDescriptor,
}

impl From<&PaletteSession> for PaletteAppearanceView {
    fn from(session: &PaletteSession) -> Self {
        Self {
            descriptor: session.working,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::focus::{NavLayout, Wrap};
    use crate::model::{TextRef, WidgetKind};
    use tsfp_mapmaker::ui::{
        EditorSurface, EditorSurfaceGeneration, MapListGeneration, MapListRow, MapListRowId,
        MapLoadOutcome, MapMakerUiCommand, MapMakerUiState, MapMakerUiUpdate, PaletteGridChoiceId,
        PaletteGridGeneration, PalettePropertiesGeneration, PalettePropertyAction,
    };

    #[derive(Default)]
    struct Document {
        selected: Option<PaletteRecordId>,
        records: std::collections::BTreeMap<PaletteRecordId, PaletteDescriptor>,
        initialized: std::collections::BTreeSet<PaletteRecordId>,
    }

    impl PaletteDocument for Document {
        fn current_palette_record(&self) -> Option<PaletteRecordId> {
            self.selected
        }
        fn write_palette_descriptor(
            &mut self,
            id: PaletteRecordId,
            value: PaletteDescriptor,
        ) -> bool {
            let Some(record) = self.records.get_mut(&id) else {
                return false;
            };
            *record = value;
            true
        }
        fn initialize_properties_marker_if_zero(&mut self, id: PaletteRecordId) -> bool {
            self.initialized.insert(id)
        }
    }

    fn id(slot: u32) -> PaletteRecordId {
        PaletteRecordId {
            owner: 1,
            slot,
            generation: 1,
        }
    }

    fn loaded_palette_grid_state() -> (MapMakerUiState, [PaletteGridChoiceId; 2]) {
        let mut state = MapMakerUiState::default();
        let row = MapListRow {
            id: MapListRowId::new(MapListGeneration::new(4), 9),
        };
        state
            .install_local_map_rows(MapListGeneration::new(4), vec![row])
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenStorage);
        state.dispatch(MapMakerUiCommand::SelectLocalStorage);
        let MapMakerUiUpdate::LoadRequested(request) =
            state.dispatch(MapMakerUiCommand::SelectMap(row.id))
        else {
            panic!("selecting a map should request loading");
        };
        state.dispatch(MapMakerUiCommand::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(1),
                vec![EditorSurface::PaletteProperties, EditorSurface::PaletteGrid],
            )
            .unwrap();
        let properties = state
            .editor_surface_target(EditorSurface::PaletteProperties)
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenEditorSurface(properties));
        state
            .install_palette_properties_catalog(
                PalettePropertiesGeneration::new(2),
                vec![PalettePropertyAction::ChangePalette],
            )
            .unwrap();
        let change = state
            .palette_property_target(PalettePropertyAction::ChangePalette)
            .unwrap();
        assert_eq!(
            state.dispatch(MapMakerUiCommand::ActivatePaletteProperty(change)),
            MapMakerUiUpdate::PalettePageReentered
        );
        let choices = [PaletteGridChoiceId::new(41), PaletteGridChoiceId::new(42)];
        state
            .install_palette_grid_catalog(PaletteGridGeneration::new(3), choices.to_vec())
            .unwrap();
        (state, choices)
    }

    fn grid_layout(choices: [PaletteGridChoiceId; 2]) -> PaletteGridLayout {
        let mut first = Widget::new(WidgetId(10), WidgetKind::Button);
        first.text = TextRef::Dynamic;
        let mut second = Widget::new(WidgetId(11), WidgetKind::Button);
        second.text = TextRef::Dynamic;
        PaletteGridLayout {
            widget_type: 3,
            panel_bounds: None,
            nav: NavLayout::Grid(2),
            wrap: Wrap::Clamp,
            choices: vec![
                PaletteGridChoiceLayout {
                    choice: choices[0],
                    widget: first,
                },
                PaletteGridChoiceLayout {
                    choice: choices[1],
                    widget: second,
                },
            ],
        }
    }

    #[test]
    fn palette_grid_projection_binds_runtime_widgets_to_owned_choice_state() {
        let (mut state, choices) = loaded_palette_grid_state();
        let page =
            build_palette_grid_page(&state.snapshot(), PageId(0x52e000), grid_layout(choices))
                .unwrap();

        assert_eq!(page.page.widget_type, 3);
        assert_eq!(page.page.nav, NavLayout::Grid(2));
        assert_eq!(page.page.wrap, Wrap::Clamp);
        assert_eq!(page.page.initial_focus, Some(WidgetId(10)));
        assert_eq!(page.page.widgets[0].text, TextRef::Dynamic);
        assert_eq!(
            page.focus_widget(&mut state, WidgetId(11)),
            MapMakerUiUpdate::PaletteGridChoiceFocused(choices[1])
        );
        assert_eq!(
            page.activate_widget(&mut state, WidgetId(11)),
            MapMakerUiUpdate::PaletteGridChoiceActivated(choices[1])
        );
        assert_eq!(
            page.activate_widget(&mut state, WidgetId(99)),
            MapMakerUiUpdate::Ignored
        );
    }

    #[test]
    fn palette_grid_projection_rejects_wrong_page_and_changed_runtime_order() {
        let (mut state, choices) = loaded_palette_grid_state();
        state.dispatch(MapMakerUiCommand::Back);
        assert_eq!(
            build_palette_grid_page(&state.snapshot(), PageId(1), grid_layout(choices)),
            Err(PaletteGridBuildError::WrongPage)
        );

        let (state, choices) = loaded_palette_grid_state();
        let mut layout = grid_layout(choices);
        layout.choices.swap(0, 1);
        assert_eq!(
            build_palette_grid_page(&state.snapshot(), PageId(1), layout),
            Err(PaletteGridBuildError::ChoiceOrderMismatch)
        );
    }

    #[test]
    fn capture_reset_sync_and_exit_keep_identities_separate() {
        let mut session = PaletteSession::default();
        let captured = id(2);
        let current = id(9);
        let first = PaletteDescriptor([0xaabb_ccdd, 11, 22, 33, 44]);
        assert!(session.capture_linked_appearance(1, 2, captured, first));
        assert!(!session.capture_linked_appearance(2, 2, id(3), PaletteDescriptor([0; 5])));
        assert_eq!(session.working(), first);

        let mut doc = Document::default();
        doc.selected = Some(current);
        doc.records.insert(current, PaletteDescriptor([0; 5]));
        let transition = session.reset_dialog_transition(
            &mut doc,
            PaletteResetInput {
                returned_first_byte: 0x5a,
                reset_word: 0x1234_5678,
                selected_record: captured,
                selected_record_initialized: false,
            },
        );
        assert_eq!(
            transition,
            PaletteDialogTransition {
                descriptor: PALETTE_PROPERTIES_DESCRIPTOR,
                view_flag: PaletteViewFlagEffect::Clear,
                initialize_properties_marker: true,
                initialize_current_descriptor_if_zero: false,
            }
        );
        assert_eq!(
            session.working(),
            PaletteDescriptor([0xaabb_075a, 0, 0, 0x1234_5678, 0])
        );
        assert_eq!(
            session.capture_source(),
            Some(PaletteCaptureSource::LinkedAppearance(captured))
        );
        assert_eq!(
            session.synchronize_before_input(&mut doc),
            PaletteSync::Written(current)
        );
        assert_eq!(doc.records.get(&current), Some(&session.working()));
        assert_eq!(
            session.exit(PaletteExit::Properties),
            PaletteNavigation::PopCurrentPage
        );
        assert_eq!(
            PaletteSession::change_row_transition(false),
            PaletteDialogTransition {
                descriptor: PALETTE_PAGE_DESCRIPTOR,
                view_flag: PaletteViewFlagEffect::Set,
                initialize_properties_marker: false,
                initialize_current_descriptor_if_zero: true,
            }
        );
        assert_eq!(
            session.working(),
            PaletteDescriptor([0xaabb_075a, 0, 0, 0x1234_5678, 0])
        );
    }

    #[test]
    fn selected_node_presence_and_copy_flag_match_the_distinct_capture_path() {
        let mut session = PaletteSession::default();
        let old = PaletteDescriptor([1, 2, 3, 4, 5]);
        session.replace_working_copy(old);
        assert!(
            session.capture_selected_object_node(Some((id(1), PaletteDescriptor([5; 5]))), false)
        );
        assert_eq!(session.working(), old);
        assert!(!session.capture_selected_object_node(None, true));
        assert!(
            session.capture_selected_object_node(Some((id(1), PaletteDescriptor([5; 5]))), true)
        );
        assert_eq!(session.working(), PaletteDescriptor([5; 5]));
    }

    #[derive(Default)]
    struct ExitServices {
        calls: Vec<&'static str>,
        rows_changed: bool,
        entry_changed: bool,
        suppressed: bool,
    }

    impl PalettePropertiesServices for ExitServices {
        fn rerun_eligible_selected_entry_handlers(&mut self) {
            self.calls.push("rerun-handlers");
        }
        fn page_rows_changed(&mut self, _page: PageId) -> bool {
            self.calls.push("compare-rows");
            self.rows_changed
        }
        fn selected_entry_snapshot_changed(&mut self) -> bool {
            self.calls.push("compare-entry-snapshot");
            self.entry_changed
        }
        fn history_suppressed(&self) -> bool {
            self.suppressed
        }
        fn submit_history(&mut self, kind: u32) {
            assert_eq!(kind, 16);
            self.calls.push("submit-history");
        }
        fn mark_history_suppressed(&mut self) {
            self.calls.push("mark-history-suppressed");
            self.suppressed = true;
        }
        fn clear_view_suppression_flag_and_selected_entry_snapshots(&mut self) {
            self.calls.push("clear-view-flag-and-snapshots");
        }
        fn navigation_parent(&self, _page: PageId) -> u32 {
            0xabc
        }
        fn navigate(&mut self, parent: u32, descriptor: u32) {
            assert_eq!((parent, descriptor), (0xabc, 0x52ea08));
            self.calls.push("navigate");
        }
    }

    #[test]
    fn properties_exit_orders_history_before_snapshot_clear_and_navigation() {
        let mut services = ExitServices {
            rows_changed: true,
            ..Default::default()
        };
        let outcome = exit_properties_dialog(PageId(7), 0x52ea08, &mut services);
        assert_eq!(
            outcome,
            PalettePropertiesExit {
                history_submitted: true,
                descriptor: 0x52ea08,
            }
        );
        assert_eq!(
            services.calls,
            [
                "rerun-handlers",
                "compare-rows",
                "submit-history",
                "mark-history-suppressed",
                "clear-view-flag-and-snapshots",
                "navigate",
            ]
        );

        let mut services = ExitServices {
            entry_changed: true,
            suppressed: true,
            ..Default::default()
        };
        let outcome = exit_properties_dialog(PageId(7), 0x52ea08, &mut services);
        assert!(!outcome.history_submitted);
        assert_eq!(
            services.calls,
            [
                "rerun-handlers",
                "compare-rows",
                "compare-entry-snapshot",
                "clear-view-flag-and-snapshots",
                "navigate",
            ]
        );
    }
}
