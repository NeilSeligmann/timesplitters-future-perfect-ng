// SPDX-License-Identifier: GPL-3.0-or-later
//! T2136: queue policy tests. They need a wgpu adapter (llvmpipe is enough, `--any-adapter`); comment lines (`# ...`) are valid
//! stream text and cost the renderer nothing, so the slowness is the test hook `--render-delay-ms`.
use std::time::{Duration, Instant};

use tsfp_rt_host::{Runtime, parse_args};

mod support;
use support::start;

#[test]
fn fast_renderer_never_stalls_the_guest() {
    let rt = start("--any-adapter --quiet --queue 8 --batch-bytes 4096");
    for i in 0..20000 {
        rt.send_line(&format!("# comment line {i}"));
    }
    let stats = rt.finish();
    assert_eq!(stats.lines_rendered, 20000);
    assert!(!stats.gave_up && stats.batches_dropped == 0);
    // A tight loop can briefly outrun even a trivial consumer on a loaded machine (MEASURED 2 waits, 0.5 ms total under load 28):
    // the bound is "negligible", not "never".
    assert!(
        stats.stall_ns_total < 100_000_000 && stats.stall_ns_max < 50_000_000,
        "a fast consumer must not stall the producer: {stats:?}"
    );
}

#[test]
fn slow_renderer_stall_is_bounded_then_gives_up() {
    // 400 ms per batch, room for 2 batches, give up after 300 ms of waiting in one hand-over.
    let rt = start(
        "--any-adapter --quiet --queue 2 --batch-bytes 64 --stall-ms 300 --render-delay-ms 400",
    );
    let mut worst = Duration::ZERO;
    let begin = Instant::now();
    for i in 0..400 {
        let t = Instant::now();
        rt.send_line(&format!(
            "# a comment line long enough to fill a batch number {i}"
        ));
        worst = worst.max(t.elapsed());
    }
    let total = begin.elapsed();
    let stats = rt.stats();
    assert!(
        stats.gave_up,
        "the renderer must be switched off: {stats:?}"
    );
    assert!(
        worst < Duration::from_millis(300 + 150),
        "one call waited {worst:?}"
    );
    assert!(
        total < Duration::from_secs(2),
        "after giving up the guest runs free, took {total:?}"
    );
    assert!(stats.stall_ns_max >= 300_000_000);
    rt.finish();
}

#[test]
fn many_short_waits_hit_the_total_budget() {
    // every wait is ~50 ms (under the per hand-over limit) but the run budget is 400 ms in total.
    let rt = start(
        "--any-adapter --quiet --queue 1 --batch-bytes 64 --stall-ms 5000 --stall-total-ms 400 --render-delay-ms 50",
    );
    let begin = Instant::now();
    for i in 0..400 {
        rt.send_line(&format!(
            "# a comment line long enough to fill a batch number {i}"
        ));
    }
    let total = begin.elapsed();
    let stats = rt.stats();
    assert!(stats.gave_up, "{stats:?}");
    assert!(
        total < Duration::from_millis(1500),
        "guest stalled {total:?} in total"
    );
    rt.finish();
}

#[test]
fn bounded_wait_without_giving_up_when_the_renderer_catches_up() {
    // 20 ms per batch with a generous limit: the guest waits (back pressure) but loses nothing.
    let rt = start(
        "--any-adapter --quiet --queue 2 --batch-bytes 64 --stall-ms 10000 --render-delay-ms 20",
    );
    for i in 0..60 {
        rt.send_line(&format!(
            "# a comment line long enough to fill a batch number {i}"
        ));
    }
    let stats = rt.finish();
    assert!(!stats.gave_up);
    assert_eq!(stats.lines_rendered, 60);
    assert!(stats.stalls > 0, "{stats:?}");
}

#[test]
fn partial_lines_are_reassembled() {
    let rt = start("--any-adapter --quiet --batch-bytes 64");
    rt.write(b"# one\n# tw");
    rt.write(b"o\n# three");
    let stats = rt.finish();
    assert_eq!(stats.lines_rendered, 3);
}

#[test]
fn bad_arguments_are_refused() {
    assert!(parse_args(&["--bogus".to_string()]).is_err());
    assert!(parse_args(&["--dump-at".to_string()]).is_err());
}

const TITLE: &str = include_str!("../../tsfp-wgpu/tests/data/title.stream");

#[test]
fn published_frame_has_the_scaled_size_and_latest_wins() {
    let rt = start("--any-adapter --quiet --publish --scale 2");
    for line in TITLE.lines() {
        rt.send_line(line);
    }
    rt.present(7);
    // Wait (bounded) until the render thread has finished the frame.
    let began = Instant::now();
    while rt.stats().frames_rendered == 0 && began.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(10));
    }
    let frame = rt.take_frame().expect("a published frame");
    assert_eq!((frame.width, frame.height), (1280, 960));
    assert_eq!(frame.rgba.len(), 1280 * 960 * 4);
    assert!(
        frame.rgba.iter().any(|&byte| byte != 0),
        "the frame is not blank"
    );
    assert!(rt.take_frame().is_none(), "taken once");
    rt.finish();
}

#[test]
fn a_renderer_that_is_behind_skips_frames_instead_of_stalling_the_guest() {
    // 30 ms per batch, one batch per present, skip frames while more than 2 frames wait.
    let rt = start("--any-adapter --quiet --publish --skip-behind 2 --render-delay-ms 30");
    for line in TITLE.lines() {
        rt.send_line(line);
    }
    let began = Instant::now();
    for number in 1..=40 {
        rt.present(number);
    }
    let guest_time = began.elapsed();
    let stats = rt.finish();
    assert_eq!(stats.stalls, 0, "no stall: {stats:?}");
    assert!(
        guest_time < Duration::from_millis(500),
        "the guest ran free: {guest_time:?}"
    );
    assert!(stats.frames_skipped > 0, "frames were skipped: {stats:?}");
    assert!(stats.frames_rendered > 0, "and some were drawn: {stats:?}");
}

/// The title frame as the lines of one frame without its own `present` line (`Runtime::present` adds it and counts it).
fn frame_lines() -> Vec<&'static str> {
    TITLE
        .lines()
        .filter(|line| line.trim() != "present")
        .collect()
}

fn run_frames(args: &str, frames: u64) -> (Runtime, Duration) {
    let rt = start(args);
    let began = Instant::now();
    for number in 1..=frames {
        for line in frame_lines() {
            rt.send_line(line);
        }
        rt.present(number);
    }
    (rt, began.elapsed())
}

#[test]
fn dropping_frames_keeps_the_next_drawn_frame_identical_to_the_no_drop_render() {
    let frames = 30;
    let (rt, _) = run_frames("--any-adapter --quiet --publish", frames);
    let reference = {
        let stats = rt.finish();
        assert_eq!(stats.frames_skipped, 0, "{stats:?}");
        rt.take_frame().expect("reference frame")
    };
    // A slow consumer (20 ms per batch) with the producer running free: frames are dropped, state events still apply.
    let (rt, guest_time) = run_frames(
        "--any-adapter --quiet --publish --skip-behind 1 --render-delay-ms 20",
        frames,
    );
    let stats = rt.finish();
    let dropped = rt.take_frame().expect("last frame of the dropping run");
    assert!(
        stats.frames_skipped > 0,
        "frames must be dropped: {stats:?}"
    );
    assert_eq!(stats.stalls, 0, "{stats:?}");
    assert!(
        guest_time < Duration::from_millis(400),
        "the guest ran free: {guest_time:?}"
    );
    assert_eq!(
        dropped.present, reference.present,
        "both end at present {frames}"
    );
    assert_eq!(
        (dropped.width, dropped.height),
        (reference.width, reference.height)
    );
    assert!(
        dropped.rgba == reference.rgba,
        "the next drawn frame after drops differs from the no drop render"
    );
}

#[test]
fn the_default_queue_never_stalls_the_guest_however_slow_the_renderer_is() {
    // No stall flags: unbounded queue. A 20 ms per batch renderer, 100 frames: the guest finishes at once, nothing is switched off.
    let (rt, guest_time) = run_frames("--any-adapter --quiet --render-delay-ms 20", 100);
    let during = rt.stats();
    assert!(
        guest_time < Duration::from_millis(1500),
        "the guest never waits: {guest_time:?}"
    );
    assert_eq!(during.stalls, 0, "{during:?}");
    assert!(!during.gave_up, "{during:?}");
    assert!(during.queue_frames_max > 10, "frames queued up: {during:?}");
    let stats = rt.finish();
    assert_eq!(stats.presents, 100, "every frame was rendered: {stats:?}");
    assert!(
        stats.enqueue_ns_max > 0 && stats.busy_ns_max > 0,
        "{stats:?}"
    );
}

#[test]
fn the_queue_memory_cap_switches_the_renderer_off_instead_of_blocking() {
    // 1 MiB cap, 50 ms per batch: a long burst overruns the cap, the guest still never waits.
    let (rt, guest_time) = run_frames(
        "--any-adapter --quiet --queue-mb 1 --render-delay-ms 50",
        200,
    );
    let stats = rt.finish();
    assert!(stats.gave_up, "the cap must trip: {stats:?}");
    assert_eq!(stats.stalls, 0, "{stats:?}");
    assert!(guest_time < Duration::from_millis(3000), "{guest_time:?}");
}

#[test]
fn output_size_changes_at_run_time_and_the_next_frame_has_the_new_size() {
    let rt = start("--any-adapter --quiet --publish");
    let wait_for = |frames: u64| {
        let began = Instant::now();
        while rt.stats().frames_rendered < frames && began.elapsed() < Duration::from_secs(60) {
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    for line in frame_lines() {
        rt.send_line(line);
    }
    rt.present(1);
    wait_for(1);
    let first = rt.take_frame().expect("first frame");
    assert_eq!((first.width, first.height), (640, 480));
    rt.set_output_size(960, 720);
    for line in frame_lines() {
        rt.send_line(line);
    }
    rt.present(2);
    wait_for(2);
    let second = rt.take_frame().expect("second frame");
    assert_eq!((second.width, second.height), (960, 720));
    assert_eq!(second.rgba.len(), 960 * 720 * 4);
    rt.finish();
}

#[test]
fn completion_stats_are_refreshed_while_the_producer_is_idle() {
    let rt = start("--any-adapter --quiet");
    for line in frame_lines() {
        rt.send_line(line);
    }
    rt.present(1);
    rt.flush();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let stats = rt.stats();
        if stats.gpu_completion_observations != 0 {
            assert_eq!(stats.gpu_completion_observations, 1);
            assert!(stats.gpu_latency_avg_us > 0);
            assert!(stats.summary().contains("not GPU time"));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "idle completion not published: {stats:?}"
        );
        std::thread::yield_now();
    }
    assert_eq!(rt.finish().gpu_completion_observations, 1);
}
