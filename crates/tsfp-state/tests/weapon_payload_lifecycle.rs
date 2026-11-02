// SPDX-License-Identifier: GPL-3.0-or-later
use tsfp_state::{
    object_pool::ObjectId,
    weapon_payload::{
        PayloadRotationMode, WeaponPayloadLifecycle, WeaponPayloadLifecycleError, WeaponPayloadPose,
    },
};

fn object(slot: u32, generation: u64) -> ObjectId {
    ObjectId { slot, generation }
}

fn pose(staged_yaw_degrees: f32) -> WeaponPayloadPose {
    WeaponPayloadPose::from_definition([0.0, 45.0, 0.0], staged_yaw_degrees)
}

#[test]
fn payload_pose_is_owned_by_the_exact_object_generation() {
    let mut lifecycle = WeaponPayloadLifecycle::with_capacity(4);
    let object_id = object(2, 17);
    let initial = pose(12.0);

    lifecycle.attach(object_id, initial).unwrap();

    assert_eq!(lifecycle.pose(object_id), Ok(initial));
    assert_eq!(
        lifecycle.attach(object_id, initial),
        Err(WeaponPayloadLifecycleError::AlreadyAttached)
    );
    assert_eq!(
        lifecycle.attach(object(2, 18), pose(20.0)),
        Err(WeaponPayloadLifecycleError::SlotOccupied)
    );
}

#[test]
fn recycled_slot_rejects_old_pose_access_and_retirement() {
    let mut lifecycle = WeaponPayloadLifecycle::with_capacity(4);
    let old = object(2, 17);
    let current = object(2, 18);
    let current_pose = pose(30.0);

    lifecycle.attach(old, pose(12.0)).unwrap();
    lifecycle.retire(old).unwrap();
    lifecycle.attach(current, current_pose).unwrap();

    assert_eq!(
        lifecycle.pose(old),
        Err(WeaponPayloadLifecycleError::StaleObjectId)
    );
    assert_eq!(
        lifecycle.set_pose(old, pose(99.0)),
        Err(WeaponPayloadLifecycleError::StaleObjectId)
    );
    assert_eq!(
        lifecycle.retire(old),
        Err(WeaponPayloadLifecycleError::StaleObjectId)
    );
    assert_eq!(lifecycle.pose(current), Ok(current_pose));
}

#[test]
fn payload_pose_keeps_staged_yaw_separate_from_template_override() {
    let mut lifecycle = WeaponPayloadLifecycle::with_capacity(2);
    let object = object(0, 4);
    let mut no_template = WeaponPayloadPose::from_definition([0.0, 10.0, 0.0], 7.0);
    assert_eq!(no_template.rotation_mode, PayloadRotationMode::Yaw);
    assert!(!no_template.apply_c1950_template(0, [1.0, 2.0, 3.0]));
    assert_eq!(no_template.rotation_mode, PayloadRotationMode::Euler);
    assert_eq!(no_template.pitch_yaw_roll_degrees, [0.0, 10.0, 0.0]);
    assert_eq!(no_template.staged_yaw_degrees, 7.0);
    let mut initial = WeaponPayloadPose::from_definition([0.0, 10.0, 0.0], 7.0);
    lifecycle.attach(object, initial).unwrap();

    assert!(initial.apply_c1950_template(0x2, [1.0, 2.0, 3.0]));
    lifecycle.set_pose(object, initial).unwrap();

    let actual = lifecycle.pose(object).unwrap();
    assert_eq!(actual.pitch_yaw_roll_degrees, [1.0, 2.0, 3.0]);
    assert_eq!(actual.staged_yaw_degrees, 7.0);
    assert_eq!(actual.rotation_mode, PayloadRotationMode::Euler);
}

#[test]
fn lifecycle_checks_slot_bounds_and_requires_retirement_before_reuse() {
    let mut lifecycle = WeaponPayloadLifecycle::with_capacity(1);

    assert_eq!(
        lifecycle.attach(object(1, 1), pose(0.0)),
        Err(WeaponPayloadLifecycleError::SlotOutOfRange)
    );
    assert_eq!(
        lifecycle.retire(object(0, 1)),
        Err(WeaponPayloadLifecycleError::NotAttached)
    );
}
