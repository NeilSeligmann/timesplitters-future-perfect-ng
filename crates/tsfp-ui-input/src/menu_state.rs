// SPDX-License-Identifier: GPL-3.0-or-later
//! T2310 UI-12: the Rust-owned, typed menu state of the widget framework.
//!
//! The original keeps the menu in 20 page records (table 0x6FEBE8, 0x23C bytes each) and a node
//! table of rows (0x10C bytes each). While the page and row handlers are still lifted they read
//! those records, so the guest record stays the store the handlers see. The Rust side owns a typed
//! snapshot of it ([`MenuState`], loaded once per frame boundary) and writes its changes through
//! named fields only (`PageState::store_*`, the widget ports in [`crate::widgets`]). Every guest
//! offset of the framework lives in [`layout`]; nothing else in the crate spells one.
//!
//! The page stack is the ordered list of live slots. [`MenuState::events`] compares two
//! snapshots and reports pushes, pops, state and cursor changes, which is the per-frame page
//! transition record the Tier 2 trace compares (tools/t2188_page_transitions.py).

use crate::native_page::GuestUi;

/// Field offsets of the original records (docs/t1640-menu-state-findings.md section 3.2,
/// docs/t-ui-framework.md section 1.2, disassembly of 0x7A7B0, 0x75430, 0x755D0, 0x73380).
pub mod layout {
    /// The page table: 20 page records of 0x23C bytes.
    pub const PAGE_TABLE: u32 = 0x6F_EBE8;
    pub const PAGE_STRIDE: u32 = 0x23C;
    pub const PAGE_SLOTS: u32 = 20;

    /// Page record fields.
    pub mod page {
        pub const VTABLE: u32 = 0x00;
        /// The page id (the context stack stores it, 0x70CC0).
        pub const ID: u32 = 0x04;
        pub const BUILDER: u32 = 0x08;
        pub const STYLE: u32 = 0x1C;
        pub const PARENT: u32 = 0x2C;
        pub const VIEWPORT: u32 = 0x38;
        pub const FLAGS: u32 = 0x40;
        pub const CONTENT_HEIGHT: u32 = 0x5C;
        pub const VIEW_TOP: u32 = 0x6C;
        pub const VIEW_BOTTOM: u32 = 0x70;
        pub const SCROLL_X: u32 = 0x84;
        pub const SCROLL: u32 = 0x88;
        /// The active child page (popup or pushed page), linked by 0x773B0.
        pub const ACTIVE_CHILD: u32 = 0x170;
        pub const FIRST_NODE: u32 = 0x178;
        pub const ROW_COUNT: u32 = 0x180;
        pub const CURSOR: u32 = 0x184;
        pub const COLUMN: u32 = 0x188;
        pub const CURSOR_ID: u32 = 0x18C;
        pub const CURSOR_ID_ORDINAL: u32 = 0x190;
        pub const CURSOR_ROW: u32 = 0x198;
        pub const STATE: u32 = 0x1B0;
        pub const ON_ACCEPT: u32 = 0x1BC;
        pub const ON_BACK: u32 = 0x1C0;
        pub const AFTER_ACCEPT: u32 = 0x1E0;
        pub const AFTER_BACK: u32 = 0x1E4;
        pub const BACK_ID: u32 = 0x1E8;
        /// `FLAGS` bit: B pops the context when the page has no back handler.
        pub const FLAG_BACK_POPS: u32 = 0x2000;
        /// `FLAGS` bit: the page does not scroll to the cursor row.
        pub const FLAG_NO_SCROLL: u32 = 0x8;
        /// 0x7A7B0 lets this page receive input when its deepest active child is immediate.
        pub const FLAG_INPUT_WITH_CHILD: u32 = 0x40_0000;
        /// `FLAGS` bit: keyboard navigation even while the viewport has a pointer context.
        pub const FLAG_KEYS_WITH_POINTER: u32 = 0x4_0000;
        /// `FLAGS` bit: the page takes no cursor input.
        pub const FLAG_NO_CURSOR: u32 = 0x1000_0000;
        /// `FLAGS` bit: pad X runs the style's shortcut `[[page + STYLE] + STYLE_SHORTCUT]` (0x7A7B0); 0x75B70 keeps it
        /// only when it shows the X "Player Progress" prompt (T3817).
        pub const FLAG_X_SHORTCUT: u32 = 0x800_0000;
        /// Style (`STYLE`) field: the X shortcut callback (no arguments).
        pub const STYLE_SHORTCUT: u32 = 0xA8;
    }

    /// Row (node) fields.
    pub mod row {
        pub const TYPE: u32 = 0x00;
        pub const NEXT: u32 = 0x04;
        pub const NEXT_CURSOR: u32 = 0x08;
        pub const NEXT_IN_GROUP: u32 = 0x0C;
        pub const GROUP_WIDTH: u32 = 0x10;
        pub const ID: u32 = 0x14;
        pub const FLAGS: u32 = 0x28;
        pub const BIND_FLAGS: u32 = 0x2C;
        pub const ON_ACTIVATE: u32 = 0x38;
        pub const VALUE: u32 = 0x48;
        pub const ON_CHANGE: u32 = 0x4C;
        pub const Y: u32 = 0x68;
        pub const HEIGHT: u32 = 0x70;
        pub const PAGE: u32 = 0x88;
        pub const NUMERIC_MAX: u32 = 0x9C;
        pub const NUMERIC_MIN: u32 = 0xA0;
        pub const NUMERIC_STEP: u32 = 0xA8;
        pub const KIND: u32 = 0xB4;
        pub const KIND_FLAGS: u32 = 0xB8;
        pub const OPTION_COUNT: u32 = 0xC4;
        pub const OPTION_INDEX: u32 = 0xC8;
        pub const OPTION_ID: u32 = 0xCC;
        pub const OPTION_PREVIOUS: u32 = 0xD0;
        pub const OPTION_AT_OPEN: u32 = 0xD4;
        pub const OPTIONS: u32 = 0xD8;
        pub const NO_AFTER_ACCEPT: u32 = 0x108;

        /// `FLAGS`: greyed (A denied, the value does not change).
        pub const FLAG_GREYED: u32 = 0x1;
        /// `FLAGS`: the cursor skips this row.
        pub const FLAG_SKIP: u32 = 0x4;
        /// `FLAGS`: the row is part of the scrolled list.
        pub const FLAG_IN_LIST: u32 = 0x40;
        /// `BIND_FLAGS`: call `VALUE(index)` after a change.
        pub const BIND_CALL: u32 = 0x4;
        /// `BIND_FLAGS`: the value changed since the page opened.
        pub const BIND_CHANGED: u32 = 0x8;
        /// `BIND_FLAGS`: write the index (or the option id) through the `VALUE` int pointer.
        pub const BIND_INT: u32 = 0x40;
        /// `BIND_FLAGS`: write `index * [0x475D1C]` through the `VALUE` float pointer.
        pub const BIND_FLOAT: u32 = 0x80;
        /// `BIND_FLAGS` (with `BIND_INT`): write the option id instead of the index.
        pub const BIND_BY_ID: u32 = 0x100;
        /// `KIND_FLAGS`: a numeric row (min, max, step) instead of an option list.
        pub const KIND_NUMERIC: u32 = 0x20_0000;
        /// `KIND_FLAGS`: left/right do not cycle the value.
        pub const KIND_NO_CYCLE: u32 = 0x40_0000;
        /// `KIND`: the option cycler.
        pub const KIND_CYCLER: u32 = 1;
        /// `KIND`: the slider (T3809); u16 `+0x9A` (high half of `+0x98`) its tick count.
        pub use tsfp_ui_row_input::{KIND_SLIDER, SLIDER_MAX};
        pub const SLIDER_TICKS_WORD: u32 = 0x98;
    }

    /// The per-viewport context record (pointer table 0x7BA264, stride 0x1E0): the page stack
    /// of the widget framework (0x70CC0 append, 0x79B70 push, 0x79C00 pop).
    pub mod context {
        pub const TABLE: u32 = 0x7B_A264;
        pub const STRIDE: u32 = 0x1E0;
        /// Number of stacked page ids.
        pub const COUNT: u32 = 0x00;
        /// Stacked page ids (`IDS + i * 4`).
        pub const IDS: u32 = 0x04;
        /// What the stack saves per entry: 0 the cursor (`page + 0x184`), 1 the scroll (`page + 0x88`).
        pub const SAVE_MODE: u32 = 0xF4;
        /// Saved cursor or scroll per entry (`SAVED + i * 4`).
        pub const SAVED: u32 = 0xF8;
        /// The stack holds `(SAVE_MODE - IDS) / 4` entries.
        pub const CAPACITY: u32 = 60;
    }

    /// Option entry fields (array of 0x14 byte entries at row `OPTIONS`, also chained by `NEXT`).
    pub mod option {
        pub const STRIDE: u32 = 0x14;
        pub const ID: u32 = 0x08;
        pub const FLAGS: u32 = 0x0C;
        pub const NEXT: u32 = 0x10;
        pub const FLAG_DISABLED: u32 = 0x1;
    }
}

use layout::{option, page, row};

/// Node types that are cursor rows.
pub fn is_cursor_row(node_type: u32) -> bool {
    matches!(node_type, 4 | 7 | 8)
}

pub(crate) fn read_f32(g: &dyn GuestUi, va: u32) -> f32 {
    f32::from_bits(g.read32(va).unwrap_or(0))
}

/// One option of an option row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptionEntry {
    pub id: u32,
    pub disabled: bool,
}

/// The value part of an option row (kind 1 cycler or numeric row).
#[derive(Clone, Debug, PartialEq)]
pub struct OptionState {
    pub kind: u32,
    pub kind_flags: u32,
    pub index: i32,
    pub id: u32,
    pub previous: i32,
    pub at_open: i32,
    pub bind_flags: u32,
    /// `Some((min, max, step))` on a numeric row.
    pub numeric: Option<(i32, i32, i32)>,
    pub entries: Vec<OptionEntry>,
}

impl OptionState {
    pub fn changed_since_open(&self) -> bool {
        self.bind_flags & row::BIND_CHANGED != 0
    }
}

/// One cursor row of a page.
#[derive(Clone, Debug, PartialEq)]
pub struct RowState {
    pub node: u32,
    pub node_type: u32,
    pub id: u32,
    pub flags: u32,
    pub has_handler: bool,
    pub group_width: i32,
    pub y: f32,
    pub height: f32,
    pub option: Option<OptionState>,
}

impl RowState {
    pub fn greyed(&self) -> bool {
        self.flags & row::FLAG_GREYED != 0
    }
    pub fn navigable(&self) -> bool {
        self.flags & row::FLAG_SKIP == 0
    }
    pub fn is_cycler(&self) -> bool {
        self.option
            .as_ref()
            .is_some_and(|o| o.kind == row::KIND_CYCLER && o.kind_flags & row::KIND_NO_CYCLE == 0)
    }

    pub fn load(g: &dyn GuestUi, node: u32) -> Self {
        let get = |field: u32| g.read32(node + field).unwrap_or(0);
        let kind = get(row::KIND);
        let kind_flags = get(row::KIND_FLAGS);
        let option = (kind != 0).then(|| {
            let count = get(row::OPTION_COUNT) as i32;
            let base = get(row::OPTIONS);
            let entries = if base == 0 {
                Vec::new()
            } else {
                (0..count.clamp(0, 256) as u32)
                    .map(|i| {
                        let at = base + i * option::STRIDE;
                        OptionEntry {
                            id: g.read32(at + option::ID).unwrap_or(0),
                            disabled: g.read32(at + option::FLAGS).unwrap_or(0)
                                & option::FLAG_DISABLED
                                != 0,
                        }
                    })
                    .collect()
            };
            OptionState {
                kind,
                kind_flags,
                index: get(row::OPTION_INDEX) as i32,
                id: get(row::OPTION_ID),
                previous: get(row::OPTION_PREVIOUS) as i32,
                at_open: get(row::OPTION_AT_OPEN) as i32,
                bind_flags: get(row::BIND_FLAGS),
                numeric: (kind_flags & row::KIND_NUMERIC != 0).then(|| {
                    (
                        get(row::NUMERIC_MIN) as i32,
                        get(row::NUMERIC_MAX) as i32,
                        get(row::NUMERIC_STEP) as i32,
                    )
                }),
                entries,
            }
        });
        RowState {
            node,
            node_type: get(row::TYPE),
            id: get(row::ID),
            flags: get(row::FLAGS),
            has_handler: get(row::ON_ACTIVATE) != 0,
            group_width: get(row::GROUP_WIDTH) as i32,
            y: read_f32(g, node + row::Y),
            height: read_f32(g, node + row::HEIGHT),
            option,
        }
    }
}

/// One page record in guest memory (the address of its record).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageRef(pub u32);

impl PageRef {
    pub fn slot(n: u32) -> Self {
        PageRef(layout::PAGE_TABLE + n * layout::PAGE_STRIDE)
    }
    pub fn get(self, g: &dyn GuestUi, field: u32) -> u32 {
        g.read32(self.0 + field).unwrap_or(0)
    }
    pub fn live(self, g: &dyn GuestUi) -> bool {
        self.get(g, page::VTABLE) != 0
    }
    pub fn builder(self, g: &dyn GuestUi) -> u32 {
        self.get(g, page::BUILDER)
    }
    pub fn state(self, g: &dyn GuestUi) -> u32 {
        self.get(g, page::STATE)
    }
    pub fn cursor(self, g: &dyn GuestUi) -> u32 {
        self.get(g, page::CURSOR)
    }
    pub fn column(self, g: &dyn GuestUi) -> i32 {
        self.get(g, page::COLUMN) as i32
    }
    pub fn row_count(self, g: &dyn GuestUi) -> u32 {
        self.get(g, page::ROW_COUNT)
    }
    pub fn cursor_node(self, g: &dyn GuestUi) -> u32 {
        self.get(g, page::CURSOR_ROW)
    }

    /// The `n`th cursor row at the page's current group column (0x71A40).
    pub fn cursor_row(self, g: &dyn GuestUi, n: u32) -> Option<u32> {
        let node = crate::widgets::cursor_row_at(g, self, n, self.column(g));
        (node != 0).then_some(node)
    }

    /// How many nodes before `target` on the creation chain carry the same row id (0x719A0).
    pub fn same_id_ordinal(self, g: &dyn GuestUi, target: u32) -> u32 {
        let id = g.read32(target + row::ID).unwrap_or(0);
        let mut count = 0;
        let mut node = self.get(g, page::FIRST_NODE);
        let mut guard = 0;
        while node != 0 && node != target && guard < 4096 {
            if g.read32(node + row::ID) == Some(id) {
                count += 1;
            }
            node = g.read32(node + row::NEXT).unwrap_or(0);
            guard += 1;
        }
        count
    }

    /// Write the cursor record of the original cursor step (0x7AE5F..0x7AF71): cursor, group
    /// column, cursor row, its id and the same-id ordinal.
    pub fn store_cursor(self, g: &mut dyn GuestUi, cursor: u32, column: i32, node: u32) {
        g.write32(self.0 + page::CURSOR, cursor);
        g.write32(self.0 + page::COLUMN, column as u32);
        g.write32(self.0 + page::CURSOR_ROW, node);
        let id = if node == 0 {
            u32::MAX
        } else {
            g.read32(node + row::ID).unwrap_or(u32::MAX)
        };
        g.write32(self.0 + page::CURSOR_ID, id);
        let ordinal = self.same_id_ordinal(g, node);
        g.write32(self.0 + page::CURSOR_ID_ORDINAL, ordinal);
    }
}

/// The typed snapshot of one live page.
#[derive(Clone, Debug, PartialEq)]
pub struct PageState {
    pub slot: u32,
    pub record: PageRef,
    pub builder: u32,
    pub state: u32,
    pub flags: u32,
    pub viewport: u32,
    pub cursor: u32,
    pub column: i32,
    pub cursor_id: u32,
    pub row_count: u32,
    pub scroll: f32,
    /// The cursor rows in cursor order (column 0 of each group).
    pub rows: Vec<RowState>,
}

impl PageState {
    pub fn load(g: &dyn GuestUi, slot: u32) -> Option<Self> {
        let record = PageRef::slot(slot);
        if !record.live(g) {
            return None;
        }
        let mut rows = Vec::new();
        let mut node = record.get(g, page::FIRST_NODE);
        let mut guard = 0;
        while node != 0 && guard < 4096 {
            if is_cursor_row(g.read32(node + row::TYPE).unwrap_or(0)) {
                rows.push(RowState::load(g, node));
            }
            node = g.read32(node + row::NEXT_CURSOR).unwrap_or(0);
            guard += 1;
        }
        Some(PageState {
            slot,
            record,
            builder: record.builder(g),
            state: record.state(g),
            flags: record.get(g, page::FLAGS),
            viewport: record.get(g, page::VIEWPORT),
            cursor: record.cursor(g),
            column: record.column(g),
            cursor_id: record.get(g, page::CURSOR_ID),
            row_count: record.row_count(g),
            scroll: read_f32(g, record.0 + page::SCROLL),
            rows,
        })
    }

    /// Open or open-for-input (states 2 and 3).
    pub fn accepts_input(&self) -> bool {
        matches!(self.state, 2 | 3)
    }

    pub fn cursor_row(&self) -> Option<&RowState> {
        self.rows.get(self.cursor as usize)
    }
}

/// The page stack: every live slot, bottom (slot 0) to top.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuState {
    pub pages: Vec<PageState>,
}

/// One change between two snapshots, in slot order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackEvent {
    Pushed {
        slot: u32,
        builder: u32,
    },
    Popped {
        slot: u32,
        builder: u32,
    },
    State {
        slot: u32,
        builder: u32,
        from: u32,
        to: u32,
    },
    Cursor {
        slot: u32,
        builder: u32,
        from: u32,
        to: u32,
    },
    Value {
        slot: u32,
        builder: u32,
        row: u32,
        from: i32,
        to: i32,
    },
}

impl MenuState {
    pub fn load(g: &dyn GuestUi) -> Self {
        MenuState {
            pages: (0..layout::PAGE_SLOTS)
                .filter_map(|slot| PageState::load(g, slot))
                .collect(),
        }
    }

    pub fn top(&self) -> Option<&PageState> {
        self.pages.last()
    }

    /// The top live page built by `builder`.
    pub fn find(&self, builder: u32) -> Option<&PageState> {
        self.pages.iter().rev().find(|p| p.builder == builder)
    }

    fn slot(&self, slot: u32) -> Option<&PageState> {
        self.pages.iter().find(|p| p.slot == slot)
    }

    /// What changed from `self` (older) to `newer`.
    pub fn events(&self, newer: &MenuState) -> Vec<StackEvent> {
        let mut out = Vec::new();
        for slot in 0..layout::PAGE_SLOTS {
            match (self.slot(slot), newer.slot(slot)) {
                (Some(old), None) => out.push(StackEvent::Popped {
                    slot,
                    builder: old.builder,
                }),
                (None, Some(new)) => out.push(StackEvent::Pushed {
                    slot,
                    builder: new.builder,
                }),
                (Some(old), Some(new)) if old.builder != new.builder => {
                    out.push(StackEvent::Popped {
                        slot,
                        builder: old.builder,
                    });
                    out.push(StackEvent::Pushed {
                        slot,
                        builder: new.builder,
                    });
                }
                (Some(old), Some(new)) => {
                    let builder = new.builder;
                    if old.state != new.state {
                        out.push(StackEvent::State {
                            slot,
                            builder,
                            from: old.state,
                            to: new.state,
                        });
                    }
                    if old.cursor != new.cursor {
                        out.push(StackEvent::Cursor {
                            slot,
                            builder,
                            from: old.cursor,
                            to: new.cursor,
                        });
                    }
                    for (i, (a, b)) in old.rows.iter().zip(&new.rows).enumerate() {
                        if let (Some(a), Some(b)) = (&a.option, &b.option)
                            && a.index != b.index
                        {
                            out.push(StackEvent::Value {
                                slot,
                                builder,
                                row: i as u32,
                                from: a.index,
                                to: b.index,
                            });
                        }
                    }
                }
                (None, None) => {}
            }
        }
        out
    }
}
