//! T1873: `0x1526C0` matrix blend, `0x152540` matrix to quaternion and `0x150960` slerp against numbers recorded from
//! the ORIGINAL code under Unicorn (generator `tools/t1873_blend_gold.py`, FABRICATED inputs). Bit exact, NaN payloads included.
use tsfp_engine_anim::blend::{matrix_blend, matrix_to_quaternion, slerp};

fn words(fields: &[&str]) -> Vec<u32> {
    fields.iter().map(|f| u32::from_str_radix(f, 16).unwrap()).collect()
}

fn floats<const N: usize>(w: &[u32]) -> [f32; N] {
    std::array::from_fn(|i| f32::from_bits(w[i]))
}

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|v| v.to_bits()).collect()
}

#[test]
fn blend_and_callees_equal_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1873/blend.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let (mut blends, mut quats, mut slerps, mut branches) = (0, 0, 0, std::collections::BTreeSet::new());
    for (number, line) in text.lines().enumerate() {
        let fields: Vec<&str> = line.split(' ').collect();
        let w = words(&fields[1..]);
        match fields[0] {
            "B" => {
                let got = matrix_blend(&floats::<16>(&w[0..16]), &floats::<16>(&w[16..32]), f32::from_bits(w[32]));
                assert_eq!(bits(&got), w[33..49], "blend line {number}");
                blends += 1;
            }
            "Q" => {
                let m = floats::<16>(&w[0..16]);
                assert_eq!(bits(&matrix_to_quaternion(&m)), w[16..20], "matrix to quaternion line {number}");
                branches.insert(if m[5] + m[0] + m[10] > 0.0 { 0 } else if m[5] > m[0] || m[10] > m[0] { 1 } else { 2 });
                quats += 1;
            }
            "S" => {
                let got = slerp(floats::<4>(&w[0..4]), floats::<4>(&w[4..8]), f32::from_bits(w[8]));
                assert_eq!(bits(&got), w[9..13], "slerp line {number}");
                slerps += 1;
            }
            other => panic!("unknown case {other}"),
        }
    }
    assert!(blends >= 300 && quats >= 300 && slerps >= 600 && branches.len() == 3, "{blends} {quats} {slerps} {branches:?}");
    eprintln!("MEASURED blend: {blends} blends, {quats} matrix to quaternion, {slerps} slerps identical to the original");
}
