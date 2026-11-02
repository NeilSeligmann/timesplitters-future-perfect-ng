//! T1846 batch 8: the run-time sine table `0x7A38A0`, the follow velocity `0x78B6C4`, and pure adapters that build the
//! hook views of levels 2/0x40, 6, 0xB and 0x11 from a [`HookView`] plus the raw engine facts a host must supply.
//! See `docs/t-story-event-level-roles-t1846.md` for addresses, evidence and confidence labels.

use super::hooks_big::{Level0bView, Level06View, Level11View, PlayerView, SlotFacts};
use super::hooks_story::{HookView, Level2Inputs};

/// Entries of the sine table (`0x153C20` loops `i` in `0..0x800`).
pub const SINE_ENTRIES: usize = 0x800;
/// Address of the table (BSS, `f32` entries, 8 KiB).
pub const SINE_TABLE_ADDRESS: u32 = 0x7A38A0;
/// Index offset of the cosine (a quarter turn).
pub const SINE_QUARTER: usize = 0x200;
/// `[0x47DC80]`: 1/2048 as an f32.
pub const SINE_STEP: f32 = 1.0 / 2048.0;
/// `[0x475C94]`: 2 pi as an f32.
pub const SINE_TWO_PI: f32 = 6.283_185_5;
/// Radians to table index factor used by the readers (`ftol(angle * 325.949 + 0.5)`), 2048 / 2 pi.
pub const SINE_INDEX_SCALE: f32 = 325.949;

/// The table `0x153C20` fills: `table[i] = (f32) fsin((f32)((f32) i * 1/2048) * 2 pi))` (MEASURED against the original
/// under the Unicorn oracle, all 2048 entries bit exact). The original argument is an f32 on the stack, the sine is the
/// x87 `fsin` and the store rounds to f32; an f64 `sin` of the same f32 argument rounded to f32 gives the same bits.
pub fn build_sine_table() -> Vec<f32> {
    (0..SINE_ENTRIES)
        .map(|index| {
            let angle: f32 = (index as f32 * SINE_STEP) * SINE_TWO_PI;
            f64::from(angle).sin() as f32
        })
        .collect()
}

/// Reader indexing: `index & 0x7FF` for the sine, `(index + 0x200) & 0x7FF` for the cosine (as `0x116510`, `0x8D430`).
pub fn table_sin(table: &[f32], index: i32) -> f32 {
    table[(index & 0x7FF) as usize]
}

pub fn table_cos(table: &[f32], index: i32) -> f32 {
    table[(index.wrapping_add(SINE_QUARTER as i32) & 0x7FF) as usize]
}

/// `ftol(angle * 325.949 + 0.5)` (the truncating conversion of the original, x87 `fistp` with the chop control word).
pub fn angle_to_index(angle: f32) -> i32 {
    (angle * SINE_INDEX_SCALE + 0.5) as i32
}

/// The follow velocity `[0x78B6C4..0x78B6CC]` of the level effect emitter step (`0x24BD74`). Its only writer is the
/// reset `0x2498C0` (callers `0x1CDE0E`, `0x24A196`), which zeroes it, so the velocity is `[0; 3]` for the whole run.
pub const FOLLOW_VELOCITY_ADDRESS: u32 = 0x78B6C4;

pub fn follow_velocity_after_reset() -> [f32; 3] {
    [0.0; 3]
}

// ---------------------------------------------------------------------------------------------------------
// Level 2 / 0x40 (hook 0x21AD00)
// ---------------------------------------------------------------------------------------------------------

/// Builds [`Level2Inputs`]. `level_id` is `[0x79094C]` (the "mode word" of `0x218DC0` is that dword, MEASURED),
/// the game flag comes from [`HookView::game_flags`], `ambience_handle` is the persistent `[0x507DB8]` (-1 after the
/// level init `0x21AD50`, then the handle `0x45820` returned) and `ombh_count` is `0x25F370("OMBH", 0)` (world only).
pub fn level2_inputs(
    view: &dyn HookView,
    level_id: u32,
    ambience_handle: i32,
    ombh_count: u32,
) -> Level2Inputs {
    Level2Inputs {
        ambience_handle,
        mode_word: level_id,
        ombh_count,
        flag_40: view.game_flags() & 0x40 != 0,
    }
}

// ---------------------------------------------------------------------------------------------------------
// Raw facts for levels 6, 0xB, 0x11
// ---------------------------------------------------------------------------------------------------------

/// One entry of the player table (`[0x7B0C48]`, stride 0x1584, count `[0x790950]`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerRecord {
    /// Object `+0x1CC` above 0.0.
    pub alive: bool,
    /// `[[object+0x7C]+0x678]`.
    pub character: u32,
    /// Record `+0xF54`.
    pub state: u32,
    /// Record `+0x570 == 1`.
    pub flag_570: bool,
    /// Record `+0x1568`.
    pub word_1568: i32,
}

impl PlayerRecord {
    fn view(&self) -> PlayerView {
        PlayerView {
            alive: self.alive,
            character: self.character,
            state: self.state,
        }
    }
}

/// Slot status 2 or more (`0x227400`), the part of [`SlotFacts`] that [`HookView`] lacks.
pub type SlotAboveOne = bool;

/// Level 6 facts only the engine knows (see the doc for the original addresses).
#[derive(Clone, Copy, Debug, Default)]
pub struct Level06Engine {
    pub slot4_above_one: SlotAboveOne,
    pub slot2_above_one: SlotAboveOne,
    pub local_ff4_matches_first: bool,
    pub cdwd_active: bool,
    pub cptb_present: bool,
    pub cptb_key_is_549: bool,
    pub cptb_flag_bit0: bool,
}

/// Builds the level 6 view from the hook view (mode byte, slots 9/4/2/0xA, local player) and player records.
pub fn level06_view(
    view: &dyn HookView,
    players: &[PlayerRecord],
    engine: &Level06Engine,
) -> Level06View {
    Level06View {
        mode_id: view.mode_byte(),
        player_count: players.len(),
        slot9_finished: view.slot_finished(9),
        player_flag_570: players.iter().map(|p| p.flag_570).collect(),
        slot4: SlotFacts {
            above_one: engine.slot4_above_one,
            finished: view.slot_finished(4),
        },
        slot2: SlotFacts {
            above_one: engine.slot2_above_one,
            finished: view.slot_finished(2),
        },
        slot_a_finished: view.slot_finished(0xA),
        local_player: view.player_area().is_some(),
        local_ff4_matches_first: engine.local_ff4_matches_first,
        states: players.iter().map(|p| p.state).collect(),
        words_1568: players.iter().map(|p| p.word_1568).collect(),
        cdwd_active: engine.cdwd_active,
        cptb_present: engine.cptb_present,
        cptb_key_is_549: engine.cptb_key_is_549,
        cptb_flag_bit0: engine.cptb_flag_bit0,
    }
}

/// Level 0xB facts only the engine knows.
#[derive(Clone, Copy, Debug, Default)]
pub struct Level0bEngine {
    pub first_entry_key: Option<u32>,
    pub gas0_state_is_50: bool,
    pub first_player_1548_is_14: bool,
    pub camera_height_above: bool,
    pub handle_present: bool,
    pub input_bytes_any: bool,
    pub camera_mode_is_57_or_58: bool,
}

/// The tags `level0b_hook` queries through `Level0bView::tags_present`.
pub const LEVEL0B_TAGS: [u32; 11] = [
    0x4F49_4C59, // OILY
    0x4F49_4C58, // OILX
    0x4D41_4331, // MAC1
    0x4741_5330, // GAS0
    0x4755_5931, // GUY1
    0x4755_5932, // GUY2
    0x4755_5933, // GUY3
    0x4455_4D42, // DUMB
    0x5741_4C4B, // WALK
    0x5749_4C4B, // WILK
    0x4849_4445, // HIDE
];

/// Builds the level 0xB view. Tag presence and the WIND object come from [`HookView::npc`], the local state from
/// [`HookView::player_area`].
pub fn level0b_view(
    view: &dyn HookView,
    players: &[PlayerRecord],
    engine: &Level0bEngine,
) -> Level0bView {
    Level0bView {
        players: players.iter().map(PlayerRecord::view).collect(),
        local_state: view.player_area(),
        first_entry_key: engine.first_entry_key,
        tags_present: LEVEL0B_TAGS
            .iter()
            .copied()
            .filter(|&tag| view.npc(tag).is_some())
            .collect(),
        gas0_state_is_50: engine.gas0_state_is_50,
        wind_present: view.npc(0x5749_4E44).is_some(),
        first_player_1548_is_14: engine.first_player_1548_is_14,
        camera_height_above: engine.camera_height_above,
        handle_present: engine.handle_present,
        input_bytes_any: engine.input_bytes_any,
        camera_mode_is_57_or_58: engine.camera_mode_is_57_or_58,
    }
}

/// Level 0x11 facts only the engine knows: for each tagged object whether its tick word `+0x160` is zero.
#[derive(Clone, Copy, Debug, Default)]
pub struct Level11Engine {
    pub htbj_tick_zero: bool,
    pub jani_tick_zero: bool,
    pub hgz1_tick_zero: bool,
    pub hgz2_tick_zero: bool,
    pub hgz_gate: bool,
    pub slot1_above_one: SlotAboveOne,
    pub first_player: bool,
    pub bind_done: bool,
}

/// Builds the level 0x11 view (tag existence from [`HookView::npc`], slot 1 from the slot queries).
pub fn level11_view(
    view: &dyn HookView,
    players: &[PlayerRecord],
    engine: &Level11Engine,
) -> Level11View {
    let tagged = |tag: u32, tick_zero: bool| view.npc(tag).map(|_| tick_zero);
    Level11View {
        players: players.iter().map(PlayerRecord::view).collect(),
        mode_id: view.mode_byte(),
        player_count: players.len(),
        local_state: view.player_area(),
        htbj: tagged(super::hooks_big::LEVEL11_TAG_HTBJ, engine.htbj_tick_zero),
        jani: tagged(super::hooks_big::LEVEL11_TAG_JANI, engine.jani_tick_zero),
        hgz1: tagged(super::hooks_big::LEVEL11_TAG_HGZ1, engine.hgz1_tick_zero),
        hgz2: tagged(super::hooks_big::LEVEL11_TAG_HGZ2, engine.hgz2_tick_zero),
        hgz_gate: engine.hgz_gate,
        slot1: SlotFacts {
            above_one: engine.slot1_above_one,
            finished: view.slot_finished(1),
        },
        first_player: engine.first_player,
        bind_done: engine.bind_done,
    }
}
