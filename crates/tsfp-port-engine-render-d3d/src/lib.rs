// SPDX-License-Identifier: GPL-3.0-or-later
//! T3433: engine/render/d3d C2R family crate, split mechanically out of tsfp-ports (tools/dev/split_ports.py).
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

/// Readable native translations for D3D8 state handlers behind RenderDevice.
pub mod native_state;

/// Rust-owned primitive expansion used by the live renderer's staged draw-assembly port.
pub mod native_draw;

#[cfg(test)]
mod tests {
    #[test]
    fn split_modules_keep_their_roots() {
        assert_eq!(super::c2r::t1976_render::ROOTS.len(), 10);
    }
}
