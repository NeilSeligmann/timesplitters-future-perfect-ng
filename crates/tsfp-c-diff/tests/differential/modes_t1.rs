// SPDX-License-Identifier: GPL-3.0-or-later
//! T1896 Tier 1: the gameplay/modes ports (`tsfp_gameplay::modes`) against the PRIVATE LIFTED
//! ORIGINAL. `build.rs` + `modes_lifted.rs` extract the 28 lifted functions into OUT_DIR at build
//! time (gitignored source, nothing committed) and `md_run` runs them on the C arena while the
//! Rust port runs on an identical Rust arena. Every nested guest call lands in a generated stub
//! with its own trace; the Rust side records through `Host::call`. Compared per case: EAX where
//! the function has a defined return value, the call trace (callee VA and stack arguments, not
//! return PCs or the stack depth, because `ModeHost::call` does not model them), the pool, the
//! argument frame, every address the builder seeded and the declared watch ranges. Without the
//! lifted tree the file compiles to nothing.
#![cfg(modes_lifted)]
use super::*;

use tsfp_gameplay::modes::challenge_setup::{
    ChallengeId, ChallengeLaunchRequest, LAUNCH_RECORD_BYTES, LaunchConfiguration, LaunchRecord,
    LaunchSource, SAVE_SETUP_FLAG, SetupHandoff,
};
use tsfp_gameplay::modes::{self, ModeHost};

unsafe extern "C" {
    fn md_run(va: u32, regs: *mut u32) -> i32;
    fn md_script(words: *const u32, entries: u32);
    fn md_st0(words: *const u32, entries: u32);
    fn md_writes(words: *const u32, entries: u32);
    fn md_argc(va: u32) -> i32;
    fn md_trace_len() -> u32;
    fn md_trace_entry(index: u32) -> *const u32;
}

const SEEDS: usize = 3000;
const POOL: u32 = 0x0090_0000;
const POOL_LEN: u32 = 0x8000;
const ARG_FRAME: u32 = 0x40;

/// Trace row reduced to (callee, up to three stack arguments).
type Row = [u32; 4];

struct Host<'a> {
    view: &'a mut View,
    script: Vec<[u32; 3]>,
    /// ST0 float bits by call index (x87 returning callees).
    st0: Vec<u32>,
    /// Memory a callee writes: (call index, address, dword), applied after the call is recorded.
    writes: Vec<[u32; 3]>,
    trace: Vec<Row>,
}
impl Host<'_> {
    fn record(&mut self, target: u32, args: &[u32]) -> usize {
        let argc = unsafe { md_argc(target) };
        assert!(argc >= 0, "unknown callee {target:x}");
        let mut row = [target, 0, 0, 0];
        for (i, a) in args.iter().take((argc as usize).min(3)).enumerate() {
            row[1 + i] = *a;
        }
        let index = self.trace.len();
        self.trace.push(row);
        for [call, address, value] in self.writes.clone() {
            if call as usize == index {
                for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
                    self.view.set(address.wrapping_add(i as u32), byte);
                }
            }
        }
        index
    }
}
impl GameplayMemory for Host<'_> {
    fn read8(&self, address: u32) -> u8 {
        self.view.byte(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.view.set(address, value)
    }
}
impl ModeHost for Host<'_> {
    fn call(&mut self, target: u32, args: &[u32]) -> u32 {
        let index = self.record(target, args);
        self.script.get(index).map_or(0, |r| r[0])
    }
    fn call_f32(&mut self, target: u32, args: &[u32]) -> f32 {
        let index = self.record(target, args);
        f32::from_bits(self.st0.get(index).copied().unwrap_or(0))
    }
    fn call_x87_unary(&mut self, target: u32, argument: f32) -> f32 {
        let index = self.record(target, &[argument.to_bits()]);
        // The argument travels in ST0, so the C stub records it as the first traced word.
        self.trace[index][1] = argument.to_bits();
        f32::from_bits(self.st0.get(index).copied().unwrap_or(0))
    }
    fn stack_pointer(&self) -> u32 {
        STACK
    }
}

struct MCase {
    va: u32,
    args: Vec<u32>,
    regs: [u32; 7],
    script: Vec<[u32; 3]>,
    st0: Vec<u32>,
    writes: Vec<[u32; 3]>,
    /// Extra addresses (start, len) compared in addition to the seeded ones.
    watch: Vec<(u32, u32)>,
}
fn mcase(r: &mut Rng, va: u32, args: Vec<u32>) -> MCase {
    MCase {
        va,
        args,
        regs: std::array::from_fn(|_| r.next()),
        script: vec![],
        st0: vec![],
        writes: vec![],
        watch: vec![],
    }
}
fn zero(p: &mut Pair, start: u32, len: u32) {
    unsafe {
        std::ptr::write_bytes(p.c.add(start as usize), 0, len as usize);
        std::ptr::write_bytes(p.rust.add(start as usize), 0, len as usize);
    }
}
fn script(r: &mut Rng, rows: usize, special: &[u32]) -> Vec<[u32; 3]> {
    (0..rows)
        .map(|_| [r.word(special), r.next(), r.next()])
        .collect()
}
fn put8s(p: &mut Pair, base: u32, bytes: &[u8]) {
    for (i, b) in bytes.iter().enumerate() {
        p.put8(base + i as u32, *b);
    }
}

fn run_c(p: &mut Pair, c: &MCase) -> (u32, Vec<Row>) {
    p.put32(STACK, 0);
    for (i, a) in c.args.iter().enumerate() {
        p.put32(STACK + 4 + 4 * i as u32, *a);
    }
    let flat: Vec<u32> = c.script.iter().flatten().copied().collect();
    unsafe { md_script(flat.as_ptr(), c.script.len() as u32) };
    unsafe { md_st0(c.st0.as_ptr(), c.st0.len() as u32) };
    let wflat: Vec<u32> = c.writes.iter().flatten().copied().collect();
    unsafe { md_writes(wflat.as_ptr(), c.writes.len() as u32) };
    let mut regs = [
        c.regs[0], c.regs[1], c.regs[2], c.regs[3], c.regs[4], c.regs[5], c.regs[6], STACK,
    ];
    assert_eq!(
        unsafe { md_run(c.va, regs.as_mut_ptr()) },
        1,
        "unknown root {:x}",
        c.va
    );
    let trace = (0..unsafe { md_trace_len() })
        .map(|i| {
            let e = unsafe { *(md_trace_entry(i) as *const [u32; 16]) };
            [e[0], e[3], e[4], e[5]]
        })
        .collect();
    (regs[0], trace)
}

fn mem_diff(p: &Pair, logged: &[u32], watch: &[(u32, u32)]) -> Option<String> {
    let ranges = [(POOL, POOL_LEN), (STACK, ARG_FRAME)];
    let extra = logged.iter().map(|a| (*a, 4)).chain(watch.iter().copied());
    for (start, len) in ranges.iter().copied().chain(extra) {
        for off in 0..len {
            let a = start.wrapping_add(off);
            if p.get(false, a) != p.get(true, a) {
                return Some(format!(
                    "memory {a:08x}: C {:02x} Rust {:02x}",
                    p.get(false, a),
                    p.get(true, a)
                ));
            }
        }
    }
    None
}

/// `port` returns the EAX to compare, or `None` when the original's EAX is undefined residue.
fn drive(
    seed: u64,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> MCase,
    port: impl Fn(&mut Host, &MCase) -> Option<u32>,
) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut r = Rng(seed);
    let (mut with_calls, mut compared) = (0usize, 0usize);
    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        p.log.clear();
        let c = build(&mut r, p);
        let logged: Vec<u32> = p.log.drain(..).collect();
        let (ceax, ctrace) = run_c(p, &c);
        let mut view = View(p.rust);
        let mut host = Host {
            view: &mut view,
            script: c.script.clone(),
            st0: c.st0.clone(),
            writes: c.writes.clone(),
            trace: vec![],
        };
        let reax = port(&mut host, &c);
        let rtrace = host.trace;
        let mut why = vec![];
        if let Some(value) = reax {
            compared += 1;
            if value != ceax {
                why.push(format!("eax C {ceax:#x} Rust {value:#x}"));
            }
        }
        if ctrace != rtrace {
            let first = ctrace.iter().zip(rtrace.iter()).position(|(a, b)| a != b);
            why.push(format!(
                "trace len C {} Rust {} first diff {first:?}: C {:x?} Rust {:x?}",
                ctrace.len(),
                rtrace.len(),
                first.map(|i| ctrace[i]),
                first.map(|i| rtrace[i])
            ));
        }
        why.extend(mem_diff(p, &logged, &c.watch));
        assert!(
            why.is_empty(),
            "root {:08X} seed {seed:#x} case {index}: {why:?}",
            c.va
        );
        with_calls += usize::from(!ctrace.is_empty());
    }
    eprintln!(
        "modes t1 {seed:#x}: {SEEDS} cases, {with_calls} with calls, eax compared in {compared}"
    );
}

macro_rules! md_test {
    ($name:ident, $seed:expr, $build:expr, $port:expr) => {
        #[test]
        fn $name() {
            drive($seed, $build, $port);
        }
    };
}

fn flags(r: &mut Rng) -> u32 {
    [
        0u32,
        0x80_0000,
        0x40,
        0x800_0000,
        0x80_0040,
        r.next(),
        r.next() & !0x80_0040,
    ][r.below(7) as usize]
}
fn state(r: &mut Rng) -> u32 {
    [2u32, 3, 5, 0, 1, 4, r.next()][r.below(7) as usize]
}

// 000425E0 / 00042620 transition state
fn build_425e0(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(0x7DE458, flags(r));
    p.put32(0x6B98A4, state(r));
    p.put32(0x475D18, r.next());
    p.put32(0x6B98B0, r.next());
    mcase(r, 0x425E0, vec![])
}
md_test!(t1_000425e0, 0x1896_0001, build_425e0, |h, _| {
    modes::predicates::transition_enter_5(h);
    None
});
fn build_42620(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(0x6B98A4, state(r));
    mcase(r, 0x42620, vec![])
}
md_test!(t1_00042620, 0x1896_0002, build_42620, |h, _| Some(
    modes::predicates::transition_idle(h)
));

// 000431F0 / 00069E70 / 00069E80 transition set
fn build_431f0(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(0x6F1E10, r.next());
    for a in [0x6B98A4, 0x6B98A8, 0x6B989C, 0x6B98A0] {
        p.put32(a, r.next());
    }
    let args = vec![r.next(), r.next(), r.next()];
    let mut c = mcase(r, 0x431F0, args);
    c.script = script(r, 1, &[0, 1]);
    c
}
md_test!(t1_000431f0, 0x1896_0003, build_431f0, |h, c| Some(
    modes::setup::transition_set(h, c.args[0], c.args[1], c.args[2])
));
fn build_69e70(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(0x6F1E10, r.next());
    mcase(r, 0x69E70, vec![])
}
md_test!(t1_00069e70, 0x1896_0004, build_69e70, |h, _| Some(
    modes::setup::setup_record_transition_block(h)
));
fn build_69e80(r: &mut Rng, p: &mut Pair) -> MCase {
    let rec = r.pool(POOL);
    p.put32(0x6F1E10, rec);
    p.put32(rec + 0xC, r.next());
    p.put32(rec + 0x10, r.next());
    let mut c = mcase(r, 0x69E80, vec![]);
    c.script = script(r, 1, &[0, 1]);
    c
}
md_test!(t1_00069e80, 0x1896_0005, build_69e80, |h, _| Some(
    modes::setup::apply_setup_record(h)
));

// 000BE850 / 001B6030 / 001CD1E0 / 00227C30
md_test!(
    t1_000be850,
    0x1896_0006,
    |r, _| {
        let a = r.next();
        mcase(r, 0xBE850, vec![a])
    },
    |_, c| Some(modes::tables::table_4ddae8_entry(c.args[0]))
);
fn build_1b6030(r: &mut Rng, _: &mut Pair) -> MCase {
    let a = r.next();
    let mut c = mcase(r, 0x1B6030, vec![a]);
    c.script = script(r, 1, &[0, 1]);
    c
}
md_test!(t1_001b6030, 0x1896_0007, build_1b6030, |h, c| Some(
    modes::setup::weapon_record_preload(h, c.args[0])
));
fn build_1cd1e0(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(0x7DE458, flags(r));
    p.put32(0x790958, r.next());
    let mut c = mcase(r, 0x1CD1E0, vec![]);
    c.script = script(r, 1, &[0, 1]);
    c.watch.push((0x790958, 4));
    c
}
md_test!(t1_001cd1e0, 0x1896_0008, build_1cd1e0, |h, _| {
    let blocked = h.read32(0x7DE458) & 0x80_0040 != 0;
    let value = modes::predicates::clear_790958_or_refresh(h);
    (!blocked).then_some(value)
});
fn build_227c30(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(
        0x79094C,
        [0x64u32, 0x64, 0x65, r.next()][r.below(4) as usize],
    );
    let a = r.next();
    let mut c = mcase(r, 0x227C30, vec![a]);
    c.script = script(r, 1, &[0, 1]);
    c
}
md_test!(t1_00227c30, 0x1896_0009, build_227c30, |h, c| Some(
    modes::predicates::dispatch_text_or_mode_0x64(h, &c.args)
));

// 000652C0 / 0013DCF0 predicates with guest index tables
fn seed_652c0_tables(r: &mut Rng, p: &mut Pair) {
    p.put32(0x652F8, 0x652F0);
    p.put32(0x652FC, 0x652ED);
    let bytes: Vec<u8> = (0..0x33).map(|_| r.below(2) as u8).collect();
    put8s(p, 0x65300, &bytes);
}
fn build_652c0(r: &mut Rng, p: &mut Pair) -> MCase {
    seed_652c0_tables(r, p);
    let mode = [0x3Au32, 0x19, 0x3B, 0, r.next()][r.below(5) as usize];
    let key = [0x71 + r.below(0x34), r.next(), 0x70, 0xA4][r.below(4) as usize];
    mcase(r, 0x652C0, vec![mode, key])
}
md_test!(t1_000652c0, 0x1896_000a, build_652c0, |h, c| Some(
    modes::predicates::mode_or_key_predicate(h, c.args[0], c.args[1])
));
fn build_13dcf0(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(0x13DD28, 0x13DD1E);
    p.put32(0x13DD2C, 0x13DD24);
    let bytes: Vec<u8> = (0..14).map(|_| r.below(2) as u8).collect();
    put8s(p, 0x13DD30, &bytes);
    p.put8(
        0x7DE455,
        [0u32, 1, 2, 3, 4, 6, 7, r.below(256)][r.below(8) as usize] as u8,
    );
    p.put8(
        0x7DE456,
        [r.below(0x16) as u32, r.below(256)][r.below(2) as usize] as u8,
    );
    mcase(r, 0x13DCF0, vec![])
}
md_test!(t1_0013dcf0, 0x1896_000b, build_13dcf0, |h, _| Some(
    modes::predicates::mode_predicate(h)
));

// 00067450 / 00067E60 match end
fn seed_match(r: &mut Rng, p: &mut Pair) {
    p.put8(
        0x7DE455,
        [3u32, 7, 1, 10, r.below(256)][r.below(5) as usize] as u8,
    );
    p.put32(0x7DE458, flags(r));
    p.put32(0x7DE30C, [8u32, 2, r.next()][r.below(3) as usize]);
    p.put32(0x7DE300, r.next());
    p.put32(0x6F11A4, r.next());
}
const MATCH_WATCH: [(u32, u32); 3] = [(0x7DE30C, 4), (0x7DE300, 4), (0x6F11A4, 4)];
fn build_67450(r: &mut Rng, p: &mut Pair) -> MCase {
    seed_match(r, p);
    let a = [0u32, 1, r.next()][r.below(3) as usize];
    let mut c = mcase(r, 0x67450, vec![a]);
    c.script = script(r, 4, &[0, 1]);
    c.watch = MATCH_WATCH.to_vec();
    c
}
md_test!(t1_00067450, 0x1896_000c, build_67450, |h, c| Some(
    modes::match_flow::match_end_enter_state_8(h, c.args[0])
));
fn build_67e60(r: &mut Rng, p: &mut Pair) -> MCase {
    seed_match(r, p);
    let target = [0u32, 5, 9][r.below(3) as usize];
    p.put32(0x7DE468, target);
    let mut c = mcase(r, 0x67E60, vec![]);
    c.script = (0..6)
        .map(|_| [[target, 0, 1, 2][r.below(4) as usize], r.next(), r.next()])
        .collect();
    c.watch = MATCH_WATCH.to_vec();
    c
}
// EAX at exit is residue (the loop counter, 2) when no slot matches, so it is not compared.
md_test!(t1_00067e60, 0x1896_000d, build_67e60, |h, _| {
    modes::match_flow::match_end_if_slot_matches(h);
    None
});

// 00158C80 / 002B0AF0 side tables
fn build_158c80(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put8(
        0x7DE455,
        [3u32, 3, 1, r.below(256)][r.below(4) as usize] as u8,
    );
    p.put8(
        0x7DE402,
        [0u32, 1, 2, 0x80, 0xFF, r.below(256)][r.below(6) as usize] as u8,
    );
    let mut c = mcase(r, 0x158C80, vec![]);
    c.script = script(r, 3, &[0, 1]);
    c
}
md_test!(t1_00158c80, 0x1896_000e, build_158c80, |h, _| {
    let value = modes::setup::side_tables_stamp_or_setup(h);
    // EAX is only defined when the tail call ran.
    (h.trace.last().map(|row| row[0]) == Some(0x273240)).then_some(value)
});
fn build_2b0af0(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put8(
        0x7DE455,
        [3u32, 3, 1, r.below(256)][r.below(4) as usize] as u8,
    );
    let mut c = mcase(r, 0x2B0AF0, vec![]);
    c.script = script(r, 3, &[0, 1]);
    c
}
md_test!(t1_002b0af0, 0x1896_000f, build_2b0af0, |h, _| Some(
    modes::setup::alloc_mode_tables(h)
));

// 00233EF0 count entries, 002719F0 find entry
fn build_233ef0(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(0x79094C, r.next());
    p.put32(0x6B7AA8, [0u32, 0, 0, 1][r.below(4) as usize]);
    let setup = r.pool(POOL);
    let entries = r.pool(POOL + 0x2000) & !3;
    let count = [0u32, 1, 2, 3, 4, 0xFFFF_FFFF][r.below(6) as usize];
    p.put32(setup + 0xCC, entries);
    p.put32(setup + 0xD0, count);
    for i in 0..5 {
        p.put32(
            entries + 0x60 + i * 0xC8,
            [0u32, 1, r.next()][r.below(3) as usize],
        );
        p.put32(entries + 0x60 + i * 0xC8 + 0x54, r.next());
    }
    p.put32(0x75CCB4, r.next());
    p.put32(0x75CCB8, r.next());
    let mut c = mcase(r, 0x233EF0, vec![]);
    c.script = std::iter::once([[setup, 0][r.below(5).min(1) as usize], 0, 0])
        .chain((0..5).map(|_| [r.below(2), 0, 0]))
        .collect();
    c.watch = vec![(0x75CCB4, 8)];
    c
}
md_test!(t1_00233ef0, 0x1896_0010, build_233ef0, |h, _| {
    modes::setup::setup_count_entries_matching(h);
    None
});
fn build_2719f0(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(0x79094C, r.next());
    let setup = r.pool(POOL);
    let base = r.pool(POOL + 0x2000) & !3;
    let count = [0u32, 1, 2, 3, 5, 0xFFFF_FFFF][r.below(6) as usize];
    p.put32(setup + 0xF4, base);
    p.put32(setup + 0xF8, count);
    let id = r.below(4);
    for i in 0..6 {
        p.put32(base + i * 0x20, r.below(5));
    }
    let mut c = mcase(r, 0x2719F0, vec![id]);
    c.script = vec![[[setup, 0][r.below(5).min(1) as usize], 0, 0]];
    c
}
md_test!(t1_002719f0, 0x1896_0011, build_2719f0, |h, c| Some(
    modes::setup::find_setup_entry_by_id(h, c.args[0])
));

// 00238D50 row count, 002E7EB0 players allow mode
fn build_238d50(r: &mut Rng, p: &mut Pair) -> MCase {
    let row = [r.below(0x12), 0xFFFF_FFFF, 0x8000_0000, 0x10, 0xF][r.below(5) as usize];
    for i in 0..3 {
        p.put32(
            0x75CD90u32
                .wrapping_add(row.wrapping_mul(12))
                .wrapping_add(4 * i),
            [0u32, 1, r.next()][r.below(3) as usize],
        );
    }
    mcase(r, 0x238D50, vec![row])
}
md_test!(t1_00238d50, 0x1896_0012, build_238d50, |h, c| Some(
    modes::setup::row_nonzero_dword_count(h, c.args[0])
));
fn build_2e7eb0(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(
        0x79094C,
        [0x64u32, 0x65, 1, 0xFFFF_FFFF, r.next()][r.below(5) as usize],
    );
    let kind = [1u32, 2, 3, 0, r.next()][r.below(5) as usize];
    let mut c = mcase(r, 0x2E7EB0, vec![kind]);
    c.script = (0..2)
        .map(|_| {
            [
                [0u32, 1, 2, 3, 4, 5, 0xFFFF_FFFF][r.below(7) as usize],
                0,
                0,
            ]
        })
        .collect();
    c
}
md_test!(t1_002e7eb0, 0x1896_0013, build_2e7eb0, |h, c| Some(
    modes::predicates::players_allow_mode(h, c.args[0])
));

// 0023F320 / 0023F350 preload lists (register inputs EBX and EAX)
fn ids16(r: &mut Rng) -> u32 {
    [0xFFFFu32, r.below(0x100), r.below(0x10000)][r.below(3) as usize]
}
fn build_23f320(r: &mut Rng, p: &mut Pair) -> MCase {
    let list = r.pool(POOL);
    let count = [0u32, 1, 2, 3, 4, 0xFF][r.below(6) as usize];
    p.put8(list, count as u8);
    for i in 0..8 {
        let id = ids16(r);
        p.put8(list + 2 + 4 * i, id as u8);
        p.put8(list + 3 + 4 * i, (id >> 8) as u8);
    }
    let mut c = mcase(r, 0x23F320, vec![]);
    c.regs[3] = list;
    c.script = vec![];
    c
}
md_test!(t1_0023f320, 0x1896_0014, build_23f320, |h, c| {
    modes::setup::preload_counted_list(h, c.regs[3]);
    None
});
fn build_23f350(r: &mut Rng, p: &mut Pair) -> MCase {
    let index = r.below(4);
    let rec = 0x53E41C + index * 0xA4;
    p.put32(rec, [0u32, 1, 2, 3, 8, 0xFFFF_FFFF][r.below(6) as usize]);
    for i in 0..9 {
        let id = ids16(r);
        p.put8(rec + 4 + 8 * i, id as u8);
        p.put8(rec + 5 + 8 * i, (id >> 8) as u8);
    }
    let mut c = mcase(r, 0x23F350, vec![]);
    c.regs[0] = index;
    c
}
md_test!(t1_0023f350, 0x1896_0015, build_23f350, |h, c| {
    modes::setup::preload_type_list(h, c.regs[0]);
    None
});

// record 58 accessors and payload copy, six byte entries
fn build_record58(va: u32) -> impl FnMut(&mut Rng, &mut Pair) -> MCase {
    move |r, p| {
        let index = [r.below(4), r.below(0x100), r.next() & 0xFFFF][r.below(3) as usize];
        let base = index.wrapping_mul(0x58).wrapping_add(0x52CF40);
        for off in 2..5 {
            p.put8(base + off, r.next() as u8);
        }
        mcase(r, va, vec![index])
    }
}
md_test!(
    t1_00311860,
    0x1896_0016,
    build_record58(0x311860),
    |h, c| Some(modes::tables::record_58_byte_a(h, c.args[0]))
);
md_test!(
    t1_00311870,
    0x1896_0017,
    build_record58(0x311870),
    |h, c| Some(modes::tables::record_58_byte_b(h, c.args[0]))
);
md_test!(
    t1_00311880,
    0x1896_0018,
    build_record58(0x311880),
    |h, c| Some(modes::tables::record_58_byte_c(h, c.args[0]))
);
fn build_3118b0(r: &mut Rng, p: &mut Pair) -> MCase {
    let index = r.below(8);
    let dest = r.pool(POOL + 0x4000);
    for w in 0..15 {
        p.put32(0x52CF48 + index * 0x58 + 4 * w, r.next());
    }
    for w in 0..16 {
        p.put32(dest + 4 * w, r.next());
    }
    let mut c = mcase(r, 0x3118B0, vec![dest, index]);
    c.watch = vec![(dest, 64)];
    c
}
md_test!(t1_003118b0, 0x1896_0019, build_3118b0, |h, c| {
    modes::tables::record_58_copy_payload(h, c.args[0], c.args[1]);
    None
});
fn build_320fc0(r: &mut Rng, p: &mut Pair) -> MCase {
    let src = r.pool(POOL + 0x4000);
    let size = [0u32, 8, 16, 24, 40, 48, 7, 0x8000_0000 >> 28, r.below(64)][r.below(9) as usize];
    for w in 0..16 {
        p.put32(src + 4 * w, r.next());
    }
    for w in 0..16 {
        p.put32(0x782E60 + 4 * w, r.next());
    }
    let mut c = mcase(r, 0x320FC0, vec![src, size]);
    c.watch = vec![(0x782E60, 64)];
    c
}
md_test!(t1_00320fc0, 0x1896_001a, build_320fc0, |h, c| {
    modes::tables::copy_six_byte_entries(h, c.args[0], c.args[1]);
    None
});

// 002F76D0 / 002F76E0 results widget wrappers
fn build_widget(va: u32) -> impl FnMut(&mut Rng, &mut Pair) -> MCase {
    move |r, _| {
        let widget = r.next();
        let mut c = mcase(r, va, vec![widget]);
        c.script = script(r, 1, &[0, 1]);
        c
    }
}
md_test!(t1_002f76d0, 0x1896_001b, build_widget(0x2F76D0), |h, c| {
    Some(modes::tables::results_build_widget(h, c.args[0], 0))
});
md_test!(t1_002f76e0, 0x1896_001c, build_widget(0x2F76E0), |h, c| {
    Some(modes::tables::results_build_widget(h, c.args[0], 1))
});

// ---- batch 2: 00067280 setup load or copy, 00068610 pick next state, 0028C560 other branch ----
fn fill_dwords(r: &mut Rng, p: &mut Pair, base: u32, count: u32) {
    for w in 0..count {
        p.put32(base + 4 * w, r.next());
    }
}
fn build_67280(r: &mut Rng, p: &mut Pair) -> MCase {
    let loaded = r.below(2) == 0;
    let flags398 = if loaded {
        0x80_0000 | (r.next() & 0x6000_0000)
    } else {
        r.next() & !0x80_0000
    };
    p.put32(0x7DE398, flags398);
    let mode_flags = flags(r);
    p.put32(0x7DE458, mode_flags);
    p.put8(0x7DE3DF, r.next() as u8);
    p.put8(0x7DE454, r.next() as u8);
    p.put32(0x790950, r.next());
    p.put32(0x7DE304, r.next());
    p.put8(0x7DE49A, r.next() as u8);
    p.put8(0x7DE49B, r.next() as u8);
    let ui_mode = match r.below(3) {
        0 => 6,
        1 => 7,
        _ => 0,
    };
    p.put32(0x78AD2C, ui_mode);
    p.put32(0x78AD38, r.next());
    fill_dwords(r, p, 0x7DE340, 0x28);
    fill_dwords(r, p, 0x7DE400, 0x28);
    // Seed the branch selectors after the random record fills so the scripted helper path
    // matches the state observed by the lifted function.
    p.put32(0x7DE398, flags398);
    p.put32(0x7DE458, mode_flags);
    let record = r.pool(POOL);
    fill_dwords(r, p, record + 0x10, 0x28);
    let tail = [0u32, 1, r.next()];
    let pick = r.below(3) as usize;
    let row = |v: u32| [v, 0, 0];
    let mut c = mcase(r, 0x67280, vec![]);
    let seeded = [0u32, 1][r.below(2) as usize];
    let rng_roll = r.next();
    c.script = if loaded {
        vec![
            row(0),
            row(record),
            row(0),
            row(rng_roll),
            row(seeded),
            row(0),
            row(tail[pick]),
        ]
    } else if mode_flags & 0x40 == 0 {
        vec![row(0), row(rng_roll), row(seeded), row(0), row(tail[pick])]
    } else {
        vec![
            row(record),
            row(0),
            row(rng_roll),
            row(seeded),
            row(0),
            row(tail[pick]),
        ]
    };
    c.watch = vec![
        (0x7DE340, 0xA0),
        (0x7DE400, 0xA0),
        (record + 0x10, 0xA0),
        (0x7DE398, 4),
        (0x7DE49A, 2),
        (0x7DE458, 4),
        (0x7DE304, 4),
        (0x7DE328, 4),
        (0x7DE314, 4),
        (0x7DE310, 4),
        (0x6B7AAC, 4),
    ];
    c
}
md_test!(t1_00067280, 0x1896_001d, build_67280, |h, _| Some(
    modes::setup::mode_setup_load_or_copy_then_start(h)
));

fn read_launch_record(view: &View, base: u32) -> LaunchRecord {
    LaunchRecord::from_bytes(std::array::from_fn(|index| {
        view.byte(base.wrapping_add(index as u32))
    }))
}

fn write_launch_record(view: &mut View, base: u32, record: &LaunchRecord) {
    for (index, byte) in record.as_bytes().iter().copied().enumerate() {
        view.set(base.wrapping_add(index as u32), byte);
    }
}

/// The selected-descriptor builder's documented fields are Tier 1 against lifted 0x2D2C70.
/// Variant 2 returns before the roster helpers, letting this comparison isolate the known setup
/// record contract while still checking full-width variant selection and byte preservation.
#[test]
fn t1_arcade_selected_setup_fields_match_lifted_2d2c70() {
    const DESCRIPTOR: u32 = POOL + 0x2000;
    const PROFILE: u32 = POOL + 0x3000;
    const MENU_RECORD: u32 = 0x0078_ad58;
    let offsets: &[(usize, usize)] = &[
        (0x00, 2),
        (0x54, 5),
        (0x58, 4),
        (0x60, 4),
        (0x64, 4),
        (0x68, 4),
        (0x6c, 1),
        (0x70, 4),
        (0x74, 4),
        (0x8c, 4),
    ];
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut rng = Rng(0x2381_2d2c_70a1_5eed);

    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        p.log.clear();
        let ui_mode = if rng.below(2) == 0 { 6 } else { 7 };
        let mission = rng.next();
        let participant_count = rng.next() as u8;
        let type_byte = rng.next() as u8;
        let level = rng.next() as u8;
        let variant = 2u32;
        let descriptor_flags = rng.next();
        let timing = rng.next();
        let duration = rng.next();
        let setup_94 = rng.next();
        let setup_98 = rng.next();
        let setup_9c = rng.next();
        let menu_previous: [u8; LAUNCH_RECORD_BYTES] = std::array::from_fn(|_| rng.next() as u8);
        put8s(p, MENU_RECORD, &menu_previous);
        p.put32(0x78ad2c, 2);
        p.put32(0x78ad40, mission);
        p.put32(0x5236c4, u32::from(participant_count));
        p.put32(0x79094c, 0x64);
        p.put8(0x7de455, 0x0a);
        p.put32(0x4e8cec, timing);
        p.put32(0x475c78, 1.0f32.to_bits());
        p.put8(0x4d1f7c + variant * 0x20, type_byte);
        p.put32(DESCRIPTOR + 0x04, variant);
        p.put32(DESCRIPTOR + 0x08, u32::from(level));
        p.put8(DESCRIPTOR + 0x0c, rng.next() as u8);
        p.put32(DESCRIPTOR + 0x10, descriptor_flags);
        p.put32(DESCRIPTOR + 0x6c, 0); // nonnegative first variant-2 participant
        p.put32(DESCRIPTOR + 0x90, duration);
        p.put32(DESCRIPTOR + 0x94, setup_94);
        p.put32(DESCRIPTOR + 0x98, setup_98);
        p.put32(DESCRIPTOR + 0x9c, setup_9c);
        p.put8(0x2d389c + 5, 4);
        p.put8(0x2d389c + 6, 4);
        p.put32(0x2d386c + 4 * 4, 0x2d2f4c);

        let mut case = mcase(&mut rng, 0x2d2c70, vec![]);
        case.script = vec![
            [0, 0, 0],
            [PROFILE, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [DESCRIPTOR, 0, 0],
            [variant, 0, 0],
            [u32::from(level), 0, 0],
            [variant, 0, 0],
        ];
        case.writes = vec![[4, 0x78ad2c, ui_mode]];

        let (_eax, _trace) = run_c(p, &case);
        let mut view = View(p.rust);
        let request = ChallengeLaunchRequest {
            category: modes::challenge_setup::ArcadeCategoryId(rng.next()),
            row: Some(modes::challenge_setup::CategoryRowId(rng.next())),
            challenge: ChallengeId(mission),
            ui_mode: if ui_mode == 6 {
                modes::challenge_setup::ChallengeUiMode::Mode6
            } else {
                modes::challenge_setup::ChallengeUiMode::Mode7
            },
        };
        let descriptor = modes::challenge_setup::ChallengeDescriptor {
            word_04: variant,
            word_08: u32::from(level),
            byte_0c: view.byte(DESCRIPTOR + 0x0c),
            word_10: descriptor_flags,
            first_roster_id: Some(0),
            word_90: duration,
            word_94: setup_94,
            word_98: setup_98,
            word_9c: setup_9c,
        };
        let mut type_by_variant = [0; 256];
        type_by_variant[variant as usize] = type_byte;
        let context = modes::challenge_setup::ChallengeSetupContext {
            menu_participant_count: participant_count,
            current_runtime_mode: 0x0a,
            variant_policy_word_is_zero: false,
            type_by_variant,
            variant2_fallback_participant: None,
            timing_multiplier: timing,
        };
        let previous = read_launch_record(&view, MENU_RECORD);
        let built = modes::challenge_setup::SelectedChallengeSetup::build(
            request, descriptor, &context, &previous,
        )
        .expect("nonnegative descriptor participant does not call fallback");
        write_launch_record(&mut view, MENU_RECORD, &built.record);

        for (offset, length) in offsets {
            for byte in 0..*length {
                let address = MENU_RECORD + *offset as u32 + byte as u32;
                assert_eq!(
                    p.get(false, address),
                    p.get(true, address),
                    "case {index}, record offset {:#x}: lifted {:02x}, Rust {:02x}",
                    *offset + byte,
                    p.get(false, address),
                    p.get(true, address),
                );
            }
        }
    }
}

/// Tier 1 for the owned staging/saved-record/active copy edge. The lifted 0x67280 body owns
/// unrelated transition and RNG effects, so compare only the exact blocks this model publishes.
#[test]
fn t1_arcade_owned_setup_handoff_matches_lifted_67280_copy_paths() {
    const BLOCK: u32 = 0x007d_e340;
    const ACTIVE: u32 = 0x007d_e400;
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut rng = Rng(0x2381_6728_0a11_ce55);

    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        p.log.clear();
        let mut case = build_67280(&mut rng, p);
        let flags = unsafe { std::ptr::read_unaligned(p.rust.add(0x7de398) as *const u32) };
        let mode_flags = unsafe { std::ptr::read_unaligned(p.rust.add(0x7de458) as *const u32) };
        let loaded = flags & SAVE_SETUP_FLAG != 0;
        let saved_record = if loaded {
            case.script[1][0]
        } else if mode_flags & 0x40 != 0 {
            case.script[0][0]
        } else {
            0
        };
        // Keep 0x67280's later RNG and side-table effects outside this record-copy comparison.
        // Preserve only the saved-record pointer return required by the chosen copy branch.
        for row in &mut case.script {
            *row = [0; 3];
        }
        if saved_record != 0 {
            case.script[if loaded { 1 } else { 0 }][0] = saved_record;
        }

        let (_eax, _trace) = run_c(p, &case);
        let mut view = View(p.rust);
        let selected_request = ChallengeLaunchRequest {
            category: modes::challenge_setup::ArcadeCategoryId(0),
            row: Some(modes::challenge_setup::CategoryRowId(0)),
            challenge: ChallengeId(0),
            ui_mode: modes::challenge_setup::ChallengeUiMode::Mode6,
        };
        let staging = read_launch_record(&view, BLOCK);
        let mut handoff = SetupHandoff {
            staging: LaunchConfiguration {
                source: LaunchSource::SelectedChallenge(selected_request),
                record: staging,
            },
            active: None,
        };

        let saved = (saved_record != 0).then(|| read_launch_record(&view, saved_record + 0x10));
        let active = handoff
            .publish(saved.as_ref())
            .unwrap_or_else(|error| panic!("case {index}: handoff input error: {error:?}"))
            .clone();
        write_launch_record(&mut view, ACTIVE, &active.record);
        if loaded {
            write_launch_record(&mut view, BLOCK, &handoff.staging.record);
        } else if mode_flags & 0x40 != 0 {
            write_launch_record(&mut view, saved_record + 0x10, &handoff.staging.record);
        }

        // Compare the published runtime record. The owned model names the effective saved
        // configuration as staging; the original helper's guest staging scratch may retain
        // unrelated bytes on saved-load branches. Bytes +0x9a/+0x9b are later RNG/side-table
        // outputs, outside the setup-record copy contract; the scripted side-table return is 0.
        for offset in 0..LAUNCH_RECORD_BYTES {
            if matches!(offset, 0x9a | 0x9b) {
                continue;
            }
            let address = ACTIVE + offset as u32;
            assert_eq!(
                p.get(false, address),
                p.get(true, address),
                "case {index}, active record offset {offset:#x}, loaded={loaded}, saved_ptr={saved_record:#x}, input_staging_flags={flags:#010x}, saved_flags={:?}, lifted_active_flags={:#010x}, rust_active_flags={:#010x}",
                saved.as_ref().map(|record| record.read_u32(0x58)),
                read_launch_record(&View(p.c), ACTIVE).read_u32(0x58),
                read_launch_record(&View(p.rust), ACTIVE).read_u32(0x58),
            );
        }
    }
}

fn build_68610(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(
        0x7DE30C,
        [3u32, 1, 4, 7, 8, 0, r.next()][r.below(7) as usize],
    );
    p.put32(
        0x79094C,
        [0x64u32, 0x65, 0, 1, 0xFFFF_FFFF, r.next()][r.below(6) as usize],
    );
    p.put32(
        0x7DE458,
        [0u32, 0x2000_0000, 0x40, 0x2000_0040, 0x4000_0000, r.next()][r.below(6) as usize],
    );
    p.put8(
        0x7DE455,
        [1u32, 0xA, 3, r.below(256)][r.below(4) as usize] as u8,
    );
    p.put32(0x6B7AAC, [0u32, 1, 2][r.below(3) as usize]);
    p.put32(0x7DE3E0, r.next());
    p.put32(0x7DE300, r.next());
    p.put32(0x6F1194, r.next());
    let mut c = mcase(r, 0x68610, vec![]);
    c.script = (0..6)
        .map(|_| [[0u32, 1, 7][r.below(3) as usize], r.next(), r.next()])
        .collect();
    // The second 7 check reads the state again after 667B0, which the script cannot change.
    c.watch = vec![
        (0x7DE30C, 4),
        (0x7DE300, 4),
        (0x7DE3E0, 4),
        (0x6F1194, 4),
        (0x7DE458, 4),
    ];
    c
}
md_test!(t1_00068610, 0x1896_001e, build_68610, |h, _| Some(
    modes::match_flow::match_end_pick_next_state(h)
));
fn build_28c560(r: &mut Rng, p: &mut Pair) -> MCase {
    let targets = [0x28C57Au32, 0x28C584, 0x28C615, 0x28C618];
    for i in 0..6 {
        p.put32(0x28C61C + 4 * i, targets[r.below(4) as usize]);
    }
    let resources = POOL + 0x1000;
    p.put32(0x78B1EC, resources);
    for field in [0x4BDCu32, 0x4BD8, 0x4BCC, 0x4270] {
        p.put32(
            resources + field,
            [0u32, 0x1234, r.next()][r.below(3) as usize],
        );
    }
    let object = r.pool(POOL + 0x6000);
    p.put32(object + 0x28, r.next());
    let kind = [r.below(8), 0, 7, 0xFFFF_FFFF, r.next()][r.below(5) as usize];
    let mut c = mcase(r, 0x28C560, vec![object, kind]);
    c.script = vec![[0, 0, 0]; 4];
    c.watch = vec![(object + 0x28, 4)];
    c
}
md_test!(t1_0028c560, 0x1896_001f, build_28c560, |h, c| Some(
    modes::tables::mode_other_branch(h, c.args[0], c.args[1])
));

// ---- 00067CD0 match leader update ----
fn build_67cd0(r: &mut Rng, p: &mut Pair) -> MCase {
    let count = [0u32, 1, 2, 3, 4][r.below(5) as usize];
    p.put32(0x74C37C, count);
    let leader_pool = |r: &mut Rng| [0u32, 1, 2, 3, 0xFFFF_FFFF][r.below(5) as usize];
    for i in 0..4u32 {
        let object = if r.below(4) == 0 {
            0
        } else {
            r.pool(POOL + 0x6000) & !3
        };
        p.put32(0x7A3580 + 4 * i, object);
        if object != 0 {
            p.put32(object + 0x80, [0u32, 1][r.below(2) as usize]);
            p.put32(object + 8, leader_pool(r));
        }
    }
    p.put32(0x7DE3E0, [0u32, 1, 2, 3, 0xFFFF_FFFF][r.below(5) as usize]);
    let players = POOL;
    p.put32(0x7356D8, players);
    for i in 0..4u32 {
        p.put32(players + i * 0xEA0 + 0x70, r.next());
        p.put32(players + i * 0xEA0 + 0xA64, r.next());
    }
    p.put32(0x790950, [0u32, 1, 2, 3, 4][r.below(5) as usize]);
    let slots = POOL + 0x3000;
    p.put32(0x7B0C48, slots);
    for i in 0..4u32 {
        p.put32(
            slots + i * 0x1584 + 4,
            [0u32, 1, 2, 3, 0xFFFF_FFFF][r.below(5) as usize],
        );
    }
    p.put32(0x4E7944, r.next());
    p.put32(0x4E8CEC, r.next());
    let mut c = mcase(r, 0x67CD0, vec![]);
    c.script = (0..14).map(|_| [r.below(4), 0, 0]).collect();
    c.watch = (0..4)
        .map(|i| (players + i * 0xEA0 + 0xA64, 4))
        .chain([(0x7DE3E0, 4)])
        .collect();
    c
}
md_test!(t1_00067cd0, 0x1896_0020, build_67cd0, |h, _| {
    modes::match_flow::match_leader_update(h);
    None
});

// ---- 00080F40 mode flag mask applies ----
fn build_80f40(r: &mut Rng, p: &mut Pair) -> MCase {
    let targets = [
        0x80F82u32, 0x80F9E, 0x80FB9, 0x80FC3, 0x80FCD, 0x80FD7, 0x80FE1,
    ];
    for i in 0..7 {
        p.put32(0x81098 + 4 * i, targets[r.below(7) as usize]);
    }
    let mode = [
        0x64u32,
        0x65,
        0x66,
        0x67,
        0x68,
        0x69,
        0x6A,
        0x6B,
        0x6C,
        0x53,
        0x54,
        1,
        0,
        0xFFFF_FFFF,
        r.below(0x80),
        r.next(),
    ][r.below(16) as usize];
    p.put32(0x79094C, mode);
    p.put32(
        0x7DE458,
        [0u32, 0x10_0000, 0x20_0000, 0x40, 0x10_0040, r.next()][r.below(6) as usize],
    );
    p.put8(
        0x7DE455,
        [0xAu32, 0xA, 1, r.below(256)][r.below(4) as usize] as u8,
    );
    p.put8(0x7DE456, [0xCu32, r.below(256)][r.below(2) as usize] as u8);
    let entry = r.pool(POOL);
    p.put32(entry + 8, r.next());
    let bits = [
        0x100_0000u32,
        0x4000_0000,
        0x8000,
        0x1000_0000,
        0x20_1000,
        0x800,
        0x1_0000,
        0x400_0000,
        0x2000_0000,
        0x8000_0000,
        0x80_0000,
        0x2_0000,
        0x40_0000,
        0x20_0000,
        0x2000,
        0x800_0000,
    ];
    let mut mask = 0;
    for _ in 0..r.below(4) {
        mask |= bits[r.below(16) as usize];
    }
    if r.below(5) == 0 {
        mask = r.next();
    }
    if r.below(8) == 0 {
        mask = 0x4000_0000;
    }
    let mut c = mcase(r, 0x80F40, vec![mask]);
    c.script = vec![
        [[entry, 0][r.below(3).min(1) as usize], 0, 0],
        [r.below(2), 0, 0],
    ];
    c
}
md_test!(t1_00080f40, 0x1896_0021, build_80f40, |h, c| Some(
    modes::predicates::mode_flag_mask_applies(h, c.args[0])
));

// ---- 000B9D00 post result messages ----
fn build_b9d00(r: &mut Rng, p: &mut Pair) -> MCase {
    let character = r.pool(POOL);
    let brain = r.pool(POOL + 0x1000);
    p.put32(character + 0xBF0, brain);
    p.put32(
        brain + 0x28,
        [0x5041_5250u32, r.next()][r.below(2) as usize],
    );
    let slots = if r.below(8) == 0 { 0 } else { POOL + 0x3000 };
    p.put32(0x7B0C48, slots);
    for i in 0..4u32 {
        let object = if r.below(3) == 0 {
            0
        } else {
            r.pool(POOL + 0x6000)
        };
        if slots != 0 {
            p.put32(slots + i * 0x1584 + 8, object);
        }
        p.put32(object + 0x214, r.next());
    }
    p.put32(0x7DE45C, r.next());
    let mut c = mcase(r, 0xB9D00, vec![character]);
    c.script = (0..20)
        .map(|k| {
            [
                if k == 0 || k % 6 == 0 {
                    r.below(5)
                } else {
                    r.next()
                },
                0,
                0,
            ]
        })
        .collect();
    c.watch = vec![(0x7DE45C, 4)];
    c
}
// The original overwrites its own argument slot with 0 before the tail call (the notify
// argument of 67450). `ModeHost` has no caller frame, so the test mirrors that store.
md_test!(t1_000b9d00, 0x1896_0022, build_b9d00, |h, c| {
    let value = modes::match_flow::match_end_post_result_messages(h, c.args[0]);
    h.write32(STACK + 4, 0);
    Some(value)
});

// ---- T1910: float and frame-aware bodies ----
const FL: [u32; 14] = [
    0,
    0x8000_0000,
    0x3F80_0000,
    0xBF80_0000,
    0x3F00_0000,
    0x42C8_0000,
    0x43A0_0000,
    0x4000_0000,
    0x7F80_0000,
    0xFF80_0000,
    0x0000_0001,
    0x3DCC_CCCD,
    0x4B80_0000,
    0x3A83_126F,
];
fn flt(r: &mut Rng) -> u32 {
    if r.below(3) == 0 {
        FL[r.below(14) as usize]
    } else {
        (((r.next() % 4000) as i32 - 2000) as f32 / 64.0).to_bits()
    }
}
fn build_253020(r: &mut Rng, p: &mut Pair) -> MCase {
    let state = r.pool(POOL);
    p.put32(0x78B564, state);
    p.put32(state + 0x14, flt(r));
    p.put32(state + 0x2C, flt(r));
    p.put32(
        0x78B570,
        [r.below(4), r.below(4096), 0xFFFF_FFFF, r.next()][r.below(4) as usize],
    );
    p.put32(0x47D740, flt(r));
    p.put32(0x475CE0, flt(r));
    p.put32(
        0x475CD4,
        [flt(r), 0.5f32.to_bits(), 1.0f32.to_bits()][r.below(3) as usize],
    );
    p.put32(0x78B574, flt(r));
    p.put32(0x47D56C, flt(r));
    p.put32(0x47CFF4, flt(r));
    p.put32(0x78B578, flt(r));
    p.put32(0x475CC8, flt(r));
    for i in 0..2048u32 {
        let v = ((i as f32) * 0.01).sin();
        let bits = if i % 64 == 5 { flt(r) } else { v.to_bits() };
        for (k, b) in bits.to_le_bytes().into_iter().enumerate() {
            p.put8(0x7A38A0 + 4 * i + k as u32, b);
        }
    }
    let mut c = mcase(r, 0x253020, vec![]);
    c.watch = vec![(0x78B570, 4), (0x78B574, 8)];
    c
}
md_test!(t1_00253020, 0x1910_0001, build_253020, |h, _| {
    modes::motion::sine_oscillator_step(h);
    None
});
fn build_252ed0(r: &mut Rng, p: &mut Pair) -> MCase {
    let object = r.pool(POOL);
    let list = r.pool(POOL + 0x1000);
    let records = POOL + 0x2000 + 0x800;
    let count = r.below(3);
    p.put32(object + 4, list);
    p.put32(list + 4, records + 0x400 * 0); // record_list pointer: count word at its start
    let record_list = records;
    p.put32(list + 4, record_list);
    p.put32(record_list, count);
    let record = record_list.wrapping_sub(count * 0xC8);
    let position = r.pool(POOL + 0x4000);
    p.put32(record + 0x2D0, position);
    for k in 0..3 {
        p.put32(position + 4 * k, flt(r));
    }
    p.put32(list + 0x10, r.pool(POOL + 0x5000));
    p.put32(0x78B578, flt(r));
    p.put32(0x475D0C, flt(r));
    let table = r.pool(POOL + 0x6000);
    p.put32(0x78B560, table);
    for k in 0..6u32 {
        p.put32(table + k * 0x4C + 0x24, r.below(8));
        p.put32(table + k * 0x4C + 0x28, r.below(8));
    }
    let mut c = mcase(r, 0x252ED0, vec![]);
    c.regs[5] = object;
    c
}
md_test!(t1_00252ed0, 0x1910_0002, build_252ed0, |h, c| {
    modes::motion::apply_follower_rotation_to_matrices(h, c.regs[5]);
    None
});

fn build_67f00(r: &mut Rng, p: &mut Pair) -> MCase {
    let count = [0u32, 1, 2, 3, 4][r.below(5) as usize];
    p.put32(0x74C37C, count);
    p.put32(
        0x4E8CEC,
        [1u32, 2, 30, 0, r.below(1000), r.next()][r.below(6) as usize],
    );
    for a in [
        0x4E794C, 0x7A36C0, 0x475C88, 0x478910, 0x47890C, 0x4785E4, 0x475C78, 0x478398,
    ] {
        p.put32(a, flt(r));
    }
    p.put32(
        0x7A2BC8,
        r.next() & [0xF, 0xFF, 0x5, 0xFFFF_FFFF][r.below(4) as usize],
    );
    p.put8(
        0x7DE456,
        [0xCu32, 0xB, 0xC, r.below(256)][r.below(4) as usize] as u8,
    );
    p.put32(
        0x7DE318,
        [r.below(100), r.below(100000), 0xFFFF_FFFF, r.next()][r.below(4) as usize],
    );
    p.put32(
        0x7DE458,
        [0u32, 0x40, 0x800_0000, r.next()][r.below(4) as usize],
    );
    p.put32(0x7DE30C, [8u32, 2][r.below(2) as usize]);
    p.put32(0x7DE300, r.next());
    p.put32(0x4E7944, r.below(100));
    p.put32(0x790950, [0u32, 2, 4, 0xFFFF_FFFF][r.below(4) as usize]);
    p.put32(0x7B0C48, POOL + 0x2000);
    let mut watch = vec![(0x7DE4A0, 4), (0x7A36C0, 4), (0x7DE30C, 8)];
    for i in 0..4u32 {
        let object = if r.below(5) == 0 {
            0
        } else {
            POOL + 0x100 + i * 0x1000
        };
        p.put32(0x7A3580 + 4 * i, object);
        if object != 0 {
            p.put32(
                object + 8,
                [r.below(4), r.below(40), r.next()][r.below(3) as usize],
            );
            p.put32(object + 0x48, r.next());
            p.put32(
                object + 0xC6C,
                [0u32, 0x1000, r.next()][r.below(3) as usize],
            );
            p.put32(object + 0xA64, r.next());
            watch.push((object + 0xA64, 4));
            watch.push((object + 0xAD0, 4));
            watch.push((object + 0xB14, 4));
            watch.push((object + 0xB2C, 4));
        }
    }
    let mut c = mcase(r, 0x67F00, vec![]);
    c.script = (0..14).map(|_| [r.below(2), 0, 0]).collect();
    c.st0 = (0..14).map(|_| flt(r)).collect();
    c.watch = watch;
    c
}
md_test!(t1_00067f00, 0x1910_0003, build_67f00, |h, _| {
    let value = modes::match_flow::match_frame_update(h);
    // EAX is only defined when a tail call ran.
    h.trace
        .last()
        .filter(|row| row[0] == 0x66FC0 || row[0] == 0x231A20)
        .map(|_| value)
});

fn build_298ba0(r: &mut Rng, p: &mut Pair) -> MCase {
    let world = r.pool(POOL);
    p.put32(0x78B1E8, world);
    let projectile = r.pool(POOL + 0x2000);
    let owner = [1u32, 2, 3, 0][r.below(4) as usize];
    p.put32(world + 0x5AC, [owner, 1, 0][r.below(3) as usize]);
    p.put32(world + 0x5B4, [owner, 2, 0][r.below(3) as usize]);
    p.put32(world + 0x5B0, flt(r));
    p.put32(world + 0x5B8, flt(r));
    p.put32(0x4785E4, flt(r));
    p.put32(projectile + 4, owner);
    p.put32(projectile, [owner, owner, 9][r.below(3) as usize]);
    p.put32(
        projectile + 0x18,
        [0x17u32, 0x17, 5, r.next()][r.below(4) as usize],
    );
    let target = r.pool(POOL + 0x3000);
    for k in 0..3 {
        p.put32(target + 4 * k, flt(r));
    }
    p.put32(projectile + 0x28, target);
    let normal = r.pool(POOL + 0x4000);
    for k in 0..3 {
        p.put32(normal + 4 * k, flt(r));
    }
    p.put32(projectile + 0x30, normal);
    p.put32(projectile + 0x3C, r.next());
    p.put32(projectile + 0x24, r.next());
    p.put32(0x4760D0, flt(r));
    p.put32(0x475D10, flt(r));
    // The matrix callee (call index 1) writes the transformed vector into the locals at S+4..S+0xC,
    // where S = entry ESP - 0x28.
    let s = STACK - 0x28;
    let vector: Vec<[u32; 3]> = (0..3).map(|k| [1, s + 4 + 4 * k, flt(r)]).collect();
    let mut c = mcase(r, 0x298BA0, vec![projectile]);
    c.script = (0..6).map(|_| [r.below(2), 0, 0]).collect();
    c.writes = vector;
    c.watch = vec![(world + 0x5B0, 4), (world + 0x5B8, 4)];
    c
}
md_test!(t1_00298ba0, 0x1910_0004, build_298ba0, |h, _| {
    modes::motion::projectile_hit(h);
    None
});

fn build_254690(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(
        0x78B594,
        [6u32, 7, 8, 1, 2, 9, r.next()][r.below(7) as usize],
    );
    let track = if r.below(8) == 0 { 0 } else { r.pool(POOL) };
    p.put32(0x78B5B0, track);
    for k in 0..3 {
        p.put32(track + 0x64 + 4 * k, flt(r));
    }
    let entry = r.pool(POOL + 0x1000);
    p.put32(0x78B560, entry);
    p.put32(entry + 0x140, flt(r));
    p.put32(entry + 0x148, flt(r));
    for a in [
        0x49F118u32,
        0x49E0FC,
        0x49E058,
        0x47D208,
        0x47D560,
        0x4760DC,
        0x49F114,
        0x49F110,
        0x47D6E0,
        0x475CCC,
        0x4785D0,
        0x475C94,
        0x4788FC,
        0x47862C,
    ] {
        p.put32(a, flt(r));
    }
    // Keep the angle scale sane most of the time so sin and cos see ordinary arguments.
    if r.below(4) != 0 {
        p.put32(0x4785D0, 0x2F80_0000); // 2^-32
        p.put32(0x475C94, 0x40C9_0FDB); // 2*pi
        p.put32(0x4788FC, (1.0 + r.below(100) as f32 / 10.0).to_bits());
        p.put32(0x47862C, (r.below(60) as f32 / 4.0).to_bits());
    }
    let object = r.pool(POOL + 0x2000);
    for k in 0..3 {
        p.put32(object + 0x64 + 4 * k, flt(r));
    }
    let mut c = mcase(r, 0x254690, vec![]);
    c.regs[5] = object;
    c.script = (0..64u32)
        .map(|k| {
            if k % 2 == 0 {
                [r.next(), 0, 0]
            } else {
                [if k >= 40 { 1 } else { r.below(2) }, 0, 0]
            }
        })
        .collect();
    c.watch = vec![(0x78B580, 12)];
    c
}
md_test!(t1_00254690, 0x1910_0005, build_254690, |h, c| {
    modes::motion::pick_target_position(h, c.regs[5]);
    None
});

fn build_2548f0(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(0x78B590, [0u32, 1, 1, 1][r.below(4) as usize]);
    p.put32(0x78B594, [7u32, 8, 8, 1, 6][r.below(5) as usize]);
    let controls = r.pool(POOL);
    p.put32(0x78B564, controls);
    p.put32(controls + 0x10, r.next());
    p.put32(controls + 0x28, r.next());
    for a in [
        0x78B580u32,
        0x78B584,
        0x78B588,
        0x475DB4,
        0x478628,
        0x47862C,
        0x475D0C,
        0x475CE0,
        0x475CD4,
        0x475CAC,
        0x4760CC,
        0x4765F4,
        0x475C78,
        0x4760D0,
        0x478638,
        0x478900,
        0x47CFF4,
        0x47D820,
        0x47D730,
    ] {
        p.put32(a, flt(r));
    }
    if r.below(3) != 0 {
        // ordinary heading maths so the table lookup and thresholds are exercised
        p.put32(0x475D0C, 0x3F80_0000);
        p.put32(0x475CE0, 0x4400_0000);
        p.put32(0x475CD4, 0x0);
        p.put32(0x475DB4, (r.below(40) as f32).to_bits());
        p.put32(0x4765F4, 0x3F00_0000);
        p.put32(0x475C78, 0x3F80_0000);
        p.put32(0x4760D0, 0xBF80_0000);
        p.put32(0x4760CC, 0x3F80_0000);
        p.put32(0x478900, (r.below(50) as f32 / 10.0).to_bits());
        p.put32(0x47CFF4, (r.below(30) as f32 / 10.0).to_bits());
        p.put32(0x47D820, (r.below(30) as f32 / 10.0).to_bits());
        p.put32(0x47D730, (r.below(50) as f32 / 10.0).to_bits());
    }
    for i in 0..2048u32 {
        let bits = if r.below(64) == 0 {
            flt(r)
        } else {
            ((i as f32) * 0.00306796).sin().to_bits()
        };
        for (k, b) in bits.to_le_bytes().into_iter().enumerate() {
            p.put8(0x7A38A0 + 4 * i + k as u32, b);
        }
    }
    let object = r.pool(POOL + 0x1000);
    for k in 0..5 {
        p.put32(object + 0x64 + 4 * k, flt(r));
    }
    let mut c = mcase(r, 0x2548F0, vec![]);
    c.regs[0] = object;
    c.script = (0..4).map(|_| [0, 0, 0]).collect();
    c.st0 = (0..4)
        .map(|_| [flt(r), 0x3FC9_0FDB, 0x4049_0FDB, 0][r.below(4) as usize])
        .collect();
    c.watch = vec![(controls + 0x10, 4), (controls + 0x28, 4)];
    c
}
md_test!(t1_002548f0, 0x1910_0006, build_2548f0, |h, c| {
    modes::motion::steer_toward_target(h, c.regs[0]);
    None
});

fn build_66fc0(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(0x7DE30C, [8u32, 2, 3][r.below(3) as usize]);
    p.put32(0x7DE300, r.next());
    p.put32(0x7DE318, r.next());
    p.put32(0x6F11B8, r.next());
    p.put32(
        0x7DE458,
        [0u32, 0x40, 0x800_0000, 0x40, r.next()][r.below(5) as usize],
    );
    p.put32(
        0x7DE45C,
        [0u32, 0x8000, 0x1_0000, 0x1_8000, r.next()][r.below(5) as usize],
    );
    p.put8(
        0x7DE455,
        [0xAu32, 0xA, 1, r.below(256)][r.below(4) as usize] as u8,
    );
    p.put32(
        0x79094C,
        [0x64u32, 0x38, 0x65, r.next()][r.below(4) as usize],
    );
    let mut c = mcase(r, 0x66FC0, vec![]);
    c.script = (0..40).map(|_| [r.below(4), 0, 0]).collect();
    c.watch = vec![(0x7DE30C, 4), (0x7DE300, 4), (0x6F11B8, 4)];
    c
}
md_test!(t1_00066fc0, 0x1910_0007, build_66fc0, |h, _| {
    modes::match_flow::round_end_enter_state_8(h);
    None
});

fn build_158bc0(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(
        0x7DE458,
        [0u32, 0x2000_0000, 0, 0, r.next()][r.below(5) as usize],
    );
    p.put8(
        0x7DE455,
        [0xAu32, 0xA, 1, r.below(256)][r.below(4) as usize] as u8,
    );
    p.put32(
        0x79094C,
        [0x64u32, 0x65, 0x38, r.next()][r.below(4) as usize],
    );
    p.put32(0x7A2958, [0u32, 1, 3, 0xFFFF_FFFF][r.below(4) as usize]);
    p.put8(
        0x7DE402,
        [0u32, 1, 2, 3, 0x80, 0xFF][r.below(6) as usize] as u8,
    );
    p.put8(
        0x7DE456,
        [0x13u32, 0x13, r.below(256)][r.below(3) as usize] as u8,
    );
    p.put32(0x790950, r.below(3));
    for k in 0..8u32 {
        p.put8(0x7DE414 + k, r.next() as u8);
    }
    let mut c = mcase(r, 0x158BC0, vec![]);
    // 6BC00 (entry count) answers on call 1 and on the later loop checks; keep counts small.
    c.script = (0..30).map(|_| [r.below(4), 0, 0]).collect();
    // 6BC50 writes the two out values into the 8-byte frame at entry ESP - 8 and - 4.
    let base = STACK - 8;
    c.writes = (0..6u32)
        .flat_map(|k| [[2 + 3 * k, base, r.next()], [2 + 3 * k, base + 4, r.next()]])
        .collect();
    c
}
md_test!(t1_00158bc0, 0x1910_0008, build_158bc0, |h, _| {
    modes::setup::apply_mode_preloads(h);
    None
});

// ---------------------------------------------------------------------------------------------
// T2431: Story progression sequencer 0x65940 (Tier 1, FABRICATED inputs, weak evidence).
// Seeded cursor (0..=13), phase (0..=4), flags, game mode, participants, selector, a random 13-row
// cursor table, entry table bytes and scripted service results. Compared: call trace, the block
// (pool) and the cursor/phase cells. EAX is undefined residue and is not compared.
// ---------------------------------------------------------------------------------------------
const SEQ_BLOCK: u32 = POOL + 0x400;
const SEQ_WATCH: [(u32, u32); 2] = [(0x78ad48, 4), (0x78ad50, 4)];

fn build_65940(r: &mut Rng, p: &mut Pair) -> MCase {
    use modes::story_sequencer as seq;
    let none = u32::MAX;
    // Mixed ids: none, small known entries, any byte value.
    let id = |r: &mut Rng| [none, none, r.below(8), r.below(8), r.next()][r.below(5) as usize];
    for row in 0..13 {
        let base = seq::CursorRow::address(row);
        let level = [
            r.below(0x30),
            r.below(0x30),
            r.below(0x100),
            0x100 | r.below(0x100),
        ][r.below(4) as usize];
        for off in 0..0x44 {
            p.put8(base + off, r.next() as u8);
        }
        p.put8(base + 2, level as u8);
        p.put8(base + 3, (level >> 8) as u8);
        let (pre, post) = (id(r), id(r));
        p.put32(base + 0x1c, pre);
        p.put32(base + 0x20, post);
    }
    for entry in 0..256u32 {
        p.put8(seq::ENTRY_TABLE + entry * seq::ENTRY_STRIDE, r.next() as u8);
    }
    for i in 0..seq::BLOCK_BYTES as u32 {
        p.put8(SEQ_BLOCK + i, r.next() as u8);
    }
    p.put32(0x78ad48, r.below(14));
    p.put32(
        0x78ad50,
        [0u32, 1, 2, 3, 2, 2, 4, r.next()][r.below(8) as usize],
    );
    p.put32(0x7de458, flags(r) | [0u32, 0x400_0000][r.below(2) as usize]);
    p.put32(
        0x79094c,
        [0x68u32, 0x6a, 0x6a, 0x64, r.next()][r.below(5) as usize],
    );
    p.put32(
        0x7de45c,
        [r.next(), 0x8000, 0, r.next() & !0x8000][r.below(4) as usize],
    );
    p.put8(
        0x5236c4,
        [0u32, 1, 2, 3, 4, 0x7f, 0x80, 0xff, r.below(256)][r.below(9) as usize] as u8,
    );
    p.put8(0x78ad54, r.next() as u8);
    p.put8(0x78adaf, r.next() as u8);
    let mut c = mcase(r, 0x65940, vec![SEQ_BLOCK]);
    c.script = script(r, 200, &[0, 1]);
    c.watch = SEQ_WATCH.to_vec();
    c
}
md_test!(t1_00065940, 0x2431_0001, build_65940, |h, _| {
    modes::story_sequencer::story_progression_sequencer(h, SEQ_BLOCK);
    None
});

// T2446: Story-only mode activation arm of 0x670C0 (Tier 1, FABRICATED inputs, weak evidence).
fn build_670c0_story(r: &mut Rng, p: &mut Pair) -> MCase {
    for index in 0..40 {
        p.put32(0x7de340 + index * 4, r.next());
        p.put32(0x7de400 + index * 4, r.next());
    }
    p.put8(0x7de340 + 0x54, 0x65);
    p.put8(0x7de340 + 0x55, 10);
    p.put8(0x7de340 + 3, 0);
    p.put8(0x67270, 0);
    p.put32(0x67268, 0x671af);
    for address in [0x7de30c, 0x7de300, 0x7de458, 0x475c78] {
        p.put32(address, r.next());
    }
    let mut c = mcase(r, 0x670c0, vec![]);
    c.script = script(r, 64, &[0, 1, 2]);
    c
}
md_test!(t1_000670c0_story, 0x2446_670c, build_670c0_story, |h, _| {
    modes::story_activation::activate_story(h)
});

#[test]
fn t1_000670c0_story_mutation_is_detected() {
    let result = std::panic::catch_unwind(|| {
        drive(0x2446_670c_0001, build_670c0_story, |h, _| {
            modes::story_activation::activate_story(h);
            let value = h.read32(0x7de488) ^ 1;
            h.write32(0x7de488, value);
            None
        });
    });
    assert!(
        result.is_err(),
        "activation-state mutation escaped Tier 1 comparison"
    );
}

/// Mutation check: a port that differs from the original by one byte, one cursor step or one extra
/// service call must be reported by the same harness (guards against a comparison that cannot fail).
#[test]
fn t1_00065940_mutants_are_detected() {
    type Mutant = fn(&mut Host);
    let mutants: [(&str, Mutant); 3] = [
        ("level byte flipped", |h| {
            let value = h.view.byte(SEQ_BLOCK + 0x54) ^ 1;
            h.view.set(SEQ_BLOCK + 0x54, value);
        }),
        ("cursor advanced", |h| {
            let value = h.read32(0x78ad48).wrapping_add(1);
            h.write32(0x78ad48, value);
        }),
        ("extra service call", |h| {
            h.call(0x35de70, &[]);
        }),
    ];
    for (index, (name, mutate)) in mutants.into_iter().enumerate() {
        let outcome = std::panic::catch_unwind(|| {
            drive(0x2431_1000 + index as u64, build_65940, |h, _| {
                modes::story_sequencer::story_progression_sequencer(h, SEQ_BLOCK);
                mutate(h);
                None
            });
        });
        assert!(outcome.is_err(), "mutant not detected: {name}");
    }
}

// ---------------------------------------------------------------------------------------------
// T2431: the Story menu chain (Tier 1, FABRICATED inputs, weak evidence). The lifted bodies run
// on the C arena, the Rust ports on the identical Rust arena. Roots: 0x2D2C70 in menu mode 4 (the
// lifted sequencer 0x65940 runs inline), the page tick 0x2D3BF0 (with the lifted `_ftol` inline)
// and the UI callbacks 0x2CAFE0, 0x2CAD20 and 0x2BEAA0 against `tsfp_ui_input::frontend_handlers`.
// ---------------------------------------------------------------------------------------------
const MENU_BLOCK: u32 = 0x0078_ad58;
const MENU_PAGE: u32 = POOL + 0x600;

fn build_2d2c70_mode4(r: &mut Rng, p: &mut Pair) -> MCase {
    // Reuse the sequencer seeding (cursor table, entry table, flags), then retarget the block.
    let _ = build_65940(r, p);
    for i in 0..modes::story_sequencer::BLOCK_BYTES as u32 {
        p.put8(MENU_BLOCK + i, r.next() as u8);
    }
    p.put32(0x78ad2c, 4);
    p.put8(0x78adad, r.next() as u8);
    // The dispatch tables the lifted body reads: mode 4 maps through byte-map slot 3.
    p.put8(0x2d389c + 3, 5);
    p.put32(0x2d386c + 5 * 4, 0x2d2f03);
    let mut c = mcase(r, 0x2D2C70, vec![]);
    c.script = script(r, 200, &[0, 1]);
    c.watch = vec![
        (MENU_BLOCK, modes::story_sequencer::BLOCK_BYTES as u32),
        (0x78ad48, 4),
        (0x78ad50, 4),
        (0x78adec, 4),
    ];
    c
}
md_test!(
    t1_002d2c70_mode4,
    0x2431_0002,
    build_2d2c70_mode4,
    |h, _| {
        modes::story_menu::setup_mode_4(h);
        None
    }
);

fn build_2d3bf0(r: &mut Rng, p: &mut Pair) -> MCase {
    p.put32(
        0x4e8cec,
        [0u32, 1, 2, 60, r.next(), r.below(120)][r.below(6) as usize],
    );
    for address in [0x475c78, 0x4e7948, 0x76004c, 0x475ccc, 0x476688, 0x4788fc] {
        p.put32(address, flt(r));
    }
    let size = |r: &mut Rng| {
        [r.below(2000), r.below(2000).wrapping_neg(), 0, 1, r.next()][r.below(5) as usize]
    };
    p.put32(0x4c0458, size(r));
    p.put32(0x4c045c, size(r));
    p.put32(
        0x4e8cf0,
        [r.below(2000), r.next(), 720, 480][r.below(4) as usize],
    );
    p.put32(0x78ad2c, [1u32, 1, 4, 0, 14, r.next()][r.below(6) as usize]);
    p.put32(0x7de30c, [1u32, 1, 0, 2, r.next()][r.below(5) as usize]);
    let mut c = mcase(r, 0x2D3BF0, vec![MENU_PAGE]);
    // Row 0 is 0x70D90 (finished test), later rows include 0x3C420 (only AL is tested).
    c.script = (0..8)
        .map(|_| [r.word(&[0, 1, 0x100]), r.next(), r.next()])
        .collect();
    c.watch = vec![(0x76004c, 4)];
    c
}
md_test!(t1_002d3bf0, 0x2431_0003, build_2d3bf0, |h, _| {
    modes::story_menu::story_page_tick(h, MENU_PAGE);
    None
});

/// `GuestUi` over the differential host, so the Rust UI handlers run on the Rust arena.
struct UiHost<'a, 'b>(&'a mut Host<'b>);
impl tsfp_ui_input::native_page::GuestUi for UiHost<'_, '_> {
    fn read32(&self, va: u32) -> Option<u32> {
        Some(self.0.read32(va))
    }
    fn write32(&mut self, va: u32, value: u32) -> bool {
        self.0.write32(va, value);
        true
    }
    fn call(&mut self, va: u32, args: &[u32]) -> Option<u32> {
        Some(self.0.call(va, args))
    }
}

/// The lifted callbacks play their click as `0x45390` plus `0x45820` (the body of `0x21780`);
/// the Rust handler calls `0x21780(0xD1E)`. Both are compared as the single `0x21780` row.
fn normalise_click(trace: &mut Vec<Row>) {
    let mut out = Vec::with_capacity(trace.len());
    let mut index = 0;
    while index < trace.len() {
        if trace[index][0] == 0x45390 && trace.get(index + 1).is_some_and(|r| r[0] == 0x45820) {
            out.push([0x21780, 0xd1e, 0, 0]);
            index += 2;
        } else {
            out.push(trace[index]);
            index += 1;
        }
    }
    *trace = out;
}

fn build_story_callback(va: u32) -> impl FnMut(&mut Rng, &mut Pair) -> MCase {
    move |r, p| {
        p.put32(
            MENU_PAGE + 0x18c,
            [r.below(13), r.below(3), r.next()][r.below(3) as usize],
        );
        for (address, value) in [
            (0x7613e4, r.next()),
            (0x78ad2c, r.next()),
            (0x78ad48, r.next()),
            (0x78ae08, r.next()),
            (0x78ad54, r.next()),
            (0x78ae28, r.next()),
            (0x78ae1c, r.next()),
        ] {
            p.put32(address, value);
        }
        let mut c = mcase(r, va, vec![MENU_PAGE]);
        c.script = script(r, 8, &[0, 1]);
        c.watch = vec![
            (0x78ad2c, 4),
            (0x78ad48, 4),
            (0x78ae08, 4),
            (0x78ad54, 4),
            (0x78ae28, 4),
            (0x78ae1c, 4),
        ];
        c
    }
}

/// Like `drive` for the UI callbacks: the C trace is click-normalised before the comparison, so
/// this runs its own loop (the shared `drive` compares raw traces).
fn drive_story_callback(seed: u64, va: u32, handler: u32) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut r = Rng(seed);
    let mut build = build_story_callback(va);
    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        p.log.clear();
        let c = build(&mut r, p);
        let logged: Vec<u32> = p.log.drain(..).collect();
        let (_, mut ctrace) = run_c(p, &c);
        normalise_click(&mut ctrace);
        let mut view = View(p.rust);
        let mut host = Host {
            view: &mut view,
            script: c.script.clone(),
            st0: vec![],
            writes: vec![],
            trace: vec![],
        };
        let handled = tsfp_ui_input::frontend_handlers::dispatch_selected(
            &mut UiHost(&mut host),
            tsfp_ui_input::native_page::PageRef(MENU_PAGE),
            handler,
        );
        assert!(handled, "handler {handler:x} not routed");
        let rtrace = host.trace;
        let mut why = vec![];
        if ctrace != rtrace {
            why.push(format!("trace C {ctrace:x?} Rust {rtrace:x?}"));
        }
        why.extend(mem_diff(p, &logged, &c.watch));
        assert!(
            why.is_empty(),
            "root {va:08X} seed {seed:#x} case {index}: {why:?}"
        );
    }
}

#[test]
fn t1_002cafe0() {
    drive_story_callback(0x2431_0004, 0x2CAFE0, 0x2CAFE0);
}
#[test]
fn t1_002cad20() {
    drive_story_callback(0x2431_0005, 0x2CAD20, 0x2CAD20);
}
#[test]
fn t1_002beaa0() {
    drive_story_callback(0x2431_0006, 0x2BEAA0, 0x2BEAA0);
}

/// Mutation checks for the chain harness: a mode 4 port with a stale phase or an uncleared block
/// link and a tick with a flipped scroll sign must each be reported by the same harness.
#[test]
fn t1_story_menu_mutants_are_detected() {
    type Mutant = fn(&mut Host);
    let mode4: [(&str, Mutant); 2] = [
        ("phase left stale", |h| h.write32(0x78ad50, 7)),
        ("block link not cleared", |h| h.write32(0x78adec, 1)),
    ];
    for (index, (name, mutate)) in mode4.into_iter().enumerate() {
        let outcome = std::panic::catch_unwind(|| {
            drive(0x2431_2000 + index as u64, build_2d2c70_mode4, |h, _| {
                modes::story_menu::setup_mode_4(h);
                mutate(h);
                None
            });
        });
        assert!(outcome.is_err(), "mutant not detected: {name}");
    }
    let outcome = std::panic::catch_unwind(|| {
        drive(0x2431_2010, build_2d3bf0, |h, _| {
            modes::story_menu::story_page_tick(h, MENU_PAGE);
            let value = h.read32(0x76004c) ^ 0x8000_0000;
            h.write32(0x76004c, value);
            None
        });
    });
    assert!(outcome.is_err(), "mutant not detected: scroll sign");
}

// ---------------------------------------------------------------------------------------------
// T1703 (ladder 0, Story L1 missions-lane crash): the c2r port of the timed effect entry table
// update 0x216230 (`t2040_missions_3`) against the PRIVATE LIFTED ORIGINAL on seeded L1-shaped
// state. Tier 1, FABRICATED inputs (effect type table, entry table, player record, globals and
// callee results are generated, not captured from a live L1 run), weak evidence. Compared: the
// call trace (callee, first three stack words), the pool (entry table, player and tag records),
// the argument frame and every seeded global. Callees write no memory (scripted EAX/ECX/EDX,
// ST0 for 0x153BD0).
// ---------------------------------------------------------------------------------------------
mod timed_effects_t1703 {
    use super::*;
    use tsfp_ports::{
        abi::{ExtRegisters, Fpu, GuestCall, Xmm},
        c2r::t2040_missions_3,
        memory::GuestMemory as PortMemory,
    };

    const TABLE: u32 = POOL + 0x1000;
    const PLAYER: u32 = POOL + 0x2000;
    const TAG_A: u32 = POOL + 0x2400;
    const TAG_B: u32 = POOL + 0x2800;
    const TYPES: u32 = 0x0050_7a18;
    const TYPE_STRIDE: u32 = 0x44;
    const TYPE_COUNT: u32 = 8;

    struct PView(*mut u8);
    impl PortMemory for PView {
        fn read8(&self, address: u32) -> u8 {
            unsafe { *self.0.add(address as usize) }
        }
        fn write8(&mut self, address: u32, value: u8) {
            unsafe { *self.0.add(address as usize) = value }
        }
    }

    struct Services<'a, 'b> {
        host: &'a mut Host<'b>,
        mem: PView,
    }
    impl GuestCall for Services<'_, '_> {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            if va == 0x3c848c {
                // The lifted `_ftol` runs inline in the reference: ST0 truncated into EAX, float bits in EDX.
                let top = regs.fpu.st(0);
                regs.eax = (top as i64) as u32;
                regs.edx = (top as f32).to_bits();
                regs.fpu.pop();
                return;
            }
            let argc = unsafe { md_argc(va) };
            assert!(argc >= 0, "unknown callee {va:x}");
            let args: Vec<u32> = (0..argc as u32)
                .map(|i| self.mem.read32(regs.esp.wrapping_add(4 * i)))
                .collect();
            let x87 = va == 0x153bd0;
            let argument = regs.fpu.st(0) as f32;
            let index = self.host.record(va, &args);
            let row = self.host.script.get(index).copied().unwrap_or([0; 3]);
            regs.eax = row[0];
            regs.ecx = row[1];
            regs.edx = row[2];
            if x87 {
                // The ST0 argument of 0x153BD0 is stale x87 residue in this harness (the lifted `_ftol` is
                // not slot-exact in the adapter), so it is not compared (see modes_lifted.rs).
                let _ = argument;
                regs.fpu.pop();
                let bits = self.host.st0.get(index).copied().unwrap_or(0);
                regs.fpu.push(f32::from_bits(bits) as f64);
            }
        }
    }

    fn registers(c: &MCase) -> ExtRegisters {
        ExtRegisters {
            eax: c.regs[0],
            ecx: c.regs[1],
            edx: c.regs[2],
            esp: STACK,
            ebx: c.regs[3],
            ebp: c.regs[6],
            esi: c.regs[4],
            edi: c.regs[5],
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x37f,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        }
    }

    fn float(r: &mut Rng, span: u32) -> u32 {
        let value = (r.below(span * 2 + 1) as i32 - span as i32) as f32 / 8.0;
        value.to_bits()
    }

    fn build(r: &mut Rng, p: &mut Pair) -> MCase {
        for t in 0..TYPE_COUNT {
            let base = TYPES + t * TYPE_STRIDE;
            for word in 0..(TYPE_STRIDE / 4) {
                let value = float(r, 400);
                p.put32(base + word * 4, value);
            }
            // +0 u16 slot count (>= 1: the original divides by it), +2 u8 kind, +3 u8 flag,
            // +0x34 divisor (non-zero), +0x38/+0x3C scale factors.
            p.put8(base, 1 + r.below(4) as u8);
            p.put8(base + 1, 0);
            p.put8(base + 2, [0u8, 1, 2, 3, 8, 9][r.below(6) as usize]);
            p.put8(base + 3, r.below(2) as u8);
            p.put32(base + 0x34, (1.0f32 + r.below(16) as f32 / 4.0).to_bits());
            p.put32(base + 0x38, (r.below(8) as f32 / 4.0).to_bits());
            p.put32(base + 0x3c, (r.below(8) as f32 / 4.0).to_bits());
        }
        for (address, value) in [
            (0x0047_e2b4u32, 1.0e7f32),
            (0x0047_85c8, 1.5),
            (0x0047_d01c, 2.5),
            (0x0047_5c78, 16.0),
            (0x004e_794c, 0.5),
            (0x0047_5cd4, 0.25),
            (0x0047_5c90, 3.0),
            (0x0047_5d2c, 40.0),
            (0x0047_5ce0, 0.125),
        ] {
            p.put32(address, value.to_bits());
        }
        p.put32(0x004e_8cec, 1 + r.below(120));
        for index in 0..0x800u32 {
            let value = float(r, 64);
            p.put32(0x007a_38a0 + index * 4, value);
        }
        for word in 0..0x100u32 {
            for base in [PLAYER, TAG_A, TAG_B] {
                let value = float(r, 4000);
                p.put32(base + word * 4, value);
            }
        }
        p.put32(0x007b_0c48, PLAYER);
        p.put32(
            0x0079_094c,
            [0x1au32, 2, 0x64, 1, 5, 0x65][r.below(6) as usize],
        );
        p.put8(
            0x007d_e455,
            if r.below(3) == 0 {
                10
            } else {
                r.below(12) as u8
            },
        );
        p.put32(0x007d_e30c, 1 + r.below(3));
        let count = 1 + r.below(4);
        for entry in 0..count {
            let base = TABLE + entry * 0x28;
            for word in 0..10u32 {
                let value = float(r, 400);
                p.put32(base + word * 4, value);
            }
            p.put8(base, r.below(TYPE_COUNT) as u8);
            p.put8(base + 1, 0);
            p.put8(base + 2, r.below(3) as u8);
            p.put8(base + 3, 0);
        }
        let mut c = mcase(r, 0x216230, vec![TABLE, count]);
        let specials = [0u32, TAG_A, TAG_B];
        c.script = (0..32)
            .map(|_| {
                [
                    specials[r.below(3) as usize]
                        | (r.next() & 0x7fff) * u32::from(r.below(2) == 0),
                    r.next(),
                    r.next(),
                ]
            })
            .collect();
        c.st0 = (0..32).map(|_| float(r, 100)).collect();
        c.watch = vec![(TABLE, 0x28 * 4)];
        c
    }

    #[test]
    fn timed_effect_update_216230_c2r_matches_lifted_original() {
        drive(0x1703_0216, build, |h, c| {
            let mem = PView(h.view.0);
            let mut regs = registers(c);
            let mut services = Services { host: h, mem };
            let mut view = PView(services.mem.0);
            assert!(t2040_missions_3::run(
                0x216230,
                &mut view,
                &mut services,
                &mut regs
            ));
            None
        });
    }
}
