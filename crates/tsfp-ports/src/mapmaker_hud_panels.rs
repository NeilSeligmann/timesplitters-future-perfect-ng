// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/editor
//! T2415: Rust-owned Map Maker editor HUD panel slides (readable port, Tier 1).
//!
//! The editor keeps ten sliding HUD panels in a table of 0x48-byte records at `0x778A00`. Panel 9
//! is the tooltip panel, which fades in place instead of sliding. Five original functions own the
//! table:
//!
//! | VA | role | ABI |
//! |---|---|---|
//! | `0x335500` | advance one panel by one frame | panel index in ECX |
//! | `0x3356A0` | advance every non-hidden panel and publish its active flag | none |
//! | `0x3356F0` | begin (or reverse into) a slide in, then advance once | cdecl index |
//! | `0x335790` | hide the tooltip panel | none |
//! | `0x335840` | begin a slide out of a shown panel | cdecl index |
//!
//! The record is read into a typed [`HudPanel`], updated by readable Rust and written back, so the
//! logic acts on Rust-owned values. Guest memory stays the authoritative store between calls
//! (the rest of the editor still reads the table), which is the attachment point of this slice.
//! Timing constants are read from the original image at run time ([`SlideTiming`]), never
//! committed. Floating point follows the original: SSE single precision for the timer and fade
//! terms, x87 at the caller's precision control for the square-root easing and position lerp.
//!
//! A/B: the roots are lane roots (`--rust-ports`, group `modes`) and additionally declined unless
//! Rust is the default after the T2415 replay adoption gate; set
//! `TSFP_MAPMAKER_HUD_PANELS=original` to keep the lifted implementation.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
    system_slice::{round_pc, sqrt_double},
};

/// First record of the panel table.
pub const PANEL_TABLE: u32 = 0x0077_8a00;
/// Bytes per panel record.
pub const PANEL_STRIDE: u32 = 0x48;
/// Records in the table (the original loop ends at `0x778CD0`).
pub const PANEL_COUNT: u32 = 10;
/// The tooltip panel fades in place and has its own step rule.
pub const TOOLTIP_PANEL: u32 = 9;

/// The lifecycle word at record `+0x00`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlidePhase {
    /// 0: not drawn, never advanced by the update pass.
    Hidden,
    /// 1: sliding in, then held fully shown once the timer reaches the duration.
    Entering,
    /// 2: sliding out; returns to `Hidden` when the timer reaches the duration.
    Leaving,
    /// Any other value; kept so a write-back never changes it.
    Other(u32),
}

impl SlidePhase {
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            0 => Self::Hidden,
            1 => Self::Entering,
            2 => Self::Leaving,
            other => Self::Other(other),
        }
    }

    pub fn raw(self) -> u32 {
        match self {
            Self::Hidden => 0,
            Self::Entering => 1,
            Self::Leaving => 2,
            Self::Other(other) => other,
        }
    }
}

/// A screen position pair of f32 words.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// One panel record. Offsets are the original layout; `+0x20` is not used by these functions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudPanel {
    /// `+0x00`.
    pub phase: SlidePhase,
    /// `+0x04/+0x08`: where a slide in starts.
    pub entry_from: Point,
    /// `+0x0C/+0x10`: the shown position.
    pub shown: Point,
    /// `+0x14/+0x18`: where a slide out ends.
    pub exit_to: Point,
    /// `+0x1C`: width used for the right-edge test.
    pub width: f32,
    /// `+0x24/+0x28`: the drawn position this frame.
    pub position: Point,
    /// `+0x2C`: elapsed slide time.
    pub timer: f32,
    /// `+0x30`: slide length.
    pub duration: f32,
    /// `+0x34`: eased fraction before the square root.
    pub progress: f32,
    /// `+0x38`: fade factor.
    pub alpha: f32,
    /// `+0x3C`: published by the update pass (drawn this frame).
    pub active: u32,
    /// `+0x40` (0/1 once advanced, raw until then): the shown x is right of the screen origin.
    pub right_of_origin: u32,
    /// `+0x44` (0/1 once advanced, raw until then): the shown panel ends left of the screen width.
    pub fits_screen: u32,
}

fn read_f32<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

fn write_f32<M: GuestMemory>(m: &mut M, address: u32, value: f32) {
    m.write32(address, value.to_bits());
}

fn read_point<M: GuestMemory>(m: &M, address: u32) -> Point {
    Point {
        x: read_f32(m, address),
        y: read_f32(m, address + 4),
    }
}

/// Record address of panel `index` (unchecked, like the original).
pub fn panel_address(index: u32) -> u32 {
    PANEL_TABLE.wrapping_add(index.wrapping_mul(PANEL_STRIDE))
}

impl HudPanel {
    pub fn read<M: GuestMemory>(m: &M, index: u32) -> Self {
        let base = panel_address(index);
        let at = |offset: u32| base.wrapping_add(offset);
        Self {
            phase: SlidePhase::from_raw(m.read32(base)),
            entry_from: read_point(m, at(0x04)),
            shown: read_point(m, at(0x0c)),
            exit_to: read_point(m, at(0x14)),
            width: read_f32(m, at(0x1c)),
            position: read_point(m, at(0x24)),
            timer: read_f32(m, at(0x2c)),
            duration: read_f32(m, at(0x30)),
            progress: read_f32(m, at(0x34)),
            alpha: read_f32(m, at(0x38)),
            active: m.read32(at(0x3c)),
            right_of_origin: m.read32(at(0x40)),
            fits_screen: m.read32(at(0x44)),
        }
    }

    /// Writes the fields these functions own; the geometry inputs are left untouched.
    pub fn write<M: GuestMemory>(&self, m: &mut M, index: u32) {
        let base = panel_address(index);
        let at = |offset: u32| base.wrapping_add(offset);
        m.write32(base, self.phase.raw());
        write_f32(m, at(0x24), self.position.x);
        write_f32(m, at(0x28), self.position.y);
        write_f32(m, at(0x2c), self.timer);
        write_f32(m, at(0x30), self.duration);
        write_f32(m, at(0x34), self.progress);
        write_f32(m, at(0x38), self.alpha);
        m.write32(at(0x3c), self.active);
        m.write32(at(0x40), self.right_of_origin);
        m.write32(at(0x44), self.fits_screen);
    }
}

/// Constants and frame globals the original reads. Values come from the running image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlideTiming {
    /// `0x4E8CEC` (i32): frames per second.
    pub frame_rate: i32,
    /// `0x4E8CF0` (i32): screen width.
    pub screen_width: i32,
    /// `0x475C78`: one.
    pub one: f32,
    /// `0x4E7948`: per-frame timer scale.
    pub step_scale: f32,
    /// `0x475CD4`: slide-in fade scale.
    pub enter_fade_scale: f32,
    /// `0x4785E0`: slide-out fade scale.
    pub leave_fade_scale: f32,
    /// `0x4765E0`: slide-out duration.
    pub leave_duration: f32,
    /// `0x475CDC`: slide-in duration.
    pub enter_duration: f32,
    /// `0x47DAEC`: timer value a slide in starts from.
    pub enter_start: f32,
}

impl SlideTiming {
    pub fn read<M: GuestMemory>(m: &M) -> Self {
        Self {
            frame_rate: m.read32(0x004e_8cec) as i32,
            screen_width: m.read32(0x004e_8cf0) as i32,
            one: read_f32(m, 0x0047_5c78),
            step_scale: read_f32(m, 0x004e_7948),
            enter_fade_scale: read_f32(m, 0x0047_5cd4),
            leave_fade_scale: read_f32(m, 0x0047_85e0),
            leave_duration: read_f32(m, 0x0047_65e0),
            enter_duration: read_f32(m, 0x0047_5cdc),
            enter_start: read_f32(m, 0x0047_daec),
        }
    }

    /// Timer advance per frame (SSE: one / rate * scale).
    pub fn frame_step(&self) -> f32 {
        self.one / self.frame_rate as f32 * self.step_scale
    }
}

/// x87 arithmetic at the caller's precision control.
#[derive(Clone, Copy)]
struct X87(u16);

impl X87 {
    fn round(self, value: f64) -> f64 {
        round_pc(value, self.0)
    }

    fn sqrt(self, value: f32) -> f64 {
        self.round(sqrt_double(f64::from(value)))
    }

    /// `(to - from) * t + from`, each operation rounded, stored as f32.
    fn lerp(self, from: f32, to: f32, t: f64) -> f32 {
        let from = f64::from(from);
        let span = self.round(f64::from(to) - from);
        self.round(self.round(span * t) + from) as f32
    }
}

impl HudPanel {
    /// `0x335500`: one frame of the panel at `index`.
    fn advance(&mut self, index: u32, timing: &SlideTiming, x87: X87) {
        let step = timing.frame_step();
        if index == TOOLTIP_PANEL {
            self.advance_tooltip(step);
        } else {
            self.advance_slide(step, timing, x87);
        }
        self.right_of_origin = u32::from(self.shown.x > 0.0);
        self.fits_screen = u32::from(timing.screen_width as f32 > self.width + self.shown.x);
    }

    /// The tooltip fades in while entering and out otherwise; it is always drawn at `shown`.
    fn advance_tooltip(&mut self, step: f32) {
        if self.phase == SlidePhase::Entering {
            self.timer += step;
            if self.timer > self.duration {
                self.timer = self.duration;
            }
        } else {
            self.timer -= step;
            if self.timer < 0.0 {
                self.timer = 0.0;
                self.phase = SlidePhase::Hidden;
            }
        }
        self.position = self.shown;
        self.alpha = self.timer / self.duration;
    }

    fn advance_slide(&mut self, step: f32, timing: &SlideTiming, x87: X87) {
        self.timer += step;
        // Written as `!(a < b)` so a NaN timer clamps, like the original's unordered branch.
        if !(self.timer < self.duration) {
            self.timer = self.duration;
            if self.phase == SlidePhase::Leaving {
                self.phase = SlidePhase::Hidden;
                self.progress = 0.0;
                return;
            }
        }
        match self.phase {
            SlidePhase::Entering => {
                self.progress = if self.timer > 0.0 || self.timer.is_nan() {
                    self.timer / self.duration
                } else {
                    0.0
                };
                let eased = x87.sqrt(self.progress);
                self.alpha = (self.progress + timing.one) * timing.enter_fade_scale;
                self.position = Point {
                    x: x87.lerp(self.entry_from.x, self.shown.x, eased),
                    y: x87.lerp(self.entry_from.y, self.shown.y, eased),
                };
            }
            SlidePhase::Leaving => {
                self.progress = timing.one - self.timer / self.duration;
                let eased = x87.sqrt(self.progress);
                self.position = Point {
                    x: x87.lerp(self.exit_to.x, self.shown.x, eased),
                    y: x87.lerp(self.exit_to.y, self.shown.y, eased),
                };
                self.alpha = x87.round(eased * f64::from(timing.leave_fade_scale)) as f32;
            }
            SlidePhase::Hidden | SlidePhase::Other(_) => {}
        }
    }

    /// `0x3356F0` before its tail call: start entering, or reverse a slide out.
    fn begin_enter(&mut self, timing: &SlideTiming) {
        match self.phase {
            SlidePhase::Hidden => {
                self.timer = timing.enter_start;
                self.duration = timing.enter_duration;
                self.progress = 0.0;
            }
            SlidePhase::Leaving => {
                self.duration = timing.enter_duration;
                if timing.enter_duration > self.progress {
                    self.timer = 0.0;
                    self.progress = 0.0;
                } else {
                    self.timer = timing.enter_duration;
                    self.progress = timing.one;
                }
            }
            SlidePhase::Entering | SlidePhase::Other(_) => {}
        }
        self.phase = SlidePhase::Entering;
    }

    /// `0x335840`: only an entering (or shown) panel starts leaving.
    fn begin_leave(&mut self, timing: &SlideTiming) {
        if self.phase == SlidePhase::Entering {
            self.timer = 0.0;
            self.duration = timing.leave_duration;
            self.progress = timing.one;
            self.phase = SlidePhase::Leaving;
        }
    }

    /// The `+0x3C` flag the update pass publishes after advancing.
    fn is_active(&self) -> bool {
        match self.phase {
            SlidePhase::Hidden => false,
            SlidePhase::Entering => self.timer >= 0.0,
            SlidePhase::Leaving | SlidePhase::Other(_) => true,
        }
    }
}

/// The panel table as Rust-owned values for one operation.
#[derive(Clone, Debug, PartialEq)]
pub struct HudPanels {
    pub panels: [HudPanel; PANEL_COUNT as usize],
    pub timing: SlideTiming,
    x87_control: u16,
}

impl HudPanels {
    pub fn read<M: GuestMemory>(m: &M, x87_control: u16) -> Self {
        Self {
            panels: core::array::from_fn(|index| HudPanel::read(m, index as u32)),
            timing: SlideTiming::read(m),
            x87_control,
        }
    }

    fn x87(&self) -> X87 {
        X87(self.x87_control)
    }

    /// `0x335500` for an in-table index.
    pub fn advance(&mut self, index: usize) {
        let (timing, x87) = (self.timing, self.x87());
        self.panels[index].advance(index as u32, &timing, x87);
    }

    /// `0x3356A0`: returns the last published active flag (the original's EAX).
    pub fn update_all(&mut self) -> bool {
        let mut active = false;
        for index in 0..self.panels.len() {
            if self.panels[index].phase != SlidePhase::Hidden {
                self.advance(index);
            }
            active = self.panels[index].is_active();
            self.panels[index].active = u32::from(active);
        }
        active
    }

    /// `0x3356F0`.
    pub fn begin_enter(&mut self, index: usize) {
        let timing = self.timing;
        self.panels[index].begin_enter(&timing);
        self.advance(index);
    }

    /// `0x335840`.
    pub fn begin_leave(&mut self, index: usize) {
        let timing = self.timing;
        self.panels[index].begin_leave(&timing);
    }

    /// `0x335790`: phase and timer of the tooltip only.
    pub fn hide_tooltip(&mut self) {
        let tooltip = &mut self.panels[TOOLTIP_PANEL as usize];
        tooltip.phase = SlidePhase::Hidden;
        tooltip.timer = 0.0;
    }

    pub fn write<M: GuestMemory>(&self, m: &mut M) {
        for (index, panel) in self.panels.iter().enumerate() {
            panel.write(m, index as u32);
        }
    }
}

/// Runs one original entry on guest memory through the typed model. `false` for other VAs or an
/// index outside the table (the original would index past it; the lifted body then runs).
pub fn run_original_entry<M: GuestMemory>(va: u32, m: &mut M, regs: &mut ExtRegisters) -> bool {
    let index = match va {
        0x0033_5500 => regs.ecx,
        0x0033_56f0 | 0x0033_5840 => m.read32(regs.esp.wrapping_add(4)),
        0x0033_56a0 | 0x0033_5790 => 0,
        _ => return false,
    };
    if index >= PANEL_COUNT {
        return false;
    }
    let mut panels = HudPanels::read(m, regs.fpu.control);
    let slot = index as usize;
    match va {
        0x0033_5500 => {
            panels.advance(slot);
            regs.eax = panel_address(index);
        }
        0x0033_56a0 => regs.eax = u32::from(panels.update_all()),
        0x0033_56f0 => {
            panels.begin_enter(slot);
            regs.eax = panel_address(index);
            regs.ecx = index;
        }
        0x0033_5790 => panels.hide_tooltip(),
        _ => {
            panels.begin_leave(slot);
            regs.eax = panel_address(index);
        }
    }
    panels.write(m);
    true
}

/// The lane roots in registration order.
pub const ROOTS: [u32; 5] = [0x335500, 0x3356a0, 0x3356f0, 0x335790, 0x335840];
/// The A/B switch of these roots (selector and T3738 `--t2-offline` opt-in registry).
pub const OPT_IN: crate::runtime::OptIn = crate::runtime::OptIn {
    env: c"TSFP_MAPMAKER_HUD_PANELS",
    on: c"rust",
    roots: &ROOTS,
};

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

/// The A/B switch: Rust is default; `TSFP_MAPMAKER_HUD_PANELS=original` selects lifted code.
#[cfg(not(test))]
fn enabled() -> bool {
    unsafe {
        let value = getenv(OPT_IN.env.as_ptr());
        value.is_null() || core::ffi::CStr::from_ptr(value).to_bytes() == OPT_IN.on.to_bytes()
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
    crate::anim_lane::guarded(va, m, guest, regs, |view, _guest, regs| {
        run_original_entry(va, view, regs)
    })
}
