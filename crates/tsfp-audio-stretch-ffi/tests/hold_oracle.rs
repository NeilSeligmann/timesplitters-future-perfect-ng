// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "oracle-test")]

use tsfp_audio_stretch_ffi::stretch_hold::{advance, finish_fade};

unsafe extern "C" {
    fn stretch_hold_oracle(
        run: *mut usize,
        full: usize,
        max: usize,
        gain: *mut u32,
        held: *mut u64,
        decayed: *mut u64,
    );
    fn stretch_fade_oracle(capacity: usize, head: *mut usize, fill: *mut usize,
        consumed: *mut usize, fadeouts: *mut u64, active: *mut bool);
}

#[test]
fn exhausted_hold_fade_drains_ring_like_production_sink() {
    for capacity in [1, 7, 128] {
        for head in 0..capacity {
            for fill in 0..=capacity {
                let (mut ch, mut cf, mut cc, mut cn, mut ca) = (head, fill, 9, 4, true);
                let (mut rh, mut rf, mut rc, mut rn, mut ra) = (head, fill, 9, 4, true);
                unsafe { stretch_fade_oracle(capacity, &mut ch, &mut cf, &mut cc, &mut cn, &mut ca) };
                assert!(finish_fade(capacity, &mut rh, &mut rf, &mut rc, &mut ra, &mut rn));
                assert_eq!((rh, rf, rc, rn, ra), (ch, cf, cc, cn, ca));
            }
        }
    }
}

#[test]
fn hold_decay_matches_production_sink() {
    for full in [0, 100, 160, 321, 640] {
        for max in [640, 801, 1280] {
            for run in (0..=max - 160).step_by(37) {
                let (mut crun, mut rrun) = (run, run);
                let (mut gain, mut held, mut decayed) = (1234, 11, 29);
                unsafe {
                    stretch_hold_oracle(&mut crun, full, max, &mut gain, &mut held, &mut decayed)
                };
                let (target, decay) = advance(&mut rrun, 160, full, max).unwrap();
                assert_eq!(
                    (rrun, target, 11 + 160, 29 + if decay { 160 } else { 0 }),
                    (crun, gain, held, decayed)
                );
            }
        }
    }
}

#[test]
fn invalid_or_exhausted_hold_preserves_run() {
    for (run, overlap, full, max) in [
        (0, 0, 0, 1),
        (0, 1, 2, 1),
        (4, 2, 0, 5),
        (usize::MAX, 1, 0, usize::MAX),
    ] {
        let mut value = run;
        assert_eq!(advance(&mut value, overlap, full, max), None);
        assert_eq!(value, run);
    }
}
