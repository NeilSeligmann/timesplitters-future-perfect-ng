//! Heading value track `0x58690` and its current-time reader `0x5A220` (T1668, rule 10 batch 4). Tier 1 via `tests/heading_golden.rs`
//! (seeded worlds against the original under Unicorn, real `0x584A0` and CRT `floor`).
//!
//! A clip may carry a value track (`clip+0x2C`, one f32 per key, radians): `0x58690(clip, time)` interpolates it linearly between the keys
//! around `time` (bracket `0x584A0`), converts to degrees (`* 57.29578`) and wraps the angle into `[0, 360)` (`deg - floor(deg / 360) * 360`,
//! the division as a multiplication by `1/360`, x87 double arithmetic). Without a track it returns -1.0. `0x5A220(obj)` reads it at the
//! current layer's time (`body+0xDC`) and returns 0.0 when the clip has no track.
use crate::fmath::floor_f64;
use crate::keybracket::{bracket_weight, key_bracket};

const RAD_TO_DEG: f32 = f32::from_bits(0x4265_2EE0); // [0x478638]
const INV_FULL_TURN: f32 = f32::from_bits(0x3B36_0B61); // [0x475CB4]
const FULL_TURN: f32 = 360.0; // [0x475C88]

/// `0x58690`. `values` = the track (one f32 per key) or `None`.
pub fn heading(values: Option<&[f32]>, keys: &[u32], duration: f32, time: f32) -> Option<f64> {
    let Some(values) = values else {
        return Some(-1.0);
    };
    let (low, high) = key_bracket(keys, duration, time)?;
    let weight = bracket_weight(keys, low, high, time)?;
    let (a, b) = (
        *values.get(usize::try_from(low).ok()?)?,
        *values.get(usize::try_from(high).ok()?)?,
    );
    let degrees = ((b - a) * weight + a) * RAD_TO_DEG;
    let turns = floor_f64(f64::from(degrees) * f64::from(INV_FULL_TURN));
    Some(f64::from(degrees) - turns * f64::from(FULL_TURN))
}

/// `0x5A220`: the heading at the layer time, 0.0 for a clip without a track.
pub fn current_heading(
    values: Option<&[f32]>,
    keys: &[u32],
    duration: f32,
    layer_time: f32,
) -> Option<f64> {
    if values.is_none() {
        return Some(0.0);
    }
    heading(values, keys, duration, layer_time)
}
