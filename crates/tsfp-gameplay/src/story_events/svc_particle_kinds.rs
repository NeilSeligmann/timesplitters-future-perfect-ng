//! T1863: the 31 per kind handlers of the effect start `0xEDD40` (jump table `0xEFB18`, kinds 6, 10 to 14, 16 to 18, 20 to
//! 26, 28 to 48 except the default ones) and the engine calls they make.
//!
//! Source: `build/default.xbe` (SHA-256 `3cfd001a84fc3e08175d6c4b2e42ebf49577a089b5d0bd87ac724f61a41816bc`). Decoded with
//! `tools/x87_blocks.py`, validated bit for bit against the original under the Unicorn oracle
//! (`tools/t1846_gold/svc_particle_kinds.py`, test `tests/story_events_t1863_particle_kinds.rs`). Document:
//! `docs/t-story-event-particle-kinds-t1863.md`. Names INFERRED, behaviour MEASURED (xemu level, not silicon).
//!
//! Every handler is a sequence of calls to the emitter creator `0x111AE0(0, template, flags, &position, &direction)` and its
//! parameter setters (`0x10E660`, `0x10EAE0`, `0x10EB30`, `0x10EB70`, `0x10E9D0`, `0x10E8C0`), some with random vectors drawn
//! from the rng `0x153B40`, followed by the shared tail ([`finish_start`]). The emitters are engine state behind
//! [`KindEngine`]; the arguments (including every rng draw and every x87 expression) are what is ported and measured.
//!
//! | kind | handler | kind | handler | kind | handler |
//! |---|---|---|---|---|---|
//! | 6 | `0xEEF7E` | 10 | `0xEDD9C` | 11, 12 | `0xEDDD7` |
//! | 13, 14 | `0xEDE22` | 16 | `0xEE546` | 17 | `0xEE78A` |
//! | 18 | `0xEE9D0` | 20 | `0xEEA11` | 21 | `0xEF089` |
//! | 22 | `0xEF136` | 23 | `0xEF187` | 24 | `0xEDEE6` |
//! | 25, 48 | `0xEDE84` | 26 | `0xEF1A8` | 28 | `0xEF1D2` |
//! | 29 | `0xEEA41` | 30 | `0xEEB0E` | 31 | `0xEEBA4` |
//! | 32 | `0xEF229` | 33 | `0xEECAA` | 34 | `0xEE32E` |
//! | 35 | `0xEF026` | 36 | `0xEE0C9` | 37 | `0xEF26A` |
//! | 38 | `0xEF35B` | 39 | `0xEF2FD` | 40, 47 | `0xEF444` |
//! | 41 | `0xEF2AE` | 42 | `0xEF4D3` | 43 | `0xEEFCF` |
//! | 44, 45, 46 | `0xEF507` | | | | |

use super::prelude_1e2b80::init_spawn_record;
use super::strip_update::GuestMemory;
use super::svc_particle_efbd0::{
    EmberEngine, EmberWorld, W_FLAGS, W_MODE, W_OWNER, W_POS, W_SOURCE, finish_start,
};

/// Camera object pointer (`[0x7B0CC0]`), basis vector at `+0x334`, position at `+0x27C`.
pub const CAMERA_PTR: u32 = 0x007B_0CC0;
/// Array of actor pointers (`0x7A3580`) and its count (`[0x74C37C]`) walked by kind 17.
pub const ACTOR_LIST: u32 = 0x007A_3580;
pub const ACTOR_COUNT: u32 = 0x0074_C37C;
/// `[0x79094C]` (level id), `[0x790950]` (player count), `[0x7DE455]` (mode byte).
pub const LEVEL_ID: u32 = 0x0079_094C;
pub const PLAYER_COUNT: u32 = 0x0079_0950;
pub const MODE_BYTE: u32 = 0x007D_E455;

/// The emitter parameter setters that act on the emitter created last (`[0x7AD28C]`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmitterParam {
    /// `0x10E660(scale)`: `+0x64`.
    Scale(f32),
    /// `0x10EAE0(a, b)`: `+0x68 = a`, `+0x6C = b`.
    Span(f32, f32),
    /// `0x10EB30(v)`: `+0x70`.
    Param70(f32),
    /// `0x10EB70(v)`: `+0x74`.
    Param74(f32),
    /// `0x10E9D0(&v)`: `+0x98..+0xA0 = v`, flag `0x4000`.
    Vec98([f32; 3]),
    /// `0x10E8C0(handle, 0)`: bounds from a model handle (flag `0x100`), or cleared when the handle is 0.
    Attach(u32),
}

/// The debris object builder calls of kinds 44 to 46.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DebrisCall {
    /// `0xA6F90(class, &position, &rotation, 0, 0)` returns the new object.
    Create {
        class: u32,
        position: [f32; 3],
        rotation: [f32; 3],
    },
    /// `0x16C170(object, 2)` returns a handle.
    Handle { object: u32 },
    /// `0x172760(handle, object, 0x11, -1, 0x180)`.
    Bind { handle: u32, object: u32 },
    /// `0x16B790(handle, 8, 0xFF)`.
    Limit { handle: u32 },
    /// `0x16C980(handle, &position, &velocity)`.
    Launch {
        handle: u32,
        position: [f32; 3],
        velocity: [f32; 3],
    },
    /// `0xAE240(object, life, count)`.
    Finish { object: u32, life: f32, count: u32 },
}

/// Engine services called by the kind handlers. Guest memory is the original address space (entry, camera, actors, owner and
/// debris objects live there). Every other method is one call of the original with the argument words of the cdecl call.
pub trait KindEngine: EmberEngine {
    fn guest(&mut self) -> &mut dyn GuestMemory;
    /// `0x111AE0(0, template, flags, &position, &direction)`: create an emitter.
    fn emitter(&mut self, template: u32, flags: u32, position: [f32; 3], direction: [f32; 3]);
    /// The setters of the emitter created last.
    fn emitter_param(&mut self, param: EmitterParam);
    /// `0x126260(handle, 0)` (kick the actor at `actor + 0xC68`).
    fn actor_kick(&mut self, handle: u32);
    /// `0x278040(actor)`: non zero blocks the kick (level 4).
    fn actor_gate(&mut self, actor: u32) -> bool;
    /// `0xEDCB0` with `eax = &vector`, `ebx = &origin` (a light flash table walk through `0xEC880`).
    fn flash(&mut self, vector: [f32; 3], origin: [f32; 3]);
    /// `0xFAC70(handle, &position, &direction, 1.0, 1.0)` (beam slot of the source object).
    fn beam(&mut self, handle: u32, position: [f32; 3], direction: [f32; 3]);
    /// `0x668A0(value)` (screen flash, `[0x7DE458] |= 0x800`).
    fn screen_flash(&mut self, value: f32);
    /// The debris builder calls (kinds 44 to 46). `Create` and `Handle` return the object and the handle.
    fn debris(&mut self, call: DebrisCall) -> u32;
}

fn read_f32(engine: &mut dyn KindEngine, address: u32) -> f32 {
    f32::from_bits(engine.guest().read_u32(address))
}

fn read_vec3(engine: &mut dyn KindEngine, address: u32) -> [f32; 3] {
    [
        read_f32(engine, address),
        read_f32(engine, address.wrapping_add(4)),
        read_f32(engine, address.wrapping_add(8)),
    ]
}

/// `1 / sqrt(x)` as the original `rsqrtss` executes (QEMU: exact divide, xemu level).
fn rsqrt(value: f32) -> f32 {
    1.0 / value.sqrt()
}

/// One rng draw as the x87 sequence `fild` (+2^32 when negative), `fmul 2^-32`: a double in `[0, 1)`.
fn draw_unit(engine: &mut dyn KindEngine) -> f64 {
    f64::from(engine.rng()) * f64::from(f32::from_bits(0x2F80_0000))
}

/// `fadd st0, st0; fsub 1.0; fstp` of a unit draw: uniform in `[-1, 1)` rounded to f32.
fn draw_signed(engine: &mut dyn KindEngine) -> f32 {
    (draw_unit(engine) * 2.0 - 1.0) as f32
}

/// `fsub 0.5; fadd st0, st0` of a unit draw (double, not yet stored).
fn draw_half_doubled(engine: &mut dyn KindEngine) -> f64 {
    (draw_unit(engine) - 0.5) * 2.0
}

/// `(len2 > 1e-7)` guarded normalisation: `v * (rsqrt(len2) * 1.0)` with `len2 = (z^2 + y^2) + x^2`.
fn normalise_zyx(v: [f32; 3]) -> [f32; 3] {
    let len2 = (v[2] * v[2] + v[1] * v[1]) + v[0] * v[0];
    if len2 > f32::from_bits(0x33D6_BF95) {
        let inverse = rsqrt(len2) * 1.0;
        [inverse * v[0], inverse * v[1], inverse * v[2]]
    } else {
        v
    }
}

/// The random vector of kinds 24, 34 and 36 (`d = (u3 c.z + u2 c.y) + u1 c.x`, `v = d c + u + (0, 0.3, 0)`, normalised).
fn camera_scatter(engine: &mut dyn KindEngine, basis: [f32; 3]) -> [f32; 3] {
    let u1 = draw_signed(engine);
    let u2 = draw_signed(engine);
    let u3 = draw_signed(engine);
    let d = (u3 * basis[2] + u2 * basis[1]) + u1 * basis[0];
    let x = d * basis[0] + u1;
    let y = (d * basis[1] + u2) + f32::from_bits(0x3E99_999A);
    let z = d * basis[2] + u3;
    normalise_zyx([x, y, z])
}

/// Emission helper: the entry position and direction, with the setters.
struct Burst<'a> {
    engine: &'a mut dyn KindEngine,
    position: [f32; 3],
    direction: [f32; 3],
}

impl Burst<'_> {
    fn emit(&mut self, template: u32, flags: u32) {
        self.engine
            .emitter(template, flags, self.position, self.direction);
    }
    fn emit_with(&mut self, template: u32, flags: u32, direction: [f32; 3]) {
        self.engine
            .emitter(template, flags, self.position, direction);
    }
    fn span(&mut self, a: f32, b: f32) {
        self.engine.emitter_param(EmitterParam::Span(a, b));
    }
    fn param70(&mut self, value: f32) {
        self.engine.emitter_param(EmitterParam::Param70(value));
    }
    fn param74(&mut self, value: f32) {
        self.engine.emitter_param(EmitterParam::Param74(value));
    }
    fn scale(&mut self, value: f32) {
        self.engine.emitter_param(EmitterParam::Scale(value));
    }
    fn vec98(&mut self, value: [f32; 3]) {
        self.engine.emitter_param(EmitterParam::Vec98(value));
    }
}

/// Kinds with a handler here (the rest are the default path or kind 15 of [`super::svc_particle_efbd0::start_effect`]).
pub const HANDLED_KINDS: [u32; 37] = [
    6, 10, 11, 12, 13, 14, 16, 17, 18, 20, 21, 22, 23, 24, 25, 26, 28, 29, 30, 31, 32, 33, 34, 35,
    36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
];

/// `0xEDD40` for the kinds that [`super::svc_particle_efbd0::start_effect`] hands over: runs the handler, then the shared
/// tail. Returns the result of `0xEDD40` (0 after a release, else slot + 1), or `None` for a kind without a handler.
pub fn start_kind_handler(
    world: &mut EmberWorld,
    engine: &mut dyn KindEngine,
    slot: usize,
    kind: u32,
) -> Option<u32> {
    if !HANDLED_KINDS.contains(&kind) {
        return None;
    }
    match kind {
        16 => kind_16(world, engine, slot),
        17 => kind_17(world, engine, slot),
        21 => kind_21(world, engine, slot),
        24 => kind_24(world, engine, slot),
        33 => kind_33(world, engine, slot),
        34 => kind_34(world, engine, slot),
        36 => kind_36(world, engine, slot),
        38 => kind_38(world, engine, slot),
        43 => kind_43(world, engine, slot),
        44..=46 => kind_44(world, engine, slot),
        _ => {
            let entry = &world.pool.entries[slot];
            let mut burst = Burst {
                position: entry.position(),
                direction: entry.direction(),
                engine,
            };
            simple_kind(&mut burst, kind);
            return Some(finish_start(world, burst.engine, slot));
        }
    }
    Some(finish_start(world, engine, slot))
}

fn burst_of<'a>(world: &EmberWorld, engine: &'a mut dyn KindEngine, slot: usize) -> Burst<'a> {
    let entry = &world.pool.entries[slot];
    Burst {
        position: entry.position(),
        direction: entry.direction(),
        engine,
    }
}

/// The handlers that are a fixed sequence of emitter calls (and the direction tests of kinds 28 and 39).
fn simple_kind(burst: &mut Burst, kind: u32) {
    match kind {
        6 => {
            burst.emit(0x7B0A80, 0);
            burst.span(1.0, 1.0);
            burst.param70(2.0);
            burst.emit(0x7B06C0, 0);
            burst.span(1.0, 1.0);
            burst.param70(2.0);
        }
        10 => {
            burst.emit(0x73_9690, 0);
            burst.emit(0x73_5A80, 0);
            burst.scale(0.3);
        }
        11 | 12 => {
            burst.emit(0x73_78E0, 0);
            burst.emit(0x73_9690, 0);
            burst.emit(0x73_5A80, 0);
            burst.scale(0.3);
        }
        13 | 14 => {
            burst.emit_with(0x73_6E18, 0, [0.0, 1.0, 0.0]);
            burst.emit(0x73_93B0, 0);
            burst.emit(0x73_9328, 0);
        }
        25 | 48 => {
            burst.emit_with(0x73_7C60, 0, [0.0, 1.0, 0.0]);
            burst.emit(0x73_57D0, 0);
            burst.emit(0x73_61A8, 0);
        }
        18 => {
            burst.emit(0x7B0580, 0);
            burst.emit(0x7B0A80, 0);
            burst.emit(0x7B06C0, 0);
        }
        20 => {
            burst.emit(0x7B0A80, 0);
            burst.span(1.5, 1.5);
        }
        22 => {
            for template in [0x73_8A68, 0x73_8BC8, 0x73_9A08, 0x7A_FEA0] {
                burst.emit(template, 0);
            }
        }
        23 => burst.emit(0x7A_FF40, 0),
        26 => {
            burst.emit(0x7B0A80, 0x1000);
            burst.emit(0x7B06C0, 0x1000);
            burst.emit(0x7A_FEA0, 0);
        }
        28 => {
            let template = if burst.direction[1] == 1.0 {
                0x7B04E0
            } else {
                0x7B0300
            };
            burst.emit(template, 0);
        }
        29 => {
            let v = [3.0, 0.0, 0.0];
            burst.emit(0x7A_FFE0, 0x5000);
            burst.span(3.0, 1.0);
            burst.vec98(v);
            burst.emit(0x7B06C0, 0x1000);
            burst.span(5.0, 5.0);
            burst.emit(0x7B0A80, 0x1000);
            burst.span(5.0, 5.0);
            burst.emit(0x7B0620, 0x5000);
            burst.span(4.0, 5.0);
            burst.vec98(v);
        }
        30 => {
            let v = [10.0, 0.0, 0.0];
            burst.emit(0x7B0A80, 0x1000);
            burst.span(5.0, 5.0);
            burst.scale(0.8);
            burst.emit(0x7B0620, 0x5000);
            burst.span(4.0, 5.0);
            burst.scale(0.8);
            burst.vec98(v);
        }
        31 => {
            let v = [3.0, 0.0, 0.0];
            for _ in 0..2 {
                burst.emit(0x7A_FFE0, 0x5000);
                burst.span(3.0, 1.0);
                burst.vec98(v);
            }
            burst.scale(1.0);
            burst.emit(0x7B0A80, 0x1000);
            burst.span(3.0, 3.0);
            burst.emit(0x7B06C0, 0x1000);
            burst.span(4.0, 3.0);
            burst.emit(0x7B0620, 0x5000);
            burst.span(4.0, 5.0);
            burst.vec98(v);
        }
        32 => {
            burst.emit(0x73_8A68, 0);
            burst.emit(0x7B03A0, 0);
            burst.emit(0x7A_FEA0, 0);
        }
        35 => {
            for template in [0x7B0A80, 0x7B06C0] {
                burst.emit(template, 0);
                burst.span(0.7, 0.25);
                burst.param70(2.0);
            }
        }
        37 => {
            burst.emit(0x73_80F8, 0);
            burst.emit(0x7B0B20, 0);
            burst.emit(0x73_6828, 0x1000);
        }
        39 => {
            if burst.direction[1] > 0.8 {
                burst.emit(0x73_8EC0, 0);
            }
            burst.emit(0x73_84B0, 0);
            burst.emit(0x7B0B20, 0);
        }
        40 | 47 => {
            burst.emit(0x7B0120, 0x1000);
            burst.span(1.0, 1.0);
            burst.param70(2.0);
            burst.emit(0x73_6828, 0x1000);
            burst.emit(0x7B06C0, 0);
            burst.span(1.0, 1.0);
            burst.param70(2.0);
            burst.emit(0x7B0800, 0x1000);
        }
        41 => {
            burst.emit(0x73_80F8, 0);
            burst.span(2.0, 2.0);
            burst.emit(0x7B0B20, 0);
            burst.span(2.0, 2.0);
        }
        42 => {
            burst.emit(0x7B08A0, 0);
            burst.emit(0x7B0800, 0x1000);
        }
        other => unreachable!("kind {other} is not a fixed sequence"),
    }
}

/// Kind 21 (`0xEF089`): four emitters, the first three attached to the owner's model handle, flag `0x20000` on the entry.
fn kind_21(world: &mut EmberWorld, engine: &mut dyn KindEngine, slot: usize) {
    let owner = world.pool.entries[slot].w[W_OWNER];
    let mut burst = burst_of(world, engine, slot);
    for template in [0x7B0580, 0x7B0A80, 0x7B06C0] {
        burst.emit(template, 0);
        if owner != 0 {
            let handle = burst.engine.guest().read_u32(owner.wrapping_add(0x34));
            burst.engine.emitter_param(EmitterParam::Attach(handle));
        }
    }
    burst.emit(0x7A_FEA0, 0);
    world.pool.entries[slot].w[W_FLAGS] |= 0x2_0000;
}

/// The camera basis vector `[[0x7B0CC0] + 0x334]`.
fn camera_basis(engine: &mut dyn KindEngine) -> [f32; 3] {
    let camera = engine.guest().read_u32(CAMERA_PTR);
    read_vec3(engine, camera.wrapping_add(0x334))
}

/// Kind 16 (`0xEE546`): five scattered emitters (`v = u - 0.9 (u . c) c + (0, 0.5, 0)`), then two on the direction.
fn kind_16(world: &mut EmberWorld, engine: &mut dyn KindEngine, slot: usize) {
    let basis = camera_basis(engine);
    let mut burst = burst_of(world, engine, slot);
    let shrink = f32::from_bits(0x3F66_6666);
    for _ in 0..5 {
        let u1 = draw_signed(burst.engine);
        let u2 = draw_signed(burst.engine);
        let u3 = draw_signed(burst.engine);
        let d = (u3 * basis[2] + u2 * basis[1]) + u1 * basis[0];
        let x = u1 - (d * basis[0]) * shrink;
        let z = u3 - (d * basis[2]) * shrink;
        let y = (u2 - (d * basis[1]) * shrink) + 0.5;
        let v = normalise_zyx([x, y, z]);
        let template = if draw_unit(burst.engine) > f64::from(f32::from_bits(0x3F4C_CCCD)) {
            0x73_9438
        } else {
            0x73_98F8
        };
        burst.emit_with(template, 0, v);
    }
    burst.emit(0x73_7BD8, 0);
    burst.emit(0x73_9548, 0);
}

/// Kind 24 (`0xEDEE6`): ten scattered emitters, then one on the direction.
fn kind_24(world: &mut EmberWorld, engine: &mut dyn KindEngine, slot: usize) {
    let basis = camera_basis(engine);
    let mut burst = burst_of(world, engine, slot);
    for _ in 0..10 {
        let v = camera_scatter(burst.engine, basis);
        burst.emit_with(0x73_94C0, 0, v);
    }
    burst.emit(0x73_9870, 0);
}

/// Kind 34 (`0xEE32E`): five scattered emitters, one of three on the direction (`rng % 3`), then `0x735F40`.
fn kind_34(world: &mut EmberWorld, engine: &mut dyn KindEngine, slot: usize) {
    let basis = camera_basis(engine);
    let mut burst = burst_of(world, engine, slot);
    for _ in 0..5 {
        let v = camera_scatter(burst.engine, basis);
        burst.emit_with(0x73_95D0, 0, v);
    }
    let template = match burst.engine.rng() % 3 {
        0 => 0x73_9870,
        1 => 0x73_8878,
        _ => 0x73_7DB8,
    };
    burst.emit(template, 0);
    burst.emit(0x73_5F40, 0);
}

/// Kind 36 (`0xEE0C9`): zero or one scattered emitter (`rng % 5 == 4`, drawn again after each), then two on the direction.
fn kind_36(world: &mut EmberWorld, engine: &mut dyn KindEngine, slot: usize) {
    let basis = camera_basis(engine);
    let mut burst = burst_of(world, engine, slot);
    let wanted = |engine: &mut dyn KindEngine| i32::from(engine.rng() % 5 > 3);
    let scale = f32::from_bits(0x3E4C_CCCD);
    let mut done = 0;
    let mut want = wanted(burst.engine);
    while done < want {
        let v = camera_scatter(burst.engine, basis);
        burst.emit_with(0x73_94C0, 0, [v[0] * scale, v[1] * scale, v[2] * scale]);
        done += 1;
        want = wanted(burst.engine);
    }
    for template in [0x7B0A80, 0x7B06C0] {
        burst.emit(template, 0);
        burst.span(0.5, 0.25);
        burst.param70(2.0);
    }
}

/// Kind 33 (`0xEECAA`): nine emitters with their setters, the last one aimed from the camera to the entry.
fn kind_33(world: &mut EmberWorld, engine: &mut dyn KindEngine, slot: usize) {
    let mut burst = burst_of(world, engine, slot);
    let v = [3.0, 0.0, 0.0];
    burst.emit(0x7B01C0, 0x5000);
    burst.span(1.6, 1.2);
    burst.param74(2.0);
    burst.emit(0x7B0080, 0x5000);
    burst.span(1.6, 2.0);
    burst.param74(2.0);
    burst.vec98(v);
    burst.emit(0x7B0800, 0x1000);
    burst.span(5.0, 1.0);
    burst.param74(2.0);
    burst.emit(0x7B09E0, 0x5000);
    burst.span(3.2, 4.0);
    burst.param74(2.0);
    burst.scale(0.15);
    burst.vec98(v);
    burst.emit(0x7B01C0, 0x5000);
    burst.span(3.2, 2.4);
    burst.param74(2.0);
    burst.scale(0.15);
    burst.emit(0x7B0BC0, 0x5000);
    burst.span(2.4, 2.4);
    burst.param74(2.0);
    burst.scale(0.15);
    burst.emit(0x7A_FD60, 0x5000);
    burst.param74(0.1);
    burst.span(3.9, 1.3);
    burst.param74(2.0);
    burst.scale(0.3);
    burst.vec98(v);
    burst.emit(0x7B01C0, 0x5000);
    burst.span(5.2, 3.9);
    burst.param74(2.0);
    burst.scale(0.3);
    let camera = burst.engine.guest().read_u32(CAMERA_PTR);
    let origin = read_vec3(burst.engine, camera.wrapping_add(0x27C));
    let p = burst.position;
    let aim = normalise_zyx([p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]]);
    burst.emit_with(0x7B0940, 0x1000, aim);
    burst.scale(0.3);
}

/// Kind 38 (`0xEF35B`): three emitters at `z - 3`, `z + 3`, `z` (the entry position is modified, each step rounded).
fn kind_38(world: &mut EmberWorld, engine: &mut dyn KindEngine, slot: usize) {
    let mut z = world.pool.entries[slot].f(W_POS + 2);
    let scales = [0.0f32, 0.4, 0.8];
    let steps = [-3.0f32, 6.0, -3.0];
    for (step, scale) in steps.into_iter().zip(scales) {
        z += step;
        world.pool.entries[slot].set_f(W_POS + 2, z);
        let mut burst = burst_of(world, engine, slot);
        burst.emit(0x7B0120, 0x1000);
        burst.span(5.0, 5.0);
        burst.param70(2.0);
        burst.scale(scale);
    }
}

/// `0xE0F00(entry)`: the source object's stat block holds a non negative value and the entry mode is 1.
fn source_gate(world: &EmberWorld, engine: &mut dyn KindEngine, slot: usize) -> Option<u32> {
    let entry = &world.pool.entries[slot];
    if entry.w[W_MODE] != 1 {
        return None;
    }
    let source = entry.w[W_SOURCE];
    let block = engine.guest().read_u32(source.wrapping_add(0x150));
    let value = engine.guest().read_u32(block.wrapping_add(4));
    (value as i32 >= 0).then_some(value)
}

/// Kind 43 (`0xEEFCF`): sound `0xCB1`, a beam when the source gate passes, the screen flash `10.0`.
fn kind_43(world: &mut EmberWorld, engine: &mut dyn KindEngine, slot: usize) {
    let mut request = init_spawn_record();
    request[0] = 0xCB1;
    engine.submit_sound(request);
    if let Some(handle) = source_gate(world, engine, slot) {
        let entry = &world.pool.entries[slot];
        engine.beam(handle, entry.position(), entry.direction());
    }
    engine.screen_flash(10.0);
}

/// Kind 17 (`0xEE78A`): two sparks at the entry, two more at its position, a kick for every actor within `sqrt(30)`, and the
/// flash `0xEDCB0` with the direction scaled by 8000.
fn kind_17(world: &mut EmberWorld, engine: &mut dyn KindEngine, slot: usize) {
    let entry = &world.pool.entries[slot];
    let (position, direction) = (entry.position(), entry.direction());
    let aim = normalise_zyx([direction[0], direction[1] + 0.2, direction[2]]);
    let mut copy = [position[0], position[1] - 0.3, position[2]];
    let mut burst = Burst {
        engine,
        position: copy,
        direction: aim,
    };
    burst.emit(0x7A_FE00, 0);
    copy[1] += f32::from_bits(0x3F19_999A);
    burst.position = copy;
    burst.emit(0x7A_FE00, 0);
    burst.position = position;
    burst.emit(0x7B0440, 0);
    burst.emit(0x7B0260, 0);
    let engine = burst.engine;
    let wide = 8000.0f32;
    let flash = [aim[0] * wide, aim[1] * wide, aim[2] * wide];
    let mut index = 0u32;
    while (index as i32) < engine.guest().read_u32(ACTOR_COUNT) as i32 {
        let actor = engine.guest().read_u32(ACTOR_LIST + 4 * index);
        index += 1;
        if actor == 0 || engine.guest().read_u32(actor.wrapping_add(0xBF0)) == 0 {
            continue;
        }
        let node = engine.guest().read_u32(actor);
        let at = read_vec3(engine, node.wrapping_add(0x64));
        let (dx, dy, dz) = (
            at[0] - position[0],
            at[1] - position[1],
            at[2] - position[2],
        );
        let distance_squared = (dx * dx + dy * dy) + dz * dz;
        if !(30.0 > distance_squared) {
            continue;
        }
        if engine.guest().read_u32(LEVEL_ID) as i32 == 4 {
            let index_of = engine.guest().read_u32(actor.wrapping_add(8)) as i32;
            if index_of < engine.guest().read_u32(PLAYER_COUNT) as i32 || engine.actor_gate(actor) {
                continue;
            }
        }
        engine.actor_kick(actor.wrapping_add(0xC68));
    }
    engine.flash(flash, position);
}

/// Kinds 44 to 46 (`0xEF507`): three debris objects around the owner, each with a random rotation, offset and velocity.
fn kind_44(world: &mut EmberWorld, engine: &mut dyn KindEngine, slot: usize) {
    let owner = world.pool.entries[slot].w[W_OWNER];
    if owner == 0 {
        return;
    }
    let half = f64::from(f32::from_bits(0x3F00_0000));
    for index in 0..3u32 {
        let draw_rot = |engine: &mut dyn KindEngine| (draw_unit(engine) - half) * 2.0;
        let rot_x = draw_rot(engine) as f32;
        let rot_y = draw_rot(engine) as f32;
        let rot_z = draw_rot(engine);
        let rotation = [
            (f64::from(rot_x) * 360.0) as f32,
            (f64::from(rot_y) * 360.0) as f32,
            (rot_z * 360.0) as f32,
        ];
        let base = read_vec3(engine, owner.wrapping_add(0x1A8));
        let at = read_vec3(engine, owner.wrapping_add(0x64));
        let anchor = [base[0] + at[0], base[1] + at[1], base[2] + at[2]];
        let spread = read_vec3(engine, owner.wrapping_add(0x1C0));
        let mut offset = [0.0f32; 3];
        for axis in 0..3 {
            let unit = (draw_unit(engine) - half) * 1.5;
            offset[axis] = (unit * f64::from(spread[axis])) as f32;
        }
        let position = [
            offset[0] + anchor[0],
            offset[1] + anchor[1],
            offset[2] + anchor[2],
        ];
        let object = engine.debris(DebrisCall::Create {
            class: 0x14 + index,
            position,
            rotation,
        });
        let flags = engine.guest().read_u32(object.wrapping_add(0x28));
        engine
            .guest()
            .write_u32(object.wrapping_add(0x28), flags | 0x10);
        engine.guest().write_u32(object.wrapping_add(0x1D8), 0);
        engine.guest().write_u32(object.wrapping_add(0x1CC), 0);
        let handle = engine.debris(DebrisCall::Handle { object });
        engine.guest().write_u32(object.wrapping_add(0x80), handle);
        engine.debris(DebrisCall::Bind { handle, object });
        let handle = engine.guest().read_u32(object.wrapping_add(0x80));
        engine.debris(DebrisCall::Limit { handle });
        let e1 = draw_signed_half(engine);
        let e2 = draw_signed_half(engine);
        let e3 = draw_signed_half(engine);
        let size = read_vec3(engine, object.wrapping_add(0x1C0));
        let quarter = 0.25f32;
        let launch = [
            (size[0] * e1) * quarter + position[0],
            (size[1] * e2) * quarter + position[1],
            (size[2] * e3) * quarter + position[2],
        ];
        let f1 = draw_signed_half(engine);
        let f2 = draw_signed_half(engine);
        let f3 = draw_half_doubled(engine);
        let velocity = [
            (f64::from(f1) * 500.0) as f32,
            (f64::from(f2) * 500.0) as f32,
            (f3 * 500.0) as f32,
        ];
        let handle = engine.guest().read_u32(object.wrapping_add(0x80));
        engine.debris(DebrisCall::Launch {
            handle,
            position: launch,
            velocity,
        });
        let (life, count) = if engine.guest().read_u8(MODE_BYTE) == 0xA {
            (30.0, 6)
        } else {
            (15.0, 2)
        };
        engine.debris(DebrisCall::Finish {
            object,
            life,
            count,
        });
    }
}

/// `fstp` of `(u * 2^-32 - 0.5) * 2`: f32.
fn draw_signed_half(engine: &mut dyn KindEngine) -> f32 {
    draw_half_doubled(engine) as f32
}
