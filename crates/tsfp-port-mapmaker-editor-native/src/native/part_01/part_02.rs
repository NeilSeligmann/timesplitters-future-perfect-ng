use super::*;
#[rustfmt::skip] mod unformatted { use super::*;

/// 0x002FFAC0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2ffac0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2ffac0;
    let mut jt = 0u32;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x2ffac0, previous, pc); previous = pc; }
        match pc {
            0x2ffac0 => {
                push!(ecx);
                push!(ebx);
                ebx = eax;
                eax = m.read32(0x7844a8);
                fa = ((((m.read16(eax.wrapping_add(0x386e)))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                m.write32(esp.wrapping_add(0x4), 0);
                if (fa <= fb) { pc = 0x2ffc04; continue; }
                pc = 0x2ffadf; continue;
            }
            0x2ffadf => {
                push!(ebp);
                push!(esi);
                ebp = 0x3870;
                pc = 0x2ffae6; continue;
            }
            0x2ffae6 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(ebp).wrapping_add(0x6))) as u32)) & 0xff);
                eax = eax.wrapping_add(ebp);
                edx = ((((ecx) as u8)) as u8 as u32);
                edx = edx.wrapping_sub(1);
                fa = edx;
                fb = ((((0xdi32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x2ffb73; continue; }
                pc = 0x2ffaf5; continue;
            }
            0x2ffaf5 => {
                edx = ((m.read8(edx.wrapping_add(0x2ffc24))) as u8 as u32);
                jt = m.read32(edx.wrapping_mul(0x4).wrapping_add(0x2ffc08));
                if (jt == 0x2ffb03u32) { pc = 0x2ffb03; continue; }
                if (jt == 0x2ffb12u32) { pc = 0x2ffb12; continue; }
                if (jt == 0x2ffb21u32) { pc = 0x2ffb21; continue; }
                if (jt == 0x2ffb30u32) { pc = 0x2ffb30; continue; }
                if (jt == 0x2ffb3fu32) { pc = 0x2ffb3f; continue; }
                if (jt == 0x2ffb59u32) { pc = 0x2ffb59; continue; }
                if (jt == 0x2ffb73u32) { pc = 0x2ffb73; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x2ffb03 => {
                fa = ebx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ecx = ecx.wrapping_add(ecx).wrapping_add(0xc);
                m.write8(eax.wrapping_add(0x6), (((((ecx) as u8)) as u32)) as u8);
                pc = 0x2ffb77; continue;
            }
            0x2ffb12 => {
                fa = ebx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                edx = (edx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edx = edx.wrapping_add(edx).wrapping_add(0xb);
                m.write8(eax.wrapping_add(0x6), (((((edx) as u8)) as u32)) as u8);
                pc = 0x2ffb77; continue;
            }
            0x2ffb21 => {
                fa = ebx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ecx = ecx.wrapping_add(ecx).wrapping_add(0xc);
                m.write8(eax.wrapping_add(0x6), (((((ecx) as u8)) as u32)) as u8);
                pc = 0x2ffb77; continue;
            }
            0x2ffb30 => {
                fa = ebx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edx = edx.wrapping_add(edx).wrapping_add(0xb);
                m.write8(eax.wrapping_add(0x6), (((((edx) as u8)) as u32)) as u8);
                pc = 0x2ffb77; continue;
            }
            0x2ffb3f => {
                ecx = (ecx & 0xffff_ff00) | ((((((((ecx) as u8)) as i32).wrapping_add(((((ebx) as u8)) as i32))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                m.write8(eax.wrapping_add(0x6), (((((ecx) as u8)) as u32)) as u8);
                if (fa <= fb) { pc = 0x2ffb4d; continue; }
                pc = 0x2ffb49; continue;
            }
            0x2ffb49 => {
                m.write8(eax.wrapping_add(0x6), (0x1) as u8);
                pc = 0x2ffb4d; continue;
            }
            0x2ffb4d => {
                fa = ((((m.read8(eax.wrapping_add(0x6)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if (fa >= fb) { pc = 0x2ffb77; continue; }
                pc = 0x2ffb53; continue;
            }
            0x2ffb53 => {
                m.write8(eax.wrapping_add(0x6), (0x4) as u8);
                pc = 0x2ffb77; continue;
            }
            0x2ffb59 => {
                ecx = (ecx & 0xffff_ff00) | ((((((((ecx) as u8)) as i32).wrapping_add(((((ebx) as u8)) as i32))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0xai32)) as u32) & 0xffu32);
                m.write8(eax.wrapping_add(0x6), (((((ecx) as u8)) as u32)) as u8);
                if (fa <= fb) { pc = 0x2ffb67; continue; }
                pc = 0x2ffb63; continue;
            }
            0x2ffb63 => {
                m.write8(eax.wrapping_add(0x6), (0x7) as u8);
                pc = 0x2ffb67; continue;
            }
            0x2ffb67 => {
                fa = ((((m.read8(eax.wrapping_add(0x6)))) as u32) & 0xffu32);
                fb = ((((0x7i32)) as u32) & 0xffu32);
                if (fa >= fb) { pc = 0x2ffb77; continue; }
                pc = 0x2ffb6d; continue;
            }
            0x2ffb6d => {
                m.write8(eax.wrapping_add(0x6), (0xa) as u8);
                pc = 0x2ffb77; continue;
            }
            0x2ffb73 => {
                m.write8(eax.wrapping_add(0x6), (0) as u8);
                pc = 0x2ffb77; continue;
            }
            0x2ffb77 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x6))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0xbi32)) as u32) & 0xffu32);
                if (fa < fb) { pc = 0x2ffb90; continue; }
                pc = 0x2ffb7f; continue;
            }
            0x2ffb7f => {
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0xei32)) as u32) & 0xffu32);
                if (fa > fb) { pc = 0x2ffb90; continue; }
                pc = 0x2ffb84; continue;
            }
            0x2ffb84 => {
                esi = (esi | 0xffffffffu32);
                ecx = edi;
                call!(0x2fe9a0);
                pc = 0x2ffb8e; continue;
            }
            0x2ffb8e => {
                pc = 0x2ffbe2; continue;
            }
            0x2ffb90 => {
                fa = ebx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax)) as u32)) & 0xffff);
                esi = (esi & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x2))) as u32)) & 0xffff);
                if (fa == fb) { pc = 0x2ffbc0; continue; }
                pc = 0x2ffb9c; continue;
            }
            0x2ffb9c => {
                fa = ebx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ffbe2; continue; }
                pc = 0x2ffba1; continue;
            }
            0x2ffba1 => {
                ecx = (ecx & 0xffff_0000) | ((((m.read16(edi.wrapping_add(0x18))) as u32)) & 0xffff);
                ecx = (ecx & 0xffff_0000) | ((((((((ecx) as u16)) as i32).wrapping_sub(((((esi) as u16)) as i32))) as u32)) & 0xffff);
                write16(m, eax, (((((ecx) as u16)) as u32)) as u16);
                write16(m, eax.wrapping_add(0x2), (((((edx) as u16)) as u32)) as u16);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x6))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0xbi32)) as u32) & 0xffu32);
                if (fa >= fb) { pc = 0x2ffbe2; continue; }
                pc = 0x2ffbb7; continue;
            }
            0x2ffbb7 => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2ffbe2; continue; }
                pc = 0x2ffbbb; continue;
            }
            0x2ffbbb => {
                write16(m, eax, (((((m.read16(eax)) as i32).wrapping_sub(0x1i32)) as u32)) as u16);
                pc = 0x2ffbe2; continue;
            }
            0x2ffbc0 => {
                ecx = 0;
                write16(m, eax, (((((esi) as u16)) as u32)) as u16);
                ecx = (ecx & 0xffff_0000) | ((((m.read16(edi.wrapping_add(0x14))) as u32)) & 0xffff);
                ecx = (ecx & 0xffff_0000) | ((((((((ecx) as u16)) as i32).wrapping_sub(((((edx) as u16)) as i32))) as u32)) & 0xffff);
                write16(m, eax.wrapping_add(0x2), (((((ecx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x6))) as u32)) & 0xff);
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0xbi32)) as u32) & 0xffu32);
                if (fa >= fb) { pc = 0x2ffbe2; continue; }
                pc = 0x2ffbd8; continue;
            }
            0x2ffbd8 => {
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x2ffbe2; continue; }
                pc = 0x2ffbdd; continue;
            }
            0x2ffbdd => {
                ecx = ecx.wrapping_sub(1);
                write16(m, eax.wrapping_add(0x2), (((((ecx) as u16)) as u32)) as u16);
                pc = 0x2ffbe2; continue;
            }
            0x2ffbe2 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(esp.wrapping_add(0xc));
                edx = ((m.read16(eax.wrapping_add(0x386e))) as u16 as u32);
                ecx = ecx.wrapping_add(1);
                ebp = ebp.wrapping_add(0xc);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0xc), ecx);
                if (fas < fbs) { pc = 0x2ffae6; continue; }
                pc = 0x2ffc02; continue;
            }
            0x2ffc02 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x2ffc04; continue;
            }
            0x2ffc04 => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FFC40: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2ffc40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2ffc40;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x2ffc40, previous, pc); previous = pc; }
        match pc {
            0x2ffc40 => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0xdd10));
                esp = esp.wrapping_sub(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x2ffd40; continue; }
                pc = 0x2ffc56; continue;
            }
            0x2ffc56 => {
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = 0;
                m.write32(esp.wrapping_add(0xc), edi);
                m.write32(esp.wrapping_add(0x10), eax);
                pc = 0x2ffc70; continue;
            }
            0x2ffc70 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(edi.wrapping_add(ecx).wrapping_add(0xdd94));
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                edi = edi.wrapping_add(ecx).wrapping_add(0xdd14);
                if (fa != fb) { pc = 0x2ffd20; continue; }
                pc = 0x2ffc8d; continue;
            }
            0x2ffc8d => {
                eax = m.read32(edi.wrapping_add(0x20));
                esi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x14), eax);
                if (fas <= fbs) { pc = 0x2ffd20; continue; }
                pc = 0x2ffc9e; continue;
            }
            0x2ffc9e => {
                ebp = edi.wrapping_add(0x24);
                pc = 0x2ffca1; continue;
            }
            0x2ffca1 => {
                fa = m.read32(ebp);
                fb = ebx;
                if (fa != fb) { pc = 0x2ffd18; continue; }
                pc = 0x2ffca6; continue;
            }
            0x2ffca6 => {
                push!(esi);
                push!(edi);
                call!(0x320fa0);
                pc = 0x2ffcad; continue;
            }
            0x2ffcad => {
                edx = m.read32(ebx.wrapping_add(0xc));
                ecx = m.read32(ebx.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x2c));
                edx = (((((m.read16(eax) as i16)) as i32)) as u32);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                edx = (((((m.read16(eax.wrapping_add(0x2)) as i16)) as i32)) as u32);
                xmm1.lanes[0] = (((edx) as i32) as f32).to_bits();
                edx = m.read32(esp.wrapping_add(0x24));
                esp = esp.wrapping_add(0x8);
                fa = edx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                xmm2 = xmm0;
                if (fa == fb) { pc = 0x2ffcef; continue; }
                pc = 0x2ffcd2; continue;
            }
            0x2ffcd2 => {
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ffd05; continue; }
                pc = 0x2ffcd7; continue;
            }
            0x2ffcd7 => {
                ecx = m.read32(ecx.wrapping_add(0x18));
                xmm3 = Xmm::default();
                ecx = ecx.wrapping_sub(1);
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1 = xmm2;
                pc = 0x2ffd05; continue;
            }
            0x2ffcef => {
                edx = m.read32(ecx.wrapping_add(0x14));
                xmm3 = Xmm::default();
                xmm0 = xmm1;
                edx = edx.wrapping_sub(1);
                xmm1.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                pc = 0x2ffd05; continue;
            }
            0x2ffd05 => {
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                write16(m, eax, (((((ecx) as u16)) as u32)) as u16);
                edx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                write16(m, eax.wrapping_add(0x2), (((((edx) as u16)) as u32)) as u16);
                eax = m.read32(esp.wrapping_add(0x14));
                pc = 0x2ffd18; continue;
            }
            0x2ffd18 => {
                esi = esi.wrapping_add(1);
                ebp = ebp.wrapping_add(0x8);
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2ffca1; continue; }
                pc = 0x2ffd20; continue;
            }
            0x2ffd20 => {
                edi = m.read32(esp.wrapping_add(0xc));
                eax = m.read32(esp.wrapping_add(0x10));
                edi = edi.wrapping_add(0x8c);
                eax = eax.wrapping_sub(1);
                fa = eax;
                m.write32(esp.wrapping_add(0xc), edi);
                m.write32(esp.wrapping_add(0x10), eax);
                if ((fa != 0)) { pc = 0x2ffc70; continue; }
                pc = 0x2ffd3d; continue;
            }
            0x2ffd3d => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x2ffd40; continue;
            }
            0x2ffd40 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x002FFD50: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2ffd50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2ffd50;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x2ffd50, previous, pc); previous = pc; }
        match pc {
            0x2ffd50 => {
                esp = esp.wrapping_sub(0x30);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ffe05; continue; }
                pc = 0x2ffd5b; continue;
            }
            0x2ffd5b => {
                fa = edi;
                fb = ((((0x2fb6b0i32)) as u32) & 0xffffffffu32);
                push!(esi);
                if (fa == fb) { pc = 0x2ffda6; continue; }
                pc = 0x2ffd64; continue;
            }
            0x2ffd64 => {
                fa = edi;
                fb = ((((0x2fb6d0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ffda6; continue; }
                pc = 0x2ffd6c; continue;
            }
            0x2ffd6c => {
                fa = edi;
                fb = ((((0x2fb700i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ffda6; continue; }
                pc = 0x2ffd74; continue;
            }
            0x2ffd74 => {
                fa = edi;
                fb = ((((0x2fb580i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ffd94; continue; }
                pc = 0x2ffd7c; continue;
            }
            0x2ffd7c => {
                push!(0);
                push!(0x1);
                call!(0x30cd40);
                pc = 0x2ffd85; continue;
            }
            0x2ffd85 => {
                push!(0xb8e);
                call!(0x3d7b0);
                pc = 0x2ffd8f; continue;
            }
            0x2ffd8f => {
                esp = esp.wrapping_add(0xc);
                pc = 0x2ffdbe; continue;
            }
            0x2ffd94 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2ffd9f; continue; }
                pc = 0x2ffd98; continue;
            }
            0x2ffd98 => {
                push!(0xb89);
                pc = 0x2ffdb6; continue;
            }
            0x2ffd9f => {
                push!(0xb8a);
                pc = 0x2ffdb6; continue;
            }
            0x2ffda6 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2ffdb1; continue; }
                pc = 0x2ffdaa; continue;
            }
            0x2ffdaa => {
                push!(0xb8c);
                pc = 0x2ffdb6; continue;
            }
            0x2ffdb1 => {
                push!(0xb8d);
                pc = 0x2ffdb6; continue;
            }
            0x2ffdb6 => {
                call!(0x3d7b0);
                pc = 0x2ffdbb; continue;
            }
            0x2ffdbb => {
                esp = esp.wrapping_add(0x4);
                pc = 0x2ffdbe; continue;
            }
            0x2ffdbe => {
                esi = eax;
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x234));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ffdee; continue; }
                pc = 0x2ffdcf; continue;
            }
            0x2ffdcf => {
                ecx = esp.wrapping_add(0x4);
                push!(ecx);
                call!(0x45390);
                pc = 0x2ffdd9; continue;
            }
            0x2ffdd9 => {
                edx = esp.wrapping_add(0x8);
                push!(edx);
                m.write32(esp.wrapping_add(0xc), 0x1575);
                call!(0x45820);
                pc = 0x2ffdeb; continue;
            }
            0x2ffdeb => {
                esp = esp.wrapping_add(0x8);
                pc = 0x2ffdee; continue;
            }
            0x2ffdee => {
                eax = m.read32(esp.wrapping_add(0x38));
                ecx = m.read32(0x7842a8);
                push!(eax);
                push!(edi);
                push!(esi);
                push!(ecx);
                call!(0x7a080);
                pc = 0x2ffe01; continue;
            }
            0x2ffe01 => {
                esp = esp.wrapping_add(0x10);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x2ffe05; continue;
            }
            0x2ffe05 => {
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
