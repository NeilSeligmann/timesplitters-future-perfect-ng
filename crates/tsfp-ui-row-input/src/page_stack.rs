// SPDX-License-Identifier: GPL-3.0-or-later
//! T3853: native page topology, independent of guest memory and slot addresses.

/// Resolve the active descendant of the highest occupied slot. `pages` is the live slot
/// snapshot in allocation order; active children may reuse earlier slots. Stale child links
/// terminate at the parent. Cycles have no valid top page and return `None` in bounded time.
pub fn top<P: Copy + Eq>(pages: impl IntoIterator<Item = P>, child: impl Fn(P) -> P) -> Option<P> {
    let pages: Vec<P> = pages.into_iter().collect();
    let mut current = *pages.last()?;
    for _ in 0..pages.len() {
        let next = child(current);
        if !pages.contains(&next) {
            return Some(current);
        }
        current = next;
    }
    None
}

/// Original 0x7A7B0 input exception for a flagged parent with one active descendant.
pub fn input_owner<P: Copy + Eq>(
    top: P,
    pages: impl IntoIterator<Item = P>,
    child: impl Fn(P) -> P,
    parent: impl Fn(P) -> P,
    permits_child_input: impl Fn(P) -> bool,
) -> P {
    pages
        .into_iter()
        .find(|p| permits_child_input(*p) && child(*p) == top && parent(top) == *p)
        .unwrap_or(top)
}

/// Native keyboard or pad starts at the first usable row when pointer hover left no focus.
pub fn initial_focus(count: u32, up: bool, usable: impl Fn(u32) -> bool) -> Option<u32> {
    let count = count.min(256);
    (0..count)
        .map(|i| if up { count - i - 1 } else { i })
        .find(|i| usable(*i))
}

pub const INPUT_STALL_FRAMES: u32 = 120;

/// Frames an input command has waited; report once at the threshold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputStall {
    frames: u32,
    reported: bool,
}

impl InputStall {
    pub const fn new() -> Self {
        Self {
            frames: 0,
            reported: false,
        }
    }

    pub fn wait(&mut self) -> Option<u32> {
        self.frames = self.frames.saturating_add(1);
        (self.frames >= INPUT_STALL_FRAMES && !std::mem::replace(&mut self.reported, true))
            .then_some(self.frames)
    }
}
