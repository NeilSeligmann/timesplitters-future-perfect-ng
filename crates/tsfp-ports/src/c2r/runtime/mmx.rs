// SPDX-License-Identifier: GPL-3.0-or-later
//! Packed integer operations of the lifted MMX model. Values are little-endian lane bits;
//! guest contexts own the eight registers through GuestCall, independently of x87.
#![allow(dead_code)]

fn mask(width: u32) -> u64 {
    u64::MAX >> (64 - width)
}
fn lane(value: u64, width: u32, index: u32) -> u64 {
    (value >> (width * index)) & mask(width)
}
fn signed(value: u64, width: u32) -> i64 {
    ((value << (64 - width)) as i64) >> (64 - width)
}
fn map_lanes(a: u64, b: u64, width: u32, op: impl Fn(u64, u64) -> u64) -> u64 {
    let mut result = 0;
    for i in 0..64 / width {
        result |= (op(lane(a, width, i), lane(b, width, i)) & mask(width)) << (width * i);
    }
    result
}
macro_rules! arithmetic {
    ($add:ident, $sub:ident, $width:expr) => {
        pub fn $add(a: u64, b: u64) -> u64 {
            map_lanes(a, b, $width, u64::wrapping_add)
        }
        pub fn $sub(a: u64, b: u64) -> u64 {
            map_lanes(a, b, $width, u64::wrapping_sub)
        }
    };
}
arithmetic!(mmx_paddb, mmx_psubb, 8);
arithmetic!(mmx_paddw, mmx_psubw, 16);
arithmetic!(mmx_paddd, mmx_psubd, 32);
pub fn mmx_pand(a: u64, b: u64) -> u64 {
    a & b
}
pub fn mmx_pandn(a: u64, b: u64) -> u64 {
    !a & b
}
pub fn mmx_por(a: u64, b: u64) -> u64 {
    a | b
}
pub fn mmx_pxor(a: u64, b: u64) -> u64 {
    a ^ b
}
pub fn mmx_pmullw(a: u64, b: u64) -> u64 {
    map_lanes(a, b, 16, u64::wrapping_mul)
}
pub fn mmx_pmulhw(a: u64, b: u64) -> u64 {
    map_lanes(a, b, 16, |x, y| {
        ((signed(x, 16) * signed(y, 16)) >> 16) as u64
    })
}
pub fn mmx_pmaddwd(a: u64, b: u64) -> u64 {
    let mut result = 0;
    for i in 0..2 {
        let product = |j| signed(lane(a, 16, j), 16) * signed(lane(b, 16, j), 16);
        result |= ((product(i * 2) + product(i * 2 + 1)) as u32 as u64) << (i * 32);
    }
    result
}
macro_rules! average_equal {
    ($avg:ident, $eq:ident, $width:expr) => {
        pub fn $avg(a: u64, b: u64) -> u64 {
            map_lanes(a, b, $width, |x, y| (x + y + 1) >> 1)
        }
        pub fn $eq(a: u64, b: u64) -> u64 {
            map_lanes(a, b, $width, |x, y| if x == y { u64::MAX } else { 0 })
        }
    };
}
average_equal!(mmx_pavgb, mmx_pcmpeqb, 8);
average_equal!(mmx_pavgw, mmx_pcmpeqw, 16);
pub fn mmx_pcmpeqd(a: u64, b: u64) -> u64 {
    map_lanes(a, b, 32, |x, y| if x == y { u64::MAX } else { 0 })
}
fn shift(a: u64, count: u64, width: u32, left: bool, arithmetic: bool) -> u64 {
    map_lanes(a, 0, width, |x, _| {
        if arithmetic {
            (signed(x, width) >> count.min((width - 1) as u64)) as u64
        } else if count >= width as u64 {
            0
        } else if left {
            x << count
        } else {
            x >> count
        }
    })
}
macro_rules! shifts {
    ($left:ident, $right:ident, $width:expr) => {
        pub fn $left(a: u64, count: u64) -> u64 {
            shift(a, count, $width, true, false)
        }
        pub fn $right(a: u64, count: u64) -> u64 {
            shift(a, count, $width, false, false)
        }
    };
}
shifts!(mmx_psllw, mmx_psrlw, 16);
shifts!(mmx_pslld, mmx_psrld, 32);
shifts!(mmx_psllq, mmx_psrlq, 64);
pub fn mmx_psraw(a: u64, count: u64) -> u64 {
    shift(a, count, 16, false, true)
}
pub fn mmx_psrad(a: u64, count: u64) -> u64 {
    shift(a, count, 32, false, true)
}
fn unpack(a: u64, b: u64, width: u32, high: bool) -> u64 {
    let half = 32 / width;
    let start = if high { half } else { 0 };
    let mut result = 0;
    for i in 0..half {
        result |= lane(a, width, start + i) << (width * i * 2);
        result |= lane(b, width, start + i) << (width * (i * 2 + 1));
    }
    result
}
macro_rules! unpacks {
    ($low:ident, $high:ident, $width:expr) => {
        pub fn $low(a: u64, b: u64) -> u64 {
            unpack(a, b, $width, false)
        }
        pub fn $high(a: u64, b: u64) -> u64 {
            unpack(a, b, $width, true)
        }
    };
}
unpacks!(mmx_punpcklbw, mmx_punpckhbw, 8);
unpacks!(mmx_punpcklwd, mmx_punpckhwd, 16);
unpacks!(mmx_punpckldq, mmx_punpckhdq, 32);
fn pack(a: u64, b: u64, width: u32, unsigned: bool) -> u64 {
    let output_width = width / 2;
    let (lo, hi) = if unsigned {
        (0, mask(output_width) as i64)
    } else {
        (
            -(1i64 << (output_width - 1)),
            (1i64 << (output_width - 1)) - 1,
        )
    };
    let mut result = 0;
    for (half, value) in [a, b].into_iter().enumerate() {
        for i in 0..64 / width {
            let clamped =
                signed(lane(value, width, i), width).clamp(lo, hi) as u64 & mask(output_width);
            result |= clamped << (half as u32 * 32 + i * output_width);
        }
    }
    result
}
pub fn mmx_packsswb(a: u64, b: u64) -> u64 {
    pack(a, b, 16, false)
}
pub fn mmx_packuswb(a: u64, b: u64) -> u64 {
    pack(a, b, 16, true)
}
pub fn mmx_packssdw(a: u64, b: u64) -> u64 {
    pack(a, b, 32, false)
}
pub fn mmx_pshufw(a: u64, immediate: u64) -> u64 {
    let mut result = 0;
    for i in 0..4 {
        result |= lane(a, 16, ((immediate >> (i * 2)) & 3) as u32) << (i * 16);
    }
    result
}
