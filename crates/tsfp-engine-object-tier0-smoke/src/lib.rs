// SPDX-License-Identifier: GPL-3.0-or-later
//! Isolated fabricated smoke checks for oversized generated engine/object family crates.

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tsfp_port_runtime::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
    use tsfp_port_runtime::memory::GuestMemory;

    #[derive(Default)]
    struct Mem(BTreeMap<u32, u8>);

    impl GuestMemory for Mem {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[derive(Default)]
    struct Calls(usize);

    impl GuestCall for Calls {
        fn call(&mut self, _va: u32, _regs: &mut ExtRegisters) {
            self.0 += 1;
        }
    }

    fn regs(esp: u32) -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp,
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
    fn timed_box_update_empty_record_list_is_a_no_call_smoke() {
        let mut memory = Mem::default();
        let stack = 0x10000;
        let object = 0x20000;
        memory.write32(stack + 4, object);
        memory.write32(object + 0x1c, 0);
        let mut registers = regs(stack);
        let mut calls = Calls::default();

        tsfp_port_engine_object_4::c2r::engine_object_3::game_object_update_timed_box_records_fade_decal_slots_and_mark_room_table_708778_bits_by_p(
            &mut memory,
            &mut calls,
            &mut registers,
        );

        assert_eq!(registers.esp, stack);
        assert_eq!(calls.0, 0);
    }
}
