// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 8 batch B: more scalar-SSE leaves ported one-to-one from the proven C replacements
//! (`game_fp_math.c`, `game_t1477_appendix_fp.c`, `game_t1477_vector00[12].c`,
//! `game_t1479_vector_census.c`). Same conventions as [`crate::fp_scalar`]; see
//! docs/t1836-rust-wave8.md.
use crate::{fp_scalar::Fp, gprs::Gprs, memory::GuestMemory, sse};

fn gprs_of(eax: u32, ecx: u32, edx: u32) -> Gprs {
    Gprs { eax, ecx, edx }
}

fn load_mul<M: GuestMemory>(f: &mut Fp<M>, reg: usize, load: u32, mul: u32) {
    f.load(reg, load);
    f.mul_m(reg, mul);
}

fn arg_load<M: GuestMemory>(f: &mut Fp<M>, reg: usize, index: u32) {
    f.load(reg, f.esp.wrapping_add(4 + 4 * index));
}

/// Shared tail of 00152090 and 00152120: direction transform of the vec3 at `ecx` by the 3x3 part
/// of the matrix at `eax`, written in place.
fn direction_tail<M: GuestMemory>(f: &mut Fp<M>, eax: u32, ecx: u32) {
    let (a, c) = (eax, ecx);
    f.load(3, c.wrapping_add(4));
    f.load(4, c.wrapping_add(8));
    f.load(0, a.wrapping_add(0x24));
    f.load(1, a.wrapping_add(0x14));
    f.load(2, c);
    f.load(5, a.wrapping_add(0x18));
    f.mul(1, 3);
    f.mul(0, 4);
    f.add(0, 1);
    f.load(1, a.wrapping_add(4));
    f.mul(5, 3);
    f.mul(1, 2);
    f.add(0, 1);
    f.load(1, a.wrapping_add(0x28));
    f.mul(1, 4);
    f.add(1, 5);
    f.load(5, a.wrapping_add(8));
    f.mul(5, 2);
    f.add(1, 5);
    f.load(5, a.wrapping_add(0x20));
    f.mul(5, 4);
    f.load(4, a.wrapping_add(0x10));
    f.mul(4, 3);
    f.load(3, a);
    f.add(5, 4);
    f.mul(3, 2);
    f.add(5, 3);
    f.store(c, 5);
    f.store(c.wrapping_add(4), 0);
    f.store(c.wrapping_add(8), 1);
}

/// 00152120 cdecl(3) (matrix, src4, dest4). Copies four dwords src4 -> dest4, then the direction
/// transform on dest4. EAX = matrix, ECX = dest4, EDX = dest4.
pub fn matrix4x4_transform_direction_vec4_copy<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    let ecx = f.arg(2);
    let mut eax = f.arg(1);
    let edx = ecx;
    let mut value = f.r(eax);
    f.w(edx, value);
    value = f.r(eax.wrapping_add(4));
    f.w(edx.wrapping_add(4), value);
    value = f.r(eax.wrapping_add(8));
    f.w(edx.wrapping_add(8), value);
    eax = f.r(eax.wrapping_add(0xc));
    f.w(edx.wrapping_add(0xc), eax);
    eax = f.arg(0);
    direction_tail(f, eax, ecx);
    gprs_of(eax, ecx, edx)
}

/// 00150580 cdecl(3) (a, b, out). Quaternion product of the xyzw quads at a and b into out.
// portreg: va=0x00150580 tier=0
pub fn quat_multiply_to_out<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    let mut ecx = f.arg(1);
    let (a, b) = (eax, ecx);
    load_mul(f, 0, b, a.wrapping_add(0xc));
    load_mul(f, 1, a.wrapping_add(8), b.wrapping_add(4));
    load_mul(f, 2, a.wrapping_add(0xc), b.wrapping_add(4));
    load_mul(f, 3, b, a.wrapping_add(4));
    load_mul(f, 4, a.wrapping_add(8), b.wrapping_add(8));
    f.sub(0, 1);
    load_mul(f, 1, b.wrapping_add(8), a.wrapping_add(4));
    f.add(0, 1);
    load_mul(f, 1, a, b.wrapping_add(8));
    f.sub(2, 1);
    load_mul(f, 1, a.wrapping_add(8), b);
    f.add(2, 1);
    load_mul(f, 1, b.wrapping_add(8), a.wrapping_add(0xc));
    f.sub(1, 3);
    load_mul(f, 3, a, b.wrapping_add(4));
    f.add(1, 3);
    load_mul(f, 3, a, b);
    f.add(3, 4);
    load_mul(f, 4, a.wrapping_add(4), b.wrapping_add(4));
    ecx = f.arg(2);
    f.add(3, 4);
    f.load(4, a);
    f.mul(4, 3);
    f.movaps(5, 1);
    f.mul_m(5, a.wrapping_add(4));
    f.add(4, 5);
    f.movaps(5, 0);
    f.mul_m(5, a.wrapping_add(0xc));
    f.add(4, 5);
    f.load(5, a.wrapping_add(8));
    f.mul(5, 2);
    f.sub(4, 5);
    f.store(ecx, 4);
    f.load(4, a.wrapping_add(8));
    f.mul(4, 0);
    f.movaps(5, 3);
    f.mul_m(5, a.wrapping_add(4));
    f.add(4, 5);
    f.movaps(5, 2);
    f.mul_m(5, a.wrapping_add(0xc));
    f.add(4, 5);
    f.load(5, a);
    f.mul(5, 1);
    f.sub(4, 5);
    f.store(ecx.wrapping_add(4), 4);
    f.load(4, a.wrapping_add(8));
    f.mul_m(1, a.wrapping_add(0xc));
    f.mul_m(0, a.wrapping_add(4));
    f.mul(4, 3);
    f.load(3, a);
    f.mul(3, 2);
    f.add(4, 3);
    f.add(4, 1);
    f.sub(4, 0);
    f.store(ecx.wrapping_add(8), 4);
    gprs_of(eax, ecx, e.edx)
}

fn positive<M: GuestMemory>(f: &Fp<M>, value: usize, zero: usize) -> u32 {
    u32::from(!f.cmp(value, zero).be())
}

/// 00151190 cdecl(10) (f0..f9). Four 2D edge-function sign tests packed in bits 0..3 of EAX.
pub fn point_in_triangle_2d<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    arg_load(f, 2, 8);
    arg_load(f, 0, 1);
    arg_load(f, 3, 9);
    arg_load(f, 4, 2);
    arg_load(f, 6, 3);
    f.sub(4, 2);
    f.movaps(1, 0);
    f.sub(1, 3);
    f.mul(4, 1);
    arg_load(f, 1, 0);
    f.movaps(5, 1);
    f.sub(6, 3);
    f.sub(5, 2);
    f.mul(5, 6);
    f.zero(6);
    f.sub(4, 5);
    let mut ecx = positive(f, 4, 6);
    arg_load(f, 4, 7);
    arg_load(f, 5, 6);
    f.sub(2, 5);
    f.movaps(7, 0);
    f.sub(7, 4);
    f.mul(7, 2);
    f.movaps(2, 1);
    f.sub(2, 5);
    f.sub(3, 4);
    f.mul(2, 3);
    f.sub(7, 2);
    let mut eax = positive(f, 7, 6);
    arg_load(f, 2, 5);
    arg_load(f, 3, 4);
    f.sub(5, 3);
    f.movaps(7, 0);
    f.sub(7, 2);
    f.mul(7, 5);
    eax = eax.wrapping_add(eax);
    f.movaps(5, 1);
    f.sub(5, 3);
    f.sub(4, 2);
    f.mul(5, 4);
    ecx |= eax;
    f.sub(7, 5);
    eax = positive(f, 7, 6);
    arg_load(f, 5, 3);
    arg_load(f, 4, 2);
    f.sub(0, 5);
    let edx = eax.wrapping_mul(4);
    f.sub(3, 4);
    f.sub(1, 4);
    f.sub(2, 5);
    f.mul(0, 3);
    f.mul(1, 2);
    ecx |= edx;
    f.sub(0, 1);
    eax = positive(f, 0, 6);
    eax = (eax << 3) | ecx;
    gprs_of(eax, ecx, edx)
}

/// 001515E0 cdecl(3) (p, target, step). Moves *p towards target by step, EAX = arrived. The
/// stack slot esp+8 is used as scratch (written), like the original.
// portreg: va=0x001515e0 tier=1
pub fn float_approach_target_step<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let esp = f.esp;
    f.load(0, esp.wrapping_add(8));
    let mut eax = f.arg(0);
    f.movaps(1, 0);
    f.x[1][0] = sse::subss(f.x[1][0], f.r(eax));
    f.store(esp.wrapping_add(8), 1);
    let mut ecx = f.r(esp.wrapping_add(8));
    f.load(1, esp.wrapping_add(0xc));
    ecx &= 0x7fff_ffff;
    f.w(esp.wrapping_add(8), ecx);
    if f.cmp_m(1, esp.wrapping_add(8)).a() {
        f.store(eax, 0);
        eax = 1;
        return gprs_of(eax, ecx, e.edx);
    }
    let target_above = f.cmp_m(0, eax).a();
    f.load(0, eax);
    if target_above {
        f.add(0, 1);
    } else {
        f.sub(0, 1);
    }
    f.store(eax, 0);
    eax = 0;
    gprs_of(eax, ecx, e.edx)
}

/// 001553B0 cdecl(1). [74A990] = max([74A990], min(x, [475C78])) in the C comparison order.
pub fn global_74a990_raise_to_clamped_arg<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    f.load(0, f.esp.wrapping_add(4));
    f.load(1, 0x0047_5c78);
    if f.cmp(0, 1).a() {
        f.movaps(0, 1);
    }
    f.load(1, 0x0074_a990);
    if !f.cmp(1, 0).a() {
        f.store(0x0074_a990, 0);
    }
    e
}

/// 0012B8E0 cdecl(1). Stores the clamped scaled argument into field 0x28 of the previous ring entry.
/// EAX = entry address.
pub fn ring_742510_set_prev_entry_field_0x28_clamped<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let mut eax = f.r(0x0074_50d0);
    eax = eax.wrapping_sub(1);
    if eax >> 31 != 0 {
        eax = eax.wrapping_add(0xc8);
    }
    f.load(0, f.esp.wrapping_add(4));
    eax = eax.wrapping_mul(0x38);
    f.mul_m(0, 0x0047_5d24);
    f.load(2, 0x0047_5d1c);
    eax = eax.wrapping_add(0x0074_2510);
    let target = eax.wrapping_add(0x28);
    if !f.cmp(0, 2).a() {
        f.load(1, 0x0047_85d8);
        if !f.cmp(1, 0).be() {
            f.store(target, 1);
            return gprs_of(eax, e.ecx, e.edx);
        }
    }
    if f.cmp(0, 2).be() {
        f.store(target, 0);
    } else {
        f.store(target, 2);
    }
    gprs_of(eax, e.ecx, e.edx)
}

/// 0011FDC0 cdecl(3) (obj, ratio, extra). Sets the ratio at +98 when 0 < ratio <= [475C78]
/// (unordered excluded), else clears flags 0x20002.
pub fn effect_set_ratio_0x98_when_in_unit_range<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    f.load(0, f.esp.wrapping_add(8));
    f.zero(1);
    let mut in_range = false;
    if !f.cmp(0, 1).be() {
        f.load(2, 0x0047_5c78);
        in_range = !f.cmp(2, 0).b();
    }
    let mut ecx = f.arg(0);
    let mut edx = f.r(ecx.wrapping_add(4));
    let eax = ecx.wrapping_add(4);
    if in_range {
        edx |= 2;
        f.w(eax, edx);
        edx = f.r(f.esp.wrapping_add(0xc));
        f.w(ecx.wrapping_add(0x88), 0);
        f.store(ecx.wrapping_add(0x98), 0);
        f.w(ecx.wrapping_add(0x8c), edx);
    } else {
        edx &= 0xfffd_fffd;
        f.w(eax, edx);
        f.store(ecx.wrapping_add(0x98), 1);
        f.w(ecx.wrapping_add(0x8c), 0);
    }
    ecx = f.r(eax);
    if ecx & 2 != 0 {
        ecx |= 0x2_0000;
        f.w(eax, ecx);
    }
    gprs_of(eax, ecx, edx)
}

/// 00309160 cdecl(2) (dx, dy). Accumulates two float pairs in the 7838xx globals.
pub fn accumulate_scroll_globals<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    f.load(0, 0x0078_3dd8);
    f.x[0][0] = sse::addss(f.x[0][0], f.arg(0));
    f.load(1, 0x0078_3cc0);
    f.load(2, 0x0078_3de0);
    f.x[1][0] = sse::addss(f.x[1][0], f.arg(1));
    f.add(2, 0);
    f.store(0x0078_3dd8, 0);
    f.load(0, 0x0078_3ddc);
    f.add(0, 1);
    f.store(0x0078_3cc0, 1);
    f.store(0x0078_3dc0, 2);
    f.store(0x0078_3dcc, 0);
    e
}

/// 00319440 cdecl(2) (p, q). *p = min(*p, [4760DC]) (ja form), *q = max-style clamp against [4760CC].
pub fn clamp_pair_to_limits<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let ecx = f.arg(0);
    f.load(0, ecx);
    let eax = f.arg(1);
    f.load(2, 0x0047_60dc);
    let above = f.cmp(0, 2).a();
    f.load(1, eax);
    if above {
        f.movaps(0, 2);
    }
    f.store(ecx, 0);
    f.load(0, 0x0047_60cc);
    let edx = e.edx;
    if f.cmp(0, 1).a() {
        f.store(eax, 0);
    } else {
        f.store(eax, 1);
    }
    gprs_of(eax, ecx, edx)
}

/// 0036E920 cdecl(3) (obj, a, b). Writes four sums of the +20..+2C floats with a and b to +0..+C.
pub fn add_pairs_20_to_0<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    f.load(0, f.esp.wrapping_add(8));
    f.load(1, eax.wrapping_add(0x20));
    f.add(1, 0);
    f.store(eax, 1);
    f.load(1, eax.wrapping_add(0x28));
    f.add(1, 0);
    f.load(0, f.esp.wrapping_add(0xc));
    f.store(eax.wrapping_add(8), 1);
    f.load(1, eax.wrapping_add(0x24));
    f.add(1, 0);
    f.store(eax.wrapping_add(4), 1);
    f.load(1, eax.wrapping_add(0x2c));
    f.add(1, 0);
    f.store(eax.wrapping_add(0xc), 1);
    gprs_of(eax, e.ecx, e.edx)
}

/// 0036EC40 cdecl(3) (obj, a, b). Writes four sums of the +10..+1C floats with a and b to +0..+C.
pub fn add_pairs_10_to_0<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    f.load(0, f.esp.wrapping_add(8));
    f.load(1, eax.wrapping_add(0x10));
    f.load(2, eax.wrapping_add(0x14));
    f.add(1, 0);
    f.store(eax, 1);
    f.load(1, f.esp.wrapping_add(0xc));
    f.add(2, 1);
    f.store(eax.wrapping_add(4), 2);
    f.load(2, eax.wrapping_add(0x18));
    f.add(2, 0);
    f.load(0, eax.wrapping_add(0x1c));
    f.add(0, 1);
    f.store(eax.wrapping_add(8), 2);
    f.store(eax.wrapping_add(0xc), 0);
    gprs_of(eax, e.ecx, e.edx)
}

/// 00377560 cdecl(5) (obj, a, b, c, d). Converts four floats to ints (cvttss2si) and stores raw floats.
pub fn store_converted_float_quad<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    let eax = f.arg(0);
    f.load(1, f.esp.wrapping_add(0x10));
    f.x[3] = {
        let mut x3 = f.x[3];
        x3[0] = sse::cvtsi2ss(f.r(eax.wrapping_add(0x84)));
        x3
    };
    f.load(0, f.esp.wrapping_add(8));
    f.load(2, f.esp.wrapping_add(0xc));
    f.movaps(4, 1);
    f.sub(4, 3);
    let mut ecx = sse::cvttss2si(f.x[4][0]);
    f.store(eax.wrapping_add(0x28), 0);
    f.load(0, f.esp.wrapping_add(0x14));
    f.w(eax, ecx);
    ecx = sse::cvttss2si(f.x[1][0]);
    f.store(eax.wrapping_add(0x30), 1);
    f.x[1][0] = sse::cvtsi2ss(f.r(eax.wrapping_add(0x20)));
    let mut edx = sse::cvttss2si(f.x[2][0]);
    f.store(eax.wrapping_add(0x34), 0);
    f.w(eax.wrapping_add(4), edx);
    f.sub(0, 1);
    edx = sse::cvttss2si(f.x[0][0]);
    f.store(eax.wrapping_add(0x2c), 2);
    f.w(eax.wrapping_add(8), ecx);
    f.w(eax.wrapping_add(0xc), edx);
    gprs_of(eax, ecx, edx)
}

/// 00307E60 cdecl(3) (x, y, z). Truncates three floats to a grid index and loads the table cell.
pub fn float_grid_cell_lookup<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    let mut eax = sse::cvttss2si(f.arg(2));
    let mut ecx = sse::cvttss2si(f.arg(0));
    let mut edx = sse::cvttss2si(f.arg(1));
    eax = eax.wrapping_add(2);
    let miss = |ecx: u32, edx: u32| gprs_of(0, ecx, edx);
    if (ecx as i32) < 0 || (ecx as i32) >= 0x28 || (edx as i32) < 0 || (edx as i32) >= 0x28 {
        return miss(ecx, edx);
    }
    if (eax as i32) < 0 || (eax as i32) >= 6 {
        return miss(ecx, edx);
    }
    eax = eax.wrapping_add(eax.wrapping_mul(4));
    eax = edx.wrapping_add(eax.wrapping_mul(8));
    edx = eax.wrapping_add(eax.wrapping_mul(4));
    eax = ecx.wrapping_add(edx.wrapping_mul(8));
    if (eax as i32) < 0 || (eax as i32) >= 0x2580 {
        return miss(ecx, edx);
    }
    ecx = f.r(0x0078_4004);
    eax = f.r(ecx.wrapping_add(eax.wrapping_mul(4)));
    gprs_of(eax, ecx, edx)
}
