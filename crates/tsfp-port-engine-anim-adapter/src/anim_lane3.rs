// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/c2r/objchar_wired3
//! T1918 wave B: guest adapters (shadow mode) for the animation ports that need pose math: `posemix` (0x60540 0x605D0 0x604D0), `instance`
//! (0x60B30), `prop` (0x5F260 0x5F040), `bounds` (0x60F80), `aim` (0x5AFB0), `heading` (0x58690 0x5A220). The sources are the very files of
//! `tsfp-engine-anim` compiled into this `no_std` crate (libm through `fmath.rs`). Samples come from the lifted sampler 0x59320 through
//! `GuestCall`. Not adapted: 0x5B720 (the ground probe 0xC5A50 is not known to be free of side effects, it would run twice in shadow mode) and the
//! allocator users 0x96A40 / 0x965F0.
use crate::anim_lane::{run_original, shadow_regions_diag};
use crate::helpers::{
    arg, call, clip_key_count, clip_pointer, float, policy, read_layer, slot_of, write_float,
};
use crate::keybracket::{bracket_weight_by, key_bracket_by};
use crate::posemath::{Mat4, Sample};
use crate::{
    abi::{ExtRegisters, GuestCall},
    anim_lane::{Check, shadow_regions},
    memory::GuestMemory,
};
use crate::{aim, bounds, heading, instance, posemix, prop};
use alloc::{vec, vec::Vec};

pub const ROOTS: [u32; 9] = [
    0x60540, 0x605d0, 0x604d0, 0x60b30, 0x5f260, 0x5f040, 0x60f80, 0x5afb0, 0x58690,
];
/// Appended separately so the list in `lane_inv.rs` stays in registration order.
pub const ROOTS_MORE: [u32; 1] = [0x5a220];

/// A guest pointer worth dereferencing: the first 64 KB are never mapped, a null or tiny pointer means the structure is not set up yet.
fn ok(address: u32) -> bool {
    address >= 0x1_0000
}

const CLIP_TABLE_ADDRESS: u32 = 0x007e_3954;
/// T3818: node counts are read from the model and clip, never clamped. A fixed 40-matrix buffer used to evaluate only the
/// first 40 nodes of every skeleton: cutscene and character models have 44 to 267 nodes (MEASURED in the Tier 2 corpus),
/// so the rest kept stale matrices and skinned meshes rendered broken. Only a count above this sanity bound (a corrupt or
/// unset header, no game model comes close) is refused, and the retained original runs instead.
pub const NODE_LIMIT: usize = 1024;
/// Clip key tables above this length are read only up to it (allocation bound against a corrupt header; game clips are far below).
const KEY_LIMIT: usize = 1 << 16;

fn matrix_at<M: GuestMemory>(m: &M, address: u32) -> Mat4 {
    core::array::from_fn(|i| float(m, address.wrapping_add(4 * i as u32)))
}

fn read_keys<M: GuestMemory>(m: &M, clip: u32) -> Vec<u32> {
    let count = (clip_key_count(m, clip) as usize).min(KEY_LIMIT);
    let table = m.read32(clip.wrapping_add(0x1c));
    (0..count)
        .map(|i| m.read32(table.wrapping_add(4 * i as u32)))
        .collect()
}

/// A model or clip node count, or `None` above [`NODE_LIMIT`] (the caller then leaves the call to the original).
fn node_count<M: GuestMemory>(m: &M, address: u32) -> Option<usize> {
    let count = m.read32(address) as usize;
    (count <= NODE_LIMIT).then_some(count)
}

fn zero_matrices(count: usize) -> Vec<Mat4> {
    vec![[0f32; 16]; count]
}

fn write_matrix<M: GuestMemory>(m: &mut M, address: u32, matrix: &Mat4) {
    for (i, value) in matrix.iter().enumerate() {
        write_float(m, address.wrapping_add(4 * i as u32), *value);
    }
}

fn sample_bits<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    entry: &ExtRegisters,
    clip: u32,
    node: u32,
    key: i32,
    scale: u32,
) -> Sample {
    let out = entry.esp.wrapping_sub(0x40);
    call(
        m,
        guest,
        entry,
        0x59320,
        &[clip, node, key as u32, out, scale],
    );
    Sample {
        bits: core::array::from_fn(|i| m.read32(out.wrapping_add(4 * i as u32))),
    }
}

fn call_step<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    entry: &ExtRegisters,
    state: u32,
) {
    // the lifted time step (0x5ADC0(state, 1.0, 0)), exactly as 0x5F040 / 0x5F260 call it
    call(m, guest, entry, 0x5adc0, &[state, 1.0f32.to_bits(), 0]);
}

pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) && !ROOTS_MORE.contains(&va) {
        return false;
    }
    crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        run_inner(va, view, guest, regs)
    })
}

fn run_inner<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    let entry = *regs;
    let (a0, a1, a2) = (arg(m, &entry, 0), arg(m, &entry, 1), arg(m, &entry, 2));
    match va {
        // 0x60540 (context in ESI, stack flag): blend the pose matrices by weight
        0x60540 | 0x605d0 => {
            let ctx = entry.esi;
            let model = if ok(ctx) { m.read32(ctx) } else { 0 };
            let count = if ok(model) {
                let Some(count) = node_count(m, model.wrapping_add(0x20)) else {
                    return run_original(va, m, guest, regs);
                };
                count
            } else {
                0
            };
            let inner = if ok(model) {
                m.read32(model.wrapping_add(0x10))
            } else {
                0
            };
            let entries_at = if ok(inner) {
                m.read32(inner.wrapping_add(0xc))
            } else {
                0
            };
            let (base_at, pose_at, out_at) = if ok(model) && ok(ctx) {
                (
                    m.read32(model.wrapping_add(0x2c)),
                    m.read32(ctx.wrapping_add(4)),
                    m.read32(ctx.wrapping_add(8)),
                )
            } else {
                (0, 0, 0)
            };
            let live =
                ok(model) && ok(entries_at) && ok(base_at) && ok(pose_at) && ok(out_at) && ok(ctx);
            if !live {
                return run_original(va, m, guest, regs);
            }
            let regions = [(out_at, if live { count * 16 } else { 0 })];
            let describe = |m: &M, word: usize| {
                let (index, cell) = (word / 16, word % 16);
                let at = entries_at.wrapping_add(8 * index as u32);
                let (part, weight) = (m.read32(at), m.read32(at.wrapping_add(4)));
                let q_base = crate::blend::matrix_to_quaternion(&matrix_at(
                    m,
                    base_at.wrapping_add(64 * part),
                ));
                let q_pose = crate::blend::matrix_to_quaternion(&matrix_at(
                    m,
                    pose_at.wrapping_add(64 * part),
                ));
                let mut out = [0u32; 14];
                out[0] = cell as u32;
                out[1] = part;
                out[2] = weight;
                for i in 0..4 {
                    out[3 + i] = q_base[i].to_bits();
                    out[7 + i] = q_pose[i].to_bits();
                }
                out
            };
            shadow_regions_diag(
                va,
                m,
                guest,
                regs,
                &regions,
                Check::None,
                |m, guest, _regs| {
                    let entries: Vec<(u32, f32)> = (0..count)
                        .map(|i| {
                            let at = entries_at.wrapping_add(8 * i as u32);
                            (m.read32(at), float(m, at.wrapping_add(4)))
                        })
                        .collect();
                    let parts = entries
                        .iter()
                        .map(|entry| (entry.0 as usize).saturating_add(1))
                        .max()
                        .unwrap_or(1)
                        .max(1);
                    if parts > NODE_LIMIT {
                        // a part index past any model: the guest table is not set up (leave the matrices as they are)
                        return;
                    }
                    if va == 0x60540 {
                        // The Rust matrix helper owns each destination after this call.
                        // T3851: never overwrite it with a pre-call output snapshot.
                        for (i, &(part, weight)) in entries.iter().enumerate() {
                            if weight == 1.0 {
                                continue;
                            }
                            let blend_weight = if a0 != 0 { 1.0 } else { weight };
                            call(
                                m,
                                guest,
                                &entry,
                                0x1526c0,
                                &[
                                    base_at.wrapping_add(64 * part),
                                    pose_at.wrapping_add(64 * part),
                                    out_at.wrapping_add(64 * i as u32),
                                    blend_weight.to_bits(),
                                ],
                            );
                        }
                    } else {
                        let mut pose = zero_matrices(parts);
                        let mut out = zero_matrices(count);
                        for (i, matrix) in pose.iter_mut().enumerate() {
                            *matrix = matrix_at(m, pose_at.wrapping_add(64 * i as u32));
                        }
                        for (i, matrix) in out.iter_mut().enumerate() {
                            *matrix = matrix_at(m, out_at.wrapping_add(64 * i as u32));
                        }
                        let mut parents = vec![-1i32; parts];
                        let table = m.read32(model.wrapping_add(0x14));
                        for (i, parent) in parents.iter_mut().enumerate().take(parts) {
                            *parent = m.read32(table.wrapping_add(20 * i as u32 + 4)) as i32;
                        }
                        posemix::compose_or_copy(
                            &entries[..count],
                            &parents[..parts],
                            &pose[..parts],
                            &mut out[..count],
                        );
                        for (i, matrix) in out.iter().enumerate() {
                            write_matrix(m, out_at.wrapping_add(64 * i as u32), matrix);
                        }
                    }
                },
                describe,
            );
        }
        // 0x604D0(context, matrices, node): part tree world matrices
        0x604d0 => {
            let model = if ok(a0) { m.read32(a0) } else { 0 };
            let count = if ok(model) {
                let Some(count) = node_count(m, model.wrapping_add(0x20)) else {
                    return run_original(va, m, guest, regs);
                };
                count
            } else {
                0
            };
            let table_at = if ok(model) {
                m.read32(model.wrapping_add(0x14))
            } else {
                0
            };
            let live = ok(a1) && ok(table_at);
            if !live {
                return run_original(va, m, guest, regs);
            }
            let regions = [(a1, if live { count * 16 } else { 0 })];
            shadow_regions(
                va,
                m,
                guest,
                regs,
                &regions,
                Check::None,
                |m, _guest, _regs| {
                    let table = table_at;
                    let mut rows = vec![
                        posemix::TreeRow {
                            parent: -1,
                            child: -1,
                            sibling: -1,
                        };
                        count
                    ];
                    let mut mats = zero_matrices(count);
                    for i in 0..count {
                        let at = table.wrapping_add(20 * i as u32);
                        rows[i] = posemix::TreeRow {
                            parent: m.read32(at + 4) as i32,
                            child: m.read32(at + 8) as i32,
                            sibling: m.read32(at + 12) as i32,
                        };
                        mats[i] = matrix_at(m, a1.wrapping_add(64 * i as u32));
                    }
                    if count == 0 || a2 as usize >= count {
                        return;
                    }
                    posemix::compose_tree(&rows[..count], &mut mats[..count], a2 as i32);
                    for i in 0..count {
                        write_matrix(m, a1.wrapping_add(64 * i as u32), &mats[i]);
                    }
                },
            );
        }
        // 0x60B30(owner, index, instance, save_base): evaluate all nodes of an instance
        0x60b30 => {
            let (owner, index, inst, save) = (a0, a1, a2, arg(m, &entry, 3));
            let model = if ok(inst) { m.read32(inst) } else { 0 };
            let mats_at = if ok(inst) {
                m.read32(inst.wrapping_add(4))
            } else {
                0
            };
            let slot = if ok(inst) {
                m.read32(inst.wrapping_add(0x14))
            } else {
                0
            };
            let table = m.read32(CLIP_TABLE_ADDRESS);
            let clip = if ok(table) && slot < 0x1000 {
                clip_pointer(m, slot)
            } else {
                0
            };
            let (nodes, saved) = match (
                if ok(clip) {
                    node_count(m, clip.wrapping_add(0x14))
                } else {
                    Some(0)
                },
                if ok(model) {
                    node_count(m, model.wrapping_add(0x1c))
                } else {
                    Some(0)
                },
            ) {
                (Some(nodes), Some(saved)) => (nodes, saved),
                _ => return run_original(va, m, guest, regs),
            };
            let base_at = if ok(model) {
                m.read32(model.wrapping_add(0x2c))
            } else {
                0
            };
            let out_entries = if ok(model) {
                let Some(count) = node_count(m, model.wrapping_add(0x20)) else {
                    return run_original(va, m, guest, regs);
                };
                count
            } else {
                0
            };
            let out_at = if ok(inst) {
                m.read32(inst.wrapping_add(8))
            } else {
                0
            };
            // T3818: the original reads the owner (`owner+8`, the track offset) only to advance the layer time, and only when the clip
            // duration is above zero. Cutscene actors (caller 0x98120) pass owner 0 with a zero-duration clip and keep their own layer
            // time; requiring an owner made the port skip them (no pose at all: invisible or broken cutscene meshes).
            let advances = ok(clip) && float(m, clip.wrapping_add(0x18)) > 0.0;
            let live = ok(inst)
                && ok(model)
                && ok(mats_at)
                && ok(clip)
                && (ok(owner) || !advances)
                && (saved == 0 || ok(base_at))
                && (out_entries == 0 || ok(out_at));
            if !live {
                // never a silent no-op: what the port cannot evaluate, the retained original does (T3818)
                return run_original(va, m, guest, regs);
            }
            let regions = if live {
                [
                    (inst.wrapping_add(0x18), 1),
                    (inst.wrapping_add(0x20), 9),
                    (mats_at, nodes.max(saved) * 16),
                    (base_at, saved * 16),
                    (out_at, out_entries * 16),
                ]
            } else {
                [(0, 0); 5]
            };
            let track_offset_at = owner
                .wrapping_add(index.wrapping_mul(0x2c))
                .wrapping_add(0x34);
            let diag_before = [
                owner,
                index,
                inst,
                save,
                slot,
                clip,
                if ok(owner.wrapping_add(8)) {
                    m.read32(owner.wrapping_add(8))
                } else {
                    0
                },
                if ok(track_offset_at) {
                    m.read32(track_offset_at)
                } else {
                    0
                },
                if ok(clip.wrapping_add(0x18)) {
                    m.read32(clip.wrapping_add(0x18))
                } else {
                    0
                },
                if ok(inst.wrapping_add(0x18)) {
                    m.read32(inst.wrapping_add(0x18))
                } else {
                    0
                },
                nodes as u32,
                if ok(model) {
                    m.read32(model.wrapping_add(0x1c))
                } else {
                    0
                },
                if ok(model) {
                    m.read32(model.wrapping_add(0x20))
                } else {
                    0
                },
                mats_at,
            ];
            let diag = core::cell::Cell::new(diag_before);
            shadow_regions_diag(
                va,
                m,
                guest,
                regs,
                &regions,
                Check::None,
                |m, guest, regs| {
                    let entry = *regs;
                    let snap = call(m, guest, &entry, 0x60920, &[owner]).eax != 0;
                    let raw_key_count = clip_key_count(m, clip);
                    let key_table = m.read32(clip.wrapping_add(0x1c));
                    let duration = float(m, clip.wrapping_add(0x18));
                    let (p0, p1) = policy(m, slot);
                    // read only when the time advances (duration > 0), like the original
                    let (owner_time, offset) = if advances {
                        (
                            float(m, owner.wrapping_add(8)),
                            float(
                                m,
                                owner
                                    .wrapping_add(index.wrapping_mul(0x2c))
                                    .wrapping_add(0x34),
                            ),
                        )
                    } else {
                        (0.0, 0.0)
                    };
                    // at least 10 rows: the shadow diagnostic below reads node 9
                    let mut matrices = zero_matrices(nodes.max(saved).max(10));
                    let mut base = zero_matrices(saved.max(1));
                    for i in 0..nodes.max(saved) {
                        matrices[i] = matrix_at(m, mats_at.wrapping_add(64 * i as u32));
                    }
                    for i in 0..saved {
                        base[i] = matrix_at(m, base_at.wrapping_add(64 * i as u32));
                    }
                    let root: [f32; 9] =
                        core::array::from_fn(|i| float(m, inst.wrapping_add(0x20 + 4 * i as u32)));
                    let mut state = instance::Instance {
                        layer_time: float(m, inst.wrapping_add(0x18)),
                        root,
                        matrices: &mut matrices,
                        base: &mut base,
                        saved_count: saved,
                    };
                    let layer_time = crate::advance::advance_time(
                        duration,
                        owner_time,
                        offset,
                        state.layer_time,
                    );
                    let bracket = layer_time.and_then(|time| {
                        let key_bytes = raw_key_count.checked_mul(4)?;
                        if !m.is_readable(key_table, key_bytes) {
                            return None;
                        }
                        let key_at = |index: usize| {
                            Some(m.read32(key_table.wrapping_add((index as u32).wrapping_mul(4))))
                        };
                        let (low, high) =
                            key_bracket_by(raw_key_count as usize, duration, time, key_at)?;
                        let weight = bracket_weight_by(low, high, time, |index| {
                            Some(m.read32(key_table.wrapping_add((index as u32).wrapping_mul(4))))
                        })?;
                        Some((low, high, weight))
                    });
                    let (debug_low, debug_high, bracket_weight) = bracket.unwrap_or((-1, -1, 0.0));
                    let debug_weight = if snap {
                        if 0.5 > bracket_weight { 0.0 } else { 1.0 }
                    } else {
                        bracket_weight
                    };
                    // Measurement-only: the shadow diagnostic emits these sampler
                    // inputs only when the outer memory comparison finds a mismatch.
                    let mut debug_node9_quaternion = [0u32; 4];
                    let steps = instance::evaluate_instance_with_bracket(
                        nodes,
                        layer_time,
                        bracket,
                        (p0, p1),
                        snap,
                        save != 0,
                        |node, key| {
                            let sample = sample_bits(
                                m,
                                guest,
                                &entry,
                                clip,
                                node as u32,
                                key,
                                1.0f32.to_bits(),
                            );
                            if node == 9 {
                                if key == debug_low {
                                    debug_node9_quaternion.copy_from_slice(&sample.bits[3..7]);
                                }
                            }
                            sample
                        },
                        &mut state,
                    );
                    // 0x60B30 publishes the advanced state time even when the key
                    // bracket is empty. `evaluate_instance` updates this typed field
                    // before returning `None`, so write it back before the early exit.
                    write_float(m, inst.wrapping_add(0x18), state.layer_time);
                    let Some(steps) = steps else {
                        diag.set([
                            owner,
                            index,
                            inst,
                            raw_key_count,
                            slot,
                            clip,
                            key_table,
                            0,
                            0,
                            duration.to_bits(),
                            owner_time.to_bits(),
                            offset.to_bits(),
                            state.layer_time.to_bits(),
                            0,
                        ]);
                        return;
                    };
                    let debug_node9_local_row =
                        core::array::from_fn::<_, 4, _>(|i| state.matrices[9][i].to_bits());
                    for (i, value) in state.root.iter().enumerate() {
                        write_float(m, inst.wrapping_add(0x20 + 4 * i as u32), *value);
                    }
                    for i in 0..nodes {
                        write_matrix(m, mats_at.wrapping_add(64 * i as u32), &state.matrices[i]);
                    }
                    if save != 0 {
                        for i in 0..saved {
                            write_matrix(m, base_at.wrapping_add(64 * i as u32), &state.base[i]);
                        }
                    }
                    for step in steps {
                        match step {
                            instance::PostStep::BlendByWeight { full } => {
                                let mut cr = entry;
                                cr.esp = entry.esp.wrapping_sub(0x20);
                                cr.esi = inst;
                                m.write32(cr.esp, u32::from(full));
                                guest.call(0x60540, &mut cr);
                            }
                            instance::PostStep::ComposeTree => {
                                call(m, guest, &entry, 0x604d0, &[inst, mats_at, 0]);
                            }
                            instance::PostStep::ComposeOrCopy => {
                                let mut cr = entry;
                                cr.esp = entry.esp.wrapping_sub(0x20);
                                cr.esi = inst;
                                guest.call(0x605d0, &mut cr);
                            }
                        }
                    }
                    diag.set([
                        owner,
                        index,
                        debug_low as u32,
                        debug_high as u32,
                        debug_weight.to_bits(),
                        debug_node9_quaternion[0],
                        debug_node9_quaternion[1],
                        debug_node9_quaternion[2],
                        debug_node9_quaternion[3],
                        debug_node9_local_row[0],
                        debug_node9_local_row[1],
                        debug_node9_local_row[2],
                        debug_node9_local_row[3],
                        raw_key_count,
                    ]);
                },
                |_, _| diag.get(),
            );
        }
        // 0x5F260(state, matrix): bare layer state evaluator
        0x5f260 => {
            let live = ok(a0)
                && ok(a1)
                && m.read32(a0.wrapping_add(4)) < 0x1000
                && ok(clip_pointer(m, m.read32(a0.wrapping_add(4))));
            if !live {
                return run_original(va, m, guest, regs);
            }
            let regions = [
                (a0, if live { 22 } else { 0 }),
                (a1, if live { 16 } else { 0 }),
            ];
            shadow_regions(
                va,
                m,
                guest,
                regs,
                &regions,
                Check::None,
                |m, guest, regs| {
                    let entry = *regs;
                    call_step(m, guest, &entry, a0);
                    let clip = clip_pointer(m, m.read32(a0.wrapping_add(4)));
                    let keys = read_keys(m, clip);
                    let (duration, time) = (
                        float(m, clip.wrapping_add(0x18)),
                        float(m, a0.wrapping_add(8)),
                    );
                    let scale = 1.0f32.to_bits();
                    if let Some(matrix) = prop::state_matrix(&keys, duration, time, |key| {
                        sample_bits(m, guest, &entry, clip, 0, key, scale)
                    }) {
                        write_matrix(m, a1, &matrix);
                    }
                },
            );
        }
        // 0x5F040(body): single node prop evaluator
        0x5f040 => {
            let model = if ok(a0) { m.read32(a0) } else { 0 };
            let info = if ok(a0) {
                m.read32(a0.wrapping_add(4))
            } else {
                0
            };
            let count = if ok(info) {
                let Some(count) = node_count(m, info.wrapping_add(4)) else {
                    return run_original(va, m, guest, regs);
                };
                count
            } else {
                0
            };
            let mats_at = if ok(a0) {
                m.read32(a0.wrapping_add(0x10))
            } else {
                0
            };
            let slot = if ok(a0) {
                m.read32(a0.wrapping_add(0xd8))
            } else {
                0xffff
            };
            let live = ok(a0)
                && ok(model)
                && ok(info)
                && ok(mats_at)
                && slot < 0x1000
                && ok(clip_pointer(m, slot));
            if !live {
                return run_original(va, m, guest, regs);
            }
            let regions = if live {
                [
                    (a0.wrapping_add(0xd4), 22),
                    (mats_at, count * 16),
                    (model.wrapping_add(0x64), 3),
                    (model.wrapping_add(0x70), 1),
                ]
            } else {
                [(0, 0); 4]
            };
            shadow_regions(
                va,
                m,
                guest,
                regs,
                &regions,
                Check::None,
                |m, guest, regs| {
                    let entry = *regs;
                    m.write32(model.wrapping_add(0x70), m.read32(model.wrapping_add(0xc4)));
                    call_step(m, guest, &entry, a0.wrapping_add(0xd4));
                    let clip = clip_pointer(m, m.read32(a0.wrapping_add(0xd8)));
                    let keys = read_keys(m, clip);
                    let (duration, time) = (
                        float(m, clip.wrapping_add(0x18)),
                        float(m, a0.wrapping_add(0xdc)),
                    );
                    let scale = 1.0f32.to_bits();
                    let mut out = zero_matrices(count);
                    if let Some(position) =
                        prop::prop_body_pose(&keys, duration, time, &mut out[..count], |key| {
                            sample_bits(m, guest, &entry, clip, 0, key, scale)
                        })
                    {
                        for (i, matrix) in out.iter().enumerate().take(count) {
                            write_matrix(m, mats_at.wrapping_add(64 * i as u32), matrix);
                        }
                        for (i, value) in position.into_iter().enumerate() {
                            write_float(m, model.wrapping_add(0x64 + 4 * i as u32), value);
                        }
                    }
                },
            );
        }
        // 0x60F80(owner, index, instance, out_min, out_max): pose bounds
        0x60f80 => {
            let (owner, index, inst, out_min, out_max) =
                (a0, a1, a2, arg(m, &entry, 3), arg(m, &entry, 4));
            let bound_clip = if ok(inst) && m.read32(inst.wrapping_add(0x14)) < 0x1000 {
                clip_pointer(m, m.read32(inst.wrapping_add(0x14)))
            } else {
                0
            };
            // T3818: as 0x60B30, the owner is read only when the clip duration is above zero (cutscene actors pass owner 0)
            let advances = ok(bound_clip) && float(m, bound_clip.wrapping_add(0x18)) > 0.0;
            let Some(nodes) = (if ok(bound_clip) {
                node_count(m, bound_clip.wrapping_add(0x14))
            } else {
                Some(0)
            }) else {
                return run_original(va, m, guest, regs);
            };
            let live = ok(inst)
                && ok(out_min)
                && ok(out_max)
                && (ok(owner) || !advances)
                && ok(m.read32(inst))
                && ok(bound_clip);
            if !live {
                return run_original(va, m, guest, regs);
            }
            let regions = if live {
                [
                    (out_min, 3),
                    (out_max, 3),
                    (inst.wrapping_add(4), 1),
                    (inst.wrapping_add(0x18), 1),
                ]
            } else {
                [(0, 0); 4]
            };
            shadow_regions(
                va,
                m,
                guest,
                regs,
                &regions,
                Check::None,
                |m, guest, regs| {
                    let entry = *regs;
                    let model = m.read32(inst);
                    let slot = m.read32(inst.wrapping_add(0x14));
                    let clip = clip_pointer(m, slot);
                    let snap = call(m, guest, &entry, 0x60920, &[owner]).eax != 0;
                    let keys = read_keys(m, clip);
                    let duration = float(m, clip.wrapping_add(0x18));
                    let (p0, p1) = policy(m, slot);
                    let (owner_time, offset) = if advances {
                        (
                            float(m, owner.wrapping_add(8)),
                            float(
                                m,
                                owner
                                    .wrapping_add(index.wrapping_mul(0x2c))
                                    .wrapping_add(0x34),
                            ),
                        )
                    } else {
                        (0.0, 0.0)
                    };
                    let mut layer_time = float(m, inst.wrapping_add(0x18));
                    let clip_view = bounds::BoundsClip {
                        keys: &keys,
                        duration,
                        nodes,
                    };
                    let table = m.read32(model.wrapping_add(0x14));
                    let mut rows = vec![
                        posemix::TreeRow {
                            parent: -1,
                            child: -1,
                            sibling: -1,
                        };
                        nodes
                    ];
                    for (i, row) in rows.iter_mut().enumerate() {
                        let at = table.wrapping_add(20 * i as u32);
                        *row = posemix::TreeRow {
                            parent: m.read32(at + 4) as i32,
                            child: m.read32(at + 8) as i32,
                            sibling: m.read32(at + 12) as i32,
                        };
                    }
                    let mut scratch = zero_matrices(nodes);
                    let result = bounds::pose_bounds(
                        &clip_view,
                        owner_time,
                        offset,
                        (p0, p1),
                        snap,
                        &mut layer_time,
                        |node, key| {
                            sample_bits(m, guest, &entry, clip, node as u32, key, 1.0f32.to_bits())
                        },
                        &mut scratch,
                        |matrices| {
                            if !matrices.is_empty() {
                                posemix::compose_tree(&rows[..matrices.len()], matrices, 0);
                            }
                        },
                    );
                    if let Some((min, max)) = result {
                        for (i, value) in min.into_iter().enumerate() {
                            write_float(m, out_min.wrapping_add(4 * i as u32), value);
                        }
                        for (i, value) in max.into_iter().enumerate() {
                            write_float(m, out_max.wrapping_add(4 * i as u32), value);
                        }
                        write_float(m, inst.wrapping_add(0x18), layer_time);
                        m.write32(inst.wrapping_add(4), 0);
                    }
                },
            );
        }
        // 0x5AFB0(context, out, node): blend space pose sampler
        0x5afb0 => {
            let object = if ok(a0) {
                m.read32(a0.wrapping_add(4))
            } else {
                0
            };
            let shape = if ok(a0) {
                m.read32(a0.wrapping_add(0x7c))
            } else {
                0
            };
            let live = ok(a1) && ok(object) && ok(shape);
            if !live {
                return run_original(va, m, guest, regs);
            }
            let regions = [(a1, if live { 7 } else { 0 })];
            shadow_regions(
                va,
                m,
                guest,
                regs,
                &regions,
                Check::None,
                |m, guest, regs| {
                    let entry = *regs;
                    let scale = m.read32(shape.wrapping_add(0xb08));
                    // T2061: with `[0x6D7804] == 1` the original first runs 0x57550(ecx = context), which refreshes the cursor globals
                    // read below (rows, columns, weights) from the actor's aim angles; the port used the stale values.
                    if m.read32(0x006d_7804) == 1 {
                        let mut cr = entry;
                        cr.esp = entry.esp.wrapping_sub(0x60);
                        cr.ecx = a0;
                        guest.call(0x57550, &mut cr);
                    }
                    let cursor = aim::Cursor {
                        rows: [m.read32(0x006d_6180) as i32, m.read32(0x006d_77e8) as i32],
                        columns: [m.read32(0x006d_77e0) as i32, m.read32(0x006d_6e10) as i32],
                        column_weight: float(m, 0x006d_6e08),
                        row_weight: float(m, 0x006d_6e0c),
                    };
                    let mix = m.read32(object.wrapping_add(0xa8)) != 0;
                    let plain =
                        !mix && aim::is_plain(m.read32(shape.wrapping_add(0x80)), a2 as i32);
                    let clips = [
                        clip_pointer(m, slot_of(m, m.read32(object.wrapping_add(0xa0)))),
                        clip_pointer(m, slot_of(m, m.read32(object.wrapping_add(0xa4)))),
                        clip_pointer(
                            m,
                            i16::from_le_bytes([m.read8(0x007e_040a), m.read8(0x007e_040b)]) as i32
                                as u32,
                        ),
                    ];
                    let input = if plain {
                        aim::BlendInput::Plain
                    } else if mix {
                        aim::BlendInput::Mix(float(m, object.wrapping_add(0xac)))
                    } else {
                        aim::BlendInput::Single
                    };
                    let (position, quat) = aim::blend_space_pose(&cursor, input, |which, key| {
                        sample_bits(m, guest, &entry, clips[which], a2, key, scale)
                    });
                    for (i, value) in position.iter().chain(quat.iter()).enumerate() {
                        write_float(m, a1.wrapping_add(4 * i as u32), *value);
                    }
                },
            );
        }
        // 0x58690(clip, time) / 0x5A220(object): heading value track, result in ST0
        0x58690 | 0x5a220 => {
            shadow_regions(va, m, guest, regs, &[], Check::Double, |m, _guest, regs| {
                if va == 0x58690 && !ok(a0) {
                    regs.fpu.push(-1.0);
                    return;
                }
                if va == 0x5a220 && !(ok(a0) && ok(m.read32(a0.wrapping_add(4)))) {
                    regs.fpu.push(0.0);
                    return;
                }
                let (clip, time) = if va == 0x58690 {
                    (a0, f32::from_bits(a1))
                } else {
                    let body = m.read32(a0.wrapping_add(4));
                    (
                        clip_pointer(m, m.read32(body.wrapping_add(0xd8))),
                        float(m, body.wrapping_add(0xdc)),
                    )
                };
                let values_at = m.read32(clip.wrapping_add(0x2c));
                if values_at == 0 {
                    // A missing track is a valid clip, not an invalid guest access.
                    // Return before reading keys/values so the shadow result remains comparable.
                    regs.fpu.push(if va == 0x5a220 { 0.0 } else { -1.0 });
                    return;
                }
                let keys = read_keys(m, clip);
                let mut values = vec![0f32; keys.len()];
                for (i, value) in values.iter_mut().enumerate() {
                    *value = float(m, values_at.wrapping_add(4 * i as u32));
                }
                let track = (values_at != 0).then_some(&values[..]);
                let result =
                    heading::heading(track, &keys, float(m, clip.wrapping_add(0x18)), time);
                regs.fpu.push(result.unwrap_or(0.0));
            });
        }
        _ => return false,
    }
    let _ = read_layer::<M>;
    true
}
