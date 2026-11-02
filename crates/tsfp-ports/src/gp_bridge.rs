// SPDX-License-Identifier: GPL-3.0-or-later
//! T1923: bridges from the hybrid lane (`ExtRegisters` plus `GuestCall`, see `lane_inv.rs`) to the two call models the
//! gameplay ports use: the T1815 closure model (`Callee` plus `Cpu`, used by `t1900_*`) and the system host traits
//! (`ModeHost`, `ProfileHost`, `UnlockHost`, ...). Every nested call runs the retained lifted original through
//! `GuestCall::call`, exactly like `call_cdecl` in `abi.rs`: the arguments sit at `[esp]..`, the C side pushes the return slot.
use crate::{
    abi::{ExtRegisters, Fpu, GuestCall},
    callee::{Callee, Cpu},
    memory::GuestMemory,
};
use core::cell::RefCell;

/// Distance below the entry ESP at which host-trait calls place their argument block. The ports hand out addresses of their
/// own stack locals (a few hundred bytes below the entry ESP), the callee frame must stay clear of them.
pub const CALL_DEPTH: u32 = 0x800;

/// The entry register file as a closure-model `Cpu` (ESP is the return slot, arguments follow it).
pub fn cpu_of(regs: &ExtRegisters) -> Cpu {
    Cpu {
        eax: regs.eax,
        ecx: regs.ecx,
        edx: regs.edx,
        ebx: regs.ebx,
        esi: regs.esi,
        edi: regs.edi,
        ebp: regs.ebp,
        esp: regs.esp,
    }
}

/// Registers a port left behind (everything but ESP, which the lane checks separately).
pub fn store_cpu(regs: &mut ExtRegisters, cpu: &Cpu) {
    regs.eax = cpu.eax;
    regs.ecx = cpu.ecx;
    regs.edx = cpu.edx;
    regs.ebx = cpu.ebx;
    regs.esi = cpu.esi;
    regs.edi = cpu.edi;
    regs.ebp = cpu.ebp;
}

/// `Callee` over the lifted originals. The x87 state is reset to the entry state before every call (the ports under
/// test call from a balanced FPU stack), so a float result can be read with [`CalleeBridge::st0`] without popping.
pub struct CalleeBridge<'a, G: GuestCall> {
    guest: &'a mut G,
    regs: ExtRegisters,
    entry_fpu: Fpu,
}

impl<'a, G: GuestCall> CalleeBridge<'a, G> {
    pub fn new(guest: &'a mut G, entry: &ExtRegisters) -> Self {
        Self {
            guest,
            regs: *entry,
            entry_fpu: entry.fpu,
        }
    }
    /// ST0 as the most recent call left it.
    pub fn st0(&self) -> f64 {
        self.regs.fpu.st(0)
    }
}

impl<M: GuestMemory, G: GuestCall> Callee<M> for CalleeBridge<'_, G> {
    fn call(&mut self, _m: &mut M, target: u32, cpu: &mut Cpu) {
        self.regs.fpu = self.entry_fpu;
        self.regs.eax = cpu.eax;
        self.regs.ecx = cpu.ecx;
        self.regs.edx = cpu.edx;
        self.regs.ebx = cpu.ebx;
        self.regs.esi = cpu.esi;
        self.regs.edi = cpu.edi;
        self.regs.ebp = cpu.ebp;
        // `cpu.esp` is the callee entry ESP (return slot), the lane call wants the argument block.
        self.regs.esp = cpu.esp.wrapping_add(4);
        self.guest.call(target, &mut self.regs);
        cpu.eax = self.regs.eax;
        cpu.ecx = self.regs.ecx;
        cpu.edx = self.regs.edx;
        cpu.ebx = self.regs.ebx;
        cpu.esi = self.regs.esi;
        cpu.edi = self.regs.edi;
        cpu.ebp = self.regs.ebp;
    }
}

/// Shared guest memory: the host bridge and the pure ports (which sometimes take `mem` and `host` as two arguments) both reach
/// the same `MappedGuest` through one `RefCell`.
pub struct SharedMemory<'a, M: GuestMemory>(pub &'a RefCell<&'a mut M>);

impl<M: GuestMemory> GuestMemory for SharedMemory<'_, M> {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().read8(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().write8(address, value)
    }
}

/// Guest memory plus the lifted originals as nested calls, for the system host traits.
pub struct HostBridge<'a, M: GuestMemory, G: GuestCall> {
    memory: &'a RefCell<&'a mut M>,
    guest: &'a mut G,
    regs: ExtRegisters,
    entry_fpu: Fpu,
    entry_esp: u32,
}

impl<'a, M: GuestMemory, G: GuestCall> HostBridge<'a, M, G> {
    pub fn new(memory: &'a RefCell<&'a mut M>, guest: &'a mut G, entry: &ExtRegisters) -> Self {
        Self {
            memory,
            guest,
            regs: *entry,
            entry_fpu: entry.fpu,
            entry_esp: entry.esp,
        }
    }
    /// A second handle on the same guest memory (for `mem` + `host` argument pairs).
    pub fn memory(&self) -> SharedMemory<'a, M> {
        SharedMemory(self.memory)
    }
    /// cdecl call with `args` on the stack below the entry frame; returns EAX.
    pub fn call_cdecl(&mut self, target: u32, args: &[u32]) -> u32 {
        self.regs.fpu = self.entry_fpu;
        let block = self
            .entry_esp
            .wrapping_sub(CALL_DEPTH)
            .wrapping_sub(4 * args.len() as u32);
        {
            let mut m = self.memory.borrow_mut();
            for (i, a) in args.iter().enumerate() {
                m.write32(block.wrapping_add(4 * i as u32), *a);
            }
        }
        self.regs.esp = block;
        self.guest.call(target, &mut self.regs);
        self.regs.eax
    }
    /// EAX and EDX after the most recent nested call, or their entry values if none ran.
    pub fn return_regs(&self) -> (u32, u32) {
        (self.regs.eax, self.regs.edx)
    }
    /// Call whose result is a float in ST0.
    pub fn call_float(&mut self, target: u32, args: &[u32]) -> f32 {
        self.call_cdecl(target, args);
        self.regs.fpu.st(0) as f32
    }
    /// Call with one x87 argument in ST0 and the result in ST0.
    pub fn call_x87_unary(&mut self, target: u32, argument: f32) -> f32 {
        self.regs.fpu = self.entry_fpu;
        self.regs.fpu.push(f64::from(argument));
        let saved = self.regs.fpu;
        self.regs.esp = self.entry_esp.wrapping_sub(CALL_DEPTH);
        self.guest.call(target, &mut self.regs);
        let result = self.regs.fpu.st(0) as f32;
        self.regs.fpu = saved;
        result
    }
    pub fn entry_esp(&self) -> u32 {
        self.entry_esp
    }
}

impl<M: GuestMemory, G: GuestCall> GuestMemory for HostBridge<'_, M, G> {
    fn read8(&self, address: u32) -> u8 {
        self.memory.borrow().read8(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.memory.borrow_mut().write8(address, value)
    }
}
