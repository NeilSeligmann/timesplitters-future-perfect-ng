//! Renderer projection for the runtime-backed Weapon Set and Bot Set pages.
//!
//! T1714 measures the page builders and entry limits. Runtime data supplies
//! labels, row widgets, and profile contents; Rust owns semantic focus and
//! activation routing.

use crate::focus::{NavLayout, Wrap};
use crate::model::{Page, PageId, Widget, WidgetId};
use tsfp_mapmaker::ui::{
    EditorSetEntry, EditorSetEntryId, EditorSetEntryTarget, EditorSetListKind, EditorSurface,
    MapMakerPage, MapMakerUiCommand, MapMakerUiSnapshot, MapMakerUiState, MapMakerUiUpdate,
};
use tsfp_ui_renderer::AnchoredRect;

#[derive(Clone, Debug, PartialEq)]
pub struct EditorSetEntryLayout {
    pub entry: EditorSetEntry,
    pub widget: Widget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorSetListLayout {
    pub kind: EditorSetListKind,
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub nav: NavLayout,
    pub wrap: Wrap,
    pub entries: Vec<EditorSetEntryLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorSetListBuildError {
    WrongPage,
    WrongList,
    MissingGeneration,
    EntryCountMismatch,
    EntryOrderMismatch,
    DuplicateWidget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WidgetBinding {
    widget: WidgetId,
    target: EditorSetEntryTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorSetListPage {
    pub page: Page,
    bindings: Vec<WidgetBinding>,
}

impl EditorSetListPage {
    pub fn target_for_widget(&self, widget: WidgetId) -> Option<EditorSetEntryTarget> {
        self.bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.target)
    }

    pub fn focus_widget(&self, state: &mut MapMakerUiState, widget: WidgetId) -> MapMakerUiUpdate {
        self.target_for_widget(widget)
            .map_or(MapMakerUiUpdate::Ignored, |target| {
                state.dispatch(MapMakerUiCommand::FocusEditorSetEntry(target))
            })
    }

    /// A selected profile row is a typed request to its native editor handler.
    /// This projection never writes profile data or guest memory.
    pub fn activate_widget(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
    ) -> MapMakerUiUpdate {
        self.target_for_widget(widget)
            .map_or(MapMakerUiUpdate::Ignored, |target| {
                state.dispatch(MapMakerUiCommand::ActivateEditorSetEntry(target))
            })
    }

    pub fn entry_for_widget(&self, widget: WidgetId) -> Option<EditorSetEntryId> {
        self.target_for_widget(widget).map(|target| target.entry())
    }
}

pub fn build_editor_set_list_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    page_id: PageId,
    layout: EditorSetListLayout,
) -> Result<EditorSetListPage, EditorSetListBuildError> {
    if snapshot.page != MapMakerPage::Editor {
        return Err(EditorSetListBuildError::WrongPage);
    }
    if snapshot.editor_surface != layout.kind.surface()
        || snapshot.editor_set_list_kind != Some(layout.kind)
    {
        return Err(EditorSetListBuildError::WrongList);
    }
    let generation = snapshot
        .editor_set_list_generation
        .ok_or(EditorSetListBuildError::MissingGeneration)?;
    if layout.entries.len() != snapshot.editor_set_entries.len() {
        return Err(EditorSetListBuildError::EntryCountMismatch);
    }

    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = layout.nav;
    page.wrap = layout.wrap;
    let mut bindings = Vec::with_capacity(layout.entries.len());
    for (index, row) in layout.entries.into_iter().enumerate() {
        if snapshot.editor_set_entries.get(index) != Some(&row.entry) {
            return Err(EditorSetListBuildError::EntryOrderMismatch);
        }
        if page.widgets.iter().any(|widget| widget.id == row.widget.id) {
            return Err(EditorSetListBuildError::DuplicateWidget);
        }
        page.widgets.push(row.widget.clone());
        bindings.push(WidgetBinding {
            widget: row.widget.id,
            target: EditorSetEntryTarget::new(generation, layout.kind, row.entry.id),
        });
    }
    page.initial_focus = snapshot.focused_editor_set_entry.and_then(|focused| {
        bindings
            .iter()
            .find(|binding| binding.target.entry() == focused)
            .map(|binding| binding.widget)
    });
    Ok(EditorSetListPage { page, bindings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::focus::{NavLayout, Wrap};
    use crate::model::{TextRef, WidgetKind};
    use tsfp_mapmaker::ui::{
        EditorSetListGeneration, EditorSurfaceGeneration, MapListGeneration, MapListRow,
        MapListRowId, MapLoadOutcome, MapMakerUiCommand as C,
    };

    fn set_list_state(kind: EditorSetListKind) -> MapMakerUiState {
        let mut state = MapMakerUiState::default();
        state.dispatch(C::OpenStorage);
        state.dispatch(C::SelectLocalStorage);
        let list = MapListGeneration::new(1);
        let row = MapListRowId::new(list, 4);
        state
            .install_local_map_rows(list, vec![MapListRow { id: row }])
            .unwrap();
        let request = match state.dispatch(C::SelectMap(row)) {
            MapMakerUiUpdate::LoadRequested(request) => request,
            update => panic!("unexpected map selection: {update:?}"),
        };
        state.dispatch(C::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        state
            .install_editor_surface_catalog(EditorSurfaceGeneration::new(2), vec![kind.surface()])
            .unwrap();
        let target = state.editor_surface_target(kind.surface()).unwrap();
        state.dispatch(C::OpenEditorSurface(target));
        state
            .install_editor_set_list(
                EditorSetListGeneration::new(3),
                kind,
                vec![
                    EditorSetEntry {
                        id: EditorSetEntryId::new(10),
                    },
                    EditorSetEntry {
                        id: EditorSetEntryId::new(11),
                    },
                ],
            )
            .unwrap();
        state
    }

    fn layout(kind: EditorSetListKind, entries: &[EditorSetEntry]) -> EditorSetListLayout {
        let rows = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let mut widget = Widget::new(WidgetId(50 + index as u64), WidgetKind::Button);
                widget.text = TextRef::Dynamic;
                EditorSetEntryLayout {
                    entry: *entry,
                    widget,
                }
            })
            .collect();
        EditorSetListLayout {
            kind,
            widget_type: 1,
            panel_bounds: None,
            nav: NavLayout::Grid(1),
            wrap: Wrap::Clamp,
            entries: rows,
        }
    }

    #[test]
    fn projection_preserves_runtime_widgets_and_routes_typed_rows() {
        let mut state = set_list_state(EditorSetListKind::Weapon);
        state
            .install_editor_set_list(
                EditorSetListGeneration::new(3),
                EditorSetListKind::Weapon,
                vec![
                    EditorSetEntry {
                        id: EditorSetEntryId::new(10),
                    },
                    EditorSetEntry {
                        id: EditorSetEntryId::new(11),
                    },
                ],
            )
            .unwrap();
        let entries = state.editor_set_entries().to_vec();
        let page = build_editor_set_list_page(
            &state.snapshot(),
            PageId(0x52fcec),
            layout(EditorSetListKind::Weapon, &entries),
        )
        .unwrap();
        assert_eq!(page.page.initial_focus, Some(WidgetId(50)));
        assert_eq!(page.page.widgets[1].text, TextRef::Dynamic);
        assert_eq!(
            page.entry_for_widget(WidgetId(51)),
            Some(EditorSetEntryId::new(11))
        );
        assert_eq!(
            page.focus_widget(&mut state, WidgetId(51)),
            MapMakerUiUpdate::EditorSetEntryFocused(EditorSetEntryId::new(11))
        );
        assert_eq!(
            page.activate_widget(&mut state, WidgetId(51)),
            MapMakerUiUpdate::EditorSetEntryActivated {
                list: EditorSetListKind::Weapon,
                entry: EditorSetEntryId::new(11),
            }
        );
    }

    #[test]
    fn projection_rejects_wrong_list_order_and_duplicate_widgets() {
        let state = set_list_state(EditorSetListKind::Bot);
        let entries = state.editor_set_entries().to_vec();
        assert_eq!(
            build_editor_set_list_page(
                &state.snapshot(),
                PageId(1),
                layout(EditorSetListKind::Weapon, &entries),
            ),
            Err(EditorSetListBuildError::WrongList)
        );
        let mut reordered = layout(EditorSetListKind::Bot, &entries);
        reordered.entries.swap(0, 1);
        assert_eq!(
            build_editor_set_list_page(&state.snapshot(), PageId(2), reordered),
            Err(EditorSetListBuildError::EntryOrderMismatch)
        );
        let mut duplicate_widgets = layout(EditorSetListKind::Bot, &entries);
        duplicate_widgets.entries[1].widget.id = duplicate_widgets.entries[0].widget.id;
        assert_eq!(
            build_editor_set_list_page(&state.snapshot(), PageId(3), duplicate_widgets),
            Err(EditorSetListBuildError::DuplicateWidget)
        );
    }
}
