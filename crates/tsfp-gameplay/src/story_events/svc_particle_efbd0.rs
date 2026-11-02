//! T1856: the effect entry spawner `0xEFBD0` (the ember particle of the level 4 / 0x50 prelude) and its non-engine callees.
//!
//! Source: `build/default.xbe` (SHA-256 `3cfd001a84fc3e08175d6c4b2e42ebf49577a089b5d0bd87ac724f61a41816bc`). Decoded with
//! `tools/x87_blocks.py`, validated bit for bit against the original under the Unicorn oracle
//! (`tools/t1846_gold/svc_particle_efbd0.py`, test `tests/story_events_t1856_particle_efbd0.rs`). Document:
//! `docs/t-story-event-particle-efbd0-t1856.md`. Names INFERRED, behaviour MEASURED (xemu level, not silicon).
//!
//! | address | port |
//! |---|---|
//! | `0xEFBD0(kind, extra, pos*, dir*, source, owner, seconds, sound)` | [`spawn_effect`] |
//! | `0xDC4A0(type)` | [`EffectPool::allocate`] (reuse the oldest slot of the class, zero fill 0x22C bytes) |
//! | `0xDC2F0(slot, class)` | [`EffectPool::unlink`] (list relink; the teardown prefix is [`EmberEngine::teardown_slot`]) |
//! | `0xDC550(entry)` | [`EffectPool::release`] (give a slot back, it becomes the head of its class) |
//! | `0xC3C50(point*, y_bias, radius)` | [`ground_probe`] (floor query by region ids, then a cell walk `0x888A0`) |
//! | `0x888A0(point*, cell, vec*, flag)` | [`walk_cells`] (up to 100 `0x88430` steps) |
//! | `0xEDD40(entry)` | [`start_effect`] for kind 15 (the ember) and the default tail; other kinds via [`EmberEngine::start_kind`] |
//! | `0x15BFB0(value)` | [`RecentList::push`] |
//! | `0x238510(a, b, c)` / `0x2382B0(b)` | [`stat_add`] / [`stat_slot_valid`] |
//! | `0x21A40`, `0x219E0` | [`sound_request`] plus [`EmberEngine::submit_sound`] (the `0x45390` record is `prelude_1e2b80::init_spawn_record`) |
//!
//! Not ported (engine services, behind [`EmberEngine`]): the sound submit `0x45820` (T1856 `svc_effect_submit`), the emitter
//! creation `0x111AE0`, the region tree query `0x87590` (`0x87310`, `0x157440`), the cell step `0x88430` (ported in
//! `sweep_cells`, adapter [`CellGraphEngine::advance_cell`] helper), the kind handlers of `0xEDD40` other than 15 and the
//! default tail, the slot teardown `0xDC2F0` prefix (T1856 `svc_slot_teardown`), `0xD1EC0`, `0xEDBE0`, `0x186480`,
//! `0xFF8A0`, `0x10E660`, `0x3533F0`, `0x46A00`.
//!
//! The 0x22C byte effect entry is kept as raw words ([`EffectEntry`]) so the golden compares the whole record.

use super::callees_t1846::{ThrottledSound, throttled_sound};
use super::group_members::{AdvanceOut, AdvanceProbe};
use super::prelude_1e2b80::{PreludeEffect, init_spawn_record};
use super::world_queries::{CollisionWorld, NO_HIT, face_height_query};

/// Size of an effect entry in dwords (`0x22C` bytes).
pub const ENTRY_WORDS: usize = 0x8B;
/// Entry type passed to `0xDC4A0` by `0xEFBD0`.
pub const EFFECT_TYPE: u32 = 0x17;
/// Stride of the kind table (`0x4DB490`).
pub const KIND_ROW_BYTES: u32 = 24;

// Entry word indices (byte offset / 4). Roles INFERRED, offsets MEASURED.
pub const W_TYPE: usize = 0;
pub const W_KIND: usize = 1;
pub const W_FLAGS: usize = 3;
pub const W_PREV: usize = 4;
pub const W_NEXT: usize = 5;
pub const W_POS: usize = 7;
pub const W_TIMER: usize = 0x98 / 4;
pub const W_OWNER: usize = 0xA0 / 4;
pub const W_GROUND: usize = 0xF0 / 4;
pub const W_MODE: usize = 0x100 / 4;
pub const W_LINK: usize = 0x104 / 4;
pub const W_SOURCE: usize = 0x108 / 4;
pub const W_SOURCE_COPY: usize = 0x10C / 4;
pub const W_DIR: usize = 0x180 / 4;
pub const W_EXTRA: usize = 0x1C0 / 4;

/// One effect entry (0x22C bytes) as dwords.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectEntry {
    pub w: [u32; ENTRY_WORDS],
}

impl Default for EffectEntry {
    fn default() -> Self {
        Self {
            w: [0; ENTRY_WORDS],
        }
    }
}

impl EffectEntry {
    pub fn f(&self, index: usize) -> f32 {
        f32::from_bits(self.w[index])
    }
    pub fn set_f(&mut self, index: usize, value: f32) {
        self.w[index] = value.to_bits();
    }
    pub fn position(&self) -> [f32; 3] {
        [self.f(W_POS), self.f(W_POS + 1), self.f(W_POS + 2)]
    }
    pub fn direction(&self) -> [f32; 3] {
        [self.f(W_DIR), self.f(W_DIR + 1), self.f(W_DIR + 2)]
    }
}

/// A row of the kind table (`0x4DB490 + 24 * kind`). The stock rows are game data and are supplied by the host.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KindRow {
    /// `+0`, `+4`: effect ids tried by the default start path (-1 = none).
    pub effect_a: i32,
    pub effect_b: i32,
    /// `+8`: not read by the ported paths.
    pub word_8: u32,
    /// `+0xC`: squared and passed to the light service `0x186480`.
    pub light_radius: f32,
    /// `+0x10`: player influence radius.
    pub radius: f32,
    /// `+0x14`: bit 0 = the entry is released right after the start (one shot).
    pub flags: u32,
}

/// The effect entry pool: slot storage plus the per class doubly linked lists (`[0x737BD4]`, heads `0x7388FC`, tails
/// `0x7368AC`, class table `[type * 64 + 0x4DEB28]`). Links are slot indices, -1 = none.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectPool {
    pub entries: Vec<EffectEntry>,
    pub head: Vec<i32>,
    pub tail: Vec<i32>,
    /// Class of an entry type (index = type).
    pub class_of_type: Vec<u32>,
    /// `[0x4DFB1C]`: set to 1 whenever a slot moves.
    pub dirty: u32,
    /// Guest pointer to slot 0 (`[0x737BD4]`), only used to form entry addresses for the engine.
    pub base: u32,
}

impl EffectPool {
    fn class(&self, entry_type: u32) -> usize {
        self.class_of_type[entry_type as usize] as usize
    }

    /// `0xDC2F0(slot, class)`: tear the slot down (engine prefix) and unlink it from its class list.
    pub fn unlink(&mut self, engine: &mut dyn EmberEngine, slot: usize, class: usize) {
        let prev = self.entries[slot].w[W_PREV] as i32;
        let next = self.entries[slot].w[W_NEXT] as i32;
        engine.teardown_slot(slot, &mut self.entries[slot]);
        if prev >= 0 {
            self.entries[prev as usize].w[W_NEXT] = next as u32;
            self.entries[slot].w[W_NEXT] = u32::MAX;
        } else {
            self.head[class] = next;
        }
        if next >= 0 {
            self.entries[next as usize].w[W_PREV] = prev as u32;
            self.entries[slot].w[W_PREV] = u32::MAX;
        } else {
            self.tail[class] = prev;
        }
    }

    /// `0xDC4A0(type)`: recycles the oldest slot of the class (head of the list) as the newest (tail), zero fills it and sets
    /// the defaults. `None` when the class list is empty (the original would index slot -1). A one slot list makes the
    /// original write through slot -1 (stray store before the pool), not modelled.
    pub fn allocate(&mut self, engine: &mut dyn EmberEngine, entry_type: u32) -> Option<usize> {
        let class = self.class(entry_type);
        let first = self.head[class];
        if first < 0 {
            return None;
        }
        let slot = first as usize;
        self.unlink(engine, slot, class);
        let tail = self.tail[class];
        if tail >= 0 {
            self.entries[tail as usize].w[W_NEXT] = slot as u32;
        }
        self.tail[class] = slot as i32;
        let entry = &mut self.entries[slot];
        entry.w = [0; ENTRY_WORDS];
        entry.w[W_TYPE] = entry_type;
        entry.w[W_PREV] = tail as u32;
        entry.w[W_NEXT] = u32::MAX;
        entry.w[0x100 / 4] = u32::MAX;
        for word in 0x130 / 4..=0x13C / 4 {
            entry.w[word] = u32::MAX;
        }
        self.dirty = 1;
        Some(slot)
    }

    /// `0xDC550(entry)`: mark the slot free (type -1) and move it to the head of its class list so it is reused first.
    pub fn release(&mut self, engine: &mut dyn EmberEngine, slot: usize) {
        let entry_type = self.entries[slot].w[W_TYPE];
        if entry_type == u32::MAX {
            return;
        }
        let class = self.class(entry_type);
        self.entries[slot].w[W_TYPE] = u32::MAX;
        self.unlink(engine, slot, class);
        let head = self.head[class];
        if head >= 0 {
            self.entries[head as usize].w[W_PREV] = slot as u32;
        }
        self.entries[slot].w[W_PREV] = u32::MAX;
        self.entries[slot].w[W_NEXT] = head as u32;
        self.head[class] = slot as i32;
        self.dirty = 1;
    }
}

/// `0x15BFB0` state: rows of 11 dwords (`count`, ten values) at `0x7A2900`, current row `[0x7A295C]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecentList {
    pub words: Vec<u32>,
    pub row: u32,
}

impl RecentList {
    /// `0x15BFB0(value)`: stores `value` at `row * 11 + count + 1` and bumps `count` while it is below 10. At 10 the store
    /// lands on the count word of the next row (overrun of the original, reproduced). A store past `words` is returned
    /// as `(word index, value)`: with two rows word 22 is the global `[0x7A2958]` (`EmberGlobals::extra_players`) and
    /// word 23 the row index `[0x7A295C]`; the caller applies it.
    pub fn push(&mut self, value: u32) -> Option<(usize, u32)> {
        let base = self.row as usize * 11;
        let &count = self.words.get(base)?;
        let target = base + count as usize + 1;
        let stray = match self.words.get_mut(target) {
            Some(slot) => {
                *slot = value;
                None
            }
            None => Some((target, value)),
        };
        if (count as i32) < 10 {
            self.words[base] = count.wrapping_add(1);
        }
        stray
    }
}

/// A player as read by the ember proximity loop (`[0x7B0C48] + 0x1584 * i`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerSlot {
    /// `+0xB4`, `+0xB8`, `+0xBC`.
    pub position: [f32; 3],
    /// `+0xFBC`, `+0xFC0`: written by the proximity effect.
    pub shake: f32,
    pub shake_limit: f32,
}

/// The source object fields `0xEFBD0` reads (`[esi+0x10C]`). `address` is its guest pointer (stored in the entry).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceObject {
    pub address: u32,
    /// `+0x20`: kind, 8 = the case that links the entry.
    pub kind: u32,
    /// `+0x28`: negative = skipped.
    pub state: i32,
    /// `+0x7C`.
    pub link: u32,
    /// First dword of the block at `+0x150` (`None` when the pointer is null).
    pub stat_index: Option<i32>,
}

/// Globals read by the ported code.
#[derive(Clone, Debug, PartialEq)]
pub struct EmberGlobals {
    /// `[0x4E8CEC]` (60).
    pub frame_rate: i32,
    /// `[0x7DE455]`.
    pub mode_byte: u8,
    /// `[0x7DE458]`.
    pub mode_flags: u32,
    /// `[0x79094C]`.
    pub level_id: i32,
    /// `[0x7DE30C]`.
    pub game_state: i32,
    /// `[0x7DE318]`.
    pub clock: i32,
    /// `[0x737C5C]`.
    pub throttle_stamp: i32,
    /// `[0x790950]` is `players.len()`; `[0x7A2958]`.
    pub extra_players: i32,
    /// `[0x5236C4]`.
    pub level_limit: i32,
    /// `[0x4DCB00]`: height stored by the ground probe.
    pub ground_height: f32,
    /// `[0x7B9754]`: the cell walk `0x888A0` does nothing when it is not positive.
    pub cell_count: i32,
}

impl Default for EmberGlobals {
    fn default() -> Self {
        Self {
            frame_rate: 60,
            mode_byte: 0,
            mode_flags: 0,
            level_id: 0,
            game_state: 0,
            clock: 0,
            throttle_stamp: 0,
            extra_players: 0,
            level_limit: 0,
            ground_height: 0.0,
            cell_count: 0,
        }
    }
}

/// All state the ported functions touch.
#[derive(Clone, Debug, Default)]
pub struct EmberWorld {
    pub globals: EmberGlobals,
    pub pool: EffectPool,
    pub rows: Vec<KindRow>,
    /// Table `0x4E0FA0` (stride 0x1C): first dword non zero for each effect id.
    pub effect_defined: Vec<bool>,
    pub players: Vec<PlayerSlot>,
    pub collision: CollisionWorld,
    pub recent: RecentList,
    /// Float table `[0x75CCC0]` (row stride 0x273).
    pub stats: Vec<f32>,
}

/// The engine services behind the ported code. Every method is one call of the original (argument words as noted).
pub trait EmberEngine {
    /// `0x153B40`: 64 bit LCG, low dword.
    fn rng(&mut self) -> u32;
    /// `0x45820(&request)`: submit a 0x30 byte request (built with `0x45390`).
    fn submit_sound(&mut self, request: [u32; 12]);
    /// `0x46A00(0xCFB, 0.1, 0.95, 1.05)` via `0xE0080`.
    fn throttled_sound(&mut self, sound: ThrottledSound);
    /// `0x87590(0, &box, ring, 30)`: ids of the regions overlapping the box (at most 30).
    fn region_query(&mut self, region: [f32; 6]) -> Vec<i32>;
    /// `0x88430`.
    fn advance_cell(&mut self, probe: &AdvanceProbe) -> AdvanceOut;
    /// `0x888A0(&point, cell, &vec, flag)`: the cell walk (default: [`walk_cells`] over [`EmberEngine::advance_cell`]).
    fn walk_cells(
        &mut self,
        cell_count: i32,
        from: [f32; 3],
        cell: u32,
        vec: [f32; 3],
        flag: u32,
    ) -> u32 {
        walk_cells(self, cell_count, from, cell, vec, flag)
    }
    /// `0x3533F0`.
    fn flag_3533f0(&mut self) -> bool;
    /// `0x111AE0(0, template, 0, &position, &direction)`.
    fn spawn_emitter(&mut self, template: u32, position: [f32; 3], direction: [f32; 3]);
    /// `0x10E660(scale)`.
    fn set_scale(&mut self, scale: f32);
    /// `0xD1EC0(player, f, a, b)`.
    fn nudge_player(&mut self, player: usize, f: f32, a: f32, b: f32);
    /// `0xFF8A0(0, id, &position)`.
    fn trigger_effect(&mut self, id: i32, position: [f32; 3]);
    /// `0xEDBE0(&position, 0)`.
    fn settle(&mut self, position: [f32; 3]);
    /// `0xEDBE0(&position, 0, entry)` with the entry it reads (T1863): the default forwards to [`EmberEngine::settle`], an engine
    /// that runs the ported `svc_ember_services::settle_entry` overrides it.
    fn settle_entry(&mut self, _world: &EmberWorld, _slot: usize, position: [f32; 3]) {
        self.settle(position);
    }
    /// `0x186480(&position, radius_squared)`.
    fn light(&mut self, position: [f32; 3], radius_squared: f32);
    /// Teardown prefix of `0xDC2F0` for a slot being recycled (callbacks, handles, links). A zero filled slot needs none.
    fn teardown_slot(&mut self, slot: usize, entry: &mut EffectEntry);
    /// The per kind handler of `0xEDD40` for the kinds that are not ported. Returns the result of `0xEDD40`.
    fn start_kind(&mut self, world: &mut EmberWorld, slot: usize, kind: u32) -> u32;
}

/// `cvttss2si`: truncation, out of range and NaN give `i32::MIN`.
pub fn cvtt(value: f32) -> i32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

/// The 0x30 byte sound request built by `0x45390` and filled by `0x21A40` (`range` set) or `0x219E0` (`range` none).
pub fn sound_request(id: u32, ticks: i32, position: [f32; 3], range: Option<f32>) -> [u32; 12] {
    let mut words = init_spawn_record();
    words[0] = id;
    words[1] = ticks as u32;
    for (slot, value) in words[7..10].iter_mut().zip(position) {
        *slot = value.to_bits();
    }
    words[10] = 1;
    if let Some(range) = range {
        words[11] = range.to_bits();
    }
    words
}

/// The sound step of `0xEFBD0` for one kind (jump table `0xEFF3C`, byte map `0xEFF5C`). `seconds` is the float argument.
pub fn play_kind_sound(
    world: &mut EmberWorld,
    engine: &mut dyn EmberEngine,
    kind: u32,
    seconds: f32,
    position: [f32; 3],
) {
    let rate = world.globals.frame_rate as f32;
    let ticks = cvtt(rate * seconds);
    let submit = |engine: &mut dyn EmberEngine, id: u32, ticks: i32, range: Option<f32>| {
        engine.submit_sound(sound_request(id, ticks, position, range));
    };
    match kind.wrapping_sub(0xB) {
        0 | 1 => {
            submit(engine, 0x1A6, ticks, Some(75.0));
            let later = cvtt((seconds + 1.0) * rate);
            submit(engine, 0x1A7, later, Some(75.0));
        }
        2 | 3 => submit(engine, 0x1E6, ticks, Some(75.0)),
        4 => {}
        5 => submit(engine, 0xC33, ticks, Some(150.0)),
        0x1D | 0x24 => {
            let clock = world.globals.clock;
            if let Some(sound) = throttled_sound(clock, &mut world.globals.throttle_stamp) {
                engine.throttled_sound(sound);
            }
        }
        0x21..=0x23 => submit(engine, 0x631, ticks, None),
        0x25 => {
            let draw = engine.rng();
            let fraction = f64::from(draw) * f64::from(f32::from_bits(0x2F80_0000));
            let pick = (fraction * 2.0 + 0.5) as i32;
            submit(engine, (0x5F3 + pick) as u32, ticks, None);
        }
        _ => {
            let draw = engine.rng();
            submit(engine, 0xC33 + draw % 6, ticks, None);
        }
    }
}

/// `0x2382B0(index)`: is `index` a valid player slot of the current mode (signed compares).
pub fn stat_slot_valid(world: &EmberWorld, engine: &mut dyn EmberEngine, index: i32) -> bool {
    let players = world.players.len() as i32;
    let globals = &world.globals;
    let limit = if globals.mode_byte == 0xA {
        if index >= 2 {
            return false;
        }
        if engine.flag_3533f0() {
            0x10
        } else {
            players.wrapping_add(globals.extra_players)
        }
    } else {
        if index >= 0x1A {
            return false;
        }
        if globals.level_id == 0x65 {
            globals.level_limit
        } else if globals.mode_flags & 0x40 == 0 {
            players.wrapping_add(globals.extra_players)
        } else if engine.flag_3533f0() {
            0x10
        } else {
            globals.extra_players.wrapping_add(players)
        }
    };
    index < limit
}

/// `0x238510(a, b, c)`: `stats[b * 0x273 + a] += c` when `b` is a valid slot and `[0x7DE30C] == 7`.
pub fn stat_add(world: &mut EmberWorld, engine: &mut dyn EmberEngine, a: i32, b: i32, c: f32) {
    if !stat_slot_valid(world, engine, b) || world.globals.game_state != 7 {
        return;
    }
    let index = b.wrapping_mul(0x273).wrapping_add(a);
    if let Some(cell) = usize::try_from(index)
        .ok()
        .and_then(|i| world.stats.get_mut(i))
    {
        *cell += c;
    }
}

/// `0x888A0(point*, cell, vec*, flag)`: follow the cell chain with `0x88430` until the cell stops changing, at most 100
/// steps. Returns the final cell. A cell of 0 returns 0 after the first step.
pub fn walk_cells<E: EmberEngine + ?Sized>(
    engine: &mut E,
    cell_count: i32,
    from: [f32; 3],
    cell: u32,
    vec: [f32; 3],
    flag: u32,
) -> u32 {
    if cell_count <= 0 || cell == 0 {
        return cell;
    }
    let mut probe = AdvanceProbe {
        from_copy: from,
        vec_copy: vec,
        cell,
        previous: 0,
        flag,
    };
    let mut budget = 0x64;
    loop {
        if budget == 0 {
            return probe.cell;
        }
        budget -= 1;
        let out = engine.advance_cell(&probe);
        let old = probe.cell;
        probe = AdvanceProbe {
            from_copy: out.from_copy,
            vec_copy: out.vec_copy,
            cell: out.next,
            previous: old,
            flag,
        };
        if out.next == old {
            return old;
        }
    }
}

/// `v - v` as the original computes it: 0 for finite `v`, NaN for an infinite or NaN `v`.
#[allow(clippy::eq_op)]
fn self_difference(v: f32) -> f32 {
    v - v
}

/// `0xC3C50(point*, y_bias, radius)`: the highest floor under `point` among the regions around it, then a cell walk down to
/// that floor. Returns the final cell id (0 when no floor was found). Updates `collision.best_value`/`best_id` (`[0x4DCAFC]`,
/// `[0x733088]`) for every id and `globals.ground_height` (`[0x4DCB00]`) when the walk ends on a positive cell.
/// The cell walk is [`EmberEngine::walk_cells`] with `globals.cell_count`.
pub fn ground_probe(
    world: &mut EmberWorld,
    engine: &mut dyn EmberEngine,
    point: [f32; 3],
    y_bias: f32,
    radius: f32,
) -> i32 {
    let [x, y, z] = point;
    let query_point = [x, y + y_bias, z];
    let region = [
        x - radius,
        y,
        z - radius,
        x + radius,
        y + y_bias,
        z + radius,
    ];
    let ids: Vec<i32> = engine.region_query(region).into_iter().take(30).collect();
    let mut best = NO_HIT;
    let mut best_id = 0i32;
    for &id in &ids {
        let mut height = NO_HIT;
        if id > 0 && id <= world.collision.id_limit {
            let index = world.collision.id_table[id as usize];
            let object = &world.collision.objects[index];
            let faces = object.faces.as_deref().unwrap_or(&[]);
            let (value, _) = face_height_query(
                faces,
                object.class_mask,
                query_point,
                radius,
                world.collision.ignore_flag,
            );
            if value > world.collision.best_value {
                world.collision.best_value = value;
                world.collision.best_id = id;
            }
            height = if NO_HIT > value { NO_HIT } else { value };
        }
        if height > best {
            best = height;
            best_id = id;
        }
    }
    if best == NO_HIT {
        return 0;
    }
    let start = [x, best, z];
    let vec = [self_difference(x), y - best, self_difference(z)];
    let cell = engine.walk_cells(world.globals.cell_count, start, best_id as u32, vec, 0) as i32;
    if cell > 0 {
        world.globals.ground_height = best;
    }
    cell
}

/// Arguments of `0xEFBD0` (cdecl, in stack order).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpawnArgs {
    pub kind: u32,
    pub extra: u32,
    pub position: [f32; 3],
    pub direction: Option<[f32; 3]>,
    /// `E+0x14`: stored at `+0x108`/`+0x10C` and tested for the link step.
    pub source: Option<SourceObject>,
    /// `E+0x18`: stored at `+0xA0`, pushed to [`RecentList`] in mode `0xA`.
    pub owner: u32,
    /// `E+0x1C`: the timer is `0 - seconds`.
    pub seconds: f32,
    /// `E+0x20`: play the kind's sound.
    pub sound: bool,
}

/// Outcome of [`spawn_effect`].
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnResult {
    /// Slot of the entry (`None` when the pool class was empty).
    pub slot: Option<usize>,
    /// The return value of the original: the result of `0xEDD40` when it ran, else the entry pointer (here the slot + 1,
    /// 0 for `None`).
    pub returned: Option<u32>,
    /// True when `0xEDD40` ran (timer at or above zero).
    pub started: bool,
}

/// `0xEFBD0`: allocate an effect entry, fill it, play the kind's sound, probe the ground, link the source object and start
/// the entry (kind handler `0xEDD40`) unless its timer is negative.
pub fn spawn_effect(
    world: &mut EmberWorld,
    engine: &mut dyn EmberEngine,
    args: &SpawnArgs,
) -> SpawnResult {
    let Some(slot) = world.pool.allocate(engine, EFFECT_TYPE) else {
        return SpawnResult {
            slot: None,
            returned: None,
            started: false,
        };
    };
    let source_address = args.source.map_or(0, |s| s.address);
    {
        let entry = &mut world.pool.entries[slot];
        entry.w[W_EXTRA] = args.extra;
        entry.w[W_KIND] = args.kind;
    }
    if args.sound {
        play_kind_sound(world, engine, args.kind, args.seconds, args.position);
    }
    let timer = 0.0f32 - args.seconds;
    let entry = &mut world.pool.entries[slot];
    match args.direction {
        Some(d) => {
            let p = args.position;
            entry.set_f(W_POS, d[0] * 0.1 + p[0]);
            entry.set_f(W_POS + 1, d[1] * 0.1 + p[1]);
            entry.set_f(W_POS + 2, d[2] * 0.1 + p[2]);
            for (k, v) in d.iter().enumerate() {
                entry.set_f(W_DIR + k, *v);
            }
        }
        None => {
            for (k, v) in args.position.iter().enumerate() {
                entry.set_f(W_POS + k, *v);
            }
            entry.set_f(W_DIR, 0.0);
            entry.set_f(W_DIR + 1, 1.0);
            entry.set_f(W_DIR + 2, 0.0);
        }
    }
    entry.w[W_OWNER] = args.owner;
    entry.w[W_LINK] = 0;
    entry.w[W_SOURCE] = source_address;
    entry.w[W_SOURCE_COPY] = source_address;
    entry.set_f(W_TIMER, timer);
    let position = entry.position();
    let ground = ground_probe(world, engine, position, 0.1, 0.0);
    world.pool.entries[slot].w[W_GROUND] = ground as u32;
    if let Some(source) = args.source {
        if source.kind == 8 && source.state >= 0 {
            world.pool.entries[slot].w[W_LINK] = source.link;
            match source.stat_index {
                Some(index) => {
                    world.pool.entries[slot].w[W_MODE] = 1;
                    stat_add(world, engine, 0x52, index, 1.0);
                }
                None => world.pool.entries[slot].w[W_MODE] = 2,
            }
        }
    }
    if args.owner != 0 && world.globals.mode_byte == 0xA {
        if let Some((index, value)) = world.recent.push(args.owner) {
            match index {
                22 => world.globals.extra_players = value as i32,
                23 => world.recent.row = value,
                _ => {}
            }
        }
    }
    if timer >= 0.0 {
        let result = start_effect(world, engine, slot);
        SpawnResult {
            slot: Some(slot),
            returned: Some(result),
            started: true,
        }
    } else {
        world.pool.entries[slot].w[W_FLAGS] |= 0x80_0000;
        SpawnResult {
            slot: Some(slot),
            returned: Some(slot as u32 + 1),
            started: false,
        }
    }
}

/// The templates `0xEDD40` passes to `0x111AE0` for kind 15.
pub const EMBER_TEMPLATES: [u32; 2] = [0x0073_5FC8, 0x0073_5DE8];
/// Scale passed to `0x10E660` by kind 15 (`0x3DCCCCCD`).
pub const EMBER_SCALE: f32 = f32::from_bits(0x3DCC_CCCD);

/// `0xEDD40(entry)`: per kind start. Kind 15 (ember) creates its two emitters and sets the scale, kinds 7, 8, 9, 19, 27 and
/// anything outside 6..=48 take the default path (effect ids of the kind row), the other kinds go to
/// [`EmberEngine::start_kind`]. All ported paths end in the player proximity loop and the one shot / light tail.
/// Returns what the original returns: 0 after a release, else the entry (slot + 1 here).
pub fn start_effect(world: &mut EmberWorld, engine: &mut dyn EmberEngine, slot: usize) -> u32 {
    let kind = world.pool.entries[slot].w[W_KIND];
    let index = kind.wrapping_sub(6);
    let position = world.pool.entries[slot].position();
    match index {
        9 => {
            let direction = world.pool.entries[slot].direction();
            for template in EMBER_TEMPLATES {
                engine.spawn_emitter(template, position, direction);
            }
            engine.set_scale(EMBER_SCALE);
        }
        0..=0x2A if !matches!(index, 1 | 2 | 3 | 13 | 21) => {
            return engine.start_kind(world, slot, kind);
        }
        _ => {
            let row = world.rows.get(kind as usize).copied().unwrap_or_default();
            for id in [row.effect_a, row.effect_b] {
                if id != -1
                    && world
                        .effect_defined
                        .get(id as usize)
                        .copied()
                        .unwrap_or(false)
                {
                    engine.trigger_effect(id, position);
                }
            }
        }
    }
    finish_start(world, engine, slot)
}

/// The common tail of `0xEDD40` (`0xEF986`): players inside the kind radius get a nudge, then either release (one shot
/// flag) or settle the position and light it.
pub fn finish_start(world: &mut EmberWorld, engine: &mut dyn EmberEngine, slot: usize) -> u32 {
    let kind = world.pool.entries[slot].w[W_KIND];
    let row = world.rows.get(kind as usize).copied().unwrap_or_default();
    let position = world.pool.entries[slot].position();
    let radius_squared = row.radius * row.radius;
    for (index, player) in world.players.iter_mut().enumerate() {
        let dx = position[0] - player.position[0];
        let dz = position[2] - player.position[2];
        let dy = position[1] - player.position[1];
        let distance_squared = (dz * dz + dy * dy) + dx * dx;
        if radius_squared > distance_squared {
            let t = distance_squared / radius_squared;
            let f = f32::from_bits(0x3F99_999A) - t;
            let a = f32::from_bits(0x3F4C_CCCD) - t * f32::from_bits(0x3F4C_CCCD);
            let b = f32::from_bits(0x3DCC_CCCD) - t * f32::from_bits(0x3DCC_CCCD);
            engine.nudge_player(index, f, a, b);
            player.shake = f32::from_bits(0x4030_0000);
            player.shake_limit = if f > f32::from_bits(0x3F66_6666) {
                f32::from_bits(0x3F66_6666)
            } else {
                f
            };
        }
    }
    if row.flags & 1 != 0 {
        world.pool.release(engine, slot);
        return 0;
    }
    engine.settle_entry(world, slot, position);
    engine.light(position, row.light_radius * row.light_radius);
    slot as u32 + 1
}

/// Adapter for the prelude: runs a recorded [`PreludeEffect::Particle`] as the original call
/// `0xEFBD0(0xF, -1, &pos, &dir, 0, 0, 0, 1)`. Other effects return `None`.
pub fn run_prelude_particle(
    world: &mut EmberWorld,
    engine: &mut dyn EmberEngine,
    effect: &PreludeEffect,
) -> Option<SpawnResult> {
    let PreludeEffect::Particle { pos, dir } = effect else {
        return None;
    };
    let args = SpawnArgs {
        kind: 0xF,
        extra: u32::MAX,
        position: *pos,
        direction: Some(*dir),
        source: None,
        owner: 0,
        seconds: 0.0,
        sound: true,
    };
    Some(spawn_effect(world, engine, &args))
}

/// Convenience: [`EmberEngine::advance_cell`] on a `sweep_cells::CellGraph`.
pub struct CellGraphEngine;

impl CellGraphEngine {
    pub fn advance_cell(graph: &super::sweep_cells::CellGraph, probe: &AdvanceProbe) -> AdvanceOut {
        super::sweep_cells::advance_88430(graph, probe)
    }
}
