//! T1876: the flare update callback `0xF1A90` (class table `0x4DEB28`, slot `+0x34`, types `0x26` (flare, `R4 Flare Gun`) and `0x29`), ported from the
//! original and validated bit for bit against it under the Unicorn oracle (`tools/t1846_gold/eff_flare.py`, test `tests/story_events_t1876_flare.rs`).
//! Document `docs/t-story-event-effect-update-t1876.md`. Behaviour MEASURED (xemu level oracle), names INFERRED. Locals live at the same addresses
//! relative to the entry esp as in the original (see `eff_update::FRAME`), so the records that carry pointers to locals compare equal.

#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::eff_guest::{Guest, visual_rng, write_byte};
use super::eff_update::{FRAME, FRAME_RATE, STEP_SCALE, STEPS};
use super::eff_update2::cvtt;

/// The sine table `[0x7A38A0]`, 2048 f32 entries (a runtime table; the goldens provide the entries the original reads).
const SINE_TABLE: u32 = 0x007A_38A0;

fn frame(offset: u32) -> u32 {
    FRAME.wrapping_sub(offset)
}

fn f(bits: u32) -> f32 {
    f32::from_bits(bits)
}

fn sine(g: &mut dyn Guest, index: i32) -> f32 {
    g.readf(SINE_TABLE.wrapping_add(((index & 0x7FF) as u32) * 4))
}

/// `fild` of a random dword (plus `2^32` when negative) times `2^-32` (an exact f64), then `- 0.5`, `* 0.1`, `+ slot`, one f32 store: the three jitters of the
/// aim point (x87 doubles).
fn jitter(g: &mut dyn Guest, slot: u32) {
    let word = g.rng();
    g.write(frame(0x1F0), word);
    let mut value = f64::from(word as i32);
    if (word as i32) < 0 {
        value += f64::from(f(0x4F80_0000));
    }
    let unit = value * f64::from(f(0x2F80_0000));
    let result =
        (unit - f64::from(f(0x3F00_0000))) * f64::from(f(0x3DCC_CCCD)) + f64::from(g.readf(slot));
    g.writef(slot, result as f32);
}

/// `0xF1A90(entry)`: update of the flare entries (type `0x26` the flare gun shot, type `0x29` its child). Names INFERRED, flow MEASURED.
///
/// * Steps `[0x4E7944] == 0`: nothing (the position is saved first). `dt = (1 / rate) * [+0x88]`, the step is `velocity * dt` and `0xDCB80` sweeps it
///   (`hit` E-0x1C8, `normal` E-0x1D4, ray record E-0x10C); a positive record count starts the light `0xDCEF0`. A hit on the shooter `+0x108` while the timer is
///   below `0.1` is ignored.
/// * EXPIRY (timer `+0x98` above the limit `+0x9C`): a type `0x26` with `+4 == 0` and every type `0x29` spawn a child entry (`0xDC4A0(0x17)`, type `0x18`
///   or `0x22`, `+0x1C0` = parent type), play the sound `0xC33 + rng % 6` (`0x45390` / `0x45820`) and link it to the shooter (`0xC3C50`, attach by
///   a type 8 shooter with a non negative flag word, mode `+0x100` 1 with `0x238510(0x52, ..., 1.0)` or 2), start it (`0xEDD40`) or flag it `0x800000`;
///   then the entry is released (`0xDC550`).
/// * HIT: the velocity is reflected about the normal with factor `2 * 0.3` (`rx = vx - (2 dot nx) * 0.3` ...), `+0xF4` takes the squared distance to the
///   hit; a type 8 target runs `0xE9FE0` and `0xDC5D0`. Then the same child spawn as above (type 0x26 with `+4 == 0` at the entry position with the up
///   direction, type 0x29 at `hit + normal * 0.1` along the normal) and `0xE94E0(entry, target type, hit, 6.0)`, `0xDD170` / `0x114AF0`, `0xE3170` (nonzero
///   releases at once), a hit record (`0xB91F0` / `0xB9220` / `0xBD0F0`) and the release.
/// * NO HIT: a wobble around the aim point `+0x1A0 ..`: a phase `fmod(((t - 1)+ * c20C + c208 * t) * 2 pi + c218, 2 pi)` (x87 doubles, CRT `fmod`),
///   a fade of the amplitude after the half period, an offset from the sine table (`+0x1C4 ..` / `+0x1D0 ..` basis), three random jitters, the new velocity
///   toward the aim point (`+0x64 ..`, size `+0x88` = rate * distance); type `0x26` throws 3 (or 5 with `+4 != 0`) sparks along the path (`0xF85C0`,
///   visual random generator); finally `0xEDBE0(position, 0, entry)`.
// portreg: va=0x000f1a90
pub fn flare_update(g: &mut dyn Guest, entry: u32) {
    let ebp = entry + 0x1C;
    let (hit, normal, ray, reflected) = (frame(0x1C8), frame(0x1D4), frame(0x10C), frame(0x1BC));
    let old = g.vec3(ebp);
    g.set_vec3(frame(0x174), old);
    if g.read(STEPS) == 0 {
        return;
    }
    let rate = g.read(FRAME_RATE) as i32 as f32;
    let dt = (f(0x3F80_0000) / rate) * g.readf(entry + 0x88);
    let velocity = g.vec3(entry + 0x64);
    let step = frame(0x180);
    g.set_vec3(step, [velocity[0] * dt, velocity[1] * dt, velocity[2] * dt]);
    g.writef(frame(0x1F0), dt);
    g.write(ray, 0);
    g.write(ray + 4, 0);
    let mut target = g.call(0xDCB80, &[step, hit, normal, ray, entry]);
    g.write(frame(0x1F4), target);
    if (g.read(ray) as i32) > 0 {
        g.call(0xDCEF0, &[entry, ray]);
    }
    if g.read(entry + 0x108) == target && f(0x3DCC_CCCD) > g.readf(entry + 0x98) {
        g.write(frame(0x1F4), 0);
        target = 0;
    }
    if g.readf(entry + 0x98) > g.readf(entry + 0x9C) {
        let entry_type = g.read(entry);
        if entry_type == 0x26 && g.read(entry + 4) == 0 || entry_type == 0x29 {
            let src = g.read(entry + 0x10C);
            g.write(frame(0x1F4), src);
            spawn(g, entry, entry_type, src, Spawn::Expiry);
        }
        g.call(0xDC550, &[entry]);
        return;
    }
    if target == 0 {
        no_hit(g, entry);
        return;
    }
    // hit: reflect the velocity, accumulate the distance
    let n = g.vec3(normal);
    let h = g.vec3(hit);
    let v = g.vec3(entry + 0x64);
    let dot = (v[2] * n[2] + v[1] * n[1]) + v[0] * n[0];
    let twice = dot * f(0x4000_0000);
    let damp = f(0x3E99_999A);
    let rx = v[0] - (twice * n[0]) * damp;
    let ry = v[1] - (twice * n[1]) * damp;
    let rz = v[2] - (twice * n[2]) * damp;
    let dz = h[2] - g.readf(entry + 0x24);
    let dx = h[0] - g.readf(ebp);
    let dy = h[1] - g.readf(entry + 0x20);
    let travelled = ((dz * dz + dy * dy) + dx * dx) + g.readf(entry + 0xF4);
    g.set_vec3(reflected, [rx, ry, rz]);
    g.writef(entry + 0xF4, travelled);
    let target_type = g.read(target + 0x20);
    if target_type == 8 {
        let entry_type = g.read(entry);
        let shooter = g.read(entry + 0x108);
        g.call(
            0xE9FE0,
            &[entry, entry_type, target, hit, normal, reflected, shooter],
        );
        g.call(0xDC5D0, &[entry]);
    }
    let entry_type = g.read(entry);
    if entry_type == 0x26 && g.read(entry + 4) == 0 {
        let src = g.read(entry + 0x10C);
        spawn(g, entry, 0x26, src, Spawn::HitUp);
        target = g.read(frame(0x1F4));
    } else if entry_type == 0x29 {
        let src = g.read(entry + 0x10C);
        spawn(g, entry, 0x29, src, Spawn::HitNormal);
        target = g.read(frame(0x1F4));
    }
    let target_type = g.read(target + 0x20);
    g.call(0xE94E0, &[entry, target_type, hit, 0x40C0_0000]);
    if g.call(0xDD170, &[]) != 0 {
        let word = g.read(entry + 0xF0);
        let entry_type = g.read(entry);
        g.call(0x114AF0, &[entry_type, 0, word, target, hit, normal]);
    }
    if g.call(0xE3170, &[hit, normal, target, entry]) != 0 {
        g.call(0xDC550, &[entry]);
        return;
    }
    let record = frame(0x168);
    g.call(0xB91F0, &[record]);
    g.call(0xB9220, &[record, entry]);
    let size = g.read(entry + 0x88);
    g.write(record + 0x2C, normal);
    g.write(record, target);
    g.write(record + 0x28, ebp);
    g.write(record + 0x30, entry + 0x64);
    g.write(record + 0x34, size);
    g.write(record + 0x40, 1);
    g.call(0xBD0F0, &[record]);
    g.call(0xDC550, &[entry]);
}

/// Where a child entry is spawned: at an expiry (stack slots: shooter E-0x1F4, ticks E-0x1F8) or after a hit (type 0x26 / 0x29 slots differ).
#[derive(Clone, Copy, PartialEq)]
enum Spawn {
    Expiry,
    HitUp,
    HitNormal,
}

/// The child spawn shared by the six sites of `0xF1A90` (`0xF1BD1`, `0xF1D46`, `0xF1EFF`, `0xF207C`): the sound, the field initialisation, the link to the shooter.
fn spawn(g: &mut dyn Guest, entry: u32, kind: u32, src: u32, how: Spawn) {
    let ebp = entry + 0x1C;
    let (src_slot, ticks_slot) = match how {
        Spawn::Expiry => (frame(0x1F4), frame(0x1F8)),
        Spawn::HitUp => (frame(0x1F0), frame(0x1F8)),
        Spawn::HitNormal => (frame(0x1F8), frame(0x1F0)),
    };
    if how != Spawn::Expiry {
        g.write(src_slot, src);
    }
    let new = g.call(0xDC4A0, &[0x17]);
    g.write(new + 4, if kind == 0x26 { 0x18 } else { 0x22 });
    g.write(new + 0x1C0, kind);
    let rate = g.read(FRAME_RATE) as i32 as f32;
    g.write(ticks_slot, cvtt(rate * f(0)) as u32);
    let draw = g.rng();
    let id = draw % 6 + 0xC33;
    let request = frame(0x1B0);
    g.call(0x45390, &[request]);
    let position: [u32; 3] = if how == Spawn::HitNormal {
        [
            g.read(frame(0x1C8)),
            g.read(frame(0x1C4)),
            g.read(frame(0x1C0)),
        ]
    } else {
        [g.read(ebp), g.read(ebp + 4), g.read(ebp + 8)]
    };
    g.write(request + 0x1C, position[0]);
    let ticks = g.read(ticks_slot);
    g.write(request + 0x20, position[1]);
    write_byte(g, request + 0x28, 1);
    g.write(request, id);
    g.write(request + 4, ticks);
    g.write(request + 0x24, position[2]);
    g.call(0x45820, &[request]);
    if how == Spawn::HitNormal {
        let n = g.vec3(frame(0x1D4));
        let h = g.vec3(frame(0x1C8));
        let tenth = f(0x3DCC_CCCD);
        g.writef(new + 0x1C, n[0] * tenth + h[0]);
        g.writef(new + 0x20, n[1] * tenth + h[1]);
        g.writef(new + 0x24, n[2] * tenth + h[2]);
        g.set_vec3(new + 0x180, n);
    } else {
        g.write(new + 0x1C, position[0]);
        g.write(new + 0x20, position[1]);
        g.write(new + 0x24, position[2]);
        g.writef(new + 0x180, 0.0);
        g.writef(new + 0x184, 1.0);
        g.writef(new + 0x188, 0.0);
    }
    g.write(new + 0xA0, 0);
    g.write(new + 0x104, 0);
    g.write(new + 0x108, src);
    g.write(new + 0x10C, src);
    g.writef(new + 0x98, 0.0);
    let linked = g.call(0xC3C50, &[new + 0x1C, 0x3DCC_CCCD, 0]);
    g.write(new + 0xF0, linked);
    let owner = g.read(new + 0x10C);
    if owner != 0 && g.read(src + 0x20) == 8 && (g.read(src + 0x28) as i32) >= 0 {
        let block = g.read(src + 0x7C);
        g.write(new + 0x104, block);
        let chain = g.read(owner + 0x150);
        if chain != 0 {
            g.write(new + 0x100, 1);
            let word = g.read(chain);
            g.call(0x238510, &[0x52, word, 0x3F80_0000]);
        } else {
            g.write(new + 0x100, 2);
        }
    }
    // comiss: below zero (or unordered) flags the entry instead of starting it
    if !(g.readf(new + 0x98) >= 0.0) {
        let flags = g.read(new + 0xC);
        g.write(new + 0xC, flags | 0x80_0000);
    } else {
        g.call(0xEDD40, &[new]);
    }
}

/// No hit (`0xF2329`): the wobble toward the aim point, the new velocity and the sparks.
fn no_hit(g: &mut dyn Guest, entry: u32) {
    let ebp = entry + 0x1C;
    let one = f(0x3F80_0000);
    let two_pi_bits = 0x40C9_0FDB;
    let timer = g.readf(entry + 0x98);
    let v = if timer > one {
        one
    } else {
        g.readf(entry + 0x98)
    };
    g.writef(frame(0x1F4), v);
    let excess = g.readf(entry + 0x98) - one;
    g.writef(frame(0x1F8), 0.0);
    if !(0.0 > excess) {
        g.writef(frame(0x1F8), excess);
    }
    // x87 doubles: ((a * c20C) * 2pi + (c208 * t) * 2pi) + c218, then fmod by 2 pi
    let two_pi = f64::from(f(two_pi_bits));
    let amount = g.readf(frame(0x1F8));
    let lead = f64::from(amount) * f64::from(g.readf(entry + 0x20C)) * two_pi;
    let lag = f64::from(g.readf(entry + 0x208)) * f64::from(g.readf(entry + 0x98)) * two_pi;
    let sum = lead + lag + f64::from(g.readf(entry + 0x218));
    let remainder = sum % f64::from_bits(0x401921FB60000000);
    g.writef(frame(0x1F8), remainder as f32);
    let mut phase = g.readf(frame(0x1F8));
    let half = f(0x3F00_0000);
    let c210 = g.readf(entry + 0x210);
    let swing = ((g.readf(entry + 0x214) - c210) * g.readf(entry + 0x98)) * half + c210;
    if 0.0 > phase {
        phase += f(two_pi_bits);
    }
    let pi = f(0x4049_0FDB);
    let fade;
    if phase > pi {
        let w = (phase - pi) * f(0x3EA2_F983);
        if half > w {
            fade = (one - w) * g.readf(frame(0x1F4));
        } else {
            let scaled = w * g.readf(frame(0x1F4));
            g.writef(frame(0x1F4), scaled);
            fade = g.readf(frame(0x1F4));
        }
    } else {
        fade = g.readf(frame(0x1F4));
    }
    let mut fade = fade;
    // advance the position by the step
    let step = g.vec3(frame(0x180));
    let px = step[0] + g.readf(ebp);
    g.writef(ebp, px);
    let c21c = g.readf(entry + 0x21C);
    let c220 = g.readf(entry + 0x220);
    let py = g.readf(entry + 0x20) + step[1];
    g.writef(entry + 0x20, py);
    let pz = g.readf(entry + 0x24) + step[2];
    g.writef(entry + 0x24, pz);
    let dt = g.readf(frame(0x1F0));
    let travelled = dt * dt + g.readf(entry + 0xF4);
    g.writef(entry + 0xF4, travelled);
    let rate = g.read(FRAME_RATE) as i32 as f32;
    let k = ((one / rate) * g.readf(entry + 0x228)) * g.readf(STEP_SCALE);
    let kz = g.readf(entry + 0x224) * k;
    let kx = c21c * k;
    let ky = c220 * k;
    g.writef(frame(0x1B4), kz);
    let nx = g.readf(entry + 0x40) + kx;
    let aim = g.readf(entry + 0x1A0) - nx;
    g.writef(frame(0x1EC), aim);
    let ny = g.readf(entry + 0x44) + ky;
    let aim = g.readf(entry + 0x1A4) - ny;
    g.writef(frame(0x1E8), aim);
    let nz = g.readf(entry + 0x48) + g.readf(frame(0x1B4));
    let aim = g.readf(entry + 0x1A8) - nz;
    g.writef(frame(0x1E4), aim);
    g.writef(entry + 0x40, nx);
    g.writef(entry + 0x44, ny);
    g.writef(entry + 0x48, nz);
    let (ez, ey, ex) = (
        f64::from(g.readf(frame(0x1E4))),
        f64::from(g.readf(frame(0x1E8))),
        f64::from(g.readf(frame(0x1EC))),
    );
    let distance = (ez * ez + ey * ey + ex * ex).sqrt();
    g.writef(frame(0x1F8), distance as f32);
    if f64::from(f(0x4120_0000)) > distance {
        fade = (g.readf(frame(0x1F8)) * fade) * f(0x3DCC_CCCD);
        g.writef(frame(0x1F4), fade);
    }
    let index = cvtt(phase * f(0x43A2_F983) + half);
    let sin_a = sine(g, index.wrapping_add(0x200));
    let sin_b = sine(g, index);
    let wave_a = (sin_a * swing) * fade;
    let wave_b = (sin_b * swing) * fade;
    let cx = c21c * half + nx;
    let cy = c220 * half + ny;
    let rest_z = g.readf(entry + 0x224) * half + nz;
    let x = (g.readf(entry + 0x1D0) * wave_b) + ((g.readf(entry + 0x1C4) * wave_a) + cx);
    let y = (g.readf(entry + 0x1D4) * wave_b) + ((g.readf(entry + 0x1C8) * wave_a) + cy);
    let z = (g.readf(entry + 0x1D8) * wave_b) + ((g.readf(entry + 0x1CC) * wave_a) + rest_z);
    g.writef(frame(0x1EC), x);
    g.writef(frame(0x1E8), y);
    g.writef(frame(0x1E4), z);
    jitter(g, frame(0x1EC));
    jitter(g, frame(0x1E8));
    // third draw: the x and y offsets are formed before the z jitter is stored
    let dy = g.readf(frame(0x1E8)) - g.readf(entry + 0x20);
    let dx = g.readf(frame(0x1EC)) - g.readf(ebp);
    g.writef(frame(0x1DC), dy);
    g.writef(frame(0x1E0), dx);
    jitter(g, frame(0x1E4));
    let dz = g.readf(frame(0x1E4)) - g.readf(entry + 0x24);
    g.writef(frame(0x1D8), dz);
    let (fz, fy, fx) = (f64::from(dz), f64::from(dy), f64::from(dx));
    let length = (fz * fz + fy * fy + fx * fx).sqrt() as f32;
    g.writef(frame(0x1F8), length);
    let inverse = one / g.readf(frame(0x1F8));
    g.writef(entry + 0x64, inverse * dx);
    g.writef(entry + 0x68, inverse * dy);
    g.writef(entry + 0x6C, inverse * dz);
    let size = f64::from(g.read(FRAME_RATE) as i32) * f64::from(g.readf(frame(0x1F8)));
    g.writef(entry + 0x88, size as f32);
    if g.read(entry) == 0x26 {
        let mut index = 0u32;
        let count = |g: &mut dyn Guest| if g.read(entry + 4) != 0 { 5u32 } else { 3u32 };
        if count(g) > 0 {
            loop {
                let old = g.vec3(frame(0x174));
                let here = [g.readf(ebp), g.readf(entry + 0x20), g.readf(entry + 0x24)];
                let fraction = (index + 1) as i32 as f32 * f(0x3EAA_AAAB);
                let along = [
                    (here[0] - old[0]) * fraction + old[0],
                    (here[1] - old[1]) * fraction + old[1],
                    (here[2] - old[2]) * fraction + old[2],
                ];
                g.set_vec3(frame(0x1E0), along);
                let spread = f64::from(f(0x3E99_999A));
                let half64 = f64::from(half);
                let a = (visual_rng(g) - half64) * spread;
                g.writef(frame(0x1BC), a as f32);
                let b = (visual_rng(g) - half64) * spread + f64::from(f(0x3F47_AE14));
                g.writef(frame(0x1B8), b as f32);
                let c = (visual_rng(g) - half64) * spread;
                g.writef(frame(0x1B4), c as f32);
                let last = if g.read(entry + 4) == 0 {
                    0x3F4C_CCCD
                } else {
                    ((one - g.readf(entry + 0x98) / g.readf(entry + 0x9C)) * f(0x3E99_999A))
                        .to_bits()
                };
                g.call(
                    0xF85C0,
                    &[0x12, 0xD5, frame(0x1E0), frame(0x1BC), 0x3F19_999A, 0, last],
                );
                index += 1;
                if index >= count(g) {
                    break;
                }
            }
        }
    }
    g.call(0xEDBE0, &[ebp, 0, entry]);
}
