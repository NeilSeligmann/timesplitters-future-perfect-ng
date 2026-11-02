use super::*; // portreg: replaces=tsfp-ports/mapmaker_ui_lane
#[rustfmt::skip] mod unformatted { use super::*;
/// 0x00070E30: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f70e30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x70e30;
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
        { crate::pose::cover(0x70e30, previous, pc); previous = pc; }
        match pc {
            0x70e30 => {
                eax = m.read32(0x7bb1e0);
                eax = m.read32(eax.wrapping_add(0x4));
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
/// 0x000802C0: Tier 0, bounded zero-state smoke; no differential evidence.
// portreg: va=0x000802c0 tier=0
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f802c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x802c0;
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
        { crate::pose::cover(0x802c0, previous, pc); previous = pc; }
        match pc {
            0x802c0 => {
                eax = m.read32(0x701cb8);
                ecx = m.read32(esp.wrapping_add(0x4));
                fa = ecx;
                fb = m.read32(eax.wrapping_add(0x90));
                if (fa == fb) { pc = 0x802df; continue; }
                pc = 0x802d1; continue;
            }
            0x802d1 => {
                eax = m.read32(eax.wrapping_add(0x94));
                fa = ecx;
                fb = m.read32(eax.wrapping_add(0x90));
                if (fa != fb) { pc = 0x802d1; continue; }
                pc = 0x802df; continue;
            }
            0x802df => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
/// 0x00081480: Tier 0, bounded zero-state smoke; no differential evidence.
// portreg: va=0x00081480 tier=0
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f81480<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x81480;
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
        { crate::pose::cover(0x81480, previous, pc); previous = pc; }
        match pc {
            0x81480 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(0x7497b0)) as u32)) & 0xff);
                ecx = (ecx & 0xffff_ff00) | ((((((eax) as u8)) as u32)) & 0xff);
                ecx = (ecx & 0xffff_ff00) | (((((((((ecx) as u8)) as i32) & 0x7fi32)) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if (fa < fb) { pc = 0x81495; continue; }
                pc = 0x8148f; continue;
            }
            0x8148f => {
                edx = (edx & 0xffff_ff00) | ((0x80) & 0xff);
                edx = (edx & 0xffff_ff00) | ((((((((edx) as u8)) as i32).wrapping_sub(((((ecx) as u8)) as i32))) as u32)) & 0xff);
                ecx = (ecx & 0xffff_ff00) | ((((((edx) as u8)) as u32)) & 0xff);
                pc = 0x81495; continue;
            }
            0x81495 => {
                eax = (eax & 0xffff_ff00) | (((((((((eax) as u8)) as i32) & 0x20i32)) as u32)) & 0xff);
                ecx = (ecx & 0xffff_ff00) | ((((((((ecx) as u8)) as i32).wrapping_add(0x40i32)) as u32)) & 0xff);
                cf = (((((((((eax) as u8))) as i32) != 0i32))) as i32);
                eax = (eax & 0xffff_ff00) | (((((((((eax) as u8)) as i32).wrapping_neg())) as u32)) & 0xff);
                eax = (eax & 0xffff_ff00) | ((if (cf) != 0 { 0xffffffffu32 } else { 0 }) & 0xff);
                eax = (eax & 0xffff_ff00) | (((((((((eax) as u8)) as i32) & 0x40i32)) as u32)) & 0xff);
                eax = (eax & 0xffff_ff00) | ((((((((eax) as u8)) as i32).wrapping_add(0x40i32)) as u32)) & 0xff);
                m.write8(0x7ba034, (((((ecx) as u8)) as u32)) as u8);
                m.write8(0x7ba17c, (((((eax) as u8)) as u32)) as u8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00081C10: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f81c10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x81c10;
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
        { crate::pose::cover(0x81c10, previous, pc); previous = pc; }
        match pc {
            0x81c10 => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x7ba040));
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00081C40: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f81c40<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x81c40;
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
        { crate::pose::cover(0x81c40, previous, pc); previous = pc; }
        match pc {
            0x81c40 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x7ba040));
                edx = m.read32(0x4b85c8);
                ecx = ((ecx as i32).wrapping_mul((0x34) as i32)) as u32;
                eax = m.read32(edx);
                eax = ((m.read16(ecx.wrapping_add(eax).wrapping_add(0x2a))) as u16 as u32);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FA030: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fa030<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fa030;
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
        { crate::pose::cover(0x2fa030, previous, pc); previous = pc; }
        match pc {
            0x2fa030 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fa058; continue; }
                pc = 0x2fa03a; continue;
            }
            0x2fa03a => {
                pc = 0x2fa040; continue;
            }
            0x2fa040 => {
                edx = m.read32(ecx.wrapping_add(0x2c));
                edx = m.read32(edx);
                ecx = m.read32(ecx.wrapping_add(0x40));
                edx = edx.wrapping_add(edx.wrapping_mul(0x4));
                edx = edx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(m.read32(edx.wrapping_add(0x529400)));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fa040; continue; }
                pc = 0x2fa058; continue;
            }
            0x2fa058 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE450: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe450<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe450;
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
        { crate::pose::cover(0x2fe450, previous, pc); previous = pc; }
        match pc {
            0x2fe450 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                eax = m.read32(ecx.wrapping_add(0x84));
                fa = eax;
                fb = ((((0x35i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2fe46f; continue; }
                pc = 0x2fe45f; continue;
            }
            0x2fe45f => {
                fa = eax;
                fb = ((((0x37i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x2fe469; continue; }
                pc = 0x2fe464; continue;
            }
            0x2fe464 => {
                fa = eax;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fe46f; continue; }
                pc = 0x2fe469; continue;
            }
            0x2fe469 => {
                eax = 0x1;
                break 'run;
            }
            0x2fe46f => {
                edx = m.read32(ecx.wrapping_add(0x80));
                eax = 0;
                fa = edx;
                fb = ((((0xbi32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00308B10: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308b10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308b10;
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
        { crate::pose::cover(0x308b10, previous, pc); previous = pc; }
        match pc {
            0x308b10 => {
                esp = esp.wrapping_sub(0x8);
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                xh[0] = Xmm::scalar(m.read32(0x783dd8));
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xh[1] = Xmm::scalar(m.read32(0x783f2c));
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xh[2] = Xmm::scalar(m.read32(0x783cc0));
                xmm4.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[2].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4 = Xmm::scalar(m.read32(0x475c78));
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x4), xmm2.lanes[0]);
                m.write32(esp, xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x10), xmm3.lanes[0]);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x308bbd; continue; }
                pc = 0x308bb3; continue;
            }
            0x308bb3 => {
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm1.lanes[0]);
                pc = 0x308bbd; continue;
            }
            0x308bbd => {
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                fca = f32::from_bits(xmm4.lanes[0]) as f64; fcb = f32::from_bits(xmm3.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x308bd0; continue; }
                pc = 0x308bc6; continue;
            }
            0x308bc6 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm2.lanes[0]);
                pc = 0x308bd0; continue;
            }
            0x308bd0 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                ecx = m.read32(esp.wrapping_add(0xc));
                edx = m.read32(esp.wrapping_add(0x10));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x4));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0xc));
                push!(edx);
                push!(eax);
                push!(ecx);
                call!(0x1a2e0);
                pc = 0x308bee; continue;
            }
            0x308bee => {
                esp = esp.wrapping_add(0x1c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00308C00: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f308c00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x308c00;
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
        { crate::pose::cover(0x308c00, previous, pc); previous = pc; }
        match pc {
            0x308c00 => {
                esp = esp.wrapping_sub(0x30);
                eax = m.read32(esp.wrapping_add(0x60));
                xmm2 = Xmm::scalar(m.read32(0x783dd8));
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm3 = Xmm::scalar(m.read32(0x783f2c));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm4 = Xmm::scalar(m.read32(0x783cc0));
                xh[0] = Xmm::scalar(m.read32(0x783f28));
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                ecx = eax;
                ecx = (ecx & 0xff00);
                edx = eax;
                edx = edx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                edx = eax;
                edx = (edx & 0xff0000);
                ecx = ecx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                eax = eax.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | eax);
                eax = m.read32(esp.wrapping_add(0x5c));
                edx = edx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                m.write32(esp.wrapping_add(0x24), xmm1.lanes[0]);
                xmm1.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x2c), xh[1].lanes[0]);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x18), xh[1].lanes[0]);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                push!(ecx);
                m.write32(esp.wrapping_add(0x24), xh[1].lanes[0]);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                ecx = eax;
                ecx = (ecx & 0xff0000);
                edx = eax;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                m.write32(esp.wrapping_add(0x10), xh[1].lanes[0]);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x4c)));
                edx = eax;
                edx = (edx & 0xff00);
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                eax = eax.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | eax);
                eax = m.read32(esp.wrapping_add(0x5c));
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                ecx = ecx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                m.write32(esp.wrapping_add(0x18), xh[1].lanes[0]);
                xh[1] = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                push!(ecx);
                xh[1].lanes[0] = sse::subss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::mulss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xh[1].lanes[0] = sse::addss(f32::from_bits(xh[1].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x58)));
                ecx = eax;
                edx = eax;
                ecx = (ecx & 0xff0000);
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                ecx = (ecx | edx);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x8), xh[1].lanes[0]);
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                ecx = ecx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = eax;
                edx = (edx & 0xff00);
                eax = eax.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | eax);
                eax = m.read32(esp.wrapping_add(0x5c));
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                push!(ecx);
                ecx = eax;
                edx = eax;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx & 0xff0000);
                ecx = (ecx | edx);
                edx = eax;
                edx = (edx & 0xff00);
                eax = eax.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | eax);
                eax = m.read32(esp.wrapping_add(0x14));
                ecx = ecx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                edx = m.read32(esp.wrapping_add(0x20));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x10));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x20));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x38));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x34));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x4c));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x48));
                push!(edx);
                push!(eax);
                push!(ecx);
                call!(0x1a990);
                pc = 0x308dc4; continue;
            }
            0x308dc4 => {
                esp = esp.wrapping_add(0x60);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003091C0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3091c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3091c0;
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
        { crate::pose::cover(0x3091c0, previous, pc); previous = pc; }
        match pc {
            0x3091c0 => {
                esp = esp.wrapping_sub(0x88);
                push!(esi);
                push!(0x12);
                call!(0x3fa10);
                pc = 0x3091ce; continue;
            }
            0x3091ce => {
                esi = eax;
                eax = m.read32(esp.wrapping_add(0x94));
                eax = m.read32(eax.wrapping_add(0x18));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3092d7; continue; }
                pc = 0x3091e5; continue;
            }
            0x3091e5 => {
                xmm1.lanes[0] = (((m.read32(0x4e8cf0)) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x47d7c4))).to_bits();
                xmm0 = Xmm::scalar(m.read32(0x783f2c));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x3092ce; continue; }
                pc = 0x309206; continue;
            }
            0x309206 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x94)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475cdc))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475db4))).to_bits();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x98)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4788e8))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                xmm1.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                if (fa != fb) { pc = 0x30929d; continue; }
                pc = 0x309275; continue;
            }
            0x309275 => {
                ecx = m.read32(esp.wrapping_add(0x8));
                edx = m.read32(esp.wrapping_add(0x4));
                push!(0x3f800000);
                push!(ecx);
                push!(edx);
                call!(0x31eb40);
                pc = 0x309289; continue;
            }
            0x309289 => {
                esp = esp.wrapping_add(0xc);
                push!(esi);
                call!(0x3fa10);
                pc = 0x309292; continue;
            }
            0x309292 => {
                esp = esp.wrapping_add(0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x88);
                break 'run;
            }
            0x30929d => {
                push!(eax);
                eax = esp.wrapping_add(0x10);
                push!(0x4760fc);
                push!(eax);
                call!(0x3c85d4);
                pc = 0x3092ad; continue;
            }
            0x3092ad => {
                edx = m.read32(esp.wrapping_add(0x14));
                eax = m.read32(esp.wrapping_add(0x10));
                push!(0xf0f0f40);
                push!(0x7f7f7f80);
                ecx = esp.wrapping_add(0x20);
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x41e30);
                pc = 0x3092cb; continue;
            }
            0x3092cb => {
                esp = esp.wrapping_add(0x20);
                pc = 0x3092ce; continue;
            }
            0x3092ce => {
                push!(esi);
                call!(0x3fa10);
                pc = 0x3092d4; continue;
            }
            0x3092d4 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x3092d7; continue;
            }
            0x3092d7 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x88);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003092E0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3092e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3092e0;
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
        { crate::pose::cover(0x3092e0, previous, pc); previous = pc; }
        match pc {
            0x3092e0 => {
                esp = esp.wrapping_sub(0x14);
                push!(ebx);
                push!(ebp);
                push!(esi);
                esi = eax;
                eax = m.read32(0x7844a8);
                ecx = m.read32(esi.wrapping_add(0xc));
                xmm0.lanes[0] = (((m.read32(eax.wrapping_add(0x264))) as i32) as f32).to_bits();
                edx = m.read32(esi.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x2c));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x8)))).to_bits();
                eax = m.read32(edx.wrapping_add(0x1c));
                push!(edi);
                edi = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                fa = edi;
                fb = eax;
                if (fa >= fb) { pc = 0x30968a; continue; }
                pc = 0x309311; continue;
            }
            0x309311 => {
                ebx = m.read32(esp.wrapping_add(0x28));
                ebp = (ebp & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x2c))) as u32)) & 0xffff);
                esp = esp.wrapping_sub(0x8);
                eax = esp;
                m.write32(eax, ebx);
                write16(m, eax.wrapping_add(0x4), (((((ebp) as u16)) as u32)) as u16);
                call!(0x331c70);
                pc = 0x30932a; continue;
            }
            0x30932a => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x309363; continue; }
                pc = 0x309331; continue;
            }
            0x309331 => {
                edx = (((((m.read16(esp.wrapping_add(0x2a)) as i16)) as i32)) as u32);
                ecx = ((((ebx) as u16)) as u16 as i16 as i32 as u32);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi))).to_bits();
                eax = ((((ebp) as u16)) as u32);
                fa = eax;
                fb = edi;
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esi.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                if (fa != fb) { pc = 0x30968a; continue; }
                pc = 0x309361; continue;
            }
            0x309361 => {
                pc = 0x3093e2; continue;
            }
            0x309363 => {
                eax = m.read32(0x7ba9e0);
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba280)));
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba284)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd4))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                ecx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                xmm1 = Xmm::scalar(m.read32(0x475cd4));
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f20))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm2.lanes[0]));
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                pc = 0x3093e2; continue;
            }
            0x3093e2 => {
                push!(0x12);
                call!(0x3fa10);
                pc = 0x3093e9; continue;
            }
            0x3093e9 => {
                xmm2 = Xmm::scalar(m.read32(0x475cd4));
                xmm1 = Xmm::scalar(m.read32(0x475d4c));
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0 = Xmm::scalar(m.read32(0x783f2c));
                xmm4.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm4.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                esi = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x783f28));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                push!(0x31);
                m.write32(esp.wrapping_add(0x28), eax);
                edi = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                ebx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                ebp = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                call!(0x81c10);
                pc = 0x309478; continue;
            }
            0x309478 => {
                push!(eax);
                call!(0x1ed50);
                pc = 0x30947e; continue;
            }
            0x30947e => {
                eax = m.read32(esp.wrapping_add(0x3c));
                esp = esp.wrapping_add(0xc);
                push!(eax);
                push!(0x100);
                push!(0x100);
                push!(0);
                push!(0);
                esp = esp.wrapping_sub(0x10);
                ebp = ebp.wrapping_add(edi);
                xmm0.lanes[0] = (((ebp) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                ebx = ebx.wrapping_add(esi);
                xmm0.lanes[0] = (((ebx) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                xmm0.lanes[0] = (((edi) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0.lanes[0] = (((esi) as i32) as f32).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x1b7b0);
                pc = 0x3094c7; continue;
            }
            0x3094c7 => {
                xmm0.lanes[0] = (((m.read32(0x4e8cf0)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d7c4))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x783f2c));
                esp = esp.wrapping_add(0x24);
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30958d; continue; }
                pc = 0x3094eb; continue;
            }
            0x3094eb => {
                eax = m.read32(esp.wrapping_add(0x34));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30958d; continue; }
                pc = 0x3094f7; continue;
            }
            0x3094f7 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm2.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                push!(0xf0f0f40);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7e7264))).to_bits();
                push!(0x7f7f7f80);
                push!(eax);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475db4))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x41e30);
                pc = 0x309575; continue;
            }
            0x309575 => {
                ecx = m.read32(esp.wrapping_add(0x34));
                esp = esp.wrapping_add(0x14);
                push!(ecx);
                call!(0x3fa10);
                pc = 0x309582; continue;
            }
            0x309582 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x30958d => {
                fa = m.read32(esp.wrapping_add(0x30));
                fb = ((((0x1020407fi32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30967d; continue; }
                pc = 0x30959b; continue;
            }
            0x30959b => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30967d; continue; }
                pc = 0x3095a4; continue;
            }
            0x3095a4 => {
                push!(0x3);
                call!(0x80f20);
                pc = 0x3095ab; continue;
            }
            0x3095ab => {
                xmm2 = Xmm::scalar(m.read32(0x475cd4));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm1 = Xmm::scalar(m.read32(0x783f2c));
                xmm3.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm4.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                esi = eax;
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                push!(esi);
                m.write32(esp.wrapping_add(0x24), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x18), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x1c), xmm1.lanes[0]);
                call!(0x1ed50);
                pc = 0x309639; continue;
            }
            0x309639 => {
                esi = ((esi as i32).wrapping_mul((0x34) as i32)) as u32;
                ecx = m.read32(0x4b85c8);
                edx = m.read32(ecx);
                ecx = ((m.read16(esi.wrapping_add(edx).wrapping_add(0x2a))) as u16 as u32);
                eax = esi.wrapping_add(edx);
                edx = ((m.read16(eax.wrapping_add(0x26))) as u16 as u32);
                eax = m.read32(esp.wrapping_add(0x1c));
                push!(0x5064787f);
                ecx = ecx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x20));
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x2c));
                push!(0);
                push!(0);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x3c));
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x1b7b0);
                pc = 0x30967a; continue;
            }
            0x30967a => {
                esp = esp.wrapping_add(0x2c);
                pc = 0x30967d; continue;
            }
            0x30967d => {
                ecx = m.read32(esp.wrapping_add(0x20));
                push!(ecx);
                call!(0x3fa10);
                pc = 0x309687; continue;
            }
            0x309687 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x30968a; continue;
            }
            0x30968a => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003096A0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3096a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3096a0;
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
        { crate::pose::cover(0x3096a0, previous, pc); previous = pc; }
        match pc {
            0x3096a0 => {
                esp = esp.wrapping_sub(0x14);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = ecx;
                ecx = m.read32(edi.wrapping_add(0xc));
                edx = m.read32(edi.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x2c));
                esi = eax;
                eax = m.read32(0x7844a8);
                xmm0.lanes[0] = (((m.read32(eax.wrapping_add(0x264))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                eax = m.read32(edx.wrapping_add(0x1c));
                ebx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                fa = ebx;
                fb = eax;
                if (fa >= fb) { pc = 0x309a6a; continue; }
                pc = 0x3096d3; continue;
            }
            0x3096d3 => {
                ebp = m.read32(esp.wrapping_add(0x28));
                ecx = (ecx & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x2c))) as u32)) & 0xffff);
                esp = esp.wrapping_sub(0x8);
                eax = esp;
                m.write32(eax, ebp);
                write16(m, eax.wrapping_add(0x4), (((((ecx) as u16)) as u32)) as u16);
                call!(0x331c70);
                pc = 0x3096ec; continue;
            }
            0x3096ec => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x309727; continue; }
                pc = 0x3096f3; continue;
            }
            0x3096f3 => {
                eax = (((((m.read16(esp.wrapping_add(0x2a)) as i16)) as i32)) as u32);
                ecx = (((((m.read16(esp.wrapping_add(0x2c)) as i16)) as i32)) as u32);
                fa = ecx;
                fb = ebx;
                edx = ((((ebp) as u16)) as u32);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi))).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x4)))).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                if (fa != fb) { pc = 0x309a6a; continue; }
                pc = 0x309725; continue;
            }
            0x309725 => {
                pc = 0x3097a6; continue;
            }
            0x309727 => {
                eax = m.read32(0x7ba9e0);
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba280)));
                xmm2 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba284)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd4))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                xmm1 = Xmm::scalar(m.read32(0x475cd4));
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f20))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm2.lanes[0]));
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                pc = 0x3097a6; continue;
            }
            0x3097a6 => {
                push!(0x12);
                call!(0x3fa10);
                pc = 0x3097ad; continue;
            }
            0x3097ad => {
                xmm2 = Xmm::scalar(m.read32(0x475cd4));
                xmm1 = Xmm::scalar(m.read32(0x475d48));
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0 = Xmm::scalar(m.read32(0x783f2c));
                xmm4.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                edi = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x783f28));
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(0x475d40));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                ecx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                push!(esi);
                m.write32(esp.wrapping_add(0x28), eax);
                ebx = sse::cvttss2si(f32::from_bits(xmm3.lanes[0]));
                ebp = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                m.write32(esp.wrapping_add(0x20), ecx);
                call!(0x1ed50);
                pc = 0x309847; continue;
            }
            0x309847 => {
                esi = ((esi as i32).wrapping_mul((0x34) as i32)) as u32;
                edx = m.read32(0x4b85c8);
                eax = m.read32(edx);
                ecx = ((m.read16(eax.wrapping_add(esi).wrapping_add(0x2a))) as u16 as u32);
                edx = ((m.read16(eax.wrapping_add(esi).wrapping_add(0x26))) as u16 as u32);
                eax = eax.wrapping_add(esi);
                esi = m.read32(esp.wrapping_add(0x38));
                eax = m.read32(esp.wrapping_add(0x20));
                esp = esp.wrapping_add(0x8);
                push!(esi);
                ecx = ecx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(ecx);
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(edx);
                push!(0);
                push!(0);
                esp = esp.wrapping_sub(0x10);
                eax = eax.wrapping_add(ebx);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                ebp = ebp.wrapping_add(edi);
                xmm0.lanes[0] = (((ebp) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                xmm0.lanes[0] = (((ebx) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0.lanes[0] = (((edi) as i32) as f32).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x1b7b0);
                pc = 0x3098a9; continue;
            }
            0x3098a9 => {
                xmm0.lanes[0] = (((m.read32(0x4e8cf0)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d7c4))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x783f2c));
                esp = esp.wrapping_add(0x24);
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30996f; continue; }
                pc = 0x3098cd; continue;
            }
            0x3098cd => {
                eax = m.read32(esp.wrapping_add(0x34));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30996f; continue; }
                pc = 0x3098d9; continue;
            }
            0x3098d9 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm2.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                push!(0xf0f0f40);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7e7264))).to_bits();
                push!(0x7f7f7f80);
                push!(eax);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475db4))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x41e30);
                pc = 0x309957; continue;
            }
            0x309957 => {
                eax = m.read32(esp.wrapping_add(0x34));
                esp = esp.wrapping_add(0x14);
                push!(eax);
                call!(0x3fa10);
                pc = 0x309964; continue;
            }
            0x309964 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x30996f => {
                fa = esi;
                fb = ((((0x1020407fi32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x309a5d; continue; }
                pc = 0x30997b; continue;
            }
            0x30997b => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x309a5d; continue; }
                pc = 0x309984; continue;
            }
            0x309984 => {
                push!(0x3);
                call!(0x80f20);
                pc = 0x30998b; continue;
            }
            0x30998b => {
                xmm2 = Xmm::scalar(m.read32(0x475cd4));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                xmm1 = Xmm::scalar(m.read32(0x783f2c));
                xmm3.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm4.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = xmm0;
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                esi = eax;
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                push!(esi);
                m.write32(esp.wrapping_add(0x24), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x1c), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x20), xmm1.lanes[0]);
                call!(0x1ed50);
                pc = 0x309a19; continue;
            }
            0x309a19 => {
                esi = ((esi as i32).wrapping_mul((0x34) as i32)) as u32;
                ecx = m.read32(0x4b85c8);
                edx = m.read32(ecx);
                eax = ((m.read16(esi.wrapping_add(edx).wrapping_add(0x2a))) as u16 as u32);
                ecx = ((m.read16(esi.wrapping_add(edx).wrapping_add(0x26))) as u16 as u32);
                esi = esi.wrapping_add(edx);
                edx = m.read32(esp.wrapping_add(0x20));
                push!(0x5064787f);
                eax = eax.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x24));
                ecx = ecx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x24));
                push!(0);
                push!(0);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x3c));
                push!(eax);
                push!(ecx);
                push!(edx);
                call!(0x1b7b0);
                pc = 0x309a5a; continue;
            }
            0x309a5a => {
                esp = esp.wrapping_add(0x2c);
                pc = 0x309a5d; continue;
            }
            0x309a5d => {
                eax = m.read32(esp.wrapping_add(0x20));
                push!(eax);
                call!(0x3fa10);
                pc = 0x309a67; continue;
            }
            0x309a67 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x309a6a; continue;
            }
            0x309a6a => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00309A80: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f309a80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x309a80;
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
        { crate::pose::cover(0x309a80, previous, pc); previous = pc; }
        match pc {
            0x309a80 => {
                esp = esp.wrapping_sub(0x14);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                push!(0x12);
                call!(0x3fa10);
                pc = 0x309a8e; continue;
            }
            0x309a8e => {
                m.write32(esp.wrapping_add(0x24), eax);
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x280));
                esp = esp.wrapping_add(0x4);
                ebp = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x309d04; continue; }
                pc = 0x309aaa; continue;
            }
            0x309aaa => {
                edi = 0;
                m.write32(esp.wrapping_add(0x14), edi);
                pc = 0x309ab0; continue;
            }
            0x309ab0 => {
                esi = edi.wrapping_add(eax).wrapping_add(0x288);
                push!(ebp);
                m.write32(esp.wrapping_add(0x1c), esi);
                call!(0x31d3e0);
                pc = 0x309ac1; continue;
            }
            0x309ac1 => {
                push!(eax);
                call!(0x315d20);
                pc = 0x309ac7; continue;
            }
            0x309ac7 => {
                m.write32(esp.wrapping_add(0x18), eax);
                eax = m.read32(esi.wrapping_add(0x58));
                esp = esp.wrapping_add(0x8);
                ebx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                if (fas <= fbs) { pc = 0x309be2; continue; }
                pc = 0x309adf; continue;
            }
            0x309adf => {
                pc = 0x309af0; continue;
            }
            0x309ae1 => {
                esi = m.read32(esp.wrapping_add(0x18));
                edi = m.read32(esp.wrapping_add(0x14));
                pc = 0x309af0; continue;
            }
            0x309af0 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(edi.wrapping_add(ecx).wrapping_add(0x2e8));
                edi = m.read32(edx.wrapping_mul(0x4).wrapping_add(0x52ce00));
                eax = ebx;
                eax = eax.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = eax.wrapping_add(esi).wrapping_add(0x64);
                eax = (eax & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x2f10))) as u32)) & 0xffff);
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas < fbs) { pc = 0x309b2b; continue; }
                pc = 0x309b19; continue;
            }
            0x309b19 => {
                edx = ((((eax) as u16)) as u16 as i16 as i32 as u32);
                fa = ebp;
                fb = edx;
                if (fa == fb) { pc = 0x309b2b; continue; }
                pc = 0x309b20; continue;
            }
            0x309b20 => {
                edi = (edi & 0xffffff3fu32);
                edi = (edi | 0x3f);
                pc = 0x309b3d; continue;
            }
            0x309b2b => {
                fa = m.read32(ecx.wrapping_add(0x254));
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x309b3d; continue; }
                pc = 0x309b34; continue;
            }
            0x309b34 => {
                edi = (edi & 0xffffff28u32);
                edi = (edi | 0x28);
                pc = 0x309b3d; continue;
            }
            0x309b3d => {
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas < fbs) { pc = 0x309b4d; continue; }
                pc = 0x309b42; continue;
            }
            0x309b42 => {
                edx = ((((eax) as u16)) as u16 as i16 as i32 as u32);
                fa = edx;
                fb = ebp;
                if (fa != fb) { pc = 0x309c7d; continue; }
                pc = 0x309b4d; continue;
            }
            0x309b4d => {
                eax = ((((eax) as u16)) as u16 as i16 as i32 as u32);
                fa = ebp;
                fb = eax;
                if (fa != fb) { pc = 0x309b62; continue; }
                pc = 0x309b54; continue;
            }
            0x309b54 => {
                ecx = (((((m.read16(ecx.wrapping_add(0x2f12)) as i16)) as i32)) as u32);
                fa = ecx;
                fb = ebx;
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = ebx.wrapping_add(0x2);
                if (fas <= fbs) { pc = 0x309b65; continue; }
                pc = 0x309b62; continue;
            }
            0x309b62 => {
                eax = ebx.wrapping_add(0x1);
                pc = 0x309b65; continue;
            }
            0x309b65 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x309c7d; continue; }
                pc = 0x309b6d; continue;
            }
            0x309b6d => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x309b85; continue; }
                pc = 0x309b71; continue;
            }
            0x309b71 => {
                push!(eax);
                push!(0x4760fc);
                call!(0x151030);
                pc = 0x309b7c; continue;
            }
            0x309b7c => {
                esp = esp.wrapping_add(0x8);
                push!(eax);
                pc = 0x309c87; continue;
            }
            0x309b85 => {
                fa = m.read32(esp.wrapping_add(0x1c));
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x309c61; continue; }
                pc = 0x309b90; continue;
            }
            0x309b90 => {
                push!(eax);
                push!(0x4760fc);
                call!(0x151030);
                pc = 0x309b9b; continue;
            }
            0x309b9b => {
                esp = esp.wrapping_add(0x8);
                push!(eax);
                pc = 0x309b9f; continue;
            }
            0x309b9f => {
                push!(edi);
                eax = esi.wrapping_add(0x4);
                edx = m.read32(eax);
                eax = (eax & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x4))) as u32)) & 0xffff);
                esp = esp.wrapping_sub(0x8);
                ecx = esp;
                m.write32(ecx, edx);
                write16(m, ecx.wrapping_add(0x4), (((((eax) as u16)) as u32)) as u16);
                pc = 0x309bb4; continue;
            }
            0x309bb4 => {
                ecx = m.read32(esi);
                eax = m.read32(esp.wrapping_add(0x20));
                call!(0x3096a0);
                pc = 0x309bbf; continue;
            }
            0x309bbf => {
                eax = m.read32(esp.wrapping_add(0x38));
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x309bde; continue; }
                pc = 0x309bca; continue;
            }
            0x309bca => {
                ecx = m.read32(esp.wrapping_add(0x18));
                eax = m.read32(ecx.wrapping_add(0x58));
                ebx = ebx.wrapping_add(1);
                fa = ebx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                if (fas < fbs) { pc = 0x309ae1; continue; }
                pc = 0x309bde; continue;
            }
            0x309bde => {
                edi = m.read32(esp.wrapping_add(0x14));
                pc = 0x309be2; continue;
            }
            0x309be2 => {
                eax = m.read32(0x7844a8);
                edx = (((((m.read16(eax.wrapping_add(0x2f10)) as i16)) as i32)) as u32);
                fa = ebp;
                fb = edx;
                if (fa != fb) { pc = 0x309ceb; continue; }
                pc = 0x309bf6; continue;
            }
            0x309bf6 => {
                ecx = m.read32(eax.wrapping_add(0x2f18));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x309ceb; continue; }
                pc = 0x309c04; continue;
            }
            0x309c04 => {
                fa = m.read32(eax.wrapping_add(0x254));
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x309ceb; continue; }
                pc = 0x309c11; continue;
            }
            0x309c11 => {
                edx = m.read32(edi.wrapping_add(eax).wrapping_add(0x2e8));
                esi = m.read32(edx.wrapping_mul(0x4).wrapping_add(0x52ce00));
                edx = ((m.read8(0x7ba034)) as u8 as u32);
                esi = (esi & 0xffffff00u32);
                esi = (esi | edx);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x2f12))) as u32)) & 0xffff);
                fa = ((((((edx) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x309ca8; continue; }
                pc = 0x309c3a; continue;
            }
            0x309c3a => {
                push!(0);
                push!(esi);
                eax = eax.wrapping_add(0x2f28);
                esi = m.read32(eax);
                eax = (eax & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x4))) as u32)) & 0xffff);
                esp = esp.wrapping_sub(0x8);
                edx = esp;
                m.write32(edx, esi);
                write16(m, edx.wrapping_add(0x4), (((((eax) as u16)) as u32)) as u16);
                eax = m.read32(esp.wrapping_add(0x20));
                call!(0x3096a0);
                pc = 0x309c5c; continue;
            }
            0x309c5c => {
                pc = 0x309ce3; continue;
            }
            0x309c61 => {
                push!(0);
                push!(edi);
                ecx = esi.wrapping_add(0x4);
                eax = m.read32(ecx);
                ecx = (ecx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x4))) as u32)) & 0xffff);
                esp = esp.wrapping_sub(0x8);
                edx = esp;
                m.write32(edx, eax);
                write16(m, edx.wrapping_add(0x4), (((((ecx) as u16)) as u32)) as u16);
                pc = 0x309bb4; continue;
            }
            0x309c7d => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(0);
                if (fa == fb) { pc = 0x309b9f; continue; }
                pc = 0x309c87; continue;
            }
            0x309c87 => {
                push!(edi);
                edx = esi.wrapping_add(0x4);
                ecx = m.read32(edx);
                edx = (edx & 0xffff_0000) | ((((m.read16(edx.wrapping_add(0x4))) as u32)) & 0xffff);
                esp = esp.wrapping_sub(0x8);
                eax = esp;
                m.write32(eax, ecx);
                write16(m, eax.wrapping_add(0x4), (((((edx) as u16)) as u32)) as u16);
                eax = m.read32(esi);
                call!(0x3092e0);
                pc = 0x309ca3; continue;
            }
            0x309ca3 => {
                pc = 0x309bbf; continue;
            }
            0x309ca8 => {
                ecx = ((((edx) as u16)) as u16 as i16 as i32 as u32);
                ecx = ecx.wrapping_add(1);
                push!(ecx);
                push!(0x4760fc);
                call!(0x151030);
                pc = 0x309cb7; continue;
            }
            0x309cb7 => {
                esp = esp.wrapping_add(0x8);
                push!(eax);
                eax = m.read32(0x7844a8);
                push!(esi);
                edx = eax.wrapping_add(0x2f28);
                esi = m.read32(edx);
                edx = (edx & 0xffff_0000) | ((((m.read16(edx.wrapping_add(0x4))) as u32)) & 0xffff);
                eax = m.read32(eax.wrapping_add(0x2f18));
                esp = esp.wrapping_sub(0x8);
                ecx = esp;
                m.write32(ecx, esi);
                write16(m, ecx.wrapping_add(0x4), (((((edx) as u16)) as u32)) as u16);
                call!(0x3092e0);
                pc = 0x309ce3; continue;
            }
            0x309ce3 => {
                eax = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x10);
                pc = 0x309ceb; continue;
            }
            0x309ceb => {
                ecx = m.read32(eax.wrapping_add(0x280));
                ebp = ebp.wrapping_add(1);
                edi = edi.wrapping_add(0xe4);
                fa = ebp;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x14), edi);
                if (fas < fbs) { pc = 0x309ab0; continue; }
                pc = 0x309d04; continue;
            }
            0x309d04 => {
                eax = m.read32(esp.wrapping_add(0x20));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                m.write32(esp.wrapping_add(0x4), eax);
                tail_to!(0x3fa10); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0030AEA0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30aea0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut fp_cmp = 0i32;
    let mut pc: u32 = 0x30aea0;
    let mut jt = 0u32;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x30aea0, previous, pc); previous = pc; }
        match pc {
            0x30aea0 => {
                esp = esp.wrapping_sub(0x54);
                push!(ebp);
                push!(esi);
                ebp = 0;
                push!(edi);
                esi = 0;
                eax = 0x4c847c;
                pc = 0x30aeb0; continue;
            }
            0x30aeb0 => {
                edi = m.read32(esp.wrapping_add(0x64));
                fa = m.read32(eax);
                fb = edi;
                if (fa == fb) { pc = 0x30aeca; continue; }
                pc = 0x30aeb8; continue;
            }
            0x30aeb8 => {
                eax = eax.wrapping_add(0xc);
                esi = esi.wrapping_add(1);
                fa = eax;
                fb = ((((0x4c84aci32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x30aeb0; continue; }
                pc = 0x30aec3; continue;
            }
            0x30aec3 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x54);
                break 'run;
            }
            0x30aeca => {
                edx = m.read32(ecx.wrapping_add(0x128));
                eax = ecx.wrapping_add(0x128);
                push!(ebx);
                ebx = 0;
                fa = edx;
                fb = esi;
                m.write32(esp.wrapping_add(0x10), ebp);
                if (fa != fb) { pc = 0x30aef5; continue; }
                pc = 0x30aee1; continue;
            }
            0x30aee1 => {
                fa = eax;
                fb = ebp;
                edx = m.read32(ecx.wrapping_add(0x12c));
                m.write32(esp.wrapping_add(0x10), edx);
                if (fa == fb) { pc = 0x30aef5; continue; }
                pc = 0x30aeef; continue;
            }
            0x30aeef => {
                fa = m.read32(eax.wrapping_add(0x8));
                fb = ebp;
                ebx = (ebx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                pc = 0x30aef5; continue;
            }
            0x30aef5 => {
                edx = m.read32(ecx.wrapping_add(0x138));
                fa = edx;
                fb = esi;
                eax = ecx.wrapping_add(0x138);
                if (fa != fb) { pc = 0x30af1b; continue; }
                pc = 0x30af05; continue;
            }
            0x30af05 => {
                edx = m.read32(ecx.wrapping_add(0x13c));
                ebx = 0;
                fa = eax;
                fb = ebp;
                m.write32(esp.wrapping_add(0x10), edx);
                if (fa == fb) { pc = 0x30af1b; continue; }
                pc = 0x30af15; continue;
            }
            0x30af15 => {
                fa = m.read32(eax.wrapping_add(0x8));
                fb = ebp;
                ebx = (ebx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                pc = 0x30af1b; continue;
            }
            0x30af1b => {
                edx = m.read32(ecx.wrapping_add(0x148));
                fa = edx;
                fb = esi;
                eax = ecx.wrapping_add(0x148);
                if (fa != fb) { pc = 0x30af41; continue; }
                pc = 0x30af2b; continue;
            }
            0x30af2b => {
                edx = m.read32(ecx.wrapping_add(0x14c));
                ebx = 0;
                fa = eax;
                fb = ebp;
                m.write32(esp.wrapping_add(0x10), edx);
                if (fa == fb) { pc = 0x30af41; continue; }
                pc = 0x30af3b; continue;
            }
            0x30af3b => {
                fa = m.read32(eax.wrapping_add(0x8));
                fb = ebp;
                ebx = (ebx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                pc = 0x30af41; continue;
            }
            0x30af41 => {
                edx = m.read32(ecx.wrapping_add(0x158));
                fa = edx;
                fb = esi;
                eax = ecx.wrapping_add(0x158);
                if (fa != fb) { pc = 0x30af67; continue; }
                pc = 0x30af51; continue;
            }
            0x30af51 => {
                ecx = m.read32(ecx.wrapping_add(0x15c));
                ebx = 0;
                fa = eax;
                fb = ebp;
                m.write32(esp.wrapping_add(0x10), ecx);
                if (fa == fb) { pc = 0x30af67; continue; }
                pc = 0x30af61; continue;
            }
            0x30af61 => {
                fa = m.read32(eax.wrapping_add(0x8));
                fb = ebp;
                ebx = (ebx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                pc = 0x30af67; continue;
            }
            0x30af67 => {
                fa = m.read32(esp.wrapping_add(0x10));
                fb = ebp;
                if (fa == fb) { pc = 0x30af78; continue; }
                pc = 0x30af6d; continue;
            }
            0x30af6d => {
                fa = ebx;
                fb = ebp;
                if (fa == fb) { pc = 0x30af78; continue; }
                pc = 0x30af71; continue;
            }
            0x30af71 => {
                eax = 0x1;
                pc = 0x30af7a; continue;
            }
            0x30af78 => {
                eax = 0;
                pc = 0x30af7a; continue;
            }
            0x30af7a => {
                edx = m.read32(esp.wrapping_add(0x6c));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x74));
                push!(edi);
                push!(edx);
                push!(eax);
                call!(0x81980);
                pc = 0x30af8b; continue;
            }
            0x30af8b => {
                eax = m.read32(esp.wrapping_add(0x20));
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ebp;
                if (fa == fb) { pc = 0x30b130; continue; }
                pc = 0x30af9a; continue;
            }
            0x30af9a => {
                fa = ebx;
                fb = ebp;
                if (fa == fb) { pc = 0x30b130; continue; }
                pc = 0x30afa2; continue;
            }
            0x30afa2 => {
                ecx = m.read32(esp.wrapping_add(0x74));
                edx = m.read32(0x7844a8);
                eax = m.read32(0x7e7260);
                m.write32(esp.wrapping_add(0x18), ecx);
                esi = m.read32(edx.wrapping_add(0x228));
                push!(0x12);
                m.write32(esp.wrapping_add(0x24), eax);
                call!(0x3fa10);
                pc = 0x30afc6; continue;
            }
            0x30afc6 => {
                push!(ebp);
                m.write32(esp.wrapping_add(0x24), eax);
                call!(0x3fa80);
                pc = 0x30afd0; continue;
            }
            0x30afd0 => {
                esp = esp.wrapping_add(0x8);
                fa = edi;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                ebx = eax;
                m.write32(esp.wrapping_add(0x14), ebp);
                if (fa > fb) { pc = 0x30b027; continue; }
                pc = 0x30afde; continue;
            }
            0x30afde => {
                jt = m.read32(edi.wrapping_mul(0x4).wrapping_add(0x30b138));
                if (jt == 0x30afe5u32) { pc = 0x30afe5; continue; }
                if (jt == 0x30b007u32) { pc = 0x30b007; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x30afe5 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d208))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba180))).to_bits();
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                m.write32(esp.wrapping_add(0x14), ecx);
                pc = 0x30b027; continue;
            }
            0x30b007 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4788fc))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba180))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                m.write32(esp.wrapping_add(0x14), edx);
                pc = 0x30b027; continue;
            }
            0x30b027 => {
                fpu.push((m.read32(esp.wrapping_add(0x14)) as i32 as f64));
                eax = 0;
                ecx = 0x10;
                edi = esp.wrapping_add(0x24);
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                m.write32(esp.wrapping_add(0x14), (fpu.st(0) as f32).to_bits()); fpu.pop();
                edi = m.read32(esp.wrapping_add(0x14));
                ecx = m.read32(esp.wrapping_add(0x10));
                eax = esp.wrapping_add(0x24);
                push!(eax);
                push!(edi);
                push!(ecx);
                m.write32(0x7e7260, 0x1);
                call!(0x40c10);
                pc = 0x30b05a; continue;
            }
            0x30b05a => {
                edx = esp.wrapping_add(0x30);
                push!(edx);
                call!(0x405c0);
                pc = 0x30b064; continue;
            }
            0x30b064 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x24))) as f64));
                esp = esp.wrapping_add(0x10);
                { let a = fpu.top as usize; let b = ((fpu.top + 1) & 7) as usize; fpu.stack.swap(a, b); }
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if fp_cmp != 1 { pc = 0x0030b088; continue; }
                pc = 0x30b073; continue;
            }
            0x30b073 => {
                push!(0);
                eax = esp.wrapping_add(0x28);
                push!(edi);
                push!(eax);
                call!(0x40ad0);
                pc = 0x30b080; continue;
            }
            0x30b080 => {
                esp = esp.wrapping_add(0xc);
                ebp = 0x1;
                pc = 0x30b088; continue;
            }
            0x30b088 => {
                ecx = esp.wrapping_add(0x24);
                push!(ecx);
                call!(0x40620);
                pc = 0x30b092; continue;
            }
            0x30b092 => {
                fpu.pop();
                push!(ebx);
                call!(0x3fa80);
                pc = 0x30b09a; continue;
            }
            0x30b09a => {
                edx = m.read32(esp.wrapping_add(0x70));
                eax = edx.wrapping_add(esi.wrapping_mul(0x8));
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x52cf08));
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30b0cd; continue; }
                pc = 0x30b0af; continue;
            }
            0x30b0af => {
                ecx = m.read32(esp.wrapping_add(0x78));
                edx = m.read32(esp.wrapping_add(0x70));
                push!(ecx);
                push!(edx);
                ecx = esp.wrapping_add(0x2c);
                push!(ecx);
                edx = esp.wrapping_add(0x78);
                push!(edx);
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                call_to!(eax);
                pc = 0x30b0ca; continue;
            }
            0x30b0ca => {
                esp = esp.wrapping_add(0x14);
                pc = 0x30b0cd; continue;
            }
            0x30b0cd => {
                xmm0.lanes[0] = (((m.read32(esp.wrapping_add(0x6c))) as i32) as f32).to_bits();
                push!(0x80);
                push!(0x64646480);
                edx = esp.wrapping_add(0x2c);
                push!(edx);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(esp.wrapping_add(0x2c))) as i32) as f32).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x41e30);
                pc = 0x30b0fb; continue;
            }
            0x30b0fb => {
                esp = esp.wrapping_add(0x14);
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30b119; continue; }
                pc = 0x30b102; continue;
            }
            0x30b102 => {
                push!(0x3f800000);
                push!(0x3f800000);
                push!(0x3f800000);
                call!(0x3f9c0);
                pc = 0x30b116; continue;
            }
            0x30b116 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x30b119; continue;
            }
            0x30b119 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                push!(eax);
                call!(0x3fa10);
                pc = 0x30b123; continue;
            }
            0x30b123 => {
                ecx = m.read32(esp.wrapping_add(0x24));
                esp = esp.wrapping_add(0x4);
                m.write32(0x7e7260, ecx);
                pc = 0x30b130; continue;
            }
            0x30b130 => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x54);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030B150: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30b150<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30b150;
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
        { crate::pose::cover(0x30b150, previous, pc); previous = pc; }
        match pc {
            0x30b150 => {
                esp = esp.wrapping_sub(0x10);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x28));
                push!(esi);
                push!(0x3fc);
                call!(0x80f20);
                pc = 0x30b163; continue;
            }
            0x30b163 => {
                esi = eax;
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x30b26d; continue; }
                pc = 0x30b17c; continue;
            }
            0x30b17c => {
                push!(edi);
                push!(esi);
                call!(0x1ed50);
                pc = 0x30b183; continue;
            }
            0x30b183 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                ecx = esp.wrapping_add(0x10);
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x30)));
                push!(ecx);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                call!(0x711f0);
                pc = 0x30b1bd; continue;
            }
            0x30b1bd => {
                edi = eax;
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x8);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30b1d9; continue; }
                pc = 0x30b1d1; continue;
            }
            0x30b1d1 => {
                ecx = (ecx | 0x200);
                pc = 0x30b1df; continue;
            }
            0x30b1d9 => {
                ecx = (ecx & 0xfffffdffu32);
                pc = 0x30b1df; continue;
            }
            0x30b1df => {
                m.write32(eax.wrapping_add(0x238), ecx);
                esi = ((esi as i32).wrapping_mul((0x34) as i32)) as u32;
                edx = m.read32(0x4b85c8);
                eax = m.read32(edx);
                edx = ((m.read16(eax.wrapping_add(esi).wrapping_add(0x2a))) as u16 as u32);
                eax = eax.wrapping_add(esi);
                eax = ((m.read16(eax.wrapping_add(0x26))) as u16 as u32);
                ecx = edi;
                cf = (((((ecx) != 0))) as i32);
                ecx = (((((ecx) as i32).wrapping_neg())) as u32);
                ecx = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                ecx = (ecx & 0x1b1b1b00);
                ecx = ecx.wrapping_add(0x6464647f);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x30));
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x30));
                eax = eax.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x30));
                push!(0);
                push!(0);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x38));
                push!(edx);
                push!(eax);
                push!(ecx);
                call!(0x1b7b0);
                pc = 0x30b233; continue;
            }
            0x30b233 => {
                esp = esp.wrapping_add(0x24);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x30b26d; continue; }
                pc = 0x30b23b; continue;
            }
            0x30b23b => {
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30b241; continue; }
                pc = 0x30b23f; continue;
            }
            0x30b23f => {
                call_to!(ebx);
                pc = 0x30b241; continue;
            }
            0x30b241 => {
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30b26d; continue; }
                pc = 0x30b245; continue;
            }
            0x30b245 => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x30b25d; continue;
            }
            0x30b25d => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0xc4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30b26d; continue; }
                pc = 0x30b26b; continue;
            }
            0x30b26b => {
                call_to!(ebp);
                pc = 0x30b26d; continue;
            }
            0x30b26d => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030B280: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30b280<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30b280;
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
        { crate::pose::cover(0x30b280, previous, pc); previous = pc; }
        match pc {
            0x30b280 => {
                esp = esp.wrapping_sub(0x10);
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d604))).to_bits();
                push!(ebx);
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x49f798))).to_bits();
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x20));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x28));
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0 = Xmm::scalar(m.read32(0x475d10));
                push!(edi);
                push!(0x315870);
                esp = esp.wrapping_sub(0x10);
                xmm1.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = (((ebp) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm1.lanes[0]);
                edx = esi.wrapping_add(eax.wrapping_mul(0x2));
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x24), edx);
                edx = ecx.wrapping_add(ebp);
                xmm0.lanes[0] = (((esi) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                ebx = ebp.wrapping_add(ecx.wrapping_mul(0x2));
                m.write32(esp.wrapping_add(0x8), xmm1.lanes[0]);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                edi = eax.wrapping_add(esi);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x30), ebx);
                xmm0.lanes[0] = (((edi) as i32) as f32).to_bits();
                ebx = 0x3157c0;
                m.write32(esp.wrapping_add(0x40), edx);
                m.write32(esp, xmm0.lanes[0]);
                call!(0x30b150);
                pc = 0x30b329; continue;
            }
            0x30b329 => {
                ebx = m.read32(esp.wrapping_add(0x48));
                eax = m.read32(esp.wrapping_add(0x44));
                push!(ebx);
                push!(eax);
                push!(edi);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x48));
                push!(0x2);
                ecx = ebp;
                call!(0x30aea0);
                pc = 0x30b342; continue;
            }
            0x30b342 => {
                ecx = m.read32(esp.wrapping_add(0x58));
                edx = m.read32(esp.wrapping_add(0x38));
                eax = m.read32(esp.wrapping_add(0x54));
                push!(ebx);
                push!(ecx);
                push!(edx);
                push!(eax);
                push!(0);
                ecx = ebp;
                call!(0x30aea0);
                pc = 0x30b35b; continue;
            }
            0x30b35b => {
                ecx = m.read32(esp.wrapping_add(0x6c));
                edx = m.read32(esp.wrapping_add(0x68));
                push!(ebx);
                push!(ecx);
                push!(esi);
                push!(edx);
                push!(0x3);
                ecx = ebp;
                call!(0x30aea0);
                pc = 0x30b370; continue;
            }
            0x30b370 => {
                eax = m.read32(esp.wrapping_add(0x80));
                ecx = m.read32(esp.wrapping_add(0x6c));
                esp = esp.wrapping_add(0x50);
                push!(ebx);
                push!(eax);
                push!(edi);
                push!(ecx);
                push!(0x1);
                ecx = ebp;
                call!(0x30aea0);
                pc = 0x30b38b; continue;
            }
            0x30b38b => {
                esp = esp.wrapping_add(0x14);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030B3A0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30b3a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30b3a0;
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
        { crate::pose::cover(0x30b3a0, previous, pc); previous = pc; }
        match pc {
            0x30b3a0 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm1.lanes[0] = (((m.read32(esp.wrapping_add(0x8))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47dfe0))).to_bits();
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x10));
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x20));
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x20));
                push!(esi);
                push!(edi);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(ebx);
                push!(eax);
                push!(0x2);
                ecx = ebp;
                m.write32(esp.wrapping_add(0x2c), eax);
                call!(0x30aea0);
                pc = 0x30b3e3; continue;
            }
            0x30b3e3 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm1.lanes[0] = (((m.read32(esp.wrapping_add(0x2c))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47dfe0))).to_bits();
                push!(esi);
                push!(edi);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(ebx);
                push!(eax);
                push!(0);
                ecx = ebp;
                m.write32(esp.wrapping_add(0x40), eax);
                call!(0x30aea0);
                pc = 0x30b412; continue;
            }
            0x30b412 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm1.lanes[0] = (((m.read32(esp.wrapping_add(0x40))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47dfe0))).to_bits();
                push!(esi);
                push!(edi);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(ebx);
                push!(eax);
                push!(0x3);
                ecx = ebp;
                m.write32(esp.wrapping_add(0x54), eax);
                call!(0x30aea0);
                pc = 0x30b441; continue;
            }
            0x30b441 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47dfe0))).to_bits();
                xmm1.lanes[0] = (((m.read32(esp.wrapping_add(0x54))) as i32) as f32).to_bits();
                push!(esi);
                push!(edi);
                push!(ebx);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                push!(0x1);
                ecx = ebp;
                call!(0x30aea0);
                pc = 0x30b46c; continue;
            }
            0x30b46c => {
                esp = esp.wrapping_add(0x50);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030B480: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30b480<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30b480;
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
        { crate::pose::cover(0x30b480, previous, pc); previous = pc; }
        match pc {
            0x30b480 => {
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                push!(0x12);
                esi = 0x2;
                call!(0x3fa10);
                pc = 0x30b490; continue;
            }
            0x30b490 => {
                esp = esp.wrapping_add(0x4);
                ebp = eax;
                call!(0x774b0);
                pc = 0x30b49a; continue;
            }
            0x30b49a => {
                ecx = m.read32(eax.wrapping_add(0x40));
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                ecx = m.read32(0x7844a8);
                if ((fa & fb) != 0) { pc = 0x30b4bc; continue; }
                pc = 0x30b4a8; continue;
            }
            0x30b4a8 => {
                fa = ((((m.read8(eax.wrapping_add(0x218)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x30b4bc; continue; }
                pc = 0x30b4b1; continue;
            }
            0x30b4b1 => {
                edx = m.read32(ecx.wrapping_add(0x238));
                fa = (((((((edx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30b4be; continue; }
                pc = 0x30b4bc; continue;
            }
            0x30b4bc => {
                esi = 0;
                pc = 0x30b4be; continue;
            }
            0x30b4be => {
                ecx = m.read32(ecx.wrapping_add(0x228));
                xmm1 = Xmm::scalar(m.read32(0x7ba180));
                ecx = ecx.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0.lanes[0] = (((m.read32(ecx.wrapping_add(0x52cef8))) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba188))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                ebx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                if (fa != fb) { pc = 0x30b50d; continue; }
                pc = 0x30b4f7; continue;
            }
            0x30b4f7 => {
                xmm2.lanes[0] = (((m.read32(ecx.wrapping_add(0x52cefc))) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x49f79c))).to_bits();
                pc = 0x30b521; continue;
            }
            0x30b50d => {
                xmm2.lanes[0] = (((m.read32(ecx.wrapping_add(0x52cf00))) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x49f4a0))).to_bits();
                pc = 0x30b521; continue;
            }
            0x30b521 => {
                fa = ((((m.read8(eax.wrapping_add(0x218)))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm2.lanes[0]));
                edi = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                if ((fa & fb) != 0) { pc = 0x30b54c; continue; }
                pc = 0x30b53a; continue;
            }
            0x30b53a => {
                eax = m.read32(esp.wrapping_add(0x14));
                push!(esi);
                push!(edi);
                push!(edx);
                push!(ebx);
                push!(eax);
                call_to!(m.read32(ecx.wrapping_add(0x52cf04)));
                pc = 0x30b549; continue;
            }
            0x30b549 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x30b54c; continue;
            }
            0x30b54c => {
                push!(ebp);
                call!(0x3fa10);
                pc = 0x30b552; continue;
            }
            0x30b552 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030B560: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30b560<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30b560;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x30b560, previous, pc); previous = pc; }
        match pc {
            0x30b560 => {
                esp = esp.wrapping_sub(0xc);
                eax = m.read32(0x7ba9e0);
                xmm4.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba284)));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba280)));
                xh[0] = Xmm::scalar(m.read32(0x783cc0));
                eax = m.read32(0x7844a8);
                xmm2.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm3 = Xmm::scalar(m.read32(0x783dd8));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f20))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp, xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x2f98)));
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783f28))).to_bits();
                push!(esi);
                esi = ((m.read8(0x7ba034)) as u8 as u32);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0xc), xmm1.lanes[0]);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                ecx = m.read32(esp.wrapping_add(0xc));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd4))).to_bits();
                esi = (esi | 0x7f7f0000);
                push!(esi);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                push!(ecx);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x783f2c));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                push!(ecx);
                m.write32(esp, xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x2f94)));
                push!(ecx);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                push!(ecx);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp, xmm1.lanes[0]);
                call!(0x1afa0);
                pc = 0x30b63d; continue;
            }
            0x30b63d => {
                eax = m.read32(0x7844a8);
                fpu.push((f32::from_bits(m.read32(eax.wrapping_add(0x2f94))) as f64));
                xmm1 = Xmm::scalar(m.read32(0x783cc0));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x783dd8)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm2 = Xmm::scalar(m.read32(0x783f28));
                xmm0.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x783f2c)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                esp = esp.wrapping_add(0x14);
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                { let value = round_pc(fpu.st(0) + (m.read32(0x783ee0) as i32 as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(esi);
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                push!(ecx);
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp, xmm3.lanes[0]);
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0x2f98)));
                m.write32(esp.wrapping_add(0x14), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                ecx = m.read32(esp.wrapping_add(0x14));
                push!(ecx);
                push!(ecx);
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp, xmm3.lanes[0]);
                push!(ecx);
                call!(0x1afa0);
                pc = 0x30b6b7; continue;
            }
            0x30b6b7 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x1c))) as f64));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x783dd8)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm1.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                esp = esp.wrapping_add(0x14);
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x783f2c)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) + (m.read32(0x783ee0) as i32 as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm2 = Xmm::scalar(m.read32(0x783cc0));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4)));
                xmm3 = Xmm::scalar(m.read32(0x783f28));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                push!(esi);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                push!(ecx);
                ecx = m.read32(0x7844a8);
                m.write32(esp.wrapping_add(0x14), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = m.read32(esp.wrapping_add(0x14));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x2f98)));
                push!(eax);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                push!(ecx);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                call!(0x1afa0);
                pc = 0x30b730; continue;
            }
            0x30b730 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x18))) as f64));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x783cc0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm2 = Xmm::scalar(m.read32(0x783dd8));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xmm3 = Xmm::scalar(m.read32(0x783f2c));
                xmm1.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                edx = m.read32(0x7844a8);
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x783f28)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                esp = esp.wrapping_add(0x14);
                push!(esi);
                { let value = round_pc(fpu.st(0) + (m.read32(0x783dc4) as i32 as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = m.read32(esp.wrapping_add(0x10));
                push!(eax);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(0x2f94)));
                push!(eax);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                push!(ecx);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x1afa0);
                pc = 0x30b7a9; continue;
            }
            0x30b7a9 => {
                esp = esp.wrapping_add(0x14);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030B7C0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30b7c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30b7c0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x30b7c0, previous, pc); previous = pc; }
        match pc {
            0x30b7c0 => {
                eax = esp.wrapping_add(((0x40i32.wrapping_neg()) as u32));
                esp = esp.wrapping_sub(0x70);
                push!(eax);
                call!(0x153240);
                pc = 0x30b7cd; continue;
            }
            0x30b7cd => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x88)));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x80)));
                esp = esp.wrapping_add(0x4);
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30b800; continue; }
                pc = 0x30b7e7; continue;
            }
            0x30b7e7 => {
                m.write32(esp.wrapping_add(0x24), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0xc), xmm1.lanes[0]);
                m.write32(esp, xmm0.lanes[0]);
                pc = 0x30b817; continue;
            }
            0x30b800 => {
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x18), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                m.write32(esp, xmm1.lanes[0]);
                pc = 0x30b817; continue;
            }
            0x30b817 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x80)));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x88)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30b848; continue; }
                pc = 0x30b82e; continue;
            }
            0x30b82e => {
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x20), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x8), xmm1.lanes[0]);
                pc = 0x30b860; continue;
            }
            0x30b848 => {
                m.write32(esp.wrapping_add(0x2c), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x20), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                pc = 0x30b860; continue;
            }
            0x30b860 => {
                ecx = m.read32(esp.wrapping_add(0x8c));
                xmm0 = Xmm::default();
                push!(ecx);
                edx = esp.wrapping_add(0x34);
                push!(edx);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                call!(0x153440);
                pc = 0x30b88d; continue;
            }
            0x30b88d => {
                eax = esp.wrapping_add(0x2c);
                push!(eax);
                ecx = esp.wrapping_add(0x3c);
                push!(ecx);
                call!(0x151e80);
                pc = 0x30b89c; continue;
            }
            0x30b89c => {
                edx = esp.wrapping_add(0x28);
                push!(edx);
                eax = esp.wrapping_add(0x44);
                push!(eax);
                call!(0x151e80);
                pc = 0x30b8ab; continue;
            }
            0x30b8ab => {
                ecx = esp.wrapping_add(0x24);
                push!(ecx);
                edx = esp.wrapping_add(0x4c);
                push!(edx);
                call!(0x151e80);
                pc = 0x30b8ba; continue;
            }
            0x30b8ba => {
                eax = esp.wrapping_add(0x20);
                push!(eax);
                ecx = esp.wrapping_add(0x54);
                push!(ecx);
                call!(0x151e80);
                pc = 0x30b8c9; continue;
            }
            0x30b8c9 => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x34))) as f64));
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x9c))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x54)));
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0xa0)));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x9c)));
                m.write32(esp.wrapping_add(0x34), (fpu.st(0) as f32).to_bits()); fpu.pop();
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4c)));
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x3c))) as f64));
                push!(esi);
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0xa4))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(esi);
                push!(esi);
                push!(esi);
                m.write32(esp.wrapping_add(0x4c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ecx = m.read32(esp.wrapping_add(0x4c));
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x38))) as f64));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0xac))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x64), xmm3.lanes[0]);
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x38), (fpu.st(0) as f32).to_bits()); fpu.pop();
                eax = m.read32(esp.wrapping_add(0x38));
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x40))) as f64));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0xb0))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x58)));
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x58), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x40), (fpu.st(0) as f32).to_bits()); fpu.pop();
                edx = m.read32(esp.wrapping_add(0x40));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x48));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x60));
                push!(ecx);
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x74));
                push!(eax);
                m.write32(esp.wrapping_add(0x64), xmm3.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x64));
                push!(ecx);
                m.write32(esp.wrapping_add(0x74), xmm0.lanes[0]);
                xmm0 = Xmm::default();
                eax = m.read32(esp.wrapping_add(0x74));
                push!(edx);
                m.write32(esp.wrapping_add(0x7c), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x70), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x64), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x58), xmm0.lanes[0]);
                push!(eax);
                call!(0x308c00);
                pc = 0x30b9a3; continue;
            }
            0x30b9a3 => {
                esp = esp.wrapping_add(0x58);
                esp = esp.wrapping_add(0x70);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030B9B0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30b9b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30b9b0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x30b9b0, previous, pc); previous = pc; }
        match pc {
            0x30b9b0 => {
                esp = esp.wrapping_sub(0x8);
                fa = eax;
                fb = ((((0xci32)) as u32) & 0xffffffffu32);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                esi = ecx;
                if (fa == fb) { pc = 0x30b9cb; continue; }
                pc = 0x30b9be; continue;
            }
            0x30b9be => {
                fa = eax;
                fb = ((((0xei32)) as u32) & 0xffffffffu32);
                xmm0 = Xmm::scalar(m.read32(0x49f7a0));
                if (fa != fb) { pc = 0x30b9d3; continue; }
                pc = 0x30b9cb; continue;
            }
            0x30b9cb => {
                xmm0 = Xmm::scalar(m.read32(0x475d2c));
                pc = 0x30b9d3; continue;
            }
            0x30b9d3 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                ecx = (((((m.read16(esp.wrapping_add(0x1c)) as i16)) as i32)) as u32);
                eax = (((((eax as i32)).wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x14), eax);
                edi = m.read32(esp.wrapping_add(0x10));
                fpu.push((m.read32(esp.wrapping_add(0x14)) as i32 as f64));
                push!(edi);
                push!(0xbd800000u32);
                m.write32(esp.wrapping_add(0x24), ecx);
                m.write32(esp.wrapping_add(0x1c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ebx = m.read32(esp.wrapping_add(0x1c));
                fpu.push((m.read32(esp.wrapping_add(0x24)) as i32 as f64));
                push!(0x3facfdf4);
                push!(0xbe000000u32);
                push!(0);
                m.write32(esp.wrapping_add(0x30), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ebp = m.read32(esp.wrapping_add(0x30));
                push!(ebx);
                push!(ebp);
                call!(0x30b7c0);
                pc = 0x30ba22; continue;
            }
            0x30ba22 => {
                push!(edi);
                push!(0x3d800000);
                push!(0x3fb4fdf4);
                push!(0xbe000000u32);
                push!(0x3facfdf4);
                push!(ebx);
                push!(ebp);
                call!(0x30b7c0);
                pc = 0x30ba3e; continue;
            }
            0x30ba3e => {
                push!(edi);
                push!(0x3e000000);
                push!(0x3fb4fdf4);
                push!(0x3d800000);
                push!(0x3d800000);
                push!(ebx);
                push!(ebp);
                call!(0x30b7c0);
                pc = 0x30ba5a; continue;
            }
            0x30ba5a => {
                esp = esp.wrapping_add(0x54);
                push!(edi);
                push!(0x3e000000);
                push!(0x3d800000);
                push!(0xbd800000u32);
                push!(0);
                push!(ebx);
                push!(ebp);
                call!(0x30b7c0);
                pc = 0x30ba76; continue;
            }
            0x30ba76 => {
                esp = esp.wrapping_add(0x1c);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
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

/// 0x0030BA90: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30ba90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30ba90;
    let mut jt = 0u32;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x30ba90, previous, pc); previous = pc; }
        match pc {
            0x30ba90 => {
                esp = esp.wrapping_sub(0x18);
                push!(ebx);
                ecx = eax.wrapping_add(((0x1i32.wrapping_neg()) as u32));
                fa = ecx;
                fb = ((((0xdi32)) as u32) & 0xffffffffu32);
                push!(ebp);
                push!(edi);
                if (fa > fb) { pc = 0x30bb59; continue; }
                pc = 0x30baa2; continue;
            }
            0x30baa2 => {
                ecx = ((m.read8(ecx.wrapping_add(0x30bc40))) as u8 as u32);
                jt = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x30bc30));
                if (jt == 0x30bab0u32) { pc = 0x30bab0; continue; }
                if (jt == 0x30baf2u32) { pc = 0x30baf2; continue; }
                if (jt == 0x30bb34u32) { pc = 0x30bb34; continue; }
                if (jt == 0x30bb59u32) { pc = 0x30bb59; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x30bab0 => {
                edx = (((((m.read16(esp.wrapping_add(0x28)) as i16)) as i32)) as u32);
                eax = m.read32(esp.wrapping_add(0x28));
                xmm2 = Xmm::scalar(m.read32(0x475dc4));
                eax = (((((eax as i32)).wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(0x475d50));
                m.write32(esp.wrapping_add(0xc), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(0x475d3c));
                m.write32(esp.wrapping_add(0x14), xmm2.lanes[0]);
                pc = 0x30bb6b; continue;
            }
            0x30baf2 => {
                ecx = (((((m.read16(esp.wrapping_add(0x28)) as i16)) as i32)) as u32);
                edx = m.read32(esp.wrapping_add(0x28));
                xmm2 = Xmm::scalar(m.read32(0x475dc4));
                edx = (((((edx as i32)).wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm1.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = Xmm::scalar(m.read32(0x475d3c));
                m.write32(esp.wrapping_add(0xc), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(0x475d50));
                m.write32(esp.wrapping_add(0x14), xmm2.lanes[0]);
                pc = 0x30bb6b; continue;
            }
            0x30bb34 => {
                edx = m.read32(esp.wrapping_add(0x28));
                esp = esp.wrapping_sub(0x8);
                ecx = esp;
                m.write32(ecx, edx);
                edx = (edx & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x34))) as u32)) & 0xffff);
                write16(m, ecx.wrapping_add(0x4), (((((edx) as u16)) as u32)) as u16);
                ecx = esi;
                call!(0x30b9b0);
                pc = 0x30bb4f; continue;
            }
            0x30bb4f => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x18);
                break 'run;
            }
            0x30bb59 => {
                xmm0 = Xmm::default();
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                xmm1 = xmm0;
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                pc = 0x30bb6b; continue;
            }
            0x30bb6b => {
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0xc))) as f64));
                edi = m.read32(esp.wrapping_add(0xc));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475dc4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                push!(esi);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x24), (fpu.st(0) as f32).to_bits()); fpu.pop();
                m.write32(esp.wrapping_add(0x1c), xmm1.lanes[0]);
                fpu.push((f32::from_bits(m.read32(esp.wrapping_add(0x14))) as f64));
                ebx = m.read32(esp.wrapping_add(0x1c));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475dc4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                ebp = m.read32(esp.wrapping_add(0x24));
                push!(edi);
                push!(ebx);
                m.write32(esp.wrapping_add(0x28), (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(ebp);
                eax = m.read32(esp.wrapping_add(0x2c));
                push!(eax);
                call!(0x308b10);
                pc = 0x30bbb1; continue;
            }
            0x30bbb1 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x28)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x20)))).to_bits();
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                ecx = m.read32(esp.wrapping_add(0x28));
                push!(esi);
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x30));
                push!(ecx);
                push!(edx);
                push!(ebp);
                push!(ebx);
                call!(0x308b10);
                pc = 0x30bbe9; continue;
            }
            0x30bbe9 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                eax = m.read32(esp.wrapping_add(0x40));
                ecx = m.read32(esp.wrapping_add(0x3c));
                ebp = m.read32(esp.wrapping_add(0x38));
                m.write32(esp.wrapping_add(0x48), xmm0.lanes[0]);
                ebx = m.read32(esp.wrapping_add(0x48));
                push!(esi);
                push!(ebx);
                push!(eax);
                push!(ecx);
                push!(ebp);
                call!(0x308b10);
                pc = 0x30bc17; continue;
            }
            0x30bc17 => {
                edx = m.read32(esp.wrapping_add(0x58));
                push!(esi);
                push!(ebx);
                push!(ebp);
                push!(edi);
                push!(edx);
                call!(0x308b10);
                pc = 0x30bc25; continue;
            }
            0x30bc25 => {
                esp = esp.wrapping_add(0x50);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x18);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030BC50: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30bc50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30bc50;
    let mut jt = 0u32;
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
        { crate::pose::cover(0x30bc50, previous, pc); previous = pc; }
        match pc {
            0x30bc50 => {
                xmm0.lanes[0] = (((m.read32(0x4e8cf0)) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4785d4))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x783f2c));
                esp = esp.wrapping_sub(0x10);
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                push!(esi);
                esi = eax;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30be3a; continue; }
                pc = 0x30bc77; continue;
            }
            0x30bc77 => {
                eax = edi.wrapping_add(0x37);
                push!(eax);
                call!(0x81c10);
                pc = 0x30bc80; continue;
            }
            0x30bc80 => {
                xmm0 = Xmm::scalar(m.read32(0x475d40));
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30bc9c; continue; }
                pc = 0x30bc8f; continue;
            }
            0x30bc8f => {
                xmm2 = Xmm::scalar(m.read32(0x475c78));
                xmm4 = xmm2;
                pc = 0x30bca7; continue;
            }
            0x30bc9c => {
                xmm4 = Xmm::scalar(m.read32(0x475cc8));
                xmm2 = xmm0;
                pc = 0x30bca7; continue;
            }
            0x30bca7 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                esi = ((m.read8(0x7ba034)) as u8 as u32);
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                if (fa != fb) { pc = 0x30bcbb; continue; }
                pc = 0x30bcb6; continue;
            }
            0x30bcb6 => {
                esi = 0x7f;
                pc = 0x30bcbb; continue;
            }
            0x30bcbb => {
                fa = edi;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                xmm3 = Xmm::scalar(m.read32(0x475cd4));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                if (fa > fb) { pc = 0x30bd6f; continue; }
                pc = 0x30bce0; continue;
            }
            0x30bce0 => {
                jt = m.read32(edi.wrapping_mul(0x4).wrapping_add(0x30be40));
                if (jt == 0x30bce7u32) { pc = 0x30bce7; continue; }
                if (jt == 0x30bcf1u32) { pc = 0x30bcf1; continue; }
                if (jt == 0x30bcffu32) { pc = 0x30bcff; continue; }
                if (jt == 0x30bd0du32) { pc = 0x30bd0d; continue; }
                if (jt == 0x30bd1fu32) { pc = 0x30bd1f; continue; }
                if (jt == 0x30bd2du32) { pc = 0x30bd2d; continue; }
                if (jt == 0x30bd45u32) { pc = 0x30bd45; continue; }
                if (jt == 0x30bd56u32) { pc = 0x30bd56; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x30bce7 => {
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x47d568))).to_bits();
                pc = 0x30bd68; continue;
            }
            0x30bcf1 => {
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x47d568))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                pc = 0x30bd68; continue;
            }
            0x30bcff => {
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x47d568))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                pc = 0x30bd6f; continue;
            }
            0x30bd0d => {
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x47d568))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                pc = 0x30bd6f; continue;
            }
            0x30bd1f => {
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x47d568))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                pc = 0x30bd6f; continue;
            }
            0x30bd2d => {
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x47d568))).to_bits();
                xh[0] = xmm2;
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = xh[0];
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                pc = 0x30bd6f; continue;
            }
            0x30bd45 => {
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x47d568))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = xmm2;
                pc = 0x30bd6f; continue;
            }
            0x30bd56 => {
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x47d568))).to_bits();
                xh[0] = xmm2;
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = xh[0];
                pc = 0x30bd68; continue;
            }
            0x30bd68 => {
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1 = xmm2;
                pc = 0x30bd6f; continue;
            }
            0x30bd6f => {
                xh[0].lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm2 = xmm4;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm3 = Xmm::scalar(m.read32(0x783f2c));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm2.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x783f28));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                push!(eax);
                m.write32(esp.wrapping_add(0x8), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x14), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                call!(0x1ed50);
                pc = 0x30bde4; continue;
            }
            0x30bde4 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x8)))).to_bits();
                ecx = m.read32(esp.wrapping_add(0x8));
                edx = m.read32(esp.wrapping_add(0xc));
                esp = esp.wrapping_add(0x4);
                esi = (esi & 0xff);
                esi = (esi | 0x7f7f7f00);
                push!(esi);
                push!(0x200);
                push!(0x200);
                push!(0);
                push!(0);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x2c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x24)))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                push!(edx);
                call!(0x1b7b0);
                pc = 0x30be37; continue;
            }
            0x30be37 => {
                esp = esp.wrapping_add(0x24);
                pc = 0x30be3a; continue;
            }
            0x30be3a => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x10);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030BE60: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30be60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30be60;
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
        { crate::pose::cover(0x30be60, previous, pc); previous = pc; }
        match pc {
            0x30be60 => {
                esp = esp.wrapping_sub(0x2c);
                call!(0x22b40);
                pc = 0x30be68; continue;
            }
            0x30be68 => {
                ecx = m.read32(0x4e8cf0);
                xmm1 = Xmm::scalar(m.read32(0x7ba038));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x47d014))).to_bits();
                ecx = ecx.wrapping_sub(eax);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                call!(0x22b90);
                pc = 0x30be93; continue;
            }
            0x30be93 => {
                xmm1 = Xmm::scalar(m.read32(0x7ba188));
                ecx = m.read32(0x7844a8);
                xmm0 = xmm1;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d1e4))).to_bits();
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x47d698))).to_bits();
                xmm4.lanes[0] = (((eax) as i32) as f32).to_bits();
                eax = m.read32(ecx.wrapping_add(0x200));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0 = xmm1;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x49f7a8))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x49e324))).to_bits();
                xmm3 = xmm2;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x4760cc))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x14), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d55c))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x18), xmm4.lanes[0]);
                m.write32(esp, xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x10), xmm1.lanes[0]);
                if (fa == fb) { pc = 0x30c175; continue; }
                pc = 0x30bf26; continue;
            }
            0x30bf26 => {
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x4)))).to_bits();
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                push!(edi);
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                ebx = 0xfffffffeu32;
                m.write32(esp.wrapping_add(0x18), 0x5);
                pc = 0x30bf50; continue;
            }
            0x30bf49 => {
                ecx = m.read32(0x7844a8);
                pc = 0x30bf50; continue;
            }
            0x30bf50 => {
                edx = m.read32(ecx.wrapping_add(0x25c));
                fa = (((((((edx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                eax = 0xb;
                if ((fa & fb) == 0) { pc = 0x30bf77; continue; }
                pc = 0x30bf60; continue;
            }
            0x30bf60 => {
                fa = ebx;
                fb = m.read32(ecx.wrapping_add(0x26c));
                if (fa != fb) { pc = 0x30bf77; continue; }
                pc = 0x30bf68; continue;
            }
            0x30bf68 => {
                edi = ((m.read8(0x7ba17c)) as u8 as u32);
                edi = (edi | 0x64323200);
                pc = 0x30bfa7; continue;
            }
            0x30bf77 => {
                fa = ebx;
                fb = m.read32(ecx.wrapping_add(0x264));
                if (fa != fb) { pc = 0x30bf8e; continue; }
                pc = 0x30bf7f; continue;
            }
            0x30bf7f => {
                edi = ((m.read8(0x7ba034)) as u8 as u32);
                edi = (edi | 0x3c7800);
                pc = 0x30bfa7; continue;
            }
            0x30bf8e => {
                fa = ebx;
                fb = m.read32(ecx.wrapping_add(0x268));
                if (fa != fb) { pc = 0x30bf9d; continue; }
                pc = 0x30bf96; continue;
            }
            0x30bf96 => {
                edi = 0x1e1e1e64;
                pc = 0x30bfa7; continue;
            }
            0x30bf9d => {
                edi = 0x3232327f;
                eax = 0xa;
                pc = 0x30bfa7; continue;
            }
            0x30bfa7 => {
                push!(eax);
                call!(0x81c10);
                pc = 0x30bfad; continue;
            }
            0x30bfad => {
                esi = eax;
                push!(esi);
                call!(0x1ed50);
                pc = 0x30bfb5; continue;
            }
            0x30bfb5 => {
                esi = ((esi as i32).wrapping_mul((0x34) as i32)) as u32;
                edx = m.read32(0x4b85c8);
                eax = m.read32(edx);
                ecx = ((m.read16(eax.wrapping_add(esi).wrapping_add(0x2a))) as u16 as u32);
                edx = ((m.read16(eax.wrapping_add(esi).wrapping_add(0x26))) as u16 as u32);
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x49f7a4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                esp = esp.wrapping_add(0x8);
                eax = eax.wrapping_add(esi);
                eax = m.read32(esp.wrapping_add(0x1c));
                push!(edi);
                ecx = ecx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(ecx);
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(edx);
                push!(0);
                push!(0);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x28));
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                push!(ecx);
                push!(ebp);
                call!(0x1b7b0);
                pc = 0x30c008; continue;
            }
            0x30c008 => {
                eax = m.read32(esp.wrapping_add(0x3c));
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d698))).to_bits();
                esp = esp.wrapping_add(0x24);
                ebx = ebx.wrapping_add(1);
                eax = eax.wrapping_sub(1);
                fa = eax;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x18), eax);
                if ((fa != 0)) { pc = 0x30bf49; continue; }
                pc = 0x30c03b; continue;
            }
            0x30c03b => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                m.write32(esp.wrapping_add(0x30), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x1c)));
                edx = esp.wrapping_add(0x2c);
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                push!(edx);
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                call!(0x711f0);
                pc = 0x30c075; continue;
            }
            0x30c075 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30c095; continue; }
                pc = 0x30c07c; continue;
            }
            0x30c07c => {
                eax = m.read32(0x7844a8);
                eax = (((((m.read8(eax.wrapping_add(0x11854)) as i8)) as i32)) as u32);
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0x30c095; continue; }
                pc = 0x30c08b; continue;
            }
            0x30c08b => {
                push!(0x3e);
                call!(0x314fa0);
                pc = 0x30c092; continue;
            }
            0x30c092 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x30c095; continue;
            }
            0x30c095 => {
                ecx = m.read32(0x7844a8);
                fa = m.read32(ecx.wrapping_add(0x25c));
                fb = ((((0x4044088i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x30c0cc; continue; }
                pc = 0x30c0a7; continue;
            }
            0x30c0a7 => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x30c0bf; continue;
            }
            0x30c0bf => {
                ecx = m.read32(0x7842a8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ecx;
                if (fa == fb) { pc = 0x30c0df; continue; }
                pc = 0x30c0cc; continue;
            }
            0x30c0cc => {
                ecx = m.read32(0x7844a8);
                fa = ((((m.read8(ecx.wrapping_add(0x11854)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x30c171; continue; }
                pc = 0x30c0df; continue;
            }
            0x30c0df => {
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d20))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x7ba188));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475cc8))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                push!(0x7f7f7f7f);
                push!(0x1960);
                m.write32(esp.wrapping_add(0x24), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                call!(0x3d7b0);
                pc = 0x30c120; continue;
            }
            0x30c120 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x30)))).to_bits();
                esi = m.read32(esp.wrapping_add(0x2c));
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                push!(esi);
                call!(0x41dc0);
                pc = 0x30c140; continue;
            }
            0x30c140 => {
                esp = esp.wrapping_add(0x10);
                push!(0x7f7f7f7f);
                push!(0x195f);
                call!(0x3d7b0);
                pc = 0x30c152; continue;
            }
            0x30c152 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x24)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x28)))).to_bits();
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(ecx);
                m.write32(esp, xmm0.lanes[0]);
                push!(esi);
                call!(0x41dc0);
                pc = 0x30c16e; continue;
            }
            0x30c16e => {
                esp = esp.wrapping_add(0x10);
                pc = 0x30c171; continue;
            }
            0x30c171 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x30c175; continue;
            }
            0x30c175 => {
                esp = esp.wrapping_add(0x2c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030C210: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30c210<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30c210;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x30c210, previous, pc); previous = pc; }
        match pc {
            0x30c210 => {
                esp = esp.wrapping_sub(0x28);
                push!(esi);
                push!(0x12);
                call!(0x3fa10);
                pc = 0x30c21b; continue;
            }
            0x30c21b => {
                push!(0xa7f);
                m.write32(esp.wrapping_add(0x18), eax);
                call!(0x3d7b0);
                pc = 0x30c229; continue;
            }
            0x30c229 => {
                esi = eax;
                esp = esp.wrapping_add(0x8);
                m.write32(esp.wrapping_add(0xc), esi);
                call!(0x30c180);
                pc = 0x30c237; continue;
            }
            0x30c237 => {
                eax = m.read32(0x7844a8);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x270)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if ((fca > fcb)) { pc = 0x30c24c; continue; }
                pc = 0x30c249; continue;
            }
            0x30c249 => {
                xmm0 = xmm1;
                pc = 0x30c24c; continue;
            }
            0x30c24c => {
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                push!(ebx);
                push!(ebp);
                push!(edi);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                push!(esi);
                m.write32(esp.wrapping_add(0x18), xmm1.lanes[0]);
                call!(0x405c0);
                pc = 0x30c267; continue;
            }
            0x30c267 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x40))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                esp = esp.wrapping_add(0x4);
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x475d30)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), (fpu.st(0) as f32).to_bits());
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(0x47d1f4)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x475c78)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                call!(0x3c848c);
                pc = 0x30c289; continue;
            }
            0x30c289 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                edx = m.read32(0x7844a8);
                fa = m.read32(edx.wrapping_add(0x25c));
                fb = ((((0x410000i32)) as u32) & 0xffffffffu32);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475dc0))).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                ecx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                m.write32(esp.wrapping_add(0x20), eax);
                m.write32(esp.wrapping_add(0x24), ecx);
                esi = 0x6e6e6e7f;
                ebp = 0x3746557f;
                if ((fa & fb) == 0) { pc = 0x30c2e2; continue; }
                pc = 0x30c2ca; continue;
            }
            0x30c2ca => {
                fa = ((((m.read8(0x7497b0))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                esi = 0x80500080u32;
                if ((fa & fb) == 0) { pc = 0x30c2e2; continue; }
                pc = 0x30c2d8; continue;
            }
            0x30c2d8 => {
                esi = 0x80800080u32;
                ebp = 0x64645064;
                pc = 0x30c2e2; continue;
            }
            0x30c2e2 => {
                ebx = (((((f32::from_bits(m.read32(esp.wrapping_add(0x10))) as f64)) as i32)) as u32);
                push!(0xf0f0f64);
                push!(ecx);
                push!(eax);
                edi = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(edi);
                push!(ebx);
                call!(0x1efb0);
                pc = 0x30c2fa; continue;
            }
            0x30c2fa => {
                eax = m.read32(esp.wrapping_add(0x28));
                push!(0);
                push!(esi);
                push!(esi);
                push!(ebp);
                push!(0x10);
                push!(0x50);
                push!(edi);
                push!(ebx);
                push!(eax);
                call!(0x814b0);
                pc = 0x30c30f; continue;
            }
            0x30c30f => {
                eax = m.read32(0x7844a8);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x274)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x270)))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                esp = esp.wrapping_add(0x38);
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30c33c; continue; }
                pc = 0x30c337; continue;
            }
            0x30c337 => {
                esi = 0x3746557f;
                pc = 0x30c33c; continue;
            }
            0x30c33c => {
                ecx = m.read32(esp.wrapping_add(0xc));
                edx = m.read32(esp.wrapping_add(0x34));
                eax = m.read32(esp.wrapping_add(0x30));
                push!(0x80);
                push!(esi);
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x41e30);
                pc = 0x30c356; continue;
            }
            0x30c356 => {
                ecx = m.read32(esp.wrapping_add(0x24));
                push!(ecx);
                call!(0x3fa10);
                pc = 0x30c360; continue;
            }
            0x30c360 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x48)));
                m.write32(esp.wrapping_add(0x34), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x4c)));
                m.write32(esp.wrapping_add(0x38), xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(esp.wrapping_add(0x2c))) as i32) as f32).to_bits();
                edx = esp.wrapping_add(0x34);
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(esp.wrapping_add(0x30))) as i32) as f32).to_bits();
                push!(edx);
                m.write32(esp.wrapping_add(0x44), xmm0.lanes[0]);
                call!(0x711f0);
                pc = 0x30c39a; continue;
            }
            0x30c39a => {
                esp = esp.wrapping_add(0x1c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x30c3bb; continue; }
                pc = 0x30c3a2; continue;
            }
            0x30c3a2 => {
                eax = m.read32(0x7844a8);
                eax = (((((m.read8(eax.wrapping_add(0x11854)) as i8)) as i32)) as u32);
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0x30c3bb; continue; }
                pc = 0x30c3b1; continue;
            }
            0x30c3b1 => {
                push!(0x3d);
                call!(0x314fa0);
                pc = 0x30c3b8; continue;
            }
            0x30c3b8 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x30c3bb; continue;
            }
            0x30c3bb => {
                esp = esp.wrapping_add(0x28);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030C3C0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30c3c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30c3c0;
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
        { crate::pose::cover(0x30c3c0, previous, pc); previous = pc; }
        match pc {
            0x30c3c0 => {
                eax = m.read32(0x7844a8);
                fa = m.read32(eax.wrapping_add(0x25c));
                fb = ((((0x44088i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x30c3e8; continue; }
                pc = 0x30c3d1; continue;
            }
            0x30c3d1 => {
                call!(0x774b0);
                pc = 0x30c3d6; continue;
            }
            0x30c3d6 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30c436; continue; }
                pc = 0x30c3da; continue;
            }
            0x30c3da => {
                call!(0x774b0);
                pc = 0x30c3df; continue;
            }
            0x30c3df => {
                fa = ((((m.read8(eax.wrapping_add(0x218)))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30c436; continue; }
                pc = 0x30c3e8; continue;
            }
            0x30c3e8 => {
                call!(0x22b90);
                pc = 0x30c3ed; continue;
            }
            0x30c3ed => {
                xmm1 = Xmm::scalar(m.read32(0x7ba188));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x47d1e4))).to_bits();
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                push!(ecx);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x22b40);
                pc = 0x30c410; continue;
            }
            0x30c410 => {
                xmm1 = Xmm::scalar(m.read32(0x7ba038));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x4760cc))).to_bits();
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                push!(ecx);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x30c210);
                pc = 0x30c433; continue;
            }
            0x30c433 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x30c436; continue;
            }
            0x30c436 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030C440: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30c440<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30c440;
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
        { crate::pose::cover(0x30c440, previous, pc); previous = pc; }
        match pc {
            0x30c440 => {
                esp = esp.wrapping_sub(0x2c);
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                push!(eax);
                call!(0x802a0);
                pc = 0x30c44d; continue;
            }
            0x30c44d => {
                ecx = m.read32(esp.wrapping_add(0x48));
                push!(ecx);
                m.write32(esp.wrapping_add(0x1c), eax);
                call!(0x802c0);
                pc = 0x30c45b; continue;
            }
            0x30c45b => {
                ebp = eax;
                push!(0);
                m.write32(esp.wrapping_add(0x38), ebp);
                call!(0x80f20);
                pc = 0x30c468; continue;
            }
            0x30c468 => {
                edx = m.read32(0x4b85c8);
                ecx = m.read32(edx);
                m.write32(esp.wrapping_add(0x30), eax);
                eax = ((eax as i32).wrapping_mul((0x34) as i32)) as u32;
                ebx = m.read32(esp.wrapping_add(0x4c));
                eax = eax.wrapping_add(ecx);
                ecx = ((m.read16(eax.wrapping_add(0x26))) as u16 as u32);
                eax = ((m.read16(eax.wrapping_add(0x2a))) as u16 as u32);
                eax = eax.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write32(esp.wrapping_add(0x24), eax);
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x78) as i32)) as u32;
                eax = eax.wrapping_add(ebx);
                eax = eax.wrapping_shl((((((((0x2i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = m.read32(eax.wrapping_add(0x7ba34c));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba344)));
                eax = m.read32(ebp.wrapping_add(0x50));
                ebx = edx;
                ecx = ecx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(ebx);
                push!(eax);
                m.write32(esp.wrapping_add(0x30), ecx);
                m.write32(esp.wrapping_add(0x34), edx);
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                call!(0x154fe0);
                pc = 0x30c4c4; continue;
            }
            0x30c4c4 => {
                ecx = m.read32(ebp.wrapping_add(0x54));
                push!(ebx);
                push!(ecx);
                esi = eax;
                call!(0x154fe0);
                pc = 0x30c4d0; continue;
            }
            0x30c4d0 => {
                edx = m.read32(ebp.wrapping_add(0x58));
                push!(ebx);
                push!(edx);
                edi = eax;
                call!(0x154fe0);
                pc = 0x30c4dc; continue;
            }
            0x30c4dc => {
                ecx = m.read32(ebp.wrapping_add(0x5c));
                ebx = eax;
                eax = m.read32(esp.wrapping_add(0x44));
                push!(eax);
                push!(ecx);
                call!(0x154fe0);
                pc = 0x30c4ec; continue;
            }
            0x30c4ec => {
                edx = m.read32(esp.wrapping_add(0x4c));
                ebp = eax;
                eax = m.read32(esp.wrapping_add(0x58));
                ecx = m.read32(eax.wrapping_add(0x48));
                push!(edx);
                push!(ecx);
                call!(0x154fe0);
                pc = 0x30c500; continue;
            }
            0x30c500 => {
                ecx = eax;
                edx = eax;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx & 0xff0000);
                ecx = (ecx | edx);
                edx = eax;
                edx = edx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax & 0xff00);
                edx = (edx | eax);
                ecx = ecx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                m.write32(esp.wrapping_add(0x64), ecx);
                call!(0x774b0);
                pc = 0x30c52c; continue;
            }
            0x30c52c => {
                m.write32(esp.wrapping_add(0x60), eax);
                eax = m.read32(esp.wrapping_add(0x58));
                push!(eax);
                call!(0x1ed50);
                pc = 0x30c53a; continue;
            }
            0x30c53a => {
                push!(0);
                call!(0x19650);
                pc = 0x30c541; continue;
            }
            0x30c541 => {
                push!(0x1);
                call!(0x19d70);
                pc = 0x30c548; continue;
            }
            0x30c548 => {
                eax = m.read32(esp.wrapping_add(0x80));
                esp = esp.wrapping_add(0x40);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = m.read32(0x4e8cf0);
                if (fa != fb) { pc = 0x30c57c; continue; }
                pc = 0x30c55b; continue;
            }
            0x30c55b => {
                xmm0 = Xmm::default();
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x28)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba180))).to_bits();
                pc = 0x30c59c; continue;
            }
            0x30c57c => {
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x28)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba180))).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                pc = 0x30c59c; continue;
            }
            0x30c59c => {
                ecx = m.read32(0x4e8cf4);
                push!(ecx);
                push!(eax);
                push!(0);
                push!(0);
                m.write32(esp.wrapping_add(0x50), xmm0.lanes[0]);
                call!(0x1a010);
                pc = 0x30c5b3; continue;
            }
            0x30c5b3 => {
                eax = m.read32(esp.wrapping_add(0x24));
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0x10)));
                xmm4 = Xmm::scalar(m.read32(eax.wrapping_add(0x18)));
                eax = m.read32(esp.wrapping_add(0x3c));
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0xac)));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0xb4)));
                xh[0] = Xmm::scalar(m.read32(0x7ba188));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x24), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(0x475c78));
                esp = esp.wrapping_add(0x10);
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xh[0].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x34), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x38), xmm4.lanes[0]);
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                if ((fca > fcb)) { pc = 0x30c61b; continue; }
                pc = 0x30c618; continue;
            }
            0x30c618 => {
                xmm2 = xmm1;
                pc = 0x30c61b; continue;
            }
            0x30c61b => {
                xh[1] = Xmm::scalar(m.read32(0x7ba184));
                xh[2] = xh[0];
                xh[2].lanes[0] = sse::mulss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(m.read32(0x49f7b0))).to_bits();
                xh[2].lanes[0] = sse::addss(f32::from_bits(xh[2].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xh[2].lanes[0]) as f64;
                m.write32(esp.wrapping_add(0x24), 0x1);
                if ((fca >= fcb)) { pc = 0x30c647; continue; }
                pc = 0x30c63f; continue;
            }
            0x30c63f => {
                m.write32(esp.wrapping_add(0x24), 0);
                pc = 0x30c647; continue;
            }
            0x30c647 => {
                ecx = m.read32(esp.wrapping_add(0x48));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30c674; continue; }
                pc = 0x30c64f; continue;
            }
            0x30c64f => {
                edx = m.read32(esp.wrapping_add(0x24));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x30c674; continue; }
                pc = 0x30c657; continue;
            }
            0x30c657 => {
                fa = ((((m.read8(eax.wrapping_add(0x218)))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x30c674; continue; }
                pc = 0x30c660; continue;
            }
            0x30c660 => {
                xh[0].lanes[0] = sse::mulss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(m.read32(0x4785c8))).to_bits();
                xh[0].lanes[0] = sse::addss(f32::from_bits(xh[0].lanes[0]), f32::from_bits(xh[1].lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x48), xh[0].lanes[0]);
                pc = 0x30c682; continue;
            }
            0x30c674 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x48), xmm0.lanes[0]);
                if (fa != fb) { pc = 0x30c8a1; continue; }
                pc = 0x30c682; continue;
            }
            0x30c682 => {
                eax = m.read32(esp.wrapping_add(0x30));
                edx = m.read32(esp.wrapping_add(0x18));
                ecx = m.read32(esp.wrapping_add(0x48));
                push!(eax);
                push!(eax);
                push!(eax);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x2c));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x54));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x2c));
                push!(0);
                push!(0);
                push!(0x3f800000);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x38));
                push!(edx);
                push!(eax);
                push!(ecx);
                call!(0x1f7b0);
                pc = 0x30c6b6; continue;
            }
            0x30c6b6 => {
                edx = ebp;
                edx = (edx & 0xff0000);
                eax = ebp;
                eax = eax.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | eax);
                edx = edx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ebp;
                ecx = ecx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ebp = (ebp & 0xff00);
                ecx = (ecx | ebp);
                ecx = ecx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | ecx);
                push!(edx);
                edx = ebx;
                edx = (edx & 0xff0000);
                eax = ebx;
                eax = eax.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | eax);
                edx = edx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ebx;
                ecx = ecx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ebx = (ebx & 0xff00);
                ecx = (ecx | ebx);
                ecx = ecx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | ecx);
                push!(edx);
                edx = edi;
                edx = (edx & 0xff0000);
                eax = edi;
                eax = eax.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | eax);
                ecx = edi;
                ecx = (ecx & 0xff00);
                edx = edx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edi = edi.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edi);
                ecx = ecx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | ecx);
                push!(edx);
                edx = esi;
                edx = (edx & 0xff0000);
                eax = esi;
                ecx = esi;
                ecx = (ecx & 0xff00);
                eax = eax.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | eax);
                eax = m.read32(esp.wrapping_add(0x5c));
                esi = esi.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | esi);
                edx = edx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | ecx);
                ecx = m.read32(esp.wrapping_add(0x88));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x5c));
                push!(edx);
                push!(eax);
                push!(0);
                push!(0);
                push!(0x3f800000);
                edx = m.read32(esp.wrapping_add(0x98));
                eax = m.read32(esp.wrapping_add(0x6c));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x6c));
                push!(edx);
                push!(eax);
                push!(ecx);
                call!(0x1f7b0);
                pc = 0x30c77c; continue;
            }
            0x30c77c => {
                eax = m.read32(esp.wrapping_add(0x8c));
                esp = esp.wrapping_add(0x68);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x30c899; continue; }
                pc = 0x30c78e; continue;
            }
            0x30c78e => {
                edi = m.read32(esp.wrapping_add(0x20));
                push!(edi);
                push!(0x1818217f);
                call!(0x154fe0);
                pc = 0x30c79d; continue;
            }
            0x30c79d => {
                push!(edi);
                push!(0x7f7f7f1e);
                m.write32(esp.wrapping_add(0x48), eax);
                call!(0x154fe0);
                pc = 0x30c7ac; continue;
            }
            0x30c7ac => {
                esi = eax;
                push!(edi);
                push!(0x7f7f7f1e);
                esi = (esi & 0xff0000);
                call!(0x154fe0);
                pc = 0x30c7bf; continue;
            }
            0x30c7bf => {
                eax = eax.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = (esi | eax);
                push!(edi);
                push!(0x7f7f7f1e);
                esi = esi.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                call!(0x154fe0);
                pc = 0x30c7d2; continue;
            }
            0x30c7d2 => {
                ebx = eax;
                push!(edi);
                push!(0x7f7f7f1e);
                ebx = ebx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                call!(0x154fe0);
                pc = 0x30c7e2; continue;
            }
            0x30c7e2 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x50)));
                eax = (eax & 0xff00);
                ebx = (ebx | eax);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x4760cc))).to_bits();
                ebx = ebx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(0x1);
                esi = (esi | ebx);
                m.write32(esp.wrapping_add(0x58), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x74), xmm0.lanes[0]);
                call!(0x80f20);
                pc = 0x30c81a; continue;
            }
            0x30c81a => {
                ebp = eax;
                eax = m.read32(0x4b85c8);
                ecx = m.read32(eax);
                edx = ebp;
                edx = ((edx as i32).wrapping_mul((0x34) as i32)) as u32;
                edi = ((m.read16(edx.wrapping_add(ecx).wrapping_add(0x26))) as u16 as u32);
                ebx = ((m.read16(edx.wrapping_add(ecx).wrapping_add(0x2a))) as u16 as u32);
                eax = edx.wrapping_add(ecx);
                edx = m.read32(esp.wrapping_add(0x64));
                eax = (((((f32::from_bits(m.read32(esp.wrapping_add(0x74))) as f64)) as i32)) as u32);
                ecx = (((((f32::from_bits(m.read32(esp.wrapping_add(0x6c))) as f64)) as i32)) as u32);
                push!(edx);
                edx = (((((f32::from_bits(m.read32(esp.wrapping_add(0x5c))) as f64)) as i32)) as u32);
                push!(eax);
                eax = (((((f32::from_bits(m.read32(esp.wrapping_add(0x44))) as f64)) as i32)) as u32);
                push!(ecx);
                push!(edx);
                push!(eax);
                edi = edi.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                ebx = ebx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                call!(0x1efb0);
                pc = 0x30c861; continue;
            }
            0x30c861 => {
                esp = esp.wrapping_add(0x40);
                push!(0x2);
                call!(0x19d70);
                pc = 0x30c86b; continue;
            }
            0x30c86b => {
                push!(ebp);
                call!(0x1ed50);
                pc = 0x30c871; continue;
            }
            0x30c871 => {
                ecx = m.read32(esp.wrapping_add(0x50));
                edx = m.read32(esp.wrapping_add(0x48));
                eax = m.read32(esp.wrapping_add(0x34));
                push!(esi);
                push!(esi);
                push!(esi);
                push!(esi);
                push!(ebx);
                push!(edi);
                push!(0);
                push!(0);
                push!(0);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x40));
                push!(edx);
                push!(eax);
                push!(ecx);
                call!(0x1f7b0);
                pc = 0x30c896; continue;
            }
            0x30c896 => {
                esp = esp.wrapping_add(0x3c);
                pc = 0x30c899; continue;
            }
            0x30c899 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x2c);
                break 'run;
            }
            0x30c8a1 => {
                xmm0 = xmm1;
                xmm0.lanes[0] = sse::divss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                push!(0);
                xmm2 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                push!(esi);
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm3.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                push!(ebx);
                m.write32(esp.wrapping_add(0x30), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x38), xmm1.lanes[0]);
                call!(0x154d50);
                pc = 0x30c8cc; continue;
            }
            0x30c8cc => {
                push!(0);
                push!(edi);
                push!(ebp);
                m.write32(esp.wrapping_add(0x38), eax);
                call!(0x154d50);
                pc = 0x30c8d9; continue;
            }
            0x30c8d9 => {
                edx = m.read32(esp.wrapping_add(0x3c));
                esp = esp.wrapping_add(0x18);
                push!(0x3e99999a);
                push!(edx);
                push!(esi);
                push!(ebx);
                m.write32(esp.wrapping_add(0x58), eax);
                call!(0x154d50);
                pc = 0x30c8f1; continue;
            }
            0x30c8f1 => {
                esp = esp.wrapping_add(0xc);
                push!(eax);
                call!(0x154fe0);
                pc = 0x30c8fa; continue;
            }
            0x30c8fa => {
                esp = esp.wrapping_add(0x8);
                m.write32(esp.wrapping_add(0x30), eax);
                eax = m.read32(esp.wrapping_add(0x24));
                push!(0x3e99999a);
                push!(eax);
                push!(edi);
                push!(ebp);
                call!(0x154d50);
                pc = 0x30c912; continue;
            }
            0x30c912 => {
                esp = esp.wrapping_add(0xc);
                push!(eax);
                call!(0x154fe0);
                pc = 0x30c91b; continue;
            }
            0x30c91b => {
                ecx = eax;
                ecx = (ecx & 0xff0000);
                edx = eax;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                edx = eax;
                edx = edx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax & 0xff00);
                edx = (edx | eax);
                eax = m.read32(esp.wrapping_add(0x38));
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                esp = esp.wrapping_add(0x8);
                push!(ecx);
                ecx = eax;
                ecx = (ecx & 0xff0000);
                edx = eax;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                edx = eax;
                edx = edx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax & 0xff00);
                edx = (edx | eax);
                eax = m.read32(esp.wrapping_add(0x4c));
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                push!(ecx);
                ecx = eax;
                edx = eax;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx & 0xff0000);
                ecx = (ecx | edx);
                edx = eax;
                edx = edx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax & 0xff00);
                edx = (edx | eax);
                ecx = ecx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                push!(ecx);
                eax = m.read32(esp.wrapping_add(0x2c));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x40)))).to_bits();
                ecx = eax;
                edx = eax;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx & 0xff0000);
                ecx = (ecx | edx);
                edx = eax;
                edx = edx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax & 0xff00);
                edx = (edx | eax);
                eax = m.read32(esp.wrapping_add(0x24));
                ecx = ecx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                edx = m.read32(esp.wrapping_add(0x4c));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x2c));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x28));
                push!(ecx);
                push!(0);
                push!(0);
                push!(0x3f800000);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x38));
                m.write32(esp, xmm0.lanes[0]);
                push!(edx);
                push!(eax);
                push!(ecx);
                call!(0x1f7b0);
                pc = 0x30c9f3; continue;
            }
            0x30c9f3 => {
                edx = m.read32(esp.wrapping_add(0x60));
                esp = esp.wrapping_add(0x34);
                push!(0x3e99999a);
                push!(edx);
                push!(esi);
                push!(ebx);
                call!(0x154d50);
                pc = 0x30ca07; continue;
            }
            0x30ca07 => {
                esp = esp.wrapping_add(0xc);
                push!(eax);
                call!(0x154fe0);
                pc = 0x30ca10; continue;
            }
            0x30ca10 => {
                esp = esp.wrapping_add(0x8);
                m.write32(esp.wrapping_add(0x20), eax);
                eax = m.read32(esp.wrapping_add(0x2c));
                push!(0x3e99999a);
                push!(eax);
                push!(edi);
                push!(ebp);
                call!(0x154d50);
                pc = 0x30ca28; continue;
            }
            0x30ca28 => {
                esp = esp.wrapping_add(0xc);
                push!(eax);
                call!(0x154fe0);
                pc = 0x30ca31; continue;
            }
            0x30ca31 => {
                push!(0x3f800000);
                push!(esi);
                push!(ebx);
                m.write32(esp.wrapping_add(0x5c), eax);
                call!(0x154d50);
                pc = 0x30ca41; continue;
            }
            0x30ca41 => {
                push!(0x3f800000);
                push!(edi);
                push!(ebp);
                esi = eax;
                call!(0x154d50);
                pc = 0x30ca4f; continue;
            }
            0x30ca4f => {
                ecx = eax;
                edx = eax;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx & 0xff0000);
                ecx = (ecx | edx);
                edx = eax;
                eax = (eax & 0xff00);
                edx = edx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx | eax);
                ecx = ecx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                esp = esp.wrapping_add(0x20);
                push!(ecx);
                eax = esi;
                ecx = esi;
                eax = (eax & 0xff0000);
                ecx = ecx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax | ecx);
                eax = eax.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = esi;
                edx = edx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                esi = (esi & 0xff00);
                edx = (edx | esi);
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax | edx);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x50));
                ecx = eax;
                ecx = (ecx & 0xff0000);
                edx = eax;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                edx = eax;
                edx = edx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax & 0xff00);
                edx = (edx | eax);
                eax = m.read32(esp.wrapping_add(0x28));
                ecx = ecx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                push!(ecx);
                ecx = eax;
                edx = eax;
                edx = edx.wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx & 0xff0000);
                ecx = (ecx | edx);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x44)))).to_bits();
                edx = eax;
                edx = edx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = (eax & 0xff00);
                edx = (edx | eax);
                eax = m.read32(esp.wrapping_add(0x24));
                ecx = ecx.wrapping_shr((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = edx.wrapping_shl((((((((0x8i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = (ecx | edx);
                edx = m.read32(esp.wrapping_add(0x34));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x2c));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x54));
                push!(ecx);
                push!(0);
                push!(0);
                push!(0x3f800000);
                push!(edx);
                push!(eax);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x40));
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                call!(0x1f7b0);
                pc = 0x30cb23; continue;
            }
            0x30cb23 => {
                esp = esp.wrapping_add(0x34);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x2c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030CB30: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30cb30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30cb30;
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
        { crate::pose::cover(0x30cb30, previous, pc); previous = pc; }
        match pc {
            0x30cb30 => {
                eax = m.read32(0x7ba9e0);
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x7ba344)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475dbc)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30cb65; continue; }
                pc = 0x30cb4e; continue;
            }
            0x30cb4e => {
                push!(esi);
                push!(0);
                push!(0);
                eax = 0x1;
                call!(0x30c440);
                pc = 0x30cb5d; continue;
            }
            0x30cb5d => {
                eax = m.read32(0x7ba9e0);
                esp = esp.wrapping_add(0xc);
                pc = 0x30cb65; continue;
            }
            0x30cb65 => {
                xmm0 = Xmm::scalar(m.read32(0x49f7b4));
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(eax.wrapping_add(0x7ba348))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30cb8e; continue; }
                pc = 0x30cb7c; continue;
            }
            0x30cb7c => {
                push!(esi);
                push!(0);
                push!(0x1);
                eax = 0x2;
                call!(0x30c440);
                pc = 0x30cb8b; continue;
            }
            0x30cb8b => {
                esp = esp.wrapping_add(0xc);
                pc = 0x30cb8e; continue;
            }
            0x30cb8e => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030CB90: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30cb90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30cb90;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x30cb90, previous, pc); previous = pc; }
        match pc {
            0x30cb90 => {
                xmm0 = Xmm::scalar(m.read32(0x7844ac));
                esp = esp.wrapping_sub(0x8);
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x30ccc0; continue; }
                pc = 0x30cba8; continue;
            }
            0x30cba8 => {
                fpu.push((f32::from_bits(m.read32(0x7844ac)) as f64));
                push!(ebx);
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475cc0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(ebp);
                push!(esi);
                push!(edi);
                call!(0x3c848c);
                pc = 0x30cbbd; continue;
            }
            0x30cbbd => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm1 = Xmm::scalar(m.read32(0x7ba184));
                esi = eax;
                esi = (esi & 0xff);
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x47d1ec))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x49f7b0))).to_bits();
                eax = esi;
                eax = (eax | 0x20202000);
                ebp = esi;
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                push!(0x1);
                ebp = (ebp | 0x7f7f7f00);
                m.write32(esp.wrapping_add(0x14), eax);
                ebx = sse::cvttss2si(f32::from_bits(xmm2.lanes[0]));
                edi = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                call!(0x19d70);
                pc = 0x30cc12; continue;
            }
            0x30cc12 => {
                push!(0);
                call!(0x19650);
                pc = 0x30cc19; continue;
            }
            0x30cc19 => {
                push!(esi);
                eax = ebx.wrapping_add(0x6);
                push!(eax);
                m.write32(esp.wrapping_add(0x24), eax);
                eax = m.read32(0x4e8cf0);
                push!(eax);
                push!(ebx);
                push!(0);
                call!(0x1efb0);
                pc = 0x30cc30; continue;
            }
            0x30cc30 => {
                ecx = m.read32(esp.wrapping_add(0x2c));
                eax = m.read32(0x4e8cf0);
                push!(ecx);
                edx = ebx.wrapping_add(0x5);
                push!(edx);
                push!(eax);
                ecx = ebx.wrapping_add(0x1);
                push!(ecx);
                push!(0);
                call!(0x1efb0);
                pc = 0x30cc4a; continue;
            }
            0x30cc4a => {
                edx = m.read32(0x4e8cf0);
                push!(ebp);
                ebx = ebx.wrapping_add(0x2);
                push!(ebx);
                push!(edx);
                push!(ebx);
                push!(0);
                call!(0x1b090);
                pc = 0x30cc5e; continue;
            }
            0x30cc5e => {
                ecx = m.read32(0x4e8cf0);
                esp = esp.wrapping_add(0x44);
                push!(esi);
                eax = edi.wrapping_add(0x6);
                push!(eax);
                push!(ecx);
                push!(edi);
                push!(0);
                call!(0x1efb0);
                pc = 0x30cc75; continue;
            }
            0x30cc75 => {
                edx = m.read32(esp.wrapping_add(0x24));
                ecx = m.read32(0x4e8cf0);
                push!(edx);
                eax = edi.wrapping_add(0x5);
                push!(eax);
                push!(ecx);
                edx = edi.wrapping_add(0x1);
                push!(edx);
                push!(0);
                call!(0x1efb0);
                pc = 0x30cc90; continue;
            }
            0x30cc90 => {
                ecx = m.read32(0x4e8cf0);
                push!(ebp);
                eax = edi.wrapping_add(0x2);
                push!(eax);
                push!(ecx);
                push!(eax);
                push!(0);
                call!(0x1b090);
                pc = 0x30cca4; continue;
            }
            0x30cca4 => {
                edx = m.read32(0x4e8cf0);
                eax = m.read32(esp.wrapping_add(0x50));
                push!(esi);
                push!(edi);
                push!(edx);
                push!(eax);
                push!(0);
                call!(0x1efb0);
                pc = 0x30ccb9; continue;
            }
            0x30ccb9 => {
                esp = esp.wrapping_add(0x50);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x30ccc0; continue;
            }
            0x30ccc0 => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0030CCD0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f30ccd0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x30ccd0;
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
        { crate::pose::cover(0x30ccd0, previous, pc); previous = pc; }
        match pc {
            0x30ccd0 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d1e4))).to_bits();
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0xc));
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x1c));
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x1c));
                push!(0x7f);
                push!(0);
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                push!(eax);
                push!(esi);
                push!(edi);
                push!(ebx);
                push!(ebp);
                call!(0x1361f0);
                pc = 0x30cd07; continue;
            }
            0x30cd07 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x47d1e4))).to_bits();
                push!(0x695a9156);
                push!(0x1);
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                push!(eax);
                push!(esi);
                push!(edi);
                push!(ebx);
                push!(ebp);
                call!(0x1361f0);
                pc = 0x30cd2d; continue;
            }
            0x30cd2d => {
                esp = esp.wrapping_add(0x40);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
