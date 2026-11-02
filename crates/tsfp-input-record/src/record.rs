// SPDX-License-Identifier: GPL-3.0-or-later
//! The versioned record file: `#` header lines, the script body, marks, waits, navs and the
//! `# polls: N` completion trailer (`xinput_replay_load`).

use crate::flags::{canonical_line, flags_diff};
use crate::script::{Script, parse_script};

pub const RECORD_VERSION: u32 = 1;
pub const MAX_MARKS: usize = 16;
pub const MAX_WAITS: usize = 8;
pub const MAX_NAVS: usize = 64;
const WAIT_LEN: usize = 256;
const NAV_LEN: usize = 513;
const REFLAG_ADVICE: &str = "The route and the replay must run with identical game flags. Record the route again with tmp/record_mapmaker_route.fish (it builds the same flags as tmp/run_forced_weapons.fish) and replay that file, or replay with the flags of the recording.";

/// The wait budgets and interactive mode a recording ran with (`# budgets:` line).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budgets {
    pub owner_waits: u32,
    pub worker_blanks: u32,
    pub thread_timeout_ms: u32,
    pub interactive: bool,
}

/// Everything `xinput_replay_load` installs: the script and the record's side tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replay {
    pub script: Script,
    pub total_polls: u64,
    pub budgets: Option<Budgets>,
    pub marks: Vec<u64>,
    pub waits: Vec<String>,
    pub navs: Vec<String>,
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `%.{n}s`: at most `n` bytes.
fn prefix(text: &str, n: usize) -> String {
    lossy(&text.as_bytes()[..text.len().min(n)])
}

/// Lines of `text` the way the C readers walk them: split on `\n`, a final unterminated line counts.
fn lines(text: &[u8]) -> impl Iterator<Item = (usize, &[u8])> {
    let mut at = 0;
    std::iter::from_fn(move || {
        if at >= text.len() {
            return None;
        }
        let end = text[at..]
            .iter()
            .position(|b| *b == b'\n')
            .map_or(text.len(), |n| at + n);
        let line = (at, &text[at..end]);
        at = end + 1;
        Some(line)
    })
}

fn is_blank(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r')
}

fn trim_start(mut line: &[u8], blank: fn(u8) -> bool) -> &[u8] {
    while let Some((first, rest)) = line.split_first() {
        if !blank(*first) {
            break;
        }
        line = rest;
    }
    line
}

fn trim_end(mut line: &[u8]) -> &[u8] {
    while let Some((last, rest)) = line.split_last() {
        if !matches!(*last, b' ' | b'\r') {
            break;
        }
        line = rest;
    }
    line
}

/// The value of `# key: value` among the leading comment lines. `None` when absent or when the
/// value does not fit in a C buffer of `capacity` bytes.
fn header_value(text: &[u8], key: &str, capacity: usize) -> Option<String> {
    let key = key.as_bytes();
    for (_, line) in lines(text) {
        if line.first() != Some(&b'#') {
            break;
        }
        if line.len() > 2 + key.len() + 1
            && line.starts_with(b"# ")
            && line[2..].starts_with(key)
            && line[2 + key.len()] == b':'
        {
            let value = trim_start(&line[2 + key.len() + 1..], |b| b == b' ');
            return (value.len() < capacity).then(|| lossy(value));
        }
    }
    None
}

/// Check a record's header against the current identity; the error is the C refusal text.
pub fn check_header(
    text: &[u8],
    xbe_sha256: &str,
    flags_sha256: &str,
    flags: &str,
) -> Result<(), String> {
    let want = format!("tsfp-input v{RECORD_VERSION}");
    if text.len() < 2 || !text.starts_with(b"# ") {
        return Err("not an input record (no '# tsfp-input vN' header)".into());
    }
    let first = text[2..]
        .iter()
        .position(|b| *b == b'\n')
        .map_or(text.len(), |n| 2 + n);
    if &text[2..first] != want.as_bytes() {
        return Err(format!(
            "unsupported record version (first line is not '# {want}')"
        ));
    }
    // `value` is one buffer in the C reader: a lookup that finds nothing leaves the previous value.
    let mut value = String::from("(missing)");
    if let Some(found) = header_value(text, "xbe-sha256", 2048) {
        value = found;
    }
    if value != xbe_sha256 {
        return Err(format!(
            "recorded against a different XBE: record has {}, this run has {}",
            prefix(&value, 64),
            xbe_sha256
        ));
    }
    let mut recorded = String::from("(missing)");
    let have_flags_line = match header_value(text, "flags", 1500) {
        Some(found) => {
            recorded = found;
            true
        }
        None => false,
    };
    let canonical = if have_flags_line {
        canonical_line(&recorded, 2048).zip(canonical_line(flags, 2048))
    } else {
        None
    };
    if let Some(found) = header_value(text, "flags-sha256", 2048) {
        value = found;
    }
    let sha_match = value == flags_sha256;
    let canonical_match = canonical.as_ref().is_some_and(|(a, b)| a == b);
    if sha_match || canonical_match {
        return Ok(());
    }
    let Some((recorded_canonical, current_canonical)) = canonical else {
        return Err(format!(
            "recorded against a different flag set: the record has no usable '# flags:' line.\n  {REFLAG_ADVICE}"
        ));
    };
    let (differs, only_recorded, only_current) =
        flags_diff(&recorded_canonical, &current_canonical, 700, 700);
    if differs {
        let shown = |s: &str| {
            if s.is_empty() {
                "(nothing)".to_string()
            } else {
                s.to_string()
            }
        };
        Err(format!(
            "recorded against a different flag set.\n  only in the recording: {}\n  only in this run:      {}\n  {REFLAG_ADVICE}",
            shown(&only_recorded),
            shown(&only_current)
        ))
    } else {
        Err(format!(
            "recorded against a different flag set: the same flags in a different order.\n  recording: {}\n  this run:  {}\n  {REFLAG_ADVICE}",
            prefix(&recorded_canonical, 500),
            prefix(&current_canonical, 500)
        ))
    }
}

/// `# budgets: owner-waits=%u worker-blanks=%u thread-timeout=%u interactive=%u`, parsed as
/// glibc `sscanf` does (leading blanks skipped, optional sign, wraparound to `unsigned`).
pub fn parse_budgets(text: &[u8]) -> Option<Budgets> {
    let value = header_value(text, "budgets", 256)?;
    let mut rest = value.as_bytes();
    let mut numbers = [0u32; 4];
    for (slot, label) in [
        "owner-waits=",
        "worker-blanks=",
        "thread-timeout=",
        "interactive=",
    ]
    .iter()
    .enumerate()
    {
        rest = trim_start(rest, |b| b.is_ascii_whitespace() || b == 0x0b);
        rest = rest.strip_prefix(label.as_bytes())?;
        rest = trim_start(rest, |b| b.is_ascii_whitespace() || b == 0x0b);
        let negative = match rest.first() {
            Some(b'-') => {
                rest = &rest[1..];
                true
            }
            Some(b'+') => {
                rest = &rest[1..];
                false
            }
            _ => false,
        };
        let digits = rest.iter().take_while(|b| b.is_ascii_digit()).count();
        if digits == 0 {
            return None;
        }
        let magnitude = rest[..digits].iter().fold(0u64, |acc, d| {
            acc.saturating_mul(10).saturating_add(u64::from(d - b'0'))
        });
        rest = &rest[digits..];
        numbers[slot] = (if negative {
            magnitude.wrapping_neg()
        } else {
            magnitude
        }) as u32;
    }
    (numbers[3] <= 1).then_some(Budgets {
        owner_waits: numbers[0],
        worker_blanks: numbers[1],
        thread_timeout_ms: numbers[2],
        interactive: numbers[3] != 0,
    })
}

pub fn format_budgets(budgets: &Budgets) -> String {
    format!(
        "# budgets: owner-waits={} worker-blanks={} thread-timeout={} interactive={}\n",
        budgets.owner_waits,
        budgets.worker_blanks,
        budgets.thread_timeout_ms,
        u32::from(budgets.interactive)
    )
}

pub fn format_header(xbe_sha256: &str, flags_sha256: &str, flags: &str) -> String {
    format!(
        "# tsfp-input v{RECORD_VERSION}\n# xbe-sha256: {xbe_sha256}\n# flags-sha256: {flags_sha256}\n# flags: {flags}\n"
    )
}

/// The single terminal `# polls: N` declaration; nothing but comments may follow it.
fn completion_polls(text: &[u8]) -> Option<u64> {
    let mut found = None;
    for (at, raw) in lines(text) {
        let line = trim_start(raw, is_blank);
        if line.len() >= 8 && line.starts_with(b"# polls:") {
            if found.is_some() || at + raw.len() == text.len() {
                return None;
            }
            let rest = trim_start(&line[8..], |b| b == b' ' || b == b'\t');
            let digits = rest.iter().take_while(|b| b.is_ascii_digit()).count();
            if digits == 0 {
                return None;
            }
            let mut value = 0u64;
            for d in &rest[..digits] {
                value = value.checked_mul(10)?.checked_add(u64::from(d - b'0'))?;
            }
            if !rest[digits..].iter().all(|b| is_blank(*b)) {
                return None;
            }
            found = Some(value);
        } else if found.is_some() && !line.is_empty() && line[0] != b'#' {
            return None;
        }
    }
    found
}

fn parse_marks(text: &[u8], total: u64) -> Result<Vec<u64>, String> {
    let mut marks: Vec<u64> = Vec::new();
    for (_, line) in lines(text) {
        if line.len() < 8 || !line.starts_with(b"# mark:") {
            continue;
        }
        let rest = trim_start(&line[7..], |b| b == b' ');
        let malformed = || "malformed mark line".to_string();
        let rest = rest
            .strip_prefix(b"at=")
            .filter(|_| rest.len() >= 3)
            .ok_or_else(malformed)?;
        let digits = rest.iter().take_while(|b| b.is_ascii_digit()).count();
        let mut value = 0u64;
        for d in &rest[..digits] {
            value = value
                .checked_mul(10)
                .and_then(|v| v.checked_add(u64::from(d - b'0')))
                .ok_or_else(malformed)?;
        }
        let tail = trim_start(&rest[digits..], |b| b == b' ' || b == b'\r');
        if digits == 0
            || !tail.is_empty()
            || value > total
            || marks.len() == MAX_MARKS
            || marks.last().is_some_and(|last| value < *last)
        {
            return Err(format!(
                "bad mark {} (not a count, past the record, out of order or more than {MAX_MARKS})",
                marks.len() + 1
            ));
        }
        marks.push(value);
    }
    Ok(marks)
}

fn parse_waits(text: &[u8]) -> Result<Vec<String>, String> {
    let mut waits = Vec::new();
    for (index, (_, line)) in lines(text).enumerate() {
        if line.len() < 7 || !line.starts_with(b"# wait:") {
            continue;
        }
        let body = trim_end(trim_start(&line[7..], |b| b == b' '));
        if body.len() < 6
            || !body.starts_with(b"mark")
            || body.len() >= WAIT_LEN
            || waits.len() == MAX_WAITS
        {
            return Err(format!(
                "bad '# wait:' line {} (want 'mark<K>:fields', at most {MAX_WAITS} waits, {} characters)",
                index + 1,
                WAIT_LEN - 1
            ));
        }
        waits.push(lossy(body));
    }
    Ok(waits)
}

fn parse_navs(text: &[u8]) -> Result<Vec<String>, String> {
    let mut navs = Vec::new();
    for (index, (_, line)) in lines(text).enumerate() {
        if line.len() < 6 || !line.starts_with(b"# nav:") {
            continue;
        }
        let body = trim_end(trim_start(&line[6..], |b| b == b' '));
        if navs.len() == MAX_NAVS || body.is_empty() || body.len() >= NAV_LEN {
            return Err(format!(
                "bad '# nav:' line {} (empty, over {} characters, or more than {MAX_NAVS} nav lines)",
                index + 1,
                NAV_LEN - 1
            ));
        }
        navs.push(lossy(body));
    }
    Ok(navs)
}

/// `xinput_replay_load` on the file's bytes: verify the header, parse body and side tables.
pub fn load_replay(
    text: &[u8],
    xbe_sha256: &str,
    flags_sha256: &str,
    flags: &str,
) -> Result<Replay, String> {
    let completed = match text
        .contains(&0)
        .then_some(None)
        .unwrap_or_else(|| completion_polls(text))
    {
        Some(polls) => polls,
        None => return Err("incomplete or invalid input record trailer".into()),
    };
    check_header(text, xbe_sha256, flags_sha256, flags)?;
    let budgets = parse_budgets(text);
    let script = parse_script(text)?;
    if completed != script.total_frames() {
        return Err("record completion count differs from body".into());
    }
    let marks = parse_marks(text, completed)?;
    let waits = parse_waits(text)?;
    let navs = parse_navs(text)?;
    Ok(Replay {
        total_polls: script.total_frames(),
        script,
        budgets,
        marks,
        waits,
        navs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(body: &str) -> Vec<u8> {
        format!("{}{body}", format_header("xbe", "sha", "--a 1")).into_bytes()
    }

    #[test]
    fn loads_marks_waits_and_budgets() {
        let text = sample(
            "# budgets: owner-waits=5 worker-blanks=6 thread-timeout=7 interactive=1\n2 A\n# mark: at=2\n# wait: mark1:x=1\n1 UP\n# polls: 3\n",
        );
        let replay = load_replay(&text, "xbe", "sha", "--a 1").unwrap();
        assert_eq!(replay.total_polls, 3);
        assert_eq!(replay.marks, vec![2]);
        assert_eq!(replay.waits, vec!["mark1:x=1".to_string()]);
        assert_eq!(replay.budgets.unwrap().thread_timeout_ms, 7);
    }

    #[test]
    fn refuses_foreign_xbe_and_open_trailer() {
        let text = sample("1 A\n# polls: 1\n");
        assert!(
            load_replay(&text, "other", "sha", "--a 1")
                .unwrap_err()
                .starts_with("recorded against a different XBE")
        );
        let open = sample("1 A\n");
        assert_eq!(
            load_replay(&open, "xbe", "sha", "--a 1").unwrap_err(),
            "incomplete or invalid input record trailer"
        );
    }
}
