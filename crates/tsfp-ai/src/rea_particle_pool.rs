// SPDX-License-Identifier: GPL-3.0-or-later
//! Particle pool step `0x288030` (task T2434, REA AI frontier, mask bit 32).
//!
//! Read with REA (Ghidra on `tsfp-analysis-v4.elf`, sha256 `e99faaeb...f866f`, MEASURED static). The function
//! has no callees. The pool pointer is `[0x78B1EC]`, it holds 500 entries of 32 bytes and a live counter
//! at `+0x3E84`. Per entry (offsets from the entry): position `+0/+4/+8`, velocity `+0xC/+0x10/+0x14`, age
//! `+0x18`. Entries whose age already reached 1.0 are parked (age set to `f32::MAX`), live ones advance by
//! `dt = (1.0 / [0x4E8CEC]) * [0x4E7944]` (two guest integers converted to float), age by `0.9 * dt`,
//! position by `velocity * dt`, gravity `vy += dt` once age is above 0.75, and a floor bounce when the entry
//! moves down below `y = -0.4`: `vy *= -0.1` and the velocity is rescaled to its previous speed.
//! The original's `rsqrtss` is modelled as the exact `1 / sqrt` (same as the lifted rsqrt model) and its x87
//! speed sum is evaluated in f64 (INFERRED close to the 80-bit original).

use crate::perception_geometry::sqrt_f64;

/// Guest address of the pool pointer.
pub const POOL_POINTER: u32 = 0x78b1ec;
/// Guest integers whose ratio forms the time step.
pub const STEP_DIVISOR: u32 = 0x4e8cec;
pub const STEP_FACTOR: u32 = 0x4e7944;
pub const ENTRY_COUNT: usize = 500;
pub const ENTRY_STRIDE: u32 = 0x20;
pub const LIVE_COUNT_OFFSET: u32 = 0x3e84;

const AGE_LIMIT: f32 = 1.0; // 0x475C78
const AGE_RATE: f32 = 0.9; // 0x476040
const GRAVITY_AGE: f32 = 0.75; // 0x475D40
const BOUNCE_SCALE: f32 = -0.1; // 0x47CFDC
const FLOOR_Y: f32 = -0.4; // 0x47DAE8
const PARKED_AGE: f32 = f32::MAX; // 0x475D5C

/// One pool entry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Particle {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub age: f32,
}

/// Time step of one call.
pub fn time_step(divisor: i32, factor: i32) -> f32 {
    (AGE_LIMIT / divisor as f32) * factor as f32
}

/// Advance one entry; returns whether it was live (counted).
pub fn step_particle(p: &mut Particle, dt: f32) -> bool {
    if !(AGE_LIMIT > p.age) {
        p.age = PARKED_AGE;
        return false;
    }
    p.age += dt * AGE_RATE;
    for axis in 0..3 {
        p.position[axis] += dt * p.velocity[axis];
    }
    if p.age > GRAVITY_AGE {
        p.velocity[1] += dt;
    }
    if 0.0 > p.velocity[1] && FLOOR_Y > p.position[1] {
        let [vx, old_vy, vz] = p.velocity;
        let old_speed = sqrt_f64(
            f64::from(old_vy) * f64::from(old_vy) + f64::from(vz * vz) + f64::from(vx * vx),
        ) as f32;
        let new_vy = old_vy * BOUNCE_SCALE;
        let new_len_sq = new_vy * new_vy + vz * vz + vx * vx;
        let factor = old_speed * (1.0 / (sqrt_f64(f64::from(new_len_sq)) as f32));
        p.velocity = [vx * factor, new_vy * factor, vz * factor];
    }
    true
}

/// Advance the whole pool; returns the live count stored at `+0x3E84`.
pub fn step_pool(pool: &mut [Particle], dt: f32) -> u32 {
    pool.iter_mut()
        .map(|p| u32::from(step_particle(p, dt)))
        .sum()
}
