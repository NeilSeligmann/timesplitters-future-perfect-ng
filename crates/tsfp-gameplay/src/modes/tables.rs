// SPDX-License-Identifier: GPL-3.0-or-later
//! Mode record table accessors (all Tier 0).
use super::ModeHost;

const RECORD_58_BASE: u32 = 0x0052_cf40;
const RECORD_58_SIZE: u32 = 0x58;
// The byte meanings are still unknown; keep positional names until data confirms them.
const RECORD_BYTE_A_FIELD: u32 = 2;
const RECORD_BYTE_B_FIELD: u32 = 3;
const RECORD_BYTE_C_FIELD: u32 = 4;
const RECORD_PAYLOAD_FIELD: u32 = 8;
const RECORD_PAYLOAD_WORDS: u32 = 15;
const MODE_ENTRY_TABLE: u32 = 0x004d_dae8;
const MODE_ENTRY_SIZE: u32 = 0x54;
const SIX_BYTE_TABLE: u32 = 0x0078_2e60;
const SIX_BYTE_ENTRY_SIZE: u32 = 6;

fn record_58<H: ModeHost>(h: &H, index: u32, offset: u32) -> u32 {
    u32::from(
        h.read8(
            index
                .wrapping_mul(RECORD_58_SIZE)
                .wrapping_add(RECORD_58_BASE + offset),
        ),
    )
}

/// 000BE850 game_table_4ddae8_get_entry. cdecl(index). Address of the 0x54-byte entry.
pub fn table_4ddae8_entry(index: u32) -> u32 {
    index
        .wrapping_mul(MODE_ENTRY_SIZE)
        .wrapping_add(MODE_ENTRY_TABLE)
}

/// 00311860 game_record_58_byte_a_by_index. Byte at +2 of the 0x58-byte record `index`.
pub fn record_58_byte_a<H: ModeHost>(h: &H, index: u32) -> u32 {
    record_58(h, index, RECORD_BYTE_A_FIELD)
}
/// 00311870 game_record_58_byte_b_by_index. Byte at +3.
pub fn record_58_byte_b<H: ModeHost>(h: &H, index: u32) -> u32 {
    record_58(h, index, RECORD_BYTE_B_FIELD)
}
/// 00311880 game_record_58_byte_c_by_index. Byte at +4.
pub fn record_58_byte_c<H: ModeHost>(h: &H, index: u32) -> u32 {
    record_58(h, index, RECORD_BYTE_C_FIELD)
}

/// 003118B0 game_record_58_copy_60_bytes_from_field_10. cdecl(destination, index). Copies the
/// 60 bytes at +8 of record `index` to `destination`.
pub fn record_58_copy_payload<H: ModeHost>(h: &mut H, destination: u32, index: u32) {
    let source = index
        .wrapping_mul(RECORD_58_SIZE)
        .wrapping_add(RECORD_58_BASE + RECORD_PAYLOAD_FIELD);
    for word in 0..RECORD_PAYLOAD_WORDS {
        let value = h.read32(source.wrapping_add(4 * word));
        h.write32(destination.wrapping_add(4 * word), value);
    }
}

/// 00320FC0 game_copy_six_byte_entries_from_arg_count_size_div_8_into_static_table.
/// cdecl(source, size). Copies `size / 8` entries of six bytes from `source` to the static table
/// at 0x782E60 (the copy stride stays six bytes on both sides).
pub fn copy_six_byte_entries<H: ModeHost>(h: &mut H, source: u32, size: u32) {
    let count = (size >> 3) as i32;
    let mut destination = SIX_BYTE_TABLE;
    for index in 0..count.max(0) as u32 {
        let from = source.wrapping_add(SIX_BYTE_ENTRY_SIZE * index);
        let dword = h.read32(from);
        let word = h.read16(from.wrapping_add(4));
        h.write32(destination, dword);
        for (i, byte) in word.to_le_bytes().into_iter().enumerate() {
            h.write8(destination.wrapping_add(4 + i as u32), byte);
        }
        destination = destination.wrapping_add(SIX_BYTE_ENTRY_SIZE);
    }
}

/// 002F76D0 / 002F76E0 game_results_build_stats_and_roster_widget_arg_zero / _one. Thin wrappers
/// of 2F7610 with the page selector 0 or 1 in EDI and the caller's widget in EBX (register
/// convention INFERRED, passed here as the two leading arguments).
pub fn results_build_widget<H: ModeHost>(h: &mut H, widget: u32, page: u32) -> u32 {
    h.call(0x002f_7610, &[widget, page])
}

/// 0028C560 game_mode_other_branch_leaf. cdecl(object, kind). Kinds 1..=6 select, through a jump
/// table kept in the guest image (0x28C61C), one of: clear bit 0 of the object flags at +0x28
/// (returns 0), release the four mode resources of `[0x78B1EC]` (fields +0x4BDC, +0x4BD8, +0x4BCC,
/// +0x4270, each freed with A9070 when set, then cleared; returns -1), return -1, or return 0.
pub fn mode_other_branch<H: ModeHost>(h: &mut H, object: u32, kind: u32) -> u32 {
    const DISPATCH_TABLE: u32 = 0x0028_c61c;
    const CLEAR_OBJECT_FLAG_BRANCH: u32 = 0x0028_c57a;
    const RELEASE_RESOURCES_BRANCH: u32 = 0x0028_c584;
    const RETURN_MINUS_ONE_BRANCH: u32 = 0x0028_c615;
    const OBJECT_FLAGS_FIELD: u32 = 0x28;
    const OBJECT_ACTIVE_BIT: u32 = 1;
    const MODE_RESOURCES_POINTER: u32 = 0x0078_b1ec;
    const RESOURCE_FIELDS: [u32; 4] = [0x4bdc, 0x4bd8, 0x4bcc, 0x4270];
    const RELEASE_RESOURCE: u32 = 0x000a_9070;

    let index = kind.wrapping_sub(1);
    if index > 5 {
        return 0;
    }
    match h.read32(DISPATCH_TABLE + index * 4) {
        CLEAR_OBJECT_FLAG_BRANCH => {
            let flags_address = object.wrapping_add(OBJECT_FLAGS_FIELD);
            let flags = h.read32(flags_address);
            h.write32(flags_address, flags & !OBJECT_ACTIVE_BIT);
            0
        }
        RELEASE_RESOURCES_BRANCH => {
            for field in RESOURCE_FIELDS {
                let base = h.read32(MODE_RESOURCES_POINTER);
                let resource = h.read32(base.wrapping_add(field));
                if resource != 0 {
                    h.call(RELEASE_RESOURCE, &[resource]);
                }
            }
            // Re-read the pointer in each pass because a guest release call may change it.
            for field in RESOURCE_FIELDS {
                let base = h.read32(MODE_RESOURCES_POINTER);
                h.write32(base.wrapping_add(field), 0);
            }
            u32::MAX
        }
        RETURN_MINUS_ONE_BRANCH => u32::MAX,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_host::TestHost;
    use super::*;
    use crate::memory::GuestMemory;

    #[test]
    fn record_58_bytes_and_entry_address() {
        let mut h = TestHost::default();
        h.write8(0x0052_cf42 + 0x58 * 2, 7);
        h.write8(0x0052_cf43 + 0x58 * 2, 8);
        h.write8(0x0052_cf44 + 0x58 * 2, 9);
        assert_eq!(
            (
                record_58_byte_a(&h, 2),
                record_58_byte_b(&h, 2),
                record_58_byte_c(&h, 2)
            ),
            (7, 8, 9)
        );
        assert_eq!(table_4ddae8_entry(2), 0x004d_dae8 + 0xa8);
    }

    #[test]
    fn copy_payload_and_six_byte_entries() {
        let mut h = TestHost::default();
        for word in 0..15 {
            h.write32(0x0052_cf48 + 0x58 + 4 * word, word + 1);
        }
        record_58_copy_payload(&mut h, 0x9000, 1);
        assert_eq!((h.read32(0x9000), h.read32(0x9000 + 56)), (1, 15));
        for i in 0..12u32 {
            h.write8(0xa000 + i, i as u8 + 1);
        }
        copy_six_byte_entries(&mut h, 0xa000, 16);
        assert_eq!(h.read32(0x0078_2e60), 0x0403_0201);
        assert_eq!(h.read16(0x0078_2e64), 0x0605);
        assert_eq!(h.read32(0x0078_2e66), 0x0a09_0807);
        assert_eq!(h.read8(0x0078_2e6a), 0x0b);
    }

    #[test]
    fn widget_wrapper_passes_page() {
        let mut h = TestHost::default();
        results_build_widget(&mut h, 5, 1);
        assert_eq!(h.calls, vec![(0x002f_7610, vec![5, 1])]);
    }

    #[test]
    fn other_mode_branch_releases_and_clears_resources_in_order() {
        let mut h = TestHost::default();
        let table = 0x0028_c61c;
        h.write32(table, 0x0028_c584);
        h.write32(0x0078_b1ec, 0x9000);
        let fields = [0x4bdc, 0x4bd8, 0x4bcc, 0x4270];
        for (index, field) in fields.into_iter().enumerate() {
            h.write32(0x9000 + field, 0xa000 + index as u32);
        }
        assert_eq!(mode_other_branch(&mut h, 0xb000, 1), u32::MAX);
        assert_eq!(
            h.calls,
            (0..4)
                .map(|index| (0x000a_9070, vec![0xa000 + index]))
                .collect::<Vec<_>>()
        );
        for field in fields {
            assert_eq!(h.read32(0x9000 + field), 0);
        }
    }

    #[test]
    fn other_mode_branch_clears_flag_and_handles_default_cases() {
        let mut h = TestHost::default();
        h.write32(0x0028_c61c, 0x0028_c57a);
        h.write32(0x8000 + 0x28, 0xabcdu32);
        assert_eq!(mode_other_branch(&mut h, 0x8000, 1), 0);
        assert_eq!(h.read32(0x8000 + 0x28), 0xabcc);
        h.write32(0x0028_c61c + 4, 0x0028_c615);
        assert_eq!(mode_other_branch(&mut h, 0x8000, 2), u32::MAX);
        assert_eq!(mode_other_branch(&mut h, 0x8000, 0), 0);
        assert_eq!(mode_other_branch(&mut h, 0x8000, 7), 0);
        assert!(h.calls.is_empty());
    }
}
