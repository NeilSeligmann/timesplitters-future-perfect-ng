// SPDX-License-Identifier: GPL-3.0-or-later
//! T2310 UI-12: the native page actions of the widget framework (focus, activate, back) as
//! readable Rust over a typed view of the original page record. This replaces the pad-press
//! shim (`LegacyMenuDriver`) on pages whose input the Rust UI owns: the Rust page decides the
//! row, writes the focus through to the page record and calls the ORIGINAL page and row
//! handlers through [`GuestUi::call`] (the `UiBindings` boundary while the handlers are still
//! lifted; a handler call, not emulation of input).
//!
//! Reference oracle: `0x7A7B0 game_ui_widget_per_frame_update_input_transitions_scroll_and_callbacks`
//! (cursor 0x7AE13..0x7AFAB, activation 0x7B06E..0x7B1A9, back 0x7B1A9..0x7B23A), the cursor-row
//! lookup 0x719D0 / 0x71A40 and the same-id ordinal 0x719A0 (docs/t1640-menu-state-findings.md
//! sections 3.4 and 3.5, docs/t-ui-framework.md section 1). Every guest call below is one the
//! original makes at that point, with the same arguments.

//! T2331: qualified initial front-end callbacks are now intercepted by
//! `frontend_handlers` and run on owned typed state; remaining neighbouring
//! services and unqualified callbacks keep the GuestUi boundary.

use std::sync::Mutex;
use tsfp_ui_row_input::command_queue::{self, Focus, FocusIdentity};

use crate::bindings::{
    ActionArg, ActionId, BindResult, ListId, ListView, TextId, UiBindings, UiValue, ValueId,
};
use crate::live_menu::{
    ACTION_BACK, ACTION_FOCUS, ACTION_SELECT, ACTION_STEP, ACTION_VALUE_NEXT, ACTION_VALUE_PREV,
    LiveSlot, VALUE_CURSOR, VALUE_ROWS,
};

/// Guest access the framework needs while the page handlers are lifted. Implemented by the
/// host (frame hook on the guest thread) and by test fakes.
pub trait GuestUi {
    fn read32(&self, va: u32) -> Option<u32>;
    fn write32(&mut self, va: u32, value: u32) -> bool;
    /// cdecl call of an original function; `None` when it could not be made.
    fn call(&mut self, va: u32, args: &[u32]) -> Option<u32>;
    /// cdecl call of an original function that returns a float in ST0 (NaN when it left none);
    /// `None` when it could not be made or the host has no such call.
    fn call_st0(&mut self, _va: u32, _args: &[u32]) -> Option<f64> {
        None
    }
    /// Run the owned Map Maker Load action when the current page is qualified.
    /// Hosts without the native storage shell keep the legacy callback path.
    fn native_map_load(&mut self) -> bool {
        false
    }
    /// Returns true when this host owns and handled the Map Maker Save action.
    fn native_map_save(&mut self) -> bool {
        false
    }
    /// Apply owned Map Maker undo/redo history when the host has adopted the editor document.
    /// `None` keeps the original history callbacks; `Some(false)` means the owned route claimed
    /// the action but currently has no record in that direction.
    fn native_map_history(&mut self, _direction: u32) -> Option<bool> {
        None
    }
    /// Run one of the T1697 settings page handlers over Rust-owned profile state.
    /// Hosts without the native settings runtime retain the lifted callback path.
    fn native_settings_handler(&mut self, _handler: u32, _page: PageRef) -> bool {
        false
    }
    /// Apply a Controls or Audio/Video value step in the owned settings edit.
    /// `Some(None)` means handled without an option index (for example Turn Speed).
    fn native_settings_step(
        &mut self,
        _page: PageRef,
        _row: u32,
        _direction: Step,
    ) -> Option<Option<i32>> {
        None
    }
}

pub use crate::menu_state::PageRef;
pub use crate::menu_state::layout::{PAGE_SLOTS, PAGE_STRIDE, PAGE_TABLE};
use crate::menu_state::layout::{page, row};
use crate::widgets::{self, Step};

pub use tsfp_ui_row_input::original;

/// T1916 increment 3: push button rows (kind 5) open the text entry from their record (`0x7FD20`, MEASURED from the
/// lift): text buffer `+0x48`, limit the signed word `+0x98` (224 for the Map Maker description), `+0xA4`, `+0xA8`, `+0x94`.
pub use tsfp_ui_row_input::button;

/// `0x7FD20` kind 5: open the text entry of button row `node` on `page`. The original only does this when the pointer is
/// inside the button box (`0x711F0`, a pointer click); the native activation is the press of the focused row, so the
/// pointer test is not part of it (rule 19: the activation path of the button, ported).
fn press_button(g: &mut dyn GuestUi, page: u32, node: u32) {
    let word = |o: u32| g.read32(node + o).unwrap_or(0);
    let limit = (g.read32((node + button::LIMIT) & !3).unwrap_or(0)
        >> (8 * ((node + button::LIMIT) & 3))) as u16 as i16 as i32;
    let args = [
        page,
        word(button::TEXT),
        limit as u32,
        word(button::ARG_A4),
        word(button::ARG_A8),
        word(button::ARG_94),
        original::TEXT_ENTRY_DONE,
        0,
    ];
    g.call(original::OPEN_TEXT_ENTRY, &args);
}

use tsfp_ui_row_input::page_global as global;
pub use tsfp_ui_row_input::sound;

/// T3817: pad X as 0x7A7B0 handles it (0x7B23A..0x7B28D): on a page with `FLAG_X_SHORTCUT` the X press edge calls the
/// page style's shortcut `[[page + 0x1C] + 0xA8]()`. The pages that show the X "Player Progress" prompt (Story levels,
/// Extras, Arcade type) open Player Progress this way. Without the flag or the callback X does nothing, as the original.
pub fn shortcut(g: &mut dyn GuestUi, page: PageRef) -> Outcome {
    if page.get(g, page::FLAGS) & page::FLAG_X_SHORTCUT == 0 {
        return Outcome::NoRow;
    }
    let style = page.get(g, page::STYLE);
    match g.read32(style.wrapping_add(page::STYLE_SHORTCUT)) {
        Some(callback) if style != 0 && callback != 0 => {
            g.call(callback, &[]);
            Outcome::Activated {
                row_handler: false,
                page_handler: true,
                kind: false,
            }
        }
        _ => Outcome::NoRow,
    }
}

/// Find the top live page slot (highest index) built by `builder`.
pub fn find_page(g: &dyn GuestUi, builder: u32) -> Option<(u32, PageRef)> {
    (0..PAGE_SLOTS)
        .rev()
        .map(|n| (n, PageRef::slot(n)))
        .find(|(_, p)| p.live(g) && p.builder(g) == builder)
}

/// What a native action did (for logs and tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Focused {
        row: u32,
        sound: bool,
    },
    Activated {
        row_handler: bool,
        page_handler: bool,
        kind: bool,
    },
    Denied,
    Vetoed,
    BackHandled {
        handler: bool,
        popped: bool,
    },
    Stepped(widgets::StepResult),
    Scrolled,
    NotReady,
    NoRow,
}

fn ui_mode(g: &dyn GuestUi) -> i32 {
    g.read32(global::UI_MODE).unwrap_or(0) as i32
}

/// Move the cursor to cursor row `n` as the original cursor step does (0x7AE50..0x7AFAB):
/// cursor, cursor row, cursor id and its ordinal, then the tick sound when the cursor changed.
/// Rows flagged not navigable are refused (the original skips them).
pub fn focus(g: &mut dyn GuestUi, page: PageRef, n: u32) -> Outcome {
    let Some(node) = page.cursor_row(g, n) else {
        return Outcome::NoRow;
    };
    if g.read32(node + row::FLAGS).unwrap_or(0) & row::FLAG_SKIP != 0 {
        return Outcome::NoRow;
    }
    let before = page.cursor(g);
    let column = page.column(g);
    page.store_cursor(g, n, column, node);
    widgets::scroll_cursor_row(g, page);
    let mut sound = false;
    if before != n
        && g.read32(global::SOUND_MUTED).unwrap_or(1) == 0
        && g.read32(global::SOUND_BANK_READY).unwrap_or(0) != 0
        && let Some(tick) = g.read32(global::SOUND_TICK_PTR).and_then(|p| g.read32(p))
    {
        g.call(original::PLAY_SOUND, &[tick]);
        sound = true;
    }
    Outcome::Focused { row: n, sound }
}

/// Run `body` with the current viewport global set to the page's viewport, as 0x7A7B0 does
/// around its whole update.
fn with_viewport<T>(
    g: &mut dyn GuestUi,
    page: PageRef,
    body: impl FnOnce(&mut dyn GuestUi) -> T,
) -> T {
    let viewport = page.get(g, page::VIEWPORT);
    g.write32(global::CURRENT_VIEWPORT, viewport);
    body(g)
}

/// A on the page (0x7B0AE..0x7B1A9): the page veto, then the cursor row's own handler, the
/// page's after-accept hook and the row kind's activation, or the page handler when no row
/// handler ran; accept or denied sound by UI mode.
pub fn activate(g: &mut dyn GuestUi, page: PageRef) -> Outcome {
    with_viewport(g, page, |g| {
        let p = page.0;
        if g.call(original::PAGE_ACCEPTS, &[p]).unwrap_or(0) == 0 {
            return Outcome::Vetoed;
        }
        // T3811: a greyed entry with a help button beside it answers with the help (native, see `help_member`).
        if let Some((column, member)) = help_member(g, page) {
            let cursor = page.cursor(g);
            page.store_cursor(g, cursor, column, member);
        }
        let node = page.get(g, page::CURSOR_ROW);
        let mut row_handler = false;
        let mut kind = false;
        let mut denied = false;
        let mut unhandled_kind = None;
        if node != 0 {
            let flags = g.read32(node + row::FLAGS).unwrap_or(0);
            if flags & (row::FLAG_GREYED | row::FLAG_SKIP) != 0 {
                denied = true;
                if ui_mode(g) == 0x65 {
                    g.call(original::PLAY_SOUND, &[sound::DENIED]);
                }
            } else {
                let mut fired = 0u32;
                let handler = g.read32(node + row::ON_ACTIVATE).unwrap_or(0);
                if handler != 0 {
                    fired = 1;
                    row_handler = true;
                    if !crate::mapmaker_handlers::dispatch(g, page, handler)
                        && !crate::ingame_handlers::dispatch(g, page, handler)
                        && !crate::frontend_handlers::dispatch(g, page, handler)
                    {
                        g.call(handler, &[p]);
                    }
                }
                let after = page.get(g, page::AFTER_ACCEPT);
                if after != 0 && g.read32(node + row::NO_AFTER_ACCEPT).unwrap_or(0) != 1 {
                    g.call(after, &[]);
                    fired |= 2;
                }
                match g.read32(node + row::KIND).unwrap_or(0) {
                    0 => {}
                    button::KIND => {
                        kind = true;
                        press_button(g, p, node);
                    }
                    // T3809: 0x800A0 has no kind 7 case (the slider moves only under a pointer drag); the native
                    // UI steps it up with wrap so Enter, pad A and a click reach every value.
                    row::KIND_SLIDER => {
                        kind = true;
                        widgets::slider_step(g, node, true, true);
                    }
                    other => {
                        kind = true;
                        g.call(original::ROW_KIND_ACTIVATE, &[p, node]);
                        if !KIND_ACTIVATIONS.contains(&other) {
                            unhandled_kind = Some(other);
                        }
                    }
                }
                if fired != 0 && ui_mode(g) <= 0x64 {
                    g.call(original::PLAY_SOUND, &[sound::ACCEPT]);
                }
            }
        }
        // The page handler runs when no row handler ran, except on a greyed row without one
        // (0x7B155..0x7B1A7; a not-navigable row with a handler still reaches it, as in the original).
        let mut page_handler = false;
        let on_accept = page.get(g, page::ON_ACCEPT);
        let greyed_without_handler = node != 0
            && g.read32(node + row::FLAGS).unwrap_or(0) & row::FLAG_GREYED != 0
            && g.read32(node + row::ON_ACTIVATE).unwrap_or(0) == 0;
        if on_accept != 0 && !greyed_without_handler && !row_handler {
            // T2316: handlers that read the activating controller run natively (`native_handlers`).
            match crate::native_handlers::native_accept(on_accept) {
                Some(native) => native(g, page),
                None => {
                    if !crate::mapmaker_handlers::dispatch(g, page, on_accept)
                        && !crate::ingame_handlers::dispatch(g, page, on_accept)
                    {
                        g.call(on_accept, &[p]);
                    }
                }
            }
            page_handler = true;
            if ui_mode(g) <= 0x64 {
                g.call(original::PLAY_SOUND, &[sound::ACCEPT]);
            }
            let after = page.get(g, page::AFTER_ACCEPT);
            if after != 0 {
                g.call(after, &[]);
            }
        }
        if denied && !page_handler {
            return Outcome::Denied;
        }
        if let Some(row_kind) = unhandled_kind.filter(|_| !row_handler && !page_handler) {
            note_unhandled_kind(g, page, row_kind);
        }
        Outcome::Activated {
            row_handler,
            page_handler,
            kind,
        }
    })
}

/// T3811 (native): the help button beside a greyed cursor row with no handler, as (column, node)
/// ([`tsfp_ui_row_input::help_column`]). Pressing the greyed name runs it, so the original reason message (`0x3126D0`
/// on the Map Maker preview page) is never hidden behind a silent row; the original needs Right then A.
pub fn help_member(g: &dyn GuestUi, page: PageRef) -> Option<(i32, u32)> {
    let node = page.get(g, page::CURSOR_ROW);
    let get = |va: u32| g.read32(va).unwrap_or(0);
    if node == 0
        || get(node + row::FLAGS) & row::FLAG_GREYED == 0
        || get(node + row::ON_ACTIVATE) != 0
        || page.column(g) != 0
    {
        return None;
    }
    let width = get(node + row::GROUP_WIDTH).min(8) as i32;
    let nodes: Vec<u32> = (0..width)
        .map(|c| widgets::cursor_row_at(g, page, page.cursor(g), c))
        .collect();
    let members: Vec<_> = (0..nodes.len())
        .map(|c| tsfp_ui_row_input::GroupMember {
            usable: c > 0
                && nodes[c] != 0
                && nodes[c] != node
                && get(nodes[c] + row::FLAGS) & (row::FLAG_GREYED | row::FLAG_SKIP) == 0,
            has_handler: get(nodes[c] + row::ON_ACTIVATE) != 0,
        })
        .collect();
    tsfp_ui_row_input::help_column(&members).map(|c| (c as i32, nodes[c]))
}

/// B on the page (0x7B1BE..0x7B23A): the page veto, then the page's back handler, or a context
/// pop on pages flagged so, then the after-back hook; back sound by UI mode.
pub fn back(g: &mut dyn GuestUi, page: PageRef) -> Outcome {
    with_viewport(g, page, |g| {
        let p = page.0;
        if g.call(original::PAGE_ACCEPTS, &[p]).unwrap_or(0) == 0 {
            return Outcome::Vetoed;
        }
        let mut handled = 0u32;
        let on_back = page.get(g, page::ON_BACK);
        if on_back != 0 {
            let id = page.get(g, page::BACK_ID);
            g.write32(p + page::CURSOR_ID, id);
            if !crate::mapmaker_handlers::dispatch(g, page, on_back)
                && !crate::ingame_handlers::dispatch(g, page, on_back)
                && !crate::frontend_handlers::dispatch(g, page, on_back)
            {
                g.call(on_back, &[p]);
            }
            handled = 1;
        } else if page.get(g, page::FLAGS) & page::FLAG_BACK_POPS != 0 {
            let context = g.call(original::PARENT_CONTEXT, &[p, 1]).unwrap_or(0);
            let parent = g.read32(context + 0x2C).unwrap_or(0);
            crate::page_stack::pop(g, PageRef(parent));
            handled = 2;
        }
        let after = page.get(g, page::AFTER_BACK);
        if after != 0 {
            g.call(after, &[]);
            handled |= 4;
        }
        if handled != 0 && ui_mode(g) <= 0x64 {
            g.call(original::PLAY_SOUND, &[sound::BACK]);
        }
        Outcome::BackHandled {
            handler: handled & 1 != 0,
            popped: handled & 2 != 0,
        }
    })
}

/// One list step on the page (0x7A7B0's cursor and value step), with the current viewport set as
/// for the other actions. Map Maker's native UI owns keyboard/pad row navigation even while the
/// original viewport is in pointer mode; other pages retain the original pointer-mode gate.
pub fn step(g: &mut dyn GuestUi, page: PageRef, dir: Step) -> Outcome {
    // T3799: an option list takes the cursor policy of the page it opened over.
    let builder = popup_identity(g, page);
    let native_cursor = crate::mapmaker_handlers::NATIVE_CURSOR_PAGES.contains(&builder)
        || crate::mapmaker_handlers::INFERRED_CURSOR_PAGE_ALIASES.contains(&builder)
        || crate::mapmaker_handlers::is_mode_choice_popup(g, page);
    if native_cursor && input_with_child(g, page) && page.cursor_node(g) == 0 {
        if !matches!(dir, Step::Up | Step::Down) {
            return Outcome::NoRow;
        }
        let index =
            tsfp_ui_row_input::page_stack::initial_focus(page.row_count(g), dir == Step::Up, |i| {
                page.cursor_row(g, i).is_some_and(|node| {
                    g.read32(node + row::FLAGS).unwrap_or(0) & row::FLAG_SKIP == 0
                })
            });
        return index.map_or(Outcome::NoRow, |i| focus(g, page, i));
    }
    with_viewport(g, page, |g| {
        match widgets::step_with_pointer_context(g, page, dir, native_cursor) {
            Some(result) => Outcome::Stepped(result),
            None => Outcome::NoRow,
        }
    })
}

/// T2181 host-boundary action: dismiss the in-game BACK pages through the measured
/// Resume handler's contract. It is viewport-based and takes no arguments (0x2EBB80),
/// shared with PauseAction::Resume; its game-state effects are owned by that system.
/// T2327 also admits pause root for Escape, through the same Continue handler.
/// Ordinary Back deliberately stays a no-op on these pages in the original preset.
pub fn resume_game(g: &mut dyn GuestUi, page: PageRef) -> Outcome {
    if !matches!(page.builder(g), 0x2F_5520 | 0x2F_5920 | 0x2F_5E60)
        || page.get(g, page::VIEWPORT) >= 4
    {
        return Outcome::Denied;
    }
    with_viewport(g, page, |g| {
        if g.call(original::PAGE_ACCEPTS, &[page.0]).unwrap_or(0) == 0 {
            return Outcome::Vetoed;
        }
        let handler = tsfp_ui_model::pause_actions::PauseAction::Resume.handler();
        if !crate::mapmaker_handlers::dispatch(g, page, handler)
            && !crate::ingame_handlers::dispatch(g, page, handler)
        {
            g.call(handler, &[]);
        }
        Outcome::BackHandled {
            handler: true,
            popped: false,
        }
    })
}

/// Rust-owned scroll policy. The write-through GuestUi adapter is a TEMPORARY-SHIM
/// until T2308 replaces the remaining framework page records.
pub use tsfp_ui_row_input::scroll::ScrollView;
fn scroll_view(g: &mut dyn GuestUi, p: PageRef, notches: i32) -> Outcome {
    if p.get(g, page::FLAGS) & page::FLAG_NO_SCROLL != 0 {
        return Outcome::Denied;
    }
    let view = ScrollView {
        offset: f32::from_bits(p.get(g, page::SCROLL)),
        content: f32::from_bits(p.get(g, page::CONTENT_HEIGHT)),
        height: f32::from_bits(p.get(g, page::VIEW_BOTTOM))
            - f32::from_bits(p.get(g, page::VIEW_TOP)),
    };
    let Some(offset) = view.scroll(notches) else {
        return Outcome::Denied;
    };
    if g.write32(p.0 + page::SCROLL, offset.to_bits()) {
        Outcome::Scrolled
    } else {
        Outcome::Denied
    }
}

/// A page action the Rust page asked for, run at the next guest frame boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeCommand {
    Focus(u32),
    Select(u32),
    /// Activate the row the cursor is on when the command runs (keyboard and pad A).
    Accept,
    Back,
    /// Existing Resume contract: pause-root Escape or opt-in Close on BACK pages.
    ResumeGame,
    /// One list step (up/down move the cursor, left/right the group column and option value).
    Step(Step),
    /// View-only scrolling; never focuses or invokes an option callback.
    Scroll(i32),
    /// Change the value of option row `row`: focus it, then one left (`right == false`) or right step.
    Adjust {
        row: u32,
        right: bool,
    },
    /// T3770: the page's delete action (pad Y, keyboard Delete); only pages that have one act on it.
    Delete,
    /// T3772: START on controller port `n` on a page where a player waits for a controller (the profile page join).
    Join(u8),
    /// T3817: pad X on a page flagged for the X shortcut (the front-end "Player Progress" prompt), see [`shortcut`].
    Shortcut,
}

/// The queue between the input thread and guest frame hook, with page-local native focus.
pub type CommandQueue = command_queue::CommandQueue<NativeCommand>;

/// Run the queued commands against the page built by `builder` (guest thread, frame boundary).
/// Commands wait while the page is not ready for input (state 2 or 3 and the original
/// readiness test 0x74760), so nothing is lost across the opening transition.
pub fn service(
    g: &mut dyn GuestUi,
    builder: u32,
    queue: &CommandQueue,
    out: &mut Vec<(NativeCommand, Outcome)>,
) {
    let page = (builder != 0)
        .then(|| find_page(g, builder))
        .flatten()
        .map(|(_, page)| page);
    service_on(g, page, queue, out);
}

/// Top live page, following active children rather than treating allocation order as stack order.
/// T3853: the settings warning can reuse a lower slot than its parent. Ignore stale links; a cycle is not a page.
pub fn top_page(g: &dyn GuestUi) -> Option<PageRef> {
    tsfp_ui_row_input::page_stack::top(
        (0..PAGE_SLOTS).map(PageRef::slot).filter(|p| p.live(g)),
        |p| PageRef(p.get(g, page::ACTIVE_CHILD)),
    )
}

/// 0x7A7B0's input exception: a flagged page stays interactive when its
/// deepest active child is its immediate child (the child still draws).
fn input_with_child(g: &dyn GuestUi, parent: PageRef) -> bool {
    top_page(g).is_some_and(|top| top != parent && input_page(g) == Some(parent))
}

/// Input owner may be the parent even while its companion child is visually on top.
pub fn input_page(g: &dyn GuestUi) -> Option<PageRef> {
    let top = top_page(g)?;
    Some(tsfp_ui_row_input::page_stack::input_owner(
        top,
        (0..PAGE_SLOTS).map(PageRef::slot).filter(|p| p.live(g)),
        |p| PageRef(p.get(g, page::ACTIVE_CHILD)),
        |p| PageRef(p.get(g, page::PARENT)),
        |p| p.get(g, page::FLAGS) & page::FLAG_INPUT_WITH_CHILD != 0,
    ))
}

/// The popup (or pushed child page) shown over a page built by one of `builders`: the top live page slot when it is
/// the active child (`+0x170`, linked by 0x773B0) of such a page and not a text input widget (those are driven by
/// [`crate::native_text`]). Confirm dialogs (0x7A420 Yes/No), message popups (0x7A610) and option lists (0x7FA00)
/// open this way; their rows and handlers are ordinary framework rows, so the same native actions drive them.
pub fn popup_over(g: &dyn GuestUi, builders: &[u32]) -> Option<PageRef> {
    let top = top_page(g)?;
    if crate::native_text::TEXT_WIDGET_IDS.contains(&top.get(g, page::ID)) {
        return None;
    }
    let has_parent = |child: PageRef| {
        (0..PAGE_SLOTS)
            .map(PageRef::slot)
            .filter(|p| *p != child && p.live(g) && builders.contains(&p.builder(g)))
            .any(|p| p.get(g, page::ACTIVE_CHILD) == child.0 && !input_with_child(g, p))
    };
    if has_parent(top) {
        return Some(top);
    }
    // T3853: allocation order is not stack order. Confirm/message pages, like option lists, can occupy an earlier
    // free slot than their parent. Follow the active-child link, keeping text entry with native_text.
    (0..PAGE_SLOTS).map(PageRef::slot).find(|p| {
        p.live(g)
            && !crate::native_text::TEXT_WIDGET_IDS.contains(&p.get(g, page::ID))
            && has_parent(*p)
    })
}

/// The page-level accept handler `0x75B70` installs on the option list a dropdown row (kind 2) opens in `0x7FA00`:
/// it writes the chosen option to the parent row (`0x75430`) and closes the list (`0x78D80`). MEASURED from the lift.
pub const OPTION_LIST_ACCEPT: u32 = 0x7_BD40;

/// Only the dropdown list uses pointer hover and the original list draw; a builder-less confirmation does not.
pub fn is_option_list(g: &dyn GuestUi, popup: PageRef) -> bool {
    popup.builder(g) == 0 && popup.get(g, page::ON_ACCEPT) == OPTION_LIST_ACCEPT
}

/// The input identity of `popup`: its builder, or its parent's builder for a descriptor-less child. The live page
/// snapshot skips builder 0 slots, so input still identifies the parent. This applies to confirm/message dialogs
/// as well as option lists (T3853 recurrence of T3799); requiring an option-list accept handler stranded commands
/// on the unready parent of the Map Maker exit confirmation. The actual command target remains the child page.
pub fn popup_identity(g: &dyn GuestUi, popup: PageRef) -> u32 {
    let builder = popup.builder(g);
    if builder != 0 {
        return builder;
    }
    (0..PAGE_SLOTS)
        .map(PageRef::slot)
        .find(|p| *p != popup && p.live(g) && p.get(g, page::ACTIVE_CHILD) == popup.0)
        .map_or(0, |parent| parent.builder(g))
}

/// Guest frames queued commands may wait for a page that is not ready before the stall is reported (about 2 s).
pub use tsfp_ui_row_input::page_stack::{INPUT_STALL_FRAMES, InputStall};
static INPUT_STALL: Mutex<InputStall> = Mutex::new(InputStall::new());

/// Report a stall once, so input routed to a page that cannot take it is loud in host.err and the play stop report.
fn note_input_wait(g: &dyn GuestUi, page: PageRef, pending: usize) {
    let Some(frames) = INPUT_STALL.lock().ok().and_then(|mut stall| stall.wait()) else {
        return;
    };
    let child = PageRef(page.get(g, page::ACTIVE_CHILD));
    eprintln!(
        "native page input stalled: {pending} command(s) waited {frames} frames on page {:#x} builder {:#x} \
         state {} (not ready for input); active child {:#x} builder {:#x} (I-T3799-1)",
        page.0,
        page.builder(g),
        page.state(g),
        child.0,
        if child.0 == 0 { 0 } else { child.builder(g) },
    );
}

pub use tsfp_ui_row_input::{KIND_ACTIVATIONS, unhandled_kinds};

/// T3809 detector (I-T3809-1, [`tsfp_ui_row_input::unhandled_activation`]): the press did nothing.
fn note_unhandled_kind(g: &dyn GuestUi, page: PageRef, row_kind: u32) {
    if let Some(line) = tsfp_ui_row_input::unhandled_activation(page.0, page.builder(g), row_kind) {
        eprintln!("{line}");
    }
}

fn clear_input_wait() {
    if let Ok(mut stall) = INPUT_STALL.lock() {
        *stall = InputStall::default();
    }
}

/// [`service`] on a given page (`None`: the page is gone, the queue is dropped).
pub fn service_on(
    g: &mut dyn GuestUi,
    page: Option<PageRef>,
    queue: &CommandQueue,
    out: &mut Vec<(NativeCommand, Outcome)>,
) {
    let Ok(mut pending) = queue.lock() else {
        return;
    };
    let Ok(mut owned_focus) = queue.focus.lock() else {
        return;
    };
    let Some(page) = page else {
        *owned_focus = None;
        pending.clear();
        clear_input_wait();
        return;
    };
    let tracked =
        page.builder(g) == 0x32_78D0 && input_with_child(g, page) && matches!(page.state(g), 2 | 3);
    let identity = tracked.then(|| FocusIdentity {
        page: page.0,
        child: page.get(g, page::ACTIVE_CHILD),
        first: page.get(g, page::FIRST_NODE),
    });
    command_queue::synchronize(&mut owned_focus, identity);
    if pending.is_empty() {
        return;
    }
    if !matches!(page.state(g), 2 | 3)
        || (g.call(original::PAGE_READY, &[page.0]).unwrap_or(0) == 0 && !input_with_child(g, page))
    {
        note_input_wait(g, page, pending.len());
        return;
    }
    clear_input_wait();
    if tracked && page.cursor_node(g) == 0 {
        if let Some(f) = *owned_focus {
            if widgets::cursor_row_at(g, page, f.index, f.column) == f.node {
                page.store_cursor(g, f.index, f.column, f.node);
            } else {
                *owned_focus = None;
            }
        }
    }
    // One activation or back per frame: either may replace the page.
    while let Some(command) = pending.pop_front() {
        let outcome = match command {
            NativeCommand::Focus(n) => focus(g, page, n),
            NativeCommand::Select(n) => match focus(g, page, n) {
                Outcome::Focused { .. } => activate(g, page),
                other => other,
            },
            NativeCommand::Accept => {
                crate::profile_page::accept(g, page).unwrap_or_else(|| activate(g, page))
            }
            NativeCommand::Delete => crate::profile_page::delete(g, page).unwrap_or(Outcome::NoRow),
            NativeCommand::Join(port) => {
                crate::profile_page::join(g, page, port).unwrap_or(Outcome::NoRow)
            }
            NativeCommand::Back => back(g, page),
            NativeCommand::Shortcut => shortcut(g, page),
            NativeCommand::ResumeGame => resume_game(g, page),
            NativeCommand::Step(dir) => step(g, page, dir),
            NativeCommand::Scroll(notches) => scroll_view(g, page, notches),
            NativeCommand::Adjust { row, right } => match focus(g, page, row) {
                Outcome::Focused { .. } => {
                    step(g, page, if right { Step::Right } else { Step::Left })
                }
                other => other,
            },
        };
        if let Some(identity) = identity {
            let node = page.cursor_node(g);
            *owned_focus = (node != 0).then(|| Focus {
                identity,
                index: page.cursor(g),
                column: page.column(g),
                node,
            });
        }
        out.push((command, outcome));
        if matches!(
            command,
            NativeCommand::Select(_)
                | NativeCommand::Accept
                | NativeCommand::Back
                | NativeCommand::ResumeGame
                | NativeCommand::Delete
                | NativeCommand::Shortcut
                | NativeCommand::Join(_)
                | NativeCommand::Shortcut
        ) {
            pending.clear();
            break;
        }
    }
}

/// `UiBindings` of a page whose actions run natively: actions queue [`NativeCommand`]s.
#[derive(Clone, Debug, Default)]
pub struct NativeMenuBindings {
    pub queue: CommandQueue,
    pub live: Option<LiveSlot>,
}

impl NativeMenuBindings {
    pub fn pending(&self) -> bool {
        self.queue.lock().map(|q| !q.is_empty()).unwrap_or(false)
    }
}

impl UiBindings for NativeMenuBindings {
    fn action(&mut self, id: ActionId, arg: ActionArg) -> BindResult {
        if self.live.is_none() {
            return BindResult::Denied;
        }
        let row = match arg {
            ActionArg::Index(row) => u32::try_from(row).ok(),
            ActionArg::Int(row) => u32::try_from(row).ok(),
            _ => None,
        };
        let command = match (id, row) {
            (ACTION_BACK, _) => NativeCommand::Back,
            (ACTION_SELECT, Some(row)) => NativeCommand::Select(row),
            (ACTION_FOCUS, Some(row)) => NativeCommand::Focus(row),
            (ACTION_VALUE_PREV, Some(row)) => NativeCommand::Adjust { row, right: false },
            (ACTION_VALUE_NEXT, Some(row)) => NativeCommand::Adjust { row, right: true },
            (ACTION_STEP, Some(code)) => match code {
                0 => NativeCommand::Step(Step::Up),
                1 => NativeCommand::Step(Step::Down),
                2 => NativeCommand::Step(Step::Left),
                3 => NativeCommand::Step(Step::Right),
                _ => return BindResult::Ignored,
            },
            _ => return BindResult::Ignored,
        };
        let Ok(mut queue) = self.queue.lock() else {
            return BindResult::Denied;
        };
        // Focus moves collapse: only the latest target matters.
        if matches!(command, NativeCommand::Focus(_))
            && matches!(queue.back(), Some(NativeCommand::Focus(_)))
        {
            queue.pop_back();
        }
        queue.push_back(command);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Fake {
        mem: HashMap<u32, u32>,
        calls: Vec<(u32, Vec<u32>)>,
        returns: HashMap<u32, u32>,
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
            Some(*self.returns.get(&va).unwrap_or(&1))
        }
    }

    const BUILDER: u32 = 0x2D_0BE0;

    /// Slot 3 holds a page with a divider then three button rows (ids 10, 11, 12); row 1 has a
    /// handler, row 2 is greyed.
    fn menu() -> (Fake, PageRef, [u32; 3]) {
        let mut f = Fake::default();
        let p = PageRef::slot(3);
        f.mem.insert(p.0, 0x52_4598);
        f.mem.insert(p.0 + page::BUILDER, BUILDER);
        f.mem.insert(p.0 + page::STATE, 3);
        f.mem.insert(p.0 + page::ROW_COUNT, 3);
        f.mem.insert(p.0 + page::VIEWPORT, 0);
        f.mem.insert(p.0 + page::ON_ACCEPT, 0x2D_1000);
        let divider = 0x100_0000;
        let rows = [0x100_1000, 0x100_2000, 0x100_3000];
        f.mem.insert(p.0 + page::FIRST_NODE, divider);
        f.mem.insert(divider + row::TYPE, 9);
        f.mem.insert(divider + row::NEXT, rows[0]);
        f.mem.insert(divider + row::NEXT_CURSOR, rows[0]);
        f.mem.insert(divider + row::ID, u32::MAX);
        for (i, r) in rows.iter().enumerate() {
            f.mem.insert(r + row::TYPE, 7);
            f.mem.insert(r + row::ID, 10 + i as u32);
            let next = rows.get(i + 1).copied().unwrap_or(0);
            f.mem.insert(r + row::NEXT, next);
            f.mem.insert(r + row::NEXT_CURSOR, next);
        }
        f.mem.insert(rows[1] + row::ON_ACTIVATE, 0x2D_2000);
        f.mem.insert(rows[2] + row::FLAGS, row::FLAG_GREYED);
        f.mem.insert(global::UI_MODE, 0x64);
        (f, p, rows)
    }

    fn assert_pointer_context_cursor_navigation(builder: u32) {
        let (mut f, p, rows) = menu();
        f.mem.insert(p.0 + page::BUILDER, builder);
        f.mem.insert(p.0 + page::CURSOR, 0);
        f.mem.insert(p.0 + page::CURSOR_ROW, rows[0]);
        f.mem.insert(widgets::global::VIEWPORT_CONTEXTS, 1);
        assert!(
            matches!(
                step(&mut f, p, Step::Down),
                Outcome::Stepped(widgets::StepResult { cursor: 1, .. })
            ),
            "native Map Maker page {builder:#x} must accept keyboard/pad cursor input in pointer mode"
        );
        assert_eq!(
            activate(&mut f, p),
            Outcome::Activated {
                row_handler: true,
                page_handler: false,
                kind: false,
            },
            "native Map Maker page {builder:#x} must activate the focused row"
        );
    }

    #[test]
    fn mapmaker_editor_menu_accepts_keyboard_and_pad_while_pointer_context_is_active() {
        assert_pointer_context_cursor_navigation(0x31_3010);
    }

    #[test]
    fn mapmaker_editor_configuration_pages_accept_keyboard_and_pad_while_pointer_is_active() {
        for builder in [
            0x31_33E0, 0x31_3FE0, 0x31_2270, 0x31_5720, 0x31_4420, 0x32_7A70,
        ] {
            assert_pointer_context_cursor_navigation(builder);
        }
    }

    #[test]
    fn mapmaker_option_list_takes_keyboard_and_pad_steps_like_its_parent() {
        // T3799: the Theme list (builder 0) over Map Settings steps in pointer mode as Map Settings does.
        let (mut f, list, rows) = menu();
        f.mem.insert(list.0 + page::BUILDER, 0);
        f.mem.insert(list.0 + page::ON_ACCEPT, OPTION_LIST_ACCEPT);
        f.mem.insert(list.0 + page::CURSOR, 0);
        f.mem.insert(list.0 + page::CURSOR_ROW, rows[0]);
        f.mem.insert(widgets::global::VIEWPORT_CONTEXTS, 1);
        assert_eq!(
            step(&mut f, list, Step::Down),
            Outcome::NoRow,
            "no parent: no policy"
        );
        let settings = PageRef::slot(2);
        f.mem.insert(settings.0, 0x52_4598);
        f.mem.insert(settings.0 + page::BUILDER, 0x31_33E0);
        f.mem.insert(settings.0 + page::ACTIVE_CHILD, list.0);
        assert!(matches!(
            step(&mut f, list, Step::Down),
            Outcome::Stepped(widgets::StepResult { cursor: 1, .. })
        ));
    }

    #[test]
    fn mapmaker_storage_pages_accept_keyboard_and_pad_while_pointer_is_active() {
        for builder in [0x32_B6A0, 0x32_BEA0, 0x32_AEE0] {
            assert_pointer_context_cursor_navigation(builder);
        }
    }

    #[test]
    fn mapmaker_owner_route_aliases_accept_keyboard_and_pad_while_pointer_context_is_active() {
        for builder in [0x2B_F770, 0x19_2110] {
            assert_pointer_context_cursor_navigation(builder);
        }
    }

    #[test]
    fn mapmaker_confirmation_page_accepts_keyboard_and_pad_while_pointer_is_active() {
        assert_pointer_context_cursor_navigation(0x7_A080);
    }

    #[test]
    fn mapmaker_select_mode_popup_accepts_keyboard_and_pad_in_pointer_context() {
        let (mut f, popup, rows) = menu();
        let parent = PageRef::slot(2);
        f.mem.insert(parent.0, 1);
        f.mem.insert(parent.0 + page::BUILDER, 0x2C_8940);
        f.mem.insert(parent.0 + page::ACTIVE_CHILD, popup.0);
        f.mem.insert(popup.0, 2); // descriptor-less original type-2 child
        f.mem.insert(popup.0 + page::BUILDER, 0);
        f.mem.insert(popup.0 + page::CURSOR, 0);
        f.mem.insert(popup.0 + page::CURSOR_ROW, rows[0]);
        f.mem.insert(rows[0] + row::ON_ACTIVATE, 0x31_2B60);
        f.mem.insert(rows[1] + row::ON_ACTIVATE, 0x31_2B60);
        f.mem.insert(global::UI_MODE, 0x66);
        f.mem.insert(widgets::global::VIEWPORT_CONTEXTS, 1);

        assert!(matches!(
            step(&mut f, popup, Step::Down),
            Outcome::Stepped(widgets::StepResult { cursor: 1, .. })
        ));
        assert!(matches!(
            activate(&mut f, popup),
            Outcome::Activated {
                row_handler: true,
                ..
            }
        ));
        assert!(f.calls.iter().any(|(target, _)| *target == 0x31_2B60));
    }

    #[test]
    fn other_pages_keep_the_original_pointer_context_navigation_gate() {
        let (mut f, p, rows) = menu();
        f.mem.insert(p.0 + page::CURSOR_ROW, rows[0]);
        f.mem.insert(widgets::global::VIEWPORT_CONTEXTS, 1);
        assert_eq!(step(&mut f, p, Step::Down), Outcome::NoRow);
    }

    #[test]
    fn wheel_scroll_contract_preserves_cursor_values_and_callbacks_per_page_class() {
        for builder in [
            BUILDER, 0x2C_3E20, 0x2E_A8E0, 0x2D_62F0, 0x2F_5520, 0x2F_5920, 0x2C_8940,
        ] {
            let (mut f, p, _) = menu();
            f.mem.insert(p.0 + page::BUILDER, builder);
            f.mem.insert(p.0 + page::VIEW_TOP, 100f32.to_bits());
            f.mem.insert(p.0 + page::VIEW_BOTTOM, 300f32.to_bits());
            f.mem.insert(p.0 + page::CONTENT_HEIGHT, 500f32.to_bits());
            f.mem.insert(p.0 + page::SCROLL, 80f32.to_bits());
            let before = f.mem.clone();
            assert_eq!(scroll_view(&mut f, p, -2), Outcome::Scrolled);
            assert_eq!(f.mem[&(p.0 + page::SCROLL)], 160f32.to_bits());
            f.mem
                .insert(p.0 + page::SCROLL, before[&(p.0 + page::SCROLL)]);
            assert_eq!(f.mem, before, "only view offset changes: {builder:#x}");
            assert!(f.calls.is_empty());
            f.mem.insert(p.0 + page::CONTENT_HEIGHT, 200f32.to_bits());
            let before = f.mem.clone();
            assert_eq!(scroll_view(&mut f, p, -1), Outcome::Denied);
            assert_eq!(f.mem, before);
        }
        let view = ScrollView {
            offset: 80.0,
            content: 500.0,
            height: 200.0,
        };
        assert_eq!(view.scroll(999), Some(0.0));
        assert_eq!(view.scroll(-999), Some(300.0));
        assert_eq!(
            ScrollView {
                offset: f32::NAN,
                ..view
            }
            .scroll(1),
            None
        );
    }

    #[test]
    fn pause_root_uses_same_resume_handler_without_selecting_a_row() {
        let (mut f, p, _) = menu();
        f.mem.insert(p.0 + page::BUILDER, 0x2F_5520);
        assert_eq!(
            resume_game(&mut f, p),
            Outcome::BackHandled {
                handler: true,
                popped: false
            }
        );
        let tail = if crate::ingame_handlers::enabled_for(0x2EBB80) {
            (0x79B70, vec![0, 0])
        } else {
            (0x2E_BB80, vec![])
        };
        assert_eq!(f.calls, vec![(original::PAGE_ACCEPTS, vec![p.0]), tail]);
    }

    #[test]
    fn cursor_rows_skip_non_cursor_nodes() {
        let (f, p, rows) = menu();
        assert_eq!(p.cursor_row(&f, 0), Some(rows[0]));
        assert_eq!(p.cursor_row(&f, 2), Some(rows[2]));
        assert_eq!(p.cursor_row(&f, 3), None);
        assert_eq!(p.cursor_row(&f, u32::MAX), None);
    }

    #[test]
    fn focus_writes_the_cursor_record_and_ticks() {
        let (mut f, p, rows) = menu();
        f.mem.insert(global::SOUND_BANK_READY, 1);
        f.mem.insert(global::SOUND_TICK_PTR, 0x500);
        f.mem.insert(0x500, 0xD20);
        assert_eq!(
            focus(&mut f, p, 2),
            Outcome::Focused {
                row: 2,
                sound: true
            }
        );
        assert_eq!(f.mem[&(p.0 + page::CURSOR)], 2);
        assert_eq!(f.mem[&(p.0 + page::CURSOR_ROW)], rows[2]);
        assert_eq!(f.mem[&(p.0 + page::CURSOR_ID)], 12);
        assert_eq!(f.mem[&(p.0 + page::CURSOR_ID_ORDINAL)], 0);
        assert_eq!(f.calls, vec![(original::PLAY_SOUND, vec![0xD20])]);
        // Same row again: no tick.
        f.calls.clear();
        focus(&mut f, p, 2);
        assert!(f.calls.is_empty());
    }

    #[test]
    fn activate_prefers_the_row_handler() {
        let (mut f, p, _) = menu();
        focus(&mut f, p, 1);
        f.calls.clear();
        let out = activate(&mut f, p);
        assert_eq!(
            out,
            Outcome::Activated {
                row_handler: true,
                page_handler: false,
                kind: false
            }
        );
        assert_eq!(
            f.calls,
            vec![
                (original::PAGE_ACCEPTS, vec![p.0]),
                (0x2D_2000, vec![p.0]),
                (original::PLAY_SOUND, vec![sound::ACCEPT]),
            ]
        );
    }

    #[test]
    fn activate_presses_a_button_row_without_the_pointer_test() {
        // T1916 increment 3: kind 5 opens the text entry from the row record (`0x7FD20` without `0x711F0`).
        let (mut f, p, rows) = menu();
        f.mem.insert(rows[0] + row::KIND, button::KIND);
        f.mem.insert(rows[0] + button::TEXT, 0x55_0000);
        f.mem.insert(rows[0] + button::LIMIT, 0xABCD_00E0);
        f.mem.insert(rows[0] + button::ARG_A4, 7);
        f.mem.insert(rows[0] + button::ARG_A8, 8);
        f.mem.insert(rows[0] + button::ARG_94, 9);
        focus(&mut f, p, 0);
        f.calls.clear();
        let out = activate(&mut f, p);
        assert!(matches!(out, Outcome::Activated { kind: true, .. }));
        assert!(f.calls.contains(&(
            original::OPEN_TEXT_ENTRY,
            vec![p.0, 0x55_0000, 224, 7, 8, 9, original::TEXT_ENTRY_DONE, 0]
        )));
        assert!(
            !f.calls
                .iter()
                .any(|(va, _)| *va == original::ROW_KIND_ACTIVATE)
        );
    }

    #[test]
    fn activate_steps_a_slider_row_with_wrap() {
        // T3809: 0x800A0 has no kind 7 case; Enter, pad A and a click step the slider natively.
        let (mut f, p, rows) = menu();
        f.mem.insert(rows[0] + row::KIND, row::KIND_SLIDER);
        f.mem.insert(rows[0] + row::KIND_FLAGS, row::KIND_NUMERIC);
        f.mem.insert(rows[0] + row::OPTION_INDEX, 95);
        f.mem.insert(rows[0] + row::OPTION_PREVIOUS, 95);
        focus(&mut f, p, 0);
        f.calls.clear();
        let out = activate(&mut f, p);
        assert!(matches!(out, Outcome::Activated { kind: true, .. }));
        assert_eq!(f.mem[&(rows[0] + row::OPTION_INDEX)], 100);
        activate(&mut f, p);
        assert_eq!(f.mem[&(rows[0] + row::OPTION_INDEX)], 0, "wraps at the top");
        assert!(
            !f.calls
                .iter()
                .any(|(va, _)| *va == original::ROW_KIND_ACTIVATE)
        );
    }

    #[test]
    fn pressing_a_greyed_entry_runs_the_help_button_beside_it() {
        // T3811: the preview page builds an unavailable mode as [greyed name, "?" button 0x3126D0].
        let (mut f, p, rows) = menu();
        f.mem.insert(p.0 + page::ON_ACCEPT, 0);
        let help = 0x100_3800;
        f.mem.insert(rows[2] + row::GROUP_WIDTH, 2);
        f.mem.insert(rows[2] + row::NEXT_IN_GROUP, help);
        f.mem.insert(help + row::TYPE, 7);
        f.mem.insert(help + row::ON_ACTIVATE, 0x31_26D0);
        focus(&mut f, p, 2);
        f.calls.clear();
        let out = activate(&mut f, p);
        assert!(
            matches!(
                out,
                Outcome::Activated {
                    row_handler: true,
                    ..
                }
            ),
            "{out:?}"
        );
        assert!(
            f.calls.iter().any(|(va, _)| *va == 0x31_26D0),
            "{:?}",
            f.calls
        );
        assert_eq!(
            p.get(&f, page::CURSOR_ROW),
            help,
            "the cursor moves to the help column"
        );
        // Without the help member the greyed row stays denied, as in the original.
        let (mut f, p, _) = menu();
        f.mem.insert(p.0 + page::ON_ACCEPT, 0);
        focus(&mut f, p, 2);
        assert_eq!(activate(&mut f, p), Outcome::Denied);
    }

    #[test]
    fn activation_of_a_kind_nothing_handles_is_reported_once() {
        // T3809 detector: kind 6 has no 0x800A0 case and no native one; kind 2 (option list) does.
        let (mut f, p, rows) = menu();
        let builder = p.builder(&f);
        f.mem.insert(p.0 + page::ON_ACCEPT, 0);
        f.mem.insert(rows[0] + row::KIND, 6);
        focus(&mut f, p, 0);
        activate(&mut f, p);
        activate(&mut f, p);
        f.mem.insert(rows[0] + row::KIND, 2);
        activate(&mut f, p);
        let seen = unhandled_kinds();
        assert_eq!(seen.iter().filter(|k| **k == (builder, 6)).count(), 1);
        assert!(!seen.contains(&(builder, 2)));
    }

    #[test]
    fn activate_falls_back_to_the_page_handler() {
        let (mut f, p, _) = menu();
        focus(&mut f, p, 0);
        f.calls.clear();
        assert_eq!(
            activate(&mut f, p),
            Outcome::Activated {
                row_handler: false,
                page_handler: true,
                kind: false
            }
        );
        assert_eq!(f.calls[1], (0x2D_1000, vec![p.0]));
    }

    #[test]
    fn greyed_rows_are_denied_and_veto_stops_everything() {
        let (mut f, p, _) = menu();
        focus(&mut f, p, 2);
        assert_eq!(activate(&mut f, p), Outcome::Denied);
        f.returns.insert(original::PAGE_ACCEPTS, 0);
        f.calls.clear();
        assert_eq!(activate(&mut f, p), Outcome::Vetoed);
        assert_eq!(f.calls.len(), 1);
    }

    #[test]
    fn back_calls_the_page_back_handler_or_pops() {
        let (mut f, p, _) = menu();
        f.mem.insert(p.0 + page::ON_BACK, 0x2D_3000);
        f.mem.insert(p.0 + page::BACK_ID, 7);
        assert_eq!(
            back(&mut f, p),
            Outcome::BackHandled {
                handler: true,
                popped: false
            }
        );
        assert_eq!(f.mem[&(p.0 + page::CURSOR_ID)], 7);
        assert!(f.calls.contains(&(0x2D_3000, vec![p.0])));
        let (mut f, p, _) = menu();
        f.mem.insert(p.0 + page::FLAGS, page::FLAG_BACK_POPS);
        f.returns.insert(original::PARENT_CONTEXT, 0x900);
        f.mem.insert(0x92C, 0x777);
        assert_eq!(
            back(&mut f, p),
            Outcome::BackHandled {
                handler: false,
                popped: true
            }
        );
        // The native pop (page_stack) reopens the previous page through the original open 0x79860.
        assert!(f.calls.contains(&(
            crate::page_stack::original::OPEN_PAGE,
            vec![0x777, 0, u32::MAX]
        )));
    }

    /// T3817: pad X on a front-end page with the X "Player Progress" prompt (Story levels, Extras) runs the
    /// style's shortcut through the command queue, as 0x7A7B0 does; a page without the flag or callback ignores X.
    #[test]
    fn x_shortcut_runs_the_style_callback_only_on_flagged_pages() {
        let style = 0x52_3000;
        let (mut f, p, _) = menu();
        f.mem.insert(p.0 + page::STYLE, style);
        f.mem.insert(style + page::STYLE_SHORTCUT, 0x2D_5000);
        assert_eq!(
            shortcut(&mut f, p),
            Outcome::NoRow,
            "no flag: X does nothing"
        );
        assert!(f.calls.is_empty());
        f.mem.insert(p.0 + page::FLAGS, page::FLAG_X_SHORTCUT);
        f.mem.insert(p.0 + page::BUILDER, 0x2C_B090);
        let queue = CommandQueue::default();
        queue.lock().unwrap().push_back(NativeCommand::Shortcut);
        queue.lock().unwrap().push_back(NativeCommand::Accept);
        let mut out = Vec::new();
        service_on(&mut f, Some(p), &queue, &mut out);
        assert_eq!(
            out,
            [(
                NativeCommand::Shortcut,
                Outcome::Activated {
                    row_handler: false,
                    page_handler: true,
                    kind: false
                }
            )]
        );
        assert!(f.calls.contains(&(0x2D_5000, vec![])));
        assert!(
            queue.lock().unwrap().is_empty(),
            "the shortcut may replace the page"
        );
        f.mem.insert(style + page::STYLE_SHORTCUT, 0);
        assert_eq!(
            shortcut(&mut f, p),
            Outcome::NoRow,
            "no callback: X does nothing"
        );
    }

    #[test]
    fn close_uses_resume_contract_only_for_valid_back_pages_and_respects_veto() {
        let (mut f, p, _) = menu();
        let before = f.mem.clone();
        assert_eq!(resume_game(&mut f, p), Outcome::Denied);
        assert!(f.calls.is_empty());
        assert_eq!(f.mem, before);
        f.mem.insert(p.0 + page::BUILDER, 0x2F_5920);
        f.mem.insert(p.0 + page::VIEWPORT, 4);
        assert_eq!(resume_game(&mut f, p), Outcome::Denied);
        f.mem.insert(p.0 + page::VIEWPORT, 2);
        f.returns.insert(original::PAGE_ACCEPTS, 0);
        assert_eq!(resume_game(&mut f, p), Outcome::Vetoed);
        assert!(!f.calls.iter().any(|(va, _)| *va == 0x2E_BB80));
        f.calls.clear();
        f.returns.insert(original::PAGE_ACCEPTS, 1);
        assert_eq!(
            resume_game(&mut f, p),
            Outcome::BackHandled {
                handler: true,
                popped: false
            }
        );
        let tail = if crate::ingame_handlers::enabled_for(0x2EBB80) {
            (0x79B70, vec![0, 0])
        } else {
            (0x2E_BB80, vec![])
        };
        assert_eq!(f.calls, vec![(original::PAGE_ACCEPTS, vec![p.0]), tail]);
        assert_eq!(
            f.mem.get(&(p.0 + page::CURSOR_ID)),
            None,
            "Close never selects or rewrites a guest row"
        );
    }

    #[test]
    fn service_waits_for_ready_and_runs_one_activation() {
        let (mut f, p, _) = menu();
        let bindings = NativeMenuBindings {
            queue: CommandQueue::default(),
            live: Some(LiveSlot {
                builder: BUILDER,
                state: 3,
                cursor: 0,
                rows: 3,
            }),
        };
        let mut b = bindings.clone();
        b.action(ACTION_FOCUS, ActionArg::Index(2));
        b.action(ACTION_FOCUS, ActionArg::Index(1));
        b.action(ACTION_SELECT, ActionArg::Index(1));
        b.action(ACTION_BACK, ActionArg::None);
        assert_eq!(
            bindings.queue.lock().unwrap().len(),
            3,
            "focus moves collapse"
        );
        f.mem.insert(p.0 + page::STATE, 1);
        let mut out = Vec::new();
        service(&mut f, BUILDER, &bindings.queue, &mut out);
        assert!(
            out.is_empty() && bindings.pending(),
            "opening page: commands wait"
        );
        f.mem.insert(p.0 + page::STATE, 3);
        service(&mut f, BUILDER, &bindings.queue, &mut out);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].0, NativeCommand::Focus(1));
        assert!(matches!(
            out[1].1,
            Outcome::Activated {
                row_handler: true,
                ..
            }
        ));
        assert!(
            !bindings.pending(),
            "the back after a select is dropped (page replaced)"
        );
    }

    #[test]
    fn popup_identity_gives_a_builderless_option_list_its_parent_builder() {
        // T3799: the Theme row's option list (0x7FA00) is a builder 0 child; it answers to the page under it.
        let (mut f, p, _) = menu();
        let list = PageRef::slot(5);
        f.mem.insert(list.0, 0x52_4598);
        f.mem.insert(p.0 + page::ACTIVE_CHILD, list.0);
        assert_eq!(
            popup_identity(&f, list),
            BUILDER,
            "any descriptor-less child"
        );
        f.mem.insert(list.0 + page::ON_ACCEPT, OPTION_LIST_ACCEPT);
        assert!(is_option_list(&f, list));
        assert_eq!(popup_over(&f, &[BUILDER]), Some(list));
        assert_eq!(popup_identity(&f, list), BUILDER);
        f.mem.insert(p.0 + page::ACTIVE_CHILD, 0);
        assert_eq!(popup_identity(&f, list), 0, "no parent links the list");
        f.mem.insert(list.0 + page::BUILDER, 0x7_A420);
        assert_eq!(
            popup_identity(&f, list),
            0x7_A420,
            "a popup with a builder keeps it"
        );
    }

    #[test]
    fn option_list_accept_runs_the_list_handler_not_the_parent() {
        // The native Accept on the routed list calls the list's own accept (0x7BD40), which sets the parent row.
        let (mut f, _, rows) = menu();
        let list = PageRef::slot(5);
        f.mem.insert(list.0, 0x52_4598);
        f.mem.insert(list.0 + page::STATE, 3);
        f.mem.insert(list.0 + page::ON_ACCEPT, OPTION_LIST_ACCEPT);
        f.mem.insert(list.0 + page::CURSOR_ROW, rows[0]);
        let queue: CommandQueue = Default::default();
        queue.lock().unwrap().push_back(NativeCommand::Accept);
        let mut out = Vec::new();
        service_on(&mut f, Some(list), &queue, &mut out);
        assert_eq!(out.len(), 1);
        assert!(matches!(
            out[0].1,
            Outcome::Activated {
                page_handler: true,
                ..
            }
        ));
        assert!(
            f.calls
                .iter()
                .any(|(va, args)| *va == OPTION_LIST_ACCEPT && args == &[list.0])
        );
    }

    #[test]
    fn popup_over_finds_an_option_list_below_its_parent_in_slot_order() {
        // T3799 MEASURED: the Theme list took slot 1 while Map Settings is in slot 3.
        let (mut f, p, _) = menu();
        let list = PageRef::slot(1);
        f.mem.insert(list.0, 0x52_4598);
        f.mem.insert(p.0 + page::ACTIVE_CHILD, list.0);
        assert_eq!(popup_over(&f, &[BUILDER]), Some(list), "any linked child");
        f.mem.insert(list.0 + page::ON_ACCEPT, OPTION_LIST_ACCEPT);
        assert_eq!(popup_over(&f, &[BUILDER]), Some(list));
        assert_eq!(popup_identity(&f, list), BUILDER);
        f.mem.insert(p.0 + page::ACTIVE_CHILD, 0);
        assert_eq!(popup_over(&f, &[BUILDER]), None, "closed: no link");
    }

    #[test]
    fn input_stall_is_reported_once_at_the_threshold() {
        let mut stall = InputStall::default();
        for _ in 1..INPUT_STALL_FRAMES {
            assert_eq!(stall.wait(), None);
        }
        assert_eq!(stall.wait(), Some(INPUT_STALL_FRAMES));
        assert_eq!(stall.wait(), None, "reported once per stall");
    }

    #[test]
    fn top_page_follows_reused_slots_and_rejects_cycles() {
        let (mut f, parent, _) = menu();
        assert_eq!(top_page(&f), Some(parent));
        let child = PageRef::slot(1);
        let nested = PageRef::slot(0);
        for p in [child, nested] {
            f.mem.insert(p.0, 2);
        }
        f.mem.insert(parent.0 + page::ACTIVE_CHILD, child.0);
        assert_eq!(top_page(&f), Some(child));
        f.mem.insert(child.0 + page::ACTIVE_CHILD, nested.0);
        assert_eq!(top_page(&f), Some(nested));
        f.mem.insert(nested.0 + page::ACTIVE_CHILD, parent.0);
        assert_eq!(top_page(&f), None, "cyclic stack cannot own presentation");
        f.mem.insert(nested.0 + page::ACTIVE_CHILD, 0xdead_beef);
        assert_eq!(top_page(&f), Some(nested), "non-slot link is ignored");
        f.mem.insert(nested.0, 0);
        assert_eq!(top_page(&f), Some(child), "closed child is ignored");
        f.mem.insert(child.0, 0);
        assert_eq!(top_page(&f), Some(parent));
    }

    #[test]
    fn flagged_parent_navigation_survives_cursor_loss_between_frames() {
        let (mut f, parent, rows) = menu();
        let child = PageRef::slot(1);
        f.mem.insert(parent.0 + page::BUILDER, 0x32_78D0);
        f.mem
            .insert(parent.0 + page::FLAGS, page::FLAG_INPUT_WITH_CHILD);
        f.mem.insert(parent.0 + page::ACTIVE_CHILD, child.0);
        f.mem.insert(parent.0 + page::CURSOR, u32::MAX);
        f.mem.insert(child.0, 2);
        f.mem.insert(child.0 + page::PARENT, parent.0);
        f.mem.insert(child.0 + page::STATE, 3);
        f.returns.insert(original::PAGE_READY, 0);
        let queue: CommandQueue = Default::default();
        let mut out = Vec::new();
        queue
            .lock()
            .unwrap()
            .push_back(NativeCommand::Step(Step::Down));
        service_on(&mut f, Some(parent), &queue, &mut out);
        assert!(matches!(
            out.last(),
            Some((_, Outcome::Focused { row: 0, .. }))
        ));
        // Model the lost cursor observed between replayed button presses.
        f.mem.insert(parent.0 + page::CURSOR, u32::MAX);
        f.mem.insert(parent.0 + page::CURSOR_ROW, 0);
        queue
            .lock()
            .unwrap()
            .push_back(NativeCommand::Step(Step::Down));
        service_on(&mut f, Some(parent), &queue, &mut out);
        assert_eq!(parent.cursor_node(&f), rows[1], "second press advances");
        f.mem.insert(parent.0 + page::CURSOR, u32::MAX);
        f.mem.insert(parent.0 + page::CURSOR_ROW, 0);
        queue.lock().unwrap().push_back(NativeCommand::Accept);
        service_on(&mut f, Some(parent), &queue, &mut out);
        assert!(
            f.calls
                .iter()
                .any(|(va, args)| *va == 0x2D_2000 && args == &[parent.0])
        );
        // A closed page must not pass its focus on to a reused slot.
        service_on(&mut f, None, &queue, &mut out);
        f.mem.insert(parent.0 + page::CURSOR, u32::MAX);
        f.mem.insert(parent.0 + page::CURSOR_ROW, 0);
        queue
            .lock()
            .unwrap()
            .push_back(NativeCommand::Step(Step::Down));
        service_on(&mut f, Some(parent), &queue, &mut out);
        assert_eq!(
            parent.cursor_node(&f),
            rows[0],
            "reopened page starts fresh"
        );
    }

    #[test]
    fn flagged_parent_receives_input_beside_display_child_but_not_nested_modal() {
        let (mut f, parent, rows) = menu();
        let child = PageRef::slot(1);
        let modal = PageRef::slot(0);
        f.mem.insert(parent.0 + page::BUILDER, 0x32_78D0);
        f.mem
            .insert(parent.0 + page::FLAGS, page::FLAG_INPUT_WITH_CHILD);
        f.mem.insert(parent.0 + page::ACTIVE_CHILD, child.0);
        f.mem.insert(parent.0 + page::CURSOR, u32::MAX);
        f.mem.insert(child.0, 2);
        f.mem.insert(child.0 + page::PARENT, parent.0);
        f.mem.insert(child.0 + page::STATE, 3);
        f.returns.insert(original::PAGE_READY, 0);
        assert_eq!(top_page(&f), Some(child));
        assert_eq!(input_page(&f), Some(parent));
        assert_eq!(popup_over(&f, &[0x32_78D0]), None);
        let queue: CommandQueue = Default::default();
        queue
            .lock()
            .unwrap()
            .push_back(NativeCommand::Step(Step::Down));
        let mut out = Vec::new();
        service_on(&mut f, Some(parent), &queue, &mut out);
        assert!(matches!(out[0].1, Outcome::Focused { row: 0, .. }));
        assert_eq!(parent.cursor_node(&f), rows[0]);
        queue.lock().unwrap().push_back(NativeCommand::Select(1));
        service_on(&mut f, Some(parent), &queue, &mut out);
        assert!(
            f.calls
                .iter()
                .any(|(va, args)| *va == 0x2D_2000 && args == &[parent.0])
        );
        f.mem.insert(modal.0, 2);
        f.mem.insert(modal.0 + page::PARENT, child.0);
        f.mem.insert(child.0 + page::ACTIVE_CHILD, modal.0);
        assert_eq!(input_page(&f), Some(modal));
        queue.lock().unwrap().push_back(NativeCommand::Accept);
        service_on(&mut f, Some(parent), &queue, &mut out);
        assert!(
            !queue.lock().unwrap().is_empty(),
            "nested modal blocks parent"
        );
        f.mem.insert(child.0 + page::ACTIVE_CHILD, 0);
        f.mem.insert(modal.0, 0);
        f.mem.insert(parent.0 + page::FLAGS, 0);
        assert_eq!(input_page(&f), Some(child));
        assert_eq!(popup_over(&f, &[0x32_78D0]), Some(child));
        f.mem
            .insert(parent.0 + page::FLAGS, page::FLAG_INPUT_WITH_CHILD);
        f.mem.insert(child.0 + page::PARENT, 0);
        assert_eq!(
            input_page(&f),
            Some(child),
            "stale child link cannot grant input"
        );
    }

    #[test]
    fn builderless_confirmation_routes_commands_to_child_in_either_slot_order() {
        // T3853: the owner's exit confirmation has no builder and ordinary row callbacks, not OPTION_LIST_ACCEPT.
        for slot in [1, 5] {
            let (mut f, parent, rows) = menu();
            let popup = PageRef::slot(slot);
            f.mem.insert(parent.0 + page::BUILDER, 0x31_3010);
            f.mem.insert(parent.0 + page::ACTIVE_CHILD, popup.0);
            f.mem.insert(popup.0, 2);
            f.mem.insert(popup.0 + page::STATE, 3);
            f.mem.insert(popup.0 + page::CURSOR_ROW, rows[1]);
            let selected = popup_over(&f, &[0x31_3010]);
            assert_eq!(selected, Some(popup));
            assert_eq!(popup_identity(&f, popup), parent.builder(&f));
            assert!(
                !is_option_list(&f, popup),
                "confirmation keeps its own draw and pointer policy"
            );
            let queue: CommandQueue = Default::default();
            queue.lock().unwrap().push_back(NativeCommand::Accept);
            let mut out = Vec::new();
            service_on(&mut f, selected, &queue, &mut out);
            assert!(queue.lock().unwrap().is_empty());
            assert!(matches!(
                out.as_slice(),
                [(
                    NativeCommand::Accept,
                    Outcome::Activated {
                        row_handler: true,
                        ..
                    }
                )]
            ));
            assert!(f.calls.contains(&(original::PAGE_READY, vec![popup.0])));
            assert!(!f.calls.contains(&(original::PAGE_READY, vec![parent.0])));
            assert!(f.calls.contains(&(0x2D_2000, vec![popup.0])));
            f.mem
                .insert(popup.0 + page::ID, crate::native_text::TEXT_WIDGET_IDS[0]);
            assert_eq!(
                popup_over(&f, &[0x31_3010]),
                None,
                "text child stays with native_text"
            );
            f.mem.insert(popup.0 + page::ID, 0);
            f.mem.insert(parent.0 + page::ACTIVE_CHILD, 0);
            assert_eq!(
                popup_over(&f, &[0x31_3010]),
                None,
                "closed or unrelated child"
            );
        }
    }

    #[test]
    fn popup_over_finds_the_active_child_on_top() {
        let (mut f, p, _) = menu();
        let popup = PageRef::slot(5);
        f.mem.insert(popup.0, 0x52_4598);
        f.mem.insert(popup.0 + page::BUILDER, 0x7_A420);
        assert_eq!(popup_over(&f, &[BUILDER]), None, "not linked");
        f.mem.insert(p.0 + page::ACTIVE_CHILD, popup.0);
        assert_eq!(popup_over(&f, &[BUILDER]), Some(popup));
        assert_eq!(popup_over(&f, &[0x1234]), None, "parent not native");
        f.mem
            .insert(popup.0 + page::ID, crate::native_text::TEXT_WIDGET_IDS[0]);
        assert_eq!(
            popup_over(&f, &[BUILDER]),
            None,
            "text fields are native_text's"
        );
    }
}
