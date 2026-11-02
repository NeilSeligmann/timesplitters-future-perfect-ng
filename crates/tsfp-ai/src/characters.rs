// SPDX-License-Identifier: GPL-3.0-or-later
//! Character table/state slice (T1915). Each root is Tier 0; see docs/t-system-characters.md.
//! Guest addresses are handles; memory and cross-system calls stay behind explicit traits.
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu, guest_call, stack_arg},
    memory::GuestMemory,
};

pub const PAIR_TABLE: u32 = 0x74c368;
pub const CHARACTER_COUNT: u32 = 0x74c37c;
pub const CHARACTER_POINTERS: u32 = 0x7a3580;
pub const FIRST_REMOTE: u32 = 0x790950;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterHandle(pub u32);
impl CharacterHandle {
    pub fn extension<M: GuestMemory>(self, m: &M) -> u32 {
        m.read32(self.0.wrapping_add(0x7c))
    }
}

fn type_map<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
    first: u32,
    last: u32,
    bytes: u32,
    targets: u32,
    yes: u32,
    no: u32,
) {
    let index = stack_arg(m, cpu, 0).wrapping_sub(first);
    if index > last - first {
        cpu.eax = 0;
        return;
    }
    cpu.eax = u32::from(m.read8(bytes.wrapping_add(index)));
    let target = m.read32(targets.wrapping_add(cpu.eax.wrapping_mul(4)));
    if target == yes {
        cpu.eax = 1;
    } else if target == no {
        cpu.eax = 0;
    } else {
        c.call(m, target, cpu);
    } // preserve the original indirect tail for unexpected table data
}
// portreg: va=0x000d6c90
/// 000D6C90 cdecl(id): inclusive 0x5d..0x9b byte-map predicate. Tier 0.
pub fn type_flag_5d<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    type_map(c, m, cpu, 0x5d, 0x9b, 0xd6cbc, 0xd6cb4, 0xd6caa, 0xd6cb0);
}
/// 000D6D00 cdecl(id): inclusive 0x16..0xa3 byte-map predicate. Tier 0.
// portreg: va=0x000d6d00
pub fn type_flag_16<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    type_map(c, m, cpu, 0x16, 0xa3, 0xd6d30, 0xd6d28, 0xd6d1c, 0xd6d22);
}
/// 000D6DD0 cdecl(id): inclusive 0x12..0xaa (not 0xab) byte-map predicate. Tier 0.
// portreg: va=0x000d6dd0
pub fn type_flag_12<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    type_map(c, m, cpu, 0x12, 0xaa, 0xd6e00, 0xd6df8, 0xd6dec, 0xd6df2);
}
// portreg: va=0x000d6ea0
/// 000D6EA0 cdecl(id): true for 0x4a/0x50. Tier 0.
pub fn is_id_4a_or_50<M: GuestMemory>(m: &M, cpu: &mut Cpu) {
    cpu.eax = u32::from(matches!(stack_arg(m, cpu, 0), 0x4a | 0x50));
}
// portreg: va=0x000d6ec0
/// 000D6EC0 cdecl(id): true for 0x4e, leaves id in ECX. Tier 0.
pub fn is_id_4e<M: GuestMemory>(m: &M, cpu: &mut Cpu) {
    cpu.ecx = stack_arg(m, cpu, 0);
    cpu.eax = u32::from(cpu.ecx == 0x4e);
}
/// 000D6ED0 cdecl(id): invert D6DD0 or ECX in {4d,4f,64}; observes post-call ECX. Tier 0.
pub fn is_other_type<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    cpu.ecx = stack_arg(m, cpu, 0);
    let id = cpu.ecx;
    guest_call(c, m, cpu, 0xd6dd0, 0xd6eda, 0, &[id]);
    cpu.eax = u32::from(cpu.eax == 0 && !matches!(cpu.ecx, 0x4d | 0x4f | 0x64));
}
// portreg: va=0x0015bd00
/// 0015BD00 cdecl(a,b): symmetric packed triangular table, 12-byte entries. Tier 0.
pub fn pair_entry<M: GuestMemory>(m: &M, cpu: &mut Cpu) {
    let a = stack_arg(m, cpu, 0);
    let b = stack_arg(m, cpu, 1);
    let (hi, lo) = if (a as i32) > (b as i32) {
        (a, b)
    } else {
        (b, a)
    };
    let product = hi.wrapping_sub(1).wrapping_mul(hi);
    let index = ((product as i32) >> 1) as u32;
    cpu.edx = lo;
    cpu.ecx = m.read32(PAIR_TABLE);
    cpu.eax = cpu
        .ecx
        .wrapping_add(index.wrapping_add(lo).wrapping_mul(12));
}
// portreg: va=0x00012fa0
/// 00012FA0 cdecl(a,b): signed byte at pair entry. Tier 0.
pub fn pair_signed_flag<M: GuestMemory>(m: &M, cpu: &mut Cpu) {
    pair_entry(m, cpu);
    cpu.eax = m.read8(cpu.eax) as i8 as i32 as u32;
}
// portreg: va=0x00012fc0
/// 00012FC0 cdecl(a,b): dword at pair entry +4. Tier 0.
pub fn pair_field_4<M: GuestMemory>(m: &M, cpu: &mut Cpu) {
    pair_entry(m, cpu);
    cpu.eax = m.read32(cpu.eax.wrapping_add(4));
}
/// 001C5210 cdecl(group,index): signed index >= prefix span; invalid group returns 0. Tier 0.
pub fn group_index_outside<M: GuestMemory>(m: &M, cpu: &mut Cpu) {
    let group = stack_arg(m, cpu, 0);
    cpu.eax = group;
    if group == 0 {
        cpu.ecx = stack_arg(m, cpu, 1);
        cpu.eax = u32::from((cpu.ecx as i32) >= (m.read32(0x4fc9a0) as i32));
    } else if (1..0x2f).contains(&group) {
        cpu.ecx = m.read32(0x4fc99cu32.wrapping_add(group.wrapping_mul(4)));
        cpu.edx = m
            .read32(0x4fc9a0u32.wrapping_add(group.wrapping_mul(4)))
            .wrapping_sub(cpu.ecx);
        cpu.ecx = stack_arg(m, cpu, 1);
        cpu.eax = u32::from((cpu.ecx as i32) >= (cpu.edx as i32));
    } else {
        cpu.eax = 0;
    }
}
/// 001C5250 cdecl(): group count. Tier 0.
pub fn group_count(cpu: &mut Cpu) {
    cpu.eax = 0x2f;
}
/// 00261C70 cdecl(character,slot): mode-selected bit 0x80000 gates pending slot/state. Tier 0.
pub fn set_pending_slot<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    cpu.ecx = (cpu.ecx & !255) | u32::from(m.read8(0x7de455));
    cpu.eax = CharacterHandle(stack_arg(m, cpu, 0)).extension(m);
    let offset = if cpu.ecx & 255 == 10 { 0x10 } else { 0x18 };
    cpu.ecx = m.read32(cpu.eax.wrapping_add(offset));
    cpu.edx = m.read32(cpu.eax.wrapping_add(offset + 4));
    m.write32(cpu.esp.wrapping_sub(4), cpu.edx);
    cpu.ecx &= 0x80000;
    cpu.edx = 0;
    if cpu.ecx != 0 {
        cpu.ecx = stack_arg(m, cpu, 1);
        cpu.edx = m.read32(cpu.eax.wrapping_add(0x20));
        m.write32(cpu.eax.wrapping_add(0x9ec), cpu.ecx);
        cpu.ecx = 1;
        if cpu.edx != 1 {
            m.write32(cpu.eax.wrapping_add(0x9d0), 1);
        }
    }
}
/// 0027CE70 cdecl(character,message): 1003 sets extension flag and clears active bit; returns -2. Tier 0.
pub fn deactivate_message<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    cpu.ecx = stack_arg(m, cpu, 1);
    cpu.eax = 0;
    if cpu.ecx == 0x3eb {
        cpu.eax = stack_arg(m, cpu, 0);
        cpu.ecx = CharacterHandle(cpu.eax).extension(m);
        let flags = cpu.ecx.wrapping_add(0x2c);
        m.write32(flags, m.read32(flags) | 0x100000);
        let active = cpu.eax.wrapping_add(0x28);
        m.write32(active, m.read32(active) & !1);
        cpu.eax = u32::MAX - 1;
    }
}
/// 002B9E50 cdecl(): first remote character with object+0x10 bit 0x200000, or null. Tier 0.
pub fn first_flagged_remote<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    m.write32(cpu.esp.wrapping_sub(4), cpu.esi);
    m.write32(cpu.esp.wrapping_sub(8), cpu.edi);
    cpu.esi = m.read32(FIRST_REMOTE);
    cpu.edi = m.read32(CHARACTER_COUNT);
    while (cpu.esi as i32) < (cpu.edi as i32) {
        cpu.eax = m.read32(CHARACTER_POINTERS.wrapping_add(cpu.esi.wrapping_mul(4)));
        cpu.ecx = m.read32(cpu.eax.wrapping_add(0x10)) & 0x200000;
        cpu.edx = 0;
        if cpu.ecx != 0 {
            break;
        }
        cpu.esi = cpu.esi.wrapping_add(1);
    }
    if (cpu.esi as i32) >= (cpu.edi as i32) {
        cpu.eax = 0;
    }
    cpu.edi = m.read32(cpu.esp.wrapping_sub(8));
    cpu.esi = m.read32(cpu.esp.wrapping_sub(4));
}
// portreg: va=0x001b8c40
/// 001B8C40 cdecl(): invokes 2B8F90(character's first dword) for remote slots, reloading count. Tier 0.
pub fn update_remote_characters<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    m.write32(cpu.esp.wrapping_sub(4), cpu.esi);
    cpu.eax = m.read32(CHARACTER_COUNT);
    cpu.esi = m.read32(FIRST_REMOTE);
    while (cpu.esi as i32) < (cpu.eax as i32) {
        cpu.eax = m.read32(CHARACTER_POINTERS.wrapping_add(cpu.esi.wrapping_mul(4)));
        cpu.ecx = m.read32(cpu.eax);
        let value = cpu.ecx;
        guest_call(c, m, cpu, 0x2b8f90, 0x1b8c5f, 4, &[value]);
        cpu.eax = m.read32(CHARACTER_COUNT);
        cpu.esi = cpu.esi.wrapping_add(1);
    }
    cpu.esi = m.read32(cpu.esp.wrapping_sub(4));
}

/// Original-address registry. The tier is visible per root; no runtime registration is implied.
pub const VERIFICATION_TIERS: &[(u32, u8)] = &[
    (0xd6c90, 0),
    (0xd6d00, 0),
    (0xd6dd0, 0),
    (0xd6ea0, 0),
    (0xd6ec0, 0),
    (0xd6ed0, 0),
    (0x15bd00, 0),
    (0x12fa0, 0),
    (0x12fc0, 0),
    (0x1c5210, 0),
    (0x1c5250, 0),
    (0x261c70, 0),
    (0x27ce70, 0),
    (0x2b9e50, 0),
    (0x1b8c40, 0),
];
/// Per-root A/B boundary: return false to let the bridge execute the original.
/// Callers select addresses independently; guest ESP stays at replacement entry as in existing ports.
pub fn dispatch<M: GuestMemory, C: Callee<M>>(
    enabled: bool,
    address: u32,
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) -> bool {
    if !enabled {
        return false;
    }
    match address {
        0xd6c90 => type_flag_5d(c, m, cpu),
        0xd6d00 => type_flag_16(c, m, cpu),
        0xd6dd0 => type_flag_12(c, m, cpu),
        0xd6ea0 => is_id_4a_or_50(m, cpu),
        0xd6ec0 => is_id_4e(m, cpu),
        0xd6ed0 => is_other_type(c, m, cpu),
        0x15bd00 => pair_entry(m, cpu),
        0x12fa0 => pair_signed_flag(m, cpu),
        0x12fc0 => pair_field_4(m, cpu),
        0x1c5210 => group_index_outside(m, cpu),
        0x1c5250 => group_count(cpu),
        0x261c70 => set_pending_slot(m, cpu),
        0x27ce70 => deactivate_message(m, cpu),
        0x2b9e50 => first_flagged_remote(m, cpu),
        0x1b8c40 => update_remote_characters(c, m, cpu),
        _ => return false,
    }
    true
}
