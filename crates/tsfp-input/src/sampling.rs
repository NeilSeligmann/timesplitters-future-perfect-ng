// SPDX-License-Identifier: GPL-3.0-or-later
//! Opt-in, event-rate sampling with a read-only render look query (T2239).
//! No device polling clock is imposed: callers forward each device's events as they arrive.
use crate::accum::LookAccumulator;
use crate::player::PlayerInput;
use crate::router::{DeviceEvent, InputSource, Router, RouterConfig, SLOTS};
use crate::shim::ShimEvent;

/// Mouse turn since the last simulation tick, before camera clamps or shaping.
/// A render consumer adds this to its tick pose, never to its previous render pose.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RenderLook {
    pub yaw_deg: f64,
    pub pitch_deg: f64,
}

/// An additive boundary around the existing router. Existing callers remain unchanged.
/// Sampling and tick consumption require exclusive access; render queries borrow immutably.
pub struct SampledInput {
    router: Router,
    pending: LookAccumulator,
    sensitivity: crate::accum::Sensitivity,
}

impl SampledInput {
    pub fn new(config: RouterConfig) -> Self {
        Self {
            sensitivity: config.bindings.sensitivity,
            router: Router::new(config),
            pending: LookAccumulator::default(),
        }
    }

    /// Feed immediately at the device's rate, without waiting for a render frame or tick.
    /// Preserve event order, including focus loss and pad transitions.
    pub fn sample(&mut self, event: DeviceEvent) {
        match &event {
            DeviceEvent::MouseMotion { dx, dy } => self.pending.add(*dx, *dy),
            DeviceEvent::FocusLost => {
                self.pending.take();
            }
            _ => {}
        }
        self.router.feed(event);
    }

    /// The only simulation consumption boundary. The returned values are the router's
    /// unchanged output; even a tick with no active player consumes pending counts.
    pub fn tick(&mut self) -> [Option<PlayerInput>; SLOTS] {
        let input = self.router.tick();
        self.pending.take();
        input
    }

    /// Raw f64 counts since the last tick (or focus loss), for diagnostics/other shaping.
    /// No rounding, draining, or mutation occurs when a render frame queries this.
    pub fn pending_counts(&self) -> (f64, f64) {
        self.pending.peek()
    }

    /// Only the currently active mouse owner receives a mouse preview. Pads return None:
    /// a stick is a rate, and previewing it requires the future typed camera/shaper's time
    /// integration, not treating it as a mouse delta. Invalid/inactive slots return None.
    /// Last-device-wins decisions still happen exclusively at the simulation tick.
    pub fn look_since_tick(&self, slot: u8) -> Option<RenderLook> {
        if self.router.source(slot) != Some(InputSource::MouseKeyboard) {
            return None;
        }
        let (x, y) = self.pending.peek();
        let pitch = -y * self.sensitivity.pitch_deg_per_count;
        Some(RenderLook {
            yaw_deg: x * self.sensitivity.yaw_deg_per_count,
            pitch_deg: if self.sensitivity.invert_pitch {
                -pitch
            } else {
                pitch
            },
        })
    }

    /// Inspect latest device/source state without bypassing the sampling boundary.
    pub fn router(&self) -> &Router {
        &self.router
    }

    pub fn set_mouse_keyboard(&mut self, slot: Option<u8>) {
        self.router.set_mouse_keyboard(slot);
    }
    pub fn set_raw_look(&mut self, enabled: bool) {
        self.router.set_raw_look(enabled);
    }
    pub fn drain_events(&mut self) -> Vec<(u8, ShimEvent)> {
        self.router.drain_events()
    }
    pub fn take_weapon_requests(&mut self) -> [Option<u8>; SLOTS] {
        self.router.take_weapon_requests()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::{MouseButtonId, PadAxis, PadButton};

    fn config() -> RouterConfig {
        RouterConfig {
            mouse_keyboard_slot: Some(0),
            hysteresis_ticks: 2,
            ..RouterConfig::default()
        }
    }

    #[test]
    fn render_reads_do_not_consume_and_tick_resets() {
        let mut input = SampledInput::new(config());
        for _ in 0..1000 {
            input.sample(DeviceEvent::MouseMotion {
                dx: 0.125,
                dy: -0.25,
            });
        }
        assert_eq!(input.pending_counts(), (125.0, -250.0));
        let expected = Some(RenderLook {
            yaw_deg: 125.0 * 0.022,
            pitch_deg: 250.0 * 0.022,
        });
        for _ in 0..100 {
            assert_eq!(input.look_since_tick(0), expected);
        }
        assert_eq!(input.look_since_tick(1), None);
        assert_eq!(input.look_since_tick(255), None);
        input.tick();
        assert_eq!(input.look_since_tick(0), Some(RenderLook::default()));
        input.sample(DeviceEvent::MouseMotion { dx: 0.5, dy: 1.0 });
        input.sample(DeviceEvent::FocusLost);
        assert_eq!(input.pending_counts(), (0.0, 0.0));
    }

    #[test]
    fn ownership_and_inversion() {
        let mut cfg = config();
        cfg.bindings.sensitivity.invert_pitch = true;
        let mut input = SampledInput::new(cfg);
        input.sample(DeviceEvent::MouseMotion { dx: 1.0, dy: 2.0 });
        input.set_mouse_keyboard(Some(3));
        assert_eq!(input.look_since_tick(0), None);
        assert_eq!(input.look_since_tick(3).unwrap().pitch_deg, 2.0 * 0.022);
        input.set_mouse_keyboard(None);
        input.tick();
        input.set_mouse_keyboard(Some(0));
        assert_eq!(input.pending_counts(), (0.0, 0.0));
        let mut pad = SampledInput::new(RouterConfig::default());
        pad.sample(DeviceEvent::PadConnected(7));
        pad.sample(DeviceEvent::PadAxis {
            pad: 7,
            axis: PadAxis::RightX,
            value: 0.75,
        });
        assert_eq!(pad.router().source(0), Some(InputSource::Pad(7)));
        assert_eq!(pad.look_since_tick(0), None);
    }

    /// Deterministic property sweep: arbitrary event/tick/render interleavings, both
    /// shim modes, focus loss, hotplug and last-device switches against the old path.
    #[test]
    fn generated_streams_preserve_tick_outputs_and_counts() {
        for seed in 1..=128u64 {
            let mut cfg = config();
            cfg.raw_look = seed % 2 == 0;
            let mut reference = Router::new(cfg.clone());
            let mut sampled = SampledInput::new(cfg);
            let mut rng = seed;
            let mut sum = (0.0, 0.0);
            for _ in 0..512 {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                let event = match (rng >> 32) % 11 {
                    0 => DeviceEvent::FocusLost,
                    1 => DeviceEvent::PadConnected(7),
                    2 => DeviceEvent::PadDisconnected(7),
                    3 => DeviceEvent::PadAxis {
                        pad: 7,
                        axis: PadAxis::RightX,
                        value: 0.75,
                    },
                    4 => DeviceEvent::PadButton {
                        pad: 7,
                        button: PadButton::South,
                        value: (rng % 2) as f32,
                    },
                    5 => DeviceEvent::Key {
                        name: "KeyW".into(),
                        down: rng % 2 == 0,
                    },
                    6 => DeviceEvent::Wheel { notches: 0.25 },
                    7 => DeviceEvent::MouseButton {
                        button: MouseButtonId::Left,
                        down: rng % 2 == 0,
                    },
                    8 => DeviceEvent::Key {
                        name: "Digit1".into(),
                        down: rng % 2 == 0,
                    },
                    _ => DeviceEvent::MouseMotion {
                        dx: ((rng >> 16) % 257) as f64 * 0.125 - 16.0,
                        dy: -0.25,
                    },
                };
                match &event {
                    DeviceEvent::FocusLost => sum = (0.0, 0.0),
                    DeviceEvent::MouseMotion { dx, dy } => {
                        sum.0 += dx;
                        sum.1 += dy;
                    }
                    _ => {}
                }
                reference.feed(event.clone());
                sampled.sample(event);
                assert_eq!(sampled.pending_counts(), sum);
                for slot in 0..SLOTS as u8 {
                    let _ = sampled.look_since_tick(slot);
                }
                assert_eq!(sampled.pending_counts(), sum);
                if rng % 7 == 0 {
                    assert_eq!(sampled.tick(), reference.tick());
                    sum = (0.0, 0.0);
                    assert_eq!(sampled.pending_counts(), sum);
                    assert_eq!(
                        sampled.take_weapon_requests(),
                        reference.take_weapon_requests()
                    );
                }
                assert_eq!(sampled.drain_events(), reference.drain_events());
            }
            assert_eq!(sampled.tick(), reference.tick());
        }
    }
}
