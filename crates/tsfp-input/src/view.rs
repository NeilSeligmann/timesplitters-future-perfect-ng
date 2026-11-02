// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared native view contract, also consumed by the no_std live adapter.
/// Rust-owned view angles, independently updateable at the display rate.
/// Yaw decreases when looking right; pitch increases when looking up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewAngles {
    pub yaw: f32,
    pub pitch: f32,
}

/// Camera constraints for physical mouse counts converted to degrees by the router.
/// There is deliberately no simulation time, ramp, profile or frame-rate argument.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewConstraints {
    pub zoom: f32,
    pub pitch_limit: f32,
}
impl Default for ViewConstraints {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pitch_limit: 60.0,
        }
    }
}
impl ViewAngles {
    /// Applies physical count degrees exactly once to owned view state.
    pub fn apply_degrees(
        &mut self,
        yaw_degrees: f32,
        pitch_degrees: f32,
        constraints: ViewConstraints,
    ) -> bool {
        if !self.yaw.is_finite()
            || !self.pitch.is_finite()
            || !yaw_degrees.is_finite()
            || !pitch_degrees.is_finite()
            || !constraints.zoom.is_finite()
            || constraints.zoom < 0.0
            || !constraints.pitch_limit.is_finite()
            || constraints.pitch_limit < 0.0
        {
            return false;
        }
        if yaw_degrees == 0.0 && pitch_degrees == 0.0 {
            return true;
        }
        let yaw = self.yaw - yaw_degrees * constraints.zoom;
        let pitch = self.pitch + pitch_degrees * constraints.zoom;
        if !yaw.is_finite() || !pitch.is_finite() {
            return false;
        }
        self.yaw = {
            let wrapped = yaw % 360.0;
            if wrapped < 0.0 {
                wrapped + 360.0
            } else {
                wrapped
            }
        };
        self.pitch = pitch.clamp(-constraints.pitch_limit, constraints.pitch_limit);
        true
    }
}
