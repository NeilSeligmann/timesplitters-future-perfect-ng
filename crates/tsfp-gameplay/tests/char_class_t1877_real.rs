//! T1877: census of the class masks of every SETU spawn record on the disc (skips without TSFP_SETU_PAK_DIR).
use std::collections::BTreeMap;
use std::path::PathBuf;
use tsfp_gameplay::char_anim::dispatch_ticks;
use tsfp_gameplay::char_class::{record_mask, spawn_class};
use tsfp_gameplay::story_events::setu::setu_blobs_from_pak;

fn word(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

#[test]
fn every_spawn_record_class_resolves_to_a_tick() {
    let Some(dir) = std::env::var_os("TSFP_SETU_PAK_DIR").map(PathBuf::from) else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let mut census: BTreeMap<(u64, u64), usize> = BTreeMap::new();
    // (class, record word offset) -> (records with a set id, distinct ids)
    let mut ids: BTreeMap<(u64, usize), std::collections::BTreeSet<i16>> = BTreeMap::new();
    let mut set_count: BTreeMap<(u64, usize), usize> = BTreeMap::new();
    let (mut records, mut entries) = (0usize, 0usize);
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    for path in paths {
        for blob in setu_blobs_from_pak(&std::fs::read(&path).unwrap()).unwrap_or_default() {
            let (start, count) = (word(&blob, 0x0C), word(&blob, 0x10));
            if count == 0 {
                continue;
            }
            entries += 1;
            let table = &blob[start..start + count * 0x98];
            for index in 0..count {
                let mask = record_mask(table, index).unwrap();
                let class = spawn_class(mask).initial_mask;
                *census.entry((mask, class)).or_default() += 1;
                for offset in [0x48usize, 0x4C, 0x94] {
                    let at = index * 0x98 + offset;
                    let word = i16::from_le_bytes([table[at], table[at + 1]]);
                    if word != -1 && word != 0 {
                        ids.entry((class, offset)).or_default().insert(word);
                        *set_count.entry((class, offset)).or_default() += 1;
                    }
                }
                records += 1;
            }
        }
    }
    let (mut ticked, mut parked_ticked, mut parked_unknown) = (0usize, 0usize, 0usize);
    for ((mask, initial), n) in &census {
        let class = spawn_class(*mask);
        eprintln!(
            "rec {mask:#x} -> {initial:#x} parked {:#x} ticks {:x?} x{n}",
            class.parked_mask, class.ticks
        );
        ticked += if class.ticks.is_some() { n } else { &0 };
        if class.parked_mask != 0 {
            if dispatch_ticks(true, class.parked_mask).is_some() {
                parked_ticked += n;
            } else {
                parked_unknown += n;
            }
        }
    }
    eprintln!(
        "MEASURED {records} records in {entries} entries, {} distinct masks, {ticked} start in a ticked class, parked {parked_ticked} ticked / {parked_unknown} not a single class bit",
        census.len()
    );
    for ((class, offset), set) in &ids {
        eprintln!(
            "ids class {class:#x} rec+{offset:#x}: {} records, {} distinct",
            set_count[&(*class, *offset)],
            set.len()
        );
    }
    let static_pose = ids.get(&(0x1_0000_0000, 0x94)).map_or(0, |set| set.len());
    assert!(records > 1000 && static_pose > 0);
}
