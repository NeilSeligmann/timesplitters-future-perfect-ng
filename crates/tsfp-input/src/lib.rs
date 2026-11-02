// SPDX-License-Identifier: GPL-3.0-or-later
//! T2160, stage 1 of the input rework (T2166, contract: docs/t-contract-input.md, design: docs/t-input-rework-design.md).
//!
//! Host side input stack, no unsafe code, no Bevy or SDL dependency (the shell translates its events into
//! [`router::DeviceEvent`]). Layers:
//!
//! * [`router`]: device layer (gamepad f32 analog with hotplug, raw relative mouse, keyboard, wheel), the per slot source
//!   assignment (`Pad(id)` | `MouseKeyboard`) with last-device-wins hysteresis and one mouse owner, and the
//!   [`player::PlayerInput`] built per slot per tick.
//! * [`accum`]: f64 sub-pixel mouse accumulator, wheel accumulator, sensitivity.
//! * [`bindings`]: keyboard and mouse button to virtual pad actions, remappable through a small TOML subset file.
//! * [`capture`]: pure state of the mouse capture and its release chord (the shell applies it to the window).
//! * [`game_side`]: typed frame/XPP/pad-record and rumble boundary implemented by the native
//!   router.
//! * [`actions`], [`shaping`], [`presets`] (T2167, stage 2): intents with chords and toggles, the `original`/`modern`/`mouse-raw` shaping presets, per player settings, the extended TOML (docs/t-input-presets.md).
//! * [`shim`]: the compatibility shim, `PlayerInput` back to the old pad world (SDL-queue events as the Bevy shell feeds
//!   them today, XInput state, the 0x3C pad record words), so a pad user stays trace equal with the `original` preset.
pub mod accum;
pub mod actions;
pub mod bindings;
pub mod capture;
pub mod control_lock;
pub mod controller;
pub mod device_hle;
pub mod dispatch;
pub mod dispatch_capture;
pub mod extras;
pub mod feedback;
pub mod game_side;
pub mod guard;
pub mod look;
pub mod look_gate;
pub mod pad_report;
pub mod player;
pub mod presets;
pub mod recording;
pub mod router;
pub mod settings;
pub mod shaping;
pub mod shim;
pub mod state_digest;

pub mod sampling;
