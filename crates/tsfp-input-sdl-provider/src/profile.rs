// SPDX-License-Identifier: GPL-3.0-or-later
//! Controller profile application for samples produced by the Rust SDL provider.

pub const BUTTONS: usize = 15;
pub const AXES: usize = 6;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct AxisCalibration {
    pub min: i32,
    pub center: i32,
    pub max: i32,
    pub deadzone: u32,
    pub invert: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ControllerProfile {
    pub button_map: [i32; BUTTONS],
    pub axis_map: [i32; AXES],
    pub calibration: [AxisCalibration; AXES],
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct XInputPadState {
    pub packet_number: u32,
    pub digital_buttons: u16,
    pub analog: [u8; 8],
    pub thumb_left_x: i16,
    pub thumb_left_y: i16,
    pub thumb_right_x: i16,
    pub thumb_right_y: i16,
}

fn valid(profile: &ControllerProfile) -> bool {
    if profile.button_map.iter().any(|&s| !(-1..15).contains(&s)) || profile.button_map[14] != -1 {
        return false;
    }
    for target in 0..AXES {
        let source = profile.axis_map[target];
        if source < 0 || source >= AXES as i32 || (target < 4) != (source < 4) {
            return false;
        }
        let c = profile.calibration[target];
        if c.min < if target < 4 { -32768 } else { 0 }
            || c.max > 32767
            || c.min >= c.max
            || (target < 4 && (c.center <= c.min || c.center >= c.max))
            || (target >= 4 && c.center != c.min)
            || c.deadzone >= 32767
        {
            return false;
        }
    }
    true
}

fn normalized(c: AxisCalibration, input: i16, trigger: bool) -> i16 {
    let value = (input as i32).clamp(c.min, c.max);
    let mut n = if trigger {
        (((value - c.min) as i64) * 32767 / (c.max - c.min) as i64) as i32
    } else if value >= c.center {
        (((value - c.center) as i64) * 32767 / (c.max - c.center) as i64) as i32
    } else {
        -(((c.center - value) as i64) * 32768 / (c.center - c.min) as i64) as i32
    };
    if c.invert != 0 {
        n = if trigger {
            32767 - n
        } else if n == -32768 {
            32767
        } else {
            -n
        };
    }
    let magnitude = n.abs();
    if magnitude <= c.deadzone as i32 {
        return 0;
    }
    let maximum = if n < 0 { 32768 } else { 32767 };
    n = ((magnitude - c.deadzone as i32) as i64 * maximum as i64
        / (maximum - c.deadzone as i32) as i64) as i32
        * if n < 0 { -1 } else { 1 };
    n as i16
}

pub fn apply(
    profile: &ControllerProfile,
    buttons: &[u8; BUTTONS],
    axes: &[i16; AXES],
) -> XInputPadState {
    let mut out = XInputPadState::default();
    if !valid(profile) {
        return out;
    }
    const DIGITAL: [u16; 8] = [
        0x0010, 0x0020, 0x0040, 0x0080, 0x0001, 0x0002, 0x0004, 0x0008,
    ];
    for target in 0..14 {
        let source = profile.button_map[target];
        if source < 0 || buttons[source as usize] == 0 {
            continue;
        }
        if target < 6 {
            out.analog[target] = 255;
        } else {
            out.digital_buttons |= DIGITAL[target - 6];
        }
    }
    let mut mapped = [0i16; AXES];
    for target in 0..AXES {
        let source = profile.axis_map[target] as usize;
        mapped[target] = normalized(profile.calibration[source], axes[source], target >= 4);
    }
    out.thumb_left_x = mapped[0];
    out.thumb_left_y = mapped[1];
    out.thumb_right_x = mapped[2];
    out.thumb_right_y = mapped[3];
    out.analog[6] = (mapped[4] >> 7) as u8;
    out.analog[7] = (mapped[5] >> 7) as u8;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> ControllerProfile {
        ControllerProfile {
            button_map: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, -1],
            axis_map: [0, 1, 2, 3, 4, 5],
            calibration: std::array::from_fn(|a| AxisCalibration {
                min: if a < 4 { -32768 } else { 0 },
                center: 0,
                max: 32767,
                deadzone: 0,
                invert: u8::from(a == 1 || a == 3),
            }),
        }
    }

    #[test]
    fn profile_maps_buttons_sticks_and_trigger_like_c_contract() {
        let p = defaults();
        let mut buttons = [0; BUTTONS];
        buttons[0] = 1;
        buttons[10] = 1;
        let out = apply(&p, &buttons, &[32767, -32768, 16384, 0, 32767, 0]);
        assert_eq!(out.analog[0], 255);
        assert_eq!(out.digital_buttons, 0x0001);
        assert_eq!((out.thumb_left_x, out.thumb_left_y), (32767, 32767));
        assert_eq!(out.analog[6], 255);
        assert_eq!(out.analog[7], 0);
    }

    #[test]
    fn invalid_profile_yields_zero_state() {
        let mut p = defaults();
        p.axis_map[0] = 4;
        assert_eq!(
            apply(&p, &[1; BUTTONS], &[1; AXES]),
            XInputPadState::default()
        );
    }
}
