// SPDX-License-Identifier: GPL-3.0-or-later
//! Direct guest-heap operations for native platform consumers.

use crate::{MODE_RUST, with_state};

/// Free a block in the shared guest heap; used by native FILE_OBJECT ownership.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kmem_heap_free(handle: u32, address: u32) -> bool {
    with_state(|state| state.mode == MODE_RUST && state.km.regions.heap_free(handle, address))
        .unwrap_or(false)
}
