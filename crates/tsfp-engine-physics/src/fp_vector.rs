// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-gameplay/modes/predicates
//! T1836 wave 8 batch C: legacy-SSE move roots (movss loads and stores, xorps clears) ported
//! one-to-one from the proven C replacements of the "vector-v1" rows of the replace-proof
//! snapshot. They carry XMM0 state, so they live next to the scalar-SSE model. See
//! docs/t1836-rust-wave8.md. The coverage tree of these rows stays `unknown`.
use crate::{fp_scalar::Fp, gprs::Gprs, memory::GuestMemory};

fn gp(eax: u32, ecx: u32, edx: u32) -> Gprs {
    Gprs { eax, ecx, edx }
}

/// movss xmm0, [value]: lane 0 set, lanes 1..3 zeroed.
fn scalar<M: GuestMemory>(f: &mut Fp<M>, value: u32) {
    f.x[0] = [value, 0, 0, 0];
}

/// 000425E0 cdecl(0). Promotes mode 3 or 5 to 5 and copies [475D18] to [6B98B0].
pub fn promote_mode_3_or_5_to_5<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    if f.r(0x007d_e458) & 0x0080_0000 != 0 {
        return e;
    }
    let eax = f.r(0x006b_98a4);
    if eax != 3 && eax != 5 {
        return gp(eax, e.ecx, e.edx);
    }
    f.load(0, 0x0047_5d18);
    f.store(0x006b_98b0, 0);
    f.w(0x006b_98a4, 5);
    gp(eax, e.ecx, e.edx)
}

/// 00046210 cdecl(0). Initialises the float block at 4D1838.
// portreg: va=0x00046210 tier=1
pub fn init_float_block_4d1838<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    f.load(0, 0x0047_85d4);
    f.store(0x004d_1838, 0);
    f.load(0, 0x0047_85c8);
    f.store(0x004d_1844, 0);
    f.load(0, 0x0047_5c78);
    f.store(0x004d_183c, 0);
    f.store(0x004d_1840, 0);
    f.store(0x004d_1848, 0);
    f.w(0x006d_5ec0, 0);
    e
}

/// 00065230 cdecl(1). When the value differs from [7DE30C]: [7DE30C] = value, [7DE300] = 0.
// portreg: va=0x00065230 tier=1
pub fn set_7de30c_clear_float_7de300<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    if eax == f.r(0x007d_e30c) {
        return gp(eax, e.ecx, e.edx);
    }
    f.zero(0);
    f.w(0x007d_e30c, eax);
    f.store(0x007d_e300, 0);
    gp(eax, e.ecx, e.edx)
}

/// 000746A0 cdecl(1). Clears obj+84, +C0 and +88.
// portreg: va=0x000746a0 tier=1
pub fn object_clear_three<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    f.zero(0);
    f.w(eax.wrapping_add(0x84), 0);
    f.w(eax.wrapping_add(0xc0), 0);
    f.w(eax.wrapping_add(0x88), 0);
    gp(eax, e.ecx, e.edx)
}

/// 000AEE60 cdecl(1). [[obj+7C]+A8] = [475D18]. ECX = child.
pub fn extension_store_a8<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    let ecx = f.r(eax.wrapping_add(0x7c));
    f.load(0, 0x0047_5d18);
    f.store(ecx.wrapping_add(0xa8), 0);
    gp(eax, ecx, e.edx)
}

/// 000B2D20 cdecl(1). Clears obj+C0 and +C8 and sets byte obj+32 to 1.
pub fn gp_clear_object_c0_c8<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    f.zero(0);
    f.w(eax.wrapping_add(0xc0), 0);
    f.w(eax.wrapping_add(0xc8), 0);
    f.m.write8(eax.wrapping_add(0x32), 1);
    gp(eax, e.ecx, e.edx)
}

/// 000C9440 cdecl(1). Copies three floats at [p] to the 7B0CD0 globals, reading after each store.
pub fn ordered_vector_globals<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    for (index, global) in [0x007b_0cd0u32, 0x007b_0cd4, 0x007b_0cd8].into_iter().enumerate() {
        let value = f.r(eax.wrapping_add(4 * index as u32));
        scalar(f, value);
        f.store(global, 0);
    }
    gp(eax, e.ecx, e.edx)
}

/// 000C9F60 cdecl(2). Stores the two float arguments to 7330AC and 7330B0.
pub fn store_float_arg_pair<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let (a, b) = (f.arg(0), f.arg(1));
    scalar(f, a);
    f.store(0x0073_30ac, 0);
    scalar(f, b);
    f.store(0x0073_30b0, 0);
    e
}

/// Shared body of 000E10E0 (+9C) and 000FF3D0 (+38): non-null object gets float argument 1.
fn store_arg1_unless_null<M: GuestMemory>(f: &mut Fp<M>, e: Gprs, offset: u32) -> Gprs {
    let eax = f.arg(0);
    if eax != 0 {
        f.load(0, f.esp.wrapping_add(8));
        f.store(eax.wrapping_add(offset), 0);
    }
    gp(eax, e.ecx, e.edx)
}

/// 000E10E0 cdecl(2). obj+9C = float argument unless the object is null.
pub fn object_store_9c<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    store_arg1_unless_null(f, e, 0x9c)
}

/// 000FF3D0 cdecl(2). obj+38 = float argument unless the object is null.
pub fn effect_store_38<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    store_arg1_unless_null(f, e, 0x38)
}

/// 0010E970 cdecl(1). [[7AD28C]+AC] = float, [[7AD28C]+4] |= 0x20000.
pub fn set_obj_7ad28c_float_ac_flag_20000<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let mut eax = f.r(0x007a_d28c);
    if eax == 0 {
        return gp(eax, e.ecx, e.edx);
    }
    let value = f.arg(0);
    scalar(f, value);
    f.store(eax.wrapping_add(0xac), 0);
    eax = f.r(0x007a_d28c);
    let flags = f.r(eax.wrapping_add(4)) | 0x2_0000;
    f.w(eax.wrapping_add(4), flags);
    gp(eax, e.ecx, e.edx)
}

/// Shared body of 0010EA50 (+60) and 0010EA90 (+5C): store the float argument and mark dirty 0x400.
fn set_field_mark_dirty_400<M: GuestMemory>(f: &mut Fp<M>, e: Gprs, offset: u32) -> Gprs {
    let eax = f.arg(0);
    if eax == 0 {
        return gp(eax, e.ecx, e.edx);
    }
    let mut ecx = f.r(eax.wrapping_add(4));
    let value = f.arg(1);
    scalar(f, value);
    ecx |= 0x400;
    f.store(eax.wrapping_add(offset), 0);
    f.w(eax.wrapping_add(4), ecx);
    gp(eax, ecx, e.edx)
}

/// 0010EA50 cdecl(2). obj+60 = float argument, flags (obj+4) |= 0x400, unless the object is null.
pub fn object_set_field_0x60_mark_dirty<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    set_field_mark_dirty_400(f, e, 0x60)
}

/// 0010EA90 cdecl(2). obj+5C = float argument, flags (obj+4) |= 0x400, unless the object is null.
pub fn object_set_field_0x5c_mark_dirty<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    set_field_mark_dirty_400(f, e, 0x5c)
}

/// 0010EAB0 cdecl(2). obj+84 = float, flags (obj+4) = (flags & !2) | 4.
pub fn object_set_field_0x84_mark_dirty_0x4<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    if eax == 0 {
        return gp(eax, e.ecx, e.edx);
    }
    let mut ecx = f.r(eax.wrapping_add(4));
    let value = f.arg(1);
    scalar(f, value);
    ecx &= 0xffff_fffd;
    ecx |= 4;
    f.store(eax.wrapping_add(0x84), 0);
    f.w(eax.wrapping_add(4), ecx);
    gp(eax, ecx, e.edx)
}

/// 0010EB10 cdecl(3). obj+68 = float 1, obj+6C = float 2 for a non-null object.
// portreg: va=0x0010eb10 tier=1
pub fn object_set_fields_0x68_0x6c<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    if eax != 0 {
        let value = f.arg(1);
        scalar(f, value);
        f.store(eax.wrapping_add(0x68), 0);
        let value = f.arg(2);
        scalar(f, value);
        f.store(eax.wrapping_add(0x6c), 0);
    }
    gp(eax, e.ecx, e.edx)
}

/// Shared body of 0010EB50 (+70), 0010EB90 (+74) and 0010ECB0 (+64): store the float argument.
fn set_field_float<M: GuestMemory>(f: &mut Fp<M>, e: Gprs, offset: u32) -> Gprs {
    let eax = f.arg(0);
    if eax != 0 {
        let value = f.arg(1);
        scalar(f, value);
        f.store(eax.wrapping_add(offset), 0);
    }
    gp(eax, e.ecx, e.edx)
}

/// 0010EB50 cdecl(2). obj+70 = float argument unless the object is null.
pub fn object_set_field_0x70<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    set_field_float(f, e, 0x70)
}

/// 0010EB90 cdecl(2). obj+74 = float argument unless the object is null.
pub fn object_set_field_0x74<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    set_field_float(f, e, 0x74)
}

/// 0010ECB0 cdecl(2). obj+64 = float argument unless the object is null.
pub fn object_set_field_0x64<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    set_field_float(f, e, 0x64)
}

/// 00119AB0 cdecl(2). Initialises the state block at 7AC800 from the two arguments.
// portreg: va=0x00119ab0 tier=1
pub fn init_state_block_7ac800<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let mut eax = f.arg(0);
    if eax == 0 {
        f.w(0x007a_c800, 1);
        f.w(0x007a_c804, 0x004e_63f8);
    } else {
        f.w(0x007a_c800, eax);
        eax = f.arg(1);
        f.w(0x007a_c804, eax);
    }
    f.load(0, 0x0047_d580);
    f.store(0x007a_c814, 0);
    f.store(0x007a_c818, 0);
    f.store(0x007a_c81c, 0);
    f.zero(0);
    f.store(0x007a_c820, 0);
    f.w(0x007a_c808, 0);
    f.w(0x007a_c80c, 0);
    f.store(0x007a_c810, 0);
    gp(eax, 0, e.edx)
}

/// Shared body of 0011F510, 0011F540 and 0011F570: set a default float field and a dirty flag.
fn set_default_field_mark_dirty<M: GuestMemory>(
    f: &mut Fp<M>,
    e: Gprs,
    source: u32,
    flag: u32,
    offset: u32,
) -> Gprs {
    let eax = f.arg(0);
    let mut ecx = f.r(eax.wrapping_add(4));
    f.load(0, source);
    ecx |= flag;
    f.w(eax.wrapping_add(4), ecx);
    f.store(eax.wrapping_add(offset), 0);
    gp(eax, ecx, e.edx)
}

/// 0011F510 cdecl(1). obj+A8 = [47CFF4], flags (obj+4) |= 0x400.
pub fn object_set_field_0xa8_default<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    set_default_field_mark_dirty(f, e, 0x0047_cff4, 0x400, 0xa8)
}

/// 0011F540 cdecl(1). obj+AC = [4782C4], flags (obj+4) |= 0x1000.
pub fn object_set_field_0xac_default<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    set_default_field_mark_dirty(f, e, 0x0047_82c4, 0x1000, 0xac)
}

/// 0011F570 cdecl(1). obj+B0 = [47CFF4], flags (obj+4) |= 0x800.
pub fn object_set_field_0xb0_default<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    set_default_field_mark_dirty(f, e, 0x0047_cff4, 0x800, 0xb0)
}

/// 0011FE40 cdecl(2). Arms effect mode 2 with the float argument at +94.
pub fn effect_arm_mode_2<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    let eax = f.arg(0);
    let mut edx = f.r(eax.wrapping_add(4));
    f.zero(0);
    let ecx = 2;
    edx |= ecx;
    f.store(eax.wrapping_add(0x90), 0);
    let value = f.arg(1);
    scalar(f, value);
    f.w(eax.wrapping_add(4), edx);
    f.w(eax.wrapping_add(0x88), ecx);
    f.store(eax.wrapping_add(0x94), 0);
    f.w(eax.wrapping_add(0x8c), 0x3f);
    gp(eax, ecx, edx)
}

/// 0011FE80 cdecl(2). Arms effect mode 1 with the float argument at +94.
pub fn effect_arm_mode_1<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    let mut ecx = f.r(eax.wrapping_add(4));
    let value = f.r(f.esp.wrapping_add(8));
    scalar(f, value);
    ecx |= 2;
    f.store(eax.wrapping_add(0x94), 0);
    f.zero(0);
    f.w(eax.wrapping_add(4), ecx);
    f.w(eax.wrapping_add(0x88), 1);
    f.store(eax.wrapping_add(0x90), 0);
    f.w(eax.wrapping_add(0x8c), 0x3f);
    gp(eax, ecx, e.edx)
}

/// 00120FF0 cdecl(1). With flag 0x2000 set: flags |= 0x4000 and obj+CC = 0.
pub fn effect_flag_0x2000_arm<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let ecx = f.arg(0);
    let mut eax = f.r(ecx.wrapping_add(4));
    if eax & 0x2000 == 0 {
        return gp(eax, ecx, e.edx);
    }
    f.zero(0);
    eax |= 0x4000;
    f.w(ecx.wrapping_add(4), eax);
    f.store(ecx.wrapping_add(0xcc), 0);
    gp(eax, ecx, e.edx)
}

/// 0012B940 cdecl(1). Previous ring entry field 0x2C = float argument. EAX = entry offset.
pub fn ring_742510_set_prev_entry_field_0x2c<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let mut eax = f.r(0x0074_50d0).wrapping_sub(1);
    if (eax as i32) < 0 {
        eax = eax.wrapping_add(0xc8);
    }
    let value = f.arg(0);
    scalar(f, value);
    eax = eax.wrapping_mul(0x38);
    f.store(eax.wrapping_add(0x0074_253c), 0);
    gp(eax, e.ecx, e.edx)
}

/// 0012C130 cdecl(0). Initialises the float block at 7ABF60.
// portreg: va=0x0012c130 tier=1
pub fn init_float_block_7abf60<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    f.load(0, 0x0047_5c78);
    f.store(0x007a_bf7c, 0);
    for (source, target) in [
        (0x004e_6680u32, 0x007a_bf64u32),
        (0x004e_6684, 0x007a_bf68),
        (0x004e_6688, 0x007a_bf6c),
        (0x004e_668c, 0x007a_bf70),
        (0x004e_6690, 0x007a_bf74),
    ] {
        f.load(0, source);
        f.store(target, 0);
    }
    f.load(0, 0x004e_6694);
    f.w(0x007a_bf60, 0x004e_6678);
    f.store(0x007a_bf78, 0);
    e
}

/// 0012DE80 cdecl(2) (index, value). Resets the group at 7A78C8 + index * 0x177C when its head equals value.
pub fn slot_7a6160_reset_group_if_equal<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let mut eax = f.arg(0);
    if (eax as i32) < 0 {
        return gp(eax, e.ecx, e.edx);
    }
    let edx = f.arg(1);
    eax = eax.wrapping_mul(0x177c).wrapping_add(0x007a_78c8);
    let ecx = f.r(eax);
    if ecx != edx {
        return gp(eax, ecx, edx);
    }
    f.zero(0);
    f.store(eax.wrapping_add(4), 0);
    f.load(0, 0x0047_5d10);
    f.w(eax.wrapping_add(0xc), 2);
    f.store(eax.wrapping_add(8), 0);
    gp(eax, ecx, edx)
}

/// 001427D0 cdecl(1). Sets flag 0x1750 of the slot at 7A6160 and resets 0x1754.
// portreg: va=0x001427d0 tier=1
pub fn slot_7a6160_set_flag_0x1750<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let mut eax = f.arg(0);
    eax = f.r(eax.wrapping_add(4));
    if (eax as i32) < 0 {
        return gp(eax, e.ecx, e.edx);
    }
    f.zero(0);
    eax = eax.wrapping_mul(0x177c).wrapping_add(0x007a_6160);
    let ecx = f.r(eax.wrapping_add(0x1750));
    if ecx == 0 {
        f.store(eax.wrapping_add(0x175c), 0);
        f.store(eax.wrapping_add(0x1760), 0);
    }
    f.w(eax.wrapping_add(0x1750), 1);
    f.store(eax.wrapping_add(0x1754), 0);
    gp(eax, ecx, e.edx)
}

/// 00155040 cdecl(0). Clears the globals 74984C..749884, 7A3890 and 7A388C. EAX = 0, XMM0 = 0.
pub fn clear_globals_74984c_to_749884<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    f.w(0x0074_984c, 0);
    f.w(0x0074_9850, 0);
    f.zero(0);
    for address in (0x0074_9854u32..=0x0074_9880).step_by(4) {
        f.w(address, 0);
    }
    f.store(0x007a_3890, 0);
    f.store(0x007a_388c, 0);
    f.w(0x0074_9884, 0);
    gp(0, e.ecx, e.edx)
}

/// 00155410 cdecl(1). [4E7968] = value, [74A990] = 0.
pub fn set_global_4e7968_reset_74a990<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    f.zero(0);
    f.w(0x004e_7968, eax);
    f.store(0x0074_a990, 0);
    gp(eax, e.ecx, e.edx)
}

/// 00177930 cdecl(2). [[obj+7C]+8FC] = float argument.
pub fn object_set_child_0x7c_float_0x8fc<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    let ecx = f.r(eax.wrapping_add(0x7c));
    let value = f.arg(1);
    scalar(f, value);
    f.store(ecx.wrapping_add(0x8fc), 0);
    gp(eax, ecx, e.edx)
}

/// 00196170 cdecl(0). [756E7C] = [4E8CEC], [74FE24] = 0.
pub fn copy_global_4e8cec_and_clear_float<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.r(0x004e_8cec);
    f.zero(0);
    f.w(0x0075_6e7c, eax);
    f.store(0x0074_fe24, 0);
    gp(eax, e.ecx, e.edx)
}

/// 001B5680 cdecl(2). obj+44 = value, obj+48 = 0.
pub fn set_dword_44_and_clear_float_48<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    let ecx = f.arg(1);
    f.zero(0);
    f.w(eax.wrapping_add(0x44), ecx);
    f.store(eax.wrapping_add(0x48), 0);
    gp(eax, ecx, e.edx)
}

/// 001B62D0 cdecl(2). Sets flag bit 3 at +34, stores the float at +F0 and clears +F8 when +E8 equals +F8.
pub fn object_set_flag_0x34_bit3_store_float_0xf0<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    let eax = f.arg(0);
    let mut ecx = f.r(eax.wrapping_add(0x34));
    let edx = f.r(eax.wrapping_add(0xf8));
    let value = f.arg(1);
    scalar(f, value);
    ecx |= 8;
    f.w(eax.wrapping_add(0x34), ecx);
    ecx = f.r(eax.wrapping_add(0xe8));
    f.store(eax.wrapping_add(0xf0), 0);
    if ecx == edx {
        f.w(eax.wrapping_add(0xf8), 0);
    }
    gp(eax, ecx, edx)
}

/// 0024C920 cdecl(0). Resets the state block at 78B600.
pub fn reset_state_block_78b600<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    f.zero(0);
    f.w(0x0078_b600, 4);
    f.w(0x0078_b664, 0x0a);
    f.store(0x0078_b668, 0);
    f.w(0x0078_b66c, 0);
    f.store(0x0078_b670, 0);
    e
}

/// 002649D0 cdecl(1). Requests state 4 on the extension child unless flag 0x10000 or already state 4.
pub fn object_ext_request_state_4<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    let mut eax = f.arg(0);
    eax = f.r(eax.wrapping_add(0x7c));
    let mut edx = f.r(eax.wrapping_add(0x14)) & 0x0001_0000;
    let mut ecx = edx;
    if ecx != 0 {
        return gp(eax, ecx, edx);
    }
    edx = f.r(eax.wrapping_add(0x20));
    ecx = 4;
    if edx == ecx {
        return gp(eax, ecx, edx);
    }
    f.load(0, 0x0047_5c78);
    f.w(eax.wrapping_add(0x9d0), ecx);
    f.store(eax.wrapping_add(0x9e0), 0);
    gp(eax, ecx, edx)
}

fn copy_four_floats<M: GuestMemory>(f: &mut Fp<M>, e: Gprs, pairs: [(u32, u32); 4]) -> Gprs {
    for (source, target) in pairs {
        f.load(0, source);
        f.store(target, 0);
    }
    e
}

/// 00309080 cdecl(0). Copies the four floats 783F.. to 762D...
pub fn copy_four_floats_783f_to_762d<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    copy_four_floats(
        f,
        e,
        [
            (0x0078_3f2c, 0x0076_2d58),
            (0x0078_3f28, 0x0076_2d5c),
            (0x0078_3dd4, 0x0076_2d60),
            (0x0078_3f20, 0x0076_2d64),
        ],
    )
}

/// 003090D0 cdecl(0). Inverse of 00309080.
pub fn copy_four_floats_762d_to_783f<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    copy_four_floats(
        f,
        e,
        [
            (0x0076_2d58, 0x0078_3f2c),
            (0x0076_2d5c, 0x0078_3f28),
            (0x0076_2d60, 0x0078_3dd4),
            (0x0076_2d64, 0x0078_3f20),
        ],
    )
}

/// 00341630 cdecl(0). Clears three globals at 7789xx.
// portreg: va=0x00341630 tier=1
pub fn clear_7789dc_group<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    f.zero(0);
    f.store(0x0077_89dc, 0);
    f.w(0x0077_89f4, 0);
    f.store(0x0077_89e4, 0);
    e
}

/// 00341A10 cdecl(2). Sets state 3 and stores the float at +C, clearing +8 on a state change.
pub fn object_set_state_3_store_float_0xc<M: GuestMemory>(f: &mut Fp<M>, e: Gprs) -> Gprs {
    let eax = f.arg(0);
    if eax == 0 {
        return gp(eax, e.ecx, e.edx);
    }
    let edx = f.r(eax.wrapping_add(4));
    let ecx = 3;
    if edx != ecx {
        f.zero(0);
        f.store(eax.wrapping_add(8), 0);
    }
    let value = f.arg(1);
    scalar(f, value);
    f.w(eax.wrapping_add(4), ecx);
    f.store(eax.wrapping_add(0xc), 0);
    gp(eax, ecx, edx)
}

/// 00349390 cdecl(3). Copies two vec3 sources to +C and +18, clears the float at +38, sets +8 to 1.
pub fn init_entry_copy_two_vec3<M: GuestMemory>(f: &mut Fp<M>, _e: Gprs) -> Gprs {
    let eax = f.arg(0);
    let mut ecx = f.arg(1);
    let mut edx = f.r(ecx);
    f.zero(0);
    f.w(eax.wrapping_add(0xc), edx);
    edx = f.r(ecx.wrapping_add(4));
    f.w(eax.wrapping_add(0x10), edx);
    ecx = f.r(ecx.wrapping_add(8));
    f.w(eax.wrapping_add(0x14), ecx);
    ecx = f.arg(2);
    edx = f.r(ecx);
    f.w(eax.wrapping_add(0x18), edx);
    edx = f.r(ecx.wrapping_add(4));
    f.w(eax.wrapping_add(0x1c), edx);
    ecx = f.r(ecx.wrapping_add(8));
    f.w(eax.wrapping_add(0x20), ecx);
    f.store(eax.wrapping_add(0x38), 0);
    f.w(eax.wrapping_add(8), 1);
    gp(eax, ecx, edx)
}
