// SPDX-License-Identifier: GPL-3.0-or-later
#![no_std]
#![allow(unexpected_cfgs)]

pub mod abi {
    pub use tsfp_port_runtime::abi::*;
}

pub mod memory {
    pub use tsfp_port_runtime::memory::*;
}

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

pub use c2r::mapmaker_ui;

#[cfg(test)]
mod tests {
    use super::{abi::*, mapmaker_ui, memory::GuestMemory};

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
    fn mapmaker_ui_batch_keeps_its_csv_roots() {
        assert_eq!(
            mapmaker_ui::ROOTS,
            [
                0x30a550, 0x2c87b0, 0x3165c0, 0x319fc0, 0x326990, 0x32f740, 0x335980
            ]
        );
    }

    #[test]
    fn dispatcher_ignores_unowned_addresses() {
        assert!(!mapmaker_ui::run(
            u32::MAX,
            &mut UnusedMemory,
            &mut UnusedGuest,
            &mut regs(),
        ));
    }
}
