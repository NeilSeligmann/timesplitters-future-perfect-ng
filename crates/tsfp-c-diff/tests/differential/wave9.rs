// SPDX-License-Identifier: GPL-3.0-or-later
//! T1835 wave 9: the 17 pool roots that need a guest-stack push/pop helper and a `rep stosd` /
//! `rep movs` helper (`tsfp-engine-physics/src/fp_stack.rs`). The driver also seeds EBX/EDI/EBP and
//! the direction flag (`shim_set_extra`), compares the restored callee-saved registers and the
//! stack window below the entry `esp`, so a body whose record or argument overlaps the saved slots
//! must restore the same (clobbered) register in the C and the Rust. See docs/t1835-rust-wave9.md.
use super::*;
use tsfp_engine_physics::{
    fp_scalar::Fp,
    fp_stack::{self, Cpu},
};

unsafe extern "C" {
    fn shim_call_fp(va: u32, regs: *mut u32, xmm: *mut u32, mxcsr: *mut u32) -> i32;
    fn shim_set_extra(extra: *const u32);
    fn shim_get_extra(extra: *mut u32);
}

pub(super) const POOL_A: u32 = 0x0040_0000;
pub(super) const POOL_B: u32 = 0x0050_0000;
pub(super) const POOL_C: u32 = 0x0060_0000;

pub(super) type Xmms = [[u32; 4]; 8];

pub(super) fn float_bits(rng: &mut Rng) -> u32 {
    if rng.below(24) == 0 {
        return [0x4f00_0000, 0xcf00_0000, 0x7f7f_ffff, 0x4b00_0000][rng.below(4) as usize];
    }
    let sign = if rng.below(2) == 0 { 0 } else { 0x8000_0000 };
    sign | match rng.below(10) {
        0 => 0,
        1 => rng.next() & 0x007f_ffff,
        2 => 0x3f80_0000,
        3 => 0x3f00_0000 + (rng.next() & 0x00ff_ffff),
        4 => 0x7f80_0000,
        5 => 0x7fc0_0000 | (rng.next() & 0x003f_ffff),
        _ => rng.next() & 0x7fff_ffff,
    }
}

/// Mostly a small set that creates ties and NaNs, sometimes any float.
pub(super) fn palette(rng: &mut Rng) -> u32 {
    if rng.below(3) == 0 {
        float_bits(rng)
    } else {
        [0, 0x8000_0000, 0x3f80_0000, 0x4000_0000, 0x4040_0000, 0x7fc0_0000][rng.below(6) as usize]
    }
}

/// A float in a range where the table index arithmetic is not saturated.
fn sane(rng: &mut Rng) -> u32 {
    if rng.below(5) == 0 {
        float_bits(rng)
    } else {
        ((rng.below(4000) as f32 - 2000.0) / 64.0).to_bits()
    }
}

/// A value a body may dereference or store: a pool pointer or a small number.
pub(super) fn ptrish(rng: &mut Rng) -> u32 {
    match rng.below(4) {
        0 => rng.below(32),
        1 => rng.pool(POOL_A),
        2 => rng.pool(POOL_B),
        _ => rng.pool(POOL_C),
    }
}

/// Non-zero random words in a region a body zero-fills, so a missing, shifted or mirrored fill is
/// visible (all-zero guest memory would hide it). `count` words at random dword offsets.
pub(super) fn scribble(rng: &mut Rng, pair: &mut Pair, start: u32, len: u32, count: u32) {
    for _ in 0..count {
        let address = start.wrapping_add(rng.below(len / 4) * 4);
        let value = rng.next() | 1;
        pair.put32(address, value);
    }
}

pub(super) struct S9 {
    pub(super) case: Case,
    pub(super) xmm: Xmms,
    pub(super) extra: [u32; 4],
}

pub(super) fn s9(rng: &mut Rng, va: u32, args: Vec<u32>, df: bool) -> S9 {
    let mut xmm = [[0u32; 4]; 8];
    for reg in &mut xmm {
        for lane in reg.iter_mut() {
            *lane = if rng.below(2) == 0 { float_bits(rng) } else { rng.next() };
        }
    }
    S9 {
        case: Case {
            va,
            args,
            regs: [ptrish(rng), ptrish(rng), ptrish(rng)],
            esi: ptrish(rng),
            watch: vec![],
        },
        xmm,
        extra: [ptrish(rng), ptrish(rng), ptrish(rng), u32::from(df && rng.below(4) == 0)],
    }
}

/// Result of one differential run: perturbations detected, perturbations made, cases whose C run
/// restored a callee-saved register to a value other than its entry value (saved slot clobbered).
pub(super) struct Stats {
    pub(super) detected: usize,
    pub(super) total: usize,
    pub(super) clobbered: usize,
}

pub(super) fn s9_drive(
    seed: u64,
    mutant: bool,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> S9,
    port: impl Fn(&mut Fp<View>, &mut Cpu),
) -> Stats {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let pair = &mut *guard;
    let mut rng = Rng(seed);
    let mut stats = Stats { detected: 0, total: 0, clobbered: 0 };
    for index in 0..CASES {
        let mut s = build(&mut rng, pair);
        let logged: Vec<u32> = pair.log.drain(..).collect();
        let slots = 4 * s.case.args.len().max(1) as u32;
        s.case.watch.insert(0, (STACK - 64, 64 + 4 + slots));
        s.case
            .watch
            .extend(logged.into_iter().map(|address| (address, 4)));
        pair.put32(STACK, 0);
        for (i, arg) in s.case.args.iter().enumerate() {
            pair.put32(STACK + 4 + 4 * i as u32, *arg);
        }
        pair.log.clear();
        let case = &s.case;
        let mut regs = [case.regs[0], case.regs[1], case.regs[2], case.esi, STACK, 0];
        let mut c_xmm = s.xmm;
        let mut mxcsr = 0u32;
        unsafe { shim_set_extra(s.extra.as_ptr()) };
        assert_eq!(
            unsafe {
                shim_call_fp(case.va, regs.as_mut_ptr(), c_xmm.as_mut_ptr().cast(), &mut mxcsr)
            },
            1,
            "unknown root"
        );
        let mut after = [0u32; 4];
        unsafe { shim_get_extra(after.as_mut_ptr()) };
        assert_eq!(mxcsr & !0x3f, 0x1f80, "C body changed the MXCSR control bits");
        let mut view = View(pair.rust);
        let mut rust_xmm = s.xmm;
        let mut cpu = Cpu::entry(STACK, case.regs[0], case.regs[1], case.regs[2], case.esi, s.extra);
        {
            let mut f = Fp { m: &mut view, x: &mut rust_xmm, esp: STACK };
            port(&mut f, &mut cpu);
        }
        stats.clobbered += usize::from(
            regs[3] != case.esi || after[0] != s.extra[0] || after[1] != s.extra[1] || after[2] != s.extra[2],
        );
        if mutant {
            match index % 8 {
                0 => cpu.eax ^= 1,
                1 => cpu.ecx ^= 1,
                2 => cpu.edx ^= 1,
                3 => {
                    let (start, _) = case.watch[0];
                    let byte = view.byte(start);
                    view.set(start, byte ^ 0x80);
                }
                4 => rust_xmm[(rng.next() % 8) as usize][(rng.next() % 4) as usize] ^= 1 << (rng.next() % 32),
                5 => cpu.esi ^= 1 << (rng.next() % 32),
                6 => match rng.next() % 3 {
                    0 => cpu.ebx ^= 1,
                    1 => cpu.edi ^= 1,
                    _ => cpu.ebp ^= 1,
                },
                _ => cpu.esp = cpu.esp.wrapping_add(4),
            }
        }
        let mut mismatch = lifted_policy::mismatch(
            case.va,
            u8::from(regs[0] != cpu.eax)
                | u8::from(regs[1] != cpu.ecx) << 1
                | u8::from(regs[2] != cpu.edx) << 2
                | u8::from(after[0] != cpu.ebx) << 3
                | u8::from(regs[3] != cpu.esi) << 4
                | u8::from(after[1] != cpu.edi) << 5
                | u8::from(after[2] != cpu.ebp) << 6
                | u8::from(regs[4] != cpu.esp.wrapping_add(4)) << 7,
        ) || lifted_policy::other(case.va, "extra3", after[3] != s.extra[3])
            || lifted_policy::xmm_mismatch(case.va, lifted_policy::xmm_diff(case.va, &c_xmm, &rust_xmm));
        for (start, len) in &case.watch {
            for offset in 0..*len {
                let address = start.wrapping_add(offset);
                mismatch |= lifted_policy::mem(case.va, address, pair.get(false, address) != pair.get(true, address));
            }
        }
        if mutant {
            stats.total += 1;
            stats.detected += usize::from(mismatch);
            for (start, len) in &case.watch {
                for offset in 0..*len {
                    let address = start.wrapping_add(offset);
                    let byte = pair.get(false, address);
                    unsafe { *pair.rust.add(address as usize) = byte };
                }
            }
        } else {
            assert!(
                !mismatch,
                "root {:08X} case {index}: C regs {regs:x?} saved {after:x?}, Rust {cpu:x?}, args {:x?}",
                case.va, case.args
            );
        }
    }
    stats
}

/// `clobber`: the root must be seen restoring a changed callee-saved register at least once.
macro_rules! s9_suite {
    ($name:ident, $mutant:ident, $seed:expr, $clobber:expr, $build:expr, $port:expr) => {
        #[test]
        fn $name() {
            let stats = s9_drive($seed, false, $build, $port);
            eprintln!("{}: {} cases restored a clobbered saved register", stringify!($name), stats.clobbered);
            if $clobber {
                assert!(stats.clobbered > 0, "no case exercised the saved-slot overlap");
            }
        }
        #[test]
        fn $mutant() {
            let stats = s9_drive($seed, true, $build, $port);
            eprintln!(
                "mutant control {}: {}/{} perturbations detected",
                stringify!($name),
                stats.detected,
                stats.total
            );
            assert!(stats.total > 0);
            assert_eq!(stats.detected, stats.total);
        }
    };
}

// ---- 000B3C60 extension count predicate ----
fn build_b3c60(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = rng.pool(POOL_A);
    let ext = rng.pool(POOL_B);
    let block = match rng.below(8) {
        0 => STACK - 4 - 0xf0,
        1 => STACK - 4 - 0xf4,
        _ => rng.pool(POOL_C),
    };
    pair.put32(object + 0x7c, ext);
    pair.put32(ext + 0x6c, block);
    let small = [9, 10, 11, 0xffff_ffff, 0x8000_0000, 0];
    pair.put32(block + 0xf0, rng.word(&small));
    pair.put32(block + 0xf4, rng.word(&small));
    pair.put32(ext + 0x1c4, palette(rng));
    let mut s = s9(rng, 0xb3c60, vec![object], false);
    if rng.below(4) == 0 {
        s.case.esi = rng.word(&small);
    }
    s
}
s9_suite!(r_000b3c60, mutant_000b3c60, 0x1835_9001, false, build_b3c60, |f, c| {
    fp_stack::extension_count_float_predicate(f, c)
});

// ---- 000B0DD0 extension activate ----
fn build_b0dd0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = if rng.below(10) == 0 { 0 } else { rng.pool(POOL_A) };
    let ext = match rng.below(10) {
        0 => 0,
        1 => STACK - 4 - 0x10,
        2 => STACK - 4 - 0x74,
        3 => STACK - 4 - 0x24,
        4 => STACK - 4 - 0x38,
        5 => STACK - 4 - 0xc,
        _ => rng.pool(POOL_B),
    };
    pair.put32(object + 0x7c, ext);
    pair.put32(ext + 0x10, if rng.below(3) == 0 { 2 } else { rng.next() });
    pair.put32(ext + 0xc, rng.next());
    pair.put32(object + 0x18, rng.next());
    pair.put32(object + 0x28, rng.next());
    pair.put32(0x0047_5c78, palette(rng));
    pair.put32(ext + 0x74, rng.next());
    pair.put32(ext + 0x24, rng.next());
    pair.put32(ext + 0x20, rng.next());
    pair.put32(ext + 0x38, rng.next());
    let mut s = s9(rng, 0xb0dd0, vec![object], false);
    if rng.below(3) == 0 {
        s.case.esi = 2;
    }
    s
}
s9_suite!(r_000b0dd0, mutant_000b0dd0, 0x1835_9002, true, build_b0dd0, |f, c| {
    fp_stack::extension_activate(f, c)
});

// ---- 0008CBC0 scene insert ----
fn build_8cbc0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let list = match rng.below(6) {
        0 => STACK - 0x14,
        1 => STACK - 0x10,
        2 => STACK - 0x18,
        _ => rng.pool(POOL_A),
    };
    let node_base = rng.pool(POOL_B) & !0xff;
    let nodes: Vec<u32> = (0..6).map(|i| node_base + i * 0x100).collect();
    for node in &nodes {
        pair.put32(node + 4, rng.word(&[0xffff_ffff, 0x8000_0000, 1]));
        pair.put32(node + 0x2c, palette(rng));
    }
    for slot in 0..5 {
        pair.put32(list + 4 * slot, nodes[rng.below(6) as usize]);
    }
    let cell = rng.pool(POOL_C);
    pair.put32(cell, nodes[rng.below(6) as usize]);
    let cell = if rng.below(8) == 0 { STACK - 4 } else { cell };
    let mut s = s9(rng, 0x8cbc0, vec![list, cell], false);
    s.case.watch.push((list, 0x14));
    s
}
s9_suite!(r_0008cbc0, mutant_0008cbc0, 0x1835_9003, true, build_8cbc0, |f, c| {
    fp_stack::scene_insert_four(f, c)
});

// ---- 001115F0 particle descriptor init ----
fn build_1115f0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let record = match rng.below(6) {
        0 => STACK - 4 * (1 + rng.below(0x40)),
        1 => STACK + 4 * rng.below(12),
        _ => rng.pool(POOL_A),
    };
    let kind = rng.word(&[5, 6, 5, 6, 4]);
    let args = vec![record, kind, rng.next(), rng.next(), rng.next(), rng.next(), float_bits(rng), float_bits(rng)];
    pair.put32(0x0047_5c78, palette(rng));
    scribble(rng, pair, record.wrapping_sub(0xd0), 0x1a0, 64);
    let mut s = s9(rng, 0x1115f0, args, true);
    s.case.watch.push((record.wrapping_sub(0xd0), 0x1a0));
    s.case.watch.push((0x0073_bd68, 4));
    s
}
s9_suite!(r_001115f0, mutant_001115f0, 0x1835_9004, true, build_1115f0, |f, c| {
    fp_stack::particle_descriptor_init(f, c)
});

// ---- 00126AC0 ring push ----
fn build_126ac0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let index = match rng.below(6) {
        0 => 0xef,
        1 => 0xf0,
        2 => 0,
        _ => rng.below(0xf2),
    };
    pair.put32(0x0073_ed10, index);
    pair.put32(0x0074_2198, float_bits(rng));
    let args: Vec<u32> = (0..12).map(|_| if rng.below(2) == 0 { float_bits(rng) } else { rng.next() }).collect();
    let mut s = s9(rng, 0x126ac0, args, false);
    let entry = 0x0073_ed18 + 0x38 * index;
    s.case.watch.push((entry, 0x38));
    s.case.watch.push((0x0073_ed10, 4));
    s
}
s9_suite!(r_00126ac0, mutant_00126ac0, 0x1835_9005, false, build_126ac0, |f, c| {
    fp_stack::ring_push_entry(f, c)
});

// ---- 00128E90 list remove ----
fn build_128e90(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let list = match rng.below(3) {
        0 => STACK - 0x20 * (1 + rng.below(12)),
        _ => rng.pool(POOL_A),
    };
    let count = match rng.below(8) {
        0 => 0,
        1 => 0xffff_ffff,
        _ => 1 + rng.below(7),
    };
    pair.put32(list, count);
    let limit = palette(rng);
    for i in 0..8u32 {
        for word in 0..13u32 {
            pair.put32(list.wrapping_add(4 + 0x34 * i + 4 * word), rng.next());
        }
        let value = if rng.below(3) == 0 { limit } else { palette(rng) };
        pair.put32(list.wrapping_add(0x30 + 0x34 * i), value);
    }
    let mut s = s9(rng, 0x128e90, vec![list, limit], true);
    s.extra[3] = 0; // the C moves with the DF step, but a descending copy is not a list compaction
    s.case.watch.push((list, 0x34 * 8 + 8));
    s
}
s9_suite!(r_00128e90, mutant_00128e90, 0x1835_9006, true, build_128e90, |f, c| {
    fp_stack::list_remove_entries_above(f, c)
});

// ---- 00129EC0 release children ----
fn build_129ec0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = rng.pool(POOL_A);
    let count = match rng.below(8) {
        0 => 0,
        1 => 0xffff_ffff,
        _ => 1 + rng.below(4),
    };
    pair.put32(object + 0x1c, count);
    let table = rng.pool(POOL_B);
    pair.put32(object + 0x1f8, table);
    let mut children = vec![];
    for i in 0..5u32 {
        let child = if rng.below(6) == 0 { STACK - 4 * (1 + rng.below(4)) } else { POOL_C + i * 0x100 };
        pair.put32(table + 4 * i, child);
        pair.put32(child + 0x28, rng.word(&[0x40c, 0x4, 0x8, 0]));
        let byte = |rng: &mut Rng| [0u8, 1, 0x7f, 0x80, 0xff, rng.next() as u8][rng.below(6) as usize];
        let (a, b) = (byte(rng), byte(rng));
        pair.put8(child + 0xe, a);
        pair.put8(child + 0xf, b);
        pair.put32(child + 0x10, rng.next());
        children.push(child);
    }
    for entry in 0..5u32 {
        let base = 0x0074_2360 + 0x6c * entry;
        pair.put32(base, if rng.below(2) == 0 { 0 } else { rng.next() });
        let target = children[rng.below(5) as usize];
        pair.put32(base + 4, if rng.below(2) == 0 { target } else { rng.next() });
    }
    pair.put32(0x0047_d9d4, palette(rng));
    let mut s = s9(rng, 0x129ec0, vec![object], false);
    s.case.watch.push((0x007a_bfa4, 0x4c * 128));
    s.case.watch.push((0x0074_2360, 0x1b8));
    for child in children {
        s.case.watch.push((child, 0x14));
    }
    s
}
s9_suite!(r_00129ec0, mutant_00129ec0, 0x1835_9007, true, build_129ec0, |f, c| {
    fp_stack::object_release_children(f, c)
});

// ---- 00166770 sorted list insert ----
fn build_166770(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let head = rng.pool(POOL_B);
    let node = rng.pool(POOL_C) & !0x3f;
    let length = rng.below(6);
    let chain: Vec<u32> = (0..length).map(|i| POOL_A + 0x40 * i + 0x1000 * rng.below(4)).collect();
    pair.put32(head, chain.first().copied().unwrap_or(0));
    for (i, link) in chain.iter().enumerate() {
        pair.put32(link + 0x1c, palette(rng));
        pair.put32(link + 0x34, chain.get(i + 1).copied().unwrap_or(0));
    }
    pair.put32(node + 0x1c, palette(rng));
    pair.put32(node + 0x34, rng.next());
    s9(rng, 0x166770, vec![head, node], false)
}
s9_suite!(r_00166770, mutant_00166770, 0x1835_9008, false, build_166770, |f, c| {
    fp_stack::sorted_list_insert(f, c)
});

// ---- 001B7AA0 zero 0x3d8 struct ----
fn build_1b7aa0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let record = match rng.below(4) {
        0 => STACK - 4 * rng.below(0xf8),
        _ => rng.pool(POOL_A),
    };
    scribble(rng, pair, record, 0x3d8, 96);
    let (a, b) = (rng.next(), rng.next());
    let mut s = s9(rng, 0x1b7aa0, vec![record, a, b], false);
    s.case.watch.push((record, 0x3d8 + 8));
    s
}
s9_suite!(r_001b7aa0, mutant_001b7aa0, 0x1835_9009, true, build_1b7aa0, |f, c| {
    fp_stack::zero_struct_3d8_init(f, c)
});

// ---- 0025F9C0 table append ----
fn build_25f9c0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let count = match rng.below(8) {
        0 => 0xffff_ffff,
        1 => 0x8000_0000,
        2 => 17,
        3 => 18,
        _ => rng.below(20),
    };
    pair.put32(0x0078_b520, count);
    let source = match rng.below(8) {
        0 => STACK - 4 * (1 + rng.below(6)),
        1 => 0x0078_b2e0 + 0x20 * rng.below(18),
        _ => rng.pool(POOL_A),
    };
    for offset in [0, 8, 0xc, 0x10, 0x14] {
        pair.put32(source + offset, rng.next());
    }
    let mut s = s9(rng, 0x25f9c0, vec![source], false);
    s.case.watch.push((0x0078_b2e0, 0x244));
    s
}
s9_suite!(r_0025f9c0, mutant_0025f9c0, 0x1835_900a, false, build_25f9c0, |f, c| {
    fp_stack::table_append_entry(f, c)
});

// ---- 002AD370 pairwise loop ----
fn build_2ad370(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let buffer = match rng.below(10) {
        0 => STACK - 0x400,
        _ => rng.pool(POOL_A),
    };
    let full = rng.below(200) == 0;
    let records = if full { 300 } else { 12 };
    for k in 0..records {
        let i = if full { k } else { rng.below(300) };
        for word in 0..3 {
            pair.put32(buffer + 12 * i + 4 * word, palette(rng));
        }
    }
    pair.put32(0x0047_d610, palette(rng));
    pair.put32(0x0047_60cc, palette(rng));
    let mut s = s9(rng, 0x2ad370, vec![buffer], false);
    s.case.watch.push((buffer, 3600));
    s
}
s9_suite!(r_002ad370, mutant_002ad370, 0x1835_900b, false, build_2ad370, |f, c| {
    fp_stack::pairwise_scroll_loop(f, c)
});

// ---- 002E5900 table lookup triple ----
fn build_2e5900(rng: &mut Rng, pair: &mut Pair, filled: &mut bool) -> S9 {
    if !*filled {
        *filled = true;
        for i in 0..2048 {
            let value = float_bits(rng);
            pair.put32(0x007a_38a0 + 4 * i, value);
        }
    }
    let out = match rng.below(8) {
        0 => STACK - 4 * (1 + rng.below(3)),
        1 => 0x007a_38a0 + 4 * rng.below(2048),
        _ => rng.pool(POOL_B),
    };
    for address in [0x0047_5c78u32, 0x0047_5cd4, 0x0047_5ce0, 0x0049_f6d4, 0x0049_f6d0, 0x0047_5d18, 0x0047_d6d4, 0x0047_6040] {
        let value = if address == 0x0047_5c78 && rng.below(2) == 0 { 0x3f80_0000 } else { sane(rng) };
        pair.put32(address, value);
    }
    let args = vec![out, sane(rng), sane(rng), sane(rng)];
    let mut s = s9(rng, 0x2e5900, args, false);
    s.case.watch.push((out, 12));
    s
}
s9_suite!(
    r_002e5900,
    mutant_002e5900,
    0x1835_900c,
    true,
    {
        let mut filled = false;
        move |r, p| build_2e5900(r, p, &mut filled)
    },
    |f, c| fp_stack::table_lookup_triple(f, c)
);

// ---- 00347B80 clear entry table ----
fn build_347b80(rng: &mut Rng, pair: &mut Pair) -> S9 {
    pair.put32(0x0047_60d0, float_bits(rng));
    scribble(rng, pair, 0x0076_6b80, 0xb00, 96);
    let mut s = s9(rng, 0x347b80, vec![], true);
    s.case.watch.push((0x0076_6a00, 0xd00));
    s
}
s9_suite!(r_00347b80, mutant_00347b80, 0x1835_900d, false, build_347b80, |f, c| {
    fp_stack::clear_entry_table(f, c)
});

// ---- 00347D70 append record ----
fn build_347d70(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let count = match rng.below(8) {
        0 => 0xffff_ffff,
        1 => 0x8000_0000,
        2 => 0x1f,
        3 => 0x20,
        _ => rng.below(0x22),
    };
    pair.put32(0x0076_70f8, count);
    let source = match rng.below(8) {
        0 => 0,
        1 => STACK - 4 * (1 + rng.below(4)),
        _ => rng.pool(POOL_A),
    };
    pair.put32(source + 0x2c, rng.next());
    let record = 0x0076_7100u32.wrapping_add(count.wrapping_mul(0x2c));
    scribble(rng, pair, record, 0x2c, 8);
    let (a, b) = (rng.next(), float_bits(rng));
    let mut s = s9(rng, 0x347d70, vec![a, b, source], false);
    s.case.watch.push((record, 0x2c));
    s.case.watch.push((0x0076_70f8, 4));
    s
}
s9_suite!(r_00347d70, mutant_00347d70, 0x1835_900e, false, build_347d70, |f, c| {
    fp_stack::append_record_0x2c(f, c)
});

// ---- 00349570 init entry array ----
fn build_349570(rng: &mut Rng, pair: &mut Pair) -> S9 {
    scribble(rng, pair, 0x0077_8700, 0x400, 96);
    let mut s = s9(rng, 0x349570, vec![], true);
    s.case.watch.push((0x0077_8700, 0x400));
    s.case.watch.push((0x0076_7680, 0x40));
    s
}
s9_suite!(r_00349570, mutant_00349570, 0x1835_900f, false, build_349570, |f, c| {
    fp_stack::init_entry_array(f, c)
});

// ---- 00357DD0 bind record ----
fn build_357dd0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let record = if rng.below(8) == 0 { 0 } else { rng.pool(POOL_A) };
    let owner = match rng.below(6) {
        0 => record.wrapping_add(0x130),
        1 => record.wrapping_add(0x134),
        2 => record,
        _ => rng.pool(POOL_B),
    };
    let source = match rng.below(6) {
        0 => record.wrapping_add(0x1c),
        1 => record.wrapping_add(0x20),
        2 => record.wrapping_add(0x64),
        _ => rng.pool(POOL_C),
    };
    let linked = match rng.below(6) {
        0 => 0,
        1 => record.wrapping_sub(0x48),
        2 => record.wrapping_sub(0x4c),
        3 => record,
        _ => POOL_C + 0x4000 + 0x200 * rng.below(8),
    };
    for address in [owner.wrapping_add(0x28), source, source.wrapping_add(4), source.wrapping_add(8)] {
        pair.put32(address, rng.next());
    }
    pair.put32(record.wrapping_add(0xa8), linked);
    let tag = rng.next();
    let mut s = s9(rng, 0x357dd0, vec![owner, record, tag, source], false);
    s.case.watch.push((record, 0x170));
    s.case.watch.push((owner.wrapping_add(0x20), 0x10));
    s.case.watch.push((linked.wrapping_add(0x60), 0x10));
    s
}
s9_suite!(r_00357dd0, mutant_00357dd0, 0x1835_9010, false, build_357dd0, |f, c| {
    fp_stack::bind_record_to_owner(f, c)
});

// ---- 00362CC0 load table ----
fn build_362cc0(rng: &mut Rng, pair: &mut Pair, filled: &mut bool) -> S9 {
    if !*filled {
        *filled = true;
        for i in 0..0xfbu32 {
            let value = rng.next();
            pair.put32(0x0054_6304 + 4 * i, value);
        }
    }
    for _ in 0..4 {
        let value = rng.next();
        pair.put32(0x0054_6304 + 4 * rng.below(0xfb), value);
    }
    for address in [0x0077_36e0u32, 0x0077_36e4, 0x0077_3700, 0x0076_c0b4, 0x0077_388c] {
        pair.put32(address, rng.next());
    }
    let mut s = s9(rng, 0x362cc0, vec![], false);
    s.case.watch.push((0x0077_3880, 0x410));
    s
}
s9_suite!(
    r_00362cc0,
    mutant_00362cc0,
    0x1835_9011,
    false,
    {
        let mut filled = false;
        move |r, p| build_362cc0(r, p, &mut filled)
    },
    |f, c| fp_stack::load_table_546304(f, c)
);
