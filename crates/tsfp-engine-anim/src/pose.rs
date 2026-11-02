//! T1873: pose evaluator over parsed clips (`tsfp_format_anim::Clip`), a typed port of the bit-exact guest-memory
//! routines of `tsfp_gameplay::story_events::svc_pose` (T1872, MEASURED against the original under Unicorn).
//!
//! * [`sample`] is `0x59320` (the resident key sampler with `0x59210` and `0x591D0`): node `n`, key `k` to a 28 byte
//!   sample (position, quaternion x y z w), position scaled by `scale`.
//! * [`quat_matrix`] is `0x1506B0`, [`mat_mul`] is `0x153270`, [`compose_chain_a`] / [`compose_chain_b`] are the
//!   part tree walks `0x582A0` / `0x58340`.
//! * [`evaluate_pose`] chains them: every node of a clip becomes a local matrix (rotation from the quaternion,
//!   translation row from the position, as `0x976E0` fills it) and the part tree multiplies each by its parent.
//!
//! The image float constants are fixed here to the retail values (`0x475C78`, `0x475D60`, `0x478650`, `0x47865C`,
//! `0x475DB4`, `0x478658`, `0x478654`, the same numbers the goldens write into their worlds). x87 sections
//! (`0x59210` and the derived w) run in f64 like the 53 bit control word `0x27F` of the oracle.
use tsfp_format_anim::{Clip, FLAG_DERIVED_W, FLAG_PACKED16};

pub use crate::posemath::{Mat4, Sample, local_matrix, mat_mul, quat_matrix};

const ONE: f32 = f32::from_bits(0x3F80_0000); // 0x475C78
const EPSILON: f32 = f32::from_bits(0x3727_C5AC); // 0x475D60
const QUAT_STEP: f32 = f32::from_bits(0x3800_0080); // 0x478650
const POSITION_STEP: f32 = f32::from_bits(0x3900_0080); // 0x47865C
const POSITION_BIAS: f32 = f32::from_bits(0x4080_0000); // 0x475DB4
const PACK_STEP: f32 = f32::from_bits(0x3AB5_F00B); // 0x478658
const PACK_BIAS: f32 = f32::from_bits(0x3F35_C28F); // 0x478654
/// Dropped component and order of the three stored ones, per 2 bit index (image table `0x4D1998`).
const PACK_ORDER: [[usize; 3]; 4] = [[1, 2, 3], [0, 2, 3], [0, 1, 3], [0, 1, 2]];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoseError {
    /// A node, key or track byte range outside the clip.
    OutOfRange,
    /// The sampler has no layout for this selector.
    Selector(u32),
}

struct Track<'a>(&'a [u8]);

impl Track<'_> {
    fn bytes(&self, offset: usize, len: usize) -> Result<&[u8], PoseError> {
        offset
            .checked_add(len)
            .and_then(|end| self.0.get(offset..end))
            .ok_or(PoseError::OutOfRange)
    }
    fn word(&self, offset: usize) -> Result<u32, PoseError> {
        Ok(u32::from_le_bytes(self.bytes(offset, 4)?.try_into().unwrap()))
    }
    fn half(&self, offset: usize) -> Result<u32, PoseError> {
        Ok(u32::from(u16::from_le_bytes(self.bytes(offset, 2)?.try_into().unwrap())))
    }
    /// `count` raw dwords from `offset` into `out[at..]`.
    fn copy(&self, offset: usize, count: usize, out: &mut [u32; 7], at: usize) -> Result<(), PoseError> {
        for k in 0..count {
            out[at + k] = self.word(offset + 4 * k)?;
        }
        Ok(())
    }
}

/// `w = sqrt(1 - (x^2 + y^2 + z^2))` above the epsilon, else 0 (`0x59554`, the square root through x87).
fn complete(out: &mut [u32; 7]) {
    let [x, y, z] = [3, 4, 5].map(|i| f32::from_bits(out[i]));
    let remainder = ONE - ((x * x + y * y) + z * z);
    let w = if remainder > EPSILON { f64::from(remainder).sqrt() as f32 } else { 0.0 };
    out[6] = w.to_bits();
}

/// Three (or four) 16 bit quaternion fields at `offset`.
fn packed_quaternion(t: &Track, offset: usize, three: bool, out: &mut [u32; 7]) -> Result<(), PoseError> {
    let field = |index: usize| -> Result<f32, PoseError> {
        Ok((t.half(offset + 2 * index)? as f32) * QUAT_STEP - ONE)
    };
    for index in 0..3 {
        out[3 + index] = field(index)?.to_bits();
    }
    if three {
        complete(out);
    } else {
        out[6] = field(3)?.to_bits();
    }
    Ok(())
}

fn packed_position(t: &Track, offset: usize, out: &mut [u32; 7]) -> Result<(), PoseError> {
    for index in 0..3 {
        let value = (t.half(offset + 2 * index)? as f32) * POSITION_STEP - POSITION_BIAS;
        out[index] = value.to_bits();
    }
    Ok(())
}

/// `0x59210`: 32 bit smallest-three quaternion (bits 0..1 dropped component, 10 bit fields at 2, 12, 22).
fn unpack_quaternion(t: &Track, offset: usize, out: &mut [u32; 7]) -> Result<(), PoseError> {
    let packed = t.word(offset)?;
    let dropped = (packed & 3) as usize;
    let order = PACK_ORDER[dropped];
    let fields = [(packed >> 2) & 0x3FF, (packed >> 12) & 0x3FF, packed >> 22];
    for (field, index) in fields.iter().zip(order) {
        let value = f64::from(*field) * f64::from(PACK_STEP) - f64::from(PACK_BIAS);
        out[3 + index] = (value as f32).to_bits();
    }
    let [c, b, a] = [order[0], order[1], order[2]].map(|i| f64::from(f32::from_bits(out[3 + i])));
    let remainder = f64::from(ONE) - (c * c + b * b + a * a);
    let root = if remainder < 0.0 { f32::from_bits(0xFFC0_0000) } else { remainder.sqrt() as f32 };
    out[3 + dropped] = root.to_bits();
    Ok(())
}

/// `0x59320(clip, node, key, out, scale)`. `key` above the last key clamps to it (the original does not check
/// negative keys, here they are an error).
// portreg: va=0x00059320 tier=1
pub fn sample(clip: &Clip, node: usize, key: i32, scale: f32) -> Result<Sample, PoseError> {
    let flags = clip.flags();
    let keys = clip.key_count() as i32;
    let key = if key >= keys { keys - 1 } else { key };
    let key = usize::try_from(key).map_err(|_| PoseError::OutOfRange)?;
    let record = clip.nodes.get(node).ok_or(PoseError::OutOfRange)?;
    let t = Track(clip.tracks.get(node).ok_or(PoseError::OutOfRange)?);
    let packed = flags & FLAG_PACKED16 != 0;
    let short = usize::from(flags & FLAG_DERIVED_W != 0);
    let derived = short != 0;
    let mut out = [0u32; 7];
    // fixed form: position floats and quaternion (float words or 16 bit fields)
    let fixed = |t: &Track, out: &mut [u32; 7]| -> Result<(), PoseError> {
        if packed {
            t.copy(0, 3, out, 0)?;
            packed_quaternion(t, 12, derived, out)
        } else {
            t.copy(0, 7 - short, out, 0)?;
            if derived {
                complete(out);
            }
            Ok(())
        }
    };
    match record.selector() {
        0 => fixed(&t, &mut out)?,
        2 => {
            t.copy(0, 3, &mut out, 0)?;
            if packed {
                packed_quaternion(&t, 12 + 2 * (4 - short) * key, derived, &mut out)?;
            } else {
                // the original reads the float quaternion at D + 12 * key (T1273), kept
                t.copy(12 * key, 4 - short, &mut out, 3)?;
                if derived {
                    complete(&mut out);
                }
            }
        }
        4 if key == 0 => fixed(&t, &mut out)?,
        4 => {
            if packed {
                packed_quaternion(&t, 12, derived, &mut out)?;
                t.copy(8 + 2 * (6 * key - short), 3, &mut out, 0)?;
            } else {
                t.copy(12, 4 - short, &mut out, 3)?;
                t.copy(16 + 4 * (3 * key - short), 3, &mut out, 0)?;
                if derived {
                    complete(&mut out);
                }
            }
        }
        8 => {
            if packed {
                let at = 2 * (10 - short) * key;
                t.copy(at, 3, &mut out, 0)?;
                packed_quaternion(&t, at + 12, derived, &mut out)?;
            } else {
                t.copy(4 * (7 - short) * key, 7 - short, &mut out, 0)?;
                if derived {
                    complete(&mut out);
                }
            }
        }
        10 => {
            t.copy(32 * key, 7, &mut out, 0)?;
            if derived {
                complete(&mut out);
            }
        }
        11 => {
            packed_position(&t, 10 * key, &mut out)?;
            unpack_quaternion(&t, 10 * key + 6, &mut out)?;
        }
        12 => {
            packed_position(&t, 0, &mut out)?;
            unpack_quaternion(&t, 6 + 4 * key, &mut out)?;
        }
        13 => {
            packed_position(&t, 4 + 6 * key, &mut out)?;
            unpack_quaternion(&t, 0, &mut out)?;
        }
        other => return Err(PoseError::Selector(other)),
    }
    for cell in out.iter_mut().take(3) {
        *cell = (f32::from_bits(*cell) * scale).to_bits();
    }
    Ok(Sample { bits: out })
}

/// A part record reduced to what the walks read (`+0x48` kind, `+0x49` signed matrix cell, `+0x4B` child,
/// `+0x4C` sibling, negative ends a list).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part {
    pub kind: u8,
    pub cell: i8,
    pub child: i8,
    pub sibling: i8,
}

/// Which kind multiplies by its parent: `0x582A0` kind 0 (children of a kind 0 part get it as parent), `0x58340`
/// kind 2 (children of a kind 0 part get it as parent too).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    A,
    B,
}

/// `0x582A0` / `0x58340(context, matrices, index, parent)`: `matrices[cell] = matrices[cell] * matrices[parent cell]`.
/// `parent` is a part number, negative for none. Cycles in the part links loop forever in the original; here
/// the walk is bounded by `parts.len()` visits per list and reports `OutOfRange`.
pub fn compose_chain(
    chain: Chain,
    parts: &[Part],
    matrices: &mut [Mat4],
    index: usize,
    parent: i32,
) -> Result<(), PoseError> {
    let mut budget = parts.len() * 4 + 4;
    walk(chain, parts, matrices, index, parent, &mut budget)
}

fn cell_of(parts: &[Part], matrices: &[Mat4], part: usize) -> Result<usize, PoseError> {
    let cell = usize::try_from(parts.get(part).ok_or(PoseError::OutOfRange)?.cell).map_err(|_| PoseError::OutOfRange)?;
    if cell < matrices.len() { Ok(cell) } else { Err(PoseError::OutOfRange) }
}

fn walk(
    chain: Chain,
    parts: &[Part],
    matrices: &mut [Mat4],
    start: usize,
    parent: i32,
    budget: &mut usize,
) -> Result<(), PoseError> {
    let mut index = start;
    loop {
        *budget = budget.checked_sub(1).ok_or(PoseError::OutOfRange)?;
        let part = *parts.get(index).ok_or(PoseError::OutOfRange)?;
        let multiplies = match chain {
            Chain::A => part.kind == 0,
            Chain::B => part.kind == 2,
        };
        if multiplies && parent >= 0 {
            let own = cell_of(parts, matrices, index)?;
            let up = cell_of(parts, matrices, parent as usize)?;
            matrices[own] = mat_mul(&matrices[own], &matrices[up]);
        }
        let child_parent = if part.kind == 0 { index as i32 } else { parent };
        if part.child >= 0 && part.kind != 3 {
            walk(chain, parts, matrices, part.child as usize, child_parent, budget)?;
        }
        if part.sibling < 0 {
            return Ok(());
        }
        index = part.sibling as usize;
    }
}

/// Evaluates a clip at `key` over a part tree: node `n` of the clip drives the matrix cell `n`, the chain
/// multiplies every part by its parent. Returns the pose matrices (one per clip node).
pub fn evaluate_pose(
    clip: &Clip,
    parts: &[Part],
    root: usize,
    key: i32,
    scale: f32,
    chain: Chain,
) -> Result<Vec<Mat4>, PoseError> {
    let mut matrices = Vec::new();
    for node in 0..clip.nodes.len() {
        matrices.push(local_matrix(&sample(clip, node, key, scale)?));
    }
    compose_chain(chain, parts, &mut matrices, root, -1)?;
    Ok(matrices)
}
