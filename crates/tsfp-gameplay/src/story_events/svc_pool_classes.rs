//! T1870: the pool class loops `0xDC960` (destroy) and `0xDC9E0` (late update) and the class table `0x4DEB28`.
//!
//! Source `build/default.xbe` (SHA-256 `3cfd001a84fc3e08175d6c4b2e42ebf49577a089b5d0bd87ac724f61a41816bc`), validated against the
//! original under the Unicorn oracle (`tools/t1846_gold/svc_pool_loops.py`, test `tests/story_events_t1870_pool_loops.rs`).
//! Document `docs/t-story-event-pool-classes-t1870.md`. Behaviour MEASURED (xemu level), names INFERRED.
//!
//! Every pool entry type has a 64 byte class record at `0x4DEB28 + 64 * type`; the callbacks are `+0x30` init, `+0x34` update
//! (`0xDC7B0`), `+0x38` destroy (loop `0xDC960`) and `+0x3C` late update (loop `0xDC9E0`). The record is static initialised
//! `.data` of the image: [`ClassTable::from_image`] reads it from the caller's image bytes (nothing is committed).
//!
//! * `0xDC960` ([`destroy_loop`]): for every slot whose type is not `-1`: when `+0x158` (an attached object) is non zero, the object's
//!   cell word `[object + 0x34]` is copied into the entry's `+0xF0`, then into `[owner + 0x34]` (owner `+0xA8`), `[+0x18C] + 0x34` and
//!   `[+0x190] + 0x34` when those pointers are non zero. Then the class `+0x38` callback (when non zero) gets the entry.
//! * `0xDC9E0` ([`late_loop`]): first `0x1C6F0([[0x7B0CC0] + 0x208])` (an engine service), then for every slot whose type is not `-1`
//!   and whose low half of `+0x18` is 1 (set by the tick, so every entry ticked this frame): the class `+0x3C` callback.

use super::svc_effect_tick::{CLASS_STRIDE, CLASS_TABLE};
use super::svc_kind_table::bytes_at;
use super::svc_particle_efbd0::{EmberWorld, W_TYPE};

/// `0x389B0`: a bare `ret`, the do-nothing callback several classes use in every slot.
pub const NOOP_CALLBACK: u32 = 0x0003_89B0;
/// Number of types read from the image (types `0..0x30` exist; the loops never see a type above the table).
pub const CLASS_COUNT: usize = 0x30;
/// Word indices of the entry fields the destroy loop reads.
const W_OBJECT: usize = 0x158 / 4;
const W_OWNER_CELL: usize = 0xA8 / 4;
const W_CELL: usize = 0xF0 / 4;
const W_LINK_B: usize = 0x18C / 4;
const W_LINK_C: usize = 0x190 / 4;
const W_PAIR: usize = 0x18 / 4;
/// Offset of the cell word in a foreign object.
pub const OBJECT_CELL: u32 = 0x34;

/// The callback slot of a class record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassSlot {
    Init,
    Update,
    Destroy,
    Late,
}

impl ClassSlot {
    /// Byte offset in the class record.
    pub fn offset(self) -> u32 {
        match self {
            Self::Init => 0x30,
            Self::Update => 0x34,
            Self::Destroy => 0x38,
            Self::Late => 0x3C,
        }
    }
}

/// The class records `0x4DEB28 + 64 * type` as words.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClassTable {
    pub records: Vec<[u32; 16]>,
}

impl ClassTable {
    /// Read the first [`CLASS_COUNT`] records from the image bytes of the caller, `None` when the image does not map them.
    pub fn from_image(image: &[u8]) -> Option<Self> {
        let raw = bytes_at(image, CLASS_TABLE, CLASS_STRIDE * CLASS_COUNT as u32)?;
        let records = raw
            .chunks_exact(CLASS_STRIDE as usize)
            .map(|record| {
                let mut words = [0u32; 16];
                for (index, word) in words.iter_mut().enumerate() {
                    *word = u32::from_le_bytes([
                        record[4 * index],
                        record[4 * index + 1],
                        record[4 * index + 2],
                        record[4 * index + 3],
                    ]);
                }
                words
            })
            .collect();
        Some(Self { records })
    }

    /// The callback address of a type and slot, 0 when none (or the type has no record).
    pub fn callback(&self, entry_type: u32, slot: ClassSlot) -> u32 {
        self.records
            .get(entry_type as usize)
            .map_or(0, |record| record[(slot.offset() / 4) as usize])
    }
}

/// Engine side of the loops.
pub trait ClassEngine {
    /// The class callback address (0 = none) of a type and slot.
    fn callback(&self, entry_type: u32, slot: ClassSlot) -> u32;
    /// Run the callback `address` of class slot `which` on pool slot `slot`.
    fn run_callback(&mut self, world: &mut EmberWorld, slot: usize, which: ClassSlot, address: u32);
    /// A word of a foreign object (guest memory).
    fn read_word(&mut self, address: u32) -> u32;
    fn write_word(&mut self, address: u32, value: u32);
    /// `0x1C6F0([[0x7B0CC0] + 0x208])`: the prelude of the late loop.
    fn late_prelude(&mut self);
}

/// `0xDC960()`: the destroy loop.
pub fn destroy_loop(world: &mut EmberWorld, engine: &mut dyn ClassEngine) {
    for slot in 0..world.pool.entries.len() {
        let entry_type = world.pool.entries[slot].w[W_TYPE];
        if entry_type == u32::MAX {
            continue;
        }
        let object = world.pool.entries[slot].w[W_OBJECT];
        if object != 0 {
            let owner = world.pool.entries[slot].w[W_OWNER_CELL];
            let cell = engine.read_word(object.wrapping_add(OBJECT_CELL));
            world.pool.entries[slot].w[W_CELL] = cell;
            if owner != 0 {
                engine.write_word(owner.wrapping_add(OBJECT_CELL), cell);
            }
            for link in [W_LINK_B, W_LINK_C] {
                let target = world.pool.entries[slot].w[link];
                if target != 0 {
                    let cell = world.pool.entries[slot].w[W_CELL];
                    engine.write_word(target.wrapping_add(OBJECT_CELL), cell);
                }
            }
        }
        let address = engine.callback(entry_type, ClassSlot::Destroy);
        if address != 0 {
            engine.run_callback(world, slot, ClassSlot::Destroy, address);
        }
    }
}

/// The class INIT callback (`+0x30`) of one slot, run by the spawn code once the entry is filled (T1876): the callback of the entry's class, when it has one.
pub fn class_init(world: &mut EmberWorld, engine: &mut dyn ClassEngine, slot: usize) {
    let entry_type = world.pool.entries[slot].w[W_TYPE];
    let address = engine.callback(entry_type, ClassSlot::Init);
    if address != 0 {
        engine.run_callback(world, slot, ClassSlot::Init, address);
    }
}

/// `0xDC9E0()`: the late update loop.
pub fn late_loop(world: &mut EmberWorld, engine: &mut dyn ClassEngine) {
    engine.late_prelude();
    for slot in 0..world.pool.entries.len() {
        let entry = &world.pool.entries[slot];
        let entry_type = entry.w[W_TYPE];
        if entry_type == u32::MAX || entry.w[W_PAIR] & 0xFFFF != 1 {
            continue;
        }
        let address = engine.callback(entry_type, ClassSlot::Late);
        if address != 0 {
            engine.run_callback(world, slot, ClassSlot::Late, address);
        }
    }
}

/// Word index of the entry fields the source flag masking reads.
const W_SOURCE: usize = 0x108 / 4;
const W_FLAGS: usize = 3;
/// Bits of the object flags `+0x28` that are masked around a class update (`0x60`).
pub const MASKED_BITS: u32 = 0x60;
/// Timer below which the masking applies (`[0x475CE8]` = 0.05).
pub const MASK_WINDOW: f32 = f32::from_bits(0x3D4C_CCCD);

/// The foreign objects whose flags `0xDC7B0` masks around the class update: the source object (`+0x108`), and for an actor (type 8) the
/// two objects its player record (`+0x7C`) links at `+0x18C` and `+0x2BC`.
#[derive(Default)]
struct Masked {
    source: u32,
    source_bits: u32,
    second: u32,
    second_bits: u32,
    third: u32,
    third_bits: u32,
}

fn clear_bits(engine: &mut dyn ClassEngine, object: u32) -> u32 {
    let flags = engine.read_word(object.wrapping_add(0x28));
    engine.write_word(object.wrapping_add(0x28), flags & !MASKED_BITS);
    flags & MASKED_BITS
}

/// The class update of one slot (`0xDC7B0` after the timer advance; the masking decides on `timer_before`, the timer before the advance): the `+0x34` callback of the entry's class, with the `0x60` flag bits of
/// the source object (and of the two objects behind an actor's player record) cleared around it and restored after. Entries of type
/// `0x17` and entries without a source, with a timer of at least `0.05` (or NaN) or with flag `0x100` take no masking.
pub fn class_tick(
    world: &mut EmberWorld,
    engine: &mut dyn ClassEngine,
    slot: usize,
    timer_before: f32,
) {
    let entry = &world.pool.entries[slot];
    let entry_type = entry.w[W_TYPE];
    let source = entry.w[W_SOURCE];
    let timer = timer_before;
    let mut masked: Option<Masked> = None;
    if source != 0 && MASK_WINDOW > timer && entry_type != 0x17 && entry.w[W_FLAGS] & 0x100 == 0 {
        let mut state = Masked {
            source,
            source_bits: clear_bits(engine, source),
            ..Masked::default()
        };
        if engine.read_word(source.wrapping_add(0x20)) == 8 {
            let player = engine.read_word(source.wrapping_add(0x7C));
            let second = engine.read_word(player.wrapping_add(0x18C));
            if second != 0 {
                state.second = second;
                state.second_bits = clear_bits(engine, second);
            }
            let third = engine.read_word(player.wrapping_add(0x2BC));
            if third != 0 {
                state.third = third;
                state.third_bits = clear_bits(engine, third);
            }
        }
        masked = Some(state);
    }
    let address = engine.callback(entry_type, ClassSlot::Update);
    if address != 0 {
        engine.run_callback(world, slot, ClassSlot::Update, address);
    }
    let Some(state) = masked else {
        return;
    };
    let flags = engine.read_word(state.source.wrapping_add(0x28));
    engine.write_word(state.source.wrapping_add(0x28), flags | state.source_bits);
    if engine.read_word(state.source.wrapping_add(0x20)) != 8 {
        return;
    }
    let player = engine.read_word(state.source.wrapping_add(0x7C));
    if state.second != 0 && state.second == engine.read_word(player.wrapping_add(0x18C)) {
        let flags = engine.read_word(state.second.wrapping_add(0x28));
        engine.write_word(state.second.wrapping_add(0x28), flags | state.second_bits);
    }
    if state.third != 0 && state.third == engine.read_word(player.wrapping_add(0x2BC)) {
        let flags = engine.read_word(state.third.wrapping_add(0x28));
        engine.write_word(state.third.wrapping_add(0x28), flags | state.third_bits);
    }
}
