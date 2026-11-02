// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-ui/hud/glyph
//! T3768: lane adapter for 0x3FAA0 `game_hud_text_glyph_metrics_from_char` (the readable port is
//! `tsfp-ui/src/hud/glyph.rs`, about 35M entries over the owner recordings). cdecl(1): the text
//! pointer at `[esp+4]`, the advance in EAX, no callee cleanup (lanes.json row from the lift).
//!
//! The capture compare checks EDX too (crates/tsfp-t2-offline/src/compare.rs), so the adapter also
//! leaves the EDX of the original. [`exit_edx`] follows the register flow of the lifted body
//! (`sub_0003FAA0`) exit by exit; every other register is preserved or scratch (ECX, XMM0).
use crate::hud::glyph::glyph_metric;
use tsfp_port_runtime::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

const CURRENT_FONT: u32 = 0x006b_93b8;
const ACCENT_BASE_TABLE: u32 = 0x004c_9040;

pub const ROOTS: [u32; 1] = [0x3faa0];

/// Runs a root; `false` when `va` is not one of this module's.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    _guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x3faa0 => {
            let text = m.read32(regs.esp.wrapping_add(4));
            regs.eax = glyph_metric(m, text) as u32;
            regs.edx = exit_edx(m, text);
            true
        }
        _ => false,
    }
}

/// Glyph index of an accented byte (sign-extended base table entry), else the byte itself.
fn glyph_code<M: GuestMemory>(m: &M, byte: u8) -> u32 {
    if byte & 0x80 != 0 {
        m.read8(ACCENT_BASE_TABLE + u32::from(byte & 0x7f)) as i8 as i32 as u32
    } else {
        u32::from(byte)
    }
}

/// EDX of the original at RET. It holds, in order: the text pointer with its low byte replaced by
/// the first character, then `glyph * 3`, then the kerning row of the glyph entry, then the
/// kerning matrix index `row * 13 + column` (the exits are the early returns of the port).
pub fn exit_edx<M: GuestMemory>(m: &M, text: u32) -> u32 {
    let first = m.read8(text);
    let entry = (text & 0xffff_ff00) | u32::from(first);
    let glyph = match first {
        0xc7 => 0x23,
        0xe7 => 0x43,
        0x7c => 0,
        _ => {
            let glyph = glyph_code(m, first).wrapping_sub(0x20);
            if glyph >= 0x60 {
                return entry;
            }
            glyph
        }
    };
    let glyph_table = m.read32(m.read32(CURRENT_FONT).wrapping_add(0x10));
    if glyph_table == 0 {
        return entry;
    }
    if m.read32(m.read32(CURRENT_FONT).wrapping_add(0x14)) == 0 {
        return glyph.wrapping_mul(3);
    }
    let row = m.read32(
        glyph_table
            .wrapping_add(glyph.wrapping_mul(12))
            .wrapping_add(4),
    );
    let second = m.read8(text.wrapping_add(1));
    if second == 0 || second == 0x0a {
        return row;
    }
    let next = glyph_code(m, second).wrapping_sub(0x20);
    if next >= 0x60 {
        return row;
    }
    row.wrapping_mul(0xd)
        .wrapping_add(m.read32(glyph_table.wrapping_add(next.wrapping_mul(12))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tsfp_port_runtime::abi::{Fpu, Xmm};

    fn registers() -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x27f,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        }
    }

    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);
    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    struct NoCalls;
    impl GuestCall for NoCalls {
        fn call(&mut self, target: u32, _regs: &mut ExtRegisters) {
            panic!("0x3FAA0 makes no calls, got {target:#x}");
        }
    }

    const FONT: u32 = 0x10_0000;
    const FONT_OBJECT: u32 = 0x11_0000;
    const TABLE: u32 = 0x12_0000;
    const KERNING: u32 = 0x13_0000;
    const TEXT: u32 = 0x14_0040;
    const STACK: u32 = 0x20_0000;

    /// A font with a glyph table and a kerning matrix; `text` holds `bytes`.
    fn font(bytes: &[u8], table: u32, kerning: u32) -> Memory {
        let mut m = Memory::default();
        m.write32(CURRENT_FONT, FONT);
        m.write32(0x006b_93bc, FONT_OBJECT);
        m.write32(FONT_OBJECT + 0x10, 2);
        m.write32(FONT + 0x10, table);
        m.write32(FONT + 0x14, kerning);
        for glyph in 0..0x60u32 {
            let entry = TABLE + glyph * 12;
            m.write32(entry, glyph % 13);
            m.write32(entry + 4, glyph % 7);
            m.write32(entry + 8, 10 + glyph);
        }
        for index in 0..0xa9u32 {
            m.write32(KERNING + index * 4, index);
        }
        for (offset, byte) in bytes.iter().enumerate() {
            m.write8(TEXT + offset as u32, *byte);
        }
        m
    }

    fn run_root(m: &mut Memory) -> ExtRegisters {
        let mut regs = ExtRegisters {
            esp: STACK,
            edx: 0xdead_beef,
            ..registers()
        };
        m.write32(STACK + 4, TEXT);
        assert!(run(0x3faa0, m, &mut NoCalls, &mut regs));
        regs
    }

    #[test]
    fn edx_follows_each_exit() {
        let entry = (TEXT & 0xffff_ff00) | u32::from(b'A');
        // No glyph table: the entry value with the character in the low byte.
        assert_eq!(run_root(&mut font(b"AB", 0, KERNING)).edx, entry);
        // No kerning matrix: glyph * 3 from the advance lookup.
        assert_eq!(
            run_root(&mut font(b"AB", TABLE, 0)).edx,
            (u32::from(b'A') - 0x20) * 3
        );
        let glyph = u32::from(b'A') - 0x20;
        let row = glyph % 7;
        assert_eq!(run_root(&mut font(b"A\0", TABLE, KERNING)).edx, row);
        assert_eq!(run_root(&mut font(b"A\n", TABLE, KERNING)).edx, row);
        let next = u32::from(b'B') - 0x20;
        let regs = run_root(&mut font(b"AB", TABLE, KERNING));
        assert_eq!(regs.edx, row * 13 + next % 13);
        let mut m = font(b"AB", TABLE, KERNING);
        assert_eq!(regs.eax, glyph_metric(&m, TEXT) as u32);
        assert_eq!(regs.eax, (2 + 10 + glyph + row * 13 + next % 13) as u32);
        // ESP and the callee-saved registers stay as entered.
        let again = run_root(&mut m);
        assert_eq!(
            (again.esp, again.ebx, again.esi, again.edi, again.ebp),
            (STACK, 0, 0, 0, 0)
        );
    }

    #[test]
    fn special_characters_and_out_of_range() {
        // '|' maps to glyph 0, 0xC7 / 0xE7 to 0x23 / 0x43 (no accent table lookup for the index).
        assert_eq!(run_root(&mut font(b"|x", TABLE, 0)).edx, 0);
        assert_eq!(run_root(&mut font(&[0xc7, 0], TABLE, 0)).edx, 0x23 * 3);
        assert_eq!(run_root(&mut font(&[0xe7, 0], TABLE, 0)).edx, 0x43 * 3);
        // An accented byte whose base entry is 0 gives glyph 0xFFFFFFE0 (out of range): entry exit.
        let byte = 0x85u8;
        let entry = (TEXT & 0xffff_ff00) | u32::from(byte);
        assert_eq!(run_root(&mut font(&[byte, 0], TABLE, KERNING)).edx, entry);
        // Second character out of range keeps the row.
        let row = (u32::from(b'A') - 0x20) % 7;
        assert_eq!(run_root(&mut font(&[b'A', 0x85], TABLE, KERNING)).edx, row);
    }

    #[test]
    fn other_roots_are_refused() {
        let mut m = font(b"A", TABLE, KERNING);
        let mut regs = registers();
        assert!(!run(0x3fab0, &mut m, &mut NoCalls, &mut regs));
        assert_eq!(regs, registers());
    }
}
