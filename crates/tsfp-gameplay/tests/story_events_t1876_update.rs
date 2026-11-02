//! T1876: the remaining effect pool update callbacks against emulated-original golden vectors (MEASURED). Same method as T1871: the original
//! ran the callback AND its helper family for real under the Unicorn oracle (only engine leaves scripted); the port runs the helper family
//! through `eff_dispatch::native`. Goldens `tests/data/t1876/eff_*.golden` (`tools/t1846_gold/eff_update2.py`, `tools/t1876_regen.sh`).
mod common;
use common::guest::GoldenGuest;
use common::parse;
use tsfp_gameplay::story_events::eff_dispatch::NATIVE;
use tsfp_gameplay::story_events::eff_update2::*;

const ANCHOR: u32 = 0xF000_0000;

/// Run every case; returns the distinct engine call sequences (coverage of the control flow).
fn run_whole(text: &str, root: impl Fn(&mut GoldenGuest, u32)) -> usize {
    let cases = parse(text);
    assert!(!cases.is_empty());
    let mut sequences = std::collections::BTreeSet::new();
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index).with_native(&NATIVE);
        let entry = case.input(ANCHOR);
        root(&mut guest, entry);
        guest.finish();
        sequences.insert(case.calls.iter().map(|c| c.callee).collect::<Vec<_>>());
    }
    eprintln!(
        "cases {} distinct engine call sequences {}",
        cases.len(),
        sequences.len()
    );
    sequences.len()
}

#[test]
fn beam_update_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_f5ff0.golden"),
        |guest, entry| beam_update(guest, entry),
    );
    assert!(distinct > 12, "distinct engine call sequences: {distinct}");
}

#[test]
fn bolt_update_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_f5920.golden"),
        |guest, entry| bolt_update(guest, entry),
    );
    assert!(distinct > 20, "distinct engine call sequences: {distinct}");
}

#[test]
fn splash_update_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_f61e0.golden"),
        |guest, entry| splash_update(guest, entry),
    );
    assert!(distinct > 20, "distinct engine call sequences: {distinct}");
}

#[test]
fn flame_update_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_ebb00.golden"),
        |guest, entry| flame_update(guest, entry),
    );
    assert!(distinct > 20, "distinct engine call sequences: {distinct}");
}

#[test]
fn spark_update_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_ec120.golden"),
        |guest, entry| spark_update(guest, entry),
    );
    assert!(distinct > 20, "distinct engine call sequences: {distinct}");
}

#[test]
fn trace_update_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_e4800.golden"),
        |guest, entry| trace_update(guest, entry),
    );
    assert!(distinct > 20, "distinct engine call sequences: {distinct}");
}

#[test]
fn bounce_update_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_f51f0.golden"),
        |guest, entry| bounce_update(guest, entry),
    );
    assert!(distinct > 20, "distinct engine call sequences: {distinct}");
}

#[test]
fn drone_update_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_ecb90.golden"),
        |guest, entry| drone_update(guest, entry),
    );
    assert!(distinct > 20, "distinct engine call sequences: {distinct}");
}
