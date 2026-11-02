// SPDX-License-Identifier: GPL-3.0-or-later
//! T3433: data/missions C2R family crate, split mechanically out of tsfp-ports (tools/dev/split_ports.py).
//! tsfp-ports re-exports every module at its old `crate::c2r::<module>` path.
#![no_std]
#![allow(unexpected_cfgs)]

#[cfg(test)]
extern crate std;

pub mod abi {
    pub use tsfp_port_runtime::abi::*;
}

pub mod memory {
    pub use tsfp_port_runtime::memory::*;
}

pub use tsfp_port_runtime::state_wire;

// T3610: the wiring layer is hosted once in tsfp-port-wire; batches keep their `crate::<module>` paths.
pub use tsfp_port_wire::{
    contract_views, hud_metrics, leaf_wire, menu_state, missions_system, missions_wire,
    player_wire, views,
};

// The extracted state-pass batch calls these typed lanes through the original crate root.
// Compile the shared lane sources here so the leaf crate stays independent of tsfp-ports.
#[path = "../../tsfp-ports/src/story_event_combine_lane.rs"]
pub mod story_event_combine_lane;
#[path = "../../tsfp-ports/src/story_event_delay_lane.rs"]
pub mod story_event_delay_lane;
#[path = "../../tsfp-ports/src/story_event_gate_lane.rs"]
pub mod story_event_gate_lane;
#[path = "../../tsfp-ports/src/story_event_iteration_lane.rs"]
pub mod story_event_iteration_lane;
#[path = "../../tsfp-ports/src/story_event_state_lane.rs"]
pub mod story_event_state_lane;

pub mod c2r;

#[cfg(test)]
mod tests {
    #[test]
    fn split_modules_keep_their_roots() {
        assert_eq!(super::c2r::data_missions_split_001::ROOTS.len(), 58);
    }
}
