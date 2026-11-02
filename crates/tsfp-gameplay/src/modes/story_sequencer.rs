// SPDX-License-Identifier: GPL-3.0-or-later
//! T2431: the Story progression sequencer `0x65940` (cursor to level) as readable Rust over a typed
//! mode block. Tier 1 against the lifted original on FABRICATED inputs (weak evidence, see
//! `docs/t2431-story-sequencer.md`); a Tier 2 replay is blocked on owner saves (T2421).
//!
//! `0x65940(block)` is called by the mode-4 branch of `0x2D2C70` (and `0x65C70`, `0x68830`) with the
//! mode block `0x78AD58`. It walks the 13-row Story cursor table and fills the block that the
//! `0x658D0` copy (`tsfp_state::story_flow::copy_setup`) then stages for `0x670C0`.
//!
//! State it reads and writes (guest cells, the Rust owner is [`SequencerState`]):
//! * cursor `[0x78AD48]` (row 0..=12) and phase `[0x78AD50]`: 0 = pre-level entry, 1 = level,
//!   2 = post-level, 3 = advance. Phase 0 and 2 build a briefing entry block, phase 1 the level block.
//! * inputs: mode flags `[0x7DE458]`, game mode `[0x79094C]`, participants byte `[0x5236C4]`,
//!   difficulty selector byte `[0x78AD54]`, `[0x78ADAF]`, result word `[0x7DE45C]`.
//!
//! Tables are read from the running guest image on every call, never embedded: the cursor table
//! `0x4D1AC0` (13 rows, stride 0x44: level word at +6, player level words at +0xC and +0xE, pre
//! entry id +0x20, post entry id +0x24) and the entry table `0x51FC30` (stride 0x24, mode byte at
//! +0). Services stay guest calls: `0x64D10` (post-level state), `0x64DD0` (per-player level byte),
//! `0x1CD590` (mark entry seen), `0x35DE70` (skip briefings, constant 0), `0x389B0` (release).
use super::ModeHost;
use crate::story_flow::{CampaignOrdinal, MODE_STORY};

/// Mode block (the staging record of `0x658D0`), 40 dwords.
pub const BLOCK_BYTES: usize = 0xa0;
/// The Story cursor table: 13 rows of 0x44 bytes.
pub const CURSOR_TABLE: u32 = 0x004d_1ac0;
pub const ROW_STRIDE: u32 = 0x44;
pub const ROWS: i32 = 13;
/// The entry table keyed by entry id (not by row).
pub const ENTRY_TABLE: u32 = 0x0051_fc30;
pub const ENTRY_STRIDE: u32 = 0x24;
pub const CURSOR: u32 = 0x0078_ad48;
pub const PHASE: u32 = 0x0078_ad50;
const SELECTOR: u32 = 0x0078_ad54;
const AUX_BYTE: u32 = 0x0078_adaf;
const PARTICIPANTS: u32 = 0x0052_36c4;
const MODE_FLAGS: u32 = 0x007d_e458;
const RESULT_WORD: u32 = 0x007d_e45c;
const GAME_MODE: u32 = 0x0079_094c;
const NONE_ID: u32 = 0xffff_ffff;

// Service callees (VAs of other systems).
const POST_LEVEL_STATE: u32 = 0x0006_4d10;
const PLAYER_LEVEL_BYTE: u32 = 0x0006_4dd0;
const MARK_ENTRY_SEEN: u32 = 0x001c_d590;
const SKIP_BRIEFINGS: u32 = 0x0035_de70;
const RELEASE_ENTRY: u32 = 0x0003_89b0;

/// Phases of the sequencer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    PreLevel,
    Level,
    PostLevel,
    Advance,
    /// Any other stored value: the original falls through every phase test and returns.
    Other(i32),
}

impl Phase {
    pub fn from_word(word: i32) -> Self {
        match word {
            0 => Self::PreLevel,
            1 => Self::Level,
            2 => Self::PostLevel,
            3 => Self::Advance,
            other => Self::Other(other),
        }
    }
    pub fn word(self) -> i32 {
        match self {
            Self::PreLevel => 0,
            Self::Level => 1,
            Self::PostLevel => 2,
            Self::Advance => 3,
            Self::Other(word) => word,
        }
    }
}

/// Rust-owned sequencer state, loaded from and stored to the two guest cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SequencerState {
    pub cursor: i32,
    pub phase: Phase,
}

impl SequencerState {
    pub fn load<H: ModeHost>(h: &H) -> Self {
        Self {
            cursor: h.read32(CURSOR) as i32,
            phase: Phase::from_word(h.read32(PHASE) as i32),
        }
    }
    pub fn store<H: ModeHost>(&self, h: &mut H) {
        h.write32(CURSOR, self.cursor as u32);
        h.write32(PHASE, self.phase.word() as u32);
    }
}

/// One row of the original 0x44-byte Story cursor table. Only consumed fields are named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryCursorRow {
    /// Level word (+6); its low byte is the level byte.
    pub level_word: u16,
    /// Level word of player 0 (+0xC) and of the other players (+0xE).
    pub player_levels: [u16; 2],
    /// Pre-level entry id (+0x20), `0xFFFFFFFF` for none.
    pub pre_id: u32,
    /// Post-level entry id (+0x24), `0xFFFFFFFF` for none.
    pub post_id: u32,
}

impl StoryCursorRow {
    /// Address of row `index`. Indices outside 0..13 read past the table exactly like the original.
    pub fn address(index: i32) -> u32 {
        CURSOR_TABLE.wrapping_add((index as u32).wrapping_mul(ROW_STRIDE))
    }
    pub fn read<H: ModeHost>(h: &H, index: i32) -> Self {
        let row = Self::address(index);
        Self {
            level_word: h.read16(row.wrapping_add(6)),
            player_levels: [
                h.read16(row.wrapping_add(0xc)),
                h.read16(row.wrapping_add(0xe)),
            ],
            pre_id: h.read32(row.wrapping_add(0x20)),
            post_id: h.read32(row.wrapping_add(0x24)),
        }
    }
}

/// Compatibility name retained for the T2431 sequencer API.
pub type CursorRow = StoryCursorRow;

/// The 0x24-byte intro descriptor. Only byte zero is consumed by this setup path; the remaining
/// bytes are retained so readers can preserve the original record without inventing field names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryIntroDescriptor {
    pub identifier: u8,
    pub reserved: [u8; 0x23],
}

impl StoryIntroDescriptor {
    pub fn read<H: ModeHost>(h: &H, index: u32) -> Self {
        let address = ENTRY_TABLE.wrapping_add((index & 0xff).wrapping_mul(ENTRY_STRIDE));
        Self {
            identifier: h.read8(address),
            reserved: core::array::from_fn(|i| h.read8(address.wrapping_add(1 + i as u32))),
        }
    }
}

/// A branch result for the setup builder. `None` means that the original leaves that output
/// field untouched. Transitions are data only; applying a plan never writes guest memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorySetupPlan {
    pub identifier: Option<u8>,
    pub active_type: Option<u8>,
    pub selector: Option<u8>,
    pub difficulty: Option<u8>,
    pub flags: Option<u32>,
    pub movie_index: Option<u8>,
    pub next_cursor: Option<i32>,
    pub next_stage: Option<Phase>,
}

impl StorySetupPlan {
    const fn unchanged() -> Self {
        Self {
            identifier: None,
            active_type: None,
            selector: None,
            difficulty: None,
            flags: None,
            movie_index: None,
            next_cursor: None,
            next_stage: None,
        }
    }

    fn intro(descriptor: StoryIntroDescriptor, movie_index: u32) -> Self {
        Self {
            identifier: Some(descriptor.identifier),
            active_type: Some(10),
            selector: Some(2),
            difficulty: None,
            flags: Some(0x2000_0000),
            movie_index: Some(movie_index as u8),
            next_cursor: None,
            next_stage: None,
        }
    }

    fn fallback(identifier: u8) -> Self {
        Self {
            identifier: Some(identifier),
            ..Self::unchanged()
        }
    }
}

/// Inputs consumed by the Story-to-gameplay selection contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorySetupInput {
    pub current_identifier: u32,
    pub cursor_ordinal: i32,
    pub setup_stage: Phase,
    pub mode_flags: u32,
    pub row: StoryCursorRow,
    pub descriptor: Option<StoryIntroDescriptor>,
}

/// Select one output patch while preserving the original guard order. This is a pure decision
/// function: adapters own guest reads and writes, and may retain unknown descriptor bytes.
pub fn plan_story_setup(input: StorySetupInput) -> StorySetupPlan {
    let none = StorySetupPlan::unchanged();
    match input.setup_stage {
        Phase::PostLevel => {
            if input.mode_flags & 0x0080_0000 != 0 {
                return StorySetupPlan::fallback(0x65);
            }
            if input.mode_flags & 0x0400_0000 != 0 {
                return StorySetupPlan::fallback(0x66);
            }
            if input.current_identifier == 0x68 {
                return StorySetupPlan::fallback(0x65);
            }
            if input.current_identifier == 0x6a {
                if input.row.post_id == NONE_ID {
                    return StorySetupPlan {
                        next_cursor: Some(input.cursor_ordinal.wrapping_add(1)),
                        next_stage: Some(Phase::PreLevel),
                        ..none
                    };
                }
                return input.descriptor.map_or(none, |descriptor| {
                    StorySetupPlan::intro(descriptor, input.row.post_id)
                });
            }
            none
        }
        Phase::PreLevel => {
            if input.row.pre_id == NONE_ID {
                return StorySetupPlan {
                    next_stage: Some(Phase::Level),
                    ..none
                };
            }
            input.descriptor.map_or(none, |descriptor| {
                StorySetupPlan::intro(descriptor, input.row.pre_id)
            })
        }
        Phase::Level => StorySetupPlan {
            identifier: Some(input.row.level_word as u8),
            active_type: Some(10),
            selector: Some(2),
            difficulty: None,
            flags: Some(0x0008_0008),
            movie_index: None,
            next_cursor: None,
            next_stage: None,
        },
        Phase::Advance if input.cursor_ordinal < ROWS - 1 => StorySetupPlan {
            next_cursor: Some(input.cursor_ordinal.wrapping_add(1)),
            next_stage: Some(Phase::PreLevel),
            ..none
        },
        Phase::Advance => StorySetupPlan {
            identifier: Some(0x38),
            active_type: Some(10),
            selector: Some(2),
            difficulty: Some(0),
            flags: None,
            movie_index: None,
            next_cursor: None,
            next_stage: None,
        },
        Phase::Other(_) => none,
    }
}

/// Mode byte of the entry-table record for `id` (only the low byte of the id indexes).
pub fn entry_mode<H: ModeHost>(h: &H, id: u32) -> u8 {
    h.read8(ENTRY_TABLE.wrapping_add((id & 0xff).wrapping_mul(ENTRY_STRIDE)))
}

/// The typed mode block. Offsets are the original ones; unnamed bytes are preserved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeBlock {
    bytes: [u8; BLOCK_BYTES],
}

/// One completed Story gameplay row emitted by the typed sequencer.
///
/// The setup bytes and campaign ordinal travel together so consumers cannot accidentally pair
/// the active level with another cursor row. Intro, end-mode, unsupported-phase, and out-of-range
/// cursor results do not produce a gameplay selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorySequencerOutput {
    setup: ModeBlock,
    campaign_ordinal: CampaignOrdinal,
}

impl StorySequencerOutput {
    pub fn from_result(state: SequencerState, setup: ModeBlock) -> Option<Self> {
        if state.phase != Phase::Level || setup.bytes()[0x55] != MODE_STORY {
            return None;
        }
        let campaign_ordinal = u8::try_from(state.cursor)
            .ok()
            .and_then(CampaignOrdinal::new)?;
        Some(Self {
            setup,
            campaign_ordinal,
        })
    }

    pub fn setup(&self) -> &ModeBlock {
        &self.setup
    }

    pub const fn campaign_ordinal(&self) -> CampaignOrdinal {
        self.campaign_ordinal
    }
}

impl ModeBlock {
    pub fn load<H: ModeHost>(h: &H, address: u32) -> Self {
        Self {
            bytes: core::array::from_fn(|i| h.read8(address.wrapping_add(i as u32))),
        }
    }
    pub fn store<H: ModeHost>(&self, h: &mut H, address: u32) {
        for (i, byte) in self.bytes.iter().enumerate() {
            h.write8(address.wrapping_add(i as u32), *byte);
        }
    }
    pub fn from_bytes(bytes: [u8; BLOCK_BYTES]) -> Self {
        Self { bytes }
    }
    pub fn bytes(&self) -> &[u8; BLOCK_BYTES] {
        &self.bytes
    }
    /// The 40 dwords `0x658D0` copies (`SessionSetup::words`).
    pub fn words(&self) -> [u32; BLOCK_BYTES / 4] {
        core::array::from_fn(|i| {
            u32::from_le_bytes([
                self.bytes[i * 4],
                self.bytes[i * 4 + 1],
                self.bytes[i * 4 + 2],
                self.bytes[i * 4 + 3],
            ])
        })
    }

    /// Player count byte (+0), signed in the original loops.
    pub fn players(&self) -> i8 {
        self.bytes[0] as i8
    }
    /// Level or mode byte (+0x54).
    pub fn level(&self) -> u8 {
        self.bytes[0x54]
    }
    pub fn set_players(&mut self, count: u8) {
        self.bytes[0] = count;
        self.bytes[1] = count;
    }
    fn set_dword(&mut self, offset: usize, value: u32) {
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn clear_tail(&mut self) {
        for offset in [0x60, 0x64, 0x68] {
            self.set_dword(offset, 0);
        }
    }

    fn apply_setup_plan(&mut self, plan: StorySetupPlan) {
        if let Some(value) = plan.identifier {
            self.bytes[0x54] = value;
        }
        if let Some(value) = plan.active_type {
            self.bytes[0x55] = value;
        }
        if let Some(value) = plan.selector {
            self.bytes[0x56] = value;
        }
        if let Some(value) = plan.difficulty {
            self.bytes[0x57] = value;
        }
        if let Some(value) = plan.flags {
            self.set_dword(0x58, value);
        }
        if let Some(value) = plan.movie_index {
            self.bytes[0x9b] = value;
        }
    }
}

fn finish_player_loop<H: ModeHost>(h: &mut H, block: &mut ModeBlock) {
    // do { slot[i] = 0x64DD0(i, level) } while (++i < players): the body runs at least once and
    // re-reads the live level and player count each pass (the slot bytes can alias them).
    let mut player = 0i32;
    loop {
        let level = block.bytes[0x54] as i8 as i32 as u32;
        let value = h.call(PLAYER_LEVEL_BYTE, &[player as u32, level]) as u8;
        block.bytes[0x14 + player as usize] = value;
        player += 1;
        if player >= block.bytes[0] as i8 as i32 {
            break;
        }
    }
}

/// Briefing (intro or post-level) entry block for entry `id`.
fn build_entry_block<H: ModeHost>(h: &mut H, block: &mut ModeBlock, id: u32) {
    block.bytes[0] = 1;
    block.bytes[1] = 1;
    block.bytes[2] = 0;
    block.bytes[0x57] = 0;
    block.apply_setup_plan(StorySetupPlan::intro(StoryIntroDescriptor::read(h, id), id));
    finish_player_loop(h, block);
}

/// Level block for the row under the cursor (phase 1).
/// Returns the original's exit EAX: the level word stored for the last player (0 when no cursor
/// row matches), or `None` when there are no players and EAX is residue of the header stores.
fn build_level_block<H: ModeHost>(
    h: &mut H,
    block: &mut ModeBlock,
    state: &SequencerState,
) -> Option<u32> {
    let players = h.read8(PARTICIPANTS);
    block.set_players(players);
    let row = StoryCursorRow::read(h, state.cursor);
    let plan = plan_story_setup(StorySetupInput {
        current_identifier: h.read32(GAME_MODE),
        cursor_ordinal: state.cursor,
        setup_stage: Phase::Level,
        mode_flags: h.read32(MODE_FLAGS),
        row,
        descriptor: None,
    });
    block.apply_setup_plan(plan);
    block.bytes[0x57] = h.read8(SELECTOR);
    block.set_dword(0x60, 0);
    block.set_dword(0x64, 0);
    block.set_dword(0x68, 0);
    if (block.bytes[0] as i8) <= 0 {
        return None;
    }
    let mut player = 0usize;
    loop {
        // Find the first row whose level word equals the sign-extended level, take that row's
        // player-0 or other-player level word. Not found gives 0.
        let wanted = block.bytes[0x54] as i8 as i16 as u16;
        let value = (0..ROWS)
            .map(|index| (index, CursorRow::read(h, index)))
            .find(|(_, row)| row.level_word == wanted)
            .map_or(0, |(_, row)| row.player_levels[usize::from(player != 0)]);
        block.bytes[0x14 + player] = value as u8;
        player += 1;
        if player as i32 >= block.bytes[0] as i8 as i32 {
            return Some(u32::from(value));
        }
    }
}

/// Level block for a competitive result (`0x64D10` true and the result word's high flag set).
fn build_result_block<H: ModeHost>(h: &mut H, block: &mut ModeBlock, players: u8) {
    block.set_players(players);
    block.bytes[0x54] = 0x6a;
    block.bytes[0x55] = 9;
    block.bytes[0x56] = 3;
    block.bytes[0x57] = h.read8(AUX_BYTE);
    block.set_dword(0x58, 0);
    block.set_dword(0x60, 0);
    block.set_dword(0x64, 0);
    block.set_dword(0x68, 0);
    if (players as i8) > 0 {
        let mut player = 0i32;
        loop {
            let level = block.bytes[0x54] as i8 as i32 as u32;
            let value = h.call(PLAYER_LEVEL_BYTE, &[player as u32, level]) as u8;
            block.bytes[0x14 + player as usize] = value;
            player += 1;
            if player >= block.bytes[0] as i8 as i32 {
                break;
            }
        }
    }
}

/// Where control continues inside the original after a phase-2 or tail step.
enum Next {
    /// `LAB_00065ad7`: the phase 3 / phase 0 tests.
    Tail,
    /// `LAB_00065b19`: pre-level entry of the row under the cursor.
    PreEntry,
    /// `LAB_00065ae0`: phase-3 advance.
    Advance,
}

/// `0x65940`. Builds the mode block for the current cursor/phase and moves the cursor/phase.
/// `block_address` is the guest address of the mode block (`0x78AD58` in the original callers).
pub fn story_progression_sequencer<H: ModeHost>(
    h: &mut H,
    block_address: u32,
) -> Option<StorySequencerOutput> {
    story_progression_sequencer_with_exit(h, block_address).0
}

/// [`story_progression_sequencer`] plus the EAX the original leaves when its last step is not a
/// guest call: the level word stored for the last player of a level block. `None` means EAX is
/// whatever the last nested call (or the entry value) left.
pub fn story_progression_sequencer_with_exit<H: ModeHost>(
    h: &mut H,
    block_address: u32,
) -> (Option<StorySequencerOutput>, Option<u32>) {
    let mut block = ModeBlock::load(h, block_address);
    let mut state = SequencerState::load(h);
    let exit_eax = run(h, &mut block, &mut state);
    let output = StorySequencerOutput::from_result(state, block.clone());
    state.store(h);
    block.store(h, block_address);
    (output, exit_eax)
}

/// The sequencer over typed values (the guest cells are loaded and stored by the caller).
/// Returns the level block's exit EAX, see [`story_progression_sequencer_with_exit`].
pub fn run<H: ModeHost>(
    h: &mut H,
    block: &mut ModeBlock,
    state: &mut SequencerState,
) -> Option<u32> {
    block.bytes[0] = 1;
    block.bytes[1] = 1;
    block.bytes[0x55] = 0;
    let mut next = if state.phase == Phase::PostLevel {
        match post_level(h, block, state) {
            Some(next) => next,
            None => return finish(h, block, state),
        }
    } else {
        Next::Tail
    };
    loop {
        match next {
            Next::Tail => {
                if state.phase == Phase::Advance {
                    next = Next::Advance;
                    continue;
                }
                if state.phase == Phase::PreLevel {
                    next = Next::PreEntry;
                    continue;
                }
                break;
            }
            Next::Advance => {
                let row = StoryCursorRow::read(h, state.cursor);
                let plan = plan_story_setup(StorySetupInput {
                    current_identifier: h.read32(GAME_MODE),
                    cursor_ordinal: state.cursor,
                    setup_stage: Phase::Advance,
                    mode_flags: h.read32(MODE_FLAGS),
                    row,
                    descriptor: None,
                });
                if let Some(cursor) = plan.next_cursor {
                    state.cursor = cursor;
                    state.phase = plan.next_stage.unwrap_or(Phase::PreLevel);
                    next = Next::PreEntry;
                    continue;
                }
                // The last row is done: the end-of-story mode (0x38) with the standard header.
                block.apply_setup_plan(plan);
                // Phase stays 3: neither the phase 0 nor the phase 1 test passes.
                break;
            }
            Next::PreEntry => {
                if pre_entry(h, block, state) {
                    // An intro entry block was built; phase stays 0 and the function returns.
                    return finish(h, block, state);
                }
                break;
            }
        }
    }
    finish(h, block, state)
}

fn finish<H: ModeHost>(
    h: &mut H,
    block: &mut ModeBlock,
    state: &mut SequencerState,
) -> Option<u32> {
    if state.phase == Phase::Level {
        build_level_block(h, block, state)
    } else {
        None
    }
}

/// Phase 2 (post-level). `None` means the original returned without reaching the tail tests.
fn post_level<H: ModeHost>(
    h: &mut H,
    block: &mut ModeBlock,
    state: &mut SequencerState,
) -> Option<Next> {
    let flags = h.read32(MODE_FLAGS);
    let mode = h.read32(GAME_MODE);
    let row = StoryCursorRow::read(h, state.cursor);
    let descriptor = (mode == 0x6a && row.post_id != NONE_ID)
        .then(|| StoryIntroDescriptor::read(h, row.post_id));
    let plan = plan_story_setup(StorySetupInput {
        current_identifier: mode,
        cursor_ordinal: state.cursor,
        setup_stage: Phase::PostLevel,
        mode_flags: flags,
        row,
        descriptor,
    });
    if plan.identifier.is_some() && mode != 0x6a
        || flags & 0x0080_0000 != 0
        || flags & 0x0400_0000 != 0
        || mode == 0x68
    {
        block.apply_setup_plan(plan);
        return Some(Next::Tail);
    }
    if mode != 0x6a {
        let active = h.call(POST_LEVEL_STATE, &[]);
        let participants = h.read8(PARTICIPANTS);
        if active != 0 && (h.read32(RESULT_WORD) >> 8) as u8 as i8 <= -1 {
            build_result_block(h, block, participants);
        } else {
            block.bytes[0x54] = 0x65;
        }
        return Some(Next::Tail);
    }
    // Mode 0x6a: post-level briefing of the cursor row, then advance.
    let post = row.post_id;
    if let Some(cursor) = plan.next_cursor {
        state.cursor = cursor;
        state.phase = plan.next_stage.unwrap_or(Phase::PreLevel);
        return Some(Next::PreEntry);
    }
    h.call(MARK_ENTRY_SEEN, &[post]);
    if h.call(SKIP_BRIEFINGS, &[]) == 0 {
        build_entry_block(h, block, post);
        return Some(Next::Tail);
    }
    h.call(
        RELEASE_ENTRY,
        &[CursorRow::read(h, state.cursor).post_id, 0],
    );
    state.cursor += 1;
    if state.cursor < ROWS {
        state.phase = Phase::PreLevel;
        Some(Next::PreEntry)
    } else {
        state.phase = Phase::Advance;
        Some(Next::Advance)
    }
}

/// Pre-level entry of the row under the cursor. `true` when an intro block was built (the
/// original then returns with the phase still 0), `false` when the phase moved to 1.
fn pre_entry<H: ModeHost>(h: &mut H, block: &mut ModeBlock, state: &mut SequencerState) -> bool {
    let skip = h.call(SKIP_BRIEFINGS, &[]);
    let row = StoryCursorRow::read(h, state.cursor);
    let pre = row.pre_id;
    let plan = plan_story_setup(StorySetupInput {
        current_identifier: h.read32(GAME_MODE),
        cursor_ordinal: state.cursor,
        setup_stage: Phase::PreLevel,
        mode_flags: h.read32(MODE_FLAGS),
        row,
        descriptor: (pre != NONE_ID).then(|| StoryIntroDescriptor::read(h, pre)),
    });
    if skip != 0 || plan.next_stage == Some(Phase::Level) {
        let again = row.pre_id;
        if again != NONE_ID {
            h.call(MARK_ENTRY_SEEN, &[again]);
        }
        state.phase = Phase::Level;
        return false;
    }
    h.call(MARK_ENTRY_SEEN, &[pre]);
    block.bytes[0] = 1;
    block.bytes[1] = 1;
    block.bytes[2] = 0;
    block.bytes[0x57] = 0;
    block.apply_setup_plan(plan);
    finish_player_loop(h, block);
    true
}

#[cfg(test)]
mod tests {
    use super::super::test_host::TestHost;
    use super::*;
    use crate::memory::GuestMemory;

    fn put32(h: &mut TestHost, address: u32, value: u32) {
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            h.bytes.insert(address + i as u32, byte);
        }
    }

    /// FABRICATED two-row table: row 0 level 0x05 pre id 3, row 1 level 0x07 no pre, post none.
    fn host() -> TestHost {
        let mut h = TestHost::default();
        for (row, level, pre) in [(0u32, 0x05u16, 3u32), (1, 0x07, NONE_ID)] {
            let base = CursorRow::address(row as i32);
            h.bytes.insert(base + 6, level as u8);
            h.bytes.insert(base + 0xc, 0x11 + row as u8);
            h.bytes.insert(base + 0xe, 0x21 + row as u8);
            put32(&mut h, base + 0x20, pre);
            put32(&mut h, base + 0x24, NONE_ID);
        }
        h.bytes.insert(ENTRY_TABLE + 3 * ENTRY_STRIDE, 0x30);
        h.bytes.insert(PARTICIPANTS, 2);
        h
    }

    #[test]
    fn cursor_rows_use_the_canonical_table_base_and_field_offsets() {
        let mut h = TestHost::default();
        let base = 0x004d_1ac0;
        put32(&mut h, base + 6, 0x1234);
        put32(&mut h, base + 0xc, 0x5678);
        put32(&mut h, base + 0xe, 0x9abc);
        put32(&mut h, base + 0x20, 0x1122_3344);
        put32(&mut h, base + 0x24, 0x5566_7788);

        assert_eq!(CursorRow::address(0), base);
        assert_eq!(CursorRow::address(1), base + ROW_STRIDE);
        assert_eq!(
            CursorRow::read(&h, 0),
            StoryCursorRow {
                level_word: 0x1234,
                player_levels: [0x5678, 0x9abc],
                pre_id: 0x1122_3344,
                post_id: 0x5566_7788,
            }
        );
    }

    #[test]
    fn pre_level_builds_the_intro_then_level_block() {
        let mut h = host();
        put32(&mut h, PHASE, 0);
        assert!(story_progression_sequencer(&mut h, 0x1000).is_none());
        // pre id 3 gives the entry block with the entry's mode byte, phase stays 0
        assert_eq!(h.read8(0x1000 + 0x54), 0x30);
        assert_eq!(h.read32(PHASE), 0);
        put32(&mut h, PHASE, 1);
        let output = story_progression_sequencer(&mut h, 0x1000).unwrap();
        assert_eq!(output.campaign_ordinal().get(), 0);
        let stored = ModeBlock::load(&h, 0x1000);
        assert_eq!(output.setup(), &stored);
        assert_eq!(h.read8(0x1000 + 0x54), 0x05);
        assert_eq!(h.read8(0x1000 + 0x14), 0x11);
        assert_eq!(h.read8(0x1000 + 0x15), 0x21);
        assert_eq!(h.read32(0x1000 + 0x58), 0x0008_0008);
    }

    /// The lifted level block leaves the last stored level word in EAX (T3784 call-argument
    /// stream: exit EAX 12 against the entry pointer); other phases leave it to the last call.
    #[test]
    fn level_block_exit_eax_is_the_last_stored_level_word() {
        let mut h = host();
        put32(&mut h, PHASE, 1);
        let (output, exit_eax) = story_progression_sequencer_with_exit(&mut h, 0x1000);
        assert!(output.is_some());
        assert_eq!(h.read8(0x1000 + 0x15), 0x21);
        assert_eq!(exit_eax, Some(0x21));
        put32(&mut h, PHASE, 0);
        assert_eq!(
            story_progression_sequencer_with_exit(&mut h, 0x1000).1,
            None
        );
    }

    #[test]
    fn advance_past_last_row_gives_end_mode() {
        let mut h = host();
        put32(&mut h, PHASE, 3);
        put32(&mut h, CURSOR, 12);
        assert!(story_progression_sequencer(&mut h, 0x1000).is_none());
        assert_eq!(h.read8(0x1000 + 0x54), 0x38);
        assert_eq!(h.read32(PHASE), 3);
        assert_eq!(h.read32(CURSOR), 12);
    }
}
