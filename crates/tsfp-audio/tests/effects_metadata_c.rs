//! Tier 1 differential of the Rust image-layout parser against the actual C validator.

use std::{fs, process::Command};
use tsfp_audio::effects_metadata::{self, Metadata};

fn synthetic() -> Vec<u8> {
    let mut image = vec![0; 0x890];
    image[0x804..0x808].copy_from_slice(&4u32.to_le_bytes());
    image[0x80c..0x810].copy_from_slice(&4u32.to_le_bytes());
    image[0x838..0x83c].copy_from_slice(&2u32.to_le_bytes());
    image[0x83c..0x840].copy_from_slice(&32u32.to_le_bytes());
    let maps: [[u32; 8]; 2] = [
        [0x818, 8, 0x828, 8, 0, 0, 0xc000, 16],
        [0x820, 8, 0x830, 8, 0, 0, 0xc010, 16],
    ];
    for (index, map) in maps.iter().enumerate() {
        for (word, value) in map.iter().enumerate() {
            let offset = 0x840 + index * 32 + word * 4;
            image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    image
}

fn render(metadata: Option<Metadata>) -> String {
    let Some(metadata) = metadata else {
        return "REFUSED\n".into();
    };
    let mut result = format!(
        "VALID {} {} {} {} {} {}\n",
        metadata.map_count,
        metadata.workspace_bytes,
        metadata.code_bytes,
        metadata.state_bytes,
        metadata.descriptor_offset,
        metadata.iv_offset
    );
    for map in &metadata.maps[..metadata.map_count as usize] {
        result.push_str(&format!(
            "{} {} {} {} {} {} {} {}\n",
            map.code_offset,
            map.code_bytes,
            map.state_offset,
            map.state_bytes,
            map.y_offset,
            map.y_bytes,
            map.workspace_offset,
            map.workspace_bytes
        ));
    }
    result
}

#[test]
fn rust_layout_matches_actual_c_on_valid_truncated_and_mutated_images() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join(format!(
        "tmp/effects-metadata-oracle-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let binary = dir.join("oracle");
    let build = Command::new(std::env::var("CC").unwrap_or_else(|_| "cc".into()))
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2", "-I"])
        .arg(root.join("src/audio"))
        .arg("-I")
        .arg(root.join("src/xbox"))
        .arg(root.join("crates/tsfp-audio-conformance/oracle/dsound_effects_metadata.c"))
        .arg(root.join("tests/c/dsound_effects_metadata_runner.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let input_path = dir.join("image.bin");
    let mut cases = vec![synthetic()];
    for length in [0, 0x817, 0x818, 0x838, 0x83f, 0x88f] {
        cases.push(synthetic()[..length].to_vec());
    }
    let mutations = [
        (0x804, u32::MAX),
        (0x80c, u32::MAX),
        (0x838, 0),
        (0x838, 10),
        (0x83c, 0),
        (0x840, 0),
        (0x844, u32::MAX),
        (0x848, 0x818),
        (0x84c, 17),
        (0x850, 1),
        (0x854, 1),
        (0x858, 0xbfff),
        (0x85c, 33),
        (0x860, 0x818),
        (0x868, 0x828),
        (0x878, 0xc000),
        (0x878, u32::MAX),
    ];
    for (offset, value) in mutations {
        let mut image = synthetic();
        image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        cases.push(image);
    }
    let mut seed = 0x2046_7a31u32;
    for _ in 0..96 {
        let mut image = synthetic();
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let word = (seed as usize) % 20;
        let offset = 0x840 + word * 4;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        image[offset..offset + 4].copy_from_slice(&seed.to_le_bytes());
        cases.push(image);
    }

    for (case, image) in cases.into_iter().enumerate() {
        fs::write(&input_path, &image).unwrap();
        let actual = Command::new(&binary)
            .args(["layout"])
            .arg(&input_path)
            .output()
            .unwrap();
        assert!(actual.status.success(), "C oracle failed on case {case}");
        let actual = String::from_utf8(actual.stdout).unwrap();
        let expected = render(effects_metadata::validate_layout(&image));
        assert_eq!(actual, expected, "layout mismatch on case {case}");
    }
    let _ = fs::remove_dir_all(dir);
}
