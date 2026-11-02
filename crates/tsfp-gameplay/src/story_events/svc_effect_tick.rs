//! T1868: the effect pool tick `0xDC7B0` and the effect entry update `0xEFF90` (the class callback of entry type `0x17`).
//!
//! Source: `build/default.xbe` (SHA-256 `3cfd001a84fc3e08175d6c4b2e42ebf49577a089b5d0bd87ac724f61a41816bc`). Decoded with
//! `tools/x87_blocks.py`, validated bit for bit against the original under the Unicorn oracle
//! (`tools/t1846_gold/svc_effect_tick.py`, test `tests/story_events_t1868_effect_tick.rs`). Document:
//! `docs/t-story-event-effect-pool-t1868.md`. Names INFERRED, behaviour MEASURED (xemu level, not silicon).
//!
//! | address | port |
//! |---|---|
//! | `0xDC7B0()` | [`pool_tick`] (every frame: walks the 85 slots of `[0x737BD4]`, advances the timer, calls the class callback) |
//! | `0xEFF90(entry)` | [`effect_update`] (class record `0x4DEB28 + 64 * class`, callback at `+0x34`; type `0x17`) |
//! | `0x153C20()` | [`trig_table`] (2048 entry sine table at `0x7A38A0`) |
//! | `0xB91F0(rec)` | [`init_trail_record`] |
//!
//! The pool tick (`0xDC7B0`), per slot whose type is not `-1`:
//! 1. When the frame step `[0x4E7944]` is positive and the entry flag `0x20` is set: a zero `+0x150` releases the slot
//!    (`0xDC550`) and ends the slot, otherwise `+0x150` is cleared.
//! 2. Types other than `0x17` with a source object (`+0x108`), a timer below `0.05` and flag `0x100` clear mask the `0x60` bits of
//!    the source object's flags around the callback: that foreign object work belongs to [`TickEngine::tick_other`].
//! 3. `timer += [0x4E794C] / (f32) [0x4E8CEC]` (the step scale over the frame rate), the low half of `+0x18` becomes 1.
//! 4. The class callback is called with the entry. For `0x17` it is [`effect_update`].
//!
//! `effect_update` (`0xEFF90`): a waiting entry (flag `0x800000`) whose timer reached `0.0` starts (`0xEDD40`); an entry whose
//! timer exceeds `1.25` is released; an entry without flag `0x20000` reports a trail record to `0xBB850`; kinds `0x2C..=0x2E`
//! with a timer below `0.25` spray `(1 - timer * 2) * step * 6` particles through `0x345B60` and `0x345C60`.
//!
//! Not ported (engine services behind [`TickEngine`]): `0xEDD40` (the ported [`super::svc_particle_efbd0::start_effect`] is the
//! default of the host), `0xBB850`, `0x345B60`, `0x345C60`, and the callbacks of the other entry types.

use std::sync::OnceLock;

use super::svc_particle_efbd0::{
    ENTRY_WORDS, EmberEngine, EmberWorld, W_FLAGS, W_KIND, W_OWNER, W_POS, W_SOURCE, W_SOURCE_COPY,
    W_TIMER, W_TYPE, cvtt,
};

/// Entry type of the effect entries (`0xEFBD0` allocates `0x17`).
pub const EFFECT_TYPE: u32 = 0x17;
/// Flag set on an entry that waits for its negative timer to reach zero.
pub const FLAG_WAITING: u32 = 0x0080_0000;
/// Flag that suppresses the trail record.
pub const FLAG_NO_TRAIL: u32 = 0x0002_0000;
/// Flag `0x20` of the pool tick (an entry that must be released when `+0x150` is zero).
pub const FLAG_RELEASE_CHECK: u32 = 0x20;
/// Words of the record handed to `0xBB850` (`0x17` dwords, built by `0xB91F0`).
pub const TRAIL_WORDS: usize = 0x17;
/// Words of the `+0x150` field.
const W_SPRAY_GROUND: usize = 0xF0 / 4;
const W_PROBE: usize = 0x150 / 4;
const W_PAIR: usize = 0x18 / 4;
const W_EXTRA: usize = 0x1C0 / 4;
const W_LINK: usize = 0x104 / 4;
const W_SOURCE_PTR: usize = W_SOURCE;
/// Size of an entry in bytes and of the pool loop bound (`0xB89C` = 85 slots).
pub const ENTRY_BYTES: u32 = 0x22C;
pub const POOL_SLOTS: usize = 85;
/// Guest address of `[class table]` and the callback offset used by the tick.
pub const CLASS_TABLE: u32 = 0x004D_EB28;
pub const CLASS_STRIDE: u32 = 64;

/// The per frame timing globals `[0x4E7944]` (frame step count, 0 while paused) and `[0x4E794C]` (the same as a float).
/// Written by the frame loop near `0x153A00` (`[0x4E7944] = n`, `[0x4E794C] = (f32) n`; 1 and 1.0 when no clock runs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickGlobals {
    pub steps: i32,
    pub scale: f32,
}

impl TickGlobals {
    /// A frame that advances `steps` simulation steps.
    pub fn frame(steps: i32) -> Self {
        Self {
            steps,
            scale: steps as f32,
        }
    }
}

impl Default for TickGlobals {
    fn default() -> Self {
        Self::frame(1)
    }
}

/// The two particle spray service calls of kinds `0x2C..=0x2E`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SprayCall {
    /// `0x345B60(mode, owner, 0, &position, &velocity)`; `mode` is 2 for kind `0x2E`, else 1.
    Burst {
        mode: u32,
        owner: u32,
        position: [f32; 3],
        velocity: [f32; 3],
    },
    /// `0x345C60(&position, &velocity, cell, 2.0, 2)`; `cell` is `[owner + 0x34]` or the entry's `+0xF0`.
    Streak {
        position: [f32; 3],
        velocity: [f32; 3],
        cell: u32,
    },
}

/// Engine services of the tick.
pub trait TickEngine: EmberEngine {
    /// `0xEDD40(entry)`: the per kind start. The host runs [`super::svc_particle_efbd0::start_effect`] (which returns 0 after a
    /// release, else slot + 1).
    fn start_entry(&mut self, world: &mut EmberWorld, slot: usize) -> u32;
    /// `0xBB850(entry + 0xFC, &record, timer)`: the trail or light record of a live entry.
    fn trail(&mut self, slot: usize, record: &[u32; TRAIL_WORDS], timer: f32);
    /// `0xBB850` with the entry at hand: the emission counter `[entry + 0xFC]` and the position live in the entry (T1870). The default
    /// forwards to [`TickEngine::trail`], so hosts that only record the call keep working.
    fn trail_with(
        &mut self,
        _world: &mut EmberWorld,
        slot: usize,
        record: &[u32; TRAIL_WORDS],
        timer: f32,
        _tick: &TickGlobals,
    ) {
        self.trail(slot, record, timer);
    }
    /// The spray calls of kinds `0x2C..=0x2E`.
    fn spray(&mut self, call: SprayCall);
    /// `[owner + 0x34]` (the cell of an owner object).
    fn owner_cell(&mut self, owner: u32) -> u32;
    /// The class callback (and the source flag masking around it) of an entry type other than `0x17`.
    fn tick_other(&mut self, _world: &mut EmberWorld, _slot: usize) {}
    /// Same as [`TickEngine::tick_other`] with the entry timer as it was BEFORE the advance of this tick (T1870): the source flag
    /// masking of `0xDC7B0` decides on that value. The default forwards to `tick_other`; hosts that run the class callbacks call
    /// [`super::svc_pool_classes::class_tick`].
    fn tick_class(&mut self, world: &mut EmberWorld, slot: usize, _timer_before: f32) {
        self.tick_other(world, slot);
    }
}

/// `0xB91F0(rec)`: zero the record, then `-1` at `+0x14`, `+0x18`, `+0x24`, `+0x44`, `+0x1C` and `0` at `+0x48`.
pub fn init_trail_record() -> [u32; TRAIL_WORDS] {
    let mut record = [0u32; TRAIL_WORDS];
    for offset in [0x14usize, 0x18, 0x24, 0x44, 0x1C] {
        record[offset / 4] = u32::MAX;
    }
    record
}

/// `0x153C20`: `table[i] = (f32) fsin(((f32) i * 2^-11) * 2pi)` (x87 sine, 53 bit mantissa) for 2048 entries.
pub fn trig_table() -> &'static [f32] {
    static TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let step = f32::from_bits(0x3A00_0000);
        let two_pi = f32::from_bits(0x40C9_0FDB);
        (0..0x800)
            .map(|index| {
                let angle = (index as f32 * step) * two_pi;
                f64::from(angle).sin() as f32
            })
            .collect()
    })
}

/// `0x3C848C`: float to int helper, truncation toward zero, low dword.
fn ftol(value: f64) -> u32 {
    (value.trunc() as i64) as u32
}

/// One rng draw as the x87 sequence `fild` (+2^32 when negative), `fmul 2^-32`.
fn draw_unit(engine: &mut dyn TickEngine) -> f64 {
    f64::from(engine.rng()) * f64::from(f32::from_bits(0x2F80_0000))
}

fn table(index: u32) -> f32 {
    trig_table()[(index & 0x7FF) as usize]
}

/// The spray of kinds `0x2C..=0x2E` (second half of `0xEFF90`).
fn spray(world: &mut EmberWorld, engine: &mut dyn TickEngine, slot: usize, tick: &TickGlobals) {
    let entry = &world.pool.entries[slot];
    let kind = entry.w[W_KIND];
    let timer = entry.f(W_TIMER);
    if !(f32::from_bits(0x3E80_0000) > timer) {
        return;
    }
    let owner = entry.w[W_OWNER];
    let position = entry.position();
    let count_f = (1.0f32 - (timer * 4.0f32) * 0.5f32) * (tick.steps as f32) * 6.0f32;
    let count = cvtt(count_f);
    let cell = if owner != 0 {
        engine.owner_cell(owner)
    } else {
        entry.w[W_SPRAY_GROUND]
    };
    let mode = u32::from(kind == 0x2E) + 1;
    let pi = f64::from(f32::from_bits(0x4049_0FDB));
    let two_pi = f64::from(f32::from_bits(0x40C9_0FDB));
    let scale = f64::from(f32::from_bits(0x43A2_F983));
    for _ in 0..count.max(0) {
        let u = timer * 4.0f32 + 1.0f32;
        let a = (draw_unit(engine) * pi) as f32;
        let b = draw_unit(engine) * two_pi;
        let ia = ftol(f64::from(a) * scale + 0.5);
        let sin_a = table(ia);
        let ib = ftol(b * scale + 0.5);
        let radius = u * f32::from_bits(0x3E99_999A);
        let mut vx = (table(ib) * radius) * sin_a;
        let mut vz = (table(ib.wrapping_add(0x200)) * radius) * sin_a;
        let mut vy = (u * 0.5f32) * table(ia.wrapping_add(0x200));
        let at = [position[0] + vx, vy + position[1], vz + position[2]];
        let length_squared = (vz * vz + vy * vy) + vx * vx;
        if length_squared > f32::from_bits(0x33D6_BF95) {
            let inverse = (1.0 / length_squared.sqrt()) * 1.0f32;
            vx *= inverse;
            vy *= inverse;
            vz *= inverse;
        }
        engine.spray(SprayCall::Burst {
            mode,
            owner,
            position: at,
            velocity: [vx * 1.5f32, vy * 1.5f32, vz * 1.5f32],
        });
        engine.spray(SprayCall::Streak {
            position: at,
            velocity: [vx * 6.0f32, (vy * 0.75f32) * 6.0f32, vz * 6.0f32],
            cell,
        });
    }
}

/// `0xEFF90(entry)`: the per frame update of an effect entry (see the module comment).
pub fn effect_update(
    world: &mut EmberWorld,
    engine: &mut dyn TickEngine,
    slot: usize,
    tick: &TickGlobals,
) {
    let flags = world.pool.entries[slot].w[W_FLAGS];
    if flags & FLAG_WAITING != 0 && world.pool.entries[slot].f(W_TIMER) >= 0.0 {
        world.pool.entries[slot].w[W_FLAGS] = flags & !FLAG_WAITING;
        if engine.start_entry(world, slot) == 0 {
            return;
        }
    }
    let flags = world.pool.entries[slot].w[W_FLAGS];
    if flags & FLAG_WAITING != 0 {
        return;
    }
    let timer = world.pool.entries[slot].f(W_TIMER);
    if timer > 1.25f32 {
        world.pool.release(engine, slot);
        return;
    }
    if flags & FLAG_NO_TRAIL == 0 {
        let entry = &world.pool.entries[slot];
        let kind = entry.w[W_KIND] as usize;
        let row = world.rows.get(kind).copied().unwrap_or_default();
        let mut record = init_trail_record();
        let owner = entry.w[W_OWNER];
        record[0] = owner;
        record[1] = entry.w[W_SOURCE_COPY];
        record[2] = if owner == 0 {
            entry.w[W_SOURCE_COPY]
        } else {
            owner
        };
        record[3] = entry.w[W_SOURCE_PTR];
        record[4] = entry.w[W_LINK];
        record[5] = entry.w[W_EXTRA];
        record[6] = EFFECT_TYPE;
        record[7] = entry.w[W_KIND];
        record[10] = world
            .pool
            .base
            .wrapping_add(slot as u32 * ENTRY_BYTES)
            .wrapping_add(4 * W_POS as u32);
        let light = if world.globals.mode_byte == 0xA {
            row.light_radius
        } else {
            row.light_radius * f32::from_bits(0x3F33_3333)
        };
        record[0x15] = light.to_bits();
        record[0x16] = row.word_8;
        if entry.w[W_KIND] == 5 {
            record[0x13] = 0x3F59_999A;
            record[0x14] = 4.0f32.to_bits();
        }
        engine.trail_with(world, slot, &record, timer, tick);
    }
    let kind = world.pool.entries[slot].w[W_KIND];
    if (0x2C..=0x2E).contains(&kind) {
        spray(world, engine, slot, tick);
    }
}

/// `0xDC7B0()`: the pool tick over every slot of the pool (the original loops `0xB89C / 0x22C` = 85 slots).
pub fn pool_tick(world: &mut EmberWorld, engine: &mut dyn TickEngine, tick: &TickGlobals) {
    let rate = world.globals.frame_rate as f32;
    for slot in 0..world.pool.entries.len() {
        let entry_type = world.pool.entries[slot].w[W_TYPE];
        if entry_type == u32::MAX {
            continue;
        }
        if tick.steps > 0 && world.pool.entries[slot].w[W_FLAGS] & FLAG_RELEASE_CHECK != 0 {
            if world.pool.entries[slot].w[W_PROBE] == 0 {
                world.pool.release(engine, slot);
                continue;
            }
            world.pool.entries[slot].w[W_PROBE] = 0;
        }
        let entry = &mut world.pool.entries[slot];
        let timer_before = entry.f(W_TIMER);
        let timer = tick.scale / rate + timer_before;
        entry.set_f(W_TIMER, timer);
        entry.w[W_PAIR] = (entry.w[W_PAIR] & 0xFFFF_0000) | 1;
        if entry_type == EFFECT_TYPE {
            effect_update(world, engine, slot, tick);
        } else {
            engine.tick_class(world, slot, timer_before);
        }
    }
}

const _: () = assert!(ENTRY_WORDS * 4 == ENTRY_BYTES as usize);
