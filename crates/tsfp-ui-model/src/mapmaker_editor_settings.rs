//! Renderer projection for Rust-owned Map Maker Editor Settings fields.
//!
//! The runtime page supplies ordered widgets and their presentation values.
//! This projection binds Grid Intensity, Music, Sounds, and the editor-kind
//! row to generation-qualified Rust state while retaining runtime widgets,
//! labels, and option presentation.

use crate::focus::{NavLayout, Wrap};
use crate::model::{Page, PageId, Widget, WidgetId};
use tsfp_mapmaker::ui::{
    EditorConfirmationChoice, EditorKindTarget, EditorSettingChoiceId, EditorSettingField,
    EditorSettingTarget, EditorSettingsEditEffect, EditorSettingsEditPhase, EditorSurface,
    MapMakerPage, MapMakerUiCommand, MapMakerUiSnapshot, MapMakerUiState, MapMakerUiUpdate,
};
use tsfp_ui_renderer::AnchoredRect;

#[derive(Clone, Debug, PartialEq)]
pub struct EditorSettingsWidgetLayout {
    /// `None` retains a runtime widget whose behavior is outside this slice.
    pub field: Option<EditorSettingField>,
    pub widget: Widget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorSettingsLayout {
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub nav: NavLayout,
    pub wrap: Wrap,
    /// Runtime widget used for the source-defined editor-kind choices.
    pub mode_widget: Option<WidgetId>,
    pub widgets: Vec<EditorSettingsWidgetLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorSettingsBuildError {
    WrongPage,
    MissingGeneration,
    FieldOrderMismatch,
    MissingModeWidget,
    ModeWidgetAlsoField,
    ModeEditUnavailable,
    DuplicateWidget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WidgetBinding {
    widget: WidgetId,
    target: EditorSettingTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KindBinding {
    widget: WidgetId,
    targets: Vec<EditorKindTarget>,
    requested: tsfp_mapmaker::modes::EditorKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorSettingsPage {
    pub page: Page,
    bindings: Vec<WidgetBinding>,
    kind_bindings: Vec<KindBinding>,
}

/// Runtime-owned prompt page for the mode-change clear-map decision. Text,
/// widgets, order, focus, and geometry stay with the caller's page builder.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorKindConfirmationPage {
    pub page: Page,
    generation: tsfp_mapmaker::ui::EditorSettingsGeneration,
    cancel_widget: WidgetId,
    accept_widget: WidgetId,
}

impl EditorKindConfirmationPage {
    pub fn choice_for_widget(&self, widget: WidgetId) -> Option<EditorConfirmationChoice> {
        if widget == self.cancel_widget {
            Some(EditorConfirmationChoice::Cancel)
        } else if widget == self.accept_widget {
            Some(EditorConfirmationChoice::Accept)
        } else {
            None
        }
    }

    /// Route a current prompt button into the Rust-owned mode transaction.
    /// An old prompt page is inert after the editor-settings generation or
    /// transaction phase changes.
    pub fn activate_widget(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
    ) -> Vec<MapMakerUiUpdate> {
        let snapshot = state.snapshot();
        if snapshot.page != MapMakerPage::Editor
            || snapshot.editor_surface != EditorSurface::EditorSettings
            || snapshot.editor_settings_generation != Some(self.generation)
            || snapshot.editor_settings_edit.is_none_or(|edit| {
                edit.phase() != EditorSettingsEditPhase::AwaitingClearConfirmation
            })
        {
            return Vec::new();
        }
        let effects = match self.choice_for_widget(widget) {
            Some(EditorConfirmationChoice::Cancel) => state.cancel_editor_kind_clear(),
            Some(EditorConfirmationChoice::Accept) => state.confirm_editor_kind_clear(),
            None => return Vec::new(),
        };
        effects
            .into_iter()
            .map(|effect: EditorSettingsEditEffect| {
                MapMakerUiUpdate::EditorSettingsEditEffect(effect)
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorKindConfirmationBuildError {
    WrongPage,
    MissingGeneration,
    NotAwaitingConfirmation,
    MissingCancelWidget,
    MissingAcceptWidget,
    SameChoiceWidget,
    DuplicateWidget,
}

/// Bind caller-built prompt widgets to the active Rust-owned Editor Settings
/// confirmation transaction without importing prompt text or layout.
pub fn build_editor_kind_confirmation_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    page: Page,
    cancel_widget: WidgetId,
    accept_widget: WidgetId,
) -> Result<EditorKindConfirmationPage, EditorKindConfirmationBuildError> {
    if snapshot.page != MapMakerPage::Editor
        || snapshot.editor_surface != EditorSurface::EditorSettings
    {
        return Err(EditorKindConfirmationBuildError::WrongPage);
    }
    let generation = snapshot
        .editor_settings_generation
        .ok_or(EditorKindConfirmationBuildError::MissingGeneration)?;
    if snapshot
        .editor_settings_edit
        .is_none_or(|edit| edit.phase() != EditorSettingsEditPhase::AwaitingClearConfirmation)
    {
        return Err(EditorKindConfirmationBuildError::NotAwaitingConfirmation);
    }
    if cancel_widget == accept_widget {
        return Err(EditorKindConfirmationBuildError::SameChoiceWidget);
    }
    if page.widgets.iter().enumerate().any(|(index, widget)| {
        page.widgets[..index]
            .iter()
            .any(|prior| prior.id == widget.id)
    }) {
        return Err(EditorKindConfirmationBuildError::DuplicateWidget);
    }
    if !page.widgets.iter().any(|widget| widget.id == cancel_widget) {
        return Err(EditorKindConfirmationBuildError::MissingCancelWidget);
    }
    if !page.widgets.iter().any(|widget| widget.id == accept_widget) {
        return Err(EditorKindConfirmationBuildError::MissingAcceptWidget);
    }
    Ok(EditorKindConfirmationPage {
        page,
        generation,
        cancel_widget,
        accept_widget,
    })
}

impl EditorSettingsPage {
    pub fn target_for_widget(&self, widget: WidgetId) -> Option<EditorSettingTarget> {
        self.bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.target)
    }

    /// Mode option targets retain the caller's labels and values while binding
    /// each runtime choice to the generation-qualified Rust editor-kind state.
    pub fn editor_kind_targets_for_widget(&self, widget: WidgetId) -> Option<&[EditorKindTarget]> {
        self.kind_bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.targets.as_slice())
    }

    /// Current Rust-owned working-copy value for the runtime Mode row.
    pub fn requested_editor_kind_for_widget(
        &self,
        widget: WidgetId,
    ) -> Option<tsfp_mapmaker::modes::EditorKind> {
        self.kind_bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.requested)
    }

    pub fn select_editor_kind(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
        target: EditorKindTarget,
    ) -> MapMakerUiUpdate {
        let is_bound = self
            .editor_kind_targets_for_widget(widget)
            .is_some_and(|targets| targets.contains(&target));
        if !is_bound
            || !state.contains_editor_kind_target(target)
            || !state.request_editor_kind(target.kind())
        {
            return MapMakerUiUpdate::Ignored;
        }
        MapMakerUiUpdate::EditorSettingsEditRequested(target.kind())
    }

    pub fn focus_widget(&self, state: &mut MapMakerUiState, widget: WidgetId) -> MapMakerUiUpdate {
        self.target_for_widget(widget)
            .map_or(MapMakerUiUpdate::Ignored, |target| {
                state.dispatch(MapMakerUiCommand::FocusEditorSetting(target))
            })
    }

    /// A field activation focuses it; its caller supplies the runtime choice
    /// identity through `select_choice` after native input resolves the option.
    pub fn activate_widget(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
    ) -> MapMakerUiUpdate {
        self.focus_widget(state, widget)
    }

    pub fn select_choice(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
        choice: EditorSettingChoiceId,
    ) -> MapMakerUiUpdate {
        let Some(target) = self.target_for_widget(widget) else {
            return MapMakerUiUpdate::Ignored;
        };
        state.dispatch(MapMakerUiCommand::SelectEditorSetting { target, choice })
    }
}

pub fn build_editor_settings_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    page_id: PageId,
    layout: EditorSettingsLayout,
) -> Result<EditorSettingsPage, EditorSettingsBuildError> {
    if snapshot.page != MapMakerPage::Editor
        || snapshot.editor_surface != EditorSurface::EditorSettings
    {
        return Err(EditorSettingsBuildError::WrongPage);
    }
    let generation = snapshot
        .editor_settings_generation
        .ok_or(EditorSettingsBuildError::MissingGeneration)?;
    let kind_bindings = if let Some(mode_widget) = layout.mode_widget {
        let Some(mode_row) = layout
            .widgets
            .iter()
            .find(|row| row.widget.id == mode_widget)
        else {
            return Err(EditorSettingsBuildError::MissingModeWidget);
        };
        if mode_row.field.is_some() {
            return Err(EditorSettingsBuildError::ModeWidgetAlsoField);
        }
        let Some(edit) = snapshot
            .editor_settings_edit
            .filter(|edit| edit.phase() == EditorSettingsEditPhase::Editing)
        else {
            return Err(EditorSettingsBuildError::ModeEditUnavailable);
        };
        if snapshot.editor_kind_choices.is_empty() {
            return Err(EditorSettingsBuildError::ModeEditUnavailable);
        }
        vec![KindBinding {
            widget: mode_widget,
            targets: snapshot
                .editor_kind_choices
                .iter()
                .copied()
                .map(|kind| EditorKindTarget::new(generation, kind))
                .collect(),
            requested: edit.requested(),
        }]
    } else {
        Vec::new()
    };
    let fields: Vec<_> = layout.widgets.iter().filter_map(|row| row.field).collect();
    if fields
        != snapshot
            .editor_settings
            .iter()
            .map(|row| row.field)
            .collect::<Vec<_>>()
    {
        return Err(EditorSettingsBuildError::FieldOrderMismatch);
    }

    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = layout.nav;
    page.wrap = layout.wrap;
    let mut bindings = Vec::with_capacity(fields.len());
    for row in layout.widgets {
        if page.widgets.iter().any(|widget| widget.id == row.widget.id) {
            return Err(EditorSettingsBuildError::DuplicateWidget);
        }
        if let Some(field) = row.field {
            bindings.push(WidgetBinding {
                widget: row.widget.id,
                target: EditorSettingTarget::new(generation, field),
            });
        }
        let mut widget = row.widget;
        if let Some(binding) = kind_bindings
            .iter()
            .find(|binding| binding.widget == widget.id)
        {
            if widget.options.len() != binding.targets.len() {
                return Err(EditorSettingsBuildError::ModeEditUnavailable);
            }
            let Some(selected) = binding
                .targets
                .iter()
                .position(|target| target.kind() == binding.requested)
            else {
                return Err(EditorSettingsBuildError::ModeEditUnavailable);
            };
            widget.selected = selected;
        }
        page.widgets.push(widget);
    }
    page.initial_focus = snapshot.focused_editor_setting.and_then(|focused| {
        bindings
            .iter()
            .find(|binding| binding.target.field() == focused)
            .map(|binding| binding.widget)
    });
    Ok(EditorSettingsPage {
        page,
        bindings,
        kind_bindings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::focus::{NavLayout, Wrap};
    use crate::model::{TextRef, UiOption, WidgetKind};
    use tsfp_mapmaker::modes::EditorKind;
    use tsfp_mapmaker::ui::{
        EditorKindTarget, EditorSettingRow, EditorSettingsGeneration, EditorSurfaceGeneration,
        MapListGeneration, MapListRow, MapListRowId, MapLoadOutcome,
    };

    fn editor_settings_state() -> MapMakerUiState {
        use tsfp_mapmaker::ui::{MapMakerUiCommand as C, MapMakerUiState};

        let mut state = MapMakerUiState::default();
        state.dispatch(C::OpenStorage);
        state.dispatch(C::SelectLocalStorage);
        let generation = MapListGeneration::new(1);
        let row = MapListRowId::new(generation, 9);
        state
            .install_local_map_rows(generation, vec![MapListRow { id: row }])
            .unwrap();
        let request = match state.dispatch(C::SelectMap(row)) {
            MapMakerUiUpdate::LoadRequested(request) => request,
            update => panic!("unexpected map load result: {update:?}"),
        };
        state.dispatch(C::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(2),
                vec![EditorSurface::EditorSettings],
            )
            .unwrap();
        let target = state
            .editor_surface_target(EditorSurface::EditorSettings)
            .unwrap();
        state.dispatch(C::OpenEditorSurface(target));
        state
            .install_editor_settings(
                EditorSettingsGeneration::new(3),
                vec![
                    EditorSettingRow {
                        field: EditorSettingField::GridIntensity,
                        choices: vec![EditorSettingChoiceId::new(1), EditorSettingChoiceId::new(2)],
                        selected: Some(EditorSettingChoiceId::new(1)),
                    },
                    EditorSettingRow {
                        field: EditorSettingField::Music,
                        choices: vec![EditorSettingChoiceId::new(3), EditorSettingChoiceId::new(4)],
                        selected: Some(EditorSettingChoiceId::new(3)),
                    },
                ],
            )
            .unwrap();
        assert!(state.begin_editor_settings_edit(
            EditorKind(1),
            true,
            vec![EditorKind(0), EditorKind(1)],
        ));
        state
    }

    fn widget(id: u64) -> Widget {
        let mut widget = Widget::new(WidgetId(id), WidgetKind::Cycler);
        widget.text = TextRef::Dynamic;
        widget
    }

    fn layout() -> EditorSettingsLayout {
        let mut mode = widget(11);
        mode.options = vec![
            UiOption {
                id: 0x0AFF,
                text: TextRef::Dynamic,
                enabled: true,
            },
            UiOption {
                id: 0x0B00,
                text: TextRef::Dynamic,
                enabled: true,
            },
        ];
        EditorSettingsLayout {
            widget_type: 1,
            panel_bounds: None,
            nav: NavLayout::Grid(1),
            wrap: Wrap::Clamp,
            mode_widget: Some(WidgetId(11)),
            widgets: vec![
                EditorSettingsWidgetLayout {
                    field: Some(EditorSettingField::GridIntensity),
                    widget: widget(10),
                },
                EditorSettingsWidgetLayout {
                    field: None,
                    widget: mode,
                },
                EditorSettingsWidgetLayout {
                    field: Some(EditorSettingField::Music),
                    widget: widget(12),
                },
            ],
        }
    }

    fn confirmation_state() -> MapMakerUiState {
        let mut state = editor_settings_state();
        assert!(state.request_editor_kind(EditorKind(0)));
        assert!(
            state
                .leave_editor_settings_edit()
                .contains(&EditorSettingsEditEffect::RequestClearMapConfirmation)
        );
        state
    }

    fn confirmation_page() -> Page {
        let mut page = Page::new(PageId(21), 9);
        page.widgets.push(widget(20)); // runtime-provided warning/content
        page.widgets
            .push(Widget::new(WidgetId(22), WidgetKind::Button)); // Cancel
        page.widgets
            .push(Widget::new(WidgetId(23), WidgetKind::Button)); // Yes
        page.initial_focus = Some(WidgetId(22));
        page
    }

    #[test]
    fn projection_preserves_runtime_order_and_routes_owned_choice() {
        let mut state = editor_settings_state();
        let projection =
            build_editor_settings_page(&state.snapshot(), PageId(7), layout()).unwrap();
        assert_eq!(
            projection
                .page
                .widgets
                .iter()
                .map(|widget| widget.id)
                .collect::<Vec<_>>(),
            [WidgetId(10), WidgetId(11), WidgetId(12)]
        );
        assert_eq!(projection.target_for_widget(WidgetId(11)), None);
        let mode_targets = projection
            .editor_kind_targets_for_widget(WidgetId(11))
            .unwrap();
        assert_eq!(mode_targets.len(), 2);
        assert_eq!(
            projection.requested_editor_kind_for_widget(WidgetId(11)),
            Some(EditorKind(1))
        );
        assert_eq!(projection.page.widgets[1].selected, 1);
        assert_eq!(
            projection.select_editor_kind(&mut state, WidgetId(11), mode_targets[0]),
            MapMakerUiUpdate::EditorSettingsEditRequested(EditorKind(0))
        );
        assert_eq!(
            projection.select_editor_kind(
                &mut state,
                WidgetId(11),
                EditorKindTarget::new(EditorSettingsGeneration::new(4), EditorKind(1)),
            ),
            MapMakerUiUpdate::Ignored
        );
        assert_eq!(
            state.editor_settings_edit().unwrap().requested(),
            EditorKind(0)
        );
        let updated = build_editor_settings_page(&state.snapshot(), PageId(8), layout()).unwrap();
        assert_eq!(
            updated.requested_editor_kind_for_widget(WidgetId(11)),
            Some(EditorKind(0))
        );
        assert_eq!(updated.page.widgets[1].selected, 0);
        assert_eq!(projection.page.initial_focus, Some(WidgetId(10)));
        assert_eq!(
            projection.select_choice(&mut state, WidgetId(10), EditorSettingChoiceId::new(2)),
            MapMakerUiUpdate::EditorSettingChanged {
                field: EditorSettingField::GridIntensity,
                choice: EditorSettingChoiceId::new(2),
            }
        );
    }

    #[test]
    fn projection_rejects_runtime_order_that_disagrees_with_owned_state() {
        let state = editor_settings_state();
        let mut layout = layout();
        layout.widgets.swap(0, 2);
        assert_eq!(
            build_editor_settings_page(&state.snapshot(), PageId(7), layout).unwrap_err(),
            EditorSettingsBuildError::FieldOrderMismatch
        );
    }

    #[test]
    fn clear_map_confirmation_projection_applies_cancel_and_accept_effects() {
        let mut state = confirmation_state();
        let projection = build_editor_kind_confirmation_page(
            &state.snapshot(),
            confirmation_page(),
            WidgetId(22),
            WidgetId(23),
        )
        .unwrap();
        assert_eq!(projection.page.initial_focus, Some(WidgetId(22)));
        assert_eq!(
            projection.choice_for_widget(WidgetId(22)),
            Some(EditorConfirmationChoice::Cancel)
        );
        assert_eq!(
            projection.activate_widget(&mut state, WidgetId(22)),
            [MapMakerUiUpdate::EditorSettingsEditEffect(
                EditorSettingsEditEffect::DismissClearMapConfirmation
            )]
        );
        assert_eq!(
            state.editor_settings_edit().unwrap().phase(),
            EditorSettingsEditPhase::Editing
        );

        let mut state = confirmation_state();
        let projection = build_editor_kind_confirmation_page(
            &state.snapshot(),
            confirmation_page(),
            WidgetId(22),
            WidgetId(23),
        )
        .unwrap();
        assert_eq!(
            projection.activate_widget(&mut state, WidgetId(23)),
            [
                MapMakerUiUpdate::EditorSettingsEditEffect(EditorSettingsEditEffect::InitializeMap),
                MapMakerUiUpdate::EditorSettingsEditEffect(
                    EditorSettingsEditEffect::PublishRequestedKind(EditorKind(0))
                ),
                MapMakerUiUpdate::EditorSettingsEditEffect(EditorSettingsEditEffect::CloseSettings),
            ]
        );
    }

    #[test]
    fn clear_map_confirmation_projection_rejects_stale_page_and_targets() {
        let mut state = confirmation_state();
        let projection = build_editor_kind_confirmation_page(
            &state.snapshot(),
            confirmation_page(),
            WidgetId(22),
            WidgetId(23),
        )
        .unwrap();
        assert!(
            projection
                .activate_widget(&mut state, WidgetId(99))
                .is_empty()
        );
        state.dispatch(MapMakerUiCommand::Back);
        assert!(
            projection
                .activate_widget(&mut state, WidgetId(23))
                .is_empty()
        );

        let editing = editor_settings_state();
        assert_eq!(
            build_editor_kind_confirmation_page(
                &editing.snapshot(),
                confirmation_page(),
                WidgetId(22),
                WidgetId(23),
            )
            .unwrap_err(),
            EditorKindConfirmationBuildError::NotAwaitingConfirmation
        );
    }
}
