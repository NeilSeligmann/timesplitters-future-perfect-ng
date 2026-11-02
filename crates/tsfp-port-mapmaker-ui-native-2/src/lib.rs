// SPDX-License-Identifier: GPL-3.0-or-later
#![no_std]
#![allow(unexpected_cfgs)]
#![allow(
    clippy::all,
    dead_code,
    unused_assignments,
    unused_imports,
    unused_mut,
    unused_parens,
    unused_variables
)]
pub use tsfp_port_mapmaker_ui_native::{abi, inventory, memory, pose, system_slice};
pub mod native;
