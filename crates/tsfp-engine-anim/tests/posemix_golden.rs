//! T1668: `0x60540`, `0x605D0` and `0x604D0` against the ORIGINAL (`tools/t1668_posemix_gold.py`).
use tsfp_engine_anim::pose::Mat4;
use tsfp_engine_anim::posemix::{TreeRow, blend_by_weight, compose_or_copy, compose_tree};

fn matrices(text: &str) -> Vec<Mat4> {
    text.split(';').map(|m| m.split(',').map(|w| f32::from_bits(u32::from_str_radix(w, 16).unwrap())).collect::<Vec<f32>>().try_into().unwrap()).collect()
}

fn bits(m: &[Mat4]) -> Vec<Vec<u32>> {
    m.iter().map(|x| x.iter().map(|v| v.to_bits()).collect()).collect()
}

#[test]
fn pose_mix_loops_equal_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/posemix.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let (mut blends, mut composes, mut trees, mut touched) = (0, 0, 0, 0);
    for line in text.lines() {
        let (input, expected) = line.split_once(" -> ").unwrap();
        let fields: Vec<&str> = input.split(' ').collect();
        let want = matrices(expected);
        if fields[0] == "T" {
            let tree: Vec<TreeRow> = fields[1]
                .split(',')
                .map(|row| {
                    let v: Vec<i32> = row.split('/').map(|x| x.parse().unwrap()).collect();
                    TreeRow { parent: v[0], child: v[1], sibling: v[2] }
                })
                .collect();
            let mut mats = matrices(fields[2]);
            compose_tree(&tree, &mut mats, 0);
            assert_eq!(bits(&mats), bits(&want), "{line}");
            trees += 1;
            continue;
        }
        let full = fields[1] == "1";
        let entries: Vec<(u32, f32)> = fields[3]
            .split(',')
            .map(|e| {
                let (p, w) = e.split_once(':').unwrap();
                (p.parse().unwrap(), f32::from_bits(u32::from_str_radix(w, 16).unwrap()))
            })
            .collect();
        let parents: Vec<i32> = fields[4].split(',').map(|x| x.parse().unwrap()).collect();
        let base = matrices(fields[5]);
        let pose = matrices(fields[6]);
        let mut out = vec![[f32::from_bits(0xA5A5_A5A5); 16]; 6];
        if fields[0] == "B" {
            blend_by_weight(&entries, &base, &pose, &mut out, full);
            blends += 1;
        } else {
            compose_or_copy(&entries, &parents, &pose, &mut out);
            composes += 1;
        }
        touched += out.iter().filter(|m| m[0].to_bits() != 0xA5A5_A5A5).count();
        assert_eq!(bits(&out[..want.len()]), bits(&want), "{line}");
    }
    assert!(blends == 300 && composes == 300 && trees == 300 && touched > 400, "{blends} {composes} {trees} {touched}");
}
