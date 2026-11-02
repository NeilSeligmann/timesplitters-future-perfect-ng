// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differential of the mechanically lifted HUD/UI functions (T1909) against the ORIGINAL
//! retail bytes. `python3 -m tools.hud_bulk_oracle` runs each original function under Unicorn on
//! seeded random guest state and writes `tmp/hud/oracle/bulk/<entry>.cases`; this test replays every
//! case through `tsfp_ui::hud::bulk::generated::run` and compares the bytes written, the nested calls
//! (callee, six stack words, six registers, in order), EAX and st(0) at return, and fails when the
//! port reads a byte the original did not. The case files embed retail bytes and are untracked:
//! without them the test prints SKIPPED. `HUD_ORACLE_REQUIRED=1` turns that into a failure.
//!
//! `tests/hud_bulk_tier1.txt` lists the entries that must agree on every case (the Tier 1 set). The
//! other lifted entries are reported but not asserted (they are the needs-manual list).

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use tsfp_ui::hud::bulk::generated;
use tsfp_ui::hud::bulk::rt::{Callee, Regs};
use tsfp_ui::memory::GuestMemory;

#[path = "support/hex.rs"]
mod hex_support;
use hex_support::{hex, word};

const STACK_TOP: u32 = 0x00C0_0000;
const STACK_EXCLUDE: u32 = STACK_TOP - 0x1000;
const SENTINEL: u32 = 0x0098_0000;

struct Mem {
    init: HashMap<u32, u8>,
    writes: BTreeMap<u32, u8>,
    stack: HashMap<u32, u8>,
    missing: Cell<Option<u32>>,
}

impl GuestMemory for Mem {
    fn read8(&self, address: u32) -> u8 {
        if address >= STACK_EXCLUDE && address < STACK_TOP {
            return self.stack.get(&address).copied().unwrap_or(0);
        }
        if let Some(byte) = self.writes.get(&address) {
            return *byte;
        }
        match self.init.get(&address) {
            Some(byte) => *byte,
            None => {
                if self.missing.get().is_none() {
                    self.missing.set(Some(address));
                }
                0
            }
        }
    }
    fn write8(&mut self, address: u32, value: u8) {
        if address >= STACK_EXCLUDE && address < STACK_TOP {
            self.stack.insert(address, value);
        } else {
            self.writes.insert(address, value);
        }
    }
}

struct Call {
    callee: u32,
    value: u32,
    words: Vec<u32>,
    st0: Option<u32>,
}

struct Case {
    args: Vec<u32>,
    regs: Vec<u32>,
    init: HashMap<u32, u8>,
    writes: BTreeMap<u32, u8>,
    calls: Vec<Call>,
    eax: u32,
    fret: Option<(i32, Option<u32>)>,
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
                    regs: vec![],
                    init: HashMap::new(),
                    writes: BTreeMap::new(),
                    calls: vec![],
                    eax: 0,
                    fret: None,
                })
            }
            ["end"] => cases.push(current.take().unwrap()),
            [tag, rest @ ..] => {
                let case = current.as_mut().unwrap();
                match *tag {
                    "args" => case.args = rest.iter().map(|w| word(w)).collect(),
                    "regs" => case.regs = rest.iter().map(|w| word(w)).collect(),
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
                    "call" => case.calls.push(Call {
                        callee: u32::from_str_radix(rest[0], 16).unwrap(),
                        value: word(rest[1]),
                        words: rest[2..].iter().map(|w| word(w)).collect(),
                        st0: None,
                    }),
                    "callx" => {
                        let index: usize = rest[0].parse().unwrap();
                        assert_eq!(rest[1], "st0", "bad line {line}");
                        case.calls[index].st0 = Some(word(rest[2]));
                    }
                    "eax" => case.eax = word(rest[0]),
                    "fret" => {
                        let bits = if rest[1] == "nan" { None } else { Some(word(rest[1])) };
                        case.fret = Some((rest[0].parse().unwrap(), bits));
                    }
                    _ => panic!("bad line {line}"),
                }
            }
            [] => {}
        }
    }
    cases
}

/// NaN payloads are not behaviour (owner decision 9a): two words that are both f32 NaNs agree.
/// This is a stated tolerance of the bulk Tier 1, not a bit-exact claim.
fn is_nan_bits(word: u32) -> bool {
    (word & 0x7f80_0000) == 0x7f80_0000 && (word & 0x007f_ffff) != 0
}

fn words_agree(expected: &[u32], port: &[u32]) -> bool {
    expected.len() == port.len()
        && expected.iter().zip(port).all(|(a, b)| a == b || (is_nan_bits(*a) && is_nan_bits(*b)))
}

struct Replay<'a> {
    calls: &'a [Call],
    next: usize,
    error: Option<String>,
}

impl Callee<Mem> for Replay<'_> {
    fn call(&mut self, m: &mut Mem, regs: &mut Regs, target: u32) {
        let index = self.next;
        self.next += 1;
        let Some(expected) = self.calls.get(index) else {
            self.error.get_or_insert(format!("extra call to {target:#x}"));
            return;
        };
        let mut words: Vec<u32> = (0..6).map(|i| m.read32(regs.esp.wrapping_add(4 + 4 * i))).collect();
        words.extend([regs.eax, regs.ecx, regs.edx, regs.ebx, regs.esi, regs.edi]);
        if expected.callee != target || !words_agree(&expected.words, &words) {
            self.error.get_or_insert(format!(
                "call {index}: original {:#x} {:x?} port {target:#x} {words:x?}",
                expected.callee, expected.words
            ));
        }
        regs.eax = expected.value;
        if let Some(bits) = expected.st0 {
            regs.fpush(f64::from(f32::from_bits(bits)));
        }
    }
}

fn setup(case: &Case) -> (Mem, Regs) {
    let mut memory = Mem {
        init: case.init.clone(),
        writes: BTreeMap::new(),
        stack: HashMap::new(),
        missing: Cell::new(None),
    };
    let mut esp = STACK_TOP - 0x100;
    for value in case.args.iter().rev() {
        esp -= 4;
        for (i, b) in value.to_le_bytes().iter().enumerate() {
            memory.stack.insert(esp + i as u32, *b);
        }
    }
    esp -= 4;
    for (i, b) in SENTINEL.to_le_bytes().iter().enumerate() {
        memory.stack.insert(esp + i as u32, *b);
    }
    let mut regs = Regs::new(esp);
    regs.eax = case.regs[0];
    regs.ecx = case.regs[1];
    regs.edx = case.regs[2];
    regs.ebx = case.regs[3];
    regs.esi = case.regs[4];
    regs.edi = case.regs[5];
    (memory, regs)
}

/// Compare what the port left behind with the original: reads, writes and EAX.
fn verify_memory(memory: &Mem, case: &Case, eax: Option<u32>) -> Result<(), String> {
    if let Some(address) = memory.missing.get() {
        return Err(format!("port read {address:#x} which the original did not"));
    }
    if memory.writes != case.writes {
        let diff = memory
            .writes
            .iter()
            .find(|(a, v)| case.writes.get(a) != Some(v))
            .map(|(a, v)| format!("port wrote {a:#x}={v:02x} original {:?}", case.writes.get(a)))
            .or_else(|| {
                case.writes
                    .keys()
                    .find(|a| !memory.writes.contains_key(a))
                    .map(|a| format!("original wrote {a:#x}={:02x}, port did not", case.writes[a]))
            })
            .unwrap_or_default();
        return Err(format!("writes differ: {diff}"));
    }
    if let Some(eax) = eax {
        if eax != case.eax {
            return Err(format!("eax port {eax:#x} original {:#x}", case.eax));
        }
    }
    Ok(())
}

fn run_case(entry: u32, case: &Case) -> Result<(), String> {
    let (mut memory, mut regs) = setup(case);
    let mut replay = Replay { calls: &case.calls, next: 0, error: None };
    let ok = catch_unwind(AssertUnwindSafe(|| generated::run(entry, &mut memory, &mut replay, &mut regs)));
    match ok {
        Ok(true) => {}
        Ok(false) => return Err("entry not lifted".into()),
        Err(_) => return Err("port panicked (fault the original did not raise)".into()),
    }
    if let Some(error) = replay.error {
        return Err(error);
    }
    if replay.next != case.calls.len() {
        return Err(format!("calls {} vs original {}", replay.next, case.calls.len()));
    }
    verify_memory(&memory, case, Some(regs.eax))?;
    match case.fret {
        None => {
            if regs.fdepth != 0 {
                return Err(format!("x87 depth {} vs original 0", regs.fdepth));
            }
        }
        Some((depth, bits)) => {
            if regs.fdepth != depth {
                return Err(format!("x87 depth {} vs original {depth}", regs.fdepth));
            }
            let value = regs.st(0) as f32;
            // x87 computes in 80 bits: an original f32 infinity that the f64 port saw as NaN is the
            // extended range (stated tolerance, documented in docs/t-hud-slice.md 7.5)
            let agrees = match bits {
                None => value.is_nan(),
                Some(b) if f32::from_bits(b).is_infinite() => value.is_nan() || value.to_bits() == b,
                Some(b) => value.to_bits() == b || (is_nan_bits(b) && value.is_nan()),
            };
            if !agrees {
                return Err(format!("st(0) port {value:e} original bits {bits:x?}"));
            }
        }
    }
    Ok(())
}

fn oracle_dir() -> PathBuf {
    match std::env::var("HUD_ORACLE_DIR") {
        Ok(dir) => PathBuf::from(dir).join("bulk"),
        Err(_) => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tmp/hud/oracle/bulk"),
    }
}

/// The enforced set lives in the gitignored `generated/hud/bulk-tier1.txt` (per-address bulk
/// output is rejected by the repo guard); `HUD_BULK_TIER1_FILE` overrides the path. The tracked
/// `hud_bulk_tier1.txt` is the always-available base set.
fn tier1_set() -> Vec<u32> {
    let extra = std::env::var("HUD_BULK_TIER1_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../generated/hud/bulk-tier1.txt"));
    let extra = std::fs::read_to_string(extra).unwrap_or_default();
    // upper half and gap entries (T1970) live in a sibling gitignored file
    let batch_b = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../generated/hud/bulk-tier1-b.txt");
    let extra = extra + "\n" + &std::fs::read_to_string(batch_b).unwrap_or_default();
    let base = include_str!("hud_bulk_tier1.txt").to_string() + "\n" + &extra;
    base.lines()
        .filter_map(|l| l.split('#').next())
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| u32::from_str_radix(l.trim_start_matches("0x"), 16).unwrap())
        .collect()
}

#[test]
fn bulk_tier1() {
    // guest faults the original raised (idiv by zero) are expected to panic the port quietly
    std::panic::set_hook(Box::new(|_| {}));
    let dir = oracle_dir();
    let Ok(listing) = std::fs::read_dir(&dir) else {
        assert!(std::env::var("HUD_ORACLE_REQUIRED").is_err(), "bulk oracle cases missing at {dir:?}");
        eprintln!("SKIPPED bulk_tier1: no case files (run tools.hud_bulk_oracle)");
        return;
    };
    let only: Option<Vec<u32>> = std::env::var("HUD_BULK_ONLY")
        .ok()
        .map(|v| v.split(',').map(|x| u32::from_str_radix(x.trim_start_matches("0x"), 16).unwrap()).collect());
    let mut files: Vec<_> = listing.filter_map(Result::ok).map(|e| e.path()).collect();
    files.sort();
    let required = tier1_set();
    let mut report = Vec::new();
    let mut failed_required = Vec::new();
    for path in files {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        let Ok(entry) = u32::from_str_radix(stem, 16) else { continue };
        if !generated::ENTRIES.contains(&entry) {
            continue;
        }
        if let Some(only) = &only {
            if !only.contains(&entry) {
                continue;
            }
        }
        let cases = parse(&std::fs::read_to_string(&path).unwrap());
        let mut passed = 0;
        let mut first_error = None;
        for (index, case) in cases.iter().enumerate() {
            match run_case(entry, case) {
                Ok(()) => passed += 1,
                Err(error) => {
                    first_error.get_or_insert(format!("case {index}: {error}"));
                }
            }
        }
        let status = if passed == cases.len() { "PASS" } else { "FAIL" };
        report.push(format!(
            "{entry:08x} {status} {passed}/{} {}",
            cases.len(),
            first_error.clone().unwrap_or_default()
        ));
        if status == "FAIL" && required.contains(&entry) {
            failed_required.push(format!("{entry:08x}: {}", first_error.unwrap_or_default()));
        }
    }
    if let Some(parent) = dir.parent() {
        let _ = std::fs::write(parent.join("bulk-results.txt"), report.join("\n") + "\n");
    }
    println!("{}", report.join("\n"));
    assert!(failed_required.is_empty(), "Tier 1 entries disagree with the original: {failed_required:#?}");
}

/// Cases of one entry, or None (with a SKIPPED note) when the case file is missing.
fn load_cases(entry: u32) -> Option<Vec<Case>> {
    let path = oracle_dir().join(format!("{entry:08x}.cases"));
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(parse(&text)),
        Err(_) => {
            assert!(std::env::var("HUD_ORACLE_REQUIRED").is_err(), "{path:?} missing");
            eprintln!("SKIPPED {entry:08x}: no case file (run tools.hud_bulk_oracle)");
            None
        }
    }
}

fn check_hand(entry: u32, run: impl Fn(&mut Mem, &Regs, &Case) -> Result<Option<u32>, String>) {
    let Some(cases) = load_cases(entry) else { return };
    assert!(!cases.is_empty(), "{entry:08x}: empty case file");
    for (index, case) in cases.iter().enumerate() {
        let (mut memory, regs) = setup(case);
        let eax = run(&mut memory, &regs, case).unwrap_or_else(|e| panic!("{entry:08x} case {index}: {e}"));
        verify_memory(&memory, case, eax).unwrap_or_else(|e| panic!("{entry:08x} case {index}: {e}"));
    }
    println!("{entry:08x}: {} cases agree with the original (hand port)", cases.len());
}

struct ReplayBackend<'a> {
    calls: &'a [Call],
    next: usize,
}

impl ReplayBackend<'_> {
    fn expect(&mut self, callee: u32) -> &Call {
        let call = &self.calls[self.next];
        self.next += 1;
        assert_eq!(call.callee, callee, "call order");
        call
    }
}

impl tsfp_ui::hud::textpath::TextBackend for ReplayBackend<'_> {
    fn state_group(&mut self, group: u32) -> u32 {
        let call = self.expect(0x19650);
        assert_eq!(call.words[0], group, "state group argument");
        call.value
    }
    fn render(&mut self, _x: &mut f32, _y: &mut f32, text: u32, _descriptor: &tsfp_ui::hud::textpath::TextDescriptor) {
        let call = self.expect(0x40fa0);
        // words: [&x, &y, 0, text, 0x40260, &descriptor]; the pointers are stack locals
        assert_eq!(&call.words[2..5], &[0, text, 0x40260], "render core arguments");
    }
}

#[test]
fn hand_ports_text_path() {
    use tsfp_ui::hud::textpath;
    check_hand(0x3fa10, |m, _, c| {
        let mut updates = 0;
        let previous = textpath::font_exchange_with(m, c.args[0], |_| updates += 1);
        assert_eq!((updates, c.calls.len(), c.calls[0].callee), (1, 1, 0x3f8d0));
        Ok(Some(previous))
    });
    check_hand(0x3f8d0, |m, _, _| {
        textpath::scale_globals_update(m);
        Ok(None)
    });
    check_hand(0x3fc90, |m, r, c| Ok(Some(textpath::parse_inline_number(m, r.eax, c.args[0]))));
    check_hand(0x42490, |m, _, c| {
        let mut backend = ReplayBackend { calls: &c.calls, next: 0 };
        textpath::draw_string_int_pos(m, &mut backend, c.args[0], c.args[1], c.args[2], c.args[3]);
        assert_eq!(backend.next, c.calls.len(), "call count");
        Ok(None)
    });
}
