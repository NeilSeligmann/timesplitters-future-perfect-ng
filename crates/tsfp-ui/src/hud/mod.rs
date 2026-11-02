// SPDX-License-Identifier: GPL-3.0-or-later
//! The in-game HUD (`ui/hud`) as a system slice (T1891).
//!
//! The HUD touches the rest of the game through exactly three attachment points, each an explicit
//! type here, so the slice can be developed, tested and replaced without the other systems:
//!
//! 1. **State**: [`state::PlayerHudState`], the values the HUD reads from the player, character,
//!    weapon and inventory records, each field annotated with its guest source offset.
//! 2. **Rendering**: [`draw::Draw`], the calls the HUD makes into the renderer (rectangles,
//!    textured rectangles, text). [`draw::RecordingDraw`] records them for tests and pictures.
//! 3. **Text**: [`text::TextProvider`], label id to string, backed by a `tsfp-format-bs` bank.
//!
//! Ported functions keep the original guest address in their doc comment and the verification tier
//! reached (docs/t-hud-slice.md). The ported bodies read and write the guest records through
//! [`crate::memory::GuestMemory`], so they run both on a snapshot and on live guest memory.

pub mod addr;
pub mod bulk;
pub mod draw;
pub mod frame;
pub mod glyph;
pub mod health;
pub mod letterbox;
pub mod messages;
pub mod primitives;
pub mod prompt;
pub mod render_draw;
pub mod snapshot;
pub mod state;
pub mod text;
pub mod textpath;
pub mod widgets;
pub mod x86;

#[cfg(test)]
mod tests;
