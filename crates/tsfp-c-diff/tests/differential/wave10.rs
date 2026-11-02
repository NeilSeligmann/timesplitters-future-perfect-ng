// SPDX-License-Identifier: GPL-3.0-or-later
//! T1835 wave 10: Rust ports of the straight-line, loop and stack-op inventory roots
//! (`tsfp-engine-physics/src/fp_wave10.rs`) against the unmodified C replacements. Same driver as
//! wave 9 (`s9_drive`): EAX/ECX/EDX, ESI, EDI, EBX, EBP, DF, ESP, XMM, a stack window below the
//! entry ESP and every written range are compared, every suite also runs a perturbation negative
//! control. Builders create fresh, acyclic and bounded structures (a random count or a stale
//! cycle would loop for billions of iterations) and scribble non-zero words over every region a
//! body fills, so a missing, shifted or mirrored write is visible. See docs/t1835-rust-wave10.md.
use super::wave9::{POOL_A, POOL_B, POOL_C, S9, s9, s9_drive, scribble};
use super::*;
use tsfp_engine_physics::fp_wave10 as w10;

/// The k-th disjoint region (0x2000 apart) of a pool, so two structures of one case never overlap.
fn reg(rng: &mut Rng, pool: u32, k: u32) -> u32 {
    pool + k * 0x2000 + rng.below(0x40) * 4
}

fn pick(rng: &mut Rng, values: &[u32]) -> u32 {
    values[rng.below(values.len() as u32) as usize]
}

/// A flag word: empty, full, one bit, or dense and sparse random bits.
fn flagsw(rng: &mut Rng) -> u32 {
    match rng.below(6) {
        0 => 0,
        1 => 0xffff_ffff,
        2 => 1 << rng.below(32),
        3 => rng.next(),
        4 => rng.next() & rng.next(),
        _ => rng.next() | rng.next(),
    }
}

fn maybe_null(rng: &mut Rng, value: u32, one_in: u32) -> u32 {
    if rng.below(one_in) == 0 { 0 } else { value }
}

/// `maybe_null` whose value expression may itself use the generator.
macro_rules! nul {
    ($rng:expr, $value:expr, $one_in:expr) => {{
        let value = $value;
        maybe_null($rng, value, $one_in)
    }};
}

const OWNERS: u32 = 0x0072_f5e8;

/// Owner table (4 rows of 0x2E8) at 72F5E8, returns its base.
fn owner_table(rng: &mut Rng, pair: &mut Pair) -> u32 {
    let table = reg(rng, POOL_C, 7);
    pair.put32(OWNERS, table);
    table
}

/// A slot handle (row << 22 | tag) whose owner row holds the same value (valid) or not (stale).
fn handle(rng: &mut Rng, pair: &mut Pair, table: u32, valid: bool) -> u32 {
    let row = rng.below(4);
    let value = (row << 22) | (rng.next() & 0x003f_ffff) | 1;
    let stored = if valid { value } else { value ^ (1 << rng.below(22)) };
    pair.put32(table.wrapping_add(row * 0x2e8 + 0x2c), stored);
    value
}

/// Handle slots of stride 0x34 starting at `first`: mostly valid, some empty or stale.
fn slots(rng: &mut Rng, pair: &mut Pair, table: u32, first: u32, count: u32) {
    for k in 0..count {
        let value = match rng.below(8) {
            0 => 0,
            1 => handle(rng, pair, table, false),
            _ => handle(rng, pair, table, true),
        };
        pair.put32(first.wrapping_add(k * 0x34), value);
    }
}

/// A slot-count word: mostly small positive, sometimes zero or negative.
fn slot_count(rng: &mut Rng) -> u32 {
    match rng.below(10) {
        0 => 0,
        1 => pick(rng, &[0xffff_ffff, 0x8000_0000]),
        _ => 1 + rng.below(4),
    }
}

// ---- 00012EC0 gated object flag ----
fn build_12ec0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    pair.put32(0x007d_e458, pick(rng, &[0, 0x40, 0x41, 0xbf, 0x80]));
    pair.put32(0x0079_094c, pick(rng, &[100, 56, 102, 0, 7, 100, 102]));
    pair.put32(0x007d_e455, pick(rng, &[10, 9, 11, 0]));
    pair.put32(object + 0x28, flagsw(rng) | if rng.below(2) == 0 { 0x0200_0000 } else { 0 });
    { let args = vec![object]; s9(rng, 0x12ec0, args, false) }
}
s9_suite!(r_00012ec0, mutant_00012ec0, 0x1835_a001, false, build_12ec0, |f, c| {
    w10::gated_object_flag_02000000(f.m, c)
});

// ---- 00070D10 block array dword ----
fn build_70d10(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let block = reg(rng, POOL_B, 0);
    pair.put32(object + 0x90, block);
    let index = if rng.below(8) == 0 { rng.next() } else { rng.below(8) };
    for slot in 0..10 {
        pair.put32(block + 0x28 + 4 * slot, rng.next());
    }
    { let args = vec![object, index]; s9(rng, 0x70d10, args, false) }
}
s9_suite!(r_00070d10, mutant_00070d10, 0x1835_a002, false, build_70d10, |f, c| {
    w10::block_array_dword(f.m, c)
});

// ---- 000718E0 follow chain ----
fn build_718e0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = nul!(rng, reg(rng, POOL_A, 0), 8);
    let nodes: Vec<u32> = (0..10).map(|i| reg(rng, POOL_C, i)).collect();
    pair.put32(object + 0xd8, nul!(rng, nodes[0], 6));
    for i in 0..9 {
        pair.put32(nodes[i] + 0x10, if rng.below(5) == 0 { 0 } else { nodes[i + 1] });
    }
    pair.put32(nodes[9] + 0x10, rng.next());
    let steps = match rng.below(8) {
        0 => pick(rng, &[0xffff_ffff, 0x8000_0000, 0xffff_fffe]),
        _ => rng.below(9),
    };
    { let args = vec![object, steps]; s9(rng, 0x718e0, args, false) }
}
s9_suite!(r_000718e0, mutant_000718e0, 0x1835_a003, false, build_718e0, |f, c| {
    w10::follow_chain(f.m, c)
});

// ---- list builders: nodes of a singly linked list in pool `pool`, `next` at `next_off` ----
fn chain(rng: &mut Rng, pair: &mut Pair, pool: u32, base_k: u32, count: u32, next_off: u32) -> Vec<u32> {
    let nodes: Vec<u32> = (0..count).map(|i| reg(rng, pool, base_k + i)).collect();
    for i in 0..count as usize {
        let next = if i + 1 == count as usize || rng.below(9) == 0 { 0 } else { nodes[i + 1] };
        pair.put32(nodes[i] + next_off, next);
    }
    nodes
}

// ---- 00071970 nth matching node ----
fn build_71970(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let nodes = chain(rng, pair, POOL_B, 0, 5, 4);
    pair.put32(object + 0x178, nul!(rng, nodes[0], 8));
    for node in &nodes {
        pair.put32(node + 0x14, 1 + rng.below(3));
    }
    { let args = vec![object, 1 + rng.below(3), rng.below(5)]; s9(rng, 0x71970, args, false) }
}
s9_suite!(r_00071970, mutant_00071970, 0x1835_a004, false, build_71970, |f, c| {
    w10::nth_matching_node(f.m, c)
});

// ---- 00071CD0 set flag bit ----
fn build_71cd0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    pair.put32(object + 0xc, flagsw(rng));
    let enable = { let any = rng.next(); pick(rng, &[0, 1, 2, 0x8000_0000, any]) };
    let mut s = { let args = vec![object, enable]; s9(rng, 0x71cd0, args, false) };
    s.case.watch.push((object, 0x10));
    s
}
s9_suite!(r_00071cd0, mutant_00071cd0, 0x1835_a005, false, build_71cd0, |f, c| {
    w10::set_flag_bit(f.m, c)
});

// ---- 00074670 find node by key ----
fn build_74670(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let nodes = chain(rng, pair, POOL_B, 0, 5, 0x30);
    pair.put32(object + 0x16c, nul!(rng, nodes[0], 8));
    for node in &nodes {
        pair.put32(node + 4, 1 + rng.below(4));
    }
    { let args = vec![object, 1 + rng.below(5)]; s9(rng, 0x74670, args, false) }
}
s9_suite!(r_00074670, mutant_00074670, 0x1835_a006, false, build_74670, |f, c| {
    w10::find_node_by_key(f.m, c)
});

// ---- 00074840 clear slot ids ----
fn build_74840(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    scribble(rng, pair, object + 0x120, 0x60, 24);
    let mut s = { let args = vec![object]; s9(rng, 0x74840, args, false) };
    s.case.watch.push((object + 0x120, 0x60));
    s
}
s9_suite!(r_00074840, mutant_00074840, 0x1835_a007, false, build_74840, |f, c| {
    w10::clear_slot_ids(f.m, c)
});

// ---- 00074950 set slot value ----
fn build_74950(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    for slot in 0..4 {
        pair.put32(object + 0x128 + 0x10 * slot, 1 + rng.below(5));
        pair.put32(object + 0x12c + 0x10 * slot, rng.next());
    }
    let mut s = { let args = vec![object, 1 + rng.below(6), rng.next()]; s9(rng, 0x74950, args, false) };
    s.case.watch.push((object + 0x120, 0x60));
    s
}
s9_suite!(r_00074950, mutant_00074950, 0x1835_a008, false, build_74950, |f, c| {
    w10::set_slot_value(f.m, c)
});

// ---- 00074AB0 matching node child index ----
fn build_74ab0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let nodes = chain(rng, pair, POOL_B, 0, 4, 4);
    pair.put32(object + 0x178, nul!(rng, nodes[0], 8));
    let mut s = { let args = vec![]; s9(rng, 0x74ab0, args, false) };
    for (k, node) in nodes.iter().enumerate() {
        pair.put32(node + 0x14, 1 + rng.below(3));
        let kids = chain(rng, pair, POOL_C, 4 * k as u32, 4, 16);
        pair.put32(node + 0xd8, nul!(rng, kids[0], 6));
        for kid in &kids {
            pair.put32(kid + 8, 1 + rng.below(5));
        }
        s.case.watch.push((node + 0xc8, 8));
    }
    let (key, id) = (1 + rng.below(3), 1 + rng.below(5));
    s.case.args = vec![object, key, id];
    s
}
s9_suite!(r_00074ab0, mutant_00074ab0, 0x1835_a009, false, build_74ab0, |f, c| {
    w10::matching_node_child_index(f.m, c)
});

// ---- 00074B80 propagate flags ----
fn build_74b80(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let nodes = chain(rng, pair, POOL_B, 0, 4, 0x30);
    pair.put32(object + 0x16c, nul!(rng, nodes[0], 6));
    pair.put32(object + 0x40, flagsw(rng));
    pair.put32(object + 0x48, flagsw(rng));
    let mut s = { let args = vec![object]; s9(rng, 0x74b80, args, false) };
    for node in &nodes {
        pair.put32(node + 0x40, flagsw(rng));
        pair.put32(node + 0x48, flagsw(rng));
        s.case.watch.push((node + 0x40, 0x10));
    }
    s.case.watch.push((object + 0x40, 0x10));
    s
}
s9_suite!(r_00074b80, mutant_00074b80, 0x1835_a00a, false, build_74b80, |f, c| {
    w10::propagate_flags(f.m, c)
});

// ---- 00087A40 table 708778 bit set / clear ----
fn build_87a40(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let table = reg(rng, POOL_B, 0);
    pair.put32(0x0070_8778, table);
    for word in 0..0x40 {
        pair.put32(table + 4 * word, rng.next() | rng.next());
    }
    let index = rng.below(6);
    let bit = if rng.below(4) == 0 { rng.next() } else { rng.below(16) };
    let mut s = { let args = vec![index, bit]; s9(rng, 0x87a40, args, false) };
    s.case.watch.push((table, 0x100));
    s
}
s9_suite!(r_00087a40, mutant_00087a40, 0x1835_a00b, false, build_87a40, |f, c| {
    w10::table_708778_set_bit_clear_bit(f.m, c)
});

// ---- 00094290 set word 7c and header flag ----
fn build_94290(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let header = reg(rng, POOL_B, 0);
    pair.put32(object, header);
    pair.put32(object + 0x7c, rng.next());
    pair.put32(header + 0x18, flagsw(rng));
    let mut s = { let args = vec![object, rng.next()]; s9(rng, 0x94290, args, false) };
    s.case.watch.push((object + 0x7c, 4));
    s.case.watch.push((header + 0x18, 4));
    s
}
s9_suite!(r_00094290, mutant_00094290, 0x1835_a00c, false, build_94290, |f, c| {
    w10::set_word_7c_and_header_flag(f.m, c)
});

// ---- 000942D0 flags clear word ----
fn build_942d0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    pair.put32(object + 0x1f4, rng.next());
    pair.put32(object + 0x1f8, rng.next());
    let mut s = { let args = vec![object, flagsw(rng)]; s9(rng, 0x942d0, args, false) };
    s.case.watch.push((object + 0x1f4, 8));
    s
}
s9_suite!(r_000942d0, mutant_000942d0, 0x1835_a00d, false, build_942d0, |f, c| {
    w10::flags_clear_word(f.m, c)
});

// ---- 0009E730 mapped entry value ----
fn build_9e730(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let table = reg(rng, POOL_B, 0);
    pair.put32(0x0079_1d4c, table);
    for word in 0..0x80 {
        pair.put32(table + 4 * word, rng.next());
    }
    for word in 0..16u32 {
        let half = |rng: &mut Rng| pick(rng, &[0, 1, 2, 3, 5, 9, 0xffff, 0x8000, 0x7fff]);
        let value = half(rng) | (half(rng) << 16);
        pair.put32(0x0078_b860 + 4 * word, value);
    }
    let slot = if rng.below(8) == 0 { rng.next() } else { rng.below(32) };
    { let args = vec![slot]; s9(rng, 0x9e730, args, false) }
}
s9_suite!(r_0009e730, mutant_0009e730, 0x1835_a00e, false, build_9e730, |f, c| {
    w10::mapped_entry_value(f.m, c)
});

// ---- 000A2C60 slot find next by type ----
fn build_a2c60(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let first = reg(rng, POOL_B, 0);
    let count = rng.below(7);
    pair.put32(0x007b_4fac, count);
    pair.put32(OWNERS, first);
    for slot in 0..9 {
        pair.put32(first + 0x2e8 * slot + 0x20, pick(rng, &[1, 2, 2, 3, 0xffff_ffff, 4]));
    }
    let previous = match rng.below(6) {
        0 => 0,
        1 => first + 0x2e8 * rng.below(9) + 4,
        2 => rng.below(32),
        _ => first + 0x2e8 * rng.below(9),
    };
    let kind = pick(rng, &[1, 2, 3, 0xffff_ffff, 4, 2]);
    { let args = vec![previous, kind]; s9(rng, 0xa2c60, args, false) }
}
s9_suite!(r_000a2c60, mutant_000a2c60, 0x1835_a00f, false, build_a2c60, |f, c| {
    w10::slot_find_next_by_type(f.m, c)
});

// ---- 000A4C70 wrapping capacity signed test ----
fn build_a4c70(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let small = [0, 1, 4, 0x7fff_ffff, 0x8000_0000, 0xffff_fffc, 100];
    for address in [0x007b_65b8u32, 0x007b_4fac, 0x0072_f5e4] {
        pair.put32(address, rng.word(&small));
    }
    let arg = rng.word(&small);
    { let args = vec![arg]; s9(rng, 0xa4c70, args, false) }
}
s9_suite!(r_000a4c70, mutant_000a4c70, 0x1835_a010, false, build_a4c70, |f, c| {
    w10::wrapping_capacity_signed_test(f.m, c)
});

// ---- fill roots: rep stosd regions (DF honoured) ----
/// Region a downward or upward fill of `dwords` dwords starting at `start` touches.
fn fill_region(start: u32, dwords: u32, down: bool) -> (u32, u32) {
    if down {
        (start.wrapping_sub(4 * (dwords - 1)), 4 * dwords)
    } else {
        (start, 4 * dwords)
    }
}

// ---- 000B0590 / 000C2E30 table clears ----
fn build_fill(rng: &mut Rng, pair: &mut Pair, va: u32, pointer: u32, dwords: u32, extra_word: u32) -> S9 {
    let target = rng.pool(POOL_A) + 0x3000;
    pair.put32(pointer, target);
    let mut s = { let args = vec![]; s9(rng, va, args, true) };
    let (start, len) = fill_region(target, dwords, s.extra[3] != 0);
    scribble(rng, pair, start, len, 96);
    pair.put32(extra_word, rng.next() | 1);
    s.case.watch.push((start, len));
    s.case.watch.push((extra_word, 4));
    s
}
fn build_b0590(rng: &mut Rng, pair: &mut Pair) -> S9 {
    build_fill(rng, pair, 0xb0590, 0x0073_2a04, 0x9a8, 0x0073_2a08)
}
s9_suite!(r_000b0590, mutant_000b0590, 0x1835_a011, false, build_b0590, |f, c| {
    w10::table_732a04_clear(f.m, c)
});
fn build_c2e30(rng: &mut Rng, pair: &mut Pair) -> S9 {
    build_fill(rng, pair, 0xc2e30, 0x0073_2e70, 0x528, 0x0073_2e74)
}
s9_suite!(r_000c2e30, mutant_000c2e30, 0x1835_a012, false, build_c2e30, |f, c| {
    w10::table_732e70_clear(f.m, c)
});

// ---- 000B0770 current or previous reference ----
fn build_b0770(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let current = nul!(rng, pick(rng, &[0x1000, 0x2000, 0x40, 0x8000_0000]), 5);
    let delta = pick(rng, &[0, 0x10, 0x1000, 0xffff_fff0]);
    pair.put32(0x007d_e318, current);
    pair.put32(0x004e_7944, delta);
    let reference = match rng.below(5) {
        0 => 0,
        1 => current,
        2 => current.wrapping_sub(delta),
        _ => rng.next(),
    };
    pair.put32(object + 0x90, reference);
    { let args = vec![object]; s9(rng, 0xb0770, args, false) }
}
s9_suite!(r_000b0770, mutant_000b0770, 0x1835_a013, false, build_b0770, |f, c| {
    w10::current_or_previous_reference(f.m, c)
});

// ---- 000B0F90 nullable extension state 0 ----
fn build_b0f90(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = nul!(rng, reg(rng, POOL_A, 0), 6);
    let ext = reg(rng, POOL_B, 0);
    pair.put32(object + 0x7c, ext);
    pair.put32(ext + 0x10, pick(rng, &[0, 0, 1, 2, 0x8000_0000]));
    { let args = vec![object]; s9(rng, 0xb0f90, args, false) }
}
s9_suite!(r_000b0f90, mutant_000b0f90, 0x1835_a014, false, build_b0f90, |f, c| {
    w10::nullable_extension_state0(f.m, c)
});

// ---- 000B2D40 clear matching slots ----
fn build_b2d40(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let value = pick(rng, &[0, 1, 5, 0xdead_beef]);
    let count = match rng.below(10) {
        0 => pick(rng, &[0xffff_ffff, 0x8000_0000, 0xffff_fffe]),
        _ => rng.below(12),
    };
    pair.put32(0x0073_2a10, count);
    for slot in 0..12 {
        let word = if rng.below(2) == 0 { value } else { pick(rng, &[0, 1, 5, 7, 0xdead_beef]) };
        pair.put32(0x007b_2e20 + 4 * slot, word);
    }
    let mut s = { let args = vec![value]; s9(rng, 0xb2d40, args, false) };
    s.case.watch.push((0x007b_2e20, 0x30));
    s
}
s9_suite!(r_000b2d40, mutant_000b2d40, 0x1835_a015, false, build_b2d40, |f, c| {
    w10::clear_matching_slots(f.m, c)
});

// ---- 000B3CA0 optional record lookup ----
fn build_b3ca0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let ext = nul!(rng, reg(rng, POOL_B, 0), 6);
    pair.put32(object + 0x7c, ext);
    pair.put32(ext + 8, flagsw(rng) | if rng.below(2) == 0 { 0x40 } else { 0 });
    pair.put32(ext + 12, pick(rng, &[0, 1, 24, 25, 26, 0xffff_ffff, 0x8000_0000, 7]));
    { let args = vec![object]; s9(rng, 0xb3ca0, args, false) }
}
s9_suite!(r_000b3ca0, mutant_000b3ca0, 0x1835_a016, false, build_b3ca0, |f, c| {
    w10::optional_record_lookup(f.m, c)
});

// ---- 000C7360 pair fields type / flag conflict (ECX = object) ----
fn build_c7360(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let stack_slot = |rng: &mut Rng| STACK - 4 * (1 + rng.below(3));
    let first = if rng.below(10) == 0 { stack_slot(rng) } else { reg(rng, POOL_B, 0) };
    let second = match rng.below(10) {
        0 => 0,
        1 => stack_slot(rng),
        _ => reg(rng, POOL_C, 0),
    };
    pair.put32(object + 0x26c, first);
    pair.put32(object + 0x270, second);
    pair.put32(object + 0x2a8, pick(rng, &[0, 1, 5]));
    let flags = |rng: &mut Rng| { let any = rng.next(); pick(rng, &[0, 1, 0x400, 0x401, 0x800, any]) };
    let kind = |rng: &mut Rng| pick(rng, &[0x13, 0, 0, 1, 7]);
    if first < STACK - 64 || first > STACK {
        pair.put32(first, kind(rng));
        pair.put32(first + 4, flags(rng));
    }
    if second != 0 && (second < STACK - 64 || second > STACK) {
        pair.put32(second, kind(rng));
        pair.put32(second + 4, flags(rng));
    }
    let mut s = { let args = vec![]; s9(rng, 0xc7360, args, false) };
    s.case.regs[1] = object;
    s
}
s9_suite!(r_000c7360, mutant_000c7360, 0x1835_a017, false, build_c7360, |f, c| {
    w10::pair_fields_type_flag_conflict(f.m, c)
});

// ---- 000DA220 mode visibility ----
fn build_da220(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let node = reg(rng, POOL_A, 0);
    pair.put32(node + 0x48, pick(rng, &[0x15, 0x15, 0x14, 0x0, 0xffff_0015]));
    pair.put32(node + 0x14, flagsw(rng) | pick(rng, &[0, 0x80]));
    pair.put32(node + 0x10, flagsw(rng));
    pair.put32(node + 0x18, flagsw(rng));
    pair.put32(node + 0x1c, flagsw(rng));
    pair.put32(node + 8, pick(rng, &[0, 5, 10, 0xffff_ffff, 0x8000_0000]));
    pair.put32(0x007d_e455, pick(rng, &[10, 9, 10]));
    pair.put32(0x0079_0950, pick(rng, &[0, 5, 10, 0xffff_ffff, 0x8000_0000]));
    { let args = vec![node]; s9(rng, 0xda220, args, false) }
}
s9_suite!(r_000da220, mutant_000da220, 0x1835_a018, false, build_da220, |f, c| {
    w10::mode_visibility(f.m, c)
});

// ---- 0010E780 store byte 13 ----
fn build_10e780(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = nul!(rng, reg(rng, POOL_A, 0), 5);
    pair.put32(object + 0x10, rng.next() | 0x0101_0101);
    let mut s = { let args = vec![object, rng.next()]; s9(rng, 0x10e780, args, false) };
    s.case.watch.push((object + 0x10, 8));
    s
}
s9_suite!(r_0010e780, mutant_0010e780, 0x1835_a019, false, build_10e780, |f, c| {
    w10::store_byte_13(f.m, c)
});

// ---- 0011D990 tag block ----
fn build_11d990(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = nul!(rng, reg(rng, POOL_A, 0), 5);
    let block = reg(rng, POOL_B, 0);
    pair.put32(object + 0x7c, block);
    pair.put32(block + 0xc6c, flagsw(rng));
    pair.put32(block + 0xd38, rng.next());
    let mut s = { let args = vec![object, rng.next()]; s9(rng, 0x11d990, args, false) };
    s.case.watch.push((block + 0xc6c, 4));
    s.case.watch.push((block + 0xd38, 4));
    s
}
s9_suite!(r_0011d990, mutant_0011d990, 0x1835_a01a, false, build_11d990, |f, c| {
    w10::tag_block(f.m, c)
});

// ---- 001286D0 tables 742360 / 7abfa0 reset ----
fn build_1286d0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let mut s = { let args = vec![]; s9(rng, 0x1286d0, args, true) };
    let down = s.extra[3] != 0;
    for (start, dwords) in [(0x0074_2360u32, 108u32), (0x007a_bfa0, 380)] {
        let (from, len) = fill_region(start, dwords, down);
        scribble(rng, pair, from, len, 48);
        s.case.watch.push((from, len));
    }
    for address in [0x0074_235cu32, 0x007a_bf80, 0x0074_2358, 0x0074_97a8] {
        pair.put32(address, rng.next() | 1);
        s.case.watch.push((address, 4));
    }
    s
}
s9_suite!(r_001286d0, mutant_001286d0, 0x1835_a01b, false, build_1286d0, |f, c| {
    w10::tables_742360_7abfa0_reset(f.m, c)
});

// ---- 00158D40 ext clear state flag masks ----
fn build_158d40(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let ext = reg(rng, POOL_B, 0);
    pair.put32(object + 0x7c, ext);
    pair.put32(ext + 0x28, flagsw(rng));
    pair.put32(ext + 0x2c, flagsw(rng));
    let mask = |rng: &mut Rng| {
        let any = rng.next();
        pick(rng, &[0, 0x0400_0000, 0x0800_0000, 0x0c00_0000, any])
    };
    let args = vec![object, mask(rng), rng.next(), mask(rng)];
    let mut s = s9(rng, 0x158d40, args, false);
    s.case.watch.push((ext + 0x28, 8));
    s
}
s9_suite!(r_00158d40, mutant_00158d40, 0x1835_a01c, false, build_158d40, |f, c| {
    w10::ext_clear_state_flag_masks(f.m, c)
});

// ---- 0015A270 related value ----
fn build_15a270(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let ext1 = reg(rng, POOL_B, 0);
    let owner = nul!(rng, reg(rng, POOL_C, 0), 6);
    let ext2 = reg(rng, POOL_B, 1);
    let table = reg(rng, POOL_A, 1);
    let holder = reg(rng, POOL_C, 1);
    let ext3 = reg(rng, POOL_B, 2);
    pair.put32(object + 0x7c, ext1);
    pair.put32(ext1 + 4, owner);
    pair.put32(ext1 + 0x684, rng.next());
    pair.put32(owner + 0x20, pick(rng, &[1, 8, 9, 0x200, 0x201, 0x2000, 0x2001, 0x100, 2, 0]));
    pair.put32(owner + 0x110, rng.next());
    pair.put32(owner + 0x7c, ext2);
    pair.put32(ext2 + 4, rng.next());
    pair.put32(ext2 + 0x678, rng.next());
    pair.put32(0x007b_0c48, table);
    pair.put32(table + 0x14, holder);
    pair.put32(holder + 0x7c, ext3);
    pair.put32(ext3 + 0x678, rng.next());
    { let args = vec![object]; s9(rng, 0x15a270, args, false) }
}
s9_suite!(r_0015a270, mutant_0015a270, 0x1835_a01d, false, build_15a270, |f, c| {
    w10::related_value(f.m, c)
});

// ---- 0015C120 clear matching owner ----
fn build_15c120(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let owner = pick(rng, &[0, 1, 2, 5, 0x1000]);
    let count = match rng.below(10) {
        0 => pick(rng, &[0xffff_ffff, 0x8000_0000]),
        _ => rng.below(7),
    };
    pair.put32(0x0074_c37c, count);
    let mut s = { let args = vec![owner]; s9(rng, 0x15c120, args, false) };
    for slot in 0..6u32 {
        let record = nul!(rng, reg(rng, POOL_B, slot), 6);
        pair.put32(0x007a_3580 + 4 * slot, record);
        if record != 0 {
            pair.put32(record + 4, if rng.below(2) == 0 { owner } else { pick(rng, &[0, 1, 2, 5]) });
            pair.put32(record + 0x2c, flagsw(rng));
            s.case.watch.push((record + 4, 4));
            s.case.watch.push((record + 0x2c, 4));
        }
    }
    s
}
s9_suite!(r_0015c120, mutant_0015c120, 0x1835_a01e, false, build_15c120, |f, c| {
    w10::clear_matching_owner(f.m, c)
});

// ---- 0015C3D0 clear inactive flag and append ----
fn build_15c3d0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let record = reg(rng, POOL_B, 0);
    let header = reg(rng, POOL_C, 0);
    pair.put32(object + 0x7c, record);
    pair.put32(record + 0x2c, flagsw(rng) | if rng.below(5) == 0 { 0 } else { 4 });
    pair.put32(record, header);
    pair.put32(header + 0x28, flagsw(rng));
    pair.put32(header + 0x7c, rng.next());
    pair.put32(record + 0x28, if rng.below(3) == 0 { 0x80 } else { rng.next() & 0x7f });
    pair.put32(record + 0x10, if rng.below(3) == 0 { 0x0400_0000 } else { rng.next() & 0xfbff_ffff });
    pair.put32(record + 8, pick(rng, &[0, 5, 10, 0xffff_ffff, 0x8000_0000]));
    pair.put32(0x0079_0950, pick(rng, &[0, 5, 10, 0xffff_ffff, 0x8000_0000]));
    let count = match rng.below(8) {
        0 => rng.next(),
        _ => rng.below(8),
    };
    pair.put32(0x0074_c37c, count);
    let mut s = { let args = vec![object]; s9(rng, 0x15c3d0, args, false) };
    s.case.watch.push((count.wrapping_mul(4).wrapping_add(0x007a_3580), 4));
    s.case.watch.push((record + 0x2c, 4));
    s.case.watch.push((header + 0x28, 4));
    s
}
s9_suite!(r_0015c3d0, mutant_0015c3d0, 0x1835_a01f, false, build_15c3d0, |f, c| {
    w10::clear_inactive_flag_and_append(f.m, c)
});

// ---- 00160850 append flagged object ----
fn build_160850(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let ext = reg(rng, POOL_B, 0);
    pair.put32(object + 0x7c, ext);
    pair.put32(ext + 8, flagsw(rng) | if rng.below(3) == 0 { 0 } else { 0x40 });
    let count = match rng.below(8) {
        0 => rng.word(&[0xffff_ffff, 0x8000_0000, 5, 4]),
        _ => rng.below(7),
    };
    pair.put32(0x007a_36c8, count);
    let mut s = { let args = vec![object]; s9(rng, 0x160850, args, false) };
    s.case.watch.push((count.wrapping_mul(4).wrapping_add(0x007a_3560), 4));
    s.case.watch.push((0x007a_36c8, 4));
    s
}
s9_suite!(r_00160850, mutant_00160850, 0x1835_a020, false, build_160850, |f, c| {
    w10::append_flagged_object(f.m, c)
});

// ---- 00160F50 vehicle tag test ----
fn build_160f50(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let ext = reg(rng, POOL_B, 0);
    let tag_holder = reg(rng, POOL_C, 0);
    pair.put32(object + 0x20, pick(rng, &[8, 8, 8, 7, 9, 0]));
    pair.put32(object + 0x7c, ext);
    pair.put32(ext + 0xbf0, tag_holder);
    pair.put32(tag_holder + 8, pick(rng, &[0x4543_4956, 0x4f43_4956, 0x4543_4957, 0, 0x5643_4549]));
    { let args = vec![object]; s9(rng, 0x160f50, args, false) }
}
s9_suite!(r_00160f50, mutant_00160f50, 0x1835_a021, false, build_160f50, |f, c| {
    w10::vehicle_tag_test(f.m, c)
});

// ---- 00163810 global 79094c ne 4 plus 7 ----
fn build_163810(rng: &mut Rng, pair: &mut Pair) -> S9 {
    pair.put32(0x0079_094c, pick(rng, &[4, 4, 3, 5, 0, 0xffff_ffff]));
    { let args = vec![]; s9(rng, 0x163810, args, false) }
}
s9_suite!(r_00163810, mutant_00163810, 0x1835_a022, false, build_163810, |f, c| {
    w10::global_79094c_ne_4_plus_7(f.m, c)
});

// ---- 001665A0 pool 74c468 pop ----
fn build_1665a0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let count = pick(rng, &[0, 0xffff_ffff, 0x8000_0000, 1, 2, 7, 100, 0x7fff_ffff]);
    pair.put32(0x004e_826c, count);
    pair.put32(0x0074_c468, rng.pool(POOL_A));
    let mut s = { let args = vec![]; s9(rng, 0x1665a0, args, false) };
    s.case.watch.push((0x004e_826c, 4));
    s
}
s9_suite!(r_001665a0, mutant_001665a0, 0x1835_a023, false, build_1665a0, |f, c| {
    w10::pool_74c468_pop(f.m, c)
});

// ---- 00167A70 entry 74c458 field 17c ----
fn build_167a70(rng: &mut Rng, pair: &mut Pair) -> S9 {
    pair.put32(0x0074_c458, rng.next());
    let index = if rng.below(2) == 0 { rng.next() } else { rng.below(0x20000) };
    { let args = vec![index]; s9(rng, 0x167a70, args, false) }
}
s9_suite!(r_00167a70, mutant_00167a70, 0x1835_a024, false, build_167a70, |f, c| {
    w10::entry_74c458_field_17c(f.m, c)
});

// ---- 00172C80 component slot next in ring ----
fn build_172c80(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let block = reg(rng, POOL_B, 0);
    pair.put32(object + 0x7c, block);
    let count = match rng.below(8) {
        0 => 0x7fff_ffff,
        1 => (-(2 + rng.below(4) as i32)) as u32,
        _ => 1 + rng.below(6),
    };
    pair.put32(block + 0x140, count);
    let cell = match rng.below(8) {
        0 => STACK - 4,
        1 => STACK - 8,
        _ => reg(rng, POOL_C, 0),
    };
    let wanted = match rng.below(6) {
        0 => 0xffff_ffff,
        1 => rng.next() & 0x7fff_ffff,
        2 => count.wrapping_sub(1),
        _ => rng.below(8),
    };
    // The cell may be a saved slot of the body (a changed saved ESI/EDI is the aliasing case).
    if cell >= STACK - 8 {
        // the body pushes before it reads the cell, so the read sees the saved register
    } else {
        pair.put32(cell, wanted);
    }
    let mut s = { let args = vec![object, cell]; s9(rng, 0x172c80, args, false) };
    s.case.watch.push((cell, 4));
    s
}
s9_suite!(r_00172c80, mutant_00172c80, 0x1835_a025, true, build_172c80, |f, c| {
    w10::component_slot_next_in_ring(f.m, c)
});

// ---- slot handle roots (invalid / set) ----
fn build_1766e0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let ext = reg(rng, POOL_B, 0);
    let table = owner_table(rng, pair);
    pair.put32(object + 0x7c, ext);
    slots(rng, pair, ext + 0x70, table, 0);
    let index = rng.below(4);
    let value = match rng.below(4) {
        0 => 0,
        1 => handle(rng, pair, table, false),
        _ => handle(rng, pair, table, true),
    };
    pair.put32(ext + 0x70 + 0x34 * index, value);
    let mut s = { let args = vec![object, index]; s9(rng, 0x1766e0, args, false) };
    s.case.watch.push((ext + 0x70 + 0x34 * index, 4));
    s
}
s9_suite!(r_001766e0, mutant_001766e0, 0x1835_a026, false, build_1766e0, |f, c| {
    w10::slot_handle_invalid(f.m, c)
});

fn build_176a40(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let owner = reg(rng, POOL_A, 1);
    let ext = reg(rng, POOL_B, 0);
    let table = owner_table(rng, pair);
    pair.put32(object + 0x7c, ext);
    pair.put32(owner + 0x2c, rng.next() | 1);
    pair.put32(owner + 0x30, rng.next());
    let index = rng.below(4);
    let value = match rng.below(4) {
        0 => 0,
        1 => handle(rng, pair, table, false),
        _ => handle(rng, pair, table, true),
    };
    pair.put32(ext + 0x70 + 0x34 * index, value);
    let mut s = { let args = vec![object, owner, index]; s9(rng, 0x176a40, args, false) };
    s.case.watch.push((ext + 0x70 + 0x34 * index, 4));
    s.case.watch.push((owner + 0x2c, 8));
    s
}
s9_suite!(r_00176a40, mutant_00176a40, 0x1835_a027, false, build_176a40, |f, c| {
    w10::slot_handle_set(f.m, c)
});

// ---- 00177CE0 any ready slot ----
fn build_177ce0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let block = reg(rng, POOL_A, 0);
    let table = owner_table(rng, pair);
    let count = slot_count(rng);
    
    slots(rng, pair, block + 0x70, table, 4);
    pair.put32(block + 0x140, count);
    let mut s = { let args = vec![block]; s9(rng, 0x177ce0, args, false) };
    for k in 0..4u32 {
        pair.put32(block + 0x70 + 0x34 * k + 0x30, if rng.below(4) == 0 { rng.next() } else { 0 });
        let controller = nul!(rng, reg(rng, POOL_C, k), 4);
        for row in 0..4u32 {
            pair.put32(table + row * 0x2e8 + 0x150, controller);
        }
        pair.put32(controller + 4, pick(rng, &[0, 5, 0xffff_ffff, 0x8000_0000]));
        s.case.watch.push((block + 0x70 + 0x34 * k, 4));
    }
    s
}
s9_suite!(r_00177ce0, mutant_00177ce0, 0x1835_a028, false, build_177ce0, |f, c| {
    w10::any_ready_slot(f.m, c)
});

// ---- 00177EC0 init request block ----
fn build_177ec0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let block = reg(rng, POOL_B, 0);
    pair.put32(object + 0x7c, block);
    scribble(rng, pair, block + 0x560, 0x30, 12);
    let mut s = { let args = vec![object, rng.next(), rng.next(), rng.next()]; s9(rng, 0x177ec0, args, false) };
    s.case.watch.push((block + 0x560, 0x30));
    s
}
s9_suite!(r_00177ec0, mutant_00177ec0, 0x1835_a029, false, build_177ec0, |f, c| {
    w10::init_request_block(f.m, c)
});

// ---- 0017A120 find own slot ----
fn build_17a120(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let target = reg(rng, POOL_A, 1);
    let blk = reg(rng, POOL_B, 0);
    let table = owner_table(rng, pair);
    let valid = rng.below(5) != 0;
    let own = nul!(rng, handle(rng, pair, table, valid), 6);
    pair.put32(object + 0xc08, own);
    let row = (own >> 22) as u32;
    pair.put32(table + row * 0x2e8 + 0x7c, blk);
    pair.put32(object, target);
    let wanted = rng.next() | 1;
    pair.put32(target + 0x2c, wanted);
    for k in 0..4u32 {
        let value = if rng.below(3) == 0 { wanted } else { rng.next() };
        pair.put32(blk + 0x70 + 0x34 * k, value);
    }
    pair.put32(blk + 0x140, slot_count(rng));
    let mut s = { let args = vec![object]; s9(rng, 0x17a120, args, false) };
    s.case.watch.push((object + 0xc08, 4));
    s
}
s9_suite!(r_0017a120, mutant_0017a120, 0x1835_a02a, false, build_17a120, |f, c| {
    w10::find_own_slot(f.m, c)
});

// ---- 0017A200 / 0017A260 slot handle scans ----
fn build_slot_scan(rng: &mut Rng, pair: &mut Pair, va: u32, mode_field: bool) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let blk = reg(rng, POOL_B, 0);
    let table = owner_table(rng, pair);
    pair.put32(object + 0x7c, blk);
    
    slots(rng, pair, blk + 0x70, table, 4);
    pair.put32(blk + 0x140, slot_count(rng));
    if mode_field {
        pair.put32(blk + 0x54, pick(rng, &[0, 0, 0, 1, 2, 3, 4, 5, 0x8000_0000]));
        pair.put32(blk + 0x58, if rng.below(5) == 0 { 0x10 } else { rng.next() & 0xffff_ffef });
    }
    let mut s = { let args = vec![object]; s9(rng, va, args, false) };
    for k in 0..4u32 {
        s.case.watch.push((blk + 0x70 + 0x34 * k, 4));
    }
    s
}
fn build_17a200(rng: &mut Rng, pair: &mut Pair) -> S9 {
    build_slot_scan(rng, pair, 0x17a200, false)
}
s9_suite!(r_0017a200, mutant_0017a200, 0x1835_a02b, false, build_17a200, |f, c| {
    w10::count_valid_slot_handles(f.m, c)
});
fn build_17a260(rng: &mut Rng, pair: &mut Pair) -> S9 {
    build_slot_scan(rng, pair, 0x17a260, true)
}
s9_suite!(r_0017a260, mutant_0017a260, 0x1835_a02c, false, build_17a260, |f, c| {
    w10::has_invalid_slot(f.m, c)
});

// ---- 00188920 extension four categories ----
fn build_188920(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = nul!(rng, reg(rng, POOL_A, 0), 6);
    let ext = reg(rng, POOL_B, 0);
    pair.put32(object + 0x7c, ext);
    pair.put32(ext + 0x34, pick(rng, &[2, 3, 5, 7, 0, 1, 4, 6, 8, 0xffff_ffff]));
    { let args = vec![object]; s9(rng, 0x188920, args, false) }
}
s9_suite!(r_00188920, mutant_00188920, 0x1835_a02d, false, build_188920, |f, c| {
    w10::extension_four_categories(f.m, c)
});

// ---- object field roots 1B6310 / 1B63B0 / 1B63D0 / 1B7580 / 1B75B0 ----
fn build_1b6310(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let pending = pick(rng, &[0, 1, 5, 0xffff_ffff]);
    pair.put32(object + 0x34, flagsw(rng));
    pair.put32(object + 0xf8, pending);
    pair.put32(object + 0xe8, if rng.below(2) == 0 { pending } else { pick(rng, &[0, 1, 5, 7]) });
    let mut s = { let args = vec![object]; s9(rng, 0x1b6310, args, false) };
    s.case.watch.push((object + 0x34, 4));
    s.case.watch.push((object + 0xf8, 4));
    s
}
s9_suite!(r_001b6310, mutant_001b6310, 0x1835_a02e, false, build_1b6310, |f, c| {
    w10::set_flag_34_bit2_clear_f8(f.m, c)
});

fn build_1b63b0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let value = pick(rng, &[0, 1, 5, 0xffff_ffff]);
    pair.put32(object + 0xe8, if rng.below(2) == 0 { value } else { rng.next() });
    { let args = vec![object, value]; s9(rng, 0x1b63b0, args, false) }
}
s9_suite!(r_001b63b0, mutant_001b63b0, 0x1835_a02f, false, build_1b63b0, |f, c| {
    w10::field_e8_equals_arg(f.m, c)
});

fn build_1b63d0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let state = pick(rng, &[0, 1, 5, 0xffff_ffff]);
    pair.put32(object + 0xe8, if rng.below(3) != 0 { state } else { rng.next() });
    pair.put32(object + 0xf8, if rng.below(3) == 0 { state } else { rng.next() });
    { let args = vec![object, state]; s9(rng, 0x1b63d0, args, false) }
}
s9_suite!(r_001b63d0, mutant_001b63d0, 0x1835_a030, false, build_1b63d0, |f, c| {
    w10::field_e8_only_matches(f.m, c)
});

fn build_1b7580(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    pair.put32(object + 0xe8, pick(rng, &[10, 11, 12, 27, 9, 13, 26, 28, 0, 0xffff_ffff]));
    { let args = vec![object]; s9(rng, 0x1b7580, args, false) }
}
s9_suite!(r_001b7580, mutant_001b7580, 0x1835_a031, false, build_1b7580, |f, c| {
    w10::kind_is_10_11_12_27(f.m, c)
});

fn build_1b75b0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    for block in [0x184u32, 0x55c] {
        pair.put32(object + block + 0x54, pick(rng, &[0, 0, 1, 5, 0x8000_0000]));
        pair.put32(object + block + 0x68, rng.next() | 1);
    }
    let selector = pick(rng, &[0, 1, 2, 0xffff_ffff, 0x8000_0000]);
    { let args = vec![object, selector]; s9(rng, 0x1b75b0, args, false) }
}
s9_suite!(r_001b75b0, mutant_001b75b0, 0x1835_a032, false, build_1b75b0, |f, c| {
    w10::select_block_get_field_68(f.m, c)
});

// ---- 0023FF70 sorted key index ----
fn build_23ff70(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let count = rng.below(7);
    let mut keys: Vec<u32> = (0..count).map(|_| pick(rng, &[1, 2, 3, 5, 9, 0x8000_0000, 0xffff_fff0, 0x7fff_ffff])).collect();
    keys.sort_unstable();
    let base = 0x0075_daf0u32;
    let order: Vec<u32> = (0..count).map(|_| rng.below(8)).collect();
    for i in 0..count as usize {
        let node = base + 16 * order[i];
        pair.put32(node + 4, keys[i]);
        let next = if i + 1 < count as usize { base + 16 * order[i + 1] } else { 0 };
        pair.put32(node + 8, next);
    }
    pair.put32(0x0075_d318, if count == 0 { 0 } else { base + 16 * order[0] });
    let wanted = match rng.below(3) {
        0 if count > 0 => pick(rng, &keys),
        1 => rng.below(12),
        _ => rng.next(),
    };
    { let args = vec![wanted]; s9(rng, 0x23ff70, args, false) }
}
s9_suite!(r_0023ff70, mutant_0023ff70, 0x1835_a033, false, build_23ff70, |f, c| {
    w10::sorted_key_index(f.m, c)
});

// ---- 00240020 named handle resolve ----
fn build_240020(rng: &mut Rng, pair: &mut Pair, filled: &mut bool) -> S9 {
    if !*filled {
        *filled = true;
        for entry in 0..150u32 {
            pair.put32(0x0075_ce68 + 8 * entry, rng.next() | 0x100);
            pair.put32(0x0075_ce6c + 8 * entry, 0);
        }
    }
    let table = owner_table(rng, pair);
    let any = rng.next();
    let wanted = pick(rng, &[0, 1, 2, 3, 0x100, any]);
    let entry = match rng.below(4) {
        0 => 149,
        1 => 0,
        _ => rng.below(150),
    };
    let cell = reg(rng, POOL_B, 0);
    let value = match rng.below(4) {
        0 => 0,
        1 => handle(rng, pair, table, false),
        _ => handle(rng, pair, table, true),
    };
    pair.put32(cell, value);
    if rng.below(8) != 0 {
        pair.put32(0x0075_ce68 + 8 * entry, wanted);
        pair.put32(0x0075_ce6c + 8 * entry, cell);
    }
    let mut s = { let args = vec![wanted]; s9(rng, 0x240020, args, false) };
    s.case.watch.push((cell, 4));
    s
}
s9_suite!(
    r_00240020,
    mutant_00240020,
    0x1835_a034,
    false,
    {
        let mut filled = false;
        move |r, p| build_240020(r, p, &mut filled)
    },
    |f, c| w10::named_handle_resolve(f.m, c)
);

// ---- 00242EC0 extension state 1 test ----
fn build_242ec0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let ext = reg(rng, POOL_B, 0);
    pair.put32(object + 0x7c, ext);
    pair.put32(ext + 0x10, pick(rng, &[1, 1, 0, 2, 0x8000_0001]));
    { let args = vec![object]; s9(rng, 0x242ec0, args, false) }
}
s9_suite!(r_00242ec0, mutant_00242ec0, 0x1835_a035, false, build_242ec0, |f, c| {
    w10::extension_state1_test(f.m, c)
});

// ---- 00247A70 class 10000000 controller field ----
fn build_247a70(rng: &mut Rng, pair: &mut Pair) -> S9 {
    let object = reg(rng, POOL_A, 0);
    let a = nul!(rng, reg(rng, POOL_B, 0), 6);
    let b = nul!(rng, reg(rng, POOL_B, 1), 6);
    let controller = nul!(rng, reg(rng, POOL_C, 0), 6);
    let out = nul!(rng, reg(rng, POOL_C, 1), 4);
    pair.put32(object + 0x20, pick(rng, &[0x1000_0000, 0x1000_0000, 0x1000_0001, 0]));
    pair.put32(object + 0x7c, a);
    pair.put32(a + 4, nul!(rng, rng.next() | 1, 6));
    pair.put32(a + 0x10, b);
    pair.put32(b + 0x150, controller);
    pair.put32(controller + 4, pick(rng, &[0, 7, 0xffff_ffff, 0x8000_0000]));
    pair.put32(out, rng.next() | 1);
    let mut s = { let args = vec![object, out]; s9(rng, 0x247a70, args, false) };
    s.case.watch.push((out, 4));
    s
}
s9_suite!(r_00247a70, mutant_00247a70, 0x1835_a036, false, build_247a70, |f, c| {
    w10::class_10000000_controller_field(f.m, c)
});

// ---- 002533E0 global 78b594 is 10 ----
fn build_2533e0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    pair.put32(0x0078_b594, pick(rng, &[10, 10, 9, 11, 0, 0xffff_fff6]));
    { let args = vec![]; s9(rng, 0x2533e0, args, false) }
}
s9_suite!(r_002533e0, mutant_002533e0, 0x1835_a037, false, build_2533e0, |f, c| {
    w10::global_78b594_is_10(f.m, c)
});

// ---- 002744C0 flag slot extension field 3c ----
fn build_2744c0(rng: &mut Rng, pair: &mut Pair) -> S9 {
    for entry in 0..2u32 {
        let holder = reg(rng, POOL_B, entry);
        let ext = reg(rng, POOL_C, entry);
        pair.put32(0x007a_2bec + 8 * entry, holder);
        pair.put32(holder + 0x7c, ext);
        pair.put32(ext + 0x3c, rng.next());
    }
    let selector = pick(rng, &[0, 1, 1, 2, 0xffff_ffff]);
    { let args = vec![selector]; s9(rng, 0x2744c0, args, false) }
}
s9_suite!(r_002744c0, mutant_002744c0, 0x1835_a038, false, build_2744c0, |f, c| {
    w10::flag_slot_extension_field_3c(f.m, c)
});
