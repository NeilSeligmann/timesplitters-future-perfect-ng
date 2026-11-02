// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure early-return guard shared with the hybrid navigation-link boundary.

/// `0x263ED0` returns before dereferencing its route when the behavior
/// descriptor's route selector is -2.
pub const fn returns_before_route(selector: i32) -> bool {
    selector == -2
}
