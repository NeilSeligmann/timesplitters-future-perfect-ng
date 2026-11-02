//! Renderer projection for the Rust-owned Map Maker description editor.
//!
//! The runtime supplies the measured page layout and current text limit. This
//! projection binds one runtime widget to the Rust-owned edit session without
//! guessing the original page geometry or persistence handler.

use crate::focus::{NavLayout, Wrap};
use crate::model::{BindingId, Page, PageId, TextRef, Widget, WidgetId};
use tsfp_mapmaker::description::{
    DescriptionCommand, DescriptionError, DescriptionOutcome, DescriptionTarget,
    MapDescriptionSession,
};
use tsfp_mapmaker::ui::{EditorSurface, MapMakerPage, MapMakerUiSnapshot};
use tsfp_ui_renderer::AnchoredRect;

#[derive(Clone, Debug, PartialEq)]
pub struct MapDescriptionLayout {
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub nav: NavLayout,
    pub wrap: Wrap,
    pub widgets: Vec<Widget>,
    pub field_widget: WidgetId,
    pub initial_focus: Option<WidgetId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapDescriptionBuildError {
    WrongSurface,
    MissingField,
    DuplicateWidget,
    InvalidInitialFocus,
}

/// Built page plus the editable value and generation-qualified input seam.
#[derive(Clone, Debug, PartialEq)]
pub struct MapDescriptionPage {
    pub page: Page,
    pub field_widget: WidgetId,
    pub field_binding: BindingId,
    pub target: DescriptionTarget,
    pub value: String,
    pub cursor: usize,
    pub byte_limit: usize,
}

impl MapDescriptionPage {
    pub fn target_for_widget(&self, widget: WidgetId) -> Option<DescriptionTarget> {
        (widget == self.field_widget).then_some(self.target)
    }

    /// Native text input is applied directly to the Rust-owned session.
    /// Persistence remains the caller's typed responsibility after Commit.
    pub fn apply(
        &self,
        session: &mut MapDescriptionSession,
        command: DescriptionCommand,
    ) -> Result<DescriptionOutcome, DescriptionError> {
        session.apply(self.target, command)
    }
}

pub fn build_map_description_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    page_id: PageId,
    layout: MapDescriptionLayout,
    session: &MapDescriptionSession,
) -> Result<MapDescriptionPage, MapDescriptionBuildError> {
    if snapshot.page != MapMakerPage::Editor
        || snapshot.editor_surface != EditorSurface::MapDescription
    {
        return Err(MapDescriptionBuildError::WrongSurface);
    }
    if layout.initial_focus.is_some_and(|id| {
        !layout
            .widgets
            .iter()
            .any(|widget| widget.id == id && widget.enabled && widget.navigable)
    }) {
        return Err(MapDescriptionBuildError::InvalidInitialFocus);
    }

    let generation = session.generation().get();
    let field_binding = BindingId(format!("mapmaker.description.{generation}"));
    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = layout.nav;
    page.wrap = layout.wrap;
    page.initial_focus = layout.initial_focus;

    let mut found_field = false;
    for mut widget in layout.widgets {
        if page.widgets.iter().any(|prior| prior.id == widget.id) {
            return Err(MapDescriptionBuildError::DuplicateWidget);
        }
        if widget.id == layout.field_widget {
            found_field = true;
            widget.text = TextRef::Dynamic;
            widget.bound = Some(field_binding.clone());
        }
        page.widgets.push(widget);
    }
    if !found_field {
        return Err(MapDescriptionBuildError::MissingField);
    }

    Ok(MapDescriptionPage {
        page,
        field_widget: layout.field_widget,
        field_binding,
        target: session.target(),
        value: session.text(),
        cursor: session.cursor(),
        byte_limit: session.byte_limit(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::focus::{NavLayout, Wrap};
    use crate::model::{PageId, TextRef, WidgetKind};
    use tsfp_mapmaker::description::{DescriptionCommand, DescriptionGeneration};
    use tsfp_mapmaker::ui::{
        EditorSurface, EditorSurfaceGeneration, MapListGeneration, MapListRow, MapListRowId,
        MapLoadOutcome, MapMakerUiCommand, MapMakerUiState, MapMakerUiUpdate,
    };

    fn loaded_description_state() -> MapMakerUiState {
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
            panic!("selecting a map requests an authenticated load");
        };
        state.dispatch(MapMakerUiCommand::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(6),
                vec![EditorSurface::MapDescription],
            )
            .unwrap();
        let target = state
            .editor_surface_target(EditorSurface::MapDescription)
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenEditorSurface(target));
        state
    }

    #[test]
    fn description_projection_keeps_runtime_layout_and_edits_owned_text() {
        let state = loaded_description_state();
        let snapshot = state.snapshot();
        let field_id = WidgetId(44);
        let mut field = Widget::new(field_id, WidgetKind::TextField);
        field.text = TextRef::Dynamic;
        let layout = MapDescriptionLayout {
            widget_type: 9,
            panel_bounds: None,
            nav: NavLayout::Grid(1),
            wrap: Wrap::Clamp,
            widgets: vec![field],
            field_widget: field_id,
            initial_focus: Some(field_id),
        };
        let mut session =
            MapDescriptionSession::new(DescriptionGeneration::new(17), "map", 8).unwrap();
        let page = build_map_description_page(&snapshot, PageId(3), layout, &session).unwrap();

        assert_eq!(page.page.initial_focus, Some(field_id));
        assert_eq!(page.page.widgets[0].text, TextRef::Dynamic);
        assert_eq!(page.page.widgets[0].bound, Some(page.field_binding.clone()));
        assert_eq!(page.value, "map");
        assert_eq!(page.cursor, 3);
        assert_eq!(page.byte_limit, 8);
        assert_eq!(page.target_for_widget(field_id), Some(session.target()));
        assert_eq!(page.target_for_widget(WidgetId(45)), None);
        page.apply(&mut session, DescriptionCommand::Insert('!'))
            .unwrap();
        assert_eq!(session.text(), "map!");
    }

    #[test]
    fn description_projection_rejects_wrong_surface_and_missing_field() {
        let state = loaded_description_state();
        let snapshot = state.snapshot();
        let session = MapDescriptionSession::new(DescriptionGeneration::new(18), "", 8).unwrap();
        let empty_layout = MapDescriptionLayout {
            widget_type: 9,
            panel_bounds: None,
            nav: NavLayout::Grid(0),
            wrap: Wrap::Clamp,
            widgets: Vec::new(),
            field_widget: WidgetId(44),
            initial_focus: None,
        };
        assert_eq!(
            build_map_description_page(&snapshot, PageId(3), empty_layout.clone(), &session),
            Err(MapDescriptionBuildError::MissingField)
        );

        let other = MapMakerUiState::default();
        let other_snapshot = other.snapshot();
        assert_eq!(
            build_map_description_page(&other_snapshot, PageId(3), empty_layout, &session),
            Err(MapDescriptionBuildError::WrongSurface)
        );
    }
}
