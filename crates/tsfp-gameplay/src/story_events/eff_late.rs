//! Readable port of the flare draw callback `0xE3270` (T1870).
//! Source-inspected behaviour; renderer services remain Guest calls. See the T1870 findings.
use super::eff_guest::Guest;

/// Draw the flare used by classes 0x26 and 0x29. Arguments to engine services retain push order.
pub fn flare_draw(g: &mut dyn Guest, entry: u32) {
    let (width, height, colour) = if g.read(entry + 4) == 0 {
        (200.0_f32, 200.0_f32, 0x7F6F_BAFF)
    } else {
        // x87 keeps the ratio in double for alpha, but stores it as f32 before the flicker and size calculations.
        let ratio = f64::from(g.readf(entry + 0x98)) / f64::from(g.readf(entry + 0x9C));
        let age = ratio as f32;
        let alpha = ((1.0 - ratio * f64::from(0.1_f32)) * 255.0).trunc() as i32 as u8;
        let random = g.rng();
        let period = (f64::from(age) * 3.0 + 1.0).trunc() as i32 as u32;
        // Preserve the original division fault for an invalid lifetime/age yielding a zero period.
        let colour = if random % period == 0 {
            0x7F00_00FF | (u32::from(alpha) << 16) | (u32::from(alpha) << 8)
        } else {
            0
        };
        ((1.0 - age * 0.7) * 1700.0, (1.0 - age) * 1700.0, colour)
    };
    push(g, 0x19D70, &[2]);
    push(g, 0x19650, &[2]);
    let sprite = push(g, 0x80F20, &[0x28D]);
    push(
        g,
        0x107B60,
        &[
            0,
            colour,
            0,
            height.to_bits(),
            width.to_bits(),
            entry + 0x1C,
            sprite,
        ],
    );
}

/// `0xE3B00`: refresh the beam geometry and its five effect handles during teardown.
/// Despite its destroy-slot registration, this callback builds/updates effect handles.
pub fn beam_destroy(g: &mut dyn Guest, entry: u32) {
    use super::eff_guest::SCRATCH;
    let source = g.read(entry + 0x108);
    let source_link = if source != 0 {
        g.read(source + 0x150)
    } else {
        0
    };
    let actor = if source_link != 0 {
        g.read(source_link + 8)
    } else {
        0
    };
    let source_id = if actor != 0 {
        g.read(actor + 0x214)
    } else {
        u32::MAX
    };
    let current = g.read(0x7B0CC0);
    let first_person = g.read(current + 0x214) == source_id && g.read(current + 0x21C) & 4 != 0;
    let position = entry + if first_person { 0x28 } else { 0x34 };
    let direction = entry + if first_person { 0x70 } else { 0x7C };
    if (g.read(0x4E7944) as i32) < 1 {
        return;
    }
    // Preserve the overlapping copy: the source vector starts at +0x54, destination +0x30.
    g.copy(entry + 0x54, entry + 0x30, 4);
    g.copy(entry + 0x40, entry + 0x1C, 5);
    let end = g.vec3(entry + 0x1A0);
    let start = g.vec3(entry + 0x58);
    let delta = std::array::from_fn(|i| end[i] - start[i]);
    let length = norm(delta);
    g.writef(entry + 0x1EC, length);
    let inverse = 1.0 / length;
    let unit = delta.map(|v| v * inverse);
    g.set_vec3(entry + 0x7C, unit);
    let other = if source_link != 0 {
        let start = g.vec3(entry + 0x4C);
        let delta = std::array::from_fn(|i| end[i] - start[i]);
        let length = norm(delta);
        g.writef(entry + 0x1E8, length);
        let inverse = 1.0 / length;
        delta.map(|v| v * inverse)
    } else {
        g.writef(entry + 0x1E8, length);
        unit
    };
    g.set_vec3(entry + 0x70, other);
    let d = g.vec3(direction);
    // Basis cross products retain the compiler's zero multiplies (signed-zero/NaN behaviour).
    let (a, b) = basis(d, 0.707);
    let scratch = SCRATCH + 0xA00;
    g.set_vec3(scratch, a);
    g.set_vec3(scratch + 12, b);
    let mut primary = 0;
    let mut secondary = 0;
    if (push(g, 0x1CDAE0, &[]) as i32) <= 2 && push(g, 0x3533F0, &[]) == 0 {
        secondary = 2;
        if g.read(entry + 0x100) == 1 {
            primary = 3;
        }
    }
    for index in 0..primary + secondary {
        let handle_word = entry + 0xAC + index * 4;
        let mut handle = g.read(handle_word);
        let first = index < primary;
        if handle == 0 && (first || g.read(entry + 0xA0) != 0) {
            let random = f64::from(g.rng()) * f64::from(f32::from_bits(0x2F80_0000));
            let size = if first {
                ((random + 1.0) * f64::from(0.2_f32)) as f32
            } else {
                (random * f64::from(0.6_f32) + 1.0) as f32
            };
            let type19 = g.read(entry) == 0x19;
            let kind = if first {
                let random = g.rng() % 3;
                if type19 {
                    if random == 0 { 5 } else { 4 }
                } else if random == 0 {
                    1
                } else {
                    0
                }
            } else if type19 {
                6
            } else {
                2
            };
            let (red, green, blue) = if type19 {
                let green = g.rng() % 0x3F;
                let red = g.rng() & 0x2F;
                (red, green, 0xFF)
            } else {
                let blue = g.rng() & 0x2F;
                let green = g.rng() % 0x3F;
                (0xFF, green, blue)
            };
            handle = push(g, 0x102880, &[0xFFFF, position, 0, handle_word]);
            g.write(handle_word, handle);
            push(
                g,
                0x12B9D0,
                &[
                    (red << 16) | (green << 8) | blue,
                    kind,
                    size.to_bits(),
                    handle,
                ],
            );
            // Engine leaves may rewrite the entry/handle words.
            handle = g.read(handle_word);
        }
        if handle != 0 {
            let flags = g.read(entry + 0x1FC);
            push(
                g,
                0x12BDC0,
                &[
                    scratch + 12,
                    scratch,
                    flags,
                    direction,
                    entry + 0x1A0,
                    position,
                    handle,
                ],
            );
        }
    }
}

fn norm(v: [f32; 3]) -> f32 {
    let v = v.map(f64::from);
    (v[2] * v[2] + v[1] * v[1] + v[0] * v[0]).sqrt() as f32
}

/// `0xED510`: draw a ribbon over every second point in the object's circular trail.
pub fn trail_draw(g: &mut dyn Guest, entry: u32) {
    let trail = g.read(entry + 0x208);
    let count_word = if g.read(trail + 0x174) != 0 {
        trail + 0x170
    } else {
        trail + 0x16C
    };
    let pairs = (g.read(count_word).wrapping_sub(1) as i32) / 2;
    if pairs <= 1 {
        return;
    }
    let state1 = push(g, 0x19D70, &[2]);
    let state2 = push(g, 0x19650, &[2]);
    let state3 = push(g, 0x1CD60, &[0]);
    let capacity = g.read(trail + 0x170) as i32;
    let mut point = g.read(trail + 0x16C).wrapping_sub(1) as i32;
    if point < 0 {
        point = point.wrapping_add(capacity);
    }
    let mut stop = point.wrapping_sub(pairs.wrapping_mul(2));
    if stop < 0 {
        stop = stop.wrapping_add(capacity);
    }
    let sprite = push(g, 0x80F20, &[0]);
    push(
        g,
        0x1CD90,
        &[sprite, pairs.wrapping_mul(2) as u32, 0, 0x92, 2],
    );
    let mut index = 0i32;
    while point != stop {
        let ratio = (index.wrapping_sub(2) as f32) / (pairs.wrapping_sub(4) as f32);
        let fade = if index as f32 > pairs as f32 * 0.5 {
            1.0 - ratio
        } else {
            ratio
        };
        let fade = if fade > 0.0 { fade } else { 0.0 };
        let colour = 0x7F7F_4600 | (((f64::from(fade) * 2.0 * 63.0).trunc() as i32 as u32) & 0xFF);
        let swapped = colour.swap_bytes();
        let p = trail.wrapping_add((point as u32).wrapping_mul(12));
        let xyz = g.vec3(p);
        for word in [
            xyz[0].to_bits(),
            xyz[1].to_bits(),
            xyz[2].to_bits(),
            swapped,
            0,
            0,
            xyz[0].to_bits(),
            (xyz[1] + 0.5).to_bits(),
            xyz[2].to_bits(),
            0x0046_7F7F,
            0,
            0,
        ] {
            let output = g.read(0x55F974);
            g.write(output, word);
            g.write(0x55F974, output.wrapping_add(4));
        }
        point = point.wrapping_sub(2);
        if point < 0 {
            point = point.wrapping_add(capacity);
        }
        index = index.wrapping_add(1);
    }
    push(g, 0x1D380, &[]);
    push(g, 0x19D70, &[state1]);
    push(g, 0x19650, &[state2]);
    push(g, 0x1CD60, &[state3]);
}

/// `0xE29D0`: project three orbiting flare quads into the current viewport.
pub fn orbit_draw(g: &mut dyn Guest, entry: u32) {
    use super::eff_guest::SCRATCH;
    if push(g, 0x65250, &[]) == 0 {
        return;
    }
    let sprite = push(g, 0x80F20, &[0x28D]);
    let table = g.at(0x4B85C8, 0);
    let record = table.wrapping_add(sprite.wrapping_mul(0x34));
    let width = (g.read(record + 0x24) >> 16) & 0xFFFF;
    let height = (g.read(record + 0x28) >> 16) & 0xFFFF;
    let d = g.vec3(entry + 0x64);
    let zero_y = d[1] * 0.0;
    let a = if d[1].abs() > 0.70710677_f32 {
        [zero_y - d[2] * 0.0, d[2] - d[0] * 0.0, d[0] * 0.0 - d[1]]
    } else {
        [zero_y - d[2], d[2] * 0.0 - d[0] * 0.0, d[0] - zero_y]
    };
    let b = [
        d[2] * a[1] - d[1] * a[2],
        d[0] * a[2] - d[2] * a[0],
        d[1] * a[0] - d[0] * a[1],
    ];
    push(g, 0x19D70, &[2]);
    push(g, 0x19650, &[2]);
    push(g, 0x1ED50, &[sprite]);
    let scratch = SCRATCH + 0xB00;
    for index in 0..3 {
        let age = g.readf(entry + 0x98);
        let angle = ((index as f32 * (1.0_f32 / 3.0) * std::f32::consts::TAU
            + age * std::f32::consts::TAU)
            * 325.9493_f32
            + 0.5) as i32 as u32;
        let sin = g.readf(0x7A38A0 + (angle & 0x7FF) * 4) * 0.3;
        let cos = g.readf(0x7A38A0 + (angle.wrapping_add(0x200) & 0x7FF) * 4) * 0.3;
        let center = g.vec3(entry + 0x1C);
        g.set_vec3(
            scratch,
            std::array::from_fn(|i| sin * b[i] + cos * a[i] + center[i]),
        );
        let current = g.read(0x7B0CC0);
        let matrix = g.read(current + 0x20C);
        push(g, 0x151FC0, &[scratch + 16, scratch, matrix]);
        let projected = g.vec3(scratch + 16);
        let w = g.readf(scratch + 28);
        // COMISS unordered takes the rejection branches too.
        if !(projected.iter().all(|v| *v >= 0.0 - w && w >= *v) && projected[2] > 0.0) {
            continue;
        }
        let alpha_scale = if 1.0 > w { w } else { 1.0 };
        let inverse = 1.0 / w;
        let size = (g.readf(current + 0x268) * inverse * 100.0) as i32 as f32;
        let x = projected[0] * inverse;
        let y = projected[1] * inverse;
        let z = projected[2] * inverse;
        let viewport = g.read(0x7E71C8);
        let vx = g.read(viewport + 0x10) as i32 as f32 * x + g.read(viewport + 8) as i32 as f32;
        let vy = g.read(viewport + 0x24).wrapping_neg() as i32 as f32 * y
            + g.read(viewport + 0x1C) as i32 as f32;
        for (offset, xyz) in [
            (32, [vx + size, vy + size, z]),
            (44, [vx + size, vy - size, z]),
            (56, [vx - size, vy + size, z]),
            (68, [vx - size, vy - size, z]),
        ] {
            g.set_vec3(scratch + offset, xyz);
        }
        let colour = [0x1F5F7F, 0x0F4F7F, 0x0F6F6F][index]
            | (((f64::from(alpha_scale) * 79.0).trunc() as i32 as u32) << 24);
        push(
            g,
            0x167B0,
            &[
                scratch + 68,
                scratch + 56,
                scratch + 44,
                scratch + 32,
                colour,
                height,
                width,
            ],
        );
    }
}

fn first_person(g: &mut dyn Guest, entry: u32) -> bool {
    let source = g.read(entry + 0x108);
    let link = if source != 0 {
        g.read(source + 0x150)
    } else {
        0
    };
    let actor = if link != 0 { g.read(link + 8) } else { 0 };
    let id = if actor != 0 {
        g.read(actor + 0x214)
    } else {
        u32::MAX
    };
    let current = g.read(0x7B0CC0);
    g.read(current + 0x214) == id && g.read(current + 0x21C) & 4 != 0
}
fn basis(d: [f32; 3], cutoff: f32) -> ([f32; 3], [f32; 3]) {
    let a = if d[1].abs() >= cutoff {
        [
            d[1] - d[2] * 0.0,
            d[2] * 0.0 - d[0],
            d[0] * 0.0 - d[1] * 0.0,
        ]
    } else {
        [
            d[1] * 0.0 - d[2],
            d[2] * 0.0 - d[0] * 0.0,
            d[0] - d[1] * 0.0,
        ]
    };
    let b = [
        d[2] * a[1] - d[1] * a[2],
        d[0] * a[2] - d[2] * a[0],
        d[1] * a[0] - d[0] * a[1],
    ];
    (a, b)
}
fn vertex(g: &mut dyn Guest, p: [f32; 3], colour: u32, u: f32, v: f32) {
    for word in [
        p[0].to_bits(),
        p[1].to_bits(),
        p[2].to_bits(),
        colour,
        u.to_bits(),
        v.to_bits(),
    ] {
        let output = g.read(0x55F974);
        g.write(output, word);
        g.write(0x55F974, output.wrapping_add(4));
    }
}
/// `0xE40E0`: two rotating textured strips along a beam.
pub fn flame_beam_draw(g: &mut dyn Guest, entry: u32) {
    let sprite = push(g, 0x80F20, &[0x342]);
    let fp = first_person(g, entry);
    let length = g.readf(entry + if fp { 0x1E8 } else { 0x1EC });
    let position = entry + if fp { 0x28 } else { 0x34 };
    let direction = entry + if fp { 0x70 } else { 0x7C };
    let blend = g.readf(entry + 0x1FC);
    let inverse = 1.0 - blend;
    push(g, 0x19650, &[2]);
    push(g, 0x19D70, &[2]);
    g.call(0xE3730, &[1f32.to_bits(), entry]); // stack scale, eax = entry
    let base = f64::from(inverse) * 63.0;
    let red = (f64::from(inverse) * 247.0 + f64::from(blend) * 15.0).trunc() as i32 as u32;
    let green = (f64::from(blend) * 47.0 + base).trunc() as i32 as u32;
    let blue = (f64::from(blend) * 255.0 + base).trunc() as i32 as u32;
    let rgb = ((red << 8 | green) << 8) | blue;
    let alpha = g.read(entry + 0x1F0) << 24;
    if !(length > 0.0) {
        return;
    }
    let d = g.vec3(direction);
    let (a, b) = basis(d, 0.707);
    let short = g.readf(0x4DFB20 + u32::from(fp) * 4);
    let segments = if short >= length {
        2
    } else if 50.0 > length {
        3
    } else {
        4
    };
    let current = g.read(0x7B0CC0);
    let matrix = g.read(current + 0x208);
    push(g, 0x1C6F0, &[matrix]);
    for strip in 0..2 {
        let angle = ((g.readf(0x4DFB28 + strip * 4) * 0.017453292
            + g.readf(entry + 0x98) * 4.0
            + g.readf(entry + 0x1DC))
            * 325.9493
            + 0.5) as i32 as u32;
        let cos = g.readf(0x7A38A0 + (angle.wrapping_add(0x200) & 0x7FF) * 4);
        let sin = g.readf(0x7A38A0 + (angle & 0x7FF) * 4);
        let texture_base = strip as f32 * 0.5 + g.readf(entry + 0x1E4);
        push(g, 0x1CD90, &[sprite, segments * 2, 0, 0x92, 2]);
        for index in 0..segments {
            let endpoint = index == 0 || index == 3;
            let distance = if index == 0 {
                0.0
            } else if index == segments - 1 {
                length
            } else if index == 1 {
                short
            } else {
                48.0
            };
            let u = distance * 0.2 + texture_base;
            let radius = if endpoint { 0.05 } else { 0.25 };
            let colour = if endpoint { rgb } else { rgb | alpha };
            let origin = g.vec3(position);
            let center: [f32; 3] = std::array::from_fn(|i| d[i] * distance + origin[i]);
            let ca = radius * cos;
            let sb = radius * sin;
            let plus = std::array::from_fn(|i| (ca * a[i] + sb * b[i]) + center[i]);
            let minus = std::array::from_fn(|i| (center[i] - ca * a[i]) - sb * b[i]);
            vertex(g, plus, colour, u, 0.0);
            vertex(g, minus, colour, u, 1.0);
        }
        push(g, 0x1D380, &[]);
    }
}

fn sine(g: &mut dyn Guest, angle: f32) -> (f32, f32) {
    let i = (angle * 325.9493 + 0.5) as i32 as u32;
    (
        g.readf(0x7A38A0 + (i & 0x7FF) * 4),
        g.readf(0x7A38A0 + (i.wrapping_add(0x200) & 0x7FF) * 4),
    )
}
/// `0xE4EA0`: travelling beam rings and the billboard cloud at the advancing endpoint.
pub fn ring_beam_draw(g: &mut dyn Guest, entry: u32) {
    use super::eff_guest::SCRATCH;
    let state2 = push(g, 0x19650, &[2]);
    let state1 = push(g, 0x19D70, &[2]);
    let p = g.vec3(entry + 0x1C);
    let previous = g.vec3(entry + 0x40);
    let difference: [f64; 3] = std::array::from_fn(|i| f64::from(p[i]) - f64::from(previous[i]));
    let progress =
        ((difference.iter().map(|v| v * v).sum::<f64>()).sqrt() * f64::from(0.02_f32)) as f32;
    let progress = progress.clamp(0.0, 1.0);
    let remaining = 1.0 - progress;
    let mut player = 0u32;
    while (player as i32) < g.read(0x790950) as i32 {
        let base = g.read(0x7B0C48).wrapping_add(player * 0x1584);
        push(
            g,
            0xD1EF0,
            &[
                (remaining * 0.04).to_bits(),
                (remaining * 2.0).to_bits(),
                0.1f32.to_bits(),
                base,
            ],
        );
        player += 1;
    }
    let alpha = (f64::from(remaining) * 127.0).trunc() as i32 as u32 & 0xFF;
    let colour = 0x7F7F6F00 | alpha;
    let (sin, cos) = sine(g, progress * 0.15707964);
    let direction = g.vec3(entry + 0x64);
    let raw = [
        direction[1] * 0.0 - direction[2] * sin,
        direction[2] * cos - direction[0] * 0.0,
        direction[0] * sin - direction[1] * cos,
    ];
    let length = (raw[0] * raw[0] + raw[1] * raw[1] + raw[2] * raw[2]).sqrt();
    // Readable target uses exact reciprocal sqrt rather than the original approximate rsqrtss.
    let mut radial = raw.map(|v| v * (4.0 / length));
    let current = g.read(0x7B0CC0);
    let camera = g.read(current + 0x208);
    push(g, 0x1C6F0, &[camera]);
    push(g, 0x19650, &[2]);
    push(g, 0x19D70, &[2]);
    let scratch = SCRATCH + 0xC00;
    let matrix = scratch + 64;
    g.set_vec3(scratch, direction);
    g.set_vec3(scratch + 16, radial);
    g.writef(scratch + 32, cos);
    g.writef(scratch + 36, sin);
    g.writef(scratch + 40, 0.0);
    push(
        g,
        0x152770,
        &[f32::from_bits(0x3E20D97C).to_bits(), scratch, matrix],
    );
    g.set_vec3(scratch + 48, radial.map(|v| v * 0.125));
    push(g, 0x152090, &[scratch + 48, matrix]);
    push(g, 0x152090, &[scratch + 48, matrix]);
    let mut tip_radial = g.vec3(scratch + 48);
    let start = g.vec3(entry + 0x34);
    let end = g.vec3(entry + 0x1A0);
    for _ in 0..20 {
        let u = (f64::from(g.rng()) * f64::from(f32::from_bits(0x2F800000))) as f32;
        let sprite = push(g, 0x80F20, &[0x33D]);
        push(g, 0x1CD90, &[sprite, 4, 0, 0x92, 2]);
        vertex(
            g,
            std::array::from_fn(|i| radial[i] + start[i]),
            0x1F6F7F7F,
            u,
            0.0,
        );
        vertex(
            g,
            std::array::from_fn(|i| start[i] - radial[i]),
            0x1F6F7F7F,
            u,
            1.0,
        );
        vertex(
            g,
            std::array::from_fn(|i| end[i] + tip_radial[i]),
            0x006F7F7F,
            u + 1.0,
            0.5,
        );
        vertex(
            g,
            std::array::from_fn(|i| end[i] - tip_radial[i]),
            0x006F7F7F,
            u + 1.0,
            0.5,
        );
        push(g, 0x1D380, &[]);
        for segment in 0..4 {
            let (s1, c1) = sine(g, segment as f32 * 0.25 * std::f32::consts::PI);
            let (s2, c2) = sine(g, (segment + 1) as f32 * 0.25 * std::f32::consts::PI);
            let center: [f32; 3] =
                std::array::from_fn(|i| end[i] * progress + remaining * start[i]);
            let sprite = push(g, 0x80F20, &[0x33D]);
            push(g, 0x1CD90, &[sprite, 4, 0, 0x92, 2]);
            for (s, c, v) in [(s1, c1, 1.0), (-s1, c1, 0.0), (s2, c2, 1.0), (-s2, c2, 0.0)] {
                let xyz = std::array::from_fn(|i| {
                    s * 1.4 * radial[i] + ((0.0 - c * 2.0) * direction[i] + center[i])
                });
                vertex(g, xyz, colour.swap_bytes(), 0.5, v);
            }
            push(g, 0x1D380, &[]);
        }
        g.set_vec3(scratch + 16, radial);
        push(g, 0x152090, &[scratch + 16, matrix]);
        radial = g.vec3(scratch + 16);
        push(g, 0x152090, &[scratch + 32, matrix]);
        g.set_vec3(scratch + 48, tip_radial);
        push(g, 0x152090, &[scratch + 48, matrix]);
        tip_radial = g.vec3(scratch + 48);
    }
    let center: [f32; 3] = std::array::from_fn(|i| end[i] * progress + remaining * start[i]);
    push(g, 0x35DE70, &[0x7F00007F, entry + 0x1A0, entry + 0x34]); // 0x389B0 is a bare ret
    let alpha = (remaining * 2.0).min(1.0);
    let colour = (0x7F7F6F00 | ((f64::from(alpha) * 255.0).trunc() as i32 as u32)).swap_bytes();
    for record in (0x736EBC..0x7374FC).step_by(0x20) {
        let offset = g.vec3(record - 0x1C);
        let center: [f32; 3] = std::array::from_fn(|i| offset[i] + center[i]);
        let size = g.readf(record);
        let sprite = push(g, 0x80F20, &[0x28F]);
        push(g, 0x1CD90, &[sprite, 4, 0, 0x92, 2]);
        let current = g.read(0x7B0CC0);
        let right = g.vec3(current + 0x364);
        let up = g.vec3(current + 0x370);
        for (x, y, u, v) in [
            (size, -size, 0.0, 0.0),
            (size, size, 1.0, 0.0),
            (-size, -size, 0.0, 1.0),
            (-size, size, 1.0, 1.0),
        ] {
            vertex(
                g,
                std::array::from_fn(|i| right[i] * x + (up[i] * y + center[i])),
                colour,
                u,
                v,
            );
        }
        push(g, 0x1D380, &[]);
    }
    push(g, 0x19D70, &[state1]);
    push(g, 0x19650, &[state2]);
}

fn faded_colour(g: &mut dyn Guest, scale: f32) -> u32 {
    // The original invokes the helper four times; retain its observable call count/order.
    let a = push(g, 0x154FE0, &[scale.to_bits(), 0x32327F7F]);
    let b = push(g, 0x154FE0, &[scale.to_bits(), 0x32327F7F]);
    let c = push(g, 0x154FE0, &[scale.to_bits(), 0x32327F7F]);
    let d = push(g, 0x154FE0, &[scale.to_bits(), 0x32327F7F]);
    (((a & 0xFF0000) | (b >> 16)) >> 8) | (((c << 16) | (d & 0xFF00)) << 8)
}
/// `0xE6B30`: two screen flares, a beam strip and sixteen scrolling helices with endpoint sprites.
pub fn helix_beam_draw(g: &mut dyn Guest, entry: u32) {
    use super::eff_guest::SCRATCH;
    let sprite = push(g, 0x80F20, &[0x343]);
    let fp = first_person(g, entry);
    let length = g.readf(entry + if fp { 0x1E8 } else { 0x1EC });
    let position = entry + if fp { 0x28 } else { 0x34 };
    let direction = entry + if fp { 0x70 } else { 0x7C };
    let flare = push(g, 0x80F20, &[0x349]);
    let table = g.at(0x4B85C8, 0);
    let record = table.wrapping_add(flare.wrapping_mul(0x34));
    let width = (g.read(record + 0x24) >> 16) & 0xFFFF;
    let height = (g.read(record + 0x28) >> 16) & 0xFFFF;
    let intensity = g.readf(entry + 0x200) * 0.5;
    push(g, 0x19650, &[2]);
    push(g, 0x19D70, &[1]);
    let alpha = ((f64::from(intensity) * 127.0).trunc() as i32 as u32) << 24;
    let scratch = SCRATCH + 0xD00;
    for layer in 0..2 {
        let current = g.read(0x7B0CC0);
        let radius = g.readf(current + 0x268) * if layer == 0 { 600.0 } else { 150.0 };
        g.copy(position, scratch, 3);
        let matrix = g.read(current + 0x20C);
        push(g, 0x151FC0, &[scratch + 16, scratch, matrix]);
        let p = g.vec3(scratch + 16);
        let w = g.readf(scratch + 28);
        if !p.iter().all(|v| *v >= 0.0 - w && w >= *v) {
            continue;
        }
        let inverse = 1.0 / w;
        let size = (inverse * radius).min(512.0);
        let viewport = g.read(0x7E71C8);
        let x = g.read(viewport + 0x10) as i32 as f32 * (inverse * p[0])
            + g.read(viewport + 8) as i32 as f32;
        let y = g.read(viewport + 0x24).wrapping_neg() as i32 as f32 * (inverse * p[1])
            + g.read(viewport + 0x1C) as i32 as f32;
        let z = inverse * p[2];
        push(g, 0x1ED50, &[flare]);
        let s = g.readf(0x7A38A0) * size;
        let c = g.readf(0x7A40A0) * size;
        for (offset, xyz) in [
            (32, [x + c, y + s, z]),
            (44, [x + s, y - c, z]),
            (56, [x - s, y + c, z]),
            (68, [x - c, y - s, z]),
        ] {
            g.set_vec3(scratch + offset, xyz);
        }
        let colour = if layer == 0 { alpha | 0x141464 } else { alpha };
        push(
            g,
            0x167B0,
            &[
                scratch + 68,
                scratch + 56,
                scratch + 44,
                scratch + 32,
                colour,
                height,
                width,
            ],
        );
    }
    let d = g.vec3(direction);
    let (a, b) = basis(d, 0.707);
    let short = g.readf(0x4DFB20 + u32::from(fp) * 4);
    if length > 0.0 {
        let segments = if short >= length {
            2
        } else if 50.0 > length {
            3
        } else {
            4
        };
        let current = g.read(0x7B0CC0);
        let matrix = g.read(current + 0x208);
        push(g, 0x1C6F0, &[matrix]);
        push(g, 0x1CD90, &[sprite, segments * 4, 0, 0x92, 2]);
        for strip in 0..2 {
            let angle = g.readf(0x4DFB28 + strip * 4) * 0.017453292
                + g.readf(entry + 0x98) * 4.0
                + g.readf(entry + 0x1DC);
            let (sin, cos) = sine(g, angle);
            let ubase = strip as f32 * 0.5 + g.readf(entry + 0x1E4);
            for index in 0..segments {
                let endpoint = index == 0 || index == 3;
                let distance = if index == 0 {
                    0.0
                } else if index == segments - 1 {
                    length
                } else if index == 1 {
                    short
                } else {
                    48.0
                };
                let radius = if endpoint { 0.05 } else { 0.25 };
                let colour = if endpoint {
                    0x7F
                } else {
                    g.read(entry + 0x1F0) << 24 | 0x7F
                };
                let origin = g.vec3(position);
                let center: [f32; 3] = std::array::from_fn(|i| d[i] * distance + origin[i]);
                let ca = radius * cos;
                let sb = radius * sin;
                let u = distance * 0.2 + ubase;
                vertex(
                    g,
                    std::array::from_fn(|i| (ca * a[i] + sb * b[i]) + center[i]),
                    colour,
                    u,
                    0.0,
                );
                vertex(
                    g,
                    std::array::from_fn(|i| (center[i] - ca * a[i]) - sb * b[i]),
                    colour,
                    u,
                    1.0,
                );
            }
        }
        push(g, 0x1D380, &[]);
    }
    push(g, 0x19D70, &[3]);
    let current = g.read(0x7B0CC0);
    let matrix = g.read(current + 0x208);
    push(g, 0x1C6F0, &[matrix]);
    let mut ends = [[0f32; 3]; 16];
    for helix in 0..16 {
        let mut phase = (g.read(0x7DE318) as i32 % 90) as f32 * 0.011111111 + helix as f32 * 0.0625;
        if phase > 1.0 {
            phase -= 1.0;
        }
        let (a, b) = basis(g.vec3(direction), 0.707);
        g.set_vec3(scratch, a);
        g.set_vec3(scratch + 16, b);
        let rotation = scratch + 80;
        push(
            g,
            0x152770,
            &[(helix as f32 * 0.3926991).to_bits(), direction, rotation],
        );
        push(g, 0x151E80, &[scratch, rotation]);
        push(g, 0x151E80, &[scratch + 16, rotation]);
        let a = g.vec3(scratch);
        let b = g.vec3(scratch + 16);
        let sprite = push(g, 0x80F20, &[0x33D]);
        push(g, 0x1CD90, &[sprite, 40, 0, 0x92, 2]);
        for index in 0..20 {
            let u = index as f32 * 0.05263158;
            let t = phase - index as f32 * 0.005263158;
            let cube = t * t * t;
            let limited = cube.min(0.6);
            let angle = limited * 1.6666666;
            let (sin, _) = sine(g, angle * std::f32::consts::TAU);
            let radius = (angle * 0.1 + sin) * 0.4;
            let fade = (1.0 - f64::from(u) * f64::from(u)) as f32;
            let colour = faded_colour(g, fade);
            let distance = cube * 4.0;
            let origin = g.vec3(position);
            let p: [f32; 3] =
                std::array::from_fn(|i| radius * a[i] + (d[i] * distance + origin[i]));
            if index == 0 {
                ends[helix as usize] = p;
            }
            vertex(
                g,
                std::array::from_fn(|i| p[i] - b[i] * 0.025),
                colour,
                u,
                0.0,
            );
            vertex(
                g,
                std::array::from_fn(|i| b[i] * 0.025 + p[i]),
                colour,
                u,
                1.0,
            );
        }
        push(g, 0x1D380, &[]);
    }
    for p in ends {
        let colour = faded_colour(g, 0.4);
        let sprite = push(g, 0x80F20, &[0x28D]);
        push(
            g,
            0x1D3E0,
            &[1, 0, 0, 0, 0.25f32.to_bits(), 1f32.to_bits(), sprite, 1],
        );
        push(
            g,
            0x1D5F0,
            &[
                0,
                0,
                0.3f32.to_bits(),
                colour,
                p[2].to_bits(),
                p[1].to_bits(),
                p[0].to_bits(),
            ],
        );
        push(g, 0x1D380, &[]);
    }
}

// Listings name pushes chronologically; Guest uses callee stack order (last push first).
fn push(g: &mut dyn Guest, va: u32, chronological: &[u32]) -> u32 {
    let args: Vec<u32> = chronological.iter().rev().copied().collect();
    g.call(va, &args)
}
