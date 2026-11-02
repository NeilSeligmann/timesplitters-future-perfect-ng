// SPDX-License-Identifier: GPL-3.0-or-later
//! Copy of tsfp-network/src/callee.rs with `super::memory` (T1815 w7: this crate is also
//! path-included into tsfp-ports, where `crate::` is another root). Keep in sync.
//! T1848 wave 4b guest-callee model for call-bearing ports. Path-included by the ports crates
//! (like `leaf.rs`), so no crate gains a dependency. A port performs its nested guest call
//! through [`guest_call`], which mirrors the frame writes of `game_guest_call` in
//! `src/game/game_replace.h` (return address and arguments below the current ESP) and hands the
//! callee the full register file through a [`Callee`] implementation.
use super::memory::GuestMemory;

/// Register file a call-bearing replacement can observe or leave behind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cpu {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub ebx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebp: u32,
    pub esp: u32,
}

/// A guest callee. `cpu.esp` is the callee entry ESP: the return address is at `[esp]` and the
/// cdecl arguments follow. The callee may change EAX, ECX and EDX (and memory).
pub trait Callee<M: GuestMemory> {
    fn call(&mut self, m: &mut M, target: u32, cpu: &mut Cpu);
}

/// cdecl guest call (register ECX/EDX untouched). `caller_bytes` counts local dwords below ESP.
/// ESP is back at its entry value afterwards.
pub fn guest_call<M: GuestMemory, C: Callee<M>>(
    callee: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
    target: u32,
    return_pc: u32,
    caller_bytes: u32,
    args: &[u32],
) {
    let entry = cpu.esp;
    let top = entry.wrapping_sub(caller_bytes);
    let callee_esp = top.wrapping_sub(4 * (args.len() as u32 + 1));
    m.write32(callee_esp, return_pc);
    for (i, arg) in args.iter().enumerate() {
        m.write32(callee_esp.wrapping_add(4 + 4 * i as u32), *arg);
    }
    cpu.esp = callee_esp;
    callee.call(m, target, cpu);
    cpu.esp = entry;
}

/// Stack argument `index` of the running replacement (ESP is its entry value).
pub fn stack_arg<M: GuestMemory>(m: &M, cpu: &Cpu, index: u32) -> u32 {
    m.read32(cpu.esp.wrapping_add(4 + 4 * index))
}

/// Low byte of `register` replaced, like a partial-register write.
pub fn low_byte(register: u32, value: u32) -> u32 {
    (register & 0xffff_ff00) | (value & 0xff)
}
