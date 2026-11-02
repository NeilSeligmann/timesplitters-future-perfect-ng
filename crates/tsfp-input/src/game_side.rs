// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed game-side boundary for the native input router.
//!
//! `PlayerInput` is the owned frame contract. `XppPadState` is its XInput-shaped
//! projection for consumers that still read the XPP state; it does not include
//! an XPP handle, packet counter, capabilities, or device implementation.
//! `PadRecord` is a separate compatibility projection for title-side record
//! consumers. Rumble travels back through the router's current pad assignment.
use crate::player::{PlayerInput, RumbleEffect};
use crate::router::{PadId, Router};
use crate::shim::{PadRecord, XInputPad, xinput_from_player};

/// XInput data exposed to the XPP-facing compatibility boundary.
pub type XppPadState = XInputPad;

/// One immutable simulation-step input plus its compatibility projections.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameInputFrame {
    pub player: PlayerInput,
    pub xpp: XppPadState,
}

impl GameInputFrame {
    pub fn from_player(player: PlayerInput) -> Self {
        Self {
            xpp: xinput_from_player(&player),
            player,
        }
    }

    /// Update the compatibility record's input fields while preserving guest
    /// owned rumble and consumed words, matching `PadRecord::update` semantics.
    pub fn update_pad_record(&self, record: &mut PadRecord) {
        record.update(&self.xpp);
    }
}

/// The game-side input and feedback edge implemented by the native router.
///
/// Hotkey policy runs at the host event boundary. Events that policy forwards
/// remain ordinary live input here; recording suppression does not rewrite the
/// game's held/edge state. A slot receives rumble only while its active source
/// is a connected pad.
pub trait GameInputBoundary {
    /// The most recent frame published by `Router::tick`, or `None` for an
    /// inactive slot.
    fn game_input(&self, slot: u8) -> Option<GameInputFrame>;

    /// Route one game request through the slot's current source assignment.
    fn route_game_rumble(
        &self,
        slot: u8,
        effect: RumbleEffect,
        sink: &mut dyn FnMut(PadId, RumbleEffect),
    );
}

impl GameInputBoundary for Router {
    fn game_input(&self, slot: u8) -> Option<GameInputFrame> {
        self.last_input(slot).map(GameInputFrame::from_player)
    }

    fn route_game_rumble(
        &self,
        slot: u8,
        effect: RumbleEffect,
        sink: &mut dyn FnMut(PadId, RumbleEffect),
    ) {
        self.route_rumble(slot, effect, sink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{SourceKind, bits};
    use crate::router::{DeviceEvent, PadButton, RouterConfig};

    fn pad_router() -> Router {
        let mut router = Router::new(RouterConfig::default());
        router.feed(DeviceEvent::PadConnected(3));
        router
    }

    #[test]
    fn game_input_xpp_and_pad_record_share_the_owned_frame() {
        let mut router = pad_router();
        router.feed(DeviceEvent::PadButton {
            pad: 3,
            button: PadButton::South,
            value: 1.0,
        });
        router.tick();
        let frame = router.game_input(0).expect("active pad frame");
        assert_eq!(frame.player.source, SourceKind::Pad(3));
        assert_eq!(frame.player.held, bits::A);
        assert_eq!(frame.xpp.analog[0], 255);

        let mut record = PadRecord::new();
        record.words[13] = 127; // +0x34 rumble belongs to the guest.
        record.words[14] = bits::A; // +0x38 consumed mask belongs to the guest.
        frame.update_pad_record(&mut record);
        assert_eq!(record.words[PadRecord::HELD], bits::A);
        assert_eq!(record.words[PadRecord::PRESSED], bits::A);
        assert_eq!(record.words[PadRecord::RELEASED], 0);
        assert_eq!((record.words[13], record.words[14]), (127, bits::A));

        router.feed(DeviceEvent::PadButton {
            pad: 3,
            button: PadButton::South,
            value: 0.0,
        });
        router.tick();
        let frame = router.game_input(0).expect("active pad frame");
        frame.update_pad_record(&mut record);
        assert_eq!(frame.xpp.analog[0], 0);
        assert_eq!(record.words[PadRecord::HELD], 0);
        assert_eq!(record.words[PadRecord::PRESSED], 0);
        assert_eq!(record.words[PadRecord::RELEASED], bits::A);
    }

    #[test]
    fn forwarded_hotkey_lead_remains_live_game_input() {
        // T1632's lead key is forwarded to live input even when the host later
        // removes that interval from a recording. Router state is not rewritten.
        let mut router = pad_router();
        router.feed(DeviceEvent::PadButton {
            pad: 3,
            button: PadButton::Select,
            value: 1.0,
        });
        router.tick();
        let frame = router.game_input(0).expect("active pad frame");
        assert_eq!(frame.player.held, bits::BACK);
        assert_eq!(frame.xpp.digital & 0x20, 0x20);
    }

    #[test]
    fn game_rumble_routes_to_current_pad_and_keeps_motor_contract() {
        let mut router = pad_router();
        router.tick();
        let effect = RumbleEffect::from_original_level(127, 20);
        assert_eq!(
            effect,
            RumbleEffect::Dual {
                low: 65023.0 / 65535.0,
                high: 65023.0 / 65535.0,
                duration_ms: 20,
            }
        );
        let mut routed = Vec::new();
        router.route_game_rumble(0, effect, &mut |pad, effect| routed.push((pad, effect)));
        assert_eq!(routed, vec![(3, effect)]);

        let split_motors = RumbleEffect::Dual {
            low: 0.25,
            high: 0.75,
            duration_ms: 35,
        };
        router.route_game_rumble(0, split_motors, &mut |pad, effect| {
            assert_eq!(pad, 3);
            assert_eq!(effect, split_motors);
        });
        assert_eq!(RumbleEffect::from_original_level(0, 20), RumbleEffect::Off);
    }
}
