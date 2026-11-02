// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline-only adapter for `0x00028D40` (T2 item `t2_wire-engine-video_1`).
//!
//! The lifted body reads its one stack argument into EAX and tail-jumps to `0x28CB0`.
//! Mirror `video.rs::f28d40`'s tail-call stack and register handling here so the capture
//! checker can run the existing hand port without adding a live selector entry.

use tsfp_port_runtime::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

pub fn f28d40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let return_slot = entry.esp;
    let saved_return = m.read32(return_slot);

    let mut callee = entry;
    callee.eax = m.read32(entry.esp.wrapping_add(4));
    // `tail_to!` removes the current return address while invoking the target.
    callee.esp = entry.esp.wrapping_add(4);
    guest.call(0x28cb0, &mut callee);

    // The source port's `tail_to!` restores the return word and original ESP after its
    // guest call; it forwards general/XMM results while retaining entry flags and x87.
    *regs = ExtRegisters {
        eax: callee.eax,
        ecx: callee.ecx,
        edx: callee.edx,
        esp: entry.esp,
        ebx: callee.ebx,
        ebp: callee.ebp,
        esi: callee.esi,
        edi: callee.edi,
        eflags: entry.eflags,
        fpu: entry.fpu,
        xmm: callee.xmm,
        xmm_hi: callee.xmm_hi,
    };
    m.write32(return_slot, saved_return);
}
