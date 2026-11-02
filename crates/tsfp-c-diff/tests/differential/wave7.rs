// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 7: further call-free proven C replacements, each with its own random differential
//! (Rust port == unmodified C replacement) and an output-perturbation mutant control. See
//! docs/t1836-rust-wave7.md. Builders fill the guest globals and pointer chains the root reads
//! with words biased towards its branch values, and `drive` compares everything they filled.
use super::*;
use tsfp_engine_object::object_leaves;
use tsfp_gameplay::mode_leaves;
use tsfp_ui::hud_leaves;

const POOL_A: u32 = 0x0040_0000;
const POOL_B: u32 = 0x0050_0000;
const POOL_C: u32 = 0x0060_0000;

fn mk(rng: &mut Rng, va: u32, args: Vec<u32>) -> Case {
    Case {
        va,
        args,
        regs: [rng.next(), rng.next(), rng.next()],
        esi: rng.next(),
        watch: vec![],
    }
}

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

const MODE: u32 = 0x0079_094c;

// ---- batch 1: gameplay (mode, state, weapon) and ui (hud, campaign) ----

fn build_mode(rng: &mut Rng, pair: &mut Pair, va: u32, specials: &[u32]) -> Case {
    pair.put32(MODE, rng.word(specials));
    pair.put32(0x0075_ccb8, rng.next());
    mk(rng, va, vec![])
}

suite!(
    r_00240580,
    mutant_00240580,
    0x1836_0101,
    |r, p| build_mode(r, p, 0x240580, &[24, 4, 0x65, 23, 25]),
    |v, c| outcome!(mode_leaves::mode_is_0x18(&*v, entry!(tsfp_gameplay, c)))
);

suite!(
    r_00233f90,
    mutant_00233f90,
    0x1836_0102,
    |r, p| build_mode(r, p, 0x233f90, &[0x65, 0x64, 0x66, 4]),
    |v, c| outcome!(mode_leaves::global_75ccb8_unless_mode_0x65(
        &*v,
        entry!(tsfp_gameplay, c)
    ))
);

suite!(
    r_00243060,
    mutant_00243060,
    0x1836_0103,
    |r, p| build_mode(r, p, 0x243060, &[4, 3, 5, 0x65]),
    |v, c| outcome!(mode_leaves::mode_select_minus1_or_3066(
        &*v,
        entry!(tsfp_gameplay, c)
    ))
);

fn build_descriptor(rng: &mut Rng, pair: &mut Pair) -> Case {
    let mode = rng.word(&[0x14, 0x15, 0x16, 0x8000_0000, u32::MAX, 0x7fff_ffff]);
    pair.put32(0x007d_e470, mode);
    for table in [0x0051_0788u32, 0x0051_3d04] {
        pair.put32(mode.wrapping_mul(4).wrapping_add(table), rng.next());
    }
    mk(rng, 0x230510, vec![])
}
suite!(
    r_00230510,
    mutant_00230510,
    0x1836_0104,
    build_descriptor,
    |v, c| outcome!(mode_leaves::state_descriptor_for_current_state(
        &*v,
        entry!(tsfp_gameplay, c)
    ))
);

fn build_set_mode_three(rng: &mut Rng, pair: &mut Pair) -> Case {
    let object = rng.pool(POOL_A);
    let child = match rng.below(8) {
        0 => object, // the child is the object itself
        1 => rng.next(),
        _ => rng.pool(POOL_B),
    };
    pair.put32(object.wrapping_add(0x7c), child);
    pair.put32(
        child.wrapping_add(0x20),
        rng.word(&[0, 1, 2, 3, 4, 5, u32::MAX]),
    );
    pair.put32(child.wrapping_add(0x9d0), rng.next());
    mk(rng, 0x264740, vec![object])
}
suite!(
    r_00264740,
    mutant_00264740,
    0x1836_0105,
    build_set_mode_three,
    |v, c| outcome!(mode_leaves::set_mode_three_unless_done(
        v,
        STACK,
        entry!(tsfp_gameplay, c)
    ))
);

fn build_mode_bytes(rng: &mut Rng, pair: &mut Pair) -> Case {
    let first = [0u32, 1, 0xff, rng.below(256)][rng.below(4) as usize];
    let second = [2u32, 10, 0, rng.below(256)][rng.below(4) as usize];
    pair.put32(0x007d_e400, (rng.next() & 0xff00_ffff) | (first << 16));
    pair.put32(0x007d_e454, (rng.next() & 0xffff_00ff) | (second << 8));
    mk(rng, 0x275620, vec![])
}
suite!(
    r_00275620,
    mutant_00275620,
    0x1836_0106,
    build_mode_bytes,
    |v, c| outcome!(mode_leaves::byte_7de402_nonzero_and_mode_byte_is_2(
        &*v,
        entry!(tsfp_gameplay, c)
    ))
);

fn build_weapon(rng: &mut Rng, pair: &mut Pair) -> Case {
    let entry = rng.pool(POOL_A);
    let index = if rng.below(6) == 0 {
        rng.next()
    } else {
        rng.below(8)
    };
    let table = if rng.below(6) == 0 {
        rng.next()
    } else {
        rng.pool(POOL_B)
    };
    pair.put32(entry.wrapping_add(0x30), index);
    pair.put32(0x004f_9bac, table);
    pair.put32(
        index
            .wrapping_mul(0x29c)
            .wrapping_add(table)
            .wrapping_add(0xc),
        rng.word(&[0x80, 0x7f, 0xff, 0x100]),
    );
    mk(rng, 0x1b58c0, vec![entry])
}
suite!(
    r_001b58c0,
    mutant_001b58c0,
    0x1836_0107,
    build_weapon,
    |v, c| outcome!(mode_leaves::weapon_entry_flag_80(
        &*v,
        STACK,
        entry!(tsfp_gameplay, c)
    ))
);

fn build_selected_record(rng: &mut Rng, pair: &mut Pair) -> Case {
    pair.put32(MODE, rng.word(&[0x65, 0x64]));
    pair.put32(0x0054_5d80, rng.next());
    let record = rng.pool(POOL_A);
    pair.put32(record.wrapping_add(0x38), rng.next());
    mk(rng, 0x35fd70, vec![record])
}
suite!(
    r_0035fd70,
    mutant_0035fd70,
    0x1836_0108,
    build_selected_record,
    |v, c| outcome!(mode_leaves::selected_record_index_or_field_0x38(
        &*v,
        STACK,
        entry!(tsfp_gameplay, c)
    ))
);

fn build_clear_block(rng: &mut Rng, pair: &mut Pair) -> Case {
    for address in [
        0x0076_1c58u32,
        0x0076_25f4,
        0x0076_1c7c,
        0x0076_1c68,
        0x0076_1c6c,
        0x0076_1c70,
        0x0076_1c74,
        0x0076_1bdc,
        0x0076_1be0,
        0x0076_1be4,
        0x0076_1be8,
    ] {
        pair.put32(address, rng.next());
    }
    mk(rng, 0x2ed7b0, vec![])
}
suite!(
    r_002ed7b0,
    mutant_002ed7b0,
    0x1836_0109,
    build_clear_block,
    |v, c| outcome!(mode_leaves::clear_state_block_761c58(
        v,
        entry!(tsfp_gameplay, c)
    ))
);

fn pointer_target(rng: &mut Rng, hazards: &[u32]) -> u32 {
    match rng.below(5) {
        0 => hazards[rng.below(hazards.len() as u32) as usize],
        1 => rng.next(),
        _ => rng.pool(POOL_C),
    }
}

fn build_campaign(rng: &mut Rng, pair: &mut Pair) -> Case {
    pair.put32(0x0068_9178, rng.word(&[u32::MAX, 2, 0, 1, 3]));
    pair.put32(0x0068_91ac, rng.next());
    pair.put32(0x0068_9160, rng.next());
    // Targets include the status word, the source words and the argument slots themselves.
    let hazards = [0x0068_9178, 0x0068_91ac, 0x0068_9160, STACK + 4, STACK + 8];
    let (a, b) = (pointer_target(rng, &hazards), pointer_target(rng, &hazards));
    pair.put32(a, rng.next());
    pair.put32(b, rng.next());
    mk(rng, 0x32f10, vec![a, b])
}
suite!(
    r_00032f10,
    mutant_00032f10,
    0x1836_010a,
    build_campaign,
    |v, c| outcome!(hud_leaves::campaign_status_write_result(
        v,
        STACK,
        entry!(tsfp_ui, c)
    ))
);

fn build_overlay(rng: &mut Rng, pair: &mut Pair) -> Case {
    let count = rng.word(&[0, 31, 32, 33, 0x8000_0000, u32::MAX, 0xffff_fff0]);
    let at = count.wrapping_mul(12).wrapping_add(0x0076_6f60);
    for offset in [0, 4, 8] {
        pair.put32(at.wrapping_add(offset), rng.next());
    }
    // Last, because the record for count 32 would overlay the count word itself.
    pair.put32(0x0076_70e0, count);
    let args = vec![rng.next(), rng.next(), rng.next()];
    mk(rng, 0x346250, args)
}
suite!(
    r_00346250,
    mutant_00346250,
    0x1836_010b,
    build_overlay,
    |v, c| outcome!(hud_leaves::hud_queue_overlay_request(
        v,
        STACK,
        entry!(tsfp_ui, c)
    ))
);

fn build_two_params(rng: &mut Rng, pair: &mut Pair) -> Case {
    pair.put32(0x0076_76dc, rng.next());
    pair.put32(0x0076_76c8, rng.next());
    let args = vec![rng.next(), rng.next()];
    mk(rng, 0x34c0d0, args)
}
suite!(
    r_0034c0d0,
    mutant_0034c0d0,
    0x1836_010c,
    build_two_params,
    |v, c| outcome!(hud_leaves::hud_transition_set_two_params(
        v,
        STACK,
        entry!(tsfp_ui, c)
    ))
);

// ---- batch 2: object leaves ----

/// An object pointer: a pool address, null, a random word, or an address chosen so a store of
/// the root lands on the stack argument slots (`hazards` are stack addresses to alias).
fn object_pointer(rng: &mut Rng, hazards: &[u32]) -> u32 {
    match rng.below(10) {
        0 => 0,
        1 | 2 if !hazards.is_empty() => hazards[rng.below(hazards.len() as u32) as usize],
        1 => rng.next(),
        _ => rng.pool(POOL_A),
    }
}

fn build_flag_pair(rng: &mut Rng, pair: &mut Pair) -> Case {
    pair.put32(
        0x007d_e454,
        (rng.next() & 0xffff_00ff) | (rng.word(&[10, 9, 11]) & 0xff) << 8,
    );
    let object = rng.pool(POOL_A);
    for offset in [0x10, 0x14, 0x18, 0x1c] {
        pair.put32(object + offset, rng.word(&[0, 1, 0x8000_0000]));
    }
    let masks = vec![
        object,
        rng.word(&[0, 1, 0x8000_0000, u32::MAX]),
        rng.word(&[0, 1, 0x8000_0000, u32::MAX]),
    ];
    mk(rng, 0x174e0, masks)
}
suite!(
    r_000174e0,
    mutant_000174e0,
    0x1836_0201,
    build_flag_pair,
    |v, c| outcome!(object_leaves::mode_selected_flag_pair_intersects(
        &*v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

fn build_set_field_24(rng: &mut Rng, pair: &mut Pair) -> Case {
    // 0x2c store aliasing the value slot (STACK + 8) changes the value stored at 0x24.
    let object = object_pointer(rng, &[STACK + 8 - 0x2c, STACK + 8 - 0x24, STACK + 4 - 0x2c]);
    pair.put32(object.wrapping_add(0x2c), rng.next());
    pair.put32(object.wrapping_add(0x24), rng.next());
    let value = rng.next();
    mk(rng, 0x71740, vec![object, value])
}
suite!(
    r_00071740,
    mutant_00071740,
    0x1836_0202,
    build_set_field_24,
    |v, c| outcome!(object_leaves::set_field_0x24_and_flag_0x2c_bit0(
        v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

fn build_ext_9d0(rng: &mut Rng, pair: &mut Pair) -> Case {
    let object = rng.pool(POOL_A);
    let child = match rng.below(8) {
        0 => object,
        1 => rng.next(),
        _ => rng.pool(POOL_B),
    };
    pair.put32(object.wrapping_add(0x7c), child);
    pair.put32(child.wrapping_add(0x20), rng.word(&[3, 2, 4, 0]));
    pair.put32(child.wrapping_add(0x9d0), rng.next());
    mk(rng, 0x264720, vec![object])
}
suite!(
    r_00264720,
    mutant_00264720,
    0x1836_0203,
    build_ext_9d0,
    |v, c| outcome!(object_leaves::ext_set_9d0_if_state_3(
        v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

fn build_field_value(
    rng: &mut Rng,
    pair: &mut Pair,
    va: u32,
    offset: u32,
    specials: &[u32],
) -> Case {
    let object = object_pointer(rng, &[]);
    pair.put32(object.wrapping_add(offset), rng.word(specials));
    mk(rng, va, vec![object])
}
suite!(
    r_0010ebd0,
    mutant_0010ebd0,
    0x1836_0204,
    |r, p| build_field_value(r, p, 0x10ebd0, 0x50, &[2, 1, 3]),
    |v, c| outcome!(object_leaves::is_field_0x50_equal_2(
        &*v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);
suite!(
    r_00234270,
    mutant_00234270,
    0x1836_0205,
    |r, p| build_field_value(r, p, 0x234270, 0x34, &[0, 1, u32::MAX]),
    |v, c| outcome!(object_leaves::is_field_0x34_nonzero_null_safe(
        &*v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

fn build_store_three(rng: &mut Rng, pair: &mut Pair) -> Case {
    // 0x24 / 0x28 stores landing on the fourth argument slot (STACK + 16) change the last store.
    let object = object_pointer(
        rng,
        &[STACK + 16 - 0x24, STACK + 16 - 0x28, STACK + 12 - 0x24],
    );
    for offset in [0x24, 0x28, 0x2c] {
        pair.put32(object.wrapping_add(offset), rng.next());
    }
    let args = vec![object, rng.next(), rng.next(), rng.next()];
    mk(rng, 0x36cc10, args)
}
suite!(
    r_0036cc10,
    mutant_0036cc10,
    0x1836_0206,
    build_store_three,
    |v, c| outcome!(object_leaves::store_three_args_into_fields_0x24_0x28_0x2c(
        v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

fn build_table_copy(rng: &mut Rng, pair: &mut Pair) -> Case {
    let index = if rng.below(8) == 0 {
        rng.next()
    } else {
        rng.below(8)
    };
    let table_word = index
        .wrapping_mul(5)
        .wrapping_mul(4)
        .wrapping_add(0x004d_1e28);
    let object = match rng.below(6) {
        // The 0x84 store lands on the table word that is read afterwards.
        0 => table_word.wrapping_sub(0x84),
        1 => table_word.wrapping_sub(0xa58),
        _ => rng.pool(POOL_A),
    };
    pair.put32(table_word, rng.next());
    pair.put32(object.wrapping_add(0x84), rng.next());
    pair.put32(object.wrapping_add(0xa58), rng.next());
    mk(rng, 0xd7550, vec![object, index])
}
suite!(
    r_000d7550,
    mutant_000d7550,
    0x1836_0207,
    build_table_copy,
    |v, c| outcome!(object_leaves::set_field_0x84_and_0xa58_from_table(
        v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

fn build_append(rng: &mut Rng, pair: &mut Pair) -> Case {
    let object = rng.pool(POOL_A);
    pair.put32(object.wrapping_add(0x7c), rng.next());
    // The count that makes the array store land on the count word itself.
    let alias = 0x0074_c37cu32.wrapping_sub(0x007a_3580) / 4 | 0xc000_0000;
    let count = rng.word(&[alias, 0x1000, u32::MAX, 0x4000_0000]);
    let slot = count.wrapping_mul(4).wrapping_add(0x007a_3580);
    pair.put32(slot, rng.next());
    pair.put32(0x0074_c37c, count);
    mk(rng, 0x15bca0, vec![object])
}
suite!(
    r_0015bca0,
    mutant_0015bca0,
    0x1836_0208,
    build_append,
    |v, c| outcome!(object_leaves::array_7a3580_append_field_0x7c(
        v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

fn build_nonnegative(rng: &mut Rng, pair: &mut Pair) -> Case {
    let object = rng.pool(POOL_A);
    let child = rng.pool(POOL_B);
    pair.put32(object.wrapping_add(0x7c), child);
    for offset in [0x56c, 0x578, 0x57c] {
        pair.put32(
            child.wrapping_add(offset),
            rng.word(&[0, 1, u32::MAX, 0x8000_0000, 0x7fff_ffff]),
        );
    }
    mk(rng, 0x177e90, vec![object])
}
suite!(
    r_00177e90,
    mutant_00177e90,
    0x1836_0209,
    build_nonnegative,
    |v, c| outcome!(object_leaves::child_fields_all_nonnegative(
        &*v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

fn build_flag_40(rng: &mut Rng, pair: &mut Pair) -> Case {
    let object = rng.pool(POOL_A);
    pair.put32(
        object.wrapping_add(8),
        (rng.next() & 0xffff_00ff) | (rng.word(&[0x40, 0, 0x3f, 0x80]) & 0xff) << 8,
    );
    let record = rng.pool(POOL_B);
    pair.put32(record.wrapping_add(0x38), rng.word(&[1, 0, 2]));
    let index = if rng.below(6) == 0 {
        rng.next()
    } else {
        rng.below(8)
    };
    pair.put32(record.wrapping_add(0x30), index);
    let table = if rng.below(6) == 0 {
        rng.next()
    } else {
        rng.pool(POOL_C)
    };
    pair.put32(0x004f_9bac, table);
    pair.put32(
        index
            .wrapping_mul(0x29c)
            .wrapping_add(table)
            .wrapping_add(12),
        rng.word(&[0x2_0000, 0, 0x1_0000, 0x4_0000, u32::MAX]),
    );
    mk(rng, 0x1b5840, vec![object, record])
}
suite!(
    r_001b5840,
    mutant_001b5840,
    0x1836_020a,
    build_flag_40,
    |v, c| outcome!(object_leaves::flag_0x40_or_kind_1_record_flag_20000_clear(
        &*v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

// ---- batch 3: object leaves with pointer chains and type tables ----

fn build_stash(rng: &mut Rng, pair: &mut Pair) -> Case {
    let object = if rng.below(8) == 0 {
        0
    } else {
        rng.pool(POOL_A)
    };
    pair.put32(
        object.wrapping_add(8),
        rng.word(&[0, 1, u32::MAX, 0x8000_0000]),
    );
    pair.put32(0x0079_0950, rng.word(&[0, 1, u32::MAX, 0x8000_0000, 5]));
    pair.put32(0x0075_9ed4, rng.word(&[0, 1]));
    pair.put32(object.wrapping_add(0x94), rng.word(&[0, 0, 7]));
    pair.put32(object.wrapping_add(0x98), rng.next());
    pair.put32(object.wrapping_add(0x9c), rng.next());
    mk(rng, 0x1b6540, vec![object])
}
suite!(
    r_001b6540,
    mutant_001b6540,
    0x1836_0301,
    build_stash,
    |v, c| outcome!(object_leaves::stash_field_0x94_to_0x9c(
        v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

fn build_masked(rng: &mut Rng, pair: &mut Pair) -> Case {
    let object = rng.pool(POOL_A);
    pair.put32(object.wrapping_add(0x80), rng.word(&[0, 1]));
    pair.put32(0x007d_e458, rng.word(&[0x1000, 0, 0xefff, u32::MAX]));
    let enable = rng.word(&[0, 1]);
    mk(rng, 0x1b6a10, vec![object, enable])
}
suite!(
    r_001b6a10,
    mutant_001b6a10,
    0x1836_0302,
    build_masked,
    |v, c| outcome!(object_leaves::field_0x80_set_and_not_masked(
        &*v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

fn build_type_entry(rng: &mut Rng, pair: &mut Pair) -> Case {
    let object = rng.pool(POOL_A);
    let child = rng.pool(POOL_B);
    pair.put32(object.wrapping_add(0x7c), child);
    let index = rng.word(&[0, 29, 30, 31, u32::MAX, 0x8000_0000]);
    pair.put32(child.wrapping_add(12), index);
    pair.put32(child.wrapping_add(0x10), rng.word(&[9, 12, 8, 13, 10, 11]));
    let entry = if (index as i32) >= 0 && index < 30 {
        index * 0xb4 + 0x0051_9758
    } else {
        0
    };
    pair.put32(entry + 0x90, rng.word(&[0, 1, 2, 4, 0x10, u32::MAX]));
    let owner = rng.pool(POOL_C);
    pair.put32(object.wrapping_add(4), owner);
    pair.put32(owner.wrapping_add(0x30), rng.word(&[1, 2, 4, 0x10, 0x20]));
    mk(rng, 0x242ee0, vec![object])
}
suite!(
    r_00242ee0,
    mutant_00242ee0,
    0x1836_0303,
    build_type_entry,
    |v, c| outcome!(object_leaves::type_entry_flag_mask_hit_or_kind_9_or_12(
        &*v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

fn build_type_table(rng: &mut Rng, pair: &mut Pair, va: u32, with_mask: bool) -> Case {
    let object = rng.pool(POOL_A);
    let half = rng.word(&[0, 1, 5, 0x7fff, 0x8000, 0xffff, 0xfffe]) & 0xffff;
    pair.put32(object.wrapping_add(0x48), (rng.next() & 0xffff_0000) | half);
    let index = half as i16 as i32 as u32;
    pair.put32(
        index.wrapping_mul(20).wrapping_add(0x0053_c508),
        rng.word(&[0, 1, 2, 3, 0x10, u32::MAX]),
    );
    pair.put32(
        0x007d_e454,
        (rng.next() & 0xffff_00ff) | (rng.word(&[10, 9, 11]) & 0xff) << 8,
    );
    pair.put32(0x007d_e458, rng.word(&[0x8_0000, 0, 0x4_0000, u32::MAX]));
    let mut args = vec![object];
    if with_mask {
        args.push(rng.word(&[1, 2, 3, 0x10, u32::MAX, 0]));
    }
    mk(rng, va, args)
}
suite!(
    r_00350d40,
    mutant_00350d40,
    0x1836_0304,
    |r, p| build_type_table(r, p, 0x350d40, false),
    |v, c| outcome!(object_leaves::type_table_dword_by_word_0x48(
        &*v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);
suite!(
    r_00350e70,
    mutant_00350e70,
    0x1836_0305,
    |r, p| build_type_table(r, p, 0x350e70, true),
    |v, c| outcome!(object_leaves::type_table_flag_mask_test(
        &*v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

fn build_vec3_copy(rng: &mut Rng, pair: &mut Pair) -> Case {
    let object = rng.pool(POOL_A);
    pair.put32(object.wrapping_add(32), rng.word(&[8, 7, 9]));
    let child = if rng.below(6) == 0 {
        0
    } else {
        rng.pool(POOL_B)
    };
    pair.put32(object.wrapping_add(0x7c), child);
    let ext = if rng.below(6) == 0 {
        0
    } else {
        rng.pool(POOL_C)
    };
    pair.put32(child.wrapping_add(0xaa0), ext);
    for offset in [20, 24, 28] {
        pair.put32(ext.wrapping_add(offset), rng.next());
    }
    // The destination may overlap the source fields still to be read.
    let out = match rng.below(6) {
        0 => ext.wrapping_add(20),
        1 => ext.wrapping_add(16),
        2 => ext.wrapping_add(24),
        _ => rng.pool(0x0070_0000),
    };
    for offset in [0, 4, 8] {
        pair.put32(out.wrapping_add(offset), rng.next());
    }
    mk(rng, 0x37a6f0, vec![object, out])
}
suite!(
    r_0037a6f0,
    mutant_0037a6f0,
    0x1836_0306,
    build_vec3_copy,
    |v, c| outcome!(object_leaves::type_8_copy_vec3_from_ext_child(
        v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);

fn build_room_entry(rng: &mut Rng, pair: &mut Pair) -> Case {
    let index = rng.below(6);
    let flag = rng.word(&[0, 1]);
    let list = rng.pool(POOL_B);
    pair.put32(0x007b_9784, list);
    let object = rng.pool(POOL_A);
    pair.put32(list.wrapping_add(index * 4), object);
    pair.put32(object.wrapping_add(0x1f0), rng.next());
    let table = rng.pool(POOL_C);
    pair.put32(0x0070_8788, table);
    let entry = rng.pool(0x0070_0000);
    pair.put32(index.wrapping_mul(0xb4).wrapping_add(table), entry);
    let multiple = rng.below(4);
    pair.put32(entry, multiple);
    for k in 0..4 {
        pair.put32(entry.wrapping_sub(0xc8 * k).wrapping_add(0x7c), rng.next());
    }
    mk(rng, 0x87840, vec![index, flag])
}
suite!(
    r_00087840,
    mutant_00087840,
    0x1836_0307,
    build_room_entry,
    |v, c| outcome!(object_leaves::field_0x1f0_or_room_entry_field_0x7c(
        &*v,
        STACK,
        entry!(tsfp_engine_object, c)
    ))
);
