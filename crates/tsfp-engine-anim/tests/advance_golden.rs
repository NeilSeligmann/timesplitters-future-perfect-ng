//! T1668: `0x60670` time advance against numbers recorded from the ORIGINAL (`tools/t1668_advance_gold.py`).
#[path = "support/bits.rs"]
mod bits_support;
use bits_support::f32_bits as bits;
use tsfp_engine_anim::advance::advance_time;

#[test]
fn advance_time_equals_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/advance.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let (mut checked, mut stuck) = (0, 0);
    for line in text.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let got = advance_time(bits(f[1]), bits(f[2]), bits(f[3]), bits(f[4]));
        if f[6] == "X" {
            assert_eq!(got, None, "{line}");
            stuck += 1;
        } else {
            let want = u32::from_str_radix(f[6], 16).unwrap();
            let got = got.expect(line).to_bits();
            // NaN payloads: compare as NaN-ness.
            assert!(got == want || (f32::from_bits(got).is_nan() && f32::from_bits(want).is_nan()), "{line}");
        }
        checked += 1;
    }
    assert!(checked >= 500 && stuck >= 1, "{checked} {stuck}");
}
