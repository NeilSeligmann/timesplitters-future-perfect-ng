// SPDX-License-Identifier: GPL-3.0-or-later
//! Small direct edges of the T2049 mission-action dispatcher.
//! Source contracts and verification debt: docs/t2049-manual-audit.md.
use crate::abi::ExtRegisters;
use crate::memory::GuestMemory;
use crate::views::generated_views::TsfpMissionActionGateEntryView;

/// Two independent byte gates in a 20-byte mission entry.
/// Their gameplay meanings are unresolved; neither byte is mutated by the predicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryGates {
    pub first: u8,
    pub second: u8,
}
impl EntryGates {
    pub fn both_set(self) -> bool {
        self.first != 0 && self.second != 0
    }
}

/// Opaque action value: identity/units remain unresolved, so retain all 32 bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActionValue(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActionState {
    pub value: ActionValue,
}
impl ActionState {
    pub fn set_value(&mut self, value: ActionValue) {
        self.value = value;
    }
}

// portreg: va=0x0006b7e0
/// cdecl(entry_index); ECX retains the table pointer's upper bytes and last read byte.
pub fn entry_gates_guest<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    let index = m.read32(r.esp.wrapping_add(4));
    let table = m.read32(0x6f2ed8);
    let entry = TsfpMissionActionGateEntryView(table.wrapping_add(
        index.wrapping_mul(TsfpMissionActionGateEntryView::SIZE),
    ));
    let first = entry.first_gate(m);
    r.ecx = (table & 0xffffff00) | u32::from(first);
    // Preserve the original short circuit: the second byte need not be mapped on a miss.
    let second = if first == 0 {
        0
    } else {
        let second = entry.second_gate(m);
        r.ecx = (table & 0xffffff00) | u32::from(second);
        second
    };
    r.eax = u32::from(EntryGates { first, second }.both_set());
}

// portreg: va=0x0025fa40
/// cdecl(value); publishes exactly one dword, returns the same value in EAX.
pub fn action_value_guest<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    let value = ActionValue(m.read32(r.esp.wrapping_add(4)));
    let mut state = ActionState::default();
    state.set_value(value);
    m.write32(0x75f5e0, state.value.0);
    r.eax = value.0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, vec::Vec};

    struct ReadLog {
        first: u8,
        second: u8,
        reads: RefCell<Vec<u32>>,
    }
    impl GuestMemory for ReadLog {
        fn read8(&self, address: u32) -> u8 {
            self.reads.borrow_mut().push(address);
            match address {
                0x1010 => self.first,
                0x1011 => self.second,
                _ => panic!("unexpected byte read at {address:x}"),
            }
        }
        fn write8(&mut self, _: u32, _: u8) { panic!("unexpected write") }
        fn read32(&self, address: u32) -> u32 {
            self.reads.borrow_mut().push(address);
            match address {
                0x2004 => 0,
                0x6f2ed8 => 0x1000,
                _ => panic!("unexpected word read at {address:x}"),
            }
        }
    }

    #[test]
    fn gate_reads_are_live_ordered_and_short_circuit() {
        for (first, second, expected_reads, eax, ecx) in [
            (0, 1, vec![0x2004, 0x6f2ed8, 0x1010], 0, 0x1000),
            (2, 0, vec![0x2004, 0x6f2ed8, 0x1010, 0x1011], 0, 0x1000),
            (2, 3, vec![0x2004, 0x6f2ed8, 0x1010, 0x1011], 1, 0x1003),
        ] {
            let mut memory = ReadLog { first, second, reads: RefCell::new(Vec::new()) };
            let mut registers: ExtRegisters = unsafe { core::mem::zeroed() };
            registers.esp = 0x2000;
            entry_gates_guest(&mut memory, &mut registers);
            assert_eq!(registers.eax, eax);
            assert_eq!(registers.ecx, ecx);
            assert_eq!(registers.esp, 0x2000);
            assert_eq!(*memory.reads.borrow(), expected_reads);
        }
    }
}
