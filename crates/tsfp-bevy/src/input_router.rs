// SPDX-License-Identifier: GPL-3.0-or-later
//! Bevy device glue for the opt-in native `tsfp-input` router. Mouse, keyboard and pads map back to guest input events.
use crate::shared::{SHARED, Wi, lock};
use crate::window_input::RawEvent;
use bevy::input::ButtonState;
use bevy::input::gamepad::{
    GamepadAxis, GamepadButton, GamepadConnection, GamepadRumbleIntensity, GamepadRumbleRequest,
    RawGamepadEvent,
};
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::{MouseButton, MouseButtonInput, MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tsfp_input::bindings::Bindings;
use tsfp_input::capture::{Capture, RELEASE_HELP};
use tsfp_input::controller::ControllerEventPath;
use tsfp_input::game_side::GameInputBoundary;
use tsfp_input::guard::{GuardEvent, HostProbe, PadFilter, RawLookGuard};
use tsfp_input::player::{Look, PlayerInput, RumbleEffect, SourceKind};
use tsfp_input::router::{
    DeviceEvent, InputSource, MouseButtonId, PadAxis, PadButton, Router, RouterConfig,
};
use tsfp_input::shim::ShimEvent;

pub const DEFAULT_BINDINGS: &str = "tmp/input-bindings.toml";

#[derive(Clone, Debug)]
pub struct Options {
    pub mouse_keyboard: bool,
    /// T2161: raw look mailbox plus assist disable for the mouse and keyboard slot (default on with `--mouse-keyboard`).
    pub raw_look: bool,
    pub bindings: Bindings,
    /// T2167: presets, per player settings and action maps (default: every slot on `original`).
    pub config: tsfp_input::presets::InputConfig,
    /// T2200: which enumerated devices are not gamepads (`--ignore-pad`, `--no-pad-filter`, env `TSFP_PAD_IGNORE`).
    pub pad_filter: PadFilterArgs,
    /// T2348: native fixed-tick recording path; off by default.
    pub record_path: Option<std::path::PathBuf>,
    /// T2348: replay of typed native input; mutually exclusive with recording.
    pub replay_path: Option<std::path::PathBuf>,
    /// T2436: continue with live Rust input after a native replay ends.
    pub replay_handover: bool,
}

/// Shell arguments of the gamepad ignore list.
#[derive(Clone, Debug)]
pub struct PadFilterArgs {
    pub defaults: bool,
    pub extra: Vec<String>,
}

impl Default for PadFilterArgs {
    fn default() -> Self {
        Self {
            defaults: true,
            extra: Vec::new(),
        }
    }
}

static OPTIONS: OnceLock<Options> = OnceLock::new();
/// T3771: the per slot mailboxes live in `tsfp-input-slots`; these re-exports keep the paths.
pub(crate) use tsfp_input_slots::hud::message_buttons as hud_message_buttons;

pub fn options() -> Option<&'static Options> {
    OPTIONS.get()
}

/// Take the shell's input options out of the arguments. `None` when `--input-router` is absent.
pub fn extract(arguments: &mut Vec<String>) -> Result<Option<Options>, String> {
    let mut router = false;
    let mut mouse_keyboard = false;
    let mut raw_look: Option<bool> = None;
    let mut path: Option<String> = None;
    let mut preset_args: Vec<String> = Vec::new();
    let mut ignore_pads: Vec<String> = Vec::new();
    let mut no_pad_filter = false;
    let mut record_path: Option<std::path::PathBuf> = None;
    let mut replay_path: Option<std::path::PathBuf> = None;
    let mut replay_handover = false;
    let mut at = 0;
    while at < arguments.len() {
        match arguments[at].as_str() {
            "--input-router" => {
                router = true;
                arguments.remove(at);
            }
            "--mouse-keyboard" => {
                mouse_keyboard = true;
                arguments.remove(at);
            }
            "--raw-look" => {
                raw_look = Some(true);
                arguments.remove(at);
            }
            "--no-raw-look" => {
                raw_look = Some(false);
                arguments.remove(at);
            }
            "--no-pad-filter" => {
                no_pad_filter = true;
                arguments.remove(at);
            }
            "--ignore-pad" => {
                if at + 1 >= arguments.len() {
                    return Err("--ignore-pad needs a device name fragment".into());
                }
                ignore_pads.push(arguments.remove(at + 1));
                arguments.remove(at);
            }
            "--input-bindings" => {
                if at + 1 >= arguments.len() {
                    return Err("--input-bindings needs a path".into());
                }
                path = Some(arguments.remove(at + 1));
                arguments.remove(at);
            }
            "--input-record" => {
                if at + 1 >= arguments.len() {
                    return Err("--input-record needs a path".into());
                }
                record_path = Some(arguments.remove(at + 1).into());
                arguments.remove(at);
            }
            "--input-replay" => {
                if at + 1 >= arguments.len() {
                    return Err("--input-replay needs a path".into());
                }
                replay_path = Some(arguments.remove(at + 1).into());
                arguments.remove(at);
            }
            "--input-replay-handover" => {
                replay_handover = true;
                arguments.remove(at);
            }
            "--input-preset" => {
                if at + 1 >= arguments.len() {
                    return Err("--input-preset needs NAME[:slot]".into());
                }
                preset_args.push(arguments.remove(at + 1));
                arguments.remove(at);
            }
            _ => at += 1,
        }
    }
    if !router {
        if mouse_keyboard
            || path.is_some()
            || !preset_args.is_empty()
            || raw_look.is_some()
            || no_pad_filter
            || !ignore_pads.is_empty()
            || record_path.is_some()
            || replay_path.is_some()
        {
            return Err("--mouse-keyboard, --raw-look, --input-bindings, --input-record, --input-replay, --input-preset, --ignore-pad and --no-pad-filter need --input-router".into());
        }
        return Ok(None);
    }
    if record_path.is_some() && replay_path.is_some() {
        return Err("--input-record and --input-replay are mutually exclusive".into());
    }
    if replay_handover && replay_path.is_none() {
        return Err("--input-replay-handover needs --input-replay".into());
    }
    let mut config = match &path {
        Some(path) => tsfp_input::presets::InputConfig::load(path, true)?,
        None => tsfp_input::presets::InputConfig::load(DEFAULT_BINDINGS, false)?,
    };
    for text in &preset_args {
        let (slot, name) = tsfp_input::presets::parse_selection(text, &config.presets)?;
        config.select(slot, &name);
    }
    let bindings: Bindings = config.bindings.clone();
    if raw_look == Some(true) && !mouse_keyboard {
        return Err("--raw-look needs --mouse-keyboard".into());
    }
    Ok(Some(Options {
        mouse_keyboard,
        raw_look: raw_look.unwrap_or(mouse_keyboard),
        bindings,
        config,
        pad_filter: PadFilterArgs {
            defaults: !no_pad_filter,
            extra: ignore_pads,
        },
        record_path,
        replay_path,
        replay_handover,
    }))
}

pub fn set_options(options: Options) {
    let _ = OPTIONS.set(options);
}

pub fn banner(options: &Options) -> String {
    format!(
        "tsfp-bevy: --input-router (EXPERIMENTAL, T2160): {}. {RELEASE_HELP}. Escape is the pad's START (pause toggle) and Backspace its B (back), Alt+F4 quits. \
         Pad users: unchanged queue stream (shim). {}",
        if options.mouse_keyboard {
            "player 1 on mouse and keyboard (WASD, mouse buttons, wheel, 1-9 weapon slots), a pad used later takes over"
        } else {
            "pads only"
        },
        if options.raw_look {
            "Raw mouse look (T2161, TEMPORARY-SHIM until T2194): counts times degrees per count straight into yaw and pitch, Auto Aim, Auto Lookahead and Inverse Look off for the mouse player only (needs the player roots in Rust: --rust-ports=all,player; without them it falls back to the interim stick after 3 s and says so)."
        } else {
            "Mouse look is the interim virtual right stick (--raw-look for the raw path)."
        }
    )
}

/// Rumble requested by the guest side (slot, effect); drained by `rumble_system`. Nothing produces it yet (T2163 owns the word).
static RUMBLE: Mutex<Vec<(u8, RumbleEffect)>> = Mutex::new(Vec::new());

pub fn push_rumble(slot: u8, effect: RumbleEffect) {
    if let Ok(mut queue) = RUMBLE.lock() {
        queue.push((slot, effect));
    }
}

#[derive(Resource)]
pub struct RouterRes {
    pub router: Router,
    /// Opt-in, profile-aware physical controller reports for the Bevy gamepad source.
    pub controllers: ControllerEventPath,
    pub raw_look: bool,
    pub capture: Capture,
    pub focused: bool,
    grabbed: bool,
    /// T2167: one shaper per slot built from the selected preset. Not consumed yet: the guest still shapes the pad stream
    /// (the `original` preset is what it does), so selecting `original` or nothing changes no behaviour.
    #[allow(dead_code)]
    pub shapers: Vec<tsfp_input::shaping::Shaper>,
    /// T2200: falls back to the interim virtual stick when the raw look hook is not reached.
    pub guard: RawLookGuard,
    /// T2200: devices that are enumerated as gamepads but are not (keyboard consumer control nodes).
    pub filter: PadFilter,
    started: Instant,
    stats: LookStats,
    owners: [Option<InputSource>; 4],
    last_recorded_tick: u64,
}

/// T2200: what the shell saw of the mouse since the last diagnostics line (printed with `--perf-log` or `TSFP_INPUT_DEBUG=1`).
#[derive(Clone, Debug)]
pub struct LookStats {
    /// Bevy `MouseMotion` events and the sum of |dx| + |dy| counts received, all and those dropped (released pointer, no focus).
    pub motion_events: u64,
    pub counts: f64,
    pub dropped_events: u64,
    /// Ticks that handed look degrees to the mailbox.
    pub pushes: u64,
    last_log: Instant,
    last_events: u64,
    last_counts: f64,
    last_pushes: u64,
}

impl LookStats {
    fn new() -> Self {
        Self {
            motion_events: 0,
            counts: 0.0,
            dropped_events: 0,
            pushes: 0,
            last_log: Instant::now(),
            last_events: 0,
            last_counts: 0.0,
            last_pushes: 0,
        }
    }
}

static DIAG: OnceLock<bool> = OnceLock::new();

/// `--perf-log` on the command line (set by main) or `TSFP_INPUT_DEBUG=1`.
pub fn set_diag(perf_log: bool) {
    let _ = DIAG.set(perf_log || std::env::var("TSFP_INPUT_DEBUG").is_ok_and(|value| value == "1"));
}

fn diag_enabled() -> bool {
    DIAG.get().copied().unwrap_or(false)
}

/// Gamepad ignore list from the options plus `TSFP_PAD_IGNORE` (comma separated name fragments).
fn pad_filter(args: &PadFilterArgs) -> PadFilter {
    let mut extra = args.extra.clone();
    if let Ok(value) = std::env::var("TSFP_PAD_IGNORE") {
        extra.extend(value.split(',').map(str::to_string));
    }
    PadFilter::new(args.defaults, &extra)
}

impl RouterRes {
    pub fn new(options: &Options) -> Self {
        crate::ui_extensions::initialise(&options.bindings, options.mouse_keyboard);
        let config = RouterConfig {
            bindings: options.bindings.clone(),
            mouse_keyboard_slot: options.mouse_keyboard.then_some(0),
            raw_look: options.raw_look,
            ..RouterConfig::default()
        };
        let shapers = (0..tsfp_input::presets::MAX_SLOTS as u8)
            .map(|slot| options.config.shaper(slot))
            .collect();
        let mut router = Router::new(config);
        if let Some(settings) = crate::ui_extensions::settings() {
            router.set_player_bindings(&settings);
        }
        let mut controllers = ControllerEventPath::default();
        tsfp_input_intake::controller_storage::load(&mut controllers).unwrap_or_else(|error| {
            eprintln!("input router: cannot load controller profiles: {error}")
        });
        Self {
            router,
            controllers,
            raw_look: options.raw_look,
            capture: Capture::default(),
            focused: true,
            grabbed: false,
            shapers,
            guard: RawLookGuard::new(),
            filter: pad_filter(&options.pad_filter),
            started: Instant::now(),
            stats: LookStats::new(),
            owners: [None; 4],
            last_recorded_tick: 0,
        }
    }

    pub fn key(&mut self, key: &KeyboardInput) {
        if key.repeat {
            return;
        }
        let name = format!("{:?}", key.key_code);
        let down = key.state == ButtonState::Pressed;
        if down && crate::ui_extensions::capture_input(tsfp_input::settings::InputKind::Key, &name)
            || crate::input_recording::replaying()
        {
            return; // T3820: a replay's router input is its `T` rows
        }
        self.capture.key(&name, down);
        if name != "F10" {
            self.router.feed(DeviceEvent::Key { name, down });
        }
    }

    pub fn focus_lost(&mut self) {
        self.router.feed(DeviceEvent::FocusLost);
        self.capture.focus_lost();
    }

    pub fn wants_grab(&self) -> bool {
        self.capture
            .wants_grab(self.router.mouse_keyboard_active(), self.focused)
    }
}

pub fn pad_button(button: GamepadButton) -> Option<PadButton> {
    Some(match button {
        GamepadButton::South => PadButton::South,
        GamepadButton::East => PadButton::East,
        GamepadButton::West => PadButton::West,
        GamepadButton::North => PadButton::North,
        GamepadButton::Select => PadButton::Select,
        GamepadButton::Mode => PadButton::Mode,
        GamepadButton::Start => PadButton::Start,
        GamepadButton::LeftThumb => PadButton::LeftThumb,
        GamepadButton::RightThumb => PadButton::RightThumb,
        GamepadButton::LeftTrigger => PadButton::LeftBumper,
        GamepadButton::RightTrigger => PadButton::RightBumper,
        GamepadButton::DPadUp => PadButton::DPadUp,
        GamepadButton::DPadDown => PadButton::DPadDown,
        GamepadButton::DPadLeft => PadButton::DPadLeft,
        GamepadButton::DPadRight => PadButton::DPadRight,
        _ => return None,
    })
}

/// A raw Bevy gamepad event as router device events and the bookkeeping the queue needs (pads open/closed).
pub fn route_pad_event(
    router: &mut Router,
    wi: &mut Wi,
    controllers: &mut ControllerEventPath,
    event: &RawGamepadEvent,
) {
    use tsfp_input_intake::controller_route::{ConnectionChange, route};
    match route(router, controllers, event) {
        ConnectionChange::Connected(id) => {
            wi.pads.insert(id);
            wi.feed(RawEvent::GamepadAdded(id));
        }
        ConnectionChange::Disconnected(id) => {
            wi.pads.remove(&id);
            wi.feed(RawEvent::GamepadRemoved(id));
        }
        ConnectionChange::None => {}
    }
    let (devices, profiles) = controllers.settings_snapshot();
    crate::ui_extensions::update_controller_settings(devices, profiles);
}

/// Router events of player 1 (slot 0) into the queue.
pub fn flush_to_queue(router: &mut Router, wi: &mut Wi) {
    for (slot, event) in router.drain_events() {
        if slot != 0 {
            continue;
        }
        wi.feed(match event {
            ShimEvent::ButtonDown(button) => RawEvent::button_down(None, button),
            ShimEvent::ButtonUp(button) => RawEvent::button_up(None, button),
            ShimEvent::Axis { axis, value } => RawEvent::axis(None, axis, value),
        });
    }
}

pub fn pad_events(mut raw: EventReader<RawGamepadEvent>, mut res: ResMut<RouterRes>) {
    let res = &mut *res;
    let mut wi = lock(&SHARED.wi);
    tsfp_input_intake::controller_route::process_pending_profile_edit(
        crate::input_recording::replaying(),
        crate::ui_extensions::take_controller_profile_edit,
        |edit| {
            if let Err(error) =
                res.controllers
                    .set_profile(&mut res.router, edit.identity, edit.profile)
            {
                eprintln!("input router: cannot apply controller profile edit: {error}");
            } else {
                tsfp_input_intake::controller_storage::persist(&res.controllers).unwrap_or_else(
                    |error| eprintln!("input router: cannot persist controller profiles: {error}"),
                );
                let (devices, profiles) = res.controllers.settings_snapshot();
                crate::ui_extensions::update_controller_settings(devices, profiles);
            }
        },
    );
    for event in raw.read() {
        if crate::input_recording::replaying() {
            continue;
        }
        // T2188/T2168: the Rust main menu owns the pad (it feeds dispatched taps); releases still pass.
        let held_back = crate::ui_live::capturing()
            && match event {
                RawGamepadEvent::Button(change) => {
                    crate::ui_live::holds_pad_button(change.button) && change.value > 0.5
                }
                RawGamepadEvent::Axis(change) => change.value.abs() > 0.3,
                RawGamepadEvent::Connection(_) => false,
            };
        if held_back {
            continue;
        }
        if admit_pad_event(&mut res.filter, event) {
            route_pad_event(&mut res.router, &mut wi, &mut res.controllers, event);
        }
    }
}

/// T2200: log every gamepad that appears and drop the events of devices that are not gamepads (a keyboard's consumer control
/// node enumerated as a joystick would otherwise count as pad activity and take the player from the mouse and keyboard).
pub fn admit_pad_event(filter: &mut PadFilter, event: &RawGamepadEvent) -> bool {
    match event {
        RawGamepadEvent::Connection(connection) => {
            let id = connection.gamepad.index();
            match &connection.connection {
                GamepadConnection::Connected {
                    name,
                    vendor_id,
                    product_id,
                } => {
                    let ids = format!(
                        "{:04x}:{:04x}",
                        vendor_id.unwrap_or(0),
                        product_id.unwrap_or(0)
                    );
                    match filter.connect(id, name) {
                        Ok(()) => {
                            eprintln!(
                                "tsfp-bevy: gamepad {id} '{name}' ({ids}) connected, routed to the players (--ignore-pad FRAGMENT excludes a device)"
                            );
                            true
                        }
                        Err(fragment) => {
                            eprintln!(
                                "tsfp-bevy: gamepad {id} '{name}' ({ids}) IGNORED, not a gamepad (name contains '{fragment}'; --no-pad-filter keeps every device)"
                            );
                            false
                        }
                    }
                }
                GamepadConnection::Disconnected => {
                    let ignored = filter.is_ignored(id);
                    eprintln!(
                        "tsfp-bevy: gamepad {id} '{}' disconnected{}",
                        filter.name(id).unwrap_or("?"),
                        if ignored { " (was ignored)" } else { "" }
                    );
                    filter.disconnect(id);
                    !ignored
                }
            }
        }
        RawGamepadEvent::Button(change) => !filter.is_ignored(change.gamepad.index()),
        RawGamepadEvent::Axis(change) => !filter.is_ignored(change.gamepad.index()),
    }
}

fn mouse_button(button: MouseButton) -> Option<MouseButtonId> {
    Some(match button {
        MouseButton::Left => MouseButtonId::Left,
        MouseButton::Right => MouseButtonId::Right,
        MouseButton::Middle => MouseButtonId::Middle,
        MouseButton::Back => MouseButtonId::Back,
        MouseButton::Forward => MouseButtonId::Forward,
        MouseButton::Other(_) => return None,
    })
}

pub fn mouse_events(
    mut motion: EventReader<MouseMotion>,
    mut buttons: EventReader<MouseButtonInput>,
    mut wheel: EventReader<MouseWheel>,
    mut res: ResMut<RouterRes>,
) {
    let res = &mut *res;
    if crate::input_recording::semantic_replay() {
        motion.read().for_each(drop);
        buttons.read().for_each(drop);
        wheel.read().for_each(drop);
        return;
    }
    // T3820: a device replay routes recorded buttons to the pages; the router itself takes its `T` rows.
    let replay = crate::input_recording::replaying();
    // T2168: while a Rust UI page is live the pointer is the menu pointer, the mk player gets no motion, presses or wheel.
    let live =
        !replay && res.focused && !res.capture.is_released() && !crate::ui_live::owns_pointer();
    for event in motion.read() {
        res.stats.motion_events += 1;
        res.stats.counts += f64::from(event.delta.x.abs() + event.delta.y.abs());
        if !live {
            res.stats.dropped_events += 1;
        }
        if live {
            res.router.feed(DeviceEvent::MouseMotion {
                dx: f64::from(event.delta.x),
                dy: f64::from(event.delta.y),
            });
        }
    }
    for event in buttons.read() {
        let down = event.state == ButtonState::Pressed;
        let route = if down
            && mouse_button(event.button).is_some_and(|button| {
                crate::ui_extensions::capture_input(
                    tsfp_input::settings::InputKind::Mouse,
                    button.name(),
                )
            }) {
            "capture"
        } else if down && crate::ui_live::owns_pointer() {
            "ui-pointer"
        } else if crate::events::ui_mouse_fallback(event.button, down) {
            "ui-fallback"
        } else if down
            && !replay
            && !crate::ui_live::owns_pointer()
            && res.capture.is_released()
            && res.focused
            && res.capture.click()
        {
            "pointer-retake" // the click that retakes the pointer is not a shot
        } else {
            if let (true, Some(button)) = (!replay && (live || !down), mouse_button(event.button)) {
                res.router.feed(DeviceEvent::MouseButton { button, down });
            }
            "router"
        };
        tsfp_input_intake::route(tsfp_input_intake::RouteKind::MouseButton, route);
    }
    for event in wheel.read() {
        let notches = match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y / 40.0,
        };
        if live {
            res.router.feed(DeviceEvent::Wheel {
                notches: f64::from(notches),
            });
        }
    }
}

#[cfg(not(tsfp_no_host))]
unsafe extern "C" {
    // crates/tsfp-ports/src/raw_look.rs, linked into the C host archive.
    fn tsfp_raw_look_enable(mask: u32);
    fn tsfp_raw_look_push(slot: u32, yaw_deg: f64, pitch_deg: f64);
    fn tsfp_raw_look_reachable() -> bool;
    fn tsfp_raw_look_diag(out: *mut u32);
}
#[cfg(tsfp_no_host)]
unsafe fn tsfp_raw_look_enable(_mask: u32) {}
#[cfg(tsfp_no_host)]
unsafe fn tsfp_raw_look_push(_slot: u32, _yaw_deg: f64, _pitch_deg: f64) {}
#[cfg(tsfp_no_host)]
unsafe fn tsfp_raw_look_reachable() -> bool {
    true
}
#[cfg(tsfp_no_host)]
unsafe fn tsfp_raw_look_diag(out: *mut u32) {
    unsafe { std::ptr::write_bytes(out, 0, 6) }
}

/// Host side counters of the raw look hook: `[enable calls, look root calls, kept original path, pushes, applied, overlaid scopes]`.
fn host_counters() -> (HostProbe, [u32; 7]) {
    let mut counters = [0u32; 7];
    // SAFETY: plain atomics on the ports side, `counters` has the six u32 the call writes.
    let reachable = unsafe {
        tsfp_raw_look_diag(counters.as_mut_ptr());
        tsfp_raw_look_reachable()
    };
    (
        HostProbe {
            reachable,
            hits: counters[1],
        },
        counters,
    )
}

/// Slots whose raw look and assist disable are active this tick (mouse and keyboard source) and the look degrees to hand over.
pub fn raw_look_plan(inputs: &[Option<PlayerInput>]) -> (u32, Vec<(u32, f64, f64)>) {
    let mut mask = 0u32;
    let mut pushes = Vec::new();
    for (slot, input) in inputs.iter().enumerate() {
        let Some(input) = input else { continue };
        if input.source != SourceKind::MouseKeyboard {
            continue;
        }
        mask |= 1 << slot;
        if let Look::Delta { yaw_deg, pitch_deg } = input.look {
            if yaw_deg != 0.0 || pitch_deg != 0.0 {
                pushes.push((slot as u32, f64::from(yaw_deg), f64::from(pitch_deg)));
            }
        }
    }
    (mask, pushes)
}

/// `deliver` false: observe hook calls for recovery, but leave the interim stick and assist flags untouched.
fn raw_look_feed(inputs: &[Option<PlayerInput>], deliver: bool) {
    let (mask, mut pushes) = raw_look_plan(inputs);
    if !deliver {
        pushes.clear();
    }
    // SAFETY: plain atomics on the ports side; the enable call comes first so a slot leaving the mouse drains its mailbox.
    unsafe {
        tsfp_raw_look_enable(if deliver || mask == 0 { mask } else { 1 << 31 });
        for (slot, yaw, pitch) in pushes {
            tsfp_raw_look_push(slot, yaw, pitch);
        }
    }
}

/// T3453: the raw-look guard's clock. A tick-clock replay measures its decision window in
/// simulation ticks (60 per second), so the fallback lands on the same guest tick in every run;
/// on the wall clock it landed wherever the fast-forward guest was after 3 s of host time.
fn guard_elapsed_seconds(replay_clock: bool, simulation_tick: u64, wall_seconds: f32) -> f32 {
    if replay_clock {
        simulation_tick as f32 / 60.0
    } else {
        wall_seconds
    }
}

fn source_name(source: Option<InputSource>) -> String {
    match source {
        None => "none".into(),
        Some(InputSource::MouseKeyboard) => "mouse+keyboard".into(),
        Some(InputSource::Pad(id)) => format!("pad {id}"),
    }
}

fn should_sample_record_tick(last_tick: u64, simulation_tick: u64) -> bool {
    simulation_tick != 0 && simulation_tick > last_tick
}

fn gameplay_inputs_for_page(
    inputs: [Option<PlayerInput>; 4],
    native_ui_owns_input: bool,
) -> [Option<PlayerInput>; 4] {
    if native_ui_owns_input {
        [None; 4]
    } else {
        inputs
    }
}

pub(crate) fn replay_shims_for_page(
    events: Vec<ShimEvent>,
    native_ui_owns_input: bool,
) -> Vec<ShimEvent> {
    if native_ui_owns_input {
        Vec::new()
    } else {
        events
    }
}

/// Once per Bevy frame: tick the router, queue the shim events, apply the pointer capture to the window.
pub fn tick(mut res: ResMut<RouterRes>, mut window: Query<&mut Window, With<PrimaryWindow>>) {
    let res = &mut *res;
    if let Some(settings) = crate::ui_extensions::settings() {
        if crate::ui_extensions::devices_edited() {
            res.router.apply_settings(&settings);
        } else {
            res.router.restore_automatic_devices();
            res.router.set_player_bindings(&settings);
        }
    }
    if let Ok(mut window) = window.single_mut() {
        res.focused = window.focused;
        // T2168: a live Rust UI page releases the pointer (visible menu cursor); gameplay captures it again.
        let menu = crate::ui_live::owns_pointer();
        let grab = res.wants_grab() && !menu;
        tsfp_input_intake::decisions::pointer_grab(grab); // T3820: recorded and compared, not forced
        // Native display pointer takes over only after its first successful GPU draw.
        if crate::mapmaker_ui_live::native_page() {
            window.cursor_options.visible = !grab && crate::mapmaker_ui_live::system_cursor();
        } else if tsfp_wgpu::frame_layer::display_rate() {
            window.cursor_options.visible = !grab && !crate::ui_overlay::display_pointer_active();
        }
        if grab != res.grabbed {
            res.grabbed = grab;
            // Locked on Wayland (pointer-constraints), bevy_winit falls back to Confined on X11 and XWayland by itself.
            window.cursor_options.grab_mode = if grab {
                CursorGrabMode::Locked
            } else {
                CursorGrabMode::None
            };
            window.cursor_options.visible = !grab && !crate::ui_overlay::display_pointer_active();
            if crate::mapmaker_ui_live::native_page() {
                window.cursor_options.visible = !grab && crate::mapmaker_ui_live::system_cursor();
            }
            let reason = if grab {
                "mouse and keyboard player, window focused"
            } else if !res.focused {
                "window not focused"
            } else if menu {
                "a menu page is live, the mouse points at the menu"
            } else if res.capture.is_released() {
                "released with Ctrl+Alt or F10"
            } else {
                "slot 0 is on a pad"
            };
            eprintln!(
                "tsfp-bevy: pointer {} ({reason})",
                if grab {
                    "captured (Ctrl+Alt or F10 releases)"
                } else {
                    "released"
                }
            );
        }
    }
    let mut replaying = crate::input_recording::replaying();
    let recorded_tick = if crate::input_recording::enabled() {
        let tick = crate::wi::simulation_tick();
        if !should_sample_record_tick(res.last_recorded_tick, tick) {
            return;
        }
        res.last_recorded_tick = tick;
        Some(tick)
    } else if replaying {
        // Replay polls on every host frame. Playback rows are gated by recorded host polls so
        // boot and loading stalls do not couple route progress to wall-clock or game-tick pacing.
        Some(crate::wi::simulation_tick())
    } else {
        None
    };
    let (inputs, replay_events) = if replaying {
        let sim_tick = recorded_tick.expect("replay sampling has a simulation tick");
        match crate::input_recording::replay_tick(sim_tick) {
            Ok(crate::input_recording::ReplayStep::Wait) => return,
            Ok(crate::input_recording::ReplayStep::End) => {
                // T3390: once; an uncapped headless loop printed it ~1250 times before the host
                // stopped, and its lines interleaved with the host's exit counters.
                static REPORTED: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                if !REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    eprintln!(
                        "tsfp-bevy: input replay route end sim_tick={sim_tick} poll={}",
                        crate::input_recording::route_end_marker_poll()
                    );
                }
                crate::wi::request_quit(RawEvent::Quit);
                return;
            }
            Ok(crate::input_recording::ReplayStep::Handover) => {
                eprintln!(
                    "tsfp-bevy: native input replay handed over to live input sim_tick={sim_tick} poll={}",
                    crate::wi::host_poll_index()
                );
                crate::input_recording::finish_handover();
                replaying = false;
                qualification::before_tick(&mut res.router, host_counters().1);
                let _ = res.router.tick();
                (
                    std::array::from_fn(|slot| {
                        res.router.game_input(slot as u8).map(|frame| frame.player)
                    }),
                    None,
                )
            }
            Ok(crate::input_recording::ReplayStep::Tick(inputs, events)) => (inputs, Some(events)),
            Err(error) => {
                eprintln!("tsfp-bevy: input replay failed: {error}");
                crate::wi::request_quit(RawEvent::Quit);
                return;
            }
        }
    } else {
        qualification::before_tick(&mut res.router, host_counters().1);
        let _ = res.router.tick();
        (
            std::array::from_fn(|slot| res.router.game_input(slot as u8).map(|frame| frame.player)),
            None,
        )
    };
    let native = crate::ui_live::native_ui_owns_input();
    let gameplay_inputs = gameplay_inputs_for_page(inputs, native);
    // T3771: router slots 1 to 3 are guest controller ports 1 to 3 (`--pad-ports`), so a second pad is player 2.
    let live = std::array::from_fn(|slot| res.router.source(slot as u8).is_some());
    let replay_tick = replaying.then(crate::wi::simulation_tick); // T3781: recorded presence
    tsfp_input_slots::ports::publish(live, &inputs, &gameplay_inputs, replay_tick);
    let sources = std::array::from_fn(|slot| res.router.source(slot as u8));
    tsfp_input_slots::sources::publish(tsfp_input_intake::decisions::slot_sources(sources)); // T3772, T3820
    if let Some(session) = crate::story_mission_runtime::story_gameplay_input_session()
        && let Err(error) = crate::story_mission_runtime::publish_story_player_inputs_for_step(
            session,
            gameplay_inputs,
        )
    {
        eprintln!("tsfp-bevy: Story input snapshot publication failed: {error:?}");
    }
    if crate::hud_producer_live::messages_native_enabled() {
        tsfp_input_slots::hud::publish(&inputs);
    }
    if let Some(sim_tick) = recorded_tick {
        crate::input_recording::state_digest(sim_tick, crate::wi::host_poll_index(), replaying);
    }
    if let Some(sim_tick) = recorded_tick
        && !replaying
        && let Err(error) = crate::input_recording::record_tick(
            sim_tick,
            crate::wi::host_poll_index(),
            tsfp_input_slots::ports::with_presence(gameplay_inputs, live, native, sim_tick),
        )
    {
        eprintln!("tsfp-bevy: input recording write failed: {error}");
    }
    if let Some(sim_tick) = recorded_tick
        && !replaying
        && crate::ui_live::native_ui_owns_input()
        && inputs
            .iter()
            .any(|input| input.is_some_and(|input| input.held != 0))
        && let Err(error) = crate::input_recording::record_native_pad(
            sim_tick,
            crate::wi::host_poll_index(),
            inputs,
        )
    {
        eprintln!("tsfp-bevy: input recording write failed: {error}");
    }
    if !replaying {
        qualification::after_tick(&inputs);
    }
    for slot in 0..res.owners.len() {
        let now = res.router.source(slot as u8);
        if now != res.owners[slot] {
            eprintln!(
                "tsfp-bevy: slot {slot} input owner: {} -> {}",
                source_name(res.owners[slot]),
                source_name(now)
            );
            res.owners[slot] = now;
        }
    }
    if res.raw_look {
        let mouse_active = inputs[0].is_some_and(|input| matches!(input.look, Look::Delta { yaw_deg, pitch_deg } if input.source == SourceKind::MouseKeyboard && (yaw_deg != 0.0 || pitch_deg != 0.0)));
        if mouse_active {
            res.stats.pushes += 1;
        }
        let (host, counters) = host_counters();
        match res.guard.update(
            guard_elapsed_seconds(
                replaying && crate::input_recording::tick_clock(),
                crate::wi::simulation_tick(),
                res.started.elapsed().as_secs_f32(),
            ),
            mouse_active && !tsfp_input::guard::replay_holds_raw(replaying),
            host,
        ) {
            Some(GuardEvent::Active) => eprintln!(
                "tsfp-bevy: raw look ACTIVE: the player look root runs in Rust (hook reached {} times so far), mouse counts go to yaw and pitch directly",
                counters[1]
            ),
            Some(GuardEvent::NotActive(text)) => eprintln!("tsfp-bevy: {text}"),
            Some(GuardEvent::Reactivated) => {
                eprintln!("tsfp-bevy: raw look ACTIVE again: the look hook reports hits")
            }
            None => {}
        }
        res.router.set_mounted(counters[6] as u8);
        let raw = tsfp_input_intake::decisions::raw_look(res.guard.raw()); // T3820: recorded, replayed verbatim
        if raw != res.router.raw_look() {
            res.router.set_raw_look(raw);
        }
        raw_look_feed(&inputs, raw);
        if diag_enabled() {
            log_look_stats(res, counters, host.reachable);
        }
    }
    for (slot, request) in res.router.take_weapon_requests().into_iter().enumerate() {
        if let Some(number) = request {
            tsfp_input_slots::weapons::store(slot, number);
        }
    }
    if let Some(events) = replay_events {
        let native_ui_owns_input = crate::ui_live::native_ui_owns_input();
        if std::env::var_os("TSFP_POINTER_LOG").is_some() && !events.is_empty() {
            let active_page = crate::ui_live::replay_active_page()
                .map_or_else(|| "none".to_owned(), |page| format!("{page:#x}"));
            eprintln!(
                "tsfp-bevy: pointer-log: replay shim poll {} active Rust page {} destination {} events {:?}",
                crate::input_recording::replay_poll(),
                active_page,
                if native_ui_owns_input {
                    "native-ui"
                } else {
                    "guest-queue"
                },
                events,
            );
        }
        if std::env::var_os("TSFP_REPLAY_INPUT_LOG").is_some() && !events.is_empty() {
            eprintln!(
                "replay-route: tick {} replay-poll {} guest-polls {} events {:?}",
                crate::wi::simulation_tick(),
                crate::input_recording::replay_poll(),
                crate::wi::guest_pad_poll_count(),
                events,
            );
        }
        // T3453: the guest's next controller poll routes them (guest thread), see
        // input_recording::deliver_guest_bound.
        crate::input_recording::queue_for_guest_poll(events);
        crate::input_recording::publish_replay_progress(); // T2487: lockstep, events are queued
    } else {
        crate::input_recording::deliver_guest_bound(); // T3453: rows queued before a handover first
        flush_to_queue(&mut res.router, &mut lock(&SHARED.wi));
    }
}

/// The input-queue event of a pad shim event.
pub(crate) fn shim_raw_event(event: ShimEvent) -> RawEvent {
    match event {
        ShimEvent::ButtonDown(button) => RawEvent::button_down(None, button),
        ShimEvent::ButtonUp(button) => RawEvent::button_up(None, button),
        ShimEvent::Axis { axis, value } => RawEvent::axis(None, axis, value),
    }
}

/// At most one line per second: what the mouse delivered since the last line and what the hook did with it.
fn log_look_stats(res: &mut RouterRes, counters: [u32; 7], reachable: bool) {
    if res.stats.last_log.elapsed().as_secs_f32() < 1.0 {
        return;
    }
    let stats = &mut res.stats;
    eprintln!(
        "tsfp-bevy: look: mode={} captured={} focused={} owner={} | last s: motion events {} counts {:.0} pushed ticks {} (dropped events total {}) | host: reachable={} enable calls {} hook calls {} kept original {} mailbox pushes {} applied {} overlays {}",
        if res.guard.raw() {
            "raw"
        } else {
            "interim-stick"
        },
        res.grabbed,
        res.focused,
        source_name(res.router.source(0)),
        stats.motion_events - stats.last_events,
        stats.counts - stats.last_counts,
        stats.pushes - stats.last_pushes,
        stats.dropped_events,
        reachable,
        counters[0],
        counters[1],
        counters[2],
        counters[3],
        counters[4],
        counters[5],
    );
    stats.last_log = Instant::now();
    stats.last_events = stats.motion_events;
    stats.last_counts = stats.counts;
    stats.last_pushes = stats.pushes;
}

fn gamepad_rumble_request(gamepad: Entity, effect: RumbleEffect) -> GamepadRumbleRequest {
    match effect {
        RumbleEffect::Off => GamepadRumbleRequest::Stop { gamepad },
        RumbleEffect::Dual {
            low,
            high,
            duration_ms,
        } => GamepadRumbleRequest::Add {
            gamepad,
            duration: std::time::Duration::from_millis(u64::from(duration_ms)),
            intensity: GamepadRumbleIntensity {
                strong_motor: low.clamp(0.0, 1.0),
                weak_motor: high.clamp(0.0, 1.0),
            },
        },
    }
}

pub fn rumble_system(
    res: Res<RouterRes>,
    pads: Query<(Entity, &Gamepad)>,
    mut out: EventWriter<GamepadRumbleRequest>,
) {
    let requests = RUMBLE
        .lock()
        .map(|mut queue| std::mem::take(&mut *queue))
        .unwrap_or_default();
    for (slot, effect) in requests {
        res.router
            .route_game_rumble(slot, effect, &mut |pad, effect| {
                let Some((entity, _)) = pads.iter().find(|(entity, _)| entity.index() == pad)
                else {
                    return;
                };
                out.write(gamepad_rumble_request(entity, effect));
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::apply_gamepad_event;
    use crate::window_input::{InputEvent, Kind};

    #[test]
    fn recorder_samples_only_advancing_nonzero_simulation_ticks() {
        assert!(!should_sample_record_tick(0, 0));
        assert!(should_sample_record_tick(0, 1));
        assert!(!should_sample_record_tick(1, 1));
        assert!(should_sample_record_tick(1, 2));
        assert!(!should_sample_record_tick(2, 1));
    }

    #[test]
    fn native_ui_pages_keep_gameplay_snapshots_out_of_record_and_legacy_replay_queue() {
        let input = PlayerInput {
            step: 1,
            slot: 0,
            source: SourceKind::Replay,
            held: tsfp_input::player::bits::A,
            pressed: 0,
            released: 0,
            move_axis: tsfp_input::player::Vec2::new(0.0, 0.0),
            look: Look::Stick(tsfp_input::player::Vec2::new(0.0, 0.0)),
            triggers: [0.0; 2],
            menu: tsfp_input::player::MenuIntent {
                accept: false,
                back: false,
            },
            assists: tsfp_input::player::AssistMask::NONE,
        };
        let inputs = [Some(input), None, None, None];
        assert_eq!(gameplay_inputs_for_page(inputs, true), [None; 4]);
        assert_eq!(gameplay_inputs_for_page(inputs, false), inputs);

        let shims = vec![ShimEvent::ButtonDown(0), ShimEvent::ButtonUp(0)];
        assert!(replay_shims_for_page(shims.clone(), true).is_empty());
        assert_eq!(replay_shims_for_page(shims.clone(), false), shims);
    }

    /// T3453: a replay's raw-look guard decides on simulation ticks, so a fast-forward run that
    /// spent 50 s of host time in boot is still undecided at tick 120 and decides at tick 180.
    #[test]
    fn replay_raw_look_guard_runs_on_simulation_ticks() {
        let off = HostProbe {
            reachable: false,
            hits: 0,
        };
        let mut replay = RawLookGuard::new();
        assert_eq!(
            replay.update(guard_elapsed_seconds(true, 120, 50.0), false, off),
            None
        );
        assert!(replay.raw());
        assert!(matches!(
            replay.update(guard_elapsed_seconds(true, 180, 0.1), false, off),
            Some(GuardEvent::NotActive(_))
        ));
        let mut live = RawLookGuard::new();
        assert!(matches!(
            live.update(guard_elapsed_seconds(false, 120, 50.0), false, off),
            Some(GuardEvent::NotActive(_))
        ));
    }

    use bevy::input::gamepad::{
        GamepadConnectionEvent, RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent,
    };

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn input_preset_flag_selects_per_slot_and_defaults_to_original() {
        let mut args = strings(&[
            "a.xbe",
            "--input-router",
            "--input-preset",
            "modern:1",
            "--input-preset",
            "mouse-raw:2",
        ]);
        let options = extract(&mut args).unwrap().unwrap();
        assert_eq!(args, strings(&["a.xbe"]));
        let names: Vec<&str> = options
            .config
            .players
            .iter()
            .map(|p| p.preset.as_str())
            .collect();
        assert_eq!(names, ["original", "modern", "mouse-raw", "original"]);
        assert!(extract(&mut strings(&["--input-router", "--input-preset", "nope"])).is_err());
        assert!(
            extract(&mut strings(&["--input-preset", "modern"])).is_err(),
            "needs --input-router"
        );
        let options = extract(&mut strings(&["--input-router"])).unwrap().unwrap();
        assert!(
            options
                .config
                .players
                .iter()
                .all(|p| p.preset == "original")
        );
    }

    #[test]
    fn extract_strips_the_shell_options() {
        let mut args = strings(&[
            "a.xbe",
            "--input-router",
            "--mouse-keyboard",
            "--input-bindings",
            "tmp/missing.toml",
        ]);
        assert!(
            extract(&mut args).is_err(),
            "an explicit bindings file must exist"
        );
        assert_eq!(args, strings(&["a.xbe"]));
        let mut args = strings(&[
            "a.xbe",
            "--input-router",
            "--mouse-keyboard",
            "--interactive",
        ]);
        let options = extract(&mut args).unwrap().unwrap();
        assert!(options.mouse_keyboard);
        assert_eq!(args, strings(&["a.xbe", "--interactive"]));
        let mut args = strings(&["a.xbe", "--interactive"]);
        assert!(extract(&mut args).unwrap().is_none());
        let mut args = strings(&["--mouse-keyboard"]);
        assert!(extract(&mut args).is_err());
    }

    #[test]
    fn native_record_option_requires_router_and_is_removed_from_host_args() {
        let mut args = strings(&[
            "a.xbe",
            "--input-router",
            "--input-record",
            "tmp/play/run/input.rec",
        ]);
        let options = extract(&mut args).unwrap().unwrap();
        assert_eq!(args, strings(&["a.xbe"]));
        assert_eq!(
            options.record_path.as_deref(),
            Some(std::path::Path::new("tmp/play/run/input.rec"))
        );
        assert!(extract(&mut strings(&["--input-record", "tmp/input.rec"])).is_err());
    }

    #[test]
    fn native_replay_option_requires_router_and_excludes_recording() {
        let mut args = strings(&[
            "a.xbe",
            "--input-router",
            "--input-replay",
            "tmp/play/run/input.rec",
            "--input-replay-handover",
        ]);
        let options = extract(&mut args).unwrap().unwrap();
        assert_eq!(args, strings(&["a.xbe"]));
        assert_eq!(
            options.replay_path.as_deref(),
            Some(std::path::Path::new("tmp/play/run/input.rec"))
        );
        assert!(options.replay_handover);
        assert!(
            extract(&mut strings(&[
                "--input-router",
                "--input-record",
                "a",
                "--input-replay",
                "b"
            ]))
            .is_err()
        );
        assert!(extract(&mut strings(&["--input-replay", "tmp/input.rec"])).is_err());
        assert!(extract(&mut strings(&["--input-router", "--input-replay-handover"])).is_err());
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    fn drain(wi: &mut Wi) -> Vec<(i32, Option<&'static std::ffi::CStr>, i32, i32)> {
        let mut out = Vec::new();
        while let Some(InputEvent {
            kind,
            name,
            raw,
            value,
        }) = wi.input.next()
        {
            out.push((kind as i32, name, raw, value));
        }
        out
    }

    fn fresh() -> Wi {
        let mut wi = Wi::default();
        wi.input.gamepad = true;
        wi
    }

    /// The shim on versus off: random Bevy gamepad events give the identical host queue (kind, name, raw code, value).
    #[test]
    fn router_on_equals_router_off_for_a_pad() {
        use GamepadButton::*;
        let buttons = [
            South,
            East,
            West,
            North,
            Select,
            Mode,
            Start,
            LeftThumb,
            RightThumb,
            LeftTrigger,
            RightTrigger,
            DPadUp,
            DPadDown,
            DPadLeft,
            DPadRight,
            LeftTrigger2,
            RightTrigger2,
        ];
        let axes = [
            GamepadAxis::LeftStickX,
            GamepadAxis::LeftStickY,
            GamepadAxis::RightStickX,
            GamepadAxis::RightStickY,
        ];
        let entity = Entity::from_raw(7);
        for seed in 1..=10u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let mut off = fresh();
            let mut on = fresh();
            let mut router = Router::new(RouterConfig::default());
            let mut controllers = ControllerEventPath::default();
            let connect = RawGamepadEvent::Connection(GamepadConnectionEvent::new(
                entity,
                GamepadConnection::Connected {
                    name: "t".into(),
                    vendor_id: None,
                    product_id: None,
                },
            ));
            apply_gamepad_event(&mut off, &connect);
            route_pad_event(&mut router, &mut on, &mut controllers, &connect);
            let (mut left, mut right) = (Vec::new(), Vec::new());
            for step in 0..3_000 {
                let value = match rng.next() % 8 {
                    0 => 0.0,
                    1 => 1.0,
                    2 => -1.0,
                    3 => 0.5,
                    _ => (rng.next() % 2_000_001) as f32 / 1_000_000.0 - 1.0,
                };
                let event = if rng.next() % 2 == 0 {
                    RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
                        entity,
                        buttons[(rng.next() % 17) as usize],
                        value,
                    ))
                } else {
                    RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                        entity,
                        axes[(rng.next() % 4) as usize],
                        value,
                    ))
                };
                apply_gamepad_event(&mut off, &event);
                route_pad_event(&mut router, &mut on, &mut controllers, &event);
                if step % 3 == 0 {
                    router.tick();
                    flush_to_queue(&mut router, &mut on);
                }
                left.extend(drain(&mut off));
                right.extend(drain(&mut on));
            }
            router.tick();
            flush_to_queue(&mut router, &mut on);
            right.extend(drain(&mut on));
            // GUIDE exists in Bevy's legacy host queue but has no guest XInput target.
            left.retain(|(_, name, _, _)| {
                name.as_deref().and_then(|name| name.to_str().ok()) != Some("GUIDE")
            });
            assert!(left.len() > 500, "{} events", left.len());
            assert_eq!(left, right, "seed {seed}");
        }
    }

    #[test]
    fn escape_never_releases_the_pointer_and_the_chord_does() {
        let options = Options {
            mouse_keyboard: true,
            raw_look: false,
            bindings: Bindings::default(),
            config: Default::default(),
            pad_filter: Default::default(),
            record_path: None,
            replay_path: None,
            replay_handover: false,
        };
        let mut res = RouterRes::new(&options);
        assert!(res.wants_grab());
        assert!(
            !res.capture.key("Escape", true),
            "Escape is not a release chord"
        );
        assert!(res.wants_grab(), "Escape keeps the grab");
        res.capture.key("ControlLeft", true);
        res.capture.key("AltLeft", true);
        assert!(!res.wants_grab(), "released by the chord");
        res.focused = false;
        res.capture.key("F10", true);
        assert!(!res.wants_grab(), "no grab without focus");
        let pad_only = RouterRes::new(&Options {
            mouse_keyboard: false,
            raw_look: false,
            bindings: Bindings::default(),
            config: Default::default(),
            pad_filter: Default::default(),
            record_path: None,
            replay_path: None,
            replay_handover: false,
        });
        assert!(!pad_only.wants_grab());
    }

    #[test]
    fn f10_capture_hotkey_is_not_forwarded_to_game_input() {
        use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};

        let options = Options {
            mouse_keyboard: true,
            raw_look: false,
            bindings: Bindings::default(),
            config: Default::default(),
            pad_filter: Default::default(),
            record_path: None,
            replay_path: None,
            replay_handover: false,
        };
        let mut res = RouterRes::new(&options);
        res.key(&KeyboardInput {
            key_code: KeyCode::F10,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        assert!(res.capture.is_released());
        assert!(res.router.drain_events().is_empty());
    }

    #[test]
    fn dual_rumble_keeps_low_and_high_motor_channels_distinct() {
        let gamepad = Entity::from_raw(7);
        let request = gamepad_rumble_request(
            gamepad,
            RumbleEffect::Dual {
                low: 0.25,
                high: 0.75,
                duration_ms: 35,
            },
        );
        let GamepadRumbleRequest::Add {
            gamepad: actual_gamepad,
            duration,
            intensity,
        } = request
        else {
            panic!("expected dual-motor request");
        };
        assert_eq!(actual_gamepad, gamepad);
        assert_eq!(duration, std::time::Duration::from_millis(35));
        assert_eq!(intensity.strong_motor, 0.25);
        assert_eq!(intensity.weak_motor, 0.75);
    }

    #[test]
    fn mouse_keyboard_slot_reaches_the_queue_as_pad_events() {
        let options = Options {
            mouse_keyboard: true,
            raw_look: false,
            bindings: Bindings::default(),
            config: Default::default(),
            pad_filter: Default::default(),
            record_path: None,
            replay_path: None,
            replay_handover: false,
        };
        let mut res = RouterRes::new(&options);
        let mut wi = fresh();
        res.router.feed(DeviceEvent::Key {
            name: "Space".into(),
            down: true,
        });
        res.router.feed(DeviceEvent::Key {
            name: "KeyW".into(),
            down: true,
        });
        res.router.tick();
        flush_to_queue(&mut res.router, &mut wi);
        let events = drain(&mut wi);
        assert!(events.iter().any(|(_, name, _, _)| *name == Some(c"A")));
        assert!(
            events
                .iter()
                .any(|(_, name, _, value)| *name == Some(c"LY") && *value == 32767),
            "forward = LY up positive in the queue"
        );
    }

    #[test]
    fn escape_presses_start_and_backspace_presses_b() {
        let options = Options {
            mouse_keyboard: true,
            raw_look: false,
            bindings: Bindings::default(),
            config: Default::default(),
            pad_filter: Default::default(),
            record_path: None,
            replay_path: None,
            replay_handover: false,
        };
        let mut res = RouterRes::new(&options);
        let mut wi = fresh();
        res.router.feed(DeviceEvent::Key {
            name: "Escape".into(),
            down: true,
        });
        res.router.feed(DeviceEvent::Key {
            name: "Backspace".into(),
            down: true,
        });
        res.router.tick();
        flush_to_queue(&mut res.router, &mut wi);
        let events = drain(&mut wi);
        assert!(
            events
                .iter()
                .any(|(kind, name, _, _)| *kind == Kind::PadDown as i32 && *name == Some(c"START")),
            "{events:?}"
        );
        assert!(
            events
                .iter()
                .any(|(kind, name, _, _)| *kind == Kind::PadDown as i32 && *name == Some(c"B")),
            "{events:?}"
        );
        res.router.feed(DeviceEvent::Key {
            name: "Escape".into(),
            down: false,
        });
        res.router.tick();
        flush_to_queue(&mut res.router, &mut wi);
        assert!(
            drain(&mut wi)
                .iter()
                .any(|(kind, name, _, _)| *kind == Kind::PadUp as i32 && *name == Some(c"START")),
            "release toggles back"
        );
    }

    #[test]
    fn raw_look_option_defaults_on_with_mouse_keyboard_and_needs_it() {
        let parse =
            |args: &[&str]| extract(&mut args.iter().map(|a| a.to_string()).collect::<Vec<_>>());
        let on = parse(&["--input-router", "--mouse-keyboard"])
            .unwrap()
            .unwrap();
        assert!(on.mouse_keyboard && on.raw_look);
        let off = parse(&["--input-router", "--mouse-keyboard", "--no-raw-look"])
            .unwrap()
            .unwrap();
        assert!(off.mouse_keyboard && !off.raw_look);
        let pads = parse(&["--input-router"]).unwrap().unwrap();
        assert!(!pads.raw_look, "pad only: the feature stays off");
        assert!(parse(&["--input-router", "--raw-look"]).is_err());
        assert!(parse(&["--raw-look"]).is_err());
    }

    #[test]
    fn raw_look_plan_hands_degrees_to_the_mouse_slot_only_and_zeroes_the_stick() {
        let options = Options {
            mouse_keyboard: true,
            raw_look: true,
            bindings: Bindings::default(),
            config: Default::default(),
            pad_filter: Default::default(),
            record_path: None,
            replay_path: None,
            replay_handover: false,
        };
        let mut res = RouterRes::new(&options);
        let mut wi = fresh();
        res.router.feed(DeviceEvent::MouseMotion {
            dx: 100.0,
            dy: -50.0,
        });
        let inputs = res.router.tick();
        let (mask, pushes) = raw_look_plan(&inputs);
        assert_eq!(mask, 1);
        assert_eq!(pushes.len(), 1);
        let (slot, yaw, pitch) = pushes[0];
        assert_eq!(slot, 0);
        assert!(
            (yaw - 2.2).abs() < 1e-4 && (pitch - 1.1).abs() < 1e-4,
            "{yaw} {pitch}: 0.022 deg per count, mouse up = pitch up"
        );
        flush_to_queue(&mut res.router, &mut wi);
        let events = drain(&mut wi);
        assert!(
            !events.iter().any(
                |(_, name, _, value)| matches!(name, Some(n) if *n == c"RX" || *n == c"RY")
                    && *value != 0
            ),
            "no virtual right stick in raw mode"
        );
        // a pad slot never feeds the mailbox
        let mut pads = Router::new(RouterConfig::default());
        pads.feed(DeviceEvent::PadConnected(7));
        pads.feed(DeviceEvent::PadAxis {
            pad: 7,
            axis: PadAxis::RightX,
            value: 1.0,
        });
        let inputs = pads.tick();
        assert_eq!(raw_look_plan(&inputs), (0, Vec::new()));
    }

    /// T2200: a keyboard's consumer control node enumerated as a gamepad never reaches the router, a real pad does.
    #[test]
    fn keyboard_consumer_control_node_is_filtered_out_of_the_router() {
        let mut filter = PadFilter::new(true, &[]);
        let bogus = Entity::from_raw(6);
        let real = Entity::from_raw(7);
        let connect = |entity: Entity, name: &str| {
            RawGamepadEvent::Connection(GamepadConnectionEvent::new(
                entity,
                GamepadConnection::Connected {
                    name: name.into(),
                    vendor_id: Some(0x4b4d),
                    product_id: None,
                },
            ))
        };
        assert!(!admit_pad_event(
            &mut filter,
            &connect(bogus, "SmartCloud AL68 Keyboard System Control")
        ));
        assert!(admit_pad_event(
            &mut filter,
            &connect(real, "Xbox Wireless Controller")
        ));
        let noise = RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
            bogus,
            GamepadAxis::LeftStickX,
            1.0,
        ));
        let stick = RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
            real,
            GamepadAxis::LeftStickX,
            1.0,
        ));
        assert!(
            !admit_pad_event(&mut filter, &noise),
            "the bogus pad's axis noise is dropped"
        );
        assert!(admit_pad_event(&mut filter, &stick));
        // The slot stays on the mouse and keyboard: no pad ever connected to the router for the bogus device.
        let mut router = Router::new(RouterConfig {
            mouse_keyboard_slot: Some(0),
            ..RouterConfig::default()
        });
        let mut wi = fresh();
        let mut controllers = ControllerEventPath::default();
        for event in [
            connect(bogus, "SmartCloud AL68 Keyboard System Control"),
            noise,
        ] {
            let mut gate = PadFilter::new(true, &[]);
            if admit_pad_event(&mut gate, &event) {
                route_pad_event(&mut router, &mut wi, &mut controllers, &event);
            }
        }
        for _ in 0..100 {
            router.tick();
        }
        assert_eq!(router.source(0), Some(InputSource::MouseKeyboard));
        let disconnect = RawGamepadEvent::Connection(GamepadConnectionEvent::new(
            bogus,
            GamepadConnection::Disconnected,
        ));
        assert!(
            !admit_pad_event(&mut filter, &disconnect),
            "disconnect of an ignored device is dropped too"
        );
    }
}

#[path = "input_qualification.rs"]
mod qualification;
