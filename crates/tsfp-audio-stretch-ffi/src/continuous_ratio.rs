// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline model of the legacy continuous-rate sink governor.

pub fn update(fill: usize, low: usize, floor: u32, ratio: &mut u32) -> u32 {
    if low == 0 {
        return 65536;
    }
    let mut wanted = 65536;
    if fill < low {
        wanted = ((fill as u64).wrapping_mul(65536) / low as u64) as u32;
        wanted = wanted.max(floor);
    }
    if wanted < *ratio {
        *ratio -= (*ratio - wanted) / 2;
    } else {
        *ratio = ratio.wrapping_add(wanted.wrapping_sub(*ratio).wrapping_add(7) / 8);
    }
    if *ratio > 65536 - 64 {
        *ratio = 65536;
    }
    *ratio
}
