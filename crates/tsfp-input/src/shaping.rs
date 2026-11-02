// SPDX-License-Identifier: GPL-3.0-or-later
//! T2167 stage 2, the shaping layer: raw look and move samples to what the guest integrates, with presets
//! (docs/t-input-presets.md, contract docs/t-contract-input.md section 3).
//!
//! Conventions: stick vectors are x right positive, y up positive, range [-1, 1]. Look output is degrees per step in the
//! contract sign (yaw positive = turn right, pitch positive = look up). The `original` preset is a port of the measured guest
//! chain (`0x438B0` poll, `0x42970` axis read, `0x25CE80` look pair, `0x25D520` tail); `modern` and `mouse-raw` are new.
use crate::player::{AssistMask, Vec2};

/// Original look rates, degrees per second at full look (guest `0x7B0CB8/BC`, mode 0xA) and steps per second.
pub const STEPS_PER_SECOND: f32 = 60.0;
pub const YAW_RATE_DPS: f32 = 160.0;
pub const PITCH_RATE_DPS: f32 = 68.0;
/// Original record dead zone (record word +0) and the pad poll dead band fraction (`0x475D24`).
pub const ORIGINAL_DEAD_ZONE: f32 = 0.25;
pub const POLL_DEAD_BAND: f32 = 0.1;
const DIRECTION_STEPS: f32 = 2048.0;
const EDGE_THRESHOLD: f32 = 0.95;
const EDGE_CHARGE_PER_STEP: f32 = 3.0 / 60.0;
const EDGE_LOOK_GAIN: f32 = 0.9;
const EDGE_BOOST_GAIN: f32 = 0.5;
const PITCH_DEAD_ZONE: f32 = 0.1;
/// Move slew limits per step, forward and strafe (table `0x49F2A0` row 0: 5 and 8 per second).
const SLEW_FORWARD: f32 = 5.0 / 60.0;
const SLEW_STRAFE: f32 = 8.0 / 60.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Original,
    Modern,
    MouseRaw,
}

/// A named shaping preset. Fields a kind does not use are ignored (see the table in docs/t-input-presets.md).
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    pub kind: Kind,
    /// Radial dead zone of the stick, [0, 1).
    pub dead_zone: f32,
    /// Response exponent applied to the dead-zone-rescaled magnitude.
    pub curve_exponent: f32,
    /// Yaw and pitch at full deflection, degrees per step, before the per player sensitivity.
    pub yaw_deg_per_step: f32,
    pub pitch_deg_per_step: f32,
    /// Extra gain growing with deflection: `mag * (1 + acceleration * mag)`, 0 = off.
    pub acceleration: f32,
    /// Exponential smoothing of the output, 0 = off, [0, 1).
    pub smoothing: f32,
    /// Degrees per raw mouse count at sensitivity 1 (yaw and pitch).
    pub mouse_deg_per_count: f32,
    /// Gyro degrees of look per degree of device rotation.
    pub gyro_gain: f32,
}

impl Preset {
    pub fn original() -> Preset {
        Preset {
            name: "original".into(),
            kind: Kind::Original,
            dead_zone: ORIGINAL_DEAD_ZONE,
            curve_exponent: 2.0,
            yaw_deg_per_step: YAW_RATE_DPS / STEPS_PER_SECOND,
            pitch_deg_per_step: PITCH_RATE_DPS / STEPS_PER_SECOND,
            acceleration: 0.0,
            smoothing: 0.0,
            mouse_deg_per_count: 0.022,
            gyro_gain: 1.0,
        }
    }
    /// Float resolution. The default dead zone 0.325 is the original's poll band and record dead zone folded into one
    /// radial value, `((f - 0.1) / 0.9 - 0.25) / 0.75 = (f - 0.325) / 0.675`.
    pub fn modern() -> Preset {
        Preset {
            name: "modern".into(),
            kind: Kind::Modern,
            dead_zone: 0.325,
            ..Preset::original()
        }
    }
    pub fn mouse_raw() -> Preset {
        Preset {
            name: "mouse-raw".into(),
            kind: Kind::MouseRaw,
            dead_zone: 0.0,
            curve_exponent: 1.0,
            ..Preset::original()
        }
    }
}

/// Per player settings (docs/t-input-presets.md). Sensitivity is a multiplier on the preset rates (1 = as the preset).
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSettings {
    pub preset: String,
    pub sensitivity_x: f32,
    pub sensitivity_y: f32,
    pub invert_x: bool,
    pub invert_y: bool,
    /// Overrides the preset's dead zone / curve exponent when set. Ignored by `original`.
    pub dead_zone: Option<f32>,
    pub curve_exponent: Option<f32>,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            preset: "original".into(),
            sensitivity_x: 1.0,
            sensitivity_y: 1.0,
            invert_x: false,
            invert_y: false,
            dead_zone: None,
            curve_exponent: None,
        }
    }
}

/// Pad poll `0x438B0`: float stick to the guest axis byte (int16 range, dead band 0.1, 0x80 neutral).
pub fn poll_byte(value: f32) -> u8 {
    let half: i32 = 32767;
    let v = ((value.clamp(-1.0, 1.0) * half as f32) as i32).clamp(-half, half);
    let d = (half as f32 * POLL_DEAD_BAND) as i32;
    let shifted = if v < -d {
        v + d
    } else if v > d {
        v - d
    } else {
        0
    };
    (0x80 + (shifted * 127) / (half - d)).clamp(0, 255) as u8
}

/// Axis read `0x42970`: byte to [-1, 1] with the record dead zone renormalised.
pub fn axis_float(byte: u8, dead_zone: f32) -> f32 {
    let delta = byte as i32 - 0x80;
    let a = if delta < 0 {
        delta as f32 * 0.0078125
    } else {
        delta as f32 * (1.0 / 127.0)
    };
    if a.abs() <= dead_zone {
        0.0
    } else {
        a.signum() * (a.abs() - dead_zone) / (1.0 - dead_zone)
    }
}

/// The 2048-entry direction quantiser of `0x25CE80`: returns the guest look pair (x, y) before the rates.
fn quantised_look(x: f32, y: f32) -> (f32, f32) {
    let m = x.hypot(y).min(1.0);
    if m <= 0.0 {
        return (0.0, 0.0);
    }
    let scale = DIRECTION_STEPS / std::f32::consts::TAU;
    let idx = ((y.atan2(x) * scale + 0.5) as i32) as i64;
    let angle = |i: i64| (i.rem_euclid(2048) as f64 * std::f64::consts::TAU / 2048.0).sin() as f32;
    (angle(idx + 512) * m * m, angle(idx) * m * m)
}

/// Stateful shaper of one player slot.
#[derive(Clone, Debug)]
pub struct Shaper {
    preset: Preset,
    settings: PlayerSettings,
    /// Edge boost charge per look axis (yaw, pitch), original only.
    edge: [f32; 2],
    /// Previous output of the move slew (strafe, forward), original only.
    slew: Vec2,
    smooth: Vec2,
}

impl Shaper {
    pub fn new(preset: Preset, settings: PlayerSettings) -> Shaper {
        Shaper {
            preset,
            settings,
            edge: [0.0; 2],
            slew: Vec2::ZERO,
            smooth: Vec2::ZERO,
        }
    }
    pub fn preset(&self) -> &Preset {
        &self.preset
    }
    /// Seed the owned movement slew from the typed player state at the attachment boundary.
    pub fn restore_move(&mut self, previous: Vec2) {
        self.slew = previous;
    }

    pub fn reset(&mut self) {
        self.edge = [0.0; 2];
        self.slew = Vec2::ZERO;
        self.smooth = Vec2::ZERO;
    }

    /// Bytes of the right stick (x, y) through the original chain, look degrees per step in the guest sign
    /// (yaw positive = turn LEFT, the sign the CSV rows use). `stick` goes through [`poll_byte`] first.
    pub fn original_look_bytes(&mut self, rx: u8, ry: u8, assists: AssistMask) -> Vec2 {
        let (ax, ay) = (
            axis_float(rx, ORIGINAL_DEAD_ZONE),
            axis_float(ry, ORIGINAL_DEAD_ZONE),
        );
        let (mut lx, mut ly) = quantised_look(-ax, ay);
        if assists.0 & AssistMask::PITCH_DEAD_ZONE != 0 {
            ly = if ly.abs() <= PITCH_DEAD_ZONE {
                0.0
            } else {
                ly.signum() * (ly.abs() - PITCH_DEAD_ZONE) / (1.0 - PITCH_DEAD_ZONE)
            };
        }
        if assists.0 & AssistMask::INVERSE_LOOK != 0 {
            ly = -ly;
        }
        if assists.0 & AssistMask::EDGE_BOOST != 0 {
            for (value, charge) in [&mut lx, &mut ly].into_iter().zip(self.edge.iter_mut()) {
                *charge = if value.abs() > EDGE_THRESHOLD {
                    (*charge + EDGE_CHARGE_PER_STEP).min(1.0)
                } else {
                    0.0
                };
                *value = *value * EDGE_LOOK_GAIN + EDGE_BOOST_GAIN * *charge * value.signum();
            }
        }
        Vec2::new(self.preset_rate_yaw() * lx, self.preset_rate_pitch() * ly)
    }
    fn preset_rate_yaw(&self) -> f32 {
        YAW_RATE_DPS / STEPS_PER_SECOND
    }
    fn preset_rate_pitch(&self) -> f32 {
        PITCH_RATE_DPS / STEPS_PER_SECOND
    }

    /// Right stick to look degrees per step (contract sign). `assists` only matters for `original`.
    pub fn shape_stick(&mut self, stick: Vec2, assists: AssistMask) -> Vec2 {
        match self.preset.kind {
            Kind::Original => {
                let guest =
                    self.original_look_bytes(poll_byte(stick.x), poll_byte(stick.y), assists);
                Vec2::new(-guest.x, guest.y)
            }
            Kind::Modern | Kind::MouseRaw => {
                let dead_zone = self
                    .settings
                    .dead_zone
                    .unwrap_or(self.preset.dead_zone)
                    .clamp(0.0, 0.99);
                let exponent = self
                    .settings
                    .curve_exponent
                    .unwrap_or(self.preset.curve_exponent)
                    .clamp(0.1, 8.0);
                let m = stick.x.hypot(stick.y);
                let mut out = Vec2::ZERO;
                if m > dead_zone && m > 0.0 {
                    let scaled = ((m - dead_zone) / (1.0 - dead_zone)).min(1.0);
                    let mag = scaled.powf(exponent) * (1.0 + self.preset.acceleration * scaled);
                    out = Vec2::new(
                        stick.x / m * mag * self.preset.yaw_deg_per_step,
                        stick.y / m * mag * self.preset.pitch_deg_per_step,
                    );
                }
                self.finish(out)
            }
        }
    }

    /// Raw mouse counts of one step to look degrees (contract sign: dx right positive, dy up positive).
    pub fn shape_counts(&mut self, dx: f64, dy: f64) -> Vec2 {
        let out = Vec2::new(
            dx as f32 * self.preset.mouse_deg_per_count,
            dy as f32 * self.preset.mouse_deg_per_count,
        );
        self.finish(out)
    }

    /// Applies sensitivity, inversion and smoothing (the latter two never to `original`, which returns early).
    fn finish(&mut self, out: Vec2) -> Vec2 {
        let sx = if self.settings.invert_x { -1.0 } else { 1.0 } * self.settings.sensitivity_x;
        let sy = if self.settings.invert_y { -1.0 } else { 1.0 } * self.settings.sensitivity_y;
        let target = Vec2::new(out.x * sx, out.y * sy);
        let s = self.preset.smoothing.clamp(0.0, 0.99);
        if s > 0.0 {
            self.smooth = Vec2::new(
                self.smooth.x * s + target.x * (1.0 - s),
                self.smooth.y * s + target.y * (1.0 - s),
            );
            self.smooth
        } else {
            target
        }
    }

    /// Move target before the original slew (x strafe, y forward). Non-original kinds
    /// have no slew, so their target is the shaped move itself.
    pub fn move_target(&mut self, stick: Vec2) -> Vec2 {
        match self.preset.kind {
            Kind::Original => Vec2::new(
                axis_float(poll_byte(stick.x), ORIGINAL_DEAD_ZONE),
                axis_float(poll_byte(stick.y), ORIGINAL_DEAD_ZONE),
            ),
            _ => self.shape_move(stick),
        }
    }

    /// Left stick (x strafe right positive, y forward positive) to the move vector.
    pub fn shape_move(&mut self, stick: Vec2) -> Vec2 {
        match self.preset.kind {
            Kind::Original => {
                let target = Vec2::new(
                    axis_float(poll_byte(stick.x), ORIGINAL_DEAD_ZONE),
                    axis_float(poll_byte(stick.y), ORIGINAL_DEAD_ZONE),
                );
                let step = |prev: f32, to: f32, limit: f32| prev + (to - prev).clamp(-limit, limit);
                self.slew = Vec2::new(
                    step(self.slew.x, target.x, SLEW_STRAFE),
                    step(self.slew.y, target.y, SLEW_FORWARD),
                );
                self.slew
            }
            Kind::Modern => {
                let dead_zone = self
                    .settings
                    .dead_zone
                    .unwrap_or(self.preset.dead_zone)
                    .clamp(0.0, 0.99);
                let m = stick.x.hypot(stick.y);
                if m <= dead_zone || m <= 0.0 {
                    return Vec2::ZERO;
                }
                let scaled = ((m - dead_zone) / (1.0 - dead_zone)).min(1.0);
                Vec2::new(stick.x / m * scaled, stick.y / m * scaled)
            }
            Kind::MouseRaw => stick,
        }
    }
}

/// One gyro reading: angular rates in degrees per second (yaw positive = turn right, pitch positive = look up).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct GyroSample {
    pub yaw_dps: f32,
    pub pitch_dps: f32,
}

/// Device trait for an optional gyro source (a pad or phone IMU). No hardware dependency lives in this crate.
pub trait GyroSource {
    /// Latest sample for `slot` since the last call, `None` when the slot has no gyro.
    fn read(&mut self, slot: u8) -> Option<GyroSample>;
}

/// A gyro sample over `dt_seconds` to look degrees, through the preset gain and the player's sensitivity and inversion.
pub fn gyro_look(
    sample: GyroSample,
    dt_seconds: f32,
    preset: &Preset,
    settings: &PlayerSettings,
) -> Vec2 {
    let sx = if settings.invert_x { -1.0 } else { 1.0 } * settings.sensitivity_x;
    let sy = if settings.invert_y { -1.0 } else { 1.0 } * settings.sensitivity_y;
    Vec2::new(
        sample.yaw_dps * dt_seconds * preset.gyro_gain * sx,
        sample.pitch_dps * dt_seconds * preset.gyro_gain * sy,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Row {
        signal: String,
        byte: u8,
        measured: f32,
        model: f32,
    }

    fn rows(text: &str) -> Vec<Row> {
        text.lines()
            .skip(1)
            .filter_map(|line| {
                let f: Vec<&str> = line.splitn(7, ',').collect();
                Some(Row {
                    signal: f[0].to_string(),
                    byte: f[1].parse().ok()?,
                    measured: f[3].parse().unwrap_or(f32::NAN),
                    model: f[4].parse().ok()?,
                })
            })
            .collect()
    }

    const BASELINE: &str = include_str!("../../../docs/data/t2159-input-response.csv");
    const CONTROL: &str = include_str!("../../../docs/data/t2159-input-response-control.csv");

    /// Steady state of `steps` identical samples (the edge boost needs 20 to charge).
    fn steady(rx: u8, ry: u8, assists: AssistMask) -> Vec2 {
        let mut shaper = Shaper::new(Preset::original(), PlayerSettings::default());
        let mut out = Vec2::ZERO;
        for _ in 0..40 {
            out = shaper.original_look_bytes(rx, ry, assists);
        }
        out
    }

    fn check_single_axis(csv: &str, assists: AssistMask) -> usize {
        let mut checked = 0;
        for row in rows(csv) {
            let (yaw, pitch) = (
                row.signal.starts_with("look_yaw_deg"),
                row.signal.starts_with("look_pitch_deg"),
            );
            if !(yaw || pitch) {
                continue;
            }
            let out = if yaw {
                steady(row.byte, 0x80, assists).x
            } else {
                steady(0x80, row.byte, assists).y
            };
            assert!(
                (out - row.model).abs() < 1e-5,
                "{} byte {}: {out} vs model {}",
                row.signal,
                row.byte,
                row.model
            );
            if row.measured.is_finite() {
                assert!(
                    (out - row.measured).abs() < 1e-5 + 1e-5,
                    "{} byte {}: {out} vs measured {}",
                    row.signal,
                    row.byte,
                    row.measured
                );
            }
            checked += 1;
        }
        checked
    }

    #[test]
    fn original_reproduces_the_baseline_csv() {
        // Baseline run: Inverse Look on, word +0x74 == 2 (edge boost on, no pitch dead zone).
        let assists = AssistMask(AssistMask::INVERSE_LOOK | AssistMask::EDGE_BOOST);
        let checked = check_single_axis(BASELINE, assists);
        assert!(checked >= 12, "{checked}");
    }

    #[test]
    fn original_reproduces_the_control_csv() {
        // Control run: flags 0 and word +0x74 0 (pitch dead zone on, no boost, no invert).
        let checked = check_single_axis(CONTROL, AssistMask(AssistMask::PITCH_DEAD_ZONE));
        assert!(checked >= 8, "{checked}");
    }

    #[test]
    fn original_reproduces_the_diagonal_rows_and_the_move_rows() {
        let assists = AssistMask(AssistMask::INVERSE_LOOK | AssistMask::EDGE_BOOST);
        for row in rows(BASELINE) {
            if let Some(pair) = row.signal.strip_prefix("look_diag_") {
                let (kind, rest) = pair.split_once('_').unwrap();
                let parts: Vec<u8> = rest.split('_').flat_map(|p| p[2..].parse()).collect();
                let out = steady(parts[0], parts[1], assists);
                let got = if kind == "yaw" { out.x } else { out.y };
                if row.measured.is_finite() {
                    assert!(
                        (got - row.model).abs() < 1e-5,
                        "{}: {got} vs {}",
                        row.signal,
                        row.model
                    );
                }
            }
            if row.signal.starts_with("move_vector") {
                let a = axis_float(row.byte, ORIGINAL_DEAD_ZONE);
                assert!(
                    (a - row.model).abs() < 1e-5,
                    "{}: {a} vs {}",
                    row.signal,
                    row.model
                );
            }
        }
    }

    #[test]
    fn poll_bytes_match_the_measured_levels() {
        let expect = [
            (15000, 178),
            (18000, 191),
            (21000, 204),
            (24000, 217),
            (27000, 230),
            (30000, 243),
            (32767, 255),
            (-8000, 108),
            (-32767, 1),
            (3000, 128),
        ];
        for (v, byte) in expect {
            assert_eq!(poll_byte(v as f32 / 32767.0), byte, "{v}");
        }
    }

    #[test]
    fn edge_boost_charges_by_twenty_steps_and_slew_limits_the_move() {
        let mut shaper = Shaper::new(Preset::original(), PlayerSettings::default());
        let assists = AssistMask(AssistMask::INVERSE_LOOK | AssistMask::EDGE_BOOST);
        let first = shaper.original_look_bytes(255, 0x80, assists).x;
        let second = shaper.original_look_bytes(255, 0x80, assists).x;
        assert!(
            (first + 2.4 + 0.5 * 0.05 * 2.6666667).abs() < 1e-4,
            "{first}"
        );
        assert!((second - first + 0.5 * 0.05 * 2.6666667).abs() < 1e-4);
        let mut steps = 0;
        let mut last = 0.0;
        while last < 0.9999 {
            last = shaper.shape_move(Vec2::new(0.0, 1.0)).y;
            steps += 1;
        }
        assert_eq!(steps, 12, "12 steps from 0 to 1 forward");
    }

    #[test]
    fn mouse_raw_is_counts_times_sensitivity() {
        let settings = PlayerSettings {
            sensitivity_x: 2.0,
            ..PlayerSettings::default()
        };
        let mut shaper = Shaper::new(Preset::mouse_raw(), settings);
        let out = shaper.shape_counts(10.0, -4.0);
        assert!((out.x - 0.44).abs() < 1e-6 && (out.y + 0.088).abs() < 1e-6);
    }

    #[test]
    fn modern_approximates_original_without_assists() {
        let (mut axis_worst, mut grid_worst) = (0.0f32, 0.0f32);
        let n = 40;
        for ix in -n..=n {
            for iy in -n..=n {
                let stick = Vec2::new(ix as f32 / n as f32, iy as f32 / n as f32);
                let mut original = Shaper::new(Preset::original(), PlayerSettings::default());
                let mut modern = Shaper::new(Preset::modern(), PlayerSettings::default());
                let (a, b) = (
                    original.shape_stick(stick, AssistMask::NONE),
                    modern.shape_stick(stick, AssistMask::NONE),
                );
                let diff = (a.x - b.x).abs().max((a.y - b.y).abs());
                grid_worst = grid_worst.max(diff);
                if ix == 0 || iy == 0 {
                    axis_worst = axis_worst.max(diff);
                }
            }
        }
        eprintln!("modern vs original: axis {axis_worst}, grid {grid_worst}");
        assert!(axis_worst < MODERN_AXIS_TOLERANCE_DEG, "axis {axis_worst}");
        assert!(grid_worst < MODERN_GRID_TOLERANCE_DEG, "grid {grid_worst}");
    }

    /// Documented in docs/t-input-presets.md: max |modern - original| per axis, degrees per step, no assists.
    /// Axis aligned pushes differ only by the byte quantisation, diagonals by the original's per axis dead band.
    pub const MODERN_AXIS_TOLERANCE_DEG: f32 = 0.12;
    pub const MODERN_GRID_TOLERANCE_DEG: f32 = 0.9;

    #[test]
    fn presets_are_monotone_and_bounded() {
        for preset in [Preset::original(), Preset::modern(), Preset::mouse_raw()] {
            let mut previous = 0.0f32;
            for i in 0..=200 {
                let x = i as f32 / 200.0;
                let mut shaper = Shaper::new(preset.clone(), PlayerSettings::default());
                let out = shaper.shape_stick(Vec2::new(x, 0.0), AssistMask::NONE);
                assert!(
                    out.x >= previous - 1e-5,
                    "{} not monotone at {x}",
                    preset.name
                );
                assert!(
                    out.x <= preset.yaw_deg_per_step + 1e-4 && out.y.abs() < 1e-4,
                    "{} unbounded at {x}: {out:?}",
                    preset.name
                );
                previous = out.x;
            }
            let mut shaper = Shaper::new(preset.clone(), PlayerSettings::default());
            let out = shaper.shape_stick(Vec2::new(0.0, 1.0), AssistMask::NONE);
            assert!(out.y <= preset.pitch_deg_per_step + 1e-4);
        }
    }

    #[test]
    fn modern_options_apply() {
        let settings = PlayerSettings {
            invert_y: true,
            sensitivity_y: 0.5,
            dead_zone: Some(0.0),
            curve_exponent: Some(1.0),
            ..PlayerSettings::default()
        };
        let mut shaper = Shaper::new(Preset::modern(), settings);
        let out = shaper.shape_stick(Vec2::new(0.0, 1.0), AssistMask::NONE);
        assert!((out.y + 0.5 * 68.0 / 60.0).abs() < 1e-5);
        let smooth = Preset {
            smoothing: 0.5,
            ..Preset::modern()
        };
        let mut shaper = Shaper::new(smooth, PlayerSettings::default());
        let a = shaper.shape_stick(Vec2::new(1.0, 0.0), AssistMask::NONE).x;
        let b = shaper.shape_stick(Vec2::new(1.0, 0.0), AssistMask::NONE).x;
        assert!(b > a && (a - 0.5 * 160.0 / 60.0).abs() < 1e-5);
    }

    #[test]
    fn gyro_look_scales_rate_by_time_gain_and_sensitivity() {
        let settings = PlayerSettings {
            sensitivity_x: 2.0,
            invert_y: true,
            ..PlayerSettings::default()
        };
        let out = gyro_look(
            GyroSample {
                yaw_dps: 30.0,
                pitch_dps: 10.0,
            },
            1.0 / 60.0,
            &Preset::modern(),
            &settings,
        );
        assert!((out.x - 1.0).abs() < 1e-6 && (out.y + 10.0 / 60.0).abs() < 1e-6);
    }
}
