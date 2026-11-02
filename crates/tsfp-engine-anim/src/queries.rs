//! Key number and clip event queries of the animation body (T1668, rule 10 batch 2): `0x585E0`, `0x613D0`, `0x61570`, `0x614B0`,
//! `0x61610`, `0x61630`. Tier 1 when `tests/queries_golden.rs` passes (seeded worlds against the original under Unicorn).
//!
//! A resident animation (`0x56FC0(id)`) has an event list: `short +6` count, 4 byte entries from `+8` (`u16 frame`, `u16 flags`). The key number of a
//! layer is `trunc(time * 60)`; `previous` is `trunc(layer.+0xC * 60)` (the time before the last step). Speed at `layer+0x34` is the negated rate
//! (see [`crate::tail::step_time`]): positive speed moves the time backwards.

/// One event entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub frame: u16,
    pub flags: u16,
}

/// `0x585E0(layer, &current, &previous)`: key numbers of the layer's time (`+8`) and previous time (`+0xC`).
pub fn key_numbers(time: f32, previous_time: f32) -> (i32, i32) {
    key_numbers_at(time, previous_time, 60.0f32)
}

/// Same with the frame rate float the original reads from `[0x4D1994]`. **It is NOT the constant 60.0 at run time** (T1918 live shadow run: the lifted
/// `0x585E0` returned (105, 104) where 60.0 gives (104, 103) for layer times 1.749999 and 1.7333324): callers on guest memory must pass the live value.
pub fn key_numbers_at(time: f32, previous_time: f32, frames_per_second: f32) -> (i32, i32) {
    (
        cvttss2si(time * frames_per_second),
        cvttss2si(previous_time * frames_per_second),
    )
}

/// x86 `cvttss2si`: truncation toward zero, NaN and values outside `i32` give the "integer indefinite" value `i32::MIN` (Rust's `as` saturates and maps NaN
/// to 0: found by the T1918 live shadow run, where a layer time outside the range made `0x61570` disagree with the original).
pub fn cvttss2si(value: f32) -> i32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

/// `0x613D0(id, mask, n)`: frame of the `n`-th event (1 based) whose flags overlap `mask`; `None` when the animation is not resident or has
/// fewer matches (the original returns -1).
pub fn nth_event_frame(events: Option<&[Event]>, mask: u32, n: i32) -> Option<u32> {
    let events = events?;
    let mut remaining = n;
    for event in events {
        if u32::from(event.flags) & mask & 0xFFFF != 0 {
            remaining -= 1;
            if remaining == 0 {
                return Some(u32::from(event.frame));
            }
        }
    }
    None
}

/// `0x61450(id, mask)`: does any event of the resident animation `id` have flags overlapping `mask` (16 bit test, no time window)? An id
/// of 0 or less is answered before the original looks the animation up (`events` must then not be consulted, so it is ignored here).
pub fn event_flags_overlap(id: i32, events: Option<&[Event]>, mask: u32) -> bool {
    id > 0
        && events.is_some_and(|list| {
            list.iter()
                .any(|event| u32::from(event.flags) & mask & 0xFFFF != 0)
        })
}

/// `0x61570(obj, mask)`: does an event with flags in `mask` lie in the key window `(previous, current]` of the last step (wrapped
/// when `previous > current`: `frame > previous` or `frame <= current`); an event at frame 0 counts when `current` is 0.
pub fn event_in_window(events: Option<&[Event]>, mask: u32, current: i32, previous: i32) -> bool {
    let Some(events) = events else { return false };
    events
        .iter()
        .filter(|e| u32::from(e.flags) & mask & 0xFFFF != 0)
        .any(|event| {
            let frame = i32::from(event.frame);
            if frame == 0 {
                current == 0
            } else if previous > current {
                frame > previous || frame <= current
            } else {
                frame > previous && frame <= current
            }
        })
}

/// `0x614B0(obj, mask, n)`: whether the step just taken passed the `n`-th matching event. Positive speed uses the window `(previous, current]`
/// (an event at frame 0 counts when `current` is 0), negative speed `[current, previous)`; both wrap when the key numbers cross the loop.
pub fn passed_event(
    events: Option<&[Event]>,
    mask: u32,
    n: i32,
    speed: f32,
    current: i32,
    previous: i32,
) -> bool {
    let Some(frame) = nth_event_frame(events, mask, n) else {
        return false;
    };
    let frame = frame as i32;
    if speed > 0.0 {
        if frame == 0 {
            return current == 0;
        }
        if previous > current {
            frame > previous || frame <= current
        } else {
            frame > previous && frame <= current
        }
    } else if 0.0 > speed {
        if previous < current {
            frame < previous || frame >= current
        } else {
            frame >= current && frame < previous
        }
    } else {
        false
    }
}

/// `0x61630(obj, mask, n)`: the active layer's current key is at or past the `n`-th matching event (false without one).
pub fn at_or_past_event(events: Option<&[Event]>, mask: u32, n: i32, current: i32) -> bool {
    nth_event_frame(events, mask, n).is_some_and(|frame| current >= frame as i32)
}
