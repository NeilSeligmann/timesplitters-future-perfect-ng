// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared runtime setup with the same argument parsing as production.
use tsfp_rt_host::{Runtime, parse_args};

/// Start a runtime with explicit test options and require an adapter.
pub fn start(args: &str) -> Runtime {
    let words: Vec<String> = args.split_whitespace().map(str::to_string).collect();
    Runtime::start(parse_args(&words).expect("args")).expect("adapter")
}
