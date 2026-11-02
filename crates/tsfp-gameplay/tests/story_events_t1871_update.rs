//! T1871: the effect pool update callbacks against emulated-original golden vectors (MEASURED). The original ran the update AND its helper family
//! for real under the Unicorn oracle (only engine leaves scripted); the port runs the helper family through `eff_dispatch::native`.
//! Goldens `tests/data/t1871/eff_f*.golden` (`tools/t1846_gold/eff_update.py`).
mod common;
use common::guest::GoldenGuest;
use common::parse;
use tsfp_gameplay::story_events::eff_dispatch::NATIVE;
use tsfp_gameplay::story_events::eff_update::*;

const ANCHOR: u32 = 0xF000_0000;

fn run_whole(text: &str, root: impl Fn(&mut GoldenGuest, u32)) -> Vec<Vec<u32>> {
    let cases = parse(text);
    assert!(!cases.is_empty());
    let mut sequences = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index).with_native(&NATIVE);
        let entry = case.input(ANCHOR);
        root(&mut guest, entry);
        guest.finish();
        sequences.push(case.calls.iter().map(|c| c.callee).collect());
    }
    sequences
}

#[test]
fn projectile_update_matches_original() {
    let sequences = run_whole(
        include_str!("data/t1871/eff_f4a50.golden"),
        |guest, entry| projectile_update(guest, entry),
    );
    let distinct: std::collections::BTreeSet<_> = sequences.iter().collect();
    assert!(
        distinct.len() > 60,
        "distinct engine call sequences: {}",
        distinct.len()
    );
}

#[test]
fn particle_update_matches_original() {
    let sequences = run_whole(
        include_str!("data/t1871/eff_f08d0.golden"),
        |guest, entry| particle_update(guest, entry),
    );
    let distinct: std::collections::BTreeSet<_> = sequences.iter().collect();
    assert!(
        distinct.len() > 40,
        "distinct engine call sequences: {}",
        distinct.len()
    );
}

#[test]
fn debris_update_matches_original() {
    let sequences = run_whole(
        include_str!("data/t1871/eff_f2b80.golden"),
        |guest, entry| debris_update(guest, entry),
    );
    let distinct: std::collections::BTreeSet<_> = sequences.iter().collect();
    assert!(
        distinct.len() > 40,
        "distinct engine call sequences: {}",
        distinct.len()
    );
}

#[test]
fn stuck_update_matches_original() {
    let sequences = run_whole(
        include_str!("data/t1871/eff_f39a0.golden"),
        |guest, entry| stuck_update(guest, entry),
    );
    let distinct: std::collections::BTreeSet<_> = sequences.iter().collect();
    assert!(
        distinct.len() > 40,
        "distinct engine call sequences: {}",
        distinct.len()
    );
}
