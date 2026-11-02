// SPDX-License-Identifier: GPL-3.0-or-later
//! T3975 slice 3: the `--route-wait`, `--route-wait-event`, `# wait:` and `--poke-at-poll` grammar in Rust. A
//! faithful port of `src/input/xinput_route_spec.c` (same accept/reject set, same parsed fields, same messages); the
//! structs are `repr(C)` so the C callers keep their layouts. `ffi` exports the C names.
pub mod cond;
pub mod ffi;
pub mod nav_spec;
pub mod number;
pub mod poke;
pub mod wait;

pub use cond::{Cmp, Cond, CondKind};
pub use poke::{PokeEntry, PokeSet, PokeWhere};
pub use wait::{Wait, wait_parse};

pub const DEFAULT_MAX_POLLS: u64 = 36000;
pub const MAX_POLLS_LIMIT: u64 = 10_000_000;
pub const MAX_WAITS: usize = 8;
pub const MAX_MARKS: u64 = 16;
pub const POKE_MAX: usize = 4;
pub const COND_MAX: usize = 6;
pub const COND_TEXT: usize = 48;
pub const DEFAULT_TIMEOUT_MS: u64 = 600_000;
pub const TIMEOUT_MS_LIMIT: u64 = 86_400_000;
