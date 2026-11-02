// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "oracle-test")]

use tsfp_audio_stretch_ffi::fill_smoother::FillSmoother;
use tsfp_audio_stretch_ffi::stretch_ratio::Controller;

unsafe extern "C" {
    fn fill_oracle(fill: usize, window: usize, pull: usize, value: *mut f64, valid: *mut bool);
    fn continuous_ratio_oracle(fill: usize, low: usize, floor: u32, ratio: *mut u32) -> u32;
    fn stretch_ratio_oracle(state: *mut Controller, pull: usize) -> u32;
    fn silence_oracle(ring: *mut i16, frames: usize, channels: u16, head: *mut usize,
        fill: *mut usize, excess: usize, pulled: *mut u64, silence: *mut u64) -> usize;
    fn latency_excess_oracle(
        fill: usize,
        target: usize,
        hysteresis: usize,
        trimming: *mut bool,
    ) -> usize;
}

#[test]
fn silence_skip_matches_production_sink() {
    for channels in [1, 2, 4] {
        for head in [0, 200, 299] {
            for run in [0, 127, 128, 129, 299, 300] {
                for excess in [0, 1, 127, 128, 300] {
                    let mut ring = vec![1i16; 300 * channels];
                    for offset in 0..run {
                        let index = ((head + offset) % 300) * channels;
                        ring[index..index + channels].fill(0);
                    }
                    let (mut rh, mut ch, mut rf, mut cf) = (head, head, 300, 300);
                    let (mut pulled, mut silence) = (7, 11);
                    let expected = unsafe { silence_oracle(ring.as_mut_ptr(), 300, channels as u16,
                        &mut ch, &mut cf, excess, &mut pulled, &mut silence) };
                    let actual = tsfp_audio_stretch_ffi::latency_trim::skip_silence(
                        &ring, channels, &mut rh, &mut rf, excess);
                    assert_eq!((actual, rh, rf, 7 + actual as u64, 11 + actual as u64),
                        (expected, ch, cf, pulled, silence));
                }
            }
        }
    }
}

#[test]
fn stretch_ratio_matches_production_sink_across_pulls() {
    for down in [0, 1000, 65536] {
        for smooth_window in [0, 7200] {
            for ema_valid in [false, true] {
                let mut c = Controller {
                    fill: 0,
                    need: 0,
                    smooth_window,
                    ema: 2500.0,
                    ema_valid,
                    low: 4800,
                    floor: 32768,
                    ratio: 65536,
                    engage: 62000,
                    leave: 65000,
                    down,
                    up: 400,
                    engagements: 0,
                    panic_pulls: 0,
                    max_step: 0,
                };
                let mut probe = c;
                unsafe { stretch_ratio_oracle(&mut probe, 512) };
                assert!(probe.need > 0);
                for fill in [
                    12000, 4800, 2500, 900, 0, 900, 2500, 4800, 12000,
                    probe.need * 2 - 1, probe.need * 2, probe.need * 2 + 1,
                    probe.need * 3 - 1, probe.need * 3,
                ] {
                    c.fill = fill;
                    let mut rust = c;
                    let expected = unsafe { stretch_ratio_oracle(&mut c, 512) };
                    rust.need = c.need;
                    let actual = rust.update(512);
                    assert_eq!(
                        (actual, rust.engagements, rust.panic_pulls, rust.max_step),
                        (expected, c.engagements, c.panic_pulls, c.max_step),
                        "fill={fill} down={down} smooth={smooth_window} valid={ema_valid}"
                    );
                }
            }
        }
    }
}

#[test]
fn latency_hysteresis_matches_production_sink() {
    for target in [0, 1, 4800, usize::MAX] {
        for hysteresis in [0, 1, 500, usize::MAX] {
            for initial in [false, true] {
                let (mut rust, mut c) = (initial, initial);
                for fill in [
                    0,
                    1,
                    4799,
                    4800,
                    4801,
                    5300,
                    5301,
                    usize::MAX,
                    4801,
                    4800,
                    0,
                ] {
                    let actual = tsfp_audio_stretch_ffi::latency_trim::excess(
                        fill, target, hysteresis, &mut rust,
                    );
                    let expected =
                        unsafe { latency_excess_oracle(fill, target, hysteresis, &mut c) };
                    assert_eq!(
                        (actual, rust),
                        (expected, c),
                        "fill={fill} target={target} hysteresis={hysteresis}"
                    );
                }
            }
        }
    }
}

#[test]
fn continuous_ratio_matches_production_sink_across_pulls() {
    for low in [0, 1, 4800, 12000] {
        for floor in [0, 32768, 58982, 65536] {
            let (mut rust, mut c) = (65536, 65536);
            for fill in [12000, 4800, 4000, 1, 0, 0, 1200, 4799, 4800, 12000] {
                let actual =
                    tsfp_audio_stretch_ffi::continuous_ratio::update(fill, low, floor, &mut rust);
                let expected = unsafe { continuous_ratio_oracle(fill, low, floor, &mut c) };
                assert_eq!(
                    (actual, rust),
                    (expected, c),
                    "fill={fill} low={low} floor={floor}"
                );
            }
        }
    }
}

#[test]
fn smoothed_fill_matches_production_sink_across_pulls() {
    let mut rust = FillSmoother::default();
    let (mut c_value, mut c_valid) = (0.0, false);
    for window in [0, 1, 7200, 0, 7200, usize::MAX] {
        for pull in [0, 1, 256, 1024, usize::MAX] {
            for fill in [0, 1, 4096, 12000, usize::MAX] {
                rust.update(fill, window, pull);
                unsafe { fill_oracle(fill, window, pull, &mut c_value, &mut c_valid) };
                assert_eq!(rust.valid, c_valid);
                assert_eq!(
                    rust.frames.to_bits(),
                    c_value.to_bits(),
                    "fill={fill} window={window} pull={pull}"
                );
            }
        }
    }
}
