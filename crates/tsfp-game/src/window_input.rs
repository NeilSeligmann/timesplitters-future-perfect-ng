// SPDX-License-Identifier: GPL-3.0-or-later
//! T2003 phase 3a: the SDL-free logic of the host window and input module (contract:
//! docs/t-host-window-input-contract.md). It turns raw window/keyboard/gamepad events into the named
//! `present_input_event` queue and the window state flags the C presenter consumes. Everything here is
//! pure (no SDL, no I/O) so it is unit tested with synthetic events; `sdl_window.rs` feeds it real ones.
//!
//! Behaviour is the C `window_input_event`/`window_pump` of src/host/present_sink.c, preserved exactly:
//! key repeat dropped, focus loss queues an UP for every held key (most recently pressed first), the two
//! stick Y axes negated, queue of 256 events with counted drops, the Alt+F4 chord, quit and close request end the run (Escape is the pad START key, T2178).
use std::collections::VecDeque;
use std::ffi::CStr;

pub const QUEUE_SIZE: usize = 256;
pub const HELD_KEYS: usize = 64;
pub const MAX_PADS: usize = 8;

// SDL3 3.2.10 keycodes and codes (verified by compiling against /usr/include/SDL3, docs/t-host-window-input-contract.md).
pub const SDLK_ESCAPE: u32 = 27;
const SDLK_0: u32 = 48;
const SDLK_A: u32 = 97;
#[cfg(test)]
const SDLK_Q: u32 = 113;
/// SDL3 gamepad button index of START (see `button_name`).
const START_BUTTON: u8 = 6;
const SDLK_RETURN: u32 = 13;
const SDLK_BACKSPACE: u32 = 8;
const SDLK_SPACE: u32 = 32;
const SDLK_TAB: u32 = 9;
const SDLK_UP: u32 = 0x4000_0052;
const SDLK_DOWN: u32 = 0x4000_0051;
const SDLK_LEFT: u32 = 0x4000_0050;
const SDLK_RIGHT: u32 = 0x4000_004F;
const SDLK_KP_ENTER: u32 = 0x4000_0058;
const SDLK_F1: u32 = 0x4000_003A;
/// T3812: the quit chord key (Alt+F4); neither key is bound in the game.
const SDLK_F4: u32 = 0x4000_003D;
const SDLK_LCTRL: u32 = 0x4000_00E0;
const SDLK_LSHIFT: u32 = 0x4000_00E1;
const SDLK_LALT: u32 = 0x4000_00E2;
const SDLK_RCTRL: u32 = 0x4000_00E4;
const SDLK_RSHIFT: u32 = 0x4000_00E5;
const SDLK_RALT: u32 = 0x4000_00E6;

const LETTERS: [&CStr; 26] = [
    c"A", c"B", c"C", c"D", c"E", c"F", c"G", c"H", c"I", c"J", c"K", c"L", c"M", c"N", c"O", c"P",
    c"Q", c"R", c"S", c"T", c"U", c"V", c"W", c"X", c"Y", c"Z",
];
const DIGITS: [&CStr; 10] = [c"0", c"1", c"2", c"3", c"4", c"5", c"6", c"7", c"8", c"9"];

/// Table name of a host key, `None` when the host code has no name (the pad source counts it as unknown).
pub fn key_name(key: u32) -> Option<&'static CStr> {
    if (SDLK_A..SDLK_A + 26).contains(&key) {
        return Some(LETTERS[(key - SDLK_A) as usize]);
    }
    if (SDLK_0..SDLK_0 + 10).contains(&key) {
        return Some(DIGITS[(key - SDLK_0) as usize]);
    }
    Some(match key {
        SDLK_UP => c"UP",
        SDLK_DOWN => c"DOWN",
        SDLK_LEFT => c"LEFT",
        SDLK_RIGHT => c"RIGHT",
        SDLK_RETURN | SDLK_KP_ENTER => c"ENTER",
        SDLK_BACKSPACE => c"BACKSPACE",
        SDLK_SPACE => c"SPACE",
        SDLK_TAB => c"TAB",
        SDLK_ESCAPE => c"ESCAPE",
        SDLK_F1 => c"F1",
        // Named for host hotkeys (xinput_hotkey.h), UNMAPPED in the pad table.
        SDLK_LCTRL => c"LCTRL",
        SDLK_RCTRL => c"RCTRL",
        SDLK_LSHIFT => c"LSHIFT",
        SDLK_RSHIFT => c"RSHIFT",
        SDLK_LALT => c"LALT",
        SDLK_RALT => c"RALT",
        _ => return None,
    })
}

/// SDL gamepad button (Xbox layout) to the table name.
pub fn button_name(button: u8) -> Option<&'static CStr> {
    Some(match button {
        0 => c"A",
        1 => c"B",
        2 => c"X",
        3 => c"Y",
        4 => c"BACK",
        5 => c"GUIDE",
        6 => c"START",
        7 => c"LSTICK",
        8 => c"RSTICK",
        9 => c"LB",
        10 => c"RB",
        11 => c"DPAD_UP",
        12 => c"DPAD_DOWN",
        13 => c"DPAD_LEFT",
        14 => c"DPAD_RIGHT",
        _ => return None,
    })
}

/// SDL gamepad axis to (table name, negate). SDL stick up is negative, XInput up is positive (INFERRED).
pub fn axis_name(axis: u8) -> (Option<&'static CStr>, bool) {
    match axis {
        0 => (Some(c"LX"), false),
        1 => (Some(c"LY"), true),
        2 => (Some(c"RX"), false),
        3 => (Some(c"RY"), true),
        4 => (Some(c"LT"), false),
        5 => (Some(c"RT"), false),
        _ => (None, false),
    }
}

/// Negation of a stick value: -32768 has no positive twin and saturates to 32767.
pub fn negate_axis(value: i16) -> i32 {
    if value == i16::MIN {
        i32::from(i16::MAX)
    } else {
        -i32::from(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Kind {
    KeyDown = 1,
    KeyUp,
    PadDown,
    PadUp,
    PadAxis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputEvent {
    pub kind: Kind,
    pub name: Option<&'static CStr>,
    pub raw: i32,
    pub value: i32,
}

/// SDL-free image of the SDL events the presenter pump looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawEvent {
    Quit,
    CloseRequested,
    /// Exposed, resized, pixel size changed, shown, restored.
    Redraw,
    FocusLost,
    KeyDown {
        key: u32,
        repeat: bool,
    },
    KeyUp {
        key: u32,
        repeat: bool,
    },
    GamepadAdded(u32),
    GamepadRemoved(u32),
    ButtonDown {
        pad: Option<u32>,
        button: u8,
    },
    ButtonUp {
        pad: Option<u32>,
        button: u8,
    },
    Axis {
        pad: Option<u32>,
        axis: u8,
        value: i16,
    },
    Other,
}

impl RawEvent {
    pub const fn button_down(pad: Option<u32>, button: u8) -> Self {
        Self::ButtonDown { pad, button }
    }

    pub const fn button_up(pad: Option<u32>, button: u8) -> Self {
        Self::ButtonUp { pad, button }
    }

    pub const fn axis(pad: Option<u32>, axis: u8, value: i16) -> Self {
        Self::Axis { pad, axis, value }
    }
}

/// What the SDL layer must still do after an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Effect {
    pub open_pad: Option<u32>,
    pub close_pad: Option<u32>,
}

/// Result of one pump: the first close cause of the pump and the redraw request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PumpResult {
    pub close_cause: Option<&'static CStr>,
    pub redraw: bool,
}

#[derive(Default)]
pub struct InputState {
    pub keyboard: bool,
    pub gamepad: bool,
    queue: VecDeque<InputEvent>,
    dropped: u64,
    held: Vec<&'static CStr>,
    /// Alt keys currently down (tracked even with the keyboard queue off) for the Alt+F4 quit chord (T3812:
    /// Ctrl+Q collided with crouch and melee).
    alt_down: u8,
    /// T2178: Escape synthesised a pad START press (gamepad source without the keyboard source), for the release on focus loss.
    escape_pad: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, kind: Kind, name: Option<&'static CStr>, raw: i32, value: i32) {
        if self.queue.len() == QUEUE_SIZE {
            self.dropped += 1;
            return;
        }
        self.queue.push_back(InputEvent {
            kind,
            name,
            raw,
            value,
        });
    }

    pub fn next(&mut self) -> Option<InputEvent> {
        self.queue.pop_front()
    }

    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    #[cfg(test)]
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// One event. Close events (and the Alt+F4 chord) set the cause if none was set yet in this pump.
    pub fn handle(&mut self, event: RawEvent, result: &mut PumpResult) -> Effect {
        let mut effect = Effect::default();
        match event {
            RawEvent::Quit => result
                .close_cause
                .get_or_insert(c"SDL quit event (SIGINT, SIGTERM or session end)"),
            RawEvent::CloseRequested => result.close_cause.get_or_insert(c"window close request"),
            RawEvent::KeyDown {
                key: SDLK_F4,
                repeat: false,
            } if self.alt_down != 0 => result.close_cause.get_or_insert(c"Alt+F4 quit chord"),
            RawEvent::Redraw => {
                result.redraw = true;
                return effect;
            }
            RawEvent::KeyDown { key, repeat } | RawEvent::KeyUp { key, repeat } => {
                let down = matches!(event, RawEvent::KeyDown { .. });
                if key == SDLK_LALT || key == SDLK_RALT {
                    let bit = if key == SDLK_LALT { 1 } else { 2 };
                    self.alt_down = if down {
                        self.alt_down | bit
                    } else {
                        self.alt_down & !bit
                    };
                }
                if key == SDLK_ESCAPE && !repeat && self.gamepad && !self.keyboard {
                    // T2178: with only the pad source on, Escape still toggles the pause menu as the pad's START.
                    self.escape_pad = down;
                    self.push(
                        if down { Kind::PadDown } else { Kind::PadUp },
                        button_name(START_BUTTON),
                        i32::from(START_BUTTON),
                        0,
                    );
                }
                if !self.keyboard || repeat {
                    return effect;
                }
                let name = key_name(key);
                if let Some(name) = name {
                    if down {
                        if !self.held.contains(&name) && self.held.len() < HELD_KEYS {
                            self.held.push(name);
                        }
                    } else if let Some(at) = self.held.iter().position(|held| *held == name) {
                        self.held.swap_remove(at);
                    }
                }
                self.push(
                    if down { Kind::KeyDown } else { Kind::KeyUp },
                    name,
                    key as i32,
                    0,
                );
                return effect;
            }
            RawEvent::FocusLost => {
                if self.escape_pad {
                    self.escape_pad = false;
                    self.push(
                        Kind::PadUp,
                        button_name(START_BUTTON),
                        i32::from(START_BUTTON),
                        0,
                    );
                }
                if self.keyboard {
                    while let Some(name) = self.held.pop() {
                        self.push(Kind::KeyUp, Some(name), 0, 0);
                    }
                }
                return effect;
            }
            RawEvent::GamepadAdded(id) => {
                if self.gamepad {
                    effect.open_pad = Some(id);
                }
                return effect;
            }
            RawEvent::GamepadRemoved(id) => {
                if self.gamepad {
                    effect.close_pad = Some(id);
                }
                return effect;
            }
            RawEvent::ButtonDown { button, .. } | RawEvent::ButtonUp { button, .. } => {
                if self.gamepad {
                    let kind = if matches!(event, RawEvent::ButtonDown { .. }) {
                        Kind::PadDown
                    } else {
                        Kind::PadUp
                    };
                    self.push(kind, button_name(button), i32::from(button), 0);
                }
                return effect;
            }
            RawEvent::Axis { axis, value, .. } => {
                if self.gamepad {
                    let (name, invert) = axis_name(axis);
                    let value = if invert {
                        negate_axis(value)
                    } else {
                        i32::from(value)
                    };
                    self.push(Kind::PadAxis, name, i32::from(axis), value);
                }
                return effect;
            }
            RawEvent::Other => return effect,
        };
        effect
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(state: &mut InputState, events: &[RawEvent]) -> PumpResult {
        let mut result = PumpResult::default();
        for event in events {
            state.handle(*event, &mut result);
        }
        result
    }

    fn drain(state: &mut InputState) -> Vec<InputEvent> {
        std::iter::from_fn(|| state.next()).collect()
    }

    fn keyboard_state() -> InputState {
        InputState {
            keyboard: true,
            gamepad: true,
            ..InputState::new()
        }
    }

    #[test]
    fn key_names_match_the_c_table() {
        assert_eq!(key_name(97), Some(c"A"));
        assert_eq!(key_name(122), Some(c"Z"));
        assert_eq!(key_name(48), Some(c"0"));
        assert_eq!(key_name(57), Some(c"9"));
        assert_eq!(key_name(SDLK_UP), Some(c"UP"));
        assert_eq!(key_name(SDLK_RETURN), Some(c"ENTER"));
        assert_eq!(key_name(SDLK_KP_ENTER), Some(c"ENTER"));
        assert_eq!(key_name(SDLK_RALT), Some(c"RALT"));
        assert_eq!(key_name(SDLK_ESCAPE), Some(c"ESCAPE"));
        assert_eq!(key_name(96), None);
        assert_eq!(key_name(123), None);
    }

    #[test]
    fn buttons_and_axes_use_the_xbox_layout() {
        let names: Vec<_> = (0..15)
            .map(|b| button_name(b).unwrap().to_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "A",
                "B",
                "X",
                "Y",
                "BACK",
                "GUIDE",
                "START",
                "LSTICK",
                "RSTICK",
                "LB",
                "RB",
                "DPAD_UP",
                "DPAD_DOWN",
                "DPAD_LEFT",
                "DPAD_RIGHT"
            ]
        );
        assert_eq!(button_name(15), None);
        assert_eq!(axis_name(1), (Some(c"LY"), true));
        assert_eq!(axis_name(3), (Some(c"RY"), true));
        assert_eq!(axis_name(0), (Some(c"LX"), false));
        assert_eq!(axis_name(5), (Some(c"RT"), false));
        assert_eq!(axis_name(6), (None, false));
    }

    #[test]
    fn axis_negation_saturates() {
        assert_eq!(negate_axis(i16::MIN), 32767);
        assert_eq!(negate_axis(i16::MAX), -32767);
        assert_eq!(negate_axis(0), 0);
        assert_eq!(negate_axis(1234), -1234);
    }

    #[test]
    fn stick_y_is_negated_and_triggers_are_not() {
        let mut state = keyboard_state();
        run(
            &mut state,
            &[
                RawEvent::Axis {
                    pad: Some(7),
                    axis: 1,
                    value: -32768,
                },
                RawEvent::Axis {
                    pad: Some(7),
                    axis: 0,
                    value: -32768,
                },
                RawEvent::Axis {
                    pad: Some(7),
                    axis: 4,
                    value: 32767,
                },
            ],
        );
        let events = drain(&mut state);
        assert_eq!((events[0].name, events[0].value), (Some(c"LY"), 32767));
        assert_eq!((events[1].name, events[1].value), (Some(c"LX"), -32768));
        assert_eq!(
            (events[2].name, events[2].value, events[2].raw),
            (Some(c"LT"), 32767, 4)
        );
    }

    #[test]
    fn key_repeat_is_dropped_and_unknown_keys_are_queued_unnamed() {
        let mut state = keyboard_state();
        run(
            &mut state,
            &[
                RawEvent::KeyDown {
                    key: 97,
                    repeat: false,
                },
                RawEvent::KeyDown {
                    key: 97,
                    repeat: true,
                },
                RawEvent::KeyDown {
                    key: 0x4000_0099,
                    repeat: false,
                },
                RawEvent::KeyUp {
                    key: 97,
                    repeat: false,
                },
            ],
        );
        let events = drain(&mut state);
        assert_eq!(events.len(), 3);
        assert_eq!(
            events[0],
            InputEvent {
                kind: Kind::KeyDown,
                name: Some(c"A"),
                raw: 97,
                value: 0
            }
        );
        assert_eq!(events[1].name, None);
        assert_eq!(events[1].raw, 0x4000_0099);
        assert_eq!(events[2].kind, Kind::KeyUp);
    }

    #[test]
    fn focus_loss_releases_held_keys_newest_first() {
        let mut state = keyboard_state();
        run(
            &mut state,
            &[
                RawEvent::KeyDown {
                    key: 97,
                    repeat: false,
                },
                RawEvent::KeyDown {
                    key: 98,
                    repeat: false,
                },
                RawEvent::KeyDown {
                    key: 99,
                    repeat: false,
                },
                RawEvent::KeyUp {
                    key: 98,
                    repeat: false,
                },
            ],
        );
        drain(&mut state);
        run(&mut state, &[RawEvent::FocusLost]);
        // swap_remove of B moved C into B's slot, so C is released before A.
        let released: Vec<_> = drain(&mut state)
            .into_iter()
            .map(|e| (e.kind, e.name, e.raw))
            .collect();
        assert_eq!(
            released,
            [(Kind::KeyUp, Some(c"A"), 0), (Kind::KeyUp, Some(c"C"), 0)]
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
        );
        run(&mut state, &[RawEvent::FocusLost]);
        assert_eq!(state.queued(), 0);
    }

    #[test]
    fn disabled_devices_queue_nothing_but_close_still_works() {
        let mut state = InputState::new();
        let result = run(
            &mut state,
            &[
                RawEvent::KeyDown {
                    key: 97,
                    repeat: false,
                },
                RawEvent::ButtonDown {
                    pad: Some(7),
                    button: 0,
                },
                RawEvent::Axis {
                    pad: Some(7),
                    axis: 0,
                    value: 5,
                },
                RawEvent::FocusLost,
                RawEvent::CloseRequested,
            ],
        );
        assert_eq!(state.queued(), 0);
        assert_eq!(result.close_cause, Some(c"window close request"));
    }

    #[test]
    fn first_close_cause_wins_and_escape_is_the_start_key() {
        let mut state = keyboard_state();
        // T2178: Escape no longer ends the run, it is queued like any named key (the pad source maps ESCAPE to START).
        let result = run(
            &mut state,
            &[
                RawEvent::KeyDown {
                    key: SDLK_ESCAPE,
                    repeat: false,
                },
                RawEvent::KeyUp {
                    key: SDLK_ESCAPE,
                    repeat: false,
                },
            ],
        );
        assert_eq!(result.close_cause, None);
        let events = drain(&mut state);
        assert_eq!(
            events.iter().map(|e| (e.kind, e.name)).collect::<Vec<_>>(),
            [
                (Kind::KeyDown, Some(c"ESCAPE")),
                (Kind::KeyUp, Some(c"ESCAPE"))
            ]
        );
        let result = run(&mut state, &[RawEvent::Quit, RawEvent::CloseRequested]);
        assert_eq!(
            result.close_cause,
            Some(c"SDL quit event (SIGINT, SIGTERM or session end)")
        );
    }

    #[test]
    fn escape_is_pad_start_when_only_the_pad_source_is_on() {
        let mut state = InputState {
            gamepad: true,
            ..InputState::new()
        };
        run(
            &mut state,
            &[
                RawEvent::KeyDown {
                    key: SDLK_ESCAPE,
                    repeat: false,
                },
                RawEvent::KeyDown {
                    key: SDLK_ESCAPE,
                    repeat: true,
                },
                RawEvent::KeyUp {
                    key: SDLK_ESCAPE,
                    repeat: false,
                },
                RawEvent::KeyDown {
                    key: SDLK_ESCAPE,
                    repeat: false,
                },
                RawEvent::FocusLost,
            ],
        );
        let events = drain(&mut state);
        assert_eq!(
            events.iter().map(|e| (e.kind, e.name)).collect::<Vec<_>>(),
            [
                (Kind::PadDown, Some(c"START")),
                (Kind::PadUp, Some(c"START")),
                (Kind::PadDown, Some(c"START")),
                (Kind::PadUp, Some(c"START"))
            ]
        );
    }

    #[test]
    fn alt_f4_is_the_only_keyboard_quit() {
        for keyboard in [true, false] {
            let mut state = InputState {
                keyboard,
                ..InputState::new()
            };
            // F4 alone, Alt alone and Alt released before F4 never quit.
            let result = run(
                &mut state,
                &[
                    RawEvent::KeyDown {
                        key: SDLK_F4,
                        repeat: false,
                    },
                    RawEvent::KeyDown {
                        key: SDLK_LALT,
                        repeat: false,
                    },
                    RawEvent::KeyUp {
                        key: SDLK_LALT,
                        repeat: false,
                    },
                    RawEvent::KeyDown {
                        key: SDLK_F4,
                        repeat: false,
                    },
                ],
            );
            assert_eq!(result.close_cause, None);
            let result = run(
                &mut state,
                &[
                    RawEvent::KeyDown {
                        key: SDLK_RALT,
                        repeat: false,
                    },
                    RawEvent::KeyDown {
                        key: SDLK_F4,
                        repeat: false,
                    },
                ],
            );
            assert_eq!(result.close_cause, Some(c"Alt+F4 quit chord"));
            // T3812: Ctrl+Q (crouch + melee) no longer quits.
            let result = run(
                &mut state,
                &[
                    RawEvent::KeyDown {
                        key: SDLK_LCTRL,
                        repeat: false,
                    },
                    RawEvent::KeyDown {
                        key: SDLK_Q,
                        repeat: false,
                    },
                ],
            );
            assert_eq!(result.close_cause, None);
        }
    }

    #[test]
    fn redraw_events_request_a_redraw_only() {
        let mut state = keyboard_state();
        assert!(run(&mut state, &[RawEvent::Redraw]).redraw);
        assert!(!run(&mut state, &[RawEvent::Other]).redraw);
        assert_eq!(state.queued(), 0);
    }

    #[test]
    fn pad_hotplug_asks_the_sdl_layer_only_when_enabled() {
        let mut state = keyboard_state();
        let mut result = PumpResult::default();
        assert_eq!(
            state
                .handle(RawEvent::GamepadAdded(7), &mut result)
                .open_pad,
            Some(7)
        );
        assert_eq!(
            state
                .handle(RawEvent::GamepadRemoved(7), &mut result)
                .close_pad,
            Some(7)
        );
        state.gamepad = false;
        assert_eq!(
            state.handle(RawEvent::GamepadAdded(7), &mut result),
            Effect::default()
        );
    }

    #[test]
    fn queue_overflow_is_counted_not_silent() {
        let mut state = keyboard_state();
        for _ in 0..QUEUE_SIZE + 5 {
            run(
                &mut state,
                &[RawEvent::ButtonDown {
                    pad: Some(7),
                    button: 0,
                }],
            );
        }
        assert_eq!(state.queued(), QUEUE_SIZE);
        assert_eq!(state.dropped(), 5);
        assert_eq!(drain(&mut state).len(), QUEUE_SIZE);
        run(
            &mut state,
            &[RawEvent::ButtonUp {
                pad: Some(7),
                button: 1,
            }],
        );
        assert_eq!(
            state.next().map(|e| (e.kind, e.name)),
            Some((Kind::PadUp, Some(c"B")))
        );
    }

    #[test]
    fn held_key_table_is_bounded_without_losing_events() {
        let mut state = keyboard_state();
        // 26 letters + 10 digits + 15 named keys is below 64, so every distinct key is tracked.
        for key in 97..123 {
            run(&mut state, &[RawEvent::KeyDown { key, repeat: false }]);
        }
        assert_eq!(state.queued(), 26);
        run(&mut state, &[RawEvent::FocusLost]);
        assert_eq!(state.queued(), 52);
    }
}
