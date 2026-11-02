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
extern crate alloc;

pub mod anim_resource_prefix;
#[path = "../../tsfp-engine-anim/src/dependency_prefix.rs"]
pub mod dependency_prefix;

// Keep the adapter's leaf modules in this no_std family crate. Depending on
// tsfp-engine-anim would pull std into tsfp-ports' lifted archive.
#[path = "../../tsfp-engine-anim/src/advance.rs"]
pub mod advance;
#[path = "../../tsfp-engine-anim/src/aim.rs"]
pub mod aim;
#[path = "../../tsfp-engine-anim/src/blend.rs"]
pub mod blend;
#[path = "../../tsfp-engine-anim/src/bounds.rs"]
pub mod bounds;
#[path = "../../tsfp-ports/src/fmath.rs"]
pub mod fmath;
#[path = "../../tsfp-engine-anim/src/heading.rs"]
pub mod heading;
#[path = "../../tsfp-engine-anim/src/instance.rs"]
pub mod instance;
#[path = "../../tsfp-engine-anim/src/keybracket.rs"]
pub mod keybracket;
#[path = "../../tsfp-engine-anim/src/layers.rs"]
pub mod layers;
#[path = "../../tsfp-engine-anim/src/posemath.rs"]
pub mod posemath;
#[path = "../../tsfp-engine-anim/src/posemix.rs"]
pub mod posemix;
#[path = "../../tsfp-engine-anim/src/prop.rs"]
pub mod prop;
#[path = "../../tsfp-engine-anim/src/queries.rs"]
pub mod queries;
#[path = "../../tsfp-engine-anim/src/start.rs"]
pub mod start;
#[path = "../../tsfp-engine-anim/src/tail.rs"]
pub mod tail;

pub mod anim_lane;
pub mod anim_lane3;
mod helpers;
