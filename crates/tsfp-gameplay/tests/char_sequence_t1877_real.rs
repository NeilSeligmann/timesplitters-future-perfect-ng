//! Private real-data checks of the loader's sequence relocation (T1877).
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use tsfp_gameplay::{char_sequence, story_events::setu::setu_blobs_from_pak};
fn word(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}
#[test]
fn sequence_steps_carry_animation_ids() {
    let Some(dir) = std::env::var_os("TSFP_SETU_PAK_DIR").map(PathBuf::from) else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    let (mut entries, mut sequences, mut steps, mut referenced, mut animations, mut playable) =
        (0, 0, 0, 0, 0, 0);
    let mut kinds = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for path in paths {
        for blob in setu_blobs_from_pak(&std::fs::read(path).unwrap()).unwrap() {
            let parsed = char_sequence::parse(&blob).unwrap();
            if !parsed.is_empty() {
                entries += 1;
            }
            sequences += parsed.len();
            for sequence in &parsed {
                for step in sequence {
                    steps += 1;
                    *kinds.entry(step.kind).or_insert(0) += 1;
                    assert!(step.kind <= 18, "unknown sequence command");
                    if step.kind == 8 {
                        animations += 1;
                        playable += usize::from((0x25..0xBFE).contains(&step.value));
                        ids.insert(step.value);
                    }
                }
            }
            let (rec_start, rec_count) = (word(&blob, 0x0C), word(&blob, 0x10));
            for i in 0..rec_count {
                let at = rec_start + i * 0x98;
                if word(&blob, at) & 0x10800000 == 0x10800000 {
                    referenced += 1;
                    assert!(
                        word(&blob, at + 0x68) < parsed.len(),
                        "record sequence index"
                    );
                }
            }
        }
    }
    eprintln!(
        "MEASURED: {entries} SETU entries, {sequences} sequences, {steps} steps, kinds {kinds:?}; {animations} animation commands, {playable} playable, {} distinct ids; {referenced} activate-now references",
        ids.len()
    );
    assert_eq!(
        (entries, sequences, steps, referenced),
        (13, 384, 1971, 107)
    );
    assert_eq!((animations, playable, ids.len()), (59, 59, 25));
}
