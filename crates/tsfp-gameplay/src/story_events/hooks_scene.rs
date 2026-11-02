//! Level 1 scene drivers called by the level 1 hook `0x1DDC90` (T1845 round 3, task F).
//!
//! Static reading of `build/default.xbe` (SHA-256 `3cfd001a...1816bc`), no xemu run. Control flow, offsets and
//! constants are MEASURED; the game meaning is INFERRED. Details and limits: `docs/t-story-event-hooks-scene.md`.
//!
//! * [`blast_frame`] is `0x36B7A0`, the scripted blast gate and refill. Its callees are [`blast_ambience`] (`0x3697C0`,
//!   fully ported), [`blast_update`] (`0x368A10`, ported; the four room face geometry callees are the
//!   [`BlastGeometry`] trait) with [`blast_split`] (`0x366200`), and [`blast_pieces`] (`0x369940`, ported except the
//!   motion of pieces with status 0 or 2). The caller runs them in the order ambience, frame, update, pieces.
//! * [`beam_frame`] is `0x1DD510`, the 5 particle beam (state 1 charge, state 2 fade) with the beam damage test.
//!   Fully modelled.
//! * [`worm_frame`] is `0x1DAAB0`, the per frame update of the five "WRM" worm creatures, fully modelled since
//!   round 4 (states 9, 0xA, 0xE: [`breath_step`], [`turning_breath_step`], [`recover_breath_step`]).

use std::f32::consts::{FRAC_PI_2, PI};

/// A 3 float vector.
pub type Vec3 = [f32; 3];

/// Frame step data every driver reads: `[0x4E7944]` (frame multiplier) and `[0x4E8CEC]` (rate, 60).
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub steps: i32,
    pub rate: i32,
}

impl Timing {
    /// `(1 / rate) * steps` (the order of the image's multiplications).
    pub fn dt(&self) -> f32 {
        (1.0 / self.rate as f32) * self.steps as f32
    }
}

/// An effect asked of the world. Order of emission follows the image.
#[derive(Clone, Debug, PartialEq)]
pub enum SceneEffect {
    /// `0x2274C0(slot, arg)`: finish the static slot.
    SlotFinish { slot: u32, arg: u32 },
    /// `0x45460(handle)`: stop a sound.
    StopSound { handle: i32 },
    /// `0x111AE0(0, template, flags, anchor, vec)`: spawn a particle emitter.
    Particle {
        template: u32,
        anchor: ParticleAnchor,
        vector: Vec3,
    },
    /// `0x45820` one shot sound request at a position (`byte +0x28 = 1`).
    Sound { id: u32, position: Vec3 },
    /// `0x468A0(handle, pos, id, a, b, c, flag)` looped sound update.
    LoopSound {
        handle: LoopHandle,
        position: Option<Vec3>,
        id: u32,
        volume: f32,
        range: f32,
        pitch: f32,
        flags: u32,
    },
    /// `0x1D7130(point)`: ground effect at a point.
    GroundEffect { point: Vec3 },
    /// `0x10E2A0(handle)`: free a particle handle if non-null.
    FreeParticle { handle: u32 },
    /// `0x10EB90(handle, value)` store float at particle `+0x74`.
    ParticleField74 { handle: u32, value: f32 },
    /// `0x10EB10(handle, a, b)` store two floats at particle `+0x68`, `+0x6C`.
    ParticleFields68_6C { handle: u32, a: f32, b: f32 },
    /// `0xBB6C0` damage record: attacker object, victim, amount, kind, hit point and direction.
    Damage {
        victim: DamageVictim,
        amount: f32,
        kind: u32,
        point: Vec3,
        direction: Vec3,
    },
    /// `0x1252C0(player block + 0xC68, 1.0, 0)` effect on a player-controlled victim.
    PlayerEffect { target: u32 },
    /// `0xD1EC0(player, a, b, duration)` camera shake and `+0xFBC/+0xFC0` kick.
    CameraShake {
        player: usize,
        intensity: f32,
        secondary: f32,
        duration: f32,
        kick: f32,
    },
    /// `0x5F460(object, anim, 0, 1.0)` start an animation.
    SetAnimation { entity: usize, animation: u32 },
    /// `0x15DDA0(object)` destroy the creature object.
    DestroyObject { entity: usize },
    /// `0x1D9AF0` creates the creature object of an empty slot.
    CreateObject { entity: usize },
    /// Set (`true`) or clear the object flag `0x40` of word `+0x28`.
    ObjectFlag40 { entity: usize, set: bool },
    /// `0x17570(object + 0x7C, 0, 0x2000000)` per frame call on a live creature object.
    ObjectControl { entity: usize },
    /// `0x11FE80(player block + 0xC68, 10.0)` effect arm after the player was swallowed.
    ArmPlayerEffect { amount: f32 },
    /// `0x234CD0(key)` arms the event instance of the key when `armed` (the hit object's word `+0x150` is non-zero).
    ArmEvent { key: u32, armed: bool },
    /// Room object `id` (table `0x7B9784`): flag `+0x28 |= 0x10` and callback field `+0x19C` set (tail of `0x369940`).
    MarkRoomObject { id: u32, callback: u32 },
    /// A callee that is not decoded: address range and reason.
    Unsupported { callee: u32, reason: &'static str },
    /// `0x10E730(object, &position)`: copies the vector to the object's `+0x14..+0x20` (T1846 batch 8, MEASURED).
    ObjectVector { object: u32, position: Vec3 },
    /// `0x1128C0([0x4D2D64 + 24 * kind], 1, flags, 0, scale, 1, room, piece, &offset)`: a decal on piece `piece`
    /// (`0x80F20(kind)` is the table lookup). T1846 batch 8, MEASURED arguments.
    Decal {
        kind: u32,
        flags: u32,
        scale: f32,
        room: i32,
        piece: usize,
        offset: Vec3,
    },
}

/// Where a particle spawn is anchored.
#[derive(Clone, Debug, PartialEq)]
pub enum ParticleAnchor {
    /// The emitter template is positioned on piece `index`.
    Piece(usize),
    /// On creature `index` (`entity + 0`).
    Creature(usize),
    /// A fixed world point.
    Point(Vec3),
}

/// The looped sound handle that `0x468A0` updates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopHandle {
    /// `[0x76C238] + 0x362C` (blast ambience, set by `0x3697C0`).
    BlastAmbience,
    /// The creature's object `+0x22C`.
    CreatureObject(usize),
    /// `[0x75B0C4] + 4` (beam hum).
    BeamHum,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DamageVictim {
    Object(u32),
    /// A creature (`entity` of the worm array) is the attacker, the victim object was found by the ray.
    ByRay,
    /// Beam target by index into the object list `[0x7B5F50]`.
    ListObject(usize),
}

// ------------------------------------------------------------------ 0x36B7A0 blast

/// Maximum number of blast pieces (`0x320`).
pub const BLAST_MAX_PIECES: usize = 0x320;
/// Pieces handed out per frame while the pool refills.
pub const BLAST_REFILL_PER_FRAME: usize = 4;
/// Pieces removed per frame while the frame step is too long.
pub const BLAST_TRIM_PER_FRAME: i32 = 8;
/// The pool never trims below this count.
pub const BLAST_MIN_PIECES: i32 = 0x4B;
/// A refill/trim of the pool, and the `dead > 720.0` spawn threshold (`[0x47E2D8]`).
pub const BLAST_SPAWN_THRESHOLD: f32 = 720.0;
/// Per player trail ring: 0x85 entries of 3 floats (`0x63C` bytes) at `S + 0x50 + 0x63C * player`.
pub const TRAIL_ENTRIES: usize = 0x85;

#[derive(Clone, Copy, Debug, Default)]
pub struct BlastPiece {
    pub position: Vec3,
    /// Word `+0x2C`: 0 = flying, 3 = dead (`0x36B7A0` sets 3 when it spawns the emitter).
    pub state: u32,
    /// `+0x44`: the player whose trail the piece replays.
    pub player: usize,
    /// `+0x48`: frames behind the trail head.
    pub frames_back: i32,
    /// `+0x0C`, `+0x10`: heading words (written by the face callees, T1846 batch 8). `+0x14`: spin angle,
    /// `+0x18..+0x20`: velocity (T1845 round 4).
    pub heading_a: f32,
    pub heading_b: f32,
    pub spin: f32,
    pub velocity: Vec3,
    /// `+0x24`: attached to a face; `+0x28`: room id.
    pub attached: i32,
    pub room: i32,
    /// `+0x34`: frames until the piece is launched (counts down while a player is near, T1846 batch 8).
    pub timer: i32,
    /// `+0x38`: seconds left in status 2.
    pub countdown: f32,
    /// `+0x40`: the engine object that follows the piece in status 2 (`0x10E730`).
    pub object: u32,
    /// `+0x3C`: the face record (copy of the fields read).
    pub face: BlobFace,
}

/// The persistent state of the blast struct `[0x76C238]`.
#[derive(Clone, Debug)]
pub struct BlastState {
    /// Dword `+0x3364`.
    pub active: bool,
    /// Dword `+4`.
    pub piece_count: i32,
    pub pieces: Vec<BlastPiece>,
    /// `0x76C180`: 30 entry ring of past frame steps, write position `[0x76C240]`.
    pub history: [i32; 30],
    pub history_position: usize,
    /// Dword `+0x360C`: frames left since the "DOOF" object was last seen (3 seconds), and `+0x3610..+0x3618` its position.
    pub doof_countdown: i32,
    pub doof_position: Vec3,
    /// Dword `+0x362C`, -1 = none.
    pub ambience_handle: i32,
    /// Dwords `+0x361C..+0x3624`: the smoothed ambience position (centroid of the live pieces).
    pub ambient_position: Vec3,
    /// Dword `+0x3628`: the smoothed ambience volume.
    pub ambient_volume: f32,
    /// `S + 0xC + 0x63C * n`: the eight flying blobs (T1845 round 4: the "player" of a piece is its blob index).
    pub blobs: Vec<Blob>,
    /// `S + 0`: seconds since the blast started.
    pub time: f32,
    /// Dword `S + 8`: 0 after the last schedule entry, 1 while the first two entries run, 2 after.
    pub phase: u32,
}

impl Default for BlastState {
    fn default() -> Self {
        BlastState {
            active: false,
            piece_count: 0,
            pieces: vec![BlastPiece::default(); BLAST_MAX_PIECES],
            history: [0; 30],
            history_position: 0,
            doof_countdown: 0,
            doof_position: [0.0; 3],
            ambience_handle: -1,
            ambient_position: [0.0; 3],
            ambient_volume: 0.0,
            blobs: vec![Blob::default(); BLAST_BODIES],
            time: 0.0,
            phase: 0,
        }
    }
}

/// What `0x36B7A0` reads besides the struct.
#[derive(Clone, Copy, Debug)]
pub struct BlastInput {
    pub timing: Timing,
    /// The OR of the seven room visibility tables at `0x5476C4..0x5476DC` for room `[0x7B977C]` is non-zero.
    pub room_visible: bool,
    /// Build mode byte `[0x7DE455]` and local player count `[0x790950]`.
    pub mode_byte: u8,
    pub local_players: i32,
}

/// The step threshold: 2 for a split screen story game, else 1 (`[0x7DE455] == 0xA && players > 1`).
pub fn blast_step_threshold(mode_byte: u8, local_players: i32) -> i32 {
    if mode_byte == 0x0A && local_players > 1 {
        2
    } else {
        1
    }
}

/// `0x3697C0` (MEASURED): the blast ambience loop. The live pieces (status other than 3) among the first
/// `piece_count` give a centroid; the smoothed ambience position moves a tenth of the way to it (not at all without a
/// live piece). Pool entries past `piece_count` with a status other than 3 are then added to the live count (the image
/// keeps counting in the same register, so never-spawned entries raise the volume). The target volume is
/// `min(live * 0.05, 1.0)`, approached with step 0.1 (`0x1515E0`), and loop sound `0x15A` plays at the smoothed
/// position with range 20.0, pitch 1.0 and flags 4 on the handle `+0x362C`.
pub fn blast_ambience(state: &mut BlastState) -> SceneEffect {
    let count = state.piece_count.clamp(0, BLAST_MAX_PIECES as i32) as usize;
    let mut sum = [0.0f32; 3];
    let mut live = 0i32;
    for piece in &state.pieces[..count] {
        if piece.state != 3 {
            for axis in 0..3 {
                sum[axis] += piece.position[axis];
            }
            live += 1;
        }
    }
    if live != 0 {
        let inverse = 1.0 / live as f32;
        for axis in 0..3 {
            let delta = inverse * sum[axis] - state.ambient_position[axis];
            state.ambient_position[axis] = delta * 0.1 + state.ambient_position[axis];
        }
    }
    if count < BLAST_MAX_PIECES {
        live += state.pieces[count..BLAST_MAX_PIECES]
            .iter()
            .filter(|p| p.state != 3)
            .count() as i32;
    }
    let target = (live as f32 * 0.05).min(1.0);
    approach(&mut state.ambient_volume, target, 0.1);
    SceneEffect::LoopSound {
        handle: LoopHandle::BlastAmbience,
        position: Some(state.ambient_position),
        id: 0x15A,
        volume: state.ambient_volume,
        range: 20.0,
        pitch: 1.0,
        flags: 4,
    }
}

/// `0x36B7A0` (MEASURED). `rand` supplies `0x153B40` results (only the low two bits matter).
pub fn blast_frame(
    state: &mut BlastState,
    input: &BlastInput,
    rand: &mut dyn FnMut() -> u32,
) -> Vec<SceneEffect> {
    let mut out = Vec::new();
    if !state.active {
        return out;
    }
    out.push(blast_ambience(state));
    if !input.room_visible {
        return out;
    }
    let count = state.piece_count.clamp(0, BLAST_MAX_PIECES as i32) as usize;
    let dead = state.pieces[..count]
        .iter()
        .filter(|p| p.state == 3)
        .count() as i32;
    state.history[state.history_position] = input.timing.steps;
    state.history_position += 1;
    if state.history_position >= 30 {
        state.history_position = 0;
    }
    let threshold = blast_step_threshold(input.mode_byte, input.local_players);
    let all_short = !(input.timing.rate > 0 && input.timing.steps > threshold);
    if input.timing.steps > threshold {
        state.piece_count = (state.piece_count - BLAST_TRIM_PER_FRAME).max(BLAST_MIN_PIECES);
    } else if all_short && (state.piece_count as usize) < BLAST_MAX_PIECES {
        let need = BLAST_MAX_PIECES - state.piece_count as usize;
        for _ in 0..need.min(BLAST_REFILL_PER_FRAME) {
            let at = state.piece_count as usize;
            let piece = state.pieces[at];
            let blob = &state.blobs[piece.player.min(BLAST_BODIES - 1)];
            let mut back = blob.ring_index - piece.frames_back;
            if back < 0 {
                back += input.timing.rate * 2;
            }
            let index = (back.max(0) as usize).min(blob.ring.len().max(1) - 1);
            state.pieces[at].position = blob.ring.get(index).copied().unwrap_or([0.0; 3]);
            state.piece_count += 1;
        }
    }
    if dead == BLAST_MAX_PIECES as i32 {
        out.push(SceneEffect::SlotFinish { slot: 9, arg: 0 });
        state.active = false;
        out.push(SceneEffect::StopSound {
            handle: state.ambience_handle,
        });
        state.ambience_handle = -1;
        return out;
    }
    if dead as f32 > BLAST_SPAWN_THRESHOLD {
        for index in 0..state.piece_count.max(0) as usize {
            if state.pieces[index].state == 0 {
                out.push(SceneEffect::Particle {
                    template: 0x3500,
                    anchor: ParticleAnchor::Piece(index),
                    vector: [0.0, 1.0, 0.0],
                });
                state.pieces[index].state = 3;
                let id = 0x156 + (rand() & 3);
                out.push(SceneEffect::Sound {
                    id,
                    position: state.pieces[index].position,
                });
            }
        }
    }
    out
}

// ------------------------------------------------------------------ 0x1DD510 beam

/// The beam struct `[0x75B0C4]`.
#[derive(Clone, Debug, Default)]
pub struct BeamState {
    /// `+0x20`: 0 idle, 1 charging, 2 fading.
    pub phase: u32,
    /// `+0x24`: ramp, 0 to 1.
    pub ramp: f32,
    /// `+8..+0x10` beam start and `+0x14..+0x1C` end.
    pub start: Vec3,
    pub end: Vec3,
    /// Five particle handles `+0x28..+0x38`.
    pub particles: [u32; 5],
}

/// One object of the list `[0x7B5F50]` the beam tests.
#[derive(Clone, Copy, Debug)]
pub struct BeamTarget {
    /// Word `+0x28` is negative: skipped.
    pub skipped: bool,
    /// `+0x64 + 0x1A8`, `+0x68 + 0x1AC`, `+0x6C + 0x1B0`.
    pub position: Vec3,
    /// Extension floats `+0x1C0` and `+0x1C8`.
    pub extent: [f32; 2],
    /// Word `+0x20 == 8` (a character).
    pub is_character: bool,
    /// Character path: the controller `+0x7C` exists and its `+8` is below the local player count (a player),
    /// or the type word `[[ctrl]+0x60]` is "ROCK" (`0x524F434B`). `None` when the controller is null.
    pub controller: Option<BeamController>,
}

#[derive(Clone, Copy, Debug)]
pub struct BeamController {
    pub is_player: bool,
    pub is_rock: bool,
}

/// What the beam driver reads besides its struct.
#[derive(Clone, Debug)]
pub struct BeamInput {
    pub timing: Timing,
    /// `0x153B40() % rate < steps` was true this frame (the ground effect roll) and the random fraction in [0, 1).
    pub roll: Option<u32>,
    pub fraction: f32,
    /// Player 0 block `+0xF54` (area) and, in split screen story, `+0x24D8` of the same block.
    pub area: u32,
    pub second_area: u32,
    pub mode_byte: u8,
    pub local_players: i32,
    /// `0x879A0(0x23)` and `0x879A0(0x19)`: table entry word `+0x16` set.
    pub table_23: bool,
    pub table_19: bool,
    pub targets: Vec<BeamTarget>,
}

/// Beam ground effect chance `rand % rate < steps` (`0x1DD595`): `random` is the raw value.
pub fn beam_roll(random: u32, timing: Timing) -> bool {
    timing.rate > 0 && (random % timing.rate as u32) < timing.steps as u32
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `0x1DD510` (MEASURED).
pub fn beam_frame(state: &mut BeamState, input: &BeamInput) -> Vec<SceneEffect> {
    let mut out = Vec::new();
    let dt = input.timing.dt();
    let delta = [
        state.end[0] - state.start[0],
        state.end[1] - state.start[1],
        state.end[2] - state.start[2],
    ];
    match state.phase {
        1 => {
            state.ramp = (state.ramp + dt).min(1.0);
            if let Some(random) = input.roll {
                let _ = random;
                let t = state.ramp * input.fraction;
                let point = [
                    state.start[0] + t * delta[0],
                    state.start[1] + t * delta[1],
                    state.start[2] + t * delta[2],
                ];
                out.push(SceneEffect::GroundEffect { point });
            }
        }
        2 => {
            state.ramp -= dt * 0.125;
            if 0.0 > state.ramp {
                for &handle in &state.particles {
                    out.push(SceneEffect::FreeParticle { handle });
                }
                state.ramp = 0.0;
                state.phase = 0;
            } else {
                for (index, &handle) in state.particles.iter().enumerate() {
                    // Handles 0 and 3 only get the +0x74 reset, 1 gets both, 2 and 4 only the scale pair.
                    if index != 0 && index != 3 {
                        out.push(SceneEffect::ParticleFields68_6C {
                            handle,
                            a: state.ramp,
                            b: state.ramp,
                        });
                    }
                    if index == 0 || index == 3 || index == 1 {
                        out.push(SceneEffect::ParticleField74 { handle, value: 0.0 });
                    }
                }
            }
        }
        _ => {}
    }
    // Particle intensity (+0x74) by area (0x1DD626..0x1DD7CF).
    let split_other =
        input.mode_byte == 0x0A && input.local_players > 1 && input.second_area != 0x18;
    let intensity = if input.area == 0x18 && !split_other && !input.table_23 {
        Some(if input.table_19 { 2.5 } else { 0.01 })
    } else if state.phase == 1 {
        Some(10.0)
    } else {
        None
    };
    if let Some(value) = intensity {
        for &handle in &state.particles {
            out.push(SceneEffect::ParticleField74 { handle, value });
        }
    }
    if state.phase != 0 && !(state.phase == 2 && 0.5 > state.ramp) {
        beam_damage(state, input, delta, &mut out);
    }
    out.push(SceneEffect::LoopSound {
        handle: LoopHandle::BeamHum,
        position: Some([-39.58, 0.0, 19.0]),
        id: 0xBFB,
        volume: state.ramp * 2.0,
        range: 20.0,
        pitch: 1.0,
        flags: 0,
    });
    out
}

/// `0x1DD7F6..0x1DDADF`: closest point on the beam segment to each object, damage when inside the radius.
fn beam_damage(state: &BeamState, input: &BeamInput, delta: Vec3, out: &mut Vec<SceneEffect>) {
    let length_squared = dot(delta, delta);
    for (index, target) in input.targets.iter().enumerate() {
        if target.skipped {
            continue;
        }
        let from_object = [
            state.start[0] - target.position[0],
            state.start[1] - target.position[1],
            state.start[2] - target.position[2],
        ];
        let projected = if length_squared == 0.0 {
            f32::NAN
        } else {
            -dot(from_object, delta) / length_squared
        };
        let s = if projected > 1.0 {
            1.0
        } else if 0.0 > projected {
            0.0
        } else if projected.is_nan() {
            0.0
        } else {
            projected
        };
        let closest = [
            state.start[0] + s * delta[0],
            state.start[1] + s * delta[1],
            state.start[2] + s * delta[2],
        ];
        let diff = [
            closest[0] - target.position[0],
            closest[1] - target.position[1],
            closest[2] - target.position[2],
        ];
        let distance_squared = dot(diff, diff);
        let radius_squared =
            target.extent[0] * target.extent[0] + target.extent[1] * target.extent[1] + 4.0;
        if !(radius_squared > distance_squared) {
            continue;
        }
        let inverse = 1.0 / distance_squared.sqrt();
        let direction = [
            inverse * (target.position[0] - closest[0]),
            inverse * (target.position[1] - closest[1]),
            inverse * (target.position[2] - closest[2]),
        ];
        let damage = |out: &mut Vec<SceneEffect>| {
            out.push(SceneEffect::Damage {
                victim: DamageVictim::ListObject(index),
                amount: 0.2,
                kind: 0x15,
                point: closest,
                direction,
            })
        };
        if target.is_character {
            match target.controller {
                // MEASURED: the image dereferences the null controller (0x1DDA0C). Treated as a skipped object.
                None => {}
                Some(controller) if controller.is_player || controller.is_rock => damage(out),
                Some(_) => out.push(SceneEffect::PlayerEffect {
                    target: index as u32,
                }),
            }
        } else {
            damage(out);
        }
    }
}

// ------------------------------------------------------------------ 0x1DAAB0 worms

pub const WORM_COUNT: usize = 5;
/// Arena box of the worms (x, y, z), MEASURED constants.
pub const ARENA_MIN: Vec3 = [-77.2, f32::MIN, -6.0];
pub const ARENA_MAX: Vec3 = [-55.7, f32::MAX, 39.5];
/// Height every worm is pinned to (`[0x49DF08]`).
pub const WORM_Y: f32 = -2.399;
/// The turn added when a worm touches the arena wall (pi / 8).
pub const WALL_TURN: f32 = 0.392_699_1;
/// Worm speed (`[0x475D30]`).
pub const WORM_SPEED: f32 = 6.0 / 1.0;

/// Worm states. Names are INFERRED, the numbers are MEASURED.
pub mod worm_state {
    pub const IDLE: u32 = 0;
    pub const PATH_ENTRY: u32 = 1;
    pub const PATH_FIXED: u32 = 2;
    pub const PATH_TO_PLAYER: u32 = 3;
    pub const DESPAWN: u32 = 4;
    pub const ATTACK_RUN: u32 = 5;
    pub const RECOVER: u32 = 6;
    pub const WAIT_PLAYER: u32 = 7;
    pub const LEAP: u32 = 8;
    pub const BREATH: u32 = 9;
    pub const ANIM_A: u32 = 0xA;
    pub const ANIM_B: u32 = 0xB;
    pub const ANIM_C: u32 = 0xC;
    pub const ANIM_D: u32 = 0xD;
    pub const ANIM_E: u32 = 0xE;
    pub const ANIM_F: u32 = 0xF;
    pub const ANIM_10: u32 = 0x10;
}

/// Per state class of `0x1DAEDC`: 1 sets the object flag `0x40`, 0 clears it, 2 leaves it. States above 0x10 do nothing.
pub fn worm_flag_class(state: u32) -> Option<bool> {
    const CLASS: [u8; 17] = [0, 0, 2, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 0, 1];
    match CLASS.get(state as usize)? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// What a breath ray hit (`0x88C60` result `ebp`, hit point and negated ray direction).
#[derive(Clone, Debug, PartialEq)]
pub struct RayHit {
    pub object: u32,
    /// The hit object's word `+0x150` is non-zero: the event "CCFS" (`0x53464343`) is armed by `0x234CD0`.
    pub arms_event: bool,
    pub point: Vec3,
    /// Negated ray direction (the damage record's direction).
    pub direction: Vec3,
}

/// One worm (stride `0x450`).
#[derive(Clone, Debug)]
pub struct Worm {
    pub position: Vec3,
    pub previous: Vec3,
    /// `+0x18`: smoothed speed.
    pub speed: f32,
    /// `+0x1C`: heading in radians.
    pub heading: f32,
    /// `+0x20`: counter (breath count in state 0xC).
    pub count: i32,
    pub state: u32,
    /// `+0x28`: timer or height.
    pub height: f32,
    /// `+0x2C..+0x34` target and `+0x38` target heading.
    pub target: Vec3,
    pub target_heading: f32,
    /// `+0x440` current and `+0x444` previous path node, -1 = none.
    pub node: i32,
    pub previous_node: i32,
    /// `+0x448` flag, `+0x44C` mode (2 = special, 7 = from state 7, -1 none).
    pub flag: i32,
    pub mode: i32,
    /// `+0x3C`: 64 trail points of 3 floats (stride 12). Re-seeded by [`reseed_trail`].
    pub trail: [Vec3; 64],
}

impl Worm {
    pub fn new(position: Vec3) -> Worm {
        Worm {
            position,
            previous: position,
            speed: 0.0,
            heading: 0.0,
            count: 0,
            state: 0,
            height: 0.0,
            target: position,
            target_heading: 0.0,
            node: -1,
            previous_node: -1,
            flag: 0,
            mode: 0,
            trail: [position; 64],
        }
    }
}

/// Sine table `0x7A38A0` (2048 entries over a full turn). The table is filled at run time; computed here.
pub fn sine_entry(index: i32) -> f32 {
    (f64::from(index & 0x7FF) * std::f64::consts::TAU / 2048.0).sin() as f32
}

/// Table index of an angle: `(int)(angle * 325.949 + 0.5)` (truncation toward zero).
pub fn angle_index(angle: f32) -> i32 {
    (angle * 325.949_32 + 0.5) as i32
}

/// Wrap an angle into `(-pi, pi]` the way `0x1DB190..0x1DB1BF` does.
pub fn wrap_angle(mut angle: f32) -> f32 {
    while -PI >= angle {
        angle += 2.0 * PI;
    }
    while angle > PI {
        angle -= 2.0 * PI;
    }
    angle
}

/// A player as the worms see it.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerView {
    pub position: Vec3,
    /// Controller path node id (`ctrl + 0x678`).
    pub node: i32,
}

/// World queries of the worm driver.
pub trait WormWorld {
    /// `[0x75B4BC] != 0`.
    fn enabled(&self) -> bool;
    /// Byte `+0x18` of the structure at `[0x7B977C]` is non-zero.
    fn room_flag(&self) -> bool;
    /// `[0x4FDC20]`.
    fn global_flag(&self) -> i32;
    fn set_global_flag(&mut self, value: i32);
    /// `[0x75B4B8]`: worms currently attacking.
    fn attacker_count(&self) -> i32;
    fn set_attacker_count(&mut self, value: i32);
    fn players(&self) -> Vec<PlayerView>;
    /// Does worm `entity` have a live object (`[0x78FAE0]`, `+4`, flag word not negative)?
    fn object_alive(&self, entity: usize) -> bool;
    fn object_exists(&self, entity: usize) -> bool;
    /// Object flag word `+0x28`.
    fn object_controller_timer(&self, entity: usize) -> Option<f32>;
    /// `0x1D5D70`: some player is inside the fixed level box.
    fn players_inside_box(&self) -> bool;
    /// `0x1D2160(worm)`: a player is within the bite cone.
    fn bite_cone_hit(&self, entity: usize, worm: &Worm) -> bool;
    /// `0x9BCF0(worm, 0)`: nearest active path node id.
    fn nearest_node(&self, worm: &Worm) -> i32;
    /// `0x9A420(id)` node position (`+0xC`, `+0x10`, `+0x14`), `None` for a missing node.
    fn node_position(&self, id: i32) -> Option<Vec3>;
    /// The six patrol waypoints `[0x4FDC24 + 4*k]`.
    fn patrol_nodes(&self) -> [i32; 6];
    /// `0x1596F0` next hop from node `from` to node `to`; negative = no path.
    fn path_next_hop(&self, from: i32, to: i32) -> i32;
    /// `0x9A420(0x2349)` position (the key node of state 7).
    fn key_node_position(&self) -> Vec3;
    /// `0x59D20(object, animation)`: is that animation the current one?
    fn animation_is(&self, entity: usize, animation: u32) -> bool;
    /// `0x59DD0`: the current animation finished.
    fn animation_finished(&self, entity: usize) -> bool;
    /// Random value `0x153B40`.
    fn random(&mut self) -> u32;
    /// `0x468A0` returned non-zero for this worm's loop sound this frame.
    fn loop_sound_started(&self, entity: usize) -> bool;
    /// Dword at `0x75B4B4 + 0x1164` (end marker of the worm array).
    fn end_marker(&self) -> i32;
    /// T1845: the controller float the breath and recover states scale into the height: `ctrl + 0x1F4`, or for
    /// the special mode 2 `ctrl + 0x34` (the model adds 4.0 for mode 2 like the image). Default 0.
    fn controller_rise(&self, _entity: usize, _special: bool) -> f32 {
        0.0
    }
    /// T1845: `0x59E70(object)`: the ratio of the object's field `+0xDC` over its table entry (state 9, mode 2: the
    /// particle burst needs it above 0.1). Default 1.0.
    fn object_ratio(&self, _entity: usize) -> f32 {
        1.0
    }
    /// T1845: the breath ray of states 9 (mode other than 2) and 0xA: a world geometry ray cast (`0x88C60`) along the
    /// controller segment `+0x1B0..+0x1B8` to `+0x1F0..+0x1F8` rotated by the object yaw (`+0x70` degrees) and moved to
    /// the object position (`+0x64..+0x6C`), extended by 2.0 times the direction (INFERRED geometry). `None` when the
    /// ray hits nothing or the creature itself.
    fn breath_ray(&mut self, _entity: usize) -> Option<RayHit> {
        None
    }
}

fn distance(a: Vec3, b: Vec3) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    dot(d, d).sqrt()
}

/// Index of the player nearest to `position` (first minimum; initial best is `f32::MAX` as `[0x475D5C]`).
pub fn nearest_player(position: Vec3, players: &[PlayerView]) -> usize {
    let mut best = 0;
    let mut best_distance = f32::MAX;
    for (index, player) in players.iter().enumerate() {
        let d = [
            player.position[0] - position[0],
            player.position[1] - position[1],
            player.position[2] - position[2],
        ];
        let squared = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        if best_distance > squared {
            best_distance = squared;
            best = index;
        }
    }
    best
}

/// The unit direction and heading step shared by the movement code. `heading` is turned and the worm moved along
/// `sin`/`cos` of it (`0x1DB754..0x1DB7F1`): x uses the sine table at the heading index, z at +0x200 (cosine).
pub fn worm_move(worm: &mut Worm, timing: Timing) {
    let index = angle_index(worm.heading);
    let dt = (1.0 / timing.rate as f32) * timing.steps as f32;
    // The image multiplies (1/rate) * table * steps * speed in that order.
    let step = |table: f32| (1.0 / timing.rate as f32) * table * timing.steps as f32 * WORM_SPEED;
    let _ = dt;
    worm.position[0] += step(sine_entry(index));
    worm.position[2] += step(sine_entry(index + 0x200));
    worm.position[1] = WORM_Y;
    clamp_to_arena(worm);
}

fn clamp_to_arena(worm: &mut Worm) {
    for axis in 0..3 {
        if worm.position[axis] > ARENA_MAX[axis] {
            worm.heading += WALL_TURN;
            worm.position[axis] = ARENA_MAX[axis];
        } else if ARENA_MIN[axis] > worm.position[axis] {
            worm.heading += WALL_TURN;
            worm.position[axis] = ARENA_MIN[axis];
        }
    }
}

/// Steering toward the nearest player in states 0, 5 and 6 (`0x1DB142..0x1DB22D`): the heading error is
/// `atan2(dx, dz) + pi - heading` wrapped, the turn rate is 0.628 rad per second, and the sign of the update is the
/// opposite of the sign of the error (MEASURED, kept literally).
pub fn chase_turn(worm: &mut Worm, player: Vec3, timing: Timing) {
    let angle =
        (player[0] - worm.position[0]).atan2(player[2] - worm.position[2]) + PI - worm.heading;
    let delta = wrap_angle(angle);
    let turn = (1.0 / timing.rate as f32) * timing.steps as f32 * 0.628_318_5;
    if delta > 0.0 {
        worm.heading -= turn;
    } else {
        worm.heading += turn;
    }
}

/// Turn toward a path target in states 1, 2 and 3 (`0x1DB5F4..0x1DB749`), 2.513 rad per second, never overshooting.
pub fn path_turn(worm: &mut Worm, target: Vec3, timing: Timing) {
    let angle = (target[0] - worm.position[0]).atan2(target[2] - worm.position[2]);
    let delta = wrap_angle(worm.heading - angle);
    let step = (1.0 / timing.rate as f32) * timing.steps as f32 * 2.513_274;
    if delta > step {
        worm.heading -= step;
    } else if -step > delta {
        worm.heading += step;
    }
}

/// Which waypoint the patrol follows next (`0x1DB370..0x1DB3D1`): the entry after the current node, in a ring of
/// six; when the current node is not in the ring the nearest waypoint to the worm is chosen.
pub fn patrol_target(worm: &Worm, world: &dyn WormWorld) -> Option<PatrolTarget> {
    let nodes = world.patrol_nodes();
    if let Some(k) = nodes.iter().position(|&n| n == worm.node) {
        let next = nodes[(k + 1) % 6];
        return world.node_position(next).map(PatrolTarget::Position);
    }
    let mut best = f32::MAX;
    let mut best_k = 0;
    for (k, &node) in nodes.iter().enumerate() {
        if let Some(p) = world.node_position(node) {
            let d = [
                worm.position[0] - p[0],
                worm.position[1] - p[1],
                worm.position[2] - p[2],
            ];
            let squared = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            if best > squared {
                best = squared;
                best_k = k;
            }
        }
    }
    Some(PatrolTarget::Route {
        goal: nodes[best_k],
    })
}

/// Result of [`patrol_target`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PatrolTarget {
    Position(Vec3),
    /// Ask the path finder (`0x1596F0`) for the next hop toward the node.
    Route {
        goal: i32,
    },
}

/// Node bookkeeping of states 1 to 3 (`0x1DB254..0x1DB35A`).
pub fn update_nodes(worm: &mut Worm, world: &dyn WormWorld) {
    let current = world.nearest_node(worm);
    let near = current != worm.node
        && world
            .node_position(current)
            .map(|p| {
                let d = [
                    p[0] - worm.position[0],
                    p[1] - worm.position[1],
                    p[2] - worm.position[2],
                ];
                1.5 > d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
            })
            .unwrap_or(false);
    let shift = near || worm.previous_node == -1 || worm.node == -1;
    if current == worm.node && !shift {
        return;
    }
    if shift {
        worm.previous_node = worm.node;
        if worm.node == -1 {
            worm.previous_node = current;
        }
        worm.node = current;
    }
}

/// The separation pass between worms `me < other` (`0x1DBC50..0x1DBE64`). Only the x and z of the pair move.
pub fn separate(worms: &mut [Worm; WORM_COUNT], me: usize, timing: Timing) {
    for other in me + 1..WORM_COUNT {
        let d = [
            worms[me].position[0] - worms[other].position[0],
            worms[me].position[1] - worms[other].position[1],
            worms[me].position[2] - worms[other].position[2],
        ];
        let squared = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        if !(4.0 > squared) {
            continue;
        }
        let inverse = 1.0 / squared.sqrt();
        let (ux, uz) = (inverse * d[0], inverse * d[2]);
        let (mut push_me, mut push_other) = (4.0_f32, 4.0_f32);
        if worms[me].state >= 8 {
            worms[other].heading += FRAC_PI_2 / 2.0;
            push_me = 0.0;
            push_other = 8.0;
        } else if worms[other].state >= 8 {
            worms[me].heading += FRAC_PI_2 / 2.0;
            push_me = 8.0;
            push_other = 0.0;
        }
        let step = |weight: f32, component: f32| {
            (1.0 / timing.rate as f32) * timing.steps as f32 * weight * component
        };
        worms[me].position[0] += step(push_me, ux);
        worms[me].position[2] += step(push_me, uz);
        worms[other].position[0] -= step(push_other, ux);
        worms[other].position[2] -= step(push_other, uz);
    }
}

/// Animation ids of the attack states (MEASURED): the animation a state plays.
pub fn worm_state_animation(state: u32) -> Option<u32> {
    Some(match state {
        worm_state::ANIM_B => 0x195,
        worm_state::ANIM_C => 0x199,
        worm_state::ANIM_D => 0x19A,
        worm_state::ANIM_E => 0x198,
        worm_state::ANIM_F => 0x19B,
        worm_state::ANIM_10 => 0x199,
        worm_state::ANIM_A => 0x197,
        worm_state::BREATH => 0x149,
        _ => return None,
    })
}

/// Result of one state step in the attack range.
#[derive(Clone, Debug, PartialEq)]
pub struct AttackStep {
    pub effects: Vec<SceneEffect>,
}

/// States 0xB, 0xC, 0xD, 0xF and 0x10 (`0x1DCEBC..0x1DD3DD`, MEASURED). `entity` is the worm index.
pub fn attack_state_step(
    worms: &mut [Worm; WORM_COUNT],
    entity: usize,
    world: &mut dyn WormWorld,
    timing: Timing,
    target_player: Vec3,
) -> Vec<SceneEffect> {
    let mut out = Vec::new();
    let state = worms[entity].state;
    let ensure = |animation: u32, world: &dyn WormWorld, out: &mut Vec<SceneEffect>| {
        if !world.animation_is(entity, animation) {
            out.push(SceneEffect::SetAnimation { entity, animation });
        }
    };
    match state {
        worm_state::ANIM_B => {
            ensure(0x195, world, &mut out);
            if world.animation_finished(entity) {
                let worm = &mut worms[entity];
                let sin = sine_entry(angle_index(worm.target_heading));
                let cos = sine_entry(angle_index(worm.target_heading) + 0x200);
                let dx = target_player[0] - worm.position[0];
                let dz = target_player[2] - worm.position[2];
                let facing = sin * dx + cos * dz;
                let far = dz * dz + dx * dx;
                worm.state = if 0.0 > facing || far > 64.0 {
                    worm_state::ANIM_E
                } else {
                    worm_state::ANIM_A
                };
            }
        }
        worm_state::ANIM_F => {
            ensure(0x19B, world, &mut out);
            if !world.animation_is(entity, 0x19B) {
                let id = 0x13AB + world.random() % 3;
                out.push(SceneEffect::Sound {
                    id,
                    position: worms[entity].position,
                });
            }
            if world.animation_finished(entity) {
                out.push(SceneEffect::FreeParticle { handle: 0 });
                out.push(SceneEffect::ArmPlayerEffect { amount: 10.0 });
                let attackers = world.attacker_count();
                world.set_attacker_count(attackers - 1);
                worms[entity].state = worm_state::DESPAWN;
            }
            worms[entity].height *= 0.2;
        }
        worm_state::ANIM_10 => {
            ensure(0x199, world, &mut out);
            if world.animation_finished(entity) {
                worms[entity].state = worm_state::ANIM_D;
            }
        }
        worm_state::ANIM_C => {
            if world.animation_finished(entity) {
                if world.animation_is(entity, 0x199) {
                    out.push(SceneEffect::SetAnimation {
                        entity,
                        animation: 0x196,
                    });
                    worms[entity].count += 1;
                } else if world.animation_is(entity, 0x196) {
                    out.push(SceneEffect::SetAnimation {
                        entity,
                        animation: 0x199,
                    });
                } else {
                    let id = 0x13A8 + world.random() % 3;
                    out.push(SceneEffect::Sound {
                        id,
                        position: worms[entity].position,
                    });
                    out.push(SceneEffect::SetAnimation {
                        entity,
                        animation: 0x196,
                    });
                }
            }
            let count = worms[entity].count;
            if count >= 1 {
                for _ in 0..timing.steps.max(0) {
                    worms[entity].height *= 0.99;
                }
            }
            if count >= 2 {
                worms[entity].state = worm_state::ANIM_F;
            }
        }
        worm_state::ANIM_D => {
            ensure(0x19A, world, &mut out);
            for _ in 0..timing.steps.max(0) {
                worms[entity].height *= 0.97;
            }
            if 0.01 > worms[entity].height {
                let attackers = world.attacker_count();
                world.set_attacker_count(attackers - 1);
                let worm = &mut worms[entity];
                worm.state = worm_state::RECOVER;
                worm.height = -1.1;
                reseed_trail(worm);
            }
        }
        _ => {}
    }
    out
}

/// The per frame gate and prologue of `0x1DAAB0` for all five worms (`0x1DAAB0..0x1DAC9C`): speed smoothing,
/// the object (re)creation, the control call and the engine loop sound. Returns whether any loop sound was running.
pub fn worm_prologue(
    worms: &mut [Worm; WORM_COUNT],
    world: &mut dyn WormWorld,
    timing: Timing,
    out: &mut Vec<SceneEffect>,
) -> bool {
    let mut any_sound = false;
    for (index, worm) in worms.iter_mut().enumerate() {
        if worm.state == worm_state::DESPAWN {
            continue;
        }
        let d = [
            worm.position[0] - worm.previous[0],
            worm.position[1] - worm.previous[1],
            worm.position[2] - worm.previous[2],
        ];
        worm.previous = worm.position;
        let length = dot(d, d).sqrt();
        let target = (length * timing.rate as f32 / timing.steps as f32 * 0.25).min(1.0);
        approach(&mut worm.speed, target, 0.1);
        if !world.object_exists(index) {
            out.push(SceneEffect::CreateObject { entity: index });
        }
        if world.object_exists(index) {
            out.push(SceneEffect::ObjectControl { entity: index });
        }
        out.push(SceneEffect::LoopSound {
            handle: LoopHandle::CreatureObject(index),
            position: Some(worm.position),
            id: 0x155,
            volume: worm.speed * 1.2,
            range: 25.0,
            pitch: (worm.speed + 1.0) * 0.5,
            flags: 1,
        });
        if world.loop_sound_started(index) {
            any_sound = true;
        }
    }
    any_sound
}

/// `0x1515E0(value, target, step)`: move toward the target by at most `step`, returns true when reached.
pub fn approach(value: &mut f32, target: f32, step: f32) -> bool {
    let difference = (target - *value).abs();
    if step > difference {
        *value = target;
        true
    } else {
        if target > *value {
            *value += step;
        } else {
            *value -= step;
        }
        false
    }
}

/// `0x1DAAB0` as a whole for one frame. Returns the effects in image order. States 9, 0xA and 0xE are ported by
/// [`breath_step`], [`turning_breath_step`] and [`recover_breath_step`] (T1845 round 4).
pub fn worm_frame(
    worms: &mut [Worm; WORM_COUNT],
    world: &mut dyn WormWorld,
    timing: Timing,
) -> Vec<SceneEffect> {
    let mut out = Vec::new();
    if !world.enabled() || (!world.room_flag() && world.global_flag() == 0) {
        return out;
    }
    let any_sound = worm_prologue(worms, world, timing, &mut out);
    if (world.end_marker() == 0 || world.end_marker() == 1) && !any_sound {
        world.set_global_flag(0);
    }
    let players = world.players();
    for entity in 0..WORM_COUNT {
        if !world.object_alive(entity) || players.is_empty() {
            continue;
        }
        let best = nearest_player(worms[entity].position, &players);
        if let Some(set) = worm_flag_class(worms[entity].state) {
            out.push(SceneEffect::ObjectFlag40 { entity, set });
        }
        match worms[entity].state {
            worm_state::DESPAWN => {
                let timer = world.object_controller_timer(entity);
                let ready = timer
                    .map(|t| !((10.0 / timing.rate as f32) > t))
                    .unwrap_or(true);
                if ready {
                    out.push(SceneEffect::DestroyObject { entity });
                }
                continue;
            }
            worm_state::WAIT_PLAYER => {
                let key = world.key_node_position();
                for player in &players {
                    let dz = player.position[2] - key[2];
                    let dx = player.position[0] - key[0];
                    if 100.0 > dz * dz + dx * dx {
                        worms[entity].state = worm_state::ATTACK_RUN;
                        worms[entity].mode = 7;
                    }
                }
            }
            _ => {}
        }
        let state = worms[entity].state;
        let moving = matches!(state, 0 | 1 | 2 | 3 | 5 | 6);
        if moving && timing.steps != 0 {
            if world.players_inside_box() {
                if state == worm_state::PATH_ENTRY {
                    worms[entity].state = worm_state::IDLE;
                }
            } else if state == worm_state::IDLE || state == worm_state::PATH_TO_PLAYER {
                worms[entity].state = worm_state::PATH_ENTRY;
                worms[entity].node = -1;
                worms[entity].previous_node = -1;
            }
            let state = worms[entity].state;
            if matches!(state, 1..=3) {
                update_nodes(&mut worms[entity], world);
                let target = if state == worm_state::PATH_TO_PLAYER {
                    let hop = world.path_next_hop(worms[entity].node, players[best].node);
                    if hop >= 0 {
                        world.node_position(hop).unwrap_or(players[best].position)
                    } else {
                        worms[entity].state = worm_state::IDLE;
                        players[best].position
                    }
                } else {
                    match patrol_target(&worms[entity], world) {
                        Some(PatrolTarget::Position(p)) => p,
                        Some(PatrolTarget::Route { goal }) => {
                            let hop = world.path_next_hop(worms[entity].node, goal);
                            if hop > 0 {
                                world.node_position(hop).unwrap_or(players[best].position)
                            } else {
                                players[best].position
                            }
                        }
                        None => players[best].position,
                    }
                };
                path_turn(&mut worms[entity], target, timing);
            } else {
                chase_turn(&mut worms[entity], players[best].position, timing);
            }
            worm_move(&mut worms[entity], timing);
            let state = worms[entity].state;
            let spotted = if state == worm_state::IDLE || state == worm_state::PATH_TO_PLAYER {
                world.bite_cone_hit(entity, &worms[entity])
            } else {
                false
            };
            if spotted || (state == worm_state::PATH_FIXED && 27.0 > worms[entity].position[2]) {
                if worms[entity].mode != 2 {
                    world.set_attacker_count(world.attacker_count() + 1);
                    worms[entity].mode = state as i32;
                    worms[entity].state = worm_state::ATTACK_RUN;
                }
                worms[entity].height = 0.0;
            } else if state == worm_state::ATTACK_RUN {
                attack_run(
                    &mut worms[entity],
                    &mut out,
                    entity,
                    &players,
                    best,
                    timing,
                    world,
                );
                if leap_allowed(worms, entity) {
                    if worms[entity].mode == 7 {
                        worms[entity].mode = 0;
                    }
                    worms[entity].state = worm_state::LEAP;
                }
            } else if state == worm_state::RECOVER {
                worms[entity].height += (1.0 / timing.rate as f32) * timing.steps as f32 * 0.8;
                if worms[entity].height > 0.0 {
                    worms[entity].state = worm_state::IDLE;
                    worms[entity].height = 0.0;
                }
            }
            separate(worms, entity, timing);
        }
        if worms[entity].state >= 8 && timing.steps != 0 {
            let player_position = players[best].position;
            if worms[entity].state == worm_state::LEAP {
                let w = &mut worms[entity];
                w.position = w.target;
                w.height = 0.0;
                w.state = worm_state::BREATH;
                w.flag = 0;
                if w.mode != 2 {
                    out.push(SceneEffect::Particle {
                        template: 0x75B208,
                        anchor: ParticleAnchor::Creature(entity),
                        vector: [0.0, 1.0, 0.0],
                    });
                }
            }
            match worms[entity].state {
                worm_state::BREATH => {
                    let effects = breath_step(worms, entity, world, timing, &players);
                    out.extend(effects);
                }
                worm_state::ANIM_E => {
                    let effects = recover_breath_step(worms, entity, world, timing, &players);
                    out.extend(effects);
                }
                worm_state::ANIM_A => {
                    let effects =
                        turning_breath_step(worms, entity, world, timing, player_position);
                    out.extend(effects);
                }
                _ => {
                    let effects = attack_state_step(worms, entity, world, timing, player_position);
                    out.extend(effects);
                }
            }
        }
    }
    out
}

/// State 5, the attack run (`0x1DB8BB..0x1DBC0B`).
fn attack_run(
    worm: &mut Worm,
    out: &mut Vec<SceneEffect>,
    entity: usize,
    players: &[PlayerView],
    best: usize,
    timing: Timing,
    world: &dyn WormWorld,
) {
    let player = players[best].position;
    let flat =
        ((worm.position[2] - player[2]).powi(2) + (worm.position[0] - player[0]).powi(2)).sqrt();
    let scale = ((flat - 3.0) * (1.0 / 12.0)).clamp(-1.0, 1.0);
    worm.height = (1.0 - scale) * -1.1;
    if flat > 15.0 && worm.mode != 7 {
        worm.state = worm_state::IDLE;
    }
    if worm.mode != 2 {
        let base = worm.height * -1.818_181_8 - 1.0;
        for (index, p) in players.iter().enumerate() {
            let distance_to_worm = distance(worm.position, p.position);
            let weight = (distance_to_worm * 0.1).min(1.0);
            let value = (1.0 - weight) * base;
            out.push(SceneEffect::CameraShake {
                player: index,
                intensity: value * 0.4,
                secondary: value * 0.06,
                duration: 1.0 / timing.rate as f32,
                kick: value * 0.2,
            });
        }
        worm.target = worm.position;
        worm.target_heading = (player[0] - worm.position[0]).atan2(player[2] - worm.position[2]);
    } else {
        worm.target = [-62.314, 0.0, 16.9];
    }
    for axis in 0..3 {
        if worm.target[axis] > ARENA_MAX[axis] {
            worm.target[axis] = ARENA_MAX[axis];
        } else if ARENA_MIN[axis] > worm.target[axis] {
            worm.target[axis] = ARENA_MIN[axis];
        }
    }
    worm.target[1] = WORM_Y;
    let _ = (entity, world);
}

/// `0x1DBB8C..0x1DBC0B`: while the height is below -1.1 the worm leaps (state 8) unless another worm in state 8 or
/// higher has its target within 4.8 squared units of this worm.
pub fn leap_allowed(worms: &[Worm; WORM_COUNT], me: usize) -> bool {
    if !(-1.1 > worms[me].height) {
        return false;
    }
    for (index, other) in worms.iter().enumerate() {
        if index == me || other.state < 8 {
            continue;
        }
        let dx = other.target[0] - worms[me].position[0];
        let dz = other.target[2] - worms[me].position[2];
        if 4.8 > dx * dx + dz * dz {
            return false;
        }
    }
    true
}

// ------------------------------------------------------- worm states 9, 0xA, 0xE (T1845 round 4)

/// `0x1DD2CE..0x1DD3D7` (also copied at `0x1DBFA9` and `0x1DCA35`): the 64 trail points are set to the worm position
/// with the height added to y (MEASURED).
pub fn reseed_trail(worm: &mut Worm) {
    let point = [
        worm.position[0],
        worm.position[1] + worm.height,
        worm.position[2],
    ];
    worm.trail = [point; 64];
}

/// The shared tail of the breath states: `h = clamp(rise * 0.25, 0, 0.2)` stored at `+0x28` (MEASURED: the image
/// compares against 0.2 and 0, constants `[0x475D4C]` = 0.25, `[0x4785E4]` = 0.2). Mode 2 adds 4.0 to the rise first.
pub fn breath_height(rise: f32, special: bool) -> f32 {
    let value = (if special { rise + 4.0 } else { rise }) * 0.25;
    if 0.2 > value {
        if value > 0.0 { value } else { 0.0 }
    } else {
        0.2
    }
}

/// The per player camera shake of the breath states (`0x1DC6D0..0x1DC7D6`, `0x1DC8C0..`): the weight falls with the
/// distance (`min(d * 0.1, 1)`), value = `(1 - weight) * 2`, shake `(value * 0.4, value * 0.06, 1 / rate)` and the kick
/// word `+0xFBC += value * 0.2`, `+0xFC0 = 0.1`.
fn breath_shake(worm: &Worm, players: &[PlayerView], timing: Timing, out: &mut Vec<SceneEffect>) {
    for (index, player) in players.iter().enumerate() {
        let weight = (distance(worm.position, player.position) * 0.1).min(1.0);
        let value = (1.0 - weight) * 2.0;
        out.push(SceneEffect::CameraShake {
            player: index,
            intensity: value * 0.4,
            secondary: value * 0.06,
            duration: 1.0 / timing.rate as f32,
            kick: value * 0.2,
        });
    }
}

fn ensure_animation(
    entity: usize,
    animation: u32,
    world: &dyn WormWorld,
    out: &mut Vec<SceneEffect>,
) {
    if !world.animation_is(entity, animation) {
        out.push(SceneEffect::SetAnimation { entity, animation });
    }
}

/// Breath ray damage (`0x1DC3F4..0x1DC4E4` and `0x1DCCCD..0x1DCDA5`): amount 0.8, record kind left at the reset value
/// 0 (INFERRED: no kind is stored), the hit object is the victim; the hit may arm event "CCFS". Returns whether it hit.
fn breath_ray_damage(entity: usize, world: &mut dyn WormWorld, out: &mut Vec<SceneEffect>) -> bool {
    let Some(hit) = world.breath_ray(entity) else {
        return false;
    };
    if hit.arms_event {
        out.push(SceneEffect::ArmEvent {
            key: 0x5346_4343,
            armed: true,
        });
    }
    out.push(SceneEffect::Damage {
        victim: DamageVictim::Object(hit.object),
        amount: 0.8,
        kind: 0,
        point: hit.point,
        direction: hit.direction,
    });
    true
}

/// State 9 (`0x1DBF2E..0x1DC7DB`, MEASURED control flow). Mode 2 (the scripted worm): animation `0x149`; when it ends
/// the worm gives up (attacker count -1, mode -1, state 6, height -1.1, trail re-seeded). Other modes: animation `0x194`
/// with the roar (`0x13A8 + random % 3` and `0x13AF`), the breath ray and its damage (sets the flag `+0x448`), and when
/// the animation ends state `0xB` (target behind or farther than 8) or `0xE`. Both then run the shared height and shake
/// tail. `players` are the views of all players, the target player is the nearest to the worm.
pub fn breath_step(
    worms: &mut [Worm; WORM_COUNT],
    entity: usize,
    world: &mut dyn WormWorld,
    timing: Timing,
    players: &[PlayerView],
) -> Vec<SceneEffect> {
    let mut out = Vec::new();
    let previous_height = worms[entity].height;
    if worms[entity].mode == 2 {
        ensure_animation(entity, 0x149, world, &mut out);
        if world.animation_finished(entity) {
            let attackers = world.attacker_count();
            world.set_attacker_count(attackers - 1);
            let worm = &mut worms[entity];
            worm.mode = -1;
            worm.state = worm_state::RECOVER;
            worm.height = -1.1;
            reseed_trail(worm);
        }
    } else {
        if !world.animation_is(entity, 0x194) {
            out.push(SceneEffect::SetAnimation {
                entity,
                animation: 0x194,
            });
            let id = 0x13A8 + world.random() % 3;
            out.push(SceneEffect::Sound {
                id,
                position: worms[entity].position,
            });
            out.push(SceneEffect::Sound {
                id: 0x13AF,
                position: worms[entity].position,
            });
        }
        if breath_ray_damage(entity, world, &mut out) {
            worms[entity].flag = 1;
        }
        if world.animation_finished(entity) {
            let target = players[nearest_player(worms[entity].position, players)].position;
            let worm = &mut worms[entity];
            let sin = sine_entry(angle_index(worm.target_heading));
            let cos = sine_entry(angle_index(worm.target_heading) + 0x200);
            let dx = target[0] - worm.position[0];
            let dz = target[2] - worm.position[2];
            let facing = sin * dx + cos * dz;
            let far = dz * dz + dx * dx;
            worm.state = if worm.flag != 0 {
                worm_state::ANIM_E
            } else if 0.0 > facing || far > 64.0 {
                worm_state::ANIM_B
            } else {
                worm_state::ANIM_E
            };
        }
    }
    let special = worms[entity].mode == 2;
    let height = breath_height(world.controller_rise(entity, special), special);
    worms[entity].height = height;
    if special && height > 0.0 && !(previous_height > 0.0) && world.object_ratio(entity) > 0.1 {
        out.push(SceneEffect::Particle {
            template: 0x75B208,
            anchor: ParticleAnchor::Point([-62.314, -2.399, 16.9]),
            vector: [0.0, 1.0, 0.0],
        });
    }
    if 0.3 > height && (!special || (height > 0.0 && !(previous_height > 0.0))) {
        breath_shake(&worms[entity], players, timing, &mut out);
    }
    out
}

/// State 0xE (`0x1DC7DB..0x1DCB4C`, MEASURED): animation `0x198`, the shared height rule, then when the height is
/// exactly 0 or the animation ended the worm recovers (attacker count -1, state 6, height -1.1, trail re-seeded);
/// otherwise every player gets the breath shake.
pub fn recover_breath_step(
    worms: &mut [Worm; WORM_COUNT],
    entity: usize,
    world: &mut dyn WormWorld,
    timing: Timing,
    players: &[PlayerView],
) -> Vec<SceneEffect> {
    let mut out = Vec::new();
    ensure_animation(entity, 0x198, world, &mut out);
    let special = worms[entity].mode == 2;
    let height = breath_height(world.controller_rise(entity, special), special);
    worms[entity].height = height;
    if height == 0.0 || world.animation_finished(entity) {
        let attackers = world.attacker_count();
        world.set_attacker_count(attackers - 1);
        let worm = &mut worms[entity];
        worm.state = worm_state::RECOVER;
        worm.height = -1.1;
        reseed_trail(worm);
    } else {
        breath_shake(&worms[entity], players, timing, &mut out);
    }
    out
}

/// State 0xA (`0x1DCB4C..0x1DCEBC`, MEASURED): the breath ray and damage (no flag), the target heading turns toward the
/// player by at most `dt * pi/2` per frame (`dt` from the integer step `[0x4E7944]`), animation `0x197`, and when it
/// ends state 0xE.
pub fn turning_breath_step(
    worms: &mut [Worm; WORM_COUNT],
    entity: usize,
    world: &mut dyn WormWorld,
    timing: Timing,
    player: Vec3,
) -> Vec<SceneEffect> {
    let mut out = Vec::new();
    breath_ray_damage(entity, world, &mut out);
    let worm = &mut worms[entity];
    let difference = wrap_angle(
        (player[0] - worm.position[0]).atan2(player[2] - worm.position[2]) - worm.target_heading,
    );
    let step = (1.0 / timing.rate as f32) * timing.steps as f32 * FRAC_PI_2;
    if difference > step {
        worm.target_heading += step;
    } else if -step > difference {
        worm.target_heading -= step;
    }
    ensure_animation(entity, 0x197, world, &mut out);
    if world.animation_finished(entity) {
        worms[entity].state = worm_state::ANIM_E;
    }
    out
}

// ------------------------------------------------ 0x368A10 blast blobs (T1845 round 4)

/// Number of blobs (blocks of `0x63C` bytes at `S + 0xC`).
pub const BLAST_BODIES: usize = 8;

/// A face of the room the blob slides on (record at the blob's `+0x62C`): the fields the update reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlobFace {
    /// Identity of the face record (the original compares record addresses; 0 = null pointer). Set by
    /// [`super::face_geometry::FaceWorld`].
    pub id: u32,
    /// `+0x08`, `+0x10`: the edge start in x and z (T1846 batch 8, `0x3675E0` and the face callees).
    pub x0: f32,
    pub z0: f32,
    /// `+0x14`, `+0x1C`: the edge end in x and z.
    pub x1: f32,
    pub z1: f32,
    /// `+0x0C` and `+0x18`: y range.
    pub y_low: f32,
    pub y_high: f32,
    /// `+0x20` and `+0x28`: x range.
    pub x_low: f32,
    pub x_high: f32,
    /// `+0x24` and `+0x2C`: z range.
    pub z_low: f32,
    pub z_high: f32,
}

/// One blob (`0x63C` bytes). Offsets in the comments are within the block.
#[derive(Clone, Debug)]
pub struct Blob {
    /// `+0x00`.
    pub position: Vec3,
    /// `+0x0C`: unit flight direction.
    pub direction: Vec3,
    /// `+0x24..+0x2C`: the steering axis (reset to `(0, 1, 0)` when the blob leaves a face).
    pub axis: Vec3,
    /// `+0x3C`: turn rate, kept inside `[2, 4]`.
    pub turn: f32,
    /// `+0x40`: pieces that belong to the blob; the blob is idle at 0.
    pub count: i32,
    /// `+0x44`: next ring slot.
    pub ring_index: i32,
    /// `+0x48`: index of the blob this one chases (-1 none).
    pub target: i32,
    /// `+0x4C`: speed factor, decays toward 1.0.
    pub speed_factor: f32,
    /// `+0x18..+0x20`: centroid of the blob's pieces and `+0x30..+0x38` the running sum (T1845 round 4, `0x369940`).
    pub centroid: Vec3,
    pub sum: Vec3,
    /// `+0x50`: position history, `2 * rate` entries of 12 bytes.
    pub ring: Vec<Vec3>,
    /// `+0x5F0`, `+0x608`: new position and direction of this frame.
    pub work_position: Vec3,
    pub work_direction: Vec3,
    /// `+0x5FC`, `+0x600`, `+0x604`: heading words written when a face is attached (`+0x600` = `atan2(nx, nz)`).
    pub heading: Vec3,
    /// `+0x614`: attached to a room face.
    pub attached: i32,
    /// `+0x618`: room id the blob is in.
    pub room: i32,
    /// `+0x61C`: set to 3 by `0x367CE0` when the room has no face (T1846 batch 8).
    pub state_61c: i32,
    /// `+0x62C`: the face the blob is attached to.
    pub face: BlobFace,
}

impl Default for Blob {
    fn default() -> Self {
        Blob {
            position: [0.0; 3],
            direction: [0.0, 0.0, 1.0],
            axis: [0.0, 1.0, 0.0],
            turn: 3.0,
            count: 0,
            ring_index: 0,
            target: -1,
            speed_factor: 1.0,
            centroid: [0.0; 3],
            sum: [0.0; 3],
            ring: vec![[0.0; 3]; 120],
            work_position: [0.0; 3],
            work_direction: [0.0, 0.0, 1.0],
            heading: [0.0; 3],
            attached: 0,
            room: 0,
            state_61c: 0,
            face: BlobFace::default(),
        }
    }
}

/// One entry of the three entry schedule at `0x5476E0` (stride `0x2C`): kind, three vec3 aim points (one per active
/// blob), and the time below which the entry is current.
#[derive(Clone, Copy, Debug)]
pub struct ScheduleEntry {
    pub kind: i32,
    pub aim: [Vec3; 3],
    pub until: f32,
}

const fn f(bits: u32) -> f32 {
    f32::from_bits(bits)
}

/// The schedule table (MEASURED values).
pub const BLAST_SCHEDULE: [ScheduleEntry; 3] = [
    ScheduleEntry {
        kind: 2,
        aim: [
            [f(0xC213F5C3), 12.0, f(0x4203C28F)],
            [f(0xC1E50A3D), 13.0, f(0x41FB1EB8)],
            [f(0xC1AD47AE), f(0x415C7AE1), 37.75],
        ],
        until: 2.5,
    },
    ScheduleEntry {
        kind: 2,
        aim: [
            [f(0xC1D26666), 12.0, f(0x4202AE14)],
            [f(0xC1E50A3D), 13.0, f(0x41FB1EB8)],
            [f(0xC1AD47AE), f(0x415C7AE1), 37.75],
        ],
        until: f(0x406CCCCD),
    },
    ScheduleEntry {
        kind: 2,
        aim: [
            [f(0xC18E51EC), 12.0, f(0x420551EC)],
            [f(0xC1CB3333), f(0x4171999A), f(0x421E51EC)],
            [f(0xC1AD47AE), f(0x415C7AE1), 37.75],
        ],
        until: 6.5,
    },
];

/// Room ids of the seven room boxes at `0x5476C4` (the order of the box records).
pub const BLAST_ROOM_IDS: [u32; 7] = [0x23, 0x25, 0x27, 0x28, 0x30, 0x31, 0x29];

/// A room box record (stride 24 bytes at `[0x76C23C]`): ground height and x/z range.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RoomBox {
    pub min_x: f32,
    /// Record `+8`: the lowest y (floor).
    pub ground_y: f32,
    pub min_z: f32,
    pub max_x: f32,
    /// Record `+0x14`: the highest y.
    pub max_y: f32,
    pub max_z: f32,
}

/// What `0x368A10` needs besides the state.
pub struct BlastUpdateInput<'a> {
    pub timing: Timing,
    /// `[0x4E794C]`: the float frame multiplier used by the speed decay.
    pub step_float: f32,
    /// Local player positions (`+0xA8..+0xB0` of the player blocks).
    pub players: &'a [Vec3],
    /// Box record per room id of [`BLAST_ROOM_IDS`].
    pub rooms: &'a [Option<RoomBox>; 7],
    /// The kind slot the image reads after the last schedule entry (an uninitialised stack slot: the value is stale).
    pub stale_kind: i32,
}

/// The four room geometry callees (separate functions, not decoded here): contact and sliding on faces.
pub trait BlastGeometry {
    /// `0x3675E0` (`esi` = the work position): slide the work position on the attached face and update the contact.
    fn slide_on_face(&mut self, _blob: &mut Blob) {}
    /// `0x368280` (`eax` = the work position): collide with the room faces and the neighbours, may detach.
    fn collide(&mut self, _blob: &mut Blob) {}
    /// `0x367820` (`esi` = the blob, stack argument = the previous position): find the face the segment hits and reflect.
    fn find_hit_and_reflect(&mut self, _blob: &mut Blob, _previous: Vec3) {}
    /// `0x367CE0` (`eax` = the work position, stack argument = the axis): attach to the nearest face.
    fn attach_to_nearest_face(&mut self, _blob: &mut Blob) {}
    /// `0x367CE0` for a piece (`eax` = the piece, stack argument 0).
    fn attach_piece(&mut self, _piece: &mut BlastPiece) {}
    /// The inlined `0x3675E0` of `0x369940` for a piece (no output normal, the pending flag decides the heading).
    fn slide_piece(&mut self, _piece: &mut BlastPiece) {}
    /// `0x368280` for a piece (`eax` = the piece).
    fn collide_piece(&mut self, _piece: &mut BlastPiece) {}
}

/// `0x366200`: split blob `source` into three: two free blobs receive copies rotated by +80 and -80 degrees about the
/// steering axis, and the pieces of the source (status not 2 or 3) are dealt round robin to source, first and second.
pub fn blast_split(state: &mut BlastState, source: usize) {
    let free: Vec<usize> = (0..BLAST_BODIES)
        .filter(|&n| state.blobs[n].count == 0)
        .collect();
    let [first, second] = match free.as_slice() {
        [a, b, ..] => [*a, *b],
        _ => return,
    };
    state.blobs[source].count = 0;
    state.blobs[source].target = -1;
    let original = state.blobs[source].clone();
    let angle = 80.0f32.to_radians();
    for (child, sign) in [(first, 1.0f32), (second, -1.0f32)] {
        state.blobs[child] = original.clone();
        let matrix = super::pose::rotation_axis_angle(original.axis, sign * angle);
        state.blobs[child].direction =
            super::pose::transform_direction(&matrix, original.direction);
    }
    let owners = [source, first, second];
    let mut dealt = 0usize;
    for piece in state.pieces.iter_mut() {
        if piece.player == source && piece.state != 2 && piece.state != 3 {
            let owner = owners[dealt % 3];
            piece.player = owner;
            state.blobs[owner].count += 1;
            dealt += 1;
        }
    }
}

fn blob_aim(
    kind: i32,
    entry: Option<&ScheduleEntry>,
    body: usize,
    players: &[Vec3],
    position: Vec3,
) -> Option<Vec3> {
    if kind == 1 || kind == 2 {
        // Bodies above 2 would read stack garbage in the image; they never have pieces.
        return entry.and_then(|e| e.aim.get(body).copied());
    }
    let mut best = f32::MAX;
    let mut aim = [0.0, 0.0, -1.0];
    for player in players {
        let d = [
            player[0] - position[0],
            player[1] - position[1],
            player[2] - position[2],
        ];
        let squared = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        if best > squared {
            best = squared;
            aim = *player;
        }
    }
    Some(aim)
}

/// `0x368A10` (MEASURED control flow, one call per frame). Advances the clock, picks the schedule entry, and moves
/// every blob that has pieces: speed decay, up to `steps` steering sub steps with a random turn jitter, the direction
/// projected off the steering axis, the move, the face attach / room box logic through [`BlastGeometry`], and the
/// position ring. `rand` supplies `0x153B40`.
pub fn blast_update(
    state: &mut BlastState,
    input: &BlastUpdateInput,
    geometry: &mut dyn BlastGeometry,
    rand: &mut dyn FnMut() -> u32,
) {
    let steps = input.timing.steps;
    if steps == 0 {
        return;
    }
    let rate = input.timing.rate;
    let inverse_rate = 1.0 / rate as f32;
    state.time = inverse_rate * steps as f32 + state.time;
    let current = BLAST_SCHEDULE
        .iter()
        .position(|entry| entry.until > state.time);
    let mut kind = input.stale_kind;
    match current {
        None => state.phase = 0,
        Some(index) => {
            kind = BLAST_SCHEDULE[index].kind;
            if kind == 1 {
                let previous = index.saturating_sub(1);
                if BLAST_SCHEDULE[previous].until > state.time {
                    blast_split(state, 0);
                }
            }
            if index < 2 {
                let ring_length = (rate * 2).max(0) as usize;
                let origin = state.blobs[1].position;
                let ring = &mut state.blobs[1].ring;
                if ring.len() < ring_length {
                    ring.resize(ring_length, [0.0; 3]);
                }
                for slot in ring.iter_mut().take(ring_length) {
                    *slot = origin;
                }
                state.phase = 1;
            } else {
                state.phase = 2;
            }
        }
    }
    let entry = current.map(|index| &BLAST_SCHEDULE[index]);
    for body in 0..BLAST_BODIES {
        let decay = (0.99f64).powf(f64::from(input.step_float)) as f32;
        let blob = &mut state.blobs[body];
        blob.speed_factor = (blob.speed_factor - 1.0) * decay + 1.0;
        if blob.count <= 0 {
            continue;
        }
        if kind == -1 {
            blast_merge_pass(state, body);
        }
        let Some(aim) = blob_aim(kind, entry, body, input.players, state.blobs[body].position)
        else {
            continue;
        };
        blob_move(state, body, aim, input, geometry, rand);
    }
}

/// The merge pass for kind -1 (`0x368BB0..0x368CD4`): reachable only through the stale kind slot, ported for
/// completeness. Blobs closer than 0.2 merge (counts add, the pieces move to the survivor).
fn blast_merge_pass(state: &mut BlastState, body: usize) {
    for other in 0..BLAST_BODIES {
        if other == body || state.blobs[other].count <= 0 {
            continue;
        }
        let a = state.blobs[body].position;
        let b = state.blobs[other].position;
        let squared = (b[1] - a[1]).powi(2) * 1.0 + (b[2] - a[2]).powi(2) + (b[0] - a[0]).powi(2);
        if state.blobs[other].target == body as i32 {
            if squared > 0.09 {
                state.blobs[other].target = -1;
                state.blobs[body].target = -1;
            }
        } else if 0.04 > squared {
            let gained = state.blobs[other].count;
            state.blobs[body].count += gained;
            state.blobs[other].count = 0;
            state.blobs[other].target = -1;
            for blob in state.blobs.iter_mut() {
                if blob.count > 0 && blob.target == other as i32 {
                    blob.target = -1;
                }
            }
            for piece in state.pieces.iter_mut() {
                if piece.player == other {
                    piece.player = body;
                }
            }
        }
    }
}

fn blob_move(
    state: &mut BlastState,
    body: usize,
    aim: Vec3,
    input: &BlastUpdateInput,
    geometry: &mut dyn BlastGeometry,
    rand: &mut dyn FnMut() -> u32,
) {
    let rate = input.timing.rate;
    let steps = input.timing.steps;
    let inverse_rate = 1.0 / rate as f32;
    let blob = &mut state.blobs[body];
    let previous_position = blob.position;
    let mut max_turn = 0.0f32;
    for sub in 0..steps.max(0) {
        let d = [
            aim[0] - blob.position[0],
            aim[1] - blob.position[1],
            aim[2] - blob.position[2],
        ];
        let (target_angle, current_angle) = if blob.axis[1] == 1.0 {
            (d[0].atan2(d[2]), blob.direction[0].atan2(blob.direction[2]))
        } else {
            let (a, c) = (blob.axis[0], blob.axis[2]);
            (
                d[1].atan2(c * d[0] + (0.0 - a) * d[2]),
                blob.direction[1].atan2(c * blob.direction[0] + (0.0 - a) * blob.direction[2]),
            )
        };
        if sub == 0 {
            let unit = f64::from(rand()) * 2.328_306_436_538_696_3e-10;
            let jitter =
                ((unit + unit - 1.0) * f64::from(1.0 / rate as f32)) * 10.0 + f64::from(blob.turn);
            let jitter = jitter as f32;
            let clamped = if 2.0 > jitter.min(4.0) {
                2.0
            } else {
                jitter.min(4.0)
            };
            blob.turn = clamped;
            max_turn = inverse_rate * clamped;
        }
        let difference = wrap_angle(current_angle - target_angle);
        let angle = if 0.0 > difference {
            0.0 - max_turn
        } else {
            max_turn
        };
        let matrix = super::pose::rotation_axis_angle(blob.axis, angle);
        blob.direction = super::pose::transform_direction(&matrix, blob.direction);
        if sub < steps - 1 {
            for axis in 0..3 {
                blob.position[axis] = inverse_rate * blob.speed_factor * 1.8 * blob.direction[axis]
                    + blob.position[axis];
            }
        }
    }
    // Project the direction off the steering axis and renormalise (`rsqrtss` in the image: exact here).
    let dot = blob.axis[2] * blob.direction[2]
        + blob.axis[0] * blob.direction[0]
        + blob.axis[1] * blob.direction[1];
    let removed = 0.0 - dot;
    for axis in 0..3 {
        blob.direction[axis] = blob.axis[axis] * removed + blob.direction[axis];
    }
    let length_squared = blob.direction[2] * blob.direction[2]
        + blob.direction[1] * blob.direction[1]
        + blob.direction[0] * blob.direction[0];
    if (length_squared - 1.0).abs() > 0.1 && length_squared > 1e-7 {
        let scale = 1.0 / length_squared.sqrt();
        for axis in 0..3 {
            blob.direction[axis] *= scale;
        }
    }
    for axis in 0..3 {
        blob.position[axis] = inverse_rate * 4.0 * blob.direction[axis] + blob.position[axis];
    }
    blob.work_position = blob.position;
    blob.work_direction = blob.direction;
    if blob.attached != 0 {
        let saved = blob.work_position;
        geometry.slide_on_face(blob);
        let face = blob.face;
        let work = blob.work_position;
        let outside = face.x_low - work[0] > 0.05
            || -0.05 > face.x_high - work[0]
            || face.y_low > work[1]
            || work[1] > face.y_high
            || face.z_low - work[2] > 0.05
            || -0.05 > face.z_high - work[2];
        if outside {
            blob.work_position = saved;
            geometry.collide(blob);
            if blob.attached == 0 {
                blob.axis = [0.0, 1.0, 0.0];
            } else {
                let face = blob.face;
                let work = &mut blob.work_position;
                if face.x_low > work[0] {
                    work[0] = face.x_low;
                } else if work[0] > face.x_high {
                    work[0] = face.x_high;
                }
                if face.y_low > work[1] {
                    work[1] = face.y_low;
                } else if work[1] > face.y_high {
                    work[1] = face.y_high;
                }
                if face.z_low > work[2] {
                    work[2] = face.z_low;
                    geometry.slide_on_face(blob);
                } else {
                    if work[2] > face.z_high {
                        work[2] = face.z_high;
                    }
                    geometry.slide_on_face(blob);
                }
            }
        }
    } else {
        let room = BLAST_ROOM_IDS
            .iter()
            .position(|&id| id as i32 == blob.room)
            .and_then(|n| input.rooms[n]);
        let area = room.unwrap_or_default();
        blob.work_position[1] = area.ground_y;
        geometry.find_hit_and_reflect(blob, previous_position);
        if blob.attached == 0 {
            let position = blob.position;
            if area.min_x > position[0]
                || position[0] > area.max_x
                || area.min_z > position[2]
                || position[2] > area.max_z
            {
                geometry.attach_to_nearest_face(blob);
            }
        }
    }
    blob.position = blob.work_position;
    blob.direction = blob.work_direction;
    let slot = blob.ring_index.max(0) as usize;
    if slot < blob.ring.len() {
        blob.ring[slot] = blob.position;
    }
    let length = (rate * 2).max(1);
    blob.ring_index = (blob.ring_index + 1) % length;
}

// ------------------------------------------------ 0x369940 blast pieces (T1845 round 4)

/// A local player as the pieces see it.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerSpot {
    /// `+0xB4..+0xBC` of the player block.
    pub position: Vec3,
    /// `+0x14`: the player's object.
    pub object: u32,
}

/// What `0x369940` reads besides the state.
pub struct BlastPiecesInput<'a> {
    pub timing: Timing,
    /// The OR of the seven room visibility tables at `0x5476C4..0x5476DC` for the room `[0x7B977C]`.
    pub room_visible: bool,
    /// Position (`ext + 0xBB4..`) of the object with tag "DOOF" (`0x15BA90(tag, 3)`), `None` when absent.
    pub doof: Option<Vec3>,
    pub players: &'a [PlayerSpot],
    /// Room box per room id of [`BLAST_ROOM_IDS`].
    pub rooms: &'a [Option<RoomBox>; 7],
    /// `[0x4E794C]`: the float frame multiplier (status 2 countdown).
    pub step_float: f32,
    /// The uninitialised stack words `0x369940` reads when a piece launches with neither the "DOOF" object nor a player.
    pub stale_offset: Vec3,
}

/// Callback the image installs on the seven room objects every frame.
pub const BLAST_ROOM_CALLBACK: u32 = 0x366440;

/// `0x36B7A0` as one call (T1846 batch 8): `blast_frame` (which runs the ambience `0x3697C0` first), then, when the blast
/// is still active and its room is visible, the blob update `0x368A10` and the piece update `0x369940` (the image calls
/// them at `0x36BA97` and `0x36BA9C`, after the spawn loop; the early exits of `blast_frame` skip them). One rng
/// (`0x153B40`) serves all three, in call order.
pub fn blast_step(
    state: &mut BlastState,
    frame: &BlastInput,
    update: &BlastUpdateInput,
    pieces: &BlastPiecesInput,
    geometry: &mut dyn BlastGeometry,
    rng: &mut dyn FnMut() -> u32,
) -> Vec<SceneEffect> {
    let mut out = blast_frame(state, frame, rng);
    if state.active && frame.room_visible {
        blast_update(state, update, geometry, rng);
        out.extend(blast_pieces(state, pieces, geometry, rng));
    }
    out
}

/// `0x369940` (MEASURED control flow, per frame). Ported: the gates, the "DOOF" bookkeeping, the per blob running sums, the
/// ballistic phase of pieces with status 1 (gravity 9.8, spin, damage 0.01 to players within 1.0, the room box exit that
/// lands or attaches the piece), the tail that counts pieces beyond the pool and computes the blob centroids, and the room
/// object marks, and (T1846 batch 8) the motion of pieces with status 0 or 2 (`0x369F06..0x36B2E1`) in
/// [`super::piece_follow::follow_piece`].
pub fn blast_pieces(
    state: &mut BlastState,
    input: &BlastPiecesInput,
    geometry: &mut dyn BlastGeometry,
    rng: &mut dyn FnMut() -> u32,
) -> Vec<SceneEffect> {
    let mut out = Vec::new();
    let steps = input.timing.steps;
    if steps == 0 || !state.active || !input.room_visible {
        return out;
    }
    let rate = input.timing.rate;
    match input.doof {
        Some(position) => {
            state.doof_countdown = rate * 3;
            state.doof_position = position;
        }
        None => {
            if state.doof_countdown > 0 {
                state.doof_countdown -= steps;
            }
        }
    }
    for blob in state.blobs.iter_mut() {
        blob.sum = [0.0; 3];
        blob.count = 0;
    }
    let dt = (1.0 / rate as f32) * steps as f32;
    for index in 0..state.piece_count.clamp(0, BLAST_MAX_PIECES as i32) as usize {
        let (status, owner) = (
            state.pieces[index].state,
            state.pieces[index].player.min(BLAST_BODIES - 1),
        );
        let room = BLAST_ROOM_IDS
            .iter()
            .position(|&id| id as i32 == state.pieces[index].room)
            .and_then(|n| input.rooms[n]);
        let area = room.unwrap_or_default();
        if status != 3 {
            let position = state.pieces[index].position;
            let blob = &mut state.blobs[owner];
            for axis in 0..3 {
                blob.sum[axis] += position[axis];
            }
            blob.count += 1;
        }
        match status {
            0 | 2 => {
                super::piece_follow::follow_piece(state, index, input, geometry, rng, &mut out)
            }
            1 => {
                let piece = &mut state.pieces[index];
                piece.velocity[1] -= dt * 9.8;
                for axis in 0..3 {
                    piece.position[axis] += dt * piece.velocity[axis];
                }
                piece.spin += dt * PI;
                for player in input.players {
                    let d = [
                        piece.position[0] - player.position[0],
                        piece.position[1] - player.position[1],
                        piece.position[2] - player.position[2],
                    ];
                    if 1.0 > d[0] * d[0] + d[1] * d[1] + d[2] * d[2] {
                        out.push(SceneEffect::Damage {
                            victim: DamageVictim::Object(player.object),
                            amount: 0.01,
                            kind: 0,
                            point: piece.position,
                            direction: [0.0, -1.0, 0.0],
                        });
                    }
                }
                let minimum = [area.min_x, area.ground_y, area.min_z];
                let maximum = [area.max_x, area.max_y, area.max_z];
                for axis in 0..3 {
                    let position = state.pieces[index].position[axis];
                    if position > maximum[axis] && axis != 1 {
                        geometry.attach_piece(&mut state.pieces[index]);
                        state.pieces[index].state = 0;
                    } else if minimum[axis] > position {
                        if axis == 1 {
                            state.pieces[index].attached = 0;
                        } else {
                            geometry.attach_piece(&mut state.pieces[index]);
                        }
                        state.pieces[index].state = 0;
                    }
                }
            }
            _ => {}
        }
    }
    if (state.piece_count as usize) < BLAST_MAX_PIECES {
        for piece in &state.pieces[state.piece_count.max(0) as usize..BLAST_MAX_PIECES] {
            if piece.state != 3 {
                state.blobs[piece.player.min(BLAST_BODIES - 1)].count += 1;
            }
        }
    }
    for blob in state.blobs.iter_mut() {
        if blob.count > 0 {
            let inverse = 1.0 / blob.count as f32;
            for axis in 0..3 {
                blob.centroid[axis] = blob.sum[axis] * inverse;
            }
        }
    }
    for id in BLAST_ROOM_IDS {
        out.push(SceneEffect::MarkRoomObject {
            id,
            callback: BLAST_ROOM_CALLBACK,
        });
    }
    out
}
