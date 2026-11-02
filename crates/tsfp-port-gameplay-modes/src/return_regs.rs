// SPDX-License-Identifier: GPL-3.0-or-later
//! Register return helpers shared by the gameplay mode lane.

use crate::abi::ExtRegisters;

/// Apply a live EDX result while returning EAX through the existing lane result path.
#[inline]
pub fn apply_edx(registers: &mut ExtRegisters, eax: u32, edx: u32) -> u32 {
    registers.edx = edx;
    eax
}

/// Publish a live EDX result only when the original path performed the nested call.
#[inline]
pub fn apply_edx_if(
    registers: &mut ExtRegisters,
    eax: u32,
    edx: u32,
    applies: bool,
) -> Option<u32> {
    applies.then(|| apply_edx(registers, eax, edx))
}

#[cfg(test)]
mod tests {
    use super::{apply_edx, apply_edx_if};
    use crate::abi::ExtRegisters;

    #[test]
    fn applies_edx_and_returns_eax() {
        let mut registers = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: crate::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: Default::default(),
            xmm_hi: Default::default(),
        };
        assert_eq!(apply_edx(&mut registers, 0x1234, 0x65), 0x1234);
        assert_eq!(registers.edx, 0x65);
    }

    #[test]
    fn conditional_edx_publication_preserves_register_when_path_skips_call() {
        let mut registers = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0x99,
            esp: 0,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: crate::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: Default::default(),
            xmm_hi: Default::default(),
        };
        assert_eq!(apply_edx_if(&mut registers, 0x1234, 0x65, false), None);
        assert_eq!(registers.edx, 0x99);
        assert_eq!(
            apply_edx_if(&mut registers, 0x1234, 0x65, true),
            Some(0x1234)
        );
        assert_eq!(registers.edx, 0x65);
    }
}
