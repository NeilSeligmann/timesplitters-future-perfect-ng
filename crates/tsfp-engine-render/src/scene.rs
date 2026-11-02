// SPDX-License-Identifier: GPL-3.0-or-later
//! Scene side render entry points: the model node tree walk and the scene object draw
//! (T1898). They own no render state of their own. They drive the slot table, the scissor and
//! the draw calls of the other modules and call the not yet ported model drawing through
//! [`RenderHost`].
//!
//! Evidence class: differential versus the lifted original, see docs/t-render-slice.md.
use crate::{
    Renderer,
    device::{HostFunction, RenderDevice, RenderHost},
    mem::GuestAccess,
    memory::GuestMemory,
    state::slot,
};

/// Original addresses of the functions ported in this module (T1898, see docs/t-render-slice.md).
pub const ROOTS: [u32; 3] = [0x8e710, 0x8f5e0, 0xa2740];

/// Native immutable spatial state and traversal.
#[path = "spatial.rs"]
pub mod spatial;

/// Native immutable portal projection and room visibility.
#[path = "visibility.rs"]
pub mod visibility;

/// Native typed room/object collection and submission order.
#[path = "scene_collect.rs"]
pub mod scene_collect;

/// Opt-in Rust-owned scene light setup, per-view publication and teardown.
#[path = "scene_light_state.rs"]
pub mod light_state;

/// Native immutable object/model preparation contract.
#[path = "render_prep.rs"]
pub mod render_prep;

const NODE_SIZE: u32 = 0xC8;
const MODEL_FLAG_FORCE_COLOUR_MODE: u32 = 0x2000_0000;
const FORCED_COLOUR_MODE: u32 = 0x901;
const CAMERA_POINTER: u32 = 0x007B_0CC0;
const VIEWPORT_RECORD_POINTER: u32 = 0x007E_71C8;
const DEFERRED_LIST: u32 = 0x007B_5B9C;
const DEFERRED_COUNT: u32 = 0x007B_4FB0;

impl<M: GuestMemory, D: RenderDevice, H: RenderHost> Renderer<'_, M, D, H> {
    /// 0x8E710 game_rec_get_resolved_field_0x0_flag_1c_08: the first word of `record`, or the
    /// resolved value when the record's flag byte `+0x1C` has bit 8. -1 for a null record.
    pub fn record_resolved_field(&mut self, record: u32) -> u32 {
        if record == 0 {
            return u32::MAX;
        }
        let field = self.mem.read32(record);
        if self.mem.read8(record.wrapping_add(0x1C)) & 8 != 0 {
            self.host.call(HostFunction::ResolveModelField, &[field])
        } else {
            field
        }
    }

    /// 0x8F5E0 game_model_node_tree_draw_recursive_with_render_state_slot_6_save_restore:
    /// draw node `index` of `model` and, recursively, its first child and its sibling.
    /// Slot 6 is forced to 0x901 for models flagged in `[[model] + 0x28]`. Like the original
    /// it is not restored when `index` is out of range.
    pub fn draw_model_node_tree(&mut self, model: u32, index: u32, arg2: u32, arg3: u32) {
        let data = self.mem.read32(model.wrapping_add(4));
        let count = self.mem.read32(data);
        let nodes = data.wrapping_sub((count as i32).wrapping_mul(NODE_SIZE as i32) as u32);
        let saved = self.state_slot_pending(slot::MODEL_COLOUR_MODE);
        let header = self.mem.read32(model);
        let forced = self.mem.read32(header.wrapping_add(0x28)) & MODEL_FLAG_FORCE_COLOUR_MODE != 0;
        self.state_slot_set(
            slot::MODEL_COLOUR_MODE,
            if forced { FORCED_COLOUR_MODE } else { saved },
        );
        // TEMPORARY-SHIM (removal T2244): import an owned model-node snapshot.
        // Native traversal owns neither guest state nor a per-frame cursor.
        let signed = index as i32;
        if signed < 0 || signed >= count as i32 {
            return;
        }
        let mut snapshot = alloc::vec::Vec::with_capacity(count as usize);
        for i in 0..count {
            let node = nodes.wrapping_add(i.wrapping_mul(NODE_SIZE));
            let link = |offset| {
                let value = self.mem.read8(node.wrapping_add(offset)) as i8;
                if value < 0 {
                    None
                } else {
                    Some(value as usize)
                }
            };
            snapshot.push(render_prep::ModelNode {
                mask: self.mem.read32(node.wrapping_add(0x5c)),
                child: link(0x4b),
                sibling: link(0x4c),
                stop_children: self.mem.read8(node.wrapping_add(0x48)) == 3,
            });
        }
        let mask = self.mem.read32(model.wrapping_add(0x30));
        if let Ok(commands) = render_prep::model_commands(&snapshot, index as usize, mask) {
            let mut slots = alloc::vec::Vec::new();
            for command in commands {
                match command {
                    render_prep::ModelCommand::Enter => {
                        let current = self.state_slot_pending(slot::MODEL_COLOUR_MODE);
                        slots.push(current);
                        self.state_slot_set(
                            slot::MODEL_COLOUR_MODE,
                            if forced { FORCED_COLOUR_MODE } else { current },
                        );
                    }
                    render_prep::ModelCommand::Draw(node) => {
                        self.host.call(
                            HostFunction::DrawModelNode,
                            &[model, node as u32, arg2, arg3],
                        );
                    }
                    render_prep::ModelCommand::Leave => {
                        if let Some(current) = slots.pop() {
                            self.state_slot_set(slot::MODEL_COLOUR_MODE, current);
                        }
                    }
                }
            }
        }
        let _ = forced.then(|| self.state_slot_set(slot::MODEL_COLOUR_MODE, saved));
    }

    /// 0xA2740 game_render_object_set_scissor_and_draw: draw scene `object` into `viewport`.
    /// Marks the object drawn this frame (`+0xC` bit 31, bit 30 when it belongs to the camera
    /// target), decides visibility (the player proxy class defers to 0xA1AB0), derives the
    /// scissor rectangle (the default viewport rectangle when the object has none), draws the
    /// class specific overlay and either queues the object on the deferred list (`+0x10`
    /// bit 25) or draws its model.
    pub fn draw_scene_object(&mut self, object: u32, viewport: u32) {
        let flags = self.mem.read32(object.wrapping_add(0xC)) | 0x8000_0000;
        self.mem.write32(object.wrapping_add(0xC), flags);
        let camera = self.mem.read32(CAMERA_POINTER);
        if self.mem.read32(camera.wrapping_add(0x2A0)) == self.mem.read32(object.wrapping_add(0x34))
        {
            self.mem
                .write32(object.wrapping_add(0xC), flags | 0x4000_0000);
        }
        let class = self.mem.read32(object.wrapping_add(0x20));
        let linked = self.mem.read32(object.wrapping_add(0x208));
        let visible = if self.mem.read16(object.wrapping_add(0x94)) != 0x754
            && class == 0x10_0000
            && linked != 0
        {
            self.host.call(HostFunction::ObjectVisible, &[linked, 0])
        } else {
            !(self.mem.read32(object.wrapping_add(0x2B4)) >> 0x1E) & 1
        };
        if visible == 0 {
            return;
        }
        let target = self.mem.read32(object.wrapping_add(0x34)) as i32;
        let scissor_flags = if target > 0 && target <= 0xC8 {
            self.mem.read32(object.wrapping_add(0x28))
        } else {
            let record = self.mem.read32(VIEWPORT_RECORD_POINTER);
            let fields = [(0x118, 0x00), (0x11C, 0x14), (0x120, 0x04), (0x124, 0x18)];
            for (target, source) in fields {
                let value = self.mem.read32(record.wrapping_add(source)) as i32 as f32;
                self.mem.write_f32(object.wrapping_add(target), value);
            }
            let adjusted = (self.mem.read32(object.wrapping_add(0x28)) & 0xFF7F_DFFF) | 0x40_0000;
            self.mem.write32(object.wrapping_add(0x28), adjusted);
            adjusted
        };
        if scissor_flags & 0x80_0000 != 0 {
            return;
        }
        let model = self.mem.read32(object.wrapping_add(4));
        if self.mem.read8(model.wrapping_add(0x1F6)) & 8 != 0 {
            return;
        }
        // TEMPORARY-SHIM (removal T2244): snapshot the fields owned by the
        // hybrid object producer. Native decisions operate on this typed value.
        let overlay = match class {
            0x20 => render_prep::Overlay::Character,
            8 => render_prep::Overlay::Player,
            _ => render_prep::Overlay::None,
        };
        let snapshot = render_prep::SceneObject {
            // TEMPORARY-SHIM (removal T2244): this invocation-local identity
            // wraps the guest address. Native owners use their stable slot and
            // generation directly; model owners supply asset IDs, not pointers.
            id: render_prep::ObjectId {
                slot: object,
                generation: 0,
            },
            model: render_prep::ModelId(u64::from(model)),
            visible: true,
            scissor_rejected: false,
            model_hidden: false,
            deferred: self.mem.read32(object.wrapping_add(0x10)) & 0x200_0000 != 0,
            overlay,
            scissor: [0x118u32, 0x11c, 0x120, 0x124]
                .map(|o| self.mem.read_f32(object.wrapping_add(o))),
        };
        for command in render_prep::prepare_object(&snapshot) {
            match command {
                render_prep::ObjectCommand::Scissor(rect) => {
                    self.set_scissor_clamped(rect[0], rect[1], rect[2], rect[3])
                }
                render_prep::ObjectCommand::CharacterOverlay(_) => {
                    self.host
                        .call(HostFunction::CharacterOverlay, &[object, viewport]);
                }
                render_prep::ObjectCommand::PlayerOverlay(_) => {
                    let record = self
                        .mem
                        .read32(object.wrapping_add(0x7c))
                        .wrapping_add(0xc68);
                    self.host
                        .call(HostFunction::PlayerRecordDraw, &[record, viewport]);
                }
                render_prep::ObjectCommand::Defer(_) | render_prep::ObjectCommand::DrawModel(_) => {
                    // Adjacent overlay handlers can change the current model or
                    // defer policy. Refresh after their call, preserving original order.
                    if self.mem.read32(object.wrapping_add(0x10)) & 0x200_0000 != 0 {
                        let list = self.mem.read32(DEFERRED_LIST);
                        if list != 0 {
                            let count = self.mem.read32(DEFERRED_COUNT);
                            self.mem
                                .write32(list.wrapping_add(count.wrapping_mul(4)), object);
                            self.mem.write32(DEFERRED_COUNT, count.wrapping_add(1));
                            let queued = (self.mem.read32(object.wrapping_add(0x10)) & 0xfdff_ffff)
                                | 0x400_0000;
                            self.mem.write32(object.wrapping_add(0x10), queued);
                        }
                    } else {
                        let model = self.mem.read32(object.wrapping_add(4));
                        if self.mem.read8(model.wrapping_add(0x1f6)) & 8 == 0 {
                            self.host
                                .call(HostFunction::DrawSceneObject, &[model, viewport, 0]);
                        }
                    }
                }
            }
        }
    }
}
