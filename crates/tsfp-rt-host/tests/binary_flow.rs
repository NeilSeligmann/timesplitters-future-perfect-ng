// SPDX-License-Identifier: GPL-3.0-or-later
//! T2136d/T2226: binary frame flags, feedback-safe producer pressure, and the byte bound of the queue.
use tsfp_rt_host::Options;
use tsfp_wgpu::binary::text_to_binary;
use tsfp_wgpu::trace_wire;

const TITLE: &str = include_str!("../../tsfp-wgpu/tests/data/title.stream");

mod support;
use support::start;

fn frame_binary(number: u64) -> Vec<u8> {
    let body: String = TITLE
        .lines()
        .filter(|line| line.trim() != "present")
        .map(|line| format!("{line}\n"))
        .collect();
    text_to_binary(&format!("{body}# present {number}\npresent\n")).expect("convert")
}

fn frame_record(flags: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    trace_wire::frame(&mut bytes, flags);
    bytes
}

#[test]
fn legacy_omission_flags_suppress_publication_and_the_next_one_is_identical() {
    let reference = start("--any-adapter --quiet --publish");
    for number in 1..=5 {
        reference.write_binary(&frame_binary(number), 1);
    }
    reference.finish();
    let reference = reference.take_frame().expect("reference");
    let rt = start("--any-adapter --quiet --publish");
    rt.write_binary(&frame_binary(1), 1);
    rt.write_binary(&frame_record(1), 0); // legacy flag: these synthetic frames still carry draws
    for number in 2..=4 {
        rt.write_binary(&frame_binary(number), 1);
    }
    rt.write_binary(&frame_record(0), 0);
    rt.write_binary(&frame_binary(5), 1);
    let stats = rt.finish();
    assert_eq!(stats.presents, 5, "{stats:?}");
    assert_eq!(stats.frames_omitted, 3, "{stats:?}");
    assert_eq!(
        stats.frames_rendered, 2,
        "frame 1 and frame 5 are published: {stats:?}"
    );
    assert!(!stats.render_error, "{stats:?}");
    let frame = rt.take_frame().expect("last frame");
    assert!(
        frame.rgba == reference.rgba,
        "the frame after the omitted ones differs"
    );
}

#[test]
fn async_pipelines_never_block_the_frame_and_the_settled_frame_is_identical_to_the_sync_render() {
    let sync = start("--any-adapter --quiet --publish --sync-pipelines");
    for number in 1..=30 {
        sync.write_binary(&frame_binary(number), 1);
    }
    let sync_stats = sync.finish();
    assert_eq!(sync_stats.draws_deferred, 0, "{sync_stats:?}");
    assert!(sync_stats.pipelines_built > 0, "{sync_stats:?}");
    let reference = sync.take_frame().expect("sync frame");
    // a renderer that takes 15 ms per batch gives the pipeline workers time to finish between frames
    let rt = start("--any-adapter --quiet --publish --async-pipelines 2 --render-delay-ms 15");
    for number in 1..=30 {
        rt.write_binary(&frame_binary(number), 1);
    }
    let stats = rt.finish();
    assert!(stats.pipelines_built > 0, "{stats:?}");
    assert!(!stats.render_error, "{stats:?}");
    let frame = rt.take_frame().expect("async frame");
    assert!(
        frame.rgba == reference.rgba,
        "the settled frame differs from the synchronous render (deferred draws {})",
        stats.draws_deferred
    );
}

#[test]
fn pressure_never_omits_draws_even_when_the_renderer_is_behind() {
    let rt = start("--any-adapter --quiet --render-delay-ms 40 --drop-behind 2");
    assert!(!rt.pressure());
    for number in 1..=12 {
        rt.write_binary(&frame_binary(number), 1);
    }
    assert!(
        !rt.pressure(),
        "feedback needs queued draws: {:?}",
        rt.stats()
    );
    rt.finish();
    assert!(!rt.pressure(), "caught up after finish");
    let rt = start("--any-adapter --quiet --render-delay-ms 40");
    for number in 1..=12 {
        rt.write_binary(&frame_binary(number), 1);
    }
    assert!(!rt.pressure(), "no limit, no pressure");
    rt.finish();
}

#[test]
fn the_default_queue_cap_is_64_mib_and_a_big_queue_keeps_surface_writes() {
    assert_eq!(Options::default().queue_bytes_cap, 64 << 20);
    // a big payload (a 3 MiB memory record) queued behind a slow renderer raises the pressure by bytes alone
    let rt =
        start("--any-adapter --quiet --render-delay-ms 200 --drop-behind 50 --drop-queue-mb 2");
    let mut payload = Vec::new();
    trace_wire::memory(&mut payload, 0x1000, &vec![7u8; 3 << 20]);
    rt.write_binary(&payload, 0);
    rt.write_binary(&payload, 0);
    assert!(
        !rt.pressure(),
        "feedback needs queued draws: {:?}",
        rt.stats()
    );
    rt.finish();
}
