// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure route-executor guards shared with the hybrid runtime boundary.

pub const TURN_LOCK_MASK: u32 = 0x0010_0000;

/// Whether the measured kind-12 executor branch resets the route cursor.
/// The `0x100000` override is tested before the kind switch, so callers must
/// retain the existing path when it is set.
pub const fn resets_route_to_first(local_flags: u32, route_kind: u32) -> bool {
    local_flags & TURN_LOCK_MASK == 0 && route_kind == 12
}
