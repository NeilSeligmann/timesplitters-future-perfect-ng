//! T1668 tier 1: `0x1CA2F0` against the ORIGINAL (`tools/t1668_levelsets_gold.py`, mount patterns regenerated from the seed).
use tsfp_engine_anim::levelsets::{ANIM_SET_PAKS, level_set_loads};

mod support;
use support::Rng;

#[test]
fn level_set_loads_equal_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/levelsets.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut total = 0;
    for line in text.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let mut r = Rng(f[0].parse().unwrap());
        let mounted: Vec<bool> = (0..ANIM_SET_PAKS.len()).map(|_| r.below(2) == 1).collect();
        assert_eq!(f[1], mounted.iter().map(|m| if *m { '1' } else { '0' }).collect::<String>());
        let got = level_set_loads(|path| mounted[ANIM_SET_PAKS.iter().position(|p| p.0 == path).unwrap()]);
        let text: Vec<String> = got.iter().map(|(entries, count)| format!("{entries:x}/{count}")).collect();
        let joined = if text.is_empty() { "-".to_string() } else { text.join(",") };
        assert_eq!(joined, f[2], "{line}");
        total += 1;
    }
    assert!(total >= 390, "{total}");
}
