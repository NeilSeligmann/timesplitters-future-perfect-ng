// SPDX-License-Identifier: GPL-3.0-or-later
//! T3433: unknown C2R family crate, split mechanically out of tsfp-ports (tools/dev/split_ports.py).
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

pub mod c2r;

#[cfg(test)]
mod tests {
    #[test]
    fn split_modules_keep_their_roots() {
        assert_eq!(super::c2r::t2049_cls_001::ROOTS.len(), 50);
        assert_eq!(super::c2r::t2125c_001::ROOTS.len(), 27);
        assert_eq!(super::c2r::t2128_001::ROOTS.len(), 7);
    }
}
