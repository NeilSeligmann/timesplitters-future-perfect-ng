// SPDX-License-Identifier: GPL-3.0-or-later
//! T3820: the router's live-state input decisions, recorded at the intake and replayed verbatim.
//!
//! A replay must not decide these again from replay-time state: the raw-look guard falls back on a stall count that
//! depends on how sim ticks meet guest frames (`docs/t-replay-desync-l3-l4.md`: replays fell back to the virtual stick
//! where the owner kept raw look, and the heading drifted), and the router's slot owners come from devices the router
//! does not see in a replay.

use crate::intake::{decide, recorded_decision, replay_pad_index, replaying};
use std::sync::Mutex;
use tsfp_input::control_lock::ControlLock;
use tsfp_input::player::PlayerInput;
use tsfp_input::router::InputSource;

pub const RAW_LOOK: &str = "raw-look";
pub const SLOT_SOURCES: &str = "slot-sources";
pub const POINTER_GRAB: &str = "pointer-grab";
pub const CONTROL: &str = "control";

fn flag(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

/// Whether mouse degrees go to the raw look mailbox (true) or the interim virtual stick: the recorded guard decision
/// in a replay.
pub fn raw_look(live: bool) -> bool {
    decide(RAW_LOOK, flag(live)) == "1"
}

/// Whether the pointer is captured: recorded, and compared on replay (a difference counts as overridden). The shell
/// does not force it: a replay's router input is its `T` rows, so capture has no input consumer there, and forcing it
/// would lock the pointer of whoever watches a windowed replay (live keys, Ctrl+Alt and F10 included, are dropped).
pub fn pointer_grab(live: bool) {
    decide(POINTER_GRAB, flag(live));
}

/// Whether the owner's player ignored look and move input (`live` is the game's own lock, `None` without a player):
/// recorded when it changes, the recorded value in a replay (whose own lock ends at another tick, so a difference is
/// not an override). `docs/t-replay-desync-l3-l4.md` (Control lock).
pub fn control_locked(live: Option<bool>) -> bool {
    let recorded = crate::intake::recorded_decision(CONTROL);
    recorded.unwrap_or_else(|| decide(CONTROL, flag(live.unwrap_or(false)))) == "1"
}

static CONTROL_GATE: Mutex<Option<ControlLock>> = Mutex::new(None);

/// One router tick of the control lock in a replay: slot 0's look and move rows are dropped while the recorded
/// `control` decision says the owner's player was locked ([`ControlLock`]). The decision comes from the recording, or
/// from its frozen stretches ([`crate::frozen`]). The replay's own lock is not known yet (the mode word is not it,
/// `docs/t-replay-desync-l3-l4.md`), so nothing is held back for it.
pub fn gate_control(mut inputs: [Option<PlayerInput>; 4]) -> [Option<PlayerInput>; 4] {
    let recorded_locked = recorded_decision(CONTROL).is_some_and(|value| value == "1");
    if let (true, Some(input)) = (replaying(), inputs[0]) {
        let mut gate = CONTROL_GATE.lock().unwrap_or_else(|e| e.into_inner());
        let gate = gate.get_or_insert_with(ControlLock::default);
        inputs[0] = Some(gate.apply(input, recorded_locked, false));
    }
    inputs
}

/// The device owning each router slot: the recorded owners in a replay, with recorded pad ids mapped to the replay's
/// gamepad entities.
pub fn slot_sources<const N: usize>(live: [Option<InputSource>; N]) -> [Option<InputSource>; N] {
    let recorded = decide(SLOT_SOURCES, &encode_sources(&live));
    decode_sources(&recorded).map_or(live, |sources| {
        sources.map(|source| match source {
            Some(InputSource::Pad(pad)) => {
                Some(InputSource::Pad(replay_pad_index(pad).unwrap_or(pad)))
            }
            other => other,
        })
    })
}

pub fn encode_sources(sources: &[Option<InputSource>]) -> String {
    sources
        .iter()
        .map(|source| match source {
            None => "-".to_owned(),
            Some(InputSource::MouseKeyboard) => "mk".to_owned(),
            Some(InputSource::Pad(pad)) => format!("p{pad}"),
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub fn decode_sources<const N: usize>(text: &str) -> Option<[Option<InputSource>; N]> {
    let parts: Vec<&str> = text.split(',').collect();
    if parts.len() != N {
        return None;
    }
    let mut out = [None; N];
    for (slot, part) in parts.iter().enumerate() {
        out[slot] = match *part {
            "-" => None,
            "mk" => Some(InputSource::MouseKeyboard),
            pad => Some(InputSource::Pad(pad.strip_prefix('p')?.parse().ok()?)),
        };
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_sources_round_trip_as_decision_values() {
        let sources = [
            Some(InputSource::MouseKeyboard),
            Some(InputSource::Pad(12)),
            None,
            None,
        ];
        let text = encode_sources(&sources);
        assert_eq!(text, "mk,p12,-,-");
        assert!(crate::codec::is_decision_value(&text));
        assert_eq!(decode_sources::<4>(&text), Some(sources));
        assert_eq!(decode_sources::<4>("mk,p12,-"), None);
        assert_eq!(decode_sources::<4>("mk,x,-,-"), None);
    }
}
