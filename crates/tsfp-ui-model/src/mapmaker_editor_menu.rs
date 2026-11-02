//! Renderer projection for the Rust-owned loaded Map Maker editor menu.
//!
//! The runtime supplies localized widgets and geometry. This layer preserves
//! those values while binding actionable rows to generation-qualified Rust
//! menu identities; it does not call guest callbacks.

use crate::focus::{NavLayout, Wrap};
use crate::model::{Page, PageId, Widget, WidgetId};
use tsfp_mapmaker::ui::{
    EditorMenuAction, EditorMenuTarget, EditorSurface, MapMakerPage, MapMakerUiCommand,
    MapMakerUiSnapshot, MapMakerUiState, MapMakerUiUpdate,
};
use tsfp_ui_renderer::AnchoredRect;

#[derive(Clone, Debug, PartialEq)]
pub struct EditorMenuWidgetLayout {
    /// `None` retains a runtime heading or other non-action widget.
    pub action: Option<EditorMenuAction>,
    pub widget: Widget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorMenuLayout {
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub nav: NavLayout,
    pub wrap: Wrap,
    pub widgets: Vec<EditorMenuWidgetLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorMenuBuildError {
    WrongPage,
    MissingGeneration,
    ConfirmationOpen,
    ActionOrderMismatch,
    DuplicateWidget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WidgetBinding {
    widget: WidgetId,
    target: EditorMenuTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorMenuPage {
    pub page: Page,
    bindings: Vec<WidgetBinding>,
}

impl EditorMenuPage {
    pub fn target_for_widget(&self, widget: WidgetId) -> Option<EditorMenuTarget> {
        self.bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.target)
    }

    pub fn focus_widget(&self, state: &mut MapMakerUiState, widget: WidgetId) -> MapMakerUiUpdate {
        self.target_for_widget(widget)
            .map_or(MapMakerUiUpdate::Ignored, |target| {
                state.dispatch(MapMakerUiCommand::FocusEditorMenuAction(target))
            })
    }

    pub fn activate_widget(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
    ) -> MapMakerUiUpdate {
        self.target_for_widget(widget)
            .map_or(MapMakerUiUpdate::Ignored, |target| {
                state.dispatch(MapMakerUiCommand::ActivateEditorMenuAction(target))
            })
    }
}

pub fn build_editor_menu_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    page_id: PageId,
    layout: EditorMenuLayout,
) -> Result<EditorMenuPage, EditorMenuBuildError> {
    if snapshot.page != MapMakerPage::Editor || snapshot.editor_surface != EditorSurface::MainMenu {
        return Err(EditorMenuBuildError::WrongPage);
    }
    let generation = snapshot
        .editor_menu_generation
        .ok_or(EditorMenuBuildError::MissingGeneration)?;
    if snapshot.editor_confirmation.is_some() {
        return Err(EditorMenuBuildError::ConfirmationOpen);
    }

    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = layout.nav;
    page.wrap = layout.wrap;
    let mut bindings = Vec::new();
    let mut actions = Vec::new();
    for item in layout.widgets {
        if page
            .widgets
            .iter()
            .any(|widget| widget.id == item.widget.id)
        {
            return Err(EditorMenuBuildError::DuplicateWidget);
        }
        let mut widget = item.widget;
        if let Some(availability) = snapshot.editor_history_availability {
            match item.action {
                Some(EditorMenuAction::Undo) => widget.enabled &= availability.can_undo,
                Some(EditorMenuAction::Redo) => widget.enabled &= availability.can_redo,
                _ => {}
            }
        }
        if let Some(action) = item.action {
            actions.push(action);
            bindings.push(WidgetBinding {
                widget: widget.id,
                target: EditorMenuTarget::new(generation, action),
            });
        }
        page.widgets.push(widget);
    }
    if actions != snapshot.editor_menu_actions {
        return Err(EditorMenuBuildError::ActionOrderMismatch);
    }
    page.initial_focus = snapshot.focused_editor_menu_action.and_then(|focused| {
        bindings
            .iter()
            .find(|binding| binding.target.action() == focused)
            .map(|binding| binding.widget)
    });
    Ok(EditorMenuPage { page, bindings })
}

#[cfg(test)]
mod tests {
    use super::{
        EditorMenuBuildError, EditorMenuLayout, EditorMenuWidgetLayout, build_editor_menu_page,
    };
    use crate::focus::{NavLayout, Wrap};
    use crate::model::{PageId, TextId, TextRef, Widget, WidgetId, WidgetKind, menu_rect};
    use tsfp_mapmaker::history::HistoryAvailability;
    use tsfp_mapmaker::ui::{
        EditorMenuAction, EditorMenuGeneration, EditorSurface, EditorSurfaceGeneration,
        MapListGeneration, MapListRow, MapListRowId, MapLoadOutcome, MapMakerUiCommand,
        MapMakerUiState, MapMakerUiUpdate,
    };

    fn editor_state() -> MapMakerUiState {
        let mut state = MapMakerUiState::default();
        let row = MapListRowId::new(MapListGeneration::new(3), 8);
        state
            .install_local_map_rows(MapListGeneration::new(3), vec![MapListRow { id: row }])
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenStorage);
        state.dispatch(MapMakerUiCommand::SelectLocalStorage);
        let MapMakerUiUpdate::LoadRequested(request) =
            state.dispatch(MapMakerUiCommand::SelectMap(row))
        else {
            panic!("selecting a local map should create an owned load request");
        };
        state.dispatch(MapMakerUiCommand::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(10),
                vec![EditorSurface::EditorSettings],
            )
            .unwrap();
        state
            .install_original_editor_menu(EditorMenuGeneration::new(11))
            .unwrap();
        state
    }

    fn widget(id: u64, label: &'static str) -> Widget {
        let mut widget = Widget::new(WidgetId(id), WidgetKind::Button);
        widget.rect = menu_rect(
            tsfp_ui_renderer::Vec2::new(20.0, id as f32 * 10.0),
            tsfp_ui_renderer::Vec2::new(200.0, 8.0),
        );
        widget.text = TextRef::Id(TextId(label.len() as u32));
        widget
    }

    fn layout(actions: &[EditorMenuAction]) -> EditorMenuLayout {
        let mut widgets = vec![EditorMenuWidgetLayout {
            action: None,
            widget: widget(0, "Edit"),
        }];
        widgets.extend(
            actions
                .iter()
                .enumerate()
                .map(|(index, action)| EditorMenuWidgetLayout {
                    action: Some(*action),
                    widget: widget(index as u64 + 1, "Runtime label"),
                }),
        );
        EditorMenuLayout {
            widget_type: 1,
            panel_bounds: None,
            nav: NavLayout::Grid(1),
            wrap: Wrap::Clamp,
            widgets,
        }
    }

    #[test]
    fn editor_menu_projection_preserves_runtime_widgets_and_dispatches_typed_actions() {
        let mut state = editor_state();
        let actions = state.editor_menu_actions().to_vec();
        let projection =
            build_editor_menu_page(&state.snapshot(), PageId(7), layout(&actions)).unwrap();
        assert_eq!(projection.page.widgets.len(), actions.len() + 1);
        assert_eq!(projection.page.widgets[0].text, TextRef::Id(TextId(4)));
        assert_eq!(projection.page.widgets[1].text, TextRef::Id(TextId(13)));
        assert_eq!(projection.page.widgets[1].rect.offset.x, 20.0);
        assert!(projection.target_for_widget(WidgetId(0)).is_none());

        assert_eq!(
            projection.focus_widget(&mut state, WidgetId(5)),
            MapMakerUiUpdate::EditorMenuActionFocused(EditorMenuAction::EditorSettings)
        );
        assert_eq!(
            projection.activate_widget(&mut state, WidgetId(5)),
            MapMakerUiUpdate::EditorMenuActionActivated(EditorMenuAction::EditorSettings)
        );
    }

    #[test]
    fn editor_menu_projects_only_published_history_availability() {
        let mut state = editor_state();
        let actions = state.editor_menu_actions().to_vec();

        let untouched =
            build_editor_menu_page(&state.snapshot(), PageId(8), layout(&actions)).unwrap();
        assert!(untouched.page.widgets[2].enabled);
        assert!(untouched.page.widgets[3].enabled);

        state.set_editor_history_availability(Some(HistoryAvailability {
            can_undo: false,
            can_redo: true,
        }));
        let projected =
            build_editor_menu_page(&state.snapshot(), PageId(9), layout(&actions)).unwrap();
        assert!(!projected.page.widgets[2].enabled);
        assert!(projected.page.widgets[3].enabled);
        assert_eq!(
            projected.activate_widget(&mut state, WidgetId(2)),
            MapMakerUiUpdate::Ignored
        );
        assert_eq!(
            projected.activate_widget(&mut state, WidgetId(3)),
            MapMakerUiUpdate::EditorHistoryRequested(
                tsfp_mapmaker::ui::EditorHistoryOperation::Redo
            )
        );
    }

    #[test]
    fn editor_menu_projection_rejects_reordered_and_duplicate_runtime_widgets() {
        let state = editor_state();
        let mut reordered = state.editor_menu_actions().to_vec();
        reordered.swap(0, 1);
        assert_eq!(
            build_editor_menu_page(&state.snapshot(), PageId(7), layout(&reordered)),
            Err(EditorMenuBuildError::ActionOrderMismatch)
        );

        let mut duplicate = layout(state.editor_menu_actions());
        duplicate.widgets[1].widget.id = WidgetId(0);
        assert_eq!(
            build_editor_menu_page(&state.snapshot(), PageId(7), duplicate),
            Err(EditorMenuBuildError::DuplicateWidget)
        );
    }

    #[test]
    fn editor_menu_targets_from_an_old_page_generation_are_inert() {
        let mut state = editor_state();
        let projection = build_editor_menu_page(
            &state.snapshot(),
            PageId(7),
            layout(state.editor_menu_actions()),
        )
        .unwrap();
        state
            .install_original_editor_menu(EditorMenuGeneration::new(12))
            .unwrap();
        assert_eq!(
            projection.activate_widget(&mut state, WidgetId(5)),
            MapMakerUiUpdate::Ignored
        );
        assert_eq!(
            state.editor_surface(),
            tsfp_mapmaker::ui::EditorSurface::MainMenu
        );
    }
}
