// SPDX-License-Identifier: GPL-3.0-or-later
//! T3433: engine/render/effects C2R family crate, split mechanically out of tsfp-ports (tools/dev/split_ports.py).
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
        assert_eq!(super::c2r::t2006_002::ROOTS.len(), 12);
        assert_eq!(super::c2r::t2049_exec_004::ROOTS.len(), 24);
        assert_eq!(super::c2r::t2049_exec_005::ROOTS.len(), 22);
        assert_eq!(super::c2r::t2049_exec_006::ROOTS.len(), 24);
        assert_eq!(super::c2r::t2049_exec_007::ROOTS.len(), 24);
        assert_eq!(super::c2r::t2049_exec_008::ROOTS.len(), 25);
        assert_eq!(super::c2r::t2049_nx_001::ROOTS.len(), 24);
        assert_eq!(super::c2r::t2049_nx_003::ROOTS.len(), 24);
        assert_eq!(super::c2r::t2049_nx_004::ROOTS.len(), 24);
        assert_eq!(super::c2r::t2049_nx_005::ROOTS.len(), 25);
        assert_eq!(super::c2r::t2125c_002::ROOTS.len(), 5);
    }
}
