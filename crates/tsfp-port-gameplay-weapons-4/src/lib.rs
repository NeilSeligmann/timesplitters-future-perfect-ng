// SPDX-License-Identifier: GPL-3.0-or-later
//! T3643: t1925 weapon C2R chunk, split out of the frozen tsfp-ports facade.
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

pub mod c2r;

#[cfg(test)]
mod tests {
    #[test]
    fn split_module_keeps_its_roots() {
        assert_eq!(super::c2r::t1925_weapons_split_001::ROOTS.len(), 37);
    }
}
