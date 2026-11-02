// SPDX-License-Identifier: GPL-3.0-or-later
//! T2080 (S3-12 of T2051): read-only pushed snapshots for the HUD and menu consumers. Design and
//! honest limits: docs/t-s3-snapshots.md. A snapshot owns nothing: it is a value built once per
//! frame from the owned S3 structs (`GameState`, `PlayerSet`, `Inventory`, `ObjectPool`) plus a
//! small table of raw guest cells that HUD or menu code reads today and that no S3 struct owns yet.
//! There is no `store`: consumers cannot write through a snapshot. Cell names are INFERRED, the
//! address list is MEASURED from the read sites in crates/tsfp-ports/src/c2r/ui_hud*.rs and
//! ui_menu*.rs (read counts in `docs/t-s3-snapshots.md`).
use super::GuestMemory;
use super::game_state::GameState;
use super::inventory::Inventory;
use super::menu::{
    CLICK_SOUND_ENABLED, CLICK_SOUND_REQUEST, OPEN_TRANSITION_FLAG, PLAYER_SLOT, UI_STATE_OBJECT,
};
use super::object_pool::ObjectPool;
use super::player::PlayerSet;

/// Local player slots the HUD draws (split screen is at most 4).
pub const MAX_SLOTS: usize = 4;
/// Player record field read by the prompt gate 0x42F90 (`[controlled + 0x114] == 0x100`).
pub const VIEWER_STATE_OFFSET: u32 = 0x114;
/// Flag bits of `[0x7DE458]` that veto the prompt (0x42F90 mask `0xE0000200`).
pub const PROMPT_VETO_FLAGS: u32 = 0xe000_0200;
/// Mode word values above this are not gameplay levels (0x42F90 compares `<= 0x64`).
pub const GAMEPLAY_LEVEL_MAX: i32 = 0x64;

/// One raw guest cell: the name is INFERRED, the address and width are MEASURED from read sites.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawCell {
    pub name: &'static str,
    pub address: u32,
    /// Width in bytes: 1 or 4.
    pub width: u8,
}

const fn c(name: &'static str, address: u32, width: u8) -> RawCell {
    RawCell {
        name,
        address,
        width,
    }
}

/// Cells the HUD roots (ui/hud c2r modules) read and no S3 struct owns. Read counts in the c2r
/// sources: 7e71c8 322, 55f974 277, 75cc98 123, 4e68f8 121, 4e68f4 118, 7a6140 110, 7a6148 88,
/// 7e7264 77, 7a6144 59, 7b0cc0 49, 4e8cf4 48, 7a614c 43, 4e8cf0 38, 7de318 31, 7de470 27.
pub const HUD_CELLS: [RawCell; 15] = [
    c("hud_7e71c8", 0x007e_71c8, 4),
    c("hud_55f974", 0x0055_f974, 4),
    c("hud_75cc98", 0x0075_cc98, 4),
    c("hud_4e68f8", 0x004e_68f8, 4),
    c("hud_4e68f4", 0x004e_68f4, 4),
    c("hud_7a6140", 0x007a_6140, 4),
    c("hud_7a6144", 0x007a_6144, 4),
    c("hud_7a6148", 0x007a_6148, 4),
    c("hud_7a614c", 0x007a_614c, 4),
    c("hud_7e7264", 0x007e_7264, 4),
    c("player_global_7b0cc0", 0x007b_0cc0, 4),
    c("tick_cell_4e8cf0", 0x004e_8cf0, 4),
    c("tick_cell_4e8cf4", 0x004e_8cf4, 4),
    c("hud_7de318", 0x007d_e318, 4),
    c("hud_7de470", 0x007d_e470, 4),
];

/// Cells the menu code (ui/menu c2r modules) reads. `MenuState` in crates/tsfp-ports owns the first
/// five as writers (T2042); the rest are read-only here. Read counts: 7bb1e8 506, 7613e4 205,
/// 7ba178/7ba16c 181 each, 756f0c 75, 78ad2c 70, 789834 59, 7ba038 57, 7973c8 56, 78adae 48,
/// 7ba9e0 45, 789824 37, 7ba188 36, 79e600 35.
pub const MENU_CELLS: [RawCell; 15] = [
    c("click_sound_enabled", CLICK_SOUND_ENABLED, 4),
    c("click_sound_request", CLICK_SOUND_REQUEST, 4),
    c("player_slot", PLAYER_SLOT, 4),
    c("ui_state_object", UI_STATE_OBJECT, 4),
    c("open_transition_flag", OPEN_TRANSITION_FLAG, 4),
    c("menu_7bb1e8", 0x007b_b1e8, 4),
    c("menu_78ad2c", 0x0078_ad2c, 4),
    c("menu_78adae", 0x0078_adae, 1),
    c("menu_789834", 0x0078_9834, 4),
    c("menu_789824", 0x0078_9824, 4),
    c("menu_7ba038", 0x007b_a038, 4),
    c("menu_7973c8", 0x0079_73c8, 4),
    c("menu_7ba9e0", 0x007b_a9e0, 4),
    c("menu_7ba188", 0x007b_a188, 4),
    c("menu_79e600", 0x0079_e600, 4),
];

fn read_cell<M: GuestMemory>(m: &M, cell: &RawCell) -> u32 {
    if cell.width == 1 {
        u32::from(m.read8(cell.address))
    } else {
        m.read32(cell.address)
    }
}

/// Index of a cell by name, `None` when the table has no such cell.
pub fn cell_index(table: &[RawCell], name: &str) -> Option<usize> {
    table.iter().position(|cell| cell.name == name)
}

/// What the HUD reads, pushed once per frame. Scalars come from `GameState`, the viewer fields from
/// `PlayerSet`, the weapon cells from `Inventory`, the pool length from `ObjectPool`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HudSnapshot {
    pub level: u32,
    pub players: u32,
    pub mode: u8,
    pub flags: u32,
    pub tick_divisor: u32,
    pub tick_scale_bits: u32,
    /// `[0x7B0C7C]`, zero while no player record exists (menus).
    pub viewer_record: u32,
    /// `[0x7B0C48]`.
    pub controlled: u32,
    /// `[controlled + 0x114]`, `None` when there is no controlled object.
    pub viewer_state: Option<u32>,
    /// Same boolean as 0x42F90 (`prompt_allowed`), computed from the fields above.
    pub prompt_allowed: bool,
    /// Update list length of the object pool, `None` when no pool was supplied.
    pub pool_list_len: Option<u32>,
    /// Per local slot inventory (current/previous/throwable weapon, ammo), `None` while the
    /// extension block pointer is null or the slot is not local.
    pub inventory: [Option<Inventory>; MAX_SLOTS],
    /// Raw cells, same order as `HUD_CELLS`.
    pub cells: [u32; HUD_CELLS.len()],
}

impl HudSnapshot {
    /// Builds from the owned structs. `slots` is the number of local slots to read inventory for.
    pub fn build<M: GuestMemory>(
        m: &M,
        game: &GameState,
        player: &PlayerSet,
        pool: Option<&ObjectPool>,
        slots: u32,
    ) -> Self {
        let viewer_state = (player.controlled_ptr != 0).then(|| {
            if player.controlled_is_record() {
                player.record_word(VIEWER_STATE_OFFSET)
            } else {
                m.read32(player.controlled_ptr.wrapping_add(VIEWER_STATE_OFFSET))
            }
        });
        let mut inventory = [None; MAX_SLOTS];
        for (slot, entry) in inventory.iter_mut().enumerate().take(slots as usize) {
            *entry = Inventory::load_slot(m, slot as u32);
        }
        let mut cells = [0u32; HUD_CELLS.len()];
        for (value, cell) in cells.iter_mut().zip(HUD_CELLS.iter()) {
            *value = read_cell(m, cell);
        }
        Self {
            level: game.level,
            players: game.players,
            mode: game.mode,
            flags: game.flags,
            tick_divisor: game.tick_divisor,
            tick_scale_bits: game.tick_scale_bits,
            viewer_record: player.record_ptr,
            controlled: player.controlled_ptr,
            viewer_state,
            prompt_allowed: prompt_allowed(game.flags, game.level, viewer_state),
            pool_list_len: pool.map(ObjectPool::list_len),
            inventory,
            cells,
        }
    }

    /// Loads every owned struct from the guest image and builds (the frame-start push).
    pub fn build_from_guest<M: GuestMemory>(m: &M, pool: Option<&ObjectPool>) -> Self {
        let game = GameState::load(m);
        let player = PlayerSet::load(m);
        Self::build(m, &game, &player, pool, game.players.min(MAX_SLOTS as u32))
    }

    pub fn cell(&self, name: &str) -> Option<u32> {
        cell_index(&HUD_CELLS, name).map(|i| self.cells[i])
    }
}

/// Port of the 0x42F90 decision: veto flags clear, and not (gameplay level with the controlled
/// object in state 0x100). `viewer_state` is `None` when no controlled object exists.
pub fn prompt_allowed(flags: u32, level: u32, viewer_state: Option<u32>) -> bool {
    if flags & PROMPT_VETO_FLAGS != 0 {
        return false;
    }
    !((level as i32) <= GAMEPLAY_LEVEL_MAX && viewer_state == Some(0x100))
}

/// What the menu code reads, pushed once per frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuSnapshot {
    pub level: u32,
    pub players: u32,
    pub mode: u8,
    pub flags: u32,
    /// Raw cells, same order as `MENU_CELLS`.
    pub cells: [u32; MENU_CELLS.len()],
}

impl MenuSnapshot {
    pub fn build<M: GuestMemory>(m: &M, game: &GameState) -> Self {
        let mut cells = [0u32; MENU_CELLS.len()];
        for (value, cell) in cells.iter_mut().zip(MENU_CELLS.iter()) {
            *value = read_cell(m, cell);
        }
        Self {
            level: game.level,
            players: game.players,
            mode: game.mode,
            flags: game.flags,
            cells,
        }
    }

    pub fn build_from_guest<M: GuestMemory>(m: &M) -> Self {
        Self::build(m, &GameState::load(m))
    }

    pub fn cell(&self, name: &str) -> Option<u32> {
        cell_index(&MENU_CELLS, name).map(|i| self.cells[i])
    }
}
