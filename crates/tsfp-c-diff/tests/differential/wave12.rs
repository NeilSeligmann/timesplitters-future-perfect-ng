// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 12: 18 call-free proven C replacements (ai, gameplay, ui) ported as readable Rust,
//! each differential-tested (Rust == unmodified C replacement) with an output-perturbation
//! mutant control. See docs/t1836-rust-wave12.md. Builders seed the guest tables the root reads
//! with biased words, `drive` compares every address they filled plus the declared watches.
use super::*;
use tsfp_ai::table_leaves;
use tsfp_gameplay::slot_leaves;
use tsfp_ui::flag_leaves;

const POOL_A: u32 = 0x0040_0000;
const POOL_B: u32 = 0x0050_0000;
const POOL_C: u32 = 0x0060_0000;

macro_rules! entry {
    ($krate:ident, $case:expr) => {
        $krate::gprs::Gprs {
            eax: $case.regs[0],
            ecx: $case.regs[1],
            edx: $case.regs[2],
        }
    };
}
macro_rules! outcome {
    ($gprs:expr) => {{
        let g = $gprs;
        Outcome {
            eax: g.eax,
            ecx: Some(g.ecx),
            edx: Some(g.edx),
        }
    }};
}

fn mk(rng: &mut Rng, va: u32, args: Vec<u32>, watch: Vec<(u32, u32)>) -> Case {
    Case {
        va,
        args,
        regs: [rng.next(), rng.next(), rng.next()],
        esi: rng.next(),
        watch,
    }
}

// ---- side table lookups (274330 274350 274380 2744F0) ----

/// Five side-table entries (0x7A2BE4 + 8k) pointing at side objects with random flags and
/// extension objects.
fn fill_sides(rng: &mut Rng, pair: &mut Pair) {
    for slot in 0..5u32 {
        let object = rng.pool(POOL_A);
        pair.put32(0x007a_2be4 + 8 * slot, object);
        pair.put32(object + 0x28, rng.next());
        let extension = rng.pool(POOL_B);
        pair.put32(object + 0x7c, extension);
        pair.put32(extension + 0xa0, rng.next());
    }
}

fn selector(rng: &mut Rng) -> u32 {
    [0u32, 1, 2, 3, 4, rng.next()][rng.below(6) as usize]
}

fn build_side(rng: &mut Rng, pair: &mut Pair, va: u32) -> Case {
    fill_sides(rng, pair);
    let argument = selector(rng);
    mk(rng, va, vec![argument], vec![])
}

macro_rules! side_suite {
    ($name:ident, $mutant:ident, $seed:expr, $va:expr, $port:path) => {
        suite!(
            $name,
            $mutant,
            $seed,
            |r, p| build_side(r, p, $va),
            |v, c| outcome!($port(v, STACK, entry!(tsfp_gameplay, c)))
        );
    };
}
side_suite!(r_00274330, mutant_00274330, 0x1836_1201, 0x274330, slot_leaves::side_extension_flag4_by_index);
side_suite!(r_00274350, mutant_00274350, 0x1836_1202, 0x274350, slot_leaves::side_extension_flag4_by_flag);
side_suite!(r_00274380, mutant_00274380, 0x1836_1203, 0x274380, slot_leaves::side_bit7_clear);
side_suite!(r_002744f0, mutant_002744f0, 0x1836_1204, 0x2744f0, slot_leaves::side_object_from_flag);

// ---- 00168160 entry field store ----

fn build_entry_field(rng: &mut Rng, pair: &mut Pair) -> Case {
    let table = rng.pool(POOL_A);
    pair.put32(0x0074_c458, table);
    let id = [rng.below(4), rng.below(0x10000), rng.next(), 0xffff][rng.below(4) as usize];
    let value = rng.next();
    let target = ((id & 0xffff) << 9).wrapping_add(table).wrapping_add(0x1c4);
    mk(rng, 0x168160, vec![id, value], vec![(target, 4)])
}
suite!(
    r_00168160,
    mutant_00168160,
    0x1836_1205,
    build_entry_field,
    |v, c| outcome!(slot_leaves::entry_set_field_1c4(
        v,
        STACK,
        entry!(tsfp_gameplay, c)
    ))
);

// ---- 0010E9A0 record position copy ----

fn build_set_position(rng: &mut Rng, pair: &mut Pair) -> Case {
    let record = if rng.below(6) == 0 { 0 } else { rng.pool(POOL_A) };
    let source = match rng.below(5) {
        0 => record.wrapping_add(0x94), // the copy overlaps its own destination
        1 => record.wrapping_add(0x9c),
        _ => rng.pool(POOL_B),
    };
    for word in 0..3 {
        pair.put32(source.wrapping_add(4 * word), rng.next());
    }
    pair.put32(record.wrapping_add(4), rng.next());
    mk(
        rng,
        0x10e9a0,
        vec![record, source],
        vec![(record.wrapping_add(0x98), 12), (record.wrapping_add(4), 4)],
    )
}
suite!(
    r_0010e9a0,
    mutant_0010e9a0,
    0x1836_1206,
    build_set_position,
    |v, c| outcome!(slot_leaves::record_set_position(
        v,
        STACK,
        entry!(tsfp_gameplay, c)
    ))
);

// ---- 00012F30 bounded array read ----

fn build_bounded(rng: &mut Rng, pair: &mut Pair) -> Case {
    let object = rng.pool(POOL_A);
    let array = rng.pool(POOL_B);
    let count = [rng.below(8), 0, u32::MAX, 0x8000_0000, rng.next()][rng.below(5) as usize];
    pair.put32(object + 0x1e8, count);
    pair.put32(object + 0x1fc, array);
    pair.put32(0x007a_d288, rng.next());
    for slot in 0..8 {
        pair.put32(array + 4 * slot, rng.next());
    }
    let index = [rng.below(8), u32::MAX, 0x8000_0000, rng.next()][rng.below(4) as usize];
    let mut case = mk(rng, 0x12f30, vec![object, index], vec![]);
    case.watch.push((object, 4));
    case
}
suite!(
    r_00012f30,
    mutant_00012f30,
    0x1836_1207,
    build_bounded,
    |v, c| outcome!(slot_leaves::bounded_array_read(
        &*v,
        STACK,
        entry!(tsfp_gameplay, c)
    ))
);

// ---- 00012F50 indexed record field ----

fn build_record_field(rng: &mut Rng, pair: &mut Pair) -> Case {
    let base = rng.pool(POOL_A);
    pair.put32(0x004f_9bac, base);
    let index = [rng.below(6), u32::MAX, rng.next()][rng.below(3) as usize];
    for slot in 0..6u32 {
        pair.put32(base + slot * 0x29c + 0xc, rng.next());
    }
    mk(rng, 0x12f50, vec![index], vec![(0x004f_9bac, 4)])
}
suite!(
    r_00012f50,
    mutant_00012f50,
    0x1836_1208,
    build_record_field,
    |v, c| outcome!(slot_leaves::indexed_record_field_c(
        &*v,
        STACK,
        entry!(tsfp_gameplay, c)
    ))
);

// ---- 00150200 three global outputs ----

fn build_three_outputs(rng: &mut Rng, pair: &mut Pair) -> Case {
    let globals = [0x0074_9160u32, 0x0074_9158, 0x0074_9148];
    for global in globals {
        pair.put32(global, rng.next());
    }
    // Outputs are mostly distinct pool slots, sometimes the source globals themselves so a
    // store changes a later read.
    let pick = |rng: &mut Rng| match rng.below(4) {
        0 => globals[rng.below(3) as usize],
        _ => rng.pool(POOL_C),
    };
    let outputs = [pick(rng), pick(rng), pick(rng)];
    let watch = outputs.iter().map(|&address| (address, 4)).collect();
    mk(rng, 0x150200, outputs.to_vec(), watch)
}
suite!(
    r_00150200,
    mutant_00150200,
    0x1836_1209,
    build_three_outputs,
    |v, c| outcome!(slot_leaves::three_global_outputs(
        v,
        STACK,
        entry!(tsfp_gameplay, c)
    ))
);

// ---- global stores ----

fn build_stores(rng: &mut Rng, pair: &mut Pair, va: u32, args: u32, watch: &[(u32, u32)]) -> Case {
    for &(start, len) in watch {
        for word in 0..len / 4 {
            pair.put32(start + 4 * word, rng.next());
        }
    }
    let arguments = (0..args).map(|_| rng.next()).collect();
    mk(rng, va, arguments, watch.to_vec())
}

suite!(
    r_00227be0,
    mutant_00227be0,
    0x1836_120a,
    |r, p| build_stores(r, p, 0x227be0, 2, &[(0x75c148, 4), (0x75c150, 4)]),
    |v, c| outcome!(slot_leaves::set_global_pair_75c148(
        v,
        STACK,
        entry!(tsfp_gameplay, c)
    ))
);
suite!(
    r_0023ef70,
    mutant_0023ef70,
    0x1836_120b,
    |r, p| build_stores(r, p, 0x23ef70, 1, &[(0x78cd54, 4)]),
    |v, c| outcome!(slot_leaves::set_global_78cd54(
        v,
        STACK,
        entry!(tsfp_gameplay, c)
    ))
);
suite!(
    r_0025b770,
    mutant_0025b770,
    0x1836_120c,
    |r, p| build_stores(r, p, 0x25b770, 5, &[(0x78b540, 20)]),
    |v, c| eax_only(slot_leaves::store_five_args_78b540(v, STACK, entry!(tsfp_gameplay, c)).eax)
);
suite!(
    r_002586a0,
    mutant_002586a0,
    0x1836_120d,
    |r, p| build_stores(r, p, 0x2586a0, 0, &[(0x75f5cc, 4), (0x5201e8, 8)]),
    |v, c| eax_only(slot_leaves::clear_three_globals(v, entry!(tsfp_gameplay, c)).eax)
);
suite!(
    r_001553e0,
    mutant_001553e0,
    0x1836_120e,
    |r, p| build_stores(r, p, 0x1553e0, 4, &[(0x4e7958, 16)]),
    |v, c| outcome!(flag_leaves::set_settings_quad(
        v,
        STACK,
        entry!(tsfp_ui, c)
    ))
);

// ---- 002AEFB0 behaviour table (ai) ----

fn build_behaviour(rng: &mut Rng, pair: &mut Pair) -> Case {
    let group = [rng.below(8), rng.next()][rng.below(2) as usize];
    let slot = [rng.below(8), rng.next()][rng.below(2) as usize];
    let address = group
        .wrapping_add(slot.wrapping_mul(4))
        .wrapping_mul(4)
        .wrapping_add(0x0052_0a48);
    pair.put32(address, rng.next());
    mk(rng, 0x2aefb0, vec![group, slot], vec![(address, 4)])
}
suite!(
    r_002aefb0,
    mutant_002aefb0,
    0x1836_120f,
    build_behaviour,
    |v, c| eax_only(table_leaves::behaviour_table_entry(&*v, STACK, entry!(tsfp_ai, c)).eax)
);

// ---- ui predicates ----

fn build_value(rng: &mut Rng, va: u32) -> Case {
    let value = [2u32, 3, rng.below(6), rng.next()][rng.below(4) as usize];
    mk(rng, va, vec![value], vec![(STACK + 4, 4)])
}
suite!(
    r_00311a80,
    mutant_00311a80,
    0x1836_1210,
    |r, _| build_value(r, 0x311a80),
    |v, c| outcome!(flag_leaves::is_two(&*v, STACK, entry!(tsfp_ui, c)))
);
suite!(
    r_00311a90,
    mutant_00311a90,
    0x1836_1211,
    |r, _| build_value(r, 0x311a90),
    |v, c| outcome!(flag_leaves::is_three(&*v, STACK, entry!(tsfp_ui, c)))
);

// ---- 0022D790 any entry flag ----

fn build_entry_list(rng: &mut Rng, pair: &mut Pair) -> Case {
    let list = rng.pool(POOL_A);
    let entries = rng.pool(POOL_B);
    let count = [0u32, rng.below(7), rng.below(7), u32::MAX, 0x8000_0000][rng.below(5) as usize];
    pair.put32(list + 0x20, count);
    pair.put32(list + 0x1c, entries);
    let hit = rng.below(3) == 0;
    for entry in 0..8u32 {
        let flags = (rng.next() & 0xfd) as u8 | if hit && rng.below(3) == 0 { 2 } else { 0 };
        pair.put8(entries + entry * 0x14 + 1, flags);
    }
    mk(rng, 0x22d790, vec![list], vec![(list, 4)])
}
suite!(
    r_0022d790,
    mutant_0022d790,
    0x1836_1212,
    build_entry_list,
    |v, c| outcome!(flag_leaves::any_entry_flag_set(
        &*v,
        STACK,
        entry!(tsfp_ui, c)
    ))
);

// ---- wave 14 call-free additions: 0009E750 00073E10 00349710 00147200 ----

fn build_any_index(rng: &mut Rng, pair: &mut Pair) -> Case {
    let object = rng.pool(POOL_A);
    let table = rng.pool(POOL_B);
    pair.put32(0x007b_977c, table);
    pair.put32(object + 0x30, [rng.below(6), 0x80, 0xff, rng.next()][rng.below(4) as usize]);
    for slot in 0..6u32 {
        let selected = rng.below(16);
        pair.put32(object + 0x38 + 4 * slot, selected);
    }
    for byte in 0..16u32 {
        pair.put8(table + byte, if rng.below(5) == 0 { rng.next() as u8 } else { 0 });
    }
    mk(rng, 0x9e750, vec![object], vec![(object, 4)])
}
suite!(
    r_0009e750,
    mutant_0009e750,
    0x1836_1213,
    build_any_index,
    |v, c| eax_only(slot_leaves::any_index_flagged(&*v, STACK, entry!(tsfp_gameplay, c)).eax)
);

fn build_slot_ready(rng: &mut Rng, pair: &mut Pair) -> Case {
    let object = rng.pool(POOL_A);
    let slot = rng.below(4);
    pair.put32(object + (slot << 4) + 0x128, [u32::MAX, rng.below(4), rng.next()][rng.below(3) as usize]);
    pair.put32(object + ((slot + 0x13) << 4), [2u32, rng.below(4), rng.next()][rng.below(3) as usize]);
    mk(rng, 0x73e10, vec![object, slot], vec![(object, 4)])
}
suite!(
    r_00073e10,
    mutant_00073e10,
    0x1836_1214,
    build_slot_ready,
    |v, c| eax_only(flag_leaves::slot_ready(&*v, STACK, entry!(tsfp_ui, c)).eax)
);

fn build_record_68(rng: &mut Rng, pair: &mut Pair) -> Case {
    let index = [rng.below(4), rng.below(0x1000), rng.next()][rng.below(3) as usize];
    let record = index.wrapping_mul(0x68).wrapping_add(0x0077_8800);
    pair.put32(record, rng.next());
    pair.put32(record.wrapping_add(0x40), rng.next());
    mk(rng, 0x349710, vec![index], vec![(record, 4), (record.wrapping_add(0x40), 4)])
}
suite!(
    r_00349710,
    mutant_00349710,
    0x1836_1215,
    build_record_68,
    |v, c| eax_only(flag_leaves::record_68_clear_bit1_set_state(v, STACK, entry!(tsfp_ui, c)).eax)
);

fn build_notice(rng: &mut Rng, pair: &mut Pair) -> Case {
    let list = rng.pool(POOL_A);
    // The count is mostly small, sometimes points the record at the count word itself.
    let count = [rng.below(4), 0, (0xe0 / 0x1c) as u32, rng.below(40)][rng.below(4) as usize];
    pair.put32(list + 0xe0, count);
    let args = vec![list, rng.next(), rng.next(), rng.next()];
    let record = count.wrapping_mul(0x1c).wrapping_add(list);
    mk(rng, 0x147200, args, vec![(record, 0x1c), (list + 0xe0, 4)])
}
suite!(
    r_00147200,
    mutant_00147200,
    0x1836_1216,
    build_notice,
    |v, c| outcome!(flag_leaves::append_notice_record(v, STACK, entry!(tsfp_ui, c)))
);

// ---- wave 15 call-free additions (Tier 1): 00070C90 0007C1B0 0007C4D0 ----

fn build_find_index(rng: &mut Rng, pair: &mut Pair) -> Case {
    let list = if rng.below(8) == 0 { 0 } else { rng.pool(POOL_A) };
    let count = [rng.below(6), 0, u32::MAX][rng.below(3) as usize];
    let needle = rng.below(4);
    if list != 0 {
        pair.put32(list, count);
        for slot in 0..6 {
            pair.put32(list + 4 + 4 * slot, rng.below(5));
        }
    }
    mk(rng, 0x70c90, vec![list, needle], vec![(STACK + 4, 8)])
}
suite!(
    r_00070c90,
    mutant_00070c90,
    0x1836_1217,
    build_find_index,
    |v, c| outcome!(flag_leaves::int_array_find_index(&*v, STACK, entry!(tsfp_ui, c)))
);

fn build_spaces(rng: &mut Rng, pair: &mut Pair) -> Case {
    let text = rng.pool(POOL_A);
    for i in 0..6 {
        pair.put8(text + i, [b' ', b' ', 0, b'a'][rng.below(4) as usize]);
    }
    let mut case = mk(rng, 0x7c4d0, vec![], vec![(text, 6)]);
    case.regs[1] = text;
    case
}
suite!(
    r_0007c4d0,
    mutant_0007c4d0,
    0x1836_1218,
    build_spaces,
    |v, c| eax_only(flag_leaves::text_is_empty_or_spaces(&*v, c.regs[1]))
);

fn build_charset(rng: &mut Rng, pair: &mut Pair) -> Case {
    let out = if rng.below(4) == 0 { 0 } else { rng.pool(POOL_A) };
    pair.put32(out.wrapping_add(0), rng.next());
    let flags = rng.next() & [0x108e, 0xffff, 0x1086, 0x9e][rng.below(4) as usize] | 0x4_0000;
    let mut case = mk(rng, 0x7c1b0, vec![], vec![(out, 4)]);
    case.regs[1] = out;
    case.regs[2] = flags;
    case
}
suite!(
    r_0007c1b0,
    mutant_0007c1b0,
    0x1836_1219,
    build_charset,
    |v, c| eax_only(flag_leaves::font_charset_select(v, c.regs[1], c.regs[2]))
);
