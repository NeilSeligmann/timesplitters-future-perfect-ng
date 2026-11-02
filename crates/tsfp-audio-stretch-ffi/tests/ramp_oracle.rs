// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "oracle-test")]

use tsfp_audio_stretch_ffi::stretch_ramp::apply;

unsafe extern "C" {
    fn ramp_oracle(samples: *mut i16, frames: usize, channels: u16, gain: *mut u32, target: u32);
}

#[test]
fn ramp_matches_production_c() {
    for channels in [1_u16, 2, 4] {
        for frames in [0, 1, 2, 7, 128] {
            for (start, target) in [(0, 65536), (65536, 0), (12345, 54321), (65536, 65536)] {
                let mut c: Vec<i16> = (0..frames * usize::from(channels))
                    .map(|i| (i as i16).wrapping_mul(1973).wrapping_sub(16384))
                    .collect();
                let mut rust = c.clone();
                let mut cg = start;
                let mut rg = start;
                unsafe { ramp_oracle(c.as_mut_ptr(), frames, channels, &mut cg, target) };
                assert!(apply(&mut rust, channels.into(), &mut rg, target));
                assert_eq!((rust, rg), (c, cg));
            }
        }
    }
}

#[test]
fn invalid_geometry_and_gain_do_not_write() {
    let mut samples = [123, -456, 789];
    let mut gain = 65536;
    assert!(!apply(&mut samples, 2, &mut gain, 0));
    assert!(!apply(&mut samples, 1, &mut gain, 65537));
    assert_eq!((samples, gain), ([123, -456, 789], 65536));
}
