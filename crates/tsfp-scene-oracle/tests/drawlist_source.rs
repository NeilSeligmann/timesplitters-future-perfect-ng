// SPDX-License-Identifier: GPL-3.0-or-later
//! Fabricated wire fixtures only; retail capture bytes are never committed.
use std::io::Cursor;
use tsfp_render_materials::Assets;
use tsfp_scene_compare::{Capture, Failure, FrameSource, Limits as CompareLimits, Side, compare};
use tsfp_scene_model::{CONTRACT_VERSION, ContentId};
use tsfp_scene_oracle::{StreamDrawListSource, Window, wire};

fn window(frames: u64) -> Window {
    Window {
        first_present: 17,
        frame_count: frames,
    }
}

fn capture(frame_count: u64) -> Capture {
    let id = |byte| ContentId([byte; 32]);
    Capture {
        contract_version: CONTRACT_VERSION,
        decoder_version: 1,
        original_revision: [1; 20],
        original_prebuilt: id(1),
        route: id(2),
        input: id(3),
        stream: id(4),
        resources: id(5),
        first_present: 17,
        frame_count,
    }
}

fn trace(extra_call: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    wire::frame(&mut bytes, 0);
    wire::call(&mut bytes, "DrawVertices", &[3, 0, 6]);
    if extra_call {
        wire::call(&mut bytes, "SetTexture", &[0, 7]);
    }
    wire::memory(&mut bytes, 0x100, &[1, 2, 3, 4]);
    wire::texture(&mut bytes, 7, 1, 1, &[1, 2, 3, 4]);
    wire::present(&mut bytes, 17);
    bytes
}

#[test]
fn original_stream_becomes_present_aligned_but_incomplete_drawlist() {
    let bytes = trace(false);
    let mut source = StreamDrawListSource::new(Cursor::new(bytes), window(1)).unwrap();
    let frame = source.next_frame(64 * 1024).unwrap().unwrap();
    assert_eq!(frame.present, 17);
    assert_eq!(frame.gaps.unmatched_views, 1);
    assert_eq!(frame.gaps.unsupported_states, 1);
    assert_eq!(frame.gaps.missing_resources, 2);
    assert!(frame.draws.cameras.is_empty());
    assert!(frame.draws.passes.is_empty());
    assert!(source.next_frame(64 * 1024).unwrap().is_none());
}

#[test]
fn semantic_comparison_rejects_the_adapter_output_as_incomplete() {
    let bytes = trace(false);
    let mut oracle = StreamDrawListSource::new(Cursor::new(bytes.clone()), window(1)).unwrap();
    let mut original = StreamDrawListSource::new(Cursor::new(bytes), window(1)).unwrap();
    let error = compare(
        &capture(1),
        &capture(1),
        [2; 20],
        &mut oracle,
        &mut original,
        CompareLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error.failure,
        Failure::Incomplete {
            side: Side::Oracle,
            ..
        }
    ));
}

#[test]
fn added_render_call_increases_the_measured_unsupported_state_gap() {
    let mut base = StreamDrawListSource::new(Cursor::new(trace(false)), window(1)).unwrap();
    let mut mutated = StreamDrawListSource::new(Cursor::new(trace(true)), window(1)).unwrap();
    let original = base.next_frame(64 * 1024).unwrap().unwrap();
    let changed = mutated.next_frame(64 * 1024).unwrap().unwrap();
    assert_eq!(
        changed.gaps.unsupported_states,
        original.gaps.unsupported_states + 1
    );
}

#[test]
fn record_limit_is_applied_before_the_reader_allocates_it() {
    let bytes = trace(false);
    let mut source = StreamDrawListSource::new(Cursor::new(bytes), window(1)).unwrap();
    assert!(source.next_frame(8).is_err());
}

#[test]
fn snapshot_rejects_duplicate_and_out_of_window_material_snapshots() {
    let source = StreamDrawListSource::new(Cursor::new(trace(false)), window(1))
        .unwrap()
        .with_material_snapshot(17, &Assets::default(), &[])
        .unwrap();

    assert!(
        source
            .with_material_snapshot(17, &Assets::default(), &[])
            .is_err()
    );
    assert!(
        StreamDrawListSource::new(Cursor::new(trace(false)), window(1))
            .unwrap()
            .with_material_snapshot(18, &Assets::default(), &[])
            .is_err()
    );
}
