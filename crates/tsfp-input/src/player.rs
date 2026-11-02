// SPDX-License-Identifier: GPL-3.0-or-later
//! `PlayerInput`, the one boundary value of the contract (docs/t-contract-input.md section 2). Plain `Copy` data.

pub use crate::look::Vec2;

/// Guest button layout (the held word `+0xC` of a pad record, docs/t-contract-input.md section 4.1).
pub type ButtonBits = u32;

pub mod bits {
    use super::ButtonBits;
    pub const DPAD_LEFT: ButtonBits = 0x1;
    pub const DPAD_RIGHT: ButtonBits = 0x2;
    pub const DPAD_UP: ButtonBits = 0x4;
    pub const DPAD_DOWN: ButtonBits = 0x8;
    pub const X: ButtonBits = 0x10;
    pub const B: ButtonBits = 0x20;
    pub const Y: ButtonBits = 0x40;
    pub const A: ButtonBits = 0x80;
    pub const BACK: ButtonBits = 0x100;
    pub const START: ButtonBits = 0x200;
    pub const LT: ButtonBits = 0x400;
    pub const RT: ButtonBits = 0x800;
    pub const L3: ButtonBits = 0x1000;
    pub const R3: ButtonBits = 0x2000;
    pub const BLACK: ButtonBits = 0x4000;
    pub const WHITE: ButtonBits = 0x8000;
    pub const ALL: ButtonBits = 0xFFFF;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Pad(u32),
    MouseKeyboard,
    Script,
    Replay,
    Network { peer: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    /// Device stick, [-1, 1], x right positive, y up positive; goes through the shaping preset.
    Stick(Vec2),
    /// Final turn for this step: yaw positive = turn right, pitch positive = look up.
    Delta { yaw_deg: f32, pitch_deg: f32 },
}

/// Meaningful in the frontend (T2162). Stage 1 leaves it at the default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MenuIntent {
    pub accept: bool,
    pub back: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct AssistMask(pub u8);

impl AssistMask {
    pub const NONE: AssistMask = AssistMask(0);
    pub const INVERSE_LOOK: u8 = 1;
    pub const AUTO_LOOKAHEAD: u8 = 2;
    pub const AUTO_AIM: u8 = 4;
    pub const PITCH_DEAD_ZONE: u8 = 8;
    pub const EDGE_BOOST: u8 = 16;
    /// Whatever the saved profile says.
    pub const PROFILE: AssistMask = AssistMask(0x1F);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerInput {
    pub step: u64,
    pub slot: u8,
    pub source: SourceKind,
    pub held: ButtonBits,
    pub pressed: ButtonBits,
    pub released: ButtonBits,
    pub move_axis: Vec2,
    pub look: Look,
    pub triggers: [f32; 2],
    pub menu: MenuIntent,
    pub assists: AssistMask,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RumbleEffect {
    Off,
    Dual {
        low: f32,
        high: f32,
        duration_ms: u32,
    },
}

impl RumbleEffect {
    /// Original pad feedback intensity is one signed scalar, with full scale 128.
    /// T2174 hybrid measurement corroborates the original 0x18FF70 conversion.
    /// Duration is a host refresh policy supplied by the caller, not encoded in the word.
    pub fn from_original_level(level: u32, duration_ms: u32) -> Self {
        let level = (level as i32).clamp(0, 128) as u32;
        if level == 0 {
            return Self::Off;
        }
        // Preserve the original 16-bit motor quantisation, for both motors.
        let motor = (65535 * level / 128) as f32 / 65535.0;
        Self::Dual {
            low: motor,
            high: motor,
            duration_ms,
        }
    }
}

#[cfg(test)]
mod rumble_measurements {
    use super::RumbleEffect;
    #[test]
    fn original_scalar_matches_hybrid_motor_reports() {
        for (level, motor) in [
            (127, 65023),
            (52, 26623),
            (22, 11263),
            (61, 31231),
            (31, 15871),
            (25, 12799),
        ] {
            assert_eq!(
                RumbleEffect::from_original_level(level, 20),
                RumbleEffect::Dual {
                    low: motor as f32 / 65535.0,
                    high: motor as f32 / 65535.0,
                    duration_ms: 20
                }
            );
        }
        assert_eq!(RumbleEffect::from_original_level(0, 20), RumbleEffect::Off);
        assert_eq!(
            RumbleEffect::from_original_level(u32::MAX, 20),
            RumbleEffect::Off
        );
        assert_eq!(
            RumbleEffect::from_original_level(129, 20),
            RumbleEffect::Dual {
                low: 1.0,
                high: 1.0,
                duration_ms: 20
            }
        );
    }
}

pub trait HapticsSink {
    fn rumble(&mut self, slot: u8, effect: RumbleEffect);
}
