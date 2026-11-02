//! Blend space pose sampler `0x5AFB0` (T1668): the pose of a node taken from a 2D grid of key frames of one clip (or of two
//! clips mixed), as used for aim or look offsets. MEASURED against the ORIGINAL under Unicorn
//! (`tools/t1668_aim_gold.py`, `tests/aim_golden.rs`, real `0x150960` slerp, key sampler `0x59320` stubbed).
//!
//! Signature `0x5AFB0(ctx, out*, node)`. `ctx+4` is the object (`+0xA0` / `+0xA4` two animation ids, `+0xA8` mix flag, `+0xAC`
//! mix weight), `ctx+0x7C` the shape data (`+0xB08` scale, `+0x80` flag). Globals (runtime blend space cursor): rows
//! `[0x6D6180]`, `[0x6D77E8]`, columns `[0x6D77E0]`, `[0x6D6E10]` (so the four corner keys are `column + row * 10`: the clip stores
//! a grid ten keys wide), weights `[0x6D6E08]` (between the two columns) and `[0x6D6E0C]` (between the two rows).
//! The corner poses are slerped column wise with the first weight, then row wise with the second; the position comes from the first
//! corner (positions are not interpolated). With a mix flag the same is done on the second clip and the two results are slerped with
//! the mix weight. `out` receives position (3 floats) then quaternion (4 floats).
//!
//! Special case: in single clip mode, when the shape flag (`+0x80`) is set and `8 < node < 13` the routine returns one plain sample of the clip in animation
//! slot `[0x7E040A]` at key 0 instead ([`BlendInput::Plain`]).
use crate::blend::slerp;
use crate::posemath::Sample;

/// The grid cursor globals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cursor {
    /// `[0x6D6180]` and `[0x6D77E8]`: the two row indices.
    pub rows: [i32; 2],
    /// `[0x6D77E0]` and `[0x6D6E10]`: the two column indices.
    pub columns: [i32; 2],
    /// `[0x6D6E08]`: weight between the columns.
    pub column_weight: f32,
    /// `[0x6D6E0C]`: weight between the rows.
    pub row_weight: f32,
}

impl Cursor {
    /// The four corner keys in the order the original samples them.
    pub fn corner_keys(&self) -> [i32; 4] {
        let key = |column: i32, row: i32| column.wrapping_add(row.wrapping_mul(10));
        [
            key(self.columns[0], self.rows[0]),
            key(self.columns[1], self.rows[0]),
            key(self.columns[0], self.rows[1]),
            key(self.columns[1], self.rows[1]),
        ]
    }
}

/// Which clip(s) are sampled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlendInput {
    /// One clip.
    Single,
    /// Two clips mixed with this weight (`obj+0xAC`).
    Mix(f32),
    /// The shape flag special case: one sample at key 0 of the fixed clip.
    Plain,
}

/// A pose: position and quaternion.
pub type Pose = ([f32; 3], [f32; 4]);

fn grid_pose(cursor: &Cursor, mut sample: impl FnMut(i32) -> Sample) -> Pose {
    let [a, b, c, d] = cursor.corner_keys().map(&mut sample);
    let top = slerp(a.quaternion(), b.quaternion(), cursor.column_weight);
    let bottom = slerp(c.quaternion(), d.quaternion(), cursor.column_weight);
    (a.position(), slerp(top, bottom, cursor.row_weight))
}

/// `0x5AFB0`. `sample(clip index, key)` is `0x59320` for node `node` (clip index 0 = animation `+0xA0`, 1 = `+0xA4`, 2 = the
/// special clip).
pub fn blend_space_pose(
    cursor: &Cursor,
    input: BlendInput,
    mut sample: impl FnMut(usize, i32) -> Sample,
) -> Pose {
    match input {
        BlendInput::Plain => {
            let s = sample(2, 0);
            (s.position(), s.quaternion())
        }
        BlendInput::Single => grid_pose(cursor, |key| sample(0, key)),
        BlendInput::Mix(weight) => {
            let first = grid_pose(cursor, |key| sample(0, key));
            let second = grid_pose(cursor, |key| sample(1, key));
            (first.0, slerp(first.1, second.1, weight))
        }
    }
}

/// Whether the plain special case applies (single clip mode only, `shape flag` set and `8 < node < 13`).
pub fn is_plain(shape_flag: u32, node: i32) -> bool {
    shape_flag != 0 && node > 8 && node < 13
}

/// Degrees to radians factor `[0x475D0C]`.
const DEG_TO_RAD: f32 = f32::from_bits(0x3C8E_FA35);
/// Height thresholds `[0x478660]` (level below the global level) and `[0x476044]` (otherwise).
const HEIGHT_LOW_LEVEL: f32 = f32::from_bits(0x3F81_47AE);
const HEIGHT_HIGH_LEVEL: f32 = f32::from_bits(0x3F59_999A);

/// Inputs of the move animation probe `0x5B720(obj, id, t, out)` (T1668), MEASURED against the original with the key sampler, the
/// radius table lookup `0x9E260` and the ground probe `0xC5A50` stubbed (`tools/t1668_probe_gold.py`, `tests/probe_golden.rs`,
/// 800 cases, the rotation `0x153440` / `0x152090` real).
///
/// "Where would this move animation end up, and is that place acceptable?": the root displacement between key 0 and key
/// `trunc((keys - 1) * t)` of node 0 (x and z, y dropped) is rotated by the object's yaw and added to its position. If the end point is
/// within `radius` of the object table target (`[0x715F2C] + shape[+0x678] * 32`, x at `+0xC`, z at `+0x14`) the move is refused (result 0, `out` =
/// the end point). Otherwise the ground probe `0xC5A50` may adjust the end point; the result is 2 when the probe returned bit 2, plus 1 when the
/// object stands higher than the end point by more than the threshold (1.01 when the shape level is below the global level `[0x790950]`, else
/// 0.85). `out` receives the (adjusted) end point.
pub struct MoveQuery {
    /// `obj+0x70`, degrees.
    pub yaw_degrees: f32,
    /// `obj+0x64..0x6C`.
    pub position: [f32; 3],
    /// `shape+8` and the global `[0x790950]`.
    pub level: i32,
    pub level_threshold: i32,
    /// Position of the node 0 sample at key 0 and at the end key.
    pub start: [f32; 3],
    pub end: [f32; 3],
    /// Target x and z from the object table, and its radius from `0x9E260`.
    pub target: [f32; 2],
    pub radius: f32,
}

/// Returns `(result flags, end point)`. `probe` is `0xC5A50`: it receives the end point and returns `(flags, adjusted end point)`.
pub fn move_probe(
    query: &MoveQuery,
    probe: impl FnOnce([f32; 3]) -> (u32, [f32; 3]),
) -> (u32, [f32; 3]) {
    let displacement = [
        query.end[0] - query.start[0],
        0.0f32,
        query.end[2] - query.start[2],
    ];
    let angle = query.yaw_degrees * DEG_TO_RAD;
    // 0x153440: identity with cos / -sin / sin / cos (x87 sin and cos rounded to f32).
    let (sin, cos) = (
        crate::fmath::sin_f64(f64::from(angle)) as f32,
        crate::fmath::cos_f64(f64::from(angle)) as f32,
    );
    let m = {
        let mut m = [0.0f32; 16];
        for i in 0..4 {
            m[5 * i] = 1.0;
        }
        (m[0], m[2], m[8], m[10]) = (cos, -sin, sin, cos);
        m
    };
    // 0x152090: the vector is transformed with the product order of the SSE code.
    let [vx, vy, vz] = displacement;
    let rotated = [
        (m[8] * vz + m[4] * vy) + m[0] * vx,
        (m[9] * vz + m[5] * vy) + m[1] * vx,
        (m[10] * vz + m[6] * vy) + m[2] * vx,
    ];
    let end = [
        query.position[0] + rotated[0],
        query.position[1] + rotated[1],
        query.position[2] + rotated[2],
    ];
    let (dx, dz) = (query.target[0] - end[0], query.target[1] - end[2]);
    let radius = f64::from(query.radius);
    let reach = radius * radius;
    let distance = f64::from(dz) * f64::from(dz) + f64::from(dx) * f64::from(dx);
    if reach > distance {
        return (0, end);
    }
    let (probe_flags, end) = probe(end);
    let mut flags = if probe_flags & 2 != 0 { 2 } else { 0 };
    let threshold = if query.level < query.level_threshold {
        HEIGHT_LOW_LEVEL
    } else {
        HEIGHT_HIGH_LEVEL
    };
    if query.position[1] - end[1] > threshold {
        flags |= 1;
    }
    (flags, end)
}
