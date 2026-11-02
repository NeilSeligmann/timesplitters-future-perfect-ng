//! T1845: every distinct non-null hook of the level table has a port entry.
use std::collections::BTreeSet;
use tsfp_gameplay::story_events::levels::{HOOK_PORTS, LEVEL_HOOKS};

#[test]
fn every_distinct_hook_has_a_port_entry() {
    let hooks: BTreeSet<u32> = LEVEL_HOOKS
        .iter()
        .flat_map(|r| [r.1, r.2])
        .filter(|&a| a != 0)
        .collect();
    let ported: BTreeSet<u32> = HOOK_PORTS.iter().map(|p| p.0).collect();
    assert_eq!(hooks.len(), 25);
    assert_eq!(hooks, ported);
}
