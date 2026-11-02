// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/editor
//! T2429: whole-function Rust port of the Map Maker per-frame editor update `0x306460`
//! (1,380 instructions, 58 distinct callees, map in `docs/t2427-mapmaker-306460-map.md`).
//!
//! The function body is Rust: it reads and writes the editor context through the typed
//! [`EditorContext`] view over `[0x7844A8]`, uses the pure pieces of [`crate::mapmaker_view`]
//! (hint fade, tool ring, build height, scroll facing, zoom) and calls every callee as a guest
//! call (`TEMPORARY-SHIM`: removal follows each callee's own port; T2432 ported five in [`callees`]). Callee argument
//! shapes (stack words, 6-byte cell structs, x87 results) were read from the lifted translation
//! and are checked by the Tier 1 differential `tests/mapmaker_update_tier1.rs` (callee call log,
//! memory, registers; fabricated seeds, weak evidence).
//!
//! Evidence: Tier 1 plus attempted Tier 2 gates. The latest current-host gate was INVALID
//! because its lifted/off control diverged, so it is not adoption evidence. The A/B lane root
//! `0x306460` defaults to lifted code; set `TSFP_MAPMAKER_OVERLAY=rust` to select Rust.
//! The float constants are the literal values stored in the image (`0x4785D4`=0.02, `0x475D20`=1.5,
//! `0x475D24`=0.1, `0x47D780`=1.02, `0x47D6DC`=0.98, ...); the differential seeds the image with the same
//! values, so a wrong literal shows up as a diff.
//!
//! Scratch registers other than the callee-saved ones are not reproduced; the function is void.
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    inventory::sse,
    mapmaker_view::{HintFade, Zoom, cycle_tool_mode, scroll_facing, step_height},
    memory::GuestMemory,
    system_slice::round_pc,
};

#[path = "mapmaker_update/callees.rs"]
pub mod callees;
use callees::CalleeHost;

/// Pointer to the current [`EditorContext`].
pub const CONTEXT_POINTER: u32 = 0x0078_44a8;
/// The current widget page.
pub const PAGE: u32 = 0x0078_42a8;
/// Pointer to the hint panel block (see the hint helpers below).
pub const HINT_BLOCK_POINTER: u32 = 0x0077_8d34;
/// Index of the active pointer view; view records are `0x1E0` bytes apart.
const VIEW_INDEX: u32 = 0x007b_a9e0;
const VIEW_STRIDE: u32 = 0x1e0;
/// Per view: root widget (`+0x2C`-relative table entry), pointer x and y (f32), button byte.
const VIEW_WINDOW: u32 = 0x007b_a26c;
const VIEW_POINTER_X: u32 = 0x007b_a280;
const VIEW_POINTER_Y: u32 = 0x007b_a284;
const VIEW_BUTTONS: u32 = 0x007b_a260;
const UI_SCALE: u32 = 0x007b_a038;
const UI_MARGIN: u32 = 0x007b_a188;
const SCREEN_WIDTH: u32 = 0x004e_8cf4;
const SCREEN_HEIGHT: u32 = 0x004e_8cf0;
const FRAME_RATE: u32 = 0x004e_8cec;
const TIME_SCALE: u32 = 0x004e_7948;
const SCROLL_FACING: u32 = 0x0078_3cc8;
const FINAL_TARGET: u32 = 0x0078_4014;
const SPRITE_TABLE: u32 = 0x0052_9510;
const ANGLE_TABLE: u32 = 0x0052_8c98;
const MODE_PAGE_TABLE: u32 = 0x0052_cd50;
const KIND_TABLE: u32 = 0x0052_4f5a;
const KIND_STRIDE: u32 = 0xc4;
const KIND_PLACEMENT: u16 = 0x22;
/// Hint block (`[0x778D34]`) offsets: state word, last sprite, last angle (f32), last record.
const HINT_STATE: u32 = 0x80;
const HINT_SPRITE: u32 = 0x90;
const HINT_ANGLE: u32 = 0x3c;
const HINT_RECORD: u32 = 0x8c;

/// Frame flag bits of `ctx+0x25C`.
mod frame {
    pub const ZOOM_OR_SCROLL: u32 = 1;
    pub const SCROLLED: u32 = 2;
    pub const HOVER_VALID: u32 = 4;
    pub const SHIFTED: u32 = 0x20;
    pub const SIGN: u32 = 0x80;
    pub const OTHER_VIEW: u32 = 0x0004_4088;
    pub const BAIL_MASK: u32 = 0x1600_4008;
    pub const KEEP_PICKS: u32 = 0x2000_0000;
    pub const ALT_VIEW: u32 = 0x0200_0000;
    pub const HIDE_MASK: u32 = 0x0404_4088;
    pub const NO_POINTER: u32 = 0x0800_0000;
}

/// Typed view over the editor context record (`[0x7844A8]`). Field names are INFERRED
/// (docs/t2427-mapmaker-306460-map.md); each accessor is one guest word.
#[derive(Clone, Copy, Debug)]
pub struct EditorContext {
    pub base: u32,
}

macro_rules! context_words {
    ($($(#[$doc:meta])* $get:ident / $set:ident = $offset:literal;)*) => {
        impl EditorContext {
            $(
                $(#[$doc])*
                pub fn $get<M: GuestMemory>(&self, m: &M) -> u32 {
                    m.read32(self.base.wrapping_add($offset))
                }
                pub fn $set<M: GuestMemory>(&self, m: &mut M, value: u32) {
                    m.write32(self.base.wrapping_add($offset), value)
                }
            )*
        }
    };
}

context_words! {
    /// Extended toolset flag (tool ring of five, height steps).
    extended_toolset / set_extended_toolset = 0x200;
    /// Mode flags: bit 1 pending, `0x2000` modal request, `0x4000` suppress, bit 15 hide overlay.
    mode_flags / set_mode_flags = 0x238;
    /// Height of the picked record relative to the build level.
    pick_height / set_pick_height = 0x240;
    previous_tool_mode / set_previous_tool_mode = 0x250;
    tool_mode / set_tool_mode = 0x254;
    /// Cleared at the start of every tool pass.
    tool_scratch / set_tool_scratch = 0x258;
    frame_flags / set_frame_flags = 0x25c;
    /// Hint fade timer (f32 bits).
    hint_fade / set_hint_fade = 0x260;
    build_level / set_build_level = 0x264;
    build_level_below / set_build_level_below = 0x268;
    /// Picked object kind, -1 none.
    pick_kind / set_pick_kind = 0x2f34;
    /// Picked record (placed object), 0 none.
    pick_record / set_pick_record = 0x2f18;
    pick_aux / set_pick_aux = 0x2f1c;
    pick_cell_link / set_pick_cell_link = 0x2f20;
    pick_saved / set_pick_saved = 0x2f70;
    pick_slot / set_pick_slot = 0x2f74;
    pick_group / set_pick_group = 0x2f78;
    /// Hovered second record.
    hover_record / set_hover_record = 0x2f7c;
    /// Optional float offset triple applied to the cursor cell.
    cell_offset_pointer / set_cell_offset_pointer = 0x2f80;
    last_pointer_x / set_last_pointer_x = 0x2f84;
    last_pointer_y / set_last_pointer_y = 0x2f88;
    anchor_x / set_anchor_x = 0x2f94;
    anchor_y / set_anchor_y = 0x2f98;
    drag_source / set_drag_source = 0x2f9c;
    /// Tool sub-state (-1 none).
    tool_substate / set_tool_substate = 0x1185c;
}

/// A cursor cell: three 16-bit grid coordinates, passed to callees as a six byte stack struct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    x: i16,
    y: i16,
    z: i16,
}

impl EditorContext {
    fn tool_substate_byte<M: GuestMemory>(&self, m: &M) -> u8 {
        m.read8(self.base.wrapping_add(0x11854))
    }

    fn picked_cell<M: GuestMemory>(&self, m: &M) -> i16 {
        m.read16(self.base.wrapping_add(0x2f10)) as i16
    }

    fn set_picked_cell<M: GuestMemory>(&self, m: &mut M, value: u16) {
        write16(m, self.base.wrapping_add(0x2f10), value);
    }

    /// The raw cursor cell at `+0x2F28`.
    fn raw_cell<M: GuestMemory>(&self, m: &M) -> Cell {
        Cell {
            x: m.read16(self.base.wrapping_add(0x2f28)) as i16,
            y: m.read16(self.base.wrapping_add(0x2f2a)) as i16,
            z: m.read16(self.base.wrapping_add(0x2f2c)) as i16,
        }
    }

    /// The cursor cell plus the optional float offset triple (`+0x2F80`), truncated to 16 bits.
    fn offset_cell<M: GuestMemory>(&self, m: &M) -> Cell {
        let raw = self.raw_cell(m);
        let pointer = self.cell_offset_pointer(m);
        if pointer == 0 {
            return raw;
        }
        let shift = |value: i16, word: u32| {
            let offset = f32::from_bits(m.read32(pointer.wrapping_add(word)));
            sse::cvttss2si(f32::from(value) + offset) as u16 as i16
        };
        Cell {
            x: shift(raw.x, 0),
            y: shift(raw.y, 4),
            z: shift(raw.z, 8),
        }
    }
}

fn write16<M: GuestMemory>(m: &mut M, address: u32, value: u16) {
    m.write8(address, value as u8);
    m.write8(address.wrapping_add(1), (value >> 8) as u8);
}

/// Guest-call frame: memory, callee gateway and registers of the running body.
struct Update<'a, M: GuestMemory, C: GuestCall> {
    m: &'a mut M,
    guest: &'a mut C,
    regs: &'a mut ExtRegisters,
    /// ESP at entry (the return address slot). Locals live below it, like the original frame.
    entry_esp: u32,
}

impl<M: GuestMemory, C: GuestCall> CalleeHost for Update<'_, M, C> {
    type Memory = M;

    fn memory(&mut self) -> &mut M {
        self.m
    }

    fn guest_call(&mut self, va: u32, args: &[u32]) -> u32 {
        self.call(va, args)
    }
}

impl<M: GuestMemory, C: GuestCall> Update<'_, M, C> {
    /// The pointer slot passed to the lifted `0x2F8450` helper, relative to entry ESP.
    fn slot_c(&self) -> u32 {
        self.entry_esp.wrapping_sub(0xc)
    }

    /// The second pointer slot passed to the lifted `0x2F8450` helper.
    fn slot_10(&self) -> u32 {
        self.entry_esp.wrapping_sub(0x10)
    }

    fn ctx(&self) -> EditorContext {
        EditorContext {
            base: self.m.read32(CONTEXT_POINTER),
        }
    }

    /// cdecl guest call returning EAX.
    fn call(&mut self, va: u32, args: &[u32]) -> u32 {
        call_cdecl(self.guest, self.m, self.regs, va, args);
        self.regs.eax
    }

    /// cdecl guest call returning an x87 value in ST0 (popped here).
    fn call_st0(&mut self, va: u32, args: &[u32]) -> f64 {
        call_cdecl(self.guest, self.m, self.regs, va, args);
        let value = self.regs.fpu.st(0);
        self.regs.fpu.pop();
        value
    }

    /// Guest call with a [`Cell`] struct on the stack (dword x/y, word z), optionally preceded by one
    /// word argument. Returns EAX.
    fn call_cell(&mut self, va: u32, head: Option<u32>, cell: Cell) -> u32 {
        let entry = self.regs.esp;
        self.regs.esp = entry.wrapping_sub(8);
        self.m.write32(
            self.regs.esp,
            u32::from(cell.x as u16) | (u32::from(cell.y as u16) << 16),
        );
        write16(self.m, self.regs.esp.wrapping_add(4), cell.z as u16);
        if let Some(word) = head {
            self.regs.esp = self.regs.esp.wrapping_sub(4);
            self.m.write32(self.regs.esp, word);
        }
        self.guest.call(va, self.regs);
        self.regs.esp = entry;
        self.regs.eax
    }

    fn view_offset(&self) -> u32 {
        self.m.read32(VIEW_INDEX).wrapping_mul(VIEW_STRIDE)
    }

    /// Raw pointer words (f32 bits) of the active view.
    fn pointer_words(&self) -> (u32, u32) {
        let offset = self.view_offset();
        (
            self.m.read32(VIEW_POINTER_X.wrapping_add(offset)),
            self.m.read32(VIEW_POINTER_Y.wrapping_add(offset)),
        )
    }

    fn pointer_world(&self) -> (f32, f32) {
        let (x, y) = self.pointer_words();
        let m = &*self.m;
        let word = |address: u32| f32::from_bits(m.read32(address));
        let origin = |address: u32| m.read32(address) as i32 as f32;
        (
            (f32::from_bits(x) - origin(0x0078_3ee0)) * word(0x0078_3dd4) + word(0x0078_3dd8),
            (f32::from_bits(y) - origin(0x0078_3dc4)) * word(0x0078_3f20) + word(0x0078_3cc0),
        )
    }

    /// The widget under the active pointer view (`0x77470`).
    fn window(&mut self) -> u32 {
        let root = self.m.read32(VIEW_WINDOW.wrapping_add(self.view_offset()));
        self.call(0x77470, &[root])
    }

    fn pointer_on_page(&mut self) -> bool {
        let window = self.window();
        window == self.m.read32(PAGE)
    }

    fn flags(&self) -> u32 {
        self.ctx().frame_flags(self.m)
    }

    fn clear_flags(&mut self, mask: u32) {
        let ctx = self.ctx();
        let flags = ctx.frame_flags(self.m);
        ctx.set_frame_flags(self.m, flags & !mask);
    }

    fn set_flags(&mut self, bits: u32) {
        let ctx = self.ctx();
        let flags = ctx.frame_flags(self.m);
        ctx.set_frame_flags(self.m, flags | bits);
    }

    /// Epilogue shared by the hint, bail and early-out paths.
    fn finish_frame(&mut self) {
        let target = self.m.read32(FINAL_TARGET);
        self.call(0x337760, &[target]);
        self.call(0x32f8e0, &[]);
    }

    /// `frame bit` test used before most tool work: no "other view" bit set and the pointer is over the page.
    fn editing_here(&mut self) -> bool {
        self.flags() & frame::HIDE_MASK == 0 && self.pointer_on_page()
    }
}

/// `0x306460` on guest memory: cdecl, no arguments, void.
pub fn run_editor_update<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) {
    let entry = *regs;
    // The lifted body reserves 0x18 bytes and pushes EBP/ESI/EDI before its first
    // guest call, leaving ESP at entry - 0x24. Keep that frame base here. The
    // prologue's extra 0x0c local reservation is reflected in the frame base;
    // the helper out-parameter slots passed by the lifted body are entry - 0x0c
    // and entry - 0x10 (Tier 1 call-argument differential).
    regs.esp = entry.esp.wrapping_sub(0x24);
    let mut update = Update {
        m,
        guest,
        regs,
        entry_esp: entry.esp,
    };
    update.run();
    let regs = update.regs;
    regs.esp = entry.esp;
    regs.ebx = entry.ebx;
    regs.ebp = entry.ebp;
    regs.esi = entry.esi;
    regs.edi = entry.edi;
}

impl<M: GuestMemory, C: GuestCall> Update<'_, M, C> {
    fn run(&mut self) {
        let page = self.m.read32(PAGE);
        self.call(0x74840, &[page]);
        let stick_buttons = self.call(0x42760, &[0]);
        let buttons = self.call(0x427b0, &[0]);
        self.clear_flags(0x4000);
        self.clear_flags(0x400);
        let ctx = self.ctx();
        if ctx.mode_flags(self.m) & 2 != 0 && ctx.frame_flags(self.m) & 0x800 == 0 {
            self.call(0x2fc200, &[]);
            return;
        }
        if ctx.mode_flags(self.m) & 0x2000 != 0 {
            self.modal_request();
            return;
        }
        if self.flags() & frame::ALT_VIEW != 0 {
            let hint = self.m.read32(HINT_BLOCK_POINTER);
            self.m.write32(hint, 1);
            self.call(0x32f360, &[1]);
            self.call(0x32d320, &[]);
            self.call(0x32f8e0, &[]);
            return;
        }
        self.step_hint_fade();
        self.call(0x2f7ba0, &[]);
        let ctx = self.ctx();
        let flags = ctx.frame_flags(self.m);
        if flags & frame::OTHER_VIEW != 0 && flags & frame::KEEP_PICKS == 0 {
            ctx.set_pick_record(self.m, 0);
            ctx.set_pick_aux(self.m, 0);
            ctx.set_pick_saved(self.m, 0);
        }
        callees::update_hint_for_active_view(self);
        if self.must_bail() {
            self.finish_frame();
            return;
        }
        let ctx = self.ctx();
        ctx.set_tool_scratch(self.m, 0);
        if callees::idle_allows_cursor_action(self) {
            self.change_tool_mode(buttons);
        }
        self.release_modal_flag();
        if self.editing_here() {
            if self.tool_pass(stick_buttons, buttons) {
                return;
            }
        }
        if self.editing_here() {
            let (x, y) = self.pointer_words();
            let ctx = self.ctx();
            ctx.set_last_pointer_x(self.m, x);
            ctx.set_last_pointer_y(self.m, y);
        }
        self.clear_flags(3);
        let hidden = self.flags() & frame::HIDE_MASK != 0;
        if hidden || !self.pointer_on_page() || self.flags() & frame::NO_POINTER != 0 {
            self.hint_blocks();
            return;
        }
        self.scroll_and_zoom(stick_buttons);
        self.hint_blocks();
    }

    /// `ctx+0x238` bit 13: a modal request (confirm dialog) is answered by `0x3004E0(1)`.
    fn modal_request(&mut self) {
        if self.call(0x3004e0, &[1]) == 0 {
            return;
        }
        let root = self.m.read32(VIEW_WINDOW.wrapping_add(self.view_offset()));
        let window = self.call(0x77470, &[root]);
        self.call(0x78d80, &[window]);
        let ctx = self.ctx();
        let flags = ctx.mode_flags(self.m);
        ctx.set_mode_flags(self.m, flags & !0x2000);
        callees::set_cursor_mode_global(self.m, u32::MAX);
    }

    fn step_hint_fade(&mut self) {
        let ctx = self.ctx();
        let mut fade = HintFade {
            value: f32::from_bits(ctx.hint_fade(self.m)),
            frame_rate: self.m.read32(FRAME_RATE) as i32,
            scale: f32::from_bits(self.m.read32(TIME_SCALE)),
        };
        fade.step();
        ctx.set_hint_fade(self.m, fade.value.to_bits());
    }

    /// The bail-out of the "other view is active" screens.
    fn must_bail(&mut self) -> bool {
        if self.flags() & frame::BAIL_MASK != 0 {
            return true;
        }
        if self.call(0x774b0, &[]) != 0 {
            let leaf = self.call(0x774b0, &[]);
            if self.m.read8(leaf.wrapping_add(0x218)) & 4 != 0 {
                return true;
            }
        }
        self.flags() & frame::SIGN == 0 && !self.pointer_on_page()
    }

    /// Pad left/right (or the tool strip under the pointer) selects a tool mode.
    fn change_tool_mode(&mut self, buttons: u32) {
        let ctx = self.ctx();
        let current = ctx.tool_mode(self.m) as i32;
        let extended = ctx.extended_toolset(self.m) != 0;
        let (px, py) = self.pointer_words();
        let mut next = current;
        let window = self.window();
        let window_flags = self.m.read32(window.wrapping_add(0xc4));
        if window_flags & 0x100000 == 0 || self.flags() & frame::SHIFTED != 0 {
            let window = self.window();
            if self.m.read8(window.wrapping_add(200)) & 0x10 == 0 {
                next = cycle_tool_mode(current, extended, buttons);
            }
        } else {
            next = self.call(0x335890, &[px, py]) as i32;
            if next < 0 {
                let step = self.call(0x335900, &[px, py]) as i32;
                next = current;
                if step != 0 {
                    next = ring_step(current, extended, step);
                }
            }
        }
        if current == next {
            return;
        }
        let ctx = self.ctx();
        let hovered = ctx.hover_record(self.m);
        self.call(0x308560, &[hovered]);
        let ctx = self.ctx();
        ctx.set_hover_record(self.m, 0);
        ctx.set_pick_kind(self.m, u32::MAX);
        ctx.set_picked_cell(self.m, 0xffff);
        let mode = ctx.tool_mode(self.m);
        ctx.set_previous_tool_mode(self.m, mode);
        ctx.set_tool_mode(self.m, next as u32);
        self.call(0x2f79e0, &[0x157e]);
        if self.flags() as u8 & frame::SIGN as u8 != 0 {
            self.call(0x32f680, &[]);
            self.clear_flags(frame::KEEP_PICKS);
            let page = self.m.read32(PAGE);
            let ctx = self.ctx();
            let entry = self
                .m
                .read32(MODE_PAGE_TABLE.wrapping_add(ctx.tool_mode(self.m).wrapping_mul(4)));
            self.call(0x79860, &[page, entry, u32::MAX]);
        }
    }

    /// Clears the "modal tool key" bit once a basic tool has the page again.
    fn release_modal_flag(&mut self) {
        let ctx = self.ctx();
        if (ctx.tool_mode(self.m) as i32) < 5
            && self.flags() & frame::HIDE_MASK == 0
            && self.pointer_on_page()
            && ((self.call(0x427f0, &[0]) as u8 as i8) < 0
                || (self.call(0x42760, &[0]) as u8 as i8) >= 0)
            && self.flags() & 0x2000 != 0
        {
            self.clear_flags(0x2000);
        }
    }

    /// The pick and tool pass over the page. `true` when the frame ends here.
    fn tool_pass(&mut self, stick_buttons: u32, buttons: u32) -> bool {
        if stick_buttons & 0xc000 != 0 {
            self.call(0x73c80, &[]);
        }
        let ctx = self.ctx();
        if ctx.pick_kind(self.m) == u32::MAX
            && ctx.picked_cell(self.m) == -1
            && buttons != 0
            && ctx.tool_substate(self.m) == u32::MAX
        {
            if buttons & 0x200 != 0 {
                self.call(0x389b0, &[]);
                self.call(0x311930, &[]);
                return true;
            }
            if buttons & 0x100 != 0 {
                let action = self.call(0x2fc2a0, &[]);
                self.call(0x32f4c0, &[action]);
                return true;
            }
        }
        self.call(0x2fa720, &[]);
        self.call(0x303a30, &[]);
        let ctx = self.ctx();
        ctx.set_pick_record(self.m, 0);
        ctx.set_pick_aux(self.m, 0);
        ctx.set_pick_saved(self.m, 0);
        if self.flags() & frame::SHIFTED == 0 && ctx.hover_record(self.m) == 0 {
            self.pick_under_pointer();
        }
        self.tool_specific();
        self.height_step(buttons);
        self.validity_flags();
        let substate = self.ctx().tool_substate_byte(self.m);
        if substate == 0 {
            self.call(0x2fee60, &[]);
        }
        let substate = self.ctx().tool_substate_byte(self.m);
        if substate == 0 {
            self.call(0x3063c0, &[]);
        } else if substate == 1 {
            self.call(0x3159c0, &[]);
        }
        false
    }

    /// Picks the record under the pointer (`0x307E60`) and, for the placement tools, its slot.
    fn pick_under_pointer(&mut self) {
        let (wx, wy) = self.pointer_world();
        let ctx = self.ctx();
        let level = (ctx.build_level(self.m) as i32) as f32;
        let record = self.call(0x307e60, &[wx.to_bits(), wy.to_bits(), level.to_bits()]);
        ctx.set_pick_record(self.m, record);
        let mode = ctx.tool_mode(self.m) as i32;
        let substate = ctx.tool_substate(self.m) as i32;
        let slot_tool = mode == 1 || (mode == 4 && matches!(substate, 3 | 4 | 5 | -1));
        if !slot_tool {
            return;
        }
        let (slot_c, slot_10) = (self.slot_c(), self.slot_10());
        self.m.write32(slot_c, 0);
        self.m.write32(slot_10, 0);
        let ctx = self.ctx();
        if kind_is_placement(self.m, ctx.pick_kind(self.m)) && ctx.pick_record(self.m) != 0 {
            let slot = self.call(0x2f8450, &[ctx.pick_record(self.m), slot_c, slot_10]);
            ctx.set_pick_slot(self.m, slot);
            if (slot as i32) > -1 {
                let (c, d) = (self.m.read32(slot_c), self.m.read32(slot_10));
                self.call(
                    0x2f8a20,
                    &[ctx.pick_record(self.m), slot, c, d, slot_10, slot_c],
                );
                if (self.m.read32(slot_c) as i32) > -1 {
                    let record = ctx.pick_record(self.m);
                    ctx.set_pick_saved(self.m, record);
                    ctx.set_pick_record(self.m, 0);
                }
                return;
            }
        }
        if (ctx.pick_kind(self.m) as i32) < 0 && ctx.pick_record(self.m) != 0 {
            let group = self.call(0x2f8610, &[]);
            ctx.set_pick_group(self.m, group);
        } else {
            ctx.set_pick_group(self.m, u32::MAX);
        }
    }

    /// Tool specific handling for the object (0), height (2) and other tools.
    fn tool_specific(&mut self) {
        let mode = self.ctx().tool_mode(self.m);
        if mode == 0 || mode == 2 {
            let window = self.window();
            if self.m.read8(window.wrapping_add(0xc4)) & 0x10 != 0 {
                let ctx = self.ctx();
                let flags = ctx.mode_flags(self.m);
                ctx.set_mode_flags(self.m, flags & !0x4000);
            }
            let window = self.window();
            if self.m.read32(window.wrapping_add(200)) & 0x100000 != 0 {
                if self.flags() & frame::SHIFTED == 0 {
                    if self.ctx().mode_flags(self.m) & 0x4000 == 0 {
                        self.call(0x2fbf10, &[]);
                    }
                } else {
                    // T2432: native Rust port of 0x2FEB50.
                    callees::count_selected_objects(self.m);
                    self.clear_flags(frame::SHIFTED);
                }
            }
            let window = self.window();
            let ctx = self.ctx();
            if (self.m.read8(window.wrapping_add(0xc4)) as i8) < 0
                && ctx.pick_record(self.m) == 0
                && ctx.drag_source(self.m) != 0
            {
                // T2432: native Rust port of 0x2F92E0.
                callees::clear_link_marks(self.m);
            }
            let ctx = self.ctx();
            let mode = ctx.tool_mode(self.m);
            let window = if mode == 0 || mode == 2 {
                self.window()
            } else {
                0
            };
            if (mode == 0 || mode == 2) && self.m.read8(window.wrapping_add(0xc4)) & 0x10 != 0 {
                let (wx, wy) = self.pointer_world();
                ctx.set_anchor_x(self.m, wx.to_bits());
                ctx.set_anchor_y(self.m, wy.to_bits());
            }
            let window = self.window();
            let ctx = self.ctx();
            if self.m.read8(window.wrapping_add(200)) & 0x10 != 0
                && ctx.hover_record(self.m) == 0
                && ctx.mode_flags(self.m) & 0x4000 == 0
            {
                self.call(0x2f9040, &[]);
            }
        } else {
            let window = self.window();
            if self.m.read32(window.wrapping_add(200)) & 0x100000 != 0 {
                self.call(0x2fbf10, &[]);
            }
        }
    }

    /// Build height steps (`+0x264`) with their sounds.
    fn height_step(&mut self, buttons: u32) {
        let ctx = self.ctx();
        let step = step_height(
            ctx.build_level(self.m) as i32,
            ctx.extended_toolset(self.m) != 0,
            buttons,
        );
        if let Some(step) = step {
            ctx.set_build_level(self.m, step.level as u32);
            if let Some(below) = step.below {
                ctx.set_build_level_below(self.m, below as u32);
            }
            self.call(0x2f79e0, &[step.sound]);
        }
    }

    /// Computes the "hover valid" frame bit (4) for the tool modes that use a cursor cell.
    fn validity_flags(&mut self) {
        self.clear_flags(frame::HOVER_VALID);
        let ctx = self.ctx();
        if ctx.tool_mode(self.m) == 0
            && ctx.hover_record(self.m) != 0
            && self.call(0x2f8de0, &[]) != 0
        {
            self.set_flags(frame::HOVER_VALID);
        }
        let ctx = self.ctx();
        self.call(0x331be0, &[ctx.base.wrapping_add(0x2f2e)]);
        let mode = self.ctx().tool_mode(self.m);
        if mode == 1 || mode == 4 {
            if self.hover_valid_object_tools() {
                self.set_flags(frame::HOVER_VALID);
            }
        }
        if self.ctx().tool_mode(self.m) == 3 && self.hover_valid_link_tool() {
            self.set_flags(frame::HOVER_VALID);
        }
    }

    fn hover_valid_object_tools(&mut self) -> bool {
        let ctx = self.ctx();
        ctx.set_pick_aux(self.m, 0);
        self.call(0x331ab0, &[ctx.base.wrapping_add(0x2f28)]);
        let ctx = self.ctx();
        let cell = ctx.offset_cell(self.m);
        if ctx.pick_record(self.m) != 0 {
            let link = self.call(0x2f82e0, &[]);
            ctx.set_pick_aux(self.m, link);
        }
        let mut valid = false;
        if ctx.tool_mode(self.m) == 4 {
            let substate = ctx.tool_substate(self.m) as i32;
            if substate == -1 {
                valid = true;
            } else if substate == 2 || substate == 5 {
                let link = ctx.pick_aux(self.m);
                let mut set = false;
                if link != 0 {
                    let kind = self.m.read32(link);
                    set = self.call(0x31ff30, &[kind]) != 0;
                }
                if !set {
                    set = (ctx.pick_group(self.m) as i32) >= 0;
                }
                valid = set;
            }
            let substate = ctx.tool_substate(self.m) as i32;
            if substate == 6 || substate == 7 {
                return valid || ctx.pick_record(self.m) != 0;
            }
        }
        let generic = self.hover_cell_rules(cell);
        valid || generic
    }

    /// The generic hover rules of the object tools; `true` when the hover bit is set.
    fn hover_cell_rules(&mut self, cell: Cell) -> bool {
        let ctx = self.ctx();
        let picked = ctx.picked_cell(self.m);
        let kind = ctx.pick_kind(self.m) as i32;
        if picked < 0 {
            if ctx.pick_record(self.m) != 0 || kind >= 0 {
                if kind < 0 {
                    if self.call_cell(0x3348c0, None, cell) as u8 == 0 {
                        return self.link_rule(cell);
                    }
                    return true;
                }
                let valid = self.call_cell(0x3348c0, None, cell) as u8;
                if valid == 0 || ctx.pick_aux(self.m) != 0 {
                    return self.cell_rule(cell);
                }
                return true;
            }
            return true;
        }
        self.cell_rule(cell)
    }

    fn cell_rule(&mut self, cell: Cell) -> bool {
        if (self.ctx().pick_kind(self.m) as i32) < 0 {
            self.link_rule(cell)
        } else {
            self.placement_rule()
        }
    }

    fn link_rule(&mut self, cell: Cell) -> bool {
        let ctx = self.ctx();
        if ctx.pick_record(self.m) != 0 || ctx.picked_cell(self.m) >= 0 {
            if ctx.picked_cell(self.m) < 0 {
                if self.call_cell(0x3348c0, None, cell) as u8 != 0 {
                    return true;
                }
                if self.ctx().picked_cell(self.m) < 0 {
                    return self.placement_rule();
                }
            }
            if self.call_cell(0x3348c0, None, cell) as u8 == 0 {
                return self.placement_rule();
            }
        }
        true
    }

    /// Hover stays invalid unless a placement kind has a saved pick and a slot.
    fn placement_rule(&mut self) -> bool {
        let ctx = self.ctx();
        let kind = ctx.pick_kind(self.m) as i32;
        !(kind < 0
            || !kind_is_placement(self.m, kind as u32)
            || ctx.pick_saved(self.m) == 0
            || (ctx.pick_slot(self.m) as i32) < 0)
    }

    fn hover_valid_link_tool(&mut self) -> bool {
        let ctx = self.ctx();
        ctx.set_pick_cell_link(self.m, u32::MAX);
        self.call(0x331ab0, &[ctx.base.wrapping_add(0x2f28)]);
        let ctx = self.ctx();
        let cell = ctx.offset_cell(self.m);
        if ctx.pick_record(self.m) != 0 {
            let raw = ctx.raw_cell(self.m);
            let link = self.call_cell(0x2f8b40, Some(ctx.pick_record(self.m)), raw);
            ctx.set_pick_cell_link(self.m, link);
        }
        if ctx.pick_record(self.m) != 0 || ctx.picked_cell(self.m) >= 0 {
            if ctx.picked_cell(self.m) < 0 {
                if self.call_cell(0x3348c0, None, cell) as u8 != 0 {
                    return true;
                }
                if ctx.picked_cell(self.m) < 0 {
                    return false;
                }
            }
            let valid = self.call_cell(0x3348c0, None, cell) as u8;
            return !(valid == 0 || (ctx.pick_cell_link(self.m) as i32) >= 0);
        }
        true
    }

    /// Stick scroll, zoom on the shoulder buttons, then the view tail `0x2F9F70`.
    fn scroll_and_zoom(&mut self, stick_buttons: u32) {
        let view_buttons = self.m.read8(VIEW_BUTTONS.wrapping_add(self.view_offset()));
        let control = self.regs.fpu.control;
        let (stick_x, stick_y);
        if view_buttons & 2 == 0 {
            self.call(0x42920, &[0, 0x3f00_0000]);
            let a = self.call_st0(0x42970, &[0, 3]);
            stick_x = round_pc(a + a, control) as f32;
            let b = self.call_st0(0x42970, &[0, 4]);
            stick_y = round_pc(b + b, control) as f32;
            self.call(0x42920, &[0, 0x3e80_0000]);
        } else {
            let a = self.call_st0(0x42970, &[0, 1]);
            stick_x = round_pc(a * f64::from(1.5_f32), control) as f32;
            let b = self.call_st0(0x42970, &[0, 2]);
            stick_y = round_pc(b * f64::from(1.5_f32), control) as f32;
        }
        let dx = stick_x * 0.1_f32;
        let dy = stick_y * 0.1_f32;
        if !(dx == 0.0 && dy == 0.0) {
            self.call(0x309160, &[dx.to_bits(), (0.0_f32 - dy).to_bits()]);
            self.set_flags(frame::SCROLLED);
            if let Some(facing) = scroll_facing(dx, dy) {
                self.m.write8(SCROLL_FACING, facing as u8);
            }
        }
        let factor = if ((stick_buttons >> 8) as u8 as i8) < 0 {
            Some(1.02_f32)
        } else if stick_buttons & 0x4000 != 0 {
            Some(0.98_f32)
        } else {
            None
        };
        if let Some(factor) = factor {
            self.set_flags(frame::ZOOM_OR_SCROLL);
            self.zoom(factor);
        }
        self.call(0x2f9f70, &[]);
    }

    fn zoom(&mut self, factor: f32) {
        let (pointer_x, pointer_y) = self.pointer_world();
        let m = &mut *self.m;
        let read = |m: &M, address: u32| f32::from_bits(m.read32(address));
        let zoom = Zoom {
            scale_x: read(m, 0x0078_3f2c),
            scale_y: read(m, 0x0078_3f28),
            width: m.read32(0x0078_3de4) as i32 as f32,
            height: m.read32(0x0078_3ccc) as i32 as f32,
            extent_x: read(m, 0x0078_3de0),
            extent_y: read(m, 0x0078_3ddc),
            left: read(m, 0x0078_3dd8),
            top: read(m, 0x0078_3cc0),
        };
        let result = zoom.step(factor, pointer_x, pointer_y);
        let writes = [
            (0x0078_3f2c, result.zoom.scale_x),
            (0x0078_3f28, result.zoom.scale_y),
            (0x0078_3dd4, result.inverse_x),
            (0x0078_3de0, result.zoom.extent_x),
            (0x0078_3f20, result.inverse_y),
            (0x0078_3ddc, result.zoom.extent_y),
            (0x0078_3dd8, result.zoom.left),
            (0x0078_3cc0, result.zoom.top),
            (0x0078_3dc0, result.right),
            (0x0078_3dcc, result.bottom),
        ];
        for (address, value) in writes {
            m.write32(address, value.to_bits());
        }
        self.call(0x71140, &[result.pan.0.to_bits(), result.pan.1.to_bits()]);
    }

    /// Hint panel for the picked record (`+0x2F18`), else for the hovered record (`+0x2F7C`), then the
    /// frame tail.
    fn hint_blocks(&mut self) {
        let ctx = self.ctx();
        if self.editing_here() && ctx.pick_record(self.m) != 0 {
            let record = ctx.pick_record(self.m);
            let angle = self.record_angle(record);
            self.draw_hint_header();
            let sprite = self.record_sprite(ctx.pick_record(self.m));
            let hint = self.m.read32(HINT_BLOCK_POINTER);
            let changed = self.m.read32(hint.wrapping_add(HINT_SPRITE)) != sprite
                || f32::from_bits(self.m.read32(hint.wrapping_add(HINT_ANGLE))) != angle;
            if changed {
                self.call(0x32d440, &[sprite, angle.to_bits()]);
                self.set_hint_state(8);
            }
            let ctx = self.ctx();
            let level = (ctx.build_level(self.m) as i32) as f32;
            let height = f32::from_bits(self.m.read32(ctx.pick_record(self.m).wrapping_add(8)));
            ctx.set_pick_height(self.m, sse::cvttss2si(level - height));
            self.announce_record(ctx.pick_record(self.m));
            self.finish_frame();
            return;
        }
        let ctx = self.ctx();
        let hovered = ctx.hover_record(self.m);
        if self.editing_here() && hovered != 0 && self.m.read32(hovered.wrapping_add(0x40)) == 0 {
            let angle = self.record_angle(hovered);
            self.draw_hint_header();
            let sprite = self.record_sprite(ctx.hover_record(self.m));
            self.call(0x32d440, &[sprite, angle.to_bits()]);
            self.set_hint_state(0x20);
            self.announce_record(ctx.hover_record(self.m));
        }
        self.finish_frame();
    }

    /// Heading of a record in degrees (0 to 360) from the angle table.
    fn record_angle(&self, record: u32) -> f32 {
        let index = self.m.read32(record.wrapping_add(0xc));
        let table = f32::from_bits(
            self.m
                .read32(ANGLE_TABLE.wrapping_add(index.wrapping_mul(4))),
        );
        let mut angle = 0.0_f32 - table;
        if angle < 0.0 {
            angle += 360.0;
        }
        angle
    }

    /// Sprite index of a record: its sprite pointer (`+0x2C`) relative to the table, in `0x140` steps.
    fn record_sprite(&self, record: u32) -> u32 {
        let pointer = self.m.read32(record.wrapping_add(0x2c));
        ((pointer.wrapping_sub(SPRITE_TABLE) as i32) / 0x140) as u32
    }

    /// Hint block state word: `base` then `| 0x10` (two writes, like the original).
    fn set_hint_state(&mut self, base: u32) {
        let hint = self.m.read32(HINT_BLOCK_POINTER);
        self.m.write32(hint.wrapping_add(HINT_STATE), base);
        let hint = self.m.read32(HINT_BLOCK_POINTER);
        let value = self.m.read32(hint.wrapping_add(HINT_STATE));
        self.m.write32(hint.wrapping_add(HINT_STATE), value | 0x10);
    }

    /// Shows the sprite name panel once per record (`0x32CFE0`) and remembers the record.
    fn announce_record(&mut self, record: u32) {
        let hint = self.m.read32(HINT_BLOCK_POINTER);
        let shown = self.m.read32(hint.wrapping_add(HINT_RECORD));
        if shown == 0 || shown != record {
            let sprite = self.record_sprite(record);
            self.call(0x32cfe0, &[sprite]);
        }
        let hint = self.m.read32(HINT_BLOCK_POINTER);
        self.m.write32(hint.wrapping_add(HINT_RECORD), record);
    }

    /// Sets the hint block active flag and lays the panel out under the toolbar
    /// (`0x22B90` width, `0x22B40` x, `0x32D1A0` anchor).
    fn draw_hint_header(&mut self) {
        let hint = self.m.read32(HINT_BLOCK_POINTER);
        self.m.write32(hint, 1);
        let scale = f32::from_bits(self.m.read32(UI_SCALE));
        let margin = f32::from_bits(self.m.read32(UI_MARGIN));
        let width = self.m.read32(SCREEN_WIDTH) as i32;
        let height = self.m.read32(SCREEN_HEIGHT) as i32;
        let box_size = scale * 22.0;
        let box_height = (box_size * width as f32) / height as f32;
        let measured = self.call(
            0x22b90,
            &[
                sse::cvttss2si(box_size),
                sse::cvttss2si((width as f32 * box_size) / height as f32),
            ],
        ) as i32;
        let left = (width.wrapping_sub(measured) as f32 - margin * 8.0) - box_height;
        let x = self.call(0x22b40, &[sse::cvttss2si(left + box_height * 0.5)]) as i32;
        self.call(
            0x32d1a0,
            &[sse::cvttss2si(
                (x as f32 + scale * 5.0) + (scale * 22.0) * 0.5,
            )],
        );
    }
}

/// Wraps a tool index after a table step the way the tool strip does (`ring_step`).
fn ring_step(current: i32, extended: bool, step: i32) -> i32 {
    let base = i32::from(extended) * 2;
    let next = current.wrapping_add(step);
    if next < 0 {
        base + 2
    } else if base + 3 <= next {
        0
    } else {
        next
    }
}

fn kind_is_placement<M: GuestMemory>(m: &M, kind: u32) -> bool {
    let address = KIND_TABLE.wrapping_add(kind.wrapping_mul(KIND_STRIDE));
    m.read16(address) == KIND_PLACEMENT
}
