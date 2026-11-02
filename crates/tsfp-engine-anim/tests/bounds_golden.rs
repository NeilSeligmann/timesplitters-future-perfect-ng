//! T1668 tier 1: `0x60F80` against the ORIGINAL (`tools/t1668_bounds_gold.py`, worlds regenerated from the seed).
use tsfp_engine_anim::bounds::{BoundsClip, pose_bounds};
use tsfp_engine_anim::pose::Sample;

mod support;
use support::Rng;

fn sampler(node: usize, key: i32) -> Sample {
    let angle = 0.11f64 * (key & 0xFFFF) as f64 + 0.3 * node as f64;
    let axis = [(angle * 1.3 + 1.0).sin(), (angle * 0.7).cos(), (angle * 0.4 + 2.0).sin()];
    let norm = axis.iter().map(|a| a * a).sum::<f64>().sqrt();
    let half = angle * 0.5;
    let mut bits = [0u32; 7];
    for i in 0..3 {
        bits[i] = ((key as f64 * 0.75 + node as f64 * 1.5 + i as f64 * 0.5 - 3.0) as f32).to_bits();
        bits[3 + i] = ((axis[i] / norm * half.sin()) as f32).to_bits();
    }
    bits[6] = (half.cos() as f32).to_bits();
    Sample { bits }
}

#[test]
fn pose_bounds_equal_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/bounds.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut total = 0;
    for line in text.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let mut r = Rng(f[0].parse().unwrap());
        let count = r.below(8) + 2;
        let mut keys: Vec<u32> = (0..count).map(|_| r.below(200) as u32).collect();
        keys.sort_unstable();
        keys.dedup();
        let last = f64::from(keys[keys.len() - 1]);
        let duration = [(last / 60.0) as f32, (last / 60.0) as f32, 0.0][r.below(3) as usize];
        let nodes = r.below(4) as usize + 1;
        let owner_time = (((r.below(600) as i64 - 100) as f64) / 100.0) as f32;
        let offset = [0.0f32, (((r.below(300) as i64 - 100) as f64) / 100.0) as f32][r.below(2) as usize];
        let mut layer_time = ((r.below(300) as f64) / 100.0) as f32;
        let policy = ([0u32, 0x82, 0x2][r.below(3) as usize], [0u32, 0x1800, 0x800][r.below(3) as usize]);
        let snap = r.below(2) != 0;
        let clip = BoundsClip { keys: &keys, duration, nodes };
        let (min, max) = pose_bounds(&clip, owner_time, offset, policy, snap, &mut layer_time, sampler, &mut [[0.0f32; 16]; 8], |_| {}).unwrap();
        let got: Vec<String> = min.iter().chain(max.iter()).map(|v| format!("{:x}", v.to_bits())).collect();
        assert_eq!(got.join(" "), f[1..7].join(" "), "{line}");
        assert_eq!(format!("{:x}", layer_time.to_bits()), f[7], "{line}");
        total += 1;
    }
    assert!(total > 400);
}
