// SPDX-License-Identifier: GPL-3.0-or-later
#![no_std]

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

pub use c2r::t2125c_003;

#[cfg(test)]
mod tests {
    use super::t2125c_003;

    #[test]
    fn cutscene_batch_keeps_its_original_root() {
        assert_eq!(t2125c_003::ROOTS, [0x5fcf0]);
    }
}
