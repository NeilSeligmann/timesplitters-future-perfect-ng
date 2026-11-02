// SPDX-License-Identifier: GPL-3.0-or-later
//! T3742 test-only crate: `tests/lifted_math.rs` runs the tsfp-t2-offline runner (lifted-fidelity
//! control plus port comparison) on corpora made by running the PRIVATE lifted originals of
//! 0x1531D0, 0x1515E0 and 0x5F430 in-process, against their c2r ports. build.rs extracts the lifted
//! bodies when the lift is present (`TSFP_LIFTED_DIR` or `generated/lifted/gen`); without it the
//! test compiles to nothing. Results: docs/t3742-t2-offline.md.
