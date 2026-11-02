//! Original-verified integer physics leaf; see docs/t1835-rust-wave2.md. T1836 wave 8 adds the
//! scalar-SSE `fp_leaves`, see docs/t1836-rust-wave8.md.
extern crate alloc;

pub mod fp_leaves;
pub mod fp_scalar;
pub mod fp_scalar_b;
pub mod fp_scalar_d;
pub mod fp_stack;
pub mod fp_vector;
pub mod fp_wave10;
pub mod fp_x87;
#[path = "../../tsfp-engine-memory/src/gprs.rs"]
pub mod gprs;
pub mod leaf;
#[path = "../../tsfp-engine-memory/src/leaf.rs"]
pub mod memory;
#[path = "../../tsfp-engine-memory/src/sse.rs"]
pub mod sse;
pub mod synth_leaves;

/// T1914 opt-in typed collision table boundary (Tier 0).
pub mod tables;

mod arithmetic;
/// T1922 opt-in physics remainder and explicit cross-system register/call boundary.
pub mod boundary;
/// T2394 typed contact lease and deferred owner-event service.
pub mod rea_events;
pub mod remainder;

/// T1934 opt-in audited physics follow-up (Tier 0).
pub mod followup;
