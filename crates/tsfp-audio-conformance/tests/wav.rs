//! Exercise the evidence producer, including its refusal to reuse an old receipt directory.
use std::{fs, process::Command};

struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn paired_wav_evidence_is_exact_and_cannot_reuse_receipt() {
    let path = std::env::temp_dir().join(format!("tsfp-pcm-evidence-{}", std::process::id()));
    // Exclusive creation: never remove or reuse another test's files.
    fs::create_dir(&path).unwrap();
    let scratch = Scratch(path);
    let output = scratch.0.join("result");
    let binary = env!("CARGO_BIN_EXE_tsfp-audio-conformance");
    let result = Command::new(binary).arg(&output).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let receipt = fs::read_to_string(output.join("receipt.json")).unwrap();
    assert!(receipt.contains("\"verdict\":\"EQUAL\""));
    assert!(receipt.contains("\"tolerance\":0"));
    let mut count = 0;
    for entry in fs::read_dir(&output).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if let Some(stem) = name.strip_suffix("-c.wav") {
            let c = fs::read(&path).unwrap();
            let rust = fs::read(output.join(format!("{stem}-rust.wav"))).unwrap();
            assert_eq!(&c[..4], b"RIFF");
            assert!(c.len() > 44);
            assert_eq!(
                u32::from_le_bytes(c[40..44].try_into().unwrap()) as usize,
                c.len() - 44
            );
            assert_eq!(c, rust, "{stem}");
            count += 1;
        }
    }
    assert_eq!(
        count,
        tsfp_audio_conformance::scenarios::fixed_inputs().len()
    );
    assert!(
        !Command::new(binary)
            .arg(&output)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(output.join("receipt.json")).unwrap(),
        receipt
    );
}
