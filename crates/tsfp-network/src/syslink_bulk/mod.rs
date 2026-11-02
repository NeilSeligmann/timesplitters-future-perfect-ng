// SPDX-License-Identifier: GPL-3.0-or-later
//! T1917 opt-in network/syslink remainder. See docs/t1917-syslink-remainder.md.
pub mod rt;
pub mod state;
pub mod translated;
/// Per-function hybrid gate. Disabled or absent roots retain the original implementation.
pub fn run_if_enabled<M: crate::memory::GuestMemory, C: state::NetworkCall>(
    va: u32,
    enabled: bool,
    m: &mut M,
    guest: &mut C,
    r: &mut state::NetworkRegisters,
) -> bool {
    enabled && translated::run(va, m, guest, r)
}
