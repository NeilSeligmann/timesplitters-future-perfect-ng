//! T1871: the shared helper family of the effect pool update callbacks against emulated-original golden vectors (MEASURED).
//! Goldens `tests/data/t1871/eff_*.golden` (`tools/t1846_gold/eff_helpers.py`), engine answers replayed from the saved tape.
mod common;
use common::guest::GoldenGuest;
use common::parse;
use tsfp_gameplay::story_events::eff_helpers::*;

const ARGS: u32 = 0xF000_0000;
const POS: u32 = 0x118_0000;
const POOL: u32 = 0x110_0000;

#[test]
fn ground_effect_matches_original() {
    let cases = parse(include_str!("data/t1871/eff_ground.golden"));
    assert!(!cases.is_empty());
    let mut calls = 0;
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index);
        let radius = f32::from_bits(case.input(ARGS));
        ground_effect(&mut guest, POS, radius, case.input(ARGS + 4));
        guest.finish();
        calls += case.calls.len();
    }
    assert!(
        calls > 40,
        "golden exercises the ray and release paths: {calls}"
    );
}

#[test]
fn release_hit_matches_original() {
    let cases = parse(include_str!("data/t1871/eff_release.golden"));
    assert!(!cases.is_empty());
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index);
        release_hit(&mut guest, POOL, case.input(ARGS));
        guest.finish();
    }
}

const ENTRY: u32 = POOL;

#[test]
fn source_ray_matches_original() {
    let cases = parse(include_str!("data/t1871/eff_source_ray.golden"));
    assert!(!cases.is_empty());
    let mut rays = 0;
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index);
        let [a1, a2, a3, a4] = [0, 4, 8, 12].map(|k| case.input(ARGS + k));
        let result = source_ray(&mut guest, ENTRY, a1, a2, a3, a4);
        assert_eq!(result, case.eax, "case {index}: return value");
        guest.finish();
        rays += case
            .calls
            .iter()
            .filter(|call| call.callee == 0x88C60)
            .count();
    }
    assert!(rays > 100, "rays: {rays}");
}

#[test]
fn start_light_matches_original() {
    let cases = parse(include_str!("data/t1871/eff_start_light.golden"));
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index);
        start_light(&mut guest, ENTRY, case.input(ARGS));
        guest.finish();
    }
}

#[test]
fn attached_channels_match_original() {
    let cases = parse(include_str!("data/t1871/eff_attached.golden"));
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index);
        attached_channels(&mut guest, ENTRY);
        guest.finish();
    }
}

#[test]
fn mark_players_matches_original() {
    let cases = parse(include_str!("data/t1871/eff_mark_players.golden"));
    let mut marked = 0;
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index);
        mark_players(
            &mut guest,
            ENTRY,
            case.input(ARGS),
            POS,
            f32::from_bits(case.input(ARGS + 4)),
        );
        guest.finish();
        marked += (0..4)
            .filter(|k| case.outs.get(&(0x130_0000 + 0x1000 * k + 0xB64)) == Some(&u32::MAX))
            .count();
    }
    assert!(marked > 20, "players marked in the golden: {marked}");
}

#[test]
fn refresh_contact_matches_original() {
    let cases = parse(include_str!("data/t1871/eff_refresh.golden"));
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index);
        refresh_contact(&mut guest, ENTRY);
        guest.finish();
    }
}

#[test]
fn sweep_move_matches_original() {
    let cases = parse(include_str!("data/t1871/eff_sweep.golden"));
    let mut returns = std::collections::BTreeMap::new();
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index);
        let [hit, s1, s2, s3] = [0, 4, 8, 12].map(|k| case.input(ARGS + k));
        let result = sweep_move(&mut guest, ENTRY, hit, s1, s2, s3);
        assert_eq!(result, case.eax, "case {index}: return value");
        guest.finish();
        *returns
            .entry(case.calls.iter().map(|c| c.callee).collect::<Vec<_>>())
            .or_insert(0) += 1;
    }
    assert!(
        returns.len() > 12,
        "call sequences covered: {}",
        returns.len()
    );
}

#[test]
fn hit_reaction_matches_original() {
    let cases = parse(include_str!("data/t1871/eff_reaction.golden"));
    let mut sequences = std::collections::BTreeSet::new();
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index);
        let [a0, kind, target, a5, a6] = [0, 4, 8, 12, 16].map(|k| case.input(ARGS + k));
        hit_reaction(&mut guest, a0, kind, target, 0x12B_0000, 0x12B_0020, a5, a6);
        guest.finish();
        sequences.insert(case.calls.iter().map(|c| c.callee).collect::<Vec<_>>());
    }
    assert!(
        sequences.len() > 25,
        "call sequences covered: {}",
        sequences.len()
    );
}

#[test]
fn record_helpers_match_original() {
    let init = parse(include_str!("data/t1871/eff_record_init.golden"));
    for (index, case) in init.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index);
        record_init(&mut guest, POOL);
        guest.finish();
    }
    let fill = parse(include_str!("data/t1871/eff_record_fill.golden"));
    for (index, case) in fill.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index);
        record_fill(&mut guest, case.input(ARGS), case.input(ARGS + 4));
        guest.finish();
    }
}

#[test]
fn resolve_hit_amount_matches_original() {
    let cases = parse(include_str!("data/t1871/eff_resolve.golden"));
    let mut sequences = std::collections::BTreeSet::new();
    for (index, case) in cases.iter().enumerate() {
        let mut guest = GoldenGuest::new(case, index);
        resolve_hit_amount(&mut guest, case.input(ARGS));
        guest.finish();
        sequences.insert(case.calls.iter().map(|c| c.callee).collect::<Vec<_>>());
    }
    assert!(sequences.len() >= 3, "sequences {sequences:?}");
}

#[test]
fn strength_functions_match_original() {
    for (name, text, strength) in [
        (
            "a",
            include_str!("data/t1871/eff_strength_a.golden"),
            strength_a
                as fn(&mut dyn tsfp_gameplay::story_events::eff_guest::Guest, u32, u32, u32) -> f32,
        ),
        (
            "b",
            include_str!("data/t1871/eff_strength_b.golden"),
            strength_b,
        ),
    ] {
        let cases = parse(text);
        assert!(!cases.is_empty());
        for (index, case) in cases.iter().enumerate() {
            let mut guest = GoldenGuest::new(case, index);
            let [id, x, y] = [0, 4, 8].map(|k| case.input(ARGS + k));
            let value = strength(&mut guest, id, x, y);
            let expected = f64::from_bits(case.st0.expect("st0 result")) as f32;
            assert_eq!(value.to_bits(), expected.to_bits(), "{name} case {index}");
            guest.finish();
        }
    }
}
