use super::*; // portreg: replaces=tsfp-ports/mapmaker_ui_lane
#[rustfmt::skip] mod unformatted { use super::*;
/// 0x00318EA0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f318ea0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x318ea0;
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
        { crate::pose::cover(0x318ea0, previous, pc); previous = pc; }
        match pc {
            0x318ea0 => {
                eax = m.read32(0x7844a8);
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x2f1c));
                push!(edi);
                push!(0x2fb3c0);
                push!(0);
                push!(0x42480000);
                push!(0);
                push!(0x8400);
                esi = esi.wrapping_add(0x18);
                call!(0x75b70);
                pc = 0x318ec8; continue;
            }
            0x318ec8 => {
                edi = m.read32(esp.wrapping_add(0x20));
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                m.write32(edi.wrapping_add(0x218), 0x1);
                call!(0x71b10);
                pc = 0x318ee6; continue;
            }
            0x318ee6 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x318ef0; continue;
            }
            0x318ef0 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                push!(0x1);
                push!(0xb2a);
                call!(0x3d7b0);
                pc = 0x318f05; continue;
            }
            0x318f05 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x318f0e; continue;
            }
            0x318f0e => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x318f17; continue;
            }
            0x318f17 => {
                push!(0x99);
                call!(0x716d0);
                pc = 0x318f21; continue;
            }
            0x318f21 => {
                esp = esp.wrapping_add(0x20);
                push!(0);
                push!(0xb2b);
                call!(0x3d7b0);
                pc = 0x318f30; continue;
            }
            0x318f30 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318f39; continue;
            }
            0x318f39 => {
                esp = esp.wrapping_add(0x8);
                push!(0x1);
                push!(0xb2c);
                call!(0x3d7b0);
                pc = 0x318f48; continue;
            }
            0x318f48 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318f51; continue;
            }
            0x318f51 => {
                esp = esp.wrapping_add(0x8);
                push!(0x2);
                push!(0xb2d);
                call!(0x3d7b0);
                pc = 0x318f60; continue;
            }
            0x318f60 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318f69; continue;
            }
            0x318f69 => {
                esp = esp.wrapping_add(0x8);
                push!(0x3);
                push!(0xb2e);
                call!(0x3d7b0);
                pc = 0x318f78; continue;
            }
            0x318f78 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318f81; continue;
            }
            0x318f81 => {
                esp = esp.wrapping_add(0x8);
                push!(0x4);
                push!(0xb2f);
                call!(0x3d7b0);
                pc = 0x318f90; continue;
            }
            0x318f90 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318f99; continue;
            }
            0x318f99 => {
                ecx = esi.wrapping_add(0x4);
                push!(ecx);
                call!(0x78590);
                pc = 0x318fa2; continue;
            }
            0x318fa2 => {
                esp = esp.wrapping_add(0xc);
                call!(0x783d0);
                pc = 0x318faa; continue;
            }
            0x318faa => {
                push!(0);
                push!(0);
                push!(0);
                push!(0x2);
                push!(0xb31);
                call!(0x3d7b0);
                pc = 0x318fbc; continue;
            }
            0x318fbc => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x318fc5; continue;
            }
            0x318fc5 => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x318fce; continue;
            }
            0x318fce => {
                push!(0x9b);
                call!(0x716d0);
                pc = 0x318fd8; continue;
            }
            0x318fd8 => {
                esp = esp.wrapping_add(0x20);
                push!(0xffffffffu32);
                push!(0xb32);
                call!(0x3d7b0);
                pc = 0x318fe7; continue;
            }
            0x318fe7 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x318ff0; continue;
            }
            0x318ff0 => {
                esp = esp.wrapping_add(0x8);
                push!(0);
                push!(0xb33);
                call!(0x3d7b0);
                pc = 0x318fff; continue;
            }
            0x318fff => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x319008; continue;
            }
            0x319008 => {
                esp = esp.wrapping_add(0x8);
                push!(0x1);
                push!(0xb34);
                call!(0x3d7b0);
                pc = 0x319017; continue;
            }
            0x319017 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x319020; continue;
            }
            0x319020 => {
                esi = esi.wrapping_add(0x8);
                push!(esi);
                call!(0x78590);
                pc = 0x319029; continue;
            }
            0x319029 => {
                call!(0x783d0);
                pc = 0x31902e; continue;
            }
            0x31902e => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x319038; continue;
            }
            0x319038 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0x2fb3c0);
                push!(0x40000000);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x319055; continue;
            }
            0x319055 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31905e; continue;
            }
            0x31905e => {
                call!(0x75e70);
                pc = 0x319063; continue;
            }
            0x319063 => {
                xmm0 = Xmm::scalar(m.read32(0x49f7e8));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x68)))).to_bits();
                edx = m.read32(0x7bb1e8);
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm0.lanes[0]);
                push!(edx);
                call!(0x77320);
                pc = 0x319084; continue;
            }
            0x319084 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}
/// 0x003190C0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3190c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3190c0;
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
        { crate::pose::cover(0x3190c0, previous, pc); previous = pc; }
        match pc {
            0x3190c0 => {
                eax = m.read32(0x7844a8);
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x2f1c));
                push!(edi);
                push!(0x2fb3c0);
                push!(0);
                push!(0x42480000);
                push!(0);
                push!(0x8400);
                esi = esi.wrapping_add(0x18);
                call!(0x75b70);
                pc = 0x3190e8; continue;
            }
            0x3190e8 => {
                edi = m.read32(esp.wrapping_add(0x20));
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                m.write32(edi.wrapping_add(0x218), 0x1);
                call!(0x71b10);
                pc = 0x319106; continue;
            }
            0x319106 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x319110; continue;
            }
            0x319110 => {
                eax = m.read32(esi);
                esp = esp.wrapping_add(0x28);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                push!(0);
                if (fa == fb) { pc = 0x31915d; continue; }
                pc = 0x31911c; continue;
            }
            0x31911c => {
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb36);
                call!(0x3d7b0);
                pc = 0x31912c; continue;
            }
            0x31912c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x319135; continue;
            }
            0x319135 => {
                push!(0);
                push!(0);
                push!(0x1);
                push!(0x8);
                push!(0x1);
                push!(esi);
                push!(0);
                push!(0);
                call!(0x7d5e0);
                pc = 0x319149; continue;
            }
            0x319149 => {
                push!(0x9c);
                call!(0x716d0);
                pc = 0x319153; continue;
            }
            0x319153 => {
                esp = esp.wrapping_add(0x38);
                call!(0x783d0);
                pc = 0x31915b; continue;
            }
            0x31915b => {
                pc = 0x31918b; continue;
            }
            0x31915d => {
                push!(0x319090);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xb38);
                call!(0x3d7b0);
                pc = 0x319175; continue;
            }
            0x319175 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31917e; continue;
            }
            0x31917e => {
                push!(0x9d);
                call!(0x716d0);
                pc = 0x319188; continue;
            }
            0x319188 => {
                esp = esp.wrapping_add(0x1c);
                pc = 0x31918b; continue;
            }
            0x31918b => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x319195; continue;
            }
            0x319195 => {
                esp = esp.wrapping_add(0x4);
                push!(0);
                push!(0x2fb3c0);
                push!(0x40000000);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x3191b2; continue;
            }
            0x3191b2 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x3191bb; continue;
            }
            0x3191bb => {
                call!(0x75e70);
                pc = 0x3191c0; continue;
            }
            0x3191c0 => {
                xmm0 = Xmm::scalar(m.read32(0x49f7e8));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x68)))).to_bits();
                ecx = m.read32(0x7bb1e8);
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                call!(0x77320);
                pc = 0x3191e1; continue;
            }
            0x3191e1 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003191F0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3191f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3191f0;
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
        { crate::pose::cover(0x3191f0, previous, pc); previous = pc; }
        match pc {
            0x3191f0 => {
                eax = m.read32(0x7844a8);
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x2f1c));
                push!(edi);
                push!(0x2fb3c0);
                push!(0);
                push!(0x42480000);
                push!(0);
                push!(0x8400);
                esi = esi.wrapping_add(0x18);
                call!(0x75b70);
                pc = 0x319218; continue;
            }
            0x319218 => {
                edi = m.read32(esp.wrapping_add(0x20));
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                m.write32(edi.wrapping_add(0x218), 0x1);
                call!(0x71b10);
                pc = 0x319236; continue;
            }
            0x319236 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x319240; continue;
            }
            0x319240 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xb35);
                call!(0x3d7b0);
                pc = 0x319258; continue;
            }
            0x319258 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x319261; continue;
            }
            0x319261 => {
                push!(0);
                push!(0);
                push!(0x1);
                push!(0x8);
                push!(0x1);
                push!(esi);
                push!(0);
                push!(0);
                call!(0x7d5e0);
                pc = 0x319275; continue;
            }
            0x319275 => {
                push!(0x9e);
                call!(0x716d0);
                pc = 0x31927f; continue;
            }
            0x31927f => {
                call!(0x783d0);
                pc = 0x319284; continue;
            }
            0x319284 => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x31928e; continue;
            }
            0x31928e => {
                esp = esp.wrapping_add(0x3c);
                push!(0);
                push!(0x2fb3c0);
                push!(0x40000000);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x3192ab; continue;
            }
            0x3192ab => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x3192b4; continue;
            }
            0x3192b4 => {
                call!(0x75e70);
                pc = 0x3192b9; continue;
            }
            0x3192b9 => {
                xmm0 = Xmm::scalar(m.read32(0x49f7e8));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x68)))).to_bits();
                ecx = m.read32(0x7bb1e8);
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                call!(0x77320);
                pc = 0x3192da; continue;
            }
            0x3192da => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x003192E0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f3192e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x3192e0;
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
        { crate::pose::cover(0x3192e0, previous, pc); previous = pc; }
        match pc {
            0x3192e0 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                push!(0);
                push!(esi);
                call!(0x71880);
                pc = 0x3192ed; continue;
            }
            0x3192ed => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x3192fd; continue; }
                pc = 0x3192f4; continue;
            }
            0x3192f4 => {
                push!(esi);
                call!(0x78980);
                pc = 0x3192fa; continue;
            }
            0x3192fa => {
                esp = esp.wrapping_add(0x4);
                pc = 0x3192fd; continue;
            }
            0x3192fd => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x00319300: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f319300<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x319300;
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
        { crate::pose::cover(0x319300, previous, pc); previous = pc; }
        match pc {
            0x319300 => {
                eax = m.read32(0x7844a8);
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x2f1c));
                push!(edi);
                push!(0x2fb3c0);
                push!(0);
                push!(0x42480000);
                push!(0);
                push!(0x8400);
                esi = esi.wrapping_add(0x18);
                call!(0x75b70);
                pc = 0x319328; continue;
            }
            0x319328 => {
                edi = m.read32(esp.wrapping_add(0x20));
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                m.write32(edi.wrapping_add(0x218), 0x1);
                call!(0x71b10);
                pc = 0x319346; continue;
            }
            0x319346 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x319350; continue;
            }
            0x319350 => {
                eax = m.read32(esi);
                esp = esp.wrapping_add(0x28);
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(0);
                push!(0);
                push!(ecx);
                push!(0xffffffffu32);
                push!(0xb3b);
                call!(0x3d7b0);
                pc = 0x31936d; continue;
            }
            0x31936d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x319376; continue;
            }
            0x319376 => {
                push!(0x20);
                push!(0);
                push!(0x1);
                push!(0x12c);
                push!(0x1);
                edx = esi.wrapping_add(0x4);
                push!(edx);
                push!(0);
                push!(0);
                call!(0x7d5e0);
                pc = 0x319390; continue;
            }
            0x319390 => {
                push!(0x9f);
                call!(0x716d0);
                pc = 0x31939a; continue;
            }
            0x31939a => {
                esp = esp.wrapping_add(0x38);
                call!(0x783d0);
                pc = 0x3193a2; continue;
            }
            0x3193a2 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xb3a);
                call!(0x3d7b0);
                pc = 0x3193b9; continue;
            }
            0x3193b9 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x3193c2; continue;
            }
            0x3193c2 => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x3193cb; continue;
            }
            0x3193cb => {
                push!(0xa0);
                call!(0x716d0);
                pc = 0x3193d5; continue;
            }
            0x3193d5 => {
                push!(esi);
                call!(0x78560);
                pc = 0x3193db; continue;
            }
            0x3193db => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x3193e5; continue;
            }
            0x3193e5 => {
                esp = esp.wrapping_add(0x2c);
                push!(0);
                push!(0x2fb3c0);
                push!(0x40000000);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x319402; continue;
            }
            0x319402 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31940b; continue;
            }
            0x31940b => {
                call!(0x75e70);
                pc = 0x319410; continue;
            }
            0x319410 => {
                xmm0 = Xmm::scalar(m.read32(0x49f7e8));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x68)))).to_bits();
                eax = m.read32(0x7bb1e8);
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm0.lanes[0]);
                push!(eax);
                call!(0x77320);
                pc = 0x319430; continue;
            }
            0x319430 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00319BD0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f319bd0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x319bd0;
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
        { crate::pose::cover(0x319bd0, previous, pc); previous = pc; }
        match pc {
            0x319bd0 => {
                push!(0x319b50);
                push!(0);
                push!(0x42480000);
                push!(0);
                push!(0x10400);
                call!(0x75b70);
                pc = 0x319be8; continue;
            }
            0x319be8 => {
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                call!(0x71b10);
                pc = 0x319bf8; continue;
            }
            0x319bf8 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x319c02; continue;
            }
            0x319c02 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0xab8);
                call!(0x3d7b0);
                pc = 0x319c13; continue;
            }
            0x319c13 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x1);
                call!(0x74a00);
                pc = 0x319c1e; continue;
            }
            0x319c1e => {
                push!(0x319550);
                push!(0);
                push!(0x41f00000);
                push!(0x41f00000);
                push!(0x4);
                push!(0);
                push!(0);
                call!(0x792b0);
                pc = 0x319c3a; continue;
            }
            0x319c3a => {
                esp = esp.wrapping_add(0x2c);
                push!(0);
                push!(0);
                push!(0);
                push!(0x1);
                push!(0xa92);
                call!(0x3d7b0);
                pc = 0x319c4f; continue;
            }
            0x319c4f => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x319c58; continue;
            }
            0x319c58 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                call!(0x7d680);
                pc = 0x319c67; continue;
            }
            0x319c67 => {
                call!(0x71630);
                pc = 0x319c6c; continue;
            }
            0x319c6c => {
                push!(eax);
                push!(0x762f84);
                call!(0x785c0);
                pc = 0x319c77; continue;
            }
            0x319c77 => {
                esp = esp.wrapping_add(0x30);
                call!(0x783d0);
                pc = 0x319c7f; continue;
            }
            0x319c7f => {
                push!(0);
                push!(0);
                push!(0);
                push!(0x2);
                push!(0xa93);
                call!(0x3d7b0);
                pc = 0x319c91; continue;
            }
            0x319c91 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x319c9a; continue;
            }
            0x319c9a => {
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                call!(0x7d680);
                pc = 0x319ca9; continue;
            }
            0x319ca9 => {
                call!(0x71630);
                pc = 0x319cae; continue;
            }
            0x319cae => {
                push!(eax);
                push!(0x762f88);
                call!(0x785c0);
                pc = 0x319cb9; continue;
            }
            0x319cb9 => {
                esp = esp.wrapping_add(0x30);
                call!(0x783d0);
                pc = 0x319cc1; continue;
            }
            0x319cc1 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0x3);
                push!(0xa94);
                call!(0x3d7b0);
                pc = 0x319cd3; continue;
            }
            0x319cd3 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x319cdc; continue;
            }
            0x319cdc => {
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                call!(0x7d680);
                pc = 0x319ceb; continue;
            }
            0x319ceb => {
                call!(0x71630);
                pc = 0x319cf0; continue;
            }
            0x319cf0 => {
                push!(eax);
                push!(0x762f8c);
                call!(0x785c0);
                pc = 0x319cfb; continue;
            }
            0x319cfb => {
                esp = esp.wrapping_add(0x30);
                call!(0x783d0);
                pc = 0x319d03; continue;
            }
            0x319d03 => {
                push!(0x319b20);
                push!(0);
                push!(0);
                push!(0x4);
                push!(0xa95);
                call!(0x3d7b0);
                pc = 0x319d18; continue;
            }
            0x319d18 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x319d21; continue;
            }
            0x319d21 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                call!(0x7d680);
                pc = 0x319d30; continue;
            }
            0x319d30 => {
                call!(0x71630);
                pc = 0x319d35; continue;
            }
            0x319d35 => {
                push!(eax);
                push!(0x783b88);
                call!(0x785c0);
                pc = 0x319d40; continue;
            }
            0x319d40 => {
                call!(0x783d0);
                pc = 0x319d45; continue;
            }
            0x319d45 => {
                call!(0x22b90);
                pc = 0x319d4a; continue;
            }
            0x319d4a => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                eax = m.read32(0x7bb1e8);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                esp = esp.wrapping_add(0x2c);
                m.write32(esp, xmm1.lanes[0]);
                push!(eax);
                call!(0x77320);
                pc = 0x319d94; continue;
            }
            0x319d94 => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00319DA0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f319da0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x319da0;
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
        { crate::pose::cover(0x319da0, previous, pc); previous = pc; }
        match pc {
            0x319da0 => {
                push!(ebx);
                push!(ebp);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x14));
                push!(edi);
                push!(esi);
                call!(0x757e0);
                pc = 0x319dae; continue;
            }
            0x319dae => {
                ebp = m.read32(esp.wrapping_add(0x18));
                fpu.push((f32::from_bits(m.read32(ebp.wrapping_add(0xf4))) as f64));
                ebx = eax;
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475cc0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                call!(0x3c848c);
                pc = 0x319dc5; continue;
            }
            0x319dc5 => {
                ecx = m.read32(esi.wrapping_add(0x14));
                edx = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x783a80));
                edi = eax;
                eax = m.read32(ebp.wrapping_add(0xf4));
                push!(eax);
                push!(edx);
                call!(0x154fe0);
                pc = 0x319dde; continue;
            }
            0x319dde => {
                esi = m.read32(esp.wrapping_add(0x28));
                esp = esp.wrapping_add(0xc);
                edi = (edi & 0xff);
                fa = ebx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebp = eax;
                if (fa == fb) { pc = 0x319e11; continue; }
                pc = 0x319df1; continue;
            }
            0x319df1 => {
                eax = (((((f32::from_bits(m.read32(esi.wrapping_add(0xc))) as f64)) as i32)) as u32);
                ecx = (((((f32::from_bits(m.read32(esi.wrapping_add(0x8))) as f64)) as i32)) as u32);
                edx = (((((f32::from_bits(m.read32(esi.wrapping_add(0x4))) as f64)) as i32)) as u32);
                edi = (edi | 0x78787800);
                push!(edi);
                push!(eax);
                eax = (((((f32::from_bits(m.read32(esi)) as f64)) as i32)) as u32);
                push!(ecx);
                push!(edx);
                push!(eax);
                pc = 0x319e29; continue;
            }
            0x319e11 => {
                ecx = (((((f32::from_bits(m.read32(esi.wrapping_add(0xc))) as f64)) as i32)) as u32);
                edx = (((((f32::from_bits(m.read32(esi.wrapping_add(0x8))) as f64)) as i32)) as u32);
                eax = (((((f32::from_bits(m.read32(esi.wrapping_add(0x4))) as f64)) as i32)) as u32);
                push!(edi);
                push!(ecx);
                ecx = (((((f32::from_bits(m.read32(esi)) as f64)) as i32)) as u32);
                push!(edx);
                push!(eax);
                push!(ecx);
                pc = 0x319e29; continue;
            }
            0x319e29 => {
                call!(0x1efb0);
                pc = 0x319e2e; continue;
            }
            0x319e2e => {
                edx = (((((f32::from_bits(m.read32(esi.wrapping_add(0xc))) as f64)) as i32)) as u32);
                eax = (((((f32::from_bits(m.read32(esi.wrapping_add(0x8))) as f64)) as i32)) as u32);
                ecx = (((((f32::from_bits(m.read32(esi.wrapping_add(0x4))) as f64)) as i32)) as u32);
                esp = esp.wrapping_add(0x14);
                edx = edx.wrapping_sub(1);
                push!(ebp);
                push!(edx);
                edx = (((((f32::from_bits(m.read32(esi)) as f64)) as i32)) as u32);
                eax = eax.wrapping_sub(1);
                push!(eax);
                ecx = ecx.wrapping_add(1);
                push!(ecx);
                edx = edx.wrapping_add(1);
                push!(edx);
                call!(0x1efb0);
                pc = 0x319e52; continue;
            }
            0x319e52 => {
                esp = esp.wrapping_add(0x14);
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

/// 0x00319E60: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f319e60<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
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
    let mut pc: u32 = 0x319e60;
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
        { crate::pose::cover(0x319e60, previous, pc); previous = pc; }
        match pc {
            0x319e60 => {
                esp = esp.wrapping_sub(0xc);
                push!(ebx);
                push!(ebp);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x1c));
                fa = m.read32(edi.wrapping_add(0x4));
                fb = ((((0x52ea48i32)) as u32) & 0xffffffffu32);
                eax = m.read32(esp.wrapping_add(0x20));
                if (fa != fb) { pc = 0x319e7c; continue; }
                pc = 0x319e77; continue;
            }
            0x319e77 => {
                ebx = m.read32(eax.wrapping_add(0x14));
                pc = 0x319e81; continue;
            }
            0x319e7c => {
                ebx = 0xbc2;
                pc = 0x319e81; continue;
            }
            0x319e81 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x28));
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(0x8))) as f64));
                xmm0 = Xmm::scalar(m.read32(esi));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x7ba038)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                ecx = esp.wrapping_add(0x28);
                push!(ecx);
                edx = esp.wrapping_add(0x14);
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(esi)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                push!(edx);
                push!(eax);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esi.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x2c), (fpu.st(0) as f32).to_bits()); fpu.pop();
                push!(edi);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                call!(0x75f60);
                pc = 0x319ebb; continue;
            }
            0x319ebb => {
                push!(0);
                push!(edi);
                call!(0x70ef0);
                pc = 0x319ec3; continue;
            }
            0x319ec3 => {
                push!(eax);
                call!(0x3fa10);
                pc = 0x319ec9; continue;
            }
            0x319ec9 => {
                ebp = m.read32(esp.wrapping_add(0x3c));
                esp = esp.wrapping_add(0x1c);
                push!(ebp);
                push!(ebx);
                call!(0x3d7b0);
                pc = 0x319ed7; continue;
            }
            0x319ed7 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x40f00);
                pc = 0x319ee0; continue;
            }
            0x319ee0 => {
                edi = eax;
                push!(edi);
                call!(0x40620);
                pc = 0x319ee8; continue;
            }
            0x319ee8 => {
                fpu.push((f32::from_bits(m.read32(esi.wrapping_add(0xc))) as f64));
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(esi.wrapping_add(0x4))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                esp = esp.wrapping_add(0xc);
                { let a = fpu.top as usize; let b = ((fpu.top + 1) & 7) as usize; fpu.stack.swap(a, b); }
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                fp_cmp = compare_fp(fpu.st(0), fpu.st(1)); fpu.pop();
                fpu.pop();
                if fp_cmp != 1 { pc = 0x00319f19; continue; }
                pc = 0x319efa; continue;
            }
            0x319efa => {
                push!(0x2);
                call!(0x3fa10);
                pc = 0x319f01; continue;
            }
            0x319f01 => {
                esp = esp.wrapping_add(0x4);
                push!(ebp);
                push!(ebx);
                call!(0x3d7b0);
                pc = 0x319f0b; continue;
            }
            0x319f0b => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x40f00);
                pc = 0x319f14; continue;
            }
            0x319f14 => {
                esp = esp.wrapping_add(0x8);
                edi = eax;
                pc = 0x319f19; continue;
            }
            0x319f19 => {
                eax = m.read32(esp.wrapping_add(0x24));
                ecx = m.read32(esp.wrapping_add(0xc));
                edx = m.read32(esp.wrapping_add(0x10));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x18));
                push!(ecx);
                push!(edi);
                push!(edx);
                push!(eax);
                call!(0x41e30);
                pc = 0x319f33; continue;
            }
            0x319f33 => {
                esp = esp.wrapping_add(0x14);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031AB80: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31ab80<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31ab80;
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
        { crate::pose::cover(0x31ab80, previous, pc); previous = pc; }
        match pc {
            0x31ab80 => {
                eax = m.read32(0x7844a8);
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x2f18));
                push!(edi);
                push!(0x2fb3c0);
                push!(0);
                push!(0x42480000);
                push!(0);
                push!(0x8400);
                esi = esi.wrapping_add(0x18);
                call!(0x75b70);
                pc = 0x31aba8; continue;
            }
            0x31aba8 => {
                edi = m.read32(esp.wrapping_add(0x20));
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                m.write32(edi.wrapping_add(0x218), 0x1);
                call!(0x71b10);
                pc = 0x31abc6; continue;
            }
            0x31abc6 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x31abd0; continue;
            }
            0x31abd0 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0x9ca);
                call!(0x3d7b0);
                pc = 0x31abe5; continue;
            }
            0x31abe5 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31abee; continue;
            }
            0x31abee => {
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                push!(0);
                call!(0x7d680);
                pc = 0x31abfd; continue;
            }
            0x31abfd => {
                call!(0x71630);
                pc = 0x31ac02; continue;
            }
            0x31ac02 => {
                push!(eax);
                ecx = esi.wrapping_add(0x10);
                push!(ecx);
                call!(0x785c0);
                pc = 0x31ac0c; continue;
            }
            0x31ac0c => {
                esp = esp.wrapping_add(0x30);
                call!(0x783d0);
                pc = 0x31ac14; continue;
            }
            0x31ac14 => {
                push!(0);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0x9c7);
                call!(0x3d7b0);
                pc = 0x31ac26; continue;
            }
            0x31ac26 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31ac2f; continue;
            }
            0x31ac2f => {
                push!(0x9c9);
                call!(0x3d7b0);
                pc = 0x31ac39; continue;
            }
            0x31ac39 => {
                esp = esp.wrapping_add(0x18);
                push!(eax);
                push!(0x9c8);
                call!(0x3d7b0);
                pc = 0x31ac47; continue;
            }
            0x31ac47 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                push!(0);
                push!(0);
                call!(0x7d680);
                pc = 0x31ac56; continue;
            }
            0x31ac56 => {
                call!(0x71630);
                pc = 0x31ac5b; continue;
            }
            0x31ac5b => {
                push!(eax);
                esi = esi.wrapping_add(0xc);
                push!(esi);
                call!(0x785c0);
                pc = 0x31ac65; continue;
            }
            0x31ac65 => {
                call!(0x783d0);
                pc = 0x31ac6a; continue;
            }
            0x31ac6a => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x31ac74; continue;
            }
            0x31ac74 => {
                esp = esp.wrapping_add(0x20);
                push!(0);
                push!(0x2fb3c0);
                push!(0x40000000);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x31ac91; continue;
            }
            0x31ac91 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31ac9a; continue;
            }
            0x31ac9a => {
                call!(0x75e70);
                pc = 0x31ac9f; continue;
            }
            0x31ac9f => {
                xmm0 = Xmm::scalar(m.read32(0x49f7e8));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(edi.wrapping_add(0x68)))).to_bits();
                edx = m.read32(0x7bb1e8);
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm0.lanes[0]);
                push!(edx);
                call!(0x77320);
                pc = 0x31acc0; continue;
            }
            0x31acc0 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031AD90: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31ad90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31ad90;
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
        { crate::pose::cover(0x31ad90, previous, pc); previous = pc; }
        match pc {
            0x31ad90 => {
                esp = esp.wrapping_sub(0x28);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x34));
                push!(esi);
                push!(edi);
                edi = m.read32(ebp.wrapping_add(0x38));
                fa = edi;
                fb = ((((0x320030i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31addf; continue; }
                pc = 0x31ada5; continue;
            }
            0x31ada5 => {
                xmm0 = Xmm::default();
                eax = 0;
                ecx = 0;
                m.write8(esp.wrapping_add(0x20), (0) as u8);
                m.write8(esp.wrapping_add(0x21), (0) as u8);
                esi = m.read32(esp.wrapping_add(0x20));
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                m.write32(esp.wrapping_add(0x10), eax);
                eax = m.read32(esp.wrapping_add(0x2c));
                edx = 0;
                m.write32(esp.wrapping_add(0xc), esi);
                m.write32(esp.wrapping_add(0x14), ecx);
                m.write32(esp.wrapping_add(0x18), eax);
                m.write32(esp.wrapping_add(0x1c), edx);
                pc = 0x31af33; continue;
            }
            0x31addf => {
                fa = edi;
                fb = ((((0x31a600i32)) as u32) & 0xffffffffu32);
                eax = m.read32(0x7844a8);
                if (fa != fb) { pc = 0x31ae37; continue; }
                pc = 0x31adec; continue;
            }
            0x31adec => {
                ecx = m.read32(eax.wrapping_add(0x11858));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31ae37; continue; }
                pc = 0x31adf6; continue;
            }
            0x31adf6 => {
                ecx = (((((m.read16(ecx.wrapping_add(0xc)) as i16)) as i32)) as u32);
                ecx = ((ecx as i32).wrapping_mul((0x8c) as i32)) as u32;
                eax = ecx.wrapping_add(eax).wrapping_add(0xdd14);
                push!(eax);
                call!(0x321400);
                pc = 0x31ae0d; continue;
            }
            0x31ae0d => {
                edx = m.read32(eax);
                m.write32(esp.wrapping_add(0x10), edx);
                ecx = m.read32(eax.wrapping_add(0x4));
                m.write32(esp.wrapping_add(0x14), ecx);
                edx = m.read32(eax.wrapping_add(0x8));
                m.write32(esp.wrapping_add(0x18), edx);
                ecx = m.read32(eax.wrapping_add(0xc));
                m.write32(esp.wrapping_add(0x1c), ecx);
                edx = m.read32(eax.wrapping_add(0x10));
                esp = esp.wrapping_add(0x4);
                m.write32(esp.wrapping_add(0x1c), edx);
                pc = 0x31af33; continue;
            }
            0x31ae37 => {
                fa = edi;
                fb = ((((0x31a540i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31ae79; continue; }
                pc = 0x31ae3f; continue;
            }
            0x31ae3f => {
                call!(0x3118f0);
                pc = 0x31ae44; continue;
            }
            0x31ae44 => {
                xmm0 = Xmm::scalar(m.read32(0x475d4c));
                m.write32(esp.wrapping_add(0x18), xmm0.lanes[0]);
                xmm0 = Xmm::default();
                m.write8(esp.wrapping_add(0xc), (((((eax) as u8)) as u32)) as u8);
                m.write32(esp.wrapping_add(0x10), 0);
                m.write32(esp.wrapping_add(0x14), 0);
                m.write32(esp.wrapping_add(0x1c), xmm0.lanes[0]);
                m.write8(esp.wrapping_add(0xd), (0x7) as u8);
                pc = 0x31af33; continue;
            }
            0x31ae79 => {
                fa = edi;
                fb = ((((0x31a580i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31ae91; continue; }
                pc = 0x31ae81; continue;
            }
            0x31ae81 => {
                fa = edi;
                fb = ((((0x31a8c0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31ae91; continue; }
                pc = 0x31ae89; continue;
            }
            0x31ae89 => {
                fa = edi;
                fb = ((((0x31a560i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31aea5; continue; }
                pc = 0x31ae91; continue;
            }
            0x31ae91 => {
                esi = m.read32(eax.wrapping_add(0x2f18));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31aed7; continue; }
                pc = 0x31ae9b; continue;
            }
            0x31ae9b => {
                ecx = m.read32(eax.wrapping_add(0x11858));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31aed7; continue; }
                pc = 0x31aea5; continue;
            }
            0x31aea5 => {
                eax = m.read32(0x783a60);
                ecx = m.read32(0x783a64);
                edx = m.read32(0x783a68);
                m.write32(esp.wrapping_add(0xc), eax);
                eax = m.read32(0x783a6c);
                m.write32(esp.wrapping_add(0x10), ecx);
                ecx = m.read32(0x783a70);
                m.write32(esp.wrapping_add(0x14), edx);
                m.write32(esp.wrapping_add(0x18), eax);
                m.write32(esp.wrapping_add(0x1c), ecx);
                pc = 0x31af33; continue;
            }
            0x31aed7 => {
                ecx = m.read32(eax.wrapping_add(0x11858));
                edx = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31aef4; continue; }
                pc = 0x31aee3; continue;
            }
            0x31aee3 => {
                edx = (((((m.read16(ecx.wrapping_add(0xc)) as i16)) as i32)) as u32);
                edx = ((edx as i32).wrapping_mul((0x8c) as i32)) as u32;
                edx = edx.wrapping_add(eax).wrapping_add(0xdd14);
                pc = 0x31aef4; continue;
            }
            0x31aef4 => {
                fa = edi;
                fb = ((((0x31a560i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31af11; continue; }
                pc = 0x31aefc; continue;
            }
            0x31aefc => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31af06; continue; }
                pc = 0x31af00; continue;
            }
            0x31af00 => {
                eax = ((m.read8(esi.wrapping_add(0x18))) as u8 as u32);
                pc = 0x31af28; continue;
            }
            0x31af06 => {
                push!(edx);
                call!(0x321400);
                pc = 0x31af0c; continue;
            }
            0x31af0c => {
                eax = ((m.read8(eax)) as u8 as u32);
                pc = 0x31af25; continue;
            }
            0x31af11 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31af1b; continue; }
                pc = 0x31af15; continue;
            }
            0x31af15 => {
                eax = ((m.read8(esi.wrapping_add(0x19))) as u8 as u32);
                pc = 0x31af28; continue;
            }
            0x31af1b => {
                push!(edx);
                call!(0x321400);
                pc = 0x31af21; continue;
            }
            0x31af21 => {
                eax = ((m.read8(eax.wrapping_add(0x1))) as u8 as u32);
                pc = 0x31af25; continue;
            }
            0x31af25 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x31af28; continue;
            }
            0x31af28 => {
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x783a80));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31af40; continue; }
                pc = 0x31af33; continue;
            }
            0x31af33 => {
                eax = esp.wrapping_add(0xc);
                push!(eax);
                call!(0x30a2d0);
                pc = 0x31af3d; continue;
            }
            0x31af3d => {
                esp = esp.wrapping_add(0x4);
                pc = 0x31af40; continue;
            }
            0x31af40 => {
                push!(0);
                push!(0xa0a0a20);
                push!(eax);
                push!(0x27);
                call!(0x81c10);
                pc = 0x31af4f; continue;
            }
            0x31af4f => {
                ecx = m.read32(esp.wrapping_add(0x50));
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(ecx);
                push!(ebp);
                call!(0x757e0);
                pc = 0x31af5e; continue;
            }
            0x31af5e => {
                edx = m.read32(esp.wrapping_add(0x50));
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(edx.wrapping_add(0xf4));
                push!(eax);
                call!(0x315e50);
                pc = 0x31af72; continue;
            }
            0x31af72 => {
                esp = esp.wrapping_add(0x1c);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x28);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031B6B0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31b6b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31b6b0;
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
        { crate::pose::cover(0x31b6b0, previous, pc); previous = pc; }
        match pc {
            0x31b6b0 => {
                call!(0x31a860);
                pc = 0x31b6b5; continue;
            }
            0x31b6b5 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(0x31aae0);
                push!(0);
                push!(0x42480000);
                push!(0);
                push!(0x8400);
                m.write32(eax.wrapping_add(0x218), 0x1);
                call!(0x75b70);
                pc = 0x31b6db; continue;
            }
            0x31b6db => {
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                call!(0x71b10);
                pc = 0x31b6eb; continue;
            }
            0x31b6eb => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x31b6f5; continue;
            }
            0x31b6f5 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x31b703; continue;
            }
            0x31b703 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm2 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                xmm0 = Xmm::scalar(m.read32(0x49f7d0));
                push!(ecx);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x78170);
                pc = 0x31b751; continue;
            }
            0x31b751 => {
                push!(0x40400000);
                call!(0x784a0);
                pc = 0x31b75b; continue;
            }
            0x31b75b => {
                push!(0x783a60);
                call!(0x31a8e0);
                pc = 0x31b765; continue;
            }
            0x31b765 => {
                push!(0x783a60);
                call!(0x31a990);
                pc = 0x31b76f; continue;
            }
            0x31b76f => {
                push!(0x783a60);
                call!(0x31aa00);
                pc = 0x31b779; continue;
            }
            0x31b779 => {
                push!(0x783a60);
                call!(0x31b5f0);
                pc = 0x31b783; continue;
            }
            0x31b783 => {
                push!(0x40400000);
                call!(0x784a0);
                pc = 0x31b78d; continue;
            }
            0x31b78d => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0x31aae0);
                push!(0x40000000);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x31b7aa; continue;
            }
            0x31b7aa => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31b7b3; continue;
            }
            0x31b7b3 => {
                call!(0x781a0);
                pc = 0x31b7b8; continue;
            }
            0x31b7b8 => {
                ecx = m.read32(0x7bb1e8);
                push!(0x41180000);
                push!(ecx);
                call!(0x77320);
                pc = 0x31b7c9; continue;
            }
            0x31b7c9 => {
                esp = esp.wrapping_add(0x20);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031B7D0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31b7d0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31b7d0;
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
        { crate::pose::cover(0x31b7d0, previous, pc); previous = pc; }
        match pc {
            0x31b7d0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                push!(ebx);
                push!(esi);
                eax = 0;
                push!(edi);
                edi = m.read32(ecx.wrapping_add(0x4));
                ecx = m.read32(0x784020);
                fa = edi;
                fb = ((((0x52e040i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa == fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ebx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebx = (ebx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebx = ebx.wrapping_add(ebx).wrapping_add(0x9bb);
                if (fa == fb) { pc = 0x31b80f; continue; }
                pc = 0x31b7fd; continue;
            }
            0x31b7fd => {
                push!(ecx);
                call!(0x311890);
                pc = 0x31b803; continue;
            }
            0x31b803 => {
                esp = esp.wrapping_add(0x4);
                edi = eax;
                esi = 0x31a300;
                pc = 0x31b817; continue;
            }
            0x31b80f => {
                edi = (edi | 0xffffffffu32);
                esi = 0x31a5a0;
                pc = 0x31b817; continue;
            }
            0x31b817 => {
                push!(0x319510);
                push!(0);
                push!(0x42480000);
                push!(0);
                push!(0x8400);
                call!(0x75b70);
                pc = 0x31b82f; continue;
            }
            0x31b82f => {
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                call!(0x71b10);
                pc = 0x31b83f; continue;
            }
            0x31b83f => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x31b849; continue;
            }
            0x31b849 => {
                call!(0x31a780);
                pc = 0x31b84e; continue;
            }
            0x31b84e => {
                push!(ebx);
                push!(edi);
                push!(esi);
                call!(0x31acd0);
                pc = 0x31b856; continue;
            }
            0x31b856 => {
                call!(0x22b90);
                pc = 0x31b85b; continue;
            }
            0x31b85b => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                edx = m.read32(0x7bb1e8);
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                esp = esp.wrapping_add(0x30);
                m.write32(esp, xmm1.lanes[0]);
                push!(edx);
                call!(0x77320);
                pc = 0x31b8a6; continue;
            }
            0x31b8a6 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031B8B0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31b8b0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31b8b0;
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
        { crate::pose::cover(0x31b8b0, previous, pc); previous = pc; }
        match pc {
            0x31b8b0 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(ebx.wrapping_add(0x4));
                eax = 0;
                fa = ecx;
                fb = ((((0x52e000i32)) as u32) & 0xffffffffu32);
                eax = (eax & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                push!(ebp);
                push!(esi);
                push!(edi);
                edi = eax;
                esi = edi;
                cf = (((((esi) != 0))) as i32);
                esi = (((((esi) as i32).wrapping_neg())) as u32);
                esi = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                esi = (esi & 0xffffffbdu32);
                ecx = 0;
                esi = esi.wrapping_add(0xbc6);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ecx = ecx.wrapping_sub(1);
                ecx = (ecx & 0x31a690);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ebp = ecx;
                if (fa != fb) { pc = 0x31b8f2; continue; }
                pc = 0x31b8eb; continue;
            }
            0x31b8eb => {
                eax = ebx;
                call!(0x31a710);
                pc = 0x31b8f2; continue;
            }
            0x31b8f2 => {
                call!(0x31a860);
                pc = 0x31b8f7; continue;
            }
            0x31b8f7 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31b91e; continue; }
                pc = 0x31b8fb; continue;
            }
            0x31b8fb => {
                eax = m.read32(0x7844a8);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x25c))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (((((((((((((fa)) as u8)) as i32).wrapping_sub((((((fb)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) == 0i32)) { pc = 0x31b914; continue; }
                pc = 0x31b90a; continue;
            }
            0x31b90a => {
                m.write32(eax.wrapping_add(0x25c), (m.read32(eax.wrapping_add(0x25c)) | 0x20000000));
                pc = 0x31b914; continue;
            }
            0x31b914 => {
                m.write32(ebx.wrapping_add(0x218), 0x1);
                pc = 0x31b91e; continue;
            }
            0x31b91e => {
                push!(ebp);
                push!(0);
                push!(0x42480000);
                push!(0);
                push!(0x400);
                call!(0x75b70);
                pc = 0x31b932; continue;
            }
            0x31b932 => {
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                call!(0x71b10);
                pc = 0x31b942; continue;
            }
            0x31b942 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x31b94c; continue;
            }
            0x31b94c => {
                esp = esp.wrapping_add(0x28);
                call!(0x31a780);
                pc = 0x31b954; continue;
            }
            0x31b954 => {
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x31b95f; continue;
            }
            0x31b95f => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm2 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                xmm0 = Xmm::scalar(m.read32(0x49f7d0));
                push!(ecx);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x78170);
                pc = 0x31b9ad; continue;
            }
            0x31b9ad => {
                esp = esp.wrapping_add(0x10);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31b9c4; continue; }
                pc = 0x31b9b4; continue;
            }
            0x31b9b4 => {
                push!(esi);
                push!(0x31a330);
                push!(edi);
                push!(ebx);
                call!(0x31b560);
                pc = 0x31b9c1; continue;
            }
            0x31b9c1 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x31b9c4; continue;
            }
            0x31b9c4 => {
                push!(0x9bb);
                push!(0xffffffffu32);
                push!(0x31a5a0);
                call!(0x31acd0);
                pc = 0x31b9d5; continue;
            }
            0x31b9d5 => {
                esp = esp.wrapping_add(0xc);
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa != fb) { pc = 0x31ba0b; continue; }
                pc = 0x31b9e0; continue;
            }
            0x31b9e0 => {
                push!(0x783a60);
                call!(0x31a8e0);
                pc = 0x31b9ea; continue;
            }
            0x31b9ea => {
                push!(0x783a60);
                call!(0x31a990);
                pc = 0x31b9f4; continue;
            }
            0x31b9f4 => {
                push!(0x783a60);
                call!(0x31aa00);
                pc = 0x31b9fe; continue;
            }
            0x31b9fe => {
                push!(0x783a60);
                call!(0x31b5f0);
                pc = 0x31ba08; continue;
            }
            0x31ba08 => {
                esp = esp.wrapping_add(0x10);
                pc = 0x31ba0b; continue;
            }
            0x31ba0b => {
                call!(0x781a0);
                pc = 0x31ba10; continue;
            }
            0x31ba10 => {
                call!(0x22b90);
                pc = 0x31ba15; continue;
            }
            0x31ba15 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                edx = m.read32(0x7bb1e8);
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                push!(ecx);
                m.write32(esp, xmm1.lanes[0]);
                push!(edx);
                call!(0x77320);
                pc = 0x31ba5e; continue;
            }
            0x31ba5e => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031C7C0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31c7c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31c7c0;
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
        { crate::pose::cover(0x31c7c0, previous, pc); previous = pc; }
        match pc {
            0x31c7c0 => {
                esp = esp.wrapping_sub(0x14);
                push!(esi);
                push!(0x33);
                call!(0x81c20);
                pc = 0x31c7cb; continue;
            }
            0x31c7cb => {
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                push!(0x33);
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                call!(0x81c40);
                pc = 0x31c7dc; continue;
            }
            0x31c7dc => {
                xmm2 = Xmm::scalar(m.read32(0x475cd4));
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                eax = m.read32(esp.wrapping_add(0x2c));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0xc)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                ecx = esp.wrapping_add(0x1c);
                push!(ecx);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                eax = m.read32(esp.wrapping_add(0x2c));
                esi = m.read32(eax.wrapping_add(0x14));
                edx = esp.wrapping_add(0x18);
                push!(edx);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x30));
                push!(eax);
                m.write32(esp.wrapping_add(0x28), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x3c), xmm0.lanes[0]);
                call!(0x75f60);
                pc = 0x31c848; continue;
            }
            0x31c848 => {
                esi = ((esi as i32).wrapping_mul((0xe4) as i32)) as u32;
                edx = m.read32(0x7844a8);
                ecx = m.read32(esp.wrapping_add(0x24));
                eax = m.read32(esi.wrapping_add(edx).wrapping_add(0x2e4));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x28)))).to_bits();
                edx = m.read32(esp.wrapping_add(0x20));
                esp = esp.wrapping_add(0x18);
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x28));
                eax = eax.wrapping_add(0x33);
                push!(eax);
                esp = esp.wrapping_sub(0x8);
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x18)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                push!(ecx);
                push!(edx);
                call!(0x81ce0);
                pc = 0x31c89c; continue;
            }
            0x31c89c => {
                esp = esp.wrapping_add(0x18);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031CB90: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31cb90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31cb90;
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
        { crate::pose::cover(0x31cb90, previous, pc); previous = pc; }
        match pc {
            0x31cb90 => {
                push!(ecx);
                eax = ((m.read8(0x762f94)) as u8 as u32);
                ecx = m.read32(0x7844a8);
                eax = ((eax as i32).wrapping_mul((0xe4) as i32)) as u32;
                push!(ebx);
                push!(ebp);
                push!(esi);
                push!(edi);
                esi = eax.wrapping_add(ecx).wrapping_add(0x288);
                call!(0x3129d0);
                pc = 0x31cbb4; continue;
            }
            0x31cbb4 => {
                push!(eax);
                call!(0x2c0290);
                pc = 0x31cbba; continue;
            }
            0x31cbba => {
                ebp = esi.wrapping_add(0x34);
                ebx = eax;
                eax = m.read32(ebp);
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31cbdc; continue; }
                pc = 0x31cbca; continue;
            }
            0x31cbca => {
                fa = eax;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31cbdc; continue; }
                pc = 0x31cbcf; continue;
            }
            0x31cbcf => {
                fa = eax;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x10), 0x1);
                if (fa != fb) { pc = 0x31cbe4; continue; }
                pc = 0x31cbdc; continue;
            }
            0x31cbdc => {
                m.write32(esp.wrapping_add(0x10), 0);
                pc = 0x31cbe4; continue;
            }
            0x31cbe4 => {
                edx = m.read32(esp.wrapping_add(0x1c));
                push!(edx);
                push!(0);
                push!(0x42480000);
                push!(0);
                push!(0x8400);
                call!(0x75b70);
                pc = 0x31cbfc; continue;
            }
            0x31cbfc => {
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                call!(0x71b10);
                pc = 0x31cc0c; continue;
            }
            0x31cc0c => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x31cc16; continue;
            }
            0x31cc16 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x31cc24; continue;
            }
            0x31cc24 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm2 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                xmm0 = Xmm::scalar(m.read32(0x49f7d0));
                push!(ecx);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x78170);
                pc = 0x31cc72; continue;
            }
            0x31cc72 => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xd74);
                call!(0x3d7b0);
                pc = 0x31cc8a; continue;
            }
            0x31cc8a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31cc93; continue;
            }
            0x31cc93 => {
                push!(0x2000);
                eax = esi.wrapping_add(0x20);
                push!(eax);
                push!(0x428c0000);
                push!(0x1f);
                push!(esi);
                push!(0);
                push!(0);
                call!(0x7d560);
                pc = 0x31ccad; continue;
            }
            0x31ccad => {
                push!(0xb7);
                call!(0x716d0);
                pc = 0x31ccb7; continue;
            }
            0x31ccb7 => {
                esp = esp.wrapping_add(0x34);
                call!(0x783d0);
                pc = 0x31ccbf; continue;
            }
            0x31ccbf => {
                push!(0);
                push!(0);
                push!(0x8000800);
                push!(0);
                push!(0xd75);
                call!(0x3d7b0);
                pc = 0x31ccd4; continue;
            }
            0x31ccd4 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31ccdd; continue;
            }
            0x31ccdd => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x31cce6; continue;
            }
            0x31cce6 => {
                push!(0xb8);
                call!(0x716d0);
                pc = 0x31ccf0; continue;
            }
            0x31ccf0 => {
                esp = esp.wrapping_add(0x20);
                edi = 0;
                ebx = ebx.wrapping_add(0x4);
                pc = 0x31ccf8; continue;
            }
            0x31ccf8 => {
                fa = ((((m.read16(ebx))) as u32) & 0xffffu32);
                fb = ((0xffffffffu32) & 0xffffu32);
                push!(edi);
                if (fa == fb) { pc = 0x31cd0a; continue; }
                pc = 0x31ccff; continue;
            }
            0x31ccff => {
                ecx = edi.wrapping_add(0x38);
                push!(ecx);
                call!(0x303350);
                pc = 0x31cd08; continue;
            }
            0x31cd08 => {
                pc = 0x31cd14; continue;
            }
            0x31cd0a => {
                push!(0xd7c);
                call!(0x3d7b0);
                pc = 0x31cd14; continue;
            }
            0x31cd14 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31cd1d; continue;
            }
            0x31cd1d => {
                esp = esp.wrapping_add(0x8);
                edi = edi.wrapping_add(1);
                ebx = ebx.wrapping_add(0x8);
                fa = edi;
                fb = ((((0xai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31ccf8; continue; }
                pc = 0x31cd29; continue;
            }
            0x31cd29 => {
                edx = esi.wrapping_add(0x28);
                push!(edx);
                call!(0x78590);
                pc = 0x31cd32; continue;
            }
            0x31cd32 => {
                esp = esp.wrapping_add(0x4);
                call!(0x783d0);
                pc = 0x31cd3a; continue;
            }
            0x31cd3a => {
                fa = m.read32(ebp);
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31cf25; continue; }
                pc = 0x31cd44; continue;
            }
            0x31cd44 => {
                push!(0);
                push!(0);
                push!(0x8000800);
                push!(0x1);
                push!(0xd76);
                call!(0x3d7b0);
                pc = 0x31cd59; continue;
            }
            0x31cd59 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31cd62; continue;
            }
            0x31cd62 => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x31cd6b; continue;
            }
            0x31cd6b => {
                push!(0xb9);
                call!(0x716d0);
                pc = 0x31cd75; continue;
            }
            0x31cd75 => {
                esp = esp.wrapping_add(0x20);
                push!(0xffffffffu32);
                push!(0x1816);
                call!(0x3d7b0);
                pc = 0x31cd84; continue;
            }
            0x31cd84 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31cd8d; continue;
            }
            0x31cd8d => {
                esp = esp.wrapping_add(0x8);
                edi = 0;
                pc = 0x31cd92; continue;
            }
            0x31cd92 => {
                eax = edi.wrapping_add(0x11);
                push!(edi);
                push!(eax);
                call!(0x303350);
                pc = 0x31cd9c; continue;
            }
            0x31cd9c => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31cda5; continue;
            }
            0x31cda5 => {
                esp = esp.wrapping_add(0x8);
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31cd92; continue; }
                pc = 0x31cdae; continue;
            }
            0x31cdae => {
                ecx = esi.wrapping_add(0x2c);
                push!(ecx);
                call!(0x78590);
                pc = 0x31cdb7; continue;
            }
            0x31cdb7 => {
                push!(0x3f800000);
                call!(0x795b0);
                pc = 0x31cdc1; continue;
            }
            0x31cdc1 => {
                esp = esp.wrapping_add(0x8);
                push!(0);
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0x177f);
                call!(0x3d7b0);
                pc = 0x31cddb; continue;
            }
            0x31cddb => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31cde4; continue;
            }
            0x31cde4 => {
                push!(0);
                push!(0);
                call!(0x7d4f0);
                pc = 0x31cded; continue;
            }
            0x31cded => {
                push!(0xba);
                call!(0x716d0);
                pc = 0x31cdf7; continue;
            }
            0x31cdf7 => {
                edx = esi.wrapping_add(0x30);
                push!(edx);
                call!(0x78560);
                pc = 0x31ce00; continue;
            }
            0x31ce00 => {
                esp = esp.wrapping_add(0x28);
                call!(0x783d0);
                pc = 0x31ce08; continue;
            }
            0x31ce08 => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xd71);
                call!(0x3d7b0);
                pc = 0x31ce1d; continue;
            }
            0x31ce1d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31ce26; continue;
            }
            0x31ce26 => {
                push!(0xbb);
                call!(0x716d0);
                pc = 0x31ce30; continue;
            }
            0x31ce30 => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x31ce39; continue;
            }
            0x31ce39 => {
                esp = esp.wrapping_add(0x20);
                push!(0);
                push!(0xc1f);
                call!(0x3d7b0);
                pc = 0x31ce48; continue;
            }
            0x31ce48 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31ce51; continue;
            }
            0x31ce51 => {
                esp = esp.wrapping_add(0x8);
                eax = 0x1d;
                ebx = 0x4;
                edi = edi;
                pc = 0x31ce60; continue;
            }
            0x31ce60 => {
                edi = eax.wrapping_add(0x1);
                push!(edi);
                push!(eax);
                call!(0x303350);
                pc = 0x31ce6a; continue;
            }
            0x31ce6a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31ce73; continue;
            }
            0x31ce73 => {
                esp = esp.wrapping_add(0x8);
                ebx = ebx.wrapping_sub(1);
                fa = ebx;
                eax = edi;
                if ((fa != 0)) { pc = 0x31ce60; continue; }
                pc = 0x31ce7b; continue;
            }
            0x31ce7b => {
                eax = esi.wrapping_add(0x48);
                push!(eax);
                call!(0x78590);
                pc = 0x31ce84; continue;
            }
            0x31ce84 => {
                esp = esp.wrapping_add(0x4);
                call!(0x783d0);
                pc = 0x31ce8c; continue;
            }
            0x31ce8c => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(0x2);
                push!(0xd77);
                call!(0x3d7b0);
                pc = 0x31cea1; continue;
            }
            0x31cea1 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31ceaa; continue;
            }
            0x31ceaa => {
                push!(0xbc);
                call!(0x716d0);
                pc = 0x31ceb4; continue;
            }
            0x31ceb4 => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x31cebd; continue;
            }
            0x31cebd => {
                esp = esp.wrapping_add(0x20);
                eax = 0;
                pc = 0x31cec2; continue;
            }
            0x31cec2 => {
                edi = eax.wrapping_add(0x1);
                fa = edi;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x31cf10; continue; }
                pc = 0x31ceca; continue;
            }
            0x31ceca => {
                jt = m.read32(edi.wrapping_mul(0x4).wrapping_add(0x31d120));
                if (jt == 0x31ced1u32) { pc = 0x31ced1; continue; }
                if (jt == 0x31ced9u32) { pc = 0x31ced9; continue; }
                if (jt == 0x31cee1u32) { pc = 0x31cee1; continue; }
                if (jt == 0x31cee9u32) { pc = 0x31cee9; continue; }
                if (jt == 0x31cef1u32) { pc = 0x31cef1; continue; }
                if (jt == 0x31cef9u32) { pc = 0x31cef9; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x31ced1 => {
                push!(eax);
                push!(0xc09);
                pc = 0x31ceff; continue;
            }
            0x31ced9 => {
                push!(eax);
                push!(0xc0a);
                pc = 0x31ceff; continue;
            }
            0x31cee1 => {
                push!(eax);
                push!(0xc0b);
                pc = 0x31ceff; continue;
            }
            0x31cee9 => {
                push!(eax);
                push!(0xc00);
                pc = 0x31ceff; continue;
            }
            0x31cef1 => {
                push!(eax);
                push!(0xc01);
                pc = 0x31ceff; continue;
            }
            0x31cef9 => {
                push!(eax);
                push!(0xc02);
                pc = 0x31ceff; continue;
            }
            0x31ceff => {
                call!(0x3d7b0);
                pc = 0x31cf04; continue;
            }
            0x31cf04 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31cf0d; continue;
            }
            0x31cf0d => {
                esp = esp.wrapping_add(0x8);
                pc = 0x31cf10; continue;
            }
            0x31cf10 => {
                eax = edi;
                fa = eax;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31cec2; continue; }
                pc = 0x31cf17; continue;
            }
            0x31cf17 => {
                push!(ebp);
                call!(0x78590);
                pc = 0x31cf1d; continue;
            }
            0x31cf1d => {
                esp = esp.wrapping_add(0x4);
                call!(0x783d0);
                pc = 0x31cf25; continue;
            }
            0x31cf25 => {
                push!(0);
                push!(0);
                push!(0x800);
                push!(0x3);
                push!(0xc0d);
                call!(0x3d7b0);
                pc = 0x31cf3a; continue;
            }
            0x31cf3a => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31cf43; continue;
            }
            0x31cf43 => {
                push!(0);
                push!(0);
                call!(0x7d4a0);
                pc = 0x31cf4c; continue;
            }
            0x31cf4c => {
                push!(0xbd);
                call!(0x716d0);
                pc = 0x31cf56; continue;
            }
            0x31cf56 => {
                esp = esp.wrapping_add(0x20);
                push!(0);
                push!(0xc0e);
                call!(0x3d7b0);
                pc = 0x31cf65; continue;
            }
            0x31cf65 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31cf6e; continue;
            }
            0x31cf6e => {
                esp = esp.wrapping_add(0x8);
                push!(0x1);
                push!(0xc0f);
                call!(0x3d7b0);
                pc = 0x31cf7d; continue;
            }
            0x31cf7d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31cf86; continue;
            }
            0x31cf86 => {
                esp = esp.wrapping_add(0x8);
                push!(0x2);
                push!(0xc10);
                call!(0x3d7b0);
                pc = 0x31cf95; continue;
            }
            0x31cf95 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31cf9e; continue;
            }
            0x31cf9e => {
                esp = esp.wrapping_add(0x8);
                push!(0x3);
                push!(0xc11);
                call!(0x3d7b0);
                pc = 0x31cfad; continue;
            }
            0x31cfad => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31cfb6; continue;
            }
            0x31cfb6 => {
                esp = esp.wrapping_add(0x8);
                push!(0x4);
                push!(0xc12);
                call!(0x3d7b0);
                pc = 0x31cfc5; continue;
            }
            0x31cfc5 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x71800);
                pc = 0x31cfce; continue;
            }
            0x31cfce => {
                ecx = esi.wrapping_add(0x4c);
                push!(ecx);
                call!(0x78590);
                pc = 0x31cfd7; continue;
            }
            0x31cfd7 => {
                esp = esp.wrapping_add(0xc);
                call!(0x783d0);
                pc = 0x31cfdf; continue;
            }
            0x31cfdf => {
                fa = m.read32(ebp);
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31d079; continue; }
                pc = 0x31cfe9; continue;
            }
            0x31cfe9 => {
                edi = m.read32(esp.wrapping_add(0x10));
                push!(0);
                push!(0);
                push!(edi);
                push!(0xffffffffu32);
                push!(0x177d);
                call!(0x3d7b0);
                pc = 0x31cffe; continue;
            }
            0x31cffe => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31d007; continue;
            }
            0x31d007 => {
                push!(0);
                push!(0);
                push!(0x1);
                push!(0x3e7);
                push!(0x1);
                edx = esi.wrapping_add(0x38);
                push!(edx);
                push!(0);
                push!(0);
                call!(0x7d5e0);
                pc = 0x31d021; continue;
            }
            0x31d021 => {
                push!(0xbf);
                call!(0x716d0);
                pc = 0x31d02b; continue;
            }
            0x31d02b => {
                esp = esp.wrapping_add(0x38);
                call!(0x783d0);
                pc = 0x31d033; continue;
            }
            0x31d033 => {
                push!(0);
                push!(0);
                push!(edi);
                push!(0xffffffffu32);
                push!(0x177e);
                call!(0x3d7b0);
                pc = 0x31d044; continue;
            }
            0x31d044 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31d04d; continue;
            }
            0x31d04d => {
                push!(0x20);
                push!(0);
                push!(0x1);
                push!(0x258);
                push!(0);
                esi = esi.wrapping_add(0x3c);
                push!(esi);
                push!(0);
                push!(0);
                call!(0x7d5e0);
                pc = 0x31d067; continue;
            }
            0x31d067 => {
                push!(0xc0);
                call!(0x716d0);
                pc = 0x31d071; continue;
            }
            0x31d071 => {
                esp = esp.wrapping_add(0x38);
                call!(0x783d0);
                pc = 0x31d079; continue;
            }
            0x31d079 => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x31d083; continue;
            }
            0x31d083 => {
                eax = m.read32(esp.wrapping_add(0x20));
                esp = esp.wrapping_add(0x4);
                push!(0);
                push!(eax);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xd73);
                call!(0x3d7b0);
                pc = 0x31d09d; continue;
            }
            0x31d09d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31d0a6; continue;
            }
            0x31d0a6 => {
                call!(0x781a0);
                pc = 0x31d0ab; continue;
            }
            0x31d0ab => {
                call!(0x22b90);
                pc = 0x31d0b0; continue;
            }
            0x31d0b0 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                ecx = m.read32(0x7bb1e8);
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm1.lanes[0]);
                push!(ecx);
                call!(0x77320);
                pc = 0x31d0fb; continue;
            }
            0x31d0fb => {
                esp = esp.wrapping_add(0x8);
                call!(0x75e70);
                pc = 0x31d103; continue;
            }
            0x31d103 => {
                eax = m.read32(esp.wrapping_add(0x18));
                ecx = m.read32(eax.wrapping_add(0x218));
                edx = m.read32(esp.wrapping_add(0x20));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = (ecx | edx);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax.wrapping_add(0x218), ecx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031D140: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31d140<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31d140;
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
        { crate::pose::cover(0x31d140, previous, pc); previous = pc; }
        match pc {
            0x31d140 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(0x4);
                push!(0x31cac0);
                push!(eax);
                call!(0x31cb90);
                pc = 0x31d151; continue;
            }
            0x31d151 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031D160: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31d160<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31d160;
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
        { crate::pose::cover(0x31d160, previous, pc); previous = pc; }
        match pc {
            0x31d160 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(0x5);
                push!(0x2fb3c0);
                push!(eax);
                call!(0x31cb90);
                pc = 0x31d171; continue;
            }
            0x31d171 => {
                esp = esp.wrapping_add(0xc);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031D180: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31d180<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31d180;
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
        { crate::pose::cover(0x31d180, previous, pc); previous = pc; }
        match pc {
            0x31d180 => {
                eax = m.read32(0x7844a8);
                eax = m.read32(eax.wrapping_add(0x2f20));
                push!(ebx);
                push!(esi);
                push!(edi);
                push!(0x2fb3c0);
                push!(0);
                push!(0x425c0000);
                esi = eax;
                eax = (eax & 0xffff);
                push!(0);
                push!(0x8400);
                esi = (((((esi as i32)).wrapping_shr((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31)))) as u32);
                edi = eax;
                call!(0x75b70);
                pc = 0x31d1b2; continue;
            }
            0x31d1b2 => {
                ebx = m.read32(esp.wrapping_add(0x24));
                push!(0x1);
                push!(0x1);
                push!(0x3f000000);
                push!(0x1);
                m.write32(ebx.wrapping_add(0x218), 0x1);
                call!(0x71b10);
                pc = 0x31d1d0; continue;
            }
            0x31d1d0 => {
                push!(0x20000);
                call!(0x71c50);
                pc = 0x31d1da; continue;
            }
            0x31d1da => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0x4000);
                push!(0xffffffffu32);
                push!(0xc1b);
                call!(0x3d7b0);
                pc = 0x31d1f2; continue;
            }
            0x31d1f2 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79de0);
                pc = 0x31d1fb; continue;
            }
            0x31d1fb => {
                push!(0);
                push!(0x4000);
                call!(0x7d4d0);
                pc = 0x31d207; continue;
            }
            0x31d207 => {
                push!(0xc1d);
                call!(0x3d7b0);
                pc = 0x31d211; continue;
            }
            0x31d211 => {
                push!(eax);
                call!(0x717d0);
                pc = 0x31d217; continue;
            }
            0x31d217 => {
                push!(0xc1);
                call!(0x716d0);
                pc = 0x31d221; continue;
            }
            0x31d221 => {
                push!(0xc1e);
                call!(0x3d7b0);
                pc = 0x31d22b; continue;
            }
            0x31d22b => {
                push!(eax);
                call!(0x717d0);
                pc = 0x31d231; continue;
            }
            0x31d231 => {
                push!(0xc2);
                call!(0x716d0);
                pc = 0x31d23b; continue;
            }
            0x31d23b => {
                esi = ((esi as i32).wrapping_mul((0xe4) as i32)) as u32;
                esi = esi.wrapping_add(m.read32(0x7844a8));
                edi = edi.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = edi.wrapping_add(esi).wrapping_add(0x2f8);
                push!(ecx);
                call!(0x78560);
                pc = 0x31d257; continue;
            }
            0x31d257 => {
                call!(0x783d0);
                pc = 0x31d25c; continue;
            }
            0x31d25c => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x31d266; continue;
            }
            0x31d266 => {
                esp = esp.wrapping_add(0x3c);
                push!(0);
                push!(0x2fb3c0);
                push!(0x40000000);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x31d283; continue;
            }
            0x31d283 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31d28c; continue;
            }
            0x31d28c => {
                call!(0x75e70);
                pc = 0x31d291; continue;
            }
            0x31d291 => {
                xmm0 = Xmm::scalar(m.read32(0x49f7e8));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(ebx.wrapping_add(0x68)))).to_bits();
                edx = m.read32(0x7bb1e8);
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm0.lanes[0]);
                push!(edx);
                call!(0x77320);
                pc = 0x31d2b2; continue;
            }
            0x31d2b2 => {
                esp = esp.wrapping_add(0x8);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031D3E0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31d3e0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31d3e0;
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
        { crate::pose::cover(0x31d3e0, previous, pc); previous = pc; }
        match pc {
            0x31d3e0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(0x7844a8);
                eax = ((eax as i32).wrapping_mul((0xe4) as i32)) as u32;
                eax = eax.wrapping_add(ecx).wrapping_add(0x288);
                fa = m.read32(eax.wrapping_add(0x34));
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31d403; continue; }
                pc = 0x31d3fd; continue;
            }
            0x31d3fd => {
                eax = 0x9;
                break 'run;
            }
            0x31d403 => {
                eax = m.read32(eax.wrapping_add(0x28));
                eax = eax.wrapping_add(0x38);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031D580: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31d580<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31d580;
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
        { crate::pose::cover(0x31d580, previous, pc); previous = pc; }
        match pc {
            0x31d580 => {
                esp = esp.wrapping_sub(0x1c);
                push!(ebx);
                push!(0x2fb090);
                push!(0);
                push!(0x425c0000);
                push!(0);
                push!(0x8400);
                call!(0x75b70);
                pc = 0x31d59c; continue;
            }
            0x31d59c => {
                eax = m.read32(esp.wrapping_add(0x38));
                push!(0x1);
                push!(0x1);
                push!(0);
                push!(0);
                m.write32(eax.wrapping_add(0x218), 0x1);
                call!(0x71b10);
                pc = 0x31d5b7; continue;
            }
            0x31d5b7 => {
                push!(0x28000);
                call!(0x71c50);
                pc = 0x31d5c1; continue;
            }
            0x31d5c1 => {
                esp = esp.wrapping_add(0x28);
                push!(0);
                push!(0);
                push!(0);
                call!(0x22b90);
                pc = 0x31d5cf; continue;
            }
            0x31d5cf => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm2 = xmm0;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                xmm0 = Xmm::scalar(m.read32(0x49f7d0));
                push!(ecx);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                call!(0x78170);
                pc = 0x31d61d; continue;
            }
            0x31d61d => {
                esp = esp.wrapping_add(0x10);
                push!(0);
                push!(0x31be70);
                push!(0x41100000);
                push!(0x800);
                push!(0xffffffffu32);
                push!(0xb46);
                call!(0x3d7b0);
                pc = 0x31d63d; continue;
            }
            0x31d63d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31d646; continue;
            }
            0x31d646 => {
                push!(0xb0);
                call!(0x716d0);
                pc = 0x31d650; continue;
            }
            0x31d650 => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x31d65a; continue;
            }
            0x31d65a => {
                ecx = m.read32(0x7844a8);
                eax = m.read32(ecx.wrapping_add(0x280));
                ebx = 0;
                esp = esp.wrapping_add(0x20);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x24), ebx);
                if (fas <= fbs) { pc = 0x31d979; continue; }
                pc = 0x31d677; continue;
            }
            0x31d677 => {
                push!(ebp);
                push!(esi);
                esi = 0;
                push!(edi);
                m.write32(esp.wrapping_add(0x24), esi);
                pc = 0x31d680; continue;
            }
            0x31d680 => {
                call!(0x71c60);
                pc = 0x31d685; continue;
            }
            0x31d685 => {
                { let value = round_pc(fpu.st(0) - (f32::from_bits(m.read32(0x49f7f8)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                eax = m.read32(0x7844a8);
                ecx = m.read32(esi.wrapping_add(eax).wrapping_add(0x2e0));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x28), (fpu.st(0) as f32).to_bits()); fpu.pop();
                ebp = esi.wrapping_add(eax).wrapping_add(0x2e0);
                if (fa == fb) { pc = 0x31d940; continue; }
                pc = 0x31d6aa; continue;
            }
            0x31d6aa => {
                edi = esi.wrapping_add(eax).wrapping_add(0x2bc);
                eax = m.read32(edi);
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x31d6c4; continue; }
                pc = 0x31d6b6; continue;
            }
            0x31d6b6 => {
                eax = eax.wrapping_sub(0x2);
                fa = eax;
                if ((fa == 0)) { pc = 0x31d6c4; continue; }
                pc = 0x31d6bb; continue;
            }
            0x31d6bb => {
                eax = eax.wrapping_sub(0x2);
                fa = eax;
                if ((fa != 0)) { pc = 0x31d940; continue; }
                pc = 0x31d6c4; continue;
            }
            0x31d6c4 => {
                push!(0x28c);
                ebx = ebx.wrapping_shl((((((((0x10i32)) as u32) & 0x1fu32))) as u32 & 31));
                m.write32(esp.wrapping_add(0x1c), 0);
                call!(0x80f20);
                pc = 0x31d6d9; continue;
            }
            0x31d6d9 => {
                push!(0x31);
                esi = eax;
                call!(0x81c20);
                pc = 0x31d6e2; continue;
            }
            0x31d6e2 => {
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d4c))).to_bits();
                push!(0x31);
                m.write32(esp.wrapping_add(0x2c), xmm0.lanes[0]);
                call!(0x81c40);
                pc = 0x31d6fb; continue;
            }
            0x31d6fb => {
                esi = ((esi as i32).wrapping_mul((0x34) as i32)) as u32;
                edx = m.read32(0x4b85c8);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                eax = m.read32(edx);
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d4c))).to_bits();
                ecx = ((m.read16(esi.wrapping_add(eax).wrapping_add(0x26))) as u16 as u32);
                m.write32(esp.wrapping_add(0x28), xmm0.lanes[0]);
                xmm0.lanes[0] = (((ecx) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475dc4))).to_bits();
                push!(0x31);
                m.write32(esp.wrapping_add(0x24), xmm0.lanes[0]);
                call!(0x81c40);
                pc = 0x31d736; continue;
            }
            0x31d736 => {
                ecx = m.read32(edi);
                edx = 0;
                esp = esp.wrapping_add(0x10);
                fa = ecx;
                fb = ((((0x5i32)) as u32) & 0xffffffffu32);
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                xmm0.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d4c))).to_bits();
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                esi = edx;
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31d7c7; continue; }
                pc = 0x31d75b; continue;
            }
            0x31d75b => {
                push!(0x2);
                push!(0);
                push!(0);
                call!(0x78320);
                pc = 0x31d766; continue;
            }
            0x31d766 => {
                eax = m.read32(esp.wrapping_add(0x34));
                edi = m.read32(esp.wrapping_add(0x3c));
                push!(0x31bf60);
                push!(0x31bf00);
                push!(0x412ccccc);
                push!(eax);
                push!(0x40000);
                push!(edi);
                push!(0);
                call!(0x792b0);
                pc = 0x31d78b; continue;
            }
            0x31d78b => {
                push!(0xb1);
                call!(0x716d0);
                pc = 0x31d795; continue;
            }
            0x31d795 => {
                push!(0x31c7c0);
                push!(0x31c770);
                push!(0x412ccccc);
                push!(0);
                push!(0);
                push!(edi);
                push!(0);
                call!(0x792b0);
                pc = 0x31d7b0; continue;
            }
            0x31d7b0 => {
                esp = esp.wrapping_add(0x48);
                push!(0xb3);
                call!(0x716d0);
                pc = 0x31d7bd; continue;
            }
            0x31d7bd => {
                esp = esp.wrapping_add(0x4);
                call!(0x78380);
                pc = 0x31d7c5; continue;
            }
            0x31d7c5 => {
                pc = 0x31d7de; continue;
            }
            0x31d7c7 => {
                ecx = m.read32(esp.wrapping_add(0x30));
                push!(0x31c480);
                push!(0x31bf00);
                push!(ecx);
                call!(0x31d4c0);
                pc = 0x31d7db; continue;
            }
            0x31d7db => {
                esp = esp.wrapping_add(0xc);
                pc = 0x31d7de; continue;
            }
            0x31d7de => {
                push!(0x10);
                push!(0);
                push!(0);
                call!(0x78320);
                pc = 0x31d7e9; continue;
            }
            0x31d7e9 => {
                eax = m.read32(ebp);
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0x8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x31d8ce; continue; }
                pc = 0x31d7f8; continue;
            }
            0x31d7f8 => {
                edx = m.read32(esp.wrapping_add(0x10));
                eax = m.read32(esp.wrapping_add(0x14));
                push!(0x31c6b0);
                push!(0x31c650);
                push!(edx);
                push!(eax);
                push!(0);
                push!(ebx);
                push!(0);
                call!(0x792b0);
                pc = 0x31d816; continue;
            }
            0x31d816 => {
                edi = esi;
                cf = (((((edi) != 0))) as i32);
                edi = (((((edi) as i32).wrapping_neg())) as u32);
                edi = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                edi = (edi & 0x7);
                edi = edi.wrapping_add(0xae);
                push!(edi);
                call!(0x716d0);
                pc = 0x31d82b; continue;
            }
            0x31d82b => {
                eax = m.read32(ebp);
                esp = esp.wrapping_add(0x20);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31d931; continue; }
                pc = 0x31d839; continue;
            }
            0x31d839 => {
                cf = (((((esi) != 0))) as i32);
                esi = (((((esi) as i32).wrapping_neg())) as u32);
                esi = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                esi = (esi & 0x7);
                esi = esi.wrapping_add(0xad);
                eax = (eax | 0xffffffffu32);
                eax = eax.wrapping_sub(ebx);
                m.write32(esp.wrapping_add(0x28), esi);
                esi = ebx.wrapping_add(0x1);
                ebx = m.read32(esp.wrapping_add(0x20));
                m.write32(esp.wrapping_add(0x18), eax);
                pc = 0x31d860; continue;
            }
            0x31d860 => {
                ecx = m.read32(esp.wrapping_add(0x1c));
                push!(0x31c620);
                push!(0x31c570);
                push!(ecx);
                push!(ebx);
                push!(0);
                edx = esi.wrapping_add(((0x1i32.wrapping_neg()) as u32));
                push!(edx);
                push!(0);
                call!(0x792b0);
                pc = 0x31d87d; continue;
            }
            0x31d87d => {
                eax = m.read32(esp.wrapping_add(0x44));
                push!(eax);
                call!(0x716d0);
                pc = 0x31d887; continue;
            }
            0x31d887 => {
                ecx = m.read32(esp.wrapping_add(0x30));
                edx = m.read32(esp.wrapping_add(0x34));
                push!(0x31c6b0);
                push!(0x31c650);
                push!(ecx);
                push!(edx);
                push!(0);
                push!(esi);
                push!(0);
                call!(0x792b0);
                pc = 0x31d8a5; continue;
            }
            0x31d8a5 => {
                push!(edi);
                esi = esi.wrapping_add(1);
                call!(0x716d0);
                pc = 0x31d8ac; continue;
            }
            0x31d8ac => {
                eax = m.read32(esp.wrapping_add(0x58));
                ecx = m.read32(ebp);
                eax = eax.wrapping_add(esi);
                esp = esp.wrapping_add(0x40);
                fa = eax;
                fb = ecx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31d860; continue; }
                pc = 0x31d8bc; continue;
            }
            0x31d8bc => {
                call!(0x78380);
                pc = 0x31d8c1; continue;
            }
            0x31d8c1 => {
                ebx = m.read32(esp.wrapping_add(0x30));
                esi = m.read32(esp.wrapping_add(0x24));
                pc = 0x31d953; continue;
            }
            0x31d8ce => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x31d931; continue; }
                pc = 0x31d8d2; continue;
            }
            0x31d8d2 => {
                cf = (((((esi) != 0))) as i32);
                esi = (((((esi) as i32).wrapping_neg())) as u32);
                esi = if (cf) != 0 { 0xffffffffu32 } else { 0 };
                esi = (esi & 0x6);
                esi = esi.wrapping_add(0xae);
                edi = ebx;
                pc = 0x31d8e1; continue;
            }
            0x31d8e1 => {
                ecx = m.read32(esp.wrapping_add(0x10));
                edx = m.read32(esp.wrapping_add(0x14));
                push!(0);
                push!(0);
                push!(ecx);
                push!(edx);
                push!(0x4);
                push!(edi);
                push!(0);
                call!(0x792b0);
                pc = 0x31d8f9; continue;
            }
            0x31d8f9 => {
                eax = m.read32(esp.wrapping_add(0x38));
                ecx = m.read32(esp.wrapping_add(0x3c));
                ebx = m.read32(esp.wrapping_add(0x34));
                push!(0x31c620);
                push!(0x31c570);
                push!(eax);
                push!(ecx);
                push!(0);
                push!(edi);
                ebx = ebx.wrapping_add(1);
                push!(0);
                m.write32(esp.wrapping_add(0x50), ebx);
                call!(0x792b0);
                pc = 0x31d920; continue;
            }
            0x31d920 => {
                push!(esi);
                edi = edi.wrapping_add(1);
                call!(0x716d0);
                pc = 0x31d927; continue;
            }
            0x31d927 => {
                eax = m.read32(ebp);
                esp = esp.wrapping_add(0x3c);
                fa = ebx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31d8e1; continue; }
                pc = 0x31d931; continue;
            }
            0x31d931 => {
                call!(0x78380);
                pc = 0x31d936; continue;
            }
            0x31d936 => {
                ebx = m.read32(esp.wrapping_add(0x30));
                esi = m.read32(esp.wrapping_add(0x24));
                pc = 0x31d953; continue;
            }
            0x31d940 => {
                push!(0x31c480);
                push!(0x31bf00);
                push!(ebx);
                call!(0x31d4c0);
                pc = 0x31d950; continue;
            }
            0x31d950 => {
                esp = esp.wrapping_add(0xc);
                pc = 0x31d953; continue;
            }
            0x31d953 => {
                edx = m.read32(0x7844a8);
                eax = m.read32(edx.wrapping_add(0x280));
                ebx = ebx.wrapping_add(1);
                esi = esi.wrapping_add(0xe4);
                fa = ebx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x30), ebx);
                m.write32(esp.wrapping_add(0x24), esi);
                if (fas < fbs) { pc = 0x31d680; continue; }
                pc = 0x31d976; continue;
            }
            0x31d976 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x31d979; continue;
            }
            0x31d979 => {
                call!(0x781a0);
                pc = 0x31d97e; continue;
            }
            0x31d97e => {
                call!(0x22b90);
                pc = 0x31d983; continue;
            }
            0x31d983 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                eax = m.read32(0x7bb1e8);
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                push!(ecx);
                m.write32(esp, xmm1.lanes[0]);
                push!(eax);
                call!(0x77320);
                pc = 0x31d9cb; continue;
            }
            0x31d9cb => {
                esp = esp.wrapping_add(0x8);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x1c);
                tail_to!(0x75e70); break 'run;  // direct tail jump
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031DC10: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31dc10<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31dc10;
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
        { crate::pose::cover(0x31dc10, previous, pc); previous = pc; }
        match pc {
            0x31dc10 => {
                edx = m.read32(esp.wrapping_add(0x4));
                push!(esi);
                esi = 0;
                ecx = 0;
                eax = 0x52e948;
                edi = edi;
                pc = 0x31dc20; continue;
            }
            0x31dc20 => {
                fa = m.read32(eax);
                fb = edx;
                if (fa == fb) { pc = 0x31dc34; continue; }
                pc = 0x31dc24; continue;
            }
            0x31dc24 => {
                eax = eax.wrapping_add(0x20);
                ecx = ecx.wrapping_add(1);
                fa = eax;
                fb = ((((0x52e9c8i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31dc20; continue; }
                pc = 0x31dc2f; continue;
            }
            0x31dc2f => {
                eax = m.read32(esi.wrapping_add(0x10));
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x31dc34 => {
                ecx = ecx.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = ecx.wrapping_add(0x52e948);
                eax = m.read32(ecx.wrapping_add(0x10));
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031EAB0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31eab0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31eab0;
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
        { crate::pose::cover(0x31eab0, previous, pc); previous = pc; }
        match pc {
            0x31eab0 => {
                xmm1 = Xmm::scalar(m.read32(0x7ba188));
                xmm0 = xmm1;
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475cd4))).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x31eb3e; continue; }
                pc = 0x31ead1; continue;
            }
            0x31ead1 => {
                xmm2.lanes[0] = (((m.read32(0x4e8cf4)) as i32) as f32).to_bits();
                fca = f32::from_bits(xmm2.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x31eb3e; continue; }
                pc = 0x31eade; continue;
            }
            0x31eade => {
                edx = m.read32(ecx.wrapping_add(0xf4));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x4760cc))).to_bits();
                push!(edx);
                ecx = sse::cvttss2si(f32::from_bits(xmm1.lanes[0]));
                xmm1 = Xmm::scalar(m.read32(0x7ba038));
                push!(ecx);
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                eax = m.read32(esp.wrapping_add(0xc));
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x47d61c))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d18))).to_bits();
                edx = sse::cvttss2si(f32::from_bits(xmm2.lanes[0]));
                push!(edx);
                push!(ecx);
                eax = eax.wrapping_add(0xfffffffdu32);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                eax = eax.wrapping_shl((((((((0x5i32)) as u32) & 0x1fu32))) as u32 & 31));
                ecx = m.read32(eax.wrapping_add(0x52e95c));
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(edx);
                push!(ecx);
                call!(0x2bf4e0);
                pc = 0x31eb3b; continue;
            }
            0x31eb3b => {
                esp = esp.wrapping_add(0x18);
                pc = 0x31eb3e; continue;
            }
            0x31eb3e => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031EC90: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31ec90<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31ec90;
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
        { crate::pose::cover(0x31ec90, previous, pc); previous = pc; }
        match pc {
            0x31ec90 => {
                eax = m.read32(0x7844a8);
                eax = (((((m.read8(eax.wrapping_add(0x11854)) as i8)) as i32)) as u32);
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x31ecbd; continue; }
                pc = 0x31ec9f; continue;
            }
            0x31ec9f => {
                push!(0);
                push!(0);
                push!(0x1a26);
                call!(0x3d7b0);
                pc = 0x31ecad; continue;
            }
            0x31ecad => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0);
                push!(esi);
                call!(0x74990);
                pc = 0x31ecb9; continue;
            }
            0x31ecb9 => {
                esp = esp.wrapping_add(0x14);
                break 'run;
            }
            0x31ecbd => {
                push!(0x1);
                push!(esi);
                call!(0x314bd0);
                pc = 0x31ecc5; continue;
            }
            0x31ecc5 => {
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}

/// 0x0031EE30: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31ee30<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31ee30;
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
        { crate::pose::cover(0x31ee30, previous, pc); previous = pc; }
        match pc {
            0x31ee30 => {
                eax = m.read32(0x763080);
                esp = esp.wrapping_sub(0x68);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31f20d; continue; }
                pc = 0x31ee40; continue;
            }
            0x31ee40 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x80)));
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = (f32::from_bits(m.read32(0x475cac)) as f64);
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x31f20d; continue; }
                pc = 0x31ee56; continue;
            }
            0x31ee56 => {
                xmm1.lanes[0] = (((m.read32(0x4e8cf4)) as i32) as f32).to_bits();
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca < fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x31f20d; continue; }
                pc = 0x31ee67; continue;
            }
            0x31ee67 => {
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x7c)));
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x74));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x40), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x44), xmm0.lanes[0]);
                if (fas < fbs) { pc = 0x31eeab; continue; }
                pc = 0x31ee82; continue;
            }
            0x31ee82 => {
                eax = m.read32(esp.wrapping_add(0x8c));
                ecx = m.read32(esp.wrapping_add(0x88));
                edx = m.read32(esp.wrapping_add(0x80));
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x88));
                push!(eax);
                push!(ecx);
                push!(eax);
                push!(edx);
                call!(0x1afa0);
                pc = 0x31eea8; continue;
            }
            0x31eea8 => {
                esp = esp.wrapping_add(0x14);
                pc = 0x31eeab; continue;
            }
            0x31eeab => {
                eax = m.read32(0x763084);
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                ecx = m.read32(eax.wrapping_mul(0x8).wrapping_add(0x783a20));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = eax.wrapping_mul(0x8).wrapping_add(0x783a20);
                if (fas < fbs) { pc = 0x31ef95; continue; }
                pc = 0x31eec9; continue;
            }
            0x31eec9 => {
                push!(ebx);
                edx = eax;
                esi = m.read32(edx);
                push!(ebp);
                push!(edi);
                edi = m.read32(edx.wrapping_add(0x4));
                ebx = m.read32(edx.wrapping_add(0x8));
                ebp = m.read32(edx.wrapping_add(0xc));
                ecx = m.read32(edx.wrapping_add(0x10));
                edx = m.read32(edx.wrapping_add(0x14));
                m.write32(esp.wrapping_add(0x74), edx);
                edx = m.read32(esp.wrapping_add(0x80));
                m.write32(eax, edx);
                edx = m.read32(esp.wrapping_add(0x4c));
                m.write32(eax.wrapping_add(0x4), edx);
                edx = m.read32(esp.wrapping_add(0x50));
                m.write32(eax.wrapping_add(0x8), edx);
                edx = m.read32(esp.wrapping_add(0x98));
                m.write32(eax.wrapping_add(0xc), edx);
                edx = m.read32(esp.wrapping_add(0x84));
                m.write32(eax.wrapping_add(0x10), edx);
                edx = m.read32(esp.wrapping_add(0x88));
                m.write32(eax.wrapping_add(0x14), edx);
                m.write32(esp.wrapping_add(0x70), ecx);
                edx = esi;
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x58), ecx);
                ecx = m.read32(esp.wrapping_add(0x74));
                eax = ebp;
                m.write32(esp.wrapping_add(0x4c), edi);
                m.write32(esp.wrapping_add(0x50), ebx);
                m.write32(esp.wrapping_add(0x54), eax);
                m.write32(esp.wrapping_add(0x5c), ecx);
                if (fas < fbs) { pc = 0x31f209; continue; }
                pc = 0x31ef40; continue;
            }
            0x31ef40 => {
                xmm0 = Xmm::default();
                m.write32(esp.wrapping_add(0x18), eax);
                eax = m.read32(esp.wrapping_add(0x7c));
                fpu.push((f32::from_bits(m.read32(eax.wrapping_add(0xf4))) as f64));
                m.write32(esp.wrapping_add(0x14), 0xffffffffu32);
                { let value = round_pc(fpu.st(0) * (f32::from_bits(m.read32(0x475cc0)) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                call!(0x3c848c);
                pc = 0x31ef6a; continue;
            }
            0x31ef6a => {
                eax = (eax & 0xff);
                edx = esi;
                m.write32(esp.wrapping_add(0x1c), eax);
                eax = edx;
                ecx = 0;
                eax = eax.wrapping_sub(ecx);
                fa = eax;
                m.write32(esp.wrapping_add(0x48), edx);
                if ((fa == 0)) { pc = 0x31efd0; continue; }
                pc = 0x31ef81; continue;
            }
            0x31ef81 => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa == 0)) { pc = 0x31efc2; continue; }
                pc = 0x31ef84; continue;
            }
            0x31ef84 => {
                eax = eax.wrapping_sub(1);
                fa = eax;
                if ((fa != 0)) { pc = 0x31efe6; continue; }
                pc = 0x31ef87; continue;
            }
            0x31ef87 => {
                xmm0 = Xmm::scalar(m.read32(0x4760cc));
                m.write32(esp.wrapping_add(0x14), ecx);
                pc = 0x31efec; continue;
            }
            0x31ef95 => {
                edx = m.read32(esp.wrapping_add(0x40));
                ecx = m.read32(esp.wrapping_add(0x44));
                m.write32(eax, esi);
                m.write32(eax.wrapping_add(0x4), edx);
                edx = m.read32(esp.wrapping_add(0x8c));
                m.write32(eax.wrapping_add(0x8), ecx);
                ecx = m.read32(esp.wrapping_add(0x78));
                m.write32(eax.wrapping_add(0xc), edx);
                edx = m.read32(esp.wrapping_add(0x7c));
                m.write32(eax.wrapping_add(0x10), ecx);
                m.write32(eax.wrapping_add(0x14), edx);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x68);
                break 'run;
            }
            0x31efc2 => {
                xmm0 = Xmm::scalar(m.read32(0x4760cc));
                m.write32(esp.wrapping_add(0x14), ecx);
                pc = 0x31efe0; continue;
            }
            0x31efd0 => {
                xmm0 = Xmm::scalar(m.read32(0x475d10));
                m.write32(esp.wrapping_add(0x14), 0x3);
                pc = 0x31efe0; continue;
            }
            0x31efe0 => {
                m.write32(esp.wrapping_add(0x10), xmm0.lanes[0]);
                pc = 0x31efe6; continue;
            }
            0x31efe6 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                pc = 0x31efec; continue;
            }
            0x31efec => {
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = m.read32(esp.wrapping_add(0x70));
                ecx = m.read32(esp.wrapping_add(0x74));
                m.write32(esp.wrapping_add(0x34), edi);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x34)));
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x20), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x38), ebx);
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x38)));
                xmm3 = xmm2;
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                m.write32(esp.wrapping_add(0x2c), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x28), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x64), edi);
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x64)));
                xmm2 = xmm1;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x30), esi);
                m.write32(esp.wrapping_add(0x30), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0x24), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x68), ebx);
                xmm2 = Xmm::scalar(m.read32(esp.wrapping_add(0x68)));
                xmm3 = xmm2;
                m.write32(esp.wrapping_add(0x3c), ebp);
                xmm3.lanes[0] = sse::subss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm2.lanes[0] = sse::addss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x40), eax);
                m.write32(esp.wrapping_add(0x44), ecx);
                m.write32(esp.wrapping_add(0x60), esi);
                m.write32(esp.wrapping_add(0x6c), ebp);
                m.write32(esp.wrapping_add(0x70), eax);
                m.write32(esp.wrapping_add(0x74), ecx);
                m.write32(esp.wrapping_add(0x34), xmm3.lanes[0]);
                m.write32(esp.wrapping_add(0x38), xmm1.lanes[0]);
                m.write32(esp.wrapping_add(0x3c), xmm2.lanes[0]);
                if (fa == fb) { pc = 0x31f10c; continue; }
                pc = 0x31f0a4; continue;
            }
            0x31f0a4 => {
                eax = m.read32(0x763068);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31f10c; continue; }
                pc = 0x31f0ad; continue;
            }
            0x31f0ad => {
                ecx = esp.wrapping_add(0x30);
                push!(ecx);
                call!(0x711f0);
                pc = 0x31f0b7; continue;
            }
            0x31f0b7 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31f10c; continue; }
                pc = 0x31f0be; continue;
            }
            0x31f0be => {
                esi = m.read32(esp.wrapping_add(0x7c));
                fa = ((((m.read8(esi.wrapping_add(0xc4)))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x31f0f6; continue; }
                pc = 0x31f0cb; continue;
            }
            0x31f0cb => {
                eax = m.read32(esp.wrapping_add(0x5c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31f0df; continue; }
                pc = 0x31f0d3; continue;
            }
            0x31f0d3 => {
                edx = m.read32(esp.wrapping_add(0x58));
                push!(edx);
                call_to!(eax);
                pc = 0x31f0da; continue;
            }
            0x31f0da => {
                esp = esp.wrapping_add(0x4);
                pc = 0x31f0ed; continue;
            }
            0x31f0df => {
                eax = m.read32(esp.wrapping_add(0x58));
                edx = (edx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x18))) as u32)) & 0xff);
                ecx = (ecx & 0xffff_ff00) | ((0x1) & 0xff);
                ecx = (ecx & 0xffff_ff00) | ((((((((ecx) as u8)) as i32).wrapping_sub(((((edx) as u8)) as i32))) as u32)) & 0xff);
                m.write8(eax.wrapping_add(0x18), (((((ecx) as u8)) as u32)) as u8);
                pc = 0x31f0ed; continue;
            }
            0x31f0ed => {
                push!(esi);
                call!(0x746d0);
                pc = 0x31f0f3; continue;
            }
            0x31f0f3 => {
                esp = esp.wrapping_add(0x4);
                pc = 0x31f0f6; continue;
            }
            0x31f0f6 => {
                edx = m.read32(esp.wrapping_add(0x54));
                push!(0x3fb33333);
                push!(edx);
                call!(0x154e90);
                pc = 0x31f105; continue;
            }
            0x31f105 => {
                esp = esp.wrapping_add(0x8);
                m.write32(esp.wrapping_add(0x18), eax);
                pc = 0x31f10c; continue;
            }
            0x31f10c => {
                eax = m.read32(esp.wrapping_add(0x14));
                push!(eax);
                call!(0x80f20);
                pc = 0x31f116; continue;
            }
            0x31f116 => {
                esi = eax;
                push!(esi);
                call!(0x1ed50);
                pc = 0x31f11e; continue;
            }
            0x31f11e => {
                esi = ((esi as i32).wrapping_mul((0x34) as i32)) as u32;
                ecx = m.read32(0x4b85c8);
                edx = m.read32(ecx);
                ecx = ((m.read16(esi.wrapping_add(edx).wrapping_add(0x2a))) as u16 as u32);
                eax = m.read32(esp.wrapping_add(0x20));
                esi = esi.wrapping_add(edx);
                edx = ((m.read16(esi.wrapping_add(0x26))) as u16 as u32);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x38));
                ecx = ecx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(ecx);
                ecx = m.read32(esp.wrapping_add(0x38));
                edx = edx.wrapping_shl((((((((0x4i32)) as u32) & 0x1fu32))) as u32 & 31));
                push!(edx);
                edx = m.read32(esp.wrapping_add(0x38));
                push!(0);
                push!(0);
                push!(eax);
                eax = m.read32(esp.wrapping_add(0x40));
                push!(ecx);
                push!(edx);
                push!(eax);
                call!(0x1b7b0);
                pc = 0x31f15e; continue;
            }
            0x31f15e => {
                eax = m.read32(esp.wrapping_add(0x74));
                esp = esp.wrapping_add(0x2c);
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31f1a1; continue; }
                pc = 0x31f16a; continue;
            }
            0x31f16a => {
                ecx = m.read32(esp.wrapping_add(0x1c));
                eax = (((((f32::from_bits(m.read32(esp.wrapping_add(0x50))) as f64)) as i32)) as u32);
                edx = (((((f32::from_bits(m.read32(esp.wrapping_add(0x28))) as f64)) as i32)) as u32);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                push!(ecx);
                push!(eax);
                push!(edx);
                push!(eax);
                eax = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(eax);
                call!(0x1b090);
                pc = 0x31f196; continue;
            }
            0x31f196 => {
                esp = esp.wrapping_add(0x14);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x68);
                break 'run;
            }
            0x31f1a1 => {
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x31f209; continue; }
                pc = 0x31f1a6; continue;
            }
            0x31f1a6 => {
                esi = m.read32(esp.wrapping_add(0x1c));
                eax = (((((f32::from_bits(m.read32(esp.wrapping_add(0x50))) as f64)) as i32)) as u32);
                ecx = (((((f32::from_bits(m.read32(esp.wrapping_add(0x28))) as f64)) as i32)) as u32);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x20)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                push!(esi);
                push!(eax);
                push!(ecx);
                push!(eax);
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(edx);
                call!(0x1b090);
                pc = 0x31f1d2; continue;
            }
            0x31f1d2 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x40)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475c78))).to_bits();
                eax = (((((f32::from_bits(m.read32(esp.wrapping_add(0x60))) as f64)) as i32)) as u32);
                push!(esi);
                ecx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x3c)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d18))).to_bits();
                push!(ecx);
                push!(eax);
                edx = sse::cvttss2si(f32::from_bits(xmm0.lanes[0]));
                push!(edx);
                push!(eax);
                call!(0x1b090);
                pc = 0x31f206; continue;
            }
            0x31f206 => {
                esp = esp.wrapping_add(0x28);
                pc = 0x31f209; continue;
            }
            0x31f209 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x31f20d; continue;
            }
            0x31f20d => {
                esp = esp.wrapping_add(0x68);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x0031FDC0: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f31fdc0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x31fdc0;
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
        { crate::pose::cover(0x31fdc0, previous, pc); previous = pc; }
        match pc {
            0x31fdc0 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x248));
                eax = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x52e6ac));
                push!(ebx);
                push!(esi);
                push!(edi);
                push!(0x361710);
                push!(0);
                push!(0x42480000);
                push!(eax);
                call!(0x3d7b0);
                pc = 0x31fde7; continue;
            }
            0x31fde7 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x8400);
                call!(0x75b70);
                pc = 0x31fdf5; continue;
            }
            0x31fdf5 => {
                push!(0x1);
                push!(0x1);
                push!(0x40000000);
                push!(0x1);
                call!(0x71b10);
                pc = 0x31fe05; continue;
            }
            0x31fe05 => {
                push!(0x28000);
                call!(0x71c50);
                pc = 0x31fe0f; continue;
            }
            0x31fe0f => {
                esp = esp.wrapping_add(0x28);
                edi = 0x2;
                ebx = 0x52e954;
                pc = 0x31fe20; continue;
            }
            0x31fe20 => {
                ecx = m.read32(0x7844a8);
                edx = m.read32(ecx.wrapping_add(0x248));
                eax = edi.wrapping_add(edx.wrapping_mul(0x4));
                eax = eax.wrapping_add(eax.wrapping_mul(0x2));
                esi = m.read32(ecx.wrapping_add(eax.wrapping_mul(0x8)).wrapping_add(0x11398));
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31fe42; continue; }
                pc = 0x31fe3d; continue;
            }
            0x31fe3d => {
                eax = m.read32(ebx.wrapping_add(((0x4i32.wrapping_neg()) as u32)));
                pc = 0x31fe44; continue;
            }
            0x31fe42 => {
                eax = m.read32(ebx);
                pc = 0x31fe44; continue;
            }
            0x31fe44 => {
                push!(eax);
                call!(0x3d7b0);
                pc = 0x31fe4a; continue;
            }
            0x31fe4a => {
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = 0x31fc20;
                if (fa != fb) { pc = 0x31fe5b; continue; }
                pc = 0x31fe56; continue;
            }
            0x31fe56 => {
                ecx = 0x31fcd0;
                pc = 0x31fe5b; continue;
            }
            0x31fe5b => {
                push!(0);
                push!(ecx);
                push!(0);
                push!(0x800);
                push!(edi);
                push!(eax);
                call!(0x79d60);
                pc = 0x31fe6c; continue;
            }
            0x31fe6c => {
                esp = esp.wrapping_add(0x18);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x31fe7a; continue; }
                pc = 0x31fe73; continue;
            }
            0x31fe73 => {
                push!(0xd7);
                pc = 0x31fe7f; continue;
            }
            0x31fe7a => {
                push!(0xd6);
                pc = 0x31fe7f; continue;
            }
            0x31fe7f => {
                call!(0x716d0);
                pc = 0x31fe84; continue;
            }
            0x31fe84 => {
                esp = esp.wrapping_add(0x4);
                ebx = ebx.wrapping_add(0x20);
                edi = edi.wrapping_add(1);
                fa = ebx;
                fb = ((((0x52e9d4i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x31fe20; continue; }
                pc = 0x31fe93; continue;
            }
            0x31fe93 => {
                push!(0x40000000);
                call!(0x784a0);
                pc = 0x31fe9d; continue;
            }
            0x31fe9d => {
                esp = esp.wrapping_add(0x4);
                push!(0);
                push!(0x361710);
                push!(0);
                push!(0);
                push!(0xffffffffu32);
                push!(0xb5e);
                call!(0x3d7b0);
                pc = 0x31feb7; continue;
            }
            0x31feb7 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x79d60);
                pc = 0x31fec0; continue;
            }
            0x31fec0 => {
                call!(0x22b90);
                pc = 0x31fec5; continue;
            }
            0x31fec5 => {
                xmm0 = Xmm::scalar(m.read32(0x7ba188));
                ecx = m.read32(0x7bb1e8);
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::mulss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(m.read32(0x475d24))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d30))).to_bits();
                xmm1.lanes[0] = (((eax) as i32) as f32).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba184))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x7ba030))).to_bits();
                esp = esp.wrapping_add(0x14);
                m.write32(esp, xmm1.lanes[0]);
                push!(ecx);
                call!(0x77320);
                pc = 0x31ff10; continue;
            }
            0x31ff10 => {
                esp = esp.wrapping_add(0x8);
                call!(0x75e70);
                pc = 0x31ff18; continue;
            }
            0x31ff18 => {
                eax = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(eax.wrapping_add(0x218));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = (ecx | 0x4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                m.write32(eax.wrapping_add(0x218), ecx);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00320250: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f320250<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let mut fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x320250;
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
        { crate::pose::cover(0x320250, previous, pc); previous = pc; }
        match pc {
            0x320250 => {
                esp = esp.wrapping_sub(0x10);
                eax = m.read32(esp.wrapping_add(0x1c));
                xmm0 = Xmm::scalar(m.read32(0x7ba038));
                edx = m.read32(esp.wrapping_add(0x18));
                push!(ebx);
                xmm1 = xmm0;
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x475d10))).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(0x475d18))).to_bits();
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x1c));
                m.write32(esp.wrapping_add(0x14), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                push!(esi);
                m.write32(esp.wrapping_add(0x28), xmm1.lanes[0]);
                xmm1 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                push!(edi);
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::subss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(eax))).to_bits();
                eax = esp.wrapping_add(0x14);
                push!(eax);
                ecx = esp.wrapping_add(0x1c);
                push!(ecx);
                push!(edx);
                push!(ebp);
                edi = 0;
                esi = 0;
                m.write32(esp.wrapping_add(0x20), xmm1.lanes[0]);
                call!(0x75f60);
                pc = 0x3202bb; continue;
            }
            0x3202bb => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x11858));
                esp = esp.wrapping_add(0x10);
                fa = ((((m.read16(ecx.wrapping_add(0xc)))) as u32) & 0xffffu32);
                fb = ((0xffffffffu32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas <= fbs) { pc = 0x320358; continue; }
                pc = 0x3202d4; continue;
            }
            0x3202d4 => {
                ecx = (((((m.read16(ecx.wrapping_add(0xc)) as i16)) as i32)) as u32);
                edx = ecx;
                edx = ((edx as i32).wrapping_mul((0x8c) as i32)) as u32;
                push!(ecx);
                ebx = edx.wrapping_add(eax).wrapping_add(0xdd14);
                call!(0x2fe520);
                pc = 0x3202ed; continue;
            }
            0x3202ed => {
                ecx = 0;
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                ecx = ecx.wrapping_add(0xb86);
                edi = ecx;
                ecx = m.read32(ebx.wrapping_add(0x7c));
                ecx = ecx.wrapping_sub(esi);
                fa = ecx;
                if ((fa == 0)) { pc = 0x320351; continue; }
                pc = 0x320306; continue;
            }
            0x320306 => {
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x320319; continue; }
                pc = 0x320309; continue;
            }
            0x320309 => {
                edx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edx = (edx & 0xffff_ff00) | ((((if ((fa != fb)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edx = edx.wrapping_sub(1);
                edx = (edx & 0xb85);
                esi = edx;
                pc = 0x320319; continue;
            }
            0x320319 => {
                eax = m.read32(0x7844a8);
                pc = 0x32031e; continue;
            }
            0x32031e => {
                ecx = m.read32(eax.wrapping_add(0x11858));
                edx = m.read32(ecx.wrapping_add(0x4));
                fa = edx;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x320382; continue; }
                pc = 0x32032c; continue;
            }
            0x32032c => {
                fa = edx;
                fb = ((((0xdi32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x320387; continue; }
                pc = 0x320331; continue;
            }
            0x320331 => {
                edx = (((((m.read16(ecx.wrapping_add(0xc)) as i16)) as i32)) as u32);
                edx = ((edx as i32).wrapping_mul((0x8c) as i32)) as u32;
                eax = m.read32(edx.wrapping_add(eax).wrapping_add(0xdd98));
                eax = ((eax as i32).wrapping_mul((0xc4) as i32)) as u32;
                esi = (((((m.read16(eax.wrapping_add(0x524f58)) as i16)) as i32)) as u32);
                pc = 0x320387; continue;
            }
            0x320351 => {
                esi = 0xb84;
                pc = 0x320319; continue;
            }
            0x320358 => {
                ecx = (ecx & 0xffff_0000) | ((((m.read16(ecx.wrapping_add(0xe))) as u32)) & 0xffff);
                fa = ((((((ecx) as u16))) as u32) & 0xffffu32);
                fb = ((0xffffffffu32) & 0xffffu32);
                fas = (((((fa)) as i16)) as i32);
                fbs = (((((fb)) as i16)) as i32);
                if (fas <= fbs) { pc = 0x32031e; continue; }
                pc = 0x320362; continue;
            }
            0x320362 => {
                ecx = ((((ecx) as u16)) as u16 as i16 as i32 as u32);
                ecx = ((ecx as i32).wrapping_mul((0xe4) as i32)) as u32;
                edx = m.read32(ecx.wrapping_add(eax).wrapping_add(0x2e0));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                edi = 0xbae;
                if (fa != fb) { pc = 0x32031e; continue; }
                pc = 0x32037b; continue;
            }
            0x32037b => {
                esi = 0xb84;
                pc = 0x32031e; continue;
            }
            0x320382 => {
                esi = 0xbd7;
                pc = 0x320387; continue;
            }
            0x320387 => {
                push!(0);
                push!(ebp);
                call!(0x70ef0);
                pc = 0x32038f; continue;
            }
            0x32038f => {
                push!(eax);
                call!(0x3fa10);
                pc = 0x320395; continue;
            }
            0x320395 => {
                ebx = m.read32(esp.wrapping_add(0x1c));
                esp = esp.wrapping_add(0xc);
                push!(0x2);
                push!(ebx);
                push!(edi);
                call!(0x3d7b0);
                pc = 0x3203a5; continue;
            }
            0x3203a5 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x42420);
                pc = 0x3203ae; continue;
            }
            0x3203ae => {
                esp = esp.wrapping_add(0xc);
                push!(ebx);
                push!(edi);
                call!(0x3d7b0);
                pc = 0x3203b8; continue;
            }
            0x3203b8 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x40f00);
                pc = 0x3203c1; continue;
            }
            0x3203c1 => {
                ecx = m.read32(esp.wrapping_add(0x1c));
                edx = m.read32(esp.wrapping_add(0x20));
                ebp = m.read32(esp.wrapping_add(0x24));
                push!(ecx);
                push!(edx);
                edi = eax;
                eax = m.read32(esp.wrapping_add(0x3c));
                push!(edi);
                push!(eax);
                push!(ebp);
                call!(0x41e30);
                pc = 0x3203dd; continue;
            }
            0x3203dd => {
                push!(edi);
                call!(0x40620);
                pc = 0x3203e3; continue;
            }
            0x3203e3 => {
                { let value = round_pc(fpu.st(0) + (f32::from_bits(m.read32(esp.wrapping_add(0x4c))) as f64), fpu.control); fpu.stack[((fpu.top + 0) & 7) as usize] = value; }
                ecx = m.read32(esp.wrapping_add(0x44));
                push!(0x100);
                push!(ecx);
                m.write32(esp.wrapping_add(0x54), (fpu.st(0) as f32).to_bits()); fpu.pop();
                call!(0x70ef0);
                pc = 0x3203fa; continue;
            }
            0x3203fa => {
                push!(eax);
                call!(0x3fa10);
                pc = 0x320400; continue;
            }
            0x320400 => {
                esp = esp.wrapping_add(0x2c);
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x32043e; continue; }
                pc = 0x320407; continue;
            }
            0x320407 => {
                push!(0);
                push!(ebx);
                push!(esi);
                call!(0x3d7b0);
                pc = 0x320410; continue;
            }
            0x320410 => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                call!(0x40ad0);
                pc = 0x320419; continue;
            }
            0x320419 => {
                esp = esp.wrapping_add(0xc);
                push!(0x14140a32);
                push!(0x7f503c7f);
                push!(esi);
                call!(0x3d7b0);
                pc = 0x32042c; continue;
            }
            0x32042c => {
                edx = m.read32(esp.wrapping_add(0x38));
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(edx);
                push!(ebp);
                call!(0x41e30);
                pc = 0x32043b; continue;
            }
            0x32043b => {
                esp = esp.wrapping_add(0x14);
                pc = 0x32043e; continue;
            }
            0x32043e => {
                push!(0x3f800000);
                push!(0x3f800000);
                push!(0x3f800000);
                call!(0x3f9c0);
                pc = 0x320452; continue;
            }
            0x320452 => {
                esp = esp.wrapping_add(0xc);
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

/// 0x00320580: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f320580<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x320580;
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
        { crate::pose::cover(0x320580, previous, pc); previous = pc; }
        match pc {
            0x320580 => {
                esp = esp.wrapping_sub(0x10);
                eax = m.read32(esp.wrapping_add(0x1c));
                xmm0 = Xmm::scalar(m.read32(eax));
                xmm1 = Xmm::scalar(m.read32(0x7ba188));
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(m.read32(0x49f824))).to_bits();
                m.write32(esp, xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x8)));
                m.write32(esp.wrapping_add(0x8), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0x4)));
                m.write32(esp.wrapping_add(0x4), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0xc)));
                m.write32(esp.wrapping_add(0xc), xmm0.lanes[0]);
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm1.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0x320601; continue; }
                pc = 0x3205cb; continue;
            }
            0x3205cb => {
                xmm2 = Xmm::scalar(m.read32(0x475cd4));
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0xc)));
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                xmm0.lanes[0] = sse::mulss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0x4)))).to_bits();
                xmm1.lanes[0] = sse::mulss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                xmm2 = xmm0;
                xmm2.lanes[0] = sse::subss(f32::from_bits(xmm2.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                xmm1.lanes[0] = sse::addss(f32::from_bits(xmm1.lanes[0]), f32::from_bits(xmm0.lanes[0])).to_bits();
                m.write32(esp.wrapping_add(0x4), xmm2.lanes[0]);
                m.write32(esp.wrapping_add(0xc), xmm1.lanes[0]);
                pc = 0x320601; continue;
            }
            0x320601 => {
                eax = m.read32(0x7844a8);
                ecx = m.read32(eax.wrapping_add(0x11858));
                edx = (((((m.read16(ecx.wrapping_add(0xe)) as i16)) as i32)) as u32);
                push!(0);
                push!(0xa0a0a20);
                push!(edx);
                call!(0x308af0);
                pc = 0x32061d; continue;
            }
            0x32061d => {
                esp = esp.wrapping_add(0x4);
                push!(eax);
                push!(0x42);
                call!(0x81c10);
                pc = 0x320628; continue;
            }
            0x320628 => {
                ecx = m.read32(esp.wrapping_add(0x28));
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = esp.wrapping_add(0x10);
                push!(eax);
                push!(ecx);
                call!(0x757e0);
                pc = 0x32063b; continue;
            }
            0x32063b => {
                edx = m.read32(esp.wrapping_add(0x2c));
                esp = esp.wrapping_add(0x4);
                push!(eax);
                eax = m.read32(edx.wrapping_add(0xf4));
                push!(eax);
                call!(0x315e50);
                pc = 0x32064f; continue;
            }
            0x32064f => {
                esp = esp.wrapping_add(0x2c);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}

/// 0x00321F20: Tier 0, bounded zero-state smoke; no differential evidence.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn f321f20<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x321f20;
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
        { crate::pose::cover(0x321f20, previous, pc); previous = pc; }
        match pc {
            0x321f20 => {
                eax = m.read32(esp.wrapping_add(0x4));
                edx = m.read32(0x7844a8);
                eax = ((eax as i32).wrapping_mul((0x220) as i32)) as u32;
                push!(esi);
                esi = m.read32(eax.wrapping_add(edx).wrapping_add(0x426c));
                ecx = 0;
                fa = esi;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                ecx = (ecx & 0xffff_ff00) | ((((if ((fas >= fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ecx;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}
}
pub use unformatted::*;
