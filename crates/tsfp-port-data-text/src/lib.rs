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

pub use c2r::data_text;

#[cfg(test)]
mod tests {
    use super::data_text;

    #[test]
    fn data_text_batch_keeps_its_roots() {
        assert_eq!(data_text::ROOTS.len(), 39);
        let mut roots = data_text::ROOTS;
        roots.sort_unstable();
        assert!(roots.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
