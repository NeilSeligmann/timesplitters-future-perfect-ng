// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline model of the presentation sink's stretch FIFO gain ramp.

/// Apply the sink's Q16 gain ramp to interleaved frames. Invalid geometry or
/// gains outside the sink's zero-to-unity range leave both inputs unchanged.
pub fn apply(samples: &mut [i16], channels: usize, gain: &mut u32, target: u32) -> bool {
    if channels == 0 || samples.len() % channels != 0 || *gain > 65536 || target > 65536 {
        return false;
    }
    if *gain == 65536 && target == 65536 {
        return true;
    }
    let frames = samples.len() / channels;
    for (frame, chunk) in samples.chunks_exact_mut(channels).enumerate() {
        let level = i64::from(*gain)
            + (i64::from(target) - i64::from(*gain)) * (frame as i64 + 1) / frames as i64;
        for sample in chunk {
            *sample = ((i64::from(*sample) * level) >> 16) as i16;
        }
    }
    *gain = target;
    true
}
