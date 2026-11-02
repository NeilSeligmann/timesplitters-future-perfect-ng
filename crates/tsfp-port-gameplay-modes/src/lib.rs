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
pub mod return_regs;

pub use c2r::modes_deferred;

#[cfg(test)]
mod tests {
    use super::modes_deferred;

    #[test]
    fn modes_deferred_batch_keeps_its_roots() {
        assert_eq!(modes_deferred::ROOTS.len(), 6);
        let mut roots = modes_deferred::ROOTS;
        roots.sort_unstable();
        assert!(roots.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
