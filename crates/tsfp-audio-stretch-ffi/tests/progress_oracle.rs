// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "oracle-test")]

use tsfp_audio_stretch_ffi::stretch_progress::{Leave, Prime, Progress, leave, next, prime};

unsafe extern "C" {
    fn stretch_progress_oracle(state: *mut Progress, ratio: u32) -> usize;
    fn stretch_leave_oracle(state: *mut Leave, tail: usize) -> bool;
    fn stretch_prime_oracle(state: *mut Prime, ratio: u32);
}

#[test]
fn prime_matches_production_sink() {
    for fill in [0, 319, 320, 1024] {
        for ratio in [0, 32768, 65535, 65536] {
            for fade in [false, true] {
                for active in [false, true] {
                    let mut c = Prime {
                        fill,
                        fifo_pos: 7,
                        fifo_len: 0,
                        tail: 9,
                        position_q16: 42,
                        hold_run: 10,
                        gain_q16: 30000,
                        active,
                        fade_in_pending: fade,
                    };
                    let mut rust = c;
                    unsafe { stretch_prime_oracle(&mut c, ratio) };
                    prime(&mut rust, 160, ratio, true);
                    assert_eq!(rust, c);
                }
            }
        }
    }
}

#[test]
fn leave_matches_production_sink() {
    for head in [0, 7, 1023] {
        for tail in [0, 100, 500] {
            for fill in [159, 160, 260, 660, 1024] {
                let mut c = Leave {
                    head,
                    fill,
                    consumed: 9,
                    fifo_pos: 3,
                    fifo_len: 4,
                    active: true,
                };
                let mut rust = c;
                let result = unsafe { stretch_leave_oracle(&mut c, tail) };
                assert_eq!(leave(&mut rust, 1024, tail, 160), result);
                assert_eq!(rust, c);
            }
        }
    }
}

#[test]
fn leave_invalid_inputs_preserve_state() {
    let mut state = Leave {
        head: 0,
        fill: 1,
        consumed: usize::MAX,
        fifo_pos: 3,
        fifo_len: 4,
        active: true,
    };
    let before = state;
    assert!(!leave(&mut state, 1, 0, 1));
    assert_eq!(state, before);
}

#[test]
fn successful_block_matches_production_sink() {
    for head in [0, 7, 1023] {
        for position in [0, 80, 200, 400] {
            for ratio in [1, 32768, 65535] {
                let mut c = Progress {
                    position_q16: (position << 16) + 17,
                    tail: 7,
                    head,
                    fill: 1024,
                    consumed: 9,
                    hold_run: 160,
                    blocks: 4,
                };
                let mut rust = c;
                let chosen = unsafe { stretch_progress_oracle(&mut c, ratio) };
                assert!(next(&mut rust, 1024, chosen, 160, 80, ratio));
                assert_eq!(rust, c);
            }
        }
    }
}

#[test]
fn invalid_ring_does_not_change_progress() {
    let mut state = Progress {
        position_q16: 0,
        tail: 0,
        head: 0,
        fill: 1,
        consumed: 0,
        hold_run: 3,
        blocks: 0,
    };
    let before = state;
    assert!(!next(&mut state, 0, 0, 160, 80, 1));
    assert_eq!(state, before);
}
