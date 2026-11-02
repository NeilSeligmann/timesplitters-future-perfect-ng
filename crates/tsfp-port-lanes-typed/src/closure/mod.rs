// SPDX-License-Identifier: GPL-3.0-or-later
//! The closure-model contract of tsfp-engine-object (`Cpu`, `Callee`) for the path-included
//! tsfp-ai, tsfp-gameplay and tsfp-engine-object ports (`tsfp_engine_object::closure::*`).
#[path = "../../../tsfp-engine-object/src/closure/callee.rs"]
pub mod callee;

pub mod memory {
    pub use tsfp_port_runtime::memory::GuestMemory;
}
