//! Motion of the blast pieces with status 0 or 2 (`0x369940`, the range `0x369F06..0x36B2E1`), T1846 batch 8.
//!
//! Decoded from `build/default.xbe` with `tools/x87_blocks.py` and verified against the original running under Unicorn
//! (`tests/story_events_t1846_pieces.rs`, golden vectors `tests/data/t1846/pieces_369940.golden`). The x87 sequences are
//! emulated with f64 (the title runs with control word `0x27F`, 53 bit mantissa) and rounded once at each `fstp`; the SSE
//! parts are plain f32 operations in the order of the image. Names are INFERRED, control flow and arithmetic MEASURED.
//!
//! One status 0 or 2 piece per frame:
//! 1. The face normal `n` of its attached face (`(0, 1, 0)` when free), the position `r` the owning blob's ring had
//!    `frames_back` frames ago and the direction `d = r - position`.
//! 2. The angle of the velocity and of `d` in the plane spanned by the up axis and the face tangent `up x n`
//!    (`atan2(dx, dz)` for a free piece). `steps` times the velocity angle is turned toward the target angle by a random
//!    amount (`0x153B40`) whose size shrinks with the ring generation `frames_back / (2 * rate)`.
//! 3. The velocity is rotated about `n` by the accumulated turn (`0x152770` and `0x152090`), projected onto the face plane,
//!    re-normalised when its length is off by more than 0.1, and the position advances by
//!    `steps / rate * speed_factor * 2.7 * velocity`.
//! 4. Attached: the inlined `0x3675E0` slides the piece on its face; leaving the face rectangle (tolerance 0.05) restores
//!    the position and calls `0x368280`. Free: `y = 12.0` and a position outside the room box calls `0x367CE0`.
//! 5. The launch timer counts down while a player is within 9.0 (squared 81.0) of the piece (or of the "DOOF" object), or
//!    always in phase 2. At 0 a status 0 piece becomes status 1: it is thrown toward the "DOOF" object or the nearest
//!    player with a random speed, leaves a decal and gets a random timer. A status 2 piece updates its object and, when its
//!    countdown ends, spawns the decal, particle and sound and becomes status 3.

use std::f32::consts::{FRAC_PI_2, PI};

use super::callees_t1846::rescale_vector;
use super::hooks_scene::{
    BLAST_BODIES, BLAST_ROOM_IDS, BlastGeometry, BlastPiece, BlastPiecesInput, BlastState,
    ParticleAnchor, SceneEffect, Vec3,
};
use super::pose::{Mat4, rotation_axis_angle, transform_direction};

/// `[0x4785D0]`: 2^-32, turns a 32 bit random draw into a fraction.
const TWO_NEG_32: f64 = 2.328_306_436_538_696_3e-10;
/// `[0x475CD4]`.
const HALF: f64 = 0.5;
/// `[0x547790]`: random turn spread.
const TURN_SPREAD: f64 = 80.0;
/// `[0x475DC0]`: per generation turn offset factor.
const GENERATION_FACTOR: f32 = 16.0;
/// `[0x5476B0]`: speed constant of the follow motion.
const FOLLOW_SPEED: f32 = 2.7;
/// `[0x5476B8]` and `[0x475DB4]`: launch speed multiplier and timer factor.
const LAUNCH_SPEED: f64 = 4.0;
const LAUNCH_TIMER_FACTOR: f64 = 4.0;
/// `[0x47D010]`: height a free piece is held at.
const FREE_HEIGHT: f32 = 12.0;
/// `[0x49F2EC]`: squared distance at which a player makes the timer run.
const NEAR_PLAYER_SQUARED: f32 = 81.0;
/// `[0x475CE8]` / `[0x4783DC]`: face rectangle tolerance.
const FACE_TOLERANCE: f32 = 0.05;
/// `[0x4765F4]`.
const UNIT_EPSILON: f32 = f32::from_bits(0x33D6_BF95);
const MAX_BEST: f32 = f32::MAX;

fn unsigned(draw: u32) -> f64 {
    // `fild` of the signed draw, plus 2^32 when negative: the unsigned value.
    f64::from(draw)
}

/// `fpatan` on f32 operands, result rounded to f32 at the store.
fn atan2_f32(y: f64, x: f64) -> f32 {
    y.atan2(x) as f32
}

/// The velocity and target angles of step 2 for a piece with normal `n`.
fn follow_angles(n: Vec3, delta: Vec3, velocity: Vec3) -> (f32, f32) {
    if n[1] == 1.0 {
        let target = atan2_f32(f64::from(delta[0]), f64::from(delta[2]));
        let current = atan2_f32(f64::from(velocity[0]), f64::from(velocity[2]));
        return (target, current);
    }
    let up_term = n[1] * 0.0;
    let tangent_x = n[2] - up_term;
    let tangent_z = up_term - n[0];
    let tangent_y = f64::from(n[0]) * 0.0 - f64::from(n[2]) * 0.0;
    let (dx, dy, dz) = (
        f64::from(delta[0]),
        f64::from(delta[1]),
        f64::from(delta[2]),
    );
    let (tx, tz) = (f64::from(tangent_x), f64::from(tangent_z));
    let height = (dz * 0.0 + dx * 0.0) + dy;
    let along = (tz * dz + tangent_y * dy) + tx * dx;
    let target = atan2_f32(height, along);
    let (vx, vy, vz) = (
        f64::from(velocity[0]),
        f64::from(velocity[1]),
        f64::from(velocity[2]),
    );
    let velocity_height = (vx * 0.0 + vz * 0.0) + vy;
    let velocity_along = (tangent_y * vy + tx * vx) + tz * vz;
    let current = atan2_f32(velocity_height, velocity_along);
    (target, current)
}

/// One random turn of step 2, `0x36A202`/`0x36A23F`: positive when `current < target`.
fn random_turn(draw: u32, base: f32, rate: i32, positive: bool) -> f32 {
    let value = ((unsigned(draw) * TWO_NEG_32 - HALF) * TURN_SPREAD + f64::from(base))
        * (1.0 / f64::from(rate));
    (if positive { value } else { -value }) as f32
}

/// Whether the piece left its face rectangle (tolerance 0.05): the test after the slide.
fn left_face(piece: &BlastPiece) -> bool {
    let (face, p) = (piece.face, piece.position);
    face.x_low - p[0] > FACE_TOLERANCE
        || -FACE_TOLERANCE > face.x_high - p[0]
        || face.y_low > p[1]
        || p[1] > face.y_high
        || face.z_low - p[2] > FACE_TOLERANCE
        || -FACE_TOLERANCE > face.z_high - p[2]
}

/// The unit vector code at `0x36AFFE`, shared by the launch: scale by `1 / sqrt(len2)` (QEMU `rsqrtss`).
fn normalise_launch(velocity: &mut Vec3) {
    let len2 = (velocity[1] * velocity[1] + velocity[2] * velocity[2]) + velocity[0] * velocity[0];
    if len2 > UNIT_EPSILON {
        let scale = (1.0 / len2.sqrt()) * 1.0;
        velocity[0] *= scale;
        velocity[1] *= scale;
        velocity[2] *= scale;
    }
}

/// The motion of piece `index` (status 0 or 2) for one frame; effects in the order of the image.
pub fn follow_piece(
    state: &mut BlastState,
    index: usize,
    input: &BlastPiecesInput,
    geometry: &mut dyn BlastGeometry,
    rng: &mut dyn FnMut() -> u32,
    out: &mut Vec<SceneEffect>,
) {
    let steps = input.timing.steps;
    let rate = input.timing.rate;
    let mut piece = state.pieces[index];
    let owner = piece.player.min(BLAST_BODIES - 1);

    // 1. Normal, ring target.
    let mut normal: Vec3 = [0.0, 1.0, 0.0];
    if piece.attached != 0 {
        let f = piece.face;
        let (e0, e2, e1) = (f.x1 - f.x0, f.z1 - f.z0, f.y_high - f.y_low);
        normal = [e2 * e1, e0 * 0.0 - e2 * 0.0, 0.0 - e0 * e1];
        rescale_vector(&mut normal, [0.0, e1, 0.0], [e0, 0.0, e2]);
    }
    let blob = &state.blobs[owner];
    let mut slot = blob.ring_index - piece.frames_back;
    if slot < 0 {
        slot += rate * 2;
    }
    let ring_position = blob
        .ring
        .get(slot.max(0) as usize)
        .copied()
        .unwrap_or([0.0; 3]);
    let delta = [
        ring_position[0] - piece.position[0],
        ring_position[1] - piece.position[1],
        ring_position[2] - piece.position[2],
    ];
    let speed_factor = blob.speed_factor;

    // 2. Turn the velocity angle toward the target.
    let (target, mut current) = follow_angles(normal, delta, piece.velocity);
    let generation = if rate > 0 {
        piece.frames_back / (rate * 2)
    } else {
        0
    };
    let base = 0.0 - ((generation as f32 + 1.0) * GENERATION_FACTOR);
    let mut accumulated = 0.0f32;
    for _ in 0..steps.max(0) {
        let mut difference = current - target;
        while -PI >= difference {
            difference += 2.0 * PI;
        }
        while difference > PI {
            difference -= 2.0 * PI;
        }
        let turn = random_turn(rng(), base, rate, 0.0 > difference);
        accumulated = turn + accumulated;
        current = turn + current;
    }

    // 3. Rotate, project, renormalise, advance.
    let matrix = rotation_axis_angle(normal, accumulated);
    piece.velocity = transform_direction(&matrix, piece.velocity);
    let along = (piece.velocity[1] * normal[1] + piece.velocity[0] * normal[0])
        + normal[2] * piece.velocity[2];
    let removed = 0.0 - along;
    piece.velocity[0] = removed * normal[0] + piece.velocity[0];
    piece.velocity[1] = removed * normal[1] + piece.velocity[1];
    piece.velocity[2] = removed * normal[2] + piece.velocity[2];
    let len2 = (piece.velocity[2] * piece.velocity[2] + piece.velocity[1] * piece.velocity[1])
        + piece.velocity[0] * piece.velocity[0];
    if (len2 - 1.0).abs() > 0.1 && len2 > UNIT_EPSILON {
        let scale = (1.0 / len2.sqrt()) * 1.0;
        piece.velocity[0] *= scale;
        piece.velocity[1] *= scale;
        piece.velocity[2] *= scale;
    }
    let advance = ((1.0 / rate as f32) * steps as f32 * speed_factor) * FOLLOW_SPEED;
    piece.position[0] = advance * piece.velocity[0] + piece.position[0];
    piece.position[1] = advance * piece.velocity[1] + piece.position[1];
    piece.position[2] = advance * piece.velocity[2] + piece.position[2];

    // 4. Face following or free flight.
    if piece.attached != 0 {
        let saved = piece.position;
        geometry.slide_piece(&mut piece);
        if left_face(&piece) {
            piece.position = saved;
            geometry.collide_piece(&mut piece);
            if piece.attached != 0 {
                let face = piece.face;
                let p = &mut piece.position;
                if face.x_low > p[0] {
                    p[0] = face.x_low;
                } else if p[0] > face.x_high {
                    p[0] = face.x_high;
                }
                if face.y_low > p[1] {
                    p[1] = face.y_low;
                } else if p[1] > face.y_high {
                    p[1] = face.y_high;
                }
                if face.z_low > p[2] {
                    p[2] = face.z_low;
                } else if p[2] > face.z_high {
                    p[2] = face.z_high;
                }
                geometry.slide_piece(&mut piece);
            }
        }
    } else {
        let area = BLAST_ROOM_IDS
            .iter()
            .position(|&id| id as i32 == piece.room)
            .and_then(|n| input.rooms[n])
            .unwrap_or_default();
        piece.position[1] = FREE_HEIGHT;
        let p = piece.position;
        if area.min_x > p[0] || p[0] > area.max_x || area.min_z > p[2] || p[2] > area.max_z {
            geometry.attach_piece(&mut piece);
        }
    }

    // 5. Timer.
    let doof_present = input.doof.is_some();
    match state.phase {
        0 => {
            for player in input.players {
                let target = if doof_present {
                    state.doof_position
                } else {
                    player.position
                };
                let d = [
                    target[0] - piece.position[0],
                    target[1] - piece.position[1],
                    target[2] - piece.position[2],
                ];
                let squared = if doof_present {
                    (d[0] * d[0] + d[1] * d[1]) + d[2] * d[2]
                } else {
                    (d[0] * d[0] + d[2] * d[2]) + d[1] * d[1]
                };
                if NEAR_PLAYER_SQUARED > squared {
                    piece.timer -= steps;
                    break;
                }
            }
        }
        2 => piece.timer -= steps,
        _ => {}
    }
    if piece.timer <= 0 && piece.state == 0 {
        piece.state = 1;
        launch(state, input, &mut piece, index, doof_present, rng, out);
    } else if piece.state == 2 {
        expire(state, input, &mut piece, index, rng, out);
    }
    state.pieces[index] = piece;
}

/// `0x36AC1F..0x36B137`: the status 0 piece's timer ran out.
fn launch(
    state: &BlastState,
    input: &BlastPiecesInput,
    piece: &mut BlastPiece,
    index: usize,
    doof_present: bool,
    rng: &mut dyn FnMut() -> u32,
    out: &mut Vec<SceneEffect>,
) {
    let position = piece.position;
    let offset = if doof_present || state.doof_countdown > 0 {
        [
            state.doof_position[0] - position[0],
            state.doof_position[1] - position[1],
            state.doof_position[2] - position[2],
        ]
    } else {
        let mut best = MAX_BEST;
        let mut offset = input.stale_offset;
        for player in input.players {
            let d = [
                player.position[0] - position[0],
                player.position[1] - position[1],
                player.position[2] - position[2],
            ];
            let squared = (d[1] * d[1] + d[0] * d[0]) + d[2] * d[2];
            if best > squared {
                best = squared;
                offset = d;
            }
        }
        offset
    };
    let v = piece.velocity;
    out.push(SceneEffect::Decal {
        kind: 0x2E4,
        flags: 0x7F7F_7F7F,
        scale: f32::from_bits(0x3E4C_CCCD),
        room: piece.room,
        piece: index,
        offset: [0.0 - v[0], 0.0 - v[1], 0.0 - v[2]],
    });
    piece.velocity = [offset[0] + v[0], (v[1] + offset[1]) + 1.0, offset[2] + v[2]];
    normalise_launch(&mut piece.velocity);
    let scale = ((unsigned(rng()) * TWO_NEG_32 + 1.0) * LAUNCH_SPEED) as f32;
    let launched = [
        scale * piece.velocity[0],
        piece.velocity[1] * scale,
        scale * piece.velocity[2],
    ];
    piece.velocity = launched;
    let (vx, vy, vz) = (
        f64::from(launched[0]),
        f64::from(launched[1]),
        f64::from(launched[2]),
    );
    piece.heading_a = 0.0;
    let horizontal = (vz * vz + vx * vx).sqrt();
    piece.spin = -atan2_f32(vy, horizontal);
    piece.heading_b = (vx.atan2(vz) + f64::from(FRAC_PI_2)) as f32;
    let timer = (unsigned(rng()) * TWO_NEG_32 + HALF)
        * (f64::from(input.timing.rate) * LAUNCH_TIMER_FACTOR);
    // `0x3C848C` is the truncating float to int conversion.
    piece.timer = timer as i32;
}

/// `0x36B13C..0x36B2D1`: a status 2 piece keeps its object attached and, when the countdown ends, dies in effects.
fn expire(
    state: &BlastState,
    input: &BlastPiecesInput,
    piece: &mut BlastPiece,
    index: usize,
    rng: &mut dyn FnMut() -> u32,
    out: &mut Vec<SceneEffect>,
) {
    let _ = state;
    piece.countdown -= (1.0 / input.timing.rate as f32) * input.step_float;
    out.push(SceneEffect::ObjectVector {
        object: piece.object,
        position: piece.position,
    });
    if 0.0 > piece.countdown {
        let matrix = euler_matrix(piece.heading_a, piece.heading_b, piece.spin);
        let up = transform_direction(&matrix, [0.0, 1.0, 0.0]);
        out.push(SceneEffect::Decal {
            kind: 0x33A,
            flags: 0x5F00_0020,
            scale: f32::from_bits(0x3DCC_CCCD),
            room: piece.room,
            piece: index,
            offset: [0.0 - up[0], 0.0 - up[1], 0.0 - up[2]],
        });
        out.push(SceneEffect::Particle {
            template: 0x3500,
            anchor: ParticleAnchor::Piece(index),
            vector: up,
        });
        let id = 0x156 + (rng() & 3);
        out.push(SceneEffect::Sound {
            id,
            position: piece.position,
        });
        piece.state = 3;
    }
}

/// `0x151650(out, a, b, c)`: the rotation matrix of three angles (sin and cos from the x87 unit, rounded to f32 at the
/// store; products in SSE order). Row-major 4x4, translation row `(0, 0, 0, 1)` (MEASURED).
pub fn euler_matrix(a: f32, b: f32, c: f32) -> Mat4 {
    let sin = |angle: f32| f64::from(angle).sin() as f32;
    let cos = |angle: f32| f64::from(angle).cos() as f32;
    let (sin_a, cos_a, sin_b, cos_b, sin_c, cos_c) =
        (sin(a), cos(a), sin(b), cos(b), sin(c), cos(c));
    let sb_sa = sin_b * sin_a;
    let cb_sa = cos_b * sin_a;
    let mut m = [0.0f32; 16];
    m[0] = sb_sa * sin_c + cos_c * cos_b;
    m[1] = sin_c * cos_a;
    m[2] = (0.0 - cos_c * sin_b) + cb_sa * sin_c;
    m[4] = (0.0 - sin_c * cos_b) + sb_sa * cos_c;
    m[5] = cos_c * cos_a;
    m[6] = cb_sa * cos_c + sin_c * sin_b;
    m[8] = sin_b * cos_a;
    m[9] = 0.0 - sin_a;
    m[10] = cos_b * cos_a;
    m[15] = 1.0;
    m
}
