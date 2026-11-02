//! T1877: real data checks of the character animation selection (owner ISO, SKIP when absent). Reads through
//! `tools/rust_anim_sample.py` and `tools/t1875_chr_dump.py`; no game data is stored.
//!
//! The necessary condition (T1875): a clip drives a model only when its node count equals the model's animation node count N.
//! For every class tick of the dispatchers, the ids it requests must resolve to clips whose node counts share a value
//! (one skeleton per class); the models with that N are the types the class can drive.
use std::collections::{BTreeMap, BTreeSet};
use tsfp_format_anim::Clip;
use tsfp_format_anim::chrmodel::ChrModel;
use tsfp_format_anim::propset::{AnimSetEntry, PropTable, chr_entry_key};
use tsfp_format_pak::Pak;
use tsfp_gameplay::char_anim::{CLASS_TICKS_MODE_A, CLASS_TICKS_OTHER, class_ids};

fn run(module: &str, args: &[&str]) -> Option<Vec<u8>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let python = std::env::var("TSFP_TEST_PYTHON").unwrap_or("python3".into());
    let output = std::process::Command::new(python)
        .current_dir(root)
        .args(["-m", module])
        .args(args)
        .output()
        .expect("Python reader");
    if output.status.code() == Some(77) {
        eprintln!("SKIP char anim: private ISO absent");
        return None;
    }
    assert!(
        output.status.success(),
        "{module} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(output.stdout)
}

fn props(stream: &[u8]) -> Vec<(String, u32, Vec<u8>)> {
    let (mut at, mut frames) = (0usize, Vec::new());
    let u32_at = |o: usize| u32::from_le_bytes(stream[o..o + 4].try_into().unwrap()) as usize;
    while at < stream.len() {
        let n = u16::from_le_bytes([stream[at], stream[at + 1]]) as usize;
        let path = String::from_utf8(stream[at + 2..at + 2 + n].to_vec()).unwrap();
        at += 2 + n;
        let (level, len) = (u32_at(at), u32_at(at + 4));
        frames.push((path, level as u32, stream[at + 8..at + 8 + len].to_vec()));
        at += 8 + len;
    }
    frames
}

#[test]
fn class_ticks_share_a_skeleton_and_drive_real_models() {
    let (Some(chr), Some(sets), Some(clips), Some(prop_stream)) = (
        run("tools.rust_anim_sample", &["--raw", "pak/chr.pak"]),
        run("tools.t1875_chr_dump", &["--animsets"]),
        run("tools.rust_anim_sample", &[]),
        run("tools.t1875_chr_dump", &["--props"]),
    ) else {
        return;
    };
    let (mut at, mut nodes_by_key) = (0usize, BTreeMap::new());
    let u32_at = |o: usize| u32::from_le_bytes(clips[o..o + 4].try_into().unwrap());
    while at < clips.len() {
        let n = u16::from_le_bytes([clips[at], clips[at + 1]]) as usize;
        at += 2 + n;
        let (key, len) = (u32_at(at + 4), u32_at(at + 8) as usize);
        nodes_by_key.entry(key).or_insert_with(|| {
            Clip::parse(&clips[at + 12..at + 12 + len])
                .unwrap()
                .nodes
                .len()
        });
        at += 12 + len;
    }
    // id -> node counts of every clip an anim set can bind to it (the level load order picks one of them)
    let mut counts_by_id: BTreeMap<u16, BTreeSet<usize>> = BTreeMap::new();
    for line in String::from_utf8(sets).unwrap().lines() {
        let entry = AnimSetEntry::from_line(line).unwrap();
        counts_by_id
            .entry(entry.id)
            .or_default()
            .insert(nodes_by_key[&entry.clip_key()]);
    }
    // character models of the level tables, distinct by path, with their node count
    let pak = Pak::parse(&chr).unwrap();
    let index: BTreeMap<u32, usize> = pak
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.key, i))
        .collect();
    let mut models: BTreeMap<String, usize> = BTreeMap::new();
    for (_, _, bytes) in props(&prop_stream) {
        let table = PropTable::parse(&bytes).unwrap();
        for row in &table.rows {
            let Some(path) = table.path(row).filter(|p| p.starts_with("chrs/")) else {
                continue;
            };
            if let Some(i) = index.get(&chr_entry_key(path)) {
                models.insert(
                    path.to_string(),
                    ChrModel::parse(pak.payload(*i).unwrap())
                        .unwrap()
                        .info
                        .anim_nodes() as usize,
                );
            }
        }
    }
    let mut models_by_nodes: BTreeMap<usize, usize> = BTreeMap::new();
    for nodes in models.values() {
        *models_by_nodes.entry(*nodes).or_default() += 1;
    }
    let (mut classes, mut shared, mut with_models, mut inconsistent) = (0, 0, 0, Vec::new());
    for (mode_a, table) in [(true, CLASS_TICKS_MODE_A), (false, CLASS_TICKS_OTHER)] {
        for (mask, _) in table {
            let ids = class_ids(mode_a, *mask);
            let sets: Vec<&BTreeSet<usize>> =
                ids.iter().filter_map(|id| counts_by_id.get(id)).collect();
            if sets.len() < 2 {
                continue;
            }
            classes += 1;
            let mut common = sets[0].clone();
            for set in &sets[1..] {
                common = common.intersection(set).copied().collect();
            }
            if common.is_empty() {
                inconsistent.push((mode_a, *mask, ids.len()));
                continue;
            }
            shared += 1;
            let drivable: usize = common.iter().filter_map(|n| models_by_nodes.get(n)).sum();
            with_models += usize::from(drivable > 0);
            eprintln!(
                "class {mask:#x} modeA={mode_a}: ids {} skeleton node counts {common:?} drivable character models {drivable}",
                ids.len()
            );
        }
    }
    // armed pose ids 0x69..=0x71 (the 0x1C91C0 selector)
    let pose: BTreeMap<u16, &BTreeSet<usize>> = (0x69u16..=0x71)
        .filter_map(|id| counts_by_id.get(&id).map(|set| (id, set)))
        .collect();
    eprintln!("MEASURED armed pose ids: {:?}", pose);
    eprintln!(
        "MEASURED character classes: {classes} with >=2 resolvable ids, {shared} share a skeleton, {with_models} have a matching real model, inconsistent {inconsistent:x?}"
    );
    assert!(classes >= 20, "{classes}");
    assert!(
        shared * 10 >= classes * 8,
        "{shared}/{classes} {inconsistent:x?}"
    );
    assert!(with_models * 10 >= shared * 7, "{with_models}/{shared}");
    assert_eq!(pose.len(), 9, "all nine armed pose ids resolve to a clip");
}
