// SPDX-License-Identifier: GPL-3.0-or-later
//! Float motion helpers of the mode camera and steering code (T1910). Single-precision SSE and
//! x87 sequences of the original are written as plain `f32` arithmetic: each x87 operation rounds
//! to 24 bits (control word 0x0000), which is exactly what `f32` does for + - * /.
use super::{Esp, ModeHost, cvttss2si, read_f32, x87_round24 as fp24};

const SINE_TABLE: u32 = 0x007a_38a0;
const OSCILLATOR_STATE_PTR: u32 = 0x0078_b564;
const OSCILLATOR_PHASE: u32 = 0x0078_b570;
const OSCILLATOR_VALUE: u32 = 0x0078_b574;
const OSCILLATOR_FOLLOWER: u32 = 0x0078_b578;
const STATE_AMPLITUDE_A: u32 = 0x14;
const STATE_AMPLITUDE_B: u32 = 0x2c;
const PHASE_SCALE: u32 = 0x0047_d740;
const PHASE_UNIT: u32 = 0x0047_5ce0;
const PHASE_BIAS_AND_FOLLOW_STEP: u32 = 0x0047_5cd4;
const PREVIOUS_WEIGHT: u32 = 0x0047_d56c;
const SAMPLE_SCALE: u32 = 0x0047_cff4;
const FOLLOW_TOLERANCE: u32 = 0x0047_5cc8;
const QUARTER_TABLE_TURN: i32 = 0x200;
const TABLE_INDEX_MASK: i32 = 0x7ff;

/// 00253020 game_mode_sine_table_oscillator_step_with_eased_follower_value. cdecl(0). Advances the
/// phase counter `[0x78B570]`, evaluates the difference of two lookups of the 2048-entry sine
/// table at 0x7A38A0 scaled by the larger of two amplitudes and blended with the previous value
/// `[0x78B574]`, stores the new value, and eases the follower `[0x78B578]` by a fixed step when
/// the follower is further than a tolerance from the new value.
pub fn sine_oscillator_step<H: ModeHost>(h: &mut H) {
    let state = h.read32(OSCILLATOR_STATE_PTR);
    let (amplitude_a, amplitude_b) = (
        read_f32(h, state.wrapping_add(STATE_AMPLITUDE_A)),
        read_f32(h, state.wrapping_add(STATE_AMPLITUDE_B)),
    );
    let amplitude = if amplitude_a > amplitude_b {
        amplitude_a
    } else {
        amplitude_b
    };
    let phase = h.read32(OSCILLATOR_PHASE);
    let (scale, unit, bias) = (
        read_f32(h, PHASE_SCALE),
        read_f32(h, PHASE_UNIT),
        read_f32(h, PHASE_BIAS_AND_FOLLOW_STEP),
    );
    let previous = read_f32(h, OSCILLATOR_VALUE) * read_f32(h, PREVIOUS_WEIGHT);
    let next_phase = phase.wrapping_add(1);
    let table_index = |counter: u32| {
        let angle = (counter as i32 as f32) * scale * unit + bias;
        (cvttss2si(angle).wrapping_add(QUARTER_TABLE_TURN) & TABLE_INDEX_MASK) as u32
    };
    let ahead = read_f32(h, SINE_TABLE + 4 * table_index(next_phase));
    let behind = read_f32(h, SINE_TABLE + 4 * table_index(phase));
    let value = (ahead - behind) * amplitude * read_f32(h, SAMPLE_SCALE) + previous;
    let follower = read_f32(h, OSCILLATOR_FOLLOWER);
    let tolerance = read_f32(h, FOLLOW_TOLERANCE);
    let step = bias;
    let distance = (follower - value).abs();
    h.write32(OSCILLATOR_VALUE, value.to_bits());
    h.write32(OSCILLATOR_PHASE, next_phase);
    if tolerance > distance {
        h.write32(OSCILLATOR_FOLLOWER, value.to_bits());
    } else if value > follower {
        h.write32(OSCILLATOR_FOLLOWER, (follower + step).to_bits());
    } else {
        h.write32(OSCILLATOR_FOLLOWER, (follower - step).to_bits());
    }
}

// Guest layout of the mode rotation records. These are positional contracts from the port;
// the original record types are not yet identified by tools.structs.
const MODE_ENTRY_LIST_PTR: u32 = 0x0078_b560;
const MODE_FOLLOWER_ANGLE: u32 = 0x0078_b578;
const ANGLE_TO_RADIANS: u32 = 0x0047_5d0c;
const OBJECT_NODE_LIST: u32 = 4;
const NODE_RECORD_LIST: u32 = 4;
const NODE_MATRIX_TABLE: u32 = 0x10;
const RECORD_STRIDE: u32 = 0xc8;
const RECORD_POSITION_PTR: u32 = 0x2d0;
const ENTRY_STRIDE: u32 = 0x4c;
const ENTRY_BYTE_SPAN: u32 = 0x17c;
const ENTRY_FIRST_MATRIX_INDEX: u32 = 0x24;
const ENTRY_SECOND_MATRIX_INDEX: u32 = 0x28;
const MATRIX_INDEX_SHIFT: u32 = 6;
const OBJECT_MATRIX_OFFSETS: [(u32, u32); 3] = [(0x3c0, 0x20), (0x440, 0x28), (0x140, 0x30)];

/// 00252ED0 game_mode_78b560_apply_rotation_x_78b578_and_entry_matrices_to_object_matrix_table.
/// Register input EDI = mode object. Builds a translation matrix from the first node position of
/// the object's current record, a rotation by the follower angle `[0x78B578] * [0x475D0C]`,
/// multiplies them, builds the inverse translation, and applies the combined transform to three
/// matrices of the object's matrix table (+0x3C0, +0x440, +0x140) and to the per-entry matrices
/// of the 0x4C-byte entries at `[0x78B560]` (two matrices each, 5 entries). The matrix library
/// calls (153550 translation, 1533E0 rotation, 153650 multiply) write their results into locals
/// of this function's 0x80-byte frame, so the addresses passed to them are frame addresses built
/// with [`Esp`] exactly as the original's `sub`/`push` sequence does.
pub fn apply_follower_rotation_to_matrices<H: ModeHost>(h: &mut H, object: u32) {
    let mut esp = Esp::at_entry(h);
    let node_list = h.read32(object.wrapping_add(OBJECT_NODE_LIST));
    let record_list = h.read32(node_list.wrapping_add(NODE_RECORD_LIST));
    let count = h.read32(record_list);
    let record = record_list.wrapping_sub(count.wrapping_mul(RECORD_STRIDE));
    let position = h.read32(record.wrapping_add(RECORD_POSITION_PTR));
    esp.sub(0x80);
    esp.push(); // saved EBX
    esp.push(); // saved ESI
    let frame = esp.0;
    let (x, y, z) = (
        h.read32(position),
        h.read32(position.wrapping_add(4)),
        h.read32(position.wrapping_add(8)),
    );
    esp.push();
    esp.push();
    esp.push();
    let translation = esp.at(0x14);
    esp.push();
    h.call(MATRIX_TRANSLATION, &[translation, x, y, z]);
    let angle = read_f32(h, MODE_FOLLOWER_ANGLE) * read_f32(h, ANGLE_TO_RADIANS);
    esp.add(0xc);
    let rotation = esp.at(0x4c);
    h.write32(esp.0, angle.to_bits());
    esp.push();
    h.call(MATRIX_ROTATION, &[rotation, angle.to_bits()]);
    let combined_in = esp.at(0x50);
    esp.push();
    let scratch = esp.at(0x14);
    esp.push();
    h.call(MATRIX_MULTIPLY, &[scratch, combined_in]);
    esp.add(4);
    let negated = [
        0.0f32 - read_f32(h, position),
        0.0f32 - read_f32(h, position.wrapping_add(4)),
        0.0f32 - read_f32(h, position.wrapping_add(8)),
    ];
    h.write32(esp.at(8), negated[2].to_bits());
    h.write32(esp.at(4), negated[1].to_bits());
    h.write32(esp.at(0), negated[0].to_bits());
    let inverse = esp.at(0x54);
    esp.push();
    h.call(
        MATRIX_TRANSLATION,
        &[
            inverse,
            negated[0].to_bits(),
            negated[1].to_bits(),
            negated[2].to_bits(),
        ],
    );
    let combined_in = esp.at(0x58);
    esp.push();
    let scratch = esp.at(0x1c);
    esp.push();
    h.call(MATRIX_MULTIPLY, &[scratch, combined_in]);
    for (table_offset, frame_offset) in OBJECT_MATRIX_OFFSETS {
        let list = h.read32(object.wrapping_add(OBJECT_NODE_LIST));
        let matrices = h.read32(list.wrapping_add(NODE_MATRIX_TABLE));
        let destination = esp.at(frame_offset);
        esp.push();
        let source = matrices.wrapping_add(table_offset);
        esp.push();
        h.call(MATRIX_MULTIPLY, &[source, destination]);
    }
    esp.add(0x30);
    debug_assert_eq!(esp.0, frame);
    let mut entry = 0u32;
    while (entry as i32) < ENTRY_BYTE_SPAN as i32 {
        let entries = h.read32(MODE_ENTRY_LIST_PTR);
        let first = h.read32(
            entry
                .wrapping_add(entries)
                .wrapping_add(ENTRY_FIRST_MATRIX_INDEX),
        );
        let record_ptr = entry.wrapping_add(entries);
        let list = h.read32(object.wrapping_add(OBJECT_NODE_LIST));
        let destination = esp.at(8);
        esp.push();
        let matrices = h.read32(list.wrapping_add(NODE_MATRIX_TABLE));
        let source = (first << MATRIX_INDEX_SHIFT).wrapping_add(matrices);
        esp.push();
        h.call(MATRIX_MULTIPLY, &[source, destination]);
        let second = h.read32(record_ptr.wrapping_add(ENTRY_SECOND_MATRIX_INDEX));
        let list = h.read32(object.wrapping_add(OBJECT_NODE_LIST));
        let destination = esp.at(0x10);
        esp.push();
        let matrices = h.read32(list.wrapping_add(NODE_MATRIX_TABLE));
        let source = (second << MATRIX_INDEX_SHIFT).wrapping_add(matrices);
        esp.push();
        h.call(MATRIX_MULTIPLY, &[source, destination]);
        entry = entry.wrapping_add(ENTRY_STRIDE);
        esp.add(0x10);
    }
}

const MATRIX_TRANSLATION: u32 = 0x0015_3550;
const MATRIX_ROTATION: u32 = 0x0015_33e0;
const MATRIX_MULTIPLY: u32 = 0x0015_3650;

/// 00298BA0 game_mode_51_hit_accumulate_tracked_source_spawn_pool_or_reflected_projectile.
/// cdecl(projectile). Adds the hit weight `[0x4785E4]` to the score of whichever of the two tracked
/// sources (`[world+0x5AC]` or `[world+0x5B4]`) fired the projectile (+4), then, unless the projectile
/// has expired (+4 equal to +0), either spawns a pool effect (296590 accepts the target: 298570)
/// or reflects the projectile off the surface normal at the target (+0x30): the incoming vector
/// `(0, 0, [0x4760D0])` is transformed by the world matrix at `[world]+0x649C` (152090, in place in
/// a stack local), the reflection `v - 2(n.v)n`-style result is built in stack locals and passed
/// with the projectile fields to the spawner EA9C0 (12 arguments). The locals the callees read and
/// write live in this function's 0x24-byte frame, addressed with [`Esp`].
pub fn projectile_hit<H: ModeHost>(h: &mut H) {
    let mut esp = Esp::at_entry(h);
    let projectile = h.read32(esp.at(4));
    let world = h.read32(0x0078_b1e8);
    let hit_weight = read_f32(h, 0x0047_85e4);
    esp.sub(0x24);
    esp.push();
    let first = h.read32(world.wrapping_add(0x5ac));
    let second = h.read32(world.wrapping_add(0x5b4));
    let owner = h.read32(projectile.wrapping_add(4));
    if first != 0 && owner == first {
        let score = read_f32(h, world.wrapping_add(0x5b0)) + hit_weight;
        h.write32(world.wrapping_add(0x5b0), score.to_bits());
    } else if second != 0 && owner == second {
        let score = read_f32(h, world.wrapping_add(0x5b8)) + hit_weight;
        h.write32(world.wrapping_add(0x5b8), score.to_bits());
    }
    if h.read32(projectile.wrapping_add(4)) == h.read32(projectile) {
        return;
    }
    let target = h.read32(projectile.wrapping_add(0x28));
    let normal_ptr = h.read32(projectile.wrapping_add(0x30));
    let kind = h.read32(projectile.wrapping_add(0x18));
    if kind == 0x17 {
        if h.call(0x0029_6590, &[target, 0]) == 0 {
            h.call(0x0029_8570, &[projectile]);
        }
        return;
    }
    if h.call(0x0029_6590, &[target, normal_ptr]) == 0 {
        h.call(0x0029_8570, &[projectile]);
        return;
    }
    let matrix = h.read32(0x0078_b1e8).wrapping_add(0x649c);
    let vector = esp.at(4);
    esp.push();
    h.write32(esp.at(8), 0);
    h.write32(esp.at(0xc), 0);
    let depth = read_f32(h, 0x0047_60d0);
    esp.push();
    h.write32(esp.at(0x14), depth.to_bits());
    h.call(0x0015_2090, &[matrix, vector]);
    let normal = h.read32(projectile.wrapping_add(0x30));
    let (nx, ny, nz) = (
        read_f32(h, normal),
        read_f32(h, normal.wrapping_add(4)),
        read_f32(h, normal.wrapping_add(8)),
    );
    let (vx, vy, vz) = (
        read_f32(h, esp.at(0xc)),
        read_f32(h, esp.at(0x10)),
        read_f32(h, esp.at(0x14)),
    );
    let seed = h.read32(projectile.wrapping_add(0x3c));
    let owner = h.read32(projectile.wrapping_add(4));
    let dot = (nz * vz + nx * vx + vy * ny) * read_f32(h, 0x0047_5d10);
    let rx = dot * vx - nx;
    h.write32(esp.at(0x18), rx.to_bits());
    let ry = dot * vy + ny;
    h.write32(esp.at(0x1c), ry.to_bits());
    let rz = dot * vz - nz;
    let origin = h.read32(projectile.wrapping_add(0x28));
    h.write32(esp.at(0x20), rz.to_bits());
    let px = rx + read_f32(h, origin);
    esp.push(); // 0.1f
    h.write32(esp.at(0x28), px.to_bits());
    let py = read_f32(h, origin.wrapping_add(4)) + ry;
    h.write32(esp.at(0x2c), py.to_bits());
    let pz = read_f32(h, origin.wrapping_add(8));
    let kind_word = h.read32(projectile.wrapping_add(0x24));
    esp.push();
    esp.push();
    esp.push();
    esp.push();
    esp.push();
    esp.push();
    esp.push();
    let owner_kind = h.read32(projectile.wrapping_add(0x18));
    esp.push(); // 2.0f
    let direction = esp.at(0x3c);
    esp.push();
    let position = esp.at(0x4c);
    esp.push();
    let pz = pz + rz;
    esp.push();
    h.write32(esp.at(0x5c), pz.to_bits());
    h.call(
        0x000e_a9c0,
        &[
            owner_kind,
            position,
            direction,
            0x4000_0000,
            owner,
            0,
            6,
            0,
            1,
            seed,
            kind_word,
            0x3dcc_cccd,
        ],
    );
}

/// Clamps `value` into `[low, high]` with the original's compare order (a NaN stays unchanged).
fn clamp_keep_nan(value: f32, low: f32, high: f32) -> f32 {
    if value > high {
        high
    } else if low > value {
        low
    } else {
        value
    }
}

/// 00254690 game_mode_pick_target_position_vector_by_mode_or_best_of_10_random_circle_points_in_box_table.
/// Register input EDI = mode camera object. Writes the target position `[0x78B580..0x78B588]` from
/// the mode `[0x78B594]`: 6 copies the track point at `[0x78B5B0]+0x64`, 8 uses fixed constants,
/// 7 uses the entry matrix at `[0x78B560]`, any other mode with a track point tries up to ten random
/// points on a circle (radius `[0x4788FC]`, angle from the RNG 153B40, sin/cos computed in x87
/// double precision) around the clamped centre, keeps the candidate that 251CD0 accepts and that is
/// farthest from the object (+0x64/+0x6C), and stops as soon as the best distance reaches
/// `[0x47862C]`. The candidate vector lives in the frame (251CD0 reads it through ECX = local+0x20).
pub fn pick_target_position<H: ModeHost>(h: &mut H, object: u32) {
    const TARGET: u32 = 0x0078_b580;
    let mode = h.read32(0x0078_b594);
    let track = h.read32(0x0078_b5b0);
    if mode == 6 {
        for k in 0..3 {
            let value = h.read32(track.wrapping_add(0x64 + 4 * k));
            h.write32(TARGET + 4 * k, value);
        }
        return;
    }
    if mode == 8 {
        h.write32(TARGET, h.read32(0x0049_f118));
        h.write32(TARGET + 4, h.read32(0x0049_e0fc));
        h.write32(TARGET + 8, h.read32(0x0049_e058));
        return;
    }
    if mode == 7 {
        let entry = h.read32(0x0078_b560);
        let x = h.read32(entry.wrapping_add(0x140));
        let z = read_f32(h, entry.wrapping_add(0x148)) - read_f32(h, 0x0047_d208);
        h.write32(TARGET, x);
        h.write32(TARGET + 4, h.read32(0x0047_d560));
        h.write32(TARGET + 8, z.to_bits());
        return;
    }
    if track == 0 {
        return;
    }
    let mut esp = Esp::at_entry(h);
    esp.sub(0x28);
    esp.push(); // saved ESI
    let frame = esp.0;
    let centre_x = clamp_keep_nan(
        read_f32(h, track.wrapping_add(0x64)),
        read_f32(h, 0x0049_f114),
        read_f32(h, 0x0047_60dc),
    );
    let centre_z = clamp_keep_nan(
        read_f32(h, track.wrapping_add(0x6c)),
        read_f32(h, 0x0047_d6e0),
        read_f32(h, 0x0049_f110),
    );
    let (mut best_distance, mut best_angle) = (0.0f32, 0.0f32);
    let radius = read_f32(h, 0x0047_88fc);
    let mut tries = 0u32;
    while (tries as i32) < 0x0a {
        let random = h.call(0x0015_3b40, &[]);
        let mut angle = f64::from(random as i32);
        if (random as i32) < 0 {
            angle = fp24(angle + f64::from(read_f32(h, 0x0047_5ccc)));
        }
        angle = fp24(angle * f64::from(read_f32(h, 0x0047_85d0)));
        let height = read_f32(h, object.wrapping_add(0x68));
        h.write32(frame.wrapping_add(0x24), height.to_bits());
        angle = fp24(angle * f64::from(read_f32(h, 0x0047_5c94)));
        let angle32 = angle as f32;
        let x = fp24(fp24(angle.sin() * f64::from(radius)) + f64::from(centre_x)) as f32;
        let z =
            fp24(fp24(f64::from(angle32).cos() * f64::from(radius)) + f64::from(centre_z)) as f32;
        h.write32(frame.wrapping_add(0x20), x.to_bits());
        h.write32(frame.wrapping_add(0x28), z.to_bits());
        if h.call(0x0025_1cd0, &[]) != 0 {
            let dz = fp24(f64::from(z) - f64::from(read_f32(h, object.wrapping_add(0x6c))));
            let dx = fp24(f64::from(x) - f64::from(read_f32(h, object.wrapping_add(0x64))));
            let sum = fp24(fp24(dx * dx) + fp24(dz * dz));
            let distance = fp24(sum.sqrt()) as f32;
            if distance > best_distance {
                best_distance = distance;
                best_angle = angle32;
            }
            tries += 1;
        }
        if !(read_f32(h, 0x0047_862c) > best_distance) {
            break;
        }
    }
    let height = read_f32(h, object.wrapping_add(0x68));
    let sine = fp24(f64::from(best_angle).sin() * f64::from(radius));
    let sine = fp24(sine + f64::from(centre_x));
    h.write32(TARGET + 4, height.to_bits());
    let cosine = fp24(fp24(f64::from(best_angle).cos() * f64::from(radius)) + f64::from(centre_z));
    h.write32(TARGET, (sine as f32).to_bits());
    h.write32(TARGET + 8, (cosine as f32).to_bits());
}

/// Writes the two steering controls (`+0x10` and `+0x28` of the control block `[0x78B564]`).
fn set_controls<H: ModeHost>(h: &mut H, first: u32, second: u32) {
    let controls = h.read32(0x0078_b564);
    h.write32(controls.wrapping_add(0x10), first);
    let controls = h.read32(0x0078_b564);
    h.write32(controls.wrapping_add(0x28), second);
}

/// `1.0 / sqrt(value) * scale` as the original's `rsqrtss` replacement computes it.
fn normalise_factor(value: f32, scale: f32) -> f32 {
    (1.0f32 / value.sqrt()) * scale
}

/// 002548F0 game_mode_78b594_steer_controls_78b564_toward_target_point. Register input EAX = mode
/// camera object. Steers the two controls of `[0x78B564]` (values 0, 1, 2) toward the target point
/// `[0x78B580..0x78B588]`: off when steering is disabled (`[0x78B590] == 0`) or the object is
/// close and (in mode 7) its heading is within tolerance, a re-pick of the target through 254690
/// when close in other modes, otherwise from the angle between the object's heading (sine table
/// lookup of +0x70) and the direction to the target (x87 library function 3C9CC4 on the clamped
/// dot product, scaled by `[0x478638]`) and the sign of their cross product.
pub fn steer_toward_target<H: ModeHost>(h: &mut H, object: u32) {
    if h.read32(0x0078_b590) == 0 {
        set_controls(h, 0, 0);
        return;
    }
    let dx = read_f32(h, object.wrapping_add(0x64)) - read_f32(h, 0x0078_b580);
    let dz = read_f32(h, object.wrapping_add(0x6c)) - read_f32(h, 0x0078_b588);
    let squared = dx * dx + dz * dz;
    let heading = |h: &H| {
        read_f32(h, object.wrapping_add(0x70)) * read_f32(h, 0x0047_5d0c) * read_f32(h, 0x0047_5ce0)
            + read_f32(h, 0x0047_5cd4)
    };
    let table = |h: &H, index: i32| read_f32(h, 0x007a_38a0 + 4 * (index & 0x7ff) as u32);
    if read_f32(h, 0x0047_5db4) > squared {
        let mode = h.read32(0x0078_b594);
        if mode != 7 {
            set_controls(h, 0, 0);
            h.call(0x0025_4690, &[]);
            return;
        }
        let difference = (read_f32(h, 0x0047_8628) - read_f32(h, object.wrapping_add(0x70))).abs();
        if read_f32(h, 0x0047_862c) > difference {
            set_controls(h, 0, 0);
            return;
        }
        let index = cvttss2si(heading(h)).wrapping_add(0x200);
        let sample = table(h, index);
        if sample > read_f32(h, 0x0047_5cac) {
            let controls = h.read32(0x0078_b564);
            h.write32(controls.wrapping_add(0x10), 2);
            let controls = h.read32(0x0078_b564);
            h.write32(controls.wrapping_add(0x28), 1);
        } else {
            let controls = h.read32(0x0078_b564);
            h.write32(controls.wrapping_add(0x10), 1);
            let controls = h.read32(0x0078_b564);
            h.write32(controls.wrapping_add(0x28), 2);
        }
        return;
    }
    let mode = h.read32(0x0078_b594);
    let reversed = mode == 8;
    let (first, second) = if reversed { (2u32, 1u32) } else { (1, 2) };
    let one = read_f32(h, 0x0047_5c78);
    let angle_index = cvttss2si(heading(h));
    let scale = read_f32(h, 0x0047_60cc);
    let mut sin_like = table(h, angle_index) * scale;
    let mut cos_like = table(h, angle_index.wrapping_add(0x200)) * scale;
    let limit = read_f32(h, 0x0047_65f4);
    let length = cos_like * cos_like + sin_like * sin_like;
    if length > limit {
        let factor = normalise_factor(length, one);
        sin_like *= factor;
        cos_like *= factor;
    }
    let up = read_f32(h, object.wrapping_add(0x68)) - read_f32(h, 0x0078_b584);
    let mut z = read_f32(h, object.wrapping_add(0x6c)) - read_f32(h, 0x0078_b588);
    let mut x = read_f32(h, object.wrapping_add(0x64)) - read_f32(h, 0x0078_b580);
    let length = z * z + up * up + x * x;
    if length > limit {
        let factor = normalise_factor(length, one);
        x *= factor;
        z *= factor;
    }
    let dot = z * cos_like + x * sin_like;
    let low = read_f32(h, 0x0047_60d0);
    let clamped = if dot > one {
        one
    } else if low > dot {
        low
    } else {
        dot
    };
    let angle = h.call_x87_unary(0x003c_9cc4, clamped);
    let angle = ((f64::from(angle) * f64::from(read_f32(h, 0x0047_8638))) as f32).abs();
    let cross_positive = |h: &H| x * cos_like - z * sin_like > read_f32(h, 0x0047_5cac);
    let region = if reversed {
        if read_f32(h, 0x0047_d820) > angle {
            Region::Wide
        } else if read_f32(h, 0x0047_d730) > angle {
            Region::Narrow
        } else {
            Region::Straight
        }
    } else if angle > read_f32(h, 0x0047_8900) {
        Region::Wide
    } else if angle > read_f32(h, 0x0047_cff4) {
        Region::Narrow
    } else {
        Region::Straight
    };
    match region {
        Region::Straight => set_controls(h, first, first),
        Region::Wide => {
            if cross_positive(h) {
                set_controls(h, second, first);
            } else {
                set_controls(h, first, second);
            }
        }
        Region::Narrow => {
            if cross_positive(h) {
                set_controls(h, 0, first);
            } else {
                set_controls(h, first, 0);
            }
        }
    }
}

/// How far the heading is off the target direction in [`steer_toward_target`].
enum Region {
    Straight,
    Narrow,
    Wide,
}

#[cfg(test)]
mod tests {
    use super::super::test_host::TestHost;
    use super::*;
    use crate::memory::GuestMemory;

    #[test]
    fn follower_rotation_uses_record_layout_and_five_entry_pairs() {
        let mut h = TestHost::default();
        let object = 0x8000;
        h.write32(object + OBJECT_NODE_LIST, 0x9000);
        h.write32(0x9000 + NODE_RECORD_LIST, 0xa000);
        h.write32(0xa000, 0);
        h.write32(0xa000 + RECORD_POSITION_PTR, 0xb000);
        h.write32(0xb000, 1.0f32.to_bits());
        h.write32(0xb004, 2.0f32.to_bits());
        h.write32(0xb008, 3.0f32.to_bits());
        h.write32(0x9000 + NODE_MATRIX_TABLE, 0xc000);
        h.write32(MODE_FOLLOWER_ANGLE, 2.0f32.to_bits());
        h.write32(ANGLE_TO_RADIANS, 0.5f32.to_bits());
        h.write32(MODE_ENTRY_LIST_PTR, 0xd000);
        for index in 0..5 {
            let base = 0xd000 + index * ENTRY_STRIDE;
            h.write32(base + ENTRY_FIRST_MATRIX_INDEX, index + 1);
            h.write32(base + ENTRY_SECOND_MATRIX_INDEX, index + 6);
        }
        apply_follower_rotation_to_matrices(&mut h, object);
        assert_eq!(h.calls.len(), 18);
        assert_eq!(h.calls[0].0, MATRIX_TRANSLATION);
        assert_eq!(
            &h.calls[0].1[1..],
            &[1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits()]
        );
        assert_eq!(h.calls[1].0, MATRIX_ROTATION);
        assert_eq!(h.calls[1].1[1], 1.0f32.to_bits());
        let sources: Vec<u32> = h.calls[8..].iter().map(|(_, args)| args[0]).collect();
        assert_eq!(
            sources,
            (0..5)
                .flat_map(|index| [0xc000 + (index + 1) * 0x40, 0xc000 + (index + 6) * 0x40])
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn oscillator_advances_phase_and_eases_follower() {
        let mut h = TestHost::default();
        h.write32(0x0078_b564, 0x9000);
        h.write32(0x9014, 2.0f32.to_bits());
        h.write32(0x902c, 1.0f32.to_bits());
        h.write32(0x0078_b570, 0);
        h.write32(0x0078_b574, 3.0f32.to_bits());
        h.write32(0x0078_b578, 0.0f32.to_bits());
        h.write32(0x0047_d740, 0.0f32.to_bits());
        h.write32(0x0047_5ce0, 1.0f32.to_bits());
        h.write32(0x0047_5cd4, 0.25f32.to_bits());
        h.write32(0x0047_d56c, 0.5f32.to_bits());
        h.write32(0x0047_cff4, 1.0f32.to_bits());
        h.write32(0x0047_5cc8, 0.1f32.to_bits());
        h.write32(SINE_TABLE + 4 * 0x200, 2.0f32.to_bits());
        sine_oscillator_step(&mut h);
        assert_eq!(h.read32(0x0078_b570), 1);
        assert_eq!(h.read32(0x0078_b574), 1.5f32.to_bits());
        assert_eq!(h.read32(0x0078_b578), 0.25f32.to_bits());
    }
}
