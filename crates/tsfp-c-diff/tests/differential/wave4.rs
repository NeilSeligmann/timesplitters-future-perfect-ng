// SPDX-License-Identifier: GPL-3.0-or-later
//! T1848 wave 4: call-free proven C replacements of the audio, input, render and network trees,
//! each with a random differential (Rust port == unmodified C replacement, EAX/ECX/EDX, ESI, ESP
//! and every byte the builder or the root touches) and an output-perturbation mutant control.
//! See docs/t1848-rust-wave4.md. Builders bias words towards each root's branch values.
use super::*;
use tsfp_engine_audio::leaves as audio;
use tsfp_engine_input::leaves as input;
use tsfp_engine_render::leaves as render;
use tsfp_network::leaves as network;

const POOL_A: u32 = 0x0040_0000;
const POOL_B: u32 = 0x0050_0000;

fn mk(rng: &mut Rng, va: u32, args: Vec<u32>) -> Case {
    Case { va, args, regs: [rng.next(), rng.next(), rng.next()], esi: rng.next(), watch: vec![] }
}

macro_rules! entry {
    ($krate:ident, $case:expr) => {
        $krate::gprs::Gprs { eax: $case.regs[0], ecx: $case.regs[1], edx: $case.regs[2] }
    };
}
macro_rules! outcome {
    ($gprs:expr) => {{
        let g = $gprs;
        Outcome { eax: g.eax, ecx: Some(g.ecx), edx: Some(g.edx) }
    }};
}

/// Zero about a third of the time, else a word biased by `special`.
fn maybe(rng: &mut Rng, special: &[u32]) -> u32 {
    if rng.below(3) == 0 { 0 } else { rng.word(special) }
}

// ---- audio ----

const MASK_WORD: u32 = 0x004c_b54c;
suite!(
    r_00045d60,
    mutant_00045d60,
    0x1848_0001,
    |r, p| {
        p.put32(MASK_WORD, r.next());
        let bit = if r.below(4) == 0 { r.next() } else { r.below(34) };
        mk(r, 0x45d60, vec![bit])
    },
    |v, c| outcome!(audio::global_mask_bit_test(&*v, c.args[0], entry!(tsfp_engine_audio, c)))
);

suite!(
    r_00063bf0,
    mutant_00063bf0,
    0x1848_0002,
    |r, p| {
        let ptr = r.pool(POOL_A);
        let first = r.word(&[0, 1, u32::MAX]);
        p.put32(ptr, first);
        p.put32(ptr.wrapping_add(4), if r.below(2) == 0 { first } else { r.word(&[0, 1]) });
        mk(r, 0x63bf0, vec![ptr])
    },
    |v, c| outcome!(audio::pair_cursor_equal(&*v, c.args[0], entry!(tsfp_engine_audio, c)))
);

suite!(
    r_000d76a0,
    mutant_000d76a0,
    0x1848_0003,
    |r, p| {
        let index = match r.below(5) {
            0..=1 => 0x24,
            2 => r.below(0x30),
            3 => r.next(),
            _ => r.word(&[0x24, 0x25, 0x23]),
        };
        let slot = index.wrapping_mul(48).wrapping_add(0x0053_a3cc);
        p.put32(slot, maybe(r, &[1]));
        let flag = maybe(r, &[1, 0x100]);
        mk(r, 0xd76a0, vec![index, flag])
    },
    |v, c| outcome!(audio::table_special_fallback(&*v, c.args[0], c.args[1], entry!(tsfp_engine_audio, c)))
);

fn build_clear_key(rng: &mut Rng, pair: &mut Pair) -> Case {
    let key = rng.word(&[0, 1, 0x1234]);
    let hit_a = rng.below(40);
    let hit_b = rng.below(40);
    let mode = rng.below(4);
    for index in 0..40u32 {
        let noise = |rng: &mut Rng| {
            let value = rng.next() | 2;
            if value == key { value ^ 1 } else { value }
        };
        let a = if mode & 1 == 0 && index == hit_a || (rng.below(60) == 0) { key } else { noise(rng) };
        let b = if mode & 2 == 0 && index == hit_b || (rng.below(60) == 0) { key } else { noise(rng) };
        pair.put32(0x0058_35f0 + 4 * index, a);
        pair.put32(0x0058_18e8 + 4 * index, b);
        pair.put32(0x0056_5a00 + 4 * index, rng.next() | 1);
        pair.put32(0x0056_5aa0 + 4 * index, rng.next() | 1);
    }
    let mut case = mk(rng, 0x26b70, vec![]);
    case.regs[1] = key;
    case
}
suite!(
    r_00026b70,
    mutant_00026b70,
    0x1848_0004,
    build_clear_key,
    |v, c| outcome!(audio::clear_matching_key(v, entry!(tsfp_engine_audio, c)))
);

// ---- input ----

const PAD_LOCKED: u32 = 0x006b_9888;
const ALT_MAP_FLAG: u32 = 0x0076_b0fc;

fn build_pad(rng: &mut Rng, pair: &mut Pair, va: u32) -> Case {
    pair.put32(PAD_LOCKED, maybe(rng, &[1]));
    pair.put32(ALT_MAP_FLAG, maybe(rng, &[1]));
    let pad = if rng.below(8) == 0 { rng.next() } else { rng.below(6) };
    pair.put32(pad.wrapping_mul(4).wrapping_add(0x006b_93c0), rng.below(7));
    for pad_index in 0..6u32 {
        pair.put32(pad_index * 4 + 0x006b_93c0, rng.below(7));
    }
    for slot in 0..8u32 {
        pair.put32(slot * 0x3c + 0x006b_93e0, rng.next());
        pair.put32(slot * 0x3c + 0x006b_94d8, rng.next());
    }
    mk(rng, va, vec![pad])
}
suite!(
    r_000427b0,
    mutant_000427b0,
    0x1848_0011,
    |r, p| build_pad(r, p, 0x427b0),
    |v, c| outcome!(input::pad_current_button_state(&*v, c.args[0], entry!(tsfp_engine_input, c)))
);
suite!(
    r_00042c30,
    mutant_00042c30,
    0x1848_0012,
    |r, p| build_pad(r, p, 0x42c30),
    |v, c| outcome!(input::pad_get_field_0x10_direct(&*v, c.args[0], entry!(tsfp_engine_input, c)))
);
suite!(
    r_000431c0,
    mutant_000431c0,
    0x1848_0013,
    |r, p| {
        let case = build_pad(r, p, 0x431c0);
        p.put32(0x007e_7240, r.next());
        for pad_index in 0..6u32 {
            let mapped = if r.below(3) == 0 { r.next() } else { r.below(34) };
            p.put32(pad_index * 4 + 0x006b_93c0, mapped);
        }
        case
    },
    |v, c| outcome!(input::pad_map_is_bit_set(&*v, c.args[0]))
);
suite!(
    r_00246d10,
    mutant_00246d10,
    0x1848_0014,
    |r, p| {
        p.put32(0x0075_eaf8, maybe(r, &[1]));
        mk(r, 0x246d10, vec![])
    },
    |v, c| outcome!(input::driven_object_is_null(&*v, entry!(tsfp_engine_input, c)))
);
suite!(
    r_00190d60,
    mutant_00190d60,
    0x1848_0015,
    |r, p| {
        // Small bounds only: the original loop has no cap, a random bound would run for hours.
        let bound = [u32::MAX, 0, 1, 2, 3, 4, 5, 8, 16, 33, 40, 0x8000_0000][r.below(12) as usize];
        p.put32(0x0052_36c4, bound);
        for index in 0..42u32 {
            let word = if r.below(2) == 0 { r.below(8) } else { r.next() };
            p.put32(0x0074_e914 + 4 * index, word);
        }
        mk(r, 0x190d60, vec![])
    },
    |v, c| outcome!(input::count_local_players_with_pad_mapping(&*v, entry!(tsfp_engine_input, c)))
);

fn record_address(pair: &Pair, pad: u32) -> u32 {
    let read = |a: u32| u32::from_le_bytes(std::array::from_fn(|i| pair.get(false, a + i as u32)));
    if read(ALT_MAP_FLAG) != 0 {
        pad * 0x3c + 0x006b_94c8
    } else {
        read(pad * 4 + 0x006b_93c0) * 0x3c + 0x006b_93d0
    }
}

fn build_record(rng: &mut Rng, pair: &mut Pair, va: u32, import: bool) -> Case {
    pair.put32(ALT_MAP_FLAG, maybe(rng, &[1]));
    let pad = rng.below(4);
    for pad_index in 0..4u32 {
        pair.put32(pad_index * 4 + 0x006b_93c0, rng.below(4));
    }
    let record = record_address(pair, pad);
    for word in 0..30u32 {
        pair.put32(record + 4 * word, rng.next());
    }
    // 2 means no tail, larger counts walk the tail. 0 and 1 wrap to 4 billion iterations.
    let count = if rng.below(5) == 0 { 2 } else { 2 + rng.below(19) };
    let area = match rng.below(6) {
        0 => record.wrapping_add(rng.below(0x60)),
        _ => rng.pool(POOL_B),
    };
    if import {
        for byte in 0..24u32 {
            pair.put32(area + 4 * byte, rng.next());
        }
    }
    let mut case = mk(rng, va, vec![pad, area, count]);
    case.watch.push((area, 4 * 24));
    case.watch.push((record, 0x80));
    case
}
suite!(
    r_00043a90,
    mutant_00043a90,
    0x1848_0016,
    |r, p| build_record(r, p, 0x43a90, false),
    |v, c| outcome!(input::export_record(v, c.args[0], c.args[1], c.args[2], entry!(tsfp_engine_input, c)))
);
suite!(
    r_00043af0,
    mutant_00043af0,
    0x1848_0017,
    |r, p| build_record(r, p, 0x43af0, true),
    |v, c| outcome!(input::import_record(v, c.args[0], c.args[1], c.args[2]))
);

// ---- render ----

suite!(
    r_003443f0,
    mutant_003443f0,
    0x1848_0021,
    |r, p| {
        p.put32(0x0076_5e58, maybe(r, &[1, 0x100]));
        mk(r, 0x3443f0, vec![])
    },
    |v, c| outcome!(render::is_weather_pass_flag_set(&*v, entry!(tsfp_engine_render, c)))
);
suite!(
    r_0001e8d0,
    mutant_0001e8d0,
    0x1848_0022,
    |r, p| {
        p.put32(0x007b_0000, 0);
        let flags = match r.below(4) {
            0 => 1 << r.below(32),
            1 => r.next(),
            2 => r.next() & r.next(),
            _ => r.next() & 0x3_ffff,
        };
        mk(r, 0x1e8d0, vec![flags])
    },
    |_v, c| outcome!(render::vertex_format_size(c.args[0], entry!(tsfp_engine_render, c)))
);
suite!(
    r_0008e3e0,
    mutant_0008e3e0,
    0x1848_0023,
    |r, p| {
        let table = POOL_A + r.below(8) * 0x1000 + r.below(0x40) * 16;
        let records = r.below(9);
        for index in 0..=records {
            let address = table + 16 * index;
            let first = if index == records {
                u32::MAX
            } else {
                match r.below(5) {
                    0 => 0,
                    1 => 0x4000_0000,
                    _ => r.next() & 0x7fff_ffff,
                }
            };
            p.put32(address, first);
            p.put32(address + 4, r.next());
            p.put32(address + 8, r.next());
            let flag = if r.below(2) == 0 { 0x1000_0000 } else { 0 };
            p.put32(address + 12, (r.next() & !0x1000_0000) | flag);
        }
        let delta = r.word(&[0x1000, u32::MAX, 0x8000_0000]);
        mk(r, 0x8e3e0, vec![table, delta])
    },
    |v, c| outcome!(render::relocate_flagged_records(v, c.args[0], c.args[1], entry!(tsfp_engine_render, c)))
);

// ---- network ----

suite!(
    r_000387d0,
    mutant_000387d0,
    0x1848_0031,
    |r, p| {
        p.put32(0x0068_ddac, maybe(r, &[1]));
        p.put32(0x0068_ddb0, maybe(r, &[1]));
        mk(r, 0x387d0, vec![])
    },
    |v, c| outcome!(network::state_000387d0(&*v, entry!(tsfp_network, c)))
);

fn build_selector(rng: &mut Rng, pair: &mut Pair, va: u32) -> Case {
    let this = rng.pool(POOL_A);
    pair.put32(this + 0x30, rng.word(&[2, 3, 4, 5, 6, 0x8000_0000, u32::MAX, (-1i32) as u32]));
    pair.put32(this + 0x34, rng.word(&[0, 1, 0x100, 0x101, 0x1_0000]));
    let mut case = mk(rng, va, vec![]);
    case.regs[1] = this;
    case
}
macro_rules! selector {
    ($name:ident, $mutant:ident, $va:expr, $seed:expr, $offset:expr, $min:expr) => {
        suite!(
            $name,
            $mutant,
            $seed,
            |r, p| build_selector(r, p, $va),
            |v, c| outcome!(network::flag_and_level_at_least(&*v, c.regs[1], $offset, $min, entry!(tsfp_network, c)))
        );
    };
}
selector!(r_003b20f0, mutant_003b20f0, 0x3b20f0, 0x1848_0032, 0x34, 3);
selector!(r_003b2110, mutant_003b2110, 0x3b2110, 0x1848_0033, 0x34, 5);
selector!(r_003b2130, mutant_003b2130, 0x3b2130, 0x1848_0034, 0x35, 3);
selector!(r_003b2150, mutant_003b2150, 0x3b2150, 0x1848_0035, 0x35, 5);

suite!(
    r_0035e590,
    mutant_0035e590,
    0x1848_0036,
    |r, p| {
        let specials = [0, 1, 2, 16, u32::MAX, 0x8000_0000, 0x7fff_ffff];
        p.put32(0x0076_bbbc, r.word(&specials));
        p.put32(0x0076_bbc4, r.word(&specials));
        let limit = r.word(&specials);
        let flag = maybe(r, &[1]);
        mk(r, 0x35e590, vec![flag, limit])
    },
    |v, c| outcome!(network::is_slot_limit_reached(&*v, c.args[0], c.args[1], entry!(tsfp_network, c)))
);
suite!(
    r_00356290,
    mutant_00356290,
    0x1848_0037,
    |r, p| {
        for record in 0..16u32 {
            let word = if r.below(4) == 0 { r.next() & !1 } else { r.next() };
            p.put32(0x0077_728c + 0x124 * record, word);
        }
        mk(r, 0x356290, vec![])
    },
    |v, c| outcome!(network::count_free_record_slots(&*v, entry!(tsfp_network, c)))
);
suite!(
    r_00356020,
    mutant_00356020,
    0x1848_0038,
    |r, p| {
        let index = match r.below(6) {
            0 => u32::MAX,
            1 => 16 + r.below(3),
            2 => r.next(),
            _ => r.below(16),
        };
        let slot = index.min(15) % 16;
        for offset in [0u32, 4, 8, 0x20, 0x34, 0x30c - 4] {
            p.put32(0x0077_4180 + 0x30c * slot + offset, r.next());
        }
        let (word_a, word_b) = (r.next(), r.next());
        mk(r, 0x356020, vec![index, word_a, word_b])
    },
    |v, c| outcome!(network::player_slot_claim(v, c.args[0], c.args[1], c.args[2], entry!(tsfp_network, c)))
);

suite!(
    r_003bc270,
    mutant_003bc270,
    0x1848_0039,
    |r, p| {
        let context = if r.below(6) == 0 { 0x0040_0000 - 6 + r.below(12) } else { r.pool(POOL_B) };
        for word in 0..6u32 {
            p.put32(context + 4 * word, r.next());
        }
        let mut case = mk(r, 0x3bc270, vec![context]);
        case.watch.push((context, 20));
        case
    },
    |v, c| outcome!(network::md5_context_init(v, c.args[0], entry!(tsfp_network, c)))
);
