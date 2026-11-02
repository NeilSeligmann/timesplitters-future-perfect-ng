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

pub use c2r::ai_behavior;
pub use c2r::t2125_002;

#[cfg(test)]
mod tests {
    use super::{ai_behavior, t2125_002};

    #[test]
    fn ai_behavior_batch_keeps_its_roots() {
        assert_eq!(ai_behavior::ROOTS.len(), 121);
        let mut roots = ai_behavior::ROOTS;
        roots.sort_unstable();
        assert!(roots.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn character_mode_tick_keeps_its_root() {
        assert_eq!(t2125_002::ROOTS, [0x2784d0]);
    }
}
