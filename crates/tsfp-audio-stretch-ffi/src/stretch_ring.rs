// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline model of the stretch sink's ring loading and consumption.

/// Copy frames from the ring head into contiguous interleaved scratch space.
/// Invalid geometry leaves the destination unchanged.
pub fn load(ring: &[i16], channels: usize, head: usize, output: &mut [i16]) -> bool {
    if channels == 0
        || ring.is_empty()
        || ring.len() % channels != 0
        || output.len() % channels != 0
        || head >= ring.len() / channels
    {
        return false;
    }
    let frames = ring.len() / channels;
    for (offset, frame) in output.chunks_exact_mut(channels).enumerate() {
        let index = head.wrapping_add(offset) % frames * channels;
        frame.copy_from_slice(&ring[index..index + channels]);
    }
    true
}

/// Consume at most the filled frames, returning the number consumed.
pub fn consume(
    capacity: usize,
    head: &mut usize,
    fill: &mut usize,
    requested: usize,
) -> Option<usize> {
    if capacity == 0 || *head >= capacity || *fill > capacity {
        return None;
    }
    let count = requested.min(*fill);
    *head = head.wrapping_add(count) % capacity;
    *fill -= count;
    Some(count)
}
