// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline capture support for the scene register-contract translations.
#![no_std]

pub mod c2r {
    pub use tsfp_port_engine_anim_adapter::c2r::rt;
}

#[allow(unused, clippy::all)]
#[path = "../../tsfp-engine-render/src/scene_t1938.rs"]
pub mod scene;
