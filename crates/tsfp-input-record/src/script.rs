// SPDX-License-Identifier: GPL-3.0-or-later
//! The pad script grammar: one line per run, `<frames> [TOKEN ...]`, `#` starts a comment.

const DPAD_UP: u16 = 0x0001;
const DPAD_DOWN: u16 = 0x0002;
const DPAD_LEFT: u16 = 0x0004;
const DPAD_RIGHT: u16 = 0x0008;
const START: u16 = 0x0010;
const BACK: u16 = 0x0020;
const LEFT_THUMB: u16 = 0x0040;
const RIGHT_THUMB: u16 = 0x0080;

const DIGITAL: [(&str, u16); 8] = [
    ("UP", DPAD_UP),
    ("DOWN", DPAD_DOWN),
    ("LEFT", DPAD_LEFT),
    ("RIGHT", DPAD_RIGHT),
    ("START", START),
    ("BACK", BACK),
    ("LTHUMB", LEFT_THUMB),
    ("RTHUMB", RIGHT_THUMB),
];
const ANALOG: [&str; 8] = ["A", "B", "X", "Y", "BLACK", "WHITE", "LT", "RT"];
const AXES: [&str; 4] = ["LX", "LY", "RX", "RY"];
/// Longest script line the C parser accepts (its buffer is 512 bytes including the NUL).
const MAX_LINE: usize = 511;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PadState {
    pub digital_buttons: u16,
    pub analog: [u8; 8],
    /// Left X, left Y, right X, right Y.
    pub thumbs: [i16; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub frames: u32,
    pub state: PadState,
}

/// A parsed script: runs of frames, then the pad at rest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Script {
    entries: Vec<Entry>,
    total: u64,
}

impl Script {
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn total_frames(&self) -> u64 {
        self.total
    }

    pub fn state_at(&self, poll_index: u64) -> PadState {
        let mut base = 0u64;
        for entry in &self.entries {
            base += u64::from(entry.frames);
            if poll_index < base {
                return entry.state;
            }
        }
        PadState::default()
    }
}

/// `strtol(text, base 10)` that must consume the whole text and land in `min..=max`.
fn parse_number(text: &str, min: i64, max: i64) -> Option<i64> {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        at += 1;
    }
    let negative = match bytes.get(at) {
        Some(b'-') => {
            at += 1;
            true
        }
        Some(b'+') => {
            at += 1;
            false
        }
        _ => false,
    };
    let digits = at;
    let mut value: i128 = 0;
    while at < bytes.len() && bytes[at].is_ascii_digit() {
        value = (value * 10 + i128::from(bytes[at] - b'0')).min(i128::from(u64::MAX));
        at += 1;
    }
    if at == digits || at != bytes.len() {
        return None;
    }
    let value = if negative { -value } else { value };
    // strtol saturates on overflow and sets ERANGE: the C reader refuses that too.
    if value > i128::from(i64::MAX) || value < i128::from(i64::MIN) {
        return None;
    }
    let value = value as i64;
    (min..=max).contains(&value).then_some(value)
}

fn failure(line: u32, what: &str, token: &str) -> String {
    format!("line {line}: {what} '{token}'")
}

/// Apply one token (`NAME` or `NAME=value`) to `state`; the error is the C message.
pub(crate) fn apply_token(state: &mut PadState, token: &str, line: u32) -> Result<(), String> {
    let (name, value_text) = match token.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (token, None),
    };
    let name = name.to_ascii_uppercase();
    let Some(value_text) = value_text else {
        if let Some((_, bit)) = DIGITAL.iter().find(|(n, _)| *n == name) {
            state.digital_buttons |= bit;
            return Ok(());
        }
        if let Some(index) = ANALOG[..6].iter().position(|n| *n == name) {
            state.analog[index] = 255;
            return Ok(());
        }
        return Err(failure(line, "unknown token", &name));
    };
    if let Some(index) = ANALOG.iter().position(|n| *n == name) {
        let value = parse_number(value_text, 0, 255)
            .ok_or_else(|| failure(line, "analog value not 0..255", value_text))?;
        state.analog[index] = value as u8;
        return Ok(());
    }
    if let Some(index) = AXES.iter().position(|n| *n == name) {
        let value = parse_number(value_text, -32768, 32767)
            .ok_or_else(|| failure(line, "axis value not -32768..32767", value_text))?;
        state.thumbs[index] = value as i16;
        return Ok(());
    }
    Err(failure(line, "unknown token", &name))
}

fn is_separator(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r')
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Parse a script. The error names the line, like `xinput_script_parse`.
pub fn parse_script(text: &[u8]) -> Result<Script, String> {
    let mut script = Script::default();
    let mut at = 0;
    let mut line_number = 0u32;
    while at < text.len() {
        let end = text[at..]
            .iter()
            .position(|b| *b == b'\n')
            .map_or(text.len(), |n| at + n);
        let raw = &text[at..end];
        at = end + 1;
        line_number += 1;
        if raw.len() > MAX_LINE {
            return Err(failure(line_number, "line too long", ""));
        }
        // The C line is a NUL terminated string: a NUL or '#' ends it.
        let end = raw
            .iter()
            .position(|b| *b == 0 || *b == b'#')
            .unwrap_or(raw.len());
        let mut tokens = raw[..end]
            .split(|b| is_separator(*b))
            .filter(|t| !t.is_empty());
        let Some(first) = tokens.next() else { continue };
        let first = lossy(first);
        let frames = parse_number(&first, 1, 1_000_000)
            .ok_or_else(|| failure(line_number, "frame count not 1..1000000", &first))?;
        let mut state = PadState::default();
        for token in tokens {
            apply_token(&mut state, &lossy(token), line_number)?;
        }
        script.entries.push(Entry {
            frames: frames as u32,
            state,
        });
        script.total += frames as u64;
    }
    Ok(script)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_and_rest() {
        let script = parse_script(b"2 A UP\n# note\n\n3 lx=-5 LT=7 # c\n").unwrap();
        assert_eq!(script.total_frames(), 5);
        assert_eq!(script.state_at(0).analog[0], 255);
        assert_eq!(script.state_at(1).digital_buttons, DPAD_UP);
        assert_eq!(script.state_at(2).thumbs[0], -5);
        assert_eq!(script.state_at(4).analog[6], 7);
        assert_eq!(script.state_at(5), PadState::default());
    }

    #[test]
    fn errors_match_c_wording() {
        assert_eq!(
            parse_script(b"0 A").unwrap_err(),
            "line 1: frame count not 1..1000000 '0'"
        );
        assert_eq!(
            parse_script(b"1\n1 zz").unwrap_err(),
            "line 2: unknown token 'ZZ'"
        );
        assert_eq!(
            parse_script(b"1 a=256").unwrap_err(),
            "line 1: analog value not 0..255 '256'"
        );
        assert_eq!(
            parse_script(b"1 up=1").unwrap_err(),
            "line 1: unknown token 'UP'"
        );
        assert_eq!(
            parse_script(b"1 lt").unwrap_err(),
            "line 1: unknown token 'LT'"
        );
    }
}
