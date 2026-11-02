// SPDX-License-Identifier: GPL-3.0-or-later
//! T3820: host device events and their `D` row payload text (input.rec format v6).
//!
//! One payload is one event, `:` separated, no tab or line break (the row format's rule):
//!
//! | payload | event |
//! |---|---|
//! | `k:<KeyCode>:<logical>:<P\|R>:<repeat 0\|1>:<text>` | keyboard (`logical` is `c<hex utf-8>`, `n<Key variant>` or `?`) |
//! | `b:<button>:<P\|R>` | mouse button (`Left`, `Right`, `Middle`, `Back`, `Forward`, `o<n>`) |
//! | `w:<L\|P>:<x>:<y>` | mouse wheel, line or pixel units |
//! | `m:<dx>:<dy>` | mouse motion, summed over one update |
//! | `c:<x\|->:<y\|->:<width>:<height>` | primary window cursor (logical px, `-` outside) and logical size |
//! | `f:<0\|1>` | window focus changed |
//! | `l` | keyboard focus lost (Bevy releases every held key) |
//! | `pc:<pad>:<name>:<vendor\|->:<product\|->`, `pd:<pad>` | gamepad connected, disconnected |
//! | `pb:<pad>:<button>:<value>`, `pa:<pad>:<axis>:<value>` | raw gamepad button, axis (axes: last value of the update) |
//! | `r:<k\|b>:<route>` | the consumer that took the preceding key (`k`) or mouse button (`b`) |
//!
//! Floats are their IEEE-754 bits in 8 hex digits, text is `-` (none) or `x` + utf-8 hex. Key, button and axis
//! names are the Bevy variant names (read and rebuilt through reflection, so a new variant needs no table).

use bevy_ecs::entity::Entity;
use bevy_input::ButtonState;
use bevy_input::gamepad::{GamepadAxis, GamepadButton};
use bevy_input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey, NativeKeyCode};
use bevy_input::mouse::{MouseButton, MouseScrollUnit};
use bevy_math::Vec2;
use bevy_reflect::{
    DynamicEnum, DynamicVariant, Enum, FromReflect, Typed, VariantInfo, VariantType,
};

/// One host device event as the intake saw it, without the window entity (replay uses its own).
#[derive(Clone, Debug, PartialEq)]
pub enum DeviceEvent {
    Key {
        key: KeyCode,
        logical: Key,
        pressed: bool,
        repeat: bool,
        text: Option<String>,
    },
    MouseButton {
        button: MouseButton,
        pressed: bool,
    },
    Wheel {
        unit: MouseScrollUnit,
        x: f32,
        y: f32,
    },
    Motion(Vec2),
    /// Cursor position (logical pixels, `None` outside the window) and the window's logical size.
    Cursor {
        position: Option<Vec2>,
        size: Vec2,
    },
    Focus(bool),
    FocusLost,
    PadConnected {
        pad: u32,
        name: String,
        vendor: Option<u16>,
        product: Option<u16>,
    },
    PadDisconnected(u32),
    PadButton {
        pad: u32,
        button: GamepadButton,
        value: f32,
    },
    PadAxis {
        pad: u32,
        axis: GamepadAxis,
        value: f32,
    },
    /// The routing decision for the preceding key or mouse button (see [`RouteKind`]).
    Route(RouteKind, String),
    /// A live-state input decision taking `value` from here on (raw-look guard, slot sources, pointer capture):
    /// replayed verbatim instead of decided again from replay-time state.
    Decision(String, String),
}

impl DeviceEvent {
    /// The recorded form of a Bevy keyboard event.
    pub fn key(event: &KeyboardInput) -> Self {
        Self::Key {
            key: event.key_code,
            logical: event.logical_key.clone(),
            pressed: event.state.is_pressed(),
            repeat: event.repeat,
            text: event.text.as_ref().map(|text| text.to_string()),
        }
    }

    /// The Bevy keyboard event of a recorded key, for `window`.
    pub fn keyboard_input(&self, window: Entity) -> Option<KeyboardInput> {
        let Self::Key {
            key,
            logical,
            pressed,
            repeat,
            text,
        } = self
        else {
            return None;
        };
        Some(KeyboardInput {
            key_code: *key,
            logical_key: logical.clone(),
            state: button_state(*pressed),
            text: text.as_deref().map(Into::into),
            repeat: *repeat,
            window,
        })
    }
}

/// Which device a recorded routing decision belongs to; each kind is checked in its own order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RouteKind {
    Key,
    MouseButton,
}

impl RouteKind {
    fn tag(self) -> &'static str {
        match self {
            Self::Key => "k",
            Self::MouseButton => "b",
        }
    }
}

pub fn encode(event: &DeviceEvent) -> String {
    match event {
        DeviceEvent::Key {
            key,
            logical,
            pressed,
            repeat,
            text,
        } => format!(
            "k:{}:{}:{}:{}:{}",
            key_code_name(key),
            logical_name(logical),
            state(*pressed),
            u8::from(*repeat),
            text_token(text.as_deref())
        ),
        DeviceEvent::MouseButton { button, pressed } => {
            format!("b:{}:{}", mouse_name(button), state(*pressed))
        }
        DeviceEvent::Wheel { unit, x, y } => format!(
            "w:{}:{}:{}",
            match unit {
                MouseScrollUnit::Line => "L",
                MouseScrollUnit::Pixel => "P",
            },
            bits(*x),
            bits(*y)
        ),
        DeviceEvent::Motion(delta) => format!("m:{}:{}", bits(delta.x), bits(delta.y)),
        DeviceEvent::Cursor { position, size } => {
            let (x, y) = position.map_or(("-".into(), "-".into()), |p| (bits(p.x), bits(p.y)));
            format!("c:{x}:{y}:{}:{}", bits(size.x), bits(size.y))
        }
        DeviceEvent::Focus(focused) => format!("f:{}", u8::from(*focused)),
        DeviceEvent::FocusLost => "l".into(),
        DeviceEvent::PadConnected {
            pad,
            name,
            vendor,
            product,
        } => format!(
            "pc:{pad}:{}:{}:{}",
            text_token(Some(name)),
            id_token(*vendor),
            id_token(*product)
        ),
        DeviceEvent::PadDisconnected(pad) => format!("pd:{pad}"),
        DeviceEvent::PadButton { pad, button, value } => {
            format!("pb:{pad}:{}:{}", other_name(button), bits(*value))
        }
        DeviceEvent::PadAxis { pad, axis, value } => {
            format!("pa:{pad}:{}:{}", other_name(axis), bits(*value))
        }
        DeviceEvent::Route(kind, route) => format!("r:{}:{route}", kind.tag()),
        DeviceEvent::Decision(name, value) => format!("x:{name}:{value}"),
    }
}

pub fn decode(payload: &str) -> Result<DeviceEvent, String> {
    let fields: Vec<&str> = payload.split(':').collect();
    let bad = || format!("invalid device row payload {payload:?}");
    let event = match fields.as_slice() {
        ["k", key, logical, pressed, repeat, text] => DeviceEvent::Key {
            key: if *key == "?" {
                KeyCode::Unidentified(NativeKeyCode::Unidentified)
            } else {
                unit(key).ok_or_else(bad)?
            },
            logical: decode_logical(logical).ok_or_else(bad)?,
            pressed: decode_state(pressed).ok_or_else(bad)?,
            repeat: decode_flag(repeat).ok_or_else(bad)?,
            text: decode_text(text).ok_or_else(bad)?,
        },
        ["b", button, pressed] => DeviceEvent::MouseButton {
            button: decode_mouse(button).ok_or_else(bad)?,
            pressed: decode_state(pressed).ok_or_else(bad)?,
        },
        ["w", unit, x, y] => DeviceEvent::Wheel {
            unit: match *unit {
                "L" => MouseScrollUnit::Line,
                "P" => MouseScrollUnit::Pixel,
                _ => return Err(bad()),
            },
            x: float(x).ok_or_else(bad)?,
            y: float(y).ok_or_else(bad)?,
        },
        ["m", x, y] => DeviceEvent::Motion(Vec2::new(
            float(x).ok_or_else(bad)?,
            float(y).ok_or_else(bad)?,
        )),
        ["c", x, y, width, height] => DeviceEvent::Cursor {
            position: match (*x, *y) {
                ("-", "-") => None,
                (x, y) => Some(Vec2::new(
                    float(x).ok_or_else(bad)?,
                    float(y).ok_or_else(bad)?,
                )),
            },
            size: Vec2::new(
                float(width).ok_or_else(bad)?,
                float(height).ok_or_else(bad)?,
            ),
        },
        ["f", focused] => DeviceEvent::Focus(decode_flag(focused).ok_or_else(bad)?),
        ["l"] => DeviceEvent::FocusLost,
        ["pc", pad, name, vendor, product] => DeviceEvent::PadConnected {
            pad: pad.parse().map_err(|_| bad())?,
            name: decode_text(name).flatten().ok_or_else(bad)?,
            vendor: decode_id(vendor).ok_or_else(bad)?,
            product: decode_id(product).ok_or_else(bad)?,
        },
        ["pd", pad] => DeviceEvent::PadDisconnected(pad.parse().map_err(|_| bad())?),
        ["pb", pad, button, value] => DeviceEvent::PadButton {
            pad: pad.parse().map_err(|_| bad())?,
            button: decode_other(button, GamepadButton::Other).ok_or_else(bad)?,
            value: float(value).ok_or_else(bad)?,
        },
        ["pa", pad, axis, value] => DeviceEvent::PadAxis {
            pad: pad.parse().map_err(|_| bad())?,
            axis: decode_other(axis, GamepadAxis::Other).ok_or_else(bad)?,
            value: float(value).ok_or_else(bad)?,
        },
        ["r", kind, route] if is_route_name(route) => DeviceEvent::Route(
            match *kind {
                "k" => RouteKind::Key,
                "b" => RouteKind::MouseButton,
                _ => return Err(bad()),
            },
            (*route).to_owned(),
        ),
        ["x", name, value] if is_route_name(name) && is_decision_value(value) => {
            DeviceEvent::Decision((*name).to_owned(), (*value).to_owned())
        }
        _ => return Err(bad()),
    };
    Ok(event)
}

/// Decision values: non-empty ASCII letters, digits and `,._-`.
pub fn is_decision_value(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b",._-".contains(&byte))
}

/// Route names are short lowercase identifiers (`text-entry`, `router`, ...).
pub fn is_route_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

pub fn button_state(pressed: bool) -> ButtonState {
    if pressed {
        ButtonState::Pressed
    } else {
        ButtonState::Released
    }
}

fn state(pressed: bool) -> &'static str {
    if pressed { "P" } else { "R" }
}

fn decode_state(text: &str) -> Option<bool> {
    match text {
        "P" => Some(true),
        "R" => Some(false),
        _ => None,
    }
}

fn decode_flag(text: &str) -> Option<bool> {
    match text {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

fn bits(value: f32) -> String {
    format!("{:08x}", value.to_bits())
}

fn float(text: &str) -> Option<f32> {
    (text.len() == 8)
        .then(|| u32::from_str_radix(text, 16).ok().map(f32::from_bits))
        .flatten()
}

fn text_token(text: Option<&str>) -> String {
    text.map_or_else(
        || "-".into(),
        |text| {
            let hex: String = text.bytes().map(|byte| format!("{byte:02x}")).collect();
            format!("x{hex}")
        },
    )
}

/// `-` is `Some(None)`, `x<hex>` is `Some(Some(text))`, anything else `None` (invalid).
fn decode_text(token: &str) -> Option<Option<String>> {
    if token == "-" {
        return Some(None);
    }
    let hex = token.strip_prefix('x')?;
    if hex.len() % 2 != 0 {
        return None;
    }
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    String::from_utf8(bytes).ok().map(Some)
}

fn id_token(id: Option<u16>) -> String {
    id.map_or_else(|| "-".into(), |id| format!("{id:04x}"))
}

fn decode_id(token: &str) -> Option<Option<u16>> {
    if token == "-" {
        return Some(None);
    }
    u16::from_str_radix(token, 16).ok().map(Some)
}

/// The name of a unit variant (reflection), `None` for data-carrying variants.
fn unit_name<T: Enum>(value: &T) -> Option<&str> {
    (value.variant_type() == VariantType::Unit).then(|| value.variant_name())
}

/// The unit variant named `name`, rebuilt through reflection (checked first: `from_reflect` panics on an
/// unknown variant name).
fn unit<T: FromReflect + Typed>(name: &str) -> Option<T> {
    let variant = T::type_info().as_enum().ok()?.variant(name)?;
    matches!(variant, VariantInfo::Unit(_))
        .then(|| T::from_reflect(&DynamicEnum::new(name, DynamicVariant::Unit)))
        .flatten()
}

fn key_code_name(key: &KeyCode) -> String {
    unit_name(key).map_or_else(|| "?".into(), str::to_owned)
}

fn logical_name(key: &Key) -> String {
    match key {
        Key::Character(text) => format!("c{}", &text_token(Some(text))[1..]),
        key => unit_name(key).map_or_else(|| "?".into(), |name| format!("n{name}")),
    }
}

fn decode_logical(token: &str) -> Option<Key> {
    if token == "?" {
        return Some(Key::Unidentified(NativeKey::Unidentified));
    }
    if let Some(hex) = token.strip_prefix('c') {
        let text = decode_text(&format!("x{hex}"))??;
        return (!text.is_empty()).then(|| Key::Character(text.as_str().into()));
    }
    unit(token.strip_prefix('n')?)
}

fn mouse_name(button: &MouseButton) -> String {
    match button {
        MouseButton::Other(code) => format!("o{code}"),
        button => unit_name(button).unwrap_or("?").to_owned(),
    }
}

fn decode_mouse(token: &str) -> Option<MouseButton> {
    match token.strip_prefix('o') {
        Some(code) => code.parse().ok().map(MouseButton::Other),
        None => unit(token),
    }
}

/// Gamepad buttons and axes: unit variant name, or `o<n>` for `Other(n)`.
fn other_name<T: Enum>(value: &T) -> String {
    match unit_name(value) {
        Some(name) => name.to_owned(),
        None => {
            let code = value
                .field_at(0)
                .and_then(|field| field.try_downcast_ref::<u8>())
                .copied()
                .unwrap_or(0);
            format!("o{code}")
        }
    }
}

fn decode_other<T: FromReflect + Typed>(token: &str, other: fn(u8) -> T) -> Option<T> {
    match token.strip_prefix('o') {
        Some(code) => code.parse().ok().map(other),
        None => unit(token),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples() -> Vec<DeviceEvent> {
        vec![
            DeviceEvent::Key {
                key: KeyCode::KeyA,
                logical: Key::Character("é".into()),
                pressed: true,
                repeat: false,
                text: Some("é".into()),
            },
            DeviceEvent::Key {
                key: KeyCode::Backspace,
                logical: Key::Backspace,
                pressed: false,
                repeat: true,
                text: None,
            },
            DeviceEvent::Key {
                key: KeyCode::Unidentified(NativeKeyCode::Unidentified),
                logical: Key::Unidentified(NativeKey::Unidentified),
                pressed: true,
                repeat: false,
                text: Some(String::new()),
            },
            DeviceEvent::MouseButton {
                button: MouseButton::Right,
                pressed: true,
            },
            DeviceEvent::MouseButton {
                button: MouseButton::Other(9),
                pressed: false,
            },
            DeviceEvent::Wheel {
                unit: MouseScrollUnit::Pixel,
                x: 0.0,
                y: -40.5,
            },
            DeviceEvent::Motion(Vec2::new(1.25, -3.0)),
            DeviceEvent::Cursor {
                position: Some(Vec2::new(320.5, 17.0)),
                size: Vec2::new(1280.0, 720.0),
            },
            DeviceEvent::Cursor {
                position: None,
                size: Vec2::new(1280.0, 720.0),
            },
            DeviceEvent::Focus(false),
            DeviceEvent::FocusLost,
            DeviceEvent::PadConnected {
                pad: 3,
                name: "Xbox: Wireless".into(),
                vendor: Some(0x045e),
                product: None,
            },
            DeviceEvent::PadDisconnected(3),
            DeviceEvent::PadButton {
                pad: 3,
                button: GamepadButton::DPadUp,
                value: 1.0,
            },
            DeviceEvent::PadButton {
                pad: 3,
                button: GamepadButton::Other(17),
                value: 0.5,
            },
            DeviceEvent::PadAxis {
                pad: 3,
                axis: GamepadAxis::LeftStickY,
                value: -0.75,
            },
            DeviceEvent::PadAxis {
                pad: 3,
                axis: GamepadAxis::Other(4),
                value: 0.25,
            },
            DeviceEvent::Route(RouteKind::Key, "text-entry".into()),
            DeviceEvent::Route(RouteKind::MouseButton, "ui-fallback".into()),
            DeviceEvent::Decision("raw-look".into(), "1".into()),
            DeviceEvent::Decision("slot-sources".into(), "mk,p12,-,-".into()),
        ]
    }

    #[test]
    fn every_event_round_trips_through_its_payload() {
        for event in samples() {
            let payload = encode(&event);
            assert!(!payload.contains(['\t', '\n', '\r']), "{payload}");
            assert_eq!(decode(&payload).as_ref(), Ok(&event), "{payload}");
        }
    }

    #[test]
    fn every_key_code_and_named_key_survives_by_name() {
        // Reflection, not a table: every unit variant comes back (a table would drift with Bevy).
        let info = <KeyCode as bevy_reflect::Typed>::type_info()
            .as_enum()
            .unwrap();
        let mut count = 0;
        for name in info.variant_names() {
            let Some(key) = unit::<KeyCode>(name) else {
                continue;
            };
            count += 1;
            assert_eq!(key_code_name(&key), *name);
        }
        assert!(count > 150, "{count} unit key codes");
        assert_eq!(decode_logical("nEnter"), Some(Key::Enter));
        assert_eq!(decode_logical("nNoSuchKey"), None);
    }

    #[test]
    fn malformed_payloads_are_rejected() {
        for bad in [
            "",
            "k:KeyA:nEnter:P:0",
            "k:NoSuchKey:nEnter:P:0:-",
            "k:KeyA:nEnter:X:0:-",
            "k:KeyA:c:P:0:-",
            "b:Left:P:1",
            "w:Q:00000000:00000000",
            "m:1:2",
            "c:-:00000000:00000000:00000000",
            "pc:1:-:-:-",
            "pb:x:South:00000000",
            "r:k:Text Entry",
            "r:z:router",
            "x:raw-look:",
            "x:Raw:1",
            "x:raw-look:a:b",
            "x:raw-look:a b",
            "z",
        ] {
            assert!(decode(bad).is_err(), "{bad:?}");
        }
    }
}
