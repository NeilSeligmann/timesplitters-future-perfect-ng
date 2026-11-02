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

pub mod c2r;

pub use c2r::t2086_effect;

#[cfg(test)]
mod tests {
    use super::t2086_effect;

    #[test]
    fn effects_batch_keeps_its_original_root() {
        assert_eq!(t2086_effect::ROOTS, [0xdd1b0]);
    }
}
