//! Rust-owned presentation boundary for the loaded Map Maker Preview list.
//!
//! Runtime labels, ordering, widget data and activation effects remain with
//! their existing services; the Rust page owns focus and typed choice identity.

use crate::focus::{NavLayout, Wrap};
use crate::model::{Page, PageId, Widget, WidgetId};
use tsfp_mapmaker::ui::{
    EditorPreviewChoiceId, EditorPreviewChoiceTarget, EditorSurface, MapMakerPage,
    MapMakerUiCommand, MapMakerUiSnapshot, MapMakerUiState, MapMakerUiUpdate,
};
use tsfp_ui_renderer::AnchoredRect;

#[derive(Clone, Debug, PartialEq)]
pub struct EditorPreviewChoiceLayout {
    pub choice: EditorPreviewChoiceId,
    pub widget: Widget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorPreviewLayout {
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub nav: NavLayout,
    pub wrap: Wrap,
    pub choices: Vec<EditorPreviewChoiceLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorPreviewBuildError {
    WrongPage,
    MissingGeneration,
    ChoiceCountMismatch,
    ChoiceOrderMismatch,
    DuplicateWidget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EditorPreviewWidgetBinding {
    widget: WidgetId,
    target: EditorPreviewChoiceTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorPreviewPage {
    pub page: Page,
    bindings: Vec<EditorPreviewWidgetBinding>,
}

impl EditorPreviewPage {
    pub fn target_for_widget(&self, widget: WidgetId) -> Option<EditorPreviewChoiceTarget> {
        self.bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.target)
    }

    pub fn focus_widget(&self, state: &mut MapMakerUiState, widget: WidgetId) -> MapMakerUiUpdate {
        self.target_for_widget(widget)
            .map_or(MapMakerUiUpdate::Ignored, |target| {
                state.dispatch(MapMakerUiCommand::FocusEditorPreviewChoice(target))
            })
    }

    pub fn activate_widget(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
    ) -> MapMakerUiUpdate {
        self.target_for_widget(widget)
            .map_or(MapMakerUiUpdate::Ignored, |target| {
                state.dispatch(MapMakerUiCommand::ActivateEditorPreviewChoice(target))
            })
    }
}

pub fn build_editor_preview_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    page_id: PageId,
    layout: EditorPreviewLayout,
) -> Result<EditorPreviewPage, EditorPreviewBuildError> {
    if snapshot.page != MapMakerPage::Editor || snapshot.editor_surface != EditorSurface::MapPreview
    {
        return Err(EditorPreviewBuildError::WrongPage);
    }
    let generation = snapshot
        .editor_preview_generation
        .ok_or(EditorPreviewBuildError::MissingGeneration)?;
    if layout.choices.len() != snapshot.editor_preview_choices.len() {
        return Err(EditorPreviewBuildError::ChoiceCountMismatch);
    }

    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = layout.nav;
    page.wrap = layout.wrap;
    let mut bindings = Vec::with_capacity(layout.choices.len());
    for (index, row) in layout.choices.into_iter().enumerate() {
        if snapshot.editor_preview_choices.get(index) != Some(&row.choice) {
            return Err(EditorPreviewBuildError::ChoiceOrderMismatch);
        }
        if page.widgets.iter().any(|widget| widget.id == row.widget.id) {
            return Err(EditorPreviewBuildError::DuplicateWidget);
        }
        page.widgets.push(row.widget.clone());
        bindings.push(EditorPreviewWidgetBinding {
            widget: row.widget.id,
            target: EditorPreviewChoiceTarget::new(generation, row.choice),
        });
    }
    page.initial_focus = snapshot.focused_editor_preview_choice.and_then(|focused| {
        bindings
            .iter()
            .find(|binding| binding.target.choice() == focused)
            .map(|binding| binding.widget)
    });
    Ok(EditorPreviewPage { page, bindings })
}

#[cfg(test)]
mod tests {
    use super::{
        EditorPreviewBuildError, EditorPreviewChoiceLayout, EditorPreviewLayout,
        build_editor_preview_page,
    };
    use crate::focus::{NavLayout, Wrap};
    use crate::model::{PageId, TextRef, Widget, WidgetId, WidgetKind};
    use tsfp_mapmaker::ui::{
        EditorPreviewChoiceId, EditorPreviewGeneration, EditorSurface, EditorSurfaceGeneration,
        MapListGeneration, MapListRow, MapListRowId, MapLoadOutcome, MapMakerUiCommand,
        MapMakerUiState, MapMakerUiUpdate,
    };

    fn loaded_preview_state() -> (MapMakerUiState, [EditorPreviewChoiceId; 2]) {
        let mut state = MapMakerUiState::default();
        let row = MapListRowId::new(MapListGeneration::new(4), 8);
        state
            .install_local_map_rows(MapListGeneration::new(4), vec![MapListRow { id: row }])
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenStorage);
        state.dispatch(MapMakerUiCommand::SelectLocalStorage);
        let MapMakerUiUpdate::LoadRequested(request) =
            state.dispatch(MapMakerUiCommand::SelectMap(row))
        else {
            panic!("selecting a map should request loading");
        };
        state.dispatch(MapMakerUiCommand::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(6),
                vec![EditorSurface::MapPreview],
            )
            .unwrap();
        let target = state
            .editor_surface_target(EditorSurface::MapPreview)
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenEditorSurface(target));
        let choices = [
            EditorPreviewChoiceId::new(10),
            EditorPreviewChoiceId::new(11),
        ];
        state
            .install_editor_preview_catalog(EditorPreviewGeneration::new(7), choices.to_vec())
            .unwrap();
        (state, choices)
    }

    fn layout(choices: [EditorPreviewChoiceId; 2]) -> EditorPreviewLayout {
        let mut first = Widget::new(WidgetId(40), WidgetKind::Button);
        first.text = TextRef::Dynamic;
        let mut second = Widget::new(WidgetId(41), WidgetKind::Button);
        second.text = TextRef::Dynamic;
        EditorPreviewLayout {
            widget_type: 1,
            panel_bounds: None,
            nav: NavLayout::Grid(2),
            wrap: Wrap::Clamp,
            choices: vec![
                EditorPreviewChoiceLayout {
                    choice: choices[0],
                    widget: first,
                },
                EditorPreviewChoiceLayout {
                    choice: choices[1],
                    widget: second,
                },
            ],
        }
    }

    #[test]
    fn preview_page_binds_runtime_widgets_to_owned_choices() {
        let (mut state, choices) = loaded_preview_state();
        let built = build_editor_preview_page(&state.snapshot(), PageId(0x52d494), layout(choices))
            .unwrap();
        assert_eq!(built.page.nav, NavLayout::Grid(2));
        assert_eq!(built.page.wrap, Wrap::Clamp);
        assert_eq!(built.page.initial_focus, Some(WidgetId(40)));
        assert_eq!(built.page.widgets[0].text, TextRef::Dynamic);
        assert_eq!(
            built.focus_widget(&mut state, WidgetId(41)),
            MapMakerUiUpdate::EditorPreviewChoiceFocused(choices[1])
        );
        assert_eq!(
            built.activate_widget(&mut state, WidgetId(41)),
            MapMakerUiUpdate::EditorPreviewSelectionRequested(
                tsfp_mapmaker::modes::PreviewSelection::from_row(tsfp_mapmaker::modes::PreviewRow(
                    11
                ))
                .unwrap()
            )
        );
        assert_eq!(state.snapshot().editor_preview_selection.unwrap().row.0, 11);
        assert_eq!(
            built.activate_widget(&mut state, WidgetId(99)),
            MapMakerUiUpdate::Ignored
        );
    }

    #[test]
    fn preview_page_rejects_stale_or_mismatched_runtime_layout() {
        let (mut state, choices) = loaded_preview_state();
        state.dispatch(MapMakerUiCommand::Back);
        assert_eq!(
            build_editor_preview_page(&state.snapshot(), PageId(1), layout(choices)),
            Err(EditorPreviewBuildError::WrongPage)
        );

        let (state, choices) = loaded_preview_state();
        let mut wrong_order = layout(choices);
        wrong_order.choices.swap(0, 1);
        assert_eq!(
            build_editor_preview_page(&state.snapshot(), PageId(1), wrong_order),
            Err(EditorPreviewBuildError::ChoiceOrderMismatch)
        );
    }
}
