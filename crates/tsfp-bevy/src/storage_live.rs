//! T2319: native device queries for the shell's actual persistent save root.
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tsfp_engine_fileio::local_storage::LocalStorage;

#[derive(Debug, PartialEq, Eq)]
enum DiagnosticWindow {
    Profile,
    Map,
    MapName(String),
}

/// Explicit measurement routes are admitted only behind the copied-root guard.
fn diagnostic_window(
    request: Option<&str>,
    enabled: bool,
) -> std::io::Result<Option<DiagnosticWindow>> {
    if !enabled {
        return if request.is_some() {
            Err(std::io::Error::other(
                "storage window diagnostic requires the guarded roundtrip diagnostic",
            ))
        } else {
            Ok(None)
        };
    }
    let window = match request.unwrap_or("profile") {
        "profile" => DiagnosticWindow::Profile,
        "map" => DiagnosticWindow::Map,
        value => {
            let directory = value.strip_prefix("map-name:").ok_or_else(|| std::io::Error::other("storage diagnostic window must be profile, map or map-name:<new-save-directory>"))?;
            crate::storage_caller::MapName::new(directory.to_owned()).map_err(|_| {
                std::io::Error::other("storage diagnostic map save directory is invalid")
            })?;
            DiagnosticWindow::MapName(directory.to_owned())
        }
    };
    Ok(Some(window))
}

static DIAGNOSTIC_WINDOW: OnceLock<DiagnosticWindow> = OnceLock::new();
static STORAGE: OnceLock<LocalStorage> = OnceLock::new();
static SESSION: Mutex<Option<crate::storage_session::StorageSession>> = Mutex::new(None);
static MAPMAKER_UI: OnceLock<Mutex<tsfp_mapmaker::ui::MapMakerUiState>> = OnceLock::new();
static MAPMAKER_LIST_GENERATION: AtomicU64 = AtomicU64::new(0);
static MAPMAKER_TEXT_KEYS: std::sync::LazyLock<
    Mutex<std::collections::HashSet<bevy::input::keyboard::KeyCode>>,
> = std::sync::LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));
static MAPMAKER_BRIEFING_TEXT_OUTCOMES: Mutex<
    std::collections::VecDeque<(
        tsfp_mapmaker::ui::EditorSurfaceTarget,
        tsfp_mapmaker::ui::BriefingTextOutcome,
    )>,
> = Mutex::new(std::collections::VecDeque::new());
static WINDOW_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static STORY_PROGRESS_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn mapmaker_ui_enabled_from(value: Option<&str>) -> bool {
    value == Some("rust")
}

pub(crate) fn mapmaker_ui_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED
        .get_or_init(|| mapmaker_ui_enabled_from(std::env::var("TSFP_MAPMAKER_UI").ok().as_deref()))
}

pub(crate) fn mapmaker_ui() -> &'static Mutex<tsfp_mapmaker::ui::MapMakerUiState> {
    MAPMAKER_UI.get_or_init(|| Mutex::new(tsfp_mapmaker::ui::MapMakerUiState::default()))
}

/// Publish history availability only when the Rust Map Maker history candidate is active and
/// the native lifecycle owns the currently edited document. `None` preserves the lifted page's
/// existing enablement policy until the Rule 24 replay gate admits this bridge.
pub(crate) fn mapmaker_editor_history_availability()
-> Option<tsfp_mapmaker::history::HistoryAvailability> {
    let guard = SESSION.lock().ok()?;
    let session = guard.as_ref()?;
    editor_history_availability(mapmaker_history_enabled(), Some(&session.lifecycle))
}

fn editor_history_availability(
    enabled: bool,
    lifecycle: Option<&tsfp_engine_fileio::storage_lifecycle::NativeStorageLifecycle>,
) -> Option<tsfp_mapmaker::history::HistoryAvailability> {
    use tsfp_mapmaker::history::HistoryAvailability;

    if !enabled {
        return None;
    }
    let lifecycle = lifecycle?;
    lifecycle.editor()?;
    Some(HistoryAvailability {
        can_undo: lifecycle.editor_can_undo(),
        can_redo: lifecycle.editor_can_redo(),
    })
}

/// Record keyboard, controller, or pointer activation in the Rust-owned UI
/// state. The original row callback remains the temporary mode-transfer seam.
pub(crate) fn record_mapmaker_experience_prompt_outcome(
    focused_row: &mut Option<u32>,
    command: tsfp_ui_input::native_page::NativeCommand,
    outcome: tsfp_ui_input::native_page::Outcome,
) -> bool {
    if !mapmaker_ui_enabled() {
        return false;
    }
    let Ok(mut state) = mapmaker_ui().lock() else {
        return false;
    };
    tsfp_ui_input::mapmaker_experience::apply_native_outcome(
        &mut state,
        focused_row,
        command,
        outcome,
    )
}

fn mapmaker_progress_widget(
    page: &tsfp_ui_model::Page,
    text_id: tsfp_ui_model::model::TextId,
) -> Result<tsfp_ui_model::WidgetId, tsfp_ui_model::mapmaker_storage::StorageProgressBuildError> {
    use tsfp_ui_model::mapmaker_storage::StorageProgressBuildError as Error;

    let mut matches = page
        .widgets
        .iter()
        .filter(|widget| widget.text == tsfp_ui_model::model::TextRef::Id(text_id));
    let Some(widget) = matches.next() else {
        return Err(Error::MissingStatusWidget);
    };
    if matches.next().is_some() {
        return Err(Error::DuplicateStatusWidget);
    }
    Ok(widget.id)
}

/// Attach pending Rust storage state to the original status page at the
/// registry boundary. Runtime layout/text remain live-page data; T1714's
/// measured text IDs locate the two status labels without copying strings.
fn project_mapmaker_storage_progress_from_state(
    state: &tsfp_mapmaker::ui::MapMakerUiState,
    page: tsfp_ui_model::Page,
) -> Result<
    tsfp_ui_model::mapmaker_storage::StorageProgressPage,
    tsfp_ui_model::mapmaker_storage::StorageProgressBuildError,
> {
    use tsfp_ui_model::mapmaker_storage::{StorageProgressBuildError, StorageProgressLayout};

    if page.id != tsfp_ui_model::PageId(0x32_AEE0) {
        return Err(StorageProgressBuildError::WrongPage);
    }
    let message_widget = mapmaker_progress_widget(&page, tsfp_ui_model::model::TextId(0x0E68))?;
    let warning_widget = mapmaker_progress_widget(&page, tsfp_ui_model::model::TextId(0x1986))?;
    tsfp_ui_model::mapmaker_storage::build_storage_progress_page(
        &state.snapshot(),
        StorageProgressLayout {
            page,
            message_widget,
            warning_widget,
        },
    )
}

/// Registry adapter; absent or invalid Rust progress leaves the measured live
/// page on its existing path and never fabricates an operation result.
pub(crate) fn project_live_mapmaker_storage_progress(
    page: tsfp_ui_model::Page,
) -> Option<tsfp_ui_model::mapmaker_storage::StorageProgressPage> {
    if !mapmaker_ui_enabled() {
        return None;
    }
    let ui = mapmaker_ui().lock().ok()?;
    project_mapmaker_storage_progress_from_state(&ui, page).ok()
}

/// Apply the Rust-owned dismissal before forwarding the matching Back action
/// to the live page's typed legacy boundary.
pub(crate) fn dismiss_live_mapmaker_storage_progress(
    progress: tsfp_mapmaker::ui::StorageProgressId,
) -> bool {
    let Ok(mut ui) = mapmaker_ui().lock() else {
        return false;
    };
    matches!(
        tsfp_ui_input::mapmaker_storage::dispatch_progress_back(&mut ui, progress),
        Some(tsfp_mapmaker::ui::MapMakerUiUpdate::StorageProgressDismissed(_))
    )
}

fn install_mapmaker_rows(
    ui: &mut tsfp_mapmaker::ui::MapMakerUiState,
    row_count: usize,
    cursor: Option<usize>,
    source: tsfp_mapmaker::ui::MapStorageSource,
) -> bool {
    let generation = tsfp_mapmaker::ui::MapListGeneration::new(
        MAPMAKER_LIST_GENERATION
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1),
    );
    let rows = (0..row_count)
        .map(|key| tsfp_mapmaker::ui::MapListRow {
            id: tsfp_mapmaker::ui::MapListRowId::new(generation, key as u64),
        })
        .collect();
    if ui.install_map_rows(generation, rows).is_err() {
        return false;
    }
    if ui.page() == tsfp_mapmaker::ui::MapMakerPage::StorageSource {
        let command = match source {
            tsfp_mapmaker::ui::MapStorageSource::SampleMaps => {
                tsfp_mapmaker::ui::MapMakerUiCommand::SelectSampleMaps
            }
            tsfp_mapmaker::ui::MapStorageSource::LocalStorage => {
                tsfp_mapmaker::ui::MapMakerUiCommand::SelectLocalStorage
            }
        };
        ui.dispatch(command);
    }
    if let Some(row) = cursor.and_then(|index| ui.map_rows().get(index)) {
        ui.dispatch(tsfp_mapmaker::ui::MapMakerUiCommand::FocusMap(row.id));
    }
    true
}

fn install_mapmaker_local_rows(
    ui: &mut tsfp_mapmaker::ui::MapMakerUiState,
    row_count: usize,
    cursor: Option<usize>,
) -> bool {
    install_mapmaker_rows(
        ui,
        row_count,
        cursor,
        tsfp_mapmaker::ui::MapStorageSource::LocalStorage,
    )
}

fn install_mapmaker_sample_rows(
    ui: &mut tsfp_mapmaker::ui::MapMakerUiState,
    row_count: usize,
    cursor: Option<usize>,
) -> bool {
    install_mapmaker_rows(
        ui,
        row_count,
        cursor,
        tsfp_mapmaker::ui::MapStorageSource::SampleMaps,
    )
}

/// Apply one pointer-derived catalogue action to the owned Map Maker page
/// before the storage host handles it. This keeps hit-tested row identity,
/// keyboard focus, and the load request on one Rust state path.
fn mapmaker_catalogue_action_begin(
    ui: &mut tsfp_mapmaker::ui::MapMakerUiState,
    view: &crate::storage_caller::CatalogueView,
    action: crate::storage_caller::CatalogueAction,
) -> Option<tsfp_mapmaker::ui::MapLoadRequest> {
    use crate::storage_caller::CatalogueAction;
    use tsfp_engine_fileio::storage_frontend::Target;
    use tsfp_mapmaker::ui::{
        MapMakerPage, MapMakerUiCommand, MapMakerUiState, MapMakerUiUpdate, MapStorageSource,
    };

    if view.target != Target::Map {
        return None;
    }
    if view.map_source_choice {
        if ui.page() != MapMakerPage::StorageSource {
            *ui = MapMakerUiState::default();
            ui.dispatch(MapMakerUiCommand::OpenStorage);
        }
        if let Some(source) = view.cursor.and_then(|index| match index {
            0 => Some(MapStorageSource::SampleMaps),
            1 => Some(MapStorageSource::LocalStorage),
            _ => None,
        }) {
            ui.dispatch(MapMakerUiCommand::FocusStorageSource(source));
        }
        tsfp_ui_input::mapmaker_storage::dispatch_action(ui, action);
        return None;
    }
    if matches!(ui.page(), MapMakerPage::EditorMenu | MapMakerPage::Editor) {
        *ui = MapMakerUiState::default();
        ui.dispatch(MapMakerUiCommand::OpenStorage);
    }
    // The selected typed source owns the list. The presence of disc samples in
    // the catalogue does not switch a local HDD session to the sample list.
    let source = ui
        .focused_storage_source()
        .unwrap_or(MapStorageSource::LocalStorage);
    let sample_source = source == MapStorageSource::SampleMaps;
    let row_count = if sample_source {
        view.sample_maps.len()
    } else {
        view.rows.len()
    };
    if matches!(
        ui.page(),
        MapMakerPage::StorageSource | MapMakerPage::MapList
    ) {
        // Rebind identities from the current catalogue snapshot even when its
        // row count is unchanged. A replaced map at the same index must not
        // inherit the previous entry's generation-qualified target.
        if sample_source {
            install_mapmaker_sample_rows(ui, row_count, view.cursor);
        } else {
            install_mapmaker_local_rows(ui, row_count, view.cursor);
        }
    }
    if ui.page() != MapMakerPage::MapList {
        return None;
    }
    if let Some(row_id) = view
        .cursor
        .and_then(|index| ui.map_rows().get(index).map(|row| row.id))
    {
        ui.dispatch(MapMakerUiCommand::FocusMap(row_id));
    }
    tsfp_ui_input::mapmaker_storage::dispatch_action(ui, action)
        .into_iter()
        .find_map(|update| match update {
            MapMakerUiUpdate::LoadRequested(request) => Some(request),
            _ => None,
        })
}

fn mapmaker_catalogue_action_finish(
    ui: &mut tsfp_mapmaker::ui::MapMakerUiState,
    action: crate::storage_caller::CatalogueAction,
    request: Option<tsfp_mapmaker::ui::MapLoadRequest>,
    result: &Result<Option<tsfp_engine_fileio::storage_frontend::Completion>, u32>,
    after: Option<crate::storage_caller::CatalogueView>,
) {
    use crate::storage_caller::CatalogueAction;
    use tsfp_engine_fileio::storage_frontend::Target;
    use tsfp_mapmaker::ui::{MapLoadOutcome, MapMakerPage, MapMakerUiCommand, MapMakerUiState};

    if let Some(request) = request {
        let outcome = match result {
            Ok(Some(_)) => Some(MapLoadOutcome::Loaded),
            Err(_) => Some(MapLoadOutcome::Failed),
            Ok(None) => None,
        };
        if let Some(outcome) = outcome {
            ui.dispatch(MapMakerUiCommand::CompleteMapLoad {
                request: request.id,
                outcome,
            });
        }
    }
    if let Some(view) = after.as_ref()
        && view.target == Target::Map
        && !view.map_source_choice
        && matches!(
            ui.page(),
            MapMakerPage::StorageSource | MapMakerPage::MapList
        )
    {
        let source = ui
            .focused_storage_source()
            .unwrap_or(tsfp_mapmaker::ui::MapStorageSource::LocalStorage);
        let sample_source = source == tsfp_mapmaker::ui::MapStorageSource::SampleMaps;
        let row_count = if sample_source {
            view.sample_maps.len()
        } else {
            view.rows.len()
        };
        if matches!(
            ui.page(),
            MapMakerPage::StorageSource | MapMakerPage::MapList
        ) {
            install_mapmaker_rows(ui, row_count, view.cursor, source);
        }
    }
    if matches!(action, CatalogueAction::Back) && after.is_none() {
        *ui = MapMakerUiState::default();
    }
}

fn mapmaker_aware_catalogue_action(
    session: &mut crate::storage_session::StorageSession,
    action: crate::storage_caller::CatalogueAction,
    samples: &mut impl tsfp_engine_fileio::storage_flow::SampleMaps,
    ui: Option<&mut tsfp_mapmaker::ui::MapMakerUiState>,
) -> Result<Option<tsfp_engine_fileio::storage_frontend::Completion>, u32> {
    let mut ui = ui;
    let before = session.catalogue_view();
    let request = match (ui.as_deref_mut(), before.as_ref()) {
        (Some(ui), Some(view)) => mapmaker_catalogue_action_begin(ui, view, action),
        _ => None,
    };
    let result = session.catalogue_action(action, samples);
    if let Some(ui) = ui {
        mapmaker_catalogue_action_finish(ui, action, request, &result, session.catalogue_view());
    }
    result
}

fn mapmaker_history_enabled_from(value: Option<&str>) -> bool {
    value == Some("1")
}

/// T2383 native history adoption is opt-in until the owner checks natural editor behavior.
pub fn mapmaker_history_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        mapmaker_history_enabled_from(std::env::var("TSFP_UI_MAPMAKER_HISTORY").ok().as_deref())
    })
}

pub(crate) fn set_story_progress_enabled(enabled: bool) {
    STORY_PROGRESS_ENABLED.store(enabled, Ordering::Release);
}

/// Submit one native gameplay result to the authenticated Story profile owner.
/// The host switch is opt-in; an absent StorageSession or disabled route cannot save.
pub fn native_story_outcome(
    player: usize,
    flow: &tsfp_state::story_flow::StoryFlow,
    outcome: &tsfp_gameplay::story_events::slots::MissionOutcome,
    mode: tsfp_engine_fileio::story_progress::StoryMode,
    best_time: f32,
    minimum_time: f32,
) -> Result<crate::story_progress_bridge::OutcomeResult, u32> {
    use tsfp_engine_fileio::vfs::status;
    if !STORY_PROGRESS_ENABLED.load(Ordering::Acquire) {
        return Ok(crate::story_progress_bridge::OutcomeResult::Disabled);
    }
    let mut guard = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let session = guard.as_mut().ok_or(status::INVALID_PARAMETER)?;
    let result =
        session.adopt_story_outcome(player, flow, outcome, mode, best_time, minimum_time)?;
    if let crate::story_progress_bridge::OutcomeResult::Committed {
        campaign_ordinal, ..
    } = &result
    {
        eprintln!("tsfp-bevy: Rust Story progress committed (campaign ordinal {campaign_ordinal})");
    }
    Ok(result)
}

/// Commit the oldest queued terminal Story result using the producing mission's session token
/// and the caller's explicit profile and timing context. A session mismatch prevents the profile
/// consumer from running; disabled progress defers the item, and storage failures retain it.
pub fn native_commit_next_story_mission_result(
    session: tsfp_gameplay::story_events::native_world::StoryGameplayInputSession,
    player: usize,
    mode: tsfp_engine_fileio::story_progress::StoryMode,
    best_time: f32,
    minimum_time: f32,
) -> Result<
    crate::story_mission_runtime::StoryMissionResultProcess<
        crate::story_progress_bridge::OutcomeResult,
    >,
    crate::story_mission_runtime::StoryMissionResultProcessError<u32>,
> {
    crate::story_mission_runtime::story_mission_result_store().process_next(session, |envelope| {
        let result = native_story_outcome(
            player,
            &envelope.result.flow,
            &envelope.result.outcome,
            mode,
            best_time,
            minimum_time,
        )
        .map_err(|status| status)?;
        if result == crate::story_progress_bridge::OutcomeResult::Disabled {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    })
}

/// Submit the unified Story mission owner's typed progress candidate to the authenticated profile
/// path. The candidate must have been captured by the owner; this adapter derives neither outcome
/// nor phase/ordinal from guest memory and still relies on the bridge's admission checks.
pub fn native_story_mission_progress(
    player: usize,
    mission: &mut tsfp_gameplay::story_mission::StoryMission,
    mode: tsfp_engine_fileio::story_progress::StoryMode,
    best_time: f32,
    minimum_time: f32,
) -> Result<crate::story_progress_bridge::OutcomeResult, u32> {
    use tsfp_engine_fileio::vfs::status;
    let outcome = mission
        .progress_candidate()
        .cloned()
        .ok_or(status::INVALID_PARAMETER)?;
    let result = native_story_outcome(
        player,
        mission.flow(),
        &outcome,
        mode,
        best_time,
        minimum_time,
    )?;
    if matches!(
        &result,
        crate::story_progress_bridge::OutcomeResult::Committed { .. }
    ) {
        mission.clear_progress_candidate();
    }
    Ok(result)
}

/// Native entry adapters call this only after the runtime title key is derived.
/// Reinitialization would discard active profiles/editor state and is rejected.
pub fn activate_native(
    title_key: [u8; 16],
    progress: tsfp_engine_fileio::profile_defaults::ProgressDefaults,
    initializer: tsfp_engine_fileio::storage_lifecycle::ProfileInitializer,
) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    let storage = STORAGE.get().ok_or(status::INVALID_PARAMETER)?;
    let mut session = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    if session.is_some() {
        return Err(status::INVALID_PARAMETER);
    }
    let mut owned = crate::storage_session::StorageSession::new(storage, title_key)?;
    owned.progress_defaults = Some(progress);
    owned.profile_initializer = Some(initializer);
    *session = Some(owned);
    Ok(())
}

/// Direct typed commands for native menu/game consumers; no guest callback.
pub fn native_command(
    command: tsfp_engine_fileio::storage_frontend::Command,
    samples: &mut impl tsfp_engine_fileio::storage_flow::SampleMaps,
) -> Result<Option<tsfp_engine_fileio::storage_frontend::Completion>, u32> {
    use tsfp_engine_fileio::vfs::status;
    SESSION
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?
        .as_mut()
        .ok_or(status::INVALID_PARAMETER)?
        .command(command, samples)
}

/// Checked five-root dispatch for callers with Rust-owned profile/editor context.
/// This is an owned API, not admission of the old raw-pointer callback ABI.
pub fn native_entry(
    address: u32,
    target: tsfp_engine_fileio::storage_frontend::Target,
    action: crate::storage_session::EntryAction,
) -> Result<crate::storage_session::EntryResult, u32> {
    use tsfp_engine_fileio::vfs::status;
    let mut session = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let session = session.as_mut().ok_or(status::INVALID_PARAMETER)?;
    let result = if target == tsfp_engine_fileio::storage_frontend::Target::Map {
        crate::sample_maps::with(|samples| session.entry(address, target, action, samples))?
    } else {
        session.entry(address, target, action, &mut NoSamples)?
    };
    if matches!(result, crate::storage_session::EntryResult::Screen(_)) {
        WINDOW_OPEN.store(true, Ordering::Release);
    }
    Ok(result)
}

/// Owned menu attachment: the caller names its checked player/editor context.
/// Merely recognizing a legacy address does not authorize replacing its root.
pub fn native_open(
    root: tsfp_engine_fileio::storage_route::Root,
    target: tsfp_engine_fileio::storage_frontend::Target,
) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    let mut guard = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let session = guard.as_mut().ok_or(status::INVALID_PARAMETER)?;
    if root == tsfp_engine_fileio::storage_route::Root::ProfileTick {
        let tsfp_engine_fileio::storage_frontend::Target::Profile { player } = target else {
            return Err(status::INVALID_PARAMETER);
        };
        let result = session.begin_profile_selection(&[player], &mut NoSamples);
        // A failed catalogue read still owns a visible error and a Back action.
        if session.catalogue_view().is_some() {
            WINDOW_OPEN.store(true, Ordering::Release);
        }
        result?;
    } else if root == tsfp_engine_fileio::storage_route::Root::MapCatalogue {
        crate::sample_maps::with(|samples| session.open_root(root, target, samples))?;
    } else {
        session.open_root(root, target, &mut NoSamples)?;
    }
    WINDOW_OPEN.store(true, Ordering::Release);
    Ok(())
}

/// Native frontend navigation attachment: explicit owned retained context and
/// selected/cancelled destinations. Legacy context-stack pointers are rejected
/// by construction; opening failures retain the visible catalogue for retry.
pub fn native_select_profiles_from(
    players: &[usize],
    history: Vec<crate::storage_caller::RetainedPage>,
    selected: crate::storage_caller::NativePageId,
    cancelled: crate::storage_caller::NativePageId,
) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    let mut guard = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let session = guard.as_mut().ok_or(status::INVALID_PARAMETER)?;
    let result =
        session.begin_profile_selection_from(players, history, selected, cancelled, &mut NoSamples);
    if session.catalogue_view().is_some() {
        WINDOW_OPEN.store(true, Ordering::Release);
    }
    result
}

/// Native Players frontend calls this directly with its selected sources.
/// All keyboard/controller/mouse entry handlers share the same owned launch.
/// This does not admit an old page address or infer a guest context.
pub fn native_launch_players(
    participants: crate::storage_caller::Participants,
    history: Vec<crate::storage_caller::RetainedPage>,
    selected: crate::storage_caller::NativePageId,
    cancelled: crate::storage_caller::NativePageId,
) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    let mut guard = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let session = guard.as_mut().ok_or(status::INVALID_PARAMETER)?;
    let result =
        session.begin_players_launch(participants, history, selected, cancelled, &mut NoSamples);
    if session.catalogue_view().is_some() {
        WINDOW_OPEN.store(true, Ordering::Release);
    }
    result
}

/// One locked snapshot of completed participant routing and retained native
/// destination. Suppress it during a new selection and after profile removal;
/// a renderer/controller must never treat stale adoption as a ready launch.
pub fn native_players_destination() -> Option<(
    crate::storage_caller::RetainedPage,
    crate::storage_caller::Participants,
)> {
    let guard = SESSION.lock().ok()?;
    let session = guard.as_ref()?;
    session.caller.players_destination()
}

/// Destination input invokes a native handler with the current adopted profile.
/// Capture the checked owned context under the lock, then release it before
/// calling the handler so native settings/save handlers can reenter the session.
/// Callers supply their page identity; stale-page and unassigned-device input
/// is rejected without invoking any callback or synthesizing controller input.
pub fn native_dispatch_players_input<R>(
    page: crate::storage_caller::NativePageId,
    source: crate::storage_caller::PlayerInputSource,
    handler: impl FnOnce(
        &crate::storage_caller::RetainedPage,
        usize,
        &tsfp_engine_fileio::storage_lifecycle::PlayerProfile<tsfp_input::settings::InputSettings>,
    ) -> R,
) -> Result<R, u32> {
    use tsfp_engine_fileio::vfs::status;
    let (destination, player, profile) = {
        let guard = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
        let session = guard.as_ref().ok_or(status::INVALID_PARAMETER)?;
        session
            .caller
            .dispatch_players_input(page, source, |destination, player, profile| {
                (destination.clone(), player, profile.clone())
            })?
    };
    Ok(handler(&destination, player, &profile))
}

/// Install runtime rows and drive the concrete owned destination controller.
/// These APIs neither synthesize input nor admit a legacy frontend root.
pub fn native_install_destination(
    page: crate::storage_caller::NativePageId,
    rows: Vec<crate::storage_caller::DestinationRow>,
) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    SESSION
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?
        .as_mut()
        .ok_or(status::INVALID_PARAMETER)?
        .caller
        .install_destination(page, rows)
}
/// A native frontend binds rows and their direct Rust executor together. An
/// installed presentation without a handler continues to leave input unclaimed.
pub fn native_bind_destination(
    page: crate::storage_caller::NativePageId,
    rows: Vec<crate::storage_caller::DestinationRow>,
    handler: crate::storage_session::DestinationHandler,
) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    SESSION
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?
        .as_mut()
        .ok_or(status::INVALID_PARAMETER)?
        .bind_destination(page, rows, handler)
}
pub fn native_destination_input(
    page: crate::storage_caller::NativePageId,
    source: crate::storage_caller::PlayerInputSource,
    action: crate::storage_caller::DestinationAction,
) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    SESSION
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?
        .as_mut()
        .ok_or(status::INVALID_PARAMETER)?
        .destination_input(page, source, action)
}
pub fn destination_input_presentation()
-> Option<crate::storage_presentation::DestinationPresentation> {
    let guard = SESSION.lock().ok()?;
    let session = guard.as_ref()?;
    session
        .destination_input_active()
        .then(|| destination_for_session(session, 0))
        .flatten()
}
pub fn destination_input_pointer(
    presentation: &crate::storage_presentation::DestinationPresentation,
    source: crate::storage_caller::PlayerInputSource,
    size: (f32, f32),
    position: Option<(f32, f32)>,
    moved: bool,
    accept: bool,
    back: bool,
) -> Result<(), u32> {
    use crate::storage_caller::DestinationAction;
    use tsfp_engine_fileio::vfs::status;
    let mut guard = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let session = guard.as_mut().ok_or(status::INVALID_PARAMETER)?;
    let current = destination_for_session(session, 0)
        .ok_or(status::INVALID_PARAMETER)?
        .snapshot;
    let actions = presentation.pointer(&current, size, position, moved, accept, back);
    let focus = actions.iter().find_map(|action| match action {
        DestinationAction::Focus(id) => Some(*id),
        _ => None,
    });
    session.destination_pointer_input(
        &presentation.snapshot,
        source,
        focus,
        actions
            .iter()
            .any(|a| matches!(a, DestinationAction::Accept)),
        actions.iter().any(|a| matches!(a, DestinationAction::Back)),
    )
}

pub fn native_destination_view() -> Option<crate::storage_caller::DestinationView> {
    SESSION.lock().ok()?.as_ref()?.caller.destination_view()
}
pub fn native_destination_action(
    page: crate::storage_caller::NativePageId,
    source: crate::storage_caller::PlayerInputSource,
    action: crate::storage_caller::DestinationAction,
) -> Result<Option<crate::storage_caller::DestinationRequest>, u32> {
    use tsfp_engine_fileio::vfs::status;
    SESSION
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?
        .as_mut()
        .ok_or(status::INVALID_PARAMETER)?
        .caller
        .destination_action(page, source, action)
}

/// Qualify rendered geometry, participant identity and pointer dispatch under
/// one session lock. Return the owned request to the frontend activation consumer.
pub fn native_destination_pointer(
    presentation: &crate::storage_presentation::DestinationPresentation,
    source: crate::storage_caller::PlayerInputSource,
    size: (f32, f32),
    position: Option<(f32, f32)>,
    moved: bool,
    accept: bool,
    back: bool,
) -> Result<Option<crate::storage_caller::DestinationRequest>, u32> {
    use crate::storage_caller::DestinationAction;
    use tsfp_engine_fileio::vfs::status;
    let mut guard = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let session = guard.as_mut().ok_or(status::INVALID_PARAMETER)?;
    let current = destination_for_session(session, 0)
        .ok_or(status::INVALID_PARAMETER)?
        .snapshot;
    let actions = presentation.pointer(&current, size, position, moved, accept, back);
    let focus = actions.iter().find_map(|action| match action {
        DestinationAction::Focus(id) => Some(*id),
        _ => None,
    });
    session.caller.destination_pointer(
        &presentation.snapshot,
        source,
        focus,
        actions
            .iter()
            .any(|a| matches!(a, DestinationAction::Accept)),
        actions.iter().any(|a| matches!(a, DestinationAction::Back)),
    )
}

/// Read the actual owned destination after return consumption, including its
/// retained cursor/scroll. The frontend renders this state directly.
pub fn native_returned_page() -> Option<crate::storage_caller::RetainedPage> {
    SESSION
        .lock()
        .ok()?
        .as_ref()?
        .caller
        .navigation
        .active()
        .cloned()
}

/// A native frontend supplies its runtime identity/defaults before text capture.
pub fn native_begin_profile_name(
    directory: String,
    candidate: tsfp_format_profile::Profile,
) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    SESSION
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?
        .as_mut()
        .ok_or(status::INVALID_PARAMETER)?
        .begin_profile_name(directory, candidate)?;
    WINDOW_OPEN.store(true, Ordering::Release);
    Ok(())
}

/// Explicit native editor load/save-as caller, with retained owned return pages.
/// Errors retain the window context; completion is consumed through the typed
/// editor return below, never replayed into an original callback.
pub fn native_editor_load_from(
    history: Vec<crate::storage_caller::RetainedPage>,
    completed: crate::storage_caller::NativePageId,
    cancelled: crate::storage_caller::NativePageId,
) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    let mut guard = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let session = guard.as_mut().ok_or(status::INVALID_PARAMETER)?;
    let result = crate::sample_maps::with(|samples| {
        session.begin_editor_load_from(history, completed, cancelled, samples)
    });
    if session.catalogue_view().is_some() {
        WINDOW_OPEN.store(true, Ordering::Release);
    }
    result
}
/// Production Map Maker editor callback adapter. It admits only the registered
/// editor menu and copies its identity/cursor into Rust-owned retained state.
/// No guest page pointer or storage page state crosses this boundary.
pub fn native_editor_load_current() -> bool {
    let Some(current) = crate::ui_pages::current().filter(|page| page.live.builder == 0x31_3010)
    else {
        return false;
    };
    let id = crate::storage_caller::NativePageId(u64::from(current.live.builder));
    let cursor = current
        .cursor_nodes
        .iter()
        .position(|node| *node == current.live.cursor_node)
        .unwrap_or(0) as u32;
    let retained = crate::storage_caller::RetainedPage {
        id,
        cursor,
        scroll: 0.0,
    };
    let opened = {
        let Ok(mut guard) = SESSION.lock() else {
            eprintln!(
                "tsfp-bevy: native Map Maker Load unavailable (storage lock); using original handler"
            );
            return false;
        };
        let Some(session) = guard.as_mut() else {
            eprintln!(
                "tsfp-bevy: native Map Maker Load unavailable (storage session); using original handler"
            );
            return false;
        };
        match session.begin_editor_load_choice_from(vec![retained.clone()], id, id, &mut NoSamples)
        {
            Ok(()) => native_editor_load_claim(true, true, session.catalogue_view().is_some()),
            Err(error) => {
                eprintln!(
                    "tsfp-bevy: native Map Maker Load failed ({error:#010x}); using original handler"
                );
                native_editor_load_claim(true, false, false)
            }
        }
    };
    if opened {
        WINDOW_OPEN.store(true, Ordering::Release);
    }
    opened
}

fn native_editor_load_claim(session_available: bool, started: bool, catalogue_open: bool) -> bool {
    session_available && started && catalogue_open
}

fn native_editor_save_claim(session_available: bool, owns_document: bool) -> bool {
    session_available && owns_document
}

fn begin_mapmaker_storage_progress(
    ui: &mut tsfp_mapmaker::ui::MapMakerUiState,
    operation: tsfp_mapmaker::ui::StorageOperation,
) -> Option<tsfp_mapmaker::ui::StorageProgressId> {
    match ui.dispatch(tsfp_mapmaker::ui::MapMakerUiCommand::OpenStorageProgress(
        operation,
    )) {
        tsfp_mapmaker::ui::MapMakerUiUpdate::StorageProgressOpened(id) => Some(id),
        _ => None,
    }
}

fn finish_mapmaker_storage_progress(
    ui: &mut tsfp_mapmaker::ui::MapMakerUiState,
    progress: tsfp_mapmaker::ui::StorageProgressId,
    outcome: tsfp_mapmaker::ui::StorageProgressOutcome,
) -> bool {
    matches!(
        ui.dispatch(
            tsfp_mapmaker::ui::MapMakerUiCommand::CompleteStorageProgress { progress, outcome }
        ),
        tsfp_mapmaker::ui::MapMakerUiUpdate::StorageProgressCompleted(_)
    )
}

fn mapmaker_storage_progress_outcome<T, E>(
    result: &Result<T, E>,
) -> tsfp_mapmaker::ui::StorageProgressOutcome {
    if result.is_ok() {
        tsfp_mapmaker::ui::StorageProgressOutcome::Succeeded
    } else {
        tsfp_mapmaker::ui::StorageProgressOutcome::Failed
    }
}

/// Map Maker Save consumes only an adopted Rust document. Local documents update
/// their authenticated save directly; sample/new documents open the native
/// Save As name entry. Neither route enters the legacy device chooser.
pub fn native_editor_save_current() -> bool {
    use tsfp_engine_fileio::{storage_flow::MapSource, vfs::status};
    let Some(current) = crate::ui_pages::current().filter(|page| page.live.builder == 0x31_3010)
    else {
        return false;
    };
    let result = {
        let Ok(mut guard) = SESSION.lock() else {
            eprintln!(
                "tsfp-bevy: native Map Maker Save unavailable (storage lock); using original handler"
            );
            return false;
        };
        let Some(session) = guard.as_mut() else {
            eprintln!(
                "tsfp-bevy: native Map Maker Save unavailable (storage session); using original handler"
            );
            return false;
        };
        if !native_editor_save_claim(true, session.lifecycle.editor().is_some()) {
            eprintln!(
                "tsfp-bevy: native Map Maker Save unavailable (no owned map document); using original handler"
            );
            return false;
        }
        let page = crate::storage_caller::NativePageId(u64::from(current.live.builder));
        if session.has_editor_return() && !session.consume_editor_return_for(page) {
            Err(status::INVALID_PARAMETER)
        } else {
            match session.lifecycle.editor_source() {
                Some(MapSource::Local(_)) => {
                    let progress = mapmaker_ui().lock().ok().and_then(|mut ui| {
                        begin_mapmaker_storage_progress(
                            &mut ui,
                            tsfp_mapmaker::ui::StorageOperation::SaveMap,
                        )
                    });
                    let saved = session.save_editor(None, &mut NoSamples);
                    if let Some(progress) = progress
                        && let Ok(mut ui) = mapmaker_ui().lock()
                    {
                        finish_mapmaker_storage_progress(
                            &mut ui,
                            progress,
                            mapmaker_storage_progress_outcome(&saved),
                        );
                    }
                    saved.map(|_| false)
                }
                Some(MapSource::DiscSample(_)) | None => {
                    let directory = next_map_directory();
                    session.begin_map_name(directory).map(|_| true)
                }
            }
        }
    };
    match result {
        Ok(open_name) => {
            if open_name {
                WINDOW_OPEN.store(true, Ordering::Release);
            }
            true
        }
        Err(error) => {
            eprintln!("tsfp-bevy: native Map Maker Save failed ({error:#010x})");
            true
        }
    }
}

fn next_map_directory() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("map-{time:x}-{:x}", NEXT.fetch_add(1, Ordering::Relaxed))
}

pub fn native_editor_save_as_from(
    directory: String,
    history: Vec<crate::storage_caller::RetainedPage>,
    completed: crate::storage_caller::NativePageId,
    cancelled: crate::storage_caller::NativePageId,
) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    SESSION
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?
        .as_mut()
        .ok_or(status::INVALID_PARAMETER)?
        .begin_editor_save_as_from(directory, history, completed, cancelled)?;
    WINDOW_OPEN.store(true, Ordering::Release);
    Ok(())
}
pub fn native_take_editor_return() -> Option<crate::editor_navigation::EditorReturn> {
    SESSION.lock().ok()?.as_mut()?.take_editor_return()
}
pub fn native_editor_returned_page() -> Option<crate::storage_caller::RetainedPage> {
    SESSION
        .lock()
        .ok()?
        .as_ref()?
        .editor_returned_page()
        .cloned()
}

/// Native editor Save As opens direct owned text entry on the local backend.
/// Caller supplies identity independently of the editable display/map title.
pub fn native_begin_map_name(directory: String) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    SESSION
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?
        .as_mut()
        .ok_or(status::INVALID_PARAMETER)?
        .begin_map_name(directory)?;
    WINDOW_OPEN.store(true, Ordering::Release);
    Ok(())
}

/// Runtime-owned save directory and return pages cross the editor boundary as
/// typed values. The Rust editor surface authorizes the route; StorageSession
/// owns name entry, serialization, persistence, and completion.
pub struct MapMakerSaveAsContext {
    pub target: tsfp_mapmaker::ui::EditorSurfaceTarget,
    pub directory: String,
    pub history: Vec<crate::storage_caller::RetainedPage>,
    pub completed: crate::storage_caller::NativePageId,
    pub cancelled: crate::storage_caller::NativePageId,
}

fn is_current_mapmaker_save_as_route(
    ui: &tsfp_mapmaker::ui::MapMakerUiState,
    target: tsfp_mapmaker::ui::EditorSurfaceTarget,
) -> bool {
    use tsfp_mapmaker::ui::{EditorSurface, MapMakerPage};
    ui.page() == MapMakerPage::Editor
        && ui.editor_surface() == EditorSurface::SaveMap
        && target.surface() == EditorSurface::SaveMap
        && ui.active_save_map_target() == Some(target)
}

/// Start the native editor Save As transaction from the current Rust-owned
/// Save Map page. Stale page targets fail before StorageSession changes state.
pub fn native_mapmaker_save_as(context: MapMakerSaveAsContext) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    if !mapmaker_ui_enabled() {
        return Err(status::INVALID_PARAMETER);
    }
    let mut session = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let session = session.as_mut().ok_or(status::INVALID_PARAMETER)?;
    let ui = mapmaker_ui()
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?;
    if !is_current_mapmaker_save_as_route(&ui, context.target) {
        return Err(status::INVALID_PARAMETER);
    }
    drop(ui);
    session.begin_editor_save_as_from(
        context.directory,
        context.history,
        context.completed,
        context.cancelled,
    )?;
    WINDOW_OPEN.store(true, Ordering::Release);
    Ok(())
}

/// Only explicit native menu opens own the window. Transaction diagnostics and
/// background profile edits must never capture input or overlay the frontend.
pub fn window_active() -> bool {
    if destination_input_presentation().is_some() {
        return true;
    }
    if !WINDOW_OPEN.load(Ordering::Acquire) {
        return false;
    }
    if native_catalogue().is_some()
        || SESSION
            .lock()
            .ok()
            .and_then(|s| s.as_ref().and_then(|s| s.profile_return_view()))
            .is_some()
    {
        return true;
    }
    WINDOW_OPEN.store(false, Ordering::Release);
    false
}

pub fn window_presentation() -> Option<crate::storage_presentation::CataloguePresentation> {
    window_active()
        .then(|| native_catalogue_presentation(0))
        .flatten()
}

/// Frame renderer receives the authenticated owned destination after catalogue
/// return. One lock qualifies the page, participants and durable profile keys.
/// This drawing attachment does not acquire catalogue input ownership.
pub fn destination_presentation(
    font: u32,
) -> Option<crate::storage_presentation::DestinationPresentation> {
    let guard = SESSION.lock().ok()?;
    let session = guard.as_ref()?;
    destination_for_session(session, font)
}

fn destination_for_session(
    session: &crate::storage_session::StorageSession,
    font: u32,
) -> Option<crate::storage_presentation::DestinationPresentation> {
    // Selection and return preparation always own the foreground. Never draw a
    // previous destination beneath a new participant selection or a retry.
    if session.catalogue_view().is_some() || session.profile_return_view().is_some() {
        return None;
    }
    Some(crate::storage_presentation::DestinationPresentation::new(
        session.caller.destination_view()?,
        font,
    ))
}

/// Renderers receive owned rows/errors, never pointers into the old page.
pub fn native_screen() -> Option<tsfp_engine_fileio::storage_frontend::Screen> {
    Some(SESSION.lock().ok()?.as_ref()?.frontend.screen().clone())
}

/// Frontend render caller obtains one checked target with catalogue rows,
/// focus and status. There is deliberately no device-selection presentation.
pub fn native_catalogue() -> Option<crate::storage_caller::CatalogueView> {
    SESSION.lock().ok()?.as_ref()?.catalogue_view()
}

/// Renderer adapter for an explicitly opened owned catalogue. This does not
/// infer a target from a legacy page or change the default frontend route.
pub fn native_catalogue_presentation(
    font: u32,
) -> Option<crate::storage_presentation::CataloguePresentation> {
    let guard = SESSION.lock().ok()?;
    let session = guard.as_ref()?;
    if let Some(view) = session.profile_return_view() {
        return Some(
            crate::storage_presentation::CataloguePresentation::profile_return(view, font),
        );
    }
    let view = session.catalogue_view()?;
    let mut presentation = if let Some(name) = session.caller.profile_name.as_ref() {
        crate::storage_presentation::CataloguePresentation::profile_name(
            view,
            font,
            name.text().into(),
        )
    } else if let Some(name) = session.caller.map_name.as_ref() {
        crate::storage_presentation::CataloguePresentation::map_name(view, font, name.text().into())
    } else {
        crate::storage_presentation::CataloguePresentation::new(view, font)
    };
    presentation.name_identity = session.caller.name_identity().map(str::to_owned);
    if mapmaker_ui_enabled()
        && presentation.snapshot.target == tsfp_engine_fileio::storage_frontend::Target::Map
        && presentation.snapshot.error.is_none()
        && presentation.name.is_none()
        && !presentation.returning
        && let Ok(mut ui) = mapmaker_ui().lock()
    {
        if presentation.snapshot.map_source_choice {
            use tsfp_mapmaker::ui::{
                MapMakerPage, MapMakerUiCommand, MapMakerUiState, MapStorageSource,
            };

            if ui.page() != MapMakerPage::StorageSource {
                *ui = MapMakerUiState::default();
                ui.dispatch(MapMakerUiCommand::OpenStorage);
            }
            if let Some(source) = presentation.snapshot.cursor.and_then(|index| match index {
                0 => Some(MapStorageSource::SampleMaps),
                1 => Some(MapStorageSource::LocalStorage),
                _ => None,
            }) {
                ui.dispatch(MapMakerUiCommand::FocusStorageSource(source));
            }
            let _ = presentation.project_owned_storage_source(&ui.snapshot());
        } else {
            // Catalogue input and authenticated file loading keep their typed
            // StorageSession boundary; only the row page is projected here.
            let _ = presentation.project_owned_map_list(&ui.snapshot());
        }
    }
    if DIAGNOSTIC_WINDOW.get().is_some()
        && std::env::var_os("TSFP_UI_DRAW_LOG").is_some()
        && let Some(name) = presentation.name.as_ref()
    {
        static LAST_LENGTH: AtomicU64 = AtomicU64::new(u64::MAX);
        if LAST_LENGTH.swap(name.len() as u64, Ordering::Relaxed) != name.len() as u64 {
            eprintln!(
                "tsfp-bevy: native storage diagnostic: name presentation {} bytes",
                name.len()
            );
        }
    }
    Some(presentation)
}

/// Apply a click atomically to the same catalogue that was rendered. Stale
/// snapshots fail before changing focus or loading any profile/editor record.
pub fn native_catalogue_pointer(
    presentation: &crate::storage_presentation::CataloguePresentation,
    size: (f32, f32),
    position: Option<(f32, f32)>,
    moved: bool,
    accept: bool,
    back: bool,
) -> Result<Option<tsfp_engine_fileio::storage_frontend::Completion>, u32> {
    use tsfp_engine_fileio::vfs::status;
    let mut guard = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let session = guard.as_mut().ok_or(status::INVALID_PARAMETER)?;
    let returning = session.profile_return_view();
    if presentation.returning != returning.is_some() {
        return Err(status::INVALID_PARAMETER);
    }
    let current = returning
        .or_else(|| session.catalogue_view())
        .ok_or(status::INVALID_PARAMETER)?;
    if !presentation.matches(&current)
        || presentation.name.as_deref() != session.caller.name_text()
        || presentation.name_identity.as_deref() != session.caller.name_identity()
    {
        return Err(status::INVALID_PARAMETER);
    }
    let actions = presentation.pointer(size, position, moved, accept, back);
    let mut mapmaker = if current.target == tsfp_engine_fileio::storage_frontend::Target::Map
        && mapmaker_ui_enabled()
    {
        Some(
            mapmaker_ui()
                .lock()
                .map_err(|_| status::INVALID_PARAMETER)?,
        )
    } else {
        None
    };
    if native_actions_need_samples(&current, &actions) {
        crate::sample_maps::with(|samples| {
            let mut receipt = None;
            for action in actions {
                if current.target == tsfp_engine_fileio::storage_frontend::Target::Map {
                    eprintln!(
                        "tsfp-bevy: native Map Maker storage pointer action source-choice={} {action:?}",
                        current.map_source_choice
                    );
                }
                receipt = mapmaker_aware_catalogue_action(
                    session,
                    action,
                    samples,
                    mapmaker.as_deref_mut(),
                )?;
            }
            Ok(receipt)
        })
    } else {
        let mut receipt = None;
        for action in actions {
            if current.target == tsfp_engine_fileio::storage_frontend::Target::Map {
                eprintln!(
                    "tsfp-bevy: native Map Maker storage pointer action source-choice={} {action:?}",
                    current.map_source_choice
                );
            }
            receipt = mapmaker_aware_catalogue_action(
                session,
                action,
                &mut NoSamples,
                mapmaker.as_deref_mut(),
            )?;
        }
        Ok(receipt)
    }
}

/// Native text owns character/backspace events through release, even after closing.
/// Accept/Back remain shared keyboard/controller/pointer intents.
pub fn name_keyboard(event: &bevy::input::keyboard::KeyboardInput) -> bool {
    use bevy::input::{
        ButtonState,
        keyboard::{Key, KeyCode},
    };
    static HELD: std::sync::LazyLock<Mutex<std::collections::HashSet<KeyCode>>> =
        std::sync::LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));
    let Ok(mut held) = HELD.lock() else {
        return false;
    };
    if event.state == ButtonState::Released {
        return held.remove(&event.key_code);
    }
    if !WINDOW_OPEN.load(Ordering::Acquire) {
        return false;
    }
    let Ok(mut guard) = SESSION.lock() else {
        return false;
    };
    let Some(caller) = guard
        .as_mut()
        .map(|session| &mut session.caller)
        .filter(|caller| caller.name_text().is_some())
    else {
        return false;
    };
    let result = match &event.logical_key {
        Key::Character(text) => caller.append_name(text),
        Key::Space => caller.append_name(" "),
        Key::Backspace => {
            caller.backspace_name();
            Ok(())
        }
        _ => return false,
    };
    held.insert(event.key_code);
    if let Err(status) = result {
        eprintln!("tsfp-bevy: native save text error {status:#010x}");
    }
    true
}

/// Controller, keyboard and pointer use the same direct owned handler.
pub fn native_catalogue_action(
    action: crate::storage_caller::CatalogueAction,
) -> Result<Option<tsfp_engine_fileio::storage_frontend::Completion>, u32> {
    use tsfp_engine_fileio::vfs::status;
    let mut guard = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let session = guard.as_mut().ok_or(status::INVALID_PARAMETER)?;
    let view = session.catalogue_view().ok_or(status::INVALID_PARAMETER)?;
    if view.target == tsfp_engine_fileio::storage_frontend::Target::Map {
        eprintln!(
            "tsfp-bevy: native Map Maker storage action source-choice={} {action:?}",
            view.map_source_choice
        );
    }

    let selected_map_source = if view.target == tsfp_engine_fileio::storage_frontend::Target::Map
        && mapmaker_ui_enabled()
        && view.map_source_choice
        && matches!(action, crate::storage_caller::CatalogueAction::Accept)
    {
        view.cursor.and_then(|index| match index {
            0 => Some(tsfp_mapmaker::ui::MapStorageSource::SampleMaps),
            1 => Some(tsfp_mapmaker::ui::MapStorageSource::LocalStorage),
            _ => None,
        })
    } else {
        None
    };
    if view.target == tsfp_engine_fileio::storage_frontend::Target::Map && mapmaker_ui_enabled() {
        let mut ui = mapmaker_ui()
            .lock()
            .map_err(|_| status::INVALID_PARAMETER)?;
        if view.map_source_choice {
            if ui.page() != tsfp_mapmaker::ui::MapMakerPage::StorageSource {
                *ui = tsfp_mapmaker::ui::MapMakerUiState::default();
                ui.dispatch(tsfp_mapmaker::ui::MapMakerUiCommand::OpenStorage);
            }
            if let Some(source) = view.cursor.and_then(|index| match index {
                0 => Some(tsfp_mapmaker::ui::MapStorageSource::SampleMaps),
                1 => Some(tsfp_mapmaker::ui::MapStorageSource::LocalStorage),
                _ => None,
            }) {
                ui.dispatch(tsfp_mapmaker::ui::MapMakerUiCommand::FocusStorageSource(
                    source,
                ));
            }
            tsfp_ui_input::mapmaker_storage::dispatch_action(&mut ui, action);
        } else {
            let entering_from_editor = matches!(
                ui.page(),
                tsfp_mapmaker::ui::MapMakerPage::EditorMenu
                    | tsfp_mapmaker::ui::MapMakerPage::Editor
            );
            let source = if entering_from_editor {
                tsfp_mapmaker::ui::MapStorageSource::LocalStorage
            } else {
                ui.focused_storage_source()
                    .unwrap_or(tsfp_mapmaker::ui::MapStorageSource::LocalStorage)
            };
            let sample_source = source == tsfp_mapmaker::ui::MapStorageSource::SampleMaps;
            let row_count = if sample_source {
                view.sample_maps.len()
            } else {
                view.rows.len()
            };
            if matches!(
                ui.page(),
                tsfp_mapmaker::ui::MapMakerPage::EditorMenu
                    | tsfp_mapmaker::ui::MapMakerPage::Editor
            ) {
                *ui = tsfp_mapmaker::ui::MapMakerUiState::default();
                ui.dispatch(tsfp_mapmaker::ui::MapMakerUiCommand::OpenStorage);
                ui.dispatch(tsfp_mapmaker::ui::MapMakerUiCommand::FocusStorageSource(
                    source,
                ));
            }
            if matches!(
                ui.page(),
                tsfp_mapmaker::ui::MapMakerPage::StorageSource
                    | tsfp_mapmaker::ui::MapMakerPage::MapList
            ) {
                install_mapmaker_rows(&mut ui, row_count, view.cursor, source);
            }
            if ui.page() == tsfp_mapmaker::ui::MapMakerPage::MapList {
                let focused_row = view
                    .cursor
                    .and_then(|index| ui.map_rows().get(index).map(|row| row.id));
                if let Some(row_id) = focused_row {
                    ui.dispatch(tsfp_mapmaker::ui::MapMakerUiCommand::FocusMap(row_id));
                }
                let updates = tsfp_ui_input::mapmaker_storage::dispatch_action(&mut ui, action);
                let requested_load = updates.iter().find_map(|update| match update {
                    tsfp_mapmaker::ui::MapMakerUiUpdate::LoadRequested(request) => Some(*request),
                    _ => None,
                });
                let result = if native_actions_need_samples(&view, std::slice::from_ref(&action)) {
                    crate::sample_maps::with(|samples| session.catalogue_action(action, samples))
                } else {
                    session.catalogue_action(action, &mut NoSamples)
                };
                if let Some(request) = requested_load {
                    let outcome = match &result {
                        Ok(Some(_)) => Some(tsfp_mapmaker::ui::MapLoadOutcome::Loaded),
                        Err(_) => Some(tsfp_mapmaker::ui::MapLoadOutcome::Failed),
                        Ok(None) => None,
                    };
                    if let Some(outcome) = outcome {
                        ui.dispatch(tsfp_mapmaker::ui::MapMakerUiCommand::CompleteMapLoad {
                            request: request.id,
                            outcome,
                        });
                    }
                }
                if matches!(action, crate::storage_caller::CatalogueAction::Back)
                    && session.catalogue_view().is_none()
                {
                    *ui = tsfp_mapmaker::ui::MapMakerUiState::default();
                }
                return result;
            }
        }
    }

    let result = if native_actions_need_samples(&view, std::slice::from_ref(&action)) {
        crate::sample_maps::with(|samples| session.catalogue_action(action, samples))
    } else {
        session.catalogue_action(action, &mut NoSamples)
    };
    if let Some(source) = selected_map_source {
        if let Some(list_view) = session.catalogue_view()
            && !list_view.map_source_choice
            && list_view.target == tsfp_engine_fileio::storage_frontend::Target::Map
            && let Ok(mut ui) = mapmaker_ui().lock()
        {
            let row_count = match source {
                tsfp_mapmaker::ui::MapStorageSource::SampleMaps => list_view.sample_maps.len(),
                tsfp_mapmaker::ui::MapStorageSource::LocalStorage => list_view.rows.len(),
            };
            install_mapmaker_rows(&mut ui, row_count, list_view.cursor, source);
        }
    }
    result
}

/// Load the title provider only when the selected action reads sample rows or
/// the sample payload. Local saves must remain usable if the disc provider has
/// no configured path or cannot open the selected image.
fn native_actions_need_samples(
    view: &crate::storage_caller::CatalogueView,
    actions: &[crate::storage_caller::CatalogueAction],
) -> bool {
    use crate::storage_caller::CatalogueAction;
    if !matches!(
        view.target,
        tsfp_engine_fileio::storage_frontend::Target::Map
    ) {
        return false;
    }
    let mut cursor = view.cursor.unwrap_or(0) as i64;
    let mut source_choice = view.map_source_choice;
    for action in actions {
        match action {
            CatalogueAction::Step(delta) => {
                let count = if source_choice {
                    2
                } else {
                    view.rows.len() + view.sample_maps.len()
                };
                if count > 0 {
                    cursor = (cursor + i64::from(*delta)).rem_euclid(count as i64);
                }
            }
            CatalogueAction::Focus(index) => cursor = *index as i64,
            CatalogueAction::Accept if source_choice => {
                if cursor == 0 {
                    return true;
                }
                source_choice = false;
                cursor = 0;
            }
            CatalogueAction::Accept => {
                return cursor >= view.rows.len() as i64
                    && cursor < (view.rows.len() + view.sample_maps.len()) as i64;
            }
            CatalogueAction::Back => return false,
        }
    }
    false
}
pub fn native_focus(index: usize) -> Result<(), u32> {
    native_catalogue_action(crate::storage_caller::CatalogueAction::Focus(index)).map(|_| ())
}
pub fn native_load_selection() -> Result<tsfp_engine_fileio::storage_frontend::Completion, u32> {
    native_catalogue_action(crate::storage_caller::CatalogueAction::Accept)?
        .ok_or(tsfp_engine_fileio::vfs::status::INVALID_PARAMETER)
}

pub fn native_take_completion() -> Option<tsfp_engine_fileio::storage_frontend::Completion> {
    SESSION.lock().ok()?.as_mut()?.route.take_completion()
}

/// The owned frontend caller consumes this one-shot navigation result after
/// all participating profiles are adopted, or after explicit cancellation.
pub fn native_take_profile_return() -> Option<crate::storage_caller::ProfileSelectionReturn> {
    SESSION.lock().ok()?.as_mut()?.take_profile_return()
}

/// Game consumers obtain an owned profile snapshot rather than a guest slot.
pub fn native_profile(
    player: usize,
) -> Option<tsfp_engine_fileio::storage_lifecycle::PlayerProfile<tsfp_input::settings::InputSettings>>
{
    SESSION
        .lock()
        .ok()?
        .as_ref()?
        .caller
        .players
        .get(player)?
        .clone()
}

/// Persist an accepted native settings edit through the active profile's signer and VFS path.
/// The storage session publishes the new owned profile only after the authenticated write passes.
pub fn native_save_profile_settings(
    player: usize,
    candidate: tsfp_format_profile::Profile,
) -> Result<(), u32> {
    use tsfp_engine_fileio::{storage_frontend::Command, vfs::status};
    let mut guard = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let session = guard.as_mut().ok_or(status::INVALID_PARAMETER)?;
    session
        .command(Command::SaveProfile { player, candidate }, &mut NoSamples)
        .map_err(|status| status)?
        .ok_or(status::INVALID_PARAMETER)?;
    Ok(())
}

pub fn native_profile_defaults() -> Option<tsfp_engine_fileio::profile_defaults::ProgressDefaults> {
    SESSION.lock().ok()?.as_ref()?.progress_defaults.clone()
}

pub fn native_deactivate(player: usize) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    SESSION
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?
        .as_mut()
        .ok_or(status::INVALID_PARAMETER)?
        .deactivate(player)
}

/// Editor handlers submit their owned document directly, without serializing a
/// guest arena. Existing lifted editor handlers cannot call this by pointer.
pub fn native_editor_edit(
    document: tsfp_engine_fileio::map_document::MapDocument,
) -> Result<(), u32> {
    use tsfp_engine_fileio::vfs::status;
    SESSION
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?
        .as_mut()
        .ok_or(status::INVALID_PARAMETER)?
        .lifecycle
        .edit(document);
    Ok(())
}

pub fn native_editor() -> Option<tsfp_engine_fileio::map_document::MapDocument> {
    SESSION.lock().ok()?.as_ref()?.lifecycle.editor().cloned()
}

/// Enter the Rust-owned close-prompt state for the active native document.
/// The original callback/page producer is not yet mapped, so callers must
/// supply that boundary explicitly; this function never synthesizes input.
pub fn native_begin_mapmaker_document_close_prompt() -> bool {
    if !mapmaker_ui_enabled() {
        return false;
    }
    let Ok(mut session) = SESSION.lock() else {
        return false;
    };
    let Some(session) = session.as_mut() else {
        return false;
    };
    let Some(token) = session.mapmaker_document_close_token() else {
        return false;
    };
    let Ok(mut ui) = mapmaker_ui().lock() else {
        return false;
    };
    ui.open_document_close_prompt(token)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeMapDescriptionOutcome {
    Changed,
    Unchanged,
    Committed { document_changed: bool },
    Cancelled,
}

fn current_mapmaker_description_route(
    ui: &tsfp_mapmaker::ui::MapMakerUiState,
    target: tsfp_mapmaker::ui::EditorSurfaceTarget,
) -> bool {
    target.surface() == tsfp_mapmaker::ui::EditorSurface::MapDescription
        && ui.active_map_description_target() == Some(target)
}

fn current_mapmaker_briefing_route(
    ui: &tsfp_mapmaker::ui::MapMakerUiState,
    target: tsfp_mapmaker::ui::EditorSurfaceTarget,
) -> bool {
    target.surface() == tsfp_mapmaker::ui::EditorSurface::Briefing
        && ui.active_briefing_target() == Some(target)
}

/// Snapshot the Rust-owned text and caret for a native renderer. The returned
/// page and field generations must be supplied again when dispatching input.
pub fn native_mapmaker_description_view() -> Option<tsfp_mapmaker::ui::MapDescriptionPageView> {
    if !mapmaker_ui_enabled() {
        return None;
    }
    mapmaker_ui().lock().ok()?.map_description_view()
}

/// Snapshot the Rust-owned Briefing text for a native renderer. Runtime must
/// supply the live field contents when opening it; this boundary does not map
/// Briefing text to the map description or commit it to storage.
pub fn native_mapmaker_briefing_text_view() -> Option<tsfp_mapmaker::ui::BriefingTextView> {
    if !mapmaker_ui_enabled() {
        return None;
    }
    mapmaker_ui().lock().ok()?.briefing_text_view()
}

/// Open a Rust-owned Briefing text session from caller-supplied field data.
/// The widget field, byte limit, and persistence contract remain caller-owned.
pub fn native_begin_mapmaker_briefing_text(
    target: tsfp_mapmaker::ui::EditorSurfaceTarget,
    initial: &str,
    byte_limit: usize,
) -> Result<tsfp_mapmaker::ui::BriefingTextTarget, u32> {
    use tsfp_engine_fileio::vfs::status;
    if !mapmaker_ui_enabled() {
        return Err(status::INVALID_PARAMETER);
    }
    let mut ui = mapmaker_ui()
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?;
    let field_target = begin_mapmaker_briefing_text_session(&mut ui, target, initial, byte_limit)?;
    if let Ok(mut pending) = MAPMAKER_BRIEFING_TEXT_OUTCOMES.lock() {
        pending.clear();
    }
    Ok(field_target)
}

fn begin_mapmaker_briefing_text_session(
    ui: &mut tsfp_mapmaker::ui::MapMakerUiState,
    target: tsfp_mapmaker::ui::EditorSurfaceTarget,
    initial: &str,
    byte_limit: usize,
) -> Result<tsfp_mapmaker::ui::BriefingTextTarget, u32> {
    use tsfp_engine_fileio::vfs::status;
    if !current_mapmaker_briefing_route(ui, target) {
        return Err(status::INVALID_PARAMETER);
    }
    ui.install_briefing_text_session(target, initial, byte_limit)
        .map_err(|_| status::INVALID_PARAMETER)
}

/// Apply one generation-bound Briefing command and return its typed outcome to
/// the caller. Even CommitRequested stays in Rust-owned state until its
/// caller-supplied document handler accepts it.
pub fn native_mapmaker_briefing_text_command(
    surface: tsfp_mapmaker::ui::EditorSurfaceTarget,
    target: tsfp_mapmaker::ui::BriefingTextTarget,
    command: tsfp_mapmaker::description::DescriptionCommand,
) -> Result<tsfp_mapmaker::ui::BriefingTextOutcome, u32> {
    use tsfp_engine_fileio::vfs::status;
    if !mapmaker_ui_enabled() {
        return Err(status::INVALID_PARAMETER);
    }
    let mut ui = mapmaker_ui()
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?;
    apply_mapmaker_briefing_text_command(&mut ui, surface, target, command)
}

/// Retrieve the next terminal result produced by native physical-key input.
/// The caller must validate the included page generation before applying it.
pub fn native_take_mapmaker_briefing_text_outcome() -> Option<(
    tsfp_mapmaker::ui::EditorSurfaceTarget,
    tsfp_mapmaker::ui::BriefingTextOutcome,
)> {
    MAPMAKER_BRIEFING_TEXT_OUTCOMES.lock().ok()?.pop_front()
}

fn queue_native_mapmaker_briefing_text_outcome(
    surface: tsfp_mapmaker::ui::EditorSurfaceTarget,
    outcome: tsfp_mapmaker::ui::BriefingTextOutcome,
) {
    if !matches!(
        outcome,
        tsfp_mapmaker::ui::BriefingTextOutcome::CommitRequested(_)
            | tsfp_mapmaker::ui::BriefingTextOutcome::Cancelled(_)
    ) {
        return;
    }
    if let Ok(mut pending) = MAPMAKER_BRIEFING_TEXT_OUTCOMES.lock() {
        pending.push_back((surface, outcome));
    }
}

fn apply_mapmaker_briefing_text_command(
    ui: &mut tsfp_mapmaker::ui::MapMakerUiState,
    surface: tsfp_mapmaker::ui::EditorSurfaceTarget,
    target: tsfp_mapmaker::ui::BriefingTextTarget,
    command: tsfp_mapmaker::description::DescriptionCommand,
) -> Result<tsfp_mapmaker::ui::BriefingTextOutcome, u32> {
    use tsfp_engine_fileio::vfs::status;
    if !current_mapmaker_briefing_route(ui, surface) {
        return Err(status::INVALID_PARAMETER);
    }
    ui.apply_briefing_text_command(surface, target, command)
        .map_err(|_| status::INVALID_PARAMETER)
}

/// Start the Rust-owned description session from the currently adopted map.
/// Runtime supplies the original widget's byte limit; the document codec
/// retains its separate storage capacity.
pub fn native_begin_mapmaker_description(
    target: tsfp_mapmaker::ui::EditorSurfaceTarget,
    byte_limit: usize,
) -> Result<tsfp_mapmaker::description::DescriptionTarget, u32> {
    use tsfp_engine_fileio::vfs::status;
    if !mapmaker_ui_enabled() {
        return Err(status::INVALID_PARAMETER);
    }
    let session = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let session = session.as_ref().ok_or(status::INVALID_PARAMETER)?;
    let mut ui = mapmaker_ui()
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?;
    begin_mapmaker_description_session(&mut ui, &session.lifecycle, target, byte_limit)
}

fn begin_mapmaker_description_session(
    ui: &mut tsfp_mapmaker::ui::MapMakerUiState,
    lifecycle: &tsfp_engine_fileio::storage_lifecycle::NativeStorageLifecycle,
    target: tsfp_mapmaker::ui::EditorSurfaceTarget,
    byte_limit: usize,
) -> Result<tsfp_mapmaker::description::DescriptionTarget, u32> {
    use tsfp_engine_fileio::vfs::status;
    if !current_mapmaker_description_route(ui, target) {
        return Err(status::INVALID_PARAMETER);
    }
    let document = lifecycle.editor().ok_or(status::INVALID_PARAMETER)?;
    let initial = core::str::from_utf8(document.description_bytes())
        .map_err(|_| status::INVALID_PARAMETER)?;
    ui.install_map_description_session(target, initial, byte_limit)
        .map_err(|_| status::INVALID_PARAMETER)
}

/// Apply one native text command and commit Enter directly into the owned map
/// history. Both page and text generations must still be current.
pub fn native_mapmaker_description_command(
    surface: tsfp_mapmaker::ui::EditorSurfaceTarget,
    target: tsfp_mapmaker::description::DescriptionTarget,
    command: tsfp_mapmaker::description::DescriptionCommand,
) -> Result<NativeMapDescriptionOutcome, u32> {
    use tsfp_engine_fileio::vfs::status;
    if !mapmaker_ui_enabled() {
        return Err(status::INVALID_PARAMETER);
    }
    let mut storage = SESSION.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let storage = storage.as_mut().ok_or(status::INVALID_PARAMETER)?;
    let mut ui = mapmaker_ui()
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?;
    apply_mapmaker_description_command(&mut ui, &mut storage.lifecycle, surface, target, command)
}

/// Route physical keyboard events to the active Rust-owned Map Maker text session.
/// Key-up is swallowed only for keys accepted on key-down so the legacy key path
/// cannot receive half of a native text gesture.
pub fn native_mapmaker_text_keyboard(event: &bevy::input::keyboard::KeyboardInput) -> bool {
    use bevy::input::ButtonState;

    if event.state == ButtonState::Released {
        return MAPMAKER_TEXT_KEYS
            .lock()
            .is_ok_and(|mut keys| keys.remove(&event.key_code));
    }
    if !crate::mapmaker_description_live::owns_text_input() {
        return false;
    }
    if let Some((surface, target)) = active_mapmaker_briefing_text_session() {
        let Some(commands) = mapmaker_briefing_commands_for_key(event, target) else {
            return false;
        };
        if let Ok(mut keys) = MAPMAKER_TEXT_KEYS.lock() {
            keys.insert(event.key_code);
        }
        for command in commands {
            if let Ok(outcome) = native_mapmaker_briefing_text_command(surface, target, command) {
                queue_native_mapmaker_briefing_text_outcome(surface, outcome);
            }
        }
        return true;
    }
    let Some((surface, target)) = active_mapmaker_description_session() else {
        return false;
    };
    let Some(commands) = mapmaker_description_commands_for_key(event, target) else {
        return false;
    };
    if let Ok(mut keys) = MAPMAKER_TEXT_KEYS.lock() {
        keys.insert(event.key_code);
    }
    for command in commands {
        // The page and text generations are checked again at the mutation boundary.
        // A stale native event remains consumed and cannot fall through into guest UI.
        if matches!(
            native_mapmaker_description_command(surface, target, command),
            Ok(NativeMapDescriptionOutcome::Committed { .. }
                | NativeMapDescriptionOutcome::Cancelled)
        ) {
            crate::text_entry_live::queue_native_back();
        }
    }
    true
}

fn active_mapmaker_description_session() -> Option<(
    tsfp_mapmaker::ui::EditorSurfaceTarget,
    tsfp_mapmaker::description::DescriptionTarget,
)> {
    if !mapmaker_ui_enabled() {
        return None;
    }
    let ui = mapmaker_ui().lock().ok()?;
    let surface = ui.active_map_description_target()?;
    let target = ui.map_description_session()?.target();
    Some((surface, target))
}

fn active_mapmaker_briefing_text_session() -> Option<(
    tsfp_mapmaker::ui::EditorSurfaceTarget,
    tsfp_mapmaker::ui::BriefingTextTarget,
)> {
    if !mapmaker_ui_enabled() {
        return None;
    }
    let ui = mapmaker_ui().lock().ok()?;
    let surface = ui.active_briefing_target()?;
    let view = ui.briefing_text_view()?;
    (view.surface == surface).then_some((surface, view.target))
}

#[derive(Debug)]
struct NativeTextKeyAction {
    chars: Vec<char>,
    delete: bool,
    held: tsfp_ui_input::IntentSet,
}

fn native_text_key_action(
    event: &bevy::input::keyboard::KeyboardInput,
) -> Option<NativeTextKeyAction> {
    use bevy::input::keyboard::{Key, KeyCode};
    use tsfp_ui_input::{IntentSet, MenuIntent};

    let mut chars = Vec::new();
    let mut held = IntentSet::default();
    let mut delete = false;
    match &event.logical_key {
        Key::Character(text) => {
            chars.extend(text.chars().filter(|ch| !ch.is_control()));
        }
        Key::Space => chars.push(' '),
        Key::Backspace => held = held.with(MenuIntent::Backspace),
        _ => match event.key_code {
            KeyCode::ArrowLeft => held = held.with(MenuIntent::Left),
            KeyCode::ArrowRight => held = held.with(MenuIntent::Right),
            KeyCode::Delete => delete = true,
            KeyCode::Enter | KeyCode::NumpadEnter => held = held.with(MenuIntent::Accept),
            KeyCode::Escape => held = held.with(MenuIntent::Back),
            _ => return None,
        },
    };
    Some(NativeTextKeyAction {
        chars,
        delete,
        held,
    })
}

fn mapmaker_description_commands_for_key(
    event: &bevy::input::keyboard::KeyboardInput,
    target: tsfp_mapmaker::description::DescriptionTarget,
) -> Option<Vec<tsfp_mapmaker::description::DescriptionCommand>> {
    let action = native_text_key_action(event)?;
    let commands = tsfp_ui_input::mapmaker_description::commands_with_delete(
        target,
        &action.chars,
        action.delete,
        action.held,
        tsfp_ui_input::IntentSet::default(),
    )
    .into_iter()
    .map(|(_, command)| command)
    .collect::<Vec<_>>();
    (!commands.is_empty()).then_some(commands)
}

fn mapmaker_briefing_commands_for_key(
    event: &bevy::input::keyboard::KeyboardInput,
    target: tsfp_mapmaker::ui::BriefingTextTarget,
) -> Option<Vec<tsfp_mapmaker::description::DescriptionCommand>> {
    let action = native_text_key_action(event)?;
    let commands = tsfp_ui_input::mapmaker_description::briefing_text_commands_with_delete(
        target,
        &action.chars,
        action.delete,
        action.held,
        tsfp_ui_input::IntentSet::default(),
    )
    .into_iter()
    .map(|(_, command)| command)
    .collect::<Vec<_>>();
    (!commands.is_empty()).then_some(commands)
}

fn apply_mapmaker_description_command(
    ui: &mut tsfp_mapmaker::ui::MapMakerUiState,
    lifecycle: &mut tsfp_engine_fileio::storage_lifecycle::NativeStorageLifecycle,
    surface: tsfp_mapmaker::ui::EditorSurfaceTarget,
    target: tsfp_mapmaker::description::DescriptionTarget,
    command: tsfp_mapmaker::description::DescriptionCommand,
) -> Result<NativeMapDescriptionOutcome, u32> {
    use tsfp_engine_fileio::vfs::status;
    use tsfp_mapmaker::description::DescriptionOutcome;
    use tsfp_mapmaker::ui::MapMakerUiCommand;

    if !current_mapmaker_description_route(ui, surface) {
        return Err(status::INVALID_PARAMETER);
    }
    let outcome = ui
        .apply_map_description_command(surface, target, command)
        .map_err(|_| status::INVALID_PARAMETER)?;
    match outcome {
        DescriptionOutcome::Changed => Ok(NativeMapDescriptionOutcome::Changed),
        DescriptionOutcome::Unchanged => Ok(NativeMapDescriptionOutcome::Unchanged),
        DescriptionOutcome::CommitRequested(commit) => {
            let changed = lifecycle
                .edit_map_description(&commit.text)
                .map_err(|_| status::INVALID_PARAMETER)?;
            ui.dispatch(MapMakerUiCommand::Back);
            Ok(NativeMapDescriptionOutcome::Committed {
                document_changed: changed,
            })
        }
        DescriptionOutcome::Cancelled(_) => {
            ui.dispatch(MapMakerUiCommand::Back);
            Ok(NativeMapDescriptionOutcome::Cancelled)
        }
    }
}

/// Capture the live editor serializer into the already adopted Rust document. Reading the
/// original serializer is a TEMPORARY-SHIM until T2044 S3 owns the live scene state; it is
/// inactive by default and never adopts a map without the native storage owner.
pub fn capture_native_editor_snapshot(guest: &mut dyn tsfp_ui_input::native_page::GuestUi) {
    if !mapmaker_history_enabled()
        || window_active()
        || guest.read32(tsfp_ui_input::editor_pointer::addr::UI_MODE)
            != Some(tsfp_ui_input::editor_pointer::addr::MODE_MAPMAKER)
    {
        return;
    }
    let owns_document = SESSION.lock().is_ok_and(|session| {
        session
            .as_ref()
            .is_some_and(|s| s.lifecycle.editor().is_some())
    });
    if !owns_document {
        return;
    }
    let Some((buffer, snapshot)) = read_live_editor_snapshot(guest) else {
        static READ_FAILURE_LOGGED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !READ_FAILURE_LOGGED.swap(true, Ordering::Relaxed) {
            eprintln!("tsfp-bevy: Rust Map Maker history could not read the live snapshot");
        }
        return;
    };
    let result = SESSION.lock().ok().and_then(|mut session| {
        session
            .as_mut()
            .map(|s| s.lifecycle.record_editor_snapshot(snapshot))
    });
    if matches!(result, Some(Ok(true))) {
        eprintln!("tsfp-bevy: Rust Map Maker history recorded live edit at {buffer:#010x}");
    } else if let Some(Err(status)) = result {
        static REJECT_LOGGED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !REJECT_LOGGED.swap(true, Ordering::Relaxed) {
            eprintln!("tsfp-bevy: Rust Map Maker history rejected live snapshot ({status:#010x})");
        }
    }
}

/// `None` leaves Undo/Redo on the existing handler. Once an owned editor document is present,
/// the native route claims both directions, including an empty stack, and applies the resulting
/// serialized state back through the existing decoder TEMPORARY-SHIM (removal: T2044 S3).
pub fn native_map_history(
    guest: &mut dyn tsfp_ui_input::native_page::GuestUi,
    direction: u32,
) -> Option<bool> {
    if !mapmaker_history_enabled()
        || guest.read32(tsfp_ui_input::editor_pointer::addr::UI_MODE)
            != Some(tsfp_ui_input::editor_pointer::addr::MODE_MAPMAKER)
    {
        return None;
    }
    let owns_document = SESSION
        .lock()
        .ok()?
        .as_ref()
        .is_some_and(|session| session.lifecycle.editor().is_some());
    if !owns_document {
        return None;
    }
    if !matches!(direction, 1 | 2) {
        return Some(false);
    }

    // The menu can be opened in the same frame as the last edit. Capture once at the command
    // boundary too; identical frames are ignored by StorageLifecycle.
    let Some((_, snapshot)) = read_live_editor_snapshot(guest) else {
        return Some(false);
    };
    let Ok(mut guard) = SESSION.lock() else {
        return Some(false);
    };
    let Some(session) = guard.as_mut() else {
        return None;
    };
    if let Err(status) = session.lifecycle.record_editor_snapshot(snapshot) {
        eprintln!("tsfp-bevy: Rust Map Maker history snapshot rejected ({status:#010x})");
        return Some(false);
    }
    drop(guard);

    let (changed, payload) = {
        let mut session = SESSION.lock().ok()?;
        let session = session.as_mut()?;
        let result = if direction == 1 {
            session.lifecycle.undo_editor()
        } else {
            session.lifecycle.redo_editor()
        };
        let changed = match result {
            Ok(changed) => changed,
            Err(status) => {
                eprintln!("tsfp-bevy: Rust Map Maker history apply failed ({status:#010x})");
                return Some(false);
            }
        };
        let payload = changed
            .then(|| session.lifecycle.editor()?.serialize().ok())
            .flatten();
        (changed, payload)
    };
    if !changed {
        return Some(false);
    }
    let Some(payload) = payload else {
        rollback_native_history(direction);
        return Some(false);
    };
    let Some(buffer) = guest.read32(0x78_4014).filter(|address| *address != 0) else {
        rollback_native_history(direction);
        return Some(false);
    };
    let Some(old) = read_guest_map_payload(guest, buffer) else {
        rollback_native_history(direction);
        return Some(false);
    };
    if !write_guest_map_payload(guest, buffer, &payload)
        || guest.call(0x30_1A90, &[buffer, 1]).is_none()
        || guest
            .call(0x2F_E0C0, &[buffer])
            .is_none_or(|terminator| terminator == 0)
    {
        let _ = write_guest_map_payload(guest, buffer, &old);
        let _ = guest.call(0x30_1A90, &[buffer, 1]);
        rollback_native_history(direction);
        return Some(false);
    }

    // The original apply caller refreshes the padded baseline after decoding. Keep that bridge
    // coherent while the native history owns subsequent undo/redo decisions.
    if let Some(baseline) = guest.read32(0x76_5E38).filter(|address| *address != 0) {
        let _ = write_guest_map_payload(guest, baseline, &payload);
    }
    eprintln!(
        "tsfp-bevy: Rust Map Maker {} applied",
        if direction == 1 { "undo" } else { "redo" }
    );
    Some(true)
}

fn rollback_native_history(direction: u32) {
    if let Ok(mut session) = SESSION.lock()
        && let Some(session) = session.as_mut()
    {
        let _ = if direction == 1 {
            session.lifecycle.redo_editor()
        } else {
            session.lifecycle.undo_editor()
        };
    }
}

fn read_live_editor_snapshot(
    guest: &mut dyn tsfp_ui_input::native_page::GuestUi,
) -> Option<(u32, tsfp_engine_fileio::storage_flow::MapPayload)> {
    let buffer = guest.read32(0x78_4014).filter(|address| *address != 0)?;
    guest.call(0x30_06C0, &[buffer, 4])?;
    Some((buffer, read_guest_map_payload(guest, buffer)?))
}

fn read_guest_map_payload(
    guest: &dyn tsfp_ui_input::native_page::GuestUi,
    address: u32,
) -> Option<tsfp_engine_fileio::storage_flow::MapPayload> {
    use tsfp_engine_fileio::storage_flow::{MAP_PAYLOAD_BYTES, MapPayload};
    let mut bytes = Vec::with_capacity(MAP_PAYLOAD_BYTES);
    for offset in (0..MAP_PAYLOAD_BYTES).step_by(4) {
        bytes.extend_from_slice(
            &guest
                .read32(address.checked_add(offset as u32)?)?
                .to_le_bytes(),
        );
    }
    MapPayload::new(bytes).ok()
}

fn write_guest_map_payload(
    guest: &mut dyn tsfp_ui_input::native_page::GuestUi,
    address: u32,
    payload: &tsfp_engine_fileio::storage_flow::MapPayload,
) -> bool {
    payload
        .bytes()
        .chunks_exact(4)
        .enumerate()
        .all(|(index, bytes)| {
            let Ok(offset) = u32::try_from(index * 4) else {
                return false;
            };
            let Some(destination) = address.checked_add(offset) else {
                return false;
            };
            let value = u32::from_le_bytes(bytes.try_into().expect("four-byte chunk"));
            guest.write32(destination, value)
        })
}

pub fn native_editor_save(
    new_save: Option<(String, String)>,
    samples: &mut impl tsfp_engine_fileio::storage_flow::SampleMaps,
) -> Result<tsfp_engine_fileio::storage_frontend::Completion, u32> {
    use tsfp_engine_fileio::vfs::status;
    SESSION
        .lock()
        .map_err(|_| status::INVALID_PARAMETER)?
        .as_mut()
        .ok_or(status::INVALID_PARAMETER)?
        .save_editor(new_save, samples)
}

struct NoSamples;
impl tsfp_engine_fileio::storage_flow::SampleMaps for NoSamples {
    fn load_sample(&mut self, _: u32) -> Result<tsfp_engine_fileio::storage_flow::MapPayload, u32> {
        Err(tsfp_engine_fileio::vfs::status::OBJECT_NAME_NOT_FOUND)
    }
}

struct Runtime<'a>(&'a dyn tsfp_ui_input::native_page::GuestUi);
impl tsfp_engine_fileio::storage_bootstrap::RuntimeRead for Runtime<'_> {
    fn word(&self, address: u32) -> Option<u32> {
        self.0.read32(address)
    }
}

pub fn frame_service(guest: &dyn tsfp_ui_input::native_page::GuestUi) {
    let initialized = SESSION.lock().is_ok_and(|s| s.is_some());
    if !initialized && STORAGE.get().is_some() {
        match tsfp_engine_fileio::storage_bootstrap::title_key(&Runtime(guest)) {
            Ok(Some(key)) => match tsfp_engine_fileio::profile_defaults::mode_defaults(&Runtime(
                guest,
            ))
            .and_then(|modes| {
                let initializer =
                    tsfp_engine_fileio::storage_lifecycle::ProfileInitializer::from_runtime(
                        &Runtime(guest),
                    )?;
                activate_native(
                    key,
                    tsfp_engine_fileio::profile_defaults::ProgressDefaults::new(modes),
                    initializer,
                )
            }) {
                Ok(()) => eprintln!(
                    "tsfp-bevy: native storage session initialized from runtime title authentication"
                ),
                Err(status) => bootstrap_error(status),
            },
            Ok(None) => {}
            Err(status) => bootstrap_error(status),
        }
    }
    if let Ok(mut session) = SESSION.lock()
        && let Some(session) = session.as_mut()
    {
        session.service_profile_creation(&Runtime(guest));
        if let Some(result) = session.service_profile_navigation(&Runtime(guest)) {
            let outcome = match result {
                crate::storage_caller::ProfileSelectionReturn::Selected(_) => "selected",
                crate::storage_caller::ProfileSelectionReturn::Cancelled => "cancelled",
            };
            eprintln!("tsfp-bevy: native retained-page return {outcome}");
        }
        static CHECKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if DIAGNOSTIC_WINDOW.get().is_some() && !CHECKED.swap(true, Ordering::Relaxed) {
            match session.qualify_settings_roundtrip(&mut NoSamples) {
                Ok(()) => eprintln!(
                    "tsfp-bevy: native storage diagnostic: authenticated settings save/deactivate/reload/restore PASS"
                ),
                Err(status) => eprintln!(
                    "tsfp-bevy: native storage diagnostic: settings roundtrip FAIL ({status:#010x})"
                ),
            }
            match session.qualify_map_roundtrip(&mut NoSamples) {
                Ok(true) => eprintln!(
                    "tsfp-bevy: native storage diagnostic: map catalogue/progress/save/reload PASS"
                ),
                Ok(false) => eprintln!(
                    "tsfp-bevy: native storage diagnostic: map catalogue empty; map entries NOT QUALIFIED"
                ),
                Err(status) => eprintln!(
                    "tsfp-bevy: native storage diagnostic: map entries FAIL ({status:#010x})"
                ),
            }
        }
        session.settings_tick(&mut NoSamples);
        // Diagnostic targets are explicit, never inferred from a guest page.
        static WINDOW_CHECKED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if let Some(window) = DIAGNOSTIC_WINDOW.get()
            && !WINDOW_CHECKED.swap(true, Ordering::Relaxed)
        {
            use tsfp_engine_fileio::{storage_frontend::Target, storage_route::Root};
            let (label, opened) = match window {
                DiagnosticWindow::Profile => (
                    "profile catalogue",
                    session.open_root(
                        Root::ProfileTick,
                        Target::Profile { player: 0 },
                        &mut NoSamples,
                    ),
                ),
                DiagnosticWindow::Map => (
                    "map catalogue",
                    crate::sample_maps::with(|samples| {
                        session.open_root(Root::MapCatalogue, Target::Map, samples)
                    }),
                ),
                DiagnosticWindow::MapName(directory) => {
                    ("map name", session.begin_map_name(directory.clone()))
                }
            };
            match opened {
                Ok(()) => {
                    WINDOW_OPEN.store(true, Ordering::Release);
                    eprintln!("tsfp-bevy: native storage diagnostic: owned {label} window opened");
                }
                Err(status) => eprintln!(
                    "tsfp-bevy: native storage diagnostic: window open FAIL ({status:#010x})"
                ),
            }
        }
    }
}
fn bootstrap_error(status: u32) {
    static LAST_ERROR: AtomicU64 = AtomicU64::new(0);
    if LAST_ERROR.swap(u64::from(status), Ordering::Relaxed) != u64::from(status) {
        eprintln!("tsfp-bevy: native storage initialization failed ({status:#010x})");
    }
}
static PRESENCE_QUERIES: AtomicU64 = AtomicU64::new(0);
static ENUMERATIONS: AtomicU64 = AtomicU64::new(0);
static ACTIVE_IDS: AtomicU64 = AtomicU64::new(0);

pub fn report() {
    eprintln!(
        "tsfp-bevy: local storage queries: presence={} enumerations={} active-id-mask={:#x}",
        PRESENCE_QUERIES.load(Ordering::Relaxed),
        ENUMERATIONS.load(Ordering::Relaxed),
        ACTIVE_IDS.load(Ordering::Relaxed)
    );
}

fn root(arguments: &[String]) -> std::io::Result<PathBuf> {
    let mut selected = None;
    let mut args = arguments.iter();
    while let Some(arg) = args.next() {
        if arg == "--hdd" {
            selected = Some(PathBuf::from(args.next().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "--hdd requires a path")
            })?));
        }
    }
    selected.map_or_else(tsfp_engine_fileio::local_storage::user_path, Ok)
}

pub fn init(arguments: &mut Vec<String>) -> std::io::Result<()> {
    crate::sample_maps::init(arguments);
    let selected = root(arguments)?;
    let enabled = std::env::var("TSFP_STORAGE_ROUNDTRIP_CHECK").as_deref() == Ok("1");
    let request = std::env::var("TSFP_STORAGE_ROUNDTRIP_WINDOW").ok();
    let window = diagnostic_window(request.as_deref(), enabled)?;
    if enabled {
        let expected = std::env::var_os("TSFP_STORAGE_ROUNDTRIP_ROOT")
            .map(PathBuf::from)
            .ok_or_else(|| std::io::Error::other("storage diagnostic requires TSFP_STORAGE_ROUNDTRIP_ROOT naming the isolated copied save root"))?;
        if !arguments.iter().any(|arg| arg == "--hdd")
            || expected.canonicalize()? != selected.canonicalize()?
        {
            return Err(std::io::Error::other(
                "storage diagnostic root does not match explicit --hdd",
            ));
        }
    }
    let storage = LocalStorage::open(selected)?;
    if !arguments.iter().any(|arg| arg == "--hdd") {
        arguments.push("--hdd".to_owned());
        arguments.push(storage.root().to_string_lossy().into_owned());
    }
    // Validate the namespace mount before announcing device availability.
    storage
        .vfs()
        .map_err(|status| std::io::Error::other(format!("namespace mount: {status:#x}")))?;
    eprintln!(
        "tsfp-bevy: local storage: one device id={} root={}",
        storage.devices()[0].id,
        storage.root().display()
    );
    STORAGE
        .set(storage)
        .map_err(|_| std::io::Error::other("local storage already initialized"))?;
    if let Some(window) = window {
        DIAGNOSTIC_WINDOW
            .set(window)
            .map_err(|_| std::io::Error::other("storage diagnostic already initialized"))?;
    }
    Ok(())
}

/// -1 retains the standalone/oracle implementation before shell initialization.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_storage_device_active(id: u32) -> i32 {
    STORAGE.get().map_or(-1, |s| {
        PRESENCE_QUERIES.fetch_add(1, Ordering::Relaxed);
        let active = s.device(id).is_some();
        if active && id < 64 {
            ACTIVE_IDS.fetch_or(1 << id, Ordering::Relaxed);
        }
        i32::from(active)
    })
}

/// # Safety
/// `id` is a writable host u32, never a guest address.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_storage_device_record(id: *mut u32) -> i32 {
    let Some(storage) = STORAGE.get() else {
        return 0;
    };
    let Some(id) = (unsafe { id.as_mut() }) else {
        return 0;
    };
    ENUMERATIONS.fetch_add(1, Ordering::Relaxed);
    *id = storage.devices()[0].id;
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn description_route() -> (
        tsfp_mapmaker::ui::MapMakerUiState,
        tsfp_mapmaker::ui::EditorSurfaceTarget,
    ) {
        editor_surface_route(tsfp_mapmaker::ui::EditorSurface::MapDescription)
    }

    fn briefing_route() -> (
        tsfp_mapmaker::ui::MapMakerUiState,
        tsfp_mapmaker::ui::EditorSurfaceTarget,
    ) {
        editor_surface_route(tsfp_mapmaker::ui::EditorSurface::Briefing)
    }

    fn editor_surface_route(
        editor_surface: tsfp_mapmaker::ui::EditorSurface,
    ) -> (
        tsfp_mapmaker::ui::MapMakerUiState,
        tsfp_mapmaker::ui::EditorSurfaceTarget,
    ) {
        use tsfp_mapmaker::ui::{
            EditorSurfaceGeneration, MapListGeneration, MapListRow, MapListRowId, MapLoadOutcome,
            MapMakerUiCommand, MapMakerUiState, MapMakerUiUpdate,
        };
        let mut ui = MapMakerUiState::default();
        let row = MapListRowId::new(MapListGeneration::new(7), 2);
        ui.install_local_map_rows(MapListGeneration::new(7), vec![MapListRow { id: row }])
            .unwrap();
        ui.dispatch(MapMakerUiCommand::OpenStorage);
        ui.dispatch(MapMakerUiCommand::SelectLocalStorage);
        let MapMakerUiUpdate::LoadRequested(request) =
            ui.dispatch(MapMakerUiCommand::SelectMap(row))
        else {
            panic!("selecting a local map should request a load");
        };
        ui.dispatch(MapMakerUiCommand::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        ui.install_editor_surface_catalog(EditorSurfaceGeneration::new(21), vec![editor_surface])
            .unwrap();
        let surface = ui.editor_surface_target(editor_surface).unwrap();
        ui.dispatch(MapMakerUiCommand::OpenEditorSurface(surface));
        (ui, surface)
    }

    fn description_key(
        key_code: bevy::input::keyboard::KeyCode,
        logical_key: bevy::input::keyboard::Key,
    ) -> bevy::input::keyboard::KeyboardInput {
        bevy::input::keyboard::KeyboardInput {
            key_code,
            logical_key,
            state: bevy::input::ButtonState::Pressed,
            text: None,
            repeat: false,
            window: bevy::prelude::Entity::PLACEHOLDER,
        }
    }

    #[test]
    fn mapmaker_description_keyboard_events_translate_native_commands() {
        use bevy::input::keyboard::{Key, KeyCode};
        use tsfp_mapmaker::description::DescriptionCommand as Command;
        let target = tsfp_mapmaker::description::MapDescriptionSession::new(
            tsfp_mapmaker::description::DescriptionGeneration::new(4),
            "",
            16,
        )
        .unwrap()
        .target();

        for (event, expected) in [
            (
                description_key(KeyCode::KeyA, Key::Character("é界".into())),
                vec![Command::Insert('é'), Command::Insert('界')],
            ),
            (
                description_key(KeyCode::Space, Key::Space),
                vec![Command::Insert(' ')],
            ),
            (
                description_key(KeyCode::Backspace, Key::Backspace),
                vec![Command::Backspace],
            ),
            (
                description_key(KeyCode::Delete, Key::Delete),
                vec![Command::Delete],
            ),
            (
                description_key(KeyCode::ArrowLeft, Key::ArrowLeft),
                vec![Command::MoveLeft],
            ),
            (
                description_key(KeyCode::ArrowRight, Key::ArrowRight),
                vec![Command::MoveRight],
            ),
            (
                description_key(KeyCode::Enter, Key::Enter),
                vec![Command::Commit],
            ),
            (
                description_key(KeyCode::NumpadEnter, Key::Enter),
                vec![Command::Commit],
            ),
            (
                description_key(KeyCode::Escape, Key::Escape),
                vec![Command::Cancel],
            ),
        ] {
            assert_eq!(
                mapmaker_description_commands_for_key(&event, target),
                Some(expected)
            );
        }
        assert_eq!(
            mapmaker_description_commands_for_key(
                &description_key(KeyCode::ArrowUp, Key::ArrowUp,),
                target,
            ),
            None
        );
    }

    #[test]
    fn mapmaker_briefing_keyboard_events_translate_typed_text_commands() {
        use bevy::input::keyboard::{Key, KeyCode};
        use tsfp_mapmaker::description::DescriptionCommand as Command;

        let (mut ui, surface) = briefing_route();
        let target = begin_mapmaker_briefing_text_session(&mut ui, surface, "brief", 16).unwrap();
        assert_eq!(
            mapmaker_briefing_commands_for_key(
                &description_key(KeyCode::KeyA, Key::Character("é界".into())),
                target,
            ),
            Some(vec![Command::Insert('é'), Command::Insert('界')])
        );
        assert_eq!(
            mapmaker_briefing_commands_for_key(
                &description_key(KeyCode::Enter, Key::Enter),
                target,
            ),
            Some(vec![Command::Commit])
        );
        assert_eq!(
            mapmaker_briefing_commands_for_key(
                &description_key(KeyCode::Escape, Key::Escape),
                target,
            ),
            Some(vec![Command::Cancel])
        );
        assert_eq!(
            mapmaker_briefing_commands_for_key(
                &description_key(KeyCode::ArrowUp, Key::ArrowUp),
                target,
            ),
            None
        );
    }

    fn description_document() -> tsfp_engine_fileio::map_document::MapDocument {
        use tsfp_engine_fileio::map_document::{Chunk, MapDocument, SETTINGS};
        MapDocument::from_chunks(vec![
            Chunk {
                tag: SETTINGS,
                data: vec![0; 0x3dc],
            },
            Chunk {
                tag: 0xc77672,
                data: vec![42; 256],
            },
        ])
        .unwrap()
    }

    #[test]
    fn mapmaker_briefing_bridge_returns_a_typed_caller_owned_commit() {
        use tsfp_mapmaker::description::DescriptionCommand as Command;
        use tsfp_mapmaker::ui::{BriefingTextCommit, BriefingTextOutcome, MapMakerUiCommand};

        let (mut ui, surface) = briefing_route();
        let target = begin_mapmaker_briefing_text_session(&mut ui, surface, "brief", 12).unwrap();
        assert_eq!(
            ui.briefing_text_view().unwrap().text,
            "brief",
            "the caller-provided field data is the Rust renderer source"
        );
        assert_eq!(
            apply_mapmaker_briefing_text_command(&mut ui, surface, target, Command::Insert('!'),),
            Ok(BriefingTextOutcome::Changed)
        );
        assert_eq!(
            apply_mapmaker_briefing_text_command(&mut ui, surface, target, Command::Commit),
            Ok(BriefingTextOutcome::CommitRequested(BriefingTextCommit {
                target,
                text: "brief!".to_owned(),
            }))
        );
        // The bridge does not guess which document field to update or close the
        // page; the caller owns that policy and receives the typed request.
        assert_eq!(ui.briefing_text_view().unwrap().text, "brief!");

        ui.dispatch(MapMakerUiCommand::Back);
        assert_eq!(
            apply_mapmaker_briefing_text_command(&mut ui, surface, target, Command::Commit),
            Err(tsfp_engine_fileio::vfs::status::INVALID_PARAMETER)
        );
    }

    #[test]
    fn native_briefing_keyboard_result_keeps_page_generation_and_is_taken_once() {
        use tsfp_mapmaker::description::DescriptionCommand as Command;
        use tsfp_mapmaker::ui::{BriefingTextCommit, BriefingTextOutcome};

        let (mut ui, surface) = briefing_route();
        let target = begin_mapmaker_briefing_text_session(&mut ui, surface, "brief", 12).unwrap();
        let outcome =
            apply_mapmaker_briefing_text_command(&mut ui, surface, target, Command::Commit)
                .unwrap();
        queue_native_mapmaker_briefing_text_outcome(surface, outcome);

        assert_eq!(
            native_take_mapmaker_briefing_text_outcome(),
            Some((
                surface,
                BriefingTextOutcome::CommitRequested(BriefingTextCommit {
                    target,
                    text: "brief".to_owned(),
                }),
            ))
        );
        assert_eq!(native_take_mapmaker_briefing_text_outcome(), None);
    }

    #[test]
    fn mapmaker_description_commands_commit_into_owned_history() {
        use tsfp_engine_fileio::storage_lifecycle::NativeStorageLifecycle;
        use tsfp_mapmaker::description::DescriptionCommand as Command;

        let (mut ui, surface) = description_route();
        let mut lifecycle = NativeStorageLifecycle::default();
        lifecycle.edit(description_document());
        let target = begin_mapmaker_description_session(&mut ui, &lifecycle, surface, 32).unwrap();
        assert_eq!(
            apply_mapmaker_description_command(
                &mut ui,
                &mut lifecycle,
                surface,
                target,
                Command::Insert('m'),
            ),
            Ok(NativeMapDescriptionOutcome::Changed)
        );
        for ch in ['a', 'p'] {
            apply_mapmaker_description_command(
                &mut ui,
                &mut lifecycle,
                surface,
                target,
                Command::Insert(ch),
            )
            .unwrap();
        }
        assert_eq!(
            apply_mapmaker_description_command(
                &mut ui,
                &mut lifecycle,
                surface,
                target,
                Command::Commit,
            ),
            Ok(NativeMapDescriptionOutcome::Committed {
                document_changed: true,
            })
        );
        assert_eq!(lifecycle.editor().unwrap().description_bytes(), b"map");
        assert!(lifecycle.editor_dirty());
        assert!(lifecycle.editor_can_undo());
        // A successful commit closes the Rust-owned page; a queued duplicate
        // commit must not be treated as an idempotent edit after that close.
        assert_eq!(
            apply_mapmaker_description_command(
                &mut ui,
                &mut lifecycle,
                surface,
                target,
                Command::Commit,
            ),
            Err(tsfp_engine_fileio::vfs::status::INVALID_PARAMETER)
        );
        assert!(lifecycle.undo_editor().unwrap());
        assert!(lifecycle.editor().unwrap().description_bytes().is_empty());
    }

    #[test]
    fn mapmaker_description_commit_rejects_stale_surface_after_back() {
        use tsfp_engine_fileio::storage_lifecycle::NativeStorageLifecycle;
        use tsfp_mapmaker::description::DescriptionCommand;
        use tsfp_mapmaker::ui::MapMakerUiCommand;

        let (mut ui, surface) = description_route();
        let mut lifecycle = NativeStorageLifecycle::default();
        lifecycle.edit(description_document());
        let target = begin_mapmaker_description_session(&mut ui, &lifecycle, surface, 32).unwrap();
        ui.dispatch(MapMakerUiCommand::Back);
        assert_eq!(
            apply_mapmaker_description_command(
                &mut ui,
                &mut lifecycle,
                surface,
                target,
                DescriptionCommand::Commit,
            ),
            Err(tsfp_engine_fileio::vfs::status::INVALID_PARAMETER)
        );
        assert!(lifecycle.editor().unwrap().description_bytes().is_empty());
        assert!(!lifecycle.editor_can_undo());
    }

    #[test]
    fn local_map_save_reports_storage_progress_from_session_result() {
        use tsfp_mapmaker::ui::{
            MapMakerUiState, StorageOperation, StorageProgressOutcome as Outcome,
        };

        let mut ui = MapMakerUiState::default();
        let progress = begin_mapmaker_storage_progress(&mut ui, StorageOperation::SaveMap)
            .expect("eligible Map Maker page starts progress");
        assert_eq!(ui.storage_progress().unwrap().outcome, Outcome::Pending);

        let failed: Result<(), u32> = Err(0xC000_0001);
        assert_eq!(mapmaker_storage_progress_outcome(&failed), Outcome::Failed);
        assert!(finish_mapmaker_storage_progress(
            &mut ui,
            progress,
            mapmaker_storage_progress_outcome(&failed),
        ));
        assert_eq!(ui.storage_progress().unwrap().outcome, Outcome::Failed);
        assert!(!finish_mapmaker_storage_progress(
            &mut ui,
            progress,
            Outcome::Succeeded,
        ));

        let succeeded: Result<(), u32> = Ok(());
        assert_eq!(
            mapmaker_storage_progress_outcome(&succeeded),
            Outcome::Succeeded
        );
    }

    fn storage_progress_page() -> tsfp_ui_model::Page {
        use tsfp_ui_model::model::{TextId, TextRef};
        use tsfp_ui_model::{Page, PageId, Widget, WidgetId, WidgetKind};

        let mut page = Page::new(PageId(0x32_AEE0), 1);
        for (widget_id, text_id) in [
            (WidgetId(9), TextId(0x0E68)),
            (WidgetId(10), TextId(0x1986)),
        ] {
            let mut widget = Widget::new(widget_id, WidgetKind::Label);
            widget.text = TextRef::Id(text_id);
            widget.navigable = false;
            page.widgets.push(widget);
        }
        page
    }

    #[test]
    fn registered_mapmaker_status_page_preserves_owned_operation_outcome() {
        use tsfp_mapmaker::ui::{
            MapMakerUiCommand, MapMakerUiState, StorageOperation, StorageProgressOutcome,
        };

        let mut state = MapMakerUiState::default();
        state.dispatch(MapMakerUiCommand::OpenStorageProgress(
            StorageOperation::SaveMap,
        ));
        let page = storage_progress_page();
        let projection =
            project_mapmaker_storage_progress_from_state(&state, page.clone()).unwrap();
        assert_eq!(projection.page, page);
        assert_eq!(projection.progress.operation, StorageOperation::SaveMap);
        assert_eq!(projection.progress.outcome, StorageProgressOutcome::Pending);

        let mut completed = state;
        let progress = completed.storage_progress().unwrap().id;
        completed.dispatch(MapMakerUiCommand::CompleteStorageProgress {
            progress,
            outcome: StorageProgressOutcome::Succeeded,
        });
        let completed_projection =
            project_mapmaker_storage_progress_from_state(&completed, storage_progress_page())
                .unwrap();
        assert_eq!(
            completed_projection.progress.outcome,
            StorageProgressOutcome::Succeeded
        );
    }

    #[test]
    fn registered_mapmaker_status_page_requires_measured_page_and_text_identities() {
        use tsfp_mapmaker::ui::{MapMakerUiCommand, MapMakerUiState, StorageOperation};
        use tsfp_ui_model::mapmaker_storage::StorageProgressBuildError as Error;
        use tsfp_ui_model::{Page, PageId};

        let mut state = MapMakerUiState::default();
        state.dispatch(MapMakerUiCommand::OpenStorageProgress(
            StorageOperation::LoadMap,
        ));
        assert_eq!(
            project_mapmaker_storage_progress_from_state(&state, Page::new(PageId(0x32_B6A0), 1),),
            Err(Error::WrongPage)
        );
        assert_eq!(
            project_mapmaker_storage_progress_from_state(&state, Page::new(PageId(0x32_AEE0), 1),),
            Err(Error::MissingStatusWidget)
        );
    }

    #[test]
    fn mapmaker_save_as_route_requires_current_save_surface_generation() {
        use tsfp_mapmaker::ui::{
            EditorSurface, EditorSurfaceGeneration, EditorSurfaceTarget, MapLoadOutcome,
            MapMakerPage, MapMakerUiCommand, MapMakerUiState, MapMakerUiUpdate,
        };

        let mut ui = MapMakerUiState::default();
        ui.dispatch(MapMakerUiCommand::OpenStorage);
        assert!(install_mapmaker_local_rows(&mut ui, 1, Some(0)));
        let row = ui.map_rows()[0].id;
        let MapMakerUiUpdate::LoadRequested(request) =
            ui.dispatch(MapMakerUiCommand::SelectMap(row))
        else {
            panic!("selected map row should request a load");
        };
        ui.dispatch(MapMakerUiCommand::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        assert_eq!(ui.page(), MapMakerPage::Editor);
        ui.install_editor_surface_catalog(
            EditorSurfaceGeneration::new(17),
            vec![EditorSurface::SaveMap],
        )
        .unwrap();
        let target = ui.editor_surface_target(EditorSurface::SaveMap).unwrap();
        assert!(!is_current_mapmaker_save_as_route(&ui, target));
        assert_eq!(
            ui.dispatch(MapMakerUiCommand::OpenEditorSurface(target)),
            MapMakerUiUpdate::EditorSaveMapRouteOpened
        );
        assert!(is_current_mapmaker_save_as_route(&ui, target));

        let stale =
            EditorSurfaceTarget::new(EditorSurfaceGeneration::new(16), EditorSurface::SaveMap);
        assert!(!is_current_mapmaker_save_as_route(&ui, stale));
        assert_eq!(
            ui.dispatch(MapMakerUiCommand::Back),
            MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::MainMenu)
        );
        assert!(!is_current_mapmaker_save_as_route(&ui, target));
    }

    #[test]
    fn native_map_catalogue_installs_generation_qualified_rows_and_focus() {
        let mut ui = tsfp_mapmaker::ui::MapMakerUiState::default();
        ui.dispatch(tsfp_mapmaker::ui::MapMakerUiCommand::OpenStorage);
        assert!(install_mapmaker_local_rows(&mut ui, 3, Some(1)));
        let prior = ui.focused_map().unwrap();
        assert_eq!(ui.page(), tsfp_mapmaker::ui::MapMakerPage::MapList);
        assert_eq!(prior, ui.map_rows()[1].id);

        ui.dispatch(tsfp_mapmaker::ui::MapMakerUiCommand::Back);
        assert!(install_mapmaker_local_rows(&mut ui, 2, Some(0)));
        assert_ne!(ui.map_rows()[0].id.generation(), prior.generation());
        assert_eq!(ui.focused_map(), Some(ui.map_rows()[0].id));
    }

    #[test]
    fn local_map_list_keeps_hdd_source_when_sample_catalogue_is_available() {
        use crate::storage_caller::{CatalogueAction, CatalogueView};
        use tsfp_engine_fileio::{
            storage_flow::{SampleMapEntry, SaveEntry, SaveKey, SaveKind},
            storage_frontend::Target,
        };
        use tsfp_mapmaker::ui::{MapMakerUiCommand, MapMakerUiState, MapStorageSource};

        let mut ui = MapMakerUiState::default();
        ui.dispatch(MapMakerUiCommand::OpenStorage);
        ui.dispatch(MapMakerUiCommand::FocusStorageSource(
            MapStorageSource::LocalStorage,
        ));
        ui.dispatch(MapMakerUiCommand::SelectLocalStorage);
        let view = CatalogueView {
            target: Target::Map,
            rows: vec![
                SaveEntry {
                    key: SaveKey::new("maps", "local-a").unwrap(),
                    kind: SaveKind::Map,
                    label: Ok("Local A".into()),
                    bytes: 0,
                },
                SaveEntry {
                    key: SaveKey::new("maps", "local-b").unwrap(),
                    kind: SaveKind::Map,
                    label: Ok("Local B".into()),
                    bytes: 0,
                },
            ],
            sample_maps: vec![SampleMapEntry::new(17, "Disc sample".into()).unwrap()],
            map_source_choice: false,
            cursor: Some(1),
            error: None,
        };

        mapmaker_catalogue_action_begin(&mut ui, &view, CatalogueAction::Focus(1));

        assert_eq!(
            ui.focused_storage_source(),
            Some(MapStorageSource::LocalStorage)
        );
        assert_eq!(ui.map_rows().len(), 2);
        assert_eq!(ui.focused_map(), Some(ui.map_rows()[1].id));
    }

    #[test]
    fn map_catalogue_refresh_replaces_same_count_row_generation() {
        use crate::storage_caller::{CatalogueAction, CatalogueView};
        use tsfp_engine_fileio::{
            storage_flow::{SaveEntry, SaveKey, SaveKind},
            storage_frontend::Target,
        };
        use tsfp_mapmaker::ui::MapMakerUiState;

        let mut ui = MapMakerUiState::default();
        let view = |name: &str| CatalogueView {
            target: Target::Map,
            rows: vec![SaveEntry {
                key: SaveKey::new("maps", name).unwrap(),
                kind: SaveKind::Map,
                label: Ok(name.into()),
                bytes: 0,
            }],
            sample_maps: Vec::new(),
            map_source_choice: false,
            cursor: Some(0),
            error: None,
        };

        mapmaker_catalogue_action_begin(&mut ui, &view("old-entry"), CatalogueAction::Focus(0));
        let old_target = ui.map_rows()[0].id;
        mapmaker_catalogue_action_begin(
            &mut ui,
            &view("replacement-entry"),
            CatalogueAction::Focus(0),
        );

        assert_ne!(ui.map_rows()[0].id.generation(), old_target.generation());
        assert_eq!(ui.focused_map(), Some(ui.map_rows()[0].id));
    }

    #[test]
    fn pointer_catalogue_actions_dispatch_to_owned_map_rows() {
        use crate::storage_caller::{CatalogueAction, CatalogueView};
        use tsfp_engine_fileio::storage_frontend::Target;
        use tsfp_mapmaker::ui::{MapMakerPage, MapMakerUiCommand, MapMakerUiState};

        let mut ui = MapMakerUiState::default();
        ui.dispatch(MapMakerUiCommand::OpenStorage);
        assert!(install_mapmaker_local_rows(&mut ui, 2, Some(0)));
        let first_row = ui.map_rows()[0].id;
        let second_row = ui.map_rows()[1].id;
        let view = |cursor| CatalogueView {
            target: Target::Map,
            rows: (0..2)
                .map(|index| tsfp_engine_fileio::storage_flow::SaveEntry {
                    key: tsfp_engine_fileio::storage_flow::SaveKey::new(
                        "maps",
                        &format!("row-{index}"),
                    )
                    .unwrap(),
                    kind: tsfp_engine_fileio::storage_flow::SaveKind::Map,
                    label: Ok(format!("row {index}")),
                    bytes: 0,
                })
                .collect(),
            sample_maps: Vec::new(),
            map_source_choice: false,
            cursor,
            error: None,
        };

        assert_eq!(
            mapmaker_catalogue_action_begin(&mut ui, &view(Some(0)), CatalogueAction::Focus(1),),
            None
        );
        assert_eq!(ui.page(), MapMakerPage::MapList);
        let focused_row = ui.map_rows()[1].id;
        assert_eq!(ui.focused_map(), Some(focused_row));
        assert_ne!(focused_row, second_row);
        let request =
            mapmaker_catalogue_action_begin(&mut ui, &view(Some(1)), CatalogueAction::Accept)
                .expect("pointer acceptance emits the owned load request");
        assert_eq!(request.row, ui.map_rows()[1].id);
        assert_ne!(request.row, focused_row);
        assert_ne!(request.row, first_row);
        assert_eq!(ui.pending_map_load(), Some(request));
    }

    #[test]
    fn mapmaker_source_choice_tracks_the_runtime_focused_row() {
        use crate::storage_caller::{CatalogueAction, CatalogueView};
        use tsfp_engine_fileio::storage_frontend::Target;
        use tsfp_mapmaker::ui::{MapMakerPage, MapMakerUiState, MapStorageSource};

        let mut ui = MapMakerUiState::default();
        let source_view = |cursor| CatalogueView {
            target: Target::Map,
            rows: Vec::new(),
            sample_maps: Vec::new(),
            map_source_choice: true,
            cursor,
            error: None,
        };

        assert_eq!(
            mapmaker_catalogue_action_begin(
                &mut ui,
                &source_view(Some(0)),
                CatalogueAction::Accept,
            ),
            None
        );
        assert_eq!(ui.page(), MapMakerPage::MapList);
        assert_eq!(
            ui.focused_storage_source(),
            Some(MapStorageSource::SampleMaps)
        );

        mapmaker_catalogue_action_begin(&mut ui, &source_view(Some(1)), CatalogueAction::Focus(1));
        assert_eq!(
            ui.focused_storage_source(),
            Some(MapStorageSource::LocalStorage)
        );
        mapmaker_catalogue_action_begin(&mut ui, &source_view(Some(1)), CatalogueAction::Accept);
        assert_eq!(ui.page(), MapMakerPage::MapList);
    }

    #[test]
    fn sample_catalogue_rows_use_the_owned_map_list_request() {
        use crate::storage_caller::{CatalogueAction, CatalogueView};
        use tsfp_engine_fileio::{storage_flow::SampleMapEntry, storage_frontend::Target};
        use tsfp_mapmaker::ui::{MapMakerUiCommand, MapMakerUiState, MapStorageSource};

        let mut ui = MapMakerUiState::default();
        ui.dispatch(MapMakerUiCommand::OpenStorage);
        ui.dispatch(MapMakerUiCommand::FocusStorageSource(
            MapStorageSource::SampleMaps,
        ));
        ui.dispatch(MapMakerUiCommand::SelectSampleMaps);
        let view = CatalogueView {
            target: Target::Map,
            rows: Vec::new(),
            sample_maps: vec![
                SampleMapEntry::new(31, "Runtime sample A".into()).unwrap(),
                SampleMapEntry::new(84, "Runtime sample B".into()).unwrap(),
            ],
            map_source_choice: false,
            cursor: Some(0),
            error: None,
        };

        assert_eq!(
            mapmaker_catalogue_action_begin(&mut ui, &view, CatalogueAction::Focus(1)),
            None
        );
        assert_eq!(ui.map_rows().len(), 2);
        let selected = ui.map_rows()[1].id;
        let request = mapmaker_catalogue_action_begin(
            &mut ui,
            &CatalogueView {
                cursor: Some(1),
                ..view
            },
            CatalogueAction::Accept,
        )
        .expect("sample acceptance emits the same typed host-load request");
        assert_eq!(request.row, ui.map_rows()[1].id);
        assert_ne!(request.row, selected);
        assert_eq!(ui.pending_map_load(), Some(request));
    }

    #[test]
    fn mapmaker_ui_state_attachment_is_opt_in_until_the_replay_gate() {
        assert!(!mapmaker_ui_enabled_from(None));
        assert!(!mapmaker_ui_enabled_from(Some("1")));
        assert!(!mapmaker_ui_enabled_from(Some("true")));
        assert!(mapmaker_ui_enabled_from(Some("rust")));
    }

    #[test]
    fn mapmaker_history_runtime_switch_is_opt_in() {
        assert!(!mapmaker_history_enabled_from(None));
        assert!(!mapmaker_history_enabled_from(Some("0")));
        assert!(!mapmaker_history_enabled_from(Some("true")));
        assert!(mapmaker_history_enabled_from(Some("1")));
    }

    #[test]
    fn mapmaker_menu_history_availability_uses_owned_lifecycle_and_stays_opt_in() {
        use tsfp_engine_fileio::storage_lifecycle::NativeStorageLifecycle;
        use tsfp_mapmaker::history::HistoryAvailability;

        let mut lifecycle = NativeStorageLifecycle::default();
        assert_eq!(editor_history_availability(false, Some(&lifecycle)), None);
        assert_eq!(editor_history_availability(true, Some(&lifecycle)), None);

        lifecycle.edit(description_document());
        assert_eq!(
            editor_history_availability(true, Some(&lifecycle)),
            Some(HistoryAvailability::default())
        );

        assert!(lifecycle.edit_map_description("native history").unwrap());
        assert_eq!(
            editor_history_availability(true, Some(&lifecycle)),
            Some(HistoryAvailability {
                can_undo: true,
                can_redo: false,
            })
        );
        assert!(lifecycle.undo_editor().unwrap());
        assert_eq!(
            editor_history_availability(true, Some(&lifecycle)),
            Some(HistoryAvailability {
                can_undo: false,
                can_redo: true,
            })
        );
    }

    #[test]
    fn diagnostic_windows_require_guard_and_safe_explicit_map_identity() {
        assert_eq!(diagnostic_window(None, false).unwrap(), None);
        assert_eq!(
            diagnostic_window(None, true).unwrap(),
            Some(DiagnosticWindow::Profile)
        );
        assert_eq!(
            diagnostic_window(Some("map"), true).unwrap(),
            Some(DiagnosticWindow::Map)
        );
        assert_eq!(
            diagnostic_window(Some("map-name:NativeSave"), true).unwrap(),
            Some(DiagnosticWindow::MapName("NativeSave".into()))
        );
        for value in ["profile", "map", "map-name:NativeSave"] {
            assert!(diagnostic_window(Some(value), false).is_err());
        }
        for value in [
            "",
            "unknown",
            "map-name:",
            "map-name:../owner",
            "map-name:/owner",
        ] {
            assert!(diagnostic_window(Some(value), true).is_err());
        }
    }

    #[test]
    fn native_map_directory_is_a_valid_unique_storage_identity() {
        let first = next_map_directory();
        let second = next_map_directory();
        assert_ne!(first, second);
        tsfp_engine_fileio::storage_flow::SaveKey::new(&first, "anicemap.mkr").unwrap();
        tsfp_engine_fileio::storage_flow::SaveKey::new(&second, "anicemap.mkr").unwrap();
    }

    #[test]
    fn native_map_save_falls_through_without_an_owned_document() {
        assert!(!native_editor_save_claim(false, true));
        assert!(!native_editor_save_claim(true, false));
        assert!(native_editor_save_claim(true, true));
    }

    #[test]
    fn native_map_load_falls_through_until_a_catalogue_is_open() {
        assert!(!native_editor_load_claim(false, true, true));
        assert!(!native_editor_load_claim(true, false, false));
        assert!(!native_editor_load_claim(true, true, false));
        assert!(native_editor_load_claim(true, true, true));
    }

    #[test]
    fn title_provider_is_lazy_until_a_sample_choice_or_row_is_accepted() {
        use crate::storage_caller::{CatalogueAction, CatalogueView};
        use tsfp_engine_fileio::{
            storage_flow::{SampleMapEntry, SaveEntry, SaveKey, SaveKind},
            storage_frontend::Target,
        };
        let source_choice = CatalogueView {
            target: Target::Map,
            rows: Vec::new(),
            sample_maps: Vec::new(),
            map_source_choice: true,
            cursor: Some(1),
            error: None,
        };
        assert!(!native_actions_need_samples(
            &source_choice,
            &[CatalogueAction::Accept]
        ));
        assert!(native_actions_need_samples(
            &source_choice,
            &[CatalogueAction::Focus(0), CatalogueAction::Accept]
        ));
        let local = SaveEntry {
            key: SaveKey::new("local", "anicemap.mkr").unwrap(),
            kind: SaveKind::Map,
            label: Ok("Local map".into()),
            bytes: 32_756,
        };
        let samples = CatalogueView {
            target: Target::Map,
            rows: vec![local],
            sample_maps: vec![SampleMapEntry::new(77, "Sample".into()).unwrap()],
            map_source_choice: false,
            cursor: Some(0),
            error: None,
        };
        assert!(!native_actions_need_samples(
            &samples,
            &[CatalogueAction::Accept]
        ));
        assert!(native_actions_need_samples(
            &samples,
            &[CatalogueAction::Focus(1), CatalogueAction::Accept]
        ));
    }

    #[test]
    fn actual_host_root_is_selected_and_missing_value_is_an_error() {
        let args = ["disc.xbe", "--hdd", "/first", "--hdd", "/actual root"].map(str::to_owned);
        assert_eq!(root(&args).unwrap(), PathBuf::from("/actual root"));
        assert!(root(&["--hdd".to_owned()]).is_err());
        assert_eq!(tsfp_storage_device_active(9), -1);
        let temp = std::env::temp_dir().join(format!("t2319-device-{}", std::process::id()));
        let mut arguments = vec!["--hdd".to_owned(), temp.to_str().unwrap().to_owned()];
        init(&mut arguments).unwrap();
        for id in 0..13 {
            assert_eq!(tsfp_storage_device_active(id), i32::from(id == 9));
        }
        let mut id = 0;
        assert_eq!(unsafe { tsfp_storage_device_record(&mut id) }, 1);
        assert_eq!(id, 9);
        assert_eq!(
            unsafe { tsfp_storage_device_record(std::ptr::null_mut()) },
            0
        );
        std::fs::remove_dir_all(temp).unwrap();
    }

    /// T3820: a Map Maker briefing typed live, recorded as input.rec v6 `D` rows and replayed through the same
    /// native key path, ends with the same field text (format v5 and older never held these keys).
    #[test]
    fn a_recorded_briefing_key_sequence_replays_to_the_same_text() {
        use bevy::input::ButtonState;
        use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
        use tsfp_input::recording::{Payload, Reader, Record, Writer};
        use tsfp_input_intake::{DeviceEvent, codec, schedule::Schedule};

        let typed: Vec<KeyboardInput> = [
            (KeyCode::KeyH, Key::Character("H".into())),
            (KeyCode::KeyI, Key::Character("i".into())),
            (KeyCode::Space, Key::Space),
            (KeyCode::KeyE, Key::Character("é".into())),
            (KeyCode::KeyX, Key::Character("x".into())),
            (KeyCode::Backspace, Key::Backspace),
            (KeyCode::ArrowLeft, Key::ArrowLeft),
            (KeyCode::Digit1, Key::Character("!".into())),
        ]
        .into_iter()
        .flat_map(|(key_code, logical_key)| {
            let down = description_key(key_code, logical_key);
            let up = KeyboardInput {
                state: ButtonState::Released,
                ..down.clone()
            };
            [down, up]
        })
        .collect();
        let type_into = |keys: &[KeyboardInput]| {
            let (mut ui, surface) = briefing_route();
            let target = begin_mapmaker_briefing_text_session(&mut ui, surface, "", 16).unwrap();
            for key in keys.iter().filter(|key| key.state == ButtonState::Pressed) {
                for command in mapmaker_briefing_commands_for_key(key, target).unwrap_or_default() {
                    apply_mapmaker_briefing_text_command(&mut ui, surface, target, command)
                        .unwrap();
                }
            }
            ui.briefing_text_view().unwrap().text.clone()
        };
        let live = type_into(&typed);
        assert_eq!(live, "Hi !é", "the live typing itself");

        let mut writer = Writer::stamped(Vec::new(), false, Some("t3820")).unwrap();
        for (poll, key) in (10..).zip(&typed) {
            let payload = codec::encode(&DeviceEvent::key(key));
            let row = Payload {
                sim_tick: poll / 3,
                host_poll: poll,
                wall_ns: poll,
                payload,
            };
            writer.device(&row).unwrap();
        }
        let bytes = writer.finish().unwrap();
        let mut reader = Reader::new(bytes.as_slice()).unwrap();
        let mut rows = Vec::new();
        while let Some(record) = reader.next().unwrap() {
            if let Record::Device(row) = record {
                rows.push((row.host_poll, row.payload));
            }
        }
        let mut schedule = Schedule::new(rows);
        let replayed: Vec<KeyboardInput> = (0..40)
            .flat_map(|poll| schedule.due(poll))
            .filter_map(|event| event.keyboard_input(bevy::prelude::Entity::PLACEHOLDER))
            .collect();
        assert_eq!(replayed.len(), typed.len());
        assert_eq!(type_into(&replayed), live);
    }
}
