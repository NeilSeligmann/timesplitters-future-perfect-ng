// SPDX-License-Identifier: GPL-3.0-or-later
#![no_std]
#[cfg(test)]
extern crate std;

/// Register adapters for the two capture-diagnosed render effects roots.
use tsfp_port_runtime::{abi::ExtRegisters, memory::GuestMemory};

pub const ROOTS: [u32; 2] = [0x0015_3BD0, 0x0010_E410];

pub fn run<M: GuestMemory>(va: u32, m: &mut M, regs: &mut ExtRegisters) -> bool {
    match va {
        0x0015_3BD0 => {
            let seed = (m.read32(0x0074_97CC) as i32)
                .wrapping_mul(0x343FD)
                .wrapping_add(0x269EC3) as u32;
            m.write32(0x0074_97CC, seed);
            regs.eax = seed;
            let scale = f32::from_bits(u32::from_le_bytes([
                m.read8(0x0047_5CBC),
                m.read8(0x0047_5CBD),
                m.read8(0x0047_5CBE),
                m.read8(0x0047_5CBF),
            ]));
            regs.fpu
                .push(f64::from(((seed >> 16) & 0x7fff) as f32 * scale));
            true
        }
        0x0010_E410 => {
            let p = regs.edi;
            let sources = m.read32(p.wrapping_add(0xe0)) as i32;
            let early = (sources == 0 && m.read32(p.wrapping_add(0x88)) == 0)
                || m.read32(0x0079_094c) == 0x65;
            let mask = p.wrapping_add(0x7c);
            if early {
                m.write32(mask, m.read32(p.wrapping_add(0x78)));
                regs.eax = 1;
            } else {
                m.write32(mask, 0);
                if sources > 0 {
                    let mut players = m.read32(0x007b_0ce0) as i32;
                    let mut records = m.read32(0x007b_0ce4);
                    let mut src = 0i32;
                    while src < m.read32(p.wrapping_add(0xe0)) as i32 {
                        let candidates = m.read32(p.wrapping_add(0x78));
                        for player in 0..players {
                            let bit = 1u32.wrapping_shl(player as u32);
                            let offset = (player as u32).wrapping_mul(0xffc);
                            if candidates & bit != 0 {
                                let state =
                                    m.read32(records.wrapping_add(offset).wrapping_add(0x21c));
                                if (state as i32) < 0 && state & 1 != 0 {
                                    let ix = u32::from(
                                        m.read8(p.wrapping_add(0xe4).wrapping_add(src as u32)),
                                    )
                                    .wrapping_add(offset);
                                    if m.read8(records.wrapping_add(ix).wrapping_add(0xd2c)) != 0 {
                                        let old_mask = m.read32(mask);
                                        m.write32(mask, old_mask | bit);
                                        players = m.read32(0x007b_0ce0) as i32;
                                        records = m.read32(0x007b_0ce4);
                                    }
                                }
                            }
                        }
                        src += 1;
                    }
                }
                regs.eax = u32::from(m.read32(mask) != 0);
            }
            true
        }
        _ => false,
    }
}

/// Repair captured live returns after generated c2r execution.
pub fn repair_return<M: GuestMemory>(va: u32, m: &mut M, regs: &mut ExtRegisters) -> bool {
    match va {
        0x0015_3BD0 => {
            regs.eax = m.read32(0x0074_97CC);
            true
        }
        0x0010_E410 => {
            let p = regs.edi;
            let sources = m.read32(p.wrapping_add(0xe0)) as i32;
            let early = (sources == 0 && m.read32(p.wrapping_add(0x88)) == 0)
                || m.read32(0x0079_094c) == 0x65;
            // The original advances EDX through the per-source byte array in the outer loop.
            // Its caller-live value is the pointer just past the final source byte.
            if !early && sources > 0 {
                regs.edx = p.wrapping_add(0xe4).wrapping_add(sources as u32);
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Ram(BTreeMap<u32, u8>);

    impl GuestMemory for Ram {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[test]
    fn random_float_returns_lcg_increment_and_updated_seed() {
        let mut memory = Ram(BTreeMap::new());
        memory.write32(0x0074_97cc, 7);
        memory.write32(0x0047_5cbc, 1.0f32.to_bits());
        let mut regs = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x10000,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0x202,
            fpu: tsfp_port_runtime::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x37f,
            },
            xmm: [tsfp_port_runtime::abi::Xmm::default(); 5],
            xmm_hi: [tsfp_port_runtime::abi::Xmm::default(); 3],
        };

        assert!(run(0x0015_3bd0, &mut memory, &mut regs));

        let seed = 7u32.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        assert_eq!(regs.eax, seed);
        assert_eq!(regs.edx, 0);
        assert_eq!(memory.read32(0x0074_97cc), seed);
        assert_eq!(regs.fpu.top, 7);
    }

    #[test]
    fn visibility_return_preserves_live_source_cursor() {
        let mut memory = Ram(BTreeMap::new());
        let particle = 0x4427_5eb0u32;
        memory.write32(particle.wrapping_add(0xe0), 1);
        memory.write32(0x0079_094c, 0);
        let mut regs = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0x4428_cdb0,
            esp: 0x10000,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: particle,
            eflags: 0x202,
            fpu: tsfp_port_runtime::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x37f,
            },
            xmm: [tsfp_port_runtime::abi::Xmm::default(); 5],
            xmm_hi: [tsfp_port_runtime::abi::Xmm::default(); 3],
        };

        assert!(repair_return(0x0010_e410, &mut memory, &mut regs));
        assert_eq!(regs.edx, particle + 0xe5);
    }
}
