// SPDX-License-Identifier: GPL-3.0-or-later
//! `core` only key bracket helpers split out of `interp.rs` (T1918): `0x584A0` and the `0x5AAA0` weight. Documentation in `interp.rs`.

/// `0x475C78 / 0x4D1994`: one frame in seconds with the image constants (the frame rate float is not 60.0 at run time, see `queries::key_numbers_at`).
pub const FRAME_SECONDS: f32 = 1.0f32 / 60.0f32;

pub fn key_time(keys: &[u32], index: i32) -> Option<f32> {
    let key = *keys.get(usize::try_from(index).ok()?)?;
    Some((key as i32 as f32) * FRAME_SECONDS)
}

fn key_time_by(index: i32, key_at: &mut impl FnMut(usize) -> Option<u32>) -> Option<f32> {
    let key = key_at(usize::try_from(index).ok()?)?;
    Some((key as i32 as f32) * FRAME_SECONDS)
}

/// Midpoint step of the original: `cdq; sub eax, edx; sar eax, 1` (division by two rounding to zero).
pub fn half(value: i32) -> i32 {
    (value - (value >> 31)) >> 1
}

/// `0x584A0`. `keys` are the clip's key frame numbers, `duration` the value at `clip+0x18`.
pub fn key_bracket(keys: &[u32], duration: f32, time: f32) -> Option<(i32, i32)> {
    key_bracket_by(keys.len(), duration, time, |index| keys.get(index).copied())
}

/// `0x584A0` over an indexed key table. The callback lets guest-backed callers
/// binary-search large tables without copying them into a fixed-size buffer.
pub fn key_bracket_by(
    key_count: usize,
    duration: f32,
    time: f32,
    mut key_at: impl FnMut(usize) -> Option<u32>,
) -> Option<(i32, i32)> {
    let count = i32::try_from(key_count).ok()?;
    if 0.0f32 >= time || count == 1 {
        return Some((0, 0));
    }
    if count == 0 {
        return None;
    }
    if !(time < duration) && !time.is_nan() {
        return Some((count - 1, count - 1));
    }
    let (mut low, mut high) = (0i32, count - 1);
    let mut mid = half(high);
    for _ in 0..64 {
        let (first, second) = (
            key_time_by(mid, &mut key_at)?,
            key_time_by(mid.checked_add(1)?, &mut key_at)?,
        );
        if time == first {
            return Some((mid, mid));
        }
        if time == second {
            return Some((mid + 1, mid + 1));
        }
        if time > first && second >= time {
            return Some((mid, mid + 1));
        }
        if first > time {
            let next = half(mid - low) + low;
            high = mid;
            mid = next;
        } else if time > second {
            let next = mid + half(high - mid);
            low = mid;
            mid = next;
        } else {
            return None; // NaN: the original spins on the same midpoint
        }
    }
    None
}

/// Weight between the bracketed keys (`0x5AB3B..0x5AB6C`).
pub fn bracket_weight(keys: &[u32], low: i32, high: i32, time: f32) -> Option<f32> {
    bracket_weight_by(low, high, time, |index| keys.get(index).copied())
}

/// Weight over an indexed key table, reading only the two selected frame rows.
pub fn bracket_weight_by(
    low: i32,
    high: i32,
    time: f32,
    mut key_at: impl FnMut(usize) -> Option<u32>,
) -> Option<f32> {
    if low == high {
        return Some(0.0);
    }
    let (first, second) = (
        key_time_by(low, &mut key_at)?,
        key_time_by(high, &mut key_at)?,
    );
    Some((time - first) / (second - first))
}
