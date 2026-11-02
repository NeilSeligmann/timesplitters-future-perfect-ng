//! DirectSound millibel volume to Q16 linear gain. Oracle: dsound_mixer_volume_to_gain_q16.

pub const VOLUME_MIN: i32 = -10000;

/// gain = 10^(mb / 2000) in Q16 (65536 = 1.0). None outside -10000..=0. Same range-reduced Taylor
/// series as the C oracle so the rounding is identical: exp(x) = exp(x / 256)^256.
pub fn volume_to_gain_q16(millibels: i32) -> Option<u32> {
    if !(VOLUME_MIN..=0).contains(&millibels) {
        return None;
    }
    if millibels == 0 {
        return Some(65536);
    }
    if millibels == VOLUME_MIN {
        return Some(0);
    }
    let x = f64::from(millibels) * 2.302585092994046 / 2000.0 / 256.0;
    let (mut term, mut sum) = (1.0f64, 1.0f64);
    for i in 1..14 {
        term *= x / f64::from(i);
        sum += term;
    }
    for _ in 0..8 {
        sum *= sum;
    }
    Some((sum * 65536.0 + 0.5) as u32)
}

/// `(sample * gain + 32768) >> 16`, the per stream apply of the oracle.
pub fn apply_q16(sample: i32, gain_q16: u32) -> i32 {
    ((i64::from(sample) * i64::from(gain_q16) + 32768) >> 16) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Measured table of the retired C mixer (`dsound_mixer_volume_to_gain_q16`, tests/c/test_dsound_mixer.c, T818).
    #[test]
    fn measured_table_of_the_retired_c_mixer() {
        let table = [
            (0, 65536),
            (-1, 65461),
            (-326, 45028),
            (-602, 32770),
            (-1000, 20724),
            (-2000, 6554),
            (-6000, 66),
            (-9999, 1),
            (-10000, 0),
        ];
        for (mb, q16) in table {
            assert_eq!(volume_to_gain_q16(mb), Some(q16), "{mb}");
        }
        for mb in [1, 2000, i32::MAX, -10001, i32::MIN] {
            assert_eq!(volume_to_gain_q16(mb), None, "{mb}");
        }
    }
}
