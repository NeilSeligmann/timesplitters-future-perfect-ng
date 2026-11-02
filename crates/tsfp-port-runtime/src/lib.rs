// SPDX-License-Identifier: GPL-3.0-or-later
#![no_std]
#![allow(unexpected_cfgs)]

pub mod abi;
pub mod memory;
pub mod weapon_hands_hooks;

#[cfg(any(test, feature = "thread-retire"))]
extern crate std;

// T3605: the path-included tsfp-state story_flow tests name `alloc::` (no_std crate); the lib test never compiled before.
#[cfg(test)]
extern crate alloc;

/// T3484: the single shared state_wire instance (RETIRE mode, owner state) for tsfp-ports and every family crate.
pub mod state_wire;

/// T3605: the single guest `fs` segment base shared by `tsfp-ports` and every family crate (`c2r::rt` re-exports it).
pub mod fs_base;
