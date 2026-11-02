// SPDX-License-Identifier: GPL-3.0-or-later
//! Native object submission into the actual threaded GPU runtime.
//! FABRICATED shader/geometry/clip inputs; not gameplay Tier 2 evidence.
use tsfp_engine_anim::display::Sampling;
use tsfp_engine_anim::display_scene::{AnimationScene, ObjectId, ViewObject};
use tsfp_engine_anim::presentation::{ModelCorrection, Reflection};
use tsfp_rt_host::{Options, Runtime};
use tsfp_wgpu::animation::{Geometry, MatrixLayout, NodeBinding};
use tsfp_wgpu::live::LiveConfig;
#[path = "../../tsfp-wgpu/tests/support/animation_fixture.rs"]
mod fixture;
use fixture::{Source, scene, setup};

fn view() -> ViewObject {
    ViewObject {
        object: ObjectId(1),
        reflection: Reflection::None,
        camera: None,
        correction: ModelCorrection::None,
        use_palette: true,
    }
}
fn bindings() -> [NodeBinding; 1] {
    [NodeBinding {
        node: 0,
        first_register: 9,
        vectors: 4,
        layout: MatrixLayout::Rows,
    }]
}
fn geometry() -> Geometry {
    Geometry::Vertices {
        primitive: 5,
        start: 0,
        count: 3,
    }
}
fn draw(scene: &AnimationScene, alpha: f32) -> tsfp_rt_host::PublishedFrame {
    let rt = Runtime::start(Options {
        live: LiveConfig::software(),
        quiet: true,
        publish_frames: true,
        ..Options::default()
    })
    .unwrap();
    let mut bytes = Vec::new();
    for event in setup().events {
        tsfp_wgpu::binary::encode_event(&mut bytes, &event, 0).unwrap();
    }
    rt.write_binary(&bytes, 0);
    rt.draw_animated(scene, view(), alpha, &Source, &bindings(), geometry())
        .unwrap();
    rt.present(1);
    let stats = rt.finish();
    assert!(!stats.render_error && !stats.gave_up, "{stats:?}");
    let frame = rt.take_frame().unwrap();
    assert!(
        rt.draw_animated(scene, view(), alpha, &Source, &bindings(), geometry())
            .is_err(),
        "closed queue must not acknowledge another native draw"
    );
    frame
}
#[test]
fn fractional_native_draw_crosses_thread_queue_without_simulation_mutation() {
    let mut scene = scene();
    let before = scene
        .object(ObjectId(1))
        .unwrap()
        .animation()
        .ticks()
        .1
        .clone();
    let previous_before = scene
        .object(ObjectId(1))
        .unwrap()
        .animation()
        .ticks()
        .0
        .clone();
    let fixed = draw(&scene, 0.5);
    assert_eq!(draw(&scene, 0.75).rgba, fixed.rgba);
    scene.set_sampling(Sampling::Fractional);
    assert_eq!(draw(&scene, 0.0).rgba, fixed.rgba);
    let half = draw(&scene, 0.5);
    assert_ne!(half.rgba, fixed.rgba);
    let pixel = |frame: &tsfp_rt_host::PublishedFrame, x: usize, y: usize| {
        let at = (y * frame.width as usize + x) * 4;
        frame.rgba[at..at + 3].to_vec()
    };
    assert_eq!(pixel(&fixed, 110, 110), [255, 255, 255]);
    assert_eq!(pixel(&half, 110, 110), [0, 0, 0]);
    assert_eq!(pixel(&half, 110, 170), [255, 255, 255]);
    assert_eq!(
        scene.object(ObjectId(1)).unwrap().animation().ticks().1,
        &before
    );
    assert_eq!(
        scene.object(ObjectId(1)).unwrap().animation().ticks().0,
        &previous_before
    );
}

#[test]
fn invalid_native_draw_never_enqueues_a_partial_palette() {
    let scene = scene();
    let rt = Runtime::start(Options {
        live: LiveConfig::software(),
        quiet: true,
        publish_frames: true,
        ..Options::default()
    })
    .unwrap();
    let before = rt.stats().batches_sent;
    let mut bad = bindings();
    bad[0].node = 2;
    assert!(
        rt.draw_animated(&scene, view(), 0.5, &Source, &bad, geometry())
            .is_err()
    );
    let mut missing = view();
    missing.object = ObjectId(999);
    assert!(
        rt.draw_animated(&scene, missing, 0.5, &Source, &bindings(), geometry())
            .is_err()
    );
    assert!(
        rt.draw_animated(&scene, view(), f32::NAN, &Source, &bindings(), geometry())
            .is_err()
    );
    rt.flush();
    let stats = rt.finish();
    assert_eq!(
        stats.batches_sent,
        before + 1,
        "finish emits its existing empty text batch only"
    );
    assert_eq!(stats.presents, 0);
    assert!(!stats.render_error);
}
