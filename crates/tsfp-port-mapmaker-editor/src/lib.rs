// SPDX-License-Identifier: GPL-3.0-or-later
//! T3433: mapmaker/editor C2R family crate, split mechanically out of tsfp-ports (tools/dev/split_ports.py).
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
        assert_eq!(super::c2r::mapmaker_editor::ROOTS.len(), 100);
        assert_eq!(super::c2r::mapmaker_editor_2::ROOTS.len(), 99);
        assert_eq!(super::c2r::t2049_w23_003::ROOTS.len(), 25);
        assert_eq!(super::c2r::t2086_alloc::ROOTS.len(), 2);
    }
}
