// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline encoding of the measured XInput state report. Live dispatch remains in C.

use crate::shim::XInputPad;

/// Update a mapped guest capabilities block and return the synthetic pad status.
pub fn write_capabilities(output: &mut [u8; 25], removed: bool) -> u32 {
    output[1..3].fill(0);
    if removed {
        0x48f
    } else {
        output[0] = 1;
        output[3..].fill(0xff);
        0
    }
}

/// Offline packet counter. Feed every raw report, including reports between guest polls.
pub struct PacketCounter {
    last: XInputPad,
    packet: u32,
}

impl PacketCounter {
    /// Opening a pad starts at packet 1 with the current raw state as its baseline.
    pub fn open(current: XInputPad) -> Self {
        Self {
            last: current,
            packet: 1,
        }
    }

    pub fn observe(&mut self, report: XInputPad) -> u32 {
        if report != self.last {
            self.packet = self.packet.wrapping_add(1);
            self.last = report;
        }
        self.packet
    }
}

/// Encode the 22-byte guest state. The packet counts raw reports, before pressure filtering.
pub fn encode_state(packet: u32, pad: &XInputPad) -> [u8; 22] {
    let mut result = [0; 22];
    result[..4].copy_from_slice(&packet.to_le_bytes());
    result[4..6].copy_from_slice(&pad.digital.to_le_bytes());
    for (out, pressure) in result[6..14].iter_mut().zip(pad.analog) {
        *out = if pressure < 0x20 { 0 } else { pressure };
    }
    for (out, axis) in result[14..].chunks_exact_mut(2).zip(pad.thumbs) {
        out.copy_from_slice(&axis.to_le_bytes());
    }
    result
}

/// Encode the six-byte synthetic-pad output report from the guest motor speeds.
/// The transport consumes the same little-endian speeds independently.
pub fn encode_feedback(left: u16, right: u16) -> [u8; 6] {
    let mut report = [0, 6, 0, 0, 0, 0];
    report[2..4].copy_from_slice(&left.to_le_bytes());
    report[4..6].copy_from_slice(&right.to_le_bytes());
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_report_preserves_both_motor_speeds() {
        assert_eq!(encode_feedback(0x1234, 0xabcd), [0, 6, 0x34, 0x12, 0xcd, 0xab]);
        assert_eq!(encode_feedback(0, u16::MAX), [0, 6, 0, 0, 0xff, 0xff]);
    }

    #[test]
    fn packet_counts_raw_changes_and_reopen_resets_baseline() {
        let mut pad = XInputPad::default();
        let mut counter = PacketCounter::open(pad);
        assert_eq!(counter.observe(pad), 1);
        pad.analog[0] = 1;
        assert_eq!(counter.observe(pad), 2);
        pad.analog[0] = 2;
        assert_eq!(counter.observe(pad), 3);
        assert_eq!(
            encode_state(2, &XInputPad::default())[6],
            encode_state(3, &pad)[6]
        );
        assert_eq!(counter.observe(pad), 3);
        assert_eq!(PacketCounter::open(pad).observe(pad), 1);
    }

    #[test]
    fn encodes_guest_layout_and_threshold_boundaries() {
        let pad = XInputPad {
            digital: 0x1234,
            analog: [0, 1, 30, 31, 32, 33, 127, 255],
            thumbs: [i16::MIN, -1, 0, i16::MAX],
        };
        assert_eq!(
            encode_state(0x1234_5678, &pad),
            [
                0x78, 0x56, 0x34, 0x12, 0x34, 0x12, 0, 0, 0, 0, 32, 33, 127, 255, 0, 0x80, 0xff,
                0xff, 0, 0, 0xff, 0x7f,
            ]
        );
    }

    #[test]
    fn every_pressure_value_is_filtered_independently() {
        for value in 0..=u8::MAX {
            for index in 0..8 {
                let mut pad = XInputPad::default();
                pad.analog[index] = value;
                let bytes = encode_state(1, &pad);
                assert_eq!(bytes[6 + index], if value < 32 { 0 } else { value });
                assert_eq!(
                    bytes[6..14].iter().filter(|&&v| v != 0).count(),
                    usize::from(value >= 32)
                );
            }
        }
    }
}
