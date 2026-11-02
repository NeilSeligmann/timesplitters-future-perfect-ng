// SPDX-License-Identifier: GPL-3.0-or-later
//! Measurement tool only (T2316, T2308 close-out): the T2169 closed-loop pad typing driver. Every typed key becomes pad
//! taps on the ORIGINAL on-screen keyboard page, each planned from the live page (`text_entry::plan`). The shell does not
//! use it (typing is native since T2310, `crate::native_text`); it stays to drive the original page as the pad reference
//! for Tier 2 comparisons. The open-loop route generator `tsfp-text-route` uses `text_entry::keys_to_macros`.
//! Formerly `text_entry::TypingDriver`.

use std::collections::VecDeque;

use crate::text_entry::{
    Dropped, GridLayout, KeyTables, LiveField, Plan, TextButton, TextEdge, TextKey, plan,
};

/// Tap timing (wall clock, FABRICATED host choices). The original polls the pad once per 59.94 Hz frame; a hold of
/// 50 ms spans three polls, the confirm wait bounds a tap the page ignores.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypingTiming {
    pub hold_ms: u64,
    pub gap_ms: u64,
    pub confirm_ms: u64,
}

impl Default for TypingTiming {
    fn default() -> Self {
        Self {
            hold_ms: 50,
            gap_ms: 20,
            confirm_ms: 400,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Holding {
        button: TextButton,
        release_at: u64,
        before: LiveField,
    },
    Confirming {
        button: TextButton,
        before: LiveField,
        deadline: u64,
        gap_until: u64,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TypingStats {
    pub keys: u32,
    pub taps: u32,
    pub unconfirmed: u32,
    pub dropped: u32,
}

/// Taps one key may take before the driver gives up (a grid crossing plus two mode keys).
pub(crate) const MAX_TAPS_PER_KEY: u32 = 24;

/// Whether `button`, pressed on `before`, completes `key` (A on a character key or on Done, Y, Back).
pub(crate) fn ends_key(
    layout: &GridLayout,
    key: TextKey,
    button: TextButton,
    before: &LiveField,
) -> bool {
    match (button, key) {
        (TextButton::Y | TextButton::Back, _) => true,
        (TextButton::A, TextKey::Done) => true,
        (TextButton::A, TextKey::Char(_)) => {
            before.cell.is_some_and(|c| layout.slot_at(c).is_some())
        }
        _ => false,
    }
}

/// Closed-loop typing driver: one queue of typed keys, one tap at a time, each re-planned from the live page.
pub struct PadTypingDriver {
    pub layout: GridLayout,
    pub timing: TypingTiming,
    keys: VecDeque<TextKey>,
    phase: Phase,
    taps_for_key: u32,
    pub stats: TypingStats,
    pub last_drop: Option<Dropped>,
}

impl PadTypingDriver {
    pub fn new(layout: GridLayout, timing: TypingTiming) -> Self {
        Self {
            layout,
            timing,
            keys: VecDeque::new(),
            phase: Phase::Idle,
            taps_for_key: 0,
            stats: TypingStats::default(),
            last_drop: None,
        }
    }

    pub fn push(&mut self, key: TextKey) {
        self.keys.push_back(key);
    }

    pub fn pending(&self) -> usize {
        self.keys.len()
    }

    pub fn idle(&self) -> bool {
        self.keys.is_empty() && self.phase == Phase::Idle
    }

    /// The page closed: release a held button, drop the queue.
    pub fn reset(&mut self, out: &mut Vec<TextEdge>) {
        if let Phase::Holding { button, .. } = self.phase {
            out.push(TextEdge::Release(button));
        }
        self.stats.dropped += self.keys.len() as u32;
        self.keys.clear();
        self.phase = Phase::Idle;
        self.taps_for_key = 0;
    }

    fn finish_key(&mut self) {
        self.keys.pop_front();
        self.taps_for_key = 0;
    }

    fn drop_key(&mut self, reason: Dropped) {
        self.stats.dropped += 1;
        self.last_drop = Some(reason);
        self.finish_key();
    }

    /// One frame with the live page (`None`: no text page is up).
    pub fn tick(
        &mut self,
        now_ms: u64,
        live: Option<LiveField>,
        tables: &mut impl KeyTables,
        out: &mut Vec<TextEdge>,
    ) {
        let Some(live) = live else {
            self.reset(out);
            return;
        };
        match self.phase {
            Phase::Holding {
                button,
                release_at,
                before,
            } => {
                if now_ms >= release_at {
                    out.push(TextEdge::Release(button));
                    self.phase = Phase::Confirming {
                        button,
                        before,
                        deadline: now_ms + self.timing.confirm_ms,
                        gap_until: now_ms + self.timing.gap_ms,
                    };
                }
                return;
            }
            Phase::Confirming {
                button,
                before,
                deadline,
                gap_until,
            } => {
                if now_ms < gap_until {
                    return;
                }
                let changed = live != before;
                if !changed && now_ms < deadline {
                    return;
                }
                if !changed {
                    self.stats.unconfirmed += 1;
                }
                self.phase = Phase::Idle;
                // A refused key (width rule, limit) leaves the page unchanged and still ends: the original decides.
                if let Some(key) = self.keys.front().copied()
                    && ends_key(&self.layout, key, button, &before)
                {
                    self.stats.keys += 1;
                    self.finish_key();
                }
            }
            Phase::Idle => {}
        }
        while let Some(key) = self.keys.front().copied() {
            if self.taps_for_key >= MAX_TAPS_PER_KEY {
                self.drop_key(Dropped::Lost);
                continue;
            }
            match plan(&self.layout, &live, key, tables) {
                Plan::Wait => return,
                Plan::Drop(reason) => self.drop_key(reason),
                Plan::Tap(button) => {
                    out.push(TextEdge::Press(button));
                    self.stats.taps += 1;
                    self.taps_for_key += 1;
                    self.phase = Phase::Holding {
                        button,
                        release_at: now_ms + self.timing.hold_ms,
                        before: live,
                    };
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_entry::{Cell, char_to_key, simulate};
    use tsfp_text_field::keyboard::{FieldFlags, KeyGrid, KeyTable, select_table};

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

    /// Runs the closed-loop driver against the simulated page with a text model (FABRICATED stand-in).
    fn type_into(keys: &[TextKey], start: LiveField, text: &mut Vec<u8>) -> PadTypingDriver {
        let mut driver = PadTypingDriver::new(L, TypingTiming::default());
        for key in keys {
            driver.push(*key);
        }
        let mut field = start;
        let mut now = 0;
        let mut out = Vec::new();
        while !driver.idle() && now < 200_000 {
            driver.tick(now, Some(field), &mut Fake, &mut out);
            for edge in out.drain(..) {
                let TextEdge::Press(button) = edge else {
                    continue;
                };
                if button == TextButton::A
                    && let Some(slot) = field.cell.and_then(|c| L.slot_at(c))
                {
                    let grid = Fake.grid(select_table(FieldFlags(field.flags))).unwrap();
                    if field.flags & FieldFlags::SELECT_ALL != 0 {
                        text.clear();
                    }
                    text.push(grid.keys[slot as usize]);
                }
                if button == TextButton::Y {
                    text.pop();
                }
                simulate(&L, &mut field, button);
                field.text_hash = text
                    .iter()
                    .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(*b as u32));
            }
            now += 16;
        }
        driver
    }

    #[test]
    fn closed_loop_types_a_mixed_name() {
        let mut text = b"Player 1".to_vec();
        let mut keys: Vec<TextKey> = "Ab 1z.".chars().filter_map(char_to_key).collect();
        keys.push(TextKey::Backspace);
        keys.push(TextKey::Char(b'#'));
        let driver = type_into(
            &keys,
            LiveField {
                len: 8,
                ..live(DONE, 0x190)
            },
            &mut text,
        );
        assert!(driver.idle(), "driver stuck: {:?}", driver.stats);
        assert_eq!(text, b"Ab 1z");
        assert_eq!(driver.stats.keys, 7);
        assert_eq!(driver.stats.dropped, 1);
        assert_eq!(driver.last_drop, Some(Dropped::NotInCharset(b'#')));
    }

    #[test]
    fn closing_the_page_releases_and_drops() {
        let mut driver = PadTypingDriver::new(L, TypingTiming::default());
        driver.push(TextKey::Char(b'a'));
        let mut out = Vec::new();
        driver.tick(0, Some(live(L.key_cell(0), 0x80)), &mut Fake, &mut out);
        assert_eq!(out, [TextEdge::Press(TextButton::A)]);
        out.clear();
        driver.tick(16, None, &mut Fake, &mut out);
        assert_eq!(out, [TextEdge::Release(TextButton::A)]);
        assert!(driver.idle());
        assert_eq!(driver.stats.dropped, 1);
    }
}
