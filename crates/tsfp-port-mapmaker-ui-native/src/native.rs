// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    abi::{ExtRegisters, GuestCall, Xmm},
    inventory::sse,
    memory::GuestMemory,
    pose::write16,
    system_slice::{compare_fp, read_double, reciprocal_sqrt, round_pc, sqrt_double},
};
#[rustfmt::skip]
mod part_01;
pub use part_01::*;
#[rustfmt::skip]
mod part_02;
pub use part_02::*;
#[rustfmt::skip]
mod part_03;
pub use part_03::*;
