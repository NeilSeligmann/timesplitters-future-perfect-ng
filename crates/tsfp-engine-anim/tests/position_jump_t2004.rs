//! T2004 Tier 1: `0x60920` position jump gate against numbers recorded from the ORIGINAL under Unicorn
//! (`tools/t2004_position_jump_gold.py`, key store 0x606E0 stubbed, memory constants varied per case).
#[path = "support/bits.rs"]
mod bits_support;
use bits_support::f32_bits as bits;
use std::collections::HashMap;
use tsfp_engine_anim::advance::position_jump_at;

#[test]
fn position_jump_equals_the_original() {
    let text = include_str!("data/t2004/position_jump.golden");
    let (mut checked, mut jumps, mut three_keys) = (0, 0, 0);
    for line in text.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let frames = u32::from_str_radix(f[1], 16).unwrap() as i32;
        let (time, fps, jump_squared) = (bits(f[2]), bits(f[3]), bits(f[4]));
        let divisor = u32::from_str_radix(f[5], 16).unwrap() as i32;
        let want = u32::from_str_radix(f[6], 16).unwrap() != 0;
        let mut keys: HashMap<i32, [f32; 3]> = HashMap::new();
        if f[7] != "-" {
            for entry in f[7].split(',') {
                let p: Vec<&str> = entry.split(':').collect();
                let frame = u32::from_str_radix(p[0], 16).unwrap() as i32;
                keys.insert(frame, [bits(p[1]), bits(p[2]), bits(p[3])]);
            }
        }
        let tick = 1.0f32 / divisor as f32;
        let got = position_jump_at(frames, time, fps, jump_squared, tick, |frame| {
            Some(*keys.get(&frame).unwrap_or_else(|| panic!("key {frame} not read by the original: {line}")))
        });
        assert_eq!(got, Some(want), "{line}");
        checked += 1;
        jumps += usize::from(want);
        three_keys += usize::from(keys.len() >= 3);
    }
    assert!(checked >= 2000 && jumps > 500 && three_keys > 50, "{checked} {jumps} {three_keys}");
}
