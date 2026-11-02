// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-engine-object/closure/calls_w8
//! data/save save_unlock (T1899): unlock requirement checks, profile unlock bit helpers and the
//! unlock-requirement description text. Tier 0 ports (translated, compiles, smoke tested).
//!
//! Layout notes (see docs/t-save-layout.md, "Unlock descriptor switch"):
//! * Q = the profile unlock block returned by `0x69630` (global `0x6F1E0C`).
//! * A requirement descriptor is five dwords: type, a, b, list pointer, d.
use crate::memory::GuestMemory;

/// Override flag: when nonzero every nonzero-type requirement counts as met (`[0x75AEF4]`).
pub const GLOBAL_UNLOCK_ALL: u32 = 0x0075_aef4;
/// Per-category override bytes are read through `0x1CCEB0` from the table at `0x75AEF8`.
pub const OVERRIDE_BYTES: u32 = 0x0075_aef8;
/// Scratch bytes set by the unlock-features toggle (`0x1CCF00`).
const FEATURES_FLAG_BYTE: u32 = 0x0075_aec7;
/// Dword set to 0x78 by the unlock-features toggle.
const FEATURES_TIMER: u32 = 0x0075_af00;
/// Text widget record handed to `0x3C8960` by the toggle.
const FEATURES_WIDGET: u32 = 0x0075_ae88;
/// Format argument for the toggle text (`UNLOCK FEATURES: %d` style table entry).
const FEATURES_FORMAT_ARG: u32 = 0x0049_dcdc;
/// Guest argument string passed to the description sprintf (`0x3D7F0`).
const DESCRIPTION_SPRINTF_ARG: u32 = 0x0047_60e4;

/// Descriptor tables (base addresses and strides).
pub const TABLE_4D21F8: u32 = 0x004d_21f8;
pub const TABLE_4FCE08: u32 = 0x004f_ce08;
pub const TABLE_4FCC88: u32 = 0x004f_cc88;
pub const TABLE_4FA144: u32 = 0x004f_a144;
pub const TABLE_4D23AC: u32 = 0x004d_23ac;
/// Text table with 0x44-byte entries, word at +2 is a text id (`0x1CD5D0`, type 2).
const TABLE_4D1AC0: u32 = 0x004d_1ac0;

/// Descriptor field offsets.
pub mod desc {
    pub const KIND: u32 = 0x00;
    pub const A: u32 = 0x04;
    pub const B: u32 = 0x08;
    pub const LIST: u32 = 0x0c;
    pub const D: u32 = 0x10;
}

/// Fields of the record returned by `0x22E070` (stride 0x38).
mod record38 {
    pub const KIND: u32 = 0;
    pub const BIT_COUNT: u32 = 4;
    pub const BIT_IDS: u32 = 8;
    pub const NAME_TEXT_ID: u32 = 0x1c;
    pub const REQUIREMENT: u32 = 0x24;
}

/// Fields of the second record returned by `0x22E050`.
mod record_table {
    pub const NAME_TEXT_ID: u32 = 0;
    pub const REQUIREMENT: u32 = 0xa8;
}

/// `0x2BE590` returns a record whose requirement payload begins here.
const RECORD30_REQUIREMENT: u32 = 0x14;

/// Profile block (Q) offsets.
pub mod profile {
    /// Counter dwords, stride 0x18, 48 entries (reset to 4 by the toggle).
    pub const COUNTERS: u32 = 0x49c;
    pub const COUNTER_STRIDE: u32 = 0x18;
    pub const COUNTER_COUNT: u32 = 48;
    /// Single word tested by `0x1CD560`.
    pub const FLAGS_BF0: u32 = 0xbf0;
    /// Difficulty bit blocks start here, difficulty stride 0x10 (`0x1CD2B0`).
    pub const DIFFICULTY_BLOCKS: u32 = 0xbfc;
    pub const DIFFICULTY_STRIDE: u32 = 0x10;
    /// Selector words (`0x1CD380`), fallback block stride 3 words.
    pub const SELECTOR_WORDS: u32 = 0xc00;
    /// End of the all-ones/zero filled flag area (exclusive).
    pub const FLAG_AREA_END: u32 = 0xc4c;
}

/// System boundary: every original function outside this file's set.
pub trait UnlockHost {
    /// `0x69630` profile unlock block Q (`[0x6F1E0C]`).
    fn fn_00069630_profile_block(&mut self) -> u32;
    /// `0x69900(index)` statistics/progress block base.
    fn fn_00069900_stats_block(&mut self, index: u32) -> u32;
    /// `0x356870(0)` then `0x696D0(slot)`: nonzero when a local profile slot is active.
    fn fn_000696d0_local_slot_active(&mut self) -> u32;
    /// `0x1CCEB0(category)` override byte at `0x75AEF8 + category`, nonzero means met.
    fn fn_001cceb0_override_byte(&mut self, category: u32) -> u32;
    /// `0x22E070(index)` record-38: +0 kind, +4 count, +8 bit ids, +0x1c name text id, +0x24 descriptor.
    fn fn_0022e070_record38(&mut self, index: u32) -> u32;
    /// `0x22E050(index)` second record table; +0 word text id, +0xa8 descriptor.
    fn fn_0022e050_record_table(&mut self, index: u32) -> u32;
    /// `0x2BE590(key_a, key_b)` record-30 lookup, returns record or 0.
    fn fn_002be590_find_record30(&mut self, key_a: u32, key_b: u32) -> u32;
    /// `0x3D7B0(id)` text string from the bank, returns a guest string pointer.
    fn fn_0003d7b0_text_by_id(&mut self, id: u32) -> u32;
    /// `0x3FB90(src)` copy with case convert, returns a guest string pointer.
    fn fn_0003fb90_string_copy_case_convert(&mut self, source: u32) -> u32;
    /// `0x3D7F0(dest, format, arg_a, arg_b)` sprintf into `dest`.
    fn fn_0003d7f0_sprintf(&mut self, dest: u32, format: u32, arg_a: u32, arg_b: u32) -> u32;
    /// `0x45390(buffer)` initialises a 0x30-byte text context (toggle only).
    fn fn_00045390_text_context_init(&mut self, buffer: u32);
    /// `0x45820(context)` shows the text whose id is stored at `context` (toggle only).
    fn fn_00045820_text_show(&mut self, context: u32);
    /// `0x151030(format_arg, value)` returns a string pointer.
    fn fn_00151030_format_text(&mut self, format_arg: u32, value: u32) -> u32;
    /// `0x3C8960(widget, text, limit)` sets widget text.
    fn fn_003c8960_widget_set_text(&mut self, widget: u32, text: u32, limit: u32);
}

fn bit(shift: u32) -> u32 {
    1u32 << (shift & 31)
}

/// 0x001CCF00 game_unlock_features_text. Tier 0, INFERRED.
/// Toggles `[0x75AEF4]`, shows the matching text, resets the profile flags via
/// [`profile_set_all_unlock_flags`] and arms the display timer. `scratch` is a guest address of
/// 0x30 spare bytes (the original uses its stack frame; text id is stored at `scratch + 4`).
pub fn unlock_features_text(mem: &mut impl GuestMemory, host: &mut impl UnlockHost, scratch: u32) {
    let enabled = 1u32.wrapping_sub(mem.read32(GLOBAL_UNLOCK_ALL));
    mem.write32(GLOBAL_UNLOCK_ALL, enabled);
    host.fn_00045390_text_context_init(scratch);
    let text_id = if enabled == 0 { 0xd13 } else { 0xd11 };
    mem.write32(scratch.wrapping_add(4), text_id);
    host.fn_00045820_text_show(scratch.wrapping_add(4));
    profile_set_all_unlock_flags(mem, host, enabled);
    let text = host.fn_00151030_format_text(FEATURES_FORMAT_ARG, enabled);
    host.fn_003c8960_widget_set_text(FEATURES_WIDGET, text, 0x3f);
    mem.write8(FEATURES_FLAG_BYTE, 0);
    mem.write32(FEATURES_TIMER, 0x78);
}

/// 0x001CD040 game_unlock_bitmask_of_13_entries_met. Tier 0, INFERRED.
// portreg: va=0x001cd040
pub fn unlock_bitmask_of_met_entries(mem: &impl GuestMemory, host: &mut impl UnlockHost) -> u32 {
    let mut mask = 0u32;
    for index in 0..13u32 {
        if requirement_met_table_4fcc88(mem, host, index) != 0 {
            mask |= 1 << index;
        }
    }
    mask
}

/// 0x001CD120 game_unlock_count_of_met_entries. Tier 0, INFERRED.
/// Entry 12 additionally needs at least one of five sub-entries (table 4FCE08) met.
pub fn unlock_count_of_met_entries(mem: &impl GuestMemory, host: &mut impl UnlockHost) -> u32 {
    let mut count = 0u32;
    for index in 0..13u32 {
        if requirement_met_table_4fcc88(mem, host, index) == 0 {
            continue;
        }
        if index == 12 {
            let mut met_sub = 0i32;
            for sub in 0..5u32 {
                if requirement_met_table_4fce08(mem, host, sub) != 0 {
                    met_sub += 1;
                }
            }
            if met_sub <= 0 {
                continue;
            }
        }
        count += 1;
    }
    count
}

/// 0x001CD200 game_profile_set_all_unlock_flag_words. Tier 0, INFERRED.
/// `flag != 0` fills the flag words with all ones, else zero; resets 48 counters to 4.
/// The original re-fetches the block (`0x69900(0)`) before each store; fetched once here.
pub fn profile_set_all_unlock_flags(
    mem: &mut impl GuestMemory,
    host: &mut impl UnlockHost,
    flag: u32,
) {
    let value = if flag != 0 { u32::MAX } else { 0 };
    let base = host.fn_00069900_stats_block(0);
    let mut offset = 0xbf4u32;
    while offset < profile::FLAG_AREA_END {
        mem.write32(base.wrapping_add(offset), value);
        offset += 4;
    }
    for index in 0..profile::COUNTER_COUNT {
        let address = base.wrapping_add(profile::COUNTERS + index * profile::COUNTER_STRIDE);
        mem.write32(address, 4);
    }
    mem.write32(base.wrapping_add(profile::FLAGS_BF0), value);
}

/// 0x001CD2B0 game_profile_bit_set_for_difficulty. Tier 0, INFERRED.
/// `difficulty == -1` accepts when any of difficulties 1..=4 has the bit. Signed bit indices
/// keep the original SAR/signed-remainder arithmetic (no range check).
pub fn profile_bit_set_for_difficulty(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    bit_index: i32,
    difficulty: i32,
) -> u32 {
    let word = (bit_index >> 5) as u32;
    let mask = bit((bit_index % 32) as u32);
    if difficulty == -1 {
        let mut address = word.wrapping_mul(4).wrapping_add(0xc0c);
        for _ in 1..5 {
            let base = host.fn_00069630_profile_block();
            if mem.read32(base.wrapping_add(address)) & mask != 0 {
                return 1;
            }
            address = address.wrapping_add(profile::DIFFICULTY_STRIDE);
        }
        return 0;
    }
    let base = host.fn_00069630_profile_block();
    let slot = word.wrapping_add((difficulty as u32).wrapping_mul(4));
    let value = mem.read32(
        base.wrapping_add(slot.wrapping_mul(4))
            .wrapping_add(profile::DIFFICULTY_BLOCKS),
    );
    u32::from(value & mask != 0)
}

/// 0x001CD330 game_profile_all_bits_set_for_record38. Tier 0, INFERRED.
/// Every bit id of the record must be set; there is no short circuit. Count <= 0 accepts.
pub fn profile_all_bits_set_for_record38(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    record_index: u32,
    difficulty: i32,
) -> u32 {
    let record = host.fn_0022e070_record38(record_index);
    let count = mem.read32(record.wrapping_add(record38::BIT_COUNT)) as i32;
    let mut all_set = 1u32;
    let mut index = 0i32;
    while index < count {
        let bit_id = mem.read32(
            record
                .wrapping_add(record38::BIT_IDS)
                .wrapping_add(index as u32 * 4),
        ) as i32;
        if profile_bit_set_for_difficulty(mem, host, bit_id, difficulty) == 0 {
            all_set = 0;
        }
        index += 1;
    }
    all_set
}

/// 0x001CD380 game_profile_bit_set_selector_block. Tier 0, INFERRED.
/// Bit `bit_index` of word `word_index`: first in the `+0xC00` block unless `force_fallback`,
/// then in the block at `4 * (word + 3 * (selector + 255))`.
pub fn profile_bit_set_selector_block(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    bit_index: u32,
    word_index: u32,
    force_fallback: u32,
    selector: u32,
) -> u32 {
    let mask = bit(bit_index);
    if force_fallback == 0 {
        let base = host.fn_00069630_profile_block();
        let address = base
            .wrapping_add(word_index.wrapping_mul(4))
            .wrapping_add(profile::SELECTOR_WORDS);
        if mem.read32(address) & mask != 0 {
            return 1;
        }
    }
    let base = host.fn_00069630_profile_block();
    let slot = word_index.wrapping_add(selector.wrapping_add(0xff).wrapping_mul(3));
    u32::from(mem.read32(base.wrapping_add(slot.wrapping_mul(4))) & mask != 0)
}

/// 0x001CD3E0 game_unlock_requirement_met. Tier 0, INFERRED (type mapping from
/// docs/t-save-layout.md). `descriptor` is a guest address of the five dword record.
// portreg: va=0x001cd3e0
pub fn unlock_requirement_met(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    descriptor: u32,
) -> u32 {
    if host.fn_000696d0_local_slot_active() == 0 {
        return 0;
    }
    let kind = mem.read32(descriptor.wrapping_add(desc::KIND));
    if kind != 0 && mem.read32(GLOBAL_UNLOCK_ALL) != 0 {
        return 1;
    }
    let a = mem.read32(descriptor.wrapping_add(desc::A));
    let b = mem.read32(descriptor.wrapping_add(desc::B));
    let d = mem.read32(descriptor.wrapping_add(desc::D));
    match kind {
        1 => 1,
        2 => profile_bit_set_selector_block(mem, host, a, b, 0, d),
        3 => profile_bit_set_selector_block(mem, host, 12, b, 0, d),
        4 => profile_all_bits_set_for_record38(mem, host, a, b as i32),
        5 => profile_bit_set_for_difficulty(mem, host, a as i32, b as i32),
        6 => {
            let mut met = 0i32;
            for record in 0..7u32 {
                if profile_all_bits_set_for_record38(mem, host, record, b as i32) != 0 {
                    met += 1;
                }
            }
            u32::from(met >= a as i32)
        }
        7 => {
            let list = mem.read32(descriptor.wrapping_add(desc::LIST));
            if list == 0 {
                return 1;
            }
            let mut entry = mem.read32(list);
            if entry == u32::MAX {
                return 1;
            }
            let mut all = 1u32;
            let mut offset = 0u32;
            loop {
                if profile_all_bits_set_for_record38(mem, host, entry, b as i32) == 0 {
                    all = 0;
                }
                entry = mem.read32(list.wrapping_add(offset).wrapping_add(4));
                offset = offset.wrapping_add(4);
                if entry == u32::MAX {
                    return all;
                }
            }
        }
        8 => {
            for record in 7..16u32 {
                let record_ptr = host.fn_0022e070_record38(record);
                if mem.read32(record_ptr.wrapping_add(record38::KIND)) == a
                    && profile_all_bits_set_for_record38(mem, host, record, b as i32) == 0
                {
                    return 0;
                }
            }
            1
        }
        _ => 0,
    }
}

/// 0x001CD560 game_profile_bit_set_word_0xbf0_or_override. Tier 0, INFERRED.
pub fn profile_bit_set_word_bf0_or_override(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    bit_index: u32,
) -> u32 {
    if mem.read32(GLOBAL_UNLOCK_ALL) != 0 {
        return 1;
    }
    let base = host.fn_00069630_profile_block();
    u32::from(mem.read32(base.wrapping_add(profile::FLAGS_BF0)) & bit(bit_index) != 0)
}

/// 0x001CD5D0 game_unlock_requirement_description_text. Tier 0, INFERRED.
/// Returns a guest string pointer (text from the bank, or `dest` when a formatted line was
/// built into it) or 0 when the type has no text. UNRESOLVED: the jump table at `0x1CD86C` is
/// not decoded; the type to handler mapping (2,3,4,5,6,8) follows handler/field usage against
/// the `0x1CD3E0` semantics and types 0, 1, 7 and >8 are assumed to hit the default (0).
pub fn unlock_requirement_description_text(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    descriptor: u32,
    dest: u32,
) -> u32 {
    let kind = mem.read32(descriptor.wrapping_add(desc::KIND));
    let a = mem.read32(descriptor.wrapping_add(desc::A));
    let b = mem.read32(descriptor.wrapping_add(desc::B));
    let d = mem.read32(descriptor.wrapping_add(desc::D));
    match kind {
        2 => {
            if d != 0 {
                return 0;
            }
            let entry = TABLE_4D1AC0.wrapping_add(a.wrapping_mul(0x44));
            let format_id = match b {
                1 => 0x167f,
                2 => 0x1680,
                _ => 0x167e,
            };
            let text_id = i32::from(mem.read16(entry.wrapping_add(2)) as i16) as u32;
            let name = host.fn_0003d7b0_text_by_id(text_id);
            let converted = host.fn_0003fb90_string_copy_case_convert(name);
            let format = host.fn_0003d7b0_text_by_id(format_id);
            host.fn_0003d7f0_sprintf(dest, format, DESCRIPTION_SPRINTF_ARG, converted);
            dest
        }
        3 => {
            let id = if d != 0 {
                0x1685
            } else {
                match b {
                    1 => 0x1683,
                    2 => 0x1684,
                    _ => 0x1682,
                }
            };
            host.fn_0003d7b0_text_by_id(id)
        }
        4 => {
            let record = host.fn_0022e070_record38(a);
            let name_id = mem.read32(record.wrapping_add(record38::NAME_TEXT_ID));
            let format_id = 0x166f + 4 * u32::from(a as i32 >= 7);
            let name = host.fn_0003d7b0_text_by_id(name_id);
            let converted = host.fn_0003fb90_string_copy_case_convert(name);
            let format = host.fn_0003d7b0_text_by_id(format_id);
            host.fn_0003d7f0_sprintf(dest, format, DESCRIPTION_SPRINTF_ARG, converted);
            dest
        }
        5 => {
            let entry = host.fn_0022e050_record_table(a);
            let text_id =
                i32::from(mem.read16(entry.wrapping_add(record_table::NAME_TEXT_ID)) as i16) as u32;
            let name = host.fn_0003d7b0_text_by_id(text_id);
            let format_id = if (a as i32) < 0x15 {
                match b {
                    2 => 0x166d,
                    3 => 0x166e,
                    _ => 0x166c,
                }
            } else {
                match b {
                    2 => 0x1671,
                    3 => 0x1672,
                    _ => 0x1670,
                }
            };
            let converted = host.fn_0003fb90_string_copy_case_convert(name);
            let format = host.fn_0003d7b0_text_by_id(format_id);
            host.fn_0003d7f0_sprintf(dest, format, DESCRIPTION_SPRINTF_ARG, converted);
            dest
        }
        6 => {
            let id = match b {
                2 => 0x1687,
                3 => 0x1688,
                _ => 0x1686,
            };
            host.fn_0003d7b0_text_by_id(id)
        }
        8 => {
            // Grid: a in 0..=2 by b (3, 2 or other). Out of range a asks for text id 0.
            let ids: [u32; 3] = match b {
                2 => [0x1675, 0x1678, 0x167b],
                3 => [0x1676, 0x1679, 0x167c],
                _ => [0x1674, 0x1677, 0x167a],
            };
            let id = ids.get(a as usize).copied().unwrap_or(0);
            host.fn_0003d7b0_text_by_id(id)
        }
        _ => 0,
    }
}

/// 0x001CD890 game_unlock_requirement_met_type2_with_override_1. Tier 0, INFERRED.
/// Builds a type 2 descriptor `(2, a0 - 1, 0, _, a2)` in guest scratch memory at `scratch`
/// (the original uses its stack; the list pointer slot stays untouched). `a1` is unused.
// portreg: va=0x001cd890
pub fn unlock_requirement_met_type2_override_1(
    mem: &mut impl GuestMemory,
    host: &mut impl UnlockHost,
    a0: u32,
    a2: u32,
    scratch: u32,
) -> u32 {
    if host.fn_001cceb0_override_byte(1) != 0 || a0 == 0 {
        return 1;
    }
    mem.write32(scratch.wrapping_add(desc::KIND), 2);
    mem.write32(scratch.wrapping_add(desc::A), a0.wrapping_sub(1));
    mem.write32(scratch.wrapping_add(desc::B), 0);
    mem.write32(scratch.wrapping_add(desc::D), a2);
    unlock_requirement_met(mem, host, scratch)
}

/// 0x001CD8F0 game_unlock_requirement_met_for_descriptor. Tier 0, INFERRED.
/// (The original wrapper pushes a second zero argument that the callee ignores.)
pub fn requirement_met_for_descriptor(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    descriptor: u32,
) -> u32 {
    unlock_requirement_met(mem, host, descriptor)
}

/// 0x001CD900 requirement met for table 4D21F8 entry (stride 0x14). Tier 0, INFERRED.
pub fn requirement_met_table_4d21f8(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    index: u32,
) -> u32 {
    unlock_requirement_met(
        mem,
        host,
        TABLE_4D21F8.wrapping_add(index.wrapping_mul(0x14)),
    )
}

/// 0x001CD920 requirement met for table 4FCE08 entry (stride 0x14). Tier 0, INFERRED.
pub fn requirement_met_table_4fce08(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    index: u32,
) -> u32 {
    unlock_requirement_met(
        mem,
        host,
        TABLE_4FCE08.wrapping_add(index.wrapping_mul(0x14)),
    )
}

/// 0x001CD940 requirement met for table 4FCC88 entry (stride 0x1C). Tier 0, INFERRED.
/// A zero type rejects before the category-5 override is consulted.
// portreg: va=0x001cd940
pub fn requirement_met_table_4fcc88(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    index: u32,
) -> u32 {
    let entry = TABLE_4FCC88.wrapping_add(index.wrapping_mul(0x1c));
    if mem.read32(entry) == 0 {
        return 0;
    }
    if host.fn_001cceb0_override_byte(5) != 0 {
        return 1;
    }
    unlock_requirement_met(mem, host, entry)
}

/// 0x001CD980 requirement met for the record-30 payload of a key pair (override 4). Tier 0,
/// INFERRED. Missing payload or a zero type rejects.
// portreg: va=0x001cd980
pub fn requirement_met_record30_payload(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    key_a: u32,
    key_b: u32,
) -> u32 {
    let payload = find_record30_payload(host, key_a, key_b);
    if payload == 0 || mem.read32(payload) == 0 {
        return 0;
    }
    if host.fn_001cceb0_override_byte(4) != 0 {
        return 1;
    }
    unlock_requirement_met(mem, host, payload)
}

/// 0x001CD9D0 requirement met for weapon slot table 4FA144 entry (stride 0x3C). Tier 0, INFERRED.
// portreg: va=0x001cd9d0
pub fn requirement_met_weapon_slot(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    index: u32,
) -> u32 {
    unlock_requirement_met(
        mem,
        host,
        TABLE_4FA144.wrapping_add(index.wrapping_mul(0x3c)),
    )
}

/// 0x001CD9F0 requirement met for table 4D23AC entry (stride 0x3C, override 2). Tier 0, INFERRED.
// portreg: va=0x001cd9f0
pub fn requirement_met_table_4d23ac(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    index: u32,
) -> u32 {
    if host.fn_001cceb0_override_byte(2) != 0 {
        return 1;
    }
    unlock_requirement_met(
        mem,
        host,
        TABLE_4D23AC.wrapping_add(index.wrapping_mul(0x3c)),
    )
}

/// 0x001CDA20 requirement met for record-38 descriptor at +0x24. Tier 0, INFERRED.
/// Override category 3 for index below 7, else 2.
// portreg: va=0x001cda20
pub fn requirement_met_record38_entry(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    index: u32,
) -> u32 {
    let record = host.fn_0022e070_record38(index);
    let category = if (index as i32) < 7 { 3 } else { 2 };
    if host.fn_001cceb0_override_byte(category) != 0 {
        return 1;
    }
    unlock_requirement_met(mem, host, record.wrapping_add(record38::REQUIREMENT))
}

/// 0x001CDA70 requirement met for descriptor index with overrides. Tier 0, INFERRED.
/// Global override, then category override (3 below 0x15, else 2), then the record-38 check on
/// `record_index`, then the type switch on the `+0xA8` descriptor of record table entry `index`.
pub fn requirement_met_descriptor_index(
    mem: &impl GuestMemory,
    host: &mut impl UnlockHost,
    index: u32,
    record_index: u32,
) -> u32 {
    if mem.read32(GLOBAL_UNLOCK_ALL) != 0 {
        return 1;
    }
    let category = if (index as i32) < 0x15 { 3 } else { 2 };
    if host.fn_001cceb0_override_byte(category) != 0 {
        return 1;
    }
    if requirement_met_record38_entry(mem, host, record_index) == 0 {
        return 0;
    }
    let record = host.fn_0022e050_record_table(index);
    unlock_requirement_met(mem, host, record.wrapping_add(record_table::REQUIREMENT))
}

/// 0x002BE5E0 game_find_record_30_payload_by_key_pair. Tier 0, INFERRED.
/// Record +0x14 or 0 when not found (delegates the search to `0x2BE590`).
// portreg: va=0x002be5e0 tier=0
pub fn find_record30_payload(host: &mut impl UnlockHost, key_a: u32, key_b: u32) -> u32 {
    let record = host.fn_002be590_find_record30(key_a, key_b);
    if record == 0 {
        0
    } else {
        record.wrapping_add(RECORD30_REQUIREMENT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Mem(HashMap<u32, u8>);
    impl GuestMemory for Mem {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[derive(Default)]
    struct Host {
        profile: u32,
        slot_active: u32,
        overrides: HashMap<u32, u32>,
        record38: u32,
        calls: Vec<String>,
    }
    impl UnlockHost for Host {
        fn fn_00069630_profile_block(&mut self) -> u32 {
            self.profile
        }
        fn fn_00069900_stats_block(&mut self, _index: u32) -> u32 {
            self.profile
        }
        fn fn_000696d0_local_slot_active(&mut self) -> u32 {
            self.slot_active
        }
        fn fn_001cceb0_override_byte(&mut self, category: u32) -> u32 {
            self.overrides.get(&category).copied().unwrap_or(0)
        }
        fn fn_0022e070_record38(&mut self, index: u32) -> u32 {
            self.record38 + index * 0x38
        }
        fn fn_0022e050_record_table(&mut self, index: u32) -> u32 {
            0x6000 + index * 0x100
        }
        fn fn_002be590_find_record30(&mut self, key_a: u32, _key_b: u32) -> u32 {
            if key_a == 1 { 0x7000 } else { 0 }
        }
        fn fn_0003d7b0_text_by_id(&mut self, id: u32) -> u32 {
            self.calls.push(format!("text {id:#x}"));
            0x9000 + id
        }
        fn fn_0003fb90_string_copy_case_convert(&mut self, source: u32) -> u32 {
            source + 1
        }
        fn fn_0003d7f0_sprintf(&mut self, dest: u32, _f: u32, _a: u32, _b: u32) -> u32 {
            self.calls.push(format!("sprintf {dest:#x}"));
            dest
        }
        fn fn_00045390_text_context_init(&mut self, _buffer: u32) {}
        fn fn_00045820_text_show(&mut self, context: u32) {
            self.calls.push(format!("show {context:#x}"));
        }
        fn fn_00151030_format_text(&mut self, _arg: u32, value: u32) -> u32 {
            0x8000 + value
        }
        fn fn_003c8960_widget_set_text(&mut self, _w: u32, text: u32, limit: u32) {
            self.calls.push(format!("widget {text:#x} {limit}"));
        }
    }

    fn setup() -> (Mem, Host) {
        let host = Host {
            profile: 0x1000,
            slot_active: 1,
            record38: 0x2000,
            ..Host::default()
        };
        (Mem::default(), host)
    }

    fn put_descriptor(mem: &mut Mem, at: u32, fields: [u32; 5]) {
        for (i, v) in fields.iter().enumerate() {
            mem.write32(at + 4 * i as u32, *v);
        }
    }

    #[test]
    fn difficulty_bit_specific_and_any() {
        let (mut mem, mut host) = setup();
        // bit 35 = word 1 bit 3; difficulty 2 block: 0x1000 + 0xbfc + (1 + 8) * 4
        mem.write32(0x1000 + 0xbfc + 9 * 4, 1 << 3);
        assert_eq!(profile_bit_set_for_difficulty(&mem, &mut host, 35, 2), 1);
        assert_eq!(profile_bit_set_for_difficulty(&mem, &mut host, 35, 3), 0);
        assert_eq!(profile_bit_set_for_difficulty(&mem, &mut host, 35, -1), 1);
        assert_eq!(profile_bit_set_for_difficulty(&mem, &mut host, 36, -1), 0);
    }

    #[test]
    fn selector_block_and_fallback() {
        let (mut mem, mut host) = setup();
        mem.write32(0x1000 + 0xc00 + 4, 1 << 5);
        assert_eq!(
            profile_bit_set_selector_block(&mem, &mut host, 5, 1, 0, 1),
            1
        );
        // forced fallback with selector 1 starts at +0xc00 too: word 1 -> same address
        assert_eq!(
            profile_bit_set_selector_block(&mem, &mut host, 5, 1, 1, 1),
            1
        );
        assert_eq!(
            profile_bit_set_selector_block(&mem, &mut host, 6, 1, 0, 1),
            0
        );
    }

    #[test]
    fn requirement_types() {
        let (mut mem, mut host) = setup();
        let at = 0x3000;
        put_descriptor(&mut mem, at, [1, 0, 0, 0, 0]);
        assert_eq!(unlock_requirement_met(&mem, &mut host, at), 1);
        put_descriptor(&mut mem, at, [0, 0, 0, 0, 0]);
        assert_eq!(unlock_requirement_met(&mem, &mut host, at), 0);
        put_descriptor(&mut mem, at, [5, 3, 1, 0, 0]);
        assert_eq!(unlock_requirement_met(&mem, &mut host, at), 0);
        mem.write32(0x1000 + 0xc0c, 1 << 3);
        assert_eq!(unlock_requirement_met(&mem, &mut host, at), 1);
        host.slot_active = 0;
        assert_eq!(unlock_requirement_met(&mem, &mut host, at), 0);
        host.slot_active = 1;
        // global override accepts a nonzero type with nothing set
        put_descriptor(&mut mem, at, [5, 9, 2, 0, 0]);
        assert_eq!(unlock_requirement_met(&mem, &mut host, at), 0);
        mem.write32(GLOBAL_UNLOCK_ALL, 1);
        assert_eq!(unlock_requirement_met(&mem, &mut host, at), 1);
    }

    #[test]
    fn record38_all_bits_and_list_type() {
        let (mut mem, mut host) = setup();
        let record = 0x2000 + 2 * 0x38;
        mem.write32(record + 4, 2);
        mem.write32(record + 8, 1);
        mem.write32(record + 12, 2);
        mem.write32(0x1000 + 0xc0c, 0b110);
        assert_eq!(profile_all_bits_set_for_record38(&mem, &mut host, 2, 1), 1);
        mem.write32(0x1000 + 0xc0c, 0b010);
        assert_eq!(profile_all_bits_set_for_record38(&mem, &mut host, 2, 1), 0);
        // type 7 list [2, -1]
        mem.write32(0x4000, 2);
        mem.write32(0x4004, u32::MAX);
        put_descriptor(&mut mem, 0x3000, [7, 0, 1, 0x4000, 0]);
        assert_eq!(unlock_requirement_met(&mem, &mut host, 0x3000), 0);
        mem.write32(0x1000 + 0xc0c, 0b110);
        assert_eq!(unlock_requirement_met(&mem, &mut host, 0x3000), 1);
    }

    #[test]
    fn set_all_flags_and_features_toggle() {
        let (mut mem, mut host) = setup();
        profile_set_all_unlock_flags(&mut mem, &mut host, 5);
        assert_eq!(mem.read32(0x1000 + 0xbf4), u32::MAX);
        assert_eq!(mem.read32(0x1000 + 0xc48), u32::MAX);
        assert_eq!(mem.read32(0x1000 + 0xc4c), 0);
        assert_eq!(mem.read32(0x1000 + 0xbf0), u32::MAX);
        assert_eq!(mem.read32(0x1000 + 0x49c + 0x18 * 47), 4);
        assert_eq!(mem.read32(0x1000 + 0x49c + 0x18 * 48), 0);
        profile_set_all_unlock_flags(&mut mem, &mut host, 0);
        assert_eq!(mem.read32(0x1000 + 0xc00), 0);

        unlock_features_text(&mut mem, &mut host, 0x5000);
        assert_eq!(mem.read32(GLOBAL_UNLOCK_ALL), 1);
        assert_eq!(mem.read32(0x5004), 0xd11);
        assert_eq!(mem.read32(FEATURES_TIMER), 0x78);
        assert_eq!(mem.read32(0x1000 + 0xbf0), u32::MAX);
        unlock_features_text(&mut mem, &mut host, 0x5000);
        assert_eq!(mem.read32(GLOBAL_UNLOCK_ALL), 0);
        assert_eq!(mem.read32(0x5004), 0xd13);
    }

    #[test]
    fn table_wrappers_and_counts() {
        let (mut mem, mut host) = setup();
        // table 4fcc88 entries 0 and 1 are type 5 with bit 0 / 1 at difficulty 1
        for (i, bit_id) in [(0u32, 0u32), (1, 1), (2, 2)] {
            put_descriptor(&mut mem, TABLE_4FCC88 + i * 0x1c, [5, bit_id, 1, 0, 0]);
        }
        mem.write32(0x1000 + 0xc0c, 0b011);
        assert_eq!(unlock_bitmask_of_met_entries(&mem, &mut host), 0b011);
        assert_eq!(unlock_count_of_met_entries(&mem, &mut host), 2);
        // entry 12 met but no sub entry met -> not counted
        put_descriptor(&mut mem, TABLE_4FCC88 + 12 * 0x1c, [1, 0, 0, 0, 0]);
        assert_eq!(unlock_count_of_met_entries(&mem, &mut host), 2);
        put_descriptor(&mut mem, TABLE_4FCE08 + 3 * 0x14, [1, 0, 0, 0, 0]);
        assert_eq!(unlock_count_of_met_entries(&mem, &mut host), 3);
        // zero-type entry rejects before the override
        host.overrides.insert(5, 1);
        assert_eq!(requirement_met_table_4fcc88(&mem, &mut host, 4), 0);
        assert_eq!(requirement_met_table_4fcc88(&mem, &mut host, 0), 1);
        host.overrides.insert(2, 1);
        assert_eq!(requirement_met_table_4d23ac(&mem, &mut host, 0), 1);
        assert_eq!(requirement_met_table_4d21f8(&mem, &mut host, 0), 0);
    }

    #[test]
    fn record30_and_overrides() {
        let (mut mem, mut host) = setup();
        assert_eq!(find_record30_payload(&mut host, 1, 0), 0x7014);
        assert_eq!(find_record30_payload(&mut host, 2, 0), 0);
        assert_eq!(requirement_met_record30_payload(&mem, &mut host, 2, 0), 0);
        put_descriptor(&mut mem, 0x7014, [1, 0, 0, 0, 0]);
        assert_eq!(requirement_met_record30_payload(&mem, &mut host, 1, 0), 1);
        host.overrides.insert(3, 1);
        assert_eq!(requirement_met_record38_entry(&mem, &mut host, 2), 1);
        host.overrides.clear();
        host.overrides.insert(1, 1);
        assert_eq!(
            unlock_requirement_met_type2_override_1(&mut mem, &mut host, 5, 0, 0x5000),
            1
        );
        host.overrides.clear();
        assert_eq!(
            unlock_requirement_met_type2_override_1(&mut mem, &mut host, 0, 0, 0x5000),
            1
        );
        assert_eq!(
            unlock_requirement_met_type2_override_1(&mut mem, &mut host, 3, 0, 0x5000),
            0
        );
        assert_eq!(mem.read32(0x5004), 2);
        mem.write32(GLOBAL_UNLOCK_ALL, 1);
        assert_eq!(requirement_met_descriptor_index(&mem, &mut host, 0, 0), 1);
        assert_eq!(profile_bit_set_word_bf0_or_override(&mem, &mut host, 3), 1);
    }

    #[test]
    fn description_text() {
        let (mut mem, mut host) = setup();
        put_descriptor(&mut mem, 0x3000, [3, 0, 2, 0, 0]);
        assert_eq!(
            unlock_requirement_description_text(&mem, &mut host, 0x3000, 0x6000),
            0x9000 + 0x1684
        );
        put_descriptor(&mut mem, 0x3000, [8, 2, 2, 0, 0]);
        assert_eq!(
            unlock_requirement_description_text(&mem, &mut host, 0x3000, 0x6000),
            0x9000 + 0x167b
        );
        put_descriptor(&mut mem, 0x3000, [2, 0, 1, 0, 1]);
        assert_eq!(
            unlock_requirement_description_text(&mem, &mut host, 0x3000, 0x6000),
            0
        );
        put_descriptor(&mut mem, 0x3000, [6, 0, 3, 0, 0]);
        assert_eq!(
            unlock_requirement_description_text(&mem, &mut host, 0x3000, 0x6000),
            0x9000 + 0x1688
        );
        put_descriptor(&mut mem, 0x3000, [4, 8, 1, 0, 0]);
        assert_eq!(
            unlock_requirement_description_text(&mem, &mut host, 0x3000, 0x6000),
            0x6000
        );
    }
}
