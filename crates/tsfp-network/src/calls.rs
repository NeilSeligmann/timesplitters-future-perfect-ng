// SPDX-License-Identifier: GPL-3.0-or-later
//! T1848 wave 4b network ports of C replacements that make nested guest calls (and the
//! stdcall roots). Each follows its C replacement statement by statement: register assignment
//! order, the saved-register words written below ESP, the callee frame (`caller_bytes`, return
//! PC) and the callee leftovers in EAX/ECX/EDX stay visible. The callee is a [`Callee`], so the
//! original guest callee is stubbed by a recorded value script in the differential. See
//! docs/t1848-rust-wave4.md. Stack arguments are read from `cpu.esp` (the entry ESP).
use crate::{callee::{guest_call, low_byte, stack_arg, Callee, Cpu}, memory::GuestMemory};

fn leaf<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu, target: u32, return_pc: u32) {
    guest_call(c, m, cpu, target, return_pc, 0, &[]);
}

fn leaf_below<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu, target: u32, return_pc: u32, below: u32) {
    guest_call(c, m, cpu, target, return_pc, below, &[]);
}

fn one<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu, target: u32, return_pc: u32, below: u32, arg: u32) {
    guest_call(c, m, cpu, target, return_pc, below, &[arg]);
}

/// 003B5FE0 stdcall(3): state word of arg0 (+0x28) with the top byte set and, when the low byte
/// of arg2 is nonzero, 0x30000000 added. ECX low byte = arg2 low byte, EDX untouched.
pub fn state_003b5fe0<M: GuestMemory>(m: &M, cpu: &mut Cpu) {
    cpu.eax = stack_arg(m, cpu, 0);
    cpu.eax = m.read32(cpu.eax.wrapping_add(0x28));
    cpu.ecx = low_byte(cpu.ecx, stack_arg(m, cpu, 2));
    cpu.eax = (cpu.eax & 0x00ff_ffff) | 0x8000_0000;
    if cpu.ecx & 0xff != 0 {
        cpu.eax |= 0x3000_0000;
    }
}

/// 003B3120 stdcall(2), ECX = table. Scans the 0xAC byte records between table+0x60 and
/// table+0x310 for a nonzero [rec+8] with [rec+0x24] == a and [rec+0x20] == b. ESI/EDI are
/// pushed below ESP and restored.
pub fn find_record_by_key_pair<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    cpu.eax = cpu.ecx.wrapping_add(0x60);
    cpu.ecx = cpu.ecx.wrapping_add(0x310);
    let entry = cpu.esp;
    m.write32(entry.wrapping_sub(4), cpu.esi);
    m.write32(entry.wrapping_sub(8), cpu.edi);
    let mut found = false;
    if cpu.eax < cpu.ecx {
        cpu.edx = m.read32(entry.wrapping_add(8));
        cpu.esi = m.read32(entry.wrapping_add(4));
        loop {
            cpu.edi = m.read32(cpu.eax.wrapping_add(8));
            if cpu.edi != 0
                && m.read32(cpu.eax.wrapping_add(0x24)) == cpu.esi
                && m.read32(cpu.eax.wrapping_add(0x20)) == cpu.edx
            {
                found = true;
                break;
            }
            cpu.eax = cpu.eax.wrapping_add(0xac);
            if cpu.eax >= cpu.ecx {
                break;
            }
        }
    }
    if !found {
        cpu.eax = 0;
    }
    cpu.edi = m.read32(entry.wrapping_sub(8));
    cpu.esi = m.read32(entry.wrapping_sub(4));
}

fn string_length<M: GuestMemory>(m: &M, text: u32) -> u32 {
    let mut cursor = text;
    loop {
        let byte = m.read8(cursor);
        cursor = cursor.wrapping_add(1);
        if byte == 0 {
            break;
        }
    }
    cursor.wrapping_sub(text).wrapping_sub(1)
}

/// 00033DB0 stdcall(object, a, b, text). Header store (magic 'GREQ'), bounded copy of `text`
/// through 003C8960 (cdecl, 3 arguments, 8 bytes below), then strlen(text)+1 at +0xC and in EAX.
pub fn header_store_magic_and_copy_text<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let entry = cpu.esp;
    let first = stack_arg(m, cpu, 1);
    let second = stack_arg(m, cpu, 2);
    let saved_esi = cpu.esi;
    let saved_edi = cpu.edi;
    m.write32(entry.wrapping_sub(4), saved_esi);
    let object = stack_arg(m, cpu, 0);
    m.write32(entry.wrapping_sub(8), saved_edi);
    let text = stack_arg(m, cpu, 3);
    m.write32(object.wrapping_add(0x10), first);
    m.write32(object, 0x4752_4551);
    m.write32(object.wrapping_add(4), second);
    let length = string_length(m, text);
    let destination = m.read32(object.wrapping_add(8));
    cpu.esi = object;
    cpu.edi = text;
    cpu.eax = length;
    cpu.ecx = second & 0xffff_ff00;
    cpu.edx = destination;
    guest_call(c, m, cpu, 0x003c_8960, 0x0003_3de7, 8, &[destination, text, length]);
    let stored = string_length(m, text).wrapping_add(1);
    cpu.ecx &= 0xffff_ff00;
    cpu.edx = text.wrapping_add(1);
    m.write32(object.wrapping_add(0xc), stored);
    cpu.eax = stored;
    cpu.edi = m.read32(entry.wrapping_sub(8));
    cpu.esi = m.read32(entry.wrapping_sub(4));
}

/// 0035E630 cdecl(0): total slot count when 0035E960 reports live, else 16.
pub fn net_total_slot_count<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    leaf(c, m, cpu, 0x0035_e960, 0x0035_e635);
    if cpu.eax != 0 {
        cpu.ecx = m.read32(0x0076_bbc0);
        cpu.eax = m.read32(0x0076_bbc4);
        cpu.eax = cpu.eax.wrapping_add(cpu.ecx);
        cpu.eax = cpu.eax.wrapping_add(m.read32(0x0076_bbbc));
    } else {
        cpu.eax = 16;
    }
}

/// 00356320 cdecl(0): free player slots, 16 minus the 003562E0 count, asked twice.
pub fn net_count_free_player_slots<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    leaf(c, m, cpu, 0x0035_62e0, 0x0035_6325);
    cpu.ecx = 16u32.wrapping_sub(cpu.eax);
    if cpu.ecx != 0 && cpu.ecx & 0x8000_0000 == 0 {
        leaf(c, m, cpu, 0x0035_62e0, 0x0035_6335);
        cpu.ecx = 16u32.wrapping_sub(cpu.eax);
        cpu.eax = cpu.ecx;
    } else {
        cpu.eax = 0;
    }
}

/// 003568C0 cdecl(slot): 1 when 003533F0 reports offline, else whether the slot is connected.
// portreg: va=0x003568c0 tier=1
pub fn net_is_player_slot_connected<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    leaf(c, m, cpu, 0x0035_33f0, 0x0035_68c5);
    if cpu.eax == 0 {
        cpu.eax = 1;
        return;
    }
    cpu.eax = stack_arg(m, cpu, 0);
    if (cpu.eax as i32) >= 16 {
        cpu.eax = 0;
        return;
    }
    cpu.ecx = cpu.eax.wrapping_mul(0x124);
    if m.read8(cpu.ecx.wrapping_add(0x0077_728c)) & 1 == 0 {
        cpu.eax = 0;
        return;
    }
    if m.read32(0x0079_094c) == 0x65 {
        cpu.eax = 1;
        return;
    }
    cpu.ecx = m.read32(0x0073_56d8);
    if cpu.ecx == 0 {
        cpu.eax = 0;
        return;
    }
    cpu.eax = cpu.eax.wrapping_mul(0xea0).wrapping_add(cpu.ecx);
    if cpu.eax == 0 || m.read32(cpu.eax) == 0 {
        cpu.eax = 0;
        return;
    }
    cpu.eax = m.read32(0x007d_e30c);
    cpu.eax = 1;
}

/// 0035E6A0 cdecl(3): store the slot counts, and the pending count when the role test passes.
pub fn net_set_slot_counts<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    cpu.eax = stack_arg(m, cpu, 0);
    cpu.ecx = stack_arg(m, cpu, 1);
    m.write32(0x0076_bbbc, cpu.eax);
    m.write32(0x0076_bbc4, cpu.ecx);
    leaf(c, m, cpu, 0x0035_5f10, 0x0035_e6b8);
    if cpu.eax == 0 {
        return;
    }
    cpu.eax = stack_arg(m, cpu, 2);
    if (cpu.eax as i32) > 0 {
        m.write32(0x0076_bbc0, cpu.eax);
    }
}

/// 0035EAF0 cdecl(0): advance the session key counter once when 0035E960 reports live.
pub fn net_advance_session_key<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    leaf(c, m, cpu, 0x0035_e960, 0x0035_eaf5);
    if cpu.eax == 0 {
        return;
    }
    cpu.eax = m.read32(0x0076_bd6c);
    if cpu.eax != 0 {
        return;
    }
    cpu.eax = u32::from(m.read8(0x0076_bd68));
    cpu.ecx = 0xfe;
    cpu.edx = cpu.eax % 0xfe;
    cpu.eax /= 0xfe;
    m.write32(0x0076_bd6c, 1);
    cpu.edx = low_byte(cpu.edx, cpu.edx.wrapping_add(1));
    m.write8(0x0076_bd68, cpu.edx as u8);
}

/// 00356990 cdecl(0): role 2 in state 0xA.
pub fn net_is_role_2_in_state_0xa<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    if m.read32(0x0079_094c) == 0x65 {
        cpu.ecx = m.read32(0x0076_b140);
        if cpu.ecx != 2 {
            cpu.eax = 0;
            return;
        }
    } else {
        leaf(c, m, cpu, 0x0035_33f0, 0x0035_69bf);
        if cpu.eax == 0 || m.read32(0x0076_b140) != 2 {
            cpu.eax = 0;
            return;
        }
    }
    cpu.eax = u32::from(m.read32(0x0076_b1a4) == 0x0a);
}

/// 00358130 cdecl(0): take the pickup flag when a session client.
// portreg: va=0x00358130 tier=1
pub fn net_take_pickup_flag<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    leaf(c, m, cpu, 0x0035_33f0, 0x0035_8135);
    if cpu.eax == 0 {
        return;
    }
    leaf(c, m, cpu, 0x0035_5ec0, 0x0035_813e);
    if cpu.eax != 0 {
        cpu.eax = 0;
        return;
    }
    cpu.eax = m.read32(0x0054_1e9c);
    m.write32(0x0054_1e9c, 1);
}

/// 00358160 cdecl(0): next pickup sequence id by mode, or the callee leftover when offline.
// portreg: va=0x00358160 tier=1
pub fn net_next_pickup_sequence_id<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    leaf(c, m, cpu, 0x0035_33f0, 0x0035_8165);
    if cpu.eax == 0 {
        return;
    }
    leaf(c, m, cpu, 0x0035_5ec0, 0x0035_816e);
    if cpu.eax == 0 {
        cpu.eax = m.read32(0x007d_e30c);
        if cpu.eax == 7 || cpu.eax == 6 {
            cpu.eax = m.read32(0x0054_1ea4);
            m.write32(0x0054_1ea4, 0xffff_ffff);
            return;
        }
    }
    cpu.ecx = m.read32(0x0051_9750);
    cpu.eax = cpu.ecx;
    cpu.ecx = cpu.ecx.wrapping_add(1);
    m.write32(0x0051_9750, cpu.ecx);
}

/// 00356080 cdecl(key): index of the session player whose record +0x3C equals key, -1 absent.
pub fn net_player_index_in_session<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    leaf(c, m, cpu, 0x0035_33f0, 0x0035_6085);
    if cpu.eax == 0 {
        cpu.eax = stack_arg(m, cpu, 0);
        return;
    }
    cpu.ecx = m.read32(0x0077_4028);
    cpu.edx = m.read32(cpu.ecx.wrapping_add(0x20));
    m.write32(cpu.esp.wrapping_sub(4), cpu.esi);
    m.write32(cpu.esp.wrapping_sub(8), cpu.edi);
    cpu.eax = 0;
    if (cpu.edx as i32) > 0 {
        let key = stack_arg(m, cpu, 0);
        cpu.ecx = cpu.ecx.wrapping_add(0x0c);
        loop {
            let record = m.read32(cpu.ecx);
            if m.read32(record.wrapping_add(0x3c)) == key {
                return;
            }
            cpu.eax = cpu.eax.wrapping_add(1);
            cpu.ecx = cpu.ecx.wrapping_add(4);
            if (cpu.eax as i32) >= (cpu.edx as i32) {
                break;
            }
        }
    }
    cpu.eax = 0xffff_ffff;
}

/// 00356870 cdecl(slot): player slot field +0x3C by slot, or the argument when offline.
// portreg: va=0x00356870 tier=0
pub fn net_player_slot_field_3c<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    leaf(c, m, cpu, 0x0035_33f0, 0x0035_6875);
    if cpu.eax != 0 {
        cpu.eax = m.read32(0x0077_4028);
        if cpu.eax == 0 {
            cpu.eax = stack_arg(m, cpu, 0);
            return;
        }
    }
    m.write32(cpu.esp.wrapping_sub(4), cpu.esi);
    leaf_below(c, m, cpu, 0x0035_33f0, 0x0035_688d, 4);
    let slot = stack_arg(m, cpu, 0);
    if cpu.eax != 0 {
        cpu.eax = m.read32(0x0077_4028);
        cpu.ecx = m.read32(cpu.eax.wrapping_add(slot.wrapping_mul(4)).wrapping_add(0x0c));
        if cpu.ecx == 0 {
            cpu.eax = slot;
            return;
        }
    }
    leaf_below(c, m, cpu, 0x0035_33f0, 0x0035_68a7, 4);
    if cpu.eax == 0 {
        cpu.eax = slot;
        return;
    }
    cpu.ecx = m.read32(0x0077_4028);
    cpu.edx = m.read32(cpu.ecx.wrapping_add(slot.wrapping_mul(4)).wrapping_add(0x0c));
    cpu.eax = m.read32(cpu.edx.wrapping_add(0x3c));
}

/// 00358560 cdecl(player): whether the object replicates locally.
pub fn net_object_replicates_locally<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    leaf(c, m, cpu, 0x0035_33f0, 0x0035_8565);
    if cpu.eax == 0 {
        cpu.eax = 1;
        return;
    }
    cpu.eax = stack_arg(m, cpu, 0);
    cpu.ecx = m.read32(cpu.eax.wrapping_add(0x7c));
    cpu.edx = m.read32(cpu.ecx.wrapping_add(8));
    cpu.edx = cpu.edx.wrapping_sub(m.read32(0x0079_0950));
    if cpu.edx & 0x8000_0000 != 0 {
        cpu.eax = 0;
        return;
    }
    leaf(c, m, cpu, 0x0035_5ec0, 0x0035_8583);
    cpu.eax = u32::from(cpu.eax != 0);
}

/// 003560E0 cdecl(index, word): initialise record 0x124 at `index`, after 000389B0(-1).
pub fn net_record_124_init_at_index<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    one(c, m, cpu, 0x0003_89b0, 0x0035_60e7, 0, 0xffff_ffff);
    cpu.ecx = stack_arg(m, cpu, 0);
    cpu.edx = stack_arg(m, cpu, 1);
    cpu.eax = cpu.ecx.wrapping_mul(0x124);
    m.write32(cpu.eax.wrapping_add(0x0077_727c), cpu.ecx);
    m.write32(cpu.eax.wrapping_add(0x0077_7284), cpu.ecx);
    cpu.ecx = m.read32(cpu.eax.wrapping_add(0x0077_728c));
    cpu.ecx |= 1;
    m.write32(cpu.eax.wrapping_add(0x0077_7244), cpu.edx);
    m.write32(cpu.eax.wrapping_add(0x0077_728c), cpu.ecx);
    m.write32(cpu.eax.wrapping_add(0x0077_7360), 0);
    m.write32(cpu.eax.wrapping_add(0x0077_7348), 0xffff_ffff);
}

/// 00359910 cdecl(pointer): minimum signed word at record + 0xA60 over the connected slots
/// whose entry key differs from *pointer (0x7FFFFFFF when none). Saves EBX/EBP/ESI/EDI.
pub fn net_min_word_over_connected_slots<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let entry_esp = cpu.esp;
    let mut slot = 0u32;
    let mut minimum = 0x7fff_ffffu32;
    let mut record_offset = 0u32;
    let mut entry = 0x0077_7244u32;
    m.write32(entry_esp.wrapping_sub(4), cpu.ebx);
    m.write32(entry_esp.wrapping_sub(8), cpu.ebp);
    m.write32(entry_esp.wrapping_sub(12), cpu.esi);
    m.write32(entry_esp.wrapping_sub(16), cpu.edi);
    loop {
        one(c, m, cpu, 0x0035_68c0, 0x0035_9928, 16, slot);
        if cpu.eax != 0 {
            cpu.eax = m.read32(entry);
            cpu.ecx = m.read32(cpu.eax);
            cpu.edx = stack_arg(m, cpu, 0);
            if cpu.ecx != m.read32(cpu.edx) {
                cpu.eax = m.read32(0x0073_56d8);
                let address = record_offset.wrapping_add(cpu.eax).wrapping_add(0xa60);
                cpu.eax = i32::from(m.read16(address) as i16) as u32;
                if (cpu.eax as i32) < (minimum as i32) {
                    minimum = cpu.eax;
                }
            }
        }
        entry = entry.wrapping_add(0x124);
        slot = slot.wrapping_add(1);
        record_offset = record_offset.wrapping_add(0xea0);
        if (entry as i32) >= 0x0077_8484 {
            break;
        }
    }
    cpu.eax = minimum;
}

/// 0015C4E0 cdecl(0): release each active slot record that lacks flag 0x2 by calling 0015C3D0
/// with its first dword. Saves EBP/EDI/ESI below ESP, the table is reloaded after every call.
// portreg: va=0x0015c4e0 tier=0
pub fn net_slot_release_each_active<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let entry = cpu.esp;
    m.write32(entry.wrapping_sub(4), cpu.ebp);
    m.write32(entry.wrapping_sub(8), cpu.edi);
    let saved_ebp = cpu.ebp;
    let saved_edi = cpu.edi;
    cpu.eax = m.read32(0x007a_2958);
    cpu.edi = m.read32(0x0079_0950);
    cpu.eax = cpu.eax.wrapping_add(cpu.edi);
    cpu.ebp = 0;
    if (cpu.eax as i32) > 0 {
        cpu.eax = m.read32(0x0073_56d8);
        m.write32(entry.wrapping_sub(12), cpu.esi);
        let saved_esi = cpu.esi;
        cpu.esi = 0;
        loop {
            let record = cpu.esi.wrapping_add(cpu.eax);
            if m.read8(record.wrapping_add(0x2c)) & 2 == 0 {
                if m.read8(0x007d_e455) == 0x0a {
                    cpu.ecx = m.read32(record.wrapping_add(0x10));
                    cpu.edx = m.read32(record.wrapping_add(0x14));
                } else {
                    cpu.ecx = m.read32(record.wrapping_add(0x18));
                    cpu.edx = m.read32(record.wrapping_add(0x1c));
                }
                cpu.ecx |= cpu.edx;
                if cpu.ecx != 0 || (m.read32(record.wrapping_add(8)) as i32) < (cpu.edi as i32) {
                    cpu.ecx = m.read32(record);
                    one(c, m, cpu, 0x0015_c3d0, 0x0015_c535, 12, cpu.ecx);
                    cpu.edi = m.read32(0x0079_0950);
                    cpu.eax = m.read32(0x0073_56d8);
                }
            }
            cpu.edx = m.read32(0x007a_2958);
            cpu.ebp = cpu.ebp.wrapping_add(1);
            cpu.edx = cpu.edx.wrapping_add(cpu.edi);
            cpu.esi = cpu.esi.wrapping_add(0xea0);
            if (cpu.ebp as i32) >= (cpu.edx as i32) {
                break;
            }
        }
        cpu.esi = saved_esi;
    }
    cpu.edi = saved_edi;
    cpu.ebp = saved_ebp;
}

/// 00235600 cdecl(player): set flag 0x20000000 on the object entry when field +0x14 masks 0x1201.
pub fn net_slot_object_set_flag<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    cpu.eax = stack_arg(m, cpu, 0);
    one(c, m, cpu, 0x0015_ba10, 0x0023_560a, 0, cpu.eax);
    if cpu.eax == 0 {
        return;
    }
    cpu.edx = m.read32(cpu.eax.wrapping_add(0x7c));
    cpu.ecx = m.read32(cpu.edx.wrapping_add(0x14)) & 0x1201;
    cpu.eax = cpu.ecx;
    if cpu.eax != 0 {
        let flags = m.read32(cpu.edx.wrapping_add(0x28));
        m.write32(cpu.edx.wrapping_add(0x28), flags | 0x2000_0000);
    }
}
