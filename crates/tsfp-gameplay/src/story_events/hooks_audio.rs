//! Audio and ambience level hooks `[level+0x110]` (T1845, task A).
//!
//! Static reading of `build/default.xbe` (SHA-256 `3cfd001a...1816bc`), no xemu run. Control flow, constants and
//! jump tables are MEASURED; the game meaning of a state value is INFERRED unless stated. Full findings and the
//! reproduction method: `docs/t-story-event-hooks-audio.md`.
//!
//! Common callees (MEASURED shapes, names from `tools/data/function_names.csv`):
//!
//! * `0x1515E0(value*, target, step)`: [`approach`]. Returns true when it reached the target.
//! * `0x468A0(handle*, position*, sound id, volume, range, pitch, flags)`: the looped sound set. Volume above
//!   `0.001` (`[0x475DBC]`) starts the loop when the handle is negative, else updates its volume; at or below it the
//!   loop is stopped and the handle set to -1. Modelled as [`AudioEffect::SetLoop`].
//! * `0x45390` + `0x45820`: build a 48 byte sound request and submit it ([`AudioEffect::PlayOnce`]).
//! * The frame step: `dt = (1 / [0x4E8CEC]) * [0x4E794C]` with rate 60 (the float multiplier at `0x4E794C`; the
//!   integer at `0x4E7944` is only tested for zero).
//!
//! Hooks that are plain thunks (`jmp`) in the level table are listed with both addresses in [`audio_hook_kind`].

/// The rate `[0x4E8CEC]`.
pub const FRAME_RATE: f32 = 60.0;
/// Volume at or below which `0x468A0` stops a loop (`[0x475DBC]`).
pub const LOOP_STOP_VOLUME: f32 = 0.001;

/// Where a loop handle lives: `global` is an address in BSS; when `indirect` the dword there is a pointer to a
/// state block and the handle is the dword at `offset` inside it, else the handle is the dword at `global + offset`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandleRef {
    pub global: u32,
    pub indirect: bool,
    pub offset: u32,
}

/// One side effect on the sound system.
#[derive(Clone, Debug, PartialEq)]
pub enum AudioEffect {
    /// `0x468A0`.
    SetLoop {
        handle: HandleRef,
        sound_id: u32,
        volume: f32,
        position: Option<[f32; 3]>,
        range: f32,
        pitch: f32,
    },
    /// `0x45390` + `0x45820`: a one shot sound by id.
    PlayOnce { sound_id: u32 },
    /// `0x21C80(id, 1000.0, strength, &offset)`: a positional one shot (thunder), relative offset.
    PlayRelative {
        sound_id: u32,
        strength: f32,
        offset: [f32; 3],
    },
}

fn indirect(global: u32, offset: u32) -> HandleRef {
    HandleRef {
        global,
        indirect: true,
        offset,
    }
}

fn loop_effect(handle: HandleRef, sound_id: u32, volume: f32) -> AudioEffect {
    AudioEffect::SetLoop {
        handle,
        sound_id,
        volume,
        position: None,
        range: 50.0,
        pitch: 1.0,
    }
}

/// `0x1515E0`: move `*value` toward `target` by at most `step`; true when it arrived.
pub fn approach(value: &mut f32, target: f32, step: f32) -> bool {
    if step > (target - *value).abs() {
        *value = target;
        return true;
    }
    if target > *value {
        *value += step;
    } else {
        *value -= step;
    }
    false
}

/// The frame step in seconds, `(1 / 60) * multiplier`.
pub fn step_seconds(multiplier: f32) -> f32 {
    (1.0 / FRAME_RATE) * multiplier
}

/// Random source of the hooks: `0x153B40` is a 64 bit state LCG (`state * 0x343FD + 0x269EC3` at `0x4E7950`/`0x4E7954`,
/// low dword returned, MEASURED in `sweep_dynamic_shape::rng`), `0x12380` an inclusive range. `0x153BD0` is NOT a float of
/// this generator: it is a separate 32 bit LCG at `0x7497CC` (`svc_slot_teardown::Machine::rng_float`, `(state >> 16 & 0x7FFF)
/// * 3.0518509e-5`, T1856/T1864). The `next_u32` of this trait models `0x153B40` only.
pub trait AudioRng {
    fn next_u32(&mut self) -> u32;
    /// `0x12380(low, high, excluded)`: inclusive range, the previous value excluded.
    fn range_excluding(&mut self, low: u32, high: u32, excluded: u32) -> u32;
    /// The unsigned output scaled by 2^-32 (the fild / add 2^32 / multiply idiom of the hooks).
    fn unit(&mut self) -> f32 {
        (self.next_u32() as f64 * 2.328_306_436_538_696_3e-10) as f32
    }
}

/// What a hook reads of one player slot (array `[0x7B0C48]`, stride `0x1584`, active when the dword at `+4` is
/// not -1; the count is `[0x790950]`). Field addresses are relative to the objects named.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerAudio {
    pub active: bool,
    /// `[ [slot+0x14] + 0x34 ]`.
    pub object_state: u32,
    /// `[ [ [slot+0x14] + 0x7C ] ]` is the actor; its `+0x34`.
    pub actor_state: u32,
    /// `0x9D4A0(0x9A420([ext+0x678]))`: the character class index (INFERRED name, MEASURED chain).
    pub class_index: u32,
    /// `0x9A4C0`: the skin or outfit id (INFERRED name).
    pub skin_id: u32,
    /// `[ [slot+8] + 0x2A0 ]`.
    pub class_2a0: i32,
    /// Actor position (`+0x64`, `+0x68`, `+0x6C`).
    pub position: [f32; 3],
    /// `0x59D20(actor, 0xFF)` non-zero.
    pub flag_ff: bool,
}

fn active(players: &[PlayerAudio]) -> impl Iterator<Item = &PlayerAudio> {
    players.iter().filter(|player| player.active)
}

// ---- 0x222C90: levels 0x27, 0x3C, 0x4A (both structs) ---------------------------------------------------------

/// `0x222C90` intensity of `object_state - 0x28` (jump table `0x222D6C`, byte map `0x222D80`): 0 and 4 give 0.2,
/// 5 and 7 give 0.4, 6 and 8 give 0.7, 9 to 12 give 1.0, the rest 0.
pub fn max_mix_intensity(object_state: u32) -> f32 {
    match object_state.wrapping_sub(0x28) {
        0 | 4 => 0.2,
        5 | 7 => 0.4,
        6 | 8 => 0.7,
        9..=12 => 1.0,
        _ => 0.0,
    }
}

/// `0x222C90` (the static float at `0x75C11C`, handle `0x50B6B0`, sound `0x3B`): the volume approaches the MAXIMUM
/// intensity of the active players at `dt * 0.4444`, then the loop is set at the volume with range 50.
pub fn max_mix_hook(
    players: &[PlayerAudio],
    volume: &mut f32,
    multiplier: f32,
) -> Vec<AudioEffect> {
    let target = active(players)
        .map(|player| max_mix_intensity(player.object_state))
        .fold(0.0_f32, f32::max);
    approach(volume, target, step_seconds(multiplier) * 0.444_444_45);
    vec![loop_effect(
        HandleRef {
            global: 0x50B6B0,
            indirect: false,
            offset: 0,
        },
        0x3B,
        *volume,
    )]
}

// ---- 0x225840: levels 0x31, 0x47 -------------------------------------------------------------------------------

/// `0x225840` intensity of `actor_state - 6` (jump table `0x2259AC`, byte map `0x2259BC`): 0 and 12 give 0.95,
/// 6 and 13 give 0.65, 7 to 10 give 0, everything else (1 to 5, 11, above 13) gives 1.0.
pub fn mean_mix_intensity(actor_state: u32) -> f32 {
    match actor_state.wrapping_sub(6) {
        0 | 12 => 0.95,
        6 | 13 => 0.65,
        7..=10 => 0.0,
        _ => 1.0,
    }
}

/// `0x225840` (state float pointer `[0x75C124]`: `+0` value, `+4` and `+8` handles): the MEAN intensity of the
/// active players is the target; the value moves by `max(|value - mean| * 0.05, 0.25 / 60)` (no frame multiplier)
/// and snaps when that is not smaller than the gap. Loop `0x1B` at the value on handle `+8`, loop `0x19` at
/// `1 - value` on handle `+4`.
pub fn mean_crossfade_hook(players: &[PlayerAudio], value: &mut f32) -> Vec<AudioEffect> {
    let count = active(players).count();
    let sum: f32 = active(players)
        .map(|player| mean_mix_intensity(player.actor_state))
        .sum();
    let mean = if count == 0 {
        f32::NAN
    } else {
        sum / count as f32
    };
    let rate = ((*value - mean).abs() * 0.05).max((1.0 / FRAME_RATE) * 0.25);
    if rate > (mean - *value).abs() {
        *value = mean;
    } else if *value > mean {
        *value -= rate;
    } else {
        *value += rate;
    }
    let state = 0x75C124;
    vec![
        loop_effect(indirect(state, 8), 0x1B, *value),
        loop_effect(indirect(state, 4), 0x19, 1.0 - *value),
    ]
}

// ---- 0x220F90: levels 0x35, 0x44, 0x4D; 0x207450 (levels 0xF other, 0x48); 0x208360 (level 0x1D) ----------------

/// `0x220E60` (register argument = character, `k = class_index - 5`, jump table `0x220F1C`): true for k = 8 to 11
/// and 24; k = 0 skin in `0x446..=0x44D`; k = 6 skin outside `0x432..=0x433`; k = 13 skin in `0x578..=0x57E`;
/// k = 14 skin in `0x508..=0x50E`; k = 21 skin in `0x567..=0x56A`; k = 25 skin `0x466`; else false.
pub fn outfit_predicate_220e60(player: &PlayerAudio) -> bool {
    let skin = player.skin_id;
    match player.class_index.wrapping_sub(5) {
        0 => (0x446..=0x44D).contains(&skin),
        6 => !(0x432..=0x433).contains(&skin),
        8..=11 | 24 => true,
        13 => (0x578..=0x57E).contains(&skin),
        14 => (0x508..=0x50E).contains(&skin),
        21 => (0x567..=0x56A).contains(&skin),
        25 => skin == 0x466,
        _ => false,
    }
}

/// `0x207F70` (`k = class_index - 8`, jump table `0x207FF0`): true for k = 0, 2, 12, 16, 17, 18, 30, 31, 38; for
/// k = 7 when the skin is `0x567`, k = 9 `0x549`, k = 25 `0x4AD`, k = 29 `0x4EB`, else false (k above 38 is out of
/// the `ja 0x26` bound).
pub fn outfit_predicate_207f70(player: &PlayerAudio) -> bool {
    let skin = player.skin_id;
    match player.class_index.wrapping_sub(8) {
        0 | 2 | 12 | 16 | 17 | 18 | 30 | 31 | 38 => true,
        7 => skin == 0x567,
        9 => skin == 0x549,
        25 => skin == 0x4AD,
        29 => skin == 0x4EB,
        _ => false,
    }
}

/// `0x207440`: the object state is not `0x29`.
pub fn state_predicate_207440(player: &PlayerAudio) -> bool {
    player.object_state != 0x29
}

fn all_pass(players: &[PlayerAudio], predicate: fn(&PlayerAudio) -> bool) -> bool {
    active(players).all(predicate)
}

/// Ramp direction shared by the three crossfade hooks: `+dt * rate` while every active player passes the predicate,
/// else `-dt * rate`, then clamped.
fn ramp_direction(
    players: &[PlayerAudio],
    predicate: fn(&PlayerAudio) -> bool,
    multiplier: f32,
    rate: f32,
) -> f32 {
    let step = step_seconds(multiplier) * rate;
    if all_pass(players, predicate) {
        step
    } else {
        -step
    }
}

/// `0x220F90` (levels 0x35, 0x44, 0x4D; allocated by `0x2210B0` as a 0x40 byte arena block at `[0x75C108]`): the
/// value (`+0`) moves by `0.8 * dt` toward 0.75 while every active player passes `0x220E60`, else toward 0
/// (clamped to `[0, 0.75]`); loop `0x18` on handle `+4` at `1 - value`.
pub fn outfit_ambience_hook(
    players: &[PlayerAudio],
    value: &mut f32,
    multiplier: f32,
) -> Vec<AudioEffect> {
    let delta = ramp_direction(players, outfit_predicate_220e60, multiplier, 0.8);
    *value = (*value + delta).clamp(0.0, 0.75);
    vec![loop_effect(indirect(0x75C108, 4), 0x18, 1.0 - *value)]
}

/// `0x207450(predicate)` (level 0xF other struct and 0x48, predicate `0x207440`; state `[0x75C044]`, value at `+8`,
/// handles `+0` and `+4`): ramp rate 0.8 clamped to `[0, 1]`; loop `0x1F` at the value on handle `+0`, loop `0x1E`
/// at `1 - value` on handle `+4`.
pub fn state_crossfade_hook(
    players: &[PlayerAudio],
    value: &mut f32,
    multiplier: f32,
) -> Vec<AudioEffect> {
    let delta = ramp_direction(players, state_predicate_207440, multiplier, 0.8);
    *value = (*value + delta).clamp(0.0, 1.0);
    vec![
        loop_effect(indirect(0x75C044, 0), 0x1F, *value),
        loop_effect(indirect(0x75C044, 4), 0x1E, 1.0 - *value),
    ]
}

/// `0x208360` (level 0x1D, the first call of its hook `0x2084A0`; state `[0x75C080]`, value `+4`, handles `+8` and
/// `+0xC`): rate `4/3`, clamped to `[0, 1]`, predicate `0x207F70`; loop `0x17` at the value, loop `0x16` at `1 - value`.
pub fn level_1d_crossfade_hook(
    players: &[PlayerAudio],
    value: &mut f32,
    multiplier: f32,
) -> Vec<AudioEffect> {
    let delta = ramp_direction(players, outfit_predicate_207f70, multiplier, 1.333_333_4);
    *value = (*value + delta).clamp(0.0, 1.0);
    vec![
        loop_effect(indirect(0x75C080, 8), 0x17, *value),
        loop_effect(indirect(0x75C080, 0xC), 0x16, 1.0 - *value),
    ]
}

// ---- 0x2264F0: level 0x34 (other struct) -----------------------------------------------------------------------

/// Focus points of the ellipse weight in `0x226350` (constants `0x49EE70..0x49EE7C`, `0x47D798`; INFERRED shape).
pub const WEIGHT_FOCI: [[f32; 3]; 2] = [[16.72, -17.0, 2.4], [24.97, -17.0, 3.76]];

/// `0x226350`: the audio weight of one player. 1.0 when `flag_ff`; else by `k = class_index - 2` (jump table
/// `0x2264B0`): k = 0 and 24 the height ramp `clamp((y - 9) / 8, 0, 1)`, k = 5, 6, 7 one minus the clamped
/// `(d1 + d2 - 20) / 30` of the two focus distances, k = 29 and 32 to 37 give 1.0, anything else 0.
pub fn player_weight_226350(player: &PlayerAudio) -> f32 {
    if player.flag_ff {
        return 1.0;
    }
    match player.class_index.wrapping_sub(2) {
        0 | 24 => ((player.position[1] - 9.0) * 0.125).clamp(0.0, 1.0),
        5..=7 => {
            let distance = |focus: &[f32; 3]| {
                let dx = focus[0] - player.position[0];
                let dy = focus[1] - player.position[1];
                let dz = focus[2] - player.position[2];
                (dx * dx + dy * dy + dz * dz).sqrt()
            };
            let sum = distance(&WEIGHT_FOCI[0]) + distance(&WEIGHT_FOCI[1]);
            1.0 - ((sum - 20.0) * 0.033_333_335).clamp(0.0, 1.0)
        }
        29 | 32..=37 => 1.0,
        _ => 0.0,
    }
}

/// `0x2264F0` (state `[0x75C130]`: `+0` value, handles `+4` and `+8`): the mean weight of the active players is the
/// target, step `dt * 3`; loop `0x25` at `0.99 - value`, loop `0x24` at `value - 0.01`.
pub fn weight_crossfade_hook(
    players: &[PlayerAudio],
    value: &mut f32,
    multiplier: f32,
) -> Vec<AudioEffect> {
    let count = active(players).count();
    let sum: f32 = active(players).map(player_weight_226350).sum();
    let mean = if count == 0 {
        f32::NAN
    } else {
        sum / count as f32
    };
    approach(value, mean, step_seconds(multiplier) * 3.0);
    vec![
        loop_effect(indirect(0x75C130, 4), 0x25, 0.99 - *value),
        loop_effect(indirect(0x75C130, 8), 0x24, *value - 0.01),
    ]
}

// ---- 0x46F40: level 0x3B (other struct) via 0x225C50 -----------------------------------------------------------

/// `0x46F40(sound, state, list, gain)` weight of one player for the id list `0x50D078` (entries are character
/// classes; -0x64 ends the list and gives `gain`, -0x65 gives 0.5, a class equal to `[ [slot+8] + 0x2A0 ]` gives 1.0).
/// A null list gives the target 0.
pub fn class_list_weight(list: &[i32], class: i32, gain: f32) -> f32 {
    let mut index = 0;
    let first = list.first().copied().unwrap_or(-0x64);
    if first == -0x64 {
        return gain;
    }
    loop {
        let entry = list.get(index).copied().unwrap_or(-0x64);
        if entry == -0x65 {
            return 0.5;
        }
        if entry == class {
            return 1.0;
        }
        index += 1;
        if list.get(index).copied().unwrap_or(-0x64) == -0x64 {
            return gain;
        }
    }
}

/// The list at `0x50D078` as read from the image (sentinel -0x65 first, so every player weighs 0.5).
pub const LEVEL_3B_LIST: [i32; 11] = [-0x65, 2, 3, 4, 17, 16, 12, 13, 14, 15, -0x64];

/// `0x46F40` as called by the level 0x3B hook `0x225C50` (sound `0x37`, state `0x75C128` with `+0` value and the
/// handle at `+4`, list `0x50D078`, gain 1.0): the value approaches the mean weight with the fixed step `0.025`
/// (`0x3CCCCCCD`, no frame multiplier).
pub fn class_list_hook(
    players: &[PlayerAudio],
    list: &[i32],
    gain: f32,
    value: &mut f32,
) -> Vec<AudioEffect> {
    let count = active(players).count();
    let mean = if count == 0 {
        f32::NAN
    } else {
        active(players)
            .map(|player| class_list_weight(list, player.class_2a0, gain))
            .sum::<f32>()
            / count as f32
    };
    approach(value, mean, 0.025);
    vec![loop_effect(
        HandleRef {
            global: 0x75C128,
            indirect: false,
            offset: 4,
        },
        0x37,
        *value,
    )]
}

// ---- 0x222B60: levels 0x2E, 0x3F ---------------------------------------------------------------------------------

/// `0x222B60` (state `[0x75C118]`, timer at `+4`): the timer falls by `dt`; at or below 0 it gains 900.0 and the
/// one shot sound `0x156F` is requested. INFERRED meaning: a periodic announcer sound every 900 timer seconds.
pub fn periodic_sound_hook(timer: &mut f32, multiplier: f32) -> Vec<AudioEffect> {
    *timer -= step_seconds(multiplier);
    if *timer <= 0.0 {
        *timer += 900.0;
        return vec![AudioEffect::PlayOnce { sound_id: 0x156F }];
    }
    Vec::new()
}

// ---- 0x21AF90: levels 0x14 (other), 0x4F ---------------------------------------------------------------------------

/// State `[0x75C0F0]`: handle `+0`, last pick `+4`, timer `+8`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RandomOneShot {
    pub handle: i32,
    pub last_pick: u32,
    pub timer: f32,
}

/// Text id table lookup `0x461C0` for the pick: not decoded here, the caller supplies the sound id.
pub const ONE_SHOT_PICK_RANGE: (u32, u32) = (0x1B11, 0x1B1C);

/// `0x21AF90` (zero frame multiplier returns at once). While a one shot plays (`playing`, from `0x45540`) nothing
/// else happens; when it ends the handle resets to -1. With no handle the timer falls by `dt`; at or below 0 a new
/// pick in `0x1B11..=0x1B1C` (not the previous) is requested via `sound_of_pick` and the timer is set to
/// `rand01 * 40 + 20`. Returns the pick when one started.
pub fn random_one_shot_hook(
    state: &mut RandomOneShot,
    playing: impl Fn(i32) -> bool,
    multiplier: f32,
    rng: &mut dyn AudioRng,
    start: &mut dyn FnMut(u32) -> i32,
) -> Option<u32> {
    if multiplier == 0.0 {
        return None;
    }
    if state.handle >= 0 {
        if !playing(state.handle) {
            state.handle = -1;
        }
        return None;
    }
    state.timer -= step_seconds(multiplier);
    if state.timer > 0.0 {
        return None;
    }
    let pick = rng.range_excluding(
        ONE_SHOT_PICK_RANGE.0,
        ONE_SHOT_PICK_RANGE.1,
        state.last_pick,
    );
    state.last_pick = pick;
    state.handle = start(pick);
    state.timer = rng.unit() * 40.0 + 20.0;
    Some(pick)
}

// ---- 0x1D7920 / 0x1D7A70: level 0x3D; also level 1 via 0x1DDC90 -----------------------------------------------------

/// `0x1D7A70` per player intensity from `class_2a0 - 0x15` (byte map `0x1D7B64`): 0, 1, 3, 23 give 1.0; 24 gives 0.5.
pub fn storm_intensity(class_2a0: i32) -> f32 {
    match class_2a0.wrapping_sub(0x15) {
        0 | 1 | 3 | 23 => 1.0,
        24 => 0.5,
        _ => 0.0,
    }
}

/// `0x1D7920(intensity)` random thunder. State `[0x75B0C4]`: timer at `+0`. The timer falls by `dt`; when it is
/// positive nothing happens. Else `v = (0.9 - intensity) * (1 + r1)`; when `v > 0` one positional sound
/// `0x163 + trunc(r3 * 23 + 0.5)` (via `0x21C80`, distance 1000.0) is played at the offset
/// `(sin(2 pi r2) * 250, 0, cos(2 pi r2) * 250)` with strength `v`; the timer is then set to `r4 * 40 + 10`.
pub fn thunder_hook(
    timer: &mut f32,
    intensity: f32,
    multiplier: f32,
    rng: &mut dyn AudioRng,
) -> Vec<AudioEffect> {
    *timer -= step_seconds(multiplier);
    if *timer > 0.0 {
        return Vec::new();
    }
    let mut effects = Vec::new();
    let strength = (0.9 - intensity) * (rng.unit() + 1.0);
    if strength > 0.0 {
        let angle = rng.unit() * std::f32::consts::TAU;
        let offset = [angle.sin() * 250.0, 0.0, angle.cos() * 250.0];
        let pick = (rng.unit() * 23.0 + 0.5) as u32;
        effects.push(AudioEffect::PlayRelative {
            sound_id: 0x163 + pick,
            strength,
            offset,
        });
    }
    *timer = rng.unit() * 40.0 + 10.0;
    effects
}

/// Level 0x3D hook `0x1D7B80` is `0x1D7920(0)`.
pub fn level_3d_hook(timer: &mut f32, multiplier: f32, rng: &mut dyn AudioRng) -> Vec<AudioEffect> {
    thunder_hook(timer, 0.0, multiplier, rng)
}

/// `0x1D7A70` (called by level 1's hook `0x1DDC90`; state `[0x75B0C4]`, value `+0x44`, handle `+0x40`): the mean
/// [`storm_intensity`] over the active players is approached at `dt * 0.03`, loop `0x139` is set at `value * 2`
/// and the thunder timer runs with the value.
pub fn storm_hook(
    players: &[PlayerAudio],
    value: &mut f32,
    timer: &mut f32,
    multiplier: f32,
    rng: &mut dyn AudioRng,
) -> Vec<AudioEffect> {
    let count = active(players).count();
    let sum: f32 = active(players)
        .map(|player| storm_intensity(player.class_2a0))
        .sum();
    let mean = if count == 0 {
        f32::NAN
    } else {
        sum / count as f32
    };
    approach(value, mean, multiplier * 0.03);
    let mut effects = vec![loop_effect(indirect(0x75B0C4, 0x40), 0x139, *value * 2.0)];
    effects.extend(thunder_hook(timer, *value, multiplier, rng));
    effects
}

// ---- 0x220AF0: level 0x30 ----------------------------------------------------------------------------------------

/// Centre of the random strike disc of `0x220AF0` and its radius (constants `0x49EC60`, `0x4788B8`, `0x47D584`,
/// `0x47D1F8`).
pub const STRIKE_CENTRE: [f32; 3] = [36.78, 40.0, -30.0];
pub const STRIKE_RADIUS: f32 = 70.0;

/// `0x220AF0` (level 0x30): once per `rand % 60 < multiplier` frame (the roll is `rng_u32 % 60` against the integer
/// multiplier `[0x4E7944]`) a random point is chosen in a disc of radius 70 around [`STRIKE_CENTRE`] using the sine
/// table at `0x7A38A0` (2048 entries), a ray cast (`0xC3C50`, `0x88C60`) finds ground and, when it hits more than
/// 400 units from the first player's camera, a ground effect (`0xEFBD0`, type 0xC, or 0xE for a flag) and the
/// positional sound `0x1571` (`0x21BB0`, 30.0) are started. The pick and the roll are modelled, the world side is
/// [`StrikeRequest`]. INFERRED meaning: random lightning strikes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrikeRequest {
    pub point: [f32; 3],
}

pub fn lightning_roll(roll_source: u32, multiplier: u32) -> bool {
    multiplier != 0 && roll_source % 60 < multiplier
}

/// The strike point from two units in `[0, 1)`: angle `2 pi u1`, radius `70 u2`, offsets by the sine table
/// (`sin(angle)` on x, `cos(angle)` on z), around the centre. The exact table index (`trunc(angle * 325.949 + 0.5)`
/// and 0x200 quarter turn) is the table form of the same sine and cosine.
pub fn lightning_point(unit_angle: f32, unit_radius: f32) -> StrikeRequest {
    let angle = unit_angle * std::f32::consts::TAU;
    let radius = unit_radius * STRIKE_RADIUS;
    StrikeRequest {
        point: [
            STRIKE_CENTRE[0] + angle.cos() * radius,
            STRIKE_CENTRE[1],
            STRIKE_CENTRE[2] + angle.sin() * radius,
        ],
    }
}

// ---- 0x20FBD0: level 0x16 ------------------------------------------------------------------------------------------

/// The failure timer of `0x20FBD0` (state: slot `[0x75C094]`, start clock `[0x75C098]`, seconds `[0x75C09C]`,
/// clock `[0x7DE318]`): when the slot is set and `clock - start > 60 * seconds` the slot gets status 4 (`0x2276E0`),
/// game flag `0x10000` is set and the match ends (`0x67450(0)`).
pub fn level_16_timeout(slot: i32, start_clock: i32, seconds: f32, clock: i32) -> bool {
    slot > -1 && ((clock.wrapping_sub(start_clock)) as f32) > FRAME_RATE * seconds
}

/// Fixed loop of `0x20FBD0`: sound `0x62F`, volume 1.3, range 30.0, handle at `[0x75C0A0] + 0x460`, position
/// `(54.45, 7.77, -81.74)`.
pub fn level_16_base_loop() -> AudioEffect {
    AudioEffect::SetLoop {
        handle: indirect(0x75C0A0, 0x460),
        sound_id: 0x62F,
        volume: 1.3,
        position: Some([54.45, 7.77, -81.74]),
        range: 30.0,
        pitch: 1.0,
    }
}

// ---- hook identification -------------------------------------------------------------------------------------------

/// The audio hooks of the level table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioHookKind {
    MaxMix,
    MeanCrossfade,
    OutfitAmbience,
    StateCrossfade,
    Level1dCrossfade,
    WeightCrossfade,
    ClassListLoop,
    PeriodicSound,
    RandomOneShot,
    Thunder,
    Lightning,
    Level16,
    /// A shared no-op (`0x389B0`).
    Nothing,
}

/// Map a hook address (table value or thunk target) to its kind.
pub fn audio_hook_kind(address: u32) -> Option<AudioHookKind> {
    use AudioHookKind::*;
    Some(match address {
        0x222D90 | 0x222C90 => MaxMix,
        0x225A10 | 0x225840 => MeanCrossfade,
        0x2210A0 | 0x220F90 => OutfitAmbience,
        0x207580 | 0x207450 => StateCrossfade,
        0x2084A0 => Level1dCrossfade,
        0x226600 | 0x2264F0 => WeightCrossfade,
        0x225C50 => ClassListLoop,
        0x222B60 => PeriodicSound,
        0x21AF90 => RandomOneShot,
        0x1D7B80 | 0x1D7920 => Thunder,
        0x220DC0 | 0x220AF0 => Lightning,
        0x20FBD0 => Level16,
        0x389B0 => Nothing,
        _ => return None,
    })
}
