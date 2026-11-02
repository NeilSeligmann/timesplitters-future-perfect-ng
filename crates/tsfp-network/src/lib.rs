//! network: architecture boundary reserved by T1812.
//! T1848 wave 4 adds call-free proven network leaves in [`leaves`], wave 4b the call-bearing
//! and stdcall ports in [`calls`] and [`fesl`]; see docs/t1848-rust-wave4.md.

pub mod calls;
pub mod callee;
pub mod fesl;
#[path = "../../tsfp-engine-memory/src/gprs.rs"]
pub mod gprs;
pub mod leaves;
#[path = "../../tsfp-engine-memory/src/leaf.rs"]
pub mod memory;
pub mod synth_leaves;
pub mod w4n_syslink;

pub mod syslink_transport;
pub mod w4n_syslink_llm;
pub mod syslink_bulk;

pub mod syslink_codec;
pub mod syslink_reliable;
pub mod syslink_session;
