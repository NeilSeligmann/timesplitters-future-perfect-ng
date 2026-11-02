//! T2188: the live `--ui=rust` menu loop. A Rust page (`tsfp-ui-model::Page`) owns focus and
//! input; its actions run natively (T2310: `native_page::NativeMenuBindings`, the page and row handlers called through
//! `GuestUi` at the guest frame boundary) on the live page slot (`LiveSlot`, read from the original widget table at
//! every frame boundary). Mapping row index = position among the page's focusable widgets (main menu: 8 rows, MEASURED
//! order 0 Story .. 7 Extras, docs/t-ui-coverage.md).
//!
//! T2316 (T2308 close-out): the T2188/T2168 pad-tap dispatch (the former TEMPORARY-SHIM `LegacyMenuDriver` /
//! `LegacyMenuBindings`) moved to the measurement tool module `pad_route` (`PadRouteDriver`, `PadRouteBindings`); only
//! [`LiveMenu::pad_route`] (used by `tsfp-ui-route` to generate legacy reference routes) builds it. Hit testing, focus
//! and activation (`UiInput`, `ModelLayout`, `dispatch_menu_event`, `PageMap`) are native and device-agnostic.

use crate::bindings::{ActionArg, ActionId, ActionMap, ActivationOutcome, UiBindings, ValueId};
use crate::input::UiEvent;
use crate::layout::WidgetId as InputWidgetId;
use tsfp_ui_model::{MenuAction, MenuFocus, Page, WidgetId};

pub use crate::pad_route::{
    DriverStats, Goal, PadButton, PadEdge, PadRouteBindings, PadRouteDriver, TapTiming,
    goals_to_macros, goals_to_macros_on,
};

/// One original page slot as observed at the frame boundary (C `host_ui_page_snapshots`:
/// slot base 0x6FEBE8 + n * 0x23C, builder +8, rows +0x180, cursor +0x184, state +0x1B0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveSlot {
    pub builder: u32,
    pub state: u32,
    pub cursor: u32,
    pub rows: u32,
}

/// Action ids of the legacy menu bindings.
pub const ACTION_SELECT: ActionId = ActionId(1);
pub const ACTION_BACK: ActionId = ActionId(2);
pub const ACTION_FOCUS: ActionId = ActionId(3);
/// T2168: previous / next option value of a row (`Index(row)`).
pub const ACTION_VALUE_PREV: ActionId = ActionId(4);
pub const ACTION_VALUE_NEXT: ActionId = ActionId(5);
/// T2310: one native list step, `ActionArg::Index` 0 up, 1 down, 2 left, 3 right.
pub const ACTION_STEP: ActionId = ActionId(6);
/// Value ids: the live cursor and row count of the driven slot.
pub const VALUE_CURSOR: ValueId = ValueId(1);
pub const VALUE_ROWS: ValueId = ValueId(2);

/// Row index of each focusable widget (page order). The T2188 main-menu mapping; T2168 pages use
/// the tree-generated [`crate::page_map::PageMap`] (identical on the main menu).
pub fn page_rows(page: &Page) -> Vec<WidgetId> {
    page.widgets
        .iter()
        .filter(|w| w.is_focusable())
        .map(|w| w.id)
        .collect()
}

/// The action map over cursor rows `rows`: Activate of a plain row selects it, Activate of an
/// option row steps its value forward (T2168), Back is the original back handler.
pub fn rows_action_map(rows: &[WidgetId], cyclers: &[bool]) -> ActionMap {
    let mut map = ActionMap::new();
    for (row, id) in rows.iter().enumerate() {
        let action = if cyclers.get(row).copied().unwrap_or(false) {
            ACTION_VALUE_NEXT
        } else {
            ACTION_SELECT
        };
        map.bind_activate(InputWidgetId(id.0), action, ActionArg::Index(row));
        // T3811: a press on a greyed row still reaches the page (denied sound, or its help button).
        map.bind_disabled_help(InputWidgetId(id.0), ACTION_SELECT, ActionArg::Index(row));
    }
    map.bind_back(ACTION_BACK);
    map
}

pub fn page_action_map(page: &Page) -> ActionMap {
    rows_action_map(&page_rows(page), &[])
}

/// Where a live page's actions go.
#[derive(Debug)]
pub enum Dispatch {
    /// T2310: native page actions (focus, select, back run by `native_page::service` at the guest frame boundary).
    /// Every page the shell builds.
    Native(crate::native_page::NativeMenuBindings),
    /// T2316: the measurement tool's pad-tap driver (`pad_route`), used only to generate legacy reference routes.
    PadRoute(PadRouteBindings),
}

/// Result of one live-page event, including the last-mile activation decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveMenuEventDispatch {
    pub menu_action: Option<MenuAction>,
    pub activation: ActivationOutcome,
}

/// One live Rust page: model focus, action map and the bindings to its original slot.
pub struct LiveMenu {
    pub builder: u32,
    /// Widget at each original cursor position.
    pub rows: Vec<WidgetId>,
    /// Option rows (left/right change the value), by cursor position.
    pub cyclers: Vec<bool>,
    /// Row positions map 1:1 to the live cursor (the pointer may target rows). False on
    /// scrolling lists, where only wheel, back and plain taps are dispatched.
    pub pointer_rows: bool,
    pub focus: MenuFocus,
    pub map: ActionMap,
    /// The live slot as last observed.
    pub live: Option<LiveSlot>,
    pub dispatch: Dispatch,
}

impl LiveMenu {
    /// A native live page (T2310): its actions run through `native_page`, no pad taps.
    pub fn new(page: &Page, builder: u32) -> Self {
        let rows = page_rows(page);
        let cyclers = vec![false; rows.len()];
        let native = Dispatch::Native(crate::native_page::NativeMenuBindings::default());
        Self::with_rows(page, builder, rows, cyclers, true, native)
    }

    /// T3781: a native live page whose cursor rows are given in guest cursor order (greyed rows included, see
    /// [`crate::live_page::LivePage::cursor_widgets`]); `None` keeps the focusable widgets of the page.
    pub fn with_cursor_rows(page: &Page, builder: u32, rows: Option<Vec<WidgetId>>) -> Self {
        let rows = rows.unwrap_or_else(|| page_rows(page));
        let cyclers = vec![false; rows.len()];
        let native = Dispatch::Native(crate::native_page::NativeMenuBindings::default());
        Self::with_rows(page, builder, rows, cyclers, true, native)
    }

    /// T2168: a native live page from its tree-generated cursor map.
    pub fn from_map(page: &Page, map: &crate::page_map::PageMap) -> Self {
        let native = Dispatch::Native(crate::native_page::NativeMenuBindings::default());
        Self::with_rows(
            page,
            map.builder,
            map.widgets(),
            map.cyclers(),
            map.pointer_rows_valid(),
            native,
        )
    }

    /// Measurement tool only (`tsfp-ui-route`): the page's actions become pad-tap goals (`pad_route`).
    pub fn pad_route(page: &Page, map: &crate::page_map::PageMap, timing: TapTiming) -> Self {
        let taps = Dispatch::PadRoute(PadRouteBindings {
            driver: PadRouteDriver::new(timing),
            live: None,
        });
        Self::with_rows(
            page,
            map.builder,
            map.widgets(),
            map.cyclers(),
            map.pointer_rows_valid(),
            taps,
        )
    }

    fn with_rows(
        page: &Page,
        builder: u32,
        rows: Vec<WidgetId>,
        cyclers: Vec<bool>,
        pointer_rows: bool,
        dispatch: Dispatch,
    ) -> Self {
        Self {
            builder,
            map: rows_action_map(&rows, &cyclers),
            rows,
            cyclers,
            pointer_rows,
            focus: MenuFocus::new(page),
            live: None,
            dispatch,
        }
    }

    /// The native bindings (None on a measurement-tool page).
    pub fn native(&self) -> Option<&crate::native_page::NativeMenuBindings> {
        match &self.dispatch {
            Dispatch::Native(native) => Some(native),
            Dispatch::PadRoute(_) => None,
        }
    }

    pub fn native_mut(&mut self) -> Option<&mut crate::native_page::NativeMenuBindings> {
        match &mut self.dispatch {
            Dispatch::Native(native) => Some(native),
            Dispatch::PadRoute(_) => None,
        }
    }

    /// The measurement tool's pad-tap driver (None on a native page).
    pub fn pad_route_driver(&mut self) -> Option<&mut PadRouteDriver> {
        match &mut self.dispatch {
            Dispatch::PadRoute(taps) => Some(&mut taps.driver),
            Dispatch::Native(_) => None,
        }
    }

    /// Pad taps sent so far: always zero on a native page (the shell's stop line `dispatched taps=0`).
    pub fn tap_stats(&self) -> DriverStats {
        match &self.dispatch {
            Dispatch::PadRoute(taps) => taps.driver.stats,
            Dispatch::Native(_) => DriverStats::default(),
        }
    }

    fn bindings(&mut self) -> &mut dyn UiBindings {
        match &mut self.dispatch {
            Dispatch::Native(native) => native,
            Dispatch::PadRoute(taps) => taps,
        }
    }

    pub fn row_of(&self, id: WidgetId) -> Option<u32> {
        if !self.pointer_rows {
            return None;
        }
        self.rows.iter().position(|w| *w == id).map(|r| r as u32)
    }

    fn is_cycler(&self, row: u32) -> bool {
        self.cyclers.get(row as usize).copied().unwrap_or(false)
    }

    /// The live slot matches this map (row count as captured); a rebuilt or different-state
    /// page gets no row-targeted goals.
    pub fn rows_match(&self) -> bool {
        self.live
            .is_some_and(|live| live.rows as usize == self.rows.len())
    }

    /// Follow the live slot. While no action is pending the Rust focus mirrors the guest cursor
    /// (so returning to the page, or a guest-side change, is shown); returns true when the
    /// focused widget changed.
    pub fn observe(&mut self, live: Option<LiveSlot>) -> bool {
        self.live = live;
        let pending = match &mut self.dispatch {
            Dispatch::Native(native) => {
                native.live = live;
                native.pending()
            }
            Dispatch::PadRoute(taps) => {
                taps.live = live;
                !taps.driver.idle()
            }
        };
        let Some(live) = live else { return false };
        if pending || !self.pointer_rows {
            return false;
        }
        let target = self.rows.get(live.cursor as usize).copied();
        if target.is_some() && self.focus.selected != target {
            self.focus.selected = target;
            return true;
        }
        false
    }

    /// Apply one routed input event and return its callback-boundary activation decision.
    /// Row-targeted actions need a trusted row map (`pointer_rows` and `rows_match`); Back always
    /// reaches its binding.
    pub fn dispatch(&mut self, page: &Page, event: &UiEvent) -> LiveMenuEventDispatch {
        let rows_ok = self.pointer_rows && self.rows_match();
        if !rows_ok && !matches!(event, UiEvent::Back) {
            return LiveMenuEventDispatch {
                menu_action: None,
                activation: ActivationOutcome::NotRequested,
            };
        }
        let action = crate::menu_dispatch::dispatch_menu_event(page, &mut self.focus, event);
        let activation = if matches!(
            &action,
            Some(MenuAction::Activate { .. } | MenuAction::Disabled(_))
        ) {
            let focus = self.focus.clone();
            let map = std::mem::take(&mut self.map);
            let outcome = map.dispatch_activation(page, &focus, event, self.bindings());
            self.map = map;
            outcome
        } else {
            ActivationOutcome::NotRequested
        };
        match &action {
            Some(MenuAction::Focus(id)) => {
                if let Some(row) = self.row_of(*id) {
                    self.bindings()
                        .action(ACTION_FOCUS, ActionArg::Index(row as usize));
                }
            }
            Some(MenuAction::Activate { .. }) => {}
            Some(MenuAction::Back(_)) => {
                let map = std::mem::take(&mut self.map);
                let _ = map.apply(event, self.bindings());
                self.map = map;
            }
            Some(MenuAction::Disabled(_)) | None => {}
        }
        LiveMenuEventDispatch {
            menu_action: action,
            activation,
        }
    }

    /// Compatibility wrapper for callers that need only the model action.
    pub fn apply(&mut self, page: &Page, event: &UiEvent) -> Option<MenuAction> {
        self.dispatch(page, event).menu_action
    }

    /// T2168: right click on an option row steps its value back instead of Back. Returns false
    /// when `hovered` is not an option row (the caller then treats the click as Back).
    pub fn value_prev_at(&mut self, hovered: Option<WidgetId>) -> bool {
        if !self.rows_match() {
            return false;
        }
        match hovered.and_then(|id| self.row_of(id)) {
            Some(row) if self.is_cycler(row) => {
                self.bindings()
                    .action(ACTION_VALUE_PREV, ActionArg::Index(row as usize));
                true
            }
            _ => false,
        }
    }

    /// Owner standard: wheel only scrolls the view, never changes focus or values.
    /// Measurement pad routes cannot represent view scrolling and deliberately do nothing.
    pub fn wheel(&mut self, _hovered: Option<WidgetId>, notches: i32) {
        if notches == 0 || self.live.is_none() {
            return;
        }
        if let Dispatch::Native(native) = &mut self.dispatch {
            if let Ok(mut queue) = native.queue.lock() {
                queue.push_back(crate::native_page::NativeCommand::Scroll(
                    notches.clamp(-8, 8),
                ));
            }
        }
    }

    /// T2168: one plain step or button (keyboard on a page whose navigation the original keeps).
    pub fn tap(&mut self, button: PadButton) {
        match &mut self.dispatch {
            // T2310: a native list step, select or back.
            Dispatch::Native(native) => {
                let code = match button {
                    PadButton::Up => 0,
                    PadButton::Down => 1,
                    PadButton::Left => 2,
                    PadButton::Right => 3,
                    PadButton::A => {
                        let cursor = native.live.map_or(0, |live| live.cursor);
                        native.action(ACTION_SELECT, ActionArg::Index(cursor as usize));
                        return;
                    }
                    PadButton::B => {
                        native.action(ACTION_BACK, ActionArg::None);
                        return;
                    }
                };
                native.action(ACTION_STEP, ActionArg::Index(code));
            }
            Dispatch::PadRoute(taps) => {
                if taps.live.is_some() {
                    taps.driver.push(Goal::Tap(button));
                }
            }
        }
    }

    /// Measurement tool only: advance the pad-tap driver (no edges on a native page).
    pub fn tick(&mut self, now_ms: u64, out: &mut Vec<PadEdge>) {
        if let Dispatch::PadRoute(taps) = &mut self.dispatch {
            let live = taps.live;
            taps.driver.tick(now_ms, live, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::bindings::UiValue;

    /// A measurement-tool page over the plain page rows (the pre-T2316 `LiveMenu::new` with the tap driver).
    fn tapped(page: &Page, builder: u32) -> LiveMenu {
        let rows = page_rows(page);
        let cyclers = vec![false; rows.len()];
        let taps = Dispatch::PadRoute(PadRouteBindings {
            driver: PadRouteDriver::new(TapTiming::default()),
            live: None,
        });
        LiveMenu::with_rows(page, builder, rows, cyclers, true, taps)
    }

    fn taps(menu: &mut LiveMenu) -> &mut PadRouteBindings {
        match &mut menu.dispatch {
            Dispatch::PadRoute(taps) => taps,
            Dispatch::Native(_) => panic!("a measurement-tool page"),
        }
    }

    fn slot(cursor: u32, state: u32) -> LiveSlot {
        LiveSlot {
            builder: 0x2D0BE0,
            state,
            cursor,
            rows: 8,
        }
    }

    /// Guest stand-in: one DOWN/UP release moves the cursor, A raises the state.
    fn run(
        driver: &mut PadRouteDriver,
        mut live: LiveSlot,
        frames: u64,
    ) -> (LiveSlot, Vec<PadEdge>) {
        let mut all = Vec::new();
        for frame in 0..frames {
            let mut out = Vec::new();
            driver.tick(frame * 16, Some(live), &mut out);
            for edge in &out {
                if let PadEdge::Release(button) = edge {
                    match button {
                        PadButton::Down => live.cursor = (live.cursor + 1).min(live.rows - 1),
                        PadButton::Up => live.cursor = live.cursor.saturating_sub(1),
                        PadButton::A => live.state += 1,
                        PadButton::B => live.state += 10,
                        // Option values are not in the slot (open-loop taps).
                        PadButton::Left | PadButton::Right => {}
                    }
                }
            }
            all.extend(out);
        }
        (live, all)
    }

    #[test]
    fn select_moves_the_live_cursor_then_presses_a_once() {
        let mut driver = PadRouteDriver::default();
        driver.push(Goal::Select(3));
        let (live, edges) = run(&mut driver, slot(0, 0), 200);
        assert_eq!((live.cursor, live.state), (3, 1));
        let presses: Vec<_> = edges
            .iter()
            .filter_map(|e| match e {
                PadEdge::Press(b) => Some(*b),
                _ => None,
            })
            .collect();
        assert_eq!(
            presses,
            [
                PadButton::Down,
                PadButton::Down,
                PadButton::Down,
                PadButton::A
            ]
        );
        assert!(driver.idle());
        assert_eq!(driver.stats.confirmed, 4);
    }

    #[test]
    fn focus_then_back_and_up_moves() {
        let mut driver = PadRouteDriver::default();
        driver.push(Goal::Focus(5));
        driver.push(Goal::Focus(1)); // replaces the queued focus
        driver.push(Goal::Back);
        let (live, _) = run(&mut driver, slot(4, 0), 200);
        assert_eq!((live.cursor, live.state), (1, 10));
        assert_eq!(driver.stats.taps, 4);
    }

    #[test]
    fn unconfirmed_a_times_out_and_page_close_releases() {
        let mut driver = PadRouteDriver::default();
        driver.push(Goal::Select(0));
        let mut out = Vec::new();
        for frame in 0..100 {
            driver.tick(frame * 16, Some(slot(0, 0)), &mut out);
        }
        assert!(driver.idle());
        assert_eq!(driver.stats.unconfirmed, 1);
        driver.push(Goal::Back);
        out.clear();
        driver.tick(5000, Some(slot(0, 0)), &mut out);
        assert_eq!(out, [PadEdge::Press(PadButton::B)]);
        out.clear();
        driver.tick(5016, None, &mut out);
        assert_eq!(out, [PadEdge::Release(PadButton::B)]);
        assert!(driver.idle());
    }

    #[test]
    fn out_of_range_rows_are_dropped() {
        let mut driver = PadRouteDriver::default();
        driver.push(Goal::Select(9));
        let (live, edges) = run(&mut driver, slot(2, 0), 10);
        assert_eq!(live, slot(2, 0));
        assert!(edges.is_empty());
        assert_eq!(driver.stats.dropped_goals, 1);
    }

    #[test]
    fn macros_match_the_legacy_arcade_route() {
        // tools/data/routes/ui-arcade.tail opens Arcade with DOWN, A from the main menu.
        assert_eq!(goals_to_macros(&[Goal::Select(1)], 0, 8), ["DOWN", "A"]);
        assert_eq!(
            goals_to_macros(&[Goal::Focus(3), Goal::Select(2), Goal::Back], 0, 8),
            ["DOWN", "DOWN", "DOWN", "UP", "A", "B"]
        );
    }

    #[test]
    fn live_menu_routes_events_to_original_handlers() {
        use tsfp_ui_model::{PageId, Widget, WidgetKind};
        let mut page = Page::new(PageId(1), 1);
        for id in 1..=3 {
            page.widgets
                .push(Widget::new(WidgetId(id), WidgetKind::Button));
        }
        let mut menu = tapped(&page, 0x2D0BE0);
        assert!(!menu.observe(Some(LiveSlot {
            rows: 3,
            ..slot(0, 0)
        })));
        menu.apply(&page, &UiEvent::Activate(InputWidgetId(3)));
        menu.apply(&page, &UiEvent::Back);
        assert_eq!(
            taps(&mut menu)
                .driver
                .goals
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [Goal::Select(2), Goal::Back]
        );
        assert_eq!(taps(&mut menu).value(VALUE_ROWS), UiValue::Index(3));
        // Guest cursor change while idle is mirrored into the Rust focus.
        let mut idle = tapped(&page, 0x2D0BE0);
        assert!(idle.observe(Some(LiveSlot {
            rows: 3,
            ..slot(1, 0)
        })));
        assert_eq!(idle.focus.selected, Some(WidgetId(2)));
    }

    /// T3781: the two player main menu greys one of its 8 cursor rows. With the focusable widgets only (7 rows) the
    /// row map did not match the live slot and every Focus and Select was dropped (owner playtest 2026-10-26); with the
    /// guest cursor rows the greyed row keeps its index and Focus and Activate reach the right row.
    #[test]
    fn a_greyed_main_menu_row_keeps_its_cursor_position() {
        use crate::native_page::NativeCommand;
        use tsfp_ui_model::{PageId, Widget, WidgetKind};
        let mut page = Page::new(PageId(1), 1);
        for id in 0..8 {
            let mut row = Widget::new(WidgetId(id), WidgetKind::Button);
            row.enabled = id != 2;
            page.widgets.push(row);
        }
        let live = Some(slot(1, 2));
        let mut old = LiveMenu::new(&page, 0x2D0BE0);
        old.observe(live);
        assert!(!old.rows_match(), "the focusable rows alone are 7 of 8");
        assert_eq!(old.apply(&page, &UiEvent::Activate(InputWidgetId(3))), None);

        let rows = (0..8).map(WidgetId).collect();
        let mut menu = LiveMenu::with_cursor_rows(&page, 0x2D0BE0, Some(rows));
        menu.observe(live);
        assert!(menu.rows_match());
        menu.apply(&page, &UiEvent::Activate(InputWidgetId(3)));
        let queued: Vec<_> = menu
            .native()
            .unwrap()
            .queue
            .lock()
            .unwrap()
            .drain(..)
            .collect();
        assert_eq!(
            queued,
            [NativeCommand::Select(3)],
            "row 3 keeps guest cursor index 3"
        );
        // Focus on a row after the greyed one keeps its guest index too.
        menu.apply(&page, &UiEvent::Focus(InputWidgetId(5)));
        let queued: Vec<_> = menu
            .native()
            .unwrap()
            .queue
            .lock()
            .unwrap()
            .drain(..)
            .collect();
        assert_eq!(queued, [NativeCommand::Focus(5)]);
    }

    #[test]
    fn a_click_on_a_greyed_row_reaches_the_page() {
        // T3811: hover rests on a greyed row (focus_when_disabled, as `LivePage::to_model_page` sets it) and a click
        // queues Select, so the guest plays the denied sound or runs the row's help button.
        use crate::native_page::NativeCommand;
        use tsfp_ui_model::{PageId, Widget, WidgetKind};
        let mut page = Page::new(PageId(1), 1);
        for id in 0..8 {
            let mut row = Widget::new(WidgetId(id), WidgetKind::Button);
            row.enabled = id != 2;
            row.focus_when_disabled = id == 2;
            page.widgets.push(row);
        }
        let rows = (0..8).map(WidgetId).collect();
        let mut menu = LiveMenu::with_cursor_rows(&page, 0x2D0BE0, Some(rows));
        menu.observe(Some(slot(1, 2)));
        menu.apply(&page, &UiEvent::Hover(InputWidgetId(2)));
        menu.apply(&page, &UiEvent::Activate(InputWidgetId(2)));
        let queued: Vec<_> = menu
            .native()
            .unwrap()
            .queue
            .lock()
            .unwrap()
            .drain(..)
            .collect();
        assert_eq!(queued, [NativeCommand::Focus(2), NativeCommand::Select(2)]);
    }

    fn option_page() -> (Page, crate::page_map::PageMap) {
        use tsfp_ui_model::{PageId, TextRef, UiOption, Widget, WidgetKind};
        // FABRICATED settings-like page: plain row, divider, option row, plain row.
        let mut page = Page::new(PageId(7), 1);
        page.widgets
            .push(Widget::new(WidgetId(10), WidgetKind::Button));
        page.widgets
            .push(Widget::new(WidgetId(11), WidgetKind::Panel));
        let mut cycler = Widget::new(WidgetId(12), WidgetKind::Cycler);
        let option = |id| UiOption {
            id,
            text: TextRef::None,
            enabled: true,
        };
        cycler.options = vec![option(0), option(1)];
        page.widgets.push(cycler);
        page.widgets
            .push(Widget::new(WidgetId(13), WidgetKind::Button));
        let map = crate::page_map::PageMap::from_page(&page, 0x1000, 3);
        (page, map)
    }

    #[test]
    fn option_rows_take_clicks_but_wheel_never_changes_values() {
        let (page, map) = option_page();
        let mut menu = LiveMenu::pad_route(&page, &map, TapTiming::default());
        assert_eq!(menu.rows, [WidgetId(10), WidgetId(12), WidgetId(13)]);
        menu.observe(Some(LiveSlot {
            rows: 3,
            ..slot(0, 0)
        }));
        menu.apply(&page, &UiEvent::Activate(InputWidgetId(12)));
        assert!(menu.value_prev_at(Some(WidgetId(12))));
        assert!(
            !menu.value_prev_at(Some(WidgetId(13))),
            "plain row: right click is Back"
        );
        menu.wheel(Some(WidgetId(12)), -2);
        menu.wheel(Some(WidgetId(13)), 1);
        menu.tap(PadButton::Down);
        assert_eq!(
            taps(&mut menu).driver.queued(),
            [
                Goal::Adjust {
                    row: 1,
                    right: true
                },
                Goal::Adjust {
                    row: 1,
                    right: false
                },
                Goal::Tap(PadButton::Down),
            ]
        );
        // Closed loop: DOWN to the option row, RIGHT, LEFT, DOWN; wheel emitted nothing.
        let (live, edges) = run(&mut taps(&mut menu).driver, slot(0, 0), 400);
        let presses: Vec<_> = edges
            .iter()
            .filter_map(|e| match e {
                PadEdge::Press(b) => Some(*b),
                _ => None,
            })
            .collect();
        use PadButton::*;
        assert_eq!(presses, [Down, Right, Left, Down]);
        assert_eq!(live.cursor, 2);
        assert!(taps(&mut menu).driver.idle());
    }

    #[test]
    fn scroll_lists_and_mismatched_slots_get_no_row_goals() {
        let (page, mut map) = option_page();
        map.cursor_row_count = 2; // a scrolling window: rows cannot be trusted
        let mut list = LiveMenu::pad_route(&page, &map, TapTiming::default());
        list.observe(Some(LiveSlot {
            rows: 2,
            ..slot(0, 0)
        }));
        assert_eq!(
            list.apply(&page, &UiEvent::Activate(InputWidgetId(10))),
            None
        );
        assert_eq!(list.apply(&page, &UiEvent::Hover(InputWidgetId(13))), None);
        list.wheel(Some(WidgetId(12)), 1);
        list.apply(&page, &UiEvent::Back);
        assert_eq!(taps(&mut list).driver.queued(), [Goal::Back]);
        // A trusted map whose live slot shows another row count (page rebuilt): Back only.
        let (page, map) = option_page();
        let mut menu = LiveMenu::pad_route(&page, &map, TapTiming::default());
        menu.observe(Some(LiveSlot {
            rows: 5,
            ..slot(0, 0)
        }));
        assert_eq!(
            menu.apply(&page, &UiEvent::Activate(InputWidgetId(10))),
            None
        );
        assert!(!menu.value_prev_at(Some(WidgetId(12))));
    }

    #[test]
    fn unreachable_rows_are_dropped_not_oscillated() {
        let mut driver = PadRouteDriver::default();
        driver.push(Goal::Focus(5));
        // The stand-in guest never moves (cursor pinned): the goal gives up after rows+2 moves.
        let mut out = Vec::new();
        for frame in 0..2000 {
            driver.tick(frame * 16, Some(slot(0, 0)), &mut out);
        }
        assert!(driver.idle());
        assert_eq!(driver.stats.dropped_goals, 1);
    }

    #[test]
    fn macros_skip_non_navigable_rows() {
        let navigable = [true, false, true, true];
        let (macros, cursor) = goals_to_macros_on(
            &[
                Goal::Focus(2),
                Goal::Adjust {
                    row: 3,
                    right: false,
                },
                Goal::Tap(PadButton::Up),
                Goal::Select(0),
            ],
            0,
            &navigable,
        );
        assert_eq!(macros, ["DOWN", "DOWN", "LEFT", "UP", "UP", "A"]);
        assert_eq!(cursor, 0);
    }
}
