//! T1872: the pose and skeleton engine below `0x976E0`: the matrix stack (`0x59BF0`, `0x59C00`, `0x59CB0`), the
//! context lookup `0x58270`, the keyframe sampler `0x59320` (with `0x591D0` and the quaternion unpacker `0x59210`),
//! the part tree walks `0x582A0`, `0x58340`, `0x583E0`, `0x96860`, the quaternion to matrix routine `0x1506B0` and
//! the identity / matrix copy `0x153240`, `0x1531D0`. MEASURED against the Unicorn oracle (golden vectors, see
//! `docs/t-story-event-pose-engine-t1872.md`); names are INFERRED, addresses, offsets and control flow MEASURED.
//!
//! Float constants of the image (`0x475C78` 1.0, `0x475D10` 2.0, `0x475D60` epsilon, `0x478650`, `0x47865C`,
//! `0x475DB4`, `0x478654`, `0x478658`) are read from guest memory like in `svc_shape`. Engine routines below
//! (`0x903D0` part visitor, the error report `0x389B0`) stay behind [`DestroyEffects`]. T1875 wired the streamed sampler
//! `0x58DF0` ([`super::svc_stream::stream_sample`]) and the matrix blend `0x1526C0` ([`matrix_blend_call`], the bit exact
//! `tsfp_engine_anim::blend::matrix_blend`) into the pose walks and [`PosePorts`].
//! The x87 parts (`0x59210`, the square root of `0x59320`) are computed in f64 like the oracle's 53 bit control
//! word (`0x27F`).

use super::strip_update::GuestMemory;
use super::svc_objects::DestroyEffects;
use super::world_queries::{Mat4, mat_mul};

pub mod callee {
    pub const POSE_CONTEXT: u32 = 0x5_8270;
    pub const POSE_BONE: u32 = 0x5_9320;
    pub const POSE_A: u32 = 0x5_83E0;
    pub const POSE_B: u32 = 0x5_82A0;
    pub const POSE_C: u32 = 0x5_8340;
    pub const SET_TAG: u32 = 0x5_9BF0;
    pub const STACK_APPLY: u32 = 0x5_9C00;
    pub const STACK_POP: u32 = 0x5_9CB0;
    pub const POSE_F: u32 = 0x9_6860;
    pub const QUAT_MATRIX: u32 = 0x15_06B0;
    pub const IDENTITY: u32 = 0x15_3240;
    pub const MATRIX_COPY: u32 = 0x15_31D0;
    /// Engine routines the ports leave behind the effects.
    pub const KEYS_COMPRESSED: u32 = 0x5_8DF0; // E (context, index, frame, out, scale), tail called by 0x59320
    pub const MATRIX_BLEND: u32 = 0x15_26C0; // E (out, a, b, t)
    pub const PART_VISIT: u32 = 0x9_03D0; // E (context, part, vector, 0)
    pub const MATRIX_MUL: u32 = 0x15_3270;
}

const CONTEXT_INDEX: u32 = 0x7E_03C0;
const CONTEXT_TABLE: u32 = 0x7E_3954;
const POSE_TAG: u32 = 0x6D_6E18;
const STACK_BASE: u32 = 0x6D_6188;
const STACK_TOP: u32 = 0x6D_77F0;
const STACK_FLAG: u32 = 0x6D_77F4;
const MUL_SCRATCH: u32 = 0x74_9768;
const IDENTITY_MATRIX: Mat4 = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

const C_ONE: u32 = 0x47_5C78;
const C_TWO: u32 = 0x47_5D10;
const C_EPSILON: u32 = 0x47_5D60;
const C_QUAT_STEP: u32 = 0x47_8650;
const C_POS_STEP: u32 = 0x47_865C;
const C_POS_BIAS: u32 = 0x47_5DB4;
const C_PACK_STEP: u32 = 0x47_8658;
const C_PACK_BIAS: u32 = 0x47_8654;
/// Dropped component and the order of the three stored ones, per 2 bit index (image table `0x4D1998`, MEASURED).
const PACK_ORDER: [[u32; 3]; 4] = [[1, 2, 3], [0, 2, 3], [0, 1, 3], [0, 1, 2]];

fn r32(memory: &dyn GuestMemory, address: u32) -> u32 {
    memory.read_u32(address)
}

fn w32(memory: &mut dyn GuestMemory, address: u32, value: u32) {
    memory.write_u32(address, value);
}

fn rf(memory: &dyn GuestMemory, address: u32) -> f32 {
    f32::from_bits(memory.read_u32(address))
}

fn wf(memory: &mut dyn GuestMemory, address: u32, value: f32) {
    memory.write_u32(address, value.to_bits());
}

fn r16(memory: &dyn GuestMemory, address: u32) -> u32 {
    u32::from(memory.read_u8(address)) | u32::from(memory.read_u8(address.wrapping_add(1))) << 8
}

fn sx8(memory: &dyn GuestMemory, address: u32) -> i32 {
    i32::from(memory.read_u8(address) as i8)
}

fn load_matrix(memory: &dyn GuestMemory, address: u32) -> Mat4 {
    let mut matrix = [0.0f32; 16];
    for (k, cell) in matrix.iter_mut().enumerate() {
        *cell = rf(memory, address.wrapping_add(4 * k as u32));
    }
    matrix
}

fn store_matrix(memory: &mut dyn GuestMemory, address: u32, matrix: &Mat4) {
    for (k, cell) in matrix.iter().enumerate() {
        wf(memory, address.wrapping_add(4 * k as u32), *cell);
    }
}

/// `0x58270(word)`: the pose context of a table word: `[[0x7E3954] + 4 * int16 [0x7E03C0 + 2 word]]`, 0 for a negative entry.
pub fn pose_context(memory: &dyn GuestMemory, word: u32) -> u32 {
    let at = CONTEXT_INDEX.wrapping_add(word.wrapping_mul(2));
    let entry = r16(memory, at) as u16 as i16;
    if entry < 0 {
        return 0;
    }
    let table = r32(memory, CONTEXT_TABLE);
    r32(
        memory,
        table.wrapping_add((i32::from(entry) as u32).wrapping_mul(4)),
    )
}

/// `0x59BF0(tag)`: stores the pose tag word `[0x6D6E18]`.
pub fn set_tag(memory: &mut dyn GuestMemory, tag: u32) {
    w32(memory, POSE_TAG, tag);
}

/// `0x59CB0()`: pops the matrix stack (`[0x6D77F0] -= 1`, flag `[0x6D77F4] = 0`).
pub fn stack_pop(memory: &mut dyn GuestMemory) {
    let top = r32(memory, STACK_TOP).wrapping_sub(1);
    w32(memory, STACK_TOP, top);
    w32(memory, STACK_FLAG, 0);
}

/// `0x153240(out)`: the identity matrix.
pub fn identity(memory: &mut dyn GuestMemory, out: u32) {
    store_matrix(memory, out, &IDENTITY_MATRIX);
}

/// `0x1531D0(dest, source)`: 64 byte copy (all sixteen words are read before the first store).
pub fn matrix_copy(memory: &mut dyn GuestMemory, dest: u32, source: u32) {
    let matrix = load_matrix(memory, source);
    store_matrix(memory, dest, &matrix);
}

/// `dest = a * b` as `0x153270(dest, a, b)` (inputs read before the stores). When `dest` is one of the inputs the
/// original computes into its global scratch matrix `0x749768` and copies it over `dest`, so the scratch keeps the
/// product (MEASURED, dumped by the chain and root goldens).
fn matrix_mul(memory: &mut dyn GuestMemory, dest: u32, a: u32, b: u32) {
    let left = load_matrix(memory, a);
    let right = load_matrix(memory, b);
    let product = mat_mul(&left, &right);
    if dest == a || dest == b {
        store_matrix(memory, MUL_SCRATCH, &product);
    }
    store_matrix(memory, dest, &product);
}

/// `0x59C00(matrix, flags)` (only the low byte of `flags` is read): the matrix stack at `0x6D6188` (64 byte cells,
/// index `[0x6D77F0]`, cell `top`). Bit 1: the cell above the top becomes `matrix * top` (otherwise it takes the
/// matrix: the top cell itself when bit 0 is clear). Bit 0 pushes (index + 1, flag `[0x6D77F4] = 0`); a multiply
/// without push sets the flag to 1. MEASURED.
pub fn stack_apply(memory: &mut dyn GuestMemory, matrix: u32, flags: u32) {
    let flags = flags & 0xFF;
    let top = r32(memory, STACK_TOP);
    let current = STACK_BASE.wrapping_add(top.wrapping_shl(6));
    let above = current.wrapping_add(0x40);
    if flags & 2 != 0 {
        matrix_mul(memory, above, matrix, current);
        if flags & 1 == 0 {
            w32(memory, STACK_FLAG, 1);
            return;
        }
    } else if flags & 1 != 0 {
        matrix_copy(memory, above, matrix);
    } else {
        matrix_copy(memory, current, matrix);
        return;
    }
    w32(memory, STACK_TOP, top.wrapping_add(1));
    w32(memory, STACK_FLAG, 0);
}

/// `0x1506B0(matrix, quaternion)`: unit quaternion `(x, y, z, w)` (scaled by `2 / |q|^2`) to a row major rotation
/// matrix, translation row zero and `m[15] = 1.0`; f32 operations in the order of the original. MEASURED.
pub fn quat_matrix(memory: &mut dyn GuestMemory, matrix: u32, quaternion: u32) {
    let (x, y, z, w) = (
        rf(memory, quaternion),
        rf(memory, quaternion.wrapping_add(4)),
        rf(memory, quaternion.wrapping_add(8)),
        rf(memory, quaternion.wrapping_add(12)),
    );
    let one = rf(memory, C_ONE);
    let norm = (x * x + y * y + z * z) + w * w;
    let inverse = rf(memory, C_TWO) / norm;
    let zs = z * inverse;
    let xs = x * inverse;
    let ys = y * inverse;
    let yw = ys * w;
    let zw = zs * w;
    let xx = x * xs;
    let zz = z * zs;
    let xy = x * ys;
    let yy = y * ys;
    let yz = y * zs;
    let xw = xs * w;
    let xz = x * zs;
    let cells: [(u32, f32); 16] = [
        (0x00, one - (zz + yy)),
        (0x04, xy + zw),
        (0x08, xz - yw),
        (0x0C, 0.0),
        (0x10, xy - zw),
        (0x14, one - (zz + xx)),
        (0x18, yz + xw),
        (0x1C, 0.0),
        (0x20, xz + yw),
        (0x24, yz - xw),
        (0x28, one - (yy + xx)),
        (0x2C, 0.0),
        (0x30, 0.0),
        (0x34, 0.0),
        (0x38, 0.0),
        (0x3C, rf(memory, C_ONE)),
    ];
    for (offset, value) in cells {
        wf(memory, matrix.wrapping_add(offset), value);
    }
}

fn copy_bytes(memory: &mut dyn GuestMemory, dest: u32, source: u32, count: u32) {
    for k in 0..count {
        let byte = memory.read_u8(source.wrapping_add(k));
        memory.write_u8(dest.wrapping_add(k), byte);
    }
}

/// `0x59210(source, out)`: unpacks a 32 bit "smallest three" quaternion at `source`: bits 0..1 name the dropped
/// component, three 10 bit fields (shifts 2, 12, 22) become `field * [0x478658] - [0x478654]` into the quaternion
/// cells of `out` (`out + 0xC + 4 * index`, order table `PACK_ORDER`), the dropped one is
/// `sqrt(1 - (c^2 + b^2) - a^2)` (x87 at 53 bits, so f64). MEASURED.
pub fn unpack_quaternion(memory: &mut dyn GuestMemory, source: u32, out: u32) {
    let packed = (0..4).fold(0u32, |acc, k| {
        acc | u32::from(memory.read_u8(source.wrapping_add(k))) << (8 * k)
    });
    let dropped = (packed & 3) as usize;
    let order = PACK_ORDER[dropped];
    let step = f64::from(rf(memory, C_PACK_STEP));
    let bias = f64::from(rf(memory, C_PACK_BIAS));
    let fields = [(packed >> 2) & 0x3FF, (packed >> 12) & 0x3FF, packed >> 22];
    for (field, index) in fields.iter().zip(order) {
        let value = (f64::from(*field) * step - bias) as f32;
        wf(memory, out.wrapping_add(0xC + 4 * index), value);
    }
    let cell = |memory: &dyn GuestMemory, index: u32| {
        f64::from(rf(memory, out.wrapping_add(0xC + 4 * index)))
    };
    let (c, b, a) = (
        cell(memory, order[0]),
        cell(memory, order[1]),
        cell(memory, order[2]),
    );
    let sum = c * c + b * b + a * a;
    let remainder = f64::from(rf(memory, C_ONE)) - sum;
    let root = if remainder < 0.0 {
        f32::from_bits(0xFFC0_0000)
    } else {
        remainder.sqrt() as f32
    };
    wf(memory, out.wrapping_add(0xC + 4 * dropped as u32), root);
}

/// The fourth component from the first three (`0x59554`): `w = sqrt(1 - (x^2 + y^2 + z^2))` when above the
/// epsilon `[0x475D60]` (the square root through x87), else 0; stored at `out + 0x18`.
fn complete_quaternion(memory: &mut dyn GuestMemory, out: u32) {
    let (x, y, z) = (
        rf(memory, out.wrapping_add(0xC)),
        rf(memory, out.wrapping_add(0x10)),
        rf(memory, out.wrapping_add(0x14)),
    );
    let remainder = rf(memory, C_ONE) - ((x * x + y * y) + z * z);
    let w = if remainder > rf(memory, C_EPSILON) {
        (f64::from(remainder).sqrt()) as f32
    } else {
        0.0
    };
    wf(memory, out.wrapping_add(0x18), w);
}

/// Three (or four) 16 bit quaternion fields at `base` (`u16 * [0x478650] - 1.0`) into `out + 0xC..`. With `three`
/// the fourth is derived ([`complete_quaternion`]).
fn packed_quaternion(memory: &mut dyn GuestMemory, out: u32, base: u32, three: bool) {
    let step = rf(memory, C_QUAT_STEP);
    let one = rf(memory, C_ONE);
    let field = |memory: &dyn GuestMemory, index: u32| {
        (r16(memory, base.wrapping_add(2 * index)) as f32) * step - one
    };
    for index in 0..3u32 {
        let value = field(memory, index);
        wf(memory, out.wrapping_add(0xC + 4 * index), value);
    }
    if three {
        complete_quaternion(memory, out);
    } else {
        let value = field(memory, 3);
        wf(memory, out.wrapping_add(0x18), value);
    }
}

fn packed_position(memory: &mut dyn GuestMemory, out: u32, base: u32) {
    let step = rf(memory, C_POS_STEP);
    let bias = rf(memory, C_POS_BIAS);
    for index in 0..3u32 {
        let value = (r16(memory, base.wrapping_add(2 * index)) as f32) * step - bias;
        wf(memory, out.wrapping_add(4 * index), value);
    }
}

fn nonnull(pointer: u32, offset: u32) -> u32 {
    if pointer == 0 {
        0
    } else {
        pointer.wrapping_add(offset)
    }
}

/// `0x59320(context, index, frame, out, scale)`: samples the channel `index` of an animation context at `frame`
/// (clamped to `[context+0x10] - 1`) into `out` (position floats `0..2`, quaternion floats at `+0xC..+0x18`), the
/// position scaled by `scale`. Context flags `[+8]`: bit 1 hands over to the compressed sampler `0x58DF0`, bit 0
/// selects 16 bit quaternion fields, bit 4 stores only three quaternion floats (the fourth is derived). The channel
/// record is `[context+0x28] + 0x20 * index` (kind `+4` 0..13, key data `+0x14`). Kinds 0, 2, 4, 8, 10 hold float or
/// 16 bit keys, 11..13 a 16 bit position and a packed quaternion, 1, 3, 5..7, 9 and above 13 sample nothing.
/// MEASURED (golden cases for every kind and flag combination).
pub fn pose_bone(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    context: u32,
    index: u32,
    frame: u32,
    out: u32,
    scale: u32,
) {
    let flags = r32(memory, context.wrapping_add(8));
    if flags & 2 != 0 {
        super::svc_stream::stream_sample(memory, effects, context, index, frame, out, scale);
        return;
    }
    let short = (flags >> 4) & 1; // bit 4: three quaternion floats
    let packed = flags & 1 != 0;
    let frames = r32(memory, context.wrapping_add(0x10)) as i32;
    let mut frame = frame as i32;
    if frame >= frames {
        frame = frames.wrapping_sub(1);
    }
    let frame = frame as u32;
    let channel = r32(memory, context.wrapping_add(0x28)).wrapping_add(index.wrapping_shl(5));
    let kind = r32(memory, channel.wrapping_add(4));
    let data = r32(memory, channel.wrapping_add(0x14));
    let finish = short != 0;
    let copy_pose = |memory: &mut dyn GuestMemory, finish_short: bool, packed_pose: bool| {
        if packed_pose {
            copy_bytes(memory, out, data, 0xC);
            packed_quaternion(memory, out, nonnull(data, 0xC), finish_short);
        } else if finish_short {
            copy_bytes(memory, out, data, 0x18);
            complete_quaternion(memory, out);
        } else {
            copy_bytes(memory, out, data, 0x1C);
        }
    };
    match kind {
        0 => copy_pose(memory, finish, packed),
        2 => {
            copy_bytes(memory, out, data, 0xC);
            let track = nonnull(data, frame.wrapping_mul(12));
            if !packed {
                if finish {
                    copy_bytes(memory, out.wrapping_add(0xC), track, 0xC);
                    complete_quaternion(memory, out);
                } else {
                    copy_bytes(memory, out.wrapping_add(0xC), track, 0x10);
                }
            } else {
                let stride = (4u32.wrapping_sub(short))
                    .wrapping_mul(frame)
                    .wrapping_mul(2);
                packed_quaternion(memory, out, nonnull(data, stride.wrapping_add(0xC)), finish);
            }
        }
        4 if frame == 0 => copy_pose(memory, finish, packed),
        4 => {
            if !packed {
                let skip = frame
                    .wrapping_mul(3)
                    .wrapping_sub(short)
                    .wrapping_mul(4)
                    .wrapping_add(0x10);
                let size = (4u32.wrapping_sub(short)).wrapping_mul(4);
                copy_bytes(memory, out.wrapping_add(0xC), nonnull(data, 0xC), size);
                copy_bytes(memory, out, nonnull(data, skip), 0xC);
                if finish {
                    complete_quaternion(memory, out);
                }
            } else {
                packed_quaternion(memory, out, nonnull(data, 0xC), finish);
                let skip = frame
                    .wrapping_mul(6)
                    .wrapping_sub(short)
                    .wrapping_mul(2)
                    .wrapping_add(8);
                copy_bytes(memory, out, nonnull(data, skip), 0xC);
            }
        }
        8 => {
            if !packed {
                let words = 7u32.wrapping_sub(short);
                let source = nonnull(data, words.wrapping_mul(frame).wrapping_mul(4));
                copy_bytes(memory, out, source, words.wrapping_mul(4));
                if finish {
                    complete_quaternion(memory, out);
                }
            } else {
                let offset = (10u32.wrapping_sub(short))
                    .wrapping_mul(frame)
                    .wrapping_mul(2);
                copy_bytes(memory, out, nonnull(data, offset), 0xC);
                packed_quaternion(memory, out, nonnull(data, offset.wrapping_add(0xC)), finish);
            }
        }
        10 => {
            let base = data.wrapping_add(frame.wrapping_shl(3).wrapping_mul(4));
            for k in 0..7u32 {
                let word = r32(memory, base.wrapping_add(4 * k));
                w32(memory, out.wrapping_add(4 * k), word);
            }
            if finish {
                complete_quaternion(memory, out);
            }
        }
        11 => {
            let source = nonnull(data, frame.wrapping_mul(10));
            packed_position(memory, out, source);
            unpack_quaternion(memory, source.wrapping_add(6), out);
        }
        12 => {
            let source = nonnull(data, frame.wrapping_mul(4).wrapping_add(6));
            packed_position(memory, out, data);
            unpack_quaternion(memory, source, out);
        }
        13 => {
            let source = nonnull(data, frame.wrapping_mul(6).wrapping_add(4));
            packed_position(memory, out, source);
            unpack_quaternion(memory, data, out);
        }
        _ => {
            if finish {
                complete_quaternion(memory, out);
            }
        }
    }
    let scale = f32::from_bits(scale);
    for k in 0..3u32 {
        let value = rf(memory, out.wrapping_add(4 * k)) * scale;
        wf(memory, out.wrapping_add(4 * k), value);
    }
}

/// The part record of `index` in the context's part table `[context+4]` (stride `0xC8`, the table's first dword
/// is the first part's number).
fn part_of(memory: &dyn GuestMemory, context: u32, index: u32) -> (u32, u32) {
    let table = r32(memory, context.wrapping_add(4));
    let first = r32(memory, table);
    let base = table.wrapping_sub(first.wrapping_mul(0xC8));
    (base, base.wrapping_add(index.wrapping_mul(0xC8)))
}

/// `0x582A0(context, matrices, index, parent)`: walks the part tree from `index` (siblings `+0x4C`, children
/// `+0x4B`, none when negative; children of a kind 3 part are skipped). A kind 0 part (byte `+0x48`) with a
/// non-negative `parent` multiplies its matrix cell (`matrices + 64 * byte +0x49`) by the parent's cell
/// (`m = m * parent`); its children get it as their parent, other kinds pass the incoming parent on. MEASURED.
pub fn pose_chain_a(
    memory: &mut dyn GuestMemory,
    context: u32,
    matrices: u32,
    index: u32,
    parent: u32,
) {
    let mut index = index;
    loop {
        let (base, part) = part_of(memory, context, index);
        let mut child_parent = parent;
        if memory.read_u8(part.wrapping_add(0x48)) == 0 {
            if (parent as i32) >= 0 {
                let own = (sx8(memory, part.wrapping_add(0x49)) as u32).wrapping_shl(6);
                let up_part = base.wrapping_add(parent.wrapping_mul(0xC8));
                let up = (sx8(memory, up_part.wrapping_add(0x49)) as u32).wrapping_shl(6);
                matrix_mul(
                    memory,
                    matrices.wrapping_add(own),
                    matrices.wrapping_add(own),
                    matrices.wrapping_add(up),
                );
            }
            child_parent = index;
        }
        let child = sx8(memory, part.wrapping_add(0x4B));
        if child >= 0 && memory.read_u8(part.wrapping_add(0x48)) != 3 {
            pose_chain_a(memory, context, matrices, child as u32, child_parent);
        }
        let sibling = sx8(memory, part.wrapping_add(0x4C));
        if sibling < 0 {
            return;
        }
        index = sibling as u32;
    }
}

/// `0x58340(context, matrices, index, parent)`: like [`pose_chain_a`] but the product `m = m * parent` is taken
/// for kind 2 parts and the children of a kind 0 part get that part as parent. MEASURED.
pub fn pose_chain_b(
    memory: &mut dyn GuestMemory,
    context: u32,
    matrices: u32,
    index: u32,
    parent: u32,
) {
    let mut index = index;
    loop {
        let (base, part) = part_of(memory, context, index);
        let kind = memory.read_u8(part.wrapping_add(0x48));
        if kind == 2 && (parent as i32) >= 0 {
            let own = (sx8(memory, part.wrapping_add(0x49)) as u32).wrapping_shl(6);
            let up_part = base.wrapping_add(parent.wrapping_mul(0xC8));
            let up = (sx8(memory, up_part.wrapping_add(0x49)) as u32).wrapping_shl(6);
            matrix_mul(
                memory,
                matrices.wrapping_add(own),
                matrices.wrapping_add(own),
                matrices.wrapping_add(up),
            );
        }
        let child_parent = if kind == 0 { index } else { parent };
        let child = sx8(memory, part.wrapping_add(0x4B));
        if child >= 0 && memory.read_u8(part.wrapping_add(0x48)) != 3 {
            pose_chain_b(memory, context, matrices, child as u32, child_parent);
        }
        let sibling = sx8(memory, part.wrapping_add(0x4C));
        if sibling < 0 {
            return;
        }
        index = sibling as u32;
    }
}

/// `0x1526C0(a, b, out, t)` over guest memory (cdecl; T1873 probed that the THIRD argument is the destination, the
/// T1872 walk `0x583E0` passes `(pose matrix, child cell, own cell, blend)`, so the part's own cell is overwritten):
/// both inputs are read as 16 f32, the rotation (slerp of the matrix quaternions) is written to `out` and then the
/// translation row is blended from the inputs as they are in memory at that moment (aliasing with `out` included),
/// bit exact with `tsfp_engine_anim::blend::matrix_blend` when nothing aliases.
pub fn matrix_blend_call(memory: &mut dyn GuestMemory, a: u32, b: u32, out: u32, t: u32) {
    use tsfp_engine_anim::interp::{blend_rotation, blend_translation};
    let read = |memory: &dyn GuestMemory, base: u32| -> [f32; 16] {
        std::array::from_fn(|k| f32::from_bits(r32(memory, base.wrapping_add(4 * k as u32))))
    };
    let t = f32::from_bits(t);
    let rotation = blend_rotation(&read(memory, a), &read(memory, b), t);
    for (k, value) in rotation.iter().enumerate() {
        w32(memory, out.wrapping_add(4 * k as u32), value.to_bits());
    }
    // The original reads the translation rows of both inputs from memory after the rotation was stored.
    for k in 12..15u32 {
        let (first, second) = (
            rf(memory, a.wrapping_add(4 * k)),
            rf(memory, b.wrapping_add(4 * k)),
        );
        wf(
            memory,
            out.wrapping_add(4 * k),
            blend_translation(first, second, t),
        );
    }
}

/// `0x583E0(slot, index, flag)`: walks the part tree of the slot's model (`[slot+4]`-style context at `slot`, parts
/// `[slot+4]`). For each kind 2 part the matrix blend `0x1526C0(out, child cell, own cell, t)` is called, `out` =
/// `[[slot+0x8C]+0x78] + 64 * [[slot+0x8C]+4][index]`, the cells `[slot+0x10] + 64 * byte +0x49` of the first child and
/// of the part, `t` = 1.0 with a non-zero `flag`, else the part's float `+0xB8`. MEASURED.
pub fn pose_blend_walk(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    slot: u32,
    index: u32,
    flag: u32,
) {
    let mut index = index;
    loop {
        let (base, part) = part_of(memory, slot, index);
        if memory.read_u8(part.wrapping_add(0x48)) == 2 {
            let child = sx8(memory, part.wrapping_add(0x4B)) as u32;
            let pose = r32(memory, slot.wrapping_add(0x8C));
            let child_part = base.wrapping_add(child.wrapping_mul(0xC8));
            let child_cell = (sx8(memory, child_part.wrapping_add(0x49)) as u32).wrapping_shl(6);
            let matrices = r32(memory, slot.wrapping_add(0x10));
            let blend = if flag != 0 {
                0x3F80_0000
            } else {
                r32(memory, part.wrapping_add(0xB8))
            };
            let own_cell = (sx8(memory, part.wrapping_add(0x49)) as u32).wrapping_shl(6);
            let order = r32(
                memory,
                r32(memory, pose.wrapping_add(4)).wrapping_add(index.wrapping_mul(4)),
            )
            .wrapping_shl(6);
            let out = r32(memory, pose.wrapping_add(0x78)).wrapping_add(order);
            matrix_blend_call(
                memory,
                out,
                child_cell.wrapping_add(matrices),
                own_cell.wrapping_add(matrices),
                blend,
            );
        }
        let child = sx8(memory, part.wrapping_add(0x4B));
        if child >= 0 && memory.read_u8(part.wrapping_add(0x48)) != 3 {
            pose_blend_walk(memory, effects, slot, child as u32, flag);
        }
        let sibling = sx8(memory, part.wrapping_add(0x4C));
        if sibling < 0 {
            return;
        }
        index = sibling as u32;
    }
}

/// `0x96860(context, index, vector)`: walks the part tree from `index` (as above) and calls the part visitor
/// `0x903D0(context, part, vector, 0)` for every part. MEASURED.
pub fn visit_parts(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    context: u32,
    index: u32,
    vector: u32,
) {
    let mut index = index;
    loop {
        let (_, part) = part_of(memory, context, index);
        effects.call(memory, callee::PART_VISIT, &[context, part, vector, 0]);
        let child = sx8(memory, part.wrapping_add(0x4B));
        if child >= 0 && memory.read_u8(part.wrapping_add(0x48)) != 3 {
            visit_parts(memory, effects, context, child as u32, vector);
        }
        let sibling = sx8(memory, part.wrapping_add(0x4C));
        if sibling < 0 {
            return;
        }
        index = sibling as u32;
    }
}

/// The callee addresses [`PosePorts`] handles.
pub fn is_pose_callee(address: u32) -> bool {
    matches!(
        address,
        callee::POSE_CONTEXT
            | callee::POSE_BONE
            | callee::POSE_A
            | callee::POSE_B
            | callee::POSE_C
            | callee::SET_TAG
            | callee::STACK_APPLY
            | callee::STACK_POP
            | callee::POSE_F
            | callee::QUAT_MATRIX
            | callee::IDENTITY
            | callee::MATRIX_COPY
            | callee::KEYS_COMPRESSED
            | callee::MATRIX_BLEND
    )
}

/// Routes the pose engine callee addresses to the ports and forwards everything else to `inner`.
pub struct PosePorts<'a> {
    pub inner: &'a mut dyn DestroyEffects,
}

impl DestroyEffects for PosePorts<'_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        let arg = |index: usize| args.get(index).copied().unwrap_or(0);
        match address {
            callee::POSE_CONTEXT => return pose_context(memory, arg(0)),
            callee::POSE_BONE => {
                let inner = &mut *self.inner;
                pose_bone(memory, inner, arg(0), arg(1), arg(2), arg(3), arg(4));
            }
            callee::POSE_A => pose_blend_walk(memory, &mut *self.inner, arg(0), arg(1), arg(2)),
            callee::POSE_B => pose_chain_a(memory, arg(0), arg(1), arg(2), arg(3)),
            callee::POSE_C => pose_chain_b(memory, arg(0), arg(1), arg(2), arg(3)),
            callee::SET_TAG => set_tag(memory, arg(0)),
            callee::STACK_APPLY => stack_apply(memory, arg(0), arg(1)),
            callee::STACK_POP => stack_pop(memory),
            callee::POSE_F => visit_parts(memory, &mut *self.inner, arg(0), arg(1), arg(2)),
            callee::QUAT_MATRIX => quat_matrix(memory, arg(0), arg(1)),
            callee::IDENTITY => identity(memory, arg(0)),
            callee::MATRIX_COPY => matrix_copy(memory, arg(0), arg(1)),
            callee::KEYS_COMPRESSED => {
                let inner = &mut *self.inner;
                super::svc_stream::stream_sample(
                    memory,
                    inner,
                    arg(0),
                    arg(1),
                    arg(2),
                    arg(3),
                    arg(4),
                );
            }
            callee::MATRIX_BLEND => matrix_blend_call(memory, arg(0), arg(1), arg(2), arg(3)),
            _ => return self.inner.call(memory, address, args),
        }
        0
    }
}
