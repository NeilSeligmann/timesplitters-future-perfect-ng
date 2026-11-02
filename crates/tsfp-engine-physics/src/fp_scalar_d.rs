// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 8 batch D: the remaining straight-line scalar-SSE and move roots of the pool
//! (`game_engine_batch5/6.c`, `game_vec_list003.c`, `game_t1477_vector00[12].c`), ported
//! one-to-one. Same conventions as [`crate::fp_scalar`]; see docs/t1836-rust-wave8.md.
use crate::{fp_scalar::Fp, gprs::Gprs, memory::GuestMemory, sse};

fn gp(eax: u32, ecx: u32, edx: u32) -> Gprs {
    Gprs { eax, ecx, edx }
}

/// 000A5A60 cdecl(1). Writes obj+1A8.. plus obj+64.. to the 715F98 globals. EAX = 715F98.
pub fn position_sum<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    for (field, sum, global) in [
        (0x1a8u32, 0x64u32, 0x0071_5f98u32),
        (0x1ac, 0x68, 0x0071_5f9c),
        (0x1b0, 0x6c, 0x0071_5fa0),
    ] {
        f.load(0, eax.wrapping_add(field));
        let addend = f.r(eax.wrapping_add(sum));
        f.x[0][0] = sse::addss(f.x[0][0], addend);
        f.store(global, 0);
    }
    gp(0x0071_5f98, e.ecx, e.edx)
}

/// 000B0E30 cdecl(1). Deactivates the extension child of an object and updates the object flags.
pub fn extension_deactivate<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let edx = f.arg(0);
    if edx == 0 {
        return gp(e.eax, e.ecx, edx);
    }
    let mut eax = f.r(edx.wrapping_add(0x7c));
    if eax == 0 {
        return gp(eax, e.ecx, edx);
    }
    let mut ecx = f.r(eax.wrapping_add(0x10));
    if ecx == 0 {
        return gp(eax, ecx, edx);
    }
    ecx = f.r(eax.wrapping_add(0xc));
    f.zero(0);
    ecx &= 0xfbff_ffff;
    let negative = ecx & 0x8000 != 0;
    f.store(eax.wrapping_add(0x74), 0);
    f.store(eax.wrapping_add(0x24), 0);
    f.w(eax.wrapping_add(0x10), 0);
    f.w(eax.wrapping_add(0xc), ecx);
    f.w(eax.wrapping_add(0x20), 0);
    f.store(eax.wrapping_add(0x38), 0);
    if negative {
        ecx |= 2;
        f.w(eax.wrapping_add(0xc), ecx);
    }
    ecx = f.r(edx.wrapping_add(0x18));
    eax = f.r(edx.wrapping_add(0x28));
    ecx |= 0x42;
    eax &= 0xfbff_ffff;
    f.w(edx.wrapping_add(0x18), ecx);
    f.w(edx.wrapping_add(0x28), eax);
    gp(eax, ecx, edx)
}

/// 000FF420 cdecl(2) (obj, ratio). Stores the ratio clamped to [0, [475C84]] at obj+30 and mirrors it.
// portreg: va=0x000ff420 tier=1
pub fn effect_clamp_store<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let mut ecx = f.arg(0);
    if ecx == 0 {
        return gp(e.eax, ecx, e.edx);
    }
    f.zero(1);
    f.load(0, f.esp.wrapping_add(8));
    if !f.cmp(1, 0).b() {
        f.w(ecx.wrapping_add(0x30), 0);
        return gp(e.eax, ecx, e.edx);
    }
    f.load(1, 0x0047_5c84);
    if !f.cmp(1, 0).a() {
        f.movaps(0, 1);
    }
    let mut eax = f.r(ecx);
    f.store(ecx.wrapping_add(0x30), 0);
    if matches!(eax, 0x27 | 0x1c | 0x1d | 0x23 | 0x24) {
        eax = f.r(ecx.wrapping_add(0x18));
        f.store(eax.wrapping_add(0x2ac), 0);
    }
    eax = f.r(ecx);
    if eax == 0xd || eax == 0xe {
        ecx = f.r(ecx.wrapping_add(0x18));
        f.store(ecx.wrapping_add(0x138), 0);
    }
    gp(eax, ecx, e.edx)
}

/// 00124C90 cdecl(1). Enables an effect (flag 0x200000) and resets its counters.
pub fn effect_enable<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    let mut ecx = f.r(eax.wrapping_add(4));
    if ecx & 0x20_0000 != 0 {
        let two = f.r(eax.wrapping_add(0x17c)) == 2;
        f.load(0, 0x0047_8900);
        f.store(eax.wrapping_add(0x188), 0);
        if two {
            f.w(eax.wrapping_add(0x17c), 1);
        }
        return gp(eax, ecx, e.edx);
    }
    f.zero(0);
    ecx |= 0x20_0000;
    f.w(eax.wrapping_add(0x180), 0);
    f.w(eax.wrapping_add(0x184), 0);
    f.load(0, 0x0047_8900);
    f.w(eax.wrapping_add(4), ecx);
    f.w(eax.wrapping_add(0x17c), 0);
    f.store(eax.wrapping_add(0x188), 0);
    gp(eax, ecx, e.edx)
}

/// 00124F30 cdecl(1). EAX = flag 0x8000000 set and not ([obj+194] < [475CD4]).
pub fn object_flag_float_predicate<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    if f.r(eax.wrapping_add(4)) & 0x0800_0000 != 0 {
        f.load(0, eax.wrapping_add(0x194));
        if !f.cmp_m(0, 0x0047_5cd4).b() {
            return gp(1, e.ecx, e.edx);
        }
    }
    gp(0, e.ecx, e.edx)
}

/// 0022B130 cdecl(1). Copies obj+64..6C to the object at [75CC98]+7C.., then adds [475D00] and [475C78].
pub fn global_position_copy<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    let global = 0x0075_cc98;
    let mut edx = f.r(global);
    let mut eax = f.arg(0);
    let mut ecx = f.r(eax.wrapping_add(0x64));
    f.w(edx.wrapping_add(0x7c), ecx);
    ecx = f.r(eax.wrapping_add(0x68));
    edx = f.r(global);
    f.w(edx.wrapping_add(0x80), ecx);
    eax = f.r(eax.wrapping_add(0x6c));
    ecx = f.r(global);
    f.w(ecx.wrapping_add(0x84), eax);
    eax = f.r(global);
    f.load(0, eax.wrapping_add(0x84));
    f.add_m(0, 0x0047_5d00);
    f.store(eax.wrapping_add(0x84), 0);
    eax = f.r(global);
    f.load(0, eax.wrapping_add(0x80));
    f.add_m(0, 0x0047_5c78);
    f.store(eax.wrapping_add(0x80), 0);
    gp(eax, ecx, edx)
}

/// 002F7A20 cdecl(0). Converts four scene ints to floats and copies the scene globals to 7840A0...
pub fn scene_global_copy<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let convert = |f: &mut Fp<M>, source: u32| {
        let value = f.r(source);
        f.x[0][0] = sse::cvtsi2ss(value);
    };
    convert(f, 0x0078_3ee0);
    let eax = f.r(0x0078_44a8);
    f.store(0x0078_40a0, 0);
    convert(f, 0x0078_3dc4);
    f.store(0x0078_40a4, 0);
    convert(f, 0x0078_3de4);
    f.store(0x0078_40a8, 0);
    convert(f, 0x0078_3ccc);
    f.store(0x0078_40ac, 0);
    for (source, target) in [
        (0x0078_3dd8u32, 0x0078_40b0u32),
        (0x0078_3cc0, 0x0078_40b4),
        (0x0078_3de0, 0x0078_40b8),
    ] {
        f.load(0, source);
        f.store(target, 0);
    }
    let ecx = f.r(eax.wrapping_add(0x264));
    f.w(0x0078_40bc, ecx);
    gp(eax, ecx, e.edx)
}

/// 00164960 cdecl(0). Resets the state block at 74C3CC, defaults depend on mode 4 ([79094C]).
// portreg: va=0x00164960 tier=1
pub fn reset_state_block_74c3cc<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    f.w(0x0074_c3f0, 0);
    f.w(0x0074_c3ec, 0);
    f.w(0x0074_c400, 0);
    f.w(0x0074_c404, 0);
    let eax = 0xffff_ffff;
    f.w(0x0074_c3e4, 0);
    f.w(0x0074_c400, eax);
    f.w(0x0074_c3e4, eax);
    let mode = f.r(0x0079_094c);
    f.w(0x0074_c3e8, 0);
    if mode != 4 {
        f.load(0, 0x0047_cff4);
        f.store(0x0074_c3cc, 0);
        f.load(0, 0x0047_5d28);
        f.store(0x0074_c408, 0);
    } else {
        f.load(0, 0x0047_5d28);
        f.store(0x0074_c3cc, 0);
        f.load(0, 0x0047_d1f4);
        f.store(0x0074_c408, 0);
    }
    gp(eax, 0, e.edx)
}

/// Entry base address: `[74C458] + ((index & 0xFFFF) << 9)`, shared by 00166060 and 00166140.
fn entry_of<M: GuestMemory>(f: &Fp<M>, index: u32) -> u32 {
    let base = f.r(0x0074_c458);
    ((index & 0xffff) << 9).wrapping_add(base)
}

/// 00166060 cdecl(3) (index, point, out). Cross product of the point offset with the entry velocity.
pub fn entry_point_velocity_cross<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let index = f.arg(0);
    let eax = entry_of(f, index);
    f.load(3, eax.wrapping_add(0x138));
    let mut ecx = f.arg(1);
    f.load(1, ecx.wrapping_add(4));
    f.sub_m(1, eax.wrapping_add(0x144));
    f.load(2, ecx.wrapping_add(8));
    f.sub_m(2, eax.wrapping_add(0x148));
    f.load(0, ecx);
    f.sub_m(0, eax.wrapping_add(0x140));
    ecx = f.arg(2);
    f.mul(3, 2);
    f.movaps(4, 1);
    f.mul_m(4, eax.wrapping_add(0x13c));
    f.sub(3, 4);
    f.store(ecx, 3);
    f.load(4, eax.wrapping_add(0x134));
    f.movaps(3, 0);
    f.mul_m(3, eax.wrapping_add(0x13c));
    f.mul(4, 2);
    f.sub(3, 4);
    f.store(ecx.wrapping_add(4), 3);
    f.load(2, eax.wrapping_add(0x134));
    f.mul(2, 1);
    f.load(1, eax.wrapping_add(0x138));
    f.mul(1, 0);
    f.sub(2, 1);
    f.store(ecx.wrapping_add(8), 2);
    f.load(0, eax.wrapping_add(0x14c));
    f.add_m(0, ecx);
    f.store(ecx, 0);
    f.load(0, eax.wrapping_add(0x150));
    f.add(0, 3);
    f.store(ecx.wrapping_add(4), 0);
    f.load(0, eax.wrapping_add(0x154));
    f.add(0, 2);
    f.store(ecx.wrapping_add(8), 0);
    gp(eax, ecx, e.edx)
}

/// 00166140 cdecl(4) (index, point, direction, out). Cross product with the offset to the closest point on a line.
pub fn entry_cross_closest_point_on_line<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    let index = f.arg(0);
    let edx = f.arg(2);
    f.load(0, edx.wrapping_add(8));
    let base = f.r(0x0074_c458);
    let shifted = (index & 0xffff) << 9;
    let at = |offset: u32| shifted.wrapping_add(base).wrapping_add(offset);
    f.load(3, at(0x148));
    f.load(2, at(0x140));
    f.load(1, at(0x144));
    f.load(4, at(0x154));
    let eax = shifted.wrapping_add(base);
    let mut ecx = f.arg(1);
    f.sub_m(3, ecx.wrapping_add(8));
    f.sub_m(2, ecx);
    f.sub_m(1, ecx.wrapping_add(4));
    f.mul_m(1, edx.wrapping_add(4));
    f.mul(0, 3);
    f.load(3, edx);
    f.mul(3, 2);
    f.load(2, edx);
    f.add(0, 3);
    f.load(3, edx.wrapping_add(8));
    f.add(0, 1);
    f.mul(3, 0);
    f.add_m(3, ecx.wrapping_add(8));
    f.mul(2, 0);
    f.add_m(2, ecx);
    f.movaps(1, 0);
    f.mul_m(1, edx.wrapping_add(4));
    f.add_m(1, ecx.wrapping_add(4));
    f.load(0, eax.wrapping_add(0x140));
    ecx = f.arg(3);
    f.sub(0, 2);
    f.load(2, eax.wrapping_add(0x144));
    f.sub(2, 1);
    f.load(1, eax.wrapping_add(0x148));
    f.sub(1, 3);
    f.load(3, eax.wrapping_add(0x150));
    f.mul(3, 1);
    f.mul(4, 2);
    f.sub(3, 4);
    f.store(ecx, 3);
    f.load(3, eax.wrapping_add(0x154));
    f.load(4, eax.wrapping_add(0x14c));
    f.mul(4, 1);
    f.mul(3, 0);
    f.sub(3, 4);
    f.store(ecx.wrapping_add(4), 3);
    f.load(1, eax.wrapping_add(0x14c));
    f.mul(1, 2);
    f.load(2, eax.wrapping_add(0x150));
    f.mul(2, 0);
    f.sub(1, 2);
    f.store(ecx.wrapping_add(8), 1);
    gp(eax, ecx, edx)
}
