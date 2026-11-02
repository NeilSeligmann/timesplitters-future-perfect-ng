//! Small callees of the level hooks decoded in T1846 (batch 6). Static reading of `build/default.xbe` (SHA-256
//! `3cfd001a...1816bc`), no xemu run. Control flow and constants are MEASURED; names are INFERRED.
//! Detail: `docs/t-story-event-callees-t1846.md`.

/// `0xE0080` (called from the story hooks, no arguments): a rate limited sound. When the clock `[0x7DE318]` has
/// reached the stamp `[0x737C5C]`, sound `0xCFB` is played through `0x46A00(0xCFB, 0.1, 0.95, 1.05)` and the stamp
/// becomes `clock + 10`. Returns the sound call when it fires.
pub const THROTTLED_SOUND: ThrottledSound = ThrottledSound {
    sound_id: 0xCFB,
    arg_a: 0.1,
    arg_b: 0.95,
    arg_c: 1.05,
    cooldown: 10,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThrottledSound {
    pub sound_id: u32,
    /// The three floats pushed for `0x46A00` (`0x3DCCCCCD`, `0x3F733333`, `0x3F866666`); roles INFERRED (volume and two pitches).
    pub arg_a: f32,
    pub arg_b: f32,
    pub arg_c: f32,
    pub cooldown: i32,
}

pub fn throttled_sound(clock: i32, stamp: &mut i32) -> Option<ThrottledSound> {
    if clock < *stamp {
        return None;
    }
    *stamp = clock.wrapping_add(THROTTLED_SOUND.cooldown);
    Some(THROTTLED_SOUND)
}

/// One entry of the 0x20 byte table `[0x78CC24]` (count `[0x78CC28]`): the dword `+8` (flags) and the byte `+0x1C` (group).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupEntry {
    pub flags: u32,
    pub group: u8,
}

/// `0x33AD10` (register convention: esi = entry): true when another entry (a different table slot) of the same group
/// has a different flag dword.
pub fn group_has_mixed_flags(entries: &[GroupEntry], index: usize) -> bool {
    let me = entries[index];
    entries
        .iter()
        .enumerate()
        .any(|(other, entry)| other != index && entry.group == me.group && entry.flags != me.flags)
}

/// `0x33AD50`: the highest group number among entries with group at least 1, flag bits `0x180` set and
/// [`group_has_mixed_flags`] true (0 when none). The original records each group once in an 8 dword stack array, so
/// a group above 8 would write past it (latent, not reproduced: the maximum is taken directly).
pub fn highest_mixed_group(entries: &[GroupEntry]) -> u32 {
    let mut best = 0;
    for (index, entry) in entries.iter().enumerate() {
        if entry.group >= 1 && entry.flags & 0x180 != 0 && group_has_mixed_flags(entries, index) {
            best = best.max(u32::from(entry.group));
        }
    }
    best
}

/// `0x873F0(index, min_out, max_out)`: copies two vec3 from entry `index` (stride `0xB4`) of the table `[0x708788]`:
/// `+0x10..+0x18` and `+0x1C..+0x24` (a region box: INFERRED min and max).
pub fn region_box(entry_words: &[u32; 0xB4 / 4]) -> ([f32; 3], [f32; 3]) {
    let f = |at: usize| f32::from_bits(entry_words[at / 4]);
    ([f(0x10), f(0x14), f(0x18)], [f(0x1C), f(0x20), f(0x24)])
}

/// `0x873C0(...)`: returns 0 when the table `[0x708788]` is null, else tail calls the tree query `0x87310`
/// (`0x157440` over root `0x702E00` with filter `0x87260`; it clears bit `0x10` of the byte at entry `+0xB2` for each
/// hit and stores the hit indices, wrapping a ring when the output is full). The tree walk itself is not modelled.
pub fn region_query_available(table_present: bool) -> bool {
    table_present
}

// ---- Batch 7 (second pass): small callees of the group, zone and sweep code. MEASURED shape, INFERRED names. ----

/// `0x366130(v, a, b)` (cdecl, three pointers): rescales vector `v` in place by `1.0 / divisor`, where the divisor is
/// `|a.y * b.z|` when `b.x == 0`, else `|a.y * b.x|` when `b.z == 0`, else the length of `v`. The scale constant is the
/// float at `0x475C78` (`1.0`, MEASURED). Note the original divides by a zero divisor without a guard (inf/NaN results).
pub fn rescale_vector(v: &mut [f32; 3], a: [f32; 3], b: [f32; 3]) {
    rescale_vector_ordered(v, a, b, false);
}

/// [`rescale_vector`] as the compiler inlined it into `0x369940` (the slide of the pieces): the extended precision
/// sum of squares runs `(z*z + y*y) + x*x` there, so a last bit of the f64 stand-in can differ. `reversed` selects it.
pub fn rescale_vector_ordered(v: &mut [f32; 3], a: [f32; 3], b: [f32; 3], reversed: bool) {
    let scale = if b[0] == 0.0 {
        1.0 / (a[1] * b[2]).abs()
    } else if b[2] == 0.0 {
        1.0 / (a[1] * b[0]).abs()
    } else {
        // x87 path in the original: the sum of squares, the root and `1.0 / root` all stay in extended precision and
        // round once to f32 (batch 8 correction: batch 7 rounded the root to f32 first and summed z first;
        // the image sums (x*x + y*y) + z*z). f64 stands in for the 80 bit registers (control word 0x27F).
        let square = |c: f32| f64::from(c) * f64::from(c);
        let sum = if reversed {
            (square(v[2]) + square(v[1])) + square(v[0])
        } else {
            (square(v[0]) + square(v[1])) + square(v[2])
        };
        (1.0 / sum.sqrt()) as f32
    };
    v.iter_mut().for_each(|c| *c *= scale);
}

/// One 0xB4 byte entry of the region table `[0x708788]` reduced to the fields the group code reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionRow {
    /// Dword `+0xAC`, compared by `0x33ADF0`'s key search and used as the `0x33F770` maximum.
    pub key: u32,
}

/// The 0x220 byte records behind `0x33ADF0`'s result (base `[obj+0x138]`, stride `0x220`), reduced to the fields the
/// callers read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupRecord {
    /// dword `+0`: region index into the table `[0x78CD30]` (stride `0xB4`).
    pub region_index: u32,
    /// dword `+0x4C` (copied into `[record+8]` by `0x33AE50`).
    pub word_4c: u32,
    /// dword `+0x50` and `+0x88` (both must be non zero for `0x33F770`).
    pub word_50: u32,
    pub word_88: u32,
    /// byte `+4` (must be 1 for `0x33F770`).
    pub kind_byte: u8,
}

/// `0x33ADF0(key in ebx)`: finds the row of the level object's table (`+0x130`, count `+0x12C`, stride `0xB4`) whose
/// dword `+0xAC` equals `key`, and returns the record `+0xA4` of that row indexes in the 0x220 byte array. `None` is
/// the original's 0. `rows` pairs each row's `+0xAC` key with its `+0xA4` record index.
pub fn lookup_group_record(rows: &[(u32, u32)], key: u32) -> Option<usize> {
    rows.iter()
        .find(|row| row.0 == key)
        .map(|row| row.1 as usize)
}

/// `0x33F770()`: the largest region key (`+0xAC` of table `[0x78CD30]` entry `record.region_index`) over the records for
/// keys `1..=count` (`[0x78CD34]`) that pass `+0x50 != 0`, `+0x88 != 0` and byte `+4 == 1`. 0 when none. `record_of`
/// is `0x33ADF0` (`None` skips the key), `region_key` reads `+0xAC` of a `[0x78CD30]` entry.
pub fn max_region_key(
    count: u32,
    record_of: &dyn Fn(u32) -> Option<GroupRecord>,
    region_key: &dyn Fn(u32) -> u32,
) -> u32 {
    let mut best = 0u32;
    for key in 1..=count {
        let Some(record) = record_of(key) else {
            continue;
        };
        if record.word_50 == 0 || record.word_88 == 0 || record.kind_byte != 1 {
            continue;
        }
        let candidate = region_key(record.region_index);
        // The original compares signed (`jle`).
        if (candidate as i32) > (best as i32) {
            best = candidate;
        }
    }
    best
}

/// `0x33C300(group in esi)`: true when the group has an entry with flag `0x80` and an entry with flag `0x100`
/// (table `[0x78CC24]`, dword `+8`, group byte `+0x1C`).
pub fn group_has_both_flag_kinds(entries: &[GroupEntry], group: u8) -> bool {
    let in_group = || entries.iter().filter(|entry| entry.group == group);
    in_group().any(|entry| entry.flags & 0x80 != 0)
        && in_group().any(|entry| entry.flags & 0x100 != 0)
}

/// `0x33FCD0` array sizing: `max(0x33AD50(), 0x33F770())` groups.
pub fn group_array_size(entries: &[GroupEntry], max_key: u32) -> u32 {
    highest_mixed_group(entries).max(max_key)
}

/// One filled slot of `0x33AE50`'s member list (12 bytes each at `[record]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemberSlot {
    /// Result of `0x23FF20(word + 1)`.
    pub object: u32,
    /// The member pointer from the caller's list.
    pub member: u32,
    /// Index of the member in the region table `[0x78CD30]` (stride `0xB4`), or -1.
    pub region: i32,
}

/// Result of `0x33AE50`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberList {
    /// `[record+8]`: dword `+0x4C` of the `0x33ADF0` record.
    pub word_8: u32,
    /// Slots indexed like the input (skipped members keep `None`: the original advances the 12 byte offset anyway and
    /// leaves the slot untouched).
    pub slots: Vec<Option<MemberSlot>>,
    /// The `out` pointer array: one `0x23FF20` result per processed member, in order.
    pub objects: Vec<u32>,
    /// `[record+4]`: the member count.
    pub count: usize,
}

/// `0x33AE50(list, out, count, index)`: builds the member list of group `index` (1 based record in `[0x7789F8]`).
/// `members` pairs each member pointer with its signed word `+0xC` (a negative word is skipped), `region_pointers`
/// are the entry addresses of table `[0x78CD30]` (pointer compare), `object_of` is `0x23FF20`, `word_4c` is dword
/// `+0x4C` of the `0x33ADF0` record (the original dereferences a null record when the lookup fails).
pub fn build_member_list(
    members: &[(u32, i16)],
    region_pointers: &[u32],
    object_of: &dyn Fn(i32) -> u32,
    word_4c: u32,
) -> MemberList {
    let mut slots = Vec::with_capacity(members.len());
    let mut objects = Vec::new();
    for &(member, word) in members {
        if word < 0 {
            slots.push(None);
            continue;
        }
        let object = object_of(i32::from(word) + 1);
        let region = region_pointers
            .iter()
            .position(|&entry| entry == member)
            .map_or(-1, |index| index as i32);
        slots.push(Some(MemberSlot {
            object,
            member,
            region,
        }));
        objects.push(object);
    }
    MemberList {
        word_8: word_4c,
        slots,
        objects,
        count: members.len(),
    }
}

/// `0x33B6C0(entry, id)`: finds the 0x20 byte row of table `[0x78CCF4]` (count `[0x78CCF8]`) whose first dword
/// `v` matches a typed id: for type 0 (word `+8` of `entry`) when `v - 1` is even and `(v - 1) / 2 == id`, for type 1
/// when `v` is even and `v / 2 == id`; any other type never matches. Returns the row index (original: its address, 0 for none).
pub fn find_typed_row(first_words: &[i32], entry_type: i16, id: i32) -> Option<usize> {
    first_words.iter().position(|&v| match entry_type {
        0 => (v - 1) % 2 == 0 && id == (v - 1) >> 1,
        1 => v % 2 == 0 && id == v >> 1,
        _ => false,
    })
}

/// `0x87590(box, ring, len)`: copies a six float box to a query block with its second float forced to the constant at
/// `0x478618`, then tail calls `0x87310`. Returns the query block.
///
/// CORRECTION (batch 8, MEASURED by reading the constant): `[0x478618]` is `-3.4028235e38` (`0xFF7FFFFF`, minus
/// `f32::MAX`), not `1.0` as batch 7 recorded: the query is unbounded downward in y.
pub fn region_query_block(source: [f32; 6]) -> [f32; 6] {
    [
        source[0],
        f32::from_bits(0xFF7F_FFFF),
        source[2],
        source[3],
        source[4],
        source[5],
    ]
}

/// Result of the hit loop of `0x87310`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionHits {
    /// Entry indices written to the caller ring, in hit order. Storing stops once `ring_len` are written (the
    /// original zeroes the test bit; it does not wrap, which corrects the T1846 batch 6 note).
    pub stored: Vec<u32>,
    /// Entry indices whose byte `+0xB2` lost bit `0x10` (every hit).
    pub cleared: Vec<u32>,
}

/// `0x87310` after `0x157440`: for each hit value `h` (a word) entry `h >> 1`'s byte `+0xB2` loses bit `0x10`; the
/// entry is stored when `h & 1` is set until the ring is full. Returns the stored count through `stored.len()`.
pub fn process_region_hits(hits: &[u16], ring_len: usize) -> RegionHits {
    let mut stored = Vec::new();
    let mut cleared = Vec::new();
    let mut store_enabled = true;
    for &hit in hits {
        let entry = u32::from(hit >> 1);
        cleared.push(entry);
        if store_enabled && hit & 1 != 0 {
            stored.push(entry);
            if stored.len() == ring_len {
                store_enabled = false;
            }
        }
    }
    RegionHits { stored, cleared }
}

/// An object of the `0xA3050` lists, reduced to the fields it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SweepObject {
    /// dword `+0x28` flags: `0x60` selects, bit 31 and bit `0x100` reject.
    pub flags_28: u32,
    /// dword `+0x20`, tested against the skip mask.
    pub flags_20: u32,
    /// dword `+4`, the first argument of `0x92360`.
    pub target: u32,
}

/// `0xA3050(index, ..., skip_mask at arg6, ...)`: sweep a shape through the head object of list `index` of `[0x7B9784]`
/// and the member objects (`[0x7B5F00 + index*4]`, count `[0x7B54E0 + index*4]`). `index > list_count`
/// (`[0x7B975C]`) returns 0. The head object is swept when its `+0x28 & 0x60` is set and its raw result is kept;
/// each member with a target, `+0x28` positive, `& 0x60`, bit `0x100` clear and `+0x20 & skip_mask == 0` is swept and
/// any non zero answer ORs 1 into the result. `sweep` is `0x92360(target, ...)`; its other arguments are the
/// caller's (index, args 2 to 5 and 7, a zero, 1).
pub fn sweep_objects(
    index: i32,
    list_count: i32,
    head: SweepObject,
    members: &[SweepObject],
    skip_mask: u32,
    sweep: &mut dyn FnMut(u32) -> u32,
) -> u32 {
    if index > list_count {
        return 0;
    }
    let mut result = if head.flags_28 & 0x60 != 0 {
        sweep(head.target)
    } else {
        0
    };
    for member in members {
        let usable = member.target != 0
            && (member.flags_28 as i32) >= 0
            && member.flags_28 & 0x60 != 0
            && member.flags_28 & 0x100 == 0
            && member.flags_20 & skip_mask == 0;
        if usable && sweep(member.target) != 0 {
            result |= 1;
        }
    }
    result
}
