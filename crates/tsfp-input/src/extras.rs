// SPDX-License-Identifier: GPL-3.0-or-later
//! T2211: per-player input options that `shaping::PlayerSettings` does not carry (mouse smoothing and
//! acceleration, raw look, scope sensitivity multiplier, rumble strength, device assignment). A sibling
//! record so the existing `PlayerSettings` literals and its file parser stay unchanged. Defaults are
//! today's behaviour: no smoothing, no acceleration, raw look off, scope multiplier 1, full rumble,
//! automatic device assignment. Consumers (look path, rumble, router) are not wired yet, see
//! docs/t2211-input-extras.md.

/// Which device drives a player.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeviceAssignment {
    /// The router's current rule (first pad to press start, mouse and keyboard to player 1).
    #[default]
    Auto,
    KeyboardMouse,
    /// Pad port 0..=3.
    Pad(u8),
    /// The player has no device (local split screen seat left empty).
    Unassigned,
}

impl DeviceAssignment {
    /// Cycler order: Auto, KeyboardMouse, Pad 0..=3, Unassigned.
    pub const ALL: [DeviceAssignment; 7] = [
        DeviceAssignment::Auto,
        DeviceAssignment::KeyboardMouse,
        DeviceAssignment::Pad(0),
        DeviceAssignment::Pad(1),
        DeviceAssignment::Pad(2),
        DeviceAssignment::Pad(3),
        DeviceAssignment::Unassigned,
    ];

    pub fn index(self) -> Option<u32> {
        Self::ALL.iter().position(|d| *d == self).map(|i| i as u32)
    }

    pub fn from_index(index: u32) -> Option<DeviceAssignment> {
        Self::ALL.get(index as usize).copied()
    }
}

/// Inclusive limits shared by the UI rows and any file parser.
pub const SMOOTHING_FRAMES_MAX: u8 = 8;
pub const ACCELERATION_RANGE: (f32, f32) = (0.0, 4.0);
pub const SCOPE_MULTIPLIER_RANGE: (f32, f32) = (0.05, 4.0);
pub const RUMBLE_RANGE: (f32, f32) = (0.0, 1.0);

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerExtras {
    /// Mouse look averaging window in frames; 0 or 1 is off.
    pub smoothing_frames: u8,
    /// Extra gain per count of speed (0 is off, linear response).
    pub acceleration: f32,
    /// Raw look: mouse deltas bypass the stick emulation and drive the camera directly.
    pub raw_look: bool,
    /// Sensitivity factor while zoomed or scoped.
    pub scope_multiplier: f32,
    /// Rumble strength, 0 off, 1 the game's own strength.
    pub rumble: f32,
    pub device: DeviceAssignment,
}

impl Default for PlayerExtras {
    fn default() -> Self {
        Self {
            smoothing_frames: 0,
            acceleration: 0.0,
            raw_look: false,
            scope_multiplier: 1.0,
            rumble: 1.0,
            device: DeviceAssignment::Auto,
        }
    }
}

impl PlayerExtras {
    /// Look gain for one frame of mouse motion: `1 + acceleration * speed` (speed in counts per frame),
    /// times the scope multiplier while zoomed. Pure, so the look path can adopt it unchanged.
    pub fn look_gain(&self, speed: f32, zoomed: bool) -> f32 {
        let accel = 1.0 + self.acceleration * speed.abs();
        if zoomed {
            accel * self.scope_multiplier
        } else {
            accel
        }
    }

    /// Scales a game rumble motor value (0..=65535).
    pub fn scale_rumble(&self, motor: u16) -> u16 {
        (f32::from(motor) * self.rumble.clamp(0.0, 1.0)).round() as u16
    }
}
