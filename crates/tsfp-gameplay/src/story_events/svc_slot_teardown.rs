//! T1856: the effect slot flush `0x103A00(0, slot)` and its closure (about 2500 instructions), ported bit exact.
//!
//! The sweep module used to name this call `release_slot` (renamed `build_debris` in T1864; the claim and reap code of the 4 contact effect slots calls it for
//! an evicted or reaped slot), but the measured behaviour is the opposite of a teardown: it BUILDS a debris effect from
//! the contact data the slot collected. It acquires a pooled object of class `0x15` with a `0x619C` byte working block
//! from the arena allocator, fills 80 patches (a matrix, a vertex quad, attribute quad, texture quad, velocity and
//! life per patch) from a randomised fan around the contact point, and finishes with a bounding box query of the
//! entity tree. The trait method is now [`ImpactServices::build_debris`] (T1864 rename) after the measured
//! meaning. Every address is MEASURED against `build/default.xbe` (SHA-256
//! `3cfd001a84fc3e08175d6c4b2e42ebf49577a089b5d0bd87ac724f61a41816bc`) under Unicorn, names are INFERRED.
//!
//! Port structure (all state lives in guest memory, accessed through [`EffectMemory`]):
//!
//! * [`Machine::spawn_debris`] `0x103A00`, [`Machine::build_patch`] `0x1028E0`, [`Machine::entity_query`] `0x873C0`
//!   with `0x87310`, `0x157440`, `0x1571C0`, `0x87260`, `0x47560`;
//! * pooled objects: [`Machine::pool_acquire`] `0xFF2B0`, [`Machine::pool_release`] `0xFF200`;
//! * arena allocator: [`Machine::arena_alloc_mode`] `0x3ED20`, [`Machine::arena_alloc`] `0x3EBD0`,
//!   [`Machine::arena_make_room`] `0x3EAF0`, [`Machine::arena_free_block`] `0x3EA40`, [`Machine::arena_free`] `0x3EAD0`,
//!   [`Machine::arena_reserve`] `0x3ECA0`;
//! * small leaves: `0x80F20` [`texture_index`], `0x153550` [`write_translation_matrix`], `0x153BB0` and `0x153BD0`
//!   [`Machine::rng_int`], [`Machine::rng_float`], the sine table initialiser `0x153C20` [`sine_table_entry`].
//!
//! x87 arithmetic runs with the 53 bit mantissa control word `0x27F`, so it is modelled with `f64` and rounded to `f32`
//! at each `fstp`. SSE arithmetic is `f32`. `rsqrtss` is modelled as `1 / sqrt` in `f32` (QEMU, so xemu level, not
//! silicon). `fpatan` uses `f64::atan2` (QEMU computes it in extended precision; the result only feeds a table index).
//! The arena block header and all other layouts are documented in `docs/t-story-event-slot-teardown-t1856.md`.

use super::sweep_dynamic_shape::{EffectMemory, ImpactServices};
use std::cell::RefCell;

// ---------------------------------------------------------------------------------------------------------------
// Addresses of the engine globals used by the closure (MEASURED)
// ---------------------------------------------------------------------------------------------------------------

/// Pooled object class table: 16 bytes per class `{max count (-1 none), block size, name, flag}` at `0x4E11C0`.
pub const POOL_CLASS_TABLE: u32 = 0x4E_11C0;
/// Live object count per class (dword each).
pub const POOL_COUNTS: u32 = 0x73_BA60;
/// Head of the live object list per class (dword each).
pub const POOL_HEADS: u32 = 0x73_BB08;
/// Head of the free object list.
pub const POOL_FREE_HEAD: u32 = 0x73_BD44;
/// Dirty flag set to 1 whenever a pooled object is acquired or released.
pub const POOL_DIRTY: u32 = 0x4E_60E0;
/// Arena dirty flag set to 1 after a `0x3ED20` allocation.
pub const ARENA_DIRTY: u32 = 0x6B_83BC;
/// Arena free byte total.
pub const ARENA_FREE: u32 = 0x6B_83C0;
/// Arena largest request accepted.
pub const ARENA_MAX: u32 = 0x6B_83C4;
/// Arena roving cursor (guest address of a block header).
pub const ARENA_CURSOR: u32 = 0x6B_83CC;
/// Arena first block header.
pub const ARENA_START: u32 = 0x6B_83D0;
/// Arena end (first address past the last block).
pub const ARENA_END: u32 = 0x6B_83D4;
/// Arena block header size.
pub const ARENA_HEADER: u32 = 0x20;
/// 32 bit LCG state (`state * 0x343FD + 0x269EC3`), used by `0x153BB0` and `0x153BD0`.
pub const RNG_STATE: u32 = 0x74_97CC;
/// Sine table, 2048 floats (`sin(i * 2pi / 2048)`), filled by `0x153C20`.
pub const SINE_TABLE: u32 = 0x7A_38A0;
/// Pointer to the entity array (stride `0xB4`).
pub const ENTITY_BASE: u32 = 0x70_8788;
/// Entity tree descriptor.
pub const ENTITY_TREE: u32 = 0x70_2E00;
/// Entity stride, AABB min at `+0x10`, max at `+0x1C`, flag byte at `+0xB2` (bit `0x10` = visited during a query).
pub const ENTITY_STRIDE: u32 = 0xB4;
/// Table behind `0x80F20`: 24 byte entries, the first dword is the result.
pub const TEXTURE_TABLE: u32 = 0x4D_2D64;
/// Pooled object class of the debris effect.
pub const DEBRIS_CLASS: u32 = 0x15;
/// Arena callback address that is the pooled object release `0xFF200` (the only callback the closure installs).
pub const POOL_RELEASE_ADDRESS: u32 = 0xF_F200;
/// Patch count of one debris effect.
pub const PATCH_COUNT: u32 = 0x50;

// Constants read from `.rdata` (MEASURED values, bit patterns).
const HALF: f32 = 0.5;
const ONE: f32 = 1.0;
const THREE: f32 = 3.0;
const SIX: f32 = 6.0;
const TENTH: f32 = f32::from_bits(0x3DCC_CCCD);
const ANGLE_SCALE: f32 = f32::from_bits(0x43A2_F983);
const JITTER: f32 = f32::from_bits(0x3D4C_CCCD);
const C_0_2: f32 = f32::from_bits(0x3E4C_CCCD);
const C_0_3: f32 = f32::from_bits(0x3E99_999A);
const C_0_7: f32 = f32::from_bits(0x3F33_3333);
const C_0_85: f32 = f32::from_bits(0x3F59_999A);
const THIRD: f32 = f32::from_bits(0x3EAA_AAAB);
const HUNDREDTH: f32 = f32::from_bits(0x3C23_D70A);
const FOURTEEN: f32 = 14.0;
const EIGHTEEN: f32 = 18.0;
const QUARTER: f32 = 0.25;
const THREE_QUARTERS: f32 = 0.75;
const UNIT_EPSILON: f32 = f32::from_bits(0x33D6_BF95);
const NEG_ONE: f32 = -1.0;
const RNG_SCALE: f32 = f32::from_bits(0x3800_0100);
const QUANT: f32 = f32::from_bits(0x3880_0200);
const SMALL_FLAG_SCALE: f32 = f32::from_bits(0x3E4C_CCCD);
const LIFE_BIAS: f32 = 0.5;
const SCALE_A: f32 = f32::from_bits(0x402C_CCCD);
const SCALE_B: f32 = f32::from_bits(0x3FD9_999A);

/// Sine table entry `i` as `0x153C20` computes it: `sin(f32(f32(i * 0.00048828125) * 6.2831855))` rounded to `f32`.
pub fn sine_table_entry(index: u32) -> f32 {
    let step = f32::from_bits(0x3A00_0000);
    let tau = f32::from_bits(0x40C9_0FDB);
    let angle = (index as i32 as f32) * step * tau;
    f64::from(angle).sin() as f32
}

/// `0x80F20(i)`: first dword of the 24 byte table entry `i`.
pub fn texture_index(memory: &dyn EffectMemory, index: u32) -> u32 {
    memory.read_u32(index.wrapping_mul(24).wrapping_add(TEXTURE_TABLE))
}

/// `0x153550(matrix, x, y, z)`: identity matrix with the translation row set (`+0x30/+0x34/+0x38`).
pub fn write_translation_matrix(
    memory: &mut dyn EffectMemory,
    address: u32,
    x: f32,
    y: f32,
    z: f32,
) {
    for row in 0..4u32 {
        for column in 0..4u32 {
            let value = if row == column { ONE } else { 0.0 };
            memory.write_f32(address + (row * 4 + column) * 4, value);
        }
    }
    memory.write_f32(address + 0x30, x);
    memory.write_f32(address + 0x34, y);
    memory.write_f32(address + 0x38, z);
}

/// `0x3C848C`: float to int helper, truncation toward zero, returns the low dword.
fn ftol(value: f64) -> u32 {
    (value.trunc() as i64) as u32
}

/// `0x47560(a_min, a_max, b_min, b_max)`: 1 when the two boxes overlap (an axis separates them only when strictly,
/// `comiss` + `ja`, so a NaN never separates).
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn box_overlap(
    memory: &dyn EffectMemory,
    a_min: u32,
    a_max: u32,
    b_min: u32,
    b_max: u32,
) -> bool {
    (0..3u32).all(|axis| {
        let offset = axis * 4;
        !(memory.read_f32(a_min + offset) > memory.read_f32(b_max + offset))
            && !(memory.read_f32(b_min + offset) > memory.read_f32(a_max + offset))
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Engine boundary
// ---------------------------------------------------------------------------------------------------------------

/// Callbacks the arena installs for its blocks. The only callback the closure itself installs is `0xFF200`
/// (handled internally); a block owned by another subsystem carries its own function pointer and arrives here.
pub trait ArenaHooks {
    /// `function(user)` was called by `0x3EAF0` while evicting a used block (`user` is the pointer stored at block `+0xC`).
    fn external_callback(&mut self, function: u32, user: u32);
}

/// Hooks that ignore external callbacks.
pub struct NoHooks;

impl ArenaHooks for NoHooks {
    fn external_callback(&mut self, _function: u32, _user: u32) {}
}

/// Hooks that record `(function, user)` pairs in call order.
#[derive(Default, Debug)]
pub struct RecordedCallbacks(pub Vec<(u32, u32)>);

impl ArenaHooks for RecordedCallbacks {
    fn external_callback(&mut self, function: u32, user: u32) {
        self.0.push((function, user));
    }
}

/// The closure's view of the machine: guest memory plus the external callback sink.
pub struct Machine<'a> {
    pub memory: &'a mut dyn EffectMemory,
    pub hooks: &'a mut dyn ArenaHooks,
}

/// One corner of the contact face (the `0x1C` byte records the slot points at): position, three attributes, fill word.
#[derive(Clone, Copy)]
struct Corner {
    position: [f32; 3],
}

impl Machine<'_> {
    fn rd(&self, address: u32) -> u32 {
        self.memory.read_u32(address)
    }
    fn wr(&mut self, address: u32, value: u32) {
        self.memory.write_u32(address, value);
    }
    fn rf(&self, address: u32) -> f32 {
        self.memory.read_f32(address)
    }
    fn wf(&mut self, address: u32, value: f32) {
        self.memory.write_f32(address, value);
    }

    // -----------------------------------------------------------------------------------------------------------
    // Random numbers
    // -----------------------------------------------------------------------------------------------------------

    /// `0x153BB0`: `state = state * 0x343FD + 0x269EC3`, returns `(state >> 16) & 0x7FFF`.
    pub fn rng_int(&mut self) -> u32 {
        let state = self
            .rd(RNG_STATE)
            .wrapping_mul(0x343FD)
            .wrapping_add(0x26_9EC3);
        self.wr(RNG_STATE, state);
        (state >> 16) & 0x7FFF
    }

    /// `0x153BD0`: the same draw as a double, `(state >> 16 & 0x7FFF) * 3.0518509e-5` left in `st(0)` (not rounded to f32).
    pub fn rng_float(&mut self) -> f64 {
        f64::from(self.rng_int()) * f64::from(RNG_SCALE)
    }

    // -----------------------------------------------------------------------------------------------------------
    // Arena allocator (0x3EA40, 0x3EAD0, 0x3EAF0, 0x3EBD0, 0x3ECA0, 0x3ED20)
    // -----------------------------------------------------------------------------------------------------------

    /// `0x3EAD0(data)`: frees the block of a data pointer, 0 is ignored.
    pub fn arena_free(&mut self, data: u32) {
        if data != 0 {
            self.arena_free_block(data);
        }
    }

    /// `0x3EA40(data)`: marks a used block free and merges it with a free successor and predecessor.
    pub fn arena_free_block(&mut self, data: u32) {
        let mut block = data.wrapping_sub(ARENA_HEADER);
        if self.rd(block) != 1 {
            return;
        }
        let size = self.rd(block + 4);
        self.wr(block, 0);
        let free = self.rd(ARENA_FREE).wrapping_add(size);
        self.wr(ARENA_FREE, free);
        let size = self.rd(block + 4);
        let end = self.rd(ARENA_END);
        let next = size.wrapping_add(block);
        if next < end && self.rd(next) == 0 {
            let merged = self.rd(next + 4).wrapping_add(size);
            self.wr(block + 4, merged);
            if self.rd(ARENA_CURSOR) == next {
                self.wr(ARENA_CURSOR, block);
            }
        }
        let previous = block.wrapping_sub(self.rd(block + 8));
        if self.rd(previous) == 0 && previous != block {
            let size = self.rd(block + 4);
            let merged = self.rd(previous + 4).wrapping_add(size);
            self.wr(previous + 4, merged);
            if self.rd(ARENA_CURSOR) == block {
                self.wr(ARENA_CURSOR, previous);
            }
            block = previous;
        }
        let size = self.rd(block + 4);
        let end = self.rd(ARENA_END);
        let after = block.wrapping_add(size);
        if after < end {
            self.wr(after + 8, size);
        }
    }

    /// `0x3EAF0(need)`: advances the cursor over the ring of blocks until a free block of at least `need` bytes sits
    /// at the cursor, evicting used blocks (their callback, or clearing the owner pointer, then freeing) when the scan
    /// of at most 20 steps finds none.
    pub fn arena_make_room(&mut self, need: u32) {
        let need = need as i32;
        let mut cursor = self.rd(ARENA_CURSOR);
        let first = cursor;
        let mut count: i32 = 0;
        let mut saved_count: i32;
        loop {
            // 0x3EB14: step to the next block, wrapping at the end.
            let end = self.rd(ARENA_END);
            let start = self.rd(ARENA_START);
            'scan: loop {
                cursor = cursor.wrapping_add(self.rd(cursor + 4));
                self.wr(ARENA_CURSOR, cursor);
                if cursor >= end {
                    cursor = start;
                    self.wr(ARENA_CURSOR, cursor);
                }
                if self.rd(cursor) == 0 && cursor != first {
                    if need <= self.rd(cursor + 4) as i32 {
                        return;
                    }
                    count += 1;
                    continue 'scan;
                }
                count += 1;
                saved_count = count;
                if count >= 0x14 || (self.rd(ARENA_FREE) as i32) < need || cursor == first {
                    break 'scan;
                }
            }
            // 0x3EB52: restart at the first block, skip free blocks, evict the used block found.
            cursor = first;
            self.wr(ARENA_CURSOR, cursor);
            let mut current = first;
            if self.rd(first) == 0 {
                loop {
                    cursor = cursor.wrapping_add(self.rd(current + 4));
                    self.wr(ARENA_CURSOR, cursor);
                    if cursor >= end {
                        cursor = start;
                        self.wr(ARENA_CURSOR, cursor);
                    }
                    current = cursor;
                    if self.rd(current) != 0 {
                        break;
                    }
                }
            }
            let data = cursor.wrapping_add(ARENA_HEADER);
            if self.rd(data - ARENA_HEADER) != 0 {
                let callback = self.rd(data - 0x10);
                let user = self.rd(data - 0x14);
                if callback != 0 {
                    self.call_arena_callback(callback, user);
                } else if user != 0 {
                    self.wr(user, 0);
                }
                self.arena_free_block(data);
                cursor = self.rd(ARENA_CURSOR);
            }
            if need <= self.rd(cursor + 4) as i32 {
                return;
            }
            count = saved_count;
        }
    }

    fn call_arena_callback(&mut self, function: u32, user: u32) {
        if function == POOL_RELEASE_ADDRESS {
            self.pool_release(user);
        } else {
            self.hooks.external_callback(function, user);
        }
    }

    /// `0x3EBD0(size, user, callback)`: allocates a block of `size` bytes (rounded up to a multiple of `0x40` after
    /// adding `0x5F`), returns the data pointer (block + `0x20`) or 0.
    pub fn arena_alloc(&mut self, size: u32, user: u32, callback: u32) -> u32 {
        let mut block = self.rd(ARENA_CURSOR);
        let rounded = size.wrapping_add(0x5F) & 0xFFFF_FFC0;
        if rounded as i32 > self.rd(block + 4) as i32 {
            if rounded as i32 > self.rd(ARENA_MAX) as i32 {
                return 0;
            }
            self.arena_make_room(rounded);
            block = self.rd(ARENA_CURSOR);
        }
        let old_size = self.rd(block + 4);
        let used = if old_size.wrapping_sub(rounded) < 0x20 {
            old_size
        } else {
            rounded
        };
        self.wr(block + 0xC, user);
        self.wr(block, 1);
        self.wr(block + 0x10, callback);
        self.wr(block + 4, used);
        let end = self.rd(ARENA_END);
        let free = self.rd(ARENA_FREE).wrapping_sub(used);
        self.wr(ARENA_FREE, free);
        let next = self.rd(ARENA_CURSOR).wrapping_add(used);
        self.wr(ARENA_CURSOR, next);
        if next < end {
            if used == old_size {
                self.arena_make_room(0);
            } else {
                let rest = old_size - used;
                self.wr(next, 0);
                self.wr(next + 4, rest);
                self.wr(next + 8, used);
                self.wr(next + 0xC, 0);
                let after = old_size.wrapping_add(block);
                if after < end {
                    self.wr(after + 8, rest);
                }
            }
        } else {
            let start = self.rd(ARENA_START);
            self.wr(ARENA_CURSOR, start);
            if self.rd(start) != 0 {
                self.arena_make_room(0);
            }
        }
        block + ARENA_HEADER
    }

    /// `0x3ECA0(size)`: carves a reserved block (state 2) off the front of the arena at the cursor.
    pub fn arena_reserve(&mut self, size: u32) -> u32 {
        let cursor = self.rd(ARENA_CURSOR);
        let available = self.rd(cursor + 4);
        let rounded = size.wrapping_add(0x5F) & 0xFFFF_FFC0;
        if rounded.wrapping_add(0x20) > available {
            return 0;
        }
        self.wr(cursor + 4, rounded);
        self.wr(cursor, 2);
        self.wr(cursor + 0xC, 0);
        self.wr(cursor + 0x10, 0);
        self.wr(cursor + 8, 0);
        let start = self.rd(ARENA_START).wrapping_add(rounded);
        self.wr(ARENA_START, start);
        let free = self.rd(ARENA_FREE).wrapping_sub(rounded);
        self.wr(ARENA_FREE, free);
        let maximum = self.rd(ARENA_MAX).wrapping_sub(rounded);
        self.wr(ARENA_MAX, maximum);
        let moved = self.rd(ARENA_CURSOR).wrapping_add(rounded);
        let remaining = available.wrapping_sub(rounded);
        self.wr(ARENA_CURSOR, moved);
        self.wr(moved, 0);
        self.wr(moved + 8, 0);
        self.wr(moved + 0xC, 0);
        self.wr(moved + 0x10, 0);
        self.wr(moved + 4, remaining);
        cursor + ARENA_HEADER
    }

    /// `0x3ED20(size, mode, user, callback)`: mode 1 allocates (and sets `[0x6B83BC] = 1`), mode 2 reserves, others 0.
    pub fn arena_alloc_mode(&mut self, size: u32, mode: u32, user: u32, callback: u32) -> u32 {
        match mode {
            1 => {
                let data = self.arena_alloc(size, user, callback);
                self.wr(ARENA_DIRTY, 1);
                data
            }
            2 => self.arena_reserve(size),
            _ => 0,
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Pooled objects (0xFF200, 0xFF2B0)
    // -----------------------------------------------------------------------------------------------------------

    /// `0xFF200(object)`: releases a pooled object: clears the back pointer, frees its arena block unless the object
    /// has flag `0x40000000` at `+8`, unlinks it from its class list and pushes it on the free list (`[object] = -1`).
    pub fn pool_release(&mut self, object: u32) {
        if object == 0 || self.rd(object) == 0xFFFF_FFFF {
            return;
        }
        let back = self.rd(object + 0x14);
        if back != 0 {
            self.wr(back, 0);
        }
        let handle = self.rd(object + 0x18);
        self.wr(object + 0x14, 0);
        if handle != 0 {
            if self.rd(object + 8) & 0x4000_0000 == 0 {
                self.arena_free(handle);
            }
            self.wr(object + 0x18, 0);
        }
        let next = self.rd(object + 0x64);
        if next != 0 {
            let previous = self.rd(object + 0x60);
            self.wr(next + 0x60, previous);
        }
        let previous = self.rd(object + 0x60);
        if previous != 0 {
            let next = self.rd(object + 0x64);
            self.wr(previous + 0x64, next);
        }
        let previous = self.rd(object + 0x60);
        let class = self.rd(object);
        if previous == 0 {
            let next = self.rd(object + 0x64);
            self.wr(POOL_HEADS + class.wrapping_mul(4), next);
        }
        let count = self.rd(POOL_COUNTS + class.wrapping_mul(4)).wrapping_sub(1);
        self.wr(POOL_COUNTS + class.wrapping_mul(4), count);
        let free = self.rd(POOL_FREE_HEAD);
        self.wr(object + 0x60, 0);
        self.wr(object + 0x64, free);
        if free != 0 {
            self.wr(free + 0x60, object);
        }
        self.wr(POOL_FREE_HEAD, object);
        self.wr(object, 0xFFFF_FFFF);
        self.wr(POOL_DIRTY, 1);
    }

    /// Walks a class list from `head` to its last element (0 when the list is empty).
    fn list_tail(&self, head: u32) -> u32 {
        let mut node = head;
        if node == 0 {
            return 0;
        }
        loop {
            let next = self.rd(node + 0x64);
            if next == 0 {
                return node;
            }
            node = next;
        }
    }

    /// `0xFF2B0(class, with_block)`: takes an object from the free list for `class` (evicting the oldest object of the
    /// class when its limit is reached, or of the first non empty class when the free list is empty), initialises it
    /// and links it at the head of the class list. With `with_block` the class block size is allocated from the arena
    /// (callback `0xFF200`, user = object) and stored at `+0x18` (0 when the arena has no room).
    pub fn pool_acquire(&mut self, class: u32, with_block: bool) -> u32 {
        let entry = POOL_CLASS_TABLE + (class << 4);
        let limit = self.rd(entry);
        let count = self.rd(POOL_COUNTS + class * 4);
        let mut evicted = false;
        if limit != 0xFFFF_FFFF && (limit as i32) <= (count as i32) {
            let tail = self.list_tail(self.rd(POOL_HEADS + class * 4));
            self.pool_release(tail);
            evicted = true;
        }
        if !evicted && self.rd(POOL_FREE_HEAD) == 0 {
            let mut head_address = POOL_HEADS;
            loop {
                let head = self.rd(head_address);
                let tail = self.list_tail(head);
                head_address += 4;
                if tail != 0 {
                    self.pool_release(tail);
                    break;
                }
            }
        }
        let object = self.rd(POOL_FREE_HEAD);
        let next_free = self.rd(object + 0x64);
        self.wr(POOL_FREE_HEAD, next_free);
        if next_free != 0 {
            self.wr(next_free + 0x60, 0);
        }
        let head = self.rd(POOL_HEADS + class * 4);
        self.wr(object + 0x64, head);
        if head != 0 {
            self.wr(head + 0x60, object);
        }
        self.wr(object + 0x60, 0);
        let count = self.rd(POOL_COUNTS + class * 4).wrapping_add(1);
        self.wr(POOL_HEADS + class * 4, object);
        self.wr(POOL_COUNTS + class * 4, count);
        self.wr(object, class);
        self.wr(object + 0x14, 0);
        self.wr(object + 0x34, 0xFFFF);
        self.wf(object + 0x38, 0.0);
        self.wf(object + 0x30, 0.0);
        self.wf(object + 0x2C, 0.0);
        self.wr(object + 0xCC, 0);
        self.wr(object + 0x98, 0);
        self.wr(object + 0x18, 0);
        if with_block {
            let size = self.rd(entry + 4);
            let data = self.arena_alloc_mode(size, 1, object, POOL_RELEASE_ADDRESS);
            self.wr(object + 0x18, data);
        }
        self.wr(POOL_DIRTY, 1);
        object
    }

    // -----------------------------------------------------------------------------------------------------------
    // Entity tree query (0x47560, 0x87260, 0x1571C0, 0x157440, 0x87310, 0x873C0)
    // -----------------------------------------------------------------------------------------------------------

    /// `0x87260(min, max, list, count, items)` leaf callback: marks each listed entity visited (byte `+0xB2` bit `0x10`)
    /// and appends `id * 2 | overlap(box, entity box)` to the result list (`list[0]` is the count), skipping entities
    /// already visited.
    fn entity_leaf(&mut self, min: u32, max: u32, list: &mut Vec<u16>, count: u32, items: u32) {
        let mut stored = list[0];
        for index in 0..count {
            let id = u32::from(self.memory.read_u16(items + index * 2));
            let base = self.rd(ENTITY_BASE);
            let entity = base.wrapping_add(id.wrapping_mul(ENTITY_STRIDE));
            if self.memory.read_u8(entity + 0xB2) & 0x10 == 0 {
                let flags = self.memory.read_u8(entity + 0xB2);
                self.memory.write_u8(entity + 0xB2, flags | 0x10);
                let base = self.rd(ENTITY_BASE);
                let entity = base.wrapping_add(id.wrapping_mul(ENTITY_STRIDE));
                let overlap = box_overlap(self.memory, min, max, entity + 0x10, entity + 0x1C);
                list.resize(usize::from(stored) + 2, 0);
                list[usize::from(stored) + 1] = u16::from(overlap) | (id as u16).wrapping_mul(2);
                stored = stored.wrapping_add(1);
            }
        }
        list[0] = stored;
    }

    /// `0x1571C0`: recursive walk of the kd tree (nodes are u16: top two bits axis, low 14 bits split fraction, `0xC000 |
    /// n` is a leaf list index), visiting the leaves whose box overlaps the query box.
    #[allow(clippy::too_many_arguments)]
    fn tree_walk(
        &mut self,
        tree: u32,
        min: u32,
        max: u32,
        node: u32,
        lo: [f32; 3],
        hi: [f32; 3],
        list: &mut Vec<u16>,
    ) -> u32 {
        let nodes = self.rd(tree);
        let value = u32::from(self.memory.read_u16(nodes.wrapping_add(node * 2)));
        if value & 0xC000 == 0xC000 {
            let leaves = self.rd(tree + 4);
            let leaf = leaves.wrapping_add((value & 0x3FFF) * 2);
            let count = u32::from(self.memory.read_u16(leaf));
            self.entity_leaf(min, max, list, count, leaf + 2);
            return 0;
        }
        let axis = (value >> 14) as usize;
        let fraction = (value & 0x3FFF) as i32 as f32 * QUANT;
        let split = (hi[axis] - lo[axis]) * fraction + lo[axis];
        let mut result = 0;
        if split >= self.rf(min + axis as u32 * 4) {
            let mut upper = hi;
            upper[axis] = split;
            result = self.tree_walk(tree, min, max, node * 2, lo, upper, list);
            if result == 1 {
                return 1;
            }
        }
        if self.rf(max + axis as u32 * 4) >= split {
            let mut lower = lo;
            lower[axis] = split;
            result = self.tree_walk(tree, min, max, node * 2 + 1, lower, hi, list);
        }
        result
    }

    /// `0x157440(tree, min, max, callback, list)` with the callback `0x87260`: walks the tree when its box overlaps.
    fn tree_query(&mut self, tree: u32, min: u32, max: u32, list: &mut Vec<u16>) {
        if box_overlap(self.memory, min, max, tree + 8, tree + 0x14) {
            let lo = [self.rf(tree + 8), self.rf(tree + 0xC), self.rf(tree + 0x10)];
            let hi = [
                self.rf(tree + 0x14),
                self.rf(tree + 0x18),
                self.rf(tree + 0x1C),
            ];
            self.tree_walk(tree, min, max, 1, lo, hi, list);
        }
    }

    /// `0x87310(box, out, max)`: the entity ids whose box overlaps the query box (min at `box`, max at `box + 0xC`),
    /// at most `max` stored as dwords at `out`; clears every visited flag again. Returns the stored count.
    pub fn entity_query_ids(&mut self, bounds: u32, out: u32, max: u32) -> u32 {
        let mut list: Vec<u16> = vec![0];
        self.tree_query(ENTITY_TREE, bounds, bounds + 0xC, &mut list);
        let total = list[0];
        let mut stored = 0u32;
        let mut recording = true;
        for index in 0..usize::from(total) {
            let value = u32::from(list[1 + index]);
            let id = (value as i32 >> 1) as u32;
            let base = self.rd(ENTITY_BASE);
            let address = base.wrapping_add(id.wrapping_mul(ENTITY_STRIDE)) + 0xB2;
            let flags = self.memory.read_u8(address);
            self.memory.write_u8(address, flags & 0xEF);
            if recording && value & 1 != 0 {
                self.wr(out.wrapping_add(stored * 4), id);
                stored += 1;
                if stored == max {
                    recording = false;
                }
            }
        }
        stored
    }

    /// `0x873C0(0, box, out, max)`: [`Machine::entity_query_ids`] when the entity array exists, else 0.
    pub fn entity_query(&mut self, bounds: u32, out: u32, max: u32) -> u32 {
        if self.rd(ENTITY_BASE) == 0 {
            return 0;
        }
        self.entity_query_ids(bounds, out, max)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Debris effect builder
// ---------------------------------------------------------------------------------------------------------------

/// Offsets inside the slot record (the `0x6C` byte effect slot of `sweep_dynamic_shape.rs`, section 3 of its document,
/// extended by the fields `0x103A00` computes into `+0x44..+0x68`). The builder also writes the 12 dword id list at
/// `obj+0x9C`, not into the slot.
pub mod slot_field {
    pub const FLAGS: u32 = 0x08;
    pub const SAVED_C: u32 = 0x0C;
    pub const SAVED_10: u32 = 0x10;
    pub const CORNER_A: u32 = 0x14;
    pub const CORNER_B: u32 = 0x18;
    pub const CORNER_C: u32 = 0x1C;
    pub const CORNER_D: u32 = 0x20;
    pub const VELOCITY: u32 = 0x24;
    pub const WEIGHT: u32 = 0x30;
    pub const POOL_COUNT: u32 = 0x34;
    pub const POOL_ENTRY: u32 = 0x38;
    pub const GROUND: u32 = 0x40;
    pub const EDGE_B: u32 = 0x44;
    pub const EDGE_C: u32 = 0x50;
    pub const CONTACT_U: u32 = 0x5C;
    pub const CONTACT_V: u32 = 0x60;
    pub const DIRECTION_U: u32 = 0x64;
    pub const DIRECTION_V: u32 = 0x68;
}

/// Block layout of the `0x619C` byte working block (see the document): offsets of the arrays and header.
pub mod block {
    pub const MATRICES: u32 = 0x0000;
    pub const VERTICES: u32 = 0x1400;
    pub const ATTRIBUTES: u32 = 0x2800;
    pub const TEXTURE: u32 = 0x3C00;
    pub const FILL: u32 = 0x5000;
    pub const VELOCITY: u32 = 0x5500;
    pub const SPIN: u32 = 0x58C0;
    pub const OFFSET: u32 = 0x5C80;
    pub const LIFE: u32 = 0x6040;
    pub const HEADER_ZERO: u32 = 0x6180;
    pub const TEXTURE_ID: u32 = 0x6184;
    pub const SAVED_C: u32 = 0x6188;
    pub const MODE: u32 = 0x618C;
    pub const COUNT: u32 = 0x6190;
    pub const SINGLE: u32 = 0x6194;
    pub const FLAGS: u32 = 0x6198;
    pub const SIZE: u32 = 0x619C;
}

impl Machine<'_> {
    fn corner(&self, pointer: u32) -> Corner {
        Corner {
            position: [self.rf(pointer), self.rf(pointer + 4), self.rf(pointer + 8)],
        }
    }

    /// `0x1028E0(block, index, slot, object, pa, pb, pc, pd, outer)`: builds patch `index` of the block from the four
    /// uv points (`pa`..`pd`, each `(u, v)`; the original passes pointers to caller locals). `outer` selects the final
    /// ring patch variant (no velocity jitter, rare dead patch).
    #[allow(clippy::too_many_arguments)]
    pub fn build_patch(
        &mut self,
        blk: u32,
        index: u32,
        slot: u32,
        object: u32,
        pa: [f32; 2],
        pb: [f32; 2],
        pc: [f32; 2],
        pd: [f32; 2],
        outer: bool,
    ) {
        use slot_field as sf;
        let vertex_row = blk + block::VERTICES + index * 64;
        let a_ptr = self.rd(slot + sf::CORNER_A);
        let b_ptr = self.rd(slot + sf::CORNER_B);
        let c_ptr = self.rd(slot + sf::CORNER_C);
        let d_ptr = self.rd(slot + sf::CORNER_D);
        let (a, b, c, d) = (
            self.corner(a_ptr),
            self.corner(b_ptr),
            self.corner(c_ptr),
            self.corner(d_ptr),
        );
        let s44 = self.rf(slot + 0x44);
        let s48 = self.rf(slot + 0x48);
        let s4c = self.rf(slot + 0x4C);
        let s50 = self.rf(slot + 0x50);
        let s54 = self.rf(slot + 0x54);
        let s58 = self.rf(slot + 0x58);

        // Centroid of the first three points.
        let cu = ((pa[0] + pb[0]) + pc[0]) * THIRD;
        let cv = ((pa[1] + pc[1]) + pb[1]) * THIRD;
        let du = self.rf(slot + sf::DIRECTION_U) * HUNDREDTH;
        let dv = self.rf(slot + sf::DIRECTION_V) * HUNDREDTH;
        let shift = |p: [f32; 2]| [du + p[0], p[1] + dv];
        let (ua, ub, uc, ud) = (shift(pa), shift(pb), shift(pc), shift(pd));

        // Patch origin: x87 (double precision) evaluation, stored as f32.
        let (cu64, cv64) = (f64::from(cu), f64::from(cv));
        let x64 = |v: f32| f64::from(v);
        let wx = ((cv64 * x64(s50) + cu64 * x64(s44)) + x64(a.position[0]))
            - (((x64(s50) + x64(b.position[0])) - x64(d.position[0])) * cu64
                + ((x64(s44) + x64(c.position[0])) - x64(d.position[0])) * cv64);
        let wy = ((cv64 * x64(s54) + cu64 * x64(s48)) + x64(a.position[1]))
            - (((x64(c.position[1]) + x64(s48)) - x64(d.position[1])) * cv64
                + ((x64(b.position[1]) + x64(s54)) - x64(d.position[1])) * cu64);
        let wz = ((cv64 * x64(s58) + cu64 * x64(s4c)) + x64(a.position[2]))
            - (((x64(s58) + x64(b.position[2])) - x64(d.position[2])) * cu64
                + ((x64(c.position[2]) + x64(s4c)) - x64(d.position[2])) * cv64);
        let (wx, wy, wz) = (wx as f32, wy as f32, wz as f32);
        write_translation_matrix(self.memory, blk + index * 64, wx, wy, wz);

        // Four surface points (f32 arithmetic).
        let t3 = (s50 + b.position[0]) - d.position[0];
        let t0 = (s44 + c.position[0]) - d.position[0];
        let t1 = (b.position[1] + s54) - d.position[1];
        let t4 = (c.position[1] + s48) - d.position[1];
        let z_c = (c.position[2] + s4c) - d.position[2];
        let z_b = (s58 + b.position[2]) - d.position[2];
        let surface = |u: f32, v: f32| -> [f32; 3] {
            let x = ((s50 * v + s44 * u) + a.position[0]) - (t0 * v + t3 * u);
            let y = ((v * s54 + u * s48) + a.position[1]) - (t1 * u + t4 * v);
            let z = ((s58 * v + s4c * u) + a.position[2]) - (z_b * u + z_c * v);
            [x, y, z]
        };
        let corners = [
            surface(ua[0], ua[1]),
            surface(ub[0], ub[1]),
            surface(uc[0], uc[1]),
            surface(ud[0], ud[1]),
        ];
        let flags = [0x8000u32, 0x8000, 0, 0];
        for (row, point) in corners.iter().enumerate() {
            let base = vertex_row + row as u32 * 16;
            self.wf(base, point[0] - wx);
            self.wf(base + 4, point[1] - wy);
            self.wf(base + 8, point[2] - wz);
            self.wr(base + 12, flags[row]);
        }

        // Attribute row.
        let attribute_row = blk + block::ATTRIBUTES + index * 64;
        let points = [pa, pb, pc, pd];
        if self.rd(blk + block::MODE) != 0 {
            for (row, p) in points.iter().enumerate() {
                let (u, v) = (p[0], p[1]);
                for component in 0..3u32 {
                    let offset = 0xC + component * 4;
                    let (va, vb, vc, vd) = (
                        self.rf(a_ptr + offset),
                        self.rf(b_ptr + offset),
                        self.rf(c_ptr + offset),
                        self.rf(d_ptr + offset),
                    );
                    let lower = (ONE - u) * va + u * vb;
                    let upper = (ONE - u) * vc + u * vd;
                    let value = lower * (ONE - v) + upper * v;
                    self.wf(attribute_row + row as u32 * 16 + component * 4, value);
                }
                self.wf(attribute_row + row as u32 * 16 + 12, ONE);
            }
        } else {
            let a0 = self.rf(a_ptr + 0xC);
            let a1 = self.rf(a_ptr + 0x10);
            let db0 = self.rf(b_ptr + 0xC) - a0;
            let db1 = self.rf(b_ptr + 0x10) - a1;
            let dc0 = self.rf(c_ptr + 0xC) - a0;
            let dc1 = self.rf(c_ptr + 0x10) - a1;
            for (row, p) in points.iter().enumerate() {
                let base = attribute_row + row as u32 * 16;
                self.wf(base, (p[1] * dc0 + p[0] * db0) + a0);
                self.wf(base + 4, (p[1] * dc1 + p[0] * db1) + a1);
                self.wf(base + 8, ONE);
                self.wf(base + 12, ONE);
            }
        }

        // Texture row.
        if self.rd(blk + block::TEXTURE_ID) != 0xFFFF_FFFF {
            let entry = self.rd(slot + sf::POOL_ENTRY);
            let scale_u = self.rf(entry + 0x24);
            let offset_u = self.rf(entry + 0x1C);
            let scale_v = self.rf(entry + 0x28);
            let offset_v = self.rf(entry + 0x20);
            let texture_row = blk + block::TEXTURE + index * 64;
            for (row, p) in points.iter().enumerate() {
                let base = texture_row + row as u32 * 16;
                self.wf(base, p[0] * scale_u + offset_u);
                self.wf(base + 4, p[1] * scale_v + offset_v);
                self.wf(base + 8, ONE);
                self.wf(base + 12, ONE);
            }
        }

        // Velocity, offsets and life of the patch.
        let object_position = [
            self.rf(object + 0x20),
            self.rf(object + 0x24),
            self.rf(object + 0x28),
        ];
        let mut rel = [
            wx - object_position[0],
            wy - object_position[1],
            wz - object_position[2],
        ];
        let u0 = self.rf(slot + sf::CONTACT_U);
        let v0 = self.rf(slot + sf::CONTACT_V);
        let du0 = (f64::from(cu) - f64::from(u0)) as f32;
        let dv_exact = f64::from(cv) - f64::from(v0);
        let dv0 = dv_exact as f32;
        let distance = (dv_exact * f64::from(dv0) + f64::from(du0) * f64::from(du0)).sqrt();
        let normalised = (distance / 2.0f64.sqrt()) as f32;
        let falloff = ONE - normalised;
        let cubic = (falloff * falloff) * falloff;
        let scaled = cubic * FOURTEEN;
        let velocity = blk + block::VELOCITY + index * 12;
        let spin = blk + block::SPIN + index * 12;
        let offset = blk + block::OFFSET + index * 12;
        let life = blk + block::LIFE + index * 4;
        let weight = self.rf(slot + sf::WEIGHT);
        let vel_slot = [
            self.rf(slot + 0x24),
            self.rf(slot + 0x28),
            self.rf(slot + 0x2C),
        ];
        self.wf(velocity, vel_slot[0] * scaled);
        self.wf(velocity + 4, vel_slot[1] * scaled);
        self.wf(velocity + 8, scaled * vel_slot[2]);
        if !outer {
            let length_squared = (rel[2] * rel[2] + rel[1] * rel[1]) + rel[0] * rel[0];
            if length_squared > UNIT_EPSILON {
                let inverse = ONE / length_squared.sqrt();
                let factor = inverse * ONE;
                rel = [factor * rel[0], factor * rel[1], factor * rel[2]];
            }
            let weight64 = f64::from(weight);
            let jitter_x = (((self.rng_float() * weight64) * 2.0) * f64::from(rel[0])) as f32;
            let jitter_y = (((self.rng_float() * weight64) * 2.0) * f64::from(rel[1])) as f32;
            let jitter_z = ((self.rng_float() * weight64) * 2.0) * f64::from(rel[2]);
            let new_x = (f64::from(jitter_x) + f64::from(self.rf(velocity))) as f32;
            self.wf(velocity, new_x);
            let new_y = (f64::from(jitter_y) + f64::from(self.rf(velocity + 4))) as f32;
            self.wf(velocity + 4, new_y);
            let new_z = (jitter_z + f64::from(self.rf(velocity + 8))) as f32;
            self.wf(velocity + 8, new_z);
        }
        let limit = weight * THREE;
        let capped = if ONE > limit { limit } else { ONE };
        let scale = (ONE - capped) * normalised;
        self.wf(life, 0.0 - scale);
        self.wf(spin, 0.0);
        self.wf(spin + 4, 0.0);
        self.wf(spin + 8, 0.0);
        let fold = |value: f32| {
            if 0.0 > value {
                NEG_ONE - value
            } else {
                ONE - value
            }
        };
        let first = fold(du0);
        let second = fold(dv0);
        let cubic64 = f64::from(cubic);
        let u_term = self.rng_float();
        let chain_first = ((((u_term * f64::from(QUARTER)) + f64::from(THREE_QUARTERS))
            * f64::from(first))
            * cubic64)
            * f64::from(EIGHTEEN);
        let chain_first32 = chain_first as f32;
        let v_term = self.rng_float();
        let chain_second = ((((v_term * f64::from(QUARTER)) + f64::from(THREE_QUARTERS))
            * f64::from(second))
            * cubic64)
            * f64::from(EIGHTEEN);
        let s24 = self.rf(slot + 0x24);
        let s28 = self.rf(slot + 0x28);
        let s2c = self.rf(slot + 0x2C);
        self.wf(
            offset,
            ((f64::from(s28) + f64::from(s2c)) * chain_second) as f32,
        );
        self.wf(
            offset + 4,
            ((f64::from(s24) + f64::from(s2c)) * f64::from(chain_first32)) as f32,
        );
        self.wf(
            offset + 8,
            (chain_second * f64::from(s24) + f64::from(chain_first32) * f64::from(s28)) as f32,
        );
        if outer && self.rd(slot + sf::FLAGS) & 0x200 == 0 && self.rng_int() & 7 == 0 {
            let value = self.rng_float();
            self.wf(
                life,
                (-(value * f64::from(SMALL_FLAG_SCALE) + f64::from(LIFE_BIAS))) as f32,
            );
            for address in [
                velocity,
                velocity + 4,
                velocity + 8,
                spin,
                spin + 4,
                spin + 8,
                offset,
                offset + 4,
                offset + 8,
            ] {
                self.wf(address, 0.0);
            }
        }
    }

    /// One randomised edge point of the perimeter: `(u - 0.5) * 0.1 + around` (x87, f64), replaced by `around` when it
    /// is not in `(0, 1)`.
    fn jitter_sample(&mut self, around: f32) -> f32 {
        let uniform = self.rng_float();
        let value = (uniform - f64::from(HALF)) * f64::from(TENTH) + f64::from(around);
        let rounded = value as f32;
        if 0.0 >= value || rounded >= ONE {
            around
        } else {
            rounded
        }
    }

    /// `0x103A00(owner, slot)`: builds the debris effect of a contact slot. Returns the pooled object (0 when the
    /// arena had no room for the block, in which case the object was released again).
    pub fn spawn_debris(&mut self, owner: u32, slot: u32) -> u32 {
        use slot_field as sf;
        let object = self.pool_acquire(DEBRIS_CLASS, true);
        let corner_a = self.rd(slot + sf::CORNER_A);
        let corner_b = self.rd(slot + sf::CORNER_B);
        let corner_c = self.rd(slot + sf::CORNER_C);
        let corner_d = self.rd(slot + sf::CORNER_D);
        let blk = self.rd(object + 0x18);
        self.wr(object + 0x14, owner);
        self.wr(object + 8, 0x8_0000);
        self.wf(object + 0xC, 0.0);
        self.wf(object + 0x38, 0.0);
        self.wr(object + 4, 0xFFFF_FFFF);
        self.wr(object + 0x1C, 0);
        let ground = self.rd(slot + sf::GROUND);
        self.wr(object + 0x2C, ground);
        let flags = self.rd(slot + sf::FLAGS);
        let size = if flags & 1 != 0 {
            self.wr(object + 8, 0xA_0000);
            SCALE_A
        } else {
            SCALE_B
        };
        self.wf(object + 0x10, size);
        if blk == 0 {
            self.pool_release(object);
            return 0;
        }
        let saved_c = self.rd(slot + sf::SAVED_C);
        let saved_10 = self.rd(slot + sf::SAVED_10);
        self.wr(blk + block::FLAGS, self.rd(slot + sf::FLAGS));
        self.wr(blk + block::SAVED_C, saved_c);
        self.wr(blk + block::MODE, saved_10);
        self.wr(blk + block::TEXTURE_ID, 0xFFFF_FFFF);
        self.wf(blk + block::HEADER_ZERO, 0.0);
        if self.rd(slot + sf::POOL_COUNT) != 0 {
            let entry = self.rd(slot + sf::POOL_ENTRY);
            let inner = self.rd(entry + 0x18);
            let index = self.rd(inner);
            let id = texture_index(self.memory, index);
            self.wr(blk + block::TEXTURE_ID, id);
        }
        let single = u32::from(self.rd(blk + block::MODE).wrapping_sub(1) == 0);
        self.wr(blk + block::SINGLE, single);

        // Edge vectors and the point of the contact in the face.
        for axis in 0..3u32 {
            let offset = axis * 4;
            let origin = self.rf(corner_a + offset);
            let b_edge = self.rf(corner_b + offset) - origin;
            self.wf(slot + sf::EDGE_B + offset, b_edge);
        }
        for axis in 0..3u32 {
            let offset = axis * 4;
            let origin = self.rf(corner_a + offset);
            let c_edge = self.rf(corner_c + offset) - origin;
            self.wf(slot + sf::EDGE_C + offset, c_edge);
        }
        let u0 = self.rf(slot + sf::CONTACT_U);
        let v0 = self.rf(slot + sf::CONTACT_V);
        let origin = [
            self.rf(corner_a),
            self.rf(corner_a + 4),
            self.rf(corner_a + 8),
        ];
        let e_b = [
            self.rf(slot + 0x44),
            self.rf(slot + 0x48),
            self.rf(slot + 0x4C),
        ];
        let e_c = [
            self.rf(slot + 0x50),
            self.rf(slot + 0x54),
            self.rf(slot + 0x58),
        ];
        self.wf(object + 0x20, (v0 * e_c[0] + e_b[0] * u0) + origin[0]);
        self.wf(object + 0x24, (v0 * e_c[1] + e_b[1] * u0) + origin[1]);
        self.wf(object + 0x28, (v0 * e_c[2] + u0 * e_b[2]) + origin[2]);
        self.wr(blk + block::COUNT, PATCH_COUNT);

        // Perimeter of the unit square with a jittered point on each edge, then the edge midpoints.
        let mut perimeter = [[0.0f32; 2]; 16];
        perimeter[0] = [self.jitter_sample(u0), 0.0];
        perimeter[2] = [ONE, 0.0];
        perimeter[4] = [ONE, self.jitter_sample(v0)];
        perimeter[6] = [ONE, ONE];
        perimeter[8] = [self.jitter_sample(u0), ONE];
        perimeter[10] = [0.0, ONE];
        perimeter[12] = [0.0, self.jitter_sample(v0)];
        perimeter[14] = [0.0, 0.0];
        for step in 0..8usize {
            let previous = perimeter[2 * step];
            let next = perimeter[(2 * step + 2) % 16];
            perimeter[2 * step + 1] = [
                (next[0] - previous[0]) * HALF + previous[0],
                (next[1] - previous[1]) * HALF + previous[1],
            ];
        }
        let mut relative = [[0.0f32; 2]; 16];
        for (index, point) in perimeter.iter().enumerate() {
            relative[index] = [point[0] - u0, point[1] - v0];
        }

        // The fan: 16 triangles, 5 patches each.
        for fan in 0..16usize {
            let r0 = relative[fan];
            let r1 = relative[(fan + 1) % 16];
            let angle0 = f64::from(r0[1]).atan2(f64::from(r0[0]));
            let angle1 = f64::from(r1[1]).atan2(f64::from(r1[0])) as f32;
            let index0 = ftol(angle0 * f64::from(ANGLE_SCALE) + f64::from(HALF));
            let cos0 = self.rf(SINE_TABLE + (index0.wrapping_add(0x200) & 0x7FF) * 4);
            let sin0 = self.rf(SINE_TABLE + (index0 & 0x7FF) * 4);
            let index1 = ftol(f64::from(angle1) * f64::from(ANGLE_SCALE) + f64::from(HALF));
            let cos1 = self.rf(SINE_TABLE + (index1.wrapping_add(0x200) & 0x7FF) * 4);
            let sin1 = self.rf(SINE_TABLE + (index1 & 0x7FF) * 4);
            let factor_a = (self.rng_float() * f64::from(JITTER) + f64::from(C_0_2)) as f32;
            let factor_b = (self.rng_float() * f64::from(JITTER) + f64::from(C_0_3)) as f32;
            let factor_mid = (self.rng_float() * f64::from(JITTER) + f64::from(HALF)) as f32;
            let factor_c = (self.rng_float() * f64::from(JITTER) + f64::from(factor_mid)) as f32;
            let factor_d = (self.rng_float() * f64::from(JITTER) + f64::from(C_0_7)) as f32;
            let factor_e = (self.rng_float() * f64::from(JITTER) + f64::from(C_0_85)) as f32;
            let dx = r1[0] - r0[0];
            let dy = r1[1] - r0[1];
            let mid_x = dx * HALF + r0[0];
            let mid_y = dy * HALF + r0[1];
            let length = (f64::from(mid_y) * f64::from(mid_y) + f64::from(mid_x) * f64::from(mid_x))
                .sqrt() as f32;
            let inverse = ONE / length;
            self.wf(slot + sf::DIRECTION_U, mid_x * inverse);
            self.wf(slot + sf::DIRECTION_V, mid_y * inverse);
            let negate = |value: f32| if 0.0 > value { 0.0 - value } else { value };
            let (cos0, sin0, cos1, sin1) = (negate(cos0), negate(sin0), negate(cos1), negate(sin1));
            let base = fan as u32 * 5;
            let at = |x: f32, y: f32| [x, y];
            let first = at(u0, v0);
            let second_a = at(
                (r0[0] * factor_a) * cos0 + u0,
                (factor_a * sin0) * r0[1] + v0,
            );
            let third_a = at(
                (r1[0] * factor_b) * cos1 + u0,
                (r1[1] * factor_b) * sin1 + v0,
            );
            self.build_patch(
                blk, base, slot, object, first, second_a, third_a, third_a, false,
            );
            let q1 = at(
                (r0[0] * factor_mid) * cos0 + u0,
                (factor_mid * sin0) * r0[1] + v0,
            );
            let q2 = at(
                (r1[0] * factor_c) * cos1 + u0,
                (r1[1] * factor_c) * sin1 + v0,
            );
            self.build_patch(
                blk,
                base + 1,
                slot,
                object,
                second_a,
                q1,
                third_a,
                q2,
                false,
            );
            let second_b = at(
                (r0[0] * factor_d) * cos0 + u0,
                (factor_d * sin0) * r0[1] + v0,
            );
            let third_b = at(
                (r1[0] * factor_d) * cos1 + u0,
                (r1[1] * factor_d) * sin1 + v0,
            );
            self.build_patch(
                blk,
                base + 2,
                slot,
                object,
                second_b,
                q1,
                third_b,
                q2,
                false,
            );
            let q1b = at(
                (r0[0] * factor_e) * cos0 + u0,
                (factor_e * sin0) * r0[1] + v0,
            );
            let q2b = at(
                (r1[0] * factor_e) * cos1 + u0,
                (r1[1] * factor_e) * sin1 + v0,
            );
            self.build_patch(
                blk,
                base + 3,
                slot,
                object,
                second_b,
                q1b,
                third_b,
                q2b,
                false,
            );
            let second_c = at(r0[0] + u0, v0 + r0[1]);
            let third_c = at(r1[0] + u0, v0 + r1[1]);
            self.build_patch(
                blk,
                base + 4,
                slot,
                object,
                second_c,
                q1b,
                third_c,
                q2b,
                true,
            );
        }

        // The fill word and the entity query box around the four corners.
        let fill = self.rd(self.rd(slot + sf::CORNER_A) + 0x18);
        for offset in (0..0x500u32).step_by(4) {
            self.wr(blk + block::FILL + offset, fill);
        }
        let points: [[f32; 3]; 4] = [
            self.corner(corner_a).position,
            self.corner(corner_b).position,
            self.corner(corner_c).position,
            self.corner(corner_d).position,
        ];
        let low = |axis: usize| {
            let mut value = points[3][axis];
            for other in [points[2][axis], points[1][axis], points[0][axis]] {
                value = if value > other { other } else { value };
            }
            value
        };
        let high = |axis: usize| {
            let mut value = if points[2][axis] > points[3][axis] {
                points[2][axis]
            } else {
                points[3][axis]
            };
            for other in [points[1][axis], points[0][axis]] {
                value = if other > value { other } else { value };
            }
            value
        };
        let bounds = object + 0x80;
        self.wf(bounds, low(0) - THREE);
        self.wf(bounds + 0xC, high(0) + THREE);
        self.wf(bounds + 4, low(1) - SIX);
        self.wf(bounds + 0x10, high(1) + THREE);
        self.wf(bounds + 8, low(2) - THREE);
        self.wf(bounds + 0x14, high(2) + THREE);
        let count = self.entity_query(bounds, object + 0x9C, 12);
        self.wr(object + 0x98, count);
        object
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Wiring
// ---------------------------------------------------------------------------------------------------------------

/// `0x103A00(0, slot)` as the sweep code calls it.
pub fn build_debris(memory: &mut dyn EffectMemory, hooks: &mut dyn ArenaHooks, slot: u32) {
    Machine { memory, hooks }.spawn_debris(0, slot);
}

/// An [`ImpactServices`] that forwards `disturb`, `spawn_request` and `ground_height` to `inner` and implements
/// `build_debris` implemented with [`build_debris`] over the shared guest memory. Because `sweep_dynamic_shape` passes the memory
/// as a separate `&mut` argument, the memory has to live in a [`RefCell`] shared between that argument (a proxy that
/// borrows the cell per access) and this adapter; this type only needs the cell.
pub struct TeardownImpactServices<'a, M: EffectMemory, S: ImpactServices, H: ArenaHooks> {
    pub memory: &'a RefCell<M>,
    pub inner: S,
    pub hooks: H,
}

impl<M: EffectMemory, S: ImpactServices, H: ArenaHooks> ImpactServices
    for TeardownImpactServices<'_, M, S, H>
{
    fn disturb(&mut self, position: [f32; 3], radius: f32, flags: u32) {
        self.inner.disturb(position, radius, flags);
    }
    fn spawn_request(&mut self, record: [u32; 12]) {
        self.inner.spawn_request(record);
    }
    fn build_debris(&mut self, slot: u32) {
        let mut memory = self.memory.borrow_mut();
        build_debris(&mut *memory, &mut self.hooks, slot);
    }
    fn ground_height(&mut self, point: [f32; 3], y_bias: f32, half_extent: f32) -> f32 {
        self.inner.ground_height(point, y_bias, half_extent)
    }
}
