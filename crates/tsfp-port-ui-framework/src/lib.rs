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

pub mod c2r;

pub use c2r::t1973_sweep;

#[cfg(test)]
mod tests {
    use super::t1973_sweep;

    #[test]
    fn framework_sweep_keeps_its_original_roots() {
        assert_eq!(t1973_sweep::ROOTS, [0x80300, 0x80370, 0x80af0]);
    }
}
