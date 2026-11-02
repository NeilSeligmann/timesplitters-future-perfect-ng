// SPDX-License-Identifier: GPL-3.0-or-later
//! The scripted and live FABRICATED pad sources (T3926 slice 9, `src/input/xinput_source.c`).
//!
//! Script text is one line per run of frames, `<frames> [TOKEN ...]`; `#` starts a comment and blank
//! lines are ignored. Digital tokens `UP DOWN LEFT RIGHT START BACK LTHUMB RTHUMB`, face tokens
//! `A B X Y BLACK WHITE` (analog 255), `NAME=value` for the eight analog entries (0..255) and
//! `LX LY RX RY` (-32768..32767). The live source is one such line without the frame count, re-read
//! at every poll; a bad line keeps the previous state and counts as refused.

pub const ANALOG_COUNT: usize = 8;
const LINE_MAX: usize = 512;
const FRAMES_MAX: i64 = 1_000_000;

const DIGITAL: [(&str, u16); 8] = [
    ("UP", 0x0001),
    ("DOWN", 0x0002),
    ("LEFT", 0x0004),
    ("RIGHT", 0x0008),
    ("START", 0x0010),
    ("BACK", 0x0020),
    ("LTHUMB", 0x0040),
    ("RTHUMB", 0x0080),
];
const ANALOG: [&str; ANALOG_COUNT] = ["A", "B", "X", "Y", "BLACK", "WHITE", "LT", "RT"];
/// The first six analog entries are the face buttons a bare token presses.
const FACE_BUTTONS: usize = 6;
const AXES: [&str; 4] = ["LX", "LY", "RX", "RY"];

/// Pad values a source installs (the packet number is the device model's business).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PadValues {
    pub digital: u16,
    pub analog: [u8; ANALOG_COUNT],
    /// Left X, left Y, right X, right Y.
    pub thumbs: [i16; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub frames: u32,
    pub values: PadValues,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Script {
    entries: Vec<Entry>,
    total: u64,
}

fn refusal(line: usize, what: &str, token: &str) -> String {
    format!("line {line}: {what} '{token}'")
}

/// `strtol` base 10 over the whole text, bounded: leading blanks and one sign are accepted.
fn parse_number(text: &str, min: i64, max: i64) -> Option<i64> {
    let digits = text.trim_start_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r']);
    let (negative, digits) = match digits.as_bytes().first() {
        Some(b'-') => (true, &digits[1..]),
        Some(b'+') => (false, &digits[1..]),
        _ => (false, digits),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let magnitude: i128 = digits.parse().ok()?;
    let value = i64::try_from(if negative { -magnitude } else { magnitude }).ok()?;
    (min..=max).contains(&value).then_some(value)
}

fn apply_token(values: &mut PadValues, token: &str, line: usize) -> Result<(), String> {
    let (name, value) = match token.split_once('=') {
        Some((name, value)) => (name.to_ascii_uppercase(), Some(value)),
        None => (token.to_ascii_uppercase(), None),
    };
    let Some(value) = value else {
        if let Some((_, bit)) = DIGITAL.iter().find(|(n, _)| *n == name) {
            values.digital |= bit;
            return Ok(());
        }
        if let Some(i) = ANALOG[..FACE_BUTTONS].iter().position(|n| *n == name) {
            values.analog[i] = 255;
            return Ok(());
        }
        return Err(refusal(line, "unknown token", &name));
    };
    if let Some(i) = ANALOG.iter().position(|n| *n == name) {
        let v = parse_number(value, 0, 255)
            .ok_or_else(|| refusal(line, "analog value not 0..255", value))?;
        values.analog[i] = v as u8;
        return Ok(());
    }
    if let Some(i) = AXES.iter().position(|n| *n == name) {
        let v = parse_number(value, -32768, 32767)
            .ok_or_else(|| refusal(line, "axis value not -32768..32767", value))?;
        values.thumbs[i] = v as i16;
        return Ok(());
    }
    Err(refusal(line, "unknown token", &name))
}

/// The tokens of one line: cut at a NUL (a C string end) and at `#`, split on blank, tab, CR.
fn tokens(line: &[u8]) -> Vec<String> {
    let end = line
        .iter()
        .position(|b| matches!(b, 0 | b'#'))
        .unwrap_or(line.len());
    line[..end]
        .split(|b| matches!(b, b' ' | b'\t' | b'\r'))
        .filter(|t| !t.is_empty())
        .map(|t| String::from_utf8_lossy(t).into_owned())
        .collect()
}

impl Script {
    /// Parse script text; the error names the line like the C parser did.
    pub fn parse(text: &[u8]) -> Result<Script, String> {
        let mut script = Script::default();
        // The C reader never sees the empty tail after a final newline.
        let body = text.strip_suffix(b"\n").unwrap_or(text);
        for (index, line) in body.split(|b| *b == b'\n').enumerate() {
            let number = index + 1;
            if line.len() >= LINE_MAX {
                return Err(refusal(number, "line too long", ""));
            }
            let tokens = tokens(line);
            let Some((first, rest)) = tokens.split_first() else {
                continue;
            };
            let frames = parse_number(first, 1, FRAMES_MAX)
                .ok_or_else(|| refusal(number, "frame count not 1..1000000", first))?;
            let mut values = PadValues::default();
            for token in rest {
                apply_token(&mut values, token, number)?;
            }
            script.push(Entry {
                frames: frames as u32,
                values,
            });
        }
        Ok(script)
    }

    pub fn from_entries(entries: &[Entry]) -> Script {
        let mut script = Script::default();
        for entry in entries {
            script.push(*entry);
        }
        script
    }

    fn push(&mut self, entry: Entry) {
        self.total += u64::from(entry.frames);
        self.entries.push(entry);
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn total_frames(&self) -> u64 {
        self.total
    }

    /// Values at poll `poll_index`; the pad at rest after the last run.
    pub fn state_at(&self, poll_index: u64) -> PadValues {
        let mut base = 0u64;
        for entry in &self.entries {
            if poll_index < base + u64::from(entry.frames) {
                return entry.values;
            }
            base += u64::from(entry.frames);
        }
        PadValues::default()
    }
}

/// One live line (no frame count). `None` when a token is refused.
pub fn parse_live_line(text: &[u8]) -> Option<PadValues> {
    let end = text
        .iter()
        .take(LINE_MAX - 1)
        .position(|b| matches!(b, b'\n' | 0))
        .unwrap_or(text.len().min(LINE_MAX - 1));
    let mut values = PadValues::default();
    for token in tokens(&text[..end]) {
        apply_token(&mut values, &token, 1).ok()?;
    }
    Some(values)
}

/// The live pad: the last good state and how many reads were refused.
#[derive(Clone, Debug, Default)]
pub struct Live {
    last: PadValues,
    refused: u64,
}

impl Live {
    /// Feed one read of the file (`None`: the read failed) and get the values to install.
    pub fn poll(&mut self, read: Option<&[u8]>) -> PadValues {
        match read.map(parse_live_line) {
            Some(Some(values)) => self.last = values,
            _ => self.refused += 1,
        }
        self.last
    }

    pub fn refused(&self) -> u64 {
        self.refused
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(text: &str) -> String {
        Script::parse(text.as_bytes()).unwrap_err()
    }

    #[test]
    fn runs_tokens_and_rest() {
        let script = Script::parse(b"2 UP START # c\n\n3\n1 A LT=7 LX=-5 ry=300\n").unwrap();
        assert_eq!((script.entry_count(), script.total_frames()), (3, 6));
        assert_eq!(script.state_at(1).digital, 0x0011);
        assert_eq!(script.state_at(2), PadValues::default());
        let last = script.state_at(5);
        assert_eq!((last.analog[0], last.analog[6]), (255, 7));
        assert_eq!(last.thumbs, [-5, 0, 0, 300]);
        assert_eq!(script.state_at(6), PadValues::default());
    }

    #[test]
    fn refusals_name_the_line() {
        assert_eq!(err("1\nzz"), "line 2: frame count not 1..1000000 'zz'");
        assert_eq!(err("0"), "line 1: frame count not 1..1000000 '0'");
        assert_eq!(err("1 FOO"), "line 1: unknown token 'FOO'");
        assert_eq!(err("1 UP=1"), "line 1: unknown token 'UP'");
        assert_eq!(err("1 a=256"), "line 1: analog value not 0..255 '256'");
        assert_eq!(
            err("1 lx=40000"),
            "line 1: axis value not -32768..32767 '40000'"
        );
        assert_eq!(err("1 LX="), "line 1: axis value not -32768..32767 ''");
        assert_eq!(err("1 LT"), "line 1: unknown token 'LT'");
        assert_eq!(err(&"x".repeat(512)), "line 1: line too long ''");
    }

    #[test]
    fn live_keeps_last_good_state() {
        let mut live = Live::default();
        assert_eq!(live.poll(Some(b"UP LX=9  \n")).digital, 1);
        assert_eq!(live.poll(Some(b"NOPE")).digital, 1);
        assert_eq!(live.poll(None).thumbs[0], 9);
        assert_eq!(live.refused(), 2);
        assert_eq!(live.poll(Some(b"")), PadValues::default());
    }
}
