//! engine-audio: architecture boundary reserved by T1812.
//! T1848 wave 4 adds call-free proven audio leaves in [`leaves`]; see docs/t1848-rust-wave4.md.

#[path = "../../tsfp-engine-memory/src/gprs.rs"]
pub mod gprs;
pub mod leaves;
#[path = "../../tsfp-engine-memory/src/leaf.rs"]
pub mod memory;
