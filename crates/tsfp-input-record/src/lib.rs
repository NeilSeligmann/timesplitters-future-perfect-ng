// SPDX-License-Identifier: GPL-3.0-or-later
//! T3926: the host-side input readers as Rust. `script` is the `--pad-script` text grammar
//! (`xinput_script_parse`), `flags` the record identity canonicaliser, `record` the versioned
//! record file loader (`xinput_replay_load`). Messages match the C reader byte for byte so the
//! C reader stays the baseline until an offline differential over recorded inputs is EQUAL.

pub mod flags;
pub mod record;
pub mod script;

pub use record::{Budgets, Replay, load_replay};
pub use script::{PadState, Script, parse_script};
