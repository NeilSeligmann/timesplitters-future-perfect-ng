// SPDX-License-Identifier: GPL-3.0-or-later
//! T2136 slice 2: audio thread tests (queue and back-pressure, mixer-through-thread determinism, sinks). They need no GPU.
//! `TSFP_AUDIO_CMD_LOG_FIXTURE=<log>` adds the test against a real C host command log (private, never committed).
use std::time::{Duration, Instant};

use tsfp_audio::contract::SoundHost;
use tsfp_audio::mixer::Mixer;
use tsfp_audio::replay::{fnv1a, replay};
use tsfp_rt_host::audio::{AudioRuntime, parse_audio_args};

const TPS: u64 = 733_333_333;

fn start(args: &str) -> AudioRuntime {
    let words: Vec<String> = args.split_whitespace().map(str::to_string).collect();
    AudioRuntime::start(parse_audio_args(&words).expect("args")).expect("start")
}

fn hex(pcm: &[i16]) -> String {
    pcm.iter()
        .flat_map(|s| s.to_le_bytes())
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// FABRICATED log: two PCM16 stereo streams (a 440 Hz and a 660 Hz packet train), a cut, and R lines whose hash comes from a
/// direct (same thread) run of the mixer, so the thread output must reproduce it. Returns the log text and the direct PCM hash.
fn fabricated_log(renders: u64) -> (String, u64) {
    let mut log = format!("START 48000 {TPS}\n");
    let mut direct = Mixer::new(48000, TPS);
    let mut running = 1469598103934665603u64;
    let mut pcm_all: Vec<i16> = vec![];
    for step in 0..renders {
        let now = step * TPS / 100; // 10 ms per render
        if step % 5 == 0 {
            for (key, hz) in [(1u32, 440.0f64), (2, 660.0)] {
                let pcm: Vec<i16> = (0..480)
                    .flat_map(|i| {
                        let v = ((i as f64 * hz * std::f64::consts::TAU / 48000.0).sin() * 8000.0)
                            as i16;
                        [v, v]
                    })
                    .collect();
                let line = format!("SP {key} {now} 48000 1 {}", hex(&pcm));
                log.push_str(&line);
                log.push('\n');
                // the verdict flag in the line is the C verdict; take the direct mixer's own
                direct.apply(&tsfp_audio::contract::Command::SubmitPcm16Stereo {
                    key,
                    ticks: now,
                    source_hz: 48000,
                    pcm,
                });
            }
        }
        if step == renders / 2 {
            log.push_str(&format!("CUT 2 {now} 1\n"));
            direct.apply(&tsfp_audio::contract::Command::CutStream { key: 2, ticks: now });
        }
        let out = direct.render_until(now, 4800);
        log.push_str(&format!(
            "R {now} 4800 {} {:x}\n",
            out.len() / 2,
            fnv1a(&out)
        ));
        for s in &out {
            for b in s.to_le_bytes() {
                running = (running ^ u64::from(b)).wrapping_mul(1099511628211);
            }
        }
        pcm_all.extend(out);
    }
    assert!(
        pcm_all.iter().any(|&s| s != 0),
        "fabricated log must produce sound"
    );
    (log, running)
}

#[test]
fn mixer_through_thread_is_deterministic_and_matches_direct_run() {
    let (log, direct_hash) = fabricated_log(200);
    let mut seen = vec![];
    for (chunk, args) in [
        (log.len(), "--batch-bytes 64"),
        (7, "--batch-bytes 64 --queue 1"),
        (4096, "--batch-bytes 100000"),
    ] {
        let rt = start(args);
        for part in log.as_bytes().chunks(chunk) {
            rt.write(part);
        }
        let stats = rt.finish();
        assert_eq!(stats.renders, 200);
        assert_eq!(
            (stats.render_equal, stats.render_diff),
            (200, 0),
            "{stats:?}"
        );
        assert_eq!(stats.verdict_mismatch, 0, "{stats:?}");
        assert!(!stats.gave_up);
        assert_eq!(
            stats.pcm_hash, direct_hash,
            "thread output differs from the direct run"
        );
        seen.push((stats.pcm_hash, stats.frames));
    }
    assert!(
        seen.windows(2).all(|w| w[0] == w[1]),
        "chunking and batching must not change the output: {seen:?}"
    );
}

#[test]
fn matches_the_offline_replay_report() {
    let (log, _) = fabricated_log(120);
    let report = replay(&log);
    let rt = start("");
    rt.write(log.as_bytes());
    let stats = rt.finish();
    assert_eq!(stats.renders as usize, report.renders);
    assert_eq!(stats.render_equal as usize, report.render_equal);
    assert_eq!(stats.commands as usize, report.commands);
}

#[test]
fn wav_sink_holds_exactly_the_rendered_pcm() {
    let (log, _) = fabricated_log(50);
    let path = std::env::temp_dir().join(format!("tsfp-rt-audio-{}.wav", std::process::id()));
    let rt = start(&format!("--sink wav={}", path.display()));
    rt.write(log.as_bytes());
    let stats = rt.finish();
    let bytes = std::fs::read(&path).expect("wav");
    let _ = std::fs::remove_file(&path);
    assert_eq!(&bytes[0..4], b"RIFF");
    let data = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    assert_eq!(
        data as u64,
        stats.frames * 4,
        "header data size patched at finish"
    );
    assert_eq!(bytes.len(), 44 + data);
    let pcm: Vec<i16> = bytes[44..]
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    assert_eq!(fnv1a(&pcm), stats.pcm_hash);
}

#[test]
fn ring_sink_underrun_is_silence_and_counted_never_a_stall() {
    let (log, _) = fabricated_log(10);
    let rt = start("--sink ring=1000000");
    let mut out = vec![7i16; 256];
    let started = Instant::now();
    assert_eq!(rt.pull(&mut out), 0, "nothing mixed yet");
    assert!(out.iter().all(|&s| s == 0), "underrun pads silence");
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "a pull never waits"
    );
    rt.write(log.as_bytes());
    let stats = rt.finish();
    assert_eq!(stats.ring_underruns, 1);
    assert_eq!(stats.ring_overruns, 0);
    let mut out = vec![0i16; 480 * 2];
    assert_eq!(
        rt.pull(&mut out),
        480 * 2,
        "ring holds the mixed audio after the run"
    );
    assert!(out.iter().any(|&s| s != 0));
}

#[test]
fn ring_overrun_drops_excess_and_counts() {
    let (log, _) = fabricated_log(10);
    let rt = start("--sink ring=100");
    rt.write(log.as_bytes());
    let stats = rt.finish();
    assert!(stats.ring_overruns > 0, "{stats:?}");
}

fn comment_lines(rt: &AudioRuntime, count: usize) -> Duration {
    let mut worst = Duration::ZERO;
    for i in 0..count {
        let t = Instant::now();
        rt.send_line(&format!(
            "# a comment line long enough to fill a batch number {i}"
        ));
        worst = worst.max(t.elapsed());
    }
    worst
}

#[test]
fn fast_audio_thread_never_stalls_the_guest() {
    let rt = start("--queue 8 --batch-bytes 4096");
    comment_lines(&rt, 20000);
    let stats = rt.finish();
    assert_eq!(stats.lines, 20000);
    assert!(!stats.gave_up && stats.batches_dropped == 0);
    // a tight producer loop can briefly outrun the thread (MEASURED 16 waits, 4 ms total), it must stay negligible
    assert!(
        stats.stall_ns_total < 100_000_000 && stats.stall_ns_max < 50_000_000,
        "{stats:?}"
    );
}

#[test]
fn stalled_audio_thread_blocks_the_guest_at_most_the_stall_limit_then_audio_is_off() {
    let rt = start("--queue 2 --batch-bytes 64 --stall-ms 300 --thread-delay-ms 400");
    let begin = Instant::now();
    let worst = comment_lines(&rt, 400);
    assert!(
        worst < Duration::from_millis(450),
        "longest guest wait {worst:?}"
    );
    assert!(
        begin.elapsed() < Duration::from_secs(2),
        "after giving up the calls cost nothing: {:?}",
        begin.elapsed()
    );
    let stats = rt.stats();
    assert!(
        stats.gave_up && stats.batches_dropped >= 1 && stats.stalls >= 1,
        "{stats:?}"
    );
    rt.finish();
}

#[test]
fn many_short_waits_trip_the_total_budget() {
    let rt = start(
        "--queue 1 --batch-bytes 64 --stall-ms 5000 --stall-total-ms 400 --thread-delay-ms 50",
    );
    let begin = Instant::now();
    comment_lines(&rt, 400);
    assert!(
        begin.elapsed() < Duration::from_millis(1500),
        "{:?}",
        begin.elapsed()
    );
    assert!(rt.stats().gave_up);
    rt.finish();
}

#[test]
fn slow_thread_with_a_big_limit_loses_nothing() {
    let rt =
        start("--queue 1 --batch-bytes 64 --stall-ms 0 --stall-total-ms 0 --thread-delay-ms 10");
    comment_lines(&rt, 60);
    let stats = rt.finish();
    assert_eq!(stats.lines, 60);
    assert!(!stats.gave_up && stats.stalls > 0, "{stats:?}");
}

#[test]
fn real_c_host_log_through_the_thread_matches_every_c_render_hash() {
    let Ok(path) = std::env::var("TSFP_AUDIO_CMD_LOG_FIXTURE") else {
        eprintln!("skip: TSFP_AUDIO_CMD_LOG_FIXTURE not set");
        return;
    };
    let log = std::fs::read(&path).expect("fixture");
    let rt = start("--queue 64 --batch-bytes 262144 --stall-ms 0 --stall-total-ms 0");
    for part in log.chunks(8192) {
        rt.write(part);
    }
    let stats = rt.finish();
    eprintln!("fixture {path}: {stats:?}");
    assert!(stats.renders > 100, "{stats:?}");
    assert_eq!(stats.render_diff, 0, "{stats:?}");
    assert_eq!(stats.render_equal, stats.renders);
    assert_eq!(stats.verdict_mismatch, 0, "{stats:?}");
    let report = replay(&String::from_utf8_lossy(&log));
    assert_eq!(stats.renders as usize, report.renders);
    assert_eq!(stats.frames, report.frames_compared);
}
