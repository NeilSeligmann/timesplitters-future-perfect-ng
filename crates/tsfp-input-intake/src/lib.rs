// SPDX-License-Identifier: GPL-3.0-or-later
//! T3820: the Bevy shell's single host-device intake (input.rec format v6 `D` rows).
//!
//! Every device event any consumer reads passes [`intake::intake`] first: recorded with its simulation tick and
//! host poll while recording, replaced by the recorded events while replaying. A new native page that reads Bevy
//! input is therefore recorded and replayed without a hook of its own. Format: `docs/t2348-input-recording.md`.

pub mod codec;
pub mod controller_route;
pub mod controller_storage;
pub mod decisions;
pub mod frozen;
pub mod intake;
pub mod route_boundary;
pub mod schedule;
pub mod sdl;

pub use codec::{DeviceEvent, RouteKind};
pub use intake::{
    Host, IntakeSet, MARK_HOTKEY, RECORDED_EVENTS, VirtualWindow, cursor_position, decide, focused,
    install, ready_poll, recorded_decision, recording, replaying, report, route, set_host,
    start_recording, start_replay, stop,
};
