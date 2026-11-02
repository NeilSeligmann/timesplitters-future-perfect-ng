//! Renderer projection for the Rust-owned Map Maker Palette Properties page.
//!
//! The live page supplies widget labels, geometry, and ordering. This module
//! keeps those values intact while binding each widget to a typed Rust action.

use crate::focus::{NavLayout, Wrap};
use crate::model::{Page, PageId, Widget, WidgetId};
use tsfp_mapmaker::ui::{
    EditorSurface, MapMakerPage, MapMakerUiCommand, MapMakerUiSnapshot, MapMakerUiState,
    MapMakerUiUpdate, PalettePropertyAction, PalettePropertyTarget,
};
use tsfp_ui_renderer::AnchoredRect;

#[derive(Clone, Debug, PartialEq)]
pub struct PalettePropertyActionLayout {
    pub action: PalettePropertyAction,
    pub widget: Widget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PalettePropertiesLayout {
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub nav: NavLayout,
    pub wrap: Wrap,
    pub actions: Vec<PalettePropertyActionLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PalettePropertiesBuildError {
    WrongPage,
    MissingGeneration,
    ActionCountMismatch,
    ActionOrderMismatch,
    DuplicateWidget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WidgetBinding {
    widget: WidgetId,
    target: PalettePropertyTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PalettePropertiesPage {
    pub page: Page,
    bindings: Vec<WidgetBinding>,
}

impl PalettePropertiesPage {
    pub fn target_for_widget(&self, widget: WidgetId) -> Option<PalettePropertyTarget> {
        self.bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.target)
    }

    pub fn focus_widget(&self, state: &mut MapMakerUiState, widget: WidgetId) -> MapMakerUiUpdate {
        self.target_for_widget(widget)
            .map_or(MapMakerUiUpdate::Ignored, |target| {
                state.dispatch(MapMakerUiCommand::FocusPaletteProperty(target))
            })
    }

    pub fn activate_widget(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
    ) -> MapMakerUiUpdate {
        self.target_for_widget(widget)
            .map_or(MapMakerUiUpdate::Ignored, |target| {
                state.dispatch(MapMakerUiCommand::ActivatePaletteProperty(target))
            })
    }
}

pub fn build_palette_properties_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    page_id: PageId,
    layout: PalettePropertiesLayout,
) -> Result<PalettePropertiesPage, PalettePropertiesBuildError> {
    if snapshot.page != MapMakerPage::Editor
        || snapshot.editor_surface != EditorSurface::PaletteProperties
    {
        return Err(PalettePropertiesBuildError::WrongPage);
    }
    let generation = snapshot
        .palette_properties_generation
        .ok_or(PalettePropertiesBuildError::MissingGeneration)?;
    if layout.actions.len() != snapshot.palette_property_actions.len() {
        return Err(PalettePropertiesBuildError::ActionCountMismatch);
    }

    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = layout.nav;
    page.wrap = layout.wrap;
    let mut bindings = Vec::with_capacity(layout.actions.len());
    for (index, row) in layout.actions.into_iter().enumerate() {
        if snapshot.palette_property_actions.get(index) != Some(&row.action) {
            return Err(PalettePropertiesBuildError::ActionOrderMismatch);
        }
        if page.widgets.iter().any(|widget| widget.id == row.widget.id) {
            return Err(PalettePropertiesBuildError::DuplicateWidget);
        }
        page.widgets.push(row.widget.clone());
        bindings.push(WidgetBinding {
            widget: row.widget.id,
            target: PalettePropertyTarget::new(generation, row.action),
        });
    }
    page.initial_focus = snapshot.focused_palette_property.and_then(|focused| {
        bindings
            .iter()
            .find(|binding| binding.target.action() == focused)
            .map(|binding| binding.widget)
    });
    Ok(PalettePropertiesPage { page, bindings })
}

#[cfg(test)]
mod tests {
    use super::{
        PalettePropertiesBuildError, PalettePropertiesLayout, PalettePropertyActionLayout,
        build_palette_properties_page,
    };
    use crate::focus::{NavLayout, Wrap};
    use crate::model::{PageId, TextRef, Widget, WidgetId, WidgetKind};
    use tsfp_mapmaker::ui::{
        EditorMenuGeneration, EditorSurface, EditorSurfaceGeneration, MapListGeneration,
        MapListRow, MapListRowId, MapLoadOutcome, MapMakerUiCommand, MapMakerUiState,
        MapMakerUiUpdate, PalettePropertiesGeneration, PalettePropertyAction,
    };

    fn palette_state() -> MapMakerUiState {
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
            .install_original_editor_menu(EditorMenuGeneration::new(4))
            .unwrap();
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(4),
                vec![EditorSurface::PaletteProperties],
            )
            .unwrap();
        let target = state
            .editor_surface_target(EditorSurface::PaletteProperties)
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenEditorSurface(target));
        state
            .install_palette_properties_catalog(
                PalettePropertiesGeneration::new(5),
                vec![
                    PalettePropertyAction::LightRow,
                    PalettePropertyAction::ChangePalette,
                ],
            )
            .unwrap();
        state
    }

    fn layout() -> PalettePropertiesLayout {
        let actions = [
            PalettePropertyAction::LightRow,
            PalettePropertyAction::ChangePalette,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, action)| {
            let mut widget = Widget::new(WidgetId(70 + index as u64), WidgetKind::Button);
            widget.text = TextRef::Id(tsfp_ui_model_text(action));
            PalettePropertyActionLayout { action, widget }
        })
        .collect();
        PalettePropertiesLayout {
            widget_type: 1,
            panel_bounds: None,
            nav: NavLayout::Grid(1),
            wrap: Wrap::Clamp,
            actions,
        }
    }

    fn tsfp_ui_model_text(action: PalettePropertyAction) -> crate::model::TextId {
        crate::model::TextId(action.text_id().get() as u32)
    }

    #[test]
    fn projection_keeps_runtime_widgets_and_routes_typed_actions() {
        let mut state = palette_state();
        let snapshot = state.snapshot();
        let built = build_palette_properties_page(&snapshot, PageId(0x52ea48), layout()).unwrap();
        assert_eq!(built.page.widgets[0].id, WidgetId(70));
        assert_eq!(
            built.page.widgets[0].text,
            TextRef::Id(crate::model::TextId(0x0bc4))
        );
        assert_eq!(built.page.initial_focus, Some(WidgetId(70)));
        assert_eq!(
            built.focus_widget(&mut state, WidgetId(71)),
            MapMakerUiUpdate::PalettePropertyFocused(PalettePropertyAction::ChangePalette)
        );
        assert_eq!(
            built.activate_widget(&mut state, WidgetId(71)),
            MapMakerUiUpdate::PalettePageReentered
        );
    }

    #[test]
    fn projection_rejects_unmatched_runtime_action_order() {
        let state = palette_state();
        let mut layout = layout();
        layout.actions.swap(0, 1);
        assert_eq!(
            build_palette_properties_page(&state.snapshot(), PageId(1), layout),
            Err(PalettePropertiesBuildError::ActionOrderMismatch)
        );
    }
}
