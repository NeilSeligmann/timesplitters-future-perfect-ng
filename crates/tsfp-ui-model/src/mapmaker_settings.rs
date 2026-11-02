//! Renderer projection for the Rust-owned Map Maker Map Settings catalogue.
//!
//! Runtime data supplies widget contents and layout. This module preserves
//! that presentation while routing focus and action activation through typed,
//! generation-qualified Map Maker state.

use crate::focus::{NavLayout, Wrap};
use crate::model::{Page, PageId, Widget, WidgetId};
use tsfp_mapmaker::ui::{
    EditorSurface, MapMakerPage, MapMakerUiCommand, MapMakerUiSnapshot, MapMakerUiState,
    MapMakerUiUpdate, MapSettingsAction, MapSettingsEntry, MapSettingsEntryTarget,
};
use tsfp_ui_renderer::AnchoredRect;

#[derive(Clone, Debug, PartialEq)]
pub struct MapSettingsEntryLayout {
    /// `None` preserves runtime presentation rows such as section headers.
    /// Only focusable rows receive a Rust-owned semantic target.
    pub entry: Option<MapSettingsEntry>,
    pub widget: Widget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapSettingsLayout {
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub nav: NavLayout,
    pub wrap: Wrap,
    pub entries: Vec<MapSettingsEntryLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapSettingsBuildError {
    WrongPage,
    MissingGeneration,
    EntryCountMismatch,
    EntryOrderMismatch,
    DuplicateWidget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WidgetBinding {
    widget: WidgetId,
    target: MapSettingsEntryTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapSettingsPage {
    pub page: Page,
    bindings: Vec<WidgetBinding>,
}

impl MapSettingsPage {
    pub fn target_for_widget(&self, widget: WidgetId) -> Option<MapSettingsEntryTarget> {
        self.bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.target)
    }

    pub fn focus_widget(&self, state: &mut MapMakerUiState, widget: WidgetId) -> MapMakerUiUpdate {
        self.target_for_widget(widget)
            .map_or(MapMakerUiUpdate::Ignored, |target| {
                state.dispatch(MapMakerUiCommand::FocusMapSettingsEntry(target))
            })
    }

    /// Activating a field focuses it; the choice adapter supplies its typed
    /// choice separately. Action rows route through the currently installed
    /// Rust editor-surface catalogue, or remain a typed request if no surface
    /// is registered.
    pub fn activate_widget(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
    ) -> MapMakerUiUpdate {
        let Some(target) = self.target_for_widget(widget) else {
            return MapMakerUiUpdate::Ignored;
        };
        match target.entry() {
            MapSettingsEntry::Field(_) => {
                state.dispatch(MapMakerUiCommand::FocusMapSettingsEntry(target))
            }
            MapSettingsEntry::Action(_) => {
                state.dispatch(MapMakerUiCommand::ActivateMapSettingsAction(target))
            }
        }
    }

    pub fn action_for_widget(&self, widget: WidgetId) -> Option<MapSettingsAction> {
        match self.target_for_widget(widget)?.entry() {
            MapSettingsEntry::Action(action) => Some(action),
            MapSettingsEntry::Field(_) => None,
        }
    }
}

pub fn build_map_settings_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    page_id: PageId,
    layout: MapSettingsLayout,
) -> Result<MapSettingsPage, MapSettingsBuildError> {
    if snapshot.page != MapMakerPage::Editor
        || snapshot.editor_surface != EditorSurface::MapSettings
    {
        return Err(MapSettingsBuildError::WrongPage);
    }
    let generation = snapshot
        .map_settings_generation
        .ok_or(MapSettingsBuildError::MissingGeneration)?;
    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = layout.nav;
    page.wrap = layout.wrap;
    let mut bindings = Vec::with_capacity(layout.entries.len());
    let mut entry_index = 0;
    for (index, row) in layout.entries.into_iter().enumerate() {
        if page.widgets.iter().any(|widget| widget.id == row.widget.id) {
            return Err(MapSettingsBuildError::DuplicateWidget);
        }
        page.widgets.push(row.widget.clone());
        if let Some(entry) = row.entry {
            if snapshot.map_settings_entries.get(entry_index) != Some(&entry) {
                return Err(MapSettingsBuildError::EntryOrderMismatch);
            }
            entry_index += 1;
            bindings.push(WidgetBinding {
                widget: row.widget.id,
                target: MapSettingsEntryTarget::new(generation, entry),
            });
        }
    }
    if entry_index != snapshot.map_settings_entries.len() {
        return Err(MapSettingsBuildError::EntryCountMismatch);
    }
    page.initial_focus = snapshot.focused_map_settings_entry.and_then(|focused| {
        bindings
            .iter()
            .find(|binding| binding.target.entry() == focused)
            .map(|binding| binding.widget)
    });
    Ok(MapSettingsPage { page, bindings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::focus::{NavLayout, Wrap};
    use crate::model::{TextRef, WidgetKind};
    use tsfp_mapmaker::ui::{
        EditorSurfaceGeneration, MapListGeneration, MapListRow, MapListRowId, MapLoadOutcome,
        MapMakerUiCommand as C, MapSettingChoiceId, MapSettingField, MapSettingRow,
        MapSettingsAction, MapSettingsEntry, MapSettingsGeneration,
    };

    fn settings_state() -> MapMakerUiState {
        let mut state = MapMakerUiState::default();
        state.dispatch(C::OpenStorage);
        state.dispatch(C::SelectLocalStorage);
        let list = MapListGeneration::new(1);
        let row = MapListRowId::new(list, 17);
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
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(2),
                vec![
                    EditorSurface::MapSettings,
                    EditorSurface::WeaponSets,
                    EditorSurface::BotSets,
                    EditorSurface::MapDescription,
                    EditorSurface::Briefing,
                ],
            )
            .unwrap();
        let target = state
            .editor_surface_target(EditorSurface::MapSettings)
            .unwrap();
        state.dispatch(C::OpenEditorSurface(target));
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(3),
                vec![
                    EditorSurface::WeaponSets,
                    EditorSurface::BotSets,
                    EditorSurface::MapDescription,
                    EditorSurface::Briefing,
                ],
            )
            .unwrap();
        state
            .install_map_settings_page(
                MapSettingsGeneration::new(3),
                vec![MapSettingRow {
                    field: MapSettingField::Theme,
                    choices: vec![MapSettingChoiceId::new(10), MapSettingChoiceId::new(11)],
                    selected: Some(MapSettingChoiceId::new(10)),
                }],
                vec![
                    MapSettingsEntry::Field(MapSettingField::Theme),
                    MapSettingsEntry::Action(MapSettingsAction::EditMapDescription),
                    MapSettingsEntry::Action(MapSettingsAction::Briefing),
                ],
            )
            .unwrap();
        state
    }

    fn layout(entries: &[MapSettingsEntry]) -> MapSettingsLayout {
        let widgets = entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let mut widget = Widget::new(WidgetId(40 + i as u64), WidgetKind::Button);
                widget.text = TextRef::Dynamic;
                MapSettingsEntryLayout {
                    entry: Some(*entry),
                    widget,
                }
            })
            .collect();
        MapSettingsLayout {
            widget_type: 1,
            panel_bounds: None,
            nav: NavLayout::Grid(1),
            wrap: Wrap::Clamp,
            entries: widgets,
        }
    }

    #[test]
    fn page_preserves_runtime_order_and_routes_typed_actions() {
        let mut state = settings_state();
        let entries = state.snapshot().map_settings_entries.to_vec();
        let built =
            build_map_settings_page(&state.snapshot(), PageId(0x52d474), layout(&entries)).unwrap();
        assert_eq!(built.page.nav, NavLayout::Grid(1));
        assert_eq!(built.page.wrap, Wrap::Clamp);
        assert_eq!(built.page.initial_focus, Some(WidgetId(40)));
        assert_eq!(built.page.widgets[1].text, TextRef::Dynamic);
        assert_eq!(
            built.focus_widget(&mut state, WidgetId(41)),
            MapMakerUiUpdate::MapSettingsEntryFocused(MapSettingsEntry::Action(
                MapSettingsAction::EditMapDescription
            ))
        );
        assert_eq!(
            built.activate_widget(&mut state, WidgetId(41)),
            MapMakerUiUpdate::MapSettingsActionOpened {
                action: MapSettingsAction::EditMapDescription,
                surface: EditorSurface::MapDescription,
            }
        );
        assert_eq!(state.editor_surface(), EditorSurface::MapDescription);
        assert_eq!(
            state
                .active_map_description_target()
                .map(|target| target.surface()),
            Some(EditorSurface::MapDescription)
        );
        assert_eq!(
            built.action_for_widget(WidgetId(41)),
            Some(MapSettingsAction::EditMapDescription)
        );

        let mut state = settings_state();
        let entries = state.snapshot().map_settings_entries.to_vec();
        let built =
            build_map_settings_page(&state.snapshot(), PageId(0x52d474), layout(&entries)).unwrap();
        assert_eq!(
            built.activate_widget(&mut state, WidgetId(42)),
            MapMakerUiUpdate::MapSettingsActionOpened {
                action: MapSettingsAction::Briefing,
                surface: EditorSurface::Briefing,
            }
        );
        assert_eq!(
            built.action_for_widget(WidgetId(42)),
            Some(MapSettingsAction::Briefing)
        );
        assert_eq!(built.action_for_widget(WidgetId(40)), None);
        assert_eq!(
            built.focus_widget(&mut state, WidgetId(99)),
            MapMakerUiUpdate::Ignored
        );
    }

    #[test]
    fn page_keeps_runtime_heading_rows_without_semantic_targets() {
        let mut state = settings_state();
        let entries = state.snapshot().map_settings_entries.to_vec();
        let mut rows = layout(&entries).entries;
        let mut heading = Widget::new(WidgetId(39), WidgetKind::Label);
        heading.text = TextRef::Id(crate::model::TextId(0x36b));
        rows.insert(
            0,
            MapSettingsEntryLayout {
                entry: None,
                widget: heading,
            },
        );
        let built = build_map_settings_page(
            &state.snapshot(),
            PageId(0x52d474),
            MapSettingsLayout {
                widget_type: 1,
                panel_bounds: None,
                nav: NavLayout::Grid(1),
                wrap: Wrap::Clamp,
                entries: rows,
            },
        )
        .unwrap();
        assert_eq!(built.page.widgets.len(), entries.len() + 1);
        assert_eq!(built.target_for_widget(WidgetId(39)), None);
        assert_eq!(
            built.target_for_widget(WidgetId(40)).unwrap().entry(),
            entries[0]
        );
    }

    #[test]
    fn page_rejects_wrong_route_order_and_stale_generation() {
        let mut state = settings_state();
        let entries = state.snapshot().map_settings_entries.to_vec();
        let mut reordered = layout(&entries);
        reordered.entries.swap(0, 1);
        assert_eq!(
            build_map_settings_page(&state.snapshot(), PageId(1), reordered),
            Err(MapSettingsBuildError::EntryOrderMismatch)
        );

        let built =
            build_map_settings_page(&state.snapshot(), PageId(2), layout(&entries)).unwrap();
        state
            .install_map_settings_page(
                MapSettingsGeneration::new(4),
                vec![MapSettingRow {
                    field: MapSettingField::Theme,
                    choices: vec![MapSettingChoiceId::new(10), MapSettingChoiceId::new(11)],
                    selected: Some(MapSettingChoiceId::new(10)),
                }],
                entries,
            )
            .unwrap();
        assert_eq!(
            built.activate_widget(&mut state, WidgetId(41)),
            MapMakerUiUpdate::Ignored
        );
    }
}
