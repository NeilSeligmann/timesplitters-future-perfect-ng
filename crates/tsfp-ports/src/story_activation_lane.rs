// SPDX-License-Identifier: GPL-3.0-or-later
//! T2446: opt-in Story-only activation path at `0x670C0`.
//!
//! The Story initializer is readable Rust. Other mode families and Story dispatch-table arms
//! that have not been ported retain the existing C2R/hybrid fallback.
use crate::{
    abi::{ExtRegisters, GuestCall},
    gp_bridge::HostBridge,
    memory::GuestMemory,
    modes_lane::modes,
};
use core::cell::RefCell;

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

fn enabled_from(value: Option<&[u8]>) -> bool {
    value == Some(b"rust")
}

#[cfg(not(test))]
fn enabled() -> bool {
    unsafe {
        let value = getenv(c"TSFP_STORY_ACTIVATION".as_ptr());
        if value.is_null() {
            false
        } else {
            enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
        }
    }
}

#[cfg(test)]
fn enabled() -> bool {
    true
}

/// Intercepts the Story setup route only when explicitly enabled. `false` keeps the existing
/// C2R/hybrid implementation in charge.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if va != 0x670c0 || !enabled() {
        return false;
    }
    crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        let entry = *regs;
        let cell = RefCell::new(&mut *view);
        let mut bridge = HostBridge::new(&cell, guest, &entry);
        let Some(result) = modes::story_activation::activate_story(&mut bridge) else {
            return false;
        };
        regs.eax = result;
        true
    })
}

#[cfg(test)]
mod tests {
    use super::enabled_from;

    #[test]
    fn activation_switch_is_explicit_and_default_off() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"")));
        assert!(!enabled_from(Some(b"1")));
        assert!(enabled_from(Some(b"rust")));
    }
}
