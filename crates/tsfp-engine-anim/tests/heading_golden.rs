//! T1668 tier 1: `0x58690` and `0x5A220` against the ORIGINAL (`tools/t1668_heading_gold.py`, worlds regenerated from the seed).
use tsfp_engine_anim::heading::{current_heading, heading};

mod support;
use support::Rng;

#[test]
fn heading_track_equals_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/heading.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let (mut total, mut stuck, mut exact) = (0, 0, 0);
    for line in text.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let function = u32::from_str_radix(f[0], 16).unwrap();
        let mut r = Rng(f[1].parse().unwrap());
        let count = r.below(6) + 1;
        let mut keys: Vec<u32> = (0..count).map(|_| r.below(240) as u32).collect();
        keys.sort_unstable();
        keys.dedup();
        let values: Vec<f32> = keys.iter().map(|_| (((r.below(2000) as i64 - 1000) as f64) / 100.0) as f32).collect();
        let duration = ((keys[keys.len() - 1] as f64) / 60.0) as f32;
        let time = ((r.below((f64::from(duration) * 600.0) as u64 + 2) as f64) / 10.0) as f32;
        let has_track = r.below(5) != 0;
        let track = has_track.then_some(values.as_slice());
        let got = if function == 0x58690 { heading(track, &keys, duration, time) } else { current_heading(track, &keys, duration, time) };
        if f[2] == "X" {
            assert!(got.is_none(), "{line}");
            stuck += 1;
        } else {
            let want = f64::from_bits(u64::from_str_radix(f[2], 16).unwrap());
            let got = got.expect(line);
            if got.to_bits() == want.to_bits() {
                exact += 1;
            }
            assert!((got - want).abs() < 1e-9 || got.to_bits() == want.to_bits(), "{line}: {got} vs {want}");
        }
        total += 1;
    }
    assert!(total == 1000 && exact * 10 >= (total - stuck) * 9, "{total} {stuck} {exact}");
}
