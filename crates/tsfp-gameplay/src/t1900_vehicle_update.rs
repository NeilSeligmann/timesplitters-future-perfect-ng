// SPDX-License-Identifier: GPL-3.0-or-later
//! T1900 slice B: `0x1E2B80`, the per-frame scene update of the "ember" levels (Tier 0, closure model).
//!
//! Despite its lifted name (`..._player_state_jump_table_0_to_0x5b_forced_kill_and_effects`) this function
//! is not a vehicle update. It asks `0x1E2AD0` (the player-zone jump table 0..0x5b) for a scene mode 0..=4,
//! eases the scene level factor toward a per-mode target, spawns and updates five ember slots (modes 1, 2),
//! rarely kicks the player (mode 1), releases stale entity handles, ramps two lights and, in mode 4,
//! updates a tracked object with a ground sweep. `story_events::prelude_1e2b80` already ports the whole
//! function (flat-memory effect-recording model, MEASURED against the original); this file is the same
//! flow in the T1815 closure model: every engine service is a [`Callee`] target, all state lives in guest
//! memory, static constants are read from their original addresses.
//!
//! Conventions: cdecl, no arguments, no result. A callee that returns a float (`0x10EBB0`) is modelled as
//! returning the f32 bits in `eax` (the original returns it in ST0). x87 chains are done in f64 and rounded
//! at the store, SSE paths in f32, as in the original. Scratch locals live in a fixed frame below `esp`.
//! Verification tier: 0 (translated, compiles, smoke tests). Nothing here is differential.
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu, guest_call},
    memory::GuestMemory,
};

/// `0x1E2B80` entry.
pub const SCENE_UPDATE: u32 = 0x001e_2b80;

/// Callee targets (owning system in the doc of [`scene_per_frame_update`]).
pub mod target {
    pub const ZONE_MODE: u32 = 0x001e_2ad0;
    pub const APPROACH: u32 = 0x0015_15e0;
    pub const RNG: u32 = 0x0015_3b40;
    pub const FIND_BY_TAG: u32 = 0x0024_01f0;
    pub const ROTATE_VECTOR: u32 = 0x0015_2090;
    pub const PARTICLE: u32 = 0x000e_fbd0;
    pub const INIT_SPAWN_RECORD: u32 = 0x0004_5390;
    pub const SPAWN_OBJECT: u32 = 0x0004_5820;
    pub const SOUND_1A6: u32 = 0x0002_18a0;
    pub const SOUND_1A7: u32 = 0x0002_1860;
    pub const PLAYER_NUDGE: u32 = 0x000d_1ec0;
    pub const ANIM_RECORD: u32 = 0x0016_79d0;
    pub const RELEASE_HANDLE: u32 = 0x000b_d090;
    pub const LIGHT_RAMP: u32 = 0x0004_68a0;
    pub const OBJECT_FLAG_ON: u32 = 0x0010_e6e0;
    pub const OBJECT_FLAG_OFF: u32 = 0x0010_e6f0;
    pub const OBJECT_SET_VALUE: u32 = 0x0010_eb50;
    pub const OBJECT_GET_HEIGHT: u32 = 0x0010_ebb0;
    pub const OBJECT_SET_VECTOR: u32 = 0x0010_e730;
    pub const SWEEP: u32 = 0x0008_8c60;
    pub const FLASH: u32 = 0x000d_1f10;
}

/// Globals (absolute addresses of the original).
pub mod global {
    pub const SCENE_PTR: u32 = 0x0075_bc04;
    pub const SPAWN_COUNTER: u32 = 0x0075_bc0c;
    pub const SPAWN_INDEX: u32 = 0x0075_bc10;
    pub const PLAYER_PTR: u32 = 0x007b_0c7c;
    pub const ENABLE_FLAG: u32 = 0x004f_ea30;
    pub const MODE_TARGET_TABLE: u32 = 0x004f_ea1c;
    pub const SPAWN_OFFSET_TABLE: u32 = 0x004f_ea34;
    pub const STEP: u32 = 0x004e_7944;
    pub const LEVEL: u32 = 0x004e_8cec;
    pub const ENTITY_LIST_HEAD: u32 = 0x007a_20c0;
    pub const ENTITY_TABLE: u32 = 0x0072_f5e8;
}

/// Static float constants read from the XBE data at run time.
mod konst {
    pub const LEVEL_FACTOR_STEP: u32 = 0x0047_85d4; // 0.02
    pub const TWO_POW_32: u32 = 0x0047_5ccc;
    pub const TWO_POW_MINUS_32: u32 = 0x0047_85d0;
    pub const SPAWN_CHANCE: u32 = 0x0047_d6dc; // 0.98..
    pub const HALF: u32 = 0x0047_5cd4;
    pub const ONE: u32 = 0x0047_5c78;
    pub const TEN: u32 = 0x0047_862c;
    pub const TINY: u32 = 0x0047_65f4;
    pub const SPAWN_SCALE: u32 = 0x0047_d5bc;
    pub const TENTH: u32 = 0x0047_5d24;
    pub const KICK_CHANCE: u32 = 0x0047_5dc8; // 0.999
    pub const KICK_SCALE: u32 = 0x0047_85e0; // 0.7
    pub const KICK_BASE: u32 = 0x0047_5cdc; // 0.3
    pub const KICK_SOUND_CHANCE: u32 = 0x0047_5d40; // 0.75
    pub const NUDGE_B: u32 = 0x0047_5cf8;
    pub const KICK_FBC: u32 = 0x0047_6040;
    pub const KICK_FC0: u32 = 0x0047_5cc8;
    pub const STALE_LIMIT: u32 = 0x0047_da88; // -50.0
    pub const FOUR: u32 = 0x0047_5db4;
    pub const BLOCK_STEP: u32 = 0x0047_5ce8; // 0.05
    pub const HIGH_OBJECT: u32 = 0x0047_5cd0; // 0.8
    pub const SPAWN_VALUE: u32 = 0x0047_5d04; // 0.4
    pub const LOW_OBJECT: u32 = 0x0047_5cdc; // 0.3
    pub const SCALE_X_DOUBLE: u32 = 0x0049_e120; // 3.75
    pub const SCALE_POS_DOUBLE: u32 = 0x0049_e118; // 50.0
}

/// Scene record field offsets (names INFERRED, as in the prelude port).
pub mod scene {
    pub const OBJECT: u32 = 0x4f8;
    pub const MODE: u32 = 0x674;
    pub const LEVEL_FACTOR: u32 = 0x680;
    pub const SLOTS: u32 = 0x684;
    pub const SLOT_STRIDE: u32 = 0x2c;
    pub const SLOT_COUNT: u32 = 5;
    pub const NEXT_SLOT: u32 = 0x760;
    pub const LIGHT_A: u32 = 0x7b4;
    pub const LIGHT_B: u32 = 0x7bc;
    pub const LIGHT_B_VALUE: u32 = 0x7c0;
}

/// Offsets inside one ember slot (relative to `slot = scene + k * 0x2C`, `SLOTS` based fields spelled absolute).
mod slot {
    pub const POSITION: u32 = 0x684;
    pub const ANCHOR: u32 = 0x690;
    pub const DRIFT: u32 = 0x69c;
    pub const LIFE: u32 = 0x6a8;
    pub const TIMER: u32 = 0x6ac;
}

/// Player record offsets.
mod player {
    pub const OBJECT: u32 = 8;
    pub const POSITION: u32 = 0xb4;
    pub const HEADING: u32 = 0xc0;
    pub const STATE: u32 = 0x114;
    pub const SCENE_ANCHOR: u32 = 0x158;
    pub const KICK_X: u32 = 0xfbc;
    pub const KICK_Y: u32 = 0xfc0;
    pub const WORLD_HOLDER: u32 = 0x14;
    pub const WORLD: u32 = 0x34;
    pub const IDLE_STATE: u32 = 0x100;
    pub const LINKED_VALUE: u32 = 0x398;
}

const TAG_ENLP: u32 = 0x504c_4e45;
const EMBER_OBJECT_ID: u32 = 0xc6f;
const MODE_FLAG_OBJECT_ID: u32 = 0x1a6;
const FRAME_BYTES: u32 = 0x100;
/// Scratch frame offsets, relative to `esp - FRAME_BYTES` (`B` in the notes).
mod frame {
    pub const TARGET: u32 = 0x14; // anchor, then target = anchor + reach
    pub const REACH: u32 = 0x20; // heading copy, then 4 * direction (also read back after the sweep)
    pub const UP: u32 = 0x2c; // (0, 1, 0)
    pub const SWEEP_OUT: u32 = 0x38;
    pub const RECORD: u32 = 0x50; // 0x30 byte spawn record
    pub const POS: u32 = 0x90;
    pub const DIR: u32 = 0xa0;
    pub const VEC: u32 = 0xb0;
}

struct Ctx<'a, M: GuestMemory, C: Callee<M>> {
    c: &'a mut C,
    m: &'a mut M,
    cpu: &'a mut Cpu,
    base: u32,
}

impl<M: GuestMemory, C: Callee<M>> Ctx<'_, M, C> {
    /// One cdecl call; returns `eax`.
    fn call(&mut self, target: u32, return_pc: u32, args: &[u32]) -> u32 {
        guest_call(
            self.c,
            self.m,
            self.cpu,
            target,
            return_pc,
            FRAME_BYTES,
            args,
        );
        self.cpu.eax
    }
    fn read(&self, address: u32) -> u32 {
        self.m.read32(address)
    }
    fn write(&mut self, address: u32, value: u32) {
        self.m.write32(address, value);
    }
    fn f(&self, address: u32) -> f32 {
        f32::from_bits(self.m.read32(address))
    }
    fn wf(&mut self, address: u32, value: f32) {
        self.m.write32(address, value.to_bits());
    }
    fn double(&self, address: u32) -> f64 {
        f64::from_bits(u64::from(self.read(address)) | (u64::from(self.read(address + 4)) << 32))
    }
    fn local(&self, offset: u32) -> u32 {
        self.base.wrapping_add(offset)
    }
    /// `0x153B40` draw as the original's x87 chain: unsigned fraction in `[0, 1)` (f64).
    fn unit(&mut self, return_pc: u32) -> f64 {
        let raw = self.call(target::RNG, return_pc, &[]) as i32;
        let mut value = f64::from(raw);
        if raw < 0 {
            value += f64::from(self.f(konst::TWO_POW_32));
        }
        value * f64::from(self.f(konst::TWO_POW_MINUS_32))
    }
    fn vec3(&self, address: u32) -> [f32; 3] {
        [self.f(address), self.f(address + 4), self.f(address + 8)]
    }
    fn set_vec3(&mut self, address: u32, v: [f32; 3]) {
        for (i, c) in v.iter().enumerate() {
            self.wf(address + 4 * i as u32, *c);
        }
    }
}

/// `1 / sqrt(x)` as `rsqrtss` is lifted.
fn rsqrt(x: f32) -> f32 {
    1.0 / x.sqrt()
}

/// `cvttss2si`: overflow and NaN give `i32::MIN`.
fn cvtt(value: f32) -> i32 {
    if value.is_nan() || value >= 2147483648.0 || value < -2147483648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

/// `0x1E2B80` game scene per-frame update (the ember-level scene prelude). cdecl(0), void.
///
/// Crossing out (callee VA, system INFERRED): `1E2AD0` zone to mode table (this slice's sibling), `153B40`
/// rng and `1515E0`/`152090` approach and matrix rotate (math), `2401F0` tag lookup (entity registry),
/// `EFBD0` particle spawn, `45390`/`45820` object spawn record init and spawn, `218A0`/`21860` sounds,
/// `D1EC0`/`D1F10` player nudge and screen flash, `1679D0` anim record lookup, `BD090` handle release,
/// `468A0` light ramp, `10E6E0/10E6F0/10EB50/10EBB0/10E730` tracked object setters/getter, `88C60` world sweep.
pub fn scene_per_frame_update<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let base = cpu.esp.wrapping_sub(FRAME_BYTES);
    let mut x = Ctx { c, m, cpu, base };
    let scene = update_mode_and_level_factor(&mut x);
    if x.read(global::ENABLE_FLAG) != 0 && matches!(x.read(scene + scene::MODE), 1 | 2) {
        ember_slots(&mut x, scene);
    }
    if x.read(scene + scene::MODE) == 1 {
        player_kick(&mut x);
    }
    release_stale_handles(&mut x);
    light_ramps(&mut x, scene);
    let object = x.read(scene + scene::OBJECT);
    if x.read(scene + scene::MODE) != 4 {
        x.call(target::OBJECT_FLAG_OFF, 0x001e_36fb, &[object]);
        return;
    }
    tracked_object(&mut x, scene);
}

/// Mode from `0x1E2AD0`, then `S.level_factor` approaches `table[mode] * 0.02` by 0.02 per frame. Returns `S`.
fn update_mode_and_level_factor<M: GuestMemory, C: Callee<M>>(x: &mut Ctx<'_, M, C>) -> u32 {
    let mode = x.call(target::ZONE_MODE, 0x001e_2b8b, &[]);
    let scene = x.read(global::SCENE_PTR);
    x.write(scene + scene::MODE, mode);
    let entry = x.read(global::MODE_TARGET_TABLE.wrapping_add(mode.wrapping_mul(4)));
    let step = x.f(konst::LEVEL_FACTOR_STEP);
    let target = (entry as i32 as f32) * step;
    x.call(
        target::APPROACH,
        0x001e_2bbf,
        &[scene + scene::LEVEL_FACTOR, target.to_bits(), 0x3ca3_d70a],
    );
    scene
}

/// Ember block (modes 1 and 2): count down to a new ember anchor, then update the five slots.
fn ember_slots<M: GuestMemory, C: Callee<M>>(x: &mut Ctx<'_, M, C>, scene: u32) {
    let counter = (x.read(global::SPAWN_COUNTER) as i32).wrapping_add(x.read(global::STEP) as i32);
    x.write(global::SPAWN_COUNTER, counter as u32);
    let threshold = i32::from(x.read(global::LEVEL) as i32 > 2);
    if counter > threshold && x.unit(0x001e_2c15) > f64::from(x.f(konst::SPAWN_CHANCE)) {
        spawn_ember_anchor(x, scene);
    }
    let mut spawned_this_frame = x.unit(0x001e_2e46) > f64::from(x.f(konst::HALF));
    for k in 0..scene::SLOT_COUNT {
        let slot = scene + k * scene::SLOT_STRIDE;
        let life = x.read(slot + slot::LIFE);
        if life == 0 {
            continue;
        }
        let timer = x.read(slot + slot::TIMER);
        if timer != 0 {
            x.write(slot + slot::TIMER, timer.wrapping_sub(1));
            continue;
        }
        x.write(slot + slot::TIMER, 10);
        x.write(slot + slot::LIFE, life.wrapping_sub(1));
        update_slot(x, scene, slot, &mut spawned_this_frame);
    }
}

/// Picks a random `ENLP` anchor and writes a new slot of the ring (`0x1E2C10..0x1E2E3B`).
fn spawn_ember_anchor<M: GuestMemory, C: Callee<M>>(x: &mut Ctx<'_, M, C>, scene: u32) {
    let offsets = x
        .read(global::SPAWN_INDEX)
        .wrapping_mul(12)
        .wrapping_add(global::SPAWN_OFFSET_TABLE);
    let node = x.call(target::FIND_BY_TAG, 0x001e_2c58, &[TAG_ENLP]);
    if node == 0 {
        return;
    }
    let inner = x.read(node + 4);
    if inner == 0 {
        return;
    }
    let matrix = x.read(inner + 0x10);
    if matrix == 0 {
        return;
    }
    x.write(global::SPAWN_COUNTER, 0);
    let vec = x.local(frame::VEC);
    x.set_vec3(vec, [0.0, 0.0, 1.0]);
    x.call(target::ROTATE_VECTOR, 0x001e_2caf, &[matrix, vec]);
    let v = x.vec3(vec);
    let index = x.read(scene + scene::NEXT_SLOT);
    let slot = scene.wrapping_add(index.wrapping_mul(scene::SLOT_STRIDE));
    let drift_scale = x.double(konst::SCALE_X_DOUBLE);
    let pos_scale = x.double(konst::SCALE_POS_DOUBLE);
    for (i, c) in v.iter().enumerate() {
        x.wf(
            slot + slot::DRIFT + 4 * i as u32,
            (f64::from(*c) * drift_scale) as f32,
        );
    }
    let node_pos = x.vec3(node + 0x64);
    let px = (f64::from(v[0]) * pos_scale + f64::from(node_pos[0])) as f32;
    let py = (f64::from(v[1]) * pos_scale + f64::from(node_pos[1])) as f32;
    let pz = ((f64::from(v[2]) * pos_scale) as f32 as f64 + f64::from(node_pos[2])) as f32;
    x.set_vec3(slot + slot::POSITION, [px, py, pz]);
    for i in 0..3 {
        let word = x.read(offsets + 4 * i);
        x.write(slot + slot::ANCHOR + 4 * i, word);
    }
    x.write(slot + slot::LIFE, 5);
    x.write(slot + slot::TIMER, 0);
    x.write(
        scene + scene::NEXT_SLOT,
        ((index as i32).wrapping_add(1) % 5) as u32,
    );
}

/// One live slot whose timer expired: jitter the position, emit the ember particle, drift, maybe spawn an object.
fn update_slot<M: GuestMemory, C: Callee<M>>(
    x: &mut Ctx<'_, M, C>,
    scene: u32,
    slot: u32,
    spawned: &mut bool,
) {
    let mut at = x.vec3(slot + slot::POSITION);
    let half = f64::from(x.f(konst::HALF));
    let ten = f64::from(x.f(konst::TEN));
    for (i, ret) in [0x001e_2ee1u32, 0x001e_2f12, 0x001e_2f43]
        .into_iter()
        .enumerate()
    {
        let r = x.unit(ret);
        at[i] = ((r - half) * ten + f64::from(at[i])) as f32;
    }
    let anchor = x.vec3(slot + slot::ANCHOR);
    let mut d = [at[0] - anchor[0], at[1] - anchor[1], at[2] - anchor[2]];
    let len2 = (d[2] * d[2] + d[1] * d[1]) + d[0] * d[0];
    if len2 > x.f(konst::TINY) {
        let inv = rsqrt(len2) * x.f(konst::ONE);
        d = [inv * d[0], inv * d[1], inv * d[2]];
    }
    let (pos_ptr, dir_ptr) = (x.local(frame::POS), x.local(frame::DIR));
    x.set_vec3(pos_ptr, at);
    x.set_vec3(dir_ptr, d);
    x.call(
        target::PARTICLE,
        0x001e_3047,
        &[0xf, 0xffff_ffff, pos_ptr, dir_ptr, 0, 0, 0, 1],
    );
    for i in 0..3 {
        let sum = x.f(slot + slot::DRIFT + 4 * i) + x.f(slot + slot::POSITION + 4 * i);
        x.wf(slot + slot::POSITION + 4 * i, sum);
    }
    if *spawned {
        return;
    }
    *spawned = true;
    let m =
        (f64::from(x.f(scene + scene::LEVEL_FACTOR)) * f64::from(x.f(konst::SPAWN_SCALE))) as f32;
    let q = (x.unit(0x001e_30c2) * f64::from(m) + f64::from(m)) as f32;
    let p = ((x.unit(0x001e_30eb) - half) * f64::from(x.f(konst::TENTH))
        + f64::from(x.f(konst::ONE))) as f32;
    let record = x.local(frame::RECORD);
    x.call(target::INIT_SPAWN_RECORD, 0x001e_3123, &[record]);
    x.write(record, EMBER_OBJECT_ID);
    x.wf(record + 0x14, p);
    x.wf(record + 0x18, q);
    x.call(target::SPAWN_OBJECT, 0x001e_3141, &[record]);
}

/// Mode 1 rare event: sounds, a nudge and the kick fields of the player record.
fn player_kick<M: GuestMemory, C: Callee<M>>(x: &mut Ctx<'_, M, C>) {
    if x.unit(0x001e_316a) <= f64::from(x.f(konst::KICK_CHANCE)) {
        return;
    }
    let player = x.read(global::PLAYER_PTR);
    if x.read(player + player::STATE) == player::IDLE_STATE {
        return;
    }
    let linked = x.read(player + player::OBJECT);
    if linked == 0 || !(x.f(linked + player::LINKED_VALUE) <= 0.0) {
        return;
    }
    let ext =
        x.unit(0x001e_31c9) * f64::from(x.f(konst::KICK_SCALE)) + f64::from(x.f(konst::KICK_BASE));
    let w = ext as f32;
    x.call(
        target::SOUND_1A6,
        0x001e_3205,
        &[
            MODE_FLAG_OBJECT_ID,
            ((ext * f64::from(x.f(konst::HALF))) as f32).to_bits(),
        ],
    );
    if x.unit(0x001e_320d) > f64::from(x.f(konst::KICK_SOUND_CHANCE)) {
        let count = cvtt(x.read(global::LEVEL) as i32 as f32);
        x.call(
            target::SOUND_1A7,
            0x001e_324f,
            &[0x1a7, count as u32, 0x3f00_0000],
        );
    }
    let a = w * x.f(konst::TENTH);
    let b = w * x.f(konst::NUDGE_B);
    x.call(
        target::PLAYER_NUDGE,
        0x001e_328a,
        &[player, 0x3f19_999a, a.to_bits(), b.to_bits()],
    );
    let player = x.read(global::PLAYER_PTR);
    let (fbc, fc0) = (w * x.f(konst::KICK_FBC), w * x.f(konst::KICK_FC0));
    x.wf(player + player::KICK_X, fbc);
    x.wf(player + player::KICK_Y, fc0);
}

/// Walk of the entity list: entities whose anim record float `+4` is below -50 release their four handles.
fn release_stale_handles<M: GuestMemory, C: Callee<M>>(x: &mut Ctx<'_, M, C>) {
    let mut entity = x.read(global::ENTITY_LIST_HEAD);
    while entity != 0 {
        let id = x.read(entity + 0x48);
        let record = x.call(target::ANIM_RECORD, 0x001e_32d9, &[id]);
        if x.f(konst::STALE_LIMIT) > x.f(record + 4) {
            for k in 0..4 {
                let handle_at = entity + 0x70 + k * 0x34;
                let handle = x.read(handle_at);
                if handle == 0 {
                    continue;
                }
                let entry = x
                    .read(global::ENTITY_TABLE)
                    .wrapping_add((handle >> 22).wrapping_mul(0x2e8));
                let argument = if x.read(entry + 0x2c) == handle {
                    entry
                } else {
                    x.write(handle_at, 0);
                    0
                };
                x.call(target::RELEASE_HANDLE, 0x001e_3322, &[argument]);
            }
        }
        entity = x.read(entity);
    }
}

/// The two light ramps `468A0(target, 0, select, value, 50.0, 1.0, 0)`.
fn light_ramps<M: GuestMemory, C: Callee<M>>(x: &mut Ctx<'_, M, C>, scene: u32) {
    let level = x.read(scene + scene::LEVEL_FACTOR);
    x.call(
        target::LIGHT_RAMP,
        0x001e_3358,
        &[
            scene + scene::LIGHT_A,
            0,
            3,
            level,
            0x4248_0000,
            0x3f80_0000,
            0,
        ],
    );
    let value = x.read(scene + scene::LIGHT_B_VALUE);
    x.call(
        target::LIGHT_RAMP,
        0x001e_337f,
        &[
            scene + scene::LIGHT_B,
            0,
            0x21,
            value,
            0x4248_0000,
            0x3f80_0000,
            0,
        ],
    );
}

/// Height getter `10EBB0` (float bits in `eax` by this port's convention) on the scene's tracked object.
fn object_height<M: GuestMemory, C: Callee<M>>(
    x: &mut Ctx<'_, M, C>,
    scene: u32,
    return_pc: u32,
) -> f32 {
    let object = x.read(scene + scene::OBJECT);
    f32::from_bits(x.call(target::OBJECT_GET_HEIGHT, return_pc, &[object]))
}

/// Mode 4 tail: show the tracked object, sweep the world along the player heading, set its vector and value.
/// Frame slots follow the original stack layout (`TARGET`, `REACH`, `UP`, `SWEEP_OUT`), so the post-sweep reads at
/// `REACH + 4` alias the locally written reach buffer exactly as in the lifted code.
fn tracked_object<M: GuestMemory, C: Callee<M>>(x: &mut Ctx<'_, M, C>, scene: u32) {
    let object = x.read(scene + scene::OBJECT);
    let base = x.local(0);
    let (target_buf, reach_buf, up_buf, out_buf) = (
        base + frame::TARGET,
        base + frame::REACH,
        base + frame::UP,
        base + frame::SWEEP_OUT,
    );
    x.set_vec3(up_buf, [0.0, x.f(konst::ONE), 0.0]);
    x.call(target::OBJECT_FLAG_ON, 0x001e_33c2, &[object]);
    let object = x.read(scene + scene::OBJECT);
    x.call(
        target::OBJECT_SET_VALUE,
        0x001e_33d8,
        &[object, 0x3f80_0000],
    );

    let player = x.read(global::PLAYER_PTR);
    let b = x.vec3(player + player::POSITION);
    let c = x.vec3(player + player::SCENE_ANCHOR);
    x.set_vec3(
        target_buf,
        [
            (b[0] + b[0]) - c[0],
            (b[1] + b[1]) - c[1],
            (b[2] + b[2]) - c[2],
        ],
    );
    let heading = x.vec3(player + player::HEADING);
    let len2 = heading[2] * heading[2] + heading[0] * heading[0];
    let mut dir = [heading[0], 0.0, heading[2]];
    if len2 > x.f(konst::TINY) {
        let inv = rsqrt(len2) * x.f(konst::ONE);
        dir = [inv * dir[0], inv * 0.0, inv * dir[2]];
    }
    let four = x.f(konst::FOUR);
    let reach = [dir[0] * four, dir[1] * four, dir[2] * four];
    x.set_vec3(reach_buf, reach);
    let anchor = x.vec3(target_buf);
    x.set_vec3(
        target_buf,
        [
            reach[0] + anchor[0],
            reach[1] + anchor[1],
            reach[2] + anchor[2],
        ],
    );
    let world_holder = x.read(player + player::WORLD_HOLDER);
    let world = x.read(world_holder + player::WORLD);
    x.call(
        target::SWEEP,
        0x001e_3572,
        &[
            world,
            player + player::POSITION,
            up_buf,
            1,
            target_buf,
            out_buf,
            0,
            0xffff_fffd,
            0,
            0,
        ],
    );
    let below = x.f(reach_buf + 4) - x.f(konst::TENTH);
    x.wf(target_buf + 4, below);
    let object = x.read(scene + scene::OBJECT);
    x.call(
        target::OBJECT_SET_VECTOR,
        0x001e_359d,
        &[object, target_buf],
    );

    let player = x.read(global::PLAYER_PTR);
    let dy = x.f(reach_buf + 4) - x.f(player + 0xb8);
    let clear_hit = x.f(out_buf + 4) + 1.0;
    let blocked =
        dy > x.f(konst::TEN) || x.f(konst::TENTH) > dy || clear_hit > x.f(konst::BLOCK_STEP);
    if blocked
        && f64::from(object_height(x, scene, 0x001e_35f3)) > f64::from(x.f(konst::HIGH_OBJECT))
    {
        let object = x.read(scene + scene::OBJECT);
        x.call(target::OBJECT_SET_VALUE, 0x001e_3618, &[object, 0]);
        return;
    }

    let limit =
        (1.0 / f64::from(x.read(global::LEVEL) as i32)) * f64::from(x.read(global::STEP) as i32);
    let height = f64::from(object_height(x, scene, 0x001e_3630));
    let object = x.read(scene + scene::OBJECT);
    if limit > height && x.f(object + 0x70) != 0.0 {
        let record = x.local(frame::RECORD);
        x.call(target::INIT_SPAWN_RECORD, 0x001e_3673, &[record]);
        x.write(record, MODE_FLAG_OBJECT_ID);
        let value = x.f(konst::SPAWN_VALUE);
        x.wf(record + 0x18, value);
        x.call(target::SPAWN_OBJECT, 0x001e_3693, &[record]);
    }
    let height = f64::from(object_height(x, scene, 0x001e_36a7));
    let object = x.read(scene + scene::OBJECT);
    if f64::from(x.f(konst::LOW_OBJECT)) > height && x.f(object + 0x70) != 0.0 {
        x.call(
            target::FLASH,
            0x001e_36e8,
            &[0x3dcc_cccd, 0x3dcc_cccd, 0x3dcc_cccd],
        );
    }
}
