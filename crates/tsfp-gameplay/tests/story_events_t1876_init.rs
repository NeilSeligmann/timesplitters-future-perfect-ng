//! T1876: the effect pool init callbacks (class slot `+0x30`) against emulated-original golden vectors (MEASURED). The original ran under the Unicorn oracle
//! with only the engine leaves scripted (the random generator `0x153B40` and the unit basis `0xE0F30` real). Goldens `tests/data/t1876/eff_init_*.golden`
//! (`tools/t1846_gold/eff_init.py`, `tools/t1876_regen.sh`).
mod common;
use common::guest::GoldenGuest;
use common::parse;
use tsfp_gameplay::story_events::eff_dispatch::NATIVE;
use tsfp_gameplay::story_events::eff_init::*;

const ANCHOR: u32 = 0xF000_0000;

/// Run every case; returns the distinct engine call sequences.
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
fn init_arrow_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e0a80.golden"),
        |guest, entry| init_arrow(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_owner_70b_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e0b50.golden"),
        |guest, entry| init_owner_70b(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_owner_70e_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e0bd0.golden"),
        |guest, entry| init_owner_70e(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_owner_711_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e0c50.golden"),
        |guest, entry| init_owner_711(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_sound_quad_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e14b0.golden"),
        |guest, entry| init_sound_quad(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_sound_single_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e1620.golden"),
        |guest, entry| init_sound_single(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_trail_sounds_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e1650.golden"),
        |guest, entry| init_trail_sounds(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_burst_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e28d0.golden"),
        |guest, entry| init_burst(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_part_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e2df0.golden"),
        |guest, entry| init_part(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_shell_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e2f90.golden"),
        |guest, entry| init_shell(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_debris_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e34d0.golden"),
        |guest, entry| init_debris(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_orb_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e3570.golden"),
        |guest, entry| init_orb(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_bolt_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e3640.golden"),
        |guest, entry| init_bolt(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_beam_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e36d0.golden"),
        |guest, entry| init_beam(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_jet_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e3ab0.golden"),
        |guest, entry| init_jet(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_trace_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e4690.golden"),
        |guest, entry| init_trace(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_flash_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_e78f0.golden"),
        |guest, entry| init_flash(guest, entry),
    );
    assert!(distinct >= 1);
}

#[test]
fn init_drone_matches_original() {
    let distinct = run_whole(
        include_str!("data/t1876/eff_init_ecac0.golden"),
        |guest, entry| init_drone(guest, entry),
    );
    assert!(distinct >= 1);
}
