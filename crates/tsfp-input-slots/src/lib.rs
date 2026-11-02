// SPDX-License-Identifier: GPL-3.0-or-later
//! Per local slot mailboxes between the Bevy input router and the guest side (T3771, docs/t3771-two-player-pads.md).
//!
//! * [`ports`]: which guest controller ports 1 to 3 hold a pad and the XInput state each reports. Port n follows router
//!   slot n, so a second pad is a second guest controller and the Players page enables 2 players (owner playtest
//!   2026-10-26). Port 0 stays the queue pad of `--pad-source gamepad`.
//! * [`hud`]: held buttons per player for the native HUD message bridge (moved out of `tsfp-bevy`, rule 37).
//! * [`weapons`]: direct weapon slot requests per slot (moved likewise).
//! * [`sources`]: the device owning each slot, for native pages that answer each player on its own port (T3772).
pub mod hud;
pub mod ports;
pub mod sources;
pub mod weapons;

/// Local player slots (and guest controller ports).
pub const SLOTS: usize = 4;
