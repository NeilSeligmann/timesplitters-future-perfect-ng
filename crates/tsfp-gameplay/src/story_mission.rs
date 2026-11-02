//! Rust-owned Story mission subsystem composition (T2410).
//!
//! This owner joins the typed Story flow, objective/event engine, mission terminals, and the
//! read-only progress handoff. Runtime-decoded setup/event/widget data enters through the
//! constructor; no game tables, guest pointers, or profile identity are inferred here. Durable
//! profile mutation remains behind the authenticated file-I/O boundary.

use tsfp_state::story_flow::{
    CampaignLevelTable, CampaignLevelTableDecodeError, MODE_STORY, SessionSetup, StoryContinuation,
    StoryContinuationGate, StoryController, StoryFlow, story_controller,
};
use tsfp_state::terminals::{
    PhysicalInputError, PhysicalTerminalInput, ScreenId, ScriptOverride, TerminalError,
    TerminalEvent, TerminalInputFrame, TerminalPadBank, TerminalState, WidgetId,
    resolve_terminal_input, sample_terminal_pad, terminal_event,
};

use crate::modes::story_sequencer::ModeBlock;
use crate::story_events::engine::{BuildOptions, Engine, StepReport};
use crate::story_events::setu::{SetuAssetSelector, parse_events, setu_entry_from_pak};
use crate::story_events::slots::parse_slot_defs;
use crate::story_events::slots::{MissionEvent, MissionOutcome};
use crate::story_events::world::World;

/// The owner for one Story mission session.
///
/// Objective slots live inside `events`; the value returned by `capture_progress_candidate` is an
/// observational snapshot for the typed progress seam. It does not imply success, trigger a save,
/// or replace the selected-profile transaction.
pub struct StoryMission {
    flow: StoryFlow,
    events: Engine,
    terminals: TerminalState,
    terminal_input: TerminalPadBank,
    progress_candidate: Option<MissionOutcome>,
}

/// A runtime SETU entry could not be decoded into the mission owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoryMissionLoadError {
    PakEntry(String),
    EventDefinitions(String),
    ObjectiveDefinitions(String),
    NotStorySetup { actual_mode: u8 },
    CampaignLevelTable(CampaignLevelTableDecodeError),
    UnknownCampaignLevel { level_id: i32 },
}

/// A typed physical-input sample could not be routed to this mission's terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryTerminalInputError {
    Terminal(TerminalError),
    Physical(PhysicalInputError),
}

impl StoryMission {
    /// Take ownership of the runtime-decoded state for one session.
    pub fn new(flow: StoryFlow, mut events: Engine, terminals: TerminalState) -> Self {
        events.session_flags = flow.result_flags;
        Self {
            flow,
            events,
            terminals,
            terminal_input: TerminalPadBank::default(),
            progress_candidate: None,
        }
    }

    /// Construct the mission owner from one runtime-decoded SETU entry.
    ///
    /// The entry is the original runtime data supplied by the caller. Its event records and
    /// static objective slots are parsed with the shared table decoders, then moved into the
    /// Rust event engine. The active session, build context, and terminal state are typed inputs
    /// from their respective runtime owners; this function does not inspect guest memory or
    /// invent defaults when a table is malformed.
    pub fn from_setu_entry(
        flow: StoryFlow,
        setu_entry: &[u8],
        options: BuildOptions,
        terminals: TerminalState,
    ) -> Result<Self, StoryMissionLoadError> {
        let definitions =
            parse_events(setu_entry).map_err(StoryMissionLoadError::EventDefinitions)?;
        let slot_definitions =
            parse_slot_defs(setu_entry).map_err(StoryMissionLoadError::ObjectiveDefinitions)?;
        let mut events = Engine::new(&definitions, options);
        events.load_static_slots(&slot_definitions);
        Ok(Self::new(flow, events, terminals))
    }

    /// Build a replacement owner for the Story setup already staged in `flow`.
    ///
    /// The runtime supplies the next SETU payload and typed terminal/input owners. The setup is
    /// validated before the new owner is created, and the active flow advances only in the local
    /// replacement after all SETU tables decode successfully.
    pub fn from_staged_setu_entry(
        flow: StoryFlow,
        setu_entry: &[u8],
        options: BuildOptions,
        terminals: TerminalState,
        terminal_input: TerminalPadBank,
    ) -> Result<Self, StoryMissionLoadError> {
        if flow.staging.byte(0x55) != MODE_STORY {
            return Err(StoryMissionLoadError::NotStorySetup {
                actual_mode: flow.staging.byte(0x55),
            });
        }
        let mut mission = Self::from_setu_entry(flow, setu_entry, options, terminals)?;
        mission.flow.start_level();
        mission.events.session_flags = mission.flow.result_flags;
        mission.terminal_input = terminal_input;
        Ok(mission)
    }

    /// Construct an active Story mission from the runtime sequencer's complete typed setup block.
    /// The mode must be Story before ownership transfers; all other setup bytes are preserved.
    pub fn from_sequencer_setup(
        mut flow: StoryFlow,
        setup_block: &ModeBlock,
        setu_entry: &[u8],
        options: BuildOptions,
        terminals: TerminalState,
    ) -> Result<Self, StoryMissionLoadError> {
        let setup = SessionSetup::from_words(setup_block.words());
        if setup.byte(0x55) != MODE_STORY {
            return Err(StoryMissionLoadError::NotStorySetup {
                actual_mode: setup.byte(0x55),
            });
        }
        flow.stage(setup);
        flow.start_level();
        Self::from_setu_entry(flow, setu_entry, options, terminals)
    }

    /// Decode the caller's runtime level-row view and resolve one validated continuation.
    /// The byte view begins at the measured first signed ID field; no rows are embedded here.
    pub fn continuation_from_runtime_level_rows(
        level_rows: &[u8],
        post_movie: Option<&ModeBlock>,
        next_gameplay: &ModeBlock,
    ) -> Result<StoryContinuation, StoryMissionLoadError> {
        let campaign_levels = CampaignLevelTable::decode_runtime_rows(level_rows)
            .map_err(StoryMissionLoadError::CampaignLevelTable)?;
        Self::continuation_from_sequencer(campaign_levels, post_movie, next_gameplay)
    }

    /// Join sequencer-produced records to the post-result continuation contract without rebuilding
    /// or normalizing setup bytes. The next campaign ordinal is resolved from the runtime-decoded
    /// level table and the selected gameplay setup, so callers cannot pair unrelated values.
    pub fn continuation_from_sequencer(
        campaign_levels: CampaignLevelTable,
        post_movie: Option<&ModeBlock>,
        next_gameplay: &ModeBlock,
    ) -> Result<StoryContinuation, StoryMissionLoadError> {
        let next_gameplay = SessionSetup::from_words(next_gameplay.words());
        if next_gameplay.byte(0x55) != MODE_STORY {
            return Err(StoryMissionLoadError::NotStorySetup {
                actual_mode: next_gameplay.byte(0x55),
            });
        }
        let post_movie = post_movie
            .map(|block| SessionSetup::from_words(block.words()))
            .map(|setup| {
                if setup.byte(0x55) == MODE_STORY {
                    Ok(setup)
                } else {
                    Err(StoryMissionLoadError::NotStorySetup {
                        actual_mode: setup.byte(0x55),
                    })
                }
            })
            .transpose()?;
        let level_id = next_gameplay.level_id();
        StoryContinuation::from_runtime_levels(campaign_levels, post_movie, next_gameplay)
            .ok_or(StoryMissionLoadError::UnknownCampaignLevel { level_id })
    }

    /// Construct the owner from a selected runtime SETU entry in an in-memory PAK image.
    ///
    /// The caller owns VFS access and supplies the PAK bytes selected by the runtime resource
    /// table. The selector preserves duplicate PAK-key ordering; compressed entries and
    /// non-SETU rows fail explicitly instead of falling back to another mission.
    pub fn from_pak_entry(
        flow: StoryFlow,
        pak_bytes: &[u8],
        selector: SetuAssetSelector,
        options: BuildOptions,
        terminals: TerminalState,
    ) -> Result<Self, StoryMissionLoadError> {
        let entry =
            setu_entry_from_pak(pak_bytes, selector).map_err(StoryMissionLoadError::PakEntry)?;
        Self::from_setu_entry(flow, &entry, options, terminals)
    }

    pub fn flow(&self) -> &StoryFlow {
        &self.flow
    }

    /// Resolve the measured controller record for this active setup. Unknown Story level IDs and
    /// non-Story setups remain unselected; this identifies the runtime attachment and does not
    /// invoke a guest callback address.
    pub fn controller(&self) -> Option<StoryController> {
        story_controller(self.flow.active.level_id(), self.flow.active.byte(0x55))
    }

    pub fn stage(&mut self, setup: SessionSetup) {
        self.flow.stage(setup);
    }

    pub fn start_level(&mut self) {
        self.flow.start_level();
    }

    /// Commit a staged Story setup together with its runtime-decoded SETU payload.
    ///
    /// Parsing completes before the active flow or mission state is replaced. A rejected mode or
    /// malformed payload therefore leaves the current mission available to its caller. The caller
    /// supplies the new terminal and input owners explicitly; this method does not select a table
    /// row, synthesize input, or infer campaign progress.
    pub fn start_staged_level(
        &mut self,
        setu_entry: &[u8],
        options: BuildOptions,
        terminals: TerminalState,
        terminal_input: TerminalPadBank,
    ) -> Result<(), StoryMissionLoadError> {
        let replacement = Self::from_staged_setu_entry(
            self.flow,
            setu_entry,
            options,
            terminals,
            terminal_input,
        )?;
        *self = replacement;
        Ok(())
    }

    /// Keep the shared mission-result word synchronized across the Story and event owners.
    pub fn set_result_flags(&mut self, flags: u32) {
        self.flow.set_result_flags(flags);
        self.events.session_flags = flags;
    }

    pub fn stage_success_frontend(&mut self, accepted_by_phase8_writer: bool) -> bool {
        self.flow.stage_success_frontend(accepted_by_phase8_writer)
    }

    pub fn continue_after_results(
        &mut self,
        gate: StoryContinuationGate,
        route: StoryContinuation,
    ) -> bool {
        self.flow.continue_after_results(gate, route)
    }

    pub fn finish_post_movie(&mut self) -> bool {
        self.flow.finish_post_movie()
    }

    /// Run one mission tick and copy the resulting shared result word into the Story projection.
    pub fn step(&mut self, dt: f32, world: &mut dyn World) -> StepReport {
        let report = self.events.step(dt, world);
        self.flow.result_flags = self.events.session_flags;
        report
    }

    /// Register an actor with the mission's event and objective state.
    pub fn register_actor(&mut self, actor_key: u32, class: u32) {
        self.events.register_actor(actor_key, class);
    }

    /// Remove an actor from the mission's event and objective state.
    pub fn unregister_actor(&mut self, actor_key: u32) {
        self.events.unregister_actor(actor_key);
    }

    /// Deliver a native keyed trigger to the mission event links.
    pub fn signal_event_key(&mut self, key: u32, reset: bool, modes: &[u32]) -> usize {
        self.events.signal_key(key, reset, modes)
    }

    /// Fire the first mission event with this runtime event id.
    pub fn fire_event(&mut self, event_id: u32, world: &mut dyn World) -> Option<usize> {
        let fired = self.events.fire_by_id(event_id, world);
        self.flow.result_flags = self.events.session_flags;
        fired
    }

    /// Drain event reports produced by native ingress outside the regular mission tick.
    pub fn take_event_report(&mut self) -> StepReport {
        self.events.take_side_report()
    }

    /// Apply the runtime checkpoint mask to the owned event state.
    pub fn set_checkpoint_mask(&mut self, mask: u32) {
        self.events.set_checkpoint_mask(mask);
    }

    pub fn checkpoint_mask(&self) -> u32 {
        self.events.checkpoint_mask()
    }

    /// Finish an objective slot through the event engine's callback and HUD path.
    pub fn finish_objective_slot(&mut self, id: u32, world: &mut dyn World) {
        self.events.finish_slot(id, world);
        self.flow.result_flags = self.events.session_flags;
    }

    pub fn events(&self) -> &Engine {
        &self.events
    }

    pub fn terminals(&self) -> &TerminalState {
        &self.terminals
    }

    /// Replace the typed terminal input bank with records owned by the active input system.
    pub fn set_terminal_input_bank(&mut self, bank: TerminalPadBank) {
        self.terminal_input = bank;
    }

    /// Sample this mission's physical terminal input, apply the typed suppression/script
    /// precedence, and classify the edge against the selected screen's previous-held state.
    /// Widget handlers remain a neighboring UI contract; this method does not synthesize a pad
    /// press or mutate the input bank's previous-held record.
    pub fn terminal_input_event(
        &self,
        screen: ScreenId,
        slot: usize,
        activation_mask: u32,
        suppression_latch: bool,
        script: Option<ScriptOverride>,
    ) -> Result<(TerminalInputFrame, TerminalEvent), StoryTerminalInputError> {
        let previous_held = self
            .terminals
            .screens
            .iter()
            .find(|candidate| candidate.id == screen)
            .ok_or(StoryTerminalInputError::Terminal(
                TerminalError::MissingScreen(screen),
            ))?
            .previous_held;
        let physical = sample_terminal_pad(&self.terminal_input, slot, activation_mask)
            .map_err(StoryTerminalInputError::Physical)?;
        let input = resolve_terminal_input(physical, suppression_latch, script);
        let event = terminal_event(input.held, previous_held);
        Ok((input, event))
    }

    /// Resolve one native Rust control sample for this mission's terminal without passing
    /// through the emulated pad-record bank. The input owner supplies device/slot mapping,
    /// consumed held bits, activation mask, and normalized cursor axes explicitly.
    pub fn native_terminal_input_event(
        &self,
        screen: ScreenId,
        physical: PhysicalTerminalInput,
        suppression_latch: bool,
        script: Option<ScriptOverride>,
    ) -> Result<(TerminalInputFrame, TerminalEvent), StoryTerminalInputError> {
        let previous_held = self
            .terminals
            .screens
            .iter()
            .find(|candidate| candidate.id == screen)
            .ok_or(StoryTerminalInputError::Terminal(
                TerminalError::MissingScreen(screen),
            ))?
            .previous_held;
        let input = resolve_terminal_input(physical, suppression_latch, script);
        let event = terminal_event(input.held, previous_held);
        Ok((input, event))
    }

    /// Commit the previous-held latch after the neighboring handler has consumed this frame.
    pub fn commit_terminal_input_event(
        &mut self,
        screen: ScreenId,
        input: TerminalInputFrame,
    ) -> Result<(), StoryTerminalInputError> {
        let target = self
            .terminals
            .screens
            .iter_mut()
            .find(|candidate| candidate.id == screen)
            .ok_or(StoryTerminalInputError::Terminal(
                TerminalError::MissingScreen(screen),
            ))?;
        target.previous_held = u16::from(input.held);
        Ok(())
    }

    /// Raise and focus a terminal widget through the owner boundary.
    pub fn bring_terminal_to_front(&mut self, widget: WidgetId) -> Result<(), TerminalError> {
        self.terminals.bring_to_front(widget)
    }

    /// Focus the topmost active child on a terminal screen.
    pub fn focus_terminal_topmost(
        &mut self,
        screen: ScreenId,
    ) -> Result<Option<WidgetId>, TerminalError> {
        self.terminals.focus_topmost(screen)
    }

    /// Activate an ordered terminal child and update its owned parent/widget/timer state.
    pub fn activate_terminal_child(
        &mut self,
        screen: ScreenId,
        child_position: usize,
        timer: i32,
    ) -> Result<bool, TerminalError> {
        self.terminals.activate_child(screen, child_position, timer)
    }

    /// Capture the typed progress inputs currently owned by this Story session.
    ///
    /// The mode byte and bounded campaign ordinal come from the active runtime-owned flow. `event`
    /// is supplied by the runtime caller because the REA contract does not equate a static
    /// objective key with a completion event.
    /// This snapshot is not a save request or a durable receipt.
    pub fn capture_progress_candidate(&mut self, event: Option<MissionEvent>) -> &MissionOutcome {
        let campaign_ordinal = Some(self.flow.ordinal);
        let outcome = self.events.slots.mission_outcome(
            self.flow.active.byte(0x55),
            self.flow.result_flags,
            self.flow.phase,
            event,
            campaign_ordinal,
        );
        self.progress_candidate = Some(outcome);
        self.progress_candidate
            .as_ref()
            .expect("candidate was just stored")
    }

    pub fn progress_candidate(&self) -> Option<&MissionOutcome> {
        self.progress_candidate.as_ref()
    }

    /// Clear the observational candidate after its typed consumer has handled it.
    pub fn clear_progress_candidate(&mut self) -> Option<MissionOutcome> {
        self.progress_candidate.take()
    }

    /// Whether the active setup is the REA-identified Story mode.
    pub fn is_story(&self) -> bool {
        self.flow.active.byte(0x55) == MODE_STORY
    }
}
