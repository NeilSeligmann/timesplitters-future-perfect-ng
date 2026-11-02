//! T1863: the particle emitter pool behind `0x111AE0` (emitter creation) and the setters `0x10E660`, `0x10EAE0`, `0x10EB30`,
//! `0x10EB70`, `0x10E9D0`, `0x10E8C0`.
//!
//! Source: `build/default.xbe` (SHA-256 `3cfd001a84fc3e08175d6c4b2e42ebf49577a089b5d0bd87ac724f61a41816bc`). Decoded with
//! `tools/x87_blocks.py`, validated bit for bit against the original under the Unicorn oracle
//! (`tools/t1846_gold/svc_emitter.py`, test `tests/story_events_t1863_emitter.rs`). Document:
//! `docs/t-story-event-particle-kinds-t1863.md`. Names INFERRED, behaviour MEASURED (xemu level, not silicon).
//!
//! | address | port |
//! |---|---|
//! | `0x111AE0(handle_out, template, flags, &position, &vector)` | [`create_emitter`] |
//! | `0x10DF90()` | [`emitter_alloc`] (pop the free list `[0x73BD50]`, zero `0xF8` bytes, append to the used list) |
//! | `0x10C220(emitter, template)` | [`emitter_init`] |
//! | `0x10E410` (register `edi`) | [`emitter_visible`] |
//! | `0x10E510(emitter)` | [`emitter_finish`] |
//! | `0x10E070` (register `esi`) | [`emitter_free`] |
//! | `0x10C0E0(emitter)` | [`emitter_release_buffer`] |
//! | `0x10E660` and the other setters | [`apply_param`] |
//! | `0x873F0(index, out_min, out_max)` | [`region_bounds`] |
//!
//! An emitter is a `0xF8` byte record; the pool is two doubly linked lists (free head `0x73BD50`, used head `0x73BD54`, used tail
//! `0x73BD58`, used count `0x73BD5C`; links at `+0xF0` previous and `+0xF4` next) and `[0x7AD28C]` is the emitter created last,
//! which the setters act on.

use super::strip_update::GuestMemory;
use super::svc_effect_submit::SvcMemory;
use super::svc_particle_kinds::EmitterParam;
use super::svc_particles::refresh_regions;

/// `[0x7AD28C]`: the emitter created last (null when none).
pub const CURRENT_EMITTER: u32 = 0x007A_D28C;
/// Free list head, used list head and tail, used count.
pub const FREE_HEAD: u32 = 0x0073_BD50;
pub const USED_HEAD: u32 = 0x0073_BD54;
pub const USED_TAIL: u32 = 0x0073_BD58;
pub const USED_COUNT: u32 = 0x0073_BD5C;
/// `[0x708788]`: region table (stride `0xB4`, min corner at `+0x10`, max corner at `+0x1C`).
pub const REGION_TABLE: u32 = 0x0070_8788;
/// `[0x7B0CE0]` viewport count and `[0x7B0CE4]` viewport array (stride `0xFFC`) read by [`emitter_visible`].
pub const VIEWPORT_COUNT: u32 = 0x007B_0CE0;
pub const VIEWPORT_BASE: u32 = 0x007B_0CE4;
/// `[0x79094C]`: level id (`0x65` makes every emitter visible).
pub const LEVEL_ID: u32 = 0x0079_094C;
/// `[0x7497CC]`: state of the small LCG `0x153BD0`.
pub const SMALL_RNG: u32 = 0x0074_97CC;
/// Size of an emitter record in bytes.
pub const EMITTER_BYTES: u32 = 0xF8;

/// A memory that both the emitter code (`SvcMemory`) and the region refresh (`GuestMemory`) accept.
pub trait EmitterMemory: SvcMemory + GuestMemory {}

impl<T: SvcMemory + GuestMemory + ?Sized> EmitterMemory for T {}

/// Leaves of the emitter creation.
pub trait EmitterServices {
    /// `0xC3E60(&point, 0.01, 0.5, &found, 0, 0)`: the floor height under `point` and the found flag it writes.
    fn ground(&mut self, point: [f32; 3]) -> (f32, u32);
    /// `0x3EAD0(block)`: free an arena block (the emitter's `+0x40` buffer).
    fn free_block(&mut self, block: u32);
}

fn rf(mem: &dyn EmitterMemory, address: u32) -> f32 {
    f32::from_bits(SvcMemory::r32(mem, address))
}

fn r32(mem: &dyn EmitterMemory, address: u32) -> u32 {
    SvcMemory::r32(mem, address)
}

fn w32(mem: &mut dyn EmitterMemory, address: u32, value: u32) {
    SvcMemory::w32(mem, address, value);
}

fn wf(mem: &mut dyn EmitterMemory, address: u32, value: f32) {
    SvcMemory::w32(mem, address, value.to_bits());
}

/// Lay out an empty emitter pool of `count` records at `base` (free list in address order, used list empty). The original builds
/// the same two lists at boot; the record stride here is [`EMITTER_BYTES`].
pub fn emitter_pool_init(mem: &mut dyn EmitterMemory, base: u32, count: u32) {
    for index in 0..count {
        let record = base + index * EMITTER_BYTES;
        let previous = if index == 0 {
            0
        } else {
            record - EMITTER_BYTES
        };
        let next = if index + 1 == count {
            0
        } else {
            record + EMITTER_BYTES
        };
        w32(mem, record + 0xF0, previous);
        w32(mem, record + 0xF4, next);
    }
    w32(mem, FREE_HEAD, if count == 0 { 0 } else { base });
    w32(mem, USED_HEAD, 0);
    w32(mem, USED_TAIL, 0);
    w32(mem, USED_COUNT, 0);
}

/// `0x10DF90()`: take the free list head, zero its `0xF8` bytes, make it the used list tail. Returns 0 when the pool is empty.
pub fn emitter_alloc(mem: &mut dyn EmitterMemory) -> u32 {
    let record = r32(mem, FREE_HEAD);
    if record == 0 {
        return 0;
    }
    let next = r32(mem, record + 0xF4);
    w32(mem, FREE_HEAD, next);
    if next != 0 {
        w32(mem, next + 0xF0, 0);
    }
    for k in 0..(EMITTER_BYTES / 4) {
        w32(mem, record + 4 * k, 0);
    }
    let tail = r32(mem, USED_TAIL);
    w32(mem, record + 0xF0, tail);
    if tail != 0 {
        w32(mem, tail + 0xF4, record);
    }
    let head = r32(mem, USED_HEAD);
    w32(mem, USED_TAIL, record);
    if head == 0 {
        w32(mem, USED_HEAD, record);
    }
    let count = r32(mem, USED_COUNT).wrapping_add(1);
    w32(mem, USED_COUNT, count);
    record
}

/// `0x10C220(emitter, template)`: copy the template flags, add its offsets to the position (`+0x14`) and the vector (`+0x2C`),
/// mirror the position at `+0x20`, and reset the animation state.
pub fn emitter_init(mem: &mut dyn EmitterMemory, emitter: u32, template: u32) {
    w32(mem, emitter + 0xC, template);
    w32(mem, emitter + 0x40, 0);
    let flags = r32(mem, template + 4);
    w32(mem, emitter + 4, flags);
    for (from, to) in [
        (0x8, 0x14),
        (0xC, 0x18),
        (0x10, 0x1C),
        (0x14, 0x2C),
        (0x18, 0x30),
    ] {
        let value = rf(mem, template + from) + rf(mem, emitter + to);
        wf(mem, emitter + to, value);
    }
    let last = rf(mem, template + 0x1C);
    for k in 0..3 {
        let value = r32(mem, emitter + 0x14 + 4 * k);
        w32(mem, emitter + 0x20 + 4 * k, value);
    }
    let value = last + rf(mem, emitter + 0x34);
    wf(mem, emitter + 0x34, value);
    SvcMemory::w8(mem, emitter + 0x12, 1);
    SvcMemory::w8(mem, emitter + 0x10, 1);
    SvcMemory::w8(mem, emitter + 0x11, 0);
    w32(mem, emitter + 0x50, 2);
    for offset in [0x68, 0x6C, 0x70, 0x74] {
        wf(mem, emitter + offset, 1.0);
    }
}

/// `0x10C0E0(emitter)`: free the `+0x40` buffer (and force the `+0x50` state to 1 for flag 1 emitters), clear the four buffer words.
pub fn emitter_release_buffer(
    mem: &mut dyn EmitterMemory,
    services: &mut dyn EmitterServices,
    emitter: u32,
) {
    if emitter == 0 {
        return;
    }
    let block = r32(mem, emitter + 0x40);
    if block != 0 {
        services.free_block(block);
        if SvcMemory::r8(mem, emitter + 4) & 1 != 0 {
            w32(mem, emitter + 0x50, 1);
        }
    }
    for offset in [0x40, 0x44, 0x48, 0x4C] {
        w32(mem, emitter + offset, 0);
    }
}

/// `0x10E410` (register `edi` = emitter): is the emitter seen by any viewport? Sets `+0x7C` to the mask of the viewports whose region
/// byte table contains one of the emitter's regions (`+0xE4..`), or to `+0x78` for emitters without regions (or in level `0x65`).
pub fn emitter_visible(mem: &mut dyn EmitterMemory, emitter: u32) -> bool {
    let regions = r32(mem, emitter + 0xE0);
    if (regions == 0 && r32(mem, emitter + 0x88) == 0) || r32(mem, LEVEL_ID) == 0x65 {
        let mask = r32(mem, emitter + 0x78);
        w32(mem, emitter + 0x7C, mask);
        return true;
    }
    w32(mem, emitter + 0x7C, 0);
    if (regions as i32) > 0 {
        let mut viewports = r32(mem, VIEWPORT_COUNT) as i32;
        let mut base = r32(mem, VIEWPORT_BASE);
        let mut k = 0u32;
        loop {
            let mask = r32(mem, emitter + 0x78);
            let mut offset = 0u32;
            for j in 0..viewports.max(0) {
                let bit = 1u32.wrapping_shl(j as u32);
                if mask & bit != 0 {
                    let word = r32(mem, base.wrapping_add(offset).wrapping_add(0x21C));
                    if (word as i32) < 0 && word & 1 != 0 && j < viewports {
                        let region = u32::from(SvcMemory::r8(mem, emitter + 0xE4 + k));
                        let seen = SvcMemory::r8(
                            mem,
                            base.wrapping_add(region.wrapping_add(offset))
                                .wrapping_add(0xD2C),
                        );
                        if seen != 0 {
                            let merged = r32(mem, emitter + 0x7C) | bit;
                            w32(mem, emitter + 0x7C, merged);
                            viewports = r32(mem, VIEWPORT_COUNT) as i32;
                            base = r32(mem, VIEWPORT_BASE);
                        }
                    }
                }
                offset = offset.wrapping_add(0xFFC);
            }
            k += 1;
            if k as i32 >= r32(mem, emitter + 0xE0) as i32 {
                break;
            }
        }
    }
    r32(mem, emitter + 0x7C) != 0
}

/// `0x10E070` (register `esi` = emitter): free the buffer, clear the owner's handle, unlink from the used list, push on the free
/// list and clear `[0x7AD28C]` when it is this emitter.
pub fn emitter_free(mem: &mut dyn EmitterMemory, services: &mut dyn EmitterServices, emitter: u32) {
    emitter_release_buffer(mem, services, emitter);
    let owner = r32(mem, emitter);
    if owner != 0 {
        w32(mem, owner, 0);
    }
    let next = r32(mem, emitter + 0xF4);
    if next != 0 {
        let previous = r32(mem, emitter + 0xF0);
        w32(mem, next + 0xF0, previous);
    } else {
        let previous = r32(mem, emitter + 0xF0);
        w32(mem, USED_TAIL, previous);
    }
    let previous = r32(mem, emitter + 0xF0);
    if previous != 0 {
        let next = r32(mem, emitter + 0xF4);
        w32(mem, previous + 0xF4, next);
    } else {
        let next = r32(mem, emitter + 0xF4);
        w32(mem, USED_HEAD, next);
    }
    let head = r32(mem, FREE_HEAD);
    w32(mem, emitter + 0xF0, 0);
    w32(mem, emitter + 0xF4, head);
    if head != 0 {
        w32(mem, head + 0xF0, emitter);
    }
    let count = r32(mem, USED_COUNT).wrapping_sub(1);
    let current = r32(mem, CURRENT_EMITTER);
    w32(mem, FREE_HEAD, emitter);
    w32(mem, USED_COUNT, count);
    if emitter == current {
        w32(mem, CURRENT_EMITTER, 0);
    }
}

/// `0x10E510(emitter)`: for emitters with flag 1 the visibility test decides: invisible ones are freed (returns 1); others are
/// marked `0x40000000` with `+0x7C = 0` (returns 0).
pub fn emitter_finish(
    mem: &mut dyn EmitterMemory,
    services: &mut dyn EmitterServices,
    emitter: u32,
) -> u32 {
    let flags = r32(mem, emitter + 4);
    if flags & 1 != 0 {
        if emitter_visible(mem, emitter) {
            return 0;
        }
        let current = r32(mem, CURRENT_EMITTER);
        emitter_free(mem, services, current);
        return 1;
    }
    w32(mem, emitter + 4, flags | 0x4000_0000);
    w32(mem, emitter + 0x7C, 0);
    0
}

/// `0x873F0(index, out_min, out_max)`: copy the min and max corner of region `index`.
pub fn region_bounds(mem: &mut dyn EmitterMemory, index: u32, out_min: u32, out_max: u32) {
    let record = r32(mem, REGION_TABLE).wrapping_add(index.wrapping_mul(0xB4));
    for k in 0..3 {
        let low = r32(mem, record + 0x10 + 4 * k);
        w32(mem, out_min + 4 * k, low);
        let high = r32(mem, record + 0x1C + 4 * k);
        w32(mem, out_max + 4 * k, high);
    }
}

/// The setters `0x10E660`, `0x10EAE0`, `0x10EB30`, `0x10EB70`, `0x10E9D0` and `0x10E8C0(handle, 0)` on the emitter created last
/// (they do nothing without one).
pub fn apply_param(mem: &mut dyn EmitterMemory, param: &EmitterParam) {
    let current = r32(mem, CURRENT_EMITTER);
    if current == 0 {
        return;
    }
    match *param {
        EmitterParam::Scale(value) => wf(mem, current + 0x64, value),
        EmitterParam::Span(a, b) => {
            wf(mem, current + 0x68, a);
            let current = r32(mem, CURRENT_EMITTER);
            wf(mem, current + 0x6C, b);
        }
        EmitterParam::Param70(value) => wf(mem, current + 0x70, value),
        EmitterParam::Param74(value) => wf(mem, current + 0x74, value),
        EmitterParam::Vec98(vector) => {
            for (k, component) in vector.iter().enumerate() {
                let current = r32(mem, CURRENT_EMITTER);
                wf(mem, current + 0x98 + 4 * k as u32, *component);
            }
            let flags = r32(mem, current + 4);
            w32(mem, current + 4, flags | 0x4000);
        }
        EmitterParam::Attach(handle) => {
            if handle == 0 {
                let flags = r32(mem, current + 4);
                w32(mem, current + 4, flags & 0xFFFF_FEFF);
            } else {
                region_bounds(mem, handle, current + 0xC8, current + 0xD4);
                let current = r32(mem, CURRENT_EMITTER);
                let flags = r32(mem, current + 4);
                w32(mem, current + 4, flags | 0x100);
            }
        }
    }
}

/// `0x153BD0`: the next draw of the small LCG as a double.
fn small_unit(mem: &mut dyn EmitterMemory) -> f64 {
    let state = r32(mem, SMALL_RNG)
        .wrapping_mul(0x343FD)
        .wrapping_add(0x26_9EC3);
    w32(mem, SMALL_RNG, state);
    f64::from((state >> 16) & 0x7FFF) * f64::from(f32::from_bits(0x3800_0100))
}

/// `0x111AE0(handle_out, template, flags, &position, &vector)`: create an emitter from a template. Takes a record from the pool
/// (returns 0 when empty), sets position (`+0x14`) and vector (`+0x2C`) from the pointers when non null, initialises it from the
/// template, ors the flags, then:
/// * flag `0x20` with a vector: the vector is spread by a random factor `(2u - 1) * template[0x78] + template[0x74]`
///   (`+0x98..`), negated into `+0x2C`, and the position advances by `0.04` of it;
/// * `+0xA8 = (2u - 1) * template[0x80] + template[0x7C]` (always);
/// * flag `2`: ground probe at `position + vector` (flag cleared; found: height in `+0x84`, flag `4`);
/// * flag `0x40`: bytes `+0x10` and `+0x12` cleared;
/// * flag `0x1000` is added unless the template is a type 7 or has a negative `+0x54` and the emitter lacks flag `0x10`; a
///   emitter without `0x1000` and with a position gets its regions refreshed (`0x10E2B0`) and the visibility finish `0x10E510`
///   (freed: returns 0);
/// * `handle_out` receives the emitter (and the emitter its address).
///
/// Returns 1 on success.
pub fn create_emitter(
    mem: &mut dyn EmitterMemory,
    services: &mut dyn EmitterServices,
    handle_out: u32,
    template: u32,
    flags: u32,
    position: u32,
    vector: u32,
) -> u32 {
    let emitter = emitter_alloc(mem);
    w32(mem, CURRENT_EMITTER, emitter);
    if emitter == 0 {
        return 0;
    }
    if position != 0 {
        for k in 0..3 {
            let value = r32(mem, position + 4 * k);
            w32(mem, emitter + 0x14 + 4 * k, value);
        }
    }
    if vector != 0 {
        for k in 0..3 {
            let value = r32(mem, vector + 4 * k);
            w32(mem, emitter + 0x2C + 4 * k, value);
        }
    }
    emitter_init(mem, emitter, template);
    w32(mem, emitter + 0x78, u32::MAX);
    w32(mem, emitter + 0x7C, u32::MAX);
    let merged = r32(mem, emitter + 4) | flags;
    w32(mem, emitter + 4, merged);
    if SvcMemory::r8(mem, emitter + 4) & 0x20 != 0 && vector != 0 {
        let draw = small_unit(mem);
        let spread = ((draw + draw) - 1.0) * f64::from(rf(mem, template + 0x78))
            + f64::from(rf(mem, template + 0x74));
        for k in 0..3 {
            let component = f64::from(rf(mem, vector + 4 * k));
            wf(mem, emitter + 0x98 + 4 * k, (spread * component) as f32);
        }
        for k in 0..3 {
            let negated = 0.0 - rf(mem, vector + 4 * k);
            wf(mem, emitter + 0x2C + 4 * k, negated);
        }
        let speed = r32(mem, template + 0x70);
        w32(mem, emitter + 0xA4, speed);
        let step = f32::from_bits(0x3D23_D70B);
        for k in 0..3 {
            let moved = rf(mem, emitter + 0x98 + 4 * k) * step + rf(mem, emitter + 0x14 + 4 * k);
            wf(mem, emitter + 0x14 + 4 * k, moved);
        }
    }
    let draw = small_unit(mem);
    let swing = ((draw + draw) - 1.0) * f64::from(rf(mem, template + 0x80))
        + f64::from(rf(mem, template + 0x7C));
    wf(mem, emitter + 0xA8, swing as f32);
    if SvcMemory::r8(mem, emitter + 4) & 2 != 0 {
        let point = [
            rf(mem, vector) + rf(mem, position),
            rf(mem, vector + 4) + rf(mem, position + 4),
            rf(mem, vector + 8) + rf(mem, position + 8),
        ];
        let flags = r32(mem, emitter + 4) & !2;
        w32(mem, emitter + 4, flags);
        let (height, found) = services.ground(point);
        if found != 0 {
            wf(mem, emitter + 0x84, height);
            let flags = (r32(mem, emitter + 4) | 4) & !2;
            w32(mem, emitter + 4, flags);
        }
    }
    if SvcMemory::r8(mem, emitter + 4) & 0x40 != 0 {
        SvcMemory::w8(mem, emitter + 0x12, 0);
        SvcMemory::w8(mem, emitter + 0x10, 0);
        let flags = r32(mem, emitter + 4) & !0x40;
        w32(mem, emitter + 4, flags);
    }
    let class = r32(mem, template + 0x54) as i32;
    let own = SvcMemory::r8(mem, emitter + 4) & 0x10 != 0;
    let long_lived = if class < 0 || r32(mem, template) == 7 {
        own
    } else {
        true
    };
    if long_lived {
        let flags = r32(mem, emitter + 4) | 0x1000;
        w32(mem, emitter + 4, flags);
    }
    if r32(mem, emitter + 4) & 0x1000 == 0 && position != 0 {
        refresh_regions(mem, emitter);
        if emitter_finish(mem, services, emitter) != 0 {
            return 0;
        }
    }
    if handle_out != 0 {
        let current = r32(mem, CURRENT_EMITTER);
        w32(mem, current, handle_out);
        w32(mem, handle_out, current);
    }
    1
}
