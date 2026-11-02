// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/c2r/ui_menu_t2042_dispatch
//! Hand-written ui/menu ports of T2042, wired into the c2r dispatch the way the `*_readable` modules are: `c2r::run`
//! consults this module before the mechanical one. `0x1AF950` (`crate::menu_dispatch`) shadows its translated twin
//! `ui_menu_t2042_dispatch`, which stays as the reference and the A/B fallback (delete this module to fall back).
//! `0x2E28A0` (`crate::menu_ease`) has no twin: the lifted original leaves its flag reads unresolved.
#![allow(unused_parens, unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
use super::rt::*;

/// Hybrid entry: true when `va` is one of the hand-ported roots.
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    crate::menu_dispatch::try_run(va, m, guest, regs) || crate::menu_ease::try_run(va, m, regs)
}
