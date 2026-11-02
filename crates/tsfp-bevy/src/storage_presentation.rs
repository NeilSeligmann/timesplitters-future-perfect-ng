//! Owned catalogue renderer/input boundary. Layout is a native design, not a
//! measured original preset. Visible frontend admission is a separate step.
use crate::storage_caller::{CatalogueAction, CatalogueView};
use std::{collections::BTreeMap, sync::Arc};
use tsfp_mapmaker::ui::{MapMakerPage, MapMakerUiSnapshot};
use tsfp_ui_model::{Page, PageId, TextRef, UiModel, Widget, WidgetId, WidgetKind, menu_rect};
use tsfp_ui_renderer::Vec2;
use tsfp_ui_wgpu::in_frame::LiveContent;

const PAGE: PageId = PageId(0x2319_0001);
const VISIBLE: usize = 8;

pub struct CataloguePresentation {
    pub content: Arc<LiveContent>,
    pub focused: Option<WidgetId>,
    pub snapshot: CatalogueView,
    pub name: Option<String>,
    /// Runtime save identity qualifies clicks across replaced name requests.
    pub name_identity: Option<String>,
    pub returning: bool,
}

impl CataloguePresentation {
    /// Keep focus visible without dropping catalogue rows or inventing a save.
    /// Damaged metadata stays visible as a non-interactive error row. Labels
    /// and paths are runtime data; status codes remain explicit diagnostics.
    pub fn new(snapshot: CatalogueView, font: u32) -> Self {
        let mut page = Page::new(PAGE, 1);
        let mut dynamic = BTreeMap::new();
        let start = snapshot.cursor.unwrap_or(0).saturating_sub(VISIBLE - 1);
        let total_rows = if snapshot.map_source_choice {
            2
        } else {
            snapshot.rows.len() + snapshot.sample_maps.len()
        };
        for index in start..total_rows.min(start + VISIBLE) {
            let id = WidgetId(index as u64);
            let mut widget = Widget::new(id, WidgetKind::Button);
            widget.text = TextRef::Dynamic;
            widget.font_index = font;
            widget.rect = menu_rect(
                Vec2::new(144.0, 96.0 + (index - start) as f32 * 32.0),
                Vec2::new(352.0, 28.0),
            );
            let label = if snapshot.map_source_choice {
                widget.enabled = snapshot.error.is_none();
                if index == 0 {
                    "Sample Maps".to_owned()
                } else {
                    "Local Storage".to_owned()
                }
            } else if let Some(row) = snapshot.rows.get(index) {
                widget.enabled = row.label.is_ok() && snapshot.error.is_none();
                match &row.label {
                    Ok(label) => label.clone(),
                    Err(error) => format!("{} [{error:#010x}]", row.key.path()),
                }
            } else if let Some(sample) = snapshot.sample_maps.get(index - snapshot.rows.len()) {
                widget.enabled = snapshot.error.is_none();
                sample.label.clone()
            } else {
                continue;
            };
            widget.navigable = widget.enabled;
            dynamic.insert(id, label.into_bytes());
            page.widgets.push(widget);
        }
        if matches!(
            snapshot.target,
            tsfp_engine_fileio::storage_frontend::Target::Profile { .. }
        ) {
            let id = WidgetId(snapshot.rows.len() as u64);
            let mut widget = Widget::new(id, WidgetKind::Button);
            widget.text = TextRef::Dynamic;
            widget.font_index = font;
            widget.rect = menu_rect(Vec2::new(144.0, 400.0), Vec2::new(352.0, 28.0));
            dynamic.insert(id, b"Create profile".to_vec());
            page.widgets.push(widget);
        }
        if let Some(error) = snapshot.error {
            let id = WidgetId(u64::MAX);
            let mut widget = Widget::new(id, WidgetKind::Label);
            widget.text = TextRef::Dynamic;
            widget.font_index = font;
            widget.navigable = false;
            widget.rect = menu_rect(Vec2::new(144.0, 368.0), Vec2::new(352.0, 28.0));
            dynamic.insert(id, format!("{error:#010x}").into_bytes());
            page.widgets.push(widget);
        }
        let focused = snapshot
            .cursor
            .map(|i| WidgetId(i as u64))
            .filter(|id| page.widget(*id).is_some_and(|w| w.is_focusable()));
        page.initial_focus = focused;
        let mut model = UiModel::new();
        model.preset = "native-local-storage".into();
        model.root = Some(PAGE);
        model.pages.insert(PAGE, page);
        Self {
            content: Arc::new(LiveContent {
                model,
                page: PAGE,
                dynamic,
                list: None,
                default_page: None,
            }),
            focused,
            snapshot,
            name: None,
            name_identity: None,
            returning: false,
        }
    }

    /// Replace the visible Map Maker row model with the Rust-owned page
    /// projection. Runtime text, display geometry, and widget IDs are retained
    /// from this presentation; the typed model supplies focus and row identity.
    pub fn project_owned_map_list(
        &mut self,
        state: &MapMakerUiSnapshot<'_>,
    ) -> Result<(), tsfp_ui_model::mapmaker_storage::MapListBuildError> {
        use tsfp_ui_model::mapmaker_storage::{
            MapListLayout, MapListRowLayout, build_map_list_page,
        };

        if state.page != MapMakerPage::MapList
            || self.snapshot.target != tsfp_engine_fileio::storage_frontend::Target::Map
            || self.snapshot.map_source_choice
            || self.name.is_some()
            || self.returning
        {
            return Err(tsfp_ui_model::mapmaker_storage::MapListBuildError::WrongPage);
        }
        let row_count = match state.focused_storage_source {
            Some(tsfp_mapmaker::ui::MapStorageSource::SampleMaps) => {
                self.snapshot.sample_maps.len()
            }
            Some(tsfp_mapmaker::ui::MapStorageSource::LocalStorage) => self.snapshot.rows.len(),
            None => return Err(tsfp_ui_model::mapmaker_storage::MapListBuildError::WrongPage),
        };
        if self.snapshot.error.is_some() {
            return Err(tsfp_ui_model::mapmaker_storage::MapListBuildError::UnsupportedStatusRow);
        }
        if state.map_rows.len() != row_count {
            return Err(tsfp_ui_model::mapmaker_storage::MapListBuildError::RowCountMismatch);
        }
        let live_page = self
            .content
            .model
            .pages
            .get(&self.content.page)
            .ok_or(tsfp_ui_model::mapmaker_storage::MapListBuildError::WrongPage)?;
        let mut rows = Vec::new();
        for widget in &live_page.widgets {
            let Ok(index) = usize::try_from(widget.id.0) else {
                continue;
            };
            if let Some(owned) = state.map_rows.get(index) {
                rows.push(MapListRowLayout {
                    row: owned.id,
                    widget: widget.clone(),
                });
            }
        }
        let first_row_index = rows
            .first()
            .and_then(|row| usize::try_from(row.widget.id.0).ok())
            .unwrap_or(0);
        let layout = MapListLayout {
            first_row_index,
            widget_type: live_page.widget_type,
            panel_bounds: live_page.panel_bounds,
            rows,
        };
        let projected = build_map_list_page(&state, self.content.page, layout)?;
        self.focused = projected.page.initial_focus;
        let mut content = (*self.content).clone();
        content
            .model
            .pages
            .insert(projected.page.id, projected.page);
        self.content = Arc::new(content);
        Ok(())
    }

    /// Project the live source-choice widgets through the typed Map Maker
    /// source page. Runtime text and geometry remain attached to each widget;
    /// the native model owns page focus and source identity.
    pub fn project_owned_storage_source(
        &mut self,
        state: &MapMakerUiSnapshot<'_>,
    ) -> Result<(), tsfp_ui_model::mapmaker_storage::StorageSourceBuildError> {
        use tsfp_mapmaker::ui::MapStorageSource;
        use tsfp_ui_model::mapmaker_storage::{
            StorageSourceLayout, StorageSourceRowLayout, build_storage_source_page,
        };

        if state.page != MapMakerPage::StorageSource
            || self.snapshot.target != tsfp_engine_fileio::storage_frontend::Target::Map
            || !self.snapshot.map_source_choice
            || self.name.is_some()
            || self.returning
        {
            return Err(tsfp_ui_model::mapmaker_storage::StorageSourceBuildError::WrongPage);
        }
        if self.snapshot.error.is_some() {
            return Err(tsfp_ui_model::mapmaker_storage::StorageSourceBuildError::WrongPage);
        }
        let live_page = self
            .content
            .model
            .pages
            .get(&self.content.page)
            .ok_or(tsfp_ui_model::mapmaker_storage::StorageSourceBuildError::WrongPage)?;
        let rows = live_page
            .widgets
            .iter()
            .filter_map(|widget| {
                let source = match widget.id.0 {
                    0 => MapStorageSource::SampleMaps,
                    1 => MapStorageSource::LocalStorage,
                    _ => return None,
                };
                Some(StorageSourceRowLayout {
                    source,
                    widget: widget.clone(),
                })
            })
            .collect();
        let layout = StorageSourceLayout {
            widget_type: live_page.widget_type,
            panel_bounds: live_page.panel_bounds,
            rows,
        };
        let projected = build_storage_source_page(state, self.content.page, layout)?;
        self.focused = projected.page.initial_focus;
        let mut content = (*self.content).clone();
        content
            .model
            .pages
            .insert(projected.page.id, projected.page);
        self.content = Arc::new(content);
        Ok(())
    }

    /// Return preparation has no save rows. Back is a direct cancellation
    /// action, not a retry/load disguised as a catalogue selection.
    pub fn profile_return(snapshot: CatalogueView, font: u32) -> Self {
        let mut presentation = Self::new(snapshot, font);
        let mut page = Page::new(PAGE, 1);
        let mut dynamic = BTreeMap::new();
        let message = if presentation.snapshot.error.is_some() {
            "Could not continue with the selected profiles. Go back or wait to retry."
        } else {
            "Preparing selected profiles..."
        };
        for (id, kind, label, y) in [
            (WidgetId(1), WidgetKind::Label, message, 128.0),
            (WidgetId(0), WidgetKind::Button, "Back", 224.0),
        ] {
            let mut widget = Widget::new(id, kind);
            widget.text = TextRef::Dynamic;
            widget.font_index = font;
            widget.navigable = id == WidgetId(0);
            widget.rect = menu_rect(Vec2::new(144.0, y), Vec2::new(352.0, 64.0));
            dynamic.insert(id, label.as_bytes().to_vec());
            page.widgets.push(widget);
        }
        page.initial_focus = Some(WidgetId(0));
        let mut model = UiModel::new();
        model.preset = "native-local-storage".into();
        model.root = Some(PAGE);
        model.pages.insert(PAGE, page);
        presentation.content = Arc::new(LiveContent {
            model,
            page: PAGE,
            dynamic,
            list: None,
            default_page: None,
        });
        presentation.focused = Some(WidgetId(0));
        presentation.returning = true;
        presentation
    }

    /// Native name entry replaces save rows; the sole action submits the owned text.
    pub fn profile_name(snapshot: CatalogueView, font: u32, text: String) -> Self {
        Self::save_name(
            snapshot,
            font,
            text,
            "Save profile",
            "Could not save profile. Edit the name or try again.",
        )
    }
    pub fn map_name(snapshot: CatalogueView, font: u32, text: String) -> Self {
        Self::save_name(
            snapshot,
            font,
            text,
            "Save map",
            "Could not save map. Edit the name or try again.",
        )
    }
    fn save_name(
        snapshot: CatalogueView,
        font: u32,
        text: String,
        button: &str,
        error: &str,
    ) -> Self {
        let mut presentation = Self::new(snapshot, font);
        let mut page = Page::new(PAGE, 1);
        let mut dynamic = BTreeMap::new();
        for (id, kind, label, y) in [
            (WidgetId(1), WidgetKind::Label, text.clone(), 128.0),
            (WidgetId(0), WidgetKind::Button, button.into(), 192.0),
        ] {
            let mut widget = Widget::new(id, kind);
            widget.text = TextRef::Dynamic;
            widget.font_index = font;
            widget.navigable = id == WidgetId(0);
            widget.rect = menu_rect(Vec2::new(144.0, y), Vec2::new(352.0, 48.0));
            dynamic.insert(id, label.into_bytes());
            page.widgets.push(widget);
        }
        if presentation.snapshot.error.is_some() {
            let id = WidgetId(2);
            let mut widget = Widget::new(id, WidgetKind::Label);
            widget.text = TextRef::Dynamic;
            widget.font_index = font;
            widget.navigable = false;
            widget.rect = menu_rect(Vec2::new(144.0, 272.0), Vec2::new(352.0, 48.0));
            dynamic.insert(id, error.as_bytes().to_vec());
            page.widgets.push(widget);
        }
        page.initial_focus = Some(WidgetId(0));
        let mut model = UiModel::new();
        model.preset = "native-local-storage".into();
        model.root = Some(PAGE);
        model.pages.insert(PAGE, page);
        presentation.content = Arc::new(LiveContent {
            model,
            page: PAGE,
            dynamic,
            list: None,
            default_page: None,
        });
        presentation.focused = Some(WidgetId(0));
        presentation.name = Some(text);
        presentation
    }

    /// Pointer coordinates use ModelLayout's design space (including widescreen
    /// safe-area offset). Back works on empty and failed catalogues too.
    pub fn pointer(
        &self,
        size: (f32, f32),
        position: Option<(f32, f32)>,
        moved: bool,
        accept: bool,
        back: bool,
    ) -> Vec<CatalogueAction> {
        if back {
            return vec![CatalogueAction::Back];
        }
        let Some(page) = self.content.model.page(PAGE) else {
            return vec![];
        };
        let Some(layout) = tsfp_ui_input::ModelLayout::from_page(page, size.0, size.1) else {
            return vec![];
        };
        let row = position
            .and_then(|(x, y)| tsfp_ui_input::hit_test(&layout, x, y))
            .filter(|id| {
                page.widget(WidgetId(id.0))
                    .is_some_and(|widget| widget.is_focusable())
            })
            .map(|id| id.0 as usize);
        let Some(row) = row else { return vec![] };
        let mut actions = Vec::new();
        if self.returning {
            return if accept {
                vec![CatalogueAction::Back]
            } else {
                vec![]
            };
        }
        if self.name.is_some() {
            return if accept {
                vec![CatalogueAction::Accept]
            } else {
                vec![]
            };
        }
        if moved || accept {
            actions.push(CatalogueAction::Focus(row));
        }
        if accept {
            actions.push(CatalogueAction::Accept);
        }
        actions
    }

    /// Reject input against a rendered catalogue that was replaced/reordered
    /// since drawing. Never let an old click select a different durable key.
    pub fn matches(&self, current: &CatalogueView) -> bool {
        self.snapshot.target == current.target
            && self.snapshot.map_source_choice == current.map_source_choice
            && self.snapshot.error == current.error
            && self.snapshot.rows == current.rows
            && self.snapshot.sample_maps == current.sample_maps
    }
}

/// Runtime-labelled destination rendering. This is a native layout contract;
/// frontend admission and executing qualified requests remain caller duties.
pub struct DestinationPresentation {
    pub content: Arc<LiveContent>,
    pub focused: Option<WidgetId>,
    pub snapshot: crate::storage_caller::DestinationView,
}
impl DestinationPresentation {
    pub fn new(snapshot: crate::storage_caller::DestinationView, font: u32) -> Self {
        let id = PageId(snapshot.page.id.0);
        let mut page = Page::new(id, 1);
        let mut dynamic = BTreeMap::new();
        let start = snapshot.cursor.unwrap_or(0).saturating_sub(VISIBLE - 1);
        for (index, row) in snapshot.rows.iter().enumerate().skip(start).take(VISIBLE) {
            let mut widget = Widget::new(WidgetId(row.id), WidgetKind::Button);
            widget.text = TextRef::Dynamic;
            widget.font_index = font;
            widget.enabled = row.enabled;
            widget.navigable = row.enabled;
            widget.rect = menu_rect(
                Vec2::new(144.0, 96.0 + (index - start) as f32 * 32.0),
                Vec2::new(352.0, 28.0),
            );
            dynamic.insert(widget.id, row.label.as_bytes().to_vec());
            page.widgets.push(widget);
        }
        let focused = snapshot
            .cursor
            .and_then(|i| snapshot.rows.get(i))
            .map(|row| WidgetId(row.id))
            .filter(|id| page.widget(*id).is_some_and(|w| w.is_focusable()));
        page.initial_focus = focused;
        let mut model = UiModel::new();
        model.preset = "native-local-storage".into();
        model.root = Some(id);
        model.pages.insert(id, page);
        Self {
            content: Arc::new(LiveContent {
                model,
                page: id,
                dynamic,
                list: None,
                default_page: None,
            }),
            focused,
            snapshot,
        }
    }

    /// Cursor movement does not invalidate geometry. Page/rows changing does;
    /// rejecting the entire stale click prevents a Focus failure followed by Accept.
    pub fn pointer(
        &self,
        current: &crate::storage_caller::DestinationView,
        size: (f32, f32),
        position: Option<(f32, f32)>,
        moved: bool,
        accept: bool,
        back: bool,
    ) -> Vec<crate::storage_caller::DestinationAction> {
        use crate::storage_caller::DestinationAction;
        if self.snapshot.page.id != current.page.id || self.snapshot.rows != current.rows {
            return vec![];
        }
        if back {
            return vec![DestinationAction::Back];
        }
        let Some(page) = self.content.model.page(self.content.page) else {
            return vec![];
        };
        let Some(layout) = tsfp_ui_input::ModelLayout::from_page(page, size.0, size.1) else {
            return vec![];
        };
        let hit = position
            .and_then(|(x, y)| tsfp_ui_input::hit_test(&layout, x, y))
            .filter(|id| {
                page.widget(WidgetId(id.0))
                    .is_some_and(|w| w.is_focusable())
            });
        let Some(hit) = hit else {
            return vec![];
        };
        let mut actions = Vec::new();
        if moved || accept {
            actions.push(DestinationAction::Focus(hit.0));
        }
        if accept {
            actions.push(DestinationAction::Accept);
        }
        actions
    }
}

#[cfg(test)]
mod pointer_input_tests {
    use super::*;
    use tsfp_engine_fileio::storage_flow::{SaveEntry, SaveKey, SaveKind};
    use tsfp_engine_fileio::storage_frontend::Target;

    #[test]
    fn mapmaker_owned_projection_keeps_runtime_label_geometry_and_focus() {
        use tsfp_mapmaker::ui::{
            MapListGeneration, MapListRow, MapListRowId, MapMakerUiCommand, MapMakerUiState,
        };

        let row_id = MapListRowId::new(MapListGeneration::new(72), 0x44);
        let view = CatalogueView {
            target: Target::Map,
            rows: vec![SaveEntry {
                key: SaveKey::new("local", "anicemap.mkr").unwrap(),
                kind: SaveKind::Map,
                label: Ok("Local map".into()),
                bytes: 32_756,
            }],
            sample_maps: Vec::new(),
            map_source_choice: false,
            cursor: Some(0),
            error: None,
        };
        let mut state = MapMakerUiState::default();
        state
            .install_local_map_rows(row_id.generation(), vec![MapListRow { id: row_id }])
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenStorage);
        state.dispatch(MapMakerUiCommand::SelectLocalStorage);
        state.dispatch(MapMakerUiCommand::FocusMap(row_id));

        let mut presentation = CataloguePresentation::new(view, 4);
        presentation
            .project_owned_map_list(&state.snapshot())
            .unwrap();

        let page = &presentation.content.model.pages[&presentation.content.page];
        assert_eq!(presentation.focused, Some(WidgetId(0)));
        assert_eq!(page.initial_focus, Some(WidgetId(0)));
        assert_eq!(page.widgets[0].text, TextRef::Dynamic);
        assert_eq!(page.widgets[0].font_index, 4);
        assert_eq!(presentation.content.dynamic[&WidgetId(0)], b"Local map");
    }

    #[test]
    fn mapmaker_sample_map_projection_uses_owned_row_focus_and_runtime_label() {
        use tsfp_mapmaker::ui::{
            MapListGeneration, MapListRow, MapMakerUiCommand, MapMakerUiState, MapStorageSource,
        };

        let generation = MapListGeneration::new(74);
        let row = tsfp_mapmaker::ui::MapListRowId::new(generation, 0);
        let mut state = MapMakerUiState::default();
        state.dispatch(MapMakerUiCommand::OpenStorage);
        state.dispatch(MapMakerUiCommand::SelectSampleMaps);
        state
            .install_map_rows(generation, vec![MapListRow { id: row }])
            .unwrap();
        state.dispatch(MapMakerUiCommand::FocusMap(row));
        assert_eq!(
            state.snapshot().focused_storage_source,
            Some(MapStorageSource::SampleMaps)
        );

        let snapshot = CatalogueView {
            target: Target::Map,
            rows: Vec::new(),
            sample_maps: vec![
                tsfp_engine_fileio::storage_flow::SampleMapEntry::new(17, "Sample map".into())
                    .unwrap(),
            ],
            map_source_choice: false,
            cursor: Some(0),
            error: None,
        };
        let mut presentation = CataloguePresentation::new(snapshot, 4);
        presentation
            .project_owned_map_list(&state.snapshot())
            .unwrap();

        let page = &presentation.content.model.pages[&presentation.content.page];
        assert_eq!(presentation.focused, Some(WidgetId(0)));
        assert_eq!(page.widgets[0].text, TextRef::Dynamic);
        assert_eq!(presentation.content.dynamic[&WidgetId(0)], b"Sample map");
    }

    #[test]
    fn mapmaker_owned_projection_keeps_status_rows_on_the_existing_page() {
        use tsfp_mapmaker::ui::{MapListGeneration, MapMakerUiCommand, MapMakerUiState};

        let mut state = MapMakerUiState::default();
        state
            .install_local_map_rows(MapListGeneration::new(73), Vec::new())
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenStorage);
        state.dispatch(MapMakerUiCommand::SelectLocalStorage);
        let view = CatalogueView {
            target: Target::Map,
            rows: Vec::new(),
            sample_maps: Vec::new(),
            map_source_choice: false,
            cursor: None,
            error: Some(0xc000000d),
        };
        let mut presentation = CataloguePresentation::new(view, 4);
        let original_page = presentation.content.model.pages[&presentation.content.page].clone();

        assert_eq!(
            presentation.project_owned_map_list(&state.snapshot()),
            Err(tsfp_ui_model::mapmaker_storage::MapListBuildError::UnsupportedStatusRow)
        );
        assert_eq!(
            presentation.content.model.pages[&presentation.content.page],
            original_page
        );
        assert!(presentation.content.dynamic[&WidgetId(u64::MAX)].starts_with(b"0xc000000d"));
    }

    #[test]
    fn mapmaker_local_map_rows_focus_on_hover_and_activate_on_click() {
        let snapshot = CatalogueView {
            target: Target::Map,
            rows: vec![SaveEntry {
                key: SaveKey::new("local", "anicemap.mkr").unwrap(),
                kind: SaveKind::Map,
                label: Ok("Local map".into()),
                bytes: 32_756,
            }],
            sample_maps: Vec::new(),
            map_source_choice: false,
            cursor: Some(0),
            error: None,
        };
        let presentation = CataloguePresentation::new(snapshot, 0);
        assert_eq!(
            presentation.pointer((640.0, 480.0), Some((320.0, 110.0)), true, false, false),
            vec![CatalogueAction::Focus(0)]
        );
        assert_eq!(
            presentation.pointer((640.0, 480.0), Some((320.0, 110.0)), false, true, false),
            vec![CatalogueAction::Focus(0), CatalogueAction::Accept]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_engine_fileio::{
        storage_flow::{SaveEntry, SaveKey, SaveKind},
        storage_frontend::Target,
        vfs::status,
    };
    #[test]
    fn map_name_presents_direct_save_and_inert_retained_error() {
        let snapshot = CatalogueView {
            target: tsfp_engine_fileio::storage_frontend::Target::Map,
            rows: Vec::new(),
            sample_maps: Vec::new(),
            map_source_choice: false,
            cursor: None,
            error: Some(0xc000000d),
        };
        let presentation = CataloguePresentation::map_name(snapshot, 0, "Native map".into());
        assert_eq!(presentation.content.dynamic[&WidgetId(0)], b"Save map");
        assert_eq!(presentation.content.dynamic[&WidgetId(1)], b"Native map");
        assert!(presentation.content.dynamic[&WidgetId(2)].starts_with(b"Could not save map"));
        assert!(
            presentation
                .pointer((640.0, 480.0), Some((200.0, 280.0)), true, true, false)
                .is_empty()
        );
        assert!(matches!(
            presentation
                .pointer((640.0, 480.0), Some((200.0, 200.0)), false, true, false)
                .as_slice(),
            [CatalogueAction::Accept]
        ));
        assert!(matches!(
            presentation
                .pointer((640.0, 480.0), None, false, true, true)
                .as_slice(),
            [CatalogueAction::Back]
        ));
    }
    #[test]
    fn destination_uses_stable_ids_and_rejects_stale_or_disabled_clicks() {
        use crate::storage_caller::{
            DestinationAction, DestinationRow, DestinationView, NativePageId, RetainedPage,
        };
        let mut view = DestinationView {
            page: RetainedPage {
                id: NativePageId(42),
                cursor: 0,
                scroll: 0.0,
            },
            rows: (0..12)
                .map(|i| DestinationRow {
                    id: 100 + i,
                    label: format!("Runtime {i}"),
                    enabled: i != 3,
                })
                .collect(),
            cursor: Some(10),
        };
        let p = DestinationPresentation::new(view.clone(), 4);
        assert_eq!(p.focused, Some(WidgetId(110)));
        assert_eq!(p.content.dynamic[&WidgetId(110)], b"Runtime 10");
        assert!(!p.content.dynamic.contains_key(&WidgetId(100)));
        assert!(
            p.pointer(
                &view,
                (640.0, 480.0),
                Some((200.0, 110.0)),
                true,
                true,
                false
            )
            .is_empty()
        );
        assert!(matches!(
            p.pointer(
                &view,
                (640.0, 480.0),
                Some((200.0, 142.0)),
                false,
                true,
                false
            )
            .as_slice(),
            [DestinationAction::Focus(104), DestinationAction::Accept]
        ));
        view.cursor = Some(4);
        assert!(matches!(
            p.pointer(&view, (0.0, 0.0), None, false, true, true)
                .as_slice(),
            [DestinationAction::Back]
        ));
        view.rows.swap(4, 5);
        assert!(
            p.pointer(
                &view,
                (640.0, 480.0),
                Some((200.0, 142.0)),
                true,
                true,
                false
            )
            .is_empty()
        );
        view.rows = p.snapshot.rows.clone();
        view.rows[4].enabled = false;
        assert!(
            p.pointer(
                &view,
                (640.0, 480.0),
                Some((200.0, 142.0)),
                true,
                true,
                false
            )
            .is_empty()
        );
        view.rows = p.snapshot.rows.clone();
        view.page.id = NativePageId(43);
        assert!(
            p.pointer(&view, (0.0, 0.0), None, false, false, true)
                .is_empty()
        );
    }
    fn view() -> CatalogueView {
        CatalogueView {
            target: Target::Profile { player: 2 },
            rows: (0..12)
                .map(|i| SaveEntry {
                    key: SaveKey::new(&format!("save{i}"), "Profile").unwrap(),
                    kind: SaveKind::Profile,
                    label: Ok(format!("Synthetic {i}")),
                    bytes: 8040,
                })
                .collect(),
            sample_maps: Vec::new(),
            map_source_choice: false,
            cursor: Some(10),
            error: None,
        }
    }
    #[test]
    fn map_catalogue_renders_local_and_runtime_sample_sources_together() {
        let local = SaveEntry {
            key: SaveKey::new("local", "anicemap.mkr").unwrap(),
            kind: SaveKind::Map,
            label: Ok("Local map".into()),
            bytes: 32756,
        };
        let snapshot = CatalogueView {
            target: Target::Map,
            rows: vec![local],
            sample_maps: vec![
                tsfp_engine_fileio::storage_flow::SampleMapEntry::new(77, "Runtime sample".into())
                    .unwrap(),
            ],
            map_source_choice: false,
            cursor: Some(1),
            error: None,
        };
        let presentation = CataloguePresentation::new(snapshot.clone(), 4);
        assert_eq!(presentation.focused, Some(WidgetId(1)));
        assert_eq!(presentation.content.dynamic[&WidgetId(0)], b"Local map");
        assert_eq!(
            presentation.content.dynamic[&WidgetId(1)],
            b"Runtime sample"
        );
        assert!(
            presentation
                .content
                .model
                .page(PAGE)
                .unwrap()
                .widget(WidgetId(1))
                .unwrap()
                .is_focusable()
        );
        let mut replaced = snapshot;
        replaced.sample_maps[0].label = "Different sample".into();
        assert!(!presentation.matches(&replaced));
    }

    #[test]
    fn editor_load_source_choice_renders_two_native_actions() {
        use tsfp_mapmaker::ui::{MapMakerUiCommand, MapMakerUiState, MapStorageSource};

        let snapshot = CatalogueView {
            target: Target::Map,
            rows: Vec::new(),
            sample_maps: Vec::new(),
            map_source_choice: true,
            cursor: Some(1),
            error: None,
        };
        let mut presentation = CataloguePresentation::new(snapshot.clone(), 4);
        assert_eq!(presentation.focused, Some(WidgetId(1)));
        assert_eq!(presentation.content.dynamic[&WidgetId(0)], b"Sample Maps");
        assert_eq!(presentation.content.dynamic[&WidgetId(1)], b"Local Storage");

        let mut state = MapMakerUiState::default();
        state.dispatch(MapMakerUiCommand::OpenStorage);
        state.dispatch(MapMakerUiCommand::FocusStorageSource(
            MapStorageSource::LocalStorage,
        ));
        presentation
            .project_owned_storage_source(&state.snapshot())
            .unwrap();
        let page = presentation.content.model.page(PAGE).unwrap();
        assert_eq!(page.widget_type, 1);
        assert_eq!(
            page.widgets[0].rect,
            menu_rect(Vec2::new(144.0, 96.0), Vec2::new(352.0, 28.0))
        );
        assert_eq!(presentation.focused, Some(WidgetId(1)));
        assert_eq!(presentation.content.dynamic[&WidgetId(1)], b"Local Storage");
        assert!(matches!(
            presentation
                .pointer((640.0, 480.0), Some((200.0, 110.0)), true, true, false)
                .as_slice(),
            [CatalogueAction::Focus(0), CatalogueAction::Accept]
        ));
        let mut replaced = snapshot;
        replaced.map_source_choice = false;
        assert!(!presentation.matches(&replaced));
    }
    #[test]
    fn failed_return_has_only_native_back_and_inert_error_text() {
        let mut snapshot = view();
        snapshot.rows.clear();
        snapshot.error = Some(status::INVALID_PARAMETER);
        let p = CataloguePresentation::profile_return(snapshot, 4);
        let page = p.content.model.page(PAGE).unwrap();
        assert!(p.returning);
        assert_eq!(page.widgets.len(), 2);
        assert_eq!(p.focused, Some(WidgetId(0)));
        assert_eq!(p.content.dynamic[&WidgetId(0)], b"Back");
        assert!(p.content.dynamic[&WidgetId(1)].starts_with(b"Could not continue"));
        assert!(
            p.pointer((640.0, 480.0), Some((200.0, 150.0)), true, true, false)
                .is_empty()
        );
        assert!(matches!(
            p.pointer((640.0, 480.0), Some((200.0, 240.0)), false, true, false)
                .as_slice(),
            [CatalogueAction::Back]
        ));
    }
    #[test]
    fn render_scroll_and_wide_pointer_share_durable_row_indices() {
        let p = CataloguePresentation::new(view(), 4);
        let page = p.content.model.page(PAGE).unwrap();
        assert_eq!(page.widgets.len(), 9);
        assert_eq!(page.widgets[0].id, WidgetId(3));
        assert_eq!(p.focused, Some(WidgetId(10)));
        assert_eq!(p.content.dynamic[&WidgetId(10)], b"Synthetic 10");
        // 16:9 design space adds 106.666 units to the safe-area left.
        let a = p.pointer((1280.0, 720.0), Some((300.0, 110.0)), false, true, false);
        assert!(matches!(
            &a[..],
            [CatalogueAction::Focus(3), CatalogueAction::Accept]
        ));
        let mut changed = view();
        changed.rows.swap(3, 4);
        assert!(!p.matches(&changed));
        changed = view();
        changed.target = Target::Map;
        assert!(!p.matches(&changed));
        assert!(p.matches(&view()));
    }
    #[test]
    fn name_view_never_selects_underlying_save_rows() {
        let p = CataloguePresentation::profile_name(view(), 4, "Native 🦀".into());
        assert_eq!(p.content.dynamic[&WidgetId(1)], "Native 🦀".as_bytes());
        assert_eq!(p.content.model.page(PAGE).unwrap().widgets.len(), 2);
        assert!(
            p.pointer((640.0, 480.0), Some((200.0, 145.0)), true, true, false)
                .is_empty()
        );
        assert!(matches!(
            p.pointer((640.0, 480.0), Some((200.0, 210.0)), true, true, false)
                .as_slice(),
            [CatalogueAction::Accept]
        ));
        assert!(matches!(
            p.pointer((640.0, 480.0), None, false, true, true)
                .as_slice(),
            [CatalogueAction::Back]
        ));
    }
    #[test]
    fn name_failure_is_visible_and_cannot_be_clicked_as_a_save() {
        let mut v = view();
        v.error = Some(status::ACCESS_DENIED);
        let p = CataloguePresentation::profile_name(v, 4, "Retained".into());
        assert_eq!(p.content.model.page(PAGE).unwrap().widgets.len(), 3);
        assert!(p.content.dynamic[&WidgetId(2)].starts_with(b"Could not save profile"));
        assert!(
            p.pointer((640.0, 480.0), Some((200.0, 290.0)), true, true, false)
                .is_empty()
        );
        assert!(matches!(
            p.pointer((640.0, 480.0), Some((200.0, 210.0)), false, true, false)
                .as_slice(),
            [CatalogueAction::Accept]
        ));
        assert!(
            !p.matches(&view()),
            "an old no-error snapshot cannot accept after failure"
        );
    }
    #[test]
    fn damaged_or_failed_rows_never_accept_and_back_always_wins() {
        let mut v = view();
        v.rows[3].label = Err(status::UNRECOGNIZED_VOLUME);
        let p = CataloguePresentation::new(v.clone(), 4);
        assert!(
            p.pointer((640.0, 480.0), Some((200.0, 110.0)), true, true, false)
                .is_empty()
        );
        v.error = Some(status::ACCESS_DENIED);
        let p = CataloguePresentation::new(v, 4);
        assert!(p.focused.is_none());
        assert!(
            p.pointer((640.0, 480.0), Some((200.0, 380.0)), true, true, false)
                .is_empty(),
            "status text is never a save action"
        );
        assert!(
            p.pointer((640.0, 480.0), Some((200.0, 142.0)), true, true, false)
                .is_empty()
        );
        assert!(matches!(
            &p.pointer((0.0, 0.0), None, false, true, true)[..],
            [CatalogueAction::Back]
        ));
        let mut empty = view();
        empty.rows.clear();
        empty.cursor = None;
        assert!(
            CataloguePresentation::new(empty, 4)
                .content
                .model
                .page(PAGE)
                .unwrap()
                .widgets
                .len()
                == 1
        );
    }
}
