// SPDX-License-Identifier: GPL-3.0-or-later
//! T1900 slice B Tier 1: the 17 multiplayer / team pickup ports against the PRIVATE LIFTED
//! ORIGINAL. `build.rs` + `t1900_lifted.rs` extract the lifted functions into OUT_DIR at build
//! time (gitignored source, nothing copied into tracked files) and `t1900_run` runs them on the C
//! arena while the Rust port runs on an identical Rust arena. Every nested guest call lands in the
//! shared `recomp_lookup` stub of c/shim.c and in `Stub` below. Both record the full call
//! (target, callee ESP, return address, every cdecl argument word, the entry registers a callee
//! really takes) and return the same scripted EAX/ECX/EDX, ST0 and seeded memory writes.
//! Compared per case: registers (policy in `Regs`), the call trace, the pool, the argument frame
//! above ESP and the global ranges in `GLOBALS`. Dead stack contents below ESP are zeroed in both
//! arenas before a case so that reads of uninitialised locals agree, and are not compared after.
//! Without the lifted tree the file compiles to nothing.
#![cfg(t1900_lifted)]
use super::*;

use std::collections::BTreeSet;
use tsfp_engine_object::closure::callee::{Callee, Cpu};
use tsfp_gameplay::t1702_party as party;
use tsfp_gameplay::t1900_multiplayer as mp;

unsafe extern "C" {
    fn t1900_run(va: u32, regs: *mut u32) -> i32;
    fn t1900_extra_argc(va: u32) -> i32;
    fn t1900_trace_entry(index: u32) -> *const u32;
    fn t1900_fp_set(bits: *const u32, n: u32);
    fn shim_script(words: *const u32, entries: u32);
    fn shim_script_writes(words: *const u32, entries: u32);
    fn shim_trace_len() -> u32;
}

const SEEDS: usize = 3000;
const POOL: u32 = 0x0090_0000;
const POOL_LEN: u32 = 0x3_0000;
const ARG_FRAME: u32 = 0x40;
const STACK_BELOW: u32 = 0x1000;
/// Global ranges every case clears first and compares afterwards.
const GLOBALS: &[(u32, u32)] = &[
    (0x200, 0x100), (0x4D1F00, 0x2100), (0x475C00, 0x200), (0x4E7900, 0x80), (0x4E8C00, 0x80),
    (0x715F00, 0x40), (0x74C380, 0x60), (0x790900, 0x80), (0x7A2900, 0x80), (0x7A2BE4, 0x200),
    (0x7AD280, 0x20), (0x7B0C40, 0x10), (0x7330D0, 0x10), (0x7331A8, 0x6000), (0x7DE300, 0x200), (0x7356C0, 0x40),
];

/// Exit register comparison policy (bit i = register i compared, order EAX ECX EDX EBX ESI EDI
/// EBP ESP). Caller-saved residue of a void function is dead by the cdecl ABI.
#[derive(Clone, Copy)]
struct Regs(u8);
impl Regs {
    const ALL: Regs = Regs(0xFF);
    const VOID: Regs = Regs(0xF8);
    const RET: Regs = Regs(0xF9);
}

/// Registers of a callee entry that are part of its contract (bit i = register i).
fn keep_mask(va: u32) -> u8 {
    match va {
        0xC09B0 => 0b001_0011, // EAX base, ECX player record, ESI flag object
        _ => 0,
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Call {
    va: u32,
    esp: u32,
    ret: u32,
    args: Vec<u32>,
    regs: [u32; 7],
}

#[derive(Clone, Default)]
struct Script {
    rows: Vec<[u32; 3]>,
    /// ST0 (f32 bits) a call to 00238580 leaves, per call index.
    fp: Vec<u32>,
    /// {call index, address, value}.
    writes: Vec<[u32; 3]>,
}

struct Stub {
    script: Script,
    trace: Vec<Call>,
    st0: f64,
}
fn rd(v: &View, a: u32) -> u32 {
    u32::from_le_bytes(std::array::from_fn(|i| v.byte(a.wrapping_add(i as u32))))
}
fn wr(v: &mut View, a: u32, value: u32) {
    for (i, b) in value.to_le_bytes().into_iter().enumerate() {
        v.set(a.wrapping_add(i as u32), b);
    }
}
fn mask_regs(va: u32, regs: [u32; 7]) -> [u32; 7] {
    let keep = keep_mask(va);
    std::array::from_fn(|i| if keep >> i & 1 == 1 { regs[i] } else { 0 })
}
impl Callee<View> for Stub {
    fn call(&mut self, m: &mut View, target: u32, cpu: &mut Cpu) {
        let index = self.trace.len();
        let argc = unsafe { t1900_extra_argc(target) };
        assert!(argc >= 0, "unknown callee {target:x}");
        let args = (0..argc as u32).map(|a| rd(m, cpu.esp.wrapping_add(4 + 4 * a))).collect();
        let regs = [cpu.eax, cpu.ecx, cpu.edx, cpu.ebx, cpu.esi, cpu.edi, cpu.ebp];
        self.trace.push(Call { va: target, esp: cpu.esp, ret: rd(m, cpu.esp), args, regs: mask_regs(target, regs) });
        for w in self.script.writes.iter().filter(|w| w[0] as usize == index) {
            wr(m, w[1], w[2]);
        }
        let [a, c, d] = self.script.rows.get(index).copied().unwrap_or([0; 3]);
        (cpu.eax, cpu.ecx, cpu.edx) = (a, c, d);
        if target == 0x238580 {
            self.st0 = f64::from(f32::from_bits(self.script.fp.get(index).copied().unwrap_or(0)));
        }
    }
}
impl mp::X87Callee<View> for Stub {
    fn st0(&self) -> f64 {
        self.st0
    }
}

struct Case {
    va: u32,
    args: Vec<u32>,
    regs: [u32; 7],
    script: Script,
}
fn case(r: &mut Rng, va: u32, args: Vec<u32>) -> Case {
    Case { va, args, regs: std::array::from_fn(|_| r.next()), script: Script::default() }
}

/// Bump allocator for structures inside the pool.
struct Alloc(u32);
impl Alloc {
    fn take(&mut self, size: u32) -> u32 {
        let at = self.0;
        self.0 += (size + 15) & !15;
        assert!(self.0 < POOL + POOL_LEN, "pool exhausted");
        at
    }
}
fn clear(p: &mut Pair) {
    for (start, len) in GLOBALS.iter().copied().chain([(POOL, POOL_LEN), (STACK - STACK_BELOW, STACK_BELOW + 0x100)]) {
        unsafe {
            std::ptr::write_bytes(p.c.add(start as usize), 0, len as usize);
            std::ptr::write_bytes(p.rust.add(start as usize), 0, len as usize);
        }
    }
    p.log.clear();
}
const FLOATS: [u32; 16] = [
    0, 0x8000_0000, 0x3F80_0000, 0xBF80_0000, 0x3F00_0000, 0x42C8_0000, 0x43A0_0000, 0x7FC0_0000,
    0x7F80_0000, 0xFF80_0000, 0x4F00_0000, 0xCF00_0000, 0x4E99_6000, 0x0000_0001, 0x4B80_0000, 0x3DCC_CCCD,
];
fn fl(r: &mut Rng) -> u32 {
    if r.below(2) == 0 { FLOATS[r.below(16) as usize] } else { (((r.next() % 4000) as i32 - 2000) as f32 / 8.0).to_bits() }
}
fn pick(r: &mut Rng, options: &[u32]) -> u32 {
    options[r.below(options.len() as u32) as usize]
}
fn chance(r: &mut Rng, percent: u32) -> bool {
    r.below(100) < percent
}
fn put8g(p: &mut Pair, a: u32, v: u8) {
    p.put8(a, v);
}
fn put16(p: &mut Pair, a: u32, v: u16) {
    p.put8(a, v as u8);
    p.put8(a + 1, (v >> 8) as u8);
}

/// Runs the lifted original on the C arena. Returns final registers and the call trace.
fn run_c(p: &mut Pair, c: &Case) -> ([u32; 8], Vec<Call>) {
    p.put32(STACK, 0);
    for (i, a) in c.args.iter().enumerate() {
        p.put32(STACK + 4 + 4 * i as u32, *a);
    }
    let flat: Vec<u32> = c.script.rows.iter().flatten().copied().collect();
    unsafe {
        shim_script(flat.as_ptr(), c.script.rows.len() as u32);
        let writes: Vec<u32> = c.script.writes.iter().flatten().copied().collect();
        shim_script_writes(writes.as_ptr(), c.script.writes.len() as u32);
        t1900_fp_set(c.script.fp.as_ptr(), c.script.fp.len() as u32);
    }
    let mut regs = [c.regs[0], c.regs[1], c.regs[2], c.regs[3], c.regs[4], c.regs[5], c.regs[6], STACK];
    assert_eq!(unsafe { t1900_run(c.va, regs.as_mut_ptr()) }, 1, "unknown root");
    let trace = (0..unsafe { shim_trace_len() })
        .map(|i| {
            let e: &[u32; 24] = unsafe { &*(t1900_trace_entry(i) as *const [u32; 24]) };
            let regs: [u32; 7] = std::array::from_fn(|k| e[14 + k]);
            Call { va: e[0], esp: e[1], ret: e[2], args: e[4..4 + e[3] as usize].to_vec(), regs: mask_regs(e[0], regs) }
        })
        .collect();
    (regs, trace)
}

fn slice(base: *mut u8, start: u32, len: u32) -> &'static [u8] {
    unsafe { std::slice::from_raw_parts(base.add(start as usize), len as usize) }
}
fn mem_diff(p: &Pair) -> Option<String> {
    let ranges = GLOBALS.iter().copied().chain([(POOL, POOL_LEN), (STACK, ARG_FRAME)]);
    for (start, len) in ranges {
        let (c, rust) = (slice(p.c, start, len), slice(p.rust, start, len));
        if let Some(off) = (0..len as usize).find(|&i| c[i] != rust[i]) {
            return Some(format!("memory {:08x}: C {:02x} Rust {:02x}", start as usize + off, c[off], rust[off]));
        }
    }
    None
}

#[derive(Default)]
struct Stats {
    shapes: BTreeSet<Vec<u32>>,
    results: BTreeSet<u32>,
    seen: std::collections::BTreeMap<u32, usize>,
    changed: usize,
}

/// Differential over `SEEDS` cases. `min_shapes` is the least number of distinct call sequences
/// the generator must reach and `expect` the callees that must each be reached at least 20 times.
fn drive(
    seed: u64,
    mode: Regs,
    expect: &[u32],
    min_shapes: usize,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> Case,
    port: impl Fn(&mut Stub, &mut View, &mut Cpu),
) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut r = Rng(seed);
    let mut stats = Stats::default();
    let mut survey: std::collections::BTreeMap<(u32, u32), BTreeSet<i32>> = Default::default();
    let mut survey_args: std::collections::BTreeMap<(u32, u32, usize), BTreeSet<i32>> = Default::default();
    for index in 0..SEEDS {
        clear(p);
        let c = build(&mut r, p);
        p.log.clear();
        if std::env::var_os("T1900_TRACE").is_some() {
            eprintln!("case {index} root {:x} args {:x?} regs {:x?} rows {:x?}", c.va, c.args, c.regs, &c.script.rows[..c.script.rows.len().min(3)]);
        }
        let (cregs, ctrace) = run_c(p, &c);
        let mut stub = Stub { script: c.script.clone(), trace: vec![], st0: 0.0 };
        let mut cpu = Cpu {
            eax: c.regs[0], ecx: c.regs[1], edx: c.regs[2], ebx: c.regs[3],
            esi: c.regs[4], edi: c.regs[5], ebp: c.regs[6], esp: STACK,
        };
        let mut view = View(p.rust);
        port(&mut stub, &mut view, &mut cpu);
        let rregs = [cpu.eax, cpu.ecx, cpu.edx, cpu.ebx, cpu.esi, cpu.edi, cpu.ebp, cpu.esp.wrapping_add(4)];
        let mut why = vec![];
        if (0..8).any(|i| mode.0 >> i & 1 == 1 && cregs[i] != rregs[i]) {
            why.push(format!("regs (mask {:#x}) C {cregs:x?} Rust {rregs:x?}", mode.0));
        }
        if ctrace != stub.trace {
            let at = ctrace.iter().zip(&stub.trace).position(|(a, b)| a != b).unwrap_or(ctrace.len().min(stub.trace.len()));
            why.push(format!(
                "trace differs at call {at} (C {} calls, Rust {}): C {:x?} Rust {:x?}",
                ctrace.len(), stub.trace.len(), ctrace.get(at), stub.trace.get(at)
            ));
            let brief = |t: &[Call]| t.iter().map(|e| format!("{:x}@{:x}/{:x}{:x?}", e.va, e.esp, e.ret, e.args)).collect::<Vec<_>>().join(" ");
            why.push(format!("C trace: {}\n Rust trace: {}", brief(&ctrace), brief(&stub.trace)));
        }
        why.extend(mem_diff(p));
        if std::env::var_os("T1900_SURVEY").is_some() {
            if ctrace.len() == stub.trace.len() && ctrace.iter().zip(&stub.trace).all(|(a, b)| a.va == b.va && a.ret == b.ret) {
                for (a, b) in ctrace.iter().zip(&stub.trace) {
                    survey.entry((a.va, a.ret)).or_insert_with(BTreeSet::new).insert(a.esp.wrapping_sub(b.esp) as i32);
                    for (k, (x, y)) in a.args.iter().zip(&b.args).enumerate() {
                        if x != y {
                            survey_args.entry((a.va, a.ret, k)).or_insert_with(BTreeSet::new).insert(x.wrapping_sub(a.esp) as i32 - y.wrapping_sub(b.esp) as i32);
                        }
                    }
                }
            }
            continue;
        }
        assert!(
            why.is_empty(),
            "root {:08X} seed {seed:#x} case {index}: args {:x?} regs {:x?} {why:?}",
            c.va, c.args, c.regs
        );
        stats.shapes.insert(ctrace.iter().map(|e| e.va).collect());
        stats.results.insert(cregs[0]);
        for e in &ctrace {
            *stats.seen.entry(e.va).or_default() += 1;
        }
        stats.changed += usize::from(cregs[0] != c.regs[0]);
    }
    if std::env::var_os("T1900_SURVEY").is_some() {
        eprintln!("SURVEY esp delta (C - Rust) per (callee, ret): {:x?}\nSURVEY arg offset delta: {:x?}", survey, survey_args);
        return;
    }
    for callee in expect {
        assert!(stats.seen.get(callee).copied().unwrap_or(0) >= 20, "callee {callee:x} reached too rarely: {:x?}", stats.seen);
    }
    assert!(stats.shapes.len() >= min_shapes, "weak coverage: {} call shapes, need {min_shapes}: {:x?}", stats.shapes.len(), stats.shapes);
    eprintln!(
        "t1900 root {seed:#x}: {SEEDS} cases, {} call shapes, {} distinct EAX results, callees {:x?}",
        stats.shapes.len(), stats.results.len(), stats.seen
    );
}

macro_rules! t1900_test {
    ($name:ident, $seed:expr, $mode:expr, $expect:expr, $shapes:expr, $build:expr, $port:expr) => {
        #[test]
        fn $name() {
            drive($seed, $mode, $expect, $shapes, $build, $port);
        }
    };
}

fn arg(v: &View, c: &Cpu, i: u32) -> u32 {
    rd(v, c.esp.wrapping_add(4 + 4 * i))
}

// ---------------------------------------------------------------------------------------------
// Object table leaves 00274330 / 00274350 / 00274380 / 002744F0 / 00274570
// ---------------------------------------------------------------------------------------------
/// Fills object table entries 0..8: object pointers (some null), records, flags.
fn object_table(r: &mut Rng, p: &mut Pair) {
    let mut al = Alloc(POOL + 0x100);
    for i in 0..8u32 {
        let object = if chance(r, 90) { al.take(0x100) } else { r.next() & 0x00FF_FFF0 };
        p.put32(mp::OBJECT_TABLE + i * 8, object);
        if object >= POOL && object < POOL + POOL_LEN {
            let record = al.take(0x100);
            p.put32(object + 0x7C, record);
            p.put32(record + 0xA0, r.word(&[4, 0xFFFF_FFFB, 0xFF]));
            p.put32(object + 0x28, r.word(&[0x80, 0x7F, 0xFFFF_FF7F, 0x100]));
        }
    }
}
fn index_arg(r: &mut Rng) -> u32 {
    let (a, b) = (r.below(8), r.below(8));
    pick(r, &[0, 1, 2, 3, 4, 5, 6, 7, a, b, 8, 0xFFFF_FFFF, 0x8000_0000])
}
macro_rules! leaf_test {
    ($name:ident, $va:expr, $port:expr) => {
        t1900_test!($name, $va as u64, Regs::RET, &[], 1,
            |r, p| {
                object_table(r, p);
                let index = index_arg(r);
                case(r, $va, vec![index])
            },
            |_, v, c| c.eax = $port(v, arg(v, c, 0)));
    };
}
leaf_test!(lifted_00274330, 0x274330u32, |v: &View, i| mp::slot_record_flag_a0_bit2(v, i));
leaf_test!(lifted_00274350, 0x274350u32, |v: &View, i| mp::opposing_record_flag_a0_bit2(v, i));
leaf_test!(lifted_00274380, 0x274380u32, |v: &View, i| mp::slot_object_flag_28_bit7_clear(v, i));
leaf_test!(lifted_002744f0, 0x2744F0u32, |v: &View, i| mp::opposing_side_object(v, i));

t1900_test!(lifted_00274570, 0x274570, Regs::ALL, &[0x274510], 1,
    |r, _| {
        let any = r.next();
        let side = pick(r, &[0, 1, 2, 3, 0xFFFF_FFFF, any]);
        let mut c = case(r, 0x274570, vec![side]);
        c.script.rows = vec![[r.next(), r.next(), r.next()]];
        c
    },
    |s, v, c| mp::find_first_for_opposing_side(s, v, c));

// ---- 00274650 allocate side tables ----
t1900_test!(lifted_00274650, 0x274650, Regs::ALL, &[0x3DFA0], 1,
    |r, p| {
        p.put8(mp::SIDE_COUNT_BYTE, [0u8, 1, 2, 4, 8, 0x7F, 0x80, 0xFF, r.next() as u8][r.below(9) as usize]);
        let mut c = case(r, 0x274650, vec![]);
        c.script.rows = vec![[POOL + r.below(0x100) * 16, r.next(), r.next()], [POOL + 0x1000 + r.below(0x100) * 16, r.next(), r.next()]];
        c
    },
    |s, v, c| mp::allocate_side_tables(s, v, c));

// ---- 00275640 side flag refresh ----
t1900_test!(lifted_00275640, 0x275640, Regs::VOID, &[], 1,
    |r, p| {
        object_table(r, p);
        let table = POOL + 0x4000;
        p.put32(mp::SIDE_TABLE_PTR, table);
        p.put8(mp::SIDE_COUNT_BYTE, [0u8, 1, 2, 0xFF][r.below(4) as usize]);
        p.put8(mp::MODE_BYTE, [2u8, 2, 2, 0, 1, 0xA][r.below(6) as usize]);
        p.put32(mp::SIDE_STAMP, r.next());
        for i in 0..9u32 {
            p.put32(table + i * 16 - 8 + 16, r.below(3));
            p.put32(table + i * 16 - 4 + 16, r.next());
        }
        let any = r.below(8);
        let index = pick(r, &[0, 1, 2, 3, 4, 5, 6, 7, any]);
        case(r, 0x275640, vec![index])
    },
    |_, v, c| {
        let index = arg(v, c, 0);
        mp::side_flag_refresh(v, index);
    });

// ---- 00160AC0 named table search ----
t1900_test!(lifted_00160ac0, 0x160AC0, Regs::RET, &[], 1,
    |r, p| {
        let small = r.below(16);
        let count = pick(r, &[0, 1, 2, 5, 12, 0xFFFF_FFFF, 0x8000_0000, small]);
        p.put32(mp::NAMED_TABLE_COUNT, count);
        for i in 0..16u32 {
            let (a, b) = (r.below(8), r.next());
            p.put32(mp::NAMED_TABLE + i * 8, pick(r, &[0, 1, 2, 3, 4, a, b]));
        }
        let any = r.next();
        let key = pick(r, &[0, 1, 2, 3, 4, 5, 6, 7, any]);
        case(r, 0x160AC0, vec![key])
    },
    |_, v, c| c.eax = mp::find_named_entry(v, arg(v, c, 0)));

// ---------------------------------------------------------------------------------------------
// Party mode: CB240, 2E4470, D8B10, D8A00
// ---------------------------------------------------------------------------------------------
fn party_mode(r: &mut Rng) -> u8 {
    let any = r.next();
    if chance(r, 85) { 0xA } else { pick(r, &[0, 2, 6, any & 0xFF]) as u8 }
}
fn player_count(r: &mut Rng) -> u32 {
    pick(r, &[0xFFFF_FFFF, 0, 1, 2, 2, 2, 1, 3, 4, 0x8000_0000])
}
/// Player array at `[7B0C48]` with `n` entries (stride 0x1584): +4 handle, +8 view object,
/// +0x14 player object, whose +0x7C record carries the team (+0x84) and the item list (+0x534).
fn player_array(r: &mut Rng, p: &mut Pair, al: &mut Alloc, n: u32) -> (u32, Vec<u32>) {
    let array = al.take(0x5800);
    p.put32(0x7B0C48, array);
    p.put32(array + 0xC, r.next());
    let mut records = vec![];
    for i in 0..n.max(2) {
        let entry = array + i * 0x1584;
        p.put32(entry + 4, r.next());
        let view = if i == 1 && chance(r, 40) { 0 } else { al.take(0x240) };
        p.put32(entry + 8, view);
        if view != 0 {
            p.put32(view + 0x21C, r.next());
        }
        let object = al.take(0x100);
        let record = al.take(0x700);
        p.put32(entry + 0x14, object);
        p.put32(object + 0x7C, record);
        let any = r.next();
        p.put32(record + 0x84, pick(r, &[0, 1, 2, 3, 4, 5, any]));
        records.push(record);
    }
    p.put32(array + 0x1590, r.next());
    (array, records)
}

t1900_test!(lifted_000cb240, 0xCB240, Regs::VOID, &[0x25A570], 3,
    |r, p| {
        let mut al = Alloc(POOL + 0x100);
        p.put8(mp::MODE_BYTE, party_mode(r));
        let n = player_count(r);
        p.put32(mp::PLAYER_COUNT, n);
        let (array, _) = player_array(r, p, &mut al, 2);
        // The party view objects: entry 0 at +8, entry 1 at +0x158C.
        let first = al.take(0x240);
        p.put32(array + 8, if chance(r, 95) { first } else { 0 });
        p.put32(first + 0x21C, r.next());
        p.put32(array + 0x1590, r.next());
        p.put32(mp::PARTY_FLAG, r.next());
        let mut c = case(r, 0xCB240, vec![]);
        c.script.rows = vec![[r.next(), r.next(), r.next()], [r.next(), r.next(), r.next()]];
        c
    },
    |s, v, c| mp::leave_party_mode(s, v, c));

t1900_test!(lifted_002e4470, 0x2E4470, Regs::RET, &[0xCB0F0], 2,
    |r, p| {
        p.put8(mp::MODE_BYTE, party_mode(r));
        p.put32(mp::PLAYER_COUNT, player_count(r));
        p.put32(mp::MODE_FLAGS, r.word(&[0x200, 0x100, 0xFFFF_FDFF, 0x300]));
        let mut c = case(r, 0x2E4470, vec![]);
        let any = r.next();
        c.script.rows = vec![[pick(r, &[0, 1, any]), r.next(), r.next()]];
        c
    },
    |s, v, c| mp::party_mode_active(s, v, c));

/// Linked list of items for a player at `[player object + 0x7C] + 0x534` (node +0x10 object,
/// +0x18 next). Each object has a u16 type at +0x94 and a 0xD8 byte record at `[object + 0x7C]`.
fn item_list(r: &mut Rng, p: &mut Pair, al: &mut Alloc, record: u32, length: u32) {
    let mut next = 0;
    for _ in 0..length {
        let node = al.take(0x20);
        let object = al.take(0x100);
        let data = al.take(0xE0);
        for word in 0..0xD8 / 4 {
            p.put32(data + 4 * word, r.next());
        }
        put16(p, object + 0x94, r.next() as u16);
        p.put32(object + 0x7C, data);
        p.put32(node + 0x10, object);
        p.put32(node + 0x18, next);
        next = node;
    }
    p.put32(record + 0x534, next);
}

t1900_test!(lifted_000d8b10, 0xD8B10, Regs::VOID, &[], 1,
    |r, p| {
        let mut al = Alloc(POOL + 0x100);
        p.put8(mp::MODE_BYTE, party_mode(r));
        let n = pick(r, &[0xFFFF_FFFF, 0, 1, 2, 2, 2]);
        p.put32(mp::PLAYER_COUNT, n);
        let (_, records) = player_array(r, p, &mut al, 2);
        for record in records {
            let length = r.below(4);
            item_list(r, p, &mut al, record, length);
        }
        for word in 0..0x40u32 {
            p.put32(mp::SNAPSHOT_IDS + 4 * word, r.next());
        }
        case(r, 0xD8B10, vec![])
    },
    |_, v, _| mp::snapshot_party(v));

t1900_test!(lifted_000d8a00, 0xD8A00, Regs::VOID, &[0xC1950, 0xDB420], 3,
    |r, p| {
        let mut al = Alloc(POOL + 0x100);
        p.put8(mp::MODE_BYTE, party_mode(r));
        let n = pick(r, &[0xFFFF_FFFF, 0, 1, 2, 2, 2]);
        p.put32(mp::PLAYER_COUNT, n);
        player_array(r, p, &mut al, 2);
        // Snapshot: per player a zero-terminated id list (0..3 entries) and 0xD8 byte records.
        for player in 0..2u32 {
            let length = r.below(4);
            for j in 0..length {
                p.put32(mp::SNAPSHOT_IDS + 4 * (player * 0x14 + j), 1 + r.below(0x400));
            }
        }
        for word in 0..0x10E0 * 2 / 4 {
            p.put32(mp::SNAPSHOT_RECORDS + 4 * word, r.next());
        }
        let mut c = case(r, 0xD8A00, vec![]);
        for _ in 0..8 {
            let object = al.take(0x100);
            let data = al.take(0xE0);
            p.put32(object + 0x7C, data);
            c.script.rows.push([object, r.next(), r.next()]);
            c.script.rows.push([r.next(), r.next(), r.next()]);
        }
        c
    },
    |s, v, c| mp::restore_party(s, v, c));

fn c_word(p: &Pair, address: u32) -> u32 {
    u32::from_le_bytes(std::array::from_fn(|i| {
        p.get(false, address.wrapping_add(i as u32))
    }))
}

fn write_party_id(p: &mut Pair, address: u32, id: u16) {
    p.put8(address, id as u8);
    p.put8(address + 1, (id >> 8) as u8);
}

#[test]
fn t1702_native_party_snapshot_tier1() {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut rng = Rng(0x1702_51a9);
    for case_index in 0..3000 {
        clear(p);
        let mode = pick(&mut rng, &[0x0a, 0x0a, 0, 2]) as u8;
        let player_count = pick(&mut rng, &[0, 1, 2, 2]);
        p.put8(mp::MODE_BYTE, mode);
        p.put32(mp::PLAYER_COUNT, player_count);
        p.put32(mp::PLAYER_ARRAY_PTR, 0x50000);

        let mut players = Vec::new();
        for player in 0..player_count {
            let player_object = 0x60000 + player * 0x2000;
            let player_extension = player_object + 0x1000;
            p.put32(0x50000 + 0x14 + player * mp::PLAYER_STRIDE, player_object);
            p.put32(player_object + 0x7c, player_extension);
            let length = rng.below(4) as usize;
            let mut objects = Vec::new();
            let mut next = 0;
            for item in 0..length {
                let object = 0x80000 + player * 0x10000 + item as u32 * 0x400;
                let extension = object + 0x200;
                let mut words = [0u32; party::EXTENSION_RECORD_WORDS];
                for (word, value) in words.iter_mut().enumerate() {
                    *value = rng.next();
                    p.put32(extension + word as u32 * 4, *value);
                }
                let id = if case_index % 13 == 0 && item == 0 {
                    0
                } else {
                    1 + rng.below(u16::MAX as u32) as u16
                };
                write_party_id(p, object + 0x94, id);
                p.put32(object + 0x7c, extension);
                let node = 0x70000 + player * 0x1000 + item as u32 * 0x40;
                p.put32(node + 0x10, object);
                p.put32(node + 0x18, next);
                next = node;
                objects.push(party::PartyObjectSnapshot {
                    id: party::SnapshotObjectId(id),
                    extension: party::ExtensionSnapshot(words),
                });
            }
            // The linked list is built backwards so its head yields the input vector's order.
            p.put32(player_extension + 0x534, next);
            objects.reverse();
            players.push(objects);
        }

        let c = case(&mut rng, 0xD8B10, vec![]);
        let (_, calls) = run_c(p, &c);
        assert!(calls.is_empty(), "snapshot should not call callees");

        let mut snapshot = party::PartySnapshot::empty(
            party::SnapshotGeneration(case_index as u64),
            player_count as usize,
        );
        let captured = snapshot.capture_from(mode, &players);
        assert_eq!(
            captured,
            mode == party::PARTY_MODE_BYTE && player_count != 0
        );
        if !captured {
            continue;
        }
        for (player, objects) in players.iter().enumerate() {
            let base = player * party::SNAPSHOT_SLOTS_PER_PLAYER;
            for (offset, object) in objects.iter().enumerate() {
                let index = base + offset;
                assert_eq!(
                    c_word(p, mp::SNAPSHOT_IDS + index as u32 * 4),
                    u32::from(object.id.0)
                );
                assert_eq!(snapshot.slot(index), Some(object));
                let source = mp::SNAPSHOT_RECORDS + index as u32 * mp::SNAPSHOT_RECORD_SIZE;
                for (word, expected) in object.extension.0.iter().enumerate() {
                    assert_eq!(c_word(p, source + word as u32 * 4), *expected);
                }
            }
            let terminator = base + objects.len();
            assert_eq!(c_word(p, mp::SNAPSHOT_IDS + terminator as u32 * 4), 0);
            assert_eq!(snapshot.slot(terminator), None);
        }
    }
}

#[derive(Default)]
struct NativePartyBackend {
    next_handle: usize,
    owner_lists: Vec<u32>,
    objects: Vec<u32>,
    constructor_args: Vec<party::PartyObjectCreateArgs>,
    copied_extensions: Vec<party::ExtensionSnapshot>,
    inserted: Vec<(usize, u32)>,
}

impl party::PartyRestoreBackend for NativePartyBackend {
    type Object = u32;
    type Error = ();

    fn construct(
        &mut self,
        args: party::PartyObjectCreateArgs,
    ) -> Result<Self::Object, Self::Error> {
        let object = 0x900000 + self.next_handle as u32 * 0x1000;
        self.next_handle += 1;
        self.objects.push(object);
        self.constructor_args.push(args);
        Ok(object)
    }

    fn replace_extension(
        &mut self,
        _object: Self::Object,
        saved: &party::ExtensionSnapshot,
    ) -> Result<(), Self::Error> {
        self.copied_extensions.push(saved.clone());
        Ok(())
    }

    fn insert_owned(&mut self, player: usize, object: Self::Object) -> Result<(), Self::Error> {
        self.inserted.push((player, object));
        Ok(())
    }
}

#[test]
fn t1702_native_party_restore_tier1() {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut rng = Rng(0x1702_d8a00);
    for case_index in 0..3000 {
        clear(p);
        let mode = pick(&mut rng, &[0x0a, 0x0a, 0, 2]) as u8;
        let player_count = pick(&mut rng, &[0, 1, 2, 2]);
        p.put8(mp::MODE_BYTE, mode);
        p.put32(mp::PLAYER_COUNT, player_count);
        p.put32(mp::PLAYER_ARRAY_PTR, 0x50000);
        for index in 0..0x28u32 {
            p.put32(mp::SNAPSHOT_IDS + index * 4, 0);
        }

        let mut owner_lists = Vec::new();
        for player in 0..player_count {
            let player_object = 0x60000 + player * 0x2000;
            let player_extension = player_object + 0x1000;
            let owner_list = player_extension + 0x304;
            owner_lists.push(owner_list);
            p.put32(0x50000 + 0x14 + player * mp::PLAYER_STRIDE, player_object);
            p.put32(player_object + 0x7c, player_extension);
        }

        let mut players = Vec::new();
        for player in 0..player_count {
            let length = rng.below(4) as usize;
            let mut objects = Vec::new();
            for item in 0..length {
                let mut words = [0u32; party::EXTENSION_RECORD_WORDS];
                for (word, value) in words.iter_mut().enumerate() {
                    *value = rng.next();
                    let index = player as usize * party::SNAPSHOT_SLOTS_PER_PLAYER + item;
                    p.put32(
                        mp::SNAPSHOT_RECORDS
                            + index as u32 * mp::SNAPSHOT_RECORD_SIZE
                            + word as u32 * 4,
                        *value,
                    );
                }
                let id = if case_index % 13 == 0 && item == 0 {
                    0
                } else {
                    1 + rng.below(u16::MAX as u32) as u16
                };
                let index = player as usize * party::SNAPSHOT_SLOTS_PER_PLAYER + item;
                p.put32(mp::SNAPSHOT_IDS + index as u32 * 4, u32::from(id));
                objects.push(party::PartyObjectSnapshot {
                    id: party::SnapshotObjectId(id),
                    extension: party::ExtensionSnapshot(words),
                });
            }
            let terminator = player as usize * party::SNAPSHOT_SLOTS_PER_PLAYER + length;
            p.put32(mp::SNAPSHOT_IDS + terminator as u32 * 4, 0);
            players.push(objects);
        }

        // A script returns distinct fresh objects to the lifted constructor and a residue from
        // list insertion. The Rust backend returns those same typed handles without guest calls.
        let mut c = case(&mut rng, 0xD8A00, vec![]);
        let mut next_handle = 0usize;
        for _ in 0..(player_count as usize * 3) {
            let object = 0x900000 + next_handle as u32 * 0x1000;
            p.put32(object + 0x7c, object + 0x500);
            c.script.rows.push([object, rng.next(), rng.next()]);
            c.script.rows.push([rng.next(), rng.next(), rng.next()]);
            next_handle += 1;
        }
        let (_, calls) = run_c(p, &c);

        let mut snapshot = party::PartySnapshot::empty(
            party::SnapshotGeneration(case_index as u64),
            player_count as usize,
        );
        snapshot.capture(party::PARTY_MODE_BYTE, &players);
        let mut backend = NativePartyBackend {
            owner_lists,
            ..Default::default()
        };
        let report = snapshot.restore(mode, &mut backend).unwrap();
        let expected_calls: Vec<u32> = (0..report.restored_objects)
            .flat_map(|_| [0xC1950, 0xDB420])
            .collect();
        assert_eq!(
            calls.iter().map(|call| call.va).collect::<Vec<_>>(),
            if mode == party::PARTY_MODE_BYTE {
                expected_calls
            } else {
                vec![]
            }
        );
        if mode != party::PARTY_MODE_BYTE {
            continue;
        }

        let constructors: Vec<_> = calls.iter().filter(|call| call.va == 0xC1950).collect();
        let inserts: Vec<_> = calls.iter().filter(|call| call.va == 0xDB420).collect();
        assert_eq!(constructors.len(), backend.constructor_args.len());
        assert_eq!(inserts.len(), backend.inserted.len());
        for (call, expected) in constructors.iter().zip(&backend.constructor_args) {
            assert_eq!(call.args.len(), 10);
            assert_eq!(
                [
                    call.args[0],
                    call.args[1],
                    call.args[2],
                    call.args[3],
                    call.args[5],
                    call.args[6],
                    call.args[7],
                    call.args[8],
                    call.args[9]
                ],
                [
                    expected.field_10,
                    expected.field_0c,
                    expected.field_14,
                    expected.field_1c,
                    expected.field_18,
                    expected.field_40,
                    expected.field_44,
                    expected.field_48,
                    expected.field_4c
                ],
            );
            assert_eq!(call.args[4], STACK - 0x0c);
        }
        for (call, (player, object)) in inserts.iter().zip(&backend.inserted) {
            assert_eq!(call.args, [backend.owner_lists[*player], *object]);
        }
        assert_eq!(backend.objects.len(), backend.copied_extensions.len());
        for (object, expected) in backend.objects.iter().zip(&backend.copied_extensions) {
            let start = *object + 0x500;
            for (word, value) in expected.0.iter().enumerate() {
                assert_eq!(c_word(p, start + word as u32 * 4), *value);
            }
        }
        assert_eq!(report.restored_objects, backend.objects.len());
    }
}


// ---- 002F1FC0 / 002F2050 roster scoring ----
fn roster(r: &mut Rng, p: &mut Pair, al: &mut Alloc) {
    p.put32(mp::PLAYER_COUNT, pick(r, &[0xFFFF_FFFF, 0, 1, 2, 3, 4, 2]));
    p.put32(mp::ROSTER_EXTRA_COUNT, pick(r, &[0xFFFF_FFFF, 0, 1, 2, 3]));
    p.put8(mp::MODE_BYTE_2, pick(r, &[0x13, 0x13, 0x13, 0, 1, 0x12]) as u8);
    let base = al.take(0x6000);
    p.put32(mp::ROSTER_PTR, base);
    for i in 0..6u32 {
        let entry = base + i * mp::ROSTER_STRIDE;
        p.put32(entry + 8, r.word(&[0, 1, 2, 3, 4, 5, 0xFFFF_FFFF]));
        put16(p, entry + 0x48, r.next() as u16);
    }
    p.put32(mp::COUNT_THRESHOLD, fl(r));
}
fn roster_script(r: &mut Rng, c: &mut Case, calls: usize) {
    for _ in 0..calls {
        c.script.rows.push([r.word(&[0, 1, 1, 2]), r.next(), r.next()]);
        c.script.fp.push(fl(r));
    }
}

t1900_test!(lifted_002f1fc0, 0x2F1FC0, Regs::RET, &[0x238580, 0xD6C90], 3,
    |r, p| {
        let mut al = Alloc(POOL + 0x100);
        roster(r, p, &mut al);
        let key = r.next();
        let mut c = case(r, 0x2F1FC0, vec![key]);
        roster_script(r, &mut c, 14);
        c
    },
    |s, v, c| mp::count_roster_with_positive_stat(s, v, c));

t1900_test!(lifted_002f2050, 0x2F2050, Regs::VOID, &[0x2F1FC0, 0x69640], 2,
    |r, p| {
        let mut al = Alloc(POOL + 0x100);
        p.put8(mp::MODE_BYTE, r.word(&[0xA, 0, 1, 2, 5]) as u8);
        let mode2 = r.next() as u8;
        p.put8(mp::MODE_BYTE_2, mode2);
        let kind_row = mp::RULE_KIND_TABLE + (u32::from(mode2) << 5);
        p.put32(kind_row, r.word(&[0, 1, 2, 3]));
        p.put32(mp::SCORE_WEIGHT_STEP, fl(r));
        p.put32(mp::SCORE_WEIGHT_BASE, fl(r));
        let rules = al.take(0x400);
        for word in 0..0x100 {
            let any = r.next();
            p.put32(rules + 4 * word, r.word(&[0, 1, any]));
        }
        let out = al.take(0x20);
        let keys = al.take(0x20);
        for k in 0..7 {
            p.put32(keys + 4 * k, r.next());
        }
        let mut c = case(r, 0x2F2050, vec![out, keys]);
        for _ in 0..7 {
            let count = r.word(&[0, 0, 1, 2, 3, 4, 5, 0xFFFF_FFFF]);
            c.script.rows.push([count, r.next(), r.next()]);
        }
        // 69640 (rules base) is called once in front of the last category: patch its row in.
        c.script.rows.insert(6, [rules, r.next(), r.next()]);
        c
    },
    |s, v, c| mp::rank_categories_by_roster_score(s, v, c));

// ---------------------------------------------------------------------------------------------
// Team pickup: C09B0 deliver, C0C50 dropped countdown, C11A0 touch
// ---------------------------------------------------------------------------------------------
use tsfp_gameplay::t1900_team_pickup as tp;

fn team(r: &mut Rng) -> u32 {
    r.below(6)
}
/// Object with an extension record (+0x7C) and an inner record (+4), all inside the pool.
fn object_with_ext(r: &mut Rng, p: &mut Pair, al: &mut Alloc) -> (u32, u32, u32) {
    let object = al.take(0x240);
    let extension = al.take(0x100);
    let inner = al.take(0x100);
    for word in 0..0x24 {
        p.put32(object + 0x60 + 4 * word, r.next());
    }
    p.put32(object + 0x7C, extension);
    p.put32(object + 4, inner);
    (object, extension, inner)
}
/// Player entries (`[7B0C48]`, stride 0x1584): +4 handle, +0x14 object whose record has a team.
fn pickup_players(r: &mut Rng, p: &mut Pair, al: &mut Alloc) {
    let n = pick(r, &[0, 1, 2, 3, 4, 4]);
    p.put32(tp::PLAYER_COUNT, n);
    let array = al.take(0x6000);
    p.put32(tp::PLAYER_ENTRIES, array);
    for i in 0..n {
        let entry = array + i * tp::PLAYER_ENTRY_STRIDE;
        p.put32(entry + 4, r.next());
        let object = al.take(0x100);
        let record = al.take(0x100);
        p.put32(entry + 0x14, object);
        p.put32(object + 0x7C, record);
        p.put32(record + 0x84, team(r));
    }
    p.put32(tp::TICK_COUNT, r.word(&[0, 1, 2, 3, 60]));
    p.put32(tp::TICK_SCALE, fl(r));
    p.put32(tp::FRAME_TICKS, r.word(&[0, 1, 2, 3, 5, 0xFFFF_FFFF]));
    let table = al.take(0x400);
    p.put32(tp::PICKUP_TABLE, table);
    for word in 0..0x100 {
        p.put32(table + 4 * word, r.next());
    }
    let records = al.take(0x4000);
    p.put32(tp::PLAYER_RECORD_BASE, records);
    p.put32(tp::DEFAULT_TEAM_COLOR, r.next());
}
fn random_rows(r: &mut Rng, c: &mut Case, n: usize) {
    for _ in 0..n {
        c.script.rows.push([r.word(&[0, 1]), r.next(), r.next()]);
    }
}

t1900_test!(lifted_000c09b0, 0xC09B0, Regs::VOID, &[0x45CF0, 0x45390, 0x45820], 3,
    |r, p| {
        let mut al = Alloc(POOL + 0x100);
        p.put8(tp::GAME_MODE, r.word(&[0, 6, 0xA, 3]) as u8);
        pickup_players(r, p, &mut al);
        let (base, base_ext, _) = object_with_ext(r, p, &mut al);
        let (flag, flag_ext, _) = object_with_ext(r, p, &mut al);
        let player = al.take(0x100);
        let (bt, ft, pt) = (team(r), team(r), team(r));
        p.put32(base_ext + tp::ext::TEAM, bt);
        p.put32(flag_ext + tp::ext::TEAM, if chance(r, 30) { bt } else { ft });
        p.put32(player + tp::ext::RECORD_TEAM, if chance(r, 70) { bt } else { pt });
        p.put32(player + tp::ext::RECORD_SLOT, r.below(8));
        for word in 0..0x10 {
            p.put32(flag_ext + 0x20 + 4 * word, r.next());
        }
        let mut c = case(r, 0xC09B0, vec![]);
        c.regs[0] = base;
        c.regs[1] = player;
        c.regs[4] = flag;
        random_rows(r, &mut c, 40);
        c
    },
    |s, v, c| tp::deliver_flag(s, v, c));

t1900_test!(lifted_000c0c50, 0xC0C50, Regs::VOID, &[0xBE860], 3,
    |r, p| {
        let mut al = Alloc(POOL + 0x100);
        p.put8(tp::GAME_MODE, r.word(&[0, 6, 0xA, 3]) as u8);
        pickup_players(r, p, &mut al);
        let (flag, ext_rec, inner) = object_with_ext(r, p, &mut al);
        p.put32(flag + 0x78, if chance(r, 90) { 0 } else { r.next() });
        p.put32(flag + 0x128, if chance(r, 90) { 0 } else { r.next() });
        p.put32(inner + 0x10, if chance(r, 90) { r.next() | 1 } else { 0 });
        p.put32(ext_rec + tp::ext::STATE, if chance(r, 90) { 2 } else { r.word(&[1, 0]) });
        p.put32(tp::FRAME_TICKS, pick(r, &[1, 2, 3, 4]));
        p.put32(ext_rec + tp::ext::COUNTDOWN, pick(r, &[1, 2, 3, 4, 5, 6, 7, 0, 0xFFFF_FFFF, 0x8000_0000]));
        let mode = pick(r, &[1, 2, 0, 3]);
        p.put32(ext_rec + tp::ext::SPAWN_MODE, mode);
        p.put32(ext_rec + tp::ext::SPAWN_PARAM, r.next());
        p.put32(ext_rec + tp::ext::CARRIER_INDEX, r.word(&[0xFFFF_FFFF, 0, 1, 2, 3]));
        p.put32(ext_rec + tp::ext::CARRIER_SLOT, r.next());
        p.put32(ext_rec + tp::ext::RECORD_INDEX, r.below(8));
        for offset in [0x90u32, 0x94, 0x98, 0xAC, 0xB0, 0xB4] {
            p.put32(ext_rec + offset, fl(r));
        }
        let spawn = al.take(0x80);
        p.put32(spawn + 0x28, r.next());
        p.put32(spawn + 0x30, r.next());
        let by_slot = al.take(0x80);
        p.put32(by_slot + 4, r.next());
        let mut c = case(r, 0xC0C50, vec![]);
        c.regs[0] = flag;
        c.script.rows.push([spawn, r.next(), r.next()]);
        if mode == 2 {
            random_rows(r, &mut c, 2);
        }
        if mode == 1 {
            c.script.rows.push([if chance(r, 85) { by_slot } else { 0 }, r.next(), r.next()]);
        }
        random_rows(r, &mut c, 4);
        c
    },
    |s, v, c| tp::dropped_countdown_tick(s, v, c));

t1900_test!(lifted_000c11a0, 0xC11A0, Regs::RET, &[], 3,
    |r, p| {
        let mut al = Alloc(POOL + 0x100);
        p.put8(tp::GAME_MODE, r.word(&[6, 6, 0, 0xA, 3]) as u8);
        pickup_players(r, p, &mut al);
        let (base, base_ext, base_inner) = object_with_ext(r, p, &mut al);
        let (flag, flag_ext, flag_inner) = object_with_ext(r, p, &mut al);
        let toucher = al.take(0x240);
        let record = al.take(0x100);
        p.put32(toucher + 0x7C, record);
        let bt = team(r);
        p.put32(base_ext + tp::ext::TEAM, bt);
        p.put32(record + tp::ext::RECORD_TEAM, if chance(r, 60) { bt } else { team(r) });
        p.put32(record + tp::ext::RECORD_CARRIED_FLAG, if chance(r, 80) { flag } else { 0 });
        p.put32(record + tp::ext::OWNER_PLAYER, r.next());
        p.put32(base + 0x1E8, r.word(&[0, 1, 2, 3, 0xFFFF_FFFF]));
        let list = al.take(0x20);
        p.put32(base + 0x1FC, list);
        p.put32(list, r.next());
        p.put32(list + 4, r.next());
        p.put32(base_inner + 0x30, r.next());
        // Part count chain: [[[[flag+4]+4]+4]+4] is read as [[[flag_inner+4]+4]+4] and the part array at [[flag+4]+0x10].
        let l1 = al.take(0x40);
        let l2 = al.take(0x40);
        let _ = l2;
        p.put32(flag_inner + 4, l1);
        p.put32(l1 + 4, pick(r, &[0, 1, 2, 3]));
        p.put32(flag_inner + 0x10, al.take(0x200));
        p.put32(flag_ext + tp::ext::STATE, r.next());
        let out = al.take(0x10);
        let mut c = case(r, 0xC11A0, vec![toucher, out]);
        c.regs[0] = base;
        c.script.rows.push([r.word(&[0, 1, 1, 1]), r.next(), r.next()]);
        random_rows(r, &mut c, 12);
        c
    },
    |s, v, c| tp::touch_assign_team_or_deliver(s, v, c));
