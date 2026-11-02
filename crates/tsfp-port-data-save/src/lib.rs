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

pub use c2r::save_mapmaker_001;
pub use c2r::save_menu_001;

#[cfg(test)]
mod tests {
    use super::save_mapmaker_001;
    use super::save_menu_001;

    #[test]
    fn save_menu_batch_keeps_its_original_root() {
        assert_eq!(save_menu_001::ROOTS, [0x32b540]);
    }

    #[test]
    fn save_mapmaker_batch_keeps_its_original_roots() {
        assert_eq!(
            save_mapmaker_001::ROOTS,
            [
                0x25280, 0x25710, 0x25e40, 0x25ff0, 0x26320, 0x263e0, 0x26740
            ]
        );
    }
}
