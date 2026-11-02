// SPDX-License-Identifier: GPL-3.0-or-later
//! T2310 UI-12 increment 5: the page stack of the widget framework as native Rust.
//!
//! Each viewport has a context record (pointer table 0x7BA264, stride 0x1E0) whose stack holds the ids of the pages
//! the player came through, each with a saved cursor or scroll ([`ContextStack`]). Opening a sub page pushes the
//! current page ([`push`], 0x79B70), Back reopens the previous one with its cursor or scroll restored ([`pop`],
//! 0x79C00). The page open itself (0x79860, which finds the slot or runs the page builder) stays the original,
//! reached through [`GuestUi::call`] (the UiBindings boundary), as does the pad mask clear 0x42830.
//!
//! Reference oracle: the c2r translations in `crates/tsfp-ports/src/c2r/ui_framework.rs`
//! (`game_ui_widget_push_child_by_id_onto_context_stack_7ba264_and_open`,
//! `game_ui_widget_pop_context_stack_7ba264_and_reopen_previous_with_restored_scroll`,
//! `game_record_list_append_entry_id_and_field_0x184_or_0x88_by_mode_0xf4`) and the original under Unicorn
//! (`python -m tools.small_diff --system ui_widgets`, cases `stack_append`, `stack_push`, `stack_pop`).

use crate::menu_state::layout::{context, page};
use crate::menu_state::{PageRef, read_f32};
use crate::native_page::GuestUi;

/// Original functions the stack reaches.
pub mod original {
    /// Find or open the page `id` from `parent` and select row `cursor` (-1: keep) -> the page record.
    pub const OPEN_PAGE: u32 = 0x7_9860;
    /// Clear pad mask bits of a viewport (`viewport`, mask).
    pub const CLEAR_PAD_MASK: u32 = 0x4_2830;
}

mod global {
    pub const UI_MODE: u32 = 0x79_094C;
    pub const CURRENT_VIEWPORT: u32 = 0x7B_A9E0;
    /// Pixel scale of a scroll offset (`page + 0xC0 = scroll * [this]`).
    pub const SCROLL_PIXEL_SCALE: u32 = 0x7B_A188;
}

/// Page flag 0x1000000 (in UI mode 0x65): the pop uses the page's own viewport, not viewport 0.
const FLAG_POP_OWN_VIEWPORT: u32 = 0x100_0000;
/// The pad mask the pop clears on the current viewport.
const POP_PAD_MASK: u32 = 0x120;
/// Page `+0xC0`: the scroll in pixels.
const SCROLL_PIXELS: u32 = 0xC0;

/// What the stack saves per entry ([`context::SAVE_MODE`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveMode {
    Cursor,
    Scroll,
    Nothing(u32),
}

/// One stacked page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StackEntry {
    pub id: u32,
    /// The saved word: a cursor index (`SaveMode::Cursor`) or float bits (`SaveMode::Scroll`).
    pub saved: u32,
}

/// Typed view of one context record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextStack(pub u32);

impl ContextStack {
    /// The context of `viewport` (the pointer in the table).
    pub fn of_viewport(g: &dyn GuestUi, viewport: u32) -> Self {
        let slot = context::TABLE
            .wrapping_add((viewport as i32).wrapping_mul(context::STRIDE as i32) as u32);
        ContextStack(g.read32(slot).unwrap_or(0))
    }
    fn get(self, g: &dyn GuestUi, field: u32) -> u32 {
        g.read32(self.0.wrapping_add(field)).unwrap_or(0)
    }
    pub fn count(self, g: &dyn GuestUi) -> i32 {
        self.get(g, context::COUNT) as i32
    }
    pub fn save_mode(self, g: &dyn GuestUi) -> SaveMode {
        match self.get(g, context::SAVE_MODE) {
            0 => SaveMode::Cursor,
            1 => SaveMode::Scroll,
            other => SaveMode::Nothing(other),
        }
    }
    fn id_at(self, g: &dyn GuestUi, i: u32) -> u32 {
        self.get(g, context::IDS.wrapping_add(i.wrapping_mul(4)))
    }
    fn saved_at(self, g: &dyn GuestUi, i: u32) -> u32 {
        self.get(g, context::SAVED.wrapping_add(i.wrapping_mul(4)))
    }
    /// The stacked pages, bottom first (at most [`context::CAPACITY`]).
    pub fn entries(self, g: &dyn GuestUi) -> Vec<StackEntry> {
        let count = self.count(g).clamp(0, context::CAPACITY as i32) as u32;
        (0..count)
            .map(|i| StackEntry {
                id: self.id_at(g, i),
                saved: self.saved_at(g, i),
            })
            .collect()
    }
}

/// Stack `page` on `ctx`: its id and, by the save mode, its cursor or its scroll (0x70CC0).
/// Like the original there is no capacity check (the menus never stack more than a few pages).
pub fn append(g: &mut dyn GuestUi, ctx: ContextStack, page: PageRef) {
    let count = ctx.get(g, context::COUNT);
    let at = count.wrapping_mul(4);
    let id = page.get(g, page::ID);
    g.write32(ctx.0.wrapping_add(context::IDS).wrapping_add(at), id);
    let saved = match ctx.save_mode(g) {
        SaveMode::Cursor => Some(page.get(g, page::CURSOR)),
        SaveMode::Scroll => Some(page.get(g, page::SCROLL)),
        SaveMode::Nothing(_) => None,
    };
    if let Some(saved) = saved {
        g.write32(ctx.0.wrapping_add(context::SAVED).wrapping_add(at), saved);
    }
    g.write32(ctx.0 + context::COUNT, count.wrapping_add(1));
}

/// Open the sub page `id` from `from` (0x79B70). The page currently shown as `from`'s active child is stacked
/// (unless it is closing, state 4 or 5); when `id` is already on the stack the stack is cut back to it (going
/// back to a page by opening it again). Returns the opened page record from the original open 0x79860.
pub fn push(g: &mut dyn GuestUi, from: PageRef, id: u32) -> u32 {
    if from.0 != 0 {
        let viewport = from.get(g, page::VIEWPORT);
        g.write32(global::CURRENT_VIEWPORT, viewport);
        let ctx = ContextStack::of_viewport(g, viewport);
        let shown = PageRef(from.get(g, page::ACTIVE_CHILD));
        if shown.0 != 0 && shown.get(g, page::ID) != 0 && !matches!(shown.state(g), 4 | 5) {
            append(g, ctx, shown);
        }
        let count = ctx.count(g);
        if let Some(i) = (0..count.max(0) as u32).find(|&i| ctx.id_at(g, i) == id) {
            g.write32(ctx.0 + context::COUNT, i);
        }
    }
    g.call(original::OPEN_PAGE, &[from.0, id, u32::MAX])
        .unwrap_or(0)
}

/// Back to the previous page of the stack from `parent` (0x79C00): pop the top id, reopen it through the
/// original open with the saved cursor (save mode 0), or reopen it and restore the saved scroll clamped to the
/// content (save mode 1); then clear the pad mask 0x120 of the current viewport. An empty stack reopens id 0.
/// Returns the reopened page record.
pub fn pop(g: &mut dyn GuestUi, parent: PageRef) -> u32 {
    let viewport = g.read32(global::CURRENT_VIEWPORT).unwrap_or(0);
    let mut ctx = ContextStack::of_viewport(g, viewport);
    if g.read32(global::UI_MODE).unwrap_or(0) == 0x65
        && parent.get(g, page::FLAGS) & FLAG_POP_OWN_VIEWPORT == 0
    {
        ctx = ContextStack::of_viewport(g, 0);
        g.write32(global::CURRENT_VIEWPORT, 0);
    }
    let mut id = 0;
    let mut cursor = u32::MAX;
    let mut scroll = None;
    if parent.0 != 0 && ctx.get(g, context::COUNT) != 0 {
        let mode = ctx.save_mode(g);
        let top = ctx.get(g, context::COUNT).wrapping_sub(1);
        g.write32(ctx.0 + context::COUNT, top);
        id = ctx.id_at(g, top);
        match mode {
            SaveMode::Cursor => cursor = ctx.saved_at(g, top),
            SaveMode::Scroll => scroll = Some(f32::from_bits(ctx.saved_at(g, top))),
            SaveMode::Nothing(_) => {}
        }
    }
    let opened = g
        .call(original::OPEN_PAGE, &[parent.0, id, cursor])
        .unwrap_or(0);
    if let Some(saved) = scroll {
        restore_scroll(g, PageRef(opened), saved);
    }
    let viewport = g.read32(global::CURRENT_VIEWPORT).unwrap_or(0);
    g.call(original::CLEAR_PAD_MASK, &[viewport, POP_PAD_MASK]);
    opened
}

/// The saved scroll clamped to `[0, content - view]` in the original's order (0x79C9E..0x79CF7): a negative value
/// becomes 0, then a value past the end becomes the end (so a short page ends at the negative end).
fn restore_scroll(g: &mut dyn GuestUi, opened: PageRef, saved: f32) {
    let view = read_f32(g, opened.0 + page::VIEW_BOTTOM) - read_f32(g, opened.0 + page::VIEW_TOP);
    let end = read_f32(g, opened.0 + page::CONTENT_HEIGHT) - view;
    let mut scroll = saved;
    if 0.0 > scroll {
        scroll = 0.0;
    }
    if scroll > end {
        scroll = end;
    }
    g.write32(opened.0 + page::SCROLL, scroll.to_bits());
    g.write32(opened.0 + page::SCROLL_X, scroll.to_bits());
    let pixels = scroll * read_f32(g, global::SCROLL_PIXEL_SCALE);
    g.write32(opened.0 + SCROLL_PIXELS, pixels.to_bits());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Fake {
        mem: HashMap<u32, u32>,
        calls: Vec<(u32, Vec<u32>)>,
    }
    impl GuestUi for Fake {
        fn read32(&self, va: u32) -> Option<u32> {
            Some(*self.mem.get(&va).unwrap_or(&0))
        }
        fn write32(&mut self, va: u32, value: u32) -> bool {
            self.mem.insert(va, value);
            true
        }
        fn call(&mut self, va: u32, args: &[u32]) -> Option<u32> {
            self.calls.push((va, args.to_vec()));
            Some(if va == original::OPEN_PAGE { OPENED } else { 0 })
        }
    }

    const CTX: u32 = 0x90_0000;
    const OPENED: u32 = 0x91_0000;

    fn world(mode: u32) -> (Fake, PageRef, PageRef) {
        let mut f = Fake::default();
        f.mem.insert(context::TABLE + context::STRIDE, CTX);
        f.mem.insert(global::CURRENT_VIEWPORT, 1);
        f.mem.insert(CTX + context::SAVE_MODE, mode);
        let from = PageRef::slot(2);
        let shown = PageRef::slot(3);
        f.mem.insert(from.0 + page::VIEWPORT, 1);
        f.mem.insert(from.0 + page::ACTIVE_CHILD, shown.0);
        f.mem.insert(shown.0 + page::ID, 0x11);
        f.mem.insert(shown.0 + page::STATE, 3);
        f.mem.insert(shown.0 + page::CURSOR, 4);
        f.mem.insert(shown.0 + page::SCROLL, 250f32.to_bits());
        (f, from, shown)
    }

    #[test]
    fn push_stacks_the_shown_page_and_pop_restores_its_cursor() {
        let (mut f, from, _) = world(0);
        assert_eq!(push(&mut f, from, 0x22), OPENED);
        assert_eq!(
            ContextStack(CTX).entries(&f),
            vec![StackEntry { id: 0x11, saved: 4 }]
        );
        assert_eq!(
            f.calls,
            vec![(original::OPEN_PAGE, vec![from.0, 0x22, u32::MAX])]
        );
        f.calls.clear();
        pop(&mut f, from);
        assert_eq!(ContextStack(CTX).count(&f), 0);
        assert_eq!(
            f.calls,
            vec![
                (original::OPEN_PAGE, vec![from.0, 0x11, 4]),
                (original::CLEAR_PAD_MASK, vec![1, 0x120])
            ]
        );
    }

    #[test]
    fn push_of_a_stacked_id_cuts_the_stack_back() {
        let (mut f, from, shown) = world(0);
        f.mem.insert(CTX + context::COUNT, 3);
        f.mem.insert(CTX + context::IDS, 0x5);
        f.mem.insert(CTX + context::IDS + 4, 0x22);
        f.mem.insert(CTX + context::IDS + 8, 0x7);
        f.mem.insert(shown.0 + page::STATE, 4); // closing: not stacked
        push(&mut f, from, 0x22);
        assert_eq!(ContextStack(CTX).count(&f), 1);
    }

    #[test]
    fn pop_restores_a_clamped_scroll() {
        let (mut f, from, _) = world(1);
        push(&mut f, from, 0x22);
        f.mem
            .insert(OPENED + page::CONTENT_HEIGHT, 300f32.to_bits());
        f.mem.insert(OPENED + page::VIEW_TOP, 0f32.to_bits());
        f.mem.insert(OPENED + page::VIEW_BOTTOM, 100f32.to_bits());
        f.mem.insert(global::SCROLL_PIXEL_SCALE, 2f32.to_bits());
        pop(&mut f, from);
        assert_eq!(f.mem[&(OPENED + page::SCROLL)], 200f32.to_bits());
        assert_eq!(f.mem[&(OPENED + page::SCROLL_X)], 200f32.to_bits());
        assert_eq!(f.mem[&(OPENED + SCROLL_PIXELS)], 400f32.to_bits());
        assert_eq!(
            f.calls[1],
            (original::OPEN_PAGE, vec![from.0, 0x11, u32::MAX])
        );
    }

    #[test]
    fn empty_stack_reopens_id_zero_and_mode_0x65_uses_viewport_0() {
        let (mut f, from, _) = world(0);
        f.mem.insert(global::UI_MODE, 0x65);
        f.mem.insert(context::TABLE, CTX + 0x1000);
        pop(&mut f, from);
        assert_eq!(f.mem[&global::CURRENT_VIEWPORT], 0);
        assert_eq!(f.calls[0], (original::OPEN_PAGE, vec![from.0, 0, u32::MAX]));
        assert_eq!(f.calls[1], (original::CLEAR_PAD_MASK, vec![0, 0x120]));
    }
}
