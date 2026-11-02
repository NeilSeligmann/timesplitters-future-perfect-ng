//! T1876: the update callback `0xEAD10` against emulated-original golden vectors (MEASURED): whole root, helper family real on both sides, only the engine
//! leaves scripted. Golden `tests/data/t1876/eff_ead10.golden` (`tools/t1846_gold/eff_update3.py`, `tools/t1876_regen.sh`).
mod common;
use common::guest::GoldenGuest;
use common::parse;
use tsfp_gameplay::story_events::eff_dispatch::NATIVE;
use tsfp_gameplay::story_events::eff_update3::*;

const ANCHOR: u32 = 0xF000_0000;

#[test]
fn part_update_matches_original() {
    let cases = parse(include_str!("data/t1876/eff_ead10.golden"));
    assert!(!cases.is_empty());
    let mut sequences = std::collections::BTreeSet::new();
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index).with_native(&NATIVE);
        let entry = case.input(ANCHOR);
        part_update(&mut guest, entry);
        guest.finish();
        sequences.insert(case.calls.iter().map(|c| c.callee).collect::<Vec<_>>());
    }
    eprintln!(
        "cases {} distinct engine call sequences {}",
        cases.len(),
        sequences.len()
    );
    assert!(
        sequences.len() > 40,
        "distinct engine call sequences: {}",
        sequences.len()
    );
}

#[test]
fn flare_hit_matches_original() {
    // `0xE3170` as its own root: the original ran with `0x350E70` and the hit family real, `0x1252C0` and `0xBB6C0` scripted
    let cases = parse(include_str!("data/t1876/eff_e3170.golden"));
    assert!(!cases.is_empty());
    let mut results = std::collections::BTreeSet::new();
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index).with_native(&NATIVE);
        let entry = case.input(ANCHOR);
        let target = case.input(ANCHOR + 4);
        const HIT: u32 = 0x0150_0000;
        let result = flare_hit(&mut guest, target, entry, HIT, HIT + 0x20);
        guest.finish();
        assert_eq!(u32::from(result), case.eax, "case {index}: result");
        results.insert(result);
    }
    assert_eq!(results.len(), 2, "both results occur");
}
