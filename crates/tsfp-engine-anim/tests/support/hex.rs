// SPDX-License-Identifier: GPL-3.0-or-later
//! Hex words used by the original-recorded golden fixtures.

pub fn hex(w: &str) -> u32 {
    u32::from_str_radix(w, 16).unwrap()
}
