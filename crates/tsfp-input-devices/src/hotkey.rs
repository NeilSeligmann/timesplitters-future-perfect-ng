// SPDX-License-Identifier: GPL-3.0-or-later
//! Host hotkey chord engine (`struct xinput_hotkeys` of `src/input/xinput_hotkey.c`, T1627/T1632).
//!
//! Rust owns the held-key bits, the swallow and leak tables and the fired counters. The C wrapper keeps the `--hotkey`
//! grammar and the key tables (it passes each key's accepted aliases) and delivers the [`Notice`]s this engine returns
//! to its fire and leak callbacks, in the order the C engine called them.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub const MAX_ENTRIES: usize = 8;
pub const KEYS_MAX: usize = 5;
pub const NAME_MAX: usize = 16;
pub const SWALLOWED_MAX: usize = 16;
pub const LEAK_MAX: usize = 8;

/// The C `xinput_hotkey_device` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    Keyboard = 1,
    Pad = 2,
}

impl Device {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            1 => Some(Self::Keyboard),
            2 => Some(Self::Pad),
            _ => None,
        }
    }
}

/// One chord: per key element, the names (aliases) that press it.
#[derive(Clone, Debug)]
pub struct ChordSpec {
    pub device: Device,
    pub hold_ms: u32,
    pub label: String,
    pub accept: Vec<Vec<String>>,
}

/// What the host callbacks must be told, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    Fire {
        number: u32,
        label: String,
        poll: u64,
    },
    Leak {
        device: Device,
        name: String,
        poll: u64,
        begin: bool,
    },
}

struct Entry {
    spec: ChordSpec,
    held: [u8; KEYS_MAX],
    complete: bool,
    fired: bool,
    since_ms: u64,
}

struct Tracked {
    device: Device,
    name: String,
    down_poll: u64,
    tainted: bool,
}

#[derive(Default)]
pub struct Hotkeys {
    entries: Vec<Entry>,
    dir: Option<PathBuf>,
    fired: u32,
    write_failures: u32,
    swallowed_total: u32,
    swallow_overflow: u32,
    swallowed: Vec<(Device, String)>,
    leaks: Vec<Tracked>,
    leak_overflow: u32,
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// The C engine stored names in `char[NAME_MAX]`.
fn clip(name: &str) -> String {
    let mut end = name.len().min(NAME_MAX - 1);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].to_owned()
}

fn locate(entry: &Entry, name: &str) -> Option<(usize, usize)> {
    entry
        .spec
        .accept
        .iter()
        .enumerate()
        .find_map(|(element, aliases)| {
            aliases
                .iter()
                .position(|alias| same(alias, name))
                .map(|alias| (element, alias))
        })
}

impl Hotkeys {
    /// `None` for no specs, too many, a chord of fewer than 2 or more than 5 keys, or a directory that cannot be made.
    pub fn new(specs: Vec<ChordSpec>, dir: Option<&str>) -> Option<Self> {
        if specs.is_empty() || specs.len() > MAX_ENTRIES {
            return None;
        }
        if specs.iter().any(|s| {
            s.accept.len() < 2 || s.accept.len() > KEYS_MAX || s.accept.iter().any(Vec::is_empty)
        }) {
            return None;
        }
        let dir = match dir {
            Some(path) => {
                fs::create_dir_all(path).ok()?;
                Some(PathBuf::from(path))
            }
            None => None,
        };
        let entries = specs
            .into_iter()
            .map(|spec| Entry {
                spec,
                held: [0; KEYS_MAX],
                complete: false,
                fired: false,
                since_ms: 0,
            })
            .collect();
        Some(Self {
            entries,
            dir,
            ..Self::default()
        })
    }

    pub fn fired(&self) -> u32 {
        self.fired
    }
    pub fn write_failures(&self) -> u32 {
        self.write_failures
    }
    pub fn swallowed_total(&self) -> u32 {
        self.swallowed_total
    }
    pub fn swallow_overflow(&self) -> u32 {
        self.swallow_overflow
    }
    pub fn leak_overflow(&self) -> u32 {
        self.leak_overflow
    }

    pub fn needs_device(&self, device: Device) -> bool {
        self.entries.iter().any(|e| e.spec.device == device)
    }

    fn write_file(&self, number: u32, label: &str, poll: u64, now_ms: u64) -> bool {
        let Some(dir) = &self.dir else { return true };
        let path = dir.join(format!("hotkey.{number}"));
        let temporary = dir.join(format!("hotkey.{number}.tmp"));
        let result = fs::File::create(&temporary)
            .and_then(|mut file| writeln!(file, "{number} {label} poll={poll} ms={now_ms}"))
            .and_then(|()| fs::rename(&temporary, &path));
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.is_ok()
    }

    fn fire(&mut self, index: usize, now_ms: u64, poll: u64, out: &mut Vec<Notice>) {
        self.entries[index].fired = true;
        self.fired += 1;
        let number = self.fired;
        let label = self.entries[index].spec.label.clone();
        let written = self.write_file(number, &label, poll, now_ms);
        if !written {
            self.write_failures += 1;
        }
        eprintln!(
            "hotkey (T1627): '{label}' #{number} at host poll {poll}{}",
            if written {
                ""
            } else {
                " (WRITING THE FILE FAILED)"
            }
        );
        // T1632: the chord keys the game DID see (pressed before the lead) are part of this gesture now.
        let entry = &self.entries[index];
        for episode in &mut self.leaks {
            if episode.tainted
                || episode.device != entry.spec.device
                || locate(entry, &episode.name).is_none()
            {
                continue;
            }
            episode.tainted = true;
            out.push(Notice::Leak {
                device: episode.device,
                name: episode.name.clone(),
                poll: episode.down_poll,
                begin: true,
            });
        }
        out.push(Notice::Fire {
            number,
            label,
            poll,
        });
    }

    fn evaluate(&mut self, now_ms: u64, poll: u64, out: &mut Vec<Notice>) {
        for index in 0..self.entries.len() {
            let entry = &mut self.entries[index];
            let all = entry.held[..entry.spec.accept.len()]
                .iter()
                .all(|&h| h != 0);
            if !all {
                entry.complete = false; // re-arms: the next whole chord starts a new press
                continue;
            }
            if !entry.complete {
                entry.complete = true;
                entry.fired = false;
                entry.since_ms = now_ms;
            }
            if !entry.fired && now_ms.wrapping_sub(entry.since_ms) >= u64::from(entry.spec.hold_ms)
            {
                self.fire(index, now_ms, poll, out);
            }
        }
    }

    /// Time passed with no event: a held chord whose hold time elapsed fires here.
    pub fn tick(&mut self, now_ms: u64, poll: u64) -> Vec<Notice> {
        let mut out = Vec::new();
        self.evaluate(now_ms, poll, &mut out);
        out
    }

    fn swallowed_find(&self, device: Device, name: &str) -> Option<usize> {
        self.swallowed
            .iter()
            .position(|(d, n)| *d == device && same(n, name))
    }

    fn leak_find(&self, device: Device, name: &str) -> Option<usize> {
        self.leaks
            .iter()
            .position(|l| l.device == device && same(&l.name, name))
    }

    /// One key or button event. The bool is true when the event is to be FORWARDED to the pad source.
    pub fn event(
        &mut self,
        device: Device,
        name: &str,
        down: bool,
        now_ms: u64,
        poll: u64,
    ) -> (bool, Vec<Notice>) {
        let mut out = Vec::new();
        if !down {
            for entry in &mut self.entries {
                if entry.spec.device != device {
                    continue;
                }
                if let Some((element, alias)) = locate(entry, name) {
                    entry.held[element] &= !(1u8 << alias);
                }
            }
            self.evaluate(now_ms, poll, &mut out);
            let Some(index) = self.swallowed_find(device, name) else {
                if let Some(leak) = self.leak_find(device, name) {
                    let episode = self.leaks.swap_remove(leak);
                    if episode.tainted {
                        out.push(Notice::Leak {
                            device,
                            name: clip(name),
                            poll,
                            begin: false,
                        });
                    }
                }
                return (true, out);
            };
            self.swallowed.swap_remove(index);
            self.swallowed_total += 1;
            return (false, out);
        }
        let (mut already, mut chord) = (false, false);
        for entry in &mut self.entries {
            if entry.spec.device != device {
                continue;
            }
            let Some((element, alias)) = locate(entry, name) else {
                continue;
            };
            if entry.held[element] & (1u8 << alias) != 0 {
                already = true;
            }
            entry.held[element] |= 1u8 << alias;
            // T1632: a chord key is kept from the game only while the chord's LEAD (its first key) is held.
            if element != 0 && entry.held[0] != 0 {
                chord = true;
            }
        }
        let mut forward = true;
        if already {
            forward = self.swallowed_find(device, name).is_none();
            if !forward {
                self.swallowed_total += 1;
            }
        } else if chord {
            if self.swallowed.len() < SWALLOWED_MAX {
                self.swallowed.push((device, clip(name)));
                self.swallowed_total += 1;
                forward = false;
            } else {
                self.swallow_overflow += 1;
            }
        }
        let member = self
            .entries
            .iter()
            .any(|e| e.spec.device == device && locate(e, name).is_some());
        if forward && !already && member && self.leak_find(device, name).is_none() {
            if self.leaks.len() < LEAK_MAX {
                self.leaks.push(Tracked {
                    device,
                    name: clip(name),
                    down_poll: poll,
                    tainted: false,
                });
            } else {
                self.leak_overflow += 1;
            }
        }
        self.evaluate(now_ms, poll, &mut out);
        (forward, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(keys: &[&str], hold_ms: u32, label: &str) -> ChordSpec {
        ChordSpec {
            device: Device::Pad,
            hold_ms,
            label: label.to_owned(),
            accept: keys.iter().map(|k| vec![(*k).to_owned()]).collect(),
        }
    }

    #[test]
    fn lead_forwarded_rest_swallowed_and_fires_once() {
        let mut h =
            Hotkeys::new(vec![chord(&["BACK", "START", "LB"], 0, "advance")], None).unwrap();
        assert_eq!(h.event(Device::Pad, "BACK", true, 0, 1), (true, vec![]));
        assert!(!h.event(Device::Pad, "START", true, 1, 2).0);
        let (forward, notices) = h.event(Device::Pad, "LB", true, 2, 3);
        assert!(!forward);
        // the forwarded lead is part of the gesture too (T1632)
        assert_eq!(
            notices,
            vec![
                Notice::Leak {
                    device: Device::Pad,
                    name: "BACK".into(),
                    poll: 1,
                    begin: true
                },
                Notice::Fire {
                    number: 1,
                    label: "advance".into(),
                    poll: 3
                }
            ]
        );
        assert!(h.tick(50, 4).is_empty());
        assert!(!h.event(Device::Pad, "LB", false, 60, 5).0);
        assert_eq!(h.swallowed_total(), 3);
    }

    #[test]
    fn hold_time_leak_taint_span() {
        let mut h = Hotkeys::new(vec![chord(&["BACK", "A"], 500, "mark")], None).unwrap();
        assert!(h.event(Device::Pad, "A", true, 0, 1).0); // pressed before the lead: leaked
        assert!(h.event(Device::Pad, "BACK", true, 10, 2).0);
        assert!(h.tick(400, 3).is_empty());
        let notices = h.tick(510, 4);
        assert_eq!(notices.len(), 3); // A, the lead BACK, the fire
        assert_eq!(
            notices[0],
            Notice::Leak {
                device: Device::Pad,
                name: "A".into(),
                poll: 1,
                begin: true
            }
        );
        let (_, up) = h.event(Device::Pad, "A", false, 520, 9);
        assert_eq!(
            up,
            vec![Notice::Leak {
                device: Device::Pad,
                name: "A".into(),
                poll: 9,
                begin: false
            }]
        );
    }

    #[test]
    fn repeated_down_follows_first_decision() {
        let mut h = Hotkeys::new(vec![chord(&["BACK", "A"], 0, "x")], None).unwrap();
        h.event(Device::Pad, "BACK", true, 0, 0);
        assert!(!h.event(Device::Pad, "A", true, 1, 0).0);
        assert!(!h.event(Device::Pad, "a", true, 2, 0).0);
        assert!(h.event(Device::Keyboard, "A", true, 3, 0).0);
    }

    #[test]
    fn rejects_bad_specs() {
        assert!(Hotkeys::new(vec![], None).is_none());
        assert!(Hotkeys::new(vec![chord(&["A"], 0, "x")], None).is_none());
    }
}
