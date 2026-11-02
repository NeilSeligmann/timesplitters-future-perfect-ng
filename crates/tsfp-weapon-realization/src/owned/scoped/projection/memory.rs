// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded compatibility view of a player extension. TEMPORARY-SHIM, T2411.
//! The caller supplies exclusively writable mapped storage, never a snapshot to
//! copy back. Drop this view before any callback can access the same storage.
use super::Sink;
use crate::Side;
use tsfp_weapon_owner::weapon_hands::WeaponRecordId;
use tsfp_weapon_owner::weapon_inventory::AMMO_TYPE_COUNT;

const FLAG: usize = 0xbac;
const RESERVE: usize = 0x61e;

/// No guest reads or address arithmetic occur during publication. Slice bounds
/// are checked before beginning a transition; unrelated extension bytes remain
/// untouched. Production mapped-memory acquisition remains caller-owned.
pub struct ExtensionSink<'a> {
    bytes: &'a mut [u8],
}

impl<'a> ExtensionSink<'a> {
    pub fn new(bytes: &'a mut [u8]) -> Option<Self> {
        (bytes.len() >= FLAG + 4).then_some(Self { bytes })
    }

    fn hand(side: Side) -> usize {
        match side {
            Side::Left => 0xa4,
            Side::Right => 0x1d4,
        }
    }

    fn word(&mut self, offset: usize, value: u32) {
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn ammo_words(&mut self, offset: usize, values: &[i16]) {
        for (index, value) in values.iter().enumerate() {
            let offset = offset + 2 * index;
            self.bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
    }
}

impl Sink for ExtensionSink<'_> {
    fn child(&mut self, side: Side, child: Option<u32>) {
        self.word(Self::hand(side) + 0xe8, child.unwrap_or(u32::MAX));
    }

    fn record(&mut self, side: Side, record: Option<WeaponRecordId>) {
        self.word(
            Self::hand(side) + 0x30,
            record.map_or(u32::MAX, |id| u32::from(id.0)),
        );
    }

    fn clear_realization_flag(&mut self) {
        self.word(FLAG, 0);
    }

    fn ammo(&mut self, side: Side, loaded: &[i16], saved: &[i16], reserve: &[i16]) {
        // Validate every width before the first write. This is an internal API
        // invariant: WriteProjection always supplies the owner's fixed arrays.
        assert_eq!(loaded.len(), AMMO_TYPE_COUNT);
        assert_eq!(saved.len(), AMMO_TYPE_COUNT);
        assert_eq!(reserve.len(), AMMO_TYPE_COUNT);
        self.ammo_words(Self::hand(side) + 0x38, loaded);
        self.ammo_words(Self::hand(side) + 0x90, saved);
        self.ammo_words(RESERVE, reserve);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_storage_without_writes() {
        let mut bytes = [0xa5; FLAG + 3];
        assert!(ExtensionSink::new(&mut bytes).is_none());
        assert!(bytes.iter().all(|&byte| byte == 0xa5));
    }

    #[test]
    fn publishes_exact_fields_for_both_hands_with_signed_ammo() {
        for (side, hand) in [(Side::Left, 0xa4), (Side::Right, 0x1d4)] {
            let mut bytes = [0xa5; FLAG + 8];
            let mut expected = bytes;
            let loaded = [i16::MIN; AMMO_TYPE_COUNT];
            let saved = [-1; AMMO_TYPE_COUNT];
            let reserve = [i16::MAX; AMMO_TYPE_COUNT];
            {
                let mut sink = ExtensionSink::new(&mut bytes).unwrap();
                sink.child(side, Some(0));
                sink.record(side, Some(WeaponRecordId(0x29)));
                sink.clear_realization_flag();
                sink.ammo(side, &loaded, &saved, &reserve);
            }
            expected[hand + 0xe8..hand + 0xec].copy_from_slice(&0u32.to_le_bytes());
            expected[hand + 0x30..hand + 0x34].copy_from_slice(&0x29u32.to_le_bytes());
            expected[FLAG..FLAG + 4].fill(0);
            for (offset, value) in [
                (hand + 0x38, i16::MIN),
                (hand + 0x90, -1),
                (RESERVE, i16::MAX),
            ] {
                for pair in expected[offset..offset + 88].chunks_exact_mut(2) {
                    pair.copy_from_slice(&value.to_le_bytes());
                }
            }
            assert_eq!(bytes, expected);
            let mut sink = ExtensionSink::new(&mut bytes).unwrap();
            sink.child(side, None);
            sink.record(side, None);
            expected[hand + 0xe8..hand + 0xec].fill(0xff);
            expected[hand + 0x30..hand + 0x34].fill(0xff);
            assert_eq!(bytes, expected);
        }
    }
}
