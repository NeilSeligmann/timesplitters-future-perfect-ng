// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline models of draining the stretch sink FIFO.

/// Copy a request entirely contained in the active FIFO. A request that needs
/// another engine block is left to the caller and changes nothing.
pub fn drain_active(
    fifo: &[i16],
    channels: usize,
    position: &mut usize,
    length: usize,
    output: &mut [i16],
) -> Option<usize> {
    if channels == 0
        || fifo.len() % channels != 0
        || output.len() % channels != 0
        || length > fifo.len() / channels
        || *position > length
        || output.len() / channels > length - *position
    {
        return None;
    }
    let frames = output.len() / channels;
    let start = *position * channels;
    output.copy_from_slice(&fifo[start..start + output.len()]);
    *position += frames;
    Some(frames)
}

/// Copy available interleaved frames and reset the FIFO when fully drained.
/// Invalid geometry leaves the output and state unchanged.
pub fn drain(
    fifo: &[i16],
    channels: usize,
    position: &mut usize,
    length: &mut usize,
    output: &mut [i16],
) -> Option<usize> {
    if channels == 0
        || fifo.len() % channels != 0
        || output.len() % channels != 0
        || *length > fifo.len() / channels
        || *position > *length
    {
        return None;
    }
    let count = (*length - *position).min(output.len() / channels);
    let start = *position * channels;
    output[..count * channels].copy_from_slice(&fifo[start..start + count * channels]);
    *position += count;
    if *position >= *length && !output.is_empty() {
        *position = 0;
        *length = 0;
    }
    Some(count)
}
