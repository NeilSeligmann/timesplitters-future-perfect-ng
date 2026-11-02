// SPDX-License-Identifier: GPL-3.0-or-later
// Shared pure interface; direct rustc archive and Cargo crate compile the same source.
#[path = "../../tsfp-engine-memory/src/leaf.rs"]
mod leaf;
pub use leaf::GuestMemory;
