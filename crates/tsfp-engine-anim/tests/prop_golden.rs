//! T1668: `0x5F260` and `0x5F040` against the ORIGINAL (`tools/t1668_prop_gold.py`).
use tsfp_engine_anim::pose::Sample;
use tsfp_engine_anim::prop::{prop_body_pose, state_matrix};

#[path = "support/hex.rs"]
mod hex_support;
use hex_support::hex;

/// The generator's sampler (node 0, scale 1.0).
fn sampler(key: i32) -> Sample {
    let angle = 0.11f64 * (key & 0xFFFF) as f64;
    let axis = [(angle * 1.3 + 1.0).sin(), (angle * 0.7).cos(), (angle * 0.4 + 2.0).sin()];
    let norm = axis.iter().map(|a| a * a).sum::<f64>().sqrt();
    let half = angle * 0.5;
    let mut bits = [0u32; 7];
    for i in 0..3 {
        bits[i] = ((key as f64 * 0.75 + i as f64 * 0.5 - 3.0) as f32).to_bits();
        bits[3 + i] = ((axis[i] / norm * half.sin()) as f32).to_bits();
    }
    bits[6] = (half.cos() as f32).to_bits();
    Sample { bits }
}

#[test]
fn prop_evaluators_equal_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/prop.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let (mut states, mut bodies) = (0, 0);
    for line in text.lines() {
        let parts: Vec<&str> = line.split(" | ").collect();
        let head: Vec<&str> = parts[0].split(' ').collect();
        let keys: Vec<u32> = head[1].split(',').map(|k| k.parse().unwrap()).collect();
        let (duration, time) = (f32::from_bits(hex(head[2])), f32::from_bits(hex(head[3])));
        let words: Vec<u32> = parts[1].split(' ').map(hex).collect();
        if head[0] == "M" {
            let matrix = state_matrix(&keys, duration, time, sampler).unwrap();
            assert_eq!(matrix.map(f32::to_bits).to_vec(), words, "{line}");
            states += 1;
        } else {
            let count: usize = head[4].parse().unwrap();
            let mut matrices = vec![[0.0f32; 16]; count];
            let position = prop_body_pose(&keys, duration, time, &mut matrices, sampler).unwrap();
            let got: Vec<u32> = matrices.iter().flat_map(|m| m.map(f32::to_bits)).collect();
            assert_eq!(got, words, "{line}");
            let tail: Vec<u32> = parts[2].split(' ').map(hex).collect();
            assert_eq!(position.map(f32::to_bits).to_vec(), tail[..3], "{line}");
            assert_eq!(tail[3], 0x1234);
            bodies += 1;
        }
    }
    assert!(states > 200 && bodies > 200, "{states} {bodies}");
}
