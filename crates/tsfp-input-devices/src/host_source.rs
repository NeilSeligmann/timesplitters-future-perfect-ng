// SPDX-License-Identifier: GPL-3.0-or-later
//! Held-key model of the host keyboard and gamepad pad sources (`src/input/xinput_host_source.c`, T751/T731).
//!
//! The mapping tables are FABRICATED conventions, copied from the C file (a C test compares every row of both).
//! C keeps the event feed and the packet-number path (`xinput_hle_set_synthetic_pad_state`); this model keeps the
//! held bits, the gamepad axes, the ignore counters and composes the pad state.

pub const AXIS_COUNT: usize = 6;
const AXIS_LT: usize = 4;
pub const ANALOG_COUNT: usize = 8;
const LEFT_TRIGGER: u8 = 6;
const RIGHT_TRIGGER: u8 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    Keyboard = 1,
    Gamepad = 2,
}

impl Device {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            1 => Some(Self::Keyboard),
            2 => Some(Self::Gamepad),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    None,
    Digital(u16),
    Analog(u8),
    /// Thumb axis 0 LX, 1 LY, 2 RX, 3 RY and the direction (+1 or -1).
    Stick(u8, i8),
}

pub struct Row {
    pub name: &'static str,
    pub target: Target,
}

const fn dig(name: &'static str, bit: u16) -> Row {
    Row {
        name,
        target: Target::Digital(bit),
    }
}
const fn ana(name: &'static str, index: u8) -> Row {
    Row {
        name,
        target: Target::Analog(index),
    }
}
const fn stk(name: &'static str, axis: u8, direction: i8) -> Row {
    Row {
        name,
        target: Target::Stick(axis, direction),
    }
}
const fn none(name: &'static str) -> Row {
    Row {
        name,
        target: Target::None,
    }
}

const UP: u16 = 0x0001;
const DOWN: u16 = 0x0002;
const LEFT: u16 = 0x0004;
const RIGHT: u16 = 0x0008;
const START: u16 = 0x0010;
const BACK: u16 = 0x0020;
const LTHUMB: u16 = 0x0040;
const RTHUMB: u16 = 0x0080;
const A: u8 = 0;
const B: u8 = 1;
const X: u8 = 2;
const Y: u8 = 3;
const BLACK: u8 = 4;
const WHITE: u8 = 5;

pub static KEYBOARD: [Row; 28] = [
    dig("UP", UP),
    dig("DOWN", DOWN),
    dig("LEFT", LEFT),
    dig("RIGHT", RIGHT),
    dig("ENTER", START),
    dig("ESCAPE", START),
    dig("BACKSPACE", BACK),
    dig("E", LTHUMB),
    dig("R", RTHUMB),
    ana("Z", A),
    ana("X", B),
    ana("A", X),
    ana("S", Y),
    ana("Q", BLACK),
    ana("W", WHITE),
    ana("1", LEFT_TRIGGER),
    ana("2", RIGHT_TRIGGER),
    stk("I", 1, 1),
    stk("K", 1, -1),
    stk("J", 0, -1),
    stk("L", 0, 1),
    stk("T", 3, 1),
    stk("G", 3, -1),
    stk("F", 2, -1),
    stk("H", 2, 1),
    none("SPACE"),
    none("TAB"),
    none("F1"),
];

pub static GAMEPAD: [Row; 15] = [
    ana("A", A),
    ana("B", B),
    ana("X", X),
    ana("Y", Y),
    ana("LB", BLACK),
    ana("RB", WHITE),
    dig("START", START),
    dig("BACK", BACK),
    dig("LSTICK", LTHUMB),
    dig("RSTICK", RTHUMB),
    dig("DPAD_UP", UP),
    dig("DPAD_DOWN", DOWN),
    dig("DPAD_LEFT", LEFT),
    dig("DPAD_RIGHT", RIGHT),
    none("GUIDE"),
];

pub const AXES: [&str; AXIS_COUNT] = ["LX", "LY", "RX", "RY", "LT", "RT"];

pub fn table(device: Device) -> &'static [Row] {
    match device {
        Device::Keyboard => &KEYBOARD,
        Device::Gamepad => &GAMEPAD,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Down = 1,
    Up = 2,
    Axis = 3,
}

impl Action {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            1 => Some(Self::Down),
            2 => Some(Self::Up),
            3 => Some(Self::Axis),
            _ => None,
        }
    }
}

/// The pad state the held buttons describe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Composed {
    pub digital: u16,
    pub analog: [u8; ANALOG_COUNT],
    pub thumbs: [i16; 4],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub applied: u64,
    pub unknown: u64,
    pub other_device: u64,
    pub unmapped: u64,
}

pub struct HostSource {
    device: Device,
    held: u64,
    axes: [i16; AXIS_COUNT],
    pub counters: Counters,
}

impl HostSource {
    pub fn new(device: Device) -> Self {
        Self {
            device,
            held: 0,
            axes: [0; AXIS_COUNT],
            counters: Counters::default(),
        }
    }

    /// Opposite keys held together cancel (FABRICATED).
    fn stick_from_keys(&self, axis: u8) -> i16 {
        let (mut positive, mut negative) = (false, false);
        for (i, row) in KEYBOARD.iter().enumerate() {
            if self.held & (1 << i) == 0 {
                continue;
            }
            if let Target::Stick(a, direction) = row.target {
                if a == axis {
                    if direction > 0 {
                        positive = true;
                    } else {
                        negative = true;
                    }
                }
            }
        }
        match (positive, negative) {
            (true, false) => 32767,
            (false, true) => -32768,
            _ => 0,
        }
    }

    pub fn compose(&self) -> Composed {
        let mut state = Composed::default();
        for (i, row) in table(self.device).iter().enumerate() {
            if self.held & (1 << i) == 0 {
                continue;
            }
            match row.target {
                Target::Digital(bit) => state.digital |= bit,
                Target::Analog(index) => state.analog[usize::from(index)] = 255,
                _ => {}
            }
        }
        if self.device == Device::Keyboard {
            for axis in 0..4u8 {
                state.thumbs[usize::from(axis)] = self.stick_from_keys(axis);
            }
        } else {
            state.thumbs.copy_from_slice(&self.axes[..4]);
            // Host triggers 0..32767 scale to 0..255 (FABRICATED); no dead zone.
            for t in 0..2 {
                let host = self.axes[AXIS_LT + t];
                state.analog[usize::from(LEFT_TRIGGER) + t] =
                    if host <= 0 { 0 } else { (host >> 7) as u8 };
            }
        }
        state
    }

    /// Apply one event. True when it was an applicable known event.
    pub fn apply(&mut self, device: u32, action: u32, code: u32, value: i32) -> bool {
        if Device::from_raw(device) != Some(self.device) {
            self.counters.other_device += 1;
            return false;
        }
        let rows = table(self.device);
        let gamepad = self.device == Device::Gamepad;
        if Action::from_raw(action) == Some(Action::Axis) {
            if !gamepad || code as usize >= AXIS_COUNT {
                self.counters.unknown += 1;
                eprintln!(
                    "xinput host source: axis event code {code} is not a {} axis, IGNORED",
                    if gamepad {
                        "gamepad"
                    } else {
                        "keyboard (it has none)"
                    }
                );
                return false;
            }
            self.axes[code as usize] = value.clamp(-32768, 32767) as i16;
            return true;
        }
        let Some(action) = Action::from_raw(action) else {
            self.counters.unknown += 1;
            eprintln!("xinput host source: unknown action {action}, IGNORED");
            return false;
        };
        let Some(row) = rows.get(code as usize) else {
            self.counters.unknown += 1;
            eprintln!(
                "xinput host source: unknown {} code {code}, IGNORED",
                if gamepad { "button" } else { "key" }
            );
            return false;
        };
        if row.target == Target::None {
            self.counters.unmapped += 1;
            return false;
        }
        let bit = 1u64 << code;
        if action == Action::Down {
            self.held |= bit;
        } else {
            self.held &= !bit;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(device: Device, name: &str) -> u32 {
        table(device).iter().position(|r| r.name == name).unwrap() as u32
    }

    #[test]
    fn keyboard_sticks_cancel_and_buttons_compose() {
        let mut s = HostSource::new(Device::Keyboard);
        assert!(s.apply(1, 1, code(Device::Keyboard, "I"), 0));
        assert_eq!(s.compose().thumbs[1], 32767);
        assert!(s.apply(1, 1, code(Device::Keyboard, "K"), 0));
        assert_eq!(s.compose().thumbs[1], 0);
        assert!(s.apply(1, 1, code(Device::Keyboard, "ENTER"), 0));
        assert!(s.apply(1, 1, code(Device::Keyboard, "Z"), 0));
        let c = s.compose();
        assert_eq!((c.digital, c.analog[0]), (START, 255));
        assert!(!s.apply(1, 1, code(Device::Keyboard, "SPACE"), 0));
        assert_eq!(s.counters.unmapped, 1);
        assert!(!s.apply(2, 1, 0, 0));
        assert_eq!(s.counters.other_device, 1);
        assert!(!s.apply(1, 1, 99, 0));
        assert!(!s.apply(1, 3, 0, 5));
        assert_eq!(s.counters.unknown, 2);
    }

    #[test]
    fn gamepad_axes_clamp_and_triggers_scale() {
        let mut s = HostSource::new(Device::Gamepad);
        assert!(s.apply(2, 3, 0, 100_000));
        assert!(s.apply(2, 3, 1, -100_000));
        assert!(s.apply(2, 3, 4, 32767));
        assert!(s.apply(2, 3, 5, -5));
        let c = s.compose();
        assert_eq!(&c.thumbs[..2], &[32767, -32768]);
        assert_eq!((c.analog[6], c.analog[7]), (255, 0));
        assert!(s.apply(2, 1, code(Device::Gamepad, "LB"), 0));
        assert_eq!(s.compose().analog[4], 255);
        assert!(s.apply(2, 2, code(Device::Gamepad, "LB"), 0));
        assert_eq!(s.compose().analog[4], 0);
        assert!(!s.apply(2, 9, 0, 0));
    }
}
