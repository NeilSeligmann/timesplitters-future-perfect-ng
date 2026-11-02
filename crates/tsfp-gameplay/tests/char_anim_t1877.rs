//! T1877: the armed pose selector `0x1C91C0` against the ORIGINAL under Unicorn (numbers only golden
//! `tests/data/t1877/armed_pose.golden`, produced by `tools/t1877_pose_gold.py`).
use tsfp_gameplay::char_anim::{ArmedPoseInputs, Lcg64, select_armed_pose};

fn hex(token: &str) -> u32 {
    u32::from_str_radix(token, 16).unwrap()
}

#[test]
fn armed_pose_selector_matches_the_original() {
    let text = include_str!("data/t1877/armed_pose.golden");
    let (mut cases, mut draws, mut ids) = (0usize, 0usize, std::collections::BTreeSet::new());
    for line in text.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(tokens[0], "P");
        let inputs = ArmedPoseInputs {
            previous_id: hex(tokens[1]),
            c80: hex(tokens[2]),
            c94: hex(tokens[3]),
            class_mask: u64::from(hex(tokens[4])) | u64::from(hex(tokens[5])) << 32,
            c2bc: hex(tokens[6]),
            c18c: hex(tokens[7]),
            weapon_flags: hex(tokens[8]),
            record_flags: (tokens[9] != "N").then(|| hex(tokens[9])),
            object_150: hex(tokens[10]),
        };
        let state = u64::from(hex(tokens[11])) | u64::from(hex(tokens[12])) << 32;
        assert_eq!(tokens[13], "=");
        let (mask, id) = (hex(tokens[14]), hex(tokens[15]));
        let after = u64::from(hex(tokens[16])) | u64::from(hex(tokens[17])) << 32;
        let mut rng = Lcg64 { state };
        let choice = select_armed_pose(&inputs, &mut rng);
        assert_eq!((choice.mask, choice.id), (mask, id), "{line}");
        assert_eq!(rng.state, after, "rng state after {line}");
        draws += usize::from(after != state);
        ids.insert(id);
        cases += 1;
    }
    eprintln!("MEASURED armed pose: {cases} cases, {draws} draw the rng, ids {ids:x?}");
    assert!(cases >= 1000 && draws > 150 && ids.len() >= 6);
}
