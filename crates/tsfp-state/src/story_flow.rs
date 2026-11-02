//! T1703 Story setup, result, and continuation state.
//!
//! This owns only the transition contract around the original 0xA0-byte setup record. Mission
//! tables and movie configurations are supplied by their runtime data readers; no disc data is
//! embedded here. See `docs/t1703-story-flow-rust.md` and `docs/t1703-rea-story.md`.

use super::GuestMemory;

pub const SETUP_BYTES: usize = 0xa0;
pub const SETUP_WORDS: usize = SETUP_BYTES / 4;
pub const STAGING_SETUP: u32 = 0x007d_e340;
pub const ACTIVE_SETUP: u32 = 0x007d_e400;
pub const MODE_LEVEL: u32 = 0x0079_094c;
pub const LOCAL_PLAYERS: u32 = 0x0079_0950;
pub const MATCH_PHASE: u32 = 0x007d_e30c;
pub const SESSION_RESULT: u32 = 0x007d_e45c;
const ACTIVATION_RESET_OFFSETS: [usize; 6] = [0x5c, 0x78, 0x7c, 0x80, 0x84, 0x88];
const SETUP_ITEM_COUNT_OFFSET: usize = 3;
const SETUP_ITEM_IDS_OFFSET: usize = 4;
const ACTIVE_ITEM_FLAGS_OFFSET: usize = 0x77;
const FINAL_ACTIVATION_CLEAR_OFFSET: usize = 0x88;

pub const MODE_STORY: u8 = 10;
pub const RESULT_SUCCESS: u32 = 0x8000;
pub const RESULT_FAILURE: u32 = 0x1_0000;
pub const CAMPAIGN_LEVEL_ROW_STRIDE_BYTES: usize = 0x44;

/// A position in the 13-entry Story progress table, distinct from a gameplay level id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CampaignOrdinal(u8);

impl CampaignOrdinal {
    pub const COUNT: u8 = 13;

    pub const fn new(value: u8) -> Option<Self> {
        if value < Self::COUNT {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

impl Default for CampaignOrdinal {
    fn default() -> Self {
        Self(0)
    }
}

/// Runtime-decoded gameplay identifiers for the 13 Story progress rows.
///
/// The original table interleaves this signed-word field with other row data. Its address and
/// stride belong to the runtime reader; Story state receives only the typed identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignLevelTable {
    level_ids: [i16; CampaignOrdinal::COUNT as usize],
}

impl CampaignLevelTable {
    pub const fn new(level_ids: [i16; CampaignOrdinal::COUNT as usize]) -> Self {
        Self { level_ids }
    }

    /// Decode the signed level ID field from runtime-provided campaign rows.
    ///
    /// `rows` starts at the first ID field (the measured `0x4d1ac6` field in the original
    /// layout). Each of the 13 rows advances by `0x44` bytes. The caller supplies bytes from
    /// its runtime data reader; this type does not read guest memory or embed table contents.
    pub fn decode_runtime_rows(rows: &[u8]) -> Result<Self, CampaignLevelTableDecodeError> {
        let required = (CampaignOrdinal::COUNT as usize - 1) * CAMPAIGN_LEVEL_ROW_STRIDE_BYTES + 2;
        if rows.len() < required {
            return Err(CampaignLevelTableDecodeError::TooShort {
                required,
                actual: rows.len(),
            });
        }

        let mut level_ids = [0i16; CampaignOrdinal::COUNT as usize];
        for (index, level_id) in level_ids.iter_mut().enumerate() {
            let offset = index * CAMPAIGN_LEVEL_ROW_STRIDE_BYTES;
            *level_id = i16::from_le_bytes([rows[offset], rows[offset + 1]]);
        }
        Ok(Self::new(level_ids))
    }

    /// Resolve a signed gameplay identifier to its first matching progress-table row.
    ///
    /// The original 0x64D60 scan returns the first index or -1 when no row matches. A duplicate
    /// identifier therefore resolves to its earliest row, matching the forward table scan.
    pub fn ordinal_for_level_id(self, level_id: i32) -> Option<CampaignOrdinal> {
        self.level_ids
            .iter()
            .position(|candidate| i32::from(*candidate) == level_id)
            .and_then(|index| CampaignOrdinal::new(index as u8))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignLevelTableDecodeError {
    TooShort { required: usize, actual: usize },
}

/// The original setup layout is still data-driven. Unnamed words remain untyped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionSetup {
    pub words: [u32; SETUP_WORDS],
}

impl Default for SessionSetup {
    fn default() -> Self {
        Self {
            words: [0; SETUP_WORDS],
        }
    }
}

impl SessionSetup {
    /// Take ownership of the exact 40-word setup record produced by a typed mode sequencer.
    pub const fn from_words(words: [u32; SETUP_WORDS]) -> Self {
        Self { words }
    }

    pub fn load<M: GuestMemory>(m: &M, address: u32) -> Self {
        Self {
            words: core::array::from_fn(|i| m.read32(address.wrapping_add((i * 4) as u32))),
        }
    }

    pub fn store<M: GuestMemory>(&self, m: &mut M, address: u32) {
        for (i, word) in self.words.iter().copied().enumerate() {
            m.write32(address.wrapping_add((i * 4) as u32), word);
        }
    }

    pub fn byte(&self, offset: usize) -> u8 {
        let word = self.words[offset / 4];
        (word >> ((offset & 3) * 8)) as u8
    }

    pub fn set_byte(&mut self, offset: usize, value: u8) {
        let shift = (offset & 3) * 8;
        let word = &mut self.words[offset / 4];
        *word = (*word & !(0xff << shift)) | ((value as u32) << shift);
    }

    /// Original mode/level byte at +0x54 is sign-extended into the global dword.
    pub fn level_id(&self) -> i32 {
        self.byte(0x54) as i8 as i32
    }

    /// Original player-count byte at +0 is sign-extended into the global dword.
    pub fn player_count(&self) -> i32 {
        self.byte(0) as i8 as i32
    }
}

/// The 0x658D0 contract: copy exactly 40 dwords, leaving all surrounding bytes untouched.
pub fn copy_setup<M: GuestMemory>(m: &mut M, source: u32, destination: u32) {
    for i in 0..SETUP_WORDS {
        let offset = (i * 4) as u32;
        m.write32(
            destination.wrapping_add(offset),
            m.read32(source.wrapping_add(offset)),
        );
    }
}

/// Known type-10 Story controller callbacks from original mode records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryController {
    pub level_id: i32,
    pub mode_record: u32,
    pub preload: u32,
    pub initialize: u32,
    pub tick: u32,
}

pub const STORY_CONTROLLERS: [StoryController; 2] = [
    StoryController {
        level_id: 2,
        mode_record: 0x0050_8390,
        preload: 0x0021_8ab0,
        initialize: 0x0021_ad50,
        tick: 0x0021_ad00,
    },
    StoryController {
        level_id: 4,
        mode_record: 0x004f_f470,
        preload: 0x001e_1c50,
        initialize: 0x001e_9cc0,
        tick: 0x001e_40d0,
    },
];

pub fn story_controller(level_id: i32, mode_type: u8) -> Option<StoryController> {
    (mode_type == MODE_STORY)
        .then(|| {
            STORY_CONTROLLERS
                .iter()
                .copied()
                .find(|entry| entry.level_id == level_id)
        })
        .flatten()
}

/// Identity of a runtime camera instance. This is deliberately distinct from a camera-table
/// key: REA shows the release guard compares the context camera pointer with the active camera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraInstance(pub u32);

/// A callback token owned by the runtime event/session layer. It is not a guest function address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CinematicCallback(pub u32);

pub const CINEMATIC_CONTROL_FLAG: u32 = 0x0200_0000;
pub const CINEMATIC_PLAYER_MODE: u32 = 0x100;
pub const GAMEPLAY_PLAYER_MODE: u32 = 2;
const STORY_DEATH_OR_CONTROL_FLAGS: u32 = 0x0002_0400;

/// The opening event-7 movie owns this gate until its camera blend has completed. This state is
/// separate from the later SCAM spline's player-control release.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputGateState {
    pub opening_movie_locked: bool,
}

impl InputGateState {
    /// The measured 0x23efe0 cleanup clears the opening input gate unconditionally.
    pub fn finish_opening_blend(&mut self) {
        self.opening_movie_locked = false;
    }
}

pub const OPENING_BLEND_SECONDS: f32 = 0.5;
pub const OPENING_NO_BLEND_FLAG: u32 = 0x0080_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpeningMoviePhase {
    Playing,
    Blending,
    Released,
    AlternatePath,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraBlend {
    pub elapsed_seconds: f32,
    pub duration_seconds: f32,
}

impl CameraBlend {
    pub const fn opening() -> Self {
        Self {
            elapsed_seconds: 0.0,
            duration_seconds: OPENING_BLEND_SECONDS,
        }
    }

    /// Advance using the camera owner's elapsed time. The owner reports completion; this type
    /// does not invent a frame cadence or infer completion from wall time.
    pub fn advance(&mut self, delta_seconds: f32) -> bool {
        if delta_seconds.is_finite() && delta_seconds > 0.0 {
            self.elapsed_seconds =
                (self.elapsed_seconds + delta_seconds).min(self.duration_seconds);
        }
        self.elapsed_seconds >= self.duration_seconds
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpeningMovieClock {
    pub frame_count: u32,
    pub tick_hz: f32,
    pub uses_media_clock: bool,
    pub looping: bool,
    pub camera_flags: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OpeningMovieTransition {
    WaitingForEndpoint,
    BeginBlend(CameraBlend),
    BlendInProgress,
    Released,
    AlternatePath,
}

/// The measured nonloop opening path: source clip clock, a strict movie endpoint, half-second
/// camera blend, then unconditional input-gate cleanup. Other media/loop/policy branches remain
/// owned by their runtime systems and are deliberately not guessed here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpeningMovieRelease {
    pub phase: OpeningMoviePhase,
    pub gate: InputGateState,
    pub blend: Option<CameraBlend>,
}

impl Default for OpeningMovieRelease {
    fn default() -> Self {
        Self {
            phase: OpeningMoviePhase::Playing,
            gate: InputGateState {
                opening_movie_locked: true,
            },
            blend: None,
        }
    }
}

impl OpeningMovieRelease {
    /// Admit only the documented nonloop, animation-clock branch. The endpoint is strictly
    /// greater than `(frame_count - 1) / tick_hz`, matching the original comparison.
    pub fn advance_movie_clock(
        &mut self,
        clock: OpeningMovieClock,
        elapsed_seconds: f32,
    ) -> OpeningMovieTransition {
        if self.phase != OpeningMoviePhase::Playing {
            return match self.phase {
                OpeningMoviePhase::Blending => OpeningMovieTransition::BlendInProgress,
                OpeningMoviePhase::Released => OpeningMovieTransition::Released,
                OpeningMoviePhase::AlternatePath => OpeningMovieTransition::AlternatePath,
                OpeningMoviePhase::Playing => unreachable!(),
            };
        }
        if clock.frame_count == 0
            || !clock.tick_hz.is_finite()
            || clock.tick_hz <= 0.0
            || clock.uses_media_clock
            || clock.looping
            || clock.camera_flags & OPENING_NO_BLEND_FLAG != 0
        {
            self.phase = OpeningMoviePhase::AlternatePath;
            return OpeningMovieTransition::AlternatePath;
        }
        let endpoint = (clock.frame_count.saturating_sub(1) as f32) / clock.tick_hz;
        if !elapsed_seconds.is_finite() || elapsed_seconds <= endpoint {
            return OpeningMovieTransition::WaitingForEndpoint;
        }
        let blend = CameraBlend::opening();
        self.blend = Some(blend);
        self.phase = OpeningMoviePhase::Blending;
        OpeningMovieTransition::BeginBlend(blend)
    }

    /// Called only when the camera owner advances the admitted blend. Cleanup clears the gate
    /// after the blend reaches its recorded duration.
    pub fn advance_blend(&mut self, delta_seconds: f32) -> OpeningMovieTransition {
        if self.phase != OpeningMoviePhase::Blending {
            return match self.phase {
                OpeningMoviePhase::Playing => OpeningMovieTransition::WaitingForEndpoint,
                OpeningMoviePhase::Released => OpeningMovieTransition::Released,
                OpeningMoviePhase::AlternatePath => OpeningMovieTransition::AlternatePath,
                OpeningMoviePhase::Blending => unreachable!(),
            };
        }
        let Some(blend) = self.blend.as_mut() else {
            self.phase = OpeningMoviePhase::AlternatePath;
            return OpeningMovieTransition::AlternatePath;
        };
        if !blend.advance(delta_seconds) {
            return OpeningMovieTransition::BlendInProgress;
        }
        self.gate.finish_opening_blend();
        self.phase = OpeningMoviePhase::Released;
        OpeningMovieTransition::Released
    }
}

/// Segment progression inputs recovered for the first-level SCAM camera sequence.
///
/// The animation/camera owner supplies segment completion; this value does not guess durations,
/// interpolate camera points, or turn a wall-clock timeout into a completion event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraSequence {
    pub camera: CameraInstance,
    pub segment_index: u16,
    pub segment_count: u16,
    /// The release guard reads bit 1 from the first segment, not the segment just exhausted.
    pub first_segment_flags: u8,
    pub complete: bool,
}

impl CameraSequence {
    pub const fn new(
        camera: CameraInstance,
        segment_count: u16,
        first_segment_flags: u8,
    ) -> Option<Self> {
        if segment_count == 0 {
            None
        } else {
            Some(Self {
                camera,
                segment_index: 0,
                segment_count,
                first_segment_flags,
                complete: false,
            })
        }
    }

    /// Advance after the camera owner reports the current segment exhausted.
    pub fn complete_segment(
        &mut self,
        active_camera: Option<CameraInstance>,
        attract_mode: bool,
    ) -> CameraSequenceTransition {
        if self.complete {
            return CameraSequenceTransition::AlreadyComplete;
        }
        self.segment_index = self.segment_index.saturating_add(1);
        if self.segment_index < self.segment_count {
            return CameraSequenceTransition::NextSegment(self.segment_index);
        }
        self.complete = true;
        if self.first_segment_flags & 2 != 0 {
            CameraSequenceTransition::HoldControl
        } else if attract_mode {
            CameraSequenceTransition::AttractMode
        } else if active_camera != Some(self.camera) {
            CameraSequenceTransition::InactiveCamera
        } else {
            CameraSequenceTransition::ReleaseControl
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraSequenceTransition {
    NextSegment(u16),
    HoldControl,
    AttractMode,
    InactiveCamera,
    ReleaseControl,
    AlreadyComplete,
    Released { local_players: u16 },
}

/// Typed player/session fields affected by the 0xd4be0 control release.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CinematicPlayer {
    pub local: bool,
    pub camera: Option<CameraInstance>,
    pub mode: u32,
    pub previous_mode: u32,
    pub timer: u32,
    pub flags: u32,
    pub selected_inventory: u32,
    pub saved_inventory: u32,
    pub camera_binding: u32,
    pub saved_camera_binding: u32,
}

/// Rust-owned state for the cinematic control session. The host supplies player records and
/// callback dispatch; the session owns the release ordering and typed state mutation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CinematicSession {
    pub sequence: CameraSequence,
    pub global_flags: u32,
    pub finish_callback: Option<CinematicCallback>,
    pub update_callback: Option<CinematicCallback>,
}

impl CinematicSession {
    /// Advance one segment and perform the native release only on the measured natural path:
    /// exhausted sequence, first-segment hold bit clear, non-attract mode, and active camera match.
    /// Callback dispatch runs after the global gate clears and before callback fields are cleared.
    pub fn complete_segment(
        &mut self,
        active_camera: Option<CameraInstance>,
        attract_mode: bool,
        players: &mut [CinematicPlayer],
        mut invoke_finish: impl FnMut(CinematicCallback),
    ) -> CameraSequenceTransition {
        let transition = self.sequence.complete_segment(active_camera, attract_mode);
        if transition != CameraSequenceTransition::ReleaseControl {
            return transition;
        }
        if !players
            .iter()
            .any(|player| player.local && player.camera == Some(self.sequence.camera))
        {
            return CameraSequenceTransition::InactiveCamera;
        }

        self.global_flags &= !CINEMATIC_CONTROL_FLAG;
        if let Some(callback) = self.finish_callback {
            invoke_finish(callback);
        }
        self.finish_callback = None;
        self.update_callback = None;

        let mut local_players = 0u16;
        for player in players.iter_mut().filter(|player| player.local) {
            local_players = local_players.saturating_add(1);
            player.camera_binding = player.saved_camera_binding;
            if player.mode != 0x10 {
                player.previous_mode = player.mode;
                player.flags &= !STORY_DEATH_OR_CONTROL_FLAGS;
                player.mode = GAMEPLAY_PLAYER_MODE;
                player.timer = 0;
            }
            if player.selected_inventory == 0 {
                player.selected_inventory = if player.saved_inventory == 0 {
                    1
                } else {
                    player.saved_inventory
                };
            }
        }
        CameraSequenceTransition::Released { local_players }
    }
}

/// Result of the original 0x228870 aggregate. Success and failure bits are independent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionResultEvent {
    AllPlayersDown,
    ObjectivesFailed,
    ObjectiveFailedPending,
    Complete,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectiveState {
    /// Original slot +8 is zero for entries included in the aggregate.
    pub included: bool,
    pub status: u8,
    /// Original slot +9 bit 1 (`0x2`) counts as finished.
    pub flags: u8,
}

/// Story's result-relevant match phase. Unknown phases round-trip without inventing meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoryMatchPhase {
    Phase8,
    Other(u32),
}

impl StoryMatchPhase {
    pub const fn from_raw(value: u32) -> Self {
        if value == 8 {
            Self::Phase8
        } else {
            Self::Other(value)
        }
    }

    pub const fn raw(self) -> u32 {
        match self {
            Self::Phase8 => 8,
            Self::Other(value) => value,
        }
    }
}

/// Rust-owned result and transition state. Guest addresses belong only in the lane adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StoryResultState {
    pub result_flags: u32,
    pub phase: StoryMatchPhase,
    pub transition_timer: u32,
}

impl Default for StoryMatchPhase {
    fn default() -> Self {
        Self::Other(0)
    }
}

/// Input snapshot for one result tick; objective entries are already typed by the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryResultInput<'a> {
    pub mode_type: u8,
    pub player_count: i32,
    pub all_players_down: bool,
    pub objective_counter: i32,
    pub frame: u32,
    pub objectives: &'a [ObjectiveState],
}

/// Result tick output. End handling is delegated to the guarded transition contract.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StoryResultStep {
    pub state: StoryResultState,
    pub event: Option<MissionResultEvent>,
    pub auto_finish_flagged: bool,
    pub request_end: bool,
}

/// Apply the typed result reducer while keeping phase/timer mutation at the end-transition seam.
/// A result request is not proof of a profile write; persistence remains a separate contract.
pub fn reduce_story_result(
    state: StoryResultState,
    input: StoryResultInput<'_>,
) -> StoryResultStep {
    let evaluation = evaluate_mission(
        input.mode_type,
        input.player_count,
        input.all_players_down,
        state.result_flags,
        input.objective_counter,
        input.frame,
        input.objectives.iter().copied(),
    );
    StoryResultStep {
        state: StoryResultState {
            result_flags: evaluation.result_flags,
            ..state
        },
        event: evaluation.event,
        auto_finish_flagged: evaluation.auto_finish_flagged,
        request_end: evaluation.request_end,
    }
}

/// The generic Story event executor's explicit completion action, selected by definition type
/// `0x58`. This is a second success route beside objective-slot aggregation; whether a particular
/// Story setup selects it is a separate data question.
pub const STORY_EVENT_COMPLETION_TYPE: u32 = 0x58;

/// Apply the state effect of one typed Story event action. Guest event records and dispatch remain
/// at the adapter boundary; this reducer owns only the result word and end request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StoryEventCompletionStep {
    pub state: StoryResultState,
    pub completed: bool,
    pub request_end: bool,
}

pub fn reduce_story_event_completion(
    state: StoryResultState,
    definition_type: u32,
) -> StoryEventCompletionStep {
    let completed = definition_type == STORY_EVENT_COMPLETION_TYPE;
    StoryEventCompletionStep {
        state: StoryResultState {
            result_flags: if completed {
                state.result_flags | RESULT_SUCCESS
            } else {
                state.result_flags
            },
            ..state
        },
        completed,
        request_end: completed,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoryEndRoute {
    OrdinaryEnd,
    DescriptorCompletion,
}

/// Typed end-transition input after the neighboring role/session contracts have run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryEndInput {
    pub mode_type: u8,
    pub route_flags: u32,
    pub state: StoryResultState,
    pub online_role_2: bool,
    pub session_allows: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryEndPlan {
    pub admitted: bool,
    pub notify_players: bool,
    pub route: StoryEndRoute,
    pub enter_phase_8: bool,
    pub next_state: StoryResultState,
}

/// Plan the phase-8 transition without accessing guest memory or asserting save success.
pub fn plan_story_end(input: StoryEndInput) -> StoryEndPlan {
    let descriptor = input.route_flags & (0x40 | 0x0800_0000) != 0;
    let enter_phase_8 = descriptor && input.state.phase != StoryMatchPhase::Phase8;
    StoryEndPlan {
        admitted: !input.online_role_2 || input.session_allows,
        notify_players: input.mode_type != 3 && input.mode_type != 7,
        route: if descriptor {
            StoryEndRoute::DescriptorCompletion
        } else {
            StoryEndRoute::OrdinaryEnd
        },
        enter_phase_8,
        next_state: if enter_phase_8 {
            StoryResultState {
                phase: StoryMatchPhase::Phase8,
                transition_timer: 0,
                ..input.state
            }
        } else {
            input.state
        },
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MissionEvaluation {
    pub event: Option<MissionResultEvent>,
    pub result_flags: u32,
    pub auto_finish_flagged: bool,
    /// Requests 0x67450(0). Its role/flags guard, not this reducer, owns phase-8 admission.
    pub request_end: bool,
}

pub fn evaluate_mission<I>(
    mode_type: u8,
    player_count: i32,
    all_players_down: bool,
    result_flags: u32,
    objective_counter: i32,
    frame: u32,
    objectives: I,
) -> MissionEvaluation
where
    I: IntoIterator<Item = ObjectiveState>,
{
    if result_flags & RESULT_SUCCESS == 0
        && mode_type == MODE_STORY
        && (player_count <= 0 || all_players_down)
    {
        return MissionEvaluation {
            event: Some(MissionResultEvent::AllPlayersDown),
            result_flags: result_flags | RESULT_FAILURE,
            auto_finish_flagged: false,
            request_end: true,
        };
    }
    if result_flags & RESULT_SUCCESS != 0 || objective_counter <= 0 {
        return MissionEvaluation {
            result_flags,
            ..MissionEvaluation::default()
        };
    }

    let (mut total, mut failed, mut done) = (0usize, 0usize, 0usize);
    for slot in objectives.into_iter().filter(|slot| slot.included) {
        total += 1;
        if slot.status == 4 {
            failed += 1;
        } else if slot.status == 5 || slot.flags & 2 != 0 {
            done += 1;
        }
    }
    if failed != 0 {
        if failed + done == total {
            MissionEvaluation {
                event: Some(MissionResultEvent::ObjectivesFailed),
                result_flags: result_flags | RESULT_FAILURE,
                auto_finish_flagged: false,
                request_end: true,
            }
        // The lifted body sign-extends the frame dword before signed division by ten.
        } else if (frame as i32) % 10 == 0 {
            MissionEvaluation {
                event: Some(MissionResultEvent::ObjectiveFailedPending),
                result_flags: result_flags | RESULT_FAILURE,
                auto_finish_flagged: false,
                request_end: false,
            }
        } else {
            MissionEvaluation {
                result_flags,
                ..MissionEvaluation::default()
            }
        }
    } else if done == total {
        MissionEvaluation {
            event: Some(MissionResultEvent::Complete),
            result_flags: result_flags | RESULT_SUCCESS,
            auto_finish_flagged: true,
            request_end: true,
        }
    } else {
        MissionEvaluation {
            result_flags,
            ..MissionEvaluation::default()
        }
    }
}

/// Explicit runtime data needed to continue after the phase-8 Story success handoff.
/// Both records are decoded from original tables by the caller; this type carries no assets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryContinuation {
    next_ordinal: CampaignOrdinal,
    post_movie: Option<SessionSetup>,
    next_gameplay: SessionSetup,
}

impl StoryContinuation {
    /// Build a continuation only when its runtime-selected level belongs to the decoded
    /// campaign table. This keeps the gameplay ID and profile ordinal linked at the boundary.
    pub fn from_runtime_levels(
        levels: CampaignLevelTable,
        post_movie: Option<SessionSetup>,
        next_gameplay: SessionSetup,
    ) -> Option<Self> {
        if next_gameplay.byte(0x55) != MODE_STORY
            || post_movie.is_some_and(|setup| setup.byte(0x55) != MODE_STORY)
        {
            return None;
        }
        let next_ordinal = levels.ordinal_for_level_id(next_gameplay.level_id())?;
        Some(Self {
            next_ordinal,
            post_movie,
            next_gameplay,
        })
    }

    /// Campaign profile ordinal resolved from the runtime-selected gameplay setup.
    pub const fn next_ordinal(&self) -> CampaignOrdinal {
        self.next_ordinal
    }

    /// Optional Story movie setup selected between the result screen and gameplay.
    pub const fn post_movie(&self) -> Option<SessionSetup> {
        self.post_movie
    }

    /// Story gameplay setup whose level ID resolved to `next_ordinal`.
    pub const fn next_gameplay(&self) -> SessionSetup {
        self.next_gameplay
    }
}

/// Frontend readiness conditions for the recovered Story continuation descriptor. The runtime
/// supplies readiness and unlock-notice state; the flow does not manufacture either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryContinuationGate {
    pub descriptor_ready: bool,
    pub attract_mode: bool,
    pub session_phase: u32,
    pub menu_kind: u8,
    pub unlock_notice_pending: bool,
}

impl StoryContinuationGate {
    pub const fn admits(self) -> bool {
        self.descriptor_ready
            && !self.attract_mode
            && self.session_phase == 1
            && self.menu_kind == 4
            && !self.unlock_notice_pending
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoryStep {
    Gameplay {
        ordinal: CampaignOrdinal,
        level_id: i32,
    },
    ResultsFrontend,
    PostMovie {
        ordinal: CampaignOrdinal,
        level_id: i32,
    },
    NextGameplay {
        ordinal: CampaignOrdinal,
        level_id: i32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryFlow {
    pub staging: SessionSetup,
    pub active: SessionSetup,
    pub level_id: i32,
    pub player_count: i32,
    /// Phase is separate from setup-derived mode/level identifier.
    pub phase: u32,
    /// Result flags are a bitset; both 0x8000 and 0x10000 may be present.
    pub result_flags: u32,
    /// Timer carried with the result reducer state; it is changed only by that reducer or its
    /// neighboring transition contract.
    pub transition_timer: u32,
    pub ordinal: CampaignOrdinal,
    pub step: StoryStep,
    pending_next: Option<(CampaignOrdinal, SessionSetup)>,
}

impl Default for StoryFlow {
    fn default() -> Self {
        Self {
            staging: SessionSetup::default(),
            active: SessionSetup::default(),
            level_id: 0,
            player_count: 0,
            phase: 0,
            result_flags: 0,
            transition_timer: 0,
            ordinal: CampaignOrdinal::default(),
            step: StoryStep::Gameplay {
                ordinal: CampaignOrdinal::default(),
                level_id: 0,
            },
            pending_next: None,
        }
    }
}

impl StoryFlow {
    pub fn stage(&mut self, setup: SessionSetup) {
        self.staging = setup;
    }

    /// Mirrors the typed portion of 0x670C0 after the 0x658D0 staging copy.
    pub fn start_level(&mut self) {
        self.active = self.staging;
        // 0x670C0 initializes the result word and runtime slot counters in its active copy.
        for offset in ACTIVATION_RESET_OFFSETS {
            self.active.words[offset / 4] = 0;
        }
        // 0x670C0 reapplies each setup item's active byte after clearing the slot counters.
        // Only item IDs whose target remains inside the typed 0xA0-byte setup are projected
        // here; the original setup bytes, including any out-of-record IDs, stay preserved.
        let item_count = usize::from(self.active.byte(SETUP_ITEM_COUNT_OFFSET))
            .min(SETUP_BYTES - SETUP_ITEM_IDS_OFFSET);
        for item_index in 0..item_count {
            let item_id = usize::from(self.active.byte(SETUP_ITEM_IDS_OFFSET + item_index));
            let active_offset = ACTIVE_ITEM_FLAGS_OFFSET + item_id;
            if active_offset < SETUP_BYTES {
                self.active.set_byte(active_offset, 1);
            }
        }
        // The lifted handler's final setup write clears +0x88 after item activation.
        self.active.words[FINAL_ACTIVATION_CLEAR_OFFSET / 4] = 0;
        self.level_id = self.active.level_id();
        self.player_count = self.active.player_count();
        self.phase = 2;
        self.result_flags = 0;
        self.step = StoryStep::Gameplay {
            ordinal: self.ordinal,
            level_id: self.level_id,
        };
        self.pending_next = None;
    }

    pub fn set_result_flags(&mut self, flags: u32) {
        self.result_flags = flags;
    }

    /// Transfer the complete typed reducer state into the Story flow owner.
    pub fn apply_result_state(&mut self, state: StoryResultState) {
        self.result_flags = state.result_flags;
        self.phase = state.phase.raw();
        self.transition_timer = state.transition_timer;
    }

    /// Read the result state consumed by the typed result reducer and persistence bridge.
    pub fn result_state(&self) -> StoryResultState {
        StoryResultState {
            result_flags: self.result_flags,
            phase: StoryMatchPhase::from_raw(self.phase),
            transition_timer: self.transition_timer,
        }
    }

    /// Stages the frontend record after the original phase-8 success predicate accepted the route.
    /// The caller owns that full predicate; this method checks the result bit and type-10 setup.
    pub fn stage_success_frontend(&mut self, accepted_by_phase8_writer: bool) -> bool {
        if !accepted_by_phase8_writer
            || self.result_flags & RESULT_SUCCESS == 0
            || self.active.byte(0x55) != MODE_STORY
        {
            return false;
        }
        let mut setup = self.active;
        setup.set_byte(0x54, 0x6a);
        setup.set_byte(0x55, 9);
        setup.set_byte(0x56, 3);
        self.staging = setup;
        self.step = StoryStep::ResultsFrontend;
        self.pending_next = None;
        true
    }

    /// Uses the original row/configuration data supplied by the runtime loader.
    pub fn continue_after_results(
        &mut self,
        gate: StoryContinuationGate,
        route: StoryContinuation,
    ) -> bool {
        if !gate.admits()
            || self.step != StoryStep::ResultsFrontend
            || self.staging.level_id() != 0x6a
        {
            return false;
        }
        self.pending_next = Some((route.next_ordinal, route.next_gameplay));
        if let Some(movie) = route.post_movie {
            self.staging = movie;
            self.step = StoryStep::PostMovie {
                ordinal: self.ordinal,
                level_id: movie.level_id(),
            };
        } else {
            self.staging = route.next_gameplay;
            self.ordinal = route.next_ordinal;
            self.step = StoryStep::NextGameplay {
                ordinal: route.next_ordinal,
                level_id: route.next_gameplay.level_id(),
            };
        }
        true
    }

    /// Called when the supplied post-movie/skip path returns to gameplay setup.
    pub fn finish_post_movie(&mut self) -> bool {
        let Some((ordinal, next)) = self.pending_next else {
            return false;
        };
        if !matches!(self.step, StoryStep::PostMovie { .. }) {
            return false;
        }
        self.staging = next;
        self.ordinal = ordinal;
        self.step = StoryStep::NextGameplay {
            ordinal,
            level_id: next.level_id(),
        };
        self.pending_next = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn setup(level_id: u8, mode_type: u8, players: u8) -> SessionSetup {
        let mut setup = SessionSetup::default();
        setup.set_byte(0, players);
        setup.set_byte(0x54, level_id);
        setup.set_byte(0x55, mode_type);
        setup
    }

    fn ordinal(value: u8) -> CampaignOrdinal {
        CampaignOrdinal::new(value).unwrap()
    }

    #[test]
    fn campaign_ordinal_is_bounded_by_the_profile_table() {
        assert_eq!(ordinal(CampaignOrdinal::COUNT - 1).get(), 12);
        assert!(CampaignOrdinal::new(CampaignOrdinal::COUNT).is_none());
    }

    #[test]
    fn campaign_level_lookup_uses_signed_ids_and_first_matching_row() {
        let table = CampaignLevelTable::new([2, 4, -1, 7, 9, 11, 13, 15, 17, 19, 21, 23, 4]);
        assert_eq!(
            table.ordinal_for_level_id(2).map(CampaignOrdinal::get),
            Some(0)
        );
        assert_eq!(
            table.ordinal_for_level_id(-1).map(CampaignOrdinal::get),
            Some(2)
        );
        assert_eq!(
            table.ordinal_for_level_id(4).map(CampaignOrdinal::get),
            Some(1)
        );
        assert_eq!(table.ordinal_for_level_id(65535), None);
        assert_eq!(table.ordinal_for_level_id(24), None);
    }

    #[test]
    fn campaign_runtime_rows_decode_signed_ids_at_measured_stride() {
        let required = (CampaignOrdinal::COUNT as usize - 1) * CAMPAIGN_LEVEL_ROW_STRIDE_BYTES + 2;
        let mut rows = alloc::vec![0xa5; required];
        for (index, id) in [2i16, 4, -1, 7, 9, 11, 13, 15, 17, 19, 21, 23, 4]
            .into_iter()
            .enumerate()
        {
            let offset = index * CAMPAIGN_LEVEL_ROW_STRIDE_BYTES;
            rows[offset..offset + 2].copy_from_slice(&id.to_le_bytes());
        }

        let table = CampaignLevelTable::decode_runtime_rows(&rows).unwrap();
        assert_eq!(
            table.ordinal_for_level_id(-1).map(CampaignOrdinal::get),
            Some(2)
        );
        assert_eq!(
            table.ordinal_for_level_id(4).map(CampaignOrdinal::get),
            Some(1)
        );
        assert_eq!(table.ordinal_for_level_id(65535), None);
        assert_eq!(
            CampaignLevelTable::decode_runtime_rows(&rows[..required - 1]),
            Err(CampaignLevelTableDecodeError::TooShort {
                required,
                actual: required - 1,
            })
        );
    }

    #[test]
    fn continuation_derives_ordinal_from_selected_level_and_rejects_unknown_levels() {
        let levels = CampaignLevelTable::new([2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26]);
        let route =
            StoryContinuation::from_runtime_levels(levels, None, setup(4, MODE_STORY, 1)).unwrap();
        assert_eq!(route.next_ordinal, ordinal(1));
        assert!(
            StoryContinuation::from_runtime_levels(levels, None, setup(9, MODE_STORY, 1),)
                .is_none()
        );
        assert!(StoryContinuation::from_runtime_levels(levels, None, setup(4, 9, 1)).is_none());
    }

    #[test]
    fn story_flow_owns_the_full_result_reducer_state() {
        let state = StoryResultState {
            result_flags: RESULT_SUCCESS | RESULT_FAILURE | 0x40,
            phase: StoryMatchPhase::from_raw(8),
            transition_timer: 0x1234_5678,
        };
        let mut flow = StoryFlow::default();
        flow.apply_result_state(state);
        assert_eq!(flow.result_state(), state);

        let unknown_phase = StoryResultState {
            phase: StoryMatchPhase::from_raw(0xfeed),
            ..state
        };
        flow.apply_result_state(unknown_phase);
        assert_eq!(flow.phase, 0xfeed);
        assert_eq!(flow.result_state(), unknown_phase);
    }

    #[test]
    fn start_keeps_setup_identifier_separate_from_phase_and_selects_story_controller() {
        let mut flow = StoryFlow::default();
        flow.stage(setup(2, MODE_STORY, 1));
        flow.start_level();
        assert_eq!((flow.level_id, flow.player_count, flow.phase), (2, 1, 2));
        assert_eq!(
            flow.step,
            StoryStep::Gameplay {
                ordinal: ordinal(0),
                level_id: 2
            }
        );
        assert_eq!(
            story_controller(flow.level_id, flow.active.byte(0x55)),
            Some(STORY_CONTROLLERS[0])
        );

        flow.stage(setup(0xff, MODE_STORY, 0xff));
        flow.start_level();
        assert_eq!((flow.level_id, flow.player_count), (-1, -1));
        assert_eq!(story_controller(24, MODE_STORY), None);
        assert_eq!(story_controller(2, 9), None);
    }

    #[test]
    fn success_handoff_uses_original_table_records_for_movie_and_next_level() {
        let mut flow = StoryFlow::default();
        flow.stage(setup(2, MODE_STORY, 1));
        flow.start_level();
        flow.set_result_flags(RESULT_SUCCESS | RESULT_FAILURE | 0x40);
        assert!(!flow.stage_success_frontend(false));
        assert!(flow.stage_success_frontend(true));
        assert_eq!(flow.staging.level_id(), 0x6a);
        assert_eq!((flow.staging.byte(0x55), flow.staging.byte(0x56)), (9, 3));

        let movie = setup(0x2c, MODE_STORY, 1);
        let next = setup(4, MODE_STORY, 1);
        let route = StoryContinuation::from_runtime_levels(
            CampaignLevelTable::new([2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26]),
            Some(movie),
            next,
        )
        .unwrap();
        let admitted = StoryContinuationGate {
            descriptor_ready: true,
            attract_mode: false,
            session_phase: 1,
            menu_kind: 4,
            unlock_notice_pending: false,
        };
        assert!(flow.continue_after_results(admitted, route));
        assert_eq!(flow.staging.level_id(), 0x2c);
        assert_eq!(
            flow.step,
            StoryStep::PostMovie {
                ordinal: ordinal(0),
                level_id: 0x2c
            }
        );
        assert!(flow.finish_post_movie());
        assert_eq!(flow.staging.level_id(), 4);
        assert_eq!(
            flow.step,
            StoryStep::NextGameplay {
                ordinal: ordinal(1),
                level_id: 4
            }
        );
        flow.start_level();
        assert_eq!(
            flow.step,
            StoryStep::Gameplay {
                ordinal: ordinal(1),
                level_id: 4
            }
        );
        // Activating the next level applies the measured result-word reset at 0x670C0.
        assert_eq!(flow.result_flags, 0);
    }

    #[test]
    fn success_handoff_rejects_second_pass_until_post_movie_finishes() {
        let mut flow = StoryFlow::default();
        flow.stage(setup(2, MODE_STORY, 1));
        flow.start_level();
        flow.set_result_flags(RESULT_SUCCESS);
        assert!(flow.stage_success_frontend(true));

        let route = StoryContinuation::from_runtime_levels(
            CampaignLevelTable::new([2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26]),
            Some(setup(0x2c, MODE_STORY, 1)),
            setup(4, MODE_STORY, 1),
        )
        .unwrap();
        let admitted = StoryContinuationGate {
            descriptor_ready: true,
            attract_mode: false,
            session_phase: 1,
            menu_kind: 4,
            unlock_notice_pending: false,
        };

        assert!(flow.continue_after_results(admitted, route));
        assert!(!flow.continue_after_results(admitted, route));
        assert_eq!(flow.staging.level_id(), 0x2c);
        assert!(flow.finish_post_movie());
        assert!(!flow.finish_post_movie());
        // Completing the post-movie already commits the pending next gameplay setup.
        assert!(!flow.continue_after_results(admitted, route));
        assert_eq!(flow.staging.level_id(), 4);
        assert_eq!(
            flow.step,
            StoryStep::NextGameplay {
                ordinal: ordinal(1),
                level_id: 4
            }
        );
    }

    #[test]
    fn continuation_waits_for_the_story_frontend_route_and_accepts_skip_path() {
        let mut flow = StoryFlow::default();
        flow.stage(setup(2, MODE_STORY, 1));
        flow.start_level();
        flow.set_result_flags(RESULT_SUCCESS);
        assert!(flow.stage_success_frontend(true));

        let route = StoryContinuation::from_runtime_levels(
            CampaignLevelTable::new([2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26]),
            None,
            setup(4, MODE_STORY, 1),
        )
        .unwrap();
        let denied = [
            StoryContinuationGate {
                descriptor_ready: false,
                attract_mode: false,
                session_phase: 1,
                menu_kind: 4,
                unlock_notice_pending: false,
            },
            StoryContinuationGate {
                descriptor_ready: true,
                attract_mode: true,
                session_phase: 1,
                menu_kind: 4,
                unlock_notice_pending: false,
            },
            StoryContinuationGate {
                descriptor_ready: true,
                attract_mode: false,
                session_phase: 2,
                menu_kind: 4,
                unlock_notice_pending: false,
            },
            StoryContinuationGate {
                descriptor_ready: true,
                attract_mode: false,
                session_phase: 1,
                menu_kind: 3,
                unlock_notice_pending: false,
            },
            StoryContinuationGate {
                descriptor_ready: true,
                attract_mode: false,
                session_phase: 1,
                menu_kind: 4,
                unlock_notice_pending: true,
            },
        ];
        for gate in denied {
            assert!(!flow.continue_after_results(gate, route));
            assert_eq!(flow.step, StoryStep::ResultsFrontend);
            assert_eq!(flow.staging.level_id(), 0x6a);
        }

        let admitted = StoryContinuationGate {
            descriptor_ready: true,
            attract_mode: false,
            session_phase: 1,
            menu_kind: 4,
            unlock_notice_pending: false,
        };
        assert!(flow.continue_after_results(admitted, route));
        assert_eq!(flow.ordinal, ordinal(1));
        assert_eq!(flow.staging.level_id(), 4);
        assert_eq!(
            flow.step,
            StoryStep::NextGameplay {
                ordinal: ordinal(1),
                level_id: 4
            }
        );
        assert!(!flow.finish_post_movie());
        flow.start_level();
        assert_eq!(flow.level_id, 4);
        assert_eq!(flow.ordinal, ordinal(1));
    }

    #[test]
    fn opening_movie_release_uses_strict_endpoint_then_clears_gate_after_blend() {
        let clock = OpeningMovieClock {
            frame_count: 1350,
            tick_hz: 60.0,
            uses_media_clock: false,
            looping: false,
            camera_flags: 0x8110_4433 & !OPENING_NO_BLEND_FLAG,
        };
        let mut release = OpeningMovieRelease::default();
        assert_eq!(
            release.advance_movie_clock(clock, 1349.0 / 60.0),
            OpeningMovieTransition::WaitingForEndpoint
        );
        assert!(release.gate.opening_movie_locked);
        assert_eq!(
            release.advance_movie_clock(clock, f32::from_bits((1349.0f32 / 60.0).to_bits() + 1),),
            OpeningMovieTransition::BeginBlend(CameraBlend::opening())
        );
        assert_eq!(
            release.advance_blend(0.5 - f32::EPSILON * 8.0),
            OpeningMovieTransition::BlendInProgress
        );
        assert!(release.gate.opening_movie_locked);
        assert_eq!(
            release.advance_blend(f32::EPSILON * 8.0),
            OpeningMovieTransition::Released
        );
        assert!(!release.gate.opening_movie_locked);
        assert_eq!(
            release.advance_blend(1.0),
            OpeningMovieTransition::Released,
            "release is idempotent once the camera owner completed cleanup"
        );
    }

    #[test]
    fn opening_movie_release_does_not_guess_media_loop_or_skip_paths() {
        for clock in [
            OpeningMovieClock {
                frame_count: 1350,
                tick_hz: 60.0,
                uses_media_clock: true,
                looping: false,
                camera_flags: 0,
            },
            OpeningMovieClock {
                frame_count: 1350,
                tick_hz: 60.0,
                uses_media_clock: false,
                looping: true,
                camera_flags: 0,
            },
            OpeningMovieClock {
                frame_count: 1350,
                tick_hz: 60.0,
                uses_media_clock: false,
                looping: false,
                camera_flags: OPENING_NO_BLEND_FLAG,
            },
        ] {
            let mut release = OpeningMovieRelease::default();
            assert_eq!(
                release.advance_movie_clock(clock, 1000.0),
                OpeningMovieTransition::AlternatePath
            );
            assert!(release.gate.opening_movie_locked);
            assert_eq!(release.blend, None);
        }
    }

    #[test]
    fn mission_aggregate_preserves_nonexclusive_flags_and_empty_table_edge() {
        let complete = evaluate_mission(MODE_STORY, 1, false, RESULT_FAILURE, 1, 0, []);
        assert_eq!(complete.event, Some(MissionResultEvent::Complete));
        assert_eq!(complete.result_flags, RESULT_SUCCESS | RESULT_FAILURE);
        assert!(complete.auto_finish_flagged);

        let pending = evaluate_mission(
            MODE_STORY,
            1,
            false,
            0,
            1,
            20,
            [
                ObjectiveState {
                    included: true,
                    status: 4,
                    flags: 0,
                },
                ObjectiveState {
                    included: true,
                    status: 2,
                    flags: 0,
                },
                ObjectiveState {
                    included: false,
                    status: 2,
                    flags: 0,
                },
            ],
        );
        assert_eq!(
            pending.event,
            Some(MissionResultEvent::ObjectiveFailedPending)
        );
        assert_eq!(pending.result_flags, RESULT_FAILURE);
    }

    #[test]
    fn cinematic_release_requires_sequence_guards_and_restores_typed_player_state() {
        let camera = CameraInstance(0x5343_414d);
        let mut sequence = CameraSequence::new(camera, 2, 0).unwrap();
        assert_eq!(
            sequence.complete_segment(Some(camera), false),
            CameraSequenceTransition::NextSegment(1)
        );
        assert_eq!(
            sequence.complete_segment(Some(camera), false),
            CameraSequenceTransition::ReleaseControl
        );

        for (first_flags, attract, active, local_camera, expected) in [
            (
                2,
                false,
                Some(camera),
                Some(camera),
                CameraSequenceTransition::HoldControl,
            ),
            (
                0,
                true,
                Some(camera),
                Some(camera),
                CameraSequenceTransition::AttractMode,
            ),
            (
                0,
                false,
                Some(CameraInstance(7)),
                Some(camera),
                CameraSequenceTransition::InactiveCamera,
            ),
            (
                0,
                false,
                Some(camera),
                None,
                CameraSequenceTransition::InactiveCamera,
            ),
        ] {
            let mut session = CinematicSession {
                sequence: CameraSequence::new(camera, 1, first_flags).unwrap(),
                global_flags: CINEMATIC_CONTROL_FLAG,
                finish_callback: Some(CinematicCallback(11)),
                update_callback: Some(CinematicCallback(12)),
            };
            let mut players = [CinematicPlayer {
                local: true,
                camera: local_camera,
                mode: CINEMATIC_PLAYER_MODE,
                selected_inventory: 0,
                saved_inventory: 0,
                ..CinematicPlayer::default()
            }];
            assert_eq!(
                session.complete_segment(active, attract, &mut players, |_| {}),
                expected
            );
            assert_ne!(session.global_flags & CINEMATIC_CONTROL_FLAG, 0);
            assert_eq!(session.finish_callback, Some(CinematicCallback(11)));
            assert_eq!(session.update_callback, Some(CinematicCallback(12)));
            assert_eq!(players[0].mode, CINEMATIC_PLAYER_MODE);
        }

        let mut session = CinematicSession {
            sequence: CameraSequence::new(camera, 1, 0).unwrap(),
            global_flags: CINEMATIC_CONTROL_FLAG | 0x40,
            finish_callback: Some(CinematicCallback(11)),
            update_callback: Some(CinematicCallback(12)),
        };
        let mut players = [
            CinematicPlayer {
                local: true,
                camera: Some(camera),
                mode: 7,
                previous_mode: 99,
                timer: 4,
                flags: 0x0002_0405,
                selected_inventory: 0,
                saved_inventory: 0,
                camera_binding: 1,
                saved_camera_binding: 3,
            },
            CinematicPlayer {
                local: false,
                mode: 7,
                timer: 4,
                ..CinematicPlayer::default()
            },
        ];
        let mut callback = None;
        assert_eq!(
            session.complete_segment(Some(camera), false, &mut players, |id| {
                callback = Some(id)
            }),
            CameraSequenceTransition::Released { local_players: 1 }
        );
        assert_eq!(callback, Some(CinematicCallback(11)));
        assert_eq!(session.global_flags, 0x40);
        assert_eq!(session.finish_callback, None);
        assert_eq!(session.update_callback, None);
        assert_eq!(players[0].previous_mode, 7);
        assert_eq!(players[0].mode, GAMEPLAY_PLAYER_MODE);
        assert_eq!(players[0].timer, 0);
        assert_eq!(players[0].flags, 5);
        assert_eq!(players[0].selected_inventory, 1);
        assert_eq!(players[0].camera_binding, 3);
        assert_eq!(
            players[1].mode, 7,
            "non-local player is not in the session slice"
        );
    }

    #[test]
    fn story_result_matches_lifted_aggregate_over_3000_fabricated_cases() {
        // Tier 1 / weak: deterministic synthetic state and a compact transcription of lifted
        // branch predicates; this does not execute machine code or prove natural ordering.
        fn lifted(
            mode: u8,
            players: i32,
            down: bool,
            flags: u32,
            counter: i32,
            frame: u32,
            slots: &[ObjectiveState],
        ) -> MissionEvaluation {
            if flags & 0x8000 == 0 && mode == 10 && (players <= 0 || down) {
                return MissionEvaluation {
                    event: Some(MissionResultEvent::AllPlayersDown),
                    result_flags: flags | 0x10000,
                    auto_finish_flagged: false,
                    request_end: true,
                };
            }
            if flags & 0x8000 != 0 || counter <= 0 {
                return MissionEvaluation {
                    result_flags: flags,
                    ..MissionEvaluation::default()
                };
            }
            let (mut total, mut failed, mut done) = (0usize, 0usize, 0usize);
            for s in slots.iter().filter(|s| s.included) {
                total += 1;
                if s.status == 4 {
                    failed += 1;
                } else if s.status == 5 || s.flags & 2 != 0 {
                    done += 1;
                }
            }
            if failed > 0 {
                if failed + done == total {
                    MissionEvaluation {
                        event: Some(MissionResultEvent::ObjectivesFailed),
                        result_flags: flags | 0x10000,
                        auto_finish_flagged: false,
                        request_end: true,
                    }
                } else if frame as i32 % 10 == 0 {
                    MissionEvaluation {
                        event: Some(MissionResultEvent::ObjectiveFailedPending),
                        result_flags: flags | 0x10000,
                        auto_finish_flagged: false,
                        request_end: false,
                    }
                } else {
                    MissionEvaluation {
                        result_flags: flags,
                        ..MissionEvaluation::default()
                    }
                }
            } else if done == total {
                MissionEvaluation {
                    event: Some(MissionResultEvent::Complete),
                    result_flags: flags | 0x8000,
                    auto_finish_flagged: true,
                    request_end: true,
                }
            } else {
                MissionEvaluation {
                    result_flags: flags,
                    ..MissionEvaluation::default()
                }
            }
        }
        let mut seed = 0x2448_2288u32;
        for _ in 0..3000 {
            let mut next = || {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed
            };
            let count = (next() % 9) as usize;
            let slots = (0..count)
                .map(|_| ObjectiveState {
                    included: next() % 4 != 0,
                    status: (next() % 7) as u8,
                    flags: (next() & 7) as u8,
                })
                .collect::<Vec<_>>();
            let (mode, players, down, flags, counter, frame) = (
                (next() % 12) as u8,
                (next() % 5) as i32 - 1,
                next() & 1 != 0,
                next() & 0x1_8040,
                (next() % 4) as i32 - 1,
                next(),
            );
            let actual = evaluate_mission(
                mode,
                players,
                down,
                flags,
                counter,
                frame,
                slots.iter().copied(),
            );
            assert_eq!(
                actual,
                lifted(mode, players, down, flags, counter, frame, &slots)
            );
            let prior_state = StoryResultState {
                result_flags: flags,
                phase: StoryMatchPhase::from_raw(next()),
                transition_timer: next(),
            };
            let step = reduce_story_result(
                prior_state,
                StoryResultInput {
                    mode_type: mode,
                    player_count: players,
                    all_players_down: down,
                    objective_counter: counter,
                    frame,
                    objectives: &slots,
                },
            );
            assert_eq!(step.state.result_flags, actual.result_flags);
            assert_eq!(step.event, actual.event);
            assert_eq!(step.auto_finish_flagged, actual.auto_finish_flagged);
            assert_eq!(step.request_end, actual.request_end);
            assert_eq!(step.state.phase, prior_state.phase);
            assert_eq!(step.state.transition_timer, prior_state.transition_timer);
        }
    }

    #[test]
    fn result_bit_mutation_is_observable() {
        let slots = [ObjectiveState {
            included: true,
            status: 5,
            flags: 0,
        }];
        let state = StoryResultState {
            result_flags: 0,
            phase: StoryMatchPhase::Other(3),
            transition_timer: 17,
        };
        let input = StoryResultInput {
            mode_type: MODE_STORY,
            player_count: 1,
            all_players_down: false,
            objective_counter: 1,
            frame: 0,
            objectives: &slots,
        };
        let expected = reduce_story_result(state, input);
        let mutated = reduce_story_result(
            StoryResultState {
                result_flags: RESULT_SUCCESS,
                ..state
            },
            input,
        );
        assert_ne!(expected, mutated);
        assert_eq!(expected.state.result_flags, RESULT_SUCCESS);
        assert_eq!(mutated.event, None);
        assert_eq!(expected.state.phase, state.phase);
        assert_eq!(expected.state.transition_timer, state.transition_timer);
    }

    #[test]
    fn story_success_writer_preserves_preexisting_failure_bit() {
        // REA shows separate writers OR these result bits without clearing the other bit.
        // This typed reducer case checks that coexistence without claiming natural write order.
        let prior = StoryResultState {
            result_flags: RESULT_FAILURE,
            phase: StoryMatchPhase::Other(3),
            transition_timer: 17,
        };
        let objective = [ObjectiveState {
            included: true,
            status: 5,
            flags: 0,
        }];
        let tick = reduce_story_result(
            prior,
            StoryResultInput {
                mode_type: MODE_STORY,
                player_count: 1,
                all_players_down: false,
                objective_counter: 1,
                frame: 1,
                objectives: &objective,
            },
        );
        assert_eq!(tick.event, Some(MissionResultEvent::Complete));
        assert_eq!(tick.state.result_flags, RESULT_SUCCESS | RESULT_FAILURE);

        let explicit = reduce_story_event_completion(prior, STORY_EVENT_COMPLETION_TYPE);
        assert!(explicit.completed);
        assert_eq!(explicit.state.result_flags, RESULT_SUCCESS | RESULT_FAILURE);
    }
}
