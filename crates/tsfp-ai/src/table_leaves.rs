// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 12 ai leaves: character behaviour table lookups ported from call-free C
//! replacements of `src/game` (docs/t1836-rust-wave12.md).
use crate::{gprs::Gprs, memory::GuestMemory};

const BEHAVIOUR_TABLE: u32 = 0x0052_0a48;

/// 002AEFB0 game_table_520a48_entry. cdecl(2: group, slot). Dword `group + 4 * slot`, scaled by
/// four bytes, of the behaviour table at 0x520A48.
/// Arithmetic uses wrapping guest addresses; ECX and EDX are preserved.
///
/// ```
/// use tsfp_ai::{gprs::Gprs, memory::GuestMemory, table_leaves::behaviour_table_entry};
/// struct Memory([u8; 16]);
/// impl GuestMemory for Memory {
///     fn read8(&self, address: u32) -> u8 { self.0.get(address as usize).copied().unwrap_or(0) }
///     fn write8(&mut self, address: u32, value: u8) { self.0[address as usize] = value; }
/// }
/// let mut memory = Memory([0; 16]);
/// memory.write32(4, 0); // group
/// memory.write32(8, 0); // slot
/// let result = behaviour_table_entry(&memory, 0, Gprs { eax: 9, ecx: 2, edx: 3 });
/// assert_eq!((result.eax, result.ecx, result.edx), (0, 2, 3));
/// ```
pub fn behaviour_table_entry<M: GuestMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let group = m.read32(esp.wrapping_add(4));
    let slot = m.read32(esp.wrapping_add(8));
    let index = group.wrapping_add(slot.wrapping_mul(4));
    Gprs {
        eax: m.read32(index.wrapping_mul(4).wrapping_add(BEHAVIOUR_TABLE)),
        ..e
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Memory;

    #[test]
    fn behaviour_table_wrapping_address_and_register_grid() {
        for seed in 0u32..256 {
            let mut memory = Memory::default();
            let esp = u32::MAX.wrapping_sub(seed & 7);
            let group = seed.wrapping_mul(0x0101_0101);
            let slot = u32::MAX.wrapping_sub(seed.wrapping_mul(0x0001_0001));
            let address = BEHAVIOUR_TABLE
                .wrapping_add(group.wrapping_add(slot.wrapping_mul(4)).wrapping_mul(4));
            let expected = seed ^ 0xa5a5_5a5a;
            memory.write32(esp.wrapping_add(4), group);
            memory.write32(esp.wrapping_add(8), slot);
            memory.write32(address, expected);
            let input = Gprs {
                eax: !seed,
                ecx: seed,
                edx: seed ^ u32::MAX,
            };
            let result = behaviour_table_entry(&memory, esp, input);
            assert_eq!(
                result,
                Gprs {
                    eax: expected,
                    ..input
                },
                "seed {seed}"
            );
        }
    }
}
