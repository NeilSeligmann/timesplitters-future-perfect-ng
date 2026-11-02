// SPDX-License-Identifier: GPL-3.0-or-later
//! T1916: the Map Maker on-screen pointer driven by the real mouse.
//!
//! The Map Maker editor menus are pointer pages: the D-pad moves nothing, the guest moves a
//! pointer from the left stick (`0x73870`, per page and frame) and the select handler acts on the
//! row under it (T2185, `docs/t-ui-tree-schema.md`). The pointer lives in the per-viewport UI
//! context record (`docs/t-ui-data-model.md`, TsfpUiContext; T2159 OQ7):
//!
//! * record `0x7BA260 + viewport * 0x1E0`, viewport index of the page (`page +0x38`) or the
//!   current one `[0x7BA9E0]`;
//! * `+0x10` pointer mode, 1 in Map Maker (UI mode `[0x79094C]` 0x66), 0 in the front end;
//! * `+0x18` / `+0x1C` the pointer state in percent units (the stick integrates it in `0x73870`);
//! * `+0x20` / `+0x24` pointer x / y, f32 in the 640x480 design grid, derived from the state by
//!   `0x71050(context)`: `x = [0x7BA038] * state_x + [0x7BA180]`, `y = [0x7BA188] * state_y +
//!   [0x7BA184]`, clamped to `+0x48..+0x50` (x) and `+0x4C..+0x54` (y), the state written back on a
//!   clamp through the inverse scales `[0x7BA18C]` and `[0x7BA030]` (MEASURED from the lift).
//!
//! MEASURED live (first build): writing only `+0x20/+0x24` does nothing, the next `0x73870`
//! re-derives them from the state. The native path therefore writes the state (the inverse of the
//! derivation, at the frame boundary, before the guest's page tick) and lets the original
//! `0x71050` derive and clamp the grid position, so the guest's own hover, highlight and select
//! logic see the real mouse. The stick still moves the
//! pointer when the mouse is still (a position is written only when the mouse moved). This is a
//! host-boundary input feature (rule 19 exception: host input), not a patch of game logic.
use crate::native_page::GuestUi;

pub mod addr {
    /// Per-viewport UI context records.
    pub const UI_CONTEXT: u32 = 0x7B_A260;
    pub const UI_CONTEXT_STRIDE: u32 = 0x1E0;
    pub const UI_CONTEXT_COUNT: u32 = 4;
    /// Current viewport index.
    pub const CURRENT_VIEWPORT: u32 = 0x7B_A9E0;
    /// UI mode: 0x65 front end, 0x66 Map Maker.
    pub const UI_MODE: u32 = 0x79_094C;
    pub const MODE_MAPMAKER: u32 = 0x66;
    /// Grid origin and scales of the percent units (`0x71050`).
    pub const ORIGIN_X: u32 = 0x7B_A180;
    pub const ORIGIN_Y: u32 = 0x7B_A184;
    pub const INVERSE_SCALE_X: u32 = 0x7B_A18C;
    pub const INVERSE_SCALE_Y: u32 = 0x7B_A030;
    /// `0x71050(context)`: derive and clamp the grid position from the state.
    pub const DERIVE: u32 = 0x7_1050;
    pub mod context {
        pub const POINTER_MODE: u32 = 0x10;
        pub const STATE_X: u32 = 0x18;
        pub const STATE_Y: u32 = 0x1C;
        pub const POINTER_X: u32 = 0x20;
        pub const POINTER_Y: u32 = 0x24;
    }
}

pub const GRID_WIDTH: f32 = 640.0;
pub const GRID_HEIGHT: f32 = 480.0;

/// The UI context record of `viewport` (`None` out of range).
pub fn context(viewport: u32) -> Option<u32> {
    (viewport < addr::UI_CONTEXT_COUNT)
        .then(|| addr::UI_CONTEXT + viewport * addr::UI_CONTEXT_STRIDE)
}

/// The record of the current viewport.
pub fn current_context(g: &dyn GuestUi) -> Option<u32> {
    context(g.read32(addr::CURRENT_VIEWPORT)?)
}

/// The guest is in the Map Maker and the current UI context has the pointer on.
pub fn pointer_active(g: &dyn GuestUi) -> bool {
    g.read32(addr::UI_MODE) == Some(addr::MODE_MAPMAKER)
        && current_context(g)
            .and_then(|c| g.read32(c + addr::context::POINTER_MODE))
            .is_some_and(|mode| mode != 0)
}

/// The pointer of the current UI context.
pub fn read(g: &dyn GuestUi) -> Option<(f32, f32)> {
    let c = current_context(g)?;
    Some((
        f32::from_bits(g.read32(c + addr::context::POINTER_X)?),
        f32::from_bits(g.read32(c + addr::context::POINTER_Y)?),
    ))
}

/// Grid clamp: the guest keeps its pointer inside the 640x480 grid.
pub fn clamp(x: f32, y: f32) -> Option<(f32, f32)> {
    (x.is_finite() && y.is_finite()).then(|| (x.clamp(0.0, GRID_WIDTH), y.clamp(0.0, GRID_HEIGHT)))
}

/// Put the mouse at `(x, y)` (design grid) into the current pointer. `false` when the guest has
/// no active pointer (not the Map Maker) or the write failed.
pub fn write(g: &mut dyn GuestUi, x: f32, y: f32) -> bool {
    if !pointer_active(g) {
        return false;
    }
    let (Some(c), Some((x, y))) = (current_context(g), clamp(x, y)) else {
        return false;
    };
    let float =
        |g: &dyn GuestUi, va: u32| g.read32(va).map(f32::from_bits).filter(|v| v.is_finite());
    let (Some(ox), Some(oy), Some(ix), Some(iy)) = (
        float(g, addr::ORIGIN_X),
        float(g, addr::ORIGIN_Y),
        float(g, addr::INVERSE_SCALE_X),
        float(g, addr::INVERSE_SCALE_Y),
    ) else {
        return false;
    };
    let wrote = g.write32(c + addr::context::STATE_X, ((x - ox) * ix).to_bits())
        && g.write32(c + addr::context::STATE_Y, ((y - oy) * iy).to_bits());
    if !wrote {
        return false;
    }
    // The original derivation and clamp; without a call path (tests) the grid position directly.
    g.call(addr::DERIVE, &[c]).is_some()
        || (g.write32(c + addr::context::POINTER_X, x.to_bits())
            && g.write32(c + addr::context::POINTER_Y, y.to_bits()))
}

/// Mouse samples to pointer writes: a write only when the mouse moved since the last write, so
/// the stick keeps the pointer while the mouse rests.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MouseFeed {
    last: Option<(f32, f32)>,
}

impl MouseFeed {
    pub const fn new() -> Self {
        Self { last: None }
    }
    /// The position to write for this frame's mouse sample.
    /// The first sample only sets the reference (a mouse resting somewhere at start must not
    /// move the pointer; MEASURED: the first build wrote the parked X pointer of a headless run).
    pub fn next(&mut self, mouse: Option<(f32, f32)>) -> Option<(f32, f32)> {
        let pos = mouse?;
        let before = self.last.replace(pos);
        (before.is_some() && before != Some(pos)).then_some(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Mem(HashMap<u32, u32>);
    impl GuestUi for Mem {
        fn read32(&self, va: u32) -> Option<u32> {
            self.0.get(&va).copied()
        }
        fn write32(&mut self, va: u32, value: u32) -> bool {
            self.0.insert(va, value);
            true
        }
        fn call(&mut self, _va: u32, _args: &[u32]) -> Option<u32> {
            None
        }
    }

    fn mapmaker(viewport: u32) -> Mem {
        let mut m = Mem::default();
        m.0.insert(addr::UI_MODE, addr::MODE_MAPMAKER);
        m.0.insert(addr::CURRENT_VIEWPORT, viewport);
        let c = context(viewport).unwrap();
        m.0.insert(c + addr::context::POINTER_MODE, 1);
        m.0.insert(c + addr::context::POINTER_X, 320f32.to_bits());
        m.0.insert(c + addr::context::POINTER_Y, 230f32.to_bits());
        // Live values: origin 0, 6.4 / 4.8 units per percent.
        m.0.insert(addr::ORIGIN_X, 0f32.to_bits());
        m.0.insert(addr::ORIGIN_Y, 0f32.to_bits());
        m.0.insert(addr::INVERSE_SCALE_X, (1.0f32 / 6.4).to_bits());
        m.0.insert(addr::INVERSE_SCALE_Y, (1.0f32 / 4.8).to_bits());
        m
    }

    #[test]
    fn writes_the_mouse_into_the_current_context_only_in_the_map_maker() {
        let mut m = mapmaker(1);
        assert_eq!(read(&m), Some((320.0, 230.0)));
        assert!(write(&mut m, 400.0, 100.0));
        assert_eq!(read(&m), Some((400.0, 100.0)));
        assert_eq!(
            m.0.get(&(0x7B_A260 + 0x1E0 + 0x20)),
            Some(&400f32.to_bits())
        );
        assert_eq!(m.0.get(&0x7B_A280), None, "viewport 0 untouched");
        let state = |m: &Mem, o: u32| f32::from_bits(m.0[&(0x7B_A260 + 0x1E0 + o)]);
        assert!(
            (state(&m, 0x18) - 62.5).abs() < 1e-4 && (state(&m, 0x1C) - 100.0 / 4.8).abs() < 1e-4,
            "percent state"
        );
        // Off the grid: clamped.
        assert!(write(&mut m, -20.0, 900.0));
        assert_eq!(read(&m), Some((0.0, 480.0)));
        // Front end (mode 0x65) or pointer off: nothing written.
        m.0.insert(addr::UI_MODE, 0x65);
        assert!(!write(&mut m, 10.0, 10.0));
        assert_eq!(read(&m), Some((0.0, 480.0)));
        let mut off = mapmaker(0);
        off.0.insert(0x7B_A270, 0);
        assert!(!write(&mut off, 10.0, 10.0));
        let mut wild = mapmaker(0);
        wild.0.insert(addr::CURRENT_VIEWPORT, 7);
        assert!(!write(&mut wild, 10.0, 10.0), "viewport out of range");
    }

    #[test]
    fn the_feed_writes_only_when_the_mouse_moved() {
        let mut feed = MouseFeed::default();
        assert_eq!(feed.next(None), None);
        assert_eq!(
            feed.next(Some((1.0, 2.0))),
            None,
            "the first sample is the reference"
        );
        assert_eq!(
            feed.next(Some((1.0, 2.0))),
            None,
            "resting mouse leaves the stick in charge"
        );
        assert_eq!(feed.next(Some((3.0, 2.0))), Some((3.0, 2.0)));
        assert_eq!(clamp(f32::NAN, 1.0), None);
    }
}
