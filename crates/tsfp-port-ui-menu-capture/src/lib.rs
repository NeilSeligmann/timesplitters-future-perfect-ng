// SPDX-License-Identifier: GPL-3.0-or-later
//! Small attachment helpers for UI menu capture adapters.

use tsfp_port_runtime::{abi::ExtRegisters, memory::GuestMemory};

pub trait PageTickHost: GuestMemory {
    fn call(&mut self, target: u32, args: &[u32]) -> u32;
    fn call_f32(&mut self, target: u32, args: &[u32]) -> f32;
    fn call_x87_unary(&mut self, target: u32, argument: f32) -> f32;
    fn stack_pointer(&self) -> u32;
}

pub struct FinishedPageArg<'a, H> {
    host: &'a mut H,
    argument: u32,
}

impl<'a, H> FinishedPageArg<'a, H> {
    pub fn new(host: &'a mut H, argument: u32) -> Self {
        Self { host, argument }
    }
}

impl<H: PageTickHost> GuestMemory for FinishedPageArg<'_, H> {
    fn read8(&self, address: u32) -> u8 {
        self.host.read8(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.host.write8(address, value);
    }
}

impl<H: PageTickHost> PageTickHost for FinishedPageArg<'_, H> {
    fn call(&mut self, target: u32, args: &[u32]) -> u32 {
        if target == 0x70d90 && args.is_empty() {
            self.host.call(target, &[self.argument])
        } else {
            self.host.call(target, args)
        }
    }
    fn call_f32(&mut self, target: u32, args: &[u32]) -> f32 {
        self.host.call_f32(target, args)
    }
    fn call_x87_unary(&mut self, target: u32, argument: f32) -> f32 {
        self.host.call_x87_unary(target, argument)
    }
    fn stack_pointer(&self) -> u32 {
        self.host.stack_pointer()
    }
}

/// Run a page tick and publish the EAX/EDX left live by its final guest call.
pub fn run_page_tick<H, T, R>(
    host: &mut H,
    page: u32,
    registers: &mut ExtRegisters,
    tick: T,
    live_registers: R,
) where
    T: FnOnce(&mut H, u32),
    R: FnOnce(&H) -> (u32, u32),
{
    tick(host, page);
    (registers.eax, registers.edx) = live_registers(host);
}

#[cfg(test)]
mod tests {
    use super::run_page_tick;
    use tsfp_port_runtime::abi::{ExtRegisters, Fpu, Xmm};

    #[test]
    fn page_tick_publishes_final_guest_return_registers() {
        let mut host = (0, 0);
        let mut registers = ExtRegisters {
            eax: 1,
            ecx: 0,
            edx: 2,
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
        };
        run_page_tick(
            &mut host,
            42,
            &mut registers,
            |state, page| *state = (page, !page),
            |state| *state,
        );
        assert_eq!((registers.eax, registers.edx), (42, !42));
    }

    #[test]
    fn non_attract_page_tick_keeps_the_compared_menu_mode_in_eax() {
        let mut host = (8, (1, 0x11));
        let mut registers = ExtRegisters {
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
        };
        run_page_tick(
            &mut host,
            0,
            &mut registers,
            |state, _| {
                // The fade helper's EAX is not the routine's live return on this branch.
                state.1 = (1, 0x11);
            },
            |state| state.1,
        );
        let mode = host.0;
        if mode != 1 {
            registers.eax = mode;
        }
        assert_eq!(registers.eax, 8);
        assert_eq!(registers.edx, 0x11);
    }
}
