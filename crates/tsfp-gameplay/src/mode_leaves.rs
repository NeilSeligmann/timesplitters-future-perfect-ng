// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 7 gameplay (mode, state and weapon) leaves. Each body ports a call-free C
//! replacement of `src/game`, see docs/t1836-rust-wave7.md. Every function takes the entry
//! registers `e` and returns the exact final EAX/ECX/EDX. Stack arguments are read through `esp`
//! (the guest stack pointer at entry, return address at `esp`) so a store aliasing an argument
//! slot behaves as in the C body. Reads and writes keep the C order.
use crate::{gprs::Gprs, memory::GuestMemory};

const MODE: u32 = 0x0079_094c;
// VA 0x00230510: the signed state number selects one of two guest descriptor tables.
const CURRENT_STATE: u32 = 0x007d_e470;
const DESCRIPTOR_SPLIT_STATE: i32 = 0x15;
const LOW_STATE_DESCRIPTORS: u32 = 0x0051_0788;
const HIGH_STATE_DESCRIPTORS: u32 = 0x0051_3d04;
const DESCRIPTOR_STRIDE: u32 = 4;
// VA 0x00264740: the argument owns a child pointer; the child's state gates a mode write.
const OBJECT_CHILD: u32 = 0x7c;
const CHILD_STATE: u32 = 0x20;
const CHILD_MODE: u32 = 0x9d0;
const CHILD_ACTIVE_MODE: u32 = 3;
// VA 0x00275620: two byte-sized state fields gate the mode predicate.
const MODE_GATE_BYTE: u32 = 0x007d_e402;
const MODE_KIND_BYTE: u32 = 0x007d_e455;
const MATCHING_MODE_KIND: u8 = 2;
const LOW_BYTE_MASK: u32 = 0xff;

/// 00240580 game_mode_79094c_is_0x18. cdecl(0).
pub fn mode_is_0x18<M: GuestMemory>(m: &M, e: Gprs) -> Gprs {
    let ecx = m.read32(MODE);
    Gprs {
        eax: u32::from(ecx == 24),
        ecx,
        edx: e.edx,
    }
}

/// 00233F90 game_get_global_75ccb8_unless_mode_79094c_is_0x65. cdecl(0).
// portreg: va=0x00233f90 tier=1
pub fn global_75ccb8_unless_mode_0x65<M: GuestMemory>(m: &M, e: Gprs) -> Gprs {
    let selected = u32::from(m.read32(MODE) == 0x65);
    let ecx = m.read32(0x0075_ccb8);
    Gprs {
        eax: selected.wrapping_sub(1) & ecx,
        ecx,
        edx: e.edx,
    }
}

/// 00243060 game_mode_79094c_select_minus1_or_3066. cdecl(0).
// portreg: va=0x00243060 tier=1
pub fn mode_select_minus1_or_3066<M: GuestMemory>(m: &M, e: Gprs) -> Gprs {
    let ecx = m.read32(MODE);
    Gprs {
        eax: if ecx == 4 { u32::MAX } else { 3066 },
        ecx,
        edx: e.edx,
    }
}

/// VA 0x00230510: read the descriptor pointer for the current signed state.
/// Both tables use the raw state as a dword index, including negative states.
// portreg: va=0x00230510 tier=1
pub fn state_descriptor_for_current_state<M: GuestMemory>(m: &M, e: Gprs) -> Gprs {
    let state = m.read32(CURRENT_STATE);
    let table = if (state as i32) < DESCRIPTOR_SPLIT_STATE {
        LOW_STATE_DESCRIPTORS
    } else {
        HIGH_STATE_DESCRIPTORS
    };
    Gprs {
        eax: m.read32(state.wrapping_mul(DESCRIPTOR_STRIDE).wrapping_add(table)),
        ecx: e.ecx,
        edx: e.edx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct SparseMemory(BTreeMap<u32, u8>);

    impl GuestMemory for SparseMemory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[test]
    fn descriptor_split_uses_signed_state_and_preserves_other_registers() {
        let mut memory = SparseMemory::default();
        let entry = Gprs {
            eax: 9,
            ecx: 7,
            edx: 5,
        };
        for (state, table, expected) in [
            (0x14u32, LOW_STATE_DESCRIPTORS, 0x1111u32),
            (0x15, HIGH_STATE_DESCRIPTORS, 0x2222),
            (u32::MAX, LOW_STATE_DESCRIPTORS, 0x3333),
        ] {
            memory.write32(CURRENT_STATE, state);
            memory.write32(
                table.wrapping_add(state.wrapping_mul(DESCRIPTOR_STRIDE)),
                expected,
            );
            assert_eq!(
                state_descriptor_for_current_state(&memory, entry),
                Gprs {
                    eax: expected,
                    ecx: 7,
                    edx: 5
                }
            );
        }
    }

    #[test]
    fn child_mode_changes_only_for_active_states() {
        let stack = 0x1000;
        let object = 0x2000;
        let child = 0x3000;
        let mut memory = SparseMemory::default();
        memory.write32(stack + 4, object);
        memory.write32(object + OBJECT_CHILD, child);
        for state in [0, 1, 2, 3, 4, 5] {
            memory.write32(child + CHILD_STATE, state);
            memory.write32(child + CHILD_MODE, 9);
            let result = set_mode_three_unless_done(
                &mut memory,
                stack,
                Gprs {
                    eax: 7,
                    ecx: 8,
                    edx: 11,
                },
            );
            assert_eq!(
                result,
                Gprs {
                    eax: state,
                    ecx: child,
                    edx: 11
                }
            );
            assert_eq!(
                memory.read32(child + CHILD_MODE),
                if matches!(state, 0 | 1 | 2 | 4) {
                    CHILD_ACTIVE_MODE
                } else {
                    9
                }
            );
        }
    }

    #[test]
    fn mode_byte_predicate_preserves_partial_register_writes() {
        let mut memory = SparseMemory::default();
        let entry = Gprs {
            eax: 0x1234_5678,
            ecx: 0xabcd_ef90,
            edx: 0x1357_2468,
        };
        assert_eq!(
            byte_7de402_nonzero_and_mode_byte_is_2(&memory, entry),
            Gprs {
                eax: 0,
                ecx: entry.ecx,
                edx: entry.edx
            }
        );
        memory.write8(MODE_GATE_BYTE, 1);
        for (kind, expected) in [(2, 1), (3, 0)] {
            memory.write8(MODE_KIND_BYTE, kind);
            assert_eq!(
                byte_7de402_nonzero_and_mode_byte_is_2(&memory, entry),
                Gprs {
                    eax: expected,
                    ecx: 0xabcd_ef00 | u32::from(kind),
                    edx: entry.edx
                }
            );
        }
    }
}

/// VA 0x00264740: set the child mode to 3 while its state is 0, 1, 2 or 4.
/// cdecl(1): object pointer. EAX returns the observed child state; ECX returns the child pointer.
pub fn set_mode_three_unless_done<M: GuestMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let object = m.read32(esp.wrapping_add(4));
    let child = m.read32(object.wrapping_add(OBJECT_CHILD));
    let state = m.read32(child.wrapping_add(CHILD_STATE));
    if matches!(state, 0 | 1 | 2 | 4) {
        m.write32(child.wrapping_add(CHILD_MODE), CHILD_ACTIVE_MODE);
    }
    Gprs {
        eax: state,
        ecx: child,
        edx: e.edx,
    }
}

/// VA 0x00275620: check whether the gate byte is set and the mode-kind byte equals two.
/// The guest replaces EAX's low byte before the gate check and ECX's low byte only when
/// the gate is set. The final EAX is the Boolean result; EDX is preserved.
// portreg: va=0x00275620 tier=1
pub fn byte_7de402_nonzero_and_mode_byte_is_2<M: GuestMemory>(m: &M, e: Gprs) -> Gprs {
    let gate_byte = m.read8(MODE_GATE_BYTE);
    if gate_byte == 0 {
        return Gprs {
            eax: 0,
            ecx: e.ecx,
            edx: e.edx,
        };
    }
    let mode_kind = m.read8(MODE_KIND_BYTE);
    let ecx = (e.ecx & !LOW_BYTE_MASK) | u32::from(mode_kind);
    Gprs {
        eax: u32::from(mode_kind == MATCHING_MODE_KIND),
        ecx,
        edx: e.edx,
    }
}

/// 001B58C0 game_weapon_entry_flag_80. cdecl(1): weapon id. Entries are 0x29C bytes at [4F9BAC].
pub fn weapon_entry_flag_80<M: GuestMemory>(m: &M, esp: u32, _e: Gprs) -> Gprs {
    let arg = m.read32(esp.wrapping_add(4));
    let ecx = m.read32(arg.wrapping_add(0x30));
    let edx = m.read32(0x004f_9bac);
    let ecx = ecx.wrapping_mul(0x29c);
    let eax = m.read32(ecx.wrapping_add(edx).wrapping_add(0xc)) & 0x80;
    Gprs { eax, ecx, edx }
}

/// 0035FD70 game_get_selected_online_record_index_in_mode_0x65_else_field_0x38_of_arg. cdecl(1).
pub fn selected_record_index_or_field_0x38<M: GuestMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let eax = if m.read32(MODE) == 0x65 {
        m.read32(0x0054_5d80)
    } else {
        let arg = m.read32(esp.wrapping_add(4));
        m.read32(arg.wrapping_add(0x38))
    };
    Gprs {
        eax,
        ecx: e.ecx,
        edx: e.edx,
    }
}

/// 002ED7B0 game_clear_state_block_761c58. cdecl(0). EAX = 0, eleven stores in C order.
pub fn clear_state_block_761c58<M: GuestMemory>(m: &mut M, e: Gprs) -> Gprs {
    for (address, value) in [
        (0x0076_1c58, 0),
        (0x0076_25f4, 0),
        (0x0076_1c7c, 1),
        (0x0076_1c68, 0),
        (0x0076_1c6c, 0),
        (0x0076_1c70, 0),
        (0x0076_1c74, 0),
        (0x0076_1bdc, 0),
        (0x0076_1be0, 0),
        (0x0076_1be4, 0),
        (0x0076_1be8, 0),
    ] {
        m.write32(address, value);
    }
    Gprs {
        eax: 0,
        ecx: e.ecx,
        edx: e.edx,
    }
}
