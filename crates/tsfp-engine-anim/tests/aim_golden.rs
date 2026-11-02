//! T1668: `0x5AFB0` against the ORIGINAL (`tools/t1668_aim_gold.py`).
use std::cell::RefCell;
use tsfp_engine_anim::aim::{BlendInput, Cursor, blend_space_pose, is_plain};
use tsfp_engine_anim::pose::Sample;

#[path = "support/hex.rs"]
mod hex_support;
use hex_support::hex;

#[test]
fn blend_space_pose_equals_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/aim.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let (mut total, mut mixes, mut plains) = (0, 0, 0);
    for line in text.lines() {
        let (head, tail) = line.split_once(" : ").unwrap();
        let h: Vec<&str> = head.split(' ').collect();
        let (mode, node, flag): (u32, i32, u32) = (h[1].parse().unwrap(), h[2].parse().unwrap(), h[3].parse().unwrap());
        let cursor = Cursor {
            column_weight: f32::from_bits(hex(h[4])),
            row_weight: f32::from_bits(hex(h[5])),
            rows: [h[7].parse().unwrap(), h[9].parse().unwrap()],
            columns: [h[8].parse().unwrap(), h[10].parse().unwrap()],
        };
        let mix = f32::from_bits(hex(h[6]));
        let input = if mode == 0 && is_plain(flag, node) {
            BlendInput::Plain
        } else if mode != 0 {
            BlendInput::Mix(mix)
        } else {
            BlendInput::Single
        };
        let calls = RefCell::new(Vec::new());
        // the stub of the generator: page = clip index + 1 (+ 2 for the plain clip: page 3), see tools/t1668_aim_gold.py
        let stub = |clip: usize, key: i32| {
            let page = [1, 2, 3][clip];
            calls.borrow_mut().push(format!("{page}/{node}/{key}"));
            let angle = 0.1f64 * page as f64 + 0.07 * (key & 0xFFFF) as f64 + 0.3 * node as f64;
            let axis = [(angle * 1.3 + 1.0).sin(), (angle * 0.7).cos(), (angle * 0.4 + 2.0).sin()];
            let norm = axis.iter().map(|a| a * a).sum::<f64>().sqrt();
            let half = angle * 0.5;
            let f = |v: f64| (v as f32).to_bits();
            let mut bits = [0u32; 7];
            for i in 0..3 {
                bits[i] = ((page as f64 * 0.5 + key as f64 * 0.125 + node as f64 + i as f64 * 0.25) as f32).to_bits();
                bits[3 + i] = f(axis[i] / norm * half.sin());
            }
            bits[6] = f(half.cos());
            Sample { bits }
        };
        let (position, quat) = blend_space_pose(&cursor, input, stub);
        let (words, call_text) = tail.split_once(" | ").unwrap();
        let want: Vec<u32> = words.split(' ').map(hex).collect();
        let got: Vec<u32> = position.iter().chain(quat.iter()).map(|v| v.to_bits()).collect();
        assert_eq!((got.clone(), calls.borrow().join(",")), (want[..7].to_vec(), call_text.to_string()), "{line}");
        assert_eq!(calls.borrow().join(","), call_text, "{line}");
        total += 1;
        mixes += usize::from(matches!(input, BlendInput::Mix(_)));
        plains += usize::from(matches!(input, BlendInput::Plain));
    }
    assert!(total == 600 && mixes > 100 && plains > 20, "{total} {mixes} {plains}");
}
