// SPDX-License-Identifier: GPL-3.0-or-later
//! T2189 UI-6: the Rust `main` owns the `--ui=rust|legacy` switch (`tsfp_ui_model::switch`).
//! `main` parses it (with `TSFP_UI`), removes its arguments so the C host parser never sees them,
//! and stores it once. The host asks per menu page through `tsfp_ui_backend_for`. Default is
//! legacy, so a build without the flag behaves exactly like before. Every ask is recorded in a
//! bounded route log (`route_lines`). The C host reports live page slots at the frame boundary;
//! a checked runtime layout is still required before a page can change renderer.
use std::ffi::c_int;
use std::sync::{Mutex, OnceLock};

use tsfp_ui_model::model::PageId;
use tsfp_ui_model::switch::{RouteLog, SwitchError, UiBackend, UiSwitch};

static SWITCH: OnceLock<UiSwitch> = OnceLock::new();
/// Enough for a long menu session; later asks only count as dropped.
const ROUTE_LOG_CAPACITY: usize = 4096;
static ROUTES: Mutex<Option<RouteLog>> = Mutex::new(None);
static PAGES: Mutex<[Option<PageSnapshot>; 20]> = Mutex::new([None; 20]);

/// A live page slot copied from the original widget table, without guest pointers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageSnapshot {
    pub builder: u32,
    pub state: u32,
    pub cursor: u32,
    pub rows: u32,
}

/// Parses the switch out of `arguments` (program name already removed) and returns the rest.
pub fn take(
    env: Option<&str>,
    arguments: Vec<String>,
) -> Result<(UiSwitch, Vec<String>), SwitchError> {
    let (switch, rest) = UiSwitch::parse(env, arguments.iter().map(String::as_str))?;
    let rest = rest.into_iter().map(str::to_owned).collect();
    Ok((switch, rest))
}

/// Stores the process switch; the first call wins.
pub fn install(switch: UiSwitch) {
    let _ = SWITCH.set(switch);
}

/// One line for `--rust-main-info`.
pub fn describe(switch: &UiSwitch) -> String {
    let backend = match switch.backend {
        UiBackend::Legacy => "legacy",
        UiBackend::Rust => "rust",
    };
    format!(
        "ui: {backend} ({} legacy page override(s), renderer routing not wired)",
        switch.legacy_pages.len()
    )
}

/// C face: 1 when menu `page` runs on the Rust UI, 0 for legacy. `layout_complete` is non-zero
/// when the checked layout of that page succeeded. Legacy when the switch was never installed.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_ui_backend_for(page: u64, layout_complete: c_int) -> c_int {
    let switch = SWITCH.get_or_init(UiSwitch::default);
    let route = switch.route(PageId(page), layout_complete != 0);
    if let Ok(mut log) = ROUTES.lock() {
        log.get_or_insert_with(|| RouteLog::new(ROUTE_LOG_CAPACITY))
            .record(PageId(page), route);
    }
    c_int::from(route.backend() == UiBackend::Rust)
}

/// C host frame hook. The builder address is the stable page identity; slot numbers are reused.
/// Until a runtime model has produced a checked layout, this records the route with an incomplete
/// layout and leaves the original drawing and input path in charge.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_ui_page_snapshot(
    slot: u32,
    builder: u32,
    state: u32,
    cursor: u32,
    rows: u32,
) {
    let Some(mut pages) = PAGES.lock().ok() else {
        return;
    };
    let Some(previous) = pages.get_mut(slot as usize) else {
        return;
    };
    if builder == 0 {
        let was_live = previous.is_some();
        *previous = None;
        if was_live && SWITCH.get().is_some_and(|s| s.backend == UiBackend::Rust) {
            eprintln!("ui-page slot={slot} closed");
        }
        return;
    }
    let snapshot = PageSnapshot {
        builder,
        state,
        cursor,
        rows,
    };
    let changed = previous.is_none_or(|old| old != snapshot);
    let changed_page = previous.is_none_or(|old| old.builder != builder);
    *previous = Some(snapshot);
    drop(pages);
    if changed && let Some(switch) = SWITCH.get().filter(|s| s.backend == UiBackend::Rust) {
        let route = switch.route(PageId(u64::from(builder)), false);
        if changed_page {
            let _ = tsfp_ui_backend_for(u64::from(builder), 0);
        }
        eprintln!(
            "ui-page slot={slot} builder=0x{builder:x} route={} state={state} cursor={cursor} rows={rows}",
            route.name()
        );
    }
}

#[allow(dead_code)]
pub fn page_snapshots() -> [Option<PageSnapshot>; 20] {
    PAGES.lock().map(|pages| *pages).unwrap_or([None; 20])
}

/// The route log so far, one `0xPAGE reason` line per page change.
#[allow(dead_code)] // read by the host route trace once it is wired
pub fn route_lines() -> Vec<String> {
    ROUTES
        .lock()
        .ok()
        .and_then(|log| log.as_ref().map(RouteLog::lines))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn switch_arguments_never_reach_the_host() {
        let (switch, rest) = take(
            None,
            owned(&[
                "game.xbe",
                "--ui=rust",
                "--rust-ports=all",
                "--ui-legacy-page=0x20",
            ]),
        )
        .unwrap();
        assert_eq!(rest, owned(&["game.xbe", "--rust-ports=all"]));
        assert_eq!(switch.backend, UiBackend::Rust);
        assert_eq!(switch.backend_for(PageId(0x20), true), UiBackend::Legacy);
        assert_eq!(switch.backend_for(PageId(0x21), true), UiBackend::Rust);
    }

    #[test]
    fn bad_value_is_an_error_and_default_is_legacy() {
        assert!(take(None, owned(&["--ui=fancy"])).is_err());
        let (switch, rest) = take(None, owned(&["game.xbe"])).unwrap();
        assert_eq!(rest, owned(&["game.xbe"]));
        assert_eq!(switch, UiSwitch::default());
        assert_eq!(
            describe(&switch),
            "ui: legacy (0 legacy page override(s), renderer routing not wired)"
        );
    }

    #[test]
    fn c_face_installs_once() {
        let (switch, _) = take(Some("rust"), Vec::new()).unwrap();
        install(switch);
        assert_eq!(tsfp_ui_backend_for(0x30, 1), 1);
        assert_eq!(tsfp_ui_backend_for(0x30, 0), 0);
        install(UiSwitch::default());
        assert_eq!(tsfp_ui_backend_for(0x30, 1), 1, "first install wins");
        assert_eq!(
            route_lines(),
            ["0x30 rust", "0x30 layout-incomplete", "0x30 rust"]
        );
    }

    #[test]
    fn live_slot_snapshot_tracks_reuse_and_clear() {
        tsfp_ui_page_snapshot(19, 0x524598, 3, 2, 8);
        assert_eq!(
            page_snapshots()[19],
            Some(PageSnapshot {
                builder: 0x524598,
                state: 3,
                cursor: 2,
                rows: 8,
            })
        );
        tsfp_ui_page_snapshot(19, 0x524598, 3, 3, 8);
        assert_eq!(page_snapshots()[19].unwrap().cursor, 3);
        tsfp_ui_page_snapshot(19, 0, 0, 0, 0);
        assert_eq!(page_snapshots()[19], None);
        tsfp_ui_page_snapshot(20, 0x524598, 3, 0, 8);
    }
}
