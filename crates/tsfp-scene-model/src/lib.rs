// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared native scene contract (T2238, docs/t-native-renderer-plan.md).
//!
//! This dependency-free crate owns data and admission, not simulation, culling,
//! interpolation algorithms, disc loading, shader translation or GPU objects.
//! Producers publish immutable fixed-tick endpoints. Render prep samples once
//! into an ordered DrawList; backends consume it without advancing simulation.
//! Asset references identify immutable content, never guest pointers. The asset
//! owner retains referenced resources until GPU completion. Missing resources
//! and unsupported material recipes must be errors, never silent skipped draws.
//!
//! Matrices are column-major, multiply column vectors, and retain game-world
//! units. Camera projection maps to wgpu clip depth [0, 1]; the oracle adapter
//! explicitly converts legacy conventions. Colours/parameters retain the
//! original recipe's numeric space; no implicit sRGB conversion is permitted.

mod frame;
mod model;
mod state;
mod validate;

pub use frame::*;
pub use model::*;
pub use state::*;
pub use validate::{ContractError, ValidatedDrawList};

/// Pin in oracle manifests. Rust memory layout is not a trace/wire encoding.
pub const CONTRACT_VERSION: u32 = 1;

/// A backend sees only structurally admitted, already sampled draws. Its own
/// admission also checks resource availability, mesh ranges, formats and recipe
/// support before submitting any pass. Queue owners retain the complete frame.
pub trait SceneRenderer {
    type Error;
    fn submit(&mut self, frame: ValidatedDrawList<'_>) -> Result<(), Self::Error>;
}
