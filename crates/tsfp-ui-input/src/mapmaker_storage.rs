//! Native input routing for the Rust-owned Map Maker storage source and map list.
//!
//! Device adapters supply semantic intent or hit-tested opaque row IDs. This
//! module emits page commands only; the storage host remains responsible for
//! authenticated load and completion results.

use crate::{input::IntentSet, owned_catalogue::CatalogueAction};
use tsfp_mapmaker::ui::{
    MapListRowId, MapMakerPage, MapMakerUiCommand, MapMakerUiState, MapMakerUiUpdate,
    MapStorageSource, StorageProgressId, StorageProgressOutcome,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    Hover(MapListRowId),
    Click(MapListRowId),
    HoverSource(MapStorageSource),
    ClickSource(MapStorageSource),
    Back,
}

fn storage_source(index: usize) -> Option<MapStorageSource> {
    match index {
        0 => Some(MapStorageSource::SampleMaps),
        1 => Some(MapStorageSource::LocalStorage),
        _ => None,
    }
}

fn storage_source_index(source: MapStorageSource) -> usize {
    match source {
        MapStorageSource::SampleMaps => 0,
        MapStorageSource::LocalStorage => 1,
    }
}

/// Resolve the shared catalogue action vocabulary against the active Map Maker
/// storage page. Up/Down clamp at the ends, and an unfocused list starts at the
/// first row for Down or last for Up. This focus policy is **INFERRED** until a
/// Map Maker owner route supplies Tier 2 evidence.
pub fn commands(state: &MapMakerUiState, action: CatalogueAction) -> Vec<MapMakerUiCommand> {
    match (state.page(), action) {
        (MapMakerPage::StorageSource, CatalogueAction::Accept) => {
            match state.focused_storage_source() {
                Some(MapStorageSource::LocalStorage) => vec![MapMakerUiCommand::SelectLocalStorage],
                Some(MapStorageSource::SampleMaps) => vec![MapMakerUiCommand::SelectSampleMaps],
                _ => Vec::new(),
            }
        }
        (MapMakerPage::StorageSource, CatalogueAction::Focus(index)) => storage_source(index)
            .map(|source| vec![MapMakerUiCommand::FocusStorageSource(source)])
            .unwrap_or_default(),
        (MapMakerPage::StorageSource, CatalogueAction::Step(delta)) => {
            if delta == 0 {
                return Vec::new();
            }
            let focused = state.focused_storage_source();
            let index = focused.map(storage_source_index);
            let next = if delta < 0 {
                index.map_or(1, |index| index.saturating_sub(1))
            } else {
                index.map_or(0, |index| (index + 1).min(1))
            };
            storage_source(next)
                .map(|source| vec![MapMakerUiCommand::FocusStorageSource(source)])
                .unwrap_or_default()
        }
        (MapMakerPage::StorageSource, CatalogueAction::Back) => vec![MapMakerUiCommand::Back],
        (MapMakerPage::MapList, CatalogueAction::Back) => vec![MapMakerUiCommand::Back],
        (MapMakerPage::StorageProgress, CatalogueAction::Back) => {
            vec![MapMakerUiCommand::Back]
        }
        (MapMakerPage::MapList, CatalogueAction::Focus(index)) => state
            .map_rows()
            .get(index)
            .map(|row| vec![MapMakerUiCommand::FocusMap(row.id)])
            .unwrap_or_default(),
        (MapMakerPage::MapList, CatalogueAction::Step(delta)) => {
            let rows = state.map_rows();
            if rows.is_empty() || delta == 0 {
                return Vec::new();
            }
            let current = state
                .focused_map()
                .and_then(|id| rows.iter().position(|row| row.id == id));
            let next = if delta < 0 {
                current.map_or(rows.len() - 1, |index| index.saturating_sub(1))
            } else {
                current.map_or(0, |index| (index + 1).min(rows.len() - 1))
            };
            vec![MapMakerUiCommand::FocusMap(rows[next].id)]
        }
        (MapMakerPage::MapList, CatalogueAction::Accept) => state
            .focused_map()
            .map(|id| vec![MapMakerUiCommand::SelectMap(id)])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Apply the shared keyboard/controller action to Rust-owned storage state.
pub fn dispatch_action(
    state: &mut MapMakerUiState,
    action: CatalogueAction,
) -> Vec<MapMakerUiUpdate> {
    if state.page() == MapMakerPage::StorageProgress && action == CatalogueAction::Back {
        let Some(progress) = state.storage_progress() else {
            return Vec::new();
        };
        return dispatch_progress_back(state, progress.id)
            .into_iter()
            .collect();
    }
    commands(state, action)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

/// Dismiss only the completed progress operation currently owned by Rust.
/// The identity guard rejects stale page snapshots and pending operations.
pub fn dispatch_progress_back(
    state: &mut MapMakerUiState,
    expected: StorageProgressId,
) -> Option<MapMakerUiUpdate> {
    let progress = state.storage_progress()?;
    if state.page() != MapMakerPage::StorageProgress
        || progress.id != expected
        || progress.outcome == StorageProgressOutcome::Pending
    {
        return None;
    }
    match state.dispatch(MapMakerUiCommand::Back) {
        update @ MapMakerUiUpdate::StorageProgressDismissed(_) => Some(update),
        _ => None,
    }
}

/// Convert a semantic keyboard/controller frame into storage-state updates.
/// The common input reducer supplies edge and repeat timing; Back keeps its
/// established precedence over row movement or acceptance.
pub fn dispatch_frame(
    state: &mut MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiUpdate> {
    crate::owned_catalogue::actions(held, before, now_ms, repeat_at, false)
        .into_iter()
        .flat_map(|action| dispatch_action(state, action))
        .collect()
}

/// Resolve a hit-tested pointer event to the same owned-row command as the
/// keyboard path. Hover only moves focus; click also requests a host load.
pub fn pointer_commands(state: &MapMakerUiState, event: PointerEvent) -> Vec<MapMakerUiCommand> {
    if state.page() == MapMakerPage::StorageSource {
        return match event {
            PointerEvent::Back => vec![MapMakerUiCommand::Back],
            PointerEvent::HoverSource(source) if state.focused_storage_source() != Some(source) => {
                vec![MapMakerUiCommand::FocusStorageSource(source)]
            }
            PointerEvent::ClickSource(source) => {
                let mut commands = Vec::with_capacity(2);
                if state.focused_storage_source() != Some(source) {
                    commands.push(MapMakerUiCommand::FocusStorageSource(source));
                }
                commands.push(match source {
                    MapStorageSource::SampleMaps => MapMakerUiCommand::SelectSampleMaps,
                    MapStorageSource::LocalStorage => MapMakerUiCommand::SelectLocalStorage,
                });
                commands
            }
            _ => Vec::new(),
        };
    }
    if state.page() == MapMakerPage::StorageProgress {
        return match event {
            PointerEvent::Back
                if state.storage_progress().is_some_and(|progress| {
                    progress.outcome != StorageProgressOutcome::Pending
                }) =>
            {
                vec![MapMakerUiCommand::Back]
            }
            _ => Vec::new(),
        };
    }
    if state.page() != MapMakerPage::MapList {
        return Vec::new();
    }
    match event {
        PointerEvent::Back => vec![MapMakerUiCommand::Back],
        PointerEvent::HoverSource(_) | PointerEvent::ClickSource(_) => Vec::new(),
        PointerEvent::Hover(id) | PointerEvent::Click(id) => {
            if !state.map_rows().iter().any(|row| row.id == id) {
                return Vec::new();
            }
            let clicked = matches!(event, PointerEvent::Click(_));
            let mut result = Vec::with_capacity(2);
            if state.focused_map() != Some(id) {
                result.push(MapMakerUiCommand::FocusMap(id));
            }
            if clicked {
                result.push(MapMakerUiCommand::SelectMap(id));
            }
            result
        }
    }
}

pub fn dispatch_pointer(state: &mut MapMakerUiState, event: PointerEvent) -> Vec<MapMakerUiUpdate> {
    pointer_commands(state, event)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        PointerEvent, dispatch_action, dispatch_frame, dispatch_pointer, dispatch_progress_back,
    };
    use crate::{input::MenuIntent as I, owned_catalogue::CatalogueAction as A};
    use tsfp_mapmaker::ui::{
        MapListGeneration, MapListRow, MapListRowId, MapMakerPage, MapMakerUiCommand as C,
        MapMakerUiState, MapMakerUiUpdate as U, MapStorageSource, StorageOperation,
        StorageProgressOutcome,
    };

    fn state() -> (MapMakerUiState, MapListRowId, MapListRowId) {
        let generation = MapListGeneration::new(6);
        let first = MapListRowId::new(generation, 301);
        let second = MapListRowId::new(generation, 907);
        let mut state = MapMakerUiState::default();
        state
            .install_local_map_rows(
                generation,
                vec![MapListRow { id: first }, MapListRow { id: second }],
            )
            .unwrap();
        state.dispatch(C::OpenStorage);
        state.dispatch(C::SelectLocalStorage);
        (state, first, second)
    }

    #[test]
    fn storage_source_accept_opens_the_focused_sample_or_local_list() {
        let mut state = MapMakerUiState::default();
        state.dispatch(C::OpenStorage);
        assert!(dispatch_action(&mut state, A::Accept).is_empty());
        assert_eq!(state.page(), MapMakerPage::StorageSource);

        assert_eq!(
            dispatch_action(&mut state, A::Focus(0)),
            [U::StorageSourceFocused(MapStorageSource::SampleMaps)]
        );
        assert_eq!(
            dispatch_action(&mut state, A::Accept),
            [U::PageChanged(MapMakerPage::MapList)]
        );
        assert_eq!(state.page(), MapMakerPage::MapList);

        state.dispatch(C::Back);
        assert_eq!(state.page(), MapMakerPage::StorageSource);

        assert_eq!(
            dispatch_action(&mut state, A::Focus(1)),
            [U::StorageSourceFocused(MapStorageSource::LocalStorage)]
        );
        assert_eq!(
            dispatch_action(&mut state, A::Accept),
            [U::PageChanged(MapMakerPage::MapList)]
        );
        assert_eq!(state.page(), MapMakerPage::MapList);
    }

    #[test]
    fn storage_source_pointer_focus_and_click_dispatch_typed_routes() {
        let mut state = MapMakerUiState::default();
        state.dispatch(C::OpenStorage);
        assert_eq!(
            dispatch_pointer(
                &mut state,
                PointerEvent::HoverSource(MapStorageSource::LocalStorage)
            ),
            [U::StorageSourceFocused(MapStorageSource::LocalStorage)]
        );
        assert_eq!(
            dispatch_pointer(
                &mut state,
                PointerEvent::ClickSource(MapStorageSource::SampleMaps)
            ),
            [
                U::StorageSourceFocused(MapStorageSource::SampleMaps),
                U::PageChanged(MapMakerPage::MapList),
            ]
        );
        assert_eq!(state.page(), MapMakerPage::MapList);
        assert_eq!(
            state.focused_storage_source(),
            Some(MapStorageSource::SampleMaps)
        );
        assert!(
            dispatch_pointer(
                &mut state,
                PointerEvent::ClickSource(MapStorageSource::LocalStorage)
            )
            .is_empty()
        );
    }

    #[test]
    fn progress_back_is_owned_completed_and_generation_checked() {
        let mut state = MapMakerUiState::default();
        state.dispatch(C::OpenStorageProgress(StorageOperation::SaveMap));
        let first = state.storage_progress().unwrap();
        assert!(dispatch_progress_back(&mut state, first.id).is_none());
        assert!(dispatch_pointer(&mut state, PointerEvent::Back).is_empty());
        assert_eq!(state.page(), MapMakerPage::StorageProgress);

        state.dispatch(C::CompleteStorageProgress {
            progress: first.id,
            outcome: StorageProgressOutcome::Failed,
        });
        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::Back),
            [U::StorageProgressDismissed(StorageOperation::SaveMap)]
        );
        assert_eq!(state.page(), MapMakerPage::EditorMenu);

        state.dispatch(C::OpenStorageProgress(StorageOperation::LoadMap));
        let second = state.storage_progress().unwrap();
        assert_ne!(first.id, second.id);
        state.dispatch(C::CompleteStorageProgress {
            progress: second.id,
            outcome: StorageProgressOutcome::Succeeded,
        });
        assert!(dispatch_progress_back(&mut state, first.id).is_none());
        assert_eq!(state.page(), MapMakerPage::StorageProgress);
        assert_eq!(
            dispatch_frame(
                &mut state,
                crate::input::IntentSet::default().with(I::Back),
                crate::input::IntentSet::default(),
                30,
                &mut [0; 4],
            ),
            [U::StorageProgressDismissed(StorageOperation::LoadMap)]
        );
        assert!(dispatch_progress_back(&mut state, second.id).is_none());
    }

    #[test]
    fn semantic_keyboard_path_focuses_then_accepts_owned_row() {
        let (mut state, first, second) = state();
        assert_eq!(
            dispatch_frame(
                &mut state,
                crate::input::IntentSet::default().with(I::Down),
                crate::input::IntentSet::default(),
                10,
                &mut [0; 4],
            ),
            [U::MapFocused(first)]
        );
        assert_eq!(
            dispatch_frame(
                &mut state,
                crate::input::IntentSet::default().with(I::Down),
                crate::input::IntentSet::default(),
                20,
                &mut [0; 4],
            ),
            [U::MapFocused(second)]
        );
        assert!(matches!(
            dispatch_frame(
                &mut state,
                crate::input::IntentSet::default().with(I::Accept),
                crate::input::IntentSet::default(),
                30,
                &mut [0; 4],
            )
            .as_slice(),
            [U::LoadRequested(request)] if request.row == second
        ));
        assert_eq!(state.page(), MapMakerPage::MapList);
    }

    #[test]
    fn sample_map_keyboard_path_emits_the_typed_load_request() {
        let generation = MapListGeneration::new(23);
        let first = MapListRowId::new(generation, 41);
        let second = MapListRowId::new(generation, 73);
        let mut state = MapMakerUiState::default();
        state.dispatch(C::OpenStorage);
        dispatch_action(&mut state, A::Focus(0));
        assert_eq!(
            dispatch_action(&mut state, A::Accept),
            [U::PageChanged(MapMakerPage::MapList)]
        );
        state
            .install_map_rows(
                generation,
                vec![MapListRow { id: first }, MapListRow { id: second }],
            )
            .unwrap();

        assert_eq!(
            dispatch_frame(
                &mut state,
                crate::input::IntentSet::default().with(I::Down),
                crate::input::IntentSet::default(),
                10,
                &mut [0; 4],
            ),
            [U::MapFocused(first)]
        );
        assert_eq!(
            dispatch_frame(
                &mut state,
                crate::input::IntentSet::default().with(I::Accept),
                crate::input::IntentSet::default(),
                20,
                &mut [0; 4],
            ),
            [U::LoadRequested(
                state.pending_map_load().expect("accepted sample request")
            )]
        );
        assert_eq!(state.pending_map_load().unwrap().row, first);
    }

    #[test]
    fn back_wins_and_pointer_click_uses_the_same_row_identity() {
        let (mut state, _, second) = state();
        let held = crate::input::IntentSet::default()
            .with(I::Down)
            .with(I::Accept)
            .with(I::Back);
        assert_eq!(
            dispatch_frame(
                &mut state,
                held,
                crate::input::IntentSet::default(),
                1,
                &mut [0; 4]
            ),
            [U::PageChanged(MapMakerPage::StorageSource)]
        );
        state.dispatch(C::SelectLocalStorage);
        assert!(matches!(
            dispatch_pointer(&mut state, PointerEvent::Click(second)).as_slice(),
            [U::MapFocused(id), U::LoadRequested(request)] if *id == second && request.row == second
        ));
        assert!(
            dispatch_pointer(
                &mut state,
                PointerEvent::Hover(MapListRowId::new(MapListGeneration::new(7), 907))
            )
            .is_empty()
        );
    }

    #[test]
    fn keyboard_back_exits_storage_source_without_device_emulation() {
        let (mut state, _, _) = state();
        state.dispatch(C::Back);
        assert_eq!(
            dispatch_frame(
                &mut state,
                crate::input::IntentSet::default().with(I::Back),
                crate::input::IntentSet::default(),
                2,
                &mut [0; 4],
            ),
            [U::PageChanged(MapMakerPage::EditorMenu)]
        );
    }

    #[test]
    fn semantic_keyboard_accept_opens_local_storage_source() {
        let (mut state, _, _) = state();
        state.dispatch(C::Back);
        assert_eq!(state.page(), MapMakerPage::StorageSource);
        assert_eq!(
            dispatch_frame(
                &mut state,
                crate::input::IntentSet::default().with(I::Accept),
                crate::input::IntentSet::default(),
                3,
                &mut [0; 4],
            ),
            [U::PageChanged(MapMakerPage::MapList)]
        );
    }

    #[test]
    fn keyboard_back_cannot_leave_pending_storage_progress() {
        let mut state = MapMakerUiState::default();
        let [U::StorageProgressOpened(progress)] =
            [state.dispatch(C::OpenStorageProgress(StorageOperation::SaveMap))]
        else {
            panic!("open should create a storage progress page");
        };
        assert!(
            dispatch_frame(
                &mut state,
                crate::input::IntentSet::default().with(I::Back),
                crate::input::IntentSet::default(),
                10,
                &mut [0; 4],
            )
            .is_empty()
        );
        assert_eq!(state.page(), MapMakerPage::StorageProgress);
        assert_eq!(
            state.storage_progress().unwrap().outcome,
            StorageProgressOutcome::Pending
        );

        assert!(matches!(
            state.dispatch(C::CompleteStorageProgress {
                progress,
                outcome: StorageProgressOutcome::Succeeded,
            }),
            U::StorageProgressCompleted(_)
        ));
        assert_eq!(
            dispatch_frame(
                &mut state,
                crate::input::IntentSet::default().with(I::Back),
                crate::input::IntentSet::default(),
                20,
                &mut [0; 4],
            ),
            [U::StorageProgressDismissed(StorageOperation::SaveMap)]
        );
        assert_eq!(state.page(), MapMakerPage::EditorMenu);
    }
}
