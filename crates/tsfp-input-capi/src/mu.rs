// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI over the Rust-owned memory-unit slot table of `src/input/mu_device.c` (T3926 slice 2).
//! Callers pass the table index from `tsfp_input_mu_index`; indices are validated once there.

use std::sync::{Mutex, PoisonError};

use tsfp_input_devices::mu::{MU_ENTRIES, MuTable, index_of};

static TABLE: Mutex<Option<MuTable>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut MuTable) -> R) -> R {
    let mut guard = TABLE.lock().unwrap_or_else(PoisonError::into_inner);
    f(guard.get_or_insert_with(MuTable::default))
}

/// Table index of a port/slot pair, or -1 outside the original 8-entry table.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_mu_index(port: u32, slot: u32) -> i32 {
    index_of(port, slot).map_or(-1, |i| i as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_mu_reset() {
    with(MuTable::reset);
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_mu_mounted_mask() -> u32 {
    with(|t| t.mounted_mask())
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_mu_is_mounted(index: u32) -> bool {
    (index as usize) < MU_ENTRIES && with(|t| t.is_mounted(index as usize))
}

/// Next free guest device id, or -1 when all eight are in use.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_mu_peek_free_id() -> i32 {
    with(|t| t.peek_free_id().map_or(-1, |id| id as i32))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_mu_mark_mounted(index: u32, guest_backed: bool) {
    if (index as usize) < MU_ENTRIES {
        with(|t| t.mark_mounted(index as usize, guest_backed));
    }
}

/// Returns true when a guest namespace entry was released (caller removes link and device).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_mu_mark_unmounted(index: u32) -> bool {
    (index as usize) < MU_ENTRIES && with(|t| t.mark_unmounted(index as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_mu_set_eject_pending(index: u32, pending: bool) {
    if (index as usize) < MU_ENTRIES {
        with(|t| t.set_eject_pending(index as usize, pending));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_mu_eject_pending(index: u32) -> bool {
    (index as usize) < MU_ENTRIES && with(|t| t.eject_pending(index as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_mu_mounted_flag(index: u32) -> bool {
    (index as usize) < MU_ENTRIES && with(|t| t.mounted_flag(index as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_mu_is_guest_backed(index: u32) -> bool {
    (index as usize) < MU_ENTRIES && with(|t| t.is_guest_backed(index as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_mu_device_id(index: u32) -> u32 {
    if (index as usize) < MU_ENTRIES {
        with(|t| t.device_id(index as usize))
    } else {
        0
    }
}
