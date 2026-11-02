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

pub use c2r::gameplay_inventory;

#[cfg(test)]
mod tests {
    use super::gameplay_inventory::ROOTS;

    #[test]
    fn gameplay_inventory_batch_keeps_its_original_roots() {
        assert_eq!(
            ROOTS,
            [
                0x1b8860, 0xc0eb0, 0x1b8910, 0x1b5f40, 0xc0e60, 0x1bc650, 0x1bf9e0, 0x1c0200,
            ]
        );
    }
}
