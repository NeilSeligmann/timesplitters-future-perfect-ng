// SPDX-License-Identifier: GPL-3.0-or-later
//! T2061: `slerp` (0x150960) under the live game's x87 control word 0x3F (24 bit mantissa) rounds every x87 step to f32, the 0x27F goldens do not.
//! The expected bits come from an independent numpy float32 model of the lifted routine (rounding after each fadd / fsub / fmul / fsqrt, `fsin` and
//! `fpatan` in double), run on SYNTHETIC unit quaternions (seed 2061) chosen because the two precision modes differ in the last bits.
//! One test function: the precision switch is process wide.
use tsfp_engine_anim::blend::{set_single_precision, slerp};

fn q(bits: [u32; 4]) -> [f32; 4] {
    bits.map(f32::from_bits)
}
fn bits(v: [f32; 4]) -> [u32; 4] {
    v.map(f32::to_bits)
}

#[test]
fn slerp_follows_the_x87_precision_control() {
    let cases: [([u32; 4], [u32; 4], u32, [u32; 4], [u32; 4]); 3] = [
        ([0xbe607c0e, 0xbeac62f8, 0xbf1d6cfc, 0xbf2db51b], [0x3f3d0e77, 0x3ecdf5ed, 0xbe1bd6f2, 0x3f04ee7f], 0x3ef6df8c,
         [0xbf087d72, 0xbed5cf44, 0xbe8d92e3, 0xbf2e8c52], [0xbf087d72, 0xbed5cf46, 0xbe8d92e3, 0xbf2e8c53]),
        ([0xbe252469, 0x3f507714, 0xbef6df5d, 0x3e8f597b], [0x3ebbeab3, 0x3f548e11, 0xbed65fb1, 0xbcc84da8], 0x3f072548,
         [0x3dfc034a, 0x3f5d3ed6, 0xbef155d2, 0x3e009649], [0x3dfc034e, 0x3f5d3ed7, 0xbef155d3, 0x3e009648]),
        ([0xbe8a94fb, 0xbf2011ea, 0x3eb31a2f, 0xbf249971], [0x3e286e28, 0xbf536924, 0xbf09e3aa, 0x3ced5443], 0x3eeb6d8d,
         [0xbdaeff70, 0xbf6707fa, 0xbda44b98, 0xbed43ac1], [0xbdaeff6e, 0xbf6707f9, 0xbda44b9c, 0xbed43ac0]),
    ];
    for (p, r, t, single, wide) in cases {
        let t = f32::from_bits(t);
        set_single_precision(0x3f);
        assert_eq!(bits(slerp(q(p), q(r), t)), single, "24 bit control word");
        set_single_precision(0x27f);
        assert_eq!(bits(slerp(q(p), q(r), t)), wide, "53 bit control word");
    }
    set_single_precision(0x27f);
}
