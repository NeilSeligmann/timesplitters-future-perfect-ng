// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline model of the presentation sink's smoothed ring fill.

#[derive(Clone, Copy, Debug, Default)]
pub struct FillSmoother {
    pub frames: f64,
    pub valid: bool,
}

impl FillSmoother {
    /// Apply one device pull; a zero smoothing window leaves state untouched.
    pub fn update(&mut self, ring_fill: usize, smooth_frames: usize, pull_frames: usize) {
        if smooth_frames == 0 {
            return;
        }
        if !self.valid {
            self.frames = ring_fill as f64;
            self.valid = true;
            return;
        }
        let alpha = pull_frames as f64 / (smooth_frames as f64 + pull_frames as f64);
        self.frames += alpha * (ring_fill as f64 - self.frames);
    }
}
