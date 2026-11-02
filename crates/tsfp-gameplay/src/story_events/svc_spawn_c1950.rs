//! T1856: the object spawner `0xC1950` and its pure callees (decoded from `build/default.xbe`, MEASURED against the
//! original under the Unicorn oracle, see `docs/t-story-event-spawn-c1950-t1856.md` and
//! `tests/story_events_t1856_spawn_c1950.rs`). Names are INFERRED, every address, offset and constant is MEASURED.
//!
//! Layering: [`spawn_object`] is the exact port of `0xC1950` over a [`SpawnCallees`] (its twelve direct callees).
//! [`NativeCallees`] implements those callees with the ported pure functions of this module (record table lookup
//! `0xBE860`, rng `0x153B40`, motion init `0x966E0`, callback slots `0xA5CE0`, pool allocation `0xA6F90`/`0xA6EC0`,
//! child detach `0x10E110`/`0x10E070`, ...) and leaves the true engine systems behind [`SpawnLeaves`].

use super::strip_update::GuestMemory;

/// Guest globals touched by the spawner (all MEASURED).
pub mod addr {
    /// Frame counter (dword).
    pub const FRAME_COUNTER: u32 = 0x7DE318;
    /// Game mode byte: `0xA` selects the arcade variants of tables and templates, `6` and `2` gate level specific code.
    pub const MODE: u32 = 0x7DE455;
    /// Slot state word of the game: `6` or `7` selects the replay id path of `0x358160`.
    pub const STATE: u32 = 0x7DE30C;
    /// Dynamic slot table: 30 records of 0xD8 bytes (first dword = owning object).
    pub const SLOT_RECORDS: u32 = 0x7B0D20;
    /// Per slot "in use" dwords (30).
    pub const SLOT_USED: u32 = 0x7B2700;
    /// Per slot allocation frame stamp dwords (30).
    pub const SLOT_STAMP: u32 = 0x7B2680;
    pub const SLOT_COUNT: u32 = 30;
    pub const SLOT_STRIDE: u32 = 0xD8;
    /// Static slot ring: base pointer, next index (limit 0x78).
    pub const STATIC_BASE: u32 = 0x732E60;
    pub const STATIC_NEXT: u32 = 0x732E64;
    pub const SPAWN_COUNT: u32 = 0x732E6C;
    pub const SPAWN_FLAG: u32 = 0x732E68;
    /// Dummy object used when an object has no child (flag clear target).
    pub const DUMMY_OBJECT: u32 = 0x7AD288;
    /// Integer scaled into the fade timer of evicted slot objects.
    pub const FADE_SOURCE: u32 = 0x4E8CEC;
    /// Default spawn templates (normal, arcade).
    pub const TEMPLATE_NORMAL: u32 = 0x4DB9CC;
    pub const TEMPLATE_ARCADE: u32 = 0x4DB9A8;
    /// Callback installed in slot 3 of every spawned object (a code address, never executed here).
    pub const CALLBACK_SLOT3: u32 = 0xC1850;
    /// Linear congruential generator state (low, high) and draw counter.
    pub const RNG_STATE: u32 = 0x4E7950;
    pub const RNG_COUNT: u32 = 0x7497C8;
    /// Callback list heads (slot 0, 8, 9): array, count.
    pub const LIST0: (u32, u32) = (0x7B4FC0, 0x7B6258);
    pub const LIST8: (u32, u32) = (0x7161C8, 0x7B4FA8);
    pub const LIST9: (u32, u32) = (0x716148, 0x7B625C);
    /// Model to descriptor index word table and descriptor array pointer (0x30 byte descriptors).
    pub const MODEL_INDEX: u32 = 0x78B860;
    pub const DESCRIPTORS: u32 = 0x791D4C;
    pub const POOL_MODE: u32 = 0x79094C;
    /// Child list: head, tail, free head, count, active hint.
    pub const CHILD_HEAD: u32 = 0x73BD54;
    pub const CHILD_TAIL: u32 = 0x73BD58;
    pub const CHILD_FREE: u32 = 0x73BD50;
    pub const CHILD_COUNT: u32 = 0x73BD5C;
    pub const CHILD_ACTIVE: u32 = 0x7AD28C;
    /// Table removal arrays of `0x2B26F0`.
    pub const TABLE_COUNT: u32 = 0x75F6E8;
    pub const TABLE_KEYS: u32 = 0x75F670;
    pub const TABLE_VALUES: u32 = 0x75F5F8;
    /// Id source of `0x358160`.
    pub const ID_PRESENT: u32 = 0x76B108;
    pub const ID_MODE: u32 = 0x76B140;
    pub const ID_REPLAY: u32 = 0x541EA4;
    pub const ID_COUNTER: u32 = 0x519750;
    /// Class table of `0xBE860` kind 0x40 and its index indirection (`0x1B99B0`).
    pub const CLASS_BASE: u32 = 0x4EA484;
    pub const CLASS_INDEX: u32 = 0x4FA138;
    pub const CLASS_POOL: u32 = 0x4F9BAC;
    /// Per mode default word table of `0x966E0`.
    pub const MOTION_TABLE: u32 = 0x7E03C0;
}

/// `[0x4760D0]`: a template scale equal to this means "unset".
pub const UNSET_SCALE: f32 = -1.0;
/// `[0x475D20]`: factor of the fade timer.
pub const FADE_FACTOR: f32 = 1.5;
/// `[0x475CCC]`: 2^32, converts the signed rng dword to unsigned.
pub const TWO_POW_32: f32 = 4_294_967_296.0;
/// `[0x4785D0]`: 2^-32.
pub const INV_TWO_POW_32: f32 = 1.0 / 4_294_967_296.0;
/// `[0x475C78]`: the neutral motion scale; any other value starts the motion init.
pub const NEUTRAL_SCALE: f32 = 1.0;
/// The unset scale is replaced by the dword at `[0x4DB9B8]` (arcade mode) or `[0x4DB9DC]`, which are the `+0x10`
/// fields of the two default templates themselves (MEASURED: a default template whose own scale is -1.0 keeps it).
pub const DEFAULT_SCALE_ARCADE: u32 = 0x4D_B9B8;
pub const DEFAULT_SCALE_NORMAL: u32 = 0x4D_B9DC;

fn rd_f(mem: &dyn GuestMemory, address: u32) -> f32 {
    f32::from_bits(mem.read_u32(address))
}

fn wr_f(mem: &mut dyn GuestMemory, address: u32, value: f32) {
    mem.write_u32(address, value.to_bits());
}

/// `cvttss2si`: truncation, out of range and NaN give `0x80000000`.
pub fn cvtt(value: f32) -> i32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

// ---------------------------------------------------------------------------------------------------------
// Pure callees
// ---------------------------------------------------------------------------------------------------------

/// `0x1B99B0(index)`: class record index through two tables (MEASURED), 0 when out of range.
pub fn class_index_1b99b0(mem: &dyn GuestMemory, index: u32) -> u32 {
    let signed = index as i32;
    if !(0..0x46).contains(&signed) {
        return 0;
    }
    let value = mem.read_u32(index.wrapping_mul(0x3C).wrapping_add(addr::CLASS_INDEX)) as i32;
    if !(0..0x42).contains(&value) {
        return 0;
    }
    let pool = mem.read_u32(addr::CLASS_POOL);
    mem.read_u32(
        (value as u32)
            .wrapping_mul(0x29C)
            .wrapping_add(pool)
            .wrapping_add(0x14),
    )
}

/// `0xBE860(kind, index)`: spawn record table lookup, record stride 0x54 (0xE0 for kind 0x40), 0 for unknown kinds
/// or a negative index (MEASURED). Kinds: 1, 2, 8, 0x10, 0x20, 0x40 (through [`class_index_1b99b0`]), 0x80, 0x100,
/// 0x200, 0x400, 0x800 (fixed record, ignores the index) and 0x1000.
pub fn lookup_record(mem: &dyn GuestMemory, kind: u32, index: u32) -> u32 {
    if (index as i32) < 0 {
        return 0;
    }
    let scaled = index.wrapping_mul(0x54);
    let kind_signed = kind as i32;
    let base = if kind_signed > 0x80 {
        if kind_signed > 0x400 {
            match kind {
                0x800 => return 0x4DC568,
                0x1000 => 0x4DC768,
                _ => return 0,
            }
        } else {
            match kind {
                0x400 => 0x4DC510,
                0x100 => 0x4DC768,
                0x200 => 0x4DDAE8,
                _ => return 0,
            }
        }
    } else if kind == 0x80 {
        0x4DC3C0
    } else {
        let k = kind.wrapping_sub(1);
        if k > 0x3F {
            return 0;
        }
        match k {
            0 => 0x4DC218,
            1 => 0x4FB190,
            7 => 0x4DBA38,
            15 => 0x4DBE28,
            31 => 0x4DC668,
            63 => {
                let class = class_index_1b99b0(mem, index) as i32;
                if class <= 0 || class >= 0x2C {
                    return 0;
                }
                return (class as u32).wrapping_mul(0xE0).wrapping_add(0x4EA484);
            }
            _ => return 0,
        }
    };
    scaled.wrapping_add(base)
}

/// `0x153B40()`: 64 bit LCG `state = state * 0x343FD + 0x269EC3`, returns the low dword, counts the draws (MEASURED).
pub fn rng_next(mem: &mut dyn GuestMemory) -> u32 {
    let count = mem.read_u32(addr::RNG_COUNT).wrapping_add(1);
    mem.write_u32(addr::RNG_COUNT, count);
    let low = mem.read_u32(addr::RNG_STATE);
    let high = mem.read_u32(addr::RNG_STATE + 4);
    let state = (u64::from(high) << 32 | u64::from(low))
        .wrapping_mul(0x343FD)
        .wrapping_add(0x26_9EC3);
    mem.write_u32(addr::RNG_STATE, state as u32);
    mem.write_u32(addr::RNG_STATE + 4, (state >> 32) as u32);
    state as u32
}

/// `0x10E6F0(object)`: clears the byte at `+0x12` of a non null object.
pub fn clear_child_flag(mem: &mut dyn GuestMemory, object: u32) {
    if object != 0 {
        mem.write_u8(object.wrapping_add(0x12), 0);
    }
}

/// `0x2B26F0(key)`: removes `key` from the parallel arrays (swap with the last entry), returns the value that was
/// stored with it or -1 (MEASURED).
pub fn table_remove(mem: &mut dyn GuestMemory, key: u32) -> u32 {
    let count = mem.read_u32(addr::TABLE_COUNT) as i32;
    if count <= 0 {
        return u32::MAX;
    }
    let mut index = 0u32;
    while mem.read_u32(addr::TABLE_KEYS.wrapping_add(4 * index)) != key {
        index += 1;
        if index as i32 >= count {
            return u32::MAX;
        }
    }
    let n = count as u32;
    let old = mem.read_u32(addr::TABLE_VALUES.wrapping_add(4 * index));
    let last_key = mem.read_u32(addr::TABLE_KEYS.wrapping_add(4 * n).wrapping_sub(4));
    mem.write_u32(addr::TABLE_KEYS.wrapping_add(4 * index), last_key);
    let last_value = mem.read_u32(addr::TABLE_VALUES.wrapping_add(4 * n).wrapping_sub(4));
    mem.write_u32(addr::TABLE_VALUES.wrapping_add(4 * index), last_value);
    mem.write_u32(addr::TABLE_COUNT, count.wrapping_sub(1) as u32);
    old
}

/// Removes `object` from a pointer list `(array, count)` (the shared tail of `0x9E690` and `0xA5CE0`).
fn list_remove(mem: &mut dyn GuestMemory, list: (u32, u32), object: u32) -> bool {
    let count = mem.read_u32(list.1) as i32;
    if count <= 0 {
        return false;
    }
    let Some(found) =
        (0..count as u32).find(|&i| mem.read_u32(list.0.wrapping_add(4 * i)) == object)
    else {
        return false;
    };
    for i in found..count as u32 - 1 {
        let next = mem.read_u32(list.0.wrapping_add(4 * (i + 1)));
        mem.write_u32(list.0.wrapping_add(4 * i), next);
    }
    mem.write_u32(list.1, (count - 1) as u32);
    true
}

/// `0x9E690(object)`: removes the object from the slot 9 list.
pub fn list9_remove(mem: &mut dyn GuestMemory, object: u32) {
    list_remove(mem, addr::LIST9, object);
}

/// `0xA5CE0(object, slot, function)`: stores the callback `function` in slot `slot` (`object + 0x154 + 4 * slot`),
/// stamps `object + 0x90` with the frame counter and maintains the global lists for the slots 0, 8 and 9: a non
/// zero function registers the object once, zero removes it (MEASURED).
pub fn set_callback(mem: &mut dyn GuestMemory, object: u32, slot: u32, function: u32) {
    let frame = mem.read_u32(addr::FRAME_COUNTER);
    mem.write_u32(object.wrapping_add(0x90), frame);
    mem.write_u32(
        object
            .wrapping_add(slot.wrapping_mul(4))
            .wrapping_add(0x154),
        function,
    );
    let list = match slot {
        0 => addr::LIST0,
        8 => addr::LIST8,
        9 => {
            if function == 0 {
                list9_remove(mem, object);
                return;
            }
            let count = mem.read_u32(addr::LIST9.1);
            let present = (0..count as i32)
                .any(|i| mem.read_u32(addr::LIST9.0.wrapping_add(4 * i as u32)) == object);
            if !present {
                mem.write_u32(addr::LIST9.0.wrapping_add(4 * count), object);
                mem.write_u32(addr::LIST9.1, count.wrapping_add(1));
            }
            return;
        }
        _ => return,
    };
    if function == 0 {
        list_remove(mem, list, object);
        return;
    }
    let count = mem.read_u32(list.1);
    let present =
        (0..count as i32).any(|i| mem.read_u32(list.0.wrapping_add(4 * i as u32)) == object);
    if !present {
        mem.write_u32(list.0.wrapping_add(4 * count), object);
        mem.write_u32(list.1, count.wrapping_add(1));
    }
}

/// `0x966E0(object, mode, scale)`: resets the motion block of an object (offsets `0xC8..0x1A4`), stores the mode
/// (`+0xD4`), the default word `i16[0x7E03C0 + 2 * mode]` (`+0xD8`) and the scale (`+0x108`). In mode 1 an object
/// whose owner `[object]` has the callback `0x5CEF0` or `0x5FEB0` at `+0x15C` gets that owner's slot 2 cleared.
pub fn init_motion(mem: &mut dyn GuestMemory, object: u32, mode: u32, scale: f32) {
    let at = |offset: u32| object.wrapping_add(offset);
    mem.write_u32(at(0xD4), mode);
    let word = read_u16(mem, addr::MOTION_TABLE.wrapping_add(mode.wrapping_mul(2))) as i16;
    mem.write_u32(at(0xD8), word as i32 as u32);
    wr_f(mem, at(0x108), scale);
    for offset in [
        0xDC, 0xE0, 0xFC, 0x100, 0x104, 0xE4, 0xE8, 0xEC, 0xF0, 0xF4, 0xF8, 0x128, 0x124, 0x180,
        0x154, 0x158, 0x15C, 0x13C, 0x140, 0x144, 0x148, 0x14C, 0x150, 0x17C, 0xC8, 0x1A4,
    ] {
        mem.write_u32(at(offset), 0);
    }
    if mode == 1 {
        let owner = mem.read_u32(object);
        let callback = mem.read_u32(owner.wrapping_add(0x15C));
        if callback == 0x5CEF0 || callback == 0x5FEB0 {
            set_callback(mem, owner, 2, 0);
        }
    }
}

/// `0x3533F0()` is the dword `[0x76B108]`; `0x355EC0()` decides whether replay ids are used.
fn replay_ids_active(mem: &dyn GuestMemory) -> bool {
    if mem.read_u32(addr::POOL_MODE) == 0x65 {
        return mem.read_u32(addr::ID_MODE) == 1;
    }
    if mem.read_u32(addr::ID_PRESENT) == 0 {
        return true;
    }
    mem.read_u32(addr::ID_MODE) == 1
}

/// `0x358160()`: id for a new object, 0 without an id source, the saved replay id (once, then -1) in states 6 and
/// 7, else a running counter.
pub fn next_object_id(mem: &mut dyn GuestMemory) -> u32 {
    if mem.read_u32(addr::ID_PRESENT) == 0 {
        return 0;
    }
    if !replay_ids_active(mem) {
        let state = mem.read_u32(addr::STATE);
        if state == 7 || state == 6 {
            let id = mem.read_u32(addr::ID_REPLAY);
            mem.write_u32(addr::ID_REPLAY, u32::MAX);
            return id;
        }
    }
    let id = mem.read_u32(addr::ID_COUNTER);
    mem.write_u32(addr::ID_COUNTER, id.wrapping_add(1));
    id
}

/// Engine systems below the pure callees (render object pools, scripting dispatch, destruction), all kept behind a
/// trait. Addresses are the original functions; every argument is recorded by the tests.
pub trait SpawnLeaves {
    /// `[record + 0x48](arg)`: model hook of a record (tail called by `0xDB2C0`).
    fn model_hook(&mut self, mem: &mut dyn GuestMemory, function: u32, arg: u32) -> u32;
    /// `[0x72F2E0 + ...](object, arg)`: dispatch table entry called by `0x9E330`.
    fn dispatch(&mut self, mem: &mut dyn GuestMemory, function: u32, object: u32, arg: u32);
    /// `0xA6880(model, p1, p2)`: takes an object from the model pool, 0 when exhausted.
    fn pool_get(&mut self, mem: &mut dyn GuestMemory, model: u32, p1: u32, p2: u32) -> u32;
    /// `0x6B060(object, p4)`: initialises a fresh pool object.
    fn init_object(&mut self, mem: &mut dyn GuestMemory, object: u32, p4: u32);
    /// `[descriptor + 0x2C](object, p4)`: per model post allocation hook.
    fn alloc_hook(&mut self, mem: &mut dyn GuestMemory, function: u32, object: u32, p4: u32);
    /// `0x12E30(object, 2)`: engine registration, returns non zero when handled.
    fn post_alloc(&mut self, mem: &mut dyn GuestMemory, object: u32, mode: u32) -> u32;
    /// `0x10C0E0(object)`: detaches the object from the render lists.
    fn detach(&mut self, mem: &mut dyn GuestMemory, object: u32);
    /// `0xA7090(object)`: full object destruction (220 instructions and about 16 callees, not ported).
    fn destroy(&mut self, mem: &mut dyn GuestMemory, object: u32);
}

/// `0x9E330(object, a, b)`: calls the dispatch entry for (power of two class of `[object + 0x20]`, `a`), returns 1
/// when an entry exists.
pub fn dispatch_9e330<L: SpawnLeaves + ?Sized>(
    mem: &mut dyn GuestMemory,
    leaves: &mut L,
    object: u32,
    a: u32,
    b: u32,
) -> u32 {
    let mut value = mem.read_u32(object.wrapping_add(0x20)) as i32;
    let mut bit: i32 = 0;
    loop {
        if value == 1 {
            break;
        }
        value >>= 1;
        bit += 1;
        if bit >= 0x20 {
            bit = -1;
            break;
        }
    }
    let index = a.wrapping_add((bit.wrapping_mul(6)) as u32);
    let function = mem.read_u32(index.wrapping_mul(4).wrapping_add(0x72_F2E0));
    if function == 0 {
        return 0;
    }
    leaves.dispatch(mem, function, object, b);
    1
}

fn model_slot(mem: &dyn GuestMemory, model: u32) -> u32 {
    let word = read_u16(mem, addr::MODEL_INDEX.wrapping_add(model.wrapping_mul(2))) as i16;
    (word as i32 as u32).wrapping_mul(0x30)
}

/// `0xA6EC0(owner, model, p1, p2, p3, p4)`: pool allocation plus initialisation (MEASURED).
pub fn allocate_a6ec0<L: SpawnLeaves + ?Sized>(
    mem: &mut dyn GuestMemory,
    leaves: &mut L,
    owner: u32,
    model: u32,
    params: [u32; 4],
) -> u32 {
    let [p1, p2, p3, p4] = params;
    let descriptor = model_slot(mem, model).wrapping_add(mem.read_u32(addr::DESCRIPTORS));
    let object = leaves.pool_get(mem, model, p1, p2);
    if object == 0 {
        return 0;
    }
    mem.write_u32(object.wrapping_add(0x20), owner);
    if mem.read_u32(addr::POOL_MODE) != 0x66 {
        dispatch_9e330(mem, leaves, object, 0, p3);
    }
    leaves.init_object(mem, object, p4);
    if owner != 0x10_0000 {
        let hook = mem.read_u32(descriptor.wrapping_add(0x2C));
        if hook != 0 {
            leaves.alloc_hook(mem, hook, object, p4);
        }
    }
    if leaves.post_alloc(mem, object, 2) == 0 {
        let model_again = u32::from(read_u16(mem, object.wrapping_add(0x94)));
        let entry = model_slot(mem, model_again).wrapping_add(mem.read_u32(addr::DESCRIPTORS));
        if mem.read_u32(entry.wrapping_add(0x1C)) != u32::MAX {
            let frame = mem.read_u32(addr::FRAME_COUNTER);
            mem.write_u32(object.wrapping_add(0x90), frame);
            mem.write_u32(object.wrapping_add(0x15C), 0x5CEF0);
        }
    }
    object
}

/// `0xA6F90(model, p1, p2, p3, p4)`: looks up the owner word of the model descriptor and allocates.
pub fn allocate_a6f90<L: SpawnLeaves + ?Sized>(
    mem: &mut dyn GuestMemory,
    leaves: &mut L,
    model: u32,
    params: [u32; 4],
) -> u32 {
    let owner = mem.read_u32(model_slot(mem, model).wrapping_add(mem.read_u32(addr::DESCRIPTORS)));
    allocate_a6ec0(mem, leaves, owner, model, params)
}

/// `0x10E070` (register `esi` = child): unlinks a child record from the child list and puts it at the head of
/// the free list.
pub fn unlink_child<L: SpawnLeaves + ?Sized>(
    mem: &mut dyn GuestMemory,
    leaves: &mut L,
    child: u32,
) {
    leaves.detach(mem, child);
    let at = |offset: u32| child.wrapping_add(offset);
    let owner_cell = mem.read_u32(child);
    if owner_cell != 0 {
        mem.write_u32(owner_cell, 0);
    }
    let prev = mem.read_u32(at(0xF4));
    if prev != 0 {
        let next = mem.read_u32(at(0xF0));
        mem.write_u32(prev.wrapping_add(0xF0), next);
    } else {
        let next = mem.read_u32(at(0xF0));
        mem.write_u32(addr::CHILD_TAIL, next);
    }
    let next = mem.read_u32(at(0xF0));
    if next != 0 {
        let prev = mem.read_u32(at(0xF4));
        mem.write_u32(next.wrapping_add(0xF4), prev);
    } else {
        let prev = mem.read_u32(at(0xF4));
        mem.write_u32(addr::CHILD_HEAD, prev);
    }
    let free = mem.read_u32(addr::CHILD_FREE);
    mem.write_u32(at(0xF0), 0);
    mem.write_u32(at(0xF4), free);
    if free != 0 {
        mem.write_u32(free.wrapping_add(0xF0), child);
    }
    let count = mem.read_u32(addr::CHILD_COUNT).wrapping_sub(1);
    let active = mem.read_u32(addr::CHILD_ACTIVE);
    mem.write_u32(addr::CHILD_FREE, child);
    mem.write_u32(addr::CHILD_COUNT, count);
    if child == active {
        mem.write_u32(addr::CHILD_ACTIVE, 0);
    }
}

fn release_child<L: SpawnLeaves + ?Sized>(mem: &mut dyn GuestMemory, leaves: &mut L, child: u32) {
    if mem.read_u8(child.wrapping_add(8)) & 2 != 0 {
        mem.write_u8(child.wrapping_add(0x12), 0);
        mem.write_u32(child.wrapping_add(0x88), 0);
    } else {
        unlink_child(mem, leaves, child);
    }
}

/// `0x10E110(object)`: releases every child of the object (`+0x1E8` count, `+0x1FC` array) and, with flag
/// `0x80000` set, every list child that names the object as its parent; then clears the count and the flag.
pub fn reset_children<L: SpawnLeaves + ?Sized>(
    mem: &mut dyn GuestMemory,
    leaves: &mut L,
    object: u32,
) {
    let count = mem.read_u32(object.wrapping_add(0x1E8)) as i32;
    if count > 0 {
        let mut index = 0u32;
        loop {
            let array = mem.read_u32(object.wrapping_add(0x1FC));
            let child = mem.read_u32(array.wrapping_add(4 * index));
            release_child(mem, leaves, child);
            index = index.wrapping_add(1);
            let count = mem.read_u32(object.wrapping_add(0x1E8)) as i32;
            if index as i32 >= count {
                break;
            }
        }
    }
    let flagged = mem.read_u32(object.wrapping_add(0x28)) & 0x8_0000 != 0;
    mem.write_u32(object.wrapping_add(0x1E8), 0);
    if !flagged {
        return;
    }
    let mut node = mem.read_u32(addr::CHILD_HEAD);
    while node != 0 {
        let next = mem.read_u32(node.wrapping_add(0xF4));
        if mem.read_u32(node.wrapping_add(0x88)) == object
            && mem.read_u32(node.wrapping_add(4)) & 0x10_0000 == 0
        {
            release_child(mem, leaves, node);
        }
        node = next;
    }
    let flags = mem.read_u32(object.wrapping_add(0x28));
    mem.write_u32(object.wrapping_add(0x28), flags & 0xFFF7_FFFF);
}

// ---------------------------------------------------------------------------------------------------------
// The spawner
// ---------------------------------------------------------------------------------------------------------

/// The direct callees of `0xC1950`, in the original order of use. The logging stubs of the whole root golden
/// replace exactly these twelve.
pub trait SpawnCallees {
    /// `0xBE860(kind, index)`.
    fn lookup_record(&mut self, mem: &mut dyn GuestMemory, kind: u32, index: u32) -> u32;
    /// `0xDB2C0(record)`: model id of a record.
    fn model_for(&mut self, mem: &mut dyn GuestMemory, record: u32) -> u32;
    /// `0xA6F90(model, position, variant, 0, 0)`: returns the new object or 0.
    fn allocate(&mut self, mem: &mut dyn GuestMemory, model: u32, params: [u32; 4]) -> u32;
    /// `0x10E110(object)`.
    fn reset_children(&mut self, mem: &mut dyn GuestMemory, object: u32);
    /// `0x10E6F0(object)`.
    fn clear_child_flag(&mut self, mem: &mut dyn GuestMemory, object: u32);
    /// `0x2B26F0(object)`.
    fn table_remove(&mut self, mem: &mut dyn GuestMemory, object: u32);
    /// `0xA9070(object)`: dispatch plus destroy of an evicted object.
    fn destroy_with_hook(&mut self, mem: &mut dyn GuestMemory, object: u32);
    /// `0xA5CE0(object, slot, function)`.
    fn set_callback(&mut self, mem: &mut dyn GuestMemory, object: u32, slot: u32, function: u32);
    /// `0x966E0(object, mode, scale_bits)`.
    fn init_motion(&mut self, mem: &mut dyn GuestMemory, object: u32, mode: u32, scale: u32);
    /// `0x153B40()`.
    fn random(&mut self, mem: &mut dyn GuestMemory) -> u32;
    /// `0x358160()`.
    fn next_id(&mut self, mem: &mut dyn GuestMemory) -> u32;
    /// `0xA7090(object)`.
    fn destroy(&mut self, mem: &mut dyn GuestMemory, object: u32);
}

/// Callees implemented with the ported pure functions; the engine below them stays behind `L`.
pub struct NativeCallees<L: SpawnLeaves> {
    pub leaves: L,
}

impl<L: SpawnLeaves> SpawnCallees for NativeCallees<L> {
    fn lookup_record(&mut self, mem: &mut dyn GuestMemory, kind: u32, index: u32) -> u32 {
        lookup_record(mem, kind, index)
    }
    fn model_for(&mut self, mem: &mut dyn GuestMemory, record: u32) -> u32 {
        let function = mem.read_u32(record.wrapping_add(0x48));
        let arg = mem.read_u32(record.wrapping_add(4));
        if function == 0 {
            arg
        } else {
            self.leaves.model_hook(mem, function, arg)
        }
    }
    fn allocate(&mut self, mem: &mut dyn GuestMemory, model: u32, params: [u32; 4]) -> u32 {
        allocate_a6f90(mem, &mut self.leaves, model, params)
    }
    fn reset_children(&mut self, mem: &mut dyn GuestMemory, object: u32) {
        reset_children(mem, &mut self.leaves, object);
    }
    fn clear_child_flag(&mut self, mem: &mut dyn GuestMemory, object: u32) {
        clear_child_flag(mem, object);
    }
    fn table_remove(&mut self, mem: &mut dyn GuestMemory, object: u32) {
        table_remove(mem, object);
    }
    fn destroy_with_hook(&mut self, mem: &mut dyn GuestMemory, object: u32) {
        dispatch_9e330(mem, &mut self.leaves, object, 1, 0);
        self.leaves.destroy(mem, object);
    }
    fn set_callback(&mut self, mem: &mut dyn GuestMemory, object: u32, slot: u32, function: u32) {
        set_callback(mem, object, slot, function);
    }
    fn init_motion(&mut self, mem: &mut dyn GuestMemory, object: u32, mode: u32, scale: u32) {
        init_motion(mem, object, mode, f32::from_bits(scale));
    }
    fn random(&mut self, mem: &mut dyn GuestMemory) -> u32 {
        rng_next(mem)
    }
    fn next_id(&mut self, mem: &mut dyn GuestMemory) -> u32 {
        next_object_id(mem)
    }
    fn destroy(&mut self, mem: &mut dyn GuestMemory, object: u32) {
        self.leaves.destroy(mem, object);
    }
}

/// Selection of the slot to evict (`0xC1B85`): the minimum frame stamp strictly below the frame counter, slot 0
/// when none is older.
fn oldest_stamp_slot(mem: &dyn GuestMemory) -> u32 {
    let mut best = mem.read_u32(addr::FRAME_COUNTER) as i32;
    let mut slot = 0;
    for k in 0..addr::SLOT_COUNT {
        let stamp = mem.read_u32(addr::SLOT_STAMP + 4 * k) as i32;
        if stamp < best {
            best = stamp;
            slot = k;
        }
    }
    slot
}

/// `0xC1950(flags, kind, index, a3, position, template, a6, a7, a8, a9)`: allocates an object of `kind` from the
/// record table, binds it to a slot record, and initialises it (MEASURED, cdecl, returns the object or 0).
///
/// * `flags` bit 0: use the dynamic slot table (30 slots, oldest first eviction, fade timers for the older objects
///   when at most three slots are free) instead of the static ring; bit 1: the template decides the object flags.
/// * `kind`: 1, 2, 8, 0x10, 0x20, 0x40, 0x80, 0x100, 0x200, 0x400, 0x800, 0x1000 (see [`lookup_record`]).
/// * `template`: 0 selects the default template of the mode (`0x4DB9A8` arcade, `0x4DB9CC` otherwise); flag byte
///   bit 1 copies the dwords `+0x14,+0x18,+0x1C` into the object, bit 3 clears object flag 1; `+0x10` is the scale
///   (-1.0 means unset: replaced by the dword `[0x4DB9B8]` in arcade mode, `[0x4DB9DC]` otherwise).
///
/// `frame` is the entry `esp` of the original: its locals are written to guest memory at `frame - 0x18..frame`,
/// the defaulted template back to `frame + 0x18` and the random draw to `frame + 8`, exactly as the original does,
/// and `&position_copy` passed to the kind 0x800 child allocation is `frame - 0xC`.
pub fn spawn_object<C: SpawnCallees + ?Sized>(
    mem: &mut dyn GuestMemory,
    callees: &mut C,
    frame: u32,
    args: [u32; 10],
) -> u32 {
    let [
        flags,
        kind,
        index,
        a3,
        position,
        mut template,
        a6,
        a7,
        a8,
        a9,
    ] = args;
    let local = |offset: u32| frame.wrapping_sub(offset);
    mem.write_u32(local(0x14), u32::MAX);
    let record = if kind == 0x40 {
        let class = a8 as i32;
        if (index as i32) < 0 || (index as i32) >= 0x46 || a7 != 0 || class <= 0 || class >= 0x2C {
            callees.lookup_record(mem, 0x40, index)
        } else {
            a8.wrapping_mul(0xE0).wrapping_add(addr::CLASS_BASE)
        }
    } else {
        callees.lookup_record(mem, kind, index)
    };
    let mode = mem.read_u8(addr::MODE);
    let dynamic = flags & 1 != 0;
    let variant = if mode == 0xA || dynamic {
        record.wrapping_add(0x28)
    } else {
        record.wrapping_add(0x3C)
    };
    mem.write_u32(local(0x10), variant);
    if template == 0 {
        template = if mode == 0xA {
            addr::TEMPLATE_ARCADE
        } else {
            addr::TEMPLATE_NORMAL
        };
    }
    mem.write_u32(frame.wrapping_add(0x18), template);
    let object = if dynamic && a6 == 0x20 {
        callees.allocate(mem, 0x754, [position, variant, 0, 0])
    } else if dynamic && a6 == 0x21 {
        callees.allocate(mem, 0x753, [position, variant, 0, 0])
    } else {
        let model = callees.model_for(mem, record);
        callees.allocate(mem, model, [position, variant, 0, 0])
    };
    if kind != 0x80 && kind != 0x400 && kind != 0x20 {
        callees.reset_children(mem, object);
    }
    if kind == 0x20 && mem.read_u8(addr::MODE) == 6 {
        for want_second in [false, true] {
            let count = mem.read_u32(object.wrapping_add(0x1E8)) as i32;
            let threshold = i32::from(want_second);
            let child = if count > threshold {
                let array = mem.read_u32(object.wrapping_add(0x1FC));
                mem.read_u32(array.wrapping_add(4 * u32::from(want_second)))
            } else {
                mem.read_u32(addr::DUMMY_OBJECT)
            };
            callees.clear_child_flag(mem, child);
        }
    }
    let spawned = mem.read_u32(addr::SPAWN_COUNT).wrapping_add(1);
    mem.write_u32(addr::SPAWN_COUNT, spawned);
    let template_flags = mem.read_u8(template);
    mem.write_u32(addr::SPAWN_FLAG, 1);
    if template_flags & 2 != 0 {
        let value = mem.read_u32(template.wrapping_add(0x14));
        mem.write_u32(object.wrapping_add(0xC0), value);
        let value = mem.read_u32(template.wrapping_add(0x18));
        mem.write_u32(object.wrapping_add(0x70), value);
        let value = mem.read_u32(template.wrapping_add(0x1C));
        mem.write_u32(object.wrapping_add(0xC8), value);
    }
    if rd_f(mem, template.wrapping_add(0x10)) == UNSET_SCALE {
        let source = if mem.read_u8(addr::MODE) == 0xA {
            DEFAULT_SCALE_ARCADE
        } else {
            DEFAULT_SCALE_NORMAL
        };
        let value = mem.read_u32(source);
        mem.write_u32(template.wrapping_add(0x10), value);
    }
    if dynamic {
        let mut free_count = 0u32;
        let mut slot = 0u32;
        for k in 0..addr::SLOT_COUNT {
            if mem.read_u32(addr::SLOT_USED + 4 * k) == 0 {
                slot = k;
                free_count += 1;
            }
        }
        if free_count == 0 {
            let victim = oldest_stamp_slot(mem);
            let record_at = victim * addr::SLOT_STRIDE + addr::SLOT_RECORDS;
            let owner = mem.read_u32(record_at);
            if owner != 0 {
                if mem.read_u32(record_at + 0xC) == 0x400 {
                    callees.table_remove(mem, owner);
                }
                let owner = mem.read_u32(record_at);
                callees.destroy_with_hook(mem, owner);
            }
            slot = victim;
            free_count = 1;
        }
        mem.write_u32(
            object.wrapping_add(0x7C),
            slot * addr::SLOT_STRIDE + addr::SLOT_RECORDS,
        );
        let frame_counter = mem.read_u32(addr::FRAME_COUNTER);
        mem.write_u32(local(0x14), slot);
        mem.write_u32(addr::SLOT_USED + 4 * slot, 1);
        mem.write_u32(addr::SLOT_STAMP + 4 * slot, frame_counter);
        if free_count <= 3 {
            mem.write_u32(local(0x18), free_count);
            for _ in 0..free_count {
                fade_oldest_slot(mem);
            }
            mem.write_u32(local(0x18), 0);
        }
    } else {
        let next = mem.read_u32(addr::STATIC_NEXT);
        if next as i32 >= 0x78 {
            callees.destroy(mem, object);
            return 0;
        }
        let base = mem.read_u32(addr::STATIC_BASE);
        mem.write_u32(
            object.wrapping_add(0x7C),
            next.wrapping_mul(addr::SLOT_STRIDE).wrapping_add(base),
        );
        mem.write_u32(addr::STATIC_NEXT, next.wrapping_add(1));
    }
    callees.set_callback(mem, object, 3, addr::CALLBACK_SLOT3);
    let slot_record = mem.read_u32(object.wrapping_add(0x7C));
    let slot_index = mem.read_u32(local(0x14));
    mem.write_u32(slot_record.wrapping_add(8), slot_index);
    mem.write_u32(object.wrapping_add(0x20), 0x40);
    mem.write_u8(object.wrapping_add(0x32), 2);
    let at = |offset: u32| slot_record.wrapping_add(offset);
    mem.write_u32(slot_record, object);
    mem.write_u32(at(4), 0);
    mem.write_u32(at(0x10), flags);
    mem.write_u32(at(0xC), kind);
    mem.write_u32(at(0x18), template);
    mem.write_u32(at(0x14), index);
    mem.write_u32(at(0xC4), 0);
    if kind == 0x800 {
        let child_record = if (index as i32) < 0 {
            0
        } else {
            index.wrapping_mul(0x54).wrapping_add(0x4F_B190)
        };
        let coordinates = [
            mem.read_u32(position),
            mem.read_u32(position.wrapping_add(4)),
            mem.read_u32(position.wrapping_add(8)),
        ];
        mem.write_u32(local(0xC), coordinates[0]);
        mem.write_u32(local(8), coordinates[1]);
        mem.write_u32(local(4), coordinates[2]);
        let variant_again = mem.read_u32(local(0x10));
        let model = callees.model_for(mem, child_record);
        let child = callees.allocate(mem, model, [local(0xC), variant_again, 0, 0]);
        mem.write_u32(at(0xD4), child);
        mem.write_u32(child.wrapping_add(0x20), 0x40);
        mem.write_u8(child.wrapping_add(0x32), 2);
        let child = mem.read_u32(at(0xD4));
        let sum = rd_f(mem, child_record.wrapping_add(0x38)) + rd_f(mem, child.wrapping_add(0x68));
        wr_f(mem, child.wrapping_add(0x68), sum);
        let child = mem.read_u32(at(0xD4));
        callees.reset_children(mem, child);
    }
    let draw = callees.random(mem);
    mem.write_u32(frame.wrapping_add(8), draw);
    let mut unit = f64::from(draw as i32);
    if (draw as i32) < 0 {
        unit += f64::from(TWO_POW_32);
    }
    let scaled = (unit * f64::from(INV_TWO_POW_32)) as f32;
    wr_f(mem, at(0xB8), scaled);
    let slot_now = mem.read_u32(object.wrapping_add(0x7C));
    let (slot_kind, slot_index) = (
        mem.read_u32(slot_now.wrapping_add(0xC)),
        mem.read_u32(slot_now.wrapping_add(0x14)),
    );
    let first = callees.lookup_record(mem, slot_kind, slot_index);
    let selector = |mem: &dyn GuestMemory, slot: u32| {
        mode_byte(mem) == 0xA || mem.read_u32(slot.wrapping_add(0x10)) & 1 != 0
    };
    let scale_field = if selector(mem, slot_now) { 0x20 } else { 0x34 };
    let scale = rd_f(mem, first.wrapping_add(scale_field));
    if scale != NEUTRAL_SCALE {
        let owner = mem.read_u32(object.wrapping_add(4));
        callees.init_motion(mem, owner, 1, 0);
    }
    let slot_now = mem.read_u32(object.wrapping_add(0x7C));
    let (slot_kind, slot_index) = (
        mem.read_u32(slot_now.wrapping_add(0xC)),
        mem.read_u32(slot_now.wrapping_add(0x14)),
    );
    let second = callees.lookup_record(mem, slot_kind, slot_index);
    let slot_now = mem.read_u32(object.wrapping_add(0x7C));
    let offset_field = if selector(mem, slot_now) { 0x24 } else { 0x38 };
    let offset = rd_f(mem, second.wrapping_add(offset_field));
    let height = rd_f(mem, object.wrapping_add(0x68)) + offset;
    wr_f(mem, object.wrapping_add(0x68), height);
    let template = mem.read_u32(frame.wrapping_add(0x18));
    let mut object_flags = mem.read_u32(object.wrapping_add(0x28));
    if mem.read_u8(template) & 8 != 0 {
        object_flags &= !1;
    }
    object_flags |= 0x62;
    mem.write_u32(object.wrapping_add(0x28), object_flags);
    if flags & 2 != 0 {
        if kind == 0x800 {
            let value = mem.read_u32(object.wrapping_add(0x28)) & !2;
            mem.write_u32(object.wrapping_add(0x28), value);
            let child = mem.read_u32(at(0xD4));
            let value = mem.read_u32(child.wrapping_add(0x28)) & !1;
            mem.write_u32(child.wrapping_add(0x28), value);
        } else {
            let value = mem.read_u32(object.wrapping_add(0x28)) & !3;
            mem.write_u32(object.wrapping_add(0x28), value);
        }
    }
    mem.write_u32(at(0x1C), a3);
    if kind == 0x20 && mode_byte(mem) == 2 {
        mem.write_u32(at(0x14), a3.wrapping_sub(1));
    }
    for (offset, source) in [
        (0x20, 0x64),
        (0x24, 0x68),
        (0x28, 0x6C),
        (0x2C, 0xC0),
        (0x30, 0x70),
        (0x34, 0xC8),
    ] {
        let value = mem.read_u32(object.wrapping_add(source));
        mem.write_u32(at(offset), value);
    }
    for offset in [0x38, 0x3C, 0x9C] {
        mem.write_u32(at(offset), u32::MAX);
    }
    for (offset, value) in [(0x40, a6), (0x44, a7), (0x48, a8), (0x4C, a9)] {
        mem.write_u32(at(offset), value);
    }
    mem.write_u32(at(0xA0), 1);
    mem.write_u32(at(0xC0), 3);
    mem.write_u32(at(0xBC), 0);
    mem.write_u32(at(0xCC), 0);
    mem.write_u32(at(0xC8), 0);
    let id = callees.next_id(mem);
    mem.write_u32(object.wrapping_add(0x230), id);
    object
}

fn mode_byte(mem: &dyn GuestMemory) -> u8 {
    mem.read_u8(addr::MODE)
}

/// One pass of the fade loop (`0xC1C80`): the oldest used slot (stamp strictly below the running minimum, starting at
/// the frame counter) whose owner has no fade yet gets `+0x1A0 = 1` and `+0x1A4 = trunc(f * 1.5 * f)` with
/// `f = (float)[0x4E8CEC]`.
fn fade_oldest_slot(mem: &mut dyn GuestMemory) {
    let mut best = mem.read_u32(addr::FRAME_COUNTER) as i32;
    let mut chosen = None;
    for k in 0..addr::SLOT_COUNT {
        if mem.read_u32(addr::SLOT_USED + 4 * k) == 0 {
            continue;
        }
        let stamp = mem.read_u32(addr::SLOT_STAMP + 4 * k) as i32;
        if stamp >= best {
            continue;
        }
        if mem.read_u32(addr::SLOT_RECORDS + addr::SLOT_STRIDE * k) == 0 {
            continue;
        }
        best = stamp;
        chosen = Some(k);
    }
    let Some(k) = chosen else { return };
    let object = mem.read_u32(addr::SLOT_RECORDS + addr::SLOT_STRIDE * k);
    if mem.read_u32(object.wrapping_add(0x1A0)) == 0 {
        mem.write_u32(object.wrapping_add(0x1A0), 1);
        let factor = mem.read_u32(addr::FADE_SOURCE) as i32 as f32;
        let timer = factor * FADE_FACTOR * factor;
        mem.write_u32(object.wrapping_add(0x1A4), cvtt(timer) as u32);
    }
}

// ---------------------------------------------------------------------------------------------------------
// Adapter for StripServices
// ---------------------------------------------------------------------------------------------------------

/// Lets the strip scene driver's `Service::SpawnObject` call run the ported spawner:
///
/// ```ignore
/// Service::SpawnObject => adapter.spawn(memory, args),
/// ```
///
/// `scratch_frame` plays the role of the callee's entry `esp` (any guest address with 0x30 bytes below and 0x2C
/// above that nothing else uses).
pub struct SpawnAdapter<C: SpawnCallees> {
    pub callees: C,
    pub scratch_frame: u32,
}

impl<C: SpawnCallees> SpawnAdapter<C> {
    /// `args` are the ten cdecl arguments of `0xC1950` (missing ones read as 0). Returns the new object or 0.
    pub fn spawn(&mut self, mem: &mut dyn GuestMemory, args: &[u32]) -> u32 {
        let mut full = [0u32; 10];
        for (slot, value) in full.iter_mut().zip(args) {
            *slot = *value;
        }
        spawn_object(mem, &mut self.callees, self.scratch_frame, full)
    }
}

fn read_u16(mem: &dyn GuestMemory, address: u32) -> u16 {
    u16::from(mem.read_u8(address)) | u16::from(mem.read_u8(address.wrapping_add(1))) << 8
}
