// SPDX-License-Identifier: GPL-3.0-or-later
#![no_std]
#![allow(unexpected_cfgs)]
#![allow(
    clippy::all,
    dead_code,
    unused_assignments,
    unused_imports,
    unused_mut,
    unused_parens,
    unused_variables
)]

#[cfg(test)]
extern crate std;

pub mod abi {
    pub use tsfp_port_runtime::abi::*;
}
pub mod memory {
    pub use tsfp_port_runtime::memory::*;
}
pub mod inventory {
    pub mod sse {
        #[cfg(target_arch = "x86_64")]
        pub fn mulss(a: f32, b: f32) -> f32 {
            let mut r = a;
            unsafe {
                core::arch::asm!("mulss {r}, {b}", r=inout(xmm_reg) r, b=in(xmm_reg) b, options(pure,nomem,nostack))
            };
            r
        }
        #[cfg(target_arch = "x86_64")]
        pub fn addss(a: f32, b: f32) -> f32 {
            let mut r = a;
            unsafe {
                core::arch::asm!("addss {r}, {b}", r=inout(xmm_reg) r, b=in(xmm_reg) b, options(pure,nomem,nostack))
            };
            r
        }
        #[cfg(target_arch = "x86_64")]
        pub fn subss(a: f32, b: f32) -> f32 {
            let mut r = a;
            unsafe {
                core::arch::asm!("subss {r}, {b}", r=inout(xmm_reg) r, b=in(xmm_reg) b, options(pure,nomem,nostack))
            };
            r
        }
        #[cfg(target_arch = "x86_64")]
        pub fn divss(a: f32, b: f32) -> f32 {
            let mut r = a;
            unsafe {
                core::arch::asm!("divss {r}, {b}", r=inout(xmm_reg) r, b=in(xmm_reg) b, options(pure,nomem,nostack))
            };
            r
        }
        #[cfg(target_arch = "x86_64")]
        pub fn cvttss2si(a: f32) -> u32 {
            let r: u32;
            unsafe {
                core::arch::asm!("cvttss2si {r:e}, {a}", r=out(reg) r, a=in(xmm_reg) a, options(pure,nomem,nostack))
            };
            r
        }
        #[cfg(not(target_arch = "x86_64"))]
        pub fn mulss(a: f32, b: f32) -> f32 {
            a * b
        }
        #[cfg(not(target_arch = "x86_64"))]
        pub fn addss(a: f32, b: f32) -> f32 {
            a + b
        }
        #[cfg(not(target_arch = "x86_64"))]
        pub fn subss(a: f32, b: f32) -> f32 {
            a - b
        }
        #[cfg(not(target_arch = "x86_64"))]
        pub fn divss(a: f32, b: f32) -> f32 {
            a / b
        }
        #[cfg(not(target_arch = "x86_64"))]
        pub fn cvttss2si(a: f32) -> u32 {
            if a.is_nan() || a >= 2147483648.0 || a < -2147483648.0 {
                0x80000000
            } else {
                (a as i32) as u32
            }
        }
    }
}

pub mod system_slice {
    use crate::memory::GuestMemory;
    unsafe extern "C" {
        #[link_name = "sqrt"]
        fn c_sqrt(value: f64) -> f64;
        #[link_name = "sqrtf"]
        fn c_sqrtf(value: f32) -> f32;
        #[link_name = "frexp"]
        fn c_frexp(value: f64, exponent: *mut i32) -> f64;
        #[link_name = "ldexp"]
        fn c_ldexp(value: f64, exponent: i32) -> f64;
    }
    pub fn sqrt_double(value: f64) -> f64 {
        unsafe { c_sqrt(value) }
    }
    pub fn reciprocal_sqrt(value: f32) -> f32 {
        1.0 / unsafe { c_sqrtf(value) }
    }
    pub fn read_double<M: GuestMemory>(m: &M, address: u32) -> f64 {
        f64::from_bits(
            m.read32(address) as u64 | ((m.read32(address.wrapping_add(4)) as u64) << 32),
        )
    }
    pub fn round_pc(value: f64, control: u16) -> f64 {
        if control & 0x300 != 0 {
            return value;
        }
        let magnitude = value.abs();
        if (magnitude <= f32::MAX as f64 && magnitude >= f32::MIN_POSITIVE as f64)
            || magnitude == 0.0
            || !value.is_finite()
        {
            return value as f32 as f64;
        }
        let mut exponent = 0;
        let fraction = unsafe { c_frexp(value, &mut exponent) };
        unsafe { c_ldexp(fraction as f32 as f64, exponent) }
    }
    pub fn compare_fp(a: f64, b: f64) -> i32 {
        if a.is_nan() || b.is_nan() {
            2
        } else if a < b {
            -1
        } else if a > b {
            1
        } else {
            0
        }
    }
}

pub mod pose {
    use crate::memory::GuestMemory;
    pub fn write16<M: GuestMemory>(m: &mut M, address: u32, value: u16) {
        m.write8(address, value as u8);
        m.write8(address.wrapping_add(1), (value >> 8) as u8);
    }
    #[cfg(tsfp_cover)]
    pub fn cover(function: u32, from: u32, to: u32) {
        unsafe { tsfp_port_cover(function, from, to) }
    }
    #[cfg(tsfp_cover)]
    unsafe extern "C" {
        fn tsfp_port_cover(function: u32, from: u32, to: u32);
    }
}
