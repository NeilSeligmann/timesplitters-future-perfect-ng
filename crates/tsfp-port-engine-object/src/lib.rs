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

pub use c2r::engine_object_6;

#[cfg(test)]
mod tests {
    use super::engine_object_6;

    #[test]
    fn engine_object_batch_keeps_its_roots() {
        assert_eq!(engine_object_6::ROOTS, [0x9e430, 0x163a80, 0x163af0]);
    }
}
