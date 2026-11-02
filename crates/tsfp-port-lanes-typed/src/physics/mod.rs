// SPDX-License-Identifier: GPL-3.0-or-later
//! tsfp-engine-physics register-contract translations (`remainder.rs`, T1922; `followup.rs`, T1934) with their `super::boundary` and `super::arithmetic` siblings. The physics crate's own
//! `arithmetic.rs` takes `sqrt` from std; the no_std twin used by the host archive is
//! tsfp-ports `system_slice` (the same lifted-C arithmetic model, libm `sqrt` / `frexp`).
#[allow(unused)] // shared source: only the helpers the remainder translations call are used
#[path = "../../../tsfp-ports/src/system_slice/mod.rs"]
pub(crate) mod system_slice;
/// `super::arithmetic` of the physics translations: the no_std arithmetic twin plus the
/// `Atan2` trait (tsfp-port-libm), so `followup.rs`'s `y.atan2(x)` resolves without std (T3785).
pub(crate) mod arithmetic {
    #[allow(unused_imports)]
    pub(crate) use super::system_slice::*;
    #[allow(unused_imports)]
    pub(crate) use tsfp_port_libm::Atan2;
}
#[path = "../../../tsfp-engine-physics/src/boundary.rs"]
pub mod boundary;
#[allow(unused, clippy::all)] // machine translation (T1934): dead tails of the lifted C
#[path = "../../../tsfp-engine-physics/src/followup.rs"]
pub mod followup;
#[allow(unused, clippy::all)] // machine translation: self moves and dead tails of the lifted C
#[path = "../../../tsfp-engine-physics/src/remainder.rs"]
pub mod remainder;
