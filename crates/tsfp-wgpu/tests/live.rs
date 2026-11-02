// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024 slice 7: the live renderer against the offline replay. Needs a software adapter, else the GPU tests print SKIPPED.
use tsfp_wgpu::context::{GpuContext, Preference};
use tsfp_wgpu::live::{DumpPlan, LiveConfig, LiveDevice, LiveRenderer};
use tsfp_wgpu::replay::{Frame, Replayer};
use tsfp_wgpu::scenes::{Rig, build_title_scene, title_scene};
use tsfp_wgpu::stream::{Event, Stream};

fn offline(stream: &Stream) -> Option<Vec<Frame>> {
    let gpu = GpuContext::new(&Preference::Software).ok()?;
    let mut replayer = Replayer::new(gpu, 640, 480);
    replayer.run(stream);
    Some(replayer.frames)
}

fn identical(a: &Frame, b: &Frame) {
    assert_eq!((a.width, a.height), (b.width, b.height));
    assert_eq!(a.hash(), b.hash(), "live and offline frames differ");
    assert_eq!(a.rgba, b.rgba);
}

#[test]
fn render_code_driving_the_live_device_matches_the_offline_replay() {
    let Some(frames) = offline(&title_scene()) else {
        println!("SKIPPED (no software adapter)");
        return;
    };
    let live = LiveDevice::new(LiveConfig::software()).expect("live device");
    let mut rig = Rig::on(live.recorder());
    build_title_scene(&mut rig);
    assert_eq!(live.take_error(), None);
    live.with_renderer(|renderer| {
        assert_eq!(renderer.presents(), 1);
        identical(renderer.last_frame().expect("a frame"), &frames[0]);
    });
}

#[test]
fn text_lines_fed_one_by_one_match_the_offline_replay() {
    let stream = title_scene();
    let Some(frames) = offline(&stream) else {
        println!("SKIPPED (no software adapter)");
        return;
    };
    let mut live = LiveRenderer::new(LiveConfig::software()).expect("live renderer");
    for line in stream.to_text().lines() {
        live.feed_line(line).expect("line");
    }
    identical(live.last_frame().expect("a frame"), &frames[0]);
}

#[test]
fn present_numbers_follow_the_host_counter_and_the_dump_plan() {
    let directory = std::env::temp_dir().join(format!("tsfp-wgpu-live-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let base = directory.join("shot");
    let config = LiveConfig {
        dump: DumpPlan {
            base: Some(base),
            at: [300u64].into_iter().collect(),
            every: false,
        },
        ..LiveConfig::software()
    };
    let Ok(live) = LiveDevice::new(config) else {
        println!("SKIPPED (no software adapter)");
        return;
    };
    let mut rig = Rig::on(live.recorder());
    build_title_scene(&mut rig);
    live.present(Some(299));
    live.present(Some(300));
    live.present(None);
    assert_eq!(live.take_error(), None);
    live.with_renderer(|renderer| assert_eq!(renderer.presents(), 301));
    let mut names: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    // The scene's own present (1), 299 and 301 are not dumped, 300 is.
    assert_eq!(names, ["shot.p300.png"]);
    let png = std::fs::read(directory.join("shot.p300.png")).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn first_class_inputs_reach_the_state_machine_like_trace_lines() {
    let Ok(live) = LiveDevice::new(LiveConfig::software()) else {
        println!("SKIPPED (no software adapter)");
        return;
    };
    live.vertex_constants(10, &[0, 0x3F80_0000, 0x3F00_0000, 0]);
    live.lib_method(0x30C, 1);
    live.surface(1, 2, 0x100, 0, 0x1DF_27F, 0x500, 0x200, 1);
    live.with_renderer(|renderer| {
        assert_eq!(renderer.replayer.lib_regs.get(&0x30C), Some(&1));
    });
    // The text form of the same inputs parses to the events the typed API emits.
    let lines = [
        "call SetVertexConstants a 0 3f800000 3f000000 0",
        "lib 30c 1",
        "call SetSurface 1 2 100 0 1df27f 500 200 1",
    ];
    let events: Vec<Event> = lines
        .iter()
        .map(|line| Stream::parse_line(0, line).unwrap().unwrap())
        .collect();
    assert!(matches!(
        events[1],
        Event::Lib {
            method: 0x30C,
            value: 1
        }
    ));
    assert!(matches!(
        events[2],
        Event::Surface {
            data: 0x100,
            flags: 1,
            ..
        }
    ));
}

#[test]
fn idle_poll_observes_no_readback_completion_without_a_present() {
    let mut live = LiveRenderer::new(LiveConfig::software()).expect("lavapipe adapter");
    live.replayer.readback = false;
    for event in &title_scene().events {
        live.feed(event).expect("event");
    }
    let presents = live.presents();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while live.replayer.timings.gpu_latency_frames == 0 {
        live.poll_readbacks().expect("nonblocking observation");
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(live.presents(), presents);
    assert_eq!(live.replayer.timings.gpu_latency_frames, 1);
    assert!(!live.replayer.gpu_work_pending());
    assert!(live.replayer.timings.gpu_latency_ns > 0);
    assert!(
        live.last_frame()
            .expect("metadata-only frame")
            .rgba
            .is_empty()
    );
}
