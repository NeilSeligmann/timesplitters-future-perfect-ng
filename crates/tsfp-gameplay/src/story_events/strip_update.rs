//! T1846 batch 8: the strip update tail `0x1F2810` (levels 0x21 and 0x22, called by hook `0x1F6620`).
//!
//! Names are INFERRED, every address, offset and constant is MEASURED (decoded and compared against the original
//! machine code under the Unicorn oracle, see `tests/story_events_t1846_strip.rs`). Details and rejected
//! approaches are in `docs/t-story-event-strip-t1846.md`.

// ---------------------------------------------------------------------------------------------------------
// Descriptor table 0x4FFD64
// ---------------------------------------------------------------------------------------------------------

/// One strip kind of the static descriptor table at `0x4FFD64` (stride 0x10, 10 entries, MEASURED).
/// Fields are dwords: `+0` `cells` (number of decoration cells, 0 means none), `+4` `flags` (only the low byte
/// is read: bit 0 = never picked by the random generator, bit 1 = gets a random size mask, bit 2 = unused
/// here), `+8` always 0 (unread), `+0xC` `id` (0x301..=0x309 and 0x310, INFERRED text or asset id, unread here).
/// Even entries are the "A" half of a strip, odd entries the "B" half.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StripKind {
    pub cells: u32,
    pub flags: u32,
    pub id: u32,
}

/// The ten entries read from the XBE static data (numbers only).
pub const STRIP_KINDS: [StripKind; 10] = [
    StripKind {
        cells: 3,
        flags: 0,
        id: 0x301,
    },
    StripKind {
        cells: 3,
        flags: 0,
        id: 0x302,
    },
    StripKind {
        cells: 0,
        flags: 5,
        id: 0x303,
    },
    StripKind {
        cells: 0,
        flags: 5,
        id: 0x304,
    },
    StripKind {
        cells: 3,
        flags: 2,
        id: 0x305,
    },
    StripKind {
        cells: 3,
        flags: 2,
        id: 0x306,
    },
    StripKind {
        cells: 3,
        flags: 0,
        id: 0x307,
    },
    StripKind {
        cells: 3,
        flags: 0,
        id: 0x308,
    },
    StripKind {
        cells: 3,
        flags: 0,
        id: 0x309,
    },
    StripKind {
        cells: 3,
        flags: 0,
        id: 0x310,
    },
];

/// Kind lookup by signed word index (the original shifts the sign extended word by 4). Indices outside the ten
/// entries read other static data in the original and are not modelled: they give an all-zero kind.
pub fn strip_kind(index: i16) -> StripKind {
    usize::try_from(index)
        .ok()
        .and_then(|i| STRIP_KINDS.get(i))
        .copied()
        .unwrap_or(StripKind {
            cells: 0,
            flags: 0,
            id: 0,
        })
}

/// Number of strip slots (the slot index is taken modulo 20).
pub const STRIP_SLOTS: usize = 20;

/// Random source of the original: `0x153B40`, a 64 bit LCG. `state = state * 0x343FD + 0x269EC3` (wrapping,
/// state at `[0x4E7950]` low and `[0x4E7954]` high) and the result is the new low dword. MEASURED.
pub trait StripRng {
    fn next(&mut self) -> u32;
}

/// The concrete `0x153B40` generator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lcg64 {
    pub state: u64,
}

impl StripRng for Lcg64 {
    fn next(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        self.state as u32
    }
}

/// Globals of the strip generator (all MEASURED addresses; the field names are INFERRED).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripTables {
    /// `0x75BCEC`: per slot "A" kind index (even, 0..=8), words.
    pub kind_a: [i16; STRIP_SLOTS],
    /// `0x75BD14`: per slot "B" kind index (odd, 1..=9), words.
    pub kind_b: [i16; STRIP_SLOTS],
    /// `0x75BDF8`: per slot three decoration words for the A half (filled with 0xFFFF on a new slot).
    pub cells_a: [[u16; 3]; STRIP_SLOTS],
    /// `0x75BE70`: per slot three decoration words for the B half.
    pub cells_b: [[u16; 3]; STRIP_SLOTS],
    /// `0x75BDA0`: per slot random size mask of the A half (multiples of 8 up to 0x7F8, or 0).
    pub mask_a: [u32; STRIP_SLOTS],
    /// `0x75BD40`: per slot random size mask of the B half.
    pub mask_b: [u32; STRIP_SLOTS],
    /// `0x4FFE54`: slot filled last (initial value -1).
    pub last_slot: i32,
    /// `0x75BF10`: read position of the scripted queue (word). Values 0..=3 consume the queue, otherwise random.
    pub queue_pos: i16,
    /// `0x75BC20`: scripted A kinds, four words.
    pub queue_a: [i16; 4],
    /// `0x75BFD4`: scripted B kinds, four words.
    pub queue_b: [i16; 4],
    /// `0x75BD90`: up to four follower object handles (0 = empty).
    pub followers: [u32; 4],
    /// `0x75BFF0`: number of follower handles to process (signed).
    pub follower_count: i32,
}

/// The follower object fields the update touches (pool entry stride 0x2E8 at `[0x72F5E8]`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowerView {
    /// `+0x60`: kind tag. `0x41525453` follows the first anchor, `0x4C505453` the second.
    pub kind: u32,
    /// `+0xA8`: offset added for the second kind.
    pub offset: f32,
}

/// Engine object services used by [`strip_update`]. `object` stands for the pool check (index = handle >> 22,
/// the stored handle at `+0x2C` must equal the handle) and `anchor` for the key lookup `0x23FF20`.
pub trait StripObjects {
    /// `None` when the handle is stale (the original then zeroes the follower slot).
    fn follower(&mut self, handle: u32) -> Option<FollowerView>;
    /// `0x23FF20(key)`: the `+0x64` float of the object registered under `key`, `None` when missing or stale.
    fn anchor_value(&mut self, key: u32) -> Option<f32>;
    /// Stores the follower's float at `+0x64`.
    fn set_follower_value(&mut self, handle: u32, value: f32);
}

pub const KIND_FOLLOW_FIRST: u32 = 0x4152_5453;
pub const KIND_FOLLOW_SECOND: u32 = 0x4C50_5453;
pub const ANCHOR_FIRST: u32 = 0x4152_5452;
pub const ANCHOR_SECOND: u32 = 0x4C50_5452;
/// Float constant `[0x4765F0]` added for the first follower kind.
pub const FIRST_KIND_OFFSET: f32 = 128.0;

/// x87 sequence of the original: `trunc(floor(t * speed) * C)` where `t` is the i32 sum fed through `fild`,
/// the product is rounded to a double (exact), `0x3C8501` is `floor` (FPU control word swap to round down) and
/// `0x3C848C` is the truncating `_ftol` (low dword kept). `C` is the double at `[0x49E280]`. The sequence is
/// modelled with the FPU precision control at 53 bits (control word 0x27F, the CRT default, INFERRED: nothing in
/// this function sets it; with 24 or 64 bit precision a last-bit difference could change the slot).
fn slot_phase(frame_sum: i32, speed: f32) -> i32 {
    const C: f64 = f64::from_bits(0x3F6A_C2D6_39D4_E02E);
    let result = ((frame_sum as f64 * speed as f64).floor() * C).trunc();
    if result.abs() >= 9.223_372_036_854_775_8e18 || result.is_nan() {
        0
    } else {
        (result as i64) as i32
    }
}

/// `cvttss2si`: truncate to i32, 0x80000000 when out of range or NaN.
fn cvtt(value: f32) -> i32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

/// Slot the scroll position selects: `0x1F2810` first half. `frame_sum` is `[0x7DE318] + [0x4E7944]` (wrapping
/// i32) and `speed` is `[0x4FFE50]`. Result is in `-19..=19` in theory and `0..=19` in practice.
pub fn strip_slot_index(frame_sum: i32, speed: f32) -> i32 {
    let phase = slot_phase(frame_sum, speed).wrapping_abs().wrapping_rem(20);
    // [0x4DCB0C] = 1800.0, [0x49E388] = 612.232, [0x49E38C] = 0.00653347 (f32 constants)
    let base = cvtt(1800.0_f32 - f32::from_bits(0x4419_0EDF));
    let scaled = (base as f32) * f32::from_bits(0x3BD6_16B2) + phase as f32;
    cvtt(scaled).wrapping_rem(20)
}

/// `0x1EACB0` (register ABI: esi = slot, edi = `&kind_a[slot]`, stack arg = `&kind_b[slot]`): choose the kinds of
/// a new slot. With the queue position in `0..4` the next scripted pair is taken, otherwise random kinds are drawn
/// (A even, B odd, never equal to the previous slot's kind (slot 0 wraps to slot 19) and never with flag bit 0).
pub fn pick_slot_kinds(tables: &mut StripTables, slot: usize, rng: &mut dyn StripRng) {
    let position = tables.queue_pos;
    if (0..4).contains(&position) {
        let at = position as usize;
        tables.kind_a[slot] = tables.queue_a[at];
        tables.kind_b[slot] = tables.queue_b[at];
        tables.queue_pos = position + 1;
        return;
    }
    let previous = (slot + STRIP_SLOTS - 1) % STRIP_SLOTS;
    loop {
        tables.kind_a[slot] = ((rng.next() % 5) * 2) as i16;
        if tables.kind_a[slot] != tables.kind_a[previous]
            && strip_kind(tables.kind_a[slot]).flags & 1 == 0
        {
            break;
        }
    }
    loop {
        tables.kind_b[slot] = ((rng.next() % 5) * 2 + 1) as i16;
        if tables.kind_b[slot] != tables.kind_b[previous]
            && strip_kind(tables.kind_b[slot]).flags & 1 == 0
        {
            break;
        }
    }
}

fn random_mask(rng: &mut dyn StripRng) -> u32 {
    let shift = rng.next() & 1;
    let mask = 0x5555_5555_u32 << shift;
    mask & rng.next() & 0x7F8
}

/// `0x1F2810`: the strip update. `frame_sum` is `[0x7DE318] + [0x4E7944]` (wrapping), `speed` is `[0x4FFE50]`.
/// When the selected slot differs from `last_slot` the slot is regenerated (kinds, 0xFFFF cell words, random
/// decoration cells and size masks, in this exact rng order), then every live follower object is repositioned.
pub fn strip_update(
    tables: &mut StripTables,
    frame_sum: i32,
    speed: f32,
    rng: &mut dyn StripRng,
    objects: &mut dyn StripObjects,
) {
    let slot = strip_slot_index(frame_sum, speed);
    if slot != tables.last_slot {
        let index = slot as usize;
        pick_slot_kinds(tables, index, rng);
        tables.cells_a[index] = [0xFFFF; 3];
        tables.cells_b[index] = [0xFFFF; 3];
        tables.last_slot = slot;
        let kind_a = strip_kind(tables.kind_a[index]);
        if kind_a.cells != 0 && rng.next() & 7 == 0 {
            let value = rng.next() % 7 + 3;
            let cell = (rng.next() % kind_a.cells) as usize;
            tables.cells_b[index][cell] = value as u16;
        }
        let kind_b = strip_kind(tables.kind_b[index]);
        if kind_b.cells != 0 && rng.next() & 7 == 0 {
            let value = rng.next() % 10;
            let cell = (rng.next() % kind_b.cells) as usize;
            tables.cells_a[index][cell] = value as u16;
        }
        tables.mask_a[index] = if kind_a.flags & 2 != 0 {
            random_mask(rng)
        } else {
            0
        };
        tables.mask_b[index] = if kind_b.flags & 2 != 0 {
            random_mask(rng)
        } else {
            0
        };
    }
    for position in 0..usize::try_from(tables.follower_count)
        .unwrap_or(0)
        .min(tables.followers.len())
    {
        let handle = tables.followers[position];
        if handle == 0 {
            continue;
        }
        let Some(view) = objects.follower(handle) else {
            tables.followers[position] = 0;
            continue;
        };
        if view.kind == KIND_FOLLOW_FIRST {
            if let Some(value) = objects.anchor_value(ANCHOR_FIRST) {
                objects.set_follower_value(handle, value + FIRST_KIND_OFFSET);
            }
        }
        if view.kind == KIND_FOLLOW_SECOND {
            if let Some(value) = objects.anchor_value(ANCHOR_SECOND) {
                objects.set_follower_value(handle, view.offset + value);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// 0x1F48A0: the strip spawner (guest memory + engine services model)
// ---------------------------------------------------------------------------------------------------------

/// Byte addressable view of the guest address space for [`strip_spawner`]. Reads of unmapped memory are 0 in the
/// oracle and in reference implementations. All integers are little endian.
pub trait GuestMemory {
    fn read_u8(&self, address: u32) -> u8;
    fn write_u8(&mut self, address: u32, value: u8);
    fn read_u32(&self, address: u32) -> u32;
    fn write_u32(&mut self, address: u32, value: u32);
}

/// Engine services called by `0x1F48A0`. Arguments are the cdecl stack arguments in call order (first = leftmost).
/// Pointer arguments are guest addresses, `frame - k` addresses point into the caller's stack frame (see
/// [`strip_spawner`]). Services that fill output buffers write them through the [`GuestMemory`] they are handed.
/// Names are INFERRED, addresses and arguments MEASURED.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    /// `0x15BA10(tag)` returns an object pointer or 0 (INFERRED: find a world object by four character tag).
    FindObjectA,
    /// `0x59CF0(object)` returns a pointer to the object's class record, whose first dword is the class id.
    ClassOf,
    /// `0x585E0(class, out_state, out_aux)` writes two dwords; `out_state` (signed) is the animation state.
    QueryState,
    /// `0x217B0(effect_id, position_pointer)` returns a handle stored at `object + 0x22C` (INFERRED: start a looping
    /// sound or effect at the object's position).
    StartEffect,
    /// `0x129E0(position4, table, entry, record, pick)` fills the 4 float `position4` (INFERRED: the emitter
    /// offset of a record in the emitter table).
    EmitterOffset,
    /// `0x151E80(matrix, vec4)` transforms the 4 float vector in place by the 4x4 matrix (INFERRED).
    TransformVec,
    /// `0x12B840(position4, direction4, scale_f32_bits)` spawns one particle (INFERRED).
    SpawnParticle,
    /// `0xA14B0(out_matrix, object, 1, 0)` builds the object's world matrix, 16 floats (ported elsewhere).
    WorldMatrix,
    /// `0x23FF20(key)` key list lookup (see [`StripObjects::anchor_value`]), returns an object pointer or 0.
    LookupByKey,
    /// `0x61570(object, mask)` returns non-zero when a flag test passes (INFERRED: object flags and mask).
    FlagTest,
    /// `0xBDA30(0)` returns a dword passed on to [`Service::SpawnObject`] (INFERRED: current player or camera).
    CurrentOwner,
    /// `0xC1950(0, 8, 5, 0, position3, owner, 0, 0, 0, 0)` returns the spawned object or 0.
    SpawnObject,
    /// `0x24D290(arg)` returns non-zero when a condition holds (INFERRED).
    Poll,
    /// `0x163730(tag)` returns an object pointer or 0 (second registry, same tag scheme).
    FindObjectB,
    /// `0x1BA220(controller, mode, 1000)` starts a controller state change (INFERRED).
    SetControllerMode,
    /// `0xBD090(object)` destroys the object (INFERRED).
    Destroy,
}

/// Dispatch of one service call; returns the eax result (0 for void services).
pub trait StripServices {
    fn call(&mut self, memory: &mut dyn GuestMemory, service: Service, args: &[u32]) -> u32;
}

/// Faults the original would take (a hardware divide error).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnerFault {
    /// The emitter table at `[[object + 4] + 4]` has a zero entry count (`div` by zero).
    ZeroEmitterCount,
}

/// `[0x75BFE0]`: the last object spawned at the end of the update (its float at `+0x70` is cleared).
pub const GLOBAL_LAST_SPAWN: u32 = 0x75_BFE0;
/// `[0x75BCE4]`: pending flag cleared once `Poll([0x78E218])` is non-zero.
pub const GLOBAL_PENDING: u32 = 0x75_BCE4;
/// `[0x78E218]`: argument of the poll.
pub const GLOBAL_POLL_ARG: u32 = 0x78_E218;

/// `0x1F48A0` (no arguments, cdecl, no result): scene driver tail of level 0xB (`hooks_big` pushes `CallSub 0x1F48A0`), name INFERRED.
/// `frame` is the value of esp at function entry (the return address slot); the original keeps vectors in its
/// frame and passes pointers to them to services, so the model writes them to `frame - k` in `memory`:
/// `frame - 0x6C` state out, `frame - 0x64` aux out, `frame - 0x60..-0x50` position (x, y, z, 0.0),
/// `frame - 0x50..-0x40` direction (x, y, z, 0.0), `frame - 0x40` world matrix (16 floats).
///
/// 1. Object `PITH` (0x48544950, class 0xBF8): state from `QueryState`; for `0x203 <= state < 0x242` sets
///    `[[obj + 4] + 0x30] = 1`, starts the effect once (`[obj + 0x22C]` negative), then 10 times: picks a random
///    entry of the emitter table `[[obj + 4] + 4]` (count at `[0]`, entries of 200 bytes lie directly before the
///    table address, entry `+0x60` points to records of 10 bytes terminated by a negative byte at `+3`), builds the
///    world matrix, picks a random record, offsets the object position by the emitter (two transforms) and spawns
///    a particle with direction `(-499.57, -1.16, -1.67)` and scale 0.1. Other states reset the effect handle to -1
///    and set `[[obj + 4] + 0x30] = 2`. Then objects below `y < -4.0` are destroyed.
/// 2. Object key `EFAS` (0x53414645, class 0xBF5) with flag 0x2000 spawns an object at `(-422.8, 1.3, -2.4)`.
/// 3. Clears the pending flag, then drives four controllers (`PITH` itself by class 0xB9C, 0xBB0, 0xB9D, then
///    objects `3HTO`, `GUY2`, `GUY3`, `CZS1`) between idle and active modes with flags 1 and 0x2000.
///
/// MEASURED (golden compared, 100+ cases with stubbed services). Random draws come from `rng` in the same order.
pub fn strip_spawner(
    memory: &mut dyn GuestMemory,
    services: &mut dyn StripServices,
    rng: &mut dyn StripRng,
    frame: u32,
) -> Result<(), SpawnerFault> {
    use Service::*;
    let obj = services.call(memory, FindObjectA, &[0x4854_4950]);
    if obj != 0 {
        let class = services.call(memory, ClassOf, &[obj]);
        if memory.read_u32(class) == 0xBF8 {
            services.call(memory, QueryState, &[class, frame - 0x6C, frame - 0x64]);
            let state = memory.read_u32(frame - 0x6C) as i32;
            let owner = memory.read_u32(obj + 4);
            if (0x203..0x242).contains(&state) {
                memory.write_u32(owner + 0x30, 1);
                if (memory.read_u32(obj + 0x22C) as i32) < 0 {
                    let handle = services.call(memory, StartEffect, &[0x3A4, obj + 0x64]);
                    memory.write_u32(obj + 0x22C, handle);
                }
                for _ in 0..10 {
                    spawn_one_particle(memory, services, rng, frame, obj)?;
                }
            } else {
                memory.write_u32(obj + 0x22C, 0xFFFF_FFFF);
                memory.write_u32(owner + 0x30, 2);
            }
        }
        if f32::from_bits(0xC080_0000) > f32::from_bits(memory.read_u32(obj + 0x68)) {
            services.call(memory, Destroy, &[obj]);
        }
    }
    let anchor = services.call(memory, LookupByKey, &[0x5341_4645]);
    if anchor != 0 {
        let class = services.call(memory, ClassOf, &[anchor]);
        if memory.read_u32(class) == 0xBF5
            && services.call(memory, FlagTest, &[anchor, 0x2000]) != 0
        {
            memory.write_u32(frame - 0x60, 0xC3D3_6666);
            memory.write_u32(frame - 0x5C, 0x3FA6_6666);
            memory.write_u32(frame - 0x58, 0xC019_999A);
            let owner = services.call(memory, CurrentOwner, &[0]);
            let spawned = services.call(
                memory,
                SpawnObject,
                &[0, 8, 5, 0, frame - 0x60, owner, 0, 0, 0, 0],
            );
            if spawned != 0 {
                memory.write_u32(spawned + 0x70, 0);
                memory.write_u32(GLOBAL_LAST_SPAWN, spawned);
            }
        }
    }
    if memory.read_u32(GLOBAL_PENDING) != 0 {
        let argument = memory.read_u32(GLOBAL_POLL_ARG);
        if services.call(memory, Poll, &[argument]) != 0 {
            memory.write_u32(GLOBAL_PENDING, 0);
        }
    }
    if obj != 0 && services.call(memory, FlagTest, &[obj, 1]) != 0 {
        let class = services.call(memory, ClassOf, &[obj]);
        let class_id = memory.read_u32(class);
        let controller = memory.read_u32(obj + 0x7C);
        if class_id == 0xB9C || class_id == 0xBB0 {
            memory.write_u8(controller + 0x54E, 0);
            memory.write_u32(controller + 0x94, 0);
        }
        if class_id == 0xB9D {
            memory.write_u32(controller + 0x94, 2);
            memory.write_u8(controller + 0x54C, 0);
            services.call(memory, SetControllerMode, &[controller, 1, 1000]);
        }
    }
    // (tag, class id, idle clear: (offset byte, value) or none, active mode value, mode argument)
    // `3HTO` uses class 0xBE0 and byte +0x562, `GUY2` 0xBD9 and +0x57F, `GUY3` 0xBDA and +0x57F.
    let controllers: [(u32, u32, Option<u32>, u32, u32); 4] = [
        (0x4F54_4833, 0xBE0, Some(0x562), 0x16, 8),
        (0x4755_5932, 0xBD9, Some(0x57F), 0x33, 0xD),
        (0x4755_5933, 0xBDA, Some(0x57F), 0x33, 0xD),
        (0x435A_5331, 0xBD7, None, 0x1D, 0x14),
    ];
    for (tag, class_id, idle_byte, mode, argument) in controllers {
        let object = services.call(memory, FindObjectB, &[tag]);
        if object == 0 {
            continue;
        }
        if let Some(offset) = idle_byte {
            if services.call(memory, FlagTest, &[object, 1]) != 0 {
                let class = services.call(memory, ClassOf, &[object]);
                if memory.read_u32(class) == class_id {
                    let controller = memory.read_u32(object + 0x7C);
                    memory.write_u32(controller + 0x94, 0);
                    memory.write_u8(controller + offset, 0);
                }
            }
        } else if services.call(memory, FlagTest, &[object, 1]) == 0 {
            continue;
        }
        // active part: for CZS1 the single block is the flag 1 test already done above
        let fire = if idle_byte.is_some() {
            services.call(memory, FlagTest, &[object, 0x2000]) != 0
        } else {
            true
        };
        if fire {
            let class = services.call(memory, ClassOf, &[object]);
            if memory.read_u32(class) == class_id {
                let controller = memory.read_u32(object + 0x7C);
                memory.write_u32(controller + 0x94, mode);
                memory.write_u8(controller + 0x54C, 0);
                services.call(memory, SetControllerMode, &[controller, argument, 1000]);
            }
        }
    }
    Ok(())
}

/// One of the ten particle spawns of the `PITH` branch (see [`strip_spawner`]).
fn spawn_one_particle(
    memory: &mut dyn GuestMemory,
    services: &mut dyn StripServices,
    rng: &mut dyn StripRng,
    frame: u32,
    obj: u32,
) -> Result<(), SpawnerFault> {
    let owner = memory.read_u32(obj + 4);
    let table = memory.read_u32(owner + 4);
    let entries = memory.read_u32(table);
    let draw = rng.next();
    if entries == 0 {
        return Err(SpawnerFault::ZeroEmitterCount);
    }
    let index = draw % entries;
    let entry = table.wrapping_add((index.wrapping_sub(entries) as i32).wrapping_mul(200) as u32);
    let mut count = 0u32;
    let mut record = memory.read_u32(entry + 0x60);
    while record != 0 && (memory.read_u8(record + 3) as i8) >= 0 {
        count += 1;
        record = record.wrapping_add(10);
    }
    services.call(memory, Service::WorldMatrix, &[frame - 0x40, obj, 1, 0]);
    if count != 0 {
        for (slot, bits) in [
            (0x60, memory.read_u32(obj + 0x64)),
            (0x5C, memory.read_u32(obj + 0x68)),
            (0x58, memory.read_u32(obj + 0x6C)),
            (0x54, 0),
            (0x50, 0xC3F9_C8F6),
            (0x4C, 0xBF94_7AE1),
            (0x48, 0xBFD5_C28F),
            (0x44, 0),
        ] {
            memory.write_u32(frame - slot, bits);
        }
        let pick = rng.next() % count;
        let record = memory.read_u32(entry + 0x60).wrapping_add(pick * 10);
        if record != 0 && memory.read_u8(record + 2) != 0 {
            let owner = memory.read_u32(obj + 4);
            let sub = rng.next() % u32::from(memory.read_u8(record + 2));
            let table = memory.read_u32(owner + 4);
            services.call(
                memory,
                Service::EmitterOffset,
                &[frame - 0x60, table, entry, record, sub],
            );
            let matrix = (index << 6).wrapping_add(memory.read_u32(owner + 0x10));
            services.call(memory, Service::TransformVec, &[matrix, frame - 0x60]);
            services.call(memory, Service::TransformVec, &[frame - 0x40, frame - 0x60]);
        }
        services.call(
            memory,
            Service::SpawnParticle,
            &[frame - 0x60, frame - 0x50, 0x3DCC_CCCD],
        );
    }
    Ok(())
}
