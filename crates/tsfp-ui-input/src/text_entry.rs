// SPDX-License-Identifier: GPL-3.0-or-later
//! T2169: host keyboard typing into the original on-screen keyboard page (Enter Name, T2197 widget).
//!
//! The host keyboard does not write the widget: every typed key becomes closed-loop pad taps on the original
//! page (grid moves, the Shift and Symbols page keys, A on the character key, Y for delete, Done, Back), so the
//! original key handler, the T2197 insertion (0x7DB20: charset, length limit, font width rule, tick sounds,
//! dirty call) and the done handler run exactly as for a pad. The driver only READS the live page slot (grid
//! row +0x184, column +0x188, key slot +0x18C, flags +0x228, text length, limit +0x230) at the frame boundary
//! and plans the next tap from it. Grid geometry, wrap rules and page keys are MEASURED on the profile name page
//! (docs/t2169-keyboard-text-entry.md). Rule 19 host-boundary exception: no guest memory write, no lifted code.
//! T2310: the live shell no longer uses this driver (typing is native, `crate::native_text`); it stays as the
//! generator of the pad reference route for the Tier 2 comparison (measurement tool).

use std::collections::VecDeque;
use tsfp_text_field::keyboard::{FieldFlags, KeyGrid, KeyTable, select_table};

use crate::text_route::{MAX_TAPS_PER_KEY, ends_key};

/// One typed key, already mapped from the host keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKey {
    /// A character byte (ASCII or Latin-1 as the game's tables hold it).
    Char(u8),
    /// Backspace: the page's delete (Y button, 0x7C410).
    Backspace,
    /// Enter: the page's Done key.
    Done,
    /// Escape: the pad BACK button, which leaves the page without committing (MEASURED).
    Cancel,
}

/// Pad buttons the typing driver taps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextButton {
    Up,
    Down,
    Left,
    Right,
    A,
    Y,
    Back,
}

impl TextButton {
    /// SDL3 gamepad button index used by the `tsfp_wi_*` queue (tsfp-bevy events.rs table).
    pub fn sdl_index(self) -> u8 {
        match self {
            TextButton::A => 0,
            TextButton::Y => 3,
            TextButton::Back => 4,
            TextButton::Up => 11,
            TextButton::Down => 12,
            TextButton::Left => 13,
            TextButton::Right => 14,
        }
    }
    /// Route macro name of `tools/t2116_route_build.py`.
    pub fn macro_name(self) -> &'static str {
        match self {
            TextButton::Up => "UP",
            TextButton::Down => "DOWN",
            TextButton::Left => "LEFT",
            TextButton::Right => "RIGHT",
            TextButton::A => "A",
            TextButton::Y => "Y",
            TextButton::Back => "BACK",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEdge {
    Press(TextButton),
    Release(TextButton),
}

/// The page keys of the left column, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageKey {
    Done,
    Shift,
    Caps,
    Symbols,
    Del,
}

/// A grid position: `col` 0 is the page-key column, character keys use columns 1..=columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub row: u32,
    pub col: u32,
}

/// Geometry of one on-screen keyboard grid, as the page moves its highlight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridLayout {
    /// Character keys per full row.
    pub columns: u32,
    /// Number of character keys; the last one is the wide space key alone on the bottom row.
    pub keys: u32,
    /// Page keys of column 0, top first.
    pub page_keys: [PageKey; 5],
    /// Page row word (+0x184) of grid row 0.
    pub row_base: u32,
}

impl GridLayout {
    /// The letters family grid of the profile name page (29 keys, 7 per row, space alone on row 4). MEASURED:
    /// rows +0x184 2..6, column +0x188 0..7, page keys Done (slot 0), Shift 0x3E8, Caps 0x3E9, Symbols 0x3EA, Del 0x3EB.
    pub const LETTERS: GridLayout = GridLayout {
        columns: 7,
        keys: 29,
        page_keys: [
            PageKey::Done,
            PageKey::Shift,
            PageKey::Caps,
            PageKey::Symbols,
            PageKey::Del,
        ],
        row_base: 2,
    };

    /// Row of the space key (the short bottom row).
    pub fn space_row(&self) -> u32 {
        (self.keys - 1) / self.columns
    }

    /// Cell of character slot `slot`.
    pub fn key_cell(&self, slot: u32) -> Cell {
        if slot + 1 == self.keys {
            return Cell {
                row: self.space_row(),
                col: 1,
            };
        }
        Cell {
            row: slot / self.columns,
            col: 1 + slot % self.columns,
        }
    }

    pub fn page_key_cell(&self, key: PageKey) -> Cell {
        let row = self.page_keys.iter().position(|k| *k == key).unwrap_or(0) as u32;
        Cell { row, col: 0 }
    }

    /// The page key at a column-0 cell.
    pub fn page_key_at(&self, cell: Cell) -> Option<PageKey> {
        (cell.col == 0)
            .then(|| self.page_keys.get(cell.row as usize).copied())
            .flatten()
    }

    /// Character slot at a cell, `None` for the page-key column.
    pub fn slot_at(&self, cell: Cell) -> Option<u32> {
        if cell.col == 0 {
            return None;
        }
        if cell.row == self.space_row() {
            return Some(self.keys - 1);
        }
        Some(cell.row * self.columns + cell.col - 1)
    }

    /// Cell of the live row (+0x184) and column (+0x188) words, `None` outside the grid.
    pub fn live_cell(&self, row_word: u32, col_word: u32) -> Option<Cell> {
        let row = row_word.checked_sub(self.row_base)?;
        let rows = self.space_row() + 1;
        (row < rows && col_word <= self.columns).then_some(Cell { row, col: col_word })
    }

    /// Where `button` moves the highlight from `from` (MEASURED wrap rules; `None` for an unmeasured move, which the
    /// planner avoids). Rows 0..space_row-1 wrap left/right over columns 0..=columns; the bottom row holds Del and
    /// space and toggles between them; column 0 wraps up/down; Down from the last full row or Up from row 0 of a
    /// letter column is the space key, Down from space is the first letter, Up from space the first letter column.
    pub fn neighbor(&self, from: Cell, button: TextButton) -> Option<Cell> {
        let width = self.columns + 1;
        let space = self.space_row();
        let cell = |row, col| Some(Cell { row, col });
        match button {
            TextButton::Right | TextButton::Left if from.row == space => {
                cell(space, 1 - from.col.min(1))
            }
            TextButton::Right => cell(from.row, (from.col + 1) % width),
            TextButton::Left => cell(from.row, (from.col + width - 1) % width),
            TextButton::Down if from.col == 0 => cell((from.row + 1) % (space + 1), 0),
            TextButton::Up if from.col == 0 => cell((from.row + space) % (space + 1), 0),
            TextButton::Down if from.row == space => cell(0, 1),
            TextButton::Down if from.row + 1 == space => cell(space, 1),
            TextButton::Down => cell(from.row + 1, from.col),
            TextButton::Up if from.row == space => cell(space - 1, 1),
            TextButton::Up if from.row == 0 => None,
            TextButton::Up => cell(from.row - 1, from.col),
            _ => None,
        }
    }

    /// First move of a shortest path from `from` to `to` (breadth-first over the measured moves).
    pub fn step(&self, from: Cell, to: Cell) -> Option<TextButton> {
        if from == to {
            return None;
        }
        const MOVES: [TextButton; 4] = [
            TextButton::Right,
            TextButton::Left,
            TextButton::Down,
            TextButton::Up,
        ];
        let mut seen = vec![from];
        let mut frontier: VecDeque<(Cell, TextButton)> = VecDeque::new();
        for button in MOVES {
            if let Some(next) = self.neighbor(from, button) {
                frontier.push_back((next, button));
            }
        }
        while let Some((cell, first)) = frontier.pop_front() {
            if cell == to {
                return Some(first);
            }
            if seen.contains(&cell) {
                continue;
            }
            seen.push(cell);
            for button in MOVES {
                if let Some(next) = self.neighbor(cell, button) {
                    frontier.push_back((next, first));
                }
            }
        }
        None
    }
}

/// The page slot as observed at the frame boundary (read-only).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveField {
    /// Page builder of the slot.
    pub builder: u32,
    /// Highlighted grid cell, `None` while the highlight is outside the grid (opening, closing).
    pub cell: Option<Cell>,
    /// Flag word (+0x228).
    pub flags: u32,
    /// Cursor (+0x22C).
    pub cursor: u32,
    /// Text length (strlen of the buffer at +0x220).
    pub len: u32,
    /// Limit (+0x230).
    pub limit: u32,
    /// Hash of the edited text, to see a replaced selection.
    pub text_hash: u32,
}

/// The key grids of the original tables (read from the image).
pub trait KeyTables {
    fn grid(&mut self, table: KeyTable) -> Option<KeyGrid>;
}

/// Why a typed key was dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dropped {
    /// The byte is in none of the field's tables (the font cannot draw it).
    NotInCharset(u8),
    /// The text is at the field's limit.
    Full,
    /// The highlight is outside the grid, or too many taps went by.
    Lost,
}

const MODE_BITS: u32 = FieldFlags::SYMBOLS | FieldFlags::ACCENTS;

/// The flags giving symbol mode `mode` and alternate case `alternate`, toggling SHIFT as the page key does.
fn compose(flags: u32, mode: u32, alternate: bool) -> FieldFlags {
    let mut out = (flags & !MODE_BITS) | mode;
    if FieldFlags(flags).alternate_case() != alternate {
        out ^= FieldFlags::SHIFT;
    }
    FieldFlags(out)
}

/// Next step for one key on the live page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    Tap(TextButton),
    Drop(Dropped),
    /// The highlight is between cells (page animating): wait a frame.
    Wait,
}

/// Plans the next tap for `key` from the live page. Pure: the same live state gives the same tap.
pub fn plan(
    layout: &GridLayout,
    live: &LiveField,
    key: TextKey,
    tables: &mut impl KeyTables,
) -> Plan {
    match key {
        TextKey::Backspace => return Plan::Tap(TextButton::Y),
        TextKey::Cancel => return Plan::Tap(TextButton::Back),
        _ => {}
    }
    let Some(here) = live.cell else {
        return Plan::Wait;
    };
    let toward = |target: Cell| match layout.step(here, target) {
        Some(button) => Plan::Tap(button),
        None if here == target => Plan::Tap(TextButton::A),
        None => Plan::Drop(Dropped::Lost),
    };
    let TextKey::Char(byte) = key else {
        return toward(layout.page_key_cell(PageKey::Done));
    };
    if live.len >= live.limit && live.flags & FieldFlags::SELECT_ALL == 0 {
        return Plan::Drop(Dropped::Full);
    }
    let mode = live.flags & MODE_BITS;
    let alternate = FieldFlags(live.flags).alternate_case();
    // The current mode first, then the base mode, symbols and accents; the current case before the other.
    let mut modes = vec![mode];
    for other in [0, FieldFlags::SYMBOLS, FieldFlags::ACCENTS] {
        if !modes.contains(&other) {
            modes.push(other);
        }
    }
    for want_mode in modes {
        for want_alternate in [alternate, !alternate] {
            let table = select_table(compose(live.flags, want_mode, want_alternate));
            let Some(grid) = tables.grid(table) else {
                continue;
            };
            let Some(slot) = grid.keys.iter().position(|b| *b == byte && byte != 0) else {
                continue;
            };
            if want_mode != mode {
                return toward(layout.page_key_cell(PageKey::Symbols));
            }
            if want_alternate != alternate {
                return toward(layout.page_key_cell(PageKey::Shift));
            }
            return toward(layout.key_cell(slot as u32));
        }
    }
    Plan::Drop(Dropped::NotInCharset(byte))
}

/// The page's response to one tap, as MEASURED (moves by [`GridLayout::neighbor`]; A on Shift toggles 0x2, on Caps
/// 0x4, on Symbols cycles 0 -> 0x8 -> 0x1000 -> 0; A on a character inserts and releases SHIFT and the selection; Y
/// deletes). Only the length is tracked, not the text. Used to render routes and to test the driver.
pub fn simulate(layout: &GridLayout, live: &mut LiveField, button: TextButton) {
    let Some(here) = live.cell else { return };
    match button {
        TextButton::A => match layout.page_key_at(here) {
            Some(PageKey::Shift) => live.flags ^= FieldFlags::SHIFT,
            Some(PageKey::Caps) => live.flags ^= FieldFlags::CAPS,
            Some(PageKey::Symbols) => {
                let next = match live.flags & MODE_BITS {
                    0 => FieldFlags::SYMBOLS,
                    FieldFlags::SYMBOLS => FieldFlags::ACCENTS,
                    _ => 0,
                };
                live.flags = (live.flags & !MODE_BITS) | next;
            }
            Some(_) => {}
            None => {
                if live.flags & FieldFlags::SELECT_ALL != 0 {
                    live.len = 0;
                }
                if live.len < live.limit {
                    live.len += 1;
                }
                live.flags &= !(FieldFlags::SELECT_ALL | FieldFlags::SHIFT);
            }
        },
        TextButton::Y => {
            if live.flags & FieldFlags::SELECT_ALL != 0 {
                live.len = 0;
            } else {
                live.len = live.len.saturating_sub(1);
            }
            live.flags &= !(FieldFlags::SELECT_ALL | FieldFlags::SHIFT);
        }
        TextButton::Back => {}
        _ => {
            if let Some(next) = layout.neighbor(here, button) {
                live.cell = Some(next);
            }
        }
    }
}

/// Open-loop rendering for routes: plans every key against the simulated page and returns the
/// `tools/t2116_route_build.py` macros (the trace host has no Bevy input loop).
pub fn keys_to_macros(
    layout: &GridLayout,
    mut live: LiveField,
    keys: &[TextKey],
    tables: &mut impl KeyTables,
) -> Vec<&'static str> {
    let mut out = Vec::new();
    for key in keys {
        for _ in 0..MAX_TAPS_PER_KEY {
            let Plan::Tap(button) = plan(layout, &live, *key, tables) else {
                break;
            };
            out.push(button.macro_name());
            let before = live;
            simulate(layout, &mut live, button);
            if ends_key(layout, *key, button, &before) {
                break;
            }
        }
    }
    out
}

/// Host characters to keys: printable ASCII and Latin-1 code points map to their byte; others are dropped
/// (the tables are single-byte, so the font has no glyph for them).
pub fn char_to_key(c: char) -> Option<TextKey> {
    let code = c as u32;
    ((0x20..0x7f).contains(&code) || (0xa0..=0xff).contains(&code))
        .then_some(TextKey::Char(code as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FABRICATED tables shaped like the letters family: base a..z - . space, alternate upper case, symbols digits.
    struct Fake;
    impl KeyTables for Fake {
        fn grid(&mut self, table: KeyTable) -> Option<KeyGrid> {
            let base: Vec<u8> = (b'a'..=b'z').chain([b'-', b'.', b' ']).collect();
            let keys = if table == select_table(FieldFlags(0x80)) {
                base
            } else if table == select_table(FieldFlags(0x82)) {
                base.iter().map(|b| b.to_ascii_uppercase()).collect()
            } else if table == select_table(FieldFlags(0x88)) {
                (b'0'..=b'9')
                    .chain(std::iter::repeat_n(0, 18))
                    .chain([b' '])
                    .collect()
            } else {
                std::iter::repeat_n(0, 28).chain([b' ']).collect()
            };
            Some(KeyGrid { table, keys })
        }
    }

    const L: GridLayout = GridLayout::LETTERS;
    const DONE: Cell = Cell { row: 0, col: 0 };

    fn live(cell: Cell, flags: u32) -> LiveField {
        LiveField {
            builder: 1,
            cell: Some(cell),
            flags,
            cursor: 0,
            len: 0,
            limit: 29,
            text_hash: 0,
        }
    }

    #[test]
    fn cells_and_measured_moves() {
        assert_eq!(L.key_cell(0), Cell { row: 0, col: 1 });
        assert_eq!(L.key_cell(6), Cell { row: 0, col: 7 });
        assert_eq!(L.key_cell(21), Cell { row: 3, col: 1 });
        assert_eq!(L.key_cell(28), Cell { row: 4, col: 1 });
        assert_eq!(L.slot_at(Cell { row: 3, col: 7 }), Some(27));
        assert_eq!(L.slot_at(Cell { row: 4, col: 1 }), Some(28));
        assert_eq!(L.page_key_at(Cell { row: 4, col: 0 }), Some(PageKey::Del));
        assert_eq!(L.live_cell(2, 0), Some(DONE));
        assert_eq!(L.live_cell(6, 1), Some(Cell { row: 4, col: 1 }));
        assert_eq!(L.live_cell(0xffff_ffff, 0), None);
        // The measured sequences of docs/t2169-keyboard-text-entry.md (capture measure3).
        let walk = |mut c: Cell, buttons: &[TextButton]| {
            for b in buttons {
                c = L.neighbor(c, *b).unwrap();
            }
            c
        };
        use TextButton::*;
        assert_eq!(walk(DONE, &[Left]), Cell { row: 0, col: 7 });
        assert_eq!(walk(DONE, &[Left, Right]), DONE);
        assert_eq!(walk(DONE, &[Down, Down, Down, Down, Down]), DONE);
        assert_eq!(walk(DONE, &[Up]), Cell { row: 4, col: 0 });
        assert_eq!(
            walk(DONE, &[Right, Down, Down, Down, Down]),
            Cell { row: 4, col: 1 }
        );
        assert_eq!(
            walk(Cell { row: 4, col: 1 }, &[Right]),
            Cell { row: 4, col: 0 }
        );
        assert_eq!(
            walk(Cell { row: 4, col: 1 }, &[Left, Left]),
            Cell { row: 4, col: 1 }
        );
        assert_eq!(
            walk(Cell { row: 4, col: 1 }, &[Up]),
            Cell { row: 3, col: 1 }
        );
        assert_eq!(
            walk(Cell { row: 3, col: 7 }, &[Down]),
            Cell { row: 4, col: 1 }
        );
        assert_eq!(
            walk(Cell { row: 4, col: 1 }, &[Down]),
            Cell { row: 0, col: 1 }
        );
        assert_eq!(L.neighbor(Cell { row: 0, col: 3 }, Up), None);
    }

    #[test]
    fn shortest_paths_use_the_wraps() {
        use TextButton::*;
        assert_eq!(L.step(DONE, L.key_cell(6)), Some(Left));
        assert_eq!(L.step(DONE, L.page_key_cell(PageKey::Del)), Some(Up));
        assert_eq!(L.step(L.key_cell(25), L.key_cell(28)), Some(Down));
        assert_eq!(L.step(L.key_cell(28), L.key_cell(0)), Some(Down));
        assert_eq!(L.step(L.key_cell(3), L.key_cell(3)), None);
        // Every cell reaches every other.
        let cells: Vec<Cell> = (0..L.keys)
            .map(|s| L.key_cell(s))
            .chain((0..5).map(|r| Cell { row: r, col: 0 }))
            .collect();
        for from in &cells {
            for to in &cells {
                assert!(
                    from == to || L.step(*from, *to).is_some(),
                    "{from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn plans_case_symbols_and_refusals() {
        use TextButton::*;
        let shift = L.page_key_cell(PageKey::Shift);
        let symbols = L.page_key_cell(PageKey::Symbols);
        let a = L.key_cell(0);
        assert_eq!(
            plan(&L, &live(DONE, 0x80), TextKey::Char(b'a'), &mut Fake),
            Plan::Tap(Right)
        );
        assert_eq!(
            plan(&L, &live(a, 0x80), TextKey::Char(b'a'), &mut Fake),
            Plan::Tap(A)
        );
        assert_eq!(
            plan(&L, &live(a, 0x80), TextKey::Char(b'A'), &mut Fake),
            Plan::Tap(Left)
        );
        assert_eq!(
            plan(&L, &live(shift, 0x80), TextKey::Char(b'A'), &mut Fake),
            Plan::Tap(A)
        );
        // Caps on: a lower case letter goes through Shift (SHIFT and CAPS cancel), an upper case one does not.
        assert_eq!(
            plan(&L, &live(shift, 0x84), TextKey::Char(b'a'), &mut Fake),
            Plan::Tap(A)
        );
        assert_eq!(
            plan(&L, &live(a, 0x84), TextKey::Char(b'A'), &mut Fake),
            Plan::Tap(A)
        );
        assert_eq!(
            plan(&L, &live(symbols, 0x80), TextKey::Char(b'1'), &mut Fake),
            Plan::Tap(A)
        );
        assert_eq!(
            plan(&L, &live(symbols, 0x88), TextKey::Char(b'a'), &mut Fake),
            Plan::Tap(A)
        );
        assert_eq!(
            plan(&L, &live(a, 0x88), TextKey::Char(b'1'), &mut Fake),
            Plan::Tap(Right)
        );
        assert_eq!(
            plan(
                &L,
                &live(L.key_cell(28), 0x88),
                TextKey::Char(b' '),
                &mut Fake
            ),
            Plan::Tap(A)
        );
        assert_eq!(
            plan(&L, &live(a, 0x80), TextKey::Char(b'#'), &mut Fake),
            Plan::Drop(Dropped::NotInCharset(b'#'))
        );
        let full = LiveField {
            len: 29,
            ..live(a, 0x80)
        };
        assert_eq!(
            plan(&L, &full, TextKey::Char(b'a'), &mut Fake),
            Plan::Drop(Dropped::Full)
        );
        let selected = LiveField {
            len: 29,
            ..live(a, 0x90)
        };
        assert_eq!(
            plan(&L, &selected, TextKey::Char(b'a'), &mut Fake),
            Plan::Tap(A)
        );
        assert_eq!(
            plan(&L, &live(a, 0x80), TextKey::Backspace, &mut Fake),
            Plan::Tap(Y)
        );
        assert_eq!(
            plan(&L, &live(a, 0x80), TextKey::Cancel, &mut Fake),
            Plan::Tap(Back)
        );
        assert_eq!(
            plan(&L, &live(a, 0x80), TextKey::Done, &mut Fake),
            Plan::Tap(Left)
        );
        assert_eq!(
            plan(&L, &live(DONE, 0x80), TextKey::Done, &mut Fake),
            Plan::Tap(A)
        );
        let moving = LiveField {
            cell: None,
            ..live(a, 0x80)
        };
        assert_eq!(
            plan(&L, &moving, TextKey::Char(b'a'), &mut Fake),
            Plan::Wait
        );
    }

    #[test]
    fn open_loop_route_matches_the_driver() {
        let start = LiveField {
            len: 8,
            ..live(DONE, 0x190)
        };
        let keys = [TextKey::Char(b'A'), TextKey::Char(b'b'), TextKey::Done];
        let macros = keys_to_macros(&L, start, &keys, &mut Fake);
        assert_eq!(
            macros,
            [
                "DOWN", "A", "RIGHT", "UP", "A", "RIGHT", "A", "LEFT", "LEFT", "A"
            ]
        );
    }

    #[test]
    fn host_characters_map_to_single_bytes() {
        assert_eq!(char_to_key('a'), Some(TextKey::Char(b'a')));
        assert_eq!(char_to_key('é'), Some(TextKey::Char(0xe9)));
        assert_eq!(char_to_key('\u{20ac}'), None);
        assert_eq!(char_to_key('\n'), None);
    }
}
