//! Replay original custom editor working maps from private state-trace snapshots.
use std::{env, fs};
use tsfp_format_profile::mapping::{MappingEditor, MappingTables};
fn words<const N: usize>(bytes: &[u8], offset: usize) -> [u32; N] {
    core::array::from_fn(|i| {
        u32::from_le_bytes(
            bytes[offset + i * 4..offset + i * 4 + 4]
                .try_into()
                .unwrap(),
        )
    })
}
fn main() -> Result<(), String> {
    let dir = env::args()
        .nth(1)
        .ok_or("usage: settings_mapping_replay PRIVATE_CASE_DIR")?;
    let mut count = 0;
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if !path.to_string_lossy().ends_with(".mapping.bin") {
            continue;
        }
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        if data.len() != 576 {
            return Err("mapping case length".into());
        }
        let bank = words::<1>(&data, 0)[0] as usize;
        let axes = words::<24>(&data, 136);
        let pairs = words::<42>(&data, 232);
        let tables = MappingTables {
            digital: words(&data, 80),
            axes: core::array::from_fn(|i| axes[i * 3..i * 3 + 3].try_into().unwrap()),
            pairs: core::array::from_fn(|i| pairs[i * 3..i * 3 + 3].try_into().unwrap()),
        };
        let editor = MappingEditor {
            bank,
            selections: words(&data, 4),
        };
        let got = editor
            .reconstruct(&tables, words(&data, 400))
            .map_err(String::from)?;
        let expected = words::<22>(&data, 488);
        if got != expected {
            return Err(format!("{}: working map differs", path.display()));
        }
        count += 1;
        println!("{}: original working map EQUAL (22 words)", path.display());
    }
    if count == 0 {
        return Err("no mapping cases".into());
    }
    Ok(())
}
