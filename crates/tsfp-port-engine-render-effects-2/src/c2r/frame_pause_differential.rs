use std::collections::BTreeMap;

use crate::c2r::rt::{ExtRegisters, Fpu, GuestMemory, Xmm};
use crate::c2r::t2049_nx_003::game_global_7497b4_update_state_from_arg_flag;
use tsfp_engine_core::frame_state::FramePauseState;

const PAUSE_WORD: u32 = 0x0074_97b4;
const ARGUMENT: u32 = 0x0000_1004;

#[derive(Default)]
struct PauseMemory(BTreeMap<u32, u8>);

impl GuestMemory for PauseMemory {
    fn read8(&self, address: u32) -> u8 {
        self.0.get(&address).copied().unwrap_or(0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: ARGUMENT - 4,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

#[test]
fn rust_pause_owner_matches_lifted_setter_for_seeded_requests() {
    let mut random = 0x153a_c0u32;
    for seed_index in 0..2048 {
        random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let initial = random;
        let mut lifted = PauseMemory::default();
        lifted.write32(PAUSE_WORD, initial);
        let mut regs = registers();
        let owner = FramePauseState::new();
        owner.initialize(initial);

        for request_index in 0..16 {
            random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let requested = random >> 31 != 0;
            lifted.write32(ARGUMENT, u32::from(requested));
            game_global_7497b4_update_state_from_arg_flag(&mut lifted, &mut regs);
            let rust_word = owner.set_from_flag(requested);
            assert_eq!(
                rust_word,
                lifted.read32(PAUSE_WORD),
                "seed {seed_index}, request {request_index}, requested={requested}, initial={initial:#010x}"
            );
        }
    }
}
