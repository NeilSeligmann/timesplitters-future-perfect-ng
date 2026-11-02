// SPDX-License-Identifier: GPL-3.0-or-later
//! T1815 closure-model ports: object roots that call other guest functions through the
//! [`callee`](super::callee) model. Each body mirrors its unmodified C replacement
//! (`src/game/*.c`), including the saved-register slots below ESP. See docs/t1815-closure-model.md.
use super::{
    callee::{Callee, Cpu, guest_call, stack_arg},
    memory::GuestMemory,
};

/// 0006B4E0 cdecl(mask): refreshes the 12-byte pending records 0x6F1E70..0x6F2ED8 and sets or
/// clears bit 0 of the owner flag word at +0x28 (predicate 001656D0 on [owner+0x80]).
// portreg: va=0x0006b4e0 tier=0
pub fn pending_records_refresh_owner_flag<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let entry = cpu.esp;
    m.write32(entry.wrapping_sub(4), cpu.ebx);
    m.write32(entry.wrapping_sub(8), cpu.esi);
    m.write32(entry.wrapping_sub(12), cpu.edi);
    cpu.edi = 0x006F_1E70;
    cpu.ebx = 1;
    loop {
        cpu.eax = m.read32(cpu.edi);
        if cpu.eax == 0 {
            cpu.esi = 0;
        } else {
            cpu.edx = m.read32(0x0072_F5E8);
            cpu.esi = (cpu.eax >> 22).wrapping_mul(0x2E8);
            cpu.ecx = m.read32(cpu.esi.wrapping_add(cpu.edx).wrapping_add(0x2C));
            cpu.esi = cpu.esi.wrapping_add(cpu.edx);
            if cpu.ecx != cpu.eax {
                m.write32(cpu.edi, 0);
                cpu.esi = 0;
            }
        }
        if m.read8(cpu.edi.wrapping_add(8)) & 4 != 0 {
            if cpu.esi == 0 {
                m.write32(cpu.edi, 0);
                m.write32(cpu.edi.wrapping_add(4), 0xFFFF_FFFF);
                m.write32(cpu.edi.wrapping_add(8), 0);
            } else {
                let mut held = false;
                cpu.eax = m.read32(cpu.esi.wrapping_add(0x80));
                if cpu.eax != 0 {
                    let argument = cpu.eax;
                    guest_call(c, m, cpu, 0x0016_56D0, 0x0006_B533, 12, &[argument]);
                    held = cpu.eax != 0;
                }
                let flags = cpu.esi.wrapping_add(0x28);
                if held {
                    cpu.eax = m.read32(flags) | cpu.ebx;
                    m.write32(flags, cpu.eax);
                } else {
                    cpu.eax = stack_arg(m, cpu, 0);
                    if m.read32(cpu.edi.wrapping_add(4)) & cpu.eax != 0 {
                        if m.read32(cpu.esi.wrapping_add(0x20)) != 0x40 || m.read8(flags) & 2 != 0 {
                            cpu.eax = m.read32(flags) | cpu.ebx;
                            m.write32(flags, cpu.eax);
                        }
                    } else if m.read8(cpu.edi.wrapping_add(8)) & (cpu.ebx as u8) != 0 {
                        cpu.eax = m.read32(flags) & 0xFFFF_FFFE;
                        m.write32(flags, cpu.eax);
                    }
                }
            }
        }
        cpu.edi = cpu.edi.wrapping_add(0xC);
        if (cpu.edi as i32) >= 0x006F_2ED8 {
            break;
        }
    }
    cpu.edi = m.read32(entry.wrapping_sub(12));
    cpu.esi = m.read32(entry.wrapping_sub(8));
    cpu.ebx = m.read32(entry.wrapping_sub(4));
}

/// 000AF330 cdecl(holder): publishes [holder+0x7C]+0x54 into the object that the 000B7600 lookup
/// (keyed by [[holder+0x7C]+0x60]) reaches through its +8 -> +4 chain, at +0x30.
pub fn linked_object_flags_from_ext<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let holder = stack_arg(m, cpu, 0);
    cpu.eax = holder;
    if holder == 0 {
        return;
    }
    let slot = cpu.esp.wrapping_sub(4);
    m.write32(slot, cpu.esi);
    cpu.esi = m.read32(holder.wrapping_add(0x7C));
    cpu.eax = m.read32(cpu.esi.wrapping_add(0x60));
    let key = cpu.eax;
    guest_call(c, m, cpu, 0x000B_7600, 0x000A_F345, 4, &[key]);
    if cpu.eax != 0 {
        cpu.ecx = m.read32(cpu.eax.wrapping_add(8));
        cpu.edx = m.read32(cpu.ecx.wrapping_add(4));
        cpu.eax = m.read32(cpu.esi.wrapping_add(0x54));
        m.write32(cpu.edx.wrapping_add(0x30), cpu.eax);
    }
    cpu.esi = m.read32(slot);
}

/// 00160B80 cdecl(object): entry of the 32-byte record table at [0x715F2C] indexed by
/// [object+0x678], unless mode byte [0x7DE455] is 0x0A and the 0009CCA0 lookup on
/// [[object+0xBF0]+0x10] yields an entry whose +4 has bit 0x800 and whose signed word at +8
/// equals the record's first dword (then EAX is the lookup result).
pub fn record_entry_by_field_or_mode_override<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    cpu.eax = stack_arg(m, cpu, 0);
    cpu.edx = m.read32(0x0071_5F2C);
    cpu.ecx = (cpu.ecx & 0xFFFF_FF00) | u32::from(m.read8(0x007D_E455));
    let slot = cpu.esp.wrapping_sub(4);
    m.write32(slot, cpu.esi);
    cpu.esi = (m.read32(cpu.eax.wrapping_add(0x678)) << 5).wrapping_add(cpu.edx);
    if cpu.ecx & 0xFF == 0x0A {
        cpu.eax = m.read32(cpu.eax.wrapping_add(0xBF0));
        if cpu.eax != 0 {
            let argument = m.read32(cpu.eax.wrapping_add(0x10));
            cpu.eax = argument;
            guest_call(c, m, cpu, 0x0009_CCA0, 0x0016_0BB4, 4, &[argument]);
            if cpu.eax != 0 {
                cpu.ecx = m.read32(cpu.eax.wrapping_add(4));
                if cpu.ecx & 0x800 != 0 {
                    cpu.ecx = i32::from(m.read16(cpu.eax.wrapping_add(8)) as i16) as u32;
                    if m.read32(cpu.esi) == cpu.ecx {
                        cpu.esi = m.read32(slot);
                        return;
                    }
                }
            }
        }
    }
    cpu.eax = cpu.esi;
    cpu.esi = m.read32(slot);
}
