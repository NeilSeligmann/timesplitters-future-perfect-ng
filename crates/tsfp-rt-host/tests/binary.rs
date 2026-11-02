// SPDX-License-Identifier: GPL-3.0-or-later
//! T2136d: the binary event path of the runtime (`Runtime::write_binary`, C ABI `tsfp_rt_host_send_binary`): the frames it
//! renders equal the text path's, text and binary can be mixed in order, and the queue policies (never stall, memory cap,
//! bounded back-pressure, malformed input) hold for binary batches too. They need a wgpu adapter (llvmpipe is enough).
use std::time::{Duration, Instant};

use tsfp_rt_host::Runtime;
use tsfp_wgpu::binary::text_to_binary;

const TITLE: &str = include_str!("../../tsfp-wgpu/tests/data/title.stream");

mod support;
use support::start;

/// The title frame without its own `present` line.
fn frame_text() -> String {
    TITLE
        .lines()
        .filter(|line| line.trim() != "present")
        .map(|line| format!("{line}\n"))
        .collect()
}

/// One frame as binary records ending in the named `Present`.
fn frame_binary(number: u64) -> Vec<u8> {
    text_to_binary(&format!("{}# present {number}\npresent\n", frame_text())).expect("convert")
}

/// T2315: a binary frame end names its present, so the frame layers are sealed for that guest frame.
#[test]
fn trailing_present_names_the_binary_frame_end() {
    assert_eq!(Runtime::trailing_present(&frame_binary(1234)), Some(1234));
    let mut cut = frame_binary(7);
    cut.pop();
    assert_eq!(Runtime::trailing_present(&cut), None, "a batch not ending in a whole Present record");
    assert_eq!(Runtime::trailing_present(&[]), None);
    assert_eq!(Runtime::trailing_present(&frame_binary(0)), None, "an unnamed present");
}

fn run_text(args: &str, frames: u64) -> Runtime {
    let rt = start(args);
    for number in 1..=frames {
        for line in frame_text().lines() {
            rt.send_line(line);
        }
        rt.present(number);
    }
    rt
}

fn run_binary(args: &str, frames: u64) -> (Runtime, Duration) {
    let rt = start(args);
    let began = Instant::now();
    for number in 1..=frames {
        rt.write_binary(&frame_binary(number), 1);
    }
    (rt, began.elapsed())
}

#[test]
fn binary_frames_render_pixel_identical_to_the_text_frames() {
    let text = run_text("--any-adapter --quiet --publish", 5);
    text.finish();
    let reference = text.take_frame().expect("text frame");
    let (binary, _) = run_binary("--any-adapter --quiet --publish", 5);
    let stats = binary.finish();
    let frame = binary.take_frame().expect("binary frame");
    assert_eq!(stats.presents, 5, "{stats:?}");
    assert!(!stats.render_error, "{stats:?}");
    assert_eq!(frame.present, reference.present);
    assert_eq!(
        (frame.width, frame.height),
        (reference.width, reference.height)
    );
    assert!(
        frame.rgba == reference.rgba,
        "binary frame differs from the text frame"
    );
    assert!(
        frame
            .rgba
            .chunks_exact(4)
            .any(|pixel| pixel[..3] != frame.rgba[..3]),
        "the frame is not blank"
    );
}

#[test]
fn text_and_binary_can_be_mixed_and_keep_their_order() {
    let reference = run_text("--any-adapter --quiet --publish", 3);
    reference.finish();
    let reference = reference.take_frame().expect("reference");
    // frame 1 as text, frame 2 as binary, frame 3 as text again, and a text line between binary writes
    let rt = start("--any-adapter --quiet --publish");
    for line in frame_text().lines() {
        rt.send_line(line);
    }
    rt.present(1);
    rt.write_binary(&frame_binary(2), 1);
    for line in frame_text().lines() {
        rt.send_line(line);
    }
    rt.present(3);
    let stats = rt.finish();
    assert_eq!(stats.presents, 3, "{stats:?}");
    assert!(!stats.render_error, "{stats:?}");
    let frame = rt.take_frame().expect("mixed frame");
    assert!(frame.rgba == reference.rgba, "mixed run differs");
}

#[test]
fn a_frame_split_over_several_binary_writes_is_one_frame() {
    let bytes = frame_binary(1);
    let rt = start("--any-adapter --quiet --publish --batch-bytes 1024");
    // cut at record boundaries: walk the length prefixes
    let mut at = 0;
    let mut pieces = Vec::new();
    while at < bytes.len() {
        let length = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        pieces.push((at, at + 4 + length));
        at += 4 + length;
    }
    let last = pieces.len() - 1;
    for (index, (from, to)) in pieces.into_iter().enumerate() {
        rt.write_binary(&bytes[from..to], u64::from(index == last));
    }
    let stats = rt.finish();
    assert_eq!(stats.presents, 1, "{stats:?}");
    assert!(!stats.render_error, "{stats:?}");
}

#[test]
fn the_default_queue_never_stalls_the_guest_with_binary_batches() {
    let (rt, guest_time) = run_binary("--any-adapter --quiet --render-delay-ms 20", 100);
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
}

#[test]
fn binary_frames_are_dropped_when_the_renderer_is_behind_and_the_next_drawn_frame_is_identical() {
    let (rt, _) = run_binary("--any-adapter --quiet --publish", 30);
    rt.finish();
    let reference = rt.take_frame().expect("reference");
    let (rt, guest_time) = run_binary(
        "--any-adapter --quiet --publish --skip-behind 1 --render-delay-ms 20",
        30,
    );
    let stats = rt.finish();
    let dropped = rt.take_frame().expect("last frame");
    assert!(stats.frames_skipped > 0, "{stats:?}");
    assert_eq!(stats.stalls, 0, "{stats:?}");
    assert!(guest_time < Duration::from_millis(400), "{guest_time:?}");
    assert!(dropped.rgba == reference.rgba);
}

#[test]
fn the_queue_memory_cap_switches_the_renderer_off_for_binary_batches_too() {
    let (rt, guest_time) = run_binary(
        "--any-adapter --quiet --queue-mb 1 --render-delay-ms 50",
        200,
    );
    let stats = rt.finish();
    assert!(stats.gave_up, "the cap must trip: {stats:?}");
    assert_eq!(stats.stalls, 0, "{stats:?}");
    assert!(guest_time < Duration::from_millis(3000), "{guest_time:?}");
}

#[test]
fn bounded_back_pressure_applies_to_binary_batches_and_gives_up_at_the_limit() {
    // 400 ms per batch, room for 2 batches, give up after 300 ms of waiting in one hand-over (as the text test does)
    let rt = start(
        "--any-adapter --quiet --queue 2 --batch-bytes 64 --stall-ms 300 --render-delay-ms 400",
    );
    let frame = frame_binary(1);
    let began = Instant::now();
    let mut worst = Duration::ZERO;
    for _ in 0..20 {
        let t = Instant::now();
        rt.write_binary(&frame, 1);
        worst = worst.max(t.elapsed());
    }
    let stats = rt.stats();
    assert!(stats.gave_up, "{stats:?}");
    assert!(
        worst < Duration::from_millis(450),
        "one call waited {worst:?}"
    );
    assert!(began.elapsed() < Duration::from_secs(3));
    assert!(stats.stall_ns_max >= 300_000_000, "{stats:?}");
    rt.finish();
}

#[test]
fn malformed_binary_is_a_render_error_and_the_guest_keeps_running() {
    let rt = start("--any-adapter --quiet");
    let mut bytes = frame_binary(1);
    bytes.truncate(bytes.len() - 3); // cut the last record
    rt.write_binary(&bytes, 1);
    for number in 2..50 {
        rt.write_binary(&frame_binary(number), 1);
    }
    let stats = rt.finish();
    assert!(stats.render_error, "{stats:?}");
    assert_eq!(stats.stalls, 0, "{stats:?}");
}

#[test]
fn discard_counts_binary_presents_without_rendering() {
    let (rt, _) = run_binary("--any-adapter --quiet --discard", 40);
    let stats = rt.finish();
    assert_eq!(stats.presents, 40, "{stats:?}");
    assert_eq!(stats.lines_rendered, 0, "{stats:?}");
}
