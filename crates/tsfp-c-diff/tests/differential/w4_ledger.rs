// SPDX-License-Identifier: GPL-3.0-or-later
// ======== T2048 slices L and I: ledgers and init (crates/tsfp-engine-memory/src/ledger.rs) ========
// Roots 3E440 3E4E0 3E560 3E5A0 3E650 3E740 3E930 run for real on the C side (3E740 also runs the
// lifted 3E240 and 3DFA0); 389B0, 3526E0 and 1CDAE0 are recorded stubs. Compared per case: EAX where
// meaningful, the ordered callee trace (target, first argument), the window 0x6B7A00..0x6B8400 (all
// ledger tables and allocator globals), the pool window, the argument frame, every seeded address
// and the extra ranges a root writes (kind 1 region for 3E650, the new top block for 3E740).
use super::*;
use tsfp_engine_memory::ledger::{InitHooks, Ledgers};

const LEDGER_WINDOW: (u32, u32) = (0x6B7A00, 0xA00);

#[derive(Default)]
struct InitHost(Vec<(u32, u32)>);
impl ArenaHooks for InitHost {
    fn out_of_memory(&mut self, argument: u32) {
        self.0.push((0x389B0, argument));
    }
}
impl InitHooks for InitHost {
    fn callout(&mut self, target: u32) {
        self.0.push((target, 0));
    }
}

fn fill(p: &mut Pair, start: u32, len: u32, byte: u8) {
    for off in 0..len {
        p.put8(start + off, byte);
    }
}

type Built = (W4Case, Vec<(u32, u32)>);

fn ledger_drive(
    seed: u64,
    ret: bool,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> Built,
    port: impl Fn(&mut Ledgers<AMem>, &mut InitHost, u32) -> u32,
) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut r = Rng(seed);
    let (mut with_calls, mut nonzero) = (0usize, 0usize);
    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        zero(p, LEDGER_WINDOW.0, LEDGER_WINDOW.1);
        p.log.clear();
        let (c, extra) = build(&mut r, p);
        let logged: Vec<u32> = p.log.drain(..).collect();
        let (cregs, ctrace) = run_c(p, &c);
        let c_calls: Vec<(u32, u32)> = ctrace.iter().map(|e| (e[0], e[3])).collect();
        let mut view = View(p.rust);
        let mut host = InitHost::default();
        let argument = rd(&View(p.rust), STACK + 4);
        let eax = port(
            &mut Ledgers {
                memory: &mut AMem(&mut view),
            },
            &mut host,
            argument,
        );
        let mut why = vec![];
        if ret && cregs[0] != eax {
            why.push(format!("eax C {:x} Rust {eax:x}", cregs[0]));
        }
        if c_calls != host.0 {
            why.push(format!("calls C {c_calls:x?} Rust {:x?}", host.0));
        }
        let top = rd(&View(p.c), 0x6B8388);
        let mut ranges = vec![LEDGER_WINDOW, (top, 16)];
        ranges.extend(extra);
        for (start, len) in ranges {
            // Fast path: identical slices need no byte walk (extras can be 0x55000 bytes).
            if start.checked_add(len).is_some()
                && unsafe {
                    std::slice::from_raw_parts(p.c.add(start as usize), len as usize)
                        == std::slice::from_raw_parts(p.rust.add(start as usize), len as usize)
                }
            {
                continue;
            }
            for off in 0..len {
                let a = start.wrapping_add(off);
                if p.get(false, a) != p.get(true, a) {
                    why.push(format!(
                        "memory {a:08x}: C {:02x} Rust {:02x}",
                        p.get(false, a),
                        p.get(true, a)
                    ));
                    break;
                }
            }
        }
        why.extend(mem_diff(p, &logged, c.va));
        assert!(
            why.is_empty(),
            "root {:08X} seed {seed:#x} case {index}: {why:?}",
            c.va
        );
        with_calls += usize::from(!c_calls.is_empty());
        nonzero += usize::from(cregs[0] != 0);
    }
    eprintln!(
        "w4 ledger {seed:#x}: {SEEDS} cases, {with_calls} with callouts, {nonzero} nonzero eax"
    );
}

const ROW_SIZES: [u32; 6] = [0x10, 0x20, 0x30, 0x40, 0x80, 0x1230];

/// Ledger table at `rows` (stride 8) over block base `base`; returns the row pointers and total.
fn ledger_rows(r: &mut Rng, p: &mut Pair, rows: u32, base: u32, count: u32) -> (Vec<u32>, u32) {
    let (mut pointers, mut offset) = (vec![], 0u32);
    for i in 0..count {
        let size = if r.below(12) == 0 {
            [0u32, 0x8000_0010, r.next()][r.below(3) as usize]
        } else {
            ROW_SIZES[r.below(6) as usize]
        };
        let pointer = if r.below(3) == 0 {
            0
        } else {
            base.wrapping_add(offset)
        };
        p.put32(rows + 8 * i, pointer);
        p.put32(rows + 8 * i + 4, size);
        pointers.push(pointer);
        offset = offset.wrapping_add(size);
    }
    (pointers, offset)
}

fn primary_state(r: &mut Rng, p: &mut Pair) -> Vec<u32> {
    let base = 0x0100_0000 + 0x1000 * r.below(8);
    let count = [0u32, 1, 2, 3, 5, 8, r.below(10)][r.below(7) as usize];
    let (pointers, offset) = ledger_rows(r, p, 0x6B7B68, base, count);
    p.put32(0x6B836C, base);
    p.put32(0x6B8394, if r.below(4) == 0 { r.next() } else { offset });
    p.put32(0x6B8398, count);
    p.put32(
        0x4C81B4,
        if r.below(6) == 0 {
            0
        } else {
            [1u32, r.next()][r.below(2) as usize]
        },
    );
    pointers
}

fn secondary_state(r: &mut Rng, p: &mut Pair) -> Vec<u32> {
    let base = 0x0200_0000 + 0x1000 * r.below(8);
    let count = [0u32, 1, 2, 3, 5, 8][r.below(6) as usize];
    let (pointers, offset) = ledger_rows(r, p, 0x6B7B18, base, count);
    p.put32(0x6B8380, base);
    p.put32(0x6B8368, if r.below(4) == 0 { r.next() } else { offset });
    p.put32(0x6B838C, count);
    pointers
}

fn block_arg(r: &mut Rng, pointers: &[u32]) -> u32 {
    match r.below(6) {
        0 => r.next(),
        1 => 0,
        2 if !pointers.is_empty() => pointers[pointers.len() - 1],
        _ if !pointers.is_empty() => pointers[r.below(pointers.len() as u32) as usize],
        _ => 0x0100_0010,
    }
}

#[test]
fn lifted_0003e440() {
    ledger_drive(
        0x3E440,
        true,
        |r, p| {
            primary_state(r, p);
            let s = [
                0u32,
                1,
                0xF,
                0x10,
                0x11,
                0x20,
                0x30,
                0x40,
                0x7F,
                r.below(0x2000),
                0xFFFF_FFF5,
            ][r.below(11) as usize];
            (case(r, 0x3E440, vec![s]), vec![])
        },
        |l, _, s| l.allocate_primary(s).unwrap(),
    );
}

#[test]
fn lifted_0003e4e0() {
    ledger_drive(
        0x3E4E0,
        false,
        |r, p| {
            let ptrs = primary_state(r, p);
            let b = block_arg(r, &ptrs);
            (case(r, 0x3E4E0, vec![b]), vec![])
        },
        |l, _, b| {
            l.release_primary(b).unwrap();
            0
        },
    );
}

#[test]
fn lifted_0003e560() {
    ledger_drive(
        0x3E560,
        true,
        |r, p| {
            secondary_state(r, p);
            let s = [0u32, 0x10, 0x25, r.below(0x2000), r.next()][r.below(5) as usize];
            (case(r, 0x3E560, vec![s]), vec![])
        },
        |l, _, s| l.append_secondary(s).unwrap(),
    );
}

#[test]
fn lifted_0003e5a0() {
    ledger_drive(
        0x3E5A0,
        true,
        |r, p| {
            let ptrs = secondary_state(r, p);
            let b = block_arg(r, &ptrs);
            (case(r, 0x3E5A0, vec![b]), vec![])
        },
        |l, _, b| l.release_secondary(b).unwrap(),
    );
}

#[test]
fn lifted_0003e930() {
    let mut boundary_index = 0usize;
    ledger_drive(
        0x3E930,
        false,
        |r, p| {
            arena_state(r, p);
            let (high, used) = [
                (0x20, 0x20),
                (0x20, 0x21),
                (0x7fff_ffff, 0x8000_0000),
                (0x8000_0000, 1),
            ]
            .get(boundary_index)
            .copied()
            .unwrap_or_else(|| (r.below(0x600), r.below(0x900)));
            boundary_index += 1;
            let first = POOL + 0x200;
            p.put32(0x6B83A0, first);
            p.put32(0x6B83A4, POOL + 0x800);
            p.put32(0x6B83B0, high);
            p.put32(0x6B839C, first.wrapping_add(used));
            (case(r, 0x3E930, vec![]), vec![])
        },
        |l, _, _| {
            tsfp_engine_memory::service_slice::MemoryServices {
                memory: &mut *l.memory,
            }
            .swap_scratch()
            .unwrap();
            0
        },
    );
}

#[test]
fn lifted_0003df00() {
    // 0x3DF00 memory_regions_init(base, size): straight-line, no callees, no loops (so no hang
    // shapes to avoid). Wrapping arithmetic over random base and size, flag 0x4C81B4 zero or not.
    ledger_drive(
        0x3DF00,
        false,
        |r, p| {
            let base = [0u32, 0x0100_0000, 0x10001, 0xFFF6_0000, r.next()][r.below(5) as usize];
            let size =
                [0u32, 0x300000, 0x0400_0000, r.next(), r.below(0x100_0000)][r.below(5) as usize];
            fill(p, LEDGER_WINDOW.0, LEDGER_WINDOW.1, 0xAA);
            fill(p, 0x7E72D0, 0x10, 0xAA);
            p.put32(0x4C81B4, [0u32, 1, r.next()][r.below(3) as usize]);
            (case(r, 0x3DF00, vec![base, size]), vec![(0x7E72D0, 0x10)])
        },
        |l, _, base| {
            use tsfp_engine_memory::GuestMemory;
            let size = l.memory.read_u32(tsfp_types::GuestAddr(STACK + 8)).unwrap();
            tsfp_engine_memory::service_slice::MemoryServices {
                memory: &mut *l.memory,
            }
            .initialize_arena(base, size)
            .unwrap();
            0
        },
    );
}

#[test]
fn lifted_0003e650() {
    ledger_drive(
        0x3E650,
        false,
        |r, p| {
            let base = 0x0100_0000u32;
            let split = base + 0x9DC00;
            let old = [0u32, 1, r.next()][r.below(3) as usize];
            let tag = if r.below(2) == 0 {
                0x65u32
            } else {
                r.below(256)
            };
            let bit = if r.below(2) == 0 { 0x40u32 } else { 0 };
            let flags = bit | (r.next() & !0x40);
            let ledger_on = old != 0 || tag == 0x65;
            let rem = [0u32, 1, 3, 4, 5, 0x40, 0x103, r.below(0x400)][r.below(8) as usize];
            let total = rem
                + 0x9DC00
                + if ledger_on { 0x26_6000 } else { 0 }
                + if bit != 0 { 0x13_1800 } else { 0 };
            fill(p, LEDGER_WINDOW.0, LEDGER_WINDOW.1, 0xAA);
            fill(p, split, 0x500, 0xAA);
            p.put32(0x7DE394, tag | (r.next() << 8));
            p.put32(0x7DE398, flags);
            p.put32(0x7E72D4, base);
            p.put32(0x7E72D8, total);
            p.put32(0x4C81B4, old);
            (case(r, 0x3E650, vec![]), vec![(split, 0x500)])
        },
        |l, h, _| {
            l.bounds_compute(h).unwrap();
            0
        },
    );
}

#[test]
fn lifted_0003e740() {
    ledger_drive(
        0x3E740,
        false,
        |r, p| {
            arena_state(r, p);
            for (i, t) in [0x3E750u32, 0x3E799, 0x3E7A5, 0x3E7B1].iter().enumerate() {
                p.put32(0x3E810 + 4 * i as u32, *t);
            }
            p.put32(0x7DE454, [0x0Au32, 0, r.next()][r.below(3) as usize] << 8);
            p.put32(0x79094C, [0x64u32, 0x64, r.next()][r.below(3) as usize]);
            p.put32(0x6B83AC, r.next());
            p.put32(0x6B83B4, r.next());
            // arena_state leaves little main space so most cases report overflow; a third get
            // room for the 2 * size block so the clean path is covered too.
            if r.below(3) == 0 {
                p.put32(0x6B8384, 0x40_0000);
            }
            let mode = [0u32, 1, 2, 3, 4, 5, r.next()][r.below(7) as usize];
            (case(r, 0x3E740, vec![mode]), vec![])
        },
        |l, h, mode| {
            l.history_reset(h, mode).unwrap();
            0
        },
    );
}

#[test]
fn lifted_0003e820() {
    ledger_drive(
        0x3E820,
        false,
        |r, p| {
            arena_state(r, p);
            for (i, t) in [0x3E750u32, 0x3E799, 0x3E7A5, 0x3E7B1].iter().enumerate() {
                p.put32(0x3E810 + 4 * i as u32, *t);
            }
            p.put32(0x7DE454, [0x0Au32, 0, r.next()][r.below(3) as usize] << 8);
            p.put32(0x79094C, [0x64u32, 0x64, r.next()][r.below(3) as usize]);
            p.put32(0x6B83AC, r.next());
            p.put32(0x6B83B4, r.next());
            if r.below(3) == 0 {
                p.put32(0x6B8384, 0x40_0000);
            }
            let mode = [0u32, 1, 2, 3, 4, 5, r.next()][r.below(7) as usize];
            (case(r, 0x3E820, vec![mode]), vec![])
        },
        |l, h, mode| {
            l.history_reset(h, mode).unwrap();
            0
        },
    );
}

// ---- Boot-state sequences: the allocators run from a MEASURED boot snapshot, not a synthetic pool ----
// Fixture tests/fixtures/t2048_boot_alloc_state.txt: the real allocator globals, both ledgers (20 live
// primary rows) and the pool header of a hybrid host run (docs/t-memory-replace.md section 7). Each
// case reloads it into both arenas, then applies 1..7 random operations (pool alloc kinds 1 and 2 and
// free, both ledgers, arena kinds 0/1/2/4 and the LIFO tagged release) to the C original and to the
// Rust port, comparing result, callee trace and memory after EVERY operation.
use tsfp_engine_memory::pool::{Pool, PoolHooks};
use tsfp_engine_memory::{
    contract::{GameHeapContract, HeapKind, Layout},
    heap::GuestImageHeap,
};

const FIXTURE: &str = include_str!("../fixtures/t2048_boot_alloc_state.txt");
const BOOT_CALLBACK: u32 = 0x15BCC0;

#[derive(Default)]
struct BootHost {
    oom: Vec<u32>,
    reclaimed: Vec<u32>,
}
impl ArenaHooks for BootHost {
    fn out_of_memory(&mut self, argument: u32) {
        self.oom.push(argument);
    }
}
impl PoolHooks for BootHost {
    fn reclaim(&mut self, callback: u32, argument: u32) {
        assert_eq!(callback, BOOT_CALLBACK);
        self.reclaimed.push(argument);
    }
}

/// Parsed fixture: (address, bytes) per range.
fn boot_ranges() -> Vec<(u32, Vec<u8>)> {
    let mut ranges: Vec<(u32, Vec<u8>)> = vec![];
    for line in FIXTURE.lines().filter(|l| !l.starts_with('#')) {
        if let Some(rest) = line.strip_prefix("range 0x") {
            ranges.push((
                u32::from_str_radix(rest.split(' ').next().unwrap(), 16).unwrap(),
                vec![],
            ));
        } else if let Some((_, hex)) = line.split_once(": ") {
            let bytes = hex
                .split_whitespace()
                .map(|b| u8::from_str_radix(b, 16).unwrap());
            ranges.last_mut().unwrap().1.extend(bytes);
        }
    }
    ranges
}

fn assert_heap_contract_memory(
    p: &Pair,
    address: u32,
    size: u32,
    seed: usize,
    heap: HeapKind,
    phase: &str,
) {
    let mut check = |start: u32, len: u32| {
        for offset in 0..len {
            let address = start.wrapping_add(offset);
            assert_eq!(
                p.get(false, address),
                p.get(true, address),
                "{phase}: byte {address:08x}, seed {seed}, heap {heap:?}"
            );
        }
    };
    check(LEDGER_WINDOW.0, LEDGER_WINDOW.1);
    check(STACK, ARG_FRAME);
    if address != 0 {
        check(address.saturating_sub(0x40), size.saturating_add(0x100));
    }
}

#[test]
fn boot_state_allocator_sequences() {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let ranges = boot_ranges();
    let globals = &ranges[0].1;
    let word = |o: usize| u32::from_le_bytes(globals[o..o + 4].try_into().unwrap());
    let (pool, top) = (word(0x60), word(0x20));
    assert_eq!((pool, top), (0x41C2_4C00, 0x441B_4F70), "fixture drifted");
    let mut r = Rng(0x7_2048);
    let (mut ops_run, mut reclaims, mut oom_ops) = (0usize, 0usize, 0usize);
    for index in 0..SEEDS {
        zero(p, LEDGER_WINDOW.0, LEDGER_WINDOW.1);
        zero(p, pool, 0x3000);
        zero(p, top - 0x1000, 0x1010);
        zero(p, STACK, ARG_FRAME);
        for (start, bytes) in &ranges {
            for (i, b) in bytes.iter().enumerate() {
                p.put8(start + i as u32, *b);
            }
        }
        for (i, t) in [0x3DFBFu32, 0x3E032, 0x3E063, 0x3E08D, 0x3E07B]
            .iter()
            .enumerate()
        {
            p.put32(0x3E094 + 4 * i as u32, *t);
        }
        p.log.clear();
        let mut payloads: Vec<u32> = vec![];
        let mut tagged: Vec<u32> = vec![];
        let mut rows: Vec<u32> = vec![0x4460_D000, 0x4469_7000];
        for step in 0..1 + r.below(7) {
            zero(p, STACK, ARG_FRAME);
            let pick = r.below(10);
            let size = [0u32, 1, 0x20, 0x41, 0x100, r.below(0x1800)][r.below(6) as usize];
            let owner = 0x0090_7000 + 4 * r.below(32);
            let cb = if r.below(2) == 0 { 0 } else { BOOT_CALLBACK };
            let kind = [1u32, 1, 2][r.below(3) as usize];
            let victim = |r: &mut Rng, from: &[u32]| {
                if from.is_empty() || r.below(5) == 0 {
                    r.next() & 0x00FF_FFF0
                } else {
                    from[r.below(from.len() as u32) as usize]
                }
            };
            let (va, args): (u32, Vec<u32>) = match pick {
                0 | 1 => (0x3ED20, vec![size, kind, owner, cb]),
                2 => (0x3EAD0, vec![victim(&mut r, &payloads)]),
                3 => (0x3E440, vec![size]),
                4 => (0x3E4E0, vec![victim(&mut r, &rows)]),
                5 => (0x3E560, vec![size]),
                6 => (0x3E5A0, vec![victim(&mut r, &rows)]),
                7 => (0x3DFA0, vec![size, [0u32, 1, 2, 4][r.below(4) as usize]]),
                8 => (0x3DFA0, vec![size, 0]),
                _ => (0x3E1E0, vec![tagged.last().copied().unwrap_or(top)]),
            };
            let c = case(&mut r, va, args.clone());
            let (cregs, ctrace) = run_c(p, &c);
            let mut view = View(p.rust);
            let mut host = BootHost::default();
            let eax = match va {
                0x3ED20 => Pool {
                    memory: &mut AMem(&mut view),
                    hooks: &mut host,
                }
                .allocate_by_kind(args[0], args[1], args[2], args[3])
                .unwrap(),
                0x3EAD0 => {
                    Pool {
                        memory: &mut AMem(&mut view),
                        hooks: &mut host,
                    }
                    .free(args[0])
                    .unwrap();
                    cregs[0]
                }
                0x3DFA0 => Arena {
                    memory: &mut AMem(&mut view),
                    hooks: &mut host,
                }
                .alloc_round16(args[1], args[0])
                .unwrap(),
                0x3E1E0 => {
                    Arena {
                        memory: &mut AMem(&mut view),
                        hooks: &mut host,
                    }
                    .release_tagged_block(args[0])
                    .unwrap();
                    cregs[0]
                }
                _ => {
                    let mut l = Ledgers {
                        memory: &mut AMem(&mut view),
                    };
                    match va {
                        0x3E440 => l.allocate_primary(args[0]).unwrap(),
                        0x3E4E0 => {
                            l.release_primary(args[0]).unwrap();
                            cregs[0]
                        }
                        0x3E560 => l.append_secondary(args[0]).unwrap(),
                        _ => l.release_secondary(args[0]).unwrap(),
                    }
                }
            };
            let c_oom: Vec<u32> = ctrace
                .iter()
                .filter(|e| e[0] == 0x389B0)
                .map(|e| e[3])
                .collect();
            let c_reclaim: Vec<u32> = ctrace
                .iter()
                .filter(|e| e[0] == BOOT_CALLBACK)
                .map(|e| e[3])
                .collect();
            let mut why = vec![];
            if cregs[0] != eax {
                why.push(format!("eax C {:x} Rust {eax:x}", cregs[0]));
            }
            if c_oom != host.oom || c_reclaim != host.reclaimed {
                why.push(format!(
                    "calls C oom {c_oom:x?} reclaim {c_reclaim:x?} Rust {:x?} {:x?}",
                    host.oom, host.reclaimed
                ));
            }
            let top_now = rd(&View(p.c), 0x6B8388);
            for (start, len) in [
                LEDGER_WINDOW,
                (pool, 0x3000),
                (top_now, 16),
                (top - 0x1000, 0x1010),
            ] {
                for off in 0..len {
                    let a = start.wrapping_add(off);
                    if p.get(false, a) != p.get(true, a) {
                        why.push(format!(
                            "memory {a:08x}: C {:02x} Rust {:02x}",
                            p.get(false, a),
                            p.get(true, a)
                        ));
                        break;
                    }
                }
            }
            why.extend(mem_diff(p, &[], va));
            assert!(
                why.is_empty(),
                "boot sequence case {index} step {step} root {va:08X} args {args:x?}: {why:?}"
            );
            match va {
                0x3ED20 if eax != 0 => payloads.push(eax),
                0x3E440 | 0x3E560 if eax != 0 => rows.push(eax),
                0x3DFA0 if args[1] == 0 && eax != 0 => tagged.push(eax),
                0x3E1E0 => {
                    tagged.pop();
                }
                _ => {}
            }
            ops_run += 1;
            reclaims += c_reclaim.len();
            oom_ops += usize::from(!c_oom.is_empty());
        }
    }
    eprintln!(
        "w4 boot-state sequences: {SEEDS} cases, {ops_run} operations, {reclaims} reclaim callbacks, {oom_ops} operations with oom reports"
    );
}

/// Tier 1 for the typed game-heap boundary: each request runs through the corresponding lifted
/// allocator root and through `GameHeapContract` from the same controlled image. Arena globals use
/// the allocator suite's known-valid state; reclaimable and locked pool blocks start from the
/// captured boot fixture. The guest image remains address-stable storage; this checks the
/// Rust-owned service façade rather than claiming a native owner or an original typed-handle ABI.
#[test]
fn typed_game_heap_contract_matches_lifted_allocators() {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let ranges = boot_ranges();
    let globals = &ranges[0].1;
    let word = |o: usize| u32::from_le_bytes(globals[o..o + 4].try_into().unwrap());
    let (pool, top) = (word(0x60), word(0x20));
    let mut r = Rng(0x7_2048_2392);
    let alignments = [1u32, 2, 4, 8, 16, 32, 64];
    let sizes = [1u32, 16, 0x20, 0x41, 0x100, 0x400];

    for index in 0..SEEDS {
        zero(p, LEDGER_WINDOW.0, LEDGER_WINDOW.1);
        zero(p, pool, 0x3000);
        zero(p, top - 0x1000, 0x1010);
        zero(p, STACK, ARG_FRAME);
        for (start, bytes) in &ranges {
            for (offset, byte) in bytes.iter().enumerate() {
                p.put8(start + offset as u32, *byte);
            }
        }
        // The captured boot cursor can route an arena allocation through a null indirect target in
        // this standalone harness. Use the existing controlled arena state while retaining the
        // captured pool block table for the two pool classes.
        arena_state(&mut r, p);

        let choice = index % 7;
        let size = if index == 5 {
            // Choice 5 below is the locked pool. The measured allocator rounds with
            // wrapping u32 arithmetic, so this first locked-pool request wraps to zero.
            0xffff_ffc0
        } else if r.below(2) == 0 {
            sizes[r.below(sizes.len() as u32) as usize]
        } else {
            r.below(0x800) + 1
        };
        let (heap, alignment, va, kind) = match choice {
            0 => (
                HeapKind::TaggedArena,
                alignments[r.below(7) as usize],
                0x3E0B0,
                0,
            ),
            1 => (
                HeapKind::RegionB,
                alignments[r.below(7) as usize],
                0x3E0B0,
                1,
            ),
            2 => (
                HeapKind::RegionA,
                alignments[r.below(7) as usize],
                0x3E0B0,
                2,
            ),
            3 => (
                HeapKind::UncheckedArena,
                alignments[r.below(7) as usize],
                0x3E0B0,
                4,
            ),
            4 => (
                HeapKind::ReclaimablePool,
                alignments[r.below(6) as usize],
                0x3ED20,
                1,
            ),
            5 => (
                HeapKind::LockedPool,
                alignments[r.below(6) as usize],
                0x3ED20,
                2,
            ),
            _ => (HeapKind::Scratch, 1, 0x3E970, 0),
        };
        let layout = Layout::new(size, alignment).unwrap();
        let args = match va {
            0x3E0B0 => vec![alignment, size, kind],
            0x3ED20 => vec![size, kind, 0, 0],
            _ => vec![size],
        };
        p.log.clear();
        let c = case(&mut r, va, args);
        let (cregs, ctrace) = run_c(p, &c);
        let mut view = View(p.rust);
        let mut host = BootHost::default();
        let rust_result = GameHeapContract::allocate(
            &mut GuestImageHeap {
                memory: &mut AMem(&mut view),
                hooks: &mut host,
            },
            heap,
            layout,
        )
        .unwrap();
        let rust_address = rust_result.map_or(0, |allocation| allocation.address().0);
        assert_eq!(cregs[0], rust_address, "seed {index}, heap {heap:?}");

        let c_oom: Vec<u32> = ctrace
            .iter()
            .filter(|entry| entry[0] == 0x389B0)
            .map(|entry| entry[3])
            .collect();
        let c_reclaimed: Vec<u32> = ctrace
            .iter()
            .filter(|entry| entry[0] == BOOT_CALLBACK)
            .map(|entry| entry[3])
            .collect();
        assert_eq!(c_oom, host.oom, "OOM hooks, seed {index}, heap {heap:?}");
        assert_eq!(c_reclaimed, host.reclaimed, "reclaim hooks, seed {index}");

        // Bound the local memory window for the wrapping-size case; its result is a zero-sized
        // prefix allocation, so the nearby header/global comparison still covers every write.
        assert_heap_contract_memory(p, cregs[0], size.min(0x100), index, heap, "allocate");

        if let Some(allocation) = rust_result {
            let free_va = match heap {
                HeapKind::TaggedArena => Some(0x3E1E0),
                HeapKind::ReclaimablePool => Some(0x3EAD0),
                _ => None,
            };
            if let Some(free_va) = free_va {
                let free = case(&mut r, free_va, vec![allocation.address().0]);
                let (_, free_trace) = run_c(p, &free);
                let mut view = View(p.rust);
                let mut host = BootHost::default();
                GameHeapContract::free(
                    &mut GuestImageHeap {
                        memory: &mut AMem(&mut view),
                        hooks: &mut host,
                    },
                    allocation,
                )
                .unwrap();
                assert!(
                    !free_trace.iter().any(|entry| entry[0] == 0x389B0),
                    "unexpected OOM hook while freeing seed {index}"
                );
                assert_heap_contract_memory(p, cregs[0], size, index, heap, "free");
            }
        }
    }
}

/// Tier 1 for the native owner allocation transaction: the owner-aware API uses the same
/// reclaimable pool operation as lifted 0x3ED20, publishes a live generation only after success,
/// and stores the opaque token plus callback marker in the corresponding pool header. Inputs are
/// deterministic FABRICATED requests over the captured boot pool state; this does not claim that
/// a retail owner callback has been observed under pressure.
#[test]
fn typed_owned_pool_transaction_matches_lifted_allocator() {
    use tsfp_engine_memory::owner_lifecycle::{OwnerRegistry, allocate_owned_with_registry};

    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let ranges = boot_ranges();
    let globals = &ranges[0].1;
    let word = |o: usize| u32::from_le_bytes(globals[o..o + 4].try_into().unwrap());
    let (pool, top) = (word(0x60), word(0x20));
    let mut r = Rng(0x7_2048_3EBD0);
    let alignments = [1u32, 2, 4, 8, 16, 32];

    for index in 0..SEEDS {
        zero(p, LEDGER_WINDOW.0, LEDGER_WINDOW.1);
        zero(p, pool, 0x3000);
        zero(p, top - 0x1000, 0x1010);
        zero(p, STACK, ARG_FRAME);
        for (start, bytes) in &ranges {
            for (offset, byte) in bytes.iter().enumerate() {
                p.put8(start + offset as u32, *byte);
            }
        }
        arena_state(&mut r, p);

        let size = r.below(0x800) + 1;
        let alignment = alignments[r.below(alignments.len() as u32) as usize];
        let layout = Layout::new(size, alignment).unwrap();
        let mut registry = OwnerRegistry::new();
        let owner = registry.register_owner().unwrap();
        let token = registry.owner_token(owner).unwrap();
        let mut host = BootHost::default();
        let mut dispatches = 0;
        let result = {
            let mut view = View(p.rust);
            allocate_owned_with_registry(
                &mut AMem(&mut view),
                &mut host,
                &mut registry,
                owner,
                layout,
                1,
                |_, _| dispatches += 1,
            )
            .unwrap()
        };
        assert_eq!(dispatches, 0, "unexpected owner reclaim at seed {index}");
        let (rust_address, callback) = if let Some((id, allocation)) = result {
            assert!(registry.allocation_is_live(id));
            let address = allocation.address().0;
            let callback = u32::from_le_bytes([
                p.get(true, address - 0x10),
                p.get(true, address - 0x0F),
                p.get(true, address - 0x0E),
                p.get(true, address - 0x0D),
            ]);
            let stored_token = u32::from_le_bytes([
                p.get(true, address - 0x14),
                p.get(true, address - 0x13),
                p.get(true, address - 0x12),
                p.get(true, address - 0x11),
            ]);
            assert_eq!(stored_token, token.get());
            (address, callback)
        } else {
            (0, 0)
        };

        p.log.clear();
        let c = case(&mut r, 0x3ED20, vec![size, 1, token.get(), callback]);
        let (cregs, ctrace) = run_c(p, &c);
        assert_eq!(cregs[0], rust_address, "seed {index}");
        let c_oom: Vec<u32> = ctrace
            .iter()
            .filter(|entry| entry[0] == 0x389B0)
            .map(|entry| entry[3])
            .collect();
        let c_reclaimed: Vec<u32> = ctrace
            .iter()
            .filter(|entry| entry[0] == BOOT_CALLBACK)
            .map(|entry| entry[3])
            .collect();
        assert_eq!(c_oom, host.oom, "OOM hooks, seed {index}");
        assert_eq!(c_reclaimed, host.reclaimed, "reclaim hooks, seed {index}");
        assert_heap_contract_memory(
            p,
            rust_address,
            size,
            index,
            HeapKind::ReclaimablePool,
            "owned allocate",
        );
    }
}

// ---- Slice W: game_alloc_* wrappers (crates/tsfp-engine-memory/src/wrappers.rs) ----
// The C side runs the lifted wrapper with the lifted 3E440 / 3DFA0 / 3E430 nested. Ledger is forced
// enabled (a disabled ledger makes the original zero guest address 0.., which would trample the
// harness code tables on both sides). The ledger block region 0x01000000..+0x55000 is pre-filled
// with 0xAA so a missing or short zeroing is visible.
use tsfp_engine_memory::wrappers::AllocWrappers;

const WRAP_REGION: (u32, u32) = (0x0100_0000, 0x5_8000);
const STATE_BLOCK_TABLE: u32 = 0x0200_0000;
const STATE_BLOCK_READINESS: u32 = STATE_BLOCK_TABLE + 0x18 * 0x14 + 0x0C;
const SOUND_DESCRIPTOR_1B_READY: u32 = STATE_BLOCK_TABLE + 0x1B * 0x14 + 0x0C;
const SOUND_DESCRIPTOR_19_READY: u32 = STATE_BLOCK_TABLE + 0x19 * 0x14 + 0x0C;
const TIMER_DESCRIPTOR_READY: [u32; 5] = [
    STATE_BLOCK_TABLE + 0x156F * 0x14 + 0x0C,
    STATE_BLOCK_TABLE + 0x1570 * 0x14 + 0x0C,
    STATE_BLOCK_TABLE + 0x5F3 * 0x14 + 0x0C,
    STATE_BLOCK_TABLE + 0x5F4 * 0x14 + 0x0C,
    STATE_BLOCK_TABLE + 0x5F5 * 0x14 + 0x0C,
];

fn fast_fill(p: &mut Pair, start: u32, len: u32, byte: u8) {
    unsafe {
        std::ptr::write_bytes(p.c.add(start as usize), byte, len as usize);
        std::ptr::write_bytes(p.rust.add(start as usize), byte, len as usize);
    }
}

/// Primary-ledger state with the flag on and, a third of the time, a released row of exactly
/// `size` so the reuse path is exercised.
fn wrapper_ledger(r: &mut Rng, p: &mut Pair, sizes: &[u32]) {
    fast_fill(p, WRAP_REGION.0, WRAP_REGION.1, 0xAA);
    primary_state(r, p);
    p.put32(0x4C81B4, 1);
    let count = rd(&View(p.c), 0x6B8398);
    if count >= 2 && r.below(3) == 0 {
        let row = 0x6B7B68 + 8 * r.below(count - 1);
        p.put32(row, 0);
        p.put32(row + 4, sizes[r.below(sizes.len() as u32) as usize]);
    }
}

fn wrapper_test(
    va: u32,
    sizes: &'static [u32],
    arena: bool,
    extra: &'static [(u32, u32)],
    call: fn(&mut AllocWrappers<AMem, InitHost>),
) {
    ledger_drive(
        va as u64,
        false,
        move |r, p| {
            fast_fill(p, 0x6F1E00, 0x20, 0xAA);
            fast_fill(p, 0x54EF10, 0x20, 0xAA);
            fast_fill(p, 0x689170, 0x10, 0xAA);
            fast_fill(p, 0x68C100, 0x500, 0xAA);
            fast_fill(p, 0x68DD00, 0x100, 0xAA);
            fast_fill(p, 0x68C5D0, 0x20, 0xAA);
            wrapper_ledger(r, p, sizes);
            if arena {
                arena_state(r, p);
                p.put32(
                    0x54EF20,
                    [0u32, 0x800, 0x7F0, r.next()][r.below(4) as usize],
                );
            }
            let mut ranges = vec![WRAP_REGION];
            ranges.extend_from_slice(extra);
            (case(r, va, vec![]), ranges)
        },
        move |l, h, _| {
            call(&mut AllocWrappers {
                memory: &mut *l.memory,
                hooks: h,
            });
            0
        },
    );
}

const WRAP_GLOBALS: &[(u32, u32)] = &[
    (0x689170, 0x10),
    (0x68C100, 0x500),
    (0x68DD00, 0x100),
    (0x68C5D0, 0x20),
    (0x54EF10, 0x20),
    (0x6F1E00, 0x20),
];

#[test]
fn lifted_000323e0() {
    wrapper_test(0x323E0, &[0x9000], false, WRAP_GLOBALS, |w| {
        w.alloc_zeroed_689174().unwrap()
    });
}
#[test]
fn lifted_00036fc0() {
    wrapper_test(0x36FC0, &[0x4B000], false, WRAP_GLOBALS, |w| {
        w.alloc_zeroed_68c120().unwrap()
    });
}
#[test]
fn lifted_00037410() {
    wrapper_test(0x37410, &[0x20000], false, WRAP_GLOBALS, |w| {
        w.alloc_zeroed_68c5e0().unwrap()
    });
}
#[test]
fn lifted_00018ca0() {
    wrapper_test(0x18CA0, &[0x10], true, WRAP_GLOBALS, |w| {
        w.alloc_buffers_54ef14().unwrap()
    });
}

#[test]
fn lifted_00032a860() {
    ledger_drive(
        0x32A860,
        false,
        |r, p| {
            arena_state(r, p);
            p.put32(
                0x79094C,
                [0x66u32, 0x66, 0x67, 0x65, r.next()][r.below(5) as usize],
            );
            p.put32(0x778D38, 0xDEAD_BEEF);
            fast_fill(p, POOL + 0x1000, 0x2000, 0xAA);
            (
                case(r, 0x32A860, vec![]),
                vec![(POOL + 0x1000, 0x2000), (0x778D38, 4)],
            )
        },
        |l, h, _| {
            AllocWrappers {
                memory: &mut *l.memory,
                hooks: h,
            }
            .alloc_save_slot_buffer_778d38()
            .unwrap();
            0
        },
    );
}

#[test]
fn lifted_00012c800() {
    ledger_drive(
        0x12C800,
        true,
        |r, p| {
            let count = [0u32, 1, 15, 16, 17, r.below(18)][r.below(6) as usize];
            p.put32(0x7490A0, count);
            (case(r, 0x12C800, vec![]), vec![(0x7490A0, 4)])
        },
        |l, h, _| {
            AllocWrappers {
                memory: &mut *l.memory,
                hooks: h,
            }
            .allocate_slot_745938_max17()
            .unwrap()
        },
    );
}

#[test]
fn lifted_0002fe200() {
    ledger_drive(
        0x2FE200,
        true,
        |r, p| {
            arena_state(r, p);
            fast_fill(p, POOL, 0x20000, 0xAA);
            p.put32(0x784014, 0xDEAD_BEEF);
            p.put32(0x765C20, 0xDEAD_BEEF);
            p.put32(0x765C24, 0xDEAD_BEEF);
            p.put32(0x765C28, 0xDEAD_BEEF);
            p.put32(0x765C30, 0xDEAD_BEEF);
            let extra = vec![(POOL, 0x20000), (0x765C20, 0x14), (0x784014, 4)];
            (case(r, 0x2FE200, vec![]), extra)
        },
        |l, h, _| {
            AllocWrappers {
                memory: &mut *l.memory,
                hooks: h,
            }
            .alloc_mapedit_buffers()
            .unwrap();
            0
        },
    );
}

#[test]
fn lifted_000386e0() {
    wrapper_test(
        0x386E0,
        &[0x35A8, 0xAA0, 0xFA0, 0x2080],
        false,
        WRAP_GLOBALS,
        |w| w.alloc_messenger_tables().unwrap(),
    );
}

#[test]
fn lifted_00069ee0() {
    // cdecl(force): the original takes the argument from the frame.
    ledger_drive(
        0x69EE0,
        false,
        |r, p| {
            fast_fill(p, 0x6F1E00, 0x20, 0xAA);
            wrapper_ledger(r, p, &[0x32000]);
            p.put32(0x4C81B4, [1u32, 1, 0, r.next()][r.below(4) as usize]);
            let buffer = if r.below(2) == 0 {
                0
            } else {
                0x0100_0000 + 0x100 * r.below(8)
            };
            p.put32(0x6F1E14, buffer);
            p.put32(0x6F1E10, [0u32, buffer, 0x0100_0800][r.below(3) as usize]);
            fast_fill(p, 0x0100_0000, 0x1000, 0x55);
            let force = [0u32, 1, r.next()][r.below(3) as usize];
            (
                case(r, 0x69EE0, vec![force]),
                vec![WRAP_REGION, (0x6F1E00, 0x20)],
            )
        },
        |l, h, force| {
            AllocWrappers {
                memory: &mut *l.memory,
                hooks: h,
            }
            .ensure_ledger_buffer_6f1e14(force)
            .unwrap();
            0
        },
    );
}

#[test]
fn lifted_001d1e80() {
    ledger_drive(
        0x1D1E80,
        false,
        |r, p| {
            arena_state(r, p);
            p.put32(
                0x6B8384,
                [0u32, 0x100, 0x1590, 0x2000, 0xFFFF_FF00][r.below(5) as usize],
            );
            p.put32(0x6B837C, WRAP_REGION.0 + r.below(0x100));
            p.put32(0x75B4B4, 0xA5A5_A5A5);
            fast_fill(p, WRAP_REGION.0, 0x2000, 0xAA);
            (
                case(r, 0x1D1E80, vec![]),
                vec![(WRAP_REGION.0, 0x2000), (0x75B4B4, 4)],
            )
        },
        |l, h, _| {
            AllocWrappers {
                memory: &mut *l.memory,
                hooks: h,
            }
            .alloc_aligned_75b4b4()
            .unwrap();
            0
        },
    );
}

#[path = "w4_effect_slots.rs"]
mod w4_effect_slots;
#[path = "w4_story_alloc.rs"]
mod w4_story_alloc;
#[path = "w4_weather.rs"]
mod w4_weather;

/// Region-B (arena kind 1) table constructors: random cursor and remaining capacity, globals
/// prefilled with a sentinel so the clears and the pointer stores are all compared.
fn arena_wrapper_test(
    va: u32,
    globals: &'static [(u32, u32)],
    extra_setup: fn(&mut Rng, &mut Pair) -> Vec<u32>,
    call: impl Fn(&mut AllocWrappers<'_, AMem<'_>, InitHost>, u32) + 'static,
) {
    ledger_drive(
        va as u64,
        false,
        move |r, p| {
            arena_state(r, p);
            p.put32(
                0x6B8384,
                [0u32, 0x10, 0x190, 0x9600, 0x2_0000, 0xFFFF_FF00][r.below(6) as usize],
            );
            p.put32(0x6B837C, WRAP_REGION.0 + 0x10 * r.below(0x40));
            fast_fill(p, WRAP_REGION.0, WRAP_REGION.1, 0xAA);
            for &(a, n) in globals {
                fast_fill(p, a, n, 0xAA);
            }
            let args = extra_setup(r, p);
            let mut ranges = vec![WRAP_REGION];
            ranges.extend_from_slice(globals);
            (case(r, va, args), ranges)
        },
        move |l, h, argument| {
            call(
                &mut AllocWrappers {
                    memory: &mut *l.memory,
                    hooks: h,
                },
                argument,
            );
            0
        },
    );
}

fn no_args(_: &mut Rng, _: &mut Pair) -> Vec<u32> {
    vec![]
}

fn setup_220e20_table(_: &mut Rng, p: &mut Pair) -> Vec<u32> {
    p.put32(0x4CA9B4, STATE_BLOCK_TABLE);
    p.put32(STATE_BLOCK_READINESS, 0xA5A5_A5A5);
    vec![]
}

fn setup_2259d0_table(_: &mut Rng, p: &mut Pair) -> Vec<u32> {
    p.put32(0x4CA9B4, STATE_BLOCK_TABLE);
    p.put32(SOUND_DESCRIPTOR_1B_READY, 0xA5A5_A5A5);
    p.put32(SOUND_DESCRIPTOR_19_READY, 0x5A5A_5A5A);
    vec![]
}

fn setup_2226f0_table(_: &mut Rng, p: &mut Pair) -> Vec<u32> {
    p.put32(0x4CA9B4, STATE_BLOCK_TABLE);
    for (index, &field) in TIMER_DESCRIPTOR_READY.iter().enumerate() {
        p.put32(field, 0xA5A5_0000 | index as u32);
    }
    vec![]
}

fn setup_story_objective_counters(r: &mut Rng, p: &mut Pair) -> Vec<u32> {
    const MODE: u32 = 0x0074_0000;
    const DEFINITIONS: u32 = 0x0074_1000;
    let count = r.below(33);
    p.put32(0x74C36C, MODE);
    p.put32(MODE + 0x74, DEFINITIONS);
    p.put32(MODE + 0x78, count);
    p.put32(
        0x7A2820,
        if r.below(2) == 0 {
            0
        } else {
            WRAP_REGION.0 + 0x1000
        },
    );
    // Keep the fixture on the successful-allocation path while varying cursor, count,
    // existing-buffer reuse, definition flags, and seed values.
    p.put32(0x6B8384, 0x2_0000);
    for index in 0..count {
        let row = DEFINITIONS + index * 0x98;
        let flags = if r.below(2) == 0 {
            0x8000 | (r.next() & 0x7FFF)
        } else {
            r.next() & 0x7FFF
        };
        p.put32(row + 0x54, flags);
        p.put32(row + 0x64, r.next());
    }
    vec![]
}

#[test]
fn lifted_0006b360() {
    arena_wrapper_test(0x6B360, &[(0x6F1E18, 0x10)], no_args, |w, _| {
        w.alloc_table_6f1e18().unwrap()
    });
}
#[test]
fn lifted_000b05b0() {
    arena_wrapper_test(0xB05B0, &[(0x732A04, 8)], no_args, |w, _| {
        w.alloc_zeroed_732a04().unwrap()
    });
}
#[test]
fn lifted_000c3540() {
    arena_wrapper_test(0xC3540, &[(0x732E70, 8)], no_args, |w, _| {
        w.alloc_zeroed_732e70().unwrap()
    });
}
#[test]
fn lifted_0021af30() {
    arena_wrapper_test(0x21AF30, &[(0x75C0F0, 4)], no_args, |w, _| {
        w.alloc_table_75c0f0().unwrap()
    });
}
#[test]
fn lifted_002210b0() {
    arena_wrapper_test(0x2210B0, &[(0x75C10C, 4)], no_args, |w, _| {
        w.alloc_table_75c10c().unwrap()
    });
}
#[test]
fn lifted_00220e20() {
    arena_wrapper_test(
        0x220E20,
        &[(0x75C108, 4), (0x4CA9B4, 4), (STATE_BLOCK_READINESS, 4)],
        setup_220e20_table,
        |w, _| w.alloc_state_block_75c108().unwrap(),
    );
}
#[test]
fn lifted_00220e40() {
    ledger_drive(
        0x220E40,
        true,
        |r, p| {
            arena_state(r, p);
            fast_fill(p, WRAP_REGION.0, WRAP_REGION.1, 0xAA);
            let state = 0x0105_0000;
            p.put32(0x75C108, state);
            p.put32(state, 0xA5A5_A5A5);
            p.put32(state + 4, 0x5A5A_5A5A);
            (case(r, 0x220E40, vec![]), vec![(0x75C108, 4), (state, 8)])
        },
        |l, h, _| {
            AllocWrappers {
                memory: &mut *l.memory,
                hooks: h,
            }
            .initialize_state_block_75c108()
            .unwrap()
        },
    );
}
#[test]
fn lifted_002259d0() {
    arena_wrapper_test(
        0x2259D0,
        &[
            (0x75C124, 4),
            (0x4CA9B4, 4),
            (SOUND_DESCRIPTOR_1B_READY, 4),
            (SOUND_DESCRIPTOR_19_READY, 4),
        ],
        setup_2259d0_table,
        |w, _| w.alloc_sound_state_75c124().unwrap(),
    );
}
#[test]
fn lifted_002226f0() {
    arena_wrapper_test(
        0x2226F0,
        &[
            (0x75C118, 4),
            (0x4CA9B4, 4),
            (TIMER_DESCRIPTOR_READY[0], 4),
            (TIMER_DESCRIPTOR_READY[1], 4),
            (TIMER_DESCRIPTOR_READY[2], 4),
            (TIMER_DESCRIPTOR_READY[3], 4),
            (TIMER_DESCRIPTOR_READY[4], 4),
        ],
        setup_2226f0_table,
        |w, _| w.alloc_timer_state_75c118().unwrap(),
    );
}
#[test]
fn lifted_001620c0_story_objective_counter_setup() {
    arena_wrapper_test(
        0x1620C0,
        &[(0x7A2820, 4)],
        setup_story_objective_counters,
        |w, _| w.alloc_story_objective_counters_7a2820().unwrap(),
    );
}
#[test]
fn lifted_002a62c0() {
    arena_wrapper_test(0x2A62C0, &[(0x78B0D0, 4)], no_args, |w, _| {
        w.alloc_table_78b0d0().unwrap()
    });
}
#[test]
fn lifted_002a8300() {
    arena_wrapper_test(
        0x2A8300,
        &[(0x78B08C, 4), (0x78B098, 4), (0x78B090, 4)],
        |r, _| vec![r.below(2)],
        |w, flag| {
            w.alloc_tables_78b08c(flag).unwrap();
        },
    );
}
fn pick_node_source(r: &mut Rng, p: &mut Pair) -> Vec<u32> {
    let source = 0x0076_2F80;
    for index in 0..5 {
        p.put32(source + index * 4, r.next());
    }
    vec![source]
}
#[test]
fn lifted_0031aaa0() {
    arena_wrapper_test(
        0x31AAA0,
        &[(0x762F7C, 4), (0x0076_2F80, 0x14)],
        pick_node_source,
        |w, source| {
            w.alloc_pick_node_copy_762f7c(source).unwrap();
        },
    );
}

#[test]
fn lifted_0031c930() {
    ledger_drive(
        0x31C930,
        true,
        |r, p| {
            let block = WRAP_REGION.0 + 0x1000 + r.below(0x1000) * 0x10;
            let header = block - 0x10;
            let size = [0x10, 0x20, 0x30, 0x100][r.below(4) as usize];
            p.put32(0x6B8388, header);
            p.put32(0x6B8384, 0x10000 + r.below(0x1000));
            p.put32(0x762F90, block);
            p.put32(header, 0x4143_4F4C);
            p.put32(header + 4, 0x4D45_4D4C);
            p.put32(header + 8, size);
            p.put32(header + 12, header);
            (case(r, 0x31C930, vec![]), vec![(0x762F90, 4), (header, 16)])
        },
        |memory, hooks, _| {
            AllocWrappers {
                memory: &mut *memory.memory,
                hooks,
            }
            .release_mapedit_static_block_762f90()
            .unwrap()
            .eax
        },
    );
}

#[test]
fn lifted_0031e430() {
    ledger_drive(
        0x31E430,
        true,
        |r, p| {
            let block = WRAP_REGION.0 + 0x1000 + r.below(0x1000) * 0x10;
            let header = block - 0x10;
            let size = [0x10, 0x20, 0x30, 0x100][r.below(4) as usize];
            p.put32(0x6B8388, header);
            p.put32(0x6B8384, 0x10000 + r.below(0x1000));
            p.put32(0x7630B0, block);
            p.put32(header, 0x4143_4F4C);
            p.put32(header + 4, 0x4D45_4D4C);
            p.put32(header + 8, size);
            p.put32(header + 12, header);
            (case(r, 0x31E430, vec![]), vec![(0x7630B0, 4), (header, 16)])
        },
        |memory, hooks, _| {
            AllocWrappers {
                memory: &mut *memory.memory,
                hooks,
            }
            .release_record_rows_panel_block_7630b0()
            .unwrap()
            .eax
        },
    );
}

#[test]
fn lifted_003796b0() {
    arena_wrapper_test(0x3796B0, &[(0x772B50, 0x10)], no_args, |w, _| {
        w.alloc_node_pool_772b50().unwrap();
    });
}
#[test]
fn lifted_002760f0() {
    arena_wrapper_test(0x2760F0, &[(0x78B240, 0x10)], no_args, |w, _| {
        w.alloc_table_78b240().unwrap()
    });
}
#[test]
fn lifted_00307cc0() {
    arena_wrapper_test(0x307CC0, &[(0x784004, 4)], no_args, |w, _| {
        w.alloc_zeroed_784004().unwrap()
    });
}
#[test]
fn lifted_00274650() {
    arena_wrapper_test(
        0x274650,
        &[(0x74C3B8, 8), (0x7DE400, 4)],
        |r, p| {
            p.put32(
                0x7DE400,
                [0u32, 0x0100_0000, 0x7F00_0000, 0x0500_0000, r.next()][r.below(5) as usize],
            );
            vec![]
        },
        |w, _| {
            w.alloc_tables_74c3b8().unwrap();
        },
    );
}
#[test]
fn lifted_0009a0f0() {
    arena_wrapper_test(
        0x9A0F0,
        &[(0x7B6774, 4), (0x7B677C, 4)],
        |r, _| vec![[0u32, 1, 10, 0x3F, 0x100, r.below(0x2000), r.next()][r.below(7) as usize]],
        |w, a| w.alloc_table_7b6774(a).unwrap(),
    );
}
