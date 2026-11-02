// SPDX-License-Identifier: GPL-3.0-or-later
//! Run this executable with TSFP_RT_HOST_FAST_HOOKS=1 to select the ring without racing process env in tests.
use tsfp_rt_host::{Runtime, parse_args};
use tsfp_wgpu::binary::text_to_binary;
const TITLE: &str = include_str!("../../tsfp-wgpu/tests/data/title.stream");
fn start(extra: &str) -> Runtime {
    let args: Vec<String> = format!("--software --quiet --publish {extra}")
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    Runtime::start(parse_args(&args).unwrap()).unwrap()
}
#[test]
fn ring_and_legacy_queue_preserve_pixels_and_mixed_order() {
    // A stall policy explicitly selects the reference queue, independent of env.
    let legacy = start("--stall-ms 1000 --queue 64");
    let fast = start("--queue 64");
    for number in 1..=4 {
        let bytes = text_to_binary(TITLE).unwrap();
        legacy.write_binary(&bytes, 1);
        fast.write_binary(&bytes, 1);
        let comment = format!("# after present {number}\n");
        legacy.write(comment.as_bytes());
        fast.write(comment.as_bytes());
    }
    let reference = legacy.finish();
    let actual = fast.finish();
    assert_eq!(actual.presents, reference.presents);
    assert!(!actual.gave_up && !actual.render_error, "{actual:?}");
    assert_eq!(
        fast.take_frame().unwrap().rgba,
        legacy.take_frame().unwrap().rgba
    );
    if std::env::var("TSFP_RT_HOST_FAST_HOOKS").as_deref() == Ok("1") {
        assert!(actual.ring_publish_count > 0);
        assert_eq!(actual.stalls, 0);
    }
}

#[path = "../../tsfp-engine-render/src/hook_policy.rs"]
mod policy;

#[test]
fn counter_only_library_payload_preserves_rendered_state_and_clear_commands() {
    let reference = start("--stall-ms 1000");
    let filtered = start("");
    let methods = [
        (0x0B80, 0x12345678), // ignored program upload payload
        (0x1720, 0xABCDEF),   // library arrays are not the engine's array setup
        (0x0358, 0x01010101), // colour mask
        (0x1D90, 0xFF203040), // clear colour
        (0x1D94, 0xF0),       // command, never suppressed
        (0x1D94, 0xF0),       // identical command must still execute
        (0x02A8, 0x00302010), // combiner fog register
        (0x0A60, 0xFFFFFFFF), // combiner constant
        (0x1B08, 0x01010101), // texture addressing
    ];
    let mut original = Vec::new();
    let mut kept = Vec::new();
    for (method, value) in methods {
        tsfp_wgpu::trace_wire::lib(&mut original, method, value);
        if policy::live_library_method(method) {
            tsfp_wgpu::trace_wire::lib(&mut kept, method, value);
        }
    }
    let body = text_to_binary(TITLE).unwrap();
    original.extend_from_slice(&body);
    kept.extend_from_slice(&body);
    reference.write_binary(&original, 1);
    filtered.write_binary(&kept, 1);
    let a = reference.finish();
    let b = filtered.finish();
    assert!(!a.render_error && !b.render_error);
    assert_eq!(
        reference.take_frame().unwrap().rgba,
        filtered.take_frame().unwrap().rgba
    );
}

#[test]
fn full_ring_disables_the_state_stream_without_waiting_for_rendering() {
    if std::env::var("TSFP_RT_HOST_FAST_HOOKS").as_deref() != Ok("1") {
        return; // run with the flag, documented in the task report
    }
    let rt = start("--queue 8 --render-delay-ms 30 --discard");
    let mut frame = Vec::new();
    tsfp_wgpu::trace_wire::present(&mut frame, 1);
    let began = std::time::Instant::now();
    for _ in 0..1024 {
        rt.write_binary(&frame, 1);
    }
    assert!(began.elapsed() < std::time::Duration::from_millis(100));
    assert!(!rt.alive());
    let stats = rt.finish();
    assert!(stats.gave_up);
    assert_eq!(stats.stalls, 0);
    assert_eq!(stats.batches_dropped, 1);
    assert!(stats.presents < 1024);
}
