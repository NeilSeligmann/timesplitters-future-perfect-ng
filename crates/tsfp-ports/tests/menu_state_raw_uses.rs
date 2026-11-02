// SPDX-License-Identifier: GPL-3.0-or-later
//! T2060 drift guard: raw `0x7ba16c` / `0x7ba178` address uses outside `menu_state.rs`.
//! Hand-written sources must use `menu_state::{CLICK_SOUND_ENABLED, CLICK_SOUND_REQUEST}` (zero raw uses).
//! Generated c2r modules use the same live typed view. Names and documentation may still
//! mention the addresses, but no literal memory operation may bypass the view.
use std::fs;
use std::path::Path;

// Regeneration removed the last name/doc mentions (T2183 follow-up), so no line mentions the addresses.
const GENERATED_C2R_RAW_LINES: usize = 0;

fn walk(dir: &Path, out: &mut Vec<(bool, usize)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().map_or(false, |e| e == "rs") {
            if path.file_name().unwrap() == "menu_state.rs" {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap_or_default();
            let n = text
                .lines()
                .filter(|l| {
                    let l = l.to_ascii_lowercase();
                    l.contains("7ba16c") || l.contains("7ba178")
                })
                .count();
            let generated = path.components().any(|c| c.as_os_str() == "c2r");
            out.push((generated, n));
        }
    }
}

#[test]
fn raw_menu_global_uses_are_pinned() {
    let mut found = Vec::new();
    walk(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut found,
    );
    assert!(!found.is_empty());
    let hand: usize = found.iter().filter(|(g, _)| !g).map(|(_, n)| n).sum();
    let generated: usize = found.iter().filter(|(g, _)| *g).map(|(_, n)| n).sum();
    assert_eq!(
        hand, 0,
        "hand-written sources must use the menu_state constants"
    );
    assert!(
        found.iter().any(|(g, _)| *g),
        "no generated c2r sources were scanned, the walk is broken"
    );
    assert_eq!(generated, GENERATED_C2R_RAW_LINES);
}

#[test]
fn all_port_and_scene_menu_cell_operations_use_the_typed_view() {
    fn check(path: &Path) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = fs::read_to_string(&path).unwrap();
                // Strip numeric separators and case to catch alternate literal spellings.
                for line in text.lines() {
                    let line = line.to_ascii_lowercase().replace('_', "");
                    let operation = [
                        "read8(", "read16(", "read32(", "write8(", "write16(", "write32(",
                    ];
                    for op in operation {
                        for addr in ["0x7ba16c", "0x007ba16c", "0x7ba178", "0x007ba178"] {
                            assert!(
                                !line.contains(&format!("{op}{addr}")),
                                "raw menu cell operation in {}",
                                path.display()
                            );
                        }
                    }
                }
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    check(&root.join("src"));
    // The render crate's generated loader uses the same binding in Cargo and the lane.
    let scene = root.join("../tsfp-engine-render/src/scene_batch.rs");
    let text = fs::read_to_string(scene).unwrap();
    assert!(!text.contains("m.write32(0x7ba16c"));
    assert!(!text.contains("m.write32(0x7ba178"));
}
