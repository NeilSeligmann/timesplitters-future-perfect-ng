// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared control-index selection for the REA route peek boundary.

/// Selects the next route index for the measured kind-12/kind-13 controls.
pub fn peek_next_index(cursor: u32, next_kind: u32, next_payload_a: u32) -> u32 {
    match next_kind {
        12 => 1,
        13 => cursor.wrapping_sub(next_payload_a).wrapping_add(1),
        _ => cursor.wrapping_add(1),
    }
}
