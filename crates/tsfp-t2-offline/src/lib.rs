// SPDX-License-Identifier: GPL-3.0-or-later
//! T3742: capture-based Tier 2 offline differential runner (AGENTS.md rule 43,
//! docs/t3738-capture-design.md section 3, step 2a; results in docs/t3742-t2-offline.md).
//!
//! For every captured call of a function (a T3741 spool record) the runner
//! 1. prepares a fault-tracked guest arena holding exactly the captured read-set (`arena`);
//! 2. runs the lifted original against it (the capture-fidelity control: it must reproduce the
//!    captured exit registers and write-set, otherwise the entry is `capture_unfaithful`);
//! 3. runs the Rust port through its dispatch entry with every nested call replaced by the
//!    captured callee outcome (`stub`);
//! 4. judges both runs with the EQUAL rule (`compare`) and writes one JSON line (`runner`).
//!
//! `host` is the `tsfp-bevy --t2-offline` mode; tests drive the same runner with in-process
//! executors. Linux x86_64 only (page-fault error code from the signal context).
pub mod arena;
pub mod clock;
pub mod compare;
pub mod digest;
pub mod frames;
pub mod host;
#[cfg(any(test, feature = "test-link-stubs"))]
mod link_stubs;
pub mod runner;
pub mod spool;
pub mod stub;

pub use runner::{
    EntryResult, Executor, Options, Summary, Verdict, run_entry, run_entry_in, run_spool,
};
