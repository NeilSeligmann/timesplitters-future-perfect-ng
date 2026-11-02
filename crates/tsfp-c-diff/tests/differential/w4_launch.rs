// SPDX-License-Identifier: GPL-3.0-or-later
//! T2409 Tier 1 (synthetic inputs, not capturedTier2): the launch-argument boundary of
//! `tsfp_engine_core::launch` against the PRIVATE LIFTED ORIGINAL (0x3C230 lookup, the numeric and
//! name handlers). Inputs are generated, so this is weak evidence by policy (rule 12a); the aliasing
//! cases place the looked-up name, the argv array and the scan cursor on the same guest bytes
//! (0x6B7A20 cursor, 0x74CF68/6C count and array) because the guest shares them.
use super::*;
use tsfp_engine_core::launch as lc;

const CURSOR: u32 = 0x006B_7A20;
const COUNT: u32 = 0x0074_CF68;
const ARRAY: u32 = 0x0074_CF6C;
const BLOCK: u32 = 0x007E_B300;
const TAIL: [u32; 4] = [0x006B_7A88, 0x006B_7A8C, 0x004E_8CEC, CURSOR];

fn seed_block(r: &mut Rng, p: &mut Pair) {
    for i in 0..0x28 {
        p.put32(BLOCK + 4 * i, r.next());
    }
    for a in TAIL {
        p.put32(a, r.next());
    }
}
fn put_str(p: &mut Pair, at: u32, s: &[u8]) {
    for (i, b) in s.iter().enumerate() {
        p.put8(at + i as u32, *b);
    }
    p.put8(at + s.len() as u32, 0);
}
fn word(r: &mut Rng) -> Vec<u8> {
    let n = 1 + r.below(4);
    (0..n).map(|_| b"ab"[r.below(2) as usize]).collect()
}

fn build_find(r: &mut Rng, p: &mut Pair) -> W4Case {
    seed_block(r, p);
    let argc = [0, 1, 2, 3, 5, 0xFFFF_FFFF, 0x8000_0000, 4][r.below(8) as usize];
    let array = POOL + 0x100;
    let mut name = POOL + 0x200;
    let names: Vec<Vec<u8>> = (0..6).map(|_| word(r)).collect();
    let mut strings = [0u32; 6];
    for (i, s) in names.iter().enumerate() {
        strings[i] = POOL + 0x300 + 0x20 * i as u32;
        put_str(p, strings[i], s);
    }
    let mut table = array;
    match r.below(6) {
        // The name lives at the scan cursor: a cursor write during the scan would corrupt it.
        0 => name = CURSOR,
        // The argv array overlaps the cursor word (entry 1 sits on it).
        1 => table = CURSOR - 4,
        // The name aliases an argv entry's string exactly.
        2 => name = strings[1 + r.below(5) as usize],
        _ => {}
    }
    if r.below(5) != 0 {
        let pick = names[r.below(6) as usize].clone();
        put_str(p, name, &pick);
    } else {
        p.put8(name, 0);
    }
    for (i, s) in strings.iter().enumerate() {
        if table + 4 * i as u32 != CURSOR {
            p.put32(table + 4 * i as u32, *s);
        }
    }
    p.put32(COUNT, argc);
    p.put32(ARRAY, table);
    case(r, 0x3C230, vec![name])
}
w4_test!(
    lifted_0003c230_launch_find,
    0x3C230,
    Regs::RET,
    &[],
    build_find,
    |_, m, cpu| lc::find_argument(m, cpu)
);

fn handler(va: u32) -> impl FnMut(&mut Rng, &mut Pair) -> W4Case {
    move |r, p| {
        seed_block(r, p);
        p.put32(0x006B_7A88, r.next());
        let arg = r.next();
        let mut c = case(r, va, vec![arg]);
        c.script = rows(
            r,
            2,
            &[0, 1, 0xFF, 0x100, 0xFFFF_FFFF, 0x8000_0000, 0x4000_0000],
        );
        c
    }
}
macro_rules! handler_test {
    ($name:ident, $va:expr, $f:path, $expect:expr) => {
        w4_test!(
            $name,
            $va,
            Regs::RET,
            $expect,
            handler($va),
            |s, m, cpu| $f(s, m, cpu)
        );
    };
}
handler_test!(lifted_0003c920, 0x3C920, lc::set_map_byte, &[0x3C8FD0]);
handler_test!(lifted_0003c940, 0x3C940, lc::set_named_player, &[0x6F450]);
handler_test!(lifted_0003c970, 0x3C970, lc::set_player_counts, &[0x3C8FD0]);
handler_test!(lifted_0003c990, 0x3C990, lc::set_mode_byte, &[0x3C8FD0]);
handler_test!(
    lifted_0003d140,
    0x3D140,
    lc::set_global_count,
    &[0x3C8FD0, 0x389B0]
);
handler_test!(lifted_0003d170, 0x3D170, lc::set_scaled_time, &[0x3C8FD0]);
handler_test!(lifted_0003d1a0, 0x3D1A0, lc::set_limit, &[0x3C8FD0]);
handler_test!(lifted_0003d1c0, 0x3D1C0, lc::set_score, &[0x3C8FD0]);
handler_test!(lifted_0003d1e0, 0x3D1E0, lc::set_time, &[0x3C8FD0]);
handler_test!(lifted_0003d200, 0x3D200, lc::set_special_mode, &[0x3C8FD0]);

/// Name comparisons against the guest constants: texts copy a constant, possibly with one byte flipped.
fn build_name(va: u32, table: &'static [(u32, u32)]) -> impl FnMut(&mut Rng, &mut Pair) -> W4Case {
    move |r, p| {
        seed_block(r, p);
        for (a, n) in table {
            let bytes: Vec<u8> = (0..*n).map(|_| b"xyz"[r.below(3) as usize]).collect();
            for (i, b) in bytes.iter().enumerate() {
                p.put8(a + i as u32, *b);
            }
        }
        let (a, n) = table[r.below(table.len() as u32) as usize];
        let text = POOL + 0x400;
        for i in 0..n + 2 {
            p.put8(text + i, 0);
        }
        for i in 0..n {
            let b = p.get(false, a + i);
            p.put8(text + i, b);
        }
        if r.below(3) == 0 {
            let at = text + r.below(n);
            p.put8(at, b'q');
        }
        case(r, va, vec![text])
    }
}
const NAMES: &[(u32, u32)] = &[(0x4780A8, 5), (0x4780A0, 7), (0x478098, 5)];
w4_test!(
    lifted_0003cee0,
    0x3CEE0,
    Regs::RET,
    &[],
    build_name(0x3CEE0, NAMES),
    |_, m, cpu| lc::set_variant_name(m, cpu)
);
w4_test!(
    lifted_0003d2b0,
    0x3D2B0,
    Regs::RET,
    &[],
    build_name(0x3D2B0, &[(0x4780B0, 7)]),
    |_, m, cpu| lc::set_global_switch(m, cpu)
);
