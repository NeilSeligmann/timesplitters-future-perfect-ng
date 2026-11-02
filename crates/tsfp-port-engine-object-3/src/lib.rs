// SPDX-License-Identifier: GPL-3.0-or-later
//! T3433: engine/object C2R family crate, split mechanically out of tsfp-ports (tools/dev/split_ports.py).
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
    contract_views, leaf_wire, menu_state, missions_system, missions_wire, player_wire, views,
};

pub mod c2r;

#[cfg(test)]
mod tests {
    #[test]
    fn split_modules_keep_their_roots() {
        assert_eq!(super::c2r::engine_object_2::ROOTS.len(), 239);
    }
}
