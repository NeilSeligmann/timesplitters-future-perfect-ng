// SPDX-License-Identifier: GPL-3.0-or-later
//! Raw request initializer reconstructed from original 0x45390.

/// The twelve dwords initialized by the original request helper.
///
/// Field meanings are not established. Callers may populate additional raw
/// slots after construction, as the 0x18AF40 selector does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundRequestRaw {
    words: [u32; 12],
}

impl SoundRequestRaw {
    /// Build the exact observed 0x45390 post-state without assigning semantic
    /// names to its raw fields.
    pub const fn initialized() -> Self {
        let mut words = [0; 12];
        words[2] = 1.0f32.to_bits(); // +0x08
        words[4] = 0x14; // +0x10
        words[5] = 1.0f32.to_bits(); // +0x14
        words[6] = 1.0f32.to_bits(); // +0x18
        words[11] = 50.0f32.to_bits(); // +0x2c
        Self { words }
    }

    /// Apply the observed writes for request ID `0x4C4`.
    /// The three floats and low byte are kept as raw request contents.
    pub fn with_id_4c4(mut self, values: [f32; 3]) -> Self {
        self.words[0] = 0x4c4; // +0x00
        self.words[7] = values[0].to_bits(); // +0x1c
        self.words[8] = values[1].to_bits(); // +0x20
        self.words[9] = values[2].to_bits(); // +0x24
        self.words[10] = (self.words[10] & !0xff) | 1; // byte +0x28
        self
    }

    /// Return the initialized record as its observed twelve-dword layout.
    pub const fn words(&self) -> &[u32; 12] {
        &self.words
    }

    /// Borrow the Rust-owned request for the measured 0x45820 mutating route.
    pub(crate) fn words_mut(&mut self) -> &mut [u32; 12] {
        &mut self.words
    }
}

#[cfg(test)]
mod tests {
    use super::SoundRequestRaw;

    #[test]
    fn initializes_only_measured_raw_slots() {
        let request = SoundRequestRaw::initialized();
        let words = request.words();
        assert_eq!(words.len(), 12);
        assert_eq!(words[2], 1.0f32.to_bits());
        assert_eq!(words[4], 0x14);
        assert_eq!(words[5], 1.0f32.to_bits());
        assert_eq!(words[6], 1.0f32.to_bits());
        assert_eq!(words[11], 50.0f32.to_bits());
        for index in [0, 1, 3, 7, 8, 9, 10] {
            assert_eq!(words[index], 0);
        }
    }

    #[test]
    fn id_4c4_applies_measured_raw_request_writes() {
        let request = SoundRequestRaw::initialized().with_id_4c4([2.0, 3.0, 4.0]);
        let words = request.words();
        assert_eq!(words[0], 0x4c4);
        assert_eq!(words[7], 2.0f32.to_bits());
        assert_eq!(words[8], 3.0f32.to_bits());
        assert_eq!(words[9], 4.0f32.to_bits());
        assert_eq!(words[10], 1);
        assert_eq!(words[11], 50.0f32.to_bits());
    }
}
