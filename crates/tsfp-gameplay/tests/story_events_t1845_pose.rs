//! T1845 task E: pose-tick family models.
use tsfp_gameplay::story_events::pose::*;

fn close(a: &Mat4, b: &Mat4) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
}

#[test]
fn pivot_rotation_keeps_the_pivot_fixed() {
    let pivot = [3.0, 4.0, 5.0];
    let m = rotation_about_pivot(0.7, [0.0, 0.0, 1.0], pivot);
    let v = [pivot[0], pivot[1], pivot[2], 1.0];
    let out: Vec<f32> = (0..3)
        .map(|c| (0..4).map(|k| v[k] * m[k * 4 + c]).sum())
        .collect();
    for i in 0..3 {
        assert!((out[i] - pivot[i]).abs() < 1e-4);
    }
}

#[test]
fn zero_angle_is_identity_and_rotation_y_layout() {
    assert!(close(
        &rotation_about_pivot(0.0, [0., 0., 1.], [1., 2., 3.]),
        &IDENTITY
    ));
    let y = rotation_y(0.5);
    assert_eq!((y[0], y[10]), (0.5f32.cos(), 0.5f32.cos()));
    assert_eq!((y[2], y[8]), (-0.5f32.sin(), 0.5f32.sin()));
}

#[test]
fn trst_pose_resets_nodes() {
    let p = trst_pose(4, 0.3, [1., 0., 0.]);
    assert!(close(&p[0], &IDENTITY) && close(&p[2], &IDENTITY));
    assert!(!close(&p[1], &IDENTITY));
}

#[test]
fn nklc_translates_node_two_unless_scale_one() {
    let a = nklc_pose(3, 0.0, 1.0, [0., 0., 0.], 0.2, 1.0);
    assert!(close(&a[2], &IDENTITY));
    let b = nklc_pose(3, 0.0, 1.0, [0., 0., 0.], 0.2, 0.5);
    assert_eq!(b[2][13], -0.5);
}

#[test]
fn timed_tick_counts_down_fades_then_destroys() {
    assert_eq!(timed_frames(2.0), 120);
    let mut o = TimedObject {
        flags_18: 0,
        flags_28: 0,
        mode: 1,
        timer: 140,
        anim_busy: false,
        has_handle: false,
        handle_entry_idle: true,
    };
    let fx = timed_tick(&mut o, 4);
    assert_eq!(fx, vec![TimedEffect::UnlinkDecals]);
    assert_eq!((o.timer, o.flags_18 & 0x40), (136, 0x40));
    o.timer = 130;
    let fx = timed_tick(&mut o, 4);
    assert_eq!(fx, vec![TimedEffect::Fade { alpha: 126 }]);
    o.timer = 2;
    let fx = timed_tick(&mut o, 5);
    assert_eq!(
        fx,
        vec![TimedEffect::Fade { alpha: 0 }, TimedEffect::Destroy]
    );
}

#[test]
fn timed_tick_waits_for_animation_and_handle() {
    let mut o = TimedObject {
        flags_18: 0,
        flags_28: 0,
        mode: 1 | 4,
        timer: 50,
        anim_busy: true,
        has_handle: true,
        handle_entry_idle: false,
    };
    assert!(timed_tick(&mut o, 1).is_empty());
    assert_eq!(o.timer, 50);
    o.anim_busy = false;
    o.mode |= 2;
    assert!(timed_tick(&mut o, 1).is_empty());
    o.handle_entry_idle = true;
    let fx = timed_tick(&mut o, 1);
    assert_eq!(fx[0], TimedEffect::ReleaseHandle);
    assert_eq!(o.mode, 1);
}

#[test]
fn no_active_bit_destroys_at_once_and_slots() {
    let mut o = TimedObject {
        flags_18: 0,
        flags_28: 0x400,
        mode: 0,
        timer: 0,
        anim_busy: false,
        has_handle: false,
        handle_entry_idle: true,
    };
    assert_eq!(timed_tick(&mut o, 1), vec![TimedEffect::Destroy]);
    assert_eq!(tick_slot_offset(3), 0x160);
    assert_eq!(tick_list(8), Some(TickList::State8));
    assert_eq!(tick_list(3), None);
    let (a, b, w) = beam_endpoints(&IDENTITY, [1., 2., 3.], [1., 0., 0.], [0., 1., 0.]);
    assert_eq!((a, b, w), ([2., 2., 3.], [1., 3., 3.], 30.0));
}
