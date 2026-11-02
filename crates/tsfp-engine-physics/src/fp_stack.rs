// SPDX-License-Identifier: GPL-3.0-or-later
//! T1835 wave 9: ports of the 17 pool roots that needed a guest-stack push/pop helper and a
//! `rep stosd` / `rep movs` helper (docs/t1835-rust-wave9.md). [`Cpu`] models the general registers
//! the C replacements touch including the callee-saved EBX/ESI/EDI/EBP and the direction flag.
//! `push` really writes the guest stack slot and `pop` really reads it back, so an argument or
//! record that overlaps the saved slots changes the restored register exactly as in the C body
//! (the saved-slot semantics wave 8 could not express). Every port is a one-to-one translation of
//! a proven C replacement of `src/game`; operation order, aliasing stores and untouched registers
//! are preserved. Equality with the C is MEASURED by the `wave9` differential, equality with the
//! original is INFERRED (the C bodies are proven against the original XBE).
use crate::{fp_scalar::Fp, memory::GuestMemory, sse};

/// Read a cdecl argument from the current guest stack frame.
/// Argument zero follows the return address at ESP + 4.
pub(crate) fn cdecl_arg<M: GuestMemory>(m: &M, cpu: &Cpu, index: u32) -> u32 {
    cpu.at(m, 4 + 4 * index)
}

/// Register file of a port: EAX/ECX/EDX, the callee-saved registers, the live `esp` and DF.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cpu {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebx: u32,
    pub ebp: u32,
    pub esp: u32,
    /// Direction flag: string operations step by -size when set.
    pub df: bool,
}

impl Cpu {
    /// Registers at function entry (`esp` points at the return address).
    pub fn entry(esp: u32, eax: u32, ecx: u32, edx: u32, esi: u32, extra: [u32; 4]) -> Cpu {
        Cpu { eax, ecx, edx, esi, ebx: extra[0], edi: extra[1], ebp: extra[2], esp, df: extra[3] != 0 }
    }

    /// `push value`: lower `esp`, store the dword in the guest stack slot.
    pub fn push<M: GuestMemory>(&mut self, m: &mut M, value: u32) {
        self.esp = self.esp.wrapping_sub(4);
        m.write32(self.esp, value);
    }

    /// `pop`: read the dword currently in the guest slot (which the body may have overwritten).
    pub fn pop<M: GuestMemory>(&mut self, m: &M) -> u32 {
        let value = m.read32(self.esp);
        self.esp = self.esp.wrapping_add(4);
        value
    }

    /// Dword at `esp + offset`.
    pub fn at<M: GuestMemory>(&self, m: &M, offset: u32) -> u32 {
        m.read32(self.esp.wrapping_add(offset))
    }

    fn step(&self, size: u32) -> u32 {
        if self.df { size.wrapping_neg() } else { size }
    }

    /// `rep stosd`: store EAX at EDI, ECX times, one dword per iteration.
    pub fn rep_stosd<M: GuestMemory>(&mut self, m: &mut M) {
        let step = self.step(4);
        while self.ecx != 0 {
            m.write32(self.edi, self.eax);
            self.edi = self.edi.wrapping_add(step);
            self.ecx -= 1;
        }
    }

    /// `rep movsd`: copy ECX dwords from ESI to EDI, read then write per iteration.
    pub fn rep_movsd<M: GuestMemory>(&mut self, m: &mut M) {
        let step = self.step(4);
        while self.ecx != 0 {
            let value = m.read32(self.esi);
            m.write32(self.edi, value);
            self.esi = self.esi.wrapping_add(step);
            self.edi = self.edi.wrapping_add(step);
            self.ecx -= 1;
        }
    }

    /// `rep movsb`: copy ECX bytes from ESI to EDI.
    pub fn rep_movsb<M: GuestMemory>(&mut self, m: &mut M) {
        let step = self.step(1);
        while self.ecx != 0 {
            let value = m.read8(self.esi);
            m.write8(self.edi, value);
            self.esi = self.esi.wrapping_add(step);
            self.edi = self.edi.wrapping_add(step);
            self.ecx -= 1;
        }
    }
}

/// 000B3C60 cdecl(1). EAX = 1 when either count at [[a+7C]+6C]+F0/F4 is below 10 and 0 is not below [x+1C4].
pub fn extension_count_float_predicate<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.eax = c.at(f.m, 4);
    c.ecx = f.r(c.eax.wrapping_add(0x7c));
    c.eax = f.r(c.ecx.wrapping_add(0x6c));
    c.push(f.m, c.esi);
    c.esi = f.r(c.eax.wrapping_add(0xf0));
    c.edx = 10;
    if (c.esi as i32) < 10 || (f.r(c.eax.wrapping_add(0xf4)) as i32) < 10 {
        f.x[0] = [0; 4];
        if !sse::comiss(f.x[0][0], f.r(c.ecx.wrapping_add(0x1c4))).b() {
            c.eax = 1;
            c.esi = c.pop(f.m);
            return;
        }
    }
    c.eax = 0;
    c.esi = c.pop(f.m);
}

/// 000B0DD0 cdecl(1). Activates the extension of the object: state 2, timers from [475C78], flag masks.
pub fn extension_activate<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.ecx = c.at(f.m, 4);
    if c.ecx == 0 {
        return;
    }
    c.eax = f.r(c.ecx.wrapping_add(0x7c));
    if c.eax == 0 {
        return;
    }
    c.push(f.m, c.esi);
    c.esi = f.r(c.eax.wrapping_add(0x10));
    c.edx = 2;
    if c.esi != c.edx {
        c.esi = f.r(c.eax.wrapping_add(0xc));
        f.load(0, 0x0047_5c78);
        f.w(c.eax.wrapping_add(0x10), c.edx);
        f.store(c.eax.wrapping_add(0x74), 0);
        f.store(c.eax.wrapping_add(0x24), 0);
        f.x[0] = [0; 4];
        c.edx = 0xfbff_ffff;
        c.esi &= c.edx;
        f.w(c.eax.wrapping_add(0xc), c.esi);
        f.w(c.eax.wrapping_add(0x20), 0);
        f.store(c.eax.wrapping_add(0x38), 0);
        c.esi = f.r(c.ecx.wrapping_add(0x18));
        c.eax = f.r(c.ecx.wrapping_add(0x28));
        c.esi |= 0x42;
        c.eax &= c.edx;
        f.w(c.ecx.wrapping_add(0x18), c.esi);
        f.w(c.ecx.wrapping_add(0x28), c.eax);
    }
    c.esi = c.pop(f.m);
}

/// 0008CBC0 cdecl(2). Insertion step of a 4-slot scene list ordered by the float at +2C.
pub fn scene_insert_four<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.edx = c.at(f.m, 4);
    c.push(f.m, c.esi);
    c.esi = c.at(f.m, 0xc);
    c.push(f.m, c.edi);
    c.edi = f.r(c.edx.wrapping_add(0x10));
    c.eax = f.r(c.edi.wrapping_add(4));
    if (c.eax as i32) >= 0 {
        c.eax = f.r(c.esi);
        f.load(0, c.edi.wrapping_add(0x2c));
        if f.cmp_m(0, c.eax.wrapping_add(0x2c)).be() {
            c.edi = c.pop(f.m);
            c.esi = c.pop(f.m);
            return;
        }
    }
    c.ecx = 3;
    c.push(f.m, c.ebx);
    loop {
        let slot = c.edx.wrapping_add(c.ecx.wrapping_mul(4));
        c.eax = f.r(slot);
        c.ebx = f.r(c.eax.wrapping_add(4));
        if (c.ebx as i32) >= 0 {
            c.ebx = f.r(c.esi);
            f.load(0, c.eax.wrapping_add(0x2c));
            if f.cmp_m(0, c.ebx.wrapping_add(0x2c)).be() {
                c.eax = f.r(c.esi);
                f.w(slot.wrapping_add(4), c.eax);
                f.w(c.esi, c.edi);
                break;
            }
        }
        f.w(slot.wrapping_add(4), c.eax);
        if c.ecx == 0 {
            c.eax = f.r(c.esi);
            f.w(c.edx, c.eax);
            f.w(c.esi, c.edi);
        }
        c.ecx = c.ecx.wrapping_sub(1);
        if (c.ecx as i32) < 0 {
            break;
        }
    }
    c.ebx = c.pop(f.m);
    c.edi = c.pop(f.m);
    c.esi = c.pop(f.m);
}

/// 001115F0 cdecl(8). Zeroes the 0xCC byte particle descriptor (`rep stosd`, honours DF) and fills its fields.
// portreg: va=0x001115f0 tier=1
pub fn particle_descriptor_init<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.edx = c.at(f.m, 4);
    f.x[0] = [c.at(f.m, 0x14), 0, 0, 0];
    c.push(f.m, c.edi);
    c.eax = 0;
    c.ecx = 0x33;
    c.edi = c.edx;
    c.rep_stosd(f.m);
    c.ecx = c.at(f.m, 0x10);
    c.eax = c.at(f.m, 0xc);
    f.w(c.edx.wrapping_add(0x14), c.ecx);
    c.ecx = c.at(f.m, 0x14);
    f.w(c.edx, c.ecx);
    c.ecx = c.at(f.m, 0x1c);
    f.w(c.edx.wrapping_add(0x10), c.ecx);
    c.ecx = c.at(f.m, 0x20);
    f.store(c.edx.wrapping_add(4), 0);
    f.x[0][0] = c.at(f.m, 0x24);
    f.w(0x0073_bd68, c.edx);
    f.w(c.edx.wrapping_add(0xc), c.eax);
    f.w(c.edx.wrapping_add(0x18), c.ecx);
    f.store(c.edx.wrapping_add(0x1c), 0);
    c.edi = c.pop(f.m);
    if c.eax == 5 || c.eax == 6 {
        f.load(0, 0x0047_5c78);
        f.store(c.edx.wrapping_add(0xa0), 0);
    }
}

/// 00126AC0 cdecl(12). Pushes a 0x38 byte entry into the 240 entry ring at 73ED18 (index at 73ED10).
pub fn ring_push_entry<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.ecx = f.r(0x0073_ed10);
    c.eax = c.ecx.wrapping_mul(0x38).wrapping_add(0x0073_ed18);
    c.ecx = c.ecx.wrapping_add(1);
    f.w(0x0073_ed10, c.ecx);
    if c.ecx == 0xf0 {
        f.w(0x0073_ed10, 0);
    }
    c.ecx = c.at(f.m, 4);
    f.load(0, 0x0074_2198);
    c.edx = c.at(f.m, 8);
    f.store(c.eax.wrapping_add(8), 0);
    f.load(0, c.esp.wrapping_add(0x2c));
    f.store(c.eax.wrapping_add(0xc), 0);
    f.load(0, c.esp.wrapping_add(0xc));
    c.push(f.m, c.esi);
    f.w(c.eax, c.edx);
    f.store(c.eax.wrapping_add(0x10), 0);
    f.load(0, c.esp.wrapping_add(0x14));
    f.w(c.eax.wrapping_add(4), c.ecx);
    c.ecx = c.at(f.m, 0x34);
    f.store(c.eax.wrapping_add(0x14), 0);
    f.load(0, c.esp.wrapping_add(0x18));
    c.edx = c.ecx;
    c.esi = c.ecx << 16;
    c.edx &= 0xff00;
    c.edx |= c.esi;
    f.store(c.eax.wrapping_add(0x18), 0);
    f.load(0, c.esp.wrapping_add(0x1c));
    f.store(c.eax.wrapping_add(0x20), 0);
    f.load(0, c.esp.wrapping_add(0x20));
    c.esi = c.ecx & 0x00ff_0000;
    f.store(c.eax.wrapping_add(0x24), 0);
    f.load(0, c.esp.wrapping_add(0x24));
    c.ecx >>= 16;
    c.esi |= c.ecx;
    f.store(c.eax.wrapping_add(0x28), 0);
    f.load(0, c.esp.wrapping_add(0x28));
    c.esi >>= 8;
    c.edx <<= 8;
    c.edx |= c.esi;
    f.store(c.eax.wrapping_add(0x2c), 0);
    f.load(0, c.esp.wrapping_add(0x2c));
    f.store(c.eax.wrapping_add(0x30), 0);
    f.w(c.eax.wrapping_add(0x34), c.edx);
    c.esi = c.pop(f.m);
}

/// 00128E90 cdecl(2) (list, limit). Removes the 0x34 byte entries whose float at +30 exceeds limit, compacting the tail.
pub fn list_remove_entries_above<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.push(f.m, c.ebx);
    c.ebx = c.at(f.m, 8);
    c.eax = f.r(c.ebx).wrapping_sub(1);
    if (c.eax as i32) >= 0 {
        f.load(0, c.esp.wrapping_add(0xc));
        c.ecx = c.eax.wrapping_mul(0x34);
        c.push(f.m, c.ebp);
        c.push(f.m, c.esi);
        c.edx = c.ecx.wrapping_add(c.ebx).wrapping_add(0x3c);
        c.push(f.m, c.edi);
        loop {
            f.load(1, c.edx.wrapping_sub(0xc));
            if !f.cmp(1, 0).be() {
                c.ecx = f.r(c.ebx).wrapping_sub(c.eax).wrapping_sub(1).wrapping_mul(0x34);
                c.ebp = c.ecx;
                c.ecx >>= 2;
                c.esi = c.edx;
                c.edi = c.edx.wrapping_sub(0x34);
                c.rep_movsd(f.m);
                c.ecx = c.ebp & 3;
                c.rep_movsb(f.m);
                let count = f.r(c.ebx).wrapping_sub(1);
                f.w(c.ebx, count);
            }
            c.eax = c.eax.wrapping_sub(1);
            c.edx = c.edx.wrapping_sub(0x34);
            if (c.eax as i32) < 0 {
                break;
            }
        }
        c.edi = c.pop(f.m);
        c.esi = c.pop(f.m);
        c.ebp = c.pop(f.m);
    }
    c.ebx = c.pop(f.m);
}

/// 00129EC0 cdecl(1). Releases the child pointer table of an object: clears float, owner bytes and table-742360 entries.
// portreg: va=0x00129ec0 tier=0
pub fn object_release_children<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.push(f.m, c.ebx);
    c.push(f.m, c.ebp);
    c.ebp = c.at(f.m, 0xc);
    c.ecx = f.r(c.ebp.wrapping_add(0x1c));
    c.eax = f.r(c.ebp.wrapping_add(0x1f8));
    c.push(f.m, c.edi);
    c.ebx = 0;
    c.edi = 0;
    if (c.ecx as i32) > 0 {
        f.load(0, 0x0047_d9d4);
        f.zero(1);
        c.push(f.m, c.esi);
        c.esi = c.eax;
        loop {
            c.eax = f.r(c.esi);
            if f.m.read16(c.eax.wrapping_add(0x28)) & 0x40c == 0 {
                f.store(c.eax, 0);
            } else {
                f.store(c.eax, 1);
            }
            c.eax = c.eax.wrapping_add(0xe);
            c.edx = 2;
            loop {
                c.ecx = (c.ecx & 0xffff_ff00) | u32::from(f.m.read8(c.eax));
                if (c.ecx as u8) < 0x80 {
                    c.ecx = c.ecx as u8 as i8 as i32 as u32;
                    c.ecx = c.ecx.wrapping_mul(0x4c);
                    f.w(c.ecx.wrapping_add(0x007a_bfa4), c.ebx);
                    f.m.write8(c.eax, 0xff);
                }
                c.eax = c.eax.wrapping_add(1);
                c.edx -= 1;
                if c.edx == 0 {
                    break;
                }
            }
            c.eax = 0x0074_2360;
            loop {
                if f.r(c.eax) != c.ebx {
                    c.edx = f.r(c.eax.wrapping_add(4));
                    if c.edx == f.r(c.esi) {
                        f.w(c.eax, c.ebx);
                    }
                }
                c.eax = c.eax.wrapping_add(0x6c);
                if (c.eax as i32) >= 0x0074_2510 {
                    break;
                }
            }
            c.eax = f.r(c.ebp.wrapping_add(0x1c));
            c.edi = c.edi.wrapping_add(1);
            c.esi = c.esi.wrapping_add(4);
            if (c.edi as i32) >= (c.eax as i32) {
                break;
            }
        }
        c.esi = c.pop(f.m);
    }
    c.edi = c.pop(f.m);
    c.ebp = c.pop(f.m);
    c.ebx = c.pop(f.m);
}

/// 00166770 cdecl(2) (head slot, node). Inserts node into the singly linked list (next at +34) ordered by the float at +1C.
pub fn sorted_list_insert<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.ecx = c.at(f.m, 4);
    c.eax = f.r(c.ecx);
    c.edx = c.at(f.m, 8);
    if c.eax != 0 {
        f.load(0, c.eax.wrapping_add(0x1c));
        if !f.cmp_m(0, c.edx.wrapping_add(0x1c)).a() {
            loop {
                c.ecx = f.r(c.eax.wrapping_add(0x34));
                if c.ecx == 0 {
                    break;
                }
                f.load(0, c.ecx.wrapping_add(0x1c));
                if f.cmp_m(0, c.edx.wrapping_add(0x1c)).a() {
                    break;
                }
                c.eax = c.ecx;
                if c.eax == 0 {
                    return;
                }
            }
            c.ecx = f.r(c.eax.wrapping_add(0x34));
            f.w(c.edx.wrapping_add(0x34), c.ecx);
            f.w(c.eax.wrapping_add(0x34), c.edx);
            return;
        }
    }
    f.w(c.ecx, c.edx);
    f.w(c.edx.wrapping_add(0x34), c.eax);
}

/// 001B7AA0 cdecl(3). Zeroes the 0x3D8 byte struct (`rep stosd`) and sets fields 30, 3C, 3CC from the arguments.
// portreg: va=0x001b7aa0 tier=1
pub fn zero_struct_3d8_init<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.edx = c.at(f.m, 4);
    f.x[0] = [0; 4];
    c.eax = 0;
    c.push(f.m, c.edi);
    c.ecx = 0xf6;
    c.edi = c.edx;
    c.rep_stosd(f.m);
    c.ecx = 0xffff_ffff;
    f.w(c.edx.wrapping_add(0x30), c.ecx);
    f.w(c.edx.wrapping_add(0x44), c.eax);
    f.w(c.edx.wrapping_add(0x34), c.eax);
    f.w(c.edx.wrapping_add(0x54), c.eax);
    f.w(c.edx.wrapping_add(0x5c), c.eax);
    f.w(c.edx.wrapping_add(0xc8), c.ecx);
    c.ecx = c.at(f.m, 0x10);
    f.w(c.edx.wrapping_add(0x68), c.eax);
    c.eax = c.at(f.m, 0xc);
    f.store(c.edx.wrapping_add(0x48), 0);
    f.w(c.edx.wrapping_add(0x3cc), c.eax);
    f.w(c.edx.wrapping_add(0x3c), c.ecx);
    c.edi = c.pop(f.m);
}

/// 0025F9C0 cdecl(1). Appends a 0x20 byte entry copied from the argument to the table at 78B2E0 (count 78B520, max 18).
pub fn table_append_entry<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.edx = f.r(0x0078_b520);
    if (c.edx as i32) >= 18 {
        return;
    }
    c.ecx = c.at(f.m, 4);
    f.x[0] = [0; 4];
    c.push(f.m, c.esi);
    c.esi = f.r(c.ecx);
    c.eax = (c.edx << 5).wrapping_add(0x0078_b2e0);
    f.w(c.eax, c.esi);
    c.esi = f.r(c.ecx.wrapping_add(0xc));
    f.w(c.eax.wrapping_add(4), c.esi);
    c.esi = f.r(c.ecx.wrapping_add(0x10));
    f.w(c.eax.wrapping_add(8), c.esi);
    c.esi = f.r(c.ecx.wrapping_add(8));
    f.w(c.eax.wrapping_add(0xc), c.esi);
    c.ecx = f.r(c.ecx.wrapping_add(0x14));
    f.w(c.eax.wrapping_add(0x1c), c.ecx);
    c.ecx = 0;
    c.edx = c.edx.wrapping_add(1);
    f.w(c.eax.wrapping_add(0x10), c.ecx);
    f.w(c.eax.wrapping_add(0x14), c.ecx);
    f.store(c.eax.wrapping_add(0x18), 0);
    f.w(0x0078_b520, c.edx);
    c.esi = c.pop(f.m);
}

/// 002AD370 cdecl(1). 300 iteration pairwise loop over 12 byte records: clamp-to-zero and accumulate.
pub fn pairwise_scroll_loop<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.eax = c.at(f.m, 4);
    f.load(0, 0x0047_d610);
    f.zero(1);
    f.load(2, 0x0047_60cc);
    c.ecx = 0x12c;
    loop {
        f.load(3, c.eax);
        if !f.cmp(3, 0).be() {
            f.store(c.eax, 1);
        }
        f.load(3, c.eax);
        f.add_m(3, c.eax.wrapping_add(4));
        f.store(c.eax, 3);
        f.load(3, c.eax.wrapping_add(4));
        f.mul(3, 2);
        f.add_m(3, c.eax.wrapping_add(8));
        f.store(c.eax.wrapping_add(8), 3);
        c.eax = c.eax.wrapping_add(0xc);
        c.ecx -= 1;
        if c.ecx == 0 {
            break;
        }
    }
}

/// 002E5900 cdecl(4). Table-driven pair of sine-like lookups at 7A38A0 written as three floats at the out pointer.
pub fn table_lookup_triple<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.push(f.m, c.esi);
    f.load(3, 0x0047_5c78);
    f.load(2, 0x0047_5cd4);
    f.load(5, 0x0047_5ce0);
    f.load(4, c.esp.wrapping_add(0xc));
    f.movaps(0, 3);
    f.x[0][0] = sse::subss(f.x[0][0], c.at(f.m, 0x10));
    f.sub(0, 2);
    f.mul_m(0, 0x0049_f6d4);
    f.mul(0, 5);
    f.add(0, 2);
    c.eax = sse::cvttss2si(f.x[0][0]);
    c.ecx = c.eax.wrapping_add(0x200) & 0x7ff;
    f.load(0, c.ecx.wrapping_mul(4).wrapping_add(0x007a_38a0));
    f.mul_m(0, 0x0047_5d18);
    c.ecx = c.ecx.wrapping_mul(4).wrapping_add(0x007a_38a0);
    f.store(c.esp.wrapping_add(0xc), 0);
    c.edx = c.at(f.m, 0xc);
    f.movaps(1, 3);
    f.sub(1, 4);
    f.sub(1, 2);
    f.mul_m(1, 0x0049_f6d0);
    f.mul(1, 5);
    f.add(1, 2);
    c.edx &= 0x7fff_ffff;
    f.w(c.esp.wrapping_add(0xc), c.edx);
    c.edx = sse::cvttss2si(f.x[1][0]);
    f.load(1, c.esp.wrapping_add(0xc));
    f.add(1, 3);
    c.esi = c.edx & 0x7ff;
    f.mul_m(1, c.esi.wrapping_mul(4).wrapping_add(0x007a_38a0));
    c.esi = c.at(f.m, 8);
    f.movaps(0, 3);
    f.x[0][0] = sse::subss(f.x[0][0], c.at(f.m, 0x14));
    f.mul(1, 0);
    f.mul_m(0, 0x0047_d6d4);
    f.store(c.esi, 1);
    f.sub(4, 2);
    f.movaps(1, 4);
    f.mul(1, 4);
    f.add(1, 3);
    c.eax &= 0x7ff;
    f.mul_m(1, c.eax.wrapping_mul(4).wrapping_add(0x007a_38a0));
    f.mul_m(1, 0x0047_6040);
    f.store(c.esi.wrapping_add(4), 1);
    c.edx = c.edx.wrapping_add(0x200) & 0x7ff;
    f.load(1, c.edx.wrapping_mul(4).wrapping_add(0x007a_38a0));
    f.add_m(1, c.ecx);
    f.mul(1, 2);
    f.mul(1, 0);
    f.store(c.esi.wrapping_add(8), 1);
    c.esi = c.pop(f.m);
}

/// 00347B80 cdecl(0). Clears the 0x160 dword table at 767100 (`rep stosd`), count 7670F8 = 0, [7670FC] = [4760D0].
pub fn clear_entry_table<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    f.load(0, 0x0047_60d0);
    c.push(f.m, c.edi);
    c.ecx = 0x160;
    c.eax = 0;
    c.edi = 0x0076_7100;
    c.rep_stosd(f.m);
    f.w(0x0076_70f8, 0);
    f.store(0x0076_70fc, 0);
    c.edi = c.pop(f.m);
}

/// 00347D70 cdecl(3). Appends a 0x2C byte record (count 7670F8, table 767100, max 0x20), zero filled, with value, arg0 and source+2C.
pub fn append_record_0x2c<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.push(f.m, c.ebx);
    c.ebx = f.r(0x0076_70f8);
    if (c.ebx as i32) >= 0x20 {
        c.ebx = c.pop(f.m);
        return;
    }
    c.push(f.m, c.ebp);
    c.ebp = c.at(f.m, 0x14);
    if c.ebp == 0 {
        c.ebp = c.pop(f.m);
        c.ebx = c.pop(f.m);
        return;
    }
    f.x[0] = [c.at(f.m, 0x10), 0, 0, 0];
    c.edx = c.ebx.wrapping_mul(0x2c);
    c.push(f.m, c.esi);
    c.push(f.m, c.edi);
    c.esi = c.edx.wrapping_add(0x0076_7100);
    c.eax = 0;
    c.edi = c.esi;
    c.ecx = 0xb;
    c.rep_stosd(f.m);
    c.eax = c.at(f.m, 0x14);
    f.w(c.esi.wrapping_add(0x24), c.eax);
    f.store(c.esi.wrapping_add(0x20), 0);
    f.w(c.esi.wrapping_add(0x28), 0);
    c.ecx = f.r(c.ebp.wrapping_add(0x2c));
    c.ebx = c.ebx.wrapping_add(1);
    c.edi = c.pop(f.m);
    f.w(c.esi, c.ecx);
    f.w(0x0076_70f8, c.ebx);
    c.esi = c.pop(f.m);
    c.ebp = c.pop(f.m);
    c.ebx = c.pop(f.m);
}

/// 00349570 cdecl(0), ECX input unused. Clears globals and initialises 0x778840.. in 0x68 byte steps (26 dwords each, `rep stosd`).
pub fn init_entry_array<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    f.x[0] = [0; 4];
    c.eax = 0;
    f.w(0x0076_7694, 0);
    f.w(0x0076_76b4, 0);
    f.store(0x0076_7684, 0);
    f.store(0x0076_7688, 0);
    f.store(0x0076_768c, 0);
    c.edx = 0x0077_8840;
    c.push(f.m, c.edi);
    loop {
        c.eax = 0;
        c.edi = c.edx.wrapping_sub(0x40);
        c.ecx = 0x1a;
        c.rep_stosd(f.m);
        f.w(c.edx, 2);
        f.w(c.edx.wrapping_add(4), 0xffff_ffff);
        c.edx = c.edx.wrapping_add(0x68);
        if (c.edx as i32) >= 0x0077_89e0 {
            break;
        }
    }
    c.edi = c.pop(f.m);
}

/// 00357DD0 cdecl(4) (owner, record, tag, source). Binds the record to the owner and copies the vec3 to +1C, mirrors to the linked record at +A8.
pub fn bind_record_to_owner<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    c.eax = c.at(f.m, 8);
    if c.eax == 0 {
        return;
    }
    c.ecx = c.at(f.m, 4);
    f.x[0] = [0; 4];
    f.w(c.eax.wrapping_add(0x158), c.ecx);
    let flags = f.r(c.ecx.wrapping_add(0x28)) | 0x1_0000;
    f.w(c.ecx.wrapping_add(0x28), flags);
    c.ecx = c.at(f.m, 0xc);
    f.w(c.eax.wrapping_add(0x15c), c.ecx);
    c.ecx = c.at(f.m, 0x10);
    f.store(c.eax.wrapping_add(0x64), 0);
    f.store(c.eax.wrapping_add(0x68), 0);
    f.store(c.eax.wrapping_add(0x6c), 0);
    c.edx = f.r(c.ecx);
    f.w(c.eax.wrapping_add(0x1c), c.edx);
    c.edx = f.r(c.ecx.wrapping_add(4));
    f.w(c.eax.wrapping_add(0x20), c.edx);
    c.ecx = f.r(c.ecx.wrapping_add(8));
    f.w(c.eax.wrapping_add(0x24), c.ecx);
    c.ecx = f.r(c.eax.wrapping_add(0xa8));
    if c.ecx == 0 {
        return;
    }
    c.edx = f.r(c.eax.wrapping_add(0x1c));
    f.w(c.ecx.wrapping_add(0x64), c.edx);
    c.ecx = f.r(c.eax.wrapping_add(0xa8));
    c.edx = f.r(c.eax.wrapping_add(0x20));
    f.w(c.ecx.wrapping_add(0x68), c.edx);
    c.ecx = f.r(c.eax.wrapping_add(0xa8));
    c.edx = f.r(c.eax.wrapping_add(0x24));
    f.w(c.ecx.wrapping_add(0x6c), c.edx);
}

/// 00362CC0 cdecl(0). Copies the 251 word table at 546304 to 7738A0, clears three words, sets 76C0B4 = 1 and 77388C = 0.
/// ESI and EDI are stored below `esp` (no push): the slots at entry-4 and entry-8.
pub fn load_table_546304<M: GuestMemory>(f: &mut Fp<M>, c: &mut Cpu) {
    let entry = c.esp;
    f.x[0] = [0; 4];
    f.w(entry.wrapping_sub(4), c.esi);
    f.w(entry.wrapping_sub(8), c.edi);
    for index in 0..0xfbu32 {
        let word = f.r(0x0054_6304u32.wrapping_add(4 * index));
        f.w(0x0077_38a0u32.wrapping_add(4 * index), word);
    }
    c.ecx = 0;
    c.eax = 0;
    f.w(0x0077_36e0, 0);
    f.w(0x0077_36e4, 0);
    f.w(0x0077_3700, 0);
    f.w(0x0076_c0b4, 1);
    f.store(0x0077_388c, 0);
}
