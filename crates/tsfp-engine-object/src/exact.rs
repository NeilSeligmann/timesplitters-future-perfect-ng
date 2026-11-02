// SPDX-License-Identifier: GPL-3.0-or-later
//! Call-free integer ports with ordered stack reads and exact volatile residue.
//! C-vs-Rust measurements are bounded; original equivalence is transitive.
use super::ObjectMemory;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registers {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
}

fn arg<M: ObjectMemory>(m: &M, esp: u32, index: u32) -> u32 {
    m.read32(esp.wrapping_add(4).wrapping_add(index.wrapping_mul(4)))
}

/// 00012BA0 cdecl(3)/u32: nullable base plus an unsigned 16-bit indexed field.
/// Flag0x10 chooses stride8/field2, otherwise stride16/field12. The null path
/// does not read argument2 or its index. Both branches preserve distinct ECX/EDX.
pub fn indexed_field_pointer<M: ObjectMemory>(m: &M, esp: u32, r: &mut Registers) {
    r.eax = arg(m, esp, 0);
    if m.read8(r.eax.wrapping_add(12)) & 0x10 != 0 {
        r.ecx = arg(m, esp, 1);
        r.eax = m.read32(r.ecx.wrapping_add(4));
        if r.eax != 0 {
            r.edx = arg(m, esp, 2);
            r.ecx = u32::from(m.read16(r.edx.wrapping_add(6)));
            r.eax = r.eax.wrapping_add(r.ecx.wrapping_mul(8)).wrapping_add(2);
        }
    } else {
        r.edx = arg(m, esp, 1);
        r.eax = m.read32(r.edx.wrapping_add(4));
        if r.eax != 0 {
            r.ecx = arg(m, esp, 2);
            r.edx = u32::from(m.read16(r.ecx.wrapping_add(6))) << 4;
            r.eax = r.eax.wrapping_add(r.edx).wrapping_add(12);
        }
    }
}
