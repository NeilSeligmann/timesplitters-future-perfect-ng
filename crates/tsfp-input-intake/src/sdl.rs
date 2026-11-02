// SPDX-License-Identifier: GPL-3.0-or-later
//! Bevy devices to the SDL3 codes of the `window_input` contract (moved from tsfp-bevy `events.rs` by T3820, re-exported there).

use bevy_input::gamepad::{GamepadAxis, GamepadButton};
use bevy_input::keyboard::KeyCode;

pub const SDLK_UNMAPPED: u32 = 0x2000_0000;

/// SDL3 keycode of a physical key (the codes `window_input::key_name` names), `SDLK_UNMAPPED` for the rest.
pub fn sdl_keycode(key: KeyCode) -> u32 {
    use KeyCode::*;
    match key {
        KeyA => 97,
        KeyB => 98,
        KeyC => 99,
        KeyD => 100,
        KeyE => 101,
        KeyF => 102,
        KeyG => 103,
        KeyH => 104,
        KeyI => 105,
        KeyJ => 106,
        KeyK => 107,
        KeyL => 108,
        KeyM => 109,
        KeyN => 110,
        KeyO => 111,
        KeyP => 112,
        KeyQ => 113,
        KeyR => 114,
        KeyS => 115,
        KeyT => 116,
        KeyU => 117,
        KeyV => 118,
        KeyW => 119,
        KeyX => 120,
        KeyY => 121,
        KeyZ => 122,
        Digit0 => 48,
        Digit1 => 49,
        Digit2 => 50,
        Digit3 => 51,
        Digit4 => 52,
        Digit5 => 53,
        Digit6 => 54,
        Digit7 => 55,
        Digit8 => 56,
        Digit9 => 57,
        ArrowUp => 0x4000_0052,
        ArrowDown => 0x4000_0051,
        ArrowLeft => 0x4000_0050,
        ArrowRight => 0x4000_004F,
        Enter => 13,
        NumpadEnter => 0x4000_0058,
        Backspace => 8,
        Delete => 0x7F,
        Space => 32,
        Tab => 9,
        Escape => 27,
        F1 => 0x4000_003A,
        F2 => 0x4000_003B,
        F3 => 0x4000_003C,
        F4 => 0x4000_003D,
        F5 => 0x4000_003E,
        F6 => 0x4000_003F,
        F7 => 0x4000_0040,
        F8 => 0x4000_0041,
        F9 => 0x4000_0042,
        F10 => 0x4000_0043,
        F11 => 0x4000_0044,
        F12 => 0x4000_0045,
        ControlLeft => 0x4000_00E0,
        ShiftLeft => 0x4000_00E1,
        AltLeft => 0x4000_00E2,
        ControlRight => 0x4000_00E4,
        ShiftRight => 0x4000_00E5,
        AltRight => 0x4000_00E6,
        _ => SDLK_UNMAPPED,
    }
}

/// SDL3 gamepad button index of a Bevy button, `None` for the analog triggers and unmapped buttons.
pub fn sdl_button(button: GamepadButton) -> Option<u8> {
    Some(match button {
        GamepadButton::South => 0,
        GamepadButton::East => 1,
        GamepadButton::West => 2,
        GamepadButton::North => 3,
        GamepadButton::Select => 4,
        GamepadButton::Mode => 5,
        GamepadButton::Start => 6,
        GamepadButton::LeftThumb => 7,
        GamepadButton::RightThumb => 8,
        GamepadButton::LeftTrigger => 9,
        GamepadButton::RightTrigger => 10,
        GamepadButton::DPadUp => 11,
        GamepadButton::DPadDown => 12,
        GamepadButton::DPadLeft => 13,
        GamepadButton::DPadRight => 14,
        _ => return None,
    })
}

/// Axis index and sign of a Bevy stick axis in the SDL convention (Y negated, see the module doc).
pub fn sdl_stick(axis: GamepadAxis) -> Option<(u8, f32)> {
    Some(match axis {
        GamepadAxis::LeftStickX => (0, 1.0),
        GamepadAxis::LeftStickY => (1, -1.0),
        GamepadAxis::RightStickX => (2, 1.0),
        GamepadAxis::RightStickY => (3, -1.0),
        _ => return None,
    })
}

/// Float -1..1 to the SDL int16 range with no deadzone.
pub fn to_i16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * 32767.0).round() as i16
}
