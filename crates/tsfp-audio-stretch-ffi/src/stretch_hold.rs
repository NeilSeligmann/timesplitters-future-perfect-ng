// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline model of the stretch sink's held-block gain target.

/// Advance a successful hold. Returns the gain and whether this block decays.
/// Invalid or exhausted hold geometry returns `None` without changing `run`.
pub fn advance(run: &mut usize, overlap: usize, full: usize, max: usize) -> Option<(u32, bool)> {
    if overlap == 0 || full > max {
        return None;
    }
    let next = run.checked_add(overlap)?;
    if next > max {
        return None;
    }
    let decay = next > full;
    let target = if decay {
        let span = max - full;
        let into = next - full;
        65536 - (65536_u128 * into.min(span) as u128 / span as u128) as u32
    } else {
        65536
    };
    *run = next;
    Some((target, decay))
}

/// State updates after the stretch engine fades out an exhausted hold.
/// Invalid ring geometry leaves the state untouched.
pub fn finish_fade(
    capacity: usize,
    head: &mut usize,
    fill: &mut usize,
    consumed: &mut usize,
    active: &mut bool,
    fadeouts: &mut u64,
) -> bool {
    let Some(total) = consumed.checked_add(*fill) else { return false };
    let Some(next_fadeouts) = fadeouts.checked_add(1) else { return false };
    let remaining = *fill;
    let Some(_) = crate::stretch_ring::consume(capacity, head, fill, remaining) else {
        return false;
    };
    *consumed = total;
    *active = false;
    *fadeouts = next_fadeouts;
    true
}
