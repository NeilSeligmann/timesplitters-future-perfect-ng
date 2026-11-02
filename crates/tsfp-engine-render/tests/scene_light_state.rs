// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 fabricated-input differential for the T1700 owned light block.
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use tsfp_engine_render::{
    native_scene::{NativeSceneOwner, PreparedSceneLease},
    scene::{
        light_state::*,
        render_prep::{PreparedView, View},
    },
    scene_batch, scene_t1938,
    spatial::Bounds,
};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
};

const RECORD: u32 = 0x84000;
const COLORS: u32 = 0x90000;
const COLOR_VALUES: u32 = 0x91000;
const POINTS: u32 = 0x92000;
const POINT_VALUES: u32 = 0x93000;
const SCRATCH: u32 = 0xA0000;
const CAMERA: u32 = 0xB0000;
const MATRIX: u32 = 0xB1000;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<BTreeMap<u32, u8>>>);

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        *self.0.borrow().get(&address).unwrap_or(&0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
}

struct Calls(Memory);

impl GuestCall for Calls {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        regs.esp = regs.esp.wrapping_sub(4);
        self.0.write32(regs.esp, 0x1234);
        match va {
            0x3e970 => {
                assert_eq!(self.0.read32(regs.esp + 4), 0x80);
                regs.eax = SCRATCH;
            }
            0x18e910 => {
                scene_batch::game_lighting_light_slot_fill_view_space_position_and_color_or_defaults(
                    &mut self.0.clone(),
                    self,
                    regs,
                )
            }
            0x151fc0 => tsfp_ports::c2r::t1937_math::game_matrix4x4_transform_point_vec3_to_vec4(
                &mut self.0,
                regs,
            ),
            _ => panic!("unexpected lifted call {va:#x}"),
        }
        regs.esp = regs.esp.wrapping_add(4);
    }
}

fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: 0x10000,
        ebx: 1,
        ebp: 2,
        esi: 3,
        edi: 4,
        eflags: 0x202,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x37f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

const VIEW_MATRIX: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, -3.0, 5.0, 1.0,
];

fn transform(point: WorldPoint) -> ViewPoint {
    transform_world_point_to_view(VIEW_MATRIX, point)
}

fn lifted_block(inputs: SceneLightInputs) -> [[u32; 10]; 3] {
    let mut memory = Memory::default();
    memory.write32(0x0047_5CAC, 0.0_f32.to_bits());
    memory.write32(0x0047_5CD4, 0.5_f32.to_bits());
    memory.write32(0x0047_5C78, 1.0_f32.to_bits());
    memory.write32(0x007B_0CC0, CAMERA);
    memory.write32(CAMERA + 0x208, MATRIX);
    for (index, value) in VIEW_MATRIX.iter().enumerate() {
        memory.write32(MATRIX + index as u32 * 4, value.to_bits());
    }
    for (index, color) in inputs.colors.iter().enumerate() {
        let pointer = COLOR_VALUES + index as u32 * 0x20;
        memory.write32(COLORS + index as u32 * 4, pointer);
        for (component, value) in color.0.iter().enumerate() {
            memory.write32(pointer + component as u32 * 4, value.to_bits());
        }
    }
    for (index, point) in inputs.world_points.iter().enumerate() {
        let pointer = POINT_VALUES + index as u32 * 0x20;
        memory.write32(POINTS + index as u32 * 4, pointer);
        for (component, value) in point.0.iter().enumerate() {
            memory.write32(pointer + component as u32 * 4, value.to_bits());
        }
    }

    let mut regs = registers();
    for (offset, value) in [(4, RECORD), (8, 0), (12, 0), (16, COLORS), (20, POINTS)] {
        memory.write32(regs.esp + offset, value);
    }
    let mut calls = Calls(memory.clone());
    scene_t1938::game_lighting_build_three_light_slots_from_direction_and_color_arrays(
        &mut memory.clone(),
        &mut calls,
        &mut regs,
    );
    assert_eq!(memory.read32(RECORD + 0x78), SCRATCH);
    let slots = core::array::from_fn(|slot| {
        core::array::from_fn(|word| memory.read32(SCRATCH + slot as u32 * 0x28 + word as u32 * 4))
    });
    for component in 0..3 {
        assert_eq!(
            memory.read32(RECORD + 0x6c + component as u32 * 4),
            inputs.colors[3].0[component].to_bits()
        );
    }
    slots
}

fn random_inputs(seed: &mut u32, trial: u32) -> SceneLightInputs {
    let mut next = || {
        *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (*seed % 4001) as i32 - 2000
    };
    let mut color = || {
        Color3([
            next() as f32 / 211.0,
            next() as f32 / 197.0,
            next() as f32 / 181.0,
        ])
    };
    let mut colors = [color(), color(), color(), color()];
    match trial % 8 {
        0 => colors[trial as usize % 3] = Color3([0.0, 0.0, 0.0]),
        1 => colors[trial as usize % 3] = Color3([f32::NAN, 0.0, 0.0]),
        _ => {}
    }
    let world_points = core::array::from_fn(|_| {
        WorldPoint([
            next() as f32 / 113.0,
            next() as f32 / 107.0,
            next() as f32 / 101.0,
        ])
    });
    SceneLightInputs {
        colors,
        world_points,
    }
}

#[test]
fn two_thousand_fabricated_inputs_match_lifted_light_publication() {
    let mut seed = 0x1700_2026;
    for trial in 0..2000 {
        let inputs = random_inputs(&mut seed, trial);
        let expected = lifted_block(inputs);
        let mut owner = SceneLightState::default();
        let record = owner.setup_record(inputs).unwrap();
        let view = owner.setup_view().unwrap();
        let actual = *owner
            .prepare_view(record, view, FrameResourceEpoch(7), transform)
            .unwrap();

        for (index, slot) in actual.slots.iter().enumerate() {
            let words = [
                slot.position.0[0].to_bits(),
                slot.position.0[1].to_bits(),
                slot.position.0[2].to_bits(),
                slot.position.0[3].to_bits(),
                slot.color.0[0].to_bits(),
                slot.color.0[1].to_bits(),
                slot.color.0[2].to_bits(),
                slot.parameters[0].to_bits(),
                slot.parameters[1].to_bits(),
                slot.parameters[2].to_bits(),
            ];
            assert_eq!(words, expected[index], "trial {trial}, slot {index}");
            let expected_enabled =
                expected[index][7..10] == [0.5_f32.to_bits(), 0.0_f32.to_bits(), 0.0_f32.to_bits()];
            assert_eq!(
                slot.enabled, expected_enabled,
                "trial {trial}, slot {index}"
            );
        }
        assert_eq!(actual.fourth_color, inputs.colors[3]);
    }
}

#[test]
fn setup_update_view_and_teardown_reject_stale_state() {
    let inputs = SceneLightInputs {
        colors: [Color3([1.0, 0.0, 0.0]); 4],
        world_points: [WorldPoint([0.0, 1.0, 2.0]); 3],
    };
    let mut owner = NativeSceneOwner::default();
    let record = owner.setup_scene_light_record(inputs).unwrap();
    let view = owner.setup_scene_light_view().unwrap();
    owner.teardown_scene_light_view(view).unwrap();
    assert_eq!(
        owner.scene_light_view_block(record, view),
        Err(SceneLightError::InvalidView)
    );
    let next_record = owner.setup_scene_light_record(inputs).unwrap();
    let next_view = owner.setup_scene_light_view().unwrap();
    assert_ne!(record, next_record);
    assert_ne!(view, next_view);
    owner.teardown_scene_light_record(record).unwrap();
    assert_eq!(
        owner.scene_light_view_block(record, next_view),
        Err(SceneLightError::InvalidRenderRecord)
    );
}

#[test]
fn batch_source_updates_are_atomic_and_invalidate_all_affected_views() {
    let old_a = SceneLightInputs {
        colors: [Color3([1.0, 0.0, 0.0]); 4],
        world_points: [WorldPoint([1.0, 2.0, 3.0]); 3],
    };
    let old_b = SceneLightInputs {
        colors: [Color3([0.0, 1.0, 0.0]); 4],
        world_points: [WorldPoint([4.0, 5.0, 6.0]); 3],
    };
    let new_a = SceneLightInputs {
        colors: [Color3([0.0, 0.0, 1.0]); 4],
        world_points: [WorldPoint([7.0, 8.0, 9.0]); 3],
    };
    let new_b = SceneLightInputs {
        colors: [Color3([1.0, 1.0, 0.0]); 4],
        world_points: [WorldPoint([10.0, 11.0, 12.0]); 3],
    };
    let mut state = SceneLightState::default();
    let record_a = state.setup_record(old_a).unwrap();
    let record_b = state.setup_record(old_b).unwrap();
    let stale_record = state.setup_record(old_a).unwrap();
    state.teardown_record(stale_record).unwrap();
    let view = state.setup_view().unwrap();
    let epoch = FrameResourceEpoch(1);
    state.set_publication_enabled(true);
    state
        .prepare_view(record_a, view, epoch, transform)
        .unwrap();
    state
        .prepare_view(record_b, view, epoch, transform)
        .unwrap();
    let prior = state
        .publish_frame(
            epoch,
            &[
                SceneLightBinding::new(record_a, view),
                SceneLightBinding::new(record_b, view),
            ],
        )
        .unwrap()
        .clone();

    assert_eq!(state.update_records(&[]), Ok(()));
    assert_eq!(state.published_frame(), Some(&prior));
    assert_eq!(
        state.update_records(&[
            SceneLightRecordUpdate::new(record_a, new_a),
            SceneLightRecordUpdate::new(stale_record, new_b),
        ]),
        Err(SceneLightError::InvalidRenderRecord)
    );
    assert_eq!(
        state.update_records(&[
            SceneLightRecordUpdate::new(record_a, new_a),
            SceneLightRecordUpdate::new(record_a, new_b),
        ]),
        Err(SceneLightError::DuplicateRecordUpdate)
    );
    assert_eq!(state.published_frame(), Some(&prior));
    assert_eq!(
        state.view_block(record_a, view).unwrap().unwrap().slots[0].color,
        old_a.colors[0]
    );

    state
        .update_records(&[
            SceneLightRecordUpdate::new(record_a, new_a),
            SceneLightRecordUpdate::new(record_b, new_b),
        ])
        .unwrap();
    assert!(state.published_frame().is_none());
    assert_eq!(state.view_block(record_a, view), Ok(None));
    assert_eq!(state.view_block(record_b, view), Ok(None));
    let next_epoch = FrameResourceEpoch(2);
    state
        .prepare_view(record_a, view, next_epoch, transform)
        .unwrap();
    state
        .prepare_view(record_b, view, next_epoch, transform)
        .unwrap();
    assert_eq!(
        state.view_block(record_a, view).unwrap().unwrap().slots[0].color,
        new_a.colors[0]
    );
    assert_eq!(
        state.view_block(record_b, view).unwrap().unwrap().slots[0].color,
        new_b.colors[0]
    );
}

#[test]
fn published_lights_require_complete_matching_epoch_and_attach_only_when_opted_in() {
    let inputs = SceneLightInputs {
        colors: [Color3([1.0, 0.25, 0.0]); 4],
        world_points: [WorldPoint([2.0, 3.0, 4.0]); 3],
    };
    let mut owner = NativeSceneOwner::default();
    owner.finish_tick(1).unwrap();
    let mut frame = owner.prepare_display_frame(&[], 0.0).unwrap();
    assert!(frame.scene_lights.is_none());
    let epoch = frame.resource_epoch.unwrap();
    let later_frame = owner.prepare_display_frame(&[], 0.5).unwrap();
    assert_ne!(later_frame.resource_epoch, Some(epoch));

    let record = owner.setup_scene_light_record(inputs).unwrap();
    let view = owner.setup_scene_light_view().unwrap();
    let unprepared_view = owner.setup_scene_light_view().unwrap();
    for scene_light_view in [Some(view), Some(unprepared_view)] {
        frame.views.push(PreparedView {
            tick: frame.tick,
            scene_light_view,
            view: View {
                query: Bounds {
                    min: [0.0; 3],
                    max: [1.0; 3],
                },
                planes: Vec::new(),
                visible_rooms: Default::default(),
            },
            camera: (scene_light_view == Some(view)).then_some(
                tsfp_ports::camera::CameraPresentation {
                    eye: [0.0; 3],
                    view: VIEW_MATRIX,
                    projection: tsfp_ports::camera::CameraProjection {
                        matrix: [0.0; 16],
                        tangent_extents: [0.0; 2],
                    },
                    visible_rooms: [-1; 2],
                },
            ),
            visibility: None,
            target: None,
            room_order: None,
            objects: Vec::new(),
        });
    }
    assert_eq!(
        owner.prepare_scene_light_view(&frame, record, unprepared_view),
        Err(SceneLightError::MissingPreparedCamera)
    );
    owner
        .prepare_scene_light_view(&frame, record, view)
        .unwrap();
    let expected = lifted_block(inputs);
    let prepared = owner
        .scene_light_view_block(record, view)
        .unwrap()
        .unwrap()
        .clone();
    for (index, slot) in prepared.slots.iter().enumerate() {
        assert_eq!(
            slot.position.0.map(f32::to_bits),
            core::array::from_fn(|component| expected[index][component])
        );
    }
    assert_eq!(
        owner.publish_scene_lights(&frame, &[SceneLightBinding::new(record, view)]),
        Err(SceneLightError::PublicationDisabled)
    );
    owner.set_scene_light_publication_enabled(true);
    assert!(owner.scene_light_publication_enabled());
    let mut wrong_epoch_frame = frame.clone();
    wrong_epoch_frame.resource_epoch = Some(FrameResourceEpoch(epoch.0 + 1));
    assert_eq!(
        owner.publish_scene_lights(&wrong_epoch_frame, &[SceneLightBinding::new(record, view)],),
        Err(SceneLightError::FrameEpochMismatch)
    );
    assert_eq!(
        owner.prepare_and_publish_scene_lights(
            &frame,
            &[SceneLightBinding::new(record, unprepared_view)],
        ),
        Err(SceneLightError::MissingPreparedCamera)
    );
    assert_eq!(
        owner.scene_light_view_block(record, view).unwrap(),
        Some(&prepared)
    );
    assert_eq!(
        owner.prepare_and_publish_scene_lights(
            &frame,
            &[
                SceneLightBinding::new(record, view),
                SceneLightBinding::new(record, view),
            ],
        ),
        Err(SceneLightError::DuplicatePublicationEntry)
    );
    assert!(owner.published_scene_lights().is_none());
    let published = owner
        .prepare_and_publish_scene_lights(&frame, &[SceneLightBinding::new(record, view)])
        .unwrap()
        .clone();
    assert_eq!(published.blocks.len(), 1);
    assert_eq!(published.blocks[0].record, record);
    assert_eq!(published.blocks[0].view, view);
    assert_eq!(published.blocks[0].epoch, epoch);
    assert_eq!(published.block_for(record, view), published.blocks.first());
    assert!(published.block_for(record, unprepared_view).is_none());
    let mut ambiguous = published.clone();
    ambiguous.blocks.push(ambiguous.blocks[0]);
    assert!(ambiguous.block_for(record, view).is_none());
    let mut stale_block = published.clone();
    stale_block.blocks[0].epoch = FrameResourceEpoch(epoch.0 + 1);
    assert!(stale_block.block_for(record, view).is_none());
    assert_eq!(
        owner.publish_scene_lights(
            &frame,
            &[
                SceneLightBinding::new(record, view),
                SceneLightBinding::new(record, view),
            ],
        ),
        Err(SceneLightError::DuplicatePublicationEntry)
    );
    assert_eq!(
        owner.publish_scene_lights(
            &frame,
            &[
                SceneLightBinding::new(record, view),
                SceneLightBinding::new(record, unprepared_view),
            ],
        ),
        Err(SceneLightError::MissingPreparedViewLight)
    );
    assert_eq!(owner.published_scene_lights(), Some(&published));

    let mut unstamped_frame = owner
        .pair()
        .unwrap()
        .prepare_display_frame(&[], 0.25)
        .unwrap();
    unstamped_frame.views.clone_from(&frame.views);
    assert_eq!(
        owner.attach_scene_lights(&mut unstamped_frame),
        Err(SceneLightError::MissingFrameResourceEpoch)
    );
    assert_eq!(
        owner.prepare_scene_light_view(&unstamped_frame, record, view),
        Err(SceneLightError::MissingFrameResourceEpoch)
    );
    assert_eq!(
        owner.publish_scene_lights(&unstamped_frame, &[SceneLightBinding::new(record, view)],),
        Err(SceneLightError::MissingFrameResourceEpoch)
    );
    let mut stale_frame = later_frame;
    assert_eq!(
        owner.attach_scene_lights(&mut stale_frame),
        Err(SceneLightError::FrameEpochMismatch)
    );
    let mut missing_view_frame = frame.clone();
    missing_view_frame.views.clear();
    assert_eq!(
        owner.attach_scene_lights(&mut missing_view_frame),
        Err(SceneLightError::ViewNotInPreparedFrame)
    );
    owner
        .prepare_scene_lights_for_frame(&mut frame, &[SceneLightBinding::new(record, view)])
        .unwrap();
    assert_eq!(frame.scene_lights, Some(published.clone()));

    // A new view must be publishable on its first sampled frame, without a
    // separately prepared per-view block from an older frame.
    let fresh_view = owner.setup_scene_light_view().unwrap();
    let fresh_binding = SceneLightBinding::new(record, fresh_view);
    let mut fresh_frame = frame.clone();
    fresh_frame
        .views
        .iter_mut()
        .find(|prepared| prepared.scene_light_view == Some(view))
        .unwrap()
        .scene_light_view = Some(fresh_view);
    owner
        .prepare_scene_lights_for_frame(&mut fresh_frame, &[fresh_binding])
        .unwrap();
    let fresh_publication = owner.published_scene_lights().unwrap().clone();
    assert_eq!(fresh_frame.scene_lights, Some(fresh_publication.clone()));

    // Rejected source updates and duplicate selections leave the last
    // publication and the frame attachment intact.
    let replacement = SceneLightInputs {
        colors: [Color3([0.25, 0.5, 0.75]); 4],
        world_points: [WorldPoint([4.0, 5.0, 6.0]); 3],
    };
    let updates = [SceneLightRecordUpdate::new(record, replacement)];
    assert_eq!(
        owner.update_and_prepare_scene_lights_for_frame(
            &mut fresh_frame,
            &updates,
            &[fresh_binding, fresh_binding],
        ),
        Err(SceneLightError::DuplicatePublicationEntry)
    );
    assert_eq!(owner.published_scene_lights(), Some(&fresh_publication));
    assert_eq!(fresh_frame.scene_lights, Some(fresh_publication.clone()));

    owner
        .update_and_prepare_scene_lights_for_frame(&mut fresh_frame, &updates, &[fresh_binding])
        .unwrap();
    let updated_publication = owner.published_scene_lights().unwrap();
    assert_eq!(
        updated_publication.blocks[0].slots[0].color,
        replacement.colors[0]
    );
    assert_eq!(fresh_frame.scene_lights.as_ref(), Some(updated_publication));

    let lease = PreparedSceneLease::new(frame);
    assert_eq!(lease.frame().scene_lights, Some(published.clone()));
    owner.set_scene_light_publication_enabled(false);
    assert!(!owner.scene_light_publication_enabled());
    assert!(owner.published_scene_lights().is_none());
    let mut disabled_frame = owner.prepare_display_frame(&[], 0.5).unwrap();
    assert!(disabled_frame.scene_lights.is_none());
    assert_eq!(
        owner.attach_scene_lights(&mut disabled_frame),
        Err(SceneLightError::NoPublishedFrame)
    );
    assert_eq!(lease.frame().scene_lights, Some(published.clone()));

    owner.set_scene_light_publication_enabled(true);
    owner
        .prepare_scene_light_view(lease.frame(), record, view)
        .unwrap();
    owner
        .publish_scene_lights(lease.frame(), &[SceneLightBinding::new(record, view)])
        .unwrap();
    owner.update_scene_light_record(record, inputs).unwrap();
    assert!(owner.published_scene_lights().is_none());
    assert_eq!(lease.frame().scene_lights, Some(published));
    assert!(
        owner
            .prepare_display_frame(&[], 0.5)
            .unwrap()
            .scene_lights
            .is_none()
    );
}
