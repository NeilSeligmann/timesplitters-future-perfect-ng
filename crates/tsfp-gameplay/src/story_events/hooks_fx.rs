//! Effect drivers of the story level hooks, T1845 round 3 (task G). Doc: `docs/t-story-event-hooks-fx.md`.
//!
//! Static reading of `build/default.xbe` (SHA-256 `3cfd001a...1816bc`), no xemu run. Control flow and constants are
//! MEASURED; the game meaning of a field is INFERRED. Everything here is a pure function over explicit inputs, so a
//! headless world can drive it. Randomness is passed in as a closure returning the next `0x153B40` word, and the
//! 2048 entry sine table `0x7A38A0` as a closure, so tests stay deterministic.
//!
//! Covered: `0x1F84E0` "HTIP" light, `0x1F9DA0` "ROOF" timers, `0x21C110` and `0x21D520` (level 5 sparks),
//! `0x1FEB00` bridge hull builder (level 0xE) and the structural part of `0x24ABA0` (level 1 "WRTH" emitters).

/// Step per frame of the frame multiplier: `(1 / [0x4E8CEC]) * step`, with the rate 60 of the image.
fn dt_of(step: f32) -> f32 {
    (1.0 / 60.0) * step
}

/// `u32 * 2^-32` as the unsigned fraction the original builds with `fild` (+2^32 when negative) and `fmul 2^-32`.
fn unit(word: u32) -> f64 {
    word as f64 * 2.328_306_436_538_696_3e-10
}

// ---------------------------------------------------------------------------------------------------------
// 0x1F84E0 "HTIP" (level 0x12, called by 0x1F8320)
// ---------------------------------------------------------------------------------------------------------

/// Model record id the zone test accepts (`0x1F8430`).
pub const HTIP_MODEL: u32 = 0x656;
/// Inclusive animation frame ranges of the four zones (`0x1F8430`, frame from `0x585E0`).
pub const HTIP_ZONE_FRAMES: [(i32, i32); 4] =
    [(0x2A, 0x6E), (0x7D, 0xA5), (0xAF, 0xDC), (0xF0, 0x12C)];
/// Light centres per zone, table `0x502D78` (stride 12, read from the image).
pub const HTIP_CENTERS: [[f32; 3]; 4] = [
    [14.5, -1.23, -4.21],
    [14.5, -2.3, -4.21],
    [13.79, -2.3, -4.21],
    [13.79, -1.23, -4.21],
];
/// Probe point in the node's local space (`[0x49E47C]`, `[0x475D50]`, 0): transformed to world space by the
/// object matrix (rotation about Y of `+0x70 * pi/180` at `+0x64..+0x6C`, composed with the skeleton node matrix).
pub const HTIP_LOCAL_PROBE: [f32; 3] = [0.13, 0.125, 0.0];
/// Added to the probe distance before it is stored in the global `[0x502D74]` (`[0x475D24]`).
pub const HTIP_RADIUS_BIAS: f32 = 0.1;

/// `0x1F8430`: zone of an animation frame for the model, `None` when the model is not 0x656 or the frame is outside.
pub fn htip_zone(model: u32, frame: i32) -> Option<usize> {
    if model != HTIP_MODEL {
        return None;
    }
    HTIP_ZONE_FRAMES
        .iter()
        .position(|&(lo, hi)| frame >= lo && frame <= hi)
}

/// What `0x1F84E0` does to the shared light object `[0x78DD9C]`.
#[derive(Clone, Debug, PartialEq)]
pub enum HtipLight {
    /// Not in a zone: flag `0x400000` of the light cleared and its field `+0x74` set to 0.
    Off,
    /// In a zone: the light position (`0x10E730`) is the zone centre, `+0x74` is 1.0 (`0x10EB90`), flag `0x400000`
    /// set, and the global `[0x502D74]` becomes the probe distance plus 0.1.
    On {
        position: [f32; 3],
        intensity: f32,
        radius_global: f32,
    },
}

/// `0x1F84E0`: `world_probe` is [`HTIP_LOCAL_PROBE`] transformed to world space by the caller.
pub fn htip_step(model: u32, frame: i32, world_probe: [f32; 3]) -> HtipLight {
    let Some(zone) = htip_zone(model, frame) else {
        return HtipLight::Off;
    };
    let centre = HTIP_CENTERS[zone];
    let delta = [
        world_probe[0] - centre[0],
        world_probe[1] - centre[1],
        world_probe[2] - centre[2],
    ];
    let distance = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    HtipLight::On {
        position: centre,
        intensity: 1.0,
        radius_global: distance + HTIP_RADIUS_BIAS,
    }
}

// ---------------------------------------------------------------------------------------------------------
// 0x1F9DA0 "ROOF" (level 0x12, called by 0x1F8320)
// ---------------------------------------------------------------------------------------------------------

/// Tag of the roof object ("ROOF").
pub const ROOF_TAG: u32 = 0x524F_4F46;

/// Per-step decay of the tracked object's `+0x1CC` by the build/mode byte `[0x7DE457]` (0: 0.015, 2: 0.035, else 0.025).
pub fn roof_rate(mode_byte: u8) -> f32 {
    match mode_byte {
        0 => 0.015,
        2 => 0.035,
        _ => 0.025,
    }
}

/// The two pending flags `[0x75C028]` and `[0x75C02C]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoofPending {
    pub first: bool,
    pub second: bool,
}

/// Inputs of one `0x1F9DA0` call. `value` is the `+0x1CC` word of the object `0x277FE0()` returns (same object for
/// both blocks, read again between them).
#[derive(Clone, Copy, Debug)]
pub struct RoofInput {
    pub step: f32,
    pub mode_byte: u8,
    /// `Some(flag +0x1A0)` of the object tagged "ROOF", `None` when absent.
    pub roof_flag: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RoofEffect {
    /// ROOF `+0x1A0` set to 1, pending flag cleared, then `0xBD090(object)` (destroy, INFERRED).
    FirstExpired,
    /// Pending flag cleared, three `0xEFBD0(0x16, 0x17, position, [1,0,0], 0, 0, 0, 1)` bursts, then `0xBD090`.
    SecondExpired { bursts: [[f32; 3]; 3] },
}

/// Burst positions of the second block: `[0x49E4A8..]`, `[0x49E49C..]`, `[0x49E490, 0x47D7F8, 0x49E48C]`.
pub const ROOF_BURSTS: [[f32; 3]; 3] =
    [[-69.5, 2.71, 46.7], [-71.1, 3.2, 42.8], [-73.1, 2.7, 34.0]];

/// `0x1F9DA0`: both blocks decrement the same `value` (when both are pending it falls twice per call).
pub fn roof_tick(pending: &mut RoofPending, value: &mut f32, input: &RoofInput) -> Vec<RoofEffect> {
    let mut effects = Vec::new();
    let drop = dt_of(input.step) * roof_rate(input.mode_byte);
    if pending.first {
        if input.roof_flag == Some(0) {
            *value -= drop;
            if *value < 0.0 {
                pending.first = false;
                effects.push(RoofEffect::FirstExpired);
            }
        }
    }
    if pending.second {
        *value -= drop;
        if *value < 0.0 {
            pending.second = false;
            effects.push(RoofEffect::SecondExpired {
                bursts: ROOF_BURSTS,
            });
        }
    }
    effects
}

// ---------------------------------------------------------------------------------------------------------
// 0x21C110 (level 5, called by the level 5 hook loop)
// ---------------------------------------------------------------------------------------------------------

/// Five-slot spark pool `[0x78CEE4]` ring index, `[0x78CEE8]` positions (x, y pairs), `[0x78CF10]` weights,
/// `[0x78CEE0]` a decaying flash level.
#[derive(Clone, Debug, PartialEq)]
pub struct SparkPool {
    pub flash: f32,
    pub next: usize,
    pub positions: [[f32; 2]; 5],
    pub weights: [f32; 5],
}

impl Default for SparkPool {
    fn default() -> Self {
        Self {
            flash: 0.0,
            next: 0,
            positions: [[0.0; 2]; 5],
            weights: [0.0; 5],
        }
    }
}

/// The rectangle `[0x7E71C8]`: `(x0, y0, width, height)` from dwords `+0, +0x14, +0xC, +0x20`.
#[derive(Clone, Copy, Debug)]
pub struct ScreenRect {
    pub x0: i32,
    pub y0: i32,
    pub width: i32,
    pub height: i32,
}

/// Decay of the weights per call (`[0x475CD0]`) and of the flash (`[0x476040]`).
pub const SPARK_WEIGHT_DECAY: f32 = 0.8;
pub const SPARK_FLASH_DECAY: f32 = 0.9;
/// Open interval of the trigger value `[[0x29B4].+0x30]` (`0.1`, `0.4`).
pub const SPARK_TRIGGER_RANGE: (f32, f32) = (0.1, 0.4);

/// `0x21C110`. `trigger` is `Some(value)` when the whole pointer chain (table entry 0x71, `+4`, `+4`, `+0x14`,
/// block words `+0x29B4` and `+0x2A14`) resolves, else `None`. One call in 15 (rate/4) with the trigger inside the
/// range spawns three sparks.
pub fn spark_pool_tick(
    pool: &mut SparkPool,
    trigger: Option<f32>,
    rect: ScreenRect,
    rng: &mut dyn FnMut() -> u32,
) {
    pool.flash *= SPARK_FLASH_DECAY;
    if let Some(value) = trigger {
        if value > SPARK_TRIGGER_RANGE.0 && SPARK_TRIGGER_RANGE.1 > value {
            let modulus = ((60 + ((60 >> 31) & 3)) >> 2) as u32;
            if rng() % modulus == 0 {
                pool.flash = 1.0;
                for _ in 0..3 {
                    let slot = pool.next;
                    let x = unit(rng()) * rect.width as f64 + rect.x0 as f64;
                    pool.positions[slot][0] = x as f32;
                    let y = unit(rng()) * rect.height as f64 + rect.y0 as f64;
                    pool.positions[slot][1] = y as f32;
                    pool.weights[slot] = 1.0;
                    pool.next = (slot + 1) % 5;
                }
            }
        }
    }
    for weight in &mut pool.weights {
        *weight *= SPARK_WEIGHT_DECAY;
    }
}

// ---------------------------------------------------------------------------------------------------------
// 0x21D520 (level 5, called by the level 5 hook)
// ---------------------------------------------------------------------------------------------------------

/// Anchor table `0x508EA8` (stride 16): `(owner key, position)`.
pub const SPARK_ANCHORS: [(u32, [f32; 3]); 5] = [
    (0x53, [137.5, 46.4, -550.2]),
    (0x59, [252.0, 33.0, -674.0]),
    (0x58, [144.5, 36.0, -716.6]),
    (0x5B, [56.8, 39.7, -755.4]),
    (0x61, [64.7, 27.5, -633.0]),
];

/// `[0x78D020]` enable, `[0x78D024]` timer, `[0x78D028]` accumulated speed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SparkState {
    pub enabled: bool,
    pub timer: f32,
    pub accumulator: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SparkInput {
    /// `[0x4E7944]`: zero skips the whole function.
    pub steps: i32,
    /// `[0x4E794C]` float step (the image holds -1.0, run time value).
    pub step_scale: f32,
    /// Local player 0 block `+0xA8..+0xB0` and `+0x14C..+0x154` (previous position).
    pub position: [f32; 3],
    pub previous: [f32; 3],
}

/// One spark burst.
#[derive(Clone, Debug, PartialEq)]
pub struct SparkBurst {
    /// Sound request: id `0xC47`, scalar `+0x18` 1.5, range `+0x2C` 100.0, at `position`.
    pub position: [f32; 3],
    /// Table `0xEA9C0(9, ..., 16.0, .., owner, 2, 0)` projectile-entry spawn (INFERRED role) and its owner key.
    pub owner: u32,
    pub index: usize,
}

/// Sound id of the burst (`0xC47`).
pub const SPARK_SOUND: u32 = 0xC47;

/// `0x21D520`. `rng` supplies `0x153B40`, `sin_table` the 2048 entry table `0x7A38A0`.
pub fn level5_spark_tick(
    state: &mut SparkState,
    input: &SparkInput,
    rng: &mut dyn FnMut() -> u32,
    sin_table: &dyn Fn(usize) -> f32,
) -> Option<SparkBurst> {
    if input.steps == 0 || !state.enabled {
        return None;
    }
    let dt = dt_of(input.step_scale);
    let delta = [
        input.position[0] - input.previous[0],
        input.position[1] - input.previous[1],
        input.position[2] - input.previous[2],
    ];
    let speed = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    state.timer -= dt;
    let sum = speed / dt + state.accumulator;
    state.accumulator = sum - input.step_scale * sum * 0.011_111_111;
    if !(state.timer < 0.0) {
        return None;
    }
    let angle = unit(rng()) as f32 * 6.283_185_5;
    let radius = ((unit(rng()) as f32 * 0.5 + 0.75) * state.accumulator) * 0.012;
    let index = ((angle * 325.949_3 + 0.5) as i32) as usize;
    let sin = sin_table(index & 0x7FF);
    let cos = sin_table((index + 0x200) & 0x7FF);
    let candidate = [
        cos * radius + input.position[0],
        input.position[1],
        sin * radius + input.position[2],
    ];
    let mut best = f32::MAX;
    let mut chosen = 0usize;
    for (slot, (_, anchor)) in SPARK_ANCHORS.iter().enumerate() {
        let d = [
            input.position[0] - anchor[0],
            input.position[1] - anchor[1],
            input.position[2] - anchor[2],
        ];
        let squared = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        if best > squared {
            best = squared;
            chosen = slot;
        }
    }
    let (owner, anchor) = SPARK_ANCHORS[chosen];
    let mut direction = [
        candidate[0] - anchor[0],
        candidate[1] - anchor[1],
        candidate[2] - anchor[2],
    ];
    let length2 = direction.iter().map(|v| v * v).sum::<f32>();
    if length2 > 1.0e-7 {
        let inverse = 1.0 / length2.sqrt();
        for v in &mut direction {
            *v *= inverse;
        }
    }
    let position = [
        direction[0] * 0.5 + anchor[0],
        direction[1] * 0.5 + anchor[1],
        direction[2] * 0.5 + anchor[2],
    ];
    state.timer = unit(rng()) as f32 + 0.1;
    Some(SparkBurst {
        position,
        owner,
        index: chosen,
    })
}

// ---------------------------------------------------------------------------------------------------------
// 0x1FEB00 bridge hull builder (level 0xE, tags "BDG1", "BDG2", ...)
// ---------------------------------------------------------------------------------------------------------

/// First tag of the post chain ("BDG1"); post `i` has tag `BDG1 + i`.
pub const BRIDGE_TAG: u32 = 0x4244_4731;
/// Defaults used when no next post exists (`[0x49E59C]`, `[0x49E598]`, `[0x49E594]`; z extent `[0x49E5A4]`).
pub const BRIDGE_DEFAULT_E: [f32; 3] = [251.0, 78.69, 81.02];
pub const BRIDGE_DEFAULT_F: [f32; 3] = [0.0, 0.0, 2.875];
/// Sound: id `0x4DF`, range 30.0, volume `ext+0xA8 * (1/7) * 2`.
pub const BRIDGE_SOUND: u32 = 0x4DF;

/// A hull entry of the object's list `[obj+0x1F0]`: four points (x, y, z at record `+0x24`, `+0x3C`, `+0x54`, `+0x6C`)
/// and the footprint in x and z (`+0x14` min x, `+0x18` min z, `+0x1C` max x, `+0x20` max z). The six padding
/// words per point are zero.
#[derive(Clone, Debug, PartialEq)]
pub struct Hull {
    pub points: [[f32; 3]; 4],
    pub min_x: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_z: f32,
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Footprint of four points (the original's chains of compares compute min and max of x and of z).
pub fn hull_from_points(points: [[f32; 3]; 4]) -> Hull {
    let xs = points.map(|p| p[0]);
    let zs = points.map(|p| p[2]);
    let fold = |v: [f32; 4], pick: fn(f32, f32) -> f32| v.into_iter().reduce(pick).unwrap();
    Hull {
        points,
        min_x: fold(xs, f32::min),
        min_z: fold(zs, f32::min),
        max_x: fold(xs, f32::max),
        max_z: fold(zs, f32::max),
    }
}

/// The six vectors of the stack frame: A (`+0x4C`), B (`+0x40`), C (`+0x1C`), D (`+0x34`), E (`+0x10`), F (`+0x28`).
#[derive(Clone, Copy, Debug)]
pub struct BridgeVectors {
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub c: [f32; 3],
    pub d: [f32; 3],
    pub e: [f32; 3],
    pub f: [f32; 3],
}

/// Hulls of one post: the first is `(A+B, C+D, C-D, A-B)` and the second `(C+D, E+F, E-F, C-D)` (MEASURED).
/// `has_next` is false for the last post (the defaults E and F are then used by the caller), `open` is the
/// "ext+0xAC == 1" state, `ahead` the stack word `[esp+0x64]` (1.0 for the last post, 0 when open).
/// With a next post and `ahead > 0`, E and F, and A and B, become the midpoints `(E+C)/2` and `(F+D)/2`; with a
/// next post and `ahead <= 0`, A = E, B = F, E = C, F = D (a degenerate second hull).
pub fn bridge_hulls(mut v: BridgeVectors, has_next: bool, ahead: f32) -> (Hull, Hull) {
    let first = hull_from_points([add(v.a, v.b), add(v.c, v.d), sub(v.c, v.d), sub(v.a, v.b)]);
    if !has_next {
        // Jump straight to the second hull with E and F as given.
    } else if ahead > 0.0 {
        let e = [
            (v.e[0] + v.c[0]) * 0.5,
            (v.e[1] + v.c[1]) * 0.5,
            (v.e[2] + v.c[2]) * 0.5,
        ];
        let f = [
            (v.f[0] + v.d[0]) * 0.5,
            (v.f[1] + v.d[1]) * 0.5,
            (v.f[2] + v.d[2]) * 0.5,
        ];
        v.e = e;
        v.f = f;
    } else {
        v.e = v.c;
        v.f = v.d;
    }
    let second = hull_from_points([add(v.c, v.d), add(v.e, v.f), sub(v.e, v.f), sub(v.c, v.d)]);
    (first, second)
}

/// Defaults for the first post's `A` when its extension word `+0xA8` is non-zero (`[0x49E5B0]`, `[0x49E5AC]`, `[0x49E5A8]`).
pub const BRIDGE_DEFAULT_A: [f32; 3] = [203.48, 69.88, 80.92];
/// z of `B` and of the default `F` (`[0x49E5A4]`) and the value the open post writes to its own `+0xA8` (`[0x49E5A0]`).
pub const BRIDGE_DEFAULT_B: [f32; 3] = [0.0, 0.0, 2.875];
pub const BRIDGE_OPEN_A8: f32 = 5.6;

/// One post of the chain "BDG1", "BDG2", ... (`0x23FF20(BDG1 + i)`).
#[derive(Clone, Copy, Debug)]
pub struct BridgePost {
    /// `+0x64..+0x6C`.
    pub pos: [f32; 3],
    /// Object matrix built by `0xA14B0(buffer, object, 0, 0)` (a general object transform builder, taken as input).
    pub matrix: super::pose::Mat4,
    /// Extension word `+0xA8` (float) of `[object+0x7C]`.
    pub ext_a8: f32,
    /// Extension word `+0xAC`: 1 = open.
    pub ext_ac: u32,
}

/// Result of [`bridge_derive`]: the vectors, the "ahead" word, whether a next post exists and the carry `(A, B)` for the next post.
#[derive(Clone, Copy, Debug)]
pub struct BridgeStep {
    pub vectors: BridgeVectors,
    pub has_next: bool,
    pub ahead: f32,
}

/// `0x1FEB63..0x1FEE5C` (MEASURED): the six vectors of one post. `local` is the three floats at `[model entry +0x78] + 0x30`.
/// `carry` is `(A, B)` from the previous iteration (`None` for the first post: `A` is the post position when its
/// `+0xA8` equals 0, else [`BRIDGE_DEFAULT_A`]; `B = (0, 0, 2.875)`).
/// `C = pos + M(local.x, local.y, 0)`, `D = M(0, 0, local.z)` with the post's matrix; with a next post
/// `E = next.pos + M'(local.x, local.y, 0)`, `F = M'(0, 0, local.z)` and `ahead = next.+0xA8 / 7 * 0.5`;
/// without one `E` and `F` are the defaults and `ahead = 1.0`. An open post (`+0xAC == 1`) forces `ahead = 0`.
pub fn bridge_derive(
    carry: Option<([f32; 3], [f32; 3])>,
    post: &BridgePost,
    next: Option<&BridgePost>,
    local: [f32; 3],
) -> BridgeStep {
    use super::pose::transform_direction;
    let (a, b) = carry.unwrap_or_else(|| {
        let a = if post.ext_a8 == 0.0 {
            post.pos
        } else {
            BRIDGE_DEFAULT_A
        };
        (a, BRIDGE_DEFAULT_B)
    });
    let c = add(
        post.pos,
        transform_direction(&post.matrix, [local[0], local[1], 0.0]),
    );
    let d = transform_direction(&post.matrix, [0.0, 0.0, local[2]]);
    let (e, f, mut ahead) = match next {
        Some(n) => (
            add(
                n.pos,
                transform_direction(&n.matrix, [local[0], local[1], 0.0]),
            ),
            transform_direction(&n.matrix, [0.0, 0.0, local[2]]),
            n.ext_a8 * 0.142_857_15 * 0.5,
        ),
        None => (BRIDGE_DEFAULT_E, BRIDGE_DEFAULT_B, 1.0),
    };
    if post.ext_ac == 1 {
        ahead = 0.0;
    }
    BridgeStep {
        vectors: BridgeVectors { a, b, c, d, e, f },
        has_next: next.is_some(),
        ahead,
    }
}

/// Carry `(A, B)` for the next post after [`bridge_hulls`] (`0x1FF105..0x1FF391`): with a next post and `ahead > 0` both
/// are the midpoints `((E+C)/2, (F+D)/2)`; with `ahead <= 0` they are the old `E` and `F`; the last post has no carry.
pub fn bridge_carry(v: &BridgeVectors, has_next: bool, ahead: f32) -> Option<([f32; 3], [f32; 3])> {
    if !has_next {
        return None;
    }
    if ahead > 0.0 {
        let half = |x: [f32; 3], y: [f32; 3]| {
            [
                (x[0] + y[0]) * 0.5,
                (x[1] + y[1]) * 0.5,
                (x[2] + y[2]) * 0.5,
            ]
        };
        Some((half(v.e, v.c), half(v.f, v.d)))
    } else {
        Some((v.e, v.f))
    }
}

/// Volume of the hull sound: `ext+0xA8 * [0x47D7FC] * [0x475D10]`.
pub fn bridge_sound_volume(ext_a8: f32) -> f32 {
    ext_a8 * 0.142_857_15 * 2.0
}

// ---------------------------------------------------------------------------------------------------------
// 0x24ABA0 "WRTH" emitter pool (level 1)
// ---------------------------------------------------------------------------------------------------------

/// Tags used by the function: "WRTH" (the driver object), "VDMN" and "VNMN" (marker objects), "JBTH".
pub const WRTH_TAGS: [u32; 4] = [0x5752_5448, 0x5644_4D4E, 0x564E_4D4E, 0x4A42_5448];
/// Emitter pool: 6 entries of 0x140 bytes at `[0x78B718]`, count `[0x78B6FC]`; global state `[0x78B6D4]`
/// (0 idle, 1 and 3 written elsewhere, 2 set here when progress passes 4.0).
pub const WRTH_STATE_DONE_PROGRESS: f32 = 4.0;
/// Progress scale (`[0x47D1E0]`) used for the first player kind.
pub const WRTH_KIND_SCALE: f32 = 64.0;

/// State 1 integrator of one emitter (`24B172..24B2E7`, MEASURED): per call `progress_global += dt*steps/count`;
/// when it passes 0.5 the emitter fields advance. `kind_one` selects the scale 64.0, else 1.0.
/// Returns the new `(+0x20, +0x24, +0x64, finished)` for an emitter with state words `(w20, w24, w40, w44, w48)`.
pub fn wrth_emitter_step(
    w20: f32,
    w24: f32,
    w40: f32,
    w44: f32,
    w48: f32,
    kind_one: bool,
    dt: f32,
    steps: f32,
) -> (f32, f32, f32, bool) {
    let scale = if kind_one { WRTH_KIND_SCALE } else { 1.0 };
    let shape = |value: f32| (value * 0.25 * 0.6 + 0.4) * dt * steps * scale;
    let new20 = shape(w20) * w44 + w20;
    let new24 = shape(new20) * w48 + w24;
    let _ = w40;
    (new20, new24, new20 * 0.25, new20 > WRTH_STATE_DONE_PROGRESS)
}

/// Residual unknowns of `0x24ABA0` after T1845 round 4 (the whole body is now modelled by [`wrth_frame`]).
pub const WRTH_UNSUPPORTED: &[(&str, &str)] = &[
    (
        "0x873C0 / 0x873F0",
        "zone lookup and zone bounds are queried through WrthWorld, bodies not decoded here",
    ),
    (
        "0xA3050 / 0xC3E60",
        "collision sweep and ground height are queried through WrthWorld; the bodies are ported (T1846 batch 8): callees_t1846::sweep_objects + world_queries::sweep_body, world_queries::CollisionWorld::region_height_query",
    ),
    (
        "[0x78B6C4..0x78B6CC]",
        "follow velocity: read by the function, written elsewhere (input of the model)",
    ),
];

use super::pose::{rotation_axis_angle, rotation_y, transform_direction};

/// `[0x47E2AC]`: a player is a target candidate only with x below this.
pub const WRTH_CANDIDATE_X_LIMIT: f32 = 13.5;
/// Radius of the sweep (`0x40000000` = 2.0), hit weight `1 - |hit+0x18| * 0.5`, ground distance cap 1.0, blend 0.9 / 0.1.
pub const WRTH_ALPHA_BLEND: (f32, f32) = (0.9, 0.1);
/// Wall margin of the clamp (`[0x475D24]`).
pub const WRTH_WALL_MARGIN: f32 = 0.1;
/// Release threshold of the life timer `+0x68` and the sound parameters (id `0x14E`, range 25.0, pitch 1.0).
pub const WRTH_RELEASE_TIMER: f32 = 2.0;
pub const WRTH_SOUND_ID: u32 = 0x14E;
pub const WRTH_SOUND_RANGE: f32 = 25.0;
/// Turn angle (radians, `0x3DA0D97C` = pi/40) used when the emitter is pushed away from the target.
pub const WRTH_PUSH_ANGLE: f32 = 0.078_539_82;

/// One emitter of the pool (`0x140` bytes, only the fields this function uses). State: 0 = fresh, 1 = lead, 2 = free.
#[derive(Clone, Debug, PartialEq)]
pub struct WrthEmitter {
    pub state: u32,
    /// `+0x04`: zone id written by `0x873C0`.
    pub zone: u32,
    /// `+0x18`: the key (zone/player id) found in the key table.
    pub key: u32,
    /// `+0x1C..+0x24`.
    pub pos: [f32; 3],
    /// `+0x28..+0x3C`: box lo (x, y, z) at `+0x28/+0x2C/+0x30`, hi at `+0x34/+0x38/+0x3C`.
    pub bounds: [[f32; 3]; 2],
    /// `+0x40..+0x48`: heading.
    pub dir: [f32; 3],
    /// `+0x50`: turn rate.
    pub turn: f32,
    /// `+0x54..+0x5C`: turn axis.
    pub axis: [f32; 3],
    /// `+0x60`: retarget timer.
    pub retarget: f32,
    /// `+0x64`: alpha (state 1: progress / 4 in the integrator).
    pub alpha: f32,
    /// `+0x68`: life timer.
    pub life: f32,
    /// `+0x6C`: particle handle.
    pub handle: u32,
}

/// Globals of the function (`0x78B6xx`, `0x78B7xx`).
#[derive(Clone, Debug, PartialEq)]
pub struct WrthGlobals {
    /// `[0x78B6D4]`: 0 idle, 1 and 3 written elsewhere, 2 set here.
    pub state: u32,
    /// `[0x78B6D8]`.
    pub progress: f32,
    /// Key table `0x78B6A0` (count `[0x78B6B4]`).
    pub keys: Vec<u32>,
    /// `[0x78B6B8..0x78B6C0]` smoothed follow point.
    pub follow: [f32; 3],
    /// `[0x78B6C4..0x78B6CC]` follow velocity (written elsewhere).
    pub follow_velocity: [f32; 3],
    /// `[0x78B6DC..0x78B6E4]` last marker position, `[0x78B6E8]` marker seen.
    pub marker_pos: [f32; 3],
    pub marker_seen: bool,
    /// `[0x78B6EC]` marker grace timer.
    pub marker_timer: f32,
    /// `[0x78B6F0..0x78B6F8]` anchor used to bound the target near an emitter.
    pub anchor: [f32; 3],
    /// `[0x78B700]` flags (bit 1: only objects with index below the local player count are hurt).
    pub flags: u32,
    /// `[0x78B708]` sound volume, `[0x78B70C..0x78B714]` smoothed centroid.
    pub volume: f32,
    pub centroid: [f32; 3],
    pub pool: Vec<WrthEmitter>,
}

/// A game object the function looks at: position (`+0x64..+0x6C`), key (`+0x34`), extension `+0x150` present.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WrthObject {
    pub pos: [f32; 3],
    pub key: u32,
    pub has_ext: bool,
}

/// A local player block (stride `0x1584`): position `+0xA8..+0xB0`, key `+0xF54`, object `+0x14`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WrthPlayer {
    pub pos: [f32; 3],
    pub key: u32,
    pub object: Option<WrthObject>,
}

/// A candidate for the damage pass: an entry of the object list `0x7A3580` with a controller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WrthVictim {
    pub id: u32,
    /// Controller position `+0x64..+0x6C` (the pass adds 1.5 to y).
    pub pos: [f32; 3],
    pub key: u32,
    /// Controller type word `+0x60` equals "VDMN".
    pub is_marker_type: bool,
    /// Object field `+8` (compared with the local player count under global flag 2).
    pub index: i32,
}

/// Per-frame inputs that need no emitter state.
#[derive(Clone, Debug)]
pub struct WrthInput {
    /// "WRTH" driver object extents: centre `+0x1A8..`, half extents `+0x1C0..`; `None` = object missing.
    pub wrth: Option<([f32; 3], [f32; 3])>,
    /// Byte `[0x7B977C + key]` for each table key, same order as `keys`.
    pub key_active: Vec<bool>,
    pub players: Vec<WrthPlayer>,
    /// "VDMN", "VNMN", "JBTH" lookups (`0x15BA10`).
    pub vdmn: Option<WrthObject>,
    pub vnmn: Option<WrthObject>,
    pub jbth: Option<WrthObject>,
    pub victims: Vec<WrthVictim>,
    /// `[0x79094C]` level id, `[0x4E7944]` step count, `[0x4E794C]` float step.
    pub level: u32,
    pub steps: i32,
    pub float_step: f32,
}

/// `0x873F0(key, &min, &max)` over the raw 0xB4 byte entry words of `[0x708788]` (T1846 wiring, for `WrthWorld::zone_bounds`).
pub fn zone_bounds_from_entry(entry_words: &[u32; 0xB4 / 4]) -> ([f32; 3], [f32; 3]) {
    super::callees_t1846::region_box(entry_words)
}

/// World queries that depend on emitter state.
pub trait WrthWorld {
    /// `0x873C0(0, box, &zone, 5)`: zone id for a box.
    fn zone_of(&mut self, bounds: &[[f32; 3]; 2]) -> u32;
    /// `0x873F0(key, &min, &max)`.
    fn zone_bounds(&mut self, key: u32) -> ([f32; 3], [f32; 3]);
    /// `0xA3050` sweep of radius 2.0 in zone `key` around `pos`: the `+0x18` value of every hit.
    fn sweep_hits(&mut self, key: u32, pos: [f32; 3]) -> Vec<f32>;
    /// `0xC3E60`: ground height at `(x, y + 1, z)`.
    fn ground_y(&mut self, pos: [f32; 3]) -> f32;
    /// `0x153B40`.
    fn random(&mut self) -> u32;
}

/// External effects, in call order.
#[derive(Clone, Debug, PartialEq)]
pub enum WrthEffect {
    /// Slot op `0x227710(7, 0)` (set status 2 unless 0 or 2), level 1 only.
    SlotActivate { slot: u32 },
    /// Slot op `0x2274C0(7, 0)` (finish), level 1 only.
    SlotFinish { slot: u32 },
    /// `0x10E220(handle, 1, 0)` then `0x10EB90(handle, 0)`: hide the emitter's particle record.
    ReleaseParticle { handle: u32 },
    /// `0x378530`: burst at `pos` with heading `dir` (constants: type 0x200, scales 1.0, 1.0, 1.0).
    ReleaseBurst { pos: [f32; 3], dir: [f32; 3] },
    /// `0x10E730(handle, pos)` and `0x10EB10(handle, v, v)` with `v = life / 4`.
    UpdateParticle {
        handle: u32,
        pos: [f32; 3],
        scale: f32,
    },
    /// `0xB91F0` + `0xBB6C0`: damage 0.05 to the victim along `dir`, both `0x2` kind words.
    Damage {
        victim: u32,
        direction: [f32; 3],
        amount: f32,
    },
    /// `0x468A0(&[0x78B704], &[0x78B70C], 0x14E, volume, 25.0, 1.0, 0)`.
    Loop {
        id: u32,
        pos: [f32; 3],
        volume: f32,
        range: f32,
    },
}

fn unit_signed(word: u32) -> f32 {
    ((unit(word) - 0.5) * 2.0) as f32
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

fn key_in(keys: &[u32], key: u32) -> bool {
    keys.contains(&key)
}

/// `0x24AF45..0x24B172`: among the local players whose key equals the emitter key and whose x is below 13.5, the one
/// closest (squared distance, strict less than the running best, start `f32::MAX`) to the emitter; its object.
pub fn wrth_nearest_player(
    players: &[WrthPlayer],
    emitter_key: u32,
    pos: [f32; 3],
) -> Option<WrthObject> {
    let mut best = 3.402_823_5e38_f32;
    let mut found = None;
    for player in players {
        if player.key != emitter_key || player.pos[0] >= WRTH_CANDIDATE_X_LIMIT {
            continue;
        }
        let distance = dist2(player.pos, pos);
        if best > distance {
            best = distance;
            found = player.object;
        }
    }
    found
}

/// State 2 and 3 heading movement (`0x24B2F4..0x24B4F1`). `follow` is `[0x78B6B8..]`. `dt` is `(1/60) * steps`.
fn wrth_move(emitter: &mut WrthEmitter, global_state: u32, follow: [f32; 3], dt: f32) {
    let factor = if global_state == 3 {
        1.5
    } else {
        let d2 = dist2([emitter.pos[0], emitter.pos[1], emitter.pos[2]], follow);
        if 25.0 > d2 {
            1.0 + (25.0 - d2) * 0.04 * 0.5
        } else {
            1.0
        }
    };
    for axis in 0..3 {
        emitter.pos[axis] += dt * factor * emitter.dir[axis];
    }
    emitter.dir[1] -= dt * 0.5;
}

/// `0x24B4F1..0x24B63D`: keep the emitter inside the zone box with a 0.1 margin; the heading component reflects.
fn wrth_clamp(emitter: &mut WrthEmitter, lo: [f32; 3], hi: [f32; 3]) {
    for axis in 0..3 {
        if lo[axis] + WRTH_WALL_MARGIN > emitter.pos[axis] {
            emitter.pos[axis] = lo[axis] + WRTH_WALL_MARGIN;
            emitter.dir[axis] = emitter.dir[axis].abs();
        }
        if emitter.pos[axis] > hi[axis] - WRTH_WALL_MARGIN {
            emitter.pos[axis] = hi[axis] - WRTH_WALL_MARGIN;
            emitter.dir[axis] = -emitter.dir[axis].abs();
        }
    }
}

/// One frame of `0x24ABA0` ("WRTH", level 1 and the worm arena). Mutates `g`, returns the external effects.
pub fn wrth_frame(
    g: &mut WrthGlobals,
    input: &WrthInput,
    world: &mut dyn WrthWorld,
) -> Vec<WrthEffect> {
    let mut effects = Vec::new();
    // 0x24ABA0..0x24AC26: the driver object is gone, tear everything down once.
    let Some((centre, half)) = input.wrth else {
        if g.state != 0 {
            g.state = 0;
            for emitter in &mut g.pool {
                emitter.state = 2;
                effects.push(WrthEffect::ReleaseParticle {
                    handle: emitter.handle,
                });
            }
        }
        return effects;
    };
    // 0x24AC26..0x24AC5D: nothing runs while no key of the table is active.
    if g.keys.is_empty()
        || !input
            .key_active
            .iter()
            .take(g.keys.len())
            .any(|&active| active)
    {
        return effects;
    }
    let steps = input.steps as f32;
    let dt = (1.0 / 60.0) * steps;
    let mut target: Option<WrthObject> = None;
    let mut has_marker = false;
    if g.state != 0 {
        has_marker = true;
        // 0x24AC6A..0x24AE0: marker (VDMN) bookkeeping.
        if g.marker_timer > 0.0 {
            g.marker_timer -= (1.0 / 60.0) * input.float_step;
        }
        if let Some(marker) = input.vdmn {
            g.marker_pos = marker.pos;
            g.marker_seen = true;
            target = Some(marker);
        } else if g.marker_seen {
            target = None;
        } else {
            has_marker = false;
            let any_key = input.players.iter().any(|p| key_in(&g.keys, p.key));
            let first = input.players.first().and_then(|p| p.object);
            if input.level == 1 {
                let first_or_marker = if !any_key {
                    input.vnmn.filter(|o| key_in(&g.keys, o.key)).or(first)
                } else {
                    first
                };
                // 0x24AD99: a missing object or one outside the key table is replaced by the "JBTH" object.
                let found = match first_or_marker {
                    Some(object) if key_in(&g.keys, object.key) => Some(object),
                    _ => input.jbth,
                };
                if found.is_none() {
                    return effects;
                }
                target = found;
            } else {
                target = first;
            }
        }
        // 0x24ADF1..0x24BD74: per emitter update.
        for index in 0..g.pool.len() {
            if g.pool[index].state == 2 {
                continue;
            }
            let follow = g.follow;
            let (marker_seen, marker_pos, marker_timer, anchor) =
                (g.marker_seen, g.marker_pos, g.marker_timer, g.anchor);
            let global_state = g.state;
            let count = g.pool.len() as f32;
            let emitter = &mut g.pool[index];
            let p = emitter.pos;
            emitter.bounds = [
                [
                    p[0] + centre[0] - half[0],
                    p[1] + centre[1] - half[1],
                    p[2] + centre[2] - half[2],
                ],
                [
                    half[0] + p[0] + centre[0],
                    half[1] + centre[1] + p[1],
                    half[2] + centre[2] + p[2],
                ],
            ];
            emitter.zone = world.zone_of(&emitter.bounds);
            emitter.key = if key_in(&g.keys, emitter.zone) {
                emitter.zone
            } else {
                g.keys[0]
            };
            let (lo, hi) = world.zone_bounds(emitter.key);
            // 0x24AF11..: a target with an extension is replaced by the nearest matching player.
            if let Some(object) = target {
                if object.has_ext {
                    if let Some(near) =
                        wrth_nearest_player(&input.players, emitter.key, emitter.pos)
                    {
                        target = Some(near);
                    }
                }
            }
            // 0x24B172..0x24B4F1: movement by global state.
            match global_state {
                1 => {
                    g.progress += dt / count;
                    if g.progress > 0.5 {
                        let scale = if emitter.state == 1 {
                            WRTH_KIND_SCALE
                        } else {
                            1.0
                        };
                        let shape =
                            |value: f32| (value * 0.25 * 0.6 + 0.4) * (1.0 / 60.0) * scale * steps;
                        let old20 = emitter.pos[1];
                        emitter.pos[0] += shape(old20) * emitter.dir[0];
                        emitter.pos[1] = shape(old20) * emitter.dir[1] + old20;
                        emitter.pos[2] += shape(emitter.pos[1]) * emitter.dir[2];
                        if emitter.pos[1] > WRTH_STATE_DONE_PROGRESS {
                            g.state = 2;
                        }
                        emitter.alpha = emitter.pos[1] * 0.25;
                    }
                }
                _ => wrth_move(emitter, global_state, follow, dt),
            }
            wrth_clamp(emitter, lo, hi);
            // 0x24B63D..0x24BB29: steering in states 2 and 3.
            if (g.state == 2 || g.state == 3) && dist2(emitter.pos, follow) > 0.0 {
                let point;
                if marker_seen {
                    point = marker_pos;
                } else if emitter.state == 1 {
                    let rx = unit_signed(world.random());
                    let ry = unit_signed(world.random());
                    let rz = unit_signed(world.random());
                    point = [rx * 3.0 + 0.3, ry * 3.0 + 5.5, rz * 3.0 + 5.6];
                } else if let Some(object) = target {
                    let base = [object.pos[0], object.pos[1] + 1.5, object.pos[2]];
                    point = if marker_timer > 0.0 {
                        let d2 = dist2(anchor, emitter.pos);
                        if 64.0 > d2 {
                            let n = 8.0 / d2.sqrt();
                            [
                                anchor[0] + n * (emitter.pos[0] - anchor[0]),
                                anchor[1] + n * (emitter.pos[1] - anchor[1]),
                                anchor[2] + n * (emitter.pos[2] - anchor[2]),
                            ]
                        } else {
                            base
                        }
                    } else {
                        base
                    };
                } else {
                    point = [0.0; 3];
                }
                let close = dist2(emitter.pos, point) < 16.0;
                let rate = if emitter.state == 1 { 4.0 } else { 1.0 };
                emitter.retarget -= (1.0 / 60.0) * rate * input.float_step;
                let angle = (1.0 / 60.0) * emitter.turn * steps * rate;
                let retarget = emitter.retarget <= 0.0 || (close && emitter.retarget > 0.25);
                if retarget || g.state == 3 {
                    let v = [
                        point[0] - emitter.pos[0],
                        point[1] - emitter.pos[1],
                        point[2] - emitter.pos[2],
                    ];
                    let d = emitter.dir;
                    let mut axis = [
                        d[2] * v[1] - d[1] * v[2],
                        d[0] * v[2] - d[2] * v[0],
                        d[1] * v[0] - d[0] * v[1],
                    ];
                    let length2 = axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2];
                    if length2 > 1e-7 {
                        let scale = 1.0 / length2.sqrt();
                        axis = axis.map(|c| c * scale);
                    }
                    emitter.axis = axis;
                    let matrix = rotation_axis_angle(axis, angle);
                    emitter.dir = transform_direction(&matrix, emitter.dir);
                    let u = unit(world.random()) as f32;
                    emitter.retarget = if close {
                        (u + 1.0) * 0.125
                    } else {
                        u * 0.5 + 0.25
                    };
                } else {
                    let matrix = rotation_axis_angle(emitter.axis, angle);
                    emitter.dir = transform_direction(&matrix, emitter.dir);
                }
            }
            // 0x24BB29..0x24BD57: alpha from sweep hits and ground distance.
            if g.state == 2 || g.state == 3 {
                let old = emitter.alpha;
                let mut alpha = 1.0_f32;
                for hit in world.sweep_hits(emitter.key, emitter.pos) {
                    let candidate = 1.0 - hit.abs() * 0.5;
                    if alpha > candidate {
                        alpha = candidate;
                    }
                }
                let ground = world.ground_y([emitter.pos[0], emitter.pos[1] + 1.0, emitter.pos[2]]);
                let d = (ground - emitter.pos[1]).abs();
                if 1.0 > d && alpha > d {
                    alpha = d;
                }
                let clamped = alpha.clamp(0.0, 1.0);
                emitter.alpha = old * WRTH_ALPHA_BLEND.0 + clamped * WRTH_ALPHA_BLEND.1;
            }
        }
    }
    // 0x24BD74..0x24C252: push away from the target, follow point, damage.
    if g.state == 2 || g.state == 3 {
        let follow_base;
        if let Some(object) = target {
            if !has_marker {
                if input.level == 1 {
                    effects.push(WrthEffect::SlotActivate { slot: 7 });
                }
                for emitter in &mut g.pool {
                    let dx = emitter.pos[0] - object.pos[0];
                    let dz = emitter.pos[2] - object.pos[2];
                    if 9.0 > dx * dx + dz * dz {
                        let side =
                            emitter.dir[2] * 0.91 + emitter.dir[1] * 0.0 + emitter.dir[0] * -0.4;
                        if side <= 0.0 {
                            let matrix = rotation_y(WRTH_PUSH_ANGLE);
                            emitter.dir = transform_direction(&matrix, emitter.dir);
                        }
                        if input.level == 1 {
                            effects.push(WrthEffect::SlotFinish { slot: 7 });
                        }
                    }
                }
            }
            g.follow = [object.pos[0], object.pos[1], object.pos[2]];
            follow_base = [object.pos[0], object.pos[1] + 1.5, object.pos[2]];
        } else if g.marker_seen {
            follow_base = g.marker_pos;
        } else {
            follow_base = [g.follow[0], g.follow[1], g.follow[2]];
        }
        let k = (1.0 / 60.0) * steps * 4.0;
        g.follow = [
            follow_base[0] + k * g.follow_velocity[0],
            follow_base[1] + k * g.follow_velocity[1],
            follow_base[2] + k * g.follow_velocity[2],
        ];
        if has_marker {
            for victim in &input.victims {
                let point = [victim.pos[0], victim.pos[1] + 1.5, victim.pos[2]];
                for emitter in &g.pool {
                    if emitter.state == 2 || !key_in(&g.keys, victim.key) || victim.is_marker_type {
                        continue;
                    }
                    if 1.0 <= dist2(point, emitter.pos) {
                        continue;
                    }
                    let allowed = if g.flags & 2 != 0 {
                        victim.index < input.players.len() as i32
                    } else {
                        true
                    };
                    if !allowed {
                        continue;
                    }
                    let mut direction = [
                        unit_signed(world.random()),
                        unit_signed(world.random()),
                        unit_signed(world.random()),
                    ];
                    let length2: f32 = direction.iter().map(|c| c * c).sum();
                    if length2 > 1e-7 {
                        let scale = 1.0 / length2.sqrt();
                        direction = direction.map(|c| c * scale);
                    }
                    effects.push(WrthEffect::Damage {
                        victim: victim.id,
                        direction,
                        amount: 0.05,
                    });
                }
            }
        }
    }
    // 0x24C252..0x24C500: lifetimes, centroid and the ambience loop.
    let mut sum = [0.0_f32; 3];
    let mut weight = 0.0_f32;
    for emitter in &mut g.pool {
        match emitter.state {
            0 => {
                for axis in 0..3 {
                    sum[axis] += emitter.pos[axis];
                }
                weight += 1.0;
            }
            1 => {
                emitter.life -= (1.0 / 60.0) * input.float_step;
                if WRTH_RELEASE_TIMER > emitter.life {
                    emitter.state = 2;
                    effects.push(WrthEffect::ReleaseParticle {
                        handle: emitter.handle,
                    });
                    effects.push(WrthEffect::ReleaseBurst {
                        pos: emitter.pos,
                        dir: emitter.dir,
                    });
                } else {
                    let scale = emitter.life * 0.25;
                    effects.push(WrthEffect::UpdateParticle {
                        handle: emitter.handle,
                        pos: emitter.pos,
                        scale,
                    });
                    for axis in 0..3 {
                        sum[axis] += scale * emitter.pos[axis];
                    }
                    weight += scale;
                }
            }
            _ => {}
        }
    }
    if weight > 0.05 {
        let inverse = 1.0 / weight;
        for axis in 0..3 {
            let mean = inverse * sum[axis];
            g.centroid[axis] = (mean - g.centroid[axis]) * 0.1 + g.centroid[axis];
        }
    }
    if g.state != 0 {
        let target_volume = weight.clamp(0.0, 3.0) * 0.5;
        let mut volume = g.volume;
        let step = 0.05_f32;
        if (target_volume - volume).abs() < step {
            volume = target_volume;
        } else if target_volume > volume {
            volume += step;
        } else {
            volume -= step;
        }
        g.volume = volume;
        effects.push(WrthEffect::Loop {
            id: WRTH_SOUND_ID,
            pos: g.centroid,
            volume,
            range: WRTH_SOUND_RANGE,
        });
    }
    effects
}
