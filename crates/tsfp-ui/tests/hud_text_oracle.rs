// SPDX-License-Identifier: GPL-3.0-or-later
//! T1909 Tier 1 differential of the HUD text path (`hud_text`) against the ORIGINAL retail bytes
//! (same case format and harness as `hud_oracle.rs`, which this file copies; new file by design).
//!
//! `python3 -m tools.hud_oracle` runs the original function under Unicorn on seeded random guest
//! state and writes case files (stack args, every byte read with its initial value, every byte
//! written, EAX, the renderer/session calls made). This test replays each case through the Rust
//! port and compares writes, calls and EAX, and fails if the port reads a byte the original did
//! not. The case files are untracked (they contain bytes of the retail image); without them the
//! test prints SKIPPED and passes. Set `HUD_ORACLE_REQUIRED=1` to turn a missing directory into a
//! failure. Generate: `.venv/bin/python -m tools.hud_oracle --seeds 3000`.

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use tsfp_ui::hud_text::{self, RecordedCalls, StringCoreCall, TextCore, WrapCore};
use tsfp_ui::memory::GuestMemory;

#[path = "support/hex.rs"]
mod hex_support;
use hex_support::{hex, word};

#[path = "support/oracle_path.rs"]
mod oracle_path;
use oracle_path::oracle_dir;

struct CaseMemory {
    init: HashMap<u32, u8>,
    writes: BTreeMap<u32, u8>,
    missing: Cell<usize>,
}

impl GuestMemory for CaseMemory {
    fn read8(&self, address: u32) -> u8 {
        if let Some(byte) = self.writes.get(&address) {
            return *byte;
        }
        match self.init.get(&address) {
            Some(byte) => *byte,
            None => {
                self.missing.set(self.missing.get() + 1);
                0
            }
        }
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.writes.insert(address, value);
    }
}

struct Case {
    args: Vec<u32>,
    init: HashMap<u32, u8>,
    writes: BTreeMap<u32, u8>,
    calls: Vec<(u32, u32, Vec<u32>)>,
    eax: Option<u32>,
    /// `callx <index> <kind> ...` lines: (kind, rest)
    callx: Vec<(usize, String, Vec<String>)>,
}

fn parse(text: &str) -> Vec<Case> {
    let mut cases = Vec::new();
    let mut current: Option<Case> = None;
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            ["case", _] => {
                current = Some(Case {
                    args: vec![],
                    init: HashMap::new(),
                    writes: BTreeMap::new(),
                    calls: vec![],
                    eax: None,
                    callx: vec![],
                })
            }
            ["end"] => cases.push(current.take().unwrap()),
            [tag, rest @ ..] => {
                let case = current.as_mut().unwrap();
                match *tag {
                    "args" => case.args = rest.iter().map(|w| word(w)).collect(),
                    "init" | "write" => {
                        let address = word(rest[0]);
                        for (i, byte) in hex(rest[1]).into_iter().enumerate() {
                            if *tag == "init" {
                                case.init.insert(address + i as u32, byte);
                            } else {
                                case.writes.insert(address + i as u32, byte);
                            }
                        }
                    }
                    "call" => case.calls.push((
                        u32::from_str_radix(rest[0], 16).unwrap(),
                        word(rest[1]),
                        rest[2..].iter().map(|w| word(w)).collect(),
                    )),
                    "eax" => case.eax = Some(word(rest[0])),
                    "callx" => case.callx.push((
                        rest[0].parse().unwrap(),
                        rest[1].to_string(),
                        rest[2..].iter().map(|w| w.to_string()).collect(),
                    )),
                    _ => panic!("bad line {line}"),
                }
            }
            [] => {}
        }
    }
    cases
}

/// Run one spec file. Returns the number of cases checked.
fn check(
    name: &str,
    run: impl Fn(&mut CaseMemory, &Case, &mut RecordedCalls) -> Option<u32>,
) -> usize {
    let path = oracle_dir().join(format!("{name}.cases"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        assert!(
            std::env::var("HUD_ORACLE_REQUIRED").is_err(),
            "{name}: oracle cases missing at {path:?}"
        );
        eprintln!("SKIPPED {name}: no case file (run tools.hud_oracle)");
        return 0;
    };
    let cases = parse(&text);
    assert!(!cases.is_empty(), "{name}: empty case file");
    for (index, case) in cases.iter().enumerate() {
        let mut memory = CaseMemory {
            init: case.init.clone(),
            writes: BTreeMap::new(),
            missing: Cell::new(0),
        };
        let mut calls = RecordedCalls::default();
        let eax = run(&mut memory, case, &mut calls);
        assert_eq!(
            memory.missing.get(),
            0,
            "{name} case {index}: port read bytes the original did not"
        );
        assert_eq!(
            memory.writes, case.writes,
            "{name} case {index}: writes differ"
        );
        let expected: Vec<(u32, Vec<u32>)> = case
            .calls
            .iter()
            .map(|(callee, _, args)| (*callee, args.clone()))
            .collect();
        assert_eq!(calls.0, expected, "{name} case {index}: calls differ");
        if let Some(expected_eax) = case.eax {
            assert_eq!(eax, Some(expected_eax), "{name} case {index}: EAX differs");
        }
    }
    println!("{name}: {} cases agree with the original", cases.len());
    cases.len()
}

fn arg(case: &Case, index: usize) -> u32 {
    case.args[index]
}

#[test]
fn font_exchange_3fa10() {
    check("font_exchange_3fa10", |m, c, calls| {
        Some(hud_text::font_exchange(m, calls, arg(c, 0)))
    });
}

/// Replays the hooked callees of a case: render state returns the recorded EAX of the matching
/// call, the string core applies the recorded pointer outputs to x and y.
struct Replay<'a> {
    case: &'a Case,
    next_call: usize,
    seen: Vec<(u32, Vec<u32>, Vec<u32>)>,
}
impl Replay<'_> {
    fn take(&mut self, callee: u32) -> usize {
        let index = self.next_call;
        assert_eq!(
            self.case.calls[index].0, callee,
            "call order differs at {index}"
        );
        self.next_call += 1;
        index
    }
}
impl TextCore for Replay<'_> {
    fn render_state(&mut self, group: u32) -> u32 {
        let index = self.take(0x19650);
        self.seen.push((0x19650, vec![group], vec![]));
        self.case.calls[index].1
    }
    fn string_core(&mut self, call: &StringCoreCall, x: &mut f32, y: &mut f32) {
        let index = self.take(0x40FA0);
        let outs: Vec<&(usize, String, Vec<String>)> = self
            .case
            .callx
            .iter()
            .filter(|c| c.0 == index && c.1 == "out")
            .collect();
        let bits = |hex: &str| u32::from_le_bytes(hex_bytes(hex).try_into().unwrap());
        *x = f32::from_bits(bits(&outs[0].2[1]));
        *y = f32::from_bits(bits(&outs[1].2[1]));
        let peeks: Vec<u32> = self
            .case
            .callx
            .iter()
            .filter(|c| c.0 == index && c.1 == "peek")
            .map(|c| bits(&c.2[1]))
            .collect();
        self.seen.push((
            0x40FA0,
            vec![call.flags, call.text, call.callback],
            vec![call.first, call.sixth],
        ));
        assert_eq!(
            peeks,
            vec![call.first, call.sixth],
            "context struct words differ"
        );
    }
}
fn hex_bytes(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

#[test]
fn text_measure_42490() {
    let path = oracle_dir().join("text_measure_42490.cases");
    let Ok(text) = std::fs::read_to_string(&path) else {
        assert!(
            std::env::var("HUD_ORACLE_REQUIRED").is_err(),
            "oracle cases missing at {path:?}"
        );
        eprintln!("SKIPPED text_measure_42490: no case file");
        return;
    };
    let cases = parse(&text);
    assert!(!cases.is_empty());
    for (index, case) in cases.iter().enumerate() {
        let mut memory = CaseMemory {
            init: case.init.clone(),
            writes: BTreeMap::new(),
            missing: Cell::new(0),
        };
        let mut replay = Replay {
            case,
            next_call: 0,
            seen: vec![],
        };
        hud_text::measure_text(
            &mut memory,
            &mut replay,
            arg(case, 0),
            arg(case, 1),
            arg(case, 2),
            arg(case, 3),
        );
        assert_eq!(
            memory.missing.get(),
            0,
            "case {index}: port read bytes the original did not"
        );
        assert_eq!(
            replay.next_call,
            case.calls.len(),
            "case {index}: call count differs"
        );
        // pointer arguments of 0x40FA0 (stack addresses) are not comparable: only its by-value words
        for (i, (callee, _, args)) in case.calls.iter().enumerate() {
            let (seen_callee, seen_args, _) = &replay.seen[i];
            assert_eq!(callee, seen_callee);
            if *callee == 0x40FA0 {
                assert_eq!(
                    &args[2..5],
                    &seen_args[..],
                    "case {index}: string core arguments differ"
                );
            } else {
                assert_eq!(
                    args, seen_args,
                    "case {index}: render state argument differs"
                );
            }
        }
        assert_eq!(memory.writes, case.writes, "case {index}: writes differ");
    }
    println!(
        "text_measure_42490: {} cases agree with the original",
        cases.len()
    );
}

/// Replays the hooked callees of the word wrap (0x3FC90 parse code, 0x3FCF0 measure).
struct WrapReplay<'a> {
    case: &'a Case,
    next_call: usize,
}
impl WrapReplay<'_> {
    fn take(&mut self, callee: u32) -> usize {
        let index = self.next_call;
        assert_eq!(
            self.case.calls[index].0, callee,
            "call order differs at {index}"
        );
        self.next_call += 1;
        index
    }
    fn callx(&self, index: usize, kind: &str) -> Vec<&Vec<String>> {
        self.case
            .callx
            .iter()
            .filter(|c| c.0 == index && c.1 == kind)
            .map(|c| &c.2)
            .collect()
    }
}
impl WrapCore for WrapReplay<'_> {
    fn parse_code(&mut self, text_ptr: u32, flag: u32) -> u32 {
        let index = self.take(0x3FC90);
        let regs = self.callx(index, "reg");
        assert_eq!(regs[0][0], "eax");
        assert_eq!(
            u32::from_str_radix(&regs[0][1], 16).unwrap(),
            text_ptr,
            "parse code EAX differs"
        );
        assert_eq!(
            self.case.calls[index].2,
            vec![flag],
            "parse code stack argument differs"
        );
        self.case.calls[index].1
    }
    fn measure(&mut self, text: &[u8], flag: u32) -> (f32, f32) {
        let index = self.take(0x3FCF0);
        let peek = self.callx(index, "peek");
        // an empty string is written as an address with no hex digits
        let seen = peek[0].get(1).map_or_else(Vec::new, |hex| hex_bytes(hex));
        assert_eq!(seen, text, "measured text differs");
        assert_eq!(self.case.calls[index].2[3], flag);
        let outs = self.callx(index, "out");
        let bits =
            |hex: &str| f32::from_bits(u32::from_le_bytes(hex_bytes(hex).try_into().unwrap()));
        (bits(&outs[0][1]), bits(&outs[1][1]))
    }
}

#[test]
fn text_wrap_40c10() {
    let path = oracle_dir().join("text_wrap_40c10.cases");
    let Ok(text) = std::fs::read_to_string(&path) else {
        assert!(
            std::env::var("HUD_ORACLE_REQUIRED").is_err(),
            "oracle cases missing at {path:?}"
        );
        eprintln!("SKIPPED text_wrap_40c10: no case file");
        return;
    };
    let cases = parse(&text);
    assert!(!cases.is_empty());
    for (index, case) in cases.iter().enumerate() {
        let mut memory = CaseMemory {
            init: case.init.clone(),
            writes: BTreeMap::new(),
            missing: Cell::new(0),
        };
        let mut replay = WrapReplay { case, next_call: 0 };
        hud_text::wrap_text(
            &mut memory,
            &mut replay,
            arg(case, 0),
            f32::from_bits(arg(case, 1)),
            arg(case, 2),
        );
        assert_eq!(
            memory.missing.get(),
            0,
            "case {index}: port read bytes the original did not"
        );
        assert_eq!(
            replay.next_call,
            case.calls.len(),
            "case {index}: call count differs"
        );
        assert_eq!(memory.writes, case.writes, "case {index}: writes differ");
    }
    println!(
        "text_wrap_40c10: {} cases agree with the original",
        cases.len()
    );
}
