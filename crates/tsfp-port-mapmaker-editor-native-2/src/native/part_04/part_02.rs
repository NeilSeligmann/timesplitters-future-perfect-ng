use super::*;
#[rustfmt::skip] mod unformatted { use super::*;

/// 0x0031B560: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31b560<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31b560;
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
        { crate::pose::cover(0x31b560, previous, pc); previous = pc; }
        match pc {
            0x31b560 => {
                eax = m.read32(esp.wrapping_add(0x10));
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x31b572; continue;
            }
            0x31b572 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79460);
                pc = 0x31b57b; continue;
            }
            0x31b57b => {
                push!(0x2);
                push!(0);
                push!(0);
                call!(0x78320);
                pc = 0x31b586; continue;
            }
            0x31b586 => {
                ecx = m.read32(esp.wrapping_add(0x28));
                esp = esp.wrapping_add(0x20);
                cf = (((((ecx) != 0))) as i32);
                ecx = (((((ecx) as i32).wrapping_neg())) as u32);
                ecx = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                ecx = (ecx & 0x319e60);
                push!(ecx);
                push!(0);
                push!(0x412ccccc);
                call!(0x71c60);
                pc = 0x31b5a4; continue;
            }
            0x31b5a4 => {
                { let value=round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x49f7f8)) as f64),fpu.control);fpu.stack[((fpu.top+0)&7) as usize]=value; }
                push!(ecx);
                m.write32(esp, (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(0x4);
                push!(0xffffffffu32);
                push!(0);
                call!(0x792b0);
                pc = 0x31b5b9; continue;
            }
            0x31b5b9 => {
                edx = m.read32(esp.wrapping_add(0x28));
                push!(0x31ad90);
                push!(edx);
                push!(0x412ccccc);
                push!(0);
                push!(0);
                push!(0x40);
                push!(0);
                call!(0x792b0);
                pc = 0x31b5d5; continue;
            }
            0x31b5d5 => {
                push!(0x2e);
                call!(0x716d0);
                pc = 0x31b5dc; continue;
            }
            0x31b5dc => {
                esp = esp.wrapping_add(0x3c);
                call!(0x78380);
                pc = 0x31b5e4; continue;
            }
            0x31b5e4 => {
                tail_to!(0x783d0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031B5F0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31b5f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31b5f0;
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
        { crate::pose::cover(0x31b5f0, previous, pc); previous = pc; }
        match pc {
            0x31b5f0 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0x9bc);
                call!(0x3d7b0);
                pc = 0x31b602; continue;
            }
            0x31b602 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31b60b; continue;
            }
            0x31b60b => {
                push!(0x3);
                push!(0);
                push!(0);
                call!(0x78320);
                pc = 0x31b616; continue;
            }
            0x31b616 => {
                esp = esp.wrapping_add(0x20);
                push!(0x319e60);
                push!(0);
                push!(0x412ccccc);
                call!(0x71c60);
                pc = 0x31b62a; continue;
            }
            0x31b62a => {
                { let value=round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x49f7fc)) as f64),fpu.control);fpu.stack[((fpu.top+0)&7) as usize]=value; }
                push!(ecx);
                m.write32(esp, (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(0x4);
                push!(0xb88);
                push!(0);
                call!(0x792b0);
                pc = 0x31b642; continue;
            }
            0x31b642 => {
                push!(0x31ad90);
                push!(0x31a560);
                push!(0x412ccccc);
                push!(0x410a3d71);
                push!(0);
                push!(0xffffffffu32);
                push!(0);
                call!(0x792b0);
                pc = 0x31b661; continue;
            }
            0x31b661 => {
                eax = m.read32(esp.wrapping_add(0x3c));
                ecx = m.read32(eax.wrapping_add(0x4));
                esp = esp.wrapping_add(0x38);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = 0x31a580;
                if (fa != fb) { pc = 0x31b679; continue; }
                pc = 0x31b674; continue;
            }
            0x31b674 => {
                eax = 0x31a8c0;
                pc = 0x31b679; continue;
            }
            0x31b679 => {
                push!(0x31ad90);
                push!(eax);
                push!(0x412ccccc);
                push!(0x410a3d71);
                push!(0);
                push!(0x66);
                push!(0);
                call!(0x792b0);
                pc = 0x31b694; continue;
            }
            0x31b694 => {
                esp = esp.wrapping_add(0x1c);
                call!(0x78380);
                pc = 0x31b69c; continue;
            }
            0x31b69c => {
                tail_to!(0x783d0); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031BA80: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31ba80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31ba80;
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
        { crate::pose::cover(0x31ba80, previous, pc); previous = pc; }
        match pc {
            0x31ba80 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(0x762f94)) as u32)) & 0xff);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031BA90: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31ba90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31ba90;
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
        { crate::pose::cover(0x31ba90, previous, pc); previous = pc; }
        match pc {
            0x31ba90 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(esp.wrapping_add(0x4))) as u32)) & 0xff);
                m.write8(0x762f94, (((((eax) as u8)) as u32)) as u8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031BAA0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31baa0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31baa0;
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
        { crate::pose::cover(0x31baa0, previous, pc); previous = pc; }
        match pc {
            0x31baa0 => {
                eax = ((m.read8(esp.wrapping_add(0x4))) as u8 as u32);
                ecx = m.read32(0x7844a8);
                eax = ((eax as i32).wrapping_mul((0xe4) as i32)) as u32;
                eax = eax.wrapping_add(ecx).wrapping_add(0x288);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031BAC0: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31bac0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31bac0;
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
        { crate::pose::cover(0x31bac0, previous, pc); previous = pc; }
        match pc {
            0x31bac0 => {
                ecx = ((m.read8(0x762f94)) as u8 as u32);
                eax = m.read32(0x7844a8);
                ecx = ((ecx as i32).wrapping_mul((0xe4) as i32)) as u32;
                push!(esi);
                esi = ecx.wrapping_add(eax).wrapping_add(0x288);
                ecx = m.read32(eax.wrapping_add(0x238));
                ecx = (ecx | 0x1000);
                push!(edi);
                m.write32(eax.wrapping_add(0x238), ecx);
                edi = eax.wrapping_add(0x11758);
                ecx = 0x39;
                { let step = if entry.eflags & 0x400 == 0 { 4u32 } else { 0u32.wrapping_sub(4) }; for i in 0..ecx { let value=m.read32(esi.wrapping_add(i.wrapping_mul(step)));m.write32(edi.wrapping_add(i.wrapping_mul(step)),value); } esi=esi.wrapping_add(ecx.wrapping_mul(step));edi=edi.wrapping_add(ecx.wrapping_mul(step));ecx=0; }
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031BB00: compile candidate; per-function tier in docs/t1935-editor-next.md.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31bb00<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31bb00;
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
        { crate::pose::cover(0x31bb00, previous, pc); previous = pc; }
        match pc {
            0x31bb00 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x238));
                ecx = (ecx & 0xffffefffu32);
                push!(edi);
                m.write32(eax.wrapping_add(0x238), ecx);
                edi = m.read32(0x7844a8);
                edi = edi.wrapping_add(0x11758);
                ecx = 0x39;
                eax = 0;
                { let step: u32 = if entry.eflags & 0x400 != 0 { 4u32.wrapping_neg() } else { 4 }; for i in 0..ecx { m.write32(edi.wrapping_add(i.wrapping_mul(step)), eax); } edi = edi.wrapping_add(ecx.wrapping_mul(step)); }  // rep stosd, DF from entry.eflags
                ecx = 0;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
