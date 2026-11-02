// SPDX-License-Identifier: GPL-3.0-or-later
#![no_std]

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

pub use c2r::t1954_characters;

#[cfg(test)]
mod tests {
    use super::t1954_characters;

    #[test]
    fn high_class_tick_keeps_its_original_root() {
        assert_eq!(t1954_characters::ROOTS, [0x37b720]);
    }
}
