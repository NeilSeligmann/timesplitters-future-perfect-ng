// SPDX-License-Identifier: GPL-3.0-or-later
//! The compatibility shim (docs/t-contract-input.md section 4): `PlayerInput` back into the old pad world the guest still reads.
//!
//! Three views of the same state, each pinned by tests:
//! * [`SdlReport`] and [`ShimEvent`]: what the Bevy shell feeds into the `tsfp_wi_*` queue (SDL3 button indices, the SDL stick
//!   convention with Y negated, triggers as axes 4 and 5). This is the live path. It is exactly what `tsfp-bevy/src/events.rs`
//!   produced from raw gamepad events before the router existed.
//! * [`XInputPad`]: the XInput state `xinput_host_source.c` derives from the queue (digital bits, analog bytes, int16 thumbs).
//! * [`PadRecord`]: the 0x3C byte physical pad record words of docs/t-contract-input.md 4.1 (`0x18FF70` dead band byte
//!   conversion, button tables, edges), for the later replaced readers and for the trace comparison.
use crate::player::{ButtonBits, Look, PlayerInput, Vec2, bits};

/// Float -1..1 to the SDL int16 range with no dead zone (identical to the shell's `events::to_i16`).
pub fn to_i16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

/// Guest button bit of an SDL3 gamepad button index (0 A, 1 B, 2 X, 3 Y, 4 BACK, 5 GUIDE, 6 START, 7 LSTICK, 8 RSTICK,
/// 9 LB = BLACK, 10 RB = WHITE, 11..14 DPAD up, down, left, right). GUIDE has no bit.
pub const SDL_BUTTON_BITS: [ButtonBits; 15] = [
    bits::A,
    bits::B,
    bits::X,
    bits::Y,
    bits::BACK,
    0,
    bits::START,
    bits::L3,
    bits::R3,
    bits::BLACK,
    bits::WHITE,
    bits::DPAD_UP,
    bits::DPAD_DOWN,
    bits::DPAD_LEFT,
    bits::DPAD_RIGHT,
];

pub const AXIS_COUNT: usize = 6; // LX LY RX RY LT RT

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShimEvent {
    ButtonDown(u8),
    ButtonUp(u8),
    /// SDL convention: Y axes (1 and 3) already negated, the window input state negates them back.
    Axis {
        axis: u8,
        value: i16,
    },
}

/// The pad state as the queue sees it. `None` axis = nothing sent yet (the original path sends the first value even when 0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SdlReport {
    pub buttons: u16,
    pub axes: [Option<i16>; AXIS_COUNT],
}

/// Events that turn `sent` into `target`, `sent` updated. `strict_none`: a never sent axis counts as different from 0.
pub fn diff(sent: &mut SdlReport, target: &SdlReport, out: &mut Vec<ShimEvent>) {
    for index in 0..15u8 {
        let mask = 1u16 << index;
        match (sent.buttons & mask != 0, target.buttons & mask != 0) {
            (false, true) => out.push(ShimEvent::ButtonDown(index)),
            (true, false) => out.push(ShimEvent::ButtonUp(index)),
            _ => {}
        }
    }
    sent.buttons = target.buttons;
    for axis in 0..AXIS_COUNT {
        if let Some(value) = target.axes[axis] {
            // The original path sends the first value, including an initial zero.
            if sent.axes[axis] != Some(value) {
                out.push(ShimEvent::Axis {
                    axis: axis as u8,
                    value,
                });
                sent.axes[axis] = Some(value);
            }
        }
    }
}

/// Interim look for a mouse slot until the raw look path (T2161): the virtual right stick that makes the ORIGINAL shaper
/// (dead zone 0.25 renormalised, squared, 160 and 68 degrees per second at 60 steps, 10 percent byte dead band) turn by about
/// the requested degrees this step. Inverse of docs/t-contract-input.md 3.2 without the edge boost, so a full push overshoots
/// by the boost and the profile's Inverse Look still applies. INFERRED approximation, labelled EXPERIMENTAL.
pub fn interim_look_stick(yaw_deg: f32, pitch_deg: f32) -> Vec2 {
    fn inverse(deg: f32, full_deg_per_step: f32) -> f32 {
        let m = (deg.abs() / full_deg_per_step).sqrt().min(1.0);
        if m <= 0.0 {
            return 0.0;
        }
        let axis = 0.25 + m * 0.75; // undo the 0.25 record dead zone
        let value = (3276.0 + axis * 29491.0) / 32767.0; // undo the 10 percent byte dead band
        value.min(1.0).copysign(deg)
    }
    Vec2::new(
        inverse(yaw_deg, 160.0 / 60.0),
        inverse(pitch_deg, 68.0 / 60.0),
    )
}

/// The queue state of a `PlayerInput`.
pub fn report_from_player(input: &PlayerInput) -> SdlReport {
    let mut buttons = 0u16;
    for (index, bit) in SDL_BUTTON_BITS.iter().enumerate() {
        if *bit != 0 && input.held & bit != 0 {
            buttons |= 1 << index;
        }
    }
    let look = match input.look {
        Look::Stick(stick) => stick,
        Look::Delta { yaw_deg, pitch_deg } => interim_look_stick(yaw_deg, pitch_deg),
    };
    SdlReport {
        buttons,
        axes: [
            Some(to_i16(input.move_axis.x)),
            Some(to_i16(-input.move_axis.y)),
            Some(to_i16(look.x)),
            Some(to_i16(-look.y)),
            Some(to_i16(input.triggers[0].max(0.0))),
            Some(to_i16(input.triggers[1].max(0.0))),
        ],
    }
}

/// XInput state as `xinput_host_source.c` builds it from the queue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct XInputPad {
    /// XINPUT_GAMEPAD_*: dpad up 1, down 2, left 4, right 8, start 0x10, back 0x20, L3 0x40, R3 0x80.
    pub digital: u16,
    /// 0 A, 1 B, 2 X, 3 Y, 4 BLACK, 5 WHITE, 6 LT, 7 RT.
    pub analog: [u8; 8],
    /// LX, LY, RX, RY, up positive.
    pub thumbs: [i16; 4],
}

fn negate(value: i16) -> i16 {
    value.saturating_neg()
}

pub fn xinput_from_report(report: &SdlReport) -> XInputPad {
    let mut pad = XInputPad::default();
    for index in 0..15u8 {
        if report.buttons & (1 << index) == 0 {
            continue;
        }
        match index {
            0 => pad.analog[0] = 255,
            1 => pad.analog[1] = 255,
            2 => pad.analog[2] = 255,
            3 => pad.analog[3] = 255,
            4 => pad.digital |= 0x20,
            6 => pad.digital |= 0x10,
            7 => pad.digital |= 0x40,
            8 => pad.digital |= 0x80,
            9 => pad.analog[4] = 255,
            10 => pad.analog[5] = 255,
            11 => pad.digital |= 1,
            12 => pad.digital |= 2,
            13 => pad.digital |= 4,
            14 => pad.digital |= 8,
            _ => {}
        }
    }
    let axis = |at: usize| report.axes[at].unwrap_or(0);
    pad.thumbs = [axis(0), negate(axis(1)), axis(2), negate(axis(3))];
    for trigger in 0..2 {
        let host = axis(4 + trigger);
        pad.analog[6 + trigger] = if host <= 0 { 0 } else { (host >> 7) as u8 };
    }
    pad
}

pub fn xinput_from_player(input: &PlayerInput) -> XInputPad {
    xinput_from_report(&report_from_player(input))
}

/// `0x18FF70` stick conversion (MEASURED bytes, docs/t-contract-input.md 3.2.1): 10 percent dead band, then 127 steps each way.
pub fn axis_byte(value: i16) -> u32 {
    let half = 32767i32;
    let band = half / 10; // trunc(half * 0.1) = 3276
    let v = i32::from(value);
    let shifted = if v < -band {
        v + band
    } else if v > band {
        v - band
    } else {
        0
    };
    (0x80 + shifted * 127 / (half - band)).clamp(0, 255) as u32
}

/// T2174 MEASURED hybrid trigger words: 10% band, centred at 128.
/// The pad range byte is 255; unlike sticks, trigger inputs are unsigned.
pub fn trigger_byte(value: u8) -> u32 {
    let band = 25;
    let shifted = (i32::from(value) - band).max(0);
    (128 + shifted * 127 / (255 - band)) as u32
}

const DIGITAL_TO_BIT: [(u16, ButtonBits); 8] = [
    (0x1, bits::DPAD_UP),
    (0x2, bits::DPAD_DOWN),
    (0x4, bits::DPAD_LEFT),
    (0x8, bits::DPAD_RIGHT),
    (0x10, bits::START),
    (0x20, bits::BACK),
    (0x40, bits::L3),
    (0x80, bits::R3),
];
const ANALOG_TO_BIT: [ButtonBits; 8] = [
    bits::A,
    bits::B,
    bits::X,
    bits::Y,
    bits::BLACK,
    bits::WHITE,
    bits::LT,
    bits::RT,
];
/// The title's own rule: an analog button is down when its 0..255 value is greater than 0x3C.
pub const ANALOG_THRESHOLD: u8 = 0x3C;

/// Held word of an XInput state.
pub fn held_bits(pad: &XInputPad) -> ButtonBits {
    let mut held = 0;
    for (mask, bit) in DIGITAL_TO_BIT {
        if pad.digital & mask != 0 {
            held |= bit;
        }
    }
    for (index, bit) in ANALOG_TO_BIT.iter().enumerate() {
        if pad.analog[index] > ANALOG_THRESHOLD {
            held |= bit;
        }
    }
    held
}

/// One physical pad record, 0x3C bytes as 15 words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PadRecord {
    pub words: [u32; 15],
}

impl Default for PadRecord {
    fn default() -> Self {
        Self::new()
    }
}

impl PadRecord {
    pub const DEAD_ZONE: usize = 0; // +0x00 f32 0.25
    pub const CONNECTED: usize = 1; // +0x04
    pub const PREVIOUS: usize = 2; // +0x08
    pub const HELD: usize = 3; // +0x0C
    pub const PRESSED: usize = 4; // +0x10
    pub const RELEASED: usize = 5; // +0x14
    pub const AXES: usize = 6; // +0x18 .. +0x30, seven dwords: [+0x18, LX, LY, RX, RY, +0x2C, +0x30]

    /// At rest, connected: dead zone 0.25, axes 0x80, nothing held (MEASURED rest state, section 3.5).
    pub fn new() -> Self {
        let mut words = [0u32; 15];
        words[Self::DEAD_ZONE] = 0.25f32.to_bits();
        words[Self::CONNECTED] = 1;
        for axis in 0..7 {
            words[Self::AXES + axis] = 0x80;
        }
        Self { words }
    }

    /// One poll. `+0x34` (rumble) and `+0x38` (consumed bits) belong to the guest and stay.
    pub fn update(&mut self, pad: &XInputPad) {
        let held = held_bits(pad);
        let previous = self.words[Self::HELD];
        self.words[Self::PREVIOUS] = previous;
        self.words[Self::HELD] = held;
        self.words[Self::PRESSED] = held & !previous;
        self.words[Self::RELEASED] = previous & !held;
        for (at, value) in pad.thumbs.iter().enumerate() {
            self.words[Self::AXES + 1 + at] = axis_byte(*value);
        }
        self.words[Self::AXES + 5] = trigger_byte(pad.analog[6]);
        self.words[Self::AXES + 6] = trigger_byte(pad.analog[7]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{AssistMask, MenuIntent, SourceKind};

    fn pad_input(held: ButtonBits, mv: Vec2, look: Look, triggers: [f32; 2]) -> PlayerInput {
        PlayerInput {
            step: 0,
            slot: 0,
            source: SourceKind::Pad(0),
            held,
            pressed: 0,
            released: 0,
            move_axis: mv,
            look,
            triggers,
            menu: MenuIntent::default(),
            assists: AssistMask::PROFILE,
        }
    }

    #[test]
    fn axis_byte_matches_the_measured_levels() {
        for (value, byte) in [
            (15000, 178),
            (18000, 191),
            (21000, 204),
            (24000, 217),
            (27000, 230),
            (30000, 243),
            (32767, 255),
            (-8000, 108),
            (-32767, 1),
            (3276, 0x80),
            (-3276, 0x80),
            (0, 0x80),
        ] {
            assert_eq!(axis_byte(value), byte, "{value}");
        }
        assert_eq!(axis_byte(i16::MIN), 1);
    }

    #[test]
    fn button_tables_match_the_measured_words() {
        let mut pad = XInputPad::default();
        assert_eq!(held_bits(&pad), 0);
        pad.digital = 0xFF;
        assert_eq!(
            held_bits(&pad),
            0x4 | 0x8 | 0x1 | 0x2 | 0x200 | 0x100 | 0x1000 | 0x2000
        );
        pad.digital = 0;
        for (index, bit) in [0x80u32, 0x20, 0x10, 0x40, 0x4000, 0x8000, 0x400, 0x800]
            .into_iter()
            .enumerate()
        {
            pad.analog = [0; 8];
            pad.analog[index] = 0x3C;
            assert_eq!(held_bits(&pad), 0, "0x3C is not greater than 0x3C");
            pad.analog[index] = 0x3D;
            assert_eq!(held_bits(&pad), bit);
        }
    }

    #[test]
    fn record_edges_and_rest_state() {
        let mut record = PadRecord::new();
        assert_eq!(record.words[0], 0x3E80_0000);
        let mut pad = XInputPad::default();
        record.update(&pad);
        assert_eq!(record.words[PadRecord::HELD], 0);
        assert_eq!(record.words[PadRecord::AXES + 1], 0x80);
        pad.analog[0] = 255;
        pad.thumbs[3] = 32767;
        record.update(&pad);
        assert_eq!(
            (record.words[3], record.words[4], record.words[5]),
            (0x80, 0x80, 0)
        );
        assert_eq!(record.words[PadRecord::AXES + 4], 255);
        pad.analog[0] = 0;
        record.update(&pad);
        assert_eq!(
            (
                record.words[2],
                record.words[3],
                record.words[4],
                record.words[5]
            ),
            (0x80, 0, 0, 0x80)
        );
    }

    #[test]
    fn player_input_round_trips_to_the_queue_state() {
        let input = pad_input(
            bits::A | bits::BLACK | bits::DPAD_LEFT,
            Vec2::new(-0.5, 1.0),
            Look::Stick(Vec2::new(0.25, -1.0)),
            [1.0, 0.0],
        );
        let report = report_from_player(&input);
        assert_eq!(report.buttons, 1 | 1 << 9 | 1 << 13);
        assert_eq!(
            report.axes,
            [
                Some(-16384),
                Some(-32767),
                Some(8192),
                Some(32767),
                Some(32767),
                Some(0)
            ]
        );
        let xi = xinput_from_report(&report);
        assert_eq!(xi.thumbs, [-16384, 32767, 8192, -32767]);
        assert_eq!(xi.analog[6], 255);
        assert_eq!(
            held_bits(&xi),
            bits::A | bits::BLACK | bits::DPAD_LEFT | bits::LT
        );
    }

    #[test]
    fn diff_resyncs_and_ignores_unchanged() {
        let mut sent = SdlReport::default();
        let mut target = SdlReport {
            buttons: 0b101,
            axes: [Some(0), None, Some(5), None, None, None],
        };
        let mut out = Vec::new();
        diff(&mut sent, &target, &mut out);
        assert_eq!(
            out,
            vec![
                ShimEvent::ButtonDown(0),
                ShimEvent::ButtonDown(2),
                ShimEvent::Axis { axis: 0, value: 0 },
                ShimEvent::Axis { axis: 2, value: 5 }
            ]
        );
        out.clear();
        target.buttons = 0b100;
        diff(&mut sent, &target, &mut out);
        assert_eq!(out, vec![ShimEvent::ButtonUp(0)]);
    }

    #[test]
    fn interim_stick_is_monotonic_and_reaches_the_original_rate() {
        let mut last = 0.0;
        for step in 1..=40 {
            let stick = interim_look_stick(step as f32 * 0.0667, 0.0).x;
            assert!(stick >= last && stick <= 1.0);
            last = stick;
        }
        assert_eq!(interim_look_stick(-100.0, 0.0).x, -1.0);
        assert_eq!(interim_look_stick(0.0, 0.0), Vec2::ZERO);
        // Feed the stick through the ORIGINAL chain (3.2.1 bytes, 3.2.2 axis, 3.2.3 square): the turn is about the request.
        let want = 0.8f32;
        let stick = interim_look_stick(want, 0.0).x;
        let byte = axis_byte(to_i16(stick));
        let axis = ((byte as f32 - 128.0) / 127.0 - 0.25) / 0.75;
        let turn = axis * axis * 160.0 / 60.0;
        assert!((turn - want).abs() < 0.08, "{turn}");
    }
}

#[cfg(test)]
mod t2174_measurements {
    use super::*;
    #[test]
    fn trigger_words_match_hybrid_sweep_and_strict_button_threshold() {
        for (value, word) in [
            (0, 128),
            (1, 128),
            (59, 146),
            (60, 147),
            (61, 147),
            (127, 184),
            (128, 184),
            (254, 254),
            (255, 255),
        ] {
            let mut pad = XInputPad::default();
            pad.analog[6] = value;
            let mut record = PadRecord::new();
            record.update(&pad);
            assert_eq!(record.words[11], word);
            assert_eq!(record.words[12], 128);
            assert_eq!(record.words[3] & bits::LT != 0, value > 60);
            pad.analog[6] = 0;
            pad.analog[7] = value;
            record.update(&pad);
            assert_eq!(record.words[11], 128);
            assert_eq!(record.words[12], word);
        }
    }
}
