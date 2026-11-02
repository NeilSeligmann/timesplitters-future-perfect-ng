// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline capture support for the original animation facade translations.
#![no_std]

pub use tsfp_port_engine_anim_adapter::{anim_resource_prefix, c2r::rt};
pub use tsfp_port_runtime::state_wire;

#[allow(unused, clippy::all)]
#[path = "../../tsfp-ports/src/c2r/engine_anim.rs"]
pub mod engine_anim;

pub mod c2r {
    pub use crate::engine_anim;
}
