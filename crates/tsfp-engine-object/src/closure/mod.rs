// SPDX-License-Identifier: GPL-3.0-or-later
//! T1815 closure-model ports. Self-contained under `super`/`self` paths because this crate's
//! lib.rs is also path-included into tsfp-ports (docs/t1815-closure-model.md).
pub mod callee;
pub mod calls;
pub mod calls_w8;
#[path = "../../../tsfp-engine-memory/src/leaf.rs"]
pub mod memory;
