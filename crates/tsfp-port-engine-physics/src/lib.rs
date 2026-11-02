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
/// Readable, opt-in physics contact reconstruction work.
pub mod contact_prune;

pub use c2r::t2125d_002;

#[cfg(test)]
mod tests {
    use super::t2125d_002;

    #[test]
    fn physics_batch_keeps_its_original_root() {
        assert_eq!(t2125d_002::ROOTS, [0x92a10]);
    }
}
