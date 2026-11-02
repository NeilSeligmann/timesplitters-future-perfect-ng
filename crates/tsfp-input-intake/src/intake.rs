// SPDX-License-Identifier: GPL-3.0-or-later
//! T3820: the intake system. It runs once per Bevy update, after the device backends (winit, gilrs) queued
//! their events and before `InputSystem` folds them into `ButtonInput`, `Gamepad` and the cursor state, so
//! every consumer (input router, Rust UI pages, text entry, storage, Map Maker, new pages) reads what the
//! intake saw:
//!
//! * recording: each new device event is written as a `D` row stamped with the simulation tick and host poll;
//! * replay (format v6): live device events are dropped (live keys still reach the host for its close chord)
//!   and the rows due at the replay poll are sent into the same event queues instead. Headless replays get a
//!   virtual primary window carrying the recorded size, cursor and focus.

use crate::codec::{DeviceEvent, RouteKind, button_state, encode};
use crate::schedule::Schedule;
use bevy_app::{App, PreUpdate, Update};
use bevy_ecs::event::{Event, EventCursor, Events};
use bevy_ecs::prelude::*;
use bevy_input::InputSystem;
use bevy_input::gamepad::{
    GamepadConnection, GamepadConnectionEvent, RawGamepadAxisChangedEvent,
    RawGamepadButtonChangedEvent, RawGamepadEvent,
};
use bevy_input::keyboard::{KeyCode, KeyboardFocusLost, KeyboardInput};
use bevy_input::mouse::{MouseButtonInput, MouseMotion, MouseWheel};
use bevy_math::Vec2;
use bevy_window::{CursorLeft, CursorMoved, PrimaryWindow, Window, WindowFocused};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// The recorder's own control key (F12 adds a mark): not game input, so never recorded or replayed.
pub const MARK_HOTKEY: KeyCode = KeyCode::F12;

/// Every Bevy event type the intake records and replays. A consumer reading any other device event type
/// bypasses the recorder (`tests/test_t3820_input_intake_guard.py` enforces this list).
pub const RECORDED_EVENTS: &[&str] = &[
    "KeyboardInput",
    "KeyboardFocusLost",
    "WindowFocused",
    "MouseButtonInput",
    "MouseWheel",
    "MouseMotion",
    "CursorMoved",
    "CursorLeft",
    "RawGamepadEvent",
    "GamepadConnectionEvent",
    "RawGamepadButtonChangedEvent",
    "RawGamepadAxisChangedEvent",
];

/// What the shell provides: clocks, the recording file and the lockstep.
pub trait Host: Send + Sync + 'static {
    /// `(simulation tick, host poll)` stamped on recorded rows.
    fn clock(&self) -> (u64, u64);
    fn write(&self, sim_tick: u64, host_poll: u64, payload: String);
    /// The replay schedule's host poll (recorded rows at or before it are due).
    fn replay_poll(&self) -> u64;
    /// A live key while replaying: never routed, only offered to the host (its window close chord).
    fn live_key(&self, key: &KeyboardInput);
    /// Replayed rows were processed by every consumer: wake the guest's lockstep.
    fn progress(&self);
}

static HOST: OnceLock<Box<dyn Host>> = OnceLock::new();

pub fn set_host(host: Box<dyn Host>) {
    let _ = HOST.set(host);
}

#[derive(Default)]
enum Mode {
    #[default]
    Off,
    Record,
    Replay(Schedule),
}

#[derive(Default)]
struct State {
    mode: Mode,
    /// Replay: the recorded cursor and window size, the recorded focus.
    cursor: Option<(Option<Vec2>, Vec2)>,
    focus: Option<bool>,
    /// Replay: recorded pad id -> replay entity; recorded Alt keys held (the close chord is not replayed).
    pads: Vec<(u32, Entity)>,
    alt_held: u8,
    /// Recording: the last value written per decision.
    decided: Vec<(&'static str, String)>,
}

static STATE: Mutex<State> = Mutex::new(State {
    mode: Mode::Off,
    cursor: None,
    focus: None,
    pads: Vec::new(),
    alt_held: 0,
    decided: Vec::new(),
});

fn state() -> MutexGuard<'static, State> {
    STATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn start_recording() {
    state().mode = Mode::Record;
}

/// Replay the `(host poll, payload)` rows of a v6 recording; `s:`-prefixed rows are its state digests, from which a
/// recording without `control` decisions gets the owner's frozen stretches ([`crate::frozen`], INFERRED).
pub fn start_replay(rows: impl IntoIterator<Item = (u64, String)>) {
    let (mut devices, states) = crate::frozen::split(rows.into_iter().collect());
    let inferred = crate::frozen::infer_control_rows(&devices, &states);
    if !inferred.is_empty() {
        eprintln!(
            "tsfp-bevy: input replay: no control rows, {} inferred from the state digests (frozen player, INFERRED)",
            inferred.len()
        );
        devices.extend(inferred);
        devices.sort_by_key(|(poll, _)| *poll);
    }
    let mut state = state();
    *state = State::default();
    state.mode = Mode::Replay(Schedule::new(devices));
}

/// Live input from now on (replay handover, or the end of a recording).
pub fn stop() {
    state().mode = Mode::Off;
}

pub fn recording() -> bool {
    matches!(state().mode, Mode::Record)
}

pub fn replaying() -> bool {
    matches!(state().mode, Mode::Replay(_))
}

/// Highest host poll whose replayed device rows were delivered and processed (`u64::MAX`: none pending).
pub fn ready_poll() -> u64 {
    match &state().mode {
        Mode::Replay(schedule) => schedule.ready_poll(),
        _ => u64::MAX,
    }
}

/// A key or mouse button was routed to `route`: recorded beside the device rows, checked on replay.
pub fn route(kind: RouteKind, route: &'static str) {
    let Some(host) = HOST.get() else { return };
    let mut state = state();
    match &mut state.mode {
        Mode::Record => {
            drop(state);
            let (tick, poll) = host.clock();
            host.write(tick, poll, encode(&DeviceEvent::Route(kind, route.into())));
        }
        Mode::Replay(schedule) => schedule.observe_route(kind, route, host.replay_poll()),
        Mode::Off => {}
    }
}

/// A live-state input decision (raw-look guard, slot sources, pointer capture): recorded when it changes, and in a
/// v6 replay the recorded value is returned instead of `live` (the replay does not decide again). Values follow
/// [`crate::codec::is_decision_value`].
pub fn decide(name: &'static str, live: &str) -> String {
    let Some(host) = HOST.get() else {
        return live.to_owned();
    };
    let mut state = state();
    match &mut state.mode {
        Mode::Record => {
            if state.decided.iter().any(|(n, v)| *n == name && v == live) {
                return live.to_owned();
            }
            state.decided.retain(|(n, _)| *n != name);
            state.decided.push((name, live.to_owned()));
            drop(state);
            let (tick, poll) = host.clock();
            let row = DeviceEvent::Decision(name.into(), live.into());
            host.write(tick, poll, encode(&row));
            live.to_owned()
        }
        Mode::Replay(schedule) => schedule
            .decision(name, live)
            .unwrap_or_else(|| live.to_owned()),
        Mode::Off => live.to_owned(),
    }
}

/// The recorded value of decision `name` in force now, in a replay that has reached its first row (not counted as
/// overridden: for state a replay is expected to reach at another tick than the recording).
pub fn recorded_decision(name: &str) -> Option<String> {
    match &state().mode {
        Mode::Replay(schedule) => schedule.recorded(name).map(str::to_owned),
        _ => None,
    }
}

/// The replay's gamepad entity index for a recorded pad index (`None` outside a replay or before its first row).
pub fn replay_pad_index(recorded: u32) -> Option<u32> {
    let state = state();
    matches!(state.mode, Mode::Replay(_))
        .then(|| state.pads.iter().find(|(id, _)| *id == recorded))
        .flatten()
        .map(|(_, entity)| entity.index())
}

/// The cursor a consumer must use: the recorded one (scaled to this window) while replaying.
pub fn cursor_position(window: &Window) -> Option<Vec2> {
    let state = state();
    match (&state.mode, state.cursor) {
        (Mode::Replay(_), Some((position, size))) => {
            let scale = Vec2::new(window.width(), window.height()) / size.max(Vec2::ONE);
            position.map(|position| position * scale)
        }
        (Mode::Replay(_), None) => None,
        _ => window.cursor_position(),
    }
}

/// The window focus a consumer must use: the recorded one while replaying.
pub fn focused(window: &Window) -> bool {
    let state = state();
    match (&state.mode, state.focus) {
        (Mode::Replay(_), Some(focus)) => focus,
        _ => window.focused,
    }
}

/// Stop line (replay only).
pub fn report() -> Option<String> {
    match &state().mode {
        Mode::Replay(schedule) => Some(schedule.summary()),
        _ => None,
    }
}

/// The intake's place: inside `InputSystem` (so after every backend, which Bevy orders before that set: gilrs
/// has no set of its own) and before the systems of the set that fold events into device state.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntakeSet;

pub fn install(app: &mut App) {
    use bevy_input::{gamepad, keyboard, mouse};
    app.configure_sets(
        PreUpdate,
        IntakeSet
            .in_set(InputSystem)
            .before(keyboard::keyboard_input_system)
            .before(mouse::mouse_button_input_system)
            .before(mouse::accumulate_mouse_motion_system)
            .before(mouse::accumulate_mouse_scroll_system)
            .before(gamepad::gamepad_connection_system)
            .before(gamepad::gamepad_event_processing_system),
    )
    .add_systems(PreUpdate, intake.in_set(IntakeSet))
    .add_systems(Update, processed);
}

#[derive(Default)]
pub struct Cursors {
    keys: EventCursor<KeyboardInput>,
    focus_lost: EventCursor<KeyboardFocusLost>,
    focus: EventCursor<WindowFocused>,
    buttons: EventCursor<MouseButtonInput>,
    wheel: EventCursor<MouseWheel>,
    motion: EventCursor<MouseMotion>,
    pads: EventCursor<RawGamepadEvent>,
    connections: EventCursor<GamepadConnectionEvent>,
    cursor: Option<(Option<Vec2>, Vec2)>,
}

/// PreUpdate, in [`IntakeSet`].
pub fn intake(world: &mut World, mut cursors: Local<Cursors>) {
    let Some(host) = HOST.get() else { return };
    let recording = match &state().mode {
        Mode::Off => return,
        Mode::Record => true,
        Mode::Replay(_) => false,
    };
    if recording {
        let (tick, poll) = host.clock();
        for event in capture(world, &mut cursors) {
            host.write(tick, poll, encode(&event));
        }
    } else {
        drop_live(world, host.as_ref());
        let due = match &mut state().mode {
            Mode::Replay(schedule) => schedule.due(host.replay_poll()),
            _ => Vec::new(),
        };
        inject(world, due);
    }
}

/// Update (after every PreUpdate consumer): the replayed rows were processed.
pub fn processed() {
    let done = match &mut state().mode {
        Mode::Replay(schedule) => schedule.processed(),
        _ => false,
    };
    if done && let Some(host) = HOST.get() {
        host.progress();
    }
}

fn read_new<E: Event + Clone>(world: &World, cursor: &mut EventCursor<E>) -> Vec<E> {
    world
        .get_resource::<Events<E>>()
        .map(|events| cursor.read(events).cloned().collect())
        .unwrap_or_default()
}

fn primary_window(world: &mut World) -> Option<(Entity, Window)> {
    let mut query = world.query_filtered::<(Entity, &Window), With<PrimaryWindow>>();
    query
        .single(world)
        .ok()
        .map(|(entity, window)| (entity, window.clone()))
}

/// The device events queued since the last update, in a fixed order; pad axes and mouse motion keep one
/// value per update (the state every consumer reads at the end of the update).
pub fn capture(world: &mut World, cursors: &mut Cursors) -> Vec<DeviceEvent> {
    let mut out = Vec::new();
    for key in read_new(world, &mut cursors.keys) {
        if key.key_code != MARK_HOTKEY {
            out.push(DeviceEvent::key(&key));
        }
    }
    for _ in read_new(world, &mut cursors.focus_lost) {
        out.push(DeviceEvent::FocusLost);
    }
    for focus in read_new(world, &mut cursors.focus) {
        out.push(DeviceEvent::Focus(focus.focused));
    }
    for button in read_new(world, &mut cursors.buttons) {
        out.push(DeviceEvent::MouseButton {
            button: button.button,
            pressed: button.state.is_pressed(),
        });
    }
    for wheel in read_new(world, &mut cursors.wheel) {
        out.push(DeviceEvent::Wheel {
            unit: wheel.unit,
            x: wheel.x,
            y: wheel.y,
        });
    }
    let motion: Vec2 = read_new(world, &mut cursors.motion)
        .iter()
        .map(|motion| motion.delta)
        .sum();
    if motion != Vec2::ZERO {
        out.push(DeviceEvent::Motion(motion));
    }
    // Startup connections may appear only in the separate queue. Later connection changes
    // are also in the raw stream, where their order relative to button/axis events matters.
    let connections = read_new(world, &mut cursors.connections);
    let pads = read_new(world, &mut cursors.pads);
    let mut raw_connections: Vec<_> = pads
        .iter()
        .filter_map(|event| match event {
            RawGamepadEvent::Connection(connection) => Some(connection),
            _ => None,
        })
        .collect();
    for connection in connections {
        if let Some(index) = raw_connections.iter().position(|raw| {
            raw.gamepad == connection.gamepad
                && matches!(
                    (&raw.connection, &connection.connection),
                    (
                        GamepadConnection::Connected { .. },
                        GamepadConnection::Connected { .. }
                    ) | (
                        GamepadConnection::Disconnected,
                        GamepadConnection::Disconnected
                    )
                )
        }) {
            raw_connections.remove(index);
        } else {
            out.push(pad_connection(connection));
        }
    }
    for event in pads {
        match event {
            RawGamepadEvent::Connection(connection) => out.push(pad_connection(connection)),
            RawGamepadEvent::Button(change) => out.push(DeviceEvent::PadButton {
                pad: change.gamepad.index(),
                button: change.button,
                value: change.value,
            }),
            RawGamepadEvent::Axis(change) => out.push(DeviceEvent::PadAxis {
                pad: change.gamepad.index(),
                axis: change.axis,
                value: change.value,
            }),
        }
    }
    if let Some((_, window)) = primary_window(world) {
        let now = (
            window.cursor_position(),
            Vec2::new(window.width(), window.height()),
        );
        if cursors.cursor != Some(now) {
            cursors.cursor = Some(now);
            out.push(DeviceEvent::Cursor {
                position: now.0,
                size: now.1,
            });
        }
    }
    out
}

fn pad_connection(connection: GamepadConnectionEvent) -> DeviceEvent {
    let pad = connection.gamepad.index();
    match connection.connection {
        GamepadConnection::Connected {
            name,
            vendor_id,
            product_id,
        } => DeviceEvent::PadConnected {
            pad,
            name,
            vendor: vendor_id,
            product: product_id,
        },
        GamepadConnection::Disconnected => DeviceEvent::PadDisconnected(pad),
    }
}

fn drain<E: Event>(world: &mut World) -> Vec<E> {
    world
        .get_resource_mut::<Events<E>>()
        .map(|mut events| events.drain().collect())
        .unwrap_or_default()
}

/// Replay: the live devices are not the player. Live keys are offered to the host (close chord) and dropped.
fn drop_live(world: &mut World, host: &dyn Host) {
    for key in drain::<KeyboardInput>(world) {
        host.live_key(&key);
    }
    drain::<KeyboardFocusLost>(world);
    drain::<WindowFocused>(world);
    drain::<MouseButtonInput>(world);
    drain::<MouseWheel>(world);
    drain::<MouseMotion>(world);
    drain::<CursorMoved>(world);
    drain::<CursorLeft>(world);
    drain::<RawGamepadEvent>(world);
    drain::<GamepadConnectionEvent>(world);
    drain::<RawGamepadButtonChangedEvent>(world);
    drain::<RawGamepadAxisChangedEvent>(world);
}

fn send<E: Event>(world: &mut World, event: E) {
    if let Some(mut events) = world.get_resource_mut::<Events<E>>() {
        events.send(event);
    }
}

/// Replay: put recorded events into the queues the live devices use.
pub fn inject(world: &mut World, due: Vec<DeviceEvent>) {
    if due.is_empty() && primary_window(world).is_some() {
        return;
    }
    let window = match primary_window(world) {
        Some((entity, _)) => entity,
        // Headless: a virtual primary window carries the recorded size, cursor and focus for the pages.
        None => world
            .spawn((Window::default(), PrimaryWindow, VirtualWindow))
            .id(),
    };
    for event in due {
        match event.clone() {
            DeviceEvent::Key { key, pressed, .. } => {
                if matches!(key, KeyCode::AltLeft | KeyCode::AltRight) {
                    let bit = if key == KeyCode::AltLeft { 1 } else { 2 };
                    let mut state = state();
                    state.alt_held = if pressed {
                        state.alt_held | bit
                    } else {
                        state.alt_held & !bit
                    };
                }
                if key == KeyCode::F4 && pressed && state().alt_held != 0 {
                    continue; // the recording's own quit chord: the route end stops a replay
                }
                if let Some(input) = event.keyboard_input(window) {
                    send(world, input);
                }
            }
            DeviceEvent::MouseButton { button, pressed } => send(
                world,
                MouseButtonInput {
                    button,
                    state: button_state(pressed),
                    window,
                },
            ),
            DeviceEvent::Wheel { unit, x, y } => send(world, MouseWheel { unit, x, y, window }),
            DeviceEvent::Motion(delta) => send(world, MouseMotion { delta }),
            DeviceEvent::Cursor { position, size } => {
                state().cursor = Some((position, size));
                virtual_window(world, window, |w| {
                    w.resolution.set(size.x, size.y);
                    w.set_cursor_position(position);
                });
                match position {
                    Some(position) => send(
                        world,
                        CursorMoved {
                            window,
                            position,
                            delta: None,
                        },
                    ),
                    None => send(world, CursorLeft { window }),
                }
            }
            DeviceEvent::Focus(focused) => {
                state().focus = Some(focused);
                virtual_window(world, window, |w| w.focused = focused);
                send(world, WindowFocused { window, focused });
            }
            DeviceEvent::FocusLost => send(world, KeyboardFocusLost),
            DeviceEvent::PadConnected {
                pad,
                name,
                vendor,
                product,
            } => {
                let gamepad = pad_entity(world, pad, false);
                connect(
                    world,
                    gamepad,
                    GamepadConnection::Connected {
                        name,
                        vendor_id: vendor,
                        product_id: product,
                    },
                );
            }
            DeviceEvent::PadDisconnected(pad) => {
                let gamepad = pad_entity(world, pad, false);
                connect(world, gamepad, GamepadConnection::Disconnected);
            }
            DeviceEvent::PadButton { pad, button, value } => {
                let change =
                    RawGamepadButtonChangedEvent::new(pad_entity(world, pad, true), button, value);
                send(world, change);
                send(world, RawGamepadEvent::Button(change));
            }
            DeviceEvent::PadAxis { pad, axis, value } => {
                let change =
                    RawGamepadAxisChangedEvent::new(pad_entity(world, pad, true), axis, value);
                send(world, change);
                send(world, RawGamepadEvent::Axis(change));
            }
            DeviceEvent::Route(..) | DeviceEvent::Decision(..) => {}
        }
    }
}

/// The headless virtual window takes the recorded state; a real window keeps its own (consumers ask
/// [`cursor_position`] and [`focused`]).
fn virtual_window(world: &mut World, window: Entity, apply: impl FnOnce(&mut Window)) {
    if world.get::<VirtualWindow>(window).is_some()
        && let Some(mut live) = world.get_mut::<Window>(window)
    {
        apply(&mut live);
    }
}

/// Marks the primary window the intake spawned for a headless replay.
#[derive(Component)]
pub struct VirtualWindow;

/// The replay entity of a recorded pad, spawned at its first row. A pad whose connection precedes the
/// recording (first row a button or axis) is connected first, so Bevy tracks its state.
fn pad_entity(world: &mut World, pad: u32, connect: bool) -> Entity {
    if let Some((_, entity)) = state().pads.iter().find(|(id, _)| *id == pad) {
        return *entity;
    }
    let entity = world.spawn_empty().id();
    state().pads.push((pad, entity));
    if connect {
        let name = format!("replayed pad {pad}");
        connect_pad(world, entity, name);
    }
    entity
}

fn connect_pad(world: &mut World, entity: Entity, name: String) {
    let connection = GamepadConnection::Connected {
        name,
        vendor_id: None,
        product_id: None,
    };
    connect(world, entity, connection);
}

/// A connection change in both forms gilrs sends during an update.
fn connect(world: &mut World, gamepad: Entity, connection: GamepadConnection) {
    let event = GamepadConnectionEvent::new(gamepad, connection);
    send(world, RawGamepadEvent::Connection(event.clone()));
    send(world, event);
}
