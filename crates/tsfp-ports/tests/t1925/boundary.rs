// SPDX-License-Identifier: GPL-3.0-or-later
// Scoped harness: the full no_std crate's unrelated anim tests refer to std without import.
#![allow(dead_code, unused_imports)]
#[path = "../../src/abi.rs"]
mod abi;
#[path = "../../src/memory.rs"]
mod memory;
#[path = "../../src/c2r/rt.rs"]
pub mod rt;
#[path = "../../src/c2r/t1925_weapons.rs"]
pub mod t1925_weapons;
#[path = "../../src/weapons_slice.rs"]
mod weapons_slice;
mod c2r {
    pub use crate::t1925_weapons;
}
#[path = "../../src/weapons_system.rs"]
mod weapons_system;
