// SPDX-License-Identifier: GPL-3.0-or-later
//! T1848 wave 4b FESL crypto/text ports: RC4 key setup 003C6730 and base64 encode 003BA270.
//! Both follow the C replacement statement by statement (register residue, partial-register
//! writes and the scratch stack words it keeps), see docs/t1848-rust-wave4.md.
use crate::{callee::{low_byte, stack_arg, Cpu}, memory::GuestMemory};

const BASE64_TABLE: u32 = 0x004b_5de8;

/// 003C6730 cdecl(state, key, key_length, passes). Initialises the 256 byte permutation at
/// `state + 2` (two index bytes at +0/+1 zeroed), then runs the unrolled-by-four key mixing
/// `passes` times (at least once). The loop keeps its counters in the caller's argument slots
/// (`sp+4`, `sp+12`, `sp+16`) exactly like the C. Residue: EAX/ECX/EDX final, EBX/EBP/EDI/ESI
/// restored from `sp-16..sp-28`.
pub fn rc4_key_setup<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    let sp = cpu.esp;
    let at = |offset: i32| sp.wrapping_add(offset as u32);
    cpu.ecx = stack_arg(m, cpu, 3);
    if (cpu.ecx as i32) < 1 {
        cpu.ecx = 1;
    }
    m.write32(at(-16), cpu.ebx);
    m.write32(at(-20), cpu.ebp);
    cpu.ebx = 0;
    m.write32(at(-24), cpu.esi);
    cpu.esi = stack_arg(m, cpu, 0);
    m.write8(cpu.esi, 0);
    m.write8(cpu.esi.wrapping_add(1), 0);
    cpu.eax = 0;
    loop {
        m.write8(cpu.esi.wrapping_add(cpu.eax).wrapping_add(2), cpu.eax as u8);
        cpu.eax = cpu.eax.wrapping_add(1);
        if cpu.eax >= 256 {
            break;
        }
    }
    cpu.ebp = stack_arg(m, cpu, 2);
    if (cpu.ebp as i32) > 0 && (cpu.ecx as i32) > 0 {
        cpu.eax = 0xffff_ffffu32.wrapping_sub(cpu.esi);
        m.write32(at(-12), cpu.eax);
        cpu.eax = 1u32.wrapping_sub(cpu.esi);
        m.write32(at(-8), cpu.eax);
        cpu.eax = 0xffff_fffeu32.wrapping_sub(cpu.esi);
        m.write32(at(-28), cpu.edi);
        cpu.edi = stack_arg(m, cpu, 1);
        m.write32(at(-4), cpu.eax);
        m.write32(at(12), cpu.ecx);
        let keybyte = |m: &M, cpu: &Cpu, index: u32| u32::from(m.read8(index.wrapping_add(cpu.edi)));
        loop {
            m.write32(at(4), 2);
            cpu.ecx = cpu.esi.wrapping_add(2);
            loop {
                // step 1
                cpu.edx = low_byte(cpu.edx, u32::from(m.read8(cpu.ecx)));
                m.write8(at(16), cpu.edx as u8);
                cpu.eax = cpu.eax.wrapping_add(cpu.ecx);
                cpu.edx = cpu.eax % cpu.ebp;
                cpu.eax /= cpu.ebp;
                cpu.eax = low_byte(cpu.eax, keybyte(m, cpu, cpu.edx));
                cpu.eax = low_byte(cpu.eax, cpu.eax.wrapping_add(u32::from(m.read8(at(16)))));
                cpu.ebx = low_byte(cpu.ebx, cpu.ebx.wrapping_add(cpu.eax));
                cpu.edx = cpu.ebx & 0xff;
                cpu.eax = cpu.edx.wrapping_add(cpu.esi).wrapping_add(2);
                cpu.edx = low_byte(cpu.edx, u32::from(m.read8(cpu.eax)));
                m.write8(cpu.ecx, cpu.edx as u8);
                cpu.edx = low_byte(cpu.edx, u32::from(m.read8(at(16))));
                m.write8(cpu.eax, cpu.edx as u8);
                // step 2
                cpu.eax = low_byte(cpu.eax, u32::from(m.read8(cpu.ecx.wrapping_add(1))));
                cpu.edx = m.read32(at(-12));
                m.write8(at(16), cpu.eax as u8);
                cpu.eax = cpu.edx.wrapping_add(cpu.ecx);
                cpu.edx = cpu.eax % cpu.ebp;
                cpu.eax /= cpu.ebp;
                cpu.eax = low_byte(cpu.eax, keybyte(m, cpu, cpu.edx));
                cpu.eax = low_byte(cpu.eax, cpu.eax.wrapping_add(u32::from(m.read8(at(16)))));
                cpu.ebx = low_byte(cpu.ebx, cpu.ebx.wrapping_add(cpu.eax));
                cpu.edx = cpu.ebx & 0xff;
                cpu.eax = cpu.edx.wrapping_add(cpu.esi).wrapping_add(2);
                cpu.edx = low_byte(cpu.edx, u32::from(m.read8(cpu.eax)));
                m.write8(cpu.ecx.wrapping_add(1), cpu.edx as u8);
                cpu.edx = low_byte(cpu.edx, u32::from(m.read8(at(16))));
                m.write8(cpu.eax, cpu.edx as u8);
                // step 3
                cpu.eax = low_byte(cpu.eax, u32::from(m.read8(cpu.ecx.wrapping_add(2))));
                m.write8(at(16), cpu.eax as u8);
                cpu.eax = m.read32(at(4));
                cpu.edx = cpu.eax % cpu.ebp;
                cpu.eax /= cpu.ebp;
                cpu.eax = low_byte(cpu.eax, u32::from(m.read8(at(16))));
                cpu.edx = low_byte(cpu.edx, keybyte(m, cpu, cpu.edx));
                cpu.edx = low_byte(cpu.edx, cpu.edx.wrapping_add(cpu.eax));
                cpu.ebx = low_byte(cpu.ebx, cpu.ebx.wrapping_add(cpu.edx));
                cpu.eax = cpu.ebx & 0xff;
                cpu.edx = low_byte(cpu.edx, u32::from(m.read8(cpu.eax.wrapping_add(cpu.esi).wrapping_add(2))));
                cpu.eax = cpu.eax.wrapping_add(cpu.esi).wrapping_add(2);
                m.write8(cpu.ecx.wrapping_add(2), cpu.edx as u8);
                cpu.edx = low_byte(cpu.edx, u32::from(m.read8(at(16))));
                m.write8(cpu.eax, cpu.edx as u8);
                // step 4
                cpu.eax = low_byte(cpu.eax, u32::from(m.read8(cpu.ecx.wrapping_add(3))));
                cpu.edx = m.read32(at(-8));
                m.write8(at(16), cpu.eax as u8);
                cpu.eax = cpu.edx.wrapping_add(cpu.ecx);
                cpu.edx = cpu.eax % cpu.ebp;
                cpu.eax /= cpu.ebp;
                cpu.ecx = cpu.ecx.wrapping_add(4);
                cpu.eax = low_byte(cpu.eax, keybyte(m, cpu, cpu.edx));
                cpu.eax = low_byte(cpu.eax, cpu.eax.wrapping_add(u32::from(m.read8(at(16)))));
                cpu.ebx = low_byte(cpu.ebx, cpu.ebx.wrapping_add(cpu.eax));
                cpu.edx = cpu.ebx & 0xff;
                cpu.eax = cpu.edx.wrapping_add(cpu.esi).wrapping_add(2);
                cpu.edx = low_byte(cpu.edx, u32::from(m.read8(cpu.eax)));
                m.write8(cpu.ecx.wrapping_sub(1), cpu.edx as u8);
                cpu.edx = low_byte(cpu.edx, u32::from(m.read8(at(16))));
                m.write8(cpu.eax, cpu.edx as u8);
                let counter = m.read32(at(4)).wrapping_add(4);
                m.write32(at(4), counter);
                cpu.eax = m.read32(at(-4));
                cpu.edx = cpu.eax.wrapping_add(cpu.ecx);
                if cpu.edx >= 256 {
                    break;
                }
            }
            let remaining = m.read32(at(12)).wrapping_sub(1);
            m.write32(at(12), remaining);
            if remaining == 0 {
                break;
            }
        }
        cpu.edi = m.read32(at(-28));
    }
    cpu.esi = m.read32(at(-24));
    cpu.ebp = m.read32(at(-20));
    cpu.ebx = m.read32(at(-16));
}

/// 003BA270 cdecl(length, source, destination), no return value. Encodes `length` bytes into
/// base64 text with the table at 4B5DE8 and a terminating NUL. Source bytes are re-read after
/// each destination store (aliasing safe), EAX/ECX/EDX are left untouched, EBX/ESI/EDI/EBP are
/// restored.
pub fn base64_encode<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    let sp = cpu.esp;
    let at = |offset: i32| sp.wrapping_add(offset as u32);
    let mut count = stack_arg(m, cpu, 0);
    let source = stack_arg(m, cpu, 1);
    let destination = stack_arg(m, cpu, 2);
    m.write32(at(-4), cpu.ebx);
    m.write32(at(-8), cpu.esi);
    m.write32(at(-12), cpu.edi);
    let table = |m: &M, index: u32| m.read8(BASE64_TABLE.wrapping_add(index));
    let (mut input, mut output) = (0u32, 0u32);
    let read = |m: &M, input: u32, extra: u32| u32::from(m.read8(source.wrapping_add(input).wrapping_add(extra)));
    if (count as i32) >= 3 {
        m.write32(at(-16), cpu.ebp);
        let mut groups = count / 3;
        count = count.wrapping_sub(groups.wrapping_mul(3));
        m.write32(at(4), count);
        loop {
            let a = read(m, input, 0);
            m.write8(destination.wrapping_add(output), table(m, a >> 2));
            let b = read(m, input, 1);
            let a = read(m, input, 0);
            m.write8(destination.wrapping_add(output).wrapping_add(1), table(m, (b >> 4).wrapping_add((a & 3) << 4)));
            let c = read(m, input, 2);
            let b = read(m, input, 1);
            m.write8(destination.wrapping_add(output).wrapping_add(2), table(m, ((b & 15) << 2).wrapping_add(c >> 6)));
            let c = read(m, input, 2);
            m.write8(destination.wrapping_add(output).wrapping_add(3), table(m, c & 63));
            input = input.wrapping_add(3);
            output = output.wrapping_add(4);
            groups = groups.wrapping_sub(1);
            if groups == 0 {
                break;
            }
        }
        count = m.read32(at(4));
        cpu.ebp = m.read32(at(-16));
    }
    if count == 2 {
        let a = read(m, input, 0);
        m.write8(destination.wrapping_add(output), table(m, a >> 2));
        let b = read(m, input, 1);
        let a = read(m, input, 0);
        m.write8(destination.wrapping_add(output).wrapping_add(1), table(m, (b >> 4).wrapping_add((a & 3) << 4)));
        let b = read(m, input, 1);
        m.write8(destination.wrapping_add(output).wrapping_add(2), table(m, (b & 15) << 2));
        m.write8(destination.wrapping_add(output).wrapping_add(3), b'=');
        output = output.wrapping_add(4);
    } else if count == 1 {
        let a = read(m, input, 0);
        m.write8(destination.wrapping_add(output), table(m, a >> 2));
        let a = read(m, input, 0);
        m.write8(destination.wrapping_add(output).wrapping_add(1), table(m, (a & 3) << 4));
        m.write8(destination.wrapping_add(output).wrapping_add(2), b'=');
        m.write8(destination.wrapping_add(output).wrapping_add(3), b'=');
        output = output.wrapping_add(4);
    }
    m.write8(destination.wrapping_add(output), 0);
    cpu.edi = m.read32(at(-12));
    cpu.esi = m.read32(at(-8));
    cpu.ebx = m.read32(at(-4));
}
