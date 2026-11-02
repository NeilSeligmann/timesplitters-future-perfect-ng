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

/// Readable player-interaction logic owned by this gameplay family crate.
pub mod interaction;

pub use c2r::gameplay_player_3;

#[cfg(test)]
mod tests {
    use super::c2r::{gameplay_player_2, gameplay_player_3};

    #[test]
    fn gameplay_player_batch_keeps_its_root() {
        assert_eq!(gameplay_player_3::ROOTS, [0x0d0e30]);
    }

    #[test]
    fn gameplay_player_2_batch_keeps_its_roots() {
        assert_eq!(gameplay_player_2::ROOTS, [0x37a7a0, 0x37a9b0]);
    }
}
