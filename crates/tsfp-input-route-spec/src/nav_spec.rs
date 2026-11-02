// SPDX-License-Identifier: GPL-3.0-or-later
//! The closed-loop menu navigation grammars (`# nav:` record lines, `--route-nav-timing`, the menu table of
//! `--route-nav-menus`) in Rust: a faithful port of `src/input/xinput_route_nav_spec.c` (same accept/reject set, same
//! parsed fields, same messages, including the intermediate `snprintf` truncations). The structs are `repr(C)`.
use crate::COND_MAX;
use crate::cond::{self, Cond};
use crate::number::number;
use crate::wait::{Wait, wait_parse};

pub const MAX_STEPS: usize = 64;
pub const MAX_MENUS: usize = 32;
pub const ID_MAX: usize = 32;
pub const NAME_MAX: usize = 64;
pub const SELECT_TEXT: usize = 112;
pub const EXPECT_MAX: usize = 224;
pub const LINE_MAX: usize = 512;
pub const MAX_TYPES: usize = 4;
pub const MAX_CLOSING: usize = 4;
pub const DEFAULT_TIMEOUT_MS: u64 = 120_000;
pub const DEFAULT_RETRY: u32 = 5;
pub const RETRY_LIMIT: u64 = 100;
pub const HOLD_POLLS: u32 = 2;
pub const GAP_POLLS: u32 = 3;
pub const LOST_EXTRA_POLLS: u32 = 2;
pub const TIMING_LIMIT: u64 = 120;
const NO_OFFSET: u32 = 0xFFFF_FFFF;
const BUTTON_START: u16 = 0x0010;
const BUTTON_BACK: u16 = 0x0020;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    pub hold: u32,
    pub gap: u32,
    pub extra: u32,
}

pub struct SelectKind;
impl SelectKind {
    pub const INDEX: u32 = 1;
    pub const NAME: u32 = 2;
    pub const ID: u32 = 3;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Step {
    pub at: u64,
    pub to: u64,
    pub menu: [u8; ID_MAX],
    pub kind: u32,
    pub index: u32,
    pub name: [u8; NAME_MAX],
    pub select_text: [u8; SELECT_TEXT],
    pub activate: bool,
    pub has_expect: bool,
    pub expect: [u8; EXPECT_MAX],
    pub expect_wait: Wait,
    pub timeout_ms: u64,
    pub retry: u32,
}

pub struct Mode;
impl Mode {
    pub const OFF: u32 = 0;
    pub const ON: u32 = 1;
    pub const STRICT: u32 = 2;
}

pub struct NamesKind;
impl NamesKind {
    pub const NONE: u32 = 0;
    pub const ASCII: u32 = 1;
    pub const UTF16: u32 = 2;
    pub const PTR_ASCII: u32 = 3;
    pub const PTR_UTF16: u32 = 4;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Ref {
    pub cond: Cond,
    pub page: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Names {
    pub kind: u32,
    pub base: Ref,
    pub stride: u32,
    pub max_len: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Items {
    pub present: bool,
    pub head: Ref,
    pub next_off: u32,
    pub type_off: u32,
    pub id_off: u32,
    pub text_off: u32,
    pub flags_off: u32,
    pub types: [u32; MAX_TYPES],
    pub type_count: u32,
    pub skip_mask: u32,
    pub grey_mask: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Widgets {
    pub present: bool,
    pub table: u32,
    pub stride: u32,
    pub count: u32,
    pub closing: [u32; MAX_CLOSING],
    pub closing_count: u32,
}

pub struct SelectBy;
impl SelectBy {
    pub const ID: u32 = 0;
    pub const NAME: u32 = 1;
    pub const INDEX: u32 = 2;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Menu {
    pub id: [u8; ID_MAX],
    pub dpad_off: bool,
    pub selectby: u32,
    pub page_builder: bool,
    pub builder: u32,
    pub widgets: Widgets,
    pub active: [Ref; COND_MAX],
    pub active_count: u32,
    pub ready: [Ref; COND_MAX],
    pub ready_count: u32,
    pub has_cursor: bool,
    pub cursor: Ref,
    pub count: Ref,
    pub itemid: Ref,
    pub has_itemid: bool,
    pub vertical: bool,
    pub wrap: bool,
    pub items: Items,
    pub names: Names,
    pub select_analog: i32,
    pub select_digital: u16,
    pub select_name: [u8; 8],
    pub back_name: [u8; 8],
    pub conf: [u8; 24],
    pub line: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MenuTable {
    pub widgets: Widgets,
    pub menus: [Menu; MAX_MENUS],
    pub count: usize,
}

fn zeroed<T: Copy>() -> T {
    // SAFETY: only used for the plain-old-data repr(C) structs above (integers, bools, byte arrays), all valid as zero.
    unsafe { std::mem::zeroed() }
}

impl Step {
    pub fn zero() -> Step {
        zeroed()
    }
}
impl Menu {
    pub fn zero() -> Menu {
        zeroed()
    }
}
impl MenuTable {
    pub fn zero() -> Box<MenuTable> {
        Box::new(zeroed())
    }
}

/// Text of a NUL-terminated array.
pub fn c_text(array: &[u8]) -> &[u8] {
    &array[..array.iter().position(|&b| b == 0).unwrap_or(array.len())]
}

fn set<const N: usize>(array: &mut [u8; N], text: &[u8]) {
    *array = [0; N];
    array[..text.len()].copy_from_slice(text);
}

fn show(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `snprintf(buffer, size, ..)` into an intermediate buffer: keeps `size - 1` bytes.
fn cap(mut text: String, size: usize) -> String {
    let mut keep = text.len().min(size - 1);
    while !text.is_char_boundary(keep) {
        keep -= 1;
    }
    text.truncate(keep);
    text
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

fn find(text: &[u8], byte: u8) -> Option<usize> {
    text.iter().position(|&candidate| candidate == byte)
}

pub fn timing_default() -> Timing {
    Timing {
        hold: HOLD_POLLS,
        gap: GAP_POLLS,
        extra: LOST_EXTRA_POLLS,
    }
}

pub fn timing_parse(spec: Option<&[u8]>) -> Result<Timing, String> {
    let text = spec.unwrap_or(b"");
    let parsed = (|| {
        let colon = find(text, b':')?;
        let tail = &text[colon + 1..];
        let second = find(tail, b':');
        let hold = number(&text[..colon], TIMING_LIMIT).filter(|&v| v != 0)?;
        let gap =
            number(&tail[..second.unwrap_or(tail.len())], TIMING_LIMIT).filter(|&v| v != 0)?;
        let extra = match second {
            Some(at) => number(&tail[at + 1..], TIMING_LIMIT)?,
            None => u64::from(LOST_EXTRA_POLLS),
        };
        Some(Timing {
            hold: hold as u32,
            gap: gap as u32,
            extra: extra as u32,
        })
    })();
    parsed.ok_or_else(|| {
        format!(
            "route nav timing '{}': want HOLD:GAP[:EXTRA], polls 1..{TIMING_LIMIT} (EXTRA 0..{TIMING_LIMIT})",
            show(text)
        )
    })
}

pub fn mode_parse(text: Option<&[u8]>) -> Option<u32> {
    match text? {
        b"on" => Some(Mode::ON),
        b"off" => Some(Mode::OFF),
        b"strict" => Some(Mode::STRICT),
        _ => None,
    }
}

fn hex_value(byte: u8) -> Option<u32> {
    (byte as char).to_digit(16)
}

/// `full` is the C string starting at the text (the `%.8s` of the message may read past `length`).
pub fn percent_decode(full: &[u8], length: usize, out_size: usize) -> Result<Vec<u8>, String> {
    let text = &full[..length.min(full.len())];
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let mut byte = text[i];
        if byte == b'%' {
            let hi = text.get(i + 1).and_then(|&b| hex_value(b));
            let lo = text.get(i + 2).and_then(|&b| hex_value(b));
            let (Some(hi), Some(lo)) = (hi, lo) else {
                let near = &full[i..full.len().min(i + 8)];
                return Err(format!(
                    "bad percent escape in the name near '{}' (want %XX with XX hex)",
                    show(near)
                ));
            };
            byte = (hi * 16 + lo) as u8;
            i += 2;
        }
        if !(0x20..=0x7E).contains(&byte) {
            return Err(format!(
                "the name has a byte 0x{byte:02X} outside printable ASCII"
            ));
        }
        if out.len() + 1 >= out_size {
            return Err(format!("the name is longer than {} bytes", out_size - 1));
        }
        out.push(byte);
        i += 1;
    }
    if out.is_empty() {
        return Err("the name is empty".to_string());
    }
    Ok(out)
}

fn clean_id(text: &[u8]) -> bool {
    !text.is_empty()
        && text.len() < ID_MAX
        && text
            .iter()
            .all(|&c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b'.')
}

#[derive(Default)]
struct Seen {
    at: bool,
    to: bool,
    menu: bool,
    select: bool,
    activate: bool,
    expect: bool,
    timeout: bool,
    retry: bool,
}

fn twice(flag: &mut bool, key: &str) -> Result<(), String> {
    if *flag {
        return Err(format!("'{key}' given twice"));
    }
    *flag = true;
    Ok(())
}

/// The text after `# nav:` of one record line.
pub fn line_parse(text: Option<&[u8]>) -> Result<Step, String> {
    let mut out = Step::zero();
    out.timeout_ms = DEFAULT_TIMEOUT_MS;
    out.retry = DEFAULT_RETRY;
    let Some(text) = text.filter(|t| t.len() <= LINE_MAX) else {
        return Err(format!("the nav line is longer than {LINE_MAX} characters"));
    };
    let mut seen = Seen::default();
    let mut at = 0;
    let lim = u64::from(u32::MAX) * 16;
    while at < text.len() {
        while at < text.len() && is_space(text[at]) {
            at += 1;
        }
        if at >= text.len() {
            break;
        }
        let mut end = at;
        while end < text.len() && !is_space(text[end]) {
            end += 1;
        }
        let token = &text[at..end];
        let equals = find(token, b'=');
        let key = &token[..equals.unwrap_or(token.len())];
        let value = equals.map_or(&text[end..end], |e| &token[e + 1..]);
        let has_eq = equals.is_some();
        let bad_number = |key: &str, value: &[u8]| format!("bad number in {key}={}", show(value));
        if key == b"at" && has_eq {
            twice(&mut seen.at, "at")?;
            out.at = number(value, lim).ok_or_else(|| bad_number("at", value))?;
        } else if key == b"to" && has_eq {
            twice(&mut seen.to, "to")?;
            out.to = number(value, lim).ok_or_else(|| bad_number("to", value))?;
        } else if key == b"menu" && has_eq {
            twice(&mut seen.menu, "menu")?;
            if !clean_id(value) {
                return Err(format!(
                    "bad menu id '{}' (1..{} of [A-Za-z0-9_.-])",
                    show(value),
                    ID_MAX - 1
                ));
            }
            set(&mut out.menu, value);
        } else if key == b"select" && has_eq {
            twice(&mut seen.select, "select")?;
            if value.len() >= SELECT_TEXT {
                return Err(format!(
                    "select= is longer than {} characters",
                    SELECT_TEXT - 1
                ));
            }
            set(&mut out.select_text, value);
            let bad = || format!("bad number in select={}", show(value));
            if value.len() > 6 && value.starts_with(b"index:") {
                out.index = number(&value[6..], 0xFF_FFFF).ok_or_else(bad)? as u32;
                out.kind = SelectKind::INDEX;
            } else if value.len() > 3 && value.starts_with(b"id:") {
                out.index = number(&value[3..], 0xFFFF_FFFF).ok_or_else(bad)? as u32;
                out.kind = SelectKind::ID;
            } else if value.len() > 5 && value.starts_with(b"name:") {
                let value_at = at + (token.len() - value.len());
                let reason = percent_decode(&text[value_at + 5..], value.len() - 5, NAME_MAX)
                    .map_err(|reason| format!("select={}: {}", show(value), cap(reason, 120)))?;
                set(&mut out.name, &reason);
                out.kind = SelectKind::NAME;
            } else {
                return Err(format!(
                    "select={}: want index:N, id:N or name:TEXT",
                    show(value)
                ));
            }
        } else if token == b"activate" {
            twice(&mut seen.activate, "activate")?;
            out.activate = true;
        } else if key == b"expect" && has_eq {
            twice(&mut seen.expect, "expect")?;
            if value.is_empty() || value.len() >= EXPECT_MAX {
                return Err(format!("expect= must be 1..{} characters", EXPECT_MAX - 1));
            }
            set(&mut out.expect, value);
            let mut spec = b"mark1:".to_vec();
            spec.extend_from_slice(value);
            out.expect_wait = wait_parse(Some(&spec), true)
                .map_err(|reason| format!("expect={}: {}", show(value), cap(reason, 200)))?;
            out.has_expect = true;
        } else if key == b"timeout" && has_eq {
            twice(&mut seen.timeout, "timeout")?;
            match number(value, crate::TIMEOUT_MS_LIMIT).filter(|&v| v != 0) {
                Some(v) => out.timeout_ms = v,
                None => {
                    return Err(format!(
                        "bad number in timeout={} (1..{} ms)",
                        show(value),
                        crate::TIMEOUT_MS_LIMIT
                    ));
                }
            }
        } else if key == b"retry" && has_eq {
            twice(&mut seen.retry, "retry")?;
            match number(value, RETRY_LIMIT) {
                Some(v) => out.retry = v as u32,
                None => {
                    return Err(format!(
                        "bad number in retry={} (0..{RETRY_LIMIT})",
                        show(value)
                    ));
                }
            }
        } else {
            return Err(format!(
                "unknown field '{}'",
                show(&token[..token.len().min(40)])
            ));
        }
        at = end;
    }
    if !seen.at {
        return Err("missing at=".to_string());
    }
    if !seen.to {
        return Err("missing to=".to_string());
    }
    if !seen.menu {
        return Err("missing menu=".to_string());
    }
    if !seen.select {
        return Err("missing select=".to_string());
    }
    if out.at > out.to {
        return Err(format!("at={} is above to={}", out.at, out.to));
    }
    if out.has_expect && !out.activate {
        return Err(
            "expect= is the post-condition of the activation, give `activate` too".to_string(),
        );
    }
    Ok(out)
}

pub fn steps_check(steps: &[Step], marks: &[u64], total: u64) -> Result<(), String> {
    if steps.len() > MAX_STEPS {
        return Err(format!(
            "{} nav lines, a route holds at most {MAX_STEPS}",
            steps.len()
        ));
    }
    for (i, step) in steps.iter().enumerate() {
        if step.to > total {
            return Err(format!(
                "nav step {}: to={} is past the record ({total} polls)",
                i + 1,
                step.to
            ));
        }
        if i != 0 && step.at < steps[i - 1].to {
            return Err(format!(
                "nav step {}: at={} overlaps or precedes step {} (to={}), steps must be ascending",
                i + 1,
                step.at,
                i,
                steps[i - 1].to
            ));
        }
        for (m, &mark) in marks.iter().enumerate() {
            if mark > step.at && mark < step.to {
                return Err(format!(
                    "nav step {}: mark {} at {mark} lies inside (at={}, to={}), a mark may sit at either end only",
                    i + 1,
                    m + 1,
                    step.at,
                    step.to
                ));
            }
        }
    }
    Ok(())
}

// ---- menu table ----

fn parse_ref_item(text: &[u8], condition: bool) -> Option<Ref> {
    let mut out = Ref {
        cond: Cond::ZERO,
        page: false,
    };
    let mut text = text;
    if text.first() == Some(&b'@') {
        if text.get(1) != Some(&b'+') || text.len() < 3 {
            return None;
        }
        out.page = true;
        text = &text[2..];
    }
    let parsed = if condition {
        cond::mem_event_condition(text)
    } else {
        cond::mem_ref(text)
    }?;
    out.cond = parsed;
    (!(out.page && out.cond.indirect)).then_some(out)
}

fn parse_cond_list(text: &[u8], key: &str) -> Result<Vec<Ref>, String> {
    let mut out = Vec::new();
    let mut at = 0;
    while at <= text.len() {
        let comma = find(&text[at..], b',');
        let part = comma.unwrap_or(text.len() - at);
        if part == 0 || part >= 96 || out.len() >= COND_MAX {
            return Err(format!(
                "{key}=: empty, too long or more than {COND_MAX} conditions"
            ));
        }
        let copy = &text[at..at + part];
        let item = parse_ref_item(copy, true).ok_or_else(|| {
            format!(
                "{key}=: bad condition '{}' (want [*]ADDR[+OFF][:W]OP VALUE[&MASK] or @+OFF[:W]OP VALUE[&MASK])",
                show(copy)
            )
        })?;
        out.push(item);
        match comma {
            None => break,
            Some(c) => at += c + 1,
        }
    }
    Ok(out)
}

fn parse_ref(text: &[u8], key: &str) -> Result<Ref, String> {
    if text.is_empty() || text.len() >= 96 {
        return Err(format!("{key}=: empty or too long"));
    }
    parse_ref_item(text, false).ok_or_else(|| {
        format!(
            "{key}=: bad memory reference '{}' (want [*]ADDR[+OFF][:W][&MASK] or @+OFF[:W][&MASK])",
            show(text)
        )
    })
}

fn parse_names(text: &[u8]) -> Result<Names, String> {
    if text.is_empty() || text.len() >= 160 {
        return Err("names=: empty or too long".to_string());
    }
    let mut parts: Vec<&[u8]> = Vec::new();
    let mut scan = text;
    let mut found = 0;
    loop {
        if found == 4 {
            found = 5;
            break;
        }
        parts.push(scan);
        found += 1;
        match find(scan, b'/') {
            None => break,
            Some(slash) => {
                let last = parts.len() - 1;
                parts[last] = &scan[..slash];
                scan = &scan[slash + 1..];
            }
        }
    }
    let limit_message = || {
        format!(
            "names=: want MEM/STRIDE/ascii|utf16|ptr-ascii|ptr-utf16/MAXLEN (STRIDE 1..4096, MAXLEN 1..{})",
            NAME_MAX - 1
        )
    };
    if found != 4 {
        return Err(limit_message());
    }
    let stride = number(parts[1], 4096).filter(|&v| v != 0);
    let max_len = number(parts[3], (NAME_MAX - 1) as u64).filter(|&v| v != 0);
    let (Some(stride), Some(max_len)) = (stride, max_len) else {
        return Err(limit_message());
    };
    let kind = match parts[2] {
        b"ascii" => NamesKind::ASCII,
        b"utf16" => NamesKind::UTF16,
        b"ptr-ascii" => NamesKind::PTR_ASCII,
        b"ptr-utf16" => NamesKind::PTR_UTF16,
        other => {
            return Err(format!(
                "names=: unknown encoding '{}' (ascii, utf16, ptr-ascii or ptr-utf16)",
                show(other)
            ));
        }
    };
    let base = parse_ref_item(parts[0], false)
        .ok_or_else(|| format!("names=: bad memory reference '{}'", show(parts[0])))?;
    Ok(Names {
        kind,
        base,
        stride: stride as u32,
        max_len: max_len as u32,
    })
}

fn plus_offset(text: &[u8]) -> Option<u32> {
    if text.len() < 2 || text[0] != b'+' {
        return None;
    }
    number(&text[1..], 0xFFFF).map(|v| v as u32)
}

fn parse_items(text: &[u8]) -> Result<Items, String> {
    if text.len() < 7
        || text.len() >= 300
        || !text.starts_with(b"walk(")
        || text[text.len() - 1] != b')'
    {
        return Err(
            "items=: want walk(HEAD;next=+N;type=+N:T1|T2;id=+N;text=+N;flags=+N;skip=MASK;grey=MASK)"
                .to_string(),
        );
    }
    let inner = &text[5..text.len() - 1];
    let mut out: Items = zeroed();
    out.id_off = NO_OFFSET;
    out.text_off = NO_OFFSET;
    out.flags_off = NO_OFFSET;
    let (mut have_next, mut have_type) = (false, false);
    for (index, scan) in inner.split(|&b| b == b';').enumerate() {
        if index == 0 {
            out.head = parse_ref_item(scan, false)
                .ok_or_else(|| format!("items=walk: bad head '{}'", show(scan)))?;
            continue;
        }
        let Some(eq) = find(scan, b'=') else {
            return Err(format!("items=walk: '{}' is not key=value", show(scan)));
        };
        let (key, value) = (&scan[..eq], &scan[eq + 1..]);
        let bad = |what: &str| format!("items=walk: bad {what} '{}'", show(value));
        match key {
            b"next" => {
                out.next_off = plus_offset(value).ok_or_else(|| bad("next"))?;
                have_next = true;
            }
            b"type" => {
                let bad_type =
                    || format!("items=walk: bad type '{}' (want +OFF:T1|T2)", show(value));
                let colon = find(value, b':').ok_or_else(bad_type)?;
                out.type_off = plus_offset(&value[..colon]).ok_or_else(bad_type)?;
                for item in value[colon + 1..].split(|&b| b == b'|') {
                    let number = number(item, 0xFFFF_FFFF);
                    let (true, Some(number)) = ((out.type_count as usize) < MAX_TYPES, number)
                    else {
                        return Err(format!(
                            "items=walk: bad type list '{}' (1..{MAX_TYPES} numbers)",
                            show(value)
                        ));
                    };
                    out.types[out.type_count as usize] = number as u32;
                    out.type_count += 1;
                }
                have_type = true;
            }
            b"id" => out.id_off = plus_offset(value).ok_or_else(|| bad("id"))?,
            b"text" => out.text_off = plus_offset(value).ok_or_else(|| bad("text"))?,
            b"flags" => out.flags_off = plus_offset(value).ok_or_else(|| bad("flags"))?,
            b"skip" => {
                out.skip_mask = number(value, 0xFFFF_FFFF).ok_or_else(|| bad("skip"))? as u32
            }
            b"grey" => {
                out.grey_mask = number(value, 0xFFFF_FFFF).ok_or_else(|| bad("grey"))? as u32
            }
            other => {
                return Err(format!("items=walk: unknown key '{}'", show(other)));
            }
        }
    }
    if !have_next
        || !have_type
        || ((out.skip_mask != 0 || out.grey_mask != 0) && out.flags_off == NO_OFFSET)
    {
        return Err(
            "items=walk: needs next=, type= and flags= when skip= or grey= is given".to_string(),
        );
    }
    out.present = true;
    Ok(out)
}

fn parse_button(text: &[u8]) -> Option<(i32, u16)> {
    if text.is_empty() || text.len() >= 8 {
        return None;
    }
    const ANALOG: [&[u8]; 6] = [b"A", b"B", b"X", b"Y", b"BLACK", b"WHITE"];
    if let Some(index) = ANALOG.iter().position(|&name| name == text) {
        return Some((index as i32, 0));
    }
    match text {
        b"START" => Some((-1, BUTTON_START)),
        b"BACK" => Some((-1, BUTTON_BACK)),
        _ => None,
    }
}

#[derive(Default)]
struct MenuKeys {
    page: bool,
    active: bool,
    cursor: bool,
    count: bool,
    axis: bool,
    wrap: bool,
    ready: bool,
    names: bool,
    select: bool,
    back: bool,
    conf: bool,
    itemid: bool,
    items: bool,
    dpad: bool,
    selectby: bool,
}

fn zero_one(value: &[u8], what: &str) -> Result<bool, String> {
    match value {
        b"0" => Ok(false),
        b"1" => Ok(true),
        _ => Err(format!("{what}= wants 0 or 1")),
    }
}

fn parse_menu_line(line: &[u8], number: u32) -> Result<Menu, String> {
    let mut menu = Menu::zero();
    set(&mut menu.select_name, b"A");
    menu.line = number;
    let mut seen = MenuKeys::default();
    let end = line.len();
    let mut at = 0;
    while at < end && is_space(line[at]) {
        at += 1;
    }
    if end - at < 5 || &line[at..at + 4] != b"menu" || !is_space(line[at + 4]) {
        return Err(format!("line {number}: expected `menu ID key=value ...`"));
    }
    at += 4;
    while at < end && is_space(line[at]) {
        at += 1;
    }
    let mut id_end = at;
    while id_end < end && !is_space(line[id_end]) {
        id_end += 1;
    }
    if !clean_id(&line[at..id_end]) {
        return Err(format!(
            "line {number}: bad menu id (1..{} of [A-Za-z0-9_.-])",
            ID_MAX - 1
        ));
    }
    set(&mut menu.id, &line[at..id_end]);
    at = id_end;
    while at < end {
        while at < end && is_space(line[at]) {
            at += 1;
        }
        if at >= end {
            break;
        }
        let mut token_end = at;
        while token_end < end && !is_space(line[token_end]) {
            token_end += 1;
        }
        let token = &line[at..token_end];
        let Some(eq) = find(token, b'=') else {
            return Err(format!(
                "line {number}: '{}' is not key=value",
                show(&token[..token.len().min(40)])
            ));
        };
        let (key, value) = (&token[..eq], &token[eq + 1..]);
        // The C code stores into the field only when the key was not seen; a repeat fails as "given twice".
        let (flag, result): (&mut bool, Result<(), String>) = match key {
            b"page" => {
                let flag = &mut seen.page;
                let result = if *flag {
                    Err(String::new())
                } else if value == b"none" {
                    Ok(())
                } else if value.len() > 8 && value.starts_with(b"builder:") {
                    match number_of(&value[8..]).filter(|&b| b != 0) {
                        Some(builder) => {
                            menu.page_builder = true;
                            menu.builder = builder;
                            Ok(())
                        }
                        None => Err("page= wants none or builder:0xVA".to_string()),
                    }
                } else {
                    Err("page= wants none or builder:0xVA".to_string())
                };
                (flag, result)
            }
            b"active" => {
                let flag = &mut seen.active;
                let result = if *flag {
                    Err(String::new())
                } else {
                    parse_cond_list(value, "active").map(|list| {
                        menu.active_count = list.len() as u32;
                        menu.active[..list.len()].copy_from_slice(&list);
                    })
                };
                (flag, result.map_err(|m| cap(m, 220)))
            }
            b"ready" => {
                let flag = &mut seen.ready;
                let result = if *flag {
                    Err(String::new())
                } else {
                    parse_cond_list(value, "ready").map(|list| {
                        menu.ready_count = list.len() as u32;
                        menu.ready[..list.len()].copy_from_slice(&list);
                    })
                };
                (flag, result.map_err(|m| cap(m, 220)))
            }
            b"cursor" => {
                let flag = &mut seen.cursor;
                let result = if *flag {
                    Err(String::new())
                } else {
                    parse_ref(value, "cursor").map(|r| menu.cursor = r)
                };
                (flag, result.map_err(|m| cap(m, 220)))
            }
            b"count" => {
                let flag = &mut seen.count;
                let result = if *flag {
                    Err(String::new())
                } else {
                    parse_ref(value, "count").map(|r| menu.count = r)
                };
                (flag, result.map_err(|m| cap(m, 220)))
            }
            b"itemid" => {
                let flag = &mut seen.itemid;
                let result = if *flag {
                    Err(String::new())
                } else {
                    parse_ref(value, "itemid").map(|r| {
                        menu.itemid = r;
                        menu.has_itemid = true;
                    })
                };
                (flag, result.map_err(|m| cap(m, 220)))
            }
            b"items" => {
                let flag = &mut seen.items;
                let result = if *flag {
                    Err(String::new())
                } else {
                    parse_items(value).map(|r| menu.items = r)
                };
                (flag, result.map_err(|m| cap(m, 220)))
            }
            b"axis" => {
                let flag = &mut seen.axis;
                let result = if *flag {
                    Err(String::new())
                } else if value == b"v" || value == b"h" {
                    menu.vertical = value == b"v";
                    Ok(())
                } else {
                    Err("axis= wants v or h".to_string())
                };
                (flag, result)
            }
            b"wrap" => {
                let flag = &mut seen.wrap;
                let result = if *flag {
                    Err(String::new())
                } else {
                    zero_one(value, "wrap").map(|w| menu.wrap = w)
                };
                (flag, result)
            }
            b"names" => {
                let flag = &mut seen.names;
                let result = if *flag {
                    Err(String::new())
                } else {
                    parse_names(value).map(|r| menu.names = r)
                };
                (flag, result.map_err(|m| cap(m, 220)))
            }
            b"select" => {
                let flag = &mut seen.select;
                let result = if *flag {
                    Err(String::new())
                } else {
                    match parse_button(value) {
                        Some((analog, digital)) => {
                            menu.select_analog = analog;
                            menu.select_digital = digital;
                            set(&mut menu.select_name, value);
                            Ok(())
                        }
                        None => Err("select= wants A B X Y BLACK WHITE START or BACK".to_string()),
                    }
                };
                (flag, result)
            }
            b"back" => {
                let flag = &mut seen.back;
                let result = if *flag {
                    Err(String::new())
                } else {
                    match parse_button(value) {
                        Some(_) => {
                            set(&mut menu.back_name, value);
                            Ok(())
                        }
                        None => Err("back= wants A B X Y BLACK WHITE START or BACK".to_string()),
                    }
                };
                (flag, result)
            }
            b"dpad" => {
                let flag = &mut seen.dpad;
                let result = if *flag {
                    Err(String::new())
                } else {
                    zero_one(value, "dpad").map(|w| menu.dpad_off = !w)
                };
                (flag, result)
            }
            b"selectby" => {
                let flag = &mut seen.selectby;
                let result = if *flag {
                    Err(String::new())
                } else {
                    match value {
                        b"id" => Ok(menu.selectby = SelectBy::ID),
                        b"name" => Ok(menu.selectby = SelectBy::NAME),
                        b"index" => Ok(menu.selectby = SelectBy::INDEX),
                        _ => Err("selectby= wants id, name or index".to_string()),
                    }
                };
                (flag, result)
            }
            b"conf" => {
                let flag = &mut seen.conf;
                let result = if *flag {
                    Err(String::new())
                } else if !value.is_empty() && value.len() < menu.conf.len() {
                    set(&mut menu.conf, value);
                    Ok(())
                } else {
                    Err(format!("conf= wants 1..{} characters", menu.conf.len() - 1))
                };
                (flag, result)
            }
            other => {
                return Err(format!(
                    "line {number}: unknown key '{}'",
                    show(&other[..other.len().min(40)])
                ));
            }
        };
        if let Err(reason) = result {
            if *flag && reason.is_empty() {
                return Err(format!("line {number}: '{}' given twice", show(key)));
            }
            return Err(format!("line {number}: {reason}"));
        }
        *flag = true;
        at = token_end;
    }
    menu.has_cursor = seen.cursor;
    let id = show(c_text(&menu.id));
    if !seen.active {
        return Err(format!("line {number}: menu {id} is missing active="));
    }
    if seen.cursor != seen.count {
        return Err(format!(
            "line {number}: menu {id} needs cursor= and count= together"
        ));
    }
    if seen.cursor && (!seen.axis || !seen.wrap) {
        return Err(format!(
            "line {number}: menu {id} is missing {}=",
            if !seen.axis { "axis" } else { "wrap" }
        ));
    }
    if !seen.cursor && (seen.itemid || seen.items || seen.names || seen.dpad || seen.selectby) {
        return Err(format!(
            "line {number}: menu {id} has itemid=, items=, names=, dpad= or selectby= without a cursor"
        ));
    }
    let uses_page = menu.active[..menu.active_count as usize]
        .iter()
        .any(|r| r.page)
        || menu.ready[..menu.ready_count as usize]
            .iter()
            .any(|r| r.page)
        || (seen.cursor && (menu.cursor.page || menu.count.page))
        || (seen.itemid && menu.itemid.page)
        || (seen.items && menu.items.head.page)
        || (seen.names && menu.names.base.page);
    if uses_page && !menu.page_builder {
        return Err(format!(
            "line {number}: menu {id} uses a @+OFF page spec but has no page=builder:VA"
        ));
    }
    Ok(menu)
}

fn number_of(text: &[u8]) -> Option<u32> {
    number(text, 0xFFFF_FFFF).map(|v| v as u32)
}

fn parse_widgets_line(line: &[u8], line_number: u32) -> Result<Widgets, String> {
    let mut out: Widgets = zeroed();
    let (mut have_table, mut have_stride, mut have_count, mut have_closing) =
        (false, false, false, false);
    let end = line.len();
    let mut at = 7;
    while at < end {
        while at < end && is_space(line[at]) {
            at += 1;
        }
        if at >= end {
            break;
        }
        let mut token_end = at;
        while token_end < end && !is_space(line[token_end]) {
            token_end += 1;
        }
        let token = &line[at..token_end];
        let eq = find(token, b'=');
        let key: &[u8] = eq.map_or(b"", |e| &token[..e]);
        let value: &[u8] = eq.map_or(b"", |e| &token[e + 1..]);
        let has_eq = eq.is_some();
        if has_eq
            && key == b"table"
            && !have_table
            && number(value, 0xFFFF_FFFF).is_some_and(|v| v != 0)
        {
            out.table = number(value, 0xFFFF_FFFF).unwrap() as u32;
            have_table = true;
        } else if has_eq
            && key == b"stride"
            && !have_stride
            && number(value, 0x10000).is_some_and(|v| v >= 0x1B4)
        {
            out.stride = number(value, 0x10000).unwrap() as u32;
            have_stride = true;
        } else if has_eq
            && key == b"count"
            && !have_count
            && number(value, 256).is_some_and(|v| v != 0)
        {
            out.count = number(value, 256).unwrap() as u32;
            have_count = true;
        } else if has_eq && key == b"closing" && !have_closing {
            for state in value.split(|&b| b == b',') {
                let parsed = number(state, 0xFFFF_FFFF);
                let (true, Some(parsed)) = ((out.closing_count as usize) < MAX_CLOSING, parsed)
                else {
                    return Err(format!(
                        "line {line_number}: widgets closing= wants 1..{MAX_CLOSING} state numbers"
                    ));
                };
                out.closing[out.closing_count as usize] = parsed as u32;
                out.closing_count += 1;
            }
            have_closing = true;
        } else {
            return Err(format!(
                "line {line_number}: widgets: bad, repeated or unknown field '{}' (want table=ADDR stride=N count=N [closing=S,S])",
                show(&token[..token.len().min(40)])
            ));
        }
        at = token_end;
    }
    if !have_table || !have_stride || !have_count {
        return Err(format!(
            "line {line_number}: widgets needs table=, stride= and count="
        ));
    }
    out.present = true;
    Ok(out)
}

pub fn menus_parse(text: &[u8]) -> Result<Box<MenuTable>, String> {
    let mut out = MenuTable::zero();
    let length = text.len();
    let mut at = 0;
    let mut number = 0u32;
    while at < length {
        let mut end = at;
        while end < length && text[end] != b'\n' {
            end += 1;
        }
        number += 1;
        let mut content = at;
        let mut stop = end;
        if let Some(hash) = find(&text[at..end], b'#') {
            stop = at + hash;
        }
        while content < stop && is_space(text[content]) {
            content += 1;
        }
        while stop > content && is_space(text[stop - 1]) {
            stop -= 1;
        }
        if stop > content {
            let line = &text[content..stop];
            if line.len() >= 8 && line.starts_with(b"widgets") && is_space(line[7]) {
                if out.widgets.present {
                    return Err(format!("line {number}: a second widgets line"));
                }
                out.widgets = parse_widgets_line(line, number)?;
            } else {
                if out.count >= MAX_MENUS {
                    return Err(format!("line {number}: more than {MAX_MENUS} menus"));
                }
                let menu = parse_menu_line(line, number)?;
                for existing in &out.menus[..out.count] {
                    if c_text(&existing.id) == c_text(&menu.id) {
                        return Err(format!(
                            "line {number}: menu {} is already defined on line {}",
                            show(c_text(&menu.id)),
                            existing.line
                        ));
                    }
                }
                let slot = out.count;
                out.menus[slot] = menu;
                out.count += 1;
            }
        }
        at = end + 1;
    }
    let widgets = out.widgets;
    for index in 0..out.count {
        let menu = &mut out.menus[index];
        if menu.page_builder && !widgets.present {
            return Err(format!(
                "line {}: menu {} has page=builder: but the table has no widgets line",
                menu.line,
                show(c_text(&menu.id))
            ));
        }
        menu.widgets = widgets;
    }
    Ok(out)
}

pub fn menu_find<'a>(table: &'a MenuTable, id: &[u8]) -> Option<&'a Menu> {
    table.menus[..table.count]
        .iter()
        .find(|menu| c_text(&menu.id) == id)
}
