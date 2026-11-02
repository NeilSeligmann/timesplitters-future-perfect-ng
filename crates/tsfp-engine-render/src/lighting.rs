// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-engine-render/scene_batch
//! Light slot fill of the scene lighting (T1898 wave 3).
//!
//! Evidence class: differential versus the lifted original, see docs/t-render-slice.md.
use crate::{
    Renderer,
    device::{HostFunction, RenderDevice, RenderHost},
    mem::GuestAccess,
    memory::GuestMemory,
};

/// Original addresses of the functions ported in this module (T1898, see docs/t-render-slice.md).
pub const ROOTS: [u32; 1] = [0x18e910];

const CAMERA_POINTER: u32 = 0x007B_0CC0;

impl<M: GuestMemory, D: RenderDevice, H: RenderHost> Renderer<'_, M, D, H> {
    /// 0x18E910 game_lighting_light_slot_fill_view_space_position_and_color_or_defaults: fill the
    /// light slot at `slot`. With `enabled` the world `position` is transformed into view space
    /// by the camera matrix (`camera + 0x208`, 0x151FC0) into `slot + 0`, `colour` (three
    /// floats) is copied to `slot + 0x10`, and `slot + 0x1C..0x24` get 0.5, 0, 0. Without it the
    /// slot holds the defaults (0, 0, 0, 0.5, colour 0, 1.0, 0, 1.0). Returns 1 when enabled.
    pub fn fill_light_slot(&mut self, slot: u32, enabled: u32, position: u32, colour: u32) -> u32 {
        let half = self.mem.read_f32(0x0047_5CD4);
        if enabled == 0 {
            let one = self.mem.read_f32(0x0047_5C78);
            let defaults: [(u32, f32); 10] = [
                (0x00, 0.0),
                (0x04, 0.0),
                (0x08, 0.0),
                (0x0C, half),
                (0x10, 0.0),
                (0x14, 0.0),
                (0x18, 0.0),
                (0x1C, one),
                (0x20, 0.0),
                (0x24, one),
            ];
            for (offset, value) in defaults {
                self.mem.write_f32(slot.wrapping_add(offset), value);
            }
            return 0;
        }
        let camera = self.mem.read32(CAMERA_POINTER);
        let matrix = self.mem.read32(camera.wrapping_add(0x208));
        self.host.call(
            HostFunction::TransformPointToVec4,
            &[matrix, position, slot],
        );
        for offset in [0u32, 4, 8] {
            let value = self.mem.read32(colour.wrapping_add(offset));
            self.mem.write32(slot.wrapping_add(0x10 + offset), value);
        }
        self.mem.write_f32(slot.wrapping_add(0x1C), half);
        self.mem.write_f32(slot.wrapping_add(0x20), 0.0);
        self.mem.write_f32(slot.wrapping_add(0x24), 0.0);
        1
    }
}
