// SPDX-License-Identifier: GPL-3.0-or-later
//! `markK:field,field,...` wait specs, plain (`--route-wait`) and event (`--route-wait-event`, `# wait:`).
use crate::cond::{Cond, CondKind, legacy_mem, lower_text, mem_event_condition};
use crate::number::number;
use crate::{
    COND_MAX, DEFAULT_MAX_POLLS, DEFAULT_TIMEOUT_MS, MAX_MARKS, MAX_POLLS_LIMIT, TIMEOUT_MS_LIMIT,
};

/// `xinput_route_wait` of `xinput_route_spec.h`, same layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wait {
    pub mark: u32,
    pub has_mem: bool,
    pub address: u32,
    pub width: u32,
    pub value: u32,
    pub mask: u32,
    pub min_polls: u64,
    pub max_polls: u64,
    pub event: bool,
    pub conds: [Cond; COND_MAX],
    pub cond_count: u32,
    pub any: bool,
    pub min_ms: u64,
    pub timeout_ms: u64,
}

impl Wait {
    pub const ZERO: Wait = Wait {
        mark: 0,
        has_mem: false,
        address: 0,
        width: 0,
        value: 0,
        mask: 0,
        min_polls: 0,
        max_polls: 0,
        event: false,
        conds: [Cond::ZERO; COND_MAX],
        cond_count: 0,
        any: false,
        min_ms: 0,
        timeout_ms: 0,
    };

    fn add(&mut self, cond: Cond) -> bool {
        let index = self.cond_count as usize;
        if index >= COND_MAX {
            return false;
        }
        self.conds[index] = cond;
        self.cond_count += 1;
        true
    }
}

fn find(text: &[u8], byte: u8) -> Option<usize> {
    text.iter().position(|&candidate| candidate == byte)
}

/// Text before an optional `@`, and the part after it.
fn split_at_sign(text: &[u8]) -> (&[u8], Option<&[u8]>) {
    match find(text, b'@') {
        Some(at) => (&text[..at], Some(&text[at + 1..])),
        None => (text, None),
    }
}

fn nonzero(text: &[u8], limit: u64) -> Option<u64> {
    number(text, limit).filter(|&value| value != 0)
}

/// One field of an event wait; true when it is accepted.
fn event_field(field: &[u8], wait: &mut Wait) -> bool {
    let mut cond = Cond::ZERO;
    if let Some(text) = field.strip_prefix(b"mem=") {
        return mem_event_condition(text).is_some_and(|parsed| wait.add(parsed));
    }
    if let Some(text) = field.strip_prefix(b"file-open=") {
        cond.kind = CondKind::FILE_OPEN;
        return lower_text(text).is_some_and(|lower| {
            cond.set_text(&lower);
            wait.add(cond)
        });
    }
    if let Some(text) = field.strip_prefix(b"file-read=") {
        cond.kind = CondKind::FILE_READ;
        cond.amount = 1;
        let (name, amount) = split_at_sign(text);
        if let Some(amount) = amount {
            match nonzero(amount, 0xFFFF_FFFF) {
                Some(amount) => cond.amount = amount,
                None => return false,
            }
        }
        return lower_text(name).is_some_and(|lower| {
            cond.set_text(&lower);
            wait.add(cond)
        });
    }
    if let Some(text) = field.strip_prefix(b"file-idle=") {
        cond.kind = CondKind::FILE_IDLE;
        let (ms, name) = split_at_sign(text);
        let Some(amount) = nonzero(ms, TIMEOUT_MS_LIMIT) else {
            return false;
        };
        cond.amount = amount;
        if let Some(name) = name {
            let Some(lower) = lower_text(name) else {
                return false;
            };
            cond.set_text(&lower);
        }
        return wait.add(cond);
    }
    if let Some(text) = field.strip_prefix(b"call=") {
        cond.kind = CondKind::CALL;
        cond.amount = 1;
        let (va, count) = split_at_sign(text);
        let Some(va) = nonzero(va, 0x03FF_FFFF) else {
            return false;
        };
        if let Some(count) = count {
            match nonzero(count, 0xFF_FFFF) {
                Some(count) => cond.amount = count,
                None => return false,
            }
        }
        cond.va = va as u32;
        return wait.add(cond);
    }
    if field == b"frame-change" {
        cond.kind = CondKind::FRAME_CHANGE;
        return wait.add(cond);
    }
    if let Some(text) = field.strip_prefix(b"frame-stable=") {
        cond.kind = CondKind::FRAME_STABLE;
        let Some(amount) = nonzero(text, TIMEOUT_MS_LIMIT) else {
            return false;
        };
        cond.amount = amount;
        return wait.add(cond);
    }
    let once = |slot: &mut u64, text: &[u8], limit: u64| {
        let parsed = (*slot == 0).then(|| nonzero(text, limit)).flatten();
        parsed.is_some_and(|value| {
            *slot = value;
            true
        })
    };
    if let Some(text) = field.strip_prefix(b"min=") {
        return once(&mut wait.min_polls, text, MAX_POLLS_LIMIT);
    }
    if let Some(text) = field.strip_prefix(b"min-ms=") {
        return once(&mut wait.min_ms, text, TIMEOUT_MS_LIMIT);
    }
    if let Some(text) = field.strip_prefix(b"max=") {
        return once(&mut wait.max_polls, text, MAX_POLLS_LIMIT);
    }
    if let Some(text) = field.strip_prefix(b"timeout=") {
        return once(&mut wait.timeout_ms, text, TIMEOUT_MS_LIMIT);
    }
    if field == b"any" && !wait.any {
        wait.any = true;
        return true;
    }
    false
}

/// Up to 24 bytes of `text` as `%.24s` prints them.
fn near(text: &[u8]) -> String {
    String::from_utf8_lossy(&text[..text.len().min(24)]).into_owned()
}

pub fn wait_parse(spec: Option<&[u8]>, events: bool) -> Result<Wait, String> {
    let mut wait = Wait::ZERO;
    wait.max_polls = if events { 0 } else { DEFAULT_MAX_POLLS };
    wait.event = events;
    let shown = String::from_utf8_lossy(spec.unwrap_or(b"")).into_owned();
    let Some(spec) = spec.filter(|text| text.starts_with(b"mark")) else {
        let want = if events {
            "file-open=...,mem=...,timeout=MS"
        } else {
            "mem=...,min=N,max=N"
        };
        return Err(format!("route wait '{shown}': want markK:{want}"));
    };
    let bad_mark = || format!("route wait '{shown}': the mark must be mark1..mark{MAX_MARKS}");
    let colon = find(spec, b':').ok_or_else(bad_mark)?;
    let mark = nonzero(&spec[4..colon.max(4)], MAX_MARKS)
        .filter(|_| colon >= 4)
        .ok_or_else(bad_mark)?;
    wait.mark = mark as u32;
    let mut have_min = false;
    let mut rest = &spec[colon + 1..];
    while !rest.is_empty() {
        let end = find(rest, b',').unwrap_or(rest.len());
        let field = &rest[..end];
        let good = if events {
            event_field(field, &mut wait)
        } else if field.starts_with(b"mem=") && !wait.has_mem {
            let text = &field[4..];
            let parsed = (!text.is_empty() && text.len() < 96)
                .then(|| legacy_mem(text))
                .flatten();
            parsed.is_some_and(|(address, width, value, mask)| {
                (
                    wait.has_mem,
                    wait.address,
                    wait.width,
                    wait.value,
                    wait.mask,
                ) = (true, address, width, value, mask);
                true
            })
        } else if field.starts_with(b"min=") && !have_min {
            number(&field[4..], MAX_POLLS_LIMIT).is_some_and(|polls| {
                wait.min_polls = polls;
                have_min = true;
                true
            })
        } else if field.starts_with(b"max=") {
            nonzero(&field[4..], MAX_POLLS_LIMIT).is_some_and(|polls| {
                wait.max_polls = polls;
                true
            })
        } else {
            false
        };
        if !good {
            return Err(format!(
                "route wait '{shown}': bad, repeated or too many fields near '{}'",
                near(rest)
            ));
        }
        rest = if end < rest.len() {
            &rest[end + 1..]
        } else {
            &rest[end..]
        };
    }
    finish(wait, &shown, events, have_min)
}

fn finish(mut wait: Wait, shown: &str, events: bool, have_min: bool) -> Result<Wait, String> {
    let fail = |why: &str| Err(format!("route wait '{shown}': {why}"));
    if !events {
        if !wait.has_mem && !have_min {
            return fail("give mem=ADDR==VALUE or min=N, else it never waits");
        }
        if wait.min_polls > wait.max_polls {
            return fail("min is above max");
        }
        return Ok(wait);
    }
    if wait.cond_count == 0 && wait.min_polls == 0 && wait.min_ms == 0 {
        return fail(
            "give at least one condition (mem=, file-open=, ...) or min=/min-ms=, else it never waits",
        );
    }
    if wait.any && wait.cond_count < 2 {
        return fail("'any' needs two or more conditions");
    }
    if wait.timeout_ms == 0 && wait.max_polls == 0 {
        wait.timeout_ms = DEFAULT_TIMEOUT_MS;
    }
    if wait.timeout_ms != 0 && wait.min_ms > wait.timeout_ms {
        return fail("min-ms is above timeout");
    }
    if wait.max_polls != 0 && wait.min_polls > wait.max_polls {
        return fail("min is above max");
    }
    Ok(wait)
}
