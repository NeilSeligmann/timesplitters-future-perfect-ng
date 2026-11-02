// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable input boundary. All original-address functions here are Tier 0 (T1904).
//! Runtime integration is opt-in; existing guest-memory leaves remain available.
#[derive(Clone, Debug)]
pub struct PadRecord {
    pub deadzone: f32,  // +0
    pub connected: u32, // +4
    pub held: u32,      // +0xc
    pub pressed: u32,   // +0x10
    pub released: u32,  // +0x14
    pub axes: [u32; 7], // +0x18..+0x30, neutral 128
    pub consumed: u32,  // +0x38
}
impl Default for PadRecord {
    fn default() -> Self {
        Self {
            deadzone: 0.0,
            connected: 0,
            held: 0,
            pressed: 0,
            released: 0,
            axes: [128; 7],
            consumed: 0,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct PadState {
    /// Original 6b9888: true suppresses reads and mapping rebuild.
    pub locked: bool,
    /// Original 76b0fc: alternate records use logical indices directly.
    pub alternate: bool,
    pub mapping: [usize; 4],
    pub physical: [PadRecord; 4],
    pub remote: [PadRecord; 4],
    pub connected_count: usize,
}
/// Supplies the current logical pad state to gameplay and UI consumers.
///
/// The owner updates physical or remote records, then rebuilds the mapping after
/// a connection change. A read lock hides button and axis reads but does not
/// prevent consumption or neutral reset.
///
/// ```
/// use tsfp_engine_input::pad_state::{PadProvider, PadState};
/// let mut state = PadState::default();
/// state.physical[2].connected = 1;
/// state.physical[2].held = 0b11;
/// assert_eq!(state.rebuild_mapping(), 1);
/// assert_eq!(state.pad_state().available_buttons(0), 0b11);
/// state.consume(0, 0b01);
/// assert_eq!(state.pad_state().available_buttons(0), 0b10);
/// ```
pub trait PadProvider {
    fn pad_state(&self) -> &PadState;
}
impl PadProvider for PadState {
    fn pad_state(&self) -> &PadState {
        self
    }
}
impl PadState {
    /// 426d0. Caller must supply a valid logical port.
    pub fn mapped_port(&self, pad: usize) -> usize {
        self.mapping[pad]
    }
    /// 42710. Physical connection word (ungated).
    pub fn connection_word(&self, pad: usize) -> u32 {
        self.physical[pad].connected
    }
    /// 42720. Rebuild only occupied prefix; unused mapping slots retain old values.
    pub fn rebuild_mapping(&mut self) -> usize {
        if self.locked {
            return 0;
        }
        let mut count = 0;
        for (port, record) in self.physical.iter().enumerate() {
            if record.connected != 0 {
                self.mapping[count] = port;
                count += 1;
            }
        }
        self.connected_count = count;
        count
    }
    fn record(&self, pad: usize) -> &PadRecord {
        if self.alternate {
            &self.remote[pad]
        } else {
            &self.physical[self.mapping[pad]]
        }
    }
    fn record_mut(&mut self, pad: usize) -> &mut PadRecord {
        if self.alternate {
            &mut self.remote[pad]
        } else {
            &mut self.physical[self.mapping[pad]]
        }
    }
    /// 42760. Held buttons excluding the consumed mask.
    pub fn available_buttons(&self, pad: usize) -> u32 {
        if self.locked {
            0
        } else {
            let r = self.record(pad);
            r.held & !r.consumed
        }
    }
    /// 427b0.
    pub fn pressed_buttons(&self, pad: usize) -> u32 {
        if self.locked {
            0
        } else {
            self.record(pad).pressed
        }
    }
    /// 427f0.
    pub fn released_buttons(&self, pad: usize) -> u32 {
        if self.locked {
            0
        } else {
            self.record(pad).released
        }
    }
    /// 42830. Mutation is not suppressed by the read lock.
    pub fn consume(&mut self, pad: usize, mask: u32) {
        let r = self.record_mut(pad);
        r.consumed |= mask;
        r.pressed &= !mask;
    }
    /// 42870. Original clears only the low 16 bits of the pressed word.
    pub fn reset_neutral(&mut self, pad: usize) {
        let r = self.record_mut(pad);
        r.pressed &= 0xffff_0000;
        r.consumed |= 0xffff;
        r.axes.fill(128);
    }
    /// 42970. Scales are supplied from guest constants 4783a8 and 476038;
    /// no guessed ROM values. Invalid indices are outside this typed boundary.
    pub fn axis(&self, pad: usize, axis: usize, negative_scale: f32, positive_scale: f32) -> f32 {
        if self.locked {
            return 0.0;
        }
        let r = self.record(pad);
        let raw = r.axes[axis] as i32;
        let value = raw.wrapping_sub(128) as f32
            * if raw < 128 {
                negative_scale
            } else {
                positive_scale
            };
        let d = r.deadzone;
        // COMISS treats unordered comparisons as below/equal.
        if d.partial_cmp(&0.0) != Some(core::cmp::Ordering::Greater) {
            return value;
        }
        let adjusted = if -d > value {
            value + d
        } else if value > d {
            value - d
        } else {
            return 0.0;
        };
        adjusted / (1.0 - d)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mapping_gating_and_consumption() {
        let mut s = PadState::default();
        s.mapping = [3, 2, 1, 0];
        s.physical[1].connected = 5;
        s.physical[3].connected = 1;
        assert_eq!(s.rebuild_mapping(), 2);
        assert_eq!(s.mapping, [1, 3, 1, 0]);
        s.physical[1].held = 7;
        s.physical[1].pressed = 0xabcd_0007;
        s.consume(0, 2);
        assert_eq!(s.available_buttons(0), 5);
        s.locked = true;
        assert_eq!(s.available_buttons(99), 0);
        s.reset_neutral(0);
        assert_eq!(s.physical[1].pressed, 0xabcd_0000);
        assert_eq!(s.rebuild_mapping(), 0);
        assert_eq!(s.connected_count, 2);
        s.alternate = true;
        s.remote[0].released = 9;
        s.locked = false;
        assert_eq!(s.released_buttons(0), 9);
    }
    #[test]
    fn axis_boundaries() {
        let mut s = PadState::default();
        s.physical[0].deadzone = 0.25;
        for raw in 0..=255 {
            s.physical[0].axes[0] = raw;
            let v = s.axis(0, 0, 1.0 / 128.0, 1.0 / 127.0);
            assert!((-1.0..=1.0).contains(&v));
            if (96..=159).contains(&raw) {
                assert_eq!(v, 0.0);
            }
        }
        s.physical[0].deadzone = f32::NAN;
        s.physical[0].axes[0] = 255;
        assert_eq!(s.axis(0, 0, 1.0 / 128.0, 1.0 / 127.0), 1.0);
    }
    #[test]
    fn mapping_preserves_unused_slots_for_every_connection_mask() {
        for mask in 0u8..16 {
            let mut state = PadState { mapping: [9, 8, 7, 6], ..PadState::default() };
            for port in 0..4 {
                state.physical[port].connected = u32::from(mask & (1 << port) != 0);
            }
            let expected: Vec<_> = (0..4).filter(|port| mask & (1 << port) != 0).collect();
            assert_eq!(state.rebuild_mapping(), expected.len());
            assert_eq!(state.connected_count, expected.len());
            assert_eq!(&state.mapping[..expected.len()], expected.as_slice());
            assert_eq!(&state.mapping[expected.len()..], &[9, 8, 7, 6][expected.len()..]);
            state.locked = true;
            let mapping = state.mapping;
            assert_eq!(state.rebuild_mapping(), 0);
            assert_eq!(state.mapping, mapping);
            assert_eq!(state.connected_count, expected.len());
        }
    }
    #[test]
    fn consumption_is_monotone_and_reset_preserves_upper_pressed_bits() {
        for bit in 0..32 {
            let mut state = PadState::default();
            state.physical[0].held = u32::MAX;
            state.physical[0].pressed = u32::MAX;
            let mask = 1u32 << bit;
            state.locked = true;
            state.consume(0, mask);
            assert_eq!(state.physical[0].consumed, mask);
            assert_eq!(state.physical[0].pressed, !mask);
            state.consume(0, mask);
            state.locked = false;
            assert_eq!(state.available_buttons(0), !mask);
            state.reset_neutral(0);
            assert_eq!(state.physical[0].pressed, (!mask) & 0xffff_0000);
            assert_eq!(state.physical[0].consumed, mask | 0xffff);
            assert_eq!(state.physical[0].axes, [128; 7]);
        }
    }
}
