use std::io::Cursor;
use tsfp_audio::{
    contract::{Command, SoundHost},
    mixer::Mixer,
};
use tsfp_audio_conformance::capture::compare_log;
use tsfp_audio_observe::Recorder;

fn recorded() -> String {
    let mut recorder = Recorder::new(Vec::new(), 48000, 48000).unwrap();
    let mut host = Mixer::new(48000, 48000);
    let command = Command::SubmitPcm16Stereo {
        key: 7,
        ticks: 0,
        source_hz: 48000,
        pcm: (0..256).map(|i| (i * 37 - 2400) as i16).collect(),
    };
    let accepted = host.apply(&command);
    assert!(accepted);
    recorder.command(&command, accepted, None).unwrap();
    for (ticks, capacity) in [(0, 0), (64, 32), (64, 32), (128, 64)] {
        let pcm = host.render_until(ticks, capacity);
        recorder.render(ticks, capacity, &pcm).unwrap();
    }
    String::from_utf8(recorder.finish().unwrap()).unwrap()
}

#[test]
fn rust_producer_replays_sample_exact_against_c_and_requires_a_complete_seal() {
    let log = recorded();
    let report = compare_log(Cursor::new(&log), |_, a, b| {
        assert_eq!(a, b);
        Ok(())
    })
    .unwrap();
    assert!(report.sealed);
    assert_eq!(report.renders, 4);
    assert_eq!(report.samples, 256);
    assert!(report.nonsilent_samples > 0);
    let end = log.rfind("END ").unwrap();
    let mut lines: Vec<_> = log.lines().collect();
    lines.swap(4, 5);
    let mutations = [
        log[..end].to_owned(), // formerly passed: loss exactly on a record boundary
        log.replace("CAPTURE 1", "CAPTURE 2"),
        log.replace("END 7 ", "END 6 "),
        format!("{log}RST\n"),
        log.replace("SP 7 0 48000 1", "SP 7 0 48000 0"),
        log.replace("R 64 32 32", "R 64 32 31"),
        format!("{}\n", lines.join("\n")),
    ];
    for (i, mutant) in mutations.iter().enumerate() {
        assert_ne!(*mutant, log, "inactive mutation {i}");
        assert!(
            compare_log(Cursor::new(mutant), |_, _, _| Ok(())).is_err(),
            "survived {i}"
        );
    }
    let seal_hash = log.lines().last().unwrap().split(' ').last().unwrap();
    let corrupt_hash = log.replace(seal_hash, "0000000000000000");
    assert_ne!(corrupt_hash, log);
    assert!(compare_log(Cursor::new(corrupt_hash), |_, _, _| Ok(())).is_err());
}

#[test]
fn original_adpcm_payload_is_required_and_independently_decoded() {
    let mut encoded = vec![0x11; 36];
    encoded[..4].copy_from_slice(&[0xe8, 3, 0, 0]);
    let pcm = tsfp_audio::adpcm::decode(&encoded, 1).unwrap();
    let command = Command::PlayBuffer {
        key: 9,
        serial: 17,
        ticks: 0,
        frequency: 48000,
        pcm,
        paused: false,
        looping: false,
        loop_start: 0,
        loop_end: 64,
        volume: 0,
    };
    let mut host = Mixer::new(48000, 48000);
    let accepted = host.apply(&command);
    assert!(accepted);
    let mut recorder = Recorder::new(Vec::new(), 48000, 48000).unwrap();
    recorder
        .command(&command, accepted, Some(&encoded))
        .unwrap();
    recorder.render(64, 64, &host.render_until(64, 64)).unwrap();
    let log = recorder.finish().unwrap();
    assert!(
        compare_log(Cursor::new(log), |_, _, _| Ok(()))
            .unwrap()
            .sealed
    );
    for bad in [None, Some(vec![0; 36]), Some(vec![0; 35])] {
        let mut recorder = Recorder::new(Vec::new(), 48000, 48000).unwrap();
        assert!(
            recorder
                .command(&command, accepted, bad.as_deref())
                .is_err()
        );
        assert!(recorder.finish().is_err());
    }
}

#[test]
fn xbox_adpcm_decoder_matches_production_c_oracle() {
    for predictor in [i16::MIN, -1, 0, 1000, i16::MAX] {
        for index in [0, 1, 7, 44, 88] {
            for nibble_byte in [0x00, 0x77, 0x88, 0xff] {
                let mut encoded = vec![nibble_byte; 36];
                encoded[..2].copy_from_slice(&predictor.to_le_bytes());
                encoded[2] = index;
                encoded[3] = 0;

                let rust = tsfp_audio::adpcm::decode(&encoded, 1).unwrap();
                let c = tsfp_audio_conformance::oracle::decode_mono(&encoded).unwrap();
                assert_eq!(
                    rust, c,
                    "predictor={predictor}, index={index}, nibbles={nibble_byte:#04x}"
                );
            }
        }
    }
}
