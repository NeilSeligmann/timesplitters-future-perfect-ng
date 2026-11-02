// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-engine-render/scene_batch
// portreg: wraps=tsfp-engine-render/scene_t1938
//! S4 Tier 2 (T2257): hybrid lane adapters for the `engine/render/scene` functions that have a Tier 1 machine translation
//! (`tools.c2rust`) in `tsfp-engine-render`: the T1929 batch `scene_batch.rs` (shared with `render_lane2.rs`, which registers
//! only `0x8F950`) and the T1938 batch `scene_t1938.rs` (included here by `#[path]`, its one import changed from
//! `tsfp_ports::c2r::rt` to `crate::c2r::rt` like `scene_batch.rs`). Roots: every dispatch arm of the two batches that is Tier 1, has a
//! nonzero T1884 execution count and was not already a lane root, minus the two tail-jump bodies `0x8BD10` and `0x81470` (no `ret`,
//! the lane ABI needs a cleanup convention). The translations use the `ExtRegisters` plus `GuestCall` contract, so this file only
//! gates which roots the host may select and runs every root through [`crate::anim_lane::guarded`] (a null or wild pointer is
//! refused, the write footprint is rolled back and the retained original runs; once a callee ran the access is a counted degraded
//! no-op, T2177) with ESP restored to the entry value like `text_lane.rs`. Registered through `lane_inv.rs` (`crates/tsfp-ports/lanes.json`).
use crate::{
    abi::{ExtRegisters, GuestCall},
    lane::LaneCpu,
    lane_inv::execute,
    memory::{GuestMemory, MappedGuest},
    render_lane2::scene_batch,
};

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/scene_t1938.rs"]
mod scene_t1938;

use crate::scene::spatial;
#[path = "scene_lane/collect_adapter.rs"]
mod collect_adapter;
#[path = "scene_lane/object_room_adapter.rs"]
pub(crate) mod object_room_adapter;
#[path = "scene_lane/spatial_adapter.rs"]
mod spatial_adapter;
#[path = "scene_lane/visibility_adapter.rs"]
mod visibility_adapter;

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/camera_projection.rs"]
mod camera_projection;

/// Per-function A/B selector. False keeps the translated original preset.
pub static CAMERA_PROJECTION_RUST: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Zero selects the original projection. Nonzero bits select Hor+ with a
/// presentation-owned full-picture aspect; the switch is sampled per camera call.
static HOR_PLUS_ASPECT_BITS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// Set an opt-in Hor+ aspect, or `None` to restore the original preset.
/// Invalid aspects leave the current selection unchanged.
pub fn set_hor_plus_aspect(aspect: Option<f32>) -> bool {
    let bits = match aspect {
        None => 0,
        Some(value) => {
            if camera_projection::ProjectionPreset::hor_plus(value).is_none() {
                return false;
            }
            value.to_bits()
        }
    };
    HOR_PLUS_ASPECT_BITS.store(bits, core::sync::atomic::Ordering::Release);
    true
}

/// The scene lane roots in registration order (appended to `lane_inv::INV_ROOTS`), heaviest T1884 execution count first.
pub const ROOTS: [u32; 108] = [
    0x1571c0, 0x83790, 0x157440, 0x873c0, 0x96c20, 0x88430, 0x87590, 0x88c60, 0x87720, 0xc77c0,
    0x87bb0, 0x86e10, 0xc84a0, 0xccab0, 0xc9de0, 0x3f050, 0x8c690, 0x1cebc0, 0x8c8e0, 0xc79c0,
    0x87f80, 0x87630, 0xcb3b0, 0x87a70, 0x89170, 0x94620, 0x888a0, 0x8cc20, 0x157320, 0x8eac0,
    0x89af0, 0x88ea0, 0x83980, 0x878a0, 0xf8a30, 0x12c690, 0xc9c20, 0x1b90f0, 0x2e4c80, 0x13a110,
    0xf89e0, 0x810c0, 0x63b40, 0x637e0, 0x63770, 0x18b90, 0x12910, 0x18380, 0x8faa0, 0x903d0,
    0x8bdd0, 0x8dd10, 0x96860, 0x96220, 0x8c910, 0x18fb10, 0x8df60, 0x96410, 0x8dc70, 0x8e420,
    0x157490, 0x62fb0, 0x157600, 0x1574d0, 0x18ce0, 0x89280, 0x8e590, 0x8e4b0, 0x59cd0, 0x1575b0,
    0x12c6e0, 0x963f0, 0x96330, 0x8ff30, 0x16b990, 0xcc720, 0xdca70, 0x9f490, 0x812f0, 0x157970,
    0x8c8c0, 0x81420, 0x63900, 0x63390, 0x2a370, 0x9e610, 0x3f420, 0x8e3d0, 0x86130, 0x240580,
    0x12c370, 0x86fc0, 0x835b0, 0x18fdc0, 0xcb450, 0x812c0, 0x36c9d0, 0x2be260, 0x148310, 0xcb2e0,
    0x85cc0, 0x17600, 0x23ef10, 0xcede0, 0xcbd10, 0xcb3e0, 0x63730, 0x86c40,
];

/// Dispatches a root; `false` when `va` is not one of the scene adapters.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x1571c0 | 0x83790 | 0x157440 | 0x873c0 | 0x96c20 | 0x88430 | 0x87590 | 0x88c60
        | 0x87720 | 0xc77c0 | 0x87bb0 | 0x86e10 | 0xc84a0 | 0xccab0 | 0xc9de0 | 0x3f050
        | 0x8c690 | 0x1cebc0 | 0x8c8e0 | 0xc79c0 | 0x87f80 | 0x87630 | 0xcb3b0 | 0x87a70
        | 0x89170 | 0x94620 | 0x888a0 | 0x8cc20 | 0x157320 | 0x8eac0 | 0x89af0 | 0x88ea0
        | 0x83980 | 0x878a0 | 0xf8a30 | 0x12c690 | 0xc9c20 | 0x1b90f0 | 0x2e4c80 | 0x13a110
        | 0xf89e0 | 0x810c0 | 0x63b40 | 0x637e0 | 0x63770 | 0x18b90 | 0x12910 | 0x18380
        | 0x8faa0 | 0x903d0 | 0x8bdd0 | 0x8dd10 | 0x96860 | 0x96220 | 0x8c910 | 0x18fb10
        | 0x8df60 | 0x96410 | 0x8dc70 | 0x8e420 | 0x157490 | 0x62fb0 | 0x157600 | 0x1574d0
        | 0x18ce0 | 0x89280 | 0x8e590 | 0x8e4b0 | 0x59cd0 | 0x1575b0 | 0x12c6e0 | 0x963f0
        | 0x96330 | 0x8ff30 | 0x16b990 | 0xcc720 | 0xdca70 | 0x9f490 | 0x812f0 | 0x157970
        | 0x8c8c0 | 0x81420 | 0x63900 | 0x63390 | 0x2a370 | 0x9e610 | 0x3f420 | 0x8e3d0
        | 0x86130 | 0x240580 | 0x12c370 | 0x86fc0 | 0x835b0 | 0x18fdc0 | 0xcb450 | 0x812c0
        | 0x36c9d0 | 0x2be260 | 0x148310 | 0xcb2e0 | 0x85cc0 | 0x17600 | 0x23ef10 | 0xcede0
        | 0xcbd10 | 0xcb3e0 | 0x63730 | 0x86c40 => {}
        _ => return false,
    }
    let entry_esp = regs.esp;
    let mut native_visibility = false;
    let handled = crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        let aspect_bits = HOR_PLUS_ASPECT_BITS.load(core::sync::atomic::Ordering::Acquire);
        if va == 0xc77c0
            && (aspect_bits != 0
                || CAMERA_PROJECTION_RUST.load(core::sync::atomic::Ordering::Relaxed)
                || crate::camera::projection_selected())
        {
            let preset = if aspect_bits == 0 {
                camera_projection::ProjectionPreset::Original
            } else {
                camera_projection::ProjectionPreset::HorPlus {
                    content_aspect: f32::from_bits(aspect_bits),
                }
            };
            camera_projection::run_with_preset_and_matrix(
                view,
                guest,
                regs,
                preset,
                crate::camera::projection_selected(),
            );
            true
        } else if va == 0xc79c0 && crate::camera::motion_selected() {
            crate::camera::run_motion(view, guest, regs);
            true
        } else {
            native_visibility = visibility_adapter::run(va, view, regs);
            native_visibility
                || collect_adapter::run(va, view, guest, regs)
                || spatial_adapter::run(va, view, guest, regs)
                || crate::player_wire::run_scene(va, view, guest, regs)
                || scene_batch::run(va, view, guest, regs)
                || scene_t1938::run(va, view, guest, regs)
        }
    });
    regs.esp = entry_esp;
    if va == 0x87bb0 || va == 0x87f80 {
        visibility_adapter::receipt(va, native_visibility && handled);
    }
    handled
}

macro_rules! export {
    ($name:ident, $va:expr) => {
        /// # Safety
        /// `cpu` is a live C rust_cpu object and offset maps the guest window.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(cpu: *mut LaneCpu, offset: usize) {
            let mut memory = unsafe { MappedGuest::new(offset) };
            execute($va, &mut memory, unsafe { &mut *cpu });
        }
    };
}
export!(tsfp_lane_001571c0, 0x1571c0);
export!(tsfp_lane_00083790, 0x83790);
export!(tsfp_lane_00157440, 0x157440);
export!(tsfp_lane_000873c0, 0x873c0);
export!(tsfp_lane_00096c20, 0x96c20);
export!(tsfp_lane_00088430, 0x88430);
export!(tsfp_lane_00087590, 0x87590);
export!(tsfp_lane_00088c60, 0x88c60);
export!(tsfp_lane_00087720, 0x87720);
export!(tsfp_lane_000c77c0, 0xc77c0);
export!(tsfp_lane_00087bb0, 0x87bb0);
export!(tsfp_lane_00086e10, 0x86e10);
export!(tsfp_lane_000c84a0, 0xc84a0);
export!(tsfp_lane_000ccab0, 0xccab0);
export!(tsfp_lane_000c9de0, 0xc9de0);
export!(tsfp_lane_0003f050, 0x3f050);
export!(tsfp_lane_0008c690, 0x8c690);
export!(tsfp_lane_001cebc0, 0x1cebc0);
export!(tsfp_lane_0008c8e0, 0x8c8e0);
export!(tsfp_lane_000c79c0, 0xc79c0);
export!(tsfp_lane_00087f80, 0x87f80);
export!(tsfp_lane_00087630, 0x87630);
export!(tsfp_lane_000cb3b0, 0xcb3b0);
export!(tsfp_lane_00087a70, 0x87a70);
export!(tsfp_lane_00089170, 0x89170);
export!(tsfp_lane_00094620, 0x94620);
export!(tsfp_lane_000888a0, 0x888a0);
export!(tsfp_lane_0008cc20, 0x8cc20);
export!(tsfp_lane_00157320, 0x157320);
export!(tsfp_lane_0008eac0, 0x8eac0);
export!(tsfp_lane_00089af0, 0x89af0);
export!(tsfp_lane_00088ea0, 0x88ea0);
export!(tsfp_lane_00083980, 0x83980);
export!(tsfp_lane_000878a0, 0x878a0);
export!(tsfp_lane_000f8a30, 0xf8a30);
export!(tsfp_lane_0012c690, 0x12c690);
export!(tsfp_lane_000c9c20, 0xc9c20);
export!(tsfp_lane_001b90f0, 0x1b90f0);
export!(tsfp_lane_002e4c80, 0x2e4c80);
export!(tsfp_lane_0013a110, 0x13a110);
export!(tsfp_lane_000f89e0, 0xf89e0);
export!(tsfp_lane_000810c0, 0x810c0);
export!(tsfp_lane_00063b40, 0x63b40);
export!(tsfp_lane_000637e0, 0x637e0);
export!(tsfp_lane_00063770, 0x63770);
export!(tsfp_lane_00018b90, 0x18b90);
export!(tsfp_lane_00012910, 0x12910);
export!(tsfp_lane_00018380, 0x18380);
export!(tsfp_lane_0008faa0, 0x8faa0);
export!(tsfp_lane_000903d0, 0x903d0);
export!(tsfp_lane_0008bdd0, 0x8bdd0);
export!(tsfp_lane_0008dd10, 0x8dd10);
export!(tsfp_lane_00096860, 0x96860);
export!(tsfp_lane_00096220, 0x96220);
export!(tsfp_lane_0008c910, 0x8c910);
export!(tsfp_lane_0018fb10, 0x18fb10);
export!(tsfp_lane_0008df60, 0x8df60);
export!(tsfp_lane_00096410, 0x96410);
export!(tsfp_lane_0008dc70, 0x8dc70);
export!(tsfp_lane_0008e420, 0x8e420);
export!(tsfp_lane_00157490, 0x157490);
export!(tsfp_lane_00062fb0, 0x62fb0);
export!(tsfp_lane_00157600, 0x157600);
export!(tsfp_lane_001574d0, 0x1574d0);
export!(tsfp_lane_00018ce0, 0x18ce0);
export!(tsfp_lane_00089280, 0x89280);
export!(tsfp_lane_0008e590, 0x8e590);
export!(tsfp_lane_0008e4b0, 0x8e4b0);
export!(tsfp_lane_00059cd0, 0x59cd0);
export!(tsfp_lane_001575b0, 0x1575b0);
export!(tsfp_lane_0012c6e0, 0x12c6e0);
export!(tsfp_lane_000963f0, 0x963f0);
export!(tsfp_lane_00096330, 0x96330);
export!(tsfp_lane_0008ff30, 0x8ff30);
export!(tsfp_lane_0016b990, 0x16b990);
export!(tsfp_lane_000cc720, 0xcc720);
export!(tsfp_lane_000dca70, 0xdca70);
export!(tsfp_lane_0009f490, 0x9f490);
export!(tsfp_lane_000812f0, 0x812f0);
export!(tsfp_lane_00157970, 0x157970);
export!(tsfp_lane_0008c8c0, 0x8c8c0);
export!(tsfp_lane_00081420, 0x81420);
export!(tsfp_lane_00063900, 0x63900);
export!(tsfp_lane_00063390, 0x63390);
export!(tsfp_lane_0002a370, 0x2a370);
export!(tsfp_lane_0009e610, 0x9e610);
export!(tsfp_lane_0003f420, 0x3f420);
export!(tsfp_lane_0008e3d0, 0x8e3d0);
export!(tsfp_lane_00086130, 0x86130);
export!(tsfp_lane_00240580, 0x240580);
export!(tsfp_lane_0012c370, 0x12c370);
export!(tsfp_lane_00086fc0, 0x86fc0);
export!(tsfp_lane_000835b0, 0x835b0);
export!(tsfp_lane_0018fdc0, 0x18fdc0);
export!(tsfp_lane_000cb450, 0xcb450);
export!(tsfp_lane_000812c0, 0x812c0);
export!(tsfp_lane_0036c9d0, 0x36c9d0);
export!(tsfp_lane_002be260, 0x2be260);
export!(tsfp_lane_00148310, 0x148310);
export!(tsfp_lane_000cb2e0, 0xcb2e0);
export!(tsfp_lane_00085cc0, 0x85cc0);
export!(tsfp_lane_00017600, 0x17600);
export!(tsfp_lane_0023ef10, 0x23ef10);
export!(tsfp_lane_000cede0, 0xcede0);
export!(tsfp_lane_000cbd10, 0xcbd10);
export!(tsfp_lane_000cb3e0, 0xcb3e0);
export!(tsfp_lane_00063730, 0x63730);

export!(tsfp_lane_00086c40, 0x86c40);
#[cfg(test)]
mod camera_shake_tests {
    use super::*;
    use crate::abi::{Fpu, Xmm};
    use crate::camera::{ShakePhases, ShakePolicy, ShakeState};
    use std::{cell::RefCell, rc::Rc};
    #[derive(Clone)]
    struct Memory(Rc<RefCell<std::vec::Vec<u8>>>);
    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.borrow()[address as usize]
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.borrow_mut()[address as usize] = value;
        }
    }
    struct Calls {
        memory: Memory,
        first_angles: Option<[u32; 3]>,
    }
    impl GuestCall for Calls {
        fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
            if address == 0x153440 && self.first_angles.is_none() {
                // First matrix boundary: matrix arg + yaw arg,
                // then the three local shake components from the caller frame.
                self.first_angles = Some(core::array::from_fn(|i| {
                    self.memory.read32(regs.esp + 8 + i as u32 * 4)
                }));
            }
            // Matrix/room helpers are intentionally stubbed: only the prefix's
            // timer/phases/raw offsets are evidence, never collision/pose parity.
            if address == 0x88c60 {
                regs.eax = 0;
            }
        }
    }
    fn registers() -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x700000,
            ebx: 0,
            ebp: 0,
            esi: 0x30000,
            edi: 0,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x37f,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        }
    }
    #[test]
    fn camera_shake_request_and_prefix_match_translation_with_stubbed_pose_edges() {
        let mut memory = Memory(Rc::new(RefCell::new(std::vec![0; 0x800000])));
        let table = core::array::from_fn(|i| (i as f32 / 1024.0) - 1.0);
        for (i, value) in table.iter().enumerate() {
            memory.write32(0x7a38a0 + i as u32 * 4, value.to_bits());
        }
        let policy = ShakePolicy {
            rates: [73.0, 89.0, 101.0],
            radians_per_degree: core::f32::consts::PI / 180.0,
            table_scale: 325.9493,
            table_bias: 0.5,
            vertical_scale: -1.0,
        };
        for (address, value) in [
            (0x47d618, policy.rates[0]),
            (0x47d5a4, policy.rates[1]),
            (0x47d614, policy.rates[2]),
            (0x475d0c, policy.radians_per_degree),
            (0x475ce0, policy.table_scale),
            (0x475cd4, policy.table_bias),
            (0x475d10, policy.vertical_scale),
        ] {
            memory.write32(address, value.to_bits());
        }
        memory.write32(0x7b0cc0, 0x20000);
        let mut seed = 2330u32;
        for case in 0..2048 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let remaining = match case % 4 {
                0 => 0.0,
                1 => 0.001,
                2 => -0.1,
                _ => (seed % 100) as f32 / 10.0,
            };
            let mut state = ShakeState {
                remaining,
                displacement_amplitude: (seed % 10) as f32,
                up_amplitude: (seed % 20) as f32,
            };
            let mut phases = ShakePhases([
                seed as f32 / 10000.0,
                (seed >> 8) as f32 / 100.0,
                -(seed as f32 / 10000.0),
            ]);
            for (address, value) in [
                (0x20398, remaining),
                (0x2039c, state.displacement_amplitude),
                (0x203a0, state.up_amplitude),
                (0x4dd350, phases.0[0]),
                (0x4dd354, phases.0[1]),
                (0x4dd358, phases.0[2]),
                (0x4e794c, 0.5),
            ] {
                memory.write32(address, value.to_bits());
            }
            memory.write32(0x4e8cec, 60);
            memory.write32(0x2021c, if case % 7 == 0 { 0x80 } else { 0 });
            for offset in [0x364, 0x368, 0x36c] {
                memory.write32(0x20000 + offset, 0);
            }
            let result = state.tick(&mut phases, 0.5, 60, case % 7 == 0, policy, &table);
            let mut calls = Calls {
                memory: memory.clone(),
                first_angles: None,
            };
            scene_t1938::game_camera_update_shake_from_timer_0x398_with_sine_table(
                &mut memory,
                &mut calls,
                &mut registers(),
            );
            assert_eq!(
                memory.read32(0x20398),
                state.remaining.to_bits(),
                "timer {case}"
            );
            for (i, value) in phases.0.into_iter().enumerate() {
                assert_eq!(
                    memory.read32(0x4dd350 + i as u32 * 4),
                    value.to_bits(),
                    "phase {case}/{i}"
                );
            }
            if let Some(result) = result {
                assert_eq!(
                    calls.first_angles.map(|values| values.map(f32::from_bits)),
                    Some(result.world_displacement),
                    "angles {case}"
                );
                for (i, value) in result.up_delta.into_iter().enumerate() {
                    assert_eq!(
                        memory.read32(0x20364 + i as u32 * 4),
                        (0.0 + value).to_bits(),
                        "position {case}/{i}"
                    );
                }
            } else {
                assert_eq!(calls.first_angles, None);
            }

            // Request policy is independently differential-tested before resetting
            // the camera record for the next prefix case.
            let duration: f32 = if case % 11 == 0 { 0.0 } else { 1.0 };
            let angular = (seed % 20) as f32;
            let positional = (seed % 30) as f32;
            memory.write32(0x700004, 0x20000);
            memory.write32(0x700008, duration.to_bits());
            memory.write32(0x70000c, angular.to_bits());
            memory.write32(0x700010, positional.to_bits());
            state.request(duration, angular, positional);
            scene_t1938::game_camera_set_shake_triple_0x398_0x39c_0x3a0_when_idle_or_new_0x39c_larger(&mut memory, &mut registers());
            for (address, value) in [
                (0x20398, state.remaining),
                (0x2039c, state.displacement_amplitude),
                (0x203a0, state.up_amplitude),
            ] {
                assert_eq!(memory.read32(address), value.to_bits(), "request {case}");
            }
        }
    }
    #[test]
    fn camera_full_motion_matches_original_with_translated_math_and_typed_room_fixtures() {
        use crate::camera::{
            BasisPolicy, CameraMotion, CameraPose, CameraRooms, MotionPolicy, MotionTick,
        };
        struct Rooms {
            hit: u32,
            log: std::vec::Vec<(u32, i32, [f32; 3], [f32; 3])>,
        }
        impl CameraRooms for Rooms {
            fn raycast(
                &mut self,
                room: i32,
                origin: [f32; 3],
                delta: [f32; 3],
            ) -> Option<[f32; 3]> {
                self.log.push((0x88c60, room, origin, delta));
                match self.hit {
                    0 => None,
                    1 => Some(origin),
                    _ => Some(core::array::from_fn(|i| origin[i] + delta[i] * 0.9)),
                }
            }
            fn trace_segment(&mut self, room: i32, origin: [f32; 3], delta: [f32; 3]) -> i32 {
                self.log.push((0x88430, room, origin, delta));
                room + 1
            }
        }
        struct OriginalCalls {
            memory: Memory,
            rooms: Rooms,
        }
        impl GuestCall for OriginalCalls {
            fn call(&mut self, va: u32, r: &mut ExtRegisters) {
                match va {
                    0x153440 | 0x1533e0 | 0x151e80 => {
                        r.esp -= 4; // bridge's real callee return slot
                        match va {
                            0x153440 => crate::c2r::t1937_math::game_matrix4x4_make_rotation_y(
                                &mut self.memory,
                                r,
                            ),
                            0x1533e0 => crate::c2r::t1937_math::game_matrix4x4_make_rotation_x(
                                &mut self.memory,
                                r,
                            ),
                            _ => {
                                crate::c2r::t1937_math::game_matrix4x4_transform_point_vec3_inplace(
                                    &mut self.memory,
                                    r,
                                )
                            }
                        }
                        r.esp += 4;
                    }
                    0x88c60 => {
                        // The real scene query filters by argument 7. A default
                        // zero mask passes fake geometry but changes real hits.
                        assert_eq!(self.memory.read32(r.esp + 28), (-7i32) as u32);
                        for offset in [12, 24, 32, 36] {
                            assert_eq!(self.memory.read32(r.esp + offset), 0);
                        }
                        let room = self.memory.read32(r.esp) as i32;
                        let origin_pointer = self.memory.read32(r.esp + 4);
                        let delta_pointer = self.memory.read32(r.esp + 8);
                        let origin = core::array::from_fn(|i| {
                            f32::from_bits(self.memory.read32(origin_pointer + i as u32 * 4))
                        });
                        let delta = core::array::from_fn(|i| {
                            f32::from_bits(self.memory.read32(delta_pointer + i as u32 * 4))
                        });
                        if let Some(hit) = self.rooms.raycast(room, origin, delta) {
                            let hit_pointer = self.memory.read32(r.esp + 16);
                            for (i, value) in hit.into_iter().enumerate() {
                                self.memory
                                    .write32(hit_pointer + i as u32 * 4, value.to_bits());
                            }
                            r.eax = 1;
                        } else {
                            r.eax = 0;
                        }
                    }
                    0x88430 => {
                        let origin_pointer = self.memory.read32(r.esp);
                        let delta_pointer = self.memory.read32(r.esp + 4);
                        let room = self.memory.read32(r.esp + 8) as i32;
                        let origin = core::array::from_fn(|i| {
                            f32::from_bits(self.memory.read32(origin_pointer + i as u32 * 4))
                        });
                        let delta = core::array::from_fn(|i| {
                            f32::from_bits(self.memory.read32(delta_pointer + i as u32 * 4))
                        });
                        r.eax = self.rooms.trace_segment(room, origin, delta) as u32;
                    }
                    _ => panic!("unexpected camera edge {va:x}"),
                }
            }
        }
        let mut memory = Memory(Rc::new(RefCell::new(std::vec![0; 0x800000])));
        for i in 0..16 {
            memory.write32(
                0x4e7900 + i * 4,
                if i % 5 == 0 { 1.0f32.to_bits() } else { 0 },
            );
        }
        let table = core::array::from_fn(|i| i as f32 / 1024.0 - 1.0);
        for (i, value) in table.iter().enumerate() {
            memory.write32(0x7a38a0 + i as u32 * 4, value.to_bits());
        }
        let policy = MotionPolicy {
            shake: ShakePolicy {
                rates: [73.0, 89.0, 101.0],
                radians_per_degree: core::f32::consts::PI / 180.0,
                table_scale: 325.9493,
                table_bias: 0.5,
                vertical_scale: 0.3,
            },
            basis: BasisPolicy {
                radians_per_degree: core::f32::consts::PI / 180.0,
                normalization_threshold: 0.000001,
                normalization_length: 1.0,
            },
            decay_cutoff: 0.001,
            probe_extension: 0.5,
            minimum_hit_distance: 0.1,
        };
        for (address, value) in [
            (0x47d618, 73.0f32),
            (0x47d5a4, 89.0),
            (0x47d614, 101.0),
            (0x475d0c, policy.shake.radians_per_degree),
            (0x475ce0, policy.shake.table_scale),
            (0x475cd4, 0.5),
            (0x475d10, 0.3),
            (0x475c78, 1.0),
            (0x4765f4, 0.000001),
            (0x475d1c, 0.001),
            (0x4788e8, 0.1),
            (0x4e794c, 1.0),
        ] {
            memory.write32(address, value.to_bits());
        }
        memory.write32(0x4e8cec, 60);
        memory.write32(0x7b0cc0, 0x20000);
        for case in 0..2048 {
            let mut motion = CameraMotion {
                shake: ShakeState {
                    remaining: (case % 4) as f32 * 0.1,
                    displacement_amplitude: (case % 17) as f32 * 0.05,
                    up_amplitude: 0.03,
                },
                recoil_distance: (case % 7) as f32 * 0.1,
                recoil_decay: 0.95,
                pitch_degrees: (case % 19) as f32 * 0.3,
                pitch_decay: 0.9,
                kick_distance: (case % 5) as f32 * 0.02,
            };
            let mut pose = CameraPose {
                eye: [2.0, 4.0, 6.0],
                direction: [
                    ((case * 13 % 257) as f32 - 128.0) / 128.0,
                    ((case * 19 % 251) as f32 - 125.0) / 125.0,
                    ((case * 29 % 263) as f32 - 131.0) / 131.0,
                ],
                up: [0.0, 1.0, 0.0],
                room: if case % 13 == 0 { 201 } else { 4 },
                previous_room: -10,
            };
            let mut phases = ShakePhases([case as f32, -(case as f32), (case % 360) as f32]);
            let tick = MotionTick {
                frame_scale: 1.0,
                tick_divisor: 60,
                elapsed_frames: case % 4,
                kick_direction: [0.1, -0.3, 0.2],
                base_direction: [0.3, 0.2, 0.9],
                disabled: case % 9 == 0,
            };
            for (address, values) in [
                (0x2027c, pose.eye),
                (0x20334, tick.base_direction),
                (0x20364, pose.up),
                (0x30000, pose.direction),
                (0x4dd350, phases.0),
                (0x7330a0, tick.kick_direction),
            ] {
                for (i, value) in values.into_iter().enumerate() {
                    memory.write32(address + i as u32 * 4, value.to_bits());
                }
            }
            for (offset, value) in [
                (0x398, motion.shake.remaining),
                (0x39c, motion.shake.displacement_amplitude),
                (0x3a0, motion.shake.up_amplitude),
                (0x3a4, motion.recoil_distance),
                (0x3a8, motion.recoil_decay),
                (0x3ac, motion.pitch_degrees),
                (0x3b0, motion.pitch_decay),
                (0x3b8, motion.kick_distance),
            ] {
                memory.write32(0x20000 + offset, value.to_bits());
            }
            memory.write32(0x202a0, pose.room as u32);
            memory.write32(0x202a4, pose.previous_room as u32);
            memory.write32(0x2021c, if tick.disabled { 0x80 } else { 0 });
            memory.write32(0x4e7944, tick.elapsed_frames as u32);
            let mut regs = registers();
            if case % 2 == 0 {
                regs.fpu.control = 0x7f;
            }
            let mut adapter_memory = Memory(Rc::new(RefCell::new(memory.0.borrow().clone())));
            let mut adapter_regs = regs;
            let mut adapter_calls = OriginalCalls {
                memory: adapter_memory.clone(),
                rooms: Rooms {
                    hit: case as u32 % 3,
                    log: std::vec::Vec::new(),
                },
            };
            crate::camera::run_motion(&mut adapter_memory, &mut adapter_calls, &mut adapter_regs);
            assert_eq!(
                (
                    adapter_regs.esp,
                    adapter_regs.ebx,
                    adapter_regs.ebp,
                    adapter_regs.esi,
                    adapter_regs.edi
                ),
                (regs.esp, regs.ebx, regs.ebp, regs.esi, regs.edi),
                "adapter saved ABI {case}"
            );
            let mut native_rooms = Rooms {
                hit: case as u32 % 3,
                log: std::vec::Vec::new(),
            };
            motion.tick(
                &mut pose,
                &mut phases,
                tick,
                policy,
                &table,
                |value| crate::c2r::rt::fp_pc(&regs.fpu, value),
                &mut native_rooms,
            );
            let mut original_calls = OriginalCalls {
                memory: memory.clone(),
                rooms: Rooms {
                    hit: case as u32 % 3,
                    log: std::vec::Vec::new(),
                },
            };
            scene_t1938::game_camera_update_shake_from_timer_0x398_with_sine_table(
                &mut memory,
                &mut original_calls,
                &mut regs,
            );
            for (address, values) in [
                (0x2027c, pose.eye),
                (0x20364, pose.up),
                (0x30000, pose.direction),
                (0x4dd350, phases.0),
            ] {
                for (i, value) in values.into_iter().enumerate() {
                    let expected = f32::from_bits(memory.read32(address + i as u32 * 4));
                    assert_eq!(value, expected, "pose {case}/{address:x}/{i}");
                }
            }
            assert_eq!(pose.room as u32, memory.read32(0x202a0), "room {case}");
            assert_eq!(
                pose.previous_room as u32,
                memory.read32(0x202a4),
                "old room {case}"
            );
            for (offset, value) in [
                (0x398, motion.shake.remaining),
                (0x39c, motion.shake.displacement_amplitude),
                (0x3a0, motion.shake.up_amplitude),
                (0x3a4, motion.recoil_distance),
                (0x3a8, motion.recoil_decay),
                (0x3ac, motion.pitch_degrees),
                (0x3b0, motion.pitch_decay),
                (0x3b8, motion.kick_distance),
            ] {
                assert_eq!(
                    value,
                    f32::from_bits(memory.read32(0x20000 + offset)),
                    "state {case}/{offset:x}"
                );
            }
            assert_eq!(
                native_rooms.log, original_calls.rooms.log,
                "query contract {case}"
            );
            assert_eq!(
                adapter_calls.rooms.log, original_calls.rooms.log,
                "adapter query contract {case}"
            );
            for address in (0x2027c..0x20294)
                .step_by(4)
                .chain([0x202a0, 0x202a4])
                .chain((0x20364..0x20370).step_by(4))
                .chain((0x20398..0x203bc).step_by(4))
                .chain((0x30000..0x3000c).step_by(4))
                .chain((0x4dd350..0x4dd35c).step_by(4))
            {
                if [0x202a0, 0x202a4].contains(&address) {
                    assert_eq!(
                        adapter_memory.read32(address),
                        memory.read32(address),
                        "adapter room {case}/{address:x}"
                    );
                } else {
                    assert_eq!(
                        f32::from_bits(adapter_memory.read32(address)),
                        f32::from_bits(memory.read32(address)),
                        "adapter output {case}/{address:x}"
                    );
                }
            }
        }
    }
    #[test]
    fn camera_native_projection_adapter_matches_original_matrix_and_tangents() {
        struct MatrixCall(Memory);
        impl GuestCall for MatrixCall {
            fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
                assert_eq!(va, 0x151cf0);
                regs.esp -= 4;
                crate::c2r::t1937_math::game_matrix4x4_make_perspective_fov(&mut self.0, regs);
                regs.esp += 4;
            }
        }
        struct NoCalls;
        impl GuestCall for NoCalls {
            fn call(&mut self, va: u32, _: &mut ExtRegisters) {
                panic!("native projection must not call C math {va:x}");
            }
        }
        let mut memory = Memory(Rc::new(RefCell::new(std::vec![0; 0x800000])));
        for case in 0..2048 {
            for (address, value) in [
                (0x7b0cc0, 0x20000),
                (0x20204, 0x40000),
                (0x7e71c8, 0x50000),
                (0x5000c, 640 + case),
                (0x50020, 480),
                (0x700004, 0.1f32.to_bits()),
                (0x700008, 1000.0f32.to_bits()),
                (0x70000c, (0.5 + (case % 100) as f32 / 100.0).to_bits()),
                (0x700010, (20.0 + (case % 120) as f32).to_bits()),
                (0x475cd4, 0.5f32.to_bits()),
                (0x475d0c, (core::f32::consts::PI / 180.0).to_bits()),
                (0x4760d0, (-1.0f32).to_bits()),
                (0x4760cc, 1.5f32.to_bits()),
                (0x7de458, if case % 3 == 0 { 0x40000000 } else { 0 }),
            ] {
                memory.write32(address, value);
            }
            let mut native_memory = Memory(Rc::new(RefCell::new(memory.0.borrow().clone())));
            let mut original_regs = registers();
            if case % 2 == 0 {
                original_regs.fpu.control = 0x7f;
            }
            let mut native_regs = original_regs;
            let mut calls = MatrixCall(memory.clone());
            crate::render_lane2::scene_batch::game_camera_set_perspective_from_fov_and_viewport_aspect(&mut memory, &mut calls, &mut original_regs);
            camera_projection::run_with_preset_and_matrix(
                &mut native_memory,
                &mut NoCalls,
                &mut native_regs,
                camera_projection::ProjectionPreset::Original,
                true,
            );
            for address in (0x40000..0x40040).step_by(4).chain([0x2037c, 0x20380]) {
                assert_eq!(
                    native_memory.read32(address),
                    memory.read32(address),
                    "projection {case}/{address:x}"
                );
            }
            assert_eq!(
                (
                    native_regs.esp,
                    native_regs.ebx,
                    native_regs.ebp,
                    native_regs.esi,
                    native_regs.edi,
                    native_regs.fpu.top
                ),
                (
                    original_regs.esp,
                    original_regs.ebx,
                    original_regs.ebp,
                    original_regs.esi,
                    original_regs.edi,
                    original_regs.fpu.top
                )
            );
        }
    }
}
