// SPDX-License-Identifier: GPL-3.0-or-later
//! Render state slots, the dirty list and the blend and state group selectors (T1898).
//!
//! The game keeps a 35 record table of wanted render states ([`StateSlot`], one per slot, at
//! [`SLOT_TABLE`]). The HUD, the scene and the effects write wanted values with
//! [`Renderer::state_slot_set`] (0x21450). A slot whose wanted value differs from the value
//! last sent to the device sits on a circular dirty list. [`Renderer::state_apply`] (0x214C0)
//! walks the list once per draw, sends each state to the device and empties the list. The
//! state group (0x19650) and blend mode (0x19D70) selectors are scripts of slot writes.
//!
//! Evidence class: differential versus the lifted original, see docs/t-render-slice.md.
use crate::{
    Renderer,
    device::{HostFunction, RenderDevice, RenderHost},
    map::*,
    mem::GuestAccess,
    memory::GuestMemory,
};

/// Original addresses of the functions ported in this module (T1898, see docs/t-render-slice.md).
pub const ROOTS: [u32; 14] = [
    0x21440, 0x21450, 0x214c0, 0x21740, 0x22560, 0x19640, 0x19d50, 0x19d60, 0x19d40, 0x19650,
    0x19d70, 0x8f470, 0x8f490, 0x8f4e0,
];

/// Slot numbers written by the state group and blend mode scripts. Names are INFERRED from the
/// GL style constants the scripts store (0x302 source alpha, 0x8006 func add, 0x207 always).
pub mod slot {
    pub const BLEND_ENABLE: u32 = 0x00;
    pub const ALPHA_FUNC: u32 = 0x01;
    pub const ALPHA_REF: u32 = 0x02;
    pub const ALPHA_TEST_ENABLE: u32 = 0x03;
    pub const BLEND_EQUATION: u32 = 0x04;
    pub const BLEND_DESTINATION: u32 = 0x07;
    pub const BLEND_SOURCE: u32 = 0x0E;
    pub const STENCIL_ENABLE: u32 = 0x0F;
    pub const STENCIL_FUNC: u32 = 0x10;
    pub const STENCIL_FAIL_OP: u32 = 0x13;
    pub const STENCIL_REF: u32 = 0x14;
    pub const BLEND_COLOUR: u32 = 0x15;
    pub const DEPTH_FUNC: u32 = 0x1A;
    pub const DEPTH_WRITE: u32 = 0x1B;
    /// Both set to 1 whenever the state group changes.
    pub const GROUP_RESET_A: u32 = 0x1E;
    pub const GROUP_RESET_B: u32 = 0x1F;
    /// Slot 6 holds the object colour mode, overridden to 0x901 for flagged model nodes.
    pub const MODEL_COLOUR_MODE: u32 = 0x06;
    pub const COMBINER_MODE: u32 = 0x08;
}

/// One record of the render state slot table (24 bytes). INFERRED field roles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateSlot {
    /// +0x00 render state index the slot drives (see [`Renderer::state_apply`]).
    pub state_index: u32,
    /// +0x04 value last sent to the device.
    pub committed: u32,
    /// +0x08 wanted value.
    pub pending: u32,
    /// +0x0C not touched by the ported functions.
    pub reserved_0c: u32,
    /// +0x10 next older dirty slot, 0 when the slot is not on the dirty list.
    pub older: u32,
    /// +0x14 next newer dirty slot (or the sentinel).
    pub newer: u32,
}

impl StateSlot {
    pub const SIZE: u32 = SLOT_STRIDE;

    /// Guest address of `slot` (wrapping, like the original's index arithmetic).
    pub fn address(slot: u32) -> u32 {
        SLOT_TABLE.wrapping_add(slot.wrapping_mul(SLOT_STRIDE))
    }

    pub fn read<M: GuestMemory>(memory: &M, address: u32) -> Self {
        Self {
            state_index: memory.read32(address),
            committed: memory.read32(address.wrapping_add(0x04)),
            pending: memory.read32(address.wrapping_add(0x08)),
            reserved_0c: memory.read32(address.wrapping_add(0x0C)),
            older: memory.read32(address.wrapping_add(0x10)),
            newer: memory.read32(address.wrapping_add(0x14)),
        }
    }

    pub fn write<M: GuestMemory>(&self, memory: &mut M, address: u32) {
        memory.write32(address, self.state_index);
        memory.write32(address.wrapping_add(0x04), self.committed);
        memory.write32(address.wrapping_add(0x08), self.pending);
        memory.write32(address.wrapping_add(0x0C), self.reserved_0c);
        memory.write32(address.wrapping_add(0x10), self.older);
        memory.write32(address.wrapping_add(0x14), self.newer);
    }
}

/// Entry of the D3D8 handler for each render state index `0x88..=0xA5` (MEASURED against the
/// original dispatch chain at 0x214FE, agrees with docs/d3d8-usage.md section 9.1).
pub const D3D8_STATE_HANDLERS: [(u32, u32); 30] = [
    (0x88, 0x003D_6C60),
    (0x89, 0x003D_74B0),
    (0x8A, 0x003D_7010),
    (0x8B, 0x003D_7380),
    (0x8C, 0x003D_73D0),
    (0x8D, 0x003D_7430),
    (0x8E, 0x003D_7110),
    (0x8F, 0x003D_7EE0),
    (0x90, 0x003D_7F70),
    (0x91, 0x003D_8010),
    (0x92, 0x003D_70D0),
    (0x93, 0x003D_7060),
    (0x94, 0x003D_7150),
    (0x95, 0x003D_72A0),
    (0x96, 0x003D_7320),
    (0x97, 0x003D_6F90),
    (0x98, 0x003D_8250),
    (0x99, 0x003D_82D0),
    (0x9A, 0x003D_81F0),
    (0x9B, 0x003D_8220),
    (0x9C, 0x003D_6FD0),
    (0x9D, 0x003D_71B0),
    (0x9E, 0x003D_8320),
    (0x9F, 0x003D_7220),
    (0xA0, 0x003D_8080),
    (0xA1, 0x003D_80B0),
    (0xA2, 0x003D_8120),
    (0xA3, 0x003D_8190),
    (0xA4, 0x003D_81B0),
    (0xA5, 0x003D_81D0),
];

/// Handler entry of a render state index, `None` outside `0x88..=0xA5`.
pub fn state_handler_entry(index: u32) -> Option<u32> {
    D3D8_STATE_HANDLERS
        .iter()
        .find(|(state, _)| *state == index)
        .map(|(_, entry)| *entry)
}

/// The inlined deferred arm for indices `0x5C..0x88`, for devices backed by guest memory:
/// OR the dirty bit (biased table) into the device mask and store the shadow value.
pub fn defer_state_in_guest<M: GuestMemory>(memory: &mut M, index: u32, value: u32) {
    let bit = memory.read32(DIRTY_BIT_TABLE_BIASED.wrapping_add(index.wrapping_mul(4)));
    memory.or32(DEVICE_DIRTY_MASK, bit);
    memory.write32(
        DEVICE_STATE_SHADOW.wrapping_add(index.wrapping_mul(4)),
        value,
    );
}

/// Texture stage control word for devices backed by guest memory.
pub fn defer_texture_stage_word_in_guest<M: GuestMemory>(
    memory: &mut M,
    stage: u32,
    word: u32,
    value: u32,
) {
    memory.or32(DEVICE_DIRTY_MASK, 1u32.wrapping_shl(stage));
    let address = STAGE_WORDS
        .wrapping_add(stage.wrapping_shl(7))
        .wrapping_add(word * 4);
    memory.write32(address, value);
}

/// A step of a blend mode script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlendStep {
    /// `state_slot_set(slot, value)`.
    Slot(u32, u32),
    /// Reset the blend colour cache [`BLEND_COLOUR_CACHE`] to 0x7F7F7F7F.
    Colour,
}

impl<M: GuestMemory, D: RenderDevice, H: RenderHost> Renderer<'_, M, D, H> {
    /// 0x21440 state_table_4b8d30_get: the wanted value of `slot`.
    pub fn state_slot_pending(&self, slot: u32) -> u32 {
        self.mem.read32(StateSlot::address(slot).wrapping_add(8))
    }

    /// 0x21450 d3d_render_state_slot_set_track_dirty_list: request `value` for `slot`.
    /// The slot joins the dirty list when the value differs from the committed one and
    /// leaves it when they are equal again. Returns the previous wanted value (the EAX
    /// residue of the original).
    pub fn state_slot_set(&mut self, slot: u32, value: u32) -> u32 {
        let address = StateSlot::address(slot);
        let committed = self.mem.read32(address.wrapping_add(4));
        let previous = self.mem.read32(address.wrapping_add(8));
        self.mem.write32(address.wrapping_add(8), value);
        let older = self.mem.read32(address.wrapping_add(0x10));
        if committed != value {
            if older == 0 {
                let newest = self.mem.read32(DIRTY_NEWEST);
                self.mem.write32(address.wrapping_add(0x10), newest);
                self.mem.write32(address.wrapping_add(0x14), DIRTY_SENTINEL);
                self.mem.write32(newest.wrapping_add(0x14), address);
                self.mem.write32(DIRTY_NEWEST, address);
            }
        } else if older != 0 {
            let newer = self.mem.read32(address.wrapping_add(0x14));
            self.mem.write32(newer.wrapping_add(0x10), older);
            self.mem.write32(older.wrapping_add(0x14), newer);
            self.mem.write32(address.wrapping_add(0x10), 0);
            self.mem.write32(address.wrapping_add(0x14), 0);
        }
        previous
    }

    /// 0x21740: true while the dirty list holds a slot.
    pub fn dirty_list_has_entries(&self) -> bool {
        self.mem.read32(DIRTY_NEWEST) != DIRTY_SENTINEL
    }

    /// 0x214C0 d3d_render_state_apply: send every dirty slot to the device, newest first,
    /// mark it committed and empty the list.
    pub fn state_apply(&mut self) -> u32 {
        let mut node = self.mem.read32(DIRTY_NEWEST);
        if node != DIRTY_SENTINEL {
            loop {
                let index = self.mem.read32(node);
                let older = self.mem.read32(node.wrapping_add(0x10));
                let value = self.mem.read32(node.wrapping_add(8));
                self.send_render_state(index, value);
                let wanted = self.mem.read32(node.wrapping_add(8));
                self.mem.write32(node.wrapping_add(4), wanted);
                self.mem.write32(node.wrapping_add(0x10), 0);
                self.mem.write32(node.wrapping_add(0x14), 0);
                node = older;
                if node == DIRTY_SENTINEL {
                    break;
                }
            }
        }
        self.mem.write32(DIRTY_NEWEST, DIRTY_SENTINEL);
        self.mem.write32(DIRTY_OLDEST, DIRTY_SENTINEL);
        DIRTY_SENTINEL
    }

    /// The inlined `SetRenderState` dispatch of the game (docs/d3d8-usage.md 9.1). `index`
    /// is compared signed, like the original.
    fn send_render_state(&mut self, index: u32, value: u32) {
        let signed = index as i32;
        if signed < 0x5C {
            let header = self
                .mem
                .read32(METHOD_HEADER_TABLE.wrapping_add(index.wrapping_mul(4)));
            self.device.push_method(header, value);
        } else if signed < 0x88 {
            self.device.defer_state(index, value);
        } else if state_handler_entry(index).is_some() {
            self.device.state_handler(index, value);
        }
    }

    /// 0x19640 d3d_state_group_55faf8_get.
    pub fn state_group(&self) -> u32 {
        self.mem.read32(STATE_GROUP)
    }

    /// 0x19D50 d3d_blend_mode_55fafc_get.
    pub fn blend_mode(&self) -> u32 {
        self.mem.read32(BLEND_MODE)
    }

    /// 0x19D60 d3d_blend_alpha_55fb04_get.
    pub fn blend_alpha(&self) -> u32 {
        self.mem.read32(BLEND_ALPHA)
    }

    /// 0x19D40: set the blend alpha used by the alpha keyed blend modes.
    pub fn set_blend_alpha(&mut self, alpha: u32) {
        self.mem.write32(BLEND_ALPHA, alpha);
    }

    /// 0x19650 d3d_state_group_55faf8_set_return_previous: select state group `group`
    /// (0..13 run a script of slot writes, other values other than -1 are only stored).
    /// Flushes the pending draw first. Returns the previous group.
    pub fn set_state_group(&mut self, group: u32) -> u32 {
        let previous = self.mem.read32(STATE_GROUP);
        if group == previous || group == u32::MAX {
            return previous;
        }
        self.draw_dispatch();
        self.state_slot_set(slot::GROUP_RESET_A, 1);
        self.state_slot_set(slot::GROUP_RESET_B, 1);
        if let Some(script) = STATE_GROUPS.get(group as usize) {
            for &(target, value) in *script {
                self.state_slot_set(target, value);
            }
        }
        self.mem.write32(STATE_GROUP, group);
        previous
    }

    /// 0x19D70 d3d_blend_mode_55fafc_set_return_previous: select blend mode `mode`.
    /// Modes 0xF..=0x17 first refresh the blend colour slot from the blend alpha. Modes up to
    /// 0x19 run a script of slot writes (some are only stored). Returns the previous mode.
    pub fn set_blend_mode(&mut self, mode: u32) -> u32 {
        let previous = self.mem.read32(BLEND_MODE);
        let signed = mode as i32;
        if (0xF..=0x17).contains(&signed) {
            let alpha = self.mem.read32(BLEND_ALPHA);
            if alpha != self.mem.read32(BLEND_COLOUR_CACHE) {
                self.draw_dispatch();
                let alpha = self.mem.read32(BLEND_ALPHA);
                self.mem.write32(BLEND_COLOUR_CACHE, alpha);
                self.state_slot_set(slot::BLEND_COLOUR, alpha << 0x19);
            }
        }
        if mode == self.mem.read32(BLEND_MODE) {
            return previous;
        }
        self.draw_dispatch();
        if let Some(script) = BLEND_MODES.get(mode as usize) {
            for step in *script {
                match *step {
                    BlendStep::Slot(target, value) => {
                        self.state_slot_set(target, value);
                    }
                    BlendStep::Colour => self.mem.write32(BLEND_COLOUR_CACHE, 0x7F7F_7F7F),
                }
            }
        }
        self.mem.write32(BLEND_MODE, mode);
        previous
    }

    /// 0x8F470 d3d_save_current_state_group_blend_mode_and_alpha.
    pub fn save_state_group_blend_mode_and_alpha(&mut self) {
        let group = self.state_group();
        self.mem.write32(SAVED_STATE_GROUP, group);
        let mode = self.blend_mode();
        self.mem.write32(SAVED_BLEND_MODE, mode);
        let alpha = self.blend_alpha();
        self.mem.write32(SAVED_BLEND_ALPHA, alpha);
    }

    /// 0x8F490 d3d_restore_saved_state_group_and_blend_mode_when_overridden_flags_set.
    pub fn restore_saved_state_group_and_blend_mode(&mut self) {
        if self.mem.read32(RESTORE_GROUP_PENDING) != 0 {
            let group = self.mem.read32(SAVED_STATE_GROUP);
            self.set_state_group(group);
            self.mem.write32(RESTORE_GROUP_PENDING, 0);
        }
        if self.mem.read32(RESTORE_BLEND_PENDING) != 0 {
            let alpha = self.mem.read32(SAVED_BLEND_ALPHA);
            self.set_blend_alpha(alpha);
            let mode = self.mem.read32(SAVED_BLEND_MODE);
            self.set_blend_mode(mode);
            self.mem.write32(RESTORE_BLEND_PENDING, 0);
        }
    }

    /// 0x8F4E0 d3d_effect_apply_blend_state_group_and_render_state: apply the effect flags
    /// of a draw. `flags & 0x1C80` pick one temporary blend override (bit 7 mode 0xE, 0x800
    /// mode 2, 0x400 mode 0xD, 0x1000 additive overlay), `0x10000` also forces state group 4.
    /// `texture` is stored in the texture index slot 0x6652E0 when no override is requested
    /// or when an override replaces it.
    pub fn effect_apply_blend_and_state(&mut self, flags: u32, texture: u32) {
        self.restore_saved_state_group_and_blend_mode();
        if flags & 0x1C80 == 0 {
            self.mem.write32(EFFECT_TEXTURE_INDEX, texture);
        } else if flags & 0x80 != 0 {
            self.set_blend_mode(0xE);
            self.mem.write32(EFFECT_TEXTURE_INDEX, texture);
            self.mem.write32(RESTORE_BLEND_PENDING, 1);
        } else if flags & 0x800 != 0 {
            self.set_blend_mode(2);
            self.mem.write32(EFFECT_TEXTURE_INDEX, texture);
            self.mem.write32(RESTORE_BLEND_PENDING, 1);
        } else if flags & 0x400 != 0 {
            self.set_blend_mode(0xD);
            self.mem.write32(EFFECT_TEXTURE_INDEX, texture);
            self.mem.write32(RESTORE_BLEND_PENDING, 1);
        } else {
            if flags & 0x1000 != 0 {
                self.set_blend_alpha(0xFF);
                self.set_blend_mode(0x15);
                self.host.call(
                    HostFunction::OverlayQuad,
                    &[-6i32 as u32, -1i32 as u32, 1, 0, 0],
                );
            }
            self.mem.write32(RESTORE_BLEND_PENDING, 1);
        }
        if flags & 0x10000 != 0 {
            self.set_state_group(4);
            self.mem.write32(RESTORE_GROUP_PENDING, 1);
            self.state_apply();
        } else if self.mem.read32(RESTORE_GROUP_PENDING) != 0
            || self.mem.read32(RESTORE_BLEND_PENDING) != 0
        {
            self.state_apply();
        }
    }

    /// 0x22560 d3d_render_state_simple_set: re-emit the two NV2A methods that depend on the
    /// blend mode (0x40288 and 0x4028C, plus 0x41E20 for the keyed modes) from the device
    /// shadow words and the blend alpha. The effect row flags (+0xC bit 0 and 1) can veto
    /// either method.
    pub fn refresh_blend_methods(&mut self) {
        let mode = self.blend_mode();
        let row_pointer = self.mem.read32(EFFECT_ROW_POINTER);
        let row_flags = |renderer: &Self| {
            let row = renderer.mem.read32(EFFECT_ROW_INDEX).wrapping_shl(4);
            renderer.mem.read8(row.wrapping_add(EFFECT_ROW_TABLE + 0xC))
        };
        if row_pointer == 0 || row_flags(self) & 1 == 0 {
            let shadow = self.mem.read32(DEVICE_STATE_SHADOW + 8 * 4);
            let (mask, bits) = if mode == 0x13 || mode == 0xA {
                (0xFF, 0x1000_2000)
            } else {
                let keyed = if self.state_slot_pending(slot::COMBINER_MODE) != 0 {
                    0x0303_0000
                } else {
                    0
                };
                (0xFFFF, keyed + 0x1000_0000)
            };
            self.device.push_method(0x4_0288, (mask & shadow) | bits);
        }
        if self.mem.read32(EFFECT_ROW_POINTER) != 0 && row_flags(self) & 2 != 0 {
            return;
        }
        let shadow = self.mem.read32(DEVICE_STATE_SHADOW + 9 * 4);
        let signed = mode as i32;
        let low = if mode == 0x13 {
            let alpha = self.blend_alpha();
            let upper = ((alpha & 0x00FF_0000) | (alpha >> 16)) >> 8;
            let lower = ((alpha << 16) | (alpha & 0xFF00)) << 8;
            self.device.push_method(0x4_1E20, upper | lower);
            0x1100
        } else if (0xF..=0x17).contains(&signed) || mode == 0x18 {
            let alpha = self.blend_alpha();
            let mut packed = (alpha & 0x3FC000) | (alpha & 0x3FC0_0000) | (alpha & 0xC000_3FFF);
            packed >>= 7;
            packed |= alpha & 0x0700_0000;
            packed |= alpha & 0x007F_0000;
            packed |= alpha & 0x0000_7F00;
            packed >>= 7;
            let high = (((alpha << 7) | alpha) & 0xFFFF_FFC0) << 0x12;
            self.device.push_method(0x4_1E20, packed | high);
            0x1100
        } else {
            0x1C00
        };
        self.device
            .push_method(0x4_028C, (shadow & 0xFFFF_00FF) | low);
    }
}

const STATE_GROUPS: [&[(u32, u32)]; 14] = [
    &[
        (slot::DEPTH_FUNC, 0x207),
        (slot::DEPTH_WRITE, 0x0),
        (slot::ALPHA_TEST_ENABLE, 0x0),
        (slot::STENCIL_ENABLE, 0x1),
        (slot::STENCIL_FUNC, 0x207),
        (slot::STENCIL_REF, 0xFF),
        (slot::STENCIL_FAIL_OP, 0x0),
    ],
    &[
        (slot::DEPTH_FUNC, 0x207),
        (slot::DEPTH_WRITE, 0x1),
        (slot::ALPHA_TEST_ENABLE, 0x0),
        (slot::STENCIL_ENABLE, 0x1),
        (slot::STENCIL_FUNC, 0x207),
        (slot::STENCIL_REF, 0xFF),
        (slot::STENCIL_FAIL_OP, 0x0),
    ],
    &[
        (slot::DEPTH_FUNC, 0x203),
        (slot::DEPTH_WRITE, 0x0),
        (slot::ALPHA_TEST_ENABLE, 0x0),
        (slot::STENCIL_ENABLE, 0x0),
    ],
    &[
        (slot::DEPTH_FUNC, 0x204),
        (slot::DEPTH_WRITE, 0x0),
        (slot::ALPHA_TEST_ENABLE, 0x0),
        (slot::STENCIL_ENABLE, 0x0),
    ],
    &[
        (slot::DEPTH_FUNC, 0x203),
        (slot::DEPTH_WRITE, 0x1),
        (slot::ALPHA_TEST_ENABLE, 0x0),
        (slot::STENCIL_ENABLE, 0x1),
        (slot::STENCIL_FUNC, 0x207),
        (slot::STENCIL_REF, 0xFF),
        (slot::STENCIL_FAIL_OP, 0x0),
    ],
    &[
        (slot::DEPTH_FUNC, 0x203),
        (slot::DEPTH_WRITE, 0x1),
        (slot::ALPHA_FUNC, 0x206),
        (slot::ALPHA_TEST_ENABLE, 0x1),
        (slot::ALPHA_REF, 0x73),
        (slot::STENCIL_ENABLE, 0x1),
        (slot::STENCIL_FUNC, 0x207),
        (slot::STENCIL_REF, 0xFF),
        (slot::STENCIL_FAIL_OP, 0x0),
    ],
    &[
        (slot::DEPTH_FUNC, 0x203),
        (slot::DEPTH_WRITE, 0x1),
        (slot::ALPHA_TEST_ENABLE, 0x0),
        (slot::STENCIL_ENABLE, 0x1),
        (slot::STENCIL_FUNC, 0x202),
        (slot::STENCIL_REF, 0x1),
        (slot::STENCIL_FAIL_OP, 0x1E00),
    ],
    &[
        (slot::DEPTH_FUNC, 0x203),
        (slot::DEPTH_WRITE, 0x0),
        (slot::ALPHA_TEST_ENABLE, 0x0),
        (slot::STENCIL_ENABLE, 0x1),
        (slot::STENCIL_FUNC, 0x202),
        (slot::STENCIL_REF, 0x1),
        (slot::STENCIL_FAIL_OP, 0x1E00),
    ],
    &[
        (slot::DEPTH_FUNC, 0x207),
        (slot::DEPTH_WRITE, 0x0),
        (slot::ALPHA_TEST_ENABLE, 0x0),
        (slot::STENCIL_ENABLE, 0x1),
        (slot::STENCIL_FUNC, 0x202),
        (slot::STENCIL_REF, 0x1),
        (slot::STENCIL_FAIL_OP, 0x1E00),
    ],
    &[
        (slot::DEPTH_FUNC, 0x204),
        (slot::DEPTH_WRITE, 0x1),
        (slot::ALPHA_TEST_ENABLE, 0x0),
        (slot::STENCIL_ENABLE, 0x1),
        (slot::STENCIL_FUNC, 0x207),
        (slot::STENCIL_REF, 0xFF),
        (slot::STENCIL_FAIL_OP, 0x0),
    ],
    &[
        (slot::DEPTH_FUNC, 0x203),
        (slot::DEPTH_WRITE, 0x1),
        (slot::ALPHA_FUNC, 0x206),
        (slot::ALPHA_TEST_ENABLE, 0x1),
        (slot::ALPHA_REF, 0x1),
        (slot::STENCIL_ENABLE, 0x1),
        (slot::STENCIL_FUNC, 0x207),
        (slot::STENCIL_REF, 0xFF),
        (slot::STENCIL_FAIL_OP, 0x0),
    ],
    &[
        (slot::DEPTH_FUNC, 0x203),
        (slot::DEPTH_WRITE, 0x0),
        (slot::ALPHA_FUNC, 0x206),
        (slot::ALPHA_TEST_ENABLE, 0x1),
        (slot::ALPHA_REF, 0x1),
        (slot::STENCIL_ENABLE, 0x0),
    ],
    &[
        (slot::DEPTH_FUNC, 0x207),
        (slot::DEPTH_WRITE, 0x0),
        (slot::STENCIL_ENABLE, 0x1),
        (slot::STENCIL_FUNC, 0x207),
        (slot::STENCIL_REF, 0xFF),
        (slot::STENCIL_FAIL_OP, 0x0),
        (slot::ALPHA_FUNC, 0x206),
        (slot::ALPHA_TEST_ENABLE, 0x1),
        (slot::ALPHA_REF, 0x1),
    ],
    &[
        (slot::DEPTH_FUNC, 0x203),
        (slot::DEPTH_WRITE, 0x0),
        (slot::ALPHA_FUNC, 0x201),
        (slot::ALPHA_TEST_ENABLE, 0x1),
        (slot::ALPHA_REF, 0x73),
        (slot::STENCIL_ENABLE, 0x1),
        (slot::STENCIL_FUNC, 0x207),
        (slot::STENCIL_REF, 0xFF),
        (slot::STENCIL_FAIL_OP, 0x0),
    ],
];
const BLEND_MODES: [&[BlendStep]; 26] = [
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x302),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x0),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
    ], // mode 0
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x302),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x303),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
    ], // mode 1
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x302),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x1),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
    ], // mode 2
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x302),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x1),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x800B),
    ], // mode 3
    &[], // mode 4
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x1),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x302),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
    ], // mode 5
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x304),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x305),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
    ], // mode 6
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x305),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x304),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
    ], // mode 7
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x304),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x1),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
    ], // mode 8
    &[], // mode 9
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x306),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x302),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
    ], // mode 10
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x0),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x303),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
    ], // mode 11
    &[], // mode 12
    &[], // mode 13
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x1),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x0),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
    ], // mode 14
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x302),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x303),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
    ], // mode 15
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x302),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x1),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
    ], // mode 16
    &[], // mode 17
    &[], // mode 18
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x306),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x302),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
    ], // mode 19
    &[], // mode 20
    &[], // mode 21
    &[], // mode 22
    &[], // mode 23
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x1),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x1),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
        BlendStep::Colour,
        BlendStep::Slot(slot::BLEND_COLOUR, 0xFE000000),
    ], // mode 24
    &[
        BlendStep::Slot(slot::BLEND_SOURCE, 0x0),
        BlendStep::Slot(slot::BLEND_DESTINATION, 0x301),
        BlendStep::Slot(slot::BLEND_ENABLE, 0x1),
        BlendStep::Slot(slot::BLEND_EQUATION, 0x8006),
    ], // mode 25
];
