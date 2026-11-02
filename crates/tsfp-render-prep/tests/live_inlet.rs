// SPDX-License-Identifier: GPL-3.0-or-later
//! FABRICATED contract fixtures, not Tier 2 original evidence.
use std::collections::BTreeMap;
use tsfp_engine_render::{native_scene::NativeSceneOwner, scene::render_prep::Sampling};
use tsfp_render_prep::live::{LiveError, enabled, prepare_bridge_frame};
use tsfp_render_prep_wire::{CameraSample, NativeSceneView, copy_collector_view};

const MATRIX: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

fn producer() -> (BTreeMap<u32, u32>, CameraSample) {
    let mut memory = BTreeMap::from([
        (0x7b975c, 3),
        (0x708788, 0x3000),
        (0x120c, 0x2000),
        (0x1214, 2),
        (0x1d28, 1),
        (0x4000, 320),
        (0x4004, 640),
        (0x4014, 10),
        (0x4018, 230),
        (0x4008, 480),
        (0x4010, 160),
        (0x401c, 120),
        (0x4024, 110),
    ]);
    for (i, value) in MATRIX.iter().enumerate() {
        memory.insert(0x2000 + i as u32 * 4, value.to_bits());
    }
    for (room, depth, flags) in [(1, 0, 0), (2, 1, 8), (3, 0, 0)] {
        let slot = 0x1000 + 0x3bc + room * 12;
        memory.insert(slot, depth);
        memory.insert(slot + 4, 320 | (10 << 16));
        memory.insert(slot + 8, 640 | (230 << 16));
        memory.insert(0x3000 + room * 0xb4 + 0xb0, flags << 16);
    }
    (
        memory,
        CameraSample {
            eye: [0.; 3],
            view: MATRIX,
            projection: MATRIX,
            tangent_extents: [1.; 2],
            visible_rooms: [1, 1],
        },
    )
}

fn packet() -> NativeSceneView {
    let (memory, camera) = producer();
    copy_collector_view(
        |address| memory.get(&address).copied().unwrap_or(0),
        0x1000,
        0x4000,
        Some(camera),
    )
    .unwrap()
}

fn owner() -> NativeSceneOwner {
    let mut owner = NativeSceneOwner::default();
    owner.finish_tick(6).unwrap();
    owner.publish_room_plan(7, &[1, 3, 2], 2).unwrap();
    owner.finish_tick(7).unwrap();
    owner
}

#[test]
fn live_producer_reaches_prepared_camera_without_advancing_history() {
    let packet = packet();
    let owner = owner();
    let a = prepare_bridge_frame(&owner, 7, &packet, true).unwrap();
    let b = prepare_bridge_frame(&owner, 7, &packet, true).unwrap();
    assert_eq!(a.views.len(), 1);
    assert_eq!(a.views, b.views);
    assert_ne!(a.resource_epoch, b.resource_epoch);
    assert_eq!(owner.current_tick(), Some(7));
    assert_eq!(a.sampling, Sampling::Fixed);
    assert_eq!(a.alpha, 0.);
    let view = &a.views[0];
    assert_eq!(view.camera.unwrap().projection.matrix, MATRIX);
    assert_eq!(view.target.unwrap().viewer, 2);
    let target = view.target.unwrap();
    assert_eq!(
        [
            target.rectangle.left,
            target.rectangle.top,
            target.rectangle.right,
            target.rectangle.bottom
        ],
        [320, 10, 640, 230]
    );
    assert_eq!(target.viewport.x_offset, 480.);
    assert_eq!(view.room_order.as_ref().unwrap().buckets[0], [1, 3]);
    assert_eq!(view.room_order.as_ref().unwrap().buckets[2], [2]);
    assert!(
        view.visibility.is_none(),
        "room coverage is not a portal rerun"
    );
}

#[test]
fn disabled_inlet_preserves_existing_audit_mode() {
    for value in [None, Some("0"), Some("true"), Some("2")] {
        assert!(!enabled(value));
    }
    assert!(enabled(Some("1")));
    let frame = prepare_bridge_frame(&owner(), 7, &NativeSceneView::default(), false).unwrap();
    assert!(frame.views.is_empty());
}

#[test]
fn missing_stale_or_mismatched_camera_fails_closed() {
    let owner = owner();
    let mut input = packet();
    assert_eq!(
        prepare_bridge_frame(&owner, 6, &input, true),
        Err(LiveError::StalePublication)
    );
    input.camera_valid = 0;
    assert_eq!(
        prepare_bridge_frame(&owner, 7, &input, true),
        Err(LiveError::MissingCamera)
    );
    input.camera_valid = 1;
    input.camera.eye[0] = 8.;
    assert_eq!(
        prepare_bridge_frame(&owner, 7, &input, true),
        Err(LiveError::CameraMismatch)
    );
    input.camera.eye[0] = 0.;
    input.camera.view[12] = 8.;
    assert_eq!(
        prepare_bridge_frame(&owner, 7, &input, true),
        Err(LiveError::CameraMismatch)
    );
}

#[test]
fn collector_matrix_is_view_times_projection_and_mutations_fail() {
    let owner = owner();
    let mut input = packet();
    input.camera.view[12] = 2.;
    input.camera.projection[0] = 3.;
    input.camera.projection[5] = 4.;
    input.matrix = [
        3., 0., 0., 0., 0., 4., 0., 0., 0., 0., 1., 0., 6., 0., 0., 1.,
    ];
    assert!(prepare_bridge_frame(&owner, 7, &input, true).is_ok());
    input.matrix[12] += 0.01;
    assert_eq!(
        prepare_bridge_frame(&owner, 7, &input, true),
        Err(LiveError::CameraMismatch)
    );
    input.matrix[12] -= 0.01;
    input.camera.projection[0] = 2.;
    assert_eq!(
        prepare_bridge_frame(&owner, 7, &input, true),
        Err(LiveError::CameraMismatch)
    );
}

#[test]
fn order_depth_and_numeric_mutations_are_rejected() {
    let owner = owner();
    let mut input = packet();
    input.coverage[1].after_ordinary = 8;
    assert_eq!(
        prepare_bridge_frame(&owner, 7, &input, true),
        Err(LiveError::RoomPlanMismatch)
    );
    for mutate in [
        |p: &mut NativeSceneView| p.viewer = 16,
        |p: &mut NativeSceneView| p.room_count = 201,
        |p: &mut NativeSceneView| p.camera.projection[0] = f32::NAN,
        |p: &mut NativeSceneView| p.camera.tangent_extents[0] = 0.,
        |p: &mut NativeSceneView| p.viewport[1] = 0.,
        |p: &mut NativeSceneView| p.rectangle[2] = 0,
        |p: &mut NativeSceneView| p.camera.visible_rooms[1] = 4,
    ] {
        let mut input = packet();
        mutate(&mut input);
        assert_eq!(
            prepare_bridge_frame(&owner, 7, &input, true),
            Err(LiveError::InvalidView)
        );
    }
}

#[test]
fn producer_copies_packed_signed_coverage_and_does_not_invent_camera() {
    let (mut memory, _) = producer();
    memory.insert(0x1000 + 0x3bc + 12 + 4, (-5i16 as u16 as u32) | (10 << 16));
    let saved = memory.clone();
    let read = |address| memory.get(&address).copied().unwrap_or(0);
    let packet = copy_collector_view(read, 0x1000, 0x4000, None).unwrap();
    assert_eq!(packet.camera_valid, 0);
    assert_eq!(packet.coverage[1].rectangle, [-5, 10, 640, 230]);
    assert_eq!(packet.coverage[2].after_ordinary, 8);
    assert_eq!(memory, saved);
    assert!(copy_collector_view(read, 0, 0x4000, None).is_none());
    memory.insert(0x7b975c, 201);
    assert!(
        copy_collector_view(
            |address| memory.get(&address).copied().unwrap_or(0),
            0x1000,
            0x4000,
            None
        )
        .is_none()
    );
}

#[test]
fn wire_layout_matches_c_mailbox_contract() {
    use std::mem::{offset_of, size_of};
    assert_eq!(size_of::<CameraSample>(), 156);
    assert_eq!(offset_of!(NativeSceneView, camera_valid), 112);
    assert_eq!(offset_of!(NativeSceneView, camera), 116);
    assert_eq!(offset_of!(NativeSceneView, coverage), 280);
    assert_eq!(size_of::<NativeSceneView>(), 5104);
}

#[test]
fn room_only_inlet_cannot_silently_drop_native_objects() {
    use tsfp_state::{
        GuestMemory, generated_views::TsfpObjectView, object_pool::*,
        scene_membership::RenderComponents, transforms::WorldTransform,
    };
    struct Image(BTreeMap<u32, u8>);
    impl GuestMemory for Image {
        fn read8(&self, address: u32) -> u8 {
            *self.0.get(&address).unwrap_or(&0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }
    let records = 0x4300_0000;
    let arena = 0x4400_0000;
    let mut memory = Image(BTreeMap::new());
    for (address, value) in [
        (SCALARS[S_CAPACITY].0, 1),
        (SCALARS[S_RECORDS_BASE].0, records),
        (SCALARS[S_USED_RECORDS].0, 1),
        (SCALARS[S_ARENA_BASE].0, arena),
        (SCALARS[S_ARENA_USED].0, 1),
        (cell_addr(CAPS, 1), 1),
        (cell_addr(COUNTS, 1), 1),
        (cell_addr(MEMBERS, 1), arena),
        (arena, records),
        (records + TsfpObjectView::OFFSET_MODEL_ID, 9),
    ] {
        memory.write32(address, value);
    }
    let mut owner = NativeSceneOwner::default();
    owner.pool = ObjectPool::load(&memory);
    let id = owner.spawn(7).unwrap();
    owner.attach_room(id, 0, 0x900, 9, 1, true, false).unwrap();
    owner
        .set_render_state(
            id,
            RenderComponents {
                placement: WorldTransform {
                    position: [0.; 3],
                    orientation: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                },
                bounds: [[-1.; 3], [1.; 3]],
                motion_bounds: [[-1.; 3], [1.; 3]],
                scissor: [320., 10., 640., 230.],
                scissor_rejected: false,
                model_hidden: false,
                overlay: 0,
                discontinuity: false,
            },
        )
        .unwrap();
    owner.publish_room_plan(7, &[1, 3, 2], 2).unwrap();
    owner.finish_tick(7).unwrap();
    assert_eq!(owner.object_count(), Some(1));
    assert_eq!(
        prepare_bridge_frame(&owner, 7, &packet(), true),
        Err(LiveError::ObjectInputsRequired)
    );
    assert_eq!(owner.object_count(), Some(1));
    assert_eq!(owner.current_tick(), Some(7));
}
