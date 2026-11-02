// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 12 ui leaves: menu state predicates, a four-dword settings store and a scan of
//! 20-byte menu entries, ported as readable Rust from call-free C replacements of `src/game`
//! (docs/t1836-rust-wave12.md). Registers and stack reads follow `gameplay::slot_leaves`.
use crate::{gprs::Gprs, memory::GuestMemory};

fn arg<M: GuestMemory>(m: &M, esp: u32, index: u32) -> u32 {
    m.read32(esp.wrapping_add(4 + 4 * index))
}

/// 00311A80 game_is_two. cdecl(1). 1 when the argument is 2.
pub fn is_two<M: GuestMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let value = arg(m, esp, 0);
    Gprs {
        eax: u32::from(value == 2),
        ecx: value,
        edx: e.edx,
    }
}

/// 00311A90 game_is_three. cdecl(1). 1 when the argument is 3.
pub fn is_three<M: GuestMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let value = arg(m, esp, 0);
    Gprs {
        eax: u32::from(value == 3),
        ecx: value,
        edx: e.edx,
    }
}

const SETTINGS_BASE: u32 = 0x004e_7958;

/// 001553E0 game_globals_4e7958_set_four_dwords. cdecl(4). Stores the four arguments to the
/// consecutive dwords at 0x4E7958.
// portreg: va=0x001553e0 tier=1
pub fn set_settings_quad<M: GuestMemory>(m: &mut M, esp: u32, _e: Gprs) -> Gprs {
    let values = [0, 1, 2, 3].map(|index| arg(m, esp, index));
    for (slot, value) in values.into_iter().enumerate() {
        m.write32(SETTINGS_BASE + 4 * slot as u32, value);
    }
    Gprs {
        eax: values[3],
        ecx: values[1],
        edx: values[2],
    }
}

const ENTRY_SIZE: u32 = 0x14;
const ENTRY_FLAGS_BYTE: u32 = 1;
const ENTRY_FLAG_BIT1: u8 = 2;
const LIST_ENTRIES: u32 = 0x1c;
const LIST_COUNT: u32 = 0x20;
const SLOT_SIZE: u32 = 0x10;
const SLOT_ASSIGNMENT: u32 = 0x128;
const SLOT_STATE_INDEX_BIAS: u32 = 0x13;
const RECORD_SIZE: u32 = 0x68;
const RECORD_BASE: u32 = 0x0077_8800;
const RECORD_STATE: u32 = 0x40;
const RECORD_FLAG_BIT1: u32 = 2;

/// 0022D790 game_entry_list_0x14_any_entry_byte1_bit1_set. cdecl(1: list). The list holds a
/// signed count at +0x20 and the entries (20 bytes) at the pointer +0x1C. Returns 1 when bit 1
/// of byte 1 of any entry is set. ECX ends on the entry that matched, or past the last one.
pub fn any_entry_flag_set<M: GuestMemory>(m: &M, esp: u32, _e: Gprs) -> Gprs {
    let list = arg(m, esp, 0);
    let count = m.read32(list.wrapping_add(LIST_COUNT));
    if (count as i32) <= 0 {
        return Gprs {
            eax: 0,
            ecx: list,
            edx: count,
        };
    }
    let mut entry = m.read32(list.wrapping_add(LIST_ENTRIES));
    for _ in 0..count {
        if m.read8(entry.wrapping_add(ENTRY_FLAGS_BYTE)) & ENTRY_FLAG_BIT1 != 0 {
            return Gprs {
                eax: 1,
                ecx: entry,
                edx: count,
            };
        }
        entry = entry.wrapping_add(ENTRY_SIZE);
    }
    Gprs {
        eax: 0,
        ecx: entry,
        edx: count,
    }
}

/// 00073E10 game_slot_ready. cdecl(2: object, slot). 0 when the dword at +0x128 of the 16-byte
/// slot is -1 or the state dword of slot `slot + 0x13` is 2, else 1.
pub fn slot_ready<M: GuestMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let (object, slot) = (arg(m, esp, 0), arg(m, esp, 1));
    let unassigned = m.read32(
        slot.wrapping_mul(SLOT_SIZE)
            .wrapping_add(object)
            .wrapping_add(SLOT_ASSIGNMENT),
    ) == u32::MAX;
    let eax = !unassigned
        && m.read32(
            slot.wrapping_add(SLOT_STATE_INDEX_BIAS)
                .wrapping_mul(SLOT_SIZE)
                .wrapping_add(object),
        ) != 2;
    Gprs {
        eax: u32::from(eax),
        ..e
    }
}

/// 00349710 game_record_68_clear_bit1_set_state. cdecl(1: index). In the 0x68-byte record at
/// 0x778800 + index * 0x68 clears bit 1 of the flags and sets the state dword (+0x40) to 2.
/// Returns the record address.
pub fn record_68_clear_bit1_set_state<M: GuestMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let record = arg(m, esp, 0)
        .wrapping_mul(RECORD_SIZE)
        .wrapping_add(RECORD_BASE);
    let flags = m.read32(record) & !RECORD_FLAG_BIT1;
    m.write32(record.wrapping_add(RECORD_STATE), 2);
    m.write32(record, flags);
    Gprs { eax: record, ..e }
}

/// 00147200 game_ui_campaign_append_notice_record1c. cdecl(4: list, kind, extra, text). Appends
/// a 28-byte notice record `{kind, text, 0, extra, .., -1 at +0x18}` at index `[list+0xE0]` and
/// increments the count. Reads and stores keep the C order (the record may alias the count).
pub fn append_notice_record<M: GuestMemory>(m: &mut M, esp: u32, _e: Gprs) -> Gprs {
    let (list, kind, extra, text) = (
        arg(m, esp, 0),
        arg(m, esp, 1),
        arg(m, esp, 2),
        arg(m, esp, 3),
    );
    let record = m
        .read32(list.wrapping_add(0xe0))
        .wrapping_mul(0x1c)
        .wrapping_add(list);
    m.write32(record, kind);
    m.write32(record.wrapping_add(4), text);
    m.write32(record.wrapping_add(8), 0);
    let record = m
        .read32(list.wrapping_add(0xe0))
        .wrapping_mul(0x1c)
        .wrapping_add(list);
    m.write32(record.wrapping_add(0xc), extra);
    m.write32(record.wrapping_add(0x18), u32::MAX);
    let count = m.read32(list.wrapping_add(0xe0));
    m.write32(list.wrapping_add(0xe0), count.wrapping_add(1));
    Gprs {
        eax: record,
        ecx: list,
        edx: extra,
    }
}

/// 00070C90 game_int_array_find_index_or_minus1. cdecl(2: array, needle). `array` points at a
/// count followed by that many dwords. Returns the index of the first dword equal to `needle`
/// or -1. ECX ends on the matching element (or one past the last), EDX holds the count. The
/// original saves ESI in a scratch slot below ESP, which the port does not write (unobserved).
pub fn int_array_find_index<M: GuestMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let list = arg(m, esp, 0);
    let (mut ecx, mut edx) = (list, e.edx);
    if list != 0 {
        edx = m.read32(list);
        if (edx as i32) > 0 {
            let needle = arg(m, esp, 1);
            ecx = list.wrapping_add(4);
            for index in 0..edx {
                if m.read32(ecx) == needle {
                    return Gprs {
                        eax: index,
                        ecx,
                        edx,
                    };
                }
                ecx = ecx.wrapping_add(4);
            }
        }
    }
    Gprs {
        eax: u32::MAX,
        ecx,
        edx,
    }
}

/// 0007C4D0 game_str_is_empty_or_spaces_ecx. thiscall(text in ECX). 1 when the string is empty
/// or only spaces.
pub fn text_is_empty_or_spaces<M: GuestMemory>(m: &M, text: u32) -> u32 {
    let mut cursor = text;
    while m.read8(cursor) == b' ' {
        cursor = cursor.wrapping_add(1);
    }
    u32::from(m.read8(cursor) == 0)
}

/// Font charset tables: `[bold-or-not][row] = (record address, companion address)`.
const CHARSET_ROWS: [[(u32, u32); 5]; 2] = [
    [
        (0x004d_8dd0, 0x004d_8e74),
        (0x004d_8ea0, 0x004d_8f44),
        (0x004d_8f70, 0x004d_9014),
        (0x004d_9040, 0x004d_90e4),
        (0x004d_9110, 0x004d_91b4),
    ],
    [
        (0x004d_91e0, 0x004d_9254),
        (0x004d_9278, 0x004d_92ec),
        (0x004d_9310, 0x004d_9384),
        (0x004d_93a8, 0x004d_941c),
        (0x004d_9440, 0x004d_94b4),
    ],
];

/// 0007C1B0 game_font_charset_select. fastcall(out in ECX, flags in EDX). Picks a table by bit 7
/// of the flags, then a row: 1 when exactly one of bits 1 and 2 is set, 2 for bit 3, else 3 for
/// bit 12 (4 when also exactly one of bits 1 and 2). Stores the companion address at `out` (if
/// non-null) and returns the record address.
pub fn font_charset_select<M: GuestMemory>(m: &mut M, out: u32, flags: u32) -> u32 {
    let table = &CHARSET_ROWS[usize::from(flags & 0x80 != 0)];
    let one_of_bits_1_2 = matches!(flags & 6, 2 | 4);
    let row = if flags & 8 != 0 {
        2
    } else if flags & 0x1000 != 0 {
        if one_of_bits_1_2 {
            4
        } else {
            3
        }
    } else {
        usize::from(one_of_bits_1_2)
    };
    if out != 0 {
        m.write32(out, table[row].1);
    }
    table[row].0
}
