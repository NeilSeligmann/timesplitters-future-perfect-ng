// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "oracle-test")]

use tsfp_audio_stretch_ffi::stretch_fifo::{drain, drain_active};

unsafe extern "C" {
    fn stretch_fifo_active_oracle(
        fifo: *mut i16,
        channels: u16,
        position: *mut usize,
        length: *mut usize,
        output: *mut i16,
        frames: usize,
        active: bool,
    ) -> usize;
    fn stretch_fifo_oracle(
        fifo: *mut i16,
        channels: u16,
        position: *mut usize,
        length: *mut usize,
        output: *mut i16,
        frames: usize,
    ) -> usize;
}

#[test]
fn active_fifo_contained_requests_match_production_c() {
    for channels in [1_u16, 2, 4] {
        let mut fifo: Vec<i16> = (0..8 * usize::from(channels))
            .map(|n| n as i16 * 73)
            .collect();
        for length in 0..=8 {
            for position in 0..=length {
                for frames in 0..=length - position {
                    let (mut cp, mut cl, mut rp) = (position, length, position);
                    let mut c = vec![-1; frames * usize::from(channels)];
                    let mut r = c.clone();
                    let expected = unsafe {
                        stretch_fifo_active_oracle(
                            fifo.as_mut_ptr(),
                            channels,
                            &mut cp,
                            &mut cl,
                            c.as_mut_ptr(),
                            frames,
                            true,
                        )
                    };
                    assert_eq!(
                        drain_active(&fifo, channels.into(), &mut rp, length, &mut r),
                        Some(expected)
                    );
                    assert_eq!((r, rp, length), (c, cp, cl));
                }
            }
        }
    }
    let mut pos = 1;
    let mut out = [9, 9];
    assert_eq!(drain_active(&[1, 2], 1, &mut pos, 2, &mut out), None);
    assert_eq!((pos, out), (1, [9, 9]));
}

#[test]
fn inactive_fifo_drain_matches_production_c() {
    for channels in [1_u16, 2, 4] {
        let mut fifo: Vec<i16> = (0..8 * usize::from(channels))
            .map(|n| n as i16 * 73)
            .collect();
        for length in 0..=8 {
            for position in 0..=length {
                for frames in 0..=10 {
                    let (mut cp, mut cl, mut rp, mut rl) = (position, length, position, length);
                    let mut c = vec![-1; frames * usize::from(channels)];
                    let mut r = c.clone();
                    let expected = unsafe {
                        stretch_fifo_oracle(
                            fifo.as_mut_ptr(),
                            channels,
                            &mut cp,
                            &mut cl,
                            c.as_mut_ptr(),
                            frames,
                        )
                    };
                    assert_eq!(
                        drain(&fifo, channels.into(), &mut rp, &mut rl, &mut r),
                        Some(expected)
                    );
                    assert_eq!((r, rp, rl), (c, cp, cl));
                }
            }
        }
    }
}

#[test]
fn invalid_geometry_preserves_output_and_state() {
    let (mut pos, mut len) = (2, 1);
    let mut output = [9, 9];
    assert_eq!(drain(&[1, 2], 2, &mut pos, &mut len, &mut output), None);
    assert_eq!((pos, len, output), (2, 1, [9, 9]));
}
