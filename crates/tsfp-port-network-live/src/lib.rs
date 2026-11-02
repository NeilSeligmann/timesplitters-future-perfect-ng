// SPDX-License-Identifier: GPL-3.0-or-later
//! T3433: network/live C2R family crate, split mechanically out of tsfp-ports (tools/dev/split_ports.py).
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

pub mod c2r;

#[cfg(test)]
mod tests {
    #[test]
    fn split_modules_keep_their_roots() {
        assert_eq!(super::c2r::network_live_3::ROOTS.len(), 248);
        assert_eq!(super::c2r::network_live_4::ROOTS.len(), 247);
        assert_eq!(super::c2r::network_live_5::ROOTS.len(), 224);
        assert_eq!(super::c2r::network_live_6::ROOTS.len(), 17);
    }
}
