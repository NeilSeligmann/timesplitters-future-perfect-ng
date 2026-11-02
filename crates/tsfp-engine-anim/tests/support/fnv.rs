// SPDX-License-Identifier: GPL-3.0-or-later
//! Digest step used by original-recorded animation goldens.

pub fn fnv(mut state: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        state = (state ^ u64::from(*byte)).wrapping_mul(0x100_0000_01B3);
    }
    state
}
