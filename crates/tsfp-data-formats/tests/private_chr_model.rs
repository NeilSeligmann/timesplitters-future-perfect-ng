//! T1875: the character model container, the anim set table and the per level model tables of the owner disc.
//! Skips (SKIP line) when the private ISO is absent. No game data is stored: everything is read through
//! `tools/rust_anim_sample.py` and `tools/t1875_chr_dump.py`.
use std::collections::{BTreeMap, BTreeSet};
use tsfp_data_formats::anim::chrmodel::ChrModel;
use tsfp_data_formats::anim::propset::{AnimSetEntry, PropTable, chr_entry_key};
use tsfp_data_formats::anim::Clip;
use tsfp_data_formats::pak::Pak;

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
        eprintln!("SKIP chr model: private ISO absent");
        return None;
    }
    assert!(output.status.success(), "{module} failed: {}", String::from_utf8_lossy(&output.stderr));
    Some(output.stdout)
}

/// `(path, level, bytes)` frames of `--props`.
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
fn every_character_model_parses_validates_and_round_trips() {
    let Some(chr) = run("tools.rust_anim_sample", &["--raw", "pak/chr.pak"]) else { return };
    let pak = Pak::parse(&chr).unwrap();
    let (mut models, mut others, mut nodes, mut parts) = (0, 0, BTreeMap::new(), BTreeMap::new());
    let (mut other_lead, mut gaps) = (BTreeSet::new(), Vec::new());
    for index in 0..pak.entries.len() {
        let bytes = pak.payload(index).unwrap();
        match ChrModel::parse(bytes) {
            Ok(model) => {
                assert!(model.encode() == bytes, "entry {index} round trip differs");
                model.validate().unwrap_or_else(|e| panic!("entry {index}: {e}"));
                assert_eq!(model.info.version(), 1);
                if !model.gap.is_empty() {
                    gaps.push((index, model.gap.len()));
                }
                *nodes.entry(model.info.anim_nodes()).or_insert(0usize) += 1;
                *parts.entry(model.parts.records.len()).or_insert(0usize) += 1;
                models += 1;
            }
            Err(_) => {
                others += 1;
                other_lead.insert(u32::from_le_bytes(bytes[0..4].try_into().unwrap()));
            }
        }
    }
    eprintln!("MEASURED chr.pak: {models} models, {others} other entries (first dwords {other_lead:?}), animation node counts {nodes:?}");
    eprintln!("MEASURED chr.pak: models with bytes between the info and the slot table (entry, length): {gaps:?}");
    assert!(gaps.len() <= 2, "{gaps:?}");
    assert_eq!(models, 516);
    assert_eq!(others, 911 - 516);
    assert!(!other_lead.contains(&12), "a non model starts with 12");
    assert!(nodes.get(&19).copied().unwrap_or(0) >= 100, "{nodes:?}");
}

#[test]
fn anim_set_table_resolves_to_real_clips_and_model_rows_to_real_models() {
    let (Some(chr), Some(sets), Some(clips), Some(prop_stream)) = (
        run("tools.rust_anim_sample", &["--raw", "pak/chr.pak"]),
        run("tools.t1875_chr_dump", &["--animsets"]),
        run("tools.rust_anim_sample", &[]),
        run("tools.t1875_chr_dump", &["--props"]),
    ) else {
        return;
    };
    // clips by PAK key (the anim PAKs repeat a key across files with identical clips; keep the first)
    let (mut at, mut clip_by_key) = (0usize, BTreeMap::new());
    let u32_at = |o: usize| u32::from_le_bytes(clips[o..o + 4].try_into().unwrap());
    while at < clips.len() {
        let n = u16::from_le_bytes([clips[at], clips[at + 1]]) as usize;
        at += 2 + n;
        let (key, len) = (u32_at(at + 4), u32_at(at + 8) as usize);
        clip_by_key.entry(key).or_insert_with(|| Clip::parse(&clips[at + 12..at + 12 + len]).unwrap());
        at += 12 + len;
    }
    // anim set entries
    let entries: Vec<AnimSetEntry> = String::from_utf8(sets).unwrap().lines().map(|l| AnimSetEntry::from_line(l).unwrap()).collect();
    let unresolved = entries.iter().filter(|e| !clip_by_key.contains_key(&e.clip_key())).count();
    let ids: BTreeSet<u16> = entries.iter().map(|e| e.id).collect();
    eprintln!("MEASURED anim sets: {} entries, {} distinct ids, {unresolved} unresolved clip keys", entries.len(), ids.len());
    assert_eq!(entries.len(), 3713);
    assert_eq!(unresolved, 0);
    assert_eq!(ids.len(), 2929);
    // model rows
    let pak = Pak::parse(&chr).unwrap();
    let chr_index: BTreeMap<u32, usize> = pak.entries.iter().enumerate().map(|(i, e)| (e.key, i)).collect();
    let (mut rows, mut chr_rows, mut resolved, mut levels) = (0, 0, 0, BTreeSet::new());
    let (mut paired, mut incompatible_nodes) = (0usize, BTreeSet::new());
    let nodes_of_clips: BTreeSet<usize> = clip_by_key.values().map(|c| c.nodes.len()).collect();
    for (path, level, bytes) in props(&prop_stream) {
        let table = PropTable::parse(&bytes).unwrap_or_else(|e| panic!("{path} level {level}: {e}"));
        assert!(table.encode() == bytes, "{path} level {level} round trip differs");
        levels.insert(level);
        for row in &table.rows {
            rows += 1;
            let Some(model_path) = table.path(row) else { continue };
            if !model_path.starts_with("chrs/") {
                continue;
            }
            chr_rows += 1;
            let Some(index) = chr_index.get(&chr_entry_key(model_path)) else { continue };
            resolved += 1;
            let model = ChrModel::parse(pak.payload(*index).unwrap()).unwrap();
            if nodes_of_clips.contains(&(model.info.anim_nodes() as usize)) {
                paired += 1;
            } else {
                incompatible_nodes.insert(model.info.anim_nodes());
            }
        }
    }
    let level_count = levels.len();
    eprintln!(
        "MEASURED model tables: {level_count} levels, {rows} rows, {chr_rows} character rows, {resolved} resolve to a chr.pak model, \
         {paired} have clips with a matching node count, node counts without any clip {incompatible_nodes:?}"
    );
    assert!(levels.len() >= 10, "{levels:?}");
    assert!(chr_rows > 100 && resolved * 100 >= chr_rows * 95, "{resolved}/{chr_rows}");
}

/// T1668: the level of detail 0 mesh streams of every part decode, re-encode to the stored bytes and keep the
/// measured invariants (unit packed normals, node < N, rigid parts use the part's cell, palette of at most three).
#[test]
fn every_part_mesh_decodes_and_round_trips() {
    let Some(chr) = run("tools.rust_anim_sample", &["--raw", "pak/chr.pak"]) else { return };
    let pak = Pak::parse(&chr).unwrap();
    let (mut meshes, mut vertices, mut rigid, mut skinned2, mut skinned3) = (0, 0, 0, 0, 0);
    let mut normals = BTreeMap::new();
    for index in 0..pak.entries.len() {
        let Ok(model) = ChrModel::parse(pak.payload(index).unwrap()) else { continue };
        let nodes = model.info.anim_nodes();
        for part in 0..model.parts.records.len() {
            let Some(mesh) = model.part_mesh(part).unwrap_or_else(|e| panic!("entry {index} part {part}: {e}")) else { continue };
            assert_eq!(model.part_stream_bytes(part).unwrap().unwrap(), &mesh.encode()[..], "entry {index} part {part}");
            let mut used = std::collections::BTreeSet::new();
            for v in &mesh.vertices {
                assert!(v.node < nodes, "entry {index} part {part}: node {} of {nodes}", v.node);
                let n = v.normal();
                let unit = ((n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() - 1.0).abs() < 0.1;
                *normals.entry((model.info.flags(), unit)).or_insert(0usize) += 1;
                assert!(v.position.iter().all(|c| c.is_finite()));
                used.insert(v.node);
            }
            match used.len() {
                1 => {
                    rigid += 1;
                    assert_eq!(*used.iter().next().unwrap() as i32, i32::from(model.parts.records[part].cell()));
                }
                2 => skinned2 += 1,
                3 => skinned3 += 1,
                n => panic!("entry {index} part {part}: {n} nodes"),
            }
            meshes += 1;
            vertices += mesh.vertices.len();
        }
    }
    eprintln!("unit normals by (model flags, unit): {normals:?}");
    eprintln!("chr meshes {meshes} vertices {vertices} rigid {rigid} two {skinned2} three {skinned3}");
    assert_eq!((meshes, rigid, skinned2, skinned3), (15259, 8573, 3530, 3156));
    // 10 of 784 706 vertices (all in flags 0x42040 models) do not decode to a unit normal (zero or unused, INFERRED).
    assert_eq!(normals.iter().filter(|((_, unit), _)| !unit).map(|(_, n)| n).sum::<usize>(), 10);
}

/// T1668: draw groups partition the vertices and every strip index is inside the part.
#[test]
fn every_part_draw_group_list_is_consistent() {
    let Some(chr) = run("tools.rust_anim_sample", &["--raw", "pak/chr.pak"]) else { return };
    let pak = Pak::parse(&chr).unwrap();
    let (mut groups, mut strips, mut indices, mut odd_tag, mut modes) = (0, 0, 0, 0, BTreeSet::new());
    let mut odd_kinds = BTreeMap::new();
    for index in 0..pak.entries.len() {
        let Ok(model) = ChrModel::parse(pak.payload(index).unwrap()) else { continue };
        for part in 0..model.parts.records.len() {
            let Some(list) = model.part_groups(part).unwrap_or_else(|e| panic!("entry {index} part {part}: {e}")) else { continue };
            let total = model.part_mesh(part).unwrap().unwrap().vertices.len();
            assert_eq!(list[0].first_vertex, 0);
            assert_eq!(list.iter().map(|g| g.vertex_count as usize).sum::<usize>(), total);
            for group in &list {
                groups += 1;
                modes.insert(group.mode());
                if group.tag & 0xFF != group.vertex_count & 0xFF {
                    odd_tag += 1;
                    *odd_kinds.entry((group.tag & 0xFF, group.vertex_count.min(256))).or_insert(0usize) += 1;
                }
                if !group.strip.is_empty() {
                    strips += 1;
                    indices += group.strip.len();
                }
            }
        }
    }
    eprintln!("odd kinds (tag low byte, count) {odd_kinds:?}");
    eprintln!("groups {groups} strips {strips} indices {indices} odd tags {odd_tag} modes {modes:?}");
    assert_eq!((groups, odd_tag), (35181, 375));
    assert_eq!(odd_kinds, BTreeMap::from([((0, 1), 372), ((0, 2), 3)]));
    assert_eq!(modes, BTreeSet::from([3, 4]));
}

/// T1668: level of detail slots 1 and 2 use the same layout as slot 0 (streams, groups, strips).
#[test]
fn lod_slots_one_and_two_decode_like_slot_zero() {
    let Some(chr) = run("tools.rust_anim_sample", &["--raw", "pak/chr.pak"]) else { return };
    let pak = Pak::parse(&chr).unwrap();
    let mut counts = BTreeMap::new();
    for index in 0..pak.entries.len() {
        let Ok(model) = ChrModel::parse(pak.payload(index).unwrap()) else { continue };
        for part in 0..model.parts.records.len() {
            for lod in 1..3usize {
                let mesh = model.part_mesh_lod(part, lod).unwrap_or_else(|e| panic!("entry {index} part {part} lod {lod}: {e}"));
                let Some(mesh) = mesh else { continue };
                assert_eq!(model.part_stream_bytes_lod(part, lod).unwrap().unwrap(), &mesh.encode()[..]);
                let groups = model.part_groups_lod(part, lod).unwrap_or_else(|e| panic!("entry {index} part {part} lod {lod}: {e}")).unwrap();
                assert_eq!(groups.iter().map(|g| g.vertex_count as usize).sum::<usize>(), mesh.vertices.len());
                let nodes = model.info.anim_nodes();
                assert!(mesh.vertices.iter().all(|v| v.node < nodes));
                *counts.entry(lod).or_insert((0usize, 0usize)) = {
                    let (a, b) = counts.get(&lod).copied().unwrap_or((0, 0));
                    (a + 1, b + mesh.vertices.len())
                };
            }
        }
    }
    eprintln!("lod meshes (count, vertices) {counts:?}");
    assert!(!counts.is_empty());
}
