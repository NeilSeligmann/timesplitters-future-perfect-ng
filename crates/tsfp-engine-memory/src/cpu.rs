// SPDX-License-Identifier: GPL-3.0-or-later
// T1836 wave 11: register file and stack/string helpers of the synthesized-domain ports. This file
// is `include!`d by the `synth_leaves.rs` of each ports crate, after `use ... as Guest;` brings the
// crate's own guest-memory trait into scope. Like fp_stack::Cpu, `push` and `pop` use the real guest
// stack slot, so a record that overlaps a saved slot restores the clobbered register exactly as the
// C replacement does.

/// Registers a port may read or write. `esp` points at the return address on entry.
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
    /// Direction flag: `rep` string helpers step by -4 when set.
    pub df: bool,
}

impl Cpu {
    /// Order: eax, ecx, edx, esi, edi, ebx, ebp, esp, df (0 or 1).
    pub fn from_array(r: [u32; 9]) -> Cpu {
        Cpu { eax: r[0], ecx: r[1], edx: r[2], esi: r[3], edi: r[4], ebx: r[5], ebp: r[6], esp: r[7], df: r[8] != 0 }
    }

    pub fn to_array(&self) -> [u32; 9] {
        [self.eax, self.ecx, self.edx, self.esi, self.edi, self.ebx, self.ebp, self.esp, u32::from(self.df)]
    }
}

fn push<M: Guest>(m: &mut M, c: &mut Cpu, value: u32) {
    c.esp = c.esp.wrapping_sub(4);
    m.write32(c.esp, value);
}

fn pop<M: Guest>(m: &M, c: &mut Cpu) -> u32 {
    let value = m.read32(c.esp);
    c.esp = c.esp.wrapping_add(4);
    value
}

fn rep_stosd<M: Guest>(m: &mut M, c: &mut Cpu) {
    let step = if c.df { 4u32.wrapping_neg() } else { 4 };
    while c.ecx != 0 {
        m.write32(c.edi, c.eax);
        c.edi = c.edi.wrapping_add(step);
        c.ecx -= 1;
    }
}

fn rep_movsd<M: Guest>(m: &mut M, c: &mut Cpu) {
    let step = if c.df { 4u32.wrapping_neg() } else { 4 };
    while c.ecx != 0 {
        let value = m.read32(c.esi);
        m.write32(c.edi, value);
        c.esi = c.esi.wrapping_add(step);
        c.edi = c.edi.wrapping_add(step);
        c.ecx -= 1;
    }
}

fn write16<M: Guest>(m: &mut M, address: u32, value: u16) {
    let [low, high] = value.to_le_bytes();
    m.write8(address, low);
    m.write8(address.wrapping_add(1), high);
}
