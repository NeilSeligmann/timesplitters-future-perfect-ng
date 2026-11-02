//! T1668: root motion tail of `0x5AAA0` against the ORIGINAL (`tools/t1668_tail_gold.py`).
#[path = "support/bits.rs"]
mod bits_support;
use bits_support::f32_bits as f;
use tsfp_engine_anim::tail::{OWNER_MATRICES_CHANGED, RootMotion};

#[test]
fn root_motion_tail_equals_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/tail.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let (mut tracked, mut total) = (0, 0);
    for line in text.lines() {
        let w: Vec<&str> = line.split(' ').collect();
        let (p0, p1) = (u32::from_str_radix(w[1], 16).unwrap(), u32::from_str_radix(w[2], 16).unwrap());
        let v = |at: usize| [f(w[at]), f(w[at + 1]), f(w[at + 2])];
        let mut motion = RootMotion { position: v(3), previous: v(6), delta: [f32::from_bits(0xA5A5_A5A5); 3] };
        motion.update(p0, p1);
        let want = |at: usize| [w[at], w[at + 1], w[at + 2]].map(|h| u32::from_str_radix(h, 16).unwrap());
        let bits = |a: [f32; 3]| a.map(f32::to_bits);
        assert_eq!(bits(motion.position), want(10), "{line}");
        assert_eq!(bits(motion.previous), want(13), "{line}");
        assert_eq!(bits(motion.delta), want(16), "{line}");
        assert_eq!(u32::from_str_radix(w[19], 16).unwrap(), OWNER_MATRICES_CHANGED, "{line}");
        tracked += usize::from(tsfp_engine_anim::tail::tracks_root_motion(p0, p1));
        total += 1;
    }
    assert!(total >= 400 && tracked > 100 && tracked < total, "{tracked}/{total}");
}
