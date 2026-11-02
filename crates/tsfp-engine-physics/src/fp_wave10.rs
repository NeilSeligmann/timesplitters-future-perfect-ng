// SPDX-License-Identifier: GPL-3.0-or-later
//! T1835 wave 10: straight-line, loop and stack-op roots of the wave 2 inventory that had no Rust
//! port (docs/t1835-rust-wave10.md). Each is a one-to-one translation of a proven C replacement of
//! `src/game`: operation order, aliasing stores, untouched registers and the pushes/pops of the
//! callee-saved registers are preserved ([`Cpu`] keeps the saved slots in guest memory). Equality
//! with the C is MEASURED by the `wave10` differential (120,000 cases per root at O0/O2/O3),
//! equality with the original is INFERRED (the C bodies are proven against the original XBE).
use crate::{fp_stack::{cdecl_arg as arg, Cpu}, memory::GuestMemory};

/// Short guest accessors: `m.r(base, off)` reads the dword at `base + off`.
trait Mem: GuestMemory {
    fn r(&self, base: u32, off: u32) -> u32 {
        self.read32(base.wrapping_add(off))
    }
    fn w(&mut self, base: u32, off: u32, value: u32) {
        self.write32(base.wrapping_add(off), value);
    }
    fn b(&self, base: u32, off: u32) -> u8 {
        self.read8(base.wrapping_add(off))
    }
    fn w16(&mut self, address: u32, value: u16) {
        let [low, high] = value.to_le_bytes();
        self.write8(address, low);
        self.write8(address.wrapping_add(1), high);
    }
}
impl<T: GuestMemory + ?Sized> Mem for T {}

/// 00012EC0 cdecl(1). Bit 0x02000000 of [arg+28] when the global gates (7DE458 bit 40, 79094C, 7DE455) allow.
// portreg: va=0x00012ec0 tier=1
pub fn gated_object_flag_02000000<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    if m.read8(0x007d_e458) & 0x40 != 0 {
        c.eax = 0;
        return;
    }
    c.eax = m.read32(0x0079_094c);
    if c.eax == 100 || c.eax == 56 {
        c.eax = 0;
        return;
    }
    let mode = m.read8(0x007d_e455);
    if mode != 10 && c.eax != 102 {
        c.eax = 0;
        return;
    }
    c.eax = arg(m, c, 0);
    c.eax = m.r(c.eax, 0x28);
    c.eax &= 0x0200_0000;
}

/// 00070D10 cdecl(2). Dword `index` of the array at +28 of the block [object+90].
pub fn block_array_dword<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let (object, index) = (arg(m, c, 0), arg(m, c, 1));
    let block = m.r(object, 0x90);
    c.eax = m.r(block.wrapping_add(index.wrapping_mul(4)), 0x28);
}

/// 000718E0 cdecl(2). Follows the link at +10 of the chain starting at [object+D8], `steps` times.
pub fn follow_chain<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let (object, steps) = (arg(m, c, 0), arg(m, c, 1));
    if object == 0 {
        c.eax = 0;
        return;
    }
    let mut link = m.r(object, 0xd8);
    let mut remaining = steps as i32;
    while remaining > 0 {
        if link != 0 {
            link = m.r(link, 0x10);
        }
        remaining -= 1;
    }
    c.eax = link;
}

/// 00071970 cdecl(3). The n-th (arg 2) node of the list at [object+178] whose +14 equals arg 1.
pub fn nth_matching_node<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = arg(m, c, 0);
    c.eax = m.r(c.eax, 0x178);
    if c.eax != 0 {
        c.edx = arg(m, c, 1);
        c.ecx = arg(m, c, 2);
        loop {
            if m.r(c.eax, 0x14) == c.edx {
                if c.ecx == 0 {
                    return;
                }
                c.ecx -= 1;
            }
            c.eax = m.r(c.eax, 4);
            if c.eax == 0 {
                break;
            }
        }
    }
    c.eax = 0;
}

/// 00071CD0 cdecl(2). Clears bit 0 of [object+C], sets it again when `enable` is non-zero.
pub fn set_flag_bit<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let (object, enable) = (arg(m, c, 0), arg(m, c, 1));
    let flags = m.r(object, 0xc) & 0xffff_fffe;
    m.w(object, 0xc, flags);
    if enable != 0 {
        m.w(object, 0xc, flags | 1);
    }
    c.eax = object;
}

/// 00074670 cdecl(2). First node of the list at [object+16C] (next at +30) whose +4 equals the key.
pub fn find_node_by_key<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let (object, key) = (arg(m, c, 0), arg(m, c, 1));
    let mut node = m.r(object, 0x16c);
    while node != 0 {
        if m.r(node, 4) == key {
            c.eax = node;
            return;
        }
        node = m.r(node, 0x30);
    }
    c.eax = 0;
}

/// 00074840 cdecl(1). Stores -1 into the four slot ids at object+128 (stride 10), returns the object.
// portreg: va=0x00074840 tier=0
pub fn clear_slot_ids<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let object = arg(m, c, 0);
    for slot in 0..4u32 {
        m.w(object, 0x128 + slot * 0x10, 0xffff_ffff);
    }
    c.eax = object;
}

/// 00074950 cdecl(3). Stores `value` at +4 of the first of four 10-byte slots (object+128) whose id is the key.
pub fn set_slot_value<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let (object, key, value) = (arg(m, c, 0), arg(m, c, 1), arg(m, c, 2));
    for index in 0..4u32 {
        let slot = object.wrapping_add(0x128).wrapping_add(index * 0x10);
        if m.read32(slot) == key {
            m.w(slot, 4, value);
            c.eax = slot;
            return;
        }
    }
    c.eax = 4;
}

/// 00074AB0 cdecl(3). Records child index (+C8) and id (+CC) when node (+14 == arg 2) has the child with id arg 3.
pub fn matching_node_child_index<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = arg(m, c, 0);
    c.eax = m.r(c.eax, 0x178);
    if c.eax != 0 {
        c.ecx = arg(m, c, 1);
        while m.r(c.eax, 0x14) != c.ecx {
            c.eax = m.r(c.eax, 4);
            if c.eax == 0 {
                break;
            }
        }
    }
    c.edx = c.eax;
    c.eax = m.r(c.edx, 0xd8);
    c.ecx = 0;
    if c.eax == 0 {
        return;
    }
    c.push(m, c.esi);
    c.esi = c.at(m, 16);
    while m.r(c.eax, 8) != c.esi {
        c.eax = m.r(c.eax, 16);
        c.ecx = c.ecx.wrapping_add(1);
        if c.eax == 0 {
            break;
        }
    }
    if c.eax != 0 {
        m.w(c.edx, 0xcc, c.esi);
        m.w(c.edx, 0xc8, c.ecx);
    }
    c.esi = c.pop(m);
}

/// 00074B80 cdecl(1). ORs the flags at +48 into +40 of every node of the list at [object+16C], then of the object.
// portreg: va=0x00074b80 tier=0
pub fn propagate_flags<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let object = arg(m, c, 0);
    let mut node = m.r(object, 0x16c);
    while node != 0 {
        let value = m.r(node, 0x40) | m.r(node, 0x48);
        m.w(node, 0x40, value);
        node = m.r(node, 0x30);
    }
    let extra = m.r(object, 0x48);
    let value = m.r(object, 0x40) | extra;
    m.w(object, 0x40, value);
    c.eax = extra;
}

/// 00087A40 cdecl(2). In the 0x28-byte table at [708778]: sets bit (arg 2 & 31) of the word at +20, clears it in +22.
pub fn table_708778_set_bit_clear_bit<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = arg(m, c, 0);
    c.ecx = m.read32(0x0070_8778);
    c.eax = c.eax.wrapping_mul(5);
    c.edx = c.ecx.wrapping_add(c.eax.wrapping_mul(8));
    c.ecx = arg(m, c, 1);
    c.eax = 1u32 << (c.ecx & 31);
    let low = m.read16(c.edx.wrapping_add(0x20)) | c.eax as u16;
    m.w16(c.edx.wrapping_add(0x20), low);
    c.eax = !c.eax;
    let high = m.read16(c.edx.wrapping_add(0x22)) & c.eax as u16;
    m.w16(c.edx.wrapping_add(0x22), high);
}

/// 00094290 cdecl(2). Stores the low word of `value` at object+7C, sets bit 2 of [[object]+18], returns the header.
// portreg: va=0x00094290 tier=0
pub fn set_word_7c_and_header_flag<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let (object, value) = (arg(m, c, 0), arg(m, c, 1));
    m.write8(object.wrapping_add(0x7c), value as u8);
    m.write8(object.wrapping_add(0x7d), (value >> 8) as u8);
    let header = m.read32(object);
    let flags = m.r(header, 0x18) | 4;
    m.w(header, 0x18, flags);
    c.eax = header;
}

/// 000942D0 cdecl(2). Clears the bits of arg 2 in the word at [arg 1]+1F6.
pub fn flags_clear_word<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.ecx = arg(m, c, 1);
    c.eax = arg(m, c, 0);
    c.ecx = !c.ecx;
    let address = c.eax.wrapping_add(0x1f6);
    let value = m.read16(address) & c.ecx as u16;
    m.w16(address, value);
}

/// 0009E730 cdecl(1). Dword at [791D4C] + 48 * (signed word at 78B860 + 2 * slot).
// portreg: va=0x0009e730 tier=0
pub fn mapped_entry_value<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let slot = arg(m, c, 0);
    let mapped = i32::from(m.read16(slot.wrapping_mul(2).wrapping_add(0x0078_b860)) as i16) as u32;
    let base = m.read32(0x0079_1d4c);
    c.eax = m.read32(mapped.wrapping_mul(48).wrapping_add(base));
}

/// 000A2C60 cdecl(2). Next slot (stride 2E8, 7B4FAC slots at [72F5E8]) after `previous` whose +20 is the type, cyclic.
pub fn slot_find_next_by_type<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let (previous, kind) = (arg(m, c, 0), arg(m, c, 1));
    let slot_count = m.read32(0x007b_4fac);
    let first_slot = m.read32(0x0072_f5e8);
    let last_slot = first_slot.wrapping_add(slot_count.wrapping_mul(0x2e8)).wrapping_sub(0x2e8);
    let mut wrap_count = 0u32;
    let mut slot = if previous != 0 { previous } else { first_slot };
    slot = slot.wrapping_add(0x2e8);
    if slot > last_slot {
        slot = first_slot;
        wrap_count = 1;
    }
    loop {
        if slot == previous {
            c.eax = 0;
            return;
        }
        let slot_type = m.r(slot, 0x20);
        if slot_type == kind || (kind == 0xffff_ffff && slot_type != 2) {
            c.eax = slot;
            return;
        }
        slot = slot.wrapping_add(0x2e8);
        if slot > last_slot {
            slot = first_slot;
            wrap_count = wrap_count.wrapping_add(1);
        }
        if (wrap_count as i32) >= 4 {
            c.eax = 0;
            return;
        }
    }
}

/// 000A4C70 cdecl(1). 1 when (7B4FAC - 7B65B8) + arg + 4 is below the signed value at 72F5E4.
pub fn wrapping_capacity_signed_test<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.edx = m.read32(0x007b_65b8);
    c.eax = m.read32(0x007b_4fac);
    c.ecx = arg(m, c, 0);
    c.eax = c.eax.wrapping_sub(c.edx);
    c.edx = c.eax.wrapping_add(c.ecx).wrapping_add(4);
    c.ecx = m.read32(0x0072_f5e4);
    c.eax = u32::from((c.edx as i32) < (c.ecx as i32));
}

/// 000B0590 cdecl(0). Zeroes 0x9A8 dwords at [732A04] (`rep stosd`, honours DF) and the dword at 732A08 first.
// portreg: va=0x000b0590 tier=1
pub fn table_732a04_clear<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.push(m, c.edi);
    c.edi = m.read32(0x0073_2a04);
    c.ecx = 0x9a8;
    c.eax = 0;
    m.write32(0x0073_2a08, 0);
    c.rep_stosd(m);
    c.edi = c.pop(m);
}

/// 000B0770 cdecl(1). 1 when [object+90] is the current reference at 7DE318 or that minus [4E7944].
pub fn current_or_previous_reference<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x007d_e318);
    if c.eax != 0 {
        c.ecx = arg(m, c, 0);
        c.ecx = m.r(c.ecx, 0x90);
        if c.ecx != 0 {
            if c.ecx == c.eax {
                c.eax = 1;
                return;
            }
            c.eax = c.eax.wrapping_sub(m.read32(0x004e_7944));
            if c.ecx == c.eax {
                c.eax = 1;
                return;
            }
        }
    }
    c.eax = 0;
}

/// 000B0F90 cdecl(1). 1 when the extension [[object]+7C]+10 is 0, 0 for a null object (arg returned in EAX).
pub fn nullable_extension_state0<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = arg(m, c, 0);
    if c.eax != 0 {
        c.eax = m.r(c.eax, 0x7c);
        c.edx = m.r(c.eax, 0x10);
        c.ecx = u32::from(c.edx == 0);
        c.eax = c.ecx;
    }
}

/// 000B2D40 cdecl(1). Zeroes every entry of the table at 7B2E20 (count [732A10]) equal to the value, returns the count.
pub fn clear_matching_slots<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let value = arg(m, c, 0);
    let count = m.read32(0x0073_2a10) as i32;
    let mut index = 0i32;
    while index < count {
        let slot = 0x007b_2e20u32.wrapping_add((index as u32).wrapping_mul(4));
        if m.read32(slot) == value {
            m.write32(slot, 0);
        }
        index += 1;
    }
    c.eax = index as u32;
}

/// 000B3CA0 cdecl(1). Record 4D98B8 + B4 * [[object]+7C]+C when the extension has flag 0x40 and the index is below 25.
// portreg: va=0x000b3ca0 tier=1
pub fn optional_record_lookup<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = arg(m, c, 0);
    c.eax = m.r(c.eax, 0x7c);
    if c.eax != 0 && (m.b(c.eax, 8) & 0x40) != 0 {
        c.eax = m.r(c.eax, 12);
        if (c.eax as i32) >= 0 && (c.eax as i32) < 25 {
            c.eax = c.eax.wrapping_mul(0xb4).wrapping_add(0x004d_98b8);
            return;
        }
    }
    c.eax = 0;
}

/// 000C2E30 cdecl(0). Zeroes 0x528 dwords at [732E70] (`rep stosd`, honours DF) and the dword at 732E74 first.
// portreg: va=0x000c2e30 tier=1
pub fn table_732e70_clear<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.push(m, c.edi);
    c.edi = m.read32(0x0073_2e70);
    c.ecx = 0x528;
    c.eax = 0;
    m.write32(0x0073_2e74, 0);
    c.rep_stosd(m);
    c.edi = c.pop(m);
}

/// 000C7360 cdecl(0), ECX = object. 0 when the pair at +26C/+270 has neither type 0x13 and the flag bits do not conflict.
// portreg: va=0x000c7360 tier=1
pub fn pair_fields_type_flag_conflict<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let object = c.ecx;
    c.eax = m.r(object, 0x270);
    let entry = c.esp;
    m.write32(entry.wrapping_sub(4), c.esi);
    c.esi = m.r(object, 0x26c);
    m.write32(entry.wrapping_sub(8), c.edi);
    if c.eax != 0 {
        c.edx = m.read32(c.eax);
        if c.edx != 0x13 && m.read32(c.esi) != 0x13 {
            c.edi = m.r(object, 0x2a8);
            if c.edi == 0 || c.edx != 0 {
                c.eax = m.r(c.eax, 4);
                if c.eax & 0x400 == 0 {
                    c.ecx = m.r(c.esi, 4);
                    if c.ecx & 0x400 == 0 && (c.eax & 1 == 0 || c.ecx & 1 == 0) {
                        c.edi = m.read32(entry.wrapping_sub(8));
                        c.eax = 0;
                        c.esi = m.read32(entry.wrapping_sub(4));
                        return;
                    }
                }
            }
        }
    }
    c.edi = m.read32(entry.wrapping_sub(8));
    c.eax = 1;
    c.esi = m.read32(entry.wrapping_sub(4));
}

/// 000DA220 cdecl(1). Visibility test of the node: type word 15 needs bit 80, mode 7DE455 picks the flag pair, bit 2 checks the limit.
pub fn mode_visibility<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.edx = arg(m, c, 0);
    c.esp = c.esp.wrapping_sub(8);
    if m.read16(c.edx.wrapping_add(0x48)) == 0x15 {
        c.ecx = m.r(c.edx, 0x14) & 0x80;
        c.eax = c.ecx;
        if c.eax == 0 {
            c.eax = 0;
            c.esp = c.esp.wrapping_add(8);
            return;
        }
    }
    if m.read8(0x007d_e455) == 10 {
        c.eax = m.r(c.edx, 0x10);
        c.ecx = m.r(c.edx, 0x14);
    } else {
        c.eax = m.r(c.edx, 0x18);
        c.ecx = m.r(c.edx, 0x1c);
    }
    m.w(c.esp, 4, c.ecx);
    c.eax &= 2;
    c.ecx = 0;
    if c.eax != 0 {
        c.edx = m.r(c.edx, 8);
        if (c.edx as i32) >= (m.read32(0x0079_0950) as i32) {
            c.eax = 0;
            c.esp = c.esp.wrapping_add(8);
            return;
        }
    }
    c.eax = 1;
    c.esp = c.esp.wrapping_add(8);
}

/// 0010E780 cdecl(2). Stores the low byte of `value` at object+13 when the object is non-null, returns the object.
pub fn store_byte_13<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let (object, value) = (arg(m, c, 0), arg(m, c, 1));
    if object != 0 {
        m.write8(object.wrapping_add(0x13), value as u8);
    }
    c.eax = object;
}

/// 0011D990 cdecl(2). Sets bit 80000 in [block+C6C] and stores `value` at [block+D38] (block = [object+7C]), returns the block.
pub fn tag_block<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let (object, value) = (arg(m, c, 0), arg(m, c, 1));
    if object == 0 {
        c.eax = 0;
        return;
    }
    let block = m.r(object, 0x7c);
    let flags = m.r(block, 0xc6c) | 0x80000;
    m.w(block, 0xc6c, flags);
    m.w(block, 0xd38, value);
    c.eax = block;
}

/// 001286D0 cdecl(0). Zeroes 108 dwords at 742360 and 380 at 7ABFA0 (`rep stosd`, honours DF), resets the counters.
// portreg: va=0x001286d0 tier=1
pub fn tables_742360_7abfa0_reset<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.push(m, c.edi);
    c.eax = 0;
    c.ecx = 108;
    c.edi = 0x0074_2360;
    c.rep_stosd(m);
    c.ecx = 380;
    c.edi = 0x007a_bfa0;
    c.rep_stosd(m);
    m.write32(0x0074_235c, c.eax);
    c.eax = m.read32(0x0074_97a8);
    m.write32(0x007a_bf80, 1);
    m.write32(0x0074_2358, c.eax);
    c.edi = c.pop(m);
}

/// 00158D40 cdecl(4). Clears extension state masks at +28/+2C of [arg 1]+7C, bit 80 when only arg 4 carries 0C000000.
// portreg: va=0x00158d40 tier=1
pub fn ext_clear_state_flag_masks<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = arg(m, c, 0);
    c.eax = m.r(c.eax, 0x7c);
    c.ecx = m.r(c.eax, 0x2c) & 0xffff_fbde;
    c.edx = m.r(c.eax, 0x28) & 0xfffb_b7fc;
    m.w(c.eax, 0x2c, c.ecx);
    c.ecx = arg(m, c, 3);
    c.push(m, c.esi);
    m.w(c.eax, 0x28, c.edx);
    c.esi = c.edx;
    c.ecx &= 0x0c00_0000;
    c.edx = 0;
    if c.ecx != 0 {
        c.ecx = arg(m, c, 2) & 0x0c00_0000;
        if c.ecx == 0 {
            c.esi &= 0xffff_ff7f;
            m.w(c.eax, 0x28, c.esi);
        }
    }
    c.esi = c.pop(m);
}

/// 0015A270 cdecl(1). Related value of the object by the type at [[object+7C]+4]+20, -1 when none applies.
pub fn related_value<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = arg(m, c, 0);
    c.edx = m.r(c.eax, 0x7c);
    c.ecx = m.r(c.edx, 4);
    c.eax = u32::MAX;
    if c.ecx == 0 {
        c.eax = m.r(c.edx, 0x684);
        return;
    }
    c.edx = m.r(c.ecx, 0x20);
    if (c.edx as i32) > 0x200 {
        if c.edx == 0x2000 {
            c.edx = m.r(c.ecx, 0x7c);
            c.eax = m.r(c.edx, 4);
        }
        return;
    }
    if c.edx == 0x200 {
        c.edx = m.read32(0x007b_0c48);
        c.eax = m.r(c.edx, 0x14);
        c.ecx = m.r(c.eax, 0x7c);
        c.eax = m.r(c.ecx, 0x678);
        return;
    }
    c.edx = c.edx.wrapping_sub(1);
    if c.edx == 0 {
        c.eax = m.r(c.ecx, 0x110);
        return;
    }
    c.edx = c.edx.wrapping_sub(7);
    if c.edx == 0 {
        c.ecx = m.r(c.ecx, 0x7c);
        c.eax = m.r(c.ecx, 0x678);
    }
}

/// 0015C120 cdecl(1). For the [74C37C] entries of the table at 7A3580 owned by arg: clears the owner and bit 80 of +2C.
// portreg: va=0x0015c120 tier=1
pub fn clear_matching_owner<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x0074_c37c);
    c.edx = 0;
    c.ecx = 0;
    if (c.eax as i32) <= 0 {
        return;
    }
    c.push(m, c.ebx);
    c.push(m, c.edi);
    c.edi = c.at(m, 12);
    loop {
        c.eax = m.read32(c.ecx.wrapping_mul(4).wrapping_add(0x007a_3580));
        if c.eax != 0 && m.r(c.eax, 4) == c.edi {
            c.ebx = m.r(c.eax, 0x2c) & 0xffff_ff7f;
            m.w(c.eax, 4, 0);
            m.w(c.eax, 0x2c, c.ebx);
        }
        c.eax = m.read32(0x0074_c37c);
        c.ecx = c.ecx.wrapping_add(1);
        if !((c.ecx as i32) < (c.eax as i32)) {
            break;
        }
    }
    c.edi = c.pop(m);
    c.ebx = c.pop(m);
}

/// 0015C3D0 cdecl(1). Clears pending bit 4 of the owner record and appends the object to the active array at 7A3580.
// portreg: va=0x0015c3d0 tier=1
pub fn clear_inactive_flag_and_append<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = c.at(m, 4);
    c.eax = m.r(c.eax, 0x7c);
    c.ecx = m.r(c.eax, 0x2c);
    if c.ecx & 4 == 0 {
        c.eax = 0;
        return;
    }
    c.ecx &= 0xffff_fffb;
    m.w(c.eax, 0x2c, c.ecx);
    c.ecx = m.read32(c.eax);
    let value = m.r(c.ecx, 0x28) | 0x61;
    m.w(c.ecx, 0x28, value);
    c.ecx = (c.ecx & 0xffff_ff00) | u32::from(m.b(c.eax, 0x28));
    if c.ecx & 0x80 != 0 {
        c.eax = 0;
        return;
    }
    c.ecx = m.r(c.eax, 0x10);
    c.ecx &= 0x0400_0000;
    c.edx = 0;
    c.ecx |= c.edx;
    if c.ecx != 0 {
        c.eax = 0;
        return;
    }
    c.ecx = m.r(c.eax, 8);
    if (c.ecx as i32) < (m.read32(0x0079_0950) as i32) {
        c.eax = 0;
        return;
    }
    c.edx = m.read32(c.eax);
    c.eax = m.read32(0x0074_c37c);
    c.ecx = m.r(c.edx, 0x7c);
    m.write32(c.eax.wrapping_mul(4).wrapping_add(0x007a_3580), c.ecx);
    c.eax = c.eax.wrapping_add(1);
    m.write32(0x0074_c37c, c.eax);
    c.eax = 1;
}

/// 00160850 cdecl(1). Appends the object to the five entry array at 7A3560 (count 7A36C8) when its extension has flag 40.
// portreg: va=0x00160850 tier=0
pub fn append_flagged_object<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let object = arg(m, c, 0);
    let mut count = m.read32(0x007a_36c8);
    if (count as i32) >= 5 {
        c.eax = count;
        return;
    }
    let extension = m.r(object, 0x7c);
    if m.b(extension, 8) & 0x40 == 0 {
        c.eax = count;
        return;
    }
    m.write32(count.wrapping_mul(4).wrapping_add(0x007a_3560), object);
    count = count.wrapping_add(1);
    m.write32(0x007a_36c8, count);
    c.eax = count;
}

/// 00160F50 cdecl(1). 1 when the type-8 object's tag at [[ext]+BF0]+8 is VICE or VICO.
pub fn vehicle_tag_test<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = arg(m, c, 0);
    let kind = m.r(c.eax, 0x20);
    c.ecx = m.r(c.eax, 0x7c);
    if kind == 8 {
        c.eax = m.r(c.ecx, 0xbf0);
        c.eax = m.r(c.eax, 8);
        c.eax = u32::from(c.eax == 0x4543_4956 || c.eax == 0x4f43_4956);
    } else {
        c.eax = 0;
    }
}

/// 00163810 cdecl(0). 7 + (global 79094C != 4).
pub fn global_79094c_ne_4_plus_7<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(0x0079_094c);
    c.eax = 7 + u32::from(c.ecx != 4);
}

/// 001665A0 cdecl(0). Pops the 38-byte entry of the pool at [74C468] (count 4E826C), 0 when empty.
// portreg: va=0x001665a0 tier=1
pub fn pool_74c468_pop<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x004e_826c);
    if (c.eax as i32) > 0 {
        c.ecx = m.read32(0x0074_c468);
        c.eax = c.eax.wrapping_sub(1);
        m.write32(0x004e_826c, c.eax);
        c.eax = c.eax.wrapping_mul(56).wrapping_add(c.ecx);
    } else {
        c.eax = 0;
    }
}

/// 00167A70 cdecl(1). Address 17C into the 200-byte entry (index arg & FFFF) of the table at [74C458].
pub fn entry_74c458_field_17c<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = arg(m, c, 0);
    c.ecx = m.read32(0x0074_c458);
    c.eax = ((c.eax & 0xffff) << 9).wrapping_add(c.ecx).wrapping_add(0x17c);
}

/// 00172C80 cdecl(2). Next slot (stride 34) of the ring in [object+7C]+70 by the index stored at arg 2, wrapping at count [+140].
pub fn component_slot_next_in_ring<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = arg(m, c, 0);
    c.ecx = m.r(c.eax, 0x7c);
    c.push(m, c.esi);
    c.edx = 0;
    c.push(m, c.edi);
    c.edi = c.at(m, 0x10);
    c.eax = m.read32(c.edi);
    c.edx = u32::from((c.eax as i32) < 0);
    c.edx = c.edx.wrapping_sub(1);
    c.edx &= c.eax;
    c.esi = c.edx;
    c.edx = m.r(c.ecx, 0x140);
    c.edx = c.edx.wrapping_sub(1);
    if (c.edx as i32) < (c.esi as i32) {
        c.eax = c.edx;
    } else {
        c.edx = u32::from((c.eax as i32) < 0);
        c.edx = c.edx.wrapping_sub(1);
        c.eax &= c.edx;
    }
    c.edx = c.eax;
    c.edx = c.edx.wrapping_mul(0x34);
    m.write32(c.edi, c.eax);
    c.esi = c.edx.wrapping_add(c.ecx).wrapping_add(0x70);
    c.eax = c.eax.wrapping_add(1);
    c.edx = ((c.eax as i32) >> 31) as u32;
    // idiv [ecx+140]: the divisor is read after the store above (aliasing is visible).
    let divisor = i64::from(m.r(c.ecx, 0x140) as i32);
    let numerator = ((u64::from(c.edx) << 32) | u64::from(c.eax)) as i64;
    assert!(divisor != 0, "#DE: the C replacement traps here");
    c.eax = (numerator / divisor) as i32 as u32;
    c.edx = (numerator % divisor) as i32 as u32;
    c.eax = c.esi;
    m.write32(c.edi, c.edx);
    c.edi = c.pop(m);
    c.esi = c.pop(m);
}

/// 001766E0 cdecl(2). 0 when slot arg 2 (stride 34 at [object+7C]+70) holds a live handle, else clears it and returns 1.
pub fn slot_handle_invalid<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = arg(m, c, 1);
    c.ecx = arg(m, c, 0);
    c.eax = c.eax.wrapping_mul(0x34);
    c.edx = m.r(c.ecx, 0x7c);
    c.ecx = m.read32(c.edx.wrapping_add(c.eax).wrapping_add(0x70));
    c.edx = c.edx.wrapping_add(c.eax).wrapping_add(0x70);
    if c.ecx != 0 {
        c.eax = (c.ecx >> 22).wrapping_mul(0x2e8);
        c.eax = c.eax.wrapping_add(m.read32(0x0072_f5e8));
        if m.r(c.eax, 0x2c) == c.ecx {
            c.eax = 0;
            return;
        }
        m.write32(c.edx, 0);
    }
    c.eax = 1;
}

/// 00176A40 cdecl(3). Stores the handle of the owner arg 2 in slot arg 3 unless the slot holds a live handle.
pub fn slot_handle_set<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.ecx = arg(m, c, 2);
    c.eax = arg(m, c, 0);
    c.ecx = c.ecx.wrapping_mul(0x34);
    c.eax = m.r(c.eax, 0x7c);
    c.edx = c.ecx.wrapping_add(c.eax).wrapping_add(0x70);
    c.ecx = m.read32(c.edx);
    if c.ecx != 0 {
        c.eax = (c.ecx >> 22).wrapping_mul(0x2e8);
        c.eax = c.eax.wrapping_add(m.read32(0x0072_f5e8));
        if m.r(c.eax, 0x2c) == c.ecx {
            return;
        }
        m.write32(c.edx, 0);
    }
    c.eax = arg(m, c, 1);
    c.ecx = m.r(c.eax, 0x2c);
    m.write32(c.edx, c.ecx);
    m.write8(c.eax.wrapping_add(0x32), 2);
}

/// 00177CE0 cdecl(1). 1 when some slot of the object holds a live handle with a ready controller (non-negative +4).
pub fn any_ready_slot<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.push(m, c.ebx);
    c.push(m, c.esi);
    c.push(m, c.edi);
    c.edi = c.at(m, 16);
    c.eax = m.r(c.edi, 0x140);
    c.esi = 0;
    if (c.eax as i32) > 0 {
        c.edx = c.edi.wrapping_add(0x70);
        loop {
            if m.read32(c.edx) != 0 {
                c.eax = m.r(c.edx, 0x30);
                if c.eax == 0 {
                    c.ecx = m.read32(c.edx);
                    if c.ecx != 0 {
                        c.ebx = m.read32(0x0072_f5e8);
                        c.eax = (c.ecx >> 22).wrapping_mul(0x2e8).wrapping_add(c.ebx);
                        if m.r(c.eax, 0x2c) == c.ecx {
                            c.eax = m.r(c.eax, 0x150);
                            if c.eax != 0 {
                                c.ecx = m.r(c.eax, 4);
                                if (c.ecx as i32) >= 0 {
                                    c.edi = c.pop(m);
                                    c.esi = c.pop(m);
                                    c.eax = 1;
                                    c.ebx = c.pop(m);
                                    return;
                                }
                            }
                        } else {
                            m.write32(c.edx, 0);
                        }
                    }
                }
            }
            c.eax = m.r(c.edi, 0x140);
            c.esi = c.esi.wrapping_add(1);
            c.edx = c.edx.wrapping_add(0x34);
            if !((c.esi as i32) < (c.eax as i32)) {
                break;
            }
        }
    }
    c.edi = c.pop(m);
    c.esi = c.pop(m);
    c.eax = 0;
    c.ebx = c.pop(m);
}

/// 00177EC0 cdecl(4). Initialises the request block at [object+7C]+568..588, returns the block.
pub fn init_request_block<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let (object, first, second, third) = (arg(m, c, 0), arg(m, c, 1), arg(m, c, 2), arg(m, c, 3));
    let block = m.r(object, 0x7c);
    m.w(block, 0x56c, first);
    m.w(block, 0x578, second);
    m.w(block, 0x568, 0);
    m.w(block, 0x570, 0xffff_ffff);
    m.w(block, 0x574, 1);
    m.w(block, 0x57c, third);
    m.w(block, 0x580, 0x12);
    m.w(block, 0x584, 0xffff_ffff);
    m.w(block, 0x588, 0xffff_ffff);
    c.eax = block;
}

/// 0017A120 cdecl(1). Slot (stride 34 at [owner+7C]+70) of the object's owner whose handle is the object's handle, else 0.
pub fn find_own_slot<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.push(m, c.esi);
    c.push(m, c.edi);
    c.edi = c.at(m, 12);
    c.ecx = m.r(c.edi, 0xc08);
    if c.ecx != 0 {
        c.esi = m.read32(0x0072_f5e8);
        c.eax = (c.ecx >> 22).wrapping_mul(0x2e8);
        c.edx = m.read32(c.eax.wrapping_add(c.esi).wrapping_add(0x2c));
        c.eax = c.eax.wrapping_add(c.esi);
        if c.edx == c.ecx {
            c.esi = m.r(c.eax, 0x7c);
            c.edx = m.r(c.esi, 0x140);
            c.eax = 0;
            if (c.edx as i32) > 0 {
                c.ecx = m.read32(c.edi);
                c.edi = m.r(c.ecx, 0x2c);
                c.ecx = c.esi.wrapping_add(0x70);
                loop {
                    if m.read32(c.ecx) == c.edi {
                        c.eax = c.eax.wrapping_mul(0x34);
                        c.edi = c.pop(m);
                        c.eax = c.eax.wrapping_add(c.esi).wrapping_add(0x70);
                        c.esi = c.pop(m);
                        return;
                    }
                    c.eax = c.eax.wrapping_add(1);
                    c.ecx = c.ecx.wrapping_add(0x34);
                    if !((c.eax as i32) < (c.edx as i32)) {
                        break;
                    }
                }
            }
            c.edi = c.pop(m);
            c.esi = c.pop(m);
            c.eax = 0;
            return;
        }
        m.w(c.edi, 0xc08, 0);
    }
    c.edi = c.pop(m);
    c.esi = c.pop(m);
    c.eax = 0;
}

/// 0017A200 cdecl(1). Number of live slot handles of the object (stale ones are cleared).
pub fn count_valid_slot_handles<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = c.at(m, 4);
    c.push(m, c.ebx);
    c.ebx = m.r(c.eax, 0x7c);
    c.ecx = m.r(c.ebx, 0x140);
    c.push(m, c.edi);
    c.edi = 0;
    c.eax = 0;
    if (c.ecx as i32) > 0 {
        c.push(m, c.ebp);
        c.push(m, c.esi);
        c.esi = c.ebx.wrapping_add(0x70);
        loop {
            c.edx = m.read32(c.esi);
            if c.edx != 0 {
                c.ebp = m.read32(0x0072_f5e8);
                c.ecx = (c.edx >> 22).wrapping_mul(0x2e8);
                c.ecx = c.ecx.wrapping_add(c.ebp);
                if m.r(c.ecx, 0x2c) == c.edx {
                    c.eax = c.eax.wrapping_add(1);
                } else {
                    m.write32(c.esi, 0);
                }
            }
            c.ecx = m.r(c.ebx, 0x140);
            c.edi = c.edi.wrapping_add(1);
            c.esi = c.esi.wrapping_add(0x34);
            if !((c.edi as i32) < (c.ecx as i32)) {
                break;
            }
        }
        c.esi = c.pop(m);
        c.ebp = c.pop(m);
    }
    c.edi = c.pop(m);
    c.ebx = c.pop(m);
}

/// 0017A260 cdecl(1). 1 when some slot handle of the object is empty or stale (stale ones are cleared), 0 in modes 1 to 4.
pub fn has_invalid_slot<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = c.at(m, 4);
    c.eax = m.r(c.eax, 0x7c);
    c.ecx = m.r(c.eax, 0x54);
    if matches!(c.ecx, 1..=4) || m.b(c.eax, 0x58) & 0x10 != 0 {
        c.eax = 0;
        return;
    }
    c.push(m, c.ebx);
    c.push(m, c.ebp);
    c.push(m, c.esi);
    c.push(m, c.edi);
    c.edi = m.r(c.eax, 0x140);
    c.esi = 0;
    let mut invalid = false;
    if (c.edi as i32) > 0 {
        c.ebx = m.read32(0x0072_f5e8);
        c.edx = c.eax.wrapping_add(0x70);
        loop {
            c.ecx = m.read32(c.edx);
            if c.ecx == 0 {
                invalid = true;
                break;
            }
            c.eax = (c.ecx >> 22).wrapping_mul(0x2e8);
            c.ebp = m.read32(c.eax.wrapping_add(c.ebx).wrapping_add(0x2c));
            c.eax = c.eax.wrapping_add(c.ebx);
            if c.ebp != c.ecx {
                m.write32(c.edx, 0);
                invalid = true;
                break;
            }
            c.esi = c.esi.wrapping_add(1);
            c.edx = c.edx.wrapping_add(0x34);
            if !((c.esi as i32) < (c.edi as i32)) {
                break;
            }
        }
    }
    c.edi = c.pop(m);
    c.esi = c.pop(m);
    c.ebp = c.pop(m);
    c.eax = u32::from(invalid);
    c.ebx = c.pop(m);
}

/// 00188920 cdecl(1). 1 when the extension state at [[object]+7C]+34 is 2, 3, 5 or 7 (object non-null), else the arg.
pub fn extension_four_categories<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = arg(m, c, 0);
    if c.eax != 0 {
        c.eax = m.r(c.eax, 0x7c);
        c.eax = m.r(c.eax, 0x34);
        c.eax = u32::from(matches!(c.eax, 2 | 3 | 5 | 7));
    }
}

/// 001B6310 cdecl(1). Sets bit 4 of [object+34], clears +F8 when it equals the pending value read earlier, returns the object.
pub fn set_flag_34_bit2_clear_f8<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let object = arg(m, c, 0);
    let flags = m.r(object, 0x34);
    let pending = m.r(object, 0xf8);
    m.w(object, 0x34, flags | 4);
    if m.r(object, 0xe8) == pending {
        m.w(object, 0xf8, 0);
    }
    c.eax = object;
}

/// 001B63B0 cdecl(2). 1 when [object+E8] equals arg 2.
pub fn field_e8_equals_arg<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.ecx = arg(m, c, 0);
    c.edx = m.r(c.ecx, 0xe8);
    c.ecx = arg(m, c, 1);
    c.eax = u32::from(c.edx == c.ecx);
}

/// 001B63D0 cdecl(2). 1 when [object+E8] equals the state and [object+F8] does not.
pub fn field_e8_only_matches<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let state = arg(m, c, 1);
    c.ecx = arg(m, c, 0);
    c.eax = u32::from(m.r(c.ecx, 0xe8) == state && m.r(c.ecx, 0xf8) != state);
}

/// 001B7580 cdecl(1). 1 when [object+E8] is 10, 11, 12 or 27.
// portreg: va=0x0023ff70 tier=1
pub fn kind_is_10_11_12_27<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let object = arg(m, c, 0);
    let kind = m.r(object, 0xe8);
    c.eax = u32::from(matches!(kind, 10 | 11 | 12 | 27));
}

/// 001B75B0 cdecl(2). Field +68 of the block at object+184 (or +55C for a non-zero selector) when its +54 is set.
// portreg: va=0x001b75b0 tier=0
pub fn select_block_get_field_68<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    let (object, selector) = (arg(m, c, 0), arg(m, c, 1));
    let block = object.wrapping_add(if selector != 0 { 0x55c } else { 0x184 });
    c.eax = if m.r(block, 0x54) != 0 { m.r(block, 0x68) } else { 0 };
}

/// 0023FF70 cdecl(1). Index ((node - 75DAF0) >> 4) of the sorted-by-key list node (head [75D318], next at +8) with key arg, else -1.
pub fn sorted_key_index<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x0075_d318);
    if c.eax != 0 {
        c.edx = arg(m, c, 0);
        loop {
            c.ecx = m.r(c.eax, 4);
            if c.ecx == c.edx {
                c.eax = ((c.eax.wrapping_sub(0x0075_daf0)) as i32 >> 4) as u32;
                return;
            }
            if c.ecx > c.edx {
                break;
            }
            c.eax = m.r(c.eax, 8);
            if c.eax == 0 {
                break;
            }
        }
    }
    c.eax = u32::MAX;
}

/// 00240020 cdecl(1). Owner slot of the named handle (150 entries of 8 bytes at 75CE68), clearing a stale handle, else 0.
pub fn named_handle_resolve<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.ecx = arg(m, c, 0);
    c.eax = 0;
    while m.read32(c.eax.wrapping_mul(8).wrapping_add(0x0075_ce68)) != c.ecx {
        c.eax = c.eax.wrapping_add(1);
        if (c.eax as i32) >= 150 {
            c.eax = 0;
            return;
        }
    }
    c.eax = m.read32(c.eax.wrapping_mul(8).wrapping_add(0x0075_ce6c));
    c.edx = m.read32(c.eax);
    if c.edx == 0 {
        c.eax = 0;
        return;
    }
    c.ecx = (c.edx >> 22).wrapping_mul(0x2e8);
    c.ecx = c.ecx.wrapping_add(m.read32(0x0072_f5e8));
    if m.r(c.ecx, 0x2c) == c.edx {
        c.eax = c.ecx;
    } else {
        m.write32(c.eax, 0);
        c.eax = 0;
    }
}

/// 00242EC0 cdecl(1). 1 when the extension state [[object+7C]+10] is 1.
pub fn extension_state1_test<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.eax = arg(m, c, 0);
    c.ecx = m.r(c.eax, 0x7c);
    c.edx = m.r(c.ecx, 0x10);
    c.eax = u32::from(c.edx == 1);
}

/// 00247A70 cdecl(2). Stores controller field +4 of [[[object+7C]+10]+150] to arg 2 for a class 10000000 object, returns 1/0.
pub fn class_10000000_controller_field<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.ecx = arg(m, c, 0);
    let class_matches = m.r(c.ecx, 0x20) == 0x1000_0000;
    c.eax = m.r(c.ecx, 0x7c);
    let ok = 'chain: {
        if !class_matches || c.eax == 0 {
            break 'chain false;
        }
        c.ecx = m.r(c.eax, 4);
        if c.ecx == 0 {
            break 'chain false;
        }
        c.eax = m.r(c.eax, 0x10);
        if c.eax == 0 {
            break 'chain false;
        }
        c.eax = m.r(c.eax, 0x150);
        if c.eax == 0 {
            break 'chain false;
        }
        c.eax = m.r(c.eax, 4);
        if (c.eax as i32) < 0 {
            break 'chain false;
        }
        c.ecx = arg(m, c, 1);
        if c.ecx != 0 {
            m.write32(c.ecx, c.eax);
        }
        true
    };
    c.eax = u32::from(ok);
}

/// 002533E0 cdecl(0). 1 when the global 78B594 is 10.
pub fn global_78b594_is_10<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(0x0078_b594);
    c.eax = u32::from(c.ecx == 10);
}

/// 002744C0 cdecl(1). Field +3C of the extension of the object stored at 7A2BEC + 8 * (arg == 1).
pub fn flag_slot_extension_field_3c<M: GuestMemory>(m: &mut M, c: &mut Cpu) {
    c.edx = arg(m, c, 0);
    c.eax = u32::from(c.edx == 1);
    c.ecx = m.read32(c.eax.wrapping_mul(8).wrapping_add(0x007a_2bec));
    c.edx = m.r(c.ecx, 0x7c);
    c.eax = c.eax.wrapping_add(1);
    c.eax = m.r(c.edx, 0x3c);
}
