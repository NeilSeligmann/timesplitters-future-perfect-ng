// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-gameplay/call_leaves
// portreg: mirrors=tsfp-gameplay/slot_leaves
//! T1900 slice B (Tier 0): multiplayer side/slot tables, party snapshot and roster scoring.
//!
//! Translated by hand from the lifted originals, readable names, no differential comparison.
//! Closure model of `tsfp_engine_object::closure` (nested guest calls through [`Callee`]).
//! Leaves that never call out take plain arguments and return the EAX value. Functions that
//! call out take `(callee, memory, cpu)` and read their cdecl arguments with `stack_arg`.
//! See docs/t-multiplayer-rules.md for the system context.
use crate::closure_api::{Callee, Cpu, GuestMemory, guest_call, stack_arg};

// ---- Globals (guest addresses) -------------------------------------------------------------
/// `u8` multiplayer mode. 2 = sided mode (275640), 0xA = split-screen party mode (CB240, D8A00...).
pub const MODE_BYTE: u32 = 0x007d_e455;
/// `u8` secondary mode byte, 0x13 is the roster exclusion mode tested by 2F1FC0.
pub const MODE_BYTE_2: u32 = 0x007d_e456;
/// `u32` flags, bit 9 (0x200) is tested by 2E4470.
pub const MODE_FLAGS: u32 = 0x007d_e458;
/// `i32` local player count.
pub const PLAYER_COUNT: u32 = 0x0079_0950;
/// `i32` extra roster entries added to [`PLAYER_COUNT`] (2F1FC0).
pub const ROSTER_EXTRA_COUNT: u32 = 0x007a_2958;
/// `u8` (signed) number of sides/teams. 0 disables the side table.
pub const SIDE_COUNT_BYTE: u32 = 0x007d_e402;
/// `u32` frame/event stamp copied into the side table by 275640.
pub const SIDE_STAMP: u32 = 0x007d_e318;
/// Pointer to the side-count table (0x100 bytes), allocated by 274650.
pub const SIDE_TABLE_PTR: u32 = 0x0074_c3b8;
/// Pointer to the second side block (`(sides * 12 + 15) & !15` bytes), allocated by 274650.
pub const SIDE_BLOCK_PTR: u32 = 0x0074_c3bc;
/// Object table base, stride 8, entry +0 is an object pointer (index 1 and 2 are the two sides).
pub const OBJECT_TABLE: u32 = 0x007a_2be4;
/// Stride of [`OBJECT_TABLE`] entries.
pub const OBJECT_TABLE_STRIDE: u32 = 8;
/// `i32` entry count of [`NAMED_TABLE`].
pub const NAMED_TABLE_COUNT: u32 = 0x0074_c394;
/// Table of 8 byte entries searched by 160AC0, entry +0 is the key.
pub const NAMED_TABLE: u32 = 0x007a_2c60;
/// Player-record array pointer variable, stride [`PLAYER_STRIDE`], first entry at +0x14.
pub const PLAYER_ARRAY_PTR: u32 = 0x007b_0c48;
pub const PLAYER_STRIDE: u32 = 0x1584;
/// `u32` global cleared by CB240 (getter at CB0F0).
pub const PARTY_FLAG: u32 = 0x0073_30d8;
/// Per-player u16 id lists, 0x14 ids per player, zero terminated (D8B10 writes, D8A00 reads).
pub const SNAPSHOT_IDS: u32 = 0x0073_5630;
/// Per-player 0xD8 byte record snapshots, 0x14 per player (stride 0x10E0 per player).
pub const SNAPSHOT_RECORDS: u32 = 0x0073_31a8;
pub const SNAPSHOT_SLOTS_PER_PLAYER: u32 = 0x14;
pub const SNAPSHOT_RECORD_SIZE: u32 = 0xd8;
/// Roster pointer variable, entries of [`ROSTER_STRIDE`] bytes (+8 slot id, +0x48 type word).
pub const ROSTER_PTR: u32 = 0x0073_56d8;
pub const ROSTER_STRIDE: u32 = 0xea0;
/// Float constants read by 2F1FC0 and 2F2050.
pub const COUNT_THRESHOLD: u32 = 0x0047_5cac;
pub const SCORE_WEIGHT_STEP: u32 = 0x0047_5d24;
pub const SCORE_WEIGHT_BASE: u32 = 0x0047_5c78;
/// Rodata table of 0x20 byte rows indexed by `[MODE_BYTE_2]`, dword +0 is a rule kind (2F2050).
pub const RULE_KIND_TABLE: u32 = 0x004d_1f84;

// ---- Record offsets ------------------------------------------------------------------------
/// Object +0x7C points at its state record.
pub const OBJECT_RECORD: u32 = 0x7c;
/// State record +0xA0 flags, bit 2 (4) is tested by 274330/274350.
pub const RECORD_FLAGS_A0: u32 = 0xa0;
/// Object +0x28 flags, bit 7 set means "owned" (274380).
pub const OBJECT_FLAGS_28: u32 = 0x28;

fn f32_at<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

/// Read access to the 7A2BE4 object table so other systems can attach.
pub struct ObjectTable;

impl ObjectTable {
    /// Object pointer of entry `index` (`[7A2BE4 + index * 8]`). Wraps like the original.
    pub fn object<M: GuestMemory>(m: &M, index: u32) -> u32 {
        m.read32(OBJECT_TABLE.wrapping_add(index.wrapping_mul(OBJECT_TABLE_STRIDE)))
    }

    /// State record of entry `index` (`[object + 0x7C]`).
    pub fn record<M: GuestMemory>(m: &M, index: u32) -> u32 {
        m.read32(Self::object(m, index).wrapping_add(OBJECT_RECORD))
    }

    /// Entry index of the opposing side: arg 1 maps to 2, every other value to 1.
    pub fn opposing_index(side: u32) -> u32 {
        u32::from(side == 1) + 1
    }
}

/// 00274330 cdecl(1): entry index. Returns `[[entry].+0x7C].+0xA0 & 4` (0 or 4).
pub fn slot_record_flag_a0_bit2<M: GuestMemory>(m: &M, index: u32) -> u32 {
    m.read32(ObjectTable::record(m, index).wrapping_add(RECORD_FLAGS_A0)) & 4
}

/// 00274350 cdecl(1): side. Same test as 274330 on the opposing side's entry.
pub fn opposing_record_flag_a0_bit2<M: GuestMemory>(m: &M, side: u32) -> u32 {
    slot_record_flag_a0_bit2(m, ObjectTable::opposing_index(side))
}

/// 00274380 cdecl(1): entry index. Returns 1 when bit 7 of dword `[object + 0x28]` is clear.
/// (2743A0 is the byte variant, ported in `synth_leaves`.)
pub fn slot_object_flag_28_bit7_clear<M: GuestMemory>(m: &M, index: u32) -> u32 {
    let flags = m.read32(ObjectTable::object(m, index).wrapping_add(OBJECT_FLAGS_28));
    (!(flags >> 7)) & 1
}

/// 002744F0 cdecl(1): side. Returns the object pointer of the opposing side's entry.
pub fn opposing_side_object<M: GuestMemory>(m: &M, side: u32) -> u32 {
    ObjectTable::object(m, ObjectTable::opposing_index(side))
}

/// 00274570 cdecl(1): side. Tail jump to 274510 with the argument replaced by the opposing
/// side index (1 or 2). 274510 is the "first element of array 7A3580 for this side" search.
pub fn find_first_for_opposing_side<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let side = stack_arg(m, cpu, 0);
    m.write32(cpu.esp.wrapping_add(4), ObjectTable::opposing_index(side));
    c.call(m, 0x0027_4510, cpu);
}

/// 00274650 cdecl(0): allocates the side tables. Calls the 3DFA0 allocator (cdecl(size, kind),
/// kind 1) for 0x100 bytes into [`SIDE_TABLE_PTR`], then for
/// `(signed_byte([SIDE_COUNT_BYTE]) * 12 + 15) & !15` bytes into [`SIDE_BLOCK_PTR`].
/// Returns EAX = the second allocation.
pub fn allocate_side_tables<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    guest_call(c, m, cpu, 0x0003_dfa0, 0x0027_465c, 0, &[0x100, 1]);
    m.write32(SIDE_TABLE_PTR, cpu.eax);
    let sides = m.read8(SIDE_COUNT_BYTE) as i8 as i32 as u32;
    let size = sides.wrapping_mul(12).wrapping_add(15) & 0xffff_fff0;
    // The first call's two argument words are still on the stack (the lifted code pops both after the second call).
    guest_call(c, m, cpu, 0x0003_dfa0, 0x0027_467d, 8, &[size, 1]);
    m.write32(SIDE_BLOCK_PTR, cpu.eax);
}

/// 00275640 cdecl(1): `index`. In sided mode (side count byte non-zero and mode byte 2) the
/// 16 byte entries of [`SIDE_TABLE_PTR`] (addressed at `table + index * 16 - 8`) hold a dword
/// flag at +0 and a stamp at +4. The flag becomes 1 when the object's bit 7 is set (0 when
/// clear, per 274380); on change the stamp is refreshed from [`SIDE_STAMP`].
pub fn side_flag_refresh<M: GuestMemory>(m: &mut M, index: u32) {
    if m.read8(SIDE_COUNT_BYTE) == 0 || m.read8(MODE_BYTE) != 2 {
        return;
    }
    let owned_clear = slot_object_flag_28_bit7_clear(m, index);
    let entry = m
        .read32(SIDE_TABLE_PTR)
        .wrapping_add(index << 4)
        .wrapping_sub(8);
    let flag = u32::from(owned_clear == 0);
    if m.read32(entry) != flag {
        m.write32(entry, flag);
        let table = m.read32(SIDE_TABLE_PTR);
        let stamp = m.read32(SIDE_STAMP);
        m.write32(
            index.wrapping_shl(4).wrapping_add(table).wrapping_sub(4),
            stamp,
        );
    }
}

/// 00160AC0 cdecl(1): key. Linear search of the 8 byte entries of [`NAMED_TABLE`]
/// (`[74C394]` entries, signed). Returns the entry address or 0.
pub fn find_named_entry<M: GuestMemory>(m: &M, key: u32) -> u32 {
    let count = m.read32(NAMED_TABLE_COUNT) as i32;
    (0..count.max(0) as u32)
        .map(|i| NAMED_TABLE.wrapping_add(i * 8))
        .find(|&entry| m.read32(entry) == key)
        .unwrap_or(0)
}

/// 000CB240 cdecl(0): leaves split-screen party mode. Only when the mode byte is 0xA and the
/// player count is above 1: sets bit 0 of `[player0 + 0x21C]` and calls 25A570(0, +0x224)
/// (viewport setup), the same for the second player record at `P + 0x158C` when present
/// (`25A570(1, ...)`), clears bit 0 of `P+0xC`, bit 1 of `P+0x1590` and [`PARTY_FLAG`].
/// `P = [7B0C48]` is re-read after every call, as in the original.
pub fn leave_party_mode<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    if m.read8(MODE_BYTE) != 0xa || (m.read32(PLAYER_COUNT) as i32) <= 1 {
        return;
    }
    let p = m.read32(PLAYER_ARRAY_PTR);
    let first = m.read32(p.wrapping_add(8));
    m.write32(
        first.wrapping_add(0x21c),
        m.read32(first.wrapping_add(0x21c)) | 1,
    );
    let p = m.read32(PLAYER_ARRAY_PTR);
    let first = m.read32(p.wrapping_add(8));
    guest_call(
        c,
        m,
        cpu,
        0x0025_a570,
        0x000c_b27c,
        0,
        &[0, first.wrapping_add(0x224)],
    );
    let p = m.read32(PLAYER_ARRAY_PTR);
    let second = m.read32(p.wrapping_add(0x158c));
    if second != 0 {
        m.write32(
            second.wrapping_add(0x21c),
            m.read32(second.wrapping_add(0x21c)) | 1,
        );
        let p = m.read32(PLAYER_ARRAY_PTR);
        let second = m.read32(p.wrapping_add(0x158c));
        guest_call(
            c,
            m,
            cpu,
            0x0025_a570,
            0x000c_b2b0,
            0,
            &[1, second.wrapping_add(0x224)],
        );
    }
    let p = m.read32(PLAYER_ARRAY_PTR);
    m.write32(p.wrapping_add(0xc), m.read32(p.wrapping_add(0xc)) & !1);
    let p = m.read32(PLAYER_ARRAY_PTR);
    m.write32(
        p.wrapping_add(0x1590),
        m.read32(p.wrapping_add(0x1590)) & !2,
    );
    m.write32(PARTY_FLAG, 0);
}

/// 002E4470 cdecl(0): party mode needs special handling. False unless mode byte is 0xA and
/// the player count is above 1. Then true when CB0F0 (reads [`PARTY_FLAG`]) is non-zero, else
/// bit 9 of [`MODE_FLAGS`].
pub fn party_mode_active<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    cpu.eax = 0;
    if m.read8(MODE_BYTE) != 0xa || (m.read32(PLAYER_COUNT) as i32) <= 1 {
        return;
    }
    guest_call(c, m, cpu, 0x000c_b0f0, 0x002e_4487, 0, &[]);
    cpu.eax = u32::from(cpu.eax != 0 || m.read32(MODE_FLAGS) & 0x200 != 0);
}

/// A callee that returns a value in x87 ST0 (the closure model only carries integer registers).
pub trait X87Callee<M: GuestMemory>: Callee<M> {
    /// ST0 as left by the most recent `call`.
    fn st0(&self) -> f64;
}

/// 002F1FC0 cdecl(1): `stat_key`. Counts roster entries whose stat is positive. Iterates
/// `[790950] + [7A2958]` entries (re-read each step) of the roster at `[7356D8]` (stride
/// 0xEA0). Null entries are skipped. In mode `[7DE456] == 0x13`, entries with slot id (+8)
/// at or above the player count are skipped when D6C90(i16 +0x48) is non-zero. Each remaining
/// entry calls 238580(stat_key, slot id), a float stat in ST0, counted when above
/// `[475CAC]`. Returns the count in EAX.
pub fn count_roster_with_positive_stat<M: GuestMemory, C: X87Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let stat_key = stack_arg(m, cpu, 0);
    let mut count: u32 = 0;
    let total =
        |m: &M| (m.read32(ROSTER_EXTRA_COUNT) as i32).wrapping_add(m.read32(PLAYER_COUNT) as i32);
    if total(m) <= 0 {
        cpu.eax = 0;
        return;
    }
    let mut index: i32 = 0;
    loop {
        let entry = m
            .read32(ROSTER_PTR)
            .wrapping_add((index as u32).wrapping_mul(ROSTER_STRIDE));
        if entry != 0 {
            let slot = m.read32(entry.wrapping_add(8));
            let mut skip = false;
            if m.read8(MODE_BYTE_2) == 0x13 && (slot as i32) >= (m.read32(PLAYER_COUNT) as i32) {
                let type_word = m.read16(entry.wrapping_add(0x48)) as i16 as i32 as u32;
                guest_call(c, m, cpu, 0x000d_6c90, 0x002f_2005, 0x10, &[type_word]);
                skip = cpu.eax != 0;
            }
            if !skip {
                guest_call(c, m, cpu, 0x0023_8580, 0x002f_201a, 0x10, &[stat_key, slot]);
                if c.st0() > f64::from(f32_at(m, COUNT_THRESHOLD)) {
                    count += 1;
                }
            }
        }
        index += 1;
        if index >= total(m) {
            break;
        }
    }
    cpu.eax = count;
}

/// 002F2050 cdecl(2): `(out: *mut i32 [7], keys: *const u32 [7])`, no return value.
/// Scores each of 7 categories as `count_roster_with_positive_stat(keys[i]) * ((7 - i) *
/// [475D24] + [475C78])` (f32 arithmetic, category 6 is forced to 0 when the rule kind
/// `[4D1F84 + [7DE456] * 0x20]` is 1, or 2 with a non-zero word at `69640() + mode * 20 + 0x24`).
/// Then seven rounds each pick the highest not yet used score (ties go to the later category,
/// scores below 0 never qualify) into `out[round]`: the category index when its score is above
/// 0 else -1. A picked category is marked used even when its score is exactly 0.
pub fn rank_categories_by_roster_score<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let out = stack_arg(m, cpu, 0);
    let keys = stack_arg(m, cpu, 1);
    let mut scores = [0.0f32; 7];
    for (i, score) in scores.iter_mut().enumerate() {
        if i == 6 {
            guest_call(c, m, cpu, 0x0006_9640, 0x002f_2071, 0x2c, &[]);
            let mode = u32::from(m.read8(MODE_BYTE));
            let row = u32::from(m.read8(MODE_BYTE_2)) << 5;
            let kind = m.read32(row.wrapping_add(RULE_KIND_TABLE));
            let flag_word = cpu.eax.wrapping_add(mode * 20).wrapping_add(0x24);
            if kind == 1 || (kind == 2 && m.read32(flag_word) != 0) {
                *score = 0.0;
                continue;
            }
        }
        let key = m.read32(keys.wrapping_add(4 * i as u32));
        guest_call(c, m, cpu, 0x002f_1fc0, 0x002f_20b2, 0x2c, &[key]);
        let weight =
            ((7 - i as i32) as f32) * f32_at(m, SCORE_WEIGHT_STEP) + f32_at(m, SCORE_WEIGHT_BASE);
        *score = (cpu.eax as i32 as f32) * weight;
    }
    let mut used: u32 = 0;
    for round in 0..7u32 {
        let mut best_index: i32 = -1;
        let mut best = 0.0f32;
        // Candidate 0 needs score >= 0 (an unordered compare fails), later ones score >= best.
        if !(scores[0] < 0.0 || scores[0].is_nan()) && used & 1 == 0 {
            best_index = 0;
            best = scores[0];
        }
        for (category, &score) in scores.iter().enumerate().skip(1) {
            if !(score < best || score.is_nan() || best.is_nan()) && used & (1 << category) == 0 {
                best_index = category as i32;
                best = score;
            }
        }
        let picked = if best > 0.0 {
            best_index as u32
        } else {
            u32::MAX
        };
        m.write32(out.wrapping_add(4 * round), picked);
        if best_index > -1 {
            used |= 1 << best_index;
        }
    }
}

/// 000D8B10 cdecl(0): snapshots the party. Only in mode 0xA with a positive player count. For
/// each player i, walks the linked list at `[[[P + 0x14 + i*0x1584]] + 0x7C] + 0x534]` (node
/// +0x10 = object, +0x18 = next). Per node j it stores the object's u16 type (+0x94) at
/// `[735630 + 4*(i*0x14 + j)]` and copies the 0xD8 byte record `[object + 0x7C]` to
/// `7331A8 + i*0x10E0 + j*0xD8`. The list is then terminated with a zero id. There is no bound
/// on the list length (a list longer than 0x14 overruns into the next player's slots).
pub fn snapshot_party<M: GuestMemory>(m: &mut M) {
    if m.read8(MODE_BYTE) != 0xa {
        return;
    }
    let players = m.read32(PLAYER_COUNT) as i32;
    if players <= 0 {
        return;
    }
    for player in 0..players as u32 {
        let slot = m
            .read32(PLAYER_ARRAY_PTR)
            .wrapping_add(0x14 + player * PLAYER_STRIDE);
        let record = m.read32(m.read32(slot).wrapping_add(OBJECT_RECORD));
        let mut node = m.read32(record.wrapping_add(0x534));
        let first_id = player * SNAPSHOT_SLOTS_PER_PLAYER;
        let mut j = 0;
        while node != 0 {
            let object = m.read32(node.wrapping_add(0x10));
            let id = u32::from(m.read16(object.wrapping_add(0x94)));
            m.write32(SNAPSHOT_IDS + 4 * (first_id + j), id);
            let source = m.read32(object.wrapping_add(OBJECT_RECORD));
            let dest = SNAPSHOT_RECORDS
                + player * SNAPSHOT_SLOTS_PER_PLAYER * SNAPSHOT_RECORD_SIZE
                + j * SNAPSHOT_RECORD_SIZE;
            for word in 0..SNAPSHOT_RECORD_SIZE / 4 {
                m.write32(dest + 4 * word, m.read32(source.wrapping_add(4 * word)));
            }
            node = m.read32(node.wrapping_add(0x18));
            j += 1;
        }
        m.write32(SNAPSHOT_IDS + 4 * (first_id + j), 0);
    }
}

/// 000D8A00 cdecl(0): restores the party from the D8B10 snapshot. Same gate as D8B10. For each
/// player, the id list is walked from slot `base + restored_so_far` where `base = i * 0x14` and
/// `restored_so_far` is a running total that is NOT reset per player (lifted code, see
/// doubts: reproduced as found, likely an original defect that only shows when an earlier
/// player restored entries). Per non-zero id it calls C1950 (10 args from the 0xD8 record at
/// offsets 0x10, 0xC, 0x14, 0x1C, &scratch, 0x18, 0x40, 0x44, 0x48, 0x4C, returns an object),
/// copies the whole record over `[object + 0x7C]`, then calls
/// DB420(`[[player slot].+0x7C] + 0x304`, object).
pub fn restore_party<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    if m.read8(MODE_BYTE) != 0xa {
        return;
    }
    let players = m.read32(PLAYER_COUNT) as i32;
    if players <= 0 {
        return;
    }
    let scratch_bytes = 0x2c;
    let scratch = cpu.esp.wrapping_sub(0x0c);
    let mut restored_so_far: u32 = 0;
    let mut player_slot_offset: u32 = 0x14;
    let mut player_base: u32 = 0;
    for _player in 0..players {
        let first = player_base.wrapping_add(restored_so_far);
        let mut id_index = first;
        if m.read32(SNAPSHOT_IDS + 4 * id_index) != 0 {
            let mut record = SNAPSHOT_RECORDS + first * SNAPSHOT_RECORD_SIZE;
            loop {
                let field = |offset: u32| m.read32(record.wrapping_add(offset));
                let args = [
                    field(0x10),
                    field(0xc),
                    field(0x14),
                    field(0x1c),
                    scratch,
                    field(0x18),
                    field(0x40),
                    field(0x44),
                    field(0x48),
                    field(0x4c),
                ];
                guest_call(c, m, cpu, 0x000c_1950, 0x000d_8a91, scratch_bytes, &args);
                let object = cpu.eax;
                let dest = m.read32(object.wrapping_add(OBJECT_RECORD));
                for word in 0..SNAPSHOT_RECORD_SIZE / 4 {
                    m.write32(
                        dest.wrapping_add(4 * word),
                        m.read32(record.wrapping_add(4 * word)),
                    );
                }
                let player_object =
                    m.read32(m.read32(PLAYER_ARRAY_PTR).wrapping_add(player_slot_offset));
                let list_head = m
                    .read32(player_object.wrapping_add(OBJECT_RECORD))
                    .wrapping_add(0x304);
                guest_call(
                    c,
                    m,
                    cpu,
                    0x000d_b420,
                    0x000d_8abb,
                    0x54,
                    &[list_head, object],
                );
                restored_so_far = restored_so_far.wrapping_add(1);
                record = record.wrapping_add(SNAPSHOT_RECORD_SIZE);
                id_index = id_index.wrapping_add(1);
                if m.read32(SNAPSHOT_IDS + 4 * id_index) == 0 {
                    break;
                }
            }
        }
        player_slot_offset = player_slot_offset.wrapping_add(PLAYER_STRIDE);
        player_base = player_base.wrapping_add(SNAPSHOT_SLOTS_PER_PLAYER);
    }
}
