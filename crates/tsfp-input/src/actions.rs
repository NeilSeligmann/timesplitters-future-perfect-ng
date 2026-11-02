// SPDX-License-Identifier: GPL-3.0-or-later
//! T2167 stage 2, the action layer: device inputs to game intents, with multiple bindings per intent, chords, toggles and
//! holds, per player maps and remapping through the TOML file (see `presets::InputConfig`).
//!
//! Binding text: `"mouse:Left, key:ShiftLeft+key:KeyR, toggle:key:KeyC"` (comma separated alternatives, `+` joins a chord,
//! a `toggle:` prefix makes the binding flip the intent on each press). Input names: `key:<Bevy KeyCode name>`,
//! `mouse:<Left Right Middle Back Forward WheelUp WheelDown>`, `pad:<a b x y start back l3 r3 black white lt rt dpad_*>`.
use crate::player::{ButtonBits, SourceKind, Vec2, bits};
use std::collections::HashSet;
use tsfp_state::{
    weapon_interaction::{InteractionInputSource, InteractionIntent},
    weapon_inventory::WeaponPlayerId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Intent {
    MoveForward,
    MoveBack,
    MoveLeft,
    MoveRight,
    LookUp,
    LookDown,
    LookLeft,
    LookRight,
    Fire,
    Aim,
    Reload,
    WeaponNext,
    WeaponPrev,
    /// Alt mode of the weapon (pad D-pad up: primary or x2 entry to its left hand companion, zoom step while aiming).
    WeaponAlt,
    Melee,
    Grenade,
    /// Cycles the selected throwable (pad D-pad down).
    GrenadeType,
    /// Direct select of weapon entry 1..=9 (no pad bit, request mailbox, see `Router::take_weapon_requests`).
    WeaponSlot(u8),
    Jump,
    Crouch,
    Use,
    Walk,
    Pause,
    MenuAccept,
    MenuBack,
    MenuUp,
    MenuDown,
    MenuLeft,
    MenuRight,
}

pub const INTENTS: [(Intent, &str); 37] = [
    (Intent::MoveForward, "move_forward"),
    (Intent::MoveBack, "move_back"),
    (Intent::MoveLeft, "move_left"),
    (Intent::MoveRight, "move_right"),
    (Intent::LookUp, "look_up"),
    (Intent::LookDown, "look_down"),
    (Intent::LookLeft, "look_left"),
    (Intent::LookRight, "look_right"),
    (Intent::Fire, "fire"),
    (Intent::Aim, "aim"),
    (Intent::Reload, "reload"),
    (Intent::WeaponNext, "weapon_next"),
    (Intent::WeaponPrev, "weapon_prev"),
    (Intent::WeaponAlt, "weapon_alt"),
    (Intent::Melee, "melee"),
    (Intent::Grenade, "grenade"),
    (Intent::GrenadeType, "grenade_type"),
    (Intent::WeaponSlot(1), "weapon_slot_1"),
    (Intent::WeaponSlot(2), "weapon_slot_2"),
    (Intent::WeaponSlot(3), "weapon_slot_3"),
    (Intent::WeaponSlot(4), "weapon_slot_4"),
    (Intent::WeaponSlot(5), "weapon_slot_5"),
    (Intent::WeaponSlot(6), "weapon_slot_6"),
    (Intent::WeaponSlot(7), "weapon_slot_7"),
    (Intent::WeaponSlot(8), "weapon_slot_8"),
    (Intent::WeaponSlot(9), "weapon_slot_9"),
    (Intent::Jump, "jump"),
    (Intent::Crouch, "crouch"),
    (Intent::Use, "use"),
    (Intent::Walk, "walk"),
    (Intent::Pause, "pause"),
    (Intent::MenuAccept, "menu_accept"),
    (Intent::MenuBack, "menu_back"),
    (Intent::MenuUp, "menu_up"),
    (Intent::MenuDown, "menu_down"),
    (Intent::MenuLeft, "menu_left"),
    (Intent::MenuRight, "menu_right"),
];

impl Intent {
    pub fn parse(name: &str) -> Option<Intent> {
        INTENTS.iter().find(|(_, n)| *n == name).map(|(i, _)| *i)
    }
    pub fn name(self) -> &'static str {
        INTENTS
            .iter()
            .find(|(i, _)| *i == self)
            .map(|(_, n)| *n)
            .unwrap_or("?")
    }
    fn bit(self) -> u64 {
        1 << INTENTS.iter().position(|(i, _)| *i == self).unwrap_or(0)
    }
    /// The virtual pad button the intent drives in the compatibility shim (none for movement and look).
    pub fn pad_bit(self) -> Option<ButtonBits> {
        use Intent::*;
        Some(match self {
            // Pad meanings: docs/t-button-dump-analysis.md section 3 (MEASURED) and docs/t-kbm-keymap.md (A, L3, Y INFERRED).
            Fire => bits::RT,
            Aim => bits::R3,
            Reload => bits::X,
            Use => bits::Y,
            Melee => bits::B,
            Grenade => bits::LT,
            GrenadeType => bits::DPAD_DOWN,
            Jump => bits::A,
            Crouch => bits::L3,
            WeaponNext => bits::DPAD_RIGHT,
            WeaponPrev => bits::DPAD_LEFT,
            WeaponAlt => bits::DPAD_UP,
            Pause => bits::START,
            MenuAccept => bits::A,
            MenuBack => bits::B,
            MenuUp => bits::DPAD_UP,
            MenuDown => bits::DPAD_DOWN,
            MenuLeft => bits::DPAD_LEFT,
            MenuRight => bits::DPAD_RIGHT,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InputId {
    Key(String),
    Mouse(String),
    Pad(ButtonBits),
}

const PAD_NAMES: [(&str, ButtonBits); 20] = [
    ("a", bits::A),
    ("b", bits::B),
    ("x", bits::X),
    ("y", bits::Y),
    ("start", bits::START),
    ("back", bits::BACK),
    ("l3", bits::L3),
    ("r3", bits::R3),
    ("black", bits::BLACK),
    ("white", bits::WHITE),
    ("lt", bits::LT),
    ("rt", bits::RT),
    ("dpad_up", bits::DPAD_UP),
    ("dpad_down", bits::DPAD_DOWN),
    ("dpad_left", bits::DPAD_LEFT),
    ("dpad_right", bits::DPAD_RIGHT),
    ("lb", bits::BLACK),
    ("rb", bits::WHITE),
    ("select", bits::BACK),
    ("menu", bits::START),
];

impl InputId {
    pub fn parse(text: &str) -> Result<InputId, String> {
        let (kind, name) = text
            .split_once(':')
            .ok_or_else(|| format!("expected kind:name, got {text:?}"))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(format!("empty input name in {text:?}"));
        }
        match kind.trim() {
            "key" => Ok(InputId::Key(name.to_string())),
            "mouse" => Ok(InputId::Mouse(name.to_string())),
            "pad" => PAD_NAMES
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, b)| InputId::Pad(*b))
                .ok_or_else(|| format!("unknown pad button {name:?}")),
            other => Err(format!("unknown input kind {other:?}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    /// All of these must be down; never empty.
    pub chord: Vec<InputId>,
    pub toggle: bool,
}

impl Binding {
    pub fn parse(text: &str) -> Result<Binding, String> {
        let text = text.trim();
        let (toggle, text) = text
            .strip_prefix("toggle:")
            .map(|rest| (true, rest))
            .unwrap_or((false, text));
        let chord: Result<Vec<InputId>, String> = text
            .split('+')
            .map(|part| InputId::parse(part.trim()))
            .collect();
        Ok(Binding {
            chord: chord?,
            toggle,
        })
    }
}

/// Intent to bindings, in insertion order (deterministic).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ActionMap {
    pub bindings: Vec<(Intent, Binding)>,
}

impl ActionMap {
    /// The owner's default keymap (T2179), the intent view of `bindings::Bindings::default()`. Not one to one with the pad:
    /// reload, melee, grenade, alt mode and the weapon slots each have their own input. Extras beyond the owner's list are
    /// listed in `docs/t-kbm-keymap.md`.
    pub fn default_mouse_keyboard() -> ActionMap {
        use Intent::*;
        let mut table: Vec<(Intent, String)> = [
            (MoveForward, "key:KeyW"),
            (MoveBack, "key:KeyS"),
            (MoveLeft, "key:KeyA"),
            (MoveRight, "key:KeyD"),
            (Walk, "key:ShiftLeft"),
            (Fire, "mouse:Left"),
            (Aim, "mouse:Right"),
            (WeaponAlt, "mouse:Middle"),
            (Reload, "key:KeyR"),
            (WeaponNext, "mouse:WheelUp"),
            (WeaponPrev, "mouse:WheelDown"),
            (WeaponPrev, "key:KeyQ"),
            (Pause, "key:Escape"),
            (Pause, "key:Enter"),
            (MenuBack, "key:Backspace"),
            (Jump, "key:Space"),
            (Crouch, "key:KeyC"),
            (Crouch, "key:ControlLeft"),
            (Use, "key:KeyE"),
            (Melee, "key:KeyF"),
            (Grenade, "key:KeyG"),
            (GrenadeType, "key:KeyT"),
            (MenuUp, "key:ArrowUp"),
            (MenuDown, "key:ArrowDown"),
            (MenuLeft, "key:ArrowLeft"),
            (MenuRight, "key:ArrowRight"),
        ]
        .into_iter()
        .map(|(i, b)| (i, b.to_string()))
        .collect();
        table.extend((1..=9u8).map(|n| (WeaponSlot(n), format!("key:Digit{n}"))));
        let bindings = table
            .iter()
            .filter_map(|(i, b)| Binding::parse(b).ok().map(|b| (*i, b)))
            .collect();
        ActionMap { bindings }
    }

    /// Replace all bindings of `intent` with the ones in `text` (`none` or empty clears it).
    pub fn set(&mut self, intent: Intent, text: &str) -> Result<(), String> {
        let mut parsed = Vec::new();
        for alternative in text
            .split(',')
            .map(str::trim)
            .filter(|a| !a.is_empty() && *a != "none")
        {
            parsed.push((intent, Binding::parse(alternative)?));
        }
        self.bindings.retain(|(i, _)| *i != intent);
        self.bindings.extend(parsed);
        Ok(())
    }

    /// Add one more binding for the intent (multiple bindings per action).
    pub fn add(&mut self, intent: Intent, binding: Binding) {
        if !self.bindings.contains(&(intent, binding.clone())) {
            self.bindings.push((intent, binding));
        }
    }
}

/// One tick of evaluated intents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ActionFrame {
    pub held: u64,
    pub pressed: u64,
    pub released: u64,
}

impl ActionFrame {
    pub fn is_held(&self, intent: Intent) -> bool {
        self.held & intent.bit() != 0
    }
    pub fn was_pressed(&self, intent: Intent) -> bool {
        self.pressed & intent.bit() != 0
    }
    pub fn was_released(&self, intent: Intent) -> bool {
        self.released & intent.bit() != 0
    }

    /// Carry the configured native Use action into the weapons interaction seam.
    ///
    /// This preserves input state and source without choosing the original game's use-mask,
    /// hold policy, or target. The caller supplies the player lifetime and step identity.
    pub fn native_interaction_intent(
        &self,
        player: WeaponPlayerId,
        step: u64,
        source: SourceKind,
    ) -> InteractionIntent {
        InteractionIntent {
            player,
            step,
            source: match source {
                SourceKind::Pad(id) => InteractionInputSource::Pad(id),
                SourceKind::MouseKeyboard => InteractionInputSource::MouseKeyboard,
                SourceKind::Script => InteractionInputSource::Script,
                SourceKind::Replay => InteractionInputSource::Replay,
                SourceKind::Network { peer } => InteractionInputSource::Network { peer },
            },
            pressed: self.was_pressed(Intent::Use),
            held: self.is_held(Intent::Use),
            released: self.was_released(Intent::Use),
        }
    }

    fn axis(&self, plus: Intent, minus: Intent) -> f32 {
        self.is_held(plus) as i32 as f32 - self.is_held(minus) as i32 as f32
    }
    /// Digital move vector (x strafe right, y forward), diagonals normalised to unit length.
    pub fn move_axis(&self) -> Vec2 {
        let (x, y) = (
            self.axis(Intent::MoveRight, Intent::MoveLeft),
            self.axis(Intent::MoveForward, Intent::MoveBack),
        );
        let m = x.hypot(y);
        if m > 1.0 {
            Vec2::new(x / m, y / m)
        } else {
            Vec2::new(x, y)
        }
    }
    /// Digital look stick (x right, y up), normalised the same way.
    pub fn look_axis(&self) -> Vec2 {
        let (x, y) = (
            self.axis(Intent::LookRight, Intent::LookLeft),
            self.axis(Intent::LookUp, Intent::LookDown),
        );
        let m = x.hypot(y);
        if m > 1.0 {
            Vec2::new(x / m, y / m)
        } else {
            Vec2::new(x, y)
        }
    }
    /// Held pad buttons the intents drive.
    pub fn pad_buttons(&self) -> ButtonBits {
        INTENTS
            .iter()
            .filter(|(i, _)| self.is_held(*i))
            .filter_map(|(i, _)| i.pad_bit())
            .fold(0, |a, b| a | b)
    }
}

/// Edge and toggle state of one player's map.
#[derive(Clone, Debug, Default)]
pub struct ActionState {
    previous_active: Vec<bool>,
    toggled: u64,
    previous_held: u64,
}

impl ActionState {
    pub fn update(&mut self, map: &ActionMap, down: &HashSet<InputId>) -> ActionFrame {
        let active: Vec<bool> = map
            .bindings
            .iter()
            .map(|(_, b)| b.chord.iter().all(|i| down.contains(i)))
            .collect();
        // A longer active chord consumes the shorter bindings it contains (Shift+R must not also fire plain R).
        let suppressed = |at: usize| {
            let (intent, mine) = &map.bindings[at];
            map.bindings
                .iter()
                .enumerate()
                .any(|(other, (other_intent, theirs))| {
                    active[other]
                        && other_intent != intent
                        && theirs.chord.len() > mine.chord.len()
                        && mine.chord.iter().all(|i| theirs.chord.contains(i))
                })
        };
        self.previous_active.resize(map.bindings.len(), false);
        let mut held = 0u64;
        for (at, (intent, binding)) in map.bindings.iter().enumerate() {
            let live = active[at] && !suppressed(at);
            if binding.toggle {
                if live && !self.previous_active[at] {
                    self.toggled ^= intent.bit();
                }
            } else if live {
                held |= intent.bit();
            }
            self.previous_active[at] = live;
        }
        held |= self.toggled;
        let frame = ActionFrame {
            held,
            pressed: held & !self.previous_held,
            released: self.previous_held & !held,
        };
        self.previous_held = held;
        frame
    }
    pub fn reset(&mut self) {
        *self = ActionState::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn down(items: &[&str]) -> HashSet<InputId> {
        items.iter().map(|i| InputId::parse(i).unwrap()).collect()
    }

    #[test]
    fn defaults_hold_and_edge() {
        let map = ActionMap::default_mouse_keyboard();
        let mut state = ActionState::default();
        let f = state.update(&map, &down(&["key:KeyW", "mouse:Left"]));
        assert!(f.is_held(Intent::MoveForward) && f.was_pressed(Intent::Fire));
        assert_eq!(f.move_axis(), Vec2::new(0.0, 1.0));
        assert_eq!(f.pad_buttons(), bits::RT);
        let f = state.update(&map, &down(&["key:KeyW"]));
        assert!(f.was_released(Intent::Fire) && !f.was_pressed(Intent::MoveForward));
    }

    #[test]
    fn native_interaction_intent_preserves_action_edges_and_source() {
        let use_bit = Intent::Use.bit();
        let frame = ActionFrame {
            held: use_bit,
            pressed: use_bit,
            released: 0,
        };
        assert_eq!(
            frame.native_interaction_intent(
                WeaponPlayerId::new(2),
                41,
                SourceKind::Network { peer: 3 },
            ),
            InteractionIntent {
                player: WeaponPlayerId::new(2),
                step: 41,
                source: InteractionInputSource::Network { peer: 3 },
                pressed: true,
                held: true,
                released: false,
            }
        );

        let released = ActionFrame {
            held: 0,
            pressed: 0,
            released: use_bit,
        };
        let intent = released.native_interaction_intent(
            WeaponPlayerId::new(2),
            42,
            SourceKind::MouseKeyboard,
        );
        assert!(!intent.pressed && !intent.held && intent.released);
        assert_eq!(intent.source, InteractionInputSource::MouseKeyboard);
    }

    #[test]
    fn multiple_bindings_and_remap() {
        let mut map = ActionMap::default_mouse_keyboard();
        map.set(Intent::Fire, "mouse:Left, key:KeyF").unwrap();
        let mut state = ActionState::default();
        assert!(
            state
                .update(&map, &down(&["key:KeyF"]))
                .is_held(Intent::Fire)
        );
        map.set(Intent::Fire, "none").unwrap();
        assert!(
            !state
                .update(&map, &down(&["key:KeyF", "mouse:Left"]))
                .is_held(Intent::Fire)
        );
    }

    #[test]
    fn chord_consumes_the_shorter_binding() {
        let mut map = ActionMap::default_mouse_keyboard();
        map.set(Intent::Melee, "key:ShiftLeft+key:KeyR").unwrap();
        let mut state = ActionState::default();
        let f = state.update(&map, &down(&["key:ShiftLeft", "key:KeyR"]));
        assert!(f.is_held(Intent::Melee) && !f.is_held(Intent::Reload));
        let f = state.update(&map, &down(&["key:KeyR"]));
        assert!(f.is_held(Intent::Reload) && !f.is_held(Intent::Melee));
    }

    #[test]
    fn toggle_flips_on_each_press_and_survives_release() {
        let mut map = ActionMap::default_mouse_keyboard();
        map.set(Intent::Crouch, "toggle:key:KeyC").unwrap();
        let mut state = ActionState::default();
        assert!(
            state
                .update(&map, &down(&["key:KeyC"]))
                .is_held(Intent::Crouch)
        );
        assert!(
            state
                .update(&map, &down(&["key:KeyC"]))
                .is_held(Intent::Crouch),
            "holding does not flip again"
        );
        assert!(state.update(&map, &down(&[])).is_held(Intent::Crouch));
        assert!(
            !state
                .update(&map, &down(&["key:KeyC"]))
                .is_held(Intent::Crouch)
        );
    }

    #[test]
    fn diagonal_is_normalised_and_bad_text_errors() {
        let map = ActionMap::default_mouse_keyboard();
        let f = ActionState::default().update(&map, &down(&["key:KeyW", "key:KeyD"]));
        assert!((f.move_axis().x.hypot(f.move_axis().y) - 1.0).abs() < 1e-6);
        assert!(
            Binding::parse("key").is_err()
                && Binding::parse("pad:zz").is_err()
                && Binding::parse("joy:1").is_err()
        );
    }

    /// Intents that have no default on purpose: stick look (the mouse drives look as deltas) and menu accept (the pad A
    /// bit already follows Jump, so Space accepts in menus).
    const UNBOUND_BY_DESIGN: [Intent; 5] = [
        Intent::LookUp,
        Intent::LookDown,
        Intent::LookLeft,
        Intent::LookRight,
        Intent::MenuAccept,
    ];

    #[test]
    fn every_intent_has_a_default_or_is_documented_unbound() {
        let map = ActionMap::default_mouse_keyboard();
        for (intent, name) in INTENTS {
            let bound = map.bindings.iter().any(|(i, _)| *i == intent);
            assert_eq!(bound, !UNBOUND_BY_DESIGN.contains(&intent), "{name}");
        }
        assert_eq!(
            INTENTS.len(),
            INTENTS
                .iter()
                .map(|(_, n)| *n)
                .collect::<HashSet<_>>()
                .len(),
            "names are unique"
        );
        assert!(INTENTS.len() <= 64, "ActionFrame bits are u64");
    }

    #[test]
    fn no_two_defaults_collide() {
        let map = ActionMap::default_mouse_keyboard();
        let mut owner: std::collections::HashMap<Vec<InputId>, Intent> =
            std::collections::HashMap::new();
        for (intent, binding) in &map.bindings {
            if let Some(other) = owner.insert(binding.chord.clone(), *intent) {
                assert_eq!(
                    other,
                    *intent,
                    "{:?} is bound to {} and {}",
                    binding.chord,
                    other.name(),
                    intent.name()
                );
            }
        }
    }

    #[test]
    fn owner_keymap_drives_the_measured_pad_bits() {
        let map = ActionMap::default_mouse_keyboard();
        for (inputs, intent, pad) in [
            (&["mouse:Left"][..], Intent::Fire, bits::RT),
            (&["mouse:Right"], Intent::Aim, bits::R3),
            (&["mouse:Middle"], Intent::WeaponAlt, bits::DPAD_UP),
            (&["key:KeyR"], Intent::Reload, bits::X),
            (&["mouse:WheelUp"], Intent::WeaponNext, bits::DPAD_RIGHT),
            (&["mouse:WheelDown"], Intent::WeaponPrev, bits::DPAD_LEFT),
            (&["key:KeyQ"], Intent::WeaponPrev, bits::DPAD_LEFT),
            (&["key:KeyF"], Intent::Melee, bits::B),
            (&["key:KeyG"], Intent::Grenade, bits::LT),
            (&["key:Escape"], Intent::Pause, bits::START),
        ] {
            let f = ActionState::default().update(&map, &down(inputs));
            assert!(f.is_held(intent), "{inputs:?}");
            assert_eq!(f.pad_buttons(), pad, "{inputs:?}");
        }
        for n in 1..=9u8 {
            let key = format!("key:Digit{n}");
            let f = ActionState::default().update(&map, &down(&[key.as_str()]));
            assert!(
                f.is_held(Intent::WeaponSlot(n)) && f.pad_buttons() == 0,
                "slot keys have no pad bit"
            );
        }
    }

    #[test]
    fn wheel_notches_are_pressed_edges() {
        let map = ActionMap::default_mouse_keyboard();
        let mut state = ActionState::default();
        assert!(
            state
                .update(&map, &down(&["mouse:WheelUp"]))
                .was_pressed(Intent::WeaponNext)
        );
        assert!(
            !state
                .update(&map, &down(&["mouse:WheelUp"]))
                .was_pressed(Intent::WeaponNext),
            "held is not a second edge"
        );
        assert!(
            state
                .update(&map, &down(&[]))
                .was_released(Intent::WeaponNext)
        );
        assert!(
            state
                .update(&map, &down(&["mouse:WheelDown"]))
                .was_pressed(Intent::WeaponPrev)
        );
    }
}
