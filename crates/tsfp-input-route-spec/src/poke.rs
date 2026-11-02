// SPDX-License-Identifier: GPL-3.0-or-later
//! `--poke-at-poll WHERE:LABEL` triggers (`poke_trigger_*`).
use crate::number::number;
use crate::{MAX_MARKS, POKE_MAX};

pub struct PokeWhere;
impl PokeWhere {
    pub const POLL: u32 = 1;
    pub const MARK: u32 = 2;
    pub const END: u32 = 3;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PokeEntry {
    pub where_: u32,
    pub value: u64,
    pub label: [u8; 64],
    pub fired: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PokeSet {
    pub entries: [PokeEntry; POKE_MAX],
    pub count: u32,
}

impl PokeEntry {
    pub const ZERO: PokeEntry = PokeEntry {
        where_: 0,
        value: 0,
        label: [0; 64],
        fired: false,
    };
}

pub fn parse(spec: Option<&[u8]>) -> Result<PokeEntry, String> {
    let shown = String::from_utf8_lossy(spec.unwrap_or(b""));
    let colon = spec.and_then(|text| text.iter().position(|&byte| byte == b':'));
    let (Some(spec), Some(colon)) = (spec, colon) else {
        return Err(format!(
            "poke trigger '{shown}': want WHERE:LABEL (N, markK or replay-end)"
        ));
    };
    let place = &spec[..colon];
    let mut entry = PokeEntry::ZERO;
    if place == b"replay-end" {
        entry.where_ = PokeWhere::END;
    } else if let Some(mark) = place
        .strip_prefix(b"mark")
        .filter(|tail| !tail.is_empty())
        .and_then(|tail| number(tail, MAX_MARKS))
        .filter(|&mark| mark != 0)
    {
        entry.where_ = PokeWhere::MARK;
        entry.value = mark;
    } else if let Some(poll) = number(place, u64::from(u32::MAX)).filter(|&poll| poll != 0) {
        entry.where_ = PokeWhere::POLL;
        entry.value = poll;
    } else {
        return Err(format!(
            "poke trigger '{shown}': WHERE must be a poll number >= 1, mark1..mark{MAX_MARKS} or replay-end"
        ));
    }
    let label = &spec[colon + 1..];
    let clean = !label.is_empty()
        && label.len() < entry.label.len()
        && label
            .iter()
            .all(|&byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    if !clean {
        return Err(format!(
            "poke trigger '{shown}': the label must be 1..63 of [A-Za-z0-9_-]"
        ));
    }
    entry.label[..label.len()].copy_from_slice(label);
    Ok(entry)
}

impl PokeSet {
    /// Index of the first unfired entry that is due, marking it fired. POLL: `value` >= the entry's; MARK: equal; END: any.
    pub fn take(&mut self, where_: u32, value: u64) -> Option<usize> {
        let count = (self.count as usize).min(POKE_MAX);
        for (index, entry) in self.entries[..count].iter_mut().enumerate() {
            if entry.fired || entry.where_ != where_ {
                continue;
            }
            let due = match where_ {
                PokeWhere::POLL => value >= entry.value,
                PokeWhere::MARK => value == entry.value,
                _ => true,
            };
            if due {
                entry.fired = true;
                return Some(index);
            }
        }
        None
    }
}
