// SPDX-License-Identifier: GPL-3.0-or-later
//! Bevy keyboard, gamepad and window events into the `tsfp_wi_*` queue (`shared::Wi`, `window_input::InputState`).
//!
//! Keyboard: Bevy gives the PHYSICAL key (`KeyCode`, US layout position). It is mapped to the SDL3 keycode the shared
//! `window_input::key_name` table knows, so the names (A..Z, 0..9, arrows, ENTER, BACKSPACE, SPACE, TAB, F1, modifiers) are the SDL
//! implementation's. SDL3 keycodes are layout dependent, so on a non US layout a letter key can name a different letter than
//! under the SDL window (documented limit). Keys without a name are queued with `name=NULL` and a raw code, like the C host.
//!
//! Gamepad: the RAW gamepad events are used (`RawGamepadEvent`, before Bevy's livezone/deadzone filtering), so stick values pass
//! through exactly as the SDL path does (no deadzone, no scaling beyond f32 to int16). Button mapping (SDL3 Xbox layout index ->
//! queue name -> `xinput_host_source.c` effect): South 0 A, East 1 B, West 2 X, North 3 Y, Select 4 BACK, Mode 5 GUIDE (unmapped),
//! Start 6 START, LeftThumb 7 LSTICK, RightThumb 8 RSTICK, LeftTrigger 9 LB (BLACK), RightTrigger 10 RB (WHITE), DPadUp 11,
//! DPadDown 12, DPadLeft 13, DPadRight 14. Analog triggers (LeftTrigger2, RightTrigger2) become axis 4 LT and 5 RT, 0..32767.
//! Sticks: axis 0 LX, 1 LY, 2 RX, 3 RY, value * 32767. Bevy's stick up is positive and SDL's is negative, and the shared state
//! negates axes 1 and 3 (the SDL convention of the contract), so the value fed in for Y is negated first: the XInput value that
//! reaches the pad is `+v * 32767` for stick-up `v`.
use crate::shared::{SHARED, lock};
use crate::window_input::RawEvent;
use bevy::input::ButtonState;
use bevy::input::gamepad::{GamepadAxis, GamepadButton, GamepadConnection, RawGamepadEvent};
use bevy::input::keyboard::{KeyCode, KeyboardFocusLost, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{WindowCloseRequested, WindowFocused};

static UI_KEYS: std::sync::Mutex<std::collections::BTreeMap<KeyCode, Option<u8>>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

pub use tsfp_input_intake::sdl::{SDLK_UNMAPPED, sdl_button, sdl_keycode, sdl_stick, to_i16}; // T3820

/// SIGINT and SIGTERM become the same close request as the SDL window's "SDL quit event (SIGINT, SIGTERM or session end)":
/// the guest stops cleanly with its stop report. SDL's own handler (installed by SDL_Init for the live renderer's surface) is
/// turned off in main (SDL_NO_SIGNAL_HANDLERS) because nobody pumps SDL events here.
static SIGNALLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

unsafe extern "C" {
    fn signal(signal: i32, handler: usize) -> usize;
}

extern "C" fn on_signal(_signal: i32) {
    SIGNALLED.store(true, std::sync::atomic::Ordering::Release);
}

pub fn install_signal_handlers() {
    // SAFETY: `on_signal` only stores to an atomic (async-signal-safe); 2 is SIGINT and 15 SIGTERM on Linux.
    unsafe {
        signal(2, on_signal as extern "C" fn(i32) as usize);
        signal(15, on_signal as extern "C" fn(i32) as usize);
    }
}

pub fn signal_events() {
    if SIGNALLED.swap(false, std::sync::atomic::Ordering::AcqRel) {
        lock(&SHARED.wi).feed(RawEvent::Quit);
    }
}

pub fn keyboard_events(
    mut keys: EventReader<KeyboardInput>,
    mut focus_lost: EventReader<KeyboardFocusLost>,
    mut focused: EventReader<WindowFocused>,
    mut close: EventReader<WindowCloseRequested>,
    mut router: Option<ResMut<crate::input_router::RouterRes>>,
) {
    signal_events();
    let mut wi = lock(&SHARED.wi);
    for key in keys.read() {
        if let Some(route) = route_key(key, &mut wi, router.as_deref_mut()) {
            tsfp_input_intake::route(tsfp_input_intake::RouteKind::Key, route); // T3820: recorded, checked on replay
        }
    }
    let lost = focus_lost.read().count() != 0;
    if focused
        .read()
        .fold(lost, |lost, event| lost || !event.focused)
    {
        for (_, button) in std::mem::take(&mut *lock(&UI_KEYS)) {
            if let Some(button) = button {
                wi.feed(RawEvent::button_up(None, button));
            }
        }
        wi.feed(RawEvent::FocusLost);
        if let Some(router) = router.as_mut() {
            router.focus_lost();
        }
    }
    if close.read().count() != 0 {
        wi.feed(RawEvent::CloseRequested);
    }
}

/// The guest key-queue event of a key.
pub fn raw_key(key: &KeyboardInput) -> RawEvent {
    let (key_code, repeat) = (sdl_keycode(key.key_code), key.repeat);
    match key.state {
        ButtonState::Pressed => RawEvent::KeyDown {
            key: key_code,
            repeat,
        },
        ButtonState::Released => RawEvent::KeyUp {
            key: key_code,
            repeat,
        },
    }
}

/// One key through the consumers in priority order; the name of the one that took it (T3820), `None` for the
/// recorder's own hotkey and for semantic (format v5 and older) replays.
fn route_key(
    key: &KeyboardInput,
    wi: &mut crate::shared::Wi,
    router: Option<&mut crate::input_router::RouterRes>,
) -> Option<&'static str> {
    if key.key_code == tsfp_input_intake::MARK_HOTKEY && crate::input_recording::enabled() {
        if key.state == ButtonState::Pressed && !key.repeat {
            crate::input_recording::mark_hotkey(key.key_code);
        }
        return None;
    }
    if crate::input_recording::semantic_replay() {
        wi.feed(raw_key(key)); // Keep the host's Alt+F4 close chord available, but do not mix live keys into replay.
        return None;
    }
    // T2169: keys typed into a live text page never reach the guest's key mapping or the router.
    if crate::storage_live::native_mapmaker_text_keyboard(key) {
        return Some("mapmaker-text");
    } else if crate::storage_live::name_keyboard(key) {
        return Some("storage-name");
    } else if crate::text_entry_live::swallow(key) {
        return Some("text-entry");
    }
    let code = sdl_keycode(key.key_code);
    // Keep the route chosen at key-down until key-up, even if selection changed pages.
    // TEMPORARY-SHIM fallback pad events: removal T2308; native pages use Rust intents.
    let mut held = lock(&UI_KEYS);
    if key.state == ButtonState::Released {
        if let Some(button) = held.remove(&key.key_code) {
            if let Some(button) = button {
                wi.feed(RawEvent::button_up(None, button));
            }
            return Some("ui-held");
        }
    } else if key.repeat || held.contains_key(&key.key_code) {
        return Some("ui-held"); // a gameplay press may have opened this page: do not take its repeat or release
    } else if crate::ui_live::owns_key(key.key_code) {
        held.insert(key.key_code, None);
        return Some("ui-live");
    } else if crate::ui_overlay::top_live_page().is_some()
        && (code == 32 || tsfp_ui_input::keyboard::menu_intent(code).is_some())
    {
        let button = tsfp_ui_input::keyboard::fallback_button_on_page(
            code,
            crate::ui_overlay::top_live_page().map_or(0, |p| p.builder),
        );
        held.insert(key.key_code, button);
        if let Some(button) = button {
            wi.feed(RawEvent::button_down(None, button));
        }
        return Some("ui-fallback");
    }
    drop(held);
    // Outside UI pages, Escape is the pad START (pause toggle), also while captured. Only the
    // Alt+F4 chord (window_input) ends the run from the keyboard, so nothing is skipped here.
    if let Some(router) = router {
        router.key(key);
    }
    wi.feed(raw_key(key));
    Some("router")
}

static UI_MOUSE: std::sync::Mutex<tsfp_ui_input::keyboard::MouseFallback> =
    std::sync::Mutex::new(tsfp_ui_input::keyboard::MouseFallback::new());

pub fn ui_mouse_fallback(button: MouseButton, down: bool) -> bool {
    let original = crate::ui_overlay::top_live_page().is_some() && !crate::ui_live::owns_pointer();
    let usable = original && matches!(button, MouseButton::Left | MouseButton::Right);
    let pad = lock(&UI_MOUSE).step(u8::from(button == MouseButton::Right), down, usable);
    let edge = |pad| {
        if down {
            RawEvent::button_down(None, pad)
        } else {
            RawEvent::button_up(None, pad)
        }
    };
    pad.map(|pad| lock(&SHARED.wi).feed(edge(pad))).is_some()
}

pub fn gamepad_events(mut raw: EventReader<RawGamepadEvent>) {
    let mut wi = lock(&SHARED.wi);
    for event in raw.read() {
        apply_gamepad_event(&mut wi, event);
    }
}

/// One raw gamepad event into the queue (the path used when `--input-router` is off, and the reference of its parity test).
pub fn apply_gamepad_event(wi: &mut crate::shared::Wi, event: &RawGamepadEvent) {
    match event {
        RawGamepadEvent::Connection(connection) => {
            let id = connection.gamepad.index();
            match connection.connection {
                GamepadConnection::Connected { .. } => {
                    wi.pads.insert(id);
                    wi.feed(RawEvent::GamepadAdded(id));
                }
                GamepadConnection::Disconnected => {
                    wi.pads.remove(&id);
                    wi.held_buttons.retain(|(pad, _)| *pad != id);
                    wi.last_axis.retain(|(pad, _), _| *pad != id);
                    wi.feed(RawEvent::GamepadRemoved(id));
                }
            }
        }
        // T2188: the Rust main menu owns the pad (it feeds dispatched taps instead); releases still pass so nothing sticks.
        RawGamepadEvent::Button(change)
            if crate::ui_live::holds_pad_button(change.button) && change.value > 0.5 => {}
        RawGamepadEvent::Button(change) => {
            let id = change.gamepad.index();
            if let Some(button) = sdl_button(change.button) {
                let down = change.value > 0.5;
                let held = wi.held_buttons.contains(&(id, button));
                if down && !held {
                    wi.held_buttons.insert((id, button));
                    wi.feed(RawEvent::button_down(Some(id), button));
                } else if !down && held {
                    wi.held_buttons.remove(&(id, button));
                    wi.feed(RawEvent::button_up(Some(id), button));
                }
            } else if let Some(axis) = match change.button {
                GamepadButton::LeftTrigger2 => Some(4u8),
                GamepadButton::RightTrigger2 => Some(5u8),
                _ => None,
            } {
                let value = to_i16(change.value.max(0.0));
                if wi.last_axis.insert((id, axis), value) != Some(value) {
                    wi.feed(RawEvent::axis(Some(id), axis, value));
                }
            }
        }
        RawGamepadEvent::Axis(change)
            if crate::ui_live::capturing() && change.value.abs() > 0.3 => {}
        RawGamepadEvent::Axis(change) => {
            let id = change.gamepad.index();
            if let Some((axis, sign)) = sdl_stick(change.axis) {
                let value = to_i16(change.value * sign);
                if wi.last_axis.insert((id, axis), value) != Some(value) {
                    wi.feed(RawEvent::axis(Some(id), axis, value));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window_input::{Kind, key_name};

    #[test]
    fn named_keys_agree_with_the_sdl_table() {
        for (key, name) in [
            (KeyCode::KeyA, "A"),
            (KeyCode::KeyZ, "Z"),
            (KeyCode::Digit0, "0"),
            (KeyCode::Digit9, "9"),
            (KeyCode::ArrowUp, "UP"),
            (KeyCode::ArrowRight, "RIGHT"),
            (KeyCode::Enter, "ENTER"),
            (KeyCode::NumpadEnter, "ENTER"),
            (KeyCode::Backspace, "BACKSPACE"),
            (KeyCode::Space, "SPACE"),
            (KeyCode::Tab, "TAB"),
            (KeyCode::F1, "F1"),
            (KeyCode::ControlLeft, "LCTRL"),
            (KeyCode::AltRight, "RALT"),
        ] {
            assert_eq!(
                key_name(sdl_keycode(key)).map(|text| text.to_str().unwrap()),
                Some(name),
                "{key:?}"
            );
        }
        assert_eq!(
            key_name(sdl_keycode(KeyCode::Escape)).map(|text| text.to_str().unwrap()),
            Some("ESCAPE")
        );
        assert_eq!(key_name(sdl_keycode(KeyCode::F5)), None);
        assert_eq!(key_name(sdl_keycode(KeyCode::Insert)), None);
    }

    #[test]
    fn buttons_map_to_the_sdl_layout_names() {
        use crate::window_input::button_name;
        let table = [
            (GamepadButton::South, "A"),
            (GamepadButton::East, "B"),
            (GamepadButton::West, "X"),
            (GamepadButton::North, "Y"),
            (GamepadButton::Select, "BACK"),
            (GamepadButton::Mode, "GUIDE"),
            (GamepadButton::Start, "START"),
            (GamepadButton::LeftThumb, "LSTICK"),
            (GamepadButton::RightThumb, "RSTICK"),
            (GamepadButton::LeftTrigger, "LB"),
            (GamepadButton::RightTrigger, "RB"),
            (GamepadButton::DPadUp, "DPAD_UP"),
            (GamepadButton::DPadDown, "DPAD_DOWN"),
            (GamepadButton::DPadLeft, "DPAD_LEFT"),
            (GamepadButton::DPadRight, "DPAD_RIGHT"),
        ];
        for (button, name) in table {
            assert_eq!(
                sdl_button(button)
                    .and_then(button_name)
                    .map(|text| text.to_str().unwrap()),
                Some(name)
            );
        }
        assert_eq!(sdl_button(GamepadButton::LeftTrigger2), None);
    }

    #[test]
    fn stick_up_reaches_the_queue_positive() {
        // Bevy up = +1 -> fed as -32767 (SDL convention) -> the shared state negates axis 1 -> +32767 in the queue.
        let (axis, sign) = sdl_stick(GamepadAxis::LeftStickY).unwrap();
        let mut state = crate::window_input::InputState::new();
        state.gamepad = true;
        let mut result = crate::window_input::PumpResult::default();
        state.handle(RawEvent::axis(None, axis, to_i16(1.0 * sign)), &mut result);
        let event = state.next().unwrap();
        assert_eq!(
            (event.kind, event.name, event.value),
            (Kind::PadAxis, Some(c"LY"), 32767)
        );
        let (axis, sign) = sdl_stick(GamepadAxis::LeftStickX).unwrap();
        state.handle(RawEvent::axis(None, axis, to_i16(-1.0 * sign)), &mut result);
        assert_eq!(state.next().unwrap().value, -32767);
    }

    #[test]
    fn float_conversion_has_no_deadzone() {
        assert_eq!(to_i16(0.0), 0);
        assert_eq!(to_i16(0.01), 328);
        assert_eq!(to_i16(2.0), 32767);
        assert_eq!(to_i16(-2.0), -32767);
    }
}
