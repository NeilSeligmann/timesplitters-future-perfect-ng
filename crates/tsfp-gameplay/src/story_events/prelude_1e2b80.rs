//! Scene prelude `0x1E2B80` of the level 4 and level 0x50 hooks (T1846 batch 8). Decoded from `build/default.xbe` with
//! `tools/x87_blocks.py` and checked bit for bit against the original under Unicorn (`tools/x86_oracle.py`,
//! `tools/t1846_golden.py`, generator `tools/t1846_gold/prelude.py`). The golden vectors are MEASURED (emulated original
//! code on synthetic worlds); every name is INFERRED. Findings: `docs/t-story-event-prelude-t1846.md`.
//!
//! The model works on a flat 32 bit address space ([`PreludeEnv::read`] / [`PreludeEnv::write`]) with the same absolute
//! addresses and field offsets as the original, so that the hook wiring can back it with the real world memory.
//! Engine services (particles, object spawn, sounds, light ramps, the release call, the sweep query) are not executed:
//! each is recorded as a [`PreludeEffect`] in call order. The small object setters (`0x10E6E0`, `0x10E6F0`, `0x10EB50`,
//! `0x10E730`, `0x10EBB0`), the rng (`0x153B40`), the tag lookup (`0x2401F0`), the matrix rotation (`0x152090`), the
//! approach step (`0x1515E0`), the zone-to-mode table (`0x1E2AD0`) and the object init (`0x45390`) are ported.
//!
//! Registers/ABI: `0x1E2B80` takes no arguments (cdecl, void). Floats: SSE paths are plain f32 operations in source
//! order, x87 paths (`fild`/`fmul`/`fadd` chains from the rng) use f64 and round once at the `fstp`.

use std::collections::BTreeMap;

/// `[0x75BC04]`: pointer to the scene state record `S` (fields below).
pub const SCENE_PTR: u32 = 0x75BC04;
/// `[0x75BC0C]`: spawn counter, increased by `[0x4E7944]` each call and cleared on a spawn.
pub const SPAWN_COUNTER: u32 = 0x75BC0C;
/// `[0x75BC10]`: index of the spawn offset triple (12 byte entries at `0x4FEA34`).
pub const SPAWN_INDEX: u32 = 0x75BC10;
/// `[0x7B0C7C]`: pointer to the player record `G`.
pub const PLAYER_PTR: u32 = 0x7B0C7C;
/// `[0x4FEA30]`: enable flag of the ember spawn/update block (static value 1 in the XBE).
pub const ENABLE_FLAG: u32 = 0x4FEA30;
/// `[0x4E7944]`: per call counter step (static -1) and numerator of the heal-limit test.
pub const STEP_GLOBAL: u32 = 0x4E7944;
/// `[0x4E8CEC]`: level/difficulty number (static 0x3C), compared with 2 and used as a divisor.
pub const LEVEL_GLOBAL: u32 = 0x4E8CEC;
/// Rng state low and high dword (`0x153B40`).
pub const RNG_LO: u32 = 0x4E7950;
pub const RNG_HI: u32 = 0x4E7954;
/// `[0x7497C8]`: rng call counter.
pub const RNG_CALLS: u32 = 0x7497C8;
/// `[0x7A20C0]`: head of the intrusive entity list walked at the end.
pub const LIST_HEAD: u32 = 0x7A20C0;
/// `[0x72F5E8]`: base of the entity table (stride `0x2E8`, index `handle >> 22`, check dword at `+0x2C`).
pub const ENTITY_TABLE: u32 = 0x72F5E8;
/// `[0x74C458]`: base of the 0x200 byte per id records (`+0x140`, float at `+4`).
pub const ANIM_TABLE: u32 = 0x74C458;
/// Tag table scanned by `0x2401F0`: entries of 8 bytes (tag, list head) from here up to (excluding) the end.
pub const TAG_TABLE: u32 = 0x75CE68;
pub const TAG_TABLE_END: u32 = 0x75D318;
/// Tag `"ENLP"` looked up for the ember anchor.
pub const TAG_ENLP: u32 = 0x504C_4E45;

/// Scene record `S` field offsets (INFERRED names).
pub mod scene {
    pub const OBJECT: u32 = 0x4F8;
    pub const MODE: u32 = 0x674;
    pub const LEVEL_FACTOR: u32 = 0x680;
    pub const SLOTS: u32 = 0x684;
    pub const SLOT_STRIDE: u32 = 0x2C;
    pub const SLOT_COUNT: u32 = 5;
    pub const NEXT_SLOT: u32 = 0x760;
    pub const LIGHT_A: u32 = 0x7B4;
    pub const LIGHT_B: u32 = 0x7BC;
    pub const LIGHT_B_VALUE: u32 = 0x7C0;
}

/// `0x4FEA1C`: five integers scaled by `0.02..` into the target of the level factor (MEASURED static data).
pub const MODE_TARGETS: [i32; 5] = [0, 100, 25, 75, 0];
/// `0x4FEA34`: spawn offset triples (words); the first five are real, the rest is whatever follows in the data section.
pub const SPAWN_OFFSETS: [[u32; 3]; 8] = [
    [0xC260_0000, 0xC160_0000, 0x4210_0000],
    [0, 0xC246_74BC, 0],
    [0, 0xC246_0E56, 0],
    [1, 0x41FA_147B, 0xC1F1_5C29],
    [0xBF6E_147B, 0x4217_EB85, 0xC1EA_28F6],
    [0x4044_7AE1, 0, 0],
    [2, 0, 1],
    [0, 0, 0],
];
/// `0x1E2AD0` jump table: zone number `[G+0xF54]` (index) to mode 0..=4 (MEASURED by running the original for every index).
pub const ZONE_MODE: [u8; 0x5D] = [
    0, 4, 0, 0, 0, 4, 0, 4, 0, 4, 4, 4, 0, 4, 4, 4, 4, 0, 0, 0, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 0, 4,
    4, 4, 4, 0, 0, 0, 0, 4, 4, 4, 0, 0, 0, 0, 0, 4, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1,
    1, 2, 2, 0, 3, 3, 0, 0, 3, 3, 3, 3, 0, 3, 0, 0, 0, 3, 3, 3, 0, 1, 0, 0, 1, 0, 0, 0, 4,
];

/// `0x1E2AD0`: mode from the zone number (`[G+0xF54]`), 0 for zone 0 and numbers past the table.
pub fn zone_mode(zone: u32) -> u32 {
    u32::from(ZONE_MODE.get(zone as usize).copied().unwrap_or(0))
}

fn f(bits: u32) -> f32 {
    f32::from_bits(bits)
}
/// 2^-32, the scale of the unsigned rng value (`0x4785D0`).
const UNIT: f64 = 1.0 / 4294967296.0;

/// One recorded engine call. `words()` gives the argument words the golden log uses (pointers replaced by the values
/// they point to). Labels INFERRED, argument layout MEASURED.
#[derive(Clone, Debug, PartialEq)]
pub enum PreludeEffect {
    /// `0xEFBD0(0xF, -1, &pos, &dir, 0, 0, 0, 1)`: ember particle.
    Particle { pos: [f32; 3], dir: [f32; 3] },
    /// `0x45820(&record)`: spawn from a 0x30 byte record initialised by `0x45390` (see [`init_spawn_record`]).
    SpawnObject { words: [u32; 12] },
    /// `0x218A0(0x1A6, value)`.
    Sound1A6 { value: f32 },
    /// `0x21860(0x1A7, count, 0.5)`.
    Sound1A7 { count: i32 },
    /// `0xD1EC0(G, 0.6, a, b)`.
    PlayerNudge { player: u32, a: f32, b: f32 },
    /// `0xBD090(entry)`: `entry` is 0 when the handle was stale (the original calls with 0 as well).
    Release { entry: u32 },
    /// `0x468A0(target, 0, select, value, 50.0, 1.0, 0)`: light ramp (target `S+0x7B4`, select 3, or `S+0x7BC`, select 0x21).
    Light {
        target: u32,
        select: u32,
        value: f32,
    },
    /// `0x88C60(world, &from, &up, 1, &reach, &out, 0, -3, 0, 0)`: sweep query, result via [`PreludeEnv::sweep`].
    Sweep(SweepQuery),
    /// `0x10E2B0` (register `esi` = object), called by the `0x10E730` setter when bit `0x1000` of `[obj+4]` is clear.
    Refresh { object: u32 },
    /// `0xD1F10(0.1, 0.1, 0.1)`.
    Flash,
}

impl PreludeEffect {
    /// The log words of the effect (first word is the callee address).
    pub fn words(&self) -> Vec<u32> {
        let b = f32::to_bits;
        match self {
            Self::Particle { pos, dir } => {
                let mut w = vec![0xEFBD0, 0xF, 0xFFFF_FFFF];
                w.extend(pos.iter().chain(dir).map(|v| b(*v)));
                w.extend([0, 0, 0, 1]);
                w
            }
            Self::SpawnObject { words } => std::iter::once(0x45820)
                .chain(words.iter().copied())
                .collect(),
            Self::Sound1A6 { value } => vec![0x218A0, 0x1A6, b(*value)],
            Self::Sound1A7 { count } => vec![0x21860, 0x1A7, *count as u32, 0x3F00_0000],
            Self::PlayerNudge {
                player,
                a,
                b: second,
            } => vec![0xD1EC0, *player, 0x3F19_999A, b(*a), b(*second)],
            Self::Release { entry } => vec![0xBD090, *entry],
            Self::Light {
                target,
                select,
                value,
            } => vec![
                0x468A0,
                *target,
                0,
                *select,
                b(*value),
                0x4248_0000,
                0x3F80_0000,
                0,
            ],
            Self::Sweep(q) => {
                let mut w = vec![0x88C60, q.world];
                w.extend(q.from.iter().map(|v| b(*v)));
                w.extend(q.up.iter().map(|v| b(*v)));
                w.push(1);
                w.extend(q.reach.iter().map(|v| b(*v)));
                w.extend([0, 0xFFFF_FFFD, 0, 0]);
                w
            }
            Self::Refresh { object } => vec![0x10E2B0, *object],
            Self::Flash => vec![0xD1F10, 0x3DCC_CCCD, 0x3DCC_CCCD, 0x3DCC_CCCD],
        }
    }
}

/// Arguments of the `0x88C60` sweep query (fixed arguments: `1`, `0`, `-3`, `0`, `0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepQuery {
    /// `[[G+0x14]+0x34]`.
    pub world: u32,
    /// The three floats at `G+0xB4`.
    pub from: [f32; 3],
    /// Always `(0, 1, 0)`.
    pub up: [f32; 3],
    /// `4 * horizontal move direction`.
    pub reach: [f32; 3],
}

/// What the original reads back from the sweep buffers: the three floats of the reach buffer after the call and the
/// four floats of the output buffer (`hit[1]` is read).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepResult {
    pub reach: [f32; 3],
    pub hit: [f32; 4],
}

/// The world the prelude runs in. Memory is the flat address space of the original.
pub trait PreludeEnv {
    fn read(&self, address: u32) -> u32;
    fn write(&mut self, address: u32, value: u32);
    /// Records an engine call (order preserved).
    fn effect(&mut self, effect: PreludeEffect);
    /// The `0x88C60` sweep query (not ported, see the findings doc).
    fn sweep(&mut self, query: &SweepQuery) -> SweepResult;
}

/// A sparse zero-filled memory for tests and offline runs.
#[derive(Clone, Debug, Default)]
pub struct SparseMemory {
    pub words: BTreeMap<u32, u32>,
    pub effects: Vec<PreludeEffect>,
    pub sweep_result: Option<SweepResult>,
}

impl PreludeEnv for SparseMemory {
    fn read(&self, address: u32) -> u32 {
        self.words.get(&address).copied().unwrap_or(0)
    }
    fn write(&mut self, address: u32, value: u32) {
        self.words.insert(address, value);
    }
    fn effect(&mut self, effect: PreludeEffect) {
        self.effects.push(effect);
    }
    fn sweep(&mut self, _query: &SweepQuery) -> SweepResult {
        self.sweep_result.unwrap_or(SweepResult {
            reach: [0.0; 3],
            hit: [0.0; 4],
        })
    }
}

fn rf<E: PreludeEnv + ?Sized>(env: &E, address: u32) -> f32 {
    f32::from_bits(env.read(address))
}
fn wf<E: PreludeEnv + ?Sized>(env: &mut E, address: u32, value: f32) {
    env.write(address, value.to_bits());
}
fn vec3<E: PreludeEnv + ?Sized>(env: &E, address: u32) -> [f32; 3] {
    [rf(env, address), rf(env, address + 4), rf(env, address + 8)]
}
/// Byte store into the dword memory.
fn write_byte<E: PreludeEnv + ?Sized>(env: &mut E, address: u32, value: u8) {
    let aligned = address & !3;
    let shift = (address & 3) * 8;
    let word = (env.read(aligned) & !(0xFF << shift)) | (u32::from(value) << shift);
    env.write(aligned, word);
}

/// `0x153B40`: 64 bit LCG `state = state * 0x343FD + 0x269EC3`; returns the low dword. Counts calls in `[0x7497C8]`.
/// MEASURED.
pub fn rng<E: PreludeEnv + ?Sized>(env: &mut E) -> u32 {
    let calls = env.read(RNG_CALLS).wrapping_add(1);
    env.write(RNG_CALLS, calls);
    let state = (u64::from(env.read(RNG_HI)) << 32) | u64::from(env.read(RNG_LO));
    let next = state.wrapping_mul(0x343FD).wrapping_add(0x26_9EC3);
    env.write(RNG_LO, next as u32);
    env.write(RNG_HI, (next >> 32) as u32);
    next as u32
}

/// The rng value as the original's `fild`/`fadd 2^32`/`fmul 2^-32` chain: an unsigned fraction in `[0, 1)` held in an
/// extended register (f64 stands in; it is exact for the value itself).
fn unit(raw: u32) -> f64 {
    f64::from(raw) * UNIT
}

/// `0x1515E0(ptr, target, rate)`: moves `[ptr]` toward `target` by at most `rate`; returns true when it reached it.
/// MEASURED.
pub fn approach<E: PreludeEnv + ?Sized>(env: &mut E, pointer: u32, target: f32, rate: f32) -> bool {
    let current = rf(env, pointer);
    let distance = (target - current).abs();
    if rate > distance {
        wf(env, pointer, target);
        return true;
    }
    let moved = if target > current {
        current + rate
    } else {
        current - rate
    };
    wf(env, pointer, moved);
    false
}

/// `0x152090(matrix, v)`: rotates `v` by the 3x3 block of the 4 float stride rows at `+0/4/8`, `+0x10/14/18`, `+0x20/24/28`
/// (`v' = v.x * row0 + v.y * row1 + v.z * row2`, summed z first). MEASURED.
pub fn rotate_vector<E: PreludeEnv + ?Sized>(env: &E, matrix: u32, v: [f32; 3]) -> [f32; 3] {
    let m = |offset: u32| rf(env, matrix + offset);
    let (x, y, z) = (v[0], v[1], v[2]);
    [
        (m(0x20) * z + m(0x10) * y) + m(0) * x,
        (m(0x24) * z + m(0x14) * y) + m(4) * x,
        (m(0x28) * z + m(0x18) * y) + m(8) * x,
    ]
}

/// `1 / sqrt(x)` as the original `rsqrtss` executes (exact in the oracle, QEMU's divide of 1 by the root).
fn rsqrt(x: f32) -> f32 {
    1.0 / x.sqrt()
}

/// `0x45390(record)`: initialises a 0x30 byte spawn record: zero fill, `+0x08 = 1.0`, `+0x10 = 0x14`, `+0x14 = 1.0`,
/// `+0x18 = 1.0`, `+0x28 = 0`, `+0x2C = 50.0`. MEASURED.
pub fn init_spawn_record() -> [u32; 12] {
    let mut w = [0u32; 12];
    w[2] = 0x3F80_0000;
    w[4] = 0x14;
    w[5] = 0x3F80_0000;
    w[6] = 0x3F80_0000;
    w[11] = 0x4248_0000;
    w
}

/// `0x2401F0(tag)`: picks a random live entry among the lists registered under `tag` in the table
/// `0x75CE68..0x75D318` and returns the entity table entry (0 when none). Consumes one rng draw when any list exists.
/// A matching entry's count accumulates over all matching table rows while the chosen row is the last match
/// (behaviour of the original). Clears the list node handle when its entity entry is stale. MEASURED.
pub fn find_by_tag<E: PreludeEnv + ?Sized>(env: &mut E, tag: u32) -> u32 {
    let mut count = 0u32;
    let mut row = 0u32;
    let mut cursor = TAG_TABLE;
    while cursor < TAG_TABLE_END {
        if env.read(cursor) == tag {
            let mut node = env.read(cursor + 4);
            row = cursor;
            while node != 0 {
                node = env.read(node + 4);
                count += 1;
            }
        }
        cursor += 8;
    }
    if row == 0 || count == 0 {
        return 0;
    }
    let pick = rng(env) % count;
    let mut node = env.read(row + 4);
    if node == 0 {
        return 0;
    }
    let mut index = 0;
    while index != pick {
        node = env.read(node + 4);
        index += 1;
        if node == 0 {
            return 0;
        }
    }
    let handle = env.read(node);
    if handle == 0 {
        return 0;
    }
    let entry = env
        .read(ENTITY_TABLE)
        .wrapping_add((handle >> 22).wrapping_mul(0x2E8));
    if env.read(entry + 0x2C) == handle {
        entry
    } else {
        env.write(node, 0);
        0
    }
}

/// `0x10EBB0(obj)`: float at `+0x58`, or 0.0 for a null object. MEASURED.
fn object_height<E: PreludeEnv + ?Sized>(env: &E, object: u32) -> f32 {
    if object == 0 {
        0.0
    } else {
        rf(env, object + 0x58)
    }
}

/// `0x10E6E0` (flag 1) and `0x10E6F0` (flag 0): byte `+0x12` of a non null object. MEASURED.
fn set_object_flag<E: PreludeEnv + ?Sized>(env: &mut E, object: u32, value: u8) {
    if object != 0 {
        write_byte(env, object + 0x12, value);
    }
}

/// `0x10EB50(obj, v)`: float `+0x70` of a non null object. MEASURED.
fn set_object_value<E: PreludeEnv + ?Sized>(env: &mut E, object: u32, value: f32) {
    if object != 0 {
        wf(env, object + 0x70, value);
    }
}

/// `0x10E730(obj, &v)`: copies the vector to `+0x14..0x20` and, when bit `0x1000` of `[obj+4]` is clear, calls `0x10E2B0`.
/// MEASURED (the callee is recorded as [`PreludeEffect::Refresh`]).
fn set_object_vector<E: PreludeEnv + ?Sized>(env: &mut E, object: u32, v: [f32; 3]) {
    if object == 0 {
        return;
    }
    for (i, c) in v.iter().enumerate() {
        wf(env, object + 0x14 + 4 * i as u32, *c);
    }
    if env.read(object + 4) & 0x1000 == 0 {
        env.effect(PreludeEffect::Refresh { object });
    }
}

/// `cvttss2si` (overflow and NaN give `i32::MIN`).
fn cvtt(value: f32) -> i32 {
    if value.is_nan() || value >= 2147483648.0 || value < -2147483648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

/// `0x1E2B80`. See the module doc and `docs/t-story-event-prelude-t1846.md` for the full flow. Steps in order:
/// 1. mode from the zone, level factor `S+0x680` approaches `MODE_TARGETS[mode] * 0.02` by 0.02.
/// 2. Modes 1 and 2 with the enable flag: ember anchor spawn into the ring of five slots, then the per slot update
///    (particle effect, drift, occasional object spawn).
/// 3. Mode 1: rare random player kick (sounds, `PlayerNudge`, `G+0xFBC/0xFC0`).
/// 4. Release stale handles of the entity list, two light ramps.
/// 5. Mode 4: object flag/vector update with the sweep query; other modes clear the object flag.
pub fn prelude_1e2b80<E: PreludeEnv>(env: &mut E) {
    let player = env.read(PLAYER_PTR);
    let mode = zone_mode(env.read(player + 0xF54));
    let scene = env.read(SCENE_PTR);
    env.write(scene + scene::MODE, mode);
    let target = (MODE_TARGETS[mode as usize] as f32) * f(0x3CA3_D70A);
    approach(env, scene + scene::LEVEL_FACTOR, target, f(0x3CA3_D70A));

    let enabled = env.read(ENABLE_FLAG) != 0;
    let mode = env.read(scene + scene::MODE);
    if enabled && (mode == 1 || mode == 2) {
        ember_block(env, scene);
    }
    if env.read(scene + scene::MODE) == 1 {
        player_kick(env);
    }
    release_stale_handles(env);
    let value = env.read(scene + scene::LEVEL_FACTOR);
    env.effect(PreludeEffect::Light {
        target: scene + scene::LIGHT_A,
        select: 3,
        value: f(value),
    });
    let value = env.read(scene + scene::LIGHT_B_VALUE);
    env.effect(PreludeEffect::Light {
        target: scene + scene::LIGHT_B,
        select: 0x21,
        value: f(value),
    });

    let mode = env.read(scene + scene::MODE);
    let object = env.read(scene + scene::OBJECT);
    if mode != 4 {
        set_object_flag(env, object, 0);
        return;
    }
    mode4_tail(env, scene);
}

/// Ember anchor spawn (counter, `0x2401F0`, slot ring write) and the per slot update.
fn ember_block<E: PreludeEnv>(env: &mut E, scene: u32) {
    let counter = (env.read(SPAWN_COUNTER) as i32).wrapping_add(env.read(STEP_GLOBAL) as i32);
    env.write(SPAWN_COUNTER, counter as u32);
    let threshold = i32::from(env.read(LEVEL_GLOBAL) as i32 > 2);
    if counter > threshold && unit(rng(env)) > f64::from(f(0x3F7A_E148)) {
        spawn_slot(env, scene);
    }
    let skip_spawn_after_first = unit(rng(env)) > 0.5;
    let mut spawned = skip_spawn_after_first;
    for k in 0..scene::SLOT_COUNT {
        let slot = scene + k * scene::SLOT_STRIDE;
        let life = env.read(slot + 0x6A8);
        if life == 0 {
            continue;
        }
        let timer = env.read(slot + 0x6AC);
        if timer != 0 {
            env.write(slot + 0x6AC, timer - 1);
            continue;
        }
        env.write(slot + 0x6AC, 10);
        env.write(slot + 0x6A8, life.wrapping_sub(1));
        let mut at = vec3(env, slot + 0x684);
        for c in at.iter_mut() {
            let r = unit(rng(env));
            *c = ((r - 0.5) * 10.0 + f64::from(*c)) as f32;
        }
        let anchor = vec3(env, slot + 0x690);
        let mut d = [at[0] - anchor[0], at[1] - anchor[1], at[2] - anchor[2]];
        let len2 = (d[2] * d[2] + d[1] * d[1]) + d[0] * d[0];
        if len2 > f(0x33D6_BF95) {
            let inv = rsqrt(len2) * 1.0;
            d = [inv * d[0], inv * d[1], inv * d[2]];
        }
        env.effect(PreludeEffect::Particle { pos: at, dir: d });
        for i in 0..3 {
            let sum = rf(env, slot + 0x69C + 4 * i) + rf(env, slot + 0x684 + 4 * i);
            wf(env, slot + 0x684 + 4 * i, sum);
        }
        if spawned {
            continue;
        }
        spawned = true;
        let m = (rf(env, scene + scene::LEVEL_FACTOR) as f64 * f64::from(f(0x3CCC_CCCD))) as f32;
        let q = (unit(rng(env)) * f64::from(m) + f64::from(m)) as f32;
        let p = ((unit(rng(env)) - 0.5) * f64::from(f(0x3DCC_CCCD)) + 1.0) as f32;
        let mut words = init_spawn_record();
        words[0] = 0xC6F;
        words[5] = p.to_bits();
        words[6] = q.to_bits();
        env.effect(PreludeEffect::SpawnObject { words });
    }
}

/// The anchor lookup and ring slot write of [`ember_block`] (the `0x1E2C10..0x1E2E3B` part).
fn spawn_slot<E: PreludeEnv>(env: &mut E, scene: u32) {
    let index = env.read(SPAWN_INDEX) as usize;
    let offsets = SPAWN_OFFSETS.get(index).copied().unwrap_or([0; 3]);
    let node = find_by_tag(env, TAG_ENLP);
    if node == 0 {
        return;
    }
    let inner = env.read(node + 4);
    if inner == 0 {
        return;
    }
    let matrix = env.read(inner + 0x10);
    if matrix == 0 {
        return;
    }
    env.write(SPAWN_COUNTER, 0);
    let v = rotate_vector(env, matrix, [0.0, 0.0, 1.0]);
    let slot_index = env.read(scene + scene::NEXT_SLOT);
    let slot = scene.wrapping_add(slot_index.wrapping_mul(scene::SLOT_STRIDE));
    for (i, c) in v.iter().enumerate() {
        wf(
            env,
            slot + 0x69C + 4 * i as u32,
            (f64::from(*c) * 3.75) as f32,
        );
    }
    let pos_x = (f64::from(v[0]) * 50.0 + f64::from(rf(env, node + 0x64))) as f32;
    let pos_y = (f64::from(v[1]) * 50.0 + f64::from(rf(env, node + 0x68))) as f32;
    let pos_z = ((f64::from(v[2]) * 50.0) as f32 as f64 + f64::from(rf(env, node + 0x6C))) as f32;
    wf(env, slot + 0x684, pos_x);
    wf(env, slot + 0x688, pos_y);
    wf(env, slot + 0x68C, pos_z);
    for (i, w) in offsets.iter().enumerate() {
        env.write(slot + 0x690 + 4 * i as u32, *w);
    }
    env.write(slot + 0x6A8, 5);
    env.write(slot + 0x6AC, 0);
    env.write(
        scene + scene::NEXT_SLOT,
        ((slot_index as i32).wrapping_add(1) % 5) as u32,
    );
}

/// Mode 1 rare event (`0x1E3158..0x1E32C1`): when a draw exceeds 0.999, the player record is not in state `0x100`, the
/// player's linked object exists and its value at `+0x398` is not positive.
fn player_kick<E: PreludeEnv>(env: &mut E) {
    if unit(rng(env)) <= f64::from(f(0x3F7F_BE77)) {
        return;
    }
    let player = env.read(PLAYER_PTR);
    if env.read(player + 0x114) == 0x100 {
        return;
    }
    let object = env.read(player + 8);
    if object == 0 || !(rf(env, object + 0x398) <= 0.0) {
        return;
    }
    let ext = unit(rng(env)) * f64::from(f(0x3F33_3333)) + f64::from(f(0x3E99_999A));
    let w = ext as f32;
    env.effect(PreludeEffect::Sound1A6 {
        value: (ext * 0.5) as f32,
    });
    if unit(rng(env)) > 0.75 {
        let count = cvtt(env.read(LEVEL_GLOBAL) as i32 as f32);
        env.effect(PreludeEffect::Sound1A7 { count });
    }
    let player = env.read(PLAYER_PTR);
    env.effect(PreludeEffect::PlayerNudge {
        player,
        a: w * f(0x3DCC_CCCD),
        b: w * f(0x3CF5_C28F),
    });
    wf(env, player + 0xFBC, w * f(0x3F66_6666));
    wf(env, player + 0xFC0, w * f(0x3F19_999A));
}

/// Walk of the entity list `[0x7A20C0]` (`0x1E32C6..0x1E3336`): entities whose id record float `+4` is below -50 release
/// their four handle slots (`+0x70`, stride `0x34`); a stale handle is cleared first and releases entry 0.
fn release_stale_handles<E: PreludeEnv>(env: &mut E) {
    let mut entity = env.read(LIST_HEAD);
    while entity != 0 {
        let id = env.read(entity + 0x48);
        let record = env
            .read(ANIM_TABLE)
            .wrapping_add((id & 0xFFFF) << 9)
            .wrapping_add(0x140);
        if f(0xC248_0000) > rf(env, record + 4) {
            for k in 0..4 {
                let slot = entity + 0x70 + k * 0x34;
                let handle = env.read(slot);
                if handle == 0 {
                    continue;
                }
                let entry = env
                    .read(ENTITY_TABLE)
                    .wrapping_add((handle >> 22).wrapping_mul(0x2E8));
                let arg = if env.read(entry + 0x2C) == handle {
                    entry
                } else {
                    env.write(slot, 0);
                    0
                };
                env.effect(PreludeEffect::Release { entry: arg });
            }
        }
        entity = env.read(entity);
    }
}

/// Mode 4 tail (`0x1E3399..0x1E36FE`): object flag, ground sweep, object value and flashes.
fn mode4_tail<E: PreludeEnv>(env: &mut E, scene: u32) {
    let object = env.read(scene + scene::OBJECT);
    set_object_flag(env, object, 1);
    let object = env.read(scene + scene::OBJECT);
    set_object_value(env, object, 1.0);
    let player = env.read(PLAYER_PTR);
    let b = vec3(env, player + 0xB4);
    let c = vec3(env, player + 0x158);
    let anchor = [
        (b[0] + b[0]) - c[0],
        (b[1] + b[1]) - c[1],
        (b[2] + b[2]) - c[2],
    ];
    let heading = vec3(env, player + 0xC0);
    let len2 = heading[2] * heading[2] + heading[0] * heading[0];
    let mut dir = [heading[0], 0.0, heading[2]];
    if len2 > f(0x33D6_BF95) {
        let inv = rsqrt(len2) * 1.0;
        dir = [inv * dir[0], inv * 0.0, inv * dir[2]];
    }
    let four = f(0x4080_0000);
    let reach = [dir[0] * four, dir[1] * four, dir[2] * four];
    let mut target = [
        reach[0] + anchor[0],
        reach[1] + anchor[1],
        reach[2] + anchor[2],
    ];
    let world = env.read(env.read(player + 0x14) + 0x34);
    let query = SweepQuery {
        world,
        from: vec3(env, player + 0xB4),
        up: [0.0, 1.0, 0.0],
        reach,
    };
    env.effect(PreludeEffect::Sweep(query));
    let result = env.sweep(&query);
    target[1] = result.reach[1] - f(0x3DCC_CCCD);
    let object = env.read(scene + scene::OBJECT);
    set_object_vector(env, object, target);
    let dy = result.reach[1] - rf(env, player + 0xB8);
    let blocked =
        dy > f(0x4120_0000) || f(0x3DCC_CCCD) > dy || result.hit[1] + 1.0 > f(0x3D4C_CCCD);
    let object = env.read(scene + scene::OBJECT);
    if blocked {
        if f64::from(object_height(env, object)) > f64::from(f(0x3F4C_CCCD)) {
            let object = env.read(scene + scene::OBJECT);
            set_object_value(env, object, 0.0);
            return;
        }
    }
    let object = env.read(scene + scene::OBJECT);
    let limit =
        (1.0 / f64::from(env.read(LEVEL_GLOBAL) as i32)) * f64::from(env.read(STEP_GLOBAL) as i32);
    let height = f64::from(object_height(env, object));
    if limit > height && rf(env, object + 0x70) != 0.0 {
        let mut words = init_spawn_record();
        words[0] = 0x1A6;
        words[6] = 0x3ECC_CCCD;
        env.effect(PreludeEffect::SpawnObject { words });
    }
    let object = env.read(scene + scene::OBJECT);
    if f64::from(f(0x3E99_999A)) > f64::from(object_height(env, object))
        && rf(env, object + 0x70) != 0.0
    {
        env.effect(PreludeEffect::Flash);
    }
}
