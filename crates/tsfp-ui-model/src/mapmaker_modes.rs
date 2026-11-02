//! Host-free presentation of the Rust-owned Map Maker mode-selection state.
//!
//! Runtime callers provide row labels and measured geometry. This module maps
//! page-local widget identities back to opaque mode-choice identities without
//! assigning either identity from the other's numeric domain.

use crate::model::{
    BindingId, HandlerRef, Page, PageId, TextId, TextRef, Widget, WidgetId, WidgetKind,
};
use crate::{focus::NavLayout, focus::Wrap};
use tsfp_mapmaker::modes::{LaunchConfiguration, LaunchInput, PLAYER_SLOTS, ParticipantRoster};
use tsfp_mapmaker::ui::{
    MapMakerPage, MapMakerUiCommand, MapMakerUiSnapshot, MapMakerUiState, MapMakerUiUpdate,
    ModeChoiceId,
};
use tsfp_ui_renderer::AnchoredRect;

const MODE_CHOICE_BINDING: &str = "mapmaker.mode-choice";
const MODE_CHOICE_HANDLER: &str = "mapmaker.select-mode";

/// Caller-owned label and measured geometry for one runtime mode row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeChoiceRowLayout {
    pub choice: ModeChoiceId,
    pub label: TextId,
    pub rect: AnchoredRect,
    /// Optional clipped part of a scrollable row, in the same anchor space.
    pub bounds: Option<AnchoredRect>,
}

/// Original page family and geometry supplied by the runtime page/data layer.
#[derive(Clone, Debug, PartialEq)]
pub struct ModeSelectionLayout {
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    /// Rows must be in the same order as `MapMakerUiSnapshot::mode_choices`.
    pub rows: Vec<ModeChoiceRowLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeSelectionBuildError {
    WrongPage,
    RowCountMismatch,
    RowIdentityMismatch,
}

/// Mapping between generated page-local widgets and runtime mode identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeChoiceWidgetBinding {
    pub widget: WidgetId,
    pub choice: ModeChoiceId,
}

/// Renderable mode page plus the typed route from activated rows to state IDs.
#[derive(Clone, Debug, PartialEq)]
pub struct ModeSelectionPage {
    pub page: Page,
    bindings: Vec<ModeChoiceWidgetBinding>,
}

impl ModeSelectionPage {
    pub fn bindings(&self) -> &[ModeChoiceWidgetBinding] {
        &self.bindings
    }

    pub fn choice_for_widget(&self, widget: WidgetId) -> Option<ModeChoiceId> {
        self.bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.choice)
    }

    /// Route retained-mode focus into the Rust-owned page state. The state
    /// checks that the row still belongs to the current runtime catalog.
    pub fn focus_widget(&self, state: &mut MapMakerUiState, widget: WidgetId) -> MapMakerUiUpdate {
        let Some(choice) = self.choice_for_widget(widget) else {
            return MapMakerUiUpdate::Ignored;
        };
        state.dispatch(MapMakerUiCommand::FocusMode(choice))
    }

    /// Route a retained-mode widget activation into the Rust-owned page state.
    /// The state revalidates both its current page and the stable mode identity,
    /// so a stale page or row cannot select a removed choice.
    pub fn activate_widget(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
    ) -> MapMakerUiUpdate {
        let Some(choice) = self.choice_for_widget(widget) else {
            return MapMakerUiUpdate::Ignored;
        };
        state.dispatch(MapMakerUiCommand::SelectMode(choice))
    }
}

/// Build the renderer-facing list from the owned state snapshot and caller-provided data.
///
/// Page IDs, row text, page family, and geometry are supplied by the runtime
/// registry/data layer; no guest descriptor or fixed row mapping is guessed.
pub fn build_mode_selection_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    page_id: PageId,
    layout: ModeSelectionLayout,
) -> Result<ModeSelectionPage, ModeSelectionBuildError> {
    if snapshot.page != MapMakerPage::ModeSelection {
        return Err(ModeSelectionBuildError::WrongPage);
    }
    if layout.rows.len() != snapshot.mode_choices.len() {
        return Err(ModeSelectionBuildError::RowCountMismatch);
    }
    if !layout
        .rows
        .iter()
        .zip(snapshot.mode_choices)
        .all(|(row, choice)| row.choice == choice.id)
    {
        return Err(ModeSelectionBuildError::RowIdentityMismatch);
    }

    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = NavLayout::Grid(1);
    // This matches the mode input adapter's inferred stop-at-the-list-ends policy.
    page.wrap = Wrap::Clamp;
    let mut bindings = Vec::with_capacity(layout.rows.len());

    for (index, row) in layout.rows.into_iter().enumerate() {
        let widget_id = WidgetId(index as u64 + 1);
        let mut widget = Widget::new(widget_id, WidgetKind::Button);
        widget.rect = row.rect;
        widget.bounds = row.bounds;
        widget.text = TextRef::Id(row.label);
        widget.bound = Some(BindingId(MODE_CHOICE_BINDING.to_owned()));
        widget.activate = Some(HandlerRef::Named(MODE_CHOICE_HANDLER.to_owned()));
        page.widgets.push(widget);
        bindings.push(ModeChoiceWidgetBinding {
            widget: widget_id,
            choice: row.choice,
        });
    }

    page.initial_focus = snapshot.focused_mode.and_then(|focused| {
        bindings
            .iter()
            .find(|binding| binding.choice == focused)
            .map(|binding| binding.widget)
    });

    Ok(ModeSelectionPage { page, bindings })
}

/// Runtime-owned services needed after the preview gate approves a launch.
/// Values are already typed outputs from native roster, player-count, team,
/// and RNG services; this adapter never reads guest memory or invokes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewLaunchInputs {
    pub local_player_count: u8,
    pub setup_count: i8,
    pub launch_argument_missing: bool,
    pub team_values: [u8; PLAYER_SLOTS],
    pub random_draws: [u32; 2],
    pub participants: Option<ParticipantRoster>,
}

/// Failure to turn a reducer-approved preview launch into owned game state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewLaunchBuildError {
    NotLaunchRequested,
    WrongPage,
    MissingSelectedMode,
    StalePreviewRow,
}

/// Build the owned launch record input only from an update emitted by the
/// preview reducer. T1688's existing constructor owns variant and roster
/// policy; this seam supplies its selected mode and typed runtime services.
pub fn build_preview_launch_configuration(
    state: &MapMakerUiState,
    update: MapMakerUiUpdate,
    inputs: PreviewLaunchInputs,
) -> Result<LaunchConfiguration, PreviewLaunchBuildError> {
    let MapMakerUiUpdate::PreviewLaunchRequested(row) = update else {
        return Err(PreviewLaunchBuildError::NotLaunchRequested);
    };
    if state.page() != MapMakerPage::Preview {
        return Err(PreviewLaunchBuildError::WrongPage);
    }
    if state.preview_row() != Some(row) {
        return Err(PreviewLaunchBuildError::StalePreviewRow);
    }
    let Some(mode) = state.selected_mode() else {
        return Err(PreviewLaunchBuildError::MissingSelectedMode);
    };
    if mode.settings().preview.row != row {
        return Err(PreviewLaunchBuildError::StalePreviewRow);
    }

    let launch_input = LaunchInput {
        mode: mode.clone(),
        local_player_count: inputs.local_player_count,
        setup_count: inputs.setup_count,
        launch_argument_missing: inputs.launch_argument_missing,
        team_values: inputs.team_values,
        random_draws: inputs.random_draws,
    };
    Ok(LaunchConfiguration::build(
        &launch_input,
        inputs.participants.as_ref(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{WidgetRole, menu_rect};
    use tsfp_mapmaker::modes::{
        AssaultModeConfiguration, EditorKind, GameType, LaunchVariant, ModeConfiguration,
        ModeSettings, PreviewRow, PreviewSelection, ThemeId,
    };
    use tsfp_mapmaker::ui::{MapMakerUiCommand, MapMakerUiState, ModeChoice};
    use tsfp_ui_renderer::Vec2;

    fn configuration(seed: u32) -> ModeConfiguration {
        ModeConfiguration::Assault(AssaultModeConfiguration {
            settings: ModeSettings {
                editor_kind: EditorKind(seed),
                theme: ThemeId(seed),
                preview: PreviewSelection {
                    row: PreviewRow(seed as u8),
                    variant: LaunchVariant(seed),
                    game_type: GameType(seed as u8),
                },
                row_options: 0,
                duration: 1,
            },
            phases: Default::default(),
        })
    }

    fn active_snapshot() -> (MapMakerUiState, ModeChoiceId, ModeChoiceId) {
        let first = ModeChoiceId::new(0xf001);
        let second = ModeChoiceId::new(0xf002);
        let mut state = MapMakerUiState::default();
        state
            .install_mode_choices(vec![
                ModeChoice {
                    id: first,
                    configuration: configuration(1),
                },
                ModeChoice {
                    id: second,
                    configuration: configuration(2),
                },
            ])
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenModeSelection);
        state.dispatch(MapMakerUiCommand::FocusMode(second));
        (state, first, second)
    }

    fn preview_state() -> (MapMakerUiState, PreviewRow) {
        let (mut state, _, selected) = active_snapshot();
        state.dispatch(MapMakerUiCommand::SelectMode(selected));
        state.dispatch(MapMakerUiCommand::OpenPreview);
        (state, PreviewRow(2))
    }

    fn preview_launch_inputs() -> PreviewLaunchInputs {
        PreviewLaunchInputs {
            local_player_count: 2,
            setup_count: 0,
            launch_argument_missing: false,
            team_values: [0; PLAYER_SLOTS],
            random_draws: [0; 2],
            participants: Some(ParticipantRoster::empty()),
        }
    }

    fn row(choice: ModeChoiceId, label: u32, y: f32) -> ModeChoiceRowLayout {
        ModeChoiceRowLayout {
            choice,
            label: TextId(label),
            rect: menu_rect(Vec2::new(20.0, y), Vec2::new(180.0, 24.0)),
            bounds: None,
        }
    }

    #[test]
    fn mode_page_keeps_choice_ids_separate_from_page_widget_ids() {
        let (state, first, second) = active_snapshot();
        let snapshot = state.snapshot();
        let built = build_mode_selection_page(
            &snapshot,
            PageId(0xfeed),
            ModeSelectionLayout {
                widget_type: 17,
                panel_bounds: None,
                rows: vec![row(first, 51, 40.0), row(second, 52, 68.0)],
            },
        )
        .unwrap();

        assert_eq!(built.page.id, PageId(0xfeed));
        assert_eq!(built.page.widget_type, 17);
        assert_eq!(built.page.nav, NavLayout::Grid(1));
        assert_eq!(built.page.wrap, Wrap::Clamp);
        assert_eq!(built.page.initial_focus, Some(WidgetId(2)));
        assert_eq!(built.page.start_focus(), Some(WidgetId(2)));
        assert_eq!(built.page.widgets[0].text, TextRef::Id(TextId(51)));
        assert_eq!(built.page.widgets[0].role, WidgetRole::Plain);
        assert_eq!(built.choice_for_widget(WidgetId(1)), Some(first));
        assert_eq!(built.choice_for_widget(WidgetId(2)), Some(second));
        assert_eq!(built.choice_for_widget(WidgetId(first.get())), None);
    }

    #[test]
    fn activated_widget_selects_its_owned_mode_choice() {
        let (mut state, first, second) = active_snapshot();
        let snapshot = state.snapshot();
        let built = build_mode_selection_page(
            &snapshot,
            PageId(0xfeed),
            ModeSelectionLayout {
                widget_type: 17,
                panel_bounds: None,
                rows: vec![row(first, 51, 40.0), row(second, 52, 68.0)],
            },
        )
        .unwrap();

        assert_eq!(
            built.activate_widget(&mut state, WidgetId(1)),
            MapMakerUiUpdate::PageChanged(MapMakerPage::ModeSettings)
        );
        assert_eq!(state.selected_mode_id(), Some(first));
        assert_eq!(state.selected_mode(), Some(&configuration(1)));
    }

    #[test]
    fn focused_widget_updates_owned_focus_and_rejects_stale_rows() {
        let (mut state, first, second) = active_snapshot();
        let built = build_mode_selection_page(
            &state.snapshot(),
            PageId(0xfeed),
            ModeSelectionLayout {
                widget_type: 17,
                panel_bounds: None,
                rows: vec![row(first, 51, 40.0), row(second, 52, 68.0)],
            },
        )
        .unwrap();

        assert_eq!(state.focused_mode(), Some(second));
        assert_eq!(
            built.focus_widget(&mut state, WidgetId(1)),
            MapMakerUiUpdate::ModeFocused(first)
        );
        assert_eq!(state.focused_mode(), Some(first));
        state
            .install_mode_choices(vec![ModeChoice {
                id: second,
                configuration: configuration(2),
            }])
            .unwrap();
        assert_eq!(
            built.focus_widget(&mut state, WidgetId(1)),
            MapMakerUiUpdate::Ignored
        );
        assert_eq!(state.focused_mode(), None);
        assert_eq!(
            built.focus_widget(&mut state, WidgetId(99)),
            MapMakerUiUpdate::Ignored
        );
    }

    #[test]
    fn activated_widget_ignores_unknown_and_stale_rows() {
        let (mut state, first, second) = active_snapshot();
        let snapshot = state.snapshot();
        let built = build_mode_selection_page(
            &snapshot,
            PageId(0xfeed),
            ModeSelectionLayout {
                widget_type: 17,
                panel_bounds: None,
                rows: vec![row(first, 51, 40.0), row(second, 52, 68.0)],
            },
        )
        .unwrap();

        assert_eq!(
            built.activate_widget(&mut state, WidgetId(99)),
            MapMakerUiUpdate::Ignored
        );
        state
            .install_mode_choices(vec![ModeChoice {
                id: second,
                configuration: configuration(2),
            }])
            .unwrap();
        assert_eq!(
            built.activate_widget(&mut state, WidgetId(1)),
            MapMakerUiUpdate::Ignored
        );
        assert_eq!(state.page(), MapMakerPage::ModeSelection);
        assert_eq!(state.selected_mode_id(), None);
    }

    #[test]
    fn mode_page_rejects_stale_or_reordered_runtime_rows() {
        let (state, first, second) = active_snapshot();
        let snapshot = state.snapshot();
        let layout = ModeSelectionLayout {
            widget_type: 17,
            panel_bounds: None,
            rows: vec![row(second, 52, 40.0), row(first, 51, 68.0)],
        };
        assert_eq!(
            build_mode_selection_page(&snapshot, PageId(9), layout),
            Err(ModeSelectionBuildError::RowIdentityMismatch)
        );
    }

    #[test]
    fn mode_page_is_guarded_to_the_owned_mode_selection_page() {
        let state = MapMakerUiState::default();
        let snapshot = state.snapshot();
        assert_eq!(
            build_mode_selection_page(
                &snapshot,
                PageId(9),
                ModeSelectionLayout {
                    widget_type: 17,
                    panel_bounds: None,
                    rows: Vec::new(),
                },
            ),
            Err(ModeSelectionBuildError::WrongPage)
        );
    }

    #[test]
    fn preview_launch_approved_update_builds_from_selected_typed_mode() {
        let (state, row) = preview_state();
        let launch = build_preview_launch_configuration(
            &state,
            MapMakerUiUpdate::PreviewLaunchRequested(row),
            preview_launch_inputs(),
        )
        .unwrap();

        assert_eq!(launch.local_slots, 3);
        assert_eq!(launch.game_type, GameType(2));
        assert_eq!(launch.variant, 2);
        assert_eq!(launch.participants.ids[0], 0x0c);
        assert_eq!(launch.state, 0x64);
    }

    #[test]
    fn preview_launch_rejects_unapproved_or_stale_requests() {
        let (state, row) = preview_state();
        assert_eq!(
            build_preview_launch_configuration(
                &state,
                MapMakerUiUpdate::Ignored,
                preview_launch_inputs(),
            ),
            Err(PreviewLaunchBuildError::NotLaunchRequested)
        );
        assert_eq!(
            build_preview_launch_configuration(
                &state,
                MapMakerUiUpdate::PreviewLaunchRequested(PreviewRow(row.0 + 1)),
                preview_launch_inputs(),
            ),
            Err(PreviewLaunchBuildError::StalePreviewRow)
        );
    }

    #[test]
    fn preview_launch_requires_the_current_preview_page() {
        let (state, _, _) = active_snapshot();
        assert_eq!(
            build_preview_launch_configuration(
                &state,
                MapMakerUiUpdate::PreviewLaunchRequested(PreviewRow(2)),
                preview_launch_inputs(),
            ),
            Err(PreviewLaunchBuildError::WrongPage)
        );
    }
}
