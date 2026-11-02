//! T2241 FABRICATED native object -> fractional palette -> live GPU draw.
//! No retail bytes; this is a renderer integration test, not gameplay Tier 2.
use tsfp_engine_anim::display::Sampling;
use tsfp_engine_anim::display_scene::{AnimationScene, ObjectId, ViewObject};
use tsfp_engine_anim::presentation::{ModelCorrection, Reflection};
use tsfp_wgpu::animation::{MatrixLayout, NodeBinding};
use tsfp_wgpu::live::{LiveConfig, LiveRenderer};
use tsfp_wgpu::stream::Event;
#[path = "support/animation_fixture.rs"]
mod fixture;
use fixture::{Source, scene, setup};

fn draw(live: &mut LiveRenderer, scene: &AnimationScene, alpha: f32) -> u64 {
    for event in &setup().events {
        live.feed(event).unwrap();
    }
    let view = ViewObject {
        object: ObjectId(1),
        reflection: Reflection::None,
        camera: None,
        correction: ModelCorrection::None,
        use_palette: true,
    };
    live.animated_object(
        scene,
        view,
        alpha,
        &Source,
        &[NodeBinding {
            node: 0,
            first_register: 9,
            vectors: 4,
            layout: MatrixLayout::Rows,
        }],
    )
    .unwrap();
    live.feed(&Event::Call(
        tsfp_engine_render::device::DeviceCall::DrawVertices {
            primitive: 5,
            start: 0,
            count: 3,
        },
    ))
    .unwrap();
    live.feed(&Event::Present).unwrap();
    live.last_frame().unwrap().hash()
}
#[test]
fn opt_in_fractional_pose_reaches_actual_live_draw_without_state_writes() {
    let Ok(mut live) = LiveRenderer::new(LiveConfig::software()) else {
        panic!("T2241 integration needs a software GPU adapter");
    };
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
    let fixed = draw(&mut live, &scene, 0.5);
    let fixed_pixel = live.last_frame().unwrap().pixel(110, 110);
    assert_eq!(&fixed_pixel[..3], &[255, 255, 255]);
    assert_eq!(draw(&mut live, &scene, 0.75), fixed);
    scene.set_sampling(Sampling::Fractional);
    assert_eq!(draw(&mut live, &scene, 0.0), fixed);
    let quarter = draw(&mut live, &scene, 0.25);
    let half = draw(&mut live, &scene, 0.5);
    assert_ne!(quarter, fixed);
    assert_ne!(half, quarter);
    let moved = live.last_frame().unwrap().pixel(110, 110);
    assert_eq!(&moved[..3], &[0, 0, 0]);
    let inside = live.last_frame().unwrap().pixel(110, 170);
    assert_eq!(&inside[..3], &[255, 255, 255]);
    assert_eq!(
        scene.object(ObjectId(1)).unwrap().animation().ticks().1,
        &before
    );
    assert_eq!(
        scene.object(ObjectId(1)).unwrap().animation().ticks().0,
        &previous_before
    );
}
