// SPDX-License-Identifier: GPL-3.0-or-later
//! T2431: the two Story menu pieces that live in the `0x2D2C70` / page-tick side of the chain,
//! as readable Rust over [`ModeHost`]. The row and button callbacks `0x2CAFE0`, `0x2CAD20` and
//! `0x2BEAA0` are UI handlers and live in `tsfp-ui-input` (`frontend_handlers`), the sequencer
//! they feed is [`super::story_sequencer`]. Tier 1 against the lifted originals on FABRICATED
//! inputs only (weak evidence): `crates/tsfp-c-diff` `modes_t1.rs`, see `docs/t2431-story-sequencer.md`.
//!
//! * [`setup_mode_4`] is the mode 4 (Story) branch of `0x2D2C70`: the common prologue (session
//!   gates, profile lookup, release `0x389B0`, session check, `0x2CB210`, player-count bytes of
//!   the block), then phase 0 and the sequencer call on the block `0x78AD58`.
//! * [`story_page_tick`] is the tick `0x2D3BF0` of the page `0x5236DC` (attract/credits/Story
//!   confirm): the background scroll, the fade-out when the page is finished, and, in the
//!   front-end, the hand-off to `0x2D38C0` once the match state `[0x7DE30C]` is 1.
use super::{ModeHost, read_f32, story_sequencer, x87_round24};
use story_sequencer::StorySequencerOutput;

/// Menu mode cell (`4` = Story list, `1` = attract, `0xE`, `0xF` other pages).
pub const MENU_MODE: u32 = 0x0078_ad2c;
/// The mode block `0x2D38C0` hands to `0x658D0`.
pub const MODE_BLOCK: u32 = 0x0078_ad58;
const PLAYER_SLOT: u32 = 0x0078_adad;
const BLOCK_LINK: u32 = 0x0078_adec;
const PARTICIPANTS: u32 = 0x0052_36c4;
/// The Story menu mode.
pub const MODE_STORY: u32 = 4;

// Services the prologue of `0x2D2C70` calls (VAs of other systems).
const SESSION_GATE: u32 = 0x0035_33f0;
const SESSION_READY: u32 = 0x0035_5ec0;
const PROFILE_LOOKUP: u32 = 0x0006_9640;
const RELEASE_ENTRY: u32 = 0x0003_89b0;
const SESSION_SETUP: u32 = 0x002c_b210;

/// `true` when the prologue leaves nothing to do: an online session exists but is not ready.
fn session_blocks<H: ModeHost>(h: &mut H) -> bool {
    h.call(SESSION_GATE, &[]) != 0 && h.call(SESSION_READY, &[]) == 0
}

/// The mode 4 branch of `0x2D2C70`. Returns `false` when the menu mode is not Story after the
/// prologue (the other branches stay with the original). ASSUMPTION (INFERRED): `0x2CB210` does not
/// change the menu mode, so an entry in mode 4 stays in mode 4; if it ever did, only the prologue
/// would have run here.
pub fn setup_mode_4<H: ModeHost>(h: &mut H) -> bool {
    setup_mode_4_with_sequencer_output(h).0
}

/// Where the mode 4 branch of `0x2D2C70` stands after its prologue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode4Step {
    /// The prologue left nothing to do (an online session exists but is not ready).
    Done,
    /// The menu mode is not Story after the prologue: the other branches stay with the original.
    NotStory,
    /// Phase 0 is stored; the original now calls the sequencer `0x65940` on [`MODE_BLOCK`].
    Sequencer,
}

/// Story mode setup with the optional completed gameplay row exposed to the host adapter.
/// The compatibility wrapper above retains the original handled/not-handled API.
pub fn setup_mode_4_with_sequencer_output<H: ModeHost>(
    h: &mut H,
) -> (bool, Option<StorySequencerOutput>) {
    match setup_mode_4_prologue(h) {
        Mode4Step::Done => (true, None),
        Mode4Step::NotStory => (false, None),
        Mode4Step::Sequencer => (
            true,
            story_sequencer::story_progression_sequencer(h, MODE_BLOCK),
        ),
    }
}

/// The prologue of the mode 4 branch up to, not including, the sequencer call. The lane makes
/// that call through the guest root, so it stays a visible `0x65940` root call as in the original.
pub fn setup_mode_4_prologue<H: ModeHost>(h: &mut H) -> Mode4Step {
    if session_blocks(h) {
        return Mode4Step::Done;
    }
    h.call(PROFILE_LOOKUP, &[]);
    let _player_slot = h.read8(PLAYER_SLOT);
    h.call(RELEASE_ENTRY, &[]);
    let session_active = h.call(SESSION_GATE, &[]);
    let mut mode = h.read32(MENU_MODE);
    if session_active == 0 && mode != 1 && mode != 0xe {
        h.call(SESSION_SETUP, &[]);
        mode = h.read32(MENU_MODE);
    }
    if mode != 0xf {
        let participants = h.read8(PARTICIPANTS);
        h.write8(MODE_BLOCK, participants);
        h.write8(MODE_BLOCK + 1, participants);
        h.write32(BLOCK_LINK, 0);
    }
    if mode != MODE_STORY {
        return Mode4Step::NotStory;
    }
    h.write32(story_sequencer::PHASE, 0);
    Mode4Step::Sequencer
}

// Page tick `0x2D3BF0`.
const SCROLL: u32 = 0x0076_004c;
const FRAME_DIVISOR: u32 = 0x004e_8cec;
const FRAME_SCALE: u32 = 0x004e_7948;
const FLOAT_ONE: u32 = 0x0047_5c78;
const FLOAT_TWO_POW_32: u32 = 0x0047_5ccc;
const FLOAT_INVERSE_STEP: u32 = 0x0047_6688;
const FLOAT_OFFSET: u32 = 0x0047_88fc;
const WIDTH_NUMERATOR: u32 = 0x004c_0458;
const WIDTH_DENOMINATOR: u32 = 0x004c_045c;
const HEIGHT_WORD: u32 = 0x004e_8cf0;
pub const MATCH_STATE: u32 = 0x007d_e30c;

const PAGE_FINISHED: u32 = 0x0007_0d90;
const FINISH_PAGE: u32 = 0x0007_0d50;
const FADE_STEP: u32 = 0x0004_6260;
const FADE_EXIT: u32 = 0x0004_62b0;
const WIDESCREEN_TEST: u32 = 0x0003_c420;
const DRAW_BAR: u32 = 0x0003_0210;
const DRAW_BAR_STYLE: u32 = 0x0003_0280;
const DRAW_PAGE: u32 = 0x0007_a7a0;
const PAGE_CHAIN: u32 = 0x002d_74f0;
const HUD_PANEL: u32 = 0x0015_53e0;
const START_SESSION: u32 = 0x002d_38c0;

/// Fade step while the page runs (float `0.02`) and after it finished (float `0.05`), as bits.
const FADE_RUNNING: u32 = 0x3ca3_d70a;
const FADE_FINISHED: u32 = 0x3d4c_cccd;
/// Bar style passed to `0x30280`.
const BAR_STYLE: u32 = 0x0049_f630;
const BAR_KIND: u32 = 0x28;

/// The CRT `_ftol` (`0x3C848C`): truncation toward zero, low 32 bits of the 64 bit result; NaN and
/// out of range inputs give 0. Written without `std` float functions (the hybrid lane is `no_std`).
pub fn ftol(value: f64) -> u32 {
    if value.is_nan() || value >= 9.223_372_036_854_776e18 || value <= -9.223_372_036_854_776e18 {
        0
    } else {
        (value as i64) as u32
    }
}

/// Vertical bar offset of the wide-screen letterbox: `(1 - K / (w / h)) * (height >> 1) - offset`,
/// every step rounded to the x87 24-bit precision the lifted code runs in. Returns the `_ftol` result.
fn letterbox_offset<H: ModeHost>(h: &H) -> u32 {
    let adjust = |word: u32| {
        let value = f64::from(h.read32(word) as i32);
        if (h.read32(word) as i32) < 0 {
            x87_round24(value + f64::from(read_f32(h, FLOAT_TWO_POW_32)))
        } else {
            value
        }
    };
    let width = adjust(WIDTH_NUMERATOR);
    let height = adjust(WIDTH_DENOMINATOR);
    let aspect = x87_round24(width / height);
    let half = (h.read32(HEIGHT_WORD) as i32) >> 1;
    let mut value = x87_round24(f64::from(read_f32(h, FLOAT_INVERSE_STEP)) / aspect);
    value = x87_round24(f64::from(read_f32(h, FLOAT_ONE)) - value);
    value = x87_round24(value * f64::from(half));
    value = x87_round24(value - f64::from(read_f32(h, FLOAT_OFFSET)));
    ftol(value)
}

/// The tick `0x2D3BF0` of the page `0x5236DC`; `page` is the page pointer it was called with.
pub fn story_page_tick<H: ModeHost>(h: &mut H, page: u32) {
    let sp = h.stack_pointer();
    // Background scroll (SSE single precision, left to right as compiled).
    let divisor = (h.read32(FRAME_DIVISOR) as i32) as f32;
    let step = read_f32(h, FLOAT_ONE) / divisor * read_f32(h, FRAME_SCALE) + read_f32(h, SCROLL);
    h.write32(SCROLL, step.to_bits());

    if h.call(PAGE_FINISHED, &[]) == 0 {
        // Finished: fade out and leave. The original reuses its caller's argument slot for the exit call.
        h.call(FINISH_PAGE, &[]);
        h.call(FADE_STEP, &[FADE_FINISHED]);
        h.write32(sp.wrapping_add(4), u32::MAX);
        h.call(FADE_EXIT, &[u32::MAX]);
        return;
    }
    h.call(FADE_STEP, &[FADE_RUNNING]);
    if h.read32(MENU_MODE) != 1 {
        // Front-end Story/credits: hand the selection over once the match state says so.
        if h.read32(MATCH_STATE) == 1 {
            h.call(HUD_PANEL, &[0x3c, 0x195, 0x188, 0xf]);
            h.call(START_SESSION, &[]);
        }
        return;
    }
    // Attract mode: letterbox bars, then continue with the page chain.
    if h.call(WIDESCREEN_TEST, &[]) as u8 == 0 {
        h.call(DRAW_BAR, &[BAR_KIND, u32::MAX, 1]);
    } else {
        let offset = letterbox_offset(h);
        h.call(DRAW_BAR, &[BAR_KIND, offset, 0]);
    }
    h.call(DRAW_BAR_STYLE, &[BAR_STYLE, 1]);
    h.call(DRAW_BAR, &[0, 0, 1]);
    h.call(DRAW_PAGE, &[page]);
    h.call(PAGE_CHAIN, &[page]);
}

#[cfg(test)]
mod tests {
    use super::super::test_host::TestHost;
    use super::*;
    use crate::memory::GuestMemory;

    fn put_f32(h: &mut TestHost, address: u32, value: f32) {
        h.write32(address, value.to_bits());
    }

    #[test]
    fn mode_4_runs_prologue_then_sequencer_with_phase_0() {
        let mut h = TestHost::default();
        h.write32(MENU_MODE, 4);
        h.write8(PARTICIPANTS, 2);
        h.write32(story_sequencer::PHASE, 9);
        assert!(setup_mode_4(&mut h));
        // The sequencer then rewrites the player-count bytes of the block (phase 0 intro block).
        assert_eq!(h.read32(BLOCK_LINK), 0);
        let calls: Vec<u32> = h.calls.iter().map(|c| c.0).collect();
        assert_eq!(
            &calls[..5],
            [
                SESSION_GATE,
                PROFILE_LOOKUP,
                RELEASE_ENTRY,
                SESSION_GATE,
                SESSION_SETUP
            ]
        );
        // phase 0 on cursor 0 with an all-zero table: the pre id word is 0, so the entry call follows.
        assert!(h.read32(story_sequencer::PHASE) <= 1);
    }

    #[test]
    fn mode_4_exposes_completed_gameplay_setup_without_separating_its_row() {
        let mut h = TestHost::default();
        h.write32(MENU_MODE, MODE_STORY);
        h.write8(PARTICIPANTS, 1);
        let row = story_sequencer::StoryCursorRow::address(0);
        h.write8(row + 6, 2);
        h.write8(row + 7, 0);
        h.write32(row + 0x20, 0xffff_ffff);
        h.write32(row + 0x24, 0xffff_ffff);

        let (handled, output) = setup_mode_4_with_sequencer_output(&mut h);
        let output = output.expect("no-intro first row completes the gameplay setup");
        assert!(handled);
        assert_eq!(output.campaign_ordinal().get(), 0);
        assert_eq!(output.setup().bytes()[0x54], 2);
        assert_eq!(output.setup().bytes()[0x55], crate::story_flow::MODE_STORY);
    }

    #[test]
    fn mode_4_is_skipped_for_other_modes_and_blocked_sessions() {
        let mut h = TestHost::default();
        h.write32(MENU_MODE, 6);
        assert!(!setup_mode_4(&mut h));
        let mut h = TestHost::default();
        h.write32(MENU_MODE, 4);
        h.returns.insert(SESSION_GATE, 1);
        assert!(setup_mode_4(&mut h));
        assert_eq!(h.calls.len(), 2, "gate and ready check only");
    }

    #[test]
    fn tick_scrolls_and_finished_page_exits_through_the_argument_slot() {
        let mut h = TestHost::default();
        h.write32(FRAME_DIVISOR, 2);
        put_f32(&mut h, FLOAT_ONE, 1.0);
        put_f32(&mut h, FRAME_SCALE, 4.0);
        put_f32(&mut h, SCROLL, 1.0);
        story_page_tick(&mut h, 0x1000);
        assert_eq!(f32::from_bits(h.read32(SCROLL)), 3.0);
        assert_eq!(h.read32(0x0030_0004), u32::MAX);
        let calls: Vec<u32> = h.calls.iter().map(|c| c.0).collect();
        assert_eq!(calls, [PAGE_FINISHED, FINISH_PAGE, FADE_STEP, FADE_EXIT]);
    }

    #[test]
    fn tick_hands_over_in_front_end_when_match_state_is_one() {
        let mut h = TestHost::default();
        h.returns.insert(PAGE_FINISHED, 1);
        h.write32(MENU_MODE, 4);
        h.write32(MATCH_STATE, 1);
        story_page_tick(&mut h, 0x1000);
        let calls: Vec<u32> = h.calls.iter().map(|c| c.0).collect();
        assert_eq!(calls, [PAGE_FINISHED, FADE_STEP, HUD_PANEL, START_SESSION]);
        let mut h = TestHost::default();
        h.returns.insert(PAGE_FINISHED, 1);
        h.write32(MENU_MODE, 4);
        story_page_tick(&mut h, 0x1000);
        assert_eq!(h.calls.len(), 2);
    }

    #[test]
    fn attract_tick_draws_bars_and_chains() {
        let mut h = TestHost::default();
        h.returns.insert(PAGE_FINISHED, 1);
        h.write32(MENU_MODE, 1);
        story_page_tick(&mut h, 0x1234);
        let calls: Vec<(u32, Vec<u32>)> = h.calls.clone();
        assert_eq!(calls[2], (WIDESCREEN_TEST, vec![]));
        assert_eq!(calls[3], (DRAW_BAR, vec![0x28, u32::MAX, 1]));
        assert_eq!(calls.last().unwrap(), &(PAGE_CHAIN, vec![0x1234]));
    }

    #[test]
    fn ftol_truncates_toward_zero_and_maps_invalid_to_zero() {
        assert_eq!(ftol(2.9), 2);
        assert_eq!(ftol(-2.9), (-2i32) as u32);
        assert_eq!(ftol(f64::NAN), 0);
        assert_eq!(ftol(1.0e30), 0);
    }
}
