// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "oracle-test")]

use tsfp_audio_stretch_ffi::stretch_ring::{consume, load};

unsafe extern "C" {
    fn stretch_ring_oracle(
        ring: *mut i16,
        capacity: usize,
        channels: u16,
        head: *mut usize,
        fill: *mut usize,
        scratch: *mut i16,
        frames: usize,
        requested: usize,
    ) -> usize;
}

#[test]
fn ring_load_and_consume_match_production_c() {
    for channels in [1_u16, 2, 4] {
        for capacity in [1, 3, 8] {
            let mut ring: Vec<i16> = (0..capacity * usize::from(channels))
                .map(|i| i as i16 * 73)
                .collect();
            for head in 0..capacity {
                for fill in 0..=capacity {
                    for requested in [0, 1, capacity, capacity + 1] {
                        let (mut ch, mut cf) = (head, fill);
                        let (mut rh, mut rf) = (head, fill);
                        let mut c = vec![0; capacity * usize::from(channels)];
                        let mut r = c.clone();
                        let cc = unsafe {
                            stretch_ring_oracle(
                                ring.as_mut_ptr(),
                                capacity,
                                channels,
                                &mut ch,
                                &mut cf,
                                c.as_mut_ptr(),
                                capacity,
                                requested,
                            )
                        };
                        assert!(load(&ring, channels.into(), rh, &mut r));
                        let rc = consume(capacity, &mut rh, &mut rf, requested);
                        assert_eq!((r, rh, rf, rc), (c, ch, cf, Some(cc)));
                    }
                }
            }
        }
    }
}

#[test]
fn invalid_geometry_does_not_change_state() {
    let mut output = [9; 3];
    assert!(!load(&[1, 2], 2, 0, &mut output));
    assert_eq!(output, [9; 3]);
    let (mut head, mut fill) = (0, 1);
    assert_eq!(consume(0, &mut head, &mut fill, 1), None);
    assert_eq!((head, fill), (0, 1));
}
