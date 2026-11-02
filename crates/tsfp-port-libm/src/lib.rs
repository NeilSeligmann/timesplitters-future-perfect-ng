// SPDX-License-Identifier: GPL-3.0-or-later
//! T3785: no_std `f64` math the readable ports call as std methods (`f64::atan2`), for the no_std
//! family crates of the host archive (`tsfp-port-lanes-typed` compiles tsfp-engine-physics
//! `followup.rs` there). `Atan2` is a trait so a `use` makes `y.atan2(x)` resolve to it in no_std,
//! while a std build keeps the inherent method (inherent methods win).
//!
//! Algorithm after fdlibm `s_atan.c` / `e_atan2.c` (as in musl), whose notice is:
//! Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved. Developed at SunPro, a Sun
//! Microsystems, Inc. business. Permission to use, copy, modify, and distribute this software is
//! freely granted, provided that this notice is preserved.
//! Error below 1 ulp (fdlibm). The lifted C calls glibc `atan2`, so a rare 1 ulp difference is
//! possible; the test below measures it against std (glibc) on a broad sample.
#![no_std]

/// `atan(0.5)`, `atan(1)`, `atan(1.5)`, `atan(inf)` split into a high and a low part.
const ATAN_HI: [f64; 4] = [
    4.636_476_090_008_060_935_15e-01,
    7.853_981_633_974_482_789_99e-01,
    9.827_937_232_473_290_540_82e-01,
    1.570_796_326_794_896_558_00e+00,
];
const ATAN_LO: [f64; 4] = [
    2.269_877_745_296_168_709_24e-17,
    3.061_616_997_868_383_017_93e-17,
    1.390_331_103_123_099_845_16e-17,
    6.123_233_995_736_766_035_87e-17,
];
/// Odd polynomial coefficients of atan on [-7/16, 7/16].
const AT: [f64; 11] = [
    3.333_333_333_333_293_180_27e-01,
    -1.999_999_999_987_648_324_76e-01,
    1.428_571_427_250_346_637_11e-01,
    -1.111_111_040_546_235_578_80e-01,
    9.090_887_133_436_506_561_96e-02,
    -7.691_876_205_044_829_994_95e-02,
    6.661_073_137_387_531_206_69e-02,
    -5.833_570_133_790_573_486_45e-02,
    4.976_877_994_615_932_360_17e-02,
    -3.653_157_274_421_691_552_70e-02,
    1.628_582_011_536_578_236_23e-02,
];
const PI: f64 = 3.141_592_653_589_793_116_0;
const PI_LO: f64 = 1.224_646_799_147_353_177_2e-16;

fn high_word(value: f64) -> u32 {
    (value.to_bits() >> 32) as u32
}

fn low_word(value: f64) -> u32 {
    value.to_bits() as u32
}

/// Arc tangent of `x` in radians.
pub fn atan(x: f64) -> f64 {
    let high = high_word(x);
    let negative = high >> 31 != 0;
    let magnitude = high & 0x7fff_ffff;
    if magnitude >= 0x4410_0000 {
        // |x| >= 2^66 or NaN
        if x.is_nan() {
            return x;
        }
        let z = ATAN_HI[3] + f64::from_bits(0x3870_0000_0000_0000); // + 2^-120
        return if negative { -z } else { z };
    }
    let (reduced, interval) = if magnitude < 0x3fdc_0000 {
        // |x| < 7/16
        if magnitude < 0x3e40_0000 {
            return x; // |x| < 2^-27
        }
        (x, None)
    } else {
        let x = x.abs();
        if magnitude < 0x3ff3_0000 {
            if magnitude < 0x3fe6_0000 {
                ((2.0 * x - 1.0) / (2.0 + x), Some(0)) // 7/16 <= |x| < 11/16
            } else {
                ((x - 1.0) / (x + 1.0), Some(1)) // 11/16 <= |x| < 19/16
            }
        } else if magnitude < 0x4003_8000 {
            ((x - 1.5) / (1.0 + 1.5 * x), Some(2)) // 19/16 <= |x| < 39/16
        } else {
            (-1.0 / x, Some(3))
        }
    };
    let z = reduced * reduced;
    let w = z * z;
    let odd = z * (AT[0] + w * (AT[2] + w * (AT[4] + w * (AT[6] + w * (AT[8] + w * AT[10])))));
    let even = w * (AT[1] + w * (AT[3] + w * (AT[5] + w * (AT[7] + w * AT[9]))));
    match interval {
        None => reduced - reduced * (odd + even),
        Some(index) => {
            let z = ATAN_HI[index] - ((reduced * (odd + even) - ATAN_LO[index]) - reduced);
            if negative { -z } else { z }
        }
    }
}

/// Arc tangent of `y / x` in radians, in the quadrant of `(x, y)` (`f64::atan2` semantics).
pub fn atan2(y: f64, x: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return x + y;
    }
    let (x_high, x_low) = (high_word(x), low_word(x));
    let (y_high, y_low) = (high_word(y), low_word(y));
    if x_high.wrapping_sub(0x3ff0_0000) | x_low == 0 {
        return atan(y); // x == 1.0
    }
    // bit 0: y negative, bit 1: x negative
    let quadrant = ((y_high >> 31) & 1) | ((x_high >> 30) & 2);
    let x_high = x_high & 0x7fff_ffff;
    let y_high = y_high & 0x7fff_ffff;
    if y_high | y_low == 0 {
        return match quadrant {
            0 | 1 => y, // atan(+-0, +anything) = +-0
            2 => PI,
            _ => -PI,
        };
    }
    if x_high | x_low == 0 {
        return if quadrant & 1 != 0 {
            -PI / 2.0
        } else {
            PI / 2.0
        };
    }
    if x_high == 0x7ff0_0000 {
        return if y_high == 0x7ff0_0000 {
            match quadrant {
                0 => PI / 4.0,
                1 => -PI / 4.0,
                2 => 3.0 * PI / 4.0,
                _ => -3.0 * PI / 4.0,
            }
        } else {
            match quadrant {
                0 => 0.0,
                1 => -0.0,
                2 => PI,
                _ => -PI,
            }
        };
    }
    // |y / x| > 2^64 or y infinite
    if x_high.wrapping_add(64 << 20) < y_high || y_high == 0x7ff0_0000 {
        return if quadrant & 1 != 0 {
            -PI / 2.0
        } else {
            PI / 2.0
        };
    }
    let z = if quadrant & 2 != 0 && y_high.wrapping_add(64 << 20) < x_high {
        0.0 // |y / x| < 2^-64 with x < 0
    } else {
        atan((y / x).abs())
    };
    match quadrant {
        0 => z,
        1 => -z,
        2 => PI - (z - PI_LO),
        _ => (z - PI_LO) - PI,
    }
}

/// Method form, so translated `y.atan2(x)` compiles unchanged in no_std (see the crate doc).
pub trait Atan2 {
    fn atan2(self, x: f64) -> f64;
}

impl Atan2 for f64 {
    fn atan2(self, x: f64) -> f64 {
        atan2(self, x)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::{atan, atan2};

    fn ulps(a: f64, b: f64) -> u64 {
        if a.is_nan() && b.is_nan() {
            return 0;
        }
        let order = |v: f64| {
            let bits = v.to_bits() as i64;
            if bits < 0 { i64::MIN - bits } else { bits }
        };
        order(a).abs_diff(order(b))
    }

    /// xorshift64*, deterministic sample of bit patterns and of ordinary magnitudes.
    fn sample(count: usize) -> std::vec::Vec<f64> {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut values = std::vec::Vec::with_capacity(count);
        for index in 0..count {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            let random = state.wrapping_mul(0x2545_f491_4f6c_dd1d);
            let value = if index % 2 == 0 {
                f64::from_bits(random)
            } else {
                let unit = (random >> 11) as f64 / (1u64 << 53) as f64;
                let scale = [1e-3, 0.5, 1.0, 3.0, 100.0, 1e6][index % 6];
                (unit * 2.0 - 1.0) * scale
            };
            values.push(value);
        }
        values
    }

    #[test]
    fn special_values_match_std() {
        let specials = [
            0.0,
            -0.0,
            1.0,
            -1.0,
            0.4375,
            0.6875,
            1.1875,
            2.4375,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MIN_POSITIVE,
            -f64::MIN_POSITIVE,
            f64::from_bits(1),
            f64::MAX,
            -f64::MAX,
            1e-30,
            1e30,
        ];
        let edge = |v: f64| v == 0.0 || v.is_infinite();
        for &y in &specials {
            assert!(ulps(atan(y), y.atan()) <= 1, "atan({y:e})");
            for &x in &specials {
                let distance = ulps(atan2(y, x), y.atan2(x));
                // zeros and infinities are exact constants, the rest within 1 ulp (fdlibm bound)
                let bound = if edge(x) || edge(y) { 0 } else { 1 };
                assert!(distance <= bound, "atan2({y:e}, {x:e}): {distance} ulp");
            }
        }
        assert!(atan2(f64::NAN, 1.0).is_nan() && atan2(1.0, f64::NAN).is_nan());
        assert!(atan(f64::NAN).is_nan());
    }

    #[test]
    fn random_pairs_within_one_ulp_of_std() {
        let values = sample(4000);
        let (mut compared, mut exact) = (0usize, 0usize);
        for pair in values.chunks_exact(2) {
            for (y, x) in [(pair[0], pair[1]), (pair[1], pair[0])] {
                let (ours, reference) = (atan2(y, x), y.atan2(x));
                let distance = ulps(ours, reference);
                assert!(
                    distance <= 1,
                    "atan2({y:e}, {x:e}) = {ours:e}, std {reference:e}"
                );
                compared += 1;
                exact += usize::from(distance == 0);
            }
            let distance = ulps(atan(pair[0]), pair[0].atan());
            assert!(distance <= 1, "atan({:e})", pair[0]);
        }
        std::println!("{exact} of {compared} atan2 results bit-identical to std");
        assert!(
            compared == 4000 && exact * 100 >= compared * 99,
            "{exact} of {compared} exact"
        );
    }
}
