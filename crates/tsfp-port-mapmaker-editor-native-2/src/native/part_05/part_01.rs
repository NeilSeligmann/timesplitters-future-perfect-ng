use super::*;
#[rustfmt::skip] mod unformatted { use super::*;
/// 0x0031BB30: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31bb30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31bb30;
    let mut zflag = 0u32;
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
        { crate::pose::cover(0x31bb30, previous, pc); previous = pc; }
        match pc {
            0x31bb30 => {
                edx = ((m.read8(0x762f94)) as u8 as u32);
                eax = m.read32(0x7844a8);
                edx = ((edx as i32).wrapping_mul((0xe4) as i32)) as u32;
                push!(esi);
                push!(edi);
                edi = edx.wrapping_add(eax).wrapping_add(0x288);
                esi = eax.wrapping_add(0x11758);
                eax = 0;
                fa = eax;
                ecx = 0x39;
                zflag = (((fa == 0))) as u32;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; while ecx != 0 { zflag = (m.read32(esi) == m.read32(edi)) as u32; esi = esi.wrapping_add(step); edi = edi.wrapping_add(step); ecx = ecx.wrapping_sub(1); if zflag == 0 { break; } } }
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = (eax & 0xffff_ff00) | ((((if (((zflag == 0))) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
/// 0x0031BB60: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31bb60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31bb60;
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
        { crate::pose::cover(0x31bb60, previous, pc); previous = pc; }
        match pc {
            0x31bb60 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x11614));
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x280));
                m.write32(esp.wrapping_add(0x10), esi);
                esi = ((esi as i32).wrapping_mul((0xe4) as i32)) as u32;
                ecx = ecx.wrapping_add(1);
                m.write32(eax.wrapping_add(0x11614), ecx);
                edx = m.read32(0x7844a8);
                push!(edi);
                eax = 0;
                fa = ebp;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                ecx = 0x39;
                edi = esi.wrapping_add(edx).wrapping_add(0x288);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                if (fa != fb) { pc = 0x31bbcc; continue; }
                pc = 0x31bba5; continue;
            }
            0x31bba5 => {
                push!(0xbad);
                call!(0x3d7b0);
                pc = 0x31bbaf; continue;
            }
            0x31bbaf => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                edx = esi.wrapping_add(ecx).wrapping_add(0x288);
                pc = 0x31bbc0; continue;
            }
            0x31bbc0 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax)) as u32)) & 0xff);
                eax = eax.wrapping_add(1);
                m.write8(edx, (((((ecx) as u8)) as u32)) as u8);
                edx = edx.wrapping_add(1);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x31bbc0; continue; }
                pc = 0x31bbca; continue;
            }
            0x31bbca => {
                pc = 0x31bbfd; continue;
            }
            0x31bbcc => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x11614));
                push!(eax);
                push!(0xbac);
                call!(0x3d7b0);
                pc = 0x31bbe3; continue;
            }
            0x31bbe3 => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                push!(eax);
                edx = esi.wrapping_add(ecx).wrapping_add(0x288);
                push!(edx);
                call!(0x3c85d4);
                pc = 0x31bbfa; continue;
            }
            0x31bbfa => {
                esp = esp.wrapping_add(0xc);
                pc = 0x31bbfd; continue;
            }
            0x31bbfd => {
                eax = m.read32(0x7844a8);
                edx = 0;
                fa = ebp;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ebx = 0x1;
                m.write32(esi.wrapping_add(eax).wrapping_add(0x2a8), ebx);
                ecx = m.read32(0x7844a8);
                m.write32(esi.wrapping_add(ecx).wrapping_add(0x2bc), ebp);
                eax = m.read32(0x7844a8);
                edi = 0;
                edx = edx.wrapping_add(1);
                fa = ebp;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                m.write32(esi.wrapping_add(eax).wrapping_add(0x2cc), edx);
                ecx = m.read32(0x7844a8);
                m.write32(esi.wrapping_add(ecx).wrapping_add(0x2d4), 0x2);
                edx = m.read32(0x7844a8);
                m.write32(esi.wrapping_add(edx).wrapping_add(0x2c8), 0xffffffffu32);
                eax = m.read32(0x7844a8);
                m.write32(esi.wrapping_add(eax).wrapping_add(0x2e0), edi);
                ecx = m.read32(0x7844a8);
                m.write32(esi.wrapping_add(ecx).wrapping_add(0x2c0), ebx);
                edx = m.read32(0x7844a8);
                m.write32(esi.wrapping_add(edx).wrapping_add(0x2c4), edi);
                eax = m.read32(0x7844a8);
                m.write32(esi.wrapping_add(eax).wrapping_add(0x2b8), ebx);
                ecx = m.read32(0x7844a8);
                m.write32(esi.wrapping_add(ecx).wrapping_add(0x2d0), edi);
                if (fa != fb) { pc = 0x31bcb0; continue; }
                pc = 0x31bc98; continue;
            }
            0x31bc98 => {
                edx = m.read32(0x7844a8);
                eax = esi.wrapping_add(edx).wrapping_add(0x288);
                push!(eax);
                call!(0x308ac0);
                pc = 0x31bcab; continue;
            }
            0x31bcab => {
                esp = esp.wrapping_add(0x4);
                pc = 0x31bd05; continue;
            }
            0x31bcb0 => {
                edi = m.read32(0x7844a8);
                ebp = 0;
                pc = 0x31bcc0; continue;
            }
            0x31bcc0 => {
                ecx = m.read32(edi.wrapping_add(0x280));
                edx = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31bce3; continue; }
                pc = 0x31bccc; continue;
            }
            0x31bccc => {
                eax = edi.wrapping_add(0x2e8);
                pc = 0x31bcd2; continue;
            }
            0x31bcd2 => {
                fa = m.read32(eax);
                fb = ebp;
                if (fa == fb) { edx = ebx; }
                eax = eax.wrapping_add(0xe4);
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x31bcd2; continue; }
                pc = 0x31bcdf; continue;
            }
            0x31bcdf => {
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31bcf6; continue; }
                pc = 0x31bce3; continue;
            }
            0x31bce3 => {
                push!(ebp);
                call!(0x308aa0);
                pc = 0x31bce9; continue;
            }
            0x31bce9 => {
                edi = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31bcfc; continue; }
                pc = 0x31bcf6; continue;
            }
            0x31bcf6 => {
                ebp = ebp.wrapping_add(1);
                fa = ebp;
                fb = ((((0x32i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31bcc0; continue; }
                pc = 0x31bcfc; continue;
            }
            0x31bcfc => {
                m.write32(esi.wrapping_add(edi).wrapping_add(0x2e8), ebp);
                edi = 0;
                pc = 0x31bd05; continue;
            }
            0x31bd05 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x280), m.read32(eax.wrapping_add(0x280)).wrapping_add(0x1));
                eax = m.read32(esp.wrapping_add(0x18));
                fa = eax;
                fb = edi;
                if (fa == fb) { pc = 0x31bd21; continue; }
                pc = 0x31bd18; continue;
            }
            0x31bd18 => {
                push!(eax);
                call!(0x746d0);
                pc = 0x31bd1e; continue;
            }
            0x31bd1e => {
                esp = esp.wrapping_add(0x4);
                pc = 0x31bd21; continue;
            }
            0x31bd21 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x238), (m.read32(eax.wrapping_add(0x238)) | 0x40));
                ecx = m.read32(0x7844a8);
                eax = (eax & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x238))) as u32)) & 0xff);
                ebx = 0x20;
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((eax) as u8))) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x31bd5f; continue; }
                pc = 0x31bd42; continue;
            }
            0x31bd42 => {
                push!(0x11);
                call!(0x3370a0);
                pc = 0x31bd49; continue;
            }
            0x31bd49 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | ebx);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x31bd5f; continue;
            }
            0x31bd5f => {
                eax = m.read32(esp.wrapping_add(0x14));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031BDB0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31bdb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31bdb0;
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
        { crate::pose::cover(0x31bdb0, previous, pc); previous = pc; }
        match pc {
            0x31bdb0 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x280));
                eax = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31bdd8; continue; }
                pc = 0x31bdc2; continue;
            }
            0x31bdc2 => {
                ecx = ecx.wrapping_add(0x2bc);
                pc = 0x31bdc8; continue;
            }
            0x31bdc8 => {
                fa = m.read32(ecx);
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31bddb; continue; }
                pc = 0x31bdcd; continue;
            }
            0x31bdcd => {
                eax = eax.wrapping_add(1);
                ecx = ecx.wrapping_add(0xe4);
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31bdc8; continue; }
                pc = 0x31bdd8; continue;
            }
            0x31bdd8 => {
                eax = 0;
                break 'run;
            }
            0x31bddb => {
                eax = 0x1;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031BDF0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31bdf0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31bdf0;
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
        { crate::pose::cover(0x31bdf0, previous, pc); previous = pc; }
        match pc {
            0x31bdf0 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x280));
                eax = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31be18; continue; }
                pc = 0x31be02; continue;
            }
            0x31be02 => {
                ecx = ecx.wrapping_add(0x2bc);
                pc = 0x31be08; continue;
            }
            0x31be08 => {
                fa = m.read32(ecx);
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31be24; continue; }
                pc = 0x31be0d; continue;
            }
            0x31be0d => {
                eax = eax.wrapping_add(1);
                ecx = ecx.wrapping_add(0xe4);
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31be08; continue; }
                pc = 0x31be18; continue;
            }
            0x31be18 => {
                eax = 0;
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                eax = ecx;
                break 'run;
            }
            0x31be24 => {
                eax = 0x1;
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                eax = ecx;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031BE40: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31be40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31be40;
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
        { crate::pose::cover(0x31be40, previous, pc); previous = pc; }
        match pc {
            0x31be40 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x238), (m.read32(eax.wrapping_add(0x238)) | 0x1));
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x254));
                edx = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x52cd50));
                eax = m.read32(0x7842a8);
                push!(0xffffffffu32);
                push!(edx);
                push!(eax);
                call!(0x79860);
                pc = 0x31be6c; continue;
            }
            0x31be6c => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031C480: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31c480<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31c480;
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
        { crate::pose::cover(0x31c480, previous, pc); previous = pc; }
        match pc {
            0x31c480 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(esi.wrapping_add(0x18c));
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31c560; continue; }
                pc = 0x31c494; continue;
            }
            0x31c494 => {
                edx = m.read32(0x7844a8);
                push!(ebx);
                push!(edi);
                eax = ecx;
                eax = (((((eax as i32)).wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                edi = eax;
                edi = ((edi as i32).wrapping_mul((0xe4) as i32)) as u32;
                ebx = m.read32(edi.wrapping_add(edx).wrapping_add(0x2e0));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa != fb) { pc = 0x31c4dc; continue; }
                pc = 0x31c4b6; continue;
            }
            0x31c4b6 => {
                write16(m, edx.wrapping_add(0x2f10), (((((eax) as u16)) as u32)) as u16);
                eax = (eax & 0xffff_0000) | ((((m.read16(esi.wrapping_add(0x18c))) as u32)) & 0xffff);
                ecx = m.read32(0x7844a8);
                push!(esi);
                write16(m, ecx.wrapping_add(0x2f12), (((((eax) as u16)) as u32)) as u16);
                call!(0x2fb090);
                pc = 0x31c4d7; continue;
            }
            0x31c4d7 => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31c4dc => {
                xmm2 = Xmm::scalar(m.read32(0x475cd4));
                eax = ((eax as i32).wrapping_mul((0xfff1c) as i32)) as u32;
                xmm3 = Xmm::scalar(m.read32(0x783de0));
                ecx = ecx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_sub(eax);
                eax = m.read32(ecx.wrapping_add(edx).wrapping_add(0x2ec));
                edx = m.read32(eax.wrapping_add(0xc));
                ecx = m.read32(eax.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x2c));
                xmm0.lanes[0] = (((m.read32(ecx.wrapping_add(0x14))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                m.write32(0x783dd8, xmm0.lanes[0]);
                edx = m.read32(eax.wrapping_add(0xc));
                ecx = m.read32(eax.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x2c));
                xmm1.lanes[0] = (((m.read32(ecx.wrapping_add(0x18))) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(0x783dd8, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x783ddc));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                push!(esi);
                m.write32(0x783cc0, xmm1.lanes[0]);
                call!(0x2fb090);
                pc = 0x31c55d; continue;
            }
            0x31c55d => {
                esp = esp.wrapping_add(0x4);
                pc = 0x31c560; continue;
            }
            0x31c560 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031C8B0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31c8b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31c8b0;
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
        { crate::pose::cover(0x31c8b0, previous, pc); previous = pc; }
        match pc {
            0x31c8b0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(0x7844a8);
                push!(esi);
                esi = m.read32(ecx.wrapping_add(0x280));
                push!(edi);
                edi = eax;
                edi = ((edi as i32).wrapping_mul((0xe4) as i32)) as u32;
                edx = m.read32(edi.wrapping_add(ecx).wrapping_add(0x2bc));
                esi = esi.wrapping_sub(eax);
                esi = esi.wrapping_sub(1);
                fa = edx;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31c8e9; continue; }
                pc = 0x31c8d9; continue;
            }
            0x31c8d9 => {
                fa = ((((m.read8(ecx.wrapping_add(0x238)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x31c8e9; continue; }
                pc = 0x31c8e2; continue;
            }
            0x31c8e2 => {
                m.write32(ecx.wrapping_add(0x238), (m.read32(ecx.wrapping_add(0x238)) & 0xfffffffeu32));
                pc = 0x31c8e9; continue;
            }
            0x31c8e9 => {
                push!(eax);
                call!(0x323330);
                pc = 0x31c8ef; continue;
            }
            0x31c8ef => {
                eax = m.read32(0x7844a8);
                ecx = esi;
                ecx = ((ecx as i32).wrapping_mul((0xe4) as i32)) as u32;
                esi = edi.wrapping_add(eax).wrapping_add(0x36c);
                edi = edi.wrapping_add(eax).wrapping_add(0x288);
                eax = ecx;
                ecx = ecx.wrapping_shr((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                { let step = if entry.eflags & 0x400 == 0 { 4u32 } else { 0u32.wrapping_sub(4) }; for i in 0..ecx { let value=m.read32(esi.wrapping_add(i.wrapping_mul(step)));m.write32(edi.wrapping_add(i.wrapping_mul(step)),value); } esi=esi.wrapping_add(ecx.wrapping_mul(step));edi=edi.wrapping_add(ecx.wrapping_mul(step));ecx=0; }
                ecx = eax;
                ecx = (ecx & 0x3);
                { let step = if entry.eflags & 0x400 == 0 { 1u32 } else { 0u32.wrapping_sub(1) }; for i in 0..ecx { let value=m.read8(esi.wrapping_add(i.wrapping_mul(step)));m.write8(edi.wrapping_add(i.wrapping_mul(step)),value); } esi=esi.wrapping_add(ecx.wrapping_mul(step));edi=edi.wrapping_add(ecx.wrapping_mul(step));ecx=0; }
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x280));
                esp = esp.wrapping_add(0x4);
                ecx = ecx.wrapping_sub(1);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax.wrapping_add(0x280), ecx);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031C9E0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31c9e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31c9e0;
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
        { crate::pose::cover(0x31c9e0, previous, pc); previous = pc; }
        match pc {
            0x31c9e0 => {
                eax = m.read32(0x7844a8);
                fa = ((((m.read8(eax.wrapping_add(0x238)))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                if ((fa & fb) == 0) { pc = 0x31ca15; continue; }
                pc = 0x31c9f3; continue;
            }
            0x31c9f3 => {
                push!(0x3f800000);
                push!(esi);
                call!(0x736f0);
                pc = 0x31c9fe; continue;
            }
            0x31c9fe => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x8);
                ecx = (ecx & 0xffffffbfu32);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x31ca15; continue;
            }
            0x31ca15 => {
                push!(0x3);
                push!(esi);
                call!(0x74860);
                pc = 0x31ca1d; continue;
            }
            0x31ca1d => {
                eax = m.read32(esi.wrapping_add(0x18c));
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31cab2; continue; }
                pc = 0x31ca2f; continue;
            }
            0x31ca2f => {
                ecx = m.read32(0x7844a8);
                fa = eax;
                fb = m.read32(ecx.wrapping_add(0x280));
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x31cab2; continue; }
                pc = 0x31ca3d; continue;
            }
            0x31ca3d => {
                eax = m.read32(esi.wrapping_add(0x190));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31cab2; continue; }
                pc = 0x31ca47; continue;
            }
            0x31ca47 => {
                push!(0);
                push!(0);
                push!(0xa9f);
                call!(0x3d7b0);
                pc = 0x31ca55; continue;
            }
            0x31ca55 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                push!(esi);
                call!(0x74990);
                pc = 0x31ca61; continue;
            }
            0x31ca61 => {
                eax = m.read32(esi.wrapping_add(0xc4));
                esp = esp.wrapping_add(0x14);
                fa = eax;
                fb = ((((0x800000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x31cab2; continue; }
                pc = 0x31ca71; continue;
            }
            0x31ca71 => {
                eax = m.read32(esi.wrapping_add(0x18c));
                push!(eax);
                m.write32(0x52e074, eax);
                call!(0x31e1f0);
                pc = 0x31ca82; continue;
            }
            0x31ca82 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31caa9; continue; }
                pc = 0x31ca89; continue;
            }
            0x31ca89 => {
                push!(0);
                push!(0x31c950);
                push!(0xb8b);
                call!(0x3d7b0);
                pc = 0x31ca9a; continue;
            }
            0x31ca9a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(esi);
                call!(0x7a080);
                pc = 0x31caa4; continue;
            }
            0x31caa4 => {
                esp = esp.wrapping_add(0x10);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31caa9 => {
                push!(esi);
                call!(0x31c950);
                pc = 0x31caaf; continue;
            }
            0x31caaf => {
                esp = esp.wrapping_add(0x4);
                pc = 0x31cab2; continue;
            }
            0x31cab2 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031D2C0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31d2c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31d2c0;
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
        { crate::pose::cover(0x31d2c0, previous, pc); previous = pc; }
        match pc {
            0x31d2c0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                edx = m.read32(ecx.wrapping_add(0x34));
                eax = 0;
                fa = edx;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031D2D0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31d2d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31d2d0;
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
        { crate::pose::cover(0x31d2d0, previous, pc); previous = pc; }
        match pc {
            0x31d2d0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = m.read32(eax.wrapping_add(0x34));
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x31d2e7; continue; }
                pc = 0x31d2da; continue;
            }
            0x31d2da => {
                eax = eax.wrapping_sub(0x2);
                fa = eax;
                if ((fa == 0)) { pc = 0x31d2e7; continue; }
                pc = 0x31d2df; continue;
            }
            0x31d2df => {
                eax = eax.wrapping_sub(0x2);
                fa = eax;
                if ((fa == 0)) { pc = 0x31d2e7; continue; }
                pc = 0x31d2e4; continue;
            }
            0x31d2e4 => {
                eax = 0;
                break 'run;
            }
            0x31d2e7 => {
                eax = 0x1;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031D360: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31d360<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31d360;
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
        { crate::pose::cover(0x31d360, previous, pc); previous = pc; }
        match pc {
            0x31d360 => {
                push!(esi);
                esi = m.read32(0x7844a8);
                edx = m.read32(esi.wrapping_add(0x280));
                eax = 0;
                ecx = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31d397; continue; }
                pc = 0x31d375; continue;
            }
            0x31d375 => {
                esi = esi.wrapping_add(0x2e0);
                pc = 0x31d380; continue;
            }
            0x31d380 => {
                fa = m.read32(esi);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x31d392; continue; }
                pc = 0x31d385; continue;
            }
            0x31d385 => {
                ecx = ecx.wrapping_add(1);
                esi = esi.wrapping_add(0xe4);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31d380; continue; }
                pc = 0x31d390; continue;
            }
            0x31d390 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31d392 => {
                eax = 0x1;
                pc = 0x31d397; continue;
            }
            0x31d397 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031D3A0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31d3a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31d3a0;
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
        { crate::pose::cover(0x31d3a0, previous, pc); previous = pc; }
        match pc {
            0x31d3a0 => {
                push!(0);
                push!(0x5);
                call!(0x31bb60);
                pc = 0x31d3a9; continue;
            }
            0x31d3a9 => {
                ecx = eax;
                eax = m.read32(esp.wrapping_add(0xc));
                esp = esp.wrapping_add(0x8);
                ecx = ecx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write32(eax.wrapping_add(0x18c), ecx);
                edx = m.read32(0x7844a8);
                m.write32(edx.wrapping_add(0x254), 0x3);
                m.write32(esp.wrapping_add(0x4), eax);
                tail_to!(0x31c480); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031D4C0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31d4c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31d4c0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x31d4c0, previous, pc); previous = pc; }
        match pc {
            0x31d4c0 => {
                ecx = m.read32(0x7844a8);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                eax = esi;
                eax = ((eax as i32).wrapping_mul((0xe4) as i32)) as u32;
                push!(edi);
                push!(0x2);
                push!(0);
                push!(0);
                edi = eax.wrapping_add(ecx).wrapping_add(0x288);
                call!(0x78320);
                pc = 0x31d4e6; continue;
            }
            0x31d4e6 => {
                edx = m.read32(esp.wrapping_add(0x1c));
                esp = esp.wrapping_add(0xc);
                push!(0x31bf60);
                push!(edx);
                push!(0x412ccccc);
                call!(0x71c60);
                pc = 0x31d4fd; continue;
            }
            0x31d4fd => {
                { let value=round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x49f7f8)) as f64),fpu.control);fpu.stack[((fpu.top+0)&7) as usize]=value; }
                push!(ecx);
                m.write32(esp, (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(0x40000);
                push!(esi);
                push!(0);
                call!(0x792b0);
                pc = 0x31d514; continue;
            }
            0x31d514 => {
                edx = m.read32(edi.wrapping_add(0x34));
                eax = 0;
                fa = edx;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                eax = eax.wrapping_sub(1);
                eax = (eax & 0xfffffffau32);
                eax = eax.wrapping_add(0xb1);
                push!(eax);
                call!(0x716d0);
                pc = 0x31d52e; continue;
            }
            0x31d52e => {
                ecx = m.read32(esp.wrapping_add(0x34));
                push!(0x31d410);
                push!(ecx);
                push!(0x412ccccc);
                push!(0);
                push!(0);
                esi = esi.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(esi);
                push!(0);
                call!(0x792b0);
                pc = 0x31d54c; continue;
            }
            0x31d54c => {
                ecx = m.read32(edi.wrapping_add(0x34));
                edx = 0;
                fa = ecx;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edx = edx.wrapping_sub(1);
                edx = (edx & 0xfffffffau32);
                edx = edx.wrapping_add(0xb2);
                push!(edx);
                call!(0x716d0);
                pc = 0x31d567; continue;
            }
            0x31d567 => {
                esp = esp.wrapping_add(0x40);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x78380); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031D9F0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31d9f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31d9f0;
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
        { crate::pose::cover(0x31d9f0, previous, pc); previous = pc; }
        match pc {
            0x31d9f0 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0xdd10));
                eax = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31da26; continue; }
                pc = 0x31da02; continue;
            }
            0x31da02 => {
                push!(ebx);
                push!(esi);
                ecx = ecx.wrapping_add(0xdd94);
                esi = edx;
                pc = 0x31da10; continue;
            }
            0x31da10 => {
                ebx = m.read32(ecx);
                edx = 0;
                fa = ebx;
                fb = edi;
                edx = (edx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ecx = ecx.wrapping_add(0x8c);
                eax = eax.wrapping_add(edx);
                esi = esi.wrapping_sub(1);
                fa = esi;
                if ((fa != 0)) { pc = 0x31da10; continue; }
                pc = 0x31da24; continue;
            }
            0x31da24 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x31da26; continue;
            }
            0x31da26 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DA30: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31da30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31da30;
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
        { crate::pose::cover(0x31da30, previous, pc); previous = pc; }
        match pc {
            0x31da30 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x280));
                eax = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DA50: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31da50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31da50;
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
        { crate::pose::cover(0x31da50, previous, pc); previous = pc; }
        match pc {
            0x31da50 => {
                eax = m.read32(0x762f98);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DA80: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31da80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31da80;
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
        { crate::pose::cover(0x31da80, previous, pc); previous = pc; }
        match pc {
            0x31da80 => {
                eax = m.read32(0x783a50);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DAB0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31dab0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31dab0;
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
        { crate::pose::cover(0x31dab0, previous, pc); previous = pc; }
        match pc {
            0x31dab0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(0x783a18);
                eax = eax.wrapping_add(ecx);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DAC0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31dac0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31dac0;
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
        { crate::pose::cover(0x31dac0, previous, pc); previous = pc; }
        match pc {
            0x31dac0 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x41e0));
                eax = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31daf6; continue; }
                pc = 0x31dad2; continue;
            }
            0x31dad2 => {
                ecx = ecx.wrapping_add(0x41e8);
                push!(edi);
                pc = 0x31dae0; continue;
            }
            0x31dae0 => {
                edi = ((m.read8(ecx)) as u8 as u32);
                edi = edi.wrapping_sub(esi);
                cf = (((((edi) != 0))) as i32);
                edi = (((((edi) as i32).wrapping_neg())) as u32);
                edi = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                edi = edi.wrapping_add(1);
                eax = eax.wrapping_add(edi);
                ecx = ecx.wrapping_add(0x220);
                edx = edx.wrapping_sub(1);
                fa = edx;
                if ((fa != 0)) { pc = 0x31dae0; continue; }
                pc = 0x31daf5; continue;
            }
            0x31daf5 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x31daf6; continue;
            }
            0x31daf6 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DB70: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31db70<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31db70;
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
        { crate::pose::cover(0x31db70, previous, pc); previous = pc; }
        match pc {
            0x31db70 => {
                edx = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(edx);
                eax = 0;
                fa = ecx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31db82; continue; }
                pc = 0x31db7d; continue;
            }
            0x31db7d => {
                fa = ecx;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31dbb3; continue; }
                pc = 0x31db82; continue;
            }
            0x31db82 => {
                fa = m.read32(edx.wrapping_add(0x4));
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31dbb3; continue; }
                pc = 0x31db88; continue;
            }
            0x31db88 => {
                ecx = (((((m.read16(edx.wrapping_add(0xe)) as i16)) as i32)) as u32);
                edx = m.read32(0x7844a8);
                ecx = ((ecx as i32).wrapping_mul((0xe4) as i32)) as u32;
                ecx = m.read32(ecx.wrapping_add(edx).wrapping_add(0x2bc));
                fa = ecx;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31dbae; continue; }
                pc = 0x31dba4; continue;
            }
            0x31dba4 => {
                fa = ecx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31dbae; continue; }
                pc = 0x31dba9; continue;
            }
            0x31dba9 => {
                fa = ecx;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31dbb3; continue; }
                pc = 0x31dbae; continue;
            }
            0x31dbae => {
                eax = 0x1;
                pc = 0x31dbb3; continue;
            }
            0x31dbb3 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DDA0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31dda0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31dda0;
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
        { crate::pose::cover(0x31dda0, previous, pc); previous = pc; }
        match pc {
            0x31dda0 => {
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = m.read32(0x7844a8);
                eax = m.read32(edi.wrapping_add(0x41e0));
                ebp = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31de16; continue; }
                pc = 0x31ddb6; continue;
            }
            0x31ddb6 => {
                ebx = edi.wrapping_add(0x4270);
                pc = 0x31ddc0; continue;
            }
            0x31ddc0 => {
                esi = m.read32(ebx.wrapping_add(((0x4i32.wrapping_neg()) as u32)));
                edx = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31de05; continue; }
                pc = 0x31ddc9; continue;
            }
            0x31ddc9 => {
                ecx = ebx;
                pc = 0x31ddd0; continue;
            }
            0x31ddd0 => {
                eax = m.read32(ecx);
                eax = ((eax as i32).wrapping_mul((0xb4) as i32)) as u32;
                fa = m.read32(eax.wrapping_add(edi).wrapping_add(0x81ac));
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                eax = eax.wrapping_add(edi).wrapping_add(0x81a8);
                if (fa != fb) { pc = 0x31ddfd; continue; }
                pc = 0x31dde9; continue;
            }
            0x31dde9 => {
                eax = (((((m.read16(eax.wrapping_add(0xc)) as i16)) as i32)) as u32);
                eax = ((eax as i32).wrapping_mul((0x8c) as i32)) as u32;
                fa = m.read32(eax.wrapping_add(edi).wrapping_add(0xdd90));
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31de1d; continue; }
                pc = 0x31ddfd; continue;
            }
            0x31ddfd => {
                edx = edx.wrapping_add(1);
                ecx = ecx.wrapping_add(0x4);
                fa = edx;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31ddd0; continue; }
                pc = 0x31de05; continue;
            }
            0x31de05 => {
                eax = m.read32(edi.wrapping_add(0x41e0));
                ebp = ebp.wrapping_add(1);
                ebx = ebx.wrapping_add(0x220);
                fa = ebp;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31ddc0; continue; }
                pc = 0x31de16; continue;
            }
            0x31de16 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31de1d => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DE40: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31de40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31de40;
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
        { crate::pose::cover(0x31de40, previous, pc); previous = pc; }
        match pc {
            0x31de40 => {
                push!(esi);
                esi = m.read32(0x7844a8);
                edx = 0;
                esi = esi.wrapping_add(0x113c4);
                pc = 0x31de50; continue;
            }
            0x31de50 => {
                eax = 0;
                ecx = esi;
                pc = 0x31de54; continue;
            }
            0x31de54 => {
                fa = edi;
                fb = ecx;
                if (fa == fb) { pc = 0x31de6c; continue; }
                pc = 0x31de58; continue;
            }
            0x31de58 => {
                eax = eax.wrapping_add(1);
                ecx = ecx.wrapping_add(0x18);
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31de54; continue; }
                pc = 0x31de61; continue;
            }
            0x31de61 => {
                edx = edx.wrapping_add(1);
                esi = esi.wrapping_add(0x60);
                fa = edx;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31de50; continue; }
                pc = 0x31de6a; continue;
            }
            0x31de6a => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31de6c => {
                ecx = m.read32(esp.wrapping_add(0xc));
                m.write32(ecx, edx);
                edx = m.read32(esp.wrapping_add(0x8));
                eax = eax.wrapping_add(0x2);
                m.write32(edx, eax);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DE90: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31de90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31de90;
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
        { crate::pose::cover(0x31de90, previous, pc); previous = pc; }
        match pc {
            0x31de90 => {
                eax = m.read32(esp.wrapping_add(0x8));
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & 0x98);
                eax = eax.wrapping_add(0xc8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DEB0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31deb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31deb0;
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
        { crate::pose::cover(0x31deb0, previous, pc); previous = pc; }
        match pc {
            0x31deb0 => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(esp.wrapping_add(0x4));
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & 0x98);
                eax = eax.wrapping_add(0xc8);
                ecx = ecx.wrapping_sub(0x7);
                fa = ecx;
                if ((fa == 0)) { pc = 0x31dedb; continue; }
                pc = 0x31decb; continue;
            }
            0x31decb => {
                ecx = ecx.wrapping_sub(0x2);
                fa = ecx;
                if ((fa == 0)) { pc = 0x31dedb; continue; }
                pc = 0x31ded0; continue;
            }
            0x31ded0 => {
                ecx = ecx.wrapping_sub(0x5);
                fa = ecx;
                if ((fa != 0)) { pc = 0x31dede; continue; }
                pc = 0x31ded5; continue;
            }
            0x31ded5 => {
                eax = eax.wrapping_add(0xc8);
                break 'run;
            }
            0x31dedb => {
                eax = eax.wrapping_add(0x40);
                pc = 0x31dede; continue;
            }
            0x31dede => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DEE0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31dee0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31dee0;
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
        { crate::pose::cover(0x31dee0, previous, pc); previous = pc; }
        match pc {
            0x31dee0 => {
                eax = m.read32(esp.wrapping_add(0x8));
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & 0x98);
                eax = eax.wrapping_add(0xdc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DF00: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31df00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31df00;
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
        { crate::pose::cover(0x31df00, previous, pc); previous = pc; }
        match pc {
            0x31df00 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                eax = 0x2f0;
                if ((fa != 0)) { pc = 0x31df11; continue; }
                pc = 0x31df0c; continue;
            }
            0x31df0c => {
                eax = 0x518;
                pc = 0x31df11; continue;
            }
            0x31df11 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DF20: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31df20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31df20;
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
        { crate::pose::cover(0x31df20, previous, pc); previous = pc; }
        match pc {
            0x31df20 => {
                eax = 0xc8;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DF50: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31df50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31df50;
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
        { crate::pose::cover(0x31df50, previous, pc); previous = pc; }
        match pc {
            0x31df50 => {
                eax = 0;
                m.write32(0x762f98, eax);
                m.write32(0x778d3c, eax);
                m.write32(0x77d840, eax);
                eax = m.read32(esp.wrapping_add(0x4));
                m.write32(0x783a1c, 0x1);
                m.write32(0x76307c, eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DF80: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31df80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31df80;
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
        { crate::pose::cover(0x31df80, previous, pc); previous = pc; }
        match pc {
            0x31df80 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31df92; continue; }
                pc = 0x31df84; continue;
            }
            0x31df84 => {
                ecx = m.read32(0x7844a8);
                ecx = m.read32(ecx.wrapping_add(0x248));
                pc = 0x31df94; continue;
            }
            0x31df92 => {
                ecx = 0;
                pc = 0x31df94; continue;
            }
            0x31df94 => {
                edx = ecx;
                eax = ((eax as i32).wrapping_mul((0xd) as i32)) as u32;
                edx = ((edx as i32).wrapping_mul((0x44) as i32)) as u32;
                edx = m.read32(edx.wrapping_add(0x52e8e8));
                eax = eax.wrapping_add(ecx);
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x52e6c8));
                push!(edx);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x31dfb2; continue;
            }
            0x31dfb2 => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(ecx);
                call!(0x3c85d4);
                pc = 0x31dfc2; continue;
            }
            0x31dfc2 => {
                edx = m.read32(0x7844a8);
                eax = m.read32(esp.wrapping_add(0x10));
                push!(0);
                push!(edx);
                push!(eax);
                call!(0x7a270);
                pc = 0x31dfd5; continue;
            }
            0x31dfd5 => {
                esp = esp.wrapping_add(0x18);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031E060: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31e060<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31e060;
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
        { crate::pose::cover(0x31e060, previous, pc); previous = pc; }
        match pc {
            0x31e060 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x81a4));
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x8));
                push!(edi);
                edi = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31e0ed; continue; }
                pc = 0x31e077; continue;
            }
            0x31e077 => {
                push!(esi);
                esi = 0;
                pc = 0x31e080; continue;
            }
            0x31e080 => {
                ecx = m.read32(esi.wrapping_add(eax).wrapping_add(0x81ac));
                fa = ecx;
                fb = ((((0xfi32)) as u32) & 0xffffffffu32);
                edx = esi.wrapping_add(eax).wrapping_add(0x81a8);
                if (fa != fb) { pc = 0x31e0d6; continue; }
                pc = 0x31e093; continue;
            }
            0x31e093 => {
                ecx = (((((m.read16(edx.wrapping_add(0xc)) as i16)) as i32)) as u32);
                ecx = ((ecx as i32).wrapping_mul((0x8c) as i32)) as u32;
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = ecx.wrapping_add(eax).wrapping_add(0xdd14);
                if (fa == fb) { pc = 0x31e0be; continue; }
                pc = 0x31e0a8; continue;
            }
            0x31e0a8 => {
                ecx = m.read32(eax.wrapping_add(0x7c));
                fa = ecx;
                fb = ebp;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31e0b5; continue; }
                pc = 0x31e0af; continue;
            }
            0x31e0af => {
                ecx = ecx.wrapping_sub(1);
                m.write32(eax.wrapping_add(0x7c), ecx);
                pc = 0x31e0be; continue;
            }
            0x31e0b5 => {
                if (fa != fb) { pc = 0x31e0be; continue; }
                pc = 0x31e0b7; continue;
            }
            0x31e0b7 => {
                m.write32(eax.wrapping_add(0x7c), 0xffffffffu32);
                pc = 0x31e0be; continue;
            }
            0x31e0be => {
                ecx = m.read32(esp.wrapping_add(0x14));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31e0d6; continue; }
                pc = 0x31e0c6; continue;
            }
            0x31e0c6 => {
                ecx = m.read32(eax.wrapping_add(0x7c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x31e0d6; continue; }
                pc = 0x31e0cd; continue;
            }
            0x31e0cd => {
                push!(edx);
                call!(0x322d40);
                pc = 0x31e0d3; continue;
            }
            0x31e0d3 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x31e0d6; continue;
            }
            0x31e0d6 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x81a4));
                edi = edi.wrapping_add(1);
                esi = esi.wrapping_add(0xb4);
                fa = edi;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31e080; continue; }
                pc = 0x31e0ec; continue;
            }
            0x31e0ec => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x31e0ed; continue;
            }
            0x31e0ed => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031E0F0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31e0f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31e0f0;
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
        { crate::pose::cover(0x31e0f0, previous, pc); previous = pc; }
        match pc {
            0x31e0f0 => {
                push!(edi);
                edi = m.read32(0x7844a8);
                eax = m.read32(edi.wrapping_add(0x81a4));
                edx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31e151; continue; }
                pc = 0x31e103; continue;
            }
            0x31e103 => {
                push!(ebx);
                push!(ebp);
                push!(esi);
                ebp = 0;
                pc = 0x31e110; continue;
            }
            0x31e110 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(ebp).wrapping_add(0x8258))) as u32)) & 0xff);
                ebx = m.read32(esp.wrapping_add(0x14));
                eax = edi.wrapping_add(ebp).wrapping_add(0x8258);
                esi = ((((ecx) as u8)) as u8 as u32);
                fa = esi;
                fb = ebx;
                if ((fa & fb) == 0) { pc = 0x31e13d; continue; }
                pc = 0x31e129; continue;
            }
            0x31e129 => {
                ebx = m.read32(esp.wrapping_add(0x18));
                fa = ebx;
                fb = esi;
                if ((fa & fb) == 0) { pc = 0x31e13d; continue; }
                pc = 0x31e131; continue;
            }
            0x31e131 => {
                ebx = (ebx & 0xffff_ff00) | ((((!((ebx) as u8)) as u32)) & 0xff);
                ebx = (ebx & 0xffff_ff00) | (((((((((ebx) as u8)) as i32) & ((((ecx) as u8)) as i32))) as u32)) & 0xff);
                m.write8(eax, (((((ebx) as u8)) as u32)) as u8);
                edi = m.read32(0x7844a8);
                pc = 0x31e13d; continue;
            }
            0x31e13d => {
                eax = m.read32(edi.wrapping_add(0x81a4));
                edx = edx.wrapping_add(1);
                ebp = ebp.wrapping_add(0xb4);
                fa = edx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31e110; continue; }
                pc = 0x31e14e; continue;
            }
            0x31e14e => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x31e151; continue;
            }
            0x31e151 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031E160: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31e160<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31e160;
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
        { crate::pose::cover(0x31e160, previous, pc); previous = pc; }
        match pc {
            0x31e160 => {
                push!(ebx);
                push!(esi);
                esi = m.read32(0x7844a8);
                ebx = m.read32(esi.wrapping_add(0x41e0));
                push!(edi);
                edi = 0;
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31e1e1; continue; }
                pc = 0x31e175; continue;
            }
            0x31e175 => {
                eax = esi.wrapping_add(0x4238);
                pc = 0x31e180; continue;
            }
            0x31e180 => {
                fa = ((m.read32(eax.wrapping_add(((0x4i32.wrapping_neg()) as u32)))) & 0xffffffffu32);
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31e1ca; continue; }
                pc = 0x31e186; continue;
            }
            0x31e186 => {
                ecx = m.read32(eax);
                edx = m.read32(eax.wrapping_add(0x38));
                ecx = ((ecx as i32).wrapping_mul((0xb4) as i32)) as u32;
                edx = ((edx as i32).wrapping_mul((0xb4) as i32)) as u32;
                fa = m.read32(ecx.wrapping_add(esi).wrapping_add(0x81ac));
                fb = ((((0x8i32)) as u32) & 0xffffffffu32);
                ecx = ecx.wrapping_add(esi).wrapping_add(0x81a8);
                edx = edx.wrapping_add(esi).wrapping_add(0x81a8);
                if (fa != fb) { pc = 0x31e1ca; continue; }
                pc = 0x31e1af; continue;
            }
            0x31e1af => {
                fa = m.read32(ecx.wrapping_add(0x8));
                fb = ((((0x13i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31e1ca; continue; }
                pc = 0x31e1b5; continue;
            }
            0x31e1b5 => {
                fa = m.read32(edx.wrapping_add(0xa8));
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31e1ca; continue; }
                pc = 0x31e1be; continue;
            }
            0x31e1be => {
                fa = m.read32(edx.wrapping_add(0x4));
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31e1ca; continue; }
                pc = 0x31e1c4; continue;
            }
            0x31e1c4 => {
                fa = m.read32(edx.wrapping_add(0x8));
                fb = ((((0x11i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31e1d8; continue; }
                pc = 0x31e1ca; continue;
            }
            0x31e1ca => {
                edi = edi.wrapping_add(1);
                eax = eax.wrapping_add(0x220);
                fa = edi;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31e180; continue; }
                pc = 0x31e1d4; continue;
            }
            0x31e1d4 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31e1d8 => {
                eax = m.read32(eax.wrapping_add(((0x54i32.wrapping_neg()) as u32)));
                m.write32(esi.wrapping_add(0x284), eax);
                pc = 0x31e1e1; continue;
            }
            0x31e1e1 => {
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

/// 0x0031E250: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31e250<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31e250;
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
        { crate::pose::cover(0x31e250, previous, pc); previous = pc; }
        match pc {
            0x31e250 => {
                edx = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                esi = m.read32(edx.wrapping_add(0x88));
                eax = 0;
                ecx = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31e297; continue; }
                pc = 0x31e263; continue;
            }
            0x31e263 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0xc));
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x14));
                edx = edx.wrapping_add(0x8c);
                push!(ebp);
                pc = 0x31e274; continue;
            }
            0x31e274 => {
                ebp = m.read32(edx);
                ebp = ((ebp as i32).wrapping_mul((0xb4) as i32)) as u32;
                ebp = ebp.wrapping_add(edi);
                fa = ebx;
                fb = ebp;
                if (fa == fb) { pc = 0x31e28f; continue; }
                pc = 0x31e282; continue;
            }
            0x31e282 => {
                ecx = ecx.wrapping_add(1);
                edx = edx.wrapping_add(0x4);
                fa = ecx;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31e274; continue; }
                pc = 0x31e28a; continue;
            }
            0x31e28a => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31e28f => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x31e297; continue;
            }
            0x31e297 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031E2A0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31e2a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31e2a0;
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
        { crate::pose::cover(0x31e2a0, previous, pc); previous = pc; }
        match pc {
            0x31e2a0 => {
                edx = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                esi = m.read32(edx.wrapping_add(0x4));
                eax = 0;
                ecx = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31e2e3; continue; }
                pc = 0x31e2b0; continue;
            }
            0x31e2b0 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0xc));
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x14));
                edx = edx.wrapping_add(0x8);
                push!(ebp);
                edi = edi;
                pc = 0x31e2c0; continue;
            }
            0x31e2c0 => {
                ebp = m.read32(edx);
                ebp = ((ebp as i32).wrapping_mul((0xb4) as i32)) as u32;
                ebp = ebp.wrapping_add(edi);
                fa = ebx;
                fb = ebp;
                if (fa == fb) { pc = 0x31e2db; continue; }
                pc = 0x31e2ce; continue;
            }
            0x31e2ce => {
                ecx = ecx.wrapping_add(1);
                edx = edx.wrapping_add(0x4);
                fa = ecx;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31e2c0; continue; }
                pc = 0x31e2d6; continue;
            }
            0x31e2d6 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31e2db => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x31e2e3; continue;
            }
            0x31e2e3 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031E2F0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31e2f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31e2f0;
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
        { crate::pose::cover(0x31e2f0, previous, pc); previous = pc; }
        match pc {
            0x31e2f0 => {
                fa = m.read32(ebx.wrapping_add(0x8));
                fb = ((((0x13i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31e2fd; continue; }
                pc = 0x31e2f6; continue;
            }
            0x31e2f6 => {
                fa = ((((m.read16(ebx.wrapping_add(0xc)))) as u32) & 0xffffu32);
                fb = ((0xffffffffu32) & 0xffffu32);
                if (fa == fb) { pc = 0x31e306; continue; }
                pc = 0x31e2fd; continue;
            }
            0x31e2fd => {
                fa = ((((m.read8(ebx.wrapping_add(0xb0)))) as u32) & 0xffu32);
                fb = ((((0x3ci32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x31e30c; continue; }
                pc = 0x31e306; continue;
            }
            0x31e306 => {
                eax = 0x1;
                break 'run;
            }
            0x31e30c => {
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = m.read32(0x7844a8);
                eax = m.read32(edi.wrapping_add(0x41e0));
                ebp = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31e368; continue; }
                pc = 0x31e321; continue;
            }
            0x31e321 => {
                esi = edi.wrapping_add(0x41e4);
                pc = 0x31e327; continue;
            }
            0x31e327 => {
                eax = m.read32(ebx);
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x31e33f; continue; }
                pc = 0x31e32c; continue;
            }
            0x31e32c => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0x31e357; continue; }
                pc = 0x31e32f; continue;
            }
            0x31e32f => {
                push!(esi);
                eax = edi.wrapping_add(0x81a8);
                push!(eax);
                push!(ebx);
                call!(0x31e250);
                pc = 0x31e33d; continue;
            }
            0x31e33d => {
                pc = 0x31e350; continue;
            }
            0x31e33f => {
                ecx = esi.wrapping_add(0x4c);
                push!(ecx);
                edx = edi.wrapping_add(0x81a8);
                push!(edx);
                push!(ebx);
                call!(0x31e2a0);
                pc = 0x31e350; continue;
            }
            0x31e350 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31e36e; continue; }
                pc = 0x31e357; continue;
            }
            0x31e357 => {
                eax = m.read32(edi.wrapping_add(0x41e0));
                ebp = ebp.wrapping_add(1);
                esi = esi.wrapping_add(0x220);
                fa = ebp;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31e327; continue; }
                pc = 0x31e368; continue;
            }
            0x31e368 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31e36e => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031E380: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31e380<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31e380;
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
        { crate::pose::cover(0x31e380, previous, pc); previous = pc; }
        match pc {
            0x31e380 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = (((((m.read16(eax.wrapping_add(0xc)) as i16)) as i32)) as u32);
                edx = m.read32(0x7844a8);
                ecx = ((ecx as i32).wrapping_mul((0x8c) as i32)) as u32;
                push!(esi);
                esi = m.read32(ecx.wrapping_add(edx).wrapping_add(0xdd34));
                eax = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = (eax & 0xffff_ff00) | ((((if ((fas > fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031E3B0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31e3b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31e3b0;
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
        { crate::pose::cover(0x31e3b0, previous, pc); previous = pc; }
        match pc {
            0x31e3b0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                eax = (((((m.read16(ecx.wrapping_add(0xc)) as i16)) as i32)) as u32);
                edx = eax;
                edx = ((edx as i32).wrapping_mul((0x8c) as i32)) as u32;
                push!(esi);
                esi = m.read32(0x7844a8);
                esi = edx.wrapping_add(esi).wrapping_add(0xdd14);
                fa = m.read32(ecx.wrapping_add(0x8));
                fb = ((((0x9i32)) as u32) & 0xffffffffu32);
                push!(eax);
                push!(esi);
                if (fa == fb) { pc = 0x31e3e9; continue; }
                pc = 0x31e3d6; continue;
            }
            0x31e3d6 => {
                call!(0x3083c0);
                pc = 0x31e3db; continue;
            }
            0x31e3db => {
                ecx = 0;
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fas > fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ecx;
                break 'run;
            }
            0x31e3e9 => {
                call!(0x3083c0);
                pc = 0x31e3ee; continue;
            }
            0x31e3ee => {
                ecx = m.read32(esi.wrapping_add(0x20));
                edx = 0;
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                edx = (edx & 0xffff_ff00) | ((((if ((fas >= fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = edx;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031E400: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31e400<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31e400;
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
        { crate::pose::cover(0x31e400, previous, pc); previous = pc; }
        match pc {
            0x31e400 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = (((((m.read16(eax.wrapping_add(0xc)) as i16)) as i32)) as u32);
                edx = m.read32(0x7844a8);
                ecx = ((ecx as i32).wrapping_mul((0x8c) as i32)) as u32;
                push!(esi);
                esi = m.read32(ecx.wrapping_add(edx).wrapping_add(0xdd84));
                eax = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = (eax & 0xffff_ff00) | ((((if ((fas > fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031E490: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31e490<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31e490;
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
        { crate::pose::cover(0x31e490, previous, pc); previous = pc; }
        match pc {
            0x31e490 => {
                ecx = m.read32(eax);
                eax = m.read32(0x7844a8);
                ecx = ((ecx as i32).wrapping_mul((0xb4) as i32)) as u32;
                push!(esi);
                esi = m.read32(0x78400c);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(edi);
                m.write32(edx, 0);
                edi = ecx.wrapping_add(eax).wrapping_add(0x81a8);
                m.write32(edx.wrapping_add(0x4), 0);
                if (fa == fb) { pc = 0x31e509; continue; }
                pc = 0x31e4bd; continue;
            }
            0x31e4bd => {
                push!(ebx);
                push!(ebp);
                ebx = 0x1;
                pc = 0x31e4c4; continue;
            }
            0x31e4c4 => {
                eax = m.read32(esi.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31e500; continue; }
                pc = 0x31e4cb; continue;
            }
            0x31e4cb => {
                pc = 0x31e4d0; continue;
            }
            0x31e4d0 => {
                ecx = m.read32(eax);
                fa = ecx;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31e4e6; continue; }
                pc = 0x31e4d7; continue;
            }
            0x31e4d7 => {
                ebp = m.read32(eax.wrapping_add(0x18));
                fa = ebp;
                fb = m.read32(edi.wrapping_add(0xac));
                if (fa != fb) { pc = 0x31e4e6; continue; }
                pc = 0x31e4e2; continue;
            }
            0x31e4e2 => {
                m.write32(edx, ebx);
                pc = 0x31e4f9; continue;
            }
            0x31e4e6 => {
                fa = ecx;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31e4f9; continue; }
                pc = 0x31e4eb; continue;
            }
            0x31e4eb => {
                ecx = m.read32(eax.wrapping_add(0x18));
                fa = ecx;
                fb = m.read32(edi.wrapping_add(0xac));
                if (fa != fb) { pc = 0x31e4f9; continue; }
                pc = 0x31e4f6; continue;
            }
            0x31e4f6 => {
                m.write32(edx.wrapping_add(0x4), ebx);
                pc = 0x31e4f9; continue;
            }
            0x31e4f9 => {
                eax = m.read32(eax.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31e4d0; continue; }
                pc = 0x31e500; continue;
            }
            0x31e500 => {
                esi = m.read32(esi.wrapping_add(0x40));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31e4c4; continue; }
                pc = 0x31e507; continue;
            }
            0x31e507 => {
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x31e509; continue;
            }
            0x31e509 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031E510: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31e510<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31e510;
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
        { crate::pose::cover(0x31e510, previous, pc); previous = pc; }
        match pc {
            0x31e510 => {
                eax = m.read32(ebx);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(ebx.wrapping_add(0x4))) as u32)) & 0xff);
                eax = ((eax as i32).wrapping_mul((0xb4) as i32)) as u32;
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                push!(edi);
                edi = m.read32(0x7844a8);
                eax = eax.wrapping_add(edi).wrapping_add(0x81a8);
                if (fa == fb) { pc = 0x31e532; continue; }
                pc = 0x31e52e; continue;
            }
            0x31e52e => {
                eax = 0;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31e532 => {
                push!(ebp);
                push!(esi);
                esi = m.read32(edi.wrapping_add(0x41e0));
                edx = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31e572; continue; }
                pc = 0x31e540; continue;
            }
            0x31e540 => {
                ecx = edi.wrapping_add(0x41e4);
                pc = 0x31e546; continue;
            }
            0x31e546 => {
                fa = ((((m.read8(ecx.wrapping_add(0x4)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x31e567; continue; }
                pc = 0x31e54c; continue;
            }
            0x31e54c => {
                fa = ecx;
                fb = ebx;
                if (fa == fb) { pc = 0x31e567; continue; }
                pc = 0x31e550; continue;
            }
            0x31e550 => {
                ebp = m.read32(ecx);
                ebp = ((ebp as i32).wrapping_mul((0xb4) as i32)) as u32;
                ebp = m.read32(edi.wrapping_add(ebp).wrapping_add(0x8254));
                fa = ebp;
                fb = m.read32(eax.wrapping_add(0xac));
                if (fa == fb) { pc = 0x31e578; continue; }
                pc = 0x31e567; continue;
            }
            0x31e567 => {
                edx = edx.wrapping_add(1);
                ecx = ecx.wrapping_add(0x220);
                fa = edx;
                fb = esi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31e546; continue; }
                pc = 0x31e572; continue;
            }
            0x31e572 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31e578 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031E590: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31e590<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31e590;
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
        { crate::pose::cover(0x31e590, previous, pc); previous = pc; }
        match pc {
            0x31e590 => {
                edx = m.read32(ecx.wrapping_add(0x50));
                push!(ebx);
                eax = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(esi);
                if (fas <= fbs) { pc = 0x31e5c9; continue; }
                pc = 0x31e59b; continue;
            }
            0x31e59b => {
                esi = m.read32(0x7844a8);
                ecx = ecx.wrapping_add(0x54);
                pc = 0x31e5b0; continue;
            }
            0x31e5b0 => {
                ebx = m.read32(ecx);
                ebx = ((ebx as i32).wrapping_mul((0xb4) as i32)) as u32;
                fa = m.read32(ebx.wrapping_add(esi).wrapping_add(0x81b0));
                fb = edi;
                if (fa == fb) { pc = 0x31e5ce; continue; }
                pc = 0x31e5c1; continue;
            }
            0x31e5c1 => {
                eax = eax.wrapping_add(1);
                ecx = ecx.wrapping_add(0x4);
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31e5b0; continue; }
                pc = 0x31e5c9; continue;
            }
            0x31e5c9 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31e5ce => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
