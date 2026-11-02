use super::*;
#[rustfmt::skip] mod unformatted { use super::*;
/// 0x002FFE10: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2ffe10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2ffe10;
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
        { crate::pose::cover(0x2ffe10, previous, pc); previous = pc; }
        match pc {
            0x2ffe10 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f7c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ffe2f; continue; }
                pc = 0x2ffe1f; continue;
            }
            0x2ffe1f => {
                ecx = m.read32(eax.wrapping_add(0x254));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ffe2f; continue; }
                pc = 0x2ffe29; continue;
            }
            0x2ffe29 => {
                eax = 0x2fb7b0;
                break 'run;
            }
            0x2ffe2f => {
                ecx = m.read32(eax.wrapping_add(0x2f18));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ffe49; continue; }
                pc = 0x2ffe39; continue;
            }
            0x2ffe39 => {
                ecx = m.read32(eax.wrapping_add(0x254));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ffe49; continue; }
                pc = 0x2ffe43; continue;
            }
            0x2ffe43 => {
                eax = 0x2fb830;
                break 'run;
            }
            0x2ffe49 => {
                ecx = m.read32(eax.wrapping_add(0x2f1c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ffe73; continue; }
                pc = 0x2ffe53; continue;
            }
            0x2ffe53 => {
                ecx = m.read32(ecx.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2ffed2; continue; }
                pc = 0x2ffe5a; continue;
            }
            0x2ffe5a => {
                push!(ecx);
                call!(0x322190);
                pc = 0x2ffe60; continue;
            }
            0x2ffe60 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                eax = 0x2fb6b0;
                if (fa != fb) { pc = 0x2ffed4; continue; }
                pc = 0x2ffe6d; continue;
            }
            0x2ffe6d => {
                eax = 0x2fb5e0;
                break 'run;
            }
            0x2ffe73 => {
                ecx = m.read32(eax.wrapping_add(0x2f38));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2ffea0; continue; }
                pc = 0x2ffe7d; continue;
            }
            0x2ffe7d => {
                edx = m.read32(eax.wrapping_add(0x2f34));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2ffea0; continue; }
                pc = 0x2ffe87; continue;
            }
            0x2ffe87 => {
                push!(ecx);
                call!(0x322190);
                pc = 0x2ffe8d; continue;
            }
            0x2ffe8d => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                eax = 0x2fb6d0;
                if (fa != fb) { pc = 0x2ffed4; continue; }
                pc = 0x2ffe9a; continue;
            }
            0x2ffe9a => {
                eax = 0x2fb5c0;
                break 'run;
            }
            0x2ffea0 => {
                ecx = m.read32(eax.wrapping_add(0x2f78));
                ecx = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                eax = (eax & 0xffff_0000) | ((((m.read16(eax.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x307a))) as u32)) & 0xffff);
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas < fbs) { pc = 0x2ffed2; continue; }
                pc = 0x2ffeb6; continue;
            }
            0x2ffeb6 => {
                eax = ((((eax) as u16)) as u16 as i16 as i32 as u32);
                push!(eax);
                call!(0x322190);
                pc = 0x2ffebf; continue;
            }
            0x2ffebf => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                eax = 0x2fb700;
                if (fa != fb) { pc = 0x2ffed4; continue; }
                pc = 0x2ffecc; continue;
            }
            0x2ffecc => {
                eax = 0x2fb670;
                break 'run;
            }
            0x2ffed2 => {
                eax = 0;
                pc = 0x2ffed4; continue;
            }
            0x2ffed4 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
/// 0x002FFF70: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fff70<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fff70;
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
        { crate::pose::cover(0x2fff70, previous, pc); previous = pc; }
        match pc {
            0x2fff70 => {
                esp = esp.wrapping_sub(0x1c);
                eax = m.read32(0x7844a8);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = m.read32(eax.wrapping_add(0x2f7c));
                ebp = 0;
                ebx = 0;
                fa = edi;
                fb = ebp;
                esi = edi;
                m.write32(esp.wrapping_add(0x10), ebp);
                if (fa == fb) { pc = 0x300090; continue; }
                pc = 0x2fff94; continue;
            }
            0x2fff94 => {
                push!(edi);
                call!(0x3162b0);
                pc = 0x2fff9a; continue;
            }
            0x2fff9a => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fffa2; continue; }
                pc = 0x2fffa1; continue;
            }
            0x2fffa1 => {
                ebx = ebx.wrapping_add(1);
                pc = 0x2fffa2; continue;
            }
            0x2fffa2 => {
                edi = m.read32(edi.wrapping_add(0x40));
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fff94; continue; }
                pc = 0x2fffa9; continue;
            }
            0x2fffa9 => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x300090; continue; }
                pc = 0x2fffb1; continue;
            }
            0x2fffb1 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x300090; continue; }
                pc = 0x2fffb9; continue;
            }
            0x2fffb9 => {
                ebx = 0x2;
                edi = 0x1;
                pc = 0x2fffc3; continue;
            }
            0x2fffc3 => {
                push!(0x5);
                push!(esi);
                call!(0x3163d0);
                pc = 0x2fffcb; continue;
            }
            0x2fffcb => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x300002; continue; }
                pc = 0x2fffd2; continue;
            }
            0x2fffd2 => {
                ecx = m.read32(0x7844a8);
                xmm0.lanes[0] = (((m.read32(ecx.wrapping_add(0x264))) as i32) as f32).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(eax.wrapping_add(0x8))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x300002; continue; }
                pc = 0x2fffe6; continue;
            }
            0x2fffe6 => {
                push!(0xffffffffu32);
                push!(esi);
                call!(0x316200);
                pc = 0x2fffee; continue;
            }
            0x2fffee => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x300002; continue; }
                pc = 0x2ffff5; continue;
            }
            0x2ffff5 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x14))) as u32)) & 0xff);
                ebp = ebp.wrapping_add(1);
                eax = (eax & 0xffff_ff00) | (((((((((eax) as u8)) as i32) | ((((ebx) as u8)) as i32))) as u32)) & 0xff);
                m.write8(esi.wrapping_add(0x14), (((((eax) as u8)) as u32)) as u8);
                m.write32(esp.wrapping_add(0x10), edi);
                pc = 0x300002; continue;
            }
            0x300002 => {
                esi = m.read32(esi.wrapping_add(0x40));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fffc3; continue; }
                pc = 0x300009; continue;
            }
            0x300009 => {
                eax = m.read32(esp.wrapping_add(0x10));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x300090; continue; }
                pc = 0x300011; continue;
            }
            0x300011 => {
                push!(0xb91);
                m.write32(esp.wrapping_add(0x18), 0x2ffee0);
                m.write32(esp.wrapping_add(0x1c), 0x2fb910);
                m.write32(esp.wrapping_add(0x20), 0x2fb580);
                call!(0x3d7b0);
                pc = 0x300033; continue;
            }
            0x300033 => {
                push!(0xb92);
                m.write32(esp.wrapping_add(0x28), eax);
                call!(0x3d7b0);
                pc = 0x300041; continue;
            }
            0x300041 => {
                push!(0x180e);
                m.write32(esp.wrapping_add(0x30), eax);
                call!(0x3d7b0);
                pc = 0x30004f; continue;
            }
            0x30004f => {
                push!(esi);
                push!(edi);
                m.write32(esp.wrapping_add(0x3c), eax);
                call!(0x30cd40);
                pc = 0x30005a; continue;
            }
            0x30005a => {
                esp = esp.wrapping_add(0x14);
                ecx = 0;
                fa = ebp;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(0x3);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fas > fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(ebx);
                edx = esp.wrapping_add(0x1c);
                push!(edx);
                eax = esp.wrapping_add(0x2c);
                push!(eax);
                ecx = ecx.wrapping_add(0xb8f);
                push!(ecx);
                call!(0x3d7b0);
                pc = 0x30007d; continue;
            }
            0x30007d => {
                edx = m.read32(0x7842a8);
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(edx);
                call!(0x79e40);
                pc = 0x30008d; continue;
            }
            0x30008d => {
                esp = esp.wrapping_add(0x18);
                pc = 0x300090; continue;
            }
            0x300090 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x1c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003000A0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3000a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3000a0;
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
        { crate::pose::cover(0x3000a0, previous, pc); previous = pc; }
        match pc {
            0x3000a0 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(ebx);
                push!(esi);
                ebx = 0x20;
                if (fa == fb) { pc = 0x3002e4; continue; }
                pc = 0x3000af; continue;
            }
            0x3000af => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x25c));
                xmm0.lanes[0] = (((m.read32(ecx.wrapping_add(0x264))) as i32) as f32).to_bits();
                esp = esp.wrapping_sub(0xc);
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                fa = eax;
                fb = ((((0x100000i32)) as u32) & 0xffffffffu32);
                eax = m.read32(0x7ba9e0);
                if ((fa & fb) == 0) { pc = 0x300264; continue; }
                pc = 0x3000e4; continue;
            }
            0x3000e4 => {
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba284)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f20))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba280)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd4))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                m.write32(esp, xmm1.lanes[0]);
                call!(0x307e60);
                pc = 0x30013a; continue;
            }
            0x30013a => {
                ecx = m.read32(0x7844a8);
                m.write32(ecx.wrapping_add(0x2f18), eax);
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x2f18));
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3003cb; continue; }
                pc = 0x30015d; continue;
            }
            0x30015d => {
                push!(0x31a0c0);
                push!(0x783a60);
                push!(0xffffffffu32);
                push!(eax);
                call!(0x31a130);
                pc = 0x30016f; continue;
            }
            0x30016f => {
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3001a0; continue; }
                pc = 0x300176; continue;
            }
            0x300176 => {
                eax = m.read32(0x7844a8);
                fa = ((((m.read8(eax.wrapping_add(0x238)))) as u32) & 0xffu32);
                fb = ((((((ebx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x3001a0; continue; }
                pc = 0x300183; continue;
            }
            0x300183 => {
                push!(0x8);
                call!(0x3370a0);
                pc = 0x30018a; continue;
            }
            0x30018a => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | ebx);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x3001a0; continue;
            }
            0x3001a0 => {
                push!(0x157a);
                call!(0x2f79e0);
                pc = 0x3001aa; continue;
            }
            0x3001aa => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f18));
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x14))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x3001fd; continue; }
                pc = 0x3001c1; continue;
            }
            0x3001c1 => {
                ecx = m.read32(0x783a60);
                esp = esp.wrapping_sub(0x14);
                edx = esp;
                m.write32(edx, ecx);
                ecx = m.read32(0x783a64);
                m.write32(edx.wrapping_add(0x4), ecx);
                ecx = m.read32(0x783a68);
                m.write32(edx.wrapping_add(0x8), ecx);
                ecx = m.read32(0x783a6c);
                m.write32(edx.wrapping_add(0xc), ecx);
                ecx = m.read32(0x783a70);
                push!(eax);
                m.write32(edx.wrapping_add(0x10), ecx);
                call!(0x2febb0);
                pc = 0x3001f8; continue;
            }
            0x3001f8 => {
                esp = esp.wrapping_add(0x18);
                pc = 0x300250; continue;
            }
            0x3001fd => {
                esi = m.read32(0x78400c);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30024b; continue; }
                pc = 0x300207; continue;
            }
            0x300207 => {
                fa = ((((m.read8(esi.wrapping_add(0x14)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x300244; continue; }
                pc = 0x30020d; continue;
            }
            0x30020d => {
                eax = m.read32(0x783a60);
                ecx = m.read32(0x783a64);
                esp = esp.wrapping_sub(0x14);
                edx = esp;
                m.write32(edx, eax);
                eax = m.read32(0x783a68);
                m.write32(edx.wrapping_add(0x4), ecx);
                ecx = m.read32(0x783a6c);
                m.write32(edx.wrapping_add(0x8), eax);
                eax = m.read32(0x783a70);
                m.write32(edx.wrapping_add(0xc), ecx);
                push!(esi);
                m.write32(edx.wrapping_add(0x10), eax);
                call!(0x2febb0);
                pc = 0x300241; continue;
            }
            0x300241 => {
                esp = esp.wrapping_add(0x18);
                pc = 0x300244; continue;
            }
            0x300244 => {
                esi = m.read32(esi.wrapping_add(0x40));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x300207; continue; }
                pc = 0x30024b; continue;
            }
            0x30024b => {
                call!(0x2f92e0);
                pc = 0x300250; continue;
            }
            0x300250 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x25c), (m.read32(eax.wrapping_add(0x25c)) & 0xffefffffu32));
                pc = 0x3003cb; continue;
            }
            0x300264 => {
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba284)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f20))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba280)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd4))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                m.write32(esp, xmm1.lanes[0]);
                call!(0x307e60);
                pc = 0x3002ba; continue;
            }
            0x3002ba => {
                ecx = m.read32(0x7844a8);
                m.write32(ecx.wrapping_add(0x2f18), eax);
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x2f18));
                esp = esp.wrapping_add(0xc);
                call!(0x2fb020);
                pc = 0x3002da; continue;
            }
            0x3002da => {
                call!(0x73c80);
                pc = 0x3002df; continue;
            }
            0x3002df => {
                pc = 0x3003cb; continue;
            }
            0x3002e4 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ecx);
                call!(0x77470);
                pc = 0x3002fb; continue;
            }
            0x3002fb => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (((((((((((((fa)) as u8)) as i32).wrapping_sub((((((fb)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) == 0i32)) { pc = 0x3003cb; continue; }
                pc = 0x30030c; continue;
            }
            0x30030c => {
                eax = m.read32(0x7844a8);
                fa = m.read32(eax.wrapping_add(0x25c));
                fb = ((((0x100000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x30033b; continue; }
                pc = 0x30031d; continue;
            }
            0x30031d => {
                push!(0x1577);
                call!(0x2f79e0);
                pc = 0x300327; continue;
            }
            0x300327 => {
                push!(0x1);
                push!(0x783a60);
                call!(0x319480);
                pc = 0x300333; continue;
            }
            0x300333 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x300250; continue;
            }
            0x30033b => {
                eax = m.read32(eax.wrapping_add(0x2f18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3003cb; continue; }
                pc = 0x300349; continue;
            }
            0x300349 => {
                push!(eax);
                call!(0x31a1b0);
                pc = 0x30034f; continue;
            }
            0x30034f => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x300381; continue; }
                pc = 0x300356; continue;
            }
            0x300356 => {
                edx = m.read32(0x7844a8);
                fa = ((((m.read8(edx.wrapping_add(0x238)))) as u32) & 0xffu32);
                fb = ((((((ebx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x300381; continue; }
                pc = 0x300364; continue;
            }
            0x300364 => {
                push!(0x9);
                call!(0x3370a0);
                pc = 0x30036b; continue;
            }
            0x30036b => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | ebx);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x300381; continue;
            }
            0x300381 => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x2f18));
                fa = ((((m.read8(eax.wrapping_add(0x14)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x3003a2; continue; }
                pc = 0x300392; continue;
            }
            0x300392 => {
                eax = eax.wrapping_add(0x18);
                push!(0x1);
                push!(eax);
                call!(0x319480);
                pc = 0x30039d; continue;
            }
            0x30039d => {
                esp = esp.wrapping_add(0x8);
                pc = 0x3003cb; continue;
            }
            0x3003a2 => {
                esi = m.read32(0x78400c);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3003cb; continue; }
                pc = 0x3003ac; continue;
            }
            0x3003ac => {
                pc = 0x3003b0; continue;
            }
            0x3003b0 => {
                fa = ((((m.read8(esi.wrapping_add(0x14)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3003c4; continue; }
                pc = 0x3003b6; continue;
            }
            0x3003b6 => {
                ecx = esi.wrapping_add(0x18);
                push!(0x1);
                push!(ecx);
                call!(0x319480);
                pc = 0x3003c1; continue;
            }
            0x3003c1 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x3003c4; continue;
            }
            0x3003c4 => {
                esi = m.read32(esi.wrapping_add(0x40));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3003b0; continue; }
                pc = 0x3003cb; continue;
            }
            0x3003cb => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x3003e3; continue;
            }
            0x3003e3 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30043e; continue; }
                pc = 0x3003f1; continue;
            }
            0x3003f1 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f18));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30043e; continue; }
                pc = 0x300400; continue;
            }
            0x300400 => {
                ecx = ecx.wrapping_add(0x18);
                edx = m.read32(ecx);
                m.write32(0x783a60, edx);
                edx = m.read32(ecx.wrapping_add(0x4));
                m.write32(0x783a64, edx);
                edx = m.read32(ecx.wrapping_add(0x8));
                m.write32(0x783a68, edx);
                edx = m.read32(ecx.wrapping_add(0xc));
                m.write32(0x783a6c, edx);
                ecx = m.read32(ecx.wrapping_add(0x10));
                m.write32(0x783a70, ecx);
                m.write32(eax.wrapping_add(0x25c), (m.read32(eax.wrapping_add(0x25c)) | 0x100000));
                call!(0x73c80);
                pc = 0x30043e; continue;
            }
            0x30043e => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x300456; continue;
            }
            0x300456 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x300489; continue; }
                pc = 0x300463; continue;
            }
            0x300463 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x300489; continue; }
                pc = 0x300473; continue;
            }
            0x300473 => {
                eax = eax.wrapping_add(0x18);
                push!(eax);
                call!(0x31aaa0);
                pc = 0x30047c; continue;
            }
            0x30047c => {
                push!(0x52e010);
                call!(0x2fb230);
                pc = 0x300486; continue;
            }
            0x300486 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x300489; continue;
            }
            0x300489 => {
                push!(0);
                call!(0x427b0);
                pc = 0x300490; continue;
            }
            0x300490 => {
                esp = esp.wrapping_add(0x4);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0xci32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3004d3; continue; }
                pc = 0x300497; continue;
            }
            0x300497 => {
                edx = m.read32(0x7844a8);
                ecx = m.read32(edx.wrapping_add(0x2f18));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3004d3; continue; }
                pc = 0x3004a7; continue;
            }
            0x3004a7 => {
                ecx = m.read32(ecx.wrapping_add(0x1c));
                fa = ecx;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3004b4; continue; }
                pc = 0x3004af; continue;
            }
            0x3004af => {
                fa = ecx;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3004d3; continue; }
                pc = 0x3004b4; continue;
            }
            0x3004b4 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3004c5; continue; }
                pc = 0x3004b8; continue;
            }
            0x3004b8 => {
                push!(0x1);
                call!(0x319f40);
                pc = 0x3004bf; continue;
            }
            0x3004bf => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x3004c5 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3004d3; continue; }
                pc = 0x3004c9; continue;
            }
            0x3004c9 => {
                push!(0);
                call!(0x319f40);
                pc = 0x3004d0; continue;
            }
            0x3004d0 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x3004d3; continue;
            }
            0x3004d3 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003005F0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3005f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3005f0;
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
        { crate::pose::cover(0x3005f0, previous, pc); previous = pc; }
        match pc {
            0x3005f0 => {
                push!(edi);
                edi = eax;
                call!(0x2fc340);
                pc = 0x3005f8; continue;
            }
            0x3005f8 => {
                edi = eax;
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30061b; continue; }
                pc = 0x3005fe; continue;
            }
            0x3005fe => {
                edi = edi;
                pc = 0x300600; continue;
            }
            0x300600 => {
                push!(edi);
                call!(0x2fc470);
                pc = 0x300606; continue;
            }
            0x300606 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0x2000000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x30061f; continue; }
                pc = 0x300610; continue;
            }
            0x300610 => {
                call!(0x2fc340);
                pc = 0x300615; continue;
            }
            0x300615 => {
                edi = eax;
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x300600; continue; }
                pc = 0x30061b; continue;
            }
            0x30061b => {
                eax = 0;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x30061f => {
                eax = 0x1;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00300630: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f300630<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x300630;
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
        { crate::pose::cover(0x300630, previous, pc); previous = pc; }
        match pc {
            0x300630 => {
                eax = m.read32(0x784014);
                fa = ((((m.read8(eax))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                push!(esi);
                if (fa == fb) { pc = 0x30065d; continue; }
                pc = 0x30063b; continue;
            }
            0x30063b => {
                push!(0xc77668);
                push!(eax);
                call!(0x32a5b0);
                pc = 0x300646; continue;
            }
            0x300646 => {
                esi = ((m.read8(eax.wrapping_add(0x9))) as u8 as u32);
                esp = esp.wrapping_add(0x8);
                call!(0x2fde80);
                pc = 0x300652; continue;
            }
            0x300652 => {
                ecx = 0;
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ecx;
                break 'run;
            }
            0x30065d => {
                esi = 0;
                call!(0x2fde80);
                pc = 0x300664; continue;
            }
            0x300664 => {
                ecx = 0;
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ecx;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00300670: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f300670<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x300670;
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
        { crate::pose::cover(0x300670, previous, pc); previous = pc; }
        match pc {
            0x300670 => {
                eax = m.read32(0x784014);
                fa = m.read32(eax);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3006a1; continue; }
                pc = 0x30067a; continue;
            }
            0x30067a => {
                push!(0xc77668);
                push!(eax);
                call!(0x32a5b0);
                pc = 0x300685; continue;
            }
            0x300685 => {
                eax = m.read32(eax.wrapping_add(0x248));
                ecx = m.read32(esp.wrapping_add(0xc));
                eax = eax.wrapping_add(ecx);
                edx = 0;
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0x7fe0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                edx = (edx & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                eax = edx;
                break 'run;
            }
            0x3006a1 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                eax = 0;
                eax = ecx;
                edx = 0;
                fa = eax;
                fb = ((((0x7fe0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                edx = (edx & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                eax = edx;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003031B0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3031b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3031b0;
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
        { crate::pose::cover(0x3031b0, previous, pc); previous = pc; }
        match pc {
            0x3031b0 => {
                push!(ebx);
                ebx = m.read32(0x7844a8);
                push!(esi);
                push!(edi);
                eax = 0;
                ecx = 0x20;
                edi = 0x762c30;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                edi = m.read32(esp.wrapping_add(0x10));
                esi = edi;
                esi = ((esi as i32).wrapping_mul((0x8c) as i32)) as u32;
                eax = m.read32(esi.wrapping_add(ebx).wrapping_add(0xdd98));
                fa = eax;
                fb = ((((0x35i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x303303; continue; }
                pc = 0x3031e3; continue;
            }
            0x3031e3 => {
                fa = eax;
                fb = ((((0x37i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x303297; continue; }
                pc = 0x3031ec; continue;
            }
            0x3031ec => {
                fa = eax;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x303303; continue; }
                pc = 0x3031f5; continue;
            }
            0x3031f5 => {
                push!(edi);
                call!(0x2fe4e0);
                pc = 0x3031fb; continue;
            }
            0x3031fb => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303303; continue; }
                pc = 0x303207; continue;
            }
            0x303207 => {
                esi = eax.wrapping_add(eax.wrapping_mul(0x2));
                esi = esi.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                fa = ((((m.read8(esi.wrapping_add(ebx).wrapping_add(0x3079)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30321e; continue; }
                pc = 0x303217; continue;
            }
            0x303217 => {
                push!(0x9fe);
                pc = 0x303223; continue;
            }
            0x30321e => {
                push!(0x9fd);
                pc = 0x303223; continue;
            }
            0x303223 => {
                call!(0x3d7b0);
                pc = 0x303228; continue;
            }
            0x303228 => {
                ebx = eax;
                eax = m.read32(0x7844a8);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(eax).wrapping_add(0x3079))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x303245; continue; }
                pc = 0x30323e; continue;
            }
            0x30323e => {
                push!(0x9ff);
                pc = 0x30324a; continue;
            }
            0x303245 => {
                push!(0xa00);
                pc = 0x30324a; continue;
            }
            0x30324a => {
                call!(0x3d7b0);
                pc = 0x30324f; continue;
            }
            0x30324f => {
                ecx = m.read32(0x7844a8);
                edi = eax;
                eax = (eax & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(ecx).wrapping_add(0x3079))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30326c; continue; }
                pc = 0x303265; continue;
            }
            0x303265 => {
                push!(0xa01);
                pc = 0x303271; continue;
            }
            0x30326c => {
                push!(0xa02);
                pc = 0x303271; continue;
            }
            0x303271 => {
                call!(0x3d7b0);
                pc = 0x303276; continue;
            }
            0x303276 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(edi);
                push!(ebx);
                push!(0x49f784);
                push!(0x762c30);
                call!(0x3c85d4);
                pc = 0x30328b; continue;
            }
            0x30328b => {
                esp = esp.wrapping_add(0x14);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x762c30;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x303297 => {
                push!(edi);
                push!(0);
                call!(0x2fe480);
                pc = 0x30329f; continue;
            }
            0x30329f => {
                ebx = eax;
                esp = esp.wrapping_add(0x8);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3032fd; continue; }
                pc = 0x3032a8; continue;
            }
            0x3032a8 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(ebx.wrapping_add(0x2c))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (((((((((((((fa)) as u8)) as i32).wrapping_sub((((((fb)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) == 0i32)) { pc = 0x3032b6; continue; }
                pc = 0x3032af; continue;
            }
            0x3032af => {
                push!(0x9e9);
                pc = 0x3032bb; continue;
            }
            0x3032b6 => {
                push!(0x9e8);
                pc = 0x3032bb; continue;
            }
            0x3032bb => {
                call!(0x3d7b0);
                pc = 0x3032c0; continue;
            }
            0x3032c0 => {
                esi = eax;
                eax = (eax & 0xffff_ff00) | ((((m.read8(ebx.wrapping_add(0x2c))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3032d3; continue; }
                pc = 0x3032cc; continue;
            }
            0x3032cc => {
                push!(0x9ea);
                pc = 0x3032d8; continue;
            }
            0x3032d3 => {
                push!(0x9eb);
                pc = 0x3032d8; continue;
            }
            0x3032d8 => {
                call!(0x3d7b0);
                pc = 0x3032dd; continue;
            }
            0x3032dd => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(esi);
                push!(0x49f77c);
                push!(0x762c30);
                call!(0x3c85d4);
                pc = 0x3032f1; continue;
            }
            0x3032f1 => {
                esp = esp.wrapping_add(0x10);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x762c30;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x3032fd => {
                ebx = m.read32(0x7844a8);
                pc = 0x303303; continue;
            }
            0x303303 => {
                fa = m.read32(esi.wrapping_add(ebx).wrapping_add(0xdd94));
                fb = ((((0xbi32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x303345; continue; }
                pc = 0x30330d; continue;
            }
            0x30330d => {
                push!(edi);
                call!(0x2fe4e0);
                pc = 0x303313; continue;
            }
            0x303313 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303345; continue; }
                pc = 0x30331b; continue;
            }
            0x30331b => {
                edx = eax.wrapping_add(eax.wrapping_mul(0x2));
                fa = ((((m.read8(ebx.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x3079)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                if ((fa & fb) == 0) { pc = 0x303338; continue; }
                pc = 0x30332b; continue;
            }
            0x30332b => {
                m.write32(esp.wrapping_add(0x4), 0x9fd);
                tail_to!(0x3d7b0); break 'run;  // direct tail jump
            }
            0x303338 => {
                m.write32(esp.wrapping_add(0x4), 0x9fe);
                tail_to!(0x3d7b0); break 'run;  // direct tail jump
            }
            0x303345 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00303350: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f303350<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x303350;
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
        { crate::pose::cover(0x303350, previous, pc); previous = pc; }
        match pc {
            0x303350 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                eax = esi.wrapping_add(((0x11i32.wrapping_neg()) as u32));
                fa = eax;
                fb = ((((0x30i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x3033e4; continue; }
                pc = 0x303361; continue;
            }
            0x303361 => {
                eax = ((m.read8(eax.wrapping_add(0x3033fc))) as u8 as u32);
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x3033e8));
                if (jt == 0x30336fu32) { pc = 0x30336f; continue; }
                if (jt == 0x303390u32) { pc = 0x303390; continue; }
                if (jt == 0x30339eu32) { pc = 0x30339e; continue; }
                if (jt == 0x3033cdu32) { pc = 0x3033cd; continue; }
                if (jt == 0x3033e4u32) { pc = 0x3033e4; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x30336f => {
                push!(0);
                call!(0x2d3a30);
                pc = 0x303376; continue;
            }
            0x303376 => {
                esi = m.read32(eax.wrapping_add(esi.wrapping_mul(0x4)).wrapping_add(((0x3ci32.wrapping_neg()) as u32)));
                esp = esp.wrapping_add(0x4);
                esi = ((esi as i32).wrapping_mul((0x54) as i32)) as u32;
                ecx = m.read32(esi.wrapping_add(0x4fb190));
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esp.wrapping_add(0x4), ecx);
                tail_to!(0x3d7b0); break 'run;  // direct tail jump
            }
            0x303390 => {
                esi = esi.wrapping_add(0xffffffe9u32);
                push!(esi);
                call!(0x2fe7a0);
                pc = 0x303399; continue;
            }
            0x303399 => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x30339e => {
                call!(0x3129d0);
                pc = 0x3033a3; continue;
            }
            0x3033a3 => {
                push!(eax);
                call!(0x2c0290);
                pc = 0x3033a9; continue;
            }
            0x3033a9 => {
                ecx = (((((m.read16(eax.wrapping_add(esi.wrapping_mul(0x8)).wrapping_add(((0x1bci32.wrapping_neg()) as u32))) as i16)) as i32)) as u32);
                eax = (((((m.read16(eax.wrapping_add(esi.wrapping_mul(0x8)).wrapping_add(((0x1b6i32.wrapping_neg()) as u32))) as i16)) as i32)) as u32);
                push!(eax);
                push!(ecx);
                call!(0x2be650);
                pc = 0x3033c0; continue;
            }
            0x3033c0 => {
                esp = esp.wrapping_add(0xc);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esp.wrapping_add(0x4), eax);
                tail_to!(0x3d7b0); break 'run;  // direct tail jump
            }
            0x3033cd => {
                esi = ((esi as i32).wrapping_mul((0xc4) as i32)) as u32;
                edx = (((((m.read16(esi.wrapping_add(0x524f58)) as i16)) as i32)) as u32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esp.wrapping_add(0x4), edx);
                tail_to!(0x3d7b0); break 'run;  // direct tail jump
            }
            0x3033e4 => {
                eax = 0;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00303430: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f303430<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x303430;
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
        { crate::pose::cover(0x303430, previous, pc); previous = pc; }
        match pc {
            0x303430 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(esi);
                push!(eax);
                call!(0x303350);
                pc = 0x30343d; continue;
            }
            0x30343d => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30345d; continue; }
                pc = 0x303444; continue;
            }
            0x303444 => {
                ecx = m.read32(esi);
                ecx = ((ecx as i32).wrapping_mul((0xc4) as i32)) as u32;
                edx = (((((m.read16(ecx.wrapping_add(0x524f58)) as i16)) as i32)) as u32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(esp.wrapping_add(0x4), edx);
                tail_to!(0x3d7b0); break 'run;  // direct tail jump
            }
            0x30345d => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00303460: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f303460<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x303460;
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
        { crate::pose::cover(0x303460, previous, pc); previous = pc; }
        match pc {
            0x303460 => {
                push!(ebp);
                ebp = esp;
                esp = (esp & 0xfffffff8u32);
                esp = esp.wrapping_sub(0xd4);
                eax = (eax & 0xffff_0000) | ((((m.read16(ebp.wrapping_add(0xc))) as u32)) & 0xffff);
                ecx = m.read32(ebp.wrapping_add(0xc));
                push!(ebx);
                push!(esi);
                push!(edi);
                ecx = ecx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(0xffffffffu32);
                write16(m, esp.wrapping_add(0x14), (((((eax) as u16)) as u32)) as u16);
                eax = (eax & 0xffff_0000) | ((((m.read16(ebp.wrapping_add(0x10))) as u32)) & 0xffff);
                esp = esp.wrapping_sub(0x8);
                write16(m, esp.wrapping_add(0x1e), (((((ecx) as u16)) as u32)) as u16);
                ecx = m.read32(esp.wrapping_add(0x1c));
                edx = esp;
                m.write32(edx, ecx);
                write16(m, edx.wrapping_add(0x4), (((((eax) as u16)) as u32)) as u16);
                edx = m.read32(0x778d34);
                eax = m.read32(edx.wrapping_add(0x8c));
                push!(eax);
                ebx = (ebx | 0xffffffffu32);
                call!(0x308260);
                pc = 0x3034ad; continue;
            }
            0x3034ad => {
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3034b6; continue; }
                pc = 0x3034b4; continue;
            }
            0x3034b4 => {
                ebx = m.read32(eax);
                pc = 0x3034b6; continue;
            }
            0x3034b6 => {
                esi = m.read32(0x78400c);
                eax = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = 0x25;
                edi = esp.wrapping_add(0x48);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                if (fa == fb) { pc = 0x3034e8; continue; }
                pc = 0x3034cd; continue;
            }
            0x3034cd => {
                pc = 0x3034d0; continue;
            }
            0x3034d0 => {
                edx = m.read32(esi.wrapping_add(0x3c));
                ecx = esp.wrapping_add(0x48);
                push!(ecx);
                push!(edx);
                call!(0x308420);
                pc = 0x3034de; continue;
            }
            0x3034de => {
                esi = m.read32(esi.wrapping_add(0x40));
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3034d0; continue; }
                pc = 0x3034e8; continue;
            }
            0x3034e8 => {
                edi = m.read32(ebp.wrapping_add(0x14));
                eax = m.read32(edi);
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                eax = (((((m.read16(eax.wrapping_add(0x524f5a)) as i16)) as i32)) as u32);
                ecx = m.read32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x48));
                eax = esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x48);
                ecx = ecx.wrapping_add(1);
                fa = ebx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                m.write32(eax, ecx);
                if (fa == fb) { pc = 0x303522; continue; }
                pc = 0x30350a; continue;
            }
            0x30350a => {
                ebx = ((ebx as i32).wrapping_mul((0xc4) as i32)) as u32;
                eax = (((((m.read16(ebx.wrapping_add(0x524f5a)) as i16)) as i32)) as u32);
                ecx = m.read32(esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x48));
                eax = esp.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x48);
                ecx = ecx.wrapping_sub(1);
                m.write32(eax, ecx);
                pc = 0x303522; continue;
            }
            0x303522 => {
                ebx = 0;
                esi = 0x524be4;
                pc = 0x303530; continue;
            }
            0x303530 => {
                eax = m.read32(edi);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                ecx = m.read32(esi.wrapping_add(0xc));
                if (fa == fb) { pc = 0x30355d; continue; }
                pc = 0x30353a; continue;
            }
            0x30353a => {
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                edx = (((((m.read16(eax.wrapping_add(0x524f5a)) as i16)) as i32)) as u32);
                fa = edx;
                fb = ebx;
                if (fa != fb) { pc = 0x30355d; continue; }
                pc = 0x30354b; continue;
            }
            0x30354b => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30355d; continue; }
                pc = 0x30354f; continue;
            }
            0x30354f => {
                call_to!(ecx);
                pc = 0x303551; continue;
            }
            0x303551 => {
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                fa = eax;
                if ((fa == 0)) { pc = 0x3035e0; continue; }
                pc = 0x30355d; continue;
            }
            0x30355d => {
                eax = m.read32(esp.wrapping_add(ebx.wrapping_mul(0x4)).wrapping_add(0x48));
                fa = eax;
                fb = m.read32(esi);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x3035e0; continue; }
                pc = 0x303565; continue;
            }
            0x303565 => {
                esi = esi.wrapping_add(0x18);
                ebx = ebx.wrapping_add(1);
                fa = esi;
                fb = ((((0x524f5ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x303530; continue; }
                pc = 0x303571; continue;
            }
            0x303571 => {
                ecx = m.read32(0x7844a8);
                eax = (eax & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x238))) as u32)) & 0xff);
                ebx = 0x20;
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((eax) as u8))) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x3035a3; continue; }
                pc = 0x303586; continue;
            }
            0x303586 => {
                push!(0x2);
                call!(0x3370a0);
                pc = 0x30358d; continue;
            }
            0x30358d => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | ebx);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x3035a3; continue;
            }
            0x3035a3 => {
                edx = m.read32(0x7844a8);
                fa = m.read32(edx.wrapping_add(0x25c));
                fb = ((((0x410000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x303722; continue; }
                pc = 0x3035b9; continue;
            }
            0x3035b9 => {
                eax = m.read32(0x784014);
                fa = m.read32(eax);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303668; continue; }
                pc = 0x3035c7; continue;
            }
            0x3035c7 => {
                push!(0xc77668);
                push!(eax);
                call!(0x32a5b0);
                pc = 0x3035d2; continue;
            }
            0x3035d2 => {
                eax = m.read32(eax.wrapping_add(0x248));
                esp = esp.wrapping_add(0x8);
                pc = 0x30366a; continue;
            }
            0x3035e0 => {
                push!(ebx);
                call!(0x308460);
                pc = 0x3035e6; continue;
            }
            0x3035e6 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                ecx = ebx.wrapping_add(ebx.wrapping_mul(0x2));
                edx = m.read32(ecx.wrapping_mul(0x8).wrapping_add(0x524be4));
                push!(edx);
                push!(0x49f794);
                push!(0x9b8);
                call!(0x3d7b0);
                pc = 0x303604; continue;
            }
            0x303604 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(0x7844a8);
                push!(eax);
                call!(0x3d7f0);
                pc = 0x303613; continue;
            }
            0x303613 => {
                ecx = m.read32(ebp.wrapping_add(0x18));
                edx = m.read32(0x7844a8);
                eax = m.read32(0x7842a8);
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x7a270);
                pc = 0x303629; continue;
            }
            0x303629 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x234));
                esp = esp.wrapping_add(0x20);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303770; continue; }
                pc = 0x303640; continue;
            }
            0x303640 => {
                edx = esp.wrapping_add(0x18);
                push!(edx);
                call!(0x45390);
                pc = 0x30364a; continue;
            }
            0x30364a => {
                eax = esp.wrapping_add(0x1c);
                push!(eax);
                m.write32(esp.wrapping_add(0x20), 0x1575);
                call!(0x45820);
                pc = 0x30365c; continue;
            }
            0x30365c => {
                esp = esp.wrapping_add(0x8);
                eax = 0;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x303668 => {
                eax = 0;
                pc = 0x30366a; continue;
            }
            0x30366a => {
                eax = eax.wrapping_add(0x1c);
                fa = eax;
                fb = ((((0x7fe0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x303722; continue; }
                pc = 0x303678; continue;
            }
            0x303678 => {
                edi = m.read32(esp.wrapping_add(0x10));
                ebx = m.read32(ebp.wrapping_add(0x8));
                esi = (esi & 0xffff_0000) | ((((m.read16(ebp.wrapping_add(0x10))) as u32)) & 0xffff);
                push!(0xffffffffu32);
                esp = esp.wrapping_sub(0x8);
                eax = esp;
                m.write32(eax, edi);
                push!(ebx);
                write16(m, eax.wrapping_add(0x4), (((((esi) as u16)) as u32)) as u16);
                call!(0x308260);
                pc = 0x303696; continue;
            }
            0x303696 => {
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3036a7; continue; }
                pc = 0x30369d; continue;
            }
            0x30369d => {
                push!(eax);
                push!(ebx);
                call!(0x3080b0);
                pc = 0x3036a4; continue;
            }
            0x3036a4 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x3036a7; continue;
            }
            0x3036a7 => {
                ecx = m.read32(ebp.wrapping_add(0x14));
                push!(ecx);
                esp = esp.wrapping_sub(0x8);
                edx = esp;
                m.write32(edx, edi);
                push!(ebx);
                write16(m, edx.wrapping_add(0x4), (((((esi) as u16)) as u32)) as u16);
                call!(0x3081e0);
                pc = 0x3036bc; continue;
            }
            0x3036bc => {
                esi = eax;
                esp = esp.wrapping_add(0x10);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x303779; continue; }
                pc = 0x3036c9; continue;
            }
            0x3036c9 => {
                push!(eax);
                push!(0x9b6);
                call!(0x3d7b0);
                pc = 0x3036d4; continue;
            }
            0x3036d4 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(0x7842a8);
                push!(eax);
                call!(0x7a270);
                pc = 0x3036e3; continue;
            }
            0x3036e3 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x234));
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30378f; continue; }
                pc = 0x3036fa; continue;
            }
            0x3036fa => {
                edx = esp.wrapping_add(0x18);
                push!(edx);
                call!(0x45390);
                pc = 0x303704; continue;
            }
            0x303704 => {
                eax = esp.wrapping_add(0x1c);
                push!(eax);
                m.write32(esp.wrapping_add(0x20), 0x1575);
                call!(0x45820);
                pc = 0x303716; continue;
            }
            0x303716 => {
                esp = esp.wrapping_add(0x8);
                eax = esi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x303722 => {
                push!(0);
                push!(0x9b6);
                call!(0x3d7b0);
                pc = 0x30372e; continue;
            }
            0x30372e => {
                ecx = m.read32(0x7842a8);
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(ecx);
                call!(0x7a270);
                pc = 0x30373e; continue;
            }
            0x30373e => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x234));
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303770; continue; }
                pc = 0x303751; continue;
            }
            0x303751 => {
                eax = esp.wrapping_add(0x18);
                push!(eax);
                call!(0x45390);
                pc = 0x30375b; continue;
            }
            0x30375b => {
                ecx = esp.wrapping_add(0x1c);
                push!(ecx);
                m.write32(esp.wrapping_add(0x20), 0x1575);
                call!(0x45820);
                pc = 0x30376d; continue;
            }
            0x30376d => {
                esp = esp.wrapping_add(0x8);
                pc = 0x303770; continue;
            }
            0x303770 => {
                eax = 0;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x303779 => {
                eax = m.read32(esi);
                fa = eax;
                fb = ((((0x36i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303785; continue; }
                pc = 0x303780; continue;
            }
            0x303780 => {
                fa = eax;
                fb = ((((0x37i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30378f; continue; }
                pc = 0x303785; continue;
            }
            0x303785 => {
                push!(0x5);
                call!(0x2fb930);
                pc = 0x30378c; continue;
            }
            0x30378c => {
                esp = esp.wrapping_add(0x4);
                pc = 0x30378f; continue;
            }
            0x30378f => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003037A0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3037a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3037a0;
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
        { crate::pose::cover(0x3037a0, previous, pc); previous = pc; }
        match pc {
            0x3037a0 => {
                esp = esp.wrapping_sub(0x3c);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                call!(0x3082b0);
                pc = 0x3037ac; continue;
            }
            0x3037ac => {
                xmm1.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm2.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                ecx = 0;
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = m.read32(0x7844a8);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fas >= fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ecx = ecx.wrapping_sub(1);
                ecx = (ecx & 0x8);
                m.write32(esp.wrapping_add(0x10), ecx);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x2f84)));
                ecx = m.read32(eax.wrapping_add(0x2f7c));
                edi = m.read32(eax.wrapping_add(0x264));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x2f88)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f20))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm2 = Xmm::scalar(m.read32(ecx.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm2.lanes[0]));
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                m.write32(esp.wrapping_add(0x18), xmm1.lanes[0]);
                xmm1.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(edx);
                ebx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                call!(0x2f9de0);
                pc = 0x30384c; continue;
            }
            0x30384c => {
                eax = m.read32(0x7844a8);
                ebx = m.read32(eax.wrapping_add(0x2f7c));
                esp = esp.wrapping_add(0x4);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30394c; continue; }
                pc = 0x303862; continue;
            }
            0x303862 => {
                call!(0x307ec0);
                pc = 0x303867; continue;
            }
            0x303867 => {
                ebp = eax;
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30394c; continue; }
                pc = 0x303871; continue;
            }
            0x303871 => {
                push!(ebp);
                ecx = 0x12;
                esi = ebx;
                edi = ebp;
                push!(ebx);
                { let step = if entry.eflags & 0x400 == 0 { 4u32 } else { 0u32.wrapping_sub(4) }; for i in 0..ecx { let value=m.read32(esi.wrapping_add(i.wrapping_mul(step)));m.write32(edi.wrapping_add(i.wrapping_mul(step)),value); } esi=esi.wrapping_add(ecx.wrapping_mul(step));edi=edi.wrapping_add(ecx.wrapping_mul(step));ecx=0; }
                call!(0x2f95e0);
                pc = 0x303883; continue;
            }
            0x303883 => {
                push!(ebx);
                edi = ebp;
                call!(0x2f9650);
                pc = 0x30388b; continue;
            }
            0x30388b => {
                xmm0 = Xmm::scalar(m.read32(ebx));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x20)))).to_bits();
                xmm1 = Xmm::default();
                edi = 0;
                esp = esp.wrapping_add(0xc);
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(ebx.wrapping_add(0x3c), edi);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3038ad; continue; }
                pc = 0x3038a5; continue;
            }
            0x3038a5 => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                pc = 0x3038ad; continue;
            }
            0x3038ad => {
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                m.write32(ebp, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ebx.wrapping_add(0x4)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3038d2; continue; }
                pc = 0x3038ca; continue;
            }
            0x3038ca => {
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                pc = 0x3038d2; continue;
            }
            0x3038d2 => {
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                m.write32(ebp.wrapping_add(0x4), xmm0.lanes[0]);
                eax = m.read32(0x7844a8);
                xmm0.lanes[0] = (((m.read32(eax.wrapping_add(0x264))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ebx.wrapping_add(0x8)))).to_bits();
                ecx = esp.wrapping_add(0x10);
                push!(ecx);
                push!(ebp);
                m.write32(ebp.wrapping_add(0x8), xmm0.lanes[0]);
                m.write32(ebp.wrapping_add(0x40), edi);
                call!(0x307ef0);
                pc = 0x303904; continue;
            }
            0x303904 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3039fb; continue; }
                pc = 0x30390f; continue;
            }
            0x30390f => {
                eax = m.read32(0x784014);
                fa = m.read32(eax);
                fb = edi;
                esi = m.read32(esp.wrapping_add(0x10));
                if (fa == fb) { pc = 0x303932; continue; }
                pc = 0x30391c; continue;
            }
            0x30391c => {
                push!(0xc77668);
                push!(eax);
                call!(0x32a5b0);
                pc = 0x303927; continue;
            }
            0x303927 => {
                eax = m.read32(eax.wrapping_add(0x248));
                esp = esp.wrapping_add(0x8);
                pc = 0x303934; continue;
            }
            0x303932 => {
                eax = 0;
                pc = 0x303934; continue;
            }
            0x303934 => {
                eax = eax.wrapping_add(esi);
                fa = eax;
                fb = ((((0x7fe0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x3039fb; continue; }
                pc = 0x303941; continue;
            }
            0x303941 => {
                ebx = m.read32(ebx.wrapping_add(0x40));
                fa = ebx;
                fb = edi;
                if (fa != fb) { pc = 0x303862; continue; }
                pc = 0x30394c; continue;
            }
            0x30394c => {
                edx = m.read32(0x7844a8);
                eax = (eax & 0xffff_ff00) | ((((m.read8(edx.wrapping_add(0x238))) as u32)) & 0xff);
                ebx = 0x20;
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((eax) as u8))) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x30397e; continue; }
                pc = 0x303961; continue;
            }
            0x303961 => {
                push!(0x7);
                call!(0x3370a0);
                pc = 0x303968; continue;
            }
            0x303968 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | ebx);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x30397e; continue;
            }
            0x30397e => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f7c));
                push!(ecx);
                call!(0x308560);
                pc = 0x30398f; continue;
            }
            0x30398f => {
                edx = m.read32(0x7844a8);
                push!(0x5);
                m.write32(edx.wrapping_add(0x2f7c), 0);
                call!(0x2fb930);
                pc = 0x3039a6; continue;
            }
            0x3039a6 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x234));
                esp = esp.wrapping_add(0x8);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3039dc; continue; }
                pc = 0x3039b8; continue;
            }
            0x3039b8 => {
                eax = esp.wrapping_add(0x1c);
                push!(eax);
                call!(0x45390);
                pc = 0x3039c2; continue;
            }
            0x3039c2 => {
                ecx = esp.wrapping_add(0x20);
                push!(ecx);
                m.write32(esp.wrapping_add(0x24), 0x157b);
                call!(0x45820);
                pc = 0x3039d4; continue;
            }
            0x3039d4 => {
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x8);
                pc = 0x3039dc; continue;
            }
            0x3039dc => {
                ecx = m.read32(eax.wrapping_add(0x238));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = (ecx | 0x4000);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax.wrapping_add(0x238), ecx);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x3c);
                break 'run;
            }
            0x3039fb => {
                push!(ebp);
                call!(0x308560);
                pc = 0x303a01; continue;
            }
            0x303a01 => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x2f7c));
                push!(eax);
                call!(0x308560);
                pc = 0x303a13; continue;
            }
            0x303a13 => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x8);
                m.write32(ecx.wrapping_add(0x2f7c), edi);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x3c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00303A30: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f303a30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x303a30;
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
        { crate::pose::cover(0x303a30, previous, pc); previous = pc; }
        match pc {
            0x303a30 => {
                push!(ebx);
                push!(esi);
                push!(edi);
                edi = 0;
                ebx = 0;
                call!(0x3082b0);
                pc = 0x303a3c; continue;
            }
            0x303a3c => {
                eax = m.read32(0x7844a8);
                esi = m.read32(eax.wrapping_add(0x2f7c));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303ab0; continue; }
                pc = 0x303a4b; continue;
            }
            0x303a4b => {
                push!(ebp);
                pc = 0x303a50; continue;
            }
            0x303a50 => {
                eax = m.read32(esi.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303a61; continue; }
                pc = 0x303a57; continue;
            }
            0x303a57 => {
                eax = m.read32(eax.wrapping_add(0x14));
                edi = edi.wrapping_add(0x1c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x303a57; continue; }
                pc = 0x303a61; continue;
            }
            0x303a61 => {
                eax = m.read32(esi.wrapping_add(0x10));
                push!(eax);
                call!(0x333700);
                pc = 0x303a6a; continue;
            }
            0x303a6a => {
                ecx = m.read32(esi.wrapping_add(0x10));
                push!(ecx);
                ebp = eax;
                call!(0x2fdf50);
                pc = 0x303a75; continue;
            }
            0x303a75 => {
                esi = m.read32(esi.wrapping_add(0x40));
                ebp = ebp.wrapping_add(eax);
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebx = ebx.wrapping_add(ebp.wrapping_mul(0x8)).wrapping_add(0x28);
                if (fa != fb) { pc = 0x303a50; continue; }
                pc = 0x303a85; continue;
            }
            0x303a85 => {
                ebx = ebx.wrapping_add(edi);
                push!(ebx);
                call!(0x300670);
                pc = 0x303a8d; continue;
            }
            0x303a8d => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = m.read32(0x7844a8);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa != fb) { pc = 0x303ab0; continue; }
                pc = 0x303a9a; continue;
            }
            0x303a9a => {
                ecx = m.read32(eax.wrapping_add(0x25c));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = (ecx | 0x10000);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax.wrapping_add(0x25c), ecx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x303ab0 => {
                ecx = m.read32(eax.wrapping_add(0x25c));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = (ecx & 0xfffeffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax.wrapping_add(0x25c), ecx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00303B10: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f303b10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x303b10;
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
        { crate::pose::cover(0x303b10, previous, pc); previous = pc; }
        match pc {
            0x303b10 => {
                esp = esp.wrapping_sub(0x8);
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x280));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp, 0);
                if (fas <= fbs) { pc = 0x303c3f; continue; }
                pc = 0x303b2d; continue;
            }
            0x303b2d => {
                xmm3 = Xmm::default();
                edx = 0;
                push!(ebp);
                m.write32(esp.wrapping_add(0x8), edx);
                push!(edi);
                pc = 0x303b40; continue;
            }
            0x303b40 => {
                ecx = m.read32(edx.wrapping_add(eax).wrapping_add(0x2e0));
                edx = edx.wrapping_add(eax).wrapping_add(0x288);
                edi = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x303c18; continue; }
                pc = 0x303b58; continue;
            }
            0x303b58 => {
                pc = 0x303b60; continue;
            }
            0x303b60 => {
                eax = edi;
                eax = eax.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = m.read32(eax.wrapping_add(edx).wrapping_add(0x64));
                eax = eax.wrapping_add(edx);
                fa = ecx;
                fb = ebx;
                if (fa != fb) { pc = 0x303c07; continue; }
                pc = 0x303b73; continue;
            }
            0x303b73 => {
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                ebp = (((((m.read16(eax.wrapping_add(0x68)) as i16)) as i32)) as u32);
                ecx = m.read32(ebx.wrapping_add(0xc));
                ecx = m.read32(ebx.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x2c));
                xmm0.lanes[0] = (((ebp) as i32) as f32).to_bits();
                ebp = (((((m.read16(eax.wrapping_add(0x6a)) as i16)) as i32)) as u32);
                xmm1.lanes[0] = (((ebp) as i32) as f32).to_bits();
                xmm2 = xmm0;
                if (fa == fb) { pc = 0x303baf; continue; }
                pc = 0x303b92; continue;
            }
            0x303b92 => {
                fa = esi;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x303bc5; continue; }
                pc = 0x303b97; continue;
            }
            0x303b97 => {
                ecx = m.read32(ecx.wrapping_add(0x18));
                ecx = ecx.wrapping_sub(1);
                xmm4 = xmm3;
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1 = xmm2;
                pc = 0x303bc5; continue;
            }
            0x303baf => {
                ecx = m.read32(ecx.wrapping_add(0x14));
                xmm0 = xmm1;
                ecx = ecx.wrapping_sub(1);
                xmm4 = xmm3;
                xmm1.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                pc = 0x303bc5; continue;
            }
            0x303bc5 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                write16(m, eax.wrapping_add(0x68), (((((ecx) as u16)) as u32)) as u16);
                ecx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                write16(m, eax.wrapping_add(0x6a), (((((ecx) as u16)) as u32)) as u16);
                if (fa != fb) { pc = 0x303c07; continue; }
                pc = 0x303bd9; continue;
            }
            0x303bd9 => {
                eax = m.read32(edx.wrapping_add(0x54));
                eax = eax.wrapping_add(esi);
                fa = eax;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x303be7; continue; }
                pc = 0x303be3; continue;
            }
            0x303be3 => {
                eax = 0;
                pc = 0x303bf0; continue;
            }
            0x303be7 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x303bf0; continue; }
                pc = 0x303beb; continue;
            }
            0x303beb => {
                eax = 0x7;
                pc = 0x303bf0; continue;
            }
            0x303bf0 => {
                eax = eax.wrapping_add(esi);
                fa = eax;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x303bfb; continue; }
                pc = 0x303bf7; continue;
            }
            0x303bf7 => {
                eax = 0;
                pc = 0x303c04; continue;
            }
            0x303bfb => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x303c04; continue; }
                pc = 0x303bff; continue;
            }
            0x303bff => {
                eax = 0x7;
                pc = 0x303c04; continue;
            }
            0x303c04 => {
                m.write32(edx.wrapping_add(0x54), eax);
                pc = 0x303c07; continue;
            }
            0x303c07 => {
                eax = m.read32(edx.wrapping_add(0x58));
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x303b60; continue; }
                pc = 0x303c13; continue;
            }
            0x303c13 => {
                eax = m.read32(0x7844a8);
                pc = 0x303c18; continue;
            }
            0x303c18 => {
                ecx = m.read32(esp.wrapping_add(0x8));
                edx = m.read32(esp.wrapping_add(0xc));
                edi = m.read32(eax.wrapping_add(0x280));
                ecx = ecx.wrapping_add(1);
                edx = edx.wrapping_add(0xe4);
                fa = ecx;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x8), ecx);
                m.write32(esp.wrapping_add(0xc), edx);
                if (fas < fbs) { pc = 0x303b40; continue; }
                pc = 0x303c3d; continue;
            }
            0x303c3d => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x303c3f; continue;
            }
            0x303c3f => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00303C50: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f303c50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x303c50;
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
        { crate::pose::cover(0x303c50, previous, pc); previous = pc; }
        match pc {
            0x303c50 => {
                eax = m.read32(esi.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303cfc; continue; }
                pc = 0x303c5b; continue;
            }
            0x303c5b => {
                xmm3 = Xmm::default();
                push!(edi);
                pc = 0x303c60; continue;
            }
            0x303c60 => {
                fa = edx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                edi = (((((m.read16(eax.wrapping_add(0xc)) as i16)) as i32)) as u32);
                ecx = m.read32(esi.wrapping_add(0xc));
                ecx = m.read32(esi.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x2c));
                xmm0.lanes[0] = (((edi) as i32) as f32).to_bits();
                edi = (((((m.read16(eax.wrapping_add(0xe)) as i16)) as i32)) as u32);
                xmm1.lanes[0] = (((edi) as i32) as f32).to_bits();
                xmm2 = xmm0;
                if (fa == fb) { pc = 0x303c9c; continue; }
                pc = 0x303c7f; continue;
            }
            0x303c7f => {
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x303cb2; continue; }
                pc = 0x303c84; continue;
            }
            0x303c84 => {
                ecx = m.read32(ecx.wrapping_add(0x18));
                ecx = ecx.wrapping_sub(1);
                xmm4 = xmm3;
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1 = xmm2;
                pc = 0x303cb2; continue;
            }
            0x303c9c => {
                ecx = m.read32(ecx.wrapping_add(0x14));
                xmm0 = xmm1;
                ecx = ecx.wrapping_sub(1);
                xmm4 = xmm3;
                xmm1.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                pc = 0x303cb2; continue;
            }
            0x303cb2 => {
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                write16(m, eax.wrapping_add(0xc), (((((ecx) as u16)) as u32)) as u16);
                ecx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                write16(m, eax.wrapping_add(0xe), (((((ecx) as u16)) as u32)) as u16);
                ecx = m.read32(eax.wrapping_add(0x8));
                ecx = ecx.wrapping_add(edx);
                fa = ecx;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x303cd0; continue; }
                pc = 0x303ccc; continue;
            }
            0x303ccc => {
                ecx = 0;
                pc = 0x303cd9; continue;
            }
            0x303cd0 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x303cd9; continue; }
                pc = 0x303cd4; continue;
            }
            0x303cd4 => {
                ecx = 0x7;
                pc = 0x303cd9; continue;
            }
            0x303cd9 => {
                ecx = ecx.wrapping_add(edx);
                fa = ecx;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x303ce4; continue; }
                pc = 0x303ce0; continue;
            }
            0x303ce0 => {
                ecx = 0;
                pc = 0x303ced; continue;
            }
            0x303ce4 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x303ced; continue; }
                pc = 0x303ce8; continue;
            }
            0x303ce8 => {
                ecx = 0x7;
                pc = 0x303ced; continue;
            }
            0x303ced => {
                m.write32(eax.wrapping_add(0x8), ecx);
                eax = m.read32(eax.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x303c60; continue; }
                pc = 0x303cfb; continue;
            }
            0x303cfb => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x303cfc; continue;
            }
            0x303cfc => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00303D00: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f303d00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x303d00;
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
        { crate::pose::cover(0x303d00, previous, pc); previous = pc; }
        match pc {
            0x303d00 => {
                esp = esp.wrapping_sub(0x30);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x38));
                if (fa == fb) { pc = 0x303e02; continue; }
                pc = 0x303d10; continue;
            }
            0x303d10 => {
                ecx = m.read32(eax.wrapping_add(0xc));
                edx = m.read32(0x7844a8);
                push!(edi);
                edi = m.read32(eax.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x2c));
                eax = m.read32(edx.wrapping_add(0x234));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303d47; continue; }
                pc = 0x303d28; continue;
            }
            0x303d28 => {
                eax = esp.wrapping_add(0x8);
                push!(eax);
                call!(0x45390);
                pc = 0x303d32; continue;
            }
            0x303d32 => {
                ecx = esp.wrapping_add(0xc);
                push!(ecx);
                m.write32(esp.wrapping_add(0x10), 0x1579);
                call!(0x45820);
                pc = 0x303d44; continue;
            }
            0x303d44 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x303d47; continue;
            }
            0x303d47 => {
                push!(ebx);
                eax = ebp;
                call!(0x2ffac0);
                pc = 0x303d4f; continue;
            }
            0x303d4f => {
                edx = m.read32(0x7844a8);
                ebx = m.read32(edx.wrapping_add(0x2f7c));
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303e00; continue; }
                pc = 0x303d63; continue;
            }
            0x303d63 => {
                push!(esi);
                pc = 0x303d64; continue;
            }
            0x303d64 => {
                edi = m.read32(ebx.wrapping_add(0xc));
                edi = edi.wrapping_add(ebp);
                fa = edi;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x303d72; continue; }
                pc = 0x303d6e; continue;
            }
            0x303d6e => {
                edi = 0;
                pc = 0x303d7b; continue;
            }
            0x303d72 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x303d7b; continue; }
                pc = 0x303d76; continue;
            }
            0x303d76 => {
                edi = 0x3;
                pc = 0x303d7b; continue;
            }
            0x303d7b => {
                esi = ebp;
                call!(0x303b10);
                pc = 0x303d82; continue;
            }
            0x303d82 => {
                push!(ebp);
                call!(0x2ffc40);
                pc = 0x303d88; continue;
            }
            0x303d88 => {
                esp = esp.wrapping_add(0x4);
                edx = ebp;
                esi = ebx;
                call!(0x303c50);
                pc = 0x303d94; continue;
            }
            0x303d94 => {
                fa = ebp;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                eax = m.read32(ebx.wrapping_add(0xc));
                eax = m.read32(ebx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x2c));
                xmm0 = Xmm::scalar(m.read32(ebx));
                xmm1 = Xmm::scalar(m.read32(ebx.wrapping_add(0x4)));
                if (fa == fb) { pc = 0x303dcf; continue; }
                pc = 0x303da9; continue;
            }
            0x303da9 => {
                fa = ebp;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x303df1; continue; }
                pc = 0x303dae; continue;
            }
            0x303dae => {
                edx = m.read32(eax.wrapping_add(0x18));
                xmm3 = Xmm::default();
                ecx = ebp;
                ecx = ecx.wrapping_sub(edx);
                xmm2.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(ebx, xmm2.lanes[0]);
                m.write32(ebx.wrapping_add(0x4), xmm0.lanes[0]);
                pc = 0x303df1; continue;
            }
            0x303dcf => {
                xmm2 = Xmm::default();
                m.write32(ebx, xmm1.lanes[0]);
                ecx = m.read32(eax.wrapping_add(0x14));
                edx = 0x1;
                edx = edx.wrapping_sub(ecx);
                xmm1.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(ebx.wrapping_add(0x4), xmm1.lanes[0]);
                pc = 0x303df1; continue;
            }
            0x303df1 => {
                m.write32(ebx.wrapping_add(0xc), edi);
                ebx = m.read32(ebx.wrapping_add(0x40));
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x303d64; continue; }
                pc = 0x303dff; continue;
            }
            0x303dff => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x303e00; continue;
            }
            0x303e00 => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x303e02; continue;
            }
            0x303e02 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00303E10: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f303e10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x303e10;
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
        { crate::pose::cover(0x303e10, previous, pc); previous = pc; }
        match pc {
            0x303e10 => {
                push!(ebp);
                ebp = esp;
                esp = (esp & 0xfffffff8u32);
                esp = esp.wrapping_sub(0xc8);
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(esi);
                push!(edi);
                push!(ecx);
                call!(0x77470);
                pc = 0x303e35; continue;
            }
            0x303e35 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x304085; continue; }
                pc = 0x303e4c; continue;
            }
            0x303e4c => {
                edx = m.read32(eax.wrapping_add(0x25c));
                ecx = 0x2000;
                fa = ecx;
                fb = edx;
                if ((fa & fb) != 0) { pc = 0x304085; continue; }
                pc = 0x303e5f; continue;
            }
            0x303e5f => {
                esi = edx;
                esi = (esi | ecx);
                m.write32(eax.wrapping_add(0x25c), esi);
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f7c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30404f; continue; }
                pc = 0x303e7c; continue;
            }
            0x303e7c => {
                eax = m.read32(eax.wrapping_add(0x25c));
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x304032; continue; }
                pc = 0x303e8a; continue;
            }
            0x303e8a => {
                fa = eax;
                fb = ((((0x410000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x303ebd; continue; }
                pc = 0x303e91; continue;
            }
            0x303e91 => {
                eax = m.read32(ecx.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303ea6; continue; }
                pc = 0x303e98; continue;
            }
            0x303e98 => {
                edx = m.read32(eax.wrapping_add(0x4));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x303ebd; continue; }
                pc = 0x303e9f; continue;
            }
            0x303e9f => {
                eax = m.read32(eax.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x303e98; continue; }
                pc = 0x303ea6; continue;
            }
            0x303ea6 => {
                ecx = m.read32(ecx.wrapping_add(0x40));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x303e91; continue; }
                pc = 0x303ead; continue;
            }
            0x303ead => {
                call!(0x73c80);
                pc = 0x303eb2; continue;
            }
            0x303eb2 => {
                call!(0x325e80);
                pc = 0x303eb7; continue;
            }
            0x303eb7 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x303ebd => {
                eax = 0;
                ecx = 0x25;
                edi = esp.wrapping_add(0x38);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                call!(0x2fff70);
                pc = 0x303ecf; continue;
            }
            0x303ecf => {
                esi = m.read32(0x78400c);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303ef8; continue; }
                pc = 0x303ed9; continue;
            }
            0x303ed9 => {
                pc = 0x303ee0; continue;
            }
            0x303ee0 => {
                eax = m.read32(esi.wrapping_add(0x3c));
                edx = esp.wrapping_add(0x38);
                push!(edx);
                push!(eax);
                call!(0x308420);
                pc = 0x303eee; continue;
            }
            0x303eee => {
                esi = m.read32(esi.wrapping_add(0x40));
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x303ee0; continue; }
                pc = 0x303ef8; continue;
            }
            0x303ef8 => {
                ecx = m.read32(0x7844a8);
                esi = m.read32(ecx.wrapping_add(0x2f7c));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x303f20; continue; }
                pc = 0x303f08; continue;
            }
            0x303f08 => {
                eax = m.read32(esi.wrapping_add(0x3c));
                edx = esp.wrapping_add(0x38);
                push!(edx);
                push!(eax);
                call!(0x308420);
                pc = 0x303f16; continue;
            }
            0x303f16 => {
                esi = m.read32(esi.wrapping_add(0x40));
                esp = esp.wrapping_add(0x8);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x303f08; continue; }
                pc = 0x303f20; continue;
            }
            0x303f20 => {
                esi = 0;
                eax = 0x524be4;
                pc = 0x303f27; continue;
            }
            0x303f27 => {
                ecx = m.read32(esp.wrapping_add(esi.wrapping_mul(0x4)).wrapping_add(0x38));
                fa = ecx;
                fb = m.read32(eax);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x303fa7; continue; }
                pc = 0x303f2f; continue;
            }
            0x303f2f => {
                eax = eax.wrapping_add(0x18);
                esi = esi.wrapping_add(1);
                fa = eax;
                fb = ((((0x524f5ci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x303f27; continue; }
                pc = 0x303f3a; continue;
            }
            0x303f3a => {
                fa = esi;
                fb = ((((0x25i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30403f; continue; }
                pc = 0x303f43; continue;
            }
            0x303f43 => {
                call!(0x3037a0);
                pc = 0x303f48; continue;
            }
            0x303f48 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30403f; continue; }
                pc = 0x303f50; continue;
            }
            0x303f50 => {
                esi = m.read32(0x7842a8);
                push!(eax);
                push!(0x9b6);
                call!(0x3d7b0);
                pc = 0x303f61; continue;
            }
            0x303f61 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(esi);
                call!(0x7a270);
                pc = 0x303f6b; continue;
            }
            0x303f6b => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x234));
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304324; continue; }
                pc = 0x303f82; continue;
            }
            0x303f82 => {
                edx = esp.wrapping_add(0x8);
                push!(edx);
                call!(0x45390);
                pc = 0x303f8c; continue;
            }
            0x303f8c => {
                eax = esp.wrapping_add(0xc);
                push!(eax);
                m.write32(esp.wrapping_add(0x10), 0x1575);
                call!(0x45820);
                pc = 0x303f9e; continue;
            }
            0x303f9e => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x303fa7 => {
                push!(esi);
                call!(0x308460);
                pc = 0x303fad; continue;
            }
            0x303fad => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                edx = esi.wrapping_add(esi.wrapping_mul(0x2));
                eax = m.read32(edx.wrapping_mul(0x8).wrapping_add(0x524be4));
                push!(eax);
                push!(0x49f794);
                push!(0x9b9);
                call!(0x3d7b0);
                pc = 0x303fcb; continue;
            }
            0x303fcb => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(ecx);
                call!(0x3d7f0);
                pc = 0x303fdb; continue;
            }
            0x303fdb => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x234));
                esp = esp.wrapping_add(0x14);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30400d; continue; }
                pc = 0x303fee; continue;
            }
            0x303fee => {
                eax = esp.wrapping_add(0x8);
                push!(eax);
                call!(0x45390);
                pc = 0x303ff8; continue;
            }
            0x303ff8 => {
                ecx = esp.wrapping_add(0xc);
                push!(ecx);
                m.write32(esp.wrapping_add(0x10), 0x1575);
                call!(0x45820);
                pc = 0x30400a; continue;
            }
            0x30400a => {
                esp = esp.wrapping_add(0x8);
                pc = 0x30400d; continue;
            }
            0x30400d => {
                edx = m.read32(0x7844a8);
                eax = m.read32(0x7842a8);
                push!(0x2faf70);
                push!(0x303ad0);
                push!(edx);
                push!(eax);
                call!(0x7a080);
                pc = 0x304029; continue;
            }
            0x304029 => {
                esp = esp.wrapping_add(0x10);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x304032 => {
                push!(0x1576);
                call!(0x2f79e0);
                pc = 0x30403c; continue;
            }
            0x30403c => {
                esp = esp.wrapping_add(0x4);
                pc = 0x30403f; continue;
            }
            0x30403f => {
                call!(0x73c80);
                pc = 0x304044; continue;
            }
            0x304044 => {
                call!(0x325e80);
                pc = 0x304049; continue;
            }
            0x304049 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x30404f => {
                eax = m.read32(eax.wrapping_add(0x2f18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304324; continue; }
                pc = 0x30405d; continue;
            }
            0x30405d => {
                edi = eax;
                call!(0x2f9320);
                pc = 0x304064; continue;
            }
            0x304064 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304324; continue; }
                pc = 0x30406d; continue;
            }
            0x30406d => {
                push!(0);
                edi = 0x2fb580;
                eax = (eax | 0xffffffffu32);
                call!(0x2ffd50);
                pc = 0x30407c; continue;
            }
            0x30407c => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x304085 => {
                ecx = m.read32(ebp.wrapping_add(0x8));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3040fb; continue; }
                pc = 0x30408c; continue;
            }
            0x30408c => {
                ecx = m.read32(eax.wrapping_add(0x2f7c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3040fb; continue; }
                pc = 0x304096; continue;
            }
            0x304096 => {
                push!(0x1);
                call!(0x2f9920);
                pc = 0x30409d; continue;
            }
            0x30409d => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f7c));
                esp = esp.wrapping_add(0x4);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3040f1; continue; }
                pc = 0x3040af; continue;
            }
            0x3040af => {
                fa = ((((m.read8(eax.wrapping_add(0x238)))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x3040db; continue; }
                pc = 0x3040b8; continue;
            }
            0x3040b8 => {
                push!(0x5);
                call!(0x3370a0);
                pc = 0x3040bf; continue;
            }
            0x3040bf => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | 0x20);
                m.write32(eax.wrapping_add(0x238), ecx);
                eax = m.read32(0x7844a8);
                pc = 0x3040db; continue;
            }
            0x3040db => {
                m.write32(eax.wrapping_add(0x25c), (m.read32(eax.wrapping_add(0x25c)) | 0x4));
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f7c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3040fb; continue; }
                pc = 0x3040f1; continue;
            }
            0x3040f1 => {
                call!(0x2fb020);
                pc = 0x3040f6; continue;
            }
            0x3040f6 => {
                call!(0x73c80);
                pc = 0x3040fb; continue;
            }
            0x3040fb => {
                ecx = m.read32(0x7ba9e0);
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(0x7ba26c));
                push!(edx);
                call!(0x77470);
                pc = 0x304113; continue;
            }
            0x304113 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (((((((((((((fa)) as u8)) as i32).wrapping_sub((((((fb)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) == 0i32)) { pc = 0x3041e3; continue; }
                pc = 0x304124; continue;
            }
            0x304124 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f7c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304192; continue; }
                pc = 0x304133; continue;
            }
            0x304133 => {
                eax = m.read32(ecx.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30414e; continue; }
                pc = 0x30413a; continue;
            }
            0x30413a => {
                pc = 0x304140; continue;
            }
            0x304140 => {
                edx = m.read32(eax.wrapping_add(0x4));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x30416e; continue; }
                pc = 0x304147; continue;
            }
            0x304147 => {
                eax = m.read32(eax.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x304140; continue; }
                pc = 0x30414e; continue;
            }
            0x30414e => {
                ecx = m.read32(ecx.wrapping_add(0x40));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x304133; continue; }
                pc = 0x304155; continue;
            }
            0x304155 => {
                push!(ecx);
                call!(0x2fb7b0);
                pc = 0x30415b; continue;
            }
            0x30415b => {
                esp = esp.wrapping_add(0x4);
                push!(0x5);
                call!(0x2fb930);
                pc = 0x304165; continue;
            }
            0x304165 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x30416e => {
                call!(0x2ffe10);
                pc = 0x304173; continue;
            }
            0x304173 => {
                edi = eax;
                push!(0);
                eax = (eax | 0xffffffffu32);
                call!(0x2ffd50);
                pc = 0x30417f; continue;
            }
            0x30417f => {
                esp = esp.wrapping_add(0x4);
                push!(0x5);
                call!(0x2fb930);
                pc = 0x304189; continue;
            }
            0x304189 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x304192 => {
                eax = m.read32(eax.wrapping_add(0x2f18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3041e3; continue; }
                pc = 0x30419c; continue;
            }
            0x30419c => {
                call!(0x2fe730);
                pc = 0x3041a1; continue;
            }
            0x3041a1 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3041c9; continue; }
                pc = 0x3041a5; continue;
            }
            0x3041a5 => {
                call!(0x2ffe10);
                pc = 0x3041aa; continue;
            }
            0x3041aa => {
                edi = eax;
                push!(0);
                eax = (eax | 0xffffffffu32);
                call!(0x2ffd50);
                pc = 0x3041b6; continue;
            }
            0x3041b6 => {
                esp = esp.wrapping_add(0x4);
                push!(0x5);
                call!(0x2fb930);
                pc = 0x3041c0; continue;
            }
            0x3041c0 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x3041c9 => {
                push!(0);
                call!(0x2fb830);
                pc = 0x3041d0; continue;
            }
            0x3041d0 => {
                esp = esp.wrapping_add(0x4);
                push!(0x5);
                call!(0x2fb930);
                pc = 0x3041da; continue;
            }
            0x3041da => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x3041e3 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ecx);
                call!(0x77470);
                pc = 0x3041fa; continue;
            }
            0x3041fa => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x304232; continue; }
                pc = 0x304208; continue;
            }
            0x304208 => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x2f7c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x304232; continue; }
                pc = 0x304218; continue;
            }
            0x304218 => {
                push!(0);
                call!(0x2f9920);
                pc = 0x30421f; continue;
            }
            0x30421f => {
                esp = esp.wrapping_add(0x4);
                call!(0x2f92e0);
                pc = 0x304227; continue;
            }
            0x304227 => {
                call!(0x73c80);
                pc = 0x30422c; continue;
            }
            0x30422c => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x304232 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ecx);
                call!(0x77470);
                pc = 0x304249; continue;
            }
            0x304249 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x304271; continue; }
                pc = 0x304257; continue;
            }
            0x304257 => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x2f7c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304271; continue; }
                pc = 0x304267; continue;
            }
            0x304267 => {
                push!(0xffffffffu32);
                call!(0x303d00);
                pc = 0x30426e; continue;
            }
            0x30426e => {
                esp = esp.wrapping_add(0x4);
                pc = 0x304271; continue;
            }
            0x304271 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ecx);
                call!(0x77470);
                pc = 0x304288; continue;
            }
            0x304288 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3042b0; continue; }
                pc = 0x304296; continue;
            }
            0x304296 => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x2f7c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3042b0; continue; }
                pc = 0x3042a6; continue;
            }
            0x3042a6 => {
                push!(0x1);
                call!(0x303d00);
                pc = 0x3042ad; continue;
            }
            0x3042ad => {
                esp = esp.wrapping_add(0x4);
                pc = 0x3042b0; continue;
            }
            0x3042b0 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(ecx);
                call!(0x77470);
                pc = 0x3042c7; continue;
            }
            0x3042c7 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x304324; continue; }
                pc = 0x3042d5; continue;
            }
            0x3042d5 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f7c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x304324; continue; }
                pc = 0x3042e4; continue;
            }
            0x3042e4 => {
                ecx = m.read32(eax.wrapping_add(0x2f18));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304324; continue; }
                pc = 0x3042ee; continue;
            }
            0x3042ee => {
                eax = ecx;
                edx = (edx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x14))) as u32)) & 0xff);
                ecx = (ecx & 0xffff_ff00) | ((0x2) & 0xff);
                edx = (edx & 0xffff_ff00) | (((((((((edx) as u8)) as i32) ^ ((((ecx) as u8)) as i32))) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x14), (((((edx) as u8)) as u32)) as u8);
                eax = m.read32(0x7844a8);
                edx = m.read32(eax.wrapping_add(0x2f18));
                fa = ((((m.read8(edx.wrapping_add(0x14)))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                ecx = m.read32(eax.wrapping_add(0x2f9c));
                if ((fa & fb) == 0) { pc = 0x30431d; continue; }
                pc = 0x304310; continue;
            }
            0x304310 => {
                ecx = ecx.wrapping_add(1);
                m.write32(eax.wrapping_add(0x2f9c), ecx);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x30431d => {
                ecx = ecx.wrapping_sub(1);
                m.write32(eax.wrapping_add(0x2f9c), ecx);
                pc = 0x304324; continue;
            }
            0x304324 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00304330: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f304330<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x304330;
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
        { crate::pose::cover(0x304330, previous, pc); previous = pc; }
        match pc {
            0x304330 => {
                esp = esp.wrapping_sub(0x10);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x2f70));
                ecx = m.read32(edx.wrapping_add(0x2f74));
                ecx = ecx.wrapping_add(0xc);
                esi = m.read32(eax.wrapping_add(0xc));
                esi = m.read32(eax.wrapping_add(esi.wrapping_mul(0x4)).wrapping_add(0x2c));
                ecx = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                esi = esi.wrapping_add(ecx.wrapping_mul(0x2));
                ecx = ((m.read8(esi.wrapping_add(0x1))) as u8 as u32);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                m.write32(esp.wrapping_add(0x14), ecx);
                ecx = ((m.read8(esi.wrapping_add(0x2))) as u8 as u32);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                ecx = ((m.read8(esi.wrapping_add(0x3))) as u8 as u32);
                ebp = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x8)))).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                m.write32(esp.wrapping_add(0x1c), eax);
                eax = m.read32(edx.wrapping_add(0x2f34));
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0xc4) as i32)) as u32;
                ebx = 0;
                edi = (edi | 0xffffffffu32);
                fa = ((((m.read16(ecx.wrapping_add(0x524f5a)))) as u32) & 0xffffu32);
                fb = ((((0x22i32)) as u32) & 0xffffu32);
                m.write32(esp.wrapping_add(0x10), ebx);
                if (fa != fb) { pc = 0x3043d2; continue; }
                pc = 0x3043b4; continue;
            }
            0x3043b4 => {
                ecx = eax;
                ecx = ecx.wrapping_sub(0x46);
                fa = ecx;
                if ((fa != 0)) { pc = 0x3043d2; continue; }
                pc = 0x3043bb; continue;
            }
            0x3043bb => {
                edx = ((m.read16(edx.wrapping_add(0x3068))) as u16 as u32);
                edi = 0xa83;
                m.write32(esp.wrapping_add(0x10), edx);
                ebx = 0x28;
                pc = 0x30441d; continue;
            }
            0x3043d2 => {
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0xc4) as i32)) as u32;
                fa = ((((m.read16(ecx.wrapping_add(0x524f5a)))) as u32) & 0xffffu32);
                fb = ((((0x22i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x304405; continue; }
                pc = 0x3043e4; continue;
            }
            0x3043e4 => {
                ecx = eax;
                ecx = ecx.wrapping_sub(0x46);
                fa = ecx;
                if ((fa == 0)) { pc = 0x304405; continue; }
                pc = 0x3043eb; continue;
            }
            0x3043eb => {
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x304405; continue; }
                pc = 0x3043ee; continue;
            }
            0x3043ee => {
                edx = ((m.read16(edx.wrapping_add(0x306a))) as u16 as u32);
                edi = 0xa90;
                m.write32(esp.wrapping_add(0x10), edx);
                ebx = 0x1e;
                pc = 0x30441d; continue;
            }
            0x304405 => {
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0xc4) as i32)) as u32;
                fa = ((((m.read16(ecx.wrapping_add(0x524f5a)))) as u32) & 0xffffu32);
                fb = ((((0x22i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x30441d; continue; }
                pc = 0x304417; continue;
            }
            0x304417 => {
                eax = eax.wrapping_sub(0x46);
                fa = eax;
                if ((fa == 0)) { pc = 0x30441d; continue; }
                pc = 0x30441c; continue;
            }
            0x30441c => {
                eax = eax.wrapping_sub(1);
                pc = 0x30441d; continue;
            }
            0x30441d => {
                eax = m.read32(0x784014);
                fa = m.read32(eax);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30443d; continue; }
                pc = 0x304427; continue;
            }
            0x304427 => {
                push!(0xc77668);
                push!(eax);
                call!(0x32a5b0);
                pc = 0x304432; continue;
            }
            0x304432 => {
                eax = m.read32(eax.wrapping_add(0x248));
                esp = esp.wrapping_add(0x8);
                pc = 0x30443f; continue;
            }
            0x30443d => {
                eax = 0;
                pc = 0x30443f; continue;
            }
            0x30443f => {
                eax = eax.wrapping_add(0x28);
                fa = eax;
                fb = ((((0x7fe0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x304465; continue; }
                pc = 0x304449; continue;
            }
            0x304449 => {
                edx = m.read32(0x7842a8);
                push!(edx);
                call!(0x2fb750);
                pc = 0x304455; continue;
            }
            0x304455 => {
                esp = esp.wrapping_add(0x4);
                eax = 0x1;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            0x304465 => {
                fa = m.read32(esp.wrapping_add(0x10));
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3044d7; continue; }
                pc = 0x30446b; continue;
            }
            0x30446b => {
                fa = edi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3044d7; continue; }
                pc = 0x304470; continue;
            }
            0x304470 => {
                push!(edi);
                call!(0x3d7b0);
                pc = 0x304476; continue;
            }
            0x304476 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(ebx);
                push!(0x49f794);
                push!(0x9b8);
                call!(0x3d7b0);
                pc = 0x30448a; continue;
            }
            0x30448a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(0x7844a8);
                push!(eax);
                call!(0x3d7f0);
                pc = 0x304499; continue;
            }
            0x304499 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(0x7842a8);
                push!(0);
                push!(ecx);
                push!(edx);
                call!(0x7a270);
                pc = 0x3044ae; continue;
            }
            0x3044ae => {
                push!(0x1575);
                call!(0x2f79e0);
                pc = 0x3044b8; continue;
            }
            0x3044b8 => {
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x24);
                m.write32(eax.wrapping_add(0x2f34), 0xffffffffu32);
                pc = 0x3044ca; continue;
            }
            0x3044ca => {
                eax = 0x1;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            0x3044d7 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(esi)) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0xbi32)) as u32) & 0xffu32);
                if (fa < fb) { pc = 0x30454f; continue; }
                pc = 0x3044de; continue;
            }
            0x3044de => {
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0xei32)) as u32) & 0xffu32);
                if (fa > fb) { pc = 0x30454f; continue; }
                pc = 0x3044e3; continue;
            }
            0x3044e3 => {
                eax = m.read32(0x7ba9e0);
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba280)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd4))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba284)));
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                esi = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f20))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                pc = 0x3045d3; continue;
            }
            0x30454f => {
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x304598; continue; }
                pc = 0x304554; continue;
            }
            0x304554 => {
                edx = m.read32(0x7ba9e0);
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm1 = Xmm::scalar(m.read32(edx.wrapping_add(0x7ba280)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd4))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                m.write32(esp.wrapping_add(0x14), edx);
                esi = edx;
                pc = 0x3045d7; continue;
            }
            0x304598 => {
                eax = m.read32(0x7ba9e0);
                xmm0.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba284)));
                esi = m.read32(esp.wrapping_add(0x14));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f20))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                pc = 0x3045d3; continue;
            }
            0x3045d3 => {
                ebp = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                pc = 0x3045d7; continue;
            }
            0x3045d7 => {
                edi = m.read32(esp.wrapping_add(0x1c));
                ebx = ((((ecx) as u8)) as u8 as u32);
                push!(ebx);
                esp = esp.wrapping_sub(0x8);
                write16(m, esp.wrapping_add(0x20), (((((esi) as u16)) as u32)) as u16);
                write16(m, esp.wrapping_add(0x22), (((((ebp) as u16)) as u32)) as u16);
                ecx = m.read32(esp.wrapping_add(0x20));
                eax = esp;
                m.write32(eax, ecx);
                write16(m, eax.wrapping_add(0x4), (((((edi) as u16)) as u32)) as u16);
                call!(0x3344e0);
                pc = 0x3045fd; continue;
            }
            0x3045fd => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3044ca; continue; }
                pc = 0x304608; continue;
            }
            0x304608 => {
                eax = m.read32(esp.wrapping_add(0x24));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30461d; continue; }
                pc = 0x304610; continue;
            }
            0x304610 => {
                push!(0x157a);
                call!(0x2f79e0);
                pc = 0x30461a; continue;
            }
            0x30461a => {
                esp = esp.wrapping_add(0x4);
                pc = 0x30461d; continue;
            }
            0x30461d => {
                ecx = m.read32(0x7844a8);
                fa = ((((m.read8(ecx.wrapping_add(0x238)))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x304650; continue; }
                pc = 0x30462c; continue;
            }
            0x30462c => {
                push!(0x2);
                call!(0x3370a0);
                pc = 0x304633; continue;
            }
            0x304633 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | 0x20);
                m.write32(eax.wrapping_add(0x238), ecx);
                ecx = m.read32(0x7844a8);
                pc = 0x304650; continue;
            }
            0x304650 => {
                eax = ((m.read16(ecx.wrapping_add(0x306e))) as u16 as u32);
                edx = eax.wrapping_add(eax.wrapping_mul(0x2));
                write16(m, ecx.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x3070), (((((esi) as u16)) as u32)) as u16);
                ecx = m.read32(0x7844a8);
                eax = ((m.read16(ecx.wrapping_add(0x306e))) as u16 as u32);
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                write16(m, ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x3072), (((((ebp) as u16)) as u32)) as u16);
                ecx = m.read32(0x7844a8);
                eax = ((m.read16(ecx.wrapping_add(0x306e))) as u16 as u32);
                edx = eax.wrapping_add(eax.wrapping_mul(0x2));
                write16(m, ecx.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x3074), (((((edi) as u16)) as u32)) as u16);
                ecx = m.read32(0x7844a8);
                eax = ((m.read16(ecx.wrapping_add(0x306e))) as u16 as u32);
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                m.write8(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x3076), (((((ebx) as u8)) as u32)) as u8);
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f34));
                edx = eax;
                edx = ((edx as i32).wrapping_mul((0xc4) as i32)) as u32;
                esi = 0x22;
                fa = ((((m.read16(edx.wrapping_add(0x524f5a)))) as u32) & 0xffffu32);
                fb = ((((((esi) as u16))) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x3046d0; continue; }
                pc = 0x3046cb; continue;
            }
            0x3046cb => {
                edx = (edx | 0xffffffffu32);
                pc = 0x3046e6; continue;
            }
            0x3046d0 => {
                eax = eax.wrapping_sub(0x46);
                fa = eax;
                if ((fa == 0)) { pc = 0x3046e4; continue; }
                pc = 0x3046d5; continue;
            }
            0x3046d5 => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x3046dd; continue; }
                pc = 0x3046d8; continue;
            }
            0x3046d8 => {
                edx = (edx | 0xffffffffu32);
                pc = 0x3046e6; continue;
            }
            0x3046dd => {
                edx = 0x1;
                pc = 0x3046e6; continue;
            }
            0x3046e4 => {
                edx = 0;
                pc = 0x3046e6; continue;
            }
            0x3046e6 => {
                eax = ((m.read16(ecx.wrapping_add(0x306e))) as u16 as u32);
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                m.write8(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x3077), (((((edx) as u8)) as u32)) as u8);
                eax = m.read32(0x7844a8);
                ecx = ((m.read16(eax.wrapping_add(0x306e))) as u16 as u32);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x2f38))) as u32)) & 0xffff);
                ecx = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                write16(m, eax.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x307a), (((((edx) as u16)) as u32)) as u16);
                ecx = m.read32(0x7844a8);
                eax = ((m.read16(ecx.wrapping_add(0x306e))) as u16 as u32);
                edx = eax.wrapping_add(eax.wrapping_mul(0x2));
                edx = ((m.read8(ecx.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x3077))) as u8 as u32);
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304735; continue; }
                pc = 0x304731; continue;
            }
            0x304731 => {
                edx = 0;
                pc = 0x30473b; continue;
            }
            0x304735 => {
                edx = m.read32(ecx.wrapping_add(0x2f54));
                pc = 0x30473b; continue;
            }
            0x30473b => {
                eax = eax.wrapping_add(eax.wrapping_mul(0x2)).wrapping_add(0xc1e);
                m.write8(ecx.wrapping_add(eax.wrapping_mul(0x4)), (((((edx) as u8)) as u32)) as u8);
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f4c));
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0xfi32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30476c; continue; }
                pc = 0x304755; continue;
            }
            0x304755 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x30476c; continue; }
                pc = 0x304759; continue;
            }
            0x304759 => {
                m.write32(eax.wrapping_add(0x2f4c), 0xffffffffu32);
                eax = m.read32(0x7844a8);
                edx = ecx;
                pc = 0x304771; continue;
            }
            0x30476c => {
                edx = 0x2;
                pc = 0x304771; continue;
            }
            0x304771 => {
                ecx = ((m.read16(eax.wrapping_add(0x306e))) as u16 as u32);
                ecx = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                m.write8(eax.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x3079), (((((edx) as u8)) as u32)) as u8);
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f34));
                edx = eax;
                edx = ((edx as i32).wrapping_mul((0xc4) as i32)) as u32;
                fa = ((((m.read16(edx.wrapping_add(0x524f5a)))) as u32) & 0xffffu32);
                fb = ((((((esi) as u16))) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x3047a4; continue; }
                pc = 0x30479f; continue;
            }
            0x30479f => {
                eax = (eax | 0xffffffffu32);
                pc = 0x3047ba; continue;
            }
            0x3047a4 => {
                eax = eax.wrapping_sub(0x46);
                fa = eax;
                if ((fa == 0)) { pc = 0x3047b8; continue; }
                pc = 0x3047a9; continue;
            }
            0x3047a9 => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x3047b1; continue; }
                pc = 0x3047ac; continue;
            }
            0x3047ac => {
                eax = (eax | 0xffffffffu32);
                pc = 0x3047ba; continue;
            }
            0x3047b1 => {
                eax = 0x1;
                pc = 0x3047ba; continue;
            }
            0x3047b8 => {
                eax = 0;
                pc = 0x3047ba; continue;
            }
            0x3047ba => {
                eax = eax.wrapping_sub(0);
                fa = eax;
                if ((fa == 0)) { pc = 0x3047d7; continue; }
                pc = 0x3047bf; continue;
            }
            0x3047bf => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x3047ce; continue; }
                pc = 0x3047c2; continue;
            }
            0x3047c2 => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0x3047e4; continue; }
                pc = 0x3047c5; continue;
            }
            0x3047c5 => {
                write16(m, ecx.wrapping_add(0x306c), (((((m.read16(ecx.wrapping_add(0x306c))) as i32).wrapping_add(0x1i32)) as u32)) as u16);
                pc = 0x3047de; continue;
            }
            0x3047ce => {
                write16(m, ecx.wrapping_add(0x306a), (((((m.read16(ecx.wrapping_add(0x306a))) as i32).wrapping_add(0x1i32)) as u32)) as u16);
                pc = 0x3047de; continue;
            }
            0x3047d7 => {
                write16(m, ecx.wrapping_add(0x3068), (((((m.read16(ecx.wrapping_add(0x3068))) as i32).wrapping_add(0x1i32)) as u32)) as u16);
                pc = 0x3047de; continue;
            }
            0x3047de => {
                ecx = m.read32(0x7844a8);
                pc = 0x3047e4; continue;
            }
            0x3047e4 => {
                write16(m, ecx.wrapping_add(0x306e), (((((m.read16(ecx.wrapping_add(0x306e))) as i32).wrapping_add(0x1i32)) as u32)) as u16);
                call!(0x73c80);
                pc = 0x3047f0; continue;
            }
            0x3047f0 => {
                eax = m.read32(0x7844a8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax.wrapping_add(0x2f34), 0xffffffffu32);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00304810: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f304810<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x304810;
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
        { crate::pose::cover(0x304810, previous, pc); previous = pc; }
        match pc {
            0x304810 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                esp = esp.wrapping_sub(0x30);
                push!(ebx);
                push!(esi);
                push!(edi);
                push!(ecx);
                call!(0x77470);
                pc = 0x30482d; continue;
            }
            0x30482d => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                edi = (edi | 0xffffffffu32);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                esi = 0x2000;
                edx = 0x22;
                if ((fa & fb) == 0) { pc = 0x3048a6; continue; }
                pc = 0x30484d; continue;
            }
            0x30484d => {
                ecx = m.read32(eax.wrapping_add(0x25c));
                fa = esi;
                fb = ecx;
                if ((fa & fb) != 0) { pc = 0x3048a6; continue; }
                pc = 0x304857; continue;
            }
            0x304857 => {
                ebx = m.read32(eax.wrapping_add(0x2f34));
                ebx = ((ebx as i32).wrapping_mul((0xc4) as i32)) as u32;
                fa = ((((m.read16(ebx.wrapping_add(0x524f5a)))) as u32) & 0xffffu32);
                fb = ((((((edx) as u16))) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x3048a6; continue; }
                pc = 0x30486c; continue;
            }
            0x30486c => {
                ebx = m.read32(eax.wrapping_add(0x2f70));
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3048a6; continue; }
                pc = 0x304876; continue;
            }
            0x304876 => {
                fa = m.read32(eax.wrapping_add(0x2f74));
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3048a6; continue; }
                pc = 0x30487e; continue;
            }
            0x30487e => {
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x3048a6; continue; }
                pc = 0x304883; continue;
            }
            0x304883 => {
                fa = ecx;
                fb = ((((0x410000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x3048a6; continue; }
                pc = 0x30488b; continue;
            }
            0x30488b => {
                edi = ecx;
                edi = (edi | esi);
                push!(0x1);
                m.write32(eax.wrapping_add(0x25c), edi);
                call!(0x304330);
                pc = 0x30489c; continue;
            }
            0x30489c => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            0x3048a6 => {
                ecx = m.read32(eax.wrapping_add(0x2f34));
                ecx = ((ecx as i32).wrapping_mul((0xc4) as i32)) as u32;
                fa = ((((m.read16(ecx.wrapping_add(0x524f5a)))) as u32) & 0xffffu32);
                fb = ((((((edx) as u16))) as u32) & 0xffffu32);
                ebx = (ebx & 0xffff_ff00) | ((0x80) & 0xff);
                if (fa != fb) { pc = 0x304973; continue; }
                pc = 0x3048c1; continue;
            }
            0x3048c1 => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x3048d9; continue;
            }
            0x3048d9 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30496e; continue; }
                pc = 0x3048ea; continue;
            }
            0x3048ea => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x234));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304919; continue; }
                pc = 0x3048fa; continue;
            }
            0x3048fa => {
                edx = esp.wrapping_add(0xc);
                push!(edx);
                call!(0x45390);
                pc = 0x304904; continue;
            }
            0x304904 => {
                eax = esp.wrapping_add(0x10);
                push!(eax);
                m.write32(esp.wrapping_add(0x14), 0x1577);
                call!(0x45820);
                pc = 0x304916; continue;
            }
            0x304916 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x304919; continue;
            }
            0x304919 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f38));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x304952; continue; }
                pc = 0x304929; continue;
            }
            0x304929 => {
                call!(0x2ffe10);
                pc = 0x30492e; continue;
            }
            0x30492e => {
                edx = m.read32(0x7844a8);
                edi = eax;
                eax = m.read32(edx.wrapping_add(0x2f34));
                push!(0);
                call!(0x2ffd50);
                pc = 0x304943; continue;
            }
            0x304943 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            0x304952 => {
                call!(0x73c80);
                pc = 0x304957; continue;
            }
            0x304957 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x2f34), edi);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            0x30496e => {
                eax = m.read32(0x7844a8);
                pc = 0x304973; continue;
            }
            0x304973 => {
                ecx = m.read32(eax.wrapping_add(0x2f34));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x304c23; continue; }
                pc = 0x304981; continue;
            }
            0x304981 => {
                ecx = m.read32(0x7ba9e0);
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(0x7ba26c));
                push!(edx);
                call!(0x77470);
                pc = 0x304999; continue;
            }
            0x304999 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x304a60; continue; }
                pc = 0x3049af; continue;
            }
            0x3049af => {
                fa = m.read32(eax.wrapping_add(0x2f78));
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x304a60; continue; }
                pc = 0x3049bb; continue;
            }
            0x3049bb => {
                push!(0x1577);
                call!(0x2f79e0);
                pc = 0x3049c5; continue;
            }
            0x3049c5 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f78));
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = ecx.wrapping_add(eax.wrapping_mul(0x4));
                esp = esp.wrapping_add(0x4);
                fa = ((((m.read16(eax.wrapping_add(0x307a)))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas < fbs) { pc = 0x304a0e; continue; }
                pc = 0x3049e4; continue;
            }
            0x3049e4 => {
                esi = ((m.read8(eax.wrapping_add(0x3077))) as u8 as u32);
                call!(0x2ffe10);
                pc = 0x3049f0; continue;
            }
            0x3049f0 => {
                push!(0);
                push!(esi);
                edi = eax;
                call!(0x2f8020);
                pc = 0x3049fa; continue;
            }
            0x3049fa => {
                esp = esp.wrapping_add(0x4);
                call!(0x2ffd50);
                pc = 0x304a02; continue;
            }
            0x304a02 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            0x304a0e => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x238))) as u32)) & 0xff);
                ebx = 0x20;
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((eax) as u8))) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x304a40; continue; }
                pc = 0x304a1d; continue;
            }
            0x304a1d => {
                push!(0);
                call!(0x3370a0);
                pc = 0x304a24; continue;
            }
            0x304a24 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | ebx);
                m.write32(eax.wrapping_add(0x238), ecx);
                ecx = m.read32(0x7844a8);
                pc = 0x304a40; continue;
            }
            0x304a40 => {
                ecx = m.read32(ecx.wrapping_add(0x2f78));
                push!(ecx);
                call!(0x2f7ef0);
                pc = 0x304a4c; continue;
            }
            0x304a4c => {
                esp = esp.wrapping_add(0x4);
                call!(0x73c80);
                pc = 0x304a54; continue;
            }
            0x304a54 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            0x304a60 => {
                ecx = m.read32(eax.wrapping_add(0x2f34));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x304c23; continue; }
                pc = 0x304a6e; continue;
            }
            0x304a6e => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x304a86; continue;
            }
            0x304a86 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x304ae3; continue; }
                pc = 0x304a99; continue;
            }
            0x304a99 => {
                fa = m.read32(eax.wrapping_add(0x25c));
                fb = esi;
                if ((fa & fb) != 0) { pc = 0x304ae3; continue; }
                pc = 0x304aa1; continue;
            }
            0x304aa1 => {
                fa = m.read32(eax.wrapping_add(0x2f78));
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x304ae3; continue; }
                pc = 0x304aa9; continue;
            }
            0x304aa9 => {
                push!(0x1578);
                call!(0x2f79e0);
                pc = 0x304ab3; continue;
            }
            0x304ab3 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x25c), (m.read32(eax.wrapping_add(0x25c)) | esi));
                ecx = m.read32(0x7844a8);
                esi = m.read32(ecx.wrapping_add(0x2f78));
                esp = esp.wrapping_add(0x4);
                call!(0x2f8040);
                pc = 0x304ad2; continue;
            }
            0x304ad2 => {
                call!(0x73c80);
                pc = 0x304ad7; continue;
            }
            0x304ad7 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            0x304ae3 => {
                ecx = m.read32(eax.wrapping_add(0x2f34));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x304c23; continue; }
                pc = 0x304af1; continue;
            }
            0x304af1 => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x304b09; continue;
            }
            0x304b09 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x304bbd; continue; }
                pc = 0x304b20; continue;
            }
            0x304b20 => {
                fa = m.read32(eax.wrapping_add(0x2f78));
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x304bbd; continue; }
                pc = 0x304b2c; continue;
            }
            0x304b2c => {
                push!(0x1578);
                call!(0x2f79e0);
                pc = 0x304b36; continue;
            }
            0x304b36 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f78));
                edx = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = ((m.read8(ecx.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x3077))) as u8 as u32);
                esp = esp.wrapping_add(0x4);
                eax = eax.wrapping_sub(0);
                fa = eax;
                if ((fa == 0)) { pc = 0x304b64; continue; }
                pc = 0x304b55; continue;
            }
            0x304b55 => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x304b5d; continue; }
                pc = 0x304b58; continue;
            }
            0x304b58 => {
                eax = eax.wrapping_sub(1);
                eax = edi;
                pc = 0x304b69; continue;
            }
            0x304b5d => {
                eax = 0x47;
                pc = 0x304b69; continue;
            }
            0x304b64 => {
                eax = 0x46;
                pc = 0x304b69; continue;
            }
            0x304b69 => {
                m.write32(ecx.wrapping_add(0x2f34), eax);
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f78));
                ecx = ecx.wrapping_add(ecx.wrapping_mul(0x2));
                edx = ((m.read8(eax.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x3079))) as u8 as u32);
                m.write32(eax.wrapping_add(0x2f4c), edx);
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f78));
                eax = eax.wrapping_add(0x40a);
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                edx = ((m.read8(ecx.wrapping_add(eax.wrapping_mul(0x4)))) as u8 as u32);
                m.write32(ecx.wrapping_add(0x2f54), edx);
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x2f38), edi);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x30);
                break 'run;
            }
            0x304bbd => {
                ecx = m.read32(eax.wrapping_add(0x2f34));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x304c23; continue; }
                pc = 0x304bc7; continue;
            }
            0x304bc7 => {
                ecx = m.read32(0x7ba9e0);
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(0x7ba26c));
                push!(edx);
                call!(0x77470);
                pc = 0x304bdf; continue;
            }
            0x304bdf => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x304c23; continue; }
                pc = 0x304bed; continue;
            }
            0x304bed => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f78));
                fa = eax;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x304c23; continue; }
                pc = 0x304bfd; continue;
            }
            0x304bfd => {
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                ecx = ((m.read8(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x3077))) as u8 as u32);
                push!(ecx);
                call!(0x2f8020);
                pc = 0x304c0e; continue;
            }
            0x304c0e => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0x47i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x304c23; continue; }
                pc = 0x304c16; continue;
            }
            0x304c16 => {
                push!(0x52de8c);
                call!(0x2fb230);
                pc = 0x304c20; continue;
            }
            0x304c20 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x304c23; continue;
            }
            0x304c23 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
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
