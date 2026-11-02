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

pub use c2r::render_model;

#[cfg(test)]
mod tests {
    use super::render_model;

    #[test]
    fn render_scene_batch_keeps_its_original_root() {
        assert_eq!(render_model::ROOTS, [0x18ea90]);
    }
}
