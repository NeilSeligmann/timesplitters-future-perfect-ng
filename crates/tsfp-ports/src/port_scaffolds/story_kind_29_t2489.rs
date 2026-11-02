// SPDX-License-Identifier: GPL-3.0-or-later
//! T2489 readable typed state for Story condition kind 29.

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PortState {
    pub gas_tag_0x47415330_exists: bool,
    pub scene_flag_0x75bf0c: Option<u32>,
}

/// `None` preserves the lifted fallback if the scene flag is unavailable.
pub fn port_step(state: &PortState) -> Option<bool> {
    if state.gas_tag_0x47415330_exists {
        Some(true)
    } else {
        state.scene_flag_0x75bf0c.map(|flag| flag == 1)
    }
}
