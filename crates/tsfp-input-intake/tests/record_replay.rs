// SPDX-License-Identifier: GPL-3.0-or-later
//! T3820 end to end: a scripted session (FABRICATED device events, as winit and gilrs would queue them) drives a
//! native page that reads Bevy input the way the shell's pages do; the intake records it into a real input.rec v6
//! stream; a second, headless app replays the file with live noise on every device and must reach the same page
//! state. The page has no recording or replay code of its own (a new page is covered without a hook).

use bevy_app::{App, PreUpdate};
use bevy_ecs::prelude::*;
use bevy_input::gamepad::{
    Gamepad, GamepadButton, GamepadConnection, GamepadConnectionEvent,
    RawGamepadButtonChangedEvent, RawGamepadEvent,
};
use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy_input::mouse::{MouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel};
use bevy_input::{ButtonInput, ButtonState, InputPlugin, InputSystem};
use bevy_math::Vec2;
use bevy_window::{PrimaryWindow, Window, WindowPlugin};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tsfp_input::recording::{Payload, Reader, Record, Writer};
use tsfp_input_intake::RouteKind;

static POLL: AtomicU64 = AtomicU64::new(0);
static WRITER: Mutex<Option<Writer<Vec<u8>>>> = Mutex::new(None);
static LIVE_KEYS: AtomicU64 = AtomicU64::new(0);
static PROGRESS: AtomicU64 = AtomicU64::new(0);

struct TestHost;

impl tsfp_input_intake::Host for TestHost {
    fn clock(&self) -> (u64, u64) {
        let poll = POLL.load(Ordering::SeqCst);
        (poll / 3, poll)
    }
    fn write(&self, sim_tick: u64, host_poll: u64, payload: String) {
        let row = Payload {
            sim_tick,
            host_poll,
            wall_ns: host_poll * 1000,
            payload,
        };
        WRITER
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .device(&row)
            .unwrap();
    }
    fn replay_poll(&self) -> u64 {
        POLL.load(Ordering::SeqCst)
    }
    fn live_key(&self, _key: &KeyboardInput) {
        LIVE_KEYS.fetch_add(1, Ordering::SeqCst);
    }
    fn progress(&self) {
        PROGRESS.fetch_add(1, Ordering::SeqCst);
    }
}

/// The fabricated native page: a name field, a value, pad presses, clicks, wheel.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
struct Page {
    /// Enter opens the name field; typed text before that goes nowhere.
    open: bool,
    name: String,
    value: i32,
    pad_accepts: u32,
    clicks: Vec<(i32, i32)>,
    wheel: i32,
}

fn page(
    mut page: ResMut<Page>,
    mut typed: EventReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut wheel: EventReader<MouseWheel>,
) {
    for key in typed.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
        let route = match &key.logical_key {
            Key::Enter => {
                page.open = true;
                "page-open"
            }
            Key::Character(text) if page.open => {
                page.name.push_str(text);
                "page-text"
            }
            Key::Backspace if page.open => {
                page.name.pop();
                "page-text"
            }
            _ => "page",
        };
        tsfp_input_intake::route(RouteKind::Key, route);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        page.value += 1;
    }
    if keys.pressed(KeyCode::ShiftLeft) && keys.just_pressed(KeyCode::ArrowLeft) {
        page.value -= 10;
    }
    page.pad_accepts += pads
        .iter()
        .filter(|pad| pad.just_pressed(GamepadButton::South))
        .count() as u32;
    if mouse.just_pressed(MouseButton::Left)
        && let Ok(window) = window.single()
        && let Some(at) = tsfp_input_intake::cursor_position(window)
    {
        page.clicks.push((at.x as i32, at.y as i32));
    }
    page.wheel += wheel.read().map(|event| event.y as i32).sum::<i32>();
}

/// A device backend inside the update, like gilrs (`before(InputSystem)`, no set to order against): during
/// the replay it queues live pad presses that must not reach the page.
fn late_backend(mut events: EventWriter<RawGamepadEvent>, pads: Query<Entity, With<Gamepad>>) {
    if tsfp_input_intake::replaying() {
        for pad in &pads {
            events.write(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
                pad,
                GamepadButton::South,
                (POLL.load(Ordering::SeqCst) % 2) as f32,
            )));
        }
    }
}

fn app(primary_window: Option<Window>) -> App {
    let mut app = App::new();
    app.add_plugins((
        InputPlugin,
        WindowPlugin {
            primary_window,
            exit_condition: bevy_window::ExitCondition::DontExit,
            close_when_requested: false,
        },
    ))
    .init_resource::<Page>()
    .add_systems(PreUpdate, page.after(InputSystem))
    .add_systems(PreUpdate, late_backend.before(InputSystem));
    tsfp_input_intake::install(&mut app);
    app
}

fn key(world: &mut World, key_code: KeyCode, logical: Key, pressed: bool) {
    let window = primary(world);
    world.send_event(KeyboardInput {
        key_code,
        logical_key: logical,
        state: if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        },
        text: None,
        repeat: false,
        window,
    });
}

fn primary(world: &mut World) -> Entity {
    world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .unwrap_or(Entity::PLACEHOLDER)
}

/// What the device backends queue before update `step` of the scripted session.
fn script(world: &mut World, step: u64, pad: Entity) {
    let tap = |world: &mut World, code, logical: Key| {
        key(world, code, logical.clone(), true);
        key(world, code, logical, false);
    };
    match step {
        2 => {
            // As gilrs during an update: both forms of the connection.
            let event = GamepadConnectionEvent::new(
                pad,
                GamepadConnection::Connected {
                    name: "fabricated pad".into(),
                    vendor_id: Some(0x045e),
                    product_id: Some(0x02ea),
                },
            );
            world.send_event(RawGamepadEvent::Connection(event.clone()));
            world.send_event(event);
        }
        3 => tap(world, KeyCode::KeyQ, Key::Character("q".into())), // before the field opens
        4 => {
            tap(world, KeyCode::Enter, Key::Enter);
            tap(world, KeyCode::KeyR, Key::Character("R".into()));
        }
        5 => tap(world, KeyCode::KeyO, Key::Character("o".into())),
        6 => tap(world, KeyCode::KeyX, Key::Character("x".into())),
        7 => tap(world, KeyCode::Backspace, Key::Backspace),
        8 => tap(world, KeyCode::KeyB, Key::Character("b".into())),
        9 => key(world, KeyCode::ArrowRight, Key::ArrowRight, true),
        11 => key(world, KeyCode::ArrowRight, Key::ArrowRight, false),
        12 => tap(world, KeyCode::ArrowRight, Key::ArrowRight),
        13 => key(world, KeyCode::ShiftLeft, Key::Shift, true),
        14 => tap(world, KeyCode::ArrowLeft, Key::ArrowLeft),
        15 => key(world, KeyCode::ShiftLeft, Key::Shift, false),
        16 | 17 | 19 | 20 => {
            let value = if step % 3 == 1 { 1.0 } else { 0.0 };
            world.send_event(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
                pad,
                GamepadButton::South,
                value,
            )));
        }
        22 => {
            let window = primary(world);
            world
                .get_mut::<Window>(window)
                .unwrap()
                .set_cursor_position(Some(Vec2::new(640.0, 360.0)));
            world.send_event(MouseButtonInput {
                button: MouseButton::Left,
                state: ButtonState::Pressed,
                window,
            });
        }
        23 => {
            let window = primary(world);
            world.send_event(MouseButtonInput {
                button: MouseButton::Left,
                state: ButtonState::Released,
                window,
            });
            world.send_event(MouseWheel {
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y: -2.0,
                window,
            });
        }
        _ => {}
    }
}

/// Live devices during the replay: none of this may reach the page.
fn noise(world: &mut World, step: u64, pad: Entity) {
    if step == 1 {
        world.send_event(GamepadConnectionEvent::new(
            pad,
            GamepadConnection::Connected {
                name: "live pad".into(),
                vendor_id: None,
                product_id: None,
            },
        ));
    }
    key(
        world,
        KeyCode::KeyZ,
        Key::Character("z".into()),
        step % 2 == 0,
    );
    key(world, KeyCode::ArrowRight, Key::ArrowRight, step % 2 == 0);
    world.send_event(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
        pad,
        GamepadButton::South,
        (step % 2) as f32,
    )));
    let window = primary(world);
    world.send_event(MouseButtonInput {
        button: MouseButton::Left,
        state: if step % 2 == 0 {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        },
        window,
    });
}

const STEPS: u64 = 30;

#[test]
fn a_recorded_session_replays_to_the_same_page_state_headless_with_live_noise() {
    tsfp_input_intake::set_host(Box::new(TestHost));

    // Record, windowed.
    *WRITER.lock().unwrap() = Some(Writer::stamped(Vec::new(), false, Some("t3820test")).unwrap());
    let mut recording = app(Some(Window::default()));
    let pad = recording.world_mut().spawn_empty().id();
    tsfp_input_intake::start_recording();
    for step in 0..STEPS {
        POLL.store(step, Ordering::SeqCst);
        script(recording.world_mut(), step, pad);
        recording.update();
    }
    tsfp_input_intake::stop();
    let recorded = recording.world().resource::<Page>().clone();
    assert_eq!(
        recorded,
        Page {
            open: true,
            name: "Rob".into(),
            value: -8,
            pad_accepts: 2,
            clicks: vec![(640, 360)],
            wheel: -2,
        },
        "the scripted session itself"
    );
    let bytes = WRITER.lock().unwrap().take().unwrap().finish().unwrap();

    // Replay the file, headless, with live noise.
    let mut reader = Reader::new(bytes.as_slice()).unwrap();
    assert_eq!((reader.version(), reader.build()), (6, Some("t3820test")));
    let mut rows = Vec::new();
    while let Some(record) = reader.next().unwrap() {
        if let Record::Device(row) = record {
            rows.push((row.host_poll, row.payload));
        }
    }
    assert!(
        rows.iter()
            .any(|(_, payload)| payload.starts_with("r:k:page"))
    );
    tsfp_input_intake::start_replay(rows);
    let mut replay = app(None);
    let live_pad = replay.world_mut().spawn_empty().id();
    for step in 0..STEPS + 2 {
        POLL.store(step, Ordering::SeqCst);
        noise(replay.world_mut(), step, live_pad);
        replay.update();
    }
    let report = tsfp_input_intake::report().unwrap();
    tsfp_input_intake::stop();
    assert_eq!(replay.world().resource::<Page>(), &recorded, "{report}");
    assert!(
        report.contains("undelivered=0")
            && report.contains("undecodable=0")
            && report.contains("mismatched=0"),
        "{report}"
    );
    assert!(
        LIVE_KEYS.load(Ordering::SeqCst) > 0,
        "live keys go to the host only"
    );
    assert!(PROGRESS.load(Ordering::SeqCst) > 0, "the lockstep is woken");
    assert_eq!(tsfp_input_intake::ready_poll(), u64::MAX);
    let virtual_windows = replay
        .world_mut()
        .query_filtered::<&Window, With<tsfp_input_intake::VirtualWindow>>()
        .iter(replay.world())
        .count();
    assert_eq!(
        virtual_windows, 1,
        "headless replay gets one virtual window"
    );
}
