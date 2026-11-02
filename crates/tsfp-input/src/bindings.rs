// SPDX-License-Identifier: GPL-3.0-or-later
//! Keyboard and mouse button bindings to the virtual pad, remappable through a small TOML subset:
//!
//! ```toml
//! [look]            # optional
//! sensitivity_yaw = 0.022      # degrees per raw count
//! sensitivity_pitch = 0.022
//! invert_pitch = false
//! [keys]            # physical key names (Bevy KeyCode Debug names: KeyW, Space, ShiftLeft, Escape, Enter, Digit1, ArrowUp ...)
//! KeyW = "move_forward"
//! KeyG = "none"     # "none" unbinds a default
//! [mouse]           # Left Right Middle Back Forward WheelUp WheelDown
//! Left = "rt"
//! ```
//! Actions: move_forward move_back move_left move_right walk a b x y start back l3 r3 black white lt rt
//! dpad_up dpad_down dpad_left dpad_right slot1..slot9 none. `slotN` is not a pad button: it asks the owning slot to select
//! weapon entry N directly (a request mailbox, see `Router::take_weapon_requests`). F10, Ctrl+Alt and the pointer release chord are reserved by the capture.
use crate::accum::Sensitivity;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    MoveForward,
    MoveBack,
    MoveLeft,
    MoveRight,
    Walk,
    A,
    B,
    X,
    Y,
    Start,
    Back,
    L3,
    R3,
    Black,
    White,
    Lt,
    Rt,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    /// Direct weapon slot request 1..=9 (no pad bit; the pad can only cycle).
    Slot(u8),
    None,
}

impl Action {
    /// Xbox/SDL button index for actions that can be targeted by a controller button binding.
    pub fn controller_button_index(self) -> Option<u8> {
        Some(match self {
            Self::A => 0,
            Self::B => 1,
            Self::X => 2,
            Self::Y => 3,
            Self::Back => 4,
            Self::Start => 6,
            Self::L3 => 7,
            Self::R3 => 8,
            Self::Black => 9,
            Self::White => 10,
            Self::DpadUp => 11,
            Self::DpadDown => 12,
            Self::DpadLeft => 13,
            Self::DpadRight => 14,
            _ => return None,
        })
    }

    pub fn parse(name: &str) -> Option<Action> {
        use Action::*;
        Some(match name {
            "move_forward" => MoveForward,
            "move_back" => MoveBack,
            "move_left" => MoveLeft,
            "move_right" => MoveRight,
            "walk" => Walk,
            "a" => A,
            "b" => B,
            "x" => X,
            "y" => Y,
            "start" => Start,
            "back" => Back,
            "l3" => L3,
            "r3" => R3,
            "black" => Black,
            "white" => White,
            "lt" => Lt,
            "rt" => Rt,
            "dpad_up" => DpadUp,
            "dpad_down" => DpadDown,
            "dpad_left" => DpadLeft,
            "dpad_right" => DpadRight,
            "none" => None,
            _ => {
                let number: u8 = name
                    .strip_prefix("slot")?
                    .parse()
                    .ok()
                    .filter(|n| (1..=9).contains(n))?;
                Slot(number)
            }
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bindings {
    pub keys: HashMap<String, Action>,
    pub mouse: HashMap<String, Action>,
    /// Logical controller button names such as `a`, `left_shoulder`, or `dpad_up`.
    pub controller: HashMap<String, Action>,
    pub sensitivity: Sensitivity,
}

impl Default for Bindings {
    fn default() -> Self {
        use Action::*;
        // Default keymap (T2179), decoupled from the pad where the pad shares a button. Pad meanings are MEASURED in
        // docs/t-button-dump-analysis.md section 3 (RT fire, X reload, B melee, LT grenade, R3 aim, D-pad right/left cycle,
        // D-pad up alt toggle or zoom while aiming, D-pad down grenade type); A jump, L3 crouch and Y use are INFERRED
        // (docs/t-kbm-keymap.md). WHITE and BLACK did nothing in the owner's sessions, so no default drives them.
        let mut keys: Vec<(String, Action)> = [
            ("KeyW", MoveForward),
            ("KeyS", MoveBack),
            ("KeyA", MoveLeft),
            ("KeyD", MoveRight),
            ("ShiftLeft", Walk),
            ("Space", A),
            ("KeyE", Y),
            ("KeyR", X),
            ("KeyQ", DpadLeft),
            ("KeyF", B),
            ("KeyG", Lt),
            ("KeyT", DpadDown),
            ("KeyC", L3),
            ("ControlLeft", L3),
            ("Enter", Start),
            ("Escape", Start),
            ("Tab", Back),
            ("Backspace", B),
            ("ArrowUp", DpadUp),
            ("ArrowDown", DpadDown),
            ("ArrowLeft", DpadLeft),
            ("ArrowRight", DpadRight),
        ]
        .into_iter()
        .map(|(k, a)| (k.to_string(), a))
        .collect();
        keys.extend((1..=9u8).map(|n| (format!("Digit{n}"), Slot(n))));
        let mouse = [
            ("Left", Rt),
            ("Right", R3),
            ("Middle", DpadUp),
            ("WheelUp", DpadRight),
            ("WheelDown", DpadLeft),
        ];
        // SDL3 logical gamepad controls map one-to-one to their original Xbox report by default.
        // These named rows make controller rebinding visible and editable in native settings.
        let controller = [
            ("south", A),
            ("east", B),
            ("west", X),
            ("north", Y),
            ("select", Back),
            ("mode", None),
            ("start", Start),
            ("left_thumb", L3),
            ("right_thumb", R3),
            ("left_shoulder", Black),
            ("right_shoulder", White),
            ("dpad_up", DpadUp),
            ("dpad_down", DpadDown),
            ("dpad_left", DpadLeft),
            ("dpad_right", DpadRight),
        ];
        Self {
            keys: keys.into_iter().collect(),
            mouse: mouse.into_iter().map(|(k, a)| (k.to_string(), a)).collect(),
            controller: controller
                .into_iter()
                .map(|(input, action)| (input.to_string(), action))
                .collect(),
            sensitivity: Sensitivity::default(),
        }
    }
}

impl Bindings {
    /// Defaults overridden by the file text. Errors name the line.
    pub fn parse(text: &str) -> Result<Bindings, String> {
        let mut out = Bindings::default();
        let mut section = String::new();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let fail = |why: &str| format!("line {}: {why}: {raw:?}", index + 1);
            if let Some(name) = line
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                section = name.trim().to_string();
                if !matches!(section.as_str(), "keys" | "mouse" | "controller" | "look") {
                    return Err(fail("unknown section"));
                }
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| fail("expected key = value"))?;
            let (key, value) = (key.trim().trim_matches('"'), value.trim().trim_matches('"'));
            match section.as_str() {
                "keys" | "mouse" | "controller" => {
                    let action = Action::parse(value).ok_or_else(|| fail("unknown action"))?;
                    if section == "controller"
                        && action != Action::None
                        && action.controller_button_index().is_none()
                    {
                        return Err(fail(
                            "controller buttons can only bind to pad button actions",
                        ));
                    }
                    let map = match section.as_str() {
                        "keys" => &mut out.keys,
                        "mouse" => &mut out.mouse,
                        _ => &mut out.controller,
                    };
                    map.insert(key.to_string(), action);
                }
                "look" => {
                    let number = || {
                        value
                            .parse::<f64>()
                            .ok()
                            .filter(|v| v.is_finite() && *v > 0.0 && *v < 10.0)
                    };
                    match key {
                        "sensitivity_yaw" => {
                            out.sensitivity.yaw_deg_per_count =
                                number().ok_or_else(|| fail("bad number"))?
                        }
                        "sensitivity_pitch" => {
                            out.sensitivity.pitch_deg_per_count =
                                number().ok_or_else(|| fail("bad number"))?
                        }
                        "invert_pitch" => {
                            out.sensitivity.invert_pitch =
                                value.parse().map_err(|_| fail("expected true or false"))?
                        }
                        _ => return Err(fail("unknown look key")),
                    }
                }
                _ => return Err(fail("key outside a section")),
            }
        }
        Ok(out)
    }

    /// Read a file. A missing file is an error only when `required` (an explicit path), else the defaults.
    pub fn load(path: &str, required: bool) -> Result<Bindings, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Bindings::parse(&text).map_err(|why| format!("{path}: {why}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !required => {
                Ok(Bindings::default())
            }
            Err(error) => Err(format!("{path}: {error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_wasd_and_mouse_buttons() {
        let bindings = Bindings::default();
        assert_eq!(bindings.keys["KeyW"], Action::MoveForward);
        assert_eq!(bindings.mouse["Left"], Action::Rt);
        assert_eq!(
            bindings.keys["Escape"],
            Action::Start,
            "Escape is the pause toggle (T2178), never a close"
        );
        assert_eq!(
            bindings.keys["Backspace"],
            Action::B,
            "back moved to Backspace"
        );
    }

    #[test]
    fn owner_default_keymap_is_decoupled_from_one_to_one_pad_mapping() {
        let b = Bindings::default();
        assert_eq!(b.keys["KeyR"], Action::X, "reload is X (MEASURED)");
        assert_eq!(
            b.mouse["Right"],
            Action::R3,
            "aim/zoom is R3, not LT (LT is the grenade)"
        );
        assert_eq!(b.mouse["Middle"], Action::DpadUp, "weapon alt mode");
        assert_eq!(
            (b.mouse["WheelUp"], b.mouse["WheelDown"]),
            (Action::DpadRight, Action::DpadLeft)
        );
        assert_eq!(
            (b.keys["KeyF"], b.keys["KeyG"]),
            (Action::B, Action::Lt),
            "melee and grenade are separate keys"
        );
        for n in 1..=9u8 {
            assert_eq!(b.keys[&format!("Digit{n}")], Action::Slot(n));
        }
        assert_eq!(Action::parse("slot9"), Some(Action::Slot(9)));
        assert!(
            Action::parse("slot0").is_none()
                && Action::parse("slot10").is_none()
                && Action::parse("slotx").is_none()
        );
        assert!(
            b.keys
                .values()
                .chain(b.mouse.values())
                .all(|a| !matches!(a, Action::White | Action::Black)),
            "WHITE/BLACK did nothing in the game"
        );
    }

    #[test]
    fn the_in_repo_default_file_equals_the_built_in_defaults() {
        let file = Bindings::parse(include_str!(
            "../../../tools/data/input-bindings.default.toml"
        ))
        .unwrap();
        assert_eq!(file, Bindings::default());
    }

    #[test]
    fn file_overrides_and_unbinds() {
        let text = "# comment\n[keys]\nKeyW = \"move_back\"\nKeyE = none\nKeyZ = \"a\"\n[mouse]\nLeft = \"a\"\n[look]\nsensitivity_yaw = 0.05\ninvert_pitch = true\n";
        let bindings = Bindings::parse(text).unwrap();
        assert_eq!(bindings.keys["KeyW"], Action::MoveBack);
        assert_eq!(bindings.keys["KeyE"], Action::None);
        assert_eq!(bindings.keys["KeyZ"], Action::A);
        assert_eq!(
            bindings.keys["KeyS"],
            Action::MoveBack,
            "untouched defaults stay"
        );
        assert_eq!(bindings.mouse["Left"], Action::A);
        assert_eq!(bindings.sensitivity.yaw_deg_per_count, 0.05);
        assert!(bindings.sensitivity.invert_pitch);
    }

    #[test]
    fn errors_name_the_line() {
        assert!(
            Bindings::parse("[keys]\nKeyW = fly\n")
                .unwrap_err()
                .starts_with("line 2: unknown action")
        );
        assert!(Bindings::parse("[nope]\n").is_err());
        assert!(Bindings::parse("KeyW = a\n").is_err());
        assert!(Bindings::parse("[look]\nsensitivity_yaw = -1\n").is_err());
    }

    #[test]
    fn missing_file_defaults_unless_required() {
        assert_eq!(
            Bindings::load("tmp/does-not-exist.toml", false).unwrap(),
            Bindings::default()
        );
        assert!(Bindings::load("tmp/does-not-exist.toml", true).is_err());
    }
}
