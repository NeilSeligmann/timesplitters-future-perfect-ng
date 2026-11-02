// SPDX-License-Identifier: GPL-3.0-or-later
//! T1916: the Map Maker UI on the native framework (docs/t-system-mapmaker-ui.md, "Native Map Maker UI").
//!
//! * The editor's menu, settings, load and save pages are registered in `ui_pages::PAGES` (drawn by Rust in the engine
//!   frame from the live page record where every row draw is ported, native input through the page queue).
//! * The editor's on-screen pointer follows the real mouse: the shell maps the OS cursor to the 640x480 design grid
//!   ([`mouse_tick`], PreUpdate) and the guest-thread frame service puts it into the guest's pointer floats
//!   ([`service`], `tsfp_ui_input::editor_pointer`) when the mouse moved, so the original hover and select logic see it.
//! * F9 (`ui_live::user_hidden`) gives the pointer back to the stick.
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use bevy::input::gamepad::GamepadAxis;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use tsfp_input::recording;
use tsfp_ui_input::editor_pointer::{self, MouseFeed};
use tsfp_ui_input::mapmaker_pointer::{EditorPointer, EditorPosition};
use tsfp_ui_input::native_page::GuestUi;

/// Latest OS mouse position in the design grid (`None`: outside the window or unknown).
static MOUSE: Mutex<Option<(f32, f32)>> = Mutex::new(None);
static FEED: Mutex<MouseFeed> = Mutex::new(MouseFeed::new());
/// T3392: held mouse buttons (`tsfp_input::recording::BUTTON_*`), sampled with the position.
static MOUSE_BUTTONS: AtomicU8 = AtomicU8::new(0);
static LOGGED: AtomicBool = AtomicBool::new(false);
static POINTER: Mutex<EditorPointer> = Mutex::new(EditorPointer::new());
static EDITOR: AtomicBool = AtomicBool::new(false);

/// Candidate page cursor, opt-in until the original icon boundary is checked live.
pub fn native_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("TSFP_UI_NATIVE_EDITOR_CURSOR").is_ok_and(|v| v == "1"))
}

pub fn native_page() -> bool {
    native_enabled()
        && EDITOR.load(Ordering::Acquire)
        && !crate::ui_live::user_hidden()
        && crate::ui_live::owns_pointer()
        && !crate::ui_live::option_list_open() // T3799: the original list follows the editor pointer
        && crate::ui_live::native_page_builder()
            .is_some_and(|builder| crate::ui_pages::MAPMAKER_PAGES.contains(&builder))
}

pub fn system_cursor() -> bool {
    native_page() && POINTER.lock().is_ok_and(|p| p.system_cursor())
}

/// Caller-specific render boundary. Never suppress unrelated icons or hint text.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_ui_editor_cursor_icon_owned(return_pc: u32) -> i32 {
    let owned = system_cursor() && matches!(return_pc, 0x30_D033 | 0x30_D079);
    static LOGGED_ICON: AtomicBool = AtomicBool::new(false);
    if owned && !LOGGED_ICON.swap(true, Ordering::AcqRel) {
        eprintln!("tsfp-bevy: mapmaker native OS cursor suppresses editor icon at {return_pc:#x}");
    }
    i32::from(owned)
}

/// PreUpdate: sample the OS cursor (design grid, letterbox aware).
pub fn mouse_tick(
    window: Query<&Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    mut wheel: EventReader<bevy::input::mouse::MouseWheel>,
) {
    let wheel_activity = wheel.read().any(|event| event.x != 0.0 || event.y != 0.0);
    static GEOMETRY: AtomicBool = AtomicBool::new(false);
    if let Ok(w) = window.single()
        && tsfp_input_intake::cursor_position(w).is_some()
        && std::env::var_os("TSFP_UI_DRAW_LOG").is_some()
        && !GEOMETRY.swap(true, Ordering::AcqRel)
    {
        eprintln!(
            "tsfp-bevy: mapmaker-ui: window {}x{} cursor {:?} picture {:?}",
            w.width(),
            w.height(),
            tsfp_input_intake::cursor_position(w),
            crate::app::picture_size()
        );
    }
    let pos = window.single().ok().and_then(|w| {
        crate::ui_live::cursor_to_design(
            tsfp_input_intake::cursor_position(w)?,
            Vec2::new(w.width(), w.height()),
            crate::app::picture_size(),
        )
    });
    // T3392: a replay owns the pointer, the recorded rows (or nothing, for an old recording) stand in for the device.
    if !crate::input_recording::replaying() {
        if let Ok(mut m) = MOUSE.lock() {
            *m = pos;
        }
        let held = |button, bit| if mouse.pressed(button) { bit } else { 0 };
        MOUSE_BUTTONS.store(
            held(MouseButton::Left, recording::BUTTON_LEFT)
                | held(MouseButton::Right, recording::BUTTON_RIGHT)
                | held(MouseButton::Middle, recording::BUTTON_MIDDLE),
            Ordering::Release,
        );
    }
    if let Ok(mut p) = POINTER.lock() {
        // Pad wins simultaneous activity; held sticks count as continuing activity.
        p.mouse(
            pos.and_then(|(x, y)| EditorPosition::new(x, y)),
            mouse.just_pressed(MouseButton::Left)
                || mouse.just_pressed(MouseButton::Right)
                || wheel_activity,
        );
        if pads.iter().any(|pad| {
            pad.get(GamepadAxis::LeftStickX).unwrap_or(0.0).abs() > 0.25
                || pad.get(GamepadAxis::LeftStickY).unwrap_or(0.0).abs() > 0.25
                || pad.get(GamepadAxis::RightStickX).unwrap_or(0.0).abs() > 0.25
                || pad.get(GamepadAxis::RightStickY).unwrap_or(0.0).abs() > 0.25
                || pad.get_just_pressed().next().is_some()
                || pad.get_just_released().next().is_some()
        }) {
            p.pad_activity();
        }
    }
}

/// T3392: guest thread, frame boundary, before anything can return early. While recording, the mouse
/// sample of this boundary becomes a `P` row when it changed. While replaying a recording that has `P`
/// rows, the due row replaces the mouse sample, so [`service`] feeds the recorded pointer through the
/// same `MouseFeed` and `editor_pointer::write` path as the live mouse (the same hover and select logic
/// of the guest sees it at the same host poll). An old recording has no rows: the mouse stays unknown
/// (`None`) and the legacy fallback of [`legacy_pointer_fallback`] applies.
pub fn pointer_io() {
    if crate::input_recording::replaying() {
        // T3453: the recorded timeline (replay clock), not the live window poll count.
        if let Some(sample) =
            crate::input_recording::replay_pointer(crate::input_recording::replay_poll())
            && let Ok(mut m) = MOUSE.lock()
        {
            *m = sample;
        }
    } else if crate::input_recording::enabled() {
        let position = MOUSE.lock().ok().and_then(|m| *m);
        if let Err(error) =
            crate::input_recording::record_pointer(position, MOUSE_BUTTONS.load(Ordering::Acquire))
        {
            eprintln!("tsfp-bevy: input recording pointer row failed: {error}");
        }
    }
}

/// Guest thread, frame boundary: the mouse into the Map Maker pointer.
pub fn service(g: &mut dyn GuestUi) {
    EDITOR.store(
        g.read32(editor_pointer::addr::UI_MODE) == Some(editor_pointer::addr::MODE_MAPMAKER),
        Ordering::Release,
    );
    pointer_log(g);
    if std::env::var_os("TSFP_POINTER_LOG").is_some() {
        select_mode_log(g);
        top_page_log(g);
    }
    legacy_select_mode_cursor(g);
    // Registered pages already use native OS-position hit-tests and the page command
    // queue. No pointer state is projected for this candidate's mouse page path.
    if native_page() {
        return;
    }
    if crate::ui_live::user_hidden() || !editor_pointer::pointer_active(g) {
        return;
    }
    let mouse = MOUSE.lock().ok().and_then(|m| *m);
    let Some((x, y)) = FEED.lock().ok().and_then(|mut f| f.next(mouse)) else {
        return;
    };
    let wrote = editor_pointer::write(g, x, y);
    if wrote && !LOGGED.swap(true, Ordering::AcqRel) {
        eprintln!("tsfp-bevy: mapmaker-ui: editor pointer follows the mouse ({x:.1}, {y:.1})");
    } else if std::env::var_os("TSFP_UI_DRAW_LOG").is_some() {
        eprintln!(
            "tsfp-bevy: mapmaker-ui: mouse ({x:.1}, {y:.1}) -> pointer {:?} (wrote {wrote})",
            editor_pointer::read(g)
        );
    }
}

/// T3392 diagnostic (`TSFP_POINTER_LOG=1`): the guest's editor pointer whenever it changes, with the poll,
/// and a heartbeat every 300 frames with the mode, viewport and pointer mode words.
fn pointer_log(g: &dyn GuestUi) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    static LAST: Mutex<Option<(u32, u32, bool)>> = Mutex::new(None);
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if !*ENABLED.get_or_init(|| std::env::var_os("TSFP_POINTER_LOG").is_some()) {
        return;
    }
    let active = editor_pointer::pointer_active(g);
    if CALLS.fetch_add(1, Ordering::Relaxed) % 300 == 0 {
        let context = editor_pointer::current_context(g);
        eprintln!(
            "tsfp-bevy: pointer-log: heartbeat poll {} mode {:?} viewport {:?} context {context:x?} pointer mode {:?} pointer {:?}",
            crate::wi::host_poll_index(),
            g.read32(editor_pointer::addr::UI_MODE),
            g.read32(editor_pointer::addr::CURRENT_VIEWPORT),
            context.and_then(|c| g.read32(c + editor_pointer::addr::context::POINTER_MODE)),
            editor_pointer::read(g),
        );
    }
    let Some((x, y)) = editor_pointer::read(g) else {
        return;
    };
    let key = (x.to_bits(), y.to_bits(), active);
    if let Ok(mut last) = LAST.lock()
        && *last != Some(key)
    {
        *last = Some(key);
        eprintln!(
            "tsfp-bevy: pointer-log: poll {} guest pointer ({x:.1}, {y:.1}) active {active}",
            crate::wi::host_poll_index()
        );
    }
}

/// T3392 diagnostic: the rows of the top live page whose page-level `ON_ACCEPT` is the Select Mode callback
/// `0x312B60` (once per change of cursor or row count), to measure the popup's row ids, flags and geometry.
fn select_mode_log(g: &dyn GuestUi) {
    use tsfp_ui_input::menu_state::layout::{page, row};
    use tsfp_ui_input::native_page::{PAGE_SLOTS, PageRef};
    static LAST: Mutex<Option<(u32, u32, u32)>> = Mutex::new(None);
    let Some(top) = (0..PAGE_SLOTS).rev().map(PageRef::slot).find(|p| p.live(g)) else {
        return;
    };
    let word = |va: u32| g.read32(va).unwrap_or(0);
    if word(top.0 + page::ON_ACCEPT) != 0x31_2B60 {
        return;
    }
    let key = (top.0, top.cursor(g), top.row_count(g));
    let Ok(mut last) = LAST.lock() else { return };
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    eprintln!(
        "tsfp-bevy: pointer-log: select-mode poll {} page {:#x} builder {:#x} state {} cursor {} rows {} flags {:#x} view {:?}..{:?}",
        crate::wi::host_poll_index(),
        top.0,
        top.builder(g),
        top.state(g),
        top.cursor(g) as i32,
        top.row_count(g),
        word(top.0 + page::FLAGS),
        f32::from_bits(word(top.0 + page::VIEW_TOP)),
        f32::from_bits(word(top.0 + page::VIEW_BOTTOM)),
    );
    let viewport = word(top.0 + page::VIEWPORT);
    if let Some(context) = editor_pointer::context(viewport) {
        let floats: Vec<String> = (0x10..0x58u32)
            .step_by(4)
            .map(|offset| format!("{offset:x}={:?}", f32::from_bits(word(context + offset))))
            .collect();
        eprintln!(
            "tsfp-bevy: pointer-log:   viewport {viewport} context {context:#x} words {}",
            floats.join(" ")
        );
    }
    let mut node = word(top.0 + page::FIRST_NODE);
    let mut guard = 0;
    while node != 0 && guard < 16 {
        let floats: Vec<String> = (0x58..0x90u32)
            .step_by(4)
            .map(|offset| format!("{offset:x}={:?}", f32::from_bits(word(node + offset))))
            .collect();
        eprintln!(
            "tsfp-bevy: pointer-log:     node {node:#x} {}",
            floats.join(" ")
        );
        eprintln!(
            "tsfp-bevy: pointer-log:   node {node:#x} type {} id {} flags {:#x} activate {:#x} y {:?} height {:?} nextcursor {:#x}",
            word(node + row::TYPE),
            word(node + row::ID) as i32,
            word(node + row::FLAGS),
            word(node + row::ON_ACTIVATE),
            f32::from_bits(word(node + row::Y)),
            f32::from_bits(word(node + row::HEIGHT)),
            word(node + row::NEXT_CURSOR),
        );
        node = word(node + row::NEXT);
        guard += 1;
    }
}

/// T3392 diagnostic: the top live page (slot, builder, page-level accept, cursor) whenever it changes.
fn top_page_log(g: &dyn GuestUi) {
    use tsfp_ui_input::menu_state::layout::{page, row};
    use tsfp_ui_input::native_page::{PAGE_SLOTS, PageRef};
    static LAST: Mutex<Option<(u32, u32, u32, u32, u32, u32, u32, u32, u32)>> = Mutex::new(None);
    let top = (0..PAGE_SLOTS).rev().find(|n| PageRef::slot(*n).live(g));
    let key = top.map_or((u32::MAX, 0, 0, 0, 0, 0, 0, 0, 0), |n| {
        let p = PageRef::slot(n);
        let node = p.get(g, page::CURSOR_ROW);
        let (row_id, row_accept, row_kind) = if node == 0 {
            (0, 0, 0)
        } else {
            (
                g.read32(node + row::ID).unwrap_or(0),
                g.read32(node + row::ON_ACTIVATE).unwrap_or(0),
                g.read32(node + row::KIND).unwrap_or(0),
            )
        };
        (
            n,
            p.builder(g),
            g.read32(p.0 + page::ON_ACCEPT).unwrap_or(0),
            p.cursor(g),
            p.get(g, page::ROW_COUNT),
            node,
            row_id,
            row_accept,
            row_kind,
        )
    });
    if let Ok(mut last) = LAST.lock()
        && *last != Some(key)
    {
        *last = Some(key);
        eprintln!(
            "tsfp-bevy: pointer-log: top page poll {} slot {} builder {:#x} accept {:#x} cursor {} rows {} row {:#x} row-id {:#x} row-accept {:#x} row-kind {:#x}",
            crate::wi::host_poll_index(),
            key.0 as i32,
            key.1,
            key.2,
            key.3 as i32,
            key.4,
            key.5,
            key.6,
            key.7,
            key.8
        );
    }
    // T3395 diagnostic (TSFP_POINTER_LOG): correlate recorded A/Start edges with the live page
    // contract. This observes the guest page only; it does not synthesize input or mutate state.
    // Compare against the recorded timeline. In tick-clock replays host polling can move at a
    // different rate from route rows, so host_poll_index can miss late recorded edges.
    let poll = crate::input_recording::replay_poll();
    let host_poll = crate::wi::host_poll_index();
    // T3395 probe diagnostic: sample the custom preview page across synthetic button sweeps.
    // The per-edge table above only covers buttons present in the owner recording, so it cannot
    // show whether appended probe rows changed the page. This is read-only and rate-limited.
    if std::env::var_os("TSFP_POINTER_LOG").is_some()
        && poll.is_multiple_of(30)
        && let Some(slot) = top
    {
        let p = PageRef::slot(slot);
        if p.builder(g) == 0x002c6680 {
            eprintln!(
                "tsfp-bevy: pointer-log: arcade preview sample poll {poll} host-poll {host_poll} state {} cursor {} rows {}",
                p.state(g),
                p.cursor(g) as i32,
                p.row_count(g),
            );
        }
    }
    if crate::input_recording::replay_accept_edge_near(poll, 0, 0) {
        static LAST_ACCEPT: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(u64::MAX);
        if LAST_ACCEPT.swap(poll, Ordering::AcqRel) != poll {
            if let Some(slot) = top {
                let p = PageRef::slot(slot);
                let cursor_row = p.get(g, page::CURSOR_ROW);
                eprintln!(
                    "tsfp-bevy: pointer-log: recorded A edge poll {poll} host-poll {host_poll} top slot {slot} builder {:#x} state {} cursor {} rows {} row-id {:#x} row-handler {:#x} page-handler {:#x}",
                    p.builder(g),
                    p.state(g),
                    p.cursor(g) as i32,
                    p.row_count(g),
                    if cursor_row == 0 {
                        0
                    } else {
                        g.read32(cursor_row + row::ID).unwrap_or(0)
                    },
                    if cursor_row == 0 {
                        0
                    } else {
                        g.read32(cursor_row + row::ON_ACTIVATE).unwrap_or(0)
                    },
                    g.read32(p.0 + page::ON_ACCEPT).unwrap_or(0),
                );
            } else {
                eprintln!(
                    "tsfp-bevy: pointer-log: recorded A edge poll {poll} host-poll {host_poll} with no live page"
                );
            }
        }
    }
    if crate::input_recording::replay_start_edge_at(poll) {
        static LAST_START: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(u64::MAX);
        if LAST_START.swap(poll, Ordering::AcqRel) != poll {
            if let Some(slot) = top {
                let p = PageRef::slot(slot);
                let cursor_row = p.get(g, page::CURSOR_ROW);
                eprintln!(
                    "tsfp-bevy: pointer-log: recorded START edge poll {poll} host-poll {host_poll} top slot {slot} builder {:#x} state {} cursor {} rows {} row-id {:#x} row-handler {:#x} page-handler {:#x}",
                    p.builder(g),
                    p.state(g),
                    p.cursor(g) as i32,
                    p.row_count(g),
                    if cursor_row == 0 {
                        0
                    } else {
                        g.read32(cursor_row + row::ID).unwrap_or(0)
                    },
                    if cursor_row == 0 {
                        0
                    } else {
                        g.read32(cursor_row + row::ON_ACTIVATE).unwrap_or(0)
                    },
                    g.read32(p.0 + page::ON_ACCEPT).unwrap_or(0),
                );
            } else {
                eprintln!(
                    "tsfp-bevy: pointer-log: recorded START edge poll {poll} host-poll {host_poll} with no live page"
                );
            }
        }
    }
}

/// Select Mode callback (page-level `ON_ACCEPT` of the popup, `0x312BC0` builds it).
const SELECT_MODE_ACCEPT: u32 = 0x31_2B60;

/// T3392 TEMPORARY-SHIM (removal: re-record the owner routes with format v5 pointer rows, then delete this
/// function; task T3392 follow-up). An old recording (format v1 to v4, or a v5 file without a mouse) holds the
/// mouse click on the Select Mode popup only as a pad A edge in the `T` rows, the pointer position it was made
/// at is not in the file. The popup is pointer driven: the guest derives its cursor row from the pointer every
/// frame (MEASURED: a cursor row written with `native_page::focus` was back at -1 the next frame), so at the
/// recorded A edge, if the popup has no cursor row (cursor -1, the guest's own accept would select nothing), put
/// the pointer on the centre of the Map Maker default row (Advanced, the owner decision of T2402) with the same
/// `editor_pointer::write` the live mouse uses. The guest's own hover and select logic then run on the recorded
/// activation. Nothing else is faked: no cursor, mode word or callback is written or called here.
fn legacy_select_mode_cursor(g: &mut dyn GuestUi) {
    use tsfp_ui_input::menu_state::layout::{page, row};
    use tsfp_ui_input::native_page::{PAGE_SLOTS, PageRef};
    if !crate::input_recording::semantic_replay()
        || crate::input_recording::replay_has_pointer_rows()
    {
        return;
    }
    // T3453: the recorded A edges are recorded host polls, so they are matched on the replay clock.
    // The live window poll count (`wi::host_poll_index`) counts every presenter pump, its rate per
    // tick follows the wall clock (T3393), and it matched the boot-time A edge at recorded poll 1028
    // to whatever tick the live count passed 1025 at (MEASURED mapmaker-editor-20261018: the Select
    // Mode row was activated 108 to 127 ticks before the recorded click, at a load-dependent tick).
    let poll = crate::input_recording::replay_poll();
    if !crate::input_recording::replay_accept_edge_near(poll, 3, 1) {
        return;
    }
    let Some(top) = (0..PAGE_SLOTS).rev().map(PageRef::slot).find(|p| p.live(g)) else {
        return;
    };
    if g.read32(top.0 + page::ON_ACCEPT) != Some(SELECT_MODE_ACCEPT) {
        return;
    }
    if top.cursor(g) as i32 != -1 {
        // MEASURED: the pad A edge alone does not accept in a pointer context popup (the popup stayed open with the
        // cursor on the hovered row), the original click is a pointer press. Map the recorded activation to the
        // framework's activation of the hovered row, once per edge, as the live mouse click does.
        static LAST_ACTIVATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let last = LAST_ACTIVATION.load(Ordering::Acquire);
        if last == 0 || poll >= last + 30 {
            LAST_ACTIVATION.store(poll, Ordering::Release);
            let outcome = tsfp_ui_input::native_page::activate(g, top);
            // T3799 (I-T3799-2): this activation is the recorded A edge (at most 3 polls ahead); the guest must not
            // get it again on the page that opens next (it closed the Map Maker editor menu in replays).
            crate::input_recording::consume_accept_edge_until(poll + 3);
            eprintln!(
                "tsfp-bevy: legacy replay shim (T3392, TEMPORARY): poll {poll} recorded A edge activates the hovered Select Mode row: {outcome:?}"
            );
        }
        return;
    }
    let want = tsfp_ui_input::mapmaker_handlers::DEFAULT_MODE_CHOICE_ROW;
    let mut node = g.read32(top.0 + page::FIRST_NODE).unwrap_or(0);
    let mut guard = 0;
    while node != 0 && guard < 16 {
        if g.read32(node + row::ID) == Some(want)
            && let Some([x0, y0, x1, y1]) = tsfp_ui_input::live_page::row_geometry(g, node)
        {
            let wrote = editor_pointer::write(g, (x0 + x1) * 0.5, (y0 + y1) * 0.5);
            eprintln!(
                "tsfp-bevy: legacy replay shim (T3392, TEMPORARY): poll {poll} Select Mode popup {:#x} had no cursor row, pointer to row {want} centre ({:.1}, {:.1}) wrote {wrote}",
                top.0,
                (x0 + x1) * 0.5,
                (y0 + y1) * 0.5
            );
            return;
        }
        node = g.read32(node + row::NEXT).unwrap_or(0);
        guard += 1;
    }
}
