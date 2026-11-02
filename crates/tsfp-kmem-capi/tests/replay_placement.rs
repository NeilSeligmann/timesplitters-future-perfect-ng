// SPDX-License-Identifier: GPL-3.0-or-later
//! Replay mode is checked in a fresh process so no test mutates the shared environment.
#[path = "support/witness_stubs.rs"]
mod witness_stubs;
use std::process::Command;

use tsfp_engine_memory::guest_regions::Placement;
use tsfp_kmem_capi::placement::{HINT_BASE, MmapPlacement};

#[test]
fn replay_mapping_matches_c_cursor() {
    const CHILD: &str = "TSFP_KMEM_REPLAY_PLACEMENT_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "replay_mapping_matches_c_cursor", "--nocapture"])
            .env(CHILD, "1")
            .env("TSFP_GUEST_PLACE", "det")
            .output()
            .expect("run isolated replay placement fixture");
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let base = HINT_BASE as u32;
    let mut first = MmapPlacement::new();
    let a = first.map(0x11000, None).expect("first replay mapping");
    assert_eq!(a, base);
    first.unmap(a, 0x11000);
    drop(first);

    // Freeing a map and recreating the backend must not rewind the process cursor.
    let mut next = MmapPlacement::new();
    let b = next.map(0x1000, None).expect("second replay mapping");
    assert_eq!(b, base + 0x20000);
    next.unmap(b, 0x1000);

    // Fixed mappings do not advance the cursor; a collision skips exactly 64 KB.
    let reserved = next
        .map(0x10000, Some(base + 0x30000))
        .expect("reserve collision");
    let c = next.map(0x21000, None).expect("skip collision");
    assert_eq!(c, base + 0x40000);
    assert_eq!(
        next.map(0x1000, Some(c)),
        None,
        "fixed map never replaces live memory"
    );
    let d = next
        .map(0x1000, None)
        .expect("after rounded three-unit span");
    assert_eq!(d, base + 0x70000);

    next.unmap(reserved, 0x10000);
    next.unmap(c, 0x21000);
    next.unmap(d, 0x1000);
    assert_eq!(next.mapped_bytes, 0);
}
