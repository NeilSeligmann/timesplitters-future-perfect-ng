use super::*;
#[rustfmt::skip] mod unformatted { use super::*;

/// 0x00316200: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f316200<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x316200;
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
        { crate::pose::cover(0x316200, previous, pc); previous = pc; }
        match pc {
            0x316200 => {
                push!(ebx);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x10));
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(edi.wrapping_add(0x3c));
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31622e; continue; }
                pc = 0x316214; continue;
            }
            0x316214 => {
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x316226; continue; }
                pc = 0x316219; continue;
            }
            0x316219 => {
                ebx = m.read32(ecx);
                edx = 0;
                fa = ebx;
                fb = esi;
                edx = (edx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                eax = eax.wrapping_add(edx);
                pc = 0x316227; continue;
            }
            0x316226 => {
                eax = eax.wrapping_add(1);
                pc = 0x316227; continue;
            }
            0x316227 => {
                ecx = m.read32(ecx.wrapping_add(0x14));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x316214; continue; }
                pc = 0x31622e; continue;
            }
            0x31622e => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x280));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3162a4; continue; }
                pc = 0x31623e; continue;
            }
            0x31623e => {
                ecx = ecx.wrapping_add(0x2fc);
                pc = 0x316244; continue;
            }
            0x316244 => {
                fa = ((m.read32(ecx.wrapping_add(((0x10i32.wrapping_neg()) as u32)))) & 0xffffffffu32);
                fb = edi;
                if (fa != fb) { pc = 0x31624f; continue; }
                pc = 0x316249; continue;
            }
            0x316249 => {
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31624f; continue; }
                pc = 0x31624e; continue;
            }
            0x31624e => {
                eax = eax.wrapping_add(1);
                pc = 0x31624f; continue;
            }
            0x31624f => {
                fa = m.read32(ecx);
                fb = edi;
                if (fa != fb) { pc = 0x316259; continue; }
                pc = 0x316253; continue;
            }
            0x316253 => {
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x316259; continue; }
                pc = 0x316258; continue;
            }
            0x316258 => {
                eax = eax.wrapping_add(1);
                pc = 0x316259; continue;
            }
            0x316259 => {
                fa = m.read32(ecx.wrapping_add(0x10));
                fb = edi;
                if (fa != fb) { pc = 0x316264; continue; }
                pc = 0x31625e; continue;
            }
            0x31625e => {
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x316264; continue; }
                pc = 0x316263; continue;
            }
            0x316263 => {
                eax = eax.wrapping_add(1);
                pc = 0x316264; continue;
            }
            0x316264 => {
                fa = m.read32(ecx.wrapping_add(0x20));
                fb = edi;
                if (fa != fb) { pc = 0x31626f; continue; }
                pc = 0x316269; continue;
            }
            0x316269 => {
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31626f; continue; }
                pc = 0x31626e; continue;
            }
            0x31626e => {
                eax = eax.wrapping_add(1);
                pc = 0x31626f; continue;
            }
            0x31626f => {
                fa = m.read32(ecx.wrapping_add(0x30));
                fb = edi;
                if (fa != fb) { pc = 0x31627a; continue; }
                pc = 0x316274; continue;
            }
            0x316274 => {
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31627a; continue; }
                pc = 0x316279; continue;
            }
            0x316279 => {
                eax = eax.wrapping_add(1);
                pc = 0x31627a; continue;
            }
            0x31627a => {
                fa = m.read32(ecx.wrapping_add(0x40));
                fb = edi;
                if (fa != fb) { pc = 0x316285; continue; }
                pc = 0x31627f; continue;
            }
            0x31627f => {
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x316285; continue; }
                pc = 0x316284; continue;
            }
            0x316284 => {
                eax = eax.wrapping_add(1);
                pc = 0x316285; continue;
            }
            0x316285 => {
                fa = m.read32(ecx.wrapping_add(0x50));
                fb = edi;
                if (fa != fb) { pc = 0x316290; continue; }
                pc = 0x31628a; continue;
            }
            0x31628a => {
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x316290; continue; }
                pc = 0x31628f; continue;
            }
            0x31628f => {
                eax = eax.wrapping_add(1);
                pc = 0x316290; continue;
            }
            0x316290 => {
                fa = m.read32(ecx.wrapping_add(0x60));
                fb = edi;
                if (fa != fb) { pc = 0x31629b; continue; }
                pc = 0x316295; continue;
            }
            0x316295 => {
                fa = esi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31629b; continue; }
                pc = 0x31629a; continue;
            }
            0x31629a => {
                eax = eax.wrapping_add(1);
                pc = 0x31629b; continue;
            }
            0x31629b => {
                ecx = ecx.wrapping_add(0xe4);
                edx = edx.wrapping_sub(1);
                fa = edx;
                if ((fa != 0)) { pc = 0x316244; continue; }
                pc = 0x3162a4; continue;
            }
            0x3162a4 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003162B0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3162b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3162b0;
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
        { crate::pose::cover(0x3162b0, previous, pc); previous = pc; }
        match pc {
            0x3162b0 => {
                ecx = m.read32(0x52daf8);
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x3162d1; continue; }
                pc = 0x3162bc; continue;
            }
            0x3162bc => {
                edx = m.read32(esp.wrapping_add(0x4));
                edx = m.read32(edx.wrapping_add(0x10));
                pc = 0x3162c3; continue;
            }
            0x3162c3 => {
                fa = edx;
                fb = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x52da70));
                if (fa == fb) { pc = 0x3162d4; continue; }
                pc = 0x3162cc; continue;
            }
            0x3162cc => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x3162c3; continue; }
                pc = 0x3162d1; continue;
            }
            0x3162d1 => {
                eax = 0;
                break 'run;
            }
            0x3162d4 => {
                eax = 0x1;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x003162E0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3162e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3162e0;
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
        { crate::pose::cover(0x3162e0, previous, pc); previous = pc; }
        match pc {
            0x3162e0 => {
                eax = m.read32(esp.wrapping_add(0x8));
                xmm0 = Xmm::scalar(m.read32(0x4760d0));
                esp = esp.wrapping_sub(0x8);
                fa = eax;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3162fc; continue; }
                pc = 0x3162f4; continue;
            }
            0x3162f4 => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                pc = 0x3162fc; continue;
            }
            0x3162fc => {
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x1c));
                eax = m.read32(edi.wrapping_add(0xc));
                ebx = m.read32(edi.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x2c));
                eax = m.read32(ebx.wrapping_add(0x44));
                ebp = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x14), ebx);
                if (fas <= fbs) { pc = 0x3163a2; continue; }
                pc = 0x316322; continue;
            }
            0x316322 => {
                ebx = ebx.wrapping_add(0x48);
                pc = 0x316325; continue;
            }
            0x316325 => {
                ecx = ((m.read8(ebx)) as u8 as u32);
                fa = ecx;
                fb = m.read32(esp.wrapping_add(0x20));
                if (fa != fb) { pc = 0x316393; continue; }
                pc = 0x31632e; continue;
            }
            0x31632e => {
                edx = ((m.read8(ebx.wrapping_add(0x3))) as u8 as u32);
                eax = m.read32(edi.wrapping_add(0x4));
                push!(ecx);
                ecx = m.read32(edi);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                push!(ecx);
                call!(0x307e60);
                pc = 0x316353; continue;
            }
            0x316353 => {
                esi = eax;
                esp = esp.wrapping_add(0xc);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x316393; continue; }
                pc = 0x31635c; continue;
            }
            0x31635c => {
                eax = m.read32(esp.wrapping_add(0x20));
                eax = eax.wrapping_sub(0x5);
                fa = eax;
                if ((fa == 0)) { pc = 0x31637f; continue; }
                pc = 0x316365; continue;
            }
            0x316365 => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0x316393; continue; }
                pc = 0x316368; continue;
            }
            0x316368 => {
                push!(esi);
                call!(0x331a80);
                pc = 0x31636e; continue;
            }
            0x31636e => {
                esp = esp.wrapping_add(0x4);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                fa = eax;
                m.write32(esp.wrapping_add(0x1c), eax);
                if ((fa != 0)) { pc = 0x3163bb; continue; }
                pc = 0x31637d; continue;
            }
            0x31637d => {
                pc = 0x316393; continue;
            }
            0x31637f => {
                push!(esi);
                call!(0x331a80);
                pc = 0x316385; continue;
            }
            0x316385 => {
                esp = esp.wrapping_add(0x4);
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = eax.wrapping_add(1);
                fa = eax;
                m.write32(esp.wrapping_add(0x1c), eax);
                if ((fa != 0)) { pc = 0x3163ac; continue; }
                pc = 0x316393; continue;
            }
            0x316393 => {
                edx = m.read32(esp.wrapping_add(0x14));
                eax = m.read32(edx.wrapping_add(0x44));
                ebp = ebp.wrapping_add(1);
                ebx = ebx.wrapping_add(0x6);
                fa = ebp;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x316325; continue; }
                pc = 0x3163a2; continue;
            }
            0x3163a2 => {
                eax = 0;
                pc = 0x3163a4; continue;
            }
            0x3163a4 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x3163ac => {
                push!(0x5);
                push!(esi);
                call!(0x3162e0);
                pc = 0x3163b4; continue;
            }
            0x3163b4 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x3163a4; continue; }
                pc = 0x3163bb; continue;
            }
            0x3163bb => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
