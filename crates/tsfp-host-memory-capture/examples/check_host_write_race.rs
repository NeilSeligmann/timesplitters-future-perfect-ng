// SPDX-License-Identifier: GPL-3.0-or-later
//! Replays the schedule-controlled race capture; exits 0 only if the replay's verdict matches `agree|differ`.
use std::{env, fs};
use tsfp_host_memory_capture::{host_write::replay, read};

fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    let data = fs::read(&args[0]).expect("capture file");
    let memory = fs::read(&args[1]).expect("C read-back bytes");
    let decoded = read(data.as_slice(), 65536).expect("sealed stream");
    let regions = replay(&decoded, 30).expect("replay");
    let agree = regions.values().next().unwrap() == &memory;
    assert_eq!(agree, args[2] == "agree", "replay vs memory verdict");
}
