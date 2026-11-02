//! Runtime PAK-to-StoryMission attachment (T2410).
//!
//! The host supplies the active mission PAK path and its typed SETU selector. This adapter uses
//! the live VFS and streaming PAK index, then gives the decoded entry to the gameplay owner. It
//! does not infer mission identity or active session state from guest memory.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use tsfp_data_formats::cutscenes::CutsceneState;
use tsfp_engine_core::PlayerState;
use tsfp_engine_fileio::assets::AssetKey;
use tsfp_engine_fileio::vfs::{NtStatus, Vfs, status};
use tsfp_engine_fileio::xiso::Xiso;
use tsfp_engine_memory::{GuestMemory, MemoryFault};
use tsfp_gameplay::modes::story_sequencer::{ModeBlock, StorySequencerOutput};
use tsfp_gameplay::story_events::BuildOptions;
use tsfp_gameplay::story_events::engine::StepReport;
use tsfp_gameplay::story_events::native_world::{
    NativeStoryWorld, StoryGameplayInputSession, StoryGameplayInputs, StoryGameplayInputsStore,
    StoryGameplayOwnerStoreError, StoryMissionResult, StoryPlayerState, StoryWorldEffect,
    player_owner::StoryPlayerHandle,
};
use tsfp_gameplay::story_events::setu::SetuAssetSelector;
use tsfp_gameplay::story_events::world::World;
use tsfp_gameplay::story_mission::{StoryMission, StoryMissionLoadError};
use tsfp_gameplay::story_schedule::{StoryExecution, StorySchedule};
use tsfp_state::story_flow::{
    CampaignLevelTable, CampaignOrdinal, MODE_STORY, SessionSetup, StoryContinuation,
    StoryContinuationGate, StoryController, StoryFlow, StoryStep,
};
use tsfp_state::terminals::{TerminalPadBank, TerminalState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeStoryMissionError {
    FileIo(NtStatus),
    GuestMemory(MemoryFault),
    Mission(StoryMissionLoadError),
    SelectionStore(StoryMissionSelectionStoreError),
    SequencerOutputStore(StorySequencerOutputStoreError),
    HostOwnerInstall(StoryMissionHostInstallError),
    SelectionSuperseded,
    CampaignOrdinal(i32),
}

/// Complete typed selection handed from the Story sequencer to gameplay construction.
/// The SETU selector and setup block travel together so a caller cannot accidentally pair a
/// mission payload with a different sequencer row. The flow is the Rust-owned session state
/// supplied by the sequencer; it is never recovered from guest memory here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryMissionSelection {
    flow: StoryFlow,
    setup: ModeBlock,
    selector: SetuAssetSelector,
}

impl StoryMissionSelection {
    pub fn new(
        flow: StoryFlow,
        setup: ModeBlock,
        selector: SetuAssetSelector,
    ) -> Result<Self, StoryMissionLoadError> {
        let actual_mode = setup.bytes()[0x55];
        if actual_mode != MODE_STORY {
            return Err(StoryMissionLoadError::NotStorySetup { actual_mode });
        }
        Ok(Self {
            flow,
            setup,
            selector,
        })
    }

    /// Construct the owned first gameplay flow from one completed typed sequencer result.
    ///
    /// The ordinal is the sequencer's row position, not the level identifier. PAK lookup remains
    /// an explicit input because the Story data does not establish a key/occurrence mapping here.
    pub fn from_sequencer_output(
        setup: ModeBlock,
        campaign_ordinal: i32,
        selector: SetuAssetSelector,
    ) -> Result<Self, RuntimeStoryMissionError> {
        let ordinal = u8::try_from(campaign_ordinal)
            .ok()
            .and_then(CampaignOrdinal::new)
            .ok_or(RuntimeStoryMissionError::CampaignOrdinal(campaign_ordinal))?;
        let session = SessionSetup::from_words(setup.words());
        if session.byte(0x55) != MODE_STORY {
            return Err(RuntimeStoryMissionError::Mission(
                StoryMissionLoadError::NotStorySetup {
                    actual_mode: session.byte(0x55),
                },
            ));
        }
        let mut flow = StoryFlow::default();
        flow.ordinal = ordinal;
        flow.stage(session);
        flow.start_level();
        Self::new(flow, setup, selector).map_err(RuntimeStoryMissionError::Mission)
    }

    /// Join the exact owned setup and bounded row emitted by the typed Story sequencer.
    ///
    /// The PAK selector remains a separate caller input because the measured Story contract does
    /// not establish a campaign-row-to-SETU-key mapping.
    pub fn from_typed_sequencer_output(
        output: StorySequencerOutput,
        selector: SetuAssetSelector,
    ) -> Result<Self, RuntimeStoryMissionError> {
        Self::from_sequencer_output(
            output.setup().clone(),
            i32::from(output.campaign_ordinal().get()),
            selector,
        )
    }

    pub fn flow(&self) -> &StoryFlow {
        &self.flow
    }

    pub fn setup(&self) -> &ModeBlock {
        &self.setup
    }

    pub const fn selector(&self) -> SetuAssetSelector {
        self.selector
    }
}

/// Single-use, revisioned handoff from the typed Story sequencer to the gameplay loader.
/// A gameplay consumer must request a revision newer than its last consumed value; stale setup
/// and PAK selection data therefore cannot silently start a second mission.
#[derive(Clone, Default)]
pub struct StoryMissionSelectionStore(Arc<RwLock<StoryMissionSelectionStoreState>>);

#[derive(Default)]
struct StoryMissionSelectionStoreState {
    revision: u64,
    selection: Option<StoryMissionSelection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryMissionSelectionStoreError {
    Poisoned,
    RevisionExhausted,
}

impl StoryMissionSelectionStore {
    pub fn publish(
        &self,
        selection: StoryMissionSelection,
    ) -> Result<u64, StoryMissionSelectionStoreError> {
        let mut current = self
            .0
            .write()
            .map_err(|_| StoryMissionSelectionStoreError::Poisoned)?;
        let revision = current
            .revision
            .checked_add(1)
            .ok_or(StoryMissionSelectionStoreError::RevisionExhausted)?;
        current.revision = revision;
        current.selection = Some(selection);
        Ok(revision)
    }

    /// Publish one validated output from the typed sequencer without separating its setup from
    /// its campaign row. Failure leaves the current selection and revision unchanged.
    pub fn publish_sequencer_output(
        &self,
        output: StorySequencerOutput,
        selector: SetuAssetSelector,
    ) -> Result<u64, RuntimeStoryMissionError> {
        let selection = StoryMissionSelection::from_typed_sequencer_output(output, selector)?;
        self.publish(selection)
            .map_err(RuntimeStoryMissionError::SelectionStore)
    }

    pub fn take_after(
        &self,
        after_revision: u64,
    ) -> Result<Option<(u64, StoryMissionSelection)>, StoryMissionSelectionStoreError> {
        let mut current = self
            .0
            .write()
            .map_err(|_| StoryMissionSelectionStoreError::Poisoned)?;
        if current.revision <= after_revision {
            return Ok(None);
        }
        let Some(selection) = current.selection.take() else {
            return Ok(None);
        };
        Ok(Some((current.revision, selection)))
    }

    /// Inspect a newer selection without consuming it. Consumers that must perform fallible
    /// mission construction should acknowledge only after the candidate owner is ready.
    pub fn peek_after(
        &self,
        after_revision: u64,
    ) -> Result<Option<(u64, StoryMissionSelection)>, StoryMissionSelectionStoreError> {
        let current = self
            .0
            .read()
            .map_err(|_| StoryMissionSelectionStoreError::Poisoned)?;
        if current.revision <= after_revision {
            return Ok(None);
        }
        Ok(current
            .selection
            .as_ref()
            .map(|selection| (current.revision, selection.clone())))
    }

    /// Consume exactly the revision that was successfully built. A newer publication wins and
    /// leaves that replacement available to its next consumer.
    pub fn consume_revision(&self, revision: u64) -> Result<bool, StoryMissionSelectionStoreError> {
        let mut current = self
            .0
            .write()
            .map_err(|_| StoryMissionSelectionStoreError::Poisoned)?;
        if current.revision != revision || current.selection.is_none() {
            return Ok(false);
        }
        current.selection = None;
        Ok(true)
    }
}

/// Rust-owned cross-thread handoff for the validated setup/ordinal pair emitted by the
/// `0x65940` ports lane. It deliberately does not contain a SETU selector: the measured contract
/// still does not identify the PAK key for a campaign row.
#[derive(Clone, Default)]
pub struct StorySequencerOutputStore(Arc<RwLock<StorySequencerOutputStoreState>>);

#[derive(Default)]
struct StorySequencerOutputStoreState {
    revision: u64,
    output: Option<StorySequencerOutput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorySequencerOutputStoreError {
    Poisoned,
    RevisionExhausted,
}

impl StorySequencerOutputStore {
    pub fn publish(
        &self,
        output: StorySequencerOutput,
    ) -> Result<u64, StorySequencerOutputStoreError> {
        let mut current = self
            .0
            .write()
            .map_err(|_| StorySequencerOutputStoreError::Poisoned)?;
        let revision = current
            .revision
            .checked_add(1)
            .ok_or(StorySequencerOutputStoreError::RevisionExhausted)?;
        current.revision = revision;
        current.output = Some(output);
        Ok(revision)
    }

    pub fn latest_revision(&self) -> Result<u64, StorySequencerOutputStoreError> {
        self.0
            .read()
            .map(|current| current.revision)
            .map_err(|_| StorySequencerOutputStoreError::Poisoned)
    }

    pub fn peek_after(
        &self,
        after_revision: u64,
    ) -> Result<Option<(u64, StorySequencerOutput)>, StorySequencerOutputStoreError> {
        let current = self
            .0
            .read()
            .map_err(|_| StorySequencerOutputStoreError::Poisoned)?;
        if current.revision <= after_revision {
            return Ok(None);
        }
        Ok(current
            .output
            .as_ref()
            .map(|output| (current.revision, output.clone())))
    }

    pub fn consume_revision(&self, revision: u64) -> Result<bool, StorySequencerOutputStoreError> {
        let mut current = self
            .0
            .write()
            .map_err(|_| StorySequencerOutputStoreError::Poisoned)?;
        if current.revision != revision || current.output.is_none() {
            return Ok(false);
        }
        current.output = None;
        Ok(true)
    }
}

static STORY_SEQUENCER_OUTPUTS: OnceLock<StorySequencerOutputStore> = OnceLock::new();
static STORY_GAMEPLAY_INPUTS: OnceLock<StoryGameplayInputsStore> = OnceLock::new();
static STORY_GAMEPLAY_INPUT_SESSION: OnceLock<RwLock<Option<StoryGameplayInputSession>>> =
    OnceLock::new();
static STORY_PLAYER_ENGINE_STATES: OnceLock<Mutex<StoryPlayerEngineStateStore>> = OnceLock::new();
static STORY_MISSION_RESULTS: OnceLock<StoryMissionResultStore> = OnceLock::new();
static STORY_DISC_VFS: OnceLock<Mutex<StoryDiscVfsState>> = OnceLock::new();
static STORY_OWNER_ATTEMPTED_REVISION: AtomicU64 = AtomicU64::new(0);

/// Engine state owned by one native Story player generation.
///
/// Input channels resolve to a `StoryPlayerHandle`; keeping the corresponding `PlayerState`
/// under the same handle prevents a caller from accidentally applying one player's input to a
/// different mutable engine state. The session and handles are Rust publication identities,
/// not recovered guest generations or simulation tick counters.
#[derive(Debug, Default)]
pub struct StoryPlayerEngineStateStore {
    session: Option<StoryGameplayInputSession>,
    players: HashMap<StoryPlayerHandle, PlayerState>,
}

impl StoryPlayerEngineStateStore {
    /// Replace the owner epoch and discard every old player's engine state.
    pub fn begin_session(&mut self, session: StoryGameplayInputSession) {
        if self.session != Some(session) {
            self.players.clear();
            self.session = Some(session);
        }
    }

    /// Register one typed Story player and its engine state under the same lifecycle handle.
    /// The active mission session is checked before the input store is mutated; replacing a slot
    /// retires the prior generation's engine state. This does not infer spawn pose, player
    /// identity, controller policy, or simulation cadence.
    pub fn bind_story_player(
        &mut self,
        inputs: &StoryGameplayInputsStore,
        session: StoryGameplayInputSession,
        slot: u32,
        story_state: StoryPlayerState,
        engine_state: PlayerState,
    ) -> Result<StoryPlayerHandle, StoryGameplayOwnerStoreError> {
        if self.session != Some(session) {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                tsfp_gameplay::story_events::native_world::StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        let handle = inputs.bind_player(session, slot, story_state)?;
        self.players.retain(|current, _| current.slot() != slot);
        self.players.insert(handle, engine_state);
        Ok(handle)
    }

    /// Bind engine state only to a current player from the same mission input store.
    /// Rebinding a player slot retires the previous generation's engine state.
    pub fn bind_player_state(
        &mut self,
        inputs: &StoryGameplayInputsStore,
        session: StoryGameplayInputSession,
        handle: StoryPlayerHandle,
        state: PlayerState,
    ) -> Result<bool, StoryGameplayOwnerStoreError> {
        if self.session != Some(session) {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                tsfp_gameplay::story_events::native_world::StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        if inputs.player_input_for_step(session, handle)?.is_none() {
            return Ok(false);
        }
        self.players
            .retain(|current, _| current.slot() != handle.slot());
        self.players.insert(handle, state);
        Ok(true)
    }

    /// Remove engine state only for its current mission capability and exact player generation.
    pub fn unbind_player_state(
        &mut self,
        session: StoryGameplayInputSession,
        handle: StoryPlayerHandle,
    ) -> Result<Option<PlayerState>, StoryGameplayOwnerStoreError> {
        if self.session != Some(session) {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                tsfp_gameplay::story_events::native_world::StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        Ok(self.players.remove(&handle))
    }

    pub fn player_state(
        &self,
        session: StoryGameplayInputSession,
        handle: StoryPlayerHandle,
    ) -> Option<&PlayerState> {
        (self.session == Some(session))
            .then(|| self.players.get(&handle))
            .flatten()
    }

    pub fn player_state_mut(
        &mut self,
        session: StoryGameplayInputSession,
        handle: StoryPlayerHandle,
    ) -> Option<&mut PlayerState> {
        (self.session == Some(session))
            .then(|| self.players.get_mut(&handle))
            .flatten()
    }

    /// Apply the channel sample only to the engine state stored under that exact player handle.
    /// This selects no tick, edge lifetime, or controller-to-player policy.
    pub fn attach_channel_input_for_step(
        &mut self,
        inputs: &StoryGameplayInputsStore,
        session: StoryGameplayInputSession,
        channel: u8,
        handle: StoryPlayerHandle,
    ) -> Result<bool, StoryGameplayOwnerStoreError> {
        if self.session != Some(session) {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                tsfp_gameplay::story_events::native_world::StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        let Some(player) = self.players.get_mut(&handle) else {
            return Ok(false);
        };
        attach_story_player_channel_input_for_step(inputs, session, channel, handle, player)
    }
}

mod player_motion_input;

/// Shared Rust-owned player state for the active Story mission.
pub fn story_player_engine_state_store() -> &'static Mutex<StoryPlayerEngineStateStore> {
    STORY_PLAYER_ENGINE_STATES.get_or_init(|| Mutex::new(StoryPlayerEngineStateStore::default()))
}

const MAX_PENDING_STORY_MISSION_RESULTS: usize = 32;

/// One terminal mission result retained until an owner consumes and acknowledges its revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryMissionResultEnvelope {
    pub revision: u64,
    /// Mission input-session capability that produced this terminal outcome.
    session: StoryGameplayInputSession,
    pub result: StoryMissionResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryMissionResultStoreError {
    Poisoned,
    Full,
    MissingSession,
    RevisionExhausted,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StoryMissionResultProcessError<E> {
    Store(StoryMissionResultStoreError),
    Consumer(E),
    Superseded,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StoryMissionResultProcess<T> {
    Empty,
    Deferred,
    SessionMismatch {
        expected: StoryGameplayInputSession,
        actual: StoryGameplayInputSession,
    },
    Completed(T),
}

#[derive(Default)]
struct StoryMissionResultStoreState {
    revision: u64,
    pending: VecDeque<StoryMissionResultEnvelope>,
}

/// Rust-owned handoff for terminal Story results. Each result retains the mission input-session
/// capability that produced it; consumers must present that same capability before commit. The
/// queue head remains intact on a session mismatch or failed authenticated consumer.
#[derive(Default)]
pub struct StoryMissionResultStore(Mutex<StoryMissionResultStoreState>, Mutex<()>);

impl StoryMissionResultStore {
    pub fn publish(
        &self,
        session: StoryGameplayInputSession,
        result: StoryMissionResult,
    ) -> Result<u64, StoryMissionResultStoreError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| StoryMissionResultStoreError::Poisoned)?;
        if state.pending.len() >= MAX_PENDING_STORY_MISSION_RESULTS {
            return Err(StoryMissionResultStoreError::Full);
        }
        let revision = state
            .revision
            .checked_add(1)
            .ok_or(StoryMissionResultStoreError::RevisionExhausted)?;
        state.revision = revision;
        state.pending.push_back(StoryMissionResultEnvelope {
            revision,
            session,
            result,
        });
        Ok(revision)
    }

    /// Return the earliest still-pending result newer than the caller's acknowledged revision.
    pub fn peek_after(
        &self,
        revision: u64,
    ) -> Result<Option<StoryMissionResultEnvelope>, StoryMissionResultStoreError> {
        let state = self
            .0
            .lock()
            .map_err(|_| StoryMissionResultStoreError::Poisoned)?;
        Ok(state
            .pending
            .iter()
            .find(|entry| entry.revision > revision)
            .cloned())
    }

    /// Remove only the queue head. A failed or stale consumer leaves all results available.
    pub fn consume_revision(&self, revision: u64) -> Result<bool, StoryMissionResultStoreError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| StoryMissionResultStoreError::Poisoned)?;
        if state.pending.front().map(|entry| entry.revision) != Some(revision) {
            return Ok(false);
        }
        state.pending.pop_front();
        Ok(true)
    }

    /// Serialize result consumers and acknowledge the queue head only after their transaction
    /// returns a completed value. `Ok(None)` defers the head (for example, while profile saving
    /// is disabled); a different mission session returns `SessionMismatch` without invoking the
    /// consumer. Callback errors and superseded revisions preserve the queue head.
    pub fn process_next<T, E>(
        &self,
        session: StoryGameplayInputSession,
        process: impl FnOnce(&StoryMissionResultEnvelope) -> Result<Option<T>, E>,
    ) -> Result<StoryMissionResultProcess<T>, StoryMissionResultProcessError<E>> {
        let _consumer = self.1.lock().map_err(|_| {
            StoryMissionResultProcessError::Store(StoryMissionResultStoreError::Poisoned)
        })?;
        let envelope = self
            .peek_after(0)
            .map_err(StoryMissionResultProcessError::Store)?;
        let Some(envelope) = envelope else {
            return Ok(StoryMissionResultProcess::Empty);
        };
        if envelope.session != session {
            return Ok(StoryMissionResultProcess::SessionMismatch {
                expected: session,
                actual: envelope.session,
            });
        }
        let Some(value) = process(&envelope).map_err(StoryMissionResultProcessError::Consumer)?
        else {
            return Ok(StoryMissionResultProcess::Deferred);
        };
        if !self
            .consume_revision(envelope.revision)
            .map_err(StoryMissionResultProcessError::Store)?
        {
            return Err(StoryMissionResultProcessError::Superseded);
        }
        Ok(StoryMissionResultProcess::Completed(value))
    }
}

/// Shared terminal-result handoff written by the installed Story owner.
pub fn story_mission_result_store() -> &'static StoryMissionResultStore {
    STORY_MISSION_RESULTS.get_or_init(StoryMissionResultStore::default)
}

fn publish_story_mission_effects(
    store: &StoryMissionResultStore,
    session: Option<StoryGameplayInputSession>,
    effects: Vec<StoryWorldEffect>,
) -> Result<usize, StoryMissionResultStoreError> {
    let mut published = 0;
    for effect in effects {
        if let StoryWorldEffect::MissionOutcome(result) = effect {
            let session = session.ok_or(StoryMissionResultStoreError::MissingSession)?;
            store.publish(session, result)?;
            published += 1;
        }
    }
    Ok(published)
}

enum StoryDiscVfsState {
    Unconfigured,
    Pending(PathBuf),
    Ready(Vfs),
    Unavailable,
}

/// Bind the Story asset reader to the same disc path that the host will pass to the guest.
/// Opening the image is lazy so panel-only runs without a disc remain supported.
pub fn init(arguments: &[String]) {
    let path = host_disc_path(arguments)
        .map_or(StoryDiscVfsState::Unconfigured, StoryDiscVfsState::Pending);
    let _ = STORY_DISC_VFS.set(Mutex::new(path));
}

fn host_disc_path(arguments: &[String]) -> Option<PathBuf> {
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == "--disc" {
            if let Some(path) = arguments.get(index + 1) {
                return Some(PathBuf::from(path));
            }
        } else if let Some(path) = arguments[index].strip_prefix("--disc=") {
            return Some(PathBuf::from(path));
        }
        index += 1;
    }
    std::env::var_os("TSFP_DISC").map(PathBuf::from)
}

fn with_story_disc_vfs<T>(operation: impl FnOnce(&mut Vfs) -> T) -> Result<Option<T>, NtStatus> {
    let Some(state) = STORY_DISC_VFS.get() else {
        return Ok(None);
    };
    let mut state = state.lock().map_err(|_| status::INVALID_PARAMETER)?;
    let pending_path = match &*state {
        StoryDiscVfsState::Pending(path) => Some(path.clone()),
        _ => None,
    };
    if let Some(path) = pending_path {
        match Xiso::open(&path) {
            Ok(disc) => {
                let mut vfs = Vfs::new();
                vfs.mount_disc("D:", disc);
                *state = StoryDiscVfsState::Ready(vfs);
            }
            Err(_) => {
                *state = StoryDiscVfsState::Unavailable;
                return Err(status::UNRECOGNIZED_VOLUME);
            }
        }
    }
    match &mut *state {
        StoryDiscVfsState::Ready(vfs) => Ok(Some(operation(vfs))),
        StoryDiscVfsState::Unconfigured | StoryDiscVfsState::Unavailable => Ok(None),
        StoryDiscVfsState::Pending(_) => unreachable!("pending disc path was initialized above"),
    }
}

/// The current REA set establishes only the first playable Story row's asset mapping.
/// Other rows remain pending until their PAK identities are measured.
fn first_story_asset_selection(
    output: &StorySequencerOutput,
) -> Option<(String, SetuAssetSelector, BuildOptions)> {
    let setup = output.setup();
    let bytes = setup.bytes();
    if output.campaign_ordinal().get() != 0
        || bytes[0x54] != 2
        || bytes[0x55] != MODE_STORY
        || bytes[0x57] > 2
    {
        return None;
    }
    Some((
        "D:\\pak\\story\\l_2_st.pak".to_string(),
        SetuAssetSelector {
            key: 0x4afe_3652,
            occurrence: 0,
        },
        BuildOptions::story(bytes[0x57], 0),
    ))
}

fn try_install_pending_story_owner() {
    let outputs = story_sequencer_output_store();
    let Ok(revision) = outputs.latest_revision() else {
        return;
    };
    if revision == 0 || revision <= STORY_OWNER_ATTEMPTED_REVISION.load(Ordering::Acquire) {
        return;
    }
    let inputs = story_gameplay_inputs_store().clone();
    let result = with_story_disc_vfs(|vfs| {
        StoryMissionHostOwner::replace_from_sequencer_output_store(
            vfs,
            outputs,
            revision.saturating_sub(1),
            first_story_asset_selection,
            TerminalState::new(Vec::new(), Vec::new()),
            inputs.clone(),
            |effects| {
                let result = publish_story_mission_effects(
                    story_mission_result_store(),
                    story_gameplay_input_session(),
                    effects,
                );
                if let Err(error) = result {
                    eprintln!("tsfp-bevy: Story result handoff deferred: {error:?}");
                }
            },
        )
    });
    match result {
        Ok(Some(Ok(Some((installed_revision, detached))))) => {
            STORY_OWNER_ATTEMPTED_REVISION.store(installed_revision, Ordering::Release);
            let session = match inputs.begin_session() {
                Ok(session) => match story_player_engine_state_store().lock() {
                    Ok(mut states) => {
                        states.begin_session(session);
                        Some(session)
                    }
                    Err(_) => {
                        eprintln!("tsfp-bevy: Story player engine-state lock poisoned");
                        None
                    }
                },
                Err(error) => {
                    eprintln!("tsfp-bevy: Story input reset deferred: {error:?}");
                    None
                }
            };
            match STORY_GAMEPLAY_INPUT_SESSION
                .get_or_init(|| RwLock::new(None))
                .write()
            {
                Ok(mut active_session) => *active_session = session,
                Err(_) => eprintln!("tsfp-bevy: Story input session lock poisoned"),
            }
            drop(detached);
        }
        Ok(Some(Ok(None))) | Ok(None) => {
            STORY_OWNER_ATTEMPTED_REVISION.store(revision, Ordering::Release);
        }
        Ok(Some(Err(error))) => {
            eprintln!("tsfp-bevy: Story owner install deferred: {error:?}");
            STORY_OWNER_ATTEMPTED_REVISION.store(revision, Ordering::Release);
        }
        Err(error) => {
            eprintln!("tsfp-bevy: Story disc unavailable: {error:?}");
            STORY_OWNER_ATTEMPTED_REVISION.store(revision, Ordering::Release);
        }
    }
}

/// Shared output store filled synchronously by the game-thread ports callback.
pub fn story_sequencer_output_store() -> &'static StorySequencerOutputStore {
    STORY_SEQUENCER_OUTPUTS.get_or_init(StorySequencerOutputStore::default)
}

/// Shared Rust-owned input boundary for the installed Story mission.
///
/// Native gameplay systems publish one complete snapshot per fixed tick. The store is cleared
/// after a new owner installs, so a later mission cannot consume the previous mission's world.
/// An absent or stale snapshot keeps the host callback on the lifted path.
pub fn story_gameplay_inputs_store() -> &'static StoryGameplayInputsStore {
    STORY_GAMEPLAY_INPUTS.get_or_init(StoryGameplayInputsStore::default)
}

/// Return the current Story input capability for a producer binding to the active mission.
/// Producers should retain this token for that mission's lifetime; a later replacement makes it
/// stale, so delayed writes from the previous world cannot seed the next mission.
pub fn story_gameplay_input_session() -> Option<StoryGameplayInputSession> {
    STORY_GAMEPLAY_INPUT_SESSION
        .get_or_init(|| RwLock::new(None))
        .read()
        .ok()
        .and_then(|session| *session)
}

/// Publish a complete typed Story world snapshot from native Rust gameplay owners.
///
/// Every domain, including empty domains, must come from its Rust owner. This API has no guest
/// memory fallback and requires the capability acquired for the active mission. It does not
/// select Native execution; the host still requires an explicit execution selection before it
/// advances the Rust mission.
pub fn publish_story_gameplay_inputs(
    session: StoryGameplayInputSession,
    inputs: StoryGameplayInputs,
) -> Result<(), tsfp_gameplay::story_events::native_world::StoryGameplayInputsStoreError> {
    story_gameplay_inputs_store().publish(session, inputs)
}

/// Bind a Bevy input-router channel to the native Story player selected by its owner.
pub fn bind_story_player_input_channel(
    session: StoryGameplayInputSession,
    channel: u8,
    handle: StoryPlayerHandle,
) -> Result<bool, StoryGameplayOwnerStoreError> {
    story_gameplay_inputs_store().bind_player_input_channel(session, channel, handle)
}

/// Publish one four-channel router sample at a caller-selected native simulation-step boundary.
/// This bridge preserves explicit player bindings and does not schedule or select a player.
pub fn publish_story_player_inputs_for_step(
    session: StoryGameplayInputSession,
    inputs: [Option<tsfp_input::player::PlayerInput>; 4],
) -> Result<(), StoryGameplayOwnerStoreError> {
    story_gameplay_inputs_store().publish_player_inputs_for_step(session, inputs)
}

/// Copy a caller-selected Story player's current-step input into Rust engine state.
///
/// The caller supplies both the active mission capability and generation-bound player handle;
/// `PlayerInput::slot` remains source metadata and is not used to choose the player. A live
/// player with no input clears the engine value, while a stale handle leaves it untouched.
pub fn attach_story_player_input_for_step(
    inputs: &StoryGameplayInputsStore,
    session: StoryGameplayInputSession,
    handle: StoryPlayerHandle,
    player: &mut PlayerState,
) -> Result<bool, StoryGameplayOwnerStoreError> {
    let Some(input) = inputs.player_input_for_step(session, handle)? else {
        return Ok(false);
    };
    player.set_input_for_step(input);
    Ok(true)
}

/// Attach a channel's sample only to the engine state for the same explicitly bound player.
/// A live binding with no sample clears current input; an unbound channel or mismatched handle
/// leaves engine state unchanged.
pub fn attach_story_player_channel_input_for_step(
    inputs: &StoryGameplayInputsStore,
    session: StoryGameplayInputSession,
    channel: u8,
    expected_handle: StoryPlayerHandle,
    player: &mut PlayerState,
) -> Result<bool, StoryGameplayOwnerStoreError> {
    let Some((bound_handle, input)) = inputs.player_input_for_channel(session, channel)? else {
        return Ok(false);
    };
    if bound_handle != expected_handle {
        return Ok(false);
    }
    player.set_input_for_step(input);
    Ok(true)
}

/// Apply the current Story-owned player input to engine-core's native look state.
///
/// The caller chooses the update cadence and owns the look integration, profile, assists, and
/// timing configuration. This function neither consumes the stored sample nor infers how many
/// simulation or presentation updates may reuse it.
pub fn step_story_player_look_from_current_input<P: tsfp_input::look::LookPolicy>(
    inputs: &StoryGameplayInputsStore,
    session: StoryGameplayInputSession,
    handle: StoryPlayerHandle,
    player: &mut PlayerState,
    integration: &mut tsfp_input::look::LookIntegration<P>,
    profile: tsfp_input::look::LookProfile,
    allowed: tsfp_input::player::AssistMask,
    config: tsfp_input::look::Integration,
) -> Result<bool, StoryGameplayOwnerStoreError> {
    if !attach_story_player_input_for_step(inputs, session, handle, player)? {
        return Ok(false);
    }
    Ok(player.step_look_from_input(integration, profile, allowed, config))
}

/// Resolve one router channel against the expected Story player, then apply that step's sample
/// through engine-core's native look integration. A stale binding or missing sample does not
/// advance look state; the caller still owns player creation and simulation cadence.
pub fn step_story_player_channel_look_for_step<P: tsfp_input::look::LookPolicy>(
    inputs: &StoryGameplayInputsStore,
    session: StoryGameplayInputSession,
    channel: u8,
    expected_handle: StoryPlayerHandle,
    player: &mut PlayerState,
    integration: &mut tsfp_input::look::LookIntegration<P>,
    profile: tsfp_input::look::LookProfile,
    allowed: tsfp_input::player::AssistMask,
    config: tsfp_input::look::Integration,
) -> Result<bool, StoryGameplayOwnerStoreError> {
    if !attach_story_player_channel_input_for_step(
        inputs,
        session,
        channel,
        expected_handle,
        player,
    )? {
        return Ok(false);
    }
    Ok(player.step_look_from_input(integration, profile, allowed, config))
}

/// Receive one setup block from the no_std ports lane and copy it into Rust-owned state.
/// The producer guarantees `setup` addresses `BLOCK_BYTES` valid bytes until this call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_bevy_story_sequencer_output(
    campaign_ordinal: u32,
    setup: *const u8,
) -> u32 {
    if setup.is_null() {
        return 0;
    }
    let mut bytes = [0; tsfp_gameplay::modes::story_sequencer::BLOCK_BYTES];
    // SAFETY: the ports callback contract supplies one complete setup block synchronously.
    unsafe { core::ptr::copy_nonoverlapping(setup, bytes.as_mut_ptr(), bytes.len()) };
    let Ok(cursor) = i32::try_from(campaign_ordinal) else {
        return 0;
    };
    let setup = ModeBlock::from_bytes(bytes);
    let Some(output) = StorySequencerOutput::from_result(
        tsfp_gameplay::modes::story_sequencer::SequencerState {
            cursor,
            phase: tsfp_gameplay::modes::story_sequencer::Phase::Level,
        },
        setup,
    ) else {
        return 0;
    };
    story_sequencer_output_store()
        .publish(output)
        .map_or(0, |_| 1)
}

/// Runtime owner for the typed Story mission and its default-off lifted/native schedule.
/// The caller selects mission data and supplies typed session/terminal state at construction;
/// no state is inferred from guest memory here.
pub struct StoryMissionRuntime {
    mission: StoryMission,
    schedule: StorySchedule,
}

/// Typed game-world owner attached to the C2R mode tick. Implementations refresh only Rust-owned
/// gameplay state and report whether this tick has all facts required for native execution.
/// Return `false` when any required value is missing; the C2R callback then preserves lifted
/// execution without advancing the native mission.
pub trait StoryMissionWorldOwner: Send {
    fn refresh(&mut self, world: &mut NativeStoryWorld) -> bool;
    fn apply_effects(&mut self, effects: Vec<StoryWorldEffect>);
}

/// Adapter for neighboring native Rust systems to supply complete Story inputs and consume
/// effects. A missing snapshot declines the tick; effects are handed to the sink exactly once.
/// The callbacks must be backed by Rust-owned typed systems before selecting `Native`.
pub struct TypedStoryMissionWorldOwner {
    snapshot: Box<dyn FnMut() -> Option<StoryGameplayInputs> + Send>,
    effect_sink: Box<dyn FnMut(Vec<StoryWorldEffect>) + Send>,
}

impl TypedStoryMissionWorldOwner {
    pub fn new(
        snapshot: impl FnMut() -> Option<StoryGameplayInputs> + Send + 'static,
        effect_sink: impl FnMut(Vec<StoryWorldEffect>) + Send + 'static,
    ) -> Self {
        Self {
            snapshot: Box::new(snapshot),
            effect_sink: Box::new(effect_sink),
        }
    }

    /// Read atomic, complete snapshots published by the neighboring Rust gameplay owners.
    /// Lock failure or an unpublished frame declines this tick so the C2R host retains the lifted
    /// path; the native schedule never treats missing state as an empty world.
    pub fn from_store(
        store: StoryGameplayInputsStore,
        effect_sink: impl FnMut(Vec<StoryWorldEffect>) + Send + 'static,
    ) -> Self {
        let mut seen_revision = 0;
        Self::new(
            move || {
                let (revision, inputs) = store.snapshot_after(seen_revision).ok().flatten()?;
                seen_revision = revision;
                Some(inputs)
            },
            effect_sink,
        )
    }
}

impl StoryMissionWorldOwner for TypedStoryMissionWorldOwner {
    fn refresh(&mut self, world: &mut NativeStoryWorld) -> bool {
        let Some(inputs) = (self.snapshot)() else {
            return false;
        };
        inputs.publish_into(world);
        true
    }

    fn apply_effects(&mut self, effects: Vec<StoryWorldEffect>) {
        if !effects.is_empty() {
            (self.effect_sink)(effects);
        }
    }
}

/// One installed native owner for the game thread's 60 Hz Story mode callback.
pub struct StoryMissionHostOwner {
    runtime: StoryMissionRuntime,
    world: NativeStoryWorld,
    world_owner: Box<dyn StoryMissionWorldOwner>,
    mission_outcome_forwarded: bool,
}

impl StoryMissionHostOwner {
    pub fn new(
        runtime: StoryMissionRuntime,
        world: NativeStoryWorld,
        world_owner: impl StoryMissionWorldOwner + 'static,
    ) -> Self {
        Self {
            runtime,
            world,
            world_owner: Box::new(world_owner),
            mission_outcome_forwarded: false,
        }
    }

    /// Report the selected lane. New owners start on the lifted path.
    pub fn execution(&self) -> StoryExecution {
        self.runtime.execution()
    }

    /// Resolve the active setup's measured Story controller for the native host dispatcher.
    /// The returned callback addresses remain reference metadata; this accessor never calls
    /// guest code and returns `None` for unsupported levels or non-Story setups.
    pub fn controller(&self) -> Option<StoryController> {
        self.runtime.mission().controller()
    }

    /// Select a lane before installing this owner at the host boundary. Native selection stays
    /// an explicit caller decision until the Story replay-adoption gates are satisfied.
    pub fn set_execution(&mut self, execution: StoryExecution) {
        self.runtime.set_execution(execution);
    }

    /// Build the host attachment from the selected runtime PAK row and authoritative typed
    /// session inputs. A successfully built owner retains the default Lifted selector; callers
    /// may opt into Native only after their world owner can supply a complete snapshot.
    pub fn load_from_vfs(
        vfs: &mut Vfs,
        pak_path: &str,
        selector: SetuAssetSelector,
        flow: StoryFlow,
        options: BuildOptions,
        terminals: TerminalState,
        world: NativeStoryWorld,
        world_owner: impl StoryMissionWorldOwner + 'static,
    ) -> Result<Self, RuntimeStoryMissionError> {
        let runtime = StoryMissionRuntime::load(vfs, pak_path, selector, flow, options, terminals)?;
        Ok(Self::new(runtime, world, world_owner))
    }

    /// Load the owner from a complete setup block returned by the typed Story sequencer.
    pub fn load_from_vfs_sequenced(
        vfs: &mut Vfs,
        pak_path: &str,
        selector: SetuAssetSelector,
        flow: StoryFlow,
        setup_block: &ModeBlock,
        options: BuildOptions,
        terminals: TerminalState,
        world: NativeStoryWorld,
        world_owner: impl StoryMissionWorldOwner + 'static,
    ) -> Result<Self, RuntimeStoryMissionError> {
        let runtime = StoryMissionRuntime::load_from_sequencer(
            vfs,
            pak_path,
            selector,
            flow,
            setup_block,
            options,
            terminals,
        )?;
        Ok(Self::new(runtime, world, world_owner))
    }

    /// Load a sequenced mission and attach it to the shared, revisioned gameplay snapshot
    /// producer. This is the production-facing factory for native Rust systems that publish
    /// complete Story inputs; until both a snapshot and an explicit Native selection exist,
    /// the host callback preserves the lifted tick.
    #[allow(clippy::too_many_arguments)]
    pub fn load_from_vfs_sequenced_store(
        vfs: &mut Vfs,
        pak_path: &str,
        selector: SetuAssetSelector,
        flow: StoryFlow,
        setup_block: &ModeBlock,
        options: BuildOptions,
        terminals: TerminalState,
        inputs: StoryGameplayInputsStore,
        effect_sink: impl FnMut(Vec<StoryWorldEffect>) + Send + 'static,
    ) -> Result<Self, RuntimeStoryMissionError> {
        Self::load_from_vfs_sequenced(
            vfs,
            pak_path,
            selector,
            flow,
            setup_block,
            options,
            terminals,
            NativeStoryWorld::default(),
            TypedStoryMissionWorldOwner::from_store(inputs, effect_sink),
        )
    }

    /// Build from a revisioned sequencer handoff, consuming it only after VFS/SETU construction
    /// succeeds. A malformed or missing PAK entry remains retryable. If another session is
    /// published while this candidate is loading, fail closed and leave the newer revision intact.
    #[allow(clippy::too_many_arguments)]
    pub fn load_from_selection_store(
        vfs: &mut Vfs,
        pak_path: &str,
        selections: &StoryMissionSelectionStore,
        after_revision: u64,
        options: BuildOptions,
        terminals: TerminalState,
        inputs: StoryGameplayInputsStore,
        effect_sink: impl FnMut(Vec<StoryWorldEffect>) + Send + 'static,
    ) -> Result<Option<(u64, Self)>, RuntimeStoryMissionError> {
        let Some((revision, selection)) = selections
            .peek_after(after_revision)
            .map_err(RuntimeStoryMissionError::SelectionStore)?
        else {
            return Ok(None);
        };
        let candidate = Self::load_from_vfs_sequenced_store(
            vfs,
            pak_path,
            selection.selector(),
            selection.flow.clone(),
            selection.setup(),
            options,
            terminals,
            inputs,
            effect_sink,
        )?;
        if !selections
            .consume_revision(revision)
            .map_err(RuntimeStoryMissionError::SelectionStore)?
        {
            return Err(RuntimeStoryMissionError::SelectionSuperseded);
        }
        Ok(Some((revision, candidate)))
    }

    /// Build the host owner directly from a completed sequencer callback. The campaign row and
    /// setup stay paired; the caller supplies the PAK selector because REA has not established
    /// a row-to-SETU mapping. The output remains available after selector validation or VFS/SETU
    /// failure, and is consumed only after the complete lifted-default owner has been built.
    #[allow(clippy::too_many_arguments)]
    pub fn load_from_sequencer_output_store(
        vfs: &mut Vfs,
        pak_path: &str,
        outputs: &StorySequencerOutputStore,
        after_revision: u64,
        selector: SetuAssetSelector,
        options: BuildOptions,
        terminals: TerminalState,
        inputs: StoryGameplayInputsStore,
        effect_sink: impl FnMut(Vec<StoryWorldEffect>) + Send + 'static,
    ) -> Result<Option<(u64, Self)>, RuntimeStoryMissionError> {
        let Some((revision, output)) = outputs
            .peek_after(after_revision)
            .map_err(RuntimeStoryMissionError::SequencerOutputStore)?
        else {
            return Ok(None);
        };
        let selection = StoryMissionSelection::from_typed_sequencer_output(output, selector)?;
        let candidate = Self::load_from_vfs_sequenced_store(
            vfs,
            pak_path,
            selection.selector(),
            selection.flow().clone(),
            selection.setup(),
            options,
            terminals,
            inputs,
            effect_sink,
        )?;
        if !outputs
            .consume_revision(revision)
            .map_err(RuntimeStoryMissionError::SequencerOutputStore)?
        {
            return Err(RuntimeStoryMissionError::SelectionSuperseded);
        }
        Ok(Some((revision, candidate)))
    }

    /// Build and replace the process-wide owner from one completed sequencer result.
    ///
    /// The output revision is acknowledged in the same critical section that replaces the active
    /// owner. A failed VFS load, a superseding sequencer publication, or a poisoned owner lock
    /// therefore leaves the pending output available and the current owner installed. The new
    /// owner starts on the lifted execution lane; Native remains an explicit later choice.
    #[allow(clippy::too_many_arguments)]
    pub fn replace_from_sequencer_output_store(
        vfs: &mut Vfs,
        outputs: &StorySequencerOutputStore,
        after_revision: u64,
        resolve_asset: impl FnOnce(
            &StorySequencerOutput,
        ) -> Option<(String, SetuAssetSelector, BuildOptions)>,
        terminals: TerminalState,
        inputs: StoryGameplayInputsStore,
        effect_sink: impl FnMut(Vec<StoryWorldEffect>) + Send + 'static,
    ) -> Result<Option<(u64, Option<Self>)>, RuntimeStoryMissionError> {
        Self::replace_from_sequencer_output_store_in_slot(
            vfs,
            outputs,
            after_revision,
            resolve_asset,
            terminals,
            inputs,
            effect_sink,
            host_owner_slot(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn replace_from_sequencer_output_store_in_slot(
        vfs: &mut Vfs,
        outputs: &StorySequencerOutputStore,
        after_revision: u64,
        resolve_asset: impl FnOnce(
            &StorySequencerOutput,
        ) -> Option<(String, SetuAssetSelector, BuildOptions)>,
        terminals: TerminalState,
        inputs: StoryGameplayInputsStore,
        effect_sink: impl FnMut(Vec<StoryWorldEffect>) + Send + 'static,
        owner_slot: &Mutex<Option<Self>>,
    ) -> Result<Option<(u64, Option<Self>)>, RuntimeStoryMissionError> {
        let Some((revision, output)) = outputs
            .peek_after(after_revision)
            .map_err(RuntimeStoryMissionError::SequencerOutputStore)?
        else {
            return Ok(None);
        };
        let Some((pak_path, selector, options)) = resolve_asset(&output) else {
            return Ok(None);
        };
        let selection = StoryMissionSelection::from_typed_sequencer_output(output, selector)?;
        let candidate = Self::load_from_vfs_sequenced_store(
            vfs,
            &pak_path,
            selection.selector(),
            selection.flow().clone(),
            selection.setup(),
            options,
            terminals,
            inputs,
            effect_sink,
        )?;
        let detached =
            replace_host_owner_for_output_revision(outputs, revision, candidate, owner_slot)?;
        Ok(Some((revision, detached)))
    }

    /// Replace this session from a typed sequencer result.
    ///
    /// The candidate mission is fully loaded and validated before any part of the active owner
    /// changes. A missing or malformed selected SETU therefore leaves the current session,
    /// world snapshot, and world-effect sink intact for a retry. The replacement runtime still
    /// starts on the lifted lane; selecting Native remains a separate explicit decision.
    pub fn replace_session_from_vfs_sequenced(
        &mut self,
        vfs: &mut Vfs,
        pak_path: &str,
        selector: SetuAssetSelector,
        flow: StoryFlow,
        setup_block: &ModeBlock,
        options: BuildOptions,
        terminals: TerminalState,
        world: NativeStoryWorld,
        world_owner: impl StoryMissionWorldOwner + 'static,
    ) -> Result<(), RuntimeStoryMissionError> {
        let replacement = Self::load_from_vfs_sequenced(
            vfs,
            pak_path,
            selector,
            flow,
            setup_block,
            options,
            terminals,
            world,
            world_owner,
        )?;
        self.runtime = replacement.runtime;
        self.world = replacement.world;
        self.world_owner = replacement.world_owner;
        self.mission_outcome_forwarded = false;
        Ok(())
    }

    /// Transactionally replace a sequenced session while retaining the same typed snapshot
    /// publication point and effect sink. Failed PAK/SETU construction leaves the current
    /// mission, lane, world and producer attached.
    #[allow(clippy::too_many_arguments)]
    pub fn replace_session_from_vfs_sequenced_store(
        &mut self,
        vfs: &mut Vfs,
        pak_path: &str,
        selector: SetuAssetSelector,
        flow: StoryFlow,
        setup_block: &ModeBlock,
        options: BuildOptions,
        terminals: TerminalState,
        inputs: StoryGameplayInputsStore,
        effect_sink: impl FnMut(Vec<StoryWorldEffect>) + Send + 'static,
    ) -> Result<(), RuntimeStoryMissionError> {
        let replacement = Self::load_from_vfs_sequenced_store(
            vfs,
            pak_path,
            selector,
            flow,
            setup_block,
            options,
            terminals,
            inputs,
            effect_sink,
        )?;
        *self = replacement;
        Ok(())
    }

    /// Return true only when a selected native owner completed this mode tick.
    fn step_from_host(&mut self) -> bool {
        if self.runtime.execution() != StoryExecution::Native
            || !self.runtime.is_story_gameplay_active()
            || self.runtime.mission().controller().is_none()
        {
            return false;
        }
        // Refresh is a transaction: an incomplete producer may have filled some facts before
        // discovering a missing dependency. Do not let those partial values leak into a later
        // tick that happens to complete successfully.
        let mut refreshed_world = self.world.clone();
        if !self.world_owner.refresh(&mut refreshed_world) {
            return false;
        }
        self.world = refreshed_world;
        let _ = self
            .runtime
            .step_fixed(&mut self.world, StepReport::default);
        let mut effects = self.world.take_effects();
        forward_terminal_mission_outcome(
            self.runtime.mission_mut(),
            &mut self.mission_outcome_forwarded,
            &mut effects,
        );
        self.world_owner.apply_effects(effects);
        true
    }
}

/// Attach the completed typed mission snapshot immediately before the corresponding end request.
/// The outcome is observational input for the match/UI/profile owners; it does not write storage.
fn forward_terminal_mission_outcome(
    mission: &mut StoryMission,
    already_forwarded: &mut bool,
    effects: &mut Vec<StoryWorldEffect>,
) {
    if *already_forwarded || !mission.is_story() {
        return;
    }
    let Some(end_match_at) = effects
        .iter()
        .position(|effect| matches!(effect, StoryWorldEffect::EndMatch))
    else {
        return;
    };
    let event = effects[..end_match_at]
        .iter()
        .rev()
        .find_map(|effect| match effect {
            StoryWorldEffect::MissionEvent(event) => Some(*event),
            _ => None,
        });
    let result = StoryMissionResult {
        flow: *mission.flow(),
        outcome: mission.capture_progress_candidate(event).clone(),
    };
    effects.insert(end_match_at, StoryWorldEffect::MissionOutcome(result));
    *already_forwarded = true;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryMissionHostInstallError {
    AlreadyInstalled,
    LockPoisoned,
}

static HOST_OWNER: OnceLock<Mutex<Option<StoryMissionHostOwner>>> = OnceLock::new();
static HOST_TICK_ACTIVE: AtomicBool = AtomicBool::new(false);

struct HostTickGuard;

impl Drop for HostTickGuard {
    fn drop(&mut self) {
        HOST_TICK_ACTIVE.store(false, Ordering::Release);
    }
}

fn host_owner_slot() -> &'static Mutex<Option<StoryMissionHostOwner>> {
    HOST_OWNER.get_or_init(|| Mutex::new(None))
}

/// Acknowledge an output only while atomically replacing the active owner. The output-store lock
/// is acquired before the host-owner lock; no other path acquires them in the opposite order.
fn replace_host_owner_for_output_revision(
    outputs: &StorySequencerOutputStore,
    revision: u64,
    candidate: StoryMissionHostOwner,
    slot: &Mutex<Option<StoryMissionHostOwner>>,
) -> Result<Option<StoryMissionHostOwner>, RuntimeStoryMissionError> {
    let mut output = outputs.0.write().map_err(|_| {
        RuntimeStoryMissionError::SequencerOutputStore(StorySequencerOutputStoreError::Poisoned)
    })?;
    if output.revision != revision || output.output.is_none() {
        return Err(RuntimeStoryMissionError::SelectionSuperseded);
    }
    let mut active = slot.lock().map_err(|_| {
        RuntimeStoryMissionError::HostOwnerInstall(StoryMissionHostInstallError::LockPoisoned)
    })?;
    output.output = None;
    Ok(active.replace(candidate))
}

/// Install the typed owner before a Story mode tick selects Native. The game thread invokes it
/// synchronously through the C2R callback; installation itself does not enable the native branch.
pub fn install_story_mission_host_owner(
    owner: StoryMissionHostOwner,
) -> Result<(), StoryMissionHostInstallError> {
    let mut slot = host_owner_slot()
        .lock()
        .map_err(|_| StoryMissionHostInstallError::LockPoisoned)?;
    if slot.is_some() {
        return Err(StoryMissionHostInstallError::AlreadyInstalled);
    }
    *slot = Some(owner);
    Ok(())
}

/// Replace the active mission owner at a typed session boundary.
///
/// Callers construct the new runtime from the next selected mission's PAK entry, session flow,
/// and terminal state before replacing the old owner. The returned owner is detached while the
/// mutex is held but is returned for disposal after the lock is released. A newly constructed
/// runtime still selects `Lifted` until its caller explicitly opts into native execution.
pub fn replace_story_mission_host_owner(
    owner: StoryMissionHostOwner,
) -> Result<Option<StoryMissionHostOwner>, StoryMissionHostInstallError> {
    let mut slot = host_owner_slot()
        .lock()
        .map_err(|_| StoryMissionHostInstallError::LockPoisoned)?;
    Ok(slot.replace(owner))
}

/// Detach the active mission owner before leaving Story gameplay.
///
/// The detached owner is returned so dropping it cannot run while the host-owner lock is held.
pub fn clear_story_mission_host_owner()
-> Result<Option<StoryMissionHostOwner>, StoryMissionHostInstallError> {
    let mut slot = host_owner_slot()
        .lock()
        .map_err(|_| StoryMissionHostInstallError::LockPoisoned)?;
    Ok(slot.take())
}

/// C2R mode-tick callback registered by the Bevy game thread before entering tsfp_host_entry.
/// Zero preserves the lifted 0x228870 body; one means the typed owner performed the entire step.
/// The owner is detached while stepping so event effects can replace or clear the next session
/// without re-entering the host-owner mutex. If a replacement arrives during this tick, it wins
/// over the completed owner when the callback returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_bevy_story_mission_host_tick() -> u32 {
    if HOST_TICK_ACTIVE
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        // A nested mode tick is part of the active callback, not a second lifted tick.
        return 1;
    }
    let _tick_guard = HostTickGuard;
    try_install_pending_story_owner();
    let mut owner = match host_owner_slot().lock() {
        Ok(slot) => slot,
        Err(poisoned) => poisoned.into_inner(),
    }
    .take();
    let Some(mut owner) = owner.take() else {
        return 0;
    };
    let handled = owner.step_from_host();
    let displaced = {
        let mut slot = match host_owner_slot().lock() {
            Ok(slot) => slot,
            Err(poisoned) => poisoned.into_inner(),
        };
        if slot.is_none() {
            *slot = Some(owner);
            None
        } else {
            Some(owner)
        }
    };
    drop(displaced);
    handled as u32
}

impl StoryMissionRuntime {
    pub fn from_mission(mission: StoryMission) -> Self {
        Self {
            mission,
            schedule: StorySchedule::default(),
        }
    }

    pub fn load(
        vfs: &mut Vfs,
        pak_path: &str,
        selector: SetuAssetSelector,
        flow: StoryFlow,
        options: BuildOptions,
        terminals: TerminalState,
    ) -> Result<Self, RuntimeStoryMissionError> {
        load_story_mission(vfs, pak_path, selector, flow, options, terminals)
            .map(Self::from_mission)
    }

    /// Select the SETU record through VFS and initialize the typed mission from the sequencer's
    /// runtime setup block. The lifted/native selector still defaults to Lifted.
    pub fn load_from_sequencer(
        vfs: &mut Vfs,
        pak_path: &str,
        selector: SetuAssetSelector,
        flow: StoryFlow,
        setup_block: &ModeBlock,
        options: BuildOptions,
        terminals: TerminalState,
    ) -> Result<Self, RuntimeStoryMissionError> {
        load_story_mission_from_sequencer(
            vfs,
            pak_path,
            selector,
            flow,
            setup_block,
            options,
            terminals,
        )
        .map(Self::from_mission)
    }

    /// Consume one complete typed selection published by the Story sequencer and construct the
    /// gameplay owner from its selected VFS row. Selection validation occurs before publication;
    /// loading still fails closed if the runtime PAK row or SETU payload is invalid.
    pub fn load_selected(
        vfs: &mut Vfs,
        pak_path: &str,
        selection: StoryMissionSelection,
        options: BuildOptions,
        terminals: TerminalState,
    ) -> Result<Self, RuntimeStoryMissionError> {
        Self::load_from_sequencer(
            vfs,
            pak_path,
            selection.selector,
            selection.flow,
            &selection.setup,
            options,
            terminals,
        )
    }

    /// Replace this runtime owner from the setup staged by the typed Story flow and the selected
    /// runtime PAK row. Both setup and SETU data are validated before the active owner changes;
    /// the existing lifted/native execution choice survives the replacement.
    pub fn start_staged_level(
        &mut self,
        vfs: &mut Vfs,
        pak_path: &str,
        selector: SetuAssetSelector,
        options: BuildOptions,
        terminals: TerminalState,
        terminal_input: TerminalPadBank,
    ) -> Result<(), RuntimeStoryMissionError> {
        let flow = *self.mission.flow();
        let execution = self.execution();
        let replacement = load_selected_story_mission(vfs, pak_path, selector, |entry| {
            StoryMission::from_staged_setu_entry(flow, entry, options, terminals, terminal_input)
        })?;
        self.mission = replacement;
        self.schedule.set_execution(execution);
        Ok(())
    }

    /// Accept the typed results continuation and load its next gameplay row when no post-movie
    /// setup is present. Movie routes commit only their staged StoryFlow edge; the movie owner
    /// later completes it through `finish_post_movie_and_start_level`. A rejected gate or failed
    /// PAK/SETU load leaves the current result owner intact for retry.
    pub fn continue_after_results_and_start_level(
        &mut self,
        vfs: &mut Vfs,
        pak_path: &str,
        selector: SetuAssetSelector,
        options: BuildOptions,
        terminals: TerminalState,
        terminal_input: TerminalPadBank,
        gate: StoryContinuationGate,
        route: StoryContinuation,
    ) -> Result<bool, RuntimeStoryMissionError> {
        let mut flow = *self.mission.flow();
        if !flow.continue_after_results(gate, route) {
            return Ok(false);
        }

        if matches!(flow.step, StoryStep::PostMovie { .. }) {
            let accepted = self.mission.continue_after_results(gate, route);
            debug_assert!(accepted, "copied continuation was already admitted");
            return Ok(true);
        }

        let execution = self.execution();
        let replacement = load_selected_story_mission(vfs, pak_path, selector, |entry| {
            StoryMission::from_staged_setu_entry(flow, entry, options, terminals, terminal_input)
        })?;
        self.mission = replacement;
        self.schedule.set_execution(execution);
        Ok(true)
    }

    /// Decode the measured runtime campaign rows and sequencer records, then install the selected
    /// next SETU entry through the guarded VFS transaction. The caller owns row acquisition; a
    /// short table or invalid setup fails before the current results owner is mutated.
    #[allow(clippy::too_many_arguments)]
    pub fn continue_after_runtime_rows_and_start_level(
        &mut self,
        vfs: &mut Vfs,
        pak_path: &str,
        selector: SetuAssetSelector,
        options: BuildOptions,
        terminals: TerminalState,
        terminal_input: TerminalPadBank,
        gate: StoryContinuationGate,
        level_rows: &[u8],
        post_movie: Option<&ModeBlock>,
        next_gameplay: &ModeBlock,
    ) -> Result<bool, RuntimeStoryMissionError> {
        let campaign_levels = CampaignLevelTable::decode_runtime_rows(level_rows)
            .map_err(StoryMissionLoadError::CampaignLevelTable)
            .map_err(RuntimeStoryMissionError::Mission)?;
        self.continue_after_campaign_table_and_start_level(
            vfs,
            pak_path,
            selector,
            options,
            terminals,
            terminal_input,
            gate,
            campaign_levels,
            post_movie,
            next_gameplay,
        )
    }

    /// Install a next level selected from the owned campaign table produced by the runtime
    /// reader. `CampaignLevelTable` is the typed seam between the guest table reader and Story
    /// progression; it contains IDs only and carries no guest pointer or row-layout assumption.
    #[allow(clippy::too_many_arguments)]
    pub fn continue_after_campaign_table_and_start_level(
        &mut self,
        vfs: &mut Vfs,
        pak_path: &str,
        selector: SetuAssetSelector,
        options: BuildOptions,
        terminals: TerminalState,
        terminal_input: TerminalPadBank,
        gate: StoryContinuationGate,
        campaign_levels: CampaignLevelTable,
        post_movie: Option<&ModeBlock>,
        next_gameplay: &ModeBlock,
    ) -> Result<bool, RuntimeStoryMissionError> {
        let route =
            StoryMission::continuation_from_sequencer(campaign_levels, post_movie, next_gameplay)
                .map_err(RuntimeStoryMissionError::Mission)?;
        self.continue_after_results_and_start_level(
            vfs,
            pak_path,
            selector,
            options,
            terminals,
            terminal_input,
            gate,
            route,
        )
    }

    /// Read the measured campaign IDs from guest memory, own them as a typed table, and install
    /// the sequencer-selected next level. Reader faults leave the current results owner untouched.
    #[allow(clippy::too_many_arguments)]
    pub fn continue_after_guest_campaign_table_and_start_level<M: GuestMemory>(
        &mut self,
        vfs: &mut Vfs,
        pak_path: &str,
        selector: SetuAssetSelector,
        options: BuildOptions,
        terminals: TerminalState,
        terminal_input: TerminalPadBank,
        gate: StoryContinuationGate,
        memory: &mut M,
        post_movie: Option<&ModeBlock>,
        next_gameplay: &ModeBlock,
    ) -> Result<bool, RuntimeStoryMissionError> {
        let ids = CutsceneState { memory }
            .story_campaign_level_ids()
            .map_err(RuntimeStoryMissionError::GuestMemory)?;
        self.continue_after_campaign_table_and_start_level(
            vfs,
            pak_path,
            selector,
            options,
            terminals,
            terminal_input,
            gate,
            CampaignLevelTable::new(ids),
            post_movie,
            next_gameplay,
        )
    }

    /// Complete the runtime-supplied post-movie edge and install its staged gameplay level.
    /// The flow is advanced on a copy and committed only after the selected SETU row decodes, so
    /// a missing or malformed row leaves the active owner at the post-movie boundary for retry.
    pub fn finish_post_movie_and_start_level(
        &mut self,
        vfs: &mut Vfs,
        pak_path: &str,
        selector: SetuAssetSelector,
        options: BuildOptions,
        terminals: TerminalState,
        terminal_input: TerminalPadBank,
    ) -> Result<bool, RuntimeStoryMissionError> {
        let mut flow = *self.mission.flow();
        if !flow.finish_post_movie() {
            return Ok(false);
        }
        let execution = self.execution();
        let replacement = load_selected_story_mission(vfs, pak_path, selector, |entry| {
            StoryMission::from_staged_setu_entry(flow, entry, options, terminals, terminal_input)
        })?;
        self.mission = replacement;
        self.schedule.set_execution(execution);
        Ok(true)
    }

    pub fn execution(&self) -> StoryExecution {
        self.schedule.execution()
    }

    pub fn set_execution(&mut self, execution: StoryExecution) {
        self.schedule.set_execution(execution);
    }

    pub fn mission(&self) -> &StoryMission {
        &self.mission
    }

    pub fn mission_mut(&mut self) -> &mut StoryMission {
        &mut self.mission
    }

    /// The shared C2R mode tick also runs in other modes; only an active Story gameplay phase
    /// may be replaced by this mission owner.
    pub fn is_story_gameplay_active(&self) -> bool {
        matches!(self.mission.flow().step, StoryStep::Gameplay { .. })
            && self.mission.flow().active.byte(0x55) == MODE_STORY
    }

    /// Step one fixed gameplay tick through the selected branch. Native execution uses only
    /// the supplied typed world; the caller retains ownership of its lifted fallback.
    pub fn step(
        &mut self,
        dt: f32,
        world: &mut dyn World,
        lifted: impl FnOnce() -> StepReport,
    ) -> StepReport {
        self.schedule.step(dt, &mut self.mission, world, lifted)
    }

    /// Advance one gameplay tick at the fixed 60 Hz simulation cadence. Presentation may call
    /// independently at display rate; only this method advances the mission simulation.
    pub fn step_fixed(
        &mut self,
        world: &mut dyn World,
        lifted: impl FnOnce() -> StepReport,
    ) -> StepReport {
        self.schedule.step_fixed(&mut self.mission, world, lifted)
    }
}

/// Load one mission from the runtime-selected PAK using the caller's current typed session and
/// terminal state. The PAK is opened and closed on every path; malformed or absent data is an
/// explicit error and never becomes an empty mission.
pub fn load_story_mission(
    vfs: &mut Vfs,
    pak_path: &str,
    selector: SetuAssetSelector,
    flow: StoryFlow,
    options: BuildOptions,
    terminals: TerminalState,
) -> Result<StoryMission, RuntimeStoryMissionError> {
    load_selected_story_mission(vfs, pak_path, selector, |entry| {
        StoryMission::from_setu_entry(flow, entry, options, terminals)
    })
}

/// Runtime-data attachment that pairs the selected SETU payload with a typed sequencer block.
pub fn load_story_mission_from_sequencer(
    vfs: &mut Vfs,
    pak_path: &str,
    selector: SetuAssetSelector,
    flow: StoryFlow,
    setup_block: &ModeBlock,
    options: BuildOptions,
    terminals: TerminalState,
) -> Result<StoryMission, RuntimeStoryMissionError> {
    load_selected_story_mission(vfs, pak_path, selector, |entry| {
        StoryMission::from_sequencer_setup(flow, setup_block, entry, options, terminals)
    })
}

fn load_selected_story_mission(
    vfs: &mut Vfs,
    pak_path: &str,
    selector: SetuAssetSelector,
    build: impl FnOnce(&[u8]) -> Result<StoryMission, StoryMissionLoadError>,
) -> Result<StoryMission, RuntimeStoryMissionError> {
    let key = AssetKey {
        key: selector.key,
        occurrence: selector.occurrence,
    };
    let entry = tsfp_engine_fileio::assets::payload(vfs, pak_path, key)
        .map_err(RuntimeStoryMissionError::FileIo)?;
    build(&entry).map_err(RuntimeStoryMissionError::Mission)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex as TestMutex;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tsfp_data_formats::cutscenes::{
        CutsceneState, STORY_CAMPAIGN_LEVEL_COUNT, STORY_CAMPAIGN_LEVEL_IDS,
        STORY_CAMPAIGN_LEVEL_ROW_STRIDE,
    };
    use tsfp_engine_memory::{GuestMemory, MappedMemory};
    use tsfp_gameplay::modes::story_sequencer::BLOCK_BYTES;
    use tsfp_gameplay::story_events::slots::MissionEvent;
    use tsfp_state::story_flow::{CampaignLevelTable, CampaignOrdinal, RESULT_SUCCESS};
    use tsfp_types::GuestAddr;

    static NEXT: AtomicU64 = AtomicU64::new(0);
    static HOST_CALLBACK_TEST_LOCK: TestMutex<()> = TestMutex::new(());

    fn player_input(step: u64, slot: u8) -> tsfp_input::player::PlayerInput {
        use tsfp_input::player::{AssistMask, Look, PlayerInput, SourceKind, Vec2};
        PlayerInput {
            step,
            slot,
            source: SourceKind::Pad(7),
            held: 1,
            pressed: 1,
            released: 0,
            move_axis: Vec2 { x: 0.25, y: 0.75 },
            look: Look::Stick(Vec2 { x: -0.5, y: 0.125 }),
            triggers: [0.0, 1.0],
            menu: Default::default(),
            assists: AssistMask::PROFILE,
        }
    }

    #[test]
    fn story_player_input_adapter_copies_clears_and_respects_lifetimes() {
        use tsfp_gameplay::story_events::native_world::StoryPlayerState;

        let store = StoryGameplayInputsStore::default();
        let session = store.begin_session().unwrap();
        let handle = store
            .bind_player(
                session,
                4,
                StoryPlayerState {
                    character_id: 0x2a,
                    following_link: 0,
                    resource_memberships: Default::default(),
                    first_player_flag: true,
                    health: 1.0,
                    down: false,
                },
            )
            .unwrap();
        let first = player_input(4, 1);
        store
            .set_player_input_for_step(session, handle, Some(first))
            .unwrap();
        let mut player = PlayerState::new(
            tsfp_types::TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            0.0,
        );

        assert!(attach_story_player_input_for_step(&store, session, handle, &mut player).unwrap());
        assert_eq!(player.input_for_step(), Some(first));

        store
            .set_player_input_for_step(session, handle, None)
            .unwrap();
        assert!(attach_story_player_input_for_step(&store, session, handle, &mut player).unwrap());
        assert_eq!(player.input_for_step(), None);

        let replacement = store
            .bind_player(
                session,
                4,
                StoryPlayerState {
                    character_id: 0x2b,
                    following_link: 0,
                    resource_memberships: Default::default(),
                    first_player_flag: true,
                    health: 1.0,
                    down: false,
                },
            )
            .unwrap();
        let sentinel = player_input(9, 3);
        player.set_input_for_step(Some(sentinel));
        assert!(!attach_story_player_input_for_step(&store, session, handle, &mut player).unwrap());
        assert_eq!(player.input_for_step(), Some(sentinel));

        store
            .set_player_input_for_step(session, replacement, Some(player_input(5, 0)))
            .unwrap();
        let next_session = store.begin_session().unwrap();
        assert_ne!(session, next_session);
        assert_eq!(
            attach_story_player_input_for_step(&store, session, replacement, &mut player),
            Err(StoryGameplayOwnerStoreError::Inputs(
                tsfp_gameplay::story_events::native_world::StoryGameplayInputsStoreError::StaleSession
            ))
        );
        assert_eq!(player.input_for_step(), Some(sentinel));
    }

    #[test]
    fn story_player_channel_input_adapter_resolves_and_checks_binding() {
        use tsfp_gameplay::story_events::native_world::StoryPlayerState;

        let store = StoryGameplayInputsStore::default();
        let session = store.begin_session().unwrap();
        let first = store
            .bind_player(
                session,
                4,
                StoryPlayerState {
                    character_id: 0x2a,
                    following_link: 0,
                    resource_memberships: Default::default(),
                    first_player_flag: true,
                    health: 1.0,
                    down: false,
                },
            )
            .unwrap();
        let second = store
            .bind_player(
                session,
                8,
                StoryPlayerState {
                    character_id: 0x2b,
                    following_link: 0,
                    resource_memberships: Default::default(),
                    first_player_flag: false,
                    health: 1.0,
                    down: false,
                },
            )
            .unwrap();
        assert!(store.bind_player_input_channel(session, 0, second).unwrap());

        let selected = player_input(4, 1);
        store
            .publish_player_inputs_for_step(session, [Some(selected), None, None, None])
            .unwrap();
        assert_eq!(
            store.player_input_for_channel(session, 0),
            Ok(Some((second, Some(selected))))
        );
        assert_eq!(store.player_input_for_channel(session, 1), Ok(None));

        let mut first_state = PlayerState::new(
            tsfp_types::TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            0.0,
        );
        let sentinel = player_input(9, 3);
        first_state.set_input_for_step(Some(sentinel));
        assert!(
            !attach_story_player_channel_input_for_step(
                &store,
                session,
                0,
                first,
                &mut first_state,
            )
            .unwrap()
        );
        assert_eq!(first_state.input_for_step(), Some(sentinel));

        let mut second_state = PlayerState::new(
            tsfp_types::TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            0.0,
        );
        assert!(
            attach_story_player_channel_input_for_step(
                &store,
                session,
                0,
                second,
                &mut second_state,
            )
            .unwrap()
        );
        assert_eq!(second_state.input_for_step(), Some(selected));

        store
            .publish_player_inputs_for_step(session, [None, None, None, None])
            .unwrap();
        assert!(
            attach_story_player_channel_input_for_step(
                &store,
                session,
                0,
                second,
                &mut second_state,
            )
            .unwrap()
        );
        assert_eq!(second_state.input_for_step(), None);
    }

    #[test]
    fn story_player_engine_states_bind_input_to_current_generation_and_session() {
        use tsfp_gameplay::story_events::native_world::StoryPlayerState;

        let inputs = StoryGameplayInputsStore::default();
        let session = inputs.begin_session().unwrap();
        let player_facts = |character_id| StoryPlayerState {
            character_id,
            following_link: 0,
            resource_memberships: Default::default(),
            first_player_flag: true,
            health: 1.0,
            down: false,
        };
        let mut engine_states = StoryPlayerEngineStateStore::default();
        engine_states.begin_session(session);
        let first = engine_states
            .bind_story_player(
                &inputs,
                session,
                4,
                player_facts(4),
                PlayerState::new(
                    tsfp_types::TsfpVec3 {
                        x: 10.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    0.0,
                ),
            )
            .unwrap();
        let replacement = engine_states
            .bind_story_player(
                &inputs,
                session,
                4,
                player_facts(5),
                PlayerState::new(
                    tsfp_types::TsfpVec3 {
                        x: 20.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    0.0,
                ),
            )
            .unwrap();
        assert_ne!(first, replacement);
        assert!(engine_states.player_state(session, first).is_none());
        assert_eq!(
            engine_states
                .player_state(session, replacement)
                .map(|state| state.pose.x),
            Some(20.0)
        );
        assert!(
            !engine_states
                .bind_player_state(
                    &inputs,
                    session,
                    first,
                    PlayerState::new(
                        tsfp_types::TsfpVec3 {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                        0.0,
                    ),
                )
                .unwrap()
        );

        assert!(
            inputs
                .bind_player_input_channel(session, 0, replacement)
                .unwrap()
        );
        let selected = player_input(4, 1);
        inputs
            .publish_player_inputs_for_step(session, [Some(selected), None, None, None])
            .unwrap();
        assert!(
            engine_states
                .attach_channel_input_for_step(&inputs, session, 0, replacement)
                .unwrap()
        );
        assert_eq!(
            engine_states
                .player_state(session, replacement)
                .and_then(PlayerState::input_for_step),
            Some(selected)
        );
        assert!(
            !engine_states
                .attach_channel_input_for_step(&inputs, session, 0, first)
                .unwrap()
        );

        let next_session = inputs.begin_session().unwrap();
        engine_states.begin_session(next_session);
        assert!(
            engine_states
                .player_state(next_session, replacement)
                .is_none()
        );
        assert!(
            engine_states
                .bind_story_player(
                    &inputs,
                    session,
                    4,
                    player_facts(6),
                    PlayerState::new(
                        tsfp_types::TsfpVec3 {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                        0.0,
                    ),
                )
                .is_err()
        );
        assert!(
            inputs
                .player_input_for_step(next_session, replacement)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn story_current_input_steps_native_player_look_with_caller_owned_policy() {
        use tsfp_gameplay::story_events::native_world::StoryPlayerState;
        use tsfp_input::look::{LookIntegration, LookProfile, LookState, OriginalLook};
        use tsfp_input::player::{AssistMask, Look};

        let store = StoryGameplayInputsStore::default();
        let session = store.begin_session().unwrap();
        let handle = store
            .bind_player(
                session,
                2,
                StoryPlayerState {
                    character_id: 0x2a,
                    following_link: 0,
                    resource_memberships: Default::default(),
                    first_player_flag: true,
                    health: 1.0,
                    down: false,
                },
            )
            .unwrap();
        let mut selected = player_input(4, 1);
        selected.look = Look::Delta {
            yaw_deg: 4.0,
            pitch_deg: 4.0,
        };
        store
            .set_player_input_for_step(session, handle, Some(selected))
            .unwrap();

        let mut player = PlayerState::new(
            tsfp_types::TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            1.0,
        );
        player.set_look_state(LookState {
            yaw: 1.0,
            pitch: 59.0,
            ramp: 1.0,
            state: 2,
            integration_allowed: true,
            ..Default::default()
        });
        let mut integration = LookIntegration::<OriginalLook> {
            enabled: true,
            ..Default::default()
        };
        let profile = LookProfile {
            flags: 0xff,
            crosshair: 2,
            turn_speed: 0.5,
        };

        assert!(
            step_story_player_look_from_current_input(
                &store,
                session,
                handle,
                &mut player,
                &mut integration,
                profile,
                AssistMask::NONE,
                tsfp_input::look::Integration::default(),
            )
            .unwrap()
        );
        let look = player.look_state().expect("installed native look state");
        assert_eq!(player.input_for_step(), Some(selected));
        assert_eq!(player.facing, look.yaw);
        assert_ne!((look.yaw, look.pitch), (1.0, 59.0));
    }

    #[test]
    fn story_channel_look_steps_only_the_bound_engine_state() {
        use tsfp_gameplay::story_events::native_world::StoryPlayerState;
        use tsfp_input::look::{LookIntegration, LookProfile, LookState, OriginalLook};
        use tsfp_input::player::{AssistMask, Look};

        let store = StoryGameplayInputsStore::default();
        let session = store.begin_session().unwrap();
        let handle = store
            .bind_player(
                session,
                2,
                StoryPlayerState {
                    character_id: 0x2a,
                    following_link: 0,
                    resource_memberships: Default::default(),
                    first_player_flag: true,
                    health: 1.0,
                    down: false,
                },
            )
            .unwrap();
        let other_handle = store
            .bind_player(
                session,
                3,
                StoryPlayerState {
                    character_id: 0x2b,
                    following_link: 0,
                    resource_memberships: Default::default(),
                    first_player_flag: false,
                    health: 1.0,
                    down: false,
                },
            )
            .unwrap();
        assert!(store.bind_player_input_channel(session, 0, handle).unwrap());

        let mut selected_player = PlayerState::new(
            tsfp_types::TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            1.0,
        );
        selected_player.set_look_state(LookState {
            yaw: 1.0,
            pitch: 59.0,
            ramp: 1.0,
            state: 2,
            integration_allowed: true,
            ..Default::default()
        });
        let other_player = PlayerState::new(
            tsfp_types::TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            1.0,
        );
        let mut engine_states = StoryPlayerEngineStateStore::default();
        engine_states.begin_session(session);
        assert!(
            engine_states
                .bind_player_state(&store, session, handle, selected_player)
                .unwrap()
        );
        assert!(
            engine_states
                .bind_player_state(&store, session, other_handle, other_player)
                .unwrap()
        );

        let mut selected = player_input(4, 1);
        selected.look = Look::Delta {
            yaw_deg: 4.0,
            pitch_deg: 4.0,
        };
        store
            .publish_player_inputs_for_step(session, [Some(selected), None, None, None])
            .unwrap();

        let mut integration = LookIntegration::<OriginalLook> {
            enabled: true,
            ..Default::default()
        };
        let profile = LookProfile {
            flags: 0xff,
            crosshair: 2,
            turn_speed: 0.5,
        };

        assert!(
            !engine_states
                .step_channel_look_for_step(
                    &store,
                    session,
                    0,
                    other_handle,
                    &mut integration,
                    profile,
                    AssistMask::NONE,
                    tsfp_input::look::Integration::default(),
                )
                .unwrap()
        );
        let other_player = engine_states
            .player_state(session, other_handle)
            .expect("other bound engine state");
        assert_eq!(other_player.input_for_step(), None);
        assert!(other_player.look_state().is_none());
        let selected_before_step = engine_states
            .player_state(session, handle)
            .expect("selected bound engine state");
        assert_eq!(selected_before_step.input_for_step(), None);
        assert_eq!(
            selected_before_step
                .look_state()
                .map(|look| (look.yaw, look.pitch)),
            Some((1.0, 59.0))
        );

        assert!(
            engine_states
                .step_channel_look_for_step(
                    &store,
                    session,
                    0,
                    handle,
                    &mut integration,
                    profile,
                    AssistMask::NONE,
                    tsfp_input::look::Integration::default(),
                )
                .unwrap()
        );
        let player = engine_states
            .player_state(session, handle)
            .expect("selected bound engine state");
        let look = player.look_state().expect("installed native look state");
        assert_eq!(player.input_for_step(), Some(selected));
        assert_eq!(player.facing, look.yaw);
        assert_ne!((look.yaw, look.pitch), (1.0, 59.0));
    }

    fn empty_setu() -> Vec<u8> {
        let mut entry = vec![0; 0x9c];
        entry[..4].copy_from_slice(b"SETU");
        for (at, value) in [
            (0x84, 0),
            (0x88, 0),
            (0x8c, 0x38),
            (0x90, 0),
            (0x94, 0),
            (0x98, 0xc8),
        ] {
            entry[at..at + 4].copy_from_slice(&value_u32(value));
        }
        entry
    }

    fn story_flow() -> StoryFlow {
        story_flow_for_level(0)
    }

    fn story_flow_for_level(level_id: u8) -> StoryFlow {
        let mut flow = StoryFlow::default();
        flow.staging.set_byte(0x55, MODE_STORY);
        flow.staging.set_byte(0x54, level_id);
        flow.start_level();
        flow
    }

    fn synthetic_host_owner(level_id: u8) -> StoryMissionHostOwner {
        let mission = StoryMission::from_setu_entry(
            story_flow_for_level(level_id),
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        StoryMissionHostOwner::new(
            StoryMissionRuntime::from_mission(mission),
            NativeStoryWorld::default(),
            TypedStoryMissionWorldOwner::from_store(StoryGameplayInputsStore::default(), |_| {}),
        )
    }

    fn campaign_levels() -> CampaignLevelTable {
        CampaignLevelTable::new([2, 10, 12, 4, 6, 8, 14, 16, 18, 20, 22, 24, 26])
    }

    fn guest_campaign_table_bytes() -> Vec<u8> {
        let ids = [2i16, 10, 12, 4, 6, 8, 14, 16, 18, 20, 22, 24, 26];
        let end = STORY_CAMPAIGN_LEVEL_IDS as usize
            + (STORY_CAMPAIGN_LEVEL_COUNT - 1) * STORY_CAMPAIGN_LEVEL_ROW_STRIDE as usize
            + 2;
        let mut bytes = vec![0; end];
        for (index, level_id) in ids.into_iter().enumerate() {
            let address = STORY_CAMPAIGN_LEVEL_IDS + index as u32 * STORY_CAMPAIGN_LEVEL_ROW_STRIDE;
            let start = address as usize;
            bytes[start..start + 2].copy_from_slice(&level_id.to_le_bytes());
        }
        bytes
    }

    fn sequencer_block(active_type: u8, level_id: u8) -> ModeBlock {
        let mut bytes = [0; BLOCK_BYTES];
        bytes[0] = 1;
        bytes[0x54] = level_id;
        bytes[0x55] = active_type;
        ModeBlock::from_bytes(bytes)
    }

    fn value_u32(value: u32) -> [u8; 4] {
        value.to_le_bytes()
    }

    fn pak(entries: &[(u32, &[u8])]) -> Vec<u8> {
        let mut bytes = vec![0; 12];
        bytes[..4].copy_from_slice(b"P5CK");
        let mut rows = Vec::new();
        for (key, payload) in entries {
            let offset = bytes.len() as u32;
            bytes.extend_from_slice(payload);
            rows.push((*key, offset, payload.len() as u32));
        }
        let toc = bytes.len() as u32;
        for (key, offset, size) in rows {
            bytes.extend_from_slice(&key.to_le_bytes());
            bytes.extend_from_slice(&offset.to_le_bytes());
            bytes.extend_from_slice(&size.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
        }
        bytes[4..8].copy_from_slice(&toc.to_le_bytes());
        bytes[8..12].copy_from_slice(&((entries.len() * 16) as u32).to_le_bytes());
        bytes
    }

    #[test]
    fn story_selection_store_publishes_complete_fresh_selections_once() {
        let store = StoryMissionSelectionStore::default();
        let selector = SetuAssetSelector {
            key: 0x1234,
            occurrence: 1,
        };
        assert_eq!(store.take_after(0).unwrap(), None);

        let selection =
            StoryMissionSelection::new(story_flow(), sequencer_block(MODE_STORY, 0x65), selector)
                .unwrap();
        let revision = store.publish(selection.clone()).unwrap();
        assert_eq!(revision, 1);
        assert_eq!(selection.selector(), selector);
        assert_eq!(selection.setup().bytes()[0x55], MODE_STORY);
        assert_eq!(store.take_after(revision).unwrap(), None);
        assert_eq!(store.take_after(0).unwrap(), Some((revision, selection)));
        assert_eq!(store.take_after(0).unwrap(), None);
        assert_eq!(store.take_after(revision).unwrap(), None);
    }

    #[test]
    fn ports_sequencer_callback_copies_setup_and_ordinal_into_owned_store() {
        let store = story_sequencer_output_store();
        let before = store.latest_revision().unwrap();
        let setup = sequencer_block(MODE_STORY, 0x65);

        let accepted = unsafe { tsfp_bevy_story_sequencer_output(4, setup.bytes().as_ptr()) };
        assert_eq!(accepted, 1);
        let (revision, output) = store.peek_after(before).unwrap().unwrap();
        assert_eq!(output.campaign_ordinal().get(), 4);
        assert_eq!(output.setup(), &setup);
        assert!(store.consume_revision(revision).unwrap());
        assert_eq!(store.peek_after(revision).unwrap(), None);

        assert_eq!(
            unsafe { tsfp_bevy_story_sequencer_output(13, setup.bytes().as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { tsfp_bevy_story_sequencer_output(4, std::ptr::null()) },
            0
        );
        assert_eq!(store.peek_after(revision).unwrap(), None);
    }

    #[test]
    fn story_selection_from_sequencer_output_starts_the_owned_flow_at_its_row() {
        let setup = sequencer_block(MODE_STORY, 0x65);
        let selector = SetuAssetSelector::new(0x1234, 1);
        let selection =
            StoryMissionSelection::from_sequencer_output(setup.clone(), 4, selector).unwrap();

        assert_eq!(selection.setup(), &setup);
        assert_eq!(selection.selector(), selector);
        assert_eq!(selection.flow().ordinal.get(), 4);
        assert_eq!(selection.flow().active.byte(0x54), 0x65);
        assert_eq!(selection.flow().active.byte(0x55), MODE_STORY);
        assert_eq!(selection.flow().phase, 2);
        assert_eq!(selection.flow().result_flags, 0);
    }

    #[test]
    fn typed_sequencer_output_publishes_setup_and_campaign_row_together() {
        use tsfp_gameplay::modes::story_sequencer::{Phase, SequencerState};

        let setup = sequencer_block(MODE_STORY, 0x65);
        let output = StorySequencerOutput::from_result(
            SequencerState {
                cursor: 4,
                phase: Phase::Level,
            },
            setup.clone(),
        )
        .unwrap();
        let selector = SetuAssetSelector::new(0x1234, 1);
        let store = StoryMissionSelectionStore::default();

        let revision = store.publish_sequencer_output(output, selector).unwrap();
        let (received_revision, selection) = store.take_after(0).unwrap().unwrap();

        assert_eq!(received_revision, revision);
        assert_eq!(selection.setup(), &setup);
        assert_eq!(selection.selector(), selector);
        assert_eq!(selection.flow().ordinal.get(), 4);
        assert_eq!(selection.flow().active.byte(0x54), 0x65);
        assert_eq!(selection.flow().active.byte(0x55), MODE_STORY);
        assert_eq!(selection.flow().phase, 2);
    }

    #[test]
    fn first_story_asset_mapping_accepts_only_measured_first_gameplay_row() {
        use tsfp_gameplay::modes::story_sequencer::{Phase, SequencerState};

        let output = |cursor, level, difficulty| {
            let mut bytes = *sequencer_block(MODE_STORY, level).bytes();
            bytes[0x57] = difficulty;
            StorySequencerOutput::from_result(
                SequencerState {
                    cursor,
                    phase: Phase::Level,
                },
                ModeBlock::from_bytes(bytes),
            )
            .unwrap()
        };

        let (path, selector, options) = first_story_asset_selection(&output(0, 2, 1)).unwrap();
        assert_eq!(path, "D:\\pak\\story\\l_2_st.pak");
        assert_eq!(selector.key, 0x4afe_3652);
        assert_eq!(selector.occurrence, 0);
        assert_eq!(options.context.difficulty, 1);
        assert!(first_story_asset_selection(&output(0, 0x18, 1)).is_none());
        assert!(first_story_asset_selection(&output(1, 4, 1)).is_none());
        assert!(first_story_asset_selection(&output(0, 2, 3)).is_none());
    }

    #[test]
    fn first_story_host_disc_argument_matches_the_guest_disc_argument() {
        assert_eq!(
            host_disc_path(&["xbe".into(), "--disc".into(), "owner.iso".into()]),
            Some(PathBuf::from("owner.iso"))
        );
        assert_eq!(
            host_disc_path(&["xbe".into(), "--disc=owner.iso".into()]),
            Some(PathBuf::from("owner.iso"))
        );
    }

    #[test]
    fn story_selection_from_sequencer_output_rejects_invalid_row_and_non_story_setup() {
        let selector = SetuAssetSelector::new(0x1234, 0);
        assert_eq!(
            StoryMissionSelection::from_sequencer_output(
                sequencer_block(MODE_STORY, 0x65),
                -1,
                selector,
            ),
            Err(RuntimeStoryMissionError::CampaignOrdinal(-1))
        );
        assert_eq!(
            StoryMissionSelection::from_sequencer_output(
                sequencer_block(MODE_STORY, 0x65),
                13,
                selector,
            ),
            Err(RuntimeStoryMissionError::CampaignOrdinal(13))
        );
        assert_eq!(
            StoryMissionSelection::from_sequencer_output(sequencer_block(9, 0x65), 4, selector,),
            Err(RuntimeStoryMissionError::Mission(
                StoryMissionLoadError::NotStorySetup { actual_mode: 9 }
            ))
        );
    }

    #[test]
    fn story_selection_acknowledgement_is_revision_checked() {
        let store = StoryMissionSelectionStore::default();
        let selector = SetuAssetSelector::new(0x1234, 0);
        let first =
            StoryMissionSelection::new(story_flow(), sequencer_block(MODE_STORY, 0x65), selector)
                .unwrap();
        let first_revision = store.publish(first.clone()).unwrap();

        assert_eq!(store.peek_after(0).unwrap(), Some((first_revision, first)));
        assert!(store.consume_revision(first_revision).unwrap());
        assert_eq!(store.peek_after(0).unwrap(), None);

        let replacement =
            StoryMissionSelection::new(story_flow(), sequencer_block(MODE_STORY, 0x66), selector)
                .unwrap();
        let replacement_revision = store.publish(replacement.clone()).unwrap();
        assert!(!store.consume_revision(first_revision).unwrap());
        assert_eq!(
            store.peek_after(first_revision).unwrap(),
            Some((replacement_revision, replacement))
        );
    }

    #[test]
    fn selection_store_host_factory_retains_failed_load_for_retry() {
        let store = StoryMissionSelectionStore::default();
        let selection = StoryMissionSelection::new(
            story_flow(),
            sequencer_block(MODE_STORY, 0x65),
            SetuAssetSelector::new(0x4afe_3652, 0),
        )
        .unwrap();
        let revision = store.publish(selection.clone()).unwrap();
        let inputs = StoryGameplayInputsStore::default();
        let archive = pak(&[(0x4afe_3652, b"not SETU")]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let result = StoryMissionHostOwner::load_from_selection_store(
            &mut vfs,
            "D:\\mission.pak",
            &store,
            0,
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
            inputs.clone(),
            |_| {},
        );
        assert!(matches!(result, Err(RuntimeStoryMissionError::Mission(_))));
        assert_eq!(
            store.peek_after(0).unwrap(),
            Some((revision, selection.clone()))
        );
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();

        let entry = empty_setu();
        let archive = pak(&[(0x4afe_3652, &entry)]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let built = StoryMissionHostOwner::load_from_selection_store(
            &mut vfs,
            "D:\\mission.pak",
            &store,
            0,
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
            inputs,
            |_| {},
        )
        .unwrap()
        .unwrap();
        assert_eq!(built.0, revision);
        assert_eq!(built.1.execution(), StoryExecution::Lifted);
        assert_eq!(store.peek_after(0).unwrap(), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sequencer_output_host_factory_retains_failed_load_then_consumes_success() {
        use tsfp_gameplay::modes::story_sequencer::{Phase, SequencerState};

        let outputs = StorySequencerOutputStore::default();
        let setup = sequencer_block(MODE_STORY, 0x65);
        let output = StorySequencerOutput::from_result(
            SequencerState {
                cursor: 4,
                phase: Phase::Level,
            },
            setup.clone(),
        )
        .unwrap();
        let revision = outputs.publish(output).unwrap();
        let selector = SetuAssetSelector::new(0x4afe_3652, 0);
        let inputs = StoryGameplayInputsStore::default();

        let archive = pak(&[(selector.key, b"not SETU")]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let result = StoryMissionHostOwner::load_from_sequencer_output_store(
            &mut vfs,
            "D:\\mission.pak",
            &outputs,
            0,
            selector,
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
            inputs.clone(),
            |_| {},
        );
        assert!(matches!(result, Err(RuntimeStoryMissionError::Mission(_))));
        let (pending_revision, pending) = outputs.peek_after(0).unwrap().unwrap();
        assert_eq!(pending_revision, revision);
        assert_eq!(pending.setup(), &setup);
        assert_eq!(pending.campaign_ordinal().get(), 4);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();

        let entry = empty_setu();
        let archive = pak(&[(selector.key, &entry)]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let (loaded_revision, owner) = StoryMissionHostOwner::load_from_sequencer_output_store(
            &mut vfs,
            "D:\\mission.pak",
            &outputs,
            0,
            selector,
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
            inputs,
            |_| {},
        )
        .unwrap()
        .unwrap();
        assert_eq!(loaded_revision, revision);
        assert_eq!(owner.execution(), StoryExecution::Lifted);
        assert_eq!(owner.runtime.mission().flow().ordinal.get(), 4);
        assert_eq!(owner.runtime.mission().flow().active.byte(0x54), 0x65);
        assert_eq!(owner.runtime.mission().flow().active.byte(0x55), MODE_STORY);
        assert_eq!(outputs.peek_after(0).unwrap(), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sequencer_output_replacement_installs_owner_and_acknowledges_same_revision() {
        use tsfp_gameplay::modes::story_sequencer::{Phase, SequencerState};

        let outputs = StorySequencerOutputStore::default();
        let output = StorySequencerOutput::from_result(
            SequencerState {
                cursor: 4,
                phase: Phase::Level,
            },
            sequencer_block(MODE_STORY, 2),
        )
        .unwrap();
        let revision = outputs.publish(output).unwrap();
        let slot = Mutex::new(Some(synthetic_host_owner(2)));

        let detached = replace_host_owner_for_output_revision(
            &outputs,
            revision,
            synthetic_host_owner(4),
            &slot,
        )
        .unwrap();

        assert_eq!(
            detached.and_then(|owner| owner.controller().map(|controller| controller.level_id)),
            Some(2)
        );
        assert_eq!(
            slot.lock()
                .unwrap()
                .as_ref()
                .and_then(StoryMissionHostOwner::controller)
                .map(|controller| controller.level_id),
            Some(4)
        );
        assert_eq!(outputs.peek_after(0).unwrap(), None);
    }

    #[test]
    fn measured_first_story_output_loads_its_setu_and_replaces_the_active_owner() {
        use tsfp_gameplay::modes::story_sequencer::{Phase, SequencerState};

        let entry = empty_setu();
        let archive = pak(&[(0x4afe_3652, &entry)]);
        let dir = std::env::temp_dir().join(format!(
            "t1703-story-owner-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(dir.join("pak/story")).unwrap();
        fs::write(dir.join("pak/story/l_2_st.pak"), archive).unwrap();
        let mut vfs = Vfs::new();
        vfs.mount_host_dir("D:", &dir).unwrap();

        let outputs = StorySequencerOutputStore::default();
        let revision = outputs
            .publish(
                StorySequencerOutput::from_result(
                    SequencerState {
                        cursor: 0,
                        phase: Phase::Level,
                    },
                    sequencer_block(MODE_STORY, 2),
                )
                .unwrap(),
            )
            .unwrap();
        let slot = Mutex::new(Some(synthetic_host_owner(4)));

        let (loaded_revision, detached) =
            StoryMissionHostOwner::replace_from_sequencer_output_store_in_slot(
                &mut vfs,
                &outputs,
                0,
                first_story_asset_selection,
                TerminalState::new(Vec::new(), Vec::new()),
                StoryGameplayInputsStore::default(),
                |_| {},
                &slot,
            )
            .unwrap()
            .unwrap();

        assert_eq!(loaded_revision, revision);
        assert_eq!(
            detached.and_then(|owner| owner.controller().map(|controller| controller.level_id)),
            Some(4)
        );
        assert_eq!(
            slot.lock()
                .unwrap()
                .as_ref()
                .and_then(StoryMissionHostOwner::controller)
                .map(|controller| controller.level_id),
            Some(2)
        );
        assert_eq!(outputs.peek_after(0).unwrap(), None);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn superseded_sequencer_output_does_not_replace_current_host_owner() {
        use tsfp_gameplay::modes::story_sequencer::{Phase, SequencerState};

        let outputs = StorySequencerOutputStore::default();
        let first = StorySequencerOutput::from_result(
            SequencerState {
                cursor: 4,
                phase: Phase::Level,
            },
            sequencer_block(MODE_STORY, 2),
        )
        .unwrap();
        let first_revision = outputs.publish(first).unwrap();
        let replacement = StorySequencerOutput::from_result(
            SequencerState {
                cursor: 5,
                phase: Phase::Level,
            },
            sequencer_block(MODE_STORY, 4),
        )
        .unwrap();
        let replacement_revision = outputs.publish(replacement).unwrap();
        let slot = Mutex::new(Some(synthetic_host_owner(2)));

        assert!(matches!(
            replace_host_owner_for_output_revision(
                &outputs,
                first_revision,
                synthetic_host_owner(4),
                &slot,
            ),
            Err(RuntimeStoryMissionError::SelectionSuperseded)
        ));
        assert_eq!(
            slot.lock()
                .unwrap()
                .as_ref()
                .and_then(StoryMissionHostOwner::controller)
                .map(|controller| controller.level_id),
            Some(2)
        );
        assert_eq!(
            outputs.peek_after(first_revision).unwrap().unwrap().0,
            replacement_revision
        );
    }

    #[test]
    fn story_selection_rejects_non_story_setup_before_publication() {
        let err = StoryMissionSelection::new(
            story_flow(),
            sequencer_block(6, 0x65),
            SetuAssetSelector {
                key: 0x1234,
                occurrence: 0,
            },
        )
        .unwrap_err();
        assert_eq!(err, StoryMissionLoadError::NotStorySetup { actual_mode: 6 });
    }

    fn mounted_pak(bytes: &[u8]) -> (Vfs, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "t2410-story-runtime-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("mission.pak"), bytes).unwrap();
        let mut vfs = Vfs::new();
        vfs.mount_host_dir("D:", &dir).unwrap();
        (vfs, dir)
    }

    #[test]
    fn vfs_loader_selects_duplicate_setu_entry_and_closes_pak() {
        let entry = empty_setu();
        let archive = pak(&[(0x4afe_3652, b"not SETU"), (0x4afe_3652, &entry)]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let runtime = StoryMissionRuntime::load(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 1),
            StoryFlow::default(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        assert_eq!(runtime.execution(), StoryExecution::Lifted);
        assert!(runtime.mission().events().is_empty());
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn vfs_sequencer_loader_starts_mission_from_the_typed_setup_and_closes_pak() {
        let entry = empty_setu();
        let archive = pak(&[(0x4afe_3652, b"not SETU"), (0x4afe_3652, &entry)]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut flow = StoryFlow::default();
        flow.ordinal = tsfp_state::story_flow::CampaignOrdinal::new(5).unwrap();
        let setup = sequencer_block(MODE_STORY, 2);
        let runtime = StoryMissionRuntime::load_from_sequencer(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 1),
            flow,
            &setup,
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();

        assert_eq!(runtime.execution(), StoryExecution::Lifted);
        assert!(runtime.is_story_gameplay_active());
        assert_eq!(runtime.mission().flow().active.words, setup.words());
        assert_eq!(runtime.mission().flow().ordinal.get(), 5);
        assert_eq!(runtime.mission().controller().unwrap().level_id, 2);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sequenced_store_factory_keeps_lifted_default_and_declines_without_snapshot() {
        let archive = pak(&[(0x4afe_3652, &empty_setu())]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut owner = StoryMissionHostOwner::load_from_vfs_sequenced_store(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            &sequencer_block(MODE_STORY, 2),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
            StoryGameplayInputsStore::default(),
            |_| {},
        )
        .unwrap();

        assert_eq!(owner.execution(), StoryExecution::Lifted);
        assert!(owner.runtime.is_story_gameplay_active());
        assert_eq!(vfs.open_count(), 0);
        owner.set_execution(StoryExecution::Native);
        assert!(!owner.step_from_host());
        assert_eq!(owner.runtime.mission().events().elapsed(), 0.0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn vfs_sequencer_loader_rejects_non_story_setup_and_closes_pak() {
        let archive = pak(&[(0x4afe_3652, &empty_setu())]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let result = StoryMissionRuntime::load_from_sequencer(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            &sequencer_block(9, 0x6a),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        );

        assert!(matches!(
            result,
            Err(RuntimeStoryMissionError::Mission(
                StoryMissionLoadError::NotStorySetup { actual_mode: 9 }
            ))
        ));
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    fn stage_level_four_post_movie_continuation(runtime: &mut StoryMissionRuntime) {
        runtime.mission_mut().set_result_flags(RESULT_SUCCESS);
        assert!(runtime.mission_mut().stage_success_frontend(true));
        let route = StoryMission::continuation_from_sequencer(
            campaign_levels(),
            Some(&sequencer_block(MODE_STORY, 0x20)),
            &sequencer_block(MODE_STORY, 4),
        )
        .unwrap();
        assert!(
            runtime
                .mission_mut()
                .continue_after_results(ready_story_continuation_gate(), route,)
        );
    }

    #[test]
    fn continuation_requires_level_in_campaign_table() {
        let result = StoryMission::continuation_from_sequencer(
            campaign_levels(),
            None,
            &sequencer_block(MODE_STORY, 9),
        );
        assert!(matches!(
            result,
            Err(StoryMissionLoadError::UnknownCampaignLevel { level_id: 9 })
        ));
    }

    fn ready_story_continuation_gate() -> StoryContinuationGate {
        StoryContinuationGate {
            descriptor_ready: true,
            attract_mode: false,
            session_phase: 1,
            menu_kind: 4,
            unlock_notice_pending: false,
        }
    }

    #[test]
    fn vfs_results_continuation_rejects_gate_without_loading_next_level() {
        let archive = pak(&[(0x4afe_3652, &empty_setu())]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut runtime = StoryMissionRuntime::load_from_sequencer(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            &sequencer_block(MODE_STORY, 2),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        runtime.set_execution(StoryExecution::Native);
        runtime.mission_mut().set_result_flags(RESULT_SUCCESS);
        assert!(runtime.mission_mut().stage_success_frontend(true));
        let flow_before = *runtime.mission().flow();
        let route = StoryMission::continuation_from_sequencer(
            campaign_levels(),
            None,
            &sequencer_block(MODE_STORY, 4),
        )
        .unwrap();
        let gate = StoryContinuationGate {
            descriptor_ready: false,
            ..ready_story_continuation_gate()
        };

        let accepted = runtime
            .continue_after_results_and_start_level(
                &mut vfs,
                "D:\\mission.pak",
                SetuAssetSelector::new(0x4afe_3652, 99),
                BuildOptions::story(0, 0),
                TerminalState::new(Vec::new(), Vec::new()),
                TerminalPadBank::default(),
                gate,
                route,
            )
            .unwrap();

        assert!(!accepted);
        assert_eq!(*runtime.mission().flow(), flow_before);
        assert_eq!(runtime.execution(), StoryExecution::Native);
        assert_eq!(runtime.mission().flow().active.level_id(), 2);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    fn stage_level_four_continuation(runtime: &mut StoryMissionRuntime) {
        stage_level_four_post_movie_continuation(runtime);
        assert!(runtime.mission_mut().finish_post_movie());
    }

    #[test]
    fn vfs_staged_level_replaces_story_owner_and_preserves_native_selection() {
        let entry = empty_setu();
        let archive = pak(&[(0x4afe_3652, &entry)]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut runtime = StoryMissionRuntime::load_from_sequencer(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            &sequencer_block(MODE_STORY, 2),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        runtime.set_execution(StoryExecution::Native);
        stage_level_four_continuation(&mut runtime);

        runtime
            .start_staged_level(
                &mut vfs,
                "D:\\mission.pak",
                SetuAssetSelector::new(0x4afe_3652, 0),
                BuildOptions::story(0, 0),
                TerminalState::new(Vec::new(), Vec::new()),
                TerminalPadBank::default(),
            )
            .unwrap();

        assert!(runtime.is_story_gameplay_active());
        assert_eq!(runtime.execution(), StoryExecution::Native);
        assert_eq!(runtime.mission().flow().active.level_id(), 4);
        assert_eq!(runtime.mission().flow().ordinal.get(), 3);
        assert_eq!(runtime.mission().controller().unwrap().level_id, 4);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn vfs_staged_level_rejects_bad_setu_without_replacing_current_owner() {
        let valid_entry = empty_setu();
        let archive = pak(&[(0x4afe_3652, &valid_entry), (0x4afe_3652, b"not SETU")]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut runtime = StoryMissionRuntime::load_from_sequencer(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            &sequencer_block(MODE_STORY, 2),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        runtime.set_execution(StoryExecution::Native);
        stage_level_four_continuation(&mut runtime);
        let flow_before = *runtime.mission().flow();

        let result = runtime.start_staged_level(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 1),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
            TerminalPadBank::default(),
        );

        assert!(matches!(result, Err(RuntimeStoryMissionError::Mission(_))));
        assert_eq!(*runtime.mission().flow(), flow_before);
        assert_eq!(runtime.execution(), StoryExecution::Native);
        assert_eq!(runtime.mission().flow().active.level_id(), 2);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn vfs_results_continuation_loads_direct_next_level_atomically() {
        let entry = empty_setu();
        let archive = pak(&[(0x4afe_3652, &entry)]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut runtime = StoryMissionRuntime::load_from_sequencer(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            &sequencer_block(MODE_STORY, 2),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        runtime.set_execution(StoryExecution::Native);
        runtime.mission_mut().set_result_flags(RESULT_SUCCESS);
        assert!(runtime.mission_mut().stage_success_frontend(true));
        let mut next_setup_bytes = core::array::from_fn(|index| (index as u8).wrapping_mul(37));
        next_setup_bytes[0] = 1;
        next_setup_bytes[0x54] = 4;
        next_setup_bytes[0x55] = MODE_STORY;
        let next_setup = ModeBlock::from_bytes(next_setup_bytes);
        let route = StoryMission::continuation_from_sequencer(campaign_levels(), None, &next_setup)
            .unwrap();
        let mut expected_flow = *runtime.mission().flow();
        assert!(expected_flow.continue_after_results(ready_story_continuation_gate(), route));
        expected_flow.start_level();

        let started = runtime
            .continue_after_results_and_start_level(
                &mut vfs,
                "D:\\mission.pak",
                SetuAssetSelector::new(0x4afe_3652, 0),
                BuildOptions::story(0, 0),
                TerminalState::new(Vec::new(), Vec::new()),
                TerminalPadBank::default(),
                ready_story_continuation_gate(),
                route,
            )
            .unwrap();

        assert!(started);
        assert!(runtime.is_story_gameplay_active());
        assert_eq!(runtime.execution(), StoryExecution::Native);
        assert_eq!(runtime.mission().flow().active, expected_flow.active);
        assert_eq!(runtime.mission().flow().active.level_id(), 4);
        assert_eq!(runtime.mission().flow().ordinal.get(), 3);
        assert_eq!(runtime.mission().controller().unwrap().level_id, 4);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn vfs_guest_campaign_reader_selects_and_starts_the_next_story_level() {
        let entry = empty_setu();
        let archive = pak(&[(0x4afe_3652, &entry)]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut runtime = StoryMissionRuntime::load_from_sequencer(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            &sequencer_block(MODE_STORY, 2),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        runtime.set_execution(StoryExecution::Native);
        runtime.mission_mut().set_result_flags(RESULT_SUCCESS);
        assert!(runtime.mission_mut().stage_success_frontend(true));
        let mut guest_bytes = guest_campaign_table_bytes();
        let mut guest_memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut guest_bytes,
        };

        let started = runtime
            .continue_after_guest_campaign_table_and_start_level(
                &mut vfs,
                "D:\\mission.pak",
                SetuAssetSelector::new(0x4afe_3652, 0),
                BuildOptions::story(0, 0),
                TerminalState::new(Vec::new(), Vec::new()),
                TerminalPadBank::default(),
                ready_story_continuation_gate(),
                &mut guest_memory,
                None,
                &sequencer_block(MODE_STORY, 4),
            )
            .unwrap();

        assert!(started);
        assert!(runtime.is_story_gameplay_active());
        assert_eq!(runtime.execution(), StoryExecution::Native);
        assert_eq!(runtime.mission().flow().active.level_id(), 4);
        assert_eq!(runtime.mission().flow().ordinal.get(), 3);
        assert_eq!(runtime.mission().controller().unwrap().level_id, 4);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn vfs_guest_campaign_reader_fault_preserves_results_owner() {
        let archive = pak(&[(0x4afe_3652, &empty_setu())]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut runtime = StoryMissionRuntime::load_from_sequencer(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            &sequencer_block(MODE_STORY, 2),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        runtime.set_execution(StoryExecution::Native);
        runtime.mission_mut().set_result_flags(RESULT_SUCCESS);
        assert!(runtime.mission_mut().stage_success_frontend(true));
        let flow_before = *runtime.mission().flow();
        let mut short_bytes = [0; 16];
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut short_bytes,
        };

        let result = runtime.continue_after_guest_campaign_table_and_start_level(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
            TerminalPadBank::default(),
            ready_story_continuation_gate(),
            &mut memory,
            None,
            &sequencer_block(MODE_STORY, 4),
        );

        assert!(matches!(
            result,
            Err(RuntimeStoryMissionError::GuestMemory(_))
        ));
        assert_eq!(*runtime.mission().flow(), flow_before);
        assert_eq!(runtime.execution(), StoryExecution::Native);
        assert_eq!(runtime.mission().flow().active.level_id(), 2);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn vfs_runtime_rows_reject_a_short_table_without_mutating_results_owner() {
        let archive = pak(&[(0x4afe_3652, &empty_setu())]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut runtime = StoryMissionRuntime::load_from_sequencer(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            &sequencer_block(MODE_STORY, 2),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        runtime.set_execution(StoryExecution::Native);
        runtime.mission_mut().set_result_flags(RESULT_SUCCESS);
        assert!(runtime.mission_mut().stage_success_frontend(true));
        let flow_before = *runtime.mission().flow();
        let short_rows = [2u8, 0];

        let result = runtime.continue_after_runtime_rows_and_start_level(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 99),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
            TerminalPadBank::default(),
            ready_story_continuation_gate(),
            &short_rows,
            None,
            &sequencer_block(MODE_STORY, 4),
        );

        assert!(matches!(
            result,
            Err(RuntimeStoryMissionError::Mission(
                StoryMissionLoadError::CampaignLevelTable(_)
            ))
        ));
        assert_eq!(*runtime.mission().flow(), flow_before);
        assert_eq!(runtime.execution(), StoryExecution::Native);
        assert_eq!(runtime.mission().flow().active.level_id(), 2);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn vfs_campaign_table_continuation_bad_setu_preserves_retryable_results_owner() {
        let valid_entry = empty_setu();
        let archive = pak(&[(0x4afe_3652, &valid_entry), (0x4afe_3652, b"not SETU")]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut runtime = StoryMissionRuntime::load_from_sequencer(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            &sequencer_block(MODE_STORY, 2),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        runtime.set_execution(StoryExecution::Native);
        runtime.mission_mut().set_result_flags(RESULT_SUCCESS);
        assert!(runtime.mission_mut().stage_success_frontend(true));
        let flow_before = *runtime.mission().flow();
        let result = runtime.continue_after_campaign_table_and_start_level(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 1),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
            TerminalPadBank::default(),
            ready_story_continuation_gate(),
            campaign_levels(),
            None,
            &sequencer_block(MODE_STORY, 4),
        );

        assert!(matches!(result, Err(RuntimeStoryMissionError::Mission(_))));
        assert_eq!(*runtime.mission().flow(), flow_before);
        assert_eq!(runtime.execution(), StoryExecution::Native);
        assert_eq!(runtime.mission().flow().active.level_id(), 2);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn vfs_results_continuation_stages_post_movie_without_loading_next_level() {
        let archive = pak(&[(0x4afe_3652, &empty_setu())]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut runtime = StoryMissionRuntime::load_from_sequencer(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            &sequencer_block(MODE_STORY, 2),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        runtime.mission_mut().set_result_flags(RESULT_SUCCESS);
        assert!(runtime.mission_mut().stage_success_frontend(true));
        let route = StoryMission::continuation_from_sequencer(
            campaign_levels(),
            Some(&sequencer_block(MODE_STORY, 0x20)),
            &sequencer_block(MODE_STORY, 4),
        )
        .unwrap();

        let accepted = runtime
            .continue_after_results_and_start_level(
                &mut vfs,
                "D:\\mission.pak",
                SetuAssetSelector::new(0x4afe_3652, 99),
                BuildOptions::story(0, 0),
                TerminalState::new(Vec::new(), Vec::new()),
                TerminalPadBank::default(),
                ready_story_continuation_gate(),
                route,
            )
            .unwrap();

        assert!(accepted);
        assert_eq!(
            runtime.mission().flow().step,
            StoryStep::PostMovie {
                ordinal: tsfp_state::story_flow::CampaignOrdinal::new(0).unwrap(),
                level_id: 0x20,
            }
        );
        assert_eq!(runtime.mission().flow().staging.level_id(), 0x20);
        assert_eq!(runtime.mission().flow().active.level_id(), 2);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn vfs_post_movie_completion_loads_next_level_atomically() {
        let entry = empty_setu();
        let archive = pak(&[(0x4afe_3652, &entry)]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut runtime = StoryMissionRuntime::load_from_sequencer(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            &sequencer_block(MODE_STORY, 2),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        runtime.set_execution(StoryExecution::Native);
        stage_level_four_post_movie_continuation(&mut runtime);
        assert!(matches!(
            runtime.mission().flow().step,
            StoryStep::PostMovie { .. }
        ));

        assert!(
            runtime
                .finish_post_movie_and_start_level(
                    &mut vfs,
                    "D:\\mission.pak",
                    SetuAssetSelector::new(0x4afe_3652, 0),
                    BuildOptions::story(0, 0),
                    TerminalState::new(Vec::new(), Vec::new()),
                    TerminalPadBank::default(),
                )
                .unwrap()
        );

        assert!(runtime.is_story_gameplay_active());
        assert_eq!(runtime.execution(), StoryExecution::Native);
        assert_eq!(runtime.mission().flow().active.level_id(), 4);
        assert_eq!(runtime.mission().flow().ordinal.get(), 3);
        assert_eq!(runtime.mission().controller().unwrap().level_id, 4);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn vfs_post_movie_bad_setu_preserves_retryable_post_movie_owner() {
        let valid_entry = empty_setu();
        let archive = pak(&[(0x4afe_3652, &valid_entry), (0x4afe_3652, b"not SETU")]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut runtime = StoryMissionRuntime::load_from_sequencer(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            &sequencer_block(MODE_STORY, 2),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        runtime.set_execution(StoryExecution::Native);
        stage_level_four_post_movie_continuation(&mut runtime);
        let flow_before = *runtime.mission().flow();

        let result = runtime.finish_post_movie_and_start_level(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 1),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
            TerminalPadBank::default(),
        );

        assert!(matches!(result, Err(RuntimeStoryMissionError::Mission(_))));
        assert_eq!(*runtime.mission().flow(), flow_before);
        assert!(matches!(
            runtime.mission().flow().step,
            StoryStep::PostMovie { .. }
        ));
        assert_eq!(runtime.execution(), StoryExecution::Native);
        assert_eq!(runtime.mission().flow().active.level_id(), 2);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn host_owner_factory_uses_selected_pak_and_keeps_lifted_default() {
        let entry = empty_setu();
        let archive = pak(&[(0x4afe_3652, b"not SETU"), (0x4afe_3652, &entry)]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let refreshed = std::sync::Arc::new(AtomicU64::new(0));
        let applied = std::sync::Arc::new(AtomicU64::new(0));
        let setup = sequencer_block(MODE_STORY, 2);
        let owner = StoryMissionHostOwner::load_from_vfs_sequenced(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 1),
            StoryFlow::default(),
            &setup,
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
            NativeStoryWorld::default(),
            CountingStoryWorldOwner {
                refreshed,
                applied,
                ready: false,
            },
        )
        .unwrap();

        assert_eq!(owner.runtime.execution(), StoryExecution::Lifted);
        assert!(owner.runtime.is_story_gameplay_active());
        assert_eq!(owner.runtime.mission().flow().active.words, setup.words());
        assert_eq!(
            owner.controller().map(|controller| controller.level_id),
            Some(2)
        );
        assert!(owner.runtime.mission().events().is_empty());
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_sequenced_session_replacement_preserves_active_owner() {
        let old_mission = StoryMission::from_setu_entry(
            story_flow(),
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        let mut owner = StoryMissionHostOwner::new(
            StoryMissionRuntime::from_mission(old_mission),
            NativeStoryWorld::default(),
            CountingStoryWorldOwner {
                refreshed: std::sync::Arc::new(AtomicU64::new(0)),
                applied: std::sync::Arc::new(AtomicU64::new(0)),
                ready: false,
            },
        );
        owner.runtime.mission_mut().set_result_flags(RESULT_SUCCESS);
        let invalid_archive = pak(&[(0x4afe_3652, b"not SETU")]);
        let (mut vfs, dir) = mounted_pak(&invalid_archive);
        let replacement = owner.replace_session_from_vfs_sequenced(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            story_flow(),
            &sequencer_block(MODE_STORY, 0x30),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
            NativeStoryWorld::default(),
            CountingStoryWorldOwner {
                refreshed: std::sync::Arc::new(AtomicU64::new(0)),
                applied: std::sync::Arc::new(AtomicU64::new(0)),
                ready: false,
            },
        );

        assert!(matches!(
            replacement,
            Err(RuntimeStoryMissionError::Mission(_))
        ));
        assert_eq!(owner.execution(), StoryExecution::Lifted);
        assert!(owner.runtime.is_story_gameplay_active());
        assert_eq!(owner.runtime.mission().events().elapsed(), 0.0);
        assert_eq!(owner.runtime.mission().flow().result_flags, RESULT_SUCCESS);
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn fixed_runtime_step_keeps_lifted_branch_as_default() {
        let archive = pak(&[(0x4afe_3652, &empty_setu())]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let mut runtime = StoryMissionRuntime::load(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 0),
            StoryFlow::default(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        let mut world = tsfp_gameplay::story_events::native_world::NativeStoryWorld::default();
        let mut lifted_called = false;
        let _ = runtime.step_fixed(&mut world, || {
            lifted_called = true;
            StepReport::default()
        });
        assert!(lifted_called);
        assert_eq!(runtime.execution(), StoryExecution::Lifted);
        fs::remove_dir_all(dir).unwrap();
    }

    struct CountingStoryWorldOwner {
        refreshed: std::sync::Arc<AtomicU64>,
        applied: std::sync::Arc<AtomicU64>,
        ready: bool,
    }

    impl StoryMissionWorldOwner for CountingStoryWorldOwner {
        fn refresh(&mut self, _world: &mut NativeStoryWorld) -> bool {
            self.refreshed.fetch_add(1, Ordering::Relaxed);
            self.ready
        }

        fn apply_effects(&mut self, effects: Vec<StoryWorldEffect>) {
            self.applied
                .fetch_add(effects.len() as u64 + 1, Ordering::Relaxed);
        }
    }

    struct ReplacingStoryWorldOwner {
        replacement: std::sync::Arc<TestMutex<Option<StoryMissionHostOwner>>>,
        refreshed: std::sync::Arc<AtomicU64>,
    }

    impl StoryMissionWorldOwner for ReplacingStoryWorldOwner {
        fn refresh(&mut self, _world: &mut NativeStoryWorld) -> bool {
            self.refreshed.fetch_add(1, Ordering::Relaxed);
            true
        }

        fn apply_effects(&mut self, _effects: Vec<StoryWorldEffect>) {
            let replacement = self.replacement.lock().unwrap().take();
            if let Some(replacement) = replacement {
                let detached = replace_story_mission_host_owner(replacement).unwrap();
                drop(detached);
            }
        }
    }

    #[test]
    fn c2r_host_owner_keeps_lifted_default_and_steps_native_on_fixed_tick() {
        let mission = StoryMission::from_setu_entry(
            story_flow_for_level(2),
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        let runtime = StoryMissionRuntime::from_mission(mission);
        let non_story = StoryMission::from_setu_entry(
            StoryFlow::default(),
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        assert!(!StoryMissionRuntime::from_mission(non_story).is_story_gameplay_active());
        assert!(runtime.is_story_gameplay_active());
        let refreshed = std::sync::Arc::new(AtomicU64::new(0));
        let applied = std::sync::Arc::new(AtomicU64::new(0));
        let mut owner = StoryMissionHostOwner::new(
            runtime,
            NativeStoryWorld::default(),
            CountingStoryWorldOwner {
                refreshed: refreshed.clone(),
                applied: applied.clone(),
                ready: true,
            },
        );

        assert_eq!(owner.execution(), StoryExecution::Lifted);
        assert!(!owner.step_from_host());
        assert_eq!(refreshed.load(Ordering::Relaxed), 0);
        assert_eq!(applied.load(Ordering::Relaxed), 0);

        owner.set_execution(StoryExecution::Native);
        assert_eq!(owner.execution(), StoryExecution::Native);
        assert!(owner.step_from_host());
        assert_eq!(refreshed.load(Ordering::Relaxed), 1);
        assert_eq!(applied.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn terminal_story_effects_include_one_owned_outcome_before_end_match() {
        let mut flow = story_flow_for_level(2);
        flow.phase = 8;
        let mut mission = StoryMission::from_setu_entry(
            flow,
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        mission.set_result_flags(RESULT_SUCCESS);
        assert!(mission.stage_success_frontend(true));

        let mut forwarded = false;
        let mut effects = vec![
            StoryWorldEffect::MissionEvent(MissionEvent::Complete),
            StoryWorldEffect::EndMatch,
        ];
        forward_terminal_mission_outcome(&mut mission, &mut forwarded, &mut effects);
        assert!(forwarded);
        assert!(matches!(
            effects.as_slice(),
            [
                StoryWorldEffect::MissionEvent(MissionEvent::Complete),
                StoryWorldEffect::MissionOutcome(outcome),
                StoryWorldEffect::EndMatch,
            ] if outcome.flow.active.byte(0x55) == MODE_STORY
                && outcome.flow.phase == 8
                && outcome.flow.ordinal == CampaignOrdinal::new(0).unwrap()
                && outcome.outcome.result_flags == RESULT_SUCCESS
                && outcome.outcome.phase == 8
                && outcome.outcome.event == Some(MissionEvent::Complete)
                && outcome.outcome.campaign_ordinal == Some(CampaignOrdinal::new(0).unwrap())
        ));

        let mut repeated = vec![StoryWorldEffect::EndMatch];
        forward_terminal_mission_outcome(&mut mission, &mut forwarded, &mut repeated);
        assert_eq!(repeated, vec![StoryWorldEffect::EndMatch]);
    }

    #[test]
    fn terminal_result_handoff_retains_each_result_until_ordered_acknowledgement() {
        let mut flow = story_flow_for_level(2);
        flow.phase = 8;
        let mut mission = StoryMission::from_setu_entry(
            flow,
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        mission.set_result_flags(RESULT_SUCCESS);
        assert!(mission.stage_success_frontend(true));
        let result = StoryMissionResult {
            flow: *mission.flow(),
            outcome: mission
                .capture_progress_candidate(Some(MissionEvent::Complete))
                .clone(),
        };

        let store = StoryMissionResultStore::default();
        let inputs = StoryGameplayInputsStore::default();
        let session = inputs.begin_session().unwrap();
        assert_eq!(
            publish_story_mission_effects(
                &store,
                None,
                vec![StoryWorldEffect::MissionOutcome(result.clone())],
            ),
            Err(StoryMissionResultStoreError::MissingSession)
        );
        assert!(store.peek_after(0).unwrap().is_none());
        assert_eq!(
            publish_story_mission_effects(
                &store,
                Some(session),
                vec![
                    StoryWorldEffect::MissionOutcome(result.clone()),
                    StoryWorldEffect::MissionOutcome(result.clone()),
                    StoryWorldEffect::EndMatch,
                ],
            )
            .unwrap(),
            2
        );
        let first = store.peek_after(0).unwrap().unwrap();
        assert_eq!(first.revision, 1);
        assert_eq!(first.result, result);
        assert!(!store.consume_revision(2).unwrap());
        assert_eq!(store.peek_after(0).unwrap().unwrap(), first);
        assert!(store.consume_revision(1).unwrap());
        let second = store.peek_after(1).unwrap().unwrap();
        assert_eq!(second.revision, 2);
        assert!(store.consume_revision(2).unwrap());
        assert!(store.peek_after(2).unwrap().is_none());
    }

    #[test]
    fn terminal_result_processing_defers_until_the_consumer_completes() {
        let mut flow = story_flow_for_level(2);
        flow.phase = 8;
        let mut mission = StoryMission::from_setu_entry(
            flow,
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        mission.set_result_flags(RESULT_SUCCESS);
        assert!(mission.stage_success_frontend(true));
        let result = StoryMissionResult {
            flow: *mission.flow(),
            outcome: mission
                .capture_progress_candidate(Some(MissionEvent::Complete))
                .clone(),
        };
        let store = StoryMissionResultStore::default();
        let inputs = StoryGameplayInputsStore::default();
        let session = inputs.begin_session().unwrap();
        let replacement_session = inputs.begin_session().unwrap();
        assert_eq!(store.publish(session, result).unwrap(), 1);

        let mut consumer_called = false;
        let mismatched: Result<StoryMissionResultProcess<i32>, StoryMissionResultProcessError<()>> =
            store.process_next(replacement_session, |_| {
                consumer_called = true;
                Ok(Some(1))
            });
        assert_eq!(
            mismatched,
            Ok(StoryMissionResultProcess::SessionMismatch {
                expected: replacement_session,
                actual: session,
            })
        );
        assert!(!consumer_called);
        assert_eq!(store.peek_after(0).unwrap().unwrap().session, session);

        let failed: Result<StoryMissionResultProcess<i32>, _> =
            store.process_next(session, |_| Err("profile write failed"));
        assert_eq!(
            failed,
            Err(StoryMissionResultProcessError::Consumer(
                "profile write failed"
            ))
        );
        assert_eq!(store.peek_after(0).unwrap().unwrap().revision, 1);

        let deferred: Result<StoryMissionResultProcess<i32>, StoryMissionResultProcessError<()>> =
            store.process_next(session, |_| Ok(None));
        assert_eq!(deferred, Ok(StoryMissionResultProcess::Deferred));
        assert_eq!(store.peek_after(0).unwrap().unwrap().revision, 1);

        let completed = store.process_next(session, |_| Ok::<Option<i32>, ()>(Some(7)));
        assert_eq!(completed, Ok(StoryMissionResultProcess::Completed(7)));
        assert!(store.peek_after(0).unwrap().is_none());

        let empty = store.process_next(session, |_| Ok::<Option<i32>, ()>(Some(8)));
        assert_eq!(empty, Ok(StoryMissionResultProcess::Empty));
    }

    #[test]
    fn incomplete_typed_world_declines_without_advancing_the_native_mission() {
        let mission = StoryMission::from_setu_entry(
            story_flow_for_level(2),
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        let mut runtime = StoryMissionRuntime::from_mission(mission);
        runtime.set_execution(StoryExecution::Native);
        let refreshed = std::sync::Arc::new(AtomicU64::new(0));
        let applied = std::sync::Arc::new(AtomicU64::new(0));
        let mut owner = StoryMissionHostOwner::new(
            runtime,
            NativeStoryWorld::default(),
            CountingStoryWorldOwner {
                refreshed: refreshed.clone(),
                applied: applied.clone(),
                ready: false,
            },
        );

        assert!(!owner.step_from_host());
        assert_eq!(refreshed.load(Ordering::Relaxed), 1);
        assert_eq!(applied.load(Ordering::Relaxed), 0);
        assert_eq!(owner.runtime.mission().events().elapsed(), 0.0);
    }

    #[test]
    fn unsupported_story_controller_keeps_the_lifted_host_tick() {
        let mission = StoryMission::from_setu_entry(
            story_flow_for_level(24),
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        assert!(mission.controller().is_none());
        let mut runtime = StoryMissionRuntime::from_mission(mission);
        runtime.set_execution(StoryExecution::Native);
        let refreshed = std::sync::Arc::new(AtomicU64::new(0));
        let applied = std::sync::Arc::new(AtomicU64::new(0));
        let mut owner = StoryMissionHostOwner::new(
            runtime,
            NativeStoryWorld::default(),
            CountingStoryWorldOwner {
                refreshed: refreshed.clone(),
                applied: applied.clone(),
                ready: true,
            },
        );

        assert!(!owner.step_from_host());
        assert_eq!(refreshed.load(Ordering::Relaxed), 0);
        assert_eq!(applied.load(Ordering::Relaxed), 0);
        assert_eq!(owner.runtime.mission().events().elapsed(), 0.0);
    }

    #[test]
    fn typed_world_adapter_declines_missing_snapshot_and_delivers_effects_once() {
        let delivered = std::sync::Arc::new(AtomicU64::new(0));
        let sink_count = delivered.clone();
        let mut snapshots = [
            None,
            Some(StoryGameplayInputs::new(
                Vec::new(),
                std::collections::BTreeMap::new(),
                std::collections::BTreeSet::from([0x51]),
                std::collections::BTreeMap::new(),
                std::collections::BTreeSet::new(),
                std::collections::BTreeSet::new(),
                std::collections::BTreeSet::new(),
                std::collections::BTreeSet::new(),
                Vec::new(),
                None,
            )),
        ]
        .into_iter();
        let mut owner = TypedStoryMissionWorldOwner::new(
            move || snapshots.next().flatten(),
            move |effects| {
                sink_count.fetch_add(effects.len() as u64, Ordering::Relaxed);
            },
        );
        let mut world = NativeStoryWorld::default();
        world.tags.insert(0x22);

        assert!(!owner.refresh(&mut world));
        assert_eq!(world.tags, std::collections::BTreeSet::from([0x22]));
        assert!(owner.refresh(&mut world));
        assert_eq!(world.tags, std::collections::BTreeSet::from([0x51]));

        owner.apply_effects(vec![StoryWorldEffect::SetObjective {
            actor_key: 7,
            index: 3,
        }]);
        owner.apply_effects(Vec::new());
        assert_eq!(delivered.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn typed_store_adapter_declines_repeated_frames_until_a_new_snapshot() {
        let store = StoryGameplayInputsStore::default();
        let session = store.begin_session().unwrap();
        let mut owner = TypedStoryMissionWorldOwner::from_store(store.clone(), |_| {});
        let mut world = NativeStoryWorld::default();

        store
            .publish(
                session,
                StoryGameplayInputs::new(
                    Vec::new(),
                    std::collections::BTreeMap::new(),
                    std::collections::BTreeSet::from([0x51]),
                    std::collections::BTreeMap::new(),
                    std::collections::BTreeSet::new(),
                    std::collections::BTreeSet::new(),
                    std::collections::BTreeSet::new(),
                    std::collections::BTreeSet::new(),
                    Vec::new(),
                    None,
                ),
            )
            .unwrap();
        assert!(owner.refresh(&mut world));
        assert_eq!(world.tags, std::collections::BTreeSet::from([0x51]));

        world.tags.insert(0x22);
        assert!(!owner.refresh(&mut world));
        assert_eq!(world.tags, std::collections::BTreeSet::from([0x22, 0x51]));

        store
            .publish(
                session,
                StoryGameplayInputs::new(
                    Vec::new(),
                    std::collections::BTreeMap::new(),
                    std::collections::BTreeSet::from([0x52]),
                    std::collections::BTreeMap::new(),
                    std::collections::BTreeSet::new(),
                    std::collections::BTreeSet::new(),
                    std::collections::BTreeSet::new(),
                    std::collections::BTreeSet::new(),
                    Vec::new(),
                    None,
                ),
            )
            .unwrap();
        assert!(owner.refresh(&mut world));
        assert_eq!(world.tags, std::collections::BTreeSet::from([0x52]));
    }

    #[test]
    fn production_story_input_publisher_reaches_the_installed_store_adapter() {
        let store = story_gameplay_inputs_store();
        let session = store.begin_session().unwrap();
        publish_story_gameplay_inputs(
            session,
            StoryGameplayInputs::new(
                Vec::new(),
                std::collections::BTreeMap::new(),
                std::collections::BTreeSet::from([0x53]),
                std::collections::BTreeMap::new(),
                std::collections::BTreeSet::new(),
                std::collections::BTreeSet::new(),
                std::collections::BTreeSet::new(),
                std::collections::BTreeSet::new(),
                Vec::new(),
                None,
            ),
        )
        .unwrap();

        let mut owner = TypedStoryMissionWorldOwner::from_store(store.clone(), |_| {});
        let mut world = NativeStoryWorld::default();
        assert!(owner.refresh(&mut world));
        assert_eq!(world.tags, std::collections::BTreeSet::from([0x53]));

        let next_session = store.begin_session().unwrap();
        assert_eq!(
            publish_story_gameplay_inputs(
                session,
                StoryGameplayInputs::new(
                    Vec::new(),
                    std::collections::BTreeMap::new(),
                    std::collections::BTreeSet::from([0x54]),
                    std::collections::BTreeMap::new(),
                    std::collections::BTreeSet::new(),
                    std::collections::BTreeSet::new(),
                    std::collections::BTreeSet::new(),
                    std::collections::BTreeSet::new(),
                    Vec::new(),
                    None,
                ),
            ),
            Err(tsfp_gameplay::story_events::native_world::StoryGameplayInputsStoreError::StaleSession)
        );
        publish_story_gameplay_inputs(
            next_session,
            StoryGameplayInputs::new(
                Vec::new(),
                std::collections::BTreeMap::new(),
                std::collections::BTreeSet::from([0x54]),
                std::collections::BTreeMap::new(),
                std::collections::BTreeSet::new(),
                std::collections::BTreeSet::new(),
                std::collections::BTreeSet::new(),
                std::collections::BTreeSet::new(),
                Vec::new(),
                None,
            ),
        )
        .unwrap();
        assert!(owner.refresh(&mut world));
        assert_eq!(world.tags, std::collections::BTreeSet::from([0x54]));

        let _final_session = store.begin_session().unwrap();
    }

    struct PartialWorldOwner {
        complete_on_attempt: usize,
        attempts: usize,
    }

    impl StoryMissionWorldOwner for PartialWorldOwner {
        fn refresh(&mut self, world: &mut NativeStoryWorld) -> bool {
            self.attempts += 1;
            world.tags.insert(0xfeed);
            self.attempts >= self.complete_on_attempt
        }

        fn apply_effects(&mut self, _effects: Vec<StoryWorldEffect>) {}
    }

    #[test]
    fn incomplete_world_refresh_discards_partial_facts_before_retry() {
        let mission = StoryMission::from_setu_entry(
            story_flow_for_level(2),
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        let mut runtime = StoryMissionRuntime::from_mission(mission);
        runtime.set_execution(StoryExecution::Native);
        let mut owner = StoryMissionHostOwner::new(
            runtime,
            NativeStoryWorld::default(),
            PartialWorldOwner {
                complete_on_attempt: 2,
                attempts: 0,
            },
        );

        assert!(!owner.step_from_host());
        assert!(!owner.world.tags.contains(&0xfeed));
        assert_eq!(owner.runtime.mission().events().elapsed(), 0.0);

        assert!(owner.step_from_host());
        assert!(owner.world.tags.contains(&0xfeed));
        assert_eq!(owner.runtime.mission().events().elapsed(), 1.0 / 60.0);
    }

    #[test]
    fn exported_c2r_callback_keeps_lifted_default_then_runs_installed_typed_owner() {
        let _guard = HOST_CALLBACK_TEST_LOCK.lock().unwrap();
        drop(clear_story_mission_host_owner().unwrap());
        // No installed owner means the C2R call site must run the original lifted body.
        assert_eq!(unsafe { tsfp_bevy_story_mission_host_tick() }, 0);

        let mission = StoryMission::from_setu_entry(
            story_flow_for_level(2),
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        let mut runtime = StoryMissionRuntime::from_mission(mission);
        // Native selection is explicit and remains off for newly loaded runtimes.
        assert_eq!(runtime.execution(), StoryExecution::Lifted);
        runtime.set_execution(StoryExecution::Native);

        let refreshed = std::sync::Arc::new(AtomicU64::new(0));
        let applied = std::sync::Arc::new(AtomicU64::new(0));
        install_story_mission_host_owner(StoryMissionHostOwner::new(
            runtime,
            NativeStoryWorld::default(),
            CountingStoryWorldOwner {
                refreshed: refreshed.clone(),
                applied: applied.clone(),
                ready: true,
            },
        ))
        .unwrap();

        // This is the exact no-argument C ABI registered by main.rs and called by C2R.
        assert_eq!(unsafe { tsfp_bevy_story_mission_host_tick() }, 1);
        assert_eq!(refreshed.load(Ordering::Relaxed), 1);
        assert_eq!(applied.load(Ordering::Relaxed), 1);

        // A later typed session replaces the old mission and starts from its default-off
        // schedule. Dropping the detached owner happens outside the host-owner mutex.
        let next_mission = StoryMission::from_setu_entry(
            story_flow_for_level(2),
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        let next_runtime = StoryMissionRuntime::from_mission(next_mission);
        assert_eq!(next_runtime.execution(), StoryExecution::Lifted);
        let next_refreshed = std::sync::Arc::new(AtomicU64::new(0));
        let next_applied = std::sync::Arc::new(AtomicU64::new(0));
        let detached = replace_story_mission_host_owner(StoryMissionHostOwner::new(
            {
                let mut runtime = next_runtime;
                runtime.set_execution(StoryExecution::Native);
                runtime
            },
            NativeStoryWorld::default(),
            CountingStoryWorldOwner {
                refreshed: next_refreshed.clone(),
                applied: next_applied.clone(),
                ready: true,
            },
        ))
        .unwrap();
        assert!(detached.is_some());
        drop(detached);
        assert_eq!(unsafe { tsfp_bevy_story_mission_host_tick() }, 1);
        assert_eq!(next_refreshed.load(Ordering::Relaxed), 1);
        assert_eq!(next_applied.load(Ordering::Relaxed), 1);
        assert!(clear_story_mission_host_owner().unwrap().is_some());
    }

    #[test]
    fn host_tick_allows_effect_handler_to_replace_the_next_session() {
        let _guard = HOST_CALLBACK_TEST_LOCK.lock().unwrap();
        drop(clear_story_mission_host_owner().unwrap());

        // A host tick is handled only after a supported controller is selected. Level 0 is
        // useful for setup-only fixtures but has no measured controller record.
        let next_mission = StoryMission::from_setu_entry(
            story_flow_for_level(2),
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        let mut next_runtime = StoryMissionRuntime::from_mission(next_mission);
        next_runtime.set_execution(StoryExecution::Native);
        let next_refreshed = std::sync::Arc::new(AtomicU64::new(0));
        let next_owner = StoryMissionHostOwner::new(
            next_runtime,
            NativeStoryWorld::default(),
            CountingStoryWorldOwner {
                refreshed: next_refreshed.clone(),
                applied: std::sync::Arc::new(AtomicU64::new(0)),
                ready: true,
            },
        );
        let replacement = std::sync::Arc::new(TestMutex::new(Some(next_owner)));

        let first_mission = StoryMission::from_setu_entry(
            story_flow_for_level(2),
            &empty_setu(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        )
        .unwrap();
        let mut first_runtime = StoryMissionRuntime::from_mission(first_mission);
        first_runtime.set_execution(StoryExecution::Native);
        let first_refreshed = std::sync::Arc::new(AtomicU64::new(0));
        install_story_mission_host_owner(StoryMissionHostOwner::new(
            first_runtime,
            NativeStoryWorld::default(),
            ReplacingStoryWorldOwner {
                replacement: replacement.clone(),
                refreshed: first_refreshed.clone(),
            },
        ))
        .unwrap();

        assert_eq!(unsafe { tsfp_bevy_story_mission_host_tick() }, 1);
        assert_eq!(first_refreshed.load(Ordering::Relaxed), 1);
        assert!(replacement.lock().unwrap().is_none());
        assert_eq!(unsafe { tsfp_bevy_story_mission_host_tick() }, 1);
        assert_eq!(next_refreshed.load(Ordering::Relaxed), 1);
        assert!(clear_story_mission_host_owner().unwrap().is_some());
    }

    #[test]
    fn vfs_loader_reports_missing_selector_and_closes_pak() {
        let archive = pak(&[(0x4afe_3652, &empty_setu())]);
        let (mut vfs, dir) = mounted_pak(&archive);
        let result = load_story_mission(
            &mut vfs,
            "D:\\mission.pak",
            SetuAssetSelector::new(0x4afe_3652, 1),
            StoryFlow::default(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
        );
        assert!(matches!(
            result,
            Err(RuntimeStoryMissionError::FileIo(
                status::OBJECT_NAME_NOT_FOUND
            ))
        ));
        assert_eq!(vfs.open_count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }
}
