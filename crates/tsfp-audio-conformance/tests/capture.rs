use std::{
    fs,
    io::{self, Cursor, Read},
    process::Command,
};
use tsfp_audio::{
    contract::{Command as AudioCommand, SoundHost},
    replay::fnv1a,
};
use tsfp_audio_conformance::{
    capture::{MAX_LINE_BYTES, compare_log},
    oracle::COracle,
};

fn generated_log() -> String {
    let pcm: Vec<i16> = (0..128)
        .flat_map(|i| [1000 + i * 3, -1500 + i * 2])
        .collect();
    let data: String = pcm
        .iter()
        .flat_map(|s| s.to_le_bytes())
        .map(|b| format!("{b:02x}"))
        .collect();
    let mut c = COracle::new(48000, 48000);
    assert!(c.apply(&AudioCommand::SubmitPcm16Stereo {
        key: 7,
        ticks: 0,
        source_hz: 48000,
        pcm
    }));
    let mut log = format!("CCFG 1\nSTART 48000 48000\nSP 7 0 48000 1 {data}\n");
    for (ticks, cap) in [(64, 32), (64, 32), (128, 64)] {
        let pcm = c.render_until(ticks, cap);
        assert!(pcm.iter().any(|&x| x != 0));
        log.push_str(&format!(
            "R {ticks} {cap} {} {:016x}\n",
            pcm.len() / 2,
            fnv1a(&pcm)
        ));
    }
    log
}

#[test]
fn streams_capture_with_capacity_drains_and_explicit_completion_exclusion() {
    let log = generated_log();
    let mut samples = 0;
    let report = compare_log(Cursor::new(log.as_bytes()), |rate, c, rust| {
        assert_eq!(rate, 48000);
        assert_eq!(c, rust);
        samples += c.len();
        Ok(())
    })
    .unwrap();
    assert_eq!(report.commands, 1);
    assert_eq!(report.renders, 3);
    assert_eq!(report.completion_observations, 1);
    assert_eq!(report.samples, samples);
    assert_eq!(report.input_bytes, log.len() as u64);
    assert!(report.nonsilent_samples > 0);
}

#[test]
fn captured_verdict_hash_truncation_and_unknown_mutations_fail_closed() {
    let log = generated_log();
    let last_hash = log.split_whitespace().last().unwrap();
    let mutations = [
        log.replace("SP 7 0 48000 1", "SP 7 0 48000 0"),
        log.replace(last_hash, "0000000000000000"),
        log.trim_end().to_owned(),
        format!("{log}FUTURE_COMMAND 1\n"),
        log.replace("START 48000 48000\n", ""),
        format!("{log}START 48000 48000\n"),
        log.replace("R 64 32 32", "R 64 32 31"),
        log.replace("R 64 32", "R 64 1048577"),
    ];
    for (index, mutant) in mutations.iter().enumerate() {
        assert_ne!(*mutant, log, "inactive mutation {index}");
        assert!(
            compare_log(Cursor::new(mutant), |_, _, _| Ok(())).is_err(),
            "survived mutation {index}"
        );
    }
}

#[test]
fn no_silent_vacuity_or_lost_io_errors() {
    for log in ["", "START 48000 48000\n"] {
        assert!(compare_log(Cursor::new(log), |_, _, _| Ok(())).is_err());
    }
    let error = compare_log(Cursor::new(generated_log()), |_, _, _| {
        Err(io::Error::other("injected writer failure"))
    })
    .unwrap_err();
    assert!(error.contains("injected writer failure"));
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("injected read failure"))
        }
    }
    assert!(
        compare_log(io::BufReader::new(Broken), |_, _, _| Ok(()))
            .unwrap_err()
            .contains("injected read failure")
    );
    // Infinite input must terminate after one bounded read, not allocate the file.
    let huge = io::BufReader::new(io::repeat(b'0'));
    assert!(
        compare_log(huge, |_, _, _| Ok(()))
            .unwrap_err()
            .contains("record exceeds bound")
    );
    assert_eq!(MAX_LINE_BYTES, 4 * 1024 * 1024);
}

#[test]
fn silent_started_mixer_is_rejected_after_pcm_comparison() {
    // START allocates a mixer but does not start its timeline. Submit actual silent
    // PCM first; derive the capture row from C rather than inventing a frame count.
    let mut c = COracle::new(48000, 48000);
    assert!(c.apply(&AudioCommand::SubmitPcm16Stereo {
        key: 7,
        ticks: 0,
        source_hz: 48000,
        pcm: vec![0; 32],
    }));
    let pcm = c.render_until(16, 16);
    assert!(!pcm.is_empty());
    assert!(pcm.iter().all(|&sample| sample == 0));
    let silent = format!(
        "START 48000 48000\nSP 7 0 48000 1 {}\nR 16 16 {} {:016x}\n",
        "00".repeat(64),
        pcm.len() / 2,
        fnv1a(&pcm),
    );
    let mut compared = 0;
    let error = compare_log(Cursor::new(silent), |_, a, b| {
        assert_eq!(a, pcm);
        assert_eq!(a, b);
        compared += a.len();
        Ok(())
    })
    .unwrap_err();
    assert_eq!(compared, pcm.len(), "silence must reach the PCM observer");
    assert_eq!(error, "vacuous command/PCM capture");
}

#[test]
fn captured_cli_writes_exact_wavs_and_no_receipt_for_failed_input() {
    let root = std::env::temp_dir().join(format!("tsfp-capture-evidence-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let scratch = Scratch(root);
    let input = scratch.0.join("generated.log");
    let out = scratch.0.join("good");
    let log = generated_log();
    fs::write(&input, &log).unwrap();
    let bin = env!("CARGO_BIN_EXE_tsfp-audio-conformance");
    let result = Command::new(bin)
        .arg("--capture")
        .arg(&input)
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let c = fs::read(out.join("capture-c.wav")).unwrap();
    assert_eq!(c, fs::read(out.join("capture-rust.wav")).unwrap());
    assert_eq!(
        u32::from_le_bytes(c[40..44].try_into().unwrap()) as usize,
        c.len() - 44
    );
    let receipt = fs::read(out.join("receipt.json")).unwrap();
    assert!(
        !Command::new(bin)
            .arg("--capture")
            .arg(&input)
            .arg(&out)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(receipt, fs::read(out.join("receipt.json")).unwrap());
    fs::write(&input, format!("{log}UNKNOWN\n")).unwrap();
    let bad = scratch.0.join("bad");
    assert!(
        !Command::new(bin)
            .arg("--capture")
            .arg(&input)
            .arg(&bad)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(!bad.join("receipt.json").exists());
}
