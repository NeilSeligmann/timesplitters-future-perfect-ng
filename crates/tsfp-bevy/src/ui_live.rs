// SPDX-License-Identifier: GPL-3.0-or-later
//! T2188: the interactive `--ui=rust` main menu. While the original main menu page (builder 0x2D0BE0) is the live top page slot,
//! the Rust page owns input: keyboard (arrows, Enter, Escape/Backspace), mouse (hover focuses, left click selects, right
//! click backs) and pad (D-pad or left stick, A, B, START) go through `tsfp_ui_input::UiInput` on the page's `ModelLayout`, the
//! `LiveMenu` moves the Rust focus and queues native page actions (T2310, below; the T2188 closed-loop pad taps are gone,
//! their driver is the measurement tool `tsfp_ui_input::pad_route` since T2316). The physical pad's own events are held back
//! while the Rust page owns input. Any other page (Arcade, Story, ...)
//! is legacy: the overlay hides and input passes straight through (per-page fallback). F9 hides the overlay and hands input
//! back to the original menu. The overlay is redrawn whenever the focused row changes (checked every frame).
//! T2310 UI-12: on the main menu the pad-tap shim is gone: focus, select and back are native page actions
//! (`tsfp_ui_input::native_page`) queued here and run by [`tsfp_ui_frame_service`] on the guest thread at the frame
//! boundary, which writes the focus to the page record and calls the original page/row handlers (UiBindings).
//! T2310 increment 3: the Arcade type, Settings and Controls pages take native input too ([`NATIVE_INPUT_PAGES`]): their
//! keys and pad buttons become native list steps, option-row changes, activation and back (`tsfp_ui_input::widgets`)
//! run at the frame boundary; the original still draws them (no Rust overlay yet, T2311).
//! T2310 increment 5: popups (confirm, message, option list) over those pages and over the main menu, and the Controls
//! "accept changes?" page, are native too; Back pops the page stack natively (`tsfp_ui_input::page_stack`).
use crate::shared::lock;
use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton};
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tsfp_ui_input::live_menu::{ACTION_BACK, ACTION_SELECT, LiveMenu, LiveSlot};
use tsfp_ui_input::native_page::{self, CommandQueue, GuestUi, NativeCommand, PageRef};
use tsfp_ui_input::owned_catalogue;
use tsfp_ui_input::page_intents::{
    RECORDED_DELETE_LIST, RECORDED_JOIN_LIST, RECORDED_SHORTCUT_LIST, REPEAT_DELAY_MS,
    native_page_commands, replay_menu_intents, replay_menu_presses,
};
use tsfp_ui_input::profile_page::BUILDER as PROFILE_BUILDER;
use tsfp_ui_input::profile_ports::{self, PortInput};
use tsfp_ui_input::widgets::Step;
use tsfp_ui_input::{
    ContextId, IntentSet, MenuIntent, ModelLayout, Ownership, PageEdgeGate, PlayerFrame,
    PointerButtons, PointerFrame, Routed, UiEvent, UiInput,
};
use tsfp_ui_model::frontend::MAIN_MENU_BUILDER;

/// T2329 throw-away measurement switch: authored pad captures must not inherit
/// the headless display's stationary hover. Production mouse input stays native.
fn ui_cursor(window: &Window) -> Option<Vec2> {
    static PAD_REPLAY: OnceLock<bool> = OnceLock::new();
    if *PAD_REPLAY.get_or_init(|| std::env::var("TSFP_UI_PAD_REPLAY").is_ok_and(|v| v == "1")) {
        None
    } else {
        tsfp_input_intake::cursor_position(window) // T3820: the recorded cursor in a replay
    }
}

/// The Rust page owns input and the overlay is shown.
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// F9: the owner hid the overlay (input goes back to the original menu).
static USER_HIDDEN: AtomicBool = AtomicBool::new(false);
/// Focused widget id + 1 (0 = none), read by the render world to redraw on change.
static FOCUSED: AtomicU64 = AtomicU64::new(0);

struct State {
    /// The Rust main menu's input model (None until known: without a menu tree it is built from the main menu's live page
    /// record the first time the registry reads it, T2315).
    main: Option<MainMenuInput>,
    live_before: Option<LiveSlot>,
    /// Native-input page: intents held last frame and the next auto-repeat time per direction.
    page_held: IntentSet,
    /// T3414: one press edge per page (see `PageEdgeGate`).
    page_gate: PageEdgeGate,
    page_repeat_at: [u64; 4],
    ports: PortInput, // T3772: per-port input of the profile page
    destination_page: Option<crate::storage_caller::NativePageId>,
    destination_inputs: Vec<(
        crate::storage_caller::PlayerInputSource,
        IntentSet,
        [u64; 4],
    )>,
}

/// The interactive main menu: the page the input model was built from (the tree page, or the live page record), its
/// hit layout and the live menu binding.
struct MainMenuInput {
    page: tsfp_ui_model::Page,
    input: UiInput,
    layout: ModelLayout,
    menu: LiveMenu,
}

/// T2310: pages whose input runs natively (list steps, option rows, activation, back) while the original draws them.
/// Builder VA and name (docs/t-menu-catalogue.md, docs/t-ui-framework.md section 1).
pub const NATIVE_INPUT_PAGES: &[(u32, &str)] = &[
    (0x2C_A230, "Arcade type"),
    // T3426: legacy Arcade setup pages use the same native action router as
    // the other pages. This replaces the recorded-A replay activation shim.
    (0x2D_5E70, "Arcade setup category"),
    (0x2C_19F0, "Arcade player and bot setup"),
    (0x2C_BFC0, "Settings"),
    (0x2E_A8E0, "Controls"),
    // T2310 increment 5: the Controls "accept changes?" Yes/No confirm page (pushed by the Controls back handler).
    (0x2E_9EC0, "Controls accept changes"),
];
/// T2310 increment 5: a popup (confirm, message, option list) open over a native page or the main menu is native too:
/// its builder (u32::MAX: none), published by the frame service from `native_page::popup_over`.
static POPUP_BUILDER: AtomicU32 = AtomicU32::new(u32::MAX);
/// The live native page is that popup (the frame service drives `popup_over`'s page, not a builder lookup).
static PAGE_POPUP: AtomicBool = AtomicBool::new(false);
/// T3799: that popup is a builder-less option list: the editor pointer hovers it, a click accepts or backs natively.
static OPTION_LIST: AtomicBool = AtomicBool::new(false);
pub fn option_list_open() -> bool {
    OPTION_LIST.load(Ordering::Acquire) && PAGE_POPUP.load(Ordering::Acquire)
}

/// Builders whose popups take native input.
pub(crate) fn popup_parents() -> &'static [u32] {
    static PARENTS: std::sync::OnceLock<Vec<u32>> = std::sync::OnceLock::new();
    PARENTS.get_or_init(|| {
        NATIVE_INPUT_PAGES
            .iter()
            .map(|(builder, _)| *builder)
            .chain(crate::ui_pages::PAGES.iter().map(|p| p.builder))
            .chain([MAIN_MENU_BUILDER as u32])
            .collect()
    })
}
/// A native-input page is the top live page and owns keys and pad.
static PAGE_NATIVE: AtomicBool = AtomicBool::new(false);
/// Its builder (0 = none), read by the frame service.
static PAGE_BUILDER: AtomicU32 = AtomicU32::new(0);
/// Its command queue, drained on the guest thread.
static PAGE_QUEUE: std::sync::OnceLock<CommandQueue> = std::sync::OnceLock::new();
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReplayPageCommand {
    host_poll: u64,
    builder: u32,
    command: NativeCommand,
    player: u8,
}
type ReplayPageCommands = VecDeque<ReplayPageCommand>;
static REPLAY_PAGE_COMMANDS: OnceLock<Mutex<ReplayPageCommands>> = OnceLock::new();
/// Popup page already given the Map Maker Advanced default action (0 = none).
static MAPMAKER_ADVANCED_PRESET_PAGE: AtomicU32 = AtomicU32::new(0);
static REPLAY_UI_EVENTS: OnceLock<Mutex<Vec<tsfp_input::recording::Ui>>> = OnceLock::new();
static STATE: Mutex<Option<State>> = Mutex::new(None);
// Capturing lasts for the whole Bevy input frame, even when an action closes
// the owned window. Later pad/mouse systems must not see that same edge.
static STORAGE_INPUT_FRAME: AtomicBool = AtomicBool::new(false);

/// T2310: the main menu's native command queue, drained on the guest thread.
static NATIVE_QUEUE: std::sync::OnceLock<CommandQueue> = std::sync::OnceLock::new();

/// The C host's guest access table (src/host/main.c `host_ui_guest_api`).
#[repr(C)]
pub struct HostUiGuestApi {
    read32: extern "C" fn(u32, *mut u32) -> u32,
    write32: extern "C" fn(u32, u32) -> u32,
    call: extern "C" fn(u32, *const u32, u32, *mut u32) -> u32,
    call_st0: extern "C" fn(u32, *const u32, u32, *mut f64) -> u32,
}

impl HostUiGuestApi {
    #[cfg(test)]
    pub(crate) fn test_api(
        read32: extern "C" fn(u32, *mut u32) -> u32,
        write32: extern "C" fn(u32, u32) -> u32,
        call: extern "C" fn(u32, *const u32, u32, *mut u32) -> u32,
        call_st0: extern "C" fn(u32, *const u32, u32, *mut f64) -> u32,
    ) -> Self {
        Self {
            read32,
            write32,
            call,
            call_st0,
        }
    }

    /// T2312: plain read for the native HUD (`hud_live.rs`).
    pub fn read32_raw(&self, va: u32) -> Option<u32> {
        let mut value = 0;
        ((self.read32)(va, &mut value) != 0).then_some(value)
    }
    /// T2312 increment 8: one guest word write for the native HUD (the weapon panel's slide target, the original
    /// drawer's only write, value from Rust state).
    pub fn write32_raw(&self, va: u32, value: u32) -> bool {
        (self.write32)(va, value) != 0
    }
    /// T2312: cdecl call of an original predicate for the native HUD (`None`: not callable).
    pub fn call_raw(&self, va: u32, args: &[u32]) -> Option<u32> {
        let mut result = 0;
        ((self.call)(va, args.as_ptr(), args.len() as u32, &mut result) != 0).then_some(result)
    }
}

impl GuestUi for &HostUiGuestApi {
    fn read32(&self, va: u32) -> Option<u32> {
        let mut value = 0;
        ((self.read32)(va, &mut value) != 0).then_some(value)
    }
    fn write32(&mut self, va: u32, value: u32) -> bool {
        (self.write32)(va, value) != 0
    }
    fn call(&mut self, va: u32, args: &[u32]) -> Option<u32> {
        let mut result = 0;
        ((self.call)(va, args.as_ptr(), args.len() as u32, &mut result) != 0).then_some(result)
    }
    fn call_st0(&mut self, va: u32, args: &[u32]) -> Option<f64> {
        let mut st0 = 0.0;
        ((self.call_st0)(va, args.as_ptr(), args.len() as u32, &mut st0) != 0).then_some(st0)
    }
    fn native_map_load(&mut self) -> bool {
        crate::storage_live::native_editor_load_current()
    }
    fn native_map_save(&mut self) -> bool {
        crate::storage_live::native_editor_save_current()
    }
    fn native_map_history(&mut self, direction: u32) -> Option<bool> {
        crate::storage_live::native_map_history(self, direction)
    }
    fn native_settings_handler(&mut self, handler: u32, page: PageRef) -> bool {
        crate::ui_extensions::native_settings_handler(self, handler, page)
    }
    fn native_settings_step(
        &mut self,
        page: PageRef,
        _row: u32,
        direction: Step,
    ) -> Option<Option<i32>> {
        crate::ui_extensions::native_settings_step(self, page, direction)
    }
}

/// T2310: frame boundary on the guest thread: run the main menu's queued native actions.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_ui_frame_service(api: *const HostUiGuestApi) {
    // SAFETY: the C host passes a pointer to its static API table.
    let Some(api) = (unsafe { api.as_ref() }) else {
        return;
    };
    crate::rea_ai_trace_live::service(api); // T2407: opt-in reference/owned AI actor-state trace.
    crate::hud_live::service(api); // T2312: native HUD state for this frame.
    crate::mapmaker_ui_live::pointer_io(); // T3392: record or replay the mouse pointer rows.
    let mut guest = api;
    crate::storage_live::frame_service(&guest); // T2319: import runtime authentication and persist native edits.
    crate::storage_live::capture_native_editor_snapshot(&mut guest); // T2383: opt-in owned edit history.
    // T2315: without a menu tree the main-menu queue appears only once the registry has read the main menu, so the
    // registry service below runs whenever the Rust UI is on.
    if NATIVE_QUEUE.get().is_none() && crate::ui_overlay::prepared().is_none() {
        return;
    }
    if crate::storage_live::window_active() && !user_hidden() {
        for queue in [NATIVE_QUEUE.get(), PAGE_QUEUE.get()].into_iter().flatten() {
            if let Ok(mut commands) = queue.lock() {
                commands.clear();
            }
        }
        return;
    }
    tsfp_ui_input::native_script::service(&mut guest);
    let mut done = Vec::new();
    if let Some(queue) = NATIVE_QUEUE.get() {
        native_page::service(&mut guest, MAIN_MENU_BUILDER as u32, queue, &mut done);
    }
    service_replay_page_command(&mut guest, &mut done);
    let popup = native_page::popup_over(&guest, popup_parents());
    let builder = PAGE_BUILDER.load(Ordering::Acquire);
    let page_done_start = done.len();
    let experience_prompt = crate::ui_pages::active_mapmaker_experience_prompt(&guest);
    let experience_page = if !experience_prompt {
        None
    } else if PAGE_POPUP.load(Ordering::Acquire) {
        popup
    } else if builder != 0 {
        native_page::find_page(&guest, builder).map(|(_, page)| page)
    } else {
        None
    };
    let mut experience_cursor = experience_page.map(|page| page.cursor(&guest));
    if let Some(queue) = PAGE_QUEUE.get() {
        if PAGE_POPUP.load(Ordering::Acquire) {
            native_page::service_on(&mut guest, popup, queue, &mut done);
        } else if builder == PROFILE_BUILDER {
            profile_ports::push(lock(queue).drain(..).map(|command| (0, command))); // T3772: port 0
        } else {
            native_page::service(&mut guest, builder, queue, &mut done);
        }
    }
    let mut ports_done = Vec::new();
    profile_ports::service(&mut guest, &mut ports_done);
    for (port, command, outcome) in ports_done {
        eprintln!("tsfp-bevy: --ui=rust: native port {port} {command:?} -> {outcome:?}");
    }
    if experience_prompt {
        for (command, outcome) in done.iter().skip(page_done_start).copied() {
            let _ = crate::storage_live::record_mapmaker_experience_prompt_outcome(
                &mut experience_cursor,
                command,
                outcome,
            );
        }
    }
    for (command, outcome) in done {
        eprintln!("tsfp-bevy: --ui=rust: native {command:?} -> {outcome:?}");
    }
    crate::text_entry_live::frame_service(&mut guest);
    // T3853: publish popup routing and input identity together after commands may open or close a page.
    let popup = native_page::popup_over(&guest, popup_parents());
    let popup_builder = popup.map_or(u32::MAX, |p| native_page::popup_identity(&guest, p));
    POPUP_BUILDER.store(popup_builder, Ordering::Release);
    let list = popup.is_some_and(|p| native_page::is_option_list(&guest, p));
    OPTION_LIST.store(list, Ordering::Release);
    crate::ui_overlay::observe_input_top(&guest);
    crate::mapmaker_ui_live::service(&mut guest); // T1916: the Map Maker pointer follows the mouse.
    // T2311: read the top registered page's live record for the Rust drawer and the pointer.
    crate::ui_pages::service(&mut guest);
}

/// T2311: F9 handed drawing and input back to the original.
pub fn user_hidden() -> bool {
    USER_HIDDEN.load(Ordering::Acquire)
}

pub fn active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

/// Input is captured by the Rust page (the plain pad path must not feed the guest): the Rust main menu, or a
/// native-input page (T2310).
pub fn capturing() -> bool {
    (STORAGE_INPUT_FRAME.load(Ordering::Acquire)
        || crate::storage_live::window_active()
        || active()
        || PAGE_NATIVE.load(Ordering::Acquire))
        && !USER_HIDDEN.load(Ordering::Acquire)
}

/// The Rust main menu page itself owns input (overlay and pointer).
fn main_menu_capturing() -> bool {
    active() && !USER_HIDDEN.load(Ordering::Acquire)
}

/// T2168: while the Rust main menu owns input under `--input-router --mouse-keyboard` (the default since 2026-10-13), the
/// pointer is the menu pointer: the router releases it and gets no mouse presses, motion or wheel. Other pages and gameplay
/// keep the mk behaviour unchanged.
pub fn owns_pointer() -> bool {
    ((STORAGE_INPUT_FRAME.load(Ordering::Acquire) || crate::storage_live::window_active())
        && !user_hidden())
        || main_menu_capturing()
        || (crate::ui_back::enabled()
            && !user_hidden()
            && crate::text_entry_live::footer_page().is_some())
        || (PAGE_NATIVE.load(Ordering::Acquire)
            && !USER_HIDDEN.load(Ordering::Acquire)
            && (crate::ui_pages::input_page(PAGE_BUILDER.load(Ordering::Acquire)).is_some()
                || menu_auto_release()))
}

/// Current native-input page identity, without cloning its text/draw snapshot.
pub fn native_page_builder() -> Option<u32> {
    PAGE_NATIVE
        .load(Ordering::Acquire)
        .then(|| PAGE_BUILDER.load(Ordering::Acquire))
}

/// Opt-in UI-7 pointer policy also covers native-only pages such as Settings.
fn menu_auto_release() -> bool {
    std::env::var("TSFP_UI_EXTENSIONS").is_ok_and(|v| pointer_extension(&v))
}

fn pointer_extension(value: &str) -> bool {
    value.split(',').any(|part| {
        matches!(
            part.trim(),
            "menu-pointer" | "mk-settings" | "back-button" | "all"
        )
    })
}

#[cfg(test)]
mod extension_policy_tests {
    #[test]
    fn mapmaker_native_input_routes_map_list_and_mode_popup_by_live_child() {
        assert_eq!(
            super::native_input_route(0x2D_5E70, u32::MAX),
            Some(super::NativeInputRoute::Registered("Arcade setup category"))
        );
        assert_eq!(
            super::native_input_route(0x2C_19F0, u32::MAX),
            Some(super::NativeInputRoute::Registered(
                "Arcade player and bot setup"
            ))
        );
        assert_eq!(
            super::native_input_route(0x32_BEA0, u32::MAX),
            Some(super::NativeInputRoute::Registered("Map Maker map list"))
        );
        assert_eq!(
            super::native_input_route(
                tsfp_ui_input::mapmaker_handlers::MODE_CHOICE_BUILDER,
                u32::MAX
            ),
            Some(super::NativeInputRoute::Popup)
        );
        assert_eq!(super::native_input_route(0, u32::MAX), None);
        // T3799: an option list over Map Settings publishes the settings builder (`popup_identity`), and the
        // snapshot's top is that page, so its commands go to the list, not to the page under it.
        assert_eq!(
            super::native_input_route(0x31_33E0, 0x31_33E0),
            Some(super::NativeInputRoute::Popup)
        );
    }

    #[test]
    fn mapmaker_progress_input_requires_completed_rust_owned_operation() {
        use tsfp_mapmaker::ui::{
            MapMakerUiCommand, MapMakerUiState, StorageOperation, StorageProgressOutcome,
        };
        use tsfp_ui_input::native_page::NativeCommand;

        let mut state = MapMakerUiState::default();
        state.dispatch(MapMakerUiCommand::OpenStorageProgress(
            StorageOperation::SaveMap,
        ));
        let pending = state.storage_progress().unwrap();
        assert!(
            super::mapmaker_storage_progress_commands(pending, vec![NativeCommand::Back],)
                .is_empty()
        );

        state.dispatch(MapMakerUiCommand::CompleteStorageProgress {
            progress: pending.id,
            outcome: StorageProgressOutcome::Failed,
        });
        let completed = state.storage_progress().unwrap();
        assert_eq!(
            super::mapmaker_storage_progress_commands(
                completed,
                vec![NativeCommand::Focus(1), NativeCommand::Back],
            ),
            [NativeCommand::Back]
        );
        assert_eq!(
            super::mapmaker_storage_progress_commands(completed, vec![NativeCommand::Focus(1)],),
            []
        );
    }

    #[test]
    fn mapmaker_advanced_default_is_queued_once_per_live_popup() {
        let page = 0x7000;
        assert_eq!(
            super::default_mode_selection_page(0, Some(page)),
            (page, true)
        );
        assert_eq!(
            super::default_mode_selection_page(page, Some(page)),
            (page, false)
        );
        assert_eq!(super::default_mode_selection_page(page, None), (0, false));
        assert_eq!(
            super::default_mode_selection_page(0, Some(page)),
            (page, true)
        );
        assert_eq!(
            tsfp_ui_input::mapmaker_handlers::DEFAULT_MODE_CHOICE_ROW,
            1,
            "Advanced is the second measured Select Mode row"
        );
    }

    #[test]
    fn display_pointer_accounts_for_letterbox_origin() {
        let window = bevy::prelude::Vec2::new(1920.0, 1080.0);
        let picture = bevy::prelude::Vec2::new(1440.0, 1080.0);
        assert_eq!(
            super::cursor_to_display(bevy::prelude::Vec2::new(240.0, 0.0), window, picture),
            Some([0.0, 0.0])
        );
        assert_eq!(
            super::cursor_to_display(bevy::prelude::Vec2::new(960.0, 540.0), window, picture),
            Some([0.5, 0.5])
        );
        assert_eq!(
            super::cursor_to_display(bevy::prelude::Vec2::new(239.0, 540.0), window, picture),
            None
        );
    }

    #[test]
    fn original_and_unknown_layers_do_not_release_additional_pages() {
        for value in ["", "original", "unknown", "not-menu-pointer"] {
            assert!(!super::pointer_extension(value));
        }
        for value in [
            "menu-pointer",
            "mk-settings",
            "all",
            " original, menu-pointer ",
        ] {
            assert!(super::pointer_extension(value));
        }
    }
}

/// T2168: menu keys the Rust main menu reads itself; their presses must not also reach the mk router or the guest
/// (Escape would otherwise also be the guest's START). Releases always pass.
pub fn owns_key(key: KeyCode) -> bool {
    capturing()
        && (tsfp_ui_input::keyboard::menu_intent(crate::events::sdl_keycode(key)).is_some()
            || key == KeyCode::Space)
}

/// The in-game menu is open, so the physical pad START goes to the guest pause toggle
/// while the page itself takes native input.
pub fn start_passes() -> bool {
    crate::ui_pages::ingame()
        && !crate::ui_extensions::open()
        && !STORAGE_INPUT_FRAME.load(Ordering::Acquire)
        && !crate::storage_live::window_active()
}

/// T2191: a pad button press held back from the guest while a Rust page owns input (START passes in game).
pub fn holds_pad_button(button: GamepadButton) -> bool {
    capturing() && !(button == GamepadButton::Start && start_passes())
}

pub fn toggle_hidden() {
    USER_HIDDEN.fetch_xor(true, Ordering::AcqRel);
}

pub fn overlay_visible() -> bool {
    main_menu_capturing()
}

/// The widget the overlay should show focused.
pub fn focused() -> Option<tsfp_ui_model::WidgetId> {
    match FOCUSED.load(Ordering::Acquire) {
        0 => None,
        id => Some(tsfp_ui_model::WidgetId(id - 1)),
    }
}

fn publish_focus(id: Option<tsfp_ui_model::WidgetId>) {
    FOCUSED.store(id.map_or(0, |w| w.0 + 1), Ordering::Release);
}

/// Shell start (after `ui_overlay::prepare`): the input state, with the main menu from the prepared model when a menu tree
/// was loaded. T2315 item 5: without a tree the main-menu input model is built from the main menu's live page record the
/// first time the registry reads it ([`main_from_registry`]); the other registry pages have native input from the start.
pub fn init() {
    let Some(prepared) = crate::ui_overlay::prepared() else {
        return;
    };
    let _ = PAGE_QUEUE.set(CommandQueue::default());
    let main = prepared
        .model
        .page(prepared.page)
        .and_then(|page| main_menu_input(page, "menu tree", None));
    *lock(&STATE) = Some(State {
        main,
        live_before: None,
        page_held: IntentSet::default(),
        page_gate: PageEdgeGate::default(),
        page_repeat_at: [0; 4],
        ports: PortInput::default(),
        destination_page: None,
        destination_inputs: Vec::new(),
    });
}

/// T2315: the main-menu input model from the live main-menu page of the registry (no tree file).
fn main_from_registry() -> Option<MainMenuInput> {
    let current = crate::ui_pages::current()?;
    if u64::from(current.descriptor.builder) != MAIN_MENU_BUILDER {
        return None;
    }
    let page = current.content.model.page(current.content.page)?;
    // T3781: rows in guest cursor order, greyed rows included (two player main menu).
    let rows = current.live.cursor_widgets(&current.cursor_nodes);
    main_menu_input(page, "live page record", Some(rows))
}

fn main_menu_input(
    page: &tsfp_ui_model::Page,
    source: &str,
    rows: Option<Vec<tsfp_ui_model::WidgetId>>,
) -> Option<MainMenuInput> {
    let layout = ModelLayout::from_page(page, 640.0, 480.0)?;
    let menu = LiveMenu::with_cursor_rows(page, MAIN_MENU_BUILDER as u32, rows);
    if let Some(native) = menu.native() {
        let _ = NATIVE_QUEUE.set(native.queue.clone());
    }
    let mut input = UiInput::new();
    input.open_context(ContextId(0), Ownership::Any);
    publish_focus(menu.focus.selected);
    eprintln!(
        "tsfp-bevy: --ui=rust: interactive main menu ready ({} rows, from the {source}): arrows/Enter/Esc, mouse hover/click, pad D-pad/A/B",
        menu.rows.len()
    );
    Some(MainMenuInput {
        page: page.clone(),
        input,
        layout,
        menu,
    })
}

fn pad_intents(pads: &Query<&Gamepad>, held: IntentSet) -> IntentSet {
    pads.iter().fold(held, pad_held)
}

/// T3772: held intents per router slot (guest port): each pad on its slot, the keyboard on the mouse and keyboard one.
fn port_intents(pads: &Query<(Entity, &Gamepad)>, keys: IntentSet) -> [IntentSet; 4] {
    let sources = tsfp_input_slots::sources::current();
    let mut held = [IntentSet::default(); 4];
    held[sources.keyboard_slot()] = keys;
    for (entity, pad) in pads.iter() {
        let slot = sources.pad_slot(entity.index());
        held[slot] = pad_held(held[slot], pad);
    }
    held
}

/// Record and queue the profile page commands of each port; the generic page input gets nothing this frame.
fn queue_port_commands(commands: Vec<(u8, NativeCommand)>) -> IntentSet {
    for (port, command) in &commands {
        if crate::input_recording::enabled()
            && let Some(event) = command_recorded_event(command)
            && let Err(error) = crate::input_recording::record_ui(PROFILE_BUILDER, 0, *port, event)
        {
            eprintln!("tsfp-bevy: UI input recording failed: {error}");
        }
    }
    profile_ports::push(commands);
    IntentSet::default()
}

fn pad_held(held: IntentSet, pad: &Gamepad) -> IntentSet {
    let x = pad.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
    let y = pad.get(GamepadAxis::LeftStickY).unwrap_or(0.0);
    [
        (GamepadButton::DPadUp, y > 0.5, MenuIntent::Up),
        (GamepadButton::DPadDown, y < -0.5, MenuIntent::Down),
        (GamepadButton::DPadLeft, x < -0.5, MenuIntent::Left),
        (GamepadButton::DPadRight, x > 0.5, MenuIntent::Right),
        (GamepadButton::South, false, MenuIntent::Accept),
        (GamepadButton::East, false, MenuIntent::Back),
        (GamepadButton::Start, false, MenuIntent::Start),
        (GamepadButton::North, false, MenuIntent::Delete),
        (GamepadButton::West, false, MenuIntent::Shortcut),
    ]
    .into_iter()
    .filter(|(button, stick, _)| *stick || pad.pressed(*button))
    .fold(held, |held, (.., intent)| held.with(intent))
}

fn key_intents(keys: &ButtonInput<KeyCode>) -> IntentSet {
    let mut held = IntentSet::default();
    for key in keys.get_pressed().chain(keys.get_just_pressed()) {
        if let Some(intent) = tsfp_ui_input::keyboard::menu_intent(crate::events::sdl_keycode(*key))
        {
            held = held.with(intent);
        }
    }
    held
}

/// Window cursor to the overlay's 640x480 design space (the overlay is stretched over the letterboxed picture, centred).
pub fn cursor_to_design(cursor: Vec2, window: Vec2, picture: Vec2) -> Option<(f32, f32)> {
    if picture.x <= 0.0 || picture.y <= 0.0 {
        return None;
    }
    let origin = (window - picture) * 0.5;
    let local = cursor - origin;
    Some((local.x * 640.0 / picture.x, local.y * 480.0 / picture.y))
}

/// The top live page slot (highest occupied slot index) if it is the main menu.
/// Render-time native cursor normalized within the centred picture texture.
fn cursor_to_display(cursor: Vec2, window: Vec2, picture: Vec2) -> Option<[f32; 2]> {
    let (x, y) = cursor_to_design(cursor, window, picture)?;
    let sample = [x / 640.0, y / 480.0];
    sample
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        .then_some(sample)
}

fn live_main_menu() -> Option<LiveSlot> {
    let top = crate::ui_overlay::top_live_page()?;
    (u64::from(top.builder) == MAIN_MENU_BUILDER).then_some(top)
}

/// The current Rust-owned page that may consume the next recorded UI event.
/// Keeping this identity at the replay boundary leaves events for a page that
/// is not live at the front of the recording until its preceding transition
/// has completed.
pub(crate) fn replay_active_page() -> Option<u32> {
    if crate::storage_live::window_active() {
        return Some(storage_recording_page(
            crate::storage_live::destination_input_presentation().is_some(),
        ));
    }
    if let Some((page, _)) = live_native_page() {
        return Some(page.builder);
    }
    // Registered front-end pages (including Arcade selectors) own input through
    // ui_pages rather than live_native_page's dedicated native-input routes.
    if let Some(page) = crate::ui_pages::current() {
        return Some(page.descriptor.builder);
    }
    if crate::mapmaker_ui_live::native_page() {
        return native_page_builder()
            .or_else(|| crate::ui_pages::current().map(|page| page.descriptor.builder));
    }
    if crate::mapmaker_description_live::keyboard_grid_active() {
        return crate::ui_overlay::top_live_page().map(|page| page.builder);
    }
    live_main_menu().map(|page| page.builder)
}

/// Whether a Rust-owned page owns input on this host frame. New recordings
/// persist semantic `UiInput` rows instead of gameplay snapshots. During replay,
/// `T` never enters the guest queue; unmatched legacy snapshots can still reach
/// the typed page handler below.
pub fn native_ui_owns_input() -> bool {
    !USER_HIDDEN.load(Ordering::Acquire) && replay_active_page().is_some()
}

fn replay_input_for_page(
    input: Option<tsfp_input::player::PlayerInput>,
    active_page: Option<u32>,
    page_has_semantic_events: bool,
) -> Option<tsfp_input::player::PlayerInput> {
    // A matching U row is already the native action for this recorded tick.
    // Sparse older recordings still need their unmatched T snapshot for page input.
    (active_page.is_none() || !page_has_semantic_events)
        .then_some(input)
        .flatten()
}

/// PreUpdate, before the plain gamepad path: route devices through the Rust page and feed the dispatched taps to the guest.
pub fn ui_live_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut wheel: EventReader<bevy::input::mouse::MouseWheel>,
    pad_sources: Query<(Entity, &Gamepad)>,
) {
    // Drain even when the UI is hidden: old wheel events must not leak into a reopened page.
    let wheel_delta = wheel
        .read()
        .map(|e| match e.unit {
            bevy::input::mouse::MouseScrollUnit::Line => e.y,
            bevy::input::mouse::MouseScrollUnit::Pixel => e.y / 40.0,
        })
        .sum::<f32>();
    if crate::input_recording::semantic_replay() {
        let events = match crate::input_recording::replay_ui_for_pages(
            crate::input_recording::replay_poll(),
            replay_active_page(),
            crate::text_entry_live::active_page(),
        ) {
            Ok(events) => events,
            Err(error) => {
                eprintln!("tsfp-bevy: UI input replay failed: {error}");
                return;
            }
        };
        *lock(REPLAY_UI_EVENTS.get_or_init(|| Mutex::new(Vec::new()))) = events.clone();
        crate::text_entry_live::replay_recorded_events(&events);
        // Route page-addressed commands before checking whether the frame service has published
        // that builder as the live page. Transitions can briefly leave no active builder, but
        // the recorded page still owns the semantic row and its command queue preserves it.
        replay_native_page_input(&events);
        let active_page = replay_active_page();
        let live_tick = crate::wi::simulation_tick();
        // T3393: a recorded tick's pad edges reach the native pages once. Bevy may update several
        // times per tick (windowed: many, headless: about one), and a re-read tick re-fired its
        // `pressed` edges (MEASURED r5: a second Accept on the profile text page in windowed
        // replays only), so the action count depended on the Bevy frame rate.
        static LAST_UI_TICK: AtomicU64 = AtomicU64::new(u64::MAX);
        let fresh = !crate::input_recording::tick_clock()
            || LAST_UI_TICK.swap(live_tick, Ordering::AcqRel) != live_tick;
        let player_input = if fresh {
            crate::input_recording::replay_player_input(live_tick)
        } else {
            None
        };
        let semantic_event_at_tick =
            crate::input_recording::replay_has_ui_event(active_page, live_tick);
        let input = replay_input_for_page(player_input, active_page, semantic_event_at_tick);
        let main_menu_live = live_main_menu().is_some();
        let now = crate::shared::ui_now_ms();
        if crate::storage_live::window_active() {
            // Storage has its own typed action handler and consumes this tick's
            // page-addressed events in storage_window_input below.
        } else if live_native_page().is_some() || crate::mapmaker_ui_live::native_page() {
            // Page-addressed semantic commands were queued above by recorded builder identity.
        } else {
            replay_main_menu_input(
                &events,
                if main_menu_live { input.as_ref() } else { None },
                now,
            );
        }
        let mut guard = lock(&STATE);
        if let Some(state) = guard.as_mut() {
            native_input_page(
                state,
                &keys,
                &pads,
                now,
                Some(if main_menu_live { None } else { input }),
                None,
            );
        }
        drop(guard);
        crate::input_recording::ui_tick_processed(live_tick);
        return;
    }
    lock(REPLAY_UI_EVENTS.get_or_init(|| Mutex::new(Vec::new()))).clear();
    if keys.just_pressed(KeyCode::F9) {
        toggle_hidden();
    }
    STORAGE_INPUT_FRAME.store(
        !user_hidden() && crate::storage_live::window_active(),
        Ordering::Release,
    );
    let native_pointer = (tsfp_wgpu::frame_layer::display_rate()
        && owns_pointer()
        && !crate::mapmaker_ui_live::native_page())
    .then(|| window.single().ok())
    .flatten()
    .filter(|w| tsfp_input_intake::focused(w))
    .and_then(|w| {
        cursor_to_display(
            ui_cursor(w)?,
            Vec2::new(w.width(), w.height()),
            crate::app::picture_size(),
        )
    });
    crate::ui_overlay::set_display_pointer(native_pointer);
    let mut guard = lock(&STATE);
    let Some(state) = guard.as_mut() else { return };
    if STORAGE_INPUT_FRAME.load(Ordering::Acquire) {
        // KeyboardInput text is applied by keyboard_events before storage
        // Accept/Back. A last character and Enter in one frame must save it.
        return;
    }
    if state.main.is_none() {
        state.main = main_from_registry();
    }
    let live = live_main_menu().filter(|_| state.main.is_some());
    let now = crate::shared::ui_now_ms();
    let was_active = ACTIVE.swap(live.is_some(), Ordering::AcqRel);
    if live.is_some() && !was_active {
        eprintln!("tsfp-bevy: --ui=rust: main menu live, the Rust page owns input");
        // T3781: greyed rows follow the player count; take the live record's flags on each entry.
        if let (Some(main), Some(current)) = (state.main.as_mut(), crate::ui_pages::current())
            && let Some(page) = current.content.model.page(current.content.page)
            && page.widgets.len() == main.page.widgets.len()
        {
            main.page = page.clone();
        }
    } else if live.is_none() && was_active {
        eprintln!("tsfp-bevy: --ui=rust: main menu left, legacy page in charge");
    }
    if live != state.live_before {
        if let (Some(after), Some(before)) = (live, state.live_before)
            && after.cursor != before.cursor
        {
            eprintln!(
                "tsfp-bevy: --ui=rust: guest cursor {} -> {}",
                before.cursor, after.cursor
            );
        }
        state.live_before = live;
    }
    if !USER_HIDDEN.load(Ordering::Acquire) || live.is_none() {
        if let Some(main) = state.main.as_mut()
            && main.menu.observe(live)
        {
            publish_focus(main.menu.focus.selected);
        }
    }
    // T3391: only while the main menu is the live page. The model used to route (and record) every
    // pad press on any Rust page too, leaving an echo row per press that replays as a stale activation.
    // T3414: the page that owns device input this frame; a held key never acts on the next page.
    let input_owner = if live.is_some() {
        Some(MAIN_MENU_BUILDER as u32)
    } else {
        live_native_page().map(|(page, _)| page.builder)
    };
    let gated_held = state
        .page_gate
        .filter(input_owner, pad_intents(&pads, key_intents(&keys)));
    // T3772: the profile page answers each controller port itself (player 2 joins and chooses on its own pad).
    let port_held = port_intents(&pad_sources, key_intents(&keys));
    let gated_held = match state.ports.commands(input_owner, port_held, now) {
        Some(commands) => queue_port_commands(commands),
        None => gated_held,
    };
    if let Some(main) = state.main.as_mut()
        && capturing()
        && live.is_some()
    {
        let held = gated_held;
        let pointer = window.single().ok().and_then(|w| {
            let cursor = ui_cursor(w)?;
            let picture = crate::app::picture_size();
            Some(PointerFrame {
                pos: cursor_to_design(cursor, Vec2::new(w.width(), w.height()), picture),
                buttons: PointerButtons {
                    left: mouse.pressed(MouseButton::Left),
                    right: mouse.pressed(MouseButton::Right),
                    middle: mouse.pressed(MouseButton::Middle),
                },
                wheel: 0,
            })
        });
        let mut routed: Vec<Routed> = Vec::new();
        main.input.set_focus(
            ContextId(0),
            main.menu
                .focus
                .selected
                .map(|w| tsfp_ui_input::WidgetId(w.0)),
        );
        main.input.update(
            ContextId(0),
            now,
            &main.layout,
            &[PlayerFrame {
                held,
                chars: Vec::new(),
            }],
            pointer.as_ref(),
            &mut routed,
        );
        if crate::ui_back::pointer(
            pointer.as_ref().and_then(|p| p.pos),
            mouse.just_pressed(MouseButton::Left),
        ) {
            if let Some(native) = main.menu.native_mut() {
                let _ = tsfp_ui_input::UiBindings::action(
                    native,
                    ACTION_BACK,
                    tsfp_ui_input::ActionArg::None,
                );
            }
            if crate::input_recording::enabled()
                && let Err(error) = crate::input_recording::record_ui(
                    MAIN_MENU_BUILDER as u32,
                    0,
                    0,
                    tsfp_input::recording::UiEvent::Back,
                )
            {
                eprintln!("tsfp-bevy: UI input recording failed: {error}");
            }
            routed.clear();
        }
        for r in &routed {
            if crate::input_recording::enabled()
                && let Err(error) = crate::input_recording::record_ui(
                    MAIN_MENU_BUILDER as u32,
                    r.context.0,
                    r.player.0,
                    recorded_ui_event(&r.event),
                )
            {
                eprintln!("tsfp-bevy: UI input recording failed: {error}");
            }
            if let Some(action) = main.menu.apply(&main.page, &r.event) {
                eprintln!("tsfp-bevy: --ui=rust: {action:?}");
            }
        }
        publish_focus(main.menu.focus.selected);
    }
    native_input_page(state, &keys, &pads, now, None, Some(gated_held));
    registry_pointer(&mouse, &window, wheel_delta);
    autoselect(state, now);
    // T2310 increment 5 (T2308): the pad-tap dispatch (LegacyMenuDriver edges fed to the guest pad queue) is gone from
    // the shell: the main menu `LiveMenu` is native (T2316: the tap driver is the measurement tool `pad_route`), so every
    // action is a native page command. The stop line keeps reporting the tap count, which is 0 by construction.
}

fn recorded_ui_event(event: &UiEvent) -> tsfp_input::recording::UiEvent {
    use tsfp_input::recording::UiEvent as Recorded;
    match event {
        UiEvent::Focus(widget) => Recorded::Focus(widget.0),
        UiEvent::Activate(widget) => Recorded::Activate(widget.0),
        UiEvent::Back => Recorded::Back,
        UiEvent::Start => Recorded::Start,
        UiEvent::Scroll(list, delta) => Recorded::Scroll(list.0, *delta),
        UiEvent::Hover(widget) => Recorded::Hover(widget.0),
        UiEvent::HoverLeave => Recorded::HoverLeave,
        UiEvent::TextInput(ch) => Recorded::TextInput(*ch),
        UiEvent::TextCommit { widget, text } => Recorded::TextCommit {
            widget: widget.0,
            text: text.clone(),
        },
        UiEvent::TextCancel(widget) => Recorded::TextCancel(widget.0),
    }
}

fn replay_ui_event(event: tsfp_input::recording::UiEvent) -> UiEvent {
    use tsfp_input::recording::UiEvent as Recorded;
    match event {
        Recorded::Focus(widget) => UiEvent::Focus(tsfp_ui_input::WidgetId(widget)),
        Recorded::Activate(widget) => UiEvent::Activate(tsfp_ui_input::WidgetId(widget)),
        Recorded::Back => UiEvent::Back,
        Recorded::Start => UiEvent::Start,
        Recorded::Scroll(list, delta) => UiEvent::Scroll(tsfp_ui_input::ListId(list), delta),
        Recorded::Hover(widget) => UiEvent::Hover(tsfp_ui_input::WidgetId(widget)),
        Recorded::HoverLeave => UiEvent::HoverLeave,
        Recorded::TextInput(ch) => UiEvent::TextInput(ch),
        Recorded::TextCommit { widget, text } => UiEvent::TextCommit {
            widget: tsfp_ui_input::WidgetId(widget),
            text,
        },
        Recorded::TextCancel(widget) => UiEvent::TextCancel(tsfp_ui_input::WidgetId(widget)),
    }
}

fn command_recorded_event(command: &NativeCommand) -> Option<tsfp_input::recording::UiEvent> {
    use tsfp_input::recording::UiEvent as Recorded;
    match command {
        NativeCommand::Accept => Some(Recorded::Start),
        NativeCommand::Back | NativeCommand::ResumeGame => Some(Recorded::Back),
        NativeCommand::Step(Step::Up) => Some(Recorded::Scroll(0, -1)),
        NativeCommand::Step(Step::Down) => Some(Recorded::Scroll(0, 1)),
        NativeCommand::Step(Step::Left) => Some(Recorded::Scroll(1, -1)),
        NativeCommand::Step(Step::Right) => Some(Recorded::Scroll(1, 1)),
        NativeCommand::Focus(row) | NativeCommand::Select(row) => {
            Some(Recorded::Focus(u64::from(*row)))
        }
        NativeCommand::Scroll(delta) => Some(Recorded::Scroll(0, *delta)),
        NativeCommand::Adjust { row, right } => Some(Recorded::Scroll(
            if *right { u32::MAX } else { u32::MAX - 1 },
            *row as i32,
        )),
        NativeCommand::Delete => Some(Recorded::Scroll(RECORDED_DELETE_LIST, 0)),
        NativeCommand::Shortcut => Some(Recorded::Scroll(RECORDED_SHORTCUT_LIST, 0)),
        NativeCommand::Join(port) => Some(Recorded::Scroll(RECORDED_JOIN_LIST, i32::from(*port))),
    }
}

fn recorded_native_commands(events: &[tsfp_input::recording::Ui]) -> Vec<NativeCommand> {
    events
        .iter()
        .filter_map(|event| match &event.event {
            tsfp_input::recording::UiEvent::Start => Some(NativeCommand::Accept),
            tsfp_input::recording::UiEvent::Back => Some(NativeCommand::Back),
            tsfp_input::recording::UiEvent::Focus(row) => Some(NativeCommand::Focus(*row as u32)),
            tsfp_input::recording::UiEvent::Activate(row) => {
                Some(NativeCommand::Select(*row as u32))
            }
            tsfp_input::recording::UiEvent::Scroll(1, delta) if *delta < 0 => {
                Some(NativeCommand::Step(Step::Left))
            }
            tsfp_input::recording::UiEvent::Scroll(1, delta) if *delta > 0 => {
                Some(NativeCommand::Step(Step::Right))
            }
            tsfp_input::recording::UiEvent::Scroll(u32::MAX, row) => Some(NativeCommand::Adjust {
                row: *row as u32,
                right: true,
            }),
            tsfp_input::recording::UiEvent::Scroll(0xFFFF_FFFE, row) => {
                Some(NativeCommand::Adjust {
                    row: *row as u32,
                    right: false,
                })
            }
            tsfp_input::recording::UiEvent::Scroll(RECORDED_DELETE_LIST, _) => {
                Some(NativeCommand::Delete)
            }
            tsfp_input::recording::UiEvent::Scroll(RECORDED_SHORTCUT_LIST, _) => {
                Some(NativeCommand::Shortcut)
            }
            tsfp_input::recording::UiEvent::Scroll(RECORDED_JOIN_LIST, port) => {
                u8::try_from(*port).ok().map(NativeCommand::Join)
            }
            tsfp_input::recording::UiEvent::Scroll(list, d) => {
                tsfp_ui_input::keyboard::list_scroll_step(*list, *d).map(NativeCommand::Step)
            } // T3723
            _ => None,
        })
        .collect()
}

fn events_for_page(
    events: Vec<tsfp_input::recording::Ui>,
    builder: u32,
) -> Vec<tsfp_input::recording::Ui> {
    events
        .into_iter()
        .filter(|event| event.page == builder && event.context == 0 && event.player == 0)
        .collect()
}

/// Test and diagnostic injection seam: inject a semantic UI event into an active recording with
/// an explicit native page identity. This is useful on hosts where synthetic OS input is not
/// delivered to Bevy's input router.
pub fn inject_recorded_ui_event(
    page: u32,
    event: tsfp_input::recording::UiEvent,
) -> Result<(), String> {
    crate::input_recording::record_ui(page, 0, 0, event)
}

/// Replay the same UiInput semantic events through the Rust menu model. Device polling is skipped
/// during replay so host keyboard, mouse, and pad state cannot change the route.
fn replay_main_menu_input(
    events: &[tsfp_input::recording::Ui],
    fallback: Option<&tsfp_input::player::PlayerInput>,
    now: u64,
) {
    let tick = crate::wi::simulation_tick();
    let mut guard = lock(&STATE);
    let Some(state) = guard.as_mut() else { return };
    if state.main.is_none() {
        state.main = main_from_registry();
    }
    let live = live_main_menu().filter(|_| state.main.is_some());
    let Some(main) = state.main.as_mut() else {
        if !events.is_empty() {
            eprintln!(
                "tsfp-bevy: UI input replay skipped {} event(s) at tick {tick}: no live Rust main-menu model",
                events.len()
            );
        }
        return;
    };
    if main.menu.observe(live) {
        publish_focus(main.menu.focus.selected);
    }
    for recorded in events {
        if recorded.page != 0 && recorded.page != MAIN_MENU_BUILDER as u32 {
            eprintln!(
                "tsfp-bevy: UI input replay skipped page {:#x}; current Rust menu page is {:#x}",
                recorded.page, MAIN_MENU_BUILDER
            );
            continue;
        }
        if recorded.context != 0 || recorded.player != 0 {
            eprintln!(
                "tsfp-bevy: UI input replay skipped unsupported context/player {}/{}",
                recorded.context, recorded.player
            );
            continue;
        }
        let event = replay_ui_event(recorded.event.clone());
        if let Some(action) = main.menu.apply(&main.page, &event) {
            eprintln!("tsfp-bevy: --ui=rust replay: {action:?}");
        }
    }
    if let Some(input) = fallback {
        main.input.set_focus(
            ContextId(0),
            main.menu
                .focus
                .selected
                .map(|widget| tsfp_ui_input::WidgetId(widget.0)),
        );
        let mut routed = Vec::new();
        main.input.update(
            ContextId(0),
            now,
            &main.layout,
            &[replay_main_menu_player_frame(input)],
            None,
            &mut routed,
        );
        for event in routed {
            if let Some(action) = main.menu.apply(&main.page, &event.event) {
                eprintln!("tsfp-bevy: --ui=rust replay: {action:?}");
            }
        }
    }
    publish_focus(main.menu.focus.selected);
}

fn replay_main_menu_player_frame(input: &tsfp_input::player::PlayerInput) -> PlayerFrame {
    PlayerFrame {
        held: replay_menu_intents(input),
        chars: Vec::new(),
    }
}

/// Deliver semantic recording events to the page builder that was active when they were captured.
/// Commands enter the same Rust queue used by physical pad and keyboard routing. The recorded
/// builder identity is authoritative across a one-poll transition gap; `service_replay_page_command`
/// keeps commands queued until that exact page is live and accepts them.
fn replay_native_page_input(events: &[tsfp_input::recording::Ui]) {
    let mut pending = lock(REPLAY_PAGE_COMMANDS.get_or_init(|| Mutex::new(VecDeque::new())));
    enqueue_replay_native_page_events(&mut pending, events);
}

fn enqueue_replay_native_page_events(
    pending: &mut ReplayPageCommands,
    events: &[tsfp_input::recording::Ui],
) -> usize {
    let main_menu = tsfp_ui_model::frontend::MAIN_MENU_BUILDER as u32;
    let mut queued = 0;
    for event in events {
        let builder = event.page;
        if event.context != 0
            || (event.player != 0 && builder != PROFILE_BUILDER)
            || builder == 0
            || builder == main_menu
            || !crate::input_recording::native_ui_page_has_handler(builder)
        {
            continue;
        }
        let text_page_active = crate::text_entry_live::active_page() == Some(builder);
        if matches!(
            &event.event,
            tsfp_input::recording::UiEvent::TextInput(_)
                | tsfp_input::recording::UiEvent::TextCommit { .. }
                | tsfp_input::recording::UiEvent::TextCancel(_)
        ) || (text_page_active && matches!(&event.event, tsfp_input::recording::UiEvent::Start))
        {
            continue;
        }
        let commands = recorded_native_commands(std::slice::from_ref(event));
        if commands.is_empty() {
            eprintln!(
                "tsfp-bevy: native UI replay skipped unsupported semantic event on page {builder:#x}"
            );
            continue;
        }
        enqueue_replay_page_commands(pending, event.host_poll, builder, commands, event.player);
        queued += 1;
    }
    queued
}

fn enqueue_replay_page_commands(
    pending: &mut ReplayPageCommands,
    host_poll: u64,
    builder: u32,
    commands: impl IntoIterator<Item = NativeCommand>,
    player: u8,
) {
    pending.extend(commands.into_iter().map(|command| ReplayPageCommand {
        host_poll,
        builder,
        command,
        player,
    }));
}

fn pending_replay_page_command(
    pending: &ReplayPageCommands,
    builder: u32,
) -> Option<NativeCommand> {
    pending
        .front()
        .filter(|row| row.builder == builder)
        .map(|row| row.command)
}

fn acknowledge_page_command(
    pending: &mut ReplayPageCommands,
    builder: u32,
    command: NativeCommand,
) -> bool {
    let Some(row) = pending.front() else {
        return false;
    };
    if row.builder != builder || row.command != command {
        return false;
    }
    pending.pop_front();
    true
}

fn replay_page_command() -> Option<ReplayPageCommand> {
    lock(REPLAY_PAGE_COMMANDS.get_or_init(|| Mutex::new(VecDeque::new())))
        .front()
        .copied()
}

pub fn replay_page_commands_pending() -> bool {
    !lock(REPLAY_PAGE_COMMANDS.get_or_init(|| Mutex::new(VecDeque::new()))).is_empty()
}

fn acknowledge_replay_page_command(builder: u32, command: NativeCommand) -> bool {
    let mut pending = lock(REPLAY_PAGE_COMMANDS.get_or_init(|| Mutex::new(VecDeque::new())));
    acknowledge_page_command(&mut pending, builder, command)
}

/// Replay commands carry their page identity through the asynchronous guest-frame seam.
/// The generic PAGE_QUEUE is also fed by live input and used to service the page published in
/// PAGE_BUILDER; that builder can change between host input and the guest frame. Keep a recorded
/// commands in one route-ordered queue until the exact page at the queue head is active. The
/// replay page resolver follows the live page registry, whose stack order is not the slot order.
fn service_replay_page_command(
    guest: &mut dyn GuestUi,
    done: &mut Vec<(NativeCommand, native_page::Outcome)>,
) {
    let Some(row) = replay_page_command() else {
        return;
    };
    let Some(builder) = replay_active_page() else {
        return;
    };
    if row.builder != builder {
        return;
    }
    let page = if builder == PROFILE_BUILDER {
        profile_ports::target(guest, row.player, row.command) // T3772: the recorded player's instance
    } else if PAGE_POPUP.load(Ordering::Acquire) {
        native_page::popup_over(guest, popup_parents()) // T3799: the popup or option list live input drives
    } else {
        native_page::find_page(guest, builder).map(|(_, page)| page)
    };
    let Some(page) = page else {
        return;
    };
    let queue = CommandQueue::from(VecDeque::from([row.command]));
    let mut applied = Vec::new();
    native_page::service_on(guest, Some(page), &queue, &mut applied);
    if let Some((applied_command, outcome)) = applied.first().copied()
        && applied_command == row.command
        && acknowledge_replay_page_command(row.builder, row.command)
    {
        eprintln!(
            "replay-ui: applied row poll {} page {:#x} command {:?}: {:?}",
            row.host_poll, row.builder, row.command, outcome
        );
        done.extend(applied);
    }
}

/// Completed native destinations own each assigned device independently. Pads
/// use their Bevy entity identity, not a fabricated Xbox port or an aggregate
/// intent set that could activate for an unassigned controller.
/// After text/key routing and before plain pad/mouse routing. The ownership
/// snapshot above survives a terminal action through the remaining frame.
pub fn storage_window_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    window: Query<&Window, With<PrimaryWindow>>,
    pad_sources: Query<(Entity, &Gamepad)>,
) {
    if !STORAGE_INPUT_FRAME.load(Ordering::Acquire) || user_hidden() {
        return;
    }
    let mut guard = lock(&STATE);
    let Some(state) = guard.as_mut() else { return };
    if let Some(presentation) = crate::storage_live::destination_input_presentation() {
        owned_destination_input(state, &presentation, &keys, &mouse, &pad_sources, &window);
    } else if crate::storage_live::window_active() {
        state.destination_page = None;
        state.destination_inputs.clear();
        owned_storage_input(state, &keys, &mouse, &pads, &window);
    }
}

fn owned_destination_input(
    state: &mut State,
    presentation: &crate::storage_presentation::DestinationPresentation,
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    pads: &Query<(Entity, &Gamepad)>,
    window: &Query<&Window, With<PrimaryWindow>>,
) {
    use crate::storage_caller::{CatalogueAction, DestinationAction, PlayerInputSource};
    let Some((page, participants)) = crate::storage_live::native_players_destination() else {
        return;
    };
    if page.id != presentation.snapshot.page.id {
        return;
    }
    let recorded = lock(REPLAY_UI_EVENTS.get_or_init(|| Mutex::new(Vec::new()))).clone();
    let recorded: Vec<_> = recorded
        .into_iter()
        .filter(|event| event.page == storage_recording_page(true))
        .collect();
    if crate::input_recording::semantic_replay() {
        for event in recorded_storage_actions(&recorded) {
            let action = match event {
                CatalogueAction::Step(delta) => DestinationAction::Step(delta),
                CatalogueAction::Focus(row) => DestinationAction::Focus(row as u64),
                CatalogueAction::Accept => DestinationAction::Accept,
                CatalogueAction::Back => DestinationAction::Back,
            };
            let terminal = matches!(action, DestinationAction::Accept | DestinationAction::Back);
            if let Err(status) = crate::storage_live::native_destination_input(
                page.id,
                PlayerInputSource::KeyboardMouse,
                action,
            ) {
                eprintln!("tsfp-bevy: native destination replay error {status:#010x}");
            }
            if terminal {
                break;
            }
        }
        return;
    }
    let now = crate::shared::ui_now_ms();
    if state.destination_page != Some(page.id) {
        state.destination_page = Some(page.id);
        state.destination_inputs.clear();
    }
    for source in participants.inputs() {
        let held = match source {
            PlayerInputSource::KeyboardMouse => key_intents(keys),
            PlayerInputSource::Controller(id) => {
                let Some((_, pad)) = pads.iter().find(|(entity, _)| entity.to_bits() == *id) else {
                    continue;
                };
                let mut held = IntentSet::default();
                for (pressed, intent) in [
                    (
                        pad.pressed(GamepadButton::DPadUp)
                            || pad.get(GamepadAxis::LeftStickY).unwrap_or(0.0) > 0.5,
                        MenuIntent::Up,
                    ),
                    (
                        pad.pressed(GamepadButton::DPadDown)
                            || pad.get(GamepadAxis::LeftStickY).unwrap_or(0.0) < -0.5,
                        MenuIntent::Down,
                    ),
                    (pad.pressed(GamepadButton::South), MenuIntent::Accept),
                    (pad.pressed(GamepadButton::East), MenuIntent::Back),
                ] {
                    if pressed {
                        held = held.with(intent);
                    }
                }
                held
            }
        };
        let actions = destination_source_actions(
            &mut state.destination_inputs,
            *source,
            held,
            now,
            *source == PlayerInputSource::KeyboardMouse && mouse.just_pressed(MouseButton::Right),
        );
        for action in actions {
            let action = match action {
                CatalogueAction::Step(delta) => DestinationAction::Step(delta),
                CatalogueAction::Accept => DestinationAction::Accept,
                CatalogueAction::Back => DestinationAction::Back,
                CatalogueAction::Focus(_) => continue,
            };
            let terminal = matches!(action, DestinationAction::Accept | DestinationAction::Back);
            if crate::input_recording::enabled() {
                let event = match action {
                    DestinationAction::Step(delta) => {
                        Some(tsfp_input::recording::UiEvent::Scroll(0, delta))
                    }
                    DestinationAction::Focus(id) => Some(tsfp_input::recording::UiEvent::Focus(id)),
                    DestinationAction::Accept => Some(tsfp_input::recording::UiEvent::Start),
                    DestinationAction::Back => Some(tsfp_input::recording::UiEvent::Back),
                };
                if let Some(event) = event
                    && let Err(error) =
                        crate::input_recording::record_ui(storage_recording_page(true), 0, 0, event)
                {
                    eprintln!("tsfp-bevy: destination UI recording failed: {error}");
                }
            }
            if let Err(status) =
                crate::storage_live::native_destination_input(page.id, *source, action)
            {
                eprintln!("tsfp-bevy: native destination input error {status:#010x}");
            }
            if terminal {
                // The handler may have changed foreground/page. No other device
                // or old presentation can trigger a second action this frame.
                return;
            }
        }
    }
    if participants
        .inputs()
        .contains(&PlayerInputSource::KeyboardMouse)
        && let Ok(w) = window.single()
        && tsfp_input_intake::focused(w)
    {
        static LAST: Mutex<Option<(f32, f32)>> = Mutex::new(None);
        let picture = crate::app::picture_size();
        let position = ui_cursor(w)
            .and_then(|p| cursor_to_display(p, Vec2::new(w.width(), w.height()), picture))
            .map(|p| (p[0] * picture.x, p[1] * picture.y));
        let moved = {
            let mut last = lock(&LAST);
            let moved = position.is_some() && *last != position;
            *last = position;
            moved
        };
        if let Err(status) = crate::storage_live::destination_input_pointer(
            presentation,
            PlayerInputSource::KeyboardMouse,
            (picture.x, picture.y),
            position,
            moved,
            mouse.just_pressed(MouseButton::Left),
            mouse.just_pressed(MouseButton::Right),
        ) {
            eprintln!("tsfp-bevy: native destination pointer error {status:#010x}");
        }
    }
}

/// Seed a newly admitted source with its current held state so Accept crossing
/// the catalogue/destination boundary cannot activate again before release.
fn destination_source_actions(
    states: &mut Vec<(
        crate::storage_caller::PlayerInputSource,
        IntentSet,
        [u64; 4],
    )>,
    source: crate::storage_caller::PlayerInputSource,
    held: IntentSet,
    now: u64,
    pointer_back: bool,
) -> Vec<crate::storage_caller::CatalogueAction> {
    let index = states
        .iter()
        .position(|(current, _, _)| *current == source)
        .unwrap_or_else(|| {
            states.push((source, held, [now + REPEAT_DELAY_MS; 4]));
            states.len() - 1
        });
    let (_, before, repeat) = &mut states[index];
    let actions = owned_catalogue::actions(held, *before, now, repeat, pointer_back);
    *before = held;
    actions
}

/// Native storage uses window events directly; no legacy command or pad queue.
fn owned_storage_input(
    state: &mut State,
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    pads: &Query<&Gamepad>,
    window: &Query<&Window, With<PrimaryWindow>>,
) {
    use crate::storage_caller::CatalogueAction;
    let page = storage_recording_page(false);
    let replay = crate::input_recording::semantic_replay();
    let recorded = if replay {
        lock(REPLAY_UI_EVENTS.get_or_init(|| Mutex::new(Vec::new()))).clone()
    } else {
        Vec::new()
    };
    let recorded: Vec<_> = recorded
        .into_iter()
        .filter(|event| event.page == page)
        .collect();
    let actions = if replay {
        recorded_storage_actions(&recorded)
    } else {
        let held = pad_intents(pads, key_intents(keys));
        let before = state.page_held;
        state.page_held = held;
        let now = crate::shared::ui_now_ms();
        owned_catalogue::actions(
            held,
            before,
            now,
            &mut state.page_repeat_at,
            mouse.just_pressed(MouseButton::Right),
        )
    };
    if !replay && crate::input_recording::enabled() {
        for action in &actions {
            let event = match action {
                CatalogueAction::Step(delta) => {
                    Some(tsfp_input::recording::UiEvent::Scroll(0, *delta))
                }
                CatalogueAction::Focus(row) => {
                    Some(tsfp_input::recording::UiEvent::Focus(*row as u64))
                }
                CatalogueAction::Accept => Some(tsfp_input::recording::UiEvent::Start),
                CatalogueAction::Back => Some(tsfp_input::recording::UiEvent::Back),
            };
            if let Some(event) = event
                && let Err(error) = crate::input_recording::record_ui(page, 0, 0, event)
            {
                eprintln!("tsfp-bevy: storage UI recording failed: {error}");
            }
        }
    }
    let terminal = actions
        .iter()
        .any(|a| matches!(a, CatalogueAction::Accept | CatalogueAction::Back));
    for action in actions {
        match crate::storage_live::native_catalogue_action(action) {
            Ok(Some(receipt)) => eprintln!(
                "tsfp-bevy: native storage window completion {:?} {:?}",
                receipt.target, receipt.operation
            ),
            Ok(None) => {}
            Err(status) => eprintln!("tsfp-bevy: native storage input error {status:#010x}"),
        }
    }
    if !terminal
        && let Some(presentation) = crate::storage_live::window_presentation()
        && let Ok(w) = window.single()
    {
        static LAST: Mutex<Option<(f32, f32)>> = Mutex::new(None);
        let picture = crate::app::picture_size();
        let position = ui_cursor(w)
            .and_then(|p| cursor_to_display(p, Vec2::new(w.width(), w.height()), picture))
            .map(|p| (p[0] * picture.x, p[1] * picture.y));
        let moved = {
            let mut last = lock(&LAST);
            let moved = position.is_some() && *last != position;
            *last = position;
            moved
        };
        let moved = moved;
        let accept = mouse.just_pressed(MouseButton::Left);
        let back = mouse.just_pressed(MouseButton::Right);
        if (moved || accept || back) && crate::input_recording::enabled() {
            let event = if accept {
                Some(tsfp_input::recording::UiEvent::Start)
            } else if back {
                Some(tsfp_input::recording::UiEvent::Back)
            } else {
                None
            };
            if let Some(event) = event
                && let Err(error) = crate::input_recording::record_ui(page, 0, 0, event)
            {
                eprintln!("tsfp-bevy: storage pointer recording failed: {error}");
            }
        }
        match crate::storage_live::native_catalogue_pointer(
            &presentation,
            (picture.x, picture.y),
            position,
            moved,
            accept,
            back,
        ) {
            Ok(Some(receipt)) => eprintln!(
                "tsfp-bevy: native storage window completion {:?} {:?}",
                receipt.target, receipt.operation
            ),
            Ok(None) => {}
            Err(status) => eprintln!("tsfp-bevy: native storage pointer error {status:#010x}"),
        }
    }
    if let Some(result) = crate::storage_live::native_take_profile_return() {
        // The owned selection controller has released its window/input. The
        // default legacy frontend still owns its own navigation; never pop its
        // guest stack or manufacture a completion callback here.
        let outcome = match result {
            crate::storage_caller::ProfileSelectionReturn::Selected(_) => "selected",
            crate::storage_caller::ProfileSelectionReturn::Cancelled => "cancelled",
        };
        eprintln!("tsfp-bevy: native profile selection return {outcome}");
    }
}

/// Stable identities for the two bespoke storage page families. The runtime
/// catalogue can be opened from several callers; its presentation kind selects
/// the recorded page identity so replay cannot inject into another UI page.
fn storage_recording_page(destination: bool) -> u32 {
    if destination {
        0xFFFF_FF02
    } else {
        0xFFFF_FF01
    }
}

fn recorded_storage_actions(
    events: &[tsfp_input::recording::Ui],
) -> Vec<crate::storage_caller::CatalogueAction> {
    use crate::storage_caller::CatalogueAction;
    events
        .iter()
        .filter_map(|event| match event.event {
            tsfp_input::recording::UiEvent::Scroll(0, delta) => Some(CatalogueAction::Step(delta)),
            tsfp_input::recording::UiEvent::Focus(row) => {
                Some(CatalogueAction::Focus(row as usize))
            }
            tsfp_input::recording::UiEvent::Start => Some(CatalogueAction::Accept),
            tsfp_input::recording::UiEvent::Back => Some(CatalogueAction::Back),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod storage_recording_tests {
    use super::*;
    use tsfp_input::recording::{Ui, UiEvent};

    #[test]
    fn storage_replay_decodes_semantic_actions_and_ignores_other_page_families() {
        let event = |page, event| Ui {
            sim_tick: 4,
            host_poll: 8,
            wall_ns: 10,
            context: 0,
            player: 0,
            page,
            event,
        };
        let rows = vec![
            event(0xFFFF_FF01, UiEvent::Scroll(0, 1)),
            event(0xFFFF_FF01, UiEvent::Start),
            event(0xFFFF_FF02, UiEvent::Back),
            event(0x2C_BFC0, UiEvent::Start),
        ];
        let catalogue: Vec<_> = rows
            .iter()
            .filter(|row| row.page == 0xFFFF_FF01)
            .cloned()
            .collect();
        assert!(matches!(
            recorded_storage_actions(&catalogue).as_slice(),
            [
                crate::storage_caller::CatalogueAction::Step(1),
                crate::storage_caller::CatalogueAction::Accept
            ]
        ));
        let destination: Vec<_> = rows
            .iter()
            .filter(|row| row.page == 0xFFFF_FF02)
            .cloned()
            .collect();
        assert!(matches!(
            recorded_storage_actions(&destination).as_slice(),
            [crate::storage_caller::CatalogueAction::Back]
        ));
    }
}

#[cfg(test)]
mod storage_window_tests {
    use super::*;
    use crate::storage_caller::CatalogueAction;
    #[test]
    fn back_wins_and_held_activation_does_not_repeat() {
        let mut repeat = [0; 4];
        let accept = IntentSet::default().with(MenuIntent::Accept);
        assert!(matches!(
            owned_catalogue::actions(accept, IntentSet::default(), 1, &mut repeat, true).as_slice(),
            [CatalogueAction::Back]
        ));
        assert!(matches!(
            owned_catalogue::actions(accept, IntentSet::default(), 1, &mut repeat, false)
                .as_slice(),
            [CatalogueAction::Accept]
        ));
        assert!(owned_catalogue::actions(accept, accept, 1000, &mut repeat, false).is_empty());
    }

    #[test]
    fn keyboard_arrows_and_enter_reach_owned_catalogue_actions() {
        let mut repeat = [0; 4];
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::ArrowDown);
        assert_eq!(
            owned_catalogue::actions(
                key_intents(&keys),
                IntentSet::default(),
                0,
                &mut repeat,
                false,
            ),
            [CatalogueAction::Step(1)]
        );

        keys.release(KeyCode::ArrowDown);
        keys.clear();
        keys.press(KeyCode::Enter);
        assert_eq!(
            owned_catalogue::actions(
                key_intents(&keys),
                IntentSet::default().with(MenuIntent::Down),
                1,
                &mut repeat,
                false,
            ),
            [CatalogueAction::Accept]
        );
    }

    #[test]
    fn destination_sources_keep_separate_edges_and_swallow_crossing_accept() {
        use crate::storage_caller::PlayerInputSource;
        let keyboard = PlayerInputSource::KeyboardMouse;
        let pad = PlayerInputSource::Controller(17);
        let accept = IntentSet::default().with(MenuIntent::Accept);
        let empty = IntentSet::default();
        let mut states = Vec::new();
        assert!(destination_source_actions(&mut states, keyboard, accept, 0, false).is_empty());
        assert!(destination_source_actions(&mut states, keyboard, accept, 100, false).is_empty());
        assert!(destination_source_actions(&mut states, pad, empty, 100, false).is_empty());
        assert!(matches!(
            destination_source_actions(&mut states, pad, accept, 101, false).as_slice(),
            [CatalogueAction::Accept]
        ));
        assert!(destination_source_actions(&mut states, keyboard, empty, 102, false).is_empty());
        assert!(matches!(
            destination_source_actions(&mut states, keyboard, accept, 103, false).as_slice(),
            [CatalogueAction::Accept]
        ));
        assert!(matches!(
            destination_source_actions(&mut states, keyboard, accept, 104, true).as_slice(),
            [CatalogueAction::Back]
        ));
    }
    #[test]
    fn direction_repeat_uses_bounded_menu_delay() {
        let mut repeat = [0; 4];
        let down = IntentSet::default().with(MenuIntent::Down);
        assert!(matches!(
            owned_catalogue::actions(down, IntentSet::default(), 100, &mut repeat, false)
                .as_slice(),
            [CatalogueAction::Step(1)]
        ));
        assert!(owned_catalogue::actions(down, down, 499, &mut repeat, false).is_empty());
        assert!(matches!(
            owned_catalogue::actions(down, down, 500, &mut repeat, false).as_slice(),
            [CatalogueAction::Step(1)]
        ));
        assert!(owned_catalogue::actions(down, down, 619, &mut repeat, false).is_empty());
    }
}

/// The top live page if it is a native-input page ([`NATIVE_INPUT_PAGES`]).
fn live_native_page() -> Option<(LiveSlot, &'static str)> {
    let top = crate::ui_overlay::top_live_page();
    crate::ui_pages::track_ingame(top.map(|p| p.builder));
    let top = top?;
    let route = native_input_route(top.builder, POPUP_BUILDER.load(Ordering::Acquire));
    let popup = matches!(route, Some(NativeInputRoute::Popup));
    PAGE_POPUP.store(popup, Ordering::Release);
    match route {
        Some(NativeInputRoute::Registered(name)) => Some((top, name)),
        Some(NativeInputRoute::Popup) => Some((top, "popup")),
        None => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeInputRoute {
    Registered(&'static str),
    Popup,
}

fn native_input_route(builder: u32, popup_builder: u32) -> Option<NativeInputRoute> {
    // 0x7A080 is the logical identity assigned by ui_pages to the
    // descriptor-less Select Mode child. It must use popup_over's live page
    // reference at the guest boundary; a builder lookup cannot find that child.
    if builder == tsfp_ui_input::mapmaker_handlers::MODE_CHOICE_BUILDER || builder == popup_builder
    {
        return Some(NativeInputRoute::Popup);
    }
    NATIVE_INPUT_PAGES
        .iter()
        .find(|(page, _)| *page == builder)
        .map(|(_, name)| NativeInputRoute::Registered(name))
        .or_else(|| crate::ui_pages::input_page(builder).map(NativeInputRoute::Registered))
}

fn default_mode_selection_page(previous: u32, current: Option<u32>) -> (u32, bool) {
    match current.filter(|page| *page != 0) {
        Some(page) => (page, previous != page),
        None => (0, false),
    }
}

/// Apply the Map Maker Advanced default as a native page action. The live
/// popup is the authority: `Select(1)` focuses and activates its measured
/// Advanced row through `native_page::service_on`, which calls the original
/// row handler without manufacturing a keyboard or pad event.
pub fn queue_mapmaker_advanced_default(popup_page: Option<u32>) {
    let Some(page) = popup_page else {
        MAPMAKER_ADVANCED_PRESET_PAGE.store(0, Ordering::Release);
        return;
    };
    if page == 0 {
        return;
    }
    let Some(queue) = PAGE_QUEUE.get() else {
        return;
    };
    let Ok(mut pending) = queue.lock() else {
        return;
    };
    let previous = MAPMAKER_ADVANCED_PRESET_PAGE.load(Ordering::Acquire);
    let (next, should_queue) = default_mode_selection_page(previous, Some(page));
    if !should_queue {
        return;
    }
    pending.push_front(NativeCommand::Select(
        tsfp_ui_input::mapmaker_handlers::DEFAULT_MODE_CHOICE_ROW,
    ));
    MAPMAKER_ADVANCED_PRESET_PAGE.store(next, Ordering::Release);
    eprintln!(
        "tsfp-bevy: --ui=rust: Map Maker Advanced default queued native Select({}) for popup {page:#x}",
        tsfp_ui_input::mapmaker_handlers::DEFAULT_MODE_CHOICE_ROW
    );
}

/// An activation or back may replace the page: nothing queues behind it.
fn replaces_page(command: &NativeCommand) -> bool {
    use NativeCommand::{Accept, Back, ResumeGame, Select};
    matches!(command, Select(_) | Accept | Back | ResumeGame)
}

/// T2311: the pointer on a registered page (ui_pages.rs): hover focuses the row under it, left press selects it, right
/// press backs, as native commands on the page queue.
fn registry_pointer(
    mouse: &ButtonInput<MouseButton>,
    window: &Query<&Window, With<PrimaryWindow>>,
    wheel_delta: f32,
) {
    static LAST: Mutex<Option<(f32, f32)>> = Mutex::new(None);
    let pos = window.single().ok().and_then(|w| {
        cursor_to_design(
            ui_cursor(w)?,
            Vec2::new(w.width(), w.height()),
            crate::app::picture_size(),
        )
    });
    let has_text_footer =
        crate::ui_back::enabled() && crate::text_entry_live::footer_page().is_some();
    if has_text_footer && crate::ui_back::pointer(pos, mouse.just_pressed(MouseButton::Left)) {
        crate::text_entry_live::pointer_cancel();
        return;
    }
    if crate::mapmaker_description_live::pointer_active() {
        if let Some((x, y)) = pos {
            crate::mapmaker_description_live::dispatch_pointer(
                x,
                y,
                mouse.just_pressed(MouseButton::Left),
            );
        }
        return;
    }
    if has_text_footer {
        return;
    }
    if !owns_pointer() || active() {
        return;
    }
    if option_list_open() {
        let command = match [MouseButton::Left, MouseButton::Right].map(|b| mouse.just_pressed(b)) {
            [true, _] => Some(NativeCommand::Accept),
            [_, true] => Some(NativeCommand::Back),
            _ => None,
        };
        if let (Some(command), Some(queue)) = (command, PAGE_QUEUE.get()) {
            lock(queue).push_back(command);
        }
        return;
    }
    let Some(current) = crate::ui_pages::current() else {
        return;
    };
    if current.live.builder != PAGE_BUILDER.load(Ordering::Acquire) {
        return;
    }
    let moved = {
        let mut last = lock(&LAST);
        let moved = pos.is_some() && *last != pos;
        *last = pos;
        moved
    };
    if crate::ui_back::pointer(pos, mouse.just_pressed(MouseButton::Left)) {
        if !crate::ui_extensions::command(
            current.descriptor.builder,
            &NativeCommand::Back,
            0,
            current.cursor_nodes.len(),
        ) {
            if let Some(command) = crate::ui_back::command(&current) {
                if let Some(queue) = PAGE_QUEUE.get() {
                    if let Ok(mut q) = queue.lock() {
                        if !q.back().is_some_and(replaces_page) {
                            q.push_back(command);
                        }
                    }
                }
            }
        }
        return;
    }
    if current.mapmaker_storage_progress.is_none()
        && crate::ui_extensions::pointer(
            current.descriptor.builder,
            pos,
            moved,
            mouse.just_pressed(MouseButton::Left),
            mouse.just_pressed(MouseButton::Right),
        )
    {
        return;
    }
    let mut commands = crate::ui_pages::registered_pointer_commands(
        &current,
        pos,
        moved,
        mouse.just_pressed(MouseButton::Left),
        mouse.just_pressed(MouseButton::Right),
    );
    if let Some(progress) = current.mapmaker_storage_progress {
        commands = mapmaker_storage_progress_commands(progress, commands);
    }
    if current.mapmaker_storage_progress.is_none()
        && !commands
            .iter()
            .any(|c| matches!(c, NativeCommand::Select(_) | NativeCommand::Back))
    {
        commands.extend(crate::ui_pages::wheel_commands(wheel_delta));
    }
    if current.mapmaker_storage_progress.is_none() {
        crate::mapmaker_editor_live::apply_commands(&current, &commands);
        crate::mapmaker_controls_ui_live::apply_commands(&current, &commands);
        crate::mapmaker_preview_live::apply_commands(&current, &commands);
        crate::mapmaker_palette_grid_live::apply_commands(&current, &commands);
        crate::mapmaker_assault_options_live::apply_commands(&current, &commands);
        crate::mapmaker_assault_phase_assignment_live::apply_commands(&current, &commands);
        crate::mapmaker_logic_summary_live::apply_commands(&current, &commands);
        crate::mapmaker_action_picker_live::apply_commands(&current, &commands);
        crate::mapmaker_awards_live::apply_commands(&current, &commands);
        crate::mapmaker_award_flags_live::apply_commands(&current, &commands);
        crate::mapmaker_award_table_live::apply_commands(&current, &commands);
        crate::mapmaker_kind_list_live::apply_commands(&current, &commands);
        crate::mapmaker_award_medal_live::apply_commands(&current, &commands);
        crate::mapmaker_object_kind_live::apply_commands(&current, &commands);
        crate::mapmaker_kind_picker_live::apply_commands(&current, &commands);
        crate::mapmaker_selected_entry_live::apply_commands(&current, &commands);
        crate::mapmaker_trigger_selection_live::apply_commands(&current, &commands);
        crate::mapmaker_tool_menu_live::apply_commands(&current, &commands);
        crate::mapmaker_sample_map_menu_live::apply_commands(&current, &commands);
        crate::mapmaker_editor_settings_live::apply_commands(&current, &commands);
        crate::mapmaker_palette_properties_live::apply_commands(&current, &commands);
        crate::mapmaker_settings_live::apply_commands(&current, &commands);
        crate::mapmaker_disabled_modes_live::apply_commands(&current, &commands);
        crate::mapmaker_set_lists_live::apply_commands(&current, &commands);
    }
    if commands.is_empty() {
        return;
    }
    let Some(queue) = PAGE_QUEUE.get() else {
        return;
    };
    if let Ok(mut q) = queue.lock() {
        if q.back().is_some_and(replaces_page) {
            return;
        }
        if commands
            .iter()
            .any(|command| *command == NativeCommand::Back)
            && let Some(progress) = current.mapmaker_storage_progress
            && !crate::storage_live::dismiss_live_mapmaker_storage_progress(progress.id)
        {
            return;
        }
        q.extend(commands);
    }
}

fn back_command(builder: u32, escape: bool) -> NativeCommand {
    if escape && builder == 0x2F_5520 {
        NativeCommand::ResumeGame
    } else {
        NativeCommand::Back
    }
}

/// The measured progress labels are not interactive. Only a typed Back for a
/// completed operation may dismiss Rust state and cross to the live page seam.
fn mapmaker_storage_progress_commands(
    progress: tsfp_mapmaker::ui::StorageProgress,
    commands: Vec<NativeCommand>,
) -> Vec<NativeCommand> {
    if progress.outcome == tsfp_mapmaker::ui::StorageProgressOutcome::Pending
        || !commands
            .iter()
            .any(|command| *command == NativeCommand::Back)
    {
        return Vec::new();
    }
    vec![NativeCommand::Back]
}

/// T2310: keys and pad on a native-input page become native page actions (edge triggered, directions repeat while
/// held). Up/down move the list cursor, left/right the group column and option value, accept activates the cursor
/// row, back runs the page's back path; all through `native_page::service` at the next frame boundary.
fn native_input_page(
    state: &mut State,
    keys: &ButtonInput<KeyCode>,
    pads: &Query<&Gamepad>,
    now: u64,
    replay: Option<Option<tsfp_input::player::PlayerInput>>,
    live_held: Option<IntentSet>,
) {
    let page = live_native_page();
    let replay = if replay.is_some()
        && page.is_some_and(|(live, _)| {
            replay_page_command().is_some_and(|row| row.builder == live.builder)
        }) {
        Some(None)
    } else {
        replay
    };
    crate::ui_extensions::track(page.map_or(0, |(live, _)| live.builder));
    let was = PAGE_NATIVE.swap(page.is_some(), Ordering::AcqRel);
    PAGE_BUILDER.store(page.map_or(0, |(live, _)| live.builder), Ordering::Release);
    let queue = PAGE_QUEUE.get();
    // Capture/replay semantic actions at the shared native page command seam.
    // This covers Map Maker and every registered builder page without
    // duplicating device-specific routing in each page family.
    if std::env::var_os("TSFP_INPUT_REPLAY_HOLD").is_some()
        && let Some(Some(input)) = replay
        && input.pressed != 0
    {
        static LAST_REPLAY_PRESS: Mutex<Option<(u32, u64, u32)>> = Mutex::new(None);
        let key = (
            page.map_or(0, |(live, _)| live.builder),
            input.step,
            input.pressed,
        );
        let mut last = lock(&LAST_REPLAY_PRESS);
        if *last != Some(key) {
            eprintln!(
                "tsfp-bevy: native UI replay press tick={} page={:#x} bits={:#x}",
                input.step, key.0, input.pressed
            );
            *last = Some(key);
        }
    }
    match (page, was) {
        (Some((live, name)), false) => {
            eprintln!(
                "tsfp-bevy: --ui=rust: {name} page {:#x} live, native input",
                live.builder
            );
        }
        (None, true) => {
            profile_ports::clear();
            if let Some(queue) = queue
                && let Ok(mut q) = queue.lock()
            {
                q.clear();
            }
            eprintln!("tsfp-bevy: --ui=rust: native-input page left");
        }
        _ => {}
    }
    if page.is_none() {
        if USER_HIDDEN.load(Ordering::Acquire) {
            state.page_held = IntentSet::default();
            return;
        }
        if crate::mapmaker_description_live::keyboard_grid_active() {
            let held = match replay {
                Some(input) => input
                    .as_ref()
                    .map_or_else(IntentSet::default, replay_menu_intents),
                None => live_held.unwrap_or_else(|| pad_intents(pads, key_intents(keys))),
            };
            let before = state.page_held;
            state.page_held = held;
            crate::mapmaker_description_live::dispatch_keyboard_intents(held, before);
        } else {
            state.page_held = IntentSet::default();
        }
        return;
    }
    let Some(queue) = queue else {
        return;
    };
    autoselect_page_back(queue, now);
    if USER_HIDDEN.load(Ordering::Acquire) {
        return;
    }
    let held = match replay {
        Some(input) => input
            .as_ref()
            .map_or_else(IntentSet::default, replay_menu_intents),
        None => live_held.unwrap_or_else(|| pad_intents(pads, key_intents(keys))),
    };
    let replay_pressed = replay.flatten().map(replay_menu_presses);
    let before = state.page_held;
    state.page_held = held;
    if crate::mapmaker_description_live::keyboard_grid_active() {
        crate::mapmaker_description_live::dispatch_keyboard_intents(held, before);
        return;
    }
    let mut commands =
        native_page_commands(held, before, replay_pressed, now, &mut state.page_repeat_at);
    if replay.is_none() && held.contains(MenuIntent::Back) && !before.contains(MenuIntent::Back) {
        commands.push(back_command(
            PAGE_BUILDER.load(Ordering::Acquire),
            keys.just_pressed(KeyCode::Escape),
        ));
    }
    if replay.is_none() && crate::input_recording::enabled() {
        let builder = PAGE_BUILDER.load(Ordering::Acquire);
        for command in &commands {
            if let Some(event) = command_recorded_event(command)
                && let Err(error) = crate::input_recording::record_ui(builder, 0, 0, event)
            {
                eprintln!("tsfp-bevy: UI input recording failed: {error}");
            }
        }
    }
    commands.extend(crate::ui_extensions::diagnostic(
        PAGE_BUILDER.load(Ordering::Acquire),
        now,
    ));
    commands.extend(crate::ui_back::diagnostic(
        PAGE_BUILDER.load(Ordering::Acquire),
        now,
    ));
    if commands.is_empty() {
        return;
    }
    let mut mapmaker_progress = None;
    if let Some(current) = crate::ui_pages::current().filter(|_| !option_list_open()) {
        let cursor = current
            .focused
            .and_then(|w| {
                current
                    .live
                    .cursor_index(w.0 as usize, &current.cursor_nodes)
            })
            .unwrap_or(0) as usize;
        if current.mapmaker_storage_progress.is_none() {
            crate::mapmaker_editor_live::apply_commands(&current, &commands);
            crate::mapmaker_controls_ui_live::apply_commands(&current, &commands);
            crate::mapmaker_preview_live::apply_commands(&current, &commands);
            crate::mapmaker_palette_grid_live::apply_commands(&current, &commands);
            crate::mapmaker_assault_options_live::apply_commands(&current, &commands);
            crate::mapmaker_assault_phase_assignment_live::apply_commands(&current, &commands);
            crate::mapmaker_logic_summary_live::apply_commands(&current, &commands);
            crate::mapmaker_action_picker_live::apply_commands(&current, &commands);
            crate::mapmaker_award_flags_live::apply_commands(&current, &commands);
            crate::mapmaker_kind_picker_live::apply_commands(&current, &commands);
            crate::mapmaker_object_kind_live::apply_commands(&current, &commands);
            crate::mapmaker_selected_entry_live::apply_commands(&current, &commands);
            crate::mapmaker_trigger_selection_live::apply_commands(&current, &commands);
            crate::mapmaker_tool_menu_live::apply_commands(&current, &commands);
            crate::mapmaker_sample_map_menu_live::apply_commands(&current, &commands);
            crate::mapmaker_editor_settings_live::apply_commands(&current, &commands);
            crate::mapmaker_palette_properties_live::apply_commands(&current, &commands);
            crate::mapmaker_settings_live::apply_commands(&current, &commands);
            crate::mapmaker_disabled_modes_live::apply_commands(&current, &commands);
            crate::mapmaker_set_lists_live::apply_commands(&current, &commands);
            commands.retain(|c| {
                !crate::ui_extensions::command(
                    current.descriptor.builder,
                    c,
                    cursor,
                    current.cursor_nodes.len(),
                )
            });
        }
        if let Some(progress) = current.mapmaker_storage_progress {
            mapmaker_progress = Some(progress);
            commands = mapmaker_storage_progress_commands(progress, commands);
        }
    }
    if let Ok(mut q) = queue.lock() {
        // An activation or back may replace the page: nothing queues behind it.
        if q.back().is_some_and(replaces_page) {
            return;
        }
        if commands
            .iter()
            .any(|command| *command == NativeCommand::Back)
            && let Some(progress) = mapmaker_progress
            && !crate::storage_live::dismiss_live_mapmaker_storage_progress(progress.id)
        {
            return;
        }
        q.extend(commands);
    }
}

/// T2310 diagnostic (headless runs have no working window input): `TSFP_UI_AUTOSELECT=ROW[,back]` selects cursor row
/// ROW natively once the main menu has been live for 2 s (`back`: Back on the next live main menu). Not a variant.
fn autoselect(state: &mut State, now: u64) {
    static SPEC: std::sync::OnceLock<Option<(usize, bool)>> = std::sync::OnceLock::new();
    static LIVE_SINCE: AtomicU64 = AtomicU64::new(0);
    static STEP: AtomicU64 = AtomicU64::new(0);
    let spec = SPEC.get_or_init(|| {
        let raw = std::env::var("TSFP_UI_AUTOSELECT").ok()?;
        let mut parts = raw.split(',');
        let row = parts.next()?.trim().parse().ok()?;
        Some((row, parts.next().is_some_and(|p| p.trim() == "back")))
    });
    let Some((row, back)) = *spec else { return };
    let Some(main) = state.main.as_mut() else {
        return;
    };
    if main.menu.live.is_none() {
        LIVE_SINCE.store(0, Ordering::Relaxed);
        return;
    }
    let since = LIVE_SINCE.load(Ordering::Relaxed);
    if since == 0 {
        LIVE_SINCE.store(now, Ordering::Relaxed);
        return;
    }
    if now < since + 2000 {
        return;
    }
    let step = STEP.load(Ordering::Relaxed);
    let Some(native) = main.menu.native_mut() else {
        return;
    };
    let (action, arg) = match step {
        0 => (ACTION_SELECT, tsfp_ui_input::ActionArg::Index(row)),
        1 if back => (ACTION_BACK, tsfp_ui_input::ActionArg::None),
        _ => return,
    };
    let result = tsfp_ui_input::UiBindings::action(native, action, arg);
    eprintln!("tsfp-bevy: --ui=rust: TSFP_UI_AUTOSELECT step {step}: {result:?}");
    STEP.store(step + 1, Ordering::Relaxed);
    LIVE_SINCE.store(0, Ordering::Relaxed);
}

/// T2310 increment 5 diagnostic: `TSFP_UI_AUTOSELECT=ROW,pageback` queues one native Back on the first native-input
/// page (e.g. Arcade type after ROW 1) once it has been live for 2 s: the framework's native back and page stack pop
/// (`page_stack::pop`) in a live run. Not a variant.
fn autoselect_page_back(queue: &CommandQueue, now: u64) {
    static WANTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    static LIVE_SINCE: AtomicU64 = AtomicU64::new(0);
    static DONE: AtomicBool = AtomicBool::new(false);
    let wanted = *WANTED.get_or_init(|| {
        std::env::var("TSFP_UI_AUTOSELECT")
            .is_ok_and(|raw| raw.split(',').any(|p| p.trim() == "pageback"))
    });
    if !wanted || DONE.load(Ordering::Relaxed) {
        return;
    }
    let since = LIVE_SINCE.load(Ordering::Relaxed);
    if since == 0 {
        LIVE_SINCE.store(now, Ordering::Relaxed);
        return;
    }
    if now < since + 2000 {
        return;
    }
    if let Ok(mut q) = queue.lock() {
        q.push_back(NativeCommand::Back);
        DONE.store(true, Ordering::Relaxed);
        eprintln!("tsfp-bevy: --ui=rust: TSFP_UI_AUTOSELECT pageback: native Back queued");
    }
}

/// Stop line counters.
pub fn report() {
    if crate::input_recording::replaying() {
        let pending = lock(REPLAY_PAGE_COMMANDS.get_or_init(|| Mutex::new(VecDeque::new())));
        if let Some(row) = pending.front() {
            eprintln!(
                "replay-ui: {} queued semantic row(s) remain unapplied; first is poll {} page {:#x} command {:?}",
                pending.len(),
                row.host_poll,
                row.builder,
                row.command
            );
        } else {
            eprintln!("replay-ui: all queued semantic rows were applied");
        }
    }
    if let Some(main) = lock(&STATE).as_ref().and_then(|state| state.main.as_ref()) {
        let s = main.menu.tap_stats();
        eprintln!(
            "tsfp-bevy: --ui=rust: dispatched taps={} confirmed={} unconfirmed={} dropped_goals={}",
            s.taps, s.confirmed, s.unconfirmed, s.dropped_goals
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_native_commands_keep_page_ownership_and_wait_for_acknowledgement() {
        const PROFILE: u32 = 0x194980;
        const STORY: u32 = 0x2cb090;
        let scroll = NativeCommand::Step(Step::Down);
        let accept = NativeCommand::Accept;
        let story_back = NativeCommand::Back;
        let mut pending = ReplayPageCommands::new();
        enqueue_replay_page_commands(&mut pending, 10, PROFILE, [scroll, accept], 0);
        enqueue_replay_page_commands(&mut pending, 11, STORY, [story_back], 0);

        assert_eq!(pending_replay_page_command(&pending, PROFILE), Some(scroll));
        assert_eq!(pending_replay_page_command(&pending, STORY), None);
        assert!(!acknowledge_page_command(&mut pending, PROFILE, accept));
        assert_eq!(pending_replay_page_command(&pending, PROFILE), Some(scroll));
        assert!(acknowledge_page_command(&mut pending, PROFILE, scroll));
        assert_eq!(pending_replay_page_command(&pending, PROFILE), Some(accept));
        assert_eq!(pending_replay_page_command(&pending, STORY), None);
        assert!(acknowledge_page_command(&mut pending, PROFILE, accept));
        assert_eq!(pending_replay_page_command(&pending, PROFILE), None);
        assert_eq!(
            pending_replay_page_command(&pending, STORY),
            Some(story_back)
        );
    }

    #[test]
    fn replay_queues_recorded_page_rows_through_a_page_transition_gap() {
        use tsfp_input::recording::{Ui, UiEvent as E};
        const SETTINGS: u32 = 0x3133E0;
        const EDITOR: u32 = 0x313010;
        let events = [
            Ui {
                sim_tick: 10,
                host_poll: 20,
                wall_ns: 100,
                context: 0,
                player: 0,
                page: SETTINGS,
                event: E::Back,
            },
            Ui {
                sim_tick: 11,
                host_poll: 21,
                wall_ns: 110,
                context: 0,
                player: 0,
                page: EDITOR,
                event: E::Scroll(0, 1),
            },
        ];
        let mut pending = ReplayPageCommands::new();

        assert!(crate::input_recording::native_ui_page_has_handler(SETTINGS));
        assert!(crate::input_recording::native_ui_page_has_handler(EDITOR));
        assert_eq!(enqueue_replay_native_page_events(&mut pending, &events), 2);
        assert_eq!(pending.front().map(|row| row.host_poll), Some(20));
        assert_eq!(
            pending_replay_page_command(&pending, SETTINGS),
            Some(NativeCommand::Back)
        );
        assert_eq!(pending_replay_page_command(&pending, EDITOR), None);
        assert!(acknowledge_page_command(
            &mut pending,
            SETTINGS,
            NativeCommand::Back
        ));
        assert_eq!(
            pending_replay_page_command(&pending, EDITOR),
            Some(NativeCommand::Step(Step::Down))
        );
        assert_eq!(pending.front().map(|row| row.host_poll), Some(21));
    }

    #[test]
    fn native_ui_recording_roundtrips_actions_and_routes_by_page_family() {
        use tsfp_input::recording::{Ui, UiEvent as E};
        let source = [
            NativeCommand::Step(Step::Up),
            NativeCommand::Step(Step::Down),
            NativeCommand::Step(Step::Left),
            NativeCommand::Step(Step::Right),
            NativeCommand::Accept,
            NativeCommand::Back,
            NativeCommand::Adjust {
                row: 7,
                right: false,
            },
            NativeCommand::Adjust {
                row: 3,
                right: true,
            },
        ];
        let pages: Vec<_> = crate::ui_pages::PAGES
            .iter()
            .map(|page| page.builder)
            .chain(NATIVE_INPUT_PAGES.iter().map(|(builder, _)| *builder))
            .chain([0x2E_A8E0, 0x2C_BFC0, 0x2D_62F0])
            .collect();
        for builder in pages.iter().copied() {
            let events: Vec<_> = source
                .iter()
                .map(|command| Ui {
                    sim_tick: 12,
                    host_poll: 4,
                    wall_ns: 10,
                    context: 0,
                    player: 0,
                    page: builder,
                    event: command_recorded_event(command).unwrap(),
                })
                .collect();
            let routed = events_for_page(events.clone(), builder);
            assert_eq!(
                recorded_native_commands(&routed),
                source,
                "page {builder:#x}"
            );
            assert!(
                events_for_page(events, builder.wrapping_add(1)).is_empty(),
                "neighbor page must not receive events from {builder:#x}"
            );
        }
        let legacy = Ui {
            page: 0,
            event: E::Back,
            sim_tick: 12,
            host_poll: 4,
            wall_ns: 10,
            context: 0,
            player: 0,
        };
        assert!(
            events_for_page(vec![legacy], pages[0]).is_empty(),
            "legacy untagged events must not be delivered to an arbitrary page"
        );
    }

    #[test]
    fn recording_codec_preserves_bespoke_editor_and_text_events_per_page() {
        use tsfp_input::recording::{Ui, UiEvent as E};
        // These events are represented by UiInput but currently have no native
        // handler adapter on every page. Keep their exact payloads and page key
        // intact in the recording stream so later adapters cannot silently
        // flatten them into generic Accept/Back commands.
        let cases = [
            (0x2c_8940, E::TextInput('A')),
            (
                0x2c_8940,
                E::TextCommit {
                    widget: 17,
                    text: b"Map".to_vec(),
                },
            ),
            (0x2c_8940, E::TextCancel(17)),
            (0x2d_62f0, E::Activate(42)),
            (0x2d_62f0, E::Focus(3)),
            (0x2d_62f0, E::Scroll(0, 1)),
            (0x2c_bfc0, E::Hover(9)),
            (0x2c_bfc0, E::HoverLeave),
        ];
        for (page, event) in cases {
            let source = Ui {
                sim_tick: 12,
                host_poll: 4,
                wall_ns: 10,
                context: 0,
                player: 0,
                page,
                event: event.clone(),
            };
            assert_eq!(
                events_for_page(vec![source.clone()], page),
                vec![source.clone()]
            );
            assert!(events_for_page(vec![source], page.wrapping_add(1)).is_empty());
        }
    }

    #[test]
    fn cursor_maps_through_the_letterbox() {
        // 1280x720 window, 4:3 picture 960x720 centred: the picture's centre is design (320, 240).
        let w = Vec2::new(1280.0, 720.0);
        let p = Vec2::new(960.0, 720.0);
        assert_eq!(
            cursor_to_design(Vec2::new(640.0, 360.0), w, p),
            Some((320.0, 240.0))
        );
        assert_eq!(
            cursor_to_design(Vec2::new(160.0, 0.0), w, p),
            Some((0.0, 0.0))
        );
        assert_eq!(cursor_to_design(Vec2::ZERO, w, Vec2::ZERO), None);
    }

    #[test]
    fn replayed_player_buttons_become_native_page_intents() {
        let input = tsfp_input::player::PlayerInput {
            step: 1,
            slot: 0,
            source: tsfp_input::player::SourceKind::Replay,
            held: tsfp_input::player::bits::A | tsfp_input::player::bits::DPAD_UP,
            pressed: 0,
            released: 0,
            move_axis: tsfp_input::player::Vec2::new(0.0, 0.0),
            look: tsfp_input::player::Look::Stick(tsfp_input::player::Vec2::new(0.0, 0.0)),
            triggers: [0.0; 2],
            menu: tsfp_input::player::MenuIntent {
                accept: false,
                back: true,
            },
            assists: tsfp_input::player::AssistMask::NONE,
        };
        let held = replay_menu_intents(&input);
        assert!(held.contains(MenuIntent::Accept));
        assert!(held.contains(MenuIntent::Up));
        assert!(held.contains(MenuIntent::Back));
    }

    #[test]
    fn sparse_legacy_replay_enters_the_rust_main_menu_through_ui_input() {
        use tsfp_input::player::{AssistMask, Look, PlayerInput, SourceKind, Vec2, bits};
        use tsfp_ui_input::{HitTarget, Rect, UiEvent, UiLayout, WidgetId};

        struct Target(WidgetId, Rect);
        impl HitTarget for Target {
            fn id(&self) -> WidgetId {
                self.0
            }
            fn rect(&self) -> Rect {
                self.1
            }
        }
        struct Layout([Target; 2]);
        impl UiLayout for Layout {
            fn visit(&self, f: &mut dyn FnMut(&dyn HitTarget)) {
                f(&self.0[0]);
                f(&self.0[1]);
            }
        }

        let layout = Layout([
            Target(WidgetId(1), Rect::new(0.0, 0.0, 80.0, 20.0)),
            Target(WidgetId(2), Rect::new(0.0, 40.0, 80.0, 20.0)),
        ]);
        let mut model = UiInput::new();
        model.open_context(ContextId(0), Ownership::Any);
        model.set_focus(ContextId(0), Some(WidgetId(1)));
        let recorded = PlayerInput {
            step: 10,
            slot: 0,
            source: SourceKind::Replay,
            held: bits::DPAD_DOWN | bits::START,
            pressed: bits::DPAD_DOWN | bits::START,
            released: 0,
            move_axis: Vec2::ZERO,
            look: Look::Stick(Vec2::ZERO),
            triggers: [0.0; 2],
            menu: tsfp_input::player::MenuIntent::default(),
            assists: AssistMask::NONE,
        };
        let frame = replay_main_menu_player_frame(&recorded);
        let mut routed = Vec::new();
        model.update(ContextId(0), 0, &layout, &[frame], None, &mut routed);

        assert!(
            routed
                .iter()
                .any(|event| matches!(&event.event, UiEvent::Focus(WidgetId(2))))
        );
        assert!(
            routed
                .iter()
                .any(|event| matches!(&event.event, UiEvent::Start))
        );
    }

    #[test]
    fn native_page_replay_prefers_matching_ui_and_falls_back_to_tick_input() {
        let input = tsfp_input::player::PlayerInput {
            step: 1,
            slot: 0,
            source: tsfp_input::player::SourceKind::Replay,
            held: tsfp_input::player::bits::A,
            pressed: 0,
            released: 0,
            move_axis: tsfp_input::player::Vec2::new(0.0, 0.0),
            look: tsfp_input::player::Look::Stick(tsfp_input::player::Vec2::new(0.0, 0.0)),
            triggers: [0.0; 2],
            menu: tsfp_input::player::MenuIntent {
                accept: false,
                back: false,
            },
            assists: tsfp_input::player::AssistMask::NONE,
        };
        assert_eq!(
            replay_input_for_page(Some(input), Some(0x194980), true),
            None
        );
        assert_eq!(
            replay_input_for_page(Some(input), Some(0x194980), false),
            Some(input),
            "unmatched T snapshots remain available to the native handler"
        );
        assert_eq!(replay_input_for_page(Some(input), None, false), Some(input));
    }

    #[test]
    fn replayed_press_survives_a_page_transition_while_button_is_held() {
        let accept = IntentSet::default().with(MenuIntent::Accept);
        let mut repeat_at = [0; 4];
        assert!(matches!(
            native_page_commands(accept, accept, Some(accept), 10, &mut repeat_at).as_slice(),
            [NativeCommand::Accept]
        ));
        assert!(
            native_page_commands(
                accept,
                accept,
                Some(IntentSet::default()),
                11,
                &mut repeat_at,
            )
            .is_empty()
        );
    }
}

#[cfg(test)]
mod keyboard_standard_tests {
    use super::*;
    #[test]
    fn a_tap_released_between_frames_still_selects() {
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::Enter);
        keys.release(KeyCode::Enter);
        assert_eq!(
            key_intents(&keys),
            IntentSet::default().with(MenuIntent::Accept)
        );
        keys.clear();
        assert_eq!(key_intents(&keys), IntentSet::default());
    }
    #[test]
    fn pause_root_escape_resumes_but_subpages_and_pad_back_navigate() {
        for page in crate::ui_pages::PAGES {
            assert_eq!(
                back_command(page.builder, true),
                if page.builder == 0x2F_5520 {
                    NativeCommand::ResumeGame
                } else {
                    NativeCommand::Back
                }
            );
            assert_eq!(back_command(page.builder, false), NativeCommand::Back);
        }
    }

    #[test]
    fn registered_native_popup_pause_and_mapmaker_pages_share_menu_keys() {
        for name in crate::ui_pages::PAGES
            .iter()
            .map(|p| p.name)
            .chain(NATIVE_INPUT_PAGES.iter().map(|(_, name)| *name))
            .chain(["unregistered fallback", "confirm popup", "option popup"])
        {
            for key in [KeyCode::Enter, KeyCode::NumpadEnter] {
                let mut keys = ButtonInput::default();
                keys.press(key);
                assert_eq!(
                    key_intents(&keys),
                    IntentSet::default().with(MenuIntent::Accept),
                    "{name}"
                );
            }
            let mut keys = ButtonInput::default();
            keys.press(KeyCode::Escape);
            assert_eq!(
                key_intents(&keys),
                IntentSet::default().with(MenuIntent::Back),
                "{name}"
            );
            keys.clear();
            keys.release(KeyCode::Escape);
            keys.press(KeyCode::Space);
            assert_eq!(key_intents(&keys), IntentSet::default(), "{name}");
        }
    }
}
