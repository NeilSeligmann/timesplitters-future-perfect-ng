// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    abi::{ExtRegisters, GuestCall, Xmm},
    inventory::sse,
    memory::GuestMemory,
    pose::write16,
    system_slice::{compare_fp, read_double, reciprocal_sqrt, round_pc, sqrt_double},
};
mod part_04;
pub use part_04::*;
mod part_05;
pub use part_05::*;
mod part_06;
pub use part_06::*;
