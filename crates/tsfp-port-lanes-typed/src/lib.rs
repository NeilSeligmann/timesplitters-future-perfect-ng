// SPDX-License-Identifier: GPL-3.0-or-later
//! T3768: lane adapters for readable Rust ports that had no `ROOTS` row (capture Tier 2 "Group B",
//! tmp/t2round-breakdown.md). Each `c2r/<module>.rs` lists its roots in `ROOTS` and marshals the
//! original calling convention onto the readable port, so `python3 -m tools.lane_rows` derives
//! the lanes.json rows (pops and stack arguments from the lifted body) like any c2r owner.
//!
//! The readable ports are compiled here by `#[path]` (their own crates use std); this crate is a
//! no_std family crate (tools/data/c2r_family_crates.json) linked into the host archive.
#![no_std]

#[cfg(test)]
extern crate std;

/// `crate::memory` of the path-included tsfp-ui sources: the shared guest memory trait.
pub mod memory {
    pub use tsfp_port_runtime::memory::GuestMemory;
}

// The closure-model ports name `tsfp_engine_object::closure::*`; that path resolves here.
extern crate self as tsfp_engine_object;

pub mod closure;

// tsfp-ports editor translations (T1921 `editor.rs`) and the `crate::*` items they name.
pub mod abi {
    pub use tsfp_port_runtime::abi::*;
}
pub mod inventory {
    pub(crate) use crate::physics::boundary::sse;
}
pub mod menu_state {
    pub use tsfp_port_wire::menu_state::*;
}
pub mod pose {
    pub(crate) use crate::physics::boundary::write16;
}
#[allow(unused)] // machine translation: dead tails and unused helpers of the lifted C
#[path = "../../tsfp-ports/src/editor.rs"]
pub mod editor;
#[allow(unused)] // shared source: only the helpers the editor translations call are used
#[path = "../../tsfp-ports/src/system_slice/mod.rs"]
pub(crate) mod system_slice;
/// T1935 continuation, split into the native family crates by T3747 (same functions as the
/// tsfp-ports `editor_next.rs` wrapper).
pub mod editor_next {
    pub use tsfp_port_mapmaker_editor_native::native::*;
    pub use tsfp_port_mapmaker_editor_native_2::native::*;
}
// tsfp-ports T1902 vehicles translations (register contract), T3785.
pub mod cpu_bridge;
pub mod hud;
pub mod physics;
#[path = "../../tsfp-engine-memory/src/sse.rs"]
pub mod sse;
pub mod tsfp_ports_c2r;
#[allow(unused, clippy::all)] // machine translation: dead tails of the lifted C
#[path = "../../tsfp-ports/src/vehicles.rs"]
pub mod vehicles;

// Closure-model ports (`Cpu` plus `Callee`), run through `cpu_bridge`.
#[allow(dead_code)]
#[path = "../../tsfp-ai/src/characters.rs"]
pub mod characters;
#[allow(dead_code)]
#[path = "../../tsfp-engine-object/src/w4_object.rs"]
pub mod w4_object;
#[allow(dead_code)]
#[path = "../../tsfp-gameplay/src/w4_weapons.rs"]
pub mod w4_weapons;
#[allow(dead_code)]
#[path = "../../tsfp-gameplay/src/w4b_gameplay.rs"]
pub mod w4b_gameplay;

pub mod c2r;

/// Capture-only typed adapter for the engine-video hand port `0x00028D40`.
/// This module is deliberately absent from every live root table.
pub mod offline_video;
