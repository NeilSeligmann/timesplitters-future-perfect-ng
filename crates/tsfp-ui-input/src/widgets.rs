// SPDX-License-Identifier: GPL-3.0-or-later
//! T2310 UI-12: native widget classes of the framework, readable Rust ports of the original:
//!
//! * list navigation: the cursor row lookup 0x719D0 ([`cursor_row_at`]), the cursor and group
//!   column step of 0x7A7B0 (0x7AE13..0x7AFF3, [`step`]) and the scroll to the cursor row 0x73380
//!   ([`scroll_to_row`]);
//! * option rows: select by index 0x75430 ([`option_select`]), next and previous enabled option
//!   with wrap or numeric step 0x755D0 / 0x756A0 ([`option_next`], [`option_prev`]).
//!
//! Each function makes the guest calls the original makes at that point (value and change
//! callbacks, the tick sound). The one deliberate difference: the original builds the sound
//! parameter block with 0x45390 and plays it with 0x45820 inline; the port calls 0x21780, which is
//! exactly that pair (disassembly 0x21780..0x217A1). Tier 1: tests/small_diff_widgets.rs replays
//! goldens recorded from the original XBE under Unicorn (tools/small_diff/specs_ui_widgets.py).

use crate::menu_state::layout::{option, page, row};
use crate::menu_state::{PageRef, read_f32};
use crate::native_page::{GuestUi, original};

/// Guest globals the widgets read.
pub mod global {
    pub const SOUND_MUTED: u32 = 0x70_18B4;
    pub const SOUND_BANK_READY: u32 = 0x7B_A16C;
    /// Pointer to the UI sound handles: `+0` cursor tick, `+8` option change.
    pub const SOUND_HANDLES: u32 = 0x7B_A178;
    /// Viewport contexts, 0x1E0 bytes each; word 0 nonzero = a pointer context is active.
    pub const VIEWPORT_CONTEXTS: u32 = 0x7B_A270;
    pub const VIEWPORT_STRIDE: u32 = 0x1E0;
    /// Scale of the scroll margin (float).
    pub const SCROLL_MARGIN_SCALE: u32 = 0x7B_A188;
    /// Constant of the float value binding (`index * c`).
    pub const FLOAT_BIND_SCALE: u32 = 0x47_5D1C;
    /// Constant of the scroll selection height clamp (float).
    pub const SCROLL_HEIGHT_CLAMP: u32 = 0x47_CFD8;
    /// Style field: scroll margin in rows (float).
    pub const STYLE_SCROLL_MARGIN: u32 = 0x30;
}

fn get(g: &dyn GuestUi, va: u32) -> u32 {
    g.read32(va).unwrap_or(0)
}

fn put_f32(g: &mut dyn GuestUi, va: u32, value: f32) {
    g.write32(va, value.to_bits());
}

/// 0x719D0: the `n`th cursor row (node types 4, 7, 8 on the `+0x8` chain), then `column` steps
/// along its group (`+0xC`, stopping at the last member). 0 when there is none; `n == -1` is none.
pub fn cursor_row_at(g: &dyn GuestUi, page: PageRef, n: u32, column: i32) -> u32 {
    if n == u32::MAX {
        return 0;
    }
    let mut left = n as i32;
    let mut node = page.get(g, page::FIRST_NODE);
    let mut guard = 0;
    while node != 0 && guard < 0x10000 {
        let cursor = crate::menu_state::is_cursor_row(get(g, node + row::TYPE));
        if left <= 0 && cursor {
            break;
        }
        if cursor {
            left -= 1;
        }
        node = get(g, node + row::NEXT_CURSOR);
        guard += 1;
    }
    for _ in 0..column.max(0) {
        if node != 0 {
            let next = get(g, node + row::NEXT_IN_GROUP);
            if next != 0 {
                node = next;
            }
        }
    }
    node
}

/// One navigation step of the list widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Up,
    Down,
    Left,
    Right,
}

/// What a step did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepResult {
    pub cursor: u32,
    pub column: i32,
    /// The cursor or column changed (the tick sound condition).
    pub moved: bool,
    pub tick: bool,
    /// New option index when the cursor row is an option cycler that changed value.
    pub value: Option<i32>,
    pub scrolled: bool,
}

/// The cursor step of 0x7A7B0 (0x7AE13..0x7AFF3) for one direction, plus the scroll to the cursor
/// row (0x7B00B..0x7B03D). Up and down wrap over the page's rows and skip rows flagged 0x4; left
/// and right move the group column the same way and then cycle an option row. `None` when the
/// page takes no keyboard cursor input (no cursor row, flag 0x10000000, or a pointer context on
/// the viewport without flag 0x40000; the original runs 0x777E0 then).
pub fn step(g: &mut dyn GuestUi, page: PageRef, dir: Step) -> Option<StepResult> {
    step_with_pointer_context(g, page, dir, false)
}

/// Native Map Maker input may navigate the typed row cursor while the original
/// viewport pointer context is active. The original game routes that context to
/// its pointer hover path; the Rust UI owns keyboard and pad navigation here.
pub fn step_with_pointer_context(
    g: &mut dyn GuestUi,
    page: PageRef,
    dir: Step,
    native_cursor: bool,
) -> Option<StepResult> {
    let flags = page.get(g, page::FLAGS);
    let viewport = page.get(g, page::VIEWPORT);
    let pointer_context = get(
        g,
        global::VIEWPORT_CONTEXTS + viewport.wrapping_mul(global::VIEWPORT_STRIDE),
    ) != 0;
    if pointer_context && flags & page::FLAG_KEYS_WITH_POINTER == 0 && !native_cursor {
        return None;
    }
    if page.cursor_node(g) == 0 || flags & page::FLAG_NO_CURSOR != 0 {
        return None;
    }
    let cursor_before = page.cursor(g);
    let column_before = page.column(g);
    let count = page.row_count(g) as i32;
    let mut cursor = cursor_before as i32;
    let mut column = column_before;
    let mut node = page.cursor_node(g);
    if matches!(dir, Step::Up | Step::Down) {
        if count <= 0 {
            return None; // the original divides by the row count
        }
        // Bounded where the original would spin forever on a page whose rows are all skipped.
        for _ in 0..count {
            cursor = match dir {
                Step::Up => (count + cursor - 1) % count,
                _ => (cursor + 1) % count,
            };
            node = cursor_row_at(g, page, cursor as u32, column);
            if get(g, node + row::FLAGS) & row::FLAG_SKIP == 0 {
                break;
            }
        }
    }
    let width = get(
        g,
        cursor_row_at(g, page, cursor as u32, 0) + row::GROUP_WIDTH,
    ) as i32;
    if column > width {
        column = width - 1;
        node = cursor_row_at(g, page, cursor as u32, column);
    }
    if matches!(dir, Step::Left | Step::Right) && width > 0 {
        for _ in 0..width {
            column = match dir {
                Step::Left => (column + width - 1) % width,
                _ => (column + 1) % width,
            };
            node = cursor_row_at(g, page, cursor as u32, column);
            if get(g, node + row::FLAGS) & row::FLAG_SKIP == 0 {
                break;
            }
        }
    }
    page.store_cursor(g, cursor as u32, column, node);
    let moved = cursor as u32 != cursor_before || column != column_before;
    let mut tick = false;
    if moved && get(g, global::SOUND_MUTED) == 0 && get(g, global::SOUND_BANK_READY) != 0 {
        let handle = get(g, get(g, global::SOUND_HANDLES));
        g.call(original::PLAY_SOUND, &[handle]);
        tick = true;
    }
    let mut value = None;
    if matches!(dir, Step::Left | Step::Right) {
        let row_id = get(g, node + row::ID);
        if let Some(native_value) = g.native_settings_step(page, row_id, dir) {
            value = native_value;
        } else if get(g, node + row::KIND) == row::KIND_SLIDER {
            value = slider_step(g, node, dir == Step::Right, false);
        } else if get(g, node + row::KIND) == row::KIND_CYCLER
            && get(g, node + row::KIND_FLAGS) & row::KIND_NO_CYCLE == 0
        {
            let before = get(g, node + row::OPTION_INDEX) as i32;
            match dir {
                Step::Left => option_prev(g, node),
                Step::Right => option_next(g, node),
                _ => unreachable!(),
            }
            let after = get(g, node + row::OPTION_INDEX) as i32;
            if after != before {
                value = Some(after);
            }
        }
    }
    let scrolled = scroll_cursor_row_with_pointer_context(g, page, native_cursor);
    Some(StepResult {
        cursor: cursor as u32,
        column,
        moved,
        tick,
        value,
        scrolled,
    })
}

/// 0x7B00B..0x7B03D: scroll the page to its cursor row unless the page opts out (flag 0x8) or the
/// viewport has a pointer context without flag 0x40000. True when 0x73380 ran.
pub fn scroll_cursor_row(g: &mut dyn GuestUi, page: PageRef) -> bool {
    scroll_cursor_row_with_pointer_context(g, page, false)
}

/// Keep a native Map Maker cursor visible even when the page's original pointer
/// context suppresses game-owned keyboard scrolling.
pub fn scroll_cursor_row_with_pointer_context(
    g: &mut dyn GuestUi,
    page: PageRef,
    native_cursor: bool,
) -> bool {
    let node = page.cursor_node(g);
    let flags = page.get(g, page::FLAGS);
    if node == 0 || flags & page::FLAG_NO_SCROLL != 0 {
        return false;
    }
    let viewport = page.get(g, page::VIEWPORT);
    let pointer_context = get(
        g,
        global::VIEWPORT_CONTEXTS + viewport.wrapping_mul(global::VIEWPORT_STRIDE),
    ) != 0;
    if pointer_context && flags & page::FLAG_KEYS_WITH_POINTER == 0 && !native_cursor {
        return false;
    }
    scroll_to_row(g, page.0, node);
    true
}

/// 0x73380: keep `node` inside the page's view by moving the scroll offset (`+0x88`). Rows of
/// the scrolled list carry flag 0x40; the margin is `style+0x30 * [0x7BA188]`. A page whose
/// content fits its view gets both scroll offsets zeroed. Float operations in the original's
/// order (single precision, SSE), so the result is bit for bit the original's.
pub fn scroll_to_row(g: &mut dyn GuestUi, page_va: u32, node: u32) {
    if page_va == 0 || node == 0 {
        return;
    }
    let row_height = read_f32(g, node + row::HEIGHT);
    let view_height =
        read_f32(g, page_va + page::VIEW_BOTTOM) - read_f32(g, page_va + page::VIEW_TOP);
    let row_y = read_f32(g, node + row::Y);
    // Height of the fixed (skipped) list rows above the first plain node before `node`.
    let mut above = 0.0f32;
    let mut walk = get(g, page_va + page::FIRST_NODE);
    let mut guard = 0;
    while walk != 0 && walk != node && guard < 0x10000 {
        let flags = get(g, walk + row::FLAGS);
        if flags & row::FLAG_IN_LIST != 0 {
            if flags & row::FLAG_SKIP == 0 {
                break;
            }
            above += read_f32(g, walk + row::HEIGHT);
        }
        walk = get(g, walk + row::NEXT_CURSOR);
        guard += 1;
    }
    if get(g, node + row::FLAGS) & row::FLAG_IN_LIST == 0 {
        return;
    }
    let content = read_f32(g, page_va + page::CONTENT_HEIGHT);
    // Unordered (NaN) also takes this branch, as the original `jbe` does.
    if content.partial_cmp(&view_height) != Some(std::cmp::Ordering::Greater) {
        put_f32(g, page_va + page::SCROLL_X, 0.0);
        put_f32(g, page_va + page::SCROLL, 0.0);
        return;
    }
    // The original clamps the selection height to `view * c` and then raises it back to the row
    // height, so it is always the row height; kept in the original's form.
    let clamp = view_height * read_f32(g, global::SCROLL_HEIGHT_CLAMP);
    let mut selection = row_height;
    if row_height > clamp {
        selection = clamp;
    }
    if row_height > selection {
        selection = row_height;
    }
    let style = get(g, page_va + page::STYLE);
    let margin_rows = read_f32(g, style + global::STYLE_SCROLL_MARGIN);
    let scale = read_f32(g, global::SCROLL_MARGIN_SCALE);
    let top = row_y - selection - above;
    let current = read_f32(g, page_va + page::SCROLL);
    if current >= top {
        put_f32(g, page_va + page::SCROLL, top);
        let margin = margin_rows * scale;
        if margin > top {
            put_f32(g, page_va + page::SCROLL, 0.0);
            return;
        }
        let max_scroll = content - view_height;
        if top > max_scroll - margin {
            put_f32(g, page_va + page::SCROLL, max_scroll);
        }
    } else {
        let limit = scale * margin_rows + current;
        let bottom = selection + row_height + row_y - view_height;
        if bottom > limit {
            let max_scroll = content - view_height;
            put_f32(g, page_va + page::SCROLL, bottom);
            if bottom > max_scroll - scale * margin_rows {
                put_f32(g, page_va + page::SCROLL, max_scroll);
            }
        }
    }
}

/// 0x75430: select option `index` of an option row: index, previous index, option id, the value
/// binding (int, option id, float or callback; not while the page is closing, states 4 and 5)
/// and the change callback `+0x4C(page, row)`. The first selection after the row was built
/// (previous == -1) only records the index as the value at open.
pub fn option_select(g: &mut dyn GuestUi, node: u32, index: i32) {
    if node == 0 {
        return;
    }
    let old = get(g, node + row::OPTION_INDEX) as i32;
    g.write32(node + row::OPTION_INDEX, index as u32);
    let id = if get(g, node + row::KIND_FLAGS) & row::KIND_NUMERIC != 0 {
        u32::MAX
    } else {
        let mut entry = get(g, node + row::OPTIONS);
        for _ in 0..index.max(0) {
            if entry != 0 {
                entry = get(g, entry + option::NEXT);
            }
        }
        get(g, entry + option::ID)
    };
    let previous = get(g, node + row::OPTION_PREVIOUS) as i32;
    g.write32(node + row::OPTION_ID, id);
    if previous == -1 {
        let page_va = get(g, node + row::PAGE);
        let on_change = get(g, node + row::ON_CHANGE);
        if index != -1 && on_change != 0 {
            g.call(on_change, &[page_va, node]);
        }
        let now = get(g, node + row::OPTION_INDEX);
        g.write32(node + row::OPTION_AT_OPEN, now);
        g.write32(node + row::OPTION_PREVIOUS, now);
        return;
    }
    if old == index {
        return;
    }
    let page_va = get(g, node + row::PAGE);
    g.write32(node + row::OPTION_PREVIOUS, old as u32);
    let state = get(g, page_va + page::STATE);
    if state != 4 && state != 5 {
        let bind = get(g, node + row::BIND_FLAGS);
        let target = get(g, node + row::VALUE);
        if bind & row::BIND_INT != 0 {
            let value = if bind & row::BIND_BY_ID != 0 {
                id
            } else {
                index as u32
            };
            g.write32(target, value);
        }
        let now = get(g, node + row::OPTION_INDEX) as i32;
        if bind & row::BIND_FLOAT != 0 {
            let scaled = now as f32 * read_f32(g, global::FLOAT_BIND_SCALE);
            put_f32(g, target, scaled);
        }
        if bind & row::BIND_CALL != 0 {
            g.call(target, &[now as u32]);
        }
    }
    // 0x75430 reloads both fields after the value callback. The callback can
    // update the row's change handler or owner page, so keep those reads after
    // the effect instead of carrying stale guest addresses across it.
    let on_change = get(g, node + row::ON_CHANGE);
    if on_change != 0 {
        let page_va = get(g, node + row::PAGE);
        g.call(on_change, &[page_va, node]);
    }
}

/// Which way an option row steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Way {
    Next,
    Prev,
}

/// 0x755D0 (next) / 0x756A0 (previous): the next enabled option with wrap-around, or one numeric
/// step inside `[min, max]`; then the option-change sound and 0x75430. A greyed row does nothing.
/// The row's "changed since open" flag (`+0x2C` bit 8) follows `index != index at open`.
fn option_step(g: &mut dyn GuestUi, node: u32, way: Way) {
    if get(g, node + row::FLAGS) & row::FLAG_GREYED != 0 {
        return;
    }
    let index = get(g, node + row::OPTION_INDEX) as i32;
    let target = if get(g, node + row::KIND_FLAGS) & row::KIND_NUMERIC != 0 {
        let step = get(g, node + row::NUMERIC_STEP) as i32;
        match way {
            Way::Next => {
                let to = index.wrapping_add(step);
                (to <= get(g, node + row::NUMERIC_MAX) as i32).then_some(to)
            }
            Way::Prev => {
                let to = index.wrapping_sub(step);
                (to >= get(g, node + row::NUMERIC_MIN) as i32).then_some(to)
            }
        }
    } else {
        let count = get(g, node + row::OPTION_COUNT) as i32;
        (count > 0).then(|| {
            let entries = get(g, node + row::OPTIONS);
            let mut at = index;
            // Bounded where the original loops until it meets `index` again.
            for _ in 0..=count {
                at = match way {
                    Way::Next if at + 1 >= count => 0,
                    Way::Next => at + 1,
                    Way::Prev if at - 1 < 0 => count - 1,
                    Way::Prev => at - 1,
                };
                if at == index {
                    break;
                }
                let flags = get(
                    g,
                    entries.wrapping_add((at as u32).wrapping_mul(option::STRIDE)) + option::FLAGS,
                );
                if flags & option::FLAG_DISABLED == 0 {
                    break;
                }
            }
            at
        })
    };
    change_value(g, node, target);
}

/// The tail of 0x755D0 / 0x756A0: the option-change sound and 0x75430 when there is a new value, then the row's
/// "changed since open" flag (`+0x2C` bit 8) follows `index != index at open`.
fn change_value(g: &mut dyn GuestUi, node: u32, target: Option<i32>) {
    if let Some(to) = target {
        if get(g, global::SOUND_BANK_READY) != 0 {
            let handle = get(g, get(g, global::SOUND_HANDLES) + 8);
            g.call(original::PLAY_SOUND, &[handle]);
        }
        option_select(g, node, to);
    }
    let bind = get(g, node + row::BIND_FLAGS);
    let changed = get(g, node + row::OPTION_INDEX) != get(g, node + row::OPTION_AT_OPEN);
    let bind = if changed {
        bind | row::BIND_CHANGED
    } else {
        bind & !row::BIND_CHANGED
    };
    g.write32(node + row::BIND_FLAGS, bind);
}

/// 0x755D0: the next enabled option (wrapping) or one numeric step up.
pub fn option_next(g: &mut dyn GuestUi, node: u32) {
    option_step(g, node, Way::Next);
}

/// 0x756A0: the previous enabled option (wrapping) or one numeric step down.
pub fn option_prev(g: &mut dyn GuestUi, node: u32) {
    option_step(g, node, Way::Prev);
}

pub use tsfp_ui_row_input::{SLIDER_FREE_STEP, slider_target};

/// T3809: one [`slider_target`] step of a kind 7 slider row, written as [`option_next`] writes a value (0x75430 with the
/// float binding `value * 0.01`, sound, changed flag). A greyed row does nothing. The new value, or None.
pub fn slider_step(g: &mut dyn GuestUi, node: u32, forward: bool, wrap: bool) -> Option<i32> {
    let greyed = get(g, node + row::FLAGS) & row::FLAG_GREYED != 0;
    if node == 0 || greyed || get(g, node + row::KIND) != row::KIND_SLIDER {
        return None;
    }
    let value = get(g, node + row::OPTION_INDEX) as i32;
    let ticks = (get(g, node + row::SLIDER_TICKS_WORD) >> 16) as u16;
    let target = slider_target(value, ticks, forward, wrap);
    (target != value).then(|| {
        change_value(g, node, Some(target));
        get(g, node + row::OPTION_INDEX) as i32
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu_state::{MenuState, StackEvent};
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
            Some(0)
        }
    }

    /// Slot 2: five cursor rows (ids 20..24), row 2 skipped, row 3 an option cycler with four
    /// options (option 1 disabled).
    fn list() -> (Fake, PageRef, Vec<u32>) {
        let mut f = Fake::default();
        let p = PageRef::slot(2);
        f.mem.insert(p.0, 0x52_0000);
        f.mem.insert(p.0 + page::BUILDER, 0x2C_0000);
        f.mem.insert(p.0 + page::STATE, 3);
        f.mem.insert(p.0 + page::ROW_COUNT, 5);
        f.mem.insert(p.0 + page::FLAGS, page::FLAG_NO_SCROLL);
        let rows: Vec<u32> = (0..5).map(|i| 0x200_0000 + i * 0x200).collect();
        f.mem.insert(p.0 + page::FIRST_NODE, rows[0]);
        for (i, r) in rows.iter().enumerate() {
            f.mem.insert(r + row::TYPE, 7);
            f.mem.insert(r + row::ID, 20 + i as u32);
            f.mem.insert(r + row::GROUP_WIDTH, 1);
            f.mem.insert(r + row::PAGE, p.0);
            let next = rows.get(i + 1).copied().unwrap_or(0);
            f.mem.insert(r + row::NEXT, next);
            f.mem.insert(r + row::NEXT_CURSOR, next);
        }
        f.mem.insert(rows[2] + row::FLAGS, row::FLAG_SKIP);
        let cycler = rows[3];
        let entries = 0x300_0000;
        f.mem.insert(cycler + row::KIND, row::KIND_CYCLER);
        f.mem.insert(cycler + row::OPTION_COUNT, 4);
        f.mem.insert(cycler + row::OPTIONS, entries);
        for i in 0..4u32 {
            let e = entries + i * option::STRIDE;
            f.mem.insert(e + option::ID, 100 + i);
            f.mem.insert(e + option::NEXT, e + option::STRIDE);
        }
        f.mem.insert(
            entries + option::STRIDE + option::FLAGS,
            option::FLAG_DISABLED,
        );
        f.mem.insert(p.0 + page::CURSOR_ROW, rows[0]);
        f.mem.insert(global::SOUND_BANK_READY, 1);
        f.mem.insert(global::SOUND_HANDLES, 0x900);
        f.mem.insert(0x900, 0x51);
        f.mem.insert(0x908, 0x52);
        (f, p, rows)
    }

    #[test]
    fn up_and_down_wrap_and_skip() {
        let (mut f, p, rows) = list();
        let r = step(&mut f, p, Step::Up).unwrap();
        assert_eq!((r.cursor, r.moved, r.tick), (4, true, true));
        assert_eq!(f.mem[&(p.0 + page::CURSOR_ROW)], rows[4]);
        assert_eq!(f.mem[&(p.0 + page::CURSOR_ID)], 24);
        step(&mut f, p, Step::Down).unwrap();
        step(&mut f, p, Step::Down).unwrap();
        let r = step(&mut f, p, Step::Down).unwrap();
        assert_eq!(r.cursor, 3, "row 2 is skipped");
        assert_eq!(f.calls[0], (original::PLAY_SOUND, vec![0x51]));
    }

    #[test]
    fn left_right_cycle_the_option_row_with_wrap_and_disabled_skip() {
        let (mut f, p, rows) = list();
        f.mem.insert(p.0 + page::CURSOR, 3);
        f.mem.insert(p.0 + page::CURSOR_ROW, rows[3]);
        let cycler = rows[3];
        // First selection after build: records the value at open.
        f.mem.insert(cycler + row::OPTION_PREVIOUS, u32::MAX);
        option_select(&mut f, cycler, 0);
        assert_eq!(f.mem[&(cycler + row::OPTION_AT_OPEN)], 0);
        let r = step(&mut f, p, Step::Right).unwrap();
        assert_eq!(r.value, Some(2), "option 1 is disabled");
        assert!(!r.moved);
        assert_eq!(f.mem[&(cycler + row::OPTION_ID)], 102);
        assert_eq!(f.mem[&(cycler + row::BIND_FLAGS)], row::BIND_CHANGED);
        assert!(f.calls.contains(&(original::PLAY_SOUND, vec![0x52])));
        step(&mut f, p, Step::Right).unwrap();
        let r = step(&mut f, p, Step::Right).unwrap();
        assert_eq!(r.value, Some(0), "wraps to 0");
        assert_eq!(
            f.mem[&(cycler + row::BIND_FLAGS)],
            0,
            "back at the open value"
        );
        let r = step(&mut f, p, Step::Left).unwrap();
        assert_eq!(r.value, Some(3));
    }

    /// T3809: the Grid Intensity row as `0x7D680` builds it: kind 7, numeric, value 50, float binding.
    fn slider_list() -> (Fake, PageRef, u32) {
        let (mut f, p, rows) = list();
        let slider = rows[1];
        f.mem.insert(slider + row::KIND, row::KIND_SLIDER);
        f.mem
            .insert(slider + row::KIND_FLAGS, row::KIND_NUMERIC | 0x100);
        f.mem.insert(slider + row::OPTION_INDEX, 50);
        f.mem.insert(slider + row::OPTION_AT_OPEN, 50);
        f.mem.insert(slider + row::OPTION_PREVIOUS, 50);
        f.mem.insert(slider + row::BIND_FLAGS, row::BIND_FLOAT);
        f.mem.insert(slider + row::VALUE, 0x7000);
        f.mem.insert(global::FLOAT_BIND_SCALE, 0.01f32.to_bits());
        f.mem.insert(p.0 + page::CURSOR, 1);
        f.mem.insert(p.0 + page::CURSOR_ROW, slider);
        (f, p, slider)
    }

    #[test]
    fn left_right_step_a_slider_row_and_write_its_float_binding() {
        let (mut f, p, slider) = slider_list();
        let r = step(&mut f, p, Step::Right).unwrap();
        assert_eq!(r.value, Some(60), "kind 7 rows move on left/right");
        assert_eq!(f.mem[&(slider + row::OPTION_INDEX)], 60);
        let bound = f32::from_bits(f.mem[&0x7000]);
        assert!(
            (bound - 0.6).abs() < 1e-6,
            "float binding value * 0.01, got {bound}"
        );
        assert_eq!(
            f.mem[&(slider + row::BIND_FLAGS)] & row::BIND_CHANGED,
            row::BIND_CHANGED
        );
        assert!(f.calls.contains(&(original::PLAY_SOUND, vec![0x52])));
        assert_eq!(step(&mut f, p, Step::Left).unwrap().value, Some(50));
        assert_eq!(f.mem[&(slider + row::BIND_FLAGS)] & row::BIND_CHANGED, 0);
    }

    #[test]
    fn slider_steps_clamp_on_keys_and_wrap_on_activation() {
        let (mut f, p, slider) = slider_list();
        f.mem.insert(slider + row::OPTION_INDEX, 100);
        assert_eq!(
            step(&mut f, p, Step::Right).unwrap().value,
            None,
            "keys clamp at 100"
        );
        assert_eq!(
            slider_step(&mut f, slider, true, true),
            Some(0),
            "activation wraps"
        );
        f.mem.insert(slider + row::FLAGS, row::FLAG_GREYED);
        assert_eq!(
            slider_step(&mut f, slider, true, true),
            None,
            "a greyed slider stays"
        );
    }

    #[test]
    fn option_select_binds_values() {
        let (mut f, _, rows) = list();
        let cycler = rows[3];
        f.mem
            .insert(cycler + row::BIND_FLAGS, row::BIND_INT | row::BIND_BY_ID);
        f.mem.insert(cycler + row::VALUE, 0x4000);
        f.mem.insert(cycler + row::ON_CHANGE, 0x2D_0000);
        option_select(&mut f, cycler, 3);
        assert_eq!(f.mem[&0x4000], 103);
        assert_eq!(f.mem[&(cycler + row::OPTION_PREVIOUS)], 0);
        assert_eq!(
            f.calls.last(),
            Some(&(0x2D_0000, vec![PageRef::slot(2).0, cycler]))
        );
        // Closing page: no binding write, the change callback still runs.
        f.mem.insert(PageRef::slot(2).0 + page::STATE, 4);
        f.mem.insert(0x4000, 0);
        option_select(&mut f, cycler, 2);
        assert_eq!(f.mem[&0x4000], 0);
        // Numeric rows step inside their range.
        f.mem.insert(cycler + row::KIND_FLAGS, row::KIND_NUMERIC);
        f.mem.insert(cycler + row::NUMERIC_MAX, 5);
        f.mem.insert(cycler + row::NUMERIC_STEP, 3);
        f.mem.insert(PageRef::slot(2).0 + page::STATE, 3);
        option_next(&mut f, cycler);
        assert_eq!(f.mem[&(cycler + row::OPTION_INDEX)], 5);
        assert_eq!(f.mem[&(cycler + row::OPTION_ID)], u32::MAX);
        option_next(&mut f, cycler);
        assert_eq!(f.mem[&(cycler + row::OPTION_INDEX)], 5, "8 > max");
    }

    #[test]
    fn scroll_follows_the_cursor_row() {
        let mut f = Fake::default();
        let p = 0x6F_0000;
        let style = 0x70_0000;
        f.mem.insert(p + page::STYLE, style);
        f.mem.insert(p + page::VIEW_TOP, 0f32.to_bits());
        f.mem.insert(p + page::VIEW_BOTTOM, 100f32.to_bits());
        f.mem.insert(p + page::CONTENT_HEIGHT, 300f32.to_bits());
        f.mem
            .insert(style + global::STYLE_SCROLL_MARGIN, 1f32.to_bits());
        f.mem.insert(global::SCROLL_MARGIN_SCALE, 10f32.to_bits());
        f.mem.insert(global::SCROLL_HEIGHT_CLAMP, 0.5f32.to_bits());
        let node = 0x80_0000;
        f.mem.insert(p + page::FIRST_NODE, node);
        f.mem.insert(node + row::FLAGS, row::FLAG_IN_LIST);
        f.mem.insert(node + row::Y, 150f32.to_bits());
        f.mem.insert(node + row::HEIGHT, 20f32.to_bits());
        scroll_to_row(&mut f, p, node);
        // bottom = 20 + 20 + 150 - 100 = 90 > 0 + 10
        assert_eq!(f32::from_bits(f.mem[&(p + page::SCROLL)]), 90.0);
        // Fits: both offsets zero.
        f.mem.insert(p + page::CONTENT_HEIGHT, 50f32.to_bits());
        scroll_to_row(&mut f, p, node);
        assert_eq!(f.mem[&(p + page::SCROLL)], 0);
    }

    #[test]
    fn menu_state_reports_stack_events() {
        let (mut f, p, _) = list();
        let before = MenuState::load(&f);
        assert_eq!(before.pages.len(), 1);
        assert_eq!(before.top().unwrap().rows.len(), 5);
        assert!(before.top().unwrap().rows[3].is_cycler());
        step(&mut f, p, Step::Down).unwrap();
        let q = PageRef::slot(3);
        f.mem.insert(q.0, 0x52_0000);
        f.mem.insert(q.0 + page::BUILDER, 0x2C_1000);
        let after = MenuState::load(&f);
        assert_eq!(
            before.events(&after),
            vec![
                StackEvent::Cursor {
                    slot: 2,
                    builder: 0x2C_0000,
                    from: 0,
                    to: 1
                },
                StackEvent::Pushed {
                    slot: 3,
                    builder: 0x2C_1000
                },
            ]
        );
    }
}
