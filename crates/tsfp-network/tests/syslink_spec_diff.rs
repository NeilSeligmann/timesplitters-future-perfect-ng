//! T2387 Tier 1 differential: readable owned codec against the retained lifted 0x2EAF0 body.
//! The root in `syslink_bulk::translated` is a mechanical translation of the pinned lifted C,
//! not an original-machine-code or live-network oracle. The comparison is bounded to the exact
//! outer game-data bytes passed by 0x2EAF0 to 0x2E980.
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use tsfp_network::{
    memory::GuestMemory,
    syslink_bulk::{
        state::{NetworkCall, NetworkRegisters},
        translated,
    },
    syslink_codec::vdp::Control,
};

const ROOT_STACK: u32 = 0x0010_0000;
const PAYLOAD_PTR: u32 = 0x0020_0000;
const REMOTE_SLOT: u32 = 1;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<BTreeMap<u32, u8>>>);

impl Memory {
    fn put(&mut self, address: u32, bytes: &[u8]) {
        for (offset, byte) in bytes.iter().copied().enumerate() {
            self.0
                .borrow_mut()
                .insert(address.wrapping_add(offset as u32), byte);
        }
    }

    fn get(&self, address: u32, len: usize) -> Vec<u8> {
        (0..len)
            .map(|offset| {
                self.0
                    .borrow()
                    .get(&address.wrapping_add(offset as u32))
                    .copied()
                    .unwrap_or(0)
            })
            .collect()
    }
}

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().get(&address).copied().unwrap_or(0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
}

#[derive(Default)]
struct LiftedCalls {
    memory: Memory,
    game_data_headers: Vec<Vec<u8>>,
}

impl NetworkCall for LiftedCalls {
    fn call(&mut self, va: u32, regs: &mut NetworkRegisters) {
        match va {
            // 0x355EC0: host-role predicate; this fixture selects the remote-send branch.
            0x355EC0 => regs.eax = 1,
            // 0x2E980: capture the header pointer/length supplied by the lifted 0x2EAF0 caller.
            0x2E980 => {
                let pointer = self.memory.0.borrow();
                let read32 = |address: u32| {
                    u32::from_le_bytes(std::array::from_fn(|i| {
                        pointer
                            .get(&address.wrapping_add(i as u32))
                            .copied()
                            .unwrap_or(0)
                    }))
                };
                let address = read32(regs.esp);
                let length = read32(regs.esp.wrapping_add(4)) as usize;
                drop(pointer);
                self.game_data_headers
                    .push(self.memory.get(address, length));
                regs.eax = 0;
            }
            other => panic!("unexpected lifted callee 0x{other:08X}"),
        }
    }
}

#[test]
fn game_data_wrapper_matches_lifted_2eaf0_for_boundary_payloads() {
    let payload_lengths = [0, 1, 3, 4, 15, 16, 0xEF, 0xF0, 0x1FF, 0x200];
    for len in payload_lengths {
        let payload: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(37)).collect();
        let mut memory = Memory::default();
        memory.put(ROOT_STACK, &[0; 4]);
        memory.put(ROOT_STACK + 4, &REMOTE_SLOT.to_le_bytes());
        memory.put(ROOT_STACK + 8, &PAYLOAD_PTR.to_le_bytes());
        memory.put(ROOT_STACK + 12, &(len as u32).to_le_bytes());
        memory.put(0x666040 + REMOTE_SLOT * 4, &1u32.to_le_bytes());
        memory.put(0x774040, &0u32.to_le_bytes());
        memory.put(PAYLOAD_PTR, &payload);

        let mut calls = LiftedCalls {
            memory: memory.clone(),
            ..LiftedCalls::default()
        };
        let mut registers = NetworkRegisters {
            esp: ROOT_STACK,
            ..NetworkRegisters::default()
        };
        translated::root_0002eaf0(&mut memory, &mut calls, &mut registers);

        assert_eq!(calls.game_data_headers.len(), 1, "payload length {len}");
        let typed = Control::GameData(payload).encode_header();
        assert_eq!(calls.game_data_headers[0], typed, "payload length {len}");
        assert_eq!(&typed[..4], &0xDEAD_B33Fu32.to_le_bytes());
    }
}
