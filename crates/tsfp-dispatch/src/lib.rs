// SPDX-License-Identifier: GPL-3.0-or-later
//! Lane B (T3869): the Rust guest runtime that replaces the C one under every port.
//!
//! Phase 1 slice: the guest VA to lifted-function [`DispatchTable`] (the Rust twin of the
//! generated `recomp_dispatch.c`) and the nested [`guest_call`] trampolines (the Rust twin of
//! `src/host/recomp_guest_call.c`). Both are generic over the function handle and the guest
//! thread, so this crate has no FFI and no `unsafe`; `crates/tsfp-dispatch-capi` binds them to
//! the C host behind the default-off `TSFP_RUST_DISPATCH` switch. Plan and migration order:
//! `docs/t-rust-dispatcher-plan.md`.
#![forbid(unsafe_code)]

pub mod guest_call;
pub mod table;

pub use guest_call::{
    CDECL_HEADROOM_BYTES, CDECL_MAX_ARGS, CdeclOutcome, FpuState, GuestCallError, GuestThread,
    Registers, STDCALL_MAX_ARGS, call_cdecl, call_stdcall,
};
pub use table::{Agreement, DispatchTable, MAX_SPAN, TableError, verify_against};
