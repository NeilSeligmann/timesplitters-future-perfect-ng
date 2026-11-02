//! engine-input: architecture boundary reserved by T1812.
//! T1848 wave 4 adds call-free proven pad leaves in [`leaves`] and wave 4b the call-bearing
//! 002D74B0 in [`calls`]; see docs/t1848-rust-wave4.md.

pub mod calls;
#[path = "../../tsfp-network/src/callee.rs"]
pub mod callee;
#[path = "../../tsfp-engine-memory/src/gprs.rs"]
pub mod gprs;
pub mod leaves;
#[path = "../../tsfp-engine-memory/src/leaf.rs"]
pub mod memory;

pub mod pad_state;
