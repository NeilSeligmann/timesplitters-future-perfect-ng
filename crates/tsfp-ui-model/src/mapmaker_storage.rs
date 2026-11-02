//! Host-free presentation of the Rust-owned local Map Maker catalogue.
//!
//! The runtime supplies labels, page identity, and measured geometry. Stable
//! row identities remain distinct from page-local renderer widget IDs.

use crate::focus::{NavLayout, Wrap};
use crate::model::{Page, PageId, Widget, WidgetId};
use tsfp_mapmaker::ui::{
    MapListRowId, MapMakerPage, MapMakerUiCommand, MapMakerUiSnapshot, MapMakerUiState,
    MapMakerUiUpdate, MapStorageSource, StorageProgress, StorageProgressOutcome,
};
use tsfp_ui_renderer::AnchoredRect;

/// Measured runtime page and its two non-interactive storage status labels.
/// The page and widget data stay caller supplied so this projection does not
/// invent original-preset layout or embed game strings.
#[derive(Clone, Debug, PartialEq)]
pub struct StorageProgressLayout {
    pub page: Page,
    pub message_widget: WidgetId,
    pub warning_widget: WidgetId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageProgressBuildError {
    WrongPage,
    MissingProgress,
    DuplicateStatusWidget,
    MissingStatusWidget,
    InteractiveStatusWidget,
}

/// Renderer-facing status page. It carries the Rust-owned operation identity
/// while preserving the caller's measured page, widgets, labels, and geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct StorageProgressPage {
    pub page: Page,
    pub progress: StorageProgress,
    pub message_widget: WidgetId,
    pub warning_widget: WidgetId,
}

/// Project Rust-owned operation state onto measured runtime page data.
/// Status labels must be distinct, non-interactive labels; Back remains a
/// typed page action handled by the Rust input reducer.
pub fn build_storage_progress_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    layout: StorageProgressLayout,
) -> Result<StorageProgressPage, StorageProgressBuildError> {
    if snapshot.page != MapMakerPage::StorageProgress {
        return Err(StorageProgressBuildError::WrongPage);
    }
    let progress = snapshot
        .storage_progress
        .ok_or(StorageProgressBuildError::MissingProgress)?;
    if layout.message_widget == layout.warning_widget {
        return Err(StorageProgressBuildError::DuplicateStatusWidget);
    }

    for id in [layout.message_widget, layout.warning_widget] {
        let matches = layout.page.widgets.iter().filter(|widget| widget.id == id);
        let mut matches = matches;
        let Some(widget) = matches.next() else {
            return Err(StorageProgressBuildError::MissingStatusWidget);
        };
        if matches.next().is_some() {
            return Err(StorageProgressBuildError::DuplicateStatusWidget);
        }
        if widget.kind != crate::model::WidgetKind::Label || widget.is_focusable() {
            return Err(StorageProgressBuildError::InteractiveStatusWidget);
        }
    }

    Ok(StorageProgressPage {
        page: layout.page,
        progress,
        message_widget: layout.message_widget,
        warning_widget: layout.warning_widget,
    })
}

/// Runtime widget identity, source identity, text source, and geometry for one
/// storage-source row.
#[derive(Clone, Debug, PartialEq)]
pub struct StorageSourceRowLayout {
    pub source: MapStorageSource,
    pub widget: Widget,
}

/// Caller-supplied page family and ordered source rows. The original row order
/// and display details stay with the runtime page/data layer.
#[derive(Clone, Debug, PartialEq)]
pub struct StorageSourceLayout {
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub rows: Vec<StorageSourceRowLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageSourceBuildError {
    WrongPage,
    RowCountMismatch,
    DuplicateSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageSourceWidgetBinding {
    pub widget: WidgetId,
    pub source: MapStorageSource,
}

/// Renderer-facing Storage Source page with a typed route back to owned state.
#[derive(Clone, Debug, PartialEq)]
pub struct StorageSourcePage {
    pub page: Page,
    bindings: Vec<StorageSourceWidgetBinding>,
}

impl StorageSourcePage {
    pub fn bindings(&self) -> &[StorageSourceWidgetBinding] {
        &self.bindings
    }

    pub fn source_for_widget(&self, widget: WidgetId) -> Option<MapStorageSource> {
        self.bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.source)
    }

    pub fn focus_widget(&self, state: &mut MapMakerUiState, widget: WidgetId) -> MapMakerUiUpdate {
        let Some(source) = self.source_for_widget(widget) else {
            return MapMakerUiUpdate::Ignored;
        };
        state.dispatch(MapMakerUiCommand::FocusStorageSource(source))
    }

    pub fn activate_widget(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
    ) -> MapMakerUiUpdate {
        let Some(source) = self.source_for_widget(widget) else {
            return MapMakerUiUpdate::Ignored;
        };
        let command = match source {
            MapStorageSource::SampleMaps => MapMakerUiCommand::SelectSampleMaps,
            MapStorageSource::LocalStorage => MapMakerUiCommand::SelectLocalStorage,
        };
        state.dispatch(command)
    }
}

/// Build the renderer-facing source picker from typed state and measured
/// runtime rows. Widget IDs and source identities remain separate namespaces.
pub fn build_storage_source_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    page_id: PageId,
    layout: StorageSourceLayout,
) -> Result<StorageSourcePage, StorageSourceBuildError> {
    if snapshot.page != MapMakerPage::StorageSource {
        return Err(StorageSourceBuildError::WrongPage);
    }
    if layout.rows.len() != 2 {
        return Err(StorageSourceBuildError::RowCountMismatch);
    }
    if layout.rows[0].source == layout.rows[1].source {
        return Err(StorageSourceBuildError::DuplicateSource);
    }

    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = NavLayout::Grid(1);
    page.wrap = Wrap::Clamp;
    let mut bindings = Vec::with_capacity(layout.rows.len());
    for row in layout.rows {
        let widget_id = row.widget.id;
        page.widgets.push(row.widget);
        bindings.push(StorageSourceWidgetBinding {
            widget: widget_id,
            source: row.source,
        });
    }
    page.initial_focus = snapshot.focused_storage_source.and_then(|focused| {
        bindings
            .iter()
            .find(|binding| binding.source == focused)
            .map(|binding| binding.widget)
    });
    Ok(StorageSourcePage { page, bindings })
}

/// Runtime widget data for one installed map row.
#[derive(Clone, Debug, PartialEq)]
pub struct MapListRowLayout {
    pub row: MapListRowId,
    /// Preserve the live page's widget identity, text source, and display geometry.
    pub widget: Widget,
}

/// Page family and display geometry supplied by the runtime presentation.
#[derive(Clone, Debug, PartialEq)]
pub struct MapListLayout {
    /// Absolute first catalogue index represented by `rows` (the renderer may
    /// show a scrolling window into a larger catalogue).
    pub first_row_index: usize,
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    /// Rows must match the owned catalogue order and generation exactly.
    pub rows: Vec<MapListRowLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapListBuildError {
    WrongPage,
    UnsupportedStatusRow,
    RowCountMismatch,
    RowIdentityMismatch,
}

/// Mapping between renderer widget IDs and owned catalogue rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapListWidgetBinding {
    pub widget: WidgetId,
    pub row: MapListRowId,
}

/// Renderable local map page and typed route from activated rows to UI state.
#[derive(Clone, Debug, PartialEq)]
pub struct MapListPage {
    pub page: Page,
    bindings: Vec<MapListWidgetBinding>,
}

impl MapListPage {
    pub fn bindings(&self) -> &[MapListWidgetBinding] {
        &self.bindings
    }

    pub fn row_for_widget(&self, widget: WidgetId) -> Option<MapListRowId> {
        self.bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.row)
    }

    /// Activate a row in the owned state. The reducer revalidates the active
    /// page, current catalogue generation, and absence of another load request.
    pub fn activate_widget(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
    ) -> MapMakerUiUpdate {
        let Some(row) = self.row_for_widget(widget) else {
            return MapMakerUiUpdate::Ignored;
        };
        state.dispatch(MapMakerUiCommand::SelectMap(row))
    }
}

/// Build the renderer-facing local map page from owned state and runtime data.
/// The builder never derives a row identity from its widget position.
pub fn build_map_list_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    page_id: PageId,
    layout: MapListLayout,
) -> Result<MapListPage, MapListBuildError> {
    if snapshot.page != MapMakerPage::MapList {
        return Err(MapListBuildError::WrongPage);
    }
    let Some(last_row_index) = layout.first_row_index.checked_add(layout.rows.len()) else {
        return Err(MapListBuildError::RowCountMismatch);
    };
    if last_row_index > snapshot.map_rows.len() {
        return Err(MapListBuildError::RowCountMismatch);
    }
    if layout
        .rows
        .iter()
        .zip(&snapshot.map_rows[layout.first_row_index..last_row_index])
        .any(|(row, owned)| row.row != owned.id)
    {
        return Err(MapListBuildError::RowIdentityMismatch);
    }

    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = NavLayout::Grid(1);
    page.wrap = Wrap::Clamp;
    let mut bindings = Vec::with_capacity(layout.rows.len());

    for row in layout.rows {
        let widget_id = row.widget.id;
        page.widgets.push(row.widget);
        bindings.push(MapListWidgetBinding {
            widget: widget_id,
            row: row.row,
        });
    }

    page.initial_focus = snapshot.focused_map.and_then(|focused| {
        bindings
            .iter()
            .find(|binding| binding.row == focused)
            .map(|binding| binding.widget)
    });

    Ok(MapListPage { page, bindings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{TextId, TextRef, WidgetKind, menu_rect};
    use tsfp_mapmaker::ui::{
        MapListGeneration, MapListRow, MapMakerUiCommand, MapMakerUiState, MapStorageSource,
        StorageOperation, StorageProgressOutcome,
    };
    use tsfp_ui_renderer::Vec2;

    fn active_state() -> (MapMakerUiState, MapListRowId, MapListRowId) {
        let generation = MapListGeneration::new(14);
        let first = MapListRowId::new(generation, 0x9001);
        let second = MapListRowId::new(generation, 0x9002);
        let mut state = MapMakerUiState::default();
        state
            .install_local_map_rows(
                generation,
                vec![MapListRow { id: first }, MapListRow { id: second }],
            )
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenStorage);
        state.dispatch(MapMakerUiCommand::SelectLocalStorage);
        state.dispatch(MapMakerUiCommand::FocusMap(second));
        (state, first, second)
    }

    fn row(row: MapListRowId, widget_id: u64, label: u32, y: f32) -> MapListRowLayout {
        let mut widget = Widget::new(WidgetId(widget_id), WidgetKind::Button);
        widget.text = TextRef::Id(TextId(label));
        widget.rect = menu_rect(Vec2::new(18.0, y), Vec2::new(240.0, 22.0));
        MapListRowLayout { row, widget }
    }

    fn layout(first: MapListRowId, second: MapListRowId) -> MapListLayout {
        MapListLayout {
            first_row_index: 0,
            widget_type: 1,
            panel_bounds: None,
            rows: vec![row(first, 1, 101, 70.0), row(second, 2, 102, 96.0)],
        }
    }

    fn source_row(
        source: MapStorageSource,
        widget_id: u64,
        label: u32,
        y: f32,
    ) -> StorageSourceRowLayout {
        let mut widget = Widget::new(WidgetId(widget_id), WidgetKind::Button);
        widget.text = TextRef::Id(TextId(label));
        widget.rect = menu_rect(Vec2::new(18.0, y), Vec2::new(240.0, 22.0));
        StorageSourceRowLayout { source, widget }
    }

    fn source_layout() -> StorageSourceLayout {
        StorageSourceLayout {
            widget_type: 2,
            panel_bounds: None,
            rows: vec![
                source_row(MapStorageSource::SampleMaps, 12, 201, 70.0),
                source_row(MapStorageSource::LocalStorage, 13, 202, 96.0),
            ],
        }
    }

    fn storage_source_state() -> MapMakerUiState {
        let mut state = MapMakerUiState::default();
        state.dispatch(MapMakerUiCommand::OpenStorage);
        state
    }

    #[test]
    fn storage_source_page_preserves_runtime_rows_and_owned_focus() {
        let mut state = storage_source_state();
        state.dispatch(MapMakerUiCommand::FocusStorageSource(
            MapStorageSource::LocalStorage,
        ));
        let built =
            build_storage_source_page(&state.snapshot(), PageId(0x771), source_layout()).unwrap();

        assert_eq!(built.page.id, PageId(0x771));
        assert_eq!(built.page.widget_type, 2);
        assert_eq!(built.page.nav, NavLayout::Grid(1));
        assert_eq!(built.page.wrap, Wrap::Clamp);
        assert_eq!(built.page.initial_focus, Some(WidgetId(13)));
        assert_eq!(built.page.widgets[0].text, TextRef::Id(TextId(201)));
        assert_eq!(
            built.source_for_widget(WidgetId(12)),
            Some(MapStorageSource::SampleMaps)
        );
        assert_eq!(
            built.source_for_widget(WidgetId(13)),
            Some(MapStorageSource::LocalStorage)
        );
        assert_eq!(built.source_for_widget(WidgetId(0x9002)), None);
    }

    #[test]
    fn storage_source_widget_activation_enters_the_selected_owned_route() {
        let mut state = storage_source_state();
        let built =
            build_storage_source_page(&state.snapshot(), PageId(1), source_layout()).unwrap();
        assert_eq!(
            built.activate_widget(&mut state, WidgetId(13)),
            MapMakerUiUpdate::PageChanged(MapMakerPage::MapList)
        );
        assert_eq!(
            state.focused_storage_source(),
            Some(MapStorageSource::LocalStorage)
        );
    }

    #[test]
    fn storage_source_page_rejects_wrong_page_and_duplicate_rows() {
        let mut state = storage_source_state();
        state.dispatch(MapMakerUiCommand::SelectLocalStorage);
        assert_eq!(
            build_storage_source_page(&state.snapshot(), PageId(1), source_layout()),
            Err(StorageSourceBuildError::WrongPage)
        );

        let mut state = storage_source_state();
        let mut layout = source_layout();
        layout.rows[1].source = MapStorageSource::SampleMaps;
        assert_eq!(
            build_storage_source_page(&state.snapshot(), PageId(1), layout),
            Err(StorageSourceBuildError::DuplicateSource)
        );
    }

    #[test]
    fn map_list_page_preserves_row_identity_and_focus() {
        let (state, first, second) = active_state();
        let built =
            build_map_list_page(&state.snapshot(), PageId(0xbeef), layout(first, second)).unwrap();

        assert_eq!(built.page.id, PageId(0xbeef));
        assert_eq!(built.page.widget_type, 1);
        assert_eq!(built.page.nav, NavLayout::Grid(1));
        assert_eq!(built.page.wrap, Wrap::Clamp);
        assert_eq!(built.page.initial_focus, Some(WidgetId(2)));
        assert_eq!(built.page.widgets[0].text, TextRef::Id(TextId(101)));
        assert_eq!(built.row_for_widget(WidgetId(1)), Some(first));
        assert_eq!(built.row_for_widget(WidgetId(2)), Some(second));
        assert_eq!(built.row_for_widget(WidgetId(first.key())), None);
    }

    #[test]
    fn activated_widget_requests_load_for_its_owned_row() {
        let (mut state, first, second) = active_state();
        let built =
            build_map_list_page(&state.snapshot(), PageId(0xbeef), layout(first, second)).unwrap();

        let MapMakerUiUpdate::LoadRequested(request) =
            built.activate_widget(&mut state, WidgetId(1))
        else {
            panic!("row activation should create a typed load request");
        };
        assert_eq!(request.row, first);
        assert_eq!(state.pending_map_load(), Some(request));
        assert_eq!(state.focused_map(), Some(first));
    }

    #[test]
    fn map_list_projection_rejects_wrong_page_and_stale_order() {
        let (mut state, first, second) = active_state();
        let snapshot = state.snapshot();
        assert_eq!(
            build_map_list_page(
                &snapshot,
                PageId(2),
                MapListLayout {
                    first_row_index: 0,
                    widget_type: 1,
                    panel_bounds: None,
                    rows: vec![row(second, 1, 102, 70.0), row(first, 2, 101, 96.0)],
                },
            ),
            Err(MapListBuildError::RowIdentityMismatch)
        );
        state.dispatch(MapMakerUiCommand::Back);
        assert_eq!(
            build_map_list_page(&state.snapshot(), PageId(1), layout(first, second)),
            Err(MapListBuildError::WrongPage)
        );
    }

    #[test]
    fn map_list_projection_accepts_a_visible_window_with_live_widget_ids() {
        let (state, _first, second) = active_state();
        let built = build_map_list_page(
            &state.snapshot(),
            PageId(0xbeef),
            MapListLayout {
                first_row_index: 1,
                widget_type: 1,
                panel_bounds: None,
                rows: vec![row(second, 9, 102, 70.0)],
            },
        )
        .unwrap();

        assert_eq!(built.page.initial_focus, Some(WidgetId(9)));
        assert_eq!(built.row_for_widget(WidgetId(9)), Some(second));
    }

    fn pending_storage_progress() -> MapMakerUiState {
        let mut state = MapMakerUiState::default();
        assert!(matches!(
            state.dispatch(MapMakerUiCommand::OpenStorageProgress(
                StorageOperation::SaveMap
            )),
            MapMakerUiUpdate::StorageProgressOpened(_)
        ));
        state
    }

    fn storage_progress_layout() -> StorageProgressLayout {
        let mut page = Page::new(PageId(0x32aee0), 1);
        let mut message = Widget::new(WidgetId(0x11), WidgetKind::Label);
        message.text = TextRef::Id(TextId(0xe68));
        message.rect = menu_rect(Vec2::new(41.0, 52.0), Vec2::new(230.0, 24.0));
        message.navigable = false;
        let mut warning = Widget::new(WidgetId(0x12), WidgetKind::Label);
        warning.text = TextRef::Id(TextId(0x1986));
        warning.rect = menu_rect(Vec2::new(31.0, 252.0), Vec2::new(440.0, 38.0));
        warning.navigable = false;
        page.widgets.extend([message, warning]);
        StorageProgressLayout {
            page,
            message_widget: WidgetId(0x11),
            warning_widget: WidgetId(0x12),
        }
    }

    #[test]
    fn storage_progress_projection_preserves_runtime_widgets_and_operation_identity() {
        let state = pending_storage_progress();
        let layout = storage_progress_layout();
        let expected_page = layout.page.clone();

        let projected = build_storage_progress_page(&state.snapshot(), layout).unwrap();

        assert_eq!(projected.page, expected_page);
        assert_eq!(projected.page.id, PageId(0x32aee0));
        assert_eq!(projected.message_widget, WidgetId(0x11));
        assert_eq!(projected.warning_widget, WidgetId(0x12));
        assert_eq!(projected.progress.operation, StorageOperation::SaveMap);
        assert_eq!(projected.progress.outcome, StorageProgressOutcome::Pending);
    }

    #[test]
    fn storage_progress_projection_rejects_wrong_page_and_preserves_completion() {
        let state = MapMakerUiState::default();
        assert_eq!(
            build_storage_progress_page(&state.snapshot(), storage_progress_layout()),
            Err(StorageProgressBuildError::WrongPage)
        );

        let mut state = pending_storage_progress();
        let progress = state.storage_progress().unwrap().id;
        assert!(matches!(
            state.dispatch(MapMakerUiCommand::CompleteStorageProgress {
                progress,
                outcome: StorageProgressOutcome::Succeeded,
            }),
            MapMakerUiUpdate::StorageProgressCompleted(_)
        ));
        let completed =
            build_storage_progress_page(&state.snapshot(), storage_progress_layout()).unwrap();
        assert_eq!(
            completed.progress.outcome,
            StorageProgressOutcome::Succeeded
        );
    }

    #[test]
    fn storage_progress_projection_rejects_interactive_or_duplicate_status_widgets() {
        let state = pending_storage_progress();
        let mut layout = storage_progress_layout();
        layout.page.widgets[0].navigable = true;
        assert_eq!(
            build_storage_progress_page(&state.snapshot(), layout),
            Err(StorageProgressBuildError::InteractiveStatusWidget)
        );

        let mut layout = storage_progress_layout();
        layout.warning_widget = layout.message_widget;
        assert_eq!(
            build_storage_progress_page(&state.snapshot(), layout),
            Err(StorageProgressBuildError::DuplicateStatusWidget)
        );
    }
}
