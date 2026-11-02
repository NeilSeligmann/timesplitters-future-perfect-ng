// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned scroll policy.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollView {
    pub offset: f32,
    pub content: f32,
    pub height: f32,
}

impl ScrollView {
    pub fn scroll(self, notches: i32) -> Option<f32> {
        if notches == 0
            || ![self.offset, self.content, self.height]
                .iter()
                .all(|v| v.is_finite())
            || self.height <= 0.0
            || self.content <= self.height
        {
            return None;
        }
        Some(
            (self.offset - notches.clamp(-8, 8) as f32 * 40.0)
                .clamp(0.0, self.content - self.height),
        )
    }
}
