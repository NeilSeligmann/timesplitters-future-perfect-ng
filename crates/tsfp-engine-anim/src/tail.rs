//! Root motion bookkeeping at the end of `0x5AAA0` (T1668), MEASURED against the ORIGINAL with every callee stubbed
//! (`tools/t1668_tail_gold.py`, `tests/tail_golden.rs`, 500 fabricated cases).
//!
//! After the part chain `0x582A0(obj, matrix, 0, -1)` the original keeps the object's root position (`obj+0xE4..0xEC`,
//! written by the chain) and derives a per frame displacement:
//! * if `policy0 & 0x82 != 0` or `policy1 & 0x1800 != 0` (the two policy words of the clip's anim set entry, stored per slot at
//!   `0x7DE660` / `0x7DE664` by `0x5B980`): `delta (obj+0xFC..0x104) = position - previous (obj+0xF0..0xF8)`, then
//!   `previous = position`;
//! * otherwise `delta = 0` and `previous` is left alone.
//! Finally the owner object (`[obj]`) gets `flags (+0x28) |= 8` (matrices changed). The whole tail is skipped when the object
//! has no matrix pointer (`obj+0x10 == 0`).
//!
//! Note: `0x5AAA0` pushes FOUR arguments to `0x60670` (`arg0, arg1, state, scale`), which reads only the first three
//! (see [`crate::advance`]); T1875's four argument signature was right, the T1668 first pass wording "three args" meant used args.

/// The root motion words of an animated object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RootMotion {
    /// `obj+0xE4..0xEC` current root position.
    pub position: [f32; 3],
    /// `obj+0xF0..0xF8` position at the previous update.
    pub previous: [f32; 3],
    /// `obj+0xFC..0x104` displacement of the last update.
    pub delta: [f32; 3],
}

/// Owner flag set by the tail.
pub const OWNER_MATRICES_CHANGED: u32 = 8;

/// Whether the clip's policy words ask for root motion tracking.
pub fn tracks_root_motion(policy0: u32, policy1: u32) -> bool {
    policy0 & 0x82 != 0 || policy1 & 0x1800 != 0
}

impl RootMotion {
    /// The tail of `0x5AAA0` after the chain call.
    pub fn update(&mut self, policy0: u32, policy1: u32) {
        if tracks_root_motion(policy0, policy1) {
            for axis in 0..3 {
                self.delta[axis] = self.position[axis] - self.previous[axis];
            }
            self.previous = self.position;
        } else {
            self.delta = [0.0; 3];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_tracks_only_with_policy_bits() {
        let mut m = RootMotion {
            position: [3.0, 2.0, 1.0],
            previous: [1.0, 1.0, 1.0],
            delta: [9.0; 3],
        };
        m.update(0x1800, 0x2);
        assert_eq!((m.delta, m.previous), ([0.0; 3], [1.0; 3]));
        m.update(0x2, 0);
        assert_eq!((m.delta, m.previous), ([2.0, 1.0, 0.0], [3.0, 2.0, 1.0]));
        m.update(0, 0x800);
        assert_eq!(m.delta, [0.0; 3]);
    }
}

/// Result of [`step_time`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub time: f32,
    /// `state+0xC`: the time before the step (unchanged when the clip has no duration).
    pub previous_time: f32,
    /// `state+0x50`: 1 when the step passed a clip end (loop or clamp), else 0 (untouched for an empty clip).
    pub wrapped: Option<u32>,
    /// 1 when the time ran past the end, -1 when it ran before the start, 0 otherwise.
    pub direction: i32,
    /// Times the loop event `0xDB5E0` fires (once per forward wrap) when the event object asks for it.
    pub events: u32,
}

/// Maximum wraps before the step is declared non terminating (the original has no bound).
const MAX_WRAPS: u32 = 1 << 22;

/// `0x5ADC0` time part: advances `time` by `speed * (-1.0) / 60` (the constant at `0x4E794C` is -1.0, so the stored speed is
/// the negated per second rate), then clamps (clip not looping) or wraps (policy0 bit 0) it into `0..=duration`.
///
/// `fires_events` = the event object argument is present, its `+0x20` is 8 and `policy1 & 0x1000000`. After a wrap on a looping
/// clip the caller adds the root offset ([`apply_loop_root_offset`]). Returns `None` when a wrap would never end.
/// MEASURED: 800 cases of the original (`tools/t1668_step_gold.py`, `tests/step_golden.rs`).
pub fn step_time(
    duration: f32,
    time: f32,
    speed: f32,
    policy0: u32,
    fires_events: bool,
) -> Option<Step> {
    step_time_at(duration, time, speed, policy0, fires_events, -1.0, 60.0)
}

/// [`step_time`] with the two constants the original reads from memory (`[0x4E794C]` and `(float)[0x4E8CEC]`): they are -1.0 and 60 in the image
/// but guest code may change them, adapters on guest memory pass the live values.
pub fn step_time_at(
    duration: f32,
    time: f32,
    speed: f32,
    policy0: u32,
    fires_events: bool,
    rate: f32,
    per: f32,
) -> Option<Step> {
    let mut step = Step {
        time,
        previous_time: time,
        wrapped: None,
        direction: 0,
        events: 0,
    };
    if !(duration > 0.0) {
        return Some(step);
    }
    let looping = policy0 & 1 != 0;
    step.wrapped = Some(0);
    let mut t = speed * rate / per + time;
    if looping {
        if t > duration {
            step.direction = 1;
            let mut wraps = 0u32;
            loop {
                t -= duration;
                step.wrapped = Some(1);
                step.events += u32::from(fires_events);
                wraps += 1;
                if !(t > duration) {
                    break;
                }
                if wraps >= MAX_WRAPS {
                    return None;
                }
            }
        }
        if 0.0 > t {
            step.direction = -1;
            let mut wraps = 0u32;
            loop {
                t += duration;
                step.wrapped = Some(1);
                wraps += 1;
                if !(0.0 > t) {
                    break;
                }
                if wraps >= MAX_WRAPS {
                    return None;
                }
            }
        }
    } else {
        if t > duration {
            step.wrapped = Some(1);
            t = duration;
            step.direction = 1;
        }
        if 0.0 > t {
            step.wrapped = Some(1);
            t = 0.0;
            step.direction = -1;
        }
    }
    step.time = t;
    Some(step)
}

/// New root offset (`state+0x1C..0x24`) after a looping clip wrapped: the root jumped from the last key back to the first (forward
/// wrap, `direction` 1) or the reverse, so the object keeps its place. Float association as the original:
/// `(to + offset) - from`.
pub fn apply_loop_root_offset(
    offset: [f32; 3],
    direction: i32,
    first: [f32; 3],
    last: [f32; 3],
) -> [f32; 3] {
    let (to, from) = if direction > 0 {
        (first, last)
    } else {
        (last, first)
    };
    [
        to[0] + offset[0] - from[0],
        to[1] + offset[1] - from[1],
        to[2] + offset[2] - from[2],
    ]
}

#[cfg(test)]
mod step_tests {
    use super::*;

    #[test]
    fn step_clamps_or_wraps() {
        // speed -60 advances one second per update.
        let clamp = step_time(2.0, 1.5, -60.0, 0, false).unwrap();
        assert_eq!(
            (clamp.time, clamp.direction, clamp.wrapped),
            (2.0, 1, Some(1))
        );
        let wrap = step_time(2.0, 1.5, -60.0, 1, true).unwrap();
        assert_eq!((wrap.time, wrap.direction, wrap.events), (0.5, 1, 1));
        let back = step_time(2.0, 0.5, 60.0, 1, true).unwrap();
        assert_eq!((back.time, back.direction, back.events), (1.5, -1, 0));
        assert_eq!(step_time(0.0, 1.0, 5.0, 1, true).unwrap().wrapped, None);
        assert_eq!(
            apply_loop_root_offset([1.0; 3], 1, [0.0; 3], [2.0, 0.0, 0.0]),
            [-1.0, 1.0, 1.0]
        );
    }
}
