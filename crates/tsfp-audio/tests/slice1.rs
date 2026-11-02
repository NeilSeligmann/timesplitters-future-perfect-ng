//! FABRICATED synthetic fixtures only (no retail data).
use tsfp_audio::{adpcm, contract::*, gain, mixer::Mixer, recording::*};

fn block(pred: i16, idx: i16, nibbles: u8) -> Vec<u8> {
    let mut b = pred.to_le_bytes().to_vec();
    b.extend(idx.to_le_bytes());
    b.extend(std::iter::repeat(nibbles).take(32));
    b
}

#[test]
fn gain_matches_oracle_anchors() {
    assert_eq!(gain::volume_to_gain_q16(0), Some(65536));
    assert_eq!(gain::volume_to_gain_q16(-10000), Some(0));
    assert_eq!(gain::volume_to_gain_q16(1), None);
    assert_eq!(gain::volume_to_gain_q16(-2000), Some(6554)); // 0.1 in Q16
    assert_eq!(
        gain::volume_to_gain_q16(-6000).map(|g| g > 0 && g < 100),
        Some(true)
    );
}

#[test]
fn adpcm_shape_and_refusals() {
    let pcm = adpcm::decode(&block(100, 0, 0), 1).unwrap();
    assert_eq!(pcm.len(), 64);
    assert_eq!(pcm[0], 100);
    assert!(pcm[1] >= 100); // nibble 0 adds step>>3 = 0, predictor holds
    assert!(adpcm::decode(&block(0, 89, 0), 1).is_none());
    assert!(adpcm::decode(&[0; 35], 1).is_none());
    assert!(adpcm::decode(&block(0, 0, 0), 2).is_none());
}

#[test]
fn mixer_renders_and_replays_recorded_stream() {
    let tps = 733_333_333u64;
    let mut rec = Recording::new(Mixer::new(48000, tps));
    let data = block(1000, 0, 0);
    assert!(rec.apply(&Command::SubmitAdpcm {
        key: 7,
        ticks: 10,
        source_hz: 48000,
        mono: true,
        data: data.clone()
    }));
    assert!(!rec.apply(&Command::SetStreamVolume {
        key: 7,
        millibels: 5
    }));
    let one = rec.render_until(10 + tps / 1000 * 2, 4096);
    assert!(!one.is_empty() && one[0] == 1000 && one[1] == 1000);
    assert!(one[2 * 63] != 0);
    // replay the captured commands on a fresh mixer: identical PCM
    let mut again = Mixer::new(48000, tps);
    for c in rec.commands() {
        again.apply(&c);
    }
    assert_eq!(again.render_until(10 + tps / 1000 * 2, 4096), one);
}

#[test]
fn volume_attenuates_and_silent_host_records() {
    let tps = 1_000_000u64;
    let mut m = Mixer::new(1000, tps);
    m.apply(&Command::SubmitPcm16Stereo {
        key: 1,
        ticks: 0,
        source_hz: 1000,
        pcm: vec![20000; 20],
    });
    m.apply(&Command::SetStreamVolume {
        key: 1,
        millibels: -2000,
    });
    let out = m.render_until(5000, 16);
    assert_eq!(out[0], 2000);
    let mut rec = Recording::new(Silent);
    rec.apply(&Command::ResetBuffers);
    assert_eq!(rec.log.len(), 1);
    let mut sink = NullSink::default();
    sink.write(&out);
    assert_eq!(sink.frames as usize, out.len() / 2);
}

#[test]
fn replay_parser_on_synthetic_log() {
    // FABRICATED: one stereo PCM16 packet (4 frames of 1000), then a render; hash computed by the same fnv1a.
    let hex: String = (0..8).map(|_| "e803").collect();
    let mut m = Mixer::new(1000, 1_000_000);
    m.apply(&Command::SubmitPcm16Stereo {
        key: 1,
        ticks: 0,
        source_hz: 1000,
        pcm: vec![1000; 8],
    });
    let out = m.render_until(4000, 16);
    let h = tsfp_audio::replay::fnv1a(&out);
    let log = format!(
        "START 1000 1000000\nSP 1 0 1000 1 {hex}\nR 4000 16 {} {h:016x}\n",
        out.len() / 2
    );
    let rep = tsfp_audio::replay::replay(&log);
    assert_eq!(
        (rep.commands, rep.renders, rep.render_equal, rep.render_diff),
        (1, 1, 1, 0)
    );
    assert!(rep.verdict_mismatch.is_empty() && rep.nonsilent_equal == 1);
}

/// Conformance against a captured C host log (private: contains retail audio, never committed).
/// Run with TSFP_AUDIO_CMD_LOG_FIXTURE=<path>; skipped otherwise.
#[test]
fn replay_matches_c_host_log_if_present() {
    let Ok(path) = std::env::var("TSFP_AUDIO_CMD_LOG_FIXTURE") else {
        return;
    };
    let rep = tsfp_audio::replay::replay(&std::fs::read_to_string(path).unwrap());
    assert!(rep.renders > 0 && rep.nonsilent_equal > 0, "{rep:?}");
    assert_eq!(rep.render_diff, 0, "{rep:?}");
    assert!(rep.verdict_mismatch.is_empty(), "{rep:?}");
}

#[test]
fn wav_sink_ring_and_pump() {
    use tsfp_audio::sink::{Ring, WavSink, pump};
    let mut m = Mixer::new(1000, 1_000_000);
    m.apply(&Command::SubmitPcm16Stereo {
        key: 1,
        ticks: 0,
        source_hz: 1000,
        pcm: vec![500; 40],
    });
    let mut wav = WavSink::new(std::io::Cursor::new(Vec::new()), 1000, 2).unwrap();
    let frames = pump(&mut m, &mut wav, 30_000, 8);
    assert_eq!(frames, 30);
    assert_eq!(wav.frames(), 30);
    let bytes = wav.finish().unwrap().into_inner();
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(
        u32::from_le_bytes(bytes[40..44].try_into().unwrap()),
        30 * 4
    );
    assert_eq!(bytes.len(), 44 + 30 * 4);
    let mut ring = Ring::new(10);
    ring.write(&[1; 14]);
    assert_eq!((ring.fill(), ring.overruns), (10, 1));
    let mut out = [9i16; 12];
    assert_eq!(ring.pull(&mut out), 10);
    assert_eq!((out[10], out[11], ring.underruns), (0, 0, 1));
    ring.write(&[2; 6]);
    assert_eq!(ring.clear(), 6);
    assert_eq!((ring.fill(), ring.underruns), (0, 1));
}

/// Adapter-mode conformance (private log): the DsAdapter driven by reconstructed guest calls must emit the
/// same mixer commands, completion words and PCM hashes as the C runtime. Skipped without the fixture.
#[test]
fn adapter_replay_looping_play_uses_play_flags() {
    let data = "00".repeat(36);
    let log = format!(
        "START 48000 1000000\nPB 1 1 0 48000 0 1 0 64 -10000 1 {data}\nCBP 1 1 0 1\nCBG 1 1 1 5\n"
    );
    let rep = tsfp_audio::adapter_replay::replay_adapter(&log);
    assert_eq!(rep.calls, 2, "{rep:?}");
    assert!(rep.mismatches.is_empty(), "{rep:?}");
}

#[test]
fn adapter_matches_c_host_log_if_present() {
    let Ok(path) = std::env::var("TSFP_AUDIO_CMD_LOG_FIXTURE") else {
        return;
    };
    let rep = tsfp_audio::adapter_replay::replay_adapter(&std::fs::read_to_string(path).unwrap());
    assert!(rep.calls > 0 && rep.renders > 0, "{rep:?}");
    assert!(rep.mismatches.is_empty() && rep.render_diff == 0, "{rep:?}");
}
