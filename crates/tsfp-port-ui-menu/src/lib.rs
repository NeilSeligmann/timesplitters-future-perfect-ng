// SPDX-License-Identifier: GPL-3.0-or-later
#![no_std]
#![allow(unexpected_cfgs)]

pub mod abi {
    pub use tsfp_port_runtime::abi::*;
}

pub mod memory {
    pub use tsfp_port_runtime::memory::*;
}

pub use tsfp_port_runtime::state_wire;

// T3610: the wiring layer is hosted once in tsfp-port-wire; batches keep their `crate::<module>` paths.
pub use tsfp_port_wire::{
    contract_views, leaf_wire, menu_state, missions_system, missions_wire, player_wire, views,
};

pub mod c2r;

#[cfg(tsfp_cover)]
pub mod pose {
    unsafe extern "C" {
        fn tsfp_port_cover(function: u32, from: u32, to: u32);
    }

    pub fn cover(function: u32, from: u32, to: u32) {
        unsafe { tsfp_port_cover(function, from, to) }
    }
}

pub use c2r::t2125_003;

#[cfg(test)]
mod tests {
    use super::{abi::*, memory::GuestMemory, t2125_003};

    struct UnusedMemory;

    impl GuestMemory for UnusedMemory {
        fn read8(&self, _address: u32) -> u8 {
            panic!("unknown C2R address must not read guest memory")
        }

        fn write8(&mut self, _address: u32, _value: u8) {
            panic!("unknown C2R address must not write guest memory")
        }
    }

    struct UnusedGuest;

    impl GuestCall for UnusedGuest {
        fn call(&mut self, _va: u32, _regs: &mut ExtRegisters) {
            panic!("unknown C2R address must not call the guest")
        }
    }

    fn regs() -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0,
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
    fn menu_batch_keeps_its_csv_root() {
        assert_eq!(t2125_003::ROOTS, [0x191b10]);
    }

    #[test]
    fn dispatcher_ignores_unowned_addresses() {
        assert!(!t2125_003::run(
            u32::MAX,
            &mut UnusedMemory,
            &mut UnusedGuest,
            &mut regs(),
        ));
    }
}
