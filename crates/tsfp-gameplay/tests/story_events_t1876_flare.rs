//! T1876: the flare update callback 0xF1A90 against emulated-original golden vectors (MEASURED). Same method as `story_events_t1876_update`: the original ran
//! the callback, its helper family and the CRT fmod for real under the Unicorn oracle (only engine leaves scripted). Golden `tests/data/t1876/eff_f1a90.golden`
//! (`tools/t1846_gold/eff_flare.py`).
mod common;
use common::guest::GoldenGuest;
use common::parse;
use tsfp_gameplay::story_events::eff_dispatch::NATIVE;

use tsfp_gameplay::story_events::eff_flare::flare_update;

const ANCHOR: u32 = 0xF000_0000;

#[test]
fn flare_update_matches_original() {
    let cases = parse(include_str!("data/t1876/eff_f1a90.golden"));
    assert!(!cases.is_empty());
    let mut sequences = std::collections::BTreeSet::new();
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index).with_native(&NATIVE);
        let entry = case.input(ANCHOR);
        flare_update(&mut guest, entry);
        guest.finish();
        sequences.insert(case.calls.iter().map(|c| c.callee).collect::<Vec<_>>());
    }
    eprintln!(
        "cases {} distinct engine call sequences {}",
        cases.len(),
        sequences.len()
    );
    assert!(
        sequences.len() > 25,
        "distinct engine call sequences: {}",
        sequences.len()
    );
}
