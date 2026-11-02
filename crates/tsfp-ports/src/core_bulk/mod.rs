// SPDX-License-Identifier: GPL-3.0-or-later
//! T1907 mechanically translated core bootstrap/parsers; Tier 0, opt-in only.
pub mod rt;
pub mod translated;

/// Per-VA A/B gate. Disabled or unsupported roots retain their lifted implementation.
/// This interface alone does not register a root in the production host selector.
pub fn run_if_enabled<M: crate::memory::GuestMemory, C: crate::abi::GuestCall>(
    va: u32,
    enabled: bool,
    m: &mut M,
    guest: &mut C,
    regs: &mut crate::abi::ExtRegisters,
) -> bool {
    enabled && translated::run(va, m, guest, regs)
}
