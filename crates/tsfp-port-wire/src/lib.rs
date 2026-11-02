// SPDX-License-Identifier: GPL-3.0-or-later
//! T3610 (build speed, split slice 3): the S3 wiring layer shared by the lifted C2R batches and the hand-written
//! tsfp-ports modules, hosted once below both.
//!
//! Before this crate the wiring modules (`leaf_wire`, `player_wire`, `missions_wire`, ...) lived in the tsfp-ports facade,
//! called into its `c2r` batches, and the batches called back into them, so no batch that used a wiring module could
//! leave the facade for a family crate. The cycle is broken by moving the wiring state and its setters down into this
//! crate (layer order: `tsfp-port-runtime` < `tsfp-port-wire` < family crates < `tsfp-ports`) and by turning the
//! upward call of `leaf_wire` into a registered hook (`weapon_hands_hook`). `tsfp-ports` keeps one-line stub files at
//! the old paths, so `crate::leaf_wire`, `crate::player_wire`, ... resolve unchanged for every caller, test root and tool.
//!
//! Statics (`player_wire::PLAYER`, `missions_wire::MISSIONS`, the path-included owner state) exist exactly once, here.
//! Design and measurements: docs/t3605-build-speed.md, section "Split slice 3".
#![no_std]
#![allow(unexpected_cfgs)]

#[cfg(any(test, feature = "thread-retire"))]
extern crate std;

// The path-included tsfp-state owner modules name `alloc::`.
extern crate alloc;

pub mod abi {
    pub use tsfp_port_runtime::abi::*;
}

pub mod memory {
    pub use tsfp_port_runtime::memory::*;
}

pub use tsfp_port_runtime::state_wire;

pub mod contract_views;
pub mod hud_metrics;
pub mod leaf_wire;
pub mod menu_state;
pub mod missions_system;
pub mod missions_wire;
pub mod player_wire;
pub mod views;
pub mod weapon_hands_hook;
