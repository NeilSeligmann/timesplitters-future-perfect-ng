//! T1668 tier 1: `0x59A40` and `0x59B00` against the ORIGINAL (`tools/t1668_rootq_gold.py`, worlds regenerated from the seed).
use tsfp_engine_anim::keyrec::{axis_range, query_scale, root_displacement};

mod support;
use support::Rng;

fn pos(key: i32, scale: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| (key as f32 * 0.75 + i as f32 * 0.5 - 3.0) * scale)
}

#[test]
fn root_queries_equal_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/rootq.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut n = 0;
    for line in text.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let function = u32::from_str_radix(f[0], 16).unwrap();
        let mut r = Rng(f[1].parse().unwrap());
        let keys = r.below(30) as i32 + 1;
        let frames = r.below(300) as i32;
        let shape_scale = [1.0f32, 0.5, 2.0][r.below(3) as usize];
        let typed = r.below(2) == 0;
        let _slot = r.below(6);
        let null_obj = r.below(5) == 0;
        let mut out = vec!["7777".to_string(); 4];
        if function == 0x59A40 {
            let scale = query_scale((!null_obj && typed).then_some(8), shape_scale);
            let d = root_displacement(pos(0, scale), pos(keys - 1, scale));
            for (i, v) in d.iter().enumerate() {
                out[i] = format!("{:x}", v.to_bits());
            }
        } else {
            let axis = r.below(5) as u32;
            let scale = query_scale(typed.then_some(8), shape_scale);
            let (range, count) = axis_range(pos(0, scale), pos(keys - 1, scale), axis, frames);
            if let Some((a, b)) = range {
                out[0] = format!("{:x}", a.to_bits());
                out[1] = format!("{:x}", b.to_bits());
            }
            out[2] = format!("{:x}", count.to_bits());
        }
        let want = &f[2..6];
        assert_eq!(out, want, "{line}");
        n += 1;
    }
    assert_eq!(n, 600);
}
