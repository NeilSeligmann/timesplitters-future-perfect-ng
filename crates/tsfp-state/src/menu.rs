// SPDX-License-Identifier: GPL-3.0-or-later
//! T2060: guest-authoritative menu state (S3 View stage). Names match `snapshots::MENU_CELLS`.
//! Each live view access performs exactly one dword access; nothing is cached across guest calls.
//! Field names are INFERRED from use sites; addresses are MEASURED (docs/t-system-menu.md).
use super::GuestMemory;

/// Click sound gate: non-zero when menu navigation plays its click (read by every page opener, set by the
/// texture-table loader `0x812f0`, which writes 1).
pub const CLICK_SOUND_ENABLED: u32 = 0x007b_a16c;
/// Pointer to the sound request descriptor whose first dword is submitted by `0x21780` (the render lane writes
/// descriptors `0x4d94e8` / `0x4d9510` here).
pub const CLICK_SOUND_REQUEST: u32 = 0x007b_a178;
/// Index into the `0x1f54`-stride per-player table at `0x7bedc4`; cleared by the settings back callback
/// (`0x2cbfa0`). Shared with data/save (13 refs) and gameplay/player (5).
pub const PLAYER_SLOT: u32 = 0x0078_ad34;
/// The UI state object passed to the widget push/pop calls `0x79b70` / `0x79c00` (a pointer-sized global).
pub const UI_STATE_OBJECT: u32 = 0x0076_13e4;
/// Set to 1 by the alternate (create-or-find child) page path before the open transition.
pub const OPEN_TRANSITION_FLAG: u32 = 0x0075_6f0c;

/// Snapshot of the shared menu globals. `load` and `store` keep one read or write per field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuState {
    pub click_sound_enabled: u32,
    pub click_sound_request: u32,
    pub player_slot: u32,
    pub ui_state_object: u32,
}

impl MenuState {
    pub fn load<M: GuestMemory>(m: &M) -> Self {
        Self {
            click_sound_enabled: m.read32(CLICK_SOUND_ENABLED),
            click_sound_request: m.read32(CLICK_SOUND_REQUEST),
            player_slot: m.read32(PLAYER_SLOT),
            ui_state_object: m.read32(UI_STATE_OBJECT),
        }
    }

    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        m.write32(CLICK_SOUND_ENABLED, self.click_sound_enabled);
        m.write32(CLICK_SOUND_REQUEST, self.click_sound_request);
        m.write32(PLAYER_SLOT, self.player_slot);
        m.write32(UI_STATE_OBJECT, self.ui_state_object);
    }
}

/// Live view of the shared menu cells. The guest image remains the sole authority.
/// Unlike `MenuState::load`, getters touch only the requested field, at the call site.
#[derive(Clone, Copy, Debug, Default)]
pub struct MenuStateView;

impl MenuStateView {
    /// Write the open-page transition gate at the point the allocation arm reaches it.
    #[inline]
    pub fn set_open_transition_flag<M: GuestMemory>(self, m: &mut M, value: u32) {
        m.write32(OPEN_TRANSITION_FLAG, value);
    }

    #[inline]
    pub fn click_sound_enabled<M: GuestMemory>(self, m: &M) -> u32 {
        m.read32(CLICK_SOUND_ENABLED)
    }

    #[inline]
    pub fn set_click_sound_enabled<M: GuestMemory>(self, m: &mut M, value: u32) {
        m.write32(CLICK_SOUND_ENABLED, value);
    }

    #[inline]
    pub fn click_sound_request<M: GuestMemory>(self, m: &M) -> u32 {
        m.read32(CLICK_SOUND_REQUEST)
    }

    #[inline]
    pub fn set_click_sound_request<M: GuestMemory>(self, m: &mut M, value: u32) {
        m.write32(CLICK_SOUND_REQUEST, value);
    }

    #[inline]
    pub fn player_slot<M: GuestMemory>(self, m: &M) -> u32 {
        m.read32(PLAYER_SLOT)
    }

    #[inline]
    pub fn set_player_slot<M: GuestMemory>(self, m: &mut M, value: u32) {
        m.write32(PLAYER_SLOT, value);
    }

    #[inline]
    pub fn ui_state_object<M: GuestMemory>(self, m: &M) -> u32 {
        m.read32(UI_STATE_OBJECT)
    }

    #[inline]
    pub fn set_ui_state_object<M: GuestMemory>(self, m: &mut M, value: u32) {
        m.write32(UI_STATE_OBJECT, value);
    }
}
