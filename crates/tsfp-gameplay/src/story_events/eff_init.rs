//! T1876: the effect pool INIT callbacks (class table `0x4DEB28`, slot `+0x30`, one stack argument: the entry), ported from the original and validated
//! bit for bit against it under the Unicorn oracle (`tools/t1846_gold/eff_init.py`, tests `tests/story_events_t1876_init.rs`). Same [`Guest`]
//! method and frame rule as `eff_update`. Document `docs/t-story-event-effect-update-t1876.md`. Behaviour MEASURED (xemu level), names INFERRED.
//!
//! The 19 functions of T1870 section 2: `0x389B0` (a bare `ret`: nothing to port), `0xE0A80`, `0xE0B50`, `0xE0BD0`, `0xE0C50`, `0xE14B0`, `0xE1620`,
//! `0xE1650`, `0xE28D0`, `0xE2DF0`, `0xE2F90`, `0xE34D0`, `0xE3570`, `0xE3640`, `0xE36D0`, `0xE3AB0`, `0xE4690`, `0xE78F0` and `0xECAC0`.

use super::eff_guest::Guest;
use super::eff_update::FRAME;
use super::eff_update2::{cvtt, unsigned_random_unit};

/// `[0x7B0CC0]`: the view object, `+0x214` the bit index of the local viewer.
const VIEW: u32 = 0x007B_0CC0;
const PLAYER_FIRST: u32 = 0x0079_0950;
const CLOCK: u32 = 0x007D_E318;
const RATE: u32 = 0x004E_8CEC;

/// `0x389B0`, the default init of several classes: a bare `ret`.
pub fn init_nothing(_g: &mut dyn Guest, _entry: u32) {}

fn view_bit(g: &mut dyn Guest) -> u32 {
    1u32.wrapping_shl(g.at(VIEW, 0x214))
}

/// The owner object creation shared by `0xE0B50`, `0xE0BD0`, `0xE0C50` and `0xE3570`: `0xA6F90(id, entry + 0x34, zero vector)` into `+0xA8`,
/// byte `+0x32 = 2`, object flags `+0x28 |= 0x40020000`.
fn make_owner(g: &mut dyn Guest, entry: u32, id: u32, position_offset: u32) -> u32 {
    let zero = FRAME - 0xC;
    g.set_vec3(zero, [0.0, 0.0, 0.0]);
    let owner = g.call(0xA6F90, &[id, entry + position_offset, zero, 0, 0]);
    g.write(entry + 0xA8, owner);
    owner
}

fn set_owner_flags(g: &mut dyn Guest, owner: u32) {
    super::eff_guest::write_byte(g, owner + 0x32, 2);
    let flags = g.read(owner + 0x28) | 0x4002_0000;
    g.write(owner + 0x28, flags);
}

/// The size scale of `0xE0B50`, `0xE0BD0` and `0xE0C50`: `size = (+0x1FC * 0.75 + 1) * size`.
fn scale_size(g: &mut dyn Guest, entry: u32) {
    let factor = g.readf(entry + 0x1FC) * f32::from_bits(0x3F40_0000) + f32::from_bits(0x3F80_0000);
    let size = factor * g.readf(entry + 0x88);
    g.writef(entry + 0x88, size);
}

/// `0xE0B50` (type `0x10`): owner object `0x70B`, size scaled.
pub fn init_owner_70b(g: &mut dyn Guest, entry: u32) {
    let owner = make_owner(g, entry, 0x70B, 0x34);
    set_owner_flags(g, owner);
    scale_size(g, entry);
}

/// `0xE0BD0` (type `0x11`): owner object `0x70E`, size scaled.
pub fn init_owner_70e(g: &mut dyn Guest, entry: u32) {
    let owner = make_owner(g, entry, 0x70E, 0x34);
    set_owner_flags(g, owner);
    scale_size(g, entry);
}

/// `0xE0C50` (type `0x12`): owner object `0x711`, size scaled, `+0x178 = 0`.
pub fn init_owner_711(g: &mut dyn Guest, entry: u32) {
    let owner = make_owner(g, entry, 0x711, 0x34);
    set_owner_flags(g, owner);
    let factor = g.readf(entry + 0x1FC) * f32::from_bits(0x3F40_0000) + f32::from_bits(0x3F80_0000);
    let size = factor * g.readf(entry + 0x88);
    g.write(entry + 0x178, 0);
    g.writef(entry + 0x88, size);
}

/// The pitch / yaw of the velocity as the originals compute it with `fpatan`: pitch `atan2(vy, sqrt(vx^2 + vz^2))` in degrees; the second angle is
/// `atan2(vz, vx)` in degrees mapped by `map`.
fn velocity_angles(g: &mut dyn Guest, entry: u32, map: impl Fn(f64) -> f64) -> (f32, f32) {
    let (vx, vy, vz) = (
        f64::from(g.readf(entry + 0x64)),
        f64::from(g.readf(entry + 0x68)),
        f64::from(g.readf(entry + 0x6C)),
    );
    let flat = (vx * vx + vz * vz).sqrt();
    let scale = f64::from(f32::from_bits(0x4265_2EE0));
    let pitch = (vy.atan2(flat) * scale) as f32;
    let yaw = map(vz.atan2(vx) * scale) as f32;
    (pitch, yaw)
}

/// `0xE0A80` (type `0x13`): owner object `0x714` aimed along the velocity (angles `pitch`, `180 - yaw`, `270`), flags `&= ~0x60`, the spins `+0x8C .. +0x94` cleared; a
/// one second old entry riding a player (`+0x100 == 1`, timer `+0x98 >= 1`) loses its velocity.
pub fn init_arrow(g: &mut dyn Guest, entry: u32) {
    let angles = FRAME - 0xC;
    let (pitch, yaw) =
        velocity_angles(g, entry, |yaw| f64::from(f32::from_bits(0x4334_0000)) - yaw);
    g.writef(angles, pitch);
    g.writef(angles + 4, yaw);
    g.writef(angles + 8, f32::from_bits(0x4387_0000));
    let owner = g.call(0xA6F90, &[0x714, entry + 0x34, angles, 0, 0]);
    g.write(entry + 0xA8, owner);
    super::eff_guest::write_byte(g, owner + 0x32, 2);
    let flags = g.read(owner + 0x28) & 0xFFFF_FF9F;
    g.write(owner + 0x28, flags);
    for offset in [0x8C, 0x90, 0x94] {
        g.writef(entry + offset, 0.0);
    }
    if g.read(entry + 0x100) == 1 && g.readf(entry + 0x98) >= 1.0 {
        for offset in [0x64, 0x68, 0x6C] {
            g.writef(entry + offset, 0.0);
        }
    }
}

/// `0xE28D0` (types `8 9 0xA 0xB 0x27 0x2A 0x32`): owner object (`0x743`, `0x745` for type `0xB`, `0x53F` for `0x32`, `0x744` for `0x2A`) aimed along the velocity
/// (pitch, `-yaw - 90`, 0), `0x10EC20(owner)`; type `0x32` flags the owner `0x10` and hooks `0x9E720(owner, 0xE28B0)` and clears the timer;
/// an entry with flag `0x2000` clears `+0x1AC`.
pub fn init_burst(g: &mut dyn Guest, entry: u32) {
    let angles = FRAME - 0xC;
    let entry_type = g.read(entry);
    let id = match entry_type {
        0xB => 0x745,
        0x32 => 0x53F,
        0x2A => 0x744,
        _ => 0x743,
    };
    let (pitch, yaw) = velocity_angles(g, entry, |yaw| {
        -yaw - f64::from(f32::from_bits(0x42B4_0000))
    });
    g.writef(angles, pitch);
    g.writef(angles + 4, yaw);
    g.writef(angles + 8, 0.0);
    let owner = g.call(0xA6F90, &[id, entry + 0x34, angles, 0, 0]);
    g.write(entry + 0xA8, owner);
    super::eff_guest::write_byte(g, owner + 0x32, 2);
    let flags = g.read(owner + 0x28) | 0x4002_0000;
    g.write(owner + 0x28, flags);
    let owner = g.read(entry + 0xA8);
    g.call(0x10EC20, &[owner]);
    if g.read(entry) == 0x32 {
        let owner = g.read(entry + 0xA8);
        let flags = g.read(owner + 0x28) | 0x10;
        g.write(owner + 0x28, flags);
        let owner = g.read(entry + 0xA8);
        g.call(0x9E720, &[owner, 0xE28B0]);
        g.writef(entry + 0x98, 0.0);
    }
    if g.read(entry + 0xC) & 0x2000 != 0 {
        g.write(entry + 0x1AC, 0);
    }
}

/// `0xE1620` (type `0x28`): the sound `0xFFDA0(entry + 0xB0, 0, entry + 0x34, entry + 0x7C, size, 0xFFFF)`.
pub fn init_sound_single(g: &mut dyn Guest, entry: u32) {
    let size = g.read(entry + 0x88);
    let handle = g.call(
        0xFFDA0,
        &[entry + 0xB0, 0, entry + 0x34, entry + 0x7C, size, 0xFFFF],
    );
    g.write(entry + 0xB0, handle);
}

/// `0xE14B0` (types `3`, `4`): four sound handles (`0xFFDA0` into `+0xAC`, `+0xB0`; `0x102320` into `+0xB4`, `+0xB8`). A viewer's own entry (`+0x100 == 1` and a
/// non negative block word of the source) splits them into the viewer's sounds (mask `1 << view bit`, entry `+0x28 / +0x70`) and the others'
/// (`~mask & 0xFFFF`, `+0x34 / +0x7C`); otherwise only the shared pair with mask `0xFFFF`. Every `0x102320` handle gets `0xFF420(handle, size)`.
pub fn init_sound_quad(g: &mut dyn Guest, entry: u32) {
    let viewer_entry = g.read(entry + 0x100) == 1 && {
        let source = g.read(entry + 0x108);
        let block = g.read(source + 0x150);
        (g.read(block + 4) as i32) >= 0
    };
    if viewer_entry {
        let bit = view_bit(g);
        let size = g.read(entry + 0x88);
        let handle = g.call(
            0xFFDA0,
            &[entry + 0xAC, 0, entry + 0x28, entry + 0x70, size, bit],
        );
        g.write(entry + 0xAC, handle);
        let others = !view_bit(g) & 0xFFFF;
        let size = g.read(entry + 0x88);
        let handle = g.call(
            0xFFDA0,
            &[entry + 0xB0, 0, entry + 0x34, entry + 0x7C, size, others],
        );
        g.write(entry + 0xB0, handle);
        let bit = view_bit(g);
        let handle = g.call(
            0x102320,
            &[entry + 0xB4, 9, 0, entry + 0x28, entry + 0x70, bit],
        );
        let size = g.read(entry + 0x88);
        g.write(entry + 0xB4, handle);
        g.call(0xFF420, &[handle, size]);
        let others = !view_bit(g) & 0xFFFF;
        let handle = g.call(
            0x102320,
            &[entry + 0xB8, 9, 0, entry + 0x34, entry + 0x7C, others],
        );
        finish_quad(g, entry, handle);
    } else {
        let size = g.read(entry + 0x88);
        let handle = g.call(
            0xFFDA0,
            &[entry + 0xAC, 0, entry + 0x34, entry + 0x7C, size, 0xFFFF],
        );
        g.write(entry + 0xAC, handle);
        let handle = g.call(
            0x102320,
            &[entry + 0xB8, 9, 0, entry + 0x34, entry + 0x7C, 0xFFFF],
        );
        finish_quad(g, entry, handle);
    }
}

fn finish_quad(g: &mut dyn Guest, entry: u32, handle: u32) {
    g.write(entry + 0xB8, handle);
    let size = g.read(entry + 0x88);
    let stored = g.read(entry + 0xB8);
    g.call(0xFF420, &[stored, size]);
}

/// The value `[+4]` takes for the types of `0xE1650`: `0x1B -> 0`, `0x1C -> 2`, `0x1D -> 3`, `0x1E -> 1`, `0x1F -> 4`, `0x2D -> 5`, `0x2E -> 6`, others unchanged.
fn trail_kind(entry_type: u32) -> Option<u32> {
    match entry_type {
        0x1B => Some(0),
        0x1C => Some(2),
        0x1D => Some(3),
        0x1E => Some(1),
        0x1F => Some(4),
        0x2D => Some(5),
        0x2E => Some(6),
        _ => None,
    }
}

/// `0xE1650` (types `0x1B .. 0x1F`, `0x2D`, `0x2E`): `+4` takes the trail kind of the type, then two trail sound handles `0x10A230` (viewer split as `0xE14B0`)
/// into `+0xB4` / `+0xB8`, `+0xC |= 0x10000`.
pub fn init_trail_sounds(g: &mut dyn Guest, entry: u32) {
    let entry_type = g.read(entry);
    if let Some(kind) = trail_kind(entry_type) {
        g.write(entry + 4, kind);
    }
    let viewer_entry = g.read(entry + 0x100) == 1 && {
        let source = g.read(entry + 0x108);
        let block = g.read(source + 0x150);
        (g.read(block + 4) as i32) >= 0
    };
    if viewer_entry {
        let bit = view_bit(g);
        let size = g.read(entry + 0x88);
        let kind = g.read(entry + 4);
        let handle = g.call(
            0x10A230,
            &[entry + 0xB4, 0, kind, entry + 0x28, entry + 0x70, size, bit],
        );
        g.write(entry + 0xB4, handle);
        let others = !view_bit(g) & 0xFFFF;
        let size = g.read(entry + 0x88);
        let kind = g.read(entry + 4);
        let handle = g.call(
            0x10A230,
            &[
                entry + 0xB8,
                0,
                kind,
                entry + 0x34,
                entry + 0x7C,
                size,
                others,
            ],
        );
        g.write(entry + 0xB8, handle);
    } else {
        let size = g.read(entry + 0x88);
        let kind = g.read(entry + 4);
        let handle = g.call(
            0x10A230,
            &[
                entry + 0xB8,
                0,
                kind,
                entry + 0x34,
                entry + 0x7C,
                size,
                0xFFFF,
            ],
        );
        g.write(entry + 0xB8, handle);
    }
    let flags = g.read(entry + 0xC) | 0x10000;
    g.write(entry + 0xC, flags);
}

/// `0xE2DF0` (types `0x20`, `0x21`): the part object by the spawn routine `0xC1950(1, 0x40, 0x3A or 0x15, 0, entry + 0x34, 0xBDA30(2), 0x21 or 0x20, 1, 0, 0)`
/// (type `0x21` takes `0x15` and `0x20`), aimed along the velocity (pitch, `-yaw - 90`): object `+0xC0 / +0x70 / +0xC8 = 0`, byte `+0x32 = 2`, and the
/// object's block `+0xBC = entry`, `+0xC0 = 0`.
pub fn init_part(g: &mut dyn Guest, entry: u32) {
    let angles = FRAME - 0xC;
    let (model, kind) = if g.read(entry) == 0x21 {
        (0x15, 0x20)
    } else {
        (0x3A, 0x21)
    };
    let (pitch, yaw) = velocity_angles(g, entry, |yaw| {
        -yaw - f64::from(f32::from_bits(0x42B4_0000))
    });
    g.writef(angles, pitch);
    g.writef(angles + 4, yaw);
    let variant = g.call(0xBDA30, &[2]);
    let object = g.call(
        0xC1950,
        &[1, 0x40, model, 0, entry + 0x34, variant, kind, 1, 0, 0],
    );
    let pitch_word = g.read(angles);
    g.write(entry + 0xA8, object);
    let block = g.read(object + 0x7C);
    g.write(object + 0xC0, pitch_word);
    let yaw_word = g.read(angles + 4);
    g.write(object + 0x70, yaw_word);
    g.writef(object + 0xC8, 0.0);
    super::eff_guest::write_byte(g, object + 0x32, 2);
    g.write(block + 0xBC, entry);
    g.write(block + 0xC0, 0);
}

/// `0xE34D0` (types `0xC`, `0xD`, `0xF`): owner object `0x746` (`0x74B` for type `0xF`) at the position `+0x1C`, `0x966E0([owner + 4], 1, 0)`, spin `+0xCC = 0`,
/// `+0xD0 = 11.5`, flags `|= 0x40020000`.
pub fn init_debris(g: &mut dyn Guest, entry: u32) {
    let id = if g.read(entry) == 0xF { 0x74B } else { 0x746 };
    let owner = make_owner(g, entry, id, 0x1C);
    let handle = g.at(entry + 0xA8, 4);
    let _ = owner;
    g.call(0x966E0, &[handle, 1, 0]);
    let owner = g.read(entry + 0xA8);
    g.writef(owner + 0xCC, 0.0);
    let owner = g.read(entry + 0xA8);
    g.writef(owner + 0xD0, f32::from_bits(0x4138_0000));
    let owner = g.read(entry + 0xA8);
    let flags = g.read(owner + 0x28) | 0x4002_0000;
    g.write(owner + 0x28, flags);
}

/// `0xE3570` (type `0x14`): owner object `0x74C`, the spins `+0x8C` / `+0x94` take `(random - 0.5) * 10` (x87 doubles), `+0x90 = +0x9C = 0`.
pub fn init_orb(g: &mut dyn Guest, entry: u32) {
    let owner = make_owner(g, entry, 0x74C, 0x34);
    set_owner_flags(g, owner);
    let half = f64::from(f32::from_bits(0x3F00_0000));
    let ten = f64::from(f32::from_bits(0x4120_0000));
    let draw = g.rng();
    let unit = unsigned_random_unit(draw);
    g.writef(entry + 0x90, 0.0);
    g.writef(entry + 0x8C, ((unit - half) * ten) as f32);
    let draw = g.rng();
    let unit = unsigned_random_unit(draw);
    g.writef(entry + 0x9C, 0.0);
    g.writef(entry + 0x94, ((unit - half) * ten) as f32);
}

/// `0xE3640` (types `0x15`, `0x16`): the velocity times 4.
pub fn init_bolt(g: &mut dyn Guest, entry: u32) {
    let factor = f32::from_bits(0x4080_0000);
    for offset in [0x64, 0x68, 0x6C] {
        let value = g.readf(entry + offset) * factor;
        g.writef(entry + offset, value);
    }
}

/// `0xE36D0` (types `0x18`, `0x2F`): `+0x14C = 1` and the unit direction `+0x180 ..` of the velocity (x87: `1 / sqrt` in f64, one f32 store per component).
pub fn init_beam(g: &mut dyn Guest, entry: u32) {
    g.write(entry + 0x14C, 1);
    let (vx, vy, vz) = (
        f64::from(g.readf(entry + 0x64)),
        f64::from(g.readf(entry + 0x68)),
        f64::from(g.readf(entry + 0x6C)),
    );
    let length = (((vx * vx) + (vy * vy)) + (vz * vz)).sqrt();
    let inverse = f64::from(f32::from_bits(0x3F80_0000)) / length;
    g.writef(entry + 0x180, (inverse * vx) as f32);
    g.writef(entry + 0x184, (inverse * vy) as f32);
    g.writef(entry + 0x188, (inverse * vz) as f32);
}

/// `0xE3AB0` (types `0x19`, `0x1A`): the jet state `+0x1DC .. +0x1F4` cleared, `+0x1F8 = pi`.
pub fn init_jet(g: &mut dyn Guest, entry: u32) {
    for offset in [0x1E4, 0x1E8, 0x1EC, 0x1DC, 0x1F4] {
        g.writef(entry + offset, 0.0);
    }
    g.write(entry + 0x1F0, 0);
    g.writef(entry + 0x1F8, f32::from_bits(0x4049_0FDB));
}

/// `0xE4690` (type `0x2C`): the trace beam state (fade and timers cleared, `+0x1F8 = pi`, the far end `+0x1A0 = origin + velocity * 50`, position = origin) and the
/// 50 spark records at `0x736EA0` (32 bytes: random `A = 3 * 2 * (u - 0.5)`, `B` likewise, `C = (u - 0.5) * 6`, `+0xC .. +0x14 = 0`, `+0x1C = 4`).
pub fn init_trace(g: &mut dyn Guest, entry: u32) {
    let velocity = g.vec3(entry + 0x64);
    let origin_x = g.read(entry + 0x40);
    let origin_y = g.read(entry + 0x44);
    for offset in [0x1E4, 0x1DC, 0x1F4] {
        g.writef(entry + offset, 0.0);
    }
    g.writef(entry + 0x1F8, f32::from_bits(0x4049_0FDB));
    let far = f32::from_bits(0x4248_0000);
    let far_x = velocity[0] * far + g.readf(entry + 0x40);
    g.writef(entry + 0x1A0, far_x);
    let far_y = g.readf(entry + 0x68) * far + g.readf(entry + 0x44);
    g.writef(entry + 0x1A4, far_y);
    let origin_z = g.read(entry + 0x48);
    let far_z = g.readf(entry + 0x6C) * far + g.readf(entry + 0x48);
    g.write(entry + 0x1C, origin_x);
    g.write(entry + 0x1F0, 0);
    g.writef(entry + 0x1A8, far_z);
    g.write(entry + 0x20, origin_y);
    g.write(entry + 0x24, origin_z);
    let half = f64::from(f32::from_bits(0x3F00_0000));
    let three = f32::from_bits(0x4040_0000);
    let mut cursor = 0x0073_6EA4u32;
    loop {
        let draw = g.rng();
        let first = (unsigned_random_unit(draw) - half) * 2.0;
        g.writef(cursor - 4, first as f32);
        let draw = g.rng();
        let second = (unsigned_random_unit(draw) - half) * 2.0;
        let value = g.readf(cursor - 4) * three;
        g.writef(cursor - 4, value);
        g.writef(cursor, second as f32);
        let value = g.readf(cursor) * three;
        g.writef(cursor, value);
        let draw = g.rng();
        let third = unsigned_random_unit(draw);
        for offset in [8, 0xC, 0x10] {
            g.writef(cursor + offset, 0.0);
        }
        g.writef(cursor + 0x18, f32::from_bits(0x4080_0000));
        g.writef(
            cursor + 4,
            ((third - half) * f64::from(f32::from_bits(0x40C0_0000))) as f32,
        );
        cursor += 0x20;
        if cursor >= 0x0073_74E4 {
            break;
        }
    }
}

/// `0xE78F0` (type `0x23`): four sound handles `0x101660(slot, id, 0, position, 0xFFFF)` into `+0xAC`, `+0xB0`, `+0xB4`, `+0xB8` (ids `0x53 0x53 0x54 0x55`), each
/// followed by `0xFF420(handle, level)` with the levels 45, 40, 37, 35; `+0x1AC = 0`.
pub fn init_flash(g: &mut dyn Guest, entry: u32) {
    g.write(entry + 0x1AC, 0);
    for (slot, id, level) in [
        (0xAC, 0x53, 0x4234_0000u32),
        (0xB0, 0x53, 0x4220_0000),
        (0xB4, 0x54, 0x4214_0000),
        (0xB8, 0x55, 0x420C_0000),
    ] {
        let handle = g.call(0x101660, &[entry + slot, id, 0, entry + 0x1C, 0xFFFF]);
        g.write(entry + slot, handle);
        g.call(0xFF420, &[handle, level]);
    }
}

/// `0xECAC0` (type `0x2B`): the drone trail structure (`0x3ED20(0x188, 1, entry, 0xE7CA0)` into `+0x208`, ring cleared, capacity `rate / 2`, stamp = clock), its
/// two sound handles `0x10E1C0(trail + 0x180 / + 0x184)`, the slot index of the entry `(0x378400(0xE7BF0, 0x55) * 0x22C - pool + entry) / 0x22C`,
/// `0x1D5E70(trail + 0x180, position, index, 0.7)` and the wobble phase `+0x1DC = atan2(vx, vz)`.
pub fn init_drone(g: &mut dyn Guest, entry: u32) {
    let trail = g.call(0x3ED20, &[0x188, 1, entry, 0xE7CA0]);
    g.write(entry + 0x208, trail);
    g.writef(trail + 0x178, 0.0);
    g.write(trail + 0x17C, 0);
    g.write(trail + 0x16C, 0);
    let rate = g.read(RATE) as i32 as f32;
    let capacity = cvtt(rate * f32::from_bits(0x3F00_0000));
    g.write(trail + 0x170, capacity as u32);
    g.write(trail + 0x174, 0);
    let clock = g.read(CLOCK);
    g.write(trail + 0x168, clock);
    g.call(0x10E1C0, &[trail + 0x180]);
    g.call(0x10E1C0, &[trail + 0x184]);
    let slot = g.call(0x378400, &[0xE7BF0, 0x55]);
    let offset = slot
        .wrapping_mul(0x22C)
        .wrapping_sub(g.read(super::eff_guest::POOL_BASE))
        .wrapping_add(entry);
    let index = (offset as i32).wrapping_div(0x22C) as u32;
    g.call(0x1D5E70, &[trail + 0x180, entry + 0x1C, index, 0x3F33_3333]);
    let (vx, vz) = (
        f64::from(g.readf(entry + 0x64)),
        f64::from(g.readf(entry + 0x6C)),
    );
    g.writef(entry + 0x1DC, vx.atan2(vz) as f32);
}

/// `0xE2F90` (types `0x26`, `0x29`): the particle shell state: five random draws (`+0x208 .. +0x218`), the velocity copy `+0x21C ..`, size copy `+0x228`, `+0x9C = 2`, the
/// unit basis `0xE0F30` and the anchor `+0x1A0 ..` from the attached player's block or the target object `+0x19C`; `+4 = 0`.
pub fn init_shell(g: &mut dyn Guest, entry: u32) {
    let draw = g.rng();
    let (first_slot, second_slot) = (FRAME - 0x8, FRAME - 0x4);
    g.write(second_slot, draw);
    let first = unsigned_random_unit(draw);
    g.writef(first_slot, first as f32);
    unit_basis(g, entry);
    let one = f64::from(f32::from_bits(0x3F80_0000));
    let half = f64::from(f32::from_bits(0x3F00_0000));
    let second = (unsigned_random_unit(g.rng()) + one) * half;
    g.writef(second_slot, second as f32);
    let draw = g.rng();
    let sign = if draw & 1 != 0 { 1.0f64 } else { -1.0 };
    let spread = sign * f64::from(g.readf(second_slot));
    g.writef(entry + 0x208, spread as f32);
    let third = unsigned_random_unit(g.rng());
    let value =
        (third * f64::from(g.readf(entry + 0x208))) * f64::from(f32::from_bits(0xC000_0000));
    g.writef(entry + 0x20C, value as f32);
    let first = f64::from(g.readf(first_slot));
    g.writef(
        entry + 0x210,
        (first * f64::from(f32::from_bits(0x3FE6_6666)) + f64::from(f32::from_bits(0x3E4C_CCCD)))
            as f32,
    );
    g.writef(
        entry + 0x214,
        ((one - first) * f64::from(f32::from_bits(0x4021_47AE))
            + f64::from(f32::from_bits(0x3E4C_CCCD))) as f32,
    );
    let fifth = unsigned_random_unit(g.rng());
    let (vx, vy, vz) = (
        g.read(entry + 0x64),
        g.read(entry + 0x68),
        g.read(entry + 0x6C),
    );
    g.write(entry + 0x21C, vx);
    let size = g.read(entry + 0x88);
    g.writef(
        entry + 0x218,
        (fifth * f64::from(f32::from_bits(0x40C9_0FDB))) as f32,
    );
    g.write(entry + 0x220, vy);
    g.write(entry + 0x228, size);
    g.write(entry + 0x224, vz);
    g.writef(entry + 0x9C, 2.0);
    let attached = g.read(entry + 0x104);
    if attached != 0 && (g.read(attached + 8) as i32) > (g.read(PLAYER_FIRST) as i32) {
        for (offset, source) in [(0x1A0, 0xB88), (0x1A4, 0xB8C), (0x1A8, 0xB90)] {
            let word = g.read(attached + source);
            g.write(entry + offset, word);
        }
        g.write(entry + 4, 0);
        return;
    }
    let target = g.read(entry + 0x19C);
    if target != 0 && (g.read(target + 0x28) as i32) >= 0 {
        for (offset, anchor, origin) in [
            (0x1A0, 0x1A8, 0x64),
            (0x1A4, 0x1AC, 0x68),
            (0x1A8, 0x1B0, 0x6C),
        ] {
            let value = g.readf(target + anchor) + g.readf(target + origin);
            g.writef(entry + offset, value);
        }
    }
    g.write(entry + 4, 0);
}

const AXIS_UP: [f32; 3] = [0.0, 1.0, 0.0];
const AXIS_SIDE: [f32; 3] = [1.0, 0.0, 0.0];

/// `0xE0F30(entry in eax)`: the unit basis of the velocity `v` into `+0x1C4 .. +0x1D8`. `v' = v / |v|` mixes x87 and SSE products (x and z through the
/// f64 inverse / its f32 rounding, y through the f32 inverse); `t = axis x v'` with the up axis `(0, 1, 0)` (`0x4DEB10`) when `|v'.y| <= 0.7071`, else the
/// side axis `(1, 0, 0)` (`0x4DEB1C`); `s = t x v'`. The axes are image data the host does not map, so they are constants here (verified equal to the image).
pub fn unit_basis(g: &mut dyn Guest, entry: u32) {
    let (vx, vy, vz) = (
        f64::from(g.readf(entry + 0x64)),
        f64::from(g.readf(entry + 0x68)),
        f64::from(g.readf(entry + 0x6C)),
    );
    let inverse =
        f64::from(f32::from_bits(0x3F80_0000)) / (((vx * vx) + (vy * vy)) + (vz * vz)).sqrt();
    let inverse_f = inverse as f32;
    let x = (inverse * vx) as f32;
    let y = inverse_f * g.readf(entry + 0x68);
    let z = (f64::from(inverse_f) * f64::from(g.readf(entry + 0x6C))) as f32;
    let axis = if y.abs() > f32::from_bits(0x3F35_04F3) {
        AXIS_SIDE
    } else {
        AXIS_UP
    };
    let (cross_x, cross_y, cross_z) = (
        axis[2] * y - axis[1] * z,
        axis[0] * z - axis[2] * x,
        axis[1] * x - axis[0] * y,
    );
    g.writef(entry + 0x1C4, cross_x);
    g.writef(entry + 0x1C8, cross_y);
    g.writef(entry + 0x1CC, cross_z);
    let (tx, ty, tz) = (
        g.readf(entry + 0x1C4),
        g.readf(entry + 0x1C8),
        g.readf(entry + 0x1CC),
    );
    g.writef(entry + 0x1D0, ty * z - y * tz);
    g.writef(entry + 0x1D4, x * tz - tx * z);
    g.writef(entry + 0x1D8, tx * y - ty * x);
}
