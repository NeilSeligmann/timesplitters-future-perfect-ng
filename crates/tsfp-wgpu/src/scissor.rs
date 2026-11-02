// SPDX-License-Identifier: GPL-3.0-or-later
//! Scissor conversion for the wgpu replacement renderer.

/// Clamp a signed device scissor to the target. `None` means no pixels survive clipping.
pub(crate) fn scissor_rect(
    scissor: Option<[i32; 4]>,
    width: u32,
    height: u32,
) -> Option<(u32, u32, u32, u32)> {
    let [left, top, right, bottom] = scissor.unwrap_or([0, 0, width as i32, height as i32]);
    let left = left.clamp(0, width as i32) as u32;
    let top = top.clamp(0, height as i32) as u32;
    let right = right.clamp(left as i32, width as i32) as u32;
    let bottom = bottom.clamp(top as i32, height as i32) as u32;
    if right == left || bottom == top {
        return None;
    }
    Some((left, top, right - left, bottom - top))
}
