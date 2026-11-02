use super::*;
use crate::shim::{XInputPad, held_bits, xinput_from_player, xinput_from_report};
use std::collections::{HashMap, HashSet};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        // special values hit the exact edges of the conversions
        match self.next() % 12 {
            0 => 0.0,
            1 => 1.0,
            2 => -1.0,
            3 => 0.5,
            4 => 0.5000001,
            5 => 2.5,
            6 => -3.0,
            _ => (self.next() % 2_000_001) as f32 / 1_000_000.0 - 1.0,
        }
    }
}

const BUTTONS: [PadButton; 15] = [
    PadButton::South,
    PadButton::East,
    PadButton::West,
    PadButton::North,
    PadButton::Select,
    PadButton::Mode,
    PadButton::Start,
    PadButton::LeftThumb,
    PadButton::RightThumb,
    PadButton::LeftBumper,
    PadButton::RightBumper,
    PadButton::DPadUp,
    PadButton::DPadDown,
    PadButton::DPadLeft,
    PadButton::DPadRight,
];
const AXES: [PadAxis; 6] = [
    PadAxis::LeftX,
    PadAxis::LeftY,
    PadAxis::RightX,
    PadAxis::RightY,
    PadAxis::LeftTrigger,
    PadAxis::RightTrigger,
];

/// The previous gamepad path of tsfp-bevy/src/events.rs, transcribed (SDL convention events, per pad bookkeeping).
#[derive(Default)]
struct OldPath {
    held: HashSet<u8>,
    last_axis: HashMap<u8, i16>,
    out: Vec<ShimEvent>,
}
impl OldPath {
    fn button(&mut self, button: PadButton, value: f32) {
        let index = button.sdl_index();
        let down = value > 0.5;
        let held = self.held.contains(&index);
        if down && !held {
            self.held.insert(index);
            self.out.push(ShimEvent::ButtonDown(index));
        } else if !down && held {
            self.held.remove(&index);
            self.out.push(ShimEvent::ButtonUp(index));
        }
    }
    #[allow(clippy::neg_multiply)] // transcription of the old path
    fn axis(&mut self, axis: PadAxis, value: f32) {
        let (index, v) = match axis {
            PadAxis::LeftX => (0, to_i16(value)),
            PadAxis::LeftY => (1, to_i16(value * -1.0)),
            PadAxis::RightX => (2, to_i16(value)),
            PadAxis::RightY => (3, to_i16(value * -1.0)),
            PadAxis::LeftTrigger => (4, to_i16(value.max(0.0))),
            PadAxis::RightTrigger => (5, to_i16(value.max(0.0))),
        };
        if self.last_axis.insert(index, v) != Some(v) {
            self.out.push(ShimEvent::Axis {
                axis: index,
                value: v,
            });
        }
    }
}

/// Interprets the queue like xinput_host_source.c (held buttons, int16 axes with the Y negation of window_input).
#[derive(Default)]
struct Queue {
    report: SdlReport,
}
impl Queue {
    fn apply(&mut self, event: ShimEvent) {
        match event {
            ShimEvent::ButtonDown(i) => self.report.buttons |= 1 << i,
            ShimEvent::ButtonUp(i) => self.report.buttons &= !(1 << i),
            ShimEvent::Axis { axis, value } => self.report.axes[usize::from(axis)] = Some(value),
        }
    }
}

fn pad_router() -> Router {
    Router::new(RouterConfig::default())
}

#[test]
fn pad_stream_through_the_router_equals_the_old_path_byte_for_byte() {
    for seed in 1..=20u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut router = pad_router();
        router.feed(DeviceEvent::PadConnected(3));
        let mut old = OldPath::default();
        let mut queue = Queue::default();
        let mut got = Vec::new();
        for step in 0..5_000 {
            if rng.next() % 2 == 0 {
                let button = BUTTONS[(rng.next() % 15) as usize];
                let value = if rng.next() % 3 == 0 {
                    rng.unit()
                } else {
                    (rng.next() % 2) as f32
                };
                router.feed(DeviceEvent::PadButton {
                    pad: 3,
                    button,
                    value,
                });
                old.button(button, value);
            } else {
                let axis = AXES[(rng.next() % 6) as usize];
                let value = rng.unit();
                router.feed(DeviceEvent::PadAxis {
                    pad: 3,
                    axis,
                    value,
                });
                old.axis(axis, value);
            }
            if step % 5 == 0 {
                let inputs = router.tick();
                for (slot, event) in router.drain_events() {
                    assert_eq!(slot, 0);
                    queue.apply(event);
                    got.push(event);
                }
                // the PlayerInput, converted back by the shim, is the state the queue holds (axes not yet sent read 0)
                let input = inputs[0].expect("pad slot has an input");
                assert_eq!(input.source, SourceKind::Pad(3));
                assert_eq!(input.assists, AssistMask::PROFILE);
                let from_input = xinput_from_player(&input);
                let from_queue = xinput_from_report(&queue.report);
                assert_eq!(from_input, from_queue, "seed {seed} step {step}");
                // buttons: the GUIDE bit has no guest bit, everything else must agree
                assert_eq!(input.held, held_bits(&from_queue));
            }
        }
        got.extend(router.drain_events().into_iter().map(|(_, event)| event));
        assert!(!old.out.is_empty());
        if got.len() != old.out.len() {
            let common = got.iter().zip(&old.out).take_while(|(a, b)| a == b).count();
            panic!(
                "seed {seed} event count: got {}, old {}; first difference at {common}: got {:?}, old {:?}",
                got.len(), old.out.len(), got.get(common), old.out.get(common)
            );
        }
        for (index, (actual, expected)) in got.iter().zip(&old.out).enumerate() {
            assert_eq!(actual, expected, "seed {seed} event {index}");
        }
    }
}

#[test]
fn pressed_and_released_are_one_tick_edges() {
    let mut router = pad_router();
    router.feed(DeviceEvent::PadConnected(0));
    router.feed(DeviceEvent::PadButton {
        pad: 0,
        button: PadButton::South,
        value: 1.0,
    });
    let first = router.tick()[0].unwrap();
    assert_eq!(
        (first.held, first.pressed, first.released),
        (bits::A, bits::A, 0)
    );
    let second = router.tick()[0].unwrap();
    assert_eq!(
        (second.held, second.pressed, second.released),
        (bits::A, 0, 0)
    );
    router.feed(DeviceEvent::PadButton {
        pad: 0,
        button: PadButton::South,
        value: 0.0,
    });
    let third = router.tick()[0].unwrap();
    assert_eq!((third.held, third.pressed, third.released), (0, 0, bits::A));
    assert_eq!(third.step, 2);
}

#[test]
fn trigger_is_axis_and_button_with_the_strict_threshold() {
    let mut router = pad_router();
    router.feed(DeviceEvent::PadConnected(0));
    // byte = (round(t * 32767) >> 7); 60 is not enough, 61 is
    router.feed(DeviceEvent::PadAxis {
        pad: 0,
        axis: PadAxis::LeftTrigger,
        value: 61.0 * 128.0 / 32767.0,
    });
    router.feed(DeviceEvent::PadAxis {
        pad: 0,
        axis: PadAxis::RightTrigger,
        value: 60.0 * 128.0 / 32767.0,
    });
    let input = router.tick()[0].unwrap();
    assert_eq!(input.held, bits::LT);
    assert!(input.triggers[0] > 0.2 && input.triggers[1] > 0.2);
}

#[test]
fn second_pad_takes_the_next_slot_and_is_not_forwarded_on_slot_zero() {
    let mut router = pad_router();
    router.feed(DeviceEvent::PadConnected(10));
    router.feed(DeviceEvent::PadConnected(11));
    assert_eq!(router.source(0), Some(InputSource::Pad(10)));
    assert_eq!(router.source(1), Some(InputSource::Pad(11)));
    router.feed(DeviceEvent::PadButton {
        pad: 11,
        button: PadButton::East,
        value: 1.0,
    });
    assert_eq!(router.drain_events(), vec![(1, ShimEvent::ButtonDown(1))]);
}

#[test]
fn disconnect_releases_what_was_held() {
    let mut router = pad_router();
    router.feed(DeviceEvent::PadConnected(0));
    router.feed(DeviceEvent::PadButton {
        pad: 0,
        button: PadButton::North,
        value: 1.0,
    });
    router.feed(DeviceEvent::PadAxis {
        pad: 0,
        axis: PadAxis::LeftX,
        value: 1.0,
    });
    router.drain_events();
    router.feed(DeviceEvent::PadDisconnected(0));
    assert_eq!(
        router.drain_events(),
        vec![
            (0, ShimEvent::ButtonUp(3)),
            (0, ShimEvent::Axis { axis: 0, value: 0 })
        ]
    );
    assert_eq!(router.source(0), None);
    assert!(router.tick()[0].is_none());
}

fn mk_router() -> Router {
    Router::new(RouterConfig {
        mouse_keyboard_slot: Some(0),
        ..RouterConfig::default()
    })
}

fn key(router: &mut Router, name: &str, down: bool) {
    router.feed(DeviceEvent::Key {
        name: name.to_string(),
        down,
    });
}

#[test]
fn keyboard_becomes_a_virtual_pad_with_normalised_diagonals() {
    let mut router = mk_router();
    assert_eq!(router.source(0), Some(InputSource::MouseKeyboard));
    assert!(router.mouse_keyboard_active());
    key(&mut router, "KeyW", true);
    key(&mut router, "KeyD", true);
    key(&mut router, "Space", true);
    let input = router.tick()[0].unwrap();
    assert_eq!(input.source, SourceKind::MouseKeyboard);
    assert_eq!(input.assists, AssistMask::NONE);
    assert_eq!(input.held, bits::A);
    let length = input.move_axis.x.hypot(input.move_axis.y);
    assert!((length - 1.0).abs() < 1e-6 && input.move_axis.x > 0.0 && input.move_axis.y > 0.0);
    let events: Vec<_> = router.drain_events().into_iter().map(|(_, e)| e).collect();
    assert!(events.contains(&ShimEvent::ButtonDown(0)));
    // forward is up positive: the SDL LY axis is negated
    assert!(events.contains(&ShimEvent::Axis {
        axis: 1,
        value: -23170
    }));
    assert!(events.contains(&ShimEvent::Axis {
        axis: 0,
        value: 23170
    }));
    key(&mut router, "ShiftLeft", true);
    let walking = router.tick()[0].unwrap();
    assert!((walking.move_axis.x.hypot(walking.move_axis.y) - 0.5).abs() < 1e-6);
    key(&mut router, "KeyW", false);
    key(&mut router, "KeyD", false);
    key(&mut router, "ShiftLeft", false);
    key(&mut router, "Space", false);
    router.tick();
    let release: Vec<_> = router.drain_events().into_iter().map(|(_, e)| e).collect();
    assert!(
        release.contains(&ShimEvent::ButtonUp(0))
            && release.contains(&ShimEvent::Axis { axis: 0, value: 0 })
    );
}

#[test]
fn mouse_buttons_wheel_and_escape_are_buttons() {
    let mut router = mk_router();
    router.feed(DeviceEvent::MouseButton {
        button: MouseButtonId::Left,
        down: true,
    });
    key(&mut router, "Escape", true);
    let input = router.tick()[0].unwrap();
    assert_eq!(
        input.held,
        bits::RT | bits::START,
        "Escape is the pad START (T2178)"
    );
    assert_eq!(input.triggers, [0.0, 1.0]);
    router.feed(DeviceEvent::Wheel { notches: 1.0 });
    for tick in 0..WHEEL_PULSE_TICKS {
        let held = router.tick()[0].unwrap().held;
        assert_ne!(held & bits::DPAD_RIGHT, 0, "tick {tick}");
    }
    assert_eq!(router.tick()[0].unwrap().held & bits::DPAD_RIGHT, 0);
}

#[test]
fn raw_motion_is_summed_across_events_and_ticks_with_no_loss() {
    let mut router = mk_router();
    let mut yaw = 0.0f64;
    let mut sent = 0.0f64;
    for i in 0..1_000 {
        let dx = f64::from(i % 5) * 0.3 - 0.4;
        router.feed(DeviceEvent::MouseMotion { dx, dy: 1.0 });
        sent += dx;
        if i % 7 == 0 {
            if let Look::Delta { yaw_deg, pitch_deg } = router.tick()[0].unwrap().look {
                yaw += f64::from(yaw_deg);
                assert!(pitch_deg < 0.0, "mouse down looks down");
            }
        }
    }
    if let Look::Delta { yaw_deg, .. } = router.tick()[0].unwrap().look {
        yaw += f64::from(yaw_deg);
    }
    assert!(
        (yaw - sent * 0.022).abs() < 1e-3,
        "{yaw} vs {}",
        sent * 0.022
    );
}

#[test]
fn mouse_look_reaches_the_guest_as_the_interim_virtual_right_stick() {
    let mut router = mk_router();
    router.feed(DeviceEvent::MouseMotion { dx: 60.0, dy: 0.0 });
    router.tick();
    let events: Vec<_> = router.drain_events().into_iter().map(|(_, e)| e).collect();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ShimEvent::Axis { axis: 2, value } if *value > 3276))
    );
    router.tick();
    assert!(
        router
            .drain_events()
            .iter()
            .any(|(_, e)| matches!(e, ShimEvent::Axis { axis: 2, value: 0 })),
        "stick returns to rest"
    );
}

#[test]
fn last_device_wins_with_hysteresis_and_neutralises_the_old_device() {
    let mut router = mk_router();
    router.feed(DeviceEvent::PadConnected(0));
    assert_eq!(
        router.source(0),
        Some(InputSource::MouseKeyboard),
        "a pad appearing does not steal the slot"
    );
    key(&mut router, "KeyW", true);
    router.tick();
    router.drain_events();
    router.feed(DeviceEvent::PadAxis {
        pad: 0,
        axis: PadAxis::LeftX,
        value: 1.0,
    });
    for _ in 0..28 {
        router.tick();
        assert_eq!(
            router.source(0),
            Some(InputSource::MouseKeyboard),
            "inside the hysteresis"
        );
    }
    router.drain_events();
    // keep the key down: the pad is the more recent device, the swap releases the virtual pad then follows the pad
    for _ in 0..3 {
        router.tick();
    }
    assert_eq!(router.source(0), Some(InputSource::Pad(0)));
    assert!(!router.mouse_keyboard_active());
    let events: Vec<_> = router.drain_events().into_iter().map(|(_, e)| e).collect();
    assert!(
        events.contains(&ShimEvent::Axis { axis: 1, value: 0 }),
        "virtual forward released: {events:?}"
    );
    assert!(
        events.contains(&ShimEvent::Axis {
            axis: 0,
            value: 32767
        }),
        "pad state taken over"
    );
    // using the mouse now takes it back, but not before the hysteresis again
    router.feed(DeviceEvent::MouseMotion { dx: 50.0, dy: 0.0 });
    router.tick();
    assert_eq!(router.source(0), Some(InputSource::Pad(0)));
    for _ in 0..35 {
        router.feed(DeviceEvent::MouseMotion { dx: 20.0, dy: 0.0 });
        router.tick();
    }
    assert_eq!(router.source(0), Some(InputSource::MouseKeyboard));
}

#[test]
fn small_pad_noise_and_mouse_jitter_do_not_switch() {
    let mut router = mk_router();
    router.feed(DeviceEvent::PadConnected(0));
    router.feed(DeviceEvent::PadAxis {
        pad: 0,
        axis: PadAxis::RightX,
        value: 0.2,
    });
    for _ in 0..100 {
        router.tick();
    }
    assert_eq!(router.source(0), Some(InputSource::MouseKeyboard));
}

#[test]
fn only_one_mouse_owner_and_the_old_owner_falls_back_to_its_pad() {
    let mut router = mk_router();
    router.feed(DeviceEvent::PadConnected(5));
    assert_eq!(router.mouse_owner(), Some(0));
    router.set_mouse_keyboard(Some(1));
    assert_eq!(router.mouse_owner(), Some(1));
    assert_eq!(router.source(0), Some(InputSource::Pad(5)));
    assert_eq!(router.source(1), Some(InputSource::MouseKeyboard));
    key(&mut router, "KeyW", true);
    let inputs = router.tick();
    assert_eq!(inputs[1].unwrap().source, SourceKind::MouseKeyboard);
    assert_eq!(inputs[0].unwrap().source, SourceKind::Pad(5));
    assert!(
        inputs[0].unwrap().move_axis == Vec2::ZERO,
        "the keyboard never reaches a pad slot"
    );
    router.set_mouse_keyboard(None);
    assert_eq!(router.source(1), None);
}

#[test]
fn focus_loss_releases_keys_and_buttons() {
    let mut router = mk_router();
    key(&mut router, "KeyW", true);
    router.feed(DeviceEvent::MouseButton {
        button: MouseButtonId::Left,
        down: true,
    });
    router.tick();
    router.drain_events();
    router.feed(DeviceEvent::FocusLost);
    let input = router.tick()[0].unwrap();
    assert_eq!((input.held, input.move_axis), (0, Vec2::ZERO));
}

#[test]
fn rumble_goes_to_pad_slots_only() {
    let mut router = mk_router();
    router.feed(DeviceEvent::PadConnected(2));
    router.feed(DeviceEvent::PadConnected(4));
    let effect = RumbleEffect::Dual {
        low: 0.5,
        high: 0.1,
        duration_ms: 100,
    };
    let mut got = Vec::new();
    router.route_rumble(0, effect, |pad, e| got.push((pad, e)));
    assert!(got.is_empty(), "slot 0 is on the mouse and keyboard");
    router.route_rumble(1, effect, |pad, e| got.push((pad, e)));
    assert_eq!(got, vec![(4, effect)]);
}

#[test]
fn nan_and_out_of_range_axes_are_safe() {
    let mut router = pad_router();
    router.feed(DeviceEvent::PadConnected(0));
    router.feed(DeviceEvent::PadAxis {
        pad: 0,
        axis: PadAxis::LeftX,
        value: f32::NAN,
    });
    router.feed(DeviceEvent::PadAxis {
        pad: 0,
        axis: PadAxis::LeftY,
        value: f32::INFINITY,
    });
    let input = router.tick()[0].unwrap();
    assert_eq!(input.move_axis.x, 0.0);
    assert!(input.move_axis.y.is_finite() && (0.0..=1.0).contains(&input.move_axis.y));
    let _ = XInputPad::default();
}

#[test]
fn escape_press_sets_the_start_bit_in_the_pad_record_and_backspace_is_b() {
    use crate::shim::{PadRecord, held_bits, xinput_from_player};
    let mut router = mk_router();
    let mut record = PadRecord::new();
    key(&mut router, "Escape", true);
    let input = router.tick()[0].unwrap();
    let pad = xinput_from_player(&input);
    assert_ne!(pad.digital & 0x10, 0, "digital START bit");
    record.update(&pad);
    assert_eq!(
        record.words[PadRecord::HELD] & bits::START,
        bits::START,
        "pad record held word"
    );
    assert_eq!(held_bits(&pad), bits::START);
    key(&mut router, "Escape", false);
    let input = router.tick()[0].unwrap();
    record.update(&xinput_from_player(&input));
    assert_eq!(
        record.words[PadRecord::HELD] & bits::START,
        0,
        "second press toggles through a release edge"
    );
    key(&mut router, "Backspace", true);
    assert_eq!(router.tick()[0].unwrap().held, bits::B);
}

#[test]
fn number_keys_request_a_weapon_slot_for_the_keyboard_owner_only() {
    let mut router = mk_router();
    router.tick();
    assert_eq!(router.take_weapon_requests(), [None; 4]);
    key(&mut router, "Digit3", true);
    let input = router.tick()[0].unwrap();
    assert_eq!(
        input.held & (bits::WHITE | bits::BLACK),
        0,
        "a slot key has no pad bit"
    );
    assert_eq!(router.take_weapon_requests(), [Some(3), None, None, None]);
    assert_eq!(
        router.take_weapon_requests(),
        [None; 4],
        "the request is consumed once"
    );
    router.tick();
    assert_eq!(
        router.take_weapon_requests(),
        [None; 4],
        "holding the key does not repeat it"
    );
    key(&mut router, "Digit3", false);
    key(&mut router, "Digit7", true);
    key(&mut router, "Digit9", true);
    router.tick();
    assert_eq!(
        router.take_weapon_requests()[0],
        Some(9),
        "the last key of a tick wins"
    );
}

#[test]
fn slot_key_routes_on_the_tick_that_keyboard_takes_over_from_pad() {
    let mut router = Router::new(RouterConfig {
        hysteresis_ticks: 0,
        ..RouterConfig::default()
    });
    router.feed(DeviceEvent::PadConnected(7));
    router.set_mouse_keyboard(Some(0));
    assert_eq!(router.source(0), Some(InputSource::Pad(7)));
    router.tick();
    key(&mut router, "Digit4", true);
    router.tick();
    assert_eq!(router.source(0), Some(InputSource::MouseKeyboard));
    assert_eq!(router.take_weapon_requests(), [Some(4), None, None, None]);
}

#[test]
fn pending_slot_request_is_cleared_on_focus_loss_or_owner_change() {
    let mut router = mk_router();
    key(&mut router, "Digit2", true);
    router.tick();
    router.feed(DeviceEvent::FocusLost);
    assert_eq!(router.take_weapon_requests(), [None; 4]);

    key(&mut router, "Digit3", true);
    router.tick();
    router.set_mouse_keyboard(Some(1));
    assert_eq!(router.take_weapon_requests(), [None; 4]);
    key(&mut router, "Digit3", false);
    key(&mut router, "Digit5", true);
    router.tick();
    assert_eq!(router.take_weapon_requests(), [None, Some(5), None, None]);
}

#[test]
fn wheel_down_is_previous_weapon_and_keys_are_decoupled_from_the_pad_bits() {
    let mut router = mk_router();
    router.feed(DeviceEvent::Wheel { notches: -1.0 });
    key(&mut router, "KeyR", true);
    key(&mut router, "KeyG", true);
    let held = router.tick()[0].unwrap().held;
    assert_eq!(held, bits::DPAD_LEFT | bits::X | bits::LT);
}

/// T2200: the mouse look can be switched between the raw mailbox (virtual right stick held at zero) and the interim stick at run time.
#[test]
fn raw_look_switch_moves_the_mouse_look_between_the_mailbox_and_the_virtual_stick() {
    let mut router = Router::new(RouterConfig {
        mouse_keyboard_slot: Some(0),
        raw_look: true,
        ..RouterConfig::default()
    });
    router.drain_events();
    router.feed(DeviceEvent::MouseMotion { dx: 200.0, dy: 0.0 });
    let inputs = router.tick();
    assert!(
        matches!(inputs[0].map(|input| input.look), Some(Look::Delta { yaw_deg, .. }) if yaw_deg > 0.0),
        "degrees reach the mailbox"
    );
    let raw_events = router.drain_events();
    assert!(
        !raw_events
            .iter()
            .any(|(_, event)| matches!(event, ShimEvent::Axis { axis: 2, value } if *value != 0)),
        "no stick in raw mode: {raw_events:?}"
    );
    router.set_raw_look(false);
    assert!(!router.raw_look());
    router.feed(DeviceEvent::MouseMotion { dx: 200.0, dy: 0.0 });
    router.tick();
    let interim = router.drain_events();
    assert!(
        interim
            .iter()
            .any(|(_, event)| matches!(event, ShimEvent::Axis { axis: 2, value } if *value != 0)),
        "interim stick moves: {interim:?}"
    );
}

#[test]
fn native_sensitivity_edit_changes_motion_without_recreating_the_router() {
    let mut router = Router::new(RouterConfig {
        mouse_keyboard_slot: Some(0),
        raw_look: true,
        ..RouterConfig::default()
    });
    router.feed(DeviceEvent::MouseMotion { dx: 10.0, dy: 20.0 });
    router.set_sensitivity(crate::accum::Sensitivity {
        yaw_deg_per_count: 0.1,
        pitch_deg_per_count: 0.2,
        invert_pitch: true,
    });
    let input = router.tick()[0].unwrap();
    assert_eq!(
        input.look,
        Look::Delta {
            yaw_deg: 1.0,
            pitch_deg: 4.0
        }
    );
    assert_eq!(router.mouse_owner(), Some(0));
    assert_eq!(
        router.tick()[0].unwrap().look,
        Look::Delta {
            yaw_deg: 0.0,
            pitch_deg: 0.0
        }
    );
}

/// T3723: raw counts only turn an on-foot player. A mounted slot (turret, vehicle) keeps the virtual right stick, so the
/// mouse aims there too, while an on-foot slot still gets the stick held at rest.
#[test]
fn mounted_slot_keeps_the_mouse_on_the_virtual_stick_in_raw_look_mode() {
    let stick_events = |mounted: u8| {
        let mut router = Router::new(RouterConfig {
            mouse_keyboard_slot: Some(0),
            raw_look: true,
            ..RouterConfig::default()
        });
        router.set_mounted(mounted);
        router.feed(DeviceEvent::MouseMotion { dx: 60.0, dy: 0.0 });
        router.tick();
        router
            .drain_events()
            .into_iter()
            .any(|(_, e)| matches!(e, ShimEvent::Axis { axis: 2, value } if value > 3276))
    };
    assert!(
        !stick_events(0),
        "on foot: raw mailbox owns the look, stick stays at rest"
    );
    assert!(stick_events(1), "mounted: the mouse drives the right stick");
}
