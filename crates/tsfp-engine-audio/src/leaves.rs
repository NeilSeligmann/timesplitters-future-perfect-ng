// SPDX-License-Identifier: GPL-3.0-or-later
//! T1848 wave 4 audio leaves. Each body ports a call-free C replacement of `src/game`; the
//! register result is exact (`entry` supplies the registers a root leaves untouched). Reads and
//! writes keep the C order. Contracts: docs/t1848-rust-wave4.md.
use crate::{gprs::Gprs, memory::GuestMemory};

const MASK_WORD: u32 = 0x004c_b54c;
const FALLBACK_TABLE: u32 = 0x0053_a3cc;
const FALLBACK_OFF: u32 = 0x0053_a2c8;
const FALLBACK_ON: u32 = 0x0053_9b08;
const KEYS_A: u32 = 0x0058_35f0;
const SLOTS_A: u32 = 0x0056_5a00;
const KEYS_B: u32 = 0x0058_18e8;
const SLOTS_B: u32 = 0x0056_5aa0;
const SLOT_COUNT: u32 = 40;

/// 00045D60 game_audio_global_mask_bit_test. cdecl(1): EAX = bit (masked to 5 bits) of [4CB54C].
// portreg: va=0x00045d60 tier=1
pub fn global_mask_bit_test<M: GuestMemory>(m: &M, bit: u32, entry: Gprs) -> Gprs {
    let eax = (1u32.wrapping_shl(bit & 31) & m.read32(MASK_WORD) != 0) as u32;
    Gprs {
        eax,
        ecx: bit,
        edx: entry.edx,
    }
}

/// 00063BF0 game_audio_pair_cursor_equal. cdecl(1): EAX = [p] == [p+4].
// portreg: va=0x00063bf0 tier=1
pub fn pair_cursor_equal<M: GuestMemory>(m: &M, pair: u32, entry: Gprs) -> Gprs {
    let eax = m.read32(pair).wrapping_sub(m.read32(pair.wrapping_add(4)));
    Gprs {
        eax: (eax == 0) as u32,
        ecx: pair,
        edx: entry.edx,
    }
}

/// 000D76A0 game_audio_table_special_fallback. cdecl(2): 48-byte table entry word, with a
/// fixed fallback address for index 0x24 when the word is zero.
pub fn table_special_fallback<M: GuestMemory>(m: &M, index: u32, flag: u32, entry: Gprs) -> Gprs {
    let mut eax = m.read32(
        index
            .wrapping_mul(3)
            .wrapping_shl(4)
            .wrapping_add(FALLBACK_TABLE),
    );
    if eax == 0 && index == 0x24 {
        eax = if flag != 0 { FALLBACK_OFF } else { FALLBACK_ON };
    }
    Gprs {
        eax,
        ecx: index,
        edx: entry.edx,
    }
}

/// 00026B70 game_audio_clear_matching_key. cdecl(0), key in ECX. Clears the first slot whose key
/// matches in table A, else in table B. EAX = index reached (40 when absent), EDX = 0.
///
/// The first matching slot in A wins even if B also contains the key.
///
/// ```
/// use tsfp_engine_audio::{gprs::Gprs, leaves::clear_matching_key, memory::GuestMemory};
/// use std::collections::BTreeMap;
/// struct Words(BTreeMap<u32, u8>);
/// impl GuestMemory for Words {
///     fn read8(&self, a: u32) -> u8 { *self.0.get(&a).unwrap_or(&0) }
///     fn write8(&mut self, a: u32, v: u8) { self.0.insert(a, v); }
/// }
/// let mut memory = Words(BTreeMap::new());
/// memory.write32(0x0058_35f0, 7);
/// memory.write32(0x0056_5a00, 99);
/// let result = clear_matching_key(&mut memory, Gprs { ecx: 7, ..Gprs::default() });
/// assert_eq!(result.eax, 0);
/// assert_eq!(memory.read32(0x0056_5a00), 0);
/// ```
// portreg: va=0x00026b70 tier=1
pub fn clear_matching_key<M: GuestMemory>(m: &mut M, entry: Gprs) -> Gprs {
    let key = entry.ecx;
    let mut index = 0u32;
    while m.read32(index.wrapping_mul(4).wrapping_add(KEYS_A)) != key {
        index += 1;
        if index >= SLOT_COUNT {
            index = 0;
            while m.read32(index.wrapping_mul(4).wrapping_add(KEYS_B)) != key {
                index += 1;
                if index >= SLOT_COUNT {
                    return Gprs {
                        eax: index,
                        ecx: key,
                        edx: 0,
                    };
                }
            }
            m.write32(index.wrapping_mul(4).wrapping_add(SLOTS_B), 0);
            return Gprs {
                eax: index,
                ecx: key,
                edx: 0,
            };
        }
    }
    m.write32(index.wrapping_mul(4).wrapping_add(SLOTS_A), 0);
    Gprs {
        eax: index,
        ecx: key,
        edx: 0,
    }
}
