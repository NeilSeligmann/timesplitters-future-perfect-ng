// SPDX-License-Identifier: GPL-3.0-or-later
//! T3820: a replay never closes itself with the recording's own Alt+F4; every other recorded key is delivered.

use bevy_app::App;
use bevy_ecs::event::Events;
use bevy_input::InputPlugin;
use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
use tsfp_input_intake::DeviceEvent;

fn key(key: KeyCode, pressed: bool) -> DeviceEvent {
    DeviceEvent::Key {
        key,
        logical: Key::Unidentified(bevy_input::keyboard::NativeKey::Unidentified),
        pressed,
        repeat: false,
        text: None,
    }
}

#[test]
fn the_recorded_quit_chord_is_not_replayed() {
    let mut app = App::new();
    app.add_plugins(InputPlugin);
    tsfp_input_intake::start_replay(Vec::new());
    tsfp_input_intake::intake::inject(
        app.world_mut(),
        vec![
            key(KeyCode::F4, true),
            key(KeyCode::F4, false),
            key(KeyCode::AltLeft, true),
            key(KeyCode::F4, true),
            key(KeyCode::F4, false),
            key(KeyCode::AltLeft, false),
            key(KeyCode::F4, true),
        ],
    );
    tsfp_input_intake::stop();
    let mut events = app.world_mut().resource_mut::<Events<KeyboardInput>>();
    let delivered: Vec<_> = events
        .drain()
        .map(|event| (event.key_code, event.state.is_pressed()))
        .collect();
    assert_eq!(
        delivered,
        vec![
            (KeyCode::F4, true),
            (KeyCode::F4, false),
            (KeyCode::AltLeft, true),
            (KeyCode::F4, false),
            (KeyCode::AltLeft, false),
            (KeyCode::F4, true),
        ]
    );
}
