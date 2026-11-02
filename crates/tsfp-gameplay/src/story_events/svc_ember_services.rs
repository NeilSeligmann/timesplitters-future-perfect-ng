//! T1863: the ember engine services behind [`super::svc_particle_efbd0::EmberEngine`] and
//! [`super::svc_particle_kinds::KindEngine`]: `0xEDBE0` (settle), `0x46A00` (paired sound) and the volume setter `0x454E0`.
//!
//! Source: `build/default.xbe` (SHA-256 `3cfd001a84fc3e08175d6c4b2e42ebf49577a089b5d0bd87ac724f61a41816bc`). Decoded with
//! `tools/x87_blocks.py`, validated bit for bit against the original under the Unicorn oracle
//! (`tools/t1846_gold/svc_ember_services.py`, test `tests/story_events_t1863_ember_services.rs`). Document:
//! `docs/t-story-event-particle-kinds-t1863.md`. Names INFERRED, behaviour MEASURED (xemu level, not silicon).
//!
//! The services work on guest memory ([`SvcMemory`]) with the original addresses; the leaves they call that belong to other
//! systems (the level predicate dispatch `0x278040`, the object disturbance `0xE9380` that `svc_effect_submit` ports, the
//! sound submit `0x45820` and the voice hardware `0x27690`) are trait methods.

use super::prelude_1e2b80::init_spawn_record;
use super::svc_effect_submit::SvcMemory;
use super::svc_particle_efbd0::cvtt;

/// `[0x7DE455]`: the mode byte (`0xA` is the in-game mode these services act in).
pub const MODE_BYTE: u32 = 0x007D_E455;
/// `[0x7DE458]`: mode flags (bit `0x200` blocks the settle).
pub const MODE_FLAGS: u32 = 0x007D_E458;
/// Kind table `0x4DB490` (stride 24); the light radius is the float at `+0xC`.
pub const KIND_TABLE: u32 = 0x004D_B490;
/// `[0x4CA9B4]`: pointer to the sound table (stride 20, the byte at `+5` is the base volume).
pub const SOUND_TABLE_PTR: u32 = 0x004C_A9B4;
/// `[0x5236B8]`: non zero when the voice hardware is present (`0x454E0` forwards to `0x27690`).
pub const VOICE_HW_FLAG: u32 = 0x0052_36B8;

/// Leaves of `0xEDBE0`.
pub trait SettleServices {
    /// `0x278040(link)`: the level predicate of the linked object (non zero lets the settle run).
    fn actor_gate(&mut self, link: u32) -> bool;
    /// `0xE9380(position, shape_end, radius, out)`: disturb the objects near `position` (ported as
    /// `svc_effect_submit::disturb_objects`).
    fn disturb(
        &mut self,
        mem: &mut dyn SvcMemory,
        position: u32,
        shape_end: u32,
        radius: f32,
        out: u32,
    );
}

/// `0xEDBE0(position, shape_end, entry)`: after a start, wake the objects near an effect entry. Runs only in mode `0xA`, for
/// entries without flag bit 0 and while `[0x7DE458] & 0x200` is clear, and only for entries that are mode 1, whose source object
/// (type `0x200`) has flag bit 0 in its `+0x7C` block, or whose linked object passes `0x278040`. The radius is the fourth power
/// of `row.light * 1.5` for effect entries (type `0x17`), else `16.0`.
pub fn settle_entry(
    mem: &mut dyn SvcMemory,
    services: &mut dyn SettleServices,
    position: u32,
    shape_end: u32,
    entry: u32,
) {
    if mem.r8(MODE_BYTE) != 0xA {
        return;
    }
    if mem.r8(entry.wrapping_add(0xC)) & 1 != 0 {
        return;
    }
    if mem.r32(MODE_FLAGS) & 0x200 != 0 {
        return;
    }
    let mut run = mem.r32(entry.wrapping_add(0x100)) == 1;
    if !run {
        let source = mem.r32(entry.wrapping_add(0x108));
        if source != 0 && mem.r32(source.wrapping_add(0x20)) == 0x200 {
            let block = mem.r32(source.wrapping_add(0x7C));
            run = mem.r8(block.wrapping_add(8)) & 1 != 0;
        }
    }
    if !run {
        let link = mem.r32(entry.wrapping_add(0x104));
        if link == 0 || !services.actor_gate(link) {
            return;
        }
    }
    let radius = if mem.r32(entry) == 0x17 {
        let kind = mem.r32(entry.wrapping_add(4));
        let light = mem.rf(KIND_TABLE
            .wrapping_add(kind.wrapping_mul(24))
            .wrapping_add(0xC))
            * 1.5;
        let squared = light * light;
        squared * squared
    } else {
        16.0
    };
    services.disturb(mem, position, shape_end, radius, entry);
}

/// Leaves of `0x46A00` and `0x454E0`.
pub trait PairServices {
    /// `0x45820(&request)`: the sound submit; returns the handle (negative on failure).
    fn submit(&mut self, request: [u32; 12]) -> i32;
    /// `0x27690(handle, left, right)`: set the two channel volumes of a voice (hardware).
    fn voice_volumes(&mut self, handle: i32, left: i32, right: i32);
}

/// `0x454E0(handle, left, right)`: forwards to the voice hardware when it is present.
pub fn set_voice_volumes(
    mem: &dyn SvcMemory,
    services: &mut dyn PairServices,
    handle: i32,
    left: i32,
    right: i32,
) {
    if mem.r32(VOICE_HW_FLAG) != 0 {
        services.voice_volumes(handle, left, right);
    }
}

/// `0x46A00(id, scale, rate_a, rate_b)`: submit the sound twice (the request word `+0x14` is `rate_a` then `rate_b`) and give
/// the two voices swapped channel volumes, `base` and `trunc(base * scale)`, `base` being the sound table byte `[table + id * 20 + 5]`.
pub fn sound_pair(
    mem: &dyn SvcMemory,
    services: &mut dyn PairServices,
    id: u32,
    scale: f32,
    rate_a: f32,
    rate_b: f32,
) {
    let base = i32::from(
        mem.r8(mem
            .r32(SOUND_TABLE_PTR)
            .wrapping_add(id.wrapping_mul(20))
            .wrapping_add(5)),
    );
    let request = |rate: f32| {
        let mut words = init_spawn_record();
        words[0] = id;
        words[5] = rate.to_bits();
        words
    };
    let first = services.submit(request(rate_a));
    let second = services.submit(request(rate_b));
    let scaled = cvtt(base as f32 * scale);
    if first >= 0 {
        set_voice_volumes(mem, services, first, base, scaled);
    }
    if second >= 0 {
        set_voice_volumes(mem, services, second, scaled, base);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 0x186480: the pulse grid walk
// ---------------------------------------------------------------------------------------------------------------

/// `[0x74C720]`: pointer to 32 row pointers; each row is 32 `i16` node list heads (`-1` = empty), indexed by `(world + 4000) & 31`.
pub const PULSE_GRID: u32 = 0x0074_C720;
/// `[0x74C714]`: node table (stride 0x30: position at `+0`, group word `+0x22`, done byte `+0x24`, next in cell `+0x2A`).
pub const PULSE_NODES: u32 = 0x0074_C714;
/// `[0x74C6F8]`: group table (stride 24, first word is the group's kind).
pub const PULSE_GROUPS: u32 = 0x0074_C6F8;
/// `0x74C680`: 30 visited flags (one per group), cleared at every call.
pub const PULSE_VISITED: u32 = 0x0074_C680;
/// Number of visited flags cleared by `0x186480`.
pub const PULSE_VISITED_COUNT: u32 = 30;
/// Offset added to a world coordinate before the cell index (`[0x47DE34]`).
const GRID_BIAS: f32 = 4000.0;
/// Pulse strength for a node (`[0x47DE3C]`) and for a node whose group kind word is 0 (`[0x47DE38]`).
const PULSE_NEAR: f32 = f32::from_bits(0x4083_3333);
const PULSE_GROUP: f32 = f32::from_bits(0x4103_3333);

/// Leaves of `0x186480`.
pub trait PulseServices {
    /// `0x1836A0(node, value)`: start a pulse on a node (a record of the pulse pool `[0x74C67C]`); non zero when it did.
    fn node_pulse(&mut self, node: i32, value: f32) -> u32;
    /// `0x184040(&position)`: the region (portal) pulse for the same point; non zero when it did.
    fn region_pulse(&mut self, position: u32) -> u32;
    /// `0x45820(&request)`: the sound submit.
    fn submit(&mut self, request: [u32; 12]);
}

/// The CRT `ceil` (`0x3C8E75`) followed by `_ftol` (`0x3C848C`, a 64 bit `fistp` with a truncation fix up whose low dword is
/// returned): the low 32 bits of the 64 bit integer, 0 when the value does not fit 64 bits (or is NaN).
fn ceil_to_i32(value: f32) -> i32 {
    let ceiled = f64::from(value).ceil();
    if ceiled.is_nan() || ceiled.abs() >= 9_223_372_036_854_775_808.0 {
        0
    } else {
        (ceiled as i64) as i32
    }
}

/// The visiting order of the grid walk: `0, 1, -1, 2, -2, ...`.
fn zigzag(step: i32) -> i32 {
    if step > 0 {
        step.wrapping_neg()
    } else {
        1i32.wrapping_sub(step)
    }
}

/// `0x186480(position, radius_squared)`: stir the pulse nodes around a point. The grid cell of the point is
/// `trunc(coordinate + 4000) & 31` (x and z); `2 * ceil(radius_squared) + 3` rows and columns are visited in zigzag order around it.
/// Every node of a visited cell that is not done, whose group (word `+0x22`) is not yet visited, and whose squared distance is below
/// `radius_squared` receives two pulses `+limit` and `-limit` (limit `4.1`, or `8.2` when its group kind word is 0); when either
/// started the node is done and its group visited. Afterwards `0x184040` runs for the point; when either found something the sound
/// `0xC6B` is submitted at the point. Returns `found | region_result` (a bit OR of the two results, not a boolean).
pub fn light_pulse(
    mem: &mut dyn SvcMemory,
    services: &mut dyn PulseServices,
    position: u32,
    radius_squared: f32,
) -> u32 {
    let at = [mem.rf(position), mem.rf(position + 4), mem.rf(position + 8)];
    let cell_x = cvtt(at[0] + GRID_BIAS) & 0x1F;
    let cell_z = cvtt(at[2] + GRID_BIAS) & 0x1F;
    let width = ceil_to_i32(radius_squared)
        .wrapping_add(1)
        .wrapping_mul(2)
        .wrapping_add(1);
    for k in 0..PULSE_VISITED_COUNT {
        mem.w32(PULSE_VISITED + 4 * k, 0);
    }
    let mut found = 0u32;
    let mut row_step = 0i32;
    for _ in 0..width.max(0) {
        let rows = mem.r32(PULSE_GRID);
        let row = rows.wrapping_add(((row_step.wrapping_add(cell_x)) & 0x1F) as u32 * 4);
        let mut column_step = 0i32;
        for _ in 0..width {
            let list = mem.r32(row);
            let cell = ((column_step.wrapping_add(cell_z)) & 0x1F) as u32;
            let mut node = i32::from(mem.r16(list.wrapping_add(cell * 2)) as i16);
            while node != -1 {
                let current = node;
                let record = mem
                    .r32(PULSE_NODES)
                    .wrapping_add((current as u32).wrapping_mul(0x30));
                node = i32::from(mem.r16(record.wrapping_add(0x2A)) as i16);
                if mem.r8(record.wrapping_add(0x24)) != 0 {
                    continue;
                }
                let group = mem.r16(record.wrapping_add(0x22));
                let group_index = i32::from(group as i16) as u32;
                let group_slot = PULSE_VISITED.wrapping_add(group_index.wrapping_mul(4));
                if group != 0xFFFF && mem.r32(group_slot) != 0 {
                    continue;
                }
                let dy = mem.rf(record + 4) - at[1];
                let dx = mem.rf(record) - at[0];
                let dz = mem.rf(record + 8) - at[2];
                let distance = (dz * dz + dy * dy) + dx * dx;
                let mut limit = PULSE_NEAR;
                if group != 0xFFFF {
                    let kind = mem.r16(
                        mem.r32(PULSE_GROUPS)
                            .wrapping_add(group_index.wrapping_mul(24)),
                    );
                    if kind == 0 {
                        limit = PULSE_GROUP;
                    }
                }
                if !(radius_squared > distance) {
                    continue;
                }
                let first = services.node_pulse(current, limit);
                let second = services.node_pulse(current, 0.0 - limit);
                if first | second == 0 {
                    continue;
                }
                mem.w8(record.wrapping_add(0x24), 1);
                found = 1;
                if group != 0xFFFF {
                    mem.w32(group_slot, 1);
                }
            }
            column_step = zigzag(column_step);
        }
        row_step = zigzag(row_step);
    }
    let region = services.region_pulse(position);
    if found | region != 0 {
        let mut request = init_spawn_record();
        request[0] = 0xC6B;
        request[7] = at[0].to_bits();
        request[8] = at[1].to_bits();
        request[9] = at[2].to_bits();
        request[10] = 1;
        services.submit(request);
    }
    found | region
}

// ---------------------------------------------------------------------------------------------------------------
// 0xFF8A0: the spark burst
// ---------------------------------------------------------------------------------------------------------------

/// `[0x7497CC]`: the 32 bit LCG state of `0x153BB0` / `0x153BD0`.
pub const SMALL_RNG: u32 = 0x0074_97CC;
/// `[0x7AD294]`: detail scale; the burst has `trunc(scale * 14)` sparks.
pub const BURST_DETAIL: u32 = 0x007A_D294;
/// Effect record table `0x4E0FA0` (stride 0x1C): `+4` is copied, the float at `+8` sizes the burst.
pub const EFFECT_TABLE: u32 = 0x004E_0FA0;
/// `[0x708788]`: the region tree for `0x873C0`.
const RNG_SCALE: f32 = f32::from_bits(0x3800_0100);
const DEGREES_TO_RADIANS: f32 = f32::from_bits(0x3C8E_FA35);

/// Leaves of `0xFF8A0`.
pub trait TriggerServices {
    /// `0xFF2B0(1)` with `ebx = 0x1A`: take a burst record (pool class `0x1A`, evicting the oldest) and return its address.
    fn allocate(&mut self) -> u32;
    /// `0x1CDAE0()` (`jmp 0x3567C0`): 1 when the burst is for the local view; any other value gives the record flag `0x80000`.
    fn view_mode(&mut self) -> u32;
    /// `0x873C0(0, box, ring, 0xC)`: the region ids touched by the box (returns the count; `0` without a region tree).
    fn region_query(&mut self, box_start: u32, ring: u32) -> u32;
}

fn small_rng(mem: &mut dyn SvcMemory) -> u32 {
    let state = mem
        .r32(SMALL_RNG)
        .wrapping_mul(0x343FD)
        .wrapping_add(0x26_9EC3);
    mem.w32(SMALL_RNG, state);
    (state >> 16) & 0x7FFF
}

/// `0x153BD0`: the draw as a double (left in `st(0)`, not rounded).
fn small_rng_unit(mem: &mut dyn SvcMemory) -> f64 {
    f64::from(small_rng(mem)) * f64::from(RNG_SCALE)
}

/// The sign of `0x153BB0`'s low bit: `-1.0` when set.
fn small_rng_sign(mem: &mut dyn SvcMemory) -> f64 {
    if small_rng(mem) & 1 != 0 { -1.0 } else { 1.0 }
}

fn store(mem: &mut dyn SvcMemory, address: u32, value: f64) {
    mem.w32(address, (value as f32).to_bits());
}

/// `0xFF8A0(flag, id, &position)`: start the spark burst of effect record `id` at `position`. Takes a record, fills the header
/// (`+0xC = 0`, `+8 = 0x80000` unless `0x1CDAE0` is 1, `+0x14 = flag`, `+0x20 = position`, `+0x10 = 1.5`, `+4 = -1`, `+0x1C = count`
/// with `count = trunc([0x7AD294] * 14)`), then fills `count` particles: the first `count / 2` (tight, angle `(u + 0.5) * 360 deg`)
/// and the rest (wide, angle `(u + 1.0) * 360 deg`, offsets `+ 2.5 * speed`). With the flag `0x80000` the bounding box
/// `position - 1.7 .. position + 1.7` is registered through `0x873C0` (result in `+0x98`). Returns the record.
pub fn trigger_effect(
    mem: &mut dyn SvcMemory,
    services: &mut dyn TriggerServices,
    flag: u32,
    id: u32,
    position: u32,
) -> u32 {
    let record = services.allocate();
    let sub = mem.r32(record.wrapping_add(0x18));
    mem.wf(record.wrapping_add(0xC), 0.0);
    let view = services.view_mode();
    mem.w32(record.wrapping_add(8), if view == 1 { 0 } else { 0x8_0000 });
    mem.w32(record.wrapping_add(0x14), flag);
    for k in 0..3 {
        let word = mem.r32(position.wrapping_add(4 * k));
        mem.w32(record.wrapping_add(0x20 + 4 * k), word);
    }
    mem.wf(record.wrapping_add(0x10), 1.5);
    mem.w32(record.wrapping_add(4), u32::MAX);
    let count = cvtt(mem.rf(BURST_DETAIL) * 14.0);
    mem.w32(record.wrapping_add(0x1C), count as u32);
    mem.w32(sub, id);
    let row = EFFECT_TABLE.wrapping_add(id.wrapping_mul(0x1C));
    let word = mem.r32(row.wrapping_add(4));
    mem.w32(sub.wrapping_add(4), word);
    let size = mem.rf(row.wrapping_add(8));
    mem.wf(sub.wrapping_add(8), size);
    let scale = f64::from(size * f32::from_bits(0x3B03_126F));
    let at = [
        mem.rf(position),
        mem.rf(position.wrapping_add(4)),
        mem.rf(position.wrapping_add(8)),
    ];
    let half = |mem: &dyn SvcMemory| (mem.r32(record.wrapping_add(0x1C)) as i32) / 2;
    let total = |mem: &dyn SvcMemory| mem.r32(record.wrapping_add(0x1C)) as i32;
    let speed = |index: i32| {
        sub.wrapping_add(0xB4)
            .wrapping_add((index as u32).wrapping_mul(4))
    };
    let spark = |index: i32| {
        sub.wrapping_add(0xC)
            .wrapping_add((index as u32).wrapping_mul(12))
    };
    let jitter = f64::from(f32::from_bits(0x3ECC_CCCD));
    let mut index = 0i32;
    while index < half(mem) {
        let p = speed(index);
        let q = spark(index);
        let sign = small_rng_sign(mem);
        let angle = small_rng_unit(mem) + 0.5;
        mem.wf(p, 0.0);
        store(
            mem,
            p.wrapping_add(0x118),
            ((angle * sign) * 360.0) * f64::from(DEGREES_TO_RADIANS),
        );
        let kind = mem.r32(sub.wrapping_add(8));
        mem.w32(p.wrapping_add(0x38), kind);
        for axis in 0..3u32 {
            let draw = small_rng_unit(mem);
            let value = ((draw - 0.5) * jitter) * scale + f64::from(at[axis as usize]);
            store(mem, q.wrapping_add(4 * axis), value);
        }
        let x = mem.rf(q);
        mem.wf(q.wrapping_add(0x118), (x - at[0]) * 0.5);
        let tail = small_rng_unit(mem) * 0.5;
        store(mem, q.wrapping_add(0x11C), tail);
        let z = mem.rf(q.wrapping_add(8));
        mem.wf(q.wrapping_add(0x120), (z - at[2]) * 0.5);
        index += 1;
    }
    while index < total(mem) {
        let p = speed(index);
        let q = spark(index);
        let sign = small_rng_sign(mem);
        let angle = small_rng_unit(mem) + 1.0;
        store(
            mem,
            p.wrapping_add(0x118),
            ((angle * sign) * 360.0) * f64::from(DEGREES_TO_RADIANS),
        );
        let slow = small_rng_unit(mem) * f64::from(f32::from_bits(0x3E4C_CCCD));
        store(mem, p, slow);
        let kind = mem.rf(sub.wrapping_add(8)) * f32::from_bits(0x3F4C_CCCD);
        mem.wf(p.wrapping_add(0x38), kind);
        let draw = small_rng_unit(mem);
        store(mem, q, ((draw - 0.5) * scale) + f64::from(at[0]));
        let draw = small_rng_unit(mem);
        let speed_now = f64::from(mem.rf(p));
        store(
            mem,
            q.wrapping_add(4),
            ((draw - 0.5) * scale + speed_now * 2.5) + f64::from(at[1]),
        );
        let draw = small_rng_unit(mem);
        let x = mem.rf(q);
        store(
            mem,
            q.wrapping_add(8),
            ((draw - 0.5) * scale) + f64::from(at[2]),
        );
        mem.wf(q.wrapping_add(0x118), x - at[0]);
        let tail = small_rng_unit(mem);
        store(mem, q.wrapping_add(0x11C), tail);
        let z = mem.rf(q.wrapping_add(8));
        mem.wf(q.wrapping_add(0x120), z - at[2]);
        index += 1;
    }
    if mem.r32(record.wrapping_add(8)) & 0x8_0000 != 0 {
        let low = [
            at[0] - 0.5 - f32::from_bits(0x3F99_999A),
            at[1] - 0.5 - f32::from_bits(0x3F99_999A),
            at[2] - 0.5 - f32::from_bits(0x3F99_999A),
        ];
        let high = [
            at[0] + f32::from_bits(0x3FD9_999A),
            at[1] + f32::from_bits(0x3FD9_999A),
            at[2] + f32::from_bits(0x3FD9_999A),
        ];
        mem.wf(record.wrapping_add(0x80), low[0]);
        mem.wf(record.wrapping_add(0x8C), high[0]);
        mem.wf(record.wrapping_add(0x84), low[1]);
        mem.wf(record.wrapping_add(0x90), high[1]);
        mem.wf(record.wrapping_add(0x88), low[2]);
        mem.wf(record.wrapping_add(0x94), high[2]);
        let result = services.region_query(record.wrapping_add(0x80), record.wrapping_add(0x9C));
        mem.w32(record.wrapping_add(0x98), result);
    }
    record
}
