//! T1846 batch 8: group path slot update (`0x33BFA0`) and everything it needs, as a pure model (no I/O).
//!
//! Original addresses (all names INFERRED, behaviour MEASURED against the original under the Unicorn oracle):
//!
//! | address | role |
//! |---|---|
//! | `0x33BFA0(area_id)` | [`update_area_paths`]: for every 0x10 byte path entry rebuilds the ten word slots of its typed row |
//! | `0x33B8E0(out, area)` | [`collect_member_values`]: runs the member list `[0x7789F8][area]` through `0x33B5D0` |
//! | `0x33B5D0` (edi = member) | [`member_value`]: object readiness, then dispatch on the member kind (table `0x52EDA8`) |
//! | `0x33B100` / `0x33B210` | kind 1,2 / kind 3,4 handlers, the `out[acc]` word only |
//! | `0x33B750(area, typed, out, entry)` | [`random_slot_fill`]: random neighbour choice per slot |
//! | `0x33BDC0(ids, entry, typed, n)` | [`apply_ids_to_slots`]: id to key, neighbour remap, replicate and pad |
//! | `0x9CC40(key)` / `0x9A3C0(key)` | [`IdTables::row_index`] / [`IdTables::t34_index`]: binary search by key |
//! | `0x9CCA0(id)` | [`IdTables::row_ref`]: id to row |
//! | `0x9D9E0` -> `0x9D830` | [`IdTables::neighbors`]: neighbour rows of a row through the edge groups |
//! | `0x153B40` | rng, a [`PathHooks::rng`] draw ([`Lcg`] is the original generator) |
//!
//! Globals: `[0x78CCEC]` path entries (stride 0x10), `[0x78CCF0]` entry count, `[0x78CCF4]` typed rows (stride
//! 0x20), `[0x78CCF8]` typed row count, `[0x78CCE4]` area rows (stride 0x60), `[0x7789F8]` member list pointers,
//! `[0x765E44]` object context, `[0x715F2C]`/`[0x7B6664]` main id rows (stride 0x20, count), `[0x715F34]`/`[0x7B65C0]`
//! t34 id rows, `[0x715F38]` t38 id rows, `[0x715F30]` edges (stride 0x14), `[0x7B6770]`/`[0x7B666C]` groups.
//!
//! Not ported (needs the physics / spatial system): the member handlers of kinds 0 and 7 (`0x33FB50`, uses
//! `0x88C60`, `0x9C0E0`) and kinds 8 and 9 (`0x33B290`, uses `0x3384F0`, `0x2F7990`). They are delegated to
//! [`PathHooks::deep_member`]. Only their `out[acc]` word matters to `0x33BFA0` because the position and object
//! arrays are passed as null by `0x33B8E0`.

use crate::story_events::callees_t1846::find_typed_row;

/// Word slots per typed row and per area section.
pub const SLOTS: usize = 10;
/// Ids from this value are rows of the t34 table (`id - 0x1F40`).
pub const T34_BASE: i32 = 0x1F40;
/// Ids from this value are rows of the t38 table (`id - 0x209E`).
pub const T38_BASE: i32 = 0x209E;
/// First id past the t38 table.
pub const T38_END: i32 = 0x20B2;
/// Keys from this value are searched in the t34 table by `0x9CC40`.
pub const T34_KEY_MIN: i32 = 0x2329;
/// Edge flag required for a neighbour edge (`0x10000`), edge flag that blocks the reverse direction (`2`).
pub const EDGE_USABLE: u32 = 0x10000;
/// Edge flag that forbids walking an edge from its second endpoint.
pub const EDGE_ONE_WAY: u32 = 2;
/// Row flags (`+4`) that mark an alias row whose word `+8` is the id of the real row.
pub const ROW_ALIAS_FLAGS: u32 = 0x40_0800;
/// Mask added by `0x9D830` (`0x20`) and neighbour buffer size used by the two callers (`0x14`).
pub const NEIGHBOR_MASK: u32 = 0x20;
pub const NEIGHBOR_MAX: usize = 0x14;

/// The original rng (`0x153B40`): 64 bit LCG `state = state * 0x343FD + 0x269EC3`, returning the low dword of the
/// new state. State lives at `[0x4E7950]` (low) / `[0x4E7954]` (high), draw counter `[0x7497C8]`. MEASURED.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lcg {
    pub state: u64,
    pub draws: u32,
}

impl Lcg {
    pub fn next(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(0x343FD).wrapping_add(0x26_9EC3);
        self.draws = self.draws.wrapping_add(1);
        self.state as u32
    }
}

/// 0x20 byte id row (tables `[0x715F2C]`, `[0x715F34]`, `[0x715F38]`). Offsets: `+0` key, `+4` flags, `+8` word.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IdRow {
    pub key: i32,
    pub flags: u32,
    pub word_8: i16,
}

/// 0x14 byte edge record at `[0x715F30]`: `+0` flags, `+4` first row, `+8` second row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Edge {
    pub flags: u32,
    pub a: i32,
    pub b: i32,
}

/// Group record (pointer table `[0x7B6770]`). Dwords: `+0` skipped count, `+4` count B, `+8` count C, header to
/// `+0x18`, then `skipped` dwords (never read), the B edge indices, the C edge indices.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    pub skipped: u32,
    pub list_b: Vec<u32>,
    pub list_c: Vec<u32>,
}

/// One neighbour found by `0x9D830`: the other row and the edge index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Neighbor {
    pub row: i32,
    pub edge: u32,
}

/// Id tables and the edge graph.
#[derive(Clone, Debug, Default)]
pub struct IdTables {
    /// `[0x715F2C]`, count `[0x7B6664]` (must be sorted by key for the binary search).
    pub main: Vec<IdRow>,
    /// `[0x715F34]`, count `[0x7B65C0]` (sorted by key).
    pub t34: Vec<IdRow>,
    /// `[0x715F38]` (ids `0x209E..0x20B2`, no count global read by this code).
    pub t38: Vec<IdRow>,
    /// `[0x715F30]`.
    pub edges: Vec<Edge>,
    /// `[0x7B6770]`, count `[0x7B666C]`.
    pub groups: Vec<Group>,
}

/// Shared binary search of `0x9CC40` / `0x9A3C0`: index of `key` or -1. MEASURED.
fn bsearch(keys: &[i32], count: i32, key: i32) -> i32 {
    let key_at = |i: i32| {
        usize::try_from(i)
            .ok()
            .and_then(|i| keys.get(i))
            .copied()
            .unwrap_or(0)
    };
    let mut high = count;
    let mut low = 0i32;
    let mut mid = count >> 1;
    if mid == -1 {
        return -1;
    }
    loop {
        let probe = key_at(mid);
        if key < probe {
            high = mid;
        } else if key > probe {
            low = mid;
        } else {
            return mid;
        }
        let previous = mid;
        mid = ((high - low) >> 1) + low;
        if mid == previous {
            return -1;
        }
    }
}

impl IdTables {
    fn row_at(table: &[IdRow], index: i32) -> Option<&IdRow> {
        usize::try_from(index).ok().and_then(|i| table.get(i))
    }

    /// `0x9A3C0(key)`: binary search of the t34 table, result `index + 0x1F40` or -1. MEASURED.
    pub fn t34_index(&self, key: i32) -> i32 {
        let keys: Vec<i32> = self.t34.iter().map(|row| row.key).collect();
        let found = bsearch(&keys, self.t34.len() as i32, key);
        if found == -1 { -1 } else { found + T34_BASE }
    }

    /// `0x9CC40(key)`: row index of a key, -1 when absent. Keys below `0x2329` search the main table, others the
    /// t34 table (index plus `0x1F40`). MEASURED.
    pub fn row_index(&self, key: i32) -> i32 {
        if key >= T34_KEY_MIN {
            return self.t34_index(key);
        }
        let keys: Vec<i32> = self.main.iter().map(|row| row.key).collect();
        bsearch(&keys, self.main.len() as i32, key)
    }

    /// Inline row-id to key decode of `0x33B750`/`0x33BDC0`: t34 for `0x1F40..0x209E`, t38 for `0x209E..0x20B2`,
    /// main for `0..count`, else -1. Out of range rows of the first two tables read zero memory (key 0).
    /// MEASURED (golden worlds keep the tables in zero filled memory).
    pub fn key_of_row(&self, id: i32) -> i32 {
        if (T34_BASE..T38_BASE).contains(&id) {
            Self::row_at(&self.t34, id - T34_BASE).map_or(0, |row| row.key)
        } else if (T38_BASE..T38_END).contains(&id) {
            Self::row_at(&self.t38, id - T38_BASE).map_or(0, |row| row.key)
        } else if id >= 0 && (id as usize) < self.main.len() {
            self.main[id as usize].key
        } else {
            -1
        }
    }

    /// Key read by the random branch of `0x33B750` for a row id the checks did not validate (a different draw can
    /// land outside the table): zero memory outside the table, the key inside it. MEASURED.
    fn key_unchecked(table: &[IdRow], index: i32) -> i32 {
        Self::row_at(table, index).map_or(0, |row| row.key)
    }

    /// `0x9CCA0(id)`: the id row of `id` or `None` (original returns 0). Ids `0x3E9..0x2329` search the main table,
    /// ids from `0x2329` the t34 table, lower ids give none. MEASURED.
    pub fn row_ref(&self, id: i32) -> Option<&IdRow> {
        if id < 0x3E9 {
            return None;
        }
        let index = self.row_index(id);
        if index == -1 {
            return None;
        }
        if id < T34_KEY_MIN {
            Self::row_at(&self.main, index)
        } else {
            Self::row_at(&self.t34, index - T34_BASE)
        }
    }

    /// `0x9D9E0(row, ..., max, mask, flag)` = `0x9D830(row, out, 0, max, mask, 0)`: neighbours of main row `row`.
    ///
    /// Alias rows (flags `& 0x400800`) are replaced by the row `0x9CCA0(word_8)` (a missing one reads zero memory,
    /// group 0). The group is `word_8` of the effective row; it must be in `0..groups`. Walks the B list then the C
    /// list. An edge is skipped when `mask & flags`, or when `!include_unusable` and `flags & 0x10000 == 0`. An edge
    /// whose first row is `row` yields its second row; else one whose second row is `row` yields the first unless
    /// `flags & 2`. Stops once `max` neighbours are stored (checked after each non skipped edge). MEASURED.
    pub fn neighbors(
        &self,
        row: i32,
        max: usize,
        mask: u32,
        include_unusable: bool,
    ) -> Vec<Neighbor> {
        let mut found = Vec::new();
        let Some(base) = Self::row_at(&self.main, row) else {
            return found;
        };
        let mask = mask | NEIGHBOR_MASK;
        let group_id = if base.flags & ROW_ALIAS_FLAGS != 0 {
            self.row_ref(i32::from(base.word_8))
                .map_or(0, |alias| i32::from(alias.word_8))
        } else {
            i32::from(base.word_8)
        };
        let Some(group) = usize::try_from(group_id)
            .ok()
            .and_then(|g| self.groups.get(g))
        else {
            return found;
        };
        for &edge_index in group.list_b.iter().chain(group.list_c.iter()) {
            let edge = self
                .edges
                .get(edge_index as usize)
                .copied()
                .unwrap_or_default();
            if mask & edge.flags != 0 || (!include_unusable && edge.flags & EDGE_USABLE == 0) {
                continue;
            }
            if edge.a == row {
                found.push(Neighbor {
                    row: edge.b,
                    edge: edge_index,
                });
            } else if edge.b == row && edge.flags & EDGE_ONE_WAY == 0 {
                found.push(Neighbor {
                    row: edge.a,
                    edge: edge_index,
                });
            }
            if found.len() >= max {
                break;
            }
        }
        found
    }

    /// Neighbour rows as the plain list `0x33B750`/`0x33BDC0` use (`0x9D9E0` with max 0x14, mask 0, flag 0).
    pub fn neighbor_rows(&self, row: i32) -> Vec<i32> {
        self.neighbors(row, NEIGHBOR_MAX, 0, false)
            .into_iter()
            .map(|n| n.row)
            .collect()
    }
}

/// 0x10 byte path entry `[0x78CCEC]`: `+0` id, `+8` kind word (0 or 1), `+0xA` slot count word.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PathEntry {
    pub id: i32,
    pub kind: i16,
    pub slot_count: i16,
}

/// 0x20 byte typed row `[0x78CCF4]`: `+0` typed id, `+4` kind word, `+0xC..0x20` ten word slots (keys).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TypedRow {
    pub first: i32,
    pub kind: i16,
    pub slots: [i16; SLOTS],
}

/// 0x60 byte area row `[0x78CCE4]`: `+0x14` state (3 elsewhere), `+0x20` and `+0x40` sections of ten words.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AreaRow {
    pub section_20: [i16; SLOTS],
    pub section_40: [i16; SLOTS],
}

/// Object table record (`[0x765E44]+0x84`, stride 0x28; the original pointer is record `+4`): `id`, path index
/// (`+0xC`), callback (`+0x14`) and state (`+0x1C`, value 1 = ready).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectRec {
    pub id: i32,
    pub path: i32,
    pub state: i32,
    /// INFERRED: whether the callback makes the object ready. The original calls a function pointer.
    pub callback_makes_ready: bool,
}

/// Object path record (`[0x765E44]+0xB0`, stride 0x58): `+4` count, `+8` ten ids, `+0x30` ten blocked dwords.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PathRec {
    pub count: i32,
    pub ids: [i32; SLOTS],
    pub blocked: [i32; SLOTS],
}

/// Member (0xC bytes in the lists of `[0x7789F8]`): `+0` game object (flags `+0x28`, link `+0x7C` -> value `+4`),
/// `+4` pointer to a descriptor whose `+8` is the kind, `+8` object id.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Member {
    pub kind: i32,
    pub object_id: i32,
    pub game_flags_28: u32,
    pub game_link_value: i32,
}

/// Everything the group path code reads or writes.
#[derive(Clone, Debug, Default)]
pub struct PathWorld {
    pub entries: Vec<PathEntry>,
    pub typed_rows: Vec<TypedRow>,
    pub area_rows: Vec<AreaRow>,
    /// `[0x7789F8][area]`: `None` for a null list.
    pub member_lists: Vec<Option<Vec<Member>>>,
    pub objects: Vec<ObjectRec>,
    pub paths: Vec<PathRec>,
    pub ids: IdTables,
}

/// External behaviour the original reaches through code pointers or unported systems.
pub trait PathHooks {
    /// `0x153B40`.
    fn rng(&mut self) -> u32;
    /// The object callback `[obj+0x10]` called by `0x33B5D0` before the second readiness test.
    fn refresh_object(&mut self, object: &mut ObjectRec) {
        if object.callback_makes_ready {
            object.state = 1;
        }
    }
    /// Kinds 0, 7 (`0x33FB50`) and 8, 9 (`0x33B290`): pushes the `out[acc]` words. Not ported.
    fn deep_member(&mut self, member: &mut Member, out: &mut Vec<i32>);
}

/// Unsupported or inconsistent input (the original would fault or read null).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathError {
    /// `0x33B6C0` found no typed row for the entry (the original writes through null).
    MissingTypedRow { entry: usize },
    /// `0x3383A0` found no object for a member (the original calls through null).
    MissingObject { object_id: i32 },
    /// Path record index outside the table.
    MissingPath { index: i32 },
}

fn find_object(world: &PathWorld, id: i32) -> Option<usize> {
    world.objects.iter().position(|object| object.id == id)
}

/// `0x33B100` (kinds 1, 2): for every unblocked id of the object's path record pushes `word_8(row) - 0x3E9`.
/// A missing row reads zero memory (word 0). Returns the new count. MEASURED.
fn handler_path_rows(
    world: &PathWorld,
    member: &Member,
    out: &mut Vec<i32>,
) -> Result<(), PathError> {
    let object = find_object(world, member.object_id).ok_or(PathError::MissingObject {
        object_id: member.object_id,
    })?;
    let index = world.objects[object].path;
    let path = usize::try_from(index)
        .ok()
        .and_then(|i| world.paths.get(i))
        .ok_or(PathError::MissingPath { index })?;
    for j in 0..usize::try_from(path.count).unwrap_or(0) {
        if path.blocked[j] == 0 {
            let word = world
                .ids
                .row_ref(path.ids[j])
                .map_or(0, |row| i32::from(row.word_8));
            out.push(word - 0x3E9);
        }
    }
    Ok(())
}

/// `0x33B210` (kinds 3, 4): clears bit `0x10` of the game object flags and pushes its link value. MEASURED.
fn handler_game_link(member: &mut Member, out: &mut Vec<i32>) {
    member.game_flags_28 &= !0x10;
    out.push(member.game_link_value);
}

/// `0x33B5D0` for one member (`acc` = `out.len()`). Looks the object up, returns when it is ready (state 1), else
/// calls its callback and re-tests, then dispatches on the member kind: 1,2 -> `0x33B100`, 3,4 -> `0x33B210`,
/// 0,7,8,9 -> [`PathHooks::deep_member`], any other kind adds nothing. MEASURED (kinds 0,7,8,9 delegated).
pub fn member_value(
    world: &mut PathWorld,
    hooks: &mut dyn PathHooks,
    list: usize,
    slot: usize,
    out: &mut Vec<i32>,
) -> Result<(), PathError> {
    let mut member = world.member_lists[list].as_ref().expect("list")[slot];
    let object = find_object(world, member.object_id).ok_or(PathError::MissingObject {
        object_id: member.object_id,
    })?;
    if world.objects[object].state == 1 {
        return Ok(());
    }
    hooks.refresh_object(&mut world.objects[object]);
    if world.objects[object].state == 1 {
        return Ok(());
    }
    match member.kind {
        1 | 2 => handler_path_rows(world, &member, out)?,
        3 | 4 => handler_game_link(&mut member, out),
        0 | 7 | 8 | 9 => hooks.deep_member(&mut member, out),
        _ => {}
    }
    world.member_lists[list].as_mut().expect("list")[slot] = member;
    Ok(())
}

/// `0x33B8E0(out, area)`: collects the member values of `[0x7789F8][area]` (empty when the list is null). MEASURED.
pub fn collect_member_values(
    world: &mut PathWorld,
    hooks: &mut dyn PathHooks,
    area: i32,
) -> Result<Vec<i32>, PathError> {
    let mut out = Vec::new();
    let Some(list) = usize::try_from(area)
        .ok()
        .filter(|&a| world.member_lists.get(a).is_some_and(Option::is_some))
    else {
        return Ok(out);
    };
    let count = world.member_lists[list].as_ref().map_or(0, Vec::len);
    for slot in 0..count {
        member_value(world, hooks, list, slot, &mut out)?;
    }
    Ok(out)
}

/// The random pick of `0x33B750`: every use of `buf[rng % n]` in the original draws again, so the draw count per
/// branch is part of the contract. Returns the key to store (low 16 bits are kept). MEASURED.
fn pick_key(ids: &IdTables, hooks: &mut dyn PathHooks, buf: &[i32]) -> i32 {
    let n = buf.len() as u32;
    let sel = |hooks: &mut dyn PathHooks| buf[(hooks.rng() % n) as usize];
    if sel(hooks) >= T34_BASE && sel(hooks) < T38_BASE {
        let row = sel(hooks);
        return IdTables::key_unchecked(&ids.t34, row.wrapping_sub(T34_BASE));
    }
    if sel(hooks) >= T38_BASE && sel(hooks) < T38_END {
        let row = sel(hooks);
        return IdTables::key_unchecked(&ids.t38, row.wrapping_sub(T38_BASE));
    }
    if sel(hooks) < 0 {
        return -1;
    }
    if sel(hooks) >= ids.main.len() as i32 {
        return -1;
    }
    let row = sel(hooks);
    IdTables::key_unchecked(&ids.main, row)
}

/// `0x33B750(area, typed, out, entry)` (register args eax = area, edx = typed row; stack out, entry): for each of
/// the ten slots with a positive word: kind 0 loads the word from the area row section `+0x40`, kind 1 from
/// `+0x20`, then picks a random neighbour of the slot's row (when it has any) as the new key, and appends the row
/// index of the final key to the output. Returns the output count. MEASURED.
pub fn random_slot_fill(
    world: &mut PathWorld,
    hooks: &mut dyn PathHooks,
    area: usize,
    typed: usize,
    entry: PathEntry,
) -> Vec<i32> {
    let mut out = Vec::new();
    let area_row = world.area_rows.get(area).copied().unwrap_or_default();
    for slot in 0..SLOTS {
        if world.typed_rows[typed].slots[slot] <= 0 {
            continue;
        }
        match entry.kind {
            0 => world.typed_rows[typed].slots[slot] = area_row.section_40[slot],
            1 => world.typed_rows[typed].slots[slot] = area_row.section_20[slot],
            _ => {}
        }
        let row = world
            .ids
            .row_index(i32::from(world.typed_rows[typed].slots[slot]));
        let buf = world.ids.neighbor_rows(row);
        if !buf.is_empty() {
            let key = pick_key(&world.ids, hooks, &buf);
            world.typed_rows[typed].slots[slot] = key as i16;
        }
        out.push(
            world
                .ids
                .row_index(i32::from(world.typed_rows[typed].slots[slot])),
        );
    }
    out
}

/// `0x33BDC0(ids, entry, typed, n)`: for `i < n`: slot `i` = key of row `ids[i]` (-1 when invalid); for kind 0
/// entries then replaces it by the key of a neighbour of that key's row, walking the neighbours in order (index
/// `i` while `i < count`, else a counter that wraps to 0 at `count`); finally, when `n < slot_count`, repeats the
/// first slots forward (`slot[j] = slot[j - n]`) and pads the rest up to ten with -1. MEASURED. (The helper tool's
/// "state conflict at the ret" is the `and esp, -8` frame: esp is restored from ebp, not by pops.)
pub fn apply_ids_to_slots(
    ids: &IdTables,
    entry: PathEntry,
    typed: &mut TypedRow,
    rows: &[i32],
    n: usize,
) {
    let mut wrap = 0usize;
    for i in 0..n {
        let key = ids.key_of_row(rows[i]) as i16;
        typed.slots[i] = key;
        if entry.kind != 0 {
            continue;
        }
        let row = ids.row_index(i32::from(key));
        let buf = ids.neighbor_rows(row);
        let count = buf.len();
        if wrap >= count {
            wrap = 0;
        }
        let current = if i < count || count == 0 {
            i
        } else {
            let taken = wrap;
            wrap += 1;
            taken
        };
        if count != 0 {
            typed.slots[i] = ids.key_of_row(buf[current]) as i16;
        }
    }
    let slot_count = usize::try_from(entry.slot_count).unwrap_or(0).min(SLOTS);
    if n < slot_count {
        for j in n..slot_count {
            typed.slots[j] = typed.slots[j - n];
        }
        for slot in typed.slots.iter_mut().skip(slot_count) {
            *slot = -1;
        }
    }
}

/// `0x33BFA0(area_id)`: for every path entry finds its typed row (`0x33B6C0`), collects member values for area
/// `area_id - 1` (`0x33B8E0`), falls back to [`random_slot_fill`] when there are none, copies the entry kind (0 or
/// 1) into the typed row, caps the count at ten and runs [`apply_ids_to_slots`]. MEASURED.
pub fn update_area_paths(
    world: &mut PathWorld,
    hooks: &mut dyn PathHooks,
    area_id: i32,
) -> Result<(), PathError> {
    let area = area_id - 1;
    for index in 0..world.entries.len() {
        let entry = world.entries[index];
        let firsts: Vec<i32> = world.typed_rows.iter().map(|row| row.first).collect();
        let typed = find_typed_row(&firsts, entry.kind, area_id)
            .ok_or(PathError::MissingTypedRow { entry: index })?;
        let mut rows = collect_member_values(world, hooks, area)?;
        if rows.is_empty() {
            rows = random_slot_fill(
                world,
                hooks,
                usize::try_from(area).unwrap_or(0),
                typed,
                entry,
            );
        }
        match entry.kind {
            0 => world.typed_rows[typed].kind = 0,
            1 => world.typed_rows[typed].kind = 1,
            _ => {}
        }
        let n = rows.len().min(SLOTS);
        let mut row = world.typed_rows[typed];
        apply_ids_to_slots(&world.ids, entry, &mut row, &rows, n);
        world.typed_rows[typed] = row;
    }
    Ok(())
}
