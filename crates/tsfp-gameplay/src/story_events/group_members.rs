//! T1846 batch 8: the member handlers `0x33BFA0` hands off (`PathHooks::deep_member`) and the sweep query.
//!
//! Original addresses (names INFERRED, behaviour MEASURED against the original under the Unicorn oracle unless a
//! function is listed under "geometry callbacks"):
//!
//! | address | role |
//! |---|---|
//! | `0x33FB50(member, words, positions, objects, acc)` | [`member_kind_0_7`]: nearest id row of a game object, cached at `obj+0x110` |
//! | `0x33B290(member, words, positions, objects, acc)` | [`member_kind_8_9`]: registry entries matching a static table key |
//! | `0x9C0E0(pos, id, radius)` | [`nearest_row`]: nearest tagged id row to a position |
//! | `0x3384F0(key, out, flag, value)` | [`registry_matches`]: filter of the 0xD8 byte registry `[0x732E60]` |
//! | `0x2F7990(index)` | [`static_value`]: value of a static table entry (`0x524F58 + 0xC4 * index`) |
//! | `0x32A5B0(blob, tag)` | [`find_chunk`]: chunk lookup in a tagged blob |
//! | `0x88C60` -> `0x88950` | [`sweep_88c60`] / [`sweep_88950`]: nearest hit of a segment sweep over a chain of cells |
//!
//! Every handler is called by `0x33B5D0` as `handler(member, words, 0, 0, acc)` (positions and objects are null,
//! so only `words[acc..]` is observable) and returns the new accumulator. The ported handlers therefore return
//! [`Emitted`] records of which `0x33BFA0` keeps only `word`.
//!
//! Geometry callbacks ([`SweepGeometry`]; ported later in T1846 batch 8 as `sweep_cells`, which implements the trait):
//! `0xA7A00` (cell test, 367 instructions plus `0xA2CD0`,
//! `0x919D0`, `0xA3140`, `0x128C60`, ... well over 3000 instructions in total), `0xA3500` (220 instructions plus
//! `0xA3140` -> `0x477B0`) and `0x88430` (290 instructions, no calls: the step to the next cell). They are called
//! with the exact arguments of the original and their results feed the ported driver.
//!
//! Globals: `[0x74C3B0]` (pointer, flags the object when it points to a non-zero dword), `[0x7B6664]` and
//! `[0x715F2C]` (main id rows, count and base, shared with `group_paths.rs`), `[0x732E60]`/`[0x732E64]` registry and
//! count, `[0x7DE455]` (byte, value 3 disables a registry filter), `[0x74C36C]+0x128` (records of 0x8C bytes, dword
//! `+0x84` is a static table index), `[0x784014]` (tagged blob searched with tag `0xC77668`).

use crate::story_events::prelude_1e2b80::{SweepQuery, SweepResult};

/// 1.0, `-1.0`, largest f32, tiny length threshold and the radius slack (`0x475C78`, `0x4760D0`, `0x475D5C`,
/// `0x4765F4`, `0x475D1C`).
pub const ONE: f32 = 1.0;
pub const DOWN: f32 = -1.0;
pub const FLOAT_MAX: f32 = f32::MAX;
pub const MIN_LENGTH_SQUARED: f32 = f32::from_bits(0x33D6_BF95);
pub const RADIUS_SLACK: f32 = f32::from_bits(0x3C23_D70A);
/// Flag set by `0x33FB50` in the game object flags (`+0x28`) when `[0x74C3B0]` points at a non-zero dword.
pub const OBJECT_FLAG_10: u32 = 0x10;
/// Animation word range (`obj+0x94`, inclusive) for which `0x33FB50` sweeps the ground first.
pub const SWEEP_WORDS: std::ops::RangeInclusive<u16> = 0x8A5..=0x8B0;

/// Errors for inputs on which the original reads or writes through a null or wild pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemberError {
    /// Static table index outside the supplied table.
    MissingStatic { index: usize },
    /// Record index outside `[0x74C36C]+0x128`.
    MissingRecord { index: i32 },
    /// `0x32A5B0` found no chunk and `0x2F7990` reads `chunk+0xA` (null + 0xA).
    MissingChunk,
    /// Sub value index outside the supplied entry.
    MissingSubValue { index: usize },
}

// ---------------------------------------------------------------------------------------------------------------
// 0x9C0E0 nearest row
// ---------------------------------------------------------------------------------------------------------------

/// Row of the main id table `[0x715F2C]` (0x20 bytes) as read by `0x9C0E0`: `+4` flags (bit 0 = usable), `+0xA`
/// signed word tag, `+0xC` three floats position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CellRow {
    pub flags: u32,
    pub tag: i16,
    pub position: [f32; 3],
}

/// `0x9C0E0(pos, id, radius)`: index of the usable row (`flags & 1`) with `tag == id` nearest to `pos`, -1 when none.
/// Rows closer than `radius` (squared plus 0.01, only for `radius > 0`) are skipped; ties keep the first row
/// (strict `<`). Distance is `(dx*dx + dy*dy) + dz*dz` in f32, rows are visited in index order (the original unrolls
/// by four, same order). NaN distances never win. MEASURED.
pub fn nearest_row(rows: &[CellRow], position: [f32; 3], id: i32, radius: f32) -> i32 {
    let threshold = if 0.0 >= radius {
        0.0
    } else {
        radius * radius + RADIUS_SLACK
    };
    let mut best = FLOAT_MAX;
    let mut found = -1;
    for (index, row) in rows.iter().enumerate() {
        if i32::from(row.tag) != id || row.flags & 1 == 0 {
            continue;
        }
        let dy = row.position[1] - position[1];
        let dz = row.position[2] - position[2];
        let dx = row.position[0] - position[0];
        let distance = dx * dx + dy * dy + dz * dz;
        if threshold > distance {
            continue;
        }
        if best > distance {
            best = distance;
            found = index as i32;
        }
    }
    found
}

// ---------------------------------------------------------------------------------------------------------------
// 0x88950 / 0x88C60 sweep
// ---------------------------------------------------------------------------------------------------------------

/// Arguments of the per-cell test (`0xA7A00` when `use_cell_test_a`, else `0xA3500`). Both receive
/// `(cell, from, vec, flag, &point, &aux, ...)`; `0xA7A00` additionally `(extra[0], extra[1], extra[2])` and
/// `0xA3500` `(&int_out, extra[0])`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellProbe {
    pub cell: u32,
    pub from: [f32; 3],
    pub vec: [f32; 3],
    pub flag: u32,
    pub extra: [u32; 3],
    pub variant_a7a00: bool,
}

/// Result of the per-cell test: `ret` non-zero means a hit with `point` and `aux` (normal like vector) written.
/// `int_out` is only written by `0xA3500`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CellHit {
    pub ret: u32,
    pub point: [f32; 3],
    pub aux: [f32; 3],
    pub int_out: u32,
}

/// Arguments of `0x88430` (cell step): the two vector buffers are in/out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdvanceProbe {
    pub from_copy: [f32; 3],
    pub vec_copy: [f32; 3],
    pub cell: u32,
    pub previous: u32,
    pub flag: u32,
}

/// Result of `0x88430`: the next cell (`eax`) and the buffers after the call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdvanceOut {
    pub next: u32,
    pub from_copy: [f32; 3],
    pub vec_copy: [f32; 3],
}

/// The unported geometry of the sweep (see the module doc for sizes). INFERRED roles.
pub trait SweepGeometry {
    /// `0xA7A00` or `0xA3500`.
    fn cell_test(&mut self, probe: &CellProbe) -> CellHit;
    /// `0x88430`.
    fn advance(&mut self, probe: &AdvanceProbe) -> AdvanceOut;
}

/// Output buffers of `0x88950`. `Some` means the original got a non-null pointer; the payload is the buffer content
/// before the call and is updated in place.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SweepIo {
    /// `[E+0xC]`: best hit point.
    pub position: Option<[f32; 3]>,
    /// `[E+0x10]`: normalised aux vector of the best hit (three floats written).
    pub direction: Option<[f32; 3]>,
    /// `[E+0x14]`: final cell.
    pub cell: Option<u32>,
    /// `[E+0x18]`: integer output of the best `0xA3500` hit (always null from `0x88C60`).
    pub hit: Option<u32>,
    /// Stores the original makes through a null output pointer (address 0 is valid memory on the Xbox): only
    /// possible when the cell chain ends on an unchanged non-zero cell. Counted, not applied.
    pub stray_null_stores: u32,
}

/// Fixed and variable arguments of `0x88950` besides the buffers: `eax` = first cell, `edi` = `from`, `esi` = `vec`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepArgs {
    pub cell: u32,
    pub from: [f32; 3],
    pub vec: [f32; 3],
    /// `[E+4]`: non-zero selects `0xA7A00`, zero `0xA3500`.
    pub use_a7a00: bool,
    /// `[E+8]` (`ebp`): when 0 the distance limit is `|vec|^2`, else the largest f32.
    pub flag: u32,
    /// `[E+0x1C]`, `[E+0x20]`, `[E+0x24]`.
    pub extra: [u32; 3],
}

fn square_sum(v: [f32; 3]) -> f32 {
    // Image order: z*z + y*y, then + x*x.
    v[2] * v[2] + v[1] * v[1] + v[0] * v[0]
}

fn diff(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `0x88950`: walks the cell chain from `args.cell`, tests each cell, keeps the nearest hit and writes the buffers.
/// Returns the hit value (`eax`, 0 when nothing was found or the squared sweep length is below 1e-7). MEASURED
/// control flow, the three geometry callbacks are supplied.
///
/// 1. `d2 = |vec|^2`; `slot = flag == 0 ? d2 : FLT_MAX`; `d2 < 1e-7` returns 0 untouched.
/// 2. For a non-zero cell, repeat: hit = test(cell); on a hit with `distance^2(point, from)` below both the current
///    `limit` (starts at `slot`) and `slot`, remember point, aux, ret, int and lower `limit` to that distance.
///    Then `next = advance(from_copy, vec_copy, cell, previous)`; stop when `distance^2(from_copy', from) > limit`
///    or `next == cell`; else `previous = cell; cell = next` (a zero next cell is still tested).
/// 3. The cell output is always stored; when `limit < slot` the hit output, position and normalised aux
///    (x87: `1/sqrt(z^2+y^2+x^2)` in f64, each component `v*inv` rounded once to f32) are stored and the remembered
///    ret is returned, else 0.
pub fn sweep_88950(
    geometry: &mut dyn SweepGeometry,
    args: &SweepArgs,
    io: &mut SweepIo,
) -> Result<u32, MemberError> {
    let d2 = square_sum(args.vec);
    let slot = if args.flag == 0 { d2 } else { FLOAT_MAX };
    if MIN_LENGTH_SQUARED > d2 {
        return Ok(0);
    }
    let mut limit = slot;
    let mut cell = args.cell;
    let mut previous = 0u32;
    let mut best_ret = 0u32;
    let mut best_int = 0u32;
    let mut best_point = [0.0f32; 3];
    let mut best_aux = [0.0, ONE, 0.0];
    let mut from_copy = args.from;
    let mut vec_copy = args.vec;
    // `ecx` register state at the epilogue: 0 unless the loop left through the "same cell" exit.
    let mut ecx = 0u32;
    if cell != 0 {
        loop {
            let probe = CellProbe {
                cell,
                from: args.from,
                vec: args.vec,
                flag: args.flag,
                extra: args.extra,
                variant_a7a00: args.use_a7a00,
            };
            let hit = geometry.cell_test(&probe);
            if hit.ret != 0 {
                let distance = square_sum(diff(hit.point, args.from));
                if limit > distance && slot > distance {
                    best_aux = hit.aux;
                    best_point = hit.point;
                    best_ret = hit.ret;
                    best_int = if args.use_a7a00 { 0 } else { hit.int_out };
                    limit = distance;
                }
            }
            let step = geometry.advance(&AdvanceProbe {
                from_copy,
                vec_copy,
                cell,
                previous,
                flag: args.flag,
            });
            from_copy = step.from_copy;
            vec_copy = step.vec_copy;
            if square_sum(diff(from_copy, args.from)) > limit {
                break;
            }
            let old = cell;
            previous = old;
            cell = step.next;
            if step.next == old {
                ecx = old;
                break;
            }
        }
    }
    // Stores compare the output pointer with `ecx`. With `ecx != 0` a null pointer is stored through: the Xbox has
    // valid memory at address 0, the model only counts such stray stores.
    let mut store = |present: bool| -> bool {
        if ecx != 0 && !present {
            io.stray_null_stores += 1;
        }
        present
    };
    if store(io.cell.is_some()) {
        io.cell = Some(cell);
    }
    if slot > limit {
        if store(io.hit.is_some()) {
            io.hit = Some(best_int);
        }
        if store(io.position.is_some()) {
            io.position = Some(best_point);
        }
        if store(io.direction.is_some()) {
            let sum = f64::from(best_aux[2]) * f64::from(best_aux[2])
                + f64::from(best_aux[1]) * f64::from(best_aux[1]);
            let sum = sum + f64::from(best_aux[0]) * f64::from(best_aux[0]);
            let inverse = 1.0 / sum.sqrt();
            io.direction = Some([
                (f64::from(best_aux[0]) * inverse) as f32,
                (f64::from(best_aux[1]) * inverse) as f32,
                (f64::from(best_aux[2]) * inverse) as f32,
            ]);
        }
        return Ok(best_ret);
    }
    Ok(0)
}

/// `0x88C60(world, from, vec, flag, position, direction, cell, extra0, extra1, extra2)`: forwards to `0x88950` with
/// `eax = world`, `use_a7a00 = 1`, `hit` pointer null. MEASURED.
pub fn sweep_88c60(
    geometry: &mut dyn SweepGeometry,
    world: u32,
    from: [f32; 3],
    vec: [f32; 3],
    flag: u32,
    extra: [u32; 3],
    io: &mut SweepIo,
) -> Result<u32, MemberError> {
    io.hit = None;
    sweep_88950(
        geometry,
        &SweepArgs {
            cell: world,
            from,
            vec,
            use_a7a00: true,
            flag,
            extra,
        },
        io,
    )
}

/// Plug for `PreludeEnv::sweep` (level 4 prelude `0x1E2B80`): runs `0x88C60(world, &from, &up, 1, &reach, &out, 0,
/// -3, 0, 0)`. `hit_before` is the content of the 16 byte `out` buffer before the call (uninitialised stack in the
/// original; the oracle reads zeros). Three floats of it may be overwritten. A [`MemberError`] cannot occur because
/// the cell pointer is null and `ecx` stays 0 only when the chain ends by distance; if it does occur the buffers are
/// returned unchanged. MEASURED (sweep driver), geometry supplied.
pub fn prelude_sweep(
    geometry: &mut dyn SweepGeometry,
    query: &SweepQuery,
    hit_before: [f32; 4],
) -> SweepResult {
    let mut io = SweepIo {
        position: Some(query.reach),
        direction: Some([hit_before[0], hit_before[1], hit_before[2]]),
        cell: None,
        hit: None,
        ..Default::default()
    };
    let outcome = sweep_88c60(
        geometry,
        query.world,
        query.from,
        query.up,
        1,
        [-3i32 as u32, 0, 0],
        &mut io,
    );
    let mut result = SweepResult {
        reach: query.reach,
        hit: hit_before,
    };
    if outcome.is_ok() {
        if let Some(position) = io.position {
            result.reach = position;
        }
        if let Some(direction) = io.direction {
            result.hit = [direction[0], direction[1], direction[2], hit_before[3]];
        }
    }
    result
}

// ---------------------------------------------------------------------------------------------------------------
// 0x33FB50 member kinds 0 and 7
// ---------------------------------------------------------------------------------------------------------------

/// One value `0x33B5D0` could collect: `0x33BFA0` only uses `word`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitted {
    /// Object pointer stored in the `objects` array (here the object id).
    pub object: u32,
    pub position: [f32; 3],
    pub word: i32,
}

/// Fields of the game object (`[member+0]`) used by `0x33FB50`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellObject {
    /// `+0x28` flags (the same field as `Member::game_flags_28`).
    pub flags_28: u32,
    /// `+0x34` world / first cell.
    pub world: u32,
    /// `+0x64` position.
    pub position: [f32; 3],
    /// `+0x94` animation word.
    pub animation: u16,
    /// `+0x110` cached nearest row, negative = not computed yet.
    pub nearest_cache: i32,
    /// `+0x1A8` position offset.
    pub offset: [f32; 3],
}

/// Environment of `0x33FB50`: the sweep (normally [`GeometrySweeper`]), the `[0x74C3B0]` condition and the id rows.
pub trait CellEnv {
    /// `[0x74C3B0]` is non-null and points at a non-zero dword.
    fn flag_74c3b0(&self) -> bool;
    /// `0x88C60(world, from, up, 1, &reach, &dir, &slot, 0, 0, 0)` where `slot` is the argument slot of the member
    /// pointer (initial value `member_slot`, overwritten with the final cell). Returns the buffers after the call
    /// starting from `io`.
    fn sweep(
        &mut self,
        world: u32,
        from: [f32; 3],
        up: [f32; 3],
        member_slot: u32,
        io: &mut SweepIo,
    ) -> Result<(), MemberError>;
    /// The main id rows `[0x715F2C]`.
    fn rows(&self) -> &[CellRow];
}

/// `0x33FB50` for kinds 0 and 7. `member_slot` is the pointer value of the member (the `E+4` argument slot the sweep
/// overwrites with the final cell; it is read back as the row tag when the sweep returned early). `stack_garbage` is
/// the content of the uninitialised `reach` buffer (zeros under the oracle). Steps (MEASURED):
/// 1. `flag_74c3b0` -> `flags_28 |= 0x10`.
/// 2. Query point: for animation words `0x8A5..=0x8B0` sweep from `position` along `(0, -1, 0)` and use
///    `(reach.x, reach.y + 1.0, reach.z)` and tag = final cell slot; else `position` and tag = `world`.
/// 3. `nearest_cache < 0` -> `nearest_cache = nearest_row(rows, point, tag, 0.0)`.
/// 4. Emit `word = nearest_cache`, `position = offset + position` (per component, offset first).
pub fn member_kind_0_7(
    env: &mut dyn CellEnv,
    object: &mut CellObject,
    object_id: u32,
    member_slot: u32,
    stack_garbage: [f32; 3],
) -> Result<Emitted, MemberError> {
    if env.flag_74c3b0() {
        object.flags_28 |= OBJECT_FLAG_10;
    }
    let (point, tag) = if SWEEP_WORDS.contains(&object.animation) {
        let mut io = SweepIo {
            position: Some(stack_garbage),
            direction: Some([0.0; 3]),
            cell: Some(member_slot),
            hit: None,
            ..Default::default()
        };
        env.sweep(
            object.world,
            object.position,
            [0.0, DOWN, 0.0],
            member_slot,
            &mut io,
        )?;
        let reach = io.position.unwrap_or(stack_garbage);
        (
            [reach[0], reach[1] + ONE, reach[2]],
            io.cell.unwrap_or(member_slot) as i32,
        )
    } else {
        (object.position, object.world as i32)
    };
    if object.nearest_cache < 0 {
        object.nearest_cache = nearest_row(env.rows(), point, tag, 0.0);
    }
    Ok(Emitted {
        object: object_id,
        position: [
            object.offset[0] + object.position[0],
            object.offset[1] + object.position[1],
            object.offset[2] + object.position[2],
        ],
        word: object.nearest_cache,
    })
}

/// [`CellEnv`] backed by the ported driver and an externally supplied [`SweepGeometry`].
pub struct GeometrySweeper<'a> {
    pub geometry: &'a mut dyn SweepGeometry,
    pub rows: Vec<CellRow>,
    pub flag: bool,
}

impl CellEnv for GeometrySweeper<'_> {
    fn flag_74c3b0(&self) -> bool {
        self.flag
    }
    fn sweep(
        &mut self,
        world: u32,
        from: [f32; 3],
        up: [f32; 3],
        _slot: u32,
        io: &mut SweepIo,
    ) -> Result<(), MemberError> {
        sweep_88c60(self.geometry, world, from, up, 1, [0; 3], io).map(|_| ())
    }
    fn rows(&self) -> &[CellRow] {
        &self.rows
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 0x33B290 member kinds 8 and 9
// ---------------------------------------------------------------------------------------------------------------

/// Static table entry `0x524F58 + 0xC4 * index`: `+8` key, `+0x10` value, `+0xB8` flags, and for flag `4` the values
/// `+0x10 + 0x1C * w`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StaticEntry {
    pub key: u32,
    pub flags_b8: u32,
    /// Dword at `+0x10 + 0x1C * w`; `values[0]` is the plain value.
    pub values: Vec<u32>,
}

/// Header magic, version and terminator tag of the tagged blob `0x32A5B0` searches, and the scan limit.
pub const BLOB_MAGIC: u32 = 0x00C7_7667;
pub const BLOB_VERSION: u32 = 0x12D;
pub const BLOB_END_TAG: u32 = 0x00C7_766A;
pub const BLOB_LIMIT: u32 = 0x7FE0;
/// Tag looked up by `0x2F7990`.
pub const STATIC_TAG: u32 = 0x00C7_7668;

fn blob_dword(blob: &[u8], offset: u32) -> u32 {
    let at = offset as usize;
    let mut bytes = [0u8; 4];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = blob.get(at + i).copied().unwrap_or(0);
    }
    u32::from_le_bytes(bytes)
}

/// `0x32A5B0(blob, tag)`: payload offset of the first chunk with `tag` or `None`. Chunks start at offset 8 as
/// `(tag, size)` pairs (8 byte header, `size` payload bytes), the walk stops at tag `0xC7766A` or offset `0x7FE0`.
/// Bytes outside `blob` read as zero. MEASURED.
pub fn find_chunk(blob: &[u8], tag: u32) -> Option<u32> {
    if blob_dword(blob, 0) != BLOB_MAGIC || blob_dword(blob, 4) != BLOB_VERSION {
        return None;
    }
    let mut cursor = 8u32;
    while cursor < BLOB_LIMIT {
        let chunk = cursor;
        let found = blob_dword(blob, chunk);
        cursor = cursor.wrapping_add(8);
        if found == tag {
            return Some(cursor);
        }
        if found == BLOB_END_TAG {
            return None;
        }
        cursor = cursor.wrapping_add(blob_dword(blob, chunk + 4));
    }
    None
}

/// `0x2F7990(index)`: for entries with flags bit `4` the value selected by the word at payload `+0xA` of the blob
/// chunk `0xC77668`, else the plain value. MEASURED.
pub fn static_value(
    statics: &[StaticEntry],
    blob: &[u8],
    index: usize,
) -> Result<u32, MemberError> {
    let entry = statics
        .get(index)
        .ok_or(MemberError::MissingStatic { index })?;
    let selector = if entry.flags_b8 & 4 != 0 {
        let payload = find_chunk(blob, STATIC_TAG).ok_or(MemberError::MissingChunk)?;
        let low = blob.get((payload + 0xA) as usize).copied().unwrap_or(0);
        let high = blob.get((payload + 0xB) as usize).copied().unwrap_or(0);
        usize::from(u16::from_le_bytes([low, high]))
    } else {
        0
    };
    entry
        .values
        .get(selector)
        .copied()
        .ok_or(MemberError::MissingSubValue { index: selector })
}

/// Registry entry (0xD8 bytes at `[0x732E60]`): `+0` game object, `+0xC`, `+0x14` match keys, `+0x18` pointer to a
/// record whose byte 1 is `record_flags`, `+0x38` word. The game object fields are `flags_28`, `position`, `offset`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RegistryEntry {
    pub object: u32,
    pub flags_28: u32,
    pub position: [f32; 3],
    pub offset: [f32; 3],
    pub match_c: u32,
    pub match_14: u32,
    pub record_flags: u8,
    pub word_38: u32,
}

/// `0x3384F0(key, out, flag, value)`: indices of the registry entries with `match_14 == key && match_c == value`
/// that pass the two filters: (`record_flags & 1` or the byte `[0x7DE455]` is not 3) and (`flags_28 & 2` or `flag`
/// non-zero). The original returns the count (the vector length) and stores entry pointers when `out` is non-null.
/// MEASURED.
pub fn registry_matches(
    registry: &[RegistryEntry],
    mode_is_3: bool,
    key: u32,
    flag: bool,
    value: u32,
) -> Vec<usize> {
    registry
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            (entry.record_flags & 1 != 0 || !mode_is_3)
                && (entry.flags_28 & 2 != 0 || flag)
                && entry.match_14 == key
                && entry.match_c == value
        })
        .map(|(index, _)| index)
        .collect()
}

/// Data of `0x33B290`.
#[derive(Clone, Debug, Default)]
pub struct StaticData {
    /// Dword `+0x84` of the 0x8C byte records at `[[0x74C36C]+0x128]`: index into [`StaticData::statics`].
    pub record_indices: Vec<u32>,
    pub statics: Vec<StaticEntry>,
    /// Blob `[0x784014]`.
    pub blob: Vec<u8>,
    pub registry: Vec<RegistryEntry>,
    /// `[0x7DE455] == 3`.
    pub mode_is_3: bool,
}

/// Cap of the collected entries (the scratch buffer holds `min(count, 10)` pointers).
pub const MAX_MATCHES: usize = 10;

/// `0x33B290` for kinds 8 and 9: `record = descriptor_word` (member descriptor `+0xC`, signed word) selects a record,
/// its static index gives the key (`+8`) and `0x2F7990` the value; every matching registry entry (at most 10) emits
/// `(object, offset + position, word_38)`. MEASURED.
pub fn member_kind_8_9(
    data: &StaticData,
    descriptor_word: i16,
) -> Result<Vec<Emitted>, MemberError> {
    let record = i32::from(descriptor_word);
    let index = usize::try_from(record)
        .ok()
        .and_then(|r| data.record_indices.get(r))
        .copied()
        .ok_or(MemberError::MissingRecord { index: record })? as usize;
    let key = data
        .statics
        .get(index)
        .ok_or(MemberError::MissingStatic { index })?
        .key;
    let value = static_value(&data.statics, &data.blob, index)?;
    let matches = registry_matches(&data.registry, data.mode_is_3, key, false, value);
    Ok(matches
        .into_iter()
        .take(MAX_MATCHES)
        .map(|i| {
            let entry = &data.registry[i];
            Emitted {
                object: entry.object,
                position: [
                    entry.offset[0] + entry.position[0],
                    entry.offset[1] + entry.position[1],
                    entry.offset[2] + entry.position[2],
                ],
                word: entry.word_38 as i32,
            }
        })
        .collect())
}

// ---------------------------------------------------------------------------------------------------------------
// Plug for `PathHooks::deep_member`
// ---------------------------------------------------------------------------------------------------------------

/// Everything `0x33B5D0` needs to run the member kinds `group_paths::PathHooks::deep_member` delegates.
///
/// Wiring: keep one `DeepMembers` next to the path hooks and implement
/// `deep_member(&mut self, member, out)` as
/// `self.members.dispatch(member.kind, member.object_id, &mut member.game_flags_28, out)`.
pub struct DeepMembers<'a> {
    /// Sweep, id rows and the `[0x74C3B0]` condition for kinds 0 and 7.
    pub env: &'a mut dyn CellEnv,
    /// Game objects of kind 0 and 7 members by member object id.
    pub objects: std::collections::BTreeMap<i32, CellObject>,
    /// Member descriptor word (`+0xC`) of kind 8 and 9 members by member object id.
    pub descriptor_words: std::collections::BTreeMap<i32, i16>,
    pub data: StaticData,
    /// Pointer value of the member slot the sweep overwrites (only matters when the sweep returns early).
    pub member_slot: u32,
    /// Content of the uninitialised `reach` stack buffer of `0x33FB50`.
    pub stack_garbage: [f32; 3],
}

impl DeepMembers<'_> {
    /// Kinds 0, 7 -> [`member_kind_0_7`], 8, 9 -> [`member_kind_8_9`]; pushes the words `0x33B5D0` would store at
    /// `out[acc..]`. Other kinds push nothing. A member without a registered object or descriptor is an error.
    pub fn dispatch(
        &mut self,
        kind: i32,
        object_id: i32,
        flags_28: &mut u32,
        out: &mut Vec<i32>,
    ) -> Result<(), MemberError> {
        match kind {
            0 | 7 => {
                let object = self
                    .objects
                    .get_mut(&object_id)
                    .ok_or(MemberError::MissingRecord { index: object_id })?;
                object.flags_28 = *flags_28;
                let emitted = member_kind_0_7(
                    &mut *self.env,
                    object,
                    object_id as u32,
                    self.member_slot,
                    self.stack_garbage,
                )?;
                *flags_28 = object.flags_28;
                out.push(emitted.word);
            }
            8 | 9 => {
                let word = *self
                    .descriptor_words
                    .get(&object_id)
                    .ok_or(MemberError::MissingRecord { index: object_id })?;
                out.extend(member_kind_8_9(&self.data, word)?.iter().map(|e| e.word));
            }
            _ => {}
        }
        Ok(())
    }
}
