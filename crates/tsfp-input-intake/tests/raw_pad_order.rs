// SPDX-License-Identifier: GPL-3.0-or-later
use bevy_ecs::event::Events;
use bevy_ecs::prelude::World;
use bevy_input::gamepad::{
    GamepadAxis, GamepadButton, GamepadConnection, GamepadConnectionEvent,
    RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent, RawGamepadEvent,
};
use tsfp_input_intake::DeviceEvent;
use tsfp_input_intake::intake::{Cursors, capture};

#[test]
fn capture_preserves_interleaved_raw_pad_events_and_axis_transitions() {
    let mut world = World::new();
    world.init_resource::<Events<RawGamepadEvent>>();
    let pad = world.spawn_empty().id();
    world.send_event(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
        pad,
        GamepadAxis::LeftStickX,
        0.75,
    )));
    world.send_event(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
        pad,
        GamepadButton::South,
        1.0,
    )));
    world.send_event(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
        pad,
        GamepadAxis::LeftStickX,
        0.0,
    )));
    let actual = capture(&mut world, &mut Cursors::default());
    assert_eq!(
        actual,
        vec![
            DeviceEvent::PadAxis {
                pad: pad.index(),
                axis: GamepadAxis::LeftStickX,
                value: 0.75
            },
            DeviceEvent::PadButton {
                pad: pad.index(),
                button: GamepadButton::South,
                value: 1.0
            },
            DeviceEvent::PadAxis {
                pad: pad.index(),
                axis: GamepadAxis::LeftStickX,
                value: 0.0
            },
        ]
    );
}

#[test]
fn connection_changes_follow_raw_pad_events_without_duplicates() {
    let mut world = World::new();
    world.init_resource::<Events<RawGamepadEvent>>();
    world.init_resource::<Events<GamepadConnectionEvent>>();
    let pad = world.spawn_empty().id();
    let startup = GamepadConnectionEvent::new(
        pad,
        GamepadConnection::Connected {
            name: "pad".into(),
            vendor_id: None,
            product_id: None,
        },
    );
    world.send_event(startup);
    world.send_event(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
        pad,
        GamepadButton::South,
        1.0,
    )));
    let disconnect = GamepadConnectionEvent::new(pad, GamepadConnection::Disconnected);
    world.send_event(disconnect.clone());
    world.send_event(RawGamepadEvent::Connection(disconnect));
    let actual = capture(&mut world, &mut Cursors::default());
    assert_eq!(
        actual,
        vec![
            DeviceEvent::PadConnected {
                pad: pad.index(),
                name: "pad".into(),
                vendor: None,
                product: None
            },
            DeviceEvent::PadButton {
                pad: pad.index(),
                button: GamepadButton::South,
                value: 1.0
            },
            DeviceEvent::PadDisconnected(pad.index()),
        ]
    );
}
