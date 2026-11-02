use tsfp_audio::stretch::Stretch;
#[test]
fn refuses_without_mutating_and_can_restart() {
    assert!(Stretch::new(0, 4).is_none());
    assert!(Stretch::new(2, 3).is_none());
    assert!(Stretch::new(2, usize::MAX).is_none());
    let mut e = Stretch::new(2, 4).unwrap();
    assert_eq!(e.input_needed(3), Some(13));
    assert_eq!(e.input_needed(usize::MAX), None);
    let mut out = [123; 8];
    assert!(!e.hold(&mut out));
    assert!(!e.prime(&[0; 15], &mut out));
    assert_eq!(out, [123; 8]);
    assert!(!e.primed());
    assert!(e.prime(&[100; 16], &mut out));
    assert_eq!(out, [100; 8]);
    assert_eq!(e.next(&[200; 16], usize::MAX, &mut out), None);
    assert_eq!(e.next(&[200; 16], 1, &mut out), None);
    assert!(!e.leave(&[0; 7], &mut out));
    assert!(!e.fade_out(&mut out[..7]));
    assert_eq!(out, [100; 8]);
    assert!(e.primed());
    assert!(e.hold(&mut out));
    assert_eq!(out, [100; 8]);
    assert!(e.leave(&[100; 8], &mut out));
    assert!(!e.fade_out(&mut out));
    assert!(e.prime(&[-100; 16], &mut out));
    assert!(e.fade_out(&mut out));
    assert!(!e.primed());
    assert_eq!(out, [-81, -81, -61, -61, -41, -41, -21, -21]);
}
