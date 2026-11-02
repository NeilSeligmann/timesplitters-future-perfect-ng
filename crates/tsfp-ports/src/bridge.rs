// SPDX-License-Identifier: GPL-3.0-or-later
// Exact guest-register residue belongs here, not in the game behaviour API.
use crate::{
    behaviour as b,
    memory::{GuestMemory, MappedGuest},
};
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registers {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub esp: u32,
}
fn arg<M: GuestMemory>(m: &M, r: &Registers, index: u32) -> u32 {
    m.read32(r.esp.wrapping_add(4).wrapping_add(index.wrapping_mul(4)))
}
pub fn execute<M: GuestMemory>(va: u32, m: &mut M, r: &mut Registers) {
    match va {
        0x12820 => {
            let object = arg(m, r, 0);
            let table = arg(m, r, 1);
            let key = arg(m, r, 2);
            r.eax = b::table_entry(m, object, table, key);
        }
        0x71c70 => {
            let object = arg(m, r, 0);
            let enable = arg(m, r, 1);
            r.ecx = b::set_enabled(m, object, enable);
            r.eax = object;
            r.edx = enable;
        }
        0xd2ce0 => {
            let object = arg(m, r, 0);
            r.ecx = b::request_state(m, object, r.esp.wrapping_add(8));
            r.eax = object;
        }
        0x1656d0 => r.eax = b::record_positive(m, arg(m, r, 0)),
        0x168100 => {
            let id = arg(m, r, 0);
            let bits = arg(m, r, 1);
            r.eax = b::record_set_flags(m, id, bits);
        }
        0x264740 => {
            r.ecx = m.read32(arg(m, r, 0).wrapping_add(0x7c));
            r.eax = b::select_pending(m, r.ecx);
        }
        0x243040 => {
            r.ecx = m.read32(arg(m, r, 0).wrapping_add(0x7c));
            r.eax = b::state_allowed(m, r.ecx);
        }
        0x228c20 => r.eax = b::code_weight(m, arg(m, r, 0)),
        0x22df10 => {
            let bit = arg(m, r, 1);
            r.ecx = arg(m, r, 0);
            r.eax = b::has_bit(m, r.ecx, bit);
        }
        0x23ef30 => {
            r.ecx = arg(m, r, 0);
            r.eax = 1u32.wrapping_shl(r.ecx) & m.read32(0x78cd54);
        }
        0x3bd2b0 => {
            let word = arg(m, r, 0) as u16;
            r.ecx = word.swap_bytes() as u32;
            r.edx = (word >> 8) as u32;
            r.eax = (r.eax & 0xffff0000) | r.ecx;
        }
        0x3bd2d0 => {
            let value = arg(m, r, 0);
            r.ecx = value.swap_bytes();
            r.edx = value >> 24;
            r.eax = r.ecx;
        }
        0x3bd300 => {
            let word = arg(m, r, 0) as u16;
            m.write8(r.esp.wrapping_add(4), (word >> 8) as u8);
            m.write8(r.esp.wrapping_add(5), word as u8);
            r.eax = (r.eax & 0xffff0000) | word.swap_bytes() as u32;
        }
        0x3c3cd0 => {
            let block = arg(m, r, 0);
            r.eax = arg(m, r, 1);
            if m.read32(block) != 0 {
                r.ecx = m.read32(block.wrapping_add(4));
                if r.ecx != 0 && (r.eax as i32) >= 8 {
                    r.eax = r.eax.wrapping_sub(8);
                }
            }
        }
        0x3c3d00 => r.eax = b::add_header(m, arg(m, r, 0), arg(m, r, 1)),
        0x3ea20 => {
            r.ecx = m.read32(0x6b7aa8);
            r.eax = 0;
            if r.ecx == 0 {
                m.write32(0x6b83c8, 0);
                m.write32(0x6b83cc, 0);
            }
        }
        0x942f0 => r.eax = crate::leaves::count_until_negative(m, arg(m, r, 0)),
        0x151090 => {
            let bits = arg(m, r, 0);
            r.eax = crate::leaves::integer::popcount(bits);
            r.ecx = crate::leaves::integer::popcount(bits & 0xffff);
        }
        0x31dcd0 => {
            r.ecx = arg(m, r, 0);
            r.eax = m.read32(r.ecx.wrapping_add(16));
            r.edx = m.read32(r.ecx.wrapping_add(12));
            r.eax = u32::from(r.eax != 0) * 4;
            r.edx = u32::from(r.edx != 0) * 2;
            r.eax |= r.edx;
            r.edx = m.read32(r.ecx.wrapping_add(4));
            r.edx = u32::from(r.edx != 0) * 8;
            r.eax |= r.edx;
            r.edx = m.read32(r.ecx.wrapping_add(8));
            r.ecx = (r.ecx & 0xffffff00) | u32::from(r.edx != 0);
            r.eax |= r.ecx & 255;
        }
        0x31dd10 => {
            r.eax = m.read8(r.esp.wrapping_add(8)) as u32;
            r.ecx = arg(m, r, 0);
            r.edx = r.eax & 1;
            m.write32(r.ecx.wrapping_add(8), r.edx);
            r.edx = (r.eax >> 1) & 1;
            m.write32(r.ecx.wrapping_add(12), r.edx);
            r.edx = (r.eax >> 2) & 1;
            r.eax &= 8;
            m.write32(r.ecx.wrapping_add(16), r.edx);
            m.write32(r.ecx.wrapping_add(4), r.eax);
        }
        0x3b9f00 => {
            r.edx = arg(m, r, 0);
            if r.edx == 0 {
                r.eax = arg(m, r, 1);
            } else {
                let byte = m.read8(r.edx);
                let negative = byte == b'-';
                if byte == b'+' || negative {
                    r.edx = r.edx.wrapping_add(1);
                }
                r.ecx = m.read8(r.edx) as u32;
                r.eax = 0;
                while (b'0'..=b'9').contains(&(r.ecx as u8)) {
                    r.ecx &= 15;
                    r.eax = crate::leaves::integer::decimal_step(r.eax, r.ecx);
                    r.edx = r.edx.wrapping_add(1);
                    r.ecx = (r.ecx & 0xffffff00) | m.read8(r.edx) as u32;
                }
                if negative {
                    r.eax = r.eax.wrapping_neg();
                }
            }
        }
        0x3bb710 => {
            r.eax = m.read32(r.ecx.wrapping_add(4));
            r.edx = crate::leaves::integer::align4(r.eax).wrapping_add(arg(m, r, 0));
            m.write32(r.ecx.wrapping_add(4), r.edx);
        }
        0x3bb930 => r.eax = crate::leaves::first_node_data(m, r.ecx),
        0x3bd320 => {
            let value = arg(m, r, 0).swap_bytes();
            m.write32(r.esp.wrapping_add(4), value);
            r.eax = value;
        }
        0x59d70 => {
            r.eax = arg(m, r, 0);
            r.edx = anim_leaf::record_key(m, r.eax);
            r.ecx = arg(m, r, 1);
            r.eax = u32::from(r.edx == r.ecx);
        }
        0xdc290 => r.eax = physics_leaf::entry_word(m, arg(m, r, 0)),
        _ => panic!("unregistered Rust port"),
    }
    #[cfg(rust_lane_mutation)]
    {
        r.eax ^= 1;
    }
}
macro_rules! export {
    ($name:ident, $va:expr) => {
        /// # Safety
        /// `registers` is a live C Registers object and offset maps the guest window.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(registers: *mut Registers, offset: usize) {
            let mut memory = unsafe { MappedGuest::new(offset) };
            execute($va, &mut memory, unsafe { &mut *registers });
        }
    };
}
export!(tsfp_port_00012820, 0x12820);
export!(tsfp_port_00071c70, 0x71c70);
export!(tsfp_port_000d2ce0, 0xd2ce0);
export!(tsfp_port_001656d0, 0x1656d0);
export!(tsfp_port_00168100, 0x168100);
export!(tsfp_port_00264740, 0x264740);
export!(tsfp_port_00243040, 0x243040);
export!(tsfp_port_00228c20, 0x228c20);
export!(tsfp_port_0022df10, 0x22df10);
export!(tsfp_port_0023ef30, 0x23ef30);
export!(tsfp_port_003bd2b0, 0x3bd2b0);
export!(tsfp_port_003bd2d0, 0x3bd2d0);
export!(tsfp_port_003bd300, 0x3bd300);
export!(tsfp_port_003c3cd0, 0x3c3cd0);
export!(tsfp_port_003c3d00, 0x3c3d00);

export!(tsfp_port_0003ea20, 0x3ea20);
export!(tsfp_port_000942f0, 0x942f0);
export!(tsfp_port_00151090, 0x151090);
export!(tsfp_port_0031dcd0, 0x31dcd0);
export!(tsfp_port_0031dd10, 0x31dd10);
export!(tsfp_port_003b9f00, 0x3b9f00);
export!(tsfp_port_003bb710, 0x3bb710);
export!(tsfp_port_003bb930, 0x3bb930);
export!(tsfp_port_003bd320, 0x3bd320);

export!(tsfp_port_00059d70, 0x59d70);
export!(tsfp_port_000dc290, 0xdc290);

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/leaf.rs"]
mod anim_leaf;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-physics/src/leaf.rs"]
mod physics_leaf;
