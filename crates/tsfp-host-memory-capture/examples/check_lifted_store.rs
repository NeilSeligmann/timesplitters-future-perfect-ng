// SPDX-License-Identifier: GPL-3.0-or-later
//! Replays a lifted-store capture (t2cap_st* hooks, copy/fill/atomic + host helpers) and reports whether it matches C memory and
//! whether the writer-class admission gate would call the stream complete (it must for the agreeing synthetic stream).
//! Usage: check_lifted_store <capture> <c-bytes> agree|differ
use std::{env, fs};
use tsfp_host_memory_capture::{
    host_write::replay_ops,
    read,
    writer_coverage::{ClassOps, HOOKED, admit_complete},
};

fn main() {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let data = fs::read(&args[0]).expect("capture file");
    let expected = fs::read(&args[1]).expect("C bytes");
    let want_agree = args[2] == "agree";
    let decoded = read(data.as_slice(), 65536).expect("sealed stream");
    let regions = replay_ops(&decoded, &[30, 31, 32, 33, 34, 35]).expect("replay");
    let same = regions.values().next().unwrap() == &expected;
    assert_eq!(same, want_agree, "replay vs C memory");
    let ops = ClassOps {
        host_write: 30,
        lifted_store: 31,
        guest_copy: 32,
        guest_fill: 33,
        guest_atomic: 34,
        rust_mapped: 35,
    };
    let verdict = admit_complete(&decoded, ops, &HOOKED);
    // A fully witnessed agreeing stream is admitted; a stream missing a class witness is refused (NoWitness).
    if want_agree {
        assert_eq!(
            verdict,
            Ok(()),
            "complete synthetic stream must be admitted"
        );
    }
    println!("T3906: lifted+host stores replay agree={same}; admission {verdict:?}");
}
