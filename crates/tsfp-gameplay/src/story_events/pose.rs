//! Pose-tick family of the callback ticks (T1845 task E).
//!
//! Static reading of `build/default.xbe` (SHA-256 `3cfd001a...1816bc`), no xemu run. Control flow and constants are
//! MEASURED; the game meaning of a field is INFERRED where stated. Matrices are 16 floats, row-major, row-vector
//! convention (translation in the row at floats 12..14), as used by the engine matrix helpers `0x153270` (product
//! `out = A * B`), `0x152770` (axis-angle), `0x153440` (rotation about Y), `0x153550` (translation set),
//! `0x1536E0` (rotation about a pivot) and `0x152090` (direction transform). See docs/t-story-event-pose-ticks.md.

/// 4x4 matrix, row-major.
pub type Mat4 = [f32; 16];

/// The identity matrix (`0x4E7900`, copied by `0x153240`).
pub const IDENTITY: Mat4 = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

/// `0x153270`: `A * B` (row `i` of the result is row `i` of A times B). Role INFERRED from the helper name table,
/// the loop shape (row broadcast times rows of B) is read.
pub fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [0.0f32; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[row * 4 + k] * b[k * 4 + col];
            }
            out[row * 4 + col] = sum;
        }
    }
    out
}

/// `0x152770`: rotation by `angle` about the unit `axis` (MEASURED term by term, sin/cos of the x87 unit).
pub fn rotation_axis_angle(axis: [f32; 3], angle: f32) -> Mat4 {
    // `fsin` / `fcos` of the x87 unit, each rounded to f32 at the `fstp` (batch 8: f32 `sin_cos` differs in the last bit).
    let s = f64::from(angle).sin() as f32;
    let c = f64::from(angle).cos() as f32;
    let t = 1.0 - c;
    let [x, y, z] = axis;
    let mut m = [0.0f32; 16];
    m[0] = x * x * t + c;
    m[1] = x * y * t - z * s;
    m[2] = x * z * t + y * s;
    m[4] = x * y * t + z * s;
    m[5] = y * y * t + c;
    m[6] = y * z * t - x * s;
    m[8] = x * z * t - y * s;
    m[9] = y * z * t + x * s;
    m[10] = z * z * t + c;
    m[15] = 1.0;
    m
}

/// `0x153440`: rotation about Y. Floats 0 and 10 are cos, float 2 is -sin, float 8 is sin (MEASURED).
pub fn rotation_y(angle: f32) -> Mat4 {
    let (s, c) = angle.sin_cos();
    let mut m = IDENTITY;
    m[0] = c;
    m[2] = -s;
    m[8] = s;
    m[10] = c;
    m
}

/// `0x153550`: identity with the translation row set to `t` (MEASURED).
pub fn translation(t: [f32; 3]) -> Mat4 {
    let mut m = IDENTITY;
    m[12..15].copy_from_slice(&t);
    m
}

/// `0x1536E0(out, angle, axis, pivot)`: `T(-pivot) * R(axis, -angle) * T(pivot)` composed in that order with
/// `0x153270` (MEASURED call sequence; the angle is negated before `0x152770`). With the row-vector convention
/// this moves the pivot to the origin first, rotates, and moves it back.
pub fn rotation_about_pivot(angle: f32, axis: [f32; 3], pivot: [f32; 3]) -> Mat4 {
    let t_pos = mat_mul(&translation(pivot), &IDENTITY);
    let rotated = mat_mul(&rotation_axis_angle(axis, -angle), &t_pos);
    let neg = [-pivot[0], -pivot[1], -pivot[2]];
    mat_mul(&translation(neg), &rotated)
}

/// `0x152090`: rotate a direction by the upper 3x3 of `m`: `x' = m0 x + m4 y + m8 z`, and so on (MEASURED).
pub fn transform_direction(m: &Mat4, v: [f32; 3]) -> [f32; 3] {
    [
        // Summation order of the image (batch 8, MEASURED): z term first, x term last.
        (m[8] * v[2] + m[4] * v[1]) + m[0] * v[0],
        (m[9] * v[2] + m[5] * v[1]) + m[1] * v[0],
        (m[10] * v[2] + m[6] * v[1]) + m[2] * v[0],
    ]
}

/// Pivot-rotation pose of a skeleton object with `nodes` node matrices (stride 0x40, array at `[obj+4]+0x10`).
/// All nodes are reset to identity, node 1 gets the pivot rotation. `0x1EC250` (TRST state 2, MEASURED flow):
/// angle = `[obj+0x7C]+0x40`, axis = `(0, 0, [0x475C78] = 1.0)`, pivot = room sub-entry `+0x78` vector. The caller sets
/// object flag `0x200000` on `[obj+0x28]` afterwards ([`POSE_DIRTY_FLAG`]).
pub fn trst_pose(nodes: usize, angle: f32, pivot: [f32; 3]) -> Vec<Mat4> {
    let mut out = vec![IDENTITY; nodes.max(2)];
    out[1] = rotation_about_pivot(angle, [0.0, 0.0, 1.0], pivot);
    out
}

/// Flag ORed into `[obj+0x28]` by both pose ticks.
pub const POSE_DIRTY_FLAG: u32 = 0x20_0000;

/// `0x1F2B00` (NKLC state 2, flow MEASURED, role INFERRED): the same reset, node 1 rotated by blend `+0xD0` about an
/// axis whose z component is the constant at `0x4760D0` (the vector is first turned by `rotation_y([obj+0x70])` and the
/// pivot vector of the room sub-entry `+0x78` likewise), and node 2 translated by `(0, -scale, 0)` through `0x153550`
/// when `+0x1CC != 1.0`.
pub fn nklc_pose(
    nodes: usize,
    yaw: f32,
    axis_z: f32,
    pivot: [f32; 3],
    blend: f32,
    scale: f32,
) -> Vec<Mat4> {
    let y = rotation_y(yaw);
    let axis = transform_direction(&y, [0.0, 0.0, axis_z]);
    let pivot = transform_direction(&y, pivot);
    let mut out = vec![IDENTITY; nodes.max(3)];
    out[1] = rotation_about_pivot(blend, axis, pivot);
    if scale != 1.0 {
        out[2] = translation([0.0, -scale, 0.0]);
    }
    out
}

/// Width given to both beam records by `0x1E0A70` (`0x41F00000`).
pub const BEAM_WIDTH: f32 = 30.0;

/// `0x1E0A70` (INFERRED role, flow MEASURED): two anchor points `a` (room sub-entry `+0xD8`... model anchor) and `b`
/// are rotated by the object matrix (`[obj+4]+0x10`, or `obj+0x274` for type "XPSC"), the object position `+0x64..0x6C`
/// is added, and the results are the endpoints of the two beam records (`0xFF490`, then width `0xFF420`).
/// Returns `(endpoint_a, endpoint_b, width)`.
pub fn beam_endpoints(
    matrix: &Mat4,
    position: [f32; 3],
    a: [f32; 3],
    b: [f32; 3],
) -> ([f32; 3], [f32; 3], f32) {
    let add = |v: [f32; 3]| [v[0] + position[0], v[1] + position[1], v[2] + position[2]];
    (
        add(transform_direction(matrix, a)),
        add(transform_direction(matrix, b)),
        BEAM_WIDTH,
    )
}

/// Bits of the timed-state word `+0x1A0` (`0xAE240` sets bit 0 with the flags argument).
pub const TIMED_ACTIVE: u32 = 1;
pub const TIMED_RELEASE_HANDLE: u32 = 2;
pub const TIMED_WAIT_ANIM: u32 = 4;
/// Object flag `+0x18` bit set every tick, and flag `+0x28` bit `0x400` (decal unlink once).
pub const TIMED_OBJECT_FLAG: u32 = 0x40;
pub const TIMED_DECAL_FLAG: u32 = 0x400;

/// Fade starts when the remaining frames fall below this (`0x7F`).
pub const FADE_FRAMES: i32 = 0x7F;

/// `0xAE240`: the timer in frames, `(int)(60 * seconds)` with `[0x4E8CEC] = 60`.
pub fn timed_frames(seconds: f32) -> i32 {
    (60.0 * seconds) as i32
}

/// State read and written by `0xAC560`.
#[derive(Clone, Debug, PartialEq)]
pub struct TimedObject {
    pub flags_18: u32,
    pub flags_28: u32,
    /// `+0x1A0`.
    pub mode: u32,
    /// `+0x1A4` remaining frames.
    pub timer: i32,
    /// `+0x10 & 0xFFFF`: non-zero while the attached animation still runs.
    pub anim_busy: bool,
    /// `+0x80` handle non-zero.
    pub has_handle: bool,
    /// `0x1679A0(handle)`: the table entry's signed byte `+0x1C8 <= 0`.
    pub handle_entry_idle: bool,
}

/// Side effects of one `0xAC560` call.
#[derive(Clone, Debug, PartialEq)]
pub enum TimedEffect {
    /// `0x170ED0`: release the handle entry and clear the handle.
    ReleaseHandle,
    /// `0x1129B0`: unlink decals owned by the object.
    UnlinkDecals,
    /// `0x94220(obj+4, 0xF, -1, -1, alpha)`: fade byte, written while the timer is below [`FADE_FRAMES`].
    Fade { alpha: u8 },
    /// `0xA9070`: callback kind 1 then destroy the object.
    Destroy,
}

/// `0xAC560` (MEASURED). `frames` is `[0x4E7944]`.
pub fn timed_tick(obj: &mut TimedObject, frames: i32) -> Vec<TimedEffect> {
    let mut fx = Vec::new();
    obj.flags_18 |= TIMED_OBJECT_FLAG;
    if obj.mode & TIMED_ACTIVE != 0 {
        if obj.mode & TIMED_WAIT_ANIM != 0 {
            if obj.anim_busy {
                return fx;
            }
            obj.mode &= !TIMED_WAIT_ANIM;
        }
        if obj.has_handle && obj.mode & TIMED_RELEASE_HANDLE != 0 {
            if !obj.handle_entry_idle {
                return fx;
            }
            obj.mode &= !TIMED_RELEASE_HANDLE;
            obj.has_handle = false;
            fx.push(TimedEffect::ReleaseHandle);
        }
    }
    if obj.flags_28 & TIMED_DECAL_FLAG == 0 {
        obj.flags_28 |= TIMED_DECAL_FLAG;
        fx.push(TimedEffect::UnlinkDecals);
    }
    if obj.timer > 0 {
        let step = frames.min(obj.timer);
        obj.timer -= step;
        if obj.timer < FADE_FRAMES {
            fx.push(TimedEffect::Fade {
                alpha: obj.timer as u8,
            });
        }
    }
    if obj.mode & TIMED_ACTIVE != 0 && obj.timer > 0 {
        return fx;
    }
    fx.push(TimedEffect::Destroy);
    fx
}

/// `0xA5CE0(object, state, function)` list side effect: the object is tracked in a global list when a tick is set
/// for state 0 (list `0x7B4FC0`), 8 (`0x7161C8`) and 9 (`0x716148`), removed when the function is null (MEASURED).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickList {
    State0,
    State8,
    State9,
}

/// The list a state maps to, `None` for the other states (only the slot store happens).
pub fn tick_list(state: u32) -> Option<TickList> {
    match state {
        0 => Some(TickList::State0),
        8 => Some(TickList::State8),
        9 => Some(TickList::State9),
        _ => None,
    }
}

/// Address of the per-state tick slot: `object + 0x154 + 4 * state` (MEASURED).
pub fn tick_slot_offset(state: u32) -> u32 {
    0x154 + 4 * state
}
