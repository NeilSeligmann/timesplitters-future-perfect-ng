//! Measured object-pool handle stamp arithmetic.
//!
//! The guest stores a ten-bit slot index and a 22-bit serial in one word. The
//! serial wraps from its maximum to one, skipping zero. This models stamp
//! arithmetic only; slot allocation, object construction, and teardown remain
//! separate caller-owned operations.

pub const SLOT_BITS: u32 = 10;
pub const SERIAL_BITS: u32 = 22;
pub const SLOT_COUNT: u32 = 1 << SLOT_BITS;
pub const SERIAL_MASK: u32 = (1 << SERIAL_BITS) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandleStamp {
    pub slot: u16,
    pub serial: u32,
}

impl HandleStamp {
    /// Encodes the measured `(slot << 22) | serial` guest word.
    ///
    /// Returns `None` for an out-of-range slot or serial. The guest producer
    /// supplies nonzero serials; zero remains representable for decoding only.
    pub const fn encode(self) -> Option<u32> {
        if self.slot as u32 >= SLOT_COUNT || self.serial > SERIAL_MASK {
            return None;
        }
        Some(((self.slot as u32) << SERIAL_BITS) | self.serial)
    }

    /// Splits a guest word into its ten-bit slot and 22-bit serial fields.
    pub const fn decode(word: u32) -> Self {
        Self {
            slot: (word >> SERIAL_BITS) as u16,
            serial: word & SERIAL_MASK,
        }
    }

    /// Advances a guest serial, skipping zero when the 22-bit field wraps.
    pub const fn next_serial(serial: u32) -> u32 {
        let next = serial.wrapping_add(1) & SERIAL_MASK;
        if next == 0 { 1 } else { next }
    }

    /// Matches the resolver's full-word comparison for this slot.
    pub const fn resolves_as(word: u32, slot: u16, stored_word: u32) -> bool {
        (word >> SERIAL_BITS) as u16 == slot && word == stored_word
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_fields_round_trip_and_reject_invalid_encoder_inputs() {
        let stamp = HandleStamp {
            slot: 0x2ab,
            serial: 0x003f_fffe,
        };
        let word = stamp.encode().unwrap();
        assert_eq!(HandleStamp::decode(word), stamp);
        assert_eq!(
            HandleStamp {
                slot: SLOT_COUNT as u16,
                serial: 1
            }
            .encode(),
            None
        );
        assert_eq!(
            HandleStamp {
                slot: 0,
                serial: SERIAL_MASK + 1
            }
            .encode(),
            None
        );
    }

    #[test]
    fn guest_serial_wraps_to_one_and_resolver_compares_the_full_word() {
        assert_eq!(HandleStamp::next_serial(SERIAL_MASK - 1), SERIAL_MASK);
        assert_eq!(HandleStamp::next_serial(SERIAL_MASK), 1);
        let old = HandleStamp {
            slot: 7,
            serial: 12,
        }
        .encode()
        .unwrap();
        let replacement = HandleStamp {
            slot: 7,
            serial: 13,
        }
        .encode()
        .unwrap();
        assert!(HandleStamp::resolves_as(old, 7, old));
        assert!(!HandleStamp::resolves_as(old, 7, replacement));
        assert!(!HandleStamp::resolves_as(old, 8, old));
    }
}
