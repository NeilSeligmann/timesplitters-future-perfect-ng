use super::*;
#[rustfmt::skip] mod unformatted { use super::*;
/// 0x002FBF70: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fbf70<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fbf70;
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
        { crate::pose::cover(0x2fbf70, previous, pc); previous = pc; }
        match pc {
            0x2fbf70 => {
                eax = m.read32(0x7844a8);
                fa = m.read32(eax.wrapping_add(0x25c));
                fb = ((((0x16004008i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x2fc020; continue; }
                pc = 0x2fbf85; continue;
            }
            0x2fbf85 => {
                call!(0x774b0);
                pc = 0x2fbf8a; continue;
            }
            0x2fbf8a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fbfa0; continue; }
                pc = 0x2fbf8e; continue;
            }
            0x2fbf8e => {
                call!(0x774b0);
                pc = 0x2fbf93; continue;
            }
            0x2fbf93 => {
                fa = ((((m.read8(eax.wrapping_add(0x218)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x2fc020; continue; }
                pc = 0x2fbfa0; continue;
            }
            0x2fbfa0 => {
                ecx = m.read32(0x7844a8);
                eax = (eax & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x25c))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                fas = (((((fa)) as i8)) as i32);
                fbs = (((((fb)) as i8)) as i32);
                if (((((((((((((fas)) as u8)) as i32).wrapping_sub((((((fbs)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) != 0i32)) { pc = 0x2fbfd5; continue; }
                pc = 0x2fbfb0; continue;
            }
            0x2fbfb0 => {
                edx = m.read32(0x7ba9e0);
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(0x7ba26c));
                push!(eax);
                call!(0x77470);
                pc = 0x2fbfc8; continue;
            }
            0x2fbfc8 => {
                ecx = m.read32(0x7842a8);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ecx;
                if (fa != fb) { pc = 0x2fc020; continue; }
                pc = 0x2fbfd5; continue;
            }
            0x2fbfd5 => {
                ecx = m.read32(0x7844a8);
                fa = ((((m.read8(ecx.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x2fc020; continue; }
                pc = 0x2fbfe4; continue;
            }
            0x2fbfe4 => {
                call!(0x314d60);
                pc = 0x2fbfe9; continue;
            }
            0x2fbfe9 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fc020; continue; }
                pc = 0x2fbfed; continue;
            }
            0x2fbfed => {
                eax = m.read32(0x7844a8);
                edx = m.read32(eax.wrapping_add(0x2f34));
                ecx = (ecx | 0xffffffffu32);
                fa = edx;
                fb = ecx;
                if (fa != fb) { pc = 0x2fc020; continue; }
                pc = 0x2fbfff; continue;
            }
            0x2fbfff => {
                fa = ((((m.read16(eax.wrapping_add(0x2f10)))) as u32) & 0xffffu32);
                fb = ((((((ecx) as u16))) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x2fc020; continue; }
                pc = 0x2fc008; continue;
            }
            0x2fc008 => {
                edx = m.read32(eax.wrapping_add(0x2f7c));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fc020; continue; }
                pc = 0x2fc012; continue;
            }
            0x2fc012 => {
                fa = m.read32(eax.wrapping_add(0x1185c));
                fb = ecx;
                if (fa != fb) { pc = 0x2fc020; continue; }
                pc = 0x2fc01a; continue;
            }
            0x2fc01a => {
                eax = 0x1;
                break 'run;
            }
            0x2fc020 => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
/// 0x002FC0D0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fc0d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fc0d0;
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
        { crate::pose::cover(0x2fc0d0, previous, pc); previous = pc; }
        match pc {
            0x2fc0d0 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x27c), m.read32(eax.wrapping_add(0x27c)).wrapping_add(0x1));
                eax = m.read32(0x7844a8);
                fa = m.read32(eax.wrapping_add(0x27c));
                fb = ((((0x1di32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fc12f; continue; }
                pc = 0x2fc0e9; continue;
            }
            0x2fc0e9 => {
                ecx = m.read32(0x7ba9e0);
                ecx = ((ecx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(0x7ba26c));
                push!(edx);
                call!(0x77470);
                pc = 0x2fc101; continue;
            }
            0x2fc101 => {
                ecx = m.read32(0x7844a8);
                m.write32(ecx.wrapping_add(0x27c), 0);
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x238));
                edx = (edx & 0xfffffffdu32);
                push!(eax);
                m.write32(ecx.wrapping_add(0x238), edx);
                call!(0x78d80);
                pc = 0x2fc12c; continue;
            }
            0x2fc12c => {
                esp = esp.wrapping_add(0x8);
                pc = 0x2fc12f; continue;
            }
            0x2fc12f => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
/// 0x002FC130: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fc130<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fc130;
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
        { crate::pose::cover(0x2fc130, previous, pc); previous = pc; }
        match pc {
            0x2fc130 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(esi);
                push!(ecx);
                call!(0x77470);
                pc = 0x2fc148; continue;
            }
            0x2fc148 => {
                esi = m.read32(eax.wrapping_add(0x1b0));
                ecx = m.read32(eax.wrapping_add(0x2c));
                esp = esp.wrapping_add(0x4);
                edx = 0x3;
                fa = esi;
                fb = edx;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa != fb) { pc = 0x2fc18f; continue; }
                pc = 0x2fc15e; continue;
            }
            0x2fc15e => {
                fa = m.read32(ecx.wrapping_add(0x1b0));
                fb = edx;
                if (fa != fb) { pc = 0x2fc18f; continue; }
                pc = 0x2fc166; continue;
            }
            0x2fc166 => {
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(0x1b4)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475c78)) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x2fc18f; continue; }
                pc = 0x2fc177; continue;
            }
            0x2fc177 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x238), (m.read32(eax.wrapping_add(0x238)) | 0x4));
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x238), (m.read32(eax.wrapping_add(0x238)) & 0xfffffff7u32));
                pc = 0x2fc18f; continue;
            }
            0x2fc18f => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x002FC190: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fc190<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fc190;
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
        { crate::pose::cover(0x2fc190, previous, pc); previous = pc; }
        match pc {
            0x2fc190 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x238), (m.read32(eax.wrapping_add(0x238)) | 0x8));
                eax = m.read32(esp.wrapping_add(0x4));
                push!(eax);
                call!(0x311820);
                pc = 0x2fc1a6; continue;
            }
            0x2fc1a6 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0xa2a);
                call!(0x3d7b0);
                pc = 0x2fc1b4; continue;
            }
            0x2fc1b4 => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(ecx);
                call!(0x3c85d4);
                pc = 0x2fc1c4; continue;
            }
            0x2fc1c4 => {
                push!(0xa29);
                call!(0x3d7b0);
                pc = 0x2fc1ce; continue;
            }
            0x2fc1ce => {
                edx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x10);
                push!(eax);
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x7ba26c));
                push!(edx);
                push!(ecx);
                call!(0x77470);
                pc = 0x2fc1f0; continue;
            }
            0x2fc1f0 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x7a610);
                pc = 0x2fc1f9; continue;
            }
            0x2fc1f9 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FC200: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fc200<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fc200;
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
        { crate::pose::cover(0x2fc200, previous, pc); previous = pc; }
        match pc {
            0x2fc200 => {
                eax = m.read32(0x7844a8);
                fa = ((((m.read8(eax.wrapping_add(0x238)))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                push!(esi);
                esi = m.read32(0x784020);
                push!(edi);
                edi = m.read32(eax.wrapping_add(0x27c));
                if ((fa & fb) == 0) { pc = 0x2fc248; continue; }
                pc = 0x2fc21c; continue;
            }
            0x2fc21c => {
                call!(0x311bc0);
                pc = 0x2fc221; continue;
            }
            0x2fc221 => {
                call!(0x334710);
                pc = 0x2fc226; continue;
            }
            0x2fc226 => {
                push!(esi);
                call!(0x2fc190);
                pc = 0x2fc22c; continue;
            }
            0x2fc22c => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx & 0xffffffefu32);
                m.write32(eax.wrapping_add(0x238), ecx);
                eax = m.read32(0x7844a8);
                pc = 0x2fc248; continue;
            }
            0x2fc248 => {
                eax = m.read32(eax.wrapping_add(0x238));
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2fc259; continue; }
                pc = 0x2fc252; continue;
            }
            0x2fc252 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x2fc130); break 'run;  // direct tail jump
            }
            0x2fc259 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2fc284; continue; }
                pc = 0x2fc25d; continue;
            }
            0x2fc25d => {
                eax = m.read32(0x784020);
                push!(0);
                push!(eax);
                call!(0x338180);
                pc = 0x2fc26a; continue;
            }
            0x2fc26a => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x8);
                ecx = (ecx & 0xfffffffbu32);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x2fc281; continue;
            }
            0x2fc281 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2fc284 => {
                call!(0x61ca0);
                pc = 0x2fc289; continue;
            }
            0x2fc289 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fc281; continue; }
                pc = 0x2fc28d; continue;
            }
            0x2fc28d => {
                push!(edi);
                call!(0x311fb0);
                pc = 0x2fc293; continue;
            }
            0x2fc293 => {
                esp = esp.wrapping_add(0x4);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x2fc0d0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FC2A0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fc2a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fc2a0;
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
        { crate::pose::cover(0x2fc2a0, previous, pc); previous = pc; }
        match pc {
            0x2fc2a0 => {
                ecx = m.read32(0x78400c);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = ecx;
                if (fa == fb) { pc = 0x2fc339; continue; }
                pc = 0x2fc2b0; continue;
            }
            0x2fc2b0 => {
                edx = m.read32(0x7ba9e0);
                xmm0.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                edx = ((edx as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm2 = Xmm::scalar(m.read32(edx.wrapping_add(0x7ba284)));
                xmm3 = Xmm::scalar(m.read32(edx.wrapping_add(0x7ba280)));
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783f20))).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::mulss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783dd4))).to_bits();
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                pc = 0x2fc304; continue;
            }
            0x2fc304 => {
                xmm1 = xmm3;
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                xmm4 = xmm1;
                xmm4.lanes[0] = sse::mulss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0 = xmm2;
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm4.lanes[0] = sse::addss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm4.lanes[0]));
                fa = edx;
                fb = ((((0x7fffffffi32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { eax = ecx; }
                ecx = m.read32(ecx.wrapping_add(0x40));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fc304; continue; }
                pc = 0x2fc339; continue;
            }
            0x2fc339 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x002FC340: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fc340<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fc340;
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
        { crate::pose::cover(0x2fc340, previous, pc); previous = pc; }
        match pc {
            0x2fc340 => {
                xmm0 = Xmm::scalar(m.read32(edi.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                push!(ebx);
                push!(ebp);
                push!(esi);
                esi = m.read32(0x78400c);
                push!(edi);
                ebx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                ebp = 0;
                call!(0x3162b0);
                pc = 0x2fc362; continue;
            }
            0x2fc362 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fc3c0; continue; }
                pc = 0x2fc369; continue;
            }
            0x2fc369 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fc3c0; continue; }
                pc = 0x2fc36d; continue;
            }
            0x2fc36d => {
                edx = m.read32(edi.wrapping_add(0x10));
                pc = 0x2fc370; continue;
            }
            0x2fc370 => {
                eax = m.read32(esi.wrapping_add(0x10));
                xmm0 = Xmm::scalar(m.read32(esi));
                ecx = 0;
                fa = edx;
                fb = eax;
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(edi)) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (((((fa) & (fb))) as u8).count_ones() % 2 == 0) { pc = 0x2fc3af; continue; }
                pc = 0x2fc387; continue;
            }
            0x2fc387 => {
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x4)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(edi.wrapping_add(0x4))) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (((((fa) & (fb))) as u8).count_ones() % 2 == 0) { pc = 0x2fc3af; continue; }
                pc = 0x2fc396; continue;
            }
            0x2fc396 => {
                xmm1 = Xmm::scalar(m.read32(esi.wrapping_add(0x8)));
                xmm0.lanes[0] = (((ebx) as i32) as f32).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (((((fa) & (fb))) as u8).count_ones() % 2 == 0) { pc = 0x2fc3af; continue; }
                pc = 0x2fc3a8; continue;
            }
            0x2fc3a8 => {
                eax = 0x1;
                pc = 0x2fc3b1; continue;
            }
            0x2fc3af => {
                eax = 0;
                pc = 0x2fc3b1; continue;
            }
            0x2fc3b1 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fc3b9; continue; }
                pc = 0x2fc3b5; continue;
            }
            0x2fc3b5 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fc3c6; continue; }
                pc = 0x2fc3b9; continue;
            }
            0x2fc3b9 => {
                esi = m.read32(esi.wrapping_add(0x40));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fc370; continue; }
                pc = 0x2fc3c0; continue;
            }
            0x2fc3c0 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2fc3c6 => {
                eax = esi;
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

/// 0x002FC3D0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fc3d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fc3d0;
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
        { crate::pose::cover(0x2fc3d0, previous, pc); previous = pc; }
        match pc {
            0x2fc3d0 => {
                eax = m.read32(0x7844a8);
                edx = m.read32(eax.wrapping_add(0x21c));
                ecx = m.read32(0x78400c);
                eax = edx;
                cf = (((((eax) != 0))) as i32);
                eax = (((((eax) as i32).wrapping_neg())) as u32);
                eax = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                eax = (eax & 0x6);
                eax = eax.wrapping_add(0xfffffffeu32);
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fc41e; continue; }
                pc = 0x2fc3f1; continue;
            }
            0x2fc3f1 => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fc41e; continue; }
                pc = 0x2fc3f5; continue;
            }
            0x2fc3f5 => {
                edx = m.read32(ecx.wrapping_add(0x10));
                edx = edx.wrapping_add(edx.wrapping_mul(0x4));
                edx = edx.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = m.read32(edx.wrapping_add(0x52952c));
                edx = edx.wrapping_sub(1);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                ecx = m.read32(ecx.wrapping_add(0x40));
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                fa = edx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { eax = edx; }
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fc3f5; continue; }
                pc = 0x2fc41e; continue;
            }
            0x2fc41e => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x002FC420: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fc420<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fc420;
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
        { crate::pose::cover(0x2fc420, previous, pc); previous = pc; }
        match pc {
            0x2fc420 => {
                push!(ebx);
                ebx = 0;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fc460; continue; }
                pc = 0x2fc427; continue;
            }
            0x2fc427 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fc460; continue; }
                pc = 0x2fc42b; continue;
            }
            0x2fc42b => {
                eax = m.read32(esi.wrapping_add(0x10));
                push!(eax);
                call!(0x331970);
                pc = 0x2fc434; continue;
            }
            0x2fc434 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fc460; continue; }
                pc = 0x2fc43b; continue;
            }
            0x2fc43b => {
                ecx = m.read32(edi.wrapping_add(0x10));
                push!(ecx);
                call!(0x331970);
                pc = 0x2fc444; continue;
            }
            0x2fc444 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fc460; continue; }
                pc = 0x2fc44b; continue;
            }
            0x2fc44b => {
                edx = m.read32(esi.wrapping_add(0x10));
                fa = edx;
                fb = m.read32(edi.wrapping_add(0x10));
                if (fa != fb) { pc = 0x2fc460; continue; }
                pc = 0x2fc453; continue;
            }
            0x2fc453 => {
                eax = m.read32(esi.wrapping_add(0xc));
                fa = eax;
                fb = m.read32(edi.wrapping_add(0xc));
                eax = 0x1;
                if (fa == fb) { pc = 0x2fc462; continue; }
                pc = 0x2fc460; continue;
            }
            0x2fc460 => {
                eax = ebx;
                pc = 0x2fc462; continue;
            }
            0x2fc462 => {
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FC470: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fc470<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fc470;
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
        { crate::pose::cover(0x2fc470, previous, pc); previous = pc; }
        match pc {
            0x2fc470 => {
                esp = esp.wrapping_sub(0x8);
                push!(ebx);
                push!(ebp);
                push!(edi);
                call!(0x2fc3d0);
                pc = 0x2fc47b; continue;
            }
            0x2fc47b => {
                edi = m.read32(esp.wrapping_add(0x18));
                ebp = m.read32(edi.wrapping_add(0x44));
                ecx = eax;
                eax = m.read32(edi.wrapping_add(0xc));
                ebx = m.read32(edi.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x2c));
                eax = m.read32(ebx.wrapping_add(0x44));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0xc), ecx);
                m.write32(esp.wrapping_add(0x10), ebx);
                m.write32(esp.wrapping_add(0x18), 0);
                if (fas <= fbs) { pc = 0x2fc672; continue; }
                pc = 0x2fc4a6; continue;
            }
            0x2fc4a6 => {
                push!(esi);
                ebx = ebx.wrapping_add(0x4b);
                pc = 0x2fc4b0; continue;
            }
            0x2fc4ac => {
                ecx = m.read32(esp.wrapping_add(0x10));
                pc = 0x2fc4b0; continue;
            }
            0x2fc4b0 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(ebx.wrapping_add(0x2))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x2fc4e2; continue; }
                pc = 0x2fc4b7; continue;
            }
            0x2fc4b7 => {
                eax = ((m.read8(ebx.wrapping_add(((0x3i32.wrapping_neg()) as u32)))) as u8 as u32);
                eax = eax.wrapping_sub(0x5);
                fa = eax;
                if ((fa == 0)) { pc = 0x2fc4d2; continue; }
                pc = 0x2fc4c0; continue;
            }
            0x2fc4c0 => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0x2fc656; continue; }
                pc = 0x2fc4c7; continue;
            }
            0x2fc4c7 => {
                ebp = (ebp | 0x1000000);
                pc = 0x2fc656; continue;
            }
            0x2fc4d2 => {
                edx = ((m.read8(ebx)) as u8 as u32);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm1.lanes[0] = (((ecx) as i32) as f32).to_bits();
                pc = 0x2fc646; continue;
            }
            0x2fc4e2 => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x3i32)) as u32) & 0xffu32);
                eax = ((m.read8(ebx.wrapping_add(((0x3i32.wrapping_neg()) as u32)))) as u8 as u32);
                if (fa != fb) { pc = 0x2fc50a; continue; }
                pc = 0x2fc4ea; continue;
            }
            0x2fc4ea => {
                eax = eax.wrapping_sub(0x5);
                fa = eax;
                if ((fa == 0)) { pc = 0x2fc501; continue; }
                pc = 0x2fc4ef; continue;
            }
            0x2fc4ef => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0x2fc656; continue; }
                pc = 0x2fc4f6; continue;
            }
            0x2fc4f6 => {
                ebp = (ebp | 0x1000000);
                pc = 0x2fc656; continue;
            }
            0x2fc501 => {
                xmm1.lanes[0] = (((ecx) as i32) as f32).to_bits();
                pc = 0x2fc63f; continue;
            }
            0x2fc50a => {
                eax = eax.wrapping_sub(0x5);
                fa = eax;
                if ((fa == 0)) { pc = 0x2fc5ae; continue; }
                pc = 0x2fc513; continue;
            }
            0x2fc513 => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0x2fc656; continue; }
                pc = 0x2fc51a; continue;
            }
            0x2fc51a => {
                ecx = ((m.read8(ebx)) as u8 as u32);
                edx = m.read32(edi.wrapping_add(0x4));
                eax = m.read32(edi);
                push!(ecx);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(edx);
                push!(eax);
                call!(0x307e60);
                pc = 0x2fc540; continue;
            }
            0x2fc540 => {
                ecx = ((m.read8(ebx.wrapping_add(0x1))) as u8 as u32);
                edx = ((m.read8(ebx.wrapping_add(((0x3i32.wrapping_neg()) as u32)))) as u8 as u32);
                esi = eax;
                eax = ((m.read8(ebx)) as u8 as u32);
                push!(ecx);
                push!(edx);
                edx = (((((f32::from_bits(m.read32(edi.wrapping_add(0x4))) as f64)) as i32)) as u32);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                eax = (((((f32::from_bits(m.read32(edi)) as f64)) as i32)) as u32);
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(ecx);
                push!(edx);
                push!(eax);
                push!(esi);
                call!(0x2f8780);
                pc = 0x2fc56e; continue;
            }
            0x2fc56e => {
                esp = esp.wrapping_add(0x24);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2fc5a3; continue; }
                pc = 0x2fc575; continue;
            }
            0x2fc575 => {
                push!(edi);
                call!(0x331d50);
                pc = 0x2fc57b; continue;
            }
            0x2fc57b => {
                esp = esp.wrapping_add(0x4);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x2fc5a3; continue; }
                pc = 0x2fc582; continue;
            }
            0x2fc582 => {
                ecx = m.read32(edi.wrapping_add(0x10));
                push!(ecx);
                call!(0x331970);
                pc = 0x2fc58b; continue;
            }
            0x2fc58b => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fc656; continue; }
                pc = 0x2fc596; continue;
            }
            0x2fc596 => {
                call!(0x2fc420);
                pc = 0x2fc59b; continue;
            }
            0x2fc59b => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fc656; continue; }
                pc = 0x2fc5a3; continue;
            }
            0x2fc5a3 => {
                ebp = (ebp | 0x1000000);
                pc = 0x2fc656; continue;
            }
            0x2fc5ae => {
                edx = ((m.read8(ebx)) as u8 as u32);
                eax = m.read32(edi.wrapping_add(0x4));
                push!(ecx);
                ecx = m.read32(edi);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                push!(ecx);
                call!(0x307e60);
                pc = 0x2fc5d4; continue;
            }
            0x2fc5d4 => {
                edx = ((m.read8(ebx.wrapping_add(0x1))) as u8 as u32);
                ecx = ((m.read8(ebx)) as u8 as u32);
                esi = eax;
                eax = ((m.read8(ebx.wrapping_add(((0x3i32.wrapping_neg()) as u32)))) as u8 as u32);
                push!(edx);
                push!(eax);
                eax = (((((f32::from_bits(m.read32(edi.wrapping_add(0x4))) as f64)) as i32)) as u32);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                ecx = (((((f32::from_bits(m.read32(edi)) as f64)) as i32)) as u32);
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(edx);
                push!(eax);
                push!(ecx);
                push!(esi);
                call!(0x2f8780);
                pc = 0x2fc602; continue;
            }
            0x2fc602 => {
                esp = esp.wrapping_add(0x24);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2fc639; continue; }
                pc = 0x2fc609; continue;
            }
            0x2fc609 => {
                push!(esi);
                call!(0x331d50);
                pc = 0x2fc60f; continue;
            }
            0x2fc60f => {
                esp = esp.wrapping_add(0x4);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x2fc639; continue; }
                pc = 0x2fc616; continue;
            }
            0x2fc616 => {
                call!(0x2fc420);
                pc = 0x2fc61b; continue;
            }
            0x2fc61b => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fc627; continue; }
                pc = 0x2fc61f; continue;
            }
            0x2fc61f => {
                ebp = (ebp | 0x8000000);
                pc = 0x2fc656; continue;
            }
            0x2fc627 => {
                edx = m.read32(edi.wrapping_add(0x10));
                push!(edx);
                call!(0x331970);
                pc = 0x2fc630; continue;
            }
            0x2fc630 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fc656; continue; }
                pc = 0x2fc637; continue;
            }
            0x2fc637 => {
                pc = 0x2fc650; continue;
            }
            0x2fc639 => {
                xmm1.lanes[0] = (((m.read32(esp.wrapping_add(0x10))) as i32) as f32).to_bits();
                pc = 0x2fc63f; continue;
            }
            0x2fc63f => {
                eax = ((m.read8(ebx)) as u8 as u32);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                pc = 0x2fc646; continue;
            }
            0x2fc646 => {
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x8)))).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x2fc656; continue; }
                pc = 0x2fc650; continue;
            }
            0x2fc650 => {
                ebp = (ebp | 0x2000000);
                pc = 0x2fc656; continue;
            }
            0x2fc656 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                ecx = m.read32(esp.wrapping_add(0x14));
                edx = m.read32(ecx.wrapping_add(0x44));
                eax = eax.wrapping_add(1);
                ebx = ebx.wrapping_add(0x6);
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x1c), eax);
                if (fas < fbs) { pc = 0x2fc4ac; continue; }
                pc = 0x2fc671; continue;
            }
            0x2fc671 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x2fc672; continue;
            }
            0x2fc672 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebp;
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

/// 0x002FD9A0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fd9a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fd9a0;
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
        { crate::pose::cover(0x2fd9a0, previous, pc); previous = pc; }
        match pc {
            0x2fd9a0 => {
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x8));
                fa = ebp;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                push!(esi);
                push!(edi);
                edi = m.read32(0x78400c);
                esi = ebp;
                if (fa != fb) { pc = 0x2fd9ba; continue; }
                pc = 0x2fd9b4; continue;
            }
            0x2fd9b4 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2fd9ba => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fda09; continue; }
                pc = 0x2fd9be; continue;
            }
            0x2fd9be => {
                edi = edi;
                pc = 0x2fd9c0; continue;
            }
            0x2fd9c0 => {
                eax = m.read32(edi.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fda02; continue; }
                pc = 0x2fd9c7; continue;
            }
            0x2fd9c7 => {
                fa = ebx;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fd9e1; continue; }
                pc = 0x2fd9cc; continue;
            }
            0x2fd9cc => {
                fa = ebx;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fd9fb; continue; }
                pc = 0x2fd9d1; continue;
            }
            0x2fd9d1 => {
                ecx = m.read32(eax);
                fa = ecx;
                fb = ebx;
                edx = m.read32(eax.wrapping_add(0x18));
                if (fa == fb) { pc = 0x2fd9f5; continue; }
                pc = 0x2fd9da; continue;
            }
            0x2fd9da => {
                fa = ecx;
                fb = ((((0x8i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fd9fb; continue; }
                pc = 0x2fd9df; continue;
            }
            0x2fd9df => {
                pc = 0x2fd9f5; continue;
            }
            0x2fd9e1 => {
                ecx = m.read32(eax);
                fa = ecx;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                edx = m.read32(eax.wrapping_add(0x18));
                if (fa == fb) { pc = 0x2fd9f0; continue; }
                pc = 0x2fd9eb; continue;
            }
            0x2fd9eb => {
                fa = ecx;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fd9fb; continue; }
                pc = 0x2fd9f0; continue;
            }
            0x2fd9f0 => {
                fa = edx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fd9fb; continue; }
                pc = 0x2fd9f5; continue;
            }
            0x2fd9f5 => {
                fa = esi;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2fd9fb; continue; }
                pc = 0x2fd9f9; continue;
            }
            0x2fd9f9 => {
                esi = edx;
                pc = 0x2fd9fb; continue;
            }
            0x2fd9fb => {
                eax = m.read32(eax.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fd9c7; continue; }
                pc = 0x2fda02; continue;
            }
            0x2fda02 => {
                edi = m.read32(edi.wrapping_add(0x40));
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fd9c0; continue; }
                pc = 0x2fda09; continue;
            }
            0x2fda09 => {
                eax = 0;
                fa = esi;
                fb = ebp;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FDA20: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fda20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fda20;
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
        { crate::pose::cover(0x2fda20, previous, pc); previous = pc; }
        match pc {
            0x2fda20 => {
                eax = m.read32(0x784014);
                push!(0xc77668);
                push!(eax);
                call!(0x32a5b0);
                pc = 0x2fda30; continue;
            }
            0x2fda30 => {
                ecx = m.read32(esp.wrapping_add(0xc));
                ecx = ecx.wrapping_add(0xfffffffeu32);
                esp = esp.wrapping_add(0x8);
                fa = ecx;
                fb = ((((0x11i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x2fdae6; continue; }
                pc = 0x2fda43; continue;
            }
            0x2fda43 => {
                jt = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x2fdaec));
                if (jt == 0x2fda4au32) { pc = 0x2fda4a; continue; }
                if (jt == 0x2fda54u32) { pc = 0x2fda54; continue; }
                if (jt == 0x2fda5eu32) { pc = 0x2fda5e; continue; }
                if (jt == 0x2fda68u32) { pc = 0x2fda68; continue; }
                if (jt == 0x2fda72u32) { pc = 0x2fda72; continue; }
                if (jt == 0x2fda7cu32) { pc = 0x2fda7c; continue; }
                if (jt == 0x2fda86u32) { pc = 0x2fda86; continue; }
                if (jt == 0x2fda92u32) { pc = 0x2fda92; continue; }
                if (jt == 0x2fda9eu32) { pc = 0x2fda9e; continue; }
                if (jt == 0x2fdaaau32) { pc = 0x2fdaaa; continue; }
                if (jt == 0x2fdab6u32) { pc = 0x2fdab6; continue; }
                if (jt == 0x2fdac2u32) { pc = 0x2fdac2; continue; }
                if (jt == 0x2fdaceu32) { pc = 0x2fdace; continue; }
                if (jt == 0x2fdadau32) { pc = 0x2fdada; continue; }
                if (jt == 0x2fdae6u32) { pc = 0x2fdae6; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x2fda4a => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x2);
                break 'run;
            }
            0x2fda54 => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x4);
                break 'run;
            }
            0x2fda5e => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x8);
                break 'run;
            }
            0x2fda68 => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x10);
                break 'run;
            }
            0x2fda72 => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x20);
                break 'run;
            }
            0x2fda7c => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x40);
                break 'run;
            }
            0x2fda86 => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x80);
                break 'run;
            }
            0x2fda92 => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x100);
                break 'run;
            }
            0x2fda9e => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x200);
                break 'run;
            }
            0x2fdaaa => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x400);
                break 'run;
            }
            0x2fdab6 => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x800);
                break 'run;
            }
            0x2fdac2 => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x1000);
                break 'run;
            }
            0x2fdace => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x2000);
                break 'run;
            }
            0x2fdada => {
                eax = m.read32(eax.wrapping_add(0x244));
                eax = (eax & 0x4000);
                break 'run;
            }
            0x2fdae6 => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FDDF0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fddf0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fddf0;
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
        { crate::pose::cover(0x2fddf0, previous, pc); previous = pc; }
        match pc {
            0x2fddf0 => {
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xc));
                push!(esi);
                push!(edi);
                edi = m.read32(0x78400c);
                ebx = 0;
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fde72; continue; }
                pc = 0x2fde04; continue;
            }
            0x2fde04 => {
                esi = m.read32(edi.wrapping_add(0x3c));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fde6b; continue; }
                pc = 0x2fde0b; continue;
            }
            0x2fde0b => {
                pc = 0x2fde10; continue;
            }
            0x2fde10 => {
                eax = m.read32(esi);
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0xc4) as i32)) as u32;
                fa = ((((m.read16(ecx.wrapping_add(0x524f5a)))) as u32) & 0xffffu32);
                fb = ((((0x21i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x2fde63; continue; }
                pc = 0x2fde24; continue;
            }
            0x2fde24 => {
                push!(ebp);
                push!(eax);
                call!(0x2f7880);
                pc = 0x2fde2b; continue;
            }
            0x2fde2b => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fde42; continue; }
                pc = 0x2fde32; continue;
            }
            0x2fde32 => {
                edx = m.read32(esi);
                push!(ebp);
                push!(edx);
                call!(0x2f78d0);
                pc = 0x2fde3b; continue;
            }
            0x2fde3b => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fde63; continue; }
                pc = 0x2fde42; continue;
            }
            0x2fde42 => {
                eax = esi.wrapping_add(0xc);
                edx = m.read32(eax);
                eax = (eax & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x4))) as u32)) & 0xffff);
                esp = esp.wrapping_sub(0x8);
                ecx = esp;
                m.write32(ecx, edx);
                push!(edi);
                write16(m, ecx.wrapping_add(0x4), (((((eax) as u16)) as u32)) as u16);
                call!(0x334950);
                pc = 0x2fde5c; continue;
            }
            0x2fde5c => {
                esp = esp.wrapping_add(0xc);
                ebx = ebx.wrapping_add(eax);
                pc = 0x2fde64; continue;
            }
            0x2fde63 => {
                ebx = ebx.wrapping_add(1);
                pc = 0x2fde64; continue;
            }
            0x2fde64 => {
                esi = m.read32(esi.wrapping_add(0x14));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fde10; continue; }
                pc = 0x2fde6b; continue;
            }
            0x2fde6b => {
                edi = m.read32(edi.wrapping_add(0x40));
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fde04; continue; }
                pc = 0x2fde72; continue;
            }
            0x2fde72 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebx;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FDE80: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fde80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fde80;
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
        { crate::pose::cover(0x2fde80, previous, pc); previous = pc; }
        match pc {
            0x2fde80 => {
                push!(ebp);
                push!(edi);
                edi = m.read32(0x78400c);
                ebp = 0;
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fdf05; continue; }
                pc = 0x2fde8e; continue;
            }
            0x2fde8e => {
                push!(esi);
                pc = 0x2fde90; continue;
            }
            0x2fde90 => {
                eax = m.read32(edi.wrapping_add(0x10));
                esi = eax.wrapping_add(eax.wrapping_mul(0x4));
                esi = esi.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(edi);
                esi = esi.wrapping_add(0x529510);
                call!(0x2fc470);
                pc = 0x2fdea5; continue;
            }
            0x2fdea5 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(esi.wrapping_add(0x12))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x2fdefd; continue; }
                pc = 0x2fdeb0; continue;
            }
            0x2fdeb0 => {
                push!(eax);
                eax = m.read32(edi.wrapping_add(0x10));
                push!(eax);
                call!(0x331920);
                pc = 0x2fdeba; continue;
            }
            0x2fdeba => {
                ecx = m.read32(edi.wrapping_add(0x10));
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fded1; continue; }
                pc = 0x2fdec4; continue;
            }
            0x2fdec4 => {
                push!(ecx);
                call!(0x331dd0);
                pc = 0x2fdeca; continue;
            }
            0x2fdeca => {
                esp = esp.wrapping_add(0x4);
                ebp = ebp.wrapping_add(eax);
                pc = 0x2fdefd; continue;
            }
            0x2fded1 => {
                eax = ecx.wrapping_add(ecx.wrapping_mul(0x4));
                eax = eax.wrapping_shl((((((((0x6i32)) as u32) & 0x1fu32))) as u32 & 31));
                eax = eax.wrapping_add(0x529510);
                eax = (eax & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x12))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2fdeef; continue; }
                pc = 0x2fdee3; continue;
            }
            0x2fdee3 => {
                edx = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x785000));
                ebp = ebp.wrapping_add(m.read32(edx.wrapping_add(0x4)));
                pc = 0x2fdefd; continue;
            }
            0x2fdeef => {
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2fdefd; continue; }
                pc = 0x2fdef3; continue;
            }
            0x2fdef3 => {
                eax = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x789440));
                ebp = ebp.wrapping_add(m.read32(eax.wrapping_add(0x4)));
                pc = 0x2fdefd; continue;
            }
            0x2fdefd => {
                edi = m.read32(edi.wrapping_add(0x40));
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fde90; continue; }
                pc = 0x2fdf04; continue;
            }
            0x2fdf04 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x2fdf05; continue;
            }
            0x2fdf05 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FDF10: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fdf10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fdf10;
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
        { crate::pose::cover(0x2fdf10, previous, pc); previous = pc; }
        match pc {
            0x2fdf10 => {
                eax = 0x514;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FDF20: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fdf20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fdf20;
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
        { crate::pose::cover(0x2fdf20, previous, pc); previous = pc; }
        match pc {
            0x2fdf20 => {
                edx = m.read32(0x78400c);
                eax = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fdf4f; continue; }
                pc = 0x2fdf2c; continue;
            }
            0x2fdf2c => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                pc = 0x2fdf31; continue;
            }
            0x2fdf31 => {
                ecx = m.read32(edx.wrapping_add(0x3c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fdf47; continue; }
                pc = 0x2fdf38; continue;
            }
            0x2fdf38 => {
                fa = ecx;
                fb = esi;
                if (fa == fb) { pc = 0x2fdf45; continue; }
                pc = 0x2fdf3c; continue;
            }
            0x2fdf3c => {
                ecx = m.read32(ecx.wrapping_add(0x14));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fdf38; continue; }
                pc = 0x2fdf43; continue;
            }
            0x2fdf43 => {
                pc = 0x2fdf47; continue;
            }
            0x2fdf45 => {
                eax = edx;
                pc = 0x2fdf47; continue;
            }
            0x2fdf47 => {
                edx = m.read32(edx.wrapping_add(0x40));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fdf31; continue; }
                pc = 0x2fdf4e; continue;
            }
            0x2fdf4e => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x2fdf4f; continue;
            }
            0x2fdf4f => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE0C0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe0c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe0c0;
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
        { crate::pose::cover(0x2fe0c0, previous, pc); previous = pc; }
        match pc {
            0x2fe0c0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                fa = m.read32(eax);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe0de; continue; }
                pc = 0x2fe0c9; continue;
            }
            0x2fe0c9 => {
                push!(0xc77668);
                push!(eax);
                call!(0x32a5b0);
                pc = 0x2fe0d4; continue;
            }
            0x2fe0d4 => {
                eax = m.read32(eax.wrapping_add(0x248));
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x2fe0de => {
                eax = 0;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE0F0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe0f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe0f0;
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
        { crate::pose::cover(0x2fe0f0, previous, pc); previous = pc; }
        match pc {
            0x2fe0f0 => {
                eax = m.read32(esp.wrapping_add(0xc));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe12f; continue; }
                pc = 0x2fe0f8; continue;
            }
            0x2fe0f8 => {
                eax = m.read32(0x7de470);
                push!(eax);
                push!(0);
                call!(0x69930);
                pc = 0x2fe105; continue;
            }
            0x2fe105 => {
                edx = m.read32(eax);
                ecx = m.read32(esp.wrapping_add(0x10));
                m.write32(ecx, edx);
                edx = m.read32(eax.wrapping_add(0x4));
                m.write32(ecx.wrapping_add(0x4), edx);
                edx = m.read32(eax.wrapping_add(0x8));
                m.write32(ecx.wrapping_add(0x8), edx);
                edx = m.read32(eax.wrapping_add(0xc));
                m.write32(ecx.wrapping_add(0xc), edx);
                edx = m.read32(eax.wrapping_add(0x10));
                m.write32(ecx.wrapping_add(0x10), edx);
                eax = m.read32(eax.wrapping_add(0x14));
                esp = esp.wrapping_add(0x8);
                m.write32(ecx.wrapping_add(0x14), eax);
                break 'run;
            }
            0x2fe12f => {
                eax = m.read32(esp.wrapping_add(0x4));
                edx = ((m.read8(eax)) as u8 as u32);
                ecx = m.read32(esp.wrapping_add(0x8));
                m.write32(ecx, edx);
                edx = m.read32(eax.wrapping_add(0x4));
                m.write32(ecx.wrapping_add(0x4), edx);
                edx = m.read32(eax.wrapping_add(0x8));
                m.write32(ecx.wrapping_add(0x8), edx);
                edx = ((m.read8(eax.wrapping_add(0xc))) as u8 as u32);
                m.write32(ecx.wrapping_add(0xc), edx);
                eax = ((m.read8(eax.wrapping_add(0xd))) as u8 as u32);
                m.write32(ecx.wrapping_add(0x10), eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE160: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe160<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe160;
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
        { crate::pose::cover(0x2fe160, previous, pc); previous = pc; }
        match pc {
            0x2fe160 => {
                eax = m.read32(0x78400c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edx = 0xfffffffeu32;
                if (fa == fb) { pc = 0x2fe1c7; continue; }
                pc = 0x2fe16e; continue;
            }
            0x2fe16e => {
                ecx = m.read32(0x7844a8);
                m.write32(ecx.wrapping_add(0x264), 0x3);
                ecx = m.read32(0x7844a8);
                push!(esi);
                pc = 0x2fe190; continue;
                edi = edi;
                pc = 0x2fe190; continue;
            }
            0x2fe190 => {
                xmm0.lanes[0] = (((m.read32(ecx.wrapping_add(0x264))) as i32) as f32).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(eax.wrapping_add(0x8))) as f64);
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x2fe1af; continue; }
                pc = 0x2fe19e; continue;
            }
            0x2fe19e => {
                esi = (((((f32::from_bits(m.read32(eax.wrapping_add(0x8))) as f64)) as i32)) as u32);
                m.write32(ecx.wrapping_add(0x264), esi);
                ecx = m.read32(0x7844a8);
                pc = 0x2fe1af; continue;
            }
            0x2fe1af => {
                eax = m.read32(eax.wrapping_add(0x40));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fe190; continue; }
                pc = 0x2fe1b6; continue;
            }
            0x2fe1b6 => {
                fa = m.read32(ecx.wrapping_add(0x264));
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fas >= fbs) { pc = 0x2fe1dc; continue; }
                pc = 0x2fe1bf; continue;
            }
            0x2fe1bf => {
                m.write32(ecx.wrapping_add(0x264), edx);
                pc = 0x2fe1d6; continue;
            }
            0x2fe1c7 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x264), 0);
                pc = 0x2fe1d6; continue;
            }
            0x2fe1d6 => {
                ecx = m.read32(0x7844a8);
                pc = 0x2fe1dc; continue;
            }
            0x2fe1dc => {
                eax = m.read32(ecx.wrapping_add(0x264));
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x2fe1ee; continue; }
                pc = 0x2fe1e6; continue;
            }
            0x2fe1e6 => {
                eax = eax.wrapping_sub(1);
                m.write32(ecx.wrapping_add(0x268), eax);
                break 'run;
            }
            0x2fe1ee => {
                eax = edx;
                m.write32(ecx.wrapping_add(0x268), eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE420: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe420<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe420;
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
        { crate::pose::cover(0x2fe420, previous, pc); previous = pc; }
        match pc {
            0x2fe420 => {
                call!(0x308590);
                pc = 0x2fe425; continue;
            }
            0x2fe425 => {
                call!(0x308690);
                pc = 0x2fe42a; continue;
            }
            0x2fe42a => {
                tail_to!(0x3089e0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE430: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe430<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe430;
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
        { crate::pose::cover(0x2fe430, previous, pc); previous = pc; }
        match pc {
            0x2fe430 => {
                eax = 0;
                m.write32(0x783ffc, eax);
                m.write32(0x784008, eax);
                m.write32(0x762d68, eax);
                m.write32(0x78400c, eax);
                m.write32(0x784000, eax);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE480: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe480<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe480;
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
        { crate::pose::cover(0x2fe480, previous, pc); previous = pc; }
        match pc {
            0x2fe480 => {
                ecx = m.read32(0x78400c);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                if (fa == fb) { pc = 0x2fe4aa; continue; }
                pc = 0x2fe48f; continue;
            }
            0x2fe48f => {
                pc = 0x2fe490; continue;
            }
            0x2fe490 => {
                eax = m.read32(ecx.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe4a3; continue; }
                pc = 0x2fe497; continue;
            }
            0x2fe497 => {
                fa = esi;
                fb = m.read32(eax.wrapping_add(0x4));
                if (fa == fb) { pc = 0x2fe4d2; continue; }
                pc = 0x2fe49c; continue;
            }
            0x2fe49c => {
                eax = m.read32(eax.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fe497; continue; }
                pc = 0x2fe4a3; continue;
            }
            0x2fe4a3 => {
                ecx = m.read32(ecx.wrapping_add(0x40));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fe490; continue; }
                pc = 0x2fe4aa; continue;
            }
            0x2fe4aa => {
                edx = m.read32(0x7844a8);
                ecx = m.read32(edx.wrapping_add(0x2f34));
                fa = ecx;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                eax = edx.wrapping_add(0x2f34);
                if (fa == fb) { pc = 0x2fe4ce; continue; }
                pc = 0x2fe4c1; continue;
            }
            0x2fe4c1 => {
                fa = esi;
                fb = m.read32(edx.wrapping_add(0x2f38));
                if (fa != fb) { pc = 0x2fe4ce; continue; }
                pc = 0x2fe4c9; continue;
            }
            0x2fe4c9 => {
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fe4d0; continue; }
                pc = 0x2fe4ce; continue;
            }
            0x2fe4ce => {
                eax = 0;
                pc = 0x2fe4d0; continue;
            }
            0x2fe4d0 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2fe4d2 => {
                edx = m.read32(esp.wrapping_add(0x8));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe4d0; continue; }
                pc = 0x2fe4da; continue;
            }
            0x2fe4da => {
                m.write32(edx, ecx);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE520: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe520<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe520;
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
        { crate::pose::cover(0x2fe520, previous, pc); previous = pc; }
        match pc {
            0x2fe520 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                push!(0);
                call!(0x2fe480);
                pc = 0x2fe52d; continue;
            }
            0x2fe52d => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fe55c; continue; }
                pc = 0x2fe534; continue;
            }
            0x2fe534 => {
                push!(esi);
                call!(0x2fe4e0);
                pc = 0x2fe53a; continue;
            }
            0x2fe53a => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x2fe55c; continue; }
                pc = 0x2fe542; continue;
            }
            0x2fe542 => {
                eax = m.read32(0x7844a8);
                fa = esi;
                fb = m.read32(eax.wrapping_add(0x2f38));
                if (fa != fb) { pc = 0x2fe558; continue; }
                pc = 0x2fe54f; continue;
            }
            0x2fe54f => {
                fa = m.read32(eax.wrapping_add(0x2f34));
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe55c; continue; }
                pc = 0x2fe558; continue;
            }
            0x2fe558 => {
                eax = 0;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2fe55c => {
                eax = 0x1;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE570: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe570<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe570;
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
        { crate::pose::cover(0x2fe570, previous, pc); previous = pc; }
        match pc {
            0x2fe570 => {
                edx = m.read32(0x78400c);
                push!(ebx);
                push!(ebp);
                ebp = (ebp | 0xffffffffu32);
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x10));
                if (fa == fb) { pc = 0x2fe5b0; continue; }
                pc = 0x2fe584; continue;
            }
            0x2fe584 => {
                eax = m.read32(edx.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe5a9; continue; }
                pc = 0x2fe58b; continue;
            }
            0x2fe58b => {
                pc = 0x2fe590; continue;
            }
            0x2fe590 => {
                ecx = m.read32(eax.wrapping_add(0x4));
                fa = ecx;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x2fe59d; continue; }
                pc = 0x2fe597; continue;
            }
            0x2fe597 => {
                ecx = ecx.wrapping_sub(1);
                m.write32(eax.wrapping_add(0x4), ecx);
                pc = 0x2fe5a2; continue;
            }
            0x2fe59d => {
                if (fa != fb) { pc = 0x2fe5a2; continue; }
                pc = 0x2fe59f; continue;
            }
            0x2fe59f => {
                m.write32(eax.wrapping_add(0x4), ebp);
                pc = 0x2fe5a2; continue;
            }
            0x2fe5a2 => {
                eax = m.read32(eax.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fe590; continue; }
                pc = 0x2fe5a9; continue;
            }
            0x2fe5a9 => {
                edx = m.read32(edx.wrapping_add(0x40));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fe584; continue; }
                pc = 0x2fe5b0; continue;
            }
            0x2fe5b0 => {
                eax = m.read32(0x7844a8);
                ebx = 0;
                fa = ((((m.read16(eax.wrapping_add(0x306e)))) as u32) & 0xffffu32);
                fb = ((((((ebx) as u16))) as u32) & 0xffffu32);
                if (fa <= fb) { pc = 0x2fe60a; continue; }
                pc = 0x2fe5c0; continue;
            }
            0x2fe5c0 => {
                push!(esi);
                esi = 0;
                pc = 0x2fe5d0; continue;
            }
            0x2fe5d0 => {
                ecx = m.read32(0x7844a8);
                eax = esi.wrapping_add(ecx).wrapping_add(0x307a);
                ecx = 0;
                ecx = (ecx & 0xffff_0000) | ((((m.read16(eax)) as u32)) & 0xffff);
                edx = ((((ecx) as u16)) as u16 as i16 as i32 as u32);
                fa = edx;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x2fe5ef; continue; }
                pc = 0x2fe5e9; continue;
            }
            0x2fe5e9 => {
                ecx = ecx.wrapping_sub(1);
                write16(m, eax, (((((ecx) as u16)) as u32)) as u16);
                pc = 0x2fe5f4; continue;
            }
            0x2fe5ef => {
                if (fa != fb) { pc = 0x2fe5f4; continue; }
                pc = 0x2fe5f1; continue;
            }
            0x2fe5f1 => {
                write16(m, eax, (((((ebp) as u16)) as u32)) as u16);
                pc = 0x2fe5f4; continue;
            }
            0x2fe5f4 => {
                edx = m.read32(0x7844a8);
                eax = ((m.read16(edx.wrapping_add(0x306e))) as u16 as u32);
                ebx = ebx.wrapping_add(1);
                esi = esi.wrapping_add(0xc);
                fa = ebx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2fe5d0; continue; }
                pc = 0x2fe609; continue;
            }
            0x2fe609 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x2fe60a; continue;
            }
            0x2fe60a => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE610: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe610<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe610;
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
        { crate::pose::cover(0x2fe610, previous, pc); previous = pc; }
        match pc {
            0x2fe610 => {
                esp = esp.wrapping_sub(0xc);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x18));
                eax = esp.wrapping_add(0x4);
                push!(esi);
                push!(eax);
                call!(0x2fe480);
                pc = 0x2fe623; continue;
            }
            0x2fe623 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe67f; continue; }
                pc = 0x2fe62a; continue;
            }
            0x2fe62a => {
                ecx = (((((m.read16(eax.wrapping_add(0xc)) as i16)) as i32)) as u32);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                ecx = m.read32(esp.wrapping_add(0x4));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                write16(m, esp.wrapping_add(0x8), (((((edx) as u16)) as u32)) as u16);
                edx = (((((m.read16(eax.wrapping_add(0xe)) as i16)) as i32)) as u32);
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x4)))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                write16(m, esp.wrapping_add(0xa), (((((edx) as u16)) as u32)) as u16);
                eax = (((((m.read16(eax.wrapping_add(0x10)) as i16)) as i32)) as u32);
                esi = m.read32(esp.wrapping_add(0x8));
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                eax = m.read32(esp.wrapping_add(0x14));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ecx.wrapping_add(0x8)))).to_bits();
                edx = eax;
                m.write32(edx, esi);
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                write16(m, edx.wrapping_add(0x4), (((((ecx) as u16)) as u32)) as u16);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            0x2fe67f => {
                push!(esi);
                call!(0x2fe4e0);
                pc = 0x2fe685; continue;
            }
            0x2fe685 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x2fe6d0; continue; }
                pc = 0x2fe68d; continue;
            }
            0x2fe68d => {
                ecx = m.read32(0x7844a8);
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                edx = (edx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x3070))) as u32)) & 0xffff);
                eax = ecx.wrapping_add(eax.wrapping_mul(0x4));
                write16(m, esp.wrapping_add(0x8), (((((edx) as u16)) as u32)) as u16);
                ecx = (ecx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x3072))) as u32)) & 0xffff);
                write16(m, esp.wrapping_add(0xa), (((((ecx) as u16)) as u32)) as u16);
                ecx = m.read32(esp.wrapping_add(0x14));
                eax = (eax & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x3074))) as u32)) & 0xffff);
                esi = m.read32(esp.wrapping_add(0x8));
                edx = ecx;
                m.write32(edx, esi);
                write16(m, edx.wrapping_add(0x4), (((((eax) as u16)) as u32)) as u16);
                eax = ecx;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            0x2fe6d0 => {
                eax = esp.wrapping_add(0x8);
                push!(eax);
                call!(0x331ca0);
                pc = 0x2fe6da; continue;
            }
            0x2fe6da => {
                eax = m.read32(esp.wrapping_add(0x18));
                edx = m.read32(esp.wrapping_add(0xc));
                ecx = eax;
                m.write32(ecx, edx);
                edx = (edx & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x10))) as u32)) & 0xffff);
                esp = esp.wrapping_add(0x4);
                write16(m, ecx.wrapping_add(0x4), (((((edx) as u16)) as u32)) as u16);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE700: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe700<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe700;
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
        { crate::pose::cover(0x2fe700, previous, pc); previous = pc; }
        match pc {
            0x2fe700 => {
                ecx = m.read32(0x78400c);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe72a; continue; }
                pc = 0x2fe70a; continue;
            }
            0x2fe70a => {
                edx = m.read32(esp.wrapping_add(0x4));
                edi = edi;
                pc = 0x2fe710; continue;
            }
            0x2fe710 => {
                eax = m.read32(ecx.wrapping_add(0x3c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe723; continue; }
                pc = 0x2fe717; continue;
            }
            0x2fe717 => {
                fa = m.read32(eax.wrapping_add(0x4));
                fb = edx;
                if (fa == fb) { pc = 0x2fe72c; continue; }
                pc = 0x2fe71c; continue;
            }
            0x2fe71c => {
                eax = m.read32(eax.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fe717; continue; }
                pc = 0x2fe723; continue;
            }
            0x2fe723 => {
                ecx = m.read32(ecx.wrapping_add(0x40));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fe710; continue; }
                pc = 0x2fe72a; continue;
            }
            0x2fe72a => {
                eax = 0;
                pc = 0x2fe72c; continue;
            }
            0x2fe72c => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE730: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe730<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe730;
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
        { crate::pose::cover(0x2fe730, previous, pc); previous = pc; }
        match pc {
            0x2fe730 => {
                push!(esi);
                push!(edi);
                edi = eax;
                eax = m.read32(edi.wrapping_add(0x3c));
                esi = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe74e; continue; }
                pc = 0x2fe73d; continue;
            }
            0x2fe73d => {
                pc = 0x2fe740; continue;
            }
            0x2fe740 => {
                ecx = m.read32(eax.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x2fe765; continue; }
                pc = 0x2fe747; continue;
            }
            0x2fe747 => {
                eax = m.read32(eax.wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fe740; continue; }
                pc = 0x2fe74e; continue;
            }
            0x2fe74e => {
                call!(0x2f9320);
                pc = 0x2fe753; continue;
            }
            0x2fe753 => {
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fe765; continue; }
                pc = 0x2fe758; continue;
            }
            0x2fe758 => {
                push!(edi);
                call!(0x3226c0);
                pc = 0x2fe75e; continue;
            }
            0x2fe75e => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe76d; continue; }
                pc = 0x2fe765; continue;
            }
            0x2fe765 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2fe76d => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE780: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe780<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe780;
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
        { crate::pose::cover(0x2fe780, previous, pc); previous = pc; }
        match pc {
            0x2fe780 => {
                push!(0);
                call!(0x2d3a30);
                pc = 0x2fe787; continue;
            }
            0x2fe787 => {
                ecx = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(eax.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x8));
                eax = ((eax as i32).wrapping_mul((0x54) as i32)) as u32;
                esp = esp.wrapping_add(0x4);
                eax = eax.wrapping_add(0x4fb190);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE7A0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe7a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe7a0;
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
        { crate::pose::cover(0x2fe7a0, previous, pc); previous = pc; }
        match pc {
            0x2fe7a0 => {
                push!(0);
                call!(0x2d3a30);
                pc = 0x2fe7a7; continue;
            }
            0x2fe7a7 => {
                ecx = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(eax.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x8));
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe7e7; continue; }
                pc = 0x2fe7b6; continue;
            }
            0x2fe7b6 => {
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                edx = m.read32(eax.wrapping_add(0x4fa138));
                eax = m.read32(0x4f9bac);
                edx = ((edx as i32).wrapping_mul((0x29c) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(eax).wrapping_add(0x14));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe7e7; continue; }
                pc = 0x2fe7d2; continue;
            }
            0x2fe7d2 => {
                eax = ((eax as i32).wrapping_mul((0xe0) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x4ea484));
                m.write32(esp.wrapping_add(0x4), ecx);
                tail_to!(0x3d7b0); break 'run;  // direct tail jump
            }
            0x2fe7e7 => {
                m.write32(esp.wrapping_add(0x4), 0);
                tail_to!(0x3d7b0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE940: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe940<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe940;
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
        { crate::pose::cover(0x2fe940, previous, pc); previous = pc; }
        match pc {
            0x2fe940 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                push!(esi);
                esi = m.read32(ecx.wrapping_add(0xc));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fe995; continue; }
                pc = 0x2fe94c; continue;
            }
            0x2fe94c => {
                eax = m.read32(esp.wrapping_add(0xc));
                xmm3 = Xmm::default();
                ecx = ecx.wrapping_add(esi.wrapping_mul(0x4)).wrapping_add(0x2c);
                push!(edi);
                pc = 0x2fe958; continue;
            }
            0x2fe958 => {
                edi = (((((m.read16(eax) as i16)) as i32)) as u32);
                edx = m.read32(ecx);
                edx = m.read32(edx.wrapping_add(0x14));
                xmm0.lanes[0] = (((edi) as i32) as f32).to_bits();
                edi = (((((m.read16(eax.wrapping_add(0x2)) as i16)) as i32)) as u32);
                edx = edx.wrapping_sub(1);
                xmm2 = xmm0;
                xmm0.lanes[0] = (((edx) as i32) as f32).to_bits();
                xmm1.lanes[0] = (((edi) as i32) as f32).to_bits();
                xmm4 = xmm3;
                edx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                xmm4.lanes[0] = sse::subss(f32::from_bits(xmm4.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                write16(m, eax, (((((edx) as u16)) as u32)) as u16);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm4.lanes[0])).to_bits();
                ecx = ecx.wrapping_sub(0x4);
                esi = esi.wrapping_sub(1);
                fa = esi;
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                write16(m, eax.wrapping_add(0x2), (((((edx) as u16)) as u32)) as u16);
                if ((fa != 0)) { pc = 0x2fe958; continue; }
                pc = 0x2fe994; continue;
            }
            0x2fe994 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x2fe995; continue;
            }
            0x2fe995 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x002FE9A0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fe9a0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fe9a0;
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
        { crate::pose::cover(0x2fe9a0, previous, pc); previous = pc; }
        match pc {
            0x2fe9a0 => {
                fa = ebx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                edx = (edx & 0xffff_0000) | ((((m.read16(eax.wrapping_add(0x2))) as u32)) & 0xffff);
                push!(edi);
                edi = ecx;
                ecx = (ecx & 0xffff_0000) | ((((m.read16(eax)) as u32)) & 0xffff);
                if (fa == fb) { pc = 0x2fe9d5; continue; }
                pc = 0x2fe9af; continue;
            }
            0x2fe9af => {
                fa = ebx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2fe9fa; continue; }
                pc = 0x2fe9b4; continue;
            }
            0x2fe9b4 => {
                edi = (((((m.read16(edi.wrapping_add(0x18)) as i16)) as i32)) as u32);
                edi = (edi & 0xffff_0000) | ((((((((edi) as u16)) as i32).wrapping_sub(((((edx) as u16)) as i32))) as u32)) & 0xffff);
                edi = edi.wrapping_add(esi);
                write16(m, eax, (((((edi) as u16)) as u32)) as u16);
                write16(m, eax.wrapping_add(0x2), (((((ecx) as u16)) as u32)) as u16);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x6))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0xbi32)) as u32) & 0xffu32);
                if (fa >= fb) { pc = 0x2fe9fa; continue; }
                pc = 0x2fe9cc; continue;
            }
            0x2fe9cc => {
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((((ecx) as u8))) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2fe9fa; continue; }
                pc = 0x2fe9d0; continue;
            }
            0x2fe9d0 => {
                write16(m, eax, (((((m.read16(eax)) as i32).wrapping_sub(0x1i32)) as u32)) as u16);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x2fe9d5 => {
                write16(m, eax, (((((edx) as u16)) as u32)) as u16);
                edx = 0;
                edx = (edx & 0xffff_0000) | ((((m.read16(edi.wrapping_add(0x14))) as u32)) & 0xffff);
                edx = (edx & 0xffff_0000) | ((((((((edx) as u16)) as i32).wrapping_sub(((((ecx) as u16)) as i32))) as u32)) & 0xffff);
                ecx = edx.wrapping_add(esi);
                write16(m, eax.wrapping_add(0x2), (((((ecx) as u16)) as u32)) as u16);
                edx = (edx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x6))) as u32)) & 0xff);
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0xbi32)) as u32) & 0xffu32);
                if (fa >= fb) { pc = 0x2fe9fa; continue; }
                pc = 0x2fe9f0; continue;
            }
            0x2fe9f0 => {
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x2fe9fa; continue; }
                pc = 0x2fe9f5; continue;
            }
            0x2fe9f5 => {
                ecx = ecx.wrapping_sub(1);
                write16(m, eax.wrapping_add(0x2), (((((ecx) as u16)) as u32)) as u16);
                pc = 0x2fe9fa; continue;
            }
            0x2fe9fa => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FEA00: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fea00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fea00;
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
        { crate::pose::cover(0x2fea00, previous, pc); previous = pc; }
        match pc {
            0x2fea00 => {
                eax = m.read32(esp.wrapping_add(0xc));
                esp = esp.wrapping_sub(0x8);
                eax = eax.wrapping_sub(0x2);
                fa = eax;
                if ((fa == 0)) { pc = 0x2fea1e; continue; }
                pc = 0x2fea0c; continue;
            }
            0x2fea0c => {
                xmm1 = Xmm::scalar(m.read32(0x4785c8));
                xmm2 = Xmm::scalar(m.read32(0x478900));
                pc = 0x2fea2e; continue;
            }
            0x2fea1e => {
                xmm1 = Xmm::scalar(m.read32(0x47d1f4));
                xmm2 = Xmm::scalar(m.read32(0x47cff4));
                pc = 0x2fea2e; continue;
            }
            0x2fea2e => {
                eax = m.read32(esp.wrapping_add(0x10));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fea40; continue; }
                pc = 0x2fea36; continue;
            }
            0x2fea36 => {
                eax = (((((f32::from_bits(m.read32(eax.wrapping_add(0x50))) as f64)) as i32)) as u32);
                m.write32(0x762adc, eax);
                pc = 0x2fea40; continue;
            }
            0x2fea40 => {
                xmm0.lanes[0] = (((m.read32(0x762adc)) as i32) as f32).to_bits();
                eax = m.read32(esp.wrapping_add(0x18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                xmm1 = Xmm::scalar(m.read32(0x7ba188));
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba038))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x7ba180))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x4), xmm1.lanes[0]);
                if (fa == fb) { pc = 0x2fea9c; continue; }
                pc = 0x2fea8b; continue;
            }
            0x2fea8b => {
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                edx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                m.write32(eax, ecx);
                m.write32(eax.wrapping_add(0x4), edx);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x2fea9c => {
                xmm0.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                eax = m.read32(0x7844a8);
                edx = m.read32(esp.wrapping_add(0x4));
                m.write32(0x7840a0, xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                m.write32(0x7840a4, xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(0x783de4)) as i32) as f32).to_bits();
                m.write32(0x7840a8, xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(0x783ccc)) as i32) as f32).to_bits();
                m.write32(0x7840ac, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x783dd8));
                m.write32(0x7840b0, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x783cc0));
                m.write32(0x7840b4, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(0x783de0));
                m.write32(0x7840b8, xmm0.lanes[0]);
                ecx = m.read32(eax.wrapping_add(0x264));
                eax = m.read32(esp);
                push!(edx);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x14));
                m.write32(0x7840bc, ecx);
                ecx = (((((f32::from_bits(m.read32(eax.wrapping_add(0x8))) as f64)) as i32)) as u32);
                edx = m.read32(eax.wrapping_add(0x4));
                eax = m.read32(eax);
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x2f7af0);
                pc = 0x2feb3c; continue;
            }
            0x2feb3c => {
                esp = esp.wrapping_add(0x14);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x002FEB50: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2feb50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2feb50;
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
        { crate::pose::cover(0x2feb50, previous, pc); previous = pc; }
        match pc {
            0x2feb50 => {
                eax = m.read32(0x78400c);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2feb78; continue; }
                pc = 0x2feb59; continue;
            }
            0x2feb59 => {
                edx = (edx & 0xffff_ff00) | ((0xe7) & 0xff);
                pc = 0x2feb60; continue;
            }
            0x2feb60 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x14))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x8i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2feb6e; continue; }
                pc = 0x2feb68; continue;
            }
            0x2feb68 => {
                ecx = (ecx & 0xffff_ff00) | (((((((((ecx) as u8)) as i32) | 0x2i32)) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x14), (((((ecx) as u8)) as u32)) as u8);
                pc = 0x2feb6e; continue;
            }
            0x2feb6e => {
                m.write8(eax.wrapping_add(0x14), ((((((m.read8(eax.wrapping_add(0x14))) as i32) & ((((edx) as u8)) as i32))) as u32)) as u8);
                eax = m.read32(eax.wrapping_add(0x40));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2feb60; continue; }
                pc = 0x2feb78; continue;
            }
            0x2feb78 => {
                eax = m.read32(0x78400c);
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2feba2; continue; }
                pc = 0x2feb83; continue;
            }
            0x2feb83 => {
                edx = 0;
                edx = (edx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x14))) as u32)) & 0xff);
                eax = m.read32(eax.wrapping_add(0x40));
                edx = edx.wrapping_shr((((((((0x1i32)) as u32) & 0x1fu32))) as u32 & 31));
                edx = (edx & 0x1);
                ecx = ecx.wrapping_add(edx);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2feb83; continue; }
                pc = 0x2feb96; continue;
            }
            0x2feb96 => {
                eax = m.read32(0x7844a8);
                m.write32(eax.wrapping_add(0x2f9c), ecx);
                break 'run;
            }
            0x2feba2 => {
                edx = m.read32(0x7844a8);
                m.write32(edx.wrapping_add(0x2f9c), ecx);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FEBB0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2febb0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2febb0;
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
        { crate::pose::cover(0x2febb0, previous, pc); previous = pc; }
        match pc {
            0x2febb0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                edx = m.read32(esp.wrapping_add(0x8));
                ecx = eax.wrapping_add(0x18);
                m.write32(ecx, edx);
                edx = m.read32(esp.wrapping_add(0xc));
                m.write32(ecx.wrapping_add(0x4), edx);
                edx = m.read32(esp.wrapping_add(0x10));
                m.write32(ecx.wrapping_add(0x8), edx);
                edx = m.read32(esp.wrapping_add(0x14));
                m.write32(ecx.wrapping_add(0xc), edx);
                edx = m.read32(esp.wrapping_add(0x18));
                push!(eax);
                m.write32(ecx.wrapping_add(0x10), edx);
                call!(0x2fbb40);
                pc = 0x2febdf; continue;
            }
            0x2febdf => {
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FEE60: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2fee60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2fee60;
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
        { crate::pose::cover(0x2fee60, previous, pc); previous = pc; }
        match pc {
            0x2fee60 => {
                esp = esp.wrapping_sub(0x14);
                eax = m.read32(0x7ba9e0);
                xmm1.lanes[0] = (((m.read32(0x783ee0)) as i32) as f32).to_bits();
                eax = ((eax as i32).wrapping_mul((0x1e0) as i32)) as u32;
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba280)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783dd8))).to_bits();
                xmm1.lanes[0] = (((m.read32(0x783dc4)) as i32) as f32).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                edx = m.read32(esp.wrapping_add(0x4));
                push!(ebp);
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x7ba284)));
                eax = m.read32(0x7844a8);
                push!(esi);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783f20))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x783cc0))).to_bits();
                push!(edi);
                push!(ecx);
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                ecx = m.read32(esp.wrapping_add(0x10));
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                xmm0.lanes[0] = (((m.read32(eax.wrapping_add(0x264))) as i32) as f32).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                push!(edx);
                ebp = 0x10;
                call!(0x307e60);
                pc = 0x2feef8; continue;
            }
            0x2feef8 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x25c));
                edi = edx;
                esp = esp.wrapping_add(0xc);
                edi = (edi & 0x20);
                fa = edi;
                esi = eax;
                if ((fa == 0)) { pc = 0x2fef48; continue; }
                pc = 0x2fef10; continue;
            }
            0x2fef10 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x14)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(ecx.wrapping_add(0x2f94))) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (((((fa) & (fb))) as u8).count_ones() % 2 == 0) { pc = 0x2fef36; continue; }
                pc = 0x2fef23; continue;
            }
            0x2fef23 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(ecx.wrapping_add(0x2f98))) as f64);
                eax = (((eax & 0xffff00ffu32)) | ((((((if ((((fca == fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x40u32 } else { 0x0u32 }) | (if ((((fca != fca) || (fcb != fcb)))) { 0x4u32 } else { 0x0u32 })) | (if ((((fca < fcb) || (((fca != fca) || (fcb != fcb)))))) { 0x1u32 } else { 0x0u32 })) | 0x2u32)).wrapping_shl((0x8 & 31))));
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x44i32)) as u32) & 0xffu32);
                if (!(((((fa) & (fb))) as u8).count_ones() % 2 == 0)) { pc = 0x2fef48; continue; }
                pc = 0x2fef36; continue;
            }
            0x2fef36 => {
                ebp = 0x8;
                m.write32(0x784010, ebp);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x2fef48 => {
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2fefad; continue; }
                pc = 0x2fef4d; continue;
            }
            0x2fef4d => {
                eax = ((m.read8(0x783cc8)) as u8 as u32);
                eax = eax.wrapping_sub(1);
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x2ff9ad; continue; }
                pc = 0x2fef5e; continue;
            }
            0x2fef5e => {
                jt = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x2ff9bc));
                if (jt == 0x2fef65u32) { pc = 0x2fef65; continue; }
                if (jt == 0x2fef77u32) { pc = 0x2fef77; continue; }
                if (jt == 0x2fef89u32) { pc = 0x2fef89; continue; }
                if (jt == 0x2fef9bu32) { pc = 0x2fef9b; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x2fef65 => {
                ebp = 0x3;
                m.write32(0x784010, ebp);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x2fef77 => {
                ebp = 0x4;
                m.write32(0x784010, ebp);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x2fef89 => {
                ebp = 0x6;
                m.write32(0x784010, ebp);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x2fef9b => {
                ebp = 0x5;
                m.write32(0x784010, ebp);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x2fefad => {
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2fefc4; continue; }
                pc = 0x2fefb2; continue;
            }
            0x2fefb2 => {
                ebp = 0x7;
                m.write32(0x784010, ebp);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x2fefc4 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff007; continue; }
                pc = 0x2fefc8; continue;
            }
            0x2fefc8 => {
                eax = m.read32(ecx.wrapping_add(0x2f9c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff007; continue; }
                pc = 0x2fefd2; continue;
            }
            0x2fefd2 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff007; continue; }
                pc = 0x2fefd6; continue;
            }
            0x2fefd6 => {
                fa = m.read32(ecx.wrapping_add(0x1185c));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff007; continue; }
                pc = 0x2fefdf; continue;
            }
            0x2fefdf => {
                eax = m.read32(ecx.wrapping_add(0x254));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2fefee; continue; }
                pc = 0x2fefe9; continue;
            }
            0x2fefe9 => {
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff007; continue; }
                pc = 0x2fefee; continue;
            }
            0x2fefee => {
                push!(0);
                push!(0);
                push!(0x1a29);
                call!(0x3d7b0);
                pc = 0x2feffc; continue;
            }
            0x2feffc => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                pc = 0x2ff9a5; continue;
            }
            0x2ff007 => {
                eax = m.read32(ecx.wrapping_add(0x254));
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff44c; continue; }
                pc = 0x2ff016; continue;
            }
            0x2ff016 => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff44c; continue; }
                pc = 0x2ff01f; continue;
            }
            0x2ff01f => {
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff0fa; continue; }
                pc = 0x2ff028; continue;
            }
            0x2ff028 => {
                fa = edx;
                fb = ((((0x1100000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x2ff0bc; continue; }
                pc = 0x2ff034; continue;
            }
            0x2ff034 => {
                push!(0);
                push!(0);
                push!(0xa9e);
                call!(0x3d7b0);
                pc = 0x2ff042; continue;
            }
            0x2ff042 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                call!(0x74a30);
                pc = 0x2ff04d; continue;
            }
            0x2ff04d => {
                esp = esp.wrapping_add(0x10);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff9ad; continue; }
                pc = 0x2ff058; continue;
            }
            0x2ff058 => {
                push!(0);
                push!(0);
                push!(0xa99);
                call!(0x3d7b0);
                pc = 0x2ff066; continue;
            }
            0x2ff066 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                call!(0x74a30);
                pc = 0x2ff071; continue;
            }
            0x2ff071 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xa9c);
                call!(0x3d7b0);
                pc = 0x2ff082; continue;
            }
            0x2ff082 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x2);
                call!(0x74a30);
                pc = 0x2ff08d; continue;
            }
            0x2ff08d => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f18));
                esp = esp.wrapping_add(0x10);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff9ad; continue; }
                pc = 0x2ff0a3; continue;
            }
            0x2ff0a3 => {
                push!(0);
                push!(0);
                push!(0x9c0);
                call!(0x3d7b0);
                pc = 0x2ff0b1; continue;
            }
            0x2ff0b1 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                pc = 0x2ff9a5; continue;
            }
            0x2ff0bc => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebp = 0xf;
                if (fa == fb) { pc = 0x2ff0e1; continue; }
                pc = 0x2ff0c5; continue;
            }
            0x2ff0c5 => {
                push!(0);
                push!(0);
                push!(0xa9d);
                call!(0x3d7b0);
                pc = 0x2ff0d3; continue;
            }
            0x2ff0d3 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                call!(0x74a30);
                pc = 0x2ff0de; continue;
            }
            0x2ff0de => {
                esp = esp.wrapping_add(0x10);
                pc = 0x2ff0e1; continue;
            }
            0x2ff0e1 => {
                push!(0);
                push!(0);
                push!(0xa9a);
                call!(0x3d7b0);
                pc = 0x2ff0ef; continue;
            }
            0x2ff0ef => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                pc = 0x2ff9a5; continue;
            }
            0x2ff0fa => {
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff301; continue; }
                pc = 0x2ff103; continue;
            }
            0x2ff103 => {
                eax = (eax & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0x2f10))) as u32)) & 0xffff);
                fa = ((((((eax) as u16))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas < fbs) { pc = 0x2ff1bb; continue; }
                pc = 0x2ff113; continue;
            }
            0x2ff113 => {
                edx = ((((eax) as u16)) as u16 as i16 as i32 as u32);
                edx = ((edx as i32).wrapping_mul((0xe4) as i32)) as u32;
                push!(0);
                push!(0);
                push!(0xa9f);
                esi = edx.wrapping_add(ecx).wrapping_add(0x288);
                call!(0x3d7b0);
                pc = 0x2ff131; continue;
            }
            0x2ff131 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                call!(0x74a30);
                pc = 0x2ff13c; continue;
            }
            0x2ff13c => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xaa1);
                call!(0x3d7b0);
                pc = 0x2ff14d; continue;
            }
            0x2ff14d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                call!(0x74a30);
                pc = 0x2ff158; continue;
            }
            0x2ff158 => {
                eax = m.read32(esi.wrapping_add(0x34));
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff16c; continue; }
                pc = 0x2ff162; continue;
            }
            0x2ff162 => {
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff16c; continue; }
                pc = 0x2ff167; continue;
            }
            0x2ff167 => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff1b3; continue; }
                pc = 0x2ff16c; continue;
            }
            0x2ff16c => {
                ecx = m.read32(0x7844a8);
                fa = ((((m.read8(ecx.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2ff1b9; continue; }
                pc = 0x2ff17b; continue;
            }
            0x2ff17b => {
                push!(0);
                push!(0);
                push!(0xaa0);
                call!(0x3d7b0);
                pc = 0x2ff189; continue;
            }
            0x2ff189 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x2);
                call!(0x74a30);
                pc = 0x2ff194; continue;
            }
            0x2ff194 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xaa0);
                call!(0x3d7b0);
                pc = 0x2ff1a5; continue;
            }
            0x2ff1a5 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a30);
                pc = 0x2ff1b0; continue;
            }
            0x2ff1b0 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x2ff1b3; continue;
            }
            0x2ff1b3 => {
                ecx = m.read32(0x7844a8);
                pc = 0x2ff1b9; continue;
            }
            0x2ff1b9 => {
                ebp = 0;
                pc = 0x2ff1bb; continue;
            }
            0x2ff1bb => {
                fa = ((((m.read8(ecx.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x2ff1d9; continue; }
                pc = 0x2ff1c4; continue;
            }
            0x2ff1c4 => {
                push!(0);
                ebp = 0x1;
                call!(0x748b0);
                pc = 0x2ff1d0; continue;
            }
            0x2ff1d0 => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                pc = 0x2ff1d9; continue;
            }
            0x2ff1d9 => {
                fa = ((((m.read16(ecx.wrapping_add(0x2f10)))) as u32) & 0xffffu32);
                fb = ((((0i32)) as u32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas >= fbs) { pc = 0x2ff9ad; continue; }
                pc = 0x2ff1e7; continue;
            }
            0x2ff1e7 => {
                eax = m.read32(ecx.wrapping_add(0x2f20));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(0);
                push!(0);
                if (fas < fbs) { pc = 0x2ff2aa; continue; }
                pc = 0x2ff1f9; continue;
            }
            0x2ff1f9 => {
                eax = (((((eax as i32)).wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                eax = ((eax as i32).wrapping_mul((0xe4) as i32)) as u32;
                push!(0xa9f);
                esi = eax.wrapping_add(ecx).wrapping_add(0x288);
                call!(0x3d7b0);
                pc = 0x2ff213; continue;
            }
            0x2ff213 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                call!(0x74a30);
                pc = 0x2ff21e; continue;
            }
            0x2ff21e => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xaad);
                call!(0x3d7b0);
                pc = 0x2ff22f; continue;
            }
            0x2ff22f => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                call!(0x74a30);
                pc = 0x2ff23a; continue;
            }
            0x2ff23a => {
                push!(esi);
                call!(0x31d2d0);
                pc = 0x2ff240; continue;
            }
            0x2ff240 => {
                esp = esp.wrapping_add(0x14);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff963; continue; }
                pc = 0x2ff24b; continue;
            }
            0x2ff24b => {
                push!(esi);
                call!(0x31d2c0);
                pc = 0x2ff251; continue;
            }
            0x2ff251 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff272; continue; }
                pc = 0x2ff258; continue;
            }
            0x2ff258 => {
                push!(eax);
                push!(eax);
                push!(0xc1b);
                call!(0x3d7b0);
                pc = 0x2ff264; continue;
            }
            0x2ff264 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a30);
                pc = 0x2ff26f; continue;
            }
            0x2ff26f => {
                esp = esp.wrapping_add(0x10);
                pc = 0x2ff272; continue;
            }
            0x2ff272 => {
                fa = m.read32(esi.wrapping_add(0x58));
                fb = ((((0x8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x2ff97f; continue; }
                pc = 0x2ff27c; continue;
            }
            0x2ff27c => {
                push!(0);
                push!(0);
                push!(0xc1c);
                call!(0x3d7b0);
                pc = 0x2ff28a; continue;
            }
            0x2ff28a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x2);
                call!(0x74a30);
                pc = 0x2ff295; continue;
            }
            0x2ff295 => {
                ebp = 0x2;
                esp = esp.wrapping_add(0x10);
                m.write32(0x784010, ebp);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x2ff2aa => {
                eax = m.read32(ecx.wrapping_add(0x2f34));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2ff2f7; continue; }
                pc = 0x2ff2b4; continue;
            }
            0x2ff2b4 => {
                push!(0xa9f);
                call!(0x3d7b0);
                pc = 0x2ff2be; continue;
            }
            0x2ff2be => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                call!(0x74a30);
                pc = 0x2ff2c9; continue;
            }
            0x2ff2c9 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x2ff2cc; continue;
            }
            0x2ff2cc => {
                push!(0);
                push!(0);
                push!(0xaa1);
                pc = 0x2ff2d5; continue;
            }
            0x2ff2d5 => {
                call!(0x3d7b0);
                pc = 0x2ff2da; continue;
            }
            0x2ff2da => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                call!(0x74a30);
                pc = 0x2ff2e5; continue;
            }
            0x2ff2e5 => {
                esp = esp.wrapping_add(0x10);
                ebp = 0;
                m.write32(0x784010, ebp);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x2ff2f7 => {
                push!(0xb6d);
                pc = 0x2ff99a; continue;
            }
            0x2ff301 => {
                eax = m.read32(ecx.wrapping_add(0x2f7c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff38f; continue; }
                pc = 0x2ff30f; continue;
            }
            0x2ff30f => {
                push!(0);
                push!(0);
                push!(0xa9f);
                call!(0x3d7b0);
                pc = 0x2ff31d; continue;
            }
            0x2ff31d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                call!(0x74a30);
                pc = 0x2ff328; continue;
            }
            0x2ff328 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xaac);
                call!(0x3d7b0);
                pc = 0x2ff339; continue;
            }
            0x2ff339 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x2);
                call!(0x74a30);
                pc = 0x2ff344; continue;
            }
            0x2ff344 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xaa0);
                call!(0x3d7b0);
                pc = 0x2ff355; continue;
            }
            0x2ff355 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a30);
                pc = 0x2ff360; continue;
            }
            0x2ff360 => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x25c));
                esp = esp.wrapping_add(0x10);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2ff37d; continue; }
                pc = 0x2ff372; continue;
            }
            0x2ff372 => {
                fa = eax;
                fb = ((((0x410000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x2ff2cc; continue; }
                pc = 0x2ff37d; continue;
            }
            0x2ff37d => {
                ebp = 0x1;
                m.write32(0x784010, ebp);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x2ff38f => {
                eax = m.read32(ecx.wrapping_add(0x2f9c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff3b4; continue; }
                pc = 0x2ff399; continue;
            }
            0x2ff399 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff3b4; continue; }
                pc = 0x2ff39d; continue;
            }
            0x2ff39d => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff43e; continue; }
                pc = 0x2ff3a5; continue;
            }
            0x2ff3a5 => {
                fa = ((((m.read8(esi.wrapping_add(0x14)))) as u32) & 0xffu32);
                fb = ((((0x2i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2ff3b4; continue; }
                pc = 0x2ff3ab; continue;
            }
            0x2ff3ab => {
                push!(edi);
                push!(edi);
                push!(0xaa2);
                pc = 0x2ff3c5; continue;
            }
            0x2ff3b4 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff43e; continue; }
                pc = 0x2ff3bc; continue;
            }
            0x2ff3bc => {
                push!(0);
                push!(0);
                push!(0xaa3);
                pc = 0x2ff3c5; continue;
            }
            0x2ff3c5 => {
                call!(0x3d7b0);
                pc = 0x2ff3ca; continue;
            }
            0x2ff3ca => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                call!(0x74a30);
                pc = 0x2ff3d5; continue;
            }
            0x2ff3d5 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xaa4);
                call!(0x3d7b0);
                pc = 0x2ff3e6; continue;
            }
            0x2ff3e6 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a30);
                pc = 0x2ff3f1; continue;
            }
            0x2ff3f1 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xaa5);
                call!(0x3d7b0);
                pc = 0x2ff402; continue;
            }
            0x2ff402 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                call!(0x74a30);
                pc = 0x2ff40d; continue;
            }
            0x2ff40d => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xaa6);
                call!(0x3d7b0);
                pc = 0x2ff41e; continue;
            }
            0x2ff41e => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x2);
                call!(0x74a30);
                pc = 0x2ff429; continue;
            }
            0x2ff429 => {
                ebp = 0x2;
                esp = esp.wrapping_add(0x10);
                m.write32(0x784010, ebp);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x2ff43e => {
                push!(0);
                push!(0);
                push!(0xaa7);
                pc = 0x2ff99a; continue;
            }
            0x2ff44c => {
                eax = m.read32(ecx.wrapping_add(0x2f34));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2ff53e; continue; }
                pc = 0x2ff45a; continue;
            }
            0x2ff45a => {
                push!(0);
                push!(0);
                push!(0xa9f);
                call!(0x3d7b0);
                pc = 0x2ff468; continue;
            }
            0x2ff468 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                call!(0x74a30);
                pc = 0x2ff473; continue;
            }
            0x2ff473 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x2f34));
                edx = ((edx as i32).wrapping_mul((0xc4) as i32)) as u32;
                esp = esp.wrapping_add(0x10);
                fa = ((((m.read16(edx.wrapping_add(0x524f5a)))) as u32) & 0xffffu32);
                fb = ((((0x22i32)) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x2ff4bc; continue; }
                pc = 0x2ff492; continue;
            }
            0x2ff492 => {
                eax = m.read32(ecx.wrapping_add(0x2f70));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff4a6; continue; }
                pc = 0x2ff49c; continue;
            }
            0x2ff49c => {
                ebp = 0x1;
                pc = 0x2ff53e; continue;
            }
            0x2ff4a6 => {
                push!(0);
                push!(0);
                push!(0xaa1);
                call!(0x3d7b0);
                pc = 0x2ff4b4; continue;
            }
            0x2ff4b4 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                pc = 0x2ff52e; continue;
            }
            0x2ff4bc => {
                push!(0);
                push!(0);
                push!(0xaa1);
                call!(0x3d7b0);
                pc = 0x2ff4ca; continue;
            }
            0x2ff4ca => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                call!(0x74a30);
                pc = 0x2ff4d5; continue;
            }
            0x2ff4d5 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f34));
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                edx = (edx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x525010))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x10);
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0x1i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2ff53c; continue; }
                pc = 0x2ff4f5; continue;
            }
            0x2ff4f5 => {
                fa = ((((m.read8(ecx.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2ff53c; continue; }
                pc = 0x2ff4fe; continue;
            }
            0x2ff4fe => {
                push!(0);
                push!(0);
                push!(0xaa0);
                call!(0x3d7b0);
                pc = 0x2ff50c; continue;
            }
            0x2ff50c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x2);
                call!(0x74a30);
                pc = 0x2ff517; continue;
            }
            0x2ff517 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xaa0);
                call!(0x3d7b0);
                pc = 0x2ff528; continue;
            }
            0x2ff528 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                pc = 0x2ff52e; continue;
            }
            0x2ff52e => {
                call!(0x74a30);
                pc = 0x2ff533; continue;
            }
            0x2ff533 => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x10);
                pc = 0x2ff53c; continue;
            }
            0x2ff53c => {
                ebp = 0;
                pc = 0x2ff53e; continue;
            }
            0x2ff53e => {
                eax = m.read32(ecx.wrapping_add(0x25c));
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2ff54f; continue; }
                pc = 0x2ff548; continue;
            }
            0x2ff548 => {
                fa = eax;
                fb = ((((0x410000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x2ff564; continue; }
                pc = 0x2ff54f; continue;
            }
            0x2ff54f => {
                push!(0);
                ebp = 0x1;
                call!(0x748b0);
                pc = 0x2ff55b; continue;
            }
            0x2ff55b => {
                ecx = m.read32(0x7844a8);
                esp = esp.wrapping_add(0x4);
                pc = 0x2ff564; continue;
            }
            0x2ff564 => {
                eax = m.read32(ecx.wrapping_add(0x2f34));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x2ff9ad; continue; }
                pc = 0x2ff572; continue;
            }
            0x2ff572 => {
                edx = m.read32(ecx.wrapping_add(0x254));
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                esi = 0x8;
                edi = 0x21;
                if (fa != fb) { pc = 0x2ff64f; continue; }
                pc = 0x2ff58b; continue;
            }
            0x2ff58b => {
                fa = m.read32(ecx.wrapping_add(0x2f78));
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x2ff5b7; continue; }
                pc = 0x2ff594; continue;
            }
            0x2ff594 => {
                eax = m.read32(ecx.wrapping_add(0x2f1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff64f; continue; }
                pc = 0x2ff5a2; continue;
            }
            0x2ff5a2 => {
                eax = m.read32(eax);
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                fa = ((((m.read16(eax.wrapping_add(0x524f5a)))) as u32) & 0xffffu32);
                fb = ((((((edi) as u16))) as u32) & 0xffffu32);
                if (fa != fb) { pc = 0x2ff64f; continue; }
                pc = 0x2ff5b7; continue;
            }
            0x2ff5b7 => {
                fa = m.read32(ecx.wrapping_add(0x1185c));
                fb = esi;
                if (fa == fb) { pc = 0x2ff64f; continue; }
                pc = 0x2ff5c3; continue;
            }
            0x2ff5c3 => {
                push!(0);
                push!(0);
                push!(0xa9c);
                call!(0x3d7b0);
                pc = 0x2ff5d1; continue;
            }
            0x2ff5d1 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x2);
                call!(0x74a30);
                pc = 0x2ff5dc; continue;
            }
            0x2ff5dc => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xa9f);
                call!(0x3d7b0);
                pc = 0x2ff5ed; continue;
            }
            0x2ff5ed => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                call!(0x74a30);
                pc = 0x2ff5f8; continue;
            }
            0x2ff5f8 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f78));
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x2ff641; continue; }
                pc = 0x2ff60c; continue;
            }
            0x2ff60c => {
                edx = eax.wrapping_add(eax.wrapping_mul(0x2));
                eax = ((m.read8(ecx.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x3077))) as u8 as u32);
                push!(eax);
                call!(0x2f8020);
                pc = 0x2ff61d; continue;
            }
            0x2ff61d => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0x47i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff641; continue; }
                pc = 0x2ff625; continue;
            }
            0x2ff625 => {
                push!(0);
                push!(0);
                push!(0x9c0);
                call!(0x3d7b0);
                pc = 0x2ff633; continue;
            }
            0x2ff633 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a30);
                pc = 0x2ff63e; continue;
            }
            0x2ff63e => {
                esp = esp.wrapping_add(0x10);
                pc = 0x2ff641; continue;
            }
            0x2ff641 => {
                push!(0);
                push!(0);
                push!(0xaab);
                pc = 0x2ff2d5; continue;
            }
            0x2ff64f => {
                fa = edx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff828; continue; }
                pc = 0x2ff658; continue;
            }
            0x2ff658 => {
                eax = m.read32(ecx.wrapping_add(0x1185c));
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff6b4; continue; }
                pc = 0x2ff663; continue;
            }
            0x2ff663 => {
                push!(0);
                push!(0);
                push!(0x1a27);
                call!(0x3d7b0);
                pc = 0x2ff671; continue;
            }
            0x2ff671 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a30);
                pc = 0x2ff67c; continue;
            }
            0x2ff67c => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f1c));
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff9ad; continue; }
                pc = 0x2ff693; continue;
            }
            0x2ff693 => {
                edx = m.read32(eax);
                push!(edx);
                call!(0x31ff30);
                pc = 0x2ff69b; continue;
            }
            0x2ff69b => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff9ad; continue; }
                pc = 0x2ff6a6; continue;
            }
            0x2ff6a6 => {
                push!(0);
                push!(0);
                push!(0xa97);
                pc = 0x2ff99a; continue;
            }
            0x2ff6b4 => {
                fa = eax;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff6f7; continue; }
                pc = 0x2ff6b9; continue;
            }
            0x2ff6b9 => {
                push!(0);
                push!(0);
                push!(0x1a27);
                call!(0x3d7b0);
                pc = 0x2ff6c7; continue;
            }
            0x2ff6c7 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a30);
                pc = 0x2ff6d2; continue;
            }
            0x2ff6d2 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f78));
                esp = esp.wrapping_add(0x10);
                fa = ecx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x2ff9ad; continue; }
                pc = 0x2ff6e9; continue;
            }
            0x2ff6e9 => {
                push!(0);
                push!(0);
                push!(0xa97);
                pc = 0x2ff99a; continue;
            }
            0x2ff6f7 => {
                fa = eax;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff7ac; continue; }
                pc = 0x2ff700; continue;
            }
            0x2ff700 => {
                edx = m.read32(ecx.wrapping_add(0x11858));
                eax = (((((m.read16(edx.wrapping_add(0xc)) as i16)) as i32)) as u32);
                eax = ((eax as i32).wrapping_mul((0x8c) as i32)) as u32;
                esi = eax.wrapping_add(ecx).wrapping_add(0xdd14);
                ecx = esp.wrapping_add(0x18);
                push!(ecx);
                call!(0x331ab0);
                pc = 0x2ff721; continue;
            }
            0x2ff721 => {
                esp = esp.wrapping_add(0x4);
                push!(0);
                push!(0);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x2ff732; continue;
            }
            0x2ff732 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a30);
                pc = 0x2ff73d; continue;
            }
            0x2ff73d => {
                eax = m.read32(esp.wrapping_add(0x28));
                ecx = (ecx & 0xffff_0000) | ((((m.read16(esp.wrapping_add(0x2c))) as u32)) & 0xffff);
                esp = esp.wrapping_add(0x8);
                edx = esp;
                m.write32(edx, eax);
                write16(m, edx.wrapping_add(0x4), (((((ecx) as u16)) as u32)) as u16);
                call!(0x2fb9d0);
                pc = 0x2ff756; continue;
            }
            0x2ff756 => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff788; continue; }
                pc = 0x2ff75d; continue;
            }
            0x2ff75d => {
                edx = m.read32(0x7844a8);
                fa = ((((m.read8(edx.wrapping_add(0x25c)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x2ff788; continue; }
                pc = 0x2ff76c; continue;
            }
            0x2ff76c => {
                push!(0);
                push!(0);
                push!(0xa97);
                call!(0x3d7b0);
                pc = 0x2ff77a; continue;
            }
            0x2ff77a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                call!(0x74a30);
                pc = 0x2ff785; continue;
            }
            0x2ff785 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x2ff788; continue;
            }
            0x2ff788 => {
                eax = m.read32(esi.wrapping_add(0x20));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x2ff9ad; continue; }
                pc = 0x2ff793; continue;
            }
            0x2ff793 => {
                push!(0);
                push!(0);
                push!(0x1a29);
                call!(0x3d7b0);
                pc = 0x2ff7a1; continue;
            }
            0x2ff7a1 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x2);
                pc = 0x2ff9a5; continue;
            }
            0x2ff7ac => {
                fa = eax;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                push!(0);
                push!(0);
                if (fa != fb) { pc = 0x2ff81e; continue; }
                pc = 0x2ff7b5; continue;
            }
            0x2ff7b5 => {
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x2ff7bf; continue;
            }
            0x2ff7bf => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a30);
                pc = 0x2ff7ca; continue;
            }
            0x2ff7ca => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f18));
                esp = esp.wrapping_add(0x10);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff7f8; continue; }
                pc = 0x2ff7dc; continue;
            }
            0x2ff7dc => {
                push!(0);
                push!(0);
                push!(0xa97);
                call!(0x3d7b0);
                pc = 0x2ff7ea; continue;
            }
            0x2ff7ea => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                call!(0x74a30);
                pc = 0x2ff7f5; continue;
            }
            0x2ff7f5 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x2ff7f8; continue;
            }
            0x2ff7f8 => {
                call!(0x2fbb10);
                pc = 0x2ff7fd; continue;
            }
            0x2ff7fd => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff9ad; continue; }
                pc = 0x2ff805; continue;
            }
            0x2ff805 => {
                push!(0);
                push!(0);
                push!(0x1a29);
                call!(0x3d7b0);
                pc = 0x2ff813; continue;
            }
            0x2ff813 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x2);
                pc = 0x2ff9a5; continue;
            }
            0x2ff81e => {
                push!(0xbd2);
                pc = 0x2ff99a; continue;
            }
            0x2ff828 => {
                fa = m.read32(ecx.wrapping_add(0x1185c));
                fb = esi;
                if (fa != fb) { pc = 0x2ff8b8; continue; }
                pc = 0x2ff834; continue;
            }
            0x2ff834 => {
                push!(0);
                push!(0);
                push!(0x1a27);
                call!(0x3d7b0);
                pc = 0x2ff842; continue;
            }
            0x2ff842 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a30);
                pc = 0x2ff84d; continue;
            }
            0x2ff84d => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f1c));
                esp = esp.wrapping_add(0x10);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff869; continue; }
                pc = 0x2ff860; continue;
            }
            0x2ff860 => {
                call!(0x324da0);
                pc = 0x2ff865; continue;
            }
            0x2ff865 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff88a; continue; }
                pc = 0x2ff869; continue;
            }
            0x2ff869 => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x2f78));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2ff37d; continue; }
                pc = 0x2ff87d; continue;
            }
            0x2ff87d => {
                call!(0x324e00);
                pc = 0x2ff882; continue;
            }
            0x2ff882 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff37d; continue; }
                pc = 0x2ff88a; continue;
            }
            0x2ff88a => {
                push!(0);
                push!(0);
                push!(0xa97);
                call!(0x3d7b0);
                pc = 0x2ff898; continue;
            }
            0x2ff898 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                call!(0x74a30);
                pc = 0x2ff8a3; continue;
            }
            0x2ff8a3 => {
                ebp = 0x10;
                esp = esp.wrapping_add(0x10);
                m.write32(0x784010, ebp);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x2ff8b8 => {
                ecx = m.read32(ecx.wrapping_add(0x2f1c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff991; continue; }
                pc = 0x2ff8c6; continue;
            }
            0x2ff8c6 => {
                eax = m.read32(ecx);
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                fa = ((((m.read16(eax.wrapping_add(0x524f5a)))) as u32) & 0xffffu32);
                fb = ((((((edi) as u16))) as u32) & 0xffffu32);
                if (fa == fb) { pc = 0x2ff991; continue; }
                pc = 0x2ff8db; continue;
            }
            0x2ff8db => {
                fa = edx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2ff9ad; continue; }
                pc = 0x2ff8e4; continue;
            }
            0x2ff8e4 => {
                push!(0);
                push!(0);
                push!(0x9be);
                call!(0x3d7b0);
                pc = 0x2ff8f2; continue;
            }
            0x2ff8f2 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x2);
                call!(0x74a30);
                pc = 0x2ff8fd; continue;
            }
            0x2ff8fd => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xa9f);
                call!(0x3d7b0);
                pc = 0x2ff90e; continue;
            }
            0x2ff90e => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x3);
                call!(0x74a30);
                pc = 0x2ff919; continue;
            }
            0x2ff919 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0xaab);
                call!(0x3d7b0);
                pc = 0x2ff92a; continue;
            }
            0x2ff92a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                call!(0x74a30);
                pc = 0x2ff935; continue;
            }
            0x2ff935 => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x2f1c));
                edx = m.read32(eax);
                push!(edx);
                call!(0x2f7770);
                pc = 0x2ff949; continue;
            }
            0x2ff949 => {
                ecx = m.read32(eax.wrapping_add(0x8));
                esp = esp.wrapping_add(0x14);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff97f; continue; }
                pc = 0x2ff953; continue;
            }
            0x2ff953 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x2f1c));
                fa = m.read32(ecx);
                fb = ((((0x47i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2ff97f; continue; }
                pc = 0x2ff963; continue;
            }
            0x2ff963 => {
                push!(0);
                push!(0);
                push!(0x9c0);
                call!(0x3d7b0);
                pc = 0x2ff971; continue;
            }
            0x2ff971 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a30);
                pc = 0x2ff97c; continue;
            }
            0x2ff97c => {
                esp = esp.wrapping_add(0x10);
                pc = 0x2ff97f; continue;
            }
            0x2ff97f => {
                ebp = 0x2;
                m.write32(0x784010, ebp);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x2ff991 => {
                push!(0);
                push!(0);
                push!(0xa96);
                pc = 0x2ff99a; continue;
            }
            0x2ff99a => {
                call!(0x3d7b0);
                pc = 0x2ff99f; continue;
            }
            0x2ff99f => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                pc = 0x2ff9a5; continue;
            }
            0x2ff9a5 => {
                call!(0x74a30);
                pc = 0x2ff9aa; continue;
            }
            0x2ff9aa => {
                esp = esp.wrapping_add(0x10);
                pc = 0x2ff9ad; continue;
            }
            0x2ff9ad => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(0x784010, ebp);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x002FF9D0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2ff9d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2ff9d0;
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
        { crate::pose::cover(0x2ff9d0, previous, pc); previous = pc; }
        match pc {
            0x2ff9d0 => {
                push!(ecx);
                eax = m.read32(esp.wrapping_add(0x8));
                push!(ebp);
                push!(esi);
                push!(edi);
                push!(eax);
                ecx = esp.wrapping_add(0x10);
                push!(ecx);
                call!(0x2fe480);
                pc = 0x2ff9e3; continue;
            }
            0x2ff9e3 => {
                edi = m.read32(0x7844a8);
                ebp = eax;
                edi = edi.wrapping_add(0x2f34);
                ecx = 0xf;
                esi = ebp;
                { let step = if entry.eflags & 0x400 == 0 { 4u32 } else { 0u32.wrapping_sub(4) }; for i in 0..ecx { let value=m.read32(esi.wrapping_add(i.wrapping_mul(step)));m.write32(edi.wrapping_add(i.wrapping_mul(step)),value); } esi=esi.wrapping_add(ecx.wrapping_mul(step));edi=edi.wrapping_add(ecx.wrapping_mul(step));ecx=0; }
                edx = m.read32(0x7844a8);
                eax = (eax & 0xffff_ff00) | ((((m.read8(edx.wrapping_add(0x238))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x8);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x2ffa2b; continue; }
                pc = 0x2ffa0d; continue;
            }
            0x2ffa0d => {
                push!(0);
                call!(0x3370a0);
                pc = 0x2ffa14; continue;
            }
            0x2ffa14 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | 0x20);
                m.write32(eax.wrapping_add(0x238), ecx);
                pc = 0x2ffa2b; continue;
            }
            0x2ffa2b => {
                eax = m.read32(esp.wrapping_add(0xc));
                push!(ebp);
                push!(eax);
                call!(0x3080b0);
                pc = 0x2ffa36; continue;
            }
            0x2ffa36 => {
                esp = esp.wrapping_add(0x8);
                call!(0x73c80);
                pc = 0x2ffa3e; continue;
            }
            0x2ffa3e => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x002FFA50: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f2ffa50<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2ffa50;
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
        { crate::pose::cover(0x2ffa50, previous, pc); previous = pc; }
        match pc {
            0x2ffa50 => {
                ecx = m.read32(0x7844a8);
                fa = ((((m.read8(ecx.wrapping_add(0x238)))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x2ffa83; continue; }
                pc = 0x2ffa5f; continue;
            }
            0x2ffa5f => {
                push!(0);
                call!(0x3370a0);
                pc = 0x2ffa66; continue;
            }
            0x2ffa66 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                esp = esp.wrapping_add(0x4);
                ecx = (ecx | 0x20);
                m.write32(eax.wrapping_add(0x238), ecx);
                ecx = m.read32(0x7844a8);
                pc = 0x2ffa83; continue;
            }
            0x2ffa83 => {
                edx = ((m.read16(ecx.wrapping_add(0x306e))) as u16 as u32);
                push!(esi);
                eax = 0;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                push!(edi);
                if (fas <= fbs) { pc = 0x2ffaaf; continue; }
                pc = 0x2ffa92; continue;
            }
            0x2ffa92 => {
                esi = m.read32(esp.wrapping_add(0xc));
                ecx = ecx.wrapping_add(0x307a);
                pc = 0x2ffaa0; continue;
            }
            0x2ffaa0 => {
                edi = (((((m.read16(ecx) as i16)) as i32)) as u32);
                fa = edi;
                fb = esi;
                if (fa == fb) { pc = 0x2ffab2; continue; }
                pc = 0x2ffaa7; continue;
            }
            0x2ffaa7 => {
                eax = eax.wrapping_add(1);
                ecx = ecx.wrapping_add(0xc);
                fa = eax;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2ffaa0; continue; }
                pc = 0x2ffaaf; continue;
            }
            0x2ffaaf => {
                eax = (eax | 0xffffffffu32);
                pc = 0x2ffab2; continue;
            }
            0x2ffab2 => {
                esi = eax;
                call!(0x2f8040);
                pc = 0x2ffab9; continue;
            }
            0x2ffab9 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x73c80); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
