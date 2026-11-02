// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-ports/c2r/t1976_render
// portreg: wraps=tsfp-engine-render/scene_batch
//! T1976: lane adapters (Tier 2 through `tools.rust_trace`) for the render neighbours that are machine translations
//! (`tools.c2rust`): the batch `c2r/t1976_render.rs` (Tier 1, differential vs lifted, 20,000 seeds) and the T1929 scene
//! translation of `0x8F950` (`scene_batch.rs`, Tier 1, 2,000 seeds). Both use the `ExtRegisters` plus `GuestCall` contract of the
//! lane, so this file only gates which functions the host may select. Roots: 0x8F6B0 (effect apply colour preset, blend and
//! state group), 0x14D60 (quad batch draw), 0x20C40 (vertex shader constant range allocation), 0x1F790 and 0x1C620 (texture
//! stage 0 slot set), 0x1E6B0 (constant colour getter), 0x21060, 0x22740 and 0x1C600 (effect begin and setup resets), 0x8F950 (model
//! node draw with LOD swap). `0x19240` (vertex stream source, one stdcall D3D8 library call) is a hand marshalled arm in
//! `render_lane.rs`: the strict `GuestCall` bridge cannot host a callee that pops its own arguments.
//! `0x210A0` (effect state submit) is not ported: the differential harness cannot drive its D3D8 library calls (docs/t-render-slice.md).
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::t1976_render,
    memory::GuestMemory,
};

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/scene_batch.rs"]
pub(crate) mod scene_batch;

/// Dispatches a root; `false` when `va` is not one of the adapters.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x8f6b0 => t1976_render::run(va, m, guest, regs),
        0x14d60 => t1976_render::run(va, m, guest, regs),
        0x20c40 => t1976_render::run(va, m, guest, regs),
        0x1f790 => t1976_render::run(va, m, guest, regs),
        0x1c620 => t1976_render::run(va, m, guest, regs),
        0x1e6b0 => t1976_render::run(va, m, guest, regs),
        0x21060 => t1976_render::run(va, m, guest, regs),
        0x22740 => t1976_render::run(va, m, guest, regs),
        0x1c600 => t1976_render::run(va, m, guest, regs),
        0x8f950 => {
            scene_batch::game_model_node_draw_with_lod_swap_by_squared_distance_and_color_preset_for_node_and_child(m, guest, regs);
            true
        }
        _ => false,
    }
}

/// The lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 10] = [
    0x8f6b0, 0x14d60, 0x20c40, 0x1f790, 0x1c620, 0x1e6b0, 0x21060, 0x22740, 0x1c600, 0x8f950,
];
