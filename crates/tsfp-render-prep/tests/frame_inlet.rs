// SPDX-License-Identifier: GPL-3.0-or-later
//! FABRICATED fixtures: contract evidence only, not original-preset Tier 2.
use tsfp_engine_render::{
    camera::{CameraPresentation, CameraProjection},
    native_scene::NativeSceneOwner,
    scene::{
        render_prep::{Sampling, SceneError},
        scene_collect::CollectError,
        visibility::{
            ProjectionViewport, Rect, Room, RoomCoverage, VisibilityError, VisibilityOutput,
            VisibilityScene, VisibilityView,
        },
    },
    spatial::Bounds,
};
use tsfp_render_prep::{FrameError, FrameSample, FrameViews, OrderedRoom, ViewInput};

const RECT: Rect = Rect {
    left: 0,
    right: 640,
    top: 0,
    bottom: 480,
};
const MATRIX: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

struct Fixture {
    scene: VisibilityScene,
    initial: VisibilityOutput,
    order: Vec<OrderedRoom>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            scene: VisibilityScene {
                rooms: vec![
                    Room {
                        portals: vec![],
                        crossing_suppressed: false
                    };
                    4
                ],
                portals: vec![],
            },
            initial: VisibilityOutput {
                rooms: vec![
                    RoomCoverage {
                        depth: 0,
                        rect: RECT,
                    },
                    RoomCoverage {
                        depth: 0,
                        rect: RECT,
                    },
                    RoomCoverage {
                        depth: 1,
                        rect: RECT,
                    },
                    RoomCoverage {
                        depth: -1,
                        rect: RECT,
                    },
                ],
                portals: vec![],
                portal_visits: vec![],
                max_depth: 1,
                active_rooms: vec![false; 4],
            },
            order: vec![
                OrderedRoom {
                    room: 2,
                    after_ordinary: true,
                },
                OrderedRoom {
                    room: 1,
                    after_ordinary: false,
                },
                OrderedRoom {
                    room: 3,
                    after_ordinary: false,
                },
                OrderedRoom {
                    room: 0,
                    after_ordinary: false,
                },
            ],
        }
    }

    fn input(&self, sample: FrameSample, viewer: u32) -> ViewInput<'_> {
        ViewInput {
            sample,
            camera: CameraPresentation {
                eye: [0.; 3],
                view: MATRIX,
                projection: CameraProjection {
                    matrix: MATRIX,
                    tangent_extents: [1., 1.],
                },
                visible_rooms: [0, 0],
            },
            scene: &self.scene,
            visibility: VisibilityView {
                position: [0.; 3],
                matrix: MATRIX,
                viewport: ProjectionViewport {
                    x_offset: 320.,
                    x_scale: 320.,
                    y_offset: 240.,
                    y_scale: 240.,
                },
                full_rect: RECT,
                viewer,
                near_portal_distance: 0.,
                clip_groups: vec![],
                mapmaker: false,
            },
            start_room: 0,
            start_depth: 0,
            initial: &self.initial,
            room_order: &self.order,
            query: Bounds {
                min: [-10.; 3],
                max: [10.; 3],
            },
            planes: vec![],
            scene_light_view: None,
        }
    }
}

fn sample() -> FrameSample {
    FrameSample {
        tick: 7,
        alpha: 0.25,
        sampling: Sampling::BetweenTicks,
    }
}

#[test]
fn complete_views_reach_owner_and_preserve_submission_order() {
    let fixture = Fixture::new();
    let original_initial = fixture.initial.clone();
    let mut owner = NativeSceneOwner::default();
    owner.finish_tick(6).unwrap();
    owner.finish_tick(7).unwrap();
    let views = FrameViews::calculate(
        sample(),
        &[fixture.input(sample(), 3), fixture.input(sample(), 1)],
    )
    .unwrap();
    let a = views.prepare(&owner).unwrap();
    let b = views.prepare(&owner).unwrap();
    assert_eq!(a.views.len(), 2);
    assert_eq!(a.views, b.views);
    assert_ne!(a.resource_epoch, b.resource_epoch);
    assert_eq!(a.alpha, 0.25);
    assert_eq!(a.tick, 7);
    assert_eq!(owner.current_tick(), Some(7));
    assert_eq!(fixture.initial, original_initial);
    for (view, viewer) in a.views.iter().zip([3, 1]) {
        assert_eq!(view.target.unwrap().viewer, viewer);
        assert_eq!(view.target.unwrap().rectangle, RECT);
        assert_eq!(view.camera.unwrap().view, MATRIX);
        assert_eq!(
            view.view.visible_rooms.iter().copied().collect::<Vec<_>>(),
            [0, 1, 2]
        );
        let order = view.room_order.as_ref().unwrap();
        assert_eq!(order.buckets[0], [1, 0]);
        assert_eq!(order.buckets[2], [2]);
        assert_eq!(order.draw_max_depth, 2);
        assert_eq!(order.restore_max_depth, 1);
    }
}

#[test]
fn original_frame_uses_fixed_alpha_zero_contract() {
    let fixture = Fixture::new();
    let sample = FrameSample {
        alpha: 0.,
        sampling: Sampling::Fixed,
        ..sample()
    };
    let mut owner = NativeSceneOwner::default();
    owner.finish_tick(sample.tick).unwrap();
    let views = FrameViews::calculate(sample, &[fixture.input(sample, 0)]).unwrap();
    let frame = views.prepare(&owner).unwrap();
    assert_eq!(frame.sampling, Sampling::Fixed);
    assert_eq!(frame.alpha, 0.);
    assert_eq!(frame.views.len(), 1);
}

#[test]
fn empty_or_duplicate_views_fail_atomically() {
    let fixture = Fixture::new();
    assert!(matches!(
        FrameViews::calculate(sample(), &[]),
        Err(FrameError::MissingViews)
    ));
    assert!(matches!(
        FrameViews::calculate(
            sample(),
            &[fixture.input(sample(), 2), fixture.input(sample(), 2)]
        ),
        Err(FrameError::DuplicateViewer)
    ));
}

#[test]
fn mixed_sample_provenance_and_stale_scene_are_rejected() {
    let fixture = Fixture::new();
    for wrong in [
        FrameSample {
            tick: 6,
            ..sample()
        },
        FrameSample {
            alpha: 0.75,
            ..sample()
        },
        FrameSample {
            sampling: Sampling::Fixed,
            ..sample()
        },
    ] {
        assert!(matches!(
            FrameViews::calculate(
                sample(),
                &[fixture.input(sample(), 0), fixture.input(wrong, 1)]
            ),
            Err(FrameError::SampleMismatch)
        ));
    }
    let views = FrameViews::calculate(sample(), &[fixture.input(sample(), 0)]).unwrap();
    let mut owner = NativeSceneOwner::default();
    assert_eq!(views.prepare(&owner), Err(FrameError::SampleMismatch));
    owner.finish_tick(7).unwrap();
    assert!(views.prepare(&owner).is_ok());
    owner.finish_tick(8).unwrap();
    assert_eq!(views.prepare(&owner), Err(FrameError::SampleMismatch));
}

#[test]
fn mismatched_camera_cannot_supply_portal_results() {
    let fixture = Fixture::new();
    let mut input = fixture.input(sample(), 0);
    input.camera.eye[0] = 1.;
    assert!(matches!(
        FrameViews::calculate(sample(), &[input]),
        Err(FrameError::Scene(SceneError::CameraEyeMismatch))
    ));
    let mut input = fixture.input(sample(), 0);
    input.camera.view[0] = 2.;
    assert!(matches!(
        FrameViews::calculate(sample(), &[input]),
        Err(FrameError::Scene(SceneError::CameraMatrixMismatch))
    ));
}

#[test]
fn incomplete_duplicate_and_foreign_room_orders_are_rejected() {
    for ids in [vec![0, 1, 2], vec![0, 1, 2, 2], vec![0, 1, 2, 4]] {
        let mut fixture = Fixture::new();
        fixture.order = ids
            .into_iter()
            .map(|room| OrderedRoom {
                room,
                after_ordinary: false,
            })
            .collect();
        assert!(matches!(
            FrameViews::calculate(sample(), &[fixture.input(sample(), 0)]),
            Err(FrameError::IncompleteRoomOrder)
        ));
    }
}

#[test]
fn invalid_fractions_and_nonzero_original_alpha_are_rejected() {
    let fixture = Fixture::new();
    for alpha in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        let sample = FrameSample { alpha, ..sample() };
        assert!(matches!(
            FrameViews::calculate(sample, &[fixture.input(sample, 0)]),
            Err(FrameError::InvalidSample)
        ));
    }
    let sample = FrameSample {
        sampling: Sampling::Fixed,
        ..sample()
    };
    assert!(matches!(
        FrameViews::calculate(sample, &[fixture.input(sample, 0)]),
        Err(FrameError::InvalidSample)
    ));
}

#[test]
fn traversal_and_room_collection_errors_are_not_partial_frames() {
    let fixture = Fixture::new();
    assert!(matches!(
        FrameViews::calculate(
            sample(),
            &[fixture.input(sample(), 0), fixture.input(sample(), 16)]
        ),
        Err(FrameError::Visibility(VisibilityError::InvalidViewer))
    ));
    let mut input = fixture.input(sample(), 0);
    input.start_room = 4;
    assert!(matches!(
        FrameViews::calculate(sample(), &[input]),
        Err(FrameError::Visibility(VisibilityError::InvalidRoom))
    ));
    let mut input = fixture.input(sample(), 0);
    input.start_depth = 80;
    assert!(matches!(
        FrameViews::calculate(sample(), &[input]),
        Err(FrameError::Rooms(CollectError::InvalidDepth))
    ));
}
