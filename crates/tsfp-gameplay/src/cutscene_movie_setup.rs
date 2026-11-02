// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared readable movie setup arithmetic. No guest addresses or runtime defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SetupResult {
    pub crop: Option<(i32, i32)>,
    pub mode: Option<i32>,
    pub height_scratch: i32,
    pub converted_scalar: Option<f32>,
}

pub(crate) fn trunc_i32(value: f32) -> i32 {
    if !value.is_finite() || !(-2147483648.0..2147483648.0).contains(&value) {
        i32::MIN
    } else {
        value as i32
    }
}

/// -1 derives an inset from its nonnegative neighbour. Other invalid pairs retain crop.
/// Only modes zero and one update placement. Calibration comes from original runtime data.
pub fn configure(width: i32, height: i32, mode: i32, aspect: f32, inverse: f32) -> SetupResult {
    let mut dimensions = (width, height);
    let mut converted_scalar = None;
    let crop = if width == -1 && height >= 0 {
        let scalar = height as f32 * aspect;
        dimensions.0 = trunc_i32(scalar);
        converted_scalar = Some(scalar);
        Some(dimensions)
    } else if height == -1 && width >= 0 {
        let scalar = width as f32 * inverse;
        dimensions.1 = trunc_i32(scalar);
        converted_scalar = Some(scalar);
        Some(dimensions)
    } else if width >= 0 && height >= 0 {
        Some(dimensions)
    } else {
        None
    };
    SetupResult {
        crop,
        mode: match mode {
            0 | 1 => Some(mode),
            _ => None,
        },
        height_scratch: dimensions.1,
        converted_scalar,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extent {
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
