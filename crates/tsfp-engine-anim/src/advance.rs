//! Animation time advance `0x60670` and the position jump gate `0x60920` (T1668).
//!
//! Both work on the per object animation track (stride 0x2C inside the owner, see [`advance_time`]).
//! `advance_time` is verified against the ORIGINAL (`tools/t1668_advance_gold.py`, `tests/advance_golden.rs`);
//! `position_jump` is a typed port from the disassembly with the null-key paths made explicit.
//!
//! Evidence: MEASURED for [`advance_time`] (600 fabricated cases under Unicorn, xemu level), INFERRED for
//! [`position_jump`] (read from machine code, no differential test: it needs the global key store `0x606E0`).

/// Frames per second used by `0x60920` (`[0x4D1994]` = 60.0).
const FPS: f32 = 60.0;
/// Squared distance (`[0x4785C8]` = 100.0) above which two consecutive keys count as a teleport.
const JUMP_SQUARED: f32 = 100.0;
/// Safety cap for the wrap loop, which in the original has no bound (NaN-free huge negative times).
const MAX_WRAPS: u32 = 1 << 22;

/// `0x60670(owner, index, state)`: the new `state+8` time.
///
/// * `duration` = `clip+0x18` of the clip in slot `state+4` (table `[0x7E3954]`).
/// * `owner_time` = `owner+8`, `track_offset` = `owner + index*0x2C + 0x34`, `old` = current `state+8`.
///
/// A duration that is not above zero (or NaN) leaves the time alone. Otherwise the time becomes
/// `owner_time - track_offset`, clamped down to `duration` (NaN is stored as is), and a negative result is
/// wrapped upwards by whole durations until it is no longer negative. `None` when the wrap would not terminate
/// (the original loops forever, e.g. for an enormous negative time or an infinite one).
pub fn advance_time(duration: f32, owner_time: f32, track_offset: f32, old: f32) -> Option<f32> {
    if !(duration > 0.0) {
        return Some(old);
    }
    let mut time = owner_time - track_offset;
    if time > duration {
        time = duration;
    }
    if 0.0 > time {
        for _ in 0..MAX_WRAPS {
            time = duration + time;
            if !(0.0 > time) {
                return Some(time);
            }
        }
        return None;
    }
    Some(time)
}

/// A key position read from the key store (`0x606E0(frame)`, fields `+0x10..+0x18`).
pub type KeyPosition = [f32; 3];

fn squared_distance(from: KeyPosition, to: KeyPosition) -> f32 {
    // Same association as the SSE code: (dz^2 + dy^2) + dx^2.
    let (dx, dy, dz) = (to[0] - from[0], to[1] - from[1], to[2] - from[2]);
    dz * dz + dy * dy + dx * dx
}

/// `0x60920(track)`: does the tracked position jump (more than 10 units) between the keys around `time`?
///
/// `frame_count` = `track+0`, `time` = `track+8`, `key(frame)` the key store lookup (clamped to the store like the
/// original, so it is only `None` when the store is empty; the original then reuses stale registers, which this port
/// reports as `None` instead of guessing). Only frames `1..=frame_count-10` are examined. Checks frame `f` against
/// `f+1`, and when the advanced time (`time + 1/60`) lands exactly two frames ahead also `f+1` against `f+2`.
pub fn position_jump(
    frame_count: i32,
    time: f32,
    key: impl Fn(i32) -> Option<KeyPosition>,
) -> Option<bool> {
    position_jump_at(frame_count, time, FPS, JUMP_SQUARED, 1.0f32 / 60.0f32, key)
}

/// [`position_jump`] with the constants the original reads from memory: frames per second `[0x4D1994]`, the squared distance `[0x4785C8]` and the
/// one frame step `[0x475C78] / (float)[0x4E8CEC]`. The image values are 60.0, 100.0 and 1/60 but the frame rate float is NOT 60.0 at run time (T1918).
pub fn position_jump_at(
    frame_count: i32,
    time: f32,
    fps: f32,
    jump_squared: f32,
    tick: f32,
    key: impl Fn(i32) -> Option<KeyPosition>,
) -> Option<bool> {
    let frame = crate::queries::cvttss2si(time * fps);
    if frame < 1 || frame > frame_count.wrapping_sub(10) {
        return Some(false);
    }
    let (first, second) = (key(frame)?, key(frame + 1)?);
    if squared_distance(first, second) > jump_squared {
        return Some(true);
    }
    let ahead = crate::queries::cvttss2si((tick + time) * fps);
    if ahead - frame != 2 {
        return Some(false);
    }
    let third = key(frame + 2)?;
    Some(squared_distance(second, third) > jump_squared)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_clamps_wraps_and_ignores_empty_clips() {
        assert_eq!(advance_time(0.0, 5.0, 1.0, 7.0), Some(7.0));
        assert_eq!(advance_time(2.0, 5.0, 1.0, 7.0), Some(2.0));
        assert_eq!(advance_time(2.0, 1.0, 2.5, 7.0), Some(0.5));
        assert_eq!(advance_time(2.0, f32::NEG_INFINITY, 0.0, 0.0), None);
        assert!(advance_time(2.0, f32::NAN, 0.0, 0.0).unwrap().is_nan());
    }

    #[test]
    fn jump_gate_examines_two_steps() {
        let far = |frame: i32| Some([frame as f32 * 11.0, 0.0, 0.0]);
        assert_eq!(position_jump(100, 0.5, far), Some(true));
        assert_eq!(position_jump(100, 0.0, far), Some(false));
        assert_eq!(position_jump(100, 2.0, far), Some(false));
        let near = |frame: i32| Some([frame as f32, 0.0, 0.0]);
        assert_eq!(position_jump(100, 0.5, near), Some(false));
        assert_eq!(position_jump(100, 0.5, |_| None), None);
    }
}
