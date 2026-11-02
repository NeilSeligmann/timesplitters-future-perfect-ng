//! Tier 1 differential of Rust effects descriptor mapping against the actual C-host helper.

use std::{io::Write, process::Command};
use tsfp_audio::effects_metadata::{EffectMap, MAX_MAPS, Metadata};

fn serialize(metadata: &Metadata, bases: [u32; 4]) -> Vec<u8> {
    let mut words = vec![
        metadata.code_offset,
        metadata.code_bytes,
        metadata.state_offset,
        metadata.state_bytes,
        metadata.descriptor_offset,
        metadata.descriptor_bytes,
        metadata.iv_offset,
        metadata.iv_bytes,
        metadata.map_count,
        metadata.workspace_bytes,
    ];
    for map in metadata.maps {
        words.extend([
            map.code_offset,
            map.code_bytes,
            map.state_offset,
            map.state_bytes,
            map.y_offset,
            map.y_bytes,
            map.workspace_offset,
            map.workspace_bytes,
        ]);
    }
    words.extend(bases);
    words.into_iter().flat_map(u32::to_ne_bytes).collect()
}

fn metadata_for(count: usize, mut seed: u32) -> Metadata {
    let mut metadata = Metadata {
        code_offset: 0x818,
        code_bytes: 0x1000,
        state_offset: 0x1818,
        state_bytes: 0x2000,
        map_count: count as u32,
        workspace_bytes: 0x4000,
        ..Metadata::default()
    };
    for (index, map) in metadata.maps.iter_mut().take(count).enumerate() {
        let code_size = 8 + (seed % 16) * 4;
        let state_size = 4 + (seed % 32) * 4;
        let workspace_size = 16 + (seed % 64) * 4;
        *map = EffectMap {
            code_offset: metadata.code_offset + index as u32 * 0x100,
            code_bytes: code_size,
            state_offset: metadata.state_offset + index as u32 * 0x200,
            state_bytes: state_size,
            workspace_offset: index as u32 * 0x200,
            workspace_bytes: workspace_size,
            ..EffectMap::default()
        };
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
    }
    metadata
}

#[test]
fn rust_descriptor_matches_the_actual_c_host_builder_on_fabricated_layouts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join(format!(
        "tmp/effects-descriptor-oracle-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let binary = dir.join("oracle");
    let build = Command::new(std::env::var("CC").unwrap_or_else(|_| "cc".into()))
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2", "-I"])
        .arg(root.join("src/audio"))
        .arg(root.join("crates/tsfp-audio-conformance/oracle/dsound_effects_descriptor.c"))
        .arg(root.join("tests/c/dsound_effects_descriptor_runner.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let mut seed = 0x2046_88b1u32;
    for case in 0..120 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let count = 1 + (case % MAX_MAPS);
        let metadata = metadata_for(count, seed);
        let bases = [0x20_000 + (seed & 0xfff), 0x30_000, 0x50_000, 0x70_000];
        let mut child = Command::new(&binary)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&serialize(&metadata, bases))
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "C oracle failed on case {case}");
        assert_eq!(
            output.stdout.len(),
            1 + 296,
            "unexpected oracle output on case {case}"
        );
        assert_eq!(output.stdout[0], 1, "C refused valid case {case}");
        let actual: [u8; 296] = output.stdout[1..].try_into().unwrap();
        let expected = tsfp_audio::effects_binding::build_descriptor(
            &metadata, bases[0], bases[1], bases[2], bases[3],
        )
        .expect("generated mapping should be valid");
        assert_eq!(actual, expected, "descriptor mismatch on case {case}");
    }
    let _ = std::fs::remove_dir_all(dir);
}
