// SPDX-License-Identifier: GPL-3.0-or-later
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
    contract_views, leaf_wire, menu_state, missions_system, missions_wire, player_wire, views,
};

pub mod c2r;

/// Readable, typed reconstructions of navigation contracts.
pub mod route_program;

pub use c2r::ai_pathfinding;

#[cfg(test)]
mod tests {
    use super::ai_pathfinding;

    #[test]
    fn ai_pathfinding_batch_keeps_its_roots() {
        assert_eq!(ai_pathfinding::ROOTS.len(), 21);
        let mut roots = ai_pathfinding::ROOTS;
        roots.sort_unstable();
        assert!(roots.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
