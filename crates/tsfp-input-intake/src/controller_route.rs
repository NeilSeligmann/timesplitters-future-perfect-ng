// SPDX-License-Identifier: GPL-3.0-or-later
//! Converts Bevy's raw gamepad events into the profile-aware Rust controller path.

use bevy_input::gamepad::{
    GamepadAxis, GamepadButton, GamepadConnection, GamepadConnectionEvent, RawGamepadEvent,
};
use tsfp_input::controller::{
    ControllerEventPath, ControllerIdentity, ControllerProfile, fallback_gamepad_guid,
};
use tsfp_input::router::{DeviceEvent, Router};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionChange {
    Connected(u32),
    Disconnected(u32),
    None,
}

/// Register a physical controller when its host backend supplies SDL identity metadata.
///
/// The Bevy event adapter below only has a name and vendor/product IDs, so it uses a weaker
/// synthetic GUID. A native SDL3 event source should pass SDL's real GUID, serial, and path here.
pub fn connect_with_metadata(
    router: &mut Router,
    controllers: &mut ControllerEventPath,
    runtime_id: u32,
    guid: &str,
    serial: Option<&str>,
    path: Option<&str>,
    name: &str,
) -> Result<ControllerIdentity, String> {
    controllers.connect(router, runtime_id, guid, serial, path, name)
}

fn connect_bevy(
    router: &mut Router,
    controllers: &mut ControllerEventPath,
    runtime_id: u32,
    guid: &str,
    name: &str,
) -> Result<ControllerIdentity, String> {
    let identity = connect_with_metadata(router, controllers, runtime_id, guid, None, None, name)?;
    if !controllers.profiles().contains(&identity) {
        let mut profile = ControllerProfile::default();
        profile.calibration[1].invert = false;
        profile.calibration[3].invert = false;
        controllers.set_profile(router, identity.clone(), profile)?;
    }
    Ok(identity)
}

/// Route a host event through its selected profile, returning connection bookkeeping to the shell.
pub fn route(
    router: &mut Router,
    controllers: &mut ControllerEventPath,
    event: &RawGamepadEvent,
) -> ConnectionChange {
    match event {
        RawGamepadEvent::Connection(connection) => {
            let id = connection.gamepad.index();
            match &connection.connection {
                GamepadConnection::Connected {
                    name,
                    vendor_id,
                    product_id,
                } => {
                    let guid = fallback_gamepad_guid(name.as_str(), *vendor_id, *product_id);
                    match connect_bevy(router, controllers, id, &guid, name.as_str()) {
                        Ok(_) => ConnectionChange::Connected(id),
                        Err(error) => {
                            eprintln!(
                                "input router: cannot register gamepad {id} ({name}): {error}"
                            );
                            // The shell must not publish a pad that the profile-aware route
                            // refused. Otherwise it advertises a connected device that cannot
                            // deliver any button or axis events.
                            ConnectionChange::None
                        }
                    }
                }
                GamepadConnection::Disconnected => {
                    if !controllers.disconnect(router, id) {
                        router.feed(DeviceEvent::PadDisconnected(id));
                    }
                    ConnectionChange::Disconnected(id)
                }
            }
        }
        RawGamepadEvent::Button(change) => {
            let pad = change.gamepad.index();
            if let Some(button) = physical_button(change.button) {
                if let Err(error) = controllers.set_button(router, pad, button, change.value > 0.5)
                {
                    eprintln!("input router: cannot route gamepad {pad} button: {error}");
                }
            } else if let Some(axis) = trigger_axis(change.button) {
                if let Err(error) =
                    controllers.set_axis(router, pad, axis, trigger_raw(change.value))
                {
                    eprintln!("input router: cannot route gamepad {pad} trigger axis: {error}");
                }
            }
            ConnectionChange::None
        }
        RawGamepadEvent::Axis(change) => {
            if let Some(axis) = stick_axis(change.axis) {
                if let Err(error) = controllers.set_axis(
                    router,
                    change.gamepad.index(),
                    axis,
                    stick_raw(change.value),
                ) {
                    eprintln!("input router: cannot route gamepad axis: {error}");
                }
            }
            ConnectionChange::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_input::router::RouterConfig;

    #[test]
    fn host_metadata_selects_a_stable_serial_profile() {
        let mut router = Router::new(RouterConfig::default());
        let mut controllers = ControllerEventPath::default();
        let identity = connect_with_metadata(
            &mut router,
            &mut controllers,
            3,
            "0123456789abcdef0123456789abcdef",
            Some("serial-123"),
            Some("/dev/input/by-path/pad"),
            "Pad",
        )
        .unwrap();

        assert!(identity.profile_key().contains("-s73657269616c2d313233"));
        // SDL defaults are implicit until the user saves an explicit profile.
        assert!(!controllers.profiles().contains(&identity));
        let profile = controllers.profiles().profile(&identity);
        assert!(profile.calibration[1].invert);
        assert!(profile.calibration[3].invert);
    }

    #[test]
    fn bevy_connection_uses_bevy_axis_orientation() {
        let mut router = Router::new(RouterConfig::default());
        let mut controllers = ControllerEventPath::default();
        let identity = connect_bevy(
            &mut router,
            &mut controllers,
            3,
            "0123456789abcdef0123456789abcdef",
            "Pad",
        )
        .unwrap();
        let profile = controllers.profiles().profile(&identity);
        assert!(!profile.calibration[1].invert);
        assert!(!profile.calibration[3].invert);
    }

    #[test]
    fn rejected_duplicate_runtime_connection_is_not_published_as_connected() {
        use bevy_ecs::entity::Entity;

        let mut router = Router::new(RouterConfig::default());
        let mut controllers = ControllerEventPath::default();
        let event = RawGamepadEvent::Connection(GamepadConnectionEvent::new(
            Entity::from_raw(7),
            GamepadConnection::Connected {
                name: "Pad".into(),
                vendor_id: Some(0x1234),
                product_id: Some(0x5678),
            },
        ));

        assert_eq!(
            route(&mut router, &mut controllers, &event),
            ConnectionChange::Connected(7)
        );
        assert_eq!(
            route(&mut router, &mut controllers, &event),
            ConnectionChange::None
        );
        assert_eq!(controllers.settings_snapshot().0.len(), 1);
    }
}

fn physical_button(button: GamepadButton) -> Option<u8> {
    Some(match button {
        GamepadButton::South => 0,
        GamepadButton::East => 1,
        GamepadButton::West => 2,
        GamepadButton::North => 3,
        GamepadButton::Select => 4,
        GamepadButton::Mode => 5,
        GamepadButton::Start => 6,
        GamepadButton::LeftThumb => 7,
        GamepadButton::RightThumb => 8,
        GamepadButton::LeftTrigger => 9,
        GamepadButton::RightTrigger => 10,
        GamepadButton::DPadUp => 11,
        GamepadButton::DPadDown => 12,
        GamepadButton::DPadLeft => 13,
        GamepadButton::DPadRight => 14,
        _ => return None,
    })
}

fn trigger_axis(button: GamepadButton) -> Option<u8> {
    match button {
        GamepadButton::LeftTrigger2 => Some(4),
        GamepadButton::RightTrigger2 => Some(5),
        _ => None,
    }
}

fn stick_axis(axis: GamepadAxis) -> Option<u8> {
    match axis {
        GamepadAxis::LeftStickX => Some(0),
        GamepadAxis::LeftStickY => Some(1),
        GamepadAxis::RightStickX => Some(2),
        GamepadAxis::RightStickY => Some(3),
        _ => None,
    }
}

fn stick_raw(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

fn trigger_raw(value: f32) -> i16 {
    (value.clamp(0.0, 1.0) * 32767.0).round() as i16
}
/// Consume settings edits on live input updates, including updates without controller events.
/// Leave them pending during replay so they cannot change recorded controller input.
pub fn process_pending_profile_edit<T>(
    replaying: bool,
    take: impl FnOnce() -> Option<T>,
    apply: impl FnOnce(T),
) {
    if !replaying {
        if let Some(edit) = take() {
            apply(edit);
        }
    }
}

#[cfg(test)]
mod profile_edit_schedule_tests {
    use super::process_pending_profile_edit;
    use std::cell::Cell;

    #[test]
    fn profile_edits_are_processed_without_pad_events_and_deferred_during_replay() {
        let pending = Cell::new(Some(7));
        let applied = Cell::new(None);
        process_pending_profile_edit(true, || pending.take(), |edit| applied.set(Some(edit)));
        assert_eq!(pending.get(), Some(7));
        assert_eq!(applied.get(), None);
        process_pending_profile_edit(false, || pending.take(), |edit| applied.set(Some(edit)));
        assert_eq!(pending.get(), None);
        assert_eq!(applied.get(), Some(7));
    }
}
