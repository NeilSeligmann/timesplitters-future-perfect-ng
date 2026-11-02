// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline oracle admission. Transport completeness is not semantic DrawList equality.
//!
//! Reads the original binary device stream one bounded record at a time. Reuses
//! the producer's wire schema without linking its game/renderer dependency graph,
//! as the existing no_std trace producer does. No GPU or live routing changes.
extern crate alloc;

#[rustfmt::skip]
#[path = "../../tsfp-engine-render/src/trace_wire.rs"]
pub mod wire;

mod reader;
pub use reader::{Error, Limits, LocatedRecord, Reader, Window};

mod drawlist;
pub use drawlist::{StreamDrawListSource, resolve_materials};
