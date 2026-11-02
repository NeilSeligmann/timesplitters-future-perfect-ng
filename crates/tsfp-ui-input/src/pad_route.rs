//! Measurement tool only (T2316, T2308 close-out): the closed-loop pad-tap driver that turns Rust UI goals (focus,
//! select, adjust, back) into pad presses for the ORIGINAL menu, and its open-loop rendering as route macros. It
//! generates the legacy pad reference routes the Tier 2 page traces compare native page actions against
//! (`tsfp-ui-route`, `tools/t2116_route_build.py`). No shell path uses it: the Bevy shell builds every `LiveMenu`
//! native (`native_page`), and the stop line `dispatched taps=0` stays zero. Formerly the T2188/T2168 TEMPORARY-SHIM
//! `PadRouteDriver` / `PadRouteBindings` in `live_menu.rs`.

use std::collections::VecDeque;

use crate::bindings::{ActionArg, ActionId, BindResult, ListId, ListView, TextId};
use crate::bindings::{UiBindings, UiValue, ValueId};
use crate::live_menu::{
    ACTION_BACK, ACTION_FOCUS, ACTION_SELECT, ACTION_VALUE_NEXT, ACTION_VALUE_PREV, LiveSlot,
    VALUE_CURSOR, VALUE_ROWS,
};

/// Pad buttons the driver taps (gamepad source names DPAD_UP, DPAD_DOWN, DPAD_LEFT, DPAD_RIGHT,
/// A, B). Left/right change the value of an option row in the original (T2168).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PadButton {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
}

impl PadButton {
    /// SDL3 Xbox layout index used by the `tsfp_wi_*` queue (events.rs table).
    pub fn sdl_index(self) -> u8 {
        match self {
            PadButton::A => 0,
            PadButton::B => 1,
            PadButton::Up => 11,
            PadButton::Down => 12,
            PadButton::Left => 13,
            PadButton::Right => 14,
        }
    }
    /// Route macro name of `tools/t2116_route_build.py`.
    pub fn macro_name(self) -> &'static str {
        match self {
            PadButton::A => "A",
            PadButton::B => "B",
            PadButton::Up => "UP",
            PadButton::Down => "DOWN",
            PadButton::Left => "LEFT",
            PadButton::Right => "RIGHT",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadEdge {
    Press(PadButton),
    Release(PadButton),
}

/// Tap timing (wall clock). Hold covers several 59.94 Hz polls; the macro timings of the
/// scripted routes are 4 polls for a stick step and 6 for A/B (`tools/t2116_route_build.py`).
/// `confirm_ms` bounds the wait for the live slot to show the effect; values are FABRICATED
/// host-side choices, not original timings (the original has no such loop).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TapTiming {
    pub hold_ms: u64,
    pub gap_ms: u64,
    pub confirm_ms: u64,
}

impl Default for TapTiming {
    fn default() -> Self {
        Self {
            hold_ms: 100,
            gap_ms: 50,
            confirm_ms: 600,
        }
    }
}

/// What the Rust page asked the original menu to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    /// Move the guest cursor to this row (keyboard/pad focus, hover).
    Focus(u32),
    /// Move to the row, then the original select handler (A).
    Select(u32),
    /// The original back handler (B).
    Back,
    /// Move to the row, then change its option value (LEFT, or RIGHT when `right`). T2168.
    Adjust { row: u32, right: bool },
    /// One plain tap, no row target (keyboard on a legacy-drawn page, wheel on a list). T2168.
    Tap(PadButton),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Holding {
        button: PadButton,
        finishes: bool,
        release_at: u64,
        before: LiveSlot,
    },
    Confirming {
        button: PadButton,
        finishes: bool,
        before: LiveSlot,
        deadline: u64,
        gap_until: u64,
    },
}

/// Counters for the stop line and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DriverStats {
    pub taps: u32,
    pub confirmed: u32,
    pub unconfirmed: u32,
    pub dropped_goals: u32,
}

/// Closed-loop tap driver for one original page slot.
#[derive(Clone, Debug)]
pub struct PadRouteDriver {
    pub timing: TapTiming,
    pub(crate) goals: VecDeque<Goal>,
    phase: Phase,
    /// Cursor moves spent on the head goal (a target the original cursor never reaches, e.g.
    /// a row it skips, is dropped after `rows + 2` moves instead of oscillating).
    moves: u32,
    pub stats: DriverStats,
}

impl Default for PadRouteDriver {
    fn default() -> Self {
        Self::new(TapTiming::default())
    }
}

impl PadRouteDriver {
    pub fn new(timing: TapTiming) -> Self {
        Self {
            timing,
            goals: VecDeque::new(),
            phase: Phase::Idle,
            moves: 0,
            stats: DriverStats::default(),
        }
    }

    /// Queue a goal. A newer focus goal replaces a queued one (only the latest focus matters).
    pub fn push(&mut self, goal: Goal) {
        if let Goal::Focus(_) = goal
            && let Some(Goal::Focus(_)) = self.goals.back()
        {
            self.goals.pop_back();
        }
        self.goals.push_back(goal);
    }

    /// Goals not yet finished, head first.
    pub fn queued(&self) -> Vec<Goal> {
        self.goals.iter().copied().collect()
    }

    /// Take the queued goals without driving them (open-loop route rendering).
    pub fn drain(&mut self) -> Vec<Goal> {
        self.goals.drain(..).collect()
    }

    pub fn idle(&self) -> bool {
        self.goals.is_empty() && self.phase == Phase::Idle
    }

    /// Drop everything (page closed or changed under the driver). A held button is released.
    pub fn reset(&mut self, out: &mut Vec<PadEdge>) {
        if let Phase::Holding { button, .. } = self.phase {
            out.push(PadEdge::Release(button));
        }
        self.stats.dropped_goals += self.goals.len() as u32;
        self.goals.clear();
        self.phase = Phase::Idle;
        self.moves = 0;
    }

    /// The next tap toward the head goal and whether it finishes the goal, `None` when the goal
    /// is met (and popped).
    fn next_button(&mut self, live: LiveSlot) -> Option<(PadButton, bool)> {
        loop {
            let goal = *self.goals.front()?;
            let (row, last) = match goal {
                Goal::Back => return Some((PadButton::B, true)),
                Goal::Tap(button) => return Some((button, true)),
                Goal::Focus(row) => (row, None),
                Goal::Select(row) => (row, Some(PadButton::A)),
                Goal::Adjust { row, right } => (
                    row,
                    Some(if right {
                        PadButton::Right
                    } else {
                        PadButton::Left
                    }),
                ),
            };
            if row >= live.rows || self.moves > live.rows + 2 {
                self.goals.pop_front();
                self.moves = 0;
                self.stats.dropped_goals += 1;
                continue;
            }
            if live.cursor != row {
                self.moves += 1;
                let step = if live.cursor < row {
                    PadButton::Down
                } else {
                    PadButton::Up
                };
                return Some((step, false));
            }
            if let Some(button) = last {
                return Some((button, true));
            }
            self.goals.pop_front();
            self.moves = 0;
        }
    }

    /// One frame. `live` is the current slot of the page being driven (`None` closes it).
    pub fn tick(&mut self, now_ms: u64, live: Option<LiveSlot>, out: &mut Vec<PadEdge>) {
        let Some(live) = live else {
            self.reset(out);
            return;
        };
        match self.phase {
            Phase::Holding {
                button,
                finishes,
                release_at,
                before,
            } => {
                if now_ms >= release_at {
                    out.push(PadEdge::Release(button));
                    // An option value is not part of the live slot, so LEFT/RIGHT cannot be
                    // confirmed: they settle for two gaps (FABRICATED) instead of the full wait.
                    let wait = match button {
                        PadButton::Left | PadButton::Right => self.timing.gap_ms * 2,
                        _ => self.timing.confirm_ms,
                    };
                    self.phase = Phase::Confirming {
                        button,
                        finishes,
                        before,
                        deadline: now_ms + wait,
                        gap_until: now_ms + self.timing.gap_ms,
                    };
                }
            }
            Phase::Confirming {
                button: _,
                finishes,
                before,
                deadline,
                gap_until,
            } => {
                let moved = live != before;
                if now_ms < gap_until {
                    return;
                }
                if moved || now_ms >= deadline {
                    if moved {
                        self.stats.confirmed += 1;
                    } else {
                        self.stats.unconfirmed += 1;
                    }
                    // The goal's last tap (A, B, LEFT/RIGHT, a plain tap) finishes it on any
                    // effect or timeout (the original may refuse, e.g. a locked row). Moves
                    // re-plan from the live cursor.
                    if finishes {
                        self.goals.pop_front();
                        self.moves = 0;
                    }
                    self.phase = Phase::Idle;
                }
            }
            Phase::Idle => {}
        }
        if self.phase == Phase::Idle
            && let Some((button, finishes)) = self.next_button(live)
        {
            out.push(PadEdge::Press(button));
            self.stats.taps += 1;
            self.phase = Phase::Holding {
                button,
                finishes,
                release_at: now_ms + self.timing.hold_ms,
                before: live,
            };
        }
    }
}

/// `UiBindings` that reach the original handlers of one page through its live slot.
#[derive(Clone, Debug, Default)]
pub struct PadRouteBindings {
    pub driver: PadRouteDriver,
    pub live: Option<LiveSlot>,
}

impl UiBindings for PadRouteBindings {
    fn action(&mut self, id: ActionId, arg: ActionArg) -> BindResult {
        if self.live.is_none() {
            return BindResult::Denied;
        }
        let row = match arg {
            ActionArg::Index(row) => u32::try_from(row).ok(),
            ActionArg::Int(row) => u32::try_from(row).ok(),
            _ => None,
        };
        match (id, row) {
            (ACTION_BACK, _) => self.driver.push(Goal::Back),
            (ACTION_SELECT, Some(row)) => self.driver.push(Goal::Select(row)),
            (ACTION_FOCUS, Some(row)) => self.driver.push(Goal::Focus(row)),
            (ACTION_VALUE_PREV, Some(row)) => self.driver.push(Goal::Adjust { row, right: false }),
            (ACTION_VALUE_NEXT, Some(row)) => self.driver.push(Goal::Adjust { row, right: true }),
            _ => return BindResult::Ignored,
        }
        BindResult::Handled
    }
    fn value(&self, id: ValueId) -> UiValue {
        match (id, self.live) {
            (VALUE_CURSOR, Some(live)) => UiValue::Index(live.cursor),
            (VALUE_ROWS, Some(live)) => UiValue::Index(live.rows),
            _ => UiValue::None,
        }
    }
    fn set_value(&mut self, _id: ValueId, _value: UiValue) -> BindResult {
        BindResult::Ignored
    }
    fn list(&self, _id: ListId) -> ListView {
        ListView::default()
    }
    fn text(&self, _id: TextId) -> &str {
        ""
    }
}

/// Open-loop rendering of a goal list as route macros for `tools/t2116_route_build.py`
/// (`DOWN`/`UP`/`LEFT`/`RIGHT`/`A`/`B` lines), simulating the original cursor (clamped, no wrap)
/// from `cursor`. Used to replay the Rust UI's dispatch on the trace host, which has no shell
/// input loop.
pub fn goals_to_macros(goals: &[Goal], cursor: u32, rows: u32) -> Vec<&'static str> {
    goals_to_macros_on(goals, cursor, &vec![true; rows as usize]).0
}

/// [`goals_to_macros`] on a page whose cursor skips the rows with `navigable[row] == false`
/// (one DOWN moves to the next navigable row). Returns the macros and the simulated cursor.
pub fn goals_to_macros_on(
    goals: &[Goal],
    mut cursor: u32,
    navigable: &[bool],
) -> (Vec<&'static str>, u32) {
    let rows = navigable.len() as u32;
    let next = |from: u32, down: bool| -> u32 {
        let mut at = from;
        loop {
            let step = if down {
                at.checked_add(1).filter(|r| *r < rows)
            } else {
                at.checked_sub(1)
            };
            match step {
                Some(r) if navigable[r as usize] => return r,
                Some(r) => at = r,
                None => return from,
            }
        }
    };
    let mut out = Vec::new();
    for goal in goals {
        let (row, last) = match *goal {
            Goal::Back => {
                out.push(PadButton::B.macro_name());
                continue;
            }
            Goal::Tap(button) => {
                out.push(button.macro_name());
                match button {
                    PadButton::Down => cursor = next(cursor, true),
                    PadButton::Up => cursor = next(cursor, false),
                    _ => {}
                }
                continue;
            }
            Goal::Focus(row) => (row, None),
            Goal::Select(row) => (row, Some(PadButton::A)),
            Goal::Adjust { row, right } => (
                row,
                Some(if right {
                    PadButton::Right
                } else {
                    PadButton::Left
                }),
            ),
        };
        if row >= rows || !navigable[row as usize] {
            continue;
        }
        while cursor != row {
            let down = cursor < row;
            let moved = next(cursor, down);
            if moved == cursor {
                break;
            }
            out.push(if down {
                PadButton::Down.macro_name()
            } else {
                PadButton::Up.macro_name()
            });
            cursor = moved;
        }
        if let Some(button) = last {
            out.push(button.macro_name());
            if button == PadButton::A {
                cursor = 0;
            }
        }
    }
    (out, cursor)
}
