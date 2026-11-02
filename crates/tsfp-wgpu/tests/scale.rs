// SPDX-License-Identifier: GPL-3.0-or-later
//! T2136c: the render scale. The same stream at 2x and 3x of 640x480 must box-downsample to the 1x image within a small tolerance,
//! 1x stays the unscaled path, and `readback` off hands out the GPU texture instead of CPU pixels. Needs a software adapter.
use tsfp_wgpu::context::{GpuContext, Preference};
use tsfp_wgpu::replay::{Frame, Replayer};
use tsfp_wgpu::scenes::title_scene;
use tsfp_wgpu::stream::Stream;

const GOLDEN_STREAM: &str = include_str!("data/title.stream");

fn render(stream: &Stream, output: Option<(u32, u32)>, readback: bool) -> Option<Replayer> {
    let gpu = GpuContext::new(&Preference::Software).ok()?;
    let mut replayer = Replayer::new(gpu, 640, 480);
    if let Some((width, height)) = output {
        replayer.set_output_size(width, height);
    }
    replayer.readback = readback;
    replayer.run(stream);
    Some(replayer)
}

/// Mean absolute channel error of `big` box-downsampled by `factor` against `small`.
fn downsample_error(small: &Frame, big: &Frame, factor: u32) -> f64 {
    assert_eq!(
        (small.width * factor, small.height * factor),
        (big.width, big.height)
    );
    let mut total = 0u64;
    for y in 0..small.height {
        for x in 0..small.width {
            for channel in 0..4usize {
                let mut sum = 0u32;
                for dy in 0..factor {
                    for dx in 0..factor {
                        let at = (((y * factor + dy) * big.width + x * factor + dx) * 4) as usize
                            + channel;
                        sum += u32::from(big.rgba[at]);
                    }
                }
                let mean = f64::from(sum) / f64::from(factor * factor);
                let reference =
                    f64::from(small.rgba[((y * small.width + x) * 4) as usize + channel]);
                total += (mean - reference).abs().round() as u64;
            }
        }
    }
    total as f64 / f64::from(small.width * small.height * 4)
}

fn check(stream: &Stream, label: &str) {
    let Some(one) = render(stream, None, true) else {
        println!("SKIPPED (no software adapter)");
        return;
    };
    let unscaled = render(stream, Some((640, 480)), true).expect("adapter");
    assert_eq!(
        one.frames[0].rgba, unscaled.frames[0].rgba,
        "{label}: explicit 640x480 is the 1x path"
    );
    for factor in [2u32, 3] {
        let big = render(stream, Some((640 * factor, 480 * factor)), true).expect("adapter");
        assert_eq!(big.frames.len(), one.frames.len());
        let frame = &big.frames[0];
        assert_eq!((frame.width, frame.height), (640 * factor, 480 * factor));
        let error = downsample_error(&one.frames[0], frame, factor);
        println!("{label}: {factor}x downsample mean abs error {error:.3}");
        assert!(error < 3.0, "{label} {factor}x: mean abs error {error}");
        if let Ok(dir) = std::env::var("TSFP_SCALE_PNG_DIR") {
            std::fs::write(format!("{dir}/{label}-{factor}x.png"), frame.to_png()).unwrap();
        }
    }
    if let Ok(dir) = std::env::var("TSFP_SCALE_PNG_DIR") {
        std::fs::write(format!("{dir}/{label}-1x.png"), one.frames[0].to_png()).unwrap();
    }
}

#[test]
fn scaled_golden_title_stream_matches_the_unscaled_frame() {
    check(
        &Stream::from_text(GOLDEN_STREAM).expect("golden parses"),
        "golden",
    );
}

#[test]
fn scaled_title_scene_matches_the_unscaled_frame() {
    check(&title_scene(), "title-scene");
}

#[test]
fn widescreen_output_size_renders_without_error() {
    let Some(replayer) = render(&title_scene(), Some((1280, 720)), true) else {
        println!("SKIPPED (no software adapter)");
        return;
    };
    assert_eq!(
        (replayer.frames[0].width, replayer.frames[0].height),
        (1280, 720)
    );
}

/// Cost per frame of the title stream by output size and readback, printed for docs/t-wgpu-default-presentation.md.
/// A measurement, not an assertion: run with `--nocapture`. The numbers depend on the adapter, which is printed.
#[test]
fn scale_cost_report() {
    let stream = Stream::from_text(GOLDEN_STREAM).expect("golden parses");
    for (width, height) in [
        (640, 480),
        (1280, 960),
        (1920, 1080),
        (1920, 1440),
        (2560, 1920),
    ] {
        for readback in [true, false] {
            let Some(mut replayer) = render(&stream, Some((width, height)), readback) else {
                println!("SKIPPED (no software adapter)");
                return;
            };
            let started = std::time::Instant::now();
            for _ in 0..10 {
                replayer.run(&stream);
            }
            println!(
                "cost {} {width}x{height} readback {readback}: {:.1} ms per frame",
                replayer.adapter_name(),
                started.elapsed().as_secs_f64() * 100.0
            );
        }
    }
}

#[test]
fn readback_off_keeps_the_texture_and_skips_the_pixels() {
    let Some(replayer) = render(&title_scene(), Some((960, 720)), false) else {
        println!("SKIPPED (no software adapter)");
        return;
    };
    assert!(replayer.frames[0].rgba.is_empty());
    let texture = replayer.display_texture().expect("display texture");
    assert_eq!((texture.width(), texture.height()), (960, 720));
}

/// T2136e: a runtime resize keeps the stream state: after 1x then 2x the next frame equals a fresh renderer that ran at 2x all along.
#[test]
fn runtime_resize_matches_a_fresh_renderer_at_the_new_size() {
    let stream = Stream::from_text(GOLDEN_STREAM).expect("golden parses");
    let Ok(gpu) = GpuContext::new(&Preference::Software) else {
        println!("SKIPPED (no software adapter)");
        return;
    };
    let mut resized = Replayer::new(gpu, 640, 480);
    resized.run(&stream);
    let first_1x = resized.frames[0].rgba.clone();
    resized.set_output_size(1280, 960);
    resized.run(&stream);
    let after = resized.frames.last().expect("frame after resize").clone();
    assert_eq!((after.width, after.height), (1280, 960));

    let mut fresh = render(&stream, Some((1280, 960)), true).expect("adapter");
    fresh.run(&stream);
    assert_eq!(
        after.rgba,
        fresh.frames.last().unwrap().rgba,
        "resized frame equals a fresh 2x frame of the same state"
    );
    resized.set_output_size(640, 480);
    resized.run(&stream);
    assert_eq!(resized.frames.last().unwrap().rgba, first_1x);
}

/// T2095: a window sized present at the internal size is the identity, a 16:9 window pillarboxes the 4:3 image in black, and a
/// runtime window resize and the return to no window both work.
#[test]
fn window_present_is_identity_letterboxes_and_resizes() {
    let stream = Stream::from_text(GOLDEN_STREAM).expect("golden parses");
    let Ok(gpu) = GpuContext::new(&Preference::Software) else {
        println!("SKIPPED (no software adapter)");
        return;
    };
    let mut replayer = Replayer::new(gpu, 640, 480);
    replayer.run(&stream);
    let plain = replayer.frames.last().unwrap().clone();

    replayer.set_window_size(Some((640, 480)));
    replayer.run(&stream);
    let same = replayer.frames.last().unwrap();
    assert_eq!((same.width, same.height), (640, 480));
    assert_eq!(
        same.rgba, plain.rgba,
        "present at equal size is the identity"
    );

    // 1920x1080 window from a 2x internal image: bars of 240 px left and right
    replayer.set_output_size(1280, 960);
    replayer.set_window_size(Some((1920, 1080)));
    replayer.run(&stream);
    let wide = replayer.frames.last().unwrap().clone();
    assert_eq!((wide.width, wide.height), (1920, 1080));
    let pixel = |x: u32, y: u32| {
        let at = ((y * wide.width + x) * 4) as usize;
        [wide.rgba[at], wide.rgba[at + 1], wide.rgba[at + 2]]
    };
    for y in [0, 540, 1079] {
        assert_eq!(pixel(0, y), [0, 0, 0], "left bar");
        assert_eq!(pixel(239, y), [0, 0, 0], "left bar edge");
        assert_eq!(pixel(1680, y), [0, 0, 0], "right bar edge");
    }
    let inside: u32 = (0..1080)
        .step_by(7)
        .flat_map(|y| (240..1680).step_by(11).map(move |x| (x, y)))
        .map(|(x, y)| pixel(x, y).iter().map(|&c| u32::from(c)).sum::<u32>())
        .sum();
    assert!(inside > 0, "the picture is drawn inside the bars");

    // runtime resize of the window, then back to the internal size
    replayer.set_window_size(Some((800, 600)));
    assert_eq!(replayer.output_size(), (1280, 960));
    assert_eq!(replayer.window_size(), Some((800, 600)));
    replayer.run(&stream);
    let small = replayer.frames.last().unwrap();
    assert_eq!((small.width, small.height), (800, 600));
    replayer.set_window_size(None);
    replayer.run(&stream);
    let internal = replayer.frames.last().unwrap();
    assert_eq!((internal.width, internal.height), (1280, 960));
}
