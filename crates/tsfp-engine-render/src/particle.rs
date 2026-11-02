// SPDX-License-Identifier: GPL-3.0-or-later
//! Particle colour and curve helpers of the effects system (T1898). They do not touch the
//! render state. The effects system calls them while it steps emitters.
//!
//! The SSE paths of the original are single precision (`f32` operations here). The x87 curve
//! sampler also rounds its arithmetic to f32 precision; its Rust wrapper returns that result as
//! `f64` to preserve the guest ABI.
//!
//! Evidence class: differential versus the lifted original, see docs/t-render-slice.md.
use crate::{mem::GuestAccess, memory::GuestMemory};

/// Original addresses of the functions ported in this module (T1898, see docs/t-render-slice.md).
pub const ROOTS: [u32; 6] = [0x10c390, 0x10c400, 0x10c550, 0x153bd0, 0xffe50, 0x10e410];

const RANDOM_SEED: u32 = 0x0074_97CC;
const RANDOM_SCALE: u32 = 0x0047_5CBC;
const CONST_HALF: u32 = 0x0047_5CD4;
const CONST_ONE: u32 = 0x0047_5C78;

/// The CRT float to integer helper `_ftol` (0x3C848C): truncate toward zero, keep the low 32
/// bits of the 64 bit result. NaN and out of range inputs give 0 (the integer indefinite
/// value has a zero low word).
pub fn ftol(value: f32) -> u32 {
    let truncated = crate::render_math::trunc64(f64::from(value));
    if truncated.abs() < 9.223_372_036_854_776e18 {
        truncated as i64 as u32
    } else {
        0
    }
}
/// 0x153BD0: the effects random generator. A linear congruential step on the seed at
/// `0x7497CC` (multiplier 0x343FD, increment 0x269EC3), 15 bits of the high half scaled by
/// the constant at `0x475CBC`.
pub fn random_unit_float<M: GuestMemory>(memory: &mut M) -> f64 {
    let seed = (memory.read32(RANDOM_SEED) as i32)
        .wrapping_mul(0x343FD)
        .wrapping_add(0x26_9EC3) as u32;
    memory.write32(RANDOM_SEED, seed);
    let bits = (seed >> 0x10) & 0x7FFF;
    f64::from(bits) * f64::from(memory.read_f32(RANDOM_SCALE))
}

/// 0x10C390 game_curve_sample_piecewise_linear_float_pairs: sample the table of `count`
/// `(key, value)` float pairs at `table` for `x`. Below the first key it returns the first
/// value, above the last key the last value, between keys it interpolates linearly. A
/// count of 0 gives the constant at `0x475CD4`. NaN follows the original's `comiss` branches.
/// Interpolation arithmetic rounds to f32 after each operation, matching captured ST0 results.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the negated forms keep the NaN branches of `comiss`
pub fn curve_sample<M: GuestMemory>(memory: &M, x: f32, table: u32, count: u32) -> f64 {
    if count == 0 {
        return f64::from(memory.read_f32(CONST_HALF));
    }
    if count == 1 {
        return f64::from(memory.read_f32(table.wrapping_add(4)));
    }
    let key = |index: u32| memory.read_f32(table.wrapping_add(index.wrapping_mul(8)));
    let value =
        |index: u32| memory.read_f32(table.wrapping_add(index.wrapping_mul(8)).wrapping_add(4));
    let first_key = key(0);
    if first_key >= x {
        return f64::from(value(0));
    }
    let last = count.wrapping_sub(1);
    if x >= key(last) {
        return f64::from(value(last));
    }
    let mut index = last.wrapping_sub(1);
    if index != 0 {
        while !(x >= key(index)) {
            index = index.wrapping_sub(1);
            if index == 0 {
                break;
            }
        }
    }
    let (low_key, high_key) = (key(index), key(index + 1));
    let fraction = (x - low_key) / (high_key - low_key);
    let low_value = value(index);
    let delta = value(index + 1) - low_value;
    f64::from(fraction * delta + low_value)
}

/// Components `[+first+2, +first+1, +first]` of a byte triplet of the emitter definition, in
/// the order the original interpolates and packs them.
fn byte_triplet<M: GuestMemory>(memory: &M, definition: u32, first: u32) -> [f32; 3] {
    [2, 1, 0].map(|offset| f32::from(memory.read8(definition.wrapping_add(first + offset))))
}

/// `(to - from) * fraction + from` per component, one `f32` operation at a time.
fn interpolate(from: [f32; 3], to: [f32; 3], fraction: f32) -> [f32; 3] {
    [0, 1, 2].map(|index| (to[index] - from[index]) * fraction + from[index])
}

/// Pack three interpolated components like the original. The middle component is masked to a
/// byte first and shifted below the first, the first is not masked before it is shifted (an
/// overflow of the original that is kept).
fn pack_colour(components: [f32; 3]) -> u32 {
    let middle = ftol(components[1]) & 0xFF;
    let first = ftol(components[0]);
    let upper = (middle | first.wrapping_shl(8)).wrapping_shl(8);
    (upper | (ftol(components[2]) & 0xFF)).wrapping_shl(8)
}

fn emitter_definition<M: GuestMemory>(memory: &M, emitter: u32) -> u32 {
    memory.read32(memory.read32(emitter.wrapping_add(0xC)).wrapping_add(0x4C))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::SparseMemory;

    #[test]
    fn curve_sample_rounds_each_interpolation_step_to_f32() {
        let mut memory = SparseMemory::new();
        let table = 0x1000;
        memory.write_f32(table, 0.0);
        memory.write_f32(table + 4, 0.0);
        memory.write_f32(table + 8, f32::from_bits(0x3e78_af81));
        memory.write_f32(table + 12, f32::from_bits(0x3ee1_47ae));

        let got = curve_sample(&memory, f32::from_bits(0x3d2a_aaab), table, 2);

        assert_eq!(
            got.to_bits(),
            f64::from(f32::from_bits(0x3d9a_9aa1)).to_bits()
        );
    }
}

/// 0x10C400: packed colour of `particle` for `emitter`, interpolated along the definition's
/// two segments (start to middle until the fraction at definition `+0x28`, then middle to
/// end). Register convention of the original: EAX emitter, ECX particle.
pub fn particle_colour_step<M: GuestMemory>(memory: &M, emitter: u32, particle: u32) -> u32 {
    let progress =
        memory.read_f32(particle.wrapping_add(0x28)) / memory.read_f32(particle.wrapping_add(0x24));
    let definition = emitter_definition(memory, emitter);
    let split = memory.read_f32(definition.wrapping_add(0x28));
    let start = byte_triplet(memory, definition, 0x21);
    let middle = byte_triplet(memory, definition, 0x25);
    let end = byte_triplet(memory, definition, 0x2D);
    let components = if split >= progress {
        interpolate(start, middle, progress / split)
    } else {
        let fraction = (progress - split) / (memory.read_f32(CONST_ONE) - split);
        interpolate(middle, end, fraction)
    };
    pack_colour(components)
}

/// 0x10C550 game_particle_color_lerp_between_byte_triplets_packed_rgb: the colour word of
/// `particle`: the emitter definition's base colour (`+0x20`) when it has no colour
/// animation, the two segment interpolation (0x10C400) when a split fraction is set, else
/// a straight start to end interpolation. The result is added to the particle's own
/// colour word at `+0x2C`.
pub fn particle_colour<M: GuestMemory>(memory: &M, emitter: u32, particle: u32) -> u32 {
    let definition = emitter_definition(memory, emitter);
    let base = memory.read32(definition.wrapping_add(0x20));
    let alternate = memory.read32(definition.wrapping_add(0x2C));
    let split = memory.read_f32(definition.wrapping_add(0x28));
    let offset = memory.read32(particle.wrapping_add(0x2C));
    if base == alternate && split == 0.0 {
        return offset.wrapping_add(base);
    }
    if split != 0.0 {
        return offset.wrapping_add(particle_colour_step(memory, emitter, particle));
    }
    let progress =
        memory.read_f32(particle.wrapping_add(0x28)) / memory.read_f32(particle.wrapping_add(0x24));
    let start = byte_triplet(memory, definition, 0x21);
    let end = byte_triplet(memory, definition, 0x2D);
    offset.wrapping_add(pack_colour(interpolate(start, end, progress)))
}

/// 0xFFE50 game_pool_record_advance_point_by_velocity_with_frame_dt_scale: advance the point of
/// the pool record by its velocity for one frame (`1 / frames_per_second` scale from
/// `0x4E8CEC` and `0x73BBA8`). While the record's age limit (`+0xC`) exceeds the scale it is a
/// plain step, otherwise the step is jittered by the random generator (halved when `+0x34`
/// has at most one bit set). Does nothing while `0x4E7944` is zero.
pub fn advance_pool_record_point<M: GuestMemory>(memory: &mut M, record: u32) {
    if memory.read32(0x004E_7944) == 0 {
        return;
    }
    let one = memory.read_f32(CONST_ONE);
    let frames = memory.read32(0x004E_8CEC) as i32 as f32;
    let scale = one / frames * memory.read_f32(0x0073_BBA8);
    let point = memory.read32(record.wrapping_add(0x18));
    let fast = memory.read_f32(record.wrapping_add(0xC)) > scale;
    let jitter = if fast {
        1.0
    } else {
        let random = random_unit_float(memory) as f32;
        let bits = memory.read32(record.wrapping_add(0x34));
        if bits & bits.wrapping_sub(1) == 0 {
            random * memory.read_f32(CONST_HALF)
        } else {
            random
        }
    };
    for axis in 0..3u32 {
        let velocity = memory.read_f32(point.wrapping_add(0x10 + 4 * axis));
        let position = memory.read_f32(point.wrapping_add(4 + 4 * axis));
        let weight = memory.read_f32(point.wrapping_add(0x1C));
        let moved = if fast {
            velocity * scale * weight + position
        } else {
            velocity * jitter * scale * weight + position
        };
        memory.write_f32(point.wrapping_add(4 + 4 * axis), moved);
    }
}

/// 0x10E410 game_particle_compute_player_visibility_mask_0x7c (register convention: EDI is the
/// particle). Builds the mask of players `+0x7C` that can see the particle from its candidate
/// player mask `+0x78`: every candidate when the particle has no sources (`+0xE0 == 0`, `+0x88
/// == 0`) or the game mode word `0x79094C` is `0x65`, else the players whose record
/// (`0x7B0CE4 + player * 0xFFC`) is active (`+0x21C` negative with bit 0) and whose visibility
/// byte `+0xD2C + source index` is set for at least one source. Returns whether the mask is
/// non zero.
pub fn player_visibility_mask<M: GuestMemory>(memory: &mut M, particle: u32) -> u32 {
    let mask_address = particle.wrapping_add(0x7C);
    let sources = memory.read32(particle.wrapping_add(0xE0)) as i32;
    if (sources == 0 && memory.read32(particle.wrapping_add(0x88)) == 0)
        || memory.read32(0x0079_094C) == 0x65
    {
        let candidates = memory.read32(particle.wrapping_add(0x78));
        memory.write32(mask_address, candidates);
        return 1;
    }
    memory.write32(mask_address, 0);
    if sources > 0 {
        let mut players = memory.read32(0x007B_0CE0) as i32;
        let mut records = memory.read32(0x007B_0CE4);
        let mut source = 0i32;
        let mut count = sources;
        while source < count {
            let source_byte = particle.wrapping_add(0xE4).wrapping_add(source as u32);
            let candidates = memory.read32(particle.wrapping_add(0x78));
            let mut player = 0i32;
            let mut offset = 0u32;
            while player < players {
                let bit = 1u32.wrapping_shl(player as u32);
                if candidates & bit != 0 {
                    let state = memory.read32(records.wrapping_add(offset).wrapping_add(0x21C));
                    if (state as i32) < 0 && state & 1 != 0 {
                        let index = u32::from(memory.read8(source_byte)).wrapping_add(offset);
                        if memory.read8(records.wrapping_add(index).wrapping_add(0xD2C)) != 0 {
                            memory.or32(mask_address, bit);
                            players = memory.read32(0x007B_0CE0) as i32;
                            records = memory.read32(0x007B_0CE4);
                        }
                    }
                }
                player += 1;
                offset = offset.wrapping_add(0xFFC);
            }
            source += 1;
            count = memory.read32(particle.wrapping_add(0xE0)) as i32;
        }
    }
    u32::from(memory.read32(mask_address) != 0)
}
