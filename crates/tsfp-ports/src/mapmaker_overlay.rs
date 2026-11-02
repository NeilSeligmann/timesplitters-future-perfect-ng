// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/editor
//! T2418/T2420: Rust-owned Map Maker editor overlay alpha ramp, hint label anchors and editor frame
//! draw (Tier 1, fabricated inputs, weak).
//!
//! T2420 adds `0x30ACB0` (hint anchor right of a measured label, [`anchor_after_label`]) and `0x3111F0`
//! (editor frame draw, [`run_editor_frame`]). The x87 helpers are results in `ST0`: the text measure
//! `0x405C0` (width) / `0x40620` (height), both thin wrappers over `0x3FCF0`, are read back as `f64`
//! at the adapter boundary ([`guest_text_metric`]); the float-to-int helper `0x3C848C` is modelled by
//! [`ftol_low32`] (truncation, INFERRED from its fix-up code) and is not called.
//!
//! | VA | role | ABI |
//! |---|---|---|
//! | `0x2FB4C0` | per-frame editor update: refresh the widget tree, step the HUD panels, ramp the overlay alpha at `0x7844AC` | none (cdecl, void) |
//! | `0x30AD30` | shift a hint label anchor by the pointer-lerp offset (no text measure) | cdecl, 4 stack words |
//!
//! The alpha ramp is a typed [`OverlayRamp`] (alpha, frames per second, time scale) with a readable
//! [`OverlayRamp::step`]: it moves toward 1.0 (overlay shown) or 0.0 (hidden) by `scale / fps` per
//! frame and clamps. Whether the overlay is shown depends on three guest queries (the current leaf
//! widget of the viewport via `0x774B0`) which stay guest calls; the decision (leaf exists, not flagged hidden, not the page itself, context bit 15 clear) is
//! inline in [`run_overlay_update`]. `0x7A7B0` (widget update) and `0x3356A0` (HUD slides, itself a lane
//! root) are still called through the guest. `0x30AD30` is pure arithmetic ([`shift_anchor`]).
//!
//! Float rounding follows the original: SSE single precision, `cvttss2si` for the integer
//! conversion (out of range gives `0x80000000`). Constants are read from the running image, never
//! committed. Scratch registers ECX, EDX and XMM0-XMM2 are not reproduced (the callers treat both
//! as void); EAX of `0x2FB4C0` is.
//!
//! A/B: lane roots (`--rust-ports`, group `modes`), enabled by default; set
//! `TSFP_MAPMAKER_OVERLAY=off` to retain the lifted originals.
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    inventory::sse,
    memory::GuestMemory,
    system_slice::round_pc,
};

/// Pointer to the flag word that `0x2FB4C0` clears first.
pub const FLAG_POINTER: u32 = 0x0077_8d34;
/// The current widget page handed to `0x7A7B0` and compared against the leaf widget.
pub const PAGE: u32 = 0x0078_42a8;
/// Context record; `+0x238` bit 15 hides the overlay.
pub const CONTEXT_POINTER: u32 = 0x0078_44a8;
/// The overlay alpha.
pub const OVERLAY_ALPHA: u32 = 0x0078_44ac;
/// Leaf widget flag `+0x218` bit 0 hides the overlay.
pub const LEAF_FLAGS: u32 = 0x218;
/// Context word whose bit 15 hides the overlay.
pub const CONTEXT_FLAGS: u32 = 0x238;

fn read_f32<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

/// The overlay alpha with the frame values that move it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayRamp {
    /// `0x7844AC`: current alpha.
    pub alpha: f32,
    /// `0x4E8CEC` (i32): frames per second.
    pub frame_rate: i32,
    /// `0x475C78`: one.
    pub one: f32,
    /// `0x475D10`: ramp time scale.
    pub scale: f32,
}

impl OverlayRamp {
    pub fn read<M: GuestMemory>(m: &M) -> Self {
        Self {
            alpha: read_f32(m, OVERLAY_ALPHA),
            frame_rate: m.read32(0x004e_8cec) as i32,
            one: read_f32(m, 0x0047_5c78),
            scale: read_f32(m, 0x0047_5d10),
        }
    }

    /// Per-frame delta `one / fps * scale`, in the original operation order.
    fn delta(&self) -> f32 {
        (self.one / (self.frame_rate as f32)) * self.scale
    }

    /// One frame of the ramp: up when shown (clamped to `one`), down when hidden (clamped to 0).
    /// NaN is never clamped, like the original `COMISS`/`JBE` pair.
    pub fn step(&mut self, shown: bool) {
        let delta = self.delta();
        if shown {
            let next = delta + self.alpha;
            self.alpha = if next <= self.one || next.is_nan() {
                next
            } else {
                self.one
            };
        } else {
            let next = self.alpha - delta;
            self.alpha = if 0.0 <= next || next.is_nan() {
                next
            } else {
                0.0
            };
        }
    }
}

/// `0x2FB4C0`. Returns the original EAX.
pub fn run_overlay_update<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> u32 {
    let flag = m.read32(FLAG_POINTER);
    m.write32(flag, 0);
    let page = m.read32(PAGE);
    call_cdecl(guest, m, regs, 0x0007_a7b0, &[page]);
    guest.call(0x0033_56a0, regs);
    guest.call(0x0007_74b0, regs);
    let mut eax = regs.eax;
    let mut shown = false;
    if eax != 0 {
        guest.call(0x0007_74b0, regs);
        eax = regs.eax;
        let flags = m.read8(eax.wrapping_add(LEAF_FLAGS));
        if flags & 1 == 0 {
            guest.call(0x0007_74b0, regs);
            eax = regs.eax;
            if eax != m.read32(PAGE) {
                let context = m.read32(CONTEXT_POINTER);
                eax = m.read32(context.wrapping_add(CONTEXT_FLAGS));
                // bit 15 of the context word hides the overlay (`TEST AH, AH; JS`)
                shown = (eax >> 8) as u8 & 0x80 == 0;
            }
        }
    }
    let mut ramp = OverlayRamp::read(m);
    ramp.step(shown);
    m.write32(OVERLAY_ALPHA, ramp.alpha.to_bits());
    regs.eax = eax;
    eax
}

/// Pointer-lerp constants of the anchor shift.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnchorShift {
    /// `0x7BA188` times `0x4785E4`, then the bias `0x47DB00` added.
    pub x_lerp: f32,
    pub x_scale: f32,
    pub x_bias: f32,
    /// `0x7BA038` times `0x4760CC` times `0x475CD4`.
    pub y_lerp: f32,
    pub y_scale_a: f32,
    pub y_scale_b: f32,
}

impl AnchorShift {
    pub fn read<M: GuestMemory>(m: &M) -> Self {
        Self {
            x_lerp: read_f32(m, 0x007b_a188),
            x_scale: read_f32(m, 0x0047_85e4),
            x_bias: read_f32(m, 0x0047_db00),
            y_lerp: read_f32(m, 0x007b_a038),
            y_scale_a: read_f32(m, 0x0047_60cc),
            y_scale_b: read_f32(m, 0x0047_5cd4),
        }
    }
}

/// `0x30AD30`: returns the shifted `(x, y)` from the incoming x and the y offset word. The original
/// reads the x in place before writing either output.
pub fn shift_anchor(shift: &AnchorShift, x: i32, y_offset: i32) -> (i32, i32) {
    let new_x = sse::cvttss2si((shift.x_lerp * shift.x_scale + x as f32) + shift.x_bias) as i32;
    let new_y =
        sse::cvttss2si(shift.y_lerp * shift.y_scale_a * shift.y_scale_b + y_offset as f32) as i32;
    (new_x, new_y)
}

/// `0x30AD30` on guest memory: arguments at `esp+4` (y out), `esp+8` (x in/out), `esp+0x10` (y offset).
pub fn run_anchor_shift<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let y_out = m.read32(regs.esp.wrapping_add(4));
    let x_io = m.read32(regs.esp.wrapping_add(8));
    let y_offset = m.read32(regs.esp.wrapping_add(0x10)) as i32;
    let shift = AnchorShift::read(m);
    let x = m.read32(x_io) as i32;
    let (new_x, new_y) = shift_anchor(&shift, x, y_offset);
    m.write32(x_io, new_x as u32);
    m.write32(y_out, new_y as u32);
    regs.eax = y_out;
    regs.ecx = new_x as u32;
    regs.edx = new_y as u32;
}

// ---------------------------------------------------------------------------------------------
// T2420: 0x30ACB0, hint anchor right of a measured label
// ---------------------------------------------------------------------------------------------

/// The label width term of `0x30ACB0` is added only when this word is `1`.
pub const MEASURE_LABEL_MODE: u32 = 0x007e_7260;

/// Constants and pointer-lerp state of the label anchor. Floats are the stored `f32` values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelAnchor {
    /// `[0x7E7260] == 1`: add the measured label width to the x anchor.
    pub measure_width: bool,
    /// `0x7BA188` (pointer lerp) times `0x4785E4`.
    pub x_lerp: f32,
    pub x_scale: f32,
    /// `0x47DB00`: the y bias the measured height is subtracted from.
    pub y_bias: f32,
    /// `0x475CD4`: scale applied to both measured extents.
    pub text_scale: f32,
    /// x87 control word (24 bit precision in the live game).
    pub control: u16,
}

impl LabelAnchor {
    pub fn read<M: GuestMemory>(m: &M, control: u16) -> Self {
        Self {
            measure_width: m.read32(MEASURE_LABEL_MODE) == 1,
            x_lerp: read_f32(m, 0x007b_a188),
            x_scale: read_f32(m, 0x0047_85e4),
            y_bias: read_f32(m, 0x0047_db00),
            text_scale: read_f32(m, 0x0047_5cd4),
            control,
        }
    }

    /// Scaled label width, stored as `f32` (`FMUL` then `FSTP float`). `0` when width is not measured.
    pub fn scaled_width(&self, measured_width: f64) -> f32 {
        if self.measure_width {
            round_pc(measured_width * self.text_scale as f64, self.control) as f32
        } else {
            0.0
        }
    }

    /// Shifted x: `lerp * scale + (x_offset + 0x18) + width`, truncated like `cvttss2si`.
    pub fn x(&self, scaled_width: f32, x_offset: i32) -> u32 {
        let margin = x_offset.wrapping_add(0x18) as f32;
        let base = sse::mulss(self.x_lerp, self.x_scale);
        sse::cvttss2si(sse::addss(sse::addss(base, margin), scaled_width))
    }

    /// Shifted y: `(bias - height) * text_scale + y_in` on the x87 stack, then the float-to-int helper.
    pub fn y(&self, measured_height: f64, y_in: i32) -> u32 {
        let c = self.control;
        let gap = round_pc(self.y_bias as f64 - measured_height, c);
        let scaled = round_pc(gap * self.text_scale as f64, c);
        ftol_low32(round_pc(scaled + y_in as f64, c))
    }
}

/// `0x3C848C` (float to int64, low dword in EAX): truncation toward zero. INFERRED from the fix-up
/// code that corrects `FISTP` rounding; out of range values keep the low 32 bits of the int64.
pub fn ftol_low32(value: f64) -> u32 {
    // `as` truncates toward zero and maps NaN to 0 (no std `trunc` in the no_std archive)
    (value as i64) as u32
}

/// Text extent from the guest measure helpers (`0x405C0` width, `0x40620` height; both wrap
/// `0x3FCF0`), read from `ST0`. The ABI (one stack argument, result on the x87 stack) stays here at
/// the adapter boundary.
pub fn guest_text_metric<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    helper: u32,
    text: u32,
) -> f64 {
    call_cdecl(guest, m, regs, helper, &[text]);
    let value = regs.fpu.st(0);
    regs.fpu.pop();
    value
}

/// `0x30ACB0` on guest memory: cdecl `(x_out, y_io, text, x_offset)`. Writes `x_out` before the
/// height is measured and `y_io` last. EAX is the y result, EDX the x pointer, ECX and ESI keep their
/// entry values.
pub fn run_label_anchor<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) {
    let entry = *regs;
    let x_out = m.read32(entry.esp.wrapping_add(4));
    let y_io = m.read32(entry.esp.wrapping_add(8));
    let text = m.read32(entry.esp.wrapping_add(0xc));
    let x_offset = m.read32(entry.esp.wrapping_add(0x10)) as i32;
    let anchor = LabelAnchor::read(m, regs.fpu.control);
    let width = if anchor.measure_width {
        guest_text_metric(m, guest, regs, 0x405c0, text)
    } else {
        0.0
    };
    let x = anchor.x(anchor.scaled_width(width), x_offset);
    m.write32(x_out, x);
    let height = guest_text_metric(m, guest, regs, 0x40620, text);
    let y = anchor.y(height, m.read32(y_io) as i32);
    m.write32(y_io, y);
    regs.eax = y;
    regs.ecx = entry.ecx;
    regs.edx = x_out;
    regs.esi = entry.esi;
    regs.esp = entry.esp;
}

// ---------------------------------------------------------------------------------------------
// T2420: 0x3111F0, editor frame draw
// ---------------------------------------------------------------------------------------------

/// Editor context record (`[0x7844A8]`) fields used by the frame draw (INFERRED names).
mod context {
    /// Word whose bit 15 skips the world pass (`TEST CH, CH; JS`).
    pub const HIDE_FLAGS: u32 = 0x238;
    /// Tool mode: 0 none, 1 and 4 pick from the record list, 2 selection details.
    pub const MODE: u32 = 0x254;
    /// State flags, see [`ALT_VIEW`], [`FORCE_HINT`], [`FOCUS_FLAGS`].
    pub const FLAGS: u32 = 0x25c;
    /// Selected widget record: `+0x10` icon id, `+0x1c` kind, `+0x2c` then `+8` name id.
    pub const SELECTION: u32 = 0x2f18;
    pub const SELECTION_RECORDS: u32 = 0x2f1c;
    pub const SLOT: u32 = 0x2f78;
    pub const ALT_SELECTION: u32 = 0x2f7c;
    /// Slot array, stride 12: `+0` label, `+7` kind byte, `+0xa` object id (i16).
    pub const SLOTS: u32 = 0x3070;
    /// Object name table, stride `0x8c`.
    pub const NAME_TABLE: u32 = 0xdd14;
    /// Alternate view: draws the focused widget only.
    pub const ALT_VIEW: u32 = 0x0200_0000;
    /// Forces the hint pass without the focus test.
    pub const FORCE_HINT: u32 = 0x0400_0000;
    /// Flags that make a focused widget a hint target.
    pub const FOCUS_FLAGS: u32 = 0x0004_4088;
}

const KIND_RECORDS: u32 = 0x0052_4f58;
const KIND_STRIDE: u32 = 0xc4;
const KIND_EXTRA_FLAGS: u32 = 0x0052_5010;
const ICON_TABLE: u32 = 0x0052_9514;
const KIND_NAME_TABLE: u32 = 0x0052_cdf0;
const ACTIVE_VIEW: u32 = 0x007b_a9e0;
const VIEW_STRIDE: u32 = 0x1e0;

/// Integer corners of the hint box handed to `0x30CCD0` (`cvttss2si` of the float results).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HintBox {
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub e: u32,
}

/// Hint label state resolved before the box is drawn.
struct HintLabel {
    /// Label text pointer (the original ESI), `0` for none.
    text: u32,
    /// Record list entry (the original EBX).
    record: u32,
    /// The label came from the slot array (the original EBP).
    from_slot: bool,
    /// Kind index passed to `0x3357B0`, `-1` when none.
    index: u32,
    /// Extra text appended to the label, `0` for none.
    extra: u32,
    /// Fourth argument of `0x3357B0`.
    marker: u32,
}

struct Frame<'a, M, C> {
    m: &'a mut M,
    guest: &'a mut C,
    regs: &'a mut ExtRegisters,
}

impl<M: GuestMemory, C: GuestCall> Frame<'_, M, C> {
    fn call(&mut self, va: u32, args: &[u32]) -> u32 {
        call_cdecl(self.guest, self.m, self.regs, va, args);
        self.regs.eax
    }

    fn context(&self) -> u32 {
        self.m.read32(CONTEXT_POINTER)
    }

    fn field(&self, offset: u32) -> u32 {
        self.m.read32(self.context().wrapping_add(offset))
    }

    fn flags(&self) -> u32 {
        self.field(context::FLAGS)
    }

    /// `0x77470` of the active view's widget id.
    fn view_widget(&mut self) -> u32 {
        let view = self.m.read32(ACTIVE_VIEW).wrapping_mul(VIEW_STRIDE);
        let id = self.m.read32(view.wrapping_add(0x007b_a26c));
        self.call(0x0007_7470, &[id])
    }

    /// `0x774B0`: current leaf widget.
    fn leaf(&mut self) -> u32 {
        self.call(0x0007_74b0, &[])
    }

    /// Name text of the picked slot or record. The label stays empty when nothing is picked.
    fn resolve_label(&mut self, mode: u32) -> HintLabel {
        let mut label = HintLabel {
            text: 0,
            record: 0,
            from_slot: false,
            index: u32::MAX,
            extra: 0,
            marker: 0,
        };
        if mode == 2 {
            let selection = self.field(context::SELECTION);
            if selection == 0 {
                return label;
            }
            let kind = self.m.read32(selection.wrapping_add(0x1c));
            if (kind == 3 || kind == 2) && self.flags() & context::FORCE_HINT == 0 {
                label.marker = 1;
            }
            let table = self
                .m
                .read32(KIND_NAME_TABLE.wrapping_add(kind.wrapping_mul(4)));
            let kind_name = self.call(0x0003_d7b0, &[table]);
            let title = self.call(0x0003_d7b0, &[0xa80]);
            label.text = self.call(0x0015_1030, &[0x0047_8904, title, kind_name]);
            return label;
        }
        if mode != 1 && mode != 4 {
            return label;
        }
        label.record = self.call(0x002f_82e0, &[]);
        if self.flags() & context::FORCE_HINT != 0 {
            label.record = self.field(context::SELECTION_RECORDS);
        }
        let slot = self.field(context::SLOT) as i32;
        // `Some(id)`: the object name table entry; `None`: the kind name through `0x3D7B0`
        let object: Option<u32>;
        if slot > -1 {
            let base = self.context().wrapping_add((slot as u32).wrapping_mul(12));
            let kind = self.m.read8(base.wrapping_add(context::SLOTS + 7)) as u32;
            let found = self.call(0x002f_8020, &[kind]);
            label.text = found.wrapping_mul(KIND_STRIDE).wrapping_add(KIND_RECORDS);
            label.index = found;
            label.from_slot = true;
            let id = self.m.read16(base.wrapping_add(context::SLOTS + 0xa));
            let id_wide = id as i16 as i32 as u32;
            object = if id == 0xffff || self.call(0x0031_dbc0, &[id_wide]) != 0 {
                None
            } else {
                Some(id_wide)
            };
        } else {
            if self.field(context::SELECTION) == 0 || label.record == 0 {
                return label;
            }
            let entry = self.m.read32(label.record);
            label.text = entry.wrapping_mul(KIND_STRIDE).wrapping_add(KIND_RECORDS);
            label.index = entry;
            if self.m.read32(label.text.wrapping_add(0xb8)) & 0x1000 != 0 {
                label.extra = self.call(0x0030_3350, &[entry]);
            }
            let id = self.m.read32(label.record.wrapping_add(4));
            object = if id == u32::MAX || self.call(0x0031_dbc0, &[id]) != 0 {
                None
            } else {
                Some(id)
            };
        }
        match object {
            None => {
                let kind_name = self.m.read16(label.text) as i16 as i32 as u32;
                label.text = self.call(0x0003_d7b0, &[kind_name]);
            }
            Some(id) => {
                label.text = self
                    .context()
                    .wrapping_add(id.wrapping_mul(0x8c))
                    .wrapping_add(context::NAME_TABLE);
            }
        }
        if label.text != 0
            && label.record != 0
            && !label.from_slot
            && self.field(context::MODE) != 2
        {
            let entry = self.m.read32(label.record);
            let flags = self.m.read32(
                entry
                    .wrapping_mul(KIND_STRIDE)
                    .wrapping_add(KIND_EXTRA_FLAGS),
            );
            if flags & 0x1000 != 0 {
                label.extra = self.call(0x0030_3350, &[entry]);
            }
            if label.extra != 0 {
                label.text = self.call(0x0015_1030, &[0x0049_f7c8, label.text, label.extra]);
            }
        }
        label
    }

    /// The pointer-lerp hint box corners; the position getters `0x22B40` / `0x22B90` return ints.
    /// Globals are re-read after every getter, like the original.
    fn hint_box(&mut self) -> HintBox {
        let float = |frame: &Self, address: u32| read_f32(&*frame.m, address);
        let int = |frame: &Self, address: u32| frame.m.read32(address) as i32;
        let y0 = float(self, 0x007b_a038);
        let x0 = float(self, 0x007b_a188);
        let p1 = self.call(0x0002_2b40, &[]) as i32;
        let y_scaled = sse::mulss(float(self, 0x007b_a038), float(self, 0x0047_60cc));
        let a = sse::subss(sse::addss(p1 as f32, y_scaled), y0);
        let p2 = self.call(0x0002_2b90, &[]) as i32;
        let width = int(self, 0x004e_8cf4);
        let x_scaled = sse::mulss(float(self, 0x007b_a188), float(self, 0x0047_d1e4));
        let across = sse::subss(width.wrapping_sub(p2) as f32, x_scaled);
        let ratio = sse::divss(
            sse::mulss(
                sse::mulss(float(self, 0x007b_a038), float(self, 0x0047_db00)),
                width as f32,
            ),
            int(self, 0x004e_8cf0) as f32,
        );
        let b = sse::subss(sse::subss(across, ratio), x0);
        let p3 = self.call(0x0002_2b40, &[]) as i32;
        let y = float(self, 0x007b_a038);
        let bias = float(self, 0x0047_db00);
        let y_part = sse::addss(
            sse::addss(p3 as f32, sse::mulss(y, float(self, 0x0047_60cc))),
            sse::mulss(y, bias),
        );
        let c = sse::addss(y_part, y0);
        let d = sse::divss(
            sse::mulss(sse::mulss(y, bias), int(self, 0x004e_8cf4) as f32),
            int(self, 0x004e_8cf0) as f32,
        );
        let p4 = self.call(0x0002_2b90, &[]) as i32;
        let x_scaled = sse::mulss(float(self, 0x007b_a188), float(self, 0x0047_d1e4));
        let e = sse::subss(int(self, 0x004e_8cf4).wrapping_sub(p4) as f32, x_scaled);
        let e = sse::addss(sse::addss(sse::subss(e, d), d), x0);
        HintBox {
            a: sse::cvttss2si(a),
            b: sse::cvttss2si(b),
            c: sse::cvttss2si(c),
            e: sse::cvttss2si(e),
        }
    }

    /// `0x30CCD0(a, b, c, e)` with the icon byte in ESI (register argument), then `0x32DBA0`.
    /// Returns the icon byte: the original leaves it in ESI, where it replaces the label text.
    fn draw_hint_box(&mut self, boxed: HintBox, icon: u32) -> u32 {
        let saved_esi = self.regs.esi;
        let offset = icon.wrapping_mul(5).wrapping_shl(6);
        let icon_byte = self.m.read8(offset.wrapping_add(ICON_TABLE)) as u32;
        self.regs.esi = icon_byte;
        self.call(0x0030_ccd0, &[boxed.a, boxed.b, boxed.c, boxed.e]);
        self.regs.esi = saved_esi;
        self.call(0x0032_dba0, &[]);
        icon_byte
    }

    /// The two panel passes (`0x78950` modes 1 and 2) and the side panels.
    fn panels(&mut self) {
        self.call(0x0001_9650, &[0]);
        self.call(0x0030_cb30, &[]);
        let page = self.m.read32(PAGE);
        self.call(0x0007_8950, &[page, 1]);
        self.call(0x0001_9d70, &[1]);
        self.call(0x0001_9650, &[0]);
        self.call(0x0030_cb30, &[]);
        let page = self.m.read32(PAGE);
        self.call(0x0007_8950, &[page, 2]);
        for panel in [0, 2, 3, 7, 9] {
            self.call(0x0033_6570, &[panel]);
        }
        let leaf = self.leaf();
        let view = self
            .m
            .read32(leaf.wrapping_add(0x38))
            .wrapping_mul(VIEW_STRIDE);
        if self.m.read32(view.wrapping_add(0x007b_a270)) != 0 {
            let leaf = self.leaf();
            if self.m.read32(leaf.wrapping_add(0x40)) & 0x4_0000 == 0 {
                self.call(0x0030_cf60, &[]);
            }
        }
    }

    /// Hint box and text, then the label draw `0x3357B0`. The box needs mode 0, no alternate view and
    /// a selected (or alternate) widget; every other case goes straight to the label draw.
    fn hint_pass(&mut self, label: &mut HintLabel) {
        if self.flags() & context::FORCE_HINT == 0 {
            self.call(0x0030_c3c0, &[]);
            if self.field(context::MODE) == 0 && self.flags() & context::ALT_VIEW == 0 {
                let boxed = self.hint_box();
                let selection = self.field(context::SELECTION);
                let target = if selection != 0 {
                    Some((context::SELECTION, selection))
                } else {
                    let alt = self.field(context::ALT_SELECTION);
                    (alt != 0 && self.m.read32(alt.wrapping_add(0x40)) == 0)
                        .then_some((context::ALT_SELECTION, alt))
                };
                if let Some((field, record)) = target {
                    let icon = self.m.read32(record.wrapping_add(0x10));
                    // the original formats with the icon byte still in ESI, not the label text
                    let icon_byte = self.draw_hint_box(boxed, icon);
                    let current = self.field(field);
                    let name_record = self.m.read32(current.wrapping_add(0x2c));
                    let name_id = self.m.read32(name_record.wrapping_add(8));
                    let name = self.call(0x0003_d7b0, &[name_id]);
                    label.text = self.call(0x0015_1030, &[0x0049_f7c0, icon_byte, name]);
                }
            }
        }
        self.call(0x0033_57b0, &[label.text, 0, label.index, label.marker]);
    }
}

/// `0x3111F0`: per-frame editor draw. Saves the draw state (`0x19650`), draws the focused widget
/// (alternate view) or the world pass with the hint label and box, draws the panels and restores.
/// Every callee stays a guest call (attachment points to the widget tree, panels and renderer).
/// The alternate view returns without restoring the saved state, like the original.
pub fn run_editor_frame<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) {
    let mut f = Frame { m, guest, regs };
    let saved = f.call(0x0001_9650, &[0]);
    f.call(0x0001_9d70, &[1]);
    if f.flags() & context::ALT_VIEW != 0 {
        let widget = f.view_widget();
        f.call(0x0001_9d70, &[0]);
        f.call(0x0030_a550, &[]);
        f.call(0x0001_9d70, &[1]);
        f.call(0x0032_dba0, &[]);
        f.call(0x0030_be60, &[]);
        f.call(0x0032_f740, &[widget]);
        f.call(0x0030_b480, &[widget]);
        return;
    }
    f.call(0x0031_0a20, &[]);
    if f.field(context::HIDE_FLAGS) & 0x8000 == 0 {
        f.call(0x0033_6570, &[8]);
        if f.flags() & context::FOCUS_FLAGS == 0 {
            f.call(0x0030_be60, &[]);
        } else if f.leaf() != 0 {
            let leaf = f.leaf();
            if f.m.read8(leaf.wrapping_add(LEAF_FLAGS)) & 8 != 0 {
                f.call(0x0030_be60, &[]);
            }
        }
        let mut hinted = f.flags() & context::FORCE_HINT != 0;
        if f.flags() & 0x0404_4088 == 0 {
            let widget = f.view_widget();
            hinted |= widget == f.m.read32(PAGE);
        }
        if hinted {
            let mode = f.field(context::MODE);
            let mut label = f.resolve_label(mode);
            f.hint_pass(&mut label);
        } else {
            f.call(0x0033_5790, &[]);
        }
        f.call(0x0030_c3c0, &[]);
        f.call(0x0030_cb90, &[]);
    }
    f.panels();
    f.call(0x0001_9650, &[saved]);
}

/// The lane roots in registration order.
pub const ROOTS: [u32; 5] = [0x2fb4c0, 0x30ad30, 0x30acb0, 0x3111f0, 0x306460];
/// The A/B switch of these roots (selector and T3738 `--t2-offline` opt-in registry).
pub const OPT_IN: crate::runtime::OptIn = crate::runtime::OptIn {
    env: c"TSFP_MAPMAKER_OVERLAY",
    on: c"rust",
    roots: &ROOTS,
};

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

/// The A/B switch: the editor overlay remains opt-in until the isolated `0x306460` body
/// passes the current Rule 24 replay gate; `TSFP_MAPMAKER_OVERLAY=rust` selects Rust.
#[cfg(not(test))]
fn enabled() -> bool {
    unsafe {
        let value = getenv(OPT_IN.env.as_ptr());
        !value.is_null() && core::ffi::CStr::from_ptr(value).to_bytes() == OPT_IN.on.to_bytes()
    }
}

#[cfg(test)]
fn enabled() -> bool {
    true
}

/// Selector hook: a root is declined (the retained original runs) unless the switch is on.
pub fn declined(va: u32) -> bool {
    ROOTS.contains(&va) && !enabled()
}

/// Runs one original entry on guest memory through the typed model. `false` for other VAs.
pub fn run_original_entry<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x002f_b4c0 => {
            run_overlay_update(m, guest, regs);
            true
        }
        0x0030_ad30 => {
            run_anchor_shift(m, regs);
            true
        }
        0x0030_acb0 => {
            run_label_anchor(m, guest, regs);
            true
        }
        0x0031_11f0 => {
            run_editor_frame(m, guest, regs);
            true
        }
        0x0030_6460 => {
            crate::mapmaker_update::run_editor_update(m, guest, regs);
            true
        }
        _ => false,
    }
}

/// Lane dispatch, guarded like the other lanes (a bad pointer rolls back and runs the original).
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) || !enabled() {
        return false;
    }
    crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        run_original_entry(va, view, guest, regs)
    })
}
