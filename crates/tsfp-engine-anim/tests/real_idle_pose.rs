//! T1877: the pose chain for the IDLE clips the game plays (ids of `IDLE_IDS` in `tools/t1877_idle_pose_gold.py`) against the
//! original: 70 models, 209 poses. Copy of real_pose.rs with its own golden; digests only (generator `tools/t1877_idle_pose_gold.py`). Skips
//! (SKIP line) when the private ISO is absent.
use tsfp_engine_anim::pose::{Chain, Part, evaluate_pose};
use tsfp_format_anim::{Clip, chrmodel::ChrModel, skeleton::PartTable};
use tsfp_format_pak::Pak;

#[path = "support/sample_stream.rs"]
mod sample_stream;
use sample_stream::stream;

#[path = "support/fnv.rs"]
mod digest;
use digest::fnv;

fn parts_of(table: &PartTable) -> Vec<Part> {
    table
        .records
        .iter()
        .map(|r| Part {
            kind: r.kind(),
            cell: r.cell(),
            child: r.child(),
            sibling: r.sibling(),
        })
        .collect()
}

#[test]
fn idle_clips_the_game_plays_match_the_original_pose_chain() {
    let Some(chr) = stream(&["--raw", "pak/chr.pak"]) else {
        return;
    };
    let clips = stream(&[]).unwrap();
    let golden = std::fs::read_to_string(format!(
        "{}/tests/data/t1877/idle_pose.golden",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("golden file");
    // every resident clip by (PAK path, entry ordinal)
    let (mut at, mut by_clip) = (0usize, std::collections::BTreeMap::new());
    let u32_at = |o: usize| u32::from_le_bytes(clips[o..o + 4].try_into().unwrap());
    while at < clips.len() {
        let n = u16::from_le_bytes([clips[at], clips[at + 1]]) as usize;
        let path = String::from_utf8(clips[at + 2..at + 2 + n].to_vec()).unwrap();
        at += 2 + n;
        let (ordinal, len) = (u32_at(at), u32_at(at + 8) as usize);
        by_clip.insert(
            (path, ordinal),
            Clip::parse(&clips[at + 12..at + 12 + len]).unwrap(),
        );
        at += 12 + len;
    }
    let pak = Pak::parse(&chr).unwrap();
    let mut models = std::collections::BTreeMap::new();
    let mut compared = 0;
    for line in golden.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let (model, offset, count): (usize, usize, usize) = (
            f[0].parse().unwrap(),
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
        );
        let (clip_ordinal, key): (u32, i32) = (f[4].parse().unwrap(), f[5].parse().unwrap());
        let want = u64::from_str_radix(f[6], 16).unwrap();
        let parsed = models
            .entry(model)
            .or_insert_with(|| ChrModel::parse(pak.payload(model).unwrap()).unwrap());
        assert_eq!(
            (parsed.parts.offset, parsed.parts.records.len()),
            (offset, count),
            "model {model}: container parse disagrees with the golden table"
        );
        parsed.validate().unwrap();
        let clip = &by_clip[&(f[3].to_string(), clip_ordinal)];
        assert!(
            parsed.accepts_clip(clip),
            "model {model} does not accept clip {}#{clip_ordinal}",
            f[3]
        );
        let matrices =
            evaluate_pose(clip, &parts_of(&parsed.parts), 0, key, 1.0, Chain::A).unwrap();
        let mut state = 0xCBF2_9CE4_8422_2325u64;
        for matrix in &matrices {
            for cell in matrix {
                state = fnv(state, &cell.to_bits().to_le_bytes());
            }
        }
        assert_eq!(
            state, want,
            "model {model} table {offset} clip {} #{clip_ordinal} key {key}",
            f[3]
        );
        compared += 1;
    }
    assert!(compared >= 150, "only {compared} cases");
    eprintln!(
        "MEASURED idle pose (T1877): {compared} (model, clip, key) poses identical to the original, {} models",
        models.len()
    );
}
