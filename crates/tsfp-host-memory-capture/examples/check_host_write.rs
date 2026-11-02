// SPDX-License-Identifier: GPL-3.0-or-later
//! Replays a C host-write capture onto Rust bytes and compares them with the bytes C read back.
//! Host helpers only: recompiled guest stores are NOT witnessed, so this never earns capture credit.
use std::{env, fs};
use tsfp_host_memory_capture::{events, host_write::replay, read};

fn main() {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let data = fs::read(&args[0]).expect("capture file");
    let expected = fs::read(&args[1]).expect("C read-back bytes");
    let decoded = read(data.as_slice(), 65536).expect("sealed stream");
    let ops = [(1, 9, 1), (2, 1, 0), (30, 2, 0)];
    events::validate(&decoded, b"synthetic-host-write", &ops).expect("valid envelope");
    let writes = decoded
        .iter()
        .filter(|e| e.0 == events::MEMORY_WRITE)
        .count();
    assert_eq!(
        writes, 6,
        "u32/u8/bytes/edge/freed-region/overwrite writes; refused writes leave no event"
    );
    let regions = replay(&decoded, 30).expect("ordered replay");
    assert_eq!(regions.len(), 1, "freed region unplaced, kept region live");
    assert_eq!(
        regions.values().next().unwrap(),
        &expected,
        "replayed bytes equal C read-back"
    );
    println!(
        "T3906: synthetic host-helper writes replayed in order and matched C bytes; lifted stores uncovered, no capture credit"
    );
}
