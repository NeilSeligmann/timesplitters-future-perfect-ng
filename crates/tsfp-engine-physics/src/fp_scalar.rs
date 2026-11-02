// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 8 scalar-SSE leaves ported one-to-one from the proven C replacements in
//! `src/game/game_fp_list001.c`, `game_fp_list002.c` and `game_fp_step2b.c`, see
//! docs/t1836-rust-wave8.md. The crate is chosen for the shared XMM model only, the coverage tree
//! of these rows stays `unknown` (no placement is invented, INFERRED name rule not applied).
//! [`Fp`] mirrors the C helpers (`fp_load_ss`, `fp_mulss_reg`, ...) so a port reads like its C
//! source, and every operation order, aliasing store and untouched register is preserved.
use crate::{
    gprs::Gprs,
    memory::GuestMemory,
    sse::{self, Xmm},
};

/// Guest memory, the XMM file and the entry `esp` (return address at `esp`).
pub struct Fp<'a, M: GuestMemory> {
    pub m: &'a mut M,
    pub x: &'a mut [Xmm; 8],
    pub esp: u32,
}

impl<M: GuestMemory> Fp<'_, M> {
    pub fn arg(&self, index: u32) -> u32 {
        self.m.read32(self.esp.wrapping_add(4 + 4 * index))
    }
    pub fn r(&self, address: u32) -> u32 {
        self.m.read32(address)
    }
    pub fn w(&mut self, address: u32, value: u32) {
        self.m.write32(address, value);
    }
    /// movss xmm, [address]: lane 0 loaded, lanes 1..3 zeroed.
    pub fn load(&mut self, reg: usize, address: u32) {
        self.x[reg] = [self.m.read32(address), 0, 0, 0];
    }
    /// movss [address], xmm.
    pub fn store(&mut self, address: u32, reg: usize) {
        self.m.write32(address, self.x[reg][0]);
    }
    /// xorps xmm, xmm.
    pub fn zero(&mut self, reg: usize) {
        self.x[reg] = [0; 4];
    }
    /// movaps dest, source (all 128 bits).
    pub fn movaps(&mut self, dest: usize, source: usize) {
        self.x[dest] = self.x[source];
    }
    pub fn add(&mut self, dest: usize, source: usize) {
        self.x[dest][0] = sse::addss(self.x[dest][0], self.x[source][0]);
    }
    pub fn sub(&mut self, dest: usize, source: usize) {
        self.x[dest][0] = sse::subss(self.x[dest][0], self.x[source][0]);
    }
    pub fn mul(&mut self, dest: usize, source: usize) {
        self.x[dest][0] = sse::mulss(self.x[dest][0], self.x[source][0]);
    }
    pub fn div(&mut self, dest: usize, source: usize) {
        self.x[dest][0] = sse::divss(self.x[dest][0], self.x[source][0]);
    }
    pub fn add_m(&mut self, dest: usize, address: u32) {
        self.x[dest][0] = sse::addss(self.x[dest][0], self.m.read32(address));
    }
    pub fn sub_m(&mut self, dest: usize, address: u32) {
        self.x[dest][0] = sse::subss(self.x[dest][0], self.m.read32(address));
    }
    pub fn mul_m(&mut self, dest: usize, address: u32) {
        self.x[dest][0] = sse::mulss(self.x[dest][0], self.m.read32(address));
    }
    /// comiss/ucomiss xmm_a, xmm_b flags.
    pub fn cmp(&self, a: usize, b: usize) -> sse::Flags {
        sse::comiss(self.x[a][0], self.x[b][0])
    }
    /// comiss xmm_a, [address] flags.
    pub fn cmp_m(&self, a: usize, address: u32) -> sse::Flags {
        sse::comiss(self.x[a][0], self.m.read32(address))
    }
}

fn gprs_of(eax: u32, ecx: u32, edx: u32) -> Gprs {
    Gprs { eax, ecx, edx }
}

/// 00046280 cdecl(1). EAX = value, [4D184C] = [4D1850] = value, [4D183C] = [4D1848] * [4D1840].
// portreg: va=0x00046280 tier=1
pub fn store_arg_and_product<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    f.load(0, 0x004d_1848);
    f.mul_m(0, 0x004d_1840);
    f.w(0x004d_184c, eax);
    f.w(0x004d_1850, eax);
    f.store(0x004d_183c, 0);
    gprs_of(eax, e.ecx, e.edx)
}

/// 0005A150 cdecl(1). EAX = 0 when [obj+198] is ordered equal to [475CAC], else 1.
// portreg: va=0x0005a150 tier=1
pub fn float_198_differs_from_475cac<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let object = f.arg(0);
    f.load(0, object.wrapping_add(0x198));
    let equal = f.cmp_m(0, 0x0047_5cac).ordered_equal();
    gprs_of(u32::from(!equal), e.ecx, e.edx)
}

/// 0005A1E0 cdecl(2) (obj, key). ECX = key, EAX = 1 on the two key matches, xmm1 = [o+198], xmm0 = 0.
pub fn float_198_positive_matches_key<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let mut eax = f.arg(0);
    eax = f.r(eax.wrapping_add(4));
    f.load(1, eax.wrapping_add(0x198));
    f.zero(0);
    let above = f.cmp(1, 0).a();
    let ecx = f.arg(1);
    let found = (above && f.r(eax.wrapping_add(0x12c)) == ecx)
        || (!f.cmp_m(0, eax.wrapping_add(0x198)).b() && f.r(eax.wrapping_add(0xd4)) == ecx);
    gprs_of(u32::from(found), ecx, e.edx)
}

/// 00073530 cdecl(1). [obj+84] = [obj+88], [obj+C0] = [obj+88] * [7BA188].
pub fn float_88_copy_and_scale<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    f.load(0, eax.wrapping_add(0x88));
    f.store(eax.wrapping_add(0x84), 0);
    f.mul_m(0, 0x007b_a188);
    f.store(eax.wrapping_add(0xc0), 0);
    gprs_of(eax, e.ecx, e.edx)
}

/// 000A4CA0 cdecl(1). EAX = [obj+AC] >= [obj+218] and [obj+218] + [475CE8] > [obj+AC].
pub fn float_ac_within_218_plus_475ce8<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    f.load(0, eax.wrapping_add(0xac));
    if f.cmp_m(0, eax.wrapping_add(0x218)).b() {
        return gprs_of(0, e.ecx, e.edx);
    }
    f.load(0, eax.wrapping_add(0x218));
    f.add_m(0, 0x0047_5ce8);
    let above = f.cmp_m(0, eax.wrapping_add(0xac)).a();
    gprs_of(u32::from(above), e.ecx, e.edx)
}

/// 000AEE20 cdecl(1). Decrement [o+AC] and set bit 0 in the dword of [actor+4] + 0x30, or clear bit 5 of [o+4].
pub fn actor_float_ac_decrement_or_clear_flag<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let ecx = f.arg(0);
    let mut eax = f.r(ecx.wrapping_add(0x7c));
    f.load(0, eax.wrapping_add(0xac));
    if !f.cmp_m(0, 0x0047_d5a8).a() {
        let flags = f.r(eax.wrapping_add(4)) & 0xffff_ffdf;
        f.w(eax.wrapping_add(4), flags);
        return gprs_of(eax, ecx, e.edx);
    }
    f.sub_m(0, 0x004e_794c);
    f.store(eax.wrapping_add(0xac), 0);
    eax = f.r(ecx.wrapping_add(4));
    let flags = f.r(eax.wrapping_add(0x30)) | 1;
    f.w(eax.wrapping_add(0x30), flags);
    gprs_of(eax, ecx, e.edx)
}

/// 000C9BC0 cdecl(0). EAX = [733108] != 0 and 0 >= [7330BC] (xmm0 cleared only when [733108] != 0).
pub fn camera_set_and_timer_nonpositive<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    if f.r(0x0073_3108) == 0 {
        return gprs_of(0, e.ecx, e.edx);
    }
    f.zero(0);
    let below = f.cmp_m(0, 0x0073_30bc).b();
    gprs_of(u32::from(!below), e.ecx, e.edx)
}

/// 00120220 cdecl(3) (obj, a, b). Stores the float pair unless flag 0x200 is set and [obj+A0] > a.
pub fn store_float_pair_a0_a4<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    let mut ecx = f.r(eax.wrapping_add(4));
    f.load(0, f.esp.wrapping_add(8));
    if ecx & 0x200 != 0 {
        f.load(1, eax.wrapping_add(0xa0));
        if f.cmp(1, 0).a() {
            return gprs_of(eax, ecx, e.edx);
        }
    }
    f.store(eax.wrapping_add(0xa0), 0);
    f.load(0, f.esp.wrapping_add(0xc));
    ecx |= 0x200;
    f.store(eax.wrapping_add(0xa4), 0);
    f.zero(0);
    f.w(eax.wrapping_add(4), ecx);
    f.store(eax.wrapping_add(0x9c), 0);
    gprs_of(eax, ecx, e.edx)
}

/// 00151F20 cdecl(2) (matrix, vector). Transforms (v - m[30..38]) by the 3x3 matrix over the vector.
pub fn transform_vector_by_matrix_30_38<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    let ecx = f.arg(1);
    let (a, c) = (eax, ecx);
    f.load(3, a.wrapping_add(8));
    f.load(4, a.wrapping_add(4));
    f.load(1, c.wrapping_add(4));
    f.sub_m(1, a.wrapping_add(0x34));
    f.load(2, c.wrapping_add(8));
    f.sub_m(2, a.wrapping_add(0x38));
    f.load(0, c);
    f.sub_m(0, a.wrapping_add(0x30));
    f.mul(3, 2);
    f.mul(4, 1);
    f.add(3, 4);
    f.load(4, a);
    f.mul(4, 0);
    f.add(3, 4);
    f.store(c, 3);
    f.load(3, a.wrapping_add(0x18));
    f.load(4, a.wrapping_add(0x14));
    f.mul(3, 2);
    f.mul(4, 1);
    f.add(3, 4);
    f.load(4, a.wrapping_add(0x10));
    f.mul(4, 0);
    f.add(3, 4);
    f.store(c.wrapping_add(4), 3);
    f.load(3, a.wrapping_add(0x28));
    f.mul(3, 2);
    f.load(2, a.wrapping_add(0x24));
    f.mul(2, 1);
    f.load(1, a.wrapping_add(0x20));
    f.add(3, 2);
    f.mul(1, 0);
    f.add(3, 1);
    f.store(c.wrapping_add(8), 3);
    gprs_of(eax, ecx, e.edx)
}

/// 00347FA0 cdecl(1). [7670FC] = clamp(x, 0, [475C78]) with the C comparison order (NaN stored as is).
pub fn store_float_clamped_0_to_475c78<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    f.load(0, f.esp.wrapping_add(4));
    f.load(1, 0x0047_5c78);
    if !f.cmp(0, 1).a() {
        f.zero(2);
        if f.cmp(2, 0).a() {
            f.store(0x0076_70fc, 2);
            return e;
        }
        if !f.cmp(0, 1).a() {
            f.store(0x0076_70fc, 0);
            return e;
        }
    }
    f.store(0x0076_70fc, 1);
    e
}

/// 000D1E20 cdecl(4) (obj, f1, f3, f4). Clamped delta target at [obj+B8], f1 at +B4, f4 at +BC.
pub fn store_clamped_delta_target<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    f.load(1, f.esp.wrapping_add(8));
    f.load(3, f.esp.wrapping_add(0xc));
    f.store(eax.wrapping_add(0xb4), 1);
    f.movaps(0, 3);
    f.sub_m(0, eax.wrapping_add(0xac));
    if (f.r(0x0079_094c) as i32) <= 0x64 {
        f.load(1, 0x0047_8620);
        f.load(2, 0x0047_5d10);
        if f.cmp(1, 0).a() || !f.cmp(0, 2).be() {
            if !f.cmp(1, 0).be() {
                f.load(3, eax.wrapping_add(0xac));
                f.x[3][0] = sse::addss(f.x[3][0], f.x[1][0]);
                f.store(eax.wrapping_add(0xb8), 3);
            }
            if !f.cmp(0, 2).be() {
                f.load(0, eax.wrapping_add(0xac));
                f.x[0][0] = sse::addss(f.x[0][0], f.x[2][0]);
                f.store(eax.wrapping_add(0xb8), 0);
            }
            f.load(0, f.esp.wrapping_add(0x10));
            f.store(eax.wrapping_add(0xbc), 0);
            return gprs_of(eax, e.ecx, e.edx);
        }
    }
    f.store(eax.wrapping_add(0xb8), 3);
    f.load(0, f.esp.wrapping_add(0x10));
    f.store(eax.wrapping_add(0xbc), 0);
    gprs_of(eax, e.ecx, e.edx)
}

/// 00238D90 cdecl(2) (value, out). [out] = value / ([4C0454] == 0 ? [49EF8C] : [475DB8]).
/// EAX = ([4C0454] != 0), ECX = out.
pub fn store_float_divided_by_flag<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let flag = f.r(0x004c_0454);
    f.load(0, 0x0049_ef8c);
    let eax = u32::from(flag != 0);
    if eax != 0 {
        f.load(0, 0x0047_5db8);
    }
    f.load(1, f.esp.wrapping_add(4));
    let ecx = f.r(f.esp.wrapping_add(8));
    f.div(1, 0);
    f.store(ecx, 1);
    gprs_of(eax, ecx, e.edx)
}

/// 002DAF90 cdecl(2) (obj, dt). Advances the phase at [obj+20] by [obj+24]*dt with latch and wrap.
// portreg: va=0x002daf90 tier=1
pub fn menu_background_advance_phase<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    f.load(1, eax.wrapping_add(0x24));
    let dt = f.arg(1);
    f.x[1][0] = sse::mulss(f.x[1][0], dt);
    f.movaps(0, 1);
    f.add_m(0, eax.wrapping_add(0x20));
    let at_most_zero = f.cmp_m(0, 0x0047_5cac).be();
    f.store(eax.wrapping_add(0x20), 0);
    let mut ecx = 1;
    if !at_most_zero {
        f.w(eax.wrapping_add(0x3c), ecx);
    }
    f.load(2, 0x0047_5c84);
    if f.cmp(0, 2).be() {
        return gprs_of(eax, ecx, e.edx);
    }
    f.add(1, 2);
    if f.cmp(1, 0).b() {
        f.load(1, 0x0047_5c88);
        if f.cmp(0, 1).b() {
            return gprs_of(eax, ecx, e.edx);
        }
        f.sub(0, 1);
        f.store(eax.wrapping_add(0x20), 0);
        return gprs_of(eax, ecx, e.edx);
    }
    if f.r(eax.wrapping_add(0x3c)) != ecx {
        return gprs_of(eax, ecx, e.edx);
    }
    let mut edx = f.r(eax.wrapping_add(0x38));
    edx = edx.wrapping_add(1);
    ecx = edx;
    f.w(eax.wrapping_add(0x38), edx);
    if (ecx as i32) < 2 {
        return gprs_of(eax, ecx, edx);
    }
    f.w(eax.wrapping_add(0x38), 0);
    gprs_of(eax, ecx, edx)
}

/// 00309120 cdecl(2) (x, y). Stores x, y and the reciprocals of [475C78] / x and / y.
pub fn set_scale_pair_and_reciprocals<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    f.load(0, 0x0047_5c78);
    f.load(1, f.esp.wrapping_add(4));
    f.load(2, f.esp.wrapping_add(8));
    f.movaps(3, 0);
    f.div(3, 1);
    f.div(0, 2);
    f.store(0x0078_3f2c, 1);
    f.store(0x0078_3f28, 2);
    f.store(0x0078_3dd4, 3);
    f.store(0x0078_3f20, 0);
    e
}

/// 00341980 cdecl(4) (slot, state, value, duration). Starts a fade towards a new state.
pub fn weather_slot_set_state_with_fade<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    let eax = f.arg(0);
    let mut ecx = f.r(eax.wrapping_add(0x78));
    let edx = f.arg(1);
    if ecx == edx {
        return gprs_of(eax, ecx, edx);
    }
    f.load(0, f.esp.wrapping_add(0x10));
    let instant = f.cmp_m(0, 0x0047_5cac).be();
    f.w(eax.wrapping_add(0x74), ecx);
    ecx = f.r(eax.wrapping_add(0x80));
    f.w(eax.wrapping_add(0x7c), ecx);
    ecx = f.arg(2);
    f.w(eax.wrapping_add(0x78), edx);
    f.w(eax.wrapping_add(0x80), ecx);
    if instant {
        f.load(0, 0x0047_5c78);
        f.store(eax.wrapping_add(0x84), 0);
        f.w(eax.wrapping_add(0x74), edx);
        f.w(eax.wrapping_add(0x7c), ecx);
        f.store(eax.wrapping_add(0x88), 0);
        return gprs_of(eax, ecx, edx);
    }
    f.load(1, eax.wrapping_add(0x88));
    f.load(2, 0x0047_5c78);
    f.sub_m(2, eax.wrapping_add(0x84));
    f.div(1, 0);
    f.mul(1, 2);
    f.store(eax.wrapping_add(0x84), 1);
    f.store(eax.wrapping_add(0x88), 0);
    gprs_of(eax, ecx, edx)
}

/// 003441A0 cdecl(2) (obj, target). Moves the anchor at obj+20 to target, shifts the dependent points.
pub fn translate_anchor_and_dependent_points<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    let ecx = f.arg(1);
    let o = |offset: u32| eax.wrapping_add(offset);
    f.load(0, ecx);
    f.load(1, ecx.wrapping_add(4));
    f.load(2, ecx.wrapping_add(8));
    f.sub_m(0, o(0x20));
    f.sub_m(1, o(0x24));
    f.sub_m(2, o(0x28));
    f.movaps(3, 0);
    f.add_m(3, o(0x20));
    f.store(o(0x20), 3);
    for (offset, delta) in [(0x24, 1), (0x28, 2), (0x14, 0), (0x18, 1), (0x1c, 2), (0x2c, 0), (0x30, 1), (0x34, 2)] {
        f.load(3, o(offset));
        f.add(3, delta);
        f.store(o(offset), 3);
    }
    f.load(3, o(0x38));
    f.add(3, 0);
    f.load(0, o(0x3c));
    f.add(0, 1);
    f.store(o(0x3c), 0);
    f.load(0, o(0x40));
    f.add(0, 2);
    f.store(o(0x38), 3);
    f.store(o(0x40), 0);
    gprs_of(eax, ecx, e.edx)
}

/// 001543A0 cdecl(6) (out, t, p0, p1, p2, p3). Cubic blend of two floats, ESI/EDI are C locals.
pub fn ui_curved_menu_bezier_sample_point<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    f.load(2, f.esp.wrapping_add(8));
    f.load(0, 0x0047_5c78);
    let eax = f.arg(3);
    let ecx = f.arg(4);
    let edx = f.arg(2);
    f.sub(0, 2);
    f.movaps(3, 2);
    f.mul(3, 2);
    f.movaps(1, 0);
    f.mul(1, 0);
    f.movaps(4, 3);
    f.mul(4, 2);
    f.movaps(5, 1);
    f.mul(1, 2);
    f.load(2, 0x0047_5d18);
    f.mul(5, 0);
    f.mul(0, 3);
    f.load(3, ecx);
    f.mul(0, 2);
    f.mul(1, 2);
    f.load(2, eax);
    f.mul(3, 0);
    f.mul(2, 1);
    f.add(2, 3);
    f.load(3, edx);
    let esi = f.arg(5);
    f.mul(3, 5);
    f.add(2, 3);
    f.load(3, esi);
    let edi = f.arg(0);
    f.mul(3, 4);
    f.add(2, 3);
    f.store(edi, 2);
    f.load(2, ecx.wrapping_add(4));
    f.mul(2, 0);
    f.load(0, eax.wrapping_add(4));
    f.mul(0, 1);
    f.add(2, 0);
    f.load(0, edx.wrapping_add(4));
    f.mul(0, 5);
    f.add(2, 0);
    f.load(0, esi.wrapping_add(4));
    f.mul(0, 4);
    f.add(2, 0);
    f.store(edi.wrapping_add(4), 2);
    gprs_of(eax, ecx, edx)
}

/// 00154470 cdecl(6) (out, t, p0, p1, p2, p3). Same blend as 001543A0 for three floats.
pub fn bezier_sample_point_vec3<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    f.load(4, f.esp.wrapping_add(8));
    f.load(0, 0x0047_5c78);
    let eax = f.arg(3);
    let ecx = f.arg(4);
    f.sub(0, 4);
    let edx = f.arg(2);
    f.movaps(5, 4);
    f.mul(5, 4);
    f.movaps(2, 5);
    f.mul(2, 4);
    f.movaps(1, 0);
    f.mul(1, 0);
    f.movaps(3, 1);
    f.mul(1, 4);
    f.load(4, 0x0047_5d18);
    f.mul(1, 4);
    f.mul(3, 0);
    f.mul(0, 5);
    f.load(5, ecx);
    f.mul(0, 4);
    f.load(4, eax);
    f.mul(5, 0);
    f.mul(4, 1);
    f.add(4, 5);
    f.load(5, edx);
    f.mul(5, 3);
    f.add(4, 5);
    let esi = f.arg(5);
    f.load(5, esi);
    f.mul(5, 2);
    f.add(4, 5);
    let edi = f.arg(0);
    f.store(edi, 4);
    f.load(4, ecx.wrapping_add(4));
    f.load(5, eax.wrapping_add(4));
    f.mul(4, 0);
    f.mul(5, 1);
    f.add(4, 5);
    f.load(5, edx.wrapping_add(4));
    f.mul(5, 3);
    f.add(4, 5);
    f.load(5, esi.wrapping_add(4));
    f.mul(5, 2);
    f.add(4, 5);
    f.store(edi.wrapping_add(4), 4);
    f.load(4, ecx.wrapping_add(8));
    f.mul(4, 0);
    f.load(0, eax.wrapping_add(8));
    f.mul(0, 1);
    f.add(4, 0);
    f.load(0, edx.wrapping_add(8));
    f.mul(0, 3);
    f.add(4, 0);
    f.load(0, esi.wrapping_add(8));
    f.mul(0, 2);
    f.add(4, 0);
    f.store(edi.wrapping_add(8), 4);
    gprs_of(eax, ecx, edx)
}
