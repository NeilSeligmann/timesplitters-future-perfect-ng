// SPDX-License-Identifier: GPL-3.0-or-later
//! Data/assets guest-memory slice. See docs/t-system-assets.md.
//!
//! Each function remains independently selectable. Calls across the system edge
//! use GuestCall, and cache operations require an explicit host policy.
use crate::{abi::{ExtRegisters, GuestCall}, memory::GuestMemory};
pub mod bodies;

/// Cache/device visibility belongs to the host, not to file parsing.
/// Implementations must describe their coherence policy; no implicit no-op exists.
pub trait AssetGuest: GuestCall {
    fn writeback_invalidate(&mut self, instruction_address: u32);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationTier {
    /// Compiles; runtime smoke acceptance is tracked separately (not Tier 0).
    Compiled,
    /// Tier 0: translated, compiles, terminating synthetic smoke fixture.
    Smoke,
    /// Tier 1: differential against lifted original.
    Differential,
}
#[derive(Clone, Copy, Debug)]
pub struct AssetFunction {
    pub address: u32,
    pub name: &'static str,
    pub tier: VerificationTier,
}
include!("assets/registry.rs");

/// Return false without touching state for disabled or unknown functions.
/// Caller retains the lifted original on false: this is the per-function A/B edge.
pub fn dispatch<M: GuestMemory, C: AssetGuest>(
    address: u32,
    enabled: &[u32],
    memory: &mut M,
    guest: &mut C,
    registers: &mut ExtRegisters,
) -> bool {
    enabled.contains(&address) && run(address, memory, guest, registers)
}

#[cfg(test)]
#[path = "../tests/assets.rs"]
mod tests;
