// SPDX-License-Identifier: GPL-3.0-or-later
//! `--hotkey DEVICE=KEY+KEY[@HOLDMS]:LABEL`, same decisions and messages as `xinput_hotkey_parse`.
use tsfp_input_devices::hotkey::{Device, KEYS_MAX, NAME_MAX};

pub const LABEL_MAX: usize = 32;
pub const HOLD_MAX_MS: u32 = 10_000;
pub const PAD_DEFAULT_HOLD_MS: u32 = 500;

const PAD_NAMES: [&str; 15] = [
    "A",
    "B",
    "X",
    "Y",
    "LB",
    "RB",
    "START",
    "BACK",
    "GUIDE",
    "LSTICK",
    "RSTICK",
    "DPAD_UP",
    "DPAD_DOWN",
    "DPAD_LEFT",
    "DPAD_RIGHT",
];
const KB_NAMES: [&str; 51] = [
    "A",
    "B",
    "C",
    "D",
    "E",
    "F",
    "G",
    "H",
    "I",
    "J",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "R",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "UP",
    "DOWN",
    "LEFT",
    "RIGHT",
    "ENTER",
    "BACKSPACE",
    "SPACE",
    "TAB",
    "F1",
    "LCTRL",
    "RCTRL",
    "LSHIFT",
    "RSHIFT",
    "LALT",
    "RALT",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub device: Device,
    pub keys: Vec<String>,
    pub hold_ms: u32,
    pub label: String,
}

pub fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// `xinput_hotkey_action_of`: 0 none, 1 mark, 2 stop, 3 shot, 4 mu.
pub fn action_of(label: &str) -> u32 {
    ["mark", "stop", "shot", "mu"]
        .iter()
        .position(|name| same(label, name))
        .map_or(0, |index| index as u32 + 1)
}

/// The names a key element accepts: CTRL is LCTRL or RCTRL (keyboard only). Empty when the name is unknown.
pub fn accept_of(device: Device, name: &str) -> Vec<String> {
    if device == Device::Keyboard {
        for modifier in ["CTRL", "SHIFT", "ALT"] {
            if same(name, modifier) {
                return vec![format!("L{modifier}"), format!("R{modifier}")];
            }
        }
        return KB_NAMES
            .iter()
            .find(|n| same(name, n))
            .map(|n| vec![(*n).to_string()])
            .unwrap_or_default();
    }
    PAD_NAMES
        .iter()
        .find(|n| same(name, n))
        .map(|n| vec![(*n).to_string()])
        .unwrap_or_default()
}

fn parse_hold(text: &str) -> Option<u32> {
    if text.is_empty() || text.len() > 5 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().filter(|v| *v <= HOLD_MAX_MS)
}

pub fn parse(text: &str) -> Result<Spec, String> {
    let Some((device_text, rest)) = text.split_once('=') else {
        return Err(format!("not DEVICE=KEY+KEY[@MS]:LABEL: '{text}'"));
    };
    let device = if same(device_text, "pad") {
        Device::Pad
    } else if same(device_text, "kb") {
        Device::Keyboard
    } else {
        return Err(format!("device must be pad or kb: '{text}'"));
    };
    let colon = rest.find(':');
    let Some(colon) = colon.filter(|c| *c != 0) else {
        return Err(format!("missing :LABEL or keys in '{text}'"));
    };
    let (mut keys_text, label) = (&rest[..colon], &rest[colon + 1..]);
    if label.is_empty() || label.len() >= LABEL_MAX {
        return Err(format!("label must be 1..31 characters: '{text}'"));
    }
    if !label
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(format!(
            "label may only use letters, digits, _ and -: '{text}'"
        ));
    }
    let mut hold_ms = if device == Device::Pad {
        PAD_DEFAULT_HOLD_MS
    } else {
        0
    };
    if let Some(at) = keys_text.find('@') {
        hold_ms = parse_hold(&keys_text[at + 1..])
            .ok_or_else(|| format!("hold must be 0..10000 ms: '{text}'"))?;
        keys_text = &keys_text[..at];
    }
    let mut keys: Vec<String> = Vec::new();
    let mut seen: Vec<Vec<String>> = Vec::new();
    for part in keys_text.split('+') {
        if part.is_empty() || part.len() >= NAME_MAX {
            return Err(format!("empty or too long key name in '{text}'"));
        }
        if keys.len() >= KEYS_MAX {
            return Err(format!("at most 5 keys: '{text}'"));
        }
        let name = part.to_ascii_uppercase();
        let aliases = accept_of(device, &name);
        if aliases.is_empty() {
            return Err(format!("unknown key '{name}' for that device"));
        }
        if seen
            .iter()
            .flatten()
            .any(|earlier| aliases.iter().any(|alias| same(earlier, alias)))
        {
            return Err(format!(
                "key '{name}' is listed twice (or overlaps another key)"
            ));
        }
        seen.push(aliases);
        keys.push(name);
    }
    if keys.len() < 2 {
        return Err(format!(
            "a hotkey needs at least 2 keys (a single key would be taken from the game): '{text}'"
        ));
    }
    Ok(Spec {
        device,
        keys,
        hold_ms,
        label: label.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_chords_parse() {
        let spec = parse("pad=BACK+START+LB+Y@1500:stop").unwrap();
        assert_eq!(
            (spec.keys.len(), spec.hold_ms, spec.label.as_str()),
            (4, 1500, "stop")
        );
        assert_eq!(parse("kb=CTRL+SHIFT+S:shot").unwrap().hold_ms, 0);
        assert_eq!(parse("pad=BACK+A:x").unwrap().hold_ms, 500);
    }

    #[test]
    fn refusals_match_the_c_messages() {
        assert_eq!(
            parse("pad").unwrap_err(),
            "not DEVICE=KEY+KEY[@MS]:LABEL: 'pad'"
        );
        assert_eq!(
            parse("tv=A+B:x").unwrap_err(),
            "device must be pad or kb: 'tv=A+B:x'"
        );
        assert_eq!(
            parse("pad=A+A:x").unwrap_err(),
            "key 'A' is listed twice (or overlaps another key)"
        );
        assert_eq!(
            parse("kb=CTRL+LCTRL:x").unwrap_err(),
            "key 'LCTRL' is listed twice (or overlaps another key)"
        );
        assert_eq!(
            parse("pad=A+Q:x").unwrap_err(),
            "unknown key 'Q' for that device"
        );
        assert_eq!(
            parse("pad=A@20000+B:x").unwrap_err(),
            "hold must be 0..10000 ms: 'pad=A@20000+B:x'"
        );
        assert!(
            parse("pad=A:x")
                .unwrap_err()
                .starts_with("a hotkey needs at least 2 keys")
        );
        assert_eq!(
            parse("pad=A+B+X+Y+LB+RB:x").unwrap_err(),
            "at most 5 keys: 'pad=A+B+X+Y+LB+RB:x'"
        );
        assert!(
            parse("pad=A+:x")
                .unwrap_err()
                .starts_with("empty or too long key name")
        );
    }

    #[test]
    fn actions() {
        assert_eq!(
            [
                action_of("MARK"),
                action_of("stop"),
                action_of("Shot"),
                action_of("mu"),
                action_of("dump")
            ],
            [1, 2, 3, 4, 0]
        );
    }
}
