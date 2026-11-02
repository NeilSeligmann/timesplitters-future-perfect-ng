//! Tier 1 differential of the Rust fixed-image guard against the actual C implementation.

use std::{fs, process::Command};
use tsfp_audio::effects_identity::{self, Fingerprints, IMAGE_BYTES};

fn synthetic_known_layout() -> Vec<u8> {
    let mut image = vec![0; IMAGE_BYTES];
    for (offset, value) in [
        (0x800usize, 0u32),
        (0x804, 2020),
        (0x808, 10152),
        (0x80C, 2022),
        (0x810, 3),
        (0x814, 0),
        (0x4740, 9),
        (0x4744, 393_216),
    ] {
        image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for index in 0..9 {
        let offset = 0x4748 + index * 32;
        for (word, value) in [0x818u32, 0, 0x27A8, 0, 0, 0, 0xC000, 0]
            .into_iter()
            .enumerate()
        {
            image[offset + word * 4..offset + word * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    image
}

fn expected(image: &[u8]) -> Fingerprints {
    effects_identity::fingerprints(image)
}

#[test]
fn rust_image_guard_matches_actual_c_on_synthetic_valid_and_refused_images() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join(format!(
        "tmp/effects-identity-oracle-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let binary = dir.join("oracle");
    let build = Command::new(std::env::var("CC").unwrap_or_else(|_| "cc".into()))
        .args([
            "-DTSFP_EFFECTS_METADATA_TEST",
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-O2",
            "-I",
        ])
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

    let baseline = synthetic_known_layout();
    let baseline_fingerprint = expected(&baseline);
    let mut cases = vec![(baseline.clone(), baseline_fingerprint)];

    let mut checksum_changed = baseline.clone();
    checksum_changed[0x100] ^= 0x80;
    cases.push((checksum_changed, baseline_fingerprint));

    let mut bad_header = baseline.clone();
    bad_header[0x800..0x804].copy_from_slice(&1u32.to_le_bytes());
    cases.push((bad_header.clone(), expected(&bad_header)));

    let mut bad_map_count = baseline.clone();
    bad_map_count[0x4740..0x4744].copy_from_slice(&8u32.to_le_bytes());
    cases.push((bad_map_count.clone(), expected(&bad_map_count)));

    let mut bad_workspace = baseline.clone();
    bad_workspace[0x4744..0x4748].copy_from_slice(&393_215u32.to_le_bytes());
    cases.push((bad_workspace.clone(), expected(&bad_workspace)));
    cases.push((baseline[..IMAGE_BYTES - 1].to_vec(), baseline_fingerprint));

    let input_path = dir.join("image.bin");
    for (case, (image, supplied_fingerprint)) in cases.into_iter().enumerate() {
        fs::write(&input_path, &image).unwrap();
        let actual = Command::new(&binary)
            .arg("identity")
            .arg(&input_path)
            .arg(format!("{:016x}", supplied_fingerprint.fnv1a64))
            .arg(format!("{:08x}", supplied_fingerprint.crc32))
            .output()
            .unwrap();
        assert!(actual.status.success(), "C oracle failed on case {case}");
        let actual = String::from_utf8(actual.stdout).unwrap();
        let expected =
            if effects_identity::validate_image_with_fingerprints(&image, supplied_fingerprint)
                .is_some()
            {
                "ACCEPT\n"
            } else {
                "REFUSED\n"
            };
        assert_eq!(actual, expected, "identity mismatch on case {case}");
    }
    let _ = fs::remove_dir_all(dir);
}
