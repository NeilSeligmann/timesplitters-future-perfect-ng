// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024 tests. The first group needs no GPU (stream text form, golden recorded stream, decoder
//! report). The second runs the replayer on a software adapter and checks pixels computed by
//! hand from the blend equations, plus the frame hash pinned to llvmpipe. Without a software
//! adapter the GPU tests print `SKIPPED` and return: they do not prove anything there.
use tsfp_engine_render::device::DeviceCall;
use tsfp_wgpu::context::{GpuContext, Preference};
use tsfp_wgpu::replay::{Frame, Replayer};
use tsfp_wgpu::scenes::title_scene;
use tsfp_wgpu::stream::{Event, Stream};

const GOLDEN_STREAM: &str = include_str!("data/title.stream");
/// FNV-1a of the title frame on llvmpipe (LLVM 19.1.7, Vulkan, lavapipe). MEASURED on this
/// host 2026-10-10. Other rasterisers may differ by one level at some edge pixels, so the hash
/// is only asserted on a `Cpu` adapter named llvmpipe.
const TITLE_HASH_LLVMPIPE: u64 = 0xE0A9_EFF0_8383_32D4;

#[test]
fn stream_text_round_trips() {
    let stream = title_scene();
    assert!(!stream.events.is_empty());
    let parsed = Stream::from_text(&stream.to_text()).expect("parse");
    assert_eq!(parsed, stream);
}

#[test]
fn golden_stream_matches_what_the_render_code_records() {
    let recorded = title_scene().to_text();
    if std::env::var("TSFP_UPDATE_GOLDEN").is_ok() {
        std::fs::write(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/title.stream"),
            &recorded,
        )
        .unwrap();
        return;
    }
    assert_eq!(
        recorded, GOLDEN_STREAM,
        "the render code recorded a different call stream (TSFP_UPDATE_GOLDEN=1 to accept)"
    );
}

#[test]
fn parse_rejects_garbage() {
    assert!(Stream::from_text("call Nonsense 1").is_err());
    assert!(Stream::from_text("mem zz 00").is_err());
    assert!(
        Stream::from_text("# only a comment\n\n")
            .unwrap()
            .events
            .is_empty()
    );
}

#[test]
fn typed_vertex_bindings_round_trip_through_the_stream_format() {
    let formats = core::array::from_fn(|index| 0x1000 + index as u32);
    let offsets = core::array::from_fn(|index| 0x7000_0000 + 4 * index as u32);
    let stream = Stream {
        events: vec![Event::Call(DeviceCall::SetVertexBindings {
            formats,
            offsets,
        })],
    };
    let expected_words = formats
        .iter()
        .chain(&offsets)
        .map(|word| format!(" {word:x}"))
        .collect::<String>();
    assert!(
        stream
            .to_text()
            .contains(&format!("call SetVertexBindings{expected_words}\n"))
    );
    assert_eq!(
        Stream::from_text(&stream.to_text()).expect("typed bindings parse"),
        stream
    );
}

fn replay_title() -> Option<(Replayer, Frame)> {
    let gpu = match GpuContext::new(&Preference::Software) {
        Ok(gpu) => gpu,
        Err(error) => {
            println!("SKIPPED (no software adapter): {error}");
            return None;
        }
    };
    let mut replayer = Replayer::new(gpu, 640, 480);
    replayer.run(&Stream::from_text(GOLDEN_STREAM).expect("golden parses"));
    assert_eq!(replayer.frames.len(), 1);
    let frame = replayer.frames[0].clone();
    Some((replayer, frame))
}

fn near(actual: [u8; 4], expected: [i32; 4], label: &str) {
    for channel in 0..4 {
        assert!(
            (i32::from(actual[channel]) - expected[channel]).abs() <= 2,
            "{label}: got {actual:?} want {expected:?}"
        );
    }
}

#[test]
fn title_frame_pixels_follow_the_blend_equations() {
    let Some((_, frame)) = replay_title() else {
        return;
    };
    // Opaque grey slab (100,100,100) is the destination of the three panels.
    near(frame.pixel(400, 440), [100, 100, 100, 255], "slab");
    // Mode 1 source alpha / inverse: (255,128,0) at alpha 128/255 over 100.
    near(frame.pixel(100, 350), [178, 114, 50, 191], "alpha blend");
    // Mode 2 additive: 100 + (32,64,96) * 128/255.
    near(frame.pixel(280, 350), [116, 132, 148, 255], "additive");
    // Mode 3 reverse subtract: dst - src * alpha.
    near(frame.pixel(450, 350), [84, 68, 52, 191], "reverse subtract");
    // Scissor (60,400)-(300,440): inside the banner colour, outside the slab.
    near(
        frame.pixel(100, 420),
        [200, 40, 40, 255],
        "banner inside scissor",
    );
    near(
        frame.pixel(59, 420),
        [100, 100, 100, 255],
        "banner left of scissor",
    );
    near(
        frame.pixel(301, 420),
        [100, 100, 100, 255],
        "banner right of scissor",
    );
    // Background gradient: red grows to the right, green downwards, blue constant 128.
    let (top_left, bottom_right) = (frame.pixel(2, 2), frame.pixel(637, 285));
    assert!(
        top_left[0] < 10 && top_left[1] < 10 && (126..=130).contains(&top_left[2]),
        "{top_left:?}"
    );
    assert!(
        bottom_right[0] > 240 && bottom_right[1] > 130,
        "{bottom_right:?}"
    );
    // The logo disc is bright in the middle, and fades into the gradient at its rim.
    assert!(frame.pixel(320, 100)[0] > 200);
    // The quad corner has disc alpha 0: it shows the gradient, as the pixel just outside does.
    let (inside, outside) = (frame.pixel(201, 41), frame.pixel(197, 41));
    for channel in 0..3 {
        assert!(
            (i32::from(inside[channel]) - i32::from(outside[channel])).abs() <= 6,
            "{inside:?} {outside:?}"
        );
    }
}

#[test]
fn title_frame_hash_and_report() {
    let Some((replayer, frame)) = replay_title() else {
        return;
    };
    // The decoder saw exactly the methods the slice documents as unsupported here: the
    // specular fog combiner words 0x288 and 0x28C that refresh_blend_methods pushes before
    // every draw (7 draws). Nothing else is unhandled, unmapped, skipped or missing.
    let report = &replayer.report;
    assert_eq!(
        report.unhandled_methods.keys().copied().collect::<Vec<_>>(),
        vec![0x288, 0x28C]
    );
    assert!(
        report.unhandled_methods.values().all(|count| *count == 7),
        "{report:?}"
    );
    assert!(
        report.unmapped_values.is_empty()
            && report.skipped_draws.is_empty()
            && report.missing_textures.is_empty()
    );
    let name = replayer.adapter_name();
    if name.contains("llvmpipe") {
        assert_eq!(
            frame.hash(),
            TITLE_HASH_LLVMPIPE,
            "title frame hash on {name}: {:016x}",
            frame.hash()
        );
    } else {
        println!("hash not asserted on {name}: {:016x}", frame.hash());
    }
}

#[test]
fn png_output_is_a_valid_signature_and_size() {
    let frame = Frame {
        width: 2,
        height: 2,
        rgba: vec![255; 16],
    };
    let png = frame.to_png();
    assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
}
