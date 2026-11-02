// SPDX-License-Identifier: GPL-3.0-or-later
//! T1892 Tier 1 replay support. Path-included by the integration tests of the small-system
//! crates (`#[path = "../../tsfp-engine-memory/src/golden.rs"] mod golden;`). Reads the goldens
//! written by `python -m tools.small_diff` (recorded from the original XBE under Unicorn into the
//! gitignored `tmp/small-diff/`), replays each case against a Rust port and compares the result
//! register, the changed bytes and the stubbed callee calls. The stubs mirror
//! `tools/small_diff/stubs.py` exactly.
#![allow(dead_code)]
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Addresses at or above this stand in for the original's stack locals (ignored in diffs).
pub const STACK_STAND_IN: u32 = 0xfff0_0000;
pub const STACK_LOW: u32 = 0x7ef0_0000;
pub const STACK_TOP: u32 = 0x7f00_0000;
pub const HEAP_BASE: u32 = 0x0a00_0000;
pub const SCRATCH: u32 = 0x0b00_0000;
pub const LOADED_RECORD: u32 = SCRATCH;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub va: u32,
    pub ret: u32,
    pub args: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct GoldenCase {
    pub name: String,
    pub va: u32,
    pub stack: Vec<u32>,
    pub regs: HashMap<String, u32>,
    pub init: Vec<(u32, Vec<u8>)>,
    pub calls: Vec<Call>,
    pub ret: [u32; 3],
    pub changes: Vec<(u32, Vec<u8>)>,
}

impl GoldenCase {
    pub fn reg(&self, name: &str) -> u32 {
        self.regs.get(name).copied().unwrap_or(0)
    }
    pub fn arg(&self, index: usize) -> u32 {
        self.stack.get(index).copied().unwrap_or(0)
    }
}

fn hex_words(line: &str) -> Vec<u32> {
    line.split_whitespace().map(|w| u32::from_str_radix(w, 16).expect("hex word")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2).map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).expect("hex byte")).collect()
}

pub fn parse(text: &str) -> Vec<GoldenCase> {
    let mut cases = Vec::new();
    let mut current: Option<GoldenCase> = None;
    for line in text.lines() {
        let (key, rest) = line.split_once(' ').unwrap_or((line, ""));
        match key {
            "case" => {
                let mut parts = rest.split_whitespace();
                let name = parts.next().expect("name").to_string();
                let va = u32::from_str_radix(parts.next().expect("va").trim_start_matches("0x"), 16).expect("va");
                current = Some(GoldenCase { name, va, stack: vec![], regs: HashMap::new(), init: vec![], calls: vec![], ret: [0; 3], changes: vec![] });
            }
            "stack" => current.as_mut().expect("case").stack = hex_words(rest),
            "regs" => {
                for pair in rest.split_whitespace() {
                    let (name, value) = pair.split_once('=').expect("reg");
                    current.as_mut().expect("case").regs.insert(name.to_string(), u32::from_str_radix(value, 16).expect("reg value"));
                }
            }
            "init" | "change" => {
                let (address, data) = rest.split_once(' ').expect("run");
                let run = (u32::from_str_radix(address, 16).expect("address"), unhex(data));
                let case = current.as_mut().expect("case");
                if key == "init" { case.init.push(run) } else { case.changes.push(run) }
            }
            "call" => {
                let words = hex_words(rest);
                current.as_mut().expect("case").calls.push(Call { va: words[0], ret: words[1], args: words[2..].to_vec() });
            }
            "ret" => {
                let words = hex_words(rest);
                current.as_mut().expect("case").ret = [words[0], words[1], words[2]];
            }
            "end" => cases.push(current.take().expect("case")),
            _ => {}
        }
    }
    cases
}

/// Memory plus stub bookkeeping of one replayed case.
pub struct GoldenWorld {
    bytes: HashMap<u32, u8>,
    initial: HashMap<u32, u8>,
    written: BTreeMap<u32, ()>,
    pub calls: Vec<Call>,
    heap: u32,
}

impl GoldenWorld {
    pub fn new(case: &GoldenCase) -> Self {
        let mut bytes = HashMap::new();
        for (address, run) in &case.init {
            for (i, byte) in run.iter().enumerate() {
                bytes.insert(address.wrapping_add(i as u32), *byte);
            }
        }
        Self { initial: bytes.clone(), bytes, written: BTreeMap::new(), calls: vec![], heap: HEAP_BASE }
    }
    pub fn read8(&self, address: u32) -> u8 {
        self.bytes.get(&address).copied().unwrap_or(0)
    }
    pub fn write8(&mut self, address: u32, value: u8) {
        self.written.insert(address, ());
        self.bytes.insert(address, value);
    }
    pub fn read32(&self, address: u32) -> u32 {
        u32::from_le_bytes(core::array::from_fn(|i| self.read8(address.wrapping_add(i as u32))))
    }
    pub fn cstring(&self, address: u32) -> Vec<u8> {
        let mut out = Vec::new();
        while out.len() < 4096 {
            let byte = self.read8(address.wrapping_add(out.len() as u32));
            if byte == 0 {
                break;
            }
            out.push(byte);
        }
        out
    }
    fn byte_sum(&self, address: u32) -> u32 {
        self.cstring(address).iter().fold(0u32, |sum, b| sum.wrapping_add(u32::from(*b)))
    }
    fn log(&mut self, va: u32, ret: u32, args: &[u32]) -> u32 {
        self.calls.push(Call { va, ret, args: args.to_vec() });
        ret
    }
    pub fn stub_alloc(&mut self, size: u32, tag: u32) -> u32 {
        let pointer = self.heap;
        self.heap = self.heap.wrapping_add(size.wrapping_add(15) & !15);
        self.log(0x3dfa0, pointer, &[size, tag])
    }
    pub fn stub_sprintf_path(&mut self, dst: u32, format: u32, args: [u32; 3]) {
        let mut text = b"T:".to_vec();
        text.extend(self.cstring(args[2]));
        text.extend(b".bin\0");
        for (i, byte) in text.iter().enumerate() {
            self.write8(dst.wrapping_add(i as u32), *byte);
        }
        let ret = text.len() as u32 - 1;
        self.log(0x3c85d4, ret, &[dst, format, args[0], args[1], args[2]]);
    }
    pub fn stub_file_size(&mut self, path: u32) -> i32 {
        let ret = (self.byte_sum(path) % 1500).wrapping_sub(100);
        self.log(0x62ca0, ret, &[path]) as i32
    }
    pub fn stub_find_loaded(&mut self, path: u32) -> u32 {
        let ret = if self.byte_sum(path) % 3 == 0 { LOADED_RECORD } else { 0 };
        self.log(0x630c0, ret, &[path])
    }
    pub fn stub_read_file(&mut self, path: u32, dst: u32, size: u32) {
        let seed = self.byte_sum(path);
        for i in 0..size.min(0x4300) {
            self.write8(dst.wrapping_add(i), (seed.wrapping_add(i.wrapping_mul(7)) & 0xff) as u8);
        }
        self.log(0x62d20, 0, &[path, dst, size]);
    }
    pub fn stub_preload(&mut self, path: u32, flags: u32) {
        self.log(0x639e0, 0, &[path, flags]);
    }
    pub fn stub_language_gate(&mut self, language: u32) -> u32 {
        self.log(0x1ceb00, language % 3, &[language])
    }
    pub fn stub_isctype(&mut self, ch: u32, mask: u32) -> u32 {
        let ret = if matches!(ch, 9..=13 | 32) && mask & 8 != 0 { 8 } else { 0 };
        self.log(0x3c9c4e, ret, &[ch, mask])
    }
    pub fn stub_build_id_table(&mut self, table: u32, selector: u32, flag: u32) {
        self.log(0x46030, 0, &[flag, table, selector]);
    }
    pub fn stub_widget_changed(&mut self, this: u32) {
        self.log(0x746d0, 0, &[this]);
    }
    pub fn stub_utf8_to_latin1(&mut self, source: &[u8], dst: u32) {
        let end = source.iter().position(|b| *b == 0).unwrap_or(source.len());
        for (i, byte) in source[..end].iter().chain(std::iter::once(&0u8)).enumerate() {
            self.write8(dst.wrapping_add(i as u32), *byte);
        }
        self.log(0x150e60, 0, &[0, dst]);
    }
    pub fn stub_entry_flag_1640(&mut self) -> u32 {
        self.log(0x2d1640, 1, &[])
    }
    pub fn stub_entry_flag_1700(&mut self, index: u32) -> u32 {
        self.log(0x2d1700, index & 1, &[index])
    }
    pub fn stub_page_item(&mut self, page: u32, index: u32) -> u32 {
        self.log(0x71940, SCRATCH + 0x800 + (index & 0xf) * 0x40, &[page, index])
    }

    // ---- text entry drawing callees (T1956); mirror `DRAW_STUBS` in tools/small_diff/stubs.py ----
    pub fn stub_fill_rect(&mut self, args: [u32; 5]) {
        self.log(0x1efb0, 0, &args);
    }
    pub fn stub_text_width(&mut self, text: u32) -> f32 {
        self.log(0x405c0, 0, &[text]);
        (fnv(&[text]) & 0x3ff) as f32 / 4.0
    }
    pub fn stub_pulse_colors(&mut self, style: u32, phase: u32) -> (u32, u32) {
        self.log(0x75f60, 0, &[style, phase, STACK_TOP - 0x1100, STACK_TOP - 0x1104]);
        (fnv(&[style, phase, 1]), fnv(&[style, phase, 2]))
    }
    pub fn stub_draw_two(&mut self, args: [u32; 5]) {
        self.log(0x41e30, 0, &args);
    }
    pub fn stub_draw_one(&mut self, args: [u32; 4]) {
        self.log(0x41dc0, 0, &args);
    }
    pub fn stub_modulate(&mut self, color: u32, alpha: u32) -> u32 {
        let ret = fnv(&[color, alpha, 3]);
        self.log(0x154fe0, ret, &[color, alpha])
    }
    pub fn stub_sprintf_one(&mut self, dst: u32, format: u32, value: u32) {
        self.log(0x3c85d4, 0, &[dst, format, value]);
    }

    /// Bytes whose value differs from the start of the case, ignoring the stack stand-in.
    pub fn changes(&self) -> BTreeMap<u32, u8> {
        self.written
            .keys()
            .filter(|a| **a < STACK_STAND_IN && !(STACK_LOW..STACK_TOP).contains(*a))
            .filter_map(|a| {
                let now = self.read8(*a);
                (now != self.initial.get(a).copied().unwrap_or(0)).then_some((*a, now))
            })
            .collect()
    }
}

fn is_stack_arg(value: u32) -> bool {
    (STACK_LOW..STACK_TOP).contains(&value)
}

fn expected_changes(case: &GoldenCase) -> BTreeMap<u32, u8> {
    let mut map = BTreeMap::new();
    for (address, run) in &case.changes {
        for (i, byte) in run.iter().enumerate() {
            map.insert(address.wrapping_add(i as u32), *byte);
        }
    }
    map
}

/// Compares one replayed case; `eax` is `None` for void functions. Returns a message on the
/// first mismatch. Expected call arguments inside the original's stack are wildcards.
pub fn compare(case: &GoldenCase, world: &GoldenWorld, eax: Option<u32>) -> Result<(), String> {
    if let Some(eax) = eax
        && eax != case.ret[0]
    {
        return Err(format!("eax {eax:#x} != expected {:#x}", case.ret[0]));
    }
    if world.calls.len() != case.calls.len() {
        return Err(format!("{} calls != expected {}: {:?} vs {:?}", world.calls.len(), case.calls.len(), world.calls, case.calls));
    }
    for (got, want) in world.calls.iter().zip(&case.calls) {
        let args_match = got.args.len() == want.args.len()
            && got.args.iter().zip(&want.args).all(|(g, w)| is_stack_arg(*w) || g == w);
        if got.va != want.va || got.ret != want.ret || !args_match {
            return Err(format!("call {got:x?} != expected {want:x?}"));
        }
    }
    let (got, want) = (world.changes(), expected_changes(case));
    if got != want {
        let missing: Vec<_> = want.iter().filter(|(a, v)| got.get(a) != Some(v)).take(4).collect();
        let extra: Vec<_> = got.iter().filter(|(a, _)| !want.contains_key(a)).take(4).collect();
        return Err(format!("changes differ: expected-but-wrong {missing:x?} unexpected {extra:x?}"));
    }
    Ok(())
}

pub fn golden_dir() -> PathBuf {
    std::env::var("SMALL_DIFF_DIR").map_or_else(
        |_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/small-diff"),
        PathBuf::from,
    )
}

/// Replays every case of `system/NAME.txt` through `run`, which returns `Some(eax)` or `None`
/// for void functions. `SMALL_DIFF_ONLY=a,b` limits the run. Panics with a summary when any case
/// differs or a golden file is missing.
pub fn replay_all<W>(
    system: &str,
    names: &[&str],
    make: impl Fn(GoldenWorld) -> W,
    world_of: impl Fn(&mut W) -> &mut GoldenWorld,
    run: impl Fn(&GoldenCase, &mut W) -> Option<u32>,
) {
    let only = std::env::var("SMALL_DIFF_ONLY").unwrap_or_default();
    let mut report = Vec::new();
    let mut failures = 0;
    for name in names {
        if !only.is_empty() && !only.split(',').any(|o| o == *name) {
            continue;
        }
        let path = golden_dir().join(system).join(format!("{name}.txt"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!("missing golden {} ({e}); run python -m tools.small_diff --system {system}", path.display())
        });
        let cases = parse(&text);
        assert!(!cases.is_empty(), "no cases in {}", path.display());
        let (mut ok, mut bad) = (0, 0);
        for case in &cases {
            let mut w = make(GoldenWorld::new(case));
            let eax = run(case, &mut w);
            match compare(case, world_of(&mut w), eax) {
                Ok(()) => ok += 1,
                Err(message) => {
                    bad += 1;
                    if bad <= 2 {
                        report.push(format!("{name}: case stack={:x?} regs={:x?}: {message}", case.stack, case.regs));
                    }
                }
            }
        }
        failures += bad;
        eprintln!("{name}: {ok} ok, {bad} differ");
    }
    assert!(failures == 0, "{failures} cases differ:\n{}", report.join("\n"));
}

/// 32-bit FNV-1a over little-endian words; twin of `fnv` in tools/small_diff/stubs.py.
pub fn fnv(words: &[u32]) -> u32 {
    let mut h: u32 = 2166136261;
    for word in words {
        for byte in word.to_le_bytes() {
            h = (h ^ u32::from(byte)).wrapping_mul(16777619);
        }
    }
    h
}
